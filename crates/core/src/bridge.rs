//! What the engine asks of the outside world, as traits the apps inject: a renderer that
//! turns the quote's HTML (and letterhead) into DOCX and PDF, and a sender that mails the
//! files. The `bridge` crate implements them with WordCraft and MailCraft; tests use doubles.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::config::{Preferences, Secrets};
use crate::{Error, Result};

/// A document to produce.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderJob {
    /// The quote's body (no `<html>`): goes into the letterhead at `{{corpo}}`.
    pub body_html: String,
    /// A whole page with the company header: the document when there is no letterhead.
    pub page_html: String,
    /// `{{field}}` values for the letterhead.
    pub fields: BTreeMap<String, String>,
    /// The tables a letterhead can place on its own (`tabella.canone` → HTML fragment).
    pub tables: BTreeMap<String, String>,
    /// The repeating sections of an HTML letterhead (`righe` → one field map per line).
    pub sections: BTreeMap<String, Vec<BTreeMap<String, String>>>,
    /// The drawings, in order (network diagrams, plans): the quote's pictures with captions.
    pub drawings: Vec<Picture>,
    /// The letterhead (`.docx` or `.html`), if any.
    pub template: Option<PathBuf>,
    /// Where the files go (created).
    pub out_dir: PathBuf,
    /// File name without extension: `2026-0001-rossi-srl`.
    pub base: String,
    /// `docx`, `pdf` (the HTML is always written).
    pub formats: Vec<String>,
}

/// A drawing of a render job: the picture's bytes and its caption.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Picture {
    /// The file name (among the quote's documents).
    pub name: String,
    pub caption: String,
    pub data: Vec<u8>,
}

/// A file produced.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Produced {
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
}

/// Makes DOCX/PDF out of a job (the HTML is written by the engine beforehand).
pub trait Renderer: Send {
    fn render(&self, job: &RenderJob) -> Result<Vec<Produced>>;
    /// A line saying what will do the work (for the settings screen).
    fn describe(&self) -> String;
}

pub type RendererFactory = Arc<dyn Fn(&Preferences) -> Box<dyn Renderer> + Send + Sync>;

/// A mail to send with the documents attached.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutgoingMail {
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub subject: String,
    pub body: String,
    pub attachments: Vec<PathBuf>,
}

/// Sends the mail as `user` (MailCraft: the user's own account).
pub trait Sender: Send {
    fn send(&self, mail: &OutgoingMail, user: &str) -> Result<String>;
    fn describe(&self) -> String;
}

pub type SenderFactory = Arc<dyn Fn(&Preferences, &Secrets) -> Result<Box<dyn Sender>> + Send + Sync>;

/// Fills an HTML letterhead: the `{{#righe}}…{{/righe}}` sections expanded, placeholders
/// replaced, the tables (`{{tabella.canone}}`), the drawings (`{{schema.1}}`, `{{schemi}}`) and
/// the body at `{{corpo}}`. Without `{{corpo}}`, the body goes at the end of `<body>` (or is
/// appended), unless the letterhead lays the lines out itself with tables or sections. The
/// drawings the letterhead does not place follow the body (or end the page).
pub fn fill_html_template(template: &str, job: &RenderJob) -> String {
    let lays_out_lines = template.contains("{{tabella.") || template.contains("{{#righe");
    let filled = crate::render::fill_sections(template, &job.sections);
    let filled = crate::render::fill(&filled, &job.fields);
    let filled = crate::render::fill(&filled, &job.tables);
    let (filled, rest) = crate::render::place_drawings(&filled, &job.drawings);
    let rest = crate::render::drawings_section(&rest);
    let body = format!("{}{rest}", job.body_html);
    if filled.contains("{{corpo}}") {
        filled.replace("{{corpo}}", &body)
    } else if lays_out_lines {
        crate::render::append_to_body(&filled, &rest)
    } else {
        crate::render::append_to_body(&filled, &body)
    }
}

/// Writes the HTML of a job: the letterhead filled, or the standalone page.
pub fn write_html(job: &RenderJob) -> Result<Produced> {
    std::fs::create_dir_all(&job.out_dir).map_err(|e| Error::Io(format!("{}: {e}", job.out_dir.display())))?;
    let html = match &job.template {
        Some(t) if t.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm")) => {
            let text = std::fs::read_to_string(t).map_err(|e| Error::Io(format!("{}: {e}", t.display())))?;
            fill_html_template(&text, job)
        }
        _ => crate::render::append_to_body(&job.page_html, &crate::render::drawings_section(&job.drawings.iter().collect::<Vec<_>>())),
    };
    let name = format!("{}.html", job.base);
    let path = job.out_dir.join(&name);
    std::fs::write(&path, &html).map_err(|e| Error::Io(format!("{}: {e}", path.display())))?;
    Ok(Produced { name, path, bytes: html.len() as u64 })
}

/// A letterhead's name is a plain file name inside the templates folder.
pub fn template_path(dir: &Path, name: &str) -> Result<PathBuf> {
    let n = name.trim();
    if n.is_empty() || n.contains('/') || n.contains('\\') || n.contains("..") || n.starts_with('.') {
        return Err(Error::Invalid(format!("template name {n:?}")));
    }
    let p = dir.join(n);
    if !p.is_file() {
        return Err(Error::NotFound(format!("template {n} in {}", dir.display())));
    }
    Ok(p)
}

/// A renderer that makes nothing beyond the HTML (no WordCraft: the web build, tests).
pub struct NoRenderer;

impl Renderer for NoRenderer {
    fn render(&self, _job: &RenderJob) -> Result<Vec<Produced>> {
        Err(Error::NotConfigured("no document renderer: install wordcraft-cli and name it in the preferences".into()))
    }
    fn describe(&self) -> String {
        "HTML only (no wordcraft-cli)".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(dir: &Path) -> RenderJob {
        let mut fields = BTreeMap::new();
        fields.insert("numero".to_string(), "2026-0001".to_string());
        let mut tables = BTreeMap::new();
        tables.insert("tabella.canone".to_string(), "<table>c</table>".to_string());
        let mut sections = BTreeMap::new();
        let row = |n: &str| BTreeMap::from([("riga.nome".to_string(), n.to_string())]);
        sections.insert("righe".to_string(), vec![row("A"), row("B")]);
        RenderJob {
            body_html: "<h1>Body</h1>".into(),
            page_html: "<html><body>Page</body></html>".into(),
            fields,
            tables,
            sections,
            drawings: Vec::new(),
            template: None,
            out_dir: dir.to_path_buf(),
            base: "2026-0001-x".into(),
            formats: vec!["pdf".into()],
        }
    }

    #[test]
    fn html_templates_are_filled_and_written() {
        let dir = std::env::temp_dir().join(format!("quotecraft-bridge-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut j = job(&dir.join("out"));
        assert_eq!(fill_html_template("<p>N. {{numero}}</p>{{corpo}}<p>fine</p>", &j), "<p>N. 2026-0001</p><h1>Body</h1><p>fine</p>");
        assert_eq!(fill_html_template("<body><p>x</p></body>", &j), "<body><p>x</p><h1>Body</h1></body>");
        assert_eq!(fill_html_template("plain", &j), "plain<h1>Body</h1>");
        assert_eq!(
            fill_html_template("<ul>{{#righe}}<li>{{riga.nome}}</li>{{/righe}}</ul>{{tabella.canone}}{{corpo}}", &j),
            "<ul><li>A</li><li>B</li></ul><table>c</table><h1>Body</h1>"
        );
        assert_eq!(
            fill_html_template("<body>{{tabella.canone}}</body>", &j),
            "<body><table>c</table></body>",
            "a letterhead that lays the lines out itself gets no body appended"
        );
        let p = write_html(&j).unwrap();
        assert_eq!(std::fs::read_to_string(&p.path).unwrap(), "<html><body>Page</body></html>");
        std::fs::write(dir.join("carta.html"), "<body>{{numero}} {{corpo}}</body>").unwrap();
        j.template = Some(template_path(&dir, "carta.html").unwrap());
        let p = write_html(&j).unwrap();
        assert_eq!(std::fs::read_to_string(&p.path).unwrap(), "<body>2026-0001 <h1>Body</h1></body>");
        assert!(template_path(&dir, "../carta.html").is_err());
        assert!(template_path(&dir, "missing.docx").is_err());
        assert!(NoRenderer.render(&j).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn drawings_go_where_the_letterhead_says_or_after_the_body() {
        let dir = std::env::temp_dir();
        let mut j = job(&dir);
        let pic = |name: &str, caption: &str| Picture { name: name.into(), caption: caption.into(), data: crate::picture::tests::png(400, 200) };
        j.drawings = vec![pic("rete.png", "Rete attuale"), pic("nuova.png", "Rete <proposta>")];
        let count = |s: &str| s.matches("<img src=\"data:image/png;base64,").count();
        // One by one, then the rest at {{schemi}}.
        let out = fill_html_template("{{schema.2}}|{{corpo}}|{{schemi}}|{{schema.9}}", &j);
        let parts: Vec<&str> = out.split('|').collect();
        assert_eq!(parts.len(), 4, "{out}");
        assert!(count(parts[0]) == 1 && parts[0].contains("Rete &lt;proposta&gt;"), "{}", parts[0]);
        assert_eq!(parts[1], "<h1>Body</h1>", "placed drawings do not follow the body");
        assert!(count(parts[2]) == 1 && parts[2].contains("<i>Rete attuale</i>"));
        assert_eq!(parts[3], "", "no ninth drawing: nothing");
        assert!(parts[0].contains("width=\"400\" height=\"200\""), "natural size when it fits");
        // No drawing placeholder: after the body, with a heading.
        let out = fill_html_template("<body>{{corpo}}<p>fine</p></body>", &j);
        assert!(
            out.starts_with("<body><h1>Body</h1><h2 style=\"page-break-before:always\">Schemi</h2>")
                && count(&out) == 2
                && out.ends_with("<p>fine</p></body>"),
            "{out}"
        );
        // A letterhead that lays the lines out itself: at the end of the page.
        let out = fill_html_template("<body>{{tabella.canone}}</body>", &j);
        assert!(out.starts_with("<body><table>c</table><h2 style=\"page-break-before:always\">Schemi</h2>") && !out.contains("Body"), "{out}");
        // The built-in page: at the end.
        let p = write_html(&j).unwrap();
        let page = std::fs::read_to_string(&p.path).unwrap();
        assert!(page.starts_with("<html><body>Page<h2 style=\"page-break-before:always\">Schemi</h2>") && count(&page) == 2, "{page}");
        let _ = std::fs::remove_file(&p.path);
        // Not a picture, or no drawings: nothing, and the placeholders disappear.
        j.drawings = vec![Picture { name: "x.png".into(), caption: "x".into(), data: b"not a picture".to_vec() }];
        assert_eq!(fill_html_template("a{{schemi}}b{{schema.1}}c{{corpo}}", &j), "abc<h1>Body</h1>");
        j.drawings.clear();
        assert_eq!(fill_html_template("a{{schemi}}{{schema.x}}{{corpo}}", &j), "a{{schema.x}}<h1>Body</h1>", "an unknown placeholder stays visible");
    }
}

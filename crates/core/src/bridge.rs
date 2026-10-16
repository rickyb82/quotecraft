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
    /// The letterhead (`.docx` or `.html`), if any.
    pub template: Option<PathBuf>,
    /// Where the files go (created).
    pub out_dir: PathBuf,
    /// File name without extension: `2026-0001-rossi-srl`.
    pub base: String,
    /// `docx`, `pdf` (the HTML is always written).
    pub formats: Vec<String>,
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

/// Fills an HTML letterhead: placeholders replaced, the body at `{{corpo}}` (or at the end
/// of `<body>`, or appended).
pub fn fill_html_template(template: &str, job: &RenderJob) -> String {
    let filled = crate::render::fill(template, &job.fields);
    if filled.contains("{{corpo}}") {
        filled.replace("{{corpo}}", &job.body_html)
    } else if let Some(i) = filled.rfind("</body>") {
        let (a, b) = filled.split_at(i);
        format!("{a}{}{b}", job.body_html)
    } else {
        format!("{filled}{}", job.body_html)
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
        _ => job.page_html.clone(),
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
        RenderJob {
            body_html: "<h1>Body</h1>".into(),
            page_html: "<html><body>Page</body></html>".into(),
            fields,
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
}

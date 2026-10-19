//! DOCX and PDF through `wordcraft-cli`:
//!
//! - no letterhead, or an HTML one: `wordcraft-cli convert quote.html quote.docx` (and `.pdf`),
//!   the HTML being the filled page the engine wrote;
//! - a DOCX letterhead: `wordcraft-cli run --file letterhead.docx --cmd edit.replaceAll=…`
//!   for every placeholder, `edit.find {{corpo}}` + `insert.textFromFile body.html` (and the
//!   same for every `{{tabella.…}}` the letterhead carries, found with `wordcraft-cli text`),
//!   `file.exportPdf`, `--save quote.docx`.

use std::path::{Path, PathBuf};
use std::process::Command;

use quotecraft_core::bridge::{Produced, RenderJob, Renderer};
use quotecraft_core::{Error, Result};
use serde_json::json;

pub struct WordCraft {
    cli: String,
}

impl WordCraft {
    pub fn new(cli: &str) -> Self {
        Self { cli: if cli.trim().is_empty() { "wordcraft-cli".into() } else { cli.trim().to_string() } }
    }

    fn run(&self, args: &[String]) -> Result<String> {
        let out = Command::new(&self.cli)
            .args(args)
            .output()
            .map_err(|e| Error::NotConfigured(format!("{}: {e} (install WordCraft's CLI or name it in the preferences)", self.cli)))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            let err = err.trim().lines().last().unwrap_or("failed").to_string();
            return Err(Error::External(format!("{} {}: {err}", self.cli, args.first().map(String::as_str).unwrap_or_default())));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn produced(path: &Path) -> Result<Produced> {
        let meta = std::fs::metadata(path).map_err(|e| Error::External(format!("{}: {e}", path.display())))?;
        Ok(Produced {
            name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            path: path.to_path_buf(),
            bytes: meta.len(),
        })
    }

    /// The text of a document (`wordcraft-cli text`), to see which placeholders it carries;
    /// `None` when the CLI cannot say (an old version): everything is then assumed present.
    fn text_of(&self, path: &Path) -> Option<String> {
        self.run(&["text".to_string(), path.to_string_lossy().into_owned()]).ok()
    }

    /// The arguments of the `run` that fills a DOCX letterhead: every field replaced, then
    /// every `(placeholder, file)` of `inserts` found and replaced by the file's HTML.
    pub fn letterhead_args(job: &RenderJob, template: &Path, inserts: &[(String, PathBuf)], docx: &Path, pdf: Option<&Path>) -> Vec<String> {
        let mut args = vec!["run".to_string(), "--file".into(), template.to_string_lossy().into_owned()];
        for (k, v) in &job.fields {
            let cmd = json!({"text": format!("{{{{{k}}}}}"), "with": v, "matchCase": true});
            args.push("--cmd".into());
            args.push(format!("edit.replaceAll={cmd}"));
        }
        for (placeholder, file) in inserts {
            args.push("--cmd".into());
            args.push(format!("edit.find={}", json!({"text": placeholder, "matchCase": true})));
            args.push("--cmd".into());
            args.push(format!("insert.textFromFile={}", json!({"path": file.to_string_lossy()})));
        }
        if let Some(p) = pdf {
            args.push("--cmd".into());
            args.push(format!("file.exportPdf={}", json!({"path": p.to_string_lossy()})));
        }
        args.push("--save".into());
        args.push(docx.to_string_lossy().into_owned());
        args
    }

    /// What goes into a DOCX letterhead: the body at `{{corpo}}` and the tables it names.
    /// Without `{{corpo}}` but with tables or line rows of its own (`{{riga.…}}`), the body
    /// stays out (the letterhead lays the lines out itself); without any of them, the body is
    /// inserted where the cursor is, as before.
    pub fn inserts_for(job: &RenderJob, text: Option<&str>) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let tables: Vec<&String> = job.tables.keys().filter(|k| text.is_none_or(|t| t.contains(&format!("{{{{{k}}}}}")))).collect();
        let own_rows = text.is_some_and(|t| t.contains("{{riga.") || t.contains("{{#righe"));
        if text.is_none_or(|t| t.contains("{{corpo}}")) || (tables.is_empty() && !own_rows) {
            out.push(("{{corpo}}".to_string(), "body".to_string()));
        }
        for k in tables {
            out.push((format!("{{{{{k}}}}}"), k.clone()));
        }
        out
    }
}

impl Renderer for WordCraft {
    fn render(&self, job: &RenderJob) -> Result<Vec<Produced>> {
        std::fs::create_dir_all(&job.out_dir).map_err(|e| Error::Io(format!("{}: {e}", job.out_dir.display())))?;
        let want = |f: &str| job.formats.iter().any(|x| x.eq_ignore_ascii_case(f));
        let docx: PathBuf = job.out_dir.join(format!("{}.docx", job.base));
        let pdf: PathBuf = job.out_dir.join(format!("{}.pdf", job.base));
        let html: PathBuf = job.out_dir.join(format!("{}.html", job.base));
        let mut out = Vec::new();
        let is_docx_template =
            job.template.as_ref().is_some_and(|t| t.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("docx")));
        if is_docx_template {
            let Some(original) = &job.template else { return Ok(out) };
            // The letterhead's own line rows written out first (`{{riga.…}}`), into a copy.
            let bytes = std::fs::read(original).map_err(|e| Error::Io(format!("{}: {e}", original.display())))?;
            let expanded = crate::docx::expand_docx(&bytes, &job.sections)?;
            let copy = job.out_dir.join(format!("{}.letterhead.docx", job.base));
            if let Some(b) = &expanded {
                std::fs::write(&copy, b).map_err(|e| Error::Io(format!("{}: {e}", copy.display())))?;
            }
            let template = if expanded.is_some() { &copy } else { original };
            // The body and the tables, each a file, for insertion into the letterhead.
            let text = self.text_of(original);
            let mut inserts = Vec::new();
            let mut files = Vec::new();
            for (placeholder, key) in Self::inserts_for(job, text.as_deref()) {
                let html = if key == "body" { &job.body_html } else { job.tables.get(&key).map(String::as_str).unwrap_or_default() };
                let file = job.out_dir.join(format!("{}.{key}.html", job.base));
                std::fs::write(&file, format!("<!doctype html><html><body>{html}</body></html>"))
                    .map_err(|e| Error::Io(format!("{}: {e}", file.display())))?;
                inserts.push((placeholder, file.clone()));
                files.push(file);
            }
            let args = Self::letterhead_args(job, template, &inserts, &docx, want("pdf").then_some(pdf.as_path()));
            let r = self.run(&args);
            for f in files {
                let _ = std::fs::remove_file(f);
            }
            if expanded.is_some() {
                let _ = std::fs::remove_file(&copy);
            }
            r?;
            if want("docx") {
                out.push(Self::produced(&docx)?);
            } else {
                let _ = std::fs::remove_file(&docx);
            }
            if want("pdf") {
                out.push(Self::produced(&pdf)?);
            }
        } else {
            // The filled page (written by the engine) converted as it is.
            if !html.is_file() {
                return Err(Error::Io(format!("{}: the HTML must be written first", html.display())));
            }
            for (fmt, path) in [("docx", &docx), ("pdf", &pdf)] {
                if want(fmt) {
                    self.run(&["convert".to_string(), html.to_string_lossy().into_owned(), path.to_string_lossy().into_owned()])?;
                    out.push(Self::produced(path)?);
                }
            }
        }
        Ok(out)
    }

    fn describe(&self) -> String {
        match Command::new(&self.cli).arg("--version").output() {
            Ok(o) if o.status.success() => format!("{} ({})", self.cli, String::from_utf8_lossy(&o.stdout).trim()),
            _ => format!("{} (not found)", self.cli),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn letterhead_arguments_replace_fields_and_insert_the_body() {
        let mut fields = BTreeMap::new();
        fields.insert("numero".to_string(), "2026-0001".to_string());
        let job = RenderJob {
            body_html: "<p>b</p>".into(),
            page_html: String::new(),
            fields,
            tables: BTreeMap::from([("tabella.canone".to_string(), "<table>c</table>".to_string())]),
            sections: BTreeMap::new(),
            template: Some(PathBuf::from("/t/carta.docx")),
            out_dir: PathBuf::from("/o"),
            base: "q".into(),
            formats: vec!["docx".into(), "pdf".into()],
        };
        let inserts = vec![
            ("{{corpo}}".to_string(), PathBuf::from("/o/q.body.html")),
            ("{{tabella.canone}}".to_string(), PathBuf::from("/o/q.tabella.canone.html")),
        ];
        let args = WordCraft::letterhead_args(&job, Path::new("/t/carta.docx"), &inserts, Path::new("/o/q.docx"), Some(Path::new("/o/q.pdf")));
        let joined = args.join(" ");
        assert!(
            joined.starts_with("run --file /t/carta.docx --cmd edit.replaceAll={\"matchCase\":true,\"text\":\"{{numero}}\",\"with\":\"2026-0001\"}"),
            "{joined}"
        );
        assert!(
            joined.contains("edit.find={\"matchCase\":true,\"text\":\"{{corpo}}\"}")
                && joined.contains("insert.textFromFile=")
                && joined.contains("file.exportPdf=")
        );
        assert!(joined.ends_with("--save /o/q.docx"));
        assert!(
            joined.contains(
                "edit.find={\"matchCase\":true,\"text\":\"{{tabella.canone}}\"} --cmd insert.textFromFile={\"path\":\"/o/q.tabella.canone.html\"}"
            ),
            "{joined}"
        );
        // Which inserts, from the letterhead's text.
        let keys = |t: Option<&str>| WordCraft::inserts_for(&job, t).into_iter().map(|(_, k)| k).collect::<Vec<_>>();
        assert_eq!(keys(None), ["body", "tabella.canone"], "unknown text: everything");
        assert_eq!(keys(Some("Spett.le {{cliente.nome}} {{corpo}}")), ["body"]);
        assert_eq!(keys(Some("{{corpo}} e {{tabella.canone}}")), ["body", "tabella.canone"]);
        assert_eq!(keys(Some("solo {{tabella.canone}}")), ["tabella.canone"], "no corpo: the letterhead lays the lines out");
        assert_eq!(keys(Some("niente")), ["body"], "no placeholder at all: the body as before");
        assert!(keys(Some("Nr. {{riga.qta}} {{riga.nome}}")).is_empty(), "its own line rows: no body");
        assert!(keys(Some("{{#righe.canone}}{{riga.nome}}{{/righe.canone}}")).is_empty());
    }

    #[test]
    fn a_missing_cli_is_not_configured_and_the_html_must_exist() {
        let w = WordCraft::new("/definitely/not/wordcraft-cli");
        let dir = std::env::temp_dir().join(format!("quotecraft-wc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let job = RenderJob {
            body_html: String::new(),
            page_html: String::new(),
            fields: BTreeMap::new(),
            tables: BTreeMap::new(),
            sections: BTreeMap::new(),
            template: None,
            out_dir: dir.clone(),
            base: "q".into(),
            formats: vec!["pdf".into()],
        };
        let e = w.render(&job).unwrap_err().to_string();
        assert!(e.contains("HTML must be written first"), "{e}");
        std::fs::write(dir.join("q.html"), "<p>x</p>").unwrap();
        let e = w.render(&job).unwrap_err().to_string();
        assert!(e.contains("not configured") && e.contains("wordcraft-cli"), "{e}");
        assert!(w.describe().contains("not found"));
        assert_eq!(WordCraft::new("  ").cli, "wordcraft-cli");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

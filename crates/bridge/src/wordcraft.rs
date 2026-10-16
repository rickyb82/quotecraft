//! DOCX and PDF through `wordcraft-cli`:
//!
//! - no letterhead, or an HTML one: `wordcraft-cli convert quote.html quote.docx` (and `.pdf`),
//!   the HTML being the filled page the engine wrote;
//! - a DOCX letterhead: `wordcraft-cli run --file letterhead.docx --cmd edit.replaceAll=…`
//!   for every placeholder, `edit.find {{corpo}}` + `insert.textFromFile body.html`,
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

    /// The arguments of the `run` that fills a DOCX letterhead.
    pub fn letterhead_args(job: &RenderJob, template: &Path, body: &Path, docx: &Path, pdf: Option<&Path>) -> Vec<String> {
        let mut args = vec!["run".to_string(), "--file".into(), template.to_string_lossy().into_owned()];
        for (k, v) in &job.fields {
            let cmd = json!({"text": format!("{{{{{k}}}}}"), "with": v, "matchCase": true});
            args.push("--cmd".into());
            args.push(format!("edit.replaceAll={cmd}"));
        }
        args.push("--cmd".into());
        args.push(format!("edit.find={}", json!({"text": "{{corpo}}", "matchCase": true})));
        args.push("--cmd".into());
        args.push(format!("insert.textFromFile={}", json!({"path": body.to_string_lossy()})));
        if let Some(p) = pdf {
            args.push("--cmd".into());
            args.push(format!("file.exportPdf={}", json!({"path": p.to_string_lossy()})));
        }
        args.push("--save".into());
        args.push(docx.to_string_lossy().into_owned());
        args
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
            let Some(template) = &job.template else { return Ok(out) };
            // The body alone, for insertion into the letterhead.
            let body = job.out_dir.join(format!("{}.body.html", job.base));
            std::fs::write(&body, format!("<!doctype html><html><body>{}</body></html>", job.body_html))
                .map_err(|e| Error::Io(format!("{}: {e}", body.display())))?;
            let args = Self::letterhead_args(job, template, &body, &docx, want("pdf").then_some(pdf.as_path()));
            let r = self.run(&args);
            let _ = std::fs::remove_file(&body);
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
            template: Some(PathBuf::from("/t/carta.docx")),
            out_dir: PathBuf::from("/o"),
            base: "q".into(),
            formats: vec!["docx".into(), "pdf".into()],
        };
        let args = WordCraft::letterhead_args(
            &job,
            Path::new("/t/carta.docx"),
            Path::new("/o/q.body.html"),
            Path::new("/o/q.docx"),
            Some(Path::new("/o/q.pdf")),
        );
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

//! Sending through MailCraft's server: `POST /api/cmd` with `mail.send` (attachments as
//! base64), the framework's service token and `X-Mailcraft-User` naming the user, so the
//! mail goes out from the user's own account; then `/api/snapshot` until the job completes.

use base64::Engine;
use quotecraft_core::bridge::{OutgoingMail, Sender};
use quotecraft_core::{Error, Result, limits};
use serde_json::{Value, json};

pub struct MailCraft {
    base: String,
    token: String,
}

impl MailCraft {
    pub fn new(url: &str, token: &str) -> Result<Self> {
        let base = url.trim().trim_end_matches('/').to_string();
        if base.is_empty() {
            return Err(Error::NotConfigured("MailCraft's URL (preference mailcraft_url, e.g. http://mailcraft:7878)".into()));
        }
        if !(base.starts_with("http://") || base.starts_with("https://")) {
            return Err(Error::Invalid(format!("mailcraft_url {base:?}: must start with http:// or https://")));
        }
        Ok(Self { base, token: token.trim().to_string() })
    }

    fn headers(&self, user: &str, json: bool) -> ehttp::Headers {
        let mut pairs: Vec<(&str, String)> = vec![("Accept", "application/json".into())];
        if json {
            pairs.push(("Content-Type", "application/json".into()));
        }
        if !self.token.is_empty() {
            pairs.push(("Authorization", format!("Bearer {}", self.token)));
        }
        if !user.is_empty() {
            pairs.push(("X-Mailcraft-User", user.to_string()));
        }
        let borrowed: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (*k, v.as_str())).collect();
        ehttp::Headers::new(&borrowed)
    }

    fn post(&self, user: &str, body: Value) -> Result<Value> {
        let mut req = ehttp::Request::post(format!("{}/api/cmd", self.base), body.to_string().into_bytes());
        req.headers = self.headers(user, true);
        let r = ehttp::fetch_blocking(&req).map_err(|e| Error::External(format!("mailcraft {}: {}", self.base, e.trim_start_matches("io: "))))?;
        let v: Value = serde_json::from_slice(&r.bytes).map_err(|_| Error::External(format!("mailcraft: HTTP {} {}", r.status, r.status_text)))?;
        if let Some(e) = v.get("error").and_then(Value::as_str) {
            return Err(Error::External(format!("mailcraft: {e}")));
        }
        Ok(v.get("result").cloned().unwrap_or(Value::Null))
    }

    fn snapshot(&self, user: &str) -> Result<Value> {
        let mut req = ehttp::Request::get(format!("{}/api/snapshot?since=0", self.base));
        req.headers = self.headers(user, false);
        let r = ehttp::fetch_blocking(&req).map_err(|e| Error::External(format!("mailcraft: {e}")))?;
        serde_json::from_slice(&r.bytes).map_err(|_| Error::External(format!("mailcraft: HTTP {} {}", r.status, r.status_text)))
    }

    /// The `mail.send` parameters of a mail (attachments read and base64-encoded).
    pub fn params(mail: &OutgoingMail) -> Result<Value> {
        let mut attachments = Vec::new();
        for p in &mail.attachments {
            let data = std::fs::read(p).map_err(|e| Error::Io(format!("{}: {e}", p.display())))?;
            if data.len() > limits::ATTACHMENT {
                return Err(Error::Invalid(format!("{}: larger than {} MB", p.display(), limits::ATTACHMENT / 1024 / 1024)));
            }
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "allegato".into());
            attachments.push(json!({"name": name, "mime": mime_of(&name), "data": base64::engine::general_purpose::STANDARD.encode(&data)}));
        }
        Ok(json!({"to": mail.to.join(", "), "cc": mail.cc.join(", "), "subject": mail.subject, "body": mail.body, "attachments": attachments}))
    }
}

fn mime_of(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or_default().to_ascii_lowercase().as_str() {
        "pdf" => "application/pdf",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "html" | "htm" => "text/html",
        "txt" => "text/plain",
        _ => "application/octet-stream",
    }
}

impl Sender for MailCraft {
    fn send(&self, mail: &OutgoingMail, user: &str) -> Result<String> {
        if mail.to.iter().all(|t| t.trim().is_empty()) {
            return Err(Error::Invalid("the mail needs a recipient".into()));
        }
        let v = self.post(user, json!({"command": "mail.send", "params": Self::params(mail)?}))?;
        let Some(job) = v.get("job").and_then(Value::as_u64) else {
            return Ok("sent".into());
        };
        // The send runs on MailCraft's worker: wait for the job (up to a minute).
        let started = std::time::Instant::now();
        while started.elapsed() < std::time::Duration::from_secs(60) {
            std::thread::sleep(std::time::Duration::from_millis(400));
            let s = self.snapshot(user)?;
            if let Some(c) =
                s.get("completed").and_then(Value::as_array).and_then(|l| l.iter().find(|c| c.get("job").and_then(Value::as_u64) == Some(job)))
            {
                let err = c.get("error").and_then(Value::as_str).unwrap_or_default();
                return if err.is_empty() { Ok("sent".into()) } else { Err(Error::External(format!("mailcraft: {err}"))) };
            }
        }
        Ok("queued".into())
    }

    fn describe(&self) -> String {
        format!("MailCraft at {}{}", self.base, if self.token.is_empty() { " (no service token)" } else { "" })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Answers mail.send with a job, then a snapshot with the job completed.
    fn serve(listener: TcpListener, fail: bool) {
        for stream in listener.incoming().take(2) {
            let Ok(mut s) = stream else { continue };
            // Headers first, then the body up to Content-Length (it may arrive in a later read).
            let mut raw = Vec::new();
            let mut chunk = [0u8; 8192];
            loop {
                let n = s.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break;
                }
                raw.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&raw).to_string();
                if let Some(i) = text.find("\r\n\r\n") {
                    let len: usize = text
                        .to_ascii_lowercase()
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0)))
                        .unwrap_or(0);
                    if raw.len() >= i + 4 + len {
                        break;
                    }
                }
            }
            let req = String::from_utf8_lossy(&raw).to_string();
            let lower = req.to_ascii_lowercase();
            assert!(lower.contains("x-mailcraft-user: anna") && lower.contains("authorization: bearer tok"), "{req}");
            let body = if req.starts_with("POST") {
                assert!(req.contains("\"command\":\"mail.send\"") && req.contains("\"name\":\"q.pdf\"") && req.contains("application/pdf"), "{req}");
                r#"{"result":{"job":7}}"#.to_string()
            } else {
                format!(r#"{{"completed":[{{"job":7,"command":"mail.send","result":{{}},"error":"{}"}}]}}"#, if fail { "smtp: refused" } else { "" })
            };
            let _ = s.write_all(
                format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
                    .as_bytes(),
            );
        }
    }

    #[test]
    fn sends_as_the_user_and_waits_for_the_job() {
        let dir = std::env::temp_dir().join(format!("quotecraft-mc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pdf = dir.join("q.pdf");
        std::fs::write(&pdf, b"%PDF-1.4").unwrap();
        for fail in [false, true] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            std::thread::spawn(move || serve(listener, fail));
            let m = MailCraft::new(&format!("http://{addr}/"), "tok").unwrap();
            let mail = OutgoingMail {
                to: vec!["x@example.com".into()],
                subject: "s".into(),
                body: "b".into(),
                attachments: vec![pdf.clone()],
                ..Default::default()
            };
            let r = m.send(&mail, "anna");
            if fail {
                assert!(r.unwrap_err().to_string().contains("smtp: refused"));
            } else {
                assert_eq!(r.unwrap(), "sent");
            }
        }
        assert!(MailCraft::new("", "t").is_err());
        assert!(MailCraft::new("mailcraft:7878", "t").is_err());
        let dead = MailCraft::new("http://127.0.0.1:1", "t").unwrap();
        assert!(dead.send(&OutgoingMail { to: vec!["x@example.com".into()], ..Default::default() }, "anna").is_err());
        assert!(dead.send(&OutgoingMail::default(), "anna").unwrap_err().to_string().contains("recipient"));
        assert!(dead.describe().contains("127.0.0.1:1"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

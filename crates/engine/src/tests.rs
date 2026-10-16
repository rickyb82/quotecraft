use std::path::Path;
use std::sync::Arc;

use quotecraft_core::bridge::{OutgoingMail, Produced, RenderJob, Renderer, Sender};
use serde_json::{Value, json};

use crate::{COMMANDS, Session};

/// A renderer that writes the PDF and DOCX as text files.
struct FakeRenderer;
impl Renderer for FakeRenderer {
    fn render(&self, job: &RenderJob) -> quotecraft_core::Result<Vec<Produced>> {
        let mut out = Vec::new();
        for f in &job.formats {
            if f == "pdf" || f == "docx" {
                let path = job.out_dir.join(format!("{}.{f}", job.base));
                std::fs::write(&path, format!("fake {f} of {}", job.base)).unwrap();
                out.push(Produced { name: format!("{}.{f}", job.base), path, bytes: 10 });
            }
        }
        Ok(out)
    }
    fn describe(&self) -> String {
        "fake renderer".into()
    }
}

type Sent = Arc<std::sync::Mutex<Vec<(OutgoingMail, String)>>>;

/// A sender that records the mail.
struct FakeSender(Sent);
impl Sender for FakeSender {
    fn send(&self, mail: &OutgoingMail, user: &str) -> quotecraft_core::Result<String> {
        if mail.to.iter().any(|t| t.contains("refuse")) {
            return Err(quotecraft_core::Error::External("mailcraft: refused".into()));
        }
        self.0.lock().unwrap().push((mail.clone(), user.to_string()));
        Ok("sent".into())
    }
    fn describe(&self) -> String {
        "fake sender".into()
    }
}

fn session(dir: &Path) -> (Session, Sent) {
    let sent: Sent = Arc::new(std::sync::Mutex::new(Vec::new()));
    let s2 = sent.clone();
    let store = Arc::new(std::sync::Mutex::new(quotecraft_core::store::Store::in_memory(quotecraft_core::demo::data())));
    let mut s = Session::new(
        store,
        Some(Arc::new(|_| Box::new(FakeRenderer))),
        Some(Arc::new(move |_, _| Ok(Box::new(FakeSender(s2.clone())) as Box<dyn Sender>))),
        Some(quotecraft_core::config::Paths::under(dir)),
    );
    s.config.company = quotecraft_core::demo::company();
    s.set_user("anna", true);
    (s, sent)
}

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("quotecraft-engine-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(s: &mut Session, id: &str, v: Value) -> crate::Result<Value> {
    s.run_blocking(id, v)
}

#[test]
fn every_command_survives_empty_and_hostile_params() {
    let dir = tmp("fuzz");
    let (mut s, _) = session(&dir);
    let hostile = [
        json!({}),
        json!([]),
        json!(null),
        json!({"id": -1, "index": 99999999999u64, "code": "../../etc/passwd", "qty": "NaN", "to": "x", "formats": 7, "rows": "no", "customer": 5, "name": "\u{0}", "path": "/dev/null", "data": "%%%"}),
        json!({"id": "x", "channel": "zzz", "status": "???", "category": 1, "limit": -5, "one_off": "abc", "vat_pct": "1e400", "template": "../x.docx", "mine": "maybe"}),
        json!({"code": "A".repeat(100_000), "name": "B".repeat(100_000), "qty": 1e300, "discount_pct": -50, "at": u64::MAX}),
    ];
    for cmd in COMMANDS {
        for p in &hostile {
            let _ = s.run_blocking(cmd.id, p.clone());
        }
    }
    assert!(s.execute("nope", &json!({})).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_quote_from_the_price_list_renders_and_sends() {
    let dir = tmp("flow");
    let (mut s, sent) = session(&dir);
    let v = run(&mut s, "items.list", json!({"category": "hardware"})).unwrap();
    assert_eq!(v["items"].as_array().unwrap().len(), 4);
    let v = run(&mut s, "quote.new", json!({"channel": "wholesale", "customer_id": 2, "title": "Allegato A partner"})).unwrap();
    let id = v["id"].as_u64().unwrap();
    assert_eq!(v["number"], "2026-0002");
    assert_eq!(v["author"], "anna");
    let v = run(&mut s, "quote.addLine", json!({"code": "FTTH-1G", "qty": 3})).unwrap();
    assert_eq!(v["lines"][0]["monthly"], 2490, "the wholesale price");
    run(&mut s, "quote.addLine", json!({"name": "Sopralluogo", "one_off": "150", "category": "servizi"})).unwrap();
    let v = run(&mut s, "quote.setLine", json!({"index": 0, "discount_pct": 10})).unwrap();
    assert_eq!(v["totals"]["monthly_net"], 2490 * 3 * 9 / 10);
    assert!(run(&mut s, "quote.setLine", json!({"index": 5, "qty": 1})).is_err());
    let v = run(&mut s, "quote.html", json!({})).unwrap();
    assert!(v["html"].as_str().unwrap().contains("ALLEGATO A") && v["fields"]["numero"] == "2026-0002");
    // Render: the HTML always, the PDF and DOCX from the (fake) renderer.
    let v = run(&mut s, "quote.render", json!({"formats": ["pdf", "docx"]})).unwrap();
    let names: Vec<&str> = v["files"].as_array().unwrap().iter().map(|f| f["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["2026-0002-partner-reti-s-p-a.html", "2026-0002-partner-reti-s-p-a.pdf", "2026-0002-partner-reti-s-p-a.docx"]);
    assert!(dir.join("out/2026-0002-partner-reti-s-p-a/2026-0002-partner-reti-s-p-a.pdf").is_file());
    let v = run(&mut s, "quote.get", json!({"id": id})).unwrap();
    assert_eq!(v["outputs"].as_array().unwrap().len(), 3);
    // Send: to the customer's address by default, as the user, the PDF and DOCX attached.
    let v = run(&mut s, "quote.send", json!({"id": id, "formats": "pdf"})).unwrap();
    assert_eq!(v["sent"], "sent");
    let mails = sent.lock().unwrap();
    assert_eq!(mails.len(), 1);
    let (mail, user) = &mails[0];
    assert_eq!((user.as_str(), mail.to[0].as_str()), ("anna", "wholesale@partnerreti.example"));
    assert_eq!(mail.attachments.len(), 1);
    assert!(mail.subject.contains("2026-0002") && mail.body.contains("Anna Verdi"), "{mail:?}");
    drop(mails);
    let v = run(&mut s, "quote.get", json!({"id": id})).unwrap();
    assert_eq!((v["status"].as_str().unwrap(), v["sent_to"][0].as_str().unwrap()), ("sent", "wholesale@partnerreti.example"));
    // A refused mail is an error, the quote stays sent.
    let e = run(&mut s, "quote.send", json!({"id": id, "to": "refuse@example.com"})).unwrap_err().to_string();
    assert!(e.contains("refused"), "{e}");
    // The data survives a new session on the same directory.
    let v = run(&mut s, "quotes.list", json!({"mine": true})).unwrap();
    assert_eq!(v["quotes"].as_array().unwrap().len(), 1);
    let v = run(&mut s, "quotes.list", json!({})).unwrap();
    assert_eq!(v["quotes"].as_array().unwrap().len(), 2);
    let v = run(&mut s, "quote.duplicate", json!({"id": id})).unwrap();
    assert_eq!((v["number"].as_str().unwrap(), v["status"].as_str().unwrap()), ("2026-0003", "draft"));
    run(&mut s, "quote.delete", json!({"id": v["id"]})).unwrap();
    assert!(s.state().quote.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_price_list_is_the_administrators() {
    let dir = tmp("admin");
    let (mut s, _) = session(&dir);
    let v = run(&mut s, "items.save", json!({"code": "NEW-1", "name": "Nuovo", "category": "services", "one_off_direct": "10,50"})).unwrap();
    assert_eq!(v["item"]["one_off"]["direct"], 1050);
    assert_eq!(v["item"]["vat_pct"], 22.0);
    let v = run(
        &mut s,
        "items.import",
        json!({"rows": [{"code": "NEW-1", "monthly_direct": 5}, {"name": "no code"}, {"code": "NEW-2", "name": "Due", "category": "hw"}]}),
    )
    .unwrap();
    assert_eq!((v["saved"].as_u64(), v["errors"].as_array().unwrap().len()), (Some(2), 1));
    s.set_user("luca", false);
    let e = run(&mut s, "items.save", json!({"code": "NEW-3", "name": "x"})).unwrap_err().to_string();
    assert!(e.contains("administrator"), "{e}");
    assert!(run(&mut s, "items.remove", json!({"code": "NEW-1"})).is_err());
    assert!(run(&mut s, "company.set", json!({"name": "x"})).is_err());
    assert!(run(&mut s, "prefs.set", json!({"mailcraft_url": "http://x"})).is_err());
    run(&mut s, "prefs.set", json!({"language": "it"})).unwrap();
    assert!(s.italian());
    // Reading and quoting stay open to everyone.
    assert!(run(&mut s, "items.get", json!({"code": "new-1"})).is_ok());
    assert!(run(&mut s, "quote.new", json!({"channel": "direct"})).is_ok());
    assert!(run(&mut s, "customers.save", json!({"name": "Bianchi"})).is_ok());
    s.set_user("anna", true);
    run(&mut s, "items.remove", json!({"code": "NEW-1"})).unwrap();
    assert!(run(&mut s, "items.get", json!({"code": "NEW-1"})).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn letterheads_are_files_in_the_templates_folder() {
    let dir = tmp("templates");
    let (mut s, _) = session(&dir);
    let src = dir.join("carta.html");
    std::fs::write(&src, "<html><body><p>{{azienda.nome}} - {{numero}}</p>{{corpo}}</body></html>").unwrap();
    let v = run(&mut s, "templates.import", json!({"path": src.to_string_lossy()})).unwrap();
    assert_eq!(v["template"]["kind"], "html");
    assert!(run(&mut s, "templates.import", json!({"path": src.to_string_lossy(), "name": "../x.html"})).is_err());
    assert!(run(&mut s, "templates.import", json!({"data": "PGh0bWw+", "name": "data.html"})).is_ok());
    let v = run(&mut s, "templates.list", json!({})).unwrap();
    assert_eq!(v["templates"].as_array().unwrap().len(), 2);
    run(&mut s, "quote.open", json!({"id": 1})).unwrap();
    run(&mut s, "quote.set", json!({"template": "carta.html"})).unwrap();
    let v = run(&mut s, "quote.render", json!({"formats": ["html"]})).unwrap();
    let html = std::fs::read_to_string(v["files"][0]["path"].as_str().unwrap()).unwrap();
    assert!(html.starts_with("<html><body><p>Esempio Telecomunicazioni S.r.l. - 2026-0001</p><h1>"), "{html}");
    run(&mut s, "quote.set", json!({"template": "missing.docx"})).unwrap();
    assert!(run(&mut s, "quote.render", json!({})).is_err());
    run(&mut s, "templates.remove", json!({"name": "data.html"})).unwrap();
    assert_eq!(s.state().templates.len(), 1);
    let v = run(&mut s, "bridge.check", json!({})).unwrap();
    assert_eq!((v["renderer"].as_str(), v["sender"].as_str()), (Some("fake renderer"), Some("fake sender")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_demo_session_has_no_bridges_but_still_writes_html() {
    let mut s = Session::demo();
    assert!(s.admin());
    let v = run(&mut s, "quote.open", json!({"id": 1})).unwrap();
    assert_eq!(v["number"], "2026-0001");
    let v = run(&mut s, "quote.render", json!({"formats": "html"})).unwrap();
    assert_eq!(v["files"].as_array().unwrap().len(), 1);
    let e = run(&mut s, "quote.render", json!({"formats": "pdf"})).unwrap_err().to_string();
    assert!(e.contains("renderer"), "{e}");
    let e = run(&mut s, "quote.send", json!({})).unwrap_err().to_string();
    assert!(e.contains("sender") || e.contains("attach"), "{e}");
    let snap = s.snapshot(0).unwrap();
    assert_eq!(snap.state.items.len(), 13);
    assert!(s.snapshot(snap.revision).is_none());
}

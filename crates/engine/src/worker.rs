//! The worker thread: renders documents (WordCraft) and sends mail (MailCraft) off the
//! interface thread. A panicking job is reported as an error, never a crash.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use quotecraft_core::bridge::{OutgoingMail, Produced, RenderJob, RendererFactory, SenderFactory};
use quotecraft_core::config::{Preferences, Secrets};
use quotecraft_core::{Error, Result};

pub enum JobKind {
    Render { job: RenderJob, prefs: Preferences },
    Send { mail: OutgoingMail, user: String, prefs: Preferences, secrets: Secrets },
    Probe { prefs: Preferences, secrets: Secrets },
}

pub struct Job {
    pub id: u64,
    pub kind: JobKind,
}

#[derive(Debug, Clone)]
pub enum Output {
    Rendered(Vec<Produced>),
    Sent(String),
    Probed { renderer: String, sender: String },
}

pub struct Event {
    pub id: u64,
    pub result: Result<Output>,
}

pub struct Worker {
    tx: Sender<Job>,
    rx: Receiver<Event>,
}

struct Inner {
    renderers: Option<RendererFactory>,
    senders: Option<SenderFactory>,
}

impl Inner {
    fn run(&self, kind: JobKind) -> Result<Output> {
        match kind {
            JobKind::Render { job, prefs } => {
                // The HTML always, then what the renderer adds (WordCraft).
                let mut out = vec![quotecraft_core::bridge::write_html(&job)?];
                let wants_more = job.formats.iter().any(|f| f.eq_ignore_ascii_case("pdf") || f.eq_ignore_ascii_case("docx"));
                if wants_more {
                    let renderer = self
                        .renderers
                        .as_ref()
                        .map(|f| f(&prefs))
                        .ok_or_else(|| Error::NotConfigured("no document renderer in this build".into()))?;
                    out.extend(renderer.render(&job)?);
                }
                Ok(Output::Rendered(out))
            }
            JobKind::Send { mail, user, prefs, secrets } => {
                let factory = self.senders.as_ref().ok_or_else(|| Error::NotConfigured("no mail sender in this build".into()))?;
                let sender = factory(&prefs, &secrets)?;
                Ok(Output::Sent(sender.send(&mail, &user)?))
            }
            JobKind::Probe { prefs, secrets } => {
                let renderer = self.renderers.as_ref().map(|f| f(&prefs).describe()).unwrap_or_else(|| "none".into());
                let sender = match self.senders.as_ref() {
                    Some(f) => match f(&prefs, &secrets) {
                        Ok(s) => s.describe(),
                        Err(e) => e.to_string(),
                    },
                    None => "none".into(),
                };
                Ok(Output::Probed { renderer, sender })
            }
        }
    }
}

impl Worker {
    pub fn start(renderers: Option<RendererFactory>, senders: Option<SenderFactory>) -> Self {
        let (tx, jobs) = channel::<Job>();
        let (events, rx) = channel::<Event>();
        let inner = Inner { renderers, senders };
        let spawned = std::thread::Builder::new().name("quotecraft-jobs".into()).spawn(move || {
            while let Ok(job) = jobs.recv() {
                let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| inner.run(job.kind))) {
                    Ok(r) => r,
                    Err(_) => Err(Error::External("the job crashed".into())),
                };
                if events.send(Event { id: job.id, result }).is_err() {
                    break;
                }
            }
        });
        let _ = spawned;
        Self { tx, rx }
    }

    pub fn submit(&self, job: Job) -> bool {
        self.tx.send(job).is_ok()
    }

    pub fn poll(&self) -> Vec<Event> {
        let mut out = Vec::new();
        while let Ok(e) = self.rx.try_recv() {
            out.push(e);
        }
        out
    }

    pub fn wait(&self, timeout: Duration) -> Option<Event> {
        self.rx.recv_timeout(timeout).ok()
    }
}

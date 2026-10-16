//! QuoteCraft's engine (layer L5): the session that holds the configuration, the shared
//! data and the state; the command table every front end calls; the jobs that render and
//! send; the snapshot a browser mirrors.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod cmd;
mod worker;

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub use quotecraft_core as core;
use quotecraft_core::bridge::{Produced, RendererFactory, SenderFactory};
use quotecraft_core::config::{Config, Paths, Secrets};
use quotecraft_core::model::*;
use quotecraft_core::money::Cents;
use quotecraft_core::store::Store;
use serde_json::{Value, json};

pub use cmd::{COMMANDS, Command};
pub use worker::{JobKind, Output};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Core(#[from] quotecraft_core::Error),
    #[error("{0}")]
    Param(String),
    #[error("unknown command `{0}`")]
    UnknownCommand(String),
    #[error("only an administrator can {0}")]
    Forbidden(String),
    #[error("job {0} timed out")]
    Timeout(u64),
    #[error("the worker is gone")]
    WorkerGone,
}

pub type Result<T> = std::result::Result<T, Error>;

/// A quote in a list.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct QuoteSummary {
    pub id: u64,
    pub number: String,
    pub date: String,
    pub channel: Channel,
    pub status: Status,
    pub customer: String,
    pub title: String,
    pub one_off_net: Cents,
    pub monthly_net: Cents,
    pub author: String,
    pub updated: u64,
    pub lines: usize,
}

impl QuoteSummary {
    pub fn of(q: &Quote) -> Self {
        let t = q.totals();
        Self {
            id: q.id,
            number: q.number.clone(),
            date: q.date.clone(),
            channel: q.channel,
            status: q.status,
            customer: q.customer.name.clone(),
            title: q.title.clone(),
            one_off_net: t.one_off_net,
            monthly_net: t.monthly_net,
            author: q.author.clone(),
            updated: q.updated,
            lines: q.lines.len(),
        }
    }
}

/// A letterhead in the templates folder.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct TemplateInfo {
    pub name: String,
    /// `docx` or `html`.
    pub kind: String,
    pub bytes: u64,
}

/// What the interfaces show, mirrored to the browser as JSON.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct State {
    pub items: Vec<Item>,
    pub customers: Vec<Customer>,
    pub quotes: Vec<QuoteSummary>,
    /// The open quote.
    pub quote: Option<Quote>,
    pub templates: Vec<TemplateInfo>,
    /// The files produced for the open quote.
    pub outputs: Vec<Produced>,
    pub last_error: String,
    /// What makes the documents and what sends the mail (from `bridge.check`).
    pub renderer: String,
    pub sender: String,
}

/// A finished job, as the interfaces see it.
#[derive(Debug)]
pub struct Completed {
    pub job: u64,
    pub command: String,
    pub result: Result<Value>,
}

/// The same, in a snapshot.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct CompletedJson {
    pub job: u64,
    pub command: String,
    pub result: Value,
    pub error: String,
}

impl From<&CompletedJson> for Completed {
    fn from(c: &CompletedJson) -> Self {
        let result = if c.error.is_empty() { Ok(c.result.clone()) } else { Err(Error::Param(c.error.clone())) };
        Self { job: c.job, command: c.command.clone(), result }
    }
}

/// The state at a revision, for the web app.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub revision: u64,
    pub config: Config,
    pub has_token: bool,
    pub user: String,
    pub admin: bool,
    pub state: State,
    pub jobs_pending: usize,
    pub completed: Vec<CompletedJson>,
}

/// A session, local or remote: what the interfaces are written against.
pub trait QuoteSession {
    fn config(&self) -> &Config;
    fn state(&self) -> &State;
    fn user(&self) -> String;
    fn admin(&self) -> bool;
    fn has_token(&self) -> bool;
    fn jobs_pending(&self) -> usize;
    fn execute(&mut self, id: &str, params: &Value) -> Result<Value>;
    fn pump(&mut self) -> Vec<Completed>;
    fn run_blocking(&mut self, id: &str, params: Value) -> Result<Value>;
    fn problem(&self) -> Option<String> {
        None
    }
    fn server(&self) -> String {
        String::new()
    }
    fn set_server(&mut self, _url: &str) {}
    fn italian(&self) -> bool {
        self.config().preferences.language.starts_with("it")
    }
}

/// The data every session of a server shares.
pub type SharedStore = Arc<Mutex<Store>>;

/// The local session.
pub struct Session {
    config: Config,
    secrets: Secrets,
    paths: Option<Paths>,
    store: SharedStore,
    worker: worker::Worker,
    state: State,
    user: String,
    admin: bool,
    revision: u64,
    next_job: u64,
    pending: BTreeMap<u64, String>,
    completed: VecDeque<(u64, CompletedJson)>,
}

impl Session {
    /// A session on the shared store, with the configuration under `paths` (or defaults).
    pub fn new(store: SharedStore, renderers: Option<RendererFactory>, senders: Option<SenderFactory>, paths: Option<Paths>) -> Self {
        let (config, secrets) = match &paths {
            Some(p) => (Config::load(&p.config()).unwrap_or_default(), Secrets::load(&p.secrets()).unwrap_or_default()),
            None => (Config::default(), Secrets::default()),
        };
        let worker = worker::Worker::start(renderers, senders);
        let mut s = Self {
            config,
            secrets,
            paths,
            store,
            worker,
            state: State::default(),
            user: String::new(),
            admin: true,
            revision: 1,
            next_job: 1,
            pending: BTreeMap::new(),
            completed: VecDeque::new(),
        };
        s.refresh_lists();
        s
    }

    /// Opens (or starts) the data file under `paths`: the store the apps share.
    pub fn open_store(paths: &Paths) -> quotecraft_core::Result<SharedStore> {
        Ok(Arc::new(Mutex::new(Store::open(&paths.data())?)))
    }

    /// The demo session: the made-up price list and quote, in memory, no WordCraft or
    /// MailCraft unless factories are given.
    pub fn demo() -> Self {
        Self::demo_with(None, None)
    }

    pub fn demo_with(renderers: Option<RendererFactory>, senders: Option<SenderFactory>) -> Self {
        let store = Arc::new(Mutex::new(Store::in_memory(quotecraft_core::demo::data())));
        let mut s = Self::new(store, renderers, senders, None);
        s.config.company = quotecraft_core::demo::company();
        s.refresh_lists();
        s
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn store(&self) -> SharedStore {
        self.store.clone()
    }

    pub fn paths(&self) -> Option<&Paths> {
        self.paths.as_ref()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn jobs_pending(&self) -> usize {
        self.pending.len()
    }

    pub fn italian(&self) -> bool {
        self.config.preferences.language.starts_with("it")
    }

    /// Who is working and whether they may change the price list and the settings. A server
    /// sets both per request from the framework's headers; a local session is its owner's.
    pub fn set_user(&mut self, user: &str, admin: bool) {
        self.user = quotecraft_core::limits::clip_to(user.trim(), 120);
        self.admin = admin;
    }

    pub fn user(&self) -> String {
        self.user.clone()
    }

    pub fn admin(&self) -> bool {
        self.admin
    }

    /// The state since `since`; `None` when nothing changed.
    pub fn snapshot(&self, since: u64) -> Option<Snapshot> {
        if since != 0 && since == self.revision {
            return None;
        }
        Some(Snapshot {
            revision: self.revision,
            config: self.config.clone(),
            has_token: !self.secrets.mailcraft_token.is_empty(),
            user: self.user.clone(),
            admin: self.admin,
            state: self.state.clone(),
            jobs_pending: self.pending.len(),
            completed: self.completed.iter().filter(|(rev, _)| *rev > since).map(|(_, c)| c.clone()).collect(),
        })
    }

    /// Re-reads the lists from the shared store (another user may have changed them).
    pub fn refresh_lists(&mut self) {
        let Ok(store) = self.store.lock() else { return };
        let d = &store.data;
        self.state.items = d.items.clone();
        self.state.customers = d.customers.clone();
        let mut quotes: Vec<QuoteSummary> = d.quotes.iter().map(QuoteSummary::of).collect();
        quotes.sort_by(|a, b| b.updated.cmp(&a.updated).then(b.id.cmp(&a.id)));
        self.state.quotes = quotes;
        if let Some(open) = &self.state.quote {
            self.state.quote = d.quote(open.id).cloned();
        }
        drop(store);
        self.state.templates = self.list_templates();
        self.touch();
    }

    /// The letterheads in the templates folder (`.docx`, `.html`), by name.
    pub fn list_templates(&self) -> Vec<TemplateInfo> {
        let Some(dir) = self.templates_dir() else { return Vec::new() };
        let Ok(rd) = std::fs::read_dir(&dir) else { return Vec::new() };
        let mut out: Vec<TemplateInfo> = rd
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let kind = name.rsplit('.').next().unwrap_or_default().to_ascii_lowercase();
                if !(kind == "docx" || kind == "html" || kind == "htm") || name.starts_with('.') {
                    return None;
                }
                let bytes = e.metadata().ok()?.len();
                Some(TemplateInfo { name, kind: if kind == "htm" { "html".into() } else { kind }, bytes })
            })
            .take(500)
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// The templates folder: the preference, or `templates` in the data directory.
    pub fn templates_dir(&self) -> Option<std::path::PathBuf> {
        let prefs = &self.config.preferences;
        match &self.paths {
            Some(p) => Some(p.templates(prefs)),
            None if !prefs.templates_dir.trim().is_empty() => Some(std::path::PathBuf::from(prefs.templates_dir.trim())),
            None => None,
        }
    }

    /// Where the documents of a quote go.
    pub fn out_dir(&self, q: &Quote) -> std::path::PathBuf {
        let base = self.paths.as_ref().map(|p| p.out()).unwrap_or_else(|| std::env::temp_dir().join("quotecraft-out"));
        base.join(q.file_base())
    }

    fn save_config(&mut self) -> Result<()> {
        if let Some(p) = &self.paths {
            self.config.save(&p.config())?;
            self.secrets.save(&p.secrets())?;
        }
        Ok(())
    }

    fn touch(&mut self) {
        self.revision += 1;
    }

    fn require_admin(&self, what: &str) -> Result<()> {
        if self.admin { Ok(()) } else { Err(Error::Forbidden(what.to_string())) }
    }

    /// Runs a command: local ones answer at once, the others queue a job (`{"job": id}`).
    pub fn execute(&mut self, id: &str, params: &Value) -> Result<Value> {
        let cmd = COMMANDS.iter().find(|c| c.id == id).ok_or_else(|| Error::UnknownCommand(id.to_string()))?;
        let params = if params.is_object() { params.clone() } else { json!({}) };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (cmd.run)(self, &params)));
        match result {
            Ok(r) => r,
            Err(_) => Err(Error::Param(format!("command `{id}` crashed"))),
        }
    }

    fn submit(&mut self, command: &str, kind: JobKind) -> Result<u64> {
        let id = self.next_job;
        self.next_job += 1;
        if !self.worker.submit(worker::Job { id, kind }) {
            return Err(Error::WorkerGone);
        }
        self.pending.insert(id, command.to_string());
        self.touch();
        Ok(id)
    }

    /// Applies finished jobs; the interfaces call it every frame.
    pub fn pump(&mut self) -> Vec<Completed> {
        let events = self.worker.poll();
        events.into_iter().map(|e| self.apply(e)).collect()
    }

    /// Runs a command and waits for its job (CLI, MCP).
    pub fn run_blocking(&mut self, id: &str, params: Value) -> Result<Value> {
        let v = self.execute(id, &params)?;
        match v.get("job").and_then(Value::as_u64) {
            Some(j) => self.wait(j, Duration::from_secs(300)),
            None => Ok(v),
        }
    }

    /// Waits for a job to finish.
    pub fn wait(&mut self, id: u64, timeout: Duration) -> Result<Value> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            for c in self.pump() {
                if c.job == id {
                    return c.result;
                }
            }
            if !self.pending.contains_key(&id) {
                if let Some((_, c)) = self.completed.iter().find(|(_, c)| c.job == id) {
                    return Completed::from(c).result;
                }
                return Err(Error::Param(format!("job {id} unknown")));
            }
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                return Err(Error::Timeout(id));
            }
            if let Some(e) = self.worker.wait(left.min(Duration::from_millis(200))) {
                let c = self.apply(e);
                if c.job == id {
                    return c.result;
                }
            }
        }
    }

    fn apply(&mut self, e: worker::Event) -> Completed {
        let command = self.pending.remove(&e.id).unwrap_or_default();
        let result: Result<Value> = match e.result {
            Ok(out) => self.apply_output(&command, out),
            Err(err) => {
                self.state.last_error = err.to_string();
                Err(Error::Core(err))
            }
        };
        let json = CompletedJson {
            job: e.id,
            command: command.clone(),
            result: result.as_ref().ok().cloned().unwrap_or(Value::Null),
            error: result.as_ref().err().map(|e| e.to_string()).unwrap_or_default(),
        };
        self.touch();
        self.completed.push_back((self.revision, json));
        while self.completed.len() > 50 {
            self.completed.pop_front();
        }
        Completed { job: e.id, command, result }
    }

    fn apply_output(&mut self, command: &str, out: Output) -> Result<Value> {
        self.state.last_error.clear();
        match out {
            Output::Rendered(files) => {
                // Remember the files on the quote named by the command (`quote.render:ID`).
                let id: u64 = command.rsplit(':').next().and_then(|s| s.parse().ok()).unwrap_or(0);
                let names: Vec<String> = files.iter().map(|f| f.name.clone()).collect();
                if id != 0 {
                    self.with_quote(id, |q| {
                        for n in &names {
                            if !q.outputs.contains(n) {
                                q.outputs.push(n.clone());
                            }
                        }
                        q.updated = quotecraft_core::date::now_secs();
                        Ok(())
                    })?;
                }
                self.state.outputs = files.clone();
                Ok(json!({"files": files}))
            }
            Output::Sent(status) => {
                let id: u64 = command.rsplit(':').next().and_then(|s| s.parse().ok()).unwrap_or(0);
                if id != 0 {
                    self.with_quote(id, |q| {
                        if q.status == Status::Draft {
                            q.status = Status::Sent;
                        }
                        q.sent_at = quotecraft_core::date::now_secs();
                        q.updated = q.sent_at;
                        Ok(())
                    })?;
                }
                Ok(json!({"sent": status}))
            }
            Output::Probed { renderer, sender } => {
                self.state.renderer = renderer.clone();
                self.state.sender = sender.clone();
                Ok(json!({"renderer": renderer, "sender": sender}))
            }
        }
    }

    /// Changes a quote in the store, saves, and refreshes the lists.
    fn with_quote(&mut self, id: u64, f: impl FnOnce(&mut Quote) -> Result<()>) -> Result<Quote> {
        let q = {
            let mut store = self.store.lock().map_err(|_| Error::Param("the data is locked".into()))?;
            let q = store.data.quote_mut(id).ok_or_else(|| Error::Core(quotecraft_core::Error::NotFound(format!("quote {id}"))))?;
            f(q)?;
            let q = q.clone();
            store.save()?;
            q
        };
        self.refresh_lists();
        self.state.quote = Some(q.clone());
        Ok(q)
    }
}

impl QuoteSession for Session {
    fn config(&self) -> &Config {
        &self.config
    }
    fn state(&self) -> &State {
        &self.state
    }
    fn user(&self) -> String {
        self.user.clone()
    }
    fn admin(&self) -> bool {
        self.admin
    }
    fn has_token(&self) -> bool {
        !self.secrets.mailcraft_token.is_empty()
    }
    fn jobs_pending(&self) -> usize {
        self.pending.len()
    }
    fn execute(&mut self, id: &str, params: &Value) -> Result<Value> {
        Session::execute(self, id, params)
    }
    fn pump(&mut self) -> Vec<Completed> {
        Session::pump(self)
    }
    fn run_blocking(&mut self, id: &str, params: Value) -> Result<Value> {
        Session::run_blocking(self, id, params)
    }
}

/// The JSON of a quote with its totals and summary.
pub fn quote_json(q: &Quote, italian: bool) -> Value {
    let mut v = serde_json::to_value(q).unwrap_or(Value::Null);
    v["totals"] = serde_json::to_value(q.totals()).unwrap_or(Value::Null);
    v["summary"] = json!(q.summary(italian));
    v["expires"] = json!(q.expires());
    v["file_base"] = json!(q.file_base());
    v
}

#[cfg(test)]
mod tests;

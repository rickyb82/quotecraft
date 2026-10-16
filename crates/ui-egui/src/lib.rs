//! QuoteCraft's egui front end (layer L6): the quotes and their editor, the price list, the
//! customers, the letterheads and the settings, on top of the engine. It is thin: it draws
//! the session's state and acts only through [`QuoteApp::run`], so every action is also a
//! command for the CLI and the MCP server.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod chrome;
mod customers;
pub mod i18n;
pub mod icons;
mod items;
mod nav;
mod quotes;
mod settings;
mod templates;
pub mod theme;
mod widgets;

use std::time::Duration;

pub use i18n::Language;
pub use quotecraft_engine as engine;
pub use quotecraft_engine::core;
use quotecraft_engine::{Completed, QuoteSession, Session};
use serde_json::{Value, json};
use web_time::Instant;

/// What the central panel shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Quotes,
    Editor,
    Items,
    Customers,
    Templates,
    Settings,
}

/// Host-only actions the app asks for (served by apps/*).
#[derive(Clone, Debug)]
pub enum Request {
    OpenUrl(String),
    /// Open or download a document produced for a quote: the desktop opens `path`, the
    /// web app fetches `/api/files/{base}/{name}` from its server.
    OpenOutput {
        base: String,
        name: String,
        path: String,
    },
    /// Let the person pick a letterhead file (`.docx`/`.html`) with the host's file dialog;
    /// the host then calls [`QuoteApp::import_template_bytes`] with it.
    PickTemplate,
    /// A file dropped on the letterheads screen: the host reads it and calls
    /// [`QuoteApp::import_template_bytes`].
    ImportDropped(egui::DroppedFileHandle),
    /// Let the person pick a document to attach to quote `id`; the host then calls
    /// [`QuoteApp::attach_bytes`] with it.
    PickAttachment {
        id: u64,
    },
    /// A file dropped on a quote: the host reads it and calls [`QuoteApp::attach_bytes`].
    AttachDropped {
        id: u64,
        file: egui::DroppedFileHandle,
    },
}

/// Standard base64 (for `templates.import`'s `data`).
pub fn base64_encode(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk.first().copied().unwrap_or(0), chunk.get(1).copied().unwrap_or(0), chunk.get(2).copied().unwrap_or(0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub struct QuoteApp {
    pub session: Box<dyn QuoteSession>,
    pub language: Language,
    pub dark: bool,
    applied_dark: Option<bool>,
    pub screen: Screen,
    pub status: String,
    toast: Option<(String, Instant)>,
    pub requests: Vec<Request>,
    /// The settings screen and its button are shown (off for non-administrators on a server).
    pub allow_settings: bool,
    pub quotes: quotes::QuotesUi,
    pub items: items::ItemsUi,
    pub customers: customers::CustomersUi,
    pub templates: templates::TemplatesUi,
    pub settings: settings::SettingsForm,
    loaded: Loaded,
}

#[derive(Default)]
struct Loaded {
    quotes: bool,
    templates: bool,
    probe: bool,
}

impl QuoteApp {
    pub fn new(session: Box<dyn QuoteSession>) -> Self {
        let language = match session.config().preferences.language.as_str() {
            "it" => Language::It,
            "en" => Language::En,
            _ => Language::system(),
        };
        let dark = session.config().preferences.theme == "dark";
        Self {
            session,
            language,
            dark,
            applied_dark: None,
            screen: Screen::Quotes,
            status: String::new(),
            toast: None,
            requests: Vec::new(),
            allow_settings: true,
            quotes: quotes::QuotesUi::default(),
            items: items::ItemsUi::default(),
            customers: customers::CustomersUi::default(),
            templates: templates::TemplatesUi::default(),
            settings: settings::SettingsForm::default(),
            loaded: Loaded::default(),
        }
    }

    pub fn demo() -> Self {
        Self::new(Box::new(Session::demo()))
    }

    pub fn tr<'a>(&self, text: &'a str) -> &'a str {
        self.language.tr(text)
    }

    pub fn italian(&self) -> bool {
        self.language == Language::It
    }

    pub fn admin(&self) -> bool {
        self.session.admin()
    }

    pub fn window_title(&self) -> String {
        match (&self.session.state().quote, self.screen) {
            (Some(q), Screen::Editor) => format!("{} · QuoteCraft", q.number),
            _ => "QuoteCraft".into(),
        }
    }

    /// Runs a command; failures go to the status bar. A local session answers at once, a
    /// remote one through `pump`; both answers reach [`Self::handle_result`].
    pub fn run(&mut self, id: &str, params: Value) -> Option<Value> {
        match self.session.execute(id, &params) {
            Ok(v) => {
                if v.get("queued").is_none() && v.get("job").is_none() {
                    self.handle_result(id, &v);
                }
                Some(v)
            }
            Err(e) => {
                self.status = self.language.tr_message(&e.to_string());
                None
            }
        }
    }

    fn handle_result(&mut self, command: &str, v: &Value) {
        let command = command.split(':').next().unwrap_or(command);
        match command {
            "quote.new" | "quote.open" | "quote.duplicate" => {
                self.screen = Screen::Editor;
                self.quotes.reload_forms();
                self.status.clear();
            }
            "quote.set" | "quote.addLine" | "quote.setLine" | "quote.removeLine" | "quote.moveLine" => {
                self.quotes.reload_forms();
                self.status.clear();
            }
            "quote.delete" => {
                self.screen = Screen::Quotes;
                self.toast(self.tr("Saved").to_string());
            }
            "quote.render" => {
                let names: Vec<String> = v["files"].as_array().into_iter().flatten().filter_map(|f| f["name"].as_str().map(str::to_string)).collect();
                self.status.clear();
                self.toast(self.language.fill("Documents ready: {names}", &[("names", &names.join(", "))]));
            }
            "quote.send" => {
                self.status.clear();
                self.quotes.show_mail = false;
                let to = self.quotes.mail_to.clone();
                self.toast(self.language.fill("Mail sent to {to}", &[("to", &to)]));
            }
            "items.save" | "items.remove" | "customers.save" | "customers.remove" | "company.set" | "prefs.set" | "templates.import"
            | "templates.remove" => {
                self.toast(self.tr("Saved").to_string());
                self.settings.loaded = false;
            }
            "bridge.check" => {
                self.settings.probe = format!(
                    "{} · {}",
                    self.language.fill("Renderer: {renderer}", &[("renderer", v["renderer"].as_str().unwrap_or_default())]),
                    self.language.fill("Sender: {sender}", &[("sender", v["sender"].as_str().unwrap_or_default())])
                );
            }
            "quotes.list" => self.loaded.quotes = true,
            "templates.list" => self.loaded.templates = true,
            _ => {}
        }
    }

    fn toast(&mut self, text: String) {
        self.toast = Some((text, Instant::now()));
    }

    fn pump(&mut self) {
        let done: Vec<Completed> = self.session.pump();
        for c in done {
            match c.result {
                Ok(v) => self.handle_result(&c.command, &v),
                Err(e) => {
                    let text = self.language.tr_message(&e.to_string());
                    self.status = text;
                }
            }
        }
    }

    /// Opens a quote in the editor.
    pub fn open_quote(&mut self, id: u64) {
        self.run("quote.open", json!({"id": id}));
    }

    /// Starts a quote on the channel.
    pub fn new_quote(&mut self, channel: core::model::Channel) {
        self.run("quote.new", json!({"channel": channel.id()}));
    }

    fn ensure_loaded(&mut self) {
        if !self.loaded.quotes && self.session.jobs_pending() == 0 {
            self.loaded.quotes = true;
            self.run("quotes.list", json!({}));
        }
        if !self.loaded.templates && matches!(self.screen, Screen::Templates | Screen::Editor) {
            self.loaded.templates = true;
            self.run("templates.list", json!({}));
        }
        if !self.loaded.probe && self.screen == Screen::Settings && self.session.server().is_empty() {
            self.loaded.probe = true;
            self.run("bridge.check", json!({}));
        }
    }

    /// Imports a letterhead from its bytes (a picked or dropped file): `templates.import`
    /// with the data in base64, so it works on the desktop and through a server alike.
    pub fn import_template_bytes(&mut self, name: &str, bytes: &[u8]) {
        let name = std::path::Path::new(name.trim()).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if name.is_empty() {
            return;
        }
        if bytes.len() > 50 * 1024 * 1024 {
            self.status = self.language.fill("{name} is larger than 50 MB", &[("name", &name)]);
            return;
        }
        let data = base64_encode(bytes);
        if self.run("templates.import", json!({"name": name, "data": data})).is_some() {
            self.reload();
        }
    }

    /// A file picked or dropped for a quote: `quote.attach` with its bytes (the name alone,
    /// not the path).
    pub fn attach_bytes(&mut self, id: u64, name: &str, bytes: &[u8]) {
        let name = std::path::Path::new(name.trim()).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if name.is_empty() {
            return;
        }
        if bytes.len() > 50 * 1024 * 1024 {
            self.status = self.language.fill("{name} is larger than 50 MB", &[("name", &name)]);
            return;
        }
        let data = base64_encode(bytes);
        if self.run("quote.attach", json!({"id": id, "name": name, "data": data})).is_some() {
            self.status = self.language.fill("{name} attached", &[("name", &name)]);
        }
    }

    /// Reloads the lists on the next frame.
    pub fn reload(&mut self) {
        self.loaded.quotes = false;
        self.loaded.templates = false;
    }

    fn keyboard(&mut self, ui: &egui::Ui) {
        use egui::{Key, Modifiers};
        if ui.ctx().memory(|m| m.focused().is_some()) {
            return;
        }
        let pressed = |m: Modifiers, k: Key| ui.ctx().input_mut(|i| i.consume_key(m, k));
        if pressed(Modifiers::NONE, Key::Escape) {
            self.quotes.confirm_delete = None;
            self.quotes.picking_customer = false;
            self.quotes.show_mail = false;
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let tokens = theme::Tokens::for_dark_mode(self.dark);
        if self.applied_dark != Some(self.dark) {
            theme::apply(ui.ctx(), &tokens);
            self.applied_dark = Some(self.dark);
        }
        self.pump();
        self.ensure_loaded();
        if self.session.jobs_pending() > 0 {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        self.keyboard(ui);
        if !self.allow_settings && self.screen == Screen::Settings {
            self.screen = Screen::Quotes;
        }
        if self.screen == Screen::Editor && self.session.state().quote.is_none() {
            self.screen = Screen::Quotes;
        }
        chrome::top_bar(self, ui, &tokens);
        chrome::status_bar(self, ui, &tokens);
        nav::show(self, ui, &tokens);
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(tokens.panel)).show(ui, |ui| match self.screen {
            Screen::Quotes => quotes::list(self, ui, &tokens),
            Screen::Editor => quotes::editor(self, ui, &tokens),
            Screen::Items => items::show(self, ui, &tokens),
            Screen::Customers => customers::show(self, ui, &tokens),
            Screen::Templates => templates::show(self, ui, &tokens),
            Screen::Settings => settings::show(self, ui, &tokens),
        });
        chrome::toasts(self, ui, &tokens);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(app: &mut QuoteApp, ctx: &egui::Context) {
        for _ in 0..200 {
            let input =
                egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))), ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| app.ui(ui));
            out.textures_delta.clear();
            if app.session.jobs_pending() == 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn walks_the_screens_and_edits_a_quote() {
        let ctx = egui::Context::default();
        let mut app = QuoteApp::demo();
        app.language = Language::It;
        frame(&mut app, &ctx);
        frame(&mut app, &ctx);
        assert_eq!(app.session.state().quotes.len(), 1);
        app.open_quote(1);
        frame(&mut app, &ctx);
        assert_eq!(app.screen, Screen::Editor);
        assert!(app.window_title().starts_with("2026-0001"));
        assert_eq!(app.quotes.header.title, "Connettività e rete per la sede di Via Roma");
        app.run("quote.addLine", json!({"code": "SW-24", "qty": 2}));
        frame(&mut app, &ctx);
        assert_eq!(app.session.state().quote.as_ref().map(|q| q.lines.len()), Some(6));
        app.quotes.header.title = "Nuovo titolo".into();
        app.quotes.save_header_from(&mut app.session);
        frame(&mut app, &ctx);
        assert_eq!(app.session.state().quote.as_ref().map(|q| q.title.as_str()), Some("Nuovo titolo"));
        app.run("quote.render", json!({"formats": ["html"]}));
        frame(&mut app, &ctx);
        assert_eq!(app.session.state().outputs.len(), 1);
        for s in [Screen::Items, Screen::Customers, Screen::Templates, Screen::Settings, Screen::Quotes, Screen::Editor] {
            app.screen = s;
            frame(&mut app, &ctx);
            frame(&mut app, &ctx);
        }
        app.new_quote(core::model::Channel::Wholesale);
        frame(&mut app, &ctx);
        assert_eq!(app.session.state().quote.as_ref().map(|q| q.number.as_str()), Some("2026-0002"));
        app.dark = true;
        frame(&mut app, &ctx);
        app.allow_settings = false;
        app.screen = Screen::Settings;
        frame(&mut app, &ctx);
        assert_eq!(app.screen, Screen::Quotes);
    }

    #[test]
    fn a_picked_or_dropped_file_becomes_a_letterhead() {
        let dir = std::env::temp_dir().join(format!("quotecraft-ui-tpl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ctx = egui::Context::default();
        let mut app = QuoteApp::demo();
        app.run("prefs.set", json!({"templates_dir": dir.to_string_lossy()}));
        app.import_template_bytes("/Users/anna/Documents/carta.html", b"<body>{{numero}} {{corpo}}</body>");
        frame(&mut app, &ctx);
        app.screen = Screen::Templates;
        frame(&mut app, &ctx);
        assert!(app.session.state().templates.iter().any(|t| t.name == "carta.html"), "{:?}", app.session.state().templates);
        assert_eq!(std::fs::read_to_string(dir.join("carta.html")).unwrap(), "<body>{{numero}} {{corpo}}</body>");
        app.run("quote.open", json!({"id": 1}));
        app.attach_bytes(1, "/tmp/offerta-v2.pdf", b"%PDF-1.4");
        assert!(app.session.state().outputs.iter().any(|o| o.name == "offerta-v2.pdf"), "{}", app.status);
        app.attach_bytes(1, "virus.exe", b"x");
        assert!(app.status.contains("virus.exe"), "{}", app.status);
        app.import_template_bytes("nota.txt", b"x");
        assert!(!app.status.is_empty(), "a .txt is refused: {}", app.status);
        app.import_template_bytes("", b"x");
        assert_eq!(base64_encode(b"hi"), "aGk=");
        assert_eq!(base64_encode(b"hello"), "aGVsbG8=");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

//! QuoteCraft desktop app.
//!
//! Usage: `quotecraft [--demo] [--home DIR] [--lang en|it] [--user NAME]`. The data lives in the
//! platform configuration directory (or `QUOTECRAFT_HOME`); `--demo` shows the built-in data.
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use quotecraft_ui_egui::core::config::Paths;
use quotecraft_ui_egui::engine::Session;
use quotecraft_ui_egui::{Language, QuoteApp, Request};

struct App(QuoteApp);

fn open_with_system(target: &str) {
    let _ = std::process::Command::new(if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "cmd"
    } else {
        "xdg-open"
    })
    .args(if cfg!(windows) { vec!["/C", "start", "", target] } else { vec![target] })
    .spawn();
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let title = self.0.window_title();
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(title));
        self.0.ui(ui);
        for request in std::mem::take(&mut self.0.requests) {
            match request {
                Request::OpenUrl(url) => {
                    if url.starts_with("http://") || url.starts_with("https://") {
                        open_with_system(&url);
                    }
                }
                Request::OpenOutput { path, .. } => {
                    if std::path::Path::new(&path).is_file() {
                        open_with_system(&path);
                    }
                }
                Request::ImportDropped(file) => match file.bytes() {
                    Ok(bytes) => self.0.import_template_bytes(&file.path().to_string_lossy(), &bytes),
                    Err(e) => self.0.status = e,
                },
                Request::AttachDropped { id, file } => match file.bytes() {
                    Ok(bytes) => self.0.attach_bytes(id, &file.path().to_string_lossy(), &bytes),
                    Err(e) => self.0.status = e,
                },
                Request::PickAttachment { id } => {
                    let picked = rfd::FileDialog::new().set_title("Attach").pick_file();
                    if let Some(path) = picked {
                        match std::fs::read(&path) {
                            Ok(bytes) => self.0.attach_bytes(id, &path.to_string_lossy(), &bytes),
                            Err(e) => self.0.status = format!("{}: {e}", path.display()),
                        }
                    }
                }
                Request::PickDrawing { id } => {
                    let picked = rfd::FileDialog::new().add_filter("Picture", &["png", "jpg", "jpeg", "gif"]).set_title("Drawing").pick_file();
                    if let Some(path) = picked {
                        match std::fs::read(&path) {
                            Ok(bytes) => self.0.add_drawing_bytes(id, &path.to_string_lossy(), &bytes),
                            Err(e) => self.0.status = format!("{}: {e}", path.display()),
                        }
                    }
                }
                Request::PickTemplate => {
                    let picked = rfd::FileDialog::new().add_filter("Letterhead", &["docx", "html", "htm"]).set_title("Letterhead").pick_file();
                    if let Some(path) = picked {
                        match std::fs::read(&path) {
                            Ok(bytes) => self.0.import_template_bytes(&path.to_string_lossy(), &bytes),
                            Err(e) => self.0.status = format!("{}: {e}", path.display()),
                        }
                    }
                }
            }
        }
    }
}

const USAGE: &str = "Usage: quotecraft [--demo] [--home DIR] [--lang en|it] [--user NAME]";

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut demo = false;
    let mut home: Option<String> = None;
    let mut lang: Option<Language> = None;
    let mut user: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--demo" => demo = true,
            "--home" => home = it.next().cloned(),
            "--lang" => lang = it.next().and_then(|l| Language::parse(l)),
            "--user" => user = it.next().cloned(),
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            _ => return Err(USAGE.to_string()),
        }
    }
    let renderers = Some(quotecraft_bridge::renderer_factory());
    let senders = Some(quotecraft_bridge::sender_factory());
    let mut session = if demo {
        Session::demo_with(renderers, senders)
    } else {
        let paths = match home {
            Some(h) => Paths::under(std::path::Path::new(&h)),
            None => Paths::default_dirs(),
        };
        let store = Session::open_store(&paths).map_err(|e| e.to_string())?;
        Session::new(store, renderers, senders, Some(paths))
    };
    let user = user.or_else(|| std::env::var("USER").ok()).or_else(|| std::env::var("USERNAME").ok()).unwrap_or_default();
    session.set_user(&user, true);
    let mut app = QuoteApp::new(Box::new(session));
    if let Some(l) = lang {
        app.language = l;
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1380.0, 880.0]).with_min_inner_size([900.0, 600.0]).with_title("QuoteCraft"),
        ..Default::default()
    };
    eframe::run_native("QuoteCraft", options, Box::new(move |_cc| Ok(Box::new(App(app))))).map_err(|e| e.to_string())
}

//! QuoteCraft in the browser. Build: `cd apps/quotecraft-web && trunk build --release` (trunk
//! generates the JS loader; no handwritten JS).
//!
//! The page mirrors a `quotecraft-cli serve` server: its own origin when the server serves the
//! page, else `http://127.0.0.1:7878`. URL parameters: `?server=…`, `?token=…`, `?user=…`,
//! `?lang=it`, `?settings=off` (the framework hides the settings from non-administrators).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(target_arch = "wasm32")]
mod web {
    use eframe::wasm_bindgen::JsCast;
    use quotecraft_remote::RemoteSession;
    use quotecraft_ui_egui::{Language, QuoteApp, Request};
    use serde_json::{Value, json};

    const STORAGE_KEY: &str = "quotecraft";
    const DEFAULT_SERVER: &str = "http://127.0.0.1:7878";

    pub struct App {
        app: QuoteApp,
        token: String,
        user: String,
    }

    fn query_param(name: &str) -> Option<String> {
        let search = web_sys::window()?.location().search().ok()?;
        web_sys::UrlSearchParams::new_with_str(&search).ok()?.get(name)
    }

    fn own_server() -> Option<String> {
        let origin = web_sys::window()?.location().origin().ok()?;
        let host = web_sys::window()?.location().hostname().ok()?;
        (!host.ends_with("github.io") && !origin.starts_with("file:")).then_some(origin)
    }

    impl App {
        pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
            let saved: Option<Value> = cc.storage.and_then(|s| s.get_string(STORAGE_KEY)).and_then(|t| serde_json::from_str(&t).ok());
            let get = |k: &str| saved.as_ref().and_then(|v| v.get(k)?.as_str().map(str::to_string));
            let server = query_param("server").or_else(|| get("server")).or_else(own_server).unwrap_or_else(|| DEFAULT_SERVER.to_string());
            let token = query_param("token").or_else(|| get("token")).unwrap_or_default();
            let user = query_param("user").or_else(|| get("user")).unwrap_or_default();
            let mut app = QuoteApp::new(Box::new(RemoteSession::new(&server, &token, &user)));
            let browser_language = web_sys::window().and_then(|w| w.navigator().language()).and_then(|l| Language::parse(&l));
            app.language = get("language").and_then(|l| Language::parse(&l)).or(browser_language).unwrap_or_default();
            if let Some(l) = query_param("lang").and_then(|l| Language::parse(&l)) {
                app.language = l;
            }
            app.dark = saved.as_ref().and_then(|v| v.get("dark")?.as_bool()).unwrap_or_else(|| cc.egui_ctx.system_theme() == Some(egui::Theme::Dark));
            app.allow_settings = query_param("settings").is_none_or(|s| s != "off" && s != "0" && s != "false");
            Self { app, token, user }
        }

        fn file_url(&self, base: &str, name: &str) -> String {
            let server = self.app.session.server();
            let mut url = format!("{server}/api/files/{base}/{name}");
            let mut sep = '?';
            if !self.token.is_empty() {
                url.push_str(&format!("{sep}token={}", self.token));
                sep = '&';
            }
            if !self.user.is_empty() {
                url.push_str(&format!("{sep}user={}", self.user.replace(' ', "%20")));
            }
            url
        }
    }

    impl eframe::App for App {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            self.app.ui(ui);
            for request in std::mem::take(&mut self.app.requests) {
                let url = match request {
                    Request::OpenUrl(url) if url.starts_with("http://") || url.starts_with("https://") => url,
                    Request::OpenOutput { base, name, .. } => self.file_url(&base, &name),
                    _ => continue,
                };
                if let Some(w) = web_sys::window() {
                    let _ = w.open_with_url_and_target(&url, "_blank");
                }
            }
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
        }

        fn save(&mut self, storage: &mut dyn eframe::Storage) {
            let state = json!({"language": self.app.language.code(), "dark": self.app.dark, "server": self.app.session.server(), "token": self.token, "user": self.user});
            storage.set_string(STORAGE_KEY, state.to_string());
        }
    }

    pub fn start() {
        let options = eframe::WebOptions::default();
        wasm_bindgen_futures::spawn_local(async move {
            let Some(canvas) = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id("quotecraft"))
                .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            else {
                return;
            };
            let started = eframe::WebRunner::new().start(canvas, options, Box::new(|cc| Ok(Box::new(App::new(cc))))).await;
            if let Err(e) = started {
                eframe::web_sys::console::error_1(&e);
            }
        });
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("quotecraft-web runs in the browser: cd apps/quotecraft-web && trunk serve --release (and quotecraft-cli serve)");
}

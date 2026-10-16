//! The top bar (logo, new quote, theme, language, user), the status bar and toasts.

use std::time::Duration;

use egui::{Align, FontId, Layout, Rect, RichText, Sense, pos2, vec2};
use serde_json::json;

use crate::core::model::Channel;
use crate::icons::{self, Icon};
use crate::theme::Tokens;
use crate::widgets::{icon_button, primary_button};
use crate::{Language, QuoteApp, Screen};

pub fn top_bar(app: &mut QuoteApp, ui: &mut egui::Ui, t: &Tokens) {
    let frame = egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(14, 8));
    egui::Panel::top("top").frame(frame).show(ui, |ui| {
        ui.horizontal(|ui| {
            let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
            icons::paint(ui.painter(), rect.shrink(2.0), Icon::Logo, t.accent);
            if resp.on_hover_text("QuoteCraft").clicked() {
                app.screen = Screen::Quotes;
            }
            ui.label(RichText::new("QuoteCraft").size(18.0).strong().color(t.ui_text));
            ui.add_space(14.0);
            let new = primary_button(ui, t, Some(Icon::Plus), app.tr("New quote"));
            egui::Popup::menu(&new).show(|ui| {
                for c in Channel::ALL {
                    if ui.button(app.tr(c.label(false))).clicked() {
                        app.new_quote(c);
                        ui.close();
                    }
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if app.allow_settings && icon_button(ui, t, Icon::Settings, app.tr("Settings"), true, app.screen == Screen::Settings).clicked() {
                    app.screen = if app.screen == Screen::Settings { Screen::Quotes } else { Screen::Settings };
                }
                let (icon, tip) = if app.dark { (Icon::Sun, "Light theme") } else { (Icon::Moon, "Dark theme") };
                if icon_button(ui, t, icon, app.tr(tip), true, false).clicked() {
                    app.dark = !app.dark;
                    let theme = if app.dark { "dark" } else { "light" };
                    app.run("prefs.set", json!({"theme": theme}));
                }
                let lang = icon_button(ui, t, Icon::Globe, &format!("{}: {}", app.tr("Language"), app.language.name()), true, false);
                egui::Popup::menu(&lang).show(|ui| {
                    for l in Language::ALL {
                        if ui.selectable_label(app.language == l, l.name()).clicked() {
                            app.language = l;
                            app.run("prefs.set", json!({"language": l.code()}));
                            ui.close();
                        }
                    }
                });
                let user = app.session.user();
                if !user.is_empty() {
                    let label = if app.admin() { format!("{user} · {}", app.tr("Administrator")) } else { user };
                    ui.label(RichText::new(label).color(t.muted));
                }
            });
        });
    });
}

pub fn status_bar(app: &mut QuoteApp, ui: &mut egui::Ui, t: &Tokens) {
    let frame = egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(14, 4));
    egui::Panel::bottom("status").frame(frame).show(ui, |ui| {
        ui.horizontal(|ui| {
            let problem = app.session.problem().filter(|p| p != "connecting").map(|p| {
                app.language.fill(
                    "Server unreachable ({url}): {error}. Start it with: quotecraft-cli serve",
                    &[("url", &app.session.server()), ("error", &p)],
                )
            });
            match problem {
                Some(e) => {
                    let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                    icons::paint(ui.painter(), r, Icon::Warning, t.danger);
                    ui.label(RichText::new(e).small().color(t.danger));
                }
                None if !app.status.is_empty() => {
                    let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                    icons::paint(ui.painter(), r, Icon::Warning, t.warn);
                    ui.label(RichText::new(app.status.clone()).small().color(t.ui_text));
                }
                None => {
                    ui.label(RichText::new(app.tr("Ready")).small().color(t.muted));
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let pending = app.session.jobs_pending();
                if pending > 0 {
                    ui.add(egui::Spinner::new().size(12.0).color(t.accent));
                    ui.label(RichText::new(app.language.fill("Jobs pending: {n}", &[("n", &pending.to_string())])).small().color(t.muted));
                }
                if let Some(q) = &app.session.state().quote {
                    ui.label(RichText::new(format!("{}: {}", app.tr("Open quote"), q.number)).small().color(t.muted));
                }
            });
        });
    });
}

pub fn toasts(app: &mut QuoteApp, ui: &mut egui::Ui, t: &Tokens) {
    let Some((text, since)) = app.toast.clone() else { return };
    if since.elapsed() > Duration::from_secs(4) {
        app.toast = None;
        return;
    }
    ui.ctx().request_repaint_after(Duration::from_millis(250));
    let screen = ui.ctx().content_rect();
    let galley = ui.painter().layout_no_wrap(text, FontId::proportional(13.0), t.on_accent);
    let size = galley.size() + vec2(32.0, 18.0);
    let rect = Rect::from_min_size(pos2(screen.center().x - size.x / 2.0, screen.bottom() - 70.0), size);
    let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("toast")));
    painter.rect_filled(rect, 10.0, egui::Color32::from_rgb(0x32, 0x33, 0x3a));
    painter.galley(rect.min + vec2(16.0, 9.0), galley, t.on_accent);
}

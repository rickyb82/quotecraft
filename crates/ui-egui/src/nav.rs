//! The navigation on the left: quotes, price list, customers, letterheads, settings.

use egui::{Align, FontId, Layout, Rect, RichText, Sense, vec2};

use crate::icons::{self, Icon};
use crate::theme::Tokens;
use crate::{QuoteApp, Screen};

fn item(ui: &mut egui::Ui, t: &Tokens, icon: Icon, label: &str, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 18.0, t.active);
    } else if resp.hovered() {
        p.rect_filled(rect, 18.0, t.hover);
    }
    let color = if selected { t.accent } else { t.ui_text };
    icons::paint(p, Rect::from_center_size(rect.left_center() + vec2(24.0, 0.0), vec2(18.0, 18.0)), icon, color);
    p.text(rect.left_center() + vec2(46.0, 0.0), egui::Align2::LEFT_CENTER, label, FontId::proportional(14.0), color);
    resp
}

pub fn show(app: &mut QuoteApp, ui: &mut egui::Ui, t: &Tokens) {
    let frame = egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(10, 10));
    egui::Panel::left("nav").exact_size(220.0).resizable(false).frame(frame).show(ui, |ui| {
        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            let quotes_selected = matches!(app.screen, Screen::Quotes | Screen::Editor);
            if item(ui, t, Icon::Quotes, app.tr("Quotes"), quotes_selected).clicked() {
                app.screen = Screen::Quotes;
                app.reload();
            }
            if item(ui, t, Icon::Items, app.tr("Price list"), app.screen == Screen::Items).clicked() {
                app.screen = Screen::Items;
            }
            if item(ui, t, Icon::Customers, app.tr("Customers"), app.screen == Screen::Customers).clicked() {
                app.screen = Screen::Customers;
            }
            if item(ui, t, Icon::Templates, app.tr("Letterheads"), app.screen == Screen::Templates).clicked() {
                app.screen = Screen::Templates;
                app.reload();
            }
            if app.allow_settings && item(ui, t, Icon::Settings, app.tr("Settings"), app.screen == Screen::Settings).clicked() {
                app.screen = Screen::Settings;
            }
            ui.add_space(16.0);
            if let Some(q) = &app.session.state().quote {
                let t_ = q.totals();
                ui.label(RichText::new(app.tr("Open quote")).small().color(t.muted));
                ui.label(RichText::new(format!("{}\n{}", q.number, q.customer.name)).color(t.ui_text));
                ui.label(RichText::new(format!("{} + {}/{}", t_.one_off_net.fmt_eur(), t_.monthly_net.fmt_eur(), app.tr("month"))).small().color(t.muted));
                if app.screen != Screen::Editor && ui.small_button(app.tr("Open")).clicked() {
                    app.screen = Screen::Editor;
                }
            } else {
                ui.label(RichText::new(app.tr("QuoteCraft keeps your price lists (connectivity, hardware, services) and writes the quotes: direct ones and the wholesale Allegato A. WordCraft makes the documents, MailCraft sends them.")).small().color(t.muted));
            }
            if !app.admin() {
                ui.add_space(10.0);
                ui.label(RichText::new(app.tr("Only administrators change the price list, the letterheads and the settings.")).small().color(t.muted));
            }
        });
    });
}

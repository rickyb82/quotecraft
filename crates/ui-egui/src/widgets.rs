//! QuoteCraft's buttons and cards.

use egui::{FontId, Rect, Response, RichText, Sense, Stroke, Ui, vec2};

use crate::icons::{self, Icon};
use crate::theme::Tokens;

/// A square icon button with a hover tint; `selected` keeps it tinted.
pub fn icon_button(ui: &mut Ui, t: &Tokens, icon: Icon, tip: &str, enabled: bool, selected: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(32.0, 32.0), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 8.0, t.active);
    } else if enabled && resp.hovered() {
        p.rect_filled(rect, 8.0, t.hover);
    }
    let color = if !enabled {
        t.muted.gamma_multiply(0.55)
    } else if selected {
        t.accent
    } else {
        t.ui_text
    };
    icons::paint(p, Rect::from_center_size(rect.center(), vec2(19.0, 19.0)), icon, color);
    resp.on_hover_text(tip)
}

/// A labelled button with a leading icon, tinted on hover.
pub fn icon_text_button(ui: &mut Ui, t: &Tokens, icon: Icon, text: &str, enabled: bool) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(14.0), t.ui_text);
    let size = vec2(galley.size().x + 44.0, 34.0);
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    if enabled && resp.hovered() {
        p.rect_filled(rect, 8.0, t.hover);
    }
    let color = if enabled { t.ui_text } else { t.muted.gamma_multiply(0.55) };
    icons::paint(p, Rect::from_center_size(rect.left_center() + vec2(20.0, 0.0), vec2(18.0, 18.0)), icon, color);
    p.galley_with_override_text_color(rect.left_center() + vec2(36.0, -galley.size().y / 2.0), galley, color);
    resp
}

/// The filled accent button (the main action of a bar).
pub fn primary_button(ui: &mut Ui, t: &Tokens, icon: Option<Icon>, text: &str) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(14.0), t.on_accent);
    let extra = if icon.is_some() { 26.0 } else { 0.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 28.0 + extra, 34.0), Sense::click());
    let p = ui.painter();
    let fill = if resp.hovered() { t.accent.gamma_multiply(0.9) } else { t.accent };
    p.rect_filled(rect, 8.0, fill);
    let mut x = rect.left() + 14.0;
    if let Some(i) = icon {
        icons::paint(p, Rect::from_center_size(egui::pos2(x + 9.0, rect.center().y), vec2(17.0, 17.0)), i, t.on_accent);
        x += extra;
    }
    p.galley(egui::pos2(x, rect.center().y - galley.size().y / 2.0), galley, t.on_accent);
    resp
}

/// A card with a title.
pub fn card<R>(ui: &mut Ui, t: &Tokens, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::NONE
        .fill(t.card)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(12.0)
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if !title.is_empty() {
                ui.label(RichText::new(title).size(15.0).strong().color(t.ui_text));
                ui.add_space(6.0);
            }
            add(ui)
        })
        .inner
}

/// A labelled text field on one row of a grid (`label | field`).
pub fn field(ui: &mut Ui, t: &Tokens, label: &str, value: &mut String, width: f32) -> Response {
    ui.label(RichText::new(label).color(t.muted));
    let r = ui.add_sized([width, 22.0], egui::TextEdit::singleline(value));
    ui.end_row();
    r
}

/// A small coloured pill.
pub fn pill(ui: &mut Ui, text: &str, color: egui::Color32) {
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(12.0), egui::Color32::WHITE);
    let (rect, _) = ui.allocate_exact_size(galley.size() + vec2(14.0, 6.0), Sense::hover());
    ui.painter().rect_filled(rect, 9.0, color);
    ui.painter().galley(rect.min + vec2(7.0, 3.0), galley, egui::Color32::WHITE);
}

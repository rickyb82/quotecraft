//! Colours and the egui style. Panels read colours from here, never as literals.

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Visuals};

#[derive(Clone, Copy, Debug)]
pub struct Tokens {
    pub dark: bool,
    pub chrome: Color32,
    pub panel: Color32,
    pub border: Color32,
    pub ui_text: Color32,
    pub muted: Color32,
    pub hover: Color32,
    pub active: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub card: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub danger: Color32,
}

impl Tokens {
    pub const LIGHT: Self = Self {
        dark: false,
        chrome: Color32::from_rgb(0xff, 0xff, 0xff),
        panel: Color32::from_rgb(0xf7, 0xf7, 0xfa),
        border: Color32::from_rgb(0xe2, 0xe4, 0xeb),
        ui_text: Color32::from_rgb(0x21, 0x24, 0x2e),
        muted: Color32::from_rgb(0x6b, 0x70, 0x80),
        hover: Color32::from_rgb(0xee, 0xef, 0xf5),
        active: Color32::from_rgb(0xe3, 0xf0, 0xe9),
        accent: Color32::from_rgb(0x1f, 0x8a, 0x5c),
        on_accent: Color32::WHITE,
        card: Color32::WHITE,
        ok: Color32::from_rgb(0x22, 0xa0, 0x6b),
        warn: Color32::from_rgb(0xf0, 0xa2, 0x0c),
        danger: Color32::from_rgb(0xe5, 0x48, 0x4d),
    };

    pub const DARK: Self = Self {
        dark: true,
        chrome: Color32::from_rgb(0x20, 0x22, 0x28),
        panel: Color32::from_rgb(0x25, 0x27, 0x2e),
        border: Color32::from_rgb(0x36, 0x39, 0x44),
        ui_text: Color32::from_rgb(0xe8, 0xe9, 0xee),
        muted: Color32::from_rgb(0xa0, 0xa5, 0xb4),
        hover: Color32::from_rgb(0x31, 0x34, 0x3d),
        active: Color32::from_rgb(0x2b, 0x45, 0x38),
        accent: Color32::from_rgb(0x3f, 0xb3, 0x7f),
        on_accent: Color32::WHITE,
        card: Color32::from_rgb(0x2b, 0x2d, 0x36),
        ok: Color32::from_rgb(0x3f, 0xb3, 0x7f),
        warn: Color32::from_rgb(0xf0, 0xb4, 0x3d),
        danger: Color32::from_rgb(0xf0, 0x6a, 0x6e),
    };

    pub fn for_dark_mode(dark: bool) -> Self {
        if dark { Self::DARK } else { Self::LIGHT }
    }
}

/// Applies QuoteCraft's look to egui: rounder corners, roomier spacing, calmer widgets.
pub fn apply(ctx: &egui::Context, t: &Tokens) {
    let mut visuals = if t.dark { Visuals::dark() } else { Visuals::light() };
    visuals.panel_fill = t.chrome;
    visuals.window_fill = t.card;
    visuals.extreme_bg_color = t.panel;
    visuals.faint_bg_color = t.panel;
    visuals.window_stroke = Stroke::new(1.0, t.border);
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.menu_corner_radius = CornerRadius::same(10);
    visuals.selection.bg_fill = t.active;
    visuals.selection.stroke = Stroke::new(1.0, t.accent);
    visuals.hyperlink_color = t.accent;
    let radius = CornerRadius::same(8);
    for w in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        w.corner_radius = radius;
    }
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, t.border);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, t.ui_text);
    visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    visuals.widgets.inactive.bg_fill = t.panel;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, t.border);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, t.ui_text);
    visuals.widgets.hovered.weak_bg_fill = t.hover;
    visuals.widgets.hovered.bg_fill = t.hover;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, t.border);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, t.ui_text);
    visuals.widgets.active.weak_bg_fill = t.active;
    visuals.widgets.active.bg_fill = t.active;
    visuals.widgets.active.bg_stroke = Stroke::NONE;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, t.ui_text);
    visuals.widgets.open.weak_bg_fill = t.active;
    let theme = if t.dark { egui::Theme::Dark } else { egui::Theme::Light };
    ctx.set_theme(theme);
    ctx.style_mut_of(theme, |s| s.visuals = visuals);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(10.0, 5.0);
        s.spacing.interact_size.y = 30.0;
        s.spacing.menu_margin = egui::Margin::same(8);
        s.text_styles.insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Button, FontId::new(14.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Small, FontId::new(12.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Heading, FontId::new(18.0, FontFamily::Proportional));
    });
}

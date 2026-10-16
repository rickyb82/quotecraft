//! Settings: the company on the documents, the preferences, WordCraft and MailCraft.

use egui::{RichText, vec2};
use serde_json::json;

use crate::QuoteApp;
use crate::icons::Icon;
use crate::theme::Tokens;
use crate::widgets::{card, field, primary_button};

#[derive(Clone, Debug, Default)]
pub struct SettingsForm {
    pub loaded: bool,
    pub company: Vec<(&'static str, &'static str, String)>,
    pub footer: String,
    pub vat: String,
    pub validity: String,
    pub duration: String,
    pub templates_dir: String,
    pub wordcraft_cli: String,
    pub mailcraft_url: String,
    pub mailcraft_token: String,
    pub mail_subject: String,
    pub mail_body: String,
    pub probe: String,
}

impl SettingsForm {
    fn load(&mut self, app: &QuoteApp) {
        let c = &app.session.config().company;
        self.company = vec![
            ("name", "Name", c.name.clone()),
            ("address", "Address", c.address.clone()),
            ("cap", "Postcode", c.cap.clone()),
            ("city", "City", c.city.clone()),
            ("province", "Province", c.province.clone()),
            ("vat", "VAT number", c.vat.clone()),
            ("fiscal_code", "Fiscal code", c.fiscal_code.clone()),
            ("email", "E-mail", c.email.clone()),
            ("pec", "PEC", c.pec.clone()),
            ("phone", "Phone", c.phone.clone()),
            ("web", "Web", c.web.clone()),
            ("iban", "IBAN", c.iban.clone()),
        ];
        self.footer = c.footer.clone();
        let p = &app.session.config().preferences;
        self.vat = crate::core::money::fmt_qty(p.default_vat_pct);
        self.validity = p.default_validity_days.to_string();
        self.duration = p.default_duration_months.to_string();
        self.templates_dir = p.templates_dir.clone();
        self.wordcraft_cli = p.wordcraft_cli.clone();
        self.mailcraft_url = p.mailcraft_url.clone();
        self.mailcraft_token.clear();
        self.mail_subject = p.mail_subject.clone();
        self.mail_body = p.mail_body.clone();
        self.loaded = true;
    }
}

pub fn show(app: &mut QuoteApp, ui: &mut egui::Ui, t: &Tokens) {
    if !app.settings.loaded {
        let mut f = std::mem::take(&mut app.settings);
        f.load(app);
        app.settings = f;
    }
    let admin = app.admin();
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Frame::NONE.inner_margin(egui::Margin::same(18)).show(ui, |ui| {
            ui.label(RichText::new(app.tr("Settings")).size(20.0).strong().color(t.ui_text));
            if !admin {
                ui.label(RichText::new(app.tr("Only an administrator changes the settings.")).small().color(t.muted));
            }
            ui.add_space(10.0);
            let title = app.tr("Company").to_string();
            card(ui, t, &title, |ui| {
                ui.add_enabled_ui(admin, |ui| {
                    egui::Grid::new("company").num_columns(4).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                        let lang = app.language;
                        let rows = app.settings.company.len();
                        for (i, (_, label, value)) in app.settings.company.iter_mut().enumerate() {
                            ui.label(RichText::new(lang.tr(label)).color(t.muted));
                            ui.add_sized([260.0, 22.0], egui::TextEdit::singleline(value));
                            if i % 2 == 1 || i + 1 == rows {
                                ui.end_row();
                            }
                        }
                    });
                    ui.label(RichText::new(app.language.tr("Footer of every quote")).color(t.muted));
                    ui.add(egui::TextEdit::multiline(&mut app.settings.footer).desired_rows(2).desired_width(560.0));
                    if primary_button(ui, t, Some(Icon::Save), app.language.tr("Save company")).clicked() {
                        let mut p = json!({"footer": app.settings.footer.trim()});
                        for (key, _, value) in &app.settings.company {
                            p[*key] = json!(value.trim());
                        }
                        app.run("company.set", p);
                    }
                });
            });
            ui.add_space(10.0);
            let title = app.tr("Preferences").to_string();
            card(ui, t, &title, |ui| {
                ui.add_enabled_ui(admin, |ui| {
                    egui::Grid::new("prefs").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                        field(ui, t, app.language.tr("Default VAT %"), &mut app.settings.vat, 80.0);
                        field(ui, t, app.language.tr("Default validity (days)"), &mut app.settings.validity, 80.0);
                        field(ui, t, app.language.tr("Default duration (months)"), &mut app.settings.duration, 80.0);
                        field(ui, t, app.language.tr("Letterheads"), &mut app.settings.templates_dir, 420.0);
                    });
                    if primary_button(ui, t, Some(Icon::Save), app.language.tr("Save preferences")).clicked() {
                        let s = &app.settings;
                        let p = json!({"default_vat_pct": s.vat.trim(), "default_validity_days": s.validity.trim(), "default_duration_months": s.duration.trim(), "templates_dir": s.templates_dir.trim()});
                        app.run("prefs.set", p);
                    }
                });
            });
            ui.add_space(10.0);
            let title = app.tr("Documents and mail").to_string();
            card(ui, t, &title, |ui| {
                ui.add_enabled_ui(admin, |ui| {
                    egui::Grid::new("bridges").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                        field(ui, t, app.language.tr("WordCraft CLI (makes DOCX and PDF)"), &mut app.settings.wordcraft_cli, 420.0);
                        field(ui, t, app.language.tr("MailCraft server URL"), &mut app.settings.mailcraft_url, 420.0);
                        ui.label(RichText::new(app.language.tr("MailCraft service token")).color(t.muted));
                        ui.add(egui::TextEdit::singleline(&mut app.settings.mailcraft_token).password(true).desired_width(420.0).hint_text(if app.session.has_token() { "••••••••" } else { "" }));
                        ui.end_row();
                        field(ui, t, app.language.tr("Mail subject"), &mut app.settings.mail_subject, 420.0);
                        ui.label(RichText::new(app.language.tr("Mail body")).color(t.muted));
                        ui.add(egui::TextEdit::multiline(&mut app.settings.mail_body).desired_rows(5).desired_width(420.0));
                        ui.end_row();
                    });
                    ui.horizontal(|ui| {
                        if primary_button(ui, t, Some(Icon::Save), app.language.tr("Save preferences")).clicked() {
                            let s = &app.settings;
                            let mut p = json!({"wordcraft_cli": s.wordcraft_cli.trim(), "mailcraft_url": s.mailcraft_url.trim(), "mail_subject": s.mail_subject.trim(), "mail_body": s.mail_body.trim()});
                            if !s.mailcraft_token.trim().is_empty() {
                                p["mailcraft_token"] = json!(s.mailcraft_token.trim());
                            }
                            app.run("prefs.set", p);
                        }
                        if ui.button(app.language.tr("Check WordCraft and MailCraft")).clicked() {
                            app.run("bridge.check", json!({}));
                        }
                    });
                });
                let probe = if app.settings.probe.is_empty() {
                    let st = app.session.state();
                    if st.renderer.is_empty() && st.sender.is_empty() {
                        String::new()
                    } else {
                        format!("{} · {}", app.language.fill("Renderer: {renderer}", &[("renderer", &st.renderer)]), app.language.fill("Sender: {sender}", &[("sender", &st.sender)]))
                    }
                } else {
                    app.settings.probe.clone()
                };
                if !probe.is_empty() {
                    ui.label(RichText::new(probe).small().color(t.muted));
                }
            });
        });
    });
}

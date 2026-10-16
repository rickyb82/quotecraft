//! The letterheads: the files in the templates folder, the placeholders, import and defaults.

use egui::{RichText, vec2};
use serde_json::json;

use crate::QuoteApp;
use crate::icons::Icon;
use crate::theme::Tokens;
use crate::widgets::{card, icon_button, primary_button};

#[derive(Clone, Debug, Default)]
pub struct TemplatesUi {
    pub import_path: String,
    pub import_name: String,
    pub dir: String,
    pub loaded: bool,
}

pub fn show(app: &mut QuoteApp, ui: &mut egui::Ui, t: &Tokens) {
    let admin = app.admin();
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Frame::NONE.inner_margin(egui::Margin::same(18)).show(ui, |ui| {
            ui.label(RichText::new(app.tr("Letterheads")).size(20.0).strong().color(t.ui_text));
            ui.label(
                RichText::new(
                    app.tr("A letterhead is a .docx (WordCraft fills it) or an .html file with placeholders; the quote's body goes at {{corpo}}."),
                )
                .small()
                .color(t.muted),
            );
            if !app.templates.dir.is_empty() {
                ui.label(RichText::new(app.language.fill("Letterheads folder: {dir}", &[("dir", &app.templates.dir)])).small().color(t.muted));
            }
            ui.add_space(10.0);
            let templates = app.session.state().templates.clone();
            let prefs = app.session.config().preferences.clone();
            card(ui, t, "", |ui| {
                if templates.is_empty() {
                    ui.label(RichText::new(app.tr("No letterheads yet: put .docx or .html files in the folder, or import one.")).color(t.muted));
                }
                egui::Grid::new("templates").striped(true).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
                    for tpl in &templates {
                        ui.label(RichText::new(&tpl.name).strong());
                        ui.label(RichText::new(tpl.kind.to_uppercase()).small().color(t.muted));
                        ui.label(RichText::new(format!("{} kB", tpl.bytes / 1024)).small().color(t.muted));
                        ui.horizontal(|ui| {
                            let is_direct = prefs.template_direct == tpl.name;
                            let is_wholesale = prefs.template_wholesale == tpl.name;
                            if admin {
                                if ui.selectable_label(is_direct, app.language.tr("Default for direct quotes")).clicked() {
                                    app.run("prefs.set", json!({"template_direct": if is_direct { "" } else { &tpl.name }}));
                                }
                                if ui.selectable_label(is_wholesale, app.language.tr("Default for wholesale")).clicked() {
                                    app.run("prefs.set", json!({"template_wholesale": if is_wholesale { "" } else { &tpl.name }}));
                                }
                                if icon_button(ui, t, Icon::Trash, app.language.tr("Remove"), true, false).clicked() {
                                    app.run("templates.remove", json!({"name": tpl.name}));
                                    app.reload();
                                }
                            } else {
                                if is_direct {
                                    ui.label(RichText::new(app.language.tr("Default for direct quotes")).small().color(t.accent));
                                }
                                if is_wholesale {
                                    ui.label(RichText::new(app.language.tr("Default for wholesale")).small().color(t.accent));
                                }
                            }
                        });
                        ui.end_row();
                    }
                });
            });
            if admin {
                ui.add_space(10.0);
                let title = app.tr("Import a letterhead (path on the server)").to_string();
                card(ui, t, &title, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            [420.0, 22.0],
                            egui::TextEdit::singleline(&mut app.templates.import_path).hint_text("/data/sirius/commerciale/carta-intestata.docx"),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut app.templates.import_name).hint_text(app.language.tr("File name")).desired_width(200.0),
                        );
                        if primary_button(ui, t, Some(Icon::Plus), app.language.tr("Import")).clicked() {
                            let p = json!({"path": app.templates.import_path.trim(), "name": app.templates.import_name.trim()});
                            if app.run("templates.import", p).is_some() {
                                app.templates.import_path.clear();
                                app.templates.import_name.clear();
                                app.reload();
                            }
                        }
                    });
                });
            }
            ui.add_space(10.0);
            let title = app.tr("Placeholders").to_string();
            card(ui, t, &title, |ui| {
                egui::Grid::new("placeholders").striped(true).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
                    for (p, d) in crate::core::render::PLACEHOLDERS {
                        ui.label(RichText::new(*p).monospace());
                        ui.label(RichText::new(*d).small().color(t.muted));
                        ui.end_row();
                    }
                });
            });
        });
    });
}

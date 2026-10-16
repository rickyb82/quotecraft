//! The customers: search, the table, the form.

use egui::{RichText, vec2};
use serde_json::json;

use crate::QuoteApp;
use crate::core::model::Customer;
use crate::icons::Icon;
use crate::quotes::CustomerForm;
use crate::theme::Tokens;
use crate::widgets::{card, icon_button, icon_text_button, primary_button};

#[derive(Clone, Debug, Default)]
pub struct CustomersUi {
    pub query: String,
    pub editing: Option<CustomerForm>,
    pub confirm_remove: Option<u64>,
}

pub fn show(app: &mut QuoteApp, ui: &mut egui::Ui, t: &Tokens) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Frame::NONE.inner_margin(egui::Margin::same(18)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(app.tr("Customers")).size(20.0).strong().color(t.ui_text));
                ui.add_space(12.0);
                ui.add(egui::TextEdit::singleline(&mut app.customers.query).hint_text(app.language.tr("Search")).desired_width(220.0));
                if icon_text_button(ui, t, Icon::Plus, app.tr("Add customer"), true).clicked() {
                    app.customers.editing = Some(CustomerForm::default());
                }
            });
            ui.add_space(10.0);
            if let Some(mut f) = app.customers.editing.clone() {
                let title = app.tr(if f.id == 0 { "Add customer" } else { "Edit" }).to_string();
                let lang = app.language;
                let mut save = false;
                let mut cancel = false;
                card(ui, t, &title, |ui| {
                    f.rows(ui, t, |l| lang.tr(l).to_string());
                    ui.horizontal(|ui| {
                        if primary_button(ui, t, Some(Icon::Save), lang.tr("Save customer")).clicked() {
                            save = true;
                        }
                        if ui.button(lang.tr("Cancel")).clicked() {
                            cancel = true;
                        }
                    });
                });
                app.customers.editing = Some(f.clone());
                if save {
                    let mut p = f.params();
                    if f.id != 0 {
                        p["id"] = json!(f.id);
                    }
                    if app.run("customers.save", p).is_some() {
                        app.customers.editing = None;
                    }
                }
                if cancel {
                    app.customers.editing = None;
                }
                ui.add_space(10.0);
            }
            let q = app.customers.query.clone();
            let rows: Vec<Customer> = app.session.state().customers.iter().filter(|c| c.matches(&q)).cloned().collect();
            if rows.is_empty() {
                card(ui, t, "", |ui| {
                    ui.label(RichText::new(app.tr("No customers yet.")).color(t.muted));
                });
                return;
            }
            card(ui, t, "", |ui| {
                egui::Grid::new("customers").striped(true).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
                    for h in ["Name", "Address", "VAT number", "E-mail", "Contact", "Phone", ""] {
                        ui.label(RichText::new(app.language.tr(h)).small().color(t.muted));
                    }
                    ui.end_row();
                    for c in &rows {
                        ui.label(RichText::new(&c.name).strong());
                        ui.label(RichText::new(c.address_line()).small());
                        ui.label(&c.vat);
                        ui.label(&c.email);
                        ui.label(&c.contact);
                        ui.label(&c.phone);
                        ui.horizontal(|ui| {
                            if ui.small_button(app.language.tr("Edit")).clicked() {
                                app.customers.editing = Some(CustomerForm::from(c));
                            }
                            if ui.small_button(app.language.tr("New quote")).clicked() {
                                app.run("quote.new", json!({"customer_id": c.id}));
                            }
                            if app.customers.confirm_remove == Some(c.id) {
                                if ui.small_button(app.language.tr("Yes, delete")).clicked() {
                                    app.customers.confirm_remove = None;
                                    app.run("customers.remove", json!({"id": c.id}));
                                }
                                if ui.small_button(app.language.tr("Cancel")).clicked() {
                                    app.customers.confirm_remove = None;
                                }
                            } else if icon_button(ui, t, Icon::Trash, app.language.tr("Remove customer"), true, false).clicked() {
                                app.customers.confirm_remove = Some(c.id);
                            }
                        });
                        ui.end_row();
                    }
                });
            });
        });
    });
}

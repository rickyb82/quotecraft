//! The price list: search, the table, and (for administrators) the item form.

use egui::{RichText, vec2};
use serde_json::json;

use crate::QuoteApp;
use crate::core::model::{Category, Item};
use crate::core::money::fmt_qty;
use crate::icons::Icon;
use crate::theme::Tokens;
use crate::widgets::{card, icon_button, icon_text_button, primary_button};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemForm {
    pub new: bool,
    pub code: String,
    pub name: String,
    pub description: String,
    pub category: Category,
    pub unit: String,
    pub one_off_direct: String,
    pub one_off_wholesale: String,
    pub monthly_direct: String,
    pub monthly_wholesale: String,
    pub vat: String,
    pub active: bool,
    pub tags: String,
}

impl ItemForm {
    fn from(i: &Item) -> Self {
        Self {
            new: false,
            code: i.code.clone(),
            name: i.name.clone(),
            description: i.description.clone(),
            category: i.category,
            unit: i.unit.clone(),
            one_off_direct: i.one_off.direct.fmt(),
            one_off_wholesale: i.one_off.wholesale.fmt(),
            monthly_direct: i.monthly.direct.fmt(),
            monthly_wholesale: i.monthly.wholesale.fmt(),
            vat: fmt_qty(i.vat_pct),
            active: i.active,
            tags: i.tags.join(", "),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ItemsUi {
    pub query: String,
    pub category: Option<Category>,
    pub inactive: bool,
    pub editing: Option<ItemForm>,
    pub confirm_remove: Option<String>,
}

pub fn show(app: &mut QuoteApp, ui: &mut egui::Ui, t: &Tokens) {
    let italian = app.italian();
    let admin = app.admin();
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Frame::NONE.inner_margin(egui::Margin::same(18)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(app.tr("Price list")).size(20.0).strong().color(t.ui_text));
                ui.add_space(12.0);
                ui.add(egui::TextEdit::singleline(&mut app.items.query).hint_text(app.language.tr("Search")).desired_width(220.0));
                if ui.selectable_label(app.items.category.is_none(), app.tr("All")).clicked() {
                    app.items.category = None;
                }
                for c in Category::ALL {
                    if ui.selectable_label(app.items.category == Some(c), c.label(italian)).clicked() {
                        app.items.category = Some(c);
                    }
                }
                ui.checkbox(&mut app.items.inactive, app.language.tr("Inactive too"));
                if admin && icon_text_button(ui, t, Icon::Plus, app.tr("Add item"), true).clicked() {
                    let vat = app.session.config().preferences.default_vat_pct;
                    app.items.editing = Some(ItemForm { new: true, active: true, vat: fmt_qty(vat), unit: "pz".into(), ..Default::default() });
                }
            });
            if !admin {
                ui.label(RichText::new(app.tr("Only an administrator changes the price list.")).small().color(t.muted));
            }
            ui.add_space(10.0);
            if let Some(mut f) = app.items.editing.clone() {
                let (save, cancel) = item_form(app, ui, t, &mut f, italian);
                app.items.editing = Some(f.clone());
                if save {
                    let p = json!({
                        "code": f.code.trim(), "name": f.name.trim(), "description": f.description.trim(), "category": f.category.id(), "unit": f.unit.trim(),
                        "one_off_direct": f.one_off_direct.trim(), "one_off_wholesale": f.one_off_wholesale.trim(), "monthly_direct": f.monthly_direct.trim(),
                        "monthly_wholesale": f.monthly_wholesale.trim(), "vat_pct": f.vat.trim(), "active": f.active, "tags": f.tags.trim(),
                    });
                    if app.run("items.save", p).is_some() {
                        app.items.editing = None;
                    }
                }
                if cancel {
                    app.items.editing = None;
                }
                ui.add_space(10.0);
            }
            let q = app.items.query.clone();
            let rows: Vec<Item> =
                app.session.state().items.iter().filter(|i| (app.items.inactive || i.active) && app.items.category.is_none_or(|c| i.category == c) && i.matches(&q)).cloned().collect();
            ui.label(RichText::new(app.language.fill("Items: {n}", &[("n", &rows.len().to_string())])).small().color(t.muted));
            card(ui, t, "", |ui| {
                egui::Grid::new("items").striped(true).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
                    for h in ["Code", "Item", "Category", "Unit", "One-off", "Monthly", "VAT %", "Active", ""] {
                        ui.label(RichText::new(app.language.tr(h)).small().color(t.muted));
                    }
                    ui.end_row();
                    for i in &rows {
                        ui.label(RichText::new(&i.code).strong());
                        ui.vertical(|ui| {
                            ui.set_min_width(240.0);
                            ui.set_max_width(380.0);
                            ui.label(RichText::new(&i.name).strong());
                            if !i.description.is_empty() {
                                ui.label(RichText::new(&i.description).small().color(t.muted));
                            }
                        });
                        ui.label(i.category.label(italian));
                        ui.label(&i.unit);
                        ui.label(format!("{} / {}", i.one_off.direct.fmt_eur(), i.one_off.wholesale.fmt_eur()));
                        ui.label(format!("{} / {}", i.monthly.direct.fmt_eur(), i.monthly.wholesale.fmt_eur()));
                        ui.label(format!("{}%", fmt_qty(i.vat_pct)));
                        if i.active {
                            let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), egui::Sense::hover());
                            crate::icons::paint(ui.painter(), r, Icon::Check, t.ok);
                        } else {
                            ui.label("");
                        }
                        ui.horizontal(|ui| {
                            if admin && ui.small_button(app.language.tr("Edit")).clicked() {
                                app.items.editing = Some(ItemForm::from(i));
                            }
                            if admin {
                                if app.items.confirm_remove.as_deref() == Some(i.code.as_str()) {
                                    if ui.small_button(app.language.tr("Yes, delete")).clicked() {
                                        app.items.confirm_remove = None;
                                        app.run("items.remove", json!({"code": i.code}));
                                    }
                                    if ui.small_button(app.language.tr("Cancel")).clicked() {
                                        app.items.confirm_remove = None;
                                    }
                                } else if icon_button(ui, t, Icon::Trash, app.language.tr("Remove item"), true, false).clicked() {
                                    app.items.confirm_remove = Some(i.code.clone());
                                }
                            }
                        });
                        ui.end_row();
                    }
                });
                ui.label(RichText::new(format!("{} / {}", app.language.tr("Direct price"), app.language.tr("Wholesale price"))).small().color(t.muted));
            });
        });
    });
}

fn item_form(app: &QuoteApp, ui: &mut egui::Ui, t: &Tokens, f: &mut ItemForm, italian: bool) -> (bool, bool) {
    let mut save = false;
    let mut cancel = false;
    let title = app.tr(if f.new { "Add item" } else { "Edit" }).to_string();
    card(ui, t, &title, |ui| {
        egui::Grid::new("item-form").num_columns(4).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
            let lang = app.language;
            ui.label(RichText::new(lang.tr("Code")).color(t.muted));
            ui.add_enabled(f.new, egui::TextEdit::singleline(&mut f.code).desired_width(140.0));
            ui.label(RichText::new(lang.tr("Category")).color(t.muted));
            egui::ComboBox::from_id_salt("item-cat").selected_text(f.category.label(italian)).show_ui(ui, |ui| {
                for c in Category::ALL {
                    ui.selectable_value(&mut f.category, c, c.label(italian));
                }
            });
            ui.end_row();
            ui.label(RichText::new(lang.tr("Item")).color(t.muted));
            ui.add_sized([300.0, 22.0], egui::TextEdit::singleline(&mut f.name));
            ui.label(RichText::new(lang.tr("Unit")).color(t.muted));
            ui.add_sized([80.0, 22.0], egui::TextEdit::singleline(&mut f.unit));
            ui.end_row();
            ui.label(RichText::new(lang.tr("Description")).color(t.muted));
            ui.add(egui::TextEdit::multiline(&mut f.description).desired_rows(2).desired_width(300.0));
            ui.label(RichText::new(lang.tr("Tags")).color(t.muted));
            ui.add_sized([200.0, 22.0], egui::TextEdit::singleline(&mut f.tags));
            ui.end_row();
            ui.label(RichText::new(format!("{} · {}", lang.tr("One-off"), lang.tr("Direct price"))).color(t.muted));
            ui.add_sized([100.0, 22.0], egui::TextEdit::singleline(&mut f.one_off_direct));
            ui.label(RichText::new(format!("{} · {}", lang.tr("One-off"), lang.tr("Wholesale price"))).color(t.muted));
            ui.add_sized([100.0, 22.0], egui::TextEdit::singleline(&mut f.one_off_wholesale));
            ui.end_row();
            ui.label(RichText::new(format!("{} · {}", lang.tr("Monthly"), lang.tr("Direct price"))).color(t.muted));
            ui.add_sized([100.0, 22.0], egui::TextEdit::singleline(&mut f.monthly_direct));
            ui.label(RichText::new(format!("{} · {}", lang.tr("Monthly"), lang.tr("Wholesale price"))).color(t.muted));
            ui.add_sized([100.0, 22.0], egui::TextEdit::singleline(&mut f.monthly_wholesale));
            ui.end_row();
            ui.label(RichText::new(lang.tr("VAT %")).color(t.muted));
            ui.add_sized([60.0, 22.0], egui::TextEdit::singleline(&mut f.vat));
            ui.label(RichText::new(lang.tr("Active")).color(t.muted));
            ui.checkbox(&mut f.active, "");
            ui.end_row();
        });
        ui.horizontal(|ui| {
            if primary_button(ui, t, Some(Icon::Save), app.language.tr("Save item")).clicked() {
                save = true;
            }
            if ui.button(app.language.tr("Cancel")).clicked() {
                cancel = true;
            }
        });
    });
    (save, cancel)
}

//! The quotes: the list, and the editor (header, customer, lines, totals, documents, mail).

use egui::{Color32, RichText, vec2};
use quotecraft_engine::QuoteSession;
use serde_json::{Value, json};

use crate::core::model::{Category, Channel, Customer, Line, Quote, Status};
use crate::core::money::{Cents, fmt_qty};
use crate::icons::Icon;
use crate::theme::Tokens;
use crate::widgets::{card, field, icon_button, icon_text_button, pill, primary_button};
use crate::{QuoteApp, Request, Screen};

/// The header fields as typed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeaderForm {
    pub title: String,
    pub date: String,
    pub validity: String,
    pub duration: String,
    pub template: String,
    pub notes: String,
    pub channel: Channel,
    pub status: Status,
}

impl HeaderForm {
    fn from(q: &Quote) -> Self {
        Self {
            title: q.title.clone(),
            date: q.date.clone(),
            validity: q.validity_days.to_string(),
            duration: q.duration_months.to_string(),
            template: q.template.clone(),
            notes: q.notes.clone(),
            channel: q.channel,
            status: q.status,
        }
    }
}

/// The customer fields as typed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CustomerForm {
    pub id: u64,
    pub name: String,
    pub address: String,
    pub cap: String,
    pub city: String,
    pub province: String,
    pub vat: String,
    pub fiscal_code: String,
    pub email: String,
    pub pec: String,
    pub sdi: String,
    pub contact: String,
    pub phone: String,
}

impl CustomerForm {
    pub fn from(c: &Customer) -> Self {
        Self {
            id: c.id,
            name: c.name.clone(),
            address: c.address.clone(),
            cap: c.cap.clone(),
            city: c.city.clone(),
            province: c.province.clone(),
            vat: c.vat.clone(),
            fiscal_code: c.fiscal_code.clone(),
            email: c.email.clone(),
            pec: c.pec.clone(),
            sdi: c.sdi.clone(),
            contact: c.contact.clone(),
            phone: c.phone.clone(),
        }
    }

    pub fn params(&self) -> Value {
        json!({
            "name": self.name.trim(), "address": self.address.trim(), "cap": self.cap.trim(), "city": self.city.trim(), "province": self.province.trim(),
            "vat": self.vat.trim(), "fiscal_code": self.fiscal_code.trim(), "email": self.email.trim(), "pec": self.pec.trim(), "sdi": self.sdi.trim(),
            "contact": self.contact.trim(), "phone": self.phone.trim(),
        })
    }

    /// The form rows (label, field).
    pub fn rows(&mut self, ui: &mut egui::Ui, t: &Tokens, tr: impl Fn(&str) -> String) {
        egui::Grid::new(("customer-form", self.id)).num_columns(4).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
            let f = |ui: &mut egui::Ui, label: &str, value: &mut String, w: f32| {
                ui.label(RichText::new(tr(label)).color(t.muted));
                ui.add_sized([w, 22.0], egui::TextEdit::singleline(value));
            };
            f(ui, "Name", &mut self.name, 260.0);
            f(ui, "VAT number", &mut self.vat, 160.0);
            ui.end_row();
            f(ui, "Address", &mut self.address, 260.0);
            f(ui, "Fiscal code", &mut self.fiscal_code, 160.0);
            ui.end_row();
            f(ui, "Postcode", &mut self.cap, 80.0);
            f(ui, "E-mail", &mut self.email, 160.0);
            ui.end_row();
            f(ui, "City", &mut self.city, 260.0);
            f(ui, "PEC", &mut self.pec, 160.0);
            ui.end_row();
            f(ui, "Province", &mut self.province, 60.0);
            f(ui, "SDI code", &mut self.sdi, 160.0);
            ui.end_row();
            f(ui, "Contact", &mut self.contact, 260.0);
            f(ui, "Phone", &mut self.phone, 160.0);
            ui.end_row();
        });
    }
}

/// A line being edited.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LineForm {
    pub index: usize,
    pub name: String,
    pub description: String,
    pub qty: String,
    pub one_off: String,
    pub monthly: String,
    pub discount: String,
    pub vat: String,
    pub category: Category,
    pub unit: String,
}

impl LineForm {
    fn from(index: usize, l: &Line) -> Self {
        Self {
            index,
            name: l.name.clone(),
            description: l.description.clone(),
            qty: fmt_qty(l.qty),
            one_off: l.one_off.fmt(),
            monthly: l.monthly.fmt(),
            discount: fmt_qty(l.discount_pct),
            vat: fmt_qty(l.vat_pct),
            category: l.category,
            unit: l.unit.clone(),
        }
    }

    fn params(&self) -> Value {
        json!({
            "index": self.index, "name": self.name.trim(), "description": self.description.trim(), "qty": self.qty.trim(),
            "one_off": self.one_off.trim(), "monthly": self.monthly.trim(), "discount_pct": self.discount.trim(), "vat_pct": self.vat.trim(),
            "category": self.category.id(), "unit": self.unit.trim(),
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct QuotesUi {
    // The list.
    pub query: String,
    pub status: Option<Status>,
    pub mine: bool,
    pub confirm_delete: Option<u64>,
    // The editor.
    pub loaded_for: Option<u64>,
    pub header: HeaderForm,
    pub customer: CustomerForm,
    pub picking_customer: bool,
    pub customer_query: String,
    pub add_query: String,
    pub adding: bool,
    pub custom: Option<LineForm>,
    pub line_edit: Option<LineForm>,
    pub want_pdf: bool,
    pub want_docx: bool,
    pub show_mail: bool,
    pub mail_to: String,
    pub mail_cc: String,
    pub mail_subject: String,
    pub mail_body: String,
}

impl QuotesUi {
    /// Fills the forms from the open quote again on the next frame.
    pub fn reload_forms(&mut self) {
        self.loaded_for = None;
    }

    fn load(&mut self, q: &Quote, prefs: &crate::core::config::Preferences, company: &crate::core::config::Company) {
        self.header = HeaderForm::from(q);
        self.customer = CustomerForm::from(&q.customer);
        self.loaded_for = Some(q.id);
        self.line_edit = None;
        self.custom = None;
        self.want_pdf = true;
        self.want_docx = true;
        self.mail_to = q.customer.email.clone();
        let (s, b) = crate::core::render::mail_text(q, company, &prefs.mail_subject, &prefs.mail_body);
        self.mail_subject = s;
        self.mail_body = b;
    }

    /// Saves the header form (the test and the Save button).
    pub fn save_header_from(&mut self, session: &mut Box<dyn QuoteSession>) {
        let h = &self.header;
        let _ = session.execute(
            "quote.set",
            &json!({
                "title": h.title.trim(), "date": h.date.trim(), "validity_days": h.validity.trim(), "duration_months": h.duration.trim(),
                "template": h.template.trim(), "notes": h.notes.trim(), "channel": h.channel.id(), "status": h.status.id(),
            }),
        );
    }
}

fn status_color(s: Status, t: &Tokens) -> Color32 {
    match s {
        Status::Draft => t.muted,
        Status::Sent => Color32::from_rgb(0x3b, 0x7d, 0xd8),
        Status::Accepted => t.ok,
        Status::Rejected => t.danger,
        Status::Expired => t.warn,
    }
}

pub fn list(app: &mut QuoteApp, ui: &mut egui::Ui, t: &Tokens) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Frame::NONE.inner_margin(egui::Margin::same(18)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(app.tr("Quotes")).size(20.0).strong().color(t.ui_text));
                ui.add_space(12.0);
                ui.add(egui::TextEdit::singleline(&mut app.quotes.query).hint_text(app.language.tr("Search")).desired_width(220.0));
                if ui.selectable_label(app.quotes.status.is_none(), app.tr("All")).clicked() {
                    app.quotes.status = None;
                }
                for s in Status::ALL {
                    if ui.selectable_label(app.quotes.status == Some(s), app.tr(s.label(false))).clicked() {
                        app.quotes.status = Some(s);
                    }
                }
                ui.add_space(8.0);
                if ui.selectable_label(!app.quotes.mine, app.tr("Everyone's")).clicked() {
                    app.quotes.mine = false;
                }
                if ui.selectable_label(app.quotes.mine, app.tr("Mine")).clicked() {
                    app.quotes.mine = true;
                }
            });
            ui.add_space(10.0);
            let user = app.session.user();
            let q = app.quotes.query.trim().to_lowercase();
            let rows: Vec<quotecraft_engine::QuoteSummary> = app
                .session
                .state()
                .quotes
                .iter()
                .filter(|s| app.quotes.status.is_none_or(|st| s.status == st))
                .filter(|s| !app.quotes.mine || s.author == user)
                .filter(|s| {
                    q.is_empty()
                        || s.number.to_lowercase().contains(&q)
                        || s.customer.to_lowercase().contains(&q)
                        || s.title.to_lowercase().contains(&q)
                })
                .cloned()
                .collect();
            if rows.is_empty() {
                card(ui, t, "", |ui| {
                    ui.label(RichText::new(app.tr("No quotes yet: make one with New quote.")).color(t.muted));
                });
                return;
            }
            card(ui, t, "", |ui| {
                egui::Grid::new("quotes").striped(true).spacing(vec2(14.0, 6.0)).show(ui, |ui| {
                    for h in ["Number", "Date", "Customer", "Title", "Channel", "One-off", "Monthly", "Status", "Author", ""] {
                        ui.label(RichText::new(app.tr(h)).small().color(t.muted));
                    }
                    ui.end_row();
                    for s in &rows {
                        if ui.link(RichText::new(&s.number).strong()).clicked() {
                            app.open_quote(s.id);
                        }
                        ui.label(RichText::new(&s.date).small());
                        ui.label(&s.customer);
                        ui.label(RichText::new(&s.title).small());
                        ui.label(RichText::new(app.tr(s.channel.label(false))).small());
                        ui.label(s.one_off_net.fmt_eur());
                        ui.label(format!("{}/{}", s.monthly_net.fmt_eur(), app.tr("month")));
                        pill(ui, app.tr(s.status.label(false)), status_color(s.status, t));
                        ui.label(RichText::new(&s.author).small().color(t.muted));
                        ui.horizontal(|ui| {
                            if icon_button(ui, t, Icon::Copy, app.tr("Duplicate"), true, false).clicked() {
                                app.run("quote.duplicate", json!({"id": s.id}));
                            }
                            if app.quotes.confirm_delete == Some(s.id) {
                                ui.label(RichText::new(app.tr("Delete this quote?")).small().color(t.danger));
                                if ui.small_button(app.tr("Yes, delete")).clicked() {
                                    app.quotes.confirm_delete = None;
                                    app.run("quote.delete", json!({"id": s.id}));
                                }
                                if ui.small_button(app.tr("Cancel")).clicked() {
                                    app.quotes.confirm_delete = None;
                                }
                            } else if icon_button(ui, t, Icon::Trash, app.tr("Delete"), true, false).clicked() {
                                app.quotes.confirm_delete = Some(s.id);
                            }
                        });
                        ui.end_row();
                    }
                });
            });
        });
    });
}

fn money_cell(ui: &mut egui::Ui, c: Cents) {
    ui.label(if c.is_zero() { "-".to_string() } else { c.fmt_eur() });
}

pub fn editor(app: &mut QuoteApp, ui: &mut egui::Ui, t: &Tokens) {
    let Some(q) = app.session.state().quote.clone() else { return };
    if app.quotes.loaded_for != Some(q.id) {
        let prefs = app.session.config().preferences.clone();
        let company = app.session.config().company.clone();
        app.quotes.load(&q, &prefs, &company);
    }
    let italian = app.italian();
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Frame::NONE.inner_margin(egui::Margin::same(18)).show(ui, |ui| {
            ui.horizontal(|ui| {
                if icon_button(ui, t, Icon::Back, app.tr("Back to the quotes"), true, false).clicked() {
                    app.screen = Screen::Quotes;
                    app.reload();
                }
                ui.label(RichText::new(app.language.fill("Quote {number}", &[("number", &q.number)])).size(20.0).strong().color(t.ui_text));
                pill(ui, app.tr(q.status.label(false)), status_color(q.status, t));
                ui.label(RichText::new(app.tr(q.channel.label(false))).color(t.muted));
                if let Some(e) = q.expires() {
                    let d = crate::core::date::Date::parse(&e).map(|d| d.italian()).unwrap_or(e);
                    ui.label(RichText::new(app.language.fill("expires {date}", &[("date", &d)])).small().color(t.muted));
                }
            });
            ui.add_space(10.0);
            header_card(app, ui, t, &q);
            ui.add_space(10.0);
            customer_card(app, ui, t, &q);
            ui.add_space(10.0);
            lines_card(app, ui, t, &q, italian);
            ui.add_space(10.0);
            totals_card(app, ui, t, &q);
            ui.add_space(10.0);
            documents_card(app, ui, t, &q);
        });
    });
}

fn header_card(app: &mut QuoteApp, ui: &mut egui::Ui, t: &Tokens, q: &Quote) {
    let title = app.tr("Header").to_string();
    card(ui, t, &title, |ui| {
        let h = &mut app.quotes.header;
        egui::Grid::new("header").num_columns(4).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
            ui.label(RichText::new(app.language.tr("Title")).color(t.muted));
            ui.add_sized([360.0, 22.0], egui::TextEdit::singleline(&mut h.title));
            ui.label(RichText::new(app.language.tr("Date")).color(t.muted));
            ui.add_sized([110.0, 22.0], egui::TextEdit::singleline(&mut h.date));
            ui.end_row();
            ui.label(RichText::new(app.language.tr("Channel")).color(t.muted));
            ui.horizontal(|ui| {
                for c in Channel::ALL {
                    ui.selectable_value(&mut h.channel, c, app.language.tr(c.label(false)));
                }
            });
            ui.label(RichText::new(app.language.tr("Status")).color(t.muted));
            egui::ComboBox::from_id_salt("status").selected_text(app.language.tr(h.status.label(false))).show_ui(ui, |ui| {
                for s in Status::ALL {
                    ui.selectable_value(&mut h.status, s, app.language.tr(s.label(false)));
                }
            });
            ui.end_row();
            ui.label(RichText::new(app.language.tr("Validity (days)")).color(t.muted));
            ui.add_sized([60.0, 22.0], egui::TextEdit::singleline(&mut h.validity));
            ui.label(RichText::new(app.language.tr("Duration (months)")).color(t.muted));
            ui.add_sized([60.0, 22.0], egui::TextEdit::singleline(&mut h.duration));
            ui.end_row();
            ui.label(RichText::new(app.language.tr("Letterhead")).color(t.muted));
            let shown = if h.template.is_empty() { app.language.tr("Built-in layout").to_string() } else { h.template.clone() };
            egui::ComboBox::from_id_salt("template").selected_text(shown).width(260.0).show_ui(ui, |ui| {
                if ui.selectable_label(h.template.is_empty(), app.language.tr("Built-in layout")).clicked() {
                    h.template.clear();
                }
                for tpl in &app.session.state().templates {
                    if ui.selectable_label(h.template == tpl.name, &tpl.name).clicked() {
                        h.template = tpl.name.clone();
                    }
                }
            });
            ui.end_row();
            ui.label(RichText::new(app.language.tr("Notes")).color(t.muted));
            ui.add(egui::TextEdit::multiline(&mut h.notes).desired_rows(2).desired_width(560.0));
            ui.end_row();
        });
        let dirty = *h != HeaderForm::from(q);
        ui.horizontal(|ui| {
            if ui.add_enabled(dirty, egui::Button::new(app.language.tr("Save header"))).clicked() {
                app.quotes.save_header_from(&mut app.session);
                app.quotes.reload_forms();
            }
            if dirty {
                ui.label(RichText::new(app.language.tr("Unsaved changes")).small().color(t.warn));
            }
        });
    });
}

fn customer_card(app: &mut QuoteApp, ui: &mut egui::Ui, t: &Tokens, q: &Quote) {
    let title = app.tr("Customer").to_string();
    card(ui, t, &title, |ui| {
        ui.horizontal(|ui| {
            let pick = ui.button(app.language.tr("Pick a customer"));
            if pick.clicked() {
                app.quotes.picking_customer = !app.quotes.picking_customer;
            }
            if app.quotes.picking_customer {
                egui::Popup::from_response(&pick).open(true).align(egui::RectAlign::BOTTOM_START).show(|ui| {
                    ui.set_min_width(360.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut app.quotes.customer_query)
                            .hint_text(app.language.tr("Type to search the customers"))
                            .desired_width(340.0),
                    );
                    let query = app.quotes.customer_query.clone();
                    let hits: Vec<Customer> = app.session.state().customers.iter().filter(|c| c.matches(&query)).take(50).cloned().collect();
                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        for c in &hits {
                            if ui.selectable_label(false, format!("{} · {}", c.name, c.city)).clicked() {
                                app.quotes.picking_customer = false;
                                app.run("quote.set", json!({"customer_id": c.id}));
                            }
                        }
                    });
                });
            }
            if ui.button(app.language.tr("Save to the customers list")).clicked() {
                let mut p = app.quotes.customer.params();
                if app.quotes.customer.id != 0 {
                    p["id"] = json!(app.quotes.customer.id);
                }
                app.run("customers.save", p);
            }
        });
        let tr = app.language;
        app.quotes.customer.rows(ui, t, |l| tr.tr(l).to_string());
        let dirty = app.quotes.customer != CustomerForm::from(&q.customer);
        ui.horizontal(|ui| {
            if ui.add_enabled(dirty, egui::Button::new(app.language.tr("Save customer"))).clicked() {
                let p = app.quotes.customer.params();
                app.run("quote.set", json!({"customer": p}));
            }
            if dirty {
                ui.label(RichText::new(app.language.tr("Unsaved changes")).small().color(t.warn));
            }
        });
    });
}

fn lines_card(app: &mut QuoteApp, ui: &mut egui::Ui, t: &Tokens, q: &Quote, italian: bool) {
    let title = app.tr("Lines").to_string();
    card(ui, t, &title, |ui| {
        ui.horizontal(|ui| {
            let add = icon_text_button(ui, t, Icon::Plus, app.language.tr("Add from the price list"), true);
            if add.clicked() {
                app.quotes.adding = !app.quotes.adding;
            }
            if app.quotes.adding {
                egui::Popup::from_response(&add).open(true).align(egui::RectAlign::BOTTOM_START).show(|ui| {
                    ui.set_min_width(520.0);
                    ui.add(egui::TextEdit::singleline(&mut app.quotes.add_query).hint_text(app.language.tr("Search")).desired_width(500.0));
                    let query = app.quotes.add_query.clone();
                    let channel = q.channel;
                    let hits: Vec<crate::core::model::Item> =
                        app.session.state().items.iter().filter(|i| i.active && i.matches(&query)).take(100).cloned().collect();
                    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                        for cat in Category::ALL {
                            let in_cat: Vec<&crate::core::model::Item> = hits.iter().filter(|i| i.category == cat).collect();
                            if in_cat.is_empty() {
                                continue;
                            }
                            ui.label(RichText::new(cat.label(italian)).small().color(t.muted));
                            for i in in_cat {
                                let price = format!(
                                    "{} · {}/{}",
                                    i.one_off.for_channel(channel).fmt_eur(),
                                    i.monthly.for_channel(channel).fmt_eur(),
                                    app.language.tr("month")
                                );
                                if ui.selectable_label(false, format!("{}  {}   {}", i.code, i.name, price)).clicked() {
                                    app.quotes.adding = false;
                                    app.run("quote.addLine", json!({"code": i.code, "qty": 1}));
                                }
                            }
                        }
                    });
                });
            }
            if icon_text_button(ui, t, Icon::Plus, app.language.tr("Add a custom line"), true).clicked() {
                let vat = app.session.config().preferences.default_vat_pct;
                app.quotes.custom = Some(LineForm {
                    qty: "1".into(),
                    one_off: "0,00".into(),
                    monthly: "0,00".into(),
                    discount: "0".into(),
                    vat: fmt_qty(vat),
                    category: Category::Services,
                    ..Default::default()
                });
            }
        });
        if let Some(mut f) = app.quotes.custom.clone() {
            ui.add_space(6.0);
            let (save, cancel) = line_form(ui, t, &app.language, &mut f, italian);
            app.quotes.custom = Some(f.clone());
            if save {
                let mut p = f.params();
                if let Some(o) = p.as_object_mut() {
                    o.remove("index");
                }
                app.quotes.custom = None;
                app.run("quote.addLine", p);
            }
            if cancel {
                app.quotes.custom = None;
            }
        }
        ui.add_space(6.0);
        if q.lines.is_empty() {
            ui.label(RichText::new(app.language.tr("No lines yet: add from the price list.")).color(t.muted));
            return;
        }
        egui::Grid::new("lines").striped(true).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
            for h in ["Code", "Description", "Qty", "One-off", "Monthly", "Discount %", "VAT %", "Total", ""] {
                ui.label(RichText::new(app.language.tr(h)).small().color(t.muted));
            }
            ui.end_row();
            let n = q.lines.len();
            for (i, l) in q.lines.iter().enumerate() {
                ui.label(RichText::new(&l.code).small());
                ui.vertical(|ui| {
                    ui.set_min_width(240.0);
                    ui.set_max_width(380.0);
                    ui.label(RichText::new(&l.name).strong());
                    if !l.description.is_empty() {
                        ui.label(RichText::new(&l.description).small().color(t.muted));
                    }
                });
                ui.label(if l.unit.is_empty() { fmt_qty(l.qty) } else { format!("{} {}", fmt_qty(l.qty), l.unit) });
                money_cell(ui, l.one_off);
                money_cell(ui, l.monthly);
                ui.label(if l.discount_pct > 0.0 { format!("{}%", fmt_qty(l.discount_pct)) } else { "-".into() });
                ui.label(format!("{}%", fmt_qty(l.vat_pct)));
                ui.label(format!("{} + {}/{}", l.one_off_net().fmt_eur(), l.monthly_net().fmt_eur(), app.language.tr("month")));
                ui.horizontal(|ui| {
                    if icon_button(ui, t, Icon::Up, "", i > 0, false).clicked() {
                        app.run("quote.moveLine", json!({"index": i, "to": i - 1}));
                    }
                    if icon_button(ui, t, Icon::Down, "", i + 1 < n, false).clicked() {
                        app.run("quote.moveLine", json!({"index": i, "to": i + 1}));
                    }
                    if ui.small_button(app.language.tr("Edit")).clicked() {
                        app.quotes.line_edit = Some(LineForm::from(i, l));
                    }
                    if icon_button(ui, t, Icon::Trash, app.language.tr("Remove"), true, false).clicked() {
                        app.run("quote.removeLine", json!({"index": i}));
                    }
                });
                ui.end_row();
            }
        });
        if let Some(mut f) = app.quotes.line_edit.clone() {
            ui.add_space(6.0);
            let (save, cancel) = line_form(ui, t, &app.language, &mut f, italian);
            app.quotes.line_edit = Some(f.clone());
            if save {
                app.quotes.line_edit = None;
                app.run("quote.setLine", f.params());
            }
            if cancel {
                app.quotes.line_edit = None;
            }
        }
    });
}

/// The fields of a line; returns (save, cancel).
fn line_form(ui: &mut egui::Ui, t: &Tokens, lang: &crate::Language, f: &mut LineForm, italian: bool) -> (bool, bool) {
    let mut save = false;
    let mut cancel = false;
    egui::Frame::NONE.fill(t.panel).corner_radius(8.0).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
        egui::Grid::new(("line-form", f.index)).num_columns(6).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
            ui.label(RichText::new(lang.tr("Name")).color(t.muted));
            ui.add_sized([260.0, 22.0], egui::TextEdit::singleline(&mut f.name));
            ui.label(RichText::new(lang.tr("Category")).color(t.muted));
            egui::ComboBox::from_id_salt(("cat", f.index)).selected_text(f.category.label(italian)).show_ui(ui, |ui| {
                for c in Category::ALL {
                    ui.selectable_value(&mut f.category, c, c.label(italian));
                }
            });
            ui.label(RichText::new(lang.tr("Unit")).color(t.muted));
            ui.add_sized([60.0, 22.0], egui::TextEdit::singleline(&mut f.unit));
            ui.end_row();
            ui.label(RichText::new(lang.tr("Description")).color(t.muted));
            ui.add_sized([260.0, 22.0], egui::TextEdit::singleline(&mut f.description));
            ui.label(RichText::new(lang.tr("Qty")).color(t.muted));
            ui.add_sized([60.0, 22.0], egui::TextEdit::singleline(&mut f.qty));
            ui.label(RichText::new(lang.tr("Discount %")).color(t.muted));
            ui.add_sized([60.0, 22.0], egui::TextEdit::singleline(&mut f.discount));
            ui.end_row();
            ui.label(RichText::new(lang.tr("One-off")).color(t.muted));
            ui.add_sized([100.0, 22.0], egui::TextEdit::singleline(&mut f.one_off));
            ui.label(RichText::new(lang.tr("Monthly")).color(t.muted));
            ui.add_sized([100.0, 22.0], egui::TextEdit::singleline(&mut f.monthly));
            ui.label(RichText::new(lang.tr("VAT %")).color(t.muted));
            ui.add_sized([60.0, 22.0], egui::TextEdit::singleline(&mut f.vat));
            ui.end_row();
        });
        ui.horizontal(|ui| {
            if primary_button(ui, t, Some(Icon::Check), lang.tr("Save")).clicked() {
                save = true;
            }
            if ui.button(lang.tr("Cancel")).clicked() {
                cancel = true;
            }
        });
    });
    (save, cancel)
}

fn totals_card(app: &mut QuoteApp, ui: &mut egui::Ui, t: &Tokens, q: &Quote) {
    let title = app.tr("Totals").to_string();
    let tot = q.totals();
    card(ui, t, &title, |ui| {
        egui::Grid::new("totals").spacing(vec2(18.0, 4.0)).show(ui, |ui| {
            ui.label("");
            for h in ["Net", "VAT", "Gross"] {
                ui.label(RichText::new(app.language.tr(h)).small().color(t.muted));
            }
            ui.end_row();
            ui.label(app.language.tr("One-off"));
            ui.label(tot.one_off_net.fmt_eur());
            ui.label(tot.one_off_vat.fmt_eur());
            ui.label(RichText::new(tot.one_off_gross.fmt_eur()).strong());
            ui.end_row();
            ui.label(app.language.tr("Monthly"));
            ui.label(tot.monthly_net.fmt_eur());
            ui.label(tot.monthly_vat.fmt_eur());
            ui.label(RichText::new(tot.monthly_gross.fmt_eur()).strong());
            ui.end_row();
            if q.duration_months > 0 && !tot.monthly_net.is_zero() {
                ui.label(app.language.fill("Contract value ({months} months)", &[("months", &q.duration_months.to_string())]));
                ui.label(tot.contract_net.fmt_eur());
                ui.label(tot.contract_gross.checked_sub(tot.contract_net).fmt_eur());
                ui.label(RichText::new(tot.contract_gross.fmt_eur()).strong());
                ui.end_row();
            }
        });
    });
}

fn documents_card(app: &mut QuoteApp, ui: &mut egui::Ui, t: &Tokens, q: &Quote) {
    let title = app.tr("Documents").to_string();
    card(ui, t, &title, |ui| {
        ui.label(
            RichText::new(app.language.tr("Makes the HTML, and the PDF and DOCX through WordCraft (letterhead included).")).small().color(t.muted),
        );
        ui.horizontal(|ui| {
            ui.checkbox(&mut app.quotes.want_pdf, "PDF");
            ui.checkbox(&mut app.quotes.want_docx, "DOCX");
            if primary_button(ui, t, Some(Icon::Document), app.language.tr("Make the documents")).clicked() {
                let mut formats = vec!["html"];
                if app.quotes.want_pdf {
                    formats.push("pdf");
                }
                if app.quotes.want_docx {
                    formats.push("docx");
                }
                app.status = app.language.tr("Making the documents…").to_string();
                app.run("quote.render", json!({"id": q.id, "formats": formats}));
            }
            if icon_text_button(ui, t, Icon::Send, app.language.tr("Send by mail"), !q.outputs.is_empty()).clicked() {
                app.quotes.show_mail = !app.quotes.show_mail;
            }
        });
        let outputs = app.session.state().outputs.clone();
        if outputs.is_empty() {
            ui.label(RichText::new(app.language.tr("No document yet.")).small().color(t.muted));
        } else {
            ui.label(RichText::new(app.language.tr("Files")).small().color(t.muted));
            for f in &outputs {
                ui.horizontal(|ui| {
                    ui.label(&f.name);
                    ui.label(RichText::new(format!("{} kB", f.bytes / 1024)).small().color(t.muted));
                    if icon_button(ui, t, Icon::Download, app.language.tr("Download"), true, false).clicked() {
                        app.requests.push(Request::OpenOutput {
                            base: q.file_base(),
                            name: f.name.clone(),
                            path: f.path.to_string_lossy().into_owned(),
                        });
                    }
                });
            }
        }
        if app.quotes.show_mail {
            ui.add_space(8.0);
            egui::Frame::NONE.fill(t.panel).corner_radius(8.0).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                egui::Grid::new("mail").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                    field(ui, t, app.language.tr("To"), &mut app.quotes.mail_to, 420.0);
                    field(ui, t, app.language.tr("Cc"), &mut app.quotes.mail_cc, 420.0);
                    field(ui, t, app.language.tr("Subject"), &mut app.quotes.mail_subject, 420.0);
                    ui.label(RichText::new(app.language.tr("Body")).color(t.muted));
                    ui.add(egui::TextEdit::multiline(&mut app.quotes.mail_body).desired_rows(6).desired_width(420.0));
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    if primary_button(ui, t, Some(Icon::Send), app.language.tr("Send")).clicked() {
                        let mut formats = Vec::new();
                        if app.quotes.want_pdf {
                            formats.push("pdf");
                        }
                        if app.quotes.want_docx {
                            formats.push("docx");
                        }
                        app.status = app.language.tr("Sending…").to_string();
                        let m = &app.quotes;
                        let p = json!({"id": q.id, "to": m.mail_to.trim(), "cc": m.mail_cc.trim(), "subject": m.mail_subject.trim(), "body": m.mail_body.trim(), "formats": formats});
                        app.run("quote.send", p);
                    }
                    if ui.button(app.language.tr("Cancel")).clicked() {
                        app.quotes.show_mail = false;
                    }
                });
            });
        }
        if !q.sent_to.is_empty() {
            ui.label(RichText::new(app.language.fill("Mail sent to {to}", &[("to", &q.sent_to.join(", "))])).small().color(t.muted));
        }
    });
}

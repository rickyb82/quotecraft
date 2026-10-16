//! A quote as a document: the HTML body (the direct layout, or the wholesale "Allegato A"),
//! the placeholder fields a letterhead can carry (`{{numero}}`, `{{cliente.nome}}`…), and
//! the text of the mail that carries it. WordCraft turns the HTML (inserted into the
//! letterhead) into DOCX and PDF.

use std::collections::BTreeMap;

use crate::config::Company;
use crate::date::Date;
use crate::model::{Category, Channel, Line, Quote, Totals};
use crate::money::{Cents, fmt_qty};

/// Escapes text for HTML.
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

fn para(s: &str) -> String {
    esc(s).replace('\n', "<br>")
}

fn date_it(iso: &str) -> String {
    Date::parse(iso).map(|d| d.italian()).unwrap_or_else(|| iso.to_string())
}

/// The placeholder fields of a quote, as a letterhead can name them (`{{numero}}`).
pub fn fields(q: &Quote, company: &Company) -> BTreeMap<String, String> {
    let t = q.totals();
    let mut f = BTreeMap::new();
    let mut put = |k: &str, v: String| {
        f.insert(k.to_string(), v);
    };
    put("numero", q.number.clone());
    put("data", date_it(&q.date));
    put("data_iso", q.date.clone());
    put("scadenza", q.expires().map(|d| date_it(&d)).unwrap_or_default());
    put("validita_giorni", q.validity_days.to_string());
    put("durata_mesi", q.duration_months.to_string());
    put("titolo", q.title.clone());
    put("canale", q.channel.label(true).to_string());
    put("note", q.notes.clone());
    put("autore", q.author.clone());
    put("cliente.nome", q.customer.name.clone());
    put("cliente.indirizzo", q.customer.address.clone());
    put("cliente.cap", q.customer.cap.clone());
    put("cliente.citta", q.customer.city.clone());
    put("cliente.provincia", q.customer.province.clone());
    put("cliente.indirizzo_completo", q.customer.address_line());
    put("cliente.piva", q.customer.vat.clone());
    put("cliente.cf", q.customer.fiscal_code.clone());
    put("cliente.email", q.customer.email.clone());
    put("cliente.pec", q.customer.pec.clone());
    put("cliente.sdi", q.customer.sdi.clone());
    put("cliente.referente", q.customer.contact.clone());
    put("cliente.telefono", q.customer.phone.clone());
    put("azienda.nome", company.name.clone());
    put("azienda.indirizzo", company.address_line());
    put("azienda.piva", company.vat.clone());
    put("azienda.email", company.email.clone());
    put("azienda.pec", company.pec.clone());
    put("azienda.telefono", company.phone.clone());
    put("azienda.web", company.web.clone());
    put("azienda.iban", company.iban.clone());
    put("totale.una_tantum", t.one_off_net.fmt_eur());
    put("totale.una_tantum_iva", t.one_off_vat.fmt_eur());
    put("totale.una_tantum_lordo", t.one_off_gross.fmt_eur());
    put("totale.canone", t.monthly_net.fmt_eur());
    put("totale.canone_iva", t.monthly_vat.fmt_eur());
    put("totale.canone_lordo", t.monthly_gross.fmt_eur());
    put("totale.contratto", t.contract_net.fmt_eur());
    put("totale.contratto_lordo", t.contract_gross.fmt_eur());
    put("righe", q.lines.len().to_string());
    f
}

/// The placeholders a letterhead may use, for the documentation and the interface.
pub const PLACEHOLDERS: &[(&str, &str)] = &[
    ("{{corpo}}", "where the quote's body (lines and totals) goes; without it the body goes at the end"),
    ("{{numero}}", "quote number"),
    ("{{data}}", "date (GG/MM/AAAA)"),
    ("{{scadenza}}", "expiry date"),
    ("{{validita_giorni}}", "validity in days"),
    ("{{durata_mesi}}", "contract duration in months"),
    ("{{titolo}}", "title"),
    ("{{canale}}", "channel"),
    ("{{note}}", "notes"),
    ("{{autore}}", "who wrote it"),
    ("{{cliente.nome}}", "customer name"),
    ("{{cliente.indirizzo_completo}}", "customer address, one line"),
    ("{{cliente.piva}}", "customer VAT number"),
    ("{{cliente.email}}", "customer e-mail"),
    ("{{cliente.referente}}", "customer contact"),
    ("{{azienda.nome}}", "your company"),
    ("{{azienda.indirizzo}}", "your address"),
    ("{{totale.una_tantum}}", "one-off total, net"),
    ("{{totale.canone}}", "monthly total, net"),
    ("{{totale.contratto}}", "contract total, net"),
    ("{{totale.una_tantum_lordo}}", "one-off total with VAT"),
    ("{{totale.canone_lordo}}", "monthly total with VAT"),
];

/// The tables a letterhead may place on its own (`{{tabella.canone}}`): HTML fragments
/// (WordCraft turns them into Word tables in a `.docx`).
pub const TABLES: &[(&str, &str)] = &[
    ("{{tabella.righe}}", "all the lines, one table (code, description, quantity, prices, discount, totals)"),
    ("{{tabella.una_tantum}}", "the lines with a one-off price: quantity, price, discount, total, with a total row"),
    ("{{tabella.canone}}", "the lines with a monthly fee: quantity, fee, discount, total, with a total row"),
    ("{{tabella.connettivita}}", "the connectivity lines"),
    ("{{tabella.hardware}}", "the hardware lines"),
    ("{{tabella.servizi}}", "the service lines"),
    ("{{tabella.riepilogo}}", "the totals (net, VAT, gross; one-off, monthly, contract value)"),
];

/// The repeating sections an HTML letterhead may carry: `{{#righe}}…{{/righe}}` is repeated
/// for every line, with the line's fields (`{{riga.nome}}`) inside.
pub const SECTIONS: &[(&str, &str)] = &[
    ("{{#righe}}…{{/righe}}", "every line"),
    ("{{#righe.una_tantum}}…{{/righe.una_tantum}}", "the lines with a one-off price"),
    ("{{#righe.canone}}…{{/righe.canone}}", "the lines with a monthly fee"),
    ("{{#righe.connettivita}}…{{/righe.connettivita}}", "the connectivity lines"),
    ("{{#righe.hardware}}…{{/righe.hardware}}", "the hardware lines"),
    ("{{#righe.servizi}}…{{/righe.servizi}}", "the service lines"),
];

/// The fields of a line, inside a section.
pub const LINE_FIELDS: &[(&str, &str)] = &[
    ("{{riga.n}}", "position (1, 2…)"),
    ("{{riga.codice}}", "code"),
    ("{{riga.nome}}", "name"),
    ("{{riga.descrizione}}", "description"),
    ("{{riga.categoria}}", "category"),
    ("{{riga.qta}}", "quantity"),
    ("{{riga.unita}}", "unit"),
    ("{{riga.una_tantum}}", "unit one-off price"),
    ("{{riga.canone}}", "unit monthly fee"),
    ("{{riga.sconto}}", "discount (`10%`, or empty)"),
    ("{{riga.iva}}", "VAT rate"),
    ("{{riga.totale_una_tantum}}", "one-off total of the line (quantity, discount)"),
    ("{{riga.totale_canone}}", "monthly total of the line"),
];

/// The fields of one line, as a section names them (`{{riga.nome}}`).
pub fn line_fields(l: &Line, n: usize) -> BTreeMap<String, String> {
    let mut f = BTreeMap::new();
    let mut put = |k: &str, v: String| {
        f.insert(format!("riga.{k}"), v);
    };
    put("n", n.to_string());
    put("codice", l.code.clone());
    put("nome", l.name.clone());
    put("descrizione", l.description.clone());
    put("categoria", l.category.label(true).to_string());
    put("qta", fmt_qty(l.qty));
    put("unita", l.unit.clone());
    put("una_tantum", money(l.one_off));
    put("canone", money(l.monthly));
    put("sconto", discount(l));
    put("iva", format!("{}%", fmt_qty(l.vat_pct)));
    put("totale_una_tantum", money(l.one_off_net()));
    put("totale_canone", money(l.monthly_net()));
    f
}

/// The groups of lines a letterhead can list: `righe` (all), `righe.una_tantum`,
/// `righe.canone`, and one per category (`righe.connettivita`…), each with its line fields.
pub fn sections(q: &Quote) -> BTreeMap<String, Vec<BTreeMap<String, String>>> {
    let mut out = BTreeMap::new();
    for (key, lines) in line_groups(q) {
        out.insert(key, lines.iter().enumerate().map(|(i, l)| line_fields(l, i.saturating_add(1))).collect());
    }
    out
}

/// The tables a letterhead can place (`tabella.canone` → HTML).
pub fn tables(q: &Quote) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (key, lines) in line_groups(q) {
        let cols = match key.as_str() {
            "righe.una_tantum" => Cols::OneOff,
            "righe.canone" => Cols::Monthly,
            _ => Cols::Full,
        };
        let name = if key == "righe" { "tabella.righe".to_string() } else { key.replacen("righe.", "tabella.", 1) };
        out.insert(name, lines_table(&lines, cols));
    }
    out.insert("tabella.riepilogo".into(), totals_table(q, &q.totals()));
    out
}

fn category_key(cat: Category) -> &'static str {
    match cat {
        Category::Connectivity => "connettivita",
        Category::Hardware => "hardware",
        Category::Services => "servizi",
    }
}

fn line_groups(q: &Quote) -> Vec<(String, Vec<&Line>)> {
    let mut groups: Vec<(String, Vec<&Line>)> = vec![
        ("righe".into(), q.lines.iter().collect()),
        ("righe.una_tantum".into(), q.lines.iter().filter(|l| !l.one_off.is_zero()).collect()),
        ("righe.canone".into(), q.lines.iter().filter(|l| !l.monthly.is_zero()).collect()),
    ];
    for cat in Category::ALL {
        groups.push((format!("righe.{}", category_key(cat)), q.lines.iter().filter(|l| l.category == cat).collect()));
    }
    groups
}

fn money(c: Cents) -> String {
    if c.is_zero() { String::new() } else { c.fmt_eur() }
}

fn discount(l: &Line) -> String {
    if l.discount_pct > 0.0 { format!("{}%", fmt_qty(l.discount_pct)) } else { String::new() }
}

/// Which columns a lines table shows.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cols {
    /// Code, description, quantity, both prices, discount, both totals.
    Full,
    /// Only the one-off side, with a total row.
    OneOff,
    /// Only the monthly side, with a total row.
    Monthly,
}

const TABLE: &str = "<table border=\"1\" cellpadding=\"4\">";

fn lines_table(lines: &[&Line], cols: Cols) -> String {
    let mut h = String::from(TABLE);
    h.push_str("<tr><th>Codice</th><th>Descrizione</th><th class=\"n\">Q.tà</th>");
    match cols {
        Cols::Full => h.push_str("<th class=\"n\">Una tantum</th><th class=\"n\">Canone mensile</th><th class=\"n\">Sconto</th><th class=\"n\">Totale una tantum</th><th class=\"n\">Totale canone</th></tr>"),
        Cols::OneOff => h.push_str("<th class=\"n\">Prezzo</th><th class=\"n\">Sconto</th><th class=\"n\">Totale</th></tr>"),
        Cols::Monthly => h.push_str("<th class=\"n\">Canone mensile</th><th class=\"n\">Sconto</th><th class=\"n\">Totale canone</th></tr>"),
    }
    let mut one_off = Cents::default();
    let mut monthly = Cents::default();
    for l in lines {
        let desc = if l.description.is_empty() {
            esc(&l.name)
        } else {
            format!("<b>{}</b><br><span class=\"muted\">{}</span>", esc(&l.name), para(&l.description))
        };
        let qty = if l.unit.is_empty() { fmt_qty(l.qty) } else { format!("{} {}", fmt_qty(l.qty), esc(&l.unit)) };
        let disc = if l.discount_pct > 0.0 { format!("{}%", fmt_qty(l.discount_pct)) } else { "-".into() };
        one_off = one_off.checked_add(l.one_off_net());
        monthly = monthly.checked_add(l.monthly_net());
        h.push_str(&format!("<tr><td>{}</td><td>{}</td><td class=\"n\">{}</td>", esc(&l.code), desc, qty));
        match cols {
            Cols::Full => h.push_str(&format!(
                "<td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td></tr>",
                cell(l.one_off),
                cell(l.monthly),
                disc,
                cell(l.one_off_net()),
                cell(l.monthly_net()),
            )),
            Cols::OneOff => h.push_str(&format!(
                "<td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td></tr>",
                cell(l.one_off),
                disc,
                cell(l.one_off_net())
            )),
            Cols::Monthly => h.push_str(&format!(
                "<td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td></tr>",
                cell(l.monthly),
                disc,
                cell(l.monthly_net())
            )),
        }
    }
    match cols {
        Cols::Full => {}
        Cols::OneOff => h.push_str(&format!(
            "<tr class=\"tot\"><td colspan=\"5\">Totale una tantum (imponibile)</td><td class=\"n\">{}</td></tr>",
            one_off.fmt_eur()
        )),
        Cols::Monthly => h.push_str(&format!(
            "<tr class=\"tot\"><td colspan=\"5\">Totale canone mensile (imponibile)</td><td class=\"n\">{}</td></tr>",
            monthly.fmt_eur()
        )),
    }
    h.push_str("</table>");
    h
}

fn totals_table(q: &Quote, t: &Totals) -> String {
    let mut h = String::from(TABLE);
    h.push_str("<tr><th></th><th class=\"n\">Imponibile</th><th class=\"n\">IVA</th><th class=\"n\">Totale</th></tr>");
    h.push_str(&format!(
        "<tr><td>Una tantum</td><td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td></tr>",
        t.one_off_net.fmt_eur(),
        t.one_off_vat.fmt_eur(),
        t.one_off_gross.fmt_eur()
    ));
    h.push_str(&format!(
        "<tr><td>Canone mensile</td><td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td></tr>",
        t.monthly_net.fmt_eur(),
        t.monthly_vat.fmt_eur(),
        t.monthly_gross.fmt_eur()
    ));
    if q.duration_months > 0 && !t.monthly_net.is_zero() {
        h.push_str(&format!(
            "<tr class=\"tot\"><td>Valore del contratto ({} mesi)</td><td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td></tr>",
            q.duration_months,
            t.contract_net.fmt_eur(),
            t.contract_gross.checked_sub(t.contract_net).fmt_eur(),
            t.contract_gross.fmt_eur()
        ));
    }
    h.push_str("</table>");
    h
}

/// Expands the repeating sections of a text: `{{#righe}}…{{/righe}}` becomes the inner text
/// once per row, with the row's fields filled. A section without its closing tag stays as it is.
pub fn fill_sections(text: &str, sections: &BTreeMap<String, Vec<BTreeMap<String, String>>>) -> String {
    let mut out = text.to_string();
    for (key, rows) in sections {
        let open = format!("{{{{#{key}}}}}");
        let close = format!("{{{{/{key}}}}}");
        // Bounded: a letterhead names a section a few times, not thousands.
        for _ in 0..64 {
            let Some(a) = out.find(&open) else { break };
            let Some(rel) = out.get(a + open.len()..).and_then(|rest| rest.find(&close)) else { break };
            let inner_start = a + open.len();
            let inner_end = inner_start + rel;
            let Some(inner) = out.get(inner_start..inner_end) else { break };
            let expanded: String = rows.iter().map(|r| fill(inner, r)).collect();
            let Some(after) = out.get(inner_end + close.len()..) else { break };
            let Some(before) = out.get(..a) else { break };
            out = format!("{before}{expanded}{after}");
        }
    }
    out
}

/// Replaces `{{field}}` placeholders in a text (an HTML letterhead, a mail subject).
pub fn fill(text: &str, fields: &BTreeMap<String, String>) -> String {
    let mut out = text.to_string();
    for (k, v) in fields {
        out = out.replace(&format!("{{{{{k}}}}}"), v);
    }
    out
}

const STYLE: &str = r#"body{font-family:Helvetica,Arial,sans-serif;font-size:10.5pt;color:#222}
h1{font-size:16pt;margin:0 0 4pt 0}h2{font-size:12pt;margin:14pt 0 4pt 0}
table{border-collapse:collapse;width:100%}th,td{border:1px solid #999;padding:4pt 6pt;vertical-align:top}
th{background:#eceef3;text-align:left}td.n,th.n{text-align:right;white-space:nowrap}
.head td{border:none;padding:2pt 4pt}.tot td{font-weight:bold}.muted{color:#666;font-size:9pt}"#;

/// The HTML body of the quote (what goes into the letterhead at `{{corpo}}`). With
/// `standalone`, a whole page with the company header (for the built-in layout and previews).
pub fn html(q: &Quote, company: &Company, standalone: bool) -> String {
    let italian = true;
    let t = q.totals();
    let mut h = String::new();
    if standalone {
        h.push_str("<!doctype html><html lang=\"it\"><head><meta charset=\"utf-8\"><title>");
        h.push_str(&esc(&format!("{} {}", if q.channel == Channel::Wholesale { "Allegato A" } else { "Preventivo" }, q.number)));
        h.push_str("</title><style>");
        h.push_str(STYLE);
        h.push_str("</style></head><body>");
        h.push_str("<table class=\"head\"><tr><td><b>");
        h.push_str(&esc(&company.name));
        h.push_str("</b><br>");
        h.push_str(&esc(&company.address_line()));
        for (label, v) in
            [("P. IVA", &company.vat), ("Tel.", &company.phone), ("E-mail", &company.email), ("PEC", &company.pec), ("Web", &company.web)]
        {
            if !v.is_empty() {
                h.push_str(&format!("<br>{label} {}", esc(v)));
            }
        }
        h.push_str("</td></tr></table><hr>");
    }
    match q.channel {
        Channel::Wholesale => {
            h.push_str("<h1>ALLEGATO A - Elenco dei servizi e condizioni economiche</h1>");
            h.push_str(&format!("<p>Allegato A n. <b>{}</b> del {}", esc(&q.number), esc(&date_it(&q.date))));
        }
        Channel::Direct => {
            h.push_str(&format!("<h1>Preventivo n. {}</h1><p>del {}", esc(&q.number), esc(&date_it(&q.date))));
        }
    }
    if let Some(e) = q.expires() {
        h.push_str(&format!(" · valido fino al {}", esc(&date_it(&e))));
    }
    if q.duration_months > 0 && !t.monthly_net.is_zero() {
        h.push_str(&format!(" · durata contrattuale {} mesi", q.duration_months));
    }
    h.push_str("</p>");
    if !q.title.is_empty() {
        h.push_str(&format!("<p><b>{}</b></p>", esc(&q.title)));
    }
    let c = &q.customer;
    h.push_str(&format!(
        "<p><span class=\"muted\">{}</span><br><b>{}</b>",
        if q.channel == Channel::Wholesale { "Partner" } else { "Spett.le" },
        esc(&c.name)
    ));
    let line = c.address_line();
    if !line.is_empty() {
        h.push_str(&format!("<br>{}", esc(&line)));
    }
    for (label, v) in [("P. IVA", &c.vat), ("C.F.", &c.fiscal_code), ("SDI", &c.sdi), ("PEC", &c.pec), ("E-mail", &c.email), ("Rif.", &c.contact)] {
        if !v.is_empty() {
            h.push_str(&format!("<br>{label} {}", esc(v)));
        }
    }
    h.push_str("</p>");
    // Lines, grouped by category in the order connectivity, hardware, services.
    for cat in Category::ALL {
        let lines: Vec<&Line> = q.lines.iter().filter(|l| l.category == cat).collect();
        if lines.is_empty() {
            continue;
        }
        h.push_str(&format!("<h2>{}</h2>", cat.label(italian)));
        h.push_str(&lines_table(&lines, Cols::Full));
    }
    if q.lines.is_empty() {
        h.push_str("<p class=\"muted\">(nessuna voce)</p>");
    }
    // Totals.
    h.push_str("<h2>Riepilogo</h2>");
    h.push_str(&totals_table(q, &t));
    if !q.notes.is_empty() {
        h.push_str(&format!("<h2>Note</h2><p>{}</p>", para(&q.notes)));
    }
    if !company.footer.is_empty() {
        h.push_str(&format!("<p class=\"muted\">{}</p>", para(&company.footer)));
    }
    if q.channel == Channel::Wholesale {
        h.push_str("<p>Il presente Allegato A forma parte integrante del contratto wholesale e ne segue le condizioni generali.</p><table class=\"head\"><tr><td>Per il Fornitore<br><br><br>______________________</td><td>Per il Partner<br><br><br>______________________</td></tr></table>");
    }
    if standalone {
        h.push_str("</body></html>");
    }
    h
}

fn cell(c: Cents) -> String {
    if c.is_zero() { "-".into() } else { c.fmt_eur() }
}

/// The mail that carries the quote: subject and body from the preferences' texts.
pub fn mail_text(q: &Quote, company: &Company, subject: &str, body: &str) -> (String, String) {
    let who = if q.customer.contact.is_empty() { q.customer.name.clone() } else { q.customer.contact.clone() };
    let sub = |t: &str| t.replace("{numero}", &q.number).replace("{cliente}", &who).replace("{azienda}", &company.name).replace("{titolo}", &q.title);
    (sub(subject), sub(body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo;

    #[test]
    fn html_and_fields_carry_the_quote() {
        let d = demo::data();
        let q = d.quotes[0].clone();
        let company = demo::company();
        let page = html(&q, &company, true);
        assert!(
            page.starts_with("<!doctype html>")
                && page.contains("Preventivo n. 2026-0001")
                && page.contains("FTTH 1 Gbps")
                && page.contains("Riepilogo")
        );
        assert!(page.contains("&lt;script&gt;"), "customer text is escaped: {page}");
        let body = html(&q, &company, false);
        assert!(!body.contains("<html") && body.contains("<h1>"));
        let f = fields(&q, &company);
        assert_eq!(f["numero"], "2026-0001");
        assert_eq!(f["data"], "16/10/2026");
        assert_eq!(f["cliente.nome"], q.customer.name);
        assert!(f["totale.canone"].ends_with(" €"));
        assert_eq!(fill("N. {{numero}} per {{cliente.nome}} {{x}}", &f), format!("N. 2026-0001 per {} {{{{x}}}}", q.customer.name));
        let mut w = q.clone();
        w.channel = Channel::Wholesale;
        let a = html(&w, &company, false);
        assert!(a.contains("ALLEGATO A") && a.contains("Per il Partner"));
        let (s, b) = mail_text(&q, &company, "Preventivo {numero} - {azienda}", "Gentile {cliente}");
        assert_eq!(s, format!("Preventivo 2026-0001 - {}", company.name));
        assert!(b.starts_with("Gentile "));
        assert!(PLACEHOLDERS.iter().all(|(p, _)| p == &"{{corpo}}" || f.contains_key(p.trim_start_matches("{{").trim_end_matches("}}"))));
    }

    #[test]
    fn tables_and_sections_list_the_lines() {
        let d = demo::data();
        let q = d.quotes[0].clone();
        let tb = tables(&q);
        assert!(TABLES.iter().all(|(p, _)| tb.contains_key(p.trim_start_matches("{{").trim_end_matches("}}"))), "{:?}", tb.keys());
        let all = &tb["tabella.righe"];
        assert!(all.starts_with("<table") && all.contains("FTTH 1 Gbps") && all.contains("Totale canone"));
        let monthly = &tb["tabella.canone"];
        assert!(monthly.contains("Totale canone mensile (imponibile)") && !monthly.contains("Una tantum</th>"), "{monthly}");
        let riepilogo = &tb["tabella.riepilogo"];
        assert!(riepilogo.contains("Imponibile") && riepilogo.contains(&q.totals().monthly_gross.fmt_eur()));
        let sc = sections(&q);
        assert!(SECTIONS.iter().all(|(p, _)| {
            let key = p.trim_start_matches("{{#").split("}}").next().unwrap_or_default();
            sc.contains_key(key)
        }));
        assert_eq!(sc["righe"].len(), q.lines.len());
        let first = &sc["righe"][0];
        assert_eq!(first["riga.n"], "1");
        assert_eq!(first["riga.nome"], q.lines[0].name);
        assert!(LINE_FIELDS.iter().all(|(p, _)| first.contains_key(p.trim_start_matches("{{").trim_end_matches("}}"))));
        let n_monthly = q.lines.iter().filter(|l| !l.monthly.is_zero()).count();
        assert_eq!(sc["righe.canone"].len(), n_monthly);
        let text = "<ul>{{#righe}}<li>{{riga.n}}. {{riga.nome}} {{riga.totale_canone}}</li>{{/righe}}</ul>{{#righe.hardware}}x{{/righe.hardware}}{{#righe}}open";
        let out = fill_sections(text, &sc);
        assert!(out.starts_with(&format!("<ul><li>1. {} ", q.lines[0].name)), "{out}");
        assert_eq!(out.matches("<li>").count(), q.lines.len());
        assert!(out.ends_with("{{#righe}}open"), "an unclosed section stays: {out}");
        assert!(!out.contains("{{/righe"));
        let empty = Quote::default();
        assert!(fill_sections("a{{#righe}}b{{/righe}}c", &sections(&empty)) == "ac");
        assert!(tables(&empty)["tabella.righe"].contains("<table"));
    }
}

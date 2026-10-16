//! A quote as a document: the HTML body (the direct layout, or the wholesale "Allegato A"),
//! the placeholder fields a letterhead can carry (`{{numero}}`, `{{cliente.nome}}`…), and
//! the text of the mail that carries it. WordCraft turns the HTML (inserted into the
//! letterhead) into DOCX and PDF.

use std::collections::BTreeMap;

use crate::config::Company;
use crate::date::Date;
use crate::model::{Category, Channel, Quote};
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
        let lines: Vec<_> = q.lines.iter().filter(|l| l.category == cat).collect();
        if lines.is_empty() {
            continue;
        }
        h.push_str(&format!("<h2>{}</h2><table border=\"1\" cellpadding=\"4\"><tr><th>Codice</th><th>Descrizione</th><th class=\"n\">Q.tà</th><th class=\"n\">Una tantum</th><th class=\"n\">Canone mensile</th><th class=\"n\">Sconto</th><th class=\"n\">Totale una tantum</th><th class=\"n\">Totale canone</th></tr>", cat.label(italian)));
        for l in lines {
            let desc = if l.description.is_empty() {
                esc(&l.name)
            } else {
                format!("<b>{}</b><br><span class=\"muted\">{}</span>", esc(&l.name), para(&l.description))
            };
            let qty = if l.unit.is_empty() { fmt_qty(l.qty) } else { format!("{} {}", fmt_qty(l.qty), esc(&l.unit)) };
            h.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td></tr>",
                esc(&l.code),
                desc,
                qty,
                cell(l.one_off),
                cell(l.monthly),
                if l.discount_pct > 0.0 { format!("{}%", fmt_qty(l.discount_pct)) } else { "-".into() },
                cell(l.one_off_net()),
                cell(l.monthly_net()),
            ));
        }
        h.push_str("</table>");
    }
    if q.lines.is_empty() {
        h.push_str("<p class=\"muted\">(nessuna voce)</p>");
    }
    // Totals.
    h.push_str("<h2>Riepilogo</h2><table border=\"1\" cellpadding=\"4\"><tr><th></th><th class=\"n\">Imponibile</th><th class=\"n\">IVA</th><th class=\"n\">Totale</th></tr>");
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
}

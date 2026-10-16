//! The price list, customers and quotes, and how a quote's totals are computed.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::money::{Cents, parse_pct, parse_qty};
use crate::{Error, Result, limits};

/// What a price list item is: a connectivity service, hardware supplied, a service rendered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    #[default]
    Connectivity,
    Hardware,
    Services,
}

impl Category {
    pub const ALL: [Self; 3] = [Self::Connectivity, Self::Hardware, Self::Services];

    pub fn id(self) -> &'static str {
        match self {
            Self::Connectivity => "connectivity",
            Self::Hardware => "hardware",
            Self::Services => "services",
        }
    }

    /// English and Italian words accepted.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "connectivity" | "connettivita" | "connettività" | "conn" | "connessione" => Some(Self::Connectivity),
            "hardware" | "hw" | "apparati" => Some(Self::Hardware),
            "services" | "service" | "servizi" | "servizio" => Some(Self::Services),
            _ => None,
        }
    }

    pub fn label(self, italian: bool) -> &'static str {
        match (self, italian) {
            (Self::Connectivity, true) => "Connettività",
            (Self::Connectivity, false) => "Connectivity",
            (Self::Hardware, true) => "Fornitura hardware",
            (Self::Hardware, false) => "Hardware",
            (Self::Services, true) => "Fornitura servizi",
            (Self::Services, false) => "Services",
        }
    }
}

/// Who the quote is for: a direct customer, or a wholesale partner (the "Allegato A").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    #[default]
    Direct,
    Wholesale,
}

impl Channel {
    pub const ALL: [Self; 2] = [Self::Direct, Self::Wholesale];

    pub fn id(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Wholesale => "wholesale",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "direct" | "diretta" | "diretto" | "retail" => Some(Self::Direct),
            "wholesale" | "allegato_a" | "allegato a" | "partner" => Some(Self::Wholesale),
            _ => None,
        }
    }

    pub fn label(self, italian: bool) -> &'static str {
        match (self, italian) {
            (Self::Direct, true) => "Diretta",
            (Self::Direct, false) => "Direct",
            (Self::Wholesale, true) => "Wholesale (Allegato A)",
            (Self::Wholesale, false) => "Wholesale (Allegato A)",
        }
    }
}

/// A list price per channel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prices {
    pub direct: Cents,
    pub wholesale: Cents,
}

impl Prices {
    pub fn for_channel(self, c: Channel) -> Cents {
        match c {
            Channel::Direct => self.direct,
            Channel::Wholesale => self.wholesale,
        }
    }
}

/// A price list item. A connectivity service has a monthly fee and a one-off activation; a
/// piece of hardware a one-off price; a service either (an hourly rate is one-off per unit).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Item {
    pub code: String,
    pub name: String,
    pub description: String,
    pub category: Category,
    /// `pz`, `mese`, `ora`, `m`…
    pub unit: String,
    pub one_off: Prices,
    pub monthly: Prices,
    pub vat_pct: f64,
    pub active: bool,
    pub tags: Vec<String>,
    /// Seconds since the epoch of the last change.
    pub updated: u64,
}

impl Item {
    /// Applies the fields given (strings or numbers; prices in euro), keeping the others.
    pub fn apply(&mut self, v: &Value) -> Result<()> {
        if let Some(x) = v.get("code") {
            self.code = limits::clip(&text(x));
        }
        if let Some(x) = v.get("name") {
            self.name = limits::clip(&text(x));
        }
        if let Some(x) = v.get("description") {
            self.description = limits::clip_long(&text(x));
        }
        if let Some(x) = v.get("category") {
            self.category =
                Category::parse(&text(x)).ok_or_else(|| Error::Invalid(format!("category {:?} (connectivity, hardware, services)", text(x))))?;
        }
        if let Some(x) = v.get("unit") {
            self.unit = limits::clip_to(&text(x), 20);
        }
        for (key, slot) in [
            ("one_off_direct", &mut self.one_off.direct),
            ("one_off_wholesale", &mut self.one_off.wholesale),
            ("monthly_direct", &mut self.monthly.direct),
            ("monthly_wholesale", &mut self.monthly.wholesale),
        ] {
            if let Some(x) = v.get(key) {
                *slot = money(x).ok_or_else(|| Error::Invalid(format!("{key}: not an amount")))?;
            }
        }
        if let Some(x) = v.get("vat_pct") {
            self.vat_pct = number(x).and_then(|n| parse_pct(&n.to_string())).ok_or_else(|| Error::Invalid("vat_pct: 0-100".into()))?;
        }
        if let Some(x) = v.get("active") {
            self.active = x.as_bool().unwrap_or_else(|| text(x) != "false");
        }
        if let Some(x) = v.get("tags") {
            self.tags = match x {
                Value::Array(a) => a.iter().map(|t| limits::clip_to(&text(t), 40)).filter(|t| !t.is_empty()).take(20).collect(),
                other => text(other).split(',').map(|t| limits::clip_to(t, 40)).filter(|t| !t.is_empty()).take(20).collect(),
            };
        }
        if self.code.is_empty() {
            return Err(Error::Invalid("code is required".into()));
        }
        if self.name.is_empty() {
            return Err(Error::Invalid("name is required".into()));
        }
        Ok(())
    }

    pub fn matches(&self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        q.is_empty()
            || self.code.to_lowercase().contains(&q)
            || self.name.to_lowercase().contains(&q)
            || self.description.to_lowercase().contains(&q)
            || self.tags.iter().any(|t| t.to_lowercase().contains(&q))
    }
}

/// A customer (direct) or a partner (wholesale).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Customer {
    pub id: u64,
    pub name: String,
    pub address: String,
    pub cap: String,
    pub city: String,
    pub province: String,
    /// Partita IVA.
    pub vat: String,
    pub fiscal_code: String,
    pub email: String,
    pub pec: String,
    /// Codice destinatario SDI.
    pub sdi: String,
    pub contact: String,
    pub phone: String,
    pub notes: String,
}

impl Customer {
    pub fn apply(&mut self, v: &Value) {
        for (key, slot) in [
            ("name", &mut self.name),
            ("address", &mut self.address),
            ("cap", &mut self.cap),
            ("city", &mut self.city),
            ("province", &mut self.province),
            ("vat", &mut self.vat),
            ("fiscal_code", &mut self.fiscal_code),
            ("email", &mut self.email),
            ("pec", &mut self.pec),
            ("sdi", &mut self.sdi),
            ("contact", &mut self.contact),
            ("phone", &mut self.phone),
        ] {
            if let Some(x) = v.get(key) {
                *slot = limits::clip(&text(x));
            }
        }
        if let Some(x) = v.get("notes") {
            self.notes = limits::clip_long(&text(x));
        }
    }

    /// `Via Roma 10, 09100 Cagliari (CA)`.
    pub fn address_line(&self) -> String {
        let mut parts = Vec::new();
        if !self.address.is_empty() {
            parts.push(self.address.clone());
        }
        let town = [self.cap.as_str(), self.city.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" ");
        let town = if self.province.is_empty() { town } else { format!("{town} ({})", self.province) };
        if !town.trim().is_empty() {
            parts.push(town.trim().to_string());
        }
        parts.join(", ")
    }

    pub fn matches(&self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        q.is_empty()
            || self.name.to_lowercase().contains(&q)
            || self.vat.contains(&q)
            || self.city.to_lowercase().contains(&q)
            || self.email.to_lowercase().contains(&q)
    }
}

/// A line of a quote: a copy of the item at the time, with quantity, discount and the
/// prices actually offered (editable).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Line {
    pub code: String,
    pub name: String,
    pub description: String,
    pub category: Category,
    pub unit: String,
    pub qty: f64,
    /// Unit prices offered.
    pub one_off: Cents,
    pub monthly: Cents,
    pub discount_pct: f64,
    pub vat_pct: f64,
}

impl Line {
    pub fn from_item(item: &Item, channel: Channel, qty: f64) -> Self {
        Self {
            code: item.code.clone(),
            name: item.name.clone(),
            description: item.description.clone(),
            category: item.category,
            unit: item.unit.clone(),
            qty,
            one_off: item.one_off.for_channel(channel),
            monthly: item.monthly.for_channel(channel),
            discount_pct: 0.0,
            vat_pct: item.vat_pct,
        }
    }

    pub fn apply(&mut self, v: &Value) -> Result<()> {
        if let Some(x) = v.get("name") {
            self.name = limits::clip(&text(x));
        }
        if let Some(x) = v.get("description") {
            self.description = limits::clip_long(&text(x));
        }
        if let Some(x) = v.get("code") {
            self.code = limits::clip(&text(x));
        }
        if let Some(x) = v.get("category") {
            self.category = Category::parse(&text(x)).ok_or_else(|| Error::Invalid("category: connectivity, hardware, services".into()))?;
        }
        if let Some(x) = v.get("unit") {
            self.unit = limits::clip_to(&text(x), 20);
        }
        if let Some(x) = v.get("qty") {
            self.qty = number(x).and_then(|n| parse_qty(&n.to_string())).ok_or_else(|| Error::Invalid("qty: a non-negative number".into()))?;
        }
        if let Some(x) = v.get("one_off") {
            self.one_off = money(x).ok_or_else(|| Error::Invalid("one_off: not an amount".into()))?;
        }
        if let Some(x) = v.get("monthly") {
            self.monthly = money(x).ok_or_else(|| Error::Invalid("monthly: not an amount".into()))?;
        }
        if let Some(x) = v.get("discount_pct") {
            self.discount_pct = number(x).and_then(|n| parse_pct(&n.to_string())).ok_or_else(|| Error::Invalid("discount_pct: 0-100".into()))?;
        }
        if let Some(x) = v.get("vat_pct") {
            self.vat_pct = number(x).and_then(|n| parse_pct(&n.to_string())).ok_or_else(|| Error::Invalid("vat_pct: 0-100".into()))?;
        }
        if self.name.is_empty() {
            return Err(Error::Invalid("name is required".into()));
        }
        Ok(())
    }

    /// One-off amount of the line, net of discount and VAT.
    pub fn one_off_net(&self) -> Cents {
        self.one_off.times(self.qty).discounted(self.discount_pct)
    }

    /// Monthly amount of the line, net of discount and VAT.
    pub fn monthly_net(&self) -> Cents {
        self.monthly.times(self.qty).discounted(self.discount_pct)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    Draft,
    Sent,
    Accepted,
    Rejected,
    Expired,
}

impl Status {
    pub const ALL: [Self; 5] = [Self::Draft, Self::Sent, Self::Accepted, Self::Rejected, Self::Expired];

    pub fn id(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Sent => "sent",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "draft" | "bozza" => Some(Self::Draft),
            "sent" | "inviato" | "inviata" => Some(Self::Sent),
            "accepted" | "accettato" | "accettata" => Some(Self::Accepted),
            "rejected" | "rifiutato" | "rifiutata" => Some(Self::Rejected),
            "expired" | "scaduto" | "scaduta" => Some(Self::Expired),
            _ => None,
        }
    }

    pub fn label(self, italian: bool) -> &'static str {
        match (self, italian) {
            (Self::Draft, true) => "Bozza",
            (Self::Draft, false) => "Draft",
            (Self::Sent, true) => "Inviato",
            (Self::Sent, false) => "Sent",
            (Self::Accepted, true) => "Accettato",
            (Self::Accepted, false) => "Accepted",
            (Self::Rejected, true) => "Rifiutato",
            (Self::Rejected, false) => "Rejected",
            (Self::Expired, true) => "Scaduto",
            (Self::Expired, false) => "Expired",
        }
    }
}

/// A quote (preventivo) or, for wholesale, an Allegato A.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Quote {
    pub id: u64,
    /// `2026-0001`.
    pub number: String,
    /// `YYYY-MM-DD`.
    pub date: String,
    pub channel: Channel,
    pub status: Status,
    pub customer: Customer,
    pub title: String,
    pub lines: Vec<Line>,
    pub notes: String,
    pub validity_days: u32,
    /// Contract duration for the monthly fees (months).
    pub duration_months: u32,
    /// The letterhead/template file name (empty: the built-in layout).
    pub template: String,
    pub author: String,
    pub created: u64,
    pub updated: u64,
    pub sent_to: Vec<String>,
    pub sent_at: u64,
    /// Files produced for this quote (names under its output folder).
    pub outputs: Vec<String>,
}

/// The sums of a quote.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Totals {
    pub one_off_net: Cents,
    pub one_off_vat: Cents,
    pub one_off_gross: Cents,
    pub monthly_net: Cents,
    pub monthly_vat: Cents,
    pub monthly_gross: Cents,
    /// Over the contract: one-off + monthly × duration (net).
    pub contract_net: Cents,
    pub contract_gross: Cents,
}

impl Quote {
    pub fn totals(&self) -> Totals {
        let mut t = Totals::default();
        for l in &self.lines {
            let o = l.one_off_net();
            let m = l.monthly_net();
            t.one_off_net = t.one_off_net.checked_add(o);
            t.one_off_vat = t.one_off_vat.checked_add(o.pct(l.vat_pct));
            t.monthly_net = t.monthly_net.checked_add(m);
            t.monthly_vat = t.monthly_vat.checked_add(m.pct(l.vat_pct));
        }
        t.one_off_gross = t.one_off_net.checked_add(t.one_off_vat);
        t.monthly_gross = t.monthly_net.checked_add(t.monthly_vat);
        let months = f64::from(self.duration_months.min(600));
        t.contract_net = t.one_off_net.checked_add(t.monthly_net.times(months));
        t.contract_gross = t.one_off_gross.checked_add(t.monthly_gross.times(months));
        t
    }

    /// Applies the editable header fields.
    pub fn apply(&mut self, v: &Value) -> Result<()> {
        if let Some(x) = v.get("title") {
            self.title = limits::clip(&text(x));
        }
        if let Some(x) = v.get("notes") {
            self.notes = limits::clip_long(&text(x));
        }
        if let Some(x) = v.get("date") {
            self.date = crate::date::Date::parse(&text(x)).ok_or_else(|| Error::Invalid("date: YYYY-MM-DD".into()))?.iso();
        }
        if let Some(x) = v.get("channel") {
            self.channel = Channel::parse(&text(x)).ok_or_else(|| Error::Invalid("channel: direct or wholesale".into()))?;
        }
        if let Some(x) = v.get("status") {
            self.status = Status::parse(&text(x)).ok_or_else(|| Error::Invalid("status: draft, sent, accepted, rejected, expired".into()))?;
        }
        if let Some(x) = v.get("validity_days") {
            self.validity_days =
                number(x).filter(|n| *n >= 0.0 && *n <= 3650.0).ok_or_else(|| Error::Invalid("validity_days: 0-3650".into()))? as u32;
        }
        if let Some(x) = v.get("duration_months") {
            self.duration_months =
                number(x).filter(|n| *n >= 0.0 && *n <= 600.0).ok_or_else(|| Error::Invalid("duration_months: 0-600".into()))? as u32;
        }
        if let Some(x) = v.get("template") {
            self.template = limits::clip_to(&text(x), 200);
        }
        if let Some(c) = v.get("customer") {
            self.customer.apply(c);
        }
        Ok(())
    }

    /// The date the offer expires (`YYYY-MM-DD`), when a validity is set.
    pub fn expires(&self) -> Option<String> {
        if self.validity_days == 0 {
            return None;
        }
        crate::date::Date::parse(&self.date).map(|d| d.plus_days(i64::from(self.validity_days)).iso())
    }

    /// A one-line description for lists.
    pub fn summary(&self, italian: bool) -> String {
        let t = self.totals();
        let who = if self.customer.name.is_empty() { if italian { "(senza cliente)" } else { "(no customer)" } } else { self.customer.name.as_str() };
        format!(
            "{} · {} · {} · {} {}/{}",
            self.number,
            who,
            self.channel.label(italian),
            t.one_off_net.fmt_eur(),
            t.monthly_net.fmt_eur(),
            if italian { "mese" } else { "month" }
        )
    }

    /// The file-name-safe base of its outputs: `2026-0001-rossi-srl`.
    pub fn file_base(&self) -> String {
        let s = crate::slug(&self.customer.name);
        if s.is_empty() { self.number.clone() } else { format!("{}-{s}", self.number) }
    }
}

/// The text of a JSON value (numbers and booleans as text).
pub fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.trim().to_string(),
        Value::Null => String::new(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

/// A number from a JSON number or a text.
pub fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64().filter(|f| f.is_finite()),
        Value::String(s) => s.trim().replace(',', ".").parse::<f64>().ok().filter(|f| f.is_finite()),
        _ => None,
    }
}

/// An amount in euro from a JSON number or a text in either notation.
pub fn money(v: &Value) -> Option<Cents> {
    match v {
        Value::Number(n) => n.as_f64().filter(|f| f.is_finite()).map(Cents::from_euro),
        Value::String(s) => Cents::parse(s),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn item() -> Item {
        let mut i = Item::default();
        i.apply(&json!({"code": "FTTH1G", "name": "FTTH 1 Gbps", "category": "connettività", "unit": "mese", "monthly_direct": "39,90", "monthly_wholesale": 25, "one_off_direct": "99", "vat_pct": 22}))
            .unwrap();
        i
    }

    #[test]
    fn items_apply_fields_in_both_languages() {
        let i = item();
        assert_eq!(
            (i.category, i.monthly.direct, i.monthly.wholesale, i.one_off.direct),
            (Category::Connectivity, Cents(3990), Cents(2500), Cents(9900))
        );
        assert!(i.matches("ftth") && !i.matches("adsl"));
        let mut bad = Item::default();
        assert!(bad.apply(&json!({"name": "x"})).is_err(), "code required");
        assert!(bad.apply(&json!({"code": "x", "name": "x", "category": "food"})).is_err());
        assert!(bad.apply(&json!({"code": "x", "name": "x", "vat_pct": 250})).is_err());
        assert!(bad.apply(&json!({"code": "x", "name": "x", "monthly_direct": "abc"})).is_err());
    }

    #[test]
    fn totals_follow_channel_discount_and_vat() {
        let i = item();
        let mut q = Quote { duration_months: 24, date: "2026-10-16".into(), validity_days: 30, ..Default::default() };
        let mut l = Line::from_item(&i, Channel::Wholesale, 2.0);
        assert_eq!((l.monthly, l.one_off), (Cents(2500), Cents(0)));
        l.apply(&json!({"discount_pct": 10})).unwrap();
        q.lines.push(l);
        let mut hw = Line { name: "Router".into(), qty: 1.0, one_off: Cents(12000), vat_pct: 22.0, ..Default::default() };
        hw.apply(&json!({"qty": "1,5"})).unwrap();
        q.lines.push(hw);
        let t = q.totals();
        assert_eq!(t.monthly_net, Cents(4500), "2 × 25,00 less 10%");
        assert_eq!(t.one_off_net, Cents(18000));
        assert_eq!(t.one_off_vat, Cents(3960));
        assert_eq!(t.monthly_gross, Cents(5490), "with 22% VAT");
        assert_eq!(t.contract_net, Cents(18000 + 4500 * 24));
        assert_eq!(q.expires(), Some("2026-11-15".into()));
        assert!(q.summary(true).contains("45,00 €/mese"));
        assert!(q.apply(&json!({"status": "nope"})).is_err());
        assert!(q.apply(&json!({"validity_days": -1})).is_err());
        q.apply(&json!({"status": "accettato", "channel": "diretta", "customer": {"name": "Rossi S.r.l.", "city": "Cagliari", "cap": "09100", "province": "CA", "address": "Via Roma 10"}})).unwrap();
        assert_eq!((q.status, q.channel), (Status::Accepted, Channel::Direct));
        assert_eq!(q.customer.address_line(), "Via Roma 10, 09100 Cagliari (CA)");
        q.number = "2026-0007".into();
        assert_eq!(q.file_base(), "2026-0007-rossi-s-r-l");
    }
}

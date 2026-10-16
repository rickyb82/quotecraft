//! Amounts in euro cents: exact sums, Italian formatting (`1.234,56`), percentages rounded
//! half away from zero. Every operation is checked: a hostile number never overflows.

use serde::{Deserialize, Serialize};

/// An amount in cents (`Cents(123456)` is 1.234,56 €).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Cents(pub i64);

/// The largest amount accepted (one billion euro): beyond it a value is a mistake.
pub const MAX: i64 = 100_000_000_000;

impl Cents {
    pub const ZERO: Self = Self(0);

    /// From euro as a float (user input): rounded to the cent, capped.
    pub fn from_euro(v: f64) -> Self {
        if !v.is_finite() {
            return Self::ZERO;
        }
        let c = (v * 100.0).round();
        Self((c.clamp(-(MAX as f64), MAX as f64)) as i64)
    }

    /// Parses `1.234,56`, `1234.56`, `1 234,56`, `€ 12`, `12`; `None` when it is not a number.
    pub fn parse(text: &str) -> Option<Self> {
        let t: String = text.trim().chars().filter(|c| !c.is_whitespace() && *c != '€').collect();
        if t.is_empty() {
            return None;
        }
        let (neg, t) = match t.strip_prefix('-') {
            Some(r) => (true, r.to_string()),
            None => (false, t),
        };
        // The last separator is the decimal one when it is followed by 1-2 digits.
        let last_sep = t.rfind([',', '.']);
        let (int_part, frac_part) = match last_sep {
            Some(i) if t.get(i + 1..).is_some_and(|f| !f.is_empty() && f.len() <= 2 && f.chars().all(|c| c.is_ascii_digit())) => {
                (t.get(..i).unwrap_or_default().to_string(), t.get(i + 1..).unwrap_or_default().to_string())
            }
            _ => (t.clone(), String::new()),
        };
        let digits: String = int_part.chars().filter(|c| *c != '.' && *c != ',').collect();
        if digits.is_empty() && frac_part.is_empty() {
            return None;
        }
        if !digits.chars().all(|c| c.is_ascii_digit()) || digits.len() > 12 {
            return None;
        }
        let euros: i64 = if digits.is_empty() { 0 } else { digits.parse().ok()? };
        let cents: i64 = match frac_part.len() {
            0 => 0,
            1 => frac_part.parse::<i64>().ok()? * 10,
            _ => frac_part.parse().ok()?,
        };
        let total = euros.checked_mul(100)?.checked_add(cents)?;
        if total > MAX {
            return None;
        }
        Some(Self(if neg { -total } else { total }))
    }

    pub fn euro(self) -> f64 {
        self.0 as f64 / 100.0
    }

    /// `1.234,56` (no currency sign).
    pub fn fmt(self) -> String {
        let neg = self.0 < 0;
        let abs = self.0.unsigned_abs();
        let (euros, cents) = (abs / 100, abs % 100);
        let s = euros.to_string();
        let mut grouped = String::new();
        for (i, c) in s.chars().enumerate() {
            if i > 0 && (s.len() - i) % 3 == 0 {
                grouped.push('.');
            }
            grouped.push(c);
        }
        format!("{}{grouped},{cents:02}", if neg { "-" } else { "" })
    }

    /// `1.234,56 €`.
    pub fn fmt_eur(self) -> String {
        format!("{} €", self.fmt())
    }

    pub fn checked_add(self, other: Self) -> Self {
        Self(self.0.checked_add(other.0).unwrap_or(MAX).clamp(-MAX, MAX))
    }

    pub fn checked_sub(self, other: Self) -> Self {
        Self(self.0.checked_sub(other.0).unwrap_or(-MAX).clamp(-MAX, MAX))
    }

    /// `self × qty`, rounded to the cent (a quantity may be fractional: hours, metres).
    pub fn times(self, qty: f64) -> Self {
        if !qty.is_finite() {
            return Self::ZERO;
        }
        Self::from_euro(self.euro() * qty.clamp(-1_000_000.0, 1_000_000.0))
    }

    /// `pct` percent of the amount, rounded half away from zero.
    pub fn pct(self, pct: f64) -> Self {
        if !pct.is_finite() {
            return Self::ZERO;
        }
        let v = (self.0 as f64) * pct.clamp(-1000.0, 1000.0) / 100.0;
        Self((v.round()).clamp(-(MAX as f64), MAX as f64) as i64)
    }

    /// The amount after a discount of `pct` percent.
    pub fn discounted(self, pct: f64) -> Self {
        self.checked_sub(self.pct(pct.clamp(0.0, 100.0)))
    }

    pub fn is_zero(self) -> bool {
        self.0 == 0
    }
}

/// A quantity as typed: `1`, `2,5`, `1.5`; non-negative, capped.
pub fn parse_qty(text: &str) -> Option<f64> {
    let t = text.trim().replace(',', ".");
    let v: f64 = t.parse().ok()?;
    (v.is_finite() && (0.0..=1_000_000.0).contains(&v)).then_some(v)
}

/// A percentage as typed: `22`, `22%`, `7,5`.
pub fn parse_pct(text: &str) -> Option<f64> {
    let t = text.trim().trim_end_matches('%').trim().replace(',', ".");
    let v: f64 = t.parse().ok()?;
    (v.is_finite() && (0.0..=100.0).contains(&v)).then_some(v)
}

/// A quantity for people: `2`, `2,5`.
pub fn fmt_qty(q: f64) -> String {
    if (q - q.round()).abs() < 1e-9 {
        format!("{}", q.round() as i64)
    } else {
        format!("{:.2}", q).replace('.', ",").trim_end_matches('0').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_italian_and_english_forms() {
        assert_eq!(Cents::parse("1.234,56"), Some(Cents(123456)));
        assert_eq!(Cents::parse("1234.56"), Some(Cents(123456)));
        assert_eq!(Cents::parse("1,234.5"), Some(Cents(123450)));
        assert_eq!(Cents::parse("€ 12"), Some(Cents(1200)));
        assert_eq!(Cents::parse("12,5"), Some(Cents(1250)));
        assert_eq!(Cents::parse("1.000"), Some(Cents(100_000)), "a thousands separator, not 1 euro");
        assert_eq!(Cents::parse("-3,10"), Some(Cents(-310)));
        assert_eq!(Cents::parse(""), None);
        assert_eq!(Cents::parse("abc"), None);
        assert_eq!(Cents::parse("9999999999999999999"), None);
        assert_eq!(Cents::parse("12.345.678,90"), Some(Cents(1_234_567_890)));
    }

    #[test]
    fn formats_with_groups() {
        assert_eq!(Cents(123456).fmt(), "1.234,56");
        assert_eq!(Cents(5).fmt(), "0,05");
        assert_eq!(Cents(-10_000_000).fmt_eur(), "-100.000,00 €");
        assert_eq!(Cents(0).fmt(), "0,00");
    }

    #[test]
    fn arithmetic_is_bounded() {
        assert_eq!(Cents(1000).pct(22.0), Cents(220));
        assert_eq!(Cents(1001).pct(50.0), Cents(501), "half rounds away from zero");
        assert_eq!(Cents(1000).discounted(10.0), Cents(900));
        assert_eq!(Cents(1000).discounted(150.0), Cents(0));
        assert_eq!(Cents(1000).times(2.5), Cents(2500));
        assert_eq!(Cents(1000).times(f64::NAN), Cents(0));
        assert_eq!(Cents(MAX).checked_add(Cents(MAX)), Cents(MAX));
        assert_eq!(Cents::from_euro(f64::INFINITY), Cents(0));
        assert_eq!(parse_qty("2,5"), Some(2.5));
        assert_eq!(parse_qty("-1"), None);
        assert_eq!(parse_pct("22%"), Some(22.0));
        assert_eq!(parse_pct("120"), None);
        assert_eq!(fmt_qty(2.0), "2");
        assert_eq!(fmt_qty(2.5), "2,5");
    }
}

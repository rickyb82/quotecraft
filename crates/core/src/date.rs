//! Calendar dates without a calendar crate: today from the system clock, `YYYY-MM-DD` in
//! and out, Italian `GG/MM/AAAA` for documents, days added for validity.

/// A civil date.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: i64,
    pub month: u32,
    pub day: u32,
}

/// Days since 1970-01-01 → civil date (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> Date {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    Date { year: y, month: m, day: d }
}

/// Civil date → days since 1970-01-01.
fn days_from_civil(d: Date) -> i64 {
    let y = if d.month <= 2 { d.year - 1 } else { d.year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(d.month);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + i64::from(d.day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

impl Date {
    pub fn today() -> Self {
        let secs = web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        civil_from_days(secs.div_euclid(86_400))
    }

    /// `YYYY-MM-DD` (a loose read: `2026-1-5` works too).
    pub fn parse(text: &str) -> Option<Self> {
        let mut it = text.trim().split(['-', '/']);
        let a: i64 = it.next()?.trim().parse().ok()?;
        let b: u32 = it.next()?.trim().parse().ok()?;
        let c: i64 = it.next()?.trim().parse().ok()?;
        if it.next().is_some() {
            return None;
        }
        // DD/MM/YYYY when the first number is a day.
        let (year, month, day) = if a > 31 { (a, b, c as u32) } else { (c, b, a as u32) };
        let d = Self { year, month, day };
        d.valid().then_some(d)
    }

    fn valid(self) -> bool {
        (1900..=2200).contains(&self.year) && (1..=12).contains(&self.month) && self.day >= 1 && self.day <= days_in_month(self.year, self.month)
    }

    /// `YYYY-MM-DD`.
    pub fn iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// `GG/MM/AAAA`.
    pub fn italian(self) -> String {
        format!("{:02}/{:02}/{:04}", self.day, self.month, self.year)
    }

    pub fn plus_days(self, days: i64) -> Self {
        civil_from_days(days_from_civil(self).saturating_add(days.clamp(-100_000, 100_000)))
    }
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Seconds since the epoch, for stamps.
pub fn now_secs() -> u64 {
    web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// `YYYY-MM-DD HH:MM` (UTC) for a stamp.
pub fn stamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let d = civil_from_days(days);
    let rem = secs % 86_400;
    format!("{} {:02}:{:02}", d.iso(), rem / 3600, (rem % 3600) / 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_adds() {
        let d = Date::parse("2026-10-16").unwrap();
        assert_eq!(d.italian(), "16/10/2026");
        assert_eq!(d.plus_days(30).iso(), "2026-11-15");
        assert_eq!(Date::parse("16/10/2026"), Some(d));
        assert_eq!(Date::parse("2024-02-29").map(|d| d.iso()), Some("2024-02-29".into()));
        assert_eq!(Date::parse("2023-02-29"), None);
        assert_eq!(Date::parse("nope"), None);
        assert_eq!(Date::parse("2026-13-01"), None);
        assert_eq!(days_from_civil(civil_from_days(20_000)), 20_000);
        assert!(Date::today().year >= 2026);
        assert_eq!(stamp(0), "1970-01-01 00:00");
    }
}

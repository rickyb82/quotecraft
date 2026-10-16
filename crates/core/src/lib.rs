//! QuoteCraft's core (layer L0): money, dates, the price list, customers and quotes with their
//! totals, the JSON store, the configuration, the HTML of a quote and the fields for a
//! letterhead. No network, no process: wasm-safe. The documents (DOCX, PDF) are made by
//! WordCraft and the mail is sent by MailCraft, through the `bridge` crate.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod bridge;
pub mod config;
pub mod date;
pub mod demo;
pub mod model;
pub mod money;
pub mod render;
pub mod store;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid {0}")]
    Invalid(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("not configured: {0}")]
    NotConfigured(String),
    #[error("i/o: {0}")]
    Io(String),
    #[error("{0}")]
    External(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Bounds on everything that comes from outside (files, parameters).
pub mod limits {
    /// Longest text kept in a field.
    pub const TEXT: usize = 4_000;
    /// Longest free text (descriptions, notes, mail bodies).
    pub const LONG_TEXT: usize = 20_000;
    /// Most items in a price list.
    pub const ITEMS: usize = 20_000;
    /// Most customers kept.
    pub const CUSTOMERS: usize = 20_000;
    /// Most quotes kept.
    pub const QUOTES: usize = 50_000;
    /// Most lines in a quote.
    pub const LINES: usize = 200;
    /// Most rows a listing returns by default.
    pub const ROWS: usize = 500;
    /// Largest attachment handed to the mail (bytes).
    pub const ATTACHMENT: usize = 25 * 1024 * 1024;

    /// A field value: trimmed and cut at [`TEXT`] characters.
    pub fn clip(s: &str) -> String {
        clip_to(s, TEXT)
    }

    /// Free text: trimmed and cut at [`LONG_TEXT`] characters.
    pub fn clip_long(s: &str) -> String {
        clip_to(s, LONG_TEXT)
    }

    pub fn clip_to(s: &str, n: usize) -> String {
        s.trim().chars().take(n).collect()
    }
}

/// A lower-case file-name-safe id for a name (users, output folders).
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in name.trim().chars().take(80) {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_file_safe() {
        assert_eq!(slug("Anna Rossi / 2026"), "anna-rossi-2026");
        assert_eq!(slug("  ../../etc  "), "etc");
        assert_eq!(slug(""), "");
    }

    #[test]
    fn clip_cuts_at_chars() {
        assert_eq!(limits::clip_to("àbcdef", 3), "àbc");
        assert_eq!(limits::clip("  x  "), "x");
    }
}

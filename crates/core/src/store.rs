//! The data: price list, customers, quotes and counters, in one JSON file (`data.json`)
//! written atomically. Shared by every session of a server.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{Customer, Item, Quote};
use crate::{Error, Result, limits};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Data {
    pub items: Vec<Item>,
    pub customers: Vec<Customer>,
    pub quotes: Vec<Quote>,
    /// Quote numbers per year: `"2026" → 7` (the last used).
    pub counters: BTreeMap<String, u64>,
    pub next_customer_id: u64,
    pub next_quote_id: u64,
}

impl Data {
    /// Keeps the data within the limits whatever the file said.
    fn bound(&mut self) {
        self.items.truncate(limits::ITEMS);
        self.customers.truncate(limits::CUSTOMERS);
        self.quotes.truncate(limits::QUOTES);
        for q in &mut self.quotes {
            q.lines.truncate(limits::LINES);
        }
        if self.next_customer_id == 0 {
            self.next_customer_id = self.customers.iter().map(|c| c.id).max().unwrap_or(0) + 1;
        }
        if self.next_quote_id == 0 {
            self.next_quote_id = self.quotes.iter().map(|q| q.id).max().unwrap_or(0) + 1;
        }
    }

    pub fn item(&self, code: &str) -> Option<&Item> {
        let c = code.trim();
        self.items.iter().find(|i| i.code.eq_ignore_ascii_case(c))
    }

    pub fn item_mut(&mut self, code: &str) -> Option<&mut Item> {
        let c = code.trim();
        self.items.iter_mut().find(|i| i.code.eq_ignore_ascii_case(c))
    }

    pub fn quote(&self, id: u64) -> Option<&Quote> {
        self.quotes.iter().find(|q| q.id == id)
    }

    pub fn quote_mut(&mut self, id: u64) -> Option<&mut Quote> {
        self.quotes.iter_mut().find(|q| q.id == id)
    }

    /// The next number of a year: `2026-0001`.
    pub fn next_number(&mut self, year: i64) -> String {
        let key = year.to_string();
        let n = self.counters.get(&key).copied().unwrap_or(0).saturating_add(1);
        self.counters.insert(key, n);
        format!("{year}-{n:04}")
    }

    pub fn new_customer_id(&mut self) -> u64 {
        let id = self.next_customer_id.max(1);
        self.next_customer_id = id + 1;
        id
    }

    pub fn new_quote_id(&mut self) -> u64 {
        let id = self.next_quote_id.max(1);
        self.next_quote_id = id + 1;
        id
    }
}

/// The data file and the folders next to it.
pub struct Store {
    /// `None`: in memory only (demo, tests).
    path: Option<PathBuf>,
    pub data: Data,
}

impl Store {
    pub fn in_memory(data: Data) -> Self {
        let mut data = data;
        data.bound();
        Self { path: None, data }
    }

    /// Opens (or starts) the data file; an unreadable file is reported, not overwritten.
    pub fn open(path: &Path) -> Result<Self> {
        let data = match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str::<Data>(&text).map_err(|e| Error::Io(format!("{}: {e}", path.display())))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Data::default(),
            Err(e) => return Err(Error::Io(format!("{}: {e}", path.display()))),
        };
        let mut s = Self { path: Some(path.to_path_buf()), data };
        s.data.bound();
        Ok(s)
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Writes the file (temporary file, then rename).
    pub fn save(&self) -> Result<()> {
        let Some(path) = &self.path else { return Ok(()) };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| Error::Io(format!("{}: {e}", dir.display())))?;
        }
        let text = serde_json::to_string_pretty(&self.data).map_err(|e| Error::Io(e.to_string()))?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text).map_err(|e| Error::Io(format!("{}: {e}", tmp.display())))?;
        std::fs::rename(&tmp, path).map_err(|e| Error::Io(format!("{}: {e}", path.display())))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_ids_grow_and_the_file_round_trips() {
        let dir = std::env::temp_dir().join(format!("quotecraft-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("data.json");
        let mut s = Store::open(&path).unwrap();
        assert_eq!(s.data.next_number(2026), "2026-0001");
        assert_eq!(s.data.next_number(2026), "2026-0002");
        assert_eq!(s.data.next_number(2027), "2027-0001");
        assert_eq!(s.data.new_quote_id(), 1);
        assert_eq!(s.data.new_customer_id(), 1);
        s.save().unwrap();
        let s2 = Store::open(&path).unwrap();
        assert_eq!(s2.data.counters.get("2026"), Some(&2));
        assert_eq!(s2.data.next_quote_id, 2);
        std::fs::write(&path, "{ not json").unwrap();
        assert!(Store::open(&path).is_err(), "a broken file is an error, never overwritten");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bounds_apply_to_loaded_data() {
        let mut d = Data::default();
        d.quotes.push(Quote { id: 9, lines: vec![crate::model::Line::default(); limits::LINES + 5], ..Default::default() });
        let s = Store::in_memory(d);
        assert_eq!(s.data.quotes[0].lines.len(), limits::LINES);
        assert_eq!(s.data.next_quote_id, 10);
    }
}

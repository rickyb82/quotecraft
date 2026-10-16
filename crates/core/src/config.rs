//! Configuration (`config.json`): the company on the documents, preferences, where
//! WordCraft and MailCraft are; secrets (`secrets.json`, owner-only): MailCraft's token.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::text;
use crate::{Error, Result, limits};

/// The company issuing the quotes (header of the built-in layouts, fields for letterheads).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Company {
    pub name: String,
    pub address: String,
    pub cap: String,
    pub city: String,
    pub province: String,
    pub vat: String,
    pub fiscal_code: String,
    pub email: String,
    pub pec: String,
    pub phone: String,
    pub web: String,
    pub iban: String,
    /// Closing lines of every quote (payment terms, legal notes).
    pub footer: String,
}

impl Company {
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
            ("phone", &mut self.phone),
            ("web", &mut self.web),
            ("iban", &mut self.iban),
        ] {
            if let Some(x) = v.get(key) {
                *slot = limits::clip(&text(x));
            }
        }
        if let Some(x) = v.get("footer") {
            self.footer = limits::clip_long(&text(x));
        }
    }

    pub fn address_line(&self) -> String {
        let c = crate::model::Customer {
            address: self.address.clone(),
            cap: self.cap.clone(),
            city: self.city.clone(),
            province: self.province.clone(),
            ..Default::default()
        };
        c.address_line()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// `en`, `it`, or empty for the system's.
    pub language: String,
    /// `light`, `dark`, or empty for the system's.
    pub theme: String,
    pub default_vat_pct: f64,
    pub default_validity_days: u32,
    pub default_duration_months: u32,
    /// Letterhead file names (in the templates folder) used by default per channel.
    pub template_direct: String,
    pub template_wholesale: String,
    /// The folder with the letterheads (empty: `templates` next to the data file).
    pub templates_dir: String,
    /// `wordcraft-cli` (on the PATH, or a full path): makes DOCX and PDF.
    pub wordcraft_cli: String,
    /// MailCraft's server (`http://mailcraft:7878`): sends the mail as the user.
    pub mailcraft_url: String,
    /// Subject and body proposed for the mail (`{numero}`, `{cliente}`, `{azienda}`).
    pub mail_subject: String,
    pub mail_body: String,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            language: String::new(),
            theme: String::new(),
            default_vat_pct: 22.0,
            default_validity_days: 30,
            default_duration_months: 24,
            template_direct: String::new(),
            template_wholesale: String::new(),
            templates_dir: String::new(),
            wordcraft_cli: "wordcraft-cli".into(),
            mailcraft_url: String::new(),
            mail_subject: "Preventivo {numero} - {azienda}".into(),
            mail_body: "Gentile {cliente},\n\nin allegato il preventivo {numero} come concordato.\nRestiamo a disposizione per ogni chiarimento.\n\nCordiali saluti\n{azienda}".into(),
        }
    }
}

impl Preferences {
    pub fn apply(&mut self, v: &Value) -> Result<()> {
        let Some(obj) = v.as_object() else { return Err(Error::Invalid("preferences must be an object".into())) };
        for (key, val) in obj {
            match key.as_str() {
                "language" => self.language = limits::clip_to(&text(val), 8),
                "theme" => self.theme = limits::clip_to(&text(val), 8),
                "default_vat_pct" => {
                    self.default_vat_pct = crate::model::number(val)
                        .filter(|n| (0.0..=100.0).contains(n))
                        .ok_or_else(|| Error::Invalid("default_vat_pct: 0-100".into()))?
                }
                "default_validity_days" => {
                    self.default_validity_days = crate::model::number(val)
                        .filter(|n| (0.0..=3650.0).contains(n))
                        .ok_or_else(|| Error::Invalid("default_validity_days: 0-3650".into()))?
                        as u32
                }
                "default_duration_months" => {
                    self.default_duration_months = crate::model::number(val)
                        .filter(|n| (0.0..=600.0).contains(n))
                        .ok_or_else(|| Error::Invalid("default_duration_months: 0-600".into()))?
                        as u32
                }
                "template_direct" => self.template_direct = limits::clip_to(&text(val), 200),
                "template_wholesale" => self.template_wholesale = limits::clip_to(&text(val), 200),
                "templates_dir" => self.templates_dir = limits::clip(&text(val)),
                "wordcraft_cli" => self.wordcraft_cli = limits::clip(&text(val)),
                "mailcraft_url" => self.mailcraft_url = limits::clip(&text(val)).trim_end_matches('/').to_string(),
                "mail_subject" => self.mail_subject = limits::clip(&text(val)),
                "mail_body" => self.mail_body = limits::clip_long(&text(val)),
                _ => {}
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub company: Company,
    pub preferences: Preferences,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| Error::Io(format!("{}: {e}", path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(Error::Io(format!("{}: {e}", path.display()))),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        write_atomic(path, &serde_json::to_string_pretty(self).map_err(|e| Error::Io(e.to_string()))?, false)
    }
}

/// What never goes in `config.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Secrets {
    /// MailCraft's service token (the framework's), so the mail goes out as the user.
    pub mailcraft_token: String,
}

impl Secrets {
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| Error::Io(format!("{}: {e}", path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(Error::Io(format!("{}: {e}", path.display()))),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        write_atomic(path, &serde_json::to_string_pretty(self).map_err(|e| Error::Io(e.to_string()))?, true)
    }
}

fn write_atomic(path: &Path, text: &str, private: bool) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::Io(format!("{}: {e}", dir.display())))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text).map_err(|e| Error::Io(format!("{}: {e}", tmp.display())))?;
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = private;
    std::fs::rename(&tmp, path).map_err(|e| Error::Io(format!("{}: {e}", path.display())))?;
    Ok(())
}

/// Where everything lives: one directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paths {
    pub dir: PathBuf,
}

impl Paths {
    pub fn under(dir: &Path) -> Self {
        Self { dir: dir.to_path_buf() }
    }

    /// `$QUOTECRAFT_HOME`, else the platform's configuration directory.
    pub fn default_dirs() -> Self {
        if let Some(h) = std::env::var_os("QUOTECRAFT_HOME") {
            return Self::under(Path::new(&h));
        }
        let base = if cfg!(windows) {
            std::env::var_os("APPDATA").map(PathBuf::from)
        } else if cfg!(target_os = "macos") {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
        } else {
            std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        };
        Self::under(&base.unwrap_or_else(|| PathBuf::from(".")).join("quotecraft"))
    }

    pub fn config(&self) -> PathBuf {
        self.dir.join("config.json")
    }

    pub fn secrets(&self) -> PathBuf {
        self.dir.join("secrets.json")
    }

    pub fn data(&self) -> PathBuf {
        self.dir.join("data.json")
    }

    /// The letterheads: the preference, or `templates` in the directory.
    pub fn templates(&self, prefs: &Preferences) -> PathBuf {
        if prefs.templates_dir.trim().is_empty() { self.dir.join("templates") } else { PathBuf::from(prefs.templates_dir.trim()) }
    }

    /// Where the documents produced go: `out/<quote base>/`.
    pub fn out(&self) -> PathBuf {
        self.dir.join("out")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn preferences_apply_and_files_round_trip() {
        let dir = std::env::temp_dir().join(format!("quotecraft-config-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let paths = Paths::under(&dir);
        let mut c = Config::default();
        c.preferences.apply(&json!({"language": "it", "default_vat_pct": 10, "mailcraft_url": "http://mailcraft:7878/", "unknown": 1})).unwrap();
        assert!(c.preferences.apply(&json!({"default_vat_pct": 200})).is_err());
        assert!(c.preferences.apply(&json!("x")).is_err());
        c.company.apply(&json!({"name": "Sirius Tec", "city": "Cagliari", "cap": "09100", "province": "CA", "address": "Via Roma 1"}));
        c.save(&paths.config()).unwrap();
        let back = Config::load(&paths.config()).unwrap();
        assert_eq!(back.preferences.mailcraft_url, "http://mailcraft:7878");
        assert_eq!(back.company.address_line(), "Via Roma 1, 09100 Cagliari (CA)");
        let s = Secrets { mailcraft_token: "t".into() };
        s.save(&paths.secrets()).unwrap();
        assert_eq!(Secrets::load(&paths.secrets()).unwrap(), s);
        assert!(paths.templates(&back.preferences).ends_with("templates"));
        assert_eq!(Config::load(&dir.join("missing.json")).unwrap(), Config::default());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

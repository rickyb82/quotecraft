//! Interface languages. Labels are written in English in the code and looked up here; anything
//! without a translation shows in English. Messages with values use `{name}` placeholders.

mod it;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    En,
    It,
}

impl Language {
    pub const ALL: [Self; 2] = [Self::En, Self::It];

    pub const fn name(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::It => "Italiano",
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::It => "it",
        }
    }

    pub fn parse(tag: &str) -> Option<Self> {
        match tag.trim().split(['-', '_', '.']).next()?.to_ascii_lowercase().as_str() {
            "en" => Some(Self::En),
            "it" => Some(Self::It),
            _ => None,
        }
    }

    pub fn system() -> Self {
        sys_locale::get_locale().and_then(|tag| Self::parse(&tag)).unwrap_or_default()
    }

    pub fn tr(self, text: &str) -> &str {
        match self {
            Self::En => text,
            Self::It => it::translate(text).unwrap_or(text),
        }
    }

    pub fn fill(self, text: &str, values: &[(&str, &str)]) -> String {
        let mut out = self.tr(text).to_string();
        for (k, v) in values {
            out = out.replace(&format!("{{{k}}}"), v);
        }
        out
    }

    /// Translates the engine's error messages by their leading phrase, keeping the details.
    pub fn tr_message(self, text: &str) -> String {
        if self == Self::En {
            return text.to_string();
        }
        for (en, itx) in it::MESSAGES {
            if let Some(rest) = text.strip_prefix(en) {
                return format!("{itx}{rest}");
            }
        }
        text.to_string()
    }
}

/// Every label the interface translates (kept in step with the `it` table by a test).
pub const LABELS: &[&str] = &[
    "Quotes",
    "New quote",
    "Price list",
    "Customers",
    "Letterheads",
    "Settings",
    "Light theme",
    "Dark theme",
    "Language",
    "Ready",
    "Saved",
    "Jobs pending: {n}",
    "Server unreachable ({url}): {error}. Start it with: quotecraft-cli serve",
    "Search",
    "All",
    "Mine",
    "Everyone's",
    "Direct",
    "Wholesale (Allegato A)",
    "Draft",
    "Sent",
    "Accepted",
    "Rejected",
    "Expired",
    "Number",
    "Date",
    "Customer",
    "Title",
    "One-off",
    "Monthly",
    "Author",
    "Status",
    "Channel",
    "No quotes yet: make one with New quote.",
    "Open",
    "Duplicate",
    "Delete",
    "Delete this quote?",
    "Yes, delete",
    "Cancel",
    "Back to the quotes",
    "Quote {number}",
    "Header",
    "Validity (days)",
    "Duration (months)",
    "Letterhead",
    "Built-in layout",
    "Notes",
    "Save header",
    "Pick a customer",
    "Type to search the customers",
    "Name",
    "Address",
    "Postcode",
    "City",
    "Province",
    "VAT number",
    "Fiscal code",
    "E-mail",
    "PEC",
    "SDI code",
    "Contact",
    "Phone",
    "Save customer",
    "Save to the customers list",
    "Lines",
    "Add from the price list",
    "Add a custom line",
    "Code",
    "Description",
    "Qty",
    "Unit",
    "Discount %",
    "VAT %",
    "Total",
    "Connectivity",
    "Hardware",
    "Services",
    "No lines yet: add from the price list.",
    "Totals",
    "Net",
    "VAT",
    "Gross",
    "Contract value ({months} months)",
    "Documents",
    "Make the documents",
    "Makes the HTML, and the PDF and DOCX through WordCraft (letterhead included).",
    "Send by mail",
    "To",
    "Cc",
    "Subject",
    "Body",
    "Send",
    "Sent through MailCraft from your account: {status}",
    "Files",
    "Download",
    "Making the documents…",
    "Sending…",
    "Documents ready: {names}",
    "Mail sent to {to}",
    "Items: {n}",
    "Inactive too",
    "Add item",
    "Edit",
    "Category",
    "Direct price",
    "Wholesale price",
    "Active",
    "Tags",
    "Save item",
    "Remove item",
    "Only an administrator changes the price list.",
    "Add customer",
    "Remove customer",
    "No customers yet.",
    "Letterheads folder: {dir}",
    "A letterhead is a .docx (WordCraft fills it) or an .html file with placeholders; the quote's body goes at {{corpo}}.",
    "Placeholders",
    "Fields",
    "Tables (the lines laid out by QuoteCraft, where you put them)",
    "Repeating sections (HTML letterheads): the text between the tags, once per line",
    "The fields of a line, inside a section",
    "Attach a file…",
    "Remove from the quote",
    "No document yet. Make them, or drop a file on this window to attach it.",
    "{name} attached",
    "Import a letterhead (path on the server)",
    "Add a letterhead",
    "Choose a file…",
    "or drop a .docx or .html file on this window",
    "Import from a path on the server",
    "{name} is larger than 50 MB",
    "Import",
    "Remove",
    "Default for direct quotes",
    "Default for wholesale",
    "None",
    "No letterheads yet: put .docx or .html files in the folder, or import one.",
    "Company",
    "Web",
    "IBAN",
    "Footer of every quote",
    "Save company",
    "Preferences",
    "Default VAT %",
    "Default validity (days)",
    "Default duration (months)",
    "Save preferences",
    "Documents and mail",
    "WordCraft CLI (makes DOCX and PDF)",
    "MailCraft server URL",
    "MailCraft service token",
    "Mail subject",
    "Mail body",
    "Check WordCraft and MailCraft",
    "Renderer: {renderer}",
    "Sender: {sender}",
    "Only an administrator changes the settings.",
    "Administrator",
    "Only administrators change the price list, the letterheads and the settings.",
    "QuoteCraft keeps your price lists (connectivity, hardware, services) and writes the quotes: direct ones and the wholesale Allegato A. WordCraft makes the documents, MailCraft sends them.",
    "months",
    "month",
    "Open quote",
    "expires {date}",
    "Set",
    "Item",
    "File name",
    "Unsaved changes",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_label_has_an_italian_translation() {
        let missing: Vec<&str> = LABELS.iter().copied().filter(|l| it::translate(l).is_none()).collect();
        assert!(missing.is_empty(), "missing Italian: {missing:?}");
        assert_eq!(Language::parse("it-IT"), Some(Language::It));
        assert_eq!(Language::It.fill("Jobs pending: {n}", &[("n", "3")]), "Operazioni in corso: 3");
        assert!(Language::It.tr_message("not found: quote 9").starts_with("non trovato"));
    }
}

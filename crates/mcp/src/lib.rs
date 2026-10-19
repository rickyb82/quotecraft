//! QuoteCraft's MCP server (layer L6): the Model Context Protocol over stdio (newline-delimited
//! JSON-RPC 2.0), so an assistant can read and change the price list (administrators), write
//! quotes, make the documents and send them. Every tool is an engine command.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![recursion_limit = "512"]

use std::io::{BufRead, Write};

use quotecraft_engine::{QuoteSession, Session};
use serde_json::{Value, json};

pub const PROTOCOL_VERSION: &str = "2025-06-18";
pub const SUPPORTED_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
pub const QUOTE_URI: &str = "quotecraft://quote";
pub const PRICELIST_URI: &str = "quotecraft://pricelist";
const MAX_LINE: usize = 8 * 1024 * 1024;

const INSTRUCTIONS: &str = "QuoteCraft keeps the price lists (connectivity, hardware, services; each item with a direct and a wholesale price, \
one-off and monthly) and writes the quotes: direct ones (preventivo) and the wholesale Allegato A. Workflow: list_items to find the items \
(by code), new_quote with the channel and the customer (customer_id from list_customers, or the customer's fields), add_line for every \
item (code + qty, discount_pct when agreed) or a custom line (name + prices), then get_quote to read the totals. make_documents writes the \
HTML and, through WordCraft, the PDF and DOCX (on the letterhead chosen); a document made elsewhere (a revised PDF, a drawing) joins \
the quote with attach_document; send_quote mails them through MailCraft from the user's own account (the formats, or the files named): \
ask the person before sending. Changing the price list, the letterheads, the company and the settings needs an administrator: \
the server refuses otherwise. Amounts are euro (\"39,90\" or 39.9); quantities may be fractional.";

type ToolResult = Result<Value, String>;

pub struct Server {
    session: Box<dyn QuoteSession>,
}

fn tools() -> Value {
    let id = json!({"type": "integer", "description": "Quote id (default: the open quote)"});
    let money = json!({"type": ["number", "string"], "description": "Euro, e.g. 39.9 or \"39,90\""});
    let pct = json!({"type": ["number", "string"], "minimum": 0, "maximum": 100});
    let category = json!({"type": "string", "enum": ["connectivity", "hardware", "services"]});
    let channel = json!({"type": "string", "enum": ["direct", "wholesale"], "description": "direct = preventivo, wholesale = Allegato A"});
    let customer_fields = json!({"type": "object", "properties": {"name": {"type": "string"}, "address": {"type": "string"}, "cap": {"type": "string"}, "city": {"type": "string"}, "province": {"type": "string"}, "vat": {"type": "string", "description": "Partita IVA"}, "fiscal_code": {"type": "string"}, "email": {"type": "string"}, "pec": {"type": "string"}, "sdi": {"type": "string"}, "contact": {"type": "string"}, "phone": {"type": "string"}, "notes": {"type": "string"}}});
    let item_fields = json!({"name": {"type": "string"}, "description": {"type": "string"}, "category": category, "unit": {"type": "string", "description": "pz, mese, ora…"}, "one_off_direct": money, "one_off_wholesale": money, "monthly_direct": money, "monthly_wholesale": money, "vat_pct": pct, "active": {"type": "boolean"}, "tags": {"type": "array", "items": {"type": "string"}}});
    let mut item_props = item_fields.clone();
    item_props["code"] = json!({"type": "string"});
    json!([
        {"name": "list_items", "description": "The price list: items with code, name, category, unit, one-off and monthly prices for direct and wholesale, VAT. Filter by text and category.",
         "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}, "category": category, "all": {"type": "boolean", "description": "Inactive items too"}, "limit": {"type": "integer"}}}},
        {"name": "get_item", "description": "One item by code.", "inputSchema": {"type": "object", "properties": {"code": {"type": "string"}}, "required": ["code"]}},
        {"name": "save_item", "description": "Add an item to the price list or change it (administrators only). Only the fields given change.",
         "inputSchema": {"type": "object", "properties": item_props, "required": ["code"]}},
        {"name": "remove_item", "description": "Remove an item from the price list (administrators only).", "inputSchema": {"type": "object", "properties": {"code": {"type": "string"}}, "required": ["code"]}},
        {"name": "import_items", "description": "Add or change many items at once (administrators only); replace=true empties the list first.",
         "inputSchema": {"type": "object", "properties": {"rows": {"type": "array", "items": {"type": "object", "properties": item_props}}, "replace": {"type": "boolean"}}, "required": ["rows"]}},
        {"name": "list_customers", "description": "The customers (and wholesale partners) with their ids.", "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}, "limit": {"type": "integer"}}}},
        {"name": "save_customer", "description": "Add a customer (no id) or change one (id).", "inputSchema": {"type": "object", "properties": {"id": {"type": "integer"}, "name": {"type": "string"}, "address": {"type": "string"}, "cap": {"type": "string"}, "city": {"type": "string"}, "province": {"type": "string"}, "vat": {"type": "string"}, "fiscal_code": {"type": "string"}, "email": {"type": "string"}, "pec": {"type": "string"}, "sdi": {"type": "string"}, "contact": {"type": "string"}, "phone": {"type": "string"}, "notes": {"type": "string"}}}},
        {"name": "remove_customer", "description": "Remove a customer.", "inputSchema": {"type": "object", "properties": {"id": {"type": "integer"}}, "required": ["id"]}},
        {"name": "list_quotes", "description": "The quotes, newest first: number, date, customer, channel, status, totals, author.",
         "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}, "status": {"type": "string", "enum": ["draft", "sent", "accepted", "rejected", "expired"]}, "channel": channel, "author": {"type": "string"}, "mine": {"type": "boolean"}, "limit": {"type": "integer"}}}},
        {"name": "new_quote", "description": "Start a quote (it becomes the open quote): channel, the customer (customer_id, or the fields), title, notes, letterhead.",
         "inputSchema": {"type": "object", "properties": {"channel": channel, "customer_id": {"type": "integer"}, "customer": customer_fields, "title": {"type": "string"}, "notes": {"type": "string"}, "template": {"type": "string", "description": "Letterhead file name (from list_letterheads); default: the channel's"}}}},
        {"name": "get_quote", "description": "A quote with its lines, totals (one-off, monthly, contract value, VAT) and the documents produced. Opens it.",
         "inputSchema": {"type": "object", "properties": {"id": id}}},
        {"name": "set_quote", "description": "Change a quote's header: title, notes, date, channel, status, validity_days, duration_months, template, customer.",
         "inputSchema": {"type": "object", "properties": {"id": id, "title": {"type": "string"}, "notes": {"type": "string"}, "date": {"type": "string", "description": "YYYY-MM-DD"}, "channel": channel, "status": {"type": "string", "enum": ["draft", "sent", "accepted", "rejected", "expired"]}, "validity_days": {"type": "integer"}, "duration_months": {"type": "integer"}, "template": {"type": "string"}, "customer_id": {"type": "integer"}, "customer": customer_fields}}},
        {"name": "delete_quote", "description": "Delete a quote.", "inputSchema": {"type": "object", "properties": {"id": {"type": "integer"}}, "required": ["id"]}},
        {"name": "duplicate_quote", "description": "Copy a quote into a new draft with a new number.", "inputSchema": {"type": "object", "properties": {"id": id}}},
        {"name": "add_line", "description": "Add a line: an item from the price list (code, qty; the channel's prices; discount_pct, one_off or monthly to override) or a custom line (name, category, qty, one_off, monthly, vat_pct).",
         "inputSchema": {"type": "object", "properties": {"id": id, "code": {"type": "string"}, "qty": {"type": ["number", "string"]}, "name": {"type": "string"}, "description": {"type": "string"}, "category": category, "unit": {"type": "string"}, "one_off": money, "monthly": money, "discount_pct": pct, "vat_pct": pct, "at": {"type": "integer", "description": "Position (0-based); default: the end"}}}},
        {"name": "set_line", "description": "Change a line (index from get_quote, 0-based): qty, prices, discount, VAT, name, description.",
         "inputSchema": {"type": "object", "properties": {"id": id, "index": {"type": "integer"}, "qty": {"type": ["number", "string"]}, "one_off": money, "monthly": money, "discount_pct": pct, "vat_pct": pct, "name": {"type": "string"}, "description": {"type": "string"}, "category": category, "unit": {"type": "string"}}, "required": ["index"]}},
        {"name": "remove_line", "description": "Remove a line.", "inputSchema": {"type": "object", "properties": {"id": id, "index": {"type": "integer"}}, "required": ["index"]}},
        {"name": "move_line", "description": "Move a line to another position.", "inputSchema": {"type": "object", "properties": {"id": id, "index": {"type": "integer"}, "to": {"type": "integer"}}, "required": ["index", "to"]}},
        {"name": "quote_html", "description": "The quote as HTML (a whole page and the body alone), the placeholder fields and the tables (HTML fragments) a letterhead can use, for another tool to lay out.",
         "inputSchema": {"type": "object", "properties": {"id": id}}},
        {"name": "make_documents", "description": "Write the quote's documents: the HTML always, and the PDF and DOCX through WordCraft (on the letterhead chosen, if any). Returns the files.",
         "inputSchema": {"type": "object", "properties": {"id": id, "formats": {"type": "array", "items": {"type": "string", "enum": ["pdf", "docx", "html"]}}}}},
        {"name": "send_quote", "description": "Mail the documents through MailCraft from the user's own account. Ask the person before calling it. Defaults: to the customer's e-mail, the preferences' subject and body, the PDF and DOCX made by make_documents.",
         "inputSchema": {"type": "object", "properties": {"id": id, "to": {"type": "string"}, "cc": {"type": "string"}, "subject": {"type": "string"}, "body": {"type": "string"}, "formats": {"type": "array", "items": {"type": "string", "enum": ["pdf", "docx", "html"]}}, "files": {"type": "array", "items": {"type": "string"}, "description": "The documents to attach by name (from get_quote's outputs), instead of the formats"}}}},
        {"name": "attach_document", "description": "Add a document made elsewhere (a revised PDF, a drawing, a spreadsheet) to the quote: it is kept in the quote's folder, listed with the documents and can be mailed with send_quote. Give the file's base64 in data (or a path on the server).",
         "inputSchema": {"type": "object", "properties": {"id": id, "name": {"type": "string", "description": "File name with its extension (pdf, docx, xlsx, html, png, jpg…)"}, "data": {"type": "string", "description": "base64 of the file"}, "path": {"type": "string", "description": "A path on the server, instead of data"}}, "required": ["name"]}},
        {"name": "remove_document", "description": "Remove a document from the quote (made or attached): it leaves the list and the folder.", "inputSchema": {"type": "object", "properties": {"id": id, "name": {"type": "string"}}, "required": ["name"]}},
        {"name": "list_letterheads", "description": "The letterheads (.docx or .html in the templates folder) and the placeholders they may carry: fields, tables ({{tabella.canone}}…) and repeating sections ({{#righe}}…{{/righe}}: in HTML around any text, in a .docx at the start and end of a table row with {{riga.*}} fields).", "inputSchema": {"type": "object", "properties": {}}},
        {"name": "import_letterhead", "description": "Add a letterhead from a path on the server, or from base64 data (administrators only).",
         "inputSchema": {"type": "object", "properties": {"path": {"type": "string"}, "data": {"type": "string", "description": "base64 of the file"}, "name": {"type": "string", "description": "File name ending in .docx or .html"}}}},
        {"name": "remove_letterhead", "description": "Remove a letterhead (administrators only).", "inputSchema": {"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]}},
        {"name": "get_company", "description": "The company on the documents.", "inputSchema": {"type": "object", "properties": {}}},
        {"name": "set_company", "description": "Change the company on the documents (administrators only).",
         "inputSchema": {"type": "object", "properties": {"name": {"type": "string"}, "address": {"type": "string"}, "cap": {"type": "string"}, "city": {"type": "string"}, "province": {"type": "string"}, "vat": {"type": "string"}, "fiscal_code": {"type": "string"}, "email": {"type": "string"}, "pec": {"type": "string"}, "phone": {"type": "string"}, "web": {"type": "string"}, "iban": {"type": "string"}, "footer": {"type": "string"}}}},
        {"name": "get_preferences", "description": "Preferences: defaults (VAT, validity, duration), letterheads per channel, WordCraft and MailCraft.", "inputSchema": {"type": "object", "properties": {}}},
        {"name": "set_preferences", "description": "Change preferences (language and theme: anyone; the rest: administrators only). Only the fields given change.",
         "inputSchema": {"type": "object", "properties": {"language": {"type": "string", "enum": ["en", "it"]}, "theme": {"type": "string", "enum": ["light", "dark"]}, "default_vat_pct": pct, "default_validity_days": {"type": "integer"}, "default_duration_months": {"type": "integer"}, "template_direct": {"type": "string"}, "template_wholesale": {"type": "string"}, "templates_dir": {"type": "string"}, "wordcraft_cli": {"type": "string"}, "mailcraft_url": {"type": "string"}, "mailcraft_token": {"type": "string"}, "mail_subject": {"type": "string"}, "mail_body": {"type": "string"}}}},
        {"name": "check_bridges", "description": "Whether WordCraft (documents) and MailCraft (mail) are reachable.", "inputSchema": {"type": "object", "properties": {}}},
    ])
}

fn prompts() -> Value {
    json!([
        {"name": "write_quote", "description": "Write a quote from a request in plain words", "arguments": [
            {"name": "request", "description": "Who the customer is, the channel (direct or wholesale) and what they need", "required": true},
            {"name": "language", "description": "en or it", "required": false}]},
    ])
}

fn prompt_text(name: &str, args: &Value) -> Option<(&'static str, String)> {
    let language = args.get("language").and_then(Value::as_str).unwrap_or("it");
    match name {
        "write_quote" => {
            let request = args.get("request").and_then(Value::as_str)?;
            Some((
                "Write a quote",
                format!(
                    "Write a quote for: {request}\n\n1. Find the customer with list_customers (or take the fields from the request).\n2. Find the items with list_items (search by words; mind the category) and note their codes.\n3. new_quote with the channel (wholesale when it is a partner's Allegato A), then add_line for every item with the quantity; use discount_pct only when the request says so; a custom line for anything not in the price list.\n4. get_quote and answer in {language} with the lines and the totals (one-off, monthly, contract value), then ask whether to make the documents (make_documents) and whether to send them (send_quote). Never send without a yes."
                ),
            ))
        }
        _ => None,
    }
}

fn pass_through(args: &Value, keys: &[&str]) -> Value {
    let mut v = json!({});
    for k in keys {
        if let Some(x) = args.get(k) {
            v[k] = x.clone();
        }
    }
    v
}

impl Default for Server {
    fn default() -> Self {
        Self::new(Session::demo())
    }
}

impl Server {
    pub fn new(session: impl QuoteSession + 'static) -> Self {
        Self { session: Box::new(session) }
    }

    pub fn session(&self) -> &dyn QuoteSession {
        self.session.as_ref()
    }

    fn run(&mut self, command: &str, params: Value) -> ToolResult {
        self.session.run_blocking(command, params).map_err(|e| e.to_string())
    }

    fn call(&mut self, name: &str, args: &Value) -> ToolResult {
        const ITEM: &[&str] = &[
            "code",
            "name",
            "description",
            "category",
            "unit",
            "one_off_direct",
            "one_off_wholesale",
            "monthly_direct",
            "monthly_wholesale",
            "vat_pct",
            "active",
            "tags",
        ];
        const CUSTOMER: &[&str] =
            &["id", "name", "address", "cap", "city", "province", "vat", "fiscal_code", "email", "pec", "sdi", "contact", "phone", "notes"];
        const LINE: &[&str] =
            &["id", "index", "code", "qty", "name", "description", "category", "unit", "one_off", "monthly", "discount_pct", "vat_pct", "at", "to"];
        match name {
            "list_items" => self.run("items.list", pass_through(args, &["query", "category", "all", "limit"])),
            "get_item" => self.run("items.get", pass_through(args, &["code"])),
            "save_item" => self.run("items.save", pass_through(args, ITEM)),
            "remove_item" => self.run("items.remove", pass_through(args, &["code"])),
            "import_items" => self.run("items.import", pass_through(args, &["rows", "replace"])),
            "list_customers" => self.run("customers.list", pass_through(args, &["query", "limit"])),
            "save_customer" => self.run("customers.save", pass_through(args, CUSTOMER)),
            "remove_customer" => self.run("customers.remove", pass_through(args, &["id"])),
            "list_quotes" => self.run("quotes.list", pass_through(args, &["query", "status", "channel", "author", "mine", "limit"])),
            "new_quote" => self.run("quote.new", pass_through(args, &["channel", "customer_id", "customer", "title", "notes", "template"])),
            "get_quote" => match args.get("id") {
                Some(_) => self.run("quote.open", pass_through(args, &["id"])),
                None => self.run("quote.get", json!({})),
            },
            "set_quote" => self.run(
                "quote.set",
                pass_through(
                    args,
                    &["id", "title", "notes", "date", "channel", "status", "validity_days", "duration_months", "template", "customer_id", "customer"],
                ),
            ),
            "delete_quote" => self.run("quote.delete", pass_through(args, &["id"])),
            "duplicate_quote" => self.run("quote.duplicate", pass_through(args, &["id"])),
            "add_line" => self.run("quote.addLine", pass_through(args, LINE)),
            "set_line" => self.run("quote.setLine", pass_through(args, LINE)),
            "remove_line" => self.run("quote.removeLine", pass_through(args, &["id", "index"])),
            "move_line" => self.run("quote.moveLine", pass_through(args, &["id", "index", "to"])),
            "quote_html" => self.run("quote.html", pass_through(args, &["id"])),
            "make_documents" => self.run("quote.render", pass_through(args, &["id", "formats"])),
            "send_quote" => self.run("quote.send", pass_through(args, &["id", "to", "cc", "subject", "body", "formats", "files"])),
            "attach_document" => self.run("quote.attach", pass_through(args, &["id", "name", "data", "path"])),
            "remove_document" => self.run("quote.detach", pass_through(args, &["id", "name"])),
            "list_letterheads" => self.run("templates.list", json!({})),
            "import_letterhead" => self.run("templates.import", pass_through(args, &["path", "data", "name"])),
            "remove_letterhead" => self.run("templates.remove", pass_through(args, &["name"])),
            "get_company" => self.run("company.get", json!({})),
            "set_company" => self.run(
                "company.set",
                pass_through(
                    args,
                    &["name", "address", "cap", "city", "province", "vat", "fiscal_code", "email", "pec", "phone", "web", "iban", "footer"],
                ),
            ),
            "get_preferences" => self.run("prefs.get", json!({})),
            "set_preferences" => self.run(
                "prefs.set",
                pass_through(
                    args,
                    &[
                        "language",
                        "theme",
                        "default_vat_pct",
                        "default_validity_days",
                        "default_duration_months",
                        "template_direct",
                        "template_wholesale",
                        "templates_dir",
                        "wordcraft_cli",
                        "mailcraft_url",
                        "mailcraft_token",
                        "mail_subject",
                        "mail_body",
                    ],
                ),
            ),
            "check_bridges" => self.run("bridge.check", json!({})),
            _ => Err(format!("unknown tool `{}`", name.chars().take(40).collect::<String>())),
        }
    }

    fn quote_text(&self) -> String {
        match &self.session.state().quote {
            Some(q) => quotecraft_engine::core::render::html(q, &self.session.config().company, false),
            None => "(no open quote)".into(),
        }
    }

    fn pricelist_text(&self) -> String {
        self.session
            .state()
            .items
            .iter()
            .map(|i| {
                format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    i.code,
                    i.name,
                    i.category.id(),
                    i.unit,
                    i.one_off.direct.fmt(),
                    i.one_off.wholesale.fmt(),
                    i.monthly.direct.fmt(),
                    i.monthly.wholesale.fmt()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Handles one JSON-RPC message; `None` for notifications.
    pub fn handle(&mut self, msg: &Value) -> Option<Value> {
        let id = msg.get("id").cloned()?;
        let method = msg.get("method").and_then(Value::as_str).unwrap_or_default();
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": params.get("protocolVersion").and_then(Value::as_str).filter(|v| SUPPORTED_VERSIONS.contains(v)).unwrap_or(PROTOCOL_VERSION),
                "capabilities": {"tools": {"listChanged": false}, "resources": {"listChanged": false, "subscribe": false}, "prompts": {"listChanged": false}},
                "serverInfo": {"name": "quotecraft", "title": "QuoteCraft", "version": env!("CARGO_PKG_VERSION")},
                "instructions": INSTRUCTIONS,
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": tools()})),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                Ok(match self.call(name, &args) {
                    Ok(v) => {
                        let text = match v.get("summary").and_then(Value::as_str) {
                            Some(t) => t.to_string(),
                            None => serde_json::to_string_pretty(&v).unwrap_or_default(),
                        };
                        let structured = if v.is_object() { v } else { json!({"result": v}) };
                        json!({"content": [{"type": "text", "text": text}], "structuredContent": structured, "isError": false})
                    }
                    Err(e) => json!({"content": [{"type": "text", "text": e}], "isError": true}),
                })
            }
            "prompts/list" => Ok(json!({"prompts": prompts()})),
            "prompts/get" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                match prompt_text(name, &args) {
                    Some((description, t)) => {
                        Ok(json!({"description": description, "messages": [{"role": "user", "content": {"type": "text", "text": t}}]}))
                    }
                    None => Err((-32602, format!("unknown prompt or missing arguments: {}", name.chars().take(40).collect::<String>()))),
                }
            }
            "resources/list" => Ok(json!({"resources": [
                {"uri": QUOTE_URI, "name": "quote", "title": "Open quote", "description": "The open quote as HTML", "mimeType": "text/html"},
                {"uri": PRICELIST_URI, "name": "pricelist", "title": "Price list", "description": "The price list (tab separated: code, name, category, unit, one-off direct/wholesale, monthly direct/wholesale)", "mimeType": "text/plain"},
            ]})),
            "resources/templates/list" => Ok(json!({"resourceTemplates": []})),
            "resources/read" => match params.get("uri").and_then(Value::as_str) {
                Some(QUOTE_URI) => Ok(json!({"contents": [{"uri": QUOTE_URI, "mimeType": "text/html", "text": self.quote_text()}]})),
                Some(PRICELIST_URI) => Ok(json!({"contents": [{"uri": PRICELIST_URI, "mimeType": "text/plain", "text": self.pricelist_text()}]})),
                _ => Err((-32002, "resource not found".to_string())),
            },
            _ => Err((-32601, format!("method not found: {}", method.chars().take(64).collect::<String>()))),
        };
        Some(match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err((code, message)) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}),
        })
    }

    /// Serves until `input` ends: one JSON-RPC message per line in, one response per line out.
    pub fn serve(&mut self, input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
        for line in input.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let reply = if line.len() > MAX_LINE {
                Some(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32600, "message": "request too large"}}))
            } else {
                match serde_json::from_str::<Value>(&line) {
                    Ok(Value::Array(batch)) => {
                        let replies: Vec<Value> = batch.iter().filter_map(|m| self.handle(m)).collect();
                        (!replies.is_empty()).then_some(Value::Array(replies))
                    }
                    Ok(msg) => self.handle(&msg),
                    Err(e) => Some(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("parse error: {e}")}})),
                }
            };
            if let Some(r) = reply {
                writeln!(output, "{r}")?;
                output.flush()?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(s: &mut Server, name: &str, args: Value) -> Value {
        let r = s.handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": name, "arguments": args}})).unwrap();
        assert_eq!(r["result"]["isError"], false, "{r}");
        r["result"].clone()
    }

    #[test]
    fn an_agent_writes_a_quote() {
        let mut s = Server::default();
        let init = s.handle(&json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"protocolVersion": "2024-11-05"}})).unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2024-11-05");
        assert!(s.handle(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).is_none());
        let tools = s.handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})).unwrap();
        assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 31);
        let r = call(&mut s, "list_items", json!({"query": "ftth", "category": "connectivity"}));
        assert_eq!(r["structuredContent"]["items"].as_array().unwrap().len(), 2);
        let r = call(&mut s, "list_customers", json!({"query": "partner"}));
        let cid = r["structuredContent"]["customers"][0]["id"].as_u64().unwrap();
        let r = call(&mut s, "new_quote", json!({"channel": "wholesale", "customer_id": cid, "title": "Allegato A"}));
        assert_eq!(r["structuredContent"]["number"], "2026-0002");
        let r = call(&mut s, "add_line", json!({"code": "FTTH-2.5G", "qty": 2, "discount_pct": 5}));
        assert_eq!(r["structuredContent"]["lines"][0]["monthly"], 3990);
        call(&mut s, "add_line", json!({"name": "Progetto", "category": "services", "one_off": "250"}));
        let r = call(&mut s, "get_quote", json!({}));
        assert!(r["content"][0]["text"].as_str().unwrap().starts_with("2026-0002 · Partner Reti"), "{}", r["content"][0]["text"]);
        assert_eq!(r["structuredContent"]["totals"]["one_off_net"], 9310 + 25000, "2 × 49,00 less 5% + 250,00");
        let r = call(&mut s, "attach_document", json!({"name": "disegno.png", "data": "iVBORw0K"}));
        assert_eq!(r["structuredContent"]["attached"], "disegno.png", "{r}");
        let r = call(&mut s, "remove_document", json!({"name": "disegno.png"}));
        assert_eq!(r["structuredContent"]["outputs"].as_array().map(Vec::len), Some(0), "{r}");
        let r = call(&mut s, "make_documents", json!({"formats": ["html"]}));
        assert_eq!(r["structuredContent"]["files"].as_array().unwrap().len(), 1);
        let bad = s
            .handle(&json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "set_line", "arguments": {"index": 9, "qty": 1}}}))
            .unwrap();
        assert_eq!(bad["result"]["isError"], true);
        let res = s.handle(&json!({"jsonrpc": "2.0", "id": 5, "method": "resources/read", "params": {"uri": QUOTE_URI}})).unwrap();
        assert!(res["result"]["contents"][0]["text"].as_str().unwrap().contains("ALLEGATO A"));
        let res = s.handle(&json!({"jsonrpc": "2.0", "id": 5, "method": "resources/read", "params": {"uri": PRICELIST_URI}})).unwrap();
        assert!(res["result"]["contents"][0]["text"].as_str().unwrap().contains("FTTH-1G\t"));
        let p = s.handle(&json!({"jsonrpc": "2.0", "id": 7, "method": "prompts/get", "params": {"name": "write_quote", "arguments": {"request": "FTTH per Rossi"}}})).unwrap();
        assert!(p["result"]["messages"][0]["content"]["text"].as_str().unwrap().contains("add_line"));
        let prefs = call(&mut s, "get_preferences", json!({}));
        assert!(prefs["structuredContent"]["preferences"].get("mailcraft_token").is_none(), "the token is never returned");
        let mut out = Vec::new();
        s.serve(std::io::Cursor::new("{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"ping\"}\nnot json\n"), &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("\"id\":9") && text.contains("parse error"));
    }
}

//! The command table: every action of the interface, the CLI and the MCP server. Local
//! commands answer at once; `quote.render`, `quote.send` and `bridge.check` queue a job and
//! answer `{"job": id}`. Changing the price list, the letterheads, the company and the
//! settings needs an administrator (the framework says who is one).

use quotecraft_core::bridge::{OutgoingMail, RenderJob, template_path};
use quotecraft_core::model::*;
use quotecraft_core::money::parse_qty;
use quotecraft_core::{Error as CoreError, limits};
use serde_json::{Value, json};

use crate::{Error, JobKind, QuoteSummary, Result, Session, TemplateInfo, quote_json};

pub struct Command {
    pub id: &'static str,
    pub label: &'static str,
    /// The parameters, for `--help` and the MCP tool list.
    pub params: &'static str,
    /// Needs an administrator.
    pub admin: bool,
    /// Answers at once (no job).
    pub local: bool,
    pub run: fn(&mut Session, &Value) -> Result<Value>,
}

fn s(v: &Value, key: &str) -> String {
    v.get(key).map(text).unwrap_or_default()
}

fn id_of(v: &Value, key: &str) -> Option<u64> {
    v.get(key).and_then(|x| x.as_u64().or_else(|| x.as_str().and_then(|t| t.trim().parse().ok())))
}

fn limit(v: &Value, default: usize) -> usize {
    v.get("limit")
        .and_then(|l| l.as_u64().or_else(|| l.as_str().and_then(|t| t.parse().ok())))
        .map_or(default, |l| usize::try_from(l).unwrap_or(default))
        .clamp(1, limits::QUOTES)
}

fn flag(v: &Value, key: &str) -> bool {
    v.get(key).is_some_and(|x| x.as_bool().unwrap_or_else(|| matches!(text(x).as_str(), "true" | "1" | "yes")))
}

fn lock(sess: &Session) -> Result<std::sync::MutexGuard<'_, quotecraft_core::store::Store>> {
    sess.store.lock().map_err(|_| Error::Param("the data is locked".into()))
}

// ---- Price list ----

fn items_list(sess: &mut Session, v: &Value) -> Result<Value> {
    let query = s(v, "query");
    let category = v
        .get("category")
        .map(text)
        .filter(|t| !t.is_empty())
        .map(|t| Category::parse(&t).ok_or_else(|| Error::Param("category: connectivity, hardware, services".into())))
        .transpose()?;
    let all = flag(v, "all");
    let n = limit(v, limits::ITEMS);
    let store = lock(sess)?;
    let items: Vec<&Item> =
        store.data.items.iter().filter(|i| (all || i.active) && category.is_none_or(|c| i.category == c) && i.matches(&query)).take(n).collect();
    Ok(json!({"items": items, "total": store.data.items.len()}))
}

fn items_get(sess: &mut Session, v: &Value) -> Result<Value> {
    let code = s(v, "code");
    let store = lock(sess)?;
    let item = store.data.item(&code).ok_or_else(|| Error::Core(CoreError::NotFound(format!("item {code}"))))?;
    Ok(json!({"item": item}))
}

fn items_save(sess: &mut Session, v: &Value) -> Result<Value> {
    sess.require_admin("change the price list")?;
    let code = s(v, "code");
    if code.is_empty() {
        return Err(Error::Param("code is required".into()));
    }
    let item = {
        let mut store = lock(sess)?;
        let created = store.data.item(&code).is_none();
        if created && store.data.items.len() >= limits::ITEMS {
            return Err(Error::Param("the price list is full".into()));
        }
        let default_vat = sess.config.preferences.default_vat_pct;
        let item = match store.data.item_mut(&code) {
            Some(i) => i,
            None => {
                store.data.items.push(Item { code: code.clone(), vat_pct: default_vat, active: true, ..Default::default() });
                store.data.items.last_mut().ok_or_else(|| Error::Param("no item".into()))?
            }
        };
        let mut patch = v.clone();
        if let Some(o) = patch.as_object_mut() {
            o.remove("code");
        }
        item.apply(&patch)?;
        item.updated = quotecraft_core::date::now_secs();
        let item = item.clone();
        store.save()?;
        item
    };
    sess.refresh_lists();
    Ok(json!({"item": item}))
}

fn items_remove(sess: &mut Session, v: &Value) -> Result<Value> {
    sess.require_admin("change the price list")?;
    let code = s(v, "code");
    {
        let mut store = lock(sess)?;
        let before = store.data.items.len();
        store.data.items.retain(|i| !i.code.eq_ignore_ascii_case(code.trim()));
        if store.data.items.len() == before {
            return Err(Error::Core(CoreError::NotFound(format!("item {code}"))));
        }
        store.save()?;
    }
    sess.refresh_lists();
    Ok(json!({"removed": code}))
}

fn items_import(sess: &mut Session, v: &Value) -> Result<Value> {
    sess.require_admin("change the price list")?;
    let rows = v.get("rows").and_then(Value::as_array).ok_or_else(|| Error::Param("rows: a list of items (code, name, category, prices…)".into()))?;
    let replace = flag(v, "replace");
    let mut saved = 0usize;
    let mut errors = Vec::new();
    {
        let mut store = lock(sess)?;
        if replace {
            store.data.items.clear();
        }
        for (n, row) in rows.iter().take(limits::ITEMS).enumerate() {
            let code = s(row, "code");
            if code.is_empty() {
                errors.push(format!("row {}: no code", n + 1));
                continue;
            }
            let default_vat = sess.config.preferences.default_vat_pct;
            if store.data.item(&code).is_none() {
                if store.data.items.len() >= limits::ITEMS {
                    errors.push(format!("row {}: the price list is full", n + 1));
                    break;
                }
                store.data.items.push(Item { code: code.clone(), vat_pct: default_vat, active: true, ..Default::default() });
            }
            let Some(item) = store.data.item_mut(&code) else { continue };
            let mut patch = row.clone();
            if let Some(o) = patch.as_object_mut() {
                o.remove("code");
            }
            match item.apply(&patch) {
                Ok(()) => {
                    item.updated = quotecraft_core::date::now_secs();
                    saved += 1;
                }
                Err(e) => errors.push(format!("row {} ({code}): {e}", n + 1)),
            }
        }
        store.save()?;
    }
    sess.refresh_lists();
    Ok(json!({"saved": saved, "errors": errors}))
}

// ---- Customers ----

fn customers_list(sess: &mut Session, v: &Value) -> Result<Value> {
    let query = s(v, "query");
    let n = limit(v, limits::CUSTOMERS);
    let store = lock(sess)?;
    let list: Vec<&Customer> = store.data.customers.iter().filter(|c| c.matches(&query)).take(n).collect();
    Ok(json!({"customers": list}))
}

fn customers_save(sess: &mut Session, v: &Value) -> Result<Value> {
    let c = {
        let mut store = lock(sess)?;
        let id = id_of(v, "id").unwrap_or(0);
        let c = if id == 0 {
            if store.data.customers.len() >= limits::CUSTOMERS {
                return Err(Error::Param("too many customers".into()));
            }
            let id = store.data.new_customer_id();
            store.data.customers.push(Customer { id, ..Default::default() });
            store.data.customers.last_mut().ok_or_else(|| Error::Param("no customer".into()))?
        } else {
            store.data.customers.iter_mut().find(|c| c.id == id).ok_or_else(|| Error::Core(CoreError::NotFound(format!("customer {id}"))))?
        };
        c.apply(v);
        if c.name.is_empty() {
            let id = c.id;
            store.data.customers.retain(|x| x.id != id || !x.name.is_empty());
            return Err(Error::Param("name is required".into()));
        }
        let c = c.clone();
        store.save()?;
        c
    };
    sess.refresh_lists();
    Ok(json!({"customer": c}))
}

fn customers_remove(sess: &mut Session, v: &Value) -> Result<Value> {
    let id = id_of(v, "id").ok_or_else(|| Error::Param("id is required".into()))?;
    {
        let mut store = lock(sess)?;
        let before = store.data.customers.len();
        store.data.customers.retain(|c| c.id != id);
        if store.data.customers.len() == before {
            return Err(Error::Core(CoreError::NotFound(format!("customer {id}"))));
        }
        store.save()?;
    }
    sess.refresh_lists();
    Ok(json!({"removed": id}))
}

// ---- Quotes ----

fn quotes_list(sess: &mut Session, v: &Value) -> Result<Value> {
    let query = s(v, "query").to_lowercase();
    let status = v
        .get("status")
        .map(text)
        .filter(|t| !t.is_empty())
        .map(|t| Status::parse(&t).ok_or_else(|| Error::Param("status: draft, sent, accepted, rejected, expired".into())))
        .transpose()?;
    let channel = v
        .get("channel")
        .map(text)
        .filter(|t| !t.is_empty())
        .map(|t| Channel::parse(&t).ok_or_else(|| Error::Param("channel: direct or wholesale".into())))
        .transpose()?;
    let author = if flag(v, "mine") { sess.user.clone() } else { s(v, "author") };
    let n = limit(v, limits::ROWS);
    sess.refresh_lists();
    let list: Vec<&QuoteSummary> = sess
        .state
        .quotes
        .iter()
        .filter(|q| status.is_none_or(|st| q.status == st))
        .filter(|q| channel.is_none_or(|c| q.channel == c))
        .filter(|q| author.is_empty() || q.author == author)
        .filter(|q| {
            query.is_empty()
                || q.number.to_lowercase().contains(&query)
                || q.customer.to_lowercase().contains(&query)
                || q.title.to_lowercase().contains(&query)
        })
        .take(n)
        .collect();
    Ok(json!({"quotes": list}))
}

fn quote_new(sess: &mut Session, v: &Value) -> Result<Value> {
    let channel = v
        .get("channel")
        .map(text)
        .filter(|t| !t.is_empty())
        .map(|t| Channel::parse(&t).ok_or_else(|| Error::Param("channel: direct or wholesale".into())))
        .transpose()?
        .unwrap_or_default();
    let prefs = sess.config.preferences.clone();
    let q = {
        let mut store = lock(sess)?;
        if store.data.quotes.len() >= limits::QUOTES {
            return Err(Error::Param("too many quotes".into()));
        }
        let today = quotecraft_core::date::Date::today();
        let customer = match id_of(v, "customer_id") {
            Some(id) => {
                store.data.customers.iter().find(|c| c.id == id).cloned().ok_or_else(|| Error::Core(CoreError::NotFound(format!("customer {id}"))))?
            }
            None => {
                let mut c = Customer::default();
                if let Some(cv) = v.get("customer") {
                    c.apply(cv);
                }
                c
            }
        };
        let now = quotecraft_core::date::now_secs();
        let template = v.get("template").map(text).unwrap_or_else(|| match channel {
            Channel::Direct => prefs.template_direct.clone(),
            Channel::Wholesale => prefs.template_wholesale.clone(),
        });
        let mut q = Quote {
            id: store.data.new_quote_id(),
            number: store.data.next_number(today.year),
            date: today.iso(),
            channel,
            status: Status::Draft,
            customer,
            title: limits::clip(&s(v, "title")),
            validity_days: prefs.default_validity_days,
            duration_months: prefs.default_duration_months,
            template: limits::clip_to(&template, 200),
            author: sess.user.clone(),
            created: now,
            updated: now,
            ..Default::default()
        };
        if let Some(n) = v.get("notes") {
            q.notes = limits::clip_long(&text(n));
        }
        store.data.quotes.push(q.clone());
        store.save()?;
        q
    };
    sess.refresh_lists();
    sess.state.quote = Some(q.clone());
    sess.state.outputs.clear();
    Ok(quote_json(&q, sess.italian()))
}

/// The quote a command works on: `id`, else the open one.
fn target(sess: &Session, v: &Value) -> Result<u64> {
    id_of(v, "id")
        .or_else(|| sess.state.quote.as_ref().map(|q| q.id))
        .ok_or_else(|| Error::Param("no quote: give `id` or open one with quote.open".into()))
}

fn quote_open(sess: &mut Session, v: &Value) -> Result<Value> {
    let id = id_of(v, "id").ok_or_else(|| Error::Param("id is required".into()))?;
    let q = lock(sess)?.data.quote(id).cloned().ok_or_else(|| Error::Core(CoreError::NotFound(format!("quote {id}"))))?;
    sess.state.quote = Some(q.clone());
    sess.state.outputs = outputs_of(sess, &q);
    sess.touch();
    Ok(quote_json(&q, sess.italian()))
}

/// The files already produced for a quote, from its folder.
fn outputs_of(sess: &Session, q: &Quote) -> Vec<quotecraft_core::bridge::Produced> {
    let dir = sess.out_dir(q);
    q.outputs
        .iter()
        .filter_map(|n| {
            let path = dir.join(n);
            let bytes = std::fs::metadata(&path).ok()?.len();
            Some(quotecraft_core::bridge::Produced { name: n.clone(), path, bytes })
        })
        .collect()
}

fn quote_get(sess: &mut Session, v: &Value) -> Result<Value> {
    let id = target(sess, v)?;
    let q = lock(sess)?.data.quote(id).cloned().ok_or_else(|| Error::Core(CoreError::NotFound(format!("quote {id}"))))?;
    Ok(quote_json(&q, sess.italian()))
}

fn quote_set(sess: &mut Session, v: &Value) -> Result<Value> {
    let id = target(sess, v)?;
    let customer_id = id_of(v, "customer_id");
    let customer = match customer_id {
        Some(cid) => Some(
            lock(sess)?
                .data
                .customers
                .iter()
                .find(|c| c.id == cid)
                .cloned()
                .ok_or_else(|| Error::Core(CoreError::NotFound(format!("customer {cid}"))))?,
        ),
        None => None,
    };
    let before = lock(sess)?.data.quote(id).map(|q| sess.out_dir(q));
    let q = sess.with_quote(id, |q| {
        if let Some(c) = customer {
            q.customer = c;
        }
        q.apply(v)?;
        q.updated = quotecraft_core::date::now_secs();
        Ok(())
    })?;
    // The folder is named after the number and the customer: its documents follow a rename.
    let after = sess.out_dir(&q);
    if let Some(before) = before
        && before != after
        && before.is_dir()
        && !after.exists()
    {
        std::fs::rename(&before, &after).map_err(|e| Error::Core(CoreError::Io(format!("{}: {e}", before.display()))))?;
    }
    if sess.state.quote.as_ref().is_some_and(|o| o.id == id) {
        sess.state.outputs = outputs_of(sess, &q);
    }
    Ok(quote_json(&q, sess.italian()))
}

fn quote_delete(sess: &mut Session, v: &Value) -> Result<Value> {
    let id = id_of(v, "id").ok_or_else(|| Error::Param("id is required".into()))?;
    {
        let mut store = lock(sess)?;
        let before = store.data.quotes.len();
        store.data.quotes.retain(|q| q.id != id);
        if store.data.quotes.len() == before {
            return Err(Error::Core(CoreError::NotFound(format!("quote {id}"))));
        }
        store.save()?;
    }
    if sess.state.quote.as_ref().is_some_and(|q| q.id == id) {
        sess.state.quote = None;
        sess.state.outputs.clear();
    }
    sess.refresh_lists();
    Ok(json!({"deleted": id}))
}

fn quote_duplicate(sess: &mut Session, v: &Value) -> Result<Value> {
    let id = target(sess, v)?;
    let q = {
        let mut store = lock(sess)?;
        if store.data.quotes.len() >= limits::QUOTES {
            return Err(Error::Param("too many quotes".into()));
        }
        let mut q = store.data.quote(id).cloned().ok_or_else(|| Error::Core(CoreError::NotFound(format!("quote {id}"))))?;
        let today = quotecraft_core::date::Date::today();
        q.id = store.data.new_quote_id();
        q.number = store.data.next_number(today.year);
        q.date = today.iso();
        q.status = Status::Draft;
        q.author = sess.user.clone();
        q.created = quotecraft_core::date::now_secs();
        q.updated = q.created;
        q.sent_to.clear();
        q.sent_at = 0;
        // The drawings come along (their pictures copied); the other documents do not.
        let (from, to) = (store.data.quote(id).map(|o| sess.out_dir(o)), sess.out_dir(&q));
        q.outputs.clear();
        let mut copied: Vec<String> = Vec::new();
        for d in &q.drawings {
            if copied.contains(&d.file) {
                continue;
            }
            if let Some(src) = from.as_ref().map(|f| f.join(&d.file)).filter(|p| p.is_file())
                && std::fs::create_dir_all(&to).and_then(|_| std::fs::copy(&src, to.join(&d.file))).is_ok()
            {
                copied.push(d.file.clone());
            }
        }
        q.drawings.retain(|d| copied.contains(&d.file));
        q.outputs = copied;
        store.data.quotes.push(q.clone());
        store.save()?;
        q
    };
    sess.refresh_lists();
    sess.state.quote = Some(q.clone());
    sess.state.outputs = outputs_of(sess, &q);
    Ok(quote_json(&q, sess.italian()))
}

fn quote_add_line(sess: &mut Session, v: &Value) -> Result<Value> {
    let id = target(sess, v)?;
    let qty =
        v.get("qty").map(text).map(|t| parse_qty(&t).ok_or_else(|| Error::Param("qty: a non-negative number".into()))).transpose()?.unwrap_or(1.0);
    let code = s(v, "code");
    let channel = lock(sess)?.data.quote(id).map(|q| q.channel).ok_or_else(|| Error::Core(CoreError::NotFound(format!("quote {id}"))))?;
    let default_vat = sess.config.preferences.default_vat_pct;
    let mut line = if code.is_empty() {
        Line { qty, vat_pct: default_vat, ..Default::default() }
    } else {
        let item = lock(sess)?.data.item(&code).cloned().ok_or_else(|| Error::Core(CoreError::NotFound(format!("item {code}"))))?;
        Line::from_item(&item, channel, qty)
    };
    // Overrides (a custom line, or a different price for this quote).
    let mut patch = v.clone();
    if let Some(o) = patch.as_object_mut() {
        o.remove("id");
        o.remove("code");
    }
    line.apply(&patch)?;
    let at = id_of(v, "at").map(|a| a as usize);
    let q = sess.with_quote(id, |q| {
        if q.lines.len() >= limits::LINES {
            return Err(Error::Param(format!("a quote holds at most {} lines", limits::LINES)));
        }
        match at {
            Some(i) if i < q.lines.len() => q.lines.insert(i, line),
            _ => q.lines.push(line),
        }
        q.updated = quotecraft_core::date::now_secs();
        Ok(())
    })?;
    Ok(quote_json(&q, sess.italian()))
}

fn line_index(v: &Value, len: usize) -> Result<usize> {
    let i = id_of(v, "index").ok_or_else(|| Error::Param("index is required (0-based)".into()))? as usize;
    if i >= len {
        return Err(Error::Param(format!("index {i}: the quote has {len} lines")));
    }
    Ok(i)
}

fn quote_set_line(sess: &mut Session, v: &Value) -> Result<Value> {
    let id = target(sess, v)?;
    let mut patch = v.clone();
    if let Some(o) = patch.as_object_mut() {
        o.remove("id");
        o.remove("index");
    }
    let q = sess.with_quote(id, |q| {
        let i = line_index(v, q.lines.len())?;
        let line = q.lines.get_mut(i).ok_or_else(|| Error::Param("no line".into()))?;
        line.apply(&patch)?;
        q.updated = quotecraft_core::date::now_secs();
        Ok(())
    })?;
    Ok(quote_json(&q, sess.italian()))
}

fn quote_remove_line(sess: &mut Session, v: &Value) -> Result<Value> {
    let id = target(sess, v)?;
    let q = sess.with_quote(id, |q| {
        let i = line_index(v, q.lines.len())?;
        q.lines.remove(i);
        q.updated = quotecraft_core::date::now_secs();
        Ok(())
    })?;
    Ok(quote_json(&q, sess.italian()))
}

fn quote_move_line(sess: &mut Session, v: &Value) -> Result<Value> {
    let id = target(sess, v)?;
    let to = id_of(v, "to").ok_or_else(|| Error::Param("to is required".into()))? as usize;
    let q = sess.with_quote(id, |q| {
        let i = line_index(v, q.lines.len())?;
        let line = q.lines.remove(i);
        let to = to.min(q.lines.len());
        q.lines.insert(to, line);
        q.updated = quotecraft_core::date::now_secs();
        Ok(())
    })?;
    Ok(quote_json(&q, sess.italian()))
}

fn quote_html(sess: &mut Session, v: &Value) -> Result<Value> {
    let id = target(sess, v)?;
    let q = lock(sess)?.data.quote(id).cloned().ok_or_else(|| Error::Core(CoreError::NotFound(format!("quote {id}"))))?;
    let company = &sess.config.company;
    Ok(json!({
        "html": quotecraft_core::render::html(&q, company, true),
        "body": quotecraft_core::render::html(&q, company, false),
        "fields": quotecraft_core::render::fields(&q, company),
        "tables": quotecraft_core::render::tables(&q),
    }))
}

/// The render job of a quote: HTML, fields, letterhead, formats.
fn render_job(sess: &Session, q: &Quote, formats: Vec<String>) -> Result<RenderJob> {
    let company = &sess.config.company;
    let template = if q.template.trim().is_empty() {
        None
    } else {
        let dir = sess.templates_dir().ok_or_else(|| Error::Core(CoreError::NotConfigured("templates folder".into())))?;
        Some(template_path(&dir, &q.template)?)
    };
    let mut drawings = Vec::new();
    let mut total = 0usize;
    for d in &q.drawings {
        let data = picture_of(sess, q, &d.file)?;
        total = total.saturating_add(data.len());
        if total > 4 * quotecraft_core::picture::MAX_BYTES {
            return Err(Error::Param(format!("the drawings together are larger than {} MB", 4 * quotecraft_core::picture::MAX_BYTES / 1024 / 1024)));
        }
        drawings.push(quotecraft_core::bridge::Picture { name: d.file.clone(), caption: d.caption.clone(), data });
    }
    Ok(RenderJob {
        body_html: quotecraft_core::render::html(q, company, false),
        page_html: quotecraft_core::render::html(q, company, true),
        fields: quotecraft_core::render::fields(q, company),
        tables: quotecraft_core::render::tables(q),
        sections: quotecraft_core::render::sections(q),
        drawings,
        template,
        out_dir: sess.out_dir(q),
        base: q.file_base(),
        formats,
    })
}

fn formats_of(v: &Value) -> Vec<String> {
    let list: Vec<String> = match v.get("formats") {
        Some(Value::Array(a)) => a.iter().map(text).collect(),
        Some(other) => text(other).split(',').map(|t| t.trim().to_string()).collect(),
        None => vec!["pdf".into(), "docx".into()],
    };
    let mut out: Vec<String> = list.into_iter().map(|f| f.to_ascii_lowercase()).filter(|f| matches!(f.as_str(), "pdf" | "docx" | "html")).collect();
    out.dedup();
    out
}

fn quote_render(sess: &mut Session, v: &Value) -> Result<Value> {
    let id = target(sess, v)?;
    let q = lock(sess)?.data.quote(id).cloned().ok_or_else(|| Error::Core(CoreError::NotFound(format!("quote {id}"))))?;
    let formats = formats_of(v);
    let job = render_job(sess, &q, formats)?;
    let prefs = sess.config.preferences.clone();
    let job_id = sess.submit(&format!("quote.render:{id}"), JobKind::Render { job, prefs })?;
    Ok(json!({"job": job_id}))
}

fn addresses(v: &Value, key: &str, fallback: &str) -> Vec<String> {
    let raw = match v.get(key) {
        Some(Value::Array(a)) => a.iter().map(text).collect::<Vec<_>>().join(","),
        Some(other) => text(other),
        None => fallback.to_string(),
    };
    raw.split([',', ';']).map(|a| a.trim().to_string()).filter(|a| a.contains('@') && a.len() < 200).take(50).collect()
}

fn quote_send(sess: &mut Session, v: &Value) -> Result<Value> {
    let id = target(sess, v)?;
    let q = lock(sess)?.data.quote(id).cloned().ok_or_else(|| Error::Core(CoreError::NotFound(format!("quote {id}"))))?;
    let to = addresses(v, "to", &q.customer.email);
    if to.is_empty() {
        return Err(Error::Param("to: a recipient address is required (the customer has none)".into()));
    }
    let cc = addresses(v, "cc", "");
    let prefs = sess.config.preferences.clone();
    let (subject, body) = quotecraft_core::render::mail_text(&q, &sess.config.company, &prefs.mail_subject, &prefs.mail_body);
    let subject = v.get("subject").map(text).filter(|t| !t.is_empty()).unwrap_or(subject);
    let body = v.get("body").map(text).filter(|t| !t.is_empty()).unwrap_or(body);
    // The documents of the quote: the files named, else those of the formats asked (or
    // the PDF and the DOCX), the attached ones included.
    let files: Vec<String> = match v.get("files") {
        Some(Value::Array(a)) => a.iter().map(text).collect(),
        Some(other) => text(other).split(',').map(|t| t.trim().to_string()).collect(),
        None => Vec::new(),
    };
    let wanted = formats_of(v);
    let dir = sess.out_dir(&q);
    let attachments: Vec<std::path::PathBuf> = q
        .outputs
        .iter()
        .filter(|n| if files.is_empty() { wanted.iter().any(|f| n.to_ascii_lowercase().ends_with(&format!(".{f}"))) } else { files.contains(n) })
        .map(|n| dir.join(n))
        .filter(|p| p.is_file())
        .collect();
    if attachments.is_empty() {
        return Err(Error::Param(if files.is_empty() {
            "no document to attach yet: run quote.render first".into()
        } else {
            format!("no such document on the quote: {}", files.join(", "))
        }));
    }
    let mail = OutgoingMail { to: to.clone(), cc, subject: limits::clip(&subject), body: limits::clip_long(&body), attachments };
    let user = sess.mail_user();
    let secrets = sess.secrets.clone();
    sess.with_quote(id, |q| {
        for t in &to {
            if !q.sent_to.contains(t) {
                q.sent_to.push(t.clone());
            }
        }
        Ok(())
    })?;
    let job_id = sess.submit(&format!("quote.send:{id}"), JobKind::Send { mail, user, prefs, secrets })?;
    Ok(json!({"job": job_id}))
}

/// The kinds of file a quote can carry besides the ones it makes.
const ATTACHMENT_KINDS: &[&str] = &["pdf", "docx", "doc", "odt", "xlsx", "xls", "ods", "pptx", "html", "htm", "txt", "csv", "png", "jpg", "jpeg"];

/// A document's name on a quote: a plain file name with a known extension.
fn document_name(name: &str) -> Result<String> {
    let name = limits::clip_to(name.trim(), 200);
    let kind = name.rsplit('.').next().unwrap_or_default().to_ascii_lowercase();
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || name.starts_with('.')
        || name.chars().any(char::is_control)
        || !ATTACHMENT_KINDS.contains(&kind.as_str())
    {
        return Err(Error::Param(format!("name {name:?}: a plain file name ending in {}", ATTACHMENT_KINDS.join(", "))));
    }
    Ok(name)
}

/// `quote.attach`: a file made elsewhere (a revised PDF, a drawing) joins the quote's
/// documents, so that it is listed, downloaded and mailed with the others.
fn quote_attach(sess: &mut Session, v: &Value) -> Result<Value> {
    let id = target(sess, v)?;
    let q = lock(sess)?.data.quote(id).cloned().ok_or_else(|| Error::Core(CoreError::NotFound(format!("quote {id}"))))?;
    let path = s(v, "path");
    let name = {
        let n = s(v, "name");
        if n.is_empty() { std::path::Path::new(&path).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default() } else { n }
    };
    let name = document_name(&name)?;
    let bytes = match v.get("data").and_then(Value::as_str) {
        Some(b64) => base64_decode(b64).ok_or_else(|| Error::Param("data must be base64".into()))?,
        None if path.is_empty() => return Err(Error::Param("data (base64) or path is required".into())),
        None => std::fs::read(&path).map_err(|e| Error::Core(CoreError::Io(format!("{path}: {e}"))))?,
    };
    if bytes.len() > 50 * 1024 * 1024 {
        return Err(Error::Param("the file is larger than 50 MB".into()));
    }
    if q.outputs.len() >= limits::OUTPUTS && !q.outputs.contains(&name) {
        return Err(Error::Param("too many documents on the quote".into()));
    }
    let dir = sess.out_dir(&q);
    std::fs::create_dir_all(&dir).map_err(|e| Error::Core(CoreError::Io(format!("{}: {e}", dir.display()))))?;
    let dest = dir.join(&name);
    std::fs::write(&dest, &bytes).map_err(|e| Error::Core(CoreError::Io(format!("{}: {e}", dest.display()))))?;
    let q = sess.with_quote(id, |q| {
        if !q.outputs.contains(&name) {
            q.outputs.push(name.clone());
        }
        q.updated = quotecraft_core::date::now_secs();
        Ok(())
    })?;
    sess.state.outputs = outputs_of(sess, &q);
    Ok(json!({"attached": name, "bytes": bytes.len(), "outputs": q.outputs}))
}

/// `quote.detach`: a document leaves the quote (and its folder).
fn quote_detach(sess: &mut Session, v: &Value) -> Result<Value> {
    let id = target(sess, v)?;
    let name = document_name(&s(v, "name"))?;
    let q = lock(sess)?.data.quote(id).cloned().ok_or_else(|| Error::Core(CoreError::NotFound(format!("quote {id}"))))?;
    if !q.outputs.contains(&name) {
        return Err(Error::Core(CoreError::NotFound(format!("document {name} on quote {}", q.number))));
    }
    let path = sess.out_dir(&q).join(&name);
    if path.is_file() {
        std::fs::remove_file(&path).map_err(|e| Error::Core(CoreError::Io(format!("{}: {e}", path.display()))))?;
    }
    let q = sess.with_quote(id, |q| {
        q.outputs.retain(|n| n != &name);
        q.drawings.retain(|d| d.file != name);
        q.updated = quotecraft_core::date::now_secs();
        Ok(())
    })?;
    sess.state.outputs = outputs_of(sess, &q);
    Ok(json!({"detached": name, "outputs": q.outputs}))
}

// ---- Drawings ----

/// Most drawings on one quote.
const DRAWINGS: usize = 20;

/// The bytes of one of the quote's pictures, checked: among its documents, a PNG, JPEG or GIF
/// of at most 15 MB.
fn picture_of(sess: &Session, q: &Quote, name: &str) -> Result<Vec<u8>> {
    if !q.outputs.iter().any(|n| n == name) {
        return Err(Error::Core(CoreError::NotFound(format!("document {name} on quote {}", q.number))));
    }
    let path = sess.out_dir(q).join(name);
    let len = std::fs::metadata(&path).map_err(|e| Error::Core(CoreError::Io(format!("{name}: {e}"))))?.len();
    if len > quotecraft_core::picture::MAX_BYTES as u64 {
        return Err(Error::Param(format!("{name}: a drawing is at most {} MB", quotecraft_core::picture::MAX_BYTES / 1024 / 1024)));
    }
    let data = std::fs::read(&path).map_err(|e| Error::Core(CoreError::Io(format!("{name}: {e}"))))?;
    if quotecraft_core::picture::size(&data).is_none() {
        return Err(Error::Param(format!("{name}: not a PNG, JPEG or GIF picture")));
    }
    Ok(data)
}

fn drawing_index(v: &Value, len: usize) -> Result<usize> {
    let i = id_of(v, "index").ok_or_else(|| Error::Param("index is required (0-based)".into()))? as usize;
    if i >= len {
        return Err(Error::Param(format!("index {i}: the quote has {len} drawings")));
    }
    Ok(i)
}

fn caption_of(v: &Value) -> String {
    limits::clip_to(&s(v, "caption"), 300)
}

/// `quote.addDrawing`: a picture shown in the quote's documents, with its caption. The
/// picture is one of the quote's documents (`file`), or a new one attached on the way
/// (`name` with `data` or `path`, as `quote.attach`).
fn quote_add_drawing(sess: &mut Session, v: &Value) -> Result<Value> {
    let id = target(sess, v)?;
    let mut file = s(v, "file");
    if v.get("data").is_some() || !s(v, "path").is_empty() {
        let name = {
            let n = s(v, "name");
            if n.is_empty() {
                std::path::Path::new(&s(v, "path")).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default()
            } else {
                n
            }
        };
        if !quotecraft_core::picture::is_picture_name(&name) {
            return Err(Error::Param(format!("name {name:?}: a drawing is a .png, .jpg or .gif picture")));
        }
        let attached = quote_attach(sess, v)?;
        file = attached.get("attached").map(text).unwrap_or_default();
        // Not a picture after all: it does not stay among the documents either.
        let q = lock(sess)?.data.quote(id).cloned().ok_or_else(|| Error::Core(CoreError::NotFound(format!("quote {id}"))))?;
        if let Err(e) = picture_of(sess, &q, &file) {
            let _ = quote_detach(sess, &json!({"id": id, "name": file}));
            return Err(e);
        }
    }
    if file.is_empty() {
        return Err(Error::Param("file (a picture among the quote's documents), or name with data or path, is required".into()));
    }
    let q = lock(sess)?.data.quote(id).cloned().ok_or_else(|| Error::Core(CoreError::NotFound(format!("quote {id}"))))?;
    picture_of(sess, &q, &file)?;
    let drawing = Drawing { file, caption: caption_of(v) };
    let at = id_of(v, "at");
    let q = sess.with_quote(id, |q| {
        if q.drawings.len() >= DRAWINGS {
            return Err(Error::Param(format!("a quote holds at most {DRAWINGS} drawings")));
        }
        let at = at.map(|a| (a as usize).min(q.drawings.len())).unwrap_or(q.drawings.len());
        q.drawings.insert(at, drawing);
        q.updated = quotecraft_core::date::now_secs();
        Ok(())
    })?;
    sess.state.outputs = outputs_of(sess, &q);
    Ok(quote_json(&q, sess.italian()))
}

/// `quote.setDrawing`: another caption or picture for a drawing.
fn quote_set_drawing(sess: &mut Session, v: &Value) -> Result<Value> {
    let id = target(sess, v)?;
    let file = s(v, "file");
    if !file.is_empty() {
        let q = lock(sess)?.data.quote(id).cloned().ok_or_else(|| Error::Core(CoreError::NotFound(format!("quote {id}"))))?;
        picture_of(sess, &q, &file)?;
    }
    let caption = v.get("caption").map(|_| caption_of(v));
    let q = sess.with_quote(id, |q| {
        let i = drawing_index(v, q.drawings.len())?;
        let d = q.drawings.get_mut(i).ok_or_else(|| Error::Param("no drawing".into()))?;
        if !file.is_empty() {
            d.file = file.clone();
        }
        if let Some(c) = &caption {
            d.caption = c.clone();
        }
        q.updated = quotecraft_core::date::now_secs();
        Ok(())
    })?;
    Ok(quote_json(&q, sess.italian()))
}

/// `quote.removeDrawing`: the drawing leaves the documents; its picture stays among the
/// quote's documents.
fn quote_remove_drawing(sess: &mut Session, v: &Value) -> Result<Value> {
    let id = target(sess, v)?;
    let q = sess.with_quote(id, |q| {
        let i = drawing_index(v, q.drawings.len())?;
        q.drawings.remove(i);
        q.updated = quotecraft_core::date::now_secs();
        Ok(())
    })?;
    Ok(quote_json(&q, sess.italian()))
}

fn quote_move_drawing(sess: &mut Session, v: &Value) -> Result<Value> {
    let id = target(sess, v)?;
    let to = id_of(v, "to").ok_or_else(|| Error::Param("to is required".into()))? as usize;
    let q = sess.with_quote(id, |q| {
        let i = drawing_index(v, q.drawings.len())?;
        let d = q.drawings.remove(i);
        let to = to.min(q.drawings.len());
        q.drawings.insert(to, d);
        q.updated = quotecraft_core::date::now_secs();
        Ok(())
    })?;
    Ok(quote_json(&q, sess.italian()))
}

// ---- Letterheads ----

fn templates_list(sess: &mut Session, _: &Value) -> Result<Value> {
    sess.state.templates = sess.list_templates();
    sess.touch();
    let dir = sess.templates_dir().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(json!({
        "templates": sess.state.templates, "dir": dir,
        "placeholders": doc_list(quotecraft_core::render::PLACEHOLDERS),
        "tables": doc_list(quotecraft_core::render::TABLES),
        "sections": doc_list(quotecraft_core::render::SECTIONS),
        "line_fields": doc_list(quotecraft_core::render::LINE_FIELDS),
        "drawings": doc_list(quotecraft_core::render::DRAWINGS),
    }))
}

fn doc_list(list: &[(&str, &str)]) -> Vec<Value> {
    list.iter().map(|(p, d)| json!({"placeholder": p, "meaning": d})).collect()
}

fn templates_import(sess: &mut Session, v: &Value) -> Result<Value> {
    sess.require_admin("change the letterheads")?;
    let path = s(v, "path");
    let dir = sess.templates_dir().ok_or_else(|| Error::Core(CoreError::NotConfigured("templates folder".into())))?;
    let name = {
        let n = s(v, "name");
        if n.is_empty() { std::path::Path::new(&path).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default() } else { n }
    };
    let kind = name.rsplit('.').next().unwrap_or_default().to_ascii_lowercase();
    if name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || name.starts_with('.')
        || !(kind == "docx" || kind == "html" || kind == "htm")
    {
        return Err(Error::Param(format!("name {name:?}: a plain file name ending in .docx or .html")));
    }
    let bytes = match v.get("data").and_then(Value::as_str) {
        Some(b64) => base64_decode(b64).ok_or_else(|| Error::Param("data must be base64".into()))?,
        None => std::fs::read(&path).map_err(|e| Error::Core(CoreError::Io(format!("{path}: {e}"))))?,
    };
    if bytes.len() > 50 * 1024 * 1024 {
        return Err(Error::Param("the letterhead is larger than 50 MB".into()));
    }
    std::fs::create_dir_all(&dir).map_err(|e| Error::Core(CoreError::Io(format!("{}: {e}", dir.display()))))?;
    let dest = dir.join(&name);
    std::fs::write(&dest, bytes).map_err(|e| Error::Core(CoreError::Io(format!("{}: {e}", dest.display()))))?;
    sess.refresh_lists();
    Ok(
        json!({"template": TemplateInfo { name, kind: if kind == "htm" { "html".into() } else { kind }, bytes: std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0) }}),
    )
}

/// A small base64 decoder (standard alphabet, padding optional): no dependency for it.
fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b'\n' | b'\r' | b' ' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xff) as u8);
        }
    }
    Some(out)
}

fn templates_remove(sess: &mut Session, v: &Value) -> Result<Value> {
    sess.require_admin("change the letterheads")?;
    let name = s(v, "name");
    let dir = sess.templates_dir().ok_or_else(|| Error::Core(CoreError::NotConfigured("templates folder".into())))?;
    let p = template_path(&dir, &name)?;
    std::fs::remove_file(&p).map_err(|e| Error::Core(CoreError::Io(format!("{}: {e}", p.display()))))?;
    sess.refresh_lists();
    Ok(json!({"removed": name}))
}

// ---- Settings ----

fn company_get(sess: &mut Session, _: &Value) -> Result<Value> {
    Ok(json!({"company": sess.config.company}))
}

fn company_set(sess: &mut Session, v: &Value) -> Result<Value> {
    sess.require_admin("change the company data")?;
    sess.config.company.apply(v);
    sess.save_config()?;
    sess.touch();
    Ok(json!({"company": sess.config.company}))
}

fn prefs_get(sess: &mut Session, _: &Value) -> Result<Value> {
    Ok(json!({"preferences": sess.config.preferences, "has_mailcraft_token": !sess.secrets.mailcraft_token.is_empty()}))
}

fn prefs_set(sess: &mut Session, v: &Value) -> Result<Value> {
    // Language and theme are anyone's; the rest is the administrator's.
    let personal = v.as_object().is_some_and(|o| o.keys().all(|k| k == "language" || k == "theme"));
    if !personal {
        sess.require_admin("change the settings")?;
    }
    sess.config.preferences.apply(v)?;
    if let Some(t) = v.get("mailcraft_token") {
        sess.secrets.mailcraft_token = limits::clip(&text(t));
    }
    sess.save_config()?;
    sess.touch();
    Ok(json!({"preferences": sess.config.preferences, "has_mailcraft_token": !sess.secrets.mailcraft_token.is_empty()}))
}

fn bridge_check(sess: &mut Session, _: &Value) -> Result<Value> {
    let prefs = sess.config.preferences.clone();
    let secrets = sess.secrets.clone();
    Ok(json!({"job": sess.submit("bridge.check", JobKind::Probe { prefs, secrets })?}))
}

fn session_user(sess: &mut Session, v: &Value) -> Result<Value> {
    let admin = if v.get("admin").is_some() { flag(v, "admin") } else { sess.admin };
    sess.set_user(&s(v, "user"), admin);
    sess.touch();
    Ok(json!({"user": sess.user, "admin": sess.admin}))
}

fn state_get(sess: &mut Session, _: &Value) -> Result<Value> {
    let st = &sess.state;
    Ok(json!({
        "items": st.items.len(), "customers": st.customers.len(), "quotes": st.quotes.len(),
        "quote": st.quote.as_ref().map(|q| q.number.clone()), "templates": st.templates.len(),
        "jobs_pending": sess.jobs_pending(), "user": sess.user, "admin": sess.admin, "last_error": st.last_error,
    }))
}

const fn c(
    id: &'static str,
    label: &'static str,
    params: &'static str,
    admin: bool,
    local: bool,
    run: fn(&mut Session, &Value) -> Result<Value>,
) -> Command {
    Command { id, label, params, admin, local, run }
}

pub const COMMANDS: &[Command] = &[
    c("items.list", "Price list", "query, category (connectivity|hardware|services), all (inactive too), limit", false, true, items_list),
    c("items.get", "An item", "code", false, true, items_get),
    c(
        "items.save",
        "Add or change an item",
        "code, name, description, category, unit, one_off_direct, one_off_wholesale, monthly_direct, monthly_wholesale, vat_pct, active, tags",
        true,
        true,
        items_save,
    ),
    c("items.remove", "Remove an item", "code", true, true, items_remove),
    c("items.import", "Import items", "rows [{code, name, …}], replace", true, true, items_import),
    c("customers.list", "Customers", "query, limit", false, true, customers_list),
    c(
        "customers.save",
        "Add or change a customer",
        "id (none: new), name, address, cap, city, province, vat, fiscal_code, email, pec, sdi, contact, phone, notes",
        false,
        true,
        customers_save,
    ),
    c("customers.remove", "Remove a customer", "id", false, true, customers_remove),
    c("quotes.list", "Quotes", "query, status, channel, author, mine, limit", false, true, quotes_list),
    c("quote.new", "New quote", "channel (direct|wholesale), customer_id | customer {…}, title, notes, template", false, true, quote_new),
    c("quote.open", "Open a quote", "id", false, true, quote_open),
    c("quote.get", "A quote with its totals", "id (default: the open one)", false, true, quote_get),
    c(
        "quote.set",
        "Change a quote",
        "id, title, notes, date, channel, status, validity_days, duration_months, template, customer_id | customer {…}",
        false,
        true,
        quote_set,
    ),
    c("quote.delete", "Delete a quote", "id", false, true, quote_delete),
    c("quote.duplicate", "Duplicate a quote", "id", false, true, quote_duplicate),
    c(
        "quote.addLine",
        "Add a line",
        "id, code (from the price list) and/or name, description, category, unit, qty, one_off, monthly, discount_pct, vat_pct, at",
        false,
        true,
        quote_add_line,
    ),
    c("quote.setLine", "Change a line", "id, index, qty, one_off, monthly, discount_pct, vat_pct, name, description…", false, true, quote_set_line),
    c("quote.removeLine", "Remove a line", "id, index", false, true, quote_remove_line),
    c("quote.moveLine", "Move a line", "id, index, to", false, true, quote_move_line),
    c("quote.html", "The quote as HTML and fields", "id", false, true, quote_html),
    c("quote.render", "Make the documents (HTML, and PDF/DOCX through WordCraft)", "id, formats [pdf, docx, html]", false, false, quote_render),
    c(
        "quote.send",
        "Send the documents by mail (through MailCraft, as the user)",
        "id, to, cc, subject, body, formats | files",
        false,
        false,
        quote_send,
    ),
    c("quote.attach", "Add a document made elsewhere to the quote", "id, name, data (base64) | path", false, true, quote_attach),
    c("quote.detach", "Remove a document from the quote", "id, name", false, true, quote_detach),
    c(
        "quote.addDrawing",
        "Show a drawing in the quote's documents",
        "id, file (a picture among its documents) | name + data (base64) | path, caption, at",
        false,
        true,
        quote_add_drawing,
    ),
    c("quote.setDrawing", "Change a drawing", "id, index, caption, file", false, true, quote_set_drawing),
    c("quote.removeDrawing", "Remove a drawing from the documents", "id, index", false, true, quote_remove_drawing),
    c("quote.moveDrawing", "Move a drawing", "id, index, to", false, true, quote_move_drawing),
    c("templates.list", "Letterheads", "", false, true, templates_list),
    c("templates.import", "Add a letterhead", "path | data (base64), name", true, true, templates_import),
    c("templates.remove", "Remove a letterhead", "name", true, true, templates_remove),
    c("company.get", "Company data", "", false, true, company_get),
    c(
        "company.set",
        "Change the company data",
        "name, address, cap, city, province, vat, fiscal_code, email, pec, phone, web, iban, footer",
        true,
        true,
        company_set,
    ),
    c("prefs.get", "Preferences", "", false, true, prefs_get),
    c(
        "prefs.set",
        "Change preferences",
        "language, theme (anyone); default_vat_pct, default_validity_days, default_duration_months, template_direct, template_wholesale, templates_dir, wordcraft_cli, mailcraft_url, mailcraft_token, mail_subject, mail_body (administrator)",
        false,
        true,
        prefs_set,
    ),
    c("bridge.check", "Check WordCraft and MailCraft", "", false, false, bridge_check),
    c("session.user", "Who is working", "user, admin", false, true, session_user),
    c("state.get", "Session state", "", false, true, state_get),
];

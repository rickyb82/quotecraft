//! The letterhead's own tables, filled: in a `.docx` letterhead a table row that carries line
//! fields (`{{riga.nome}}`, `{{riga.totale_canone}}`…) is written once per line, with its own
//! style (shading, borders, fonts, column widths). Which lines: the section the row opens
//! (`{{#righe.una_tantum}}` … `{{/righe.una_tantum}}`, spanning one row or several), or every
//! line when the row only has fields. The markers disappear; a section with no lines takes
//! its rows away. This runs on the letterhead before WordCraft fills the other placeholders.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use quotecraft_core::{Error, Result};

/// The lines of each section (`righe`, `righe.canone`…), each a map of `riga.*` fields.
pub type Sections = BTreeMap<String, Vec<BTreeMap<String, String>>>;

/// Largest part read from a letterhead (uncompressed).
const MAX_PART: u64 = 64 * 1024 * 1024;
/// Most entries in a letterhead's archive.
const MAX_ENTRIES: usize = 5_000;
/// Most rows a section may write (lines × rows of the block).
const MAX_ROWS: usize = 5_000;

fn bad(e: impl std::fmt::Display) -> Error {
    Error::Invalid(format!("letterhead: {e}"))
}

/// The parts whose tables are filled: the body, the headers and the footers.
fn is_text_part(name: &str) -> bool {
    name == "word/document.xml" || ((name.starts_with("word/header") || name.starts_with("word/footer")) && name.ends_with(".xml"))
}

/// The letterhead with its line rows written out, or `None` when it has none (the file is
/// then used as it is).
pub fn expand_docx(bytes: &[u8], sections: &Sections) -> Result<Option<Vec<u8>>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(bad)?;
    if archive.len() > MAX_ENTRIES {
        return Err(bad("too many parts"));
    }
    let mut parts: Vec<(String, Option<String>)> = Vec::new();
    let mut changed = false;
    for i in 0..archive.len() {
        let mut f = archive.by_index(i).map_err(bad)?;
        let name = f.name().to_string();
        if !is_text_part(&name) {
            parts.push((name, None));
            continue;
        }
        if f.size() > MAX_PART {
            return Err(bad(format!("{name} too large")));
        }
        let mut xml = String::new();
        f.by_ref().take(MAX_PART).read_to_string(&mut xml).map_err(bad)?;
        let (out, did) = expand_xml(&xml, sections);
        changed |= did;
        parts.push((name, Some(out)));
    }
    if !changed {
        return Ok(None);
    }
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (i, (name, text)) in parts.into_iter().enumerate() {
        let f = archive.by_index_raw(i).map_err(bad)?;
        match text {
            None => writer.raw_copy_file(f).map_err(bad)?,
            Some(xml) => {
                let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
                drop(f);
                writer.start_file(name, opts).map_err(bad)?;
                writer.write_all(xml.as_bytes()).map_err(bad)?;
            }
        }
    }
    let out = writer.finish().map_err(bad)?;
    Ok(Some(out.into_inner()))
}

/// Start of the next `<w:tr>` element at or after `from` (not `<w:trPr>`).
fn next_row(xml: &str, from: usize) -> Option<usize> {
    let mut at = from;
    loop {
        let i = at + xml.get(at..)?.find("<w:tr")?;
        match xml.as_bytes().get(i + 5) {
            Some(b'>' | b' ' | b'/') => return Some(i),
            _ => at = i + 5,
        }
    }
}

/// End (just past `</w:tr>`) of the row starting at `start`, nested tables included.
fn row_end(xml: &str, start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut at = start;
    loop {
        let open = next_row(xml, at);
        let close = at + xml.get(at..)?.find("</w:tr>")?;
        match open {
            Some(o) if o < close => {
                depth += 1;
                at = o + 5;
            }
            _ => {
                depth = depth.saturating_sub(1);
                at = close + 7;
                if depth == 0 {
                    return Some(at);
                }
            }
        }
    }
}

/// The `(content start, content end)` of every `<w:t>` text in `xml`.
fn text_spans(xml: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(i) = xml.get(at..).and_then(|s| s.find("<w:t")).map(|i| at + i) {
        at = i + 4;
        if !matches!(xml.as_bytes().get(i + 4), Some(b'>' | b' ')) {
            continue;
        }
        let Some(gt) = xml.get(i..).and_then(|s| s.find('>')).map(|g| i + g) else { break };
        if xml.as_bytes().get(gt.saturating_sub(1)) == Some(&b'/') {
            at = gt + 1;
            continue;
        }
        let Some(end) = xml.get(gt..).and_then(|s| s.find("</w:t>")).map(|e| gt + e) else { break };
        out.push((gt + 1, end));
        at = end + 6;
    }
    out
}

fn visible_text(xml: &str) -> String {
    text_spans(xml).into_iter().filter_map(|(a, b)| xml.get(a..b)).collect()
}

/// Word splits text edited in pieces into several runs: a placeholder typed across runs is
/// moved whole into the first run it starts in, so it can be found.
fn join_split_placeholders(xml: &str) -> String {
    let spans = text_spans(xml);
    if spans.len() < 2 {
        return xml.to_string();
    }
    // Paragraph of each text, so a placeholder never joins two paragraphs.
    let para_of = |pos: usize| xml.get(..pos).map(|s| s.matches("</w:p>").count()).unwrap_or(0);
    let texts: Vec<&str> = spans.iter().map(|(a, b)| xml.get(*a..*b).unwrap_or("")).collect();
    let paras: Vec<usize> = spans.iter().map(|(a, _)| para_of(*a)).collect();
    let concat: String = texts.concat();
    let mut owner: Vec<usize> = Vec::with_capacity(concat.len());
    for (k, t) in texts.iter().enumerate() {
        owner.extend(std::iter::repeat_n(k, t.len()));
    }
    let mut moved = false;
    let mut at = 0;
    while let Some(s) = concat.get(at..).and_then(|c| c.find("{{")).map(|i| at + i) {
        let Some(e) = concat.get(s..).and_then(|c| c.find("}}")).map(|i| s + i + 2) else { break };
        let (first, last) = (owner.get(s).copied().unwrap_or(0), owner.get(e - 1).copied().unwrap_or(0));
        if first != last && paras.get(first) == paras.get(last) && !concat.get(s + 2..e - 2).unwrap_or("").contains('{') {
            for o in owner.iter_mut().take(e).skip(s) {
                *o = first;
            }
            moved = true;
        }
        at = e;
    }
    if !moved {
        return xml.to_string();
    }
    let mut new_texts: Vec<Vec<u8>> = vec![Vec::new(); texts.len()];
    for (i, b) in concat.bytes().enumerate() {
        if let Some(v) = owner.get(i).and_then(|o| new_texts.get_mut(*o)) {
            v.push(b);
        }
    }
    let mut out = String::with_capacity(xml.len());
    let mut last = 0;
    for ((a, b), t) in spans.iter().zip(new_texts) {
        out.push_str(xml.get(last..*a).unwrap_or(""));
        out.push_str(&String::from_utf8_lossy(&t));
        last = *b;
    }
    out.push_str(xml.get(last..).unwrap_or(""));
    out
}

fn escape(v: &str) -> String {
    v.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace(['\r', '\n'], " ")
}

/// The section a row opens (`{{#righe.canone}}` → `righe.canone`), if any.
fn opened(text: &str) -> Option<String> {
    let i = text.find("{{#righe")?;
    let rest = text.get(i + 3..)?;
    let end = rest.find("}}")?;
    Some(rest.get(..end)?.to_string())
}

/// Drops the copies' paragraph ids and bookmarks, which must stay unique in a document.
fn without_unique_ids(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut at = 0;
    while at < xml.len() {
        let rest = xml.get(at..).unwrap_or("");
        let next =
            ["<w:bookmarkStart", "<w:bookmarkEnd", " w14:paraId=\"", " w14:textId=\""].iter().filter_map(|p| rest.find(p).map(|i| (i, *p))).min();
        let Some((i, pat)) = next else {
            out.push_str(rest);
            break;
        };
        out.push_str(rest.get(..i).unwrap_or(""));
        let tail = rest.get(i..).unwrap_or("");
        let skip = if pat.starts_with("<w:bookmark") {
            tail.find("/>").map(|e| e + 2).unwrap_or(tail.len())
        } else {
            tail.get(pat.len()..).and_then(|t| t.find('"')).map(|e| pat.len() + e + 1).unwrap_or(tail.len())
        };
        at += i + skip;
    }
    out
}

/// One block of rows written for every line of the section.
fn write_block(block: &str, key: &str, lines: &[BTreeMap<String, String>]) -> String {
    let clean = join_split_placeholders(block).replace(&format!("{{{{#{key}}}}}"), "").replace(&format!("{{{{/{key}}}}}"), "");
    let rows_per_line = clean.matches("</w:tr>").count().max(1);
    let fit = (MAX_ROWS / rows_per_line).max(1);
    let mut out = String::new();
    for (i, line) in lines.iter().take(fit).enumerate() {
        let mut copy = if i == 0 { clean.clone() } else { without_unique_ids(&clean) };
        let mut at = 0;
        while let Some(s) = copy.get(at..).and_then(|c| c.find("{{riga.")).map(|x| at + x) {
            let Some(e) = copy.get(s..).and_then(|c| c.find("}}")).map(|x| s + x + 2) else { break };
            let field = copy.get(s + 2..e - 2).unwrap_or("").to_string();
            let value = escape(line.get(&field).map(String::as_str).unwrap_or(""));
            copy.replace_range(s..e, &value);
            at = s + value.len();
        }
        out.push_str(&copy);
    }
    out
}

/// The XML of a part with its line rows written out, and whether anything changed.
pub fn expand_xml(xml: &str, sections: &Sections) -> (String, bool) {
    let mut out = String::with_capacity(xml.len());
    let mut changed = false;
    let mut at = 0;
    while let Some(start) = next_row(xml, at) {
        let Some(end) = row_end(xml, start) else { break };
        let row = xml.get(start..end).unwrap_or("");
        let text = visible_text(row);
        let key = match opened(&text) {
            Some(k) => Some(k),
            None if text.contains("{{riga.") => Some("righe".to_string()),
            None => None,
        };
        let Some(key) = key.filter(|k| sections.contains_key(k)) else {
            out.push_str(xml.get(at..end).unwrap_or(""));
            at = end;
            continue;
        };
        // The block: this row, up to the row that closes the section in the same table.
        let close = format!("{{{{/{key}}}}}");
        let mut block_end = end;
        if text.contains(&format!("{{{{#{key}}}}}")) && !text.contains(&close) {
            let mut probe = end;
            while let Some(s) = next_row(xml, probe) {
                if xml.get(probe..s).is_none_or(|gap| gap.contains("</w:tbl>")) {
                    break;
                }
                let Some(e) = row_end(xml, s) else { break };
                if visible_text(xml.get(s..e).unwrap_or("")).contains(&close) {
                    block_end = e;
                    break;
                }
                probe = e;
            }
        }
        out.push_str(xml.get(at..start).unwrap_or(""));
        let lines = sections.get(&key).map(Vec::as_slice).unwrap_or(&[]);
        out.push_str(&write_block(xml.get(start..block_end).unwrap_or(""), &key, lines));
        changed = true;
        at = block_end;
    }
    out.push_str(xml.get(at..).unwrap_or(""));
    (out, changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tr(cells: &[&str]) -> String {
        let mut s = String::from("<w:tr w14:paraId=\"1A2B3C4D\"><w:trPr><w:cantSplit/></w:trPr>");
        for c in cells {
            s.push_str(&format!("<w:tc><w:tcPr><w:shd w:fill=\"222222\"/></w:tcPr><w:p><w:r><w:rPr><w:b/></w:rPr><w:t xml:space=\"preserve\">{c}</w:t></w:r></w:p></w:tc>"));
        }
        s.push_str("</w:tr>");
        s
    }

    fn line(name: &str, qty: &str, total: &str) -> BTreeMap<String, String> {
        BTreeMap::from([
            ("riga.nome".to_string(), name.to_string()),
            ("riga.qta".to_string(), qty.to_string()),
            ("riga.totale_una_tantum".to_string(), total.to_string()),
            ("riga.totale_canone".to_string(), "39,90 €".to_string()),
        ])
    }

    fn sections() -> Sections {
        BTreeMap::from([
            ("righe".to_string(), vec![line("FTTH 1 Gbps", "1", "99,00 €"), line("Router & AP", "2", "")]),
            ("righe.una_tantum".to_string(), vec![line("Router <Wi-Fi 6>", "1", "149,00 €"), line("Installazione", "1", "120,00 €")]),
            ("righe.canone".to_string(), vec![]),
        ])
    }

    #[test]
    fn a_marked_row_is_written_once_per_line_with_its_style() {
        let xml = format!(
            "<w:body><w:tbl>{}{}{}</w:tbl></w:body>",
            tr(&["Quantità", "Descrizione", "Totale"]),
            tr(&["{{#righe.una_tantum}}Nr. {{riga.qta}}", "{{riga.nome}}", "{{riga.totale_una_tantum}}{{/righe.una_tantum}}"]),
            tr(&["", "Totale", "{{totale.una_tantum}}"])
        );
        let (out, changed) = expand_xml(&xml, &sections());
        assert!(changed);
        assert_eq!(out.matches("</w:tr>").count(), 4, "header, two lines, total");
        assert!(out.contains(">Nr. 1<") && out.contains(">Router &lt;Wi-Fi 6&gt;<") && out.contains(">120,00 €<"));
        assert!(!out.contains("{{#righe") && !out.contains("{{/righe") && !out.contains("{{riga."));
        assert_eq!(out.matches("w:fill=\"222222\"").count(), 12, "the style comes with every copy");
        assert_eq!(out.matches("w14:paraId").count(), 3, "copies drop the unique ids");
        assert!(out.contains("{{totale.una_tantum}}"), "the other placeholders stay for WordCraft");
    }

    #[test]
    fn rows_without_a_marker_list_every_line_and_an_empty_section_drops_its_rows() {
        let xml = format!(
            "<w:tbl>{}</w:tbl><w:tbl>{}</w:tbl>",
            tr(&["{{riga.nome}}", "{{riga.qta}}"]),
            tr(&["{{#righe.canone}}{{riga.nome}}{{/righe.canone}}"])
        );
        let (out, _) = expand_xml(&xml, &sections());
        assert!(out.contains(">FTTH 1 Gbps<") && out.contains(">Router &amp; AP<"));
        assert_eq!(out.matches("</w:tr>").count(), 2, "two lines; no monthly lines, no row");
        // An unknown section is left as it is, so a typo shows.
        let (same, changed) = expand_xml(&tr(&["{{#righe.zzz}}x"]), &sections());
        assert!(!changed && same.contains("{{#righe.zzz}}"));
    }

    #[test]
    fn a_section_spans_several_rows_and_stops_at_its_table() {
        let xml = format!(
            "<w:tbl>{}{}{}</w:tbl>",
            tr(&["{{#righe.una_tantum}}{{riga.nome}}"]),
            tr(&["{{riga.totale_una_tantum}}{{/righe.una_tantum}}"]),
            tr(&["fine"])
        );
        let (out, _) = expand_xml(&xml, &sections());
        assert_eq!(out.matches("</w:tr>").count(), 5, "two rows per line, then the last one");
        let open_only = format!("<w:tbl>{}</w:tbl><w:tbl>{}</w:tbl>", tr(&["{{#righe.una_tantum}}{{riga.nome}}"]), tr(&["{{/righe.una_tantum}}"]));
        let (out, _) = expand_xml(&open_only, &sections());
        assert!(out.contains(">Installazione<") && out.contains("{{/righe.una_tantum}}"), "never across tables");
    }

    #[test]
    fn a_placeholder_split_across_runs_is_joined() {
        let row = "<w:tr><w:tc><w:p><w:r><w:t>Nr. {{riga.</w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>qta}} pz</w:t></w:r></w:p></w:tc></w:tr>";
        let (out, changed) = expand_xml(&format!("<w:tbl>{row}</w:tbl>"), &sections());
        assert!(changed);
        assert!(out.contains("<w:t>Nr. 1</w:t>") && out.contains("<w:t> pz</w:t>"), "{out}");
        assert!(out.contains("<w:t>Nr. 2</w:t>"));
        // Not across paragraphs, and nested tables keep their rows.
        let two = "<w:p><w:r><w:t>{{</w:t></w:r></w:p><w:p><w:r><w:t>x}}</w:t></w:r></w:p>";
        assert_eq!(join_split_placeholders(two), two);
        let nested = format!("<w:tbl><w:tr><w:tc><w:tbl>{}</w:tbl></w:tc></w:tr></w:tbl>", tr(&["a"]));
        assert_eq!(expand_xml(&nested, &sections()), (nested.clone(), false));
    }

    #[test]
    fn a_docx_is_rewritten_only_when_it_has_line_rows() {
        let make = |doc: &str| {
            let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
            let o = zip::write::SimpleFileOptions::default();
            w.start_file("[Content_Types].xml", o).unwrap();
            w.write_all(b"<Types/>").unwrap();
            w.start_file("word/document.xml", o).unwrap();
            w.write_all(doc.as_bytes()).unwrap();
            w.start_file("word/media/logo.png", o).unwrap();
            w.write_all(&[0x89, b'P', b'N', b'G']).unwrap();
            w.finish().unwrap().into_inner()
        };
        let plain = make("<w:body><w:p><w:r><w:t>{{numero}}</w:t></w:r></w:p></w:body>");
        assert_eq!(expand_docx(&plain, &sections()).unwrap(), None);
        let with_rows = make(&format!("<w:body><w:tbl>{}</w:tbl></w:body>", tr(&["{{riga.nome}}"])));
        let out = expand_docx(&with_rows, &sections()).unwrap().unwrap();
        let mut a = zip::ZipArchive::new(Cursor::new(out)).unwrap();
        assert_eq!(a.len(), 3);
        let mut doc = String::new();
        a.by_name("word/document.xml").unwrap().read_to_string(&mut doc).unwrap();
        assert!(doc.contains("FTTH 1 Gbps") && doc.contains("Router &amp; AP"));
        let mut png = Vec::new();
        a.by_name("word/media/logo.png").unwrap().read_to_end(&mut png).unwrap();
        assert_eq!(png, [0x89, b'P', b'N', b'G']);
        assert!(expand_docx(b"not a zip", &sections()).is_err());
    }
}

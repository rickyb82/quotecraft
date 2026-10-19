//! The drawings of a quote in a `.docx` letterhead: the paragraph that holds `{{schema.N}}`
//! becomes drawing N, the one that holds `{{schemi}}` the drawings not placed one by one, each
//! a picture fitted to the page (or to its table cell) with its caption. The drawings the
//! letterhead does not place follow `{{corpo}}`, or end the document. Pictures are written
//! here, into the letterhead's archive, because a picture inside inserted HTML does not reach
//! WordCraft's document.

use quotecraft_core::bridge::Picture;
use quotecraft_core::picture;

/// EMU (the unit of a drawing's size) per point.
const EMU_PER_PT: f64 = 12_700.0;
/// The relationship ids of the pictures (unlikely in a letterhead of its own).
const RID: &str = "rIdQuotecraftDrawing";

/// A picture to add to the archive: its part name and bytes.
pub struct Media {
    pub part: String,
    pub data: Vec<u8>,
    pub ext: &'static str,
}

/// What placing the drawings changed in `document.xml`, and what the archive needs.
pub struct Placed {
    pub xml: String,
    /// The pictures' parts, with relationship `{RID}{n}` for media `n` (1-based).
    pub media: Vec<Media>,
}

/// Start of the next `<w:p>` element at or after `from` (not `<w:pPr>`, `<w:pStyle>`…).
fn next_para(xml: &str, from: usize) -> Option<usize> {
    let mut at = from;
    loop {
        let i = at + xml.get(at..)?.find("<w:p")?;
        match xml.as_bytes().get(i + 4) {
            Some(b'>' | b' ' | b'/') => return Some(i),
            _ => at = i + 4,
        }
    }
}

/// End (just past `</w:p>`, or `/>`) of the paragraph starting at `start`, the paragraphs of
/// text boxes inside it included.
fn para_end(xml: &str, start: usize) -> Option<usize> {
    let gt = start + xml.get(start..)?.find('>')?;
    if xml.as_bytes().get(gt.checked_sub(1)?) == Some(&b'/') {
        return Some(gt + 1);
    }
    let mut depth = 1usize;
    let mut at = gt + 1;
    for _ in 0..100_000 {
        let close = at + xml.get(at..)?.find("</w:p>")?;
        match next_para(xml, at) {
            Some(o) if o < close => {
                let ogt = o + xml.get(o..)?.find('>')?;
                if xml.as_bytes().get(ogt.checked_sub(1)?) != Some(&b'/') {
                    depth += 1;
                }
                at = ogt + 1;
            }
            _ => {
                depth = depth.saturating_sub(1);
                at = close + 6;
                if depth == 0 {
                    return Some(at);
                }
            }
        }
    }
    None
}

fn visible(xml: &str) -> String {
    let mut out = String::new();
    let mut at = 0;
    while let Some(i) = xml.get(at..).and_then(|s| s.find("<w:t")).map(|i| at + i) {
        at = i + 4;
        if !matches!(xml.as_bytes().get(i + 4), Some(b'>' | b' ')) {
            continue;
        }
        let Some(gt) = xml.get(i..).and_then(|s| s.find('>')).map(|g| i + g) else { break };
        if xml.as_bytes().get(gt.saturating_sub(1)) == Some(&b'/') {
            continue;
        }
        let Some(end) = xml.get(gt..).and_then(|s| s.find("</w:t>")).map(|e| gt + e) else { break };
        out.push_str(xml.get(gt + 1..end).unwrap_or(""));
        at = end + 6;
    }
    out
}

/// What a paragraph's text asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ask {
    /// `{{schema.N}}` (1-based).
    One(usize),
    /// `{{schemi}}`.
    Rest,
}

fn ask_of(text: &str) -> Option<Ask> {
    if text.contains("{{schemi}}") {
        return Some(Ask::Rest);
    }
    let i = text.find("{{schema.")?;
    let rest = text.get(i + 9..)?;
    let n = rest.get(..rest.find("}}")?)?.trim().parse::<usize>().ok()?;
    Some(Ask::One(n))
}

/// The value of attribute `name` (`w:w`) in the first `<tag …>` of `xml`, as a number.
fn attr_num(xml: &str, tag: &str, name: &str) -> Option<f64> {
    let i = xml.find(tag)?;
    let end = i + xml.get(i..)?.find('>')?;
    let el = xml.get(i..end)?;
    let j = el.find(&format!(" {name}=\""))? + name.len() + 3;
    let v = el.get(j..)?;
    v.get(..v.find('"')?)?.trim().parse::<f64>().ok().filter(|n| n.is_finite())
}

/// The page's text area (points), from the document's last section: width and height.
fn text_area(xml: &str) -> (f64, f64) {
    let sect = xml.rfind("<w:sectPr").and_then(|i| xml.get(i..)).unwrap_or("");
    let w = attr_num(sect, "<w:pgSz", "w:w").unwrap_or(11_906.0);
    let h = attr_num(sect, "<w:pgSz", "w:h").unwrap_or(16_838.0);
    let m = |k: &str| attr_num(sect, "<w:pgMar", k).unwrap_or(1_134.0).max(0.0);
    let width = ((w - m("w:left") - m("w:right")) / 20.0).clamp(72.0, picture::MAX_WIDTH_PT.max(1_000.0));
    // Room for the caption below.
    let height = ((h - m("w:top") - m("w:bottom")) / 20.0 - 48.0).clamp(72.0, 1_500.0);
    (width, height)
}

/// The width of the table cell holding position `at` (points, its margins taken off), if any.
fn cell_width(xml: &str, at: usize) -> Option<f64> {
    let before = xml.get(..at)?;
    let open = before.rfind("<w:tc>").into_iter().chain(before.rfind("<w:tc ")).max()?;
    if before.rfind("</w:tc>").is_some_and(|c| c > open) {
        return None;
    }
    let cell = before.get(open..)?;
    let w = attr_num(cell, "<w:tcW", "w:w")?;
    let dxa = cell.find("<w:tcW").and_then(|i| cell.get(i..)).and_then(|c| c.get(..c.find('>')?)).is_some_and(|t| t.contains("w:type=\"dxa\""));
    (dxa && w > 0.0).then(|| (w / 20.0 - 11.0).max(36.0))
}

fn escape(v: &str) -> String {
    v.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace(['\r', '\n'], " ")
}

/// The paragraph properties of a paragraph (`<w:pPr>…</w:pPr>`), without a section break,
/// centred unless it says otherwise, keeping with the next one (the caption).
fn figure_ppr(para: &str) -> String {
    let ppr = para
        .find("<w:pPr>")
        .and_then(|i| Some((i, i + para.get(i..)?.find("</w:pPr>")? + 8)))
        .and_then(|(a, b)| para.get(a..b))
        .unwrap_or("<w:pPr></w:pPr>");
    let mut inner = ppr.get(7..ppr.len().saturating_sub(8)).unwrap_or("").to_string();
    if let Some(i) = inner.find("<w:sectPr") {
        let end = inner.find("</w:sectPr>").map(|e| e + 11).unwrap_or(inner.len());
        inner.replace_range(i..end.max(i), "");
    }
    if !inner.contains("<w:jc ") {
        inner.push_str("<w:jc w:val=\"center\"/>");
    }
    if !inner.contains("<w:keepNext") {
        inner.insert_str(0, "<w:keepNext/>");
    }
    format!("<w:pPr>{inner}</w:pPr>")
}

/// The drawing's run: an inline picture of `w` × `h` points.
fn picture_run(n: usize, doc_id: u64, caption: &str, w: f64, h: f64) -> String {
    let (cx, cy) = ((w * EMU_PER_PT).round() as u64, (h * EMU_PER_PT).round() as u64);
    let name = format!("Schema {n}");
    let descr = escape(caption);
    format!(
        "<w:r><w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{cx}\" cy=\"{cy}\"/>\
<wp:effectExtent l=\"0\" t=\"0\" r=\"0\" b=\"0\"/><wp:docPr id=\"{doc_id}\" name=\"{name}\" descr=\"{descr}\"/>\
<wp:cNvGraphicFramePr><a:graphicFrameLocks xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" noChangeAspect=\"1\"/></wp:cNvGraphicFramePr>\
<a:graphic xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">\
<pic:pic xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:nvPicPr><pic:cNvPr id=\"{doc_id}\" name=\"{name}\" descr=\"{descr}\"/><pic:cNvPicPr/></pic:nvPicPr>\
<pic:blipFill><a:blip r:embed=\"{RID}{n}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>\
<pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic>\
</a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"
    )
}

/// Writes figures, collecting their media.
struct Writer<'a> {
    pics: &'a [Picture],
    media: Vec<Media>,
    next_doc_id: u64,
    area: (f64, f64),
}

impl Writer<'_> {
    /// Drawing `k` (0-based) as two paragraphs (picture, caption) with `ppr`, at most `max_w` wide.
    fn figure(&mut self, k: usize, ppr: &str, max_w: f64) -> String {
        let Some(p) = self.pics.get(k) else { return String::new() };
        let (Some(ext), Some(px)) = (picture::kind(&p.data), picture::size(&p.data)) else { return String::new() };
        let (w, h) = picture::fit(px, max_w.min(self.area.0), self.area.1);
        self.media.push(Media { part: String::new(), data: p.data.clone(), ext });
        let n = self.media.len();
        if let Some(m) = self.media.last_mut() {
            m.part = format!("word/media/quotecraft-drawing-{n}.{}", if ext == "jpeg" { "jpg" } else { ext });
        }
        self.next_doc_id = self.next_doc_id.saturating_add(1);
        let mut out = format!("<w:p>{ppr}{}</w:p>", picture_run(n, self.next_doc_id, &p.caption, w, h));
        if !p.caption.is_empty() {
            let cap_ppr = ppr.replace("<w:keepNext/>", "");
            out.push_str(&format!("<w:p>{cap_ppr}<w:r><w:rPr><w:i/></w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>", escape(&p.caption)));
        }
        out
    }

    fn figures(&mut self, ks: &[usize], ppr: &str, max_w: f64) -> String {
        ks.iter().map(|k| self.figure(*k, ppr, max_w)).collect()
    }
}

/// The highest `docPr` id in the document (new drawings take the next ones).
fn max_doc_id(xml: &str) -> u64 {
    let mut max = 0u64;
    let mut at = 0;
    while let Some(i) = xml.get(at..).and_then(|s| s.find("docPr ")).map(|i| at + i) {
        at = i + 6;
        let tag = xml.get(i..).and_then(|t| t.get(..t.find('>')?)).unwrap_or("");
        if let Some(v) = tag.find(" id=\"").and_then(|j| tag.get(j + 5..)).and_then(|v| v.get(..v.find('"')?)).and_then(|v| v.parse::<u64>().ok()) {
            max = max.max(v);
        }
    }
    max
}

/// Where the body's content ends: before its last `<w:sectPr>` when that one belongs to the
/// body (not to a paragraph), else before `</w:body>`.
fn body_end(xml: &str) -> Option<usize> {
    let close = xml.rfind("</w:body>")?;
    let before = xml.get(..close)?;
    let last_block = before.rfind("</w:p>").into_iter().chain(before.rfind("</w:tbl>")).max().unwrap_or(0);
    match before.rfind("<w:sectPr") {
        Some(s) if s > last_block => Some(s),
        _ => Some(close),
    }
}

/// Adds the namespaces the pictures need to the document's root element.
fn with_namespaces(xml: &str) -> String {
    let Some(i) = xml.find("<w:document") else { return xml.to_string() };
    let Some(end) = xml.get(i..).and_then(|s| s.find('>')).map(|e| i + e) else { return xml.to_string() };
    let tag = xml.get(i..end).unwrap_or("");
    let mut add = String::new();
    for (p, uri) in [
        ("wp", "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"),
        ("r", "http://schemas.openxmlformats.org/officeDocument/2006/relationships"),
    ] {
        if !tag.contains(&format!("xmlns:{p}=")) {
            add.push_str(&format!(" xmlns:{p}=\"{uri}\""));
        }
    }
    let at = if tag.ends_with('/') { end - 1 } else { end };
    format!("{}{add}{}", xml.get(..at).unwrap_or(""), xml.get(at..).unwrap_or(""))
}

/// Places the drawings in `document.xml`. With `body_wanted` and no `{{corpo}}`, a `{{corpo}}`
/// paragraph is added at the end so the body follows the letterhead (and precedes the
/// drawings it does not place). `None` when nothing changes.
pub fn place(xml: &str, pics: &[Picture], body_wanted: bool) -> Option<Placed> {
    let text = visible(xml);
    let asks_any = text.contains("{{schemi}}") || text.contains("{{schema.");
    let add_body = body_wanted && !text.contains("{{corpo}}");
    if pics.is_empty() && !asks_any && !add_body {
        return None;
    }
    // The paragraphs that ask for drawings.
    let mut asks: Vec<(usize, usize, Ask)> = Vec::new();
    let mut corpo: Option<usize> = None;
    let mut at = 0;
    while let Some(start) = next_para(xml, at) {
        let Some(end) = para_end(xml, start) else { break };
        let para = xml.get(start..end).unwrap_or("");
        if next_para(para, 4).is_some() {
            // A text box: look inside.
            at = start + 4;
            continue;
        }
        let t = visible(para);
        if let Some(a) = ask_of(&t) {
            asks.push((start, end, a));
        } else if corpo.is_none() && t.contains("{{corpo}}") {
            corpo = Some(end);
        }
        at = end;
    }
    let placed: Vec<usize> = asks.iter().filter_map(|(_, _, a)| if let Ask::One(n) = a { n.checked_sub(1) } else { None }).collect();
    let rest: Vec<usize> = (0..pics.len()).filter(|k| !placed.contains(k)).collect();
    let has_rest = asks.iter().any(|(_, _, a)| *a == Ask::Rest);
    let mut w = Writer { pics, media: Vec::new(), next_doc_id: max_doc_id(xml).max(9_000), area: text_area(xml) };
    let mut out = String::with_capacity(xml.len());
    let mut last = 0;
    let mut rest_used = false;
    for (start, end, a) in &asks {
        out.push_str(xml.get(last..*start).unwrap_or(""));
        let para = xml.get(*start..*end).unwrap_or("");
        let ppr = figure_ppr(para);
        let max_w = cell_width(xml, *start).unwrap_or(f64::MAX);
        out.push_str(&match a {
            Ask::One(n) => n.checked_sub(1).map(|k| w.figures(&[k], &ppr, max_w)).unwrap_or_default(),
            Ask::Rest if !rest_used => {
                rest_used = true;
                w.figures(&rest, &ppr, max_w)
            }
            Ask::Rest => String::new(),
        });
        // A section break in the paragraph stays, in an empty paragraph of its own.
        if para.contains("<w:sectPr") {
            let p_open = para.find("<w:pPr>").unwrap_or(0);
            let p_close = para.find("</w:pPr>").map(|e| e + 8).unwrap_or(p_open);
            out.push_str(&format!("<w:p>{}</w:p>", para.get(p_open..p_close).unwrap_or("")));
        }
        last = *end;
    }
    out.push_str(xml.get(last..).unwrap_or(""));
    // The drawings left over, after the body: from a new page, like an annex.
    let leftover = !has_rest && !rest.is_empty();
    let mut tail = String::new();
    if leftover {
        tail.push_str("<w:p><w:pPr><w:keepNext/><w:pageBreakBefore/><w:spacing w:after=\"240\"/></w:pPr><w:r><w:rPr><w:b/><w:sz w:val=\"24\"/></w:rPr><w:t>Schemi</w:t></w:r></w:p>");
        tail.push_str(&w.figures(&rest, "<w:pPr><w:keepNext/><w:jc w:val=\"center\"/></w:pPr>", f64::MAX));
    }
    let out = match corpo.and_then(|_| corpo_end(&out)) {
        // After the paragraph of {{corpo}}.
        Some(pos) if leftover => format!("{}{tail}{}", out.get(..pos).unwrap_or(&out), out.get(pos..).unwrap_or("")),
        Some(_) => out,
        None => {
            let mut end_text = String::new();
            if add_body {
                end_text.push_str("<w:p><w:r><w:t>{{corpo}}</w:t></w:r></w:p>");
            }
            end_text.push_str(&tail);
            match body_end(&out) {
                Some(pos) if !end_text.is_empty() => format!("{}{end_text}{}", out.get(..pos).unwrap_or(&out), out.get(pos..).unwrap_or("")),
                _ => out,
            }
        }
    };
    let out = if w.media.is_empty() { out } else { with_namespaces(&out) };
    Some(Placed { xml: out, media: w.media })
}

/// End of the first paragraph that holds `{{corpo}}`.
fn corpo_end(xml: &str) -> Option<usize> {
    let mut at = 0;
    while let Some(start) = next_para(xml, at) {
        let end = para_end(xml, start)?;
        let para = xml.get(start..end)?;
        if next_para(para, 4).is_some() {
            at = start + 4;
            continue;
        }
        if visible(para).contains("{{corpo}}") {
            return Some(end);
        }
        at = end;
    }
    None
}

/// The relationships part with the pictures' relationships added.
pub fn with_relationships(rels: Option<&str>, media: &[Media]) -> String {
    let base = rels.unwrap_or(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"></Relationships>",
    );
    let add: String = media
        .iter()
        .enumerate()
        .map(|(i, m)| {
            format!(
                "<Relationship Id=\"{RID}{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"{}\"/>",
                i + 1,
                m.part.trim_start_matches("word/")
            )
        })
        .collect();
    match base.rfind("</Relationships>") {
        Some(i) => format!("{}{add}{}", base.get(..i).unwrap_or(""), base.get(i..).unwrap_or("")),
        None => base.to_string(),
    }
}

/// The content types with a default for every picture extension used.
pub fn with_content_types(types: &str, media: &[Media]) -> String {
    let mut add = String::new();
    let lower = types.to_ascii_lowercase();
    let mut seen: Vec<&str> = Vec::new();
    for m in media {
        let ext = m.part.rsplit('.').next().unwrap_or("png");
        if seen.contains(&ext) || lower.contains(&format!("extension=\"{ext}\"")) {
            continue;
        }
        seen.push(ext);
        add.push_str(&format!("<Default Extension=\"{ext}\" ContentType=\"{}\"/>", picture::mime(m.ext)));
    }
    match types.rfind("</Types>") {
        Some(i) => format!("{}{add}{}", types.get(..i).unwrap_or(""), types.get(i..).unwrap_or("")),
        None => types.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut d = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        d.extend_from_slice(&w.to_be_bytes());
        d.extend_from_slice(&h.to_be_bytes());
        d.extend_from_slice(&[8, 2, 0, 0, 0]);
        d
    }

    fn pics() -> Vec<Picture> {
        vec![
            Picture { name: "a.png".into(), caption: "Rete attuale".into(), data: png(2400, 1200) },
            Picture { name: "b.png".into(), caption: "Rete <proposta>".into(), data: png(400, 200) },
            Picture { name: "c.png".into(), caption: String::new(), data: png(100, 100) },
        ]
    }

    fn p(text: &str) -> String {
        format!("<w:p><w:pPr><w:jc w:val=\"left\"/></w:pPr><w:r><w:t>{text}</w:t></w:r></w:p>")
    }

    const SECT: &str =
        "<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1418\" w:right=\"1134\" w:bottom=\"1134\" w:left=\"1134\"/></w:sectPr>";

    fn doc(body: &str) -> String {
        format!("<w:document xmlns:w=\"w\"><w:body>{body}{SECT}</w:body></w:document>")
    }

    #[test]
    fn placeholders_become_figures_and_the_rest_follows_the_body() {
        let xml = doc(&format!("{}{}{}{}", p("{{schema.2}}"), p("{{corpo}}"), p("Note finali"), p("{{schema.7}}")));
        let out = place(&xml, &pics(), true).unwrap();
        assert_eq!(out.media.len(), 3, "b at its place, a and c after the body");
        let x = &out.xml;
        assert!(x.starts_with("<w:document xmlns:w=\"w\" xmlns:wp=") && x.contains("xmlns:r="), "{x}");
        assert!(!x.contains("{{schema."), "placeholders gone, the unknown seventh too");
        let (b, corpo, rest, note) =
            (x.find("Rete &lt;proposta&gt;").unwrap(), x.find("{{corpo}}").unwrap(), x.find(">Schemi<").unwrap(), x.find("Note finali").unwrap());
        assert!(b < corpo && corpo < rest && rest < note, "{x}");
        assert!(x.contains("<w:jc w:val=\"left\"/>"), "the placeholder paragraph's alignment is kept");
        // a: 2400 px is 1800 pt, fitted to the text width (11906 - 2 × 1134 twips = 481.9 pt).
        assert!(x.contains("cx=\"6120130\""), "{x}");
        assert_eq!(x.matches("<wp:docPr id=").count(), 3);
        assert!(x.contains("r:embed=\"rIdQuotecraftDrawing3\""));
        assert_eq!(out.media[0].part, "word/media/quotecraft-drawing-1.png");
    }

    #[test]
    fn schemi_takes_the_drawings_not_placed_and_nothing_is_appended() {
        let xml = doc(&format!("{}{}{}", p("{{schemi}}"), p("{{schema.1}}"), p("{{corpo}}")));
        let out = place(&xml, &pics(), true).unwrap();
        assert!(!out.xml.contains(">Schemi<") && !out.xml.contains("{{schemi}}"));
        assert_eq!(out.media.len(), 3);
        let x = &out.xml;
        assert!(x.find("Rete &lt;proposta&gt;").unwrap() < x.find("Rete attuale").unwrap(), "rest first, then the first one");
    }

    #[test]
    fn without_corpo_the_body_and_the_drawings_go_at_the_end() {
        let xml = doc(&p("Carta"));
        let out = place(&xml, &pics()[..1], true).unwrap();
        let x = &out.xml;
        let (carta, corpo, schemi, sect) =
            (x.find("Carta").unwrap(), x.find("{{corpo}}").unwrap(), x.find(">Schemi<").unwrap(), x.find("<w:sectPr").unwrap());
        assert!(carta < corpo && corpo < schemi && schemi < sect, "{x}");
        // No body wanted (the letterhead lays the lines out): only the drawings.
        let out = place(&xml, &pics()[..1], false).unwrap();
        assert!(!out.xml.contains("{{corpo}}") && out.xml.contains(">Schemi<"));
        // No drawings, a body wanted and no {{corpo}}: a {{corpo}} at the end, nothing else.
        let out = place(&xml, &[], true).unwrap();
        assert!(out.media.is_empty() && out.xml.contains("<w:t>{{corpo}}</w:t></w:r></w:p><w:sectPr"));
        assert!(place(&xml, &[], false).is_none(), "nothing to do");
        // A placeholder with no drawings: its paragraph goes.
        let out = place(&doc(&p("{{schemi}}")), &[], false).unwrap();
        assert!(!out.xml.contains("schemi") && !out.xml.contains("<w:drawing>"));
    }

    #[test]
    fn a_figure_fits_its_cell_and_a_section_break_stays() {
        let cell = format!("<w:tbl><w:tr><w:tc><w:tcPr><w:tcW w:w=\"2880\" w:type=\"dxa\"/></w:tcPr>{}</w:tc></w:tr></w:tbl>", p("{{schema.1}}"));
        let out = place(&doc(&cell), &pics(), false).unwrap();
        // 2880 twips = 144 pt, less the cell margins: 133 pt.
        assert!(out.xml.contains("cx=\"1689100\""), "{}", out.xml);
        let brk = doc("<w:p><w:pPr><w:sectPr><w:type w:val=\"nextPage\"/></w:sectPr></w:pPr><w:r><w:t>{{schema.2}}</w:t></w:r></w:p>");
        let out = place(&brk, &pics(), false).unwrap();
        assert_eq!(out.xml.matches("<w:sectPr>").count(), 2, "the paragraph's break and the body's");
        assert!(out.xml.find("<w:drawing>").unwrap() < out.xml.find("nextPage").unwrap());
    }

    #[test]
    fn text_boxes_and_hostile_xml_do_not_break_it() {
        let boxed = doc(&format!("<w:p><w:r><w:pict><w:txbxContent>{}</w:txbxContent></w:pict></w:r></w:p>", p("{{schema.1}}")));
        let out = place(&boxed, &pics(), false).unwrap();
        assert!(out.xml.contains("<w:drawing>") && out.xml.contains("<w:txbxContent><w:p>"), "{}", out.xml);
        for bad in ["<w:p", "<w:p><w:t>{{schemi}}", "<w:p/><w:p>{{schema.", "</w:body>", "<w:document"] {
            let _ = place(bad, &pics(), true);
        }
        let not_pic = vec![Picture { name: "x.png".into(), caption: "x".into(), data: b"nope".to_vec() }];
        let out = place(&doc(&p("{{schemi}}")), &not_pic, false).unwrap();
        assert!(out.media.is_empty() && !out.xml.contains("<w:drawing>"));
    }

    #[test]
    fn relationships_and_content_types_name_the_pictures() {
        let out = place(&doc(&p("{{schemi}}")), &pics(), false).unwrap();
        let rels = with_relationships(Some("<Relationships><Relationship Id=\"rId1\"/></Relationships>"), &out.media);
        assert!(rels.contains("Id=\"rIdQuotecraftDrawing2\"") && rels.contains("Target=\"media/quotecraft-drawing-2.png\""));
        assert!(with_relationships(None, &out.media).contains("<Relationships xmlns="));
        let types = with_content_types("<Types><Default Extension=\"PNG\" ContentType=\"image/png\"/></Types>", &out.media);
        assert_eq!(types.matches("Extension=").count(), 1, "png already there");
        let jpg = vec![Media { part: "word/media/quotecraft-drawing-1.jpg".into(), data: vec![], ext: "jpeg" }];
        assert!(with_content_types("<Types></Types>", &jpg).contains("<Default Extension=\"jpg\" ContentType=\"image/jpeg\"/>"));
    }
}

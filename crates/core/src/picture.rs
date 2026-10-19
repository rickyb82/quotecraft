//! The pictures a quote shows (network diagrams, plans): which kind they are and their size in
//! pixels, read from the file's header, so that they can be placed at a size that fits the
//! page. Hostile bytes give `None`, never a panic.

/// Largest picture placed in a document (bytes).
pub const MAX_BYTES: usize = 15 * 1024 * 1024;
/// Widest a picture is shown (points: 16.5 cm, an A4 page's text width).
pub const MAX_WIDTH_PT: f64 = 468.0;
/// Tallest a picture is shown (points: 21 cm, leaving room for its caption).
pub const MAX_HEIGHT_PT: f64 = 595.0;

/// The kind of picture (`png`, `jpeg`, `gif`), from its first bytes.
pub fn kind(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpeg")
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        Some("gif")
    } else {
        None
    }
}

/// The MIME type of a kind.
pub fn mime(kind: &str) -> &'static str {
    match kind {
        "png" => "image/png",
        "gif" => "image/gif",
        _ => "image/jpeg",
    }
}

/// Whether a file name has a picture's extension.
pub fn is_picture_name(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or_default().to_ascii_lowercase();
    matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif")
}

fn be16(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from(*d.get(at)?) << 8 | u32::from(*d.get(at + 1)?))
}

fn be32(d: &[u8], at: usize) -> Option<u32> {
    Some(be16(d, at)? << 16 | be16(d, at + 2)?)
}

/// The size in pixels (width, height), both above zero.
pub fn size(data: &[u8]) -> Option<(u32, u32)> {
    let (w, h) = match kind(data)? {
        "png" => {
            if data.get(12..16)? != b"IHDR" {
                return None;
            }
            (be32(data, 16)?, be32(data, 20)?)
        }
        "gif" => (u32::from(*data.get(6)?) | u32::from(*data.get(7)?) << 8, u32::from(*data.get(8)?) | u32::from(*data.get(9)?) << 8),
        _ => jpeg_size(data)?,
    };
    (w > 0 && h > 0).then_some((w, h))
}

/// A JPEG's size: the frame header (SOFn) found by walking the markers.
fn jpeg_size(d: &[u8]) -> Option<(u32, u32)> {
    let mut at = 2usize;
    for _ in 0..10_000 {
        if *d.get(at)? != 0xFF {
            return None;
        }
        let marker = *d.get(at + 1)?;
        match marker {
            // Fill bytes and markers without a length.
            0xFF => {
                at += 1;
                continue;
            }
            0x01 | 0xD0..=0xD7 => {
                at += 2;
                continue;
            }
            0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => return Some((be16(d, at + 7)?, be16(d, at + 5)?)),
            0xD9 | 0xDA => return None,
            _ => {}
        }
        let len = be16(d, at + 2)? as usize;
        if len < 2 {
            return None;
        }
        at = at.checked_add(2)?.checked_add(len)?;
    }
    None
}

/// The size a picture is shown at (points; 96 pixels to the inch), shrunk to fit within
/// `max_w` × `max_h`, never enlarged.
pub fn fit((w, h): (u32, u32), max_w: f64, max_h: f64) -> (f64, f64) {
    let (w, h) = (f64::from(w.max(1)) * 0.75, f64::from(h.max(1)) * 0.75);
    let scale = (max_w / w).min(max_h / h).min(1.0);
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    ((w * scale).max(1.0), (h * scale).max(1.0))
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding (the pictures of the HTML, as `data:` URIs).
pub fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3).saturating_mul(4));
    for ch in data.chunks(3) {
        let b = [ch.first().copied().unwrap_or(0), ch.get(1).copied().unwrap_or(0), ch.get(2).copied().unwrap_or(0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= ch.len() {
                out.push(B64.get(((n >> (18 - 6 * i)) & 63) as usize).copied().unwrap_or(b'A') as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A PNG header of `w` × `h` (enough for `kind` and `size`).
    pub(crate) fn png(w: u32, h: u32) -> Vec<u8> {
        let mut d = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        d.extend_from_slice(&w.to_be_bytes());
        d.extend_from_slice(&h.to_be_bytes());
        d.extend_from_slice(&[8, 2, 0, 0, 0]);
        d
    }

    #[test]
    fn kinds_and_sizes_from_the_header() {
        assert_eq!((kind(&png(800, 600)), size(&png(800, 600))), (Some("png"), Some((800, 600))));
        assert_eq!(size(&png(0, 600)), None);
        let gif = b"GIF89a\x40\x01\xf0\x00rest";
        assert_eq!(size(gif), Some((320, 240)));
        // JPEG: SOI, an APP0 segment, then SOF0 with height 0x01E0 and width 0x0280.
        let mut jpg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x4A, 0x46];
        jpg.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08, 0x01, 0xE0, 0x02, 0x80, 0x03]);
        assert_eq!((kind(&jpg), size(&jpg)), (Some("jpeg"), Some((640, 480))));
        assert_eq!(mime("jpeg"), "image/jpeg");
        assert!(is_picture_name("Schema.PNG") && is_picture_name("a.jpeg") && !is_picture_name("a.pdf"));
    }

    #[test]
    fn hostile_headers_give_nothing() {
        for d in [&b""[..], b"\x89PNG\r\n\x1a\n", b"\xFF\xD8\xFF", b"\xFF\xD8\xFF\xE0\x00\x00", b"\xFF\xD8\xFF\xE0\xFF\xFF", b"GIF89a\x01", b"<svg/>"]
        {
            assert_eq!(size(d), None, "{d:?}");
        }
        // A JPEG made of fill bytes only ends.
        let mut fill = vec![0xFF, 0xD8];
        fill.extend(std::iter::repeat_n(0xFF, 50_000));
        assert_eq!(size(&fill), None);
    }

    #[test]
    fn pictures_fit_the_page_and_are_never_enlarged() {
        assert_eq!(fit((400, 200), MAX_WIDTH_PT, MAX_HEIGHT_PT), (300.0, 150.0), "natural size at 96 dpi");
        let (w, h) = fit((2400, 1200), MAX_WIDTH_PT, MAX_HEIGHT_PT);
        assert!((w - 468.0).abs() < 1e-9 && (h - 234.0).abs() < 1e-9);
        let (w, h) = fit((1000, 4000), MAX_WIDTH_PT, MAX_HEIGHT_PT);
        assert!((h - 595.0).abs() < 1e-9 && (w - 148.75).abs() < 1e-9, "tall: fitted to the height");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"Man"), "TWFu");
    }
}

//! The icon set, drawn in code (strokes on a 24-unit grid): no bitmap or vendor artwork.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Logo,
    Quotes,
    Items,
    Customers,
    Templates,
    Settings,
    Moon,
    Sun,
    Globe,
    Plus,
    Close,
    Check,
    Download,
    Send,
    Save,
    Copy,
    Trash,
    Warning,
    Search,
    Back,
    Document,
    Up,
    Down,
}

/// Paints `icon` into `rect` with stroke colour `c`.
pub fn paint(p: &Painter, rect: Rect, icon: Icon, c: Color32) {
    let s = rect.width().min(rect.height()) / 24.0;
    let o = rect.center() - vec2(12.0, 12.0) * s;
    let at = |x: f32, y: f32| -> Pos2 { o + vec2(x, y) * s };
    let w = (1.7 * s).max(1.0);
    let st = Stroke::new(w, c);
    let line = |pts: &[(f32, f32)]| {
        p.add(Shape::line(pts.iter().map(|(x, y)| at(*x, *y)).collect(), st));
    };
    let rrect = |x0: f32, y0: f32, x1: f32, y1: f32, r: f32| {
        p.rect_stroke(Rect::from_min_max(at(x0, y0), at(x1, y1)), r * s, st, egui::StrokeKind::Middle);
    };
    let circle = |x: f32, y: f32, r: f32| {
        p.circle_stroke(at(x, y), r * s, st);
    };
    let dot = |x: f32, y: f32, r: f32| {
        p.circle_filled(at(x, y), r * s, c);
    };
    match icon {
        Icon::Logo => {
            // The QuoteCraft mark: a sheet with a price tag corner, and the craft-family spark.
            p.circle_filled(at(12.0, 12.0), 9.5 * s, Color32::from_rgb(0x1f, 0x8a, 0x5c));
            let white = Stroke::new(w * 1.1, Color32::WHITE);
            p.add(Shape::line(vec![at(8.0, 7.0), at(13.5, 7.0), at(16.5, 10.0), at(16.5, 17.5), at(8.0, 17.5), at(8.0, 7.0)], white));
            p.add(Shape::line(vec![at(10.0, 12.0), at(14.5, 12.0)], white));
            p.add(Shape::line(vec![at(10.0, 14.5), at(13.0, 14.5)], white));
            p.circle_filled(at(19.0, 5.0), 3.2 * s, Color32::from_rgb(0xff, 0xb4, 0x3d));
        }
        Icon::Quotes => {
            line(&[(6.0, 3.5), (14.0, 3.5), (18.5, 8.0), (18.5, 20.5), (6.0, 20.5), (6.0, 3.5)]);
            line(&[(14.0, 3.5), (14.0, 8.0), (18.5, 8.0)]);
            line(&[(9.0, 12.0), (15.5, 12.0)]);
            line(&[(9.0, 15.5), (15.5, 15.5)]);
        }
        Icon::Document => {
            line(&[(6.0, 3.5), (14.0, 3.5), (18.5, 8.0), (18.5, 20.5), (6.0, 20.5), (6.0, 3.5)]);
            line(&[(14.0, 3.5), (14.0, 8.0), (18.5, 8.0)]);
        }
        Icon::Items => {
            // A price tag.
            line(&[(4.0, 4.0), (11.5, 4.0), (20.5, 13.0), (13.0, 20.5), (4.0, 11.5), (4.0, 4.0)]);
            dot(8.0, 8.0, 1.4);
        }
        Icon::Customers => {
            circle(9.0, 8.5, 3.5);
            p.add(Shape::line(arc(at(9.0, 20.0), 6.5 * s, 200.0, 340.0, 12), st));
            p.add(Shape::line(arc(at(16.0, 9.0), 3.0 * s, 250.0, 470.0, 10), st));
            p.add(Shape::line(arc(at(16.5, 20.0), 5.5 * s, 270.0, 340.0, 8), st));
        }
        Icon::Templates => {
            rrect(4.0, 4.0, 20.0, 20.0, 1.5);
            line(&[(4.0, 9.0), (20.0, 9.0)]);
            line(&[(7.5, 6.5), (12.0, 6.5)]);
            line(&[(7.5, 13.0), (16.5, 13.0)]);
            line(&[(7.5, 16.5), (14.0, 16.5)]);
        }
        Icon::Settings => {
            circle(12.0, 12.0, 3.2);
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::FRAC_PI_4;
                let (cx, cy) = (a.cos(), a.sin());
                line(&[(12.0 + cx * 6.0, 12.0 + cy * 6.0), (12.0 + cx * 9.0, 12.0 + cy * 9.0)]);
            }
            circle(12.0, 12.0, 6.5);
        }
        Icon::Moon => {
            p.add(Shape::line(arc(at(12.0, 12.0), 8.0 * s, 40.0, 320.0, 24), st));
            p.add(Shape::line(arc(at(17.0, 9.0), 6.4 * s, 100.0, 232.0, 16), st));
        }
        Icon::Sun => {
            circle(12.0, 12.0, 4.0);
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::FRAC_PI_4;
                let (cx, cy) = (a.cos(), a.sin());
                line(&[(12.0 + cx * 7.0, 12.0 + cy * 7.0), (12.0 + cx * 9.5, 12.0 + cy * 9.5)]);
            }
        }
        Icon::Globe => {
            circle(12.0, 12.0, 8.5);
            line(&[(3.5, 12.0), (20.5, 12.0)]);
            for side in [-1.0, 1.0] {
                p.add(Shape::line(
                    arc(at(12.0, 12.0), 8.5 * s, -90.0 * side, 90.0 * side, 16)
                        .into_iter()
                        .map(|q| Pos2::new(at(12.0, 0.0).x + (q.x - at(12.0, 0.0).x) * 0.45, q.y))
                        .collect(),
                    st,
                ));
            }
        }
        Icon::Plus => {
            line(&[(12.0, 5.0), (12.0, 19.0)]);
            line(&[(5.0, 12.0), (19.0, 12.0)]);
        }
        Icon::Close => {
            line(&[(6.0, 6.0), (18.0, 18.0)]);
            line(&[(18.0, 6.0), (6.0, 18.0)]);
        }
        Icon::Check => {
            circle(12.0, 12.0, 8.5);
            line(&[(7.5, 12.5), (10.5, 15.5), (16.5, 9.0)]);
        }
        Icon::Download => {
            line(&[(12.0, 4.0), (12.0, 15.0)]);
            line(&[(7.5, 10.5), (12.0, 15.0), (16.5, 10.5)]);
            line(&[(4.5, 19.5), (19.5, 19.5)]);
        }
        Icon::Send => {
            line(&[(3.5, 12.0), (20.5, 4.0), (15.5, 20.5), (12.0, 13.5), (3.5, 12.0)]);
            line(&[(12.0, 13.5), (20.5, 4.0)]);
        }
        Icon::Save => {
            line(&[(4.0, 4.0), (16.5, 4.0), (20.0, 7.5), (20.0, 20.0), (4.0, 20.0), (4.0, 4.0)]);
            rrect(8.0, 4.0, 15.0, 9.0, 0.5);
            rrect(7.5, 13.0, 16.5, 20.0, 0.5);
        }
        Icon::Copy => {
            rrect(8.0, 8.0, 20.0, 20.0, 1.5);
            line(&[(5.0, 15.5), (4.0, 15.5), (4.0, 4.0), (15.5, 4.0), (15.5, 5.0)]);
        }
        Icon::Trash => {
            line(&[(4.5, 7.0), (19.5, 7.0)]);
            line(&[(9.5, 7.0), (9.5, 4.5), (14.5, 4.5), (14.5, 7.0)]);
            line(&[(6.5, 7.0), (7.5, 20.0), (16.5, 20.0), (17.5, 7.0)]);
            line(&[(10.5, 10.5), (10.5, 16.5)]);
            line(&[(13.5, 10.5), (13.5, 16.5)]);
        }
        Icon::Warning => {
            line(&[(12.0, 3.5), (21.0, 20.0), (3.0, 20.0), (12.0, 3.5)]);
            line(&[(12.0, 9.5), (12.0, 14.5)]);
            dot(12.0, 17.2, 1.1);
        }
        Icon::Search => {
            circle(10.5, 10.5, 6.0);
            line(&[(15.0, 15.0), (20.5, 20.5)]);
        }
        Icon::Back => {
            line(&[(10.0, 6.0), (4.0, 12.0), (10.0, 18.0)]);
            line(&[(4.0, 12.0), (20.0, 12.0)]);
        }
        Icon::Up => {
            line(&[(7.0, 14.0), (12.0, 9.0), (17.0, 14.0)]);
        }
        Icon::Down => {
            line(&[(7.0, 10.0), (12.0, 15.0), (17.0, 10.0)]);
        }
    }
}

/// Points on a circular arc, angles in degrees (0 = right, clockwise on screen).
fn arc(c: Pos2, r: f32, from: f32, to: f32, n: usize) -> Vec<Pos2> {
    (0..=n)
        .map(|i| {
            let a = (from + (to - from) * i as f32 / n as f32).to_radians();
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect()
}

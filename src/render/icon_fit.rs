//! `fit` — how an icon's symbol (or an `|image|`) maps into its box (SPEC §7/§10).
//!
//! The box is fixed by layout; `fit` only scales and centres the content inside
//! it. `auto` keeps the content's natural framing (for an icon, the full 256-grid
//! — Phosphor's authored margin). `contain`/`cover`/`stretch` instead measure the
//! glyph's *own* extent: the union of every fragment's bounding box — paths via
//! [`extent_points`], the handful of `<circle>`/`<rect>`/`<line>`/`<poly*>`/
//! `<ellipse>` primitives Phosphor also uses, each through its optional
//! `translate … rotate` transform (the only forms the set carries).

use crate::icon::Role;
use crate::layout::path_bbox::extent_points;
use crate::resolve::{AttrMap, ResolvedValue};

/// How content maps into its box. Default [`Fit::Auto`]; the value is validated at
/// resolve ([`crate::resolve`]), so an unknown one never reaches here.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fit {
    Auto,
    Contain,
    Cover,
    Stretch,
}

impl Fit {
    pub fn of(attrs: &AttrMap) -> Self {
        match attrs.get("fit") {
            Some(ResolvedValue::Ident(s)) => match s.as_str() {
                "contain" => Fit::Contain,
                "cover" => Fit::Cover,
                "stretch" => Fit::Stretch,
                _ => Fit::Auto,
            },
            _ => Fit::Auto,
        }
    }
}

/// The glyph's content rectangle as `(cx, cy, w, h)` in 256-grid units — the union
/// of every fragment's extent. `None` when no fragment yields drawable, non-degenerate
/// geometry (the caller then falls back to the full grid).
pub fn glyph_box(frags: &[(Role, &str)]) -> Option<(f64, f64, f64, f64)> {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(_, frag) in frags {
        for (x, y) in fragment_points(frag) {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if x1 - x0 < 1e-6 || y1 - y0 < 1e-6 {
        return None;
    }
    Some(((x0 + x1) / 2.0, (y0 + y1) / 2.0, x1 - x0, y1 - y0))
}

/// Bounding points of one stored SVG fragment, in the glyph's own coordinates,
/// with its `transform` (if any) applied.
fn fragment_points(frag: &str) -> Vec<(f64, f64)> {
    let tag = frag
        .trim_start_matches('<')
        .split([' ', '/', '>'])
        .next()
        .unwrap_or("");
    let mut pts = match tag {
        "path" => attr(frag, "d")
            .map(|d| extent_points(&d))
            .unwrap_or_default(),
        "line" => vec![
            (numf(frag, "x1"), numf(frag, "y1")),
            (numf(frag, "x2"), numf(frag, "y2")),
        ],
        "circle" => {
            let (cx, cy, r) = (numf(frag, "cx"), numf(frag, "cy"), numf(frag, "r"));
            vec![(cx - r, cy - r), (cx + r, cy + r)]
        }
        "ellipse" => {
            let (cx, cy, rx, ry) = (
                numf(frag, "cx"),
                numf(frag, "cy"),
                numf(frag, "rx"),
                numf(frag, "ry"),
            );
            vec![(cx - rx, cy - ry), (cx + rx, cy + ry)]
        }
        "rect" => {
            let (x, y, w, h) = (
                numf(frag, "x"),
                numf(frag, "y"),
                numf(frag, "width"),
                numf(frag, "height"),
            );
            vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)]
        }
        "polyline" | "polygon" => points_attr(frag),
        _ => Vec::new(),
    };
    if let Some(t) = attr(frag, "transform") {
        let m = parse_transform(&t);
        for p in &mut pts {
            *p = m.apply(*p);
        }
    }
    pts
}

/// The value of a space-delimited `name="…"` attribute. The leading space anchors
/// the name so `r` never matches inside `rx`, nor `x` inside `rx`/`width`.
fn attr(frag: &str, name: &str) -> Option<String> {
    let pat = format!(" {name}=\"");
    let start = frag.find(&pat)? + pat.len();
    let end = frag[start..].find('"')? + start;
    Some(frag[start..end].to_string())
}

fn numf(frag: &str, name: &str) -> f64 {
    attr(frag, name)
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0.0)
}

fn points_attr(frag: &str) -> Vec<(f64, f64)> {
    let Some(s) = attr(frag, "points") else {
        return Vec::new();
    };
    let nums: Vec<f64> = s
        .split([' ', ','])
        .filter_map(|t| t.trim().parse().ok())
        .collect();
    nums.chunks_exact(2).map(|c| (c[0], c[1])).collect()
}

// ── Affine transform (only `translate` / `rotate` appear in the set) ─────────

#[derive(Clone, Copy)]
struct Affine {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Affine {
    fn id() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }
    /// `self · other` — composing so `other` applies first (SVG list order).
    fn mul(self, o: Affine) -> Affine {
        Affine {
            a: self.a * o.a + self.c * o.b,
            b: self.b * o.a + self.d * o.b,
            c: self.a * o.c + self.c * o.d,
            d: self.b * o.c + self.d * o.d,
            e: self.a * o.e + self.c * o.f + self.e,
            f: self.b * o.e + self.d * o.f + self.f,
        }
    }
    fn apply(self, (x, y): (f64, f64)) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }
    fn translate(tx: f64, ty: f64) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: tx,
            f: ty,
        }
    }
    fn rotate(deg: f64) -> Self {
        let r = deg.to_radians();
        Self {
            a: r.cos(),
            b: r.sin(),
            c: -r.sin(),
            d: r.cos(),
            e: 0.0,
            f: 0.0,
        }
    }
}

/// Parse an SVG `transform` list into one matrix. Unknown functions fold to the
/// identity (none beyond `translate`/`rotate` occur in the icon set).
fn parse_transform(t: &str) -> Affine {
    let mut m = Affine::id();
    let mut rest = t;
    while let Some(open) = rest.find('(') {
        let name = rest[..open]
            .split([' ', ')'])
            .next_back()
            .unwrap_or("")
            .trim();
        let Some(close_rel) = rest[open..].find(')') else {
            break;
        };
        let close = open + close_rel;
        let args: Vec<f64> = rest[open + 1..close]
            .split([' ', ','])
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        let arg = |i: usize| args.get(i).copied().unwrap_or(0.0);
        let seg = match name {
            "translate" => Affine::translate(arg(0), arg(1)),
            "rotate" if args.len() >= 3 => Affine::translate(arg(1), arg(2))
                .mul(Affine::rotate(arg(0)))
                .mul(Affine::translate(-arg(1), -arg(2))),
            "rotate" => Affine::rotate(arg(0)),
            _ => Affine::id(),
        };
        m = m.mul(seg);
        rest = &rest[close + 1..];
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_with_no_fit_attr() {
        assert_eq!(Fit::of(&AttrMap::new()), Fit::Auto);
    }

    #[test]
    fn box_unions_path_and_circle() {
        // A path from 40..216 on x and a circle reaching y=40..200 → union box.
        let frags = vec![
            (
                Role::Fill,
                r#"<path d="M40 60 L216 60 L216 200 L40 200 Z"/>"#,
            ),
            (Role::Solid, r#"<circle cx="128" cy="120" r="80"/>"#),
        ];
        let (cx, cy, w, h) = glyph_box(&frags).unwrap();
        assert!((cx - 128.0).abs() < 0.5, "cx {cx}");
        assert!((cy - 120.0).abs() < 0.5, "cy {cy}");
        assert!((w - 176.0).abs() < 0.5, "w {w}"); // 40..216
        assert!((h - 160.0).abs() < 0.5, "h {h}"); // 40..200
    }

    #[test]
    fn rotate_translate_rect_extends_box() {
        // A rect rotated 90° about the origin then translated lands at x:0..160.
        let frags = vec![(
            Role::Line,
            r#"<rect x="40" y="48" width="192" height="160" transform="translate(264 -8) rotate(90)"/>"#,
        )];
        let (_, _, w, h) = glyph_box(&frags).unwrap();
        // rotating a 192×160 rect swaps its spans → 160 wide, 192 tall.
        assert!((w - 160.0).abs() < 0.5, "w {w}");
        assert!((h - 192.0).abs() < 0.5, "h {h}");
    }

    #[test]
    fn degenerate_is_none() {
        assert!(glyph_box(&[(Role::Line, r#"<line x1="0" y1="0" x2="0" y2="0"/>"#)]).is_none());
    }
}

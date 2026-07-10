//! The `|page|` sheet [SPEC 15.8]. `sheet:` is pure sugar — the trimmed ISO
//! size in millimetres, desugared **in place** to `width` / `height` (the
//! orientation keyword swaps the pair; explicit dims override through the
//! ordinary slot). The ISO 5457 furniture — the `|frame|`, the `|zone|`
//! references, the `|tick|` dividers and centring marks — is generated
//! chrome, real children pinned `center` (out of the page's flow) and
//! positioned by `layout::page::finish` once the sheet is sized; the zone
//! counts derive from the dimensions here, ≈ 50 mm divisions rounded to the
//! nearest even count per edge.

use crate::error::Error;
use crate::span::Span;
use crate::syntax::ast::{Decl, Node, TextNode, Value};

/// The trimmed sheet sizes, portrait millimetres, with each standard's
/// default orientation: ISO 216 (A4/A5 portrait, A3–A0 landscape) and — pure
/// sugar over the same mechanism, only the millimetres differ — the
/// ANSI/ASME Y14.1 letters (`a` portrait, `b`–`e` landscape).
const SIZES: &[(&str, f64, f64, bool)] = &[
    ("a0", 841.0, 1189.0, true),
    ("a1", 594.0, 841.0, true),
    ("a2", 420.0, 594.0, true),
    ("a3", 297.0, 420.0, true),
    ("a4", 210.0, 297.0, false),
    ("a5", 148.0, 210.0, false),
    ("a", 215.9, 279.4, false),
    ("b", 279.4, 431.8, true),
    ("c", 431.8, 558.8, true),
    ("d", 558.8, 863.6, true),
    ("e", 863.6, 1117.6, true),
];

/// The `|page|` bundle's default sheet — A4, ISO portrait.
pub(super) const DEFAULT: (f64, f64) = (210.0, 297.0);

/// Expand a `sheet:` declaration in place to `width` / `height` [SPEC 15.8].
pub(super) fn expand_sheet(style: &mut Vec<Decl>) -> Result<(), Error> {
    let Some(at) = style.iter().position(|d| d.name == "sheet") else {
        return Ok(());
    };
    let d = &style[at];
    let (span, values) = (d.span, d.groups.first().cloned().unwrap_or_default());
    let bad = |got: &str| {
        let mut msg =
            "'sheet' takes a size — a5…a0 (ISO) or a…e (ANSI) — and an optional portrait / landscape"
                .to_string();
        let candidates = [
            "a0",
            "a1",
            "a2",
            "a3",
            "a4",
            "a5",
            "a",
            "b",
            "c",
            "d",
            "e",
            "portrait",
            "landscape",
        ];
        let near = crate::suggest::nearest(got, candidates, 1);
        msg.push_str(&crate::suggest::did_you_mean(&near));
        Error::at(span, msg)
    };
    let mut size: Option<(f64, f64, bool)> = None;
    let mut orient: Option<&str> = None;
    for v in &values {
        let Value::Ident(word) = v else {
            return Err(bad("…"));
        };
        if let Some(&(_, w, h, land)) = SIZES.iter().find(|(n, ..)| n == word) {
            size = Some((w, h, land));
        } else if word == "portrait" || word == "landscape" {
            orient = Some(word);
        } else {
            return Err(bad(word));
        }
    }
    let Some((pw, ph, default_landscape)) = size else {
        return Err(bad(""));
    };
    // Each standard's own default orientation [SPEC 15.8]; the keyword wins.
    let landscape = match orient {
        Some(o) => o == "landscape",
        None => default_landscape,
    };
    let (w, h) = if landscape { (ph, pw) } else { (pw, ph) };
    let decl = |name: &str, v: f64| Decl {
        name: name.into(),
        groups: vec![vec![Value::Number(v)]],
        span,
    };
    style.splice(at..=at, [decl("width", w), decl("height", h)]);
    Ok(())
}

/// The sheet dimensions the chrome derives from: the node's own `width` /
/// `height` decls (post-`sheet:` expansion), else the bundle default. A
/// rule-set size is invisible here — the same class-based limit frame
/// detection has; the zones then stretch over the real size at layout.
fn dims(style: &[Decl]) -> (f64, f64) {
    let num = |name: &str| {
        style.iter().rev().find(|d| d.name == name).and_then(|d| {
            match d.groups.first()?.first()? {
                Value::Number(n) => Some(*n),
                _ => None,
            }
        })
    };
    (
        num("width").unwrap_or(DEFAULT.0),
        num("height").unwrap_or(DEFAULT.1),
    )
}

/// Zone divisions per edge: ≈ 50 mm, the nearest **even** count (ISO 5457 —
/// A4 4 × 6, A3 8 × 6, A0 24 × 16), floor 2.
fn zone_count(mm: f64) -> usize {
    let r = mm / 50.0;
    let lo = ((r / 2.0).floor() * 2.0).max(2.0);
    let hi = lo + 2.0;
    if (r - lo) <= (hi - r) {
        lo as usize
    } else {
        hi as usize
    }
}

/// The page's generated furniture [SPEC 15.8], parser-shaped: the `|frame|`,
/// a `|tick|` per zone divider + the four centring marks, and a `|zone|`
/// label per cell on all four edges — everything pinned `center`, positioned
/// by the layout once the sheet is sized. `style` is the page's own,
/// post-`sheet:` expansion.
pub(super) fn chrome_children(style: &[Decl], at: Span) -> Vec<Node> {
    let (w, h) = dims(style);
    let (cols, rows) = (zone_count(w), zone_count(h));
    let mut out = vec![chrome(
        at,
        "frame",
        vec![Value::Ident("frame".into())],
        None,
    )];
    // Zone counts are even, so the middle divider always lands exactly on a
    // centring mark — skip it, the mark serves (no doubled line).
    for (edge, n) in [("top", cols), ("bottom", cols)] {
        for i in (1..n).filter(|i| *i != n / 2) {
            out.push(tick(at, edge, i));
        }
    }
    for (edge, n) in [("left", rows), ("right", rows)] {
        for i in (1..n).filter(|i| *i != n / 2) {
            out.push(tick(at, edge, i));
        }
    }
    for edge in ["top", "bottom", "left", "right"] {
        out.push(chrome(
            at,
            "tick",
            vec![Value::Ident("mark".into()), Value::Ident(edge.into())],
            Some(points_placeholder(at)),
        ));
    }
    for (edge, n) in [("top", cols), ("bottom", cols)] {
        for i in 0..n {
            out.push(zone(at, edge, i, (i + 1).to_string()));
        }
    }
    for (edge, n) in [("left", rows), ("right", rows)] {
        for i in 0..n {
            let letter = char::from(b'A' + (i % 26) as u8).to_string();
            out.push(zone(at, edge, i, letter));
        }
    }
    out
}

fn tick(at: Span, edge: &str, i: usize) -> Node {
    chrome(
        at,
        "tick",
        vec![
            Value::Ident("tick".into()),
            Value::Ident(edge.into()),
            Value::Number(i as f64),
        ],
        Some(points_placeholder(at)),
    )
}

fn zone(at: Span, edge: &str, i: usize, label: String) -> Node {
    let tail = Span::new(at.end, at.end);
    let mut n = chrome(
        at,
        "zone",
        vec![
            Value::Ident("zone".into()),
            Value::Ident(edge.into()),
            Value::Number(i as f64),
        ],
        None,
    );
    n.label = Some(TextNode {
        text: label,
        style: Vec::new(),
        style_span: None,
        span: tail,
    });
    n
}

/// A `|line|` chrome child needs `points:` to lay out at all — a degenerate
/// placeholder the page's finish overwrites.
fn points_placeholder(at: Span) -> Decl {
    let tail = Span::new(at.end, at.end);
    Decl {
        name: "points".into(),
        groups: vec![
            vec![Value::Number(0.0), Value::Number(0.0)],
            vec![Value::Number(0.0), Value::Number(0.0)],
        ],
        span: tail,
    }
}

fn chrome(at: Span, ty: &str, marker: Vec<Value>, extra: Option<Decl>) -> Node {
    let tail = Span::new(at.end, at.end);
    let mut style = vec![
        Decl {
            name: "chrome".into(),
            groups: vec![marker],
            span: tail,
        },
        Decl {
            name: "pin".into(),
            groups: vec![vec![Value::Ident("center".into())]],
            span: tail,
        },
    ];
    style.extend(extra);
    super::chrome::node(ty, style, tail)
}

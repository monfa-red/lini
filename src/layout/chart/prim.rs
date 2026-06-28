//! `PlacedNode` builders for a chart's lowered primitives ([CHARTS.md] §15). Every
//! bar, gridline, tick, label, swatch, and title is built through these — never an
//! open-coded `PlacedNode` — so the lowering stays one mechanism and the render
//! emitters (`emit_rect` / `emit_line` / the text path) draw them unchanged.

use crate::layout::{Bbox, PlacedNode, approx_height, approx_width};
use crate::resolve::{AttrMap, Markers, NodeKind, ResolvedValue};
use crate::span::Span;

fn node(kind: NodeKind, bbox: Bbox) -> PlacedNode {
    PlacedNode {
        id: None,
        kind,
        type_chain: Vec::new(),
        applied_styles: Vec::new(),
        label: None,
        attrs: AttrMap::new(),
        own_style: AttrMap::new(),
        markers: Markers::default(),
        cx: 0.0,
        cy: 0.0,
        bbox,
        rotation: 0.0,
        children: Vec::new(),
        dividers: Vec::new(),
        span: Span::empty(),
    }
}

fn ident(s: &str) -> ResolvedValue {
    ResolvedValue::Ident(s.to_string())
}

/// A filled rectangle (a bar or a legend swatch) centred at (cx, cy). Stroke off
/// and width 0 so the drawn rect matches the bbox exactly.
pub fn rect(cx: f64, cy: f64, w: f64, h: f64, fill: ResolvedValue) -> PlacedNode {
    let mut n = node(NodeKind::Block, Bbox::centered(w, h));
    n.cx = cx;
    n.cy = cy;
    n.attrs.insert("fill", fill);
    n.attrs.insert("stroke", ident("none"));
    n.attrs.insert("stroke-width", ResolvedValue::Number(0.0));
    n
}

/// The bounding box of a point list (empty for no points).
fn bounds(points: &[(f64, f64)]) -> Bbox {
    if points.is_empty() {
        return Bbox::empty();
    }
    points.iter().fold(
        Bbox {
            min_x: f64::INFINITY,
            min_y: f64::INFINITY,
            max_x: f64::NEG_INFINITY,
            max_y: f64::NEG_INFINITY,
        },
        |b, &(x, y)| Bbox {
            min_x: b.min_x.min(x),
            min_y: b.min_y.min(y),
            max_x: b.max_x.max(x),
            max_y: b.max_y.max(y),
        },
    )
}

/// A filled ellipse (a dot, a line-vertex marker) centred at (cx, cy). Stroke off
/// and width 0 so the drawn ellipse matches the bbox exactly.
pub fn oval(cx: f64, cy: f64, w: f64, h: f64, fill: ResolvedValue) -> PlacedNode {
    let mut n = node(NodeKind::Oval, Bbox::centered(w, h));
    n.cx = cx;
    n.cy = cy;
    n.attrs.insert("fill", fill);
    n.attrs.insert("stroke", ident("none"));
    n.attrs.insert("stroke-width", ResolvedValue::Number(0.0));
    n
}

/// A filled polygon (an area's body) through `points`. Stroke off; `opacity` lets
/// overlapping areas read through.
pub fn poly(points: Vec<(f64, f64)>, fill: ResolvedValue, opacity: f64) -> PlacedNode {
    let bbox = bounds(&points);
    let pts = points
        .into_iter()
        .map(|(x, y)| {
            ResolvedValue::Tuple(vec![ResolvedValue::Number(x), ResolvedValue::Number(y)])
        })
        .collect();
    let mut n = node(NodeKind::Poly, bbox);
    n.attrs.insert("points", ResolvedValue::List(pts));
    n.attrs.insert("fill", fill);
    n.attrs.insert("stroke", ident("none"));
    n.attrs.insert("stroke-width", ResolvedValue::Number(0.0));
    n.attrs.insert("opacity", ResolvedValue::Number(opacity));
    n
}

/// A polyline (a gridline or a line series) through `points`, with the given stroke
/// colour and width.
pub fn line(points: Vec<(f64, f64)>, stroke: ResolvedValue, width: f64) -> PlacedNode {
    let bbox = bounds(&points);
    let pts = points
        .into_iter()
        .map(|(x, y)| {
            ResolvedValue::Tuple(vec![ResolvedValue::Number(x), ResolvedValue::Number(y)])
        })
        .collect();
    let mut n = node(NodeKind::Line, bbox);
    n.attrs.insert("points", ResolvedValue::List(pts));
    n.attrs.insert("fill", ident("none"));
    n.attrs.insert("stroke", stroke);
    n.attrs.insert("stroke-width", ResolvedValue::Number(width));
    n
}

/// Centred text at (cx, cy) — anchor middle, the `.lini-text` default. `size` (and
/// `bold` / `color`) ride `own_style`, so they appear in the output, overriding the
/// chart `<g>`'s inherited root font.
pub fn text(
    content: &str,
    cx: f64,
    cy: f64,
    size: f64,
    color: Option<ResolvedValue>,
    bold: bool,
) -> PlacedNode {
    let bbox = Bbox::centered(
        approx_width(content, size, 0.0),
        approx_height(content, size, 0.0),
    );
    let mut n = node(NodeKind::Text, bbox);
    n.cx = cx;
    n.cy = cy;
    n.label = Some(content.to_string());
    set(&mut n, "font-size", ResolvedValue::Number(size));
    if bold {
        set(&mut n, "font-weight", ident("bold"));
    }
    if let Some(c) = color {
        set(&mut n, "color", c);
    }
    n
}

/// Text whose **right edge** sits at `right_x` (for value-axis labels): the node is
/// anchored middle, so shift its centre left by half the measured width.
pub fn text_right(
    content: &str,
    right_x: f64,
    cy: f64,
    size: f64,
    color: Option<ResolvedValue>,
) -> PlacedNode {
    let cx = right_x - text_width(content, size) / 2.0;
    text(content, cx, cy, size, color, false)
}

/// Text whose **left edge** sits at `left_x` (for a right-side value axis).
pub fn text_left(
    content: &str,
    left_x: f64,
    cy: f64,
    size: f64,
    color: Option<ResolvedValue>,
) -> PlacedNode {
    let cx = left_x + text_width(content, size) / 2.0;
    text(content, cx, cy, size, color, false)
}

/// The drawn width of a centred label, for laying out legends and right-aligned
/// ticks (compile-time text measurement, SPEC §6).
pub fn text_width(content: &str, size: f64) -> f64 {
    approx_width(content, size, 0.0)
}

/// Attach a native `<title>` — the baked-safe tooltip floor ([CHARTS.md] §14),
/// emitted by `render_node` on any node carrying a `title:`.
pub fn set_title(n: &mut PlacedNode, title: String) {
    n.attrs.insert("title", ResolvedValue::String(title));
}

/// Set a text prop on both `attrs` (so layout measures with it) and `own_style` (so
/// render emits it, beating the inherited `.lini` value).
fn set(n: &mut PlacedNode, name: &str, v: ResolvedValue) {
    n.attrs.insert(name, v.clone());
    n.own_style.insert(name, v);
}

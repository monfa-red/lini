//! `PlacedNode` **builders** for a layout engine's lowered primitives — shared by
//! charts [SPEC 14.9] and sequences [SPEC 13]. Every bar, gridline, lifeline,
//! arrow, label, frame, and note is built through these — never an open-coded
//! `PlacedNode` — so lowering stays one mechanism and the render emitters
//! (`emit_rect` / `emit_line` / the text path) draw them unchanged. (Distinct from
//! `layout::primitives`, which *sizes* primitives.)

use crate::layout::{Bbox, PlacedNode, approx_height, approx_width};
use crate::resolve::{AttrMap, MarkerKind, Markers, NodeKind, ResolvedInst, ResolvedValue};
use crate::span::Span;
use std::f64::consts::TAU;

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
        gutters: Vec::new(),
        links: Vec::new(),
        sketch: None,
        origin: (0.0, 0.0),
        span: Span::empty(),
    }
}

fn ident(s: &str) -> ResolvedValue {
    ResolvedValue::Ident(s.to_string())
}

/// A filled rectangle (a bar, a band shade, a legend swatch) centred at (cx, cy).
/// Stroke off and width 0 so the drawn rect matches the bbox exactly; `opacity` lets a
/// band wash or an overlay bar read through (omitted at 1, so opaque rects don't churn).
pub fn rect(cx: f64, cy: f64, w: f64, h: f64, fill: ResolvedValue, opacity: f64) -> PlacedNode {
    let mut n = node(NodeKind::Block, Bbox::centered(w, h));
    n.cx = cx;
    n.cy = cy;
    n.attrs.insert("fill", fill);
    n.attrs.insert("stroke", ident("none"));
    n.attrs.insert("stroke-width", ResolvedValue::Number(0.0));
    if (opacity - 1.0).abs() > 1e-9 {
        n.attrs.insert("opacity", ResolvedValue::Number(opacity));
    }
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

/// A `w`×`h` filled primitive centred at (cx, cy), stroke off and width 0 so the drawn
/// shape matches the bbox exactly. The shared body of [`oval`] and [`marker`].
fn filled(kind: NodeKind, cx: f64, cy: f64, w: f64, h: f64, fill: ResolvedValue) -> PlacedNode {
    let mut n = node(kind, Bbox::centered(w, h));
    n.cx = cx;
    n.cy = cy;
    n.attrs.insert("fill", fill);
    n.attrs.insert("stroke", ident("none"));
    n.attrs.insert("stroke-width", ResolvedValue::Number(0.0));
    n
}

/// A filled ellipse (a dot, a bubble) centred at (cx, cy).
pub fn oval(cx: f64, cy: f64, w: f64, h: f64, fill: ResolvedValue) -> PlacedNode {
    filled(NodeKind::Oval, cx, cy, w, h, fill)
}

/// A centred chart point marker [SPEC 14.2]: a filled round point (`dot` / `circle`)
/// or a rhombus (`diamond`), `w`×`h` at (cx, cy). The kind picks the shape; the caller
/// sizes it (a line vertex by the kind, a `|dots|` by its `width`). The **one** place a
/// chart point marker is built — line/area vertices, `|dots|`, and `|mark|` points all
/// route through it, so dot/circle/diamond never diverge. `arrow` / `crow` never reach
/// here (rejected at parse, [SPEC 20]); any non-diamond draws round.
pub fn marker(
    kind: MarkerKind,
    cx: f64,
    cy: f64,
    w: f64,
    h: f64,
    fill: ResolvedValue,
) -> PlacedNode {
    let shape = if matches!(kind, MarkerKind::Diamond) {
        NodeKind::Diamond
    } else {
        NodeKind::Oval
    };
    filled(shape, cx, cy, w, h, fill)
}

/// A drawing annotation's filled marker head [SPEC 15.6/15.7] — the slender
/// dimension arrow, the surface-seated datum triangle — classed
/// `lini-marker lini-marker-{variant}`, so the shared `.lini-marker` rule
/// paints it (fill = the link stroke, stroke off) and only a recoloured
/// statement inlines anything — exactly how link markers ride the sheet
/// [SPEC 17].
pub fn dim_marker(variant: &str, points: Vec<(f64, f64)>, fill: ResolvedValue) -> PlacedNode {
    let bbox = bounds(&points);
    let pts = points
        .into_iter()
        .map(|(x, y)| {
            ResolvedValue::Tuple(vec![ResolvedValue::Number(x), ResolvedValue::Number(y)])
        })
        .collect();
    let mut n = node(NodeKind::Poly, bbox);
    n.type_chain = vec!["marker".into(), format!("marker-{variant}")];
    n.attrs.insert("points", ResolvedValue::List(pts));
    n.attrs.insert("fill", fill);
    n.attrs.insert("stroke", ident("none"));
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

/// An annular-sector filled polygon (a pie / donut slice — [SPEC 14.7] — or a
/// radial bar, [SPEC 12]): radius `r0`→`r1` over angles `[a_lo, a_hi]` (0 straight up,
/// increasing clockwise). The arcs are segmented finely enough to read smooth at any
/// size; `r0 ≈ 0` collapses the inner edge to the centre (a full wedge). `opacity` lets
/// overlapping wedges read through.
#[allow(clippy::too_many_arguments)]
pub fn wedge(
    cx: f64,
    cy: f64,
    r0: f64,
    r1: f64,
    a_lo: f64,
    a_hi: f64,
    fill: ResolvedValue,
    opacity: f64,
) -> PlacedNode {
    let span = a_hi - a_lo;
    let steps = (span.abs() / TAU * 64.0).ceil().max(2.0) as usize; // ~one segment / 6°
    let at = |r: f64, a: f64| (cx + r * a.sin(), cy - r * a.cos());
    let mut pts: Vec<(f64, f64)> = (0..=steps)
        .map(|k| at(r1, a_lo + span * k as f64 / steps as f64))
        .collect();
    if r0 <= 0.5 {
        pts.push((cx, cy));
    } else {
        pts.extend((0..=steps).map(|k| at(r0, a_hi - span * k as f64 / steps as f64)));
    }
    poly(pts, fill, opacity)
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

/// A filled shape through a raw SVG path `d` (absolute coords baked in, like [`line`] /
/// [`poly`]; the node stays at the origin). For a composed outline a `rect` / `poly`
/// can't state — a frame's banner tab or a note's folded corner [SPEC 13]. `bbox` is
/// its absolute bounds, so the enclosing engine sizes correctly.
pub fn path(d: String, fill: ResolvedValue, bbox: Bbox) -> PlacedNode {
    let mut n = node(NodeKind::Path, bbox);
    n.attrs.insert("path", ResolvedValue::String(d));
    n.attrs.insert("fill", fill);
    n.attrs.insert("stroke", ident("none"));
    n.attrs.insert("stroke-width", ResolvedValue::Number(0.0));
    n
}

/// A transparent group wrapping `children` (e.g. a tooltip card's box + text), tagged
/// with `type_chain` so it carries `.lini-{name}` classes. `bbox` is its absolute bounds
/// (the children are positioned absolutely; the group keeps `cx`/`cy` at the origin).
pub fn group(children: Vec<PlacedNode>, type_chain: Vec<String>, bbox: Bbox) -> PlacedNode {
    let mut n = node(NodeKind::Block, bbox);
    n.type_chain = type_chain;
    n.children = children;
    n
}

/// The container shell for a **layout-owning engine** (chart, pie, sequence): a `Block`
/// carrying the node's identity, classes, and paint, with the lowered primitives as its
/// pre-positioned `children`. The one place that shell is built, so the engines don't each
/// open-code a `PlacedNode`.
pub fn container(inst: &ResolvedInst, bbox: Bbox, children: Vec<PlacedNode>) -> PlacedNode {
    PlacedNode {
        id: inst.id.clone(),
        kind: NodeKind::Block,
        type_chain: inst.type_chain.clone(),
        applied_styles: inst.applied_styles.clone(),
        label: None,
        attrs: inst.attrs.clone(),
        own_style: AttrMap::new(),
        markers: inst.markers.clone(),
        cx: 0.0,
        cy: 0.0,
        bbox,
        rotation: inst.attrs.number("rotate").unwrap_or(0.0),
        children,
        gutters: Vec::new(),
        links: Vec::new(),
        sketch: None,
        origin: (0.0, 0.0),
        span: inst.span,
    }
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
    // The diagram-wide default weight is bold (`--lini-font-weight`); a chart keeps that
    // for the title and legend (`bold`) but states `normal` for its data text — axis
    // ticks, tags, annotations — so the numbers and labels don't shout [SPEC 14.6].
    set(
        &mut n,
        "font-weight",
        ident(if bold { "bold" } else { "normal" }),
    );
    if let Some(c) = color {
        set(&mut n, "color", c);
    }
    n
}

/// Text whose size / weight come from a `.lini-{class}` **stylesheet rule** rather than
/// inline `style=` — for a sequence message label and the like, mirroring how a link label
/// rides `.lini-link-label`. `size` only bounds the bbox (so the engine measures the label);
/// the class states the rendered font. Centred at (cx, cy); `.lini-text` gives the anchors.
pub fn text_classed(content: &str, cx: f64, cy: f64, size: f64, class: &str) -> PlacedNode {
    let bbox = Bbox::centered(
        approx_width(content, size, 0.0),
        approx_height(content, size, 0.0),
    );
    let mut n = node(NodeKind::Text, bbox);
    n.cx = cx;
    n.cy = cy;
    n.label = Some(content.to_string());
    n.type_chain = vec![class.to_string()];
    n
}

/// The caption size a drawing's annotation text reads at [SPEC 15.1] — the
/// `.lini-dim-text` class states it, so no dim / leader / callout leaf inlines it.
const DIM_TEXT_SIZE: f64 = 12.0;

/// Dimension / leader / callout text [SPEC 15.6/17]: a `.lini-dim-text` leaf.
/// The class states the font (12 px, normal weight), so a leaf at that default
/// inlines nothing; only a size that differs — a `tol:` deviation stack, a
/// restyled link — carries an inline `font-size` override (a statement's own
/// text styling still inlines, [SPEC 17]).
pub fn dim_text(content: &str, cx: f64, cy: f64, size: f64) -> PlacedNode {
    let mut n = text_classed(content, cx, cy, size, "dim-text");
    if (size - DIM_TEXT_SIZE).abs() > 1e-9 {
        set(&mut n, "font-size", ResolvedValue::Number(size));
    }
    n
}

/// A plain text leaf that **inherits** its font from the enclosing `<g>` — no
/// class, no inline [SPEC 17]. For text under a box that already states the font
/// (a title `|footnote|`), so nothing is stated twice. `size` bounds the bbox.
pub fn text_plain(content: &str, cx: f64, cy: f64, size: f64) -> PlacedNode {
    let bbox = Bbox::centered(
        approx_width(content, size, 0.0),
        approx_height(content, size, 0.0),
    );
    let mut n = node(NodeKind::Text, bbox);
    n.cx = cx;
    n.cy = cy;
    n.label = Some(content.to_string());
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
/// ticks (compile-time text measurement, [SPEC 5]).
pub fn text_width(content: &str, size: f64) -> f64 {
    approx_width(content, size, 0.0)
}

/// The drawn height of a label, for collision-testing inline labels [SPEC 14.8].
pub fn text_height(content: &str, size: f64) -> f64 {
    approx_height(content, size, 0.0)
}

/// Attach a native `<title>` — the baked-safe tooltip floor [SPEC 14.8],
/// emitted by `render_node` on any node carrying a `hint:`.
pub fn set_hint(n: &mut PlacedNode, hint: String) {
    n.attrs.insert("hint", ResolvedValue::String(hint));
}

/// Draw a `stroke:` outline on a fill shape [SPEC 14.6]: replace the builder's
/// `stroke: none` / width-0 default with `color` at `width`. The one place a chart
/// shape gains an outline — reused by bars, slices, and bubbles (an area's outline is
/// its own top edge, a `prim::line`), so `stroke:` never bleeds into the fill.
pub fn outline(n: &mut PlacedNode, color: ResolvedValue, width: f64) {
    n.attrs.insert("stroke", color);
    n.attrs.insert("stroke-width", ResolvedValue::Number(width));
}

/// Round a rect's corners [SPEC 14.2]: set the `radius` the `Block` renderer reads
/// (`emit_rect` → `rx`/`ry`). Skipped at 0 so a square shape's attrs don't churn. Shared
/// by bars, the legend swatches, and the tooltip card.
pub fn round(n: &mut PlacedNode, radius: f64) {
    if radius > 0.0 {
        n.attrs.insert("radius", ResolvedValue::Number(radius));
    }
}

/// Set a text prop on both `attrs` (so layout measures with it) and `own_style` (so
/// render emits it, beating the inherited `.lini` value).
fn set(n: &mut PlacedNode, name: &str, v: ResolvedValue) {
    n.attrs.insert(name, v.clone());
    n.own_style.insert(name, v);
}

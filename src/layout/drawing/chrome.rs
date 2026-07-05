//! Generated drawing chrome [SPEC 15.7] — the lines drafting always draws.
//! Desugar generates them as real children (so the cascade styles or removes
//! them: `|sketch| |centerline| { stroke: none }`), each carrying a `chrome:`
//! marker instead of geometry:
//!
//! | `chrome:` | On | Geometry |
//! |---|---|---|
//! | `x-axis` / `y-axis` / a bearing | `\|centerline\|` | the axis line through the parent's datum, spanning its geometry + overhang |
//! | `ring` | `\|pitch-circle\|` | the circle through a radial pattern's copies — sized by `pattern::expand` |
//!
//! The parent's shape decides the geometry, so a chrome child lays out as an
//! empty placeholder and is filled here, after the parent is sized.

use super::super::ir::{Bbox, PlacedNode};
use super::geometry::bearing_dir;
use crate::resolve::{AttrMap, ResolvedInst, ResolvedValue};

/// Centre marks and auto centerlines overhang the geometry they mark by this
/// sheet-space constant [SPEC 10.5] — never scaled.
const OVERHANG: f64 = 4.0;

/// Whether a node is generated chrome — it carries the `chrome:` marker.
pub(crate) fn is_chrome(attrs: &AttrMap) -> bool {
    attrs.get("chrome").is_some()
}

/// A chrome child before its parent is sized: identity and paint, no geometry.
pub(in crate::layout) fn placeholder(inst: &ResolvedInst) -> PlacedNode {
    PlacedNode {
        id: inst.id.clone(),
        kind: inst.kind,
        type_chain: inst.type_chain.clone(),
        applied_styles: inst.applied_styles.clone(),
        label: None,
        attrs: inst.attrs.clone(),
        own_style: inst.own_style.clone(),
        markers: inst.markers.clone(),
        cx: 0.0,
        cy: 0.0,
        bbox: Bbox::empty(),
        rotation: 0.0,
        children: Vec::new(),
        gutters: Vec::new(),
        links: Vec::new(),
        sketch: None,
        span: inst.span,
    }
}

/// Fill the axis chrome among a part's children from its **geometry** box
/// (stroke excluded, part-local): the line runs through the datum along the
/// axis, past the geometry's projection by the overhang. `ring` chrome is
/// sized by the pattern expansion instead.
pub(in crate::layout) fn fill(children: &mut [PlacedNode], geometry: Bbox) {
    for c in children.iter_mut() {
        let bearing = match c.attrs.get("chrome") {
            Some(ResolvedValue::Ident(k)) if k == "x-axis" => 90.0,
            Some(ResolvedValue::Ident(k)) if k == "y-axis" => 0.0,
            Some(ResolvedValue::Number(b)) => *b,
            _ => continue,
        };
        let d = bearing_dir(bearing);
        let corners = [
            (geometry.min_x, geometry.min_y),
            (geometry.max_x, geometry.min_y),
            (geometry.min_x, geometry.max_y),
            (geometry.max_x, geometry.max_y),
        ];
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for (x, y) in corners {
            let t = x * d.0 + y * d.1;
            lo = lo.min(t);
            hi = hi.max(t);
        }
        let (lo, hi) = (lo - OVERHANG, hi + OVERHANG);
        let (a, b) = ((d.0 * lo, d.1 * lo), (d.0 * hi, d.1 * hi));
        let point = |p: (f64, f64)| {
            ResolvedValue::Tuple(vec![ResolvedValue::Number(p.0), ResolvedValue::Number(p.1)])
        };
        c.attrs
            .insert("points", ResolvedValue::List(vec![point(a), point(b)]));
        let half = c.attrs.number("stroke-width").unwrap_or(0.0) / 2.0;
        c.bbox = Bbox {
            min_x: a.0.min(b.0),
            min_y: a.1.min(b.1),
            max_x: a.0.max(b.0),
            max_y: a.1.max(b.1),
        }
        .inflate(half);
    }
}

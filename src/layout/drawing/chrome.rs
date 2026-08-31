//! Generated drawing chrome [SPEC 15.7] — the lines drafting always draws.
//! Desugar generates them as real children (so the cascade styles or removes
//! them: `|sketch| |centerline| { stroke: none }`), each carrying a `chrome:`
//! marker instead of geometry:
//!
//! | `chrome:` | On | Geometry |
//! |---|---|---|
//! | `x-axis` / `y-axis` / a bearing | `\|centerline\|` | the axis line through the parent's datum, spanning its geometry + overhang |
//! | `ring` | `\|pitch-circle\|` | the circle through a radial pattern's copies — sized by `pattern::expand` |
//! | `(kind, index)` | the floorplan's leaves, sills, risers | the parent fills the `index`-th piece of that `kind` ([`indexed`], [SPEC 15.11]) |
//!
//! The parent's shape decides the geometry, so a chrome child lays out as an
//! empty placeholder and is filled here, after the parent is sized.
//!
//! **The count is the author's, the geometry the cascade's.** Desugar reads the
//! **authored** decls to decide *how many* chrome children a producer emits,
//! while the layout fills them from the resolved cascade — so a `pattern:`, a
//! `break:`, a door's `symbol:` or a flight's `steps:` that reaches a node only
//! through a **rule** sizes the shape but generates no chrome. The limit is the
//! mechanism's, shared by every producer; a filler must therefore never
//! re-derive a *count* from an attribute the cascade may have moved — it reads
//! the children it was actually given.

use super::super::ir::{Bbox, PlacedNode};
use super::geometry::bearing_dir;
use crate::ledger::consts::{CENTER_MARK_OVERHANG, THREAD_DEPTH, THREAD_DEPTH_INTERNAL};
use crate::resolve::{AttrMap, ResolvedInst, ResolvedValue};

/// Whether a node is generated chrome — it carries the `chrome:` marker.
pub(crate) fn is_chrome(attrs: &AttrMap) -> bool {
    attrs.get("chrome").is_some()
}

/// The **indexed** marker's `(kind, index)` — the shape `desugar::drawing`
/// writes for a producer that emits several pieces at once (an opening's
/// leaves, a window's sills, a flight's risers), each filled from its parent's
/// own frame. The kind is copied out so the caller can go straight on to fill
/// the child it read it from.
pub(in crate::layout) fn indexed(attrs: &AttrMap) -> Option<(String, f64)> {
    match attrs.get("chrome")? {
        ResolvedValue::Tuple(items) => match items.as_slice() {
            [ResolvedValue::Ident(kind), ResolvedValue::Number(i)] => Some((kind.clone(), *i)),
            _ => None,
        },
        _ => None,
    }
}

/// A chrome child before its parent is sized: identity and paint, no geometry.
pub(in crate::layout) fn placeholder(inst: &ResolvedInst) -> PlacedNode {
    PlacedNode {
        id: inst.id.clone(),
        kind: inst.kind,
        type_chain: inst.type_chain.clone(),
        applied_styles: inst.applied_styles.clone(),
        // A |plane| carries its section letter here [SPEC 15.8]; other
        // chrome has no label, so this is `None` for them.
        label: inst.label.clone(),
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
        origin: (0.0, 0.0),
        span: inst.span,
    }
}

/// Fill the axis chrome among a part's children from its **geometry** box
/// (stroke excluded, part-local): the line runs through the datum along the
/// axis, past the geometry's projection by the overhang. `ring` chrome is
/// sized by the pattern expansion instead; a round feature's `thread:` ¾ arc
/// is drawn here from the drawn width + the pitch ([SPEC 15.4]) — `scale` is
/// the part's own px per drawing unit.
pub(in crate::layout) fn fill(children: &mut [PlacedNode], geometry: Bbox, scale: f64) {
    for c in children.iter_mut() {
        if let Some(ResolvedValue::Tuple(items)) = c.attrs.get("chrome")
            && let [
                ResolvedValue::Ident(k),
                ResolvedValue::Ident(sense),
                ResolvedValue::Number(pitch),
            ] = items.as_slice()
            && k == "thread-arc"
        {
            thread_arc(c, geometry, sense == "internal", *pitch, scale);
            continue;
        }
        let bearing = match c.attrs.get("chrome") {
            Some(ResolvedValue::Ident(k)) if k == "x-axis" => 90.0,
            Some(ResolvedValue::Ident(k)) if k == "y-axis" => 0.0,
            Some(ResolvedValue::Number(b)) => *b,
            _ => continue,
        };
        let d = bearing_dir(bearing);
        let (lo, hi) = super::geometry::project(geometry, d);
        let (lo, hi) = (lo - CENTER_MARK_OVERHANG, hi + CENTER_MARK_OVERHANG);
        let (a, b) = ((d.0 * lo, d.1 * lo), (d.0 * hi, d.1 * hi));
        let point = |p: (f64, f64)| {
            ResolvedValue::Tuple(vec![ResolvedValue::Number(p.0), ResolvedValue::Number(p.1)])
        };
        c.attrs
            .insert("points", ResolvedValue::List(vec![point(a), point(b)]));
        let half = super::half_stroke(&c.attrs);
        c.bbox = Bbox::from_points(&[a, b]).inflate(half);
    }
}

/// The ISO 6410 thread circle [SPEC 15.4]: a thin ¾ arc, its gap over the
/// upper-right quadrant — outside the drilled bore at the major ⌀ on an
/// internal thread (`width + 1.0825 × pitch`), inside the outline at the
/// minor on an external one (`width − 1.2269 × pitch`). Drawn as a `|path|`
/// (a `|line|` can't arc — the kind flips, the old S-break play).
fn thread_arc(c: &mut PlacedNode, geometry: Bbox, internal: bool, pitch: f64, scale: f64) {
    let r_drawn = geometry.w() / 2.0;
    let r = if internal {
        r_drawn + THREAD_DEPTH_INTERNAL * pitch * scale
    } else {
        r_drawn - THREAD_DEPTH * pitch * scale
    };
    if r <= 0.0 {
        return;
    }
    let n = super::geometry::n;
    // From the east rim, sweeping clockwise through south and west to the
    // north rim — the gap spans the upper-right quadrant.
    c.attrs.insert(
        "path",
        crate::resolve::ResolvedValue::String(format!(
            "M {} 0 A {} {} 0 1 1 0 {}",
            n(r),
            n(r),
            n(r),
            n(-r)
        )),
    );
    c.kind = crate::resolve::NodeKind::Path;
    let half = super::half_stroke(&c.attrs);
    c.bbox = Bbox::centered(2.0 * r, 2.0 * r).inflate(half);
}

/// **Removal, for every producer at once** [SPEC 15.7]: chrome the cascade
/// painted away — stroke *and* fill both `none` — surrenders the geometry its
/// producer gave it, so it draws nothing (`emit_line` needs two points, and
/// the shaped kinds need their `path`) and reserves no space, overhang
/// included. One sweep over the finished tree rather than a test inside each
/// of the twelve producers — some of which (a radial `pattern:`'s pitch
/// circle, a `revolve:`'s shoulders) generate after their parent is laid out,
/// so there is no earlier point that sees them all. A producer that forgot the
/// test would be the failure mode, and here there is nothing to forget.
pub(in crate::layout) fn drop_unpainted(nodes: &mut [PlacedNode]) {
    for n in nodes.iter_mut() {
        // Only chrome that draws *itself* — the linework conventions. A piece
        // that carries content (a `|zone|`'s border letter, a fixture's label)
        // paints nothing by nature rather than by the cascade's instruction,
        // and its box is what positions what it holds.
        if is_chrome(&n.attrs) && n.children.is_empty() && n.attrs.paints_nothing() {
            n.attrs.remove("points");
            n.attrs.remove("path");
            n.bbox = Bbox::empty();
        }
        drop_unpainted(&mut n.children);
    }
}

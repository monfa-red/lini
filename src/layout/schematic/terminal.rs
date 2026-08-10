//! Where a placed part's **terminals** are [SPEC 16.1/16.2]: the point a wire
//! lands on, and the way it faces.
//!
//! Every answer is read off what desugar built, in the part's own coordinates
//! — the pose is already structural, so nothing here turns anything:
//!
//! | Terminal | Point | Facing |
//! |---|---|---|
//! | a `\|component\|` pin | its stub's **tip** — the far end of the lead | the stub's own `pin:` side |
//! | a symbol part's pin | its zero-size port node | the registry port, posed |
//! | a `\|label\|` | the symbol's box edge midpoint | the registry port, posed |
//!
//! (The router's fixed ports land on exactly these points.)

use super::super::ir::{Bbox, PlacedNode};
use crate::desugar::pose::{Pose, Side};
use crate::desugar::schematic::{part_pin_ids, terminal_facing};
use crate::resolve::{AttrMap, ResolvedValue};

/// A wirable terminal, in its **part's** coordinates (the part's origin is
/// `(0, 0)`): where the wire lands, and the way the terminal points.
#[derive(Clone, Copy, Debug)]
pub(super) struct Terminal {
    pub at: (f64, f64),
    /// `None` for a terminal with no connection geometry — a symbol-less
    /// `|label|`, or a port at its own box's centre. Such a chain grows along
    /// the pin's outward normal instead [SPEC 16.1].
    pub facing: Option<Side>,
}

/// The terminal `path` names on `part` (`None` — a bare `- gnd1` — is the
/// part's own one connection point).
pub(super) fn terminal(part: &PlacedNode, path: Option<&str>) -> Terminal {
    let symbol = ident(&part.attrs, "symbol");
    let pose = Pose::of_chain(&part.type_chain);
    // The registry's answer, turned into the part's landed frame. A
    // `|component|` has no glyph, so its pins answer from the stub below.
    let posed = |s: Side| pose.side(s);
    let facing = terminal_facing(&part.type_chain, symbol.as_deref(), path).map(posed);

    // A named terminal is a real node in the lowered tree: a pin block (whose
    // stub carries the tip and the side) or a zero-size port node.
    if let Some(id) = path.or_else(|| {
        part_pin_ids(&part.type_chain, symbol.as_deref())
            .first()
            .copied()
    }) && let Some((node, at)) = descendant(part, id)
    {
        if let Some(stub) = child_wearing(node, "pin-stub") {
            return stub_tip(stub, at);
        }
        return Terminal { at, facing };
    }
    // No node to read — a `|label|`'s connection point is its symbol's, and
    // the symbol is drawn by its one `|path|` child. The wire meets the
    // **ink**: the paint bbox is deflated by the glyph's painted half-stroke,
    // or the landing floats a half-stroke off the drawing.
    let body = ["sch-tag-line", "sch-line"]
        .iter()
        .find_map(|k| child_wearing(part, k))
        .map_or(part.bbox, |c| {
            c.bbox.inflate(-c.attrs.half_stroke()).shifted(c.cx, c.cy)
        });
    Terminal {
        at: facing.map_or(body.center(), |s| edge_midpoint(body, s)),
        facing,
    }
}

/// A component pin's connection point: the far end of its stub, on the side
/// the stub points. The stub's `pin:` is written in the **landed** frame
/// [SPEC 16.1], so a posed part needs no turn here. The stub's paint bbox
/// overshoots its butt-capped line by the half-stroke on every side — deflate
/// it, so the wire lands on the lead's true endpoint, not past its paint.
fn stub_tip(stub: &PlacedNode, pin_at: (f64, f64)) -> Terminal {
    let side = ident(&stub.attrs, "pin")
        .as_deref()
        .and_then(Side::parse)
        .unwrap_or(Side::Left);
    let box_ = stub
        .bbox
        .inflate(-stub.attrs.half_stroke())
        .shifted(pin_at.0 + stub.cx, pin_at.1 + stub.cy);
    Terminal {
        at: edge_midpoint(box_, side),
        facing: Some(side),
    }
}

/// The midpoint of a box's `side` edge — where a terminal on that side sits.
fn edge_midpoint(b: Bbox, side: Side) -> (f64, f64) {
    let (cx, cy) = b.center();
    match side {
        Side::Left => (b.min_x, cy),
        Side::Right => (b.max_x, cy),
        Side::Top => (cx, b.min_y),
        Side::Bottom => (cx, b.max_y),
    }
}

/// The descendant with `id` and its offset from `part`'s origin. Ids are
/// unique within a part, and its rails are anonymous and scope-transparent
/// [SPEC 9] — so a plain subtree search is the same walk an endpoint path is.
fn descendant<'a>(part: &'a PlacedNode, id: &str) -> Option<(&'a PlacedNode, (f64, f64))> {
    fn walk<'a>(
        nodes: &'a [PlacedNode],
        id: &str,
        ox: f64,
        oy: f64,
    ) -> Option<(&'a PlacedNode, (f64, f64))> {
        for n in nodes {
            let at = (ox + n.cx, oy + n.cy);
            if n.id.as_deref() == Some(id) {
                return Some((n, at));
            }
            if let Some(hit) = walk(&n.children, id, at.0, at.1) {
                return Some(hit);
            }
        }
        None
    }
    walk(&part.children, id, 0.0, 0.0)
}

/// The generated chrome child wearing `lini-<kind>` — the one class desugar
/// stamps on it [SPEC 16.7/18].
fn child_wearing<'a>(node: &'a PlacedNode, kind: &str) -> Option<&'a PlacedNode> {
    node.children
        .iter()
        .find(|c| c.type_chain.iter().any(|t| t == kind))
}

pub(super) fn ident(attrs: &AttrMap, name: &str) -> Option<String> {
    match attrs.get(name) {
        Some(ResolvedValue::Ident(s)) => Some(s.clone()),
        _ => None,
    }
}

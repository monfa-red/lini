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
//! | a **net run** | the run's **far** end — the edge *opposite* the facing | [`NET_RUN_FACING`], posed |
//!
//! (The router's fixed ports land on exactly these points.)
//!
//! The net run is the one terminal whose point does **not** sit on the edge it
//! faces [SPEC 16.4]: the wire arrives from the facing side and travels the
//! whole box before it lands, which is what leaves the name sitting over a
//! trace. Its connection frame is that landing line alone
//! ([`super::ports::part_ports`]) — a run has no body.

use super::super::ir::{Bbox, PlacedNode};
use crate::desugar::pose::{Pose, Side};
use crate::desugar::schematic::{is_net_run, part_pin_ids, terminal_facing, terminal_ids};
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
    let shape = ident(&part.attrs, "shape");
    let pose = Pose::of_chain(&part.type_chain);
    // The registry's answer, turned into the part's landed frame. A
    // `|component|` has no glyph, so its pins answer from the stub below.
    let posed = |s: Side| pose.side(s);
    let facing =
        terminal_facing(&part.type_chain, symbol.as_deref(), shape.as_deref(), path).map(posed);
    // A **net run**: the landing is the run's far end, so the wire crosses the
    // whole box [SPEC 16.4].
    if is_net_run(&part.type_chain, symbol.as_deref(), shape.as_deref()) {
        let facing = facing.expect("a net run faces its wire");
        return Terminal {
            at: edge_midpoint(part.bbox, facing.opposite()),
            facing: Some(facing),
        };
    }

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
    // No node to read — a `|label|`'s connection point is its symbol's ([`body`]).
    let body = body(part).unwrap_or(part.bbox);
    Terminal {
        at: facing.map_or(body.center(), |s| edge_midpoint(body, s)),
        facing,
    }
}

/// The point the lattice holds a satellite by [SPEC 16.1] — the **centre of
/// its connection geometry**, never of its drawn box: the terminals a part
/// carries, the one connection point a label is, and the whole run of trace a
/// net run draws, whose box *is* the conductor [SPEC 16.4] — so a name still
/// stands centred on its slot rather than hung off one end of it.
///
/// For a symmetric two-terminal symbol the two readings are the same point, so
/// nothing moves; for a flag drawing its name beside its symbol they are half
/// a name apart, and it is the symbol's port that belongs on the wire's line.
pub(super) fn seat_point(part: &PlacedNode) -> (f64, f64) {
    if super::net::is_run(part) {
        return part.bbox.center();
    }
    let ports: Vec<(f64, f64)> = terminal_ids(part)
        .iter()
        .map(|id| terminal(part, id.as_deref()).at)
        .collect();
    // A `|label|` carries no terminal ids at all: it *is* its own terminal.
    if ports.is_empty() {
        return terminal(part, None).at;
    }
    let n = ports.len() as f64;
    let (x, y) = ports.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
    (x / n, y / n)
}

/// A part's **body** [SPEC 16.1] — the drawing its terminals belong to, in the
/// part's own frame: the symbol it wears, or the outline a shaped tag draws.
/// `None` for a part that draws none, which is the honest answer for a net run
/// — a stretch of trace with a name over it, and no body at all [SPEC 16.4].
///
/// Never the label's text beside its symbol, and never a part's ref / value
/// readouts: neither is a conductor, so neither may reserve a thing. The wire
/// meets the **ink** — the paint bbox deflated by the drawing's own painted
/// half-stroke, or the landing floats a half-stroke off the symbol.
pub(super) fn body(part: &PlacedNode) -> Option<Bbox> {
    ["sch-tag-line", "sch-line"]
        .iter()
        .find_map(|k| child_wearing(part, k))
        .map(|c| c.bbox.inflate(-c.attrs.half_stroke()).shifted(c.cx, c.cy))
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

//! The leader skeleton [SPEC 15.7]: the tip → elbow → landing line math —
//! exit direction, outline ray-cast, and the carried-block clearing push —
//! shared by every leader-shaped dispatch in `leaders`.

use super::super::super::ir::PlacedNode;
use super::super::anchors::Anchor;
use super::super::annotate::{Ctx, Rows};
use super::super::geometry::{P, dist};
use super::super::outline;
use super::super::symbols::CarriedStack;
use crate::layout::geom::dot;
use crate::layout::geom::rotate;
use crate::layout::geom::unit;
use crate::layout::stack::{Painted, clear_past};
use crate::ledger::consts::{NOTE_LANDING, NOTE_OFFSET};

/// A leader's drawn skeleton: tip → elbow → landing, plus where its text
/// starts (just past the landing, on the `sx` side) and the direction it
/// left the feature along.
pub(super) struct LeaderLine {
    pub points: Vec<P>,
    pub text_at: P,
    pub sx: f64,
    pub u: P,
}

/// Build the leader skeleton toward `aim` (world). The text direction is
/// `side:`'s; else a **directed** feature's surface normal (the leader
/// leaves a face straight off it, then the elbow — the drafting default); a
/// point feature's is the ray from the drawing's **datum** through it. The
/// text clears the geometry union by `NOTE_OFFSET` [SPEC 15.7]; `extra`
/// pushes the elbow farther out along the exit — a carrying statement's
/// stack clearing [SPEC 15.9]. The tip: `exact` lands as given (an arc's own
/// point); `circle` intersects analytically; otherwise the ray casts onto
/// the node's drawn outline.
pub(super) fn leader_line(
    ctx: &Ctx,
    anchor: &Anchor,
    aim: P,
    dir_override: Option<P>,
    exact: Option<P>,
    circle: Option<(P, f64)>,
    extra: f64,
) -> LeaderLine {
    let u = dir_override
        .or_else(|| {
            // The normal's axis comes from the surface; its sign points away
            // from the datum — an edge authored material-on-the-left reports
            // `outward` into the part.
            let n = anchor.outward()?;
            Some(if dot(n, aim) < 0.0 { (-n.0, -n.1) } else { n })
        })
        .unwrap_or_else(|| {
            let len = dist(aim, (0.0, 0.0));
            if len > 1e-6 {
                (aim.0 / len, aim.1 / len)
            } else {
                // A feature on the datum has no outward ray — drafting's
                // default leader runs up-right.
                let d = std::f64::consts::FRAC_1_SQRT_2;
                (d, -d)
            }
        });
    let t_exit = outline::exit_box(aim, u, ctx.extent);
    let elbow = (
        aim.0 + u.0 * (t_exit + NOTE_OFFSET + extra),
        aim.1 + u.1 * (t_exit + NOTE_OFFSET + extra),
    );
    let sx = if u.0 < 0.0 { -1.0 } else { 1.0 };
    let landing = (elbow.0 + sx * NOTE_LANDING, elbow.1);
    let tip = exact
        .or_else(|| circle_tip(circle, elbow))
        .unwrap_or_else(|| {
            // An elbow on the aim point casts no ray — the zero direction
            // raycasts nowhere and the aim itself is the tip, as before.
            let d = unit((aim.0 - elbow.0, aim.1 - elbow.1)).unwrap_or((0.0, 0.0));
            let o = anchor.to_local(elbow);
            match outline::raycast(anchor.node, o, rotate(d, -anchor.rot)) {
                Some(t) => (elbow.0 + d.0 * t, elbow.1 + d.1 * t),
                None => aim,
            }
        });
    LeaderLine {
        points: vec![tip, elbow, landing],
        text_at: (landing.0 + sx * 2.0, landing.1),
        sx,
        u,
    }
}

/// The extra push along the exit `dir` a **ray-leaving** annotation takes
/// before it paints — the one law the leaders and the diametral spill share
/// [SPEC 15.6/15.9]. Two clearings, in order: a carrying statement's whole
/// block (the text seat plus the carried stack's one measured box, which
/// hangs below and can reach back onto the part) stands `NOTE_OFFSET` off the
/// drawn geometry, then the block packs against everything already painted,
/// in source order. 0 when the deterministic placement already stands clear.
pub(in crate::layout::drawing) fn outward_push(
    nodes: &[PlacedNode],
    stack: &CarriedStack,
    dir: P,
    rows: &Rows,
    clearance: f64,
) -> f64 {
    let seat = super::super::symbols::seat_of(nodes);
    // An uncarried seat is already placed clear of the geometry by
    // construction — the exit ray left it by `NOTE_OFFSET`.
    let (block, past) = match stack.box_below(seat) {
        Some(below) => {
            let block = seat.union(below);
            let past = clear_past(
                &Painted::of_box(block),
                dir,
                &Painted::of_box(rows.extent()),
                NOTE_OFFSET,
            );
            (block, past)
        }
        None => (seat, 0.0),
    };
    past + rows.spill(dir, block.shifted(dir.0 * past, dir.1 * past), clearance)
}

/// The nearest rim point of an analytic circle toward the elbow.
pub(super) fn circle_tip(circle: Option<(P, f64)>, from: P) -> Option<P> {
    let (c, r) = circle?;
    // A `from` at the centre picks no rim point — the centre, as before.
    let u = unit((from.0 - c.0, from.1 - c.1)).unwrap_or((0.0, 0.0));
    Some((c.0 + u.0 * r, c.1 + u.1 * r))
}

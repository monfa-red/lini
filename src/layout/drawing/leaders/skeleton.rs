//! The leader skeleton [SPEC 15.7]: the tip → elbow → landing line math —
//! exit direction, outline ray-cast, and the carried-block clearing push —
//! shared by every leader-shaped dispatch in `leaders`.

use super::super::super::ir::{Bbox, PlacedNode};
use super::super::anchors::{Anchor, rotated};
use super::super::annotate::Ctx;
use super::super::geometry::{P, dist, unit};
use super::super::outline;
use super::super::symbols::CarriedStack;
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
            Some(if n.0 * aim.0 + n.1 * aim.1 < 0.0 {
                (-n.0, -n.1)
            } else {
                n
            })
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
            let d = unit((aim.0 - elbow.0, aim.1 - elbow.1));
            let o = anchor.to_local(elbow);
            match outline::raycast(anchor.node, o, rotated(d, -anchor.rot)) {
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

/// A carrying statement clears the geometry for its **whole block**
/// [SPEC 15.9]: the text seat plus the carried stack's one measured box must
/// stand `NOTE_OFFSET` off `obstacle` along `dir` — the extra push past the
/// uncarried placement; 0 when nothing is carried or it already stands clear.
pub(in crate::layout::drawing) fn carried_push(
    nodes: &[PlacedNode],
    stack: &CarriedStack,
    dir: P,
    obstacle: Bbox,
) -> f64 {
    let seat = super::super::symbols::seat_of(nodes);
    let Some(below) = stack.box_below(seat) else {
        return 0.0;
    };
    clear_along(seat.union(below), dir, obstacle, NOTE_OFFSET)
}

/// The distance along unit `dir` that carries `b` past `obstacle`, standing
/// `margin` off it — clearing either axis separates the boxes, so the
/// cheaper feasible axis wins; 0 when already clear.
fn clear_along(b: Bbox, dir: P, obstacle: Bbox, margin: f64) -> f64 {
    let o = obstacle.inflate(margin);
    if !b.overlaps(o) {
        return 0.0;
    }
    let past = |lo: f64, hi: f64, o_lo: f64, o_hi: f64, d: f64| {
        if d < -1e-9 {
            (hi - o_lo) / -d
        } else if d > 1e-9 {
            (o_hi - lo) / d
        } else {
            f64::INFINITY
        }
    };
    past(b.min_x, b.max_x, o.min_x, o.max_x, dir.0)
        .min(past(b.min_y, b.max_y, o.min_y, o.max_y, dir.1))
}

/// The nearest rim point of an analytic circle toward the elbow.
pub(super) fn circle_tip(circle: Option<(P, f64)>, from: P) -> Option<P> {
    let (c, r) = circle?;
    let u = unit((from.0 - c.0, from.1 - c.1));
    Some((c.0 + u.0 * r, c.1 + u.1 * r))
}

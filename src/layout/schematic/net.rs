//! The **net-label convention** [SPEC 16.4] — the one home for how a sheet
//! writes a net name against its wire.
//!
//! A diagram's link label rides its wire and the wire opens behind it
//! [SPEC 9]. A sheet does the opposite: the name stands a constant clear
//! distance *off* the trace, and the trace is never cut. Both spellings of a
//! net name go through the rules here, so they can never drift:
//!
//! | Written | Text stepped off by | Against the tangent of |
//! |---|---|---|
//! | `u7.vs - c24.p1 "VM"` — the wire's own label | the router's label pass ([`crate::routing::ortho::labels`]) | the drawn route at the `along:` anchor |
//! | `u7.en - "EN"` — the minted net run | the field pass ([`super::field`]) | the run's own axis |
//!
//! Three answers, shared by both: **which side** of the wire the text takes
//! ([`text_normal`]), **how far off** it sits ([`offset`]) and **which way it
//! reads** ([`text_turn`]) — upright over a horizontal wire, along a vertical
//! one, the way a dimension's value rides its line [SPEC 15.6]. Each caller
//! brings its own reading of "which way is freer" — a routed label measures the
//! scene's obstacles with [`clear_run`]; a seated run reads its field instead,
//! and steps outward, away from the anchor it hangs off [SPEC 16.4].

use super::super::ir::{Bbox, PlacedNode};
use super::super::primitives;
use crate::desugar::pose::{Pose, Side};
use crate::desugar::schematic::{NET_RUN_FACING, is_net_run};
use crate::error::Error;
use crate::layout::stack::Painted;
use crate::ledger::consts::{NET_LABEL_OFFSET, NET_LABEL_RUN};
use crate::resolve::{AttrMap, ResolvedValue};

/// How much clear space either side of a vertical run still counts as
/// "freer" [SPEC 16.4]: past a run's own length both sides are roomy and the
/// tie-break decides, so the measure caps here rather than letting a distant
/// body outvote a near one.
const ROOM_LIMIT: f64 = NET_LABEL_RUN;

/// Which way a net name steps off its wire [SPEC 16.4]: a **horizontal** run
/// carries it above; a **vertical** one beside, on the freer side — `room`
/// measuring the clear space each way. `forced` is the statement's own
/// `side:` and wins outright.
///
/// The tie-break is the routing contract's fixed side rank
/// (right → bottom → left → top), so equal room always reads right and the
/// same sheet renders byte-identically.
pub(crate) fn text_normal(
    tangent: (f64, f64),
    forced: Option<Side>,
    room: impl Fn(Side) -> f64,
) -> (f64, f64) {
    if let Some(side) = forced {
        return side.normal();
    }
    if tangent.0.abs() >= tangent.1.abs() {
        return Side::Top.normal();
    }
    let side = if room(Side::Right) >= room(Side::Left) {
        Side::Right
    } else {
        Side::Left
    };
    side.normal()
}

/// Which way a net name **reads** [SPEC 16.4], in the degrees `rotate:`
/// states: upright over a horizontal wire; along a vertical one, bottom to
/// top — ISO-aligned, read from the right as a dimension's value is
/// [SPEC 15.6] — which is a quarter turn widdershins.
pub(crate) fn text_turn(tangent: (f64, f64)) -> f64 {
    if tangent.0.abs() >= tangent.1.abs() {
        0.0
    } else {
        270.0
    }
}

/// A text box after [`text_turn`]: the extent a turned name really reaches,
/// which is what its offset and its clearance read. A quarter turn about the
/// box's centre swaps its two reaches.
pub(crate) fn turned(text: Bbox, turn: f64) -> Bbox {
    if turn == 0.0 {
        return text;
    }
    let (cx, cy) = text.center();
    Bbox::centered(text.h(), text.w()).shifted(cx, cy)
}

/// Turn every run stood on end to read along itself [SPEC 16.4], before the
/// field reads a box. The name takes the core's own turn — the `rotate:` any
/// text leaf wears, a paint transform whose extent the sheet measures — and
/// the run re-boxes around the turned name through the one sizing law, so it
/// is as long as its name exactly as a horizontal run is.
pub(super) fn turn_runs(children: &mut [PlacedNode]) -> Result<(), Error> {
    for run in children.iter_mut().filter(|c| is_run(c)) {
        let turn = text_turn(run_tangent(run));
        if turn == 0.0 {
            continue;
        }
        for c in run.children.iter_mut() {
            c.rotation = turn;
            c.attrs.insert("rotate", ResolvedValue::Number(turn));
        }
        let Some(content) = content_box(run) else {
            continue;
        };
        let (dx, dy) = content.center();
        for c in run.children.iter_mut() {
            c.cx -= dx;
            c.cy -= dy;
        }
        run.bbox = primitives::box_around(&run.attrs, run.span, content, 1.0)?;
    }
    Ok(())
}

/// The text's displacement from a point on the wire: [`NET_LABEL_OFFSET`] of
/// clear space plus half the text's own reach across the line, so the daylight
/// the reader sees is the constant whatever the name measures.
pub(crate) fn offset(normal: (f64, f64), text: Bbox) -> (f64, f64) {
    let across = if normal.0 == 0.0 { text.h() } else { text.w() };
    let d = NET_LABEL_OFFSET + across / 2.0;
    (normal.0 * d, normal.1 * d)
}

/// The clear space from `at` along the axis-aligned unit direction `dir`
/// before the first box in the way, capped at [`ROOM_LIMIT`] — the freer-side
/// measure [`text_normal`] reads. A ray cast, so a box the ray never crosses
/// costs nothing; a box already covering `at` reads **negative**, the depth
/// still to clear, so between two crowded sides the shallower one still wins.
pub(crate) fn clear_run(at: (f64, f64), dir: (f64, f64), painted: &[Painted]) -> f64 {
    let mut room = ROOM_LIMIT;
    // `dir` is an axis unit vector, so its one non-zero component is its sign.
    let sign = dir.0 + dir.1;
    for p in painted {
        let b = p.bounds();
        // The box's span along the ray, and its span across it.
        let ((lo, hi), across, span) = if dir.0 != 0.0 {
            ((b.min_x, b.max_x), at.1, (b.min_y, b.max_y))
        } else {
            ((b.min_y, b.max_y), at.0, (b.min_x, b.max_x))
        };
        if across < span.0 || across > span.1 {
            continue;
        }
        let from = if dir.0 != 0.0 { at.0 } else { at.1 };
        // The box as a ray interval: entirely behind and it costs nothing;
        // ahead and its near edge is the clear run; astride `at` and the far
        // edge is the depth still to clear, reported negative.
        let (t0, t1) = ((lo - from) * sign, (hi - from) * sign);
        let (near, far) = (t0.min(t1), t0.max(t1));
        if far <= 0.0 {
            continue;
        }
        room = room.min(if near > 0.0 { near } else { -far });
    }
    room
}

/// The `side:` a net label — or a sheet's wire — forces [SPEC 16.4/17].
pub(crate) fn forced_side(attrs: &AttrMap) -> Option<Side> {
    match attrs.get("side") {
        Some(ResolvedValue::Ident(s)) => Side::parse(s),
        _ => None,
    }
}

/// Whether a placed child is a **net run** [SPEC 16.4] — the same predicate
/// the lowering and the terminal reader ask, over a placed node's resolved
/// attrs.
pub(crate) fn is_run(node: &PlacedNode) -> bool {
    let ident = |name: &str| super::terminal::ident(&node.attrs, name);
    is_net_run(
        &node.type_chain,
        ident("symbol").as_deref(),
        ident("shape").as_deref(),
    )
}

/// A net run's own axis [SPEC 16.4] — the direction its wire arrives from,
/// which is the tangent of the trace its name sits beside.
pub(super) fn run_tangent(node: &PlacedNode) -> (f64, f64) {
    Pose::of_chain(&node.type_chain)
        .side(NET_RUN_FACING)
        .normal()
}

/// Where a seated run's name steps to [SPEC 16.4]: the displacement of every
/// child it carries (its text). A horizontal run carries it above; a vertical
/// one **outward** — `outward` is the side away from the anchor whose field the
/// run stands in, and `None` where no field holds it, which leaves the routing
/// side rank to decide.
pub(super) fn seat_text(run: &PlacedNode, outward: Option<Side>) -> (f64, f64) {
    let Some(text) = content_box(run) else {
        return (0.0, 0.0);
    };
    let normal = text_normal(run_tangent(run), forced_side(&run.attrs), |side| {
        f64::from(Some(side) == outward)
    });
    offset(normal, text)
}

/// A run's content box in its own frame — the union of what it carries, which
/// for a net run is its name, as it is **drawn**: turned, where the run stands
/// on end. `None` when it carries nothing.
fn content_box(run: &PlacedNode) -> Option<Bbox> {
    run.children
        .iter()
        .map(|c| Bbox::drawn_of(c).shifted(c.cx, c.cy))
        .reduce(|a, b| a.union(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box_(x0: f64, y0: f64, x1: f64, y1: f64) -> Painted {
        Painted::of_box(Bbox {
            min_x: x0,
            min_y: y0,
            max_x: x1,
            max_y: y1,
        })
    }

    #[test]
    fn a_horizontal_run_carries_its_name_above() {
        // [SPEC 16.4] — and no room is even consulted: a horizontal trace has
        // one conventional side.
        for tangent in [(1.0, 0.0), (-1.0, 0.0)] {
            let n = text_normal(tangent, None, |_| panic!("no reading needed"));
            assert_eq!(n, (0.0, -1.0), "{tangent:?}");
        }
    }

    #[test]
    fn a_vertical_run_takes_the_freer_side() {
        // A body 5 to the right, nothing to the left: the name goes left.
        let right = [box_(5.0, -50.0, 60.0, 50.0)];
        let n = text_normal((0.0, 1.0), None, |s| {
            clear_run((0.0, 0.0), s.normal(), &right)
        });
        assert_eq!(n, (-1.0, 0.0));
        // Mirrored, it goes right; with equal room the tie breaks on the
        // routing side rank, which leads with `right`.
        let left = [box_(-60.0, -50.0, -5.0, 50.0)];
        assert_eq!(
            text_normal((0.0, 1.0), None, |s| clear_run(
                (0.0, 0.0),
                s.normal(),
                &left
            )),
            (1.0, 0.0)
        );
        assert_eq!(
            text_normal((0.0, 1.0), None, |s| clear_run((0.0, 0.0), s.normal(), &[])),
            (1.0, 0.0)
        );
    }

    #[test]
    fn a_forced_side_wins_outright() {
        // The statement's own `side:` [SPEC 16.4/17] — read on the label for a
        // minted run, on the wire for a two-ended net name.
        for (side, want) in [
            (Side::Bottom, (0.0, 1.0)),
            (Side::Top, (0.0, -1.0)),
            (Side::Left, (-1.0, 0.0)),
            (Side::Right, (1.0, 0.0)),
        ] {
            let n = text_normal((1.0, 0.0), Some(side), |_| panic!("forced, never read"));
            assert_eq!(n, want, "{side:?}");
        }
    }

    #[test]
    fn the_step_is_the_constant_plus_half_the_names_reach_across() {
        // The daylight the reader sees is the constant, whatever the name
        // measures — so the two spellings sit at the same distance.
        let text = box_(-20.0, -6.0, 20.0, 6.0).bounds();
        assert_eq!(
            offset((0.0, -1.0), text),
            (0.0, -(NET_LABEL_OFFSET + 6.0)),
            "above a horizontal run: half the height"
        );
        assert_eq!(
            offset((1.0, 0.0), text),
            (NET_LABEL_OFFSET + 20.0, 0.0),
            "beside a vertical one: half the width"
        );
    }

    #[test]
    fn a_crowded_side_reads_negative_so_the_shallower_one_still_wins() {
        // A box already covering the point: the ray reports the depth left to
        // clear, so between two blocked sides the nearer exit wins rather than
        // both reading zero and the tie-break deciding blind.
        let over = [box_(-30.0, -10.0, 10.0, 10.0)];
        assert_eq!(clear_run((0.0, 0.0), (1.0, 0.0), &over), -10.0);
        assert_eq!(clear_run((0.0, 0.0), (-1.0, 0.0), &over), -30.0);
        assert_eq!(
            text_normal((0.0, 1.0), None, |s| clear_run(
                (0.0, 0.0),
                s.normal(),
                &over
            )),
            (1.0, 0.0),
            "the shallower side"
        );
    }
}

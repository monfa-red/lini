//! The natural strategy's obstacle half (ROUTING.md The natural strategy,
//! Respect): sample the fitted curve against the world's solid bodies
//! inflated by **margin** (`clearance / 2`); the first offending body
//! inserts a **via** — its margin-inflated corner nearest the chord. A body
//! that keeps offending widens to that side's corner *pair* (the curve
//! rides straight across its face), then pushes the pair out one margin per
//! round. The spline refits through all vias in chord order, at most
//! [`DODGE_ROUNDS`] rounds. Whatever still offends **draws anyway** and is
//! returned for the report: natural never strays.

use super::curve::{self, Fitted, Pt};
use crate::ledger::consts::DODGE_ROUNDS;
use crate::routing::ortho::rect::{Rect, box_dist, rect_box, seg_box};
use crate::routing::ortho::scene::SceneIndex;

const EPS: f64 = 1e-6;

/// The obstacle constraints one natural wire is judged against — built from
/// the same [`SceneIndex`] machinery every strategy routes with, and shared
/// with the law checker's natural arm (one keep-out construction, one
/// metric).
pub(crate) struct Keepouts {
    /// Every solid body in the wire's world (uninflated; judged at margin).
    solids: Vec<Rect>,
    /// The link's own endpoint bodies. Excused only where the wire lawfully
    /// leaves them — each end's own span, the orthogonal law's
    /// own-end-segment excuse. `None` when that endpoint geometrically
    /// contains its partner (a containment link runs inside it by design).
    ends: [Option<Rect>; 2],
    /// A self-loop's two ends are one body: either end's excuse covers it.
    self_loop: bool,
    margin: f64,
}

impl Keepouts {
    pub(crate) fn build(index: &SceneIndex, ends: [(&str, Rect); 2], margin: f64) -> Keepouts {
        let [(a, ra), (b, rb)] = ends;
        let own =
            |path: &str, partner: &str, r: Rect| (!index.geo_contains(path, partner)).then_some(r);
        Keepouts {
            solids: index.solid_rects_for([a, b]),
            ends: [own(a, b, ra), own(b, a, rb)],
            self_loop: a == b,
            margin,
        }
    }

    /// The first offence along a sampled piece of wire: the sample window,
    /// the rect it violates, and their distance. `excused` marks the end
    /// bodies this piece may lawfully enter (its own perpendicular leave).
    pub(crate) fn offence(&self, pts: &[Pt], excused: [bool; 2]) -> Option<([Pt; 2], Rect, f64)> {
        let excused = if self.self_loop {
            [excused[0] || excused[1]; 2]
        } else {
            excused
        };
        let bound = self.solids.iter().chain(
            self.ends
                .iter()
                .zip(excused)
                .filter(|(_, e)| !e)
                .filter_map(|(r, _)| r.as_ref()),
        );
        for r in bound {
            for s in pts.windows(2) {
                let d = box_dist(seg_box(s), rect_box(*r));
                if d < self.margin - EPS {
                    return Some(([s[0], s[1]], *r, d));
                }
            }
        }
        None
    }
}

/// The first body a fitted curve offends, span by span in curve order —
/// each end's own body excused at its own end span.
fn first_offender(curve: &[[Pt; 4]], keep: &Keepouts) -> Option<Rect> {
    let last = curve.len().saturating_sub(1);
    curve.iter().enumerate().find_map(|(i, c)| {
        keep.offence(&curve::sample_span(c), [i == 0, i == last])
            .map(|(_, r, _)| r)
    })
}

/// One offending body's dodge state: single nearest corner first, the
/// side's corner pair on a repeat, pushed out one margin per further round.
struct BodyPlan {
    body: Rect,
    pair: bool,
    level: usize,
}

/// A body's via corners at an escalation level: the detour rides the chord
/// side whose corners deviate less (ties toward the negative side — fixed,
/// Law 4's spirit); on that side, the two corners nearest the chord, ordered
/// along it — entering corner first.
fn corners_for(body: Rect, chord: (Pt, Pt), margin: f64, level: usize) -> [Pt; 2] {
    let r = body.inflate(margin * (1.0 + level as f64));
    let d = {
        let (dx, dy) = (chord.1.0 - chord.0.0, chord.1.1 - chord.0.1);
        let l = dx.hypot(dy);
        if l <= 0.0 {
            (1.0, 0.0)
        } else {
            (dx / l, dy / l)
        }
    };
    let s = |c: &Pt| d.0 * (c.1 - chord.0.1) - d.1 * (c.0 - chord.0.0);
    let along = |c: &Pt| d.0 * (c.0 - chord.0.0) + d.1 * (c.1 - chord.0.1);
    let cs = [(r.x0, r.y0), (r.x1, r.y0), (r.x0, r.y1), (r.x1, r.y1)];
    let (neg, pos): (Vec<Pt>, Vec<Pt>) = cs.into_iter().partition(|c| s(c) < 0.0);
    let dev = |side: &[Pt]| side.iter().map(|c| s(c).abs()).fold(0.0_f64, f64::max);
    let mut side = if pos.is_empty() || (!neg.is_empty() && dev(&neg) <= dev(&pos)) {
        neg
    } else {
        pos
    };
    side.sort_by(|a, b| {
        s(a).abs()
            .total_cmp(&s(b).abs())
            .then(along(a).total_cmp(&along(b)))
    });
    let mut pair = [side[0], *side.get(1).unwrap_or(&side[0])];
    pair.sort_by(|a, b| along(a).total_cmp(&along(b)));
    pair
}

/// Fit-and-dodge: `refit` fits the wire through a via list (chord order);
/// the loop feeds it offending bodies' vias for up to [`DODGE_ROUNDS`]
/// rounds. Returns the final geometry and whatever still offends —
/// `(body, distance)` pairs for the report, stubs included (a stub off a
/// fixed port cannot dodge, but its offence is still named).
pub(crate) fn dodge(
    keep: &Keepouts,
    chord: (Pt, Pt),
    refit: impl Fn(&[Pt]) -> Fitted,
) -> (Fitted, Vec<(Rect, f64)>) {
    let along = |p: &Pt| {
        let (dx, dy) = (chord.1.0 - chord.0.0, chord.1.1 - chord.0.1);
        dx * (p.0 - chord.0.0) + dy * (p.1 - chord.0.1)
    };
    let mut plan: Vec<BodyPlan> = Vec::new();
    let mut fitted = refit(&[]);
    for _ in 0..DODGE_ROUNDS {
        let Some(body) = first_offender(&fitted.1, keep) else {
            break;
        };
        match plan.iter_mut().find(|e| e.body == body) {
            None => plan.push(BodyPlan {
                body,
                pair: false,
                level: 0,
            }),
            Some(e) if !e.pair => e.pair = true,
            Some(e) => e.level += 1,
        }
        let mut vias: Vec<Pt> = plan
            .iter()
            .flat_map(|e| {
                let cs = corners_for(e.body, chord, keep.margin, e.level);
                if e.pair { cs.to_vec() } else { vec![cs[0]] }
            })
            .collect();
        vias.sort_by(|a, b| along(a).total_cmp(&along(b)));
        // Two bodies wanting vias in the same spot merge to one knot — a
        // pair of near-coincident knots would kink the blend between them.
        vias.dedup_by(|b, a| (b.0 - a.0).hypot(b.1 - a.1) < keep.margin);
        envelope(&mut vias, chord);
        fitted = refit(&vias);
    }
    let leftover = offences(&fitted, keep);
    (fitted, leftover)
}

/// Envelope prune: an interior via that dips back toward the chord, below
/// the line of its same-side neighbours, wiggles the detour without
/// clearing anything its neighbours don't already clear — drop it, to a
/// fixpoint. Slaloms (sign changes) are untouched.
fn envelope(vias: &mut Vec<Pt>, chord: (Pt, Pt)) {
    let (dx, dy) = (chord.1.0 - chord.0.0, chord.1.1 - chord.0.1);
    let l = dx.hypot(dy);
    let d = if l <= 0.0 {
        (1.0, 0.0)
    } else {
        (dx / l, dy / l)
    };
    let s = |c: &Pt| d.0 * (c.1 - chord.0.1) - d.1 * (c.0 - chord.0.0);
    let along = |c: &Pt| d.0 * (c.0 - chord.0.0) + d.1 * (c.1 - chord.0.1);
    loop {
        let dip = (1..vias.len().saturating_sub(1)).find(|&i| {
            let (sp, si, sn) = (s(&vias[i - 1]), s(&vias[i]), s(&vias[i + 1]));
            if sp.signum() != si.signum() || si.signum() != sn.signum() {
                return false;
            }
            let (tp, ti, tn) = (along(&vias[i - 1]), along(&vias[i]), along(&vias[i + 1]));
            if tn - tp <= 0.0 {
                return false;
            }
            let at = sp + (sn - sp) * (ti - tp) / (tn - tp);
            si.abs() < at.abs() - EPS
        });
        match dip {
            Some(i) => {
                vias.remove(i);
            }
            None => break,
        }
    }
}

/// Every distinct body a drawn wire still offends, with its closest
/// distance — the stubs judged with their own-end excuse, the curve span by
/// span.
fn offences(fitted: &Fitted, keep: &Keepouts) -> Vec<(Rect, f64)> {
    let (path, curve) = fitted;
    let mut out: Vec<(Rect, f64)> = Vec::new();
    let mut push = |r: Rect, d: f64| match out.iter_mut().find(|(b, _)| *b == r) {
        Some((_, min)) => *min = min.min(d),
        None => out.push((r, d)),
    };
    let n = path.len();
    if n >= 2 {
        if let Some((_, r, d)) = keep.offence(&[path[0], path[1]], [true, false]) {
            push(r, d);
        }
        if let Some((_, r, d)) = keep.offence(&[path[n - 2], path[n - 1]], [false, true]) {
            push(r, d);
        }
    }
    let last = curve.len().saturating_sub(1);
    for (i, c) in curve.iter().enumerate() {
        if let Some((_, r, d)) = keep.offence(&curve::sample_span(c), [i == 0, i == last]) {
            push(r, d);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::natural::curve::direct;

    fn keep(solids: Vec<Rect>, margin: f64) -> Keepouts {
        Keepouts {
            solids,
            ends: [None, None],
            self_loop: false,
            margin,
        }
    }

    fn fit_between(vias: &[Pt]) -> Fitted {
        direct(
            (0.0, 50.0),
            (1.0, 0.0),
            12.0,
            (200.0, 50.0),
            (-1.0, 0.0),
            12.0,
            vias,
        )
    }

    const CHORD: (Pt, Pt) = ((12.0, 50.0), (188.0, 50.0));

    #[test]
    fn a_clean_fit_passes_through_untouched() {
        let k = keep(vec![Rect::new(80.0, 200.0, 120.0, 240.0)], 8.0);
        let (fitted, left) = dodge(&k, CHORD, fit_between);
        assert_eq!(fitted, fit_between(&[]));
        assert!(left.is_empty());
    }

    #[test]
    fn a_straddling_body_is_dodged_clear_at_margin() {
        // A body dead on the chord: vias thread the near corners and every
        // sample clears the margin.
        let body = Rect::new(90.0, 30.0, 130.0, 70.0);
        let k = keep(vec![body], 8.0);
        let ((_, curve), left) = dodge(&k, CHORD, fit_between);
        assert!(left.is_empty(), "the dodge resolves the offence");
        assert!(curve.len() >= 2, "vias split the spline");
        assert!(first_offender(&curve, &k).is_none());
    }

    #[test]
    fn a_body_hugging_the_port_draws_anyway_and_reports() {
        // A solid 3 px off port A's stub: the port is fixed, no via can fix
        // the stub — the wire still draws end to end and the body is named.
        let body = Rect::new(15.0, 40.0, 60.0, 60.0);
        let k = keep(vec![body], 8.0);
        let ((path, _), left) = dodge(&k, CHORD, fit_between);
        assert_eq!(*path.first().unwrap(), (0.0, 50.0));
        assert_eq!(*path.last().unwrap(), (200.0, 50.0));
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].0, body);
    }

    #[test]
    fn dodging_is_deterministic() {
        let body = Rect::new(90.0, 30.0, 130.0, 70.0);
        let k = keep(vec![body], 8.0);
        let once = dodge(&k, CHORD, fit_between);
        let twice = dodge(&k, CHORD, fit_between);
        assert_eq!(once, twice);
    }
}

//! Clearance-aware corridor tightening (ROUTING.md The natural strategy;
//! PLAN-TREE-alpha1 Stage 4). The plain fit follows the chain's polyline but
//! is corridor-blind: on an obstacle-dodging chain the spline can cut a
//! corner into a keep-out. This pass samples the fitted curve against the
//! same solids the router searched around and, where clearance breaks,
//! tightens the curve toward the corridor's polyline — never out of the
//! corridor. One mechanism, two converging steps: the spline re-anchors
//! through the polyline's own corners (every chord then lies **on** the
//! legal polyline), then only the offending spans' tangent handles halve,
//! round by round; a final round snaps what still offends to its chord — a
//! zero-handle span *is* its polyline piece, legal by construction. On a
//! free corridor (the tree/mindmap case) the first sampling is clean and the
//! fit passes through untouched. Deterministic: fixed rounds, fixed order,
//! no RNG.

use super::curve::{self, Pt};
use crate::routing::ortho::rect::{Rect, box_dist, rect_box, seg_box};
use crate::routing::ortho::scene::SceneIndex;

const EPS: f64 = 1e-6;

/// Handle-halving rounds before offenders snap to their chords: 0.5⁸ of a
/// handle is visually gone, and the snap guarantees legality regardless.
const ROUNDS: usize = 8;

/// The clearance constraints one natural wire is judged against — built from
/// the same [`SceneIndex`] machinery the router routed with, and shared with
/// the law checker's natural arm (one keep-out construction, one metric).
pub(crate) struct Keepouts {
    /// Every solid body the route avoided (uninflated; judged at `c`).
    solids: Vec<Rect>,
    /// The link's own endpoint bodies. Excused only where the wire
    /// lawfully leaves them — exactly the orthogonal law's own-end-segment
    /// excuse, at span granularity. `None` when that endpoint geometrically
    /// contains its partner (a containment link runs inside it by design).
    ends: [Option<Rect>; 2],
    /// A self-loop's two ends are one body: either end's excuse covers it.
    self_loop: bool,
    c: f64,
}

impl Keepouts {
    pub(crate) fn build(index: &SceneIndex, ends: [(&str, Rect); 2], c: f64) -> Keepouts {
        let [(a, ra), (b, rb)] = ends;
        let own =
            |path: &str, partner: &str, r: Rect| (!index.geo_contains(path, partner)).then_some(r);
        Keepouts {
            solids: index.solid_rects_for([a, b]),
            ends: [own(a, b, ra), own(b, a, rb)],
            self_loop: a == b,
            c,
        }
    }

    /// The first clearance offence along a sampled piece of wire: the sample
    /// window, the rect it violates, and their distance. `excused` marks the
    /// end bodies this piece may lawfully enter (its own perpendicular
    /// leave).
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
                if d < self.c - EPS {
                    return Some(([s[0], s[1]], *r, d));
                }
            }
        }
        None
    }
}

/// Span indices whose sampling violates clearance. A span's own end body is
/// excused only when it is the wire's end span — the orthogonal law's
/// own-end-segment excuse, one span deep.
fn offenders(curve: &[[Pt; 4]], keep: &Keepouts) -> Vec<usize> {
    let last = curve.len().saturating_sub(1);
    curve
        .iter()
        .enumerate()
        .filter(|(i, c)| {
            keep.offence(&curve::sample_span(c), [*i == 0, *i == last])
                .is_some()
        })
        .map(|(i, _)| i)
        .collect()
}

fn lerp(a: Pt, b: Pt, f: f64) -> Pt {
    (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f)
}

/// Make a fitted curve honor its corridor: pass a clean fit through
/// untouched; otherwise re-anchor on the polyline's corners and tighten the
/// offending spans toward their chords (see the module doc).
pub(crate) fn tighten(
    poly: &[Pt],
    stub_a: f64,
    stub_b: f64,
    keep: &Keepouts,
    fitted: (Vec<Pt>, Vec<[Pt; 4]>),
) -> (Vec<Pt>, Vec<[Pt; 4]>) {
    if offenders(&fitted.1, keep).is_empty() {
        return fitted;
    }
    let s = curve::stubs(poly, stub_a, stub_b);
    let mut knots: Vec<Pt> = vec![s.a];
    for &p in &poly[1..poly.len() - 1] {
        if *knots.last().expect("non-empty") != p {
            knots.push(p);
        }
    }
    if *knots.last().expect("non-empty") != s.b {
        knots.push(s.b);
    }
    let mut curve = curve::spans(&knots, s.da, s.db);
    for round in 0..=ROUNDS {
        let bad = offenders(&curve, keep);
        if bad.is_empty() {
            break;
        }
        let f = if round == ROUNDS { 0.0 } else { 0.5 };
        for i in bad {
            curve[i][1] = lerp(curve[i][0], curve[i][1], f);
            curve[i][2] = lerp(curve[i][3], curve[i][2], f);
        }
    }
    let path = curve::sample(poly[0], s.a, *poly.last().expect("non-empty"), &curve);
    (path, curve)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keep(solids: Vec<Rect>, ends: [Option<Rect>; 2], c: f64) -> Keepouts {
        Keepouts {
            solids,
            ends,
            self_loop: false,
            c,
        }
    }

    #[test]
    fn a_clean_fit_passes_through_untouched() {
        // The tree dogleg with nothing in the way: byte-identical pass-through.
        let poly = [(40.0, 30.0), (100.0, 30.0), (100.0, 90.0), (160.0, 90.0)];
        let fitted = curve::fit(&poly, 16.0, 16.0);
        let k = keep(Vec::new(), [None, None], 16.0);
        let (path, curve) = tighten(&poly, 16.0, 16.0, &k, fitted.clone());
        assert_eq!(path, fitted.0);
        assert_eq!(curve, fitted.1);
    }

    #[test]
    fn a_corner_cutting_curve_tightens_inside_the_corridor() {
        // An over-the-obstacle Z: the polyline clears the body at exactly
        // c = 10; the midpoint-knot fit cuts the corner into the keep-out.
        // Tightened, every sample window holds clearance again and the ends
        // stay exact.
        let body = Rect::new(60.0, -20.0, 140.0, 40.0);
        let poly = [
            (20.0, 20.0),
            (50.0, 20.0),
            (50.0, -30.0),
            (150.0, -30.0),
            (150.0, 20.0),
            (180.0, 20.0),
        ];
        let k = keep(vec![body], [None, None], 10.0);
        let fitted = curve::fit(&poly, 10.0, 10.0);
        assert!(
            !offenders(&fitted.1, &k).is_empty(),
            "the plain fit cuts the corner — the scenario is real"
        );
        let (path, curve) = tighten(&poly, 10.0, 10.0, &k, fitted);
        assert!(offenders(&curve, &k).is_empty(), "tightened curve is legal");
        assert_eq!(*path.first().unwrap(), (20.0, 20.0));
        assert_eq!(*path.last().unwrap(), (180.0, 20.0));
    }

    #[test]
    fn tightening_is_deterministic() {
        let body = Rect::new(60.0, -20.0, 140.0, 40.0);
        let poly = [
            (20.0, 20.0),
            (50.0, 20.0),
            (50.0, -30.0),
            (150.0, -30.0),
            (150.0, 20.0),
            (180.0, 20.0),
        ];
        let k = keep(vec![body], [None, None], 10.0);
        let once = tighten(&poly, 10.0, 10.0, &k, curve::fit(&poly, 10.0, 10.0));
        let twice = tighten(&poly, 10.0, 10.0, &k, curve::fit(&poly, 10.0, 10.0));
        assert_eq!(once, twice);
    }
}

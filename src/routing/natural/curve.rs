//! The pure polyline → cubic-spline fit (ROUTING.md The natural strategy,
//! PLAN-TREE decision 7): each end keeps a dead-straight stub leaving its
//! side perpendicular (the marker run-up), and between the stubs a
//! G1-continuous cubic chain absorbs the offsets — knots at the stub tips
//! and at each interior segment's midpoint, tangents along the polyline, so
//! the curve follows the corridor the search chose, never a rounded illegal
//! straight line. The canonical dogleg (opposite horizontal sides, offset)
//! comes out as the classic horizontal-tangent S — a symmetric cubic pair
//! meeting at the jog's midpoint.

use crate::ledger::consts::NATURAL_PULL;

pub(crate) type Pt = (f64, f64);

/// Samples per cubic in the shared `path` polyline — dense enough that the
/// label arc-walk and mask bbox read the drawn curve faithfully. The
/// tightening pass and the law checker sample at the same density, so what
/// one judges is exactly what the other drew.
pub(crate) const SAMPLES: usize = 24;

fn sub(a: Pt, b: Pt) -> Pt {
    (a.0 - b.0, a.1 - b.1)
}

fn add(a: Pt, b: Pt) -> Pt {
    (a.0 + b.0, a.1 + b.1)
}

fn mul(a: Pt, k: f64) -> Pt {
    (a.0 * k, a.1 * k)
}

fn dot(a: Pt, b: Pt) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

fn len(a: Pt) -> f64 {
    a.0.hypot(a.1)
}

fn unit(a: Pt) -> Pt {
    let l = len(a);
    if l <= 0.0 {
        (0.0, 0.0)
    } else {
        mul(a, 1.0 / l)
    }
}

pub(crate) fn bezier(c: &[Pt; 4], t: f64) -> Pt {
    let u = 1.0 - t;
    let (b0, b1, b2, b3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    (
        b0 * c[0].0 + b1 * c[1].0 + b2 * c[2].0 + b3 * c[3].0,
        b0 * c[0].1 + b1 * c[1].1 + b2 * c[2].1 + b3 * c[3].1,
    )
}

/// One cubic span between knots with prescribed unit tangents: handles pull
/// `NATURAL_PULL` of the chord along each tangent, each clamped to the travel
/// toward the far knot along its own tangent so the curve never overshoots a
/// short offset (monotone along the end directions).
fn span(p0: Pt, t0: Pt, p1: Pt, t1: Pt) -> [Pt; 4] {
    let d = sub(p1, p0);
    let pull = NATURAL_PULL * len(d);
    let clamp = |t: Pt| {
        let travel = dot(d, t);
        if travel > 0.0 { pull.min(travel) } else { pull }
    };
    [
        p0,
        add(p0, mul(t0, clamp(t0))),
        sub(p1, mul(t1, clamp(t1))),
        p1,
    ]
}

/// The stub geometry of a chain's polyline: the two tip points and the unit
/// leave directions (each pointing off its body, along its own end segment).
pub(crate) struct Stubs {
    pub a: Pt,
    pub b: Pt,
    pub da: Pt,
    pub db: Pt,
}

/// Stub tips on a polyline. Stubs stay on their own end segment; two stubs
/// sharing one segment (the straight pair) split it rather than cross.
pub(crate) fn stubs(poly: &[Pt], stub_a: f64, stub_b: f64) -> Stubs {
    let last = poly.len() - 1;
    let da = unit(sub(poly[1], poly[0]));
    let db = unit(sub(poly[last - 1], poly[last]));
    let (la, lb) = (
        len(sub(poly[1], poly[0])),
        len(sub(poly[last], poly[last - 1])),
    );
    let (sa_len, sb_len) = if last == 1 {
        (stub_a.min(la / 2.0), stub_b.min(la / 2.0))
    } else {
        (stub_a.min(la), stub_b.min(lb))
    };
    Stubs {
        a: add(poly[0], mul(da, sa_len)),
        b: add(poly[last], mul(db, sb_len)),
        da,
        db,
    }
}

/// The G1 cubic chain through `knots`: the forced leave directions at the two
/// tips (perpendicular arrival), Catmull-Rom blends (toward the neighbouring
/// knots) inside; repeated knots drop their degenerate span.
pub(crate) fn spans(knots: &[Pt], da: Pt, db: Pt) -> Vec<[Pt; 4]> {
    let tangent = |i: usize| {
        if i == 0 {
            da
        } else if i == knots.len() - 1 {
            mul(db, -1.0)
        } else {
            unit(sub(knots[i + 1], knots[i - 1]))
        }
    };
    (0..knots.len() - 1)
        .filter(|&i| knots[i] != knots[i + 1])
        .map(|i| span(knots[i], tangent(i), knots[i + 1], tangent(i + 1)))
        .collect()
}

/// One span's dense sampling — its start point plus `SAMPLES` steps to its
/// end. What the tightening pass and the law checker both judge.
pub(crate) fn sample_span(c: &[Pt; 4]) -> Vec<Pt> {
    std::iter::once(c[0])
        .chain((1..=SAMPLES).map(|j| bezier(c, j as f64 / SAMPLES as f64)))
        .collect()
}

/// The shared `path` polyline of a fitted curve: port, stub tip, `SAMPLES`
/// points per cubic, port — port and stub points exact.
pub(crate) fn sample(pa: Pt, sa: Pt, pb: Pt, curve: &[[Pt; 4]]) -> Vec<Pt> {
    let mut path = vec![pa];
    if sa != pa {
        path.push(sa);
    }
    for c in curve {
        for j in 1..=SAMPLES {
            path.push(bezier(c, j as f64 / SAMPLES as f64));
        }
    }
    if *path.last().expect("non-empty") != pb {
        path.push(pb);
    }
    path
}

/// Fit the drawn natural geometry along a placed chain's polyline: the exact
/// cubics between the two stub tips, and the dense sampled path (port and
/// stub points exact, `SAMPLES` points per cubic).
pub(crate) fn fit(poly: &[Pt], stub_a: f64, stub_b: f64) -> (Vec<Pt>, Vec<[Pt; 4]>) {
    let last = poly.len() - 1;
    let s = stubs(poly, stub_a, stub_b);

    // Knots: the stub tips and each interior segment's midpoint. A midpoint
    // knot eases along the overall travel instead of snapping to its own
    // segment's axis — the classic mindmap S rather than a ballooned zig.
    let mut knots: Vec<Pt> = vec![s.a];
    for i in 1..last.saturating_sub(1) {
        knots.push(mul(add(poly[i], poly[i + 1]), 0.5));
    }
    knots.push(s.b);
    // The canonical single-jog case (decision 7 — the tree dogleg, and the
    // straight pair): one cubic between the stubs absorbs the whole offset,
    // the classic horizontal-tangent S. It applies whenever both forced
    // tangents advance toward the far stub; a doubling-back jog (a U) keeps
    // its midpoint knot so the curve follows the chain the search chose.
    let d = sub(s.b, s.a);
    if knots.len() == 3 && dot(d, s.da) > 0.0 && dot(d, mul(s.db, -1.0)) > 0.0 {
        knots.remove(1);
    }
    let curve = spans(&knots, s.da, s.db);
    (sample(poly[0], s.a, poly[last], &curve), curve)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-9;

    #[test]
    fn an_aligned_pair_draws_dead_straight() {
        // Same y, opposite horizontal sides: the spine is one straight line —
        // every sample stays on it, ends exactly on the ports.
        let (path, curve) = fit(&[(40.0, 50.0), (160.0, 50.0)], 16.0, 16.0);
        assert_eq!(curve.len(), 1);
        assert_eq!(*path.first().unwrap(), (40.0, 50.0));
        assert_eq!(*path.last().unwrap(), (160.0, 50.0));
        for p in &path {
            assert!((p.1 - 50.0).abs() < EPS, "off the line: {p:?}");
        }
        for w in path.windows(2) {
            assert!(w[1].0 >= w[0].0, "doubled back: {w:?}");
        }
    }

    #[test]
    fn the_dogleg_draws_the_classic_s() {
        // A dogleg between opposite horizontal sides: horizontal tangents at
        // both stub tips, monotone x, ends exactly on the ports.
        let poly = [(40.0, 30.0), (100.0, 30.0), (100.0, 90.0), (160.0, 90.0)];
        let (path, curve) = fit(&poly, 16.0, 16.0);
        assert_eq!(curve.len(), 1, "one cubic absorbs the whole offset");
        // The stubs are exact and straight.
        assert_eq!(path[0], (40.0, 30.0));
        assert_eq!(path[1], (56.0, 30.0));
        assert_eq!(*path.last().unwrap(), (160.0, 90.0));
        assert_eq!(path[path.len() - 2], (144.0, 90.0));
        // Horizontal tangents where the curve meets the stubs: both control
        // handles are horizontal.
        assert!((curve[0][1].1 - curve[0][0].1).abs() < EPS);
        assert!((curve[0][2].1 - curve[0][3].1).abs() < EPS);
        // Monotone in x and y along the whole drawn path.
        for w in path.windows(2) {
            assert!(w[1].0 >= w[0].0 - EPS, "x doubled back: {w:?}");
            assert!(w[1].1 >= w[0].1 - EPS, "y doubled back: {w:?}");
        }
        // Symmetric: the S passes through the jog's midpoint.
        let mid = bezier(&curve[0], 0.5);
        assert!((mid.0 - 100.0).abs() < EPS && (mid.1 - 60.0).abs() < EPS);
    }

    #[test]
    fn samples_start_and_end_on_the_ports() {
        let poly = [(0.0, 0.0), (50.0, 0.0), (50.0, 40.0)];
        let (path, _) = fit(&poly, 10.0, 10.0);
        assert_eq!(*path.first().unwrap(), (0.0, 0.0));
        assert_eq!(*path.last().unwrap(), (50.0, 40.0));
    }
}

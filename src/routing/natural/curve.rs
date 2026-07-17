//! The direct spline fit (ROUTING.md The natural strategy, Smoothness): a
//! natural wire is its two dead-straight perpendicular stubs joined by one
//! G1-continuous cubic chain — knots at the stub tips and at any dodge vias,
//! tangents forced normal at the ends and Catmull-Rom-blended inside. Born a
//! curve, never lowered from a polyline: the aligned pair comes out dead
//! straight, the offset pair as the classic horizontal-tangent S.

use crate::ledger::consts::NATURAL_PULL;

pub(crate) type Pt = (f64, f64);

/// A fitted wire: the dense sampled `path` and the exact cubics between the
/// stub tips.
pub(crate) type Fitted = (Vec<Pt>, Vec<[Pt; 4]>);

/// Samples per cubic in the shared `path` polyline — dense enough that the
/// label arc-walk, mask bboxes, crossing counts, the dodge pass, and the law
/// checker all read the drawn curve faithfully, and all read the same one.
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
/// short offset — but never below half the pull: a chord running nearly
/// perpendicular to its tangent (a diagonal connection, a dodge entry) must
/// sweep out, not elbow. Crossing is lawful; sharpness is not.
fn span(p0: Pt, t0: Pt, p1: Pt, t1: Pt) -> [Pt; 4] {
    let d = sub(p1, p0);
    let pull = NATURAL_PULL * len(d);
    let clamp = |t: Pt| {
        let travel = dot(d, t);
        if travel > 0.0 {
            pull.min(travel).max(pull / 2.0)
        } else {
            pull
        }
    };
    [
        p0,
        add(p0, mul(t0, clamp(t0))),
        sub(p1, mul(t1, clamp(t1))),
        p1,
    ]
}

/// An end span advancing less than this fraction of its length along its
/// forced tangent is a **wrench**: the curve arrives nearly parallel to the
/// side it must land perpendicular on and has to hook around at the port. A
/// sweeping rise (a C arc, a wall arch) advances 0.2–0.4 — well clear.
const HOOK_RATIO: f64 = 0.1;

/// Whether a fitted chain **hooks** at a port (see [`HOOK_RATIO`]). The
/// direct fit never hooks on heuristic sides (they maximise the travel); a
/// dodge can manufacture a hooked arrival by bending the approach parallel
/// to the landing side, and the dodge policy rejects it for the smooth
/// direct fit instead.
pub(crate) fn hooky(curve: &[[Pt; 4]]) -> bool {
    let end = |c: &[Pt; 4], handle: Pt| {
        let t = unit(handle);
        if t == (0.0, 0.0) {
            return false;
        }
        let d = sub(c[3], c[0]);
        dot(d, t) < HOOK_RATIO * len(d) - 1e-9
    };
    let (Some(first), Some(last)) = (curve.first(), curve.last()) else {
        return false;
    };
    end(first, sub(first[1], first[0])) || end(last, sub(last[3], last[2]))
}

/// The G1 cubic chain through knots, each knot carrying an optional
/// **forced tangent** (its travel direction along the curve — the ends
/// always force theirs, a detour via forces the face it glides along);
/// `None` blends Catmull-Rom toward the neighbouring knots. Repeated knots
/// dedupe **before** the tangents are read — blending across a kept
/// duplicate would give the joint two different tangents, a kink.
pub(crate) fn spans(knots: &[(Pt, Option<Pt>)]) -> Vec<[Pt; 4]> {
    let mut knots: Vec<(Pt, Option<Pt>)> = knots.to_vec();
    knots.dedup_by(|b, a| b.0 == a.0);
    let tangent = |i: usize| {
        knots[i]
            .1
            .unwrap_or_else(|| unit(sub(knots[(i + 1).min(knots.len() - 1)].0, knots[i - 1].0)))
    };
    (0..knots.len().saturating_sub(1))
        .map(|i| span(knots[i].0, tangent(i), knots[i + 1].0, tangent(i + 1)))
        .collect()
}

/// One span's dense sampling — its start point plus `SAMPLES` steps to its
/// end. What the dodge pass and the law checker both judge.
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

/// The direct fit: ports `pa`/`pb`, unit leave normals `na`/`nb`, stub
/// lengths `sa`/`sb`, and the dodge vias (already in chord order, each with
/// an optional forced tangent) as interior knots. Facing stubs split the
/// gap between the port planes rather than cross (the ports may sit closer
/// than two stubs). Returns the dense sampled path and the exact cubics
/// between the stub tips.
pub(crate) fn direct(
    pa: Pt,
    na: Pt,
    sa: f64,
    pb: Pt,
    nb: Pt,
    sb: f64,
    vias: &[(Pt, Option<Pt>)],
) -> Fitted {
    let (mut sa, mut sb) = (sa, sb);
    if dot(na, nb) < -0.5 {
        let avail = dot(sub(pb, pa), na);
        if avail > 0.0 {
            sa = sa.min(avail / 2.0);
            sb = sb.min(avail / 2.0);
        }
    }
    let ta = add(pa, mul(na, sa));
    let tb = add(pb, mul(nb, sb));
    let mut knots = Vec::with_capacity(vias.len() + 2);
    knots.push((ta, Some(na)));
    knots.extend_from_slice(vias);
    knots.push((tb, Some(mul(nb, -1.0))));
    let curve = spans(&knots);
    (sample(pa, ta, pb, &curve), curve)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-9;

    #[test]
    fn an_aligned_pair_draws_dead_straight() {
        // Same y, facing horizontal sides: every sample on the line, ends
        // exactly on the ports, never doubling back.
        let (path, curve) = direct(
            (40.0, 50.0),
            (1.0, 0.0),
            16.0,
            (160.0, 50.0),
            (-1.0, 0.0),
            16.0,
            &[],
        );
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
    fn an_offset_pair_draws_the_classic_s() {
        // Facing horizontal sides, vertical offset: one cubic absorbs the
        // whole offset — horizontal tangents at both stub tips, monotone in
        // both axes, symmetric through the midpoint.
        let (path, curve) = direct(
            (40.0, 30.0),
            (1.0, 0.0),
            16.0,
            (160.0, 90.0),
            (-1.0, 0.0),
            16.0,
            &[],
        );
        assert_eq!(curve.len(), 1, "one cubic absorbs the whole offset");
        assert_eq!(path[0], (40.0, 30.0));
        assert_eq!(path[1], (56.0, 30.0));
        assert_eq!(*path.last().unwrap(), (160.0, 90.0));
        assert_eq!(path[path.len() - 2], (144.0, 90.0));
        assert!((curve[0][1].1 - curve[0][0].1).abs() < EPS);
        assert!((curve[0][2].1 - curve[0][3].1).abs() < EPS);
        for w in path.windows(2) {
            assert!(w[1].0 >= w[0].0 - EPS, "x doubled back: {w:?}");
            assert!(w[1].1 >= w[0].1 - EPS, "y doubled back: {w:?}");
        }
        let mid = bezier(&curve[0], 0.5);
        assert!((mid.0 - 100.0).abs() < EPS && (mid.1 - 60.0).abs() < EPS);
    }

    #[test]
    fn close_facing_stubs_split_the_gap_rather_than_cross() {
        // Port planes 20 apart, stubs 16 each: tips meet at the middle, the
        // path stays monotone.
        let (path, _) = direct(
            (40.0, 50.0),
            (1.0, 0.0),
            16.0,
            (60.0, 50.0),
            (-1.0, 0.0),
            16.0,
            &[],
        );
        assert_eq!(path[1], (50.0, 50.0));
        for w in path.windows(2) {
            assert!(w[1].0 >= w[0].0 - EPS, "doubled back: {w:?}");
        }
    }

    #[test]
    fn a_via_threads_the_curve_through_it() {
        let via = (100.0, 0.0);
        let (path, curve) = direct(
            (40.0, 30.0),
            (1.0, 0.0),
            10.0,
            (160.0, 30.0),
            (-1.0, 0.0),
            10.0,
            &[(via, None)],
        );
        assert_eq!(curve.len(), 2);
        assert_eq!(curve[0][3], via, "the via is a knot");
        assert_eq!(*path.first().unwrap(), (40.0, 30.0));
        assert_eq!(*path.last().unwrap(), (160.0, 30.0));
        // G1 at the via: the two handles are collinear through it.
        let out = sub(curve[1][1], curve[1][0]);
        let inn = sub(curve[0][3], curve[0][2]);
        assert!(
            (out.0 * inn.1 - out.1 * inn.0).abs() < EPS,
            "kink at the via"
        );
    }

    #[test]
    fn samples_start_and_end_on_the_ports() {
        let (path, _) = direct(
            (0.0, 0.0),
            (1.0, 0.0),
            10.0,
            (50.0, 40.0),
            (0.0, 1.0),
            10.0,
            &[],
        );
        assert_eq!(*path.first().unwrap(), (0.0, 0.0));
        assert_eq!(*path.last().unwrap(), (50.0, 40.0));
    }
}

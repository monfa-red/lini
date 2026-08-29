//! Where two offset elements **meet** at a centreline corner [SPEC 15.11].
//! The side chain hands this module a chained element and the next one's raw
//! parallel; it returns the pair joined the way SVG's own stroker would: a
//! tangent-continuous corner merges, an **outside** corner mitres to the
//! tangent intersection (bevelling past miter limit 4), and an **inside**
//! corner trims both back to their carriers' true crossing, so the inner face
//! stays crisp instead of doubling back.

use super::super::super::drawing::geometry::{P, PathSeg, SEAM_EPS, arc_center, arc_tangent, dist};
use super::push_line;
use crate::layout::geom::unit;
use crate::math;

/// SVG's default miter limit, the SPEC's number [SPEC 15.11]: a corner whose
/// miter would run past `limit × stroke-width` bevels instead. In tangent
/// terms: bevel when the wedge angle θ between the runs has
/// `1 ∕ sin(θ∕2) > 4` — equivalently `cos θ > 1 − 2 ∕ 4²`.
const MITER_COS: f64 = 1.0 - 2.0 / (4.0 * 4.0);

/// Join `next` onto the chain's end at a centreline corner: coincident ends
/// merge (a tangent-continuous corner — a fillet, a tangent arc); an
/// **outside** corner mitres — straight tangent extension to the miter
/// point, the SVG join — or bevels past the limit; an **inside** corner
/// trims both elements to their carriers' true intersection, so the inner
/// face stays crisp [SPEC 15.11].
pub(super) fn join(out: &mut Vec<PathSeg>, mut next: PathSeg) {
    let prev = *out.last().expect("join needs a chained element");
    let a = prev.to();
    let b = next.from();
    if dist(a, b) <= SEAM_EPS {
        set_from(&mut next, a);
        out.push(next);
        return;
    }
    let t1 = end_tangent(&prev);
    let t2 = start_tangent(&next);
    let cross = t1.0 * t2.1 - t1.1 * t2.0;
    let w = (b.0 - a.0, b.1 - a.1);
    if cross.abs() < 1e-9 {
        // Parallel tangents — a hairpin's infinite miter; the bevel is the
        // whole join.
        push_line(out, b);
        out.push(next);
        return;
    }
    let s = (w.0 * t2.1 - w.1 * t2.0) / cross;
    let u = (t1.0 * w.1 - t1.1 * w.0) / cross;
    if s >= -SEAM_EPS && u >= -SEAM_EPS {
        // Outside: the offsets diverge; fill the wedge.
        if -(t1.0 * t2.0 + t1.1 * t2.1) > MITER_COS {
            push_line(out, b); // bevel — the acute spike capped [SPEC 15.11]
        } else {
            let m = (a.0 + s * t1.0, a.1 + s * t1.1);
            match out.last_mut().expect("chained") {
                PathSeg::Line { to, .. } => *to = m,
                _ => push_line(out, m),
            }
            match &mut next {
                PathSeg::Line { from, .. } => *from = m,
                _ => push_line(out, b),
            }
        }
        out.push(next);
    } else {
        // Inside: the offsets overlap; trim both back to the crossing.
        match trim_pair(&prev, &next) {
            Some((e1, e2)) => {
                *out.last_mut().expect("chained") = e1;
                out.push(e2);
            }
            // Elements too short to reach their crossing — connect straight.
            None => {
                push_line(out, b);
                out.push(next);
            }
        }
    }
}

/// Trim two offset elements to their carriers' intersection — the inside
/// corner's true meeting point. Every carrier crossing is tried; the one
/// nearest the corner that lies **on both elements** wins.
fn trim_pair(prev: &PathSeg, next: &PathSeg) -> Option<(PathSeg, PathSeg)> {
    let (a, b) = (prev.to(), next.from());
    crossings(prev, next)
        .into_iter()
        .filter_map(|p| Some((trim_end(prev, p)?, trim_start(next, p)?, p)))
        .min_by(|x, y| {
            let d = |p: P| dist(p, a) + dist(p, b);
            d(x.2).total_cmp(&d(y.2))
        })
        .map(|(e1, e2, _)| (e1, e2))
}

/// Where two carriers (infinite line / full circle) cross — at most two points.
fn crossings(e1: &PathSeg, e2: &PathSeg) -> Vec<P> {
    match (carrier(e1), carrier(e2)) {
        (Carrier::Line(p1, d1), Carrier::Line(p2, d2)) => {
            let cross = d1.0 * d2.1 - d1.1 * d2.0;
            if cross.abs() < 1e-12 {
                return Vec::new();
            }
            let w = (p2.0 - p1.0, p2.1 - p1.1);
            let s = (w.0 * d2.1 - w.1 * d2.0) / cross;
            vec![(p1.0 + s * d1.0, p1.1 + s * d1.1)]
        }
        (Carrier::Line(p, d), Carrier::Circle(c, r))
        | (Carrier::Circle(c, r), Carrier::Line(p, d)) => {
            let f = (p.0 - c.0, p.1 - c.1);
            let b = f.0 * d.0 + f.1 * d.1;
            let disc = b * b - (f.0 * f.0 + f.1 * f.1 - r * r);
            if disc < 0.0 {
                return Vec::new();
            }
            let root = disc.sqrt();
            [-b - root, -b + root]
                .iter()
                .map(|s| (p.0 + s * d.0, p.1 + s * d.1))
                .collect()
        }
        (Carrier::Circle(c1, r1), Carrier::Circle(c2, r2)) => {
            let d = dist(c1, c2);
            if d < 1e-12 || d > r1 + r2 || d < (r1 - r2).abs() {
                return Vec::new();
            }
            let along = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
            let cross = (r1 * r1 - along * along).max(0.0).sqrt();
            let u = ((c2.0 - c1.0) / d, (c2.1 - c1.1) / d);
            let base = (c1.0 + along * u.0, c1.1 + along * u.1);
            vec![
                (base.0 - cross * u.1, base.1 + cross * u.0),
                (base.0 + cross * u.1, base.1 - cross * u.0),
            ]
        }
    }
}

enum Carrier {
    Line(P, P),
    Circle(P, f64),
}

fn carrier(e: &PathSeg) -> Carrier {
    match *e {
        PathSeg::Line { from, to } => Carrier::Line(
            from,
            unit((to.0 - from.0, to.1 - from.1)).expect("zero-length filtered"),
        ),
        PathSeg::Arc {
            from,
            to,
            r,
            large,
            sweep,
        } => Carrier::Circle(arc_center(from, to, r, large, sweep), r),
        PathSeg::Cubic { .. } => unreachable!("curve() rejected before offsetting"),
    }
}

/// The element shortened so it **ends** at `p` — `None` when `p` is not on
/// the element (behind its start, or past its end).
fn trim_end(e: &PathSeg, p: P) -> Option<PathSeg> {
    match *e {
        PathSeg::Line { from, to } => {
            on_span(from, to, p)?;
            Some(PathSeg::Line { from, to: p })
        }
        PathSeg::Arc {
            from,
            to,
            r,
            large,
            sweep,
        } => {
            let c = arc_center(from, to, r, large, sweep);
            let full = sweep_angle(c, from, to, sweep);
            let part = sweep_angle(c, from, p, sweep);
            (part > 1e-9 && part <= full + 1e-9).then_some(PathSeg::Arc {
                from,
                to: p,
                r,
                large: part > std::f64::consts::PI,
                sweep,
            })
        }
        PathSeg::Cubic { .. } => None,
    }
}

/// The element shortened so it **starts** at `p` — the mirror of [`trim_end`].
fn trim_start(e: &PathSeg, p: P) -> Option<PathSeg> {
    Some(trim_end(&e.reverse(), p)?.reverse())
}

/// `p`'s parameter on the closed span `from → to`, if it lies there with room
/// left (a trim must keep some element).
fn on_span(from: P, to: P, p: P) -> Option<()> {
    let len = dist(from, to);
    let d = unit((to.0 - from.0, to.1 - from.1))?;
    let t = (p.0 - from.0) * d.0 + (p.1 - from.1) * d.1;
    (t > 1e-9 && t <= len + 1e-9).then_some(())
}

/// The angle swept from `from` to `to` about `c` in the arc's own direction
/// (SVG sweep 1 = increasing screen angle), in `(0, 2π]`.
fn sweep_angle(c: P, from: P, to: P, sweep: bool) -> f64 {
    let ang = |p: P| math::atan2(p.1 - c.1, p.0 - c.0);
    let d = if sweep {
        ang(to) - ang(from)
    } else {
        ang(from) - ang(to)
    };
    d.rem_euclid(2.0 * std::f64::consts::PI)
}

fn end_tangent(e: &PathSeg) -> P {
    match *e {
        PathSeg::Line { from, to } => unit((to.0 - from.0, to.1 - from.1)).unwrap_or((0.0, 0.0)),
        PathSeg::Arc {
            from,
            to,
            r,
            large,
            sweep,
        } => arc_tangent(to, arc_center(from, to, r, large, sweep), sweep),
        PathSeg::Cubic { .. } => (0.0, 0.0),
    }
}

fn start_tangent(e: &PathSeg) -> P {
    match *e {
        PathSeg::Line { from, to } => unit((to.0 - from.0, to.1 - from.1)).unwrap_or((0.0, 0.0)),
        PathSeg::Arc {
            from,
            to,
            r,
            large,
            sweep,
        } => arc_tangent(from, arc_center(from, to, r, large, sweep), sweep),
        PathSeg::Cubic { .. } => (0.0, 0.0),
    }
}

fn set_from(e: &mut PathSeg, p: P) {
    match e {
        PathSeg::Line { from, .. } | PathSeg::Arc { from, .. } | PathSeg::Cubic { from, .. } => {
            *from = p;
        }
    }
}

//! The pen's corner modifiers [SPEC 15.3]: `fillet(r)` / `chamfer(c)` park
//! between two segments — a line or an **arc** on either side — trim both legs,
//! and drop in the joint. The pen (`super::pen`) owns *when* a modifier applies
//! (incl. cyclically through `close()`); this module owns the trim.
//!
//! A fillet's centre is at distance `r` from both legs, so it solves closed-form
//! [SPEC 15.8]: the locus at `r` from a line is a parallel **offset line**, from
//! an arc a concentric **offset circle**; the centre is their intersection
//! (line∩line, line∩circle, or circle∩circle — one quadratic, no iteration).
//! A `chamfer(c)` cuts `c` back along each leg — **by arclength** on an arc.

use super::Segment;
use super::geometry::{P, PathSeg, arc_center, dist};
use crate::error::Error;
use crate::span::Span;

/// A pending corner modifier — parked between its two segments.
#[derive(Clone, Copy)]
pub(super) enum Mod {
    Fillet(f64),
    Chamfer(f64),
}

impl Mod {
    pub(super) fn word(self) -> &'static str {
        match self {
            Mod::Fillet(_) => "fillet",
            Mod::Chamfer(_) => "chamfer",
        }
    }
}

// ── small 2-vector helpers ──
fn sub(a: P, b: P) -> P {
    (a.0 - b.0, a.1 - b.1)
}
fn add(a: P, b: P) -> P {
    (a.0 + b.0, a.1 + b.1)
}
fn scale(a: P, s: f64) -> P {
    (a.0 * s, a.1 * s)
}
fn dot(a: P, b: P) -> f64 {
    a.0 * b.0 + a.1 * b.1
}
fn cross(a: P, b: P) -> f64 {
    a.0 * b.1 - a.1 * b.0
}
fn norm(a: P) -> P {
    let l = a.0.hypot(a.1);
    if l > 1e-12 { (a.0 / l, a.1 / l) } else { a }
}
/// Rotate about the origin by `t` radians (y-down screen frame).
fn rot(a: P, t: f64) -> P {
    let (s, c) = t.sin_cos();
    (a.0 * c - a.1 * s, a.0 * s + a.1 * c)
}

/// The travel (heading) direction at point `p` on a segment — the arc's tangent
/// there, sweep-oriented (SVG sweep 1 walks the increasing angle, clockwise on a
/// y-down screen), or the line's own direction.
fn heading(seg: &PathSeg, p: P) -> P {
    match *seg {
        PathSeg::Line { from, to } => norm(sub(to, from)),
        PathSeg::Arc {
            from,
            to,
            r,
            large,
            sweep,
        } => {
            let o = arc_center(from, to, r, large, sweep);
            let v = sub(p, o);
            norm(if sweep { (-v.1, v.0) } else { (v.1, -v.0) })
        }
        PathSeg::Cubic { .. } => (0.0, 0.0),
    }
}

/// Trim the corner between two segments and drop in the modifier's joint
/// [SPEC 15.3]. Returns (trimmed prev, joint, trimmed next, the joint's
/// `:segment`).
pub(super) fn apply_mod(
    m: Mod,
    prev: PathSeg,
    next: PathSeg,
    span: Span,
) -> Result<(PathSeg, PathSeg, PathSeg, Segment), Error> {
    if matches!(prev, PathSeg::Cubic { .. }) || matches!(next, PathSeg::Cubic { .. }) {
        return Err(Error::at(
            span,
            format!("'{}' joins lines and arcs, not a curve()", m.word()),
        ));
    }
    let c = prev.to();
    debug_assert!(
        dist(c, next.from()) < 1e-9,
        "corner segments meet at one point"
    );
    let t_in = heading(&prev, c);
    let t_out = heading(&next, c);
    if cross(t_in, t_out).abs() < 1e-9 {
        return Err(Error::at(
            span,
            format!("'{}' needs a turn between its two runs", m.word()),
        ));
    }
    // The interior bisector points from the corner toward the joint's centre.
    let bi = norm(add(scale(t_in, -1.0), t_out));
    let amount = match m {
        Mod::Fillet(r) => r,
        Mod::Chamfer(cc) => cc,
    };
    if amount <= 0.0 {
        return Err(fit_err(m, span));
    }

    let (ta, tb, joint, segment) = match m {
        Mod::Fillet(r) => fillet(&prev, &next, c, bi, r, span)?,
        Mod::Chamfer(cc) => chamfer(&prev, &next, cc)?,
    };
    Ok((
        trim(&prev, true, ta),
        joint,
        trim(&next, false, tb),
        segment,
    ))
}

fn fit_err(m: Mod, span: Span) -> Error {
    Error::at(span, format!("'{}' does not fit its corner", m.word()))
}

/// The offset locus of a leg at distance `r` toward the interior `bi` — a line
/// parallel to a straight leg, a circle concentric with a curved one.
enum Offset {
    /// The line `{ P : P·n = d }` (n a unit normal toward the interior).
    Line(P, f64),
    /// The circle `|P − o| = r`.
    Circle(P, f64),
}

fn leg_offset(seg: &PathSeg, at_end: bool, r: f64, bi: P, span: Span) -> Result<Offset, Error> {
    match *seg {
        PathSeg::Line { from, to } => {
            let u = norm(sub(to, from));
            let n = (-u.1, u.0);
            let n = if dot(n, bi) >= 0.0 { n } else { scale(n, -1.0) };
            let c = if at_end { to } else { from };
            Ok(Offset::Line(n, dot(c, n) + r))
        }
        PathSeg::Arc {
            from,
            to,
            r: big,
            large,
            sweep,
        } => {
            let o = arc_center(from, to, big, large, sweep);
            let c = if at_end { to } else { from };
            // If the arc's own centre lies toward the interior, the fillet sits
            // *inside* the arc (R − r); otherwise outside (R + r).
            let rr = if dot(sub(o, c), bi) > 0.0 {
                big - r
            } else {
                big + r
            };
            if rr <= 1e-9 {
                return Err(Error::at(
                    span,
                    "'fillet' radius is too large for the arc it meets",
                ));
            }
            Ok(Offset::Circle(o, rr))
        }
        PathSeg::Cubic { .. } => unreachable!("guarded in apply_mod"),
    }
}

/// The fillet centre = the two offsets' intersection, the solution on the
/// interior side (nearest the corner along `bi`).
fn intersect(a: &Offset, b: &Offset, c: P, bi: P, span: Span) -> Result<P, Error> {
    let miss = || Error::at(span, "'fillet' does not fit its corner");
    let pick = |p1: P, p2: P| {
        // The centre lies off the corner toward the interior.
        if dot(sub(p1, c), bi) >= dot(sub(p2, c), bi) {
            p1
        } else {
            p2
        }
    };
    match (a, b) {
        (Offset::Line(n1, d1), Offset::Line(n2, d2)) => {
            let det = n1.0 * n2.1 - n1.1 * n2.0;
            if det.abs() < 1e-12 {
                return Err(miss());
            }
            Ok(((d1 * n2.1 - d2 * n1.1) / det, (n1.0 * d2 - n2.0 * d1) / det))
        }
        (Offset::Line(n, d), Offset::Circle(o, r)) | (Offset::Circle(o, r), Offset::Line(n, d)) => {
            line_circle(*n, *d, *o, *r)
                .map(|(p1, p2)| pick(p1, p2))
                .ok_or_else(miss)
        }
        (Offset::Circle(o1, r1), Offset::Circle(o2, r2)) => circle_circle(*o1, *r1, *o2, *r2)
            .map(|(p1, p2)| pick(p1, p2))
            .ok_or_else(miss),
    }
}

/// The two intersections of a line `P·n = d` and a circle `|P−o| = r`.
fn line_circle(n: P, d: f64, o: P, r: f64) -> Option<(P, P)> {
    // Foot of o on the line, then step ±√(r²−dist²) along the line direction.
    let dl = d - dot(o, n); // signed distance from o to the line
    let h2 = r * r - dl * dl;
    if h2 < 0.0 {
        return None;
    }
    let foot = add(o, scale(n, dl));
    let dir = (-n.1, n.0);
    let h = h2.sqrt();
    Some((add(foot, scale(dir, h)), add(foot, scale(dir, -h))))
}

/// The two intersections of two circles.
fn circle_circle(o1: P, r1: f64, o2: P, r2: f64) -> Option<(P, P)> {
    let d = dist(o1, o2);
    if d < 1e-12 || d > r1 + r2 + 1e-9 || d < (r1 - r2).abs() - 1e-9 {
        return None;
    }
    let a = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
    let h2 = (r1 * r1 - a * a).max(0.0);
    let u = norm(sub(o2, o1));
    let mid = add(o1, scale(u, a));
    let perp = (-u.1, u.0);
    let h = h2.sqrt();
    Some((add(mid, scale(perp, h)), add(mid, scale(perp, -h))))
}

/// The tangent point on a leg — the foot of the fillet centre on a line, or the
/// centre's radial projection onto an arc.
fn tangent_point(seg: &PathSeg, at_end: bool, centre: P) -> P {
    match *seg {
        PathSeg::Line { from, to } => {
            let u = norm(sub(to, from));
            add(from, scale(u, dot(sub(centre, from), u)))
        }
        PathSeg::Arc {
            from,
            to,
            r,
            large,
            sweep,
        } => {
            let _ = at_end;
            let o = arc_center(from, to, r, large, sweep);
            add(o, scale(norm(sub(centre, o)), r))
        }
        PathSeg::Cubic { .. } => unreachable!(),
    }
}

fn fillet(
    prev: &PathSeg,
    next: &PathSeg,
    c: P,
    bi: P,
    r: f64,
    span: Span,
) -> Result<(P, P, PathSeg, Segment), Error> {
    let op = leg_offset(prev, true, r, bi, span)?;
    let on = leg_offset(next, false, r, bi, span)?;
    let centre = intersect(&op, &on, c, bi, span)?;
    let ta = tangent_point(prev, true, centre);
    let tb = tangent_point(next, false, centre);
    // The tangent points must land on their legs, not run off an end.
    if !within_leg(prev, ta) || !within_leg(next, tb) {
        return Err(fit_err(Mod::Fillet(r), span));
    }
    // Sweep so the arc's heading at `ta` continues the prev leg's [SPEC 15.3].
    let v = sub(ta, centre);
    let sweep = dot(norm((-v.1, v.0)), heading(prev, ta)) > 0.0;
    let mid = add(centre, scale(norm(sub(midpoint(ta, tb), centre)), r));
    Ok((
        ta,
        tb,
        PathSeg::Arc {
            from: ta,
            to: tb,
            r,
            large: false,
            sweep,
        },
        Segment::Arc { mid, r },
    ))
}

fn chamfer(prev: &PathSeg, next: &PathSeg, cc: f64) -> Result<(P, P, PathSeg, Segment), Error> {
    let ta = back_along(prev, true, cc)?;
    let tb = back_along(next, false, cc)?;
    Ok((
        ta,
        tb,
        PathSeg::Line { from: ta, to: tb },
        Segment::Edge(ta, tb),
    ))
}

/// The point `cc` back along a leg from the corner — straight on a line, by
/// **arclength** on an arc [SPEC 15.3].
fn back_along(seg: &PathSeg, at_end: bool, cc: f64) -> Result<P, Error> {
    match *seg {
        PathSeg::Line { from, to } => {
            let len = dist(from, to);
            if cc > len - 1e-9 {
                return Err(fit_err(Mod::Chamfer(cc), Span::empty()));
            }
            let u = norm(sub(to, from));
            Ok(if at_end {
                sub(to, scale(u, cc))
            } else {
                add(from, scale(u, cc))
            })
        }
        PathSeg::Arc {
            from,
            to,
            r,
            large,
            sweep,
        } => {
            let o = arc_center(from, to, r, large, sweep);
            let c = if at_end { to } else { from };
            if cc > arc_progress(o, from, to, sweep) * r - 1e-9 {
                return Err(fit_err(Mod::Chamfer(cc), Span::empty()));
            }
            // Travel (sweep 1 = clockwise = increasing angle, y-down) advances
            // the angle by arclength/r; back off the corner the other way.
            let travel = if sweep { 1.0 } else { -1.0 };
            let d = travel * (cc / r) * if at_end { -1.0 } else { 1.0 };
            Ok(add(o, rot(sub(c, o), d)))
        }
        PathSeg::Cubic { .. } => unreachable!(),
    }
}

fn midpoint(a: P, b: P) -> P {
    (0.5 * (a.0 + b.0), 0.5 * (a.1 + b.1))
}

/// How far a point has travelled along an arc from `from`, as a non-negative
/// angle in the sweep direction (SVG sweep 1 = increasing angle, y-down).
fn arc_progress(o: P, from: P, p: P, sweep: bool) -> f64 {
    let a0 = (from.1 - o.1).atan2(from.0 - o.0);
    let ap = (p.1 - o.1).atan2(p.0 - o.0);
    let mut d = ap - a0;
    use std::f64::consts::TAU;
    if sweep {
        while d < 0.0 {
            d += TAU;
        }
        d
    } else {
        while d > 0.0 {
            d -= TAU;
        }
        -d
    }
}

/// Whether a tangent point sits on its leg (not run off an end) — between a
/// line's endpoints, or within an arc's angular span.
fn within_leg(seg: &PathSeg, p: P) -> bool {
    match *seg {
        PathSeg::Line { from, to } => {
            let ab = sub(to, from);
            let t = dot(sub(p, from), ab);
            (-1e-6..=dot(ab, ab) + 1e-6).contains(&t)
        }
        PathSeg::Arc {
            from,
            to,
            r,
            large,
            sweep,
        } => {
            let o = arc_center(from, to, r, large, sweep);
            arc_progress(o, from, p, sweep) <= arc_progress(o, from, to, sweep) + 1e-6
        }
        PathSeg::Cubic { .. } => false,
    }
}

/// Trim a leg to a tangent point: the corner end moves to `p`, the far end stays.
fn trim(seg: &PathSeg, at_end: bool, p: P) -> PathSeg {
    match *seg {
        PathSeg::Line { from, to } => {
            if at_end {
                PathSeg::Line { from, to: p }
            } else {
                PathSeg::Line { from: p, to }
            }
        }
        PathSeg::Arc {
            from,
            to,
            r,
            large,
            sweep,
        } => {
            if at_end {
                PathSeg::Arc {
                    from,
                    to: p,
                    r,
                    large,
                    sweep,
                }
            } else {
                PathSeg::Arc {
                    from: p,
                    to,
                    r,
                    large,
                    sweep,
                }
            }
        }
        PathSeg::Cubic { .. } => *seg,
    }
}

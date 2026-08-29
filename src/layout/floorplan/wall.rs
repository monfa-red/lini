//! The wall offset [SPEC 15.11]: a `|wall|`'s `draw:` traces its
//! **centreline**; this module grows it into the closed poché **outline** at
//! ± thickness ∕ 2 — mitred corners (an acute spike bevels at miter limit 4),
//! concentric arc offsets, butt caps on open ends, a `close()` seam mitred
//! like any corner — and the outline replaces the drawn path for paint and
//! for the geometry bbox ([SPEC 15.10] step 1: after the `draw:` fold, before
//! the bboxes). The authored `:segment`s stay **centreline** stations, so
//! dimensions measure where architects do while bbox anchors and leader
//! ray-casts read the outline.
//!
//! The walk is **per contiguous centreline run** (one pen subpath): each run
//! offsets to two joined side chains — [`offset_run`] — which the assembly
//! closes into loops. Openings (Phase 3) will cut a run's side chains at
//! stations and cap the jambs flat, so the split lives here, not in the pen.

use super::super::drawing::geometry::{
    self, P, PathSeg, SEAM_EPS, Subpath, arc_center, arc_tangent, dist, n, to_d,
};
use super::super::drawing::pen::Folded;
use crate::error::{Code, Error};
use crate::layout::geom::unit;
use crate::math;
use crate::resolve::ResolvedInst;
use crate::span::Span;

/// SVG's default miter limit, the SPEC's number [SPEC 15.11]: a corner whose
/// miter would run past `limit × stroke-width` bevels instead. In tangent
/// terms: bevel when the wedge angle θ between the runs has
/// `1 ∕ sin(θ∕2) > 4` — equivalently `cos θ > 1 − 2 ∕ 4²`.
const MITER_COS: f64 = 1.0 - 2.0 / (4.0 * 4.0);

/// Grow a folded wall centreline into its outline, in place: the subpaths,
/// the `d`, and the geometry bbox become the outline's; segments, mirrors,
/// and the view map stay the centreline's [SPEC 15.11].
pub(in crate::layout) fn offset(
    folded: &mut Folded,
    inst: &ResolvedInst,
    own: f64,
) -> Result<(), Error> {
    // Nearest-wins [SPEC 15.11]: a cascaded `thickness:` on the wall itself
    // (authored or rule-borne), else the desugar-stamped fallback — the
    // partition define / scope value / 200 mm default, already in drawing
    // units. The raw-mm constant only guards a fold desugar never saw.
    let units = inst
        .attrs
        .number("thickness")
        .or_else(|| inst.attrs.number(crate::desugar::scale::WALL_THICKNESS))
        .unwrap_or(crate::desugar::scale::WALL_MM);
    let h = units * own / 2.0;
    let mut outline = Vec::new();
    for sub in &folded.subs {
        outline.extend(offset_run(sub, h, own, inst.span)?);
    }
    folded.subs = outline;
    folded.d = to_d(&folded.subs);
    folded.geometry = geometry::geometry_bbox(&folded.d);
    Ok(())
}

/// One contiguous centreline run → its outline loops [SPEC 15.11]: a closed
/// run gives the two concentric loops (even-odd fills the band between); an
/// open run gives one loop — the left face out, a butt cap, the right face
/// back, a butt cap home.
fn offset_run(sub: &Subpath, h: f64, own: f64, span: Span) -> Result<Vec<Subpath>, Error> {
    check_run(sub, h, own, span)?;
    let segs: Vec<PathSeg> = sub
        .segs
        .iter()
        .filter(|s| dist(s.from(), s.to()) > SEAM_EPS)
        .copied()
        .collect();
    if segs.is_empty() {
        return Ok(Vec::new());
    }
    let left = side(&segs, sub.closed, h, true);
    let right = side(&segs, sub.closed, h, false);
    let right_back: Vec<PathSeg> = right.iter().rev().map(PathSeg::reverse).collect();
    if sub.closed {
        return Ok(vec![
            Subpath {
                segs: left,
                closed: true,
            },
            Subpath {
                segs: right_back,
                closed: true,
            },
        ]);
    }
    // Open ends butt-cap flat at the endpoints — no extension [SPEC 15.11].
    let mut segs = left;
    push_line(&mut segs, right_back[0].from());
    segs.extend(right_back);
    let home = segs[0].from();
    push_line(&mut segs, home);
    Ok(vec![Subpath { segs, closed: true }])
}

/// The centreline laws [SPEC 21]: `curve()` has no exact offset and errors;
/// an arc tighter than thickness ∕ 2 has no inner face and errors (`r ==
/// t ∕ 2` stays legal — the inner arc degenerates to the centre point).
fn check_run(sub: &Subpath, h: f64, own: f64, span: Span) -> Result<(), Error> {
    for seg in &sub.segs {
        match *seg {
            PathSeg::Cubic { .. } => {
                return Err(
                    Error::at(span, "a wall bends with 'arc()' — 'curve()' has no offset")
                        .code(Code::WALL_CURVE),
                );
            }
            PathSeg::Arc { r, .. } if r < h - SEAM_EPS => {
                return Err(Error::at(
                    span,
                    format!(
                        "arc radius {} is under thickness/2 — the inner face vanishes",
                        n(r / own)
                    ),
                )
                .code(Code::WALL_ARC));
            }
            _ => {}
        }
    }
    Ok(())
}

/// One side chain: every segment offset to its parallel (a line shifted along
/// its normal, an arc to its concentric), joined at each interior vertex —
/// and, for a closed run, across the seam, so `close()` mitres like any
/// corner [SPEC 15.11].
fn side(segs: &[PathSeg], closed: bool, h: f64, left: bool) -> Vec<PathSeg> {
    let mut out: Vec<PathSeg> = Vec::new();
    for seg in segs {
        let el = raw_offset(seg, h, left);
        if out.is_empty() {
            out.push(el);
        } else {
            join(&mut out, el);
        }
    }
    if closed && out.len() >= 2 {
        // The wrap join: run the first element through the same join, then
        // seat its (possibly trimmed) copy back at the head — cyclically the
        // inserted connectors belong at the tail.
        let first = out[0];
        join(&mut out, first);
        let seamed = out.pop().expect("join pushed the wrapped element");
        out[0] = seamed;
    }
    out
}

/// A segment's parallel at distance `h` on one side of travel. An arc's left
/// side is outside its circle exactly when it sweeps clockwise, so the
/// concentric radius is `r + h` when `sweep == left`, `r − h` otherwise
/// [SPEC 15.11].
fn raw_offset(seg: &PathSeg, h: f64, left: bool) -> PathSeg {
    match *seg {
        PathSeg::Line { from, to } => {
            let d = unit((to.0 - from.0, to.1 - from.1)).expect("zero-length filtered");
            let nrm = normal(d, left);
            PathSeg::Line {
                from: (from.0 + h * nrm.0, from.1 + h * nrm.1),
                to: (to.0 + h * nrm.0, to.1 + h * nrm.1),
            }
        }
        PathSeg::Arc {
            from,
            to,
            r,
            large,
            sweep,
        } => {
            let c = arc_center(from, to, r, large, sweep);
            let r2 = if sweep == left {
                r + h
            } else {
                (r - h).max(0.0)
            };
            let radial = |p: P| {
                if r2 <= SEAM_EPS {
                    c
                } else {
                    (c.0 + (p.0 - c.0) * (r2 / r), c.1 + (p.1 - c.1) * (r2 / r))
                }
            };
            PathSeg::Arc {
                from: radial(from),
                to: radial(to),
                r: r2,
                large,
                sweep,
            }
        }
        PathSeg::Cubic { .. } => unreachable!("curve() rejected before offsetting"),
    }
}

/// The unit normal on one side of travel direction `d` (y grows down): left
/// of travel is `d` turned a quarter counter-screen — the named-edge
/// convention's side [SPEC 15.5].
fn normal(d: P, left: bool) -> P {
    if left { (d.1, -d.0) } else { (-d.1, d.0) }
}

/// Join `next` onto the chain's end at a centreline corner: coincident ends
/// merge (a tangent-continuous corner — a fillet, a tangent arc); an
/// **outside** corner mitres — straight tangent extension to the miter
/// point, the SVG join — or bevels past the limit; an **inside** corner
/// trims both elements to their carriers' true intersection, so the inner
/// face stays crisp [SPEC 15.11].
fn join(out: &mut Vec<PathSeg>, mut next: PathSeg) {
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

/// Append a straight connector from the chain's end to `p`.
fn push_line(out: &mut Vec<PathSeg>, p: P) {
    let from = out.last().expect("connector needs a chain").to();
    if dist(from, p) > SEAM_EPS {
        out.push(PathSeg::Line { from, to: p });
    }
}

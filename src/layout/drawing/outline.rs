//! The drawn outline, and rays cast onto it [SPEC 15.2]: *the anchor aims;
//! the outline lands*. A leader's tip is the ray's **first crossing of the
//! drawn path** — a sketch's folded subpaths, an `|oval|`'s ellipse, a
//! `|line|` / `|poly|`'s points, any other shape's geometry rectangle; a
//! `pattern:` casts against every copy (the drawn path *is* the copies).
//! Everything is node-local; the caller transforms the ray through the
//! anchor's frame.

use super::super::ir::{Bbox, PlacedNode};
use super::chrome;
use super::geometry::{P, PathSeg, arc_center};
use crate::resolve::NodeKind;

/// One crossing of a ray with a drawn path: its parameter and the crossed
/// segment's unit tangent (in the frame the ray was given in) — the halo's
/// crossing angle, the parity test's count [SPEC 15.3/15.7].
#[derive(Clone, Copy)]
pub(super) struct Hit {
    pub t: f64,
    pub tangent: P,
    /// The hit sits on the segment's very endpoint — a corner touch. A ray
    /// riding an edge grazes the adjoining segment's end without entering
    /// material (its own edge is parallel and reports nothing), so a **lone**
    /// graze is no crossing; a true vertex crossing reports a twin graze on
    /// each adjoining segment at the same `t` [SPEC 15.7].
    pub graze: bool,
}

/// The first crossing (smallest `t > eps`) of the ray `o + t·d` with the
/// node's drawn path, node-local. `None` when the ray misses entirely.
pub(super) fn raycast(node: &PlacedNode, o: P, d: P) -> Option<f64> {
    let mut hits = Vec::new();
    crossings(node, o, d, &mut hits);
    hits.iter().map(|h| h.t).min_by(f64::total_cmp)
}

/// **Every** crossing of the ray with the node's own drawn path — the same
/// path [`raycast`] takes its first hit from, so the two never disagree on
/// what the outline is. Node-local; the caller transforms.
pub(super) fn crossings(node: &PlacedNode, o: P, d: P, out: &mut Vec<Hit>) {
    // A pattern carrier draws nothing itself — its copies are the path.
    if node.attrs.get("pattern").is_some() {
        for c in node
            .children
            .iter()
            .filter(|c| !chrome::is_chrome(&c.attrs))
        {
            crossings(c, (o.0 - c.cx, o.1 - c.cy), d, out);
        }
        return;
    }
    if let Some(geo) = &node.sketch {
        for seg in geo.outline.iter().flat_map(|sub| sub.segs.iter()) {
            seg_crossings(o, d, seg, out);
        }
        return;
    }
    let g = geometry_box(node);
    match node.kind {
        NodeKind::Oval => ellipse_crossings(o, d, g, out),
        NodeKind::Line | NodeKind::Poly => {
            let Some(pts) = super::super::primitives::attr_points(&node.attrs, "points", node.span)
                .ok()
                .flatten()
            else {
                return;
            };
            for w in pts.windows(2) {
                out.extend(hit_line(o, d, w[0], w[1]));
            }
            if node.kind == NodeKind::Poly && pts.len() > 2 {
                out.extend(hit_line(o, d, pts[pts.len() - 1], pts[0]));
            }
        }
        _ => {
            let corners = [
                (g.min_x, g.min_y),
                (g.max_x, g.min_y),
                (g.max_x, g.max_y),
                (g.min_x, g.max_y),
            ];
            for i in 0..4 {
                out.extend(hit_line(o, d, corners[i], corners[(i + 1) % 4]));
            }
        }
    }
}

/// Every crossing of the ray with one folded subpath segment [SPEC 15.3] —
/// the parity probe counts these over a whole profile.
pub(super) fn seg_crossings(o: P, d: P, seg: &PathSeg, out: &mut Vec<Hit>) {
    match *seg {
        PathSeg::Line { from, to } => out.extend(hit_line(o, d, from, to)),
        PathSeg::Arc {
            from,
            to,
            r,
            large,
            sweep,
        } => arc_crossings(o, d, from, to, r, large, sweep, out),
        // The advanced 10 % — flattened; drafting precision, not spline-exact.
        PathSeg::Cubic { from, c1, c2, to } => {
            let at = |t: f64| {
                let u = 1.0 - t;
                (
                    u * u * u * from.0
                        + 3.0 * u * u * t * c1.0
                        + 3.0 * u * t * t * c2.0
                        + t * t * t * to.0,
                    u * u * u * from.1
                        + 3.0 * u * u * t * c1.1
                        + 3.0 * u * t * t * c2.1
                        + t * t * t * to.1,
                )
            };
            const N: usize = 16;
            for i in 0..N {
                out.extend(hit_line(
                    o,
                    d,
                    at(i as f64 / N as f64),
                    at((i + 1) as f64 / N as f64),
                ));
            }
        }
    }
}

use super::geometry_box;

const EPS: f64 = 1e-6;

/// Ray × segment `a..b`: `o + t·d = a + s·(b−a)`, `t > eps`, `s ∈ [0, 1]`.
/// Cramer on `t·d − s·e = a − o`: both parameters divide by the same
/// determinant — negating one of them accepts the segment's *mirror* about
/// `a` and rejects true hits (the floating-datum bug).
fn hit_line(o: P, d: P, a: P, b: P) -> Option<Hit> {
    let e = (b.0 - a.0, b.1 - a.1);
    let denom = d.0 * e.1 - d.1 * e.0;
    if denom.abs() < 1e-12 {
        return None;
    }
    let ao = (a.0 - o.0, a.1 - o.1);
    let t = (ao.0 * e.1 - ao.1 * e.0) / denom;
    let s = (ao.0 * d.1 - ao.1 * d.0) / denom;
    (t > EPS && (-EPS..=1.0 + EPS).contains(&s)).then(|| Hit {
        t,
        tangent: super::geometry::unit(e),
        graze: !(EPS..=1.0 - EPS).contains(&s),
    })
}

/// Ray × circular arc: circle intersections filtered to the swept span.
#[allow(clippy::too_many_arguments)]
fn arc_crossings(o: P, d: P, from: P, to: P, r: f64, large: bool, sweep: bool, out: &mut Vec<Hit>) {
    let c = arc_center(from, to, r, large, sweep);
    let a0 = (from.1 - c.1).atan2(from.0 - c.0);
    let a1 = (to.1 - c.1).atan2(to.0 - c.0);
    // SVG's sweep flag is the positive-angle direction in screen coords
    // (y down), which is `atan2`'s increasing direction over raw coordinates.
    let s = if sweep { 1.0 } else { -1.0 };
    let span = ((a1 - a0) * s).rem_euclid(std::f64::consts::TAU);
    // A full-circle subpath folds to two semicircles, so span 0 means a
    // degenerate arc, not a full turn.
    for t in ray_circle(o, d, c, r) {
        let p = (o.0 + d.0 * t, o.1 + d.1 * t);
        let aq = (p.1 - c.1).atan2(p.0 - c.0);
        // Swept progress from the arc's start — on the arc within slack.
        let w = ((aq - a0) * s).rem_euclid(std::f64::consts::TAU);
        if w <= span + 1e-9 {
            out.push(Hit {
                t,
                // The circle's tangent at the hit — perpendicular to the radius.
                tangent: super::geometry::unit((-(p.1 - c.1), p.0 - c.0)),
                graze: w <= 1e-9 || w >= span - 1e-9,
            });
        }
    }
}

/// Both `t > eps` crossings of the ray with a circle.
fn ray_circle(o: P, d: P, c: P, r: f64) -> Vec<f64> {
    let f = (o.0 - c.0, o.1 - c.1);
    let a = d.0 * d.0 + d.1 * d.1;
    let b = 2.0 * (f.0 * d.0 + f.1 * d.1);
    let k = f.0 * f.0 + f.1 * f.1 - r * r;
    let disc = b * b - 4.0 * a * k;
    if disc < 0.0 || a < 1e-12 {
        return Vec::new();
    }
    let sq = disc.sqrt();
    [(-b - sq) / (2.0 * a), (-b + sq) / (2.0 * a)]
        .into_iter()
        .filter(|&t| t > EPS)
        .collect()
}

/// Ray × ellipse inscribed in `g` — scaled into the unit circle; the tangent
/// scales back out.
fn ellipse_crossings(o: P, d: P, g: Bbox, out: &mut Vec<Hit>) {
    let (cx, cy) = g.center();
    let (rx, ry) = ((g.w() / 2.0).max(1e-9), (g.h() / 2.0).max(1e-9));
    let so = ((o.0 - cx) / rx, (o.1 - cy) / ry);
    let sd = (d.0 / rx, d.1 / ry);
    for t in ray_circle(so, sd, (0.0, 0.0), 1.0) {
        let p = (so.0 + sd.0 * t, so.1 + sd.1 * t);
        out.push(Hit {
            t,
            // Unit-circle tangent (-py, px), stretched back into the ellipse.
            tangent: super::geometry::unit((-p.1 * rx, p.0 * ry)),
            // A full ellipse has no endpoints to graze.
            graze: false,
        });
    }
}

/// Where the ray `o + t·d` **leaves** an axis-aligned box — the exit `t`
/// (`0` when `o` is already outside and the ray points away). Leader texts
/// place just past the geometry union along their ray [SPEC 15.7].
pub(super) fn exit_box(o: P, d: P, g: Bbox) -> f64 {
    let mut t_exit = f64::INFINITY;
    for (oc, dc, lo, hi) in [(o.0, d.0, g.min_x, g.max_x), (o.1, d.1, g.min_y, g.max_y)] {
        if dc.abs() < 1e-12 {
            continue;
        }
        let (t1, t2) = ((lo - oc) / dc, (hi - oc) / dc);
        t_exit = t_exit.min(t1.max(t2));
    }
    if t_exit.is_finite() {
        t_exit.max(0.0)
    } else {
        0.0
    }
}

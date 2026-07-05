//! The drawn outline, and rays cast onto it [SPEC 15.2]: *the anchor aims;
//! the outline lands*. A leader's tip is the ray's **first crossing of the
//! drawn path** — a sketch's folded subpaths, an `|oval|`'s ellipse, a
//! `|line|` / `|poly|`'s points, any other shape's geometry rectangle; a
//! `pattern:` casts against every copy (the drawn path *is* the copies).
//! Everything is node-local; the caller transforms the ray through the
//! anchor's frame.

use super::super::ir::{Bbox, PlacedNode};
use super::chrome;
use super::geometry::{P, Seg, arc_center};
use crate::resolve::NodeKind;

/// The first crossing (smallest `t > eps`) of the ray `o + t·d` with the
/// node's drawn path, node-local. `None` when the ray misses entirely.
pub(super) fn raycast(node: &PlacedNode, o: P, d: P) -> Option<f64> {
    // A pattern carrier draws nothing itself — its copies are the path.
    if node.attrs.get("pattern").is_some() {
        return node
            .children
            .iter()
            .filter(|c| !chrome::is_chrome(&c.attrs))
            .filter_map(|c| raycast(c, (o.0 - c.cx, o.1 - c.cy), d))
            .min_by(f64::total_cmp);
    }
    if let Some(geo) = &node.sketch {
        return geo
            .outline
            .iter()
            .flat_map(|sub| sub.segs.iter())
            .filter_map(|seg| ray_seg(o, d, seg))
            .min_by(f64::total_cmp);
    }
    let g = geometry_box(node);
    match node.kind {
        NodeKind::Oval => ray_ellipse(o, d, g),
        NodeKind::Line | NodeKind::Poly => {
            let pts = super::super::primitives::attr_points(&node.attrs, "points", node.span)
                .ok()
                .flatten()?;
            let mut best: Option<f64> = None;
            let mut hit = |t: Option<f64>| {
                if let Some(t) = t {
                    best = Some(best.map_or(t, |b: f64| b.min(t)));
                }
            };
            for w in pts.windows(2) {
                hit(ray_line(o, d, w[0], w[1]));
            }
            if node.kind == NodeKind::Poly && pts.len() > 2 {
                hit(ray_line(o, d, pts[pts.len() - 1], pts[0]));
            }
            best
        }
        _ => ray_rect(o, d, g),
    }
}

/// The node's drawn shape, stroke excluded — the box the default outlines
/// (rect, ellipse) are read from [SPEC 15.1].
fn geometry_box(node: &PlacedNode) -> Bbox {
    let half = node.attrs.number("stroke-width").unwrap_or(0.0) / 2.0;
    node.bbox.inflate(-half)
}

const EPS: f64 = 1e-6;

/// Ray × one folded segment.
fn ray_seg(o: P, d: P, seg: &Seg) -> Option<f64> {
    match *seg {
        Seg::Line { from, to } => ray_line(o, d, from, to),
        Seg::Arc {
            from,
            to,
            r,
            large,
            sweep,
        } => ray_arc(o, d, from, to, r, large, sweep),
        // The advanced 10 % — flattened; drafting precision, not spline-exact.
        Seg::Cubic { from, c1, c2, to } => {
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
            (0..N)
                .filter_map(|i| {
                    ray_line(o, d, at(i as f64 / N as f64), at((i + 1) as f64 / N as f64))
                })
                .min_by(f64::total_cmp)
        }
    }
}

/// Ray × segment `a..b`: `o + t·d = a + s·(b−a)`, `t > eps`, `s ∈ [0, 1]`.
fn ray_line(o: P, d: P, a: P, b: P) -> Option<f64> {
    let e = (b.0 - a.0, b.1 - a.1);
    let denom = d.0 * e.1 - d.1 * e.0;
    if denom.abs() < 1e-12 {
        return None;
    }
    let ao = (a.0 - o.0, a.1 - o.1);
    let t = (ao.0 * e.1 - ao.1 * e.0) / denom;
    let s = (ao.0 * d.1 - ao.1 * d.0) / -denom;
    (t > EPS && (-EPS..=1.0 + EPS).contains(&s)).then_some(t)
}

/// Ray × circular arc: circle intersections filtered to the swept span.
fn ray_arc(o: P, d: P, from: P, to: P, r: f64, large: bool, sweep: bool) -> Option<f64> {
    let c = arc_center(from, to, r, large, sweep);
    let a0 = (from.1 - c.1).atan2(from.0 - c.0);
    let a1 = (to.1 - c.1).atan2(to.0 - c.0);
    // SVG's sweep flag is the positive-angle direction in screen coords
    // (y down), which is `atan2`'s increasing direction over raw coordinates.
    let s = if sweep { 1.0 } else { -1.0 };
    let span = ((a1 - a0) * s).rem_euclid(std::f64::consts::TAU);
    // A full-circle subpath folds to two semicircles, so span 0 means a
    // degenerate arc, not a full turn.
    let on_arc = |p: P| {
        let aq = (p.1 - c.1).atan2(p.0 - c.0);
        ((aq - a0) * s).rem_euclid(std::f64::consts::TAU) <= span + 1e-9
    };
    ray_circle(o, d, c, r)
        .into_iter()
        .filter(|&t| on_arc((o.0 + d.0 * t, o.1 + d.1 * t)))
        .min_by(f64::total_cmp)
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

/// Ray × ellipse inscribed in `g` — scaled into the unit circle.
fn ray_ellipse(o: P, d: P, g: Bbox) -> Option<f64> {
    let (cx, cy) = ((g.min_x + g.max_x) / 2.0, (g.min_y + g.max_y) / 2.0);
    let (rx, ry) = ((g.w() / 2.0).max(1e-9), (g.h() / 2.0).max(1e-9));
    let so = ((o.0 - cx) / rx, (o.1 - cy) / ry);
    let sd = (d.0 / rx, d.1 / ry);
    ray_circle(so, sd, (0.0, 0.0), 1.0)
        .into_iter()
        .min_by(f64::total_cmp)
}

/// Ray × rectangle boundary.
fn ray_rect(o: P, d: P, g: Bbox) -> Option<f64> {
    let corners = [
        (g.min_x, g.min_y),
        (g.max_x, g.min_y),
        (g.max_x, g.max_y),
        (g.min_x, g.max_y),
    ];
    (0..4)
        .filter_map(|i| ray_line(o, d, corners[i], corners[(i + 1) % 4]))
        .min_by(f64::total_cmp)
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

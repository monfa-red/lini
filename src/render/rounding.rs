//! Rounded-link geometry: a routed polyline plus its per-corner fillet radii
//! becomes a sequence of straight runs and arcs. Both the plain link path
//! ([`super::links`]) and the wavy link ([`super::wavy`]) build on this one
//! decomposition, so a corner's fillet is computed in exactly one place
//! (LINKING §Model step 7).

pub type Point = (f64, f64);

/// One piece of a rounded path. An [`Seg::Arc`] always follows the [`Seg::Line`]
/// running into its entry point, so walking the segments and emitting `L` / `A`
/// in order reproduces the classic rounded `d`.
pub enum Seg {
    Line {
        to: Point,
    },
    Arc {
        to: Point,
        center: Point,
        radius: f64,
        /// SVG sweep flag (matches the plain path's arc direction).
        sweep: u8,
    },
}

pub struct RoundedPath {
    pub start: Point,
    pub segs: Vec<Seg>,
}

/// Round each interior corner of the orthogonal polyline `pts` to its fillet
/// radius in `targets`, capped at half of each adjacent run so an arc never eats
/// a neighbour. A degenerate (sub-pixel) or collinear corner stays a sharp
/// joint. The legs are axis-aligned, so the Manhattan run length doubles as the
/// Euclidean one.
pub fn round(pts: &[Point], targets: &[f64]) -> RoundedPath {
    let mut segs = Vec::new();
    for i in 1..pts.len() - 1 {
        let (a, b, c) = (pts[i - 1], pts[i], pts[i + 1]);
        let (in_dx, in_dy) = (b.0 - a.0, b.1 - a.1);
        let (out_dx, out_dy) = (c.0 - b.0, c.1 - b.1);
        let in_len = in_dx.abs() + in_dy.abs();
        let out_len = out_dx.abs() + out_dy.abs();
        let r = targets
            .get(i - 1)
            .copied()
            .unwrap_or(0.0)
            .min(in_len / 2.0)
            .min(out_len / 2.0);
        let cross = in_dx * out_dy - in_dy * out_dx;
        if r < 0.5 || cross == 0.0 {
            segs.push(Seg::Line { to: b });
            continue;
        }
        let (ux, uy) = (in_dx / in_len, in_dy / in_len);
        let (vx, vy) = (out_dx / out_len, out_dy / out_len);
        let enter = (b.0 - ux * r, b.1 - uy * r);
        segs.push(Seg::Line { to: enter });
        segs.push(Seg::Arc {
            to: (b.0 + vx * r, b.1 + vy * r),
            // The arc centre sits a radius off the entry, perpendicular into
            // the turn — equivalently a radius off the exit along the inbound
            // leg, since the legs are perpendicular.
            center: (enter.0 + vx * r, enter.1 + vy * r),
            radius: r,
            sweep: if cross > 0.0 { 1 } else { 0 },
        });
    }
    segs.push(Seg::Line {
        to: *pts.last().expect("a link polyline has at least two points"),
    });
    RoundedPath {
        start: pts[0],
        segs,
    }
}

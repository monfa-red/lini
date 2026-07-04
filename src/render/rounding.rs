//! Rounded-link geometry: a routed polyline plus its per-corner fillet radii
//! becomes a sequence of straight runs and arcs. Both the plain link path
//! ([`super::links`]) and the wavy link ([`super::wavy`]) build on this one
//! decomposition, so a corner's fillet is computed in exactly one place
//! (ROUTING Model step 7).

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

/// The SVG path `d` for a polyline with each interior corner rounded to its
/// `targets` radius — the **one** formatter, shared by routed links
/// ([`super::links`]) and a lowered `|line|` primitive's rounded corners
/// ([`super::primitives`]), so a self-message hook bends exactly like a wire.
pub fn path_d(pts: &[Point], targets: &[f64]) -> String {
    use super::values::num;
    use std::fmt::Write;
    let rounded = round(pts, targets);
    let mut d = format!("M {} {}", num(rounded.start.0), num(rounded.start.1));
    for seg in &rounded.segs {
        match seg {
            Seg::Line { to } => write!(d, " L {} {}", num(to.0), num(to.1)).unwrap(),
            Seg::Arc {
                to, radius, sweep, ..
            } => write!(
                d,
                " A {r} {r} 0 0 {sweep} {} {}",
                num(to.0),
                num(to.1),
                r = num(*radius),
            )
            .unwrap(),
        }
    }
    d
}

/// Round each interior corner of the orthogonal polyline `pts` to its fillet
/// radius in `targets`, kept feasible per leg: a leg is shared by the corners
/// at its two ends, so their arcs together may fill it — when the pair
/// over-fills, both scale in proportion to their desires (a squeezed nest
/// keeps its pitch uniform instead of collapsing the smaller arc), and a
/// terminal leg belongs to its one corner whole (marker pull-back already
/// shortened it). A degenerate (sub-pixel) or collinear corner stays a sharp
/// joint. The legs are axis-aligned, so the Manhattan run length doubles as
/// the Euclidean one.
pub fn round(pts: &[Point], targets: &[f64]) -> RoundedPath {
    let corners = pts.len().saturating_sub(2);
    let target = |k: usize| targets.get(k).copied().unwrap_or(0.0);
    let leg = |i: usize| (pts[i + 1].0 - pts[i].0).abs() + (pts[i + 1].1 - pts[i].1).abs();
    let radius = |k: usize| {
        let t = target(k);
        if t <= 0.0 {
            return 0.0;
        }
        let before = if k == 0 { 0.0 } else { target(k - 1) };
        let after = if k + 1 < corners { target(k + 1) } else { 0.0 };
        let f = (leg(k) / (before + t)).min(leg(k + 1) / (t + after));
        t * f.min(1.0)
    };
    let mut segs = Vec::new();
    for i in 1..pts.len() - 1 {
        let (a, b, c) = (pts[i - 1], pts[i], pts[i + 1]);
        let (in_dx, in_dy) = (b.0 - a.0, b.1 - a.1);
        let (out_dx, out_dy) = (c.0 - b.0, c.1 - b.1);
        let in_len = in_dx.abs() + in_dy.abs();
        let out_len = out_dx.abs() + out_dy.abs();
        let r = radius(i - 1);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn radii(pts: &[Point], targets: &[f64]) -> Vec<f64> {
        round(pts, targets)
            .segs
            .iter()
            .filter_map(|s| match s {
                Seg::Arc { radius, .. } => Some(*radius),
                Seg::Line { .. } => None,
            })
            .collect()
    }

    #[test]
    fn corners_share_a_legs_length_in_proportion() {
        // Both corners want 30 on a 40-long shared leg: they split it evenly.
        let pts = [(0.0, 0.0), (40.0, 0.0), (40.0, 40.0), (80.0, 40.0)];
        assert_eq!(radii(&pts, &[30.0, 30.0]), vec![20.0, 20.0]);
        // Unequal desires split in proportion — the nest's constant pitch
        // scales instead of collapsing onto the smaller corner.
        let pts = [(0.0, 0.0), (60.0, 0.0), (60.0, 40.0), (120.0, 40.0)];
        assert_eq!(radii(&pts, &[10.0, 30.0]), vec![10.0, 30.0]);
        let pts = [(0.0, 0.0), (60.0, 0.0), (60.0, 20.0), (120.0, 20.0)];
        assert_eq!(radii(&pts, &[10.0, 30.0]), vec![5.0, 15.0]);
    }

    #[test]
    fn a_lone_corner_may_take_a_whole_terminal_leg() {
        // The S-bend's big first arc: 40 fits the 64-long terminal leg and
        // pairs with the far corner's 10 on the 56-long shared leg — the old
        // half-leg rule squashed it to 28 for no reason.
        let pts = [(0.0, 0.0), (64.0, 0.0), (64.0, 56.0), (100.0, 56.0)];
        assert_eq!(radii(&pts, &[40.0, 10.0]), vec![40.0, 10.0]);
    }
}

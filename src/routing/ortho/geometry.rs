//! Route geometry — chains lowered to orthogonal polylines, and the stray
//! segment for links no law can draw.
//!
//! The polyline construction (runs + placed ordinates → corners, collinear
//! merge, jogs, self-loops) lands with the pipeline driver
//! (ROUTING-V2.md stage 4); the stray is already the report made visible.

use super::rect::Rect;

/// The stray segment for an impossible link (ROUTING.md §Impossible layouts):
/// centre to centre, each end trimmed to its own body's boundary. `None` when
/// the trim leaves nothing — coincident or overlapping bodies (self-loops,
/// containment), where no between-bodies segment exists.
pub fn stray_segment(a: Rect, b: Rect) -> Option<((f64, f64), (f64, f64))> {
    let centre = |r: Rect| ((r.x0 + r.x1) / 2.0, (r.y0 + r.y1) / 2.0);
    let (ca, cb) = (centre(a), centre(b));
    let d = (cb.0 - ca.0, cb.1 - ca.1);
    if d == (0.0, 0.0) {
        return None;
    }
    // Parameter along ca→cb at which a ray from a rect's centre exits it.
    let exit = |r: Rect, o: (f64, f64), d: (f64, f64)| {
        let along = |lo: f64, hi: f64, o: f64, d: f64| {
            if d > 0.0 {
                (hi - o) / d
            } else if d < 0.0 {
                (lo - o) / d
            } else {
                f64::INFINITY
            }
        };
        along(r.x0, r.x1, o.0, d.0).min(along(r.y0, r.y1, o.1, d.1))
    };
    let t0 = exit(a, ca, d);
    let t1 = 1.0 - exit(b, cb, (-d.0, -d.1));
    (t0 < t1).then_some((
        (ca.0 + d.0 * t0, ca.1 + d.1 * t0),
        (ca.0 + d.0 * t1, ca.1 + d.1 * t1),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stray_trims_to_both_boundaries() {
        // Facing horizontally: the segment runs face to face on the centreline.
        let a = Rect::new(0.0, 0.0, 40.0, 40.0);
        let b = Rect::new(100.0, 0.0, 140.0, 40.0);
        assert_eq!(stray_segment(a, b), Some(((40.0, 20.0), (100.0, 20.0))));
        // Diagonal neighbours: a slanted segment, trimmed where the
        // centre-to-centre ray leaves each body.
        let c = Rect::new(100.0, 100.0, 140.0, 140.0);
        let (p, q) = stray_segment(a, c).expect("segment");
        assert_eq!(p, (40.0, 40.0));
        assert_eq!(q, (100.0, 100.0));
    }

    #[test]
    fn stray_skips_degenerate_pairs() {
        let a = Rect::new(0.0, 0.0, 40.0, 40.0);
        assert_eq!(stray_segment(a, a), None);
        // One body inside the other: no between-bodies segment exists.
        let inner = Rect::new(10.0, 10.0, 20.0, 20.0);
        assert_eq!(stray_segment(a, inner), None);
    }
}

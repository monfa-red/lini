//! The chart's data→pixel projection [SPEC 14.7]. This is the **seam** every
//! series and axis builder lowers through, so `direction: row` and the polar (radial)
//! projection are variants here — not a rewrite of the callers. The joint
//! [`Plot::project`] maps a (domain, value) datum to a pixel point in any direction;
//! `column` (the default) runs the domain left→right and the value up (SVG y is down, so
//! larger values sit at smaller y), `row` swaps them, `radial` bends the domain into a
//! ring of spokes and the value into a radius.

use super::scale::Scale;
use std::f64::consts::TAU;

type P = (f64, f64);

/// The chart's orientation [SPEC 14.7]. `column`/`row` are cartesian (the value
/// grows up / right); `radial` is polar (the value grows outward from the centre).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Column,
    Row,
    Radial,
}

/// The laid-out plot rectangle (chart-local pixels, origin at the chart centre). For a
/// radial chart it is the square bounding box of the spoke circle.
pub struct Plot {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    pub dir: Dir,
}

impl Plot {
    pub fn w(&self) -> f64 {
        self.x1 - self.x0
    }

    pub fn h(&self) -> f64 {
        self.y1 - self.y0
    }

    /// A datum at domain coordinate `x` (on the x scale) and value `v` (on a value
    /// scale) → its pixel point, in this chart's direction [SPEC 14.7]. This is
    /// the one projection every series lowers through, so a radar reuses the exact
    /// `|line|` / `|area|` / `|dots|` builders — only the projector differs.
    pub fn project(&self, x: &Scale, xv: f64, value: &Scale, v: f64) -> P {
        match self.dir {
            Dir::Column => (self.domain_at(x, xv), self.value_at(value, v)),
            // Row: the value runs left→right, the domain top→bottom down the left.
            Dir::Row => (self.value_at(value, v), self.domain_at(x, xv)),
            // Radial: the domain is a spoke angle (from the top, clockwise), the value a
            // radius from the centre.
            Dir::Radial => {
                let (cx, cy) = self.center();
                let theta = self.spoke_angle(x, xv);
                let r = value.frac(v) * self.radius();
                (cx + r * theta.sin(), cy - r * theta.cos())
            }
        }
    }

    pub fn is_radial(&self) -> bool {
        self.dir == Dir::Radial
    }

    /// The plot centre — the pole of a radial chart.
    pub fn center(&self) -> P {
        ((self.x0 + self.x1) / 2.0, (self.y0 + self.y1) / 2.0)
    }

    /// The rim radius of a radial chart (its square rect's half-side).
    pub fn radius(&self) -> f64 {
        self.w().min(self.h()) / 2.0
    }

    /// The angle of spoke / domain coordinate `xv` [SPEC 14.7]: `0` straight up,
    /// increasing clockwise, one full turn over the `n` band slots.
    pub fn spoke_angle(&self, x: &Scale, xv: f64) -> f64 {
        let n = match x {
            Scale::Band { n } => *n as f64,
            _ => 1.0,
        };
        TAU * xv / n.max(1.0)
    }

    /// The pixel of domain coordinate `v` along whichever screen axis the
    /// domain runs in this direction — x in a column chart, y down the left
    /// in a row [SPEC 14.7]. Cartesian only.
    pub fn domain_at(&self, x: &Scale, v: f64) -> f64 {
        match self.dir {
            Dir::Row => self.y0 + x.frac(v) * self.h(),
            _ => self.x_at(x, v),
        }
    }

    /// The pixel of `v` on a value scale along its screen axis — y in a
    /// column chart (larger values up), x in a row (larger values right).
    pub fn value_at(&self, s: &Scale, v: f64) -> f64 {
        match self.dir {
            Dir::Row => self.x0 + s.frac(v) * self.w(),
            _ => self.y_at(s, v),
        }
    }

    /// The full-plot segment crossing pixel `p` of an axis: a vertical line
    /// when the axis runs horizontally, a horizontal one otherwise.
    pub fn cross(&self, axis_horizontal: bool, p: f64) -> Vec<P> {
        if axis_horizontal {
            vec![(p, self.y0), (p, self.y1)]
        } else {
            vec![(self.x0, p), (self.x1, p)]
        }
    }

    /// The x pixel of domain coordinate `v` on `x` (a band index, or a numeric x).
    pub fn x_at(&self, x: &Scale, v: f64) -> f64 {
        self.x0 + x.frac(v) * self.w()
    }

    /// The y pixel of `value` on a value scale (0/min at the baseline, max at top).
    pub fn y_at(&self, value: &Scale, v: f64) -> f64 {
        self.y1 - value.frac(v) * self.h()
    }

    /// The x-pixel edges of band slot `i` (for a bar's footprint).
    pub fn slot_px(&self, x: &Scale, i: usize) -> (f64, f64) {
        let (f0, f1) = x.slot(i);
        (self.x0 + f0 * self.w(), self.x0 + f1 * self.w())
    }

    /// Clip a pixel polyline to the plot rect (Liang–Barsky per segment), so data
    /// outside an axis `range:` is cropped to the plot [SPEC 14.1]. Splits
    /// into runs where the line re-enters the rect.
    pub fn clip(&self, points: &[P]) -> Vec<Vec<P>> {
        let mut runs: Vec<Vec<P>> = Vec::new();
        let mut cur: Vec<P> = Vec::new();
        for w in points.windows(2) {
            if let Some((a, b, b_in)) = self.clip_segment(w[0], w[1]) {
                if cur.is_empty() {
                    cur.push(a);
                } else if cur.last() != Some(&a) {
                    // The previous segment left the rect: start a fresh run.
                    runs.push(std::mem::take(&mut cur));
                    cur.push(a);
                }
                cur.push(b);
                if !b_in {
                    // This segment exits the rect: end the run here.
                    runs.push(std::mem::take(&mut cur));
                }
            }
        }
        if !cur.is_empty() {
            runs.push(cur);
        }
        runs.retain(|r| r.len() >= 2);
        runs
    }

    /// Liang–Barsky clip of one segment to the plot rect. Returns the clipped
    /// endpoints and whether the segment reached its original end (the line
    /// continues there).
    fn clip_segment(&self, p0: P, p1: P) -> Option<(P, P, bool)> {
        let (t0, t1) = liang_barsky(p0, p1, (self.x0, self.y0), (self.x1, self.y1))?;
        let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
        let a = (p0.0 + t0 * dx, p0.1 + t0 * dy);
        let b = (p0.0 + t1 * dx, p0.1 + t1 * dy);
        Some((a, b, t1 == 1.0))
    }
}

/// Liang–Barsky: the parameter window `[t0, t1]` of segment `p0`→`p1` that lies
/// inside the axis-aligned rect `[min, max]`, or `None` if the segment misses it
/// (parallel-and-outside, or the window closes). `clip_segment` reads the
/// clipped endpoints off it; label placement only asks `is_some()` [SPEC 14.9].
pub(super) fn liang_barsky(p0: P, p1: P, min: P, max: P) -> Option<(f64, f64)> {
    let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
    let checks = [
        (-dx, p0.0 - min.0),
        (dx, max.0 - p0.0),
        (-dy, p0.1 - min.1),
        (dy, max.1 - p0.1),
    ];
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for (p, q) in checks {
        if p.abs() < 1e-9 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    (t0 <= t1).then_some((t0, t1))
}

//! The chart's data→pixel projection ([CHARTS.md] §11). This is the **seam** every
//! series and axis builder lowers through, so `direction: row` and the polar (radial)
//! projection are variants here — not a rewrite of the callers. The joint
//! [`Plot::project`] maps a (domain, value) datum to a pixel point in any direction;
//! `column` (the default) runs the domain left→right and the value up (SVG y is down, so
//! larger values sit at smaller y), `row` swaps them, `radial` bends the domain into a
//! ring of spokes and the value into a radius.

use super::scale::Scale;
use std::f64::consts::TAU;

type P = (f64, f64);

/// The chart's orientation ([CHARTS.md] §11). `column`/`row` are cartesian (the value
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
    /// scale) → its pixel point, in this chart's direction ([CHARTS.md] §11). This is
    /// the one projection every series lowers through, so a radar reuses the exact
    /// `|line|` / `|area|` / `|dots|` builders — only the projector differs.
    pub fn project(&self, x: &Scale, xv: f64, value: &Scale, v: f64) -> P {
        match self.dir {
            Dir::Column => (self.x_at(x, xv), self.y_at(value, v)),
            // Row: the value runs left→right, the domain top→bottom down the left.
            Dir::Row => (
                self.x0 + value.frac(v) * self.w(),
                self.y0 + x.frac(xv) * self.h(),
            ),
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

    /// The angle of spoke / domain coordinate `xv` ([CHARTS.md] §12): `0` straight up,
    /// increasing clockwise, one full turn over the `n` band slots.
    pub fn spoke_angle(&self, x: &Scale, xv: f64) -> f64 {
        let n = match x {
            Scale::Band { n } => *n as f64,
            _ => 1.0,
        };
        TAU * xv / n.max(1.0)
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
    /// outside an axis `range:` is cropped to the plot ([CHARTS.md] §1/§6). Splits
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

    /// Liang–Barsky clip of one segment to the rect. Returns the clipped endpoints
    /// and whether the segment reached its original end (the line continues there).
    fn clip_segment(&self, p0: P, p1: P) -> Option<(P, P, bool)> {
        let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
        let dx = p1.0 - p0.0;
        let dy = p1.1 - p0.1;
        let checks = [
            (-dx, p0.0 - self.x0),
            (dx, self.x1 - p0.0),
            (-dy, p0.1 - self.y0),
            (dy, self.y1 - p0.1),
        ];
        for (p, q) in checks {
            if p.abs() < 1e-12 {
                if q < 0.0 {
                    return None; // parallel and outside
                }
            } else {
                let r = q / p;
                if p < 0.0 {
                    if r > t1 {
                        return None;
                    }
                    if r > t0 {
                        t0 = r;
                    }
                } else {
                    if r < t0 {
                        return None;
                    }
                    if r < t1 {
                        t1 = r;
                    }
                }
            }
        }
        let a = (p0.0 + t0 * dx, p0.1 + t0 * dy);
        let b = (p0.0 + t1 * dx, p0.1 + t1 * dy);
        Some((a, b, t1 == 1.0))
    }
}

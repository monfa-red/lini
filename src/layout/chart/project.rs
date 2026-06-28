//! The chart's data→pixel projection ([CHARTS.md] §11). This is the **seam** every
//! series and axis builder lowers through, so adding `direction: row` and the polar
//! (radial) projection in a later step is a new variant here — not a rewrite of the
//! callers. Step 1 is the cartesian **column** case: categories run left→right along
//! x, values grow up the y axis (SVG y is down, so larger values sit at smaller y).

/// A laid-out plot area plus its category count and value domain — the cartesian
/// column projection. Pixel coords are chart-local (origin at the chart centre).
pub struct Plot {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    /// Number of category slots along the x axis.
    pub n: usize,
    /// Value-axis domain maximum (the minimum is 0 for bars).
    pub vmax: f64,
}

impl Plot {
    pub fn w(&self) -> f64 {
        self.x1 - self.x0
    }

    pub fn h(&self) -> f64 {
        self.y1 - self.y0
    }

    /// The y pixel of value 0 — the bar baseline / value axis foot.
    pub fn baseline(&self) -> f64 {
        self.y1
    }

    /// Project a value to its y pixel (0 at the baseline, `vmax` at the plot top).
    pub fn y(&self, value: f64) -> f64 {
        if self.vmax <= 0.0 {
            self.y1
        } else {
            self.y1 - (value / self.vmax) * self.h()
        }
    }

    /// The width of one category slot.
    pub fn slot_w(&self) -> f64 {
        if self.n == 0 {
            self.w()
        } else {
            self.w() / self.n as f64
        }
    }

    /// The x pixel at the centre of category slot `i`.
    pub fn slot_center(&self, i: usize) -> f64 {
        self.x0 + (i as f64 + 0.5) * self.slot_w()
    }
}

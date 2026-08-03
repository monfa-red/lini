//! Plane geometry the layout engines share: a point, projections onto a
//! direction, and an **oriented frame** — the local `(along, across)` basis a
//! seated band or a dimension line is expressed in. Engine-neutral: the
//! drawing's ISO reading conventions and the schematic's pin directions both
//! build their frames from here.

use super::ir::Bbox;

pub type P = (f64, f64);

/// The `[min, max]` projection of a point set onto a unit direction.
pub fn proj(pts: &[P], dir: P) -> (f64, f64) {
    pts.iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), c| {
            let t = c.0 * dir.0 + c.1 * dir.1;
            (lo.min(t), hi.max(t))
        })
}

/// The `[min, max]` projection of a box's corners onto a unit direction.
pub fn project(geo: Bbox, dir: P) -> (f64, f64) {
    proj(
        &[
            (geo.min_x, geo.min_y),
            (geo.max_x, geo.min_y),
            (geo.min_x, geo.max_y),
            (geo.max_x, geo.max_y),
        ],
        dir,
    )
}

/// An oriented frame: `u` runs along the line, `n` across it. The drawing's
/// dims read it as the measure frame — **−n is the ISO reading's "above the
/// line"** [SPEC 15.6] — and the stack packer as the seat's local basis; the
/// engine-flavoured constructors live with their engines
/// (`layout::drawing::geometry` builds the axis and aligned frames).
#[derive(Clone, Copy, PartialEq)]
pub struct Frame {
    pub u: P,
    pub n: P,
}

impl Frame {
    /// The frame that stacks **outward along `n`** — `u` is its perpendicular,
    /// so both stay exact unit vectors for an axis direction and a seat's
    /// arithmetic matches the axis-matched form byte for byte. The schematic's
    /// satellites grow along a pin's outward normal through this.
    pub fn outward(n: P) -> Frame {
        Frame { u: (n.1, -n.0), n }
    }

    /// The coordinate along the line.
    pub fn u(&self, p: P) -> f64 {
        p.0 * self.u.0 + p.1 * self.u.1
    }

    /// The coordinate across the line.
    pub fn cross(&self, p: P) -> f64 {
        p.0 * self.n.0 + p.1 * self.n.1
    }

    /// Frame coordinates back to the drawing plane.
    pub fn pt(&self, u: f64, c: f64) -> P {
        (u * self.u.0 + c * self.n.0, u * self.u.1 + c * self.n.1)
    }
}

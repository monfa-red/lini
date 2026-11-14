//! The `|cyl|` silhouette [SPEC 7]: a body capped by an end ellipse at each
//! end, the full rim on the end it **faces** — `facing: up` (the default) is
//! the upright drum, `right` lays it on its side as the `|queue|` [SPEC 8].
//! One geometry, two readers: layout leans the label off the rim and the
//! renderer draws the outline, so the two never disagree on which end is which.

use super::geom::{self, P};
use crate::resolve::{AttrMap, ResolvedValue};

/// The rim's depth — the end ellipse's half-axis along the cylinder — as a
/// fraction of the **shorter** side: a turned cylinder is then the exact
/// transpose of an upright one, and a long one never grows a cap that crowds
/// its label.
const RIM_DEPTH: f64 = 0.1;
/// The floor that keeps a tiny cylinder's rim an ellipse, not a line.
const RIM_DEPTH_MIN: f64 = 2.0;
/// How far a text-only label leans away from the rim, in rim depths: the rim
/// crowds its end, so the body's optical centre sits off the bbox centre.
const LABEL_LEAN: f64 = 0.3;

pub(crate) struct Rim {
    /// Unit vector from the centre toward the full rim.
    pub dir: P,
    pub depth: f64,
}

impl Rim {
    /// The rim of a `w × h` cylinder wearing `attrs`.
    pub(crate) fn of(attrs: &AttrMap, w: f64, h: f64) -> Rim {
        let authored = match attrs.get("facing") {
            Some(ResolvedValue::Ident(f)) => geom::facing(f),
            _ => None,
        };
        Rim {
            dir: authored.unwrap_or((0.0, -1.0)),
            depth: (w.min(h) * RIM_DEPTH).max(RIM_DEPTH_MIN),
        }
    }

    /// Where a text-only label sits relative to the bbox centre.
    pub(crate) fn label_lean(&self) -> P {
        geom::scale(self.dir, -LABEL_LEAN * self.depth)
    }
}

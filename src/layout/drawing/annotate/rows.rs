//! The dimension-row packer [SPEC 15.6]: dims sharing a side pack into rows a pitch apart, each taking the innermost row whose span clears what is already placed and any registered obstacle.

use super::*;

/// The row packer [SPEC 15.6]: dims sharing a side pack into rows `DIM_PITCH`
/// apart, the first `DIM_OFFSET` from the geometry's extent; each dim, in
/// source order, takes the innermost row where its span — text included —
/// overlaps nothing already placed. `gap:` pins a dim's own offset instead
/// (still recorded, so packed rows avoid it).
pub(in crate::layout::drawing) struct Rows {
    extent: Bbox,
    placed: Vec<(Side, f64, (f64, f64))>,
    /// World boxes a row must clear — the texts of leaders, callouts, and
    /// angles, registered before dims pack ([SPEC 15.6]).
    obstacles: Vec<Bbox>,
}

impl Rows {
    pub(super) fn new(extent: Bbox) -> Rows {
        Rows {
            extent,
            placed: Vec::new(),
            obstacles: Vec::new(),
        }
    }

    /// Register a lowered statement's texts as boxes the packed rows dodge.
    pub(super) fn obstruct_texts(&mut self, nodes: &[PlacedNode]) {
        for n in nodes.iter().filter(|n| n.kind == NodeKind::Text) {
            self.obstacles
                .push(Bbox::extent_of(std::slice::from_ref(n), |_| true));
        }
    }

    /// Seat a dim occupying `interval` on `side`; returns the dimension
    /// line's world coordinate along the stack axis.
    pub fn seat(&mut self, side: Side, interval: (f64, f64), pinned: Option<f64>) -> f64 {
        let off = pinned.unwrap_or_else(|| {
            (0..)
                .map(|k| DIM_OFFSET + k as f64 * DIM_PITCH)
                .find(|cand| {
                    let row_clash = self.placed.iter().any(|(s, o, iv)| {
                        *s == side
                            && (o - cand).abs() < DIM_PITCH - 1e-6
                            && iv.0 < interval.1 - 1e-6
                            && iv.1 > interval.0 + 1e-6
                    });
                    !row_clash && !self.blocked(side, *cand, interval)
                })
                .expect("an offset frees up")
        });
        self.placed.push((side, off, interval));
        self.line_at(side, off)
    }

    fn line_at(&self, side: Side, off: f64) -> f64 {
        match side {
            Side::Bottom => self.extent.max_y + off,
            Side::Top => self.extent.min_y - off,
            Side::Right => self.extent.max_x + off,
            Side::Left => self.extent.min_x - off,
        }
    }

    /// Whether a candidate row's band — the dim line plus the value riding
    /// above it — would land on a registered obstacle.
    fn blocked(&self, side: Side, off: f64, interval: (f64, f64)) -> bool {
        let line_c = self.line_at(side, off);
        // Text lift (fs/2 + 2) + half the text height above the line;
        // overshoot below it.
        let (lo, hi) = (line_c - 14.0, line_c + EXT_OVERSHOOT + 1.0);
        let band = match side {
            Side::Top | Side::Bottom => Bbox {
                min_x: interval.0,
                max_x: interval.1,
                min_y: lo,
                max_y: hi,
            },
            Side::Left | Side::Right => Bbox {
                min_x: lo,
                max_x: hi,
                min_y: interval.0,
                max_y: interval.1,
            },
        };
        self.obstacles.iter().any(|o| o.overlaps(band))
    }
}

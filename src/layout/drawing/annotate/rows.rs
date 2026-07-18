//! The dimension-row packer [SPEC 15.6]: row offsets derive from painted
//! bounds — a row stands `clearance` off everything already painted on its
//! side (geometry, callout texts, earlier rows), never at a fixed pitch.

use super::*;

/// The row packer [SPEC 15.6]: dims sharing a side pack into rows, each — in
/// source order — seating at the innermost offset where its painted band
/// stands its `clearance` off everything already painted there: the geometry
/// extent, registered obstacle texts, and earlier rows' bands. `clearance` is
/// a minimum, not a coordinate — the packer goes farther out to clear.
pub(in crate::layout::drawing) struct Rows {
    extent: Bbox,
    /// Everything a later row must stand `clearance` off — the texts of
    /// leaders, callouts, and angles (registered before dims pack), then each
    /// seated row's own painted band ([SPEC 15.6]).
    painted: Vec<Bbox>,
}

/// A row's painted band along the stack (cross) axis, relative to its dim
/// line in **world direction**: `neg` reaches toward smaller coordinates —
/// the ISO value always rides above / left of the line — and `pos` toward
/// larger ones. The extension springs are excluded: they connect the row to
/// its anchors by design and cross freely.
struct Band {
    neg: f64,
    pos: f64,
}

impl Band {
    fn of(side: Side, fs: f64, sw: f64) -> Band {
        let arrow = ARROW_HALF * sw;
        Band {
            // Text lift (fs/2 + 2) plus half the text height.
            neg: fs + 2.0,
            // The extension overshoot runs outward past the line — beyond it
            // only on the sides whose outward is the positive direction; the
            // arrowheads spread `ARROW_HALF · sw` either way.
            pos: match side {
                Side::Bottom | Side::Right => EXT_OVERSHOOT.max(arrow),
                Side::Top | Side::Left => arrow,
            },
        }
    }

    /// The band's reach toward the geometry — its innermost ink.
    fn inner(&self, side: Side) -> f64 {
        match side {
            Side::Bottom | Side::Right => self.neg,
            Side::Top | Side::Left => self.pos,
        }
    }
}

impl Rows {
    pub(super) fn new(extent: Bbox) -> Rows {
        Rows {
            extent,
            painted: Vec::new(),
        }
    }

    /// Register a lowered statement's texts — and a datum's framed box, whose
    /// linework reaches past its letter [SPEC 15.7] — as painted bounds the
    /// rows clear.
    pub(super) fn obstruct_texts(&mut self, nodes: &[PlacedNode]) {
        for n in nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Text || n.type_chain.iter().any(|t| t == "datum-frame"))
        {
            self.painted
                .push(Bbox::extent_of(std::slice::from_ref(n), |_| true));
        }
    }

    /// Seat a dim occupying `interval` on `side`, standing at least
    /// `clearance` off everything already painted; returns the dimension
    /// line's world coordinate along the stack axis.
    pub fn seat(
        &mut self,
        side: Side,
        interval: (f64, f64),
        clearance: f64,
        fs: f64,
        sw: f64,
    ) -> f64 {
        let band = Band::of(side, fs, sw);
        // Innermost candidate: the band's nearest ink `clearance` off the
        // geometry extent's edge.
        let mut off = clearance + band.inner(side);
        // Push outward past whatever the band (grown by the clearance along
        // the stack axis) still lands on — each pass clears at least one
        // painted box for good, so the loop is bounded.
        for _ in 0..=self.painted.len() {
            let probe = grow_cross(self.band_box(side, off, interval, &band), side, clearance);
            let push = self
                .painted
                .iter()
                .filter(|p| p.overlaps(probe))
                .map(|p| self.past(side, *p, clearance) + band.inner(side))
                .fold(f64::NEG_INFINITY, f64::max);
            if push > off + 1e-9 {
                off = push;
            } else {
                break;
            }
        }
        self.painted.push(self.band_box(side, off, interval, &band));
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

    /// The row's painted band as a world box: its interval along the row, its
    /// band reach across it. The interval pulls in a hair so rows abutting
    /// tip-to-tip — a drafting norm — still share.
    fn band_box(&self, side: Side, off: f64, interval: (f64, f64), band: &Band) -> Bbox {
        let line = self.line_at(side, off);
        let (lo, hi) = (line - band.neg, line + band.pos);
        let (i0, i1) = (interval.0 + 1e-6, interval.1 - 1e-6);
        match side {
            Side::Top | Side::Bottom => Bbox {
                min_x: i0,
                max_x: i1,
                min_y: lo,
                max_y: hi,
            },
            Side::Left | Side::Right => Bbox {
                min_x: lo,
                max_x: hi,
                min_y: i0,
                max_y: i1,
            },
        }
    }

    /// The offset that stands a band's innermost ink `clearance` beyond a
    /// painted box's outer edge (the band reach is added by the caller).
    fn past(&self, side: Side, p: Bbox, clearance: f64) -> f64 {
        clearance
            + match side {
                Side::Bottom => p.max_y - self.extent.max_y,
                Side::Top => self.extent.min_y - p.min_y,
                Side::Right => p.max_x - self.extent.max_x,
                Side::Left => self.extent.min_x - p.min_x,
            }
    }
}

/// A band box grown by the clearance along the stack axis only — rows keep
/// their stand-off across the stack; along the row they may abut.
fn grow_cross(b: Bbox, side: Side, clearance: f64) -> Bbox {
    match side {
        Side::Top | Side::Bottom => Bbox {
            min_y: b.min_y - clearance,
            max_y: b.max_y + clearance,
            ..b
        },
        Side::Left | Side::Right => Bbox {
            min_x: b.min_x - clearance,
            max_x: b.max_x + clearance,
            ..b
        },
    }
}

//! The dimension-row packer [SPEC 15.6]: row offsets derive from painted
//! bounds — a row stands `clearance` off everything already painted on its
//! side (geometry, callout texts, earlier rows), never at a fixed pitch.
//! One seating law for every dim: a side row stacks outward from the
//! geometry extent's edge, an **aligned** dim from its own span's outermost
//! anchor — both along a [`SeatLine`], the row's frame plus its outward
//! direction and base.

use super::super::dims::Frame;
use super::super::geometry::P;
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

/// Where a row seats [SPEC 15.6]: its dim line runs along `frame.u` and
/// stacks outward along `sgn · frame.n`, starting from `base` — the
/// out-projected cross coordinate of what the row stands off (the geometry
/// extent's edge for a side row, the span's outermost anchor for an aligned
/// dim). "o" coordinates below are the cross coordinate times `sgn`, so
/// outward is always positive.
pub(in crate::layout::drawing) struct SeatLine {
    frame: Frame,
    sgn: f64,
    base: f64,
}

impl SeatLine {
    /// A side row's seat: the axis frame, outward off the extent's edge.
    fn side(side: Side, extent: Bbox) -> SeatLine {
        let axis = match side {
            Side::Top | Side::Bottom => Axis::Horizontal,
            Side::Left | Side::Right => Axis::Vertical,
        };
        let (sgn, base) = match side {
            Side::Bottom => (1.0, extent.max_y),
            Side::Top => (-1.0, -extent.min_y),
            Side::Right => (1.0, extent.max_x),
            Side::Left => (-1.0, -extent.min_x),
        };
        SeatLine {
            frame: Frame::axis(axis),
            sgn,
            base,
        }
    }

    /// An aligned dim's seat [SPEC 15.6]: its own frame, outward on the away
    /// side (`away_pos` — the +n side), off the span's outermost anchor.
    pub(in crate::layout::drawing) fn span(frame: Frame, away_pos: bool, ends: (P, P)) -> SeatLine {
        let sgn = if away_pos { 1.0 } else { -1.0 };
        let base = (sgn * frame.cross(ends.0)).max(sgn * frame.cross(ends.1));
        SeatLine { frame, sgn, base }
    }

    /// The dim line's world cross coordinate at `off` outward from base.
    fn line(&self, off: f64) -> f64 {
        self.sgn * (self.base + off)
    }

    /// The row's painted band at `off` as an oriented world rectangle: its
    /// interval along the row, its band reach across it. The interval pulls
    /// in a hair so rows abutting tip-to-tip — a drafting norm — still share.
    fn band_rect(&self, off: f64, interval: (f64, f64), band: &Band) -> BandRect {
        let line_c = self.line(off);
        BandRect {
            frame: self.frame,
            u: (interval.0 + 1e-6, interval.1 - 1e-6),
            c: (line_c - band.neg, line_c + band.pos),
        }
    }

    /// The offset that stands a band's innermost ink `clearance` beyond a
    /// painted box's outer edge (the band reach is added by the caller).
    fn past(&self, p: Bbox, clearance: f64) -> f64 {
        let outermost = corners(p)
            .iter()
            .map(|&c| self.sgn * self.frame.cross(c))
            .fold(f64::NEG_INFINITY, f64::max);
        clearance + outermost - self.base
    }
}

/// An axis-aligned box's four corners.
fn corners(b: Bbox) -> [P; 4] {
    [
        (b.min_x, b.min_y),
        (b.max_x, b.min_y),
        (b.max_x, b.max_y),
        (b.min_x, b.max_y),
    ]
}

/// A point set's projection span on a unit axis.
fn proj(pts: &[P], axis: P) -> (f64, f64) {
    pts.iter()
        .map(|p| p.0 * axis.0 + p.1 * axis.1)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        })
}

/// A row's painted band across the stack axis, relative to its dim line
/// along **+n**: `neg` reaches toward −n — the ISO value always rides above
/// the line, the frame's −n side — and `pos` toward +n. The extension
/// springs are excluded: they connect the row to its anchors by design and
/// cross freely.
struct Band {
    neg: f64,
    pos: f64,
}

impl Band {
    fn of(sgn: f64, fs: f64, sw: f64) -> Band {
        let arrow = ARROW_HALF * sw;
        Band {
            // Text lift (fs/2 + 2) plus half the text height.
            neg: fs + 2.0,
            // The extension overshoot runs outward past the line — beyond it
            // only when outward is the +n direction; the arrowheads spread
            // `ARROW_HALF · sw` either way.
            pos: if sgn > 0.0 {
                EXT_OVERSHOOT.max(arrow)
            } else {
                arrow
            },
        }
    }

    /// The band's reach toward the base — its innermost ink.
    fn inner(&self, sgn: f64) -> f64 {
        if sgn > 0.0 { self.neg } else { self.pos }
    }
}

/// A row band as an oriented rectangle: along ∈ `u`, across ∈ `c`, both in
/// the seat's frame. Axis rows stay axis-aligned world boxes; an aligned
/// dim's band is genuinely rotated.
struct BandRect {
    frame: Frame,
    u: (f64, f64),
    c: (f64, f64),
}

impl BandRect {
    fn corners(&self) -> [P; 4] {
        [
            self.frame.pt(self.u.0, self.c.0),
            self.frame.pt(self.u.1, self.c.0),
            self.frame.pt(self.u.1, self.c.1),
            self.frame.pt(self.u.0, self.c.1),
        ]
    }

    /// Grown by the clearance across the stack axis only — rows keep their
    /// stand-off across the stack; along the row they may abut.
    fn grown_cross(&self, clearance: f64) -> BandRect {
        BandRect {
            frame: self.frame,
            u: self.u,
            c: (self.c.0 - clearance, self.c.1 + clearance),
        }
    }

    /// Separating-axes overlap against an axis-aligned painted box — strict
    /// at both ends, exactly [`Bbox::overlaps`] when the frame is an axis.
    fn overlaps(&self, b: Bbox) -> bool {
        let mine = self.corners();
        let theirs = corners(b);
        [(1.0, 0.0), (0.0, 1.0), self.frame.u, self.frame.n]
            .iter()
            .all(|&axis| {
                let (a0, a1) = proj(&mine, axis);
                let (b0, b1) = proj(&theirs, axis);
                a0 < b1 && b0 < a1
            })
    }

    /// The band's world bounding box — what later rows clear. Exact for an
    /// axis row; the covering box for a rotated aligned band.
    fn aabb(&self) -> Bbox {
        Bbox::from_points(&self.corners())
    }
}

impl Rows {
    pub(super) fn new(extent: Bbox) -> Rows {
        Rows {
            extent,
            painted: Vec::new(),
        }
    }

    /// Register one painted box the rows must clear — a placed drafting
    /// symbol's bounds [SPEC 15.9].
    pub(in crate::layout::drawing) fn obstruct(&mut self, bbox: Bbox) {
        self.painted.push(bbox);
    }

    /// Register a lowered statement's texts — and any annotation-obstacle
    /// linework it drew, e.g. a datum's framed box reaching past its letter
    /// [SPEC 15.7] — as painted bounds the rows clear.
    pub(super) fn obstruct_texts(&mut self, nodes: &[PlacedNode]) {
        for n in nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Text || annotation_obstacle(n))
        {
            self.painted
                .push(Bbox::extent_of(std::slice::from_ref(n), |_| true));
        }
    }

    /// A side row's seat line — outward off the geometry extent's edge.
    pub(in crate::layout::drawing) fn side_line(&self, side: Side) -> SeatLine {
        SeatLine::side(side, self.extent)
    }

    /// Seat a dim occupying `interval` along `at`, standing at least
    /// `clearance` off everything already painted; returns the dimension
    /// line's world coordinate along the stack (cross) axis. `carried` is the
    /// statement's own carried-stack box, **relative to a zero line**
    /// [SPEC 15.9]: it deepens the band and widens the interval, so the row
    /// seats where what it itself paints below its text already clears
    /// everything painted — and later rows clear it in turn.
    pub fn seat(
        &mut self,
        at: SeatLine,
        interval: (f64, f64),
        clearance: f64,
        paint: &Paint,
        carried: Option<Bbox>,
    ) -> f64 {
        let mut band = Band::of(at.sgn, paint.fs, paint.sw);
        let mut interval = interval;
        if let Some(c) = carried {
            let (cross, along) = (proj(&corners(c), at.frame.n), proj(&corners(c), at.frame.u));
            band.neg = band.neg.max(-cross.0);
            band.pos = band.pos.max(cross.1);
            interval = (interval.0.min(along.0), interval.1.max(along.1));
        }
        // Innermost candidate: the band's nearest ink `clearance` off the
        // base — the extent's edge, or the aligned span's outermost anchor.
        let mut off = clearance + band.inner(at.sgn);
        // Push outward past whatever the band (grown by the clearance along
        // the stack axis) still lands on — each pass clears at least one
        // painted box for good, so the loop is bounded.
        for _ in 0..=self.painted.len() {
            let probe = at.band_rect(off, interval, &band).grown_cross(clearance);
            let push = self
                .painted
                .iter()
                .filter(|p| probe.overlaps(**p))
                .map(|p| at.past(*p, clearance) + band.inner(at.sgn))
                .fold(f64::NEG_INFINITY, f64::max);
            if push > off + 1e-9 {
                off = push;
            } else {
                break;
            }
        }
        self.painted.push(at.band_rect(off, interval, &band).aabb());
        at.line(off)
    }
}

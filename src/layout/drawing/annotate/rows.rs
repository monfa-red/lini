//! The dimension-row packer [SPEC 15.6]: row offsets derive from painted
//! bounds — a row stands `clearance` off everything already painted on its
//! side (geometry, callout texts, earlier rows), never at a fixed pitch.
//! One seating law for every dim: a side row stacks outward from the
//! geometry extent's edge, an **aligned** dim from its own span's outermost
//! anchor — both along a [`SeatLine`], the row's frame plus its outward
//! direction and base.
//!
//! An annotation that leaves along a **ray** instead — a leader's text, a
//! diametral value spilling past its rim — packs against that same painted
//! set along its exit ([`Rows::spill`]), so every statement that can land on
//! another one answers to one packer.
//!
//! The stacking itself is [`crate::layout::stack`], shared with the schematic
//! engine's satellites; what lives here is the dimension flavour — the
//! geometry extent rows stack outside of, the drafting band a dim paints, the
//! annotation obstacles, and the `side:` readings and their wording.

use super::super::anchors::{Anchor, Spot};
use super::super::geometry::{Frame, P, project};
use super::*;
use crate::layout::geom::dot;
use crate::layout::stack::{Band, Stack};
use crate::span::Span;

pub(in crate::layout::drawing) use crate::layout::stack::SeatLine;

/// The row packer [SPEC 15.6]: dims sharing a side pack into rows, each — in
/// source order — seating at the innermost offset where its painted band
/// stands its `clearance` off everything already painted there: the geometry
/// extent, registered obstacle texts, and earlier rows' bands.
pub(in crate::layout::drawing) struct Rows {
    extent: Bbox,
    stack: Stack,
}

/// A dim's painted band across the stack axis [SPEC 15.6]. The extension
/// springs are excluded: they connect the row to its anchors by design and
/// cross freely.
fn dim_band(sgn: f64, fs: f64, sw: f64) -> Band {
    let arrow = ARROW_HALF * sw;
    Band {
        // Text lift (fs/2 + 2) plus half the text height — the ISO value
        // always rides above the line, the frame's −n side.
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

/// An aligned dim's stand-off side [SPEC 15.6]: `side: left | right` read
/// along the span, first anchor → second (the walker's left); the default
/// faces **away from the geometry centre**. Returns whether the dim line
/// sits on the frame's +n side.
pub(in crate::layout::drawing) fn away(
    attrs: &AttrMap,
    span_dir: P,
    mid: P,
    centre: P,
    n: P,
    span: Span,
) -> Result<bool, Error> {
    if let Some(name) = side_attr(attrs) {
        // Walker's left with y down: facing along `d`, left is (d.1, -d.0).
        let dir = match name {
            "left" => (span_dir.1, -span_dir.0),
            "right" => (-span_dir.1, span_dir.0),
            _ => {
                return Err(Error::at(
                    span,
                    "an aligned dimension sits left or right of its span — read along it, first anchor to second",
                ));
            }
        };
        return Ok(dot(dir, n) > 0.0);
    }
    let v = (mid.0 - centre.0, mid.1 - centre.1);
    // A tie (a right triangle's hypotenuse runs exactly through its bbox
    // centre) falls to the ISO-above side (−n) — outside the common taper.
    Ok(dot(v, n) > 1e-9)
}

impl Rows {
    pub(super) fn new(extent: Bbox) -> Rows {
        Rows {
            extent,
            stack: Stack::default(),
        }
    }

    /// The drawn-geometry extent the rows stack outside of — what a
    /// ray-leaving annotation clears before it packs [SPEC 15.6/15.7].
    pub(in crate::layout::drawing) fn extent(&self) -> Bbox {
        self.extent
    }

    /// Register one painted box the rows must clear — a placed drafting
    /// symbol's bounds [SPEC 15.9].
    pub(in crate::layout::drawing) fn obstruct(&mut self, bbox: Bbox) {
        self.stack.obstruct(bbox);
    }

    /// Register a lowered statement's texts — and any annotation-obstacle
    /// linework it drew, e.g. a datum's framed box reaching past its letter
    /// [SPEC 15.7] — as painted bounds the rows clear.
    pub(super) fn obstruct_texts(&mut self, nodes: &[PlacedNode]) {
        for n in nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Text || annotation_obstacle(n))
        {
            self.stack
                .obstruct(Bbox::extent_of(std::slice::from_ref(n), |_| true));
        }
    }

    /// A side row's seat line — the axis frame, outward off the geometry
    /// extent's edge.
    pub(in crate::layout::drawing) fn side_line(&self, side: Side) -> SeatLine {
        let axis = match side {
            Side::Top | Side::Bottom => Axis::Horizontal,
            Side::Left | Side::Right => Axis::Vertical,
        };
        let (away_pos, base) = match side {
            Side::Bottom => (true, self.extent.max_y),
            Side::Top => (false, -self.extent.min_y),
            Side::Right => (true, self.extent.max_x),
            Side::Left => (false, -self.extent.min_x),
        };
        SeatLine::new(Frame::axis(axis), away_pos, base)
    }

    /// How far along `dir` a **ray-leaving** annotation's painted block must
    /// go to stand `clearance` off everything already painted [SPEC 15.6]. A
    /// leader's text and a spilled diametral value leave the geometry along a
    /// ray instead of seating on a side row — so they pack along that ray,
    /// against the same painted set and in the same source order; only the
    /// shape of the seat differs. `block` is where the annotation would paint
    /// unpushed and the answer is 0 when that already stands clear, so the
    /// packer stays a stand-off and never a placement. The block is not
    /// registered here: its lowered nodes are, through `obstruct_texts`,
    /// which measures the real ink.
    pub(in crate::layout::drawing) fn spill(&self, dir: P, block: Bbox, clearance: f64) -> f64 {
        self.stack.clear(block, dir, clearance)
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
        let mut band = dim_band(at.sgn(), paint.fs, paint.sw);
        let mut interval = interval;
        if let Some(c) = carried {
            let frame = at.frame();
            let (cross, along) = (project(c, frame.n), project(c, frame.u));
            band.neg = band.neg.max(-cross.0);
            band.pos = band.pos.max(cross.1);
            interval = (interval.0.min(along.0), interval.1.max(along.1));
        }
        self.stack.seat(at, interval, clearance, &band)
    }
}

/// The stacking side [SPEC 15.6]: explicit `side:` (validated against the
/// axis), a corner pull, or the axis default — bottom / right.
pub(in crate::layout::drawing) fn stack_side(
    attrs: &AttrMap,
    axis: Axis,
    pull: Option<Side>,
    span: Span,
) -> Result<Side, Error> {
    let valid = |s: Side| match axis {
        Axis::Horizontal => matches!(s, Side::Top | Side::Bottom),
        Axis::Vertical => matches!(s, Side::Left | Side::Right),
    };
    let off_axis = || {
        Error::at(
            span,
            match axis {
                Axis::Horizontal => "a horizontal dimension stacks on top or bottom",
                Axis::Vertical => "a vertical dimension stacks on left or right",
            },
        )
    };
    if let Some(name) = side_attr(attrs) {
        let side = Side::parse(name).ok_or_else(off_axis)?;
        if !valid(side) {
            return Err(off_axis());
        }
        return Ok(side);
    }
    if let Some(side) = pull.filter(|s| valid(*s)) {
        return Ok(side);
    }
    Ok(match axis {
        Axis::Horizontal => Side::Bottom,
        Axis::Vertical => Side::Right,
    })
}

/// Corner anchors both on one edge pull the dim there [SPEC 15.6]:
/// `a:top-left (-) b:top-right` stacks on top.
pub(in crate::layout::drawing) fn corner_pull(a: &Anchor, b: &Anchor, axis: Axis) -> Option<Side> {
    let edge = |anchor: &Anchor| -> Option<Side> {
        let Spot::Corner((dx, dy)) = anchor.spot else {
            return None;
        };
        Some(match axis {
            Axis::Horizontal => {
                if dy < 0.0 {
                    Side::Top
                } else {
                    Side::Bottom
                }
            }
            Axis::Vertical => {
                if dx < 0.0 {
                    Side::Left
                } else {
                    Side::Right
                }
            }
        })
    };
    match (edge(a), edge(b)) {
        (Some(x), Some(y)) if x == y => Some(x),
        _ => None,
    }
}

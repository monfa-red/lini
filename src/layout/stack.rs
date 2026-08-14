//! The outward stack packer: bands seat, one at a time, at the innermost
//! offset along a [`SeatLine`] where their painted reach stands `clearance`
//! off everything already painted there. `clearance` is a minimum, not a
//! coordinate — the packer goes farther out to clear. What is not seating on
//! a line but leaving along a ray reads the same set through [`Stack::clear`],
//! so one painted set answers both questions.
//!
//! Engine-neutral [SPEC 15.6 / SPEC 16]: the drawing's dimension rows stack
//! outside the geometry extent through it, and the schematic's satellites
//! stack outward from a pin through the same code. Everything flavoured —
//! what a band reaches, which boxes count as painted, how a side is named or
//! an error is worded — belongs to the caller.

use super::geom::{Frame, P, dot, proj, unit};
use super::ir::Bbox;

/// One painted region: the oriented rectangle something actually paints —
/// an axis box for an axis seat or a registered obstacle, a genuinely
/// rotated one for a seat along an arbitrary direction. Stored as its four
/// corners, because the box that *covers* a rotated band is mostly empty
/// sheet: an aligned dimension's cover spans its whole diagonal, and anything
/// packing against that would travel the diagonal to get clear of ink that
/// was never there.
#[derive(Clone, Copy)]
pub struct Painted([P; 4]);

impl Painted {
    pub fn of_box(b: Bbox) -> Painted {
        Painted([
            (b.min_x, b.min_y),
            (b.max_x, b.min_y),
            (b.max_x, b.max_y),
            (b.min_x, b.max_y),
        ])
    }

    /// The two edge directions — the separating axes this region contributes.
    /// A degenerate edge borrows its neighbour's perpendicular, so a zero-area
    /// region still reports the frame it was built in.
    fn axes(&self) -> [P; 2] {
        let [a, b, _, d] = self.0;
        let e = |p: P| unit((p.0 - a.0, p.1 - a.1));
        match (e(b), e(d)) {
            (Some(u), Some(n)) => [u, n],
            (Some(u), None) => [u, (-u.1, u.0)],
            (None, Some(n)) => [(n.1, -n.0), n],
            (None, None) => [(1.0, 0.0), (0.0, 1.0)],
        }
    }

    /// The region's span along `axis`.
    fn extent(&self, axis: P) -> (f64, f64) {
        proj(&self.0, axis)
    }

    /// The axis-aligned box that covers it — for a coarse reader that only
    /// needs to know roughly where the ink is ([SPEC 16.4]'s net text).
    pub fn bounds(&self) -> Bbox {
        Bbox::from_points(&self.0)
    }

    fn shifted(&self, dx: f64, dy: f64) -> Painted {
        Painted(self.0.map(|(x, y)| (x + dx, y + dy)))
    }
}

/// Where a band seats: it runs along `frame.u` and stacks outward along
/// `sgn · frame.n`, starting from `base` — the out-projected cross coordinate
/// of whatever it stands off. "o" coordinates below are the cross coordinate
/// times `sgn`, so outward is always positive.
pub struct SeatLine {
    frame: Frame,
    sgn: f64,
    base: f64,
}

impl SeatLine {
    /// A seat in `frame`, stacking toward +n when `away_pos`, off the already
    /// out-projected coordinate `base` (i.e. the cross coordinate times the
    /// outward sign).
    pub fn new(frame: Frame, away_pos: bool, base: f64) -> SeatLine {
        let sgn = if away_pos { 1.0 } else { -1.0 };
        SeatLine { frame, sgn, base }
    }

    /// A seat off the outermost of two ends — a span's own stand-off
    /// [SPEC 15.6].
    pub fn span(frame: Frame, away_pos: bool, ends: (P, P)) -> SeatLine {
        let sgn = if away_pos { 1.0 } else { -1.0 };
        let base = (sgn * frame.cross(ends.0)).max(sgn * frame.cross(ends.1));
        SeatLine { frame, sgn, base }
    }

    pub fn frame(&self) -> Frame {
        self.frame
    }

    /// The outward sign along `frame.n` — what a caller's band construction
    /// reads to know which way its asymmetric reach faces.
    pub fn sgn(&self) -> f64 {
        self.sgn
    }

    /// The seated line's world cross coordinate at `off` outward from base.
    pub fn line(&self, off: f64) -> f64 {
        self.sgn * (self.base + off)
    }

    /// The band at `off` as an oriented world rectangle: its interval along
    /// the line, its band reach across it. The interval pulls in a hair so
    /// bands abutting tip-to-tip — a drafting norm — still share.
    fn band_rect(&self, off: f64, interval: (f64, f64), band: &Band) -> BandRect {
        let line_c = self.line(off);
        BandRect {
            frame: self.frame,
            u: (interval.0 + 1e-6, interval.1 - 1e-6),
            c: (line_c - band.neg, line_c + band.pos),
        }
    }

    /// The offset that stands a band's innermost ink `clearance` beyond a
    /// painted region's outer edge (the band reach is added by the caller).
    fn past(&self, p: &Painted, clearance: f64) -> f64 {
        let (lo, hi) = p.extent(self.frame.n);
        let outermost = if self.sgn > 0.0 { hi } else { -lo };
        clearance + outermost - self.base
    }
}

/// A band's painted reach across the stack axis, relative to its own line
/// along **+n**: `neg` reaches toward −n, `pos` toward +n. What each engine
/// counts as reach is its own business (the drawing excludes the extension
/// springs, which cross freely by design).
pub struct Band {
    pub neg: f64,
    pub pos: f64,
}

impl Band {
    /// The band's reach toward the base — its innermost ink.
    pub fn inner(&self, sgn: f64) -> f64 {
        if sgn > 0.0 { self.neg } else { self.pos }
    }
}

/// A band as an oriented rectangle: along ∈ `u`, across ∈ `c`, both in the
/// seat's frame. Axis seats stay axis-aligned world boxes; a seat along an
/// arbitrary direction gives a genuinely rotated band.
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

    /// Grown by the clearance across the stack axis only — bands keep their
    /// stand-off across the stack; along it they may abut.
    fn grown_cross(&self, clearance: f64) -> BandRect {
        BandRect {
            frame: self.frame,
            u: self.u,
            c: (self.c.0 - clearance, self.c.1 + clearance),
        }
    }

    /// Separating-axes overlap against a painted region — both rectangles'
    /// edge normals, so it is exact for either orientation and strict at both
    /// ends, exactly [`Bbox::overlaps`] when both are axis-aligned.
    fn overlaps(&self, o: &Painted) -> bool {
        let mine = self.corners();
        [self.frame.u, self.frame.n]
            .into_iter()
            .chain(o.axes())
            .all(|axis| {
                let (a0, a1) = proj(&mine, axis);
                let (b0, b1) = o.extent(axis);
                a0 < b1 && b0 < a1
            })
    }

    /// The band as a painted region — the strip it really paints, rotated
    /// with its seat.
    fn painted(&self) -> Painted {
        Painted(self.corners())
    }
}

/// The painted set a stack packs against: everything a later band must stand
/// `clearance` off — obstacles the caller registers, then each seated band's
/// own painted box.
#[derive(Default)]
pub struct Stack {
    painted: Vec<Painted>,
}

impl Stack {
    /// Register one painted box every later band must clear.
    pub fn obstruct(&mut self, bbox: Bbox) {
        self.painted.push(Painted::of_box(bbox));
    }

    /// What has been painted so far — read by a caller that must choose
    /// between two symmetric seats and wants to know which side is freer
    /// ([SPEC 16.4]'s net text). Measuring it is the caller's own business.
    pub fn painted(&self) -> &[Painted] {
        &self.painted
    }

    /// The distance along unit `dir` that carries `b` clear of **everything
    /// painted**, standing `margin` off it — an annotation that leaves along
    /// a ray instead of seating on a line ([`clear_past`], applied to the
    /// whole set). Each pass clears at least one box for good, so the loop is
    /// bounded; nothing is registered — a ray-leaving caller paints its own
    /// ink and registers that.
    pub fn clear(&self, b: Bbox, dir: P, margin: f64) -> f64 {
        let b = Painted::of_box(b);
        let mut t = 0.0;
        for _ in 0..=self.painted.len() {
            let at = b.shifted(dir.0 * t, dir.1 * t);
            let push = self
                .painted
                .iter()
                .map(|p| clear_past(&at, dir, p, margin))
                .fold(0.0, f64::max);
            if push <= 1e-9 {
                break;
            }
            t += push;
        }
        t
    }

    /// Seat `band` occupying `interval` along `at`, standing at least
    /// `clearance` off everything already painted; returns the seated line's
    /// world coordinate along the stack (cross) axis, and registers the band
    /// so later ones clear it in turn.
    pub fn seat(&mut self, at: SeatLine, interval: (f64, f64), clearance: f64, band: &Band) -> f64 {
        // Innermost candidate: the band's nearest ink `clearance` off the base.
        let mut off = clearance + band.inner(at.sgn);
        // Push outward past whatever the band (grown by the clearance along
        // the stack axis) still lands on — each pass clears at least one
        // painted box for good, so the loop is bounded.
        for _ in 0..=self.painted.len() {
            let probe = at.band_rect(off, interval, band).grown_cross(clearance);
            let push = self
                .painted
                .iter()
                .filter(|p| probe.overlaps(p))
                .map(|p| at.past(p, clearance) + band.inner(at.sgn))
                .fold(f64::NEG_INFINITY, f64::max);
            if push > off + 1e-9 {
                off = push;
            } else {
                break;
            }
        }
        self.painted
            .push(at.band_rect(off, interval, band).painted());
        at.line(off)
    }
}

/// The distance along unit `dir` that carries `b` past `obstacle`, standing
/// `margin` off it — parting them on any **one** separating axis is enough,
/// so the cheapest feasible axis wins; 0 when they already stand clear. The
/// one region-past-region push: a leader's block clearing the drawn geometry
/// reads it, and so does every pass of [`Stack::clear`].
pub fn clear_past(b: &Painted, dir: P, obstacle: &Painted, margin: f64) -> f64 {
    let mut need = f64::INFINITY;
    // At least one of `b`'s own two axes is within 45° of `dir`, so the
    // minimum below is always finite.
    for axis in b.axes().into_iter().chain(obstacle.axes()) {
        let (lo, hi) = b.extent(axis);
        let (o_lo, o_hi) = obstacle.extent(axis);
        let (o_lo, o_hi) = (o_lo - margin, o_hi + margin);
        if hi <= o_lo || o_hi <= lo {
            return 0.0;
        }
        let d = dot(dir, axis);
        need = need.min(if d > 1e-9 {
            (o_hi - lo) / d
        } else if d < -1e-9 {
            (hi - o_lo) / -d
        } else {
            f64::INFINITY
        });
    }
    need
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bbox(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Bbox {
        Bbox {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    /// The 45° strip a diagonal band paints, and the far corner of the box
    /// that merely covers it.
    fn diagonal_strip() -> Stack {
        let d = std::f64::consts::FRAC_1_SQRT_2;
        let mut stack = Stack::default();
        stack.seat(
            SeatLine::new(Frame::outward((d, d)), true, 0.0),
            (-100.0, 100.0),
            0.0,
            &Band { neg: 2.0, pos: 2.0 },
        );
        stack
    }

    #[test]
    fn a_rotated_band_claims_its_strip_not_the_box_that_covers_it() {
        // The corner of the cover is 70 off the ink — nothing there has to
        // move. Registered as the cover, a value packing along a ray would
        // have travelled the whole diagonal to clear ink that was never
        // there.
        let far = bbox(45.0, 45.0, 55.0, 55.0);
        assert_eq!(diagonal_strip().clear(far, (0.0, -1.0), 4.0), 0.0);
    }

    #[test]
    fn a_box_on_the_ink_is_carried_past_it_along_the_ray() {
        // Astride the strip: pushed along −y until it stands 4 clear.
        let on = bbox(-5.0, -5.0, 5.0, 5.0);
        let push = diagonal_strip().clear(on, (0.0, -1.0), 4.0);
        assert!(push > 0.0, "{push}");
        let moved = on.shifted(0.0, -push);
        assert_eq!(diagonal_strip().clear(moved, (0.0, -1.0), 4.0), 0.0);
    }
}

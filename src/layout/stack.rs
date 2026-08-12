//! The outward stack packer: bands seat, one at a time, at the innermost
//! offset along a [`SeatLine`] where their painted reach stands `clearance`
//! off everything already painted there. `clearance` is a minimum, not a
//! coordinate — the packer goes farther out to clear.
//!
//! Engine-neutral [SPEC 15.6 / SPEC 16]: the drawing's dimension rows stack
//! outside the geometry extent through it, and the schematic's satellites
//! stack outward from a pin through the same code. Everything flavoured —
//! what a band reaches, which boxes count as painted, how a side is named or
//! an error is worded — belongs to the caller.

use super::geom::{Frame, P, proj, project};
use super::ir::Bbox;

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
    /// painted box's outer edge (the band reach is added by the caller).
    fn past(&self, p: Bbox, clearance: f64) -> f64 {
        let (lo, hi) = project(p, self.frame.n);
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

    /// Separating-axes overlap against an axis-aligned painted box — strict
    /// at both ends, exactly [`Bbox::overlaps`] when the frame is an axis.
    fn overlaps(&self, b: Bbox) -> bool {
        let mine = self.corners();
        [(1.0, 0.0), (0.0, 1.0), self.frame.u, self.frame.n]
            .iter()
            .all(|&axis| {
                let (a0, a1) = proj(&mine, axis);
                let (b0, b1) = project(b, axis);
                a0 < b1 && b0 < a1
            })
    }

    /// The band's world bounding box — what later bands clear. Exact for an
    /// axis seat; the covering box for a rotated one.
    fn aabb(&self) -> Bbox {
        Bbox::from_points(&self.corners())
    }
}

/// The painted set a stack packs against: everything a later band must stand
/// `clearance` off — obstacles the caller registers, then each seated band's
/// own painted box.
#[derive(Default)]
pub struct Stack {
    painted: Vec<Bbox>,
}

impl Stack {
    /// Register one painted box every later band must clear.
    pub fn obstruct(&mut self, bbox: Bbox) {
        self.painted.push(bbox);
    }

    /// What has been painted so far — read by a caller that must choose
    /// between two symmetric seats and wants to know which side is freer
    /// ([SPEC 16.4]'s net text). Measuring it is the caller's own business.
    pub fn painted(&self) -> &[Bbox] {
        &self.painted
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
                .filter(|p| probe.overlaps(**p))
                .map(|p| at.past(*p, clearance) + band.inner(at.sgn))
                .fold(f64::NEG_INFINITY, f64::max);
            if push > off + 1e-9 {
                off = push;
            } else {
                break;
            }
        }
        self.painted.push(at.band_rect(off, interval, band).aabb());
        at.line(off)
    }
}

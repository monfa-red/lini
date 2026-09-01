//! The scope's **lattice** [SPEC 16.1] — the two pitches every schematic
//! coordinate is a multiple of.
//!
//! The **fine** pitch is [`PIN_PITCH`]: every pin, stub tip and wire track
//! lands on it. The **coarse** pitch is the scope's own `gap`, per axis — the
//! column and row pitch every part centre lands on, not the space between two
//! tracks.
//!
//! A coarse pitch is rounded **up** to a whole number of fine ones, so a part
//! centre is always a wire line too: a column of parts and the wires reaching
//! them would otherwise drift apart by the remainder, one cell at a time.

use super::super::ir::PlacedNode;
use super::super::primitives;
use crate::desugar::pose::Side;
use crate::error::Error;
use crate::ledger::consts::PIN_PITCH;
use crate::resolve::AttrMap;
use crate::span::Span;

/// Which lattice axis a coordinate lies on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Ax {
    X,
    Y,
}

impl Ax {
    /// The axis a side's outward normal runs along — *across* the side, so a
    /// left or right side answers `X`.
    pub(super) fn of(side: Side) -> Ax {
        if side.is_vertical() { Ax::X } else { Ax::Y }
    }

    /// The other axis — what runs *across* this one.
    pub(super) fn other(self) -> Ax {
        match self {
            Ax::X => Ax::Y,
            Ax::Y => Ax::X,
        }
    }

    /// `+1` when the side's normal points the increasing way, `-1` otherwise.
    /// Read off [`Side::normal`] rather than tabled again, so the sheet keeps
    /// one account of which way a side faces.
    pub(super) fn outward(side: Side) -> f64 {
        let (dx, dy) = side.normal();
        match Ax::of(side) {
            Ax::X => dx,
            Ax::Y => dy,
        }
    }
}

/// A schematic scope's grid [SPEC 16.1]: the fine pitch every wire and pin
/// lands on, and the coarse pitch every part centre does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Lattice {
    pub pitch: f64,
    pub row: f64,
    pub col: f64,
}

impl Lattice {
    /// Read the scope's lattice off its attrs.
    pub(super) fn of(attrs: &AttrMap, span: Span) -> Result<Lattice, Error> {
        let (row, col) = primitives::gap(attrs, span)?;
        let pitch = PIN_PITCH;
        // Up, and never below one fine pitch: a coarse line that is not also a
        // wire line is no use to the passes that read it.
        let coarse = |g: f64| (g / pitch).ceil().max(1.0) * pitch;
        Ok(Lattice {
            pitch,
            row: coarse(row),
            col: coarse(col),
        })
    }

    /// The coarse step along `ax`.
    pub(super) fn step(self, ax: Ax) -> f64 {
        match ax {
            Ax::X => self.col,
            Ax::Y => self.row,
        }
    }

    /// The coordinate of coarse line `i` on `ax`.
    pub(super) fn line(self, ax: Ax, i: i32) -> f64 {
        f64::from(i) * self.step(ax)
    }

    /// The first coarse line **strictly** beyond `v` along `ax`, going the
    /// way `outward` points (`+1` / `-1`).
    pub(super) fn beyond(self, ax: Ax, v: f64, outward: f64) -> i32 {
        let lines = v / self.step(ax);
        if outward > 0.0 {
            lines.floor() as i32 + 1
        } else {
            lines.ceil() as i32 - 1
        }
    }

    /// `v` rounded to the nearest fine line.
    pub(super) fn snap(self, v: f64) -> f64 {
        (v / self.pitch).round() * self.pitch
    }

    /// `w` in whole **fine** pitches — the lattice a body of that width takes
    /// up across the line it stands on [SPEC 16.1]. `0` for no body at all.
    pub(super) fn pitches(self, w: f64) -> f64 {
        (w / self.pitch - EPS).ceil().max(0.0) * self.pitch
    }
}

/// Slack for the divisions the lattice does: a cell centre sits on its line to
/// the last bit, so anything larger than rounding noise is a real step out.
pub(super) const EPS: f64 = 1e-9;

/// A scope's **track quantum** (ROUTING.md §Vocabulary) — its fine pitch
/// [SPEC 16.1] — or `None` for a node that is no schematic scope.
///
/// The router rounds an interior run's preference to it, and [`snap_scopes`]
/// puts the scope's own origin on it: one reading, so the wires' grid and the
/// parts' cannot be two grids.
pub(crate) fn quantum(attrs: &AttrMap) -> Option<f64> {
    crate::resolve::is_schematic(attrs).then_some(PIN_PITCH)
}

/// Put every schematic scope's origin on the fine lattice [SPEC 16.1].
///
/// A scope lays its parts out on multiples of its pitch **in its own frame**,
/// while the router rounds to multiples of the same quantum in the *scene*'s —
/// so a scope its parent seated half a pitch off would hand its parts one grid
/// and its wires another, and every bare run would jog by the remainder. The
/// scope moves, never its contents: the sheet inside it is already square with
/// itself, and it is only where the parent put the frame that is arbitrary.
///
/// The walk mirrors the router's own ([`crate::routing::ortho::scene`]): plain
/// offsets down the tree, rotation left to the ancestor that authored it.
pub(in crate::layout) fn snap_scopes(nodes: &mut [PlacedNode]) {
    fn walk(nodes: &mut [PlacedNode], ox: f64, oy: f64) {
        for n in nodes.iter_mut() {
            if let Some(q) = quantum(&n.attrs) {
                let snap = |v: f64| (v / q).round() * q;
                n.cx = snap(ox + n.cx) - ox;
                n.cy = snap(oy + n.cy) - oy;
            }
            // A scope nested in a scope is already on the grid — its parent
            // seated it there — so the walk costs it nothing.
            walk(&mut n.children, ox + n.cx, oy + n.cy);
        }
    }
    walk(nodes, 0.0, 0.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::consts::PIN_PITCH;
    use crate::ledger::defaults::SCH_GAP;
    use crate::testutil::program;

    fn lat(style: &str) -> Lattice {
        let p = program(&format!("|schematic#s|{style} [ |gnd#g| ]\n"));
        let scope = &p.scene.nodes[0];
        Lattice::of(&scope.attrs, scope.span).expect("a lattice")
    }

    #[test]
    fn the_scope_gap_is_the_coarse_pitch() {
        let l = lat("");
        assert_eq!((l.row, l.col), (SCH_GAP, SCH_GAP), "the scope default");
        assert_eq!(l.pitch, PIN_PITCH, "the fine pitch is the pin pitch");
    }

    #[test]
    fn a_coarse_pitch_rounds_up_to_a_whole_number_of_fine_ones() {
        // [SPEC 16.1] the coarse grid is built of fine units, so a part
        // centre is always on a wire line too.
        let l = lat(" { gap: 90 }");
        assert_eq!(l.row, 100.0, "90 rounds up to 5 pitches");
        assert_eq!(l.col, 100.0);
        assert_eq!(lat(" { gap: 100 }").row, 100.0, "an exact multiple stands");
    }

    #[test]
    fn gap_states_the_two_axes_separately() {
        let l = lat(" { gap: 120 80 }");
        assert_eq!(
            (l.row, l.col),
            (120.0, 80.0),
            "row then column, as gap reads"
        );
    }

    #[test]
    fn a_coarse_pitch_never_falls_below_one_fine_one() {
        assert_eq!(lat(" { gap: 0 }").row, PIN_PITCH);
    }

    #[test]
    fn lines_and_snapping_are_plain_arithmetic() {
        let l = Lattice {
            pitch: 20.0,
            row: 100.0,
            col: 100.0,
        };
        assert_eq!(l.line(Ax::X, 3), 300.0);
        assert_eq!(l.line(Ax::Y, -2), -200.0);
        assert_eq!(l.snap(53.0), 60.0);
        assert_eq!(l.snap(-53.0), -60.0);
    }

    #[test]
    fn beyond_is_strict_so_a_field_never_starts_on_the_ink() {
        let l = Lattice {
            pitch: 20.0,
            row: 100.0,
            col: 100.0,
        };
        assert_eq!(l.beyond(Ax::X, 120.0, 1.0), 2, "the next line out");
        assert_eq!(l.beyond(Ax::X, 200.0, 1.0), 3, "strictly beyond, never on");
        assert_eq!(l.beyond(Ax::X, -120.0, -1.0), -2);
        assert_eq!(l.beyond(Ax::X, -200.0, -1.0), -3);
    }
}

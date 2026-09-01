//! The **readout side** [SPEC 16.2] — which way a seated part wears its ref
//! and its value.
//!
//! Desugar mints the pair at the seat the part's own family and pose give it
//! ([`crate::desugar::schematic`]): above a `|component|`, above and below a
//! part riding a pin's row, and **beside** the axis of a turned one, whose wire
//! runs down the column through it. Which *way* a pair faces is the field's to
//! say, and the field is the first pass that knows it — so both choices live
//! here, and the mint upstream is simply the sheet's own reading side.
//!
//! A **turned** part's pair goes **outward**, away from the anchor whose field
//! it stands in. Left field → to its left; right field → to its right; a part
//! on no anchor's flank keeps the reading side. And the pair is aligned on its
//! **inner** edge, so the two readouts line up against the drawing and a longer
//! value grows outward alone rather than reaching back over the pin the part
//! hangs from and closing the corridor its own lead needs.
//!
//! A **corridor** part — one lying along its own pin's row — straddles that row
//! with its pair instead, and where the anchor's pins crowd one side of the row
//! and leave the other free, both lines step whole to the free side. The near
//! line otherwise closes the *neighbouring* pin's own row, which the pitch puts
//! well inside it ([`stacked`]).
//!
//! Ink still places nothing [SPEC 16.1]: this pass moves text, never a part,
//! and it runs after the rails because a rail moves a terminator — which
//! carries no readout — and never a body.

use super::field::Field;
use super::lattice::EPS;
use super::terminal::terminal;
use crate::desugar::pose::{Pose, Side};
use crate::desugar::schematic::{readout_beside, sch_kind, terminal_ids};
use crate::layout::ir::PlacedNode;
use crate::ledger::consts::{READOUT_OFFSET, READOUT_STACK};

/// Where one part's pair goes: **beside** its axis on the side `sign` points,
/// or **stacked** whole that way off the row it rides.
#[derive(Clone, Copy)]
enum Pair {
    Beside(f64),
    Stacked(f64),
}

/// Seat every readout pair [SPEC 16.2].
pub(super) fn readouts(children: &mut [PlacedNode], field: &Field) {
    // Decided over the whole slice first, applied after: the corridor reading
    // is of the *anchor* the member hangs off, which is another child of it.
    let pairs: Vec<Option<Pair>> = (0..children.len())
        .map(|i| pair(children, field, i))
        .collect();
    for (node, pair) in children.iter_mut().zip(pairs) {
        let Some(pair) = pair else { continue };
        let axis = node.bbox.center().0;
        for c in node.children.iter_mut() {
            let is_ref = c.type_chain.iter().any(|t| t == "ref");
            if !is_ref && !c.type_chain.iter().any(|t| t == "part-value") {
                continue;
            }
            match pair {
                Pair::Beside(sign) => {
                    let (text, half) = (c.bbox.center().0, c.bbox.w() / 2.0);
                    c.cx = axis + sign * (READOUT_OFFSET + half) - text;
                }
                // Both lines to one side, in reading order: whichever of them
                // the step leads with keeps the line it was minted on and the
                // other stacks under it, exactly as a component wears its own
                // pair — so the ref still reads above the value.
                Pair::Stacked(sign) => {
                    let under = (is_ref != (sign > 0.0)) as u8;
                    c.cy = sign * (c.cy.abs() + f64::from(under) * (c.bbox.h() + READOUT_STACK));
                }
            }
        }
    }
}

/// Which seat child `i`'s pair takes, or `None` where it keeps the one desugar
/// minted.
fn pair(children: &[PlacedNode], field: &Field, i: usize) -> Option<Pair> {
    let kind = sch_kind(&children[i].type_chain)?;
    let seat = field.seat(i);
    if readout_beside(kind, Pose::of_chain(&children[i].type_chain)) {
        // The lead leaves by the pin's own normal, so its side **is** the flank
        // of the anchor the part stands on; a chain growing straight out of a
        // top or bottom pin stands on neither, and reads right.
        let outward = match seat.map(|s| s.side) {
            Some(side) if side.is_vertical() => side,
            _ => Side::Right,
        };
        return Some(Pair::Beside(if outward == Side::Left { -1.0 } else { 1.0 }));
    }
    stacked(children, seat?)
}

/// A **corridor** member's step [SPEC 16.2] — a part lying along its own pin's
/// row, whose pair straddles that row.
///
/// The pitch two neighbouring pins stand at is one **fine** line, and a readout
/// line stands further off its body than that: so a member seated in one pin's
/// corridor draws over the *next* pin's row, and the wire that has to cross
/// back over the member to reach that pin — a same-side **bridge**'s return
/// ([SPEC 16.1]) — finds the short way home walled off and orbits the member's
/// body instead. Where the anchor's pins on that side crowd one way and leave
/// the other free, both lines step whole to the free side and the row is a row
/// again. Crowded both ways — or neither — there is nothing to choose, and the
/// pair straddles as minted.
fn stacked(children: &[PlacedNode], seat: super::field::Seat) -> Option<Pair> {
    if seat.ray != seat.side || !seat.side.is_vertical() {
        return None;
    }
    let rows = pin_rows(&children[seat.anchor], seat.side);
    let above = rows.iter().any(|&r| r < seat.cross - EPS);
    let below = rows.iter().any(|&r| r > seat.cross + EPS);
    (above != below).then_some(Pair::Stacked(if above { 1.0 } else { -1.0 }))
}

/// The rows an anchor's own pins take on `side`, in its own frame — read
/// through the one terminal reader, so a corridor is measured on the very
/// points the wires land at.
fn pin_rows(anchor: &PlacedNode, side: Side) -> Vec<f64> {
    terminal_ids(anchor)
        .iter()
        .map(|id| terminal(anchor, id.as_deref()))
        .filter(|t| t.facing == Some(side))
        .map(|t| t.at.1)
        .collect()
}

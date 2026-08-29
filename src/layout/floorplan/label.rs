//! Where a floorplan node's **smart label** seats [SPEC 15.11].
//!
//! Two of the dialect's labels stand *beside* the thing they name — a
//! fixture's below its body, exactly as a discrete reads its value
//! ([SPEC 16.3]), and an opening's schedule tag beside its gap — so both ask
//! for one seat: clear of the body by [`READOUT_GAP`], stacking outward when a
//! node carries more than one line. The two that do **not** seat here are the
//! ones SPEC 15.11 states as exceptions: an `|appliance|`'s label centres *in*
//! its box and a `|wall|`'s keeps the sketch's centred read — both are the
//! `|block|` default, so they are had by not calling this.
//!
//! The body is only sized at layout, which is why the seat lives here and not
//! beside the schematic readout's desugar-time `translate:`.
//!
//! However a node is turned, its label stays **readable like dimension text** —
//! ISO-aligned, from the bottom or the right, never upside-down: 15.6's rule,
//! shared through [`iso_upright`], so a schedule tag on a wall drawn
//! right-to-left reads the same way as one on a wall drawn left-to-right.

use super::super::drawing::geometry::iso_upright;
use super::super::ir::PlacedNode;
use crate::ledger::consts::{READOUT_GAP, READOUT_STACK};
use crate::resolve::{NodeKind, ResolvedValue};

/// Seat a node's text children clear of its body: `clear` is the half-extent
/// they must stand off the origin, `side` which way they go (`+1` is the
/// node's own `+y`, `-1` the other face). The first line stands off by
/// [`READOUT_GAP`] and further ones stack by [`READOUT_STACK`] — the schematic
/// readout's own two constants, so the two stages state one seat. `rot` is the
/// node's own turn, which the lines take back so they read upright.
pub(super) fn seat(children: &mut [PlacedNode], clear: f64, side: f64, rot: f64) {
    let mut edge = clear + READOUT_GAP;
    for c in children.iter_mut().filter(|c| c.kind == NodeKind::Text) {
        c.cy = side * (edge + c.bbox.h() / 2.0);
        edge += c.bbox.h() + READOUT_STACK;
    }
    upright(children, rot);
}

/// Turn a node's text children upright [SPEC 15.11]. The node's own group has
/// already turned them by `rot`, so each takes back exactly what that carried
/// past readable — a half turn, or nothing. It rides the leaf's own `rotate`,
/// which turns text about its centre, so the seat is untouched: a line stays on
/// the face chosen for it and only stops reading backwards.
pub(super) fn upright(children: &mut [PlacedNode], rot: f64) {
    let back = iso_upright(rot) - rot;
    if back == 0.0 {
        return;
    }
    for c in children.iter_mut().filter(|c| c.kind == NodeKind::Text) {
        let own = c.attrs.number("rotate").unwrap_or(0.0);
        c.attrs.insert("rotate", ResolvedValue::Number(own + back));
    }
}

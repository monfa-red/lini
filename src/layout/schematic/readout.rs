//! The **readout side** [SPEC 16.2] — which way a seated part wears its ref
//! and its value.
//!
//! Desugar mints the pair at the seat the part's own family and pose give it
//! ([`crate::desugar::schematic`]): above a `|component|`, above and below a
//! part riding a pin's row, and **beside** the axis of a turned one, whose wire
//! runs down the column through it. Only that last seat has a side to choose,
//! and the field is the first pass that knows it — so this is where the choice
//! lives, and the mint upstream is simply the sheet's own reading side.
//!
//! The rule is one sentence: **outward**, away from the anchor whose field the
//! part stands in. Left field → to its left; right field → to its right; a part
//! on no anchor's flank keeps the reading side. And the pair is aligned on its
//! **inner** edge, so the two readouts line up against the drawing and a longer
//! value grows outward alone rather than reaching back over the pin the part
//! hangs from and closing the corridor its own lead needs.
//!
//! Ink still places nothing [SPEC 16.1]: this pass moves text, never a part,
//! and it runs after the rails because a rail moves a terminator — which
//! carries no readout — and never a body.

use super::field::Field;
use crate::desugar::pose::{Pose, Side};
use crate::desugar::schematic::{readout_beside, sch_kind};
use crate::layout::ir::PlacedNode;
use crate::ledger::consts::READOUT_OFFSET;

/// Turn every beside-seated pair outward [SPEC 16.2].
pub(super) fn readouts(children: &mut [PlacedNode], field: &Field) {
    for (i, node) in children.iter_mut().enumerate() {
        let Some(kind) = sch_kind(&node.type_chain) else {
            continue;
        };
        if !readout_beside(kind, Pose::of_chain(&node.type_chain)) {
            continue;
        }
        // The lead leaves by the pin's own normal, so its side **is** the flank
        // of the anchor the part stands on; a chain growing straight out of a
        // top or bottom pin stands on neither, and reads right.
        let outward = match field.seat(i).map(|s| s.side) {
            Some(side) if side.is_vertical() => side,
            _ => Side::Right,
        };
        let sign = if outward == Side::Left { -1.0 } else { 1.0 };
        let axis = node.bbox.center().0;
        for c in node.children.iter_mut() {
            if !c.type_chain.iter().any(|t| t == "ref" || t == "part-value") {
                continue;
            }
            let (text, half) = (c.bbox.center().0, c.bbox.w() / 2.0);
            c.cx = axis + sign * (READOUT_OFFSET + half) - text;
        }
    }
}

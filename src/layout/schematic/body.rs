//! A component's **outline** [SPEC 16.2] — re-centred on its pins.
//!
//! A rail of an even count reserves the odd slot it is short of, at its far
//! end, so every pin stays a whole pitch from the part's origin — the lattice
//! point its pins are counted from, which the tracks place. Laid out as a
//! box, the outline then hangs a whole slot past the last pin on that side
//! and none on the other. So the outline re-centres on the pins: the box
//! shifts until the pins' own extent sits in its middle, while the origin and
//! everything counted from it — the pins, their stubs, the lattice — stay
//! put. The router reads the shifted box, and the readouts ride it.

use super::super::anchors;
use super::super::ir::PlacedNode;
use super::lattice::EPS;
use super::terminal::{Terminal, terminal};
use crate::desugar::schematic::{SchKind, sch_kind, terminal_ids};
use crate::error::Error;

/// Re-centre every `|component|`'s outline on its pins.
pub(super) fn recentre(children: &mut [PlacedNode]) -> Result<(), Error> {
    for node in children
        .iter_mut()
        .filter(|c| sch_kind(&c.type_chain) == Some(SchKind::Component))
    {
        let terms: Vec<Terminal> = terminal_ids(node)
            .iter()
            .map(|id| terminal(node, id.as_deref()))
            .collect();
        // The middle of the pins along one axis: the side pins' rows, or the
        // top and bottom pins' columns. `None` where that axis carries none.
        let middle = |rows: bool| -> Option<f64> {
            let (lo, hi) = terms
                .iter()
                .filter(|t| t.facing.is_some_and(|f| f.is_vertical() == rows))
                .map(|t| if rows { t.at.1 } else { t.at.0 })
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                    (lo.min(v), hi.max(v))
                });
            (lo <= hi).then_some((lo + hi) / 2.0)
        };
        let (cx, cy) = node.bbox.center();
        let dx = middle(false).map_or(0.0, |m| m - cx);
        let dy = middle(true).map_or(0.0, |m| m - cy);
        if dx.abs() <= EPS && dy.abs() <= EPS {
            continue;
        }
        node.bbox = node.bbox.shifted(dx, dy);
        anchors::reseat_overlays(node)?;
    }
    Ok(())
}

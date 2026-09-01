//! The scope's **ground row** [SPEC 16.1] — the one line a sheet really draws.
//!
//! Every drafted sheet ends its downward chains on one line: the ground of a
//! lone decoupling cap and the ground of a three-part divider stand on the same
//! row. The row is the **scope's**, so it is struck once the satellites are
//! absolute — there is no earlier frame in which two anchors' fields share a
//! line.
//!
//! It is the **downward** ray's, and there is no upward twin. A block has one
//! ground net and one line for it reads as that net; a power flag names a net
//! of its own — 24 V, 3V3, 5 V — and aligning three of them says nothing,
//! which is why no reference sheet does. A chain terminating upward stands on
//! its own slot like any other member, and two single-member up-chains in one
//! track row land on one line regardless, their slot origin being that row's
//! ([`super::field`]).
//!
//! Which chains rail is the **ray**, never the symbol: a ground posed inverted
//! above a part grows up and keeps its slot. The symbol only says whether a
//! terminator rails at all — the returns and the feed do, and the rest of the
//! set (`nc`, `antenna`) ends where it stands [SPEC 16.4].
//!
//! The row lands on a **fine** line, not a coarse one: it is a separation from
//! the deepest ink above it, and a coarse line would round that up to a whole
//! cell of bare wire. Only the terminator moves — its chain keeps every slot
//! above it, and the lead down to the row is the router's ordinary wire.

use super::field::{Field, drawn};
use super::lattice::Lattice;
use super::terminal::{ident, seat_point};
use crate::desugar::pose::Side;
use crate::desugar::schematic::{SchKind, sch_kind};
use crate::layout::ir::{Bbox, PlacedNode};

/// The symbols a rail row is drawn of [SPEC 16.4]: the three returns and the
/// feed.
const RAIL_SYMBOLS: [&str; 4] = ["gnd", "earth", "chassis", "power"];

/// Sink every downward chain's ground to one row [SPEC 16.1] — and answer the
/// cells that row took, which may reach past the box the packing struck.
pub(super) fn rails(children: &mut [PlacedNode], field: &Field, lat: Lattice) -> Bbox {
    let riders: Vec<(usize, bool)> = (0..children.len())
        .filter_map(|i| {
            let seat = field.seat(i)?;
            (seat.ray == Side::Bottom).then(|| (i, field.terminates(i) && is_rail(&children[i])))
        })
        .collect();
    // The row stands a fine pitch clear of the deepest ink that is **staying**
    // where it is — the members above it. What slot the grounds themselves
    // were given says nothing: they are the things being moved. Only a scope
    // with no such member — every chain one ground long — has nothing to
    // measure, and there the deepest ground keeps the line it drew.
    let deepest = |rails: bool, of: &dyn Fn(usize) -> f64| {
        riders
            .iter()
            .filter(|&&(_, r)| r == rails)
            .map(|&(i, _)| of(i))
            .max_by(f64::total_cmp)
    };
    let row = deepest(false, &|i| children[i].cy + drawn(&children[i]).max_y)
        .map(|d| f64::from(lat.fine_beyond(d + lat.pitch, 1.0)) * lat.pitch)
        .or_else(|| deepest(true, &|i| point(&children[i]).1));
    let Some(row) = row else {
        return Bbox::empty();
    };
    let mut cells: Vec<(f64, f64)> = Vec::new();
    for &(i, _) in riders.iter().filter(|&&(_, rails)| rails) {
        let (x, y) = point(&children[i]);
        children[i].cy += row - y;
        cells.push((x - lat.col / 2.0, row - lat.row / 2.0));
        cells.push((x + lat.col / 2.0, row + lat.row / 2.0));
    }
    Bbox::from_points(&cells)
}

/// The lattice point a placed part stands on: its **connection geometry**,
/// which is what the field pass stood on the cell [SPEC 16.1] — so the row is
/// the line the wires end on, whatever each terminator's symbol measures.
fn point(node: &PlacedNode) -> (f64, f64) {
    let (bx, by) = seat_point(node);
    (node.cx + bx, node.cy + by)
}

/// Whether a part terminates a rail: a `|label|` wearing one of the rail
/// symbols. Read off `symbol:` and never off a type name — a power rail is the
/// author's own one-line define [SPEC 16.4], and `|gnd|` is only the built-in
/// spelling of the same thing.
fn is_rail(node: &PlacedNode) -> bool {
    sch_kind(&node.type_chain) == Some(SchKind::Label)
        && ident(&node.attrs, "symbol").is_some_and(|s| RAIL_SYMBOLS.contains(&s.as_str()))
}

//! The scope's **rails** [SPEC 16.1] — the ground row and the flag row.
//!
//! Every drafted sheet ends its downward chains on one line: the ground of a
//! lone decoupling cap and the ground of a three-part divider stand on the same
//! row, and the flags feeding them on another. The row is the **scope's**, so
//! it is struck once the satellites are absolute — there is no earlier frame in
//! which two anchors' fields share a line.
//!
//! Which row a terminator joins is its chain's **ray**, never its symbol: a
//! ground posed inverted above a part grows *up*, and rides the flag row that
//! way. The symbol only says whether a terminator rails at all — the returns
//! and the feed do, and the rest of the set (`nc`, `antenna`) ends where it
//! stands [SPEC 16.4].
//!
//! Rails are **vertical only**: a chain running out along a pin's row ends
//! where it ends, which is what both reference sheets draw. Only the terminator
//! moves — its chain keeps every slot above it, and the lead down to the rail is
//! the router's ordinary wire.

use super::field::Field;
use super::lattice::{Ax, Lattice};
use super::terminal::ident;
use crate::desugar::pose::Side;
use crate::desugar::schematic::{SchKind, sch_kind};
use crate::layout::ir::{Bbox, PlacedNode};

/// The symbols a rail row is drawn of [SPEC 16.4]: the three returns and the
/// feed.
const RAIL_SYMBOLS: [&str; 4] = ["gnd", "earth", "chassis", "power"];

/// Sink every downward chain's ground to one row and raise every upward flag to
/// one [SPEC 16.1] — and answer the cells the rails took, which may reach a
/// coarse row past what the fields hold and so past the box the packing struck.
pub(super) fn rails(children: &mut [PlacedNode], field: &Field, lat: Lattice) -> Bbox {
    let mut cells: Vec<(f64, f64)> = Vec::new();
    for ray in [Side::Top, Side::Bottom] {
        let riders: Vec<(usize, bool)> = (0..children.len())
            .filter_map(|i| {
                let seat = field.seat(i)?;
                (seat.ray == ray).then(|| (i, field.terminates(i) && is_rail(&children[i])))
            })
            .collect();
        // The row stands a coarse row past the deepest slot the ray's chains
        // reached, and never short of a rail that already reached deeper — so a
        // scope whose every chain is one member long keeps the line it drew
        // rather than dropping a whole row for nothing.
        let out = Ax::outward(ray);
        let Some(row) = riders
            .iter()
            .map(|&(i, rails)| point(&children[i]).1 + if rails { 0.0 } else { out * lat.row })
            .max_by(|a, b| (a * out).total_cmp(&(b * out)))
        else {
            continue;
        };
        for &(i, _) in riders.iter().filter(|&&(_, rails)| rails) {
            let (x, y) = point(&children[i]);
            children[i].cy += row - y;
            cells.push((x - lat.col / 2.0, row - lat.row / 2.0));
            cells.push((x + lat.col / 2.0, row + lat.row / 2.0));
        }
    }
    Bbox::from_points(&cells)
}

/// The lattice point a placed part stands on: its **body's** centre, which is
/// what the field pass stood on the cell [SPEC 16.1].
fn point(node: &PlacedNode) -> (f64, f64) {
    let (bx, by) = node.bbox.center();
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

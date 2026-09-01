//! The field made **absolute** [SPEC 16.1] — the same cells, once the anchors
//! are placed.
//!
//! A seated satellite rides its anchor: its cell is an offset in that anchor's
//! own frame, so placing the anchor places it, and moving the anchor moves it.
//! A **span** rides no field at all — its members take consecutive coarse cells
//! along the landing leg, the last-named nearest the end they land on — so it
//! is struck here, where both its ends exist, and nowhere earlier.
//!
//! A member stands on its cell **centred**: the lattice point carries the
//! part's own body, so a cap and a resistor on one slot row share it whatever
//! their leads measure.

use super::super::lattice::Ax;
use super::super::net;
use super::super::terminal::{Terminal, seat_point};
use super::{Field, Spanning};
use crate::desugar::pose::Side;
use crate::layout::ir::PlacedNode;

impl Field {
    /// Land every seated satellite on its lattice point, now that its anchor
    /// is placed [SPEC 16.1], and lay the spans along their landing legs.
    pub(in crate::layout::schematic) fn absolutize(&self, children: &mut [PlacedNode]) {
        for i in 0..children.len() {
            let Some(seat) = self.seat(i) else { continue };
            let (dx, dy) = self.point(seat);
            let (ax, ay) = (children[seat.anchor].cx, children[seat.anchor].cy);
            stand(&mut children[i], (ax + dx, ay + dy), Some(seat.side));
        }
        for span in &self.spans {
            self.lay(children, span);
        }
    }

    /// The coarse cells a span asks of the region between its two anchors
    /// [SPEC 16.1] — one per member.
    pub(in crate::layout::schematic) fn span_cells(&self, span: &Spanning) -> i32 {
        span.members.len() as i32
    }

    /// One span's members on the **landing leg** [SPEC 16.1]: the straight run
    /// into the second-named end, on that pin's own line, its members on
    /// consecutive coarse cells back from that end — the last-named nearest
    /// it, so the wire order reads along the leg.
    ///
    /// The cells are the **scope's**, counted out from the landing anchor's
    /// own origin past whatever its field already holds ([`Field::free`]) —
    /// which is the very count the packer parted the two tracks by, so the
    /// members land in the region it reserved for them. Counted from the
    /// landing itself they would not: a pin stands on a *fine* line, so its
    /// own leg's cells fall between the scope's.
    fn lay(&self, children: &mut [PlacedNode], span: &Spanning) {
        let landing = |(child, t): &(usize, Terminal)| {
            let n = &children[*child];
            (n.cx + t.at.0, n.cy + t.at.1)
        };
        let (from, at) = (landing(&span.ends[0]), landing(&span.ends[1]));
        // The leg runs along the landing pin's own normal; a pin with no
        // facing states none, and the chord's longer axis stands in.
        let ax = span.ends[1].1.facing.map_or_else(
            || {
                if (at.0 - from.0).abs() >= (at.1 - from.1).abs() {
                    Ax::X
                } else {
                    Ax::Y
                }
            },
            Ax::of,
        );
        // Back toward the first end, whichever way the second pin faces.
        let anchor = span.ends[1].0;
        let node = &children[anchor];
        let origin = coordinate((node.cx, node.cy), ax);
        let back = match (ax, coordinate(from, ax) < coordinate(at, ax)) {
            (Ax::X, true) => Side::Left,
            (Ax::X, false) => Side::Right,
            (Ax::Y, true) => Side::Top,
            (Ax::Y, false) => Side::Bottom,
        };
        let step = self.lat.step(ax) * Ax::outward(back);
        let across = coordinate(at, ax.other());
        let free = (self.free(anchor, back) / self.lat.step(ax)).ceil();
        let first = free + span.members.len() as f64 - 1.0;
        for (k, &member) in span.members.iter().enumerate() {
            let line = origin + step * (first - k as f64);
            let point = match ax {
                Ax::X => (line, across),
                Ax::Y => (across, line),
            };
            stand(&mut children[member], point, None);
        }
    }
}

/// Stand a part's **connection geometry** on a lattice point [SPEC 16.1] —
/// never its drawn box, so a flag's symbol lands on the wire's line and its
/// name hangs off ([`seat_point`]) — and step the name of a net run off the
/// trace it rides ([`net::seat_text`]), `outward` being the side away from the
/// anchor whose field holds it and `None` where no field does. A part's own
/// ref / value pair turns outward too, but later and by its own pass
/// ([`super::super::readout`]), which reads the seat rather than this one hint.
fn stand(node: &mut PlacedNode, at: (f64, f64), outward: Option<Side>) {
    let (bx, by) = seat_point(node);
    node.cx = at.0 - bx;
    node.cy = at.1 - by;
    if net::is_run(node) {
        let (dx, dy) = net::seat_text(node, outward);
        for c in node.children.iter_mut() {
            c.cx += dx;
            c.cy += dy;
        }
    }
}

/// A point's coordinate on one lattice axis.
fn coordinate(at: (f64, f64), ax: Ax) -> f64 {
    match ax {
        Ax::X => at.0,
        Ax::Y => at.1,
    }
}

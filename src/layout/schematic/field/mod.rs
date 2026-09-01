//! The **field pass** [SPEC 16.1] — every satellite's cell, struck before a
//! single anchor is placed.
//!
//! A chain is a **walk**: it takes a *ray* (the direction it grows), a *lane*
//! (its line across that ray — a coarse line out from the anchor's ink where
//! it turned off its pin, the pin's own fine line where it grew straight out)
//! and a *slot* per member (the k-th coarse line along the ray from the field
//! origin). Nothing here reads ink but the field origin itself: a satellite's
//! cell comes from the lattice, never from its symbol's size or the width of
//! its value.
//!
//! **Collision is the cells'.** A member's cell is one `gap` square on its
//! lattice point; a lane is free when no cell of the chain meets one already
//! committed, and a taken lane steps out a coarse line and tries again
//! ([`allocate`]). That one test is the whole of it — an up-chain and a
//! down-chain off one pin share a lane because their cells are disjoint, and
//! no chain lands where a lead must cross because the crossed part is in the
//! set. A lead reserves nothing; the **lane order** keeps it clear.
//!
//! The chains, the growth ray, the tap classifier and the limb split are all
//! [`crate::desugar::schematic::chain`]'s, shared with the pose chooser, so a
//! part is never posed for one ray and seated along another.

// The field is built and its allocator tested before the pass that reads it;
// `place` still runs the seat pass, and picks this up at the switchover.
#![allow(dead_code)]

mod walk;

use super::lattice::{Ax, Lattice};
use super::seat::{drawn, edges};
use super::terminal::{Terminal, terminal};
use crate::desugar::pose::Side;
use crate::desugar::schematic::Role;
use crate::desugar::schematic::chain::{Chain, End, chains, holder, placed_ends};
use crate::layout::ir::{Bbox, PlacedNode};
use crate::resolve::ResolvedLink;

/// Where a satellite sits [SPEC 16.1], in cells, before the tracks size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Seat {
    /// The anchor whose field holds it.
    pub anchor: usize,
    /// The direction its chain grows.
    pub ray: Side,
    /// The side of the anchor its lead leaves by — the pin's own normal.
    pub side: Side,
    /// Coarse lanes out from the anchor's ink on `side`, 1-based; `None` for
    /// a seat that keeps a fine line instead.
    pub lane: Option<i32>,
    /// The fine line a laneless seat keeps, in the anchor's own frame: its
    /// pin's own for a chain that grew straight out, its attachment's stepped
    /// one cell for anything hanging beside such a chain.
    pub pin_line: f64,
    /// Coarse slots along `ray` from the field origin, 1-based.
    pub slot: i32,
}

/// A chain held at two *different* anchors [SPEC 16.1]: it sits between the
/// two fields rather than in one, so no anchor's lane order can speak for it.
#[derive(Clone, Debug)]
pub(super) struct Spanning {
    pub members: Vec<usize>,
    pub ends: [(usize, Terminal); 2],
}

/// Every satellite's cell in one scope.
pub(super) struct Field {
    seats: Vec<Option<Seat>>,
    spans: Vec<Spanning>,
    floating: Vec<usize>,
    /// Per anchor, the cells its field has committed — the whole of collision.
    cells: Vec<Vec<Bbox>>,
    /// Per anchor, its field origin on each side, by [`Side::index`].
    origins: Vec<[i32; 4]>,
    lat: Lattice,
}

impl Field {
    pub(super) fn build(
        children: &[PlacedNode],
        roles: &[Role],
        links: &[&ResolvedLink],
        scope: &str,
        lat: Lattice,
    ) -> Field {
        let satellite: Vec<bool> = roles.iter().map(|r| *r == Role::Satellite).collect();
        let mut field = Field {
            seats: vec![None; children.len()],
            spans: Vec::new(),
            floating: Vec::new(),
            cells: vec![Vec::new(); children.len()],
            origins: children.iter().map(|c| origins(c, lat)).collect(),
            lat,
        };
        if !satellite.contains(&true) {
            return field;
        }
        let wires = edges(children, links, scope);
        let mut chains_held: Vec<(Chain, End)> = Vec::new();
        for chain in chains(&satellite, &wires) {
            let ends = placed_ends(&chain, roles);
            // One anchor holds it → grow off that pin; two → a span between
            // them; none → the flow fallback. Which of the first two a chain
            // is, is [`holder`]'s single answer, shared with the pose chooser
            // — so a bridge needs no case of its own here.
            match (holder(&ends), ends.as_slice()) {
                (Some(one), _) => chains_held.push((chain, one.clone())),
                (None, [a, b, ..]) => field.spans.push(Spanning {
                    members: chain.members,
                    ends: [landing(children, a), landing(children, b)],
                }),
                (None, _) => field.floating.extend(chain.members),
            }
        }
        field.walk(children, &wires, chains_held);
        field
    }

    /// The seat of child `i`, if a chain held it.
    pub(super) fn seat(&self, i: usize) -> Option<Seat> {
        self.seats[i]
    }

    /// How many coarse lanes `anchor`'s field takes on `side`.
    pub(super) fn lanes(&self, anchor: usize, side: Side) -> i32 {
        self.reach(anchor, side)
    }

    /// How many coarse slots deep `anchor`'s field runs along `ray`.
    pub(super) fn depth(&self, anchor: usize, ray: Side) -> i32 {
        self.reach(anchor, ray)
    }

    /// Satellites no wire held — the flow fallback [SPEC 16.1].
    pub(super) fn floating(&self) -> &[usize] {
        &self.floating
    }

    /// The chains held at two anchors [SPEC 16.1], which ride no field.
    pub(super) fn spans(&self) -> &[Spanning] {
        &self.spans
    }

    /// Seat one member, and commit its cell to its anchor's occupancy.
    fn take(&mut self, member: usize, seat: Seat) {
        let cell = self.cell(seat);
        self.cells[seat.anchor].push(cell);
        self.seats[member] = Some(seat);
    }

    /// Where a ladder stands at step `k`: the lane it names, and the
    /// coordinate that lies on.
    fn ladder_at(&self, anchor: usize, ladder: Ladder, k: i32) -> (Option<i32>, f64) {
        match ladder {
            Ladder::Lanes(side) => (Some(k), self.coord(anchor, side, k)),
            Ladder::Beside(side, base) => (None, base + f64::from(k) * self.step(side)),
        }
    }

    /// The seat one coarse step `across` from this one, on its own slot —
    /// where a tap hangs, and where a branch crossing the trunk marches.
    fn stepped(&self, seat: Seat, across: Side) -> Seat {
        match seat.lane {
            // Stepping along the lane axis is stepping a lane.
            Some(k) if Ax::of(seat.side) == Ax::of(across) => Seat {
                lane: Some(k + if across == seat.side { 1 } else { -1 }),
                ..seat
            },
            _ => Seat {
                lane: None,
                pin_line: self.cross(seat) + self.step(across),
                ..seat
            },
        }
    }

    /// Where a seat's cell centres, in its anchor's own frame: the slot line
    /// along its ray, and the lane line — or the fine line it kept — across.
    fn point(&self, seat: Seat) -> (f64, f64) {
        let along = self.coord(seat.anchor, seat.ray, seat.slot);
        let cross = self.cross(seat);
        match Ax::of(seat.ray) {
            Ax::X => (along, cross),
            Ax::Y => (cross, along),
        }
    }

    /// The line a seat stands on across its ray.
    fn cross(&self, seat: Seat) -> f64 {
        match seat.lane {
            Some(k) => self.coord(seat.anchor, seat.side, k),
            None => seat.pin_line,
        }
    }

    /// A member's cell: one `gap` square on its lattice point [SPEC 16.1] —
    /// the cell, never the part's ink.
    fn cell(&self, seat: Seat) -> Bbox {
        let (x, y) = self.point(seat);
        Bbox::centered(self.lat.col, self.lat.row).shifted(x, y)
    }

    /// How far out on `side` an anchor's field reaches, in coarse lines — the
    /// lanes a track must hold there, and the slots a ray runs deep. **One**
    /// measure: a lane and a slot are the same count, read on the two axes.
    fn reach(&self, anchor: usize, side: Side) -> i32 {
        let ax = Ax::of(side);
        let (sign, step) = (Ax::outward(side), self.lat.step(ax));
        let base = self.coord(anchor, side, 1) * sign;
        self.cells[anchor]
            .iter()
            .map(|c| {
                let (x, y) = c.center();
                let v = if ax == Ax::X { x } else { y };
                ((v * sign - base) / step - EPS).ceil() as i32 + 1
            })
            .max()
            .unwrap_or(0)
            .max(0)
    }

    /// The `k`-th coarse line out from `anchor`'s ink on `side` — a **lane**
    /// read across a ray, a **slot** read along one, which is the same count
    /// on the two axes [SPEC 16.1]. As a line index, and as a coordinate.
    fn line_of(&self, anchor: usize, side: Side, k: i32) -> i32 {
        self.origins[anchor][side.index()] + (k - 1) * out(side)
    }

    fn coord(&self, anchor: usize, side: Side, k: i32) -> f64 {
        self.lat.line(Ax::of(side), self.line_of(anchor, side, k))
    }

    /// …and back: which `k` a line index is.
    fn ordinal(&self, anchor: usize, side: Side, line: i32) -> i32 {
        (line - self.origins[anchor][side.index()]) * out(side) + 1
    }

    /// One coarse step the way `side` faces.
    fn step(&self, side: Side) -> f64 {
        self.lat.step(Ax::of(side)) * Ax::outward(side)
    }
}

/// How a run finds its line **across** its ray [SPEC 16.1]: the lane ladder
/// out from the anchor's ink, or fine lines stepped **beside** the pin line a
/// straight-grown chain kept. `k` counts lanes from 1 and beside-steps from
/// 0 — step 0 being the line that chain itself keeps.
#[derive(Clone, Copy)]
enum Ladder {
    Lanes(Side),
    Beside(Side, f64),
}

/// The innermost lane whose cells meet nothing already committed [SPEC 16.1].
/// The lead reserves nothing: the lane order — the pin deeper along the ray
/// keeping the inner lane — already crosses a column only above where that
/// column is live. Overlap is [`Bbox::overlaps`]'s, which is strict, so
/// adjacent cells are legal neighbours rather than a lane marching outward
/// for ever.
fn allocate(taken: &[Bbox], cells: impl Fn(i32) -> Vec<Bbox>) -> i32 {
    (1..)
        .find(|&k| {
            cells(k)
                .iter()
                .all(|c| !taken.iter().any(|t| t.overlaps(*c)))
        })
        .expect("an unbounded lattice always has a free lane")
}

/// One anchor's **field origins** [SPEC 16.1]: the first coarse line clear of
/// its own drawn ink on each side, readouts included. Read in the anchor's own
/// frame, which placing it never changes, so the ordinals a seat records
/// outlive the tracks.
fn origins(node: &PlacedNode, lat: Lattice) -> [i32; 4] {
    let ink = drawn(node);
    Side::ALL.map(|s| {
        let edge = match s {
            Side::Left => ink.min_x,
            Side::Right => ink.max_x,
            Side::Top => ink.min_y,
            Side::Bottom => ink.max_y,
        };
        lat.beyond(Ax::of(s), edge, Ax::outward(s))
    })
}

/// `+1` when a side's normal points the increasing way, `-1` otherwise — the
/// integer twin of [`Ax::outward`] that lattice line indices step by.
fn out(side: Side) -> i32 {
    if Ax::outward(side) > 0.0 { 1 } else { -1 }
}

/// A span end: the anchor it lands on, and the terminal it lands at.
fn landing(children: &[PlacedNode], end: &End) -> (usize, Terminal) {
    (
        end.child,
        terminal(&children[end.child], end.terminal.as_deref()),
    )
}

/// Slack for the one division this module does: a cell centre sits on its
/// line to the last bit, so anything larger than rounding noise is a real
/// step out.
const EPS: f64 = 1e-9;

#[cfg(test)]
mod tests {
    use super::*;

    /// A chain's cells at lane `k`, on a left side growing down: one column,
    /// `n` cells deep, in a 100-pitch lattice.
    fn column(k: i32, n: i32) -> Vec<Bbox> {
        (1..=n)
            .map(|slot| {
                Bbox::centered(100.0, 100.0).shifted(-100.0 * k as f64, 100.0 * slot as f64)
            })
            .collect()
    }

    #[test]
    fn the_innermost_free_lane_wins() {
        assert_eq!(
            allocate(&[], |k| column(k, 2)),
            1,
            "an empty field takes lane 1"
        );
    }

    #[test]
    fn an_occupied_lane_steps_out_one_and_retries() {
        let taken = column(1, 2);
        assert_eq!(allocate(&taken, |k| column(k, 2)), 2);
    }

    #[test]
    fn opposite_rays_off_one_pin_share_a_lane() {
        // [SPEC 16.1] the down-chain's cells and the up-chain's are disjoint,
        // so the second one's first candidate is already free — the column
        // sharing is a consequence of the occupancy test, not a rule.
        let down = column(1, 2);
        let up = |k: i32| {
            (1..=2)
                .map(|slot| {
                    Bbox::centered(100.0, 100.0).shifted(-100.0 * k as f64, -100.0 * slot as f64)
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(allocate(&down, up), 1, "one lane, two rays");
    }

    #[test]
    fn a_deeper_chain_only_needs_the_lanes_its_own_cells_meet() {
        // Lane 1 is held two cells deep; a four-cell chain still cannot use
        // it, but a chain starting past it can.
        let taken = column(1, 2);
        let deep = |k: i32| {
            (3..=4)
                .map(|slot| {
                    Bbox::centered(100.0, 100.0).shifted(-100.0 * k as f64, 100.0 * slot as f64)
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(allocate(&taken, deep), 1, "below what lane 1 holds");
    }
}

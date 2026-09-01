//! The **field pass** [SPEC 16.1] — every satellite's cell, struck before a
//! single anchor is placed.
//!
//! A chain is a **walk**: it takes a *ray* (the direction it grows), a *lane*
//! (its line across that ray — a coarse line out from the anchor's ink where
//! it turned off its pin, the pin's own fine line where it grew straight out)
//! and a *slot* per member (the k-th coarse line along the ray from the field
//! origin). A satellite's cell comes from the lattice, never from the width of
//! its value: the only ink this pass reads is the field origin, and how far a
//! label's own symbol reaches across the line it stands on ([`Field::across`])
//! — both quantised straight back onto the lattice [SPEC 16.1].
//!
//! **Collision is the cells'.** A member's cell is one `gap` pitch along its
//! ray, and across it the pitch of the line it stands on: a **part** takes the
//! coarse cell wherever it stands, a **label** on a pin's own line takes that
//! fine line ([`Body`]). A lane is free when no cell of the chain meets one
//! already committed, and a taken lane steps out a coarse line and tries again
//! ([`allocate`]). That one test is the whole of it — an
//! up-chain and a down-chain off one pin share a lane because their cells are
//! disjoint; a second chain claiming a pin's straight corridor steps beside it
//! because they are not; and no chain lands where a lead must cross, because
//! the crossed part is in the set. A lead reserves nothing; the **lane order**
//! keeps it clear.
//!
//! The chains, the growth ray, the tap classifier and the limb split are all
//! [`crate::desugar::schematic::chain`]'s, shared with the pose chooser, so a
//! part is never posed for one ray and seated along another.

mod absolute;
mod read;
mod walk;

use super::lattice::{Ax, Lattice};
use super::place::Slot;
use super::terminal::{self, Terminal, terminal};
use crate::desugar::pose::Side;
use crate::desugar::schematic::chain::{Chain, End, chains, holder, placed_ends};
use crate::desugar::schematic::{Role, SchKind, sch_kind};
use crate::layout::ir::{Bbox, PlacedNode};
use crate::resolve::{LinkKind, ResolvedLink};

/// The two classes a slot origin is struck for [SPEC 16.1] — which is to say,
/// what a chain's own lead passes on its way out of the pin.
pub(super) const STRAIGHT: usize = 0;
pub(super) const LANED: usize = 1;

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
    /// The **fine** line it stands on along `ray` [SPEC 16.1] — absolute in
    /// the anchor's own frame, not an ordinal, so a branch grown back along
    /// the opposite ray needs no second origin to be read against.
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
    /// Per anchor, its **lane** origin on each side, by [`Side::index`]: the
    /// first coarse line whose cell clears the anchor's ink that way.
    lanes: Vec<[i32; 4]>,
    /// Per anchor, its **slot** origin along each ray, by [`Side::index`] and
    /// then by whether the chain turned — in **fine** lines, because a slot
    /// clears what its own lead passes and that is no coarse distance
    /// [SPEC 16.1].
    slots: Vec<[[i32; 2]; 4]>,
    /// Per child, what its cell has to hold **across** its ray.
    bodies: Vec<Body>,
    /// Which children end a **run** — a trunk's terminator, or a branch's.
    terminators: Vec<bool>,
    lat: Lattice,
}

impl Field {
    pub(super) fn build(
        children: &[PlacedNode],
        roles: &[Role],
        links: &[&ResolvedLink],
        scope: &str,
        tracks: &[Option<Slot>],
        lat: Lattice,
    ) -> Field {
        let satellite: Vec<bool> = roles.iter().map(|r| *r == Role::Satellite).collect();
        let mut field = Field {
            seats: vec![None; children.len()],
            spans: Vec::new(),
            floating: Vec::new(),
            cells: vec![Vec::new(); children.len()],
            lanes: children.iter().map(|c| lane_origins(c, lat)).collect(),
            slots: children.iter().map(|c| slot_origins(c, lat)).collect(),
            bodies: children.iter().map(Body::of).collect(),
            terminators: vec![false; children.len()],
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
        field.walk(children, &wires, tracks, chains_held);
        field
    }

    /// The seat of child `i`, if a chain held it.
    pub(super) fn seat(&self, i: usize) -> Option<Seat> {
        self.seats[i]
    }

    /// How far out on `side` an anchor's field reaches from the anchor's **own
    /// origin**, as a distance to the outermost cell centre — the lanes a
    /// track must hold there, and the slots a ray runs deep [SPEC 16.1]. `0.0`
    /// where no chain went that way.
    ///
    /// A distance and no longer a count of cells: a lane is a coarse line and
    /// a slot a fine one, so the two no longer share a unit.
    pub(in crate::layout::schematic) fn extent(&self, anchor: usize, side: Side) -> f64 {
        self.reach(anchor, side)
    }

    /// The first line out on `side` that an anchor's field leaves **free**,
    /// as a distance from the anchor's own origin [SPEC 16.1] — where the next
    /// thing on that side may stand. With nothing there it is the **lane**
    /// origin, which already stands a whole cell clear of the anchor's own ink:
    /// one measure for the packer's tracks and for the members of a span
    /// landing there.
    pub(in crate::layout::schematic) fn free(&self, anchor: usize, side: Side) -> f64 {
        match self.reach(anchor, side) {
            d if d <= 0.0 => self.lane_coord(anchor, side, 1).abs(),
            d => d + self.lat.step(Ax::of(side)),
        }
    }

    /// Satellites no wire held — the flow fallback [SPEC 16.1].
    pub(super) fn floating(&self) -> &[usize] {
        &self.floating
    }

    /// The chains held at two anchors [SPEC 16.1], which ride no field.
    pub(super) fn spans(&self) -> &[Spanning] {
        &self.spans
    }

    /// Whether a member is the last of a **run** — the trunk out from its pin,
    /// or a branch grown along its own ray — and so the terminator whose own
    /// drawing set that ray. The rails read it [SPEC 16.1]: a **tap** ends
    /// nothing, hanging beside the junction it taps rather than growing to it,
    /// and a branch that marched *across* its trunk carries the trunk's ray on
    /// its seat, so neither is a rail's to move.
    pub(in crate::layout::schematic) fn terminates(&self, i: usize) -> bool {
        self.terminators[i]
    }

    /// Seat one member, and commit its cell to its anchor's occupancy.
    fn take(&mut self, member: usize, seat: Seat) {
        let cell = self.cell(member, seat);
        self.cells[seat.anchor].push(cell);
        self.seats[member] = Some(seat);
    }

    /// Where a ladder stands at step `k`: the lane it names, and the
    /// coordinate that lies on.
    fn ladder_at(&self, anchor: usize, ladder: Ladder, k: i32) -> (Option<i32>, f64) {
        match ladder {
            Ladder::Lanes(side) => (Some(k), self.lane_coord(anchor, side, k)),
            Ladder::Beside(side, base) => (None, base + f64::from(k - 1) * self.step(side)),
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
        let along = f64::from(seat.slot) * self.lat.pitch;
        let cross = self.cross(seat);
        match Ax::of(seat.ray) {
            Ax::X => (along, cross),
            Ax::Y => (cross, along),
        }
    }

    /// The line a seat stands on across its ray.
    fn cross(&self, seat: Seat) -> f64 {
        match seat.lane {
            Some(k) => self.lane_coord(seat.anchor, seat.side, k),
            None => seat.pin_line,
        }
    }

    /// A member's cell [SPEC 16.1] — one coarse pitch **along** its ray, and
    /// across it the pitch of the line it stands on ([`Field::across`]).
    fn cell(&self, member: usize, seat: Seat) -> Bbox {
        let (x, y) = self.point(seat);
        let ray = Ax::of(seat.ray);
        let along = self.lat.step(ray);
        let across = self.across(member, seat);
        let (w, h) = match ray {
            Ax::X => (along, across),
            Ax::Y => (across, along),
        };
        Bbox::centered(w, h).shifted(x, y)
    }

    /// How wide a member's cell stands **across** its ray [SPEC 16.1]: the
    /// pitch of the line it stands on. A **part** takes the coarse cell
    /// wherever it stands — its ref and value stand off its body by rule and
    /// the cell is the room they need — and so does anything on a lane, a lane
    /// being a coarse column. A **label** on a pin's own fine line takes that
    /// line, widened to the whole fine pitches its own symbol draws across it:
    /// a label is its own terminal and no part [SPEC 16.4], so a no-connect
    /// cross seated off one pin leaves the pins either side their own rows,
    /// and a net run — a stretch of trace with no drawing at all — takes
    /// exactly the line it lands on.
    fn across(&self, member: usize, seat: Seat) -> f64 {
        let ax = Ax::of(seat.ray).other();
        match self.bodies[member] {
            Body::Tag(symbol) if seat.lane.is_none() => {
                let drawn = symbol.map_or(0.0, |b| match ax {
                    Ax::X => b.w(),
                    Ax::Y => b.h(),
                });
                self.lat.pitch.max(self.lat.pitches(drawn))
            }
            _ => self.lat.step(ax),
        }
    }

    /// How far out on `side` an anchor's field reaches, as a distance from the
    /// anchor's own origin to the outermost cell centre.
    fn reach(&self, anchor: usize, side: Side) -> f64 {
        let (ax, sign) = (Ax::of(side), Ax::outward(side));
        self.cells[anchor]
            .iter()
            .map(|c| {
                let (x, y) = c.center();
                (if ax == Ax::X { x } else { y }) * sign
            })
            .fold(0.0f64, f64::max)
    }

    /// The `k`-th **lane** out from `anchor`'s ink on `side` [SPEC 16.1] — a
    /// coarse line, because a lane carries a part's whole cell.
    pub(super) fn lane_line(&self, anchor: usize, side: Side, k: i32) -> i32 {
        self.lanes[anchor][side.index()] + (k - 1) * out(side)
    }

    fn lane_coord(&self, anchor: usize, side: Side, k: i32) -> f64 {
        self.lat.line(Ax::of(side), self.lane_line(anchor, side, k))
    }

    /// The `k`-th **slot** along `ray` from `anchor`'s field origin
    /// [SPEC 16.1] — a **fine** line index, stepping a coarse pitch per slot.
    /// Which origin is the chain's own: one that turned into a lane clears the
    /// deepest pin on that ray, one that grew straight out clears the ink.
    pub(super) fn slot_line(&self, anchor: usize, ray: Side, turned: bool, k: i32) -> i32 {
        self.slots[anchor][ray.index()][usize::from(turned)] + (k - 1) * self.stride(ray)
    }

    /// One coarse step along `ray`, in **fine** lines and signed the way it
    /// faces — what a slot steps by. The lattice rounds a coarse pitch up to a
    /// whole number of fine ones, so this is exact.
    pub(super) fn stride(&self, ray: Side) -> i32 {
        let whole = (self.lat.step(Ax::of(ray)) / self.lat.pitch).round() as i32;
        whole * out(ray)
    }

    /// One coarse step the way `side` faces.
    fn step(&self, side: Side) -> f64 {
        self.lat.step(Ax::of(side)) * Ax::outward(side)
    }
}

/// What a member's cell has to hold **across** its ray [SPEC 16.1] — the one
/// split SPEC 16.4 opens with: components have pins, and a label is its own
/// terminal.
#[derive(Clone, Copy)]
enum Body {
    /// A **part** — a component, a discrete, an amplifier. It wears a
    /// reference designator, so it takes a coarse cell across its ray wherever
    /// it stands: the ref and the value stand off its body by rule.
    Part,
    /// A **label**: a terminal, not a part, so it asks only for the line its
    /// wire runs on — widened to whatever its own symbol draws across that
    /// line ([`terminal::body`]), and to nothing at all for a net run, which
    /// draws none.
    Tag(Option<Bbox>),
}

impl Body {
    fn of(node: &PlacedNode) -> Body {
        match sch_kind(&node.type_chain) {
            Some(SchKind::Label) => Body::Tag(terminal::body(node)),
            _ => Body::Part,
        }
    }
}

/// How a run finds its line **across** its ray [SPEC 16.1]: the lane ladder
/// out from the anchor's ink, or coarse steps **beside** the pin line a
/// straight-grown chain kept. Both count from 1 — step 1 of a beside ladder is
/// the pin's own line — so the one allocator walks either.
#[derive(Clone, Copy)]
enum Ladder {
    Lanes(Side),
    Beside(Side, f64),
}

/// The innermost step of a [`Ladder`] whose cells meet nothing already
/// committed [SPEC 16.1] — a lane for a chain that turned off its pin, the
/// pin's own line for one that grew straight out.
///
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

/// A part's **drawn** extent: its box unioned with every descendant, so the
/// chrome that pokes outside counts. A ref or a value readout is a `pin:`
/// overlay — out of the flow that sized the box, but ink on the sheet and an
/// obstacle to the router all the same (the scene index carries it as the
/// part's `overflow`). A field origin measures this, or a chain clears a part's
/// box and starts on its readout.
pub(super) fn drawn(node: &PlacedNode) -> Bbox {
    Bbox::drawn_of(node)
}

/// The scope's wires as chain edges, one per hop, in statement order: the
/// twin of [`crate::desugar::autopose`]'s reader on the resolved side, so the
/// chains the pose chooser turned are the chains this pass seats.
pub(super) fn edges(
    children: &[PlacedNode],
    links: &[&ResolvedLink],
    scope: &str,
) -> Vec<[End; 2]> {
    // An endpoint dot-paths from the scene root; a child of this scope is
    // everything after the scope's own prefix [SPEC 9].
    let end = |path: &str| {
        let local = crate::resolve::scene::within_scope(path, scope)?;
        let (head, terminal) = match local.split_once('.') {
            Some((head, rest)) => (head, Some(rest.to_string())),
            None => (local, None),
        };
        let child = children
            .iter()
            .position(|c| c.id.as_deref() == Some(head))?;
        Some(End { child, terminal })
    };
    let mut out = Vec::new();
    for w in links.iter().filter(|w| w.kind == LinkKind::Wire) {
        for hop in w.endpoints.windows(2) {
            if let (Some(a), Some(b)) = (end(&hop[0].path), end(&hop[1].path)) {
                out.push([a, b]);
            }
        }
    }
    out
}

/// One anchor's **field origins** [SPEC 16.1]: the first coarse line clear of
/// its own drawn ink on each side, readouts included. A member stands
/// **centred** on its line, so the line a field starts on is the first one
/// whose whole cell is past the ink — measured from half a cell out, or a part
/// whose edge falls just inside a coarse line prints its neighbour's column
/// over its own pin numbers.
///
/// Read in the anchor's own frame, which placing it never changes, so the
/// ordinals a seat records outlive the tracks.
fn lane_origins(node: &PlacedNode, lat: Lattice) -> [i32; 4] {
    let ink = drawn(node);
    Side::ALL.map(|s| {
        let (ax, out) = (Ax::of(s), Ax::outward(s));
        lat.beyond(ax, ink_edge(&ink, s) + out * lat.step(ax) / 2.0, out)
    })
}

/// A node's **slot** origins [SPEC 16.1], per ray and then per class, as fine
/// line indices — seeded here from the anchor's own ink, which is what a chain
/// growing **straight** out of a pin really passes. The walk overwrites the
/// laned class, and shares both across the track line, once it knows which
/// chains grew which way.
fn slot_origins(node: &PlacedNode, lat: Lattice) -> [[i32; 2]; 4] {
    let ink = drawn(node);
    Side::ALL.map(|s| {
        let line = straight_origin(&ink, s, lat);
        [line, line]
    })
}

/// The first fine line whose cell clears `ink` on `side` — the slot origin of
/// a chain whose ray points through the anchor's body.
fn straight_origin(ink: &Bbox, side: Side, lat: Lattice) -> i32 {
    lat.fine_beyond(ink_edge(ink, side) + clear(side, lat), Ax::outward(side))
}

/// What a slot origin stands clear of whatever it measures [SPEC 16.1], signed
/// outward: half a cell, because a member centres on its line, and then the
/// one **fine pitch** two wired neighbours always keep — without it a member's
/// body lands exactly on the ink it was cleared of, and the wire between them
/// has no track to run on.
pub(super) fn clear(side: Side, lat: Lattice) -> f64 {
    Ax::outward(side) * (lat.step(Ax::of(side)) / 2.0 + lat.pitch)
}

/// How far a box reaches on one side, in its own frame.
fn ink_edge(ink: &Bbox, side: Side) -> f64 {
    match side {
        Side::Left => ink.min_x,
        Side::Right => ink.max_x,
        Side::Top => ink.min_y,
        Side::Bottom => ink.max_y,
    }
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

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
use super::terminal::{Terminal, terminal};
use crate::desugar::pose::Side;
use crate::desugar::schematic::chain::{Chain, End, chains, holder, placed_ends};
use crate::desugar::schematic::{Role, SchKind, sch_kind};
use crate::layout::ir::{Bbox, PlacedNode};
use crate::resolve::{LinkKind, ResolvedLink};

/// The two classes a slot origin is struck for [SPEC 16.1] — which is to say,
/// what a chain's own lead passes on its way out of the pin.
pub(super) const STRAIGHT: usize = 0;
pub(super) const LANED: usize = 1;

/// Where a satellite sits [SPEC 16.1], in its anchor's own frame, before the
/// tracks size. Both coordinates are **fine** lines: a lane and a slot are
/// found by stepping one out at a time until the cell clears, so neither is an
/// ordinal of anything.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Seat {
    /// The anchor whose field holds it.
    pub anchor: usize,
    /// The direction its chain grows.
    pub ray: Side,
    /// The side of the anchor its lead leaves by — the pin's own normal.
    pub side: Side,
    /// The line it stands on **across** its ray.
    pub cross: f64,
    /// …and **along** it.
    pub along: f64,
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
    /// Per anchor, where its field's **first** slot stands along each ray, by
    /// [`Side::index`] and then by whether the chain turned — the one line the
    /// track row shares, and the only one that is a rule rather than a search
    /// [SPEC 16.1].
    slots: Vec<[[f64; 2]; 4]>,
    /// Per child, the box its own drawing takes — what its cell holds.
    boxes: Vec<Bbox>,
    /// Per child, everything it draws, readouts included — what a field origin
    /// and a lane's base stand clear of.
    inks: Vec<Bbox>,
    /// Per child, what its cell has to hold **across** its ray.
    bodies: Vec<Body>,
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
            slots: vec![[[0.0; 2]; 4]; children.len()],
            boxes: children.iter().map(|c| c.bbox).collect(),
            inks: children.iter().map(drawn).collect(),
            bodies: children.iter().map(Body::of).collect(),
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

    /// The first line out on `side` that an anchor's field leaves **free**, as
    /// a distance from the anchor's own origin [SPEC 16.1] — where the next
    /// thing on that side may stand, and with nothing there, the one clear
    /// column two neighbouring anchors keep. Their own ink is the packer's to
    /// read; this answers only what the field holds.
    pub(in crate::layout::schematic) fn free(&self, anchor: usize, side: Side) -> f64 {
        self.reach(anchor, side) + self.lat.step(Ax::of(side))
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
        let cell = self.cell(member, seat);
        self.cells[seat.anchor].push(cell);
        self.seats[member] = Some(seat);
    }

    /// Where a run's cross ladder stands at step `k` [SPEC 16.1] — one **fine**
    /// line out per step from its own base, so what the cells need decides the
    /// spacing rather than a pitch stated in advance.
    fn ladder_at(&self, ladder: Ladder, k: i32) -> f64 {
        ladder.base + f64::from(k - 1) * self.lat.pitch * Ax::outward(ladder.side)
    }

    /// The seat one step `across` from this one — where a tap hangs, and where
    /// a branch crossing the trunk marches. The step is the two drawings' own:
    /// half of what stands here, half of what stands there, and out to the
    /// first fine line past it.
    fn stepped(&self, at: (usize, Seat), member: usize, across: Side) -> Seat {
        let out = Ax::outward(across);
        let half = |i: usize| self.across(i, at.1.ray) / 2.0;
        Seat {
            cross: self
                .lat
                .past(at.1.cross + out * (half(at.0) + half(member)), out),
            ..at.1
        }
    }

    /// Where a seat's cell centres, in its anchor's own frame.
    fn point(&self, seat: Seat) -> (f64, f64) {
        match Ax::of(seat.ray) {
            Ax::X => (seat.along, seat.cross),
            Ax::Y => (seat.cross, seat.along),
        }
    }

    /// A member's **cell** [SPEC 16.1] — what its own drawing takes, and no
    /// pitch stated in advance: across its ray by [`Field::across`], along it
    /// by [`Field::along`].
    fn cell(&self, member: usize, seat: Seat) -> Bbox {
        let (x, y) = self.point(seat);
        let (along, across) = (self.along(member, seat.ray), self.across(member, seat.ray));
        let (w, h) = match Ax::of(seat.ray) {
            Ax::X => (along, across),
            Ax::Y => (across, along),
        };
        Bbox::centered(w, h).shifted(x, y)
    }

    /// How wide a member's cell stands **across** its ray [SPEC 16.1] — the
    /// one split SPEC 16.4 opens with. A **part** takes the coarse cell: it
    /// wears a ref and a value beside its body, and that cell is the room they
    /// need, which is the one thing `gap` states. A **label** is its own
    /// terminal and no part, so it takes the whole fine pitches its own symbol
    /// draws across the line — a no-connect cross grown off one pin leaves the
    /// pins either side their rows, a net run takes exactly the line it lands
    /// on, and two bare grounds off one connector stand a fine pitch apart
    /// rather than a column.
    fn across(&self, member: usize, ray: Side) -> f64 {
        let ax = Ax::of(ray).other();
        match self.bodies[member] {
            Body::Tag => self
                .lat
                .pitch
                .max(self.lat.pitches(spans(self.boxes[member], ax))),
            Body::Part => self.lat.step(ax),
        }
    }

    /// How deep a member's cell stands **along** its ray [SPEC 16.1]: what its
    /// own drawing reaches that way, and the one fine pitch two wired
    /// neighbours keep. Along the ray there is no readout to hold — a part
    /// wears its pair beside its body — so this is the symbol's own length,
    /// which is why a ground ends a chain a fine step under the part above it
    /// and two stacked discretes still stand a coarse pitch apart.
    fn along(&self, member: usize, ray: Side) -> f64 {
        self.lat
            .pitches(self.reaches(member, Ax::of(ray)) + self.lat.pitch)
    }

    /// How far a member's own drawing reaches on one axis.
    fn reaches(&self, member: usize, ax: Ax) -> f64 {
        spans(self.boxes[member], ax)
    }

    /// Where the member after `prev` stands along `ray`: half of each drawing,
    /// the one fine pitch of air any two of them keep, and out to the first
    /// fine line past that.
    fn after(&self, prev: (usize, f64), member: usize, ray: Side) -> f64 {
        let out = Ax::outward(ray);
        let ax = Ax::of(ray);
        let apart = (self.reaches(prev.0, ax) + self.reaches(member, ax)) / 2.0 + self.lat.pitch;
        self.lat.past(prev.1 + out * apart, out)
    }

    /// Where a run's **first** member stands, past whatever its lead had to
    /// clear: half its own drawing, and the same fine pitch of air.
    pub(super) fn past_ink(&self, member: usize, ray: Side, ink: f64) -> f64 {
        let out = Ax::outward(ray);
        let apart = self.reaches(member, Ax::of(ray)) / 2.0 + self.lat.pitch;
        self.lat.past(ink + out * apart, out)
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

    /// Where the **first** slot along `ray` stands [SPEC 16.1] — the one line
    /// the track row shares, so two anchors' fields keep one row.
    pub(super) fn origin(&self, anchor: usize, ray: Side, turned: bool) -> f64 {
        self.slots[anchor][ray.index()][usize::from(turned)]
    }
}

/// What a member's cell has to hold **across** its ray [SPEC 16.1] — the one
/// split SPEC 16.4 opens with: components have pins, and a label is its own
/// terminal.
#[derive(Clone, Copy)]
enum Body {
    /// A **part** — a component, a discrete, an amplifier. It wears a
    /// reference designator **beside** its body by rule, so across its ray it
    /// takes the coarse cell: that room is the pair's, and it is the one thing
    /// `gap` states.
    Part,
    /// A **label**: a terminal, not a part, so it asks only for the fine
    /// pitches its own drawing takes across the line it stands on — one line
    /// for a net run, which is the trace it names, and a symbol's own reach
    /// for a ground or a flag, which has no rule setting its name aside.
    Tag,
}

impl Body {
    fn of(node: &PlacedNode) -> Body {
        match sch_kind(&node.type_chain) {
            Some(SchKind::Label) => Body::Tag,
            _ => Body::Part,
        }
    }
}

/// How a run finds its line **across** its ray [SPEC 16.1]: the base its
/// innermost step stands on, and the side it steps out toward. One **fine**
/// line per step, whether the chain turned into a lane out past the anchor's
/// ink or grew straight out and kept its pin's own line — so the one allocator
/// walks either, and the cells alone say how far apart two runs land.
#[derive(Clone, Copy)]
struct Ladder {
    side: Side,
    base: f64,
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
/// How far a box spans on one axis.
fn spans(b: Bbox, ax: Ax) -> f64 {
    match ax {
        Ax::X => b.w(),
        Ax::Y => b.h(),
    }
}

/// How far a box reaches on one side, in its own frame.
pub(super) fn ink_edge(ink: &Bbox, side: Side) -> f64 {
    match side {
        Side::Left => ink.min_x,
        Side::Right => ink.max_x,
        Side::Top => ink.min_y,
        Side::Bottom => ink.max_y,
    }
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

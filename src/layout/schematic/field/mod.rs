//! The **field pass** [SPEC 16.1] — every satellite's cell, struck before a
//! single anchor is placed.
//!
//! A chain is a **walk**: it takes a *ray* (the direction it grows), a *lane*
//! (its line across that ray — a line out from the anchor's ink where it
//! turned off its pin, the pin's own fine line where it grew straight out)
//! and a *slot* per member (the fine line along the ray it stands on). A
//! satellite's cell is **its content's**, where it draws it ([`Drawing`]):
//! the fine bands its body reaches into from its seat point, and for a part
//! the rows its ref / value pair takes across the ray — the pair's width
//! alone never placing, that being the coarse cell `gap` states. So a cell
//! is asymmetric wherever the drawing is: a ground's cell hangs below its
//! connection point, a corridor part's pair stepped whole to one side leaves
//! the other side's row free.
//!
//! **Collision is the cells', and the wires' against the paint.** A lane is
//! free when no cell of the chain meets one already committed, and its wire
//! — the run down the lane to its last member — meets no ink a committed
//! member paints, its readouts included; a taken lane steps out a fine line
//! and tries again ([`allocate`]). That is the whole of it — an up-chain and
//! a down-chain off one pin share a lane because their cells are disjoint; a
//! second chain claiming a pin's straight corridor steps beside it because
//! they are not; no chain lands where a lead must cross, because the crossed
//! part is in the set; and a value overhanging its coarse cell pushes no
//! part, only the one wire that would otherwise run through it. A lead
//! reserves nothing; the **lane order** keeps it clear.
//!
//! The chains, the growth ray, the tap classifier and the limb split are all
//! [`crate::desugar::schematic::chain`]'s, shared with the pose chooser, so a
//! part is never posed for one ray and seated along another.

mod absolute;
mod read;
mod walk;

use super::lattice::{Ax, Lattice};
use super::place::Slot;
use super::readout::{self, Pair, Readout};
use super::terminal::{Terminal, seat_point, terminal};
use crate::desugar::pose::{Pose, Side};
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
pub(in crate::layout::schematic) struct Seat {
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
    /// Per anchor, the cells its field has committed — what keeps parts apart.
    cells: Vec<Vec<Bbox>>,
    /// Per anchor, the ink its seated members paint — what keeps wires off.
    paint: Vec<Vec<Bbox>>,
    /// Per anchor, where its field's **first** slot stands along each ray, by
    /// [`Side::index`] and then by whether the chain turned — the one line the
    /// track row shares, and the only one that is a rule rather than a search
    /// [SPEC 16.1].
    slots: Vec<[[f64; 2]; 4]>,
    /// Per child, what it draws in its seat's frame — what its cell holds.
    drawings: Vec<Drawing>,
    /// Per child, everything it draws, readouts included — what a field origin
    /// and a lane's base stand clear of.
    inks: Vec<Bbox>,
    /// Per child, the rows its own pins take on each side — what a corridor
    /// member's pair crowds against ([`readout::arrangement`]).
    rows: Vec<[Vec<f64>; 4]>,
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
            paint: vec![Vec::new(); children.len()],
            slots: vec![[[0.0; 2]; 4]; children.len()],
            drawings: children.iter().map(Drawing::of).collect(),
            inks: children.iter().map(drawn).collect(),
            rows: children
                .iter()
                .map(|c| Side::ALL.map(|side| readout::pin_rows(c, side)))
                .collect(),
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
    /// origin**, as a distance to the outermost cell **edge** — what the sheet
    /// really draws there, since a cell is its content's [SPEC 16.1]: a column
    /// of parts reaches half a coarse cell past its lane, a lone ground the few
    /// fine pitches its symbol takes. `0.0` where no chain went that way.
    ///
    /// A distance and no longer a count of cells: a lane is a coarse line and
    /// a slot a fine one, so the two no longer share a unit.
    pub(in crate::layout::schematic) fn extent(&self, anchor: usize, side: Side) -> f64 {
        self.reach(anchor, side)
    }

    /// The first coarse line out on `side` whose whole cell an anchor's field
    /// leaves **free**, as a distance from the anchor's own origin [SPEC 16.1]
    /// — where the next part may stand, and with nothing there, the one clear
    /// column two neighbouring anchors keep. Their own ink is the packer's to
    /// read; this answers only what the field holds.
    pub(in crate::layout::schematic) fn free(&self, anchor: usize, side: Side) -> f64 {
        self.reach(anchor, side) + self.lat.step(Ax::of(side)) / 2.0
    }

    /// Satellites no wire held — the flow fallback [SPEC 16.1].
    pub(super) fn floating(&self) -> &[usize] {
        &self.floating
    }

    /// The chains held at two anchors [SPEC 16.1], which ride no field.
    pub(super) fn spans(&self) -> &[Spanning] {
        &self.spans
    }

    /// Seat one member, and commit its cell and its paint to its anchor's
    /// occupancy.
    fn take(&mut self, member: usize, seat: Seat) {
        let (cell, paint) = self.holds(member, seat);
        self.cells[seat.anchor].push(cell);
        self.paint[seat.anchor].push(paint);
        self.seats[member] = Some(seat);
    }

    /// Where a run's cross ladder stands at step `k` [SPEC 16.1] — one **fine**
    /// line out per step from its own base, so what the cells need decides the
    /// spacing rather than a pitch stated in advance.
    fn ladder_at(&self, ladder: Ladder, k: i32) -> f64 {
        ladder.base + f64::from(k - 1) * self.lat.pitch * Ax::outward(ladder.side)
    }

    /// The seat one step `across` from this one — where a tap hangs, and where
    /// a branch crossing the trunk marches: past the edge of what stands here,
    /// by what the member's own cell reaches back, and out to the first fine
    /// line past that.
    fn stepped(&self, at: (usize, Seat), member: usize, across: Side) -> Seat {
        let out = Ax::outward(across);
        let edge = ink_edge(&self.cell(at.0, at.1), across);
        let back = self.spread(member, at.1, across.opposite());
        Seat {
            cross: self.lat.past(edge + out * back, out),
            ..at.1
        }
    }

    /// Where a seat's point is, in its anchor's own frame.
    fn point(&self, seat: Seat) -> (f64, f64) {
        match Ax::of(seat.ray) {
            Ax::X => (seat.along, seat.cross),
            Ax::Y => (seat.cross, seat.along),
        }
    }

    /// A member's **cell** at `seat` [SPEC 16.1].
    fn cell(&self, member: usize, seat: Seat) -> Bbox {
        self.holds(member, seat).0
    }

    /// What a member at `seat` holds [SPEC 16.1], in its anchor's frame: its
    /// **cell** — the fine bands its own drawing reaches into from its seat
    /// point: its body, and for a part the rows its readout pair takes
    /// **across** the ray where the pair stands ([`readout::arrangement`])
    /// and the pitch of air its length keeps along it; a label takes only
    /// what it draws, either way — and its **paint**, the ink it really puts
    /// down there, readouts included, which no wire may run through.
    fn holds(&self, member: usize, seat: Seat) -> (Bbox, Bbox) {
        let d = &self.drawings[member];
        let ax = Ax::of(seat.ray);
        let mut ink = d.body;
        let mut paint = d.body;
        if let Some(kind) = d.kind.filter(|&k| k != SchKind::Label) {
            let pair = readout::arrangement(
                kind,
                d.pose,
                Some(seat),
                &self.rows[seat.anchor][seat.side.index()],
            );
            for r in &d.readouts {
                let (cx, cy) = pair.map_or((r.cx, r.cy), |p| readout::seated(p, d.axis, r));
                let mut b = r.bbox.shifted(cx - d.seat.0, cy - d.seat.1);
                paint = paint.union(b);
                // Text width never places [SPEC 16.1]: a pair standing beside
                // a part across its ray takes the coarse cell, centred on the
                // part's own axis — the room `gap` states, and the one rhythm
                // a column of parts keeps whatever their values' widths.
                if let (Some(Pair::Beside(_)), Ax::Y) = (pair, ax) {
                    let (axis, half) = (d.axis - d.seat.0, self.lat.step(Ax::X) / 2.0);
                    (b.min_x, b.max_x) = (axis - half, axis + half);
                }
                ink = union_across(ink, b, ax);
            }
            ink = extend_along(ink, ax, self.lat.pitch / 2.0);
        }
        let (x0, x1) = self.lat.bands(ink.min_x, ink.max_x);
        let (y0, y1) = self.lat.bands(ink.min_y, ink.max_y);
        let (x, y) = self.point(seat);
        let cell = Bbox {
            min_x: x0 + x,
            min_y: y0 + y,
            max_x: x1 + x,
            max_y: y1 + y,
        };
        (cell, paint.shifted(x, y))
    }

    /// How far a member's cell at `seat` reaches from its seat point toward
    /// `side` — `0` where the whole cell lies the other way.
    fn spread(&self, member: usize, seat: Seat, side: Side) -> f64 {
        let (x, y) = self.point(seat);
        let cell = self.cell(member, seat);
        let at = match Ax::of(side) {
            Ax::X => x,
            Ax::Y => y,
        };
        ((ink_edge(&cell, side) - at) * Ax::outward(side)).max(0.0)
    }

    /// How far a member's own **body** reaches from its seat point toward
    /// `side` — what the stepping along a ray reads, a pair being beside a
    /// part by rule and never in its way.
    fn reaches(&self, member: usize, side: Side) -> f64 {
        (ink_edge(&self.drawings[member].body, side) * Ax::outward(side)).max(0.0)
    }

    /// Where the member after `prev` stands along `ray`: past what the one
    /// before reaches, the one fine pitch of air any two of them keep, what
    /// this one reaches back, and out to the first fine line past that.
    fn after(&self, prev: (usize, f64), member: usize, ray: Side) -> f64 {
        let out = Ax::outward(ray);
        let apart =
            self.reaches(prev.0, ray) + self.lat.pitch + self.reaches(member, ray.opposite());
        self.lat.past(prev.1 + out * apart, out)
    }

    /// Where a run's **first** member stands, past whatever its lead had to
    /// clear: what it reaches back toward that, and the same fine pitch of air.
    pub(super) fn past_ink(&self, member: usize, ray: Side, ink: f64) -> f64 {
        let out = Ax::outward(ray);
        let apart = self.reaches(member, ray.opposite()) + self.lat.pitch;
        self.lat.past(ink + out * apart, out)
    }

    /// How far out on `side` an anchor's field reaches, as a distance from the
    /// anchor's own origin to the outermost cell edge.
    fn reach(&self, anchor: usize, side: Side) -> f64 {
        self.cells[anchor]
            .iter()
            .map(|c| ink_edge(c, side) * Ax::outward(side))
            .fold(0.0f64, f64::max)
    }

    /// Where the **first** slot along `ray` stands [SPEC 16.1] — the one line
    /// the track row shares, so two anchors' fields keep one row.
    pub(super) fn origin(&self, anchor: usize, ray: Side, turned: bool) -> f64 {
        self.slots[anchor][ray.index()][usize::from(turned)]
    }
}

/// What one child draws, in its **seat's** frame [SPEC 16.1] — the seat point
/// being the centre of its connection geometry ([`seat_point`]), which is the
/// point the lattice holds it by, so a flag's name and a ground's symbol both
/// hang off it the way the sheet draws them.
struct Drawing {
    kind: Option<SchKind>,
    pose: Pose,
    /// The body's box, relative to the seat point.
    body: Bbox,
    /// The seat point, in the child's own frame.
    seat: (f64, f64),
    /// The child's own centre line — the axis a beside pair stands off.
    axis: f64,
    /// A part's ref and value readouts, as desugar minted them.
    readouts: Vec<Readout>,
}

impl Drawing {
    fn of(node: &PlacedNode) -> Drawing {
        let seat = seat_point(node);
        Drawing {
            kind: sch_kind(&node.type_chain),
            pose: Pose::of_chain(&node.type_chain),
            body: node.bbox.shifted(-seat.0, -seat.1),
            seat,
            axis: node.bbox.center().0,
            readouts: node.children.iter().filter_map(Readout::of).collect(),
        }
    }
}

/// What a chain asks for at one lane step: the cells its members and stem
/// take, and the wires it draws.
#[derive(Default)]
pub(super) struct Claim {
    pub cells: Vec<Bbox>,
    pub wires: Vec<Bbox>,
}

/// `ink` widened to hold `b` **across** `ax` only — the readouts' rows, never
/// their reach along the ray.
fn union_across(ink: Bbox, b: Bbox, ax: Ax) -> Bbox {
    match ax {
        Ax::X => Bbox {
            min_y: ink.min_y.min(b.min_y),
            max_y: ink.max_y.max(b.max_y),
            ..ink
        },
        Ax::Y => Bbox {
            min_x: ink.min_x.min(b.min_x),
            max_x: ink.max_x.max(b.max_x),
            ..ink
        },
    }
}

/// `ink` grown by `air` either end **along** `ax`.
fn extend_along(ink: Bbox, ax: Ax, air: f64) -> Bbox {
    match ax {
        Ax::X => ink.expand(0.0, air, 0.0, air),
        Ax::Y => ink.expand(air, 0.0, air, 0.0),
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

/// The innermost step of a [`Ladder`] whose cells meet no cell already
/// committed, and whose wires meet no paint [SPEC 16.1] — a lane for a chain
/// that turned off its pin, the pin's own line for one that grew straight out.
///
/// The lead reserves nothing: the lane order — the pin deeper along the ray
/// keeping the inner lane — already crosses a column only above where that
/// column is live. Overlap is [`Bbox::overlaps`]'s, which is strict, so
/// adjacent cells are legal neighbours rather than a lane marching outward
/// for ever, and a wire is one pitch wide, so ink half a pitch off its line
/// is what stops it.
fn allocate(cells: &[Bbox], paint: &[Bbox], claim: impl Fn(i32) -> Claim) -> i32 {
    let clear = |taken: &[Bbox], asked: &[Bbox]| {
        asked.iter().all(|c| !taken.iter().any(|t| t.overlaps(*c)))
    };
    (1..)
        .find(|&k| {
            let asked = claim(k);
            clear(cells, &asked.cells) && clear(paint, &asked.wires)
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

    /// The same column asked for as a claim with no wires.
    fn cells(cells: Vec<Bbox>) -> Claim {
        Claim {
            cells,
            wires: Vec::new(),
        }
    }

    #[test]
    fn the_innermost_free_lane_wins() {
        assert_eq!(
            allocate(&[], &[], |k| cells(column(k, 2))),
            1,
            "an empty field takes lane 1"
        );
    }

    #[test]
    fn an_occupied_lane_steps_out_one_and_retries() {
        let taken = column(1, 2);
        assert_eq!(allocate(&taken, &[], |k| cells(column(k, 2))), 2);
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
        assert_eq!(
            allocate(&down, &[], |k| cells(up(k))),
            1,
            "one lane, two rays"
        );
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
        assert_eq!(
            allocate(&taken, &[], |k| cells(deep(k))),
            1,
            "below what lane 1 holds"
        );
    }

    #[test]
    fn a_wire_steps_off_paint_its_cells_would_have_cleared() {
        // [SPEC 16.1] a value overhanging its coarse cell pushes no cell, but
        // the wire of the next lane may not run through it: lane 1's cells
        // are clear, its wire is not, so lane 2 it is.
        let paint = [Bbox::centered(70.0, 20.0).shifted(-60.0, 100.0)];
        let wire = |k: i32| Claim {
            cells: column(k, 1),
            wires: vec![Bbox::centered(20.0, 200.0).shifted(-100.0 * k as f64, 100.0)],
        };
        assert_eq!(allocate(&[], &paint, wire), 2);
    }
}

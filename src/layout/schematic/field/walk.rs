//! **A chain is a walk** [SPEC 16.1] — the half of the field pass that turns
//! one held chain into cells.
//!
//! Its trunk takes a ray and a lane and steps a slot per member; a **tap**
//! stands on its attachment's own slot, one cell across; a multi-member
//! **branch** grows from its junction as a sub-chain, along its own
//! terminator's ray. Every one of those readings — the ray, the tap
//! classifier, the limb split, the aside step — is
//! [`crate::desugar::schematic::chain`]'s, shared with the pose chooser.

use super::super::lattice::{Ax, EPS};
use super::super::place::Slot;
use super::super::terminal::{Terminal, terminal};
use super::read::{growth, tag_facing, tap_flags};
use super::{Field, LANED, Ladder, STRAIGHT, Seat, allocate, clear};
use crate::desugar::pose::Side;
use crate::desugar::schematic::chain::{Chain, End, beside, limbs, tap_ray};
use crate::layout::geom::dot;
use crate::layout::ir::{Bbox, PlacedNode};

impl Field {
    /// Grow every chain one anchor holds, in the lane order.
    pub(super) fn walk(
        &mut self,
        children: &[PlacedNode],
        wires: &[[End; 2]],
        tracks: &[Option<Slot>],
        chains: Vec<(Chain, End)>,
    ) {
        // One lane ladder per **side** of an anchor, in the order the chains
        // arrive: two sides can never cross each other's leads, so they never
        // compete for one line.
        let mut ladders: Vec<(usize, Side)> = Vec::new();
        let mut held: Vec<Held> = chains
            .into_iter()
            .map(|(chain, pin)| Held::of(children, chain, pin, wires, &mut ladders))
            .collect();
        order(&mut held, ladders.len());
        self.strike_origins(tracks, &held);
        for h in &held {
            self.grow(children, h);
        }
    }

    /// Seat one held chain [SPEC 16.1]: its trunk on a lane of its own, each
    /// tap beside the member it hangs off, and every multi-member branch as a
    /// sub-chain from its junction — all against the one occupancy.
    fn grow(&mut self, children: &[PlacedNode], h: &Held) {
        let chain = &h.chain;
        let tap = tap_flags(children, chain);
        let limbs = limbs(chain);
        let ladder = self.ladder(h);
        let turned = h.turns();
        let trunk = Run {
            ray: h.ray,
            ladder,
            first: self.slot_line(h.anchor, h.ray, turned, 1),
            members: pick(chain, |i| limbs[i].is_none()),
            origin: Some(h.pin.at),
        };
        // One allocation for either ladder [SPEC 16.1]: a chain that turned off
        // its pin takes the innermost free lane, and one that grew straight out
        // asks for its pin's own line — stepping beside it only where a chain
        // already claimed that corridor, which is the first claimant's.
        let k = self.allot(h, &trunk);
        self.commit(h, &trunk, k);

        // A **tap** takes no slot [SPEC 16.1]: it stands on its attachment's,
        // one coarse cell across, the way its own drawing points.
        for (i, &member) in chain.members.iter().enumerate() {
            if !tap[i] {
                continue;
            }
            let Some(attach) = self.attachment(chain, i) else {
                continue;
            };
            let facing = terminal(&children[member], chain.inbound[i].as_deref()).facing;
            let aside = tap_ray(facing, h.ray, h.pin.facing);
            // A convention running **with** the trunk would stand the tap on
            // the trunk's own next slot; it steps beside instead, which is
            // where a sheet draws a flag off the junction it taps.
            let across = if aside == h.ray {
                beside(h.ray, h.pin.facing)
            } else {
                aside
            };
            self.take(member, self.stepped(attach, across));
        }

        // A multi-member **branch** grows from its junction as a sub-chain
        // along its own terminator's ray [SPEC 16.1].
        for r in 0..chain.members.len() {
            if limbs[r] != Some(r) || tap[r] {
                continue;
            }
            let Some(attach) = self.attachment(chain, r) else {
                continue;
            };
            let limb: Vec<usize> = (0..chain.members.len())
                .filter(|&i| limbs[i] == Some(r))
                .collect();
            let &last = limb.last().expect("a branch holds its root");
            let ray = tag_facing(
                &children[chain.members[last]],
                chain.inbound[last].as_deref(),
            )
            .map_or(h.ray, Side::opposite);
            let members: Vec<usize> = limb.iter().map(|&i| chain.members[i]).collect();
            if Ax::of(ray) != Ax::of(h.ray) {
                // Across the trunk: the members march out from the junction
                // on its own slot, a coarse cell each, exactly as a tap does.
                let mut seat = attach;
                for &m in &members {
                    seat = self.stepped(seat, ray);
                    self.take(m, seat);
                }
                continue;
            }
            // On the trunk's own axis: a lane of its own — the occupancy
            // carries it sideways until it stands clear — with its slots
            // carrying on from the junction.
            let branch = Run {
                ray,
                ladder,
                first: attach.slot + self.stride(ray),
                members,
                origin: None,
            };
            let k = self.allot(h, &branch);
            self.commit(h, &branch, k);
        }
    }

    /// Strike every **slot origin** [SPEC 16.1], before a chain grows.
    ///
    /// A slot clears what its own chain's lead actually passes. A chain that
    /// **turned** into a lane is beside the anchor's body already — that is
    /// what the lane is — so its slots clear only the deepest pin the ray's
    /// laned chains leave from. One that grew **straight** out has its ray
    /// pointing through the body, and clears the anchor's whole ink. Both by
    /// half a coarse cell, a member standing centred on its line, and both
    /// onto the first **fine** line beyond: the separation is ink's, and a
    /// coarse line would round up to a whole cell of bare wire.
    ///
    /// And the origin belongs to the **track line**, not the anchor: every
    /// anchor riding the line across the ray — the same row for an up or down
    /// ray, the same column for a left or right one — takes the deepest
    /// requirement among them, which is what stands two anchors' fields on one
    /// row.
    fn strike_origins(&mut self, tracks: &[Option<Slot>], held: &[Held]) {
        for ray in Side::ALL {
            let (ax, out) = (Ax::of(ray), Ax::outward(ray));
            let deeper = |a: i32, b: i32| {
                if f64::from(b) * out > f64::from(a) * out {
                    b
                } else {
                    a
                }
            };
            // A track line and a class; the line seeded from the anchor's own
            // ink, which is exactly the straight class's own measure.
            let mut want: Vec<(usize, usize, i32)> = Vec::new();
            for (i, track) in tracks.iter().enumerate() {
                let Some(track) = track else { continue };
                want.push((track.on(ax), STRAIGHT, self.slots[i][ray.index()][STRAIGHT]));
            }
            for h in held.iter().filter(|h| h.ray == ray && h.turns()) {
                let Some(track) = tracks[h.anchor] else {
                    continue;
                };
                let line = self
                    .lat
                    .fine_beyond(h.depth * out + clear(ray, self.lat), out);
                want.push((track.on(ax), LANED, line));
            }
            for (i, track) in tracks.iter().enumerate() {
                let Some(track) = track else { continue };
                for class in [STRAIGHT, LANED] {
                    let asked = want
                        .iter()
                        .filter(|w| w.0 == track.on(ax) && w.1 == class)
                        .map(|w| w.2)
                        .reduce(deeper);
                    if let Some(line) = asked {
                        self.slots[i][ray.index()][class] = line;
                    }
                }
            }
        }
    }

    /// The innermost cross step a run's cells leave free [SPEC 16.1].
    fn allot(&self, h: &Held, run: &Run) -> i32 {
        allocate(&self.cells[h.anchor], |k| self.claim(h, run, k))
    }

    /// Record a run's seats and commit its cells to the anchor's occupancy.
    fn commit(&mut self, h: &Held, run: &Run, k: i32) {
        let seats: Vec<(usize, Seat)> = self.seats_of(h, run, k).collect();
        let stem = self.stem(h, run, k);
        for (member, seat) in seats {
            self.take(member, seat);
        }
        if let Some(&last) = run.members.last() {
            self.terminators[last] = true;
        }
        self.cells[h.anchor].extend(stem);
    }

    /// Everything a run at cross step `k` would occupy: its members' cells,
    /// and the **stem** below.
    fn claim(&self, h: &Held, run: &Run, k: i32) -> Vec<Bbox> {
        self.seats_of(h, run, k)
            .map(|(m, s)| self.cell(m, s))
            .chain(self.stem(h, run, k))
            .collect()
    }

    /// The column a trunk actually draws, from its own pin's line out to its
    /// first cell [SPEC 16.1] — a chain's cells begin at its **pin**, not at
    /// the field origin.
    ///
    /// Two chains off *different* pins of one side whose rays point at each
    /// other both cross the band between those pins, and only the stem says
    /// so; without it their member cells are disjoint and they share a lane,
    /// braiding two nets into one column. A pin's **own** up/down pair still
    /// shares, because their stems meet at exactly one line and overlap is
    /// strict — the sharing stays a consequence of the cells rather than a
    /// rule beside them. The *lead* — the run out to the lane — still claims
    /// nothing; this is the chain's own column.
    fn stem(&self, h: &Held, run: &Run, k: i32) -> Option<Bbox> {
        let (px, py) = run.origin?;
        let (member, seat) = self.seats_of(h, run, k).next()?;
        let cross = self.cross(seat);
        let pin = match Ax::of(run.ray) {
            Ax::X => (px, cross),
            Ax::Y => (cross, py),
        };
        Some(self.cell(member, seat).union(Bbox::from_points(&[pin])))
    }

    /// The seats a run's members take with its cross ladder at step `k`, each
    /// beside the member that takes it.
    fn seats_of<'a>(
        &'a self,
        h: &'a Held,
        run: &'a Run,
        k: i32,
    ) -> impl Iterator<Item = (usize, Seat)> + 'a {
        let (lane, pin_line) = self.ladder_at(h.anchor, run.ladder, k);
        run.members.iter().enumerate().map(move |(j, &member)| {
            (
                member,
                Seat {
                    anchor: h.anchor,
                    ray: run.ray,
                    side: h.side,
                    lane,
                    pin_line,
                    slot: run.first + j as i32 * self.stride(run.ray),
                },
            )
        })
    }

    /// The seat of the member `i` hangs off — its attachment up the walk.
    /// `None` while that member has none, which leaves `i` unseated rather
    /// than guessing a junction.
    fn attachment(&self, chain: &Chain, i: usize) -> Option<Seat> {
        self.seats[chain.members[chain.parents[i]?]]
    }

    /// The cross ladder a chain's trunk — and every branch sharing its axis —
    /// steps on: the lanes of the side it left by where it turned off its
    /// pin, else fine lines **beside** the pin line it kept.
    fn ladder(&self, h: &Held) -> Ladder {
        if h.turns() {
            Ladder::Lanes(h.side)
        } else {
            Ladder::Beside(beside(h.ray, h.pin.facing), across(h.pin.at, h.ray))
        }
    }
}

/// A chain one anchor holds, with everything the lane order and the walk read
/// off it — struck once, because [`growth`] answers the ray and the pin
/// together and neither may be asked twice.
struct Held {
    chain: Chain,
    anchor: usize,
    pin: Terminal,
    /// The side its lead leaves by — the pin's own normal, and the fallback
    /// the growth ray itself assumes for a terminal with no facing at all.
    side: Side,
    ray: Side,
    /// Which (anchor, side) ladder it competes in.
    group: usize,
    /// How far along the ray its pin already sits — the lane order's key —
    /// and the same depth read along the canonical direction of that axis.
    depth: f64,
    canon: f64,
}

impl Held {
    fn of(
        children: &[PlacedNode],
        chain: Chain,
        pin_end: End,
        wires: &[[End; 2]],
        ladders: &mut Vec<(usize, Side)>,
    ) -> Held {
        let (ray, pin) = growth(children, &chain, &pin_end, wires);
        let side = pin.facing.unwrap_or(Side::Bottom);
        let key = (pin_end.child, side);
        let group = ladders.iter().position(|k| *k == key).unwrap_or_else(|| {
            ladders.push(key);
            ladders.len() - 1
        });
        Held {
            anchor: pin_end.child,
            depth: dot(pin.at, ray.normal()),
            canon: dot(pin.at, canonical(ray).normal()),
            chain,
            pin,
            side,
            ray,
            group,
        }
    }

    /// Whether the chain **turned** off its pin, and so takes a lane. One
    /// that grew straight out keeps the pin's own fine line and competes for
    /// nothing.
    fn turns(&self) -> bool {
        self.ray != self.side
    }
}

/// One run of members growing one way from one point — a chain's trunk from
/// its pin, or a branch from its junction.
struct Run {
    ray: Side,
    ladder: Ladder,
    /// The fine line its first member stands on along the ray.
    first: i32,
    members: Vec<usize>,
    /// Where the run's own column starts, for a trunk: its pin, in the
    /// anchor's frame ([`Field::stem`]). A branch starts at a junction whose
    /// cell is already committed, so it needs none.
    origin: Option<(f64, f64)>,
}

/// The **allocation order** [SPEC 16.1]. Chains that grew straight out along
/// their pins take no lane and compete for none, so they commit first: they
/// are the inner geography every lane then steps past. The rest go deepest
/// pin first, read along the ray — so a lead crosses an inner column only
/// above where that column is live — except on a side carrying **both** rays,
/// which cannot read depth two ways and falls back to the canonical direction,
/// the deepest pin innermost either way. A stable sort, so the chains' own
/// statement order breaks every tie.
///
/// Depth compares in whole [`EPS`] steps, never bit for bit: the pins of one
/// rail stand at one depth, but the stub tips that measure it carry that depth
/// to a bit or two and not to the bit — a pin whose name is wider than its
/// neighbour's shifts the last of them. Quantised, one depth reads as one
/// depth and the statement order decides, which is what the tie-break says;
/// raw, a name's width silently outranks it.
fn order(held: &mut [Held], ladders: usize) {
    let mut ray_of: Vec<Option<Side>> = vec![None; ladders];
    let mut mixed = vec![false; ladders];
    for h in held.iter().filter(|h| h.turns()) {
        match ray_of[h.group] {
            None => ray_of[h.group] = Some(h.ray),
            Some(r) => mixed[h.group] |= r != h.ray,
        }
    }
    let rung = |v: f64| (v / EPS).round() as i64;
    held.sort_by(|a, b| {
        a.turns()
            .cmp(&b.turns())
            .then(a.group.cmp(&b.group))
            .then_with(|| {
                let (x, y) = if mixed[a.group] {
                    (a.canon, b.canon)
                } else {
                    (a.depth, b.depth)
                };
                rung(y).cmp(&rung(x))
            })
    });
}

/// The canonical direction of a ray's own axis — down, or right.
fn canonical(ray: Side) -> Side {
    if ray.is_vertical() {
        Side::Right
    } else {
        Side::Bottom
    }
}

/// A point's coordinate **across** `ray` — the line a chain growing that way
/// stands on.
fn across(at: (f64, f64), ray: Side) -> f64 {
    match Ax::of(ray) {
        Ax::X => at.1,
        Ax::Y => at.0,
    }
}

/// The child indices of the members a chain's `keep` picks out.
fn pick(chain: &Chain, keep: impl Fn(usize) -> bool) -> Vec<usize> {
    (0..chain.members.len())
        .filter(|&i| keep(i))
        .map(|i| chain.members[i])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::super::lattice::Lattice;
    use super::super::super::place::role;
    use super::*;
    use crate::desugar::schematic::Role;
    use crate::span::Span;

    /// A root schematic scope holding `body` behind `defs` — the root, so the
    /// scope path is `""` and the links are the scene's own.
    fn sheet(defs: &str, body: &str) -> String {
        format!(
            "{{ layout: schematic;\n{defs}}}\n|component#u1| [\n  |pin#a| {{ side: left }}\n  \
             |pin#b| {{ side: left }}\n  |pin#c| {{ side: bottom }}\n  \
             |pin#d| {{ side: right }}\n]\n{body}"
        )
    }

    /// A user-defined power flag, for the chains that grow **up**.
    const FLAG: &str = "  |vp::label| { symbol: power } [ \"V+\" ]\n";

    /// The field a source lays out. The children come from the finished
    /// layout, which changes nothing this pass reads: a terminal and a part's
    /// ink are both in the part's own frame.
    fn field(src: &str) -> (Vec<PlacedNode>, Field) {
        let program = crate::testutil::program(src);
        let children = crate::testutil::laid(src).nodes;
        let roles: Vec<Role> = children.iter().map(role).collect();
        let links = crate::layout::scope_links(&program, "", None);
        let lat = Lattice::of(&program.scene.attrs, Span::empty()).expect("a lattice");
        let field = Field::build(
            &children,
            &roles,
            &links,
            "",
            &tracks(&children, &roles),
            lat,
        );
        (children, field)
    }

    /// Every anchor's ordinal slot, as [`crate::layout::schematic::place`]
    /// strikes it before the field pass runs.
    fn tracks(children: &[PlacedNode], roles: &[Role]) -> Vec<Option<Slot>> {
        let anchored: Vec<usize> = (0..children.len())
            .filter(|&i| roles[i] == Role::Anchor)
            .collect();
        let slots = crate::layout::schematic::place::slots(children, &anchored, None)
            .expect("the sample's ordinals");
        let mut out = vec![None; children.len()];
        for (&i, &s) in anchored.iter().zip(&slots) {
            out[i] = Some(s);
        }
        out
    }

    /// One coarse slot, in the fine lines a [`Seat::slot`] counts: the default
    /// `gap` of 100 over the pin pitch of 20.
    const SLOT: i32 = 5;

    /// A seat's coordinate along its ray, in the anchor's own frame.
    fn slot_at(seat: Seat) -> f64 {
        f64::from(seat.slot) * crate::ledger::consts::PIN_PITCH
    }

    /// One child's drawn ink, in its own frame.
    fn ink(children: &[PlacedNode], id: &str) -> crate::layout::ir::Bbox {
        let i = children
            .iter()
            .position(|c| c.id.as_deref() == Some(id))
            .unwrap_or_else(|| panic!("no child '{id}'"));
        super::super::drawn(&children[i])
    }

    fn seat(children: &[PlacedNode], field: &Field, id: &str) -> Seat {
        let i = children
            .iter()
            .position(|c| c.id.as_deref() == Some(id))
            .unwrap_or_else(|| panic!("no child '{id}'"));
        field.seat(i).unwrap_or_else(|| panic!("'{id}' unseated"))
    }

    #[test]
    fn a_chain_growing_straight_out_takes_no_lane() {
        // [SPEC 16.1] the ground off a bottom pin runs down the pin's own
        // line, so it keeps that fine line and competes for no lane.
        let (kids, f) = field(&sheet("", "|gnd#g1|\nu1.c - g1\n"));
        let g = seat(&kids, &f, "g1");
        assert_eq!((g.ray, g.side, g.lane), (Side::Bottom, Side::Bottom, None));
        // …and its ray points through the body, so its slots clear the whole
        // ink [SPEC 16.1] — half a cell for the member's own width, and the
        // fine pitch two wired neighbours keep.
        let clear = ink(&kids, "u1").max_y + 50.0 + 20.0;
        assert!(
            slot_at(g) > clear && slot_at(g) <= clear + 20.0,
            "the first fine line past {clear}, not {}",
            slot_at(g)
        );
    }

    #[test]
    fn a_chain_that_turns_takes_the_innermost_lane_and_slots_from_the_origin() {
        // Off a left pin the ground still grows down, so the chain turns and
        // takes lane 1; its members step one coarse slot at a time.
        let (kids, f) = field(&sheet("", "|R#r1| \"1k\"\n|gnd#g1|\nu1.a - r1 - g1\n"));
        let (r, g) = (seat(&kids, &f, "r1"), seat(&kids, &f, "g1"));
        assert_eq!((r.ray, r.side, r.lane), (Side::Bottom, Side::Left, Some(1)));
        assert_eq!(g.lane, Some(1), "one lane for the chain");
        assert_eq!(g.slot, r.slot + SLOT, "link by link, a slot each");
        // The lane already cleared the body, so the slots clear the **pin**
        // and the first member stands beside the part, not below it.
        assert!(
            slot_at(r) < ink(&kids, "u1").max_y,
            "seated beside the anchor, at {}",
            slot_at(r)
        );
    }

    #[test]
    fn the_deeper_pin_along_the_ray_keeps_the_inner_lane() {
        // [SPEC 16.1] the shallower pin's chain steps out, so its lead
        // crosses the inner column only above where that column is live.
        // `b` is the lower of the two left pins, whatever the wire order.
        let (kids, f) = field(&sheet("", "|gnd#ga|\n|gnd#gb|\nu1.a - ga\nu1.b - gb\n"));
        assert_eq!(seat(&kids, &f, "gb").lane, Some(1), "the deeper pin's");
        assert_eq!(
            seat(&kids, &f, "ga").lane,
            Some(2),
            "and the shallower steps"
        );
    }

    #[test]
    fn two_pins_at_one_depth_take_their_ladders_in_statement_order() {
        // [SPEC 16.1] both chains grow straight out of one rail, so both pins
        // stand at the same depth along the ray and statement order is the
        // whole tie-break. That depth is measured on stub tips, which agree to
        // a bit or two and not to the bit once the pins' names differ in
        // width — so the order is read in fine lines, never in raw px.
        let src = "{ layout: schematic }\n|component#u1| [\n  \
                   |pin#NRE| \"RE\" { side: left }\n  \
                   |pin#DE| { side: left }\n  \
                   |pin#VCC| { side: right }\n]\n\
                   |R#r1| \"1k\"\n|R#r2| \"2k\"\nu1.DE - r1\nu1.NRE - r2\n";
        let (kids, f) = field(src);
        let u1 = kids
            .iter()
            .position(|c| c.id.as_deref() == Some("u1"))
            .expect("u1");
        let line = |pin| terminal(&kids[u1], Some(pin)).at.1;
        let (r1, r2) = (seat(&kids, &f, "r1"), seat(&kids, &f, "r2"));
        assert_eq!((r1.lane, r2.lane), (None, None), "both grew straight out");
        assert_eq!(r1.pin_line, line("DE"), "the first stated keeps its line");
        assert_ne!(r2.pin_line, line("NRE"), "and the second steps beside it");
    }

    #[test]
    fn an_up_chain_and_a_down_chain_off_one_pin_share_a_lane() {
        // Their cells are disjoint, so the second one's innermost candidate
        // is already free — a consequence of the occupancy test, not a rule.
        let src = sheet(FLAG, "|gnd#g1|\n|vp#f1|\nu1.a - g1\nu1.a - f1\n");
        let (kids, f) = field(&src);
        let (g, v) = (seat(&kids, &f, "g1"), seat(&kids, &f, "f1"));
        assert_eq!((g.lane, v.lane), (Some(1), Some(1)), "one lane, two rays");
        assert_eq!((g.ray, v.ray), (Side::Bottom, Side::Top), "opposite ways");
    }

    #[test]
    fn a_tap_stands_on_its_attachments_slot_one_cell_across() {
        // [SPEC 16.1] a symbol-label leaf hanging mid-chain takes no slot: it
        // steps beside the member it hangs off rather than into the trunk.
        let src = sheet(
            FLAG,
            "|L#l1| \"100u\"\n|vp#f1|\n|R#r1| \"4k7\"\n|gnd#g1|\n\
             u1.a - l1 - f1\nl1.p2 - r1 - g1\n",
        );
        let (kids, f) = field(&src);
        let (l, t, r) = (
            seat(&kids, &f, "l1"),
            seat(&kids, &f, "f1"),
            seat(&kids, &f, "r1"),
        );
        assert_eq!(t.slot, l.slot, "the flag keeps its attachment's slot");
        assert_eq!(t.lane, l.lane.map(|k| k + 1), "…one lane outward of it");
        assert_eq!(r.slot, l.slot + SLOT, "and the trunk grows on past it");

        // The same chain off a **bottom** pin, where the trunk keeps its
        // pin's own fine line and there is no lane to step: the flag steps
        // that line instead, by one coarse cell.
        let (kids, f) = field(&src.replace("u1.a - l1", "u1.c - l1"));
        let (l, t) = (seat(&kids, &f, "l1"), seat(&kids, &f, "f1"));
        assert_eq!(
            (l.lane, t.lane),
            (None, None),
            "a laneless trunk, and its tap"
        );
        assert_eq!(t.slot, l.slot, "still on its attachment's slot");
        assert!(
            t.pin_line > l.pin_line,
            "stepped across: {} vs {}",
            t.pin_line,
            l.pin_line
        );
    }

    #[test]
    fn a_multi_member_branch_grows_its_own_lane_on_from_its_junction() {
        let src = sheet(
            "",
            "|R#r1| \"10k\"\n|R#r2| \"20k\"\n|C#c1| \"100n\"\n\
             u1.a - r1 - r2 - |gnd|\nr1.p2 - c1 - |gnd|\n",
        );
        let (kids, f) = field(&src);
        let (r1, r2, c1) = (
            seat(&kids, &f, "r1"),
            seat(&kids, &f, "r2"),
            seat(&kids, &f, "c1"),
        );
        assert_eq!(c1.lane, r1.lane, "the trunk keeps one lane");
        assert_eq!(r2.lane, Some(2), "the branch steps out of it");
        assert_eq!(
            (r2.slot, c1.slot),
            (r1.slot + SLOT, r1.slot + SLOT),
            "both from the junction"
        );
    }

    #[test]
    fn a_bridge_grows_off_its_first_named_pin_and_a_span_holds_no_seat() {
        // [SPEC 16.1] two ends on **one** anchor is a fan, not a span, and
        // `holder` already says so — no case of its own here.
        let (kids, f) = field(&sheet("", "|R#r1| \"100k\"\nu1.a - r1 - u1.d\n"));
        assert!(f.spans().is_empty(), "a bridge spans nothing");
        assert_eq!(seat(&kids, &f, "r1").side, Side::Left, "off the first pin");

        let src = format!(
            "{}|R#r2| \"1k\"\nu1.d - r2 - u2.a\n",
            sheet(
                "",
                "|component#u2| { cell: 2 1 } [ |pin#a| { side: left } ]\n"
            )
        );
        let (kids, f) = field(&src);
        let i = kids.iter().position(|c| c.id.as_deref() == Some("r2"));
        assert_eq!(f.spans().len(), 1, "two anchors is a span");
        assert!(f.seat(i.expect("r2")).is_none(), "and holds no seat yet");
    }

    #[test]
    fn a_satellite_no_wire_holds_floats() {
        let (kids, f) = field(&sheet("", "|gnd#g1|\n|R#r1| \"1k\"\nr1.p2 - g1\n"));
        let ids: Vec<&str> = f
            .floating()
            .iter()
            .filter_map(|&i| kids[i].id.as_deref())
            .collect();
        assert_eq!(ids, ["g1", "r1"], "both, in declaration order");
    }

    #[test]
    fn a_fields_reach_measures_the_distance_its_cells_stand_out() {
        // A distance and no longer a count: a lane is a coarse line and a slot
        // a fine one, so the packer is told how far the field reaches rather
        // than how many cells it took.
        let (kids, f) = field(&sheet("", "|R#r1| \"1k\"\n|gnd#g1|\nu1.a - r1 - g1\n"));
        let u1 = kids
            .iter()
            .position(|c| c.id.as_deref() == Some("u1"))
            .expect("u1");
        assert_eq!(f.extent(u1, Side::Left), 100.0, "one column out");
        // The chain turned off a left pin, so its slots clear that pin and not
        // the whole part: two of them, a coarse pitch apart.
        assert_eq!(f.extent(u1, Side::Bottom), 160.0, "two slots deep");
        assert_eq!(f.extent(u1, Side::Right), 0.0, "nothing on the other side");
    }
}

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
use super::super::net;
use super::super::place::Slot;
use super::super::terminal::{Terminal, ident, terminal};
use super::read::{growth, tag_facing, tap_flags};
use super::{Field, LANED, Ladder, STRAIGHT, Seat, allocate, ink_edge};
use crate::desugar::pose::Side;
use crate::desugar::schematic::chain::{Chain, End, beside, limbs, tap_ray};
use crate::desugar::schematic::{SchKind, sch_kind};
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
        share_pins(children, &mut held);
        order(&mut held, ladders.len());
        self.strike_origins(children, wires, tracks, &held);
        let bases: Vec<f64> = held
            .iter()
            .map(|h| self.base(h.anchor, h.side, &held))
            .collect();
        for (h, base) in held.iter_mut().zip(bases) {
            h.base = base;
        }
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
        let turned = h.turns();
        let members = pick(chain, |i| limbs[i].is_none());
        // A straight chain sharing its pin with chains that turned starts
        // past the lanes they took [SPEC 16.1]: its lead is theirs too, and
        // the junction where they leave it lies on the bare wire before its
        // first member — the pull-down hangs off the gate's own trace, ahead
        // of the series resistor. Its column then begins where those lanes'
        // cells end, the run in to there being one lead shared three ways.
        let (start, origin) = match self.shared_edge(h) {
            // A no-connect cross is a mark on the pin, not a member
            // [SPEC 16.4]: it stands on the first fine line at least a fine
            // pitch past the stub tip, and shares no slot row.
            None if h.is_mark(children) => {
                let out = Ax::outward(h.ray);
                let tip = match Ax::of(h.ray) {
                    Ax::X => h.pin.at.0,
                    Ax::Y => h.pin.at.1,
                };
                (self.lat.past(tip + out * self.lat.pitch, out), h.pin.at)
            }
            None => (self.origin(h.anchor, h.ray, turned), h.pin.at),
            Some(edge) => {
                let out = Ax::outward(h.ray);
                let deeper = |a: f64, b: f64| if b * out > a * out { b } else { a };
                let line = self.past_ink(members[0], h.ray, edge);
                let origin = match Ax::of(h.ray) {
                    Ax::X => (edge, h.pin.at.1),
                    Ax::Y => (h.pin.at.0, edge),
                };
                (deeper(self.origin(h.anchor, h.ray, turned), line), origin)
            }
        };
        let trunk = self.run(h, h.ray, start, members, Some(origin));
        // One allocation for either ladder [SPEC 16.1]: a chain that turned off
        // its pin takes the innermost free lane, and one that grew straight out
        // asks for its pin's own line — stepping beside it only where a chain
        // already claimed that corridor, which is the first claimant's.
        let k = self.allot(h, &trunk);
        self.commit(h, &trunk, k);

        // A **tap** takes no slot [SPEC 16.1]: it stands on its attachment's,
        // one step across, the way its own drawing points.
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
            self.take(member, self.stepped(attach, member, across));
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
                // Across the trunk: the members march out from the junction on
                // its own slot, a step each, exactly as a tap does.
                let mut at = attach;
                for &m in &members {
                    at = (m, self.stepped(at, m, ray));
                    self.take(m, at.1);
                }
                continue;
            }
            // On the trunk's own axis: a lane of its own — the occupancy
            // carries it sideways until it stands clear — with its slots
            // carrying on from the junction.
            let first = self.after((attach.0, attach.1.along), members[0], ray);
            let branch = self.run(h, ray, first, members, None);
            let k = self.allot(h, &branch);
            self.commit(h, &branch, k);
        }
    }

    /// One run laid out along its ray [SPEC 16.1]: its first member on `start`
    /// — the field's own origin for a trunk, the junction's next step for a
    /// branch — and each one after it a step past the one before, which is
    /// half of each of their drawings and a fine pitch of air. Its ladder's
    /// base is the innermost line across the ray whose cells clear the
    /// anchor's ink, for a chain that turned; the pin's own line, for one that
    /// grew straight out.
    fn run(
        &self,
        h: &Held,
        ray: Side,
        start: f64,
        members: Vec<usize>,
        origin: Option<(f64, f64)>,
    ) -> Run {
        let mut lines = Vec::with_capacity(members.len());
        let mut prev: Option<(usize, f64)> = None;
        for &m in &members {
            let at = match prev {
                None => start,
                Some(p) => self.after(p, m, ray),
            };
            lines.push(at);
            prev = Some((m, at));
        }
        let ladder = if h.turns() {
            // The lanes count from the **side's** base ([`Field::base`]), so
            // an up-chain and a down-chain off one pin can share one.
            Ladder {
                side: h.side,
                base: h.base,
            }
        } else {
            Ladder {
                side: beside(ray, h.pin.facing),
                base: across(h.pin.at, ray),
            }
        };
        Run {
            ray,
            ladder,
            lines,
            members,
            origin,
        }
    }

    /// Strike every **slot origin**    /// Strike every **slot origin** [SPEC 16.1], before a chain grows.
    ///
    /// A slot clears what its own chain's lead actually passes. A chain that
    /// **turned** into a lane is beside the anchor's body already — that is
    /// what the lane is — so its slots clear only the deepest pin the ray's
    /// laned chains leave from. One that grew **straight** out has its ray
    /// pointing through the body, and clears the anchor's whole ink. Both by
    /// half of the member that stands there, and both onto the first **fine**
    /// line beyond: the separation is ink's, and a coarse line would round it
    /// up to a whole cell of bare wire.
    ///
    /// And the origin belongs to the **track line**, not the anchor: every
    /// anchor riding the line across the ray — the same row for an up or down
    /// ray, the same column for a left or right one — takes the deepest
    /// requirement among them, which is what stands two anchors' fields on one
    /// row. It is the one line in the field that is a rule rather than a
    /// search, and the alignment is what it buys.
    fn strike_origins(
        &mut self,
        children: &[PlacedNode],
        wires: &[[End; 2]],
        tracks: &[Option<Slot>],
        held: &[Held],
    ) {
        // The deepest **wired** pin of one side along `ray` — every wire
        // leaving that side runs out along its pin's row, so a column
        // climbing past those rows crosses bare wire only once its first
        // member clears the deepest of them.
        let deepest = |anchor: usize, side: Side, ray: Side| {
            wires
                .iter()
                .flatten()
                .filter(|e| e.child == anchor)
                .map(|e| terminal(&children[anchor], e.terminal.as_deref()))
                .filter(|t| t.facing == Some(side))
                .map(|t| dot(t.at, ray.normal()))
                .fold(f64::NEG_INFINITY, f64::max)
        };
        for ray in Side::ALL {
            let (ax, out) = (Ax::of(ray), Ax::outward(ray));
            let deeper = |a: f64, b: f64| if b * out > a * out { b } else { a };
            // What one chain asks of the line its first member stands on: half
            // of that member's own drawing past whatever its lead passes —
            // the deepest wired pin of its side along this ray, for a lead
            // that turned into a lane; the anchor's ink, for one whose ray
            // points through the body.
            let mut want: Vec<(usize, usize, f64)> = Vec::new();
            for h in held.iter().filter(|h| h.ray == ray && !h.is_mark(children)) {
                let Some(track) = tracks[h.anchor] else {
                    continue;
                };
                let passes = if h.turns() {
                    deepest(h.anchor, h.side, ray).max(h.depth) * out
                } else {
                    ink_edge(&self.inks[h.anchor], ray)
                };
                let line = self.past_ink(h.chain.members[0], ray, passes);
                want.push((track.on(ax), usize::from(h.turns()), line));
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

    /// The innermost line a **side**'s lanes may start on [SPEC 16.1]: past the
    /// anchor's ink by the fine pitch of air, and by half the widest cell any
    /// chain leaving that side carries — so a part's cell edge lands on the
    /// body and no further out, and a side carrying only bare ground wires
    /// starts a fine pitch off it.
    ///
    /// The **side's**, not one chain's: an up-chain and a down-chain off one
    /// pin share a lane because their cells are disjoint, and they can only do
    /// that if they count from one line.
    fn base(&self, anchor: usize, side: Side, held: &[Held]) -> f64 {
        let out = Ax::outward(side);
        let widest = held
            .iter()
            .filter(|h| h.anchor == anchor && h.side == side && h.turns())
            .flat_map(|h| h.chain.members.iter().map(|&m| self.across(m, h.ray)))
            .fold(0.0f64, f64::max);
        let ink = ink_edge(&self.inks[anchor], side);
        self.lat
            .past(ink + out * (widest / 2.0 + self.lat.pitch), out)
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
        let cell = self.cell(member, seat);
        let half = self.lat.pitch / 2.0;
        // The edge of the cell the column arrives at.
        let near = |at: f64, lo: f64, hi: f64| {
            if (at - lo).abs() <= (at - hi).abs() {
                lo
            } else {
                hi
            }
        };
        Some(match Ax::of(run.ray) {
            Ax::X => Bbox::from_points(&[
                (px, seat.cross - half),
                (near(px, cell.min_x, cell.max_x), seat.cross + half),
            ]),
            Ax::Y => Bbox::from_points(&[
                (seat.cross - half, py),
                (seat.cross + half, near(py, cell.min_y, cell.max_y)),
            ]),
        })
    }

    /// Where the lanes of the chains sharing a straight chain's pin end,
    /// along its ray [SPEC 16.1] — the outer edge of their first cells, as
    /// far out as the deepest reaches — or `None` for a chain sharing its pin
    /// with none. Those chains grew first, so their seats are struck.
    fn shared_edge(&self, h: &Held) -> Option<f64> {
        let out = Ax::outward(h.ray);
        h.shared
            .iter()
            .filter_map(|&(member, ray)| {
                let seat = self.seats[member]?;
                Some(seat.cross + out * self.across(member, ray) / 2.0)
            })
            .reduce(|a, b| if b * out > a * out { b } else { a })
    }

    /// The seats a run's members take with its cross ladder at step `k`.
    fn seats_of<'a>(
        &'a self,
        h: &'a Held,
        run: &'a Run,
        k: i32,
    ) -> impl Iterator<Item = (usize, Seat)> + 'a {
        let cross = self.ladder_at(run.ladder, k);
        run.members
            .iter()
            .zip(&run.lines)
            .map(move |(&member, &along)| {
                (
                    member,
                    Seat {
                        anchor: h.anchor,
                        ray: run.ray,
                        side: h.side,
                        cross,
                        along,
                    },
                )
            })
    }

    /// The member `i` hangs off and its seat — its attachment up the walk.
    /// `None` while that member has none, which leaves `i` unseated rather
    /// than guessing a junction.
    fn attachment(&self, chain: &Chain, i: usize) -> Option<(usize, Seat)> {
        let parent = chain.members[chain.parents[i]?];
        Some((parent, self.seats[parent]?))
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
    /// How far along the ray its pin already sits — the lane order's key.
    depth: f64,
    /// For a chain that grew **straight** out: the first member and ray of
    /// every chain that turned off the same pin ([`share_pins`]) — the lanes
    /// its own first slot has to clear. Empty for every other chain.
    shared: Vec<(usize, Side)>,
    /// The innermost line its side's lanes start on ([`Field::base`]), struck
    /// once every chain on that side is known.
    base: f64,
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
            shared: Vec::new(),
            chain,
            pin,
            side,
            ray,
            group,
            base: 0.0,
        }
    }

    /// Whether the chain **turned** off its pin, and so takes a lane. One
    /// that grew straight out keeps the pin's own fine line and competes for
    /// nothing.
    fn turns(&self) -> bool {
        self.ray != self.side
    }

    /// Whether the chain is one no-connect cross grown straight off its pin
    /// [SPEC 16.4] — a mark on the pin, seated by its own rule.
    fn is_mark(&self, children: &[PlacedNode]) -> bool {
        !self.turns()
            && matches!(self.chain.members[..], [m] if
                sch_kind(&children[m].type_chain) == Some(SchKind::Label)
                    && ident(&children[m].attrs, "symbol").as_deref() == Some("nc"))
    }

    /// When the chain grows [SPEC 16.1]: straight chains first, as the
    /// geography every lane steps past — bar one sharing its pin with a
    /// lane, which grows with that pin's lanes ([`order`]).
    fn phase(&self) -> u8 {
        u8::from(self.turns() || !self.shared.is_empty())
    }
}

/// Mark every straight chain led by a **part** with the turned chains leaving
/// its own pin [SPEC 16.1]. A chain led by a bare net run is exempt: the run
/// *is* the trace named, so a lane sharing its pin steps past it and the
/// trunk runs on through the run to the junction, exactly as the ray rule
/// exempts it. The pin is the terminal's landing point, compared in whole
/// [`EPS`] steps as depths are.
fn share_pins(children: &[PlacedNode], held: &mut [Held]) {
    let rung = |v: f64| (v / EPS).round() as i64;
    let at = |h: &Held| (h.anchor, rung(h.pin.at.0), rung(h.pin.at.1));
    let turned: Vec<((usize, i64, i64), usize, Side)> = held
        .iter()
        .filter(|h| h.turns())
        .map(|h| (at(h), h.chain.members[0], h.ray))
        .collect();
    for h in held
        .iter_mut()
        .filter(|h| !h.turns() && !net::is_run(&children[h.chain.members[0]]))
    {
        let key = at(h);
        h.shared = turned
            .iter()
            .filter(|(k, _, _)| *k == key)
            .map(|&(_, m, ray)| (m, ray))
            .collect();
    }
}

/// One run of members growing one way from one point — a chain's trunk from
/// its pin, or a branch from its junction.
struct Run {
    ray: Side,
    ladder: Ladder,
    /// The line each member stands on along the ray, in the anchor's frame.
    lines: Vec<f64>,
    members: Vec<usize>,
    /// Where the run's own column starts, for a trunk: its pin, in the
    /// anchor's frame ([`Field::stem`]). A branch starts at a junction whose
    /// cell is already committed, so it needs none.
    origin: Option<(f64, f64)>,
}

/// The **allocation order** [SPEC 16.1]. Chains that grew straight out along
/// their pins take no lane and compete for none, so they commit first: they
/// are the inner geography every lane then steps past. The lanes go next,
/// **pin by pin** — a pin's up-chain and down-chain leave on one lead and
/// share a column, so they are allotted together — innermost first to the
/// pin whose column the fewest of the side's other leads would have to
/// cross ([`rank_pins`]); a part-led straight chain sharing its pin grows
/// right after that pin's lanes, past them, and is geography to every pin
/// ranked outside it like any straight chain. A stable sort, so the chains'
/// own statement order breaks every tie.
fn order(held: &mut Vec<Held>, ladders: usize) {
    let mut rank = vec![0usize; held.len()];
    for group in 0..ladders {
        let idx: Vec<usize> = (0..held.len())
            .filter(|&i| held[i].turns() && held[i].group == group)
            .collect();
        for (i, r) in rank_pins(held, &idx) {
            rank[i] = r;
        }
    }
    // A straight chain sharing its pin grows right after that pin's lanes:
    // it takes their rank, and stands after them within it.
    for i in 0..held.len() {
        if let Some(&(member, _)) = held[i].shared.first()
            && let Some(t) = held
                .iter()
                .position(|t| t.turns() && t.chain.members[0] == member)
        {
            rank[i] = rank[t];
        }
    }
    let mut by: Vec<usize> = (0..held.len()).collect();
    by.sort_by_key(|&i| (held[i].phase(), held[i].group, rank[i], !held[i].turns()));
    let mut taken: Vec<Option<Held>> = std::mem::take(held).into_iter().map(Some).collect();
    held.extend(
        by.iter()
            .map(|&i| taken[i].take().expect("each chain once")),
    );
}

/// The lane rank of every pin on one side [SPEC 16.1], innermost first.
///
/// A pin's column is **live** above its row where a chain climbs off it and
/// below where one drops, and a lead crosses every inner column that is
/// live toward its own pin. So the lanes go innermost first to the pin whose
/// column the fewest of the others' leads would cross — counted over the
/// pins still to be placed, since only those lie outside it. On a side whose
/// chains all grow one way that is depth along the ray: the deepest pin keeps
/// the inner lane, and a lead crosses an inner column only above where it is
/// live. A side carrying both rays crosses only where an upper pin's return
/// must drop past a lower pin's rail, and there on the rail's lead alone,
/// since a slot clears the deepest pin its side leaves from. Ties fall to the
/// pin deeper along the canonical direction of the axis — down, or right —
/// then to statement order.
///
/// Depth compares in whole [`EPS`] steps, never bit for bit: the pins of one
/// rail stand at one depth, but the stub tips that measure it carry that depth
/// to a bit or two and not to the bit — a pin whose name is wider than its
/// neighbour's shifts the last of them. Quantised, one depth reads as one
/// depth and the statement order decides, which is what the tie-break says;
/// raw, a name's width silently outranks it.
fn rank_pins(held: &[Held], idx: &[usize]) -> Vec<(usize, usize)> {
    let rung = |v: f64| (v / EPS).round() as i64;
    let key = |i: usize| (rung(held[i].pin.at.0), rung(held[i].pin.at.1));
    // The side's pins, in statement order, each with the chains it holds.
    let mut pins: Vec<((i64, i64), Vec<usize>)> = Vec::new();
    for &i in idx {
        match pins.iter_mut().find(|(k, _)| *k == key(i)) {
            Some((_, chains)) => chains.push(i),
            None => pins.push((key(i), vec![i])),
        }
    }
    // Whether pin `q`'s column is live toward pin `p`: one of q's chains
    // grows the way p lies.
    let toward = |q: &[usize], p: &[usize]| {
        let (qa, pa) = (held[q[0]].pin.at, held[p[0]].pin.at);
        q.iter()
            .any(|&c| dot((pa.0 - qa.0, pa.1 - qa.1), held[c].ray.normal()) > EPS)
    };
    let mut out = Vec::with_capacity(idx.len());
    let mut remaining: Vec<usize> = (0..pins.len()).collect();
    while !remaining.is_empty() {
        let pick = *remaining
            .iter()
            .min_by_key(|&&q| {
                let crossed = remaining
                    .iter()
                    .filter(|&&p| p != q && toward(&pins[q].1, &pins[p].1))
                    .count();
                let first = pins[q].1[0];
                let deep = dot(held[first].pin.at, canonical(held[first].ray).normal());
                (crossed, std::cmp::Reverse(rung(deep)), q)
            })
            .expect("a pin remains");
        remaining.retain(|&q| q != pick);
        let r = out.len();
        out.extend(pins[pick].1.iter().map(|&i| (i, r)));
    }
    out
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

    /// Two discretes stand this far apart along their ray: half of each of
    /// their 64-long drawings and the fine pitch of air between them, out to
    /// the next fine line — which is the default `gap`, derived rather than
    /// stated.
    const STEP: f64 = 100.0;

    /// Whether a seat's chain **turned** off its pin, and so runs in a lane.
    fn turned(seat: Seat) -> bool {
        seat.ray != seat.side
    }

    /// How far out on its own side a seat stands, from the anchor's origin —
    /// the lane, as a distance rather than an ordinal.
    fn lane(seat: Seat) -> f64 {
        seat.cross * Ax::outward(seat.side)
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
        assert_eq!((g.ray, g.side), (Side::Bottom, Side::Bottom));
        assert!(!turned(g), "it grew straight out and took no lane");
        // …and its ray points through the body, so its slot clears the whole
        // ink [SPEC 16.1] — half its own drawing, and the fine pitch two wired
        // neighbours keep.
        let ink = ink(&kids, "u1").max_y;
        assert!(
            g.along > ink + 20.0 && g.along < ink + 60.0,
            "a fine pitch clear of {ink}, and no cell more: {}",
            g.along
        );
    }

    #[test]
    fn a_chain_that_turns_takes_the_innermost_lane_and_slots_from_the_origin() {
        // Off a left pin the ground still grows down, so the chain turns and
        // takes lane 1; its members step one coarse slot at a time.
        let (kids, f) = field(&sheet("", "|R#r1| \"1k\"\n|gnd#g1|\nu1.a - r1 - g1\n"));
        let (r, g) = (seat(&kids, &f, "r1"), seat(&kids, &f, "g1"));
        assert_eq!((r.ray, r.side), (Side::Bottom, Side::Left));
        assert!(turned(r), "off a left pin, growing down, it turned");
        assert_eq!(g.cross, r.cross, "one lane for the chain");
        // A ground is a symbol and no part, so it ends the chain a step under
        // the resistor rather than a whole cell under it.
        assert!(
            g.along - r.along < STEP,
            "the ground steps under the part, not a cell: {}",
            g.along - r.along
        );
        // The lane already cleared the body, so the slots clear the **pin**
        // and the first member stands beside the part, not below it.
        assert!(
            r.along < ink(&kids, "u1").max_y,
            "seated beside the anchor, at {}",
            r.along
        );
    }

    #[test]
    fn the_deeper_pin_along_the_ray_keeps_the_inner_lane() {
        // [SPEC 16.1] the shallower pin's chain steps out, so its lead
        // crosses the inner column only above where that column is live.
        // `b` is the lower of the two left pins, whatever the wire order.
        let (kids, f) = field(&sheet("", "|gnd#ga|\n|gnd#gb|\nu1.a - ga\nu1.b - gb\n"));
        let (ga, gb) = (seat(&kids, &f, "ga"), seat(&kids, &f, "gb"));
        assert!(
            lane(gb) < lane(ga),
            "the deeper pin keeps the inner lane: {} vs {}",
            lane(gb),
            lane(ga)
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
        assert!(!turned(r1) && !turned(r2), "both grew straight out");
        assert_eq!(r1.cross, line("DE"), "the first stated keeps its line");
        assert_ne!(r2.cross, line("NRE"), "and the second steps beside it");
    }

    #[test]
    fn an_up_chain_and_a_down_chain_off_one_pin_share_a_lane() {
        // Their cells are disjoint, so the second one's innermost candidate
        // is already free — a consequence of the occupancy test, not a rule.
        let src = sheet(FLAG, "|gnd#g1|\n|vp#f1|\nu1.a - g1\nu1.a - f1\n");
        let (kids, f) = field(&src);
        let (g, v) = (seat(&kids, &f, "g1"), seat(&kids, &f, "f1"));
        assert_eq!(g.cross, v.cross, "one lane, two rays");
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
        assert_eq!(t.along, l.along, "the flag keeps its attachment's slot");
        assert!(lane(t) > lane(l), "…one step outward of it");
        assert_eq!(r.along, l.along + STEP, "and the trunk grows on past it");

        // The same chain off a **bottom** pin, where the trunk keeps its
        // pin's own fine line and there is no lane to step: the flag steps
        // that line instead, by one coarse cell.
        let (kids, f) = field(&src.replace("u1.a - l1", "u1.c - l1"));
        let (l, t) = (seat(&kids, &f, "l1"), seat(&kids, &f, "f1"));
        assert!(!turned(l) && !turned(t), "a laneless trunk, and its tap");
        assert_eq!(t.along, l.along, "still on its attachment's slot");
        assert!(
            t.cross > l.cross,
            "stepped across: {} vs {}",
            t.cross,
            l.cross
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
        assert_eq!(c1.cross, r1.cross, "the trunk keeps one lane");
        assert!(lane(r2) > lane(r1), "the branch steps out of it");
        assert_eq!(
            (r2.along, c1.along),
            (r1.along + STEP, r1.along + STEP),
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
        // The lane holds a part, so it stands a whole cell out and its cell
        // reaches half a cell past it; the ground under it holds a symbol and
        // asks for far less.
        assert_eq!(
            f.extent(u1, Side::Left),
            170.0,
            "one column out, to its edge"
        );
        // The chain turned off a left pin, so its slots clear that pin and not
        // the whole part — and its ground ends it a step under the resistor,
        // the field reaching exactly to the ground's own cell edge.
        let g = seat(&kids, &f, "g1");
        let ink = ink(&kids, "g1");
        assert_eq!(
            f.extent(u1, Side::Bottom),
            g.along + f.lat.pitches(ink.h() + f.lat.pitch) / 2.0,
            "the ground's cell edge"
        );
        assert_eq!(f.extent(u1, Side::Right), 0.0, "nothing on the other side");
    }
}

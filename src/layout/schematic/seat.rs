//! The **satellite seat pass** [SPEC 16.1] — where the parts that ride no
//! track go.
//!
//! A satellite chain reads its wire:
//!
//! - **one placed end** — it grows outward from that pin, link by link, in
//!   the direction its terminator's connection geometry faces away from
//!   (a `|gnd|`'s point is at its top, so a chain ending in one grows *down*;
//!   a power flag's is at its bottom, so up). Auto-pose has already turned
//!   each satellite to face back at the pin ([`crate::desugar::autopose`]),
//!   so for an unforced chain that direction *is* the pin's outward normal;
//!   an authored `rotate:` forces the pose and the seat follows the turned
//!   connection point instead.
//! - **two placed ends, on two anchors** — its satellites distribute along the
//!   straight line between the two pins, at even fractions. Two ends on **one**
//!   anchor are a fan, not a span, and grow like a one-end chain — the rule is
//!   [`crate::desugar::schematic::chain::holder`]'s, shared with the chooser.
//! - **no placed end** — nothing to seat against: the flow fallback, and a
//!   warning [SPEC 21].
//!
//! Seats are **pin-relative**: a one-end chain is measured off its anchor's
//! own origin, so it can join the anchor's cluster before the tracks size
//! ([`Seats::cluster`]) and land in scene coordinates after they place
//! ([`Seats::absolutize`]) — move the component and its satellites travel
//! along. A two-end chain has no single anchor, so it resolves in the same
//! absolutize step, once both its ends are placed.
//!
//! A two-end chain joins no cluster — it sits *between* the anchors, so no
//! single one owns it — but it is not free of the tracks either: it **sizes
//! the space it lands in**, asking the two tracks it spans to part far enough
//! for the even fractions to clear ([`Demand`], struck in [`super::place`]).
//!
//! **Known limit — a two-end chain is never packed.** Sizing the span is not
//! stacking: a spanning chain never enters an anchor's [`Stack`], so a pin
//! holding both a one-end chain and a two-end one can still draw them **on top
//! of each other**. That is a different limit from the sizing one above, and
//! closing it means seating the spanning chains in a second packing pass
//! *after* the tracks place — a pass reordering, deliberately not built here;
//! Phase 6 owns it.

use super::super::geom::{Frame, project};
use super::super::ir::{Bbox, PlacedNode};
use super::super::stack::{Band, SeatLine, Stack};
use super::net;
use super::terminal::{Terminal, connection_box, terminal};
use crate::desugar::pose::Side;
use crate::desugar::schematic::Role;
use crate::desugar::schematic::chain::{Chain, End, chains, holder, placed_ends};
use crate::ledger::consts::{DEFAULT_CLEARANCE, LABEL_SEAT};
use crate::resolve::{LinkKind, ResolvedLink};

/// A satellite's seat off the anchor that holds its chain: an offset from
/// that anchor's origin, in sheet coordinates.
struct Seat {
    anchor: usize,
    dx: f64,
    dy: f64,
}

/// A chain one anchor holds, with everything the lane ladder and the seat both
/// read off it — struck once in [`Seats::build`], because [`growth`] answers
/// the ray and the pin together and neither pass may ask a second time.
struct Growing {
    /// Its (anchor, ray, turn side) ladder — chains turning onto one ray from
    /// the same side compete for lanes; opposite-side chains stand on opposite
    /// sides of the part and can never cross, so they never share a ladder.
    group: usize,
    ray: Side,
    /// The pin it hangs from, in the anchor's own frame.
    pin: (f64, f64),
    /// How far along the ray that pin already sits — the ladder's order.
    depth: f64,
    /// The outward sign along the lane axis, `0` when the ray *is* the pin's
    /// own normal and the chain never turns.
    lead: f64,
    chain: Chain,
    held: End,
}

/// A chain held at both ends: its satellites distribute between the two
/// terminals once the anchors are placed.
struct Spanning {
    members: Vec<usize>,
    ends: [(usize, Terminal); 2],
}

/// Every satellite's placement decision, made before the tracks size.
pub(super) struct Seats {
    seats: Vec<Option<Seat>>,
    spanning: Vec<Spanning>,
    /// Satellites with no placed end — the flow fallback [SPEC 16.1].
    floating: Vec<usize>,
    /// Where each **net run**'s name steps off its own trace [SPEC 16.4] —
    /// decided here, because this is where the run's landing and the anchor's
    /// painted stack both exist; applied to the label's text in
    /// [`Seats::absolutize`], and counted by every extent this pass reports.
    text: Vec<(f64, f64)>,
    /// The scope's seat gap ([`seat_gap`]) — every distance this pass leaves.
    seat: f64,
}

/// The scope's **seat gap** [SPEC 16.1/10.5]: the clear run a satellite is set
/// off the pin it hangs from, and off the satellite before it.
///
/// It is a routing corridor, not just daylight — the lead between them is an
/// ordinary routed wire, and the channel model gives it a cell only where the
/// two keep-outs do not overlap. So the gap is read off the scope's own
/// `clearance`, never assumed: `2 × clearance` for the two keep-outs plus the
/// half-clearance a run needs to sit in. At the schematic scope's own
/// clearance that is exactly [`LABEL_SEAT`], which is the floor — a *tighter*
/// clearance still puts satellites on the sheet's own pitch.
pub(super) fn seat_gap(attrs: &crate::resolve::AttrMap) -> f64 {
    let c = attrs.number("clearance").unwrap_or(DEFAULT_CLEARANCE);
    LABEL_SEAT.max(2.5 * c)
}

impl Seats {
    /// Seat the scope's satellites against the pins their wires reach.
    pub(super) fn build(
        children: &[PlacedNode],
        roles: &[Role],
        links: &[&ResolvedLink],
        scope: &str,
        seat: f64,
    ) -> Seats {
        let satellite: Vec<bool> = roles.iter().map(|r| *r == Role::Satellite).collect();
        let mut out = Seats {
            seats: (0..children.len()).map(|_| None).collect(),
            spanning: Vec::new(),
            floating: Vec::new(),
            text: vec![(0.0, 0.0); children.len()],
            seat,
        };
        if !satellite.contains(&true) {
            return out;
        }
        // One packer per anchor, so every chain hanging off it — on the same
        // pin or another — clears the body and the chains seated before it
        // [SPEC 16.1]. `chains` walks the satellites in **declaration order**,
        // so that is the stacking order: the parts' own statements decide,
        // like every other thing this engine places, and rewriting the wires
        // never moves a part.
        let mut packers: Vec<Stack> = children.iter().map(|_| Stack::default()).collect();
        for (i, c) in children.iter().enumerate() {
            packers[i].obstruct(drawn(c));
        }
        let mut held: Vec<Growing> = Vec::new();
        // The rays seen so far — an (anchor, growth direction, turn side)
        // triple. A ray splits into one ladder per side it is entered from:
        // a chain turning left off a left pin and one turning right off a
        // right pin grow down the same ray on opposite sides of the part,
        // where their leads can never cross — laddering them together would
        // step one past the other's reach for nothing. Straight-growing
        // chains (`lead == 0`) take no lane, so their side is moot. The rays
        // keep the order they were declared in and only the chains within one
        // are reordered.
        let mut rays: Vec<(usize, Side, i8)> = Vec::new();
        for chain in chains(&satellite, &edges(children, links, scope)) {
            let ends = placed_ends(&chain, roles);
            // One anchor holds it → grow off that pin; two → span between them;
            // none → the flow fallback. Which of the first two a chain is, is
            // [`holder`]'s single answer, shared with the pose chooser.
            match (holder(&ends), ends.as_slice()) {
                (Some(one), _) => {
                    let one = one.clone();
                    let (ray, pin) = growth(children, &chain, &one);
                    let frame = Frame::outward(ray.normal());
                    let depth = frame.cross(pin.at);
                    let lead = frame.u(pin.facing.map_or((0.0, 0.0), Side::normal));
                    // f64::signum maps 0.0 to 1.0, so the no-turn side is spelled out.
                    let side = if lead == 0.0 { 0 } else { lead.signum() as i8 };
                    let key = (one.child, ray, side);
                    let group = rays.iter().position(|r| *r == key).unwrap_or_else(|| {
                        rays.push(key);
                        rays.len() - 1
                    });
                    held.push(Growing {
                        group,
                        ray,
                        pin: pin.at,
                        depth,
                        lead,
                        chain,
                        held: one,
                    });
                }
                (None, [a, b, ..]) => out.distribute(children, chain, a, b),
                (None, _) => out.floating.extend(chain.members),
            }
        }
        // **No chain overtakes another** [SPEC 16.1] — the routing contract's
        // own rule for the runs in a channel ("wires leave in the order they
        // arrive — nested, never braided", ROUTING.md model step 5), asked one
        // pass earlier of the parts those wires will join. Which end of the
        // order that is depends on the axis a chain competes for:
        //
        // - a chain that **turns** onto its ray competes for a lane, and its
        //   leg crosses every lane inside its own — so the one off the
        //   *shallower* pin has to take the *outer* lane, or its turn cuts
        //   through a deeper chain's leg. Deepest first.
        // - a chain that grows **straight** out along its pin takes no lane and
        //   stacks in depth instead, where the stack only ever pushes outward —
        //   so the shallowest keeps its own pin's depth and the rest pass it.
        //   Shallowest first.
        //
        // A stable sort, so chains sharing a ray *and* a pin keep their
        // statement order, which is the only thing left to break the tie.
        held.sort_by(|a, b| {
            a.group.cmp(&b.group).then(if a.lead == 0.0 {
                a.depth.total_cmp(&b.depth)
            } else {
                b.depth.total_cmp(&a.depth)
            })
        });
        let lanes = ladder(children, &held, seat);
        for (g, along) in held.iter().zip(lanes) {
            let frame = Frame::outward(g.ray.normal());
            out.grow(children, g, frame, along, &mut packers[g.held.child]);
        }
        out
    }

    /// A one-placed-end chain: each satellite seats farther out along the
    /// growth ray, its connection point landing where the packer clears.
    fn grow(
        &mut self,
        children: &[PlacedNode],
        g: &Growing,
        frame: Frame,
        along: f64,
        stack: &mut Stack,
    ) {
        let (chain, held) = (&g.chain, &g.held);
        // The chain hangs off the wire's **first leg** — out along the pin to
        // its own lane ([`Self::lane`]) — and grows from there in its
        // terminator's direction [SPEC 16.1]: a cap under a side pin sits
        // below the wire leaving it, not inside the component's own column.
        //
        // Growth is **monotone**: each member's base is the previous member's
        // outer paint edge, so a later member can never tuck into a hole a
        // foreign band opened before an earlier one — "link by link" is an
        // order, not just a distance.
        let mut base = frame.cross(g.pin);
        for (&member, inbound) in chain.members.iter().zip(&chain.inbound) {
            let sat = &children[member];
            let point = terminal(sat, inbound.as_deref()).at;
            let box_ = drawn(sat);
            // The band is the satellite's own reach either side of the point
            // its wire lands on, so the packer measures from the connection.
            let (lo, hi) = (
                frame.cross(corner(box_, false)),
                frame.cross(corner(box_, true)),
            );
            let c = frame.cross(point);
            let band = Band {
                neg: c - lo.min(hi),
                pos: hi.max(lo) - c,
            };
            // A **net run**'s name steps off the trace here [SPEC 16.4]: the
            // run's middle is `band.neg / 2` back from the innermost landing,
            // and the freer side is read against what this anchor has already
            // painted. The step is across the growth ray, so the band above is
            // untouched and only the run's own reach changes.
            if net::is_run(sat) {
                let mid = frame.pt(along, base + self.seat + band.neg / 2.0);
                self.text[member] = net::seat_text(sat, mid, stack.painted());
            }
            let box_ = self.extent(children, member);
            let (u0, u1) = (frame.u(corner(box_, false)), frame.u(corner(box_, true)));
            let u = frame.u(point);
            let interval = (along + u0.min(u1) - u, along + u0.max(u1) - u);
            let line = stack.seat(SeatLine::new(frame, true, base), interval, self.seat, &band);
            base = line + band.pos;
            let target = frame.pt(along, line);
            self.seats[member] = Some(Seat {
                anchor: held.child,
                dx: target.0 - point.0,
                dy: target.1 - point.1,
            });
        }
    }

    /// A chain held at both ends distributes along the pin-to-pin line at
    /// even fractions — `i / (n + 1)` for the `i`-th of `n` [SPEC 16.1]. Only
    /// the two terminals are read here; the line itself is not known until
    /// the anchors place, so the fractions are struck in [`Self::absolutize`].
    fn distribute(&mut self, children: &[PlacedNode], chain: Chain, a: &End, b: &End) {
        let end = |e: &End| (e.child, terminal(&children[e.child], e.terminal.as_deref()));
        self.spanning.push(Spanning {
            members: chain.members,
            ends: [end(a), end(b)],
        });
    }

    /// One anchor's **cluster** extent [SPEC 16.1]: the anchor plus the
    /// satellites seated on it, so they consume space without consuming
    /// cells. In the anchor's own coordinates, like its bbox — and measured
    /// as [`drawn`], the one extent notion the seat pass grows and steps by,
    /// so a track holds its parts' **ink**: a stub tip, a ref readout and a
    /// tag outline all sit inside the scope's own box, and the sheet a
    /// container places is the sheet the router sees.
    pub(super) fn cluster(&self, children: &[PlacedNode], anchor: usize) -> Bbox {
        self.seats
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().filter(|s| s.anchor == anchor).map(|s| (i, s)))
            .fold(drawn(&children[anchor]), |b, (i, s)| {
                b.union(self.extent(children, i).shifted(s.dx, s.dy))
            })
    }

    /// A satellite's placed extent in its own frame — the one reading of "how
    /// much room this satellite needs", so the packer, the cluster and the
    /// scope's box can never disagree about it. Ordinarily that is everything
    /// it draws; a **net run** is a stretch of wire rather than a body, so it
    /// reserves its trace and its stepped-off name alone [SPEC 16.4].
    fn extent(&self, children: &[PlacedNode], i: usize) -> Bbox {
        if net::is_run(&children[i]) {
            return net::run_extent(&children[i], self.text[i]);
        }
        drawn(&children[i])
    }

    /// Once the anchors are placed: land every seated satellite in scene
    /// coordinates. A pin-relative seat rides its anchor; a spanning chain
    /// reads both placed ends now that they exist.
    pub(super) fn absolutize(&self, children: &mut [PlacedNode]) {
        // A net run's name steps off its own trace [SPEC 16.4] — the decision
        // was struck when the run was seated; this applies it.
        for (i, &(dx, dy)) in self.text.iter().enumerate() {
            if (dx, dy) == (0.0, 0.0) {
                continue;
            }
            for c in children[i].children.iter_mut() {
                c.cx += dx;
                c.cy += dy;
            }
        }
        for (i, seat) in self.seats.iter().enumerate() {
            let Some(seat) = seat else { continue };
            let held = &children[seat.anchor];
            let (cx, cy) = (held.cx + seat.dx, held.cy + seat.dy);
            children[i].cx = cx;
            children[i].cy = cy;
        }
        for span in &self.spanning {
            let point = |(child, t): &(usize, Terminal)| {
                let n = &children[*child];
                (n.cx + t.at.0, n.cy + t.at.1)
            };
            let (a, b) = (point(&span.ends[0]), point(&span.ends[1]));
            let steps = span.members.len() + 1;
            for (i, &member) in span.members.iter().enumerate() {
                let f = (i + 1) as f64 / steps as f64;
                let (x, y) = (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f);
                let (bx, by) = children[member].bbox.center();
                children[member].cx = x - bx;
                children[member].cy = y - by;
            }
        }
    }

    /// The satellites no wire holds in place [SPEC 16.1] — the caller flows
    /// them and warns.
    pub(super) fn floating(&self) -> &[usize] {
        &self.floating
    }

    /// The placed extent of everything seated **between** two anchors — the
    /// spanning chains, which ride no cluster and so no track. The caller
    /// unions it into the scope's box, so the sheet still holds all its ink.
    pub(super) fn spanning_extent(&self, children: &[PlacedNode]) -> Option<Bbox> {
        self.spanning
            .iter()
            .flat_map(|s| &s.members)
            .map(|&m| drawn(&children[m]).shifted(children[m].cx, children[m].cy))
            .reduce(|a, b| a.union(b))
    }

    /// What the spanning chains ask of the tracks [SPEC 16.1], one [`Demand`]
    /// each, in chain order.
    pub(super) fn demands(&self, children: &[PlacedNode]) -> Vec<Demand> {
        self.spanning
            .iter()
            .map(|s| Demand {
                ends: [(s.ends[0].0, s.ends[0].1.at), (s.ends[1].0, s.ends[1].1.at)],
                need: (
                    step(&s.members, children, Bbox::w, self.seat),
                    step(&s.members, children, Bbox::h, self.seat),
                ),
            })
            .collect()
    }
}

/// What one spanning chain asks of the tracks [SPEC 16.1]. Its satellites are
/// struck at even fractions of the pin-to-pin line, so that line has to be long
/// enough for consecutive seats — and the two pins themselves — to clear by a
/// seat gap. Stated here as a distance and the two landings it is measured
/// between; the track arithmetic is [`super::place`]'s.
pub(super) struct Demand {
    /// Each end's anchor, and the landing's offset in that anchor's own frame.
    pub ends: [(usize, (f64, f64)); 2],
    /// The least pin-to-pin distance along x, along y.
    pub need: (f64, f64),
}

/// The least pin-to-pin distance one axis needs: `n` members seat a `1/(n+1)`
/// step apart, and the step is set by the greediest neighbouring pair — half of
/// each extent, plus the seat gap. The end steps answer to one half extent
/// only, the pin being a point.
fn step(
    members: &[usize],
    children: &[PlacedNode],
    extent: impl Fn(&Bbox) -> f64,
    seat: f64,
) -> f64 {
    let e: Vec<f64> = members
        .iter()
        .map(|&m| extent(&drawn(&children[m])))
        .collect();
    let (Some(&first), Some(&last)) = (e.first(), e.last()) else {
        return 0.0;
    };
    let mut step = first.max(last) / 2.0;
    for pair in e.windows(2) {
        step = step.max((pair[0] + pair[1]) / 2.0);
    }
    (e.len() + 1) as f64 * (step + seat)
}

/// Every chain's **lane** [SPEC 16.1] — how far out along its pin's own normal
/// it stands before turning onto its growth ray — one per entry of `held`, in
/// that order.
///
/// **A lane per chain, within a ray.** A chain reaches back toward the part from
/// its own lane, so sharing one lane stands a flag's body over its neighbour's
/// leg — and the router, which may not cross a body, jogs that leg into a
/// staircase rather than the one square turn a sheet draws. Each chain
/// therefore steps out past the whole reach of the one before it, in the order
/// `held` arrives in (see [`Seats::build`]). The step only ever pushes a lane
/// **outward**, so the walk is monotone and its bound is a backstop, never a
/// cutoff.
///
/// A chain growing straight out along its own pin (`lead == 0`) has no lane to
/// take: it never turns, so there is nothing to ladder.
fn ladder(children: &[PlacedNode], held: &[Growing], seat: f64) -> Vec<f64> {
    // Everything below is in **outward** coordinates — the lane axis times the
    // pin's own outward sign — so "farther out" is always larger.
    let lead: Vec<f64> = held.iter().map(|g| g.lead).collect();
    // A straight-growing chain's lane *is* its pin, and it takes no part in the
    // ladder; only a turning one is carried in outward coordinates below.
    let mut along: Vec<f64> = Vec::with_capacity(held.len());
    let mut out: Vec<f64> = Vec::with_capacity(held.len());
    let mut back: Vec<f64> = Vec::with_capacity(held.len());
    for (g, &lead) in held.iter().zip(&lead) {
        let (lane, reach) = clearing(children, g, lead, seat);
        along.push(lane);
        out.push(lead * lane);
        back.push(reach);
    }
    for _ in 0..=held.len() {
        let mut moved = false;
        // Ladder: within one ray, each chain steps past its predecessor's reach.
        let mut prev: Option<(usize, f64)> = None;
        for (i, g) in held.iter().enumerate() {
            if lead[i] == 0.0 {
                continue;
            }
            if let Some((group, edge)) = prev
                && group == g.group
                && out[i] < edge
            {
                out[i] = edge;
                moved = true;
            }
            prev = Some((g.group, out[i] + back[i] + seat));
        }
        // Share: a pin whose net branches **both** ways — a rail up to its
        // flag, down to its decoupling cap — leaves on one lead and splits
        // **once**, at one point, rather than peeling twice off its stub. So
        // chains parting onto **different rays** from one pin take the
        // outermost lane any of them asked for; the wires run co-linearly out
        // to that point, which the router draws as one lead (an implicit fan
        // on one fixed port — [ROUTING.md](../../../ROUTING.md) Special nodes
        // / Fixed ports). Chains taking the **same** ray never share: they
        // ladder side by side like any two chains — equalizing them while the
        // ladder steps one past the other is a feedback loop that walks the
        // pair out a step per round until the iteration bound.
        for i in 0..held.len() {
            for j in 0..held.len() {
                if i != j
                    && lead[i] != 0.0
                    && lead[j] != 0.0
                    && held[i].held.child == held[j].held.child
                    && held[i].pin == held[j].pin
                    && held[i].ray != held[j].ray
                    && out[i] < out[j]
                {
                    out[i] = out[j];
                    moved = true;
                }
            }
        }
        if !moved {
            break;
        }
    }
    for (i, &lead) in lead.iter().enumerate() {
        if lead != 0.0 {
            along[i] = lead * out[i];
        }
    }
    along
}

/// What one chain asks of its lane before any other is consulted [SPEC 16.1]:
/// the innermost lane clearing the part it hangs from, and how far back toward
/// that part its widest member reaches from the lane — the ink the next lane
/// out has to clear. Both in outward coordinates.
///
/// **The seat gap answers to the connection geometry, the ink only to overlap.**
/// A part stands one seat gap off the wall measured on what a wire actually
/// arrives at — a flag's symbol, a discrete's body — and its *annotation* (the
/// name beside a symbol, a ref or value readout) merely may not reach back over
/// the part. Charging the seat gap on the annotation too would make a chain's
/// lane a function of how long its name reads: on a connector wired up on one
/// side and down on the other, `VM` and a bare ground would stand off by
/// visibly different amounts, lopsided for no reason a reader can see. A name
/// long enough to actually reach the part still pushes its own lane out, which
/// is the case the clearance exists for.
fn clearing(children: &[PlacedNode], g: &Growing, lead: f64, seat: f64) -> (f64, f64) {
    let frame = Frame::outward(g.ray.normal());
    let straight = frame.u(g.pin) + lead * seat;
    if lead == 0.0 {
        return (straight, 0.0);
    }
    let wall = {
        let (lo, hi) = project(drawn(&children[g.held.child]), frame.u);
        lead * if lead > 0.0 { hi } else { lo }
    };
    // How far a box reaches back toward the part from the member's landing.
    let reach = |box_: Bbox, at: (f64, f64)| {
        let (u0, u1) = (frame.u(corner(box_, false)), frame.u(corner(box_, true)));
        lead * (frame.u(at) - if lead > 0.0 { u0.min(u1) } else { u0.max(u1) })
    };
    let (mut ink, mut connection) = (0.0f64, 0.0f64);
    for (&member, inbound) in g.chain.members.iter().zip(&g.chain.inbound) {
        let node = &children[member];
        let at = terminal(node, inbound.as_deref()).at;
        ink = ink.max(reach(drawn(node), at));
        connection = connection.max(reach(connection_box(node), at));
    }
    let out = (lead * straight)
        .max(wall + seat + connection)
        .max(wall + ink);
    (lead * out, ink)
}

/// Where a one-held chain grows **from** and **toward** [SPEC 16.1]: the pin
/// that holds it, and the ray it runs along — away from the terminator's own
/// connection geometry, else straight out along the pin.
///
/// One home for the ray, because two passes need it before anything is seated:
/// [`Seats::grow`] lays the chain along it, and [`Seats::build`] sorts the
/// chains sharing it into arrival order first.
fn growth(children: &[PlacedNode], chain: &Chain, held: &End) -> (Side, Terminal) {
    let pin = terminal(&children[held.child], held.terminal.as_deref());
    let last = *chain.members.last().expect("a chain has a member");
    let out = tag_facing(
        &children[last],
        chain.inbound.last().and_then(|t| t.as_deref()),
    )
    // A part terminator has no drawn convention to read — its pins are just
    // pins — so its chain runs along the pin's own outward normal; a text
    // label carries no connection geometry either. With neither (a wire to a
    // plain box) the chain hangs below, the one direction a sheet always has
    // room in.
    .map_or_else(|| pin.facing.unwrap_or(Side::Bottom), Side::opposite);
    (out, pin)
}

/// The direction a chain's **terminator** sets [SPEC 16.1] — the way its own
/// drawing points. Only a `|label|` carries that convention: a ground is drawn
/// with its point at the top, a power flag with its at the bottom, and the
/// sheet reads the symbol rather than a table of names. A part's pins are just
/// pins, so a part-terminated chain has nothing to say here and its caller
/// falls back to the pin's own normal.
///
/// Shared with the pose chooser, which decides the same ray one stage earlier
/// ([`crate::desugar::autopose`]).
fn tag_facing(node: &PlacedNode, inbound: Option<&str>) -> Option<Side> {
    (crate::desugar::schematic::sch_kind(&node.type_chain)
        == Some(crate::desugar::schematic::SchKind::Label))
    .then(|| terminal(node, inbound).facing)
    .flatten()
}

/// A part's **drawn** extent: its box unioned with every descendant, so the
/// chrome that pokes outside counts. A ref or a value readout is a `pin:`
/// overlay — out of the flow that sized the box, but ink on the sheet and an
/// obstacle to the router all the same (the scene index carries it as the
/// part's `overflow`). Seating measures this, or a chain clears a part's box
/// and lands on its readout.
pub(super) fn drawn(node: &PlacedNode) -> Bbox {
    Bbox::drawn_of(node)
}

/// A box corner, `min` or `max` — the two the frame projections need.
fn corner(b: Bbox, max: bool) -> (f64, f64) {
    if max {
        (b.max_x, b.max_y)
    } else {
        (b.min_x, b.min_y)
    }
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

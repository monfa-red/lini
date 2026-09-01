//! The **satellite seat pass** [SPEC 16.1] — where the parts that ride no
//! track go.
//!
//! A satellite chain reads its wire:
//!
//! - **one placed end** — it grows outward from that pin, monotone, link by
//!   link, along the ray the one shared rule states
//!   ([`crate::desugar::schematic::chain::growth_ray`]); auto-pose has
//!   already turned each satellite to face back up that ray
//!   ([`crate::desugar::autopose`]), and an authored `rotate:` forces the
//!   pose with the seat following the turned connection point. A **tap** —
//!   a symbol-label leaf hanging mid-chain
//!   ([`crate::desugar::schematic::chain::taps`]) — takes no slot in the
//!   stack and hangs off its attachment member along its own posed drawing.
//! - **two placed ends on one anchor** — a **bridge** (`U2.EN - R5 -
//!   U2.VIN`): it grows like a one-end chain off the first-named pin — the
//!   member stands in that pin's own corridor, entry terminal end-on — and
//!   the far wire is the router's, which merges it into the second pin's
//!   net at a junction ([ROUTING.md](../../../ROUTING.md) Fixed ports), the
//!   way a sheet taps a pull-up into the line it feeds. The split from a
//!   spanning chain is [`crate::desugar::schematic::chain::holder`]'s,
//!   shared with the chooser.
//! - **two placed ends, on two anchors** — a **span**: its satellites ride
//!   the wire's landing leg (the second pin's row or column), seated **off
//!   that landing** a seat gap at a time — the next columns of the ladder
//!   that end's own satellites take their lanes on.
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
//! the space it lands in**, asking the two tracks it spans to part far
//! enough for the members packed end to end, and for the stretch of leg each
//! end's cluster swallows, to clear ([`Demand`], struck in [`super::place`]).
//! Within its window it stands clear by construction; it still enters no
//! anchor's [`Stack`], so a pathological weave of spans and seats can
//! overlap — the router then reports what it cannot lawfully draw.

use std::cmp::Ordering;

use super::super::geom::{Frame, dot, project};
use super::super::ir::{Bbox, PlacedNode};
use super::super::stack::{Band, Painted, SeatLine, Stack};
use super::net;
use super::terminal::{Terminal, connection_box, terminal};
use crate::desugar::pose::Side;
use crate::desugar::schematic::Role;
use crate::desugar::schematic::chain::{Chain, End, chains, holder, placed_ends};
use crate::ledger::consts::DEFAULT_CLEARANCE;
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
    /// Whether this is the **first** chain (statement order) taking this ray
    /// off this pin — the one the lane share may pair across rays
    /// ([`ladder`]): a junction splits once, so one up-chain rides one
    /// down-chain's lane and every later same-ray chain ladders out.
    ray_first: bool,
    chain: Chain,
    held: End,
}

/// What [`Seats::grow`] reads beside the chain itself: the scope's wire
/// edges (tap attachment lookups) and the held anchor's wired-row
/// corridors.
struct GrowCx<'a> {
    edges: &'a [[End; 2]],
    rows: &'a [((f64, f64), Painted)],
}

/// One stack's rail — trunk or branch: the frame it grows in, the lane it
/// runs along, the anchor its seats belong to, and the corridors its
/// members clear ([`Seats::seat_one`]).
struct SubStack<'a> {
    frame: Frame,
    along: f64,
    anchor: usize,
    corridors: &'a [Painted],
}

/// A member's painted reach either side of its landing, in `frame` — the
/// band every seat is measured by.
fn band_of(frame: Frame, box_: Bbox, point: (f64, f64)) -> Band {
    let (lo, hi) = (
        frame.cross(corner(box_, false)),
        frame.cross(corner(box_, true)),
    );
    let c = frame.cross(point);
    Band {
        neg: c - lo.min(hi),
        pos: hi.max(lo) - c,
    }
}

/// A chain held at both ends: its satellites pack off the second terminal's
/// landing once the anchors are placed.
struct Spanning {
    members: Vec<usize>,
    ends: [(usize, Terminal); 2],
}

/// One ladder's **rhythm** [SPEC 16.1] — the pitch its columns step on, and
/// where the column after its last would stand. A chain no anchor holds rides
/// no lane of its own, so a **span** landing on this side reads its rhythm off
/// this and carries the ladder on ([`Seats::absolutize`]).
///
/// `next` is an **out-coordinate**: the lane's distance along the side's own
/// outward normal, in the anchor's own frame, exactly as [`ladder`] works.
#[derive(Clone, Copy)]
struct Rung {
    anchor: usize,
    side: Side,
    next: f64,
    pitch: f64,
}

/// Every satellite's placement decision, made before the tracks size.
pub(super) struct Seats {
    seats: Vec<Option<Seat>>,
    spanning: Vec<Spanning>,
    /// Each lane ladder's rhythm, for the spans that continue it ([`Rung`]).
    rungs: Vec<Rung>,
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

/// The seat floor. It left the ledger with the lattice [SPEC 16.1], which
/// states the distance now, and stands here until the field pass replaces
/// this module.
pub(super) const LABEL_SEAT: f64 = 25.0;

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
    /// `children` is mutable for one adjustment only: a turned member's
    /// readouts mirror onto its lane's outward side before any extent is
    /// read (see the flip below); every seat is still returned, never
    /// applied here.
    pub(super) fn build(
        children: &mut [PlacedNode],
        roles: &[Role],
        links: &[&ResolvedLink],
        scope: &str,
        seat: f64,
    ) -> Seats {
        let satellite: Vec<bool> = roles.iter().map(|r| *r == Role::Satellite).collect();
        let mut out = Seats {
            seats: (0..children.len()).map(|_| None).collect(),
            spanning: Vec::new(),
            rungs: Vec::new(),
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
        let mut mirrored: Vec<usize> = Vec::new();
        let mut restacked: Vec<(usize, f64)> = Vec::new();
        // The lanes seen so far — an (anchor, **pin side**) pair. One ladder
        // per side of a part, not per growth ray: every chain turning off
        // that side leaves along the one lane axis, so an up-chain and a
        // down-chain off two different pins land in the same column unless
        // they ladder together — the drain's flag standing over the source's
        // return, one broken line where a reader sees a short. Two sides are
        // still two ladders (a chain turning left off a left pin and one
        // turning right off a right pin grow on opposite sides of the part,
        // where their leads can never cross), and a straight-growing chain
        // (`lead == 0`) grows along its own pin, so its side *is* its ray.
        // The lanes keep the order they were declared in and only the chains
        // within one are reordered.
        let mut rays: Vec<(usize, Side)> = Vec::new();
        let wire_edges = edges(children, links, scope);
        // Every **wired** pin's row is a corridor [SPEC 16.1] — the wire off
        // it runs there, to another part or out to its chain's own column.
        // Each is a zero-height band reaching out over the pin's own side; a
        // chain seats clear of every one but its own pin's ([`corridors`]),
        // so no member's body lands on a foreign corridor and the wires
        // cross a column's lead square instead — the decoupling cap seats
        // below the A/B pair it once forced into a weave, and a divider's
        // ascent clears the VCC trunk above it.
        let mut rows: Vec<Vec<((f64, f64), Painted)>> =
            (0..children.len()).map(|_| Vec::new()).collect();
        for end in wire_edges.iter().flatten() {
            if roles[end.child] != Role::Anchor {
                continue;
            }
            let t = terminal(&children[end.child], end.terminal.as_deref());
            let Some(f) = t.facing else { continue };
            if rows[end.child].iter().any(|(at, _)| *at == t.at) {
                continue;
            }
            let big = 1e4;
            let (x, y) = t.at;
            let band = match f {
                Side::Left => Bbox {
                    min_x: -big,
                    max_x: x,
                    min_y: y,
                    max_y: y,
                },
                Side::Right => Bbox {
                    min_x: x,
                    max_x: big,
                    min_y: y,
                    max_y: y,
                },
                Side::Top => Bbox {
                    min_x: x,
                    max_x: x,
                    min_y: -big,
                    max_y: y,
                },
                Side::Bottom => Bbox {
                    min_x: x,
                    max_x: x,
                    min_y: y,
                    max_y: big,
                },
            };
            rows[end.child].push((t.at, Painted::of_box(band)));
        }
        for chain in chains(&satellite, &wire_edges) {
            let ends = placed_ends(&chain, roles);
            // One anchor holds it → grow off that pin; two → span between them;
            // none → the flow fallback. Which of the first two a chain is, is
            // [`holder`]'s single answer, shared with the pose chooser.
            match (holder(&ends), ends.as_slice()) {
                (Some(one), _) => {
                    let one = one.clone();
                    let (ray, pin) = growth(children, &chain, &one, &wire_edges);
                    let frame = Frame::outward(ray.normal());
                    let depth = frame.cross(pin.at);
                    let lead = frame.u(pin.facing.map_or((0.0, 0.0), Side::normal));
                    let key = (one.child, pin.facing.unwrap_or(ray));
                    let group = rays.iter().position(|r| *r == key).unwrap_or_else(|| {
                        rays.push(key);
                        rays.len() - 1
                    });
                    let ray_first = !held
                        .iter()
                        .any(|g| g.held == one && g.pin == pin.at && g.ray == ray);
                    if lead != 0.0
                        && matches!(ray, Side::Top | Side::Bottom)
                        && pin.facing == Some(Side::Left)
                    {
                        mirrored.extend(&chain.members);
                    }
                    // A corridor member's readouts straddle its wire; with a
                    // live pin row one pitch away, the near line closes that
                    // row's corridor (clearance outreaches the row gap) and
                    // the wire off the member's far terminal orbits. When
                    // the rows crowd one side only, both lines step to the
                    // free side.
                    if lead == 0.0 && ray.is_vertical() {
                        let rows = pin_rows(&children[one.child], ray);
                        let above = rows.iter().any(|&r| r < pin.at.1 - 1e-6);
                        let below = rows.iter().any(|&r| r > pin.at.1 + 1e-6);
                        if above != below {
                            let sgn = if above { 1.0 } else { -1.0 };
                            restacked.extend(chain.members.iter().map(|&m| (m, sgn)));
                        }
                    }
                    held.push(Growing {
                        group,
                        ray,
                        pin: pin.at,
                        depth,
                        lead,
                        ray_first,
                        chain,
                        held: one,
                    });
                }
                (None, [a, b, ..]) => out.distribute(children, chain, a, b),
                (None, _) => out.floating.extend(chain.members),
            }
        }
        // A turned member's readouts are minted on the sheet's reading side
        // (+x, [SPEC 16.2]); on a chain turning off a **left**-facing pin
        // that side reaches back over the lane toward the part, where every
        // deeper pin threads its wires. Mirror them onto the lane's outward
        // side — decided here because only the seat pass knows the lane,
        // exactly as a net run's name takes the freer side [SPEC 16.4] —
        // and before any extent below is read, so the ladder, the cluster
        // and the router all see the mirrored ink.
        for &m in &mirrored {
            for c in children[m].children.iter_mut() {
                if c.type_chain.iter().any(|t| t == "ref" || t == "part-value") {
                    c.cx = -c.cx - (c.bbox.min_x + c.bbox.max_x);
                }
            }
        }
        for &(m, sgn) in &restacked {
            for c in children[m].children.iter_mut() {
                let s = c.cy.abs();
                if c.type_chain.iter().any(|t| t == "ref") {
                    c.cy = sgn * if sgn > 0.0 { s } else { 3.0 * s };
                } else if c.type_chain.iter().any(|t| t == "part-value") {
                    c.cy = sgn * if sgn > 0.0 { 3.0 * s } else { s };
                }
            }
        }
        // **No chain overtakes another** [SPEC 16.1] — the routing contract's
        // own rule for the runs in a channel ("wires leave in the order they
        // arrive — nested, never braided", ROUTING.md model step 5), asked one
        // pass earlier of the parts those wires will join. Which end of the
        // order that is depends on the axis a chain competes for:
        //
        // - a chain that **turns** onto its ray competes for a lane, and the
        //   one off the *shallower* pin takes the *outer* lane, or its turn
        //   cuts through a deeper chain's leg. Deepest first — with depth
        //   measured along the **canonical** direction of the ray's axis
        //   (down, right), not along the ray itself: a pin's up-chain and its
        //   down-chain share one column (the lane share below), so the
        //   columns of two pins must take *one* order, or the up and down
        //   ladders demand opposite ones and the pair has no lawful lanes at
        //   all (the 2×2 divider: two pins, each a rail up and a return
        //   down). The bottom pin's column sits innermost, both ways.
        // - a chain that grows **straight** out along its pin takes no lane and
        //   stacks in depth instead, where the stack only ever pushes outward —
        //   so the shallowest keeps its own pin's depth and the rest pass it.
        //   Shallowest first, along its own ray.
        //
        // A stable sort, so chains sharing a ray *and* a pin keep their
        // statement order, which is the only thing left to break the tie.
        let canon = |g: &Growing| match g.ray {
            Side::Bottom | Side::Right => g.depth,
            Side::Top | Side::Left => -g.depth,
        };
        // One ray, or two? A pin's up-chain and its down-chain share a
        // column, so a side holding **both** rays must order its columns one
        // way for both — and only the canonical direction (down, right) is
        // one the two ladders can each read, the bottom pin's column
        // innermost whichever way its chains grow. A side growing **one**
        // way has no such tie and takes the order its own leg-crossing law
        // asks for: a chain's leg crosses every lane inside its own, so the
        // one whose pin sits *earlier* along the ray steps out, and the
        // deeper one keeps the inner lane. Read canonically that is right
        // for a downward side and backwards for an upward one — the fan
        // header's rail flag laddering outside the tach column it then had
        // to cross.
        let mut mixed: Vec<bool> = vec![false; rays.len()];
        let mut seen: Vec<Option<Side>> = vec![None; rays.len()];
        for g in held.iter().filter(|g| g.lead != 0.0) {
            match seen[g.group] {
                None => seen[g.group] = Some(g.ray),
                Some(r) => mixed[g.group] |= r != g.ray,
            }
        }
        // Straight chains seat **first**, whatever their declaration: they
        // are the inner geography — members lying in their own pins'
        // corridors — and the turning chains' columns ladder out past them
        // ([`ladder`]'s stack floor). Seated the other way round, a column
        // paints first and the stack's probe, grazing it, leaps the whole
        // column instead of keeping its natural seat.
        held.sort_by(|a, b| {
            (a.lead != 0.0)
                .cmp(&(b.lead != 0.0))
                .then(a.group.cmp(&b.group))
                .then(if a.lead == 0.0 {
                    a.depth.total_cmp(&b.depth)
                } else if mixed[a.group] {
                    canon(b).total_cmp(&canon(a))
                } else {
                    b.depth.total_cmp(&a.depth)
                })
        });
        let (lanes, rungs) = ladder(children, &held, &rays, seat);
        out.rungs = rungs;
        for (g, along) in held.iter().zip(lanes) {
            let frame = Frame::outward(g.ray.normal());
            out.grow(
                children,
                g,
                frame,
                along,
                &mut packers[g.held.child],
                GrowCx {
                    edges: &wire_edges,
                    rows: &rows[g.held.child],
                },
            );
        }
        out
    }

    /// A one-placed-end chain: its **trunk** members seat farther out along
    /// the growth ray, each one's connection point landing where the packer
    /// clears; a **tap** hangs off its attachment member, and every other
    /// **branch** grows from its junction as a sub-chain
    /// ([`crate::desugar::schematic::chain::limbs`]).
    fn grow(
        &mut self,
        children: &[PlacedNode],
        g: &Growing,
        frame: Frame,
        along: f64,
        stack: &mut Stack,
        cx: GrowCx,
    ) {
        let (chain, held) = (&g.chain, &g.held);
        let tap = tap_flags(children, chain);
        let limbs = crate::desugar::schematic::chain::limbs(chain);
        let wire_edges = cx.edges;
        // The anchor's wired corridors, its own pin's aside: what this
        // chain's members seat clear of, without walling in the chains that
        // *live* on their own pin's row.
        let corridors: Vec<Painted> = cx
            .rows
            .iter()
            .filter(|(at, _)| *at != g.pin)
            .map(|(_, p)| *p)
            .collect();
        // The chain hangs off the wire's **first leg** — out along the pin to
        // its own lane ([`Self::lane`]) — and grows from there in its
        // terminator's direction [SPEC 16.1]: a cap under a side pin sits
        // below the wire leaving it, not inside the component's own column.
        //
        // Growth is **monotone**: each member's base is the previous member's
        // outer paint edge, so a later member can never tuck into a hole a
        // foreign band opened before an earlier one — "link by link" is an
        // order, not just a distance.
        let rail = SubStack {
            frame,
            along,
            anchor: held.child,
            corridors: &corridors,
        };
        let mut base = frame.cross(g.pin);
        for (i, (&member, inbound)) in chain.members.iter().zip(&chain.inbound).enumerate() {
            if limbs[i].is_some() {
                continue;
            }
            base = self.seat_one(children, &rail, stack, base, member, inbound.as_deref());
        }
        // Taps hang off their attachment member's terminal, one seat out
        // along the ray their **posed** drawing states — the pose chooser
        // decided it ([`crate::desugar::schematic::chain::tap_ray`]) and the
        // turned connection point carries it, so this pass only reads
        // geometry, exactly as it does for the trunk's ray.
        for (i, &member) in chain.members.iter().enumerate() {
            if !tap[i] {
                continue;
            }
            let Some(attach) = self.attach_of(children, chain, wire_edges, i) else {
                continue;
            };
            let t = terminal(&children[member], chain.inbound[i].as_deref());
            let ray = t.facing.map_or(g.ray, Side::opposite);
            // A tap whose own convention points back into the trunk **steps
            // aside** [SPEC 16.1] — and stays upright: it seats one gap out
            // along the aside ray *and* one along its own, so its lead is
            // the router's one square corner, the way a sheet stands a flag
            // beside the junction it taps.
            let pin_facing = terminal(&children[held.child], held.terminal.as_deref()).facing;
            let aside = crate::desugar::schematic::chain::tap_ray(t.facing, g.ray, pin_facing);
            if aside != ray {
                let tf = Frame::outward(aside.normal());
                let band = band_of(tf, drawn(&children[member]), t.at);
                // Risen one gap along its own ray, packed out along the
                // aside — the interval is read at the risen height.
                let rn = ray.normal();
                let risen = (attach.0 + rn.0 * self.seat, attach.1 + rn.1 * self.seat);
                let box_ = drawn(&children[member]);
                let (u0, u1) = (tf.u(corner(box_, false)), tf.u(corner(box_, true)));
                let (au, u) = (tf.u(risen), tf.u(t.at));
                let interval = (au + u0.min(u1) - u, au + u0.max(u1) - u);
                let line = stack.seat(
                    SeatLine::new(tf, true, tf.cross(attach)),
                    interval,
                    self.seat,
                    &band,
                    &corridors,
                );
                let target = tf.pt(au, line);
                self.seats[member] = Some(Seat {
                    anchor: held.child,
                    dx: target.0 - t.at.0,
                    dy: target.1 - t.at.1,
                });
                continue;
            }
            let tf = Frame::outward(ray.normal());
            let band = band_of(tf, drawn(&children[member]), t.at);
            let box_ = drawn(&children[member]);
            let (u0, u1) = (tf.u(corner(box_, false)), tf.u(corner(box_, true)));
            let (au, u) = (tf.u(attach), tf.u(t.at));
            let interval = (au + u0.min(u1) - u, au + u0.max(u1) - u);
            let line = stack.seat(
                SeatLine::new(tf, true, tf.cross(attach)),
                interval,
                self.seat,
                &band,
                &corridors,
            );
            let target = tf.pt(au, line);
            self.seats[member] = Some(Seat {
                anchor: held.child,
                dx: target.0 - t.at.0,
                dy: target.1 - t.at.1,
            });
        }
        // A multi-member **branch** grows from its junction as a sub-chain
        // [SPEC 16.1]: along its own terminator's ray — the trunk's when it
        // states none — monotone through the same packer. When that ray
        // runs on the trunk's own axis, the branch's lane first steps
        // **beside** the trunk, carried sideways until its root stands
        // clear of everything painted, so the two columns descend side by
        // side instead of interleaving into one.
        for r in 0..chain.members.len() {
            if limbs[r] != Some(r) || tap[r] {
                continue;
            }
            let members_b: Vec<usize> = (0..chain.members.len())
                .filter(|&i| limbs[i] == Some(r))
                .collect();
            let Some(attach) = self.attach_of(children, chain, wire_edges, r) else {
                continue;
            };
            let &last = members_b.last().expect("a branch holds its root");
            let ray = tag_facing(
                &children[chain.members[last]],
                chain.inbound[last].as_deref(),
            )
            .map_or(g.ray, Side::opposite);
            let bf = Frame::outward(ray.normal());
            let mut along_b = bf.u(attach);
            let base_b = bf.cross(attach);
            if ray == g.ray || ray == g.ray.opposite() {
                let pin_facing = terminal(&children[held.child], held.terminal.as_deref()).facing;
                let aside = crate::desugar::schematic::chain::beside(g.ray, pin_facing);
                let root = chain.members[r];
                let point = terminal(&children[root], chain.inbound[r].as_deref()).at;
                let box_ = drawn(&children[root]);
                let band = band_of(bf, box_, point);
                let naive = bf.pt(along_b, base_b + self.seat + band.neg);
                let an = aside.normal();
                let t = stack.clear(
                    box_.shifted(naive.0 - point.0, naive.1 - point.1),
                    an,
                    self.seat,
                );
                along_b += bf.u(an) * t;
            }
            let rail = SubStack {
                frame: bf,
                along: along_b,
                anchor: held.child,
                corridors: &corridors,
            };
            let mut base = base_b;
            for &i in &members_b {
                base = self.seat_one(
                    children,
                    &rail,
                    stack,
                    base,
                    chain.members[i],
                    chain.inbound[i].as_deref(),
                );
            }
        }
    }

    /// Seat one member of a stack — trunk or branch, the same arithmetic —
    /// at the innermost line the packer clears along `rail`, from `base`;
    /// records the seat and returns the next base (the member's outer paint
    /// edge, so growth stays monotone).
    fn seat_one(
        &mut self,
        children: &[PlacedNode],
        rail: &SubStack,
        stack: &mut Stack,
        base: f64,
        member: usize,
        inbound: Option<&str>,
    ) -> f64 {
        let frame = rail.frame;
        let sat = &children[member];
        let point = terminal(sat, inbound).at;
        // The band is the satellite's own reach either side of the point
        // its wire lands on, so the packer measures from the connection.
        let band = band_of(frame, drawn(sat), point);
        // A **net run**'s name steps off the trace here [SPEC 16.4]: the
        // run's middle is `band.neg / 2` back from the innermost landing,
        // and the freer side is read against what this anchor has already
        // painted. The step is across the growth ray, so the band above is
        // untouched and only the run's own reach changes.
        if net::is_run(sat) {
            let mid = frame.pt(rail.along, base + self.seat + band.neg / 2.0);
            self.text[member] = net::seat_text(sat, mid, stack.painted());
        }
        let box_ = self.extent(children, member);
        let (u0, u1) = (frame.u(corner(box_, false)), frame.u(corner(box_, true)));
        let u = frame.u(point);
        let interval = (rail.along + u0.min(u1) - u, rail.along + u0.max(u1) - u);
        let line = stack.seat(
            SeatLine::new(frame, true, base),
            interval,
            self.seat,
            &band,
            rail.corridors,
        );
        let target = frame.pt(rail.along, line);
        self.seats[member] = Some(Seat {
            anchor: rail.anchor,
            dx: target.0 - point.0,
            dy: target.1 - point.1,
        });
        line + band.pos
    }

    /// A branch's attachment: the junction terminal on its trunk parent, at
    /// the parent's **seated** position — where the branch's own wire
    /// leaves the trunk. `None` while the parent has no seat (a foreign
    /// walk order), which leaves the member to the flow fallback's mercy —
    /// never a panic.
    fn attach_of(
        &self,
        children: &[PlacedNode],
        chain: &Chain,
        wire_edges: &[[End; 2]],
        i: usize,
    ) -> Option<(f64, f64)> {
        let parent = chain.parents[i]?;
        let parent_child = chain.members[parent];
        let pseat = self.seats[parent_child].as_ref()?;
        // The wire's terminal on the attachment side — read off the edge
        // that joins the pair, since a chain records only inbound ends.
        let mine = End {
            child: chain.members[i],
            terminal: chain.inbound[i].clone(),
        };
        let outbound = wire_edges.iter().find_map(|[a, b]| {
            if *b == mine && a.child == parent_child {
                Some(a.terminal.clone())
            } else if *a == mine && b.child == parent_child {
                Some(b.terminal.clone())
            } else {
                None
            }
        });
        let junction = terminal(&children[parent_child], outbound.flatten().as_deref()).at;
        Some((junction.0 + pseat.dx, junction.1 + pseat.dy))
    }

    /// A chain held at both ends seats off the **second** end's landing, as
    /// the next columns of the ladder that pin's own satellites take their
    /// lanes on [SPEC 16.1]. Only the two terminals are read here; the leg
    /// itself is not known until the anchors place, so the seats are struck
    /// in [`Self::absolutize`].
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
            // The members sit on the wire's **landing leg** — the straight
            // run into the second end, on that pin's own row or column
            // [SPEC 16.1] — never on the raw pin-to-pin diagonal, which cuts
            // across the sheet (and, off an away-facing pin, across its own
            // part). And they seat **off that landing**: the leg runs along
            // the very axis the second end's own satellites ladder their
            // lanes on, so the members are that ladder's next columns and
            // step on its own pitch ([`Rung`]). Split evenly over the whole
            // clear stretch they drift instead into its middle — a length
            // nobody authored, being whatever the tracks happened to part by
            // — and the blank left between them and the part they feed reads
            // as a column of nothing.
            let (a, b) = match span.ends[1].1.facing {
                // A pin on a vertical side faces horizontally: its landing
                // leg runs on its row, and the members ride that row.
                Some(f) if f.is_vertical() => ((a.0, b.1), b),
                Some(_) => ((b.0, a.1), b),
                None => (a, b),
            };
            let clear = |end: &(usize, Terminal), from: (f64, f64), to: (f64, f64)| {
                let n = &children[end.0];
                let r = self
                    .cluster(children, end.0)
                    .shifted(n.cx, n.cy)
                    .inflate(self.seat);
                exit_t(from, (to.0 - from.0, to.1 - from.1), r)
            };
            // How much of the leg each end's cluster swallows before the clear
            // window opens — each already a seat gap clear of that cluster's
            // ink — and the sign the leg leaves the landing at `b` by.
            let len = ((b.0 - a.0).abs() + (b.1 - a.1).abs()).max(1e-9);
            let horiz = (b.1 - a.1).abs() < (b.0 - a.0).abs();
            let out = if horiz {
                (a.0 - b.0).signum()
            } else {
                (a.1 - b.1).signum()
            };
            let swallow0 = clear(&span.ends[0], a, b) * len;
            let swallow1 = clear(&span.ends[1], b, a) * len;
            let reach: Vec<(f64, f64)> = span
                .members
                .iter()
                .map(|&m| member_reach(children, m, horiz))
                .collect();
            let (at, far) = march(&reach, out, swallow1, self.rung(&span.ends[1]), self.seat);
            // The leg's slack lies where the wire comes **in**. A window too
            // short to hold the members centres them on the raw leg instead.
            let at = if far <= len - swallow0 + 1e-9 {
                at
            } else {
                let block = march(&reach, out, 0.0, None, self.seat).1;
                march(&reach, out, (len - block) / 2.0, None, self.seat).0
            };
            for (&member, &d) in span.members.iter().zip(&at) {
                let f = (len - d) / len;
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
    /// each, in chain order. Beyond the members' own extents, each end's
    /// **cluster** swallows a stretch of the landing leg before the clear
    /// window opens ([`Seats::absolutize`]) — the pin-to-pin distance must
    /// carry that too, or the window between two busy clusters closes to
    /// nothing and the members land on their seats. A cluster reaches two
    /// ways from its landing and the leg leaves by exactly one of them, so
    /// both are stated here and the **caller** picks: which way the leg
    /// marches is the anchors' own track order, which this pass cannot see
    /// and [`super::place`] already knows.
    pub(super) fn demands(&self, children: &[PlacedNode]) -> Vec<Demand> {
        self.spanning
            .iter()
            .map(|s| {
                let cluster =
                    |end: &(usize, Terminal)| self.cluster(children, end.0).inflate(self.seat);
                Demand {
                    ends: [(s.ends[0].0, s.ends[0].1.at), (s.ends[1].0, s.ends[1].1.at)],
                    reach: s
                        .members
                        .iter()
                        .map(|&m| {
                            [
                                member_reach(children, m, true),
                                member_reach(children, m, false),
                            ]
                        })
                        .collect(),
                    cluster: [cluster(&s.ends[0]), cluster(&s.ends[1])],
                    leg: s.ends[1].1.facing,
                    rung: self.rung(&s.ends[1]),
                    seat: self.seat,
                }
            })
            .collect()
    }

    /// The **rhythm** a span landing on `end` carries on [SPEC 16.1]: how far
    /// out along that pin's own normal the ladder's next column would stand,
    /// measured from the landing itself, and the pitch it goes on by. `None`
    /// where that side holds no lane at all — there is no rhythm to fall in
    /// with, and the members then stand off the cluster alone.
    fn rung(&self, end: &(usize, Terminal)) -> Option<(f64, f64)> {
        let side = end.1.facing?;
        let r = self
            .rungs
            .iter()
            .find(|r| r.anchor == end.0 && r.side == side)?;
        Some((r.next - dot(end.1.at, side.normal()), r.pitch))
    }
}

/// What one spanning chain asks of the tracks [SPEC 16.1]. Its satellites
/// march out from the landing as the next columns of that side's ladder, so
/// the pin-to-pin line has to be long enough for that march — and for what
/// each end's cluster swallows before it. Everything [`march`] reads is
/// carried here, so the reserve is struck by the very arithmetic the seat
/// then runs; the track arithmetic on top is [`super::place`]'s.
pub(super) struct Demand {
    /// Each end's anchor, and the landing's offset in that anchor's own frame.
    pub ends: [(usize, (f64, f64)); 2],
    /// Each member's ink either side of its centre — along x, along y.
    reach: Vec<[(f64, f64); 2]>,
    /// Each end's cluster, seat gap included, in its anchor's own frame —
    /// the very box [`Seats::absolutize`] measures the leg against.
    cluster: [Bbox; 2],
    /// The side end 1's pin faces: the landing leg runs along its normal —
    /// `None` for a facing-less end, whose leg is the raw chord.
    leg: Option<Side>,
    /// The ladder rhythm that side offers ([`Seats::rung`]).
    rung: Option<(f64, f64)>,
    seat: f64,
}

impl Demand {
    /// The least pin-to-pin distance along one axis: the room the members'
    /// own [`march`] takes, plus the stretch each end's cluster **swallows**
    /// of the leg before the clear window opens.
    ///
    /// `order` is how end 0's track compares with end 1's, and `perp` the
    /// settled offset between the two landings across this axis, when the
    /// caller has one. Given both, the leg's line is known and the swallow
    /// is [`swallow`] — exactly what the seat pass will consume, so the
    /// reserve is neither short nor slack. Without them the line is not
    /// known here and the cluster's own projection stands: the worst case
    /// over every line the leg could take, still read on the side the leg
    /// leaves by whenever the order alone settles that much.
    pub(super) fn need(&self, horiz: bool, order: Ordering, perp: Option<f64>) -> f64 {
        let sign = match order {
            Ordering::Less => 1.0,
            Ordering::Greater => -1.0,
            Ordering::Equal => 0.0,
        };
        // The leg leaves end 1 the way end 0 lies: the opposite of `sign`.
        let out = -sign;
        let horiz_leg = self.leg.map(Side::is_vertical);
        let exact = |end: usize| -> Option<f64> {
            let perp = perp?;
            if horiz_leg? != horiz || sign == 0.0 {
                return None;
            }
            let at = self.ends[end].1;
            // End 0 probes on end 1's own line; end 1 already sits on it.
            let off = if end == 0 { perp } else { 0.0 };
            let out = if end == 0 { sign } else { -sign };
            let (probe, d) = if horiz {
                ((at.0, at.1 + off), (out, 0.0))
            } else {
                ((at.0 + off, at.1), (0.0, out))
            };
            Some(swallow(self.cluster[end], probe, d))
        };
        let projected = |end: usize| {
            let (c, p) = (self.cluster[end], self.ends[end].1);
            let (lo, hi) = if horiz {
                (p.0 - c.min_x, c.max_x - p.0)
            } else {
                (p.1 - c.min_y, c.max_y - p.1)
            };
            match order {
                Ordering::Less if end == 0 => hi,
                Ordering::Less => lo,
                Ordering::Greater if end == 0 => lo,
                Ordering::Greater => hi,
                Ordering::Equal => lo.max(hi),
            }
        };
        let swallowed = |end: usize| exact(end).unwrap_or_else(|| projected(end));
        // The ladder's rhythm rides the leg only where the leg really leaves
        // the landing along that pin's own normal — an end 0 lying *behind*
        // end 1 turns the leg back over the part, where no lane stands.
        let rung = self.rung.filter(|_| {
            horiz_leg == Some(horiz)
                && self
                    .leg
                    .map(|s| if horiz { s.normal().0 } else { s.normal().1 })
                    == Some(out)
        });
        let reach: Vec<(f64, f64)> = self
            .reach
            .iter()
            .map(|r| if horiz { r[0] } else { r[1] })
            .collect();
        march(&reach, out, swallowed(1), rung, self.seat).1 + swallowed(0)
    }
}

/// Where a span's members stand along their leg [SPEC 16.1] — the one
/// arithmetic the track reserve ([`Demand::need`]) and the seat itself
/// ([`Seats::absolutize`]) both run, so the tracks part by exactly what the
/// members then take.
///
/// `reach` is each member's ink either side of its centre on the leg's axis —
/// `(toward −, toward +)` — in wire order, so the **last**-named is the one
/// nearest the landing; `out` is the sign the leg leaves that landing by, and
/// `start` how much of the leg the landing's own cluster swallows first. Each
/// member stands one `rung` **pitch** past the column before it — the ladder's
/// own rhythm, carried on — or clear of that column's ink where its own asks
/// for more, which is also all a leg with no ladder to join ever asks.
///
/// Returns each member's centre as a distance out from the landing, and how
/// far the outermost one's ink reaches past it.
fn march(
    reach: &[(f64, f64)],
    out: f64,
    start: f64,
    rung: Option<(f64, f64)>,
    seat: f64,
) -> (Vec<f64>, f64) {
    let (mut lane, pitch) = rung.unwrap_or((f64::NEG_INFINITY, 0.0));
    let (mut edge, mut far) = (start, start);
    let mut at = vec![0.0; reach.len()];
    for (i, &(lo, hi)) in reach.iter().enumerate().rev() {
        let (inward, outward) = if out > 0.0 { (lo, hi) } else { (hi, lo) };
        at[i] = (edge + inward).max(lane);
        edge = at[i] + outward + seat;
        far = at[i] + outward;
        lane = at[i] + pitch;
    }
    (at, far)
}

/// A span member's ink either side of its centre on one axis — `(toward −,
/// toward +)`, off the same [`drawn`] extent everything else in this pass
/// measures. The centre is its **box** centre, so the cross axis keeps the row.
fn member_reach(children: &[PlacedNode], m: usize, horiz: bool) -> (f64, f64) {
    let bb = drawn(&children[m]);
    let c = children[m].bbox.center();
    if horiz {
        (c.0 - bb.min_x, bb.max_x - c.0)
    } else {
        (c.1 - bb.min_y, bb.max_y - c.1)
    }
}

/// Every chain's **lane** [SPEC 16.1] — how far out along its pin's own normal
/// it stands before turning onto its growth ray — one per entry of `held`, in
/// that order, and each ladder's own [`Rung`] beside them.
///
/// **A lane per chain, within a ray.** A chain reaches back toward the part from
/// its own lane, so sharing one lane stands a flag's body over its neighbour's
/// leg — and the router, which may not cross a body, jogs that leg into a
/// staircase rather than the one square turn a sheet draws. Each chain
/// therefore steps out past the one before it, in the order `held` arrives in
/// (see [`Seats::build`]), on the ladder's own **pitch** — the greediest of
/// those steps, taken by them all, so the columns read as a grid. The step only
/// ever pushes a lane **outward**, so the walk is monotone and its bound is a
/// backstop, never a cutoff.
///
/// A chain growing straight out along its own pin (`lead == 0`) has no lane to
/// take: it never turns, so there is nothing to ladder.
fn ladder(
    children: &[PlacedNode],
    held: &[Growing],
    rays: &[(usize, Side)],
    seat: f64,
) -> (Vec<f64>, Vec<Rung>) {
    // Everything below is in **outward** coordinates — the lane axis times the
    // pin's own outward sign — so "farther out" is always larger.
    let lead: Vec<f64> = held.iter().map(|g| g.lead).collect();
    // A straight-growing chain's lane *is* its pin, and it takes no part in the
    // ladder; only a turning one is carried in outward coordinates below.
    let mut along: Vec<f64> = Vec::with_capacity(held.len());
    let mut out: Vec<f64> = Vec::with_capacity(held.len());
    let mut back: Vec<f64> = Vec::with_capacity(held.len());
    let mut fwd: Vec<f64> = Vec::with_capacity(held.len());
    for (g, &lead) in held.iter().zip(&lead) {
        let (lane, reach) = clearing(children, g, lead, seat);
        along.push(lane);
        out.push(lead * lane);
        back.push(reach.0);
        fwd.push(reach.1);
    }
    // A lane must also clear the **stacks** it crosses [SPEC 16.1]: a chain
    // growing straight out of a deeper pin on the same side lays its members
    // across the lane axis, and a column stepped only past the part descends
    // onto them — or leaves no corridor for the wire off the stack's far
    // terminal (a bridge's return climbing to its second pin). Floor each
    // turning lane at every such stack's reach plus the seat gap. A stack on
    // the lane's own pin is the fan's — the shared lead splits at the lane —
    // and is never floored against.
    for i in 0..held.len() {
        if lead[i] == 0.0 {
            continue;
        }
        let g = &held[i];
        let gf = Frame::outward(g.ray.normal());
        for (s, &slead) in held.iter().zip(&lead) {
            if slead != 0.0 || s.held.child != g.held.child || s.pin == g.pin {
                continue;
            }
            if gf.u(s.ray.normal()) * lead[i] <= 0.0 || gf.cross(s.pin) <= g.depth {
                continue;
            }
            out[i] = out[i].max(stack_reach(children, s, seat) + seat + back[i]);
        }
    }
    // The **columns** the side's lanes really hold. A pin whose net branches
    // *both* ways — a rail up to its flag, down to its decoupling cap —
    // leaves on one lead and splits **once**, at one point, rather than
    // peeling twice off its stub, so the first chain of each ray off one pin
    // pairs with its opposite number into one column: the wires run
    // co-linearly out to it, which the router draws as one lead (an implicit
    // fan on one fixed port — [ROUTING.md](../../../ROUTING.md) Special nodes
    // / Fixed ports). Everything else is a column of its own — a later
    // same-ray chain never rides the split of the pair before it, so one
    // up-chain shares one down-chain's lane, never two. Sharing is
    // structural, decided here and once: nothing downstream may re-equalize
    // two lanes the ladder has just stepped apart.
    let mut col: Vec<Option<usize>> = vec![None; held.len()];
    let mut cols: Vec<Vec<usize>> = Vec::new();
    for i in 0..held.len() {
        if lead[i] == 0.0 || col[i].is_some() {
            continue;
        }
        let k = cols.len();
        col[i] = Some(k);
        let mut members = vec![i];
        let twin = held[i].ray_first.then(|| {
            (0..held.len()).find(|&j| {
                j != i
                    && lead[j] != 0.0
                    && col[j].is_none()
                    && held[j].ray_first
                    && held[j].held.child == held[i].held.child
                    && held[j].pin == held[i].pin
                    && held[j].ray != held[i].ray
            })
        });
        if let Some(Some(j)) = twin {
            col[j] = Some(k);
            members.push(j);
        }
        cols.push(members);
    }
    // A column asks for the outermost lane any of its chains asked for, and
    // reaches as far as the widest ink among them, each way.
    let ask = |members: &[usize], v: &[f64]| {
        members
            .iter()
            .map(|&i| v[i])
            .fold(f64::NEG_INFINITY, f64::max)
    };
    let mut cout: Vec<f64> = cols.iter().map(|m| ask(m, &out)).collect();
    let cback: Vec<f64> = cols.iter().map(|m| ask(m, &back)).collect();
    let cfwd: Vec<f64> = cols.iter().map(|m| ask(m, &fwd)).collect();
    // The ladder's **pitch** [SPEC 16.1]: what one column must step past the
    // one before it is its predecessor's **whole** ink — a chain's readout
    // text runs outward past its own lane, so the next column must clear that
    // side too, or its bodies land on the text — plus the seat gap, plus its
    // own reach back. Taken pair by pair that step is even *between the ink*
    // and uneven between the columns, which wobbles with nothing more
    // meaningful than how many characters each part's value happens to read;
    // a reader seeing a row of columns reads a grid. So the greediest step any
    // neighbouring pair of this ladder asks is the step they all take.
    // `held` is sorted canonically and one side's chains are contiguous, so
    // the columns arrive in ladder order and one pass over each settles them.
    let mut pitch: Vec<f64> = vec![0.0; rays.len()];
    let mut prev: Option<(usize, usize)> = None;
    for (k, members) in cols.iter().enumerate() {
        let group = held[members[0]].group;
        if let Some((before, j)) = prev
            && before == group
        {
            pitch[group] = pitch[group].max(cfwd[j] + seat + cback[k]);
        }
        prev = Some((group, k));
    }
    // On that pitch, then — except where a column's own lane already stands
    // farther out (the wall it clears, a stack floor under it), which it
    // keeps: the step only ever pushes outward, and the rhythm resumes from
    // wherever the column really landed.
    let mut prev: Option<(usize, f64)> = None;
    for (k, members) in cols.iter().enumerate() {
        let group = held[members[0]].group;
        if let Some((before, at)) = prev
            && before == group
        {
            cout[k] = cout[k].max(at + pitch[group]);
        }
        prev = Some((group, cout[k]));
    }
    // The rhythm itself, kept for the chains no anchor holds: a **span**'s
    // members are the next columns of the ladder its landing belongs to
    // ([`Seats::absolutize`]), so each ladder states where its next column
    // would stand and the pitch that carries on from there. Columns within a
    // group are contiguous and monotone, so the last one seen is the
    // outermost.
    let mut rungs: Vec<Option<Rung>> = vec![None; rays.len()];
    for (k, members) in cols.iter().enumerate() {
        let group = held[members[0]].group;
        let (anchor, side) = rays[group];
        rungs[group] = Some(Rung {
            anchor,
            side,
            next: cout[k] + pitch[group],
            pitch: pitch[group],
        });
    }
    for (i, &lead) in lead.iter().enumerate() {
        if let Some(k) = col[i] {
            along[i] = lead * cout[k];
        }
    }
    (along, rungs.into_iter().flatten().collect())
}

/// What one chain asks of its lane before any other is consulted [SPEC 16.1]:
/// the innermost lane clearing the part it hangs from, and how far its widest
/// member's ink reaches from that lane — back toward the part, and outward
/// past it (a readout runs on the outward side) — the ink the neighbouring
/// lanes have to clear. All in outward coordinates.
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
fn clearing(children: &[PlacedNode], g: &Growing, lead: f64, seat: f64) -> (f64, (f64, f64)) {
    let frame = Frame::outward(g.ray.normal());
    let straight = frame.u(g.pin) + lead * seat;
    if lead == 0.0 {
        return (straight, (0.0, 0.0));
    }
    let wall = {
        let (lo, hi) = project(drawn(&children[g.held.child]), frame.u);
        lead * if lead > 0.0 { hi } else { lo }
    };
    // How far a box reaches from the member's landing — back toward the part
    // (`.0`) and outward past it (`.1`).
    let reach = |box_: Bbox, at: (f64, f64)| {
        let (u0, u1) = (frame.u(corner(box_, false)), frame.u(corner(box_, true)));
        let (lo, hi) = (u0.min(u1), u0.max(u1));
        let (back, fwd) = if lead > 0.0 {
            (frame.u(at) - lo, hi - frame.u(at))
        } else {
            (hi - frame.u(at), frame.u(at) - lo)
        };
        (back, fwd)
    };
    let (mut ink, mut fwd_ink, mut connection) = (0.0f64, 0.0f64, 0.0f64);
    for (&member, inbound) in g.chain.members.iter().zip(&g.chain.inbound) {
        let node = &children[member];
        let at = terminal(node, inbound.as_deref()).at;
        let (b, f) = reach(drawn(node), at);
        ink = ink.max(b);
        fwd_ink = fwd_ink.max(f);
        connection = connection.max(reach(connection_box(node), at).0);
    }
    let out = (lead * straight)
        .max(wall + seat + connection)
        .max(wall + ink);
    (lead * out, (ink, fwd_ink))
}

/// The rows a part's pins take on `side`, in the part's own frame — the
/// corridors a member's readouts must not close ([`Seats::build`]'s
/// restack). A glyph part carries ports, not pin nodes, and reports none.
fn pin_rows(part: &PlacedNode, side: Side) -> Vec<f64> {
    fn walk(n: &PlacedNode, ox: f64, oy: f64, side: Side, out: &mut Vec<f64>) {
        for c in &n.children {
            let (cx, cy) = (ox + c.cx, oy + c.cy);
            if c.type_chain.iter().any(|t| t == "pin") {
                let landed = c.children.iter().find_map(|s| {
                    s.type_chain
                        .iter()
                        .any(|t| t == "pin-stub")
                        .then(|| super::terminal::ident(&s.attrs, "pin"))
                        .flatten()
                        .as_deref()
                        .and_then(Side::parse)
                });
                if landed == Some(side) {
                    out.push(if side.is_vertical() { cy } else { cx });
                }
                continue;
            }
            walk(c, cx, cy, side, out);
        }
    }
    let mut out = Vec::new();
    walk(part, 0.0, 0.0, side, &mut out);
    out
}

/// Which of a chain's members are **taps** [SPEC 16.1], by this pass's own
/// reading of "symbol label" — one classifier for the packer ([`Seats::grow`])
/// and the stack forecast ([`stack_reach`]), so they cannot disagree.
fn tap_flags(children: &[PlacedNode], chain: &Chain) -> Vec<bool> {
    crate::desugar::schematic::chain::taps(chain, |m| {
        crate::desugar::schematic::sch_kind(&children[m].type_chain)
            == Some(crate::desugar::schematic::SchKind::Label)
            && super::terminal::ident(&children[m].attrs, "symbol").is_some()
    })
}

/// How far a straight-stacking chain's ink will reach out along its ray —
/// [`Seats::grow`]'s arithmetic run dry (each member one seat gap past the
/// last, taps aside, no packer): the reach a crossing lane must clear
/// ([`ladder`]), forecast before anything has seated.
fn stack_reach(children: &[PlacedNode], g: &Growing, seat: f64) -> f64 {
    let frame = Frame::outward(g.ray.normal());
    let limbs = crate::desugar::schematic::chain::limbs(&g.chain);
    let mut base = frame.cross(g.pin);
    for (i, (&member, inbound)) in g.chain.members.iter().zip(&g.chain.inbound).enumerate() {
        if limbs[i].is_some() {
            continue;
        }
        let sat = &children[member];
        let band = band_of(frame, drawn(sat), terminal(sat, inbound.as_deref()).at);
        base = base + seat + band.neg + band.pos;
    }
    base
}

/// Where a one-held chain grows **from** and **toward** [SPEC 16.1]: the pin
/// that holds it, and the ray it runs along — the one shared rule
/// ([`crate::desugar::schematic::chain::growth_ray`]): the terminator's own
/// convention, yielding to the pin's normal when anti-parallel, and off the
/// straight corridor of a shared pin.
///
/// One home for the ray, because two passes need it before anything is seated:
/// [`Seats::grow`] lays the chain along it, and [`Seats::build`] sorts the
/// chains sharing it into arrival order first.
fn growth(
    children: &[PlacedNode],
    chain: &Chain,
    held: &End,
    edges: &[[End; 2]],
) -> (Side, Terminal) {
    let pin = terminal(&children[held.child], held.terminal.as_deref());
    let last = *chain.members.last().expect("a chain has a member");
    let out = crate::desugar::schematic::chain::growth_ray(
        tag_facing(
            &children[last],
            chain.inbound.last().and_then(|t| t.as_deref()),
        ),
        pin.facing,
        crate::desugar::schematic::chain::shared_pin(edges, held, |c| {
            crate::desugar::schematic::sch_kind(&children[c].type_chain).is_some()
                && super::place::role(&children[c]) == crate::desugar::schematic::Role::Satellite
        }),
        chain.members.iter().all(|&m| net::is_run(&children[m])),
    );
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

/// The smallest `t ∈ [0, 1]` at which `from + t·d` stands outside `r` — `0`
/// when it already does. The spanning clip's one primitive: how much of the
/// chord an end's inflated cluster swallows.
fn exit_t(from: (f64, f64), d: (f64, f64), r: Bbox) -> f64 {
    let len = (d.0 * d.0 + d.1 * d.1).sqrt();
    if len < 1e-9 {
        return 1.0;
    }
    (swallow(r, from, d) / len).clamp(0.0, 1.0)
}

/// The stretch of a leg leaving `at` along `d` that the box `r` **swallows**:
/// the distance to where the ray leaves it, and **zero** when `at` is not
/// inside — a leg that misses a cluster owes it nothing.
///
/// One reading, two passes. The track demand ([`Seats::demands`]) reserves
/// room between two anchors for what each end's cluster will eat of the leg,
/// and the seat itself ([`Seats::absolutize`]) then eats it. Measured two
/// ways they disagree, and a cluster the leg passes clear of — a connector's
/// ground flags hanging a row below the bus its fuse rides — is reserved for
/// and never used: the tracks part for a stretch of leg nothing ever fills.
fn swallow(r: Bbox, at: (f64, f64), d: (f64, f64)) -> f64 {
    let inside = at.0 >= r.min_x && at.0 <= r.max_x && at.1 >= r.min_y && at.1 <= r.max_y;
    if !inside {
        return 0.0;
    }
    let len = (d.0 * d.0 + d.1 * d.1).sqrt();
    if len < 1e-9 {
        return f64::INFINITY;
    }
    let axis = |p: f64, d: f64, lo: f64, hi: f64| {
        if d > 1e-9 {
            (hi - p) / d
        } else if d < -1e-9 {
            (lo - p) / d
        } else {
            f64::INFINITY
        }
    };
    axis(at.0, d.0, r.min_x, r.max_x).min(axis(at.1, d.1, r.min_y, r.max_y)) * len
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

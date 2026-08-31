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
//!   the wire's landing leg (the second pin's row or column), at even
//!   fractions of the stretch standing clear of both ends' clusters.
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
//! enough for the even fractions, and for the stretch of leg each end's
//! cluster swallows, to clear ([`Demand`], struck in [`super::place`]).
//! Within its window it stands clear by construction; it still enters no
//! anchor's [`Stack`], so a pathological weave of spans and seats can
//! overlap — the router then reports what it cannot lawfully draw.

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
    /// Whether this is the **first** chain (statement order) taking this ray
    /// off this pin — the one the lane share may pair across rays
    /// ([`ladder`]): a junction splits once, so one up-chain rides one
    /// down-chain's lane and every later same-ray chain ladders out.
    ray_first: bool,
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
        let wire_edges = edges(children, links, scope);
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
                    // f64::signum maps 0.0 to 1.0, so the no-turn side is spelled out.
                    let side = if lead == 0.0 { 0 } else { lead.signum() as i8 };
                    let key = (one.child, ray, side);
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
                } else {
                    canon(b).total_cmp(&canon(a))
                })
        });
        let lanes = ladder(children, &held, seat);
        for (g, along) in held.iter().zip(lanes) {
            let frame = Frame::outward(g.ray.normal());
            out.grow(
                children,
                g,
                frame,
                along,
                &mut packers[g.held.child],
                &wire_edges,
            );
        }
        out
    }

    /// A one-placed-end chain: each satellite seats farther out along the
    /// growth ray, its connection point landing where the packer clears —
    /// except a **tap** ([`crate::desugar::schematic::chain::taps`]), which
    /// hangs off its attachment member instead of taking a slot in the stack.
    fn grow(
        &mut self,
        children: &[PlacedNode],
        g: &Growing,
        frame: Frame,
        along: f64,
        stack: &mut Stack,
        wire_edges: &[[End; 2]],
    ) {
        let (chain, held) = (&g.chain, &g.held);
        let tap = tap_flags(children, chain);
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
        for (i, (&member, inbound)) in chain.members.iter().zip(&chain.inbound).enumerate() {
            if tap[i] {
                continue;
            }
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
        // Taps hang off their attachment member's terminal, one seat out
        // along the ray their **posed** drawing states — the pose chooser
        // decided it ([`crate::desugar::schematic::chain::tap_ray`]) and the
        // turned connection point carries it, so this pass only reads
        // geometry, exactly as it does for the trunk's ray.
        for (i, &member) in chain.members.iter().enumerate() {
            if !tap[i] {
                continue;
            }
            let Some(parent) = chain.parents[i] else {
                continue;
            };
            let parent_child = chain.members[parent];
            let Some(pseat) = &self.seats[parent_child] else {
                continue;
            };
            let (pdx, pdy) = (pseat.dx, pseat.dy);
            // The wire's terminal on the attachment side — read off the edge
            // that joins the pair, since a chain records only inbound ends.
            let mine = End {
                child: member,
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
            let attach = (junction.0 + pdx, junction.1 + pdy);
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
                let box_ = drawn(&children[member]);
                let (lo, hi) = (tf.cross(corner(box_, false)), tf.cross(corner(box_, true)));
                let c = tf.cross(t.at);
                let band = Band {
                    neg: c - lo.min(hi),
                    pos: hi.max(lo) - c,
                };
                // Risen one gap along its own ray, packed out along the
                // aside — the interval is read at the risen height.
                let rn = ray.normal();
                let risen = (attach.0 + rn.0 * self.seat, attach.1 + rn.1 * self.seat);
                let (u0, u1) = (tf.u(corner(box_, false)), tf.u(corner(box_, true)));
                let (au, u) = (tf.u(risen), tf.u(t.at));
                let interval = (au + u0.min(u1) - u, au + u0.max(u1) - u);
                let line = stack.seat(
                    SeatLine::new(tf, true, tf.cross(attach)),
                    interval,
                    self.seat,
                    &band,
                );
                let target = {
                    let p = tf.pt(au, line);
                    (p.0, p.1)
                };
                self.seats[member] = Some(Seat {
                    anchor: held.child,
                    dx: target.0 - t.at.0,
                    dy: target.1 - t.at.1,
                });
                continue;
            }
            let tf = Frame::outward(ray.normal());
            let box_ = drawn(&children[member]);
            let (lo, hi) = (tf.cross(corner(box_, false)), tf.cross(corner(box_, true)));
            let c = tf.cross(t.at);
            let band = Band {
                neg: c - lo.min(hi),
                pos: hi.max(lo) - c,
            };
            let (u0, u1) = (tf.u(corner(box_, false)), tf.u(corner(box_, true)));
            let (au, u) = (tf.u(attach), tf.u(t.at));
            let interval = (au + u0.min(u1) - u, au + u0.max(u1) - u);
            let line = stack.seat(
                SeatLine::new(tf, true, tf.cross(attach)),
                interval,
                self.seat,
                &band,
            );
            let target = tf.pt(au, line);
            self.seats[member] = Some(Seat {
                anchor: held.child,
                dx: target.0 - t.at.0,
                dy: target.1 - t.at.1,
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
            // The members sit on the wire's **landing leg** — the straight
            // run into the second end, on that pin's own row or column
            // [SPEC 16.1] — never on the raw pin-to-pin diagonal, which cuts
            // across the sheet (and, off an away-facing pin, across its own
            // part). And only on the stretch of that leg standing **clear**
            // of both ends' clusters by a seat gap; a leg swallowed whole
            // (degenerate ends) falls back to the raw endpoints — the router
            // will say what it thinks of that.
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
            let t0 = clear(&span.ends[0], a, b);
            let t1 = 1.0 - clear(&span.ends[1], b, a);
            // A member's **body** must clear too, not just its centre: inset
            // the window by the end members' half extents along the leg.
            let len = ((b.0 - a.0).abs() + (b.1 - a.1).abs()).max(1e-9);
            let horiz = (b.1 - a.1).abs() < (b.0 - a.0).abs();
            let half = |m: Option<&usize>| {
                m.map_or(0.0, |&m| {
                    let bb = drawn(&children[m]);
                    (if horiz { bb.w() } else { bb.h() }) / 2.0 / len
                })
            };
            let (t0, t1) = (
                t0 + half(span.members.first()),
                t1 - half(span.members.last()),
            );
            let (t0, t1) = if t0 + 1e-9 < t1 { (t0, t1) } else { (0.0, 1.0) };
            let steps = span.members.len() + 1;
            for (i, &member) in span.members.iter().enumerate() {
                let f = t0 + (t1 - t0) * (i + 1) as f64 / steps as f64;
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
    /// nothing and the members land on their seats. The march direction is
    /// not known until the anchors place, so the swallow is the worst case.
    pub(super) fn demands(&self, children: &[PlacedNode]) -> Vec<Demand> {
        self.spanning
            .iter()
            .map(|s| {
                let swallow = |end: &(usize, Terminal), horiz: bool| {
                    let c = self.cluster(children, end.0);
                    let p = end.1.at;
                    self.seat
                        + if horiz {
                            (p.0 - c.min_x).max(c.max_x - p.0)
                        } else {
                            (p.1 - c.min_y).max(c.max_y - p.1)
                        }
                };
                Demand {
                    ends: [(s.ends[0].0, s.ends[0].1.at), (s.ends[1].0, s.ends[1].1.at)],
                    need: (
                        step(&s.members, children, Bbox::w, self.seat)
                            + swallow(&s.ends[0], true)
                            + swallow(&s.ends[1], true),
                        step(&s.members, children, Bbox::h, self.seat)
                            + swallow(&s.ends[0], false)
                            + swallow(&s.ends[1], false),
                    ),
                }
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
    for _ in 0..=held.len() {
        let mut moved = false;
        // Ladder: within one ray, each chain steps past its predecessor's
        // **whole** ink — a chain's readout text runs outward past its own
        // lane, so the next column must clear that side too, or its bodies
        // land on the text and the packer stacks the columns end to end
        // instead of side by side.
        let mut prev: Option<(usize, f64)> = None;
        for (i, g) in held.iter().enumerate() {
            if lead[i] == 0.0 {
                continue;
            }
            if let Some((group, edge)) = prev
                && group == g.group
                && out[i] - back[i] < edge
            {
                out[i] = edge + back[i];
                moved = true;
            }
            prev = Some((g.group, out[i] + fwd[i] + seat));
        }
        // Share: a pin whose net branches **both** ways — a rail up to its
        // flag, down to its decoupling cap — leaves on one lead and splits
        // **once**, at one point, rather than peeling twice off its stub. So
        // the **first** chain of each ray off one pin takes the outermost
        // lane its opposite number asked for; the wires run co-linearly out
        // to that point, which the router draws as one lead (an implicit fan
        // on one fixed port — [ROUTING.md](../../../ROUTING.md) Special nodes
        // / Fixed ports). Everything else ladders side by side like any two
        // chains: same-ray chains never share (equalizing them while the
        // ladder steps one past the other is a feedback loop), and a later
        // chain never rides the split of the pair before it — one up-chain
        // shares one down-chain's lane, never two.
        for i in 0..held.len() {
            for j in 0..held.len() {
                if i != j
                    && lead[i] != 0.0
                    && lead[j] != 0.0
                    && held[i].ray_first
                    && held[j].ray_first
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
    let tap = tap_flags(children, &g.chain);
    let mut base = frame.cross(g.pin);
    for (i, (&member, inbound)) in g.chain.members.iter().zip(&g.chain.inbound).enumerate() {
        if tap[i] {
            continue;
        }
        let sat = &children[member];
        let c = frame.cross(terminal(sat, inbound.as_deref()).at);
        let box_ = drawn(sat);
        let (lo, hi) = (
            frame.cross(corner(box_, false)),
            frame.cross(corner(box_, true)),
        );
        base = base + seat + (c - lo.min(hi)) + (hi.max(lo) - c);
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
    let inside =
        |p: (f64, f64)| p.0 >= r.min_x && p.0 <= r.max_x && p.1 >= r.min_y && p.1 <= r.max_y;
    if !inside(from) {
        return 0.0;
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
    let t = axis(from.0, d.0, r.min_x, r.max_x).min(axis(from.1, d.1, r.min_y, r.max_y));
    t.clamp(0.0, 1.0)
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

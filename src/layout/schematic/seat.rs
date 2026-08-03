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

use super::super::geom::Frame;
use super::super::ir::{Bbox, PlacedNode};
use super::super::stack::{Band, SeatLine, Stack};
use super::terminal::{Terminal, terminal};
use crate::desugar::pose::Side;
use crate::desugar::schematic::Role;
use crate::desugar::schematic::chain::{Chain, End, chains, holder, placed_ends};
use crate::ledger::consts::LABEL_SEAT;
use crate::resolve::{LinkKind, ResolvedLink};

/// A satellite's seat off the anchor that holds its chain: an offset from
/// that anchor's origin, in sheet coordinates.
struct Seat {
    anchor: usize,
    dx: f64,
    dy: f64,
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
}

impl Seats {
    /// Seat the scope's satellites against the pins their wires reach.
    pub(super) fn build(
        children: &[PlacedNode],
        roles: &[Role],
        links: &[&ResolvedLink],
        scope: &str,
    ) -> Seats {
        let satellite: Vec<bool> = roles.iter().map(|r| *r == Role::Satellite).collect();
        let mut out = Seats {
            seats: (0..children.len()).map(|_| None).collect(),
            spanning: Vec::new(),
            floating: Vec::new(),
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
            packers[i].obstruct(c.bbox);
        }
        for chain in chains(&satellite, &edges(children, links, scope)) {
            let ends = placed_ends(&chain, roles);
            // One anchor holds it → grow off that pin; two → span between them;
            // none → the flow fallback. Which of the first two a chain is, is
            // [`holder`]'s single answer, shared with the pose chooser.
            match (holder(&ends), ends.as_slice()) {
                (Some(one), _) => out.grow(children, &chain, one, &mut packers[one.child]),
                (None, [a, b, ..]) => out.distribute(children, chain, a, b),
                (None, _) => out.floating.extend(chain.members),
            }
        }
        out
    }

    /// A one-placed-end chain: each satellite seats farther out along the
    /// growth ray, its connection point landing where the packer clears.
    fn grow(&mut self, children: &[PlacedNode], chain: &Chain, held: &End, stack: &mut Stack) {
        let anchor = &children[held.child];
        let pin = terminal(anchor, held.terminal.as_deref());
        // The direction the chain grows: away from the terminator's own
        // connection geometry, else straight out along the pin [SPEC 16.1].
        let last = *chain.members.last().expect("a chain has a member");
        let out = terminal(
            &children[last],
            chain.inbound.last().and_then(|t| t.as_deref()),
        )
        // A text label carries no connection geometry, so it grows along the
        // pin's own outward normal; with neither (a wire to a plain box) the
        // chain hangs below, the one direction a sheet always has room in.
        .facing
        .map_or_else(|| pin.facing.unwrap_or(Side::Bottom), Side::opposite);
        let frame = Frame::outward(out.normal());
        let (along, base) = (frame.u(pin.at), frame.cross(pin.at));
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
            let (u0, u1) = (frame.u(corner(box_, false)), frame.u(corner(box_, true)));
            let u = frame.u(point);
            let interval = (along + u0.min(u1) - u, along + u0.max(u1) - u);
            let line = stack.seat(
                SeatLine::new(frame, true, base),
                interval,
                LABEL_SEAT,
                &band,
            );
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
    /// cells. In the anchor's own coordinates, like its bbox.
    pub(super) fn cluster(&self, children: &[PlacedNode], anchor: usize) -> Bbox {
        self.seats
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().filter(|s| s.anchor == anchor).map(|s| (i, s)))
            .fold(children[anchor].bbox, |b, (i, s)| {
                b.union(children[i].bbox.shifted(s.dx, s.dy))
            })
    }

    /// Once the anchors are placed: land every seated satellite in scene
    /// coordinates. A pin-relative seat rides its anchor; a spanning chain
    /// reads both placed ends now that they exist.
    pub(super) fn absolutize(&self, children: &mut [PlacedNode]) {
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

    /// What the spanning chains ask of the tracks [SPEC 16.1], one [`Demand`]
    /// each, in chain order.
    pub(super) fn demands(&self, children: &[PlacedNode]) -> Vec<Demand> {
        self.spanning
            .iter()
            .map(|s| Demand {
                ends: [(s.ends[0].0, s.ends[0].1.at), (s.ends[1].0, s.ends[1].1.at)],
                need: (
                    step(&s.members, children, Bbox::w),
                    step(&s.members, children, Bbox::h),
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
fn step(members: &[usize], children: &[PlacedNode], extent: impl Fn(&Bbox) -> f64) -> f64 {
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
    (e.len() + 1) as f64 * (step + LABEL_SEAT)
}

/// A part's **drawn** extent: its box unioned with every descendant, so the
/// chrome that pokes outside counts. A ref or a value readout is a `pin:`
/// overlay — out of the flow that sized the box, but ink on the sheet and an
/// obstacle to the router all the same (the scene index carries it as the
/// part's `overflow`). Seating measures this, or a chain clears a part's box
/// and lands on its readout.
fn drawn(node: &PlacedNode) -> Bbox {
    fn walk(n: &PlacedNode, ox: f64, oy: f64, out: &mut Bbox) {
        for c in &n.children {
            let (cx, cy) = (ox + c.cx, oy + c.cy);
            *out = out.union(c.bbox.shifted(cx, cy));
            walk(c, cx, cy, out);
        }
    }
    let mut out = node.bbox;
    walk(node, 0.0, 0.0, &mut out);
    out
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

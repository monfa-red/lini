//! The `orthogonal` strategy — ROUTING.md's six-step model: keep-outs &
//! worlds → channels → requests → weighted search → placement → geometry.
//! Each step decides once; none revisits an earlier step's answer.

pub(crate) mod admit;
pub(crate) mod cluster;
pub(crate) mod cost;
pub(crate) mod entry;
pub(crate) mod geometry;
pub(crate) mod graph;
pub(crate) mod labels;
pub(crate) mod ladder;
pub(crate) mod ledger;
pub(crate) mod order;
pub(crate) mod pairwise;
pub(crate) mod place;
pub(crate) mod rect;
pub(crate) mod request;
pub(crate) mod scene;
pub(crate) mod search;
mod world;

use crate::ast::Side;
use crate::layout::ir::{RoutedLink, Stray};
use crate::resolve::Strategy;
use crate::routing::{Routing, Rule, Severity, Violation};

use cost::min_pitch;
use entry::Entry;
use graph::{Axis, ChannelGraph};
use ledger::Ledger;
use rect::Rect;
use request::{EdgeReq, End};
use scene::SceneIndex;
use world::{build_worlds, world_ladder};

/// One routing world: a container's interior (`None` = the scene root) and
/// its channel decomposition. The key is the container's scene-node identity
/// ([`scene::WorldKey`]) — anonymous containers are worlds too.
pub(crate) struct World {
    pub key: scene::WorldKey,
    pub graph: ChannelGraph,
    /// The grid this world's scope states, if any (ROUTING.md §Vocabulary)
    /// — placement rounds an interior run's preference onto it.
    pub quantum: Option<Quantum>,
}

/// A world's **track quantum** (ROUTING.md §Vocabulary): the scope's own
/// grid — its step, and the scene point its lines count from. A schematic
/// lays its parts on multiples of the pitch in its own frame [SPEC 16.1], so
/// the lines the router rounds to are `origin + k·step`, wherever the parent
/// seated the scope: the parts' grid and the wires' are one grid by
/// construction, and no scope has to move to make them agree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Quantum {
    pub step: f64,
    pub origin: (f64, f64),
}

impl Quantum {
    /// The grid line nearest `at` that the corridor `walls` hold, for a run
    /// on `axis` — `None` when it holds none. The clamp is onto a **grid
    /// line**, never onto a wall: a corridor too narrow (or too badly placed)
    /// to carry a line of the grid has nothing to say about it, and pinning
    /// the run to the keep-out edge instead would trade the anchor's clear
    /// air for a hug the grid never asked for.
    pub fn snap(self, axis: Axis, at: f64, walls: (f64, f64)) -> Option<f64> {
        // A run's ordinate lies across its axis: a vertical run is placed in
        // x, a horizontal one in y.
        let phase = match axis {
            Axis::V => self.origin.0,
            Axis::H => self.origin.1,
        };
        let q = self.step;
        let line = |k: f64| phase + k * q;
        let (lo, hi) = (
            ((walls.0 - phase) / q).ceil(),
            ((walls.1 - phase) / q).floor(),
        );
        (lo <= hi).then(|| line(((at - phase) / q).round().max(lo).min(hi)))
    }
}

/// One end of a chain: the side it lands on, the endpoint's body, the lawful
/// port window on that side, and the fan group whose siblings share the port.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EndInfo {
    pub side: Side,
    pub rect: Rect,
    pub window: (f64, f64),
    pub fan: Option<usize>,
}

/// One straight piece of a route, in one channel of its axis. The span is
/// provisional until geometry fixes corners; the ordinate is placement's.
#[derive(Clone, Debug)]
pub(crate) struct Run {
    pub axis: Axis,
    pub chan: usize,
    pub span: (f64, f64),
    pub ord: Option<f64>,
}

/// One link's route: alternating runs, `runs[0]` serving `ends[0]`'s port
/// and the last run `ends[1]`'s — a single run serves both (a straight).
#[derive(Clone, Debug)]
pub(crate) struct Chain {
    /// Request index — the declaration-order key.
    pub link: usize,
    pub world: usize,
    pub runs: Vec<Run>,
    pub ends: [EndInfo; 2],
}

impl EndInfo {
    /// The side line's coordinate along the end run's travel axis — where
    /// the wire leaves the body.
    pub fn side_coord(&self) -> f64 {
        match self.side {
            Side::Right => self.rect.x1,
            Side::Left => self.rect.x0,
            Side::Top => self.rect.y0,
            Side::Bottom => self.rect.y1,
        }
    }

    /// The window's centre — the side centre whenever margins fit.
    pub fn centre(&self) -> f64 {
        (self.window.0 + self.window.1) / 2.0
    }
}

/// ROUTING.md Impossible layouts — the stray reasons, one per failure shape.
const NO_ROUTE: &str = "no legal route: every side entry or channel is closed at this layout";
const ONE_SIDE_LOOP: &str = "self-loop with both ends forced onto one side";
/// ROUTING.md Fixed ports — infeasibility is loud, never a clamp.
const FIXED_PORT_BLOCKED: &str = "fixed port blocked: a body covers the port's landing";
const FIXED_PORTS_TOO_CLOSE: &str = "fixed ports closer than the minimum pitch on one side";
const FAN_PORT_CONFLICT: &str = "fan ends carry two different fixed ports";

/// Self-loop side resolution (ROUTING.md Special nodes): defaults
/// right → top; a forced side wins and its free partner takes the default
/// that stays adjacent; one shared side is invalid (natural draws it
/// anyway — its same-side loop is a lawful smooth curve).
pub(crate) fn self_loop_sides(a: Option<Side>, b: Option<Side>) -> Option<(Side, Side)> {
    let partner = |s: Side| {
        if s == Side::Top {
            Side::Right
        } else {
            Side::Top
        }
    };
    let (sa, sb) = match (a, b) {
        (None, None) => (Side::Right, Side::Top),
        (Some(s), None) => (s, partner(s)),
        (None, Some(s)) => (partner(s), s),
        (Some(sa), Some(sb)) => (sa, sb),
    };
    (sa != sb).then_some((sa, sb))
}

/// What one bundle's search settled on: the world it routed in, the winning
/// route, and the two ends' offered entries (the route indexes into them).
struct Solved {
    w: usize,
    route: search::Route,
    starts: Vec<Entry>,
    goals: Vec<Entry>,
}

/// One bundle's walk down the world ladder (ROUTING.md model step 4) under a
/// given pair of forced sides: entries per side, weighted search, whole-run
/// admission, retrying one world up when the inner one has no legal route.
///
/// It reads the committed state and mutates nothing, so the fan's side
/// pricing ([`fan_side`]) can ask the same question of every sibling before
/// any of them commits — one mechanism answers "what would this bundle do?",
/// whether the caller means to keep the answer or only to price it. The flag
/// alongside reports a fixed port no punch could reach, for the named stray.
#[allow(clippy::too_many_arguments)]
fn solve(
    index: &SceneIndex,
    worlds: &[World],
    chains: &[Option<Chain>],
    ledger: &Ledger,
    rep: &EdgeReq,
    link: usize,
    forced: [Option<Side>; 2],
    fan: [Option<usize>; 2],
    fan_pick: &[Option<Side>],
    fan_landed: &[bool],
    k: usize,
    c: f64,
) -> (Option<Solved>, bool) {
    let self_loop = rep.a_path == rep.b_path;
    // Members fanned at both ends are literal duplicates riding one drawn
    // line: they occupy a single track and a single port pair.
    let k_eff = if fan[0].is_some() && fan[1].is_some() {
        1
    } else {
        k
    };
    // The trunks this bundle rides: a sibling's committed lead is the very
    // line this wire draws out of the shared port, so the ledger is read
    // without it (ROUTING.md Special nodes).
    let trunks: Vec<usize> = fan.into_iter().flatten().collect();
    let held = ledger.read(&trunks);

    let a_contains_b = index.geo_contains(&rep.a_path, &rep.b_path);
    let b_contains_a = index.geo_contains(&rep.b_path, &rep.a_path);
    let solids = index.solid_rects_for([&rep.a_path, &rep.b_path]);
    let base: Vec<Rect> = solids.iter().map(|r| r.inflate(c)).collect();

    let mut fixed_blocked = false;
    // Innermost world first; a transparent ancestor lets the link route one
    // world up when the inner one has no legal route.
    for wkey in world_ladder(index, &rep.a_path, &rep.b_path) {
        let w = worlds
            .iter()
            .position(|x| x.key == wkey)
            .expect("world built");
        let graph = &worlds[w].graph;
        let end_entries = |path: &str,
                           rect: Rect,
                           stub: f64,
                           inward: bool,
                           partner: (Rect, bool),
                           fan: Option<usize>,
                           forced: Option<Side>,
                           fixed: Option<f64>| {
            let mut blockers = base.clone();
            // The partner's body walls this end in — unless it IS this end's
            // own body: two distinct fixed ports on one rect (two pins of one
            // part) are a lawful pair, and an end is never blocked by itself
            // (ROUTING.md Fixed ports).
            if !partner.1 && !self_loop && partner.0 != rect {
                blockers.push(partner.0.inflate(c));
            }
            let forced = fan.and_then(|g| fan_pick[g]).map_or(forced, Some);
            // A side must hold the whole landing: k ports, one for a fan
            // group (its side is settled for the group and its port slot
            // spent by the first sibling to land, costing nothing after).
            let need = match fan {
                Some(g) => usize::from(!fan_landed[g]),
                None => k,
            };
            let offered = entry::entries(graph, rect, stub, c, forced, fixed, &blockers, inward);
            let any = !offered.is_empty();
            let kept = offered
                .into_iter()
                .filter(|e| need == 0 || ledger.side_free(path, e.side, rect) >= need)
                .collect::<Vec<Entry>>();
            (kept, any)
        };
        let (starts, starts_any) = end_entries(
            &rep.a_path,
            rep.a_rect,
            rep.stub_a,
            a_contains_b,
            (rep.b_rect, b_contains_a),
            fan[0],
            forced[0],
            rep.port_a,
        );
        let (goals, goals_any) = end_entries(
            &rep.b_path,
            rep.b_rect,
            rep.stub_b,
            b_contains_a,
            (rep.a_rect, a_contains_b),
            fan[1],
            forced[1],
            rep.port_b,
        );
        if starts.is_empty() || goals.is_empty() {
            // A fixed port whose landing no punch can reach — covered by a
            // keep-out, or off its side — is a named failure, not a generic
            // closure (ROUTING.md Fixed ports).
            fixed_blocked |=
                (rep.port_a.is_some() && !starts_any) || (rep.port_b.is_some() && !goals_any);
            continue;
        }
        // Admission runs whole-span: the search prices edge by edge, but a
        // merged run needs one ordinate lawful over its entire travel — its
        // corridor's intersection, which a junction-fed edge can overstate. A
        // failed run's span becomes a learned closure and the same world
        // searches again around it, until a route holds whole or the world is
        // exhausted — never an unlawful squeeze.
        let mut deny: Vec<search::Deny> = Vec::new();
        let mut last: Option<search::Route> = None;
        while let Some(route) = search::cheapest(graph, w, &starts, &goals, &held, &deny, k_eff, c)
        {
            // A closure that changed nothing (an end run's channel no edge
            // consults) can't make progress; neither can unbounded learning.
            // Both give up on the world, honestly.
            if deny.len() > 32 || last.as_ref() == Some(&route) {
                break;
            }
            let (se, ge) = (&starts[route.start], &goals[route.goal]);
            // A wire never crosses itself: two punches that meet — a start
            // run driven through a transparent ancestor across the goal's —
            // draw one wire over its own end, which no placement can undo.
            // Closing the start's stub sends the search to another entry, or
            // honestly out of this world.
            if punches_cross(se, ge) {
                let (a, b) = punch_span(se);
                let chan = match se.axis {
                    Axis::H => graph.cells[se.cell].h,
                    Axis::V => graph.cells[se.cell].v,
                };
                deny.push((se.axis, chan, (a.min(b), a.max(b))));
                last = Some(route);
                continue;
            }
            let ends =
                [(se, &rep.a_rect, fan[0]), (ge, &rep.b_rect, fan[1])].map(|(e, r, fan)| EndInfo {
                    side: e.side,
                    rect: *r,
                    window: e.window,
                    fan,
                });
            let probe =
                geometry::chain(graph, w, &held, &route.cells, se, ge, ends, link, k_eff, c);
            let blocked = probe
                .runs
                .iter()
                .find(|run| held.tracks_left(w, run.axis, run.chan, run.span, graph) < k_eff)
                .map(|run| (run.axis, run.chan, run.span))
                .or_else(|| admit::admits(worlds, chains, &probe, k_eff, c));
            match blocked {
                None => {
                    return (
                        Some(Solved {
                            w,
                            route,
                            starts,
                            goals,
                        }),
                        fixed_blocked,
                    );
                }
                Some(run) => {
                    deny.push(run);
                    last = Some(route);
                }
            }
        }
    }
    (None, fixed_blocked)
}

/// An entry's punch along its travel axis: port to tip.
fn punch_span(e: &Entry) -> (f64, f64) {
    match e.axis {
        Axis::H => (e.port.0, e.tip.0),
        Axis::V => (e.port.1, e.tip.1),
    }
}

/// Whether two entries' punches (port → tip, each on its own axis) **cross**:
/// perpendicular, each one's ordinate strictly inside the other's travel.
/// Punches that merely meet — tip to tip at a corner, or a containment
/// link's two collinear stubs joining end to end — are the wire's own
/// shape and never cross.
fn punches_cross(a: &Entry, b: &Entry) -> bool {
    let inside = |v: f64, (p, q): (f64, f64)| p.min(q) < v && v < p.max(q);
    match (a.axis, b.axis) {
        (Axis::H, Axis::V) => inside(b.port.0, punch_span(a)) && inside(a.port.1, punch_span(b)),
        (Axis::V, Axis::H) => inside(a.port.0, punch_span(b)) && inside(b.port.1, punch_span(a)),
        _ => false,
    }
}

/// Keep a bundle's won route: the landing sides (a fan group's shared port
/// counts once, when its first sibling lands, and the side it lands on stands
/// for the group), a chain per member, and the route's runs in the ledger —
/// each carrying the fan group whose trunk it draws, so the ledger counts one
/// line once however many siblings ride it.
///
/// The fan pricing ([`fan_side`]) commits through this same function into
/// throwaway state, so a candidate side is judged against the load its own
/// earlier siblings really lay down, not against the fan's absence.
#[allow(clippy::too_many_arguments)]
fn commit_bundle(
    worlds: &[World],
    reqs: &[EdgeReq],
    fans: &request::Fans,
    members: &[usize],
    solved: &Solved,
    fan: [Option<usize>; 2],
    k: usize,
    c: f64,
    ledger: &mut Ledger,
    chains: &mut [Option<Chain>],
    fan_pick: &mut [Option<Side>],
    fan_landed: &mut [bool],
) {
    let Solved {
        w,
        route,
        starts,
        goals,
    } = solved;
    let (w, m0) = (*w, members[0]);
    let rep = &reqs[m0];
    let self_loop = rep.a_path == rep.b_path;
    let k_eff = if fan[0].is_some() && fan[1].is_some() {
        1
    } else {
        k
    };
    let trunks: Vec<usize> = fan.into_iter().flatten().collect();
    let (se, ge) = (&starts[route.start], &goals[route.goal]);

    for (entry, fan, path) in [(se, fan[0], &rep.a_path), (ge, fan[1], &rep.b_path)] {
        match fan {
            Some(g) if fan_landed[g] => {}
            Some(g) => {
                fan_pick[g] = Some(entry.side);
                fan_landed[g] = true;
                ledger.commit_port(path, entry.side, 1);
            }
            None => ledger.commit_port(path, entry.side, k),
        }
    }

    // Every member rides the one route — reversed for members declared
    // against the bundle's representative direction.
    for &m in members {
        let mreq = &reqs[m];
        let flipped = !self_loop && mreq.a_path == rep.b_path;
        let (es, eg) = if flipped { (ge, se) } else { (se, ge) };
        let cells: Vec<usize> = if flipped {
            route.cells.iter().rev().copied().collect()
        } else {
            route.cells.clone()
        };
        let ends = [(End::A, es), (End::B, eg)].map(|(end, e)| EndInfo {
            side: e.side,
            rect: match end {
                End::A => mreq.a_rect,
                End::B => mreq.b_rect,
            },
            window: e.window,
            fan: if self_loop {
                None
            } else {
                fans.group_at(m, end)
            },
        });
        chains[m] = Some(geometry::chain(
            &worlds[w].graph,
            w,
            &ledger.read(&trunks),
            &cells,
            es,
            eg,
            ends,
            m,
            k_eff,
            c,
        ));
    }
    let chain = chains[m0].as_ref().expect("chain built");
    // An end run pinned to a fixed port is booked **at that port** — read
    // through placement's own preference reader, so the ledger and the
    // drawing cannot disagree; every other run is booked at its corridor's
    // anchor, the one estimate a free run has before its ladder settles.
    let prefs = place::chain_prefs(chain, worlds);
    for (ri, run) in chain.runs.iter().enumerate() {
        let pinned = prefs[ri].1.filter(|w| w.0 == w.1).map(|w| w.0);
        ledger.commit_run(
            w,
            run.axis,
            run.chan,
            run.span,
            k_eff,
            &worlds[w].graph,
            pinned,
            cluster::fan_of(chain, ri),
        );
    }
}

/// A fan's shared side (ROUTING.md Special nodes): the permitted side of least
/// **fan total** — the siblings routed in declaration order under that side,
/// each committing as it wins, their costs summed. The trunk is one drawn
/// line, so the side it leaves on is the fan's decision, not the lead
/// sibling's private one; pricing it per sibling is how a lead saves one turn
/// and charges its siblings three.
///
/// The siblings commit into throwaway state through the same
/// [`commit_bundle`] the real pass uses, so the branches a candidate side
/// sends down one corridor are priced against each other, not through each
/// other.
///
/// `None` means no single side serves the whole group (or the group is one
/// bundle, which is just a link): the caller falls back to the lead sibling's
/// own free choice, and the rest follow it. Ties break on the fixed side rank
/// — `Side::RANK` order with a strict improvement test — then, inside a side,
/// on the search's own tie-break, so Law 4 is untouched.
#[allow(clippy::too_many_arguments)]
fn fan_side(
    index: &SceneIndex,
    worlds: &[World],
    chains: &[Option<Chain>],
    ledger: &Ledger,
    reqs: &[EdgeReq],
    bundles: &[request::Bundle],
    fans: &request::Fans,
    reasons: &[Option<&'static str>],
    fan_pick: &[Option<Side>],
    fan_landed: &[bool],
    group: usize,
    c: f64,
) -> Option<Side> {
    // One entry per bundle holding a member of the group: its representative
    // and the bundle it speaks for.
    let mut sharers: Vec<(usize, usize)> = Vec::new();
    for (b, bundle) in bundles.iter().enumerate() {
        let m0 = bundle.members[0];
        if reasons[m0].is_some() || reqs[m0].a_path == reqs[m0].b_path {
            continue;
        }
        let Some(end) = [End::A, End::B]
            .into_iter()
            .find(|&e| fans.group_at(m0, e) == Some(group))
        else {
            continue;
        };
        // A forced side prunes to one (model step 4), and a fan's key carries
        // that side, so every sharer agrees: there is nothing to price, and
        // the fan may never talk its members off a side they were given.
        if reqs[m0].side(end).is_some() {
            return None;
        }
        sharers.push((b, m0));
    }
    if sharers.len() < 2 {
        return None;
    }
    let mut best: Option<(f64, Side)> = None;
    for side in Side::RANK {
        // Route the whole group with the shared side pinned here, in
        // declaration order, against state that starts where the real pass
        // stands. One sibling with no route rules the side out for the fan.
        let mut pick = fan_pick.to_vec();
        pick[group] = Some(side);
        let mut landed = fan_landed.to_vec();
        let mut trial_ledger = ledger.clone();
        let mut trial_chains = chains.to_vec();
        let mut total = 0.0;
        for &(b, m0) in &sharers {
            let rep = &reqs[m0];
            let fan = [fans.group_at(m0, End::A), fans.group_at(m0, End::B)];
            let k = bundles[b].members.len();
            let (solved, _) = solve(
                index,
                worlds,
                &trial_chains,
                &trial_ledger,
                rep,
                m0,
                [rep.side_a, rep.side_b],
                fan,
                &pick,
                &landed,
                k,
                c,
            );
            let Some(solved) = solved else {
                total = f64::INFINITY;
                break;
            };
            total += solved.route.cost;
            commit_bundle(
                worlds,
                reqs,
                fans,
                &bundles[b].members,
                &solved,
                fan,
                k,
                c,
                &mut trial_ledger,
                &mut trial_chains,
                &mut pick,
                &mut landed,
            );
        }
        if total.is_finite() && best.is_none_or(|(bt, _)| total < bt) {
            best = Some((total, side));
        }
    }
    best.map(|(_, s)| s)
}

fn impossible(req: &EdgeReq, detail: &str) -> Violation {
    Violation {
        rule: Rule::Impossible,
        severity: Severity::Warning,
        links: vec![format!("{} -> {}", req.a_path, req.b_path)],
        detail: detail.to_owned(),
        span: req.span,
    }
}

/// Route the orthogonal requests over the placed scene — the six steps, in
/// order, one decision each: worlds and their channel graphs, bundles in
/// declaration order through the weighted search (committing to the ledger
/// as they win), placement over all chains at once, then geometry. Requests
/// of other strategies pass through untouched — their drivers, the shared
/// label pass, and the crossing report live with the dispatch
/// ([`crate::routing::route`]). Returns the drawn links' request indices
/// alongside, for that label pass.
pub(crate) fn route(index: &SceneIndex, reqs: &[EdgeReq]) -> (Routing, Vec<usize>) {
    let mut routing = Routing::default();
    if !reqs.iter().any(|r| r.routing == Strategy::Orthogonal) {
        return (routing, Vec::new());
    }
    // The diagram routes at the maximum clearance any link carries
    // (ROUTING.md Vocabulary); `build_worlds` spends it on keep-outs and the margin.
    let c = reqs
        .iter()
        .filter(|r| r.routing == Strategy::Orthogonal)
        .map(|r| r.clearance)
        .fold(0.0_f64, f64::max);
    let worlds = build_worlds(index, reqs, c);

    let fans = request::fan_groups(reqs, Strategy::Orthogonal);
    let bundles = request::bundles(reqs);
    // A fan group's settled shared side, and whether its one port slot has
    // been spent by the sibling that landed first (ROUTING.md Special nodes).
    let mut fan_pick: Vec<Option<Side>> = vec![None; fans.groups.len()];
    let mut fan_landed: Vec<bool> = vec![false; fans.groups.len()];
    let mut ledger = Ledger::new(c);
    let mut chains: Vec<Option<Chain>> = Vec::new();
    chains.resize_with(reqs.len(), || None);
    let mut reasons: Vec<Option<&'static str>> = vec![None; reqs.len()];

    // A fan whose shared end carries two different fixed ports is impossible
    // by construction (ROUTING.md Fixed ports): every member strays, named.
    for (g, members) in fans.groups.iter().enumerate() {
        let mut ports = members.iter().filter_map(|&i| {
            fans.of[i]
                .iter()
                .find(|(og, _)| *og == g)
                .and_then(|&(_, end)| reqs[i].port(end))
        });
        let Some(first) = ports.next() else { continue };
        if ports.any(|p| p != first) {
            for &m in members {
                reasons[m] = Some(FAN_PORT_CONFLICT);
            }
        }
    }
    // Committed fixed-port landings per (path, side): the too-close check
    // below judges a later fixed port against them by ordinate — ports come
    // from one connection-geometry computation, so equality is exact and
    // means the shared-port fan, never a collision.
    let mut landed_ports: Vec<(String, u8, f64)> = Vec::new();

    for bundle in &bundles {
        if bundle.members.iter().any(|&m| reasons[m].is_some()) {
            continue;
        }
        let m0 = bundle.members[0];
        let rep = &reqs[m0];
        let k = bundle.members.len();
        let self_loop = rep.a_path == rep.b_path;

        // The later of two fixed ports closer than the minimum pitch strays,
        // named (ROUTING.md Fixed ports) — the earlier landing stands.
        let ends_fixed = [
            (rep.side_a, rep.port_a, &rep.a_path),
            (rep.side_b, rep.port_b, &rep.b_path),
        ];
        let too_close = ends_fixed.iter().any(|(side, port, path)| {
            let (Some(side), Some(p)) = (side, port) else {
                return false;
            };
            landed_ports.iter().any(|(lp, ls, lo)| {
                lp == *path && *ls == side.index() && *lo != *p && (*lo - *p).abs() < min_pitch(c)
            })
        });
        if too_close {
            for &m in &bundle.members {
                reasons[m] = Some(FIXED_PORTS_TOO_CLOSE);
            }
            continue;
        }

        let forced = if self_loop {
            match self_loop_sides(rep.side_a, rep.side_b) {
                Some((sa, sb)) => [Some(sa), Some(sb)],
                None => {
                    reasons[m0] = Some(ONE_SIDE_LOOP);
                    continue;
                }
            }
        } else {
            [rep.side_a, rep.side_b]
        };
        let (fan_a, fan_b) = if self_loop {
            (None, None)
        } else {
            (fans.group_at(m0, End::A), fans.group_at(m0, End::B))
        };
        let fan = [fan_a, fan_b];
        let trunks: Vec<usize> = fan.into_iter().flatten().collect();

        // ROUTING.md Special nodes: a fan's shared side belongs to the fan,
        // not to whichever sibling routes first — settle it here, once, by
        // pricing every permitted side over the whole group before any member
        // commits. An unsettled group falls through to the lead's own choice.
        for g in trunks.iter().copied() {
            if fan_pick[g].is_none() {
                fan_pick[g] = fan_side(
                    index,
                    &worlds,
                    &chains,
                    &ledger,
                    reqs,
                    &bundles,
                    &fans,
                    &reasons,
                    &fan_pick,
                    &fan_landed,
                    g,
                    c,
                );
            }
        }

        let (picked, fixed_blocked) = solve(
            index,
            &worlds,
            &chains,
            &ledger,
            rep,
            m0,
            forced,
            fan,
            &fan_pick,
            &fan_landed,
            k,
            c,
        );
        let Some(Solved {
            w,
            route,
            starts,
            goals,
        }) = picked
        else {
            let why = if fixed_blocked {
                FIXED_PORT_BLOCKED
            } else {
                NO_ROUTE
            };
            for &m in &bundle.members {
                reasons[m] = Some(why);
            }
            continue;
        };

        // A fixed landing records its ordinate for the too-close check above.
        let (se, ge) = (&starts[route.start], &goals[route.goal]);
        for (entry, path, port) in [(se, &rep.a_path, rep.port_a), (ge, &rep.b_path, rep.port_b)] {
            if let Some(p) = port {
                landed_ports.push((path.to_string(), entry.side.index(), p));
            }
        }
        commit_bundle(
            &worlds,
            reqs,
            &fans,
            &bundle.members,
            &Solved {
                w,
                route,
                starts,
                goals,
            },
            fan,
            k,
            c,
            &mut ledger,
            &mut chains,
            &mut fan_pick,
            &mut fan_landed,
        );
    }

    place::place(&worlds, &mut chains, c);

    let mut req_of = Vec::new();
    for (i, req) in reqs.iter().enumerate() {
        if req.routing != Strategy::Orthogonal {
            continue;
        }
        let Some(chain) = &chains[i] else {
            routing
                .report
                .push(impossible(req, reasons[i].unwrap_or(NO_ROUTE)));
            if let Some((from, to)) = geometry::stray_segment(req.a_rect, req.b_rect) {
                routing.strays.push(Stray {
                    from,
                    to,
                    data_from: req.data_from.clone(),
                    data_to: req.data_to.clone(),
                });
            }
            continue;
        };
        req_of.push(i);
        routing.links.push(RoutedLink {
            path: geometry::polyline(chain),
            curve: Vec::new(),
            strategy: req.routing,
            markers: req.markers.clone(),
            attrs: req.attrs.clone(),
            applied_styles: req.applied_styles.clone(),
            sheet: req.sheet,
            texts: Vec::new(),
            data_from: req.data_from.clone(),
            data_to: req.data_to.clone(),
            seg_from: req.a_path.clone(),
            seg_to: req.b_path.clone(),
            decl_span: req.span,
            fan_from: fans.group_at(i, End::A).map(|g| g as u32),
            fan_to: fans.group_at(i, End::B).map(|g| g as u32),
            port_from: req.port_a.map(|p| (chain.ends[0].side, p)),
            port_to: req.port_b.map(|p| (chain.ends[1].side, p)),
        });
    }

    (routing, req_of)
}

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
    let mut fan_pick: Vec<Option<Side>> = vec![None; fans.groups.len()];
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
        // Members fanned at both ends are literal duplicates riding one
        // drawn line: they occupy a single track and a single port pair.
        let k_eff = if fan_a.is_some() && fan_b.is_some() {
            1
        } else {
            k
        };

        let a_contains_b = index.geo_contains(&rep.a_path, &rep.b_path);
        let b_contains_a = index.geo_contains(&rep.b_path, &rep.a_path);
        let solids = index.solid_rects_for([&rep.a_path, &rep.b_path]);
        let base: Vec<Rect> = solids.iter().map(|r| r.inflate(c)).collect();

        // Innermost world first; a transparent ancestor lets the link route
        // one world up when the inner one has no legal route.
        let mut picked = None;
        let mut fixed_blocked = false;
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
                if !partner.1 && !self_loop {
                    blockers.push(partner.0.inflate(c));
                }
                let forced = fan.and_then(|g| fan_pick[g]).map_or(forced, Some);
                // A side must hold the whole landing: k ports, one for a fan
                // group (its side is bound by the first-routed sibling and
                // costs nothing once landed).
                let need = match fan {
                    Some(g) => usize::from(fan_pick[g].is_none()),
                    None => k,
                };
                let offered =
                    entry::entries(graph, rect, stub, c, forced, fixed, &blockers, inward);
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
                fan_a,
                forced[0],
                rep.port_a,
            );
            let (goals, goals_any) = end_entries(
                &rep.b_path,
                rep.b_rect,
                rep.stub_b,
                b_contains_a,
                (rep.a_rect, a_contains_b),
                fan_b,
                forced[1],
                rep.port_b,
            );
            if starts.is_empty() || goals.is_empty() {
                // A fixed port whose landing no punch can reach — covered by
                // a keep-out, or off its side — is a named failure, not a
                // generic closure (ROUTING.md Fixed ports).
                fixed_blocked |=
                    (rep.port_a.is_some() && !starts_any) || (rep.port_b.is_some() && !goals_any);
                continue;
            }
            // Admission runs whole-span: the search prices edge by edge, but
            // a merged run needs one ordinate lawful over its entire travel
            // — its corridor's intersection, which a junction-fed edge can
            // overstate. A failed run's span becomes a learned closure and
            // the same world searches again around it, until a route holds
            // whole or the world is exhausted — never an unlawful squeeze.
            let mut deny: Vec<search::Deny> = Vec::new();
            let mut last: Option<search::Route> = None;
            while let Some(route) =
                search::cheapest(graph, w, &starts, &goals, &ledger, &deny, k_eff, c)
            {
                // A closure that changed nothing (an end run's channel no
                // edge consults) can't make progress; neither can unbounded
                // learning. Both give up on the world, honestly.
                if deny.len() > 32 || last.as_ref() == Some(&route) {
                    break;
                }
                let (se, ge) = (&starts[route.start], &goals[route.goal]);
                let ends =
                    [(se, &rep.a_rect, fan_a), (ge, &rep.b_rect, fan_b)].map(|(e, r, fan)| {
                        EndInfo {
                            side: e.side,
                            rect: *r,
                            window: e.window,
                            fan,
                        }
                    });
                let probe =
                    geometry::chain(graph, w, &ledger, &route.cells, se, ge, ends, m0, k_eff, c);
                let blocked = probe
                    .runs
                    .iter()
                    .find(|run| ledger.tracks_left(w, run.axis, run.chan, run.span, graph) < k_eff)
                    .map(|run| (run.axis, run.chan, run.span))
                    .or_else(|| admit::admits(&worlds, &chains, &probe, k_eff, c));
                match blocked {
                    None => {
                        picked = Some((w, route, starts, goals));
                        break;
                    }
                    Some(run) => {
                        deny.push(run);
                        last = Some(route);
                    }
                }
            }
            if picked.is_some() {
                break;
            }
        }
        let Some((w, route, starts, goals)) = picked else {
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

        // Commit the landing sides: a fan group's shared port counts once,
        // when its first sibling routes. A fixed landing also records its
        // ordinate for the too-close check above.
        let (se, ge) = (&starts[route.start], &goals[route.goal]);
        for (entry, fan, path, port) in [
            (se, fan_a, &rep.a_path, rep.port_a),
            (ge, fan_b, &rep.b_path, rep.port_b),
        ] {
            if let Some(p) = port {
                landed_ports.push((path.to_string(), entry.side.index(), p));
            }
            match fan {
                Some(g) if fan_pick[g].is_some() => {}
                Some(g) => {
                    fan_pick[g] = Some(entry.side);
                    ledger.commit_port(path, entry.side, 1);
                }
                None => ledger.commit_port(path, entry.side, k),
            }
        }

        // Every member rides the one route — reversed for members declared
        // against the bundle's representative direction.
        for &m in &bundle.members {
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
                &ledger,
                &cells,
                es,
                eg,
                ends,
                m,
                k_eff,
                c,
            ));
        }
        for run in &chains[m0].as_ref().expect("chain built").runs {
            ledger.commit_run(w, run.axis, run.chan, run.span, k_eff, &worlds[w].graph);
        }
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

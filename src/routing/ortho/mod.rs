//! The `orthogonal` strategy — ROUTING.md's six-step model: keep-outs &
//! worlds → channels → requests → weighted search → placement → geometry.
//! Each step decides once; none revisits an earlier step's answer.

pub(crate) mod cost;
pub(crate) mod geometry;
pub(crate) mod graph;
pub(crate) mod labels;
pub(crate) mod ladder;
pub(crate) mod ledger;
pub(crate) mod order;
pub(crate) mod place;
pub(crate) mod rect;
pub(crate) mod request;
pub(crate) mod scene;
pub(crate) mod search;

use crate::ast::Side;
use crate::layout::ir::{RoutedLink, Stray};
use crate::resolve::Program;
use crate::routing::{Routing, Rule, Severity, Violation, cross};

use graph::{Axis, ChannelGraph};
use ledger::Ledger;
use rect::Rect;
use request::{EdgeReq, End};
use scene::SceneIndex;
use search::Entry;

/// One routing world: a container's interior (`""` = the scene root) and its
/// channel decomposition.
pub(crate) struct World {
    pub path: String,
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

/// ROUTING.md §Impossible layouts — the stray reasons, one per failure shape.
const NO_ROUTE: &str = "no legal route: every side entry or channel is closed at this layout";
const ONE_SIDE_LOOP: &str = "self-loop with both ends forced onto one side";

fn parent_path(p: &str) -> String {
    p.rfind('.').map(|i| p[..i].to_owned()).unwrap_or_default()
}

/// The worlds an edge may route in, innermost first: the endpoints' common
/// container, then every transparent ancestor up to the scene root — a tight
/// interior never walls in a link its ancestors would let out. Containment
/// links stay inside their container.
fn world_ladder(a: &str, b: &str) -> Vec<String> {
    if SceneIndex::contains(a, b) || SceneIndex::contains(b, a) {
        return vec![SceneIndex::world_of(a, b)];
    }
    let mut w = SceneIndex::world_of(a, b);
    let mut out = vec![w.clone()];
    while !w.is_empty() {
        w = parent_path(&w);
        out.push(w.clone());
    }
    out
}

/// Self-loop side resolution (ROUTING.md §Special nodes): defaults
/// right → top; a forced side wins and its free partner takes the default
/// that stays adjacent; one shared side is invalid.
fn self_loop_sides(a: Option<Side>, b: Option<Side>) -> Option<(Side, Side)> {
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

/// Route every request over the placed scene — the six steps, in order, one
/// decision each: worlds and their channel graphs, bundles in declaration
/// order through the weighted search (committing to the ledger as they win),
/// placement over all chains at once, then geometry, labels, and the report.
pub(crate) fn route(program: &Program, index: &SceneIndex, reqs: &[EdgeReq]) -> Routing {
    let mut routing = Routing::default();
    if reqs.is_empty() {
        return routing;
    }
    // The diagram routes at the maximum clearance any link carries
    // (ROUTING.md §Vocabulary); the root world gets a canvas margin.
    let c = reqs.iter().map(|r| r.clearance).fold(0.0_f64, f64::max);
    let bounds = index.bounds().inflate(2.0 * c + 20.0);

    let mut world_paths: Vec<String> = reqs
        .iter()
        .flat_map(|r| world_ladder(&r.a_path, &r.b_path))
        .collect();
    world_paths.sort();
    world_paths.dedup();
    let worlds: Vec<World> = world_paths
        .into_iter()
        .map(|path| {
            let wb = if path.is_empty() {
                bounds
            } else {
                index.rect(&path).expect("world body placed")
            };
            let keepouts: Vec<Rect> = index
                .child_rects(&path)
                .iter()
                .map(|r| r.inflate(c))
                .collect();
            let graph = ChannelGraph::build(wb, &keepouts, path.is_empty());
            World { path, graph }
        })
        .collect();

    let fans = request::fan_groups(reqs);
    let bundles = request::bundles(reqs);
    let mut fan_pick: Vec<Option<Side>> = vec![None; fans.groups.len()];
    let mut ledger = Ledger::new(c);
    let mut chains: Vec<Option<Chain>> = Vec::new();
    chains.resize_with(reqs.len(), || None);
    let mut reasons: Vec<Option<&'static str>> = vec![None; reqs.len()];

    for bundle in &bundles {
        let m0 = bundle.members[0];
        let rep = &reqs[m0];
        let k = bundle.members.len();
        let self_loop = rep.a_path == rep.b_path;

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

        let a_contains_b = SceneIndex::contains(&rep.a_path, &rep.b_path);
        let b_contains_a = SceneIndex::contains(&rep.b_path, &rep.a_path);
        let solids = index.solid_rects_for([&rep.a_path, &rep.b_path]);
        let base: Vec<Rect> = solids.iter().map(|r| r.inflate(c)).collect();

        // Innermost world first; a transparent ancestor lets the link route
        // one world up when the inner one has no legal route.
        let mut picked = None;
        for wpath in world_ladder(&rep.a_path, &rep.b_path) {
            let w = worlds
                .iter()
                .position(|x| x.path == wpath)
                .expect("world built");
            let graph = &worlds[w].graph;
            let end_entries = |path: &str,
                               rect: Rect,
                               stub: f64,
                               inward: bool,
                               partner: (Rect, bool),
                               fan: Option<usize>,
                               forced: Option<Side>| {
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
                let offered = search::entries(graph, rect, stub, c, forced, &blockers, inward);
                offered
                    .into_iter()
                    .filter(|e| need == 0 || ledger.side_free(path, e.side, rect) >= need)
                    .collect::<Vec<Entry>>()
            };
            let starts = end_entries(
                &rep.a_path,
                rep.a_rect,
                rep.stub_a,
                a_contains_b,
                (rep.b_rect, b_contains_a),
                fan_a,
                forced[0],
            );
            let goals = end_entries(
                &rep.b_path,
                rep.b_rect,
                rep.stub_b,
                b_contains_a,
                (rep.a_rect, a_contains_b),
                fan_b,
                forced[1],
            );
            if starts.is_empty() || goals.is_empty() {
                continue;
            }
            if let Some(route) = search::cheapest(graph, w, &starts, &goals, &ledger, k_eff, c) {
                picked = Some((w, route, starts, goals));
                break;
            }
        }
        let Some((w, route, starts, goals)) = picked else {
            for &m in &bundle.members {
                reasons[m] = Some(NO_ROUTE);
            }
            continue;
        };

        // Commit the landing sides: a fan group's shared port counts once,
        // when its first sibling routes.
        let (se, ge) = (&starts[route.start], &goals[route.goal]);
        for (entry, fan, path) in [(se, fan_a, &rep.a_path), (ge, fan_b, &rep.b_path)] {
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
        });
    }

    // The exact crossing count over the final polylines — the estimate's
    // ground truth, counted once and reported as normal output.
    for i in 0..routing.links.len() {
        for j in i + 1..routing.links.len() {
            for sa in routing.links[i].path.windows(2) {
                for sb in routing.links[j].path.windows(2) {
                    if let Some(at) = cross(sa, sb) {
                        let name = |k: usize| {
                            let r = &reqs[req_of[k]];
                            format!("{} -> {}", r.a_path, r.b_path)
                        };
                        routing.report.push(Violation {
                            rule: Rule::Crossing,
                            severity: Severity::Info,
                            links: vec![name(i), name(j)],
                            detail: format!("forced crossing at ({}, {})", at.0, at.1),
                            span: reqs[req_of[j]].span,
                        });
                    }
                }
            }
        }
    }

    labels::place(&mut routing.links, &req_of, reqs, program, index);
    routing
}

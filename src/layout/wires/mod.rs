//! Wire routing — the orthogonal corridor router (`WIRING.md`, `PLAN.md`).
//!
//! Stages: requests → bundles → per-bundle path search with hard capacity
//! closure → per-channel run assignment (ports, ordinates, inversion swaps)
//! → crossing audit → polylines. Every edge routes in one **world** — the
//! channel graph of its endpoints' innermost common container — and reaches
//! it through straight punch stubs. The routing's report carries the kept
//! crossings and the wires no legal route exists for; [`validate`] re-judges
//! the drawn result against the four laws with no router knowledge.

mod audit;
/// The transversal-crossing primitive, shared with the renderer's fillet
/// pass (a crossing must never land mid-arc).
pub(crate) use audit::cross;
mod bundle;
mod capacity;
mod feedback;
mod geometry;
mod graph;
mod labels;
mod order;
mod path;
mod rect;
mod runs;
mod scene;
mod validate;

use crate::ast::Side;
use crate::error::Error;
use crate::layout::ir::{Airwire, PlacedNode, RoutedWire};
use crate::resolve::Program;
use crate::span::Span;

use bundle::{Bundle, EdgeReq, End, Fans};
use capacity::{Closure, Occupancy, Ports};
use graph::{Axis, ChannelGraph};
use path::Entry;
use rect::Rect;
use runs::{Chain, EndInfo, World};
use scene::SceneIndex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// Law 1 — a wire holds ≥ clearance from every node body.
    Clearance,
    /// Law 1 — a wire holds ≥ clearance from every other wire.
    Separation,
    /// Law 2 — every end lands perpendicular on a side, clear of corners.
    Contact,
    /// Law 3 — a kept, square-on crossing: counted output, not a defect.
    Crossing,
    /// A wire with no legal route at this layout: reported, never drawn.
    Impossible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// Surfaced as a diagnostic; `--strict` escalates it to an error.
    Warning,
    /// Normal, counted output (crossings).
    Info,
}

#[derive(Clone, Debug)]
pub struct Violation {
    pub rule: Rule,
    /// `Info` for the router's own counted output (kept crossings); `Warning`
    /// for everything the diagnostic layer must surface — impossible wires
    /// and any law breach the independent checker finds.
    pub severity: Severity,
    pub wires: Vec<String>,
    pub detail: String,
    /// The declaration this violation points back to.
    pub span: Span,
}

impl Rule {
    pub fn id(self) -> &'static str {
        match self {
            Rule::Clearance => "clearance",
            Rule::Separation => "separation",
            Rule::Contact => "contact",
            Rule::Crossing => "crossing",
            Rule::Impossible => "impossible",
        }
    }
}

/// Test-only hook: a node's absolute rect by full dot-path.
pub fn node_rect(nodes: &[PlacedNode], path: &str) -> Option<(f64, f64, f64, f64)> {
    let idx = scene::SceneIndex::build(nodes);
    idx.rect(path).map(|r| (r.x0, r.y0, r.x1, r.y1))
}

fn parent_path(p: &str) -> String {
    p.rfind('.').map(|i| p[..i].to_owned()).unwrap_or_default()
}

/// The worlds an edge may route in, innermost first: the endpoints' common
/// container, then every transparent ancestor up to the scene root — a tight
/// interior never walls a wire its ancestors would let out. Containment wires
/// stay inside their container.
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

fn side_centre(rect: Rect, side: Side) -> (f64, f64) {
    let cx = (rect.x0 + rect.x1) / 2.0;
    let cy = (rect.y0 + rect.y1) / 2.0;
    match side {
        Side::Right => (rect.x1, cy),
        Side::Bottom => (cx, rect.y1),
        Side::Left => (rect.x0, cy),
        Side::Top => (cx, rect.y0),
    }
}

/// The routing result: the drawn wires and the engine's report — the kept
/// crossings (counted output) and the wires it could not legally draw, each
/// of those drawn as an airwire (the report made visible).
#[derive(Default)]
pub struct Routing {
    pub wires: Vec<RoutedWire>,
    pub report: Vec<Violation>,
    pub airwires: Vec<Airwire>,
    /// Corridor deficits behind the impossible wires, per container dot-path
    /// (`""` = scene): `(Δgap_y, Δgap_x)` px the container's gap is short.
    /// Gap growth's feedback — empty when nothing is starved
    /// of lanes.
    pub starved: std::collections::BTreeMap<String, (f64, f64)>,
}

/// Why an edge stayed undrawn — set when a stage gives up on it, cleared
/// when a later lever draws it after all; whatever still holds a reason at
/// the end of the pipeline is reported impossible.
const NO_ROUTE: &str = "no legal route: every side entry or channel is closed at this layout";
const NO_CLEAN_ROUTE: &str = "no conflict-free route at this layout";

/// WIRING §Impossible layouts: a wire with no legal route is reported with
/// its source span, never drawn dirty. `--strict` escalates the warning.
fn impossible(req: &bundle::EdgeReq, detail: &str) -> Violation {
    Violation {
        rule: Rule::Impossible,
        severity: Severity::Warning,
        wires: vec![format!("{} -> {}", req.a_path, req.b_path)],
        detail: detail.to_owned(),
        span: req.span,
    }
}

/// Everything a route decision reads: the scene, the requests, the per-world
/// channel graphs, and the fan side picks. The audit reroutes through the
/// same methods the initial pass uses, so a reroute obeys every closure the
/// original route obeyed.
struct Router {
    index: SceneIndex,
    reqs: Vec<EdgeReq>,
    worlds: Vec<World>,
    fans: Fans,
    bundles: Vec<Bundle>,
    fan_pick: Vec<Option<Side>>,
    clearance: f64,
}

/// A bundle's chosen route: the world it runs in and the entries it used.
/// `margin` records compact-mode admission (open canvas walls).
struct Picked {
    world: usize,
    route: path::Route,
    starts: Vec<Entry>,
    goals: Vec<Entry>,
    margin: bool,
}

impl Router {
    fn world_id(&self, p: &str) -> usize {
        self.worlds
            .iter()
            .position(|w| w.path == p)
            .expect("world built")
    }

    /// Whether a routeless bundle may degrade (WIRING §Duplicates): more
    /// than one member, none of them fanned — a fan's siblings share one
    /// port and stub, which a split would tear apart.
    fn splittable(&self, bi: usize) -> bool {
        let b = &self.bundles[bi];
        b.members.len() > 1 && b.members.iter().all(|&m| self.fans.of[m].is_empty())
    }

    /// The cheapest legal route for a bundle under the given occupancy and
    /// port state — the world ladder innermost first, all four side stubs,
    /// hard capacity closure, cost `(crossings, length, turns)`. The initial
    /// pass counts no crossings ([`path::FREE`]); the audit counts them
    /// against every drawn wire. `deny` regions are off limits to stubs and
    /// runs alike and `avoid` drops one side per end (the separation audit
    /// routing away from a conflict its sides are pinned into); `relaxed`
    /// widens closure from the usable band to the walls (the rescue pass,
    /// gated on ground truth by the caller); `compact` unlocks Law 2's
    /// compaction clause for an end whose every side is past capacity —
    /// full sides reopen and the landing side re-pitches all its ports
    /// evenly below clearance (the completeness pass's last lever; side
    /// capacity stays hard during normal routing). A `probe` observes the
    /// search for gap growth's failure classification — entry sets per end,
    /// lane shortfalls per consulted channel — without changing any
    /// decision.
    #[allow(clippy::too_many_arguments)]
    fn route_bundle(
        &self,
        bi: usize,
        occ: &Occupancy,
        ports: &Ports,
        cross: path::CrossCount,
        deny: &[Rect],
        avoid: [Option<Side>; 2],
        relaxed: bool,
        compact: bool,
        probe: Option<&feedback::Probe>,
    ) -> Option<Picked> {
        let b = &self.bundles[bi];
        let m0 = b.members[0];
        let rep = &self.reqs[m0];
        let c = self.clearance;
        let k = b.members.len();
        let a_contains_b = SceneIndex::contains(&rep.a_path, &rep.b_path);
        let b_contains_a = SceneIndex::contains(&rep.b_path, &rep.a_path);
        let solids = self.index.solid_rects_for([&rep.a_path, &rep.b_path]);
        let base: Vec<Rect> = solids.iter().map(|r| r.inflate(c)).collect();

        let fan_a = self.fans.group_at(m0, End::A);
        let fan_b = self.fans.group_at(m0, End::B);
        let fan_tag = fan_a.or(fan_b);
        let full_dupe = fan_a.is_some() && fan_b.is_some();
        let k_eff = if full_dupe { 1 } else { k };

        // Innermost world first; a transparent ancestor lets the wire route
        // one world up when the inner one has no legal route.
        for wpath in world_ladder(&rep.a_path, &rep.b_path) {
            let w = self.world_id(&wpath);
            let graph = &self.worlds[w].graph;
            let end_entries = |end: End,
                               rect: Rect,
                               stub: f64,
                               inward: bool,
                               partner: Rect,
                               partner_transparent: bool,
                               fan: Option<usize>,
                               forced: Option<Side>,
                               avoid: Option<Side>| {
                let mut blockers = base.clone();
                if !partner_transparent {
                    blockers.push(partner.inflate(c));
                }
                blockers.extend_from_slice(deny);
                let forced = fan.and_then(|g| self.fan_pick[g]).map_or(forced, Some);
                let need = match fan {
                    Some(g) => usize::from(self.fan_pick[g].is_none()),
                    None => k,
                };
                let path_of = |e: End| match e {
                    End::A => &rep.a_path,
                    End::B => &rep.b_path,
                };
                let offered: Vec<Entry> =
                    path::entries(graph, rect, stub, forced, &blockers, inward)
                        .into_iter()
                        .filter(|en| avoid != Some(en.side))
                        .collect();
                let open: Vec<Entry> = offered
                    .iter()
                    .filter(|en| need == 0 || ports.free(path_of(end), en.side, rect) >= need)
                    .copied()
                    .collect();
                if let Some(p) = probe {
                    p.entries(end, !offered.is_empty());
                }
                if open.is_empty() && compact && fan.is_none() {
                    offered // every side full: the landing side will compact its port row
                } else {
                    open
                }
            };

            let starts = end_entries(
                End::A,
                rep.a_rect,
                rep.stub_a,
                a_contains_b,
                rep.b_rect,
                b_contains_a,
                fan_a,
                rep.side_a,
                avoid[0],
            );
            let goals = end_entries(
                End::B,
                rep.b_rect,
                rep.stub_b,
                b_contains_a,
                rep.a_rect,
                a_contains_b,
                fan_b,
                rep.side_b,
                avoid[1],
            );
            if starts.is_empty() || goals.is_empty() {
                continue;
            }

            let closed = |axis: Axis, chan: usize, lo: f64, hi: f64| {
                let channel = match axis {
                    Axis::H => &graph.h[chan],
                    Axis::V => &graph.v[chan],
                };
                match occ.closure(
                    channel, w, axis, chan, lo, hi, k_eff, fan_tag, relaxed, compact, deny,
                ) {
                    Closure::Open => false,
                    Closure::Short(lanes) => {
                        if let Some(p) = probe {
                            p.lanes_short(w, axis, lanes);
                        }
                        true
                    }
                    Closure::Hard => true,
                }
            };
            if let Some(route) = path::shortest(graph, &starts, &goals, &closed, cross) {
                return Some(Picked {
                    world: w,
                    route,
                    starts,
                    goals,
                    margin: compact,
                });
            }
        }
        None
    }

    /// Build every member's chain from a bundle's picked route. Pure chain
    /// construction — port and occupancy commits stay with the caller.
    fn build_chains(&self, bi: usize, picked: &Picked, chains: &mut [Option<Chain>]) {
        let b = &self.bundles[bi];
        let rep = &self.reqs[b.members[0]];
        let graph = &self.worlds[picked.world].graph;
        let (se, ge) = (
            &picked.starts[picked.route.start],
            &picked.goals[picked.route.goal],
        );
        for &m in &b.members {
            let mreq = &self.reqs[m];
            let flipped = mreq.a_path == rep.b_path;
            let (es, eg) = if flipped { (ge, se) } else { (se, ge) };
            let cells: Vec<usize> = if flipped {
                picked.route.cells.iter().rev().copied().collect()
            } else {
                picked.route.cells.clone()
            };
            let ends = [(End::A, es), (End::B, eg)].map(|(end, entry)| EndInfo {
                path: mreq.path(end).to_owned(),
                side: entry.side,
                rect: match end {
                    End::A => mreq.a_rect,
                    End::B => mreq.b_rect,
                },
                port: entry.port,
                fan: self.fans.group_at(m, end),
            });
            chains[m] = Some(geometry::chain(
                graph,
                picked.world,
                &cells,
                es,
                eg,
                ends,
                m,
                picked.margin,
            ));
        }
    }
}

/// Chains → drawn geometry: ports placed (with any accepted slides),
/// inversions realised, ordinates assigned — on a copy, so routing state
/// stays reusable for reroutes.
fn solve(
    worlds: &[World],
    raw: &[Option<Chain>],
    clearance: f64,
    slides: &runs::Slides,
) -> Vec<Option<Chain>> {
    let mut drawn = raw.to_vec();
    runs::assign(worlds, &mut drawn, clearance, slides);
    drawn
}

/// Commit a picked route: port slots (a fan group's shared port counts
/// once), the members' chains, and their channel occupancy.
fn commit_picked(
    router: &mut Router,
    occ: &mut Occupancy,
    ports: &mut Ports,
    raw: &mut [Option<Chain>],
    bi: usize,
    picked: &Picked,
) {
    let m0 = router.bundles[bi].members[0];
    let (se, ge) = (
        &picked.starts[picked.route.start],
        &picked.goals[picked.route.goal],
    );
    let k = router.bundles[bi].members.len();
    let fan_a = router.fans.group_at(m0, End::A);
    let fan_b = router.fans.group_at(m0, End::B);
    let (pa, pb) = (
        router.reqs[m0].a_path.clone(),
        router.reqs[m0].b_path.clone(),
    );
    for (entry, fan, path) in [(se, fan_a, &pa), (ge, fan_b, &pb)] {
        match fan {
            Some(g) => {
                if router.fan_pick[g].is_none() {
                    router.fan_pick[g] = Some(entry.side);
                    ports.commit(path, entry.side, 1);
                }
            }
            None => ports.commit(path, entry.side, k),
        }
    }
    router.build_chains(bi, picked, raw);
    for &m in &router.bundles[bi].members {
        occ.commit_chain(raw[m].as_ref().unwrap());
    }
}

pub fn route_wires(program: &Program, nodes: &[PlacedNode]) -> Result<Routing, Error> {
    let index = SceneIndex::build(nodes);
    let reqs = bundle::requests(program, &index)?;
    if reqs.is_empty() {
        return Ok(Routing::default());
    }
    let c = reqs.iter().map(|r| r.clearance).fold(0.0_f64, f64::max);
    let bounds = index.bounds().inflate(2.0 * c + 20.0);

    let mut world_paths: Vec<String> = reqs
        .iter()
        .flat_map(|r| {
            if r.a_path == r.b_path {
                vec![parent_path(&r.a_path)]
            } else {
                world_ladder(&r.a_path, &r.b_path)
            }
        })
        .collect();
    world_paths.sort();
    world_paths.dedup();
    let worlds: Vec<World> = world_paths
        .iter()
        .map(|p| {
            let wb = if p.is_empty() {
                bounds
            } else {
                index.rect(p).expect("world body placed")
            };
            let keepouts: Vec<Rect> = index.child_rects(p).iter().map(|r| r.inflate(c)).collect();
            World {
                path: p.clone(),
                graph: ChannelGraph::build(wb, &keepouts, p.is_empty()),
            }
        })
        .collect();

    let fans = bundle::fan_groups(&reqs);
    let mut router = Router {
        fan_pick: vec![None; fans.groups.len()],
        bundles: bundle::bundles(&reqs),
        index,
        reqs,
        worlds,
        fans,
        clearance: c,
    };
    let mut occ = Occupancy::new(c);
    let mut ports = Ports::new(c);
    let mut report: Vec<Violation> = Vec::new();
    let mut reasons: Vec<Option<&str>> = vec![None; router.reqs.len()];
    let mut raw: Vec<Option<Chain>> = Vec::new();
    raw.resize_with(router.reqs.len(), || None);

    let mut bi = 0;
    while bi < router.bundles.len() {
        let m0 = router.bundles[bi].members[0];
        let rep = &router.reqs[m0];

        if rep.a_path == rep.b_path {
            route_self_loop(&router, &mut occ, &mut ports, &mut raw, &mut report, bi);
            bi += 1;
            continue;
        }

        match router.route_bundle(
            bi,
            &occ,
            &ports,
            path::FREE,
            &[],
            [None, None],
            false,
            false,
            None,
        ) {
            Some(picked) => {
                commit_picked(&mut router, &mut occ, &mut ports, &mut raw, bi, &picked);
            }
            // No route holds the whole bundle at any rung of the world
            // ladder: degrade — halves, then singles — and retry the head
            // piece. Splitting beats vanishing (WIRING §Duplicates).
            None if router.splittable(bi) => {
                bundle::split(&mut router.bundles, bi);
                continue;
            }
            None => {
                for &m in &router.bundles[bi].members {
                    reasons[m] = Some(NO_ROUTE);
                }
            }
        }
        bi += 1;
    }

    let mut slides = runs::Slides::new();
    let mut drawn = solve(&router.worlds, &raw, c, &slides);
    audit::run(&router, &mut raw, &mut drawn, c, &slides);
    for m in audit::separation(&router, &mut raw, &mut drawn, c, &mut slides, usize::MAX) {
        reasons[m] = Some(NO_CLEAN_ROUTE);
    }
    for m in audit::complete(&mut router, &mut raw, &mut drawn, c, &mut slides) {
        reasons[m] = Some(NO_CLEAN_ROUTE);
    }
    audit::run(&router, &mut raw, &mut drawn, c, &slides);
    let kept = audit::collect(&drawn);

    let name = |i: usize| {
        let r = &router.reqs[i];
        format!("{} -> {}", r.a_path, r.b_path)
    };
    let mut airwires = Vec::new();
    let mut undrawn: Vec<usize> = Vec::new();
    for (m, reason) in reasons.iter().enumerate() {
        let Some(detail) = reason.filter(|_| drawn[m].is_none()) else {
            continue;
        };
        undrawn.push(m);
        let req = &router.reqs[m];
        report.push(impossible(req, detail));
        if let Some((from, to)) = geometry::airwire_segment(req.a_rect, req.b_rect) {
            airwires.push(Airwire {
                from,
                to,
                data_from: req.data_from.clone(),
                data_to: req.data_to.clone(),
            });
        }
    }
    let starved = feedback::starved(&router, &raw, &undrawn);
    report.extend(kept.iter().map(|x| Violation {
        rule: Rule::Crossing,
        severity: Severity::Info,
        wires: vec![name(x.pair.0), name(x.pair.1)],
        detail: format!("forced crossing at ({}, {})", x.at.0, x.at.1),
        span: router.reqs[x.pair.1].span,
    }));

    let mut wires = Vec::new();
    let mut req_of = Vec::new();
    for (i, req) in router.reqs.iter().enumerate() {
        let Some(chain) = &drawn[i] else {
            continue;
        };
        req_of.push(i);
        wires.push(RoutedWire {
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
            fan_from: router.fans.group_at(i, End::A).map(|g| g as u32),
            fan_to: router.fans.group_at(i, End::B).map(|g| g as u32),
        });
    }
    labels::place(&mut wires, &req_of, &router.reqs, program, &router.index);
    Ok(Routing {
        wires,
        report,
        airwires,
        starved,
    })
}

/// Route one self-loop bundle: a pinned shape around the keep-out corner
/// (WIRING §Special shapes), committed straight to ports and occupancy —
/// or reported impossible with its structural reason.
fn route_self_loop(
    router: &Router,
    occ: &mut Occupancy,
    ports: &mut Ports,
    raw: &mut [Option<Chain>],
    report: &mut Vec<Violation>,
    bi: usize,
) {
    let m0 = router.bundles[bi].members[0];
    let rep = &router.reqs[m0];
    let w = router.world_id(&parent_path(&rep.a_path));
    let Some((sa, sb)) = self_loop_sides(rep.side_a, rep.side_b) else {
        report.push(impossible(
            rep,
            "self-loop with both ends forced onto one side",
        ));
        return;
    };
    if ports.free(&rep.a_path, sa, rep.a_rect) < 1 || ports.free(&rep.a_path, sb, rep.a_rect) < 1 {
        report.push(impossible(rep, "no free port on the self-loop's sides"));
        return;
    }
    let ends = [sa, sb].map(|s| EndInfo {
        path: rep.a_path.clone(),
        side: s,
        rect: rep.a_rect,
        port: side_centre(rep.a_rect, s),
        fan: None,
    });
    let Some(chain) = geometry::self_loop_chain(
        &router.worlds[w].graph,
        w,
        rep.a_rect,
        rep.a_rect.inflate(router.clearance),
        ends,
        m0,
    ) else {
        report.push(impossible(
            rep,
            "no corridor around the body for a self-loop",
        ));
        return;
    };
    ports.commit(&rep.a_path, sa, 1);
    ports.commit(&rep.a_path, sb, 1);
    occ.commit_chain(&chain);
    raw[m0] = Some(chain);
}

/// Self-loop side resolution: defaults right → top; a forced side wins and its
/// free partner takes the default that stays adjacent; one shared side is
/// invalid (WIRING §Special shapes).
fn self_loop_sides(a: Option<Side>, b: Option<Side>) -> Option<(Side, Side)> {
    let (sa, sb) = match (a, b) {
        (None, None) => (Side::Right, Side::Top),
        (Some(s), None) => (
            s,
            if s == Side::Top {
                Side::Right
            } else {
                Side::Top
            },
        ),
        (None, Some(s)) => (
            if s == Side::Right {
                Side::Top
            } else {
                Side::Right
            },
            s,
        ),
        (Some(sa), Some(sb)) => (sa, sb),
    };
    (sa != sb).then_some((sa, sb))
}

/// The independent four-law check over a drawn scene (see [`validate`]).
pub fn validate_routing(
    nodes: &[PlacedNode],
    wires: &[RoutedWire],
    report: &[Violation],
) -> Vec<Violation> {
    validate::check(nodes, wires, report)
}

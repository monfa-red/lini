//! Placement (ROUTING.md model step 5): every run's ordinate, decided once,
//! per corridor, by one mechanism — cluster, order, settle.
//!
//! Runs whose spans come within a clearance of one another **and share
//! ordinate space** — one channel, fragments of one corridor
//! ([`ChannelGraph::corridor`]), or, across worlds, one landing side — form
//! a cluster. Within a cluster runs order so wires leave in the order they
//! arrive — nested, never braided — by the outward-walk comparator
//! ([`super::order`]) — and take the order-preserving ordinates nearest
//! their preferences at clearance pitch: the exact chain ([`ladder`]) when
//! the cluster's contention is a chain, the pairwise projection
//! ([`pairwise`]) when a zero gap bridges contenders the chain cannot
//! express. The relief valve compresses only what a stretch's hard boxes
//! genuinely cannot hold, never below half the clearance. Preferences are
//! the aesthetic law: interior runs want their corridor's anchor (the
//! midline between two nodes, the keep-out wall at the canvas edge); end
//! runs want the straightest lawful line to their port. Ports *are* end-run
//! ordinates — fan siblings merge into one item and share one port — so a
//! port can never disagree with the wire it serves.

use std::collections::BTreeMap;

use super::cost::min_pitch;
use super::graph::{Axis, Corridor};
use super::ladder::ladder;
use super::order;
use super::rect::Rect;
use super::{Chain, Run, World};
use crate::ast::Side;

/// One ladder item: a run (or a fan's merged end runs) awaiting its
/// ordinate.
struct Item {
    /// `(chain index, run index)` of every run taking this ordinate.
    members: Vec<(usize, usize)>,
    span: (f64, f64),
    /// The corner clamp ([`corner_clamp`]) — hard bounds keeping every
    /// corner inside both of its runs' channels.
    clamp: (f64, f64),
    pref: f64,
    /// Hard bounds from the port window; `None` for interior runs (the
    /// corridor's usable range applies alone).
    window: Option<(f64, f64)>,
    /// Declaration-order key for span ties.
    link: usize,
    /// The world whose channel graph this run rides in.
    world: usize,
    /// The channel the run rides — fragments of one corridor cluster across
    /// channels.
    chan: usize,
    /// The physical sides an end run lands on (both for a single-run wire).
    /// Worlds share these: an inner wire's port and an outer wire's punch
    /// meet on the same body side, so same-landing items cluster across
    /// worlds — the one place two worlds' wires lawfully share space.
    landings: Vec<(Side, Rect)>,
}

/// A run's ordinate preference and its hard port window, if any.
type Pref = (f64, Option<(f64, f64)>);

/// Assign every `Run::ord` in every chain — two rounds of the one pass,
/// the second's answer standing. Geometry's provisional spans reach
/// *estimates* of unplaced neighbours (a corridor anchor a jog may ladder
/// well away from), so first-round contention is partly phantom: spans that
/// touch only at a shared estimate charge pitch two wires never owe, and
/// the relief valve can then compress a window with room to spare. The
/// second round re-derives every span from the placed ordinates — the
/// corners the polyline will actually take — and settles the real
/// contention, the same probe-refine shape the search uses for learned
/// closures. Deciding on refreshed truth, once.
///
/// Corners, by contrast, never ride an estimate: a run's drawn extent
/// follows wherever its neighbours finally land, so every ordinate is
/// clamped into its perpendicular neighbours' channel travel extents — the
/// corner stays inside both runs' channels (a run lies in one channel of
/// its axis, ROUTING.md §Vocabulary), so a drawn segment can never leave
/// the free space it was priced in, no matter where a later round moves
/// the far corner.
pub(crate) fn place(worlds: &[World], chains: &mut [Option<Chain>], clearance: f64) {
    settle_axes(worlds, chains, clearance);
    refresh_spans(chains);
    settle_axes(worlds, chains, clearance);
}

/// A run's lawful ordinate range: its port window intersected with the
/// corridor's usable range — the window winning when the corridor's
/// tightening would invert it, the walls standing in for a sliver whose
/// soft margins cross (the search admitted the run, so it draws there,
/// surrendering what the sliver cannot give).
fn law_range(window: Option<(f64, f64)>, corr: &Corridor, clearance: f64) -> (f64, f64) {
    let u = corr.usable(clearance);
    match window {
        Some(w) => {
            let tight = (w.0.max(u.0), w.1.min(u.1));
            if tight.0 <= tight.1 { tight } else { w }
        }
        None => {
            if u.0 <= u.1 {
                u
            } else {
                corr.walls
            }
        }
    }
}

/// The corner clamp of run `ri`: its ordinate is the corner's coordinate
/// along each perpendicular neighbour's travel, so it must lie inside those
/// neighbours' channel travel extents — the corner may never leave either
/// run's channel.
fn corner_clamp(worlds: &[World], chain: &Chain, ri: usize) -> (f64, f64) {
    let graph = &worlds[chain.world].graph;
    let travel = |r: &Run| {
        match r.axis {
            Axis::H => &graph.h[r.chan],
            Axis::V => &graph.v[r.chan],
        }
        .travel()
    };
    let mut clamp = (f64::NEG_INFINITY, f64::INFINITY);
    if ri > 0 {
        let t = travel(&chain.runs[ri - 1]);
        clamp = (clamp.0.max(t.0), clamp.1.min(t.1));
    }
    if ri + 1 < chain.runs.len() {
        let t = travel(&chain.runs[ri + 1]);
        clamp = (clamp.0.max(t.0), clamp.1.min(t.1));
    }
    clamp
}

/// Re-derive every run's span from its neighbours' placed ordinates — end
/// runs from their side line to the first corner, interior runs corner to
/// corner (the segment extents [`super::geometry::polyline`] will draw).
fn refresh_spans(chains: &mut [Option<Chain>]) {
    for chain in chains.iter_mut().flatten() {
        let n = chain.runs.len();
        if n < 2 {
            continue;
        }
        let ords: Vec<f64> = chain
            .runs
            .iter()
            .map(|r| r.ord.expect("first round placed every run"))
            .collect();
        for (i, run) in chain.runs.iter_mut().enumerate() {
            let lo = if i == 0 {
                chain.ends[0].side_coord()
            } else {
                ords[i - 1]
            };
            let hi = if i == n - 1 {
                chain.ends[1].side_coord()
            } else {
                ords[i + 1]
            };
            run.span = (lo.min(hi), lo.max(hi));
        }
    }
}

/// One placement pass: cluster, order, ladder, per (world, axis) in fixed
/// order — preferences and the nesting walk read only static estimates, so
/// the outcome is independent of that order, and deterministic.
fn settle_axes(worlds: &[World], chains: &mut [Option<Chain>], clearance: f64) {
    let prefs: Vec<Vec<Pref>> = chains
        .iter()
        .map(|c| c.as_ref().map_or(Vec::new(), |ch| chain_prefs(ch, worlds)))
        .collect();
    let ests: Vec<Vec<f64>> = prefs
        .iter()
        .map(|v| v.iter().map(|p| p.0).collect())
        .collect();
    let mut by_axis: BTreeMap<u8, Vec<Item>> = BTreeMap::new();
    for (ci, chain) in chains.iter().enumerate() {
        let Some(chain) = chain else { continue };
        let last = chain.runs.len() - 1;
        for (ri, run) in chain.runs.iter().enumerate() {
            let mut landings = Vec::new();
            if ri == 0 {
                landings.push((chain.ends[0].side, chain.ends[0].rect));
            }
            if ri == last {
                landings.push((chain.ends[1].side, chain.ends[1].rect));
            }
            let span = (run.span.0.min(run.span.1), run.span.0.max(run.span.1));
            by_axis.entry(run.axis.index()).or_default().push(Item {
                members: vec![(ci, ri)],
                span,
                clamp: corner_clamp(worlds, chain, ri),
                pref: prefs[ci][ri].0,
                window: prefs[ci][ri].1,
                link: chain.link,
                world: chain.world,
                chan: run.chan,
                landings,
            });
        }
    }

    for (axis, mut items) in by_axis {
        let axis = [Axis::H, Axis::V][axis as usize];
        merge_fans(&mut items, chains);
        items.sort_by(|a, b| {
            a.span
                .0
                .total_cmp(&b.span.0)
                .then(a.link.cmp(&b.link))
                .then(a.world.cmp(&b.world))
                .then(a.chan.cmp(&b.chan))
        });
        let corridors: Vec<Corridor> = items
            .iter()
            .map(|i| {
                worlds[i.world]
                    .graph
                    .corridor(axis, i.chan, i.span.0, i.span.1)
            })
            .collect();

        // Cluster items that contend for tracks: spans within a clearance of
        // each other, in one channel or across fragments of one corridor —
        // or, across worlds, landing on one physical side.
        let n = items.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn root(parent: &mut [usize], mut i: usize) -> usize {
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        for i in 0..n {
            for j in i + 1..n {
                let near = near(items[i].span, items[j].span, clearance);
                let shared = (items[i].world == items[j].world
                    && (items[i].chan == items[j].chan
                        || corridors[i].chans.contains(&items[j].chan)
                        || corridors[j].chans.contains(&items[i].chan)))
                    || items[i]
                        .landings
                        .iter()
                        .any(|l| items[j].landings.contains(l));
                if near && shared {
                    let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                    parent[a.max(b)] = a.min(b);
                }
            }
        }
        let mut clusters: BTreeMap<usize, Vec<(Item, Corridor)>> = BTreeMap::new();
        for (i, (item, corr)) in items.into_iter().zip(corridors).enumerate() {
            clusters
                .entry(root(&mut parent, i))
                .or_default()
                .push((item, corr));
        }
        for (_, cluster) in clusters {
            settle(cluster, clearance, chains, &ests);
        }
    }
}

/// Per-run `(preference, port window)` for one chain (ROUTING.md step 5):
/// a single run serving both ports prefers the straightest lawful line —
/// the two side centres' midpoint clamped into the shared window; an end
/// run prefers its own side's centre inside its window; an interior run
/// prefers its channel's anchor.
fn chain_prefs(chain: &Chain, worlds: &[World]) -> Vec<Pref> {
    let last = chain.runs.len() - 1;
    chain
        .runs
        .iter()
        .enumerate()
        .map(|(ri, run)| {
            let (a, b) = (&chain.ends[0], &chain.ends[1]);
            if ri == 0 && ri == last {
                let shared = (a.window.0.max(b.window.0), a.window.1.min(b.window.1));
                debug_assert!(
                    shared.0 <= shared.1,
                    "a straight run needs overlapping windows (the search jogs otherwise)"
                );
                let mid = (a.centre() + b.centre()) / 2.0;
                (mid.max(shared.0).min(shared.1), Some(shared))
            } else if ri == 0 {
                (a.centre(), Some(a.window))
            } else if ri == last {
                (b.centre(), Some(b.window))
            } else {
                let (lo, hi) = (run.span.0.min(run.span.1), run.span.0.max(run.span.1));
                let corridor = worlds[chain.world]
                    .graph
                    .corridor(run.axis, run.chan, lo, hi);
                (corridor.anchor(), None)
            }
        })
        .collect()
}

/// Fan siblings' end runs share one port: merge same-group items into one,
/// spans united, windows intersected.
fn merge_fans(items: &mut Vec<Item>, chains: &[Option<Chain>]) {
    let mut merged: Vec<Item> = Vec::new();
    for item in items.drain(..) {
        let (ci, ri) = item.members[0];
        let chain = chains[ci].as_ref().expect("placed chain");
        let fan = fan_of(chain, ri);
        let twin = fan.and_then(|f| {
            merged.iter_mut().find(|m| {
                let (mc, mr) = m.members[0];
                fan_of(chains[mc].as_ref().expect("placed chain"), mr) == Some(f)
            })
        });
        match twin {
            Some(m) => {
                m.span = (m.span.0.min(item.span.0), m.span.1.max(item.span.1));
                m.clamp = (m.clamp.0.max(item.clamp.0), m.clamp.1.min(item.clamp.1));
                m.window = match (m.window, item.window) {
                    (Some(a), Some(b)) => Some((a.0.max(b.0), a.1.min(b.1))),
                    (w, None) | (None, w) => w,
                };
                m.link = m.link.min(item.link);
                m.members.extend(item.members);
                for l in item.landings {
                    if !m.landings.contains(&l) {
                        m.landings.push(l);
                    }
                }
            }
            None => merged.push(item),
        }
    }
    *items = std::mem::take(&mut merged);
}

/// The fan group of an **end** run, if any — interior runs never merge.
fn fan_of(chain: &Chain, ri: usize) -> Option<usize> {
    let last = chain.runs.len() - 1;
    match (ri == 0, ri == last) {
        (true, true) => chain.ends[0].fan.or(chain.ends[1].fan),
        (true, false) => chain.ends[0].fan,
        (false, true) => chain.ends[1].fan,
        _ => None,
    }
}

/// Order one cluster and ladder it into ordinates.
fn settle(
    mut cluster: Vec<(Item, Corridor)>,
    clearance: f64,
    chains: &mut [Option<Chain>],
    ests: &[Vec<f64>],
) {
    // Preference orders what geometry doesn't couple (prefs sit inside
    // their boxes, so disjoint windows order themselves); the outward walk
    // arbitrates equal preferences — nested, never braided — and declaration
    // order settles the rest inside [`order::cmp_runs`]. A fan's merged item
    // walks as its first member.
    let ctx = order::Ctx {
        chains: &*chains,
        ests,
    };
    cluster.sort_by(|a, b| {
        a.0.pref
            .total_cmp(&b.0.pref)
            .then_with(|| order::cmp_runs(&ctx, a.0.members[0], b.0.members[0]))
    });

    let n = cluster.len();
    let prefs: Vec<f64> = cluster.iter().map(|(i, _)| i.pref).collect();
    let bounds: Vec<(f64, f64)> = cluster
        .iter()
        .map(|(i, corr)| {
            let r = law_range(i.window, corr, clearance);
            // The corner clamp binds hard; a search-admitted run always has
            // room inside it (the route's corners sat in cells), so an
            // inversion only flags float dust at a channel edge.
            let tight = (r.0.max(i.clamp.0), r.1.min(i.clamp.1));
            if tight.0 <= tight.1 { tight } else { r }
        })
        .collect();
    // Every gap starts at full clearance — the relief below is the one
    // compression mechanism. Only **contending** neighbours owe each other
    // pitch; a transitively-chained pair whose spans lie far apart never
    // runs alongside — its gap is 0, so the ladder may reuse the ordinate
    // space.
    let mut seps: Vec<f64> = cluster
        .windows(2)
        .map(|w| {
            if contend(&w[0].0, &w[1].0, clearance) {
                clearance
            } else {
                0.0
            }
        })
        .collect();
    // The chain expresses this cluster only when no contending pair is
    // bridged by a zero gap. Across such a bridge the chain goes wrong both
    // ways at once: the pair's pitch dissolves (its gaps sum to nothing),
    // while order and envelope bind travel-disjoint groups the pair never
    // coupled — the relief then compresses windows with room to spare.
    // Those clusters settle on their true pairwise constraints instead.
    let chain_ok = (0..n).all(|i| {
        (i + 2..n).all(|j| {
            !contend(&cluster[i].0, &cluster[j].0, clearance) || seps[i..j].iter().all(|s| *s > 0.0)
        })
    });
    let mut feasible = chain_ok;
    if chain_ok {
        // Law 1's relief valve: only a stretch that genuinely cannot hold
        // full pitch compresses, **uniformly** — every gap in the binding
        // stretch drops toward one target, floored at half the clearance.
        // Feasibility is judged exactly, not by envelope: the greedy pass
        // exploits staggered boxes (a wire whose corridor reaches further
        // lends the room), so a stretch that fits at full clearance in the
        // drawn order is never squeezed.
        for _ in 0..n.max(1) * 2 {
            let mut binding = 0;
            let mut x = f64::NEG_INFINITY;
            let mut violated = None;
            for k in 0..n {
                let pushed = if k == 0 { bounds[k].0 } else { x + seps[k - 1] };
                if pushed <= bounds[k].0 {
                    binding = k;
                }
                x = pushed.max(bounds[k].0);
                if x > bounds[k].1 + 1e-9 {
                    violated = Some((binding, k));
                    break;
                }
            }
            let Some((i, j)) = violated else {
                feasible = true;
                break;
            };
            feasible = false;
            let avail = (bounds[j].1 - bounds[i].0).max(0.0);
            let gaps = seps[i..j].iter().filter(|s| **s > 0.0).count().max(1);
            let target = (avail / gaps as f64).max(min_pitch(clearance));
            let mut lowered = false;
            for s in &mut seps[i..j] {
                if *s > target {
                    *s = target;
                    lowered = true;
                }
            }
            if !lowered {
                break;
            }
        }
    }
    // A chain the floors cannot make feasible — the admission's
    // cross-window blind spot — settles through the pairwise solver, whose
    // final clamp keeps windows and walls absolute and lets the gaps carry
    // the visible debt.
    let ords = if feasible {
        ladder(&prefs, &bounds, &seps)
    } else {
        pairwise(&cluster, &prefs, &bounds, clearance)
    };
    for ((item, _), ord) in cluster.iter().zip(&ords) {
        for &(ci, ri) in &item.members {
            chains[ci].as_mut().expect("placed chain").runs[ri].ord = Some(*ord);
        }
    }
}

/// Whether two items owe each other pitch: spans that overlap, or end
/// within a clearance of one another (their tips flank). Two pieces of one
/// wire owe each other nothing unless their spans overlap (a U's
/// doubled-back legs; a Z's jog collapses to zero and the legs weld).
fn contend(a: &Item, b: &Item, clearance: f64) -> bool {
    let same_wire = a
        .members
        .iter()
        .any(|(c0, _)| b.members.iter().any(|(c1, _)| c0 == c1));
    let overlap = a.span.0.max(b.span.0) < a.span.1.min(b.span.1);
    overlap || (near(a.span, b.span, clearance) && !same_wire)
}

/// Whether two spans come within a clearance of one another — inclusive at
/// exactly a clearance: round one separates contenders by precisely the
/// pitch they owe, so the refreshed spans of a settled pair sit exactly a
/// clearance apart, and a strict test would let round two forget the
/// contention and collapse the pair back together.
fn near(a: (f64, f64), b: (f64, f64), clearance: f64) -> bool {
    b.0 <= a.1 + clearance + 1e-6 && a.0 <= b.1 + clearance + 1e-6
}

/// The general settle for clusters the chain cannot express: each
/// contending pair — and only those — owes its pitch, signed by the cluster
/// order (nested, never braided); non-contending items stay uncoupled, free
/// to share ordinate space. Relief first makes the system feasible (the
/// same uniform compression, applied along the tightest constraint chains),
/// then the ordinates are the least-squares projection of the preferences
/// onto the feasible set (Dykstra's alternating projections — exact in the
/// limit, run to well below geometric tolerance, deterministic).
fn pairwise(
    cluster: &[(Item, Corridor)],
    prefs: &[f64],
    bounds: &[(f64, f64)],
    clearance: f64,
) -> Vec<f64> {
    let n = cluster.len();
    let mut gaps: Vec<(usize, usize, f64)> = Vec::new();
    for i in 0..n {
        for j in i + 1..n {
            if contend(&cluster[i].0, &cluster[j].0, clearance) {
                gaps.push((i, j, clearance));
            }
        }
    }

    // Feasibility relief, judged exactly: each item's minimal lawful
    // ordinate is its box floor pushed up by every contender below it
    // (staggered boxes lend their room — an envelope test would squeeze
    // stretches that actually fit). When a chain overruns a box, the gaps
    // riding it compress uniformly toward what fits, floored at half the
    // clearance; floors bound the loop.
    for _ in 0..64 {
        let mut reach = vec![f64::NEG_INFINITY; n];
        let mut origin: Vec<usize> = (0..n).collect();
        let mut via: Vec<Option<usize>> = vec![None; n];
        let mut violated = None;
        'feasible: for j in 0..n {
            let mut x = bounds[j].0;
            for (e, &(i, jj, g)) in gaps.iter().enumerate() {
                if jj == j && reach[i] + g > x {
                    x = reach[i] + g;
                    origin[j] = origin[i];
                    via[j] = Some(e);
                }
            }
            reach[j] = x;
            if x > bounds[j].1 + 1e-9 {
                violated = Some(j);
                break 'feasible;
            }
        }
        let Some(t) = violated else { break };
        let mut path = Vec::new();
        let mut at = t;
        while let Some(e) = via[at] {
            path.push(e);
            at = gaps[e].0;
        }
        let avail = (bounds[t].1 - bounds[origin[t]].0).max(0.0);
        let target = (avail / path.len().max(1) as f64).max(min_pitch(clearance));
        let mut lowered = false;
        for e in path {
            if gaps[e].2 > target {
                gaps[e].2 = target;
                lowered = true;
            }
        }
        if !lowered {
            break;
        }
    }

    // Dykstra: project the preferences onto the boxes and every gap
    // halfspace in turn, with per-constraint corrections, until a full
    // sweep moves nothing.
    let mut x = prefs.to_vec();
    let mut box_corr = vec![0.0; n];
    let mut gap_corr = vec![0.0; gaps.len()];
    for _ in 0..10_000 {
        let mut moved = 0.0_f64;
        for i in 0..n {
            let y = x[i] + box_corr[i];
            let p = y.max(bounds[i].0).min(bounds[i].1);
            box_corr[i] = y - p;
            moved = moved.max((p - x[i]).abs());
            x[i] = p;
        }
        for (e, &(i, j, g)) in gaps.iter().enumerate() {
            let (yi, yj) = (x[i] + gap_corr[e] / 2.0, x[j] - gap_corr[e] / 2.0);
            let short = g - (yj - yi);
            let d = short.max(0.0);
            gap_corr[e] = d;
            let (pi, pj) = (yi - d / 2.0, yj + d / 2.0);
            moved = moved.max((pi - x[i]).abs()).max((pj - x[j]).abs());
            x[i] = pi;
            x[j] = pj;
        }
        if moved < 1e-11 {
            break;
        }
    }
    // A system the relief could not make feasible even at the floors — the
    // admission's cross-window blind spot (ROUTING-V2.md execution log) —
    // leaves Dykstra splitting the shortfall across every constraint.
    // Windows and walls are absolute law; pitch below them is at least
    // visible. Bounds win, and the gaps carry the debt.
    for i in 0..n {
        x[i] = x[i].max(bounds[i].0).min(bounds[i].1);
    }
    x
}

#[cfg(test)]
mod tests {
    use super::super::graph::ChannelGraph;
    use super::super::rect::Rect;
    use super::super::{EndInfo, Run};
    use super::*;
    use crate::ast::Side;

    const C: f64 = 8.0;

    fn world(bounds: Rect, keepouts: &[Rect]) -> World {
        World {
            path: String::new(),
            graph: ChannelGraph::build(bounds, keepouts, false),
        }
    }

    fn end(side: Side, rect: Rect) -> EndInfo {
        let window = match side {
            Side::Left | Side::Right => (rect.y0 + C, rect.y1 - C),
            Side::Top | Side::Bottom => (rect.x0 + C, rect.x1 - C),
        };
        EndInfo {
            side,
            rect,
            window,
            fan: None,
        }
    }

    /// The facing scene: two tall nodes (windows 44 high — room for a
    /// 4-bundle at clearance pitch) across an open corridor in a 200×100
    /// world.
    fn facing() -> (World, Rect, Rect) {
        let a = Rect::new(20.0, 20.0, 40.0, 80.0);
        let b = Rect::new(160.0, 20.0, 180.0, 80.0);
        let w = world(
            Rect::new(0.0, 0.0, 200.0, 100.0),
            &[a.inflate(C), b.inflate(C)],
        );
        (w, a, b)
    }

    fn h_chan(w: &World, x: f64, y: f64) -> usize {
        w.graph
            .h
            .iter()
            .position(|c| x >= c.rect.x0 && x <= c.rect.x1 && y >= c.rect.y0 && y <= c.rect.y1)
            .expect("h channel at point")
    }

    fn straight(link: usize, a: Rect, b: Rect, chan: usize) -> Chain {
        Chain {
            link,
            world: 0,
            runs: vec![Run {
                axis: Axis::H,
                chan,
                span: (a.x1, b.x0),
                ord: None,
            }],
            ends: [end(Side::Right, a), end(Side::Left, b)],
        }
    }

    #[test]
    fn a_lone_straight_takes_the_shared_centre() {
        let (w, a, b) = facing();
        let chan = h_chan(&w, 100.0, 50.0);
        let mut chains = vec![Some(straight(0, a, b, chan))];
        place(&[w], &mut chains, C);
        assert_eq!(chains[0].as_ref().unwrap().runs[0].ord, Some(50.0));
    }

    #[test]
    fn a_bundle_ladders_centred_on_the_shared_centre() {
        let (w, a, b) = facing();
        let chan = h_chan(&w, 100.0, 50.0);
        let mut chains: Vec<Option<Chain>> =
            (0..4).map(|i| Some(straight(i, a, b, chan))).collect();
        place(&[w], &mut chains, C);
        let ords: Vec<f64> = chains
            .iter()
            .map(|c| c.as_ref().unwrap().runs[0].ord.unwrap())
            .collect();
        // Four rails at clearance pitch, median on the aligned centres, in
        // declaration order.
        assert_eq!(ords, vec![38.0, 46.0, 54.0, 62.0]);
    }

    #[test]
    fn an_interior_run_rests_on_the_channel_midline() {
        // A three-run Z through the corridor: the jog's V run prefers the
        // anchor of the V channel between the keep-outs.
        let (w, a, b) = facing();
        let hchan = h_chan(&w, 100.0, 50.0);
        let vchan = w
            .graph
            .v
            .iter()
            .position(|c| c.rect == Rect::new(48.0, 0.0, 152.0, 100.0))
            .expect("middle V channel");
        let mut chains = vec![Some(Chain {
            link: 0,
            world: 0,
            runs: vec![
                Run {
                    axis: Axis::H,
                    chan: hchan,
                    span: (40.0, 100.0),
                    ord: None,
                },
                Run {
                    axis: Axis::V,
                    chan: vchan,
                    span: (48.0, 52.0),
                    ord: None,
                },
                Run {
                    axis: Axis::H,
                    chan: hchan,
                    span: (100.0, 160.0),
                    ord: None,
                },
            ],
            ends: [end(Side::Right, a), end(Side::Left, b)],
        })];
        place(&[w], &mut chains, C);
        let runs = &chains[0].as_ref().unwrap().runs;
        // End runs take their side centres; the jog takes the V anchor
        // (both walls are keep-out edges → their midline, x = 100).
        assert_eq!(runs[0].ord, Some(50.0));
        assert_eq!(runs[1].ord, Some(100.0));
        assert_eq!(runs[2].ord, Some(50.0));
    }

    #[test]
    fn turning_wires_nest_in_arrival_order() {
        // Two L-wires from stacked sources in the west turn south in one V
        // channel: the upper wire turns outside (east of) the lower — nested,
        // never braided (an east-then-south corner pair).
        let a1 = Rect::new(20.0, 10.0, 40.0, 26.0);
        let a2 = Rect::new(20.0, 34.0, 40.0, 50.0);
        let b = Rect::new(80.0, 160.0, 120.0, 180.0);
        let w = world(
            Rect::new(0.0, 0.0, 200.0, 200.0),
            &[a1.inflate(C), a2.inflate(C), b.inflate(C)],
        );
        // The V channel the wires descend in: the one over b, containing
        // its top window (x 88..112).
        let vchan = w
            .graph
            .v
            .iter()
            .position(|c| {
                c.rect.x0 <= 88.0 && c.rect.x1 >= 112.0 && c.rect.y0 <= 60.0 && c.rect.y1 >= 140.0
            })
            .expect("V channel above b");
        let l_chain = |link: usize, src: Rect, hchan: usize| Chain {
            link,
            world: 0,
            runs: vec![
                Run {
                    axis: Axis::H,
                    chan: hchan,
                    span: (src.x1, 100.0),
                    ord: None,
                },
                Run {
                    axis: Axis::V,
                    chan: vchan,
                    span: ((src.y0 + src.y1) / 2.0, 160.0),
                    ord: None,
                },
            ],
            ends: [end(Side::Right, src), end(Side::Top, b)],
        };
        let h1 = h_chan(&w, 60.0, 18.0);
        let h2 = h_chan(&w, 60.0, 42.0);
        let mut chains = vec![Some(l_chain(0, a1, h1)), Some(l_chain(1, a2, h2))];
        place(&[w], &mut chains, C);
        let x1 = chains[0].as_ref().unwrap().runs[1].ord.unwrap();
        let x2 = chains[1].as_ref().unwrap().runs[1].ord.unwrap();
        assert!(
            x1 > x2,
            "upper wire turns outside the lower: x1={x1} x2={x2}"
        );
    }

    #[test]
    fn fan_siblings_share_one_port_ordinate() {
        let (w, a, b) = facing();
        let chan = h_chan(&w, 100.0, 50.0);
        let mut c1 = straight(0, a, b, chan);
        let mut c2 = straight(1, a, b, chan);
        c1.ends[0].fan = Some(0);
        c2.ends[0].fan = Some(0);
        let mut chains = vec![Some(c1), Some(c2)];
        place(&[w], &mut chains, C);
        let o1 = chains[0].as_ref().unwrap().runs[0].ord.unwrap();
        let o2 = chains[1].as_ref().unwrap().runs[0].ord.unwrap();
        assert_eq!(o1, o2, "one fan, one port");
    }

    #[test]
    fn disjoint_clusters_both_take_the_midline() {
        // Two runs far apart along one channel never cluster: each sits on
        // the channel anchor independently.
        let w = world(Rect::new(0.0, 0.0, 400.0, 100.0), &[]);
        let interior = |link: usize, span: (f64, f64)| Chain {
            link,
            world: 0,
            runs: vec![
                Run {
                    axis: Axis::V,
                    chan: 0,
                    span: (10.0, 20.0),
                    ord: None,
                },
                Run {
                    axis: Axis::H,
                    chan: 0,
                    span,
                    ord: None,
                },
                Run {
                    axis: Axis::V,
                    chan: 0,
                    span: (80.0, 90.0),
                    ord: None,
                },
            ],
            ends: [
                end(Side::Bottom, Rect::new(span.0 - 20.0, 0.0, span.0, 10.0)),
                end(Side::Bottom, Rect::new(span.1, 0.0, span.1 + 20.0, 10.0)),
            ],
        };
        let mut chains = vec![
            Some(interior(0, (40.0, 120.0))),
            Some(interior(1, (240.0, 320.0))),
        ];
        place(&[w], &mut chains, C);
        let m1 = chains[0].as_ref().unwrap().runs[1].ord.unwrap();
        let m2 = chains[1].as_ref().unwrap().runs[1].ord.unwrap();
        assert_eq!(m1, 50.0, "the empty world's H anchor is its midline");
        assert_eq!(m1, m2, "disjoint spans share the midline in peace");
    }

    #[test]
    fn place_is_deterministic() {
        let (w, a, b) = facing();
        let chan = h_chan(&w, 100.0, 50.0);
        let run = |chains: &mut Vec<Option<Chain>>| {
            place(
                &[world(
                    Rect::new(0.0, 0.0, 200.0, 100.0),
                    &[a.inflate(C), b.inflate(C)],
                )],
                chains,
                C,
            );
            chains
                .iter()
                .map(|c| c.as_ref().unwrap().runs[0].ord.unwrap())
                .collect::<Vec<f64>>()
        };
        let mut first = (0..4)
            .map(|i| Some(straight(i, a, b, chan)))
            .collect::<Vec<_>>();
        let baseline = run(&mut first);
        for _ in 0..50 {
            let mut again = (0..4)
                .map(|i| Some(straight(i, a, b, chan)))
                .collect::<Vec<_>>();
            assert_eq!(run(&mut again), baseline);
        }
    }
}

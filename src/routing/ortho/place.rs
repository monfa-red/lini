//! Placement (ROUTING.md model step 5): every run's ordinate, decided once,
//! per corridor, by one mechanism — cluster, order, settle.
//!
//! Runs whose spans come within a clearance of one another **and share
//! ordinate space** — one channel, fragments of one corridor
//! ([`ChannelGraph::corridor`]), or, across worlds, one landing side — form
//! a cluster. Within a cluster runs order so wires leave in the order they
//! arrive — nested, never braided — by the outward-walk order
//! ([`super::order`]) — and take the order-preserving ordinates nearest
//! their preferences at the pitch each pair genuinely owes ([`owed`] — the
//! distance model: full clearance alongside, the diagonal remainder past
//! each other): the exact chain ([`ladder`]) when the cluster's contention
//! is a chain, the pairwise projection ([`super::pairwise`]) when an
//! under-sized bridge leaves debt the chain cannot express. The relief
//! valve compresses only what a stretch's hard boxes genuinely cannot
//! hold, never below half the clearance. Preferences are
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
pub(super) struct Item {
    /// `(chain index, run index)` of every run taking this ordinate.
    pub(super) members: Vec<(usize, usize)>,
    pub(super) span: (f64, f64),
    /// The corner clamp ([`corner_clamp`]) — hard bounds keeping every
    /// corner inside both of its runs' channels.
    clamp: (f64, f64),
    pref: f64,
    /// Hard bounds from the port window; `None` for interior runs (the
    /// corridor's usable range applies alone).
    pub(super) window: Option<(f64, f64)>,
    /// Declaration-order key for span ties.
    link: usize,
    /// The world whose channel graph this run rides in.
    world: usize,
    /// The channel the run rides — fragments of one corridor cluster across
    /// channels.
    pub(super) chan: usize,
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
/// its axis, ROUTING.md Vocabulary), so a drawn segment can never leave
/// the free space it was priced in, no matter where a later round moves
/// the far corner.
pub(crate) fn place(worlds: &[World], chains: &mut [Option<Chain>], clearance: f64) {
    settle_axes(worlds, chains, clearance);
    refresh_spans(chains);
    settle_axes(worlds, chains, clearance);
}

/// A run's lawful ordinate range: its port window intersected with the
/// corridor's usable range — the window winning when the corridor's
/// tightening would invert it (the search admitted the run, so it draws
/// there, surrendering what the sliver cannot give).
fn law_range(window: Option<(f64, f64)>, corr: &Corridor) -> (f64, f64) {
    let u = corr.usable();
    match window {
        Some(w) => {
            let tight = (w.0.max(u.0), w.1.min(u.1));
            if tight.0 <= tight.1 { tight } else { w }
        }
        None => u,
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
pub(super) fn refresh_spans(chains: &mut [Option<Chain>]) {
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
    let (ests, by_axis) = collect(worlds, chains);
    for (axis, mut items) in by_axis {
        let axis = [Axis::H, Axis::V][axis as usize];
        merge_fans(&mut items, chains);
        for cluster in clusters_of(axis, items, worlds, clearance) {
            settle(cluster, clearance, chains, &ests);
        }
    }
}

/// Every run of every chain as a ladder item, grouped by axis, plus each
/// chain's ordinate estimates — the one item model placement settles and
/// admission ([`super::admit`]) probes.
pub(super) fn collect(
    worlds: &[World],
    chains: &[Option<Chain>],
) -> (Vec<Vec<f64>>, BTreeMap<u8, Vec<Item>>) {
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
    (ests, by_axis)
}

/// Group one axis's items into contention clusters: spans within a
/// clearance of each other, in one channel or across fragments of one
/// corridor — or, across worlds, landing on one physical side.
pub(super) fn clusters_of(
    axis: Axis,
    mut items: Vec<Item>,
    worlds: &[World],
    clearance: f64,
) -> Vec<Vec<(Item, Corridor)>> {
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
            // Corridors meeting at a shared boundary couple too: their
            // walls charge no margin, so near runs on the two sides owe
            // their pitch through the one ladder — placement owns
            // cross-boundary separation (wall coordinates come from one
            // sweep-edge list, so the abutting test is exact equality).
            let abuts = corridors[i].walls.1 == corridors[j].walls.0
                || corridors[j].walls.1 == corridors[i].walls.0;
            let shared = (items[i].world == items[j].world
                && (items[i].chan == items[j].chan
                    || corridors[i].chans.contains(&items[j].chan)
                    || corridors[j].chans.contains(&items[i].chan)
                    || abuts))
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
    clusters.into_values().collect()
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
                // The aesthetic target is the anchor of the corridor the
                // run can lawfully inhabit: a span kissing a keep-out
                // corner lets the walk absorb a void the corner clamp
                // forbids, and the raw anchor then hugs a wall its twin —
                // one lane over, reading the narrow corridor — never
                // sees, ordering the pair into an unplaceable chain.
                let clamp = corner_clamp(worlds, chain, ri);
                (corridor.clipped(clamp.0, clamp.1).anchor(), None)
            }
        })
        .collect()
}

/// Fan siblings' end runs share one port: merge same-group items into one,
/// spans united, windows intersected.
pub(super) fn merge_fans(items: &mut Vec<Item>, chains: &[Option<Chain>]) {
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

/// A run's lawful bounds: law range ∩ corner clamp. The corner clamp binds
/// hard; a search-admitted run always has room inside it (the route's
/// corners sat in cells), so an inversion only flags float dust at a
/// channel edge.
pub(super) fn bound((i, corr): &(Item, Corridor)) -> (f64, f64) {
    let r = law_range(i.window, corr);
    let tight = (r.0.max(i.clamp.0), r.1.min(i.clamp.1));
    if tight.0 <= tight.1 { tight } else { r }
}

/// Order one cluster into its drawn order and lawful preferences.
///
/// Preference orders what geometry doesn't couple; the outward walk
/// arbitrates equal preferences — nested, never braided — and declaration
/// order settles the rest, all inside [`order::ranks`]. A fan's merged item
/// walks as its first member. The preference is the nearest lawful ordinate
/// to the aesthetic target (ROUTING.md step 5): a raw corridor anchor can
/// fall outside a run's own bounds — a refreshed span can reach through a
/// void far wider than the pocket its corners pin it to — and ordering by
/// the raw anchor then interleaves runs whose lawful ranges never meet, an
/// order no solver realises lawfully (the trunk rails of an S-curve bundle
/// collapse onto one ordinate). Clamping keeps the sort's premise true:
/// prefs sit inside their boxes, so disjoint ranges order themselves.
pub(super) fn arrange(
    cluster: Vec<(Item, Corridor)>,
    chains: &[Option<Chain>],
    ests: &[Vec<f64>],
) -> (Vec<f64>, Vec<(Item, Corridor)>) {
    let ctx = order::Ctx { chains, ests };
    let reps: Vec<(usize, usize)> = cluster.iter().map(|(i, _)| i.members[0]).collect();
    let item_prefs: Vec<f64> = cluster
        .iter()
        .map(|c| {
            let (lo, hi) = bound(c);
            c.0.pref.max(lo).min(hi)
        })
        .collect();
    let pos = order::ranks(&ctx, &reps, &item_prefs);
    let mut indexed: Vec<_> = pos
        .into_iter()
        .zip(item_prefs.into_iter().zip(cluster))
        .collect();
    indexed.sort_by_key(|(p, _)| *p);
    indexed.into_iter().map(|(_, pc)| pc).unzip()
}

/// The greedy feasibility scan: pack every item leftmost at its separations
/// and report the first binding stretch `(i, j)` whose box the chain
/// overruns — `None` when the order fits. The pass is exact for chain
/// constraints (staggered boxes lend their room), so a stretch that fits in
/// the drawn order is never reported.
pub(super) fn overrun(bounds: &[(f64, f64)], seps: &[f64]) -> Option<(usize, usize)> {
    let mut binding = 0;
    let mut x = f64::NEG_INFINITY;
    for k in 0..bounds.len() {
        let pushed = if k == 0 { bounds[k].0 } else { x + seps[k - 1] };
        if pushed <= bounds[k].0 {
            binding = k;
        }
        x = pushed.max(bounds[k].0);
        if x > bounds[k].1 + 1e-9 {
            return Some((binding, k));
        }
    }
    None
}

/// Order one cluster and ladder it into ordinates.
fn settle(
    cluster: Vec<(Item, Corridor)>,
    clearance: f64,
    chains: &mut [Option<Chain>],
    ests: &[Vec<f64>],
) {
    let (prefs, cluster) = arrange(cluster, &*chains, ests);

    let n = cluster.len();
    let bounds: Vec<(f64, f64)> = cluster.iter().map(bound).collect();
    // Every gap starts at what the pair genuinely owes ([`owed`]) — the
    // relief below is the one compression mechanism. Only **contending**
    // neighbours owe pitch; a transitively-chained pair whose spans lie far
    // apart never runs alongside — its gap is 0, so the ladder may reuse
    // the ordinate space.
    let mut seps: Vec<f64> = cluster
        .windows(2)
        .map(|w| owed(&w[0].0, &w[1].0, clearance, clearance))
        .collect();
    // The chain expresses this cluster only when it is chained whole:
    // every adjacent pair owes a real gap, and every farther pair's debt
    // fits through the gaps between them. A zero gap anywhere means the
    // chain over-constrains — its total order still forces x_i ≤ x_j
    // across the boundary, so a packed stretch crushes a neighbour that
    // owes it nothing (links_medium's fan ports pinned at their windows'
    // edges by the bowl↔dog band) — and an under-sized bridge means it
    // under-constrains, dissolving a pair's pitch. Either way the cluster
    // settles on its true pairwise constraints instead; when the chain
    // holds, the two models' feasible sets coincide and the ladder is the
    // exact, cheaper solve.
    let chain_ok = seps.iter().all(|s| *s > 0.0)
        && (0..n).all(|i| {
            (i + 2..n).all(|j| {
                owed(&cluster[i].0, &cluster[j].0, clearance, clearance)
                    <= seps[i..j].iter().sum::<f64>() + 1e-9
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
            let Some((i, j)) = overrun(&bounds, &seps) else {
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
        super::pairwise::pairwise(&cluster, &prefs, &bounds, clearance)
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
pub(super) fn contend(a: &Item, b: &Item, clearance: f64) -> bool {
    let same_wire = a
        .members
        .iter()
        .any(|(c0, _)| b.members.iter().any(|(c1, _)| c0 == c1));
    let overlap = a.span.0.max(b.span.0) < a.span.1.min(b.span.1);
    overlap || (near(a.span, b.span, clearance) && !same_wire)
}

/// The ordinate pitch two items genuinely owe, at separation `pitch`
/// (the clearance for placement, its floor for the admission probe).
/// Law 1 is a **distance**: runs alongside (spans overlapping) owe the
/// full pitch across; runs past each other owe only what the diagonal
/// needs — tips `g` apart along travel are lawful at ordinate offset `d`
/// once `g² + d² ≥ pitch²`, so a pair whose travel gap alone reaches the
/// pitch may share an ordinate (two collinear segments a clearance
/// apart), and the flat charge that laddered such pairs apart — stage 6's
/// recorded conservatism — is spent. The pair still couples ([`contend`]
/// stays inclusive at exactly a clearance), so round two never forgets
/// the contention; it just owes the truth.
pub(super) fn owed(a: &Item, b: &Item, clearance: f64, pitch: f64) -> f64 {
    if !contend(a, b, clearance) {
        return 0.0;
    }
    let gap = (b.span.0 - a.span.1).max(a.span.0 - b.span.1).max(0.0);
    (pitch * pitch - gap * gap).max(0.0).sqrt()
}

/// Whether two spans come within a clearance of one another — inclusive at
/// exactly a clearance: round one separates contenders by precisely the
/// pitch they owe, so the refreshed spans of a settled pair sit exactly a
/// clearance apart, and a strict test would let round two forget the
/// contention and collapse the pair back together.
fn near(a: (f64, f64), b: (f64, f64), clearance: f64) -> bool {
    b.0 <= a.1 + clearance + 1e-6 && a.0 <= b.1 + clearance + 1e-6
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

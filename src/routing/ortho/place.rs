//! Placement (ROUTING.md model step 5): every run's ordinate, decided once,
//! per corridor, by one mechanism — cluster, order, settle.
//!
//! Runs whose spans come within a clearance of one another **and share
//! ordinate space** — one channel, fragments of one corridor
//! ([`ChannelGraph::corridor`]), or, across worlds, one landing side — form
//! a cluster ([`super::cluster`], the shared contention model). Within a
//! cluster runs order so wires leave in the order they arrive — nested,
//! never braided — by the outward-walk order ([`super::order`]) — and take
//! the order-preserving ordinates nearest their preferences at the pitch
//! each pair genuinely owes ([`cluster::owed`] — the distance model: full
//! clearance alongside, the diagonal remainder past each other): the exact
//! chain ([`ladder`]) when the cluster's contention is a chain, the
//! pairwise projection ([`super::pairwise`]) when an under-sized bridge
//! leaves debt the chain cannot express. The relief valve compresses only
//! what a stretch's hard boxes genuinely cannot hold, never below half the
//! clearance. Preferences are the aesthetic law: interior runs want their
//! corridor's anchor (the midline between two nodes, the keep-out wall at
//! the canvas edge); end runs want the straightest lawful line to their
//! port. Ports *are* end-run ordinates — fan siblings merge into one item
//! and share one port — so a port can never disagree with the wire it
//! serves. Their branch runs meet *on* that trunk rather than run alongside
//! it, so they owe each other nothing where their travel merely abuts
//! ([`cluster::branch_of`]) and the anchor they both prefer is the one
//! point the fan forks at.

use std::collections::BTreeMap;

use super::cluster::{self, Item, clusters_of, merge_fans, owed};
use super::cost::min_pitch;
use super::graph::{Axis, Corridor};
use super::ladder::ladder;
use super::order;
use super::{Chain, Run, World};

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
                branch: cluster::branch_of(chain, ri),
                link: chain.link,
                world: chain.world,
                chan: run.chan,
                landings,
            });
        }
    }
    (ests, by_axis)
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
    // the visible debt. The ladder reports its own infeasibility the same
    // way (crossed boxes — fixed ports leave it no slack to clamp into).
    let ords = feasible
        .then(|| ladder(&prefs, &bounds, &seps))
        .flatten()
        .unwrap_or_else(|| super::pairwise::pairwise(&cluster, &prefs, &bounds, clearance));
    for ((item, _), ord) in cluster.iter().zip(&ords) {
        for &(ci, ri) in &item.members {
            chains[ci].as_mut().expect("placed chain").runs[ri].ord = Some(*ord);
        }
    }
}

#[cfg(test)]
mod tests;

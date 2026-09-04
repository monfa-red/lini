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
//! ([`cluster::branch_of`]) and they fork at one point — the anchor they
//! both prefer, or the split a sibling's own port pins ([`share_forks`]).

use crate::ast::Side;

use std::collections::BTreeMap;

use super::cluster::{self, Item, clusters_of, merge_fans, owed};
use super::cost::min_pitch;
use super::graph::{Axis, Corridor};
use super::ladder::ladder;
use super::order;
use super::{Chain, Run, World};

/// A run's ordinate preference and its hard port window, if any.
type Pref = (f64, Option<(f64, f64)>);

/// Assign every `Run::ord` in every chain — the one pass, probed and
/// refined **to a fixed point**. A run's span tips are its corners — the
/// *perpendicular* runs' ordinates — so every price placement quotes
/// (the diagonal discount, a corridor's reassembled walls, the nesting
/// walk's estimates) is a claim about the other axis's answer. The search
/// hands over provisional spans, so first-round contention is partly
/// phantom: spans that touch only at a shared estimate charge pitch two
/// wires never owe, and the relief valve can then compress a window with
/// room to spare. Each axis therefore settles against spans refreshed
/// from the freshest placed ordinates ([`refresh_axis`] right before it —
/// alternation, never a shared stale snapshot, which oscillates), and the
/// loop stops the moment a full round reproduces its own premises,
/// because only then is the refreshed truth *true*: a round that still
/// moves ordinates has priced separations on corners the drawing won't
/// have. Two rounds were once taken as enough; links_hard at gap 40 drew
/// two runs `√(c²−g²)` apart on a gap `g` the second round's own later
/// settle inverted into an overlap — sub-clearance with room to spare. At
/// the fixed point every span equals its drawn extent and every owed
/// pitch was judged on the final geometry.
///
/// A fixed point need not exist: the discount's gain is unbounded near
/// tangency (`d√(p²−g²)/dg → ∞` as the gap nears the pitch), so two
/// states can each price the other — links_hard at swept clearance 6
/// two-cycles between the flat-charged and the discounted state forever.
/// When the cap falls without convergence, the scene has *proved* the
/// discount's premise unreachable, and placement reprices with the flat
/// charge ([`cluster::owed`]'s `flat`) — full pitch for every contender,
/// no tip read, so the tame map settles — trading a possibly-wider
/// ladder for premises that cannot be inverted. Both legs are pure
/// functions of the input, so Law 4 holds throughout.
///
/// Corners, by contrast, never ride an estimate: a run's drawn extent
/// follows wherever its neighbours finally land, so every ordinate is
/// clamped into its perpendicular neighbours' channel travel extents — the
/// corner stays inside both runs' channels (a run lies in one channel of
/// its axis, ROUTING.md Vocabulary), so a drawn segment can never leave
/// the free space it was priced in, no matter where a later round moves
/// the far corner.
pub(crate) fn place(worlds: &[World], chains: &mut [Option<Chain>], clearance: f64) {
    if !settle_rounds(worlds, chains, clearance, false) {
        settle_rounds(worlds, chains, clearance, true);
    }
}

/// The refine loop at one pricing (`flat` as [`cluster::owed`]): rounds of
/// per-axis refresh-and-settle until a full round reproduces its own
/// premises. True on convergence; false when the cap fell — the last
/// round standing either way.
fn settle_rounds(
    worlds: &[World],
    chains: &mut [Option<Chain>],
    clearance: f64,
    flat: bool,
) -> bool {
    let mut prev: Option<Vec<f64>> = None;
    for _ in 0..PLACE_ROUNDS {
        for axis in [Axis::H, Axis::V] {
            refresh_axis(chains, axis);
            settle_axis(worlds, chains, clearance, axis, flat);
        }
        let now = ords_of(chains);
        if prev.as_ref() == Some(&now) {
            return true;
        }
        prev = Some(now);
    }
    false
}

/// Refine-round cap: every sample across the clearance sweep settles in two
/// or three rounds; the cap only fences hypothetical cycling.
const PLACE_ROUNDS: usize = 8;

/// Every placed ordinate, flattened — the fixed-point test reads exact bit
/// equality, so the loop stops on truth, never on a tolerance.
fn ords_of(chains: &[Option<Chain>]) -> Vec<f64> {
    chains
        .iter()
        .flatten()
        .flat_map(|c| c.runs.iter().map(|r| r.ord.expect("settled run")))
        .collect()
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
    refresh_axis(chains, Axis::H);
    refresh_axis(chains, Axis::V);
}

/// [`refresh_spans`] for one axis's runs alone — their tips are the
/// *perpendicular* runs' ordinates, so refreshing `axis` right before it
/// settles hands it the other axis's freshest answer. A tip whose
/// neighbour is still unplaced (the first round's bootstrap) keeps the
/// search's provisional span.
fn refresh_axis(chains: &mut [Option<Chain>], axis: Axis) {
    for chain in chains.iter_mut().flatten() {
        let n = chain.runs.len();
        if n < 2 {
            continue;
        }
        let ords: Vec<Option<f64>> = chain.runs.iter().map(|r| r.ord).collect();
        for (i, run) in chain.runs.iter_mut().enumerate() {
            if run.axis != axis {
                continue;
            }
            let lo = if i == 0 {
                Some(chain.ends[0].side_coord())
            } else {
                ords[i - 1]
            };
            let hi = if i == n - 1 {
                Some(chain.ends[1].side_coord())
            } else {
                ords[i + 1]
            };
            if let (Some(lo), Some(hi)) = (lo, hi) {
                run.span = (lo.min(hi), lo.max(hi));
            }
        }
    }
}

/// One axis's settle: cluster, order, ladder over that axis's items alone,
/// against the freshest spans ([`refresh_axis`]).
fn settle_axis(
    worlds: &[World],
    chains: &mut [Option<Chain>],
    clearance: f64,
    axis: Axis,
    flat: bool,
) {
    let (ests, mut by_axis) = collect(worlds, chains);
    let Some(mut items) = by_axis.remove(&axis.index()) else {
        return;
    };
    merge_fans(&mut items, chains);
    for cluster in clusters_of(axis, items, worlds, clearance) {
        settle(cluster, clearance, chains, &ests, flat);
    }
}

/// Every run of every chain as a ladder item, grouped by axis, plus each
/// chain's ordinate estimates — the one item model placement settles and
/// admission ([`super::admit`]) probes.
pub(super) fn collect(
    worlds: &[World],
    chains: &[Option<Chain>],
) -> (Vec<Vec<f64>>, BTreeMap<u8, Vec<Item>>) {
    let mut prefs: Vec<Vec<Pref>> = chains
        .iter()
        .map(|c| c.as_ref().map_or(Vec::new(), |ch| chain_prefs(ch, worlds)))
        .collect();
    share_forks(chains, &mut prefs);
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
pub(super) fn chain_prefs(chain: &Chain, worlds: &[World]) -> Vec<Pref> {
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
                let clipped = corridor.clipped(clamp.0, clamp.1);
                // A three-run route whose two ends leave the **same way** —
                // the canonical U (ROUTING.md step 5): both end runs travel
                // outward along one side's normal, so the turn has no reason
                // to go past the outermost of the two side lines. The anchor
                // would centre it in whatever void lies beyond, which the
                // search never priced (its L1 estimate sees no length in the
                // U's depth), and the drawn bight would then move with the
                // empty space around the pair — a tie between two pins
                // orbiting half the sheet, a pull-up's return swinging out
                // past its whole block. One body or two reads the same: with
                // one rect the two side lines coincide.
                if chain.runs.len() == 3 && a.side == b.side {
                    let (ac, bc) = (a.side_coord(), b.side_coord());
                    let t = match a.side {
                        Side::Left | Side::Top => ac.min(bc),
                        Side::Right | Side::Bottom => ac.max(bc),
                    };
                    (t.max(clipped.walls.0).min(clipped.walls.1), None)
                } else {
                    // Where the world states a track quantum (ROUTING.md
                    // §Vocabulary), the anchor rounds to it: a bare run
                    // between two gridded parts bends on their grid, not a
                    // hair off it.
                    let raw = clipped.anchor();
                    let t = worlds[chain.world]
                        .quantum
                        .and_then(|q| q.snap(run.axis, raw, clipped.walls))
                        .unwrap_or(raw);
                    (t, None)
                }
            }
        })
        .collect()
}

/// A fan forks at as few points as it can (ROUTING.md Special nodes): a
/// **branch** run whose ordinate is free — an interior run, otherwise
/// preferring its corridor's anchor — takes the nearest split a sibling's
/// own port already fixes, so the trunk's last fork is one T rather than a
/// split and a turn beside it. Law 3 is indifferent to where a monotone
/// route bends, which is why the preference gets to decide; only a split
/// inside the run's own travel is a candidate, since a farther one would
/// fold the route back on itself and cost real length.
fn share_forks(chains: &[Option<Chain>], prefs: &mut [Vec<Pref>]) {
    let mut splits: Vec<(usize, Axis, f64, usize)> = Vec::new();
    for (ci, chain) in chains.iter().enumerate() {
        let Some(chain) = chain else { continue };
        for (ri, run) in chain.runs.iter().enumerate() {
            if prefs[ci][ri].1.is_none() {
                continue;
            }
            if let Some(fan) = cluster::branch_of(chain, ri) {
                splits.push((fan, run.axis, prefs[ci][ri].0, chain.link));
            }
        }
    }
    for (ci, chain) in chains.iter().enumerate() {
        let Some(chain) = chain else { continue };
        for (ri, run) in chain.runs.iter().enumerate() {
            if ri == 0 || ri + 1 == chain.runs.len() || prefs[ci][ri].1.is_some() {
                continue;
            }
            let Some(fan) = cluster::branch_of(chain, ri) else {
                continue;
            };
            // The run's own travel: what its two neighbours span between the
            // corners they share with it.
            let (a, b) = (chain.runs[ri - 1].span, chain.runs[ri + 1].span);
            let travel = (
                a.0.min(a.1).min(b.0).min(b.1),
                a.0.max(a.1).max(b.0).max(b.1),
            );
            let at = prefs[ci][ri].0;
            let near = splits
                .iter()
                .filter(|(f, ax, ord, _)| {
                    *f == fan && *ax == run.axis && *ord >= travel.0 && *ord <= travel.1
                })
                .min_by(|x, y| {
                    (x.2 - at)
                        .abs()
                        .total_cmp(&(y.2 - at).abs())
                        .then(x.3.cmp(&y.3))
                });
            if let Some(&(_, _, ord, _)) = near {
                prefs[ci][ri].0 = ord;
            }
        }
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
    flat: bool,
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
        .map(|w| owed(&w[0].0, &w[1].0, clearance, clearance, flat))
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
                owed(&cluster[i].0, &cluster[j].0, clearance, clearance, flat)
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
        .unwrap_or_else(|| super::pairwise::pairwise(&cluster, &prefs, &bounds, clearance, flat));
    for ((item, _), ord) in cluster.iter().zip(&ords) {
        for &(ci, ri) in &item.members {
            chains[ci].as_mut().expect("placed chain").runs[ri].ord = Some(*ord);
        }
    }
}

#[cfg(test)]
mod tests;

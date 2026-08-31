//! The placement-aware admission probe (ROUTING.md model step 4: capacity
//! is never exceeded, only priced — and closed where it doesn't exist).
//!
//! The in-search ledger counts a corridor's *load* — the max concurrent
//! spans — and a side's port slots. Placement realises a nesting *order*,
//! and the two can disagree: a full-length run chained between two
//! span-disjoint neighbours needs the chain's total gaps where the
//! point-load counts only two tracks; a side's windows can jointly pinch a
//! group the slot count admits; and a bundle's own corners can spread its
//! legs into a pocket that holds one rail. A route that passes the ledger
//! but cannot be placed at the half-clearance floor would force placement
//! to break Law 1 — so before a route commits, this probe runs the real
//! thing: [`place`](super::place::place) over a copy of every committed
//! chain plus the candidate's rails, spans refreshed from the final
//! ordinates, and the drawn gaps judged against the floor. No separate
//! model, so nothing to drift: what the probe clears is exactly what
//! placement will draw (given the wires routed so far — later links carry
//! their own burden). A route the simulation cannot place lawfully becomes
//! a learned closure ([`super::search::Deny`]) and the world searches
//! again around it — the same loop the ledger's own denials ride.

use super::cluster;
use super::cost::min_pitch;
use super::graph::Axis;
use super::place;
use super::search::Deny;
use super::{Chain, World};

/// Judge `candidate` (× its bundle's `k` rails) by placing it beside every
/// committed chain. `None` admits; otherwise a failing candidate run's
/// channel-span, ready to deny.
pub(crate) fn admits(
    worlds: &[World],
    committed: &[Option<Chain>],
    candidate: &Chain,
    k: usize,
    clearance: f64,
) -> Option<Deny> {
    let base = committed.len();
    // Only the candidate's **world** simulates: worlds are separate graphs,
    // so chains elsewhere share no channel with the candidate and their
    // placement cannot move for it — re-placing them per probe was most of a
    // busy sheet's routing time, spent proving nothing.
    let mut all: Vec<Option<Chain>> = committed
        .iter()
        .map(|c| c.as_ref().filter(|ch| ch.world == candidate.world).cloned())
        .collect();
    all.extend(std::iter::repeat_with(|| Some(candidate.clone())).take(k.max(1)));
    place::place(worlds, &mut all, clearance);
    place::refresh_spans(&mut all);

    let ord = |(ci, ri): (usize, usize)| {
        all[ci].as_ref().expect("simulated chain").runs[ri]
            .ord
            .expect("simulation placed every run")
    };
    let of_candidate = |i: &cluster::Item| i.members.iter().any(|&(ci, _)| ci >= base);
    let (_, by_axis) = place::collect(worlds, &all);
    for (axis, mut items) in by_axis {
        let axis = [Axis::H, Axis::V][axis as usize];
        cluster::merge_fans(&mut items, &all);
        for cluster in cluster::clusters_of(axis, items, worlds, clearance) {
            let broken = (0..cluster.len()).find_map(|i| {
                (i + 1..cluster.len()).find_map(|j| {
                    let (a, b) = (&cluster[i].0, &cluster[j].0);
                    // The floor of the distance model: what the pair's
                    // diagonal needs at half-clearance separation.
                    // Judged on the drawn spans, so the diagonal is exact
                    // here — never `flat`.
                    let floor = cluster::owed(a, b, clearance, min_pitch(clearance), false);
                    let short = (ord(a.members[0]) - ord(b.members[0])).abs() + 1e-6 < floor;
                    short.then_some((i, j))
                })
            });
            // A run's ordinate must also hold its own law bounds judged on
            // the **drawn** spans: placement prices each run against the
            // corridor of the span it currently believes, and a settle that
            // then moves the far corner can leave the drawn span reaching
            // past the void the ordinate was lawful in (links_medium at
            // clearance 16 parked a jog over a keep-out that way — 5.5
            // from a body, a Law-1 breach no pitch floor sees). The same
            // `bound` placement clamps into, judged once more on the final
            // geometry.
            let broken = broken.or_else(|| {
                (0..cluster.len()).find_map(|i| {
                    let b = place::bound(&cluster[i]);
                    let o = ord(cluster[i].0.members[0]);
                    (o < b.0 - 1e-6 || o > b.1 + 1e-6).then_some((i, i))
                })
            });
            let Some((i, j)) = broken else { continue };
            // Deny a candidate run — the violating pair's own when it has
            // one, else any in the offended cluster (the candidate's
            // arrival shifted it), else the candidate's first run: a
            // lawful closure either way, and the search's no-progress
            // guard bounds the retries.
            let item = [&cluster[i].0, &cluster[j].0]
                .into_iter()
                .find(|it| of_candidate(it))
                .or_else(|| cluster.iter().map(|(it, _)| it).find(|it| of_candidate(it)));
            return Some(match item {
                Some(it) => (axis, it.chan, it.span),
                None => {
                    let r = &candidate.runs[0];
                    (r.axis, r.chan, r.span)
                }
            });
        }
    }
    None
}

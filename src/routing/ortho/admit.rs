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
    let mut all: Vec<Option<Chain>> = committed.to_vec();
    all.extend(std::iter::repeat_with(|| Some(candidate.clone())).take(k.max(1)));
    place::place(worlds, &mut all, clearance);
    place::refresh_spans(&mut all);

    let ord = |(ci, ri): (usize, usize)| {
        all[ci].as_ref().expect("simulated chain").runs[ri]
            .ord
            .expect("simulation placed every run")
    };
    let of_candidate = |i: &place::Item| i.members.iter().any(|&(ci, _)| ci >= base);
    let (_, by_axis) = place::collect(worlds, &all);
    for (axis, mut items) in by_axis {
        let axis = [Axis::H, Axis::V][axis as usize];
        place::merge_fans(&mut items, &all);
        for cluster in place::clusters_of(axis, items, worlds, clearance) {
            let broken = (0..cluster.len()).find_map(|i| {
                (i + 1..cluster.len()).find_map(|j| {
                    let (a, b) = (&cluster[i].0, &cluster[j].0);
                    let owed = place::contend(a, b, clearance)
                        && (ord(a.members[0]) - ord(b.members[0])).abs() + 1e-6
                            < min_pitch(clearance);
                    owed.then_some((i, j))
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

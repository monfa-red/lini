//! The contention model (ROUTING.md model step 5): every run as a ladder
//! **item**, the clusters that contend for one axis's ordinate space, and
//! the pitch each pair genuinely owes — the one distance law placement
//! ([`super::place`]), the admission probe ([`super::admit`]), and the
//! pairwise settle ([`super::pairwise`]) all judge by.

use std::collections::BTreeMap;

use super::graph::{Axis, Corridor};
use super::rect::Rect;
use super::{Chain, World};
use crate::ast::Side;

/// One ladder item: a run (or a fan's merged end runs) awaiting its
/// ordinate.
pub(super) struct Item {
    /// `(chain index, run index)` of every run taking this ordinate.
    pub(super) members: Vec<(usize, usize)>,
    pub(super) span: (f64, f64),
    /// The corner clamp ([`super::place`]) — hard bounds keeping every
    /// corner inside both of its runs' channels.
    pub(super) clamp: (f64, f64),
    pub(super) pref: f64,
    /// Hard bounds from the port window; `None` for interior runs (the
    /// corridor's usable range applies alone).
    pub(super) window: Option<(f64, f64)>,
    /// Declaration-order key for span ties.
    pub(super) link: usize,
    /// The world whose channel graph this run rides in.
    pub(super) world: usize,
    /// The channel the run rides — fragments of one corridor cluster across
    /// channels.
    pub(super) chan: usize,
    /// The physical sides an end run lands on (both for a single-run wire).
    /// Worlds share these: an inner wire's port and an outer wire's punch
    /// meet on the same body side, so same-landing items cluster across
    /// worlds — the one place two worlds' wires lawfully share space.
    pub(super) landings: Vec<(Side, Rect)>,
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
                    (Some(a), Some(b)) => Some(meet(a, b)),
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

/// Where two fan siblings' windows agree — their intersection, and the
/// **first** of them when they do not meet.
///
/// They usually do: siblings share a port, so an end run of each carries the
/// same side's window. Two things break that, and neither is a bug:
///
/// - a **single-run** sibling carries the pair of *its own* two ends' windows
///   ([`super::place`]'s `chain_prefs`), so a fan of two straight legs to far
///   ends at opposite extremes offers two disjoint slices of the shared side;
/// - a free window is clipped per link by the blockers that link's punch
///   crosses ([`super::entry`]), and a blocker splitting the side keeps the
///   wider shore — which can be a different shore for each sibling, since a
///   link's own endpoints are passable to it alone.
///
/// Only a **fixed** port makes the intersection a guarantee: it collapses the
/// window to its point, and a fan whose shared end carries two different fixed
/// ports strays whole before it ever reaches placement (ROUTING.md Fixed
/// ports). So an empty meet is a real layout, not a broken invariant — and the
/// merged item still owes the ladder one valid interval. The first window
/// stands, in item order, which is the same repair `place::law_range` and
/// `place::bound` make when a tightening inverts: keep the bound you had, let
/// the later leg jog to reach the shared landing.
fn meet(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let both = (a.0.max(b.0), a.1.min(b.1));
    if both.0 <= both.1 { both } else { a }
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

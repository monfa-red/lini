//! Crossing collection and the bounded reroute audit (LINKING §Model step 6).
//!
//! Crossings are collected geometrically from the drawn chains: every
//! transversal H×V intersection strictly inside both segments — an
//! inversion's swap jog and a perpendicular piercing alike. Fan trunks
//! overlap and ports T-join; neither crosses, so strictness excludes them.
//! Each crossing is audited in declaration order of the later link: its
//! bundle reroutes with the cell containing the crossing closed, and the
//! reroute is kept iff the diagram's total crossing count strictly drops.
//! Rounds repeat only while the total decreases — termination by
//! construction, no oscillation. Whatever remains is forced and reported.

use super::capacity::{Occupancy, Ports};
use super::geometry;
use super::runs::{Chain, Slides};
use super::{Router, solve};
use crate::ast::Side;
use std::collections::{BTreeMap, BTreeSet};

/// One drawn crossing: the two chains as `(earlier, later)` declaration
/// ranks, and the intersection point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crossing {
    pub pair: (usize, usize),
    pub at: (f64, f64),
}

/// Polylines with their bounding boxes — most link pairs are far apart, so
/// every pairwise sweep prefilters on boxes before walking segments.
type BoxedPoly = (Vec<(f64, f64)>, (f64, f64, f64, f64));

fn boxed_polys(chains: &[Option<Chain>]) -> Vec<Option<BoxedPoly>> {
    chains
        .iter()
        .map(|c| {
            c.as_ref().map(|c| {
                let p = geometry::polyline(c);
                let bb = p.iter().fold(
                    (
                        f64::INFINITY,
                        f64::INFINITY,
                        f64::NEG_INFINITY,
                        f64::NEG_INFINITY,
                    ),
                    |(x0, y0, x1, y1), &(x, y)| (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                );
                (p, bb)
            })
        })
        .collect()
}

fn boxes_apart(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64), gap: f64) -> bool {
    b.0 - a.2 >= gap || a.0 - b.2 >= gap || b.1 - a.3 >= gap || a.1 - b.3 >= gap
}

/// Every transversal crossing between drawn links, ordered by the later
/// link's declaration rank — the order the audit visits them in.
pub fn collect(chains: &[Option<Chain>]) -> Vec<Crossing> {
    let polys = boxed_polys(chains);
    let mut out = Vec::new();
    for i in 0..polys.len() {
        for j in i + 1..polys.len() {
            let (Some((a, ba)), Some((b, bb))) = (&polys[i], &polys[j]) else {
                continue;
            };
            if boxes_apart(*ba, *bb, 0.0) {
                continue;
            }
            for sa in a.windows(2) {
                for sb in b.windows(2) {
                    if let Some(at) = cross(sa, sb) {
                        out.push(Crossing { pair: (i, j), at });
                    }
                }
            }
        }
    }
    out.sort_by(|x, y| {
        (x.pair.1, x.pair.0)
            .cmp(&(y.pair.1, y.pair.0))
            .then(x.at.0.total_cmp(&y.at.0))
            .then(x.at.1.total_cmp(&y.at.1))
    });
    out
}

/// The transversal intersection of two orthogonal segments: one horizontal,
/// one vertical, each strictly inside the other — touches and collinear
/// overlaps are contact, not crossings. Shared with the independent checker:
/// pure geometry, and a miss here surfaces as a separation breach there.
pub(crate) fn cross(a: &[(f64, f64)], b: &[(f64, f64)]) -> Option<(f64, f64)> {
    let (h, v) = if a[0].1 == a[1].1 && b[0].0 == b[1].0 {
        (a, b)
    } else if a[0].0 == a[1].0 && b[0].1 == b[1].1 {
        (b, a)
    } else {
        return None;
    };
    let (x, y) = (v[0].0, h[0].1);
    let (hx0, hx1) = (h[0].0.min(h[1].0), h[0].0.max(h[1].0));
    let (vy0, vy1) = (v[0].1.min(v[1].1), v[0].1.max(v[1].1));
    (hx0 < x && x < hx1 && vy0 < y && y < vy1).then_some((x, y))
}

/// The bounded reroute audit: drive `raw`/`drawn` toward fewer crossings and
/// return the forced ones that remain.
///
/// Each round, every bundle involved in a crossing proposes reroute
/// candidates (most-crossed bundle first): its path search re-runs with the
/// **transversal count against every drawn link** leading the cost — a link
/// always detours rather than crosses, and parallel corridor sharing stays
/// free. The round applies the single candidate that lowers the diagram's
/// actual crossing count the most without raising its law-1 breach count.
/// At a plateau, paired moves run over the heaviest crossers: one bundle
/// steps aside (any candidate, even a lateral or worse one), then another
/// retries from there — kept iff the pair strictly improves. Every applied
/// move strictly decreases the total, so the audit terminates by
/// construction and the kept state is never worse than the routed one.
pub fn run(
    router: &Router,
    raw: &mut Vec<Option<Chain>>,
    drawn: &mut Vec<Option<Chain>>,
    clearance: f64,
    slides: &Slides,
) -> Vec<Crossing> {
    loop {
        let crossings = collect(drawn);
        let total = crossings.len();
        if total == 0 {
            return Vec::new();
        }
        // A reroute may trade a crossing for a separation or body conflict;
        // never accept one that leaves the diagram with more law-1 breaches.
        let base = law_score(router, drawn, clearance).0;
        let clean = |c: &Candidate| law_score(router, &c.2, clearance).0 <= base;
        let bundles = entangled_bundles(router, &crossings);
        // The single reroute that lowers the diagram's crossing count the
        // most; one move per round, so the descent is order-independent.
        let mut best: Option<Candidate> = None;
        let mut firsts: Vec<(usize, Vec<Candidate>)> = Vec::new();
        for &bi in &bundles {
            let cands = retry(router, raw, drawn, bi, clearance, slides);
            for c in &cands {
                if c.0 < total && best.as_ref().is_none_or(|b| c.0 < b.0) && clean(c) {
                    best = Some(c.clone());
                }
            }
            firsts.push((bi, cands));
        }
        // At a plateau, paired moves: one bundle steps aside (any candidate,
        // even a lateral or worse one), then another retries from there —
        // the deadlock where A blocks B's only clean detour while A itself
        // has no reason to move. Kept iff the pair strictly improves. The
        // sweep is capped to the most entangled legs: every second-leg
        // attempt pays a full re-solve, and a deadlock involves the heavy
        // crossers by definition.
        if best.is_none() {
            'pairs: for (a, cas) in firsts.iter().take(2) {
                for ca in cas.iter().take(2) {
                    for b in entangled_bundles(router, &collect(&ca.2))
                        .into_iter()
                        .take(3)
                    {
                        if b == *a {
                            continue;
                        }
                        for cb in retry(router, &ca.1, &ca.2, b, clearance, slides) {
                            if cb.0 < total && clean(&cb) {
                                best = Some(cb);
                                break 'pairs;
                            }
                        }
                    }
                }
            }
        }
        // The total strictly decreases every applied move, so the audit
        // terminates by construction.
        let Some((_, cand, cand_drawn)) = best else {
            return crossings;
        };
        *raw = cand;
        *drawn = cand_drawn;
    }
}

type Candidate = (usize, Vec<Option<Chain>>, Vec<Option<Chain>>);

/// A repair move: the rerouted raw chains and their solved drawing.
type Move = (Vec<Option<Chain>>, Vec<Option<Chain>>);

/// Law 1 in full over drawn chains, plus the crossing count: the ground
/// truth every audit move is judged by — link–link conflicts, link–body
/// intrusions, then crossings, lexicographic.
pub fn law_score(router: &Router, chains: &[Option<Chain>], clearance: f64) -> (usize, usize) {
    (
        breaches(chains, clearance).len() + body_breaches(router, chains, clearance).len(),
        collect(chains).len(),
    )
}

/// Chains drawn nearer than `clearance` to a node body — Law 1's other
/// half, judged like the validator but on chains: only the stub segment
/// surrenders its own endpoint's keep-out, containment parents excepted.
/// Returns the offending chain and the body it grazes.
pub fn body_breaches(
    router: &Router,
    chains: &[Option<Chain>],
    clearance: f64,
) -> Vec<(usize, super::rect::Rect)> {
    let mut out = Vec::new();
    for (ci, chain) in chains.iter().enumerate() {
        let Some(chain) = chain else {
            continue;
        };
        let poly = geometry::polyline(chain);
        if poly.len() < 2 {
            continue;
        }
        let req = &router.reqs[chain.req];
        let segs = poly.len() - 1;
        for r in router.index.solid_rects_for([&req.a_path, &req.b_path]) {
            if poly
                .windows(2)
                .any(|s| seg_rect_dist(s, &r) < clearance - 1e-6)
            {
                out.push((ci, r));
            }
        }
        for (e, end) in chain.ends.iter().enumerate() {
            let partner = &chain.ends[1 - e].path;
            if super::SceneIndex::contains(&end.path, partner) {
                continue;
            }
            let hit = poly.windows(2).enumerate().any(|(k, s)| {
                let own_stub = (k == 0 && chain.ends[0].path == end.path)
                    || (k == segs - 1 && chain.ends[1].path == end.path);
                !own_stub && seg_rect_dist(s, &end.rect) < clearance - 1e-6
            });
            if hit {
                out.push((ci, end.rect));
            }
        }
    }
    out
}

/// Distance between an axis-aligned segment and a rect, both as boxes.
fn seg_rect_dist(s: &[(f64, f64)], r: &super::rect::Rect) -> f64 {
    let (sx0, sx1) = (s[0].0.min(s[1].0), s[0].0.max(s[1].0));
    let (sy0, sy1) = (s[0].1.min(s[1].1), s[0].1.max(s[1].1));
    let dx = (r.x0 - sx1).max(sx0 - r.x1).max(0.0);
    let dy = (r.y0 - sy1).max(sy0 - r.y1).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

/// Link pairs of the drawn chains nearer than `clearance` anywhere along
/// their polylines, excluding the sanctioned contacts — transversal
/// crossings and fan siblings. Ordered like [`collect`]: later link first.
pub fn breaches(chains: &[Option<Chain>], clearance: f64) -> Vec<(usize, usize)> {
    let polys = boxed_polys(chains);
    let rows = compacted_bands(chains, clearance);
    let mut out = Vec::new();
    for i in 0..chains.len() {
        for j in i + 1..chains.len() {
            let (Some((a, ba)), Some((b, bb))) = (&polys[i], &polys[j]) else {
                continue;
            };
            if boxes_apart(*ba, *bb, clearance - 1e-6) {
                continue;
            }
            let (ca, cb) = (chains[i].as_ref().unwrap(), chains[j].as_ref().unwrap());
            let fans = |c: &Chain| [c.ends[0].fan, c.ends[1].fan];
            let fan_pair = fans(ca)
                .iter()
                .flatten()
                .any(|g| fans(cb).contains(&Some(*g)));
            let bands = shared_bands(&rows, ca, cb);
            let hit = a.windows(2).any(|sa| {
                b.windows(2).any(|sb| {
                    cross(sa, sb).is_none()
                        && seg_dist(sa, sb) < clearance - 1e-6
                        && !(fan_pair && trunk_contact(sa, sb, a, b))
                        && !bands.iter().any(|&band| band_contact(band, sa, sb))
                })
            });
            if hit {
                out.push((i, j));
            }
        }
    }
    out.sort_by_key(|&(a, b)| (b, a));
    out
}

/// Law-1 repair, the crossing audit's sibling: port spread settles ordinates
/// only after routing, so two pinned approaches can land nearer than
/// clearance with no closure able to foresee it. While any pair of drawn
/// links conflicts, the round pools every repair — retrying one of the pair
/// (or a link sharing a port side with one) with the conflict sites walled
/// off (stubs and runs alike), rerouting body-graziers off the body, and
/// sliding a port group along its side off the conflicted row (LINKING
/// §Ports) — and applies the **best** strict improvement of
/// `(breaches, crossings)`, the crossing audit's own discipline: a
/// first-found accept can take a crossing-heavy reroute when a gentle slide
/// repairs the same conflict. At a plateau a second link moves from a first
/// one's candidate state. A conflict nothing resolves undraws its later
/// link — reported, never drawn dirty — so the audit terminates with zero
/// breaches by construction. Returns the chains it had to undraw.
///
/// `give_up` bounds the undraw ladder for speculative work: a caller
/// repairing an insertion candidate stops once more links are undrawn than
/// the state the candidate must beat — completeness leads the acceptance
/// score and undraws are monotone, so the candidate is provably rejected
/// and finishing the repair is wasted work. The standalone pass runs
/// unbounded (`usize::MAX`).
pub fn separation(
    router: &Router,
    raw: &mut Vec<Option<Chain>>,
    drawn: &mut Vec<Option<Chain>>,
    clearance: f64,
    slides: &mut Slides,
    give_up: usize,
) -> Vec<usize> {
    separation_with_protect(router, raw, drawn, clearance, slides, give_up, &[])
}

#[allow(clippy::too_many_arguments)]
fn separation_with_protect(
    router: &Router,
    raw: &mut Vec<Option<Chain>>,
    drawn: &mut Vec<Option<Chain>>,
    clearance: f64,
    slides: &mut Slides,
    give_up: usize,
    protected: &[usize],
) -> Vec<usize> {
    let mut undrawn = Vec::new();
    loop {
        if drawn.iter().filter(|c| c.is_none()).count() > give_up {
            return undrawn;
        }
        let cur = breaches(drawn, clearance);
        let bodies = body_breaches(router, drawn, clearance);
        if cur.is_empty() && bodies.is_empty() {
            return undrawn;
        }
        let total = law_score(router, drawn, clearance);
        let score = |d: &[Option<Chain>]| law_score(router, d, clearance);
        // The round's repair pool. Every candidate is judged on its
        // re-solved drawing; the best strict improvement wins, first found
        // breaking ties, so rounds stay deterministic.
        let mut best: Option<((usize, usize), Move, Option<Slides>)> = None;
        let mut firsts: Vec<(usize, Vec<Move>)> = Vec::new();
        for &pair in &cur {
            for mover in movers(drawn, pair) {
                let cands = nudge(router, raw, drawn, mover, pair, clearance, slides);
                for cand in &cands {
                    let s = score(&cand.1);
                    if s < total && best.as_ref().is_none_or(|(bs, ..)| s < *bs) {
                        best = Some((s, cand.clone(), None));
                    }
                }
                firsts.push((mover, cands));
            }
        }
        // A link grazing a node body reroutes with that body's
        // surroundings walled off.
        for &(ci, site) in &bodies {
            let deny = vec![site.inflate(clearance - 1e-6)];
            for cand in nudged(
                router,
                raw,
                drawn,
                ci,
                deny,
                BTreeSet::new(),
                clearance,
                slides,
                false,
            ) {
                let s = score(&cand.1);
                if s < total && best.as_ref().is_none_or(|(bs, ..)| s < *bs) {
                    best = Some((s, cand, None));
                }
            }
        }
        // Port-group slides: the conflict may sit at the port row
        // itself, where no reroute can move a pinned approach. A side
        // is eligible only when the partner link passes within
        // clearance of the port — the exact situation Law 2's slide
        // clause names and the independent checker can verify. Nearest
        // offsets first; ground truth judges.
        for &pair in &cur {
            for (ci, partner) in [(pair.1, pair.0), (pair.0, pair.1)] {
                let Some(chain) = drawn[ci].clone() else {
                    continue;
                };
                let Some(partner_chain) = drawn[partner].as_ref() else {
                    continue;
                };
                let partner_poly = geometry::polyline(partner_chain);
                for end in &chain.ends {
                    let port = [(end.port.0, end.port.1), (end.port.0, end.port.1)];
                    let at_mouth = partner_poly
                        .windows(2)
                        .any(|s| seg_dist(&port, s) < clearance - 1e-6);
                    let partner_shares_side = partner_chain
                        .ends
                        .iter()
                        .any(|e| e.path == end.path && e.side == end.side);
                    if !at_mouth || partner_shares_side {
                        continue;
                    }
                    let key = (end.path.clone(), end.side.index());
                    let base = slides.get(&key).copied().unwrap_or(0.0);
                    for step in [1.0, -1.0, 2.0, -2.0] {
                        let mut cand_slides = slides.clone();
                        cand_slides.insert(key.clone(), base + step * clearance);
                        let cand_drawn = solve(&router.worlds, raw, clearance, &cand_slides);
                        let s = score(&cand_drawn);
                        if s < total
                            && contact_holds(&cand_drawn, clearance)
                            && best.as_ref().is_none_or(|(bs, ..)| s < *bs)
                        {
                            best = Some((s, (raw.clone(), cand_drawn), Some(cand_slides)));
                        }
                    }
                }
            }
        }
        let mut accepted: Option<(Move, Option<Slides>)> = best.map(|(_, mv, sl)| (mv, sl));
        if accepted.is_none() && give_up == usize::MAX {
            // Paired moves: one link steps aside (even laterally), a second
            // retries from there — kept iff the pair strictly improves on
            // the original state. Speculative repairs (finite `give_up`)
            // skip this stage: the completeness pass follows displaced
            // incumbents at the insertion level.
            'pairs: for (m1, cas) in &firsts {
                for ca in cas {
                    for &pair2 in &breaches(&ca.1, clearance) {
                        for m2 in movers(&ca.1, pair2) {
                            if m2 == *m1 {
                                continue;
                            }
                            for cb in nudge(router, &ca.0, &ca.1, m2, pair2, clearance, slides) {
                                if score(&cb.1) < total {
                                    accepted = Some((cb, None));
                                    break 'pairs;
                                }
                            }
                        }
                    }
                }
            }
        }
        match accepted {
            Some(((cand, cand_drawn), slid)) => {
                if let Some(slid) = slid {
                    *slides = slid;
                }
                *raw = cand;
                *drawn = cand_drawn;
            }
            None => {
                let later = cur.first().map_or_else(
                    || {
                        bodies
                            .iter()
                            .map(|&(ci, _)| ci)
                            .find(|ci| !protected.contains(ci))
                            .unwrap_or(bodies[0].0)
                    },
                    |&(a, b)| {
                        if protected.contains(&b) && !protected.contains(&a) {
                            a
                        } else {
                            b
                        }
                    },
                );
                raw[later] = None;
                *drawn = solve(&router.worlds, raw, clearance, slides);
                undrawn.push(later);
            }
        }
    }
}

/// One repaired insertion candidate: the raw chains, their solved drawing,
/// the slides the repair settled on, and the links it undrew in exchange.
type Insertion = (Vec<Option<Chain>>, Vec<Option<Chain>>, Slides, Vec<usize>);

/// The completeness pass — starvation rip-up (LINKING §Impossible layouts).
/// Routing is first-come in declaration order, and a hard closure never
/// asks an incumbent to move: a late bundle can starve while plenty of
/// legal geometry remains. Every bundle with an undrawn member proposes
/// repaired insertions (see [`insertions`]); at a plateau, paired moves —
/// an insertion that merely displaced an incumbent stands if the
/// displaced bundle finds a new home from there. Ground truth judges
/// every move — kept iff `(undrawn, conflicts, crossings)` strictly drops
/// lexicographically, completeness first — so the pass terminates by
/// descent and never trades a law for a link. Returns the links accepted
/// repairs undrew in exchange.
pub fn complete(
    router: &mut Router,
    raw: &mut Vec<Option<Chain>>,
    drawn: &mut Vec<Option<Chain>>,
    clearance: f64,
    slides: &mut Slides,
) -> Vec<usize> {
    let mut undrawn = Vec::new();
    // Identical inputs fail identically (Law 4), and a split restarts the
    // pass without touching the drawn state — so candidate sets are
    // memoised per (bundle members, state version) and the swap and
    // compaction stages skip when nothing they read has changed. The
    // version advances on every accepted move (anything touching
    // raw/drawn/slides); only what a restart actually changed recomputes.
    let mut version: u64 = 0;
    let mut memo: std::collections::BTreeMap<Vec<usize>, (u64, Vec<Insertion>)> =
        Default::default();
    let mut swaps_tried: BTreeSet<(u64, Vec<Vec<usize>>)> = BTreeSet::new();
    let mut compacts_tried: BTreeSet<(u64, Vec<usize>)> = BTreeSet::new();
    'pass: loop {
        let total = completeness_score(router, drawn, clearance);
        let better = |c: &Insertion| completeness_score(router, &c.1, clearance) < total;
        let starved: Vec<usize> = router
            .bundles
            .iter()
            .enumerate()
            .filter(|(_, b)| {
                let rep = &router.reqs[b.members[0]];
                rep.a_path != rep.b_path && b.members.iter().any(|&m| raw[m].is_none())
            })
            .map(|(bi, _)| bi)
            .collect();
        let mut firsts: Vec<Vec<Insertion>> = Vec::new();
        for &bi in &starved {
            let key = router.bundles[bi].members.clone();
            let mut cands = match memo.get(&key) {
                Some((v, c)) if *v == version => c.clone(),
                _ => {
                    let c = insertions(router, raw, drawn, bi, clearance, slides, total);
                    memo.insert(key, (version, c.clone()));
                    c
                }
            };
            if let Some(i) = cands.iter().position(&better) {
                let (cand, cand_drawn, cand_slides, lost) = cands.swap_remove(i);
                adopt_fan_sides(router, &cand);
                *raw = cand;
                *drawn = cand_drawn;
                *slides = cand_slides;
                undrawn.extend(lost);
                version += 1;
                continue 'pass;
            }
            firsts.push(cands);
        }
        // The plateau: every single insertion was a swap or worse. A swap
        // still opens a door — the displaced bundle routes again from the
        // swapped state, and the pair of moves is kept iff the whole
        // strictly improves on the original.
        let starved_keys: Vec<Vec<usize>> = starved
            .iter()
            .map(|&bi| router.bundles[bi].members.clone())
            .collect();
        if swaps_tried.insert((version, starved_keys)) {
            for (cand, cand_drawn, cand_slides, lost) in firsts.iter().flatten() {
                for &l in lost {
                    let bj = bundle_of(router, l);
                    let mut seconds =
                        insertions(router, cand, cand_drawn, bj, clearance, cand_slides, total);
                    if let Some(i) = seconds.iter().position(&better) {
                        let (second, second_drawn, second_slides, lost2) = seconds.swap_remove(i);
                        adopt_fan_sides(router, &second);
                        *raw = second;
                        *drawn = second_drawn;
                        *slides = second_slides;
                        undrawn.extend(lost.iter().copied());
                        undrawn.extend(lost2);
                        version += 1;
                        continue 'pass;
                    }
                }
            }
        }
        // Degradation under pressure (LINKING §Duplicates): a bundle no
        // insertion places whole — or one whose displacement kept blocking
        // every swap — splits, and the next pass tries the pieces alone.
        // Splits strictly grow the bundle count, so the loop stays bounded.
        let displaced = firsts
            .iter()
            .flatten()
            .flat_map(|c| &c.3)
            .map(|&l| bundle_of(router, l));
        let blocking: Vec<usize> = starved.iter().copied().chain(displaced).collect();
        for bi in blocking {
            if router.splittable(bi) {
                super::bundle::split(&mut router.bundles, bi);
                continue 'pass;
            }
        }
        // Port compaction — the very last routing lever before the
        // impossible report (LINKING §Ports, Law 2's compaction clause):
        // every rip-up, swap, and split spent, a link starved of port
        // slots lands on a full side, and the side re-pitches all its
        // ports evenly below clearance, like the pins of an IC.
        for &bi in &starved {
            let key = router.bundles[bi].members.clone();
            if !compacts_tried.insert((version, key)) {
                continue;
            }
            if compact_insertion(
                router,
                raw,
                drawn,
                bi,
                clearance,
                slides,
                &mut undrawn,
                total,
                true,
            ) {
                version += 1;
                continue 'pass;
            }
        }
        return undrawn;
    }
}

/// The port-compaction lever for one starved bundle (LINKING Law 2's
/// compaction clause): route with full sides reopened
/// ([`Router::route_bundle`]'s compact mode) and hand the result to the
/// separation audit like any insertion. Stateless by design — the side's
/// port count is the state: `place_ports` re-pitches every unit on an
/// over-asked side evenly inside `solve`, so there is nothing to register
/// and nothing to roll back; the candidate stands or falls on drawn ground
/// truth, completeness first. A candidate that displaced an incumbent gets
/// the swap stage's follow-up: the displaced bundle retries from the
/// compacted state — by plain insertion, then by one more compaction
/// (`rehome` bounds the recursion) — and the pair stands iff the whole
/// strictly improves.
#[allow(clippy::too_many_arguments)]
fn compact_insertion(
    router: &mut Router,
    raw: &mut Vec<Option<Chain>>,
    drawn: &mut Vec<Option<Chain>>,
    bi: usize,
    clearance: f64,
    slides: &mut Slides,
    undrawn: &mut Vec<usize>,
    total: (usize, usize, usize),
    rehome: bool,
) -> bool {
    let members = router.bundles[bi].members.clone();
    for cleared in [false, true] {
        let occ = if cleared {
            Occupancy::new(clearance)
        } else {
            occupancy_without(raw, &members, clearance)
        };
        let ports = ports_without(raw, &members, clearance);
        let polys: Vec<Vec<(f64, f64)>> = (0..drawn.len())
            .filter(|w| !members.contains(w))
            .filter_map(|w| drawn[w].as_ref().map(geometry::polyline))
            .collect();
        let count = counter(&polys);
        let Some(picked) = [false, true].into_iter().find_map(|relaxed| {
            router.route_bundle(
                bi,
                &occ,
                &ports,
                &count,
                &[],
                [None, None],
                relaxed,
                true,
                None,
            )
        }) else {
            continue;
        };
        let mut cand = raw.clone();
        router.build_chains(bi, &picked, &mut cand);
        let beat = drawn.iter().filter(|c| c.is_none()).count();
        let mut cand_drawn = solve(&router.worlds, &cand, clearance, slides);
        let mut cand_slides = slides.clone();
        let lost = separation(
            router,
            &mut cand,
            &mut cand_drawn,
            clearance,
            &mut cand_slides,
            beat,
        );
        if completeness_score(router, &cand_drawn, clearance) < total {
            *raw = cand;
            *drawn = cand_drawn;
            *slides = cand_slides;
            undrawn.extend(lost);
            return true;
        }
        for &l in &lost {
            let bj = bundle_of(router, l);
            let mut seconds = insertions(
                router,
                &cand,
                &cand_drawn,
                bj,
                clearance,
                &cand_slides,
                total,
            );
            let better = |c: &Insertion| completeness_score(router, &c.1, clearance) < total;
            if let Some(i) = seconds.iter().position(better) {
                let (second, second_drawn, second_slides, lost2) = seconds.swap_remove(i);
                adopt_fan_sides(router, &second);
                *raw = second;
                *drawn = second_drawn;
                *slides = second_slides;
                undrawn.extend(lost.iter().copied());
                undrawn.extend(lost2);
                return true;
            }
            if !rehome {
                continue;
            }
            let mut second = cand.clone();
            let mut second_drawn = cand_drawn.clone();
            let mut second_slides = cand_slides.clone();
            let mut second_lost = Vec::new();
            if compact_insertion(
                router,
                &mut second,
                &mut second_drawn,
                bj,
                clearance,
                &mut second_slides,
                &mut second_lost,
                total,
                false,
            ) {
                *raw = second;
                *drawn = second_drawn;
                *slides = second_slides;
                undrawn.extend(lost.iter().copied());
                undrawn.extend(second_lost);
                return true;
            }
        }
    }
    false
}

/// Law 1's third surrender (LINKING — the row band of a compacted side):
/// per `(node, side)`, the outermost port ordinates of every row holding
/// two distinct ports nearer than clearance. Sub-clearance pitch exists
/// only where `place_ports` compacted the side, so the pitch itself is the
/// signature.
fn compacted_bands(chains: &[Option<Chain>], clearance: f64) -> BTreeMap<(String, u8), (f64, f64)> {
    let mut ords: BTreeMap<(String, u8), Vec<f64>> = BTreeMap::new();
    for chain in chains.iter().flatten() {
        for e in &chain.ends {
            let o = match e.side {
                Side::Left | Side::Right => e.port.1,
                Side::Top | Side::Bottom => e.port.0,
            };
            ords.entry((e.path.clone(), e.side.index()))
                .or_default()
                .push(o);
        }
    }
    let mut out = BTreeMap::new();
    for (key, mut os) in ords {
        os.sort_by(f64::total_cmp);
        os.dedup_by(|a, b| (*a - *b).abs() <= 1e-6);
        let tight = os.windows(2).any(|w| w[1] - w[0] < clearance - 1e-6);
        if tight {
            out.insert(key, (os[0], *os.last().unwrap()));
        }
    }
    out
}

/// The row bands two chains share — both land on the row's side.
fn shared_bands(
    bands: &BTreeMap<(String, u8), (f64, f64)>,
    ca: &Chain,
    cb: &Chain,
) -> Vec<(bool, f64, f64)> {
    let mut out = Vec::new();
    for ea in &ca.ends {
        for eb in &cb.ends {
            if ea.path != eb.path || ea.side != eb.side {
                continue;
            }
            if let Some(&(lo, hi)) = bands.get(&(ea.path.clone(), ea.side.index())) {
                let vertical = matches!(ea.side, Side::Left | Side::Right);
                out.push((vertical, lo, hi));
            }
        }
    }
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out.dedup();
    out
}

/// Whether two segments' closest approach lies inside a row band: their
/// witness interval on the band's axis — the facing endpoints when their
/// extents are disjoint, the overlap when they run alongside — must sit
/// between the row's outermost ports. Links hugging beyond the band still
/// breach (LINKING Law 1, third surrender).
pub(super) fn band_contact(
    (vertical, lo, hi): (bool, f64, f64),
    sa: &[(f64, f64)],
    sb: &[(f64, f64)],
) -> bool {
    let range = |s: &[(f64, f64)]| {
        if vertical {
            (s[0].1.min(s[1].1), s[0].1.max(s[1].1))
        } else {
            (s[0].0.min(s[1].0), s[0].0.max(s[1].0))
        }
    };
    let (a, b) = (range(sa), range(sb));
    let (w0, w1) = if a.1 < b.0 {
        (a.1, b.0)
    } else if b.1 < a.0 {
        (b.1, a.0)
    } else {
        (a.0.max(b.0), a.1.min(b.1))
    };
    w0 >= lo - 1e-6 && w1 <= hi + 1e-6
}

/// The bundle a request rides in.
pub(super) fn bundle_of(router: &Router, m: usize) -> usize {
    router
        .bundles
        .iter()
        .position(|b| b.members.contains(&m))
        .expect("every chain belongs to a bundle")
}

/// Repaired insertion candidates for one starved bundle: [`nudged`] in
/// insertion mode proposes landings — against the incumbents' occupancy
/// first, then through cleared channels — and the separation audit repairs
/// each one (incumbents nudged off the conflict sites, port groups slid,
/// the hopeless undrawn). The caller judges them by ground truth.
fn insertions(
    router: &Router,
    raw: &[Option<Chain>],
    drawn: &[Option<Chain>],
    bi: usize,
    clearance: f64,
    slides: &Slides,
    target: (usize, usize, usize),
) -> Vec<Insertion> {
    let m0 = router.bundles[bi].members[0];
    let members = router.bundles[bi].members.clone();
    let forced = members.iter().any(|&m| {
        let req = &router.reqs[m];
        router.fans.of[m].is_empty() && req.side_a.is_some() && req.side_b.is_some()
    });
    let beat = drawn.iter().filter(|c| c.is_none()).count();
    let mut out = Vec::new();
    for cleared in [false, true] {
        let moves = nudged(
            router,
            raw,
            drawn,
            m0,
            Vec::new(),
            BTreeSet::new(),
            clearance,
            slides,
            cleared,
        );
        for (cand, cand_drawn) in moves {
            let (mut normal, mut normal_drawn) = (cand, cand_drawn);
            let mut cand_slides = slides.clone();
            let lost = separation_with_protect(
                router,
                &mut normal,
                &mut normal_drawn,
                clearance,
                &mut cand_slides,
                beat,
                if forced { &members } else { &[] },
            );
            let score = completeness_score(router, &normal_drawn, clearance);
            out.push((normal, normal_drawn, cand_slides, lost));
            if score < target {
                return out;
            }
        }
    }
    out
}

/// Undrawn edges first, then Law 1, then crossings — the lexicographic
/// ground truth the completeness pass descends.
fn completeness_score(
    router: &Router,
    drawn: &[Option<Chain>],
    clearance: f64,
) -> (usize, usize, usize) {
    let gone = drawn.iter().filter(|c| c.is_none()).count();
    let (conflicts, crossings) = law_score(router, drawn, clearance);
    (gone, conflicts, crossings)
}

/// What `commit_picked` does for fans, for an insertion accepted outside
/// the main routing loop: the first drawn sibling fixes the group's side.
fn adopt_fan_sides(router: &mut Router, raw: &[Option<Chain>]) {
    for end in raw.iter().flatten().flat_map(|chain| &chain.ends) {
        if let Some(g) = end.fan
            && router.fan_pick[g].is_none()
        {
            router.fan_pick[g] = Some(end.side);
        }
    }
}

/// Law 2 sanity for a slide candidate: every end still lands on its side,
/// perpendicular, clear of corners — a slide must never buy Law 1 with a
/// contact breach.
fn contact_holds(chains: &[Option<Chain>], clearance: f64) -> bool {
    chains.iter().flatten().all(|chain| {
        chain.ends.iter().all(|e| {
            let r = e.rect;
            let (along, lo, hi) = match e.side {
                Side::Left | Side::Right => (e.port.1, r.y0, r.y1),
                Side::Top | Side::Bottom => (e.port.0, r.x0, r.x1),
            };
            let margin = clearance.min((hi - lo) / 2.0);
            along >= lo + margin - 1e-6 && along <= hi - margin + 1e-6
        })
    })
}

/// The links worth moving to clear a conflicting pair: the pair itself
/// (later first), then every link sharing a port side with either — port
/// spread couples a side's slots, so a third link leaving the side can be
/// what frees the row.
fn movers(drawn: &[Option<Chain>], (a, b): (usize, usize)) -> Vec<usize> {
    let mut out = vec![b, a];
    let mut sides: Vec<(&str, Side)> = Vec::new();
    for &ci in &[a, b] {
        if let Some(ch) = &drawn[ci] {
            sides.extend(ch.ends.iter().map(|e| (e.path.as_str(), e.side)));
        }
    }
    for (ci, ch) in drawn.iter().enumerate() {
        let Some(ch) = ch else {
            continue;
        };
        if ci == a || ci == b {
            continue;
        }
        if ch
            .ends
            .iter()
            .any(|e| sides.contains(&(e.path.as_str(), e.side)))
        {
            out.push(ci);
        }
    }
    out
}

/// [`nudged`] for a conflicting link pair: the pair's mutual conflict
/// region is the wall, and the pair's other link seeds the partner set.
fn nudge(
    router: &Router,
    raw: &[Option<Chain>],
    drawn: &[Option<Chain>],
    mover: usize,
    pair: (usize, usize),
    clearance: f64,
    slides: &Slides,
) -> Vec<Move> {
    let mut walled: BTreeSet<usize> = BTreeSet::new();
    walled.insert(pair.0);
    walled.insert(pair.1);
    walled.remove(&mover);
    let mut deny = conflict_sites(drawn, pair.0, pair.1, clearance);
    deny.extend(conflict_sites(drawn, pair.1, pair.0, clearance));
    nudged(
        router, raw, drawn, mover, deny, walled, clearance, slides, false,
    )
}

/// Reroute candidates for one conflicting link: the given conflict region
/// is walled off, then refined with the partners each candidate still
/// conflicts with — the partner set only grows, so the loop is bounded.
/// Port spread can pull a re-landed approach back into the very conflict
/// the walls guard (provisional ports sit at side centres), so every round
/// also tries the route with the mover's current landing side excluded.
/// An **undrawn** mover turns this into insertion mode (the completeness
/// pass): the first landing is unconstrained, every next one walled away
/// from whatever the last still hit; `cleared` swaps the incumbents'
/// occupancy for empty channels when even relaxed margins find no way in.
/// Every candidate is returned; the caller judges them by ground truth.
#[allow(clippy::too_many_arguments)]
fn nudged(
    router: &Router,
    raw: &[Option<Chain>],
    drawn: &[Option<Chain>],
    mover: usize,
    mut deny: Vec<super::rect::Rect>,
    mut walled: BTreeSet<usize>,
    clearance: f64,
    slides: &Slides,
    cleared: bool,
) -> Vec<Move> {
    let req = &router.reqs[mover];
    let mut out = Vec::new();
    if req.a_path == req.b_path {
        return out; // self-loops are pinned shapes
    }
    if deny.is_empty() && drawn[mover].is_some() {
        return out; // nothing to wall a drawn mover away from
    }
    let bi = router
        .bundles
        .iter()
        .position(|b| b.members.contains(&mover))
        .expect("every chain belongs to a bundle");
    let members = &router.bundles[bi].members;
    let occ = if cleared {
        Occupancy::new(clearance)
    } else {
        occupancy_without(raw, members, clearance)
    };
    let ports = ports_without(raw, members, clearance);
    // A mover already landed on an over-asked side cannot re-enter it under
    // strict gating — without compact admission every repair of a
    // compacted-row link could only undraw it.
    let stuck = members
        .iter()
        .filter_map(|&m| raw[m].as_ref())
        .flat_map(|c| c.ends.iter())
        .any(|e| e.fan.is_none() && ports.free(&e.path, e.side, e.rect) == 0);
    let polys: Vec<Vec<(f64, f64)>> = (0..drawn.len())
        .filter(|w| !members.contains(w))
        .filter_map(|w| drawn[w].as_ref().map(geometry::polyline))
        .collect();
    let count = counter(&polys);
    let avoids = match drawn[mover].as_ref() {
        Some(cur) => {
            let free = |end: usize| {
                let forced = [req.side_a, req.side_b][end];
                let fanned = cur.ends[end].fan.is_some();
                (forced.is_none() && !fanned).then(|| cur.ends[end].side)
            };
            let mut avoids: Vec<[Option<Side>; 2]> = vec![[None, None]];
            if let Some(s) = free(0) {
                avoids.push([Some(s), None]);
            }
            if let Some(s) = free(1) {
                avoids.push([None, Some(s)]);
            }
            avoids
        }
        None => vec![[None, None]],
    };
    let mut seen: BTreeSet<Vec<(u64, u64)>> = BTreeSet::new();
    loop {
        let mut grew = false;
        for &avoid in &avoids {
            // Strict margins first; a mover walled out everywhere strict
            // gets the relaxed try (margins widened to the walls) the
            // rescue lever gets — ground truth judges either way.
            let Some(picked) = [false, true].into_iter().find_map(|relaxed| {
                router.route_bundle(bi, &occ, &ports, &count, &deny, avoid, relaxed, stuck, None)
            }) else {
                continue;
            };
            let mut cand = raw.to_vec();
            router.build_chains(bi, &picked, &mut cand);
            // A landing already proposed this call — new walls that didn't
            // bind — re-solves, re-scores, and re-walls identically: skip.
            let key: Vec<(u64, u64)> = members
                .iter()
                .flat_map(|&m| geometry::polyline(cand[m].as_ref().unwrap()))
                .map(|(x, y)| (x.to_bits(), y.to_bits()))
                .collect();
            if !seen.insert(key) {
                continue;
            }
            let cand_drawn = solve(&router.worlds, &cand, clearance, slides);
            for &(a, b) in &breaches(&cand_drawn, clearance) {
                let partner = match (a == mover, b == mover) {
                    (true, _) => b,
                    (_, true) => a,
                    _ => continue,
                };
                if walled.insert(partner) {
                    deny.extend(conflict_sites(&cand_drawn, mover, partner, clearance));
                    grew = true;
                }
            }
            out.push((cand, cand_drawn));
        }
        if !grew {
            return out;
        }
    }
}

/// Fan-sibling contact that is the shared trunk rather than a braid — the
/// validator's rule (`validate::trunk_contact`) mirrored onto polylines,
/// so the audit sees exactly the sibling breaches the checker will flag:
/// an outright overlap or touch, a segment lying on the partner's
/// polyline, or parallel extents that at most touch (staggered branch
/// points peeling off the trunk). Siblings running alongside each other
/// past the split still breach.
fn trunk_contact(
    sa: &[(f64, f64)],
    sb: &[(f64, f64)],
    pa: &[(f64, f64)],
    pb: &[(f64, f64)],
) -> bool {
    let collinear_within = |t: &[(f64, f64)], s: &[(f64, f64)]| {
        let (tx0, tx1) = (t[0].0.min(t[1].0), t[0].0.max(t[1].0));
        let (ty0, ty1) = (t[0].1.min(t[1].1), t[0].1.max(t[1].1));
        let (sx0, sx1) = (s[0].0.min(s[1].0), s[0].0.max(s[1].0));
        let (sy0, sy1) = (s[0].1.min(s[1].1), s[0].1.max(s[1].1));
        let along_x = ty0 == ty1 && sy0 == sy1 && (ty0 - sy0).abs() <= 1e-6;
        let along_y = tx0 == tx1 && sx0 == sx1 && (tx0 - sx0).abs() <= 1e-6;
        (along_x || along_y)
            && sx0 >= tx0 - 1e-6
            && sy0 >= ty0 - 1e-6
            && sx1 <= tx1 + 1e-6
            && sy1 <= ty1 + 1e-6
    };
    let on =
        |s: &[(f64, f64)], path: &[(f64, f64)]| path.windows(2).any(|t| collinear_within(t, s));
    let break_out = {
        let horizontal = |s: &[(f64, f64)]| s[0].1 == s[1].1;
        if horizontal(sa) != horizontal(sb) {
            false
        } else if horizontal(sa) {
            sa[0].0.max(sa[1].0).min(sb[0].0.max(sb[1].0))
                - sa[0].0.min(sa[1].0).max(sb[0].0.min(sb[1].0))
                <= 1e-6
        } else {
            sa[0].1.max(sa[1].1).min(sb[0].1.max(sb[1].1))
                - sa[0].1.min(sa[1].1).max(sb[0].1.min(sb[1].1))
                <= 1e-6
        }
    };
    seg_dist(sa, sb) <= 1e-6 || on(sb, pa) || on(sa, pb) || break_out
}

/// Axis-aligned segment distance — segments degenerate to boxes.
pub(super) fn seg_dist(sa: &[(f64, f64)], sb: &[(f64, f64)]) -> f64 {
    let (ax0, ax1) = (sa[0].0.min(sa[1].0), sa[0].0.max(sa[1].0));
    let (ay0, ay1) = (sa[0].1.min(sa[1].1), sa[0].1.max(sa[1].1));
    let (bx0, bx1) = (sb[0].0.min(sb[1].0), sb[0].0.max(sb[1].0));
    let (by0, by1) = (sb[0].1.min(sb[1].1), sb[0].1.max(sb[1].1));
    let dx = (bx0 - ax1).max(ax0 - bx1).max(0.0);
    let dy = (by0 - ay1).max(ay0 - by1).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

/// The kept link's segments that sit nearer than `clearance` to the mover,
/// inflated by `clearance` — the regions the mover's reroute must clear.
fn conflict_sites(
    drawn: &[Option<Chain>],
    mover: usize,
    kept: usize,
    clearance: f64,
) -> Vec<super::rect::Rect> {
    let (Some(m), Some(k)) = (&drawn[mover], &drawn[kept]) else {
        return Vec::new();
    };
    let (mp, kp) = (geometry::polyline(m), geometry::polyline(k));
    let mut out = Vec::new();
    for sk in kp.windows(2) {
        let near = mp
            .windows(2)
            .any(|sm| cross(sm, sk).is_none() && seg_dist(sm, sk) < clearance - 1e-6);
        if near {
            let rk = super::rect::Rect::new(
                sk[0].0.min(sk[1].0),
                sk[0].1.min(sk[1].1),
                sk[0].0.max(sk[1].0),
                sk[0].1.max(sk[1].1),
            );
            out.push(rk.inflate(clearance - 1e-6));
        }
    }
    out
}

/// One bundle's reroute candidates by **iterative obstacle refinement**:
/// start crossing-blind, then detour around whatever the candidate still
/// crosses — the obstacle set only grows, so the loop is bounded. Estimates
/// only generate candidates; the actual drawn count judges every one.
fn retry(
    router: &Router,
    raw: &[Option<Chain>],
    drawn: &[Option<Chain>],
    bi: usize,
    clearance: f64,
    slides: &Slides,
) -> Vec<Candidate> {
    let members = &router.bundles[bi].members;
    let occ = occupancy_without(raw, members, clearance);
    let ports = ports_without(raw, members, clearance);
    let mut out: Vec<Candidate> = Vec::new();
    // Route against one obstacle set; returns the links the candidate still
    // crosses (`None` when no route exists at all).
    let mut evaluate = |obstacle_links: &[usize]| -> Option<Vec<usize>> {
        let polys: Vec<Vec<(f64, f64)>> = obstacle_links
            .iter()
            .filter_map(|&w| drawn[w].as_ref().map(geometry::polyline))
            .collect();
        let count = counter(&polys);
        let picked = router.route_bundle(
            bi,
            &occ,
            &ports,
            &count,
            &[],
            [None, None],
            false,
            false,
            None,
        )?;
        let mut cand = raw.to_vec();
        router.build_chains(bi, &picked, &mut cand);
        let cand_drawn = solve(&router.worlds, &cand, clearance, slides);
        let crossings = collect(&cand_drawn);
        let mut still: Vec<usize> = crossings
            .iter()
            .filter_map(
                |c| match (members.contains(&c.pair.0), members.contains(&c.pair.1)) {
                    (true, false) => Some(c.pair.1),
                    (false, true) => Some(c.pair.0),
                    _ => None,
                },
            )
            .collect();
        still.sort_unstable();
        still.dedup();
        out.push((crossings.len(), cand, cand_drawn));
        Some(still)
    };

    // The most cautious candidate first — every other link an obstacle:
    // refinement converges on a small set and can miss the wide detour this
    // one proposes.
    let all: Vec<usize> = (0..drawn.len())
        .filter(|w| !members.contains(w) && drawn[*w].is_some())
        .collect();
    evaluate(&all);

    let mut obstacle_links: Vec<usize> = Vec::new();
    loop {
        let Some(still) = evaluate(&obstacle_links) else {
            return out;
        };
        let before = obstacle_links.len();
        obstacle_links.extend(still);
        obstacle_links.sort_unstable();
        obstacle_links.dedup();
        if obstacle_links.len() == before {
            return out;
        }
    }
}

/// Every bundle involved in one of `crossings`, most-crossed first (the most
/// entangled link has the most to gain from one detour), ties by declaration
/// (bundle index). Self-loops are pinned shapes and never reroute.
fn entangled_bundles(router: &Router, crossings: &[Crossing]) -> Vec<usize> {
    let mut counts: Vec<(usize, usize)> = Vec::new();
    for x in crossings {
        for link in [x.pair.1, x.pair.0] {
            let req = &router.reqs[link];
            if req.a_path == req.b_path {
                continue;
            }
            let bi = router
                .bundles
                .iter()
                .position(|b| b.members.contains(&link))
                .expect("every chain belongs to a bundle");
            match counts.iter_mut().find(|(b, _)| *b == bi) {
                Some((_, n)) => *n += 1,
                None => counts.push((bi, 1)),
            }
        }
    }
    counts.sort_by_key(|&(bi, n)| (usize::MAX - n, bi));
    counts.into_iter().map(|(bi, _)| bi).collect()
}

/// The crossing-cost callback over an obstacle set. Segments are flattened
/// once so each route edge checks only the perpendicular segment class.
fn counter(obstacles: &[Vec<(f64, f64)>]) -> impl Fn(super::rect::Rect, super::graph::Axis) -> u32 {
    let mut vertical = Vec::new();
    let mut horizontal = Vec::new();
    for s in obstacles.iter().flat_map(|p| p.windows(2)) {
        if s[0].0 == s[1].0 && s[0].1 != s[1].1 {
            vertical.push((s[0].0, s[0].1.min(s[1].1), s[0].1.max(s[1].1)));
        } else if s[0].1 == s[1].1 && s[0].0 != s[1].0 {
            horizontal.push((s[0].1, s[0].0.min(s[1].0), s[0].0.max(s[1].0)));
        }
    }
    move |band, axis| match axis {
        super::graph::Axis::H => vertical
            .iter()
            .filter(|&&(x, y0, y1)| x >= band.x0 && x <= band.x1 && y0 <= band.y1 && y1 >= band.y0)
            .count() as u32,
        super::graph::Axis::V => horizontal
            .iter()
            .filter(|&&(y, x0, x1)| y >= band.y0 && y <= band.y1 && x0 <= band.x1 && x1 >= band.x0)
            .count() as u32,
    }
}

/// Occupancy rebuilt from every chain except the bundle being rerouted — a
/// reroute must hold against everything currently drawn.
pub(super) fn occupancy_without(
    chains: &[Option<Chain>],
    skip: &[usize],
    clearance: f64,
) -> Occupancy {
    let mut occ = Occupancy::new(clearance);
    for (ci, chain) in chains.iter().enumerate() {
        if skip.contains(&ci) {
            continue;
        }
        if let Some(chain) = chain {
            occ.commit_chain(chain);
        }
    }
    occ
}

/// Port slots rebuilt the same way; a fan group's shared port counts once.
pub(super) fn ports_without(chains: &[Option<Chain>], skip: &[usize], clearance: f64) -> Ports {
    let mut ports = Ports::new(clearance);
    let mut fans_seen: BTreeSet<usize> = BTreeSet::new();
    for (ci, chain) in chains.iter().enumerate() {
        if skip.contains(&ci) {
            continue;
        }
        let Some(chain) = chain else {
            continue;
        };
        for e in &chain.ends {
            match e.fan {
                Some(g) => {
                    if fans_seen.insert(g) {
                        ports.commit(&e.path, e.side, 1);
                    }
                }
                None => ports.commit(&e.path, e.side, 1),
            }
        }
    }
    ports
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(a: (f64, f64), b: (f64, f64)) -> [(f64, f64); 2] {
        [a, b]
    }

    #[test]
    fn transversal_crossings_are_strict_interior_only() {
        let h = seg((0.0, 5.0), (10.0, 5.0));
        // A proper crossing, either argument order.
        let v = seg((4.0, 0.0), (4.0, 10.0));
        assert_eq!(cross(&h, &v), Some((4.0, 5.0)));
        assert_eq!(cross(&v, &h), Some((4.0, 5.0)));
        // A T-joint touches: the vertical ends exactly on the horizontal.
        assert_eq!(cross(&h, &seg((4.0, 5.0), (4.0, 10.0))), None);
        // An endpoint touch on the horizontal's tip.
        assert_eq!(cross(&h, &seg((10.0, 0.0), (10.0, 10.0))), None);
        // Parallel and collinear segments never cross (fan trunks).
        assert_eq!(cross(&h, &seg((0.0, 7.0), (10.0, 7.0))), None);
        assert_eq!(cross(&h, &seg((2.0, 5.0), (8.0, 5.0))), None);
    }
}

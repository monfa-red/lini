//! Weighted search over a world's channel graph (ROUTING.md model step 4).
//!
//! Multi-source, multi-target Dijkstra with the Law-3 scalar cost
//! `length + 2·clearance·turns + 4·clearance·crossings` — one formula, never
//! a lexicographic blend. Length is the L1 estimate through cell centres
//! (exact ordinates land in placement). States are (cell, travel direction),
//! so turns are counted honestly: a perpendicular step is one, a reversal —
//! doubling back along the same channel — is the two corners its drawn U
//! really has. Crossings are the ledger's committed perpendicular bands,
//! counted once per run over half-open travel intervals. A channel span
//! without the bundle's *k* tracks at minimum pitch is closed — capacity is
//! never exceeded, only priced. Routes run between the **entries**
//! ([`super::entry`]) of the two ends. Sides enter in the fixed rank
//! right → bottom → left → top; every tie breaks on discrete ids.

use super::cost::{cross_cost, min_pitch, turn_cost};
use super::entry::Entry;
use super::graph::{Axis, ChannelGraph};
use super::ledger::Ledger;
#[cfg(test)]
use super::rect::Rect;

/// Travel directions, indexed E, S, W, N — opposite = `(d + 2) % 4`.
pub(super) const DIRS: [(f64, f64); 4] = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)];

pub(super) fn opposite(dir: usize) -> usize {
    (dir + 2) % 4
}

fn axis_of(dir: usize) -> Axis {
    if dir.is_multiple_of(2) {
        Axis::H
    } else {
        Axis::V
    }
}

/// Corners drawn when travel changes from `a` to `b`: none straight on, one
/// for a perpendicular step, two for a reversal (the U's pair).
fn turn_count(a: usize, b: usize) -> u32 {
    if a == b {
        0
    } else if opposite(a) == b {
        2
    } else {
        1
    }
}

/// The chosen route: the cell path (a cell may repeat around a U-turn),
/// which start/goal entries it used, and its cost under the Law-3 formula.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Route {
    pub cells: Vec<usize>,
    pub start: usize,
    pub goal: usize,
    pub cost: f64,
}

/// Dijkstra state: one per (cell, travel direction).
fn state(cell: usize, dir: usize) -> usize {
    cell * 4 + dir
}

/// A channel span the caller has learned is unusable — a whole-run track
/// check failed there ([`crate::routing::ortho::route`]'s admission), so the
/// next search must route around it.
pub(crate) type Deny = (Axis, usize, (f64, f64));

/// The cheapest route from any start entry to any goal entry for a bundle of
/// `k`, under the committed state in `ledger` and the caller's learned
/// closures in `deny`. Ties break goal-side rank, then start-side rank
/// (entries come in side-rank order), then state id.
#[allow(clippy::too_many_arguments)]
pub(crate) fn cheapest(
    graph: &ChannelGraph,
    world: usize,
    starts: &[Entry],
    goals: &[Entry],
    ledger: &Ledger,
    deny: &[Deny],
    k: usize,
    clearance: f64,
) -> Option<Route> {
    use std::cmp::{Ordering, Reverse};
    use std::collections::BinaryHeap;

    let centre = |c: usize| {
        let r = graph.cells[c].rect;
        ((r.x0 + r.x1) / 2.0, (r.y0 + r.y1) / 2.0)
    };
    let l1 = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() + (a.1 - b.1).abs();
    let xc = cross_cost(clearance);
    let tc = turn_cost(clearance);
    // Crossings of an entry's stub piece (port → tip): the one stretch with
    // no freedom to dodge, charged when a committed rail's span overlaps the
    // window of ordinates the end run may take.
    let stub_xings = |e: &Entry| {
        let (a, b) = match e.axis {
            Axis::H => (e.port.0, e.tip.0),
            Axis::V => (e.port.1, e.tip.1),
        };
        ledger.crossings_overlapping(world, e.axis, (a.min(b), a.max(b)), e.window)
    };
    // Crossings of run travel along a channel: only the certain rails — span
    // covering the corridor's whole cross-section, every track the run could
    // take — are charged; a dodgeable rail costs nothing here and lands in
    // the report if the drawn wire does cross it.
    let edge_xings = |ax: Axis, chan: usize, travel: (f64, f64)| {
        let covered = graph.corridor(ax, chan, travel.0, travel.1).walls;
        ledger.crossings_covering(world, ax, travel, covered)
    };
    // A reversal's U-connector, estimated at the bounced cell's centre
    // ordinate across the cell's width: what the doubling-back wire must
    // cross to come back.
    let u_xings = |cell: usize, axis: Axis| {
        let r = graph.cells[cell].rect;
        let c = centre(cell);
        match axis {
            // Reversing V travel: the connector runs horizontally at the
            // bounce ordinate.
            Axis::V => ledger.crossings_overlapping(world, Axis::H, (r.x0, r.x1), (c.1, c.1)),
            Axis::H => ledger.crossings_overlapping(world, Axis::V, (r.y0, r.y1), (c.0, c.0)),
        }
    };
    // A channel span holds the bundle when it has k tracks left and the
    // caller hasn't learned otherwise (a denied span overlapping this one).
    let open = |ax: Axis, chan: usize, span: (f64, f64)| {
        deny.iter()
            .all(|d| d.0 != ax || d.1 != chan || d.2.0 >= span.1 || d.2.1 <= span.0)
            && ledger.tracks_left(world, ax, chan, span, graph) >= k
    };
    // The U-connector is a run in the bounce cell's crossing channel like
    // any other: it needs its own track over its whole stretch, or the
    // reversal is closed. `pin` is a leg's known ordinate — the port when
    // the doubled leg is an end run — widening the estimate from the bounce
    // cell's centre; an interior leg rests near the centre already.
    let u_open = |cell: usize, axis: Axis, pin: Option<f64>| {
        let (uax, uchan) = match axis {
            Axis::H => (Axis::V, graph.cells[cell].v),
            Axis::V => (Axis::H, graph.cells[cell].h),
        };
        let c = centre(cell);
        let uq = match uax {
            Axis::H => c.0,
            Axis::V => c.1,
        };
        let span = pin.map_or((uq, uq), |p| (p.min(uq), p.max(uq)));
        open(uax, uchan, span)
    };
    // An entry's port ordinate across its travel axis.
    let entry_pin = |e: &Entry| match e.axis {
        Axis::H => e.port.1,
        Axis::V => e.port.0,
    };
    let along = |c: usize, axis: Axis| {
        let p = centre(c);
        match axis {
            Axis::H => p.0,
            Axis::V => p.1,
        }
    };

    // (cost, turns, origin start, predecessor state) per state.
    type Best = (f64, u32, usize, Option<usize>);
    let mut best: Vec<Option<Best>> = vec![None; graph.cells.len() * 4];

    #[derive(PartialEq)]
    struct Item {
        cost: f64,
        state: usize,
        origin: usize,
    }
    impl Eq for Item {}
    impl Ord for Item {
        fn cmp(&self, other: &Self) -> Ordering {
            self.cost
                .total_cmp(&other.cost)
                .then(self.state.cmp(&other.state))
                .then(self.origin.cmp(&other.origin))
        }
    }
    impl PartialOrd for Item {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    let mut heap: BinaryHeap<Reverse<Item>> = BinaryHeap::new();
    let push = |best: &mut Vec<Option<Best>>,
                heap: &mut BinaryHeap<Reverse<Item>>,
                st: usize,
                cost: f64,
                turns: u32,
                origin: usize,
                prev: Option<usize>| {
        if best[st].is_none_or(|(c, ..)| cost.total_cmp(&c) == Ordering::Less) {
            best[st] = Some((cost, turns, origin, prev));
            heap.push(Reverse(Item {
                cost,
                state: st,
                origin,
            }));
        }
    };

    for (si, e) in starts.iter().enumerate() {
        let cost = l1(e.tip, centre(e.cell)) + f64::from(stub_xings(e)) * xc;
        push(
            &mut best,
            &mut heap,
            state(e.cell, e.dir),
            cost,
            0,
            si,
            None,
        );
    }

    while let Some(Reverse(item)) = heap.pop() {
        let Some((cost, turns, origin, _)) = best[item.state] else {
            continue;
        };
        if cost.total_cmp(&item.cost) != Ordering::Equal || origin != item.origin {
            continue; // stale
        }
        let (cell, dir) = (item.state / 4, item.state % 4);
        for &(next, ax, chan) in &graph.adj[cell] {
            let (qa, qb) = (along(cell, ax), along(next, ax));
            let span = (qa.min(qb), qa.max(qb));
            if !open(ax, chan, span) {
                continue;
            }
            let (ca, cb) = (centre(cell), centre(next));
            let edge_dir = match ax {
                Axis::H => {
                    if cb.0 > ca.0 {
                        0
                    } else {
                        2
                    }
                }
                Axis::V => {
                    if cb.1 > ca.1 {
                        1
                    } else {
                        3
                    }
                }
            };
            let turn = turn_count(dir, edge_dir);
            let mut ncost = cost
                + l1(ca, cb)
                + f64::from(turn) * tc
                + f64::from(edge_xings(ax, chan, span)) * xc;
            if turn == 2 {
                // Doubling back: the U's connector needs a track of its own
                // and crosses whatever covers the bounce cell. On the start's
                // own end run the doubled leg sits at the port, not the cell
                // centre.
                let pin = (turns == 0).then(|| entry_pin(&starts[origin]));
                if !u_open(cell, ax, pin) {
                    continue;
                }
                ncost += f64::from(u_xings(cell, ax)) * xc;
            }
            push(
                &mut best,
                &mut heap,
                state(next, edge_dir),
                ncost,
                turns + turn,
                origin,
                Some(item.state),
            );
        }
    }

    // Windows meet on k tracks ⇒ a single-run route draws straight; else it
    // jogs once (ROUTING.md model step 4) — two turns, and the jog is a run
    // in a crossing channel with capacity and crossings like any other.
    let fits = |a: (f64, f64), b: (f64, f64)| {
        let shared = (a.0.max(b.0), a.1.min(b.1));
        shared.0 <= shared.1
            && ((shared.1 - shared.0) / min_pitch(clearance)).floor() as usize + 1 >= k
    };
    let jog_span = |a: (f64, f64), b: (f64, f64)| {
        let (lo, hi) = (a.1.min(b.1), a.0.max(b.0));
        (lo.min(hi), hi.max(lo))
    };
    let path_cells = |mut st: usize| {
        let mut cells = vec![st / 4];
        while let Some((_, _, _, Some(prev))) = best[st] {
            st = prev;
            cells.push(st / 4);
        }
        cells.reverse();
        cells.dedup();
        cells
    };
    // The first traversed cell whose crossing channel holds the jog — the
    // deterministic estimate; placement picks the drawn spot.
    let jog = |cells: &[usize], axis: Axis, span: (f64, f64)| {
        cells.iter().find_map(|&c| {
            let (jog_axis, chan) = match axis {
                Axis::H => (Axis::V, graph.cells[c].v),
                Axis::V => (Axis::H, graph.cells[c].h),
            };
            open(jog_axis, chan, span).then(|| edge_xings(jog_axis, chan, span))
        })
    };

    // Best goal over (cost, goal rank, start rank).
    let mut winner: Option<(f64, usize, usize, usize)> = None; // (cost, gi, si, state)
    for (gi, g) in goals.iter().enumerate() {
        let goal_dir = opposite(g.dir);
        for dir in 0..4 {
            let st = state(g.cell, dir);
            let Some((c, turns, si, _)) = best[st] else {
                continue;
            };
            let goal_turn = turn_count(dir, goal_dir);
            let mut total = c
                + l1(centre(g.cell), g.tip)
                + f64::from(goal_turn) * tc
                + f64::from(stub_xings(g)) * xc;
            if goal_turn == 2 {
                if !u_open(g.cell, axis_of(dir), Some(entry_pin(g))) {
                    continue;
                }
                total += f64::from(u_xings(g.cell, axis_of(dir))) * xc;
            }
            // A single straight run claimed by both ends: its own channel
            // must hold the bundle over the whole travel (no edge relaxation
            // ever checked it), then charge the certain rails over the shared
            // window — or the jog when the windows can't hold k together.
            if turns == 0 && goal_turn == 0 && starts[si].axis == g.axis {
                let (wa, wg) = (starts[si].window, g.window);
                let (ta, tg) = match g.axis {
                    Axis::H => (starts[si].tip.0, g.tip.0),
                    Axis::V => (starts[si].tip.1, g.tip.1),
                };
                let travel = (ta.min(tg), ta.max(tg));
                let chan = match g.axis {
                    Axis::H => graph.cells[g.cell].h,
                    Axis::V => graph.cells[g.cell].v,
                };
                if !open(g.axis, chan, travel) {
                    continue;
                }
                if fits(wa, wg) {
                    let shared = (wa.0.max(wg.0), wa.1.min(wg.1));
                    total +=
                        f64::from(ledger.crossings_covering(world, g.axis, travel, shared)) * xc;
                } else {
                    let span = jog_span(wa, wg);
                    let Some(jog_xings) = jog(&path_cells(st), g.axis, span) else {
                        continue; // no crossing channel holds the jog
                    };
                    total += 2.0 * tc + f64::from(jog_xings) * xc;
                }
            }
            let better = match &winner {
                None => true,
                Some((wc, wgi, wsi, wst)) => match total.total_cmp(wc) {
                    Ordering::Less => true,
                    Ordering::Greater => false,
                    Ordering::Equal => (gi, si, st) < (*wgi, *wsi, *wst),
                },
            };
            if better {
                winner = Some((total, gi, si, st));
            }
        }
    }
    let (cost, gi, si, st) = winner?;
    Some(Route {
        cells: path_cells(st),
        start: si,
        goal: gi,
        cost,
    })
}

#[cfg(test)]
mod tests {
    use super::super::entry::entries;
    use super::*;
    use crate::ast::Side;

    const BOUNDS: Rect = Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 200.0,
        y1: 100.0,
    };
    const C: f64 = 8.0;

    fn body(x0: f64, y0: f64, x1: f64, y1: f64) -> Rect {
        Rect::new(x0, y0, x1, y1)
    }

    /// Two nodes facing each other across open space, centres aligned.
    fn facing() -> (ChannelGraph, Rect, Rect) {
        let a = body(20.0, 40.0, 40.0, 60.0);
        let b = body(160.0, 40.0, 180.0, 60.0);
        let g = ChannelGraph::build(BOUNDS, &[a.inflate(C), b.inflate(C)], false);
        (g, a, b)
    }

    fn route(
        g: &ChannelGraph,
        a: Rect,
        b: Rect,
        ledger: &Ledger,
        k: usize,
        forced: (Option<Side>, Option<Side>),
    ) -> Option<Route> {
        let starts = entries(g, a, C, C, forced.0, &[], false);
        let goals = entries(g, b, C, C, forced.1, &[], false);
        cheapest(g, 0, &starts, &goals, ledger, &[], k, C)
    }

    #[test]
    fn diagonal_neighbours_connect_with_one_l_turn() {
        let a = body(20.0, 10.0, 40.0, 30.0);
        let b = body(160.0, 70.0, 180.0, 90.0);
        let g = ChannelGraph::build(BOUNDS, &[a.inflate(C), b.inflate(C)], false);
        let ledger = Ledger::new(C);
        let r = route(&g, a, b, &ledger, 1, (None, None)).expect("route");
        let starts = entries(&g, a, C, C, None, &[], false);
        let goals = entries(&g, b, C, C, None, &[], false);
        let picked = (starts[r.start].side, goals[r.goal].side);
        assert!(
            picked == (Side::Right, Side::Top) || picked == (Side::Bottom, Side::Left),
            "an L between facing quadrants: {picked:?}"
        );
    }

    #[test]
    fn facing_nodes_connect_straight_via_their_facing_sides() {
        let (g, a, b) = facing();
        let ledger = Ledger::new(C);
        let r = route(&g, a, b, &ledger, 1, (None, None)).expect("route");
        let starts = entries(&g, a, C, C, None, &[], false);
        let goals = entries(&g, b, C, C, None, &[], false);
        assert_eq!(starts[r.start].side, Side::Right);
        assert_eq!(goals[r.goal].side, Side::Left);
        // Aligned windows: pure length, no turn or crossing surcharge.
        assert_eq!(r.cost, 104.0);
        assert_eq!(r.cells.len(), 1);
    }

    #[test]
    fn misaligned_windows_in_one_cell_cost_the_jog() {
        // Offset just past window overlap, both punches into the same cell.
        let a = body(20.0, 40.0, 40.0, 60.0); // right window (48, 52)
        let b = body(160.0, 46.0, 180.0, 66.0); // left window (54, 58)
        let g = ChannelGraph::build(BOUNDS, &[a.inflate(C), b.inflate(C)], false);
        let ledger = Ledger::new(C);
        let r = route(&g, a, b, &ledger, 1, (None, None)).expect("route");
        // L1 through the cell centre (55 + 55) plus two jog turns (2 × 16).
        assert_eq!(r.cost, 110.0 + 2.0 * turn_cost(C));
    }

    #[test]
    fn a_jog_with_no_free_crossing_channel_fails_over() {
        let a = body(20.0, 40.0, 40.0, 60.0);
        let b = body(160.0, 46.0, 180.0, 66.0);
        let g = ChannelGraph::build(BOUNDS, &[a.inflate(C), b.inflate(C)], false);
        // Fill the one V-channel the jog could use to capacity.
        let vchan =
            g.v.iter()
                .position(|c| c.rect == Rect::new(48.0, 0.0, 152.0, 100.0))
                .expect("middle V-channel");
        let mut ledger = Ledger::new(C);
        // Past capacity for every sub-span (narrow spans dodge soft-wall
        // margins, so their capacity runs higher than the full span's).
        let cap = ledger.tracks_left(0, Axis::V, vchan, (0.0, 100.0), &g);
        ledger.commit_run(0, Axis::V, vchan, (0.0, 100.0), cap + 8, &g);
        // Forced onto the facing sides, the jog has nowhere to run.
        assert_eq!(
            route(&g, a, b, &ledger, 1, (Some(Side::Right), Some(Side::Left))),
            None
        );
    }

    #[test]
    fn crossing_beats_the_long_way_and_yields_to_the_bundle() {
        let (g, a, b) = facing();
        let vchan =
            g.v.iter()
                .position(|c| c.rect == Rect::new(48.0, 0.0, 152.0, 100.0))
                .expect("middle V-channel");
        // One committed rail across the corridor, sparing the low road.
        let mut one = Ledger::new(C);
        one.commit_run(0, Axis::V, vchan, (10.0, 70.0), 1, &g);
        let r = route(&g, a, b, &one, 1, (None, None)).expect("route");
        assert_eq!(r.cells.len(), 1, "one crossing beats any detour: {r:?}");
        assert_eq!(r.cost, 104.0 + cross_cost(C));
        // Eight committed rails: crossing costs 8× — the U-detour under
        // their span end is now cheaper.
        let mut eight = Ledger::new(C);
        eight.commit_run(0, Axis::V, vchan, (10.0, 70.0), 8, &g);
        let r = route(&g, a, b, &eight, 1, (None, None)).expect("route");
        assert!(
            r.cells.len() > 1,
            "eight crossings lose to the detour: {r:?}"
        );
        assert!(r.cost < 104.0 + 8.0 * cross_cost(C));
    }

    #[test]
    fn closed_channel_forces_the_detour() {
        // The facing row runs between two blocks — a walled corridor, its
        // own void. Closing it with committed load forces the route down
        // and around, through the separate passage under the lower block.
        let a = body(20.0, 40.0, 40.0, 60.0);
        let b = body(160.0, 40.0, 180.0, 60.0);
        let above = Rect::new(60.0, 10.0, 140.0, 32.0);
        let below = Rect::new(60.0, 68.0, 140.0, 90.0);
        let g = ChannelGraph::build(BOUNDS, &[a.inflate(C), b.inflate(C), above, below], false);
        let ledger = Ledger::new(C);
        let direct = route(&g, a, b, &ledger, 1, (None, None)).expect("route");
        let row =
            g.h.iter()
                .position(|c| c.rect == Rect::new(48.0, 32.0, 152.0, 68.0))
                .expect("row channel");
        let mut full = Ledger::new(C);
        let cap = full.tracks_left(0, Axis::H, row, (48.0, 152.0), &g);
        full.commit_run(0, Axis::H, row, (48.0, 152.0), cap + 8, &g);
        let detour = route(&g, a, b, &full, 1, (None, None)).expect("route");
        assert!(
            detour.cells.len() > direct.cells.len(),
            "detour {detour:?} vs direct {direct:?}"
        );
    }

    #[test]
    fn a_bundle_needs_k_tracks_or_detours() {
        // Squeeze the corridor between two blocks so it holds few tracks.
        let a = body(20.0, 40.0, 40.0, 60.0);
        let b = body(160.0, 40.0, 180.0, 60.0);
        let above = Rect::new(60.0, 0.0, 140.0, 36.0);
        let below = Rect::new(60.0, 64.0, 140.0, 100.0);
        let g = ChannelGraph::build(BOUNDS, &[a.inflate(C), b.inflate(C), above, below], false);
        // The pinch: y 36..64 between the blocks, wall to wall — 28 usable
        // → floor(28/4)+1 = 8 tracks at min pitch. The blocks reach the
        // canvas bounds, so an over-wide bundle has no way around either.
        let ledger = Ledger::new(C);
        let eight = route(&g, a, b, &ledger, 8, (None, None)).expect("route");
        assert_eq!(eight.cells.len(), 3, "the pinch holds eight: {eight:?}");
        assert_eq!(route(&g, a, b, &ledger, 9, (None, None)), None);
    }

    #[test]
    fn cheapest_is_deterministic() {
        let (g, a, b) = facing();
        let mut ledger = Ledger::new(C);
        let vchan =
            g.v.iter()
                .position(|c| c.rect == Rect::new(48.0, 0.0, 152.0, 100.0))
                .expect("middle V-channel");
        ledger.commit_run(0, Axis::V, vchan, (10.0, 70.0), 8, &g);
        let first = route(&g, a, b, &ledger, 1, (None, None));
        for _ in 0..100 {
            assert_eq!(route(&g, a, b, &ledger, 1, (None, None)), first);
        }
    }
}

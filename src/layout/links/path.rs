//! Deterministic shortest path over a world's channel graph.
//!
//! Multi-source, multi-target Dijkstra with the lexicographic cost
//! `(crossings, length, turns)` — never a weighted blend. Crossings are
//! supplied by the caller as a segment-counting callback: zero in the
//! initial pass (ROUTING §Model step 4 routes by `(length, turns)`), the
//! transversal count against every drawn link in the audit's retries
//! (§Model step 6 — a link always detours rather than crosses). Lengths are
//! L1 distances through cell centres (exact ordinates land in the runs
//! stage); every tie breaks on discrete ids. Entries are **punch stubs**: a
//! straight perpendicular run from the side's port through any transparent
//! ancestor walls into the first world cell — blocked by any solid keep-out
//! on the way. A capacity-closed channel interval is skipped (closure is
//! binary; lanes are never squeezed). Sides enter in the fixed rank
//! right → bottom → left → top.

use super::graph::{Axis, ChannelGraph};
use super::rect::Rect;
use crate::ast::Side;

/// One way into the graph: a side's port on the node boundary and the stub
/// tip where the link reaches the world's free space.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub side: Side,
    pub port: (f64, f64),
    pub tip: (f64, f64),
    pub axis: Axis,
    pub cell: usize,
}

/// The chosen route: the cell path plus which start/goal entries it used.
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub cells: Vec<usize>,
    pub start: usize,
    pub goal: usize,
}

/// The graph entries of a node — one per side whose punch reaches a world
/// cell without crossing a blocker. `forced` prunes to that side; `inward`
/// flips the punch into the body (containment ends).
pub fn entries(
    graph: &ChannelGraph,
    body: Rect,
    stub: f64,
    forced: Option<Side>,
    blockers: &[Rect],
    inward: bool,
) -> Vec<Entry> {
    let cx = (body.x0 + body.x1) / 2.0;
    let cy = (body.y0 + body.y1) / 2.0;
    let candidates = [
        (Side::Right, (body.x1, cy), (1.0, 0.0), Axis::H),
        (Side::Bottom, (cx, body.y1), (0.0, 1.0), Axis::V),
        (Side::Left, (body.x0, cy), (-1.0, 0.0), Axis::H),
        (Side::Top, (cx, body.y0), (0.0, -1.0), Axis::V),
    ];
    candidates
        .into_iter()
        .filter(|(s, ..)| forced.is_none_or(|f| f == *s))
        .filter_map(|(side, port, dir, axis)| {
            let dir = if inward { (-dir.0, -dir.1) } else { dir };
            punch(graph, port, dir, stub, blockers).map(|(tip, cell)| Entry {
                side,
                port,
                tip,
                axis,
                cell,
            })
        })
        .collect()
}

/// March from `port` along `dir` to the nearest reachable point inside a
/// world cell: at least `stub` out when the cell allows, clamped into the
/// cell otherwise, and never across a blocker.
fn punch(
    graph: &ChannelGraph,
    port: (f64, f64),
    dir: (f64, f64),
    stub: f64,
    blockers: &[Rect],
) -> Option<((f64, f64), usize)> {
    let mut hits: Vec<(f64, f64, usize)> = Vec::new();
    for (i, c) in graph.cells.iter().enumerate() {
        let r = c.rect;
        let (near, far) = if dir.0 != 0.0 {
            if port.1 < r.y0 || port.1 > r.y1 {
                continue;
            }
            ((r.x0 - port.0) * dir.0, (r.x1 - port.0) * dir.0)
        } else {
            if port.0 < r.x0 || port.0 > r.x1 {
                continue;
            }
            ((r.y0 - port.1) * dir.1, (r.y1 - port.1) * dir.1)
        };
        let (near, far) = (near.min(far), near.max(far));
        if far <= 0.0 {
            continue;
        }
        hits.push((near.max(0.0), far, i));
    }
    hits.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.2.cmp(&b.2)));
    for (near, far, cell) in hits {
        let t = stub.clamp(near, far);
        if t <= 0.0 {
            continue;
        }
        let tip = (port.0 + dir.0 * t, port.1 + dir.1 * t);
        let clear = blockers.iter().all(|b| {
            let (x0, x1) = (port.0.min(tip.0), port.0.max(tip.0));
            let (y0, y1) = (port.1.min(tip.1), port.1.max(tip.1));
            !(x0 < b.x1 && x1 > b.x0 && y0 < b.y1 && y1 > b.y0)
        });
        return clear.then_some((tip, cell));
    }
    None
}

/// Dijkstra state: one per (cell, arrival axis).
fn state(cell: usize, axis: Axis) -> usize {
    cell * 2
        + match axis {
            Axis::H => 0,
            Axis::V => 1,
        }
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct Cost {
    xings: u32,
    len: f64,
    turns: u32,
}

impl Cost {
    fn cmp(&self, other: &Cost) -> std::cmp::Ordering {
        self.xings
            .cmp(&other.xings)
            .then(self.len.total_cmp(&other.len))
            .then(self.turns.cmp(&other.turns))
    }
}

/// The crossing callback: how many drawn links intrude transversally into a
/// corridor band swept along `axis`. The band is the whole channel cross-
/// section, not one lane — ordinates are assigned later, so any link cutting
/// into the band may have to be crossed.
pub type CrossCount<'a> = &'a dyn Fn(Rect, Axis) -> u32;

/// A crossing-blind [`CrossCount`] — the initial pass's cost.
pub const FREE: CrossCount<'static> = &|_, _| 0;

/// The cheapest `(crossings, length, turns)` route from any start entry to
/// any goal entry, skipping channel intervals where `closed` says capacity
/// binds; ties break goal-side rank, then start-side rank.
pub fn shortest(
    graph: &ChannelGraph,
    starts: &[Entry],
    goals: &[Entry],
    closed: &dyn Fn(Axis, usize, f64, f64) -> bool,
    cross: CrossCount,
) -> Option<Route> {
    use std::cmp::{Ordering, Reverse};
    use std::collections::BinaryHeap;

    let centre = |c: usize| {
        let r = graph.cells[c].rect;
        ((r.x0 + r.x1) / 2.0, (r.y0 + r.y1) / 2.0)
    };
    let l1 = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() + (a.1 - b.1).abs();
    // A thin band along one segment (entry stubs and approaches).
    let seg_band = |a: (f64, f64), b: (f64, f64)| {
        let axis = if a.1 == b.1 { Axis::H } else { Axis::V };
        let r = Rect::new(a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1));
        cross(r, axis)
    };
    // Crossings over the L between a stub tip and its cell centre, bending
    // off the stub's travel axis.
    let cross_l = |a: (f64, f64), b: (f64, f64), axis: Axis| {
        let mid = match axis {
            Axis::H => (b.0, a.1),
            Axis::V => (a.0, b.1),
        };
        seg_band(a, mid) + seg_band(mid, b)
    };
    // The full corridor band an edge sweeps between two cell centres.
    let edge_band = |a: usize, b: usize, axis: Axis, chan: usize| {
        let (ca, cb) = (centre(a), centre(b));
        let r = match axis {
            Axis::H => {
                let ch = graph.h[chan].rect;
                Rect::new(ca.0.min(cb.0), ch.y0, ca.0.max(cb.0), ch.y1)
            }
            Axis::V => {
                let ch = graph.v[chan].rect;
                Rect::new(ch.x0, ca.1.min(cb.1), ch.x1, ca.1.max(cb.1))
            }
        };
        cross(r, axis)
    };
    let along = |c: usize, axis: Axis| {
        let p = centre(c);
        match axis {
            Axis::H => p.0,
            Axis::V => p.1,
        }
    };

    // (cost, origin start, predecessor state) per state, settled by Dijkstra.
    let mut best: Vec<Option<(Cost, usize, Option<usize>)>> = vec![None; graph.cells.len() * 2];

    #[derive(PartialEq)]
    struct Item {
        cost: Cost,
        state: usize,
        origin: usize,
    }
    impl Eq for Item {}
    impl Ord for Item {
        fn cmp(&self, other: &Self) -> Ordering {
            self.cost
                .cmp(&other.cost)
                .then(self.state.cmp(&other.state))
                .then(self.origin.cmp(&other.origin))
        }
    }
    impl PartialOrd for Item {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    let mut heap = BinaryHeap::new();
    let push = |best: &mut Vec<Option<(Cost, usize, Option<usize>)>>,
                heap: &mut BinaryHeap<Reverse<Item>>,
                st: usize,
                cost: Cost,
                origin: usize,
                prev: Option<usize>| {
        if best[st].is_none_or(|(c, ..)| cost.cmp(&c) == Ordering::Less) {
            best[st] = Some((cost, origin, prev));
            heap.push(Reverse(Item {
                cost,
                state: st,
                origin,
            }));
        }
    };

    for (si, e) in starts.iter().enumerate() {
        let cost = Cost {
            xings: seg_band(e.port, e.tip) + cross_l(e.tip, centre(e.cell), e.axis),
            len: l1(e.tip, centre(e.cell)),
            turns: 0,
        };
        push(&mut best, &mut heap, state(e.cell, e.axis), cost, si, None);
    }

    while let Some(Reverse(item)) = heap.pop() {
        let Some((cost, origin, _)) = best[item.state] else {
            continue;
        };
        if cost.cmp(&item.cost) != Ordering::Equal || origin != item.origin {
            continue; // stale
        }
        let (cell, axis) = (item.state / 2, [Axis::H, Axis::V][item.state % 2]);
        for &(next, ax, chan) in &graph.adj[cell] {
            let (qa, qb) = (along(cell, ax), along(next, ax));
            if closed(ax, chan, qa.min(qb), qa.max(qb)) {
                continue;
            }
            let ncost = Cost {
                xings: cost.xings + edge_band(cell, next, ax, chan),
                len: cost.len + l1(centre(cell), centre(next)),
                turns: cost.turns + u32::from(ax != axis),
            };
            push(
                &mut best,
                &mut heap,
                state(next, ax),
                ncost,
                origin,
                Some(item.state),
            );
        }
    }

    // Best goal over (cost, goal rank, start rank).
    let mut winner: Option<(Cost, usize, usize, usize)> = None; // (cost, gi, si, state)
    for (gi, g) in goals.iter().enumerate() {
        for axis in [Axis::H, Axis::V] {
            let st = state(g.cell, axis);
            let Some((c, si, prev)) = best[st] else {
                continue;
            };
            // A single-cell straight shot draws approach – jog – approach
            // (`geometry::chain`): the jog is a real run in the cell's
            // perpendicular channel and must respect capacity like any hop.
            if prev.is_none() && axis == g.axis && starts[si].axis == g.axis {
                let (jog_axis, chan) = match g.axis {
                    Axis::H => (Axis::V, graph.cells[g.cell].v),
                    Axis::V => (Axis::H, graph.cells[g.cell].h),
                };
                let ord = |e: &Entry| match e.axis {
                    Axis::H => e.port.1,
                    Axis::V => e.port.0,
                };
                let (a, b) = (ord(&starts[si]), ord(g));
                if closed(jog_axis, chan, a.min(b), a.max(b)) {
                    continue;
                }
            }
            let total = Cost {
                xings: c.xings + cross_l(centre(g.cell), g.tip, axis) + seg_band(g.tip, g.port),
                len: c.len + l1(centre(g.cell), g.tip),
                turns: c.turns + u32::from(axis != g.axis),
            };
            let better = match &winner {
                None => true,
                Some((wc, wgi, wsi, _)) => match total.cmp(wc) {
                    Ordering::Less => true,
                    Ordering::Greater => false,
                    Ordering::Equal => (gi, si) < (*wgi, *wsi),
                },
            };
            if better {
                winner = Some((total, gi, si, st));
            }
        }
    }
    let (_, gi, si, mut st) = winner?;

    let mut cells = vec![st / 2];
    while let Some((_, _, Some(prev))) = best[st] {
        st = prev;
        cells.push(st / 2);
    }
    cells.reverse();
    cells.dedup();
    Some(Route {
        cells,
        start: si,
        goal: gi,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::links::graph::ChannelGraph;

    const BOUNDS: Rect = Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 200.0,
        y1: 100.0,
    };

    const OPEN: &dyn Fn(Axis, usize, f64, f64) -> bool = &|_, _, _, _| false;

    fn body(x0: f64, y0: f64, x1: f64, y1: f64) -> Rect {
        Rect::new(x0, y0, x1, y1)
    }

    /// Two nodes facing each other across open space.
    fn facing() -> (ChannelGraph, Rect, Rect) {
        let a = body(20.0, 40.0, 40.0, 60.0);
        let b = body(160.0, 40.0, 180.0, 60.0);
        let g = ChannelGraph::build(BOUNDS, &[a.inflate(8.0), b.inflate(8.0)], false);
        (g, a, b)
    }

    #[test]
    fn entries_offer_each_clear_side_in_rank_order() {
        let (g, a, _) = facing();
        let es = entries(&g, a, 8.0, None, &[], false);
        let sides: Vec<Side> = es.iter().map(|e| e.side).collect();
        assert_eq!(sides, [Side::Right, Side::Bottom, Side::Left, Side::Top]);
        for e in &es {
            let c = g.cells[e.cell].rect;
            assert!(
                e.tip.0 >= c.x0 && e.tip.0 <= c.x1 && e.tip.1 >= c.y0 && e.tip.1 <= c.y1,
                "tip {:?} not in its cell {c:?}",
                e.tip
            );
        }
        // Right-side port sits mid-side on the body boundary, tip one stub out.
        assert_eq!(es[0].port, (40.0, 50.0));
        assert_eq!(es[0].tip, (48.0, 50.0));
    }

    #[test]
    fn walled_off_sides_are_dropped() {
        let a = body(20.0, 40.0, 40.0, 60.0);
        let wall = Rect::new(0.0, 0.0, 12.0, 100.0); // flush against a's left keep-out
        let g = ChannelGraph::build(BOUNDS, &[a.inflate(8.0), wall], false);
        let es = entries(&g, a, 8.0, None, &[wall], false);
        assert!(es.iter().all(|e| e.side != Side::Left));
        assert_eq!(es.len(), 3);
    }

    #[test]
    fn forced_side_prunes_to_one_entry() {
        let (g, a, _) = facing();
        let es = entries(&g, a, 8.0, Some(Side::Top), &[], false);
        assert_eq!(es.len(), 1);
        assert_eq!(es[0].side, Side::Top);
    }

    #[test]
    fn punch_crosses_a_transparent_wall_to_the_world_cell() {
        // A group at x ∈ [60, 120] holds the endpoint; the world sees the
        // group as one keep-out, so the first cell starts at 128.
        let group = Rect::new(60.0, 20.0, 120.0, 80.0);
        let g = ChannelGraph::build(BOUNDS, &[group.inflate(8.0)], false);
        let inner = body(70.0, 40.0, 90.0, 60.0);
        let es = entries(&g, inner, 8.0, Some(Side::Right), &[], false);
        assert_eq!(es.len(), 1);
        assert_eq!(es[0].port, (90.0, 50.0));
        assert_eq!(es[0].tip, (128.0, 50.0));
    }

    #[test]
    fn punch_is_blocked_by_a_sibling_keepout() {
        let group = Rect::new(60.0, 20.0, 120.0, 80.0);
        let g = ChannelGraph::build(BOUNDS, &[group.inflate(8.0)], false);
        let inner = body(70.0, 40.0, 90.0, 60.0);
        let sibling = Rect::new(95.0, 30.0, 115.0, 70.0);
        let es = entries(&g, inner, 8.0, Some(Side::Right), &[sibling], false);
        assert!(es.is_empty());
    }

    #[test]
    fn inner_entries_point_into_the_body() {
        let parent = body(40.0, 20.0, 160.0, 80.0);
        let g = ChannelGraph::build(parent, &[Rect::new(90.0, 45.0, 110.0, 55.0)], false);
        let es = entries(&g, parent, 8.0, None, &[], true);
        let right = es.iter().find(|e| e.side == Side::Right).expect("right");
        assert_eq!(right.port, (160.0, 50.0));
        assert_eq!(right.tip, (152.0, 50.0));
    }

    #[test]
    fn facing_nodes_connect_via_their_facing_sides() {
        let (g, a, b) = facing();
        let starts = entries(&g, a, 8.0, None, &[], false);
        let goals = entries(&g, b, 8.0, None, &[], false);
        let r = shortest(&g, &starts, &goals, OPEN, FREE).expect("route");
        assert_eq!(starts[r.start].side, Side::Right);
        assert_eq!(goals[r.goal].side, Side::Left);
    }

    #[test]
    fn forced_far_side_still_routes_the_long_way() {
        let (g, a, b) = facing();
        let starts = entries(&g, a, 8.0, Some(Side::Left), &[], false);
        let goals = entries(&g, b, 8.0, None, &[], false);
        let r = shortest(&g, &starts, &goals, OPEN, FREE).expect("route");
        assert_eq!(starts[r.start].side, Side::Left);
        // The long way passes more cells than the direct shot would.
        assert!(r.cells.len() >= 3, "wrap route, got {:?}", r.cells);
    }

    #[test]
    fn closed_channel_forces_the_detour() {
        // A block above the facing row splits the row channel into three
        // cells; closing that channel forces the route down and around.
        let a = body(20.0, 40.0, 40.0, 60.0);
        let b = body(160.0, 40.0, 180.0, 60.0);
        let block = Rect::new(90.0, 0.0, 110.0, 30.0);
        let g = ChannelGraph::build(BOUNDS, &[a.inflate(8.0), b.inflate(8.0), block], false);
        let starts = entries(&g, a, 8.0, None, &[], false);
        let goals = entries(&g, b, 8.0, None, &[], false);
        let direct = shortest(&g, &starts, &goals, OPEN, FREE).expect("route");
        let row =
            g.h.iter()
                .position(|c| c.rect == Rect::new(48.0, 32.0, 152.0, 68.0))
                .expect("row channel");
        let closed = move |axis: Axis, chan: usize, _: f64, _: f64| axis == Axis::H && chan == row;
        let detour = shortest(&g, &starts, &goals, &closed, FREE).expect("route");
        assert!(
            detour.cells.len() > direct.cells.len(),
            "detour {:?} vs direct {:?}",
            detour.cells,
            direct.cells
        );
    }

    #[test]
    fn shortest_is_deterministic() {
        let (g, a, b) = facing();
        let starts = entries(&g, a, 8.0, None, &[], false);
        let goals = entries(&g, b, 8.0, None, &[], false);
        let first = shortest(&g, &starts, &goals, OPEN, FREE);
        for _ in 0..50 {
            assert_eq!(shortest(&g, &starts, &goals, OPEN, FREE), first);
        }
    }
}

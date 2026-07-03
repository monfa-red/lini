//! Route geometry (ROUTING.md model step 6, plus the search → placement
//! bridge): the winning cell path lowered to a [`Chain`] of channel runs,
//! placed chains lowered to orthogonal polylines, and the stray segment for
//! links no law can draw.
//!
//! Runs come out of the cell path by merging same-direction hops along one
//! channel. A reversal — doubling back inside one channel — keeps its legs
//! as two runs and hangs a **jog** run in the bounce cell's crossing channel
//! (the U the search priced as two turns); a single straight run whose two
//! port windows cannot meet expands the same way, end – jog – end. Spans are
//! provisional: each run reaches its neighbours' estimates (ports for end
//! runs, channel anchors for interior ones) — the same estimates the search
//! and the ledger price with — until placement pins ordinates and the
//! polyline takes corners from those.

use super::cost::min_pitch;
use super::entry::Entry;
use super::graph::{Axis, ChannelGraph};
use super::ledger::Ledger;
use super::rect::Rect;
use super::{Chain, EndInfo, Run};

fn perp(axis: Axis) -> Axis {
    match axis {
        Axis::H => Axis::V,
        Axis::V => Axis::H,
    }
}

fn chan_of(graph: &ChannelGraph, cell: usize, axis: Axis) -> usize {
    match axis {
        Axis::H => graph.cells[cell].h,
        Axis::V => graph.cells[cell].v,
    }
}

fn centre_along(graph: &ChannelGraph, cell: usize, axis: Axis) -> f64 {
    let r = graph.cells[cell].rect;
    match axis {
        Axis::H => (r.x0 + r.x1) / 2.0,
        Axis::V => (r.y0 + r.y1) / 2.0,
    }
}

/// An entry's port ordinate across its travel axis (an H run's y, a V run's x).
fn pin_ord(e: &Entry) -> f64 {
    match e.axis {
        Axis::H => e.port.1,
        Axis::V => e.port.0,
    }
}

/// A cell's extent along `axis` — a jog's provisional travel range.
fn cell_interval(graph: &ChannelGraph, cell: usize, axis: Axis) -> (f64, f64) {
    let r = graph.cells[cell].rect;
    match axis {
        Axis::H => (r.x0, r.x1),
        Axis::V => (r.y0, r.y1),
    }
}

/// One run in the making: its channel, travel direction (`0.0` for a jog),
/// and provisional travel extent — what an interior run's corridor anchor
/// is estimated over.
struct Seed {
    axis: Axis,
    chan: usize,
    dir: f64,
    ext: (f64, f64),
}

/// Lower a winning cell path to a chain of channel runs. `ledger`, `k`, and
/// `clearance` reproduce the search's straight-or-jog decision and its
/// jog-channel pick, so the drawn jog rides exactly the channel the search
/// verified and priced.
#[allow(clippy::too_many_arguments)]
pub(crate) fn chain(
    graph: &ChannelGraph,
    world: usize,
    ledger: &Ledger,
    cells: &[usize],
    start: &Entry,
    goal: &Entry,
    ends: [EndInfo; 2],
    link: usize,
    k: usize,
    clearance: f64,
) -> Chain {
    let sgn = |v: f64| if v < 0.0 { -1.0 } else { 1.0 };
    let seed = |cell: usize, axis: Axis, dir: f64, from: f64| {
        let c = centre_along(graph, cell, axis);
        Seed {
            axis,
            chan: chan_of(graph, cell, axis),
            dir,
            ext: (from.min(c), from.max(c)),
        }
    };
    let jog_at = |cell: usize, axis: Axis| Seed {
        axis: perp(axis),
        chan: chan_of(graph, cell, perp(axis)),
        dir: 0.0,
        ext: cell_interval(graph, cell, perp(axis)),
    };

    // Travel runs: directed hops merged along their channel; a reversal
    // splits legs around a jog at the bounce cell.
    let mut seeds: Vec<Seed> = Vec::new();
    for w in cells.windows(2) {
        let (a, b) = (w[0], w[1]);
        let axis = if graph.cells[a].h == graph.cells[b].h {
            Axis::H
        } else {
            Axis::V
        };
        let (ca, cb) = (centre_along(graph, a, axis), centre_along(graph, b, axis));
        let dir = sgn(cb - ca);
        match seeds.last_mut() {
            Some(s) if s.axis == axis && s.chan == chan_of(graph, a, axis) => {
                if s.dir == dir {
                    s.ext = (s.ext.0.min(cb), s.ext.1.max(cb));
                } else {
                    seeds.push(jog_at(a, axis));
                    seeds.push(seed(a, axis, dir, ca));
                }
            }
            _ => seeds.push(seed(a, axis, dir, ca)),
        }
    }

    // Attach the ends: the port run merges with a travel run continuing its
    // punch direction, turns a corner off a perpendicular one, and hangs a
    // jog when the first travel doubles straight back against the punch.
    // Directions are indexed E, S, W, N; the goal approach opposes its punch.
    let dir_sign = |d: usize| if d < 2 { 1.0 } else { -1.0 };
    let (sdir, gdir) = (dir_sign(start.dir), -dir_sign(goal.dir));
    let (first_cell, last_cell) = (cells[0], *cells.last().expect("entered cell"));
    let (s_line, g_line) = (ends[0].side_coord(), ends[1].side_coord());
    match seeds.first() {
        None if start.axis == goal.axis => seeds.push(seed(first_cell, start.axis, sdir, s_line)),
        None => {
            seeds.push(seed(first_cell, start.axis, sdir, s_line));
            seeds.push(seed(last_cell, goal.axis, gdir, g_line));
        }
        Some(f) if f.axis == start.axis && f.dir == sdir => {}
        Some(f) if f.axis == start.axis => {
            seeds.insert(0, jog_at(first_cell, start.axis));
            seeds.insert(0, seed(first_cell, start.axis, sdir, s_line));
        }
        Some(_) => seeds.insert(0, seed(first_cell, start.axis, sdir, s_line)),
    }
    match seeds.last() {
        Some(l) if l.axis == goal.axis && l.dir == gdir => {}
        Some(l) if l.axis == goal.axis => {
            seeds.push(jog_at(last_cell, goal.axis));
            seeds.push(seed(last_cell, goal.axis, gdir, g_line));
        }
        _ => seeds.push(seed(last_cell, goal.axis, gdir, g_line)),
    }

    // One straight run claimed by both ends draws straight only when the
    // windows meet on k tracks; otherwise it expands end – jog – end, the
    // jog riding the first traversed cell whose crossing channel holds it —
    // the search's own decision, reproduced (ROUTING.md model step 4).
    if seeds.len() == 1 {
        let (wa, wb) = (ends[0].window, ends[1].window);
        let shared = (wa.0.max(wb.0), wa.1.min(wb.1));
        let fits = shared.0 <= shared.1
            && ((shared.1 - shared.0) / min_pitch(clearance)).floor() as usize + 1 >= k;
        if !fits {
            let axis = seeds[0].axis;
            let (lo, hi) = (wa.1.min(wb.1), wa.0.max(wb.0));
            let span = (lo.min(hi), hi.max(lo));
            let cell = cells
                .iter()
                .copied()
                .find(|&c| {
                    ledger.tracks_left(
                        world,
                        perp(axis),
                        chan_of(graph, c, perp(axis)),
                        span,
                        graph,
                    ) >= k
                })
                .expect("the search verified the jog's channel");
            let dir = seeds[0].dir;
            let mut jog = jog_at(cell, axis);
            jog.ext = span;
            seeds = vec![
                seed(first_cell, axis, dir, s_line),
                jog,
                seed(last_cell, axis, dir, g_line),
            ];
        }
    }

    // Provisional spans: every run reaches its neighbours' estimates — the
    // ports for end runs, corridor anchors inside (over the seed's own
    // travel extent, the same anchor placement will prefer) — and the end
    // runs start on their side lines. Placement replaces the estimates;
    // geometry then takes corners from the placed ordinates.
    let n = seeds.len();
    let ests: Vec<f64> = seeds
        .iter()
        .enumerate()
        .map(|(i, s)| {
            if i == 0 {
                pin_ord(start)
            } else if i == n - 1 {
                pin_ord(goal)
            } else {
                graph.corridor(s.axis, s.chan, s.ext.0, s.ext.1).anchor()
            }
        })
        .collect();
    let runs = seeds
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let lo = if i == 0 {
                ends[0].side_coord()
            } else {
                ests[i - 1]
            };
            let hi = if i == n - 1 {
                ends[1].side_coord()
            } else {
                ests[i + 1]
            };
            Run {
                axis: s.axis,
                chan: s.chan,
                span: (lo.min(hi), lo.max(hi)),
                ord: None,
            }
        })
        .collect();
    Chain {
        link,
        world,
        runs,
        ends,
    }
}

/// The link polyline: port, the meets of adjacent runs, port. Collinear
/// middles merge — a jog whose legs land on one track collapses away.
pub(crate) fn polyline(chain: &Chain) -> Vec<(f64, f64)> {
    let ord = |i: usize| {
        chain.runs[i]
            .ord
            .expect("placement assigned every ordinate")
    };
    let port = |i: usize, end: &EndInfo| match chain.runs[i].axis {
        Axis::H => (end.side_coord(), ord(i)),
        Axis::V => (ord(i), end.side_coord()),
    };
    let mut pts = Vec::with_capacity(chain.runs.len() + 1);
    pts.push(port(0, &chain.ends[0]));
    for i in 0..chain.runs.len() - 1 {
        pts.push(match chain.runs[i].axis {
            Axis::H => (ord(i + 1), ord(i)),
            Axis::V => (ord(i), ord(i + 1)),
        });
    }
    pts.push(port(chain.runs.len() - 1, &chain.ends[1]));
    simplify(pts)
}

/// Drop repeated points and collinear middles.
fn simplify(pts: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::new();
    for p in pts {
        if out.last() == Some(&p) {
            continue;
        }
        if out.len() >= 2 {
            let a = out[out.len() - 2];
            let b = out[out.len() - 1];
            if (a.0 == b.0 && b.0 == p.0) || (a.1 == b.1 && b.1 == p.1) {
                out.pop();
            }
        }
        out.push(p);
    }
    out
}

/// The stray segment for an impossible link (ROUTING.md §Impossible layouts):
/// centre to centre, each end trimmed to its own body's boundary. `None` when
/// the trim leaves nothing — coincident or overlapping bodies (self-loops,
/// containment), where no between-bodies segment exists.
pub fn stray_segment(a: Rect, b: Rect) -> Option<((f64, f64), (f64, f64))> {
    let centre = |r: Rect| ((r.x0 + r.x1) / 2.0, (r.y0 + r.y1) / 2.0);
    let (ca, cb) = (centre(a), centre(b));
    let d = (cb.0 - ca.0, cb.1 - ca.1);
    if d == (0.0, 0.0) {
        return None;
    }
    // Parameter along ca→cb at which a ray from a rect's centre exits it.
    let exit = |r: Rect, o: (f64, f64), d: (f64, f64)| {
        let along = |lo: f64, hi: f64, o: f64, d: f64| {
            if d > 0.0 {
                (hi - o) / d
            } else if d < 0.0 {
                (lo - o) / d
            } else {
                f64::INFINITY
            }
        };
        along(r.x0, r.x1, o.0, d.0).min(along(r.y0, r.y1, o.1, d.1))
    };
    let t0 = exit(a, ca, d);
    let t1 = 1.0 - exit(b, cb, (-d.0, -d.1));
    (t0 < t1).then_some((
        (ca.0 + d.0 * t0, ca.1 + d.1 * t0),
        (ca.0 + d.0 * t1, ca.1 + d.1 * t1),
    ))
}

#[cfg(test)]
mod tests {
    use super::super::World;
    use super::super::entry::entries;
    use super::super::place::place;
    use super::super::search::cheapest;
    use super::*;
    use crate::ast::Side;

    const BOUNDS: Rect = Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 200.0,
        y1: 100.0,
    };
    const C: f64 = 8.0;

    fn orthogonal(p: &[(f64, f64)]) {
        for s in p.windows(2) {
            assert!(
                s[0].0 == s[1].0 || s[0].1 == s[1].1,
                "diagonal segment {s:?}"
            );
            assert!(s[0] != s[1], "zero-length segment {s:?}");
        }
    }

    fn end_of(e: &Entry, rect: Rect) -> EndInfo {
        EndInfo {
            side: e.side,
            rect,
            window: e.window,
            fan: None,
        }
    }

    /// Route a→b in an otherwise-empty world and lower to a polyline.
    fn route_between(a: Rect, b: Rect, extra: &[Rect]) -> Vec<(f64, f64)> {
        let mut keepouts = vec![a.inflate(C), b.inflate(C)];
        keepouts.extend_from_slice(extra);
        let graph = ChannelGraph::build(BOUNDS, &keepouts, false);
        let ledger = Ledger::new(C);
        let starts = entries(&graph, a, C, C, None, extra, false);
        let goals = entries(&graph, b, C, C, None, extra, false);
        let r = cheapest(&graph, 0, &starts, &goals, &ledger, &[], 1, C).expect("route");
        let (se, ge) = (&starts[r.start], &goals[r.goal]);
        let ends = [end_of(se, a), end_of(ge, b)];
        let mut chains = vec![Some(chain(
            &graph, 0, &ledger, &r.cells, se, ge, ends, 0, 1, C,
        ))];
        let worlds = [World {
            path: String::new(),
            graph,
        }];
        place(&worlds, &mut chains, C);
        polyline(chains[0].as_ref().unwrap())
    }

    #[test]
    fn aligned_facing_nodes_yield_a_straight_link() {
        let a = Rect::new(20.0, 40.0, 40.0, 60.0);
        let b = Rect::new(160.0, 40.0, 180.0, 60.0);
        let p = route_between(a, b, &[]);
        assert_eq!(p, vec![(40.0, 50.0), (160.0, 50.0)]);
    }

    #[test]
    fn misaligned_windows_jog_on_the_gap_midline() {
        // Windows (48, 52) vs (54, 58): past overlap, so the route jogs once
        // and the perpendicular run rides the corridor midline (x = 100).
        let a = Rect::new(20.0, 40.0, 40.0, 60.0);
        let b = Rect::new(160.0, 46.0, 180.0, 66.0);
        let p = route_between(a, b, &[]);
        orthogonal(&p);
        assert_eq!(p.len(), 4, "one jog: {p:?}");
        assert_eq!(p[1].0, 100.0);
        assert_eq!(p[2].0, 100.0);
        assert_eq!(p[0], (40.0, p[1].1));
        assert_eq!(p[3], (160.0, p[2].1));
    }

    #[test]
    fn blocked_route_detours_clear_of_the_blocker() {
        let a = Rect::new(20.0, 40.0, 40.0, 60.0);
        let b = Rect::new(160.0, 40.0, 180.0, 60.0);
        let wall = Rect::new(90.0, 20.0, 110.0, 80.0);
        let p = route_between(a, b, &[wall.inflate(C)]);
        orthogonal(&p);
        let is_port_of = |p: (f64, f64), r: Rect| {
            let (cx, cy) = ((r.x0 + r.x1) / 2.0, (r.y0 + r.y1) / 2.0);
            [(r.x1, cy), (cx, r.y1), (r.x0, cy), (cx, r.y0)].contains(&p)
        };
        assert!(
            is_port_of(*p.first().unwrap(), a),
            "start not a port: {p:?}"
        );
        assert!(is_port_of(*p.last().unwrap(), b), "end not a port: {p:?}");
        for s in p.windows(2) {
            let (sx0, sx1) = (s[0].0.min(s[1].0), s[0].0.max(s[1].0));
            let (sy0, sy1) = (s[0].1.min(s[1].1), s[0].1.max(s[1].1));
            let pierces =
                sx0 < wall.x1 + C && sx1 > wall.x0 - C && sy0 < wall.y1 + C && sy1 > wall.y0 - C;
            assert!(!pierces, "segment {s:?} pierces the wall's keep-out");
        }
    }

    #[test]
    fn a_reversal_expands_to_legs_around_a_jog() {
        // Hand-built U: down the left flank, bounce in the top-left cell,
        // back down — three runs plus the jog between the legs.
        let graph = ChannelGraph::build(BOUNDS, &[Rect::new(80.0, 40.0, 120.0, 100.0)], false);
        let mid = |c: &super::super::graph::Cell| {
            ((c.rect.x0 + c.rect.x1) / 2.0, (c.rect.y0 + c.rect.y1) / 2.0)
        };
        let cell_at = |x: f64, y: f64| {
            graph
                .cells
                .iter()
                .position(|c| {
                    let (cx, cy) = mid(c);
                    (cx - x).abs() < 40.0 && (cy - y).abs() < 25.0
                })
                .expect("cell")
        };
        let low = cell_at(40.0, 70.0); // left of the block
        let top = cell_at(40.0, 20.0); // above it, same V channel
        let start = Entry {
            side: Side::Top,
            port: (40.0, 100.0),
            window: (20.0, 60.0),
            tip: (40.0, 92.0),
            axis: Axis::V,
            dir: 3,
            cell: low,
        };
        let goal = Entry {
            side: Side::Top,
            port: (60.0, 100.0),
            window: (50.0, 70.0),
            tip: (60.0, 92.0),
            axis: Axis::V,
            dir: 3,
            cell: low,
        };
        let ends = [
            end_of(&start, Rect::new(20.0, 100.0, 60.0, 120.0)),
            end_of(&goal, Rect::new(40.0, 100.0, 80.0, 120.0)),
        ];
        let ledger = Ledger::new(C);
        let ch = chain(
            &graph,
            0,
            &ledger,
            &[low, top, low],
            &start,
            &goal,
            ends,
            0,
            1,
            C,
        );
        let axes: Vec<Axis> = ch.runs.iter().map(|r| r.axis).collect();
        assert_eq!(axes, vec![Axis::V, Axis::H, Axis::V]);
        let chans: Vec<usize> = ch.runs.iter().map(|r| r.chan).collect();
        assert_eq!(chans[0], chans[2], "both legs ride one V channel");
    }

    #[test]
    fn stray_trims_to_both_boundaries() {
        // Facing horizontally: the segment runs face to face on the centreline.
        let a = Rect::new(0.0, 0.0, 40.0, 40.0);
        let b = Rect::new(100.0, 0.0, 140.0, 40.0);
        assert_eq!(stray_segment(a, b), Some(((40.0, 20.0), (100.0, 20.0))));
        // Diagonal neighbours: a slanted segment, trimmed where the
        // centre-to-centre ray leaves each body.
        let c = Rect::new(100.0, 100.0, 140.0, 140.0);
        let (p, q) = stray_segment(a, c).expect("segment");
        assert_eq!(p, (40.0, 40.0));
        assert_eq!(q, (100.0, 100.0));
    }

    #[test]
    fn stray_skips_degenerate_pairs() {
        let a = Rect::new(0.0, 0.0, 40.0, 40.0);
        assert_eq!(stray_segment(a, a), None);
        // One body inside the other: no between-bodies segment exists.
        let inner = Rect::new(10.0, 10.0, 20.0, 20.0);
        assert_eq!(stray_segment(a, inner), None);
    }
}

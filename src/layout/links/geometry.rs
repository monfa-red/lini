//! Route → chain → orthogonal polyline.
//!
//! A cell route becomes alternating channel **runs**; a run that ends at a
//! port is pinned to the port's ordinate (LINKING §Model step 5 — the stub
//! continues straight into the channel), through-runs are assigned by
//! [`super::runs`]. Corners are the meet of adjacent runs' ordinates. A
//! self-loop is a fixed chain along its keep-out boundary, its legs pinned to
//! the assigned ports.

use super::graph::{Axis, ChannelGraph};
use super::path::Entry;
use super::rect::Rect;
use super::runs::{Chain, Conn, EndInfo, Pin, Run};
use crate::ast::Side;

fn centre(graph: &ChannelGraph, cell: usize, axis: Axis) -> f64 {
    let r = graph.cells[cell].rect;
    match axis {
        Axis::H => (r.x0 + r.x1) / 2.0,
        Axis::V => (r.y0 + r.y1) / 2.0,
    }
}

fn pin_ord(e: &Entry) -> f64 {
    match e.axis {
        Axis::H => e.port.1,
        Axis::V => e.port.0,
    }
}

fn tip_q(e: &Entry) -> f64 {
    match e.axis {
        Axis::H => e.tip.0,
        Axis::V => e.tip.1,
    }
}

fn chan_of(graph: &ChannelGraph, cell: usize, axis: Axis) -> usize {
    match axis {
        Axis::H => graph.cells[cell].h,
        Axis::V => graph.cells[cell].v,
    }
}

/// Build the chain for a routed edge, port to port. `margin` records a
/// route admitted by the otherwise-impossible lever — its overflow in an
/// open outer channel pitches outward past the canvas bound.
#[allow(clippy::too_many_arguments)]
pub fn chain(
    graph: &ChannelGraph,
    world: usize,
    cells: &[usize],
    start: &Entry,
    goal: &Entry,
    ends: [EndInfo; 2],
    req: usize,
    margin: bool,
) -> Chain {
    let mut runs: Vec<Run> = Vec::new();
    for w in cells.windows(2) {
        let (a, b) = (&graph.cells[w[0]], &graph.cells[w[1]]);
        let (axis, chan) = if a.h == b.h {
            (Axis::H, a.h)
        } else {
            (Axis::V, a.v)
        };
        match runs.last_mut() {
            Some(r) if r.axis == axis && r.chan == chan => {
                r.conn[1] = Conn::Junction {
                    cell: w[1],
                    q: centre(graph, w[1], axis),
                };
            }
            _ => runs.push(Run {
                axis,
                chan,
                pin: Pin::Free,
                ord: 0.0,
                conn: [
                    Conn::Junction {
                        cell: w[0],
                        q: centre(graph, w[0], axis),
                    },
                    Conn::Junction {
                        cell: w[1],
                        q: centre(graph, w[1], axis),
                    },
                ],
            }),
        }
    }

    if runs.first().map(|r| r.axis) == Some(start.axis) {
        runs[0].pin = Pin::Port(0);
        runs[0].ord = pin_ord(start);
        runs[0].conn[0] = Conn::Terminal { q: tip_q(start) };
    } else {
        let cell = cells[0];
        runs.insert(
            0,
            Run {
                axis: start.axis,
                chan: chan_of(graph, cell, start.axis),
                pin: Pin::Port(0),
                ord: pin_ord(start),
                conn: [
                    Conn::Terminal { q: tip_q(start) },
                    Conn::Junction {
                        cell,
                        q: centre(graph, cell, start.axis),
                    },
                ],
            },
        );
    }

    let last = runs.len() - 1;
    if runs[last].axis != goal.axis {
        let cell = *cells.last().unwrap();
        runs.push(Run {
            axis: goal.axis,
            chan: chan_of(graph, cell, goal.axis),
            pin: Pin::Port(1),
            ord: pin_ord(goal),
            conn: [
                Conn::Junction {
                    cell,
                    q: centre(graph, cell, goal.axis),
                },
                Conn::Terminal { q: tip_q(goal) },
            ],
        });
    } else if last > 0 {
        runs[last].pin = Pin::Port(1);
        runs[last].ord = pin_ord(goal);
        runs[last].conn[1] = Conn::Terminal { q: tip_q(goal) };
    } else {
        // One straight run claimed by both ends: split into approach – jog –
        // approach so the ports may take different ordinates (the jog
        // collapses in `simplify` when they align).
        let cell = *cells.last().unwrap();
        let jog_axis = match start.axis {
            Axis::H => Axis::V,
            Axis::V => Axis::H,
        };
        runs[0].conn[1] = Conn::Junction {
            cell,
            q: centre(graph, cell, start.axis),
        };
        runs.push(Run {
            axis: jog_axis,
            chan: chan_of(graph, cell, jog_axis),
            pin: Pin::Free,
            ord: 0.0,
            conn: [
                Conn::Junction {
                    cell,
                    q: pin_ord(start),
                },
                Conn::Junction {
                    cell,
                    q: pin_ord(goal),
                },
            ],
        });
        runs.push(Run {
            axis: goal.axis,
            chan: chan_of(graph, cell, goal.axis),
            pin: Pin::Port(1),
            ord: pin_ord(goal),
            conn: [
                Conn::Junction {
                    cell,
                    q: centre(graph, cell, goal.axis),
                },
                Conn::Terminal { q: tip_q(goal) },
            ],
        });
    }

    // A junction span-end next to a pinned neighbour reaches that ordinate,
    // not the cell centre — tighter occupancy and ordering.
    for i in 0..runs.len() {
        for (e, nb) in [(0, i.wrapping_sub(1)), (1, i + 1)] {
            if nb >= runs.len() || runs[nb].pin == Pin::Free {
                continue;
            }
            if let Conn::Junction { cell, .. } = runs[i].conn[e] {
                let q = runs[nb].ord;
                runs[i].conn[e] = Conn::Junction { cell, q };
            }
        }
    }

    Chain {
        world,
        runs,
        ends,
        req,
        margin,
    }
}

/// The link polyline: port, the meets of adjacent runs, port. Runs strictly
/// alternate axes — an inversion's swap jog is a run like any other.
pub fn polyline(chain: &Chain) -> Vec<(f64, f64)> {
    let mut pts = vec![chain.ends[0].port];
    for w in chain.runs.windows(2) {
        let corner = match w[0].axis {
            Axis::H => (w[1].ord, w[0].ord),
            Axis::V => (w[0].ord, w[1].ord),
        };
        pts.push(corner);
    }
    pts.push(chain.ends[1].port);
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

/// The stray segment for an impossible link (LINKING §Impossible layouts):
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

/// The self-loop chain: legs out of both ports, wall runs along the keep-out
/// boundary, the shorter way around (ties wrap over the top). Sides come from
/// `ends`. `None` when both ends resolve to one side, or the wall runs leave
/// the world's cells.
pub fn self_loop_chain(
    graph: &ChannelGraph,
    world: usize,
    body: Rect,
    keepout: Rect,
    ends: [EndInfo; 2],
    req: usize,
) -> Option<Chain> {
    let (sa, sb) = (ends[0].side, ends[1].side);
    if sa == sb {
        return None;
    }
    let k = keepout;
    let cx = (body.x0 + body.x1) / 2.0;
    let cy = (body.y0 + body.y1) / 2.0;

    // Clockwise on screen; `wall(s)` is the keep-out line a run rides along
    // side `s`, `port_q(s)` the provisional port ordinate on that side.
    let order = [Side::Right, Side::Bottom, Side::Left, Side::Top];
    let wall = |s: Side| match s {
        Side::Right => (Axis::V, k.x1),
        Side::Bottom => (Axis::H, k.y1),
        Side::Left => (Axis::V, k.x0),
        Side::Top => (Axis::H, k.y0),
    };
    let port_q = |s: Side| match s {
        Side::Right | Side::Left => cy,
        Side::Top | Side::Bottom => cx,
    };
    let body_line = |s: Side| match s {
        Side::Right => body.x1,
        Side::Bottom => body.y1,
        Side::Left => body.x0,
        Side::Top => body.y0,
    };

    let ia = order.iter().position(|&s| s == sa).expect("side");
    let ib = order.iter().position(|&s| s == sb).expect("side");
    let walk_sides = |cw: bool| {
        let mut sides = vec![sa];
        let mut i = ia;
        while i != ib {
            i = if cw { (i + 1) % 4 } else { (i + 3) % 4 };
            sides.push(order[i]);
        }
        sides
    };
    let (cw, ccw) = (walk_sides(true), walk_sides(false));
    // Fewer corners wins; a tie wraps over the top.
    let sides = match cw.len().cmp(&ccw.len()) {
        std::cmp::Ordering::Less => cw,
        std::cmp::Ordering::Greater => ccw,
        std::cmp::Ordering::Equal if cw.contains(&Side::Top) => cw,
        std::cmp::Ordering::Equal => ccw,
    };

    let mut runs: Vec<Run> = Vec::new();
    let leg_axis = match sa {
        Side::Right | Side::Left => Axis::H,
        Side::Top | Side::Bottom => Axis::V,
    };
    let cell_just_outside = |s: Side, ord: f64| {
        let (_, w) = wall(s);
        let p = match s {
            Side::Right => (w + 0.01, ord),
            Side::Left => (w - 0.01, ord),
            Side::Top => (ord, w - 0.01),
            Side::Bottom => (ord, w + 0.01),
        };
        graph.cells.iter().position(|c| {
            p.0 >= c.rect.x0 && p.0 <= c.rect.x1 && p.1 >= c.rect.y0 && p.1 <= c.rect.y1
        })
    };
    let start_cell = cell_just_outside(sa, port_q(sa))?;
    runs.push(Run {
        axis: leg_axis,
        chan: chan_of(graph, start_cell, leg_axis),
        pin: Pin::Port(0),
        ord: port_q(sa),
        conn: [
            Conn::Terminal { q: body_line(sa) },
            Conn::Junction {
                cell: start_cell,
                q: wall(sa).1,
            },
        ],
    });
    // Wall runs.
    for (si, &s) in sides.iter().enumerate() {
        let (axis, ord) = wall(s);
        let prev_q = if si == 0 {
            port_q(sa)
        } else {
            wall(sides[si - 1]).1
        };
        let next_q = if si + 1 == sides.len() {
            port_q(sb)
        } else {
            wall(sides[si + 1]).1
        };
        let mid = match s {
            Side::Right => (ord + 0.01, (prev_q + next_q) / 2.0),
            Side::Left => (ord - 0.01, (prev_q + next_q) / 2.0),
            Side::Top => ((prev_q + next_q) / 2.0, ord - 0.01),
            Side::Bottom => ((prev_q + next_q) / 2.0, ord + 0.01),
        };
        let cell = graph.cells.iter().position(|c| {
            mid.0 >= c.rect.x0 && mid.0 <= c.rect.x1 && mid.1 >= c.rect.y0 && mid.1 <= c.rect.y1
        })?;
        runs.push(Run {
            axis,
            chan: chan_of(graph, cell, axis),
            pin: Pin::Fixed(ord),
            ord,
            conn: [
                Conn::Junction { cell, q: prev_q },
                Conn::Junction { cell, q: next_q },
            ],
        });
    }
    // Leg back into the goal port.
    let leg_axis_b = match sb {
        Side::Right | Side::Left => Axis::H,
        Side::Top | Side::Bottom => Axis::V,
    };
    let goal_cell = cell_just_outside(sb, port_q(sb))?;
    runs.push(Run {
        axis: leg_axis_b,
        chan: chan_of(graph, goal_cell, leg_axis_b),
        pin: Pin::Port(1),
        ord: port_q(sb),
        conn: [
            Conn::Junction {
                cell: goal_cell,
                q: wall(sb).1,
            },
            Conn::Terminal { q: body_line(sb) },
        ],
    });

    Some(Chain {
        world,
        runs,
        ends,
        req,
        margin: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::links::path::{FREE, entries, shortest};
    use crate::layout::links::runs::{World, assign};

    const BOUNDS: Rect = Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 200.0,
        y1: 100.0,
    };

    fn orthogonal(p: &[(f64, f64)]) {
        for s in p.windows(2) {
            assert!(
                s[0].0 == s[1].0 || s[0].1 == s[1].1,
                "diagonal segment {s:?}"
            );
            assert!(s[0] != s[1], "zero-length segment {s:?}");
        }
    }

    fn route_between(a: Rect, b: Rect, extra: &[Rect]) -> Vec<(f64, f64)> {
        let mut keepouts = vec![a.inflate(8.0), b.inflate(8.0)];
        keepouts.extend_from_slice(extra);
        let graph = ChannelGraph::build(BOUNDS, &keepouts, false);
        let starts = entries(&graph, a, 8.0, None, extra, false);
        let goals = entries(&graph, b, 8.0, None, extra, false);
        let r = shortest(&graph, &starts, &goals, &|_, _, _, _| false, FREE).expect("route");
        let (se, ge) = (&starts[r.start], &goals[r.goal]);
        let ends = [("a", a, se), ("b", b, ge)].map(|(path, rect, e)| EndInfo {
            path: path.to_owned(),
            side: e.side,
            rect,
            port: e.port,
            fan: None,
        });
        let mut chains = vec![Some(chain(&graph, 0, &r.cells, se, ge, ends, 0, false))];
        let worlds = [World {
            path: String::new(),
            graph,
        }];
        assign(&worlds, &mut chains, 8.0, &Default::default());
        polyline(chains[0].as_ref().unwrap())
    }

    #[test]
    fn aligned_facing_nodes_yield_a_straight_link() {
        let a = Rect::new(20.0, 40.0, 40.0, 60.0);
        let b = Rect::new(160.0, 40.0, 180.0, 60.0);
        let p = route_between(a, b, &[]);
        assert_eq!(p, vec![(40.0, 50.0), (160.0, 50.0)]);
    }

    fn is_port_of(p: (f64, f64), r: Rect) -> bool {
        let (cx, cy) = ((r.x0 + r.x1) / 2.0, (r.y0 + r.y1) / 2.0);
        [(r.x1, cy), (cx, r.y1), (r.x0, cy), (cx, r.y0)].contains(&p)
    }

    #[test]
    fn misaligned_facing_nodes_jog_once() {
        let a = Rect::new(20.0, 20.0, 40.0, 40.0);
        let b = Rect::new(160.0, 60.0, 180.0, 80.0);
        let p = route_between(a, b, &[]);
        orthogonal(&p);
        assert!(
            is_port_of(*p.first().unwrap(), a),
            "start not a port: {p:?}"
        );
        assert!(is_port_of(*p.last().unwrap(), b), "end not a port: {p:?}");
        assert!(p.len() <= 4, "one jog at most, got {p:?}");
    }

    #[test]
    fn blocked_route_detours_clear_of_the_blocker() {
        let a = Rect::new(20.0, 40.0, 40.0, 60.0);
        let b = Rect::new(160.0, 40.0, 180.0, 60.0);
        let wall = Rect::new(90.0, 10.0, 110.0, 90.0);
        let p = route_between(a, b, &[wall.inflate(8.0)]);
        orthogonal(&p);
        assert!(
            is_port_of(*p.first().unwrap(), a),
            "start not a port: {p:?}"
        );
        assert!(is_port_of(*p.last().unwrap(), b), "end not a port: {p:?}");
        for s in p.windows(2) {
            let (sx0, sx1) = (s[0].0.min(s[1].0), s[0].0.max(s[1].0));
            let (sy0, sy1) = (s[0].1.min(s[1].1), s[0].1.max(s[1].1));
            let overlaps = sx0 < wall.x1 + 8.0
                && sx1 > wall.x0 - 8.0
                && sy0 < wall.y1 + 8.0
                && sy1 > wall.y0 - 8.0;
            assert!(!overlaps, "segment {s:?} pierces the wall's keep-out");
        }
    }

    fn loop_polyline(sa: Side, sb: Side) -> Vec<(f64, f64)> {
        let body = Rect::new(40.0, 40.0, 80.0, 60.0);
        let graph = ChannelGraph::build(BOUNDS, &[body.inflate(10.0)], false);
        let ends = [sa, sb].map(|s| {
            let cx = (body.x0 + body.x1) / 2.0;
            let cy = (body.y0 + body.y1) / 2.0;
            let port = match s {
                Side::Right => (body.x1, cy),
                Side::Bottom => (cx, body.y1),
                Side::Left => (body.x0, cy),
                Side::Top => (cx, body.y0),
            };
            EndInfo {
                path: String::new(),
                side: s,
                rect: body,
                port,
                fan: None,
            }
        });
        let mut chains = vec![Some(
            self_loop_chain(&graph, 0, body, body.inflate(10.0), ends, 0).expect("loop"),
        )];
        let worlds = [World {
            path: String::new(),
            graph,
        }];
        assign(&worlds, &mut chains, 10.0, &Default::default());
        polyline(chains[0].as_ref().unwrap())
    }

    #[test]
    fn self_loop_wraps_the_adjacent_corner() {
        let p = loop_polyline(Side::Right, Side::Top);
        assert_eq!(
            p,
            vec![
                (80.0, 50.0),
                (90.0, 50.0),
                (90.0, 30.0),
                (60.0, 30.0),
                (60.0, 40.0),
            ]
        );
    }

    #[test]
    fn self_loop_between_opposite_sides_goes_over_the_top() {
        let p = loop_polyline(Side::Right, Side::Left);
        orthogonal(&p);
        assert_eq!(p.first(), Some(&(80.0, 50.0)));
        assert_eq!(p.last(), Some(&(40.0, 50.0)));
        assert!(
            p.iter().all(|q| q.1 <= 50.0),
            "loop must wrap over the top, got {p:?}"
        );
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

    #[test]
    fn self_loop_on_one_side_is_refused() {
        let body = Rect::new(40.0, 40.0, 80.0, 60.0);
        let graph = ChannelGraph::build(BOUNDS, &[body.inflate(10.0)], false);
        let ends = [Side::Top, Side::Top].map(|s| EndInfo {
            path: String::new(),
            side: s,
            rect: body,
            port: (60.0, 40.0),
            fan: None,
        });
        assert!(self_loop_chain(&graph, 0, body, body.inflate(10.0), ends, 0).is_none());
    }
}

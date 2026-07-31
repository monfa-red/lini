//! Graph entries (ROUTING.md model step 4): how a link's end reaches a
//! world's free space. An entry is a **punch** — a straight perpendicular
//! run from the side's centre through any transparent ancestor walls into
//! the first world cell, blocked by any solid keep-out — carrying the
//! side's lawful port window, clipped by whatever the punch stretch
//! crosses. The search ([`super::search`]) prices routes between entries.

use super::graph::{Axis, ChannelGraph};
use super::rect::Rect;
use super::search::{DIRS, opposite};
use crate::ast::Side;

/// One way into the graph: a side's provisional port (its centre — placement
/// re-pins), the lawful port **window** on that side (corner margins
/// applied), the punch tip where the link reaches the world's free space,
/// and the punch direction (the wire leaves the port along it).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Entry {
    pub side: Side,
    pub port: (f64, f64),
    pub window: (f64, f64),
    pub tip: (f64, f64),
    pub axis: Axis,
    pub dir: usize,
    pub cell: usize,
}

/// The graph entries of a node — one per side whose punch reaches a world
/// cell without crossing a blocker. `forced` prunes to that side; `fixed`
/// pins the port to an exact ordinate on it and collapses the window to
/// that point (ROUTING.md Fixed ports — a fixed port rides a forced side);
/// `inward` flips the punch into the body (containment ends). `clearance`
/// sets the window's corner margins; a side too short for margins still
/// offers its centre point.
#[allow(clippy::too_many_arguments)]
pub(crate) fn entries(
    graph: &ChannelGraph,
    body: Rect,
    stub: f64,
    clearance: f64,
    forced: Option<Side>,
    fixed: Option<f64>,
    blockers: &[Rect],
    inward: bool,
) -> Vec<Entry> {
    debug_assert!(
        fixed.is_none() || forced.is_some(),
        "a fixed port rides a forced side (ROUTING.md Fixed ports)"
    );
    let cx = (body.x0 + body.x1) / 2.0;
    let cy = (body.y0 + body.y1) / 2.0;
    let window = |lo: f64, hi: f64, centre: f64| {
        let (wlo, whi) = (lo + clearance, hi - clearance);
        if whi < wlo {
            (centre, centre)
        } else {
            (wlo, whi)
        }
    };
    let candidates = [
        (Side::Right, (body.x1, cy), 0, Axis::H),
        (Side::Bottom, (cx, body.y1), 1, Axis::V),
        (Side::Left, (body.x0, cy), 2, Axis::H),
        (Side::Top, (cx, body.y0), 3, Axis::V),
    ];
    candidates
        .into_iter()
        .filter(|(s, ..)| forced.is_none_or(|f| f == *s))
        .filter_map(|(side, port, dir, axis)| {
            let dir = if inward { opposite(dir) } else { dir };
            let (lo, hi, centre) = match axis {
                Axis::H => (body.y0, body.y1, cy),
                Axis::V => (body.x0, body.x1, cx),
            };
            let (port, win) = match fixed {
                Some(f) => {
                    // A port off its own side has no lawful landing — no
                    // entry, and the route loop strays it, named.
                    if f < lo || f > hi {
                        return None;
                    }
                    let port = match axis {
                        Axis::H => (port.0, f),
                        Axis::V => (f, port.1),
                    };
                    (port, (f, f))
                }
                None => (port, window(lo, hi, centre)),
            };
            punch(graph, port, DIRS[dir], stub, blockers).map(|(tip, cell)| Entry {
                side,
                port,
                window: clip_window(win, port, tip, axis, blockers),
                tip,
                axis,
                dir,
                cell,
            })
        })
        .filter(|e| e.window.0 <= e.window.1)
        .collect()
}

/// Shrink a side's port window by the blockers a straight end segment would
/// cross between the side line and the punch tip: there are no cells to
/// turn in before the tip, so the segment holds its port ordinate the whole
/// stretch, and a blocker there — a label inside a transparent ancestor, a
/// walled-in sibling — rules out the port rows it covers. The world's
/// channels never see those interiors; the window is where they are priced.
/// A blocker splitting the window keeps the wider shore.
fn clip_window(
    mut win: (f64, f64),
    port: (f64, f64),
    tip: (f64, f64),
    axis: Axis,
    blockers: &[Rect],
) -> (f64, f64) {
    let (t0, t1) = match axis {
        Axis::H => (port.0.min(tip.0), port.0.max(tip.0)),
        Axis::V => (port.1.min(tip.1), port.1.max(tip.1)),
    };
    let mut cuts: Vec<(f64, f64)> = blockers
        .iter()
        .map(|b| match axis {
            Axis::H => (b.x0, b.x1, b.y0, b.y1),
            Axis::V => (b.y0, b.y1, b.x0, b.x1),
        })
        .filter(|&(blo, bhi, ..)| blo < t1 && bhi > t0)
        .map(|(.., olo, ohi)| (olo, ohi))
        .collect();
    cuts.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    for (olo, ohi) in cuts {
        if ohi <= win.0 || olo >= win.1 {
            continue;
        }
        win = if olo <= win.0 {
            (ohi.max(win.0), win.1)
        } else if ohi >= win.1 {
            (win.0, olo.min(win.1))
        } else if olo - win.0 >= win.1 - ohi {
            (win.0, olo)
        } else {
            (ohi, win.1)
        };
        if win.0 > win.1 {
            break;
        }
    }
    win
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn entries_offer_each_clear_side_in_rank_order() {
        let (g, a, _) = facing();
        let es = entries(&g, a, C, C, None, None, &[], false);
        let sides: Vec<Side> = es.iter().map(|e| e.side).collect();
        assert_eq!(sides, [Side::Right, Side::Bottom, Side::Left, Side::Top]);
        // Right-side port sits mid-side, tip one stub out, window inside the
        // corner margins.
        assert_eq!(es[0].port, (40.0, 50.0));
        assert_eq!(es[0].tip, (48.0, 50.0));
        assert_eq!(es[0].window, (48.0, 52.0));
        assert_eq!(es[0].dir, 0);
        for e in &es {
            let c = g.cells[e.cell].rect;
            assert!(
                e.tip.0 >= c.x0 && e.tip.0 <= c.x1 && e.tip.1 >= c.y0 && e.tip.1 <= c.y1,
                "tip {:?} not in its cell {c:?}",
                e.tip
            );
        }
    }

    #[test]
    fn a_short_side_offers_its_centre_point_window() {
        let (g, ..) = facing();
        let tiny = body(90.0, 40.0, 102.0, 60.0); // width 12 < 2·clearance
        let es = entries(&g, tiny, C, C, Some(Side::Top), None, &[], false);
        assert_eq!(es.len(), 1);
        assert_eq!(es[0].window, (96.0, 96.0));
    }

    #[test]
    fn walled_off_sides_are_dropped() {
        let a = body(20.0, 40.0, 40.0, 60.0);
        let wall = Rect::new(0.0, 0.0, 12.0, 100.0); // flush against a's left keep-out
        let g = ChannelGraph::build(BOUNDS, &[a.inflate(C), wall], false);
        let es = entries(&g, a, C, C, None, None, &[wall], false);
        assert!(es.iter().all(|e| e.side != Side::Left));
        assert_eq!(es.len(), 3);
    }

    #[test]
    fn forced_side_prunes_to_one_entry() {
        let (g, a, _) = facing();
        let es = entries(&g, a, C, C, Some(Side::Top), None, &[], false);
        assert_eq!(es.len(), 1);
        assert_eq!(es[0].side, Side::Top);
    }

    #[test]
    fn punch_crosses_a_transparent_wall_and_is_blocked_by_a_sibling() {
        // A group at x ∈ [60, 120] holds the endpoint; the world sees the
        // group as one keep-out, so the first cell starts at 128.
        let group = Rect::new(60.0, 20.0, 120.0, 80.0);
        let g = ChannelGraph::build(BOUNDS, &[group.inflate(C)], false);
        let inner = body(70.0, 40.0, 90.0, 60.0);
        let es = entries(&g, inner, C, C, Some(Side::Right), None, &[], false);
        assert_eq!(es.len(), 1);
        assert_eq!(es[0].port, (90.0, 50.0));
        assert_eq!(es[0].tip, (128.0, 50.0));
        let sibling = Rect::new(95.0, 30.0, 115.0, 70.0);
        let blocked = entries(&g, inner, C, C, Some(Side::Right), None, &[sibling], false);
        assert!(blocked.is_empty());
    }

    #[test]
    fn a_fixed_port_collapses_the_window_to_its_point() {
        let (g, a, _) = facing();
        let es = entries(&g, a, C, C, Some(Side::Right), Some(44.0), &[], false);
        assert_eq!(es.len(), 1);
        assert_eq!(es[0].port, (40.0, 44.0));
        assert_eq!(es[0].window, (44.0, 44.0));
        assert_eq!(es[0].tip, (48.0, 44.0));
        // An ordinate off the side has no lawful landing: no entry.
        assert!(entries(&g, a, C, C, Some(Side::Right), Some(70.0), &[], false).is_empty());
    }

    #[test]
    fn a_blocked_fixed_port_offers_no_entry() {
        // A sibling covers the port's punch row; the side centre stays
        // clear, so only the pinned landing dies.
        let (g, a, _) = facing();
        let sibling = Rect::new(42.0, 42.0, 46.0, 46.0);
        let free = entries(&g, a, C, C, Some(Side::Right), None, &[sibling], false);
        assert_eq!(free.len(), 1, "the centre punch clears the sibling");
        let pinned = entries(
            &g,
            a,
            C,
            C,
            Some(Side::Right),
            Some(44.0),
            &[sibling],
            false,
        );
        assert!(pinned.is_empty(), "the pinned landing is covered");
    }

    #[test]
    fn inner_entries_point_into_the_body() {
        let parent = body(40.0, 20.0, 160.0, 80.0);
        let g = ChannelGraph::build(parent, &[Rect::new(90.0, 45.0, 110.0, 55.0)], false);
        let es = entries(&g, parent, C, C, None, None, &[], true);
        let right = es.iter().find(|e| e.side == Side::Right).expect("right");
        assert_eq!(right.port, (160.0, 50.0));
        assert_eq!(right.tip, (152.0, 50.0));
        assert_eq!(right.dir, 2); // punches westward, into the body
    }
}

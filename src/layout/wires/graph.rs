//! Channel graph — the free space between keep-outs, decomposed for routing.
//!
//! Two independent sweeps (PLAN.md §Architecture): **V-channels** are maximal
//! free rectangles for vertical travel (x-strips between keep-out edges, equal
//! free y-intervals merged across strips); **H-channels** are the transpose.
//! Both partition the same free space, so every wire run lives in exactly one
//! channel of its orientation. A **cell** is an H∩V overlap; cells are the
//! graph's vertices, and two cells connect iff they abut in a shared channel.
//! Pure geometry: no wire knowledge, identical output for identical input.

use super::rect::Rect;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Axis {
    H,
    V,
}

impl Axis {
    /// Dense id — the routing stages' map key.
    pub fn index(self) -> u8 {
        match self {
            Axis::H => 0,
            Axis::V => 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    pub rect: Rect,
    pub axis: Axis,
    /// Per wall (`[low, high]` on the ordinate axis): the travel intervals
    /// where another same-axis channel abuts — the wall is free space there,
    /// not a keep-out edge, and separation across it is shared between the
    /// two channels rather than guaranteed by either alone.
    pub soft: [Vec<(f64, f64)>; 2],
    /// Per wall: whether it is the **open canvas bound** of the root world —
    /// not geometry, just where the decomposition stopped. Runs never lack
    /// lanes against an open wall: the margin holds any overflow, pitched
    /// outward from the scene. Group walls are real and never open.
    pub outer: [bool; 2],
}

impl Channel {
    /// The extent perpendicular to travel — what separation consumes.
    /// Route-time closure now derives lane demand from cluster loads over the
    /// `usable` range; this and [`Channel::capacity`] remain the contract's
    /// raw lane formula, pinned by unit tests.
    #[cfg(test)]
    pub fn width(&self) -> f64 {
        match self.axis {
            Axis::V => self.rect.w(),
            Axis::H => self.rect.h(),
        }
    }

    /// The channel walls on the ordinate axis.
    pub fn walls(&self) -> (f64, f64) {
        match self.axis {
            Axis::V => (self.rect.x0, self.rect.x1),
            Axis::H => (self.rect.y0, self.rect.y1),
        }
    }

    /// How many runs fit side by side: `floor(width / clearance) + 1` — runs may
    /// sit on the walls (a wall is already `clearance` from its node).
    #[cfg(test)]
    pub fn capacity(&self, clearance: f64) -> usize {
        (self.width() / clearance).floor() as usize + 1
    }

    /// The ordinate range runs spanning `[lo, hi]` may use: the walls, pulled
    /// in by half a clearance wherever the span (inflated by `clearance`)
    /// faces a soft wall — each side of a free boundary surrenders half the
    /// separation it cannot guarantee alone.
    pub fn usable(&self, lo: f64, hi: f64, clearance: f64) -> (f64, f64) {
        let (w0, w1) = self.walls();
        let margin = |soft: &[(f64, f64)]| {
            let near = soft
                .iter()
                .any(|&(a, b)| a < hi + clearance && b > lo - clearance);
            if near { clearance / 2.0 } else { 0.0 }
        };
        (w0 + margin(&self.soft[0]), w1 - margin(&self.soft[1]))
    }

    /// [`Channel::capacity`] within the [`Channel::usable`] range of a span —
    /// zero when the margins leave no room (the channel is closed there).
    #[cfg(test)]
    pub fn capacity_for(&self, lo: f64, hi: f64, clearance: f64) -> usize {
        let (u0, u1) = self.usable(lo, hi, clearance);
        if u1 < u0 {
            0
        } else {
            ((u1 - u0) / clearance).floor() as usize + 1
        }
    }
}

/// One H∩V overlap: the graph vertex. Runs turn inside cells.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub rect: Rect,
    /// Index into `ChannelGraph::h`.
    pub h: usize,
    /// Index into `ChannelGraph::v`.
    pub v: usize,
}

/// Adjacency between two cells abutting in `channel` — the run between them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    pub a: usize,
    pub b: usize,
    pub axis: Axis,
    pub channel: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChannelGraph {
    pub h: Vec<Channel>,
    pub v: Vec<Channel>,
    pub cells: Vec<Cell>,
    pub edges: Vec<Edge>,
}

impl ChannelGraph {
    /// Decompose the free space — `bounds` minus `keepouts` — into channels,
    /// cells, and adjacencies. Keep-outs may overlap each other and `bounds`.
    /// `open` marks `bounds` as the root world's canvas bound rather than a
    /// container wall: channel walls lying on it are open outward.
    pub fn build(bounds: Rect, keepouts: &[Rect], open: bool) -> ChannelGraph {
        let blocks: Vec<Rect> = keepouts
            .iter()
            .filter_map(|k| k.intersect(&bounds))
            .collect();

        let mut v = sweep_channels(bounds, &blocks, Axis::V);
        let mut h = {
            let tb = transpose(bounds);
            let tblocks: Vec<Rect> = blocks.iter().map(|b| transpose(*b)).collect();
            let mut h: Vec<Channel> = sweep_channels(tb, &tblocks, Axis::V)
                .into_iter()
                .map(|c| Channel {
                    rect: transpose(c.rect),
                    axis: Axis::H,
                    soft: [Vec::new(), Vec::new()],
                    outer: [false, false],
                })
                .collect();
            h.sort_by(|a, b| pos_order(a.rect, b.rect));
            h
        };
        soften(&mut v);
        soften(&mut h);
        if open {
            for c in &mut v {
                c.outer = [c.rect.x0 == bounds.x0, c.rect.x1 == bounds.x1];
            }
            for c in &mut h {
                c.outer = [c.rect.y0 == bounds.y0, c.rect.y1 == bounds.y1];
            }
        }

        let mut cells = Vec::new();
        for (vi, vc) in v.iter().enumerate() {
            for (hi, hc) in h.iter().enumerate() {
                if let Some(rect) = vc.rect.intersect(&hc.rect) {
                    cells.push(Cell { rect, h: hi, v: vi });
                }
            }
        }
        cells.sort_by(|a, b| pos_order(a.rect, b.rect));

        let mut edges = Vec::new();
        for (hi, _) in h.iter().enumerate() {
            let mut row: Vec<usize> = (0..cells.len()).filter(|&i| cells[i].h == hi).collect();
            row.sort_by(|&a, &b| cells[a].rect.x0.total_cmp(&cells[b].rect.x0));
            for w in row.windows(2) {
                edges.push(Edge {
                    a: w[0],
                    b: w[1],
                    axis: Axis::H,
                    channel: hi,
                });
            }
        }
        for (vi, _) in v.iter().enumerate() {
            let mut col: Vec<usize> = (0..cells.len()).filter(|&i| cells[i].v == vi).collect();
            col.sort_by(|&a, &b| cells[a].rect.y0.total_cmp(&cells[b].rect.y0));
            for w in col.windows(2) {
                edges.push(Edge {
                    a: w[0],
                    b: w[1],
                    axis: Axis::V,
                    channel: vi,
                });
            }
        }

        ChannelGraph { h, v, cells, edges }
    }
}

fn transpose(r: Rect) -> Rect {
    Rect::new(r.y0, r.x0, r.y1, r.x1)
}

/// Mark every wall stretch where two same-axis channels abut as soft on both.
/// Boundary coordinates come from the one sorted sweep-edge list, so equality
/// is exact.
fn soften(channels: &mut [Channel]) {
    let geom = |c: &Channel| match c.axis {
        Axis::V => (c.rect.x0, c.rect.x1, c.rect.y0, c.rect.y1),
        Axis::H => (c.rect.y0, c.rect.y1, c.rect.x0, c.rect.x1),
    };
    for i in 0..channels.len() {
        for j in 0..channels.len() {
            let (_, hi_wall, t0, t1) = geom(&channels[i]);
            let (lo_wall, _, s0, s1) = geom(&channels[j]);
            let (o0, o1) = (t0.max(s0), t1.min(s1));
            if i == j || hi_wall != lo_wall || o1 <= o0 {
                continue;
            }
            channels[i].soft[1].push((o0, o1));
            channels[j].soft[0].push((o0, o1));
        }
    }
    for c in channels {
        c.soft[0].sort_by(|a, b| a.0.total_cmp(&b.0));
        c.soft[1].sort_by(|a, b| a.0.total_cmp(&b.0));
    }
}

/// Reading order — the total tie-free order every channel/cell list uses.
fn pos_order(a: Rect, b: Rect) -> std::cmp::Ordering {
    a.x0.total_cmp(&b.x0).then(a.y0.total_cmp(&b.y0))
}

/// The x-sweep: strips between keep-out edges; per strip the free y-intervals;
/// identical intervals merge across contiguous strips into maximal rectangles.
fn sweep_channels(bounds: Rect, blocks: &[Rect], axis: Axis) -> Vec<Channel> {
    let mut xs = vec![bounds.x0, bounds.x1];
    for b in blocks {
        xs.push(b.x0);
        xs.push(b.x1);
    }
    xs.sort_by(f64::total_cmp);
    xs.dedup();

    // Channels still growing rightward: (y0, y1, x where they started).
    let mut open: Vec<(f64, f64, f64)> = Vec::new();
    let mut out = Vec::new();
    let close =
        |open: &mut Vec<(f64, f64, f64)>, frees: &[(f64, f64)], x: f64, out: &mut Vec<Channel>| {
            open.retain(|&(y0, y1, x0)| {
                let alive = frees.contains(&(y0, y1));
                if !alive {
                    out.push(Channel {
                        rect: Rect::new(x0, y0, x, y1),
                        axis,
                        soft: [Vec::new(), Vec::new()],
                        outer: [false, false],
                    });
                }
                alive
            });
        };

    for w in xs.windows(2) {
        let (s0, s1) = (w[0], w[1]);
        if s1 <= s0 {
            continue;
        }
        let mut spans: Vec<(f64, f64)> = blocks
            .iter()
            .filter(|b| b.x0 < s1 && b.x1 > s0)
            .map(|b| (b.y0, b.y1))
            .collect();
        let frees = free_intervals(bounds.y0, bounds.y1, &mut spans);
        close(&mut open, &frees, s0, &mut out);
        for &(y0, y1) in &frees {
            if !open.iter().any(|&(a, b, _)| (a, b) == (y0, y1)) {
                open.push((y0, y1, s0));
            }
        }
    }
    close(&mut open, &[], bounds.x1, &mut out);

    out.sort_by(|a, b| pos_order(a.rect, b.rect));
    out
}

/// The positive-width gaps of `[lo, hi]` not covered by `spans`.
fn free_intervals(lo: f64, hi: f64, spans: &mut [(f64, f64)]) -> Vec<(f64, f64)> {
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut frees = Vec::new();
    let mut cur = lo;
    for &(s0, s1) in spans.iter() {
        let (s0, s1) = (s0.max(lo), s1.min(hi));
        if s1 <= s0 {
            continue;
        }
        if s0 > cur {
            frees.push((cur, s0));
        }
        cur = cur.max(s1);
    }
    if hi > cur {
        frees.push((cur, hi));
    }
    frees
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel(axis: Axis, x0: f64, y0: f64, x1: f64, y1: f64) -> Channel {
        Channel {
            rect: Rect::new(x0, y0, x1, y1),
            axis,
            soft: [Vec::new(), Vec::new()],
            outer: [false, false],
        }
    }

    #[test]
    fn capacity_counts_wall_to_wall_runs() {
        let c = |w: f64| channel(Axis::V, 0.0, 0.0, w, 100.0);
        assert_eq!(c(24.0).capacity(8.0), 4);
        assert_eq!(c(9.0).capacity(8.0), 2);
        assert_eq!(c(7.9).capacity(8.0), 1);
        assert_eq!(c(16.0).capacity(8.0), 3);
    }

    #[test]
    fn abutting_same_axis_channels_soften_each_others_walls() {
        // The single-box ring: the left column's right wall faces the two
        // middle channels (above and below the box) across free space; the
        // stretch blocked by the box itself stays hard.
        let b = Rect::new(0.0, 0.0, 100.0, 100.0);
        let g = ChannelGraph::build(b, &[Rect::new(40.0, 40.0, 60.0, 60.0)], false);
        let left = &g.v[0];
        assert_eq!(left.rect, Rect::new(0.0, 0.0, 40.0, 100.0));
        assert_eq!(left.soft[0], Vec::new());
        assert_eq!(left.soft[1], vec![(0.0, 40.0), (60.0, 100.0)]);
        let top_mid = &g.v[1];
        assert_eq!(top_mid.rect, Rect::new(40.0, 0.0, 60.0, 40.0));
        assert_eq!(top_mid.soft[0], vec![(0.0, 40.0)]);
        assert_eq!(top_mid.soft[1], vec![(0.0, 40.0)]);
    }

    #[test]
    fn soft_margins_shrink_the_usable_range_only_near_the_span() {
        let mut chan = channel(Axis::V, 40.0, 0.0, 50.0, 100.0);
        chan.soft = [vec![(0.0, 40.0)], Vec::new()];
        // A span clear of the soft stretch (inflated by clearance) keeps the
        // full width; one within reach surrenders half a clearance.
        assert_eq!(chan.usable(50.0, 90.0, 8.0), (40.0, 50.0));
        assert_eq!(chan.usable(45.0, 90.0, 8.0), (44.0, 50.0));
        assert_eq!(chan.capacity_for(50.0, 90.0, 8.0), 2);
        assert_eq!(chan.capacity_for(45.0, 90.0, 8.0), 1);
        // Soft on both walls: a narrow channel closes outright.
        let mut sliver = channel(Axis::H, 0.0, 50.0, 100.0, 56.0);
        sliver.soft = [vec![(0.0, 100.0)], vec![(0.0, 100.0)]];
        assert_eq!(sliver.capacity_for(10.0, 20.0, 8.0), 0);
    }

    #[test]
    fn empty_scene_is_one_channel_per_axis_one_cell_no_edges() {
        let b = Rect::new(0.0, 0.0, 100.0, 100.0);
        let g = ChannelGraph::build(b, &[], false);
        assert_eq!(g.v, vec![channel(Axis::V, 0.0, 0.0, 100.0, 100.0)]);
        assert_eq!(g.h, vec![channel(Axis::H, 0.0, 0.0, 100.0, 100.0)]);
        assert_eq!(
            g.cells,
            vec![Cell {
                rect: b,
                h: 0,
                v: 0
            }]
        );
        assert_eq!(g.edges, Vec::new());
    }

    #[test]
    fn single_box_yields_the_ring() {
        let b = Rect::new(0.0, 0.0, 100.0, 100.0);
        let g = ChannelGraph::build(b, &[Rect::new(40.0, 40.0, 60.0, 60.0)], false);
        let rects = |cs: &[Channel]| cs.iter().map(|c| c.rect).collect::<Vec<_>>();
        assert_eq!(
            rects(&g.v),
            vec![
                Rect::new(0.0, 0.0, 40.0, 100.0),
                Rect::new(40.0, 0.0, 60.0, 40.0),
                Rect::new(40.0, 60.0, 60.0, 100.0),
                Rect::new(60.0, 0.0, 100.0, 100.0),
            ]
        );
        assert_eq!(
            rects(&g.h),
            vec![
                Rect::new(0.0, 0.0, 100.0, 40.0),
                Rect::new(0.0, 40.0, 40.0, 60.0),
                Rect::new(0.0, 60.0, 100.0, 100.0),
                Rect::new(60.0, 40.0, 100.0, 60.0),
            ]
        );
        // The ring: 4 corner cells + 4 side cells, connected in a cycle.
        assert_eq!(g.cells.len(), 8);
        assert_eq!(g.edges.len(), 8);
        let free: f64 = g.cells.iter().map(|c| c.rect.w() * c.rect.h()).sum();
        assert_eq!(free, 100.0 * 100.0 - 20.0 * 20.0);
    }

    #[test]
    fn between_channel_carries_the_gap_width() {
        let b = Rect::new(0.0, 0.0, 200.0, 100.0);
        let g = ChannelGraph::build(
            b,
            &[
                Rect::new(10.0, 10.0, 80.0, 90.0),
                Rect::new(104.0, 10.0, 180.0, 90.0),
            ],
            false,
        );
        let between =
            g.v.iter()
                .find(|c| c.rect == Rect::new(80.0, 0.0, 104.0, 100.0))
                .expect("between-channel exists");
        assert_eq!(between.width(), 24.0);
        assert_eq!(between.capacity(8.0), 4);
    }

    #[test]
    fn group_interior_decomposes_around_children() {
        let interior = Rect::new(0.0, 0.0, 120.0, 80.0);
        let g = ChannelGraph::build(
            interior,
            &[
                Rect::new(10.0, 10.0, 50.0, 70.0),
                Rect::new(70.0, 10.0, 110.0, 70.0),
            ],
            false,
        );
        assert!(
            g.v.iter()
                .any(|c| c.rect == Rect::new(50.0, 0.0, 70.0, 80.0))
        );
        // Free space partitions exactly.
        let free: f64 = g.cells.iter().map(|c| c.rect.w() * c.rect.h()).sum();
        assert_eq!(free, 120.0 * 80.0 - 2.0 * (40.0 * 60.0));
    }

    #[test]
    fn overlapping_keepouts_block_as_their_union() {
        let b = Rect::new(0.0, 0.0, 120.0, 100.0);
        let g = ChannelGraph::build(
            b,
            &[
                Rect::new(20.0, 0.0, 60.0, 100.0),
                Rect::new(50.0, 0.0, 90.0, 100.0),
            ],
            false,
        );
        assert_eq!(
            g.v,
            vec![
                channel(Axis::V, 0.0, 0.0, 20.0, 100.0),
                channel(Axis::V, 90.0, 0.0, 120.0, 100.0),
            ]
        );
        // Two disconnected side cells, no edges between them.
        assert_eq!(g.cells.len(), 2);
        assert_eq!(g.edges, Vec::new());
    }

    #[test]
    fn keepout_flush_to_bounds_leaves_no_sliver() {
        let b = Rect::new(0.0, 0.0, 100.0, 100.0);
        let g = ChannelGraph::build(b, &[Rect::new(0.0, 40.0, 50.0, 60.0)], false);
        assert!(g.v.iter().all(|c| c.rect.w() > 0.0));
        assert!(g.h.iter().all(|c| c.rect.h() > 0.0));
        assert!(g.cells.iter().all(|c| c.rect.w() > 0.0 && c.rect.h() > 0.0));
    }

    #[test]
    fn keepout_outside_bounds_is_clamped_away() {
        let b = Rect::new(0.0, 0.0, 100.0, 100.0);
        let g = ChannelGraph::build(b, &[Rect::new(-30.0, -30.0, -10.0, 200.0)], false);
        assert_eq!(g.v, vec![channel(Axis::V, 0.0, 0.0, 100.0, 100.0)]);
    }

    #[test]
    fn build_is_deterministic_across_100_runs() {
        let b = Rect::new(0.0, 0.0, 300.0, 200.0);
        let keepouts = [
            Rect::new(40.0, 40.0, 90.0, 90.0),
            Rect::new(120.0, 30.0, 180.0, 170.0),
            Rect::new(210.0, 80.0, 260.0, 140.0),
            Rect::new(60.0, 120.0, 110.0, 160.0),
        ];
        let first = ChannelGraph::build(b, &keepouts, false);
        for _ in 0..100 {
            assert_eq!(ChannelGraph::build(b, &keepouts, false), first);
        }
    }

    #[test]
    fn cells_abut_along_their_shared_channel() {
        let b = Rect::new(0.0, 0.0, 100.0, 100.0);
        let g = ChannelGraph::build(b, &[Rect::new(40.0, 40.0, 60.0, 60.0)], false);
        for e in &g.edges {
            let (a, b) = (&g.cells[e.a], &g.cells[e.b]);
            match e.axis {
                Axis::H => {
                    assert_eq!(a.h, b.h);
                    assert_eq!(e.channel, a.h);
                    assert_eq!(a.rect.x1, b.rect.x0, "H-neighbours touch in x");
                }
                Axis::V => {
                    assert_eq!(a.v, b.v);
                    assert_eq!(e.channel, a.v);
                    assert_eq!(a.rect.y1, b.rect.y0, "V-neighbours touch in y");
                }
            }
        }
    }
}

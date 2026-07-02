//! Channel graph — the free space between keep-outs, decomposed for routing
//! (ROUTING.md model step 2).
//!
//! Two independent sweeps: **V-channels** are maximal free rectangles for
//! vertical travel (x-strips between keep-out edges, equal free y-intervals
//! merged across strips); **H-channels** are the transpose. Both partition
//! the same free space, so every link run lives in exactly one channel of its
//! orientation. A **cell** is an H∩V overlap; cells are the graph's vertices,
//! and two cells connect iff they abut in a shared channel. Pure geometry: no
//! link knowledge, identical output for identical input — capacity and load
//! live in the [`super::ledger`].
//!
//! The sweep may slice one free corridor into several parallel channels — a
//! far-away node's edge cuts the strip list globally. A run cares about the
//! **corridor**: the void's true walls for its span, reassembled by walking
//! across shared boundaries into every same-axis channel free over the whole
//! span ([`ChannelGraph::corridor`]). Anchors, usable width, and capacity all
//! read the corridor, never the fragment — otherwise a fragment's midline
//! poses as "halfway between the nodes" and its phantom soft margins compress
//! pitch in a void with room to spare.

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
    /// The channel walls on the ordinate axis.
    pub fn walls(&self) -> (f64, f64) {
        match self.axis {
            Axis::V => (self.rect.x0, self.rect.x1),
            Axis::H => (self.rect.y0, self.rect.y1),
        }
    }

    /// The channel extent on the travel axis.
    pub fn travel(&self) -> (f64, f64) {
        match self.axis {
            Axis::V => (self.rect.y0, self.rect.y1),
            Axis::H => (self.rect.x0, self.rect.x1),
        }
    }
}

/// The void a run really lives in: the walls that bound its span once every
/// same-axis channel free over the whole span is walked across (ROUTING.md
/// model steps 2/5). One decision surface for three consumers — the anchor
/// preference, the usable ordinate range, and capacity — so a fragment of a
/// corridor can never pose as the corridor.
#[derive(Clone, Debug, PartialEq)]
pub struct Corridor {
    pub walls: (f64, f64),
    /// Whether each final wall is the root world's open canvas bound.
    pub outer: [bool; 2],
    /// Whether each final wall still faces free space somewhere over the
    /// span — a neighbour covering only part of it, whose wires the run owes
    /// half a clearance across the shared boundary.
    pub soft: [bool; 2],
    /// The same-axis channels the corridor absorbs — the committed-load set.
    pub chans: Vec<usize>,
}

impl Corridor {
    /// Where a lone run in this corridor sits (ROUTING.md model step 5): the
    /// midline between two keep-out walls — a bend between two nodes lands
    /// halfway between them — or hugging the keep-out wall when the other
    /// wall is the canvas edge. Placement's interior-run preference and the
    /// ledger's committed-ordinate estimate share this one anchor.
    pub fn anchor(&self) -> f64 {
        match self.outer {
            [false, true] => self.walls.0,
            [true, false] => self.walls.1,
            _ => (self.walls.0 + self.walls.1) / 2.0,
        }
    }

    /// The ordinate range runs may use: the corridor walls, pulled in by half
    /// a clearance where a wall still faces free space over the span — each
    /// side of a shared boundary surrenders half the separation it cannot
    /// guarantee alone.
    pub fn usable(&self, clearance: f64) -> (f64, f64) {
        let margin = |soft: bool| if soft { clearance / 2.0 } else { 0.0 };
        (
            self.walls.0 + margin(self.soft[0]),
            self.walls.1 - margin(self.soft[1]),
        )
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
    pub adj: Vec<Vec<(usize, Axis, usize)>>,
}

impl ChannelGraph {
    /// The corridor around channel `chan` of `axis` for a run spanning
    /// `[lo, hi]`: the walls grown across shared boundaries into every
    /// same-axis channel free over the whole span, out to the void's true
    /// walls. The span is clamped to the channel's own travel extent first —
    /// an end segment's tail inside its own endpoint's keep-out never blocks
    /// the walk. Wall coordinates come from one sweep-edge list, so the
    /// abutting test is exact equality, as in [`soften`].
    pub fn corridor(&self, axis: Axis, chan: usize, lo: f64, hi: f64) -> Corridor {
        let list = match axis {
            Axis::H => &self.h,
            Axis::V => &self.v,
        };
        let (t0, t1) = list[chan].travel();
        let (lo, hi) = (lo.max(t0).min(t1), hi.min(t1).max(t0));
        let covers = |c: &Channel| {
            let (a, b) = c.travel();
            a <= lo && hi <= b
        };
        let mut chans = vec![chan];
        let (mut low, mut high) = (chan, chan);
        while let Some(j) =
            (0..list.len()).find(|&j| list[j].walls().1 == list[low].walls().0 && covers(&list[j]))
        {
            low = j;
            chans.push(j);
        }
        while let Some(j) =
            (0..list.len()).find(|&j| list[j].walls().0 == list[high].walls().1 && covers(&list[j]))
        {
            high = j;
            chans.push(j);
        }
        chans.sort_unstable();
        let faced = |soft: &[(f64, f64)]| soft.iter().any(|&(a, b)| a < hi && b > lo);
        Corridor {
            walls: (list[low].walls().0, list[high].walls().1),
            outer: [list[low].outer[0], list[high].outer[1]],
            soft: [faced(&list[low].soft[0]), faced(&list[high].soft[1])],
            chans,
        }
    }

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

        let mut adj = vec![Vec::new(); cells.len()];
        for e in &edges {
            adj[e.a].push((e.b, e.axis, e.channel));
            adj[e.b].push((e.a, e.axis, e.channel));
        }

        ChannelGraph {
            h,
            v,
            cells,
            edges,
            adj,
        }
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

    /// Two full-height side walls with a small block at the top splitting
    /// the corridor's sweep into three V fragments.
    fn split_corridor() -> ChannelGraph {
        ChannelGraph::build(
            Rect::new(0.0, 0.0, 200.0, 100.0),
            &[
                Rect::new(0.0, 0.0, 40.0, 100.0),
                Rect::new(160.0, 0.0, 200.0, 100.0),
                Rect::new(90.0, 0.0, 110.0, 20.0),
            ],
            false,
        )
    }

    fn v_chan(g: &ChannelGraph, x0: f64, x1: f64) -> usize {
        g.v.iter()
            .position(|c| c.rect.x0 == x0 && c.rect.x1 == x1)
            .expect("V channel")
    }

    #[test]
    fn a_fragmented_corridor_reassembles_to_the_voids_walls() {
        let g = split_corridor();
        let west = v_chan(&g, 40.0, 90.0);
        // A span below the top block sees the whole void: hard wall to hard
        // wall, every fragment absorbed, midline anchor, no margins.
        let c = g.corridor(Axis::V, west, 30.0, 90.0);
        assert_eq!(c.walls, (40.0, 160.0));
        assert_eq!(c.soft, [false, false]);
        assert_eq!(c.anchor(), 100.0);
        assert_eq!(c.usable(8.0), (40.0, 160.0));
        assert_eq!(c.chans.len(), 3);
        // Growth works from any fragment of the void.
        let mid = v_chan(&g, 90.0, 110.0);
        assert_eq!(g.corridor(Axis::V, mid, 30.0, 90.0).walls, (40.0, 160.0));
    }

    #[test]
    fn a_partial_neighbour_stops_the_walk_and_stays_soft() {
        let g = split_corridor();
        let west = v_chan(&g, 40.0, 90.0);
        // The span reaches beside the top block: the middle fragment covers
        // only y ≥ 20, so the wall at 90 stands — still facing free space
        // below the block, where the run surrenders half a clearance.
        let c = g.corridor(Axis::V, west, 10.0, 90.0);
        assert_eq!(c.walls, (40.0, 90.0));
        assert_eq!(c.soft, [false, true]);
        assert_eq!(c.usable(8.0), (40.0, 86.0));
    }

    #[test]
    fn an_end_spans_keepout_tail_never_blocks_the_walk() {
        // The pcb shape: an east wall, two west boxes, an end run whose span
        // pokes into the east wall's keep-out (the lawful end-segment tail).
        // The walk clamps to the channel's travel extent, so the corridor
        // still opens to the whole west void.
        let g = ChannelGraph::build(
            Rect::new(0.0, 0.0, 200.0, 150.0),
            &[
                Rect::new(160.0, 0.0, 200.0, 150.0),
                Rect::new(0.0, 20.0, 40.0, 60.0),
                Rect::new(0.0, 90.0, 40.0, 130.0),
            ],
            false,
        );
        let row =
            g.h.iter()
                .position(|c| c.rect == Rect::new(40.0, 90.0, 160.0, 130.0))
                .expect("the lower row fragment");
        let c = g.corridor(Axis::H, row, 100.0, 170.0);
        assert_eq!(c.walls, (0.0, 150.0));
        assert_eq!(c.outer, [false, false]);
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
        // The gap between the two keep-outs is one V-channel, wall to wall.
        let between =
            g.v.iter()
                .find(|c| c.rect == Rect::new(80.0, 0.0, 104.0, 100.0))
                .expect("between-channel exists");
        assert_eq!(between.walls(), (80.0, 104.0));
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

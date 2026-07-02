//! The committed-state ledger (ROUTING.md model step 4) — what earlier links
//! already hold, asked three ways: track room in a channel span, crossings
//! over a band, free port slots on a side.
//!
//! Closure is *counting*, never simulation: a channel span holds
//! `floor(usable / min_pitch) + 1` tracks, its load is the maximum point-load
//! of committed runs over the span, and an edge is closed to a bundle of *k*
//! when fewer than *k* tracks remain. Capacity is never exceeded, only
//! priced — the search detours around a closed span or reports a stray.

// Scaffold: consumed by the pipeline driver too (ROUTING-V2.md stage 4);
// the allow leaves with it.
#![allow(dead_code)]

use std::collections::BTreeMap;

use super::cost::min_pitch;
use super::graph::{Axis, ChannelGraph};
use super::rect::Rect;
use crate::ast::Side;

/// One committed run: `k` parallel rails riding one channel span, estimated
/// at the channel's anchor ordinate — the same anchor placement prefers, so
/// the estimate and the drawn wire agree wherever the channel isn't crowded.
#[derive(Clone, Debug)]
struct Committed {
    span: (f64, f64),
    k: usize,
    ord: f64,
}

pub(crate) struct Ledger {
    clearance: f64,
    /// Committed runs per `(world, axis, channel)`.
    runs: BTreeMap<(usize, u8, usize), Vec<Committed>>,
    /// Landed port slots per `(node path, side)` — a fan group counts once.
    ports: BTreeMap<(String, u8), usize>,
}

impl Ledger {
    pub fn new(clearance: f64) -> Ledger {
        Ledger {
            clearance,
            runs: BTreeMap::new(),
            ports: BTreeMap::new(),
        }
    }

    /// Commit one run of a routed bundle: `k` rails over `span` in a channel.
    pub fn commit_run(
        &mut self,
        world: usize,
        axis: Axis,
        chan: usize,
        span: (f64, f64),
        k: usize,
        graph: &ChannelGraph,
    ) {
        let span = (span.0.min(span.1), span.0.max(span.1));
        let ord = match axis {
            Axis::H => graph.h[chan].anchor(),
            Axis::V => graph.v[chan].anchor(),
        };
        self.runs
            .entry((world, axis.index(), chan))
            .or_default()
            .push(Committed { span, k, ord });
    }

    /// Land `n` port slots on a side.
    pub fn commit_port(&mut self, path: &str, side: Side, n: usize) {
        *self
            .ports
            .entry((path.to_owned(), side.index()))
            .or_insert(0) += n;
    }

    /// Tracks still free over `span` of a channel at maximum compression:
    /// capacity `floor(usable / min_pitch) + 1` minus the committed maximum
    /// point-load. Spans count as concurrent within `min_pitch` of each other
    /// — near-touching runs need distinct tracks, exactly as placement will
    /// cluster them.
    pub fn tracks_left(
        &self,
        world: usize,
        axis: Axis,
        chan: usize,
        span: (f64, f64),
        graph: &ChannelGraph,
    ) -> usize {
        let (lo, hi) = (span.0.min(span.1), span.0.max(span.1));
        let channel = match axis {
            Axis::H => &graph.h[chan],
            Axis::V => &graph.v[chan],
        };
        let (u0, u1) = channel.usable(lo, hi, self.clearance);
        if u1 < u0 {
            return 0;
        }
        let capacity = ((u1 - u0) / min_pitch(self.clearance)).floor() as usize + 1;
        capacity.saturating_sub(self.max_load(world, axis, chan, (lo, hi)))
    }

    /// The maximum k-weighted number of committed runs concurrent at any
    /// point of `span`, runs reaching `min_pitch` past their ends.
    fn max_load(&self, world: usize, axis: Axis, chan: usize, span: (f64, f64)) -> usize {
        let Some(committed) = self.runs.get(&(world, axis.index(), chan)) else {
            return 0;
        };
        let reach = min_pitch(self.clearance);
        // Sweep events over the query span; at equal position ends retire
        // before starts, so a gap of exactly min_pitch shares a track.
        let mut events: Vec<(f64, i64)> = Vec::new();
        for c in committed {
            let lo = (c.span.0 - reach).max(span.0);
            let hi = (c.span.1 + reach).min(span.1);
            if hi <= lo {
                continue;
            }
            events.push((lo, c.k as i64));
            events.push((hi, -(c.k as i64)));
        }
        events.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let (mut load, mut max) = (0i64, 0i64);
        for (_, d) in events {
            load += d;
            max = max.max(load);
        }
        max as usize
    }

    /// The **certain** crossings of candidate travel along `axis`: committed
    /// perpendicular rails whose ordinate lies in the half-open `travel`
    /// interval and whose span **covers** the whole `covered` window — the
    /// candidate crosses them whatever track placement later picks. Estimates
    /// stay optimistic: an avoidable rail is never charged, and the exact
    /// count lands in the report once geometry is drawn. Half-open travel
    /// intervals let a route's consecutive pieces share endpoints without
    /// double-charging the rail sitting exactly on the joint.
    pub fn crossings_covering(
        &self,
        world: usize,
        axis: Axis,
        travel: (f64, f64),
        covered: (f64, f64),
    ) -> u32 {
        self.perpendicular(world, axis)
            .filter(|c| travel.0 <= c.ord && c.ord < travel.1)
            .filter(|c| c.span.0 <= covered.0 && c.span.1 >= covered.1)
            .map(|c| c.k as u32)
            .sum()
    }

    /// The **pinned** crossings of a stub-like piece along `axis`: committed
    /// perpendicular rails strictly inside the open `travel` interval whose
    /// span overlaps the piece's `window` of possible ordinates. Used only
    /// where the candidate has no freedom to dodge (the run into a port).
    pub fn crossings_overlapping(
        &self,
        world: usize,
        axis: Axis,
        travel: (f64, f64),
        window: (f64, f64),
    ) -> u32 {
        self.perpendicular(world, axis)
            .filter(|c| travel.0 < c.ord && c.ord < travel.1)
            .filter(|c| c.span.0 < window.1 && c.span.1 > window.0)
            .map(|c| c.k as u32)
            .sum()
    }

    /// Committed runs of the axis perpendicular to `axis` in `world`.
    fn perpendicular(&self, world: usize, axis: Axis) -> impl Iterator<Item = &Committed> {
        let other = match axis {
            Axis::H => Axis::V,
            Axis::V => Axis::H,
        };
        self.runs
            .range((world, other.index(), 0)..=(world, other.index(), usize::MAX))
            .flat_map(|(_, v)| v)
    }

    /// Free port slots on a side at maximum compression: the side minus a
    /// `clearance` corner margin each end holds `floor(window / min_pitch) + 1`
    /// ports (one always fits — a short side still takes its centre port),
    /// minus what already landed.
    pub fn side_free(&self, path: &str, side: Side, rect: Rect) -> usize {
        let len = match side {
            Side::Left | Side::Right => rect.h(),
            Side::Top | Side::Bottom => rect.w(),
        };
        let window = len - 2.0 * self.clearance;
        let capacity = if window < 0.0 {
            1
        } else {
            (window / min_pitch(self.clearance)).floor() as usize + 1
        };
        let landed = self
            .ports
            .get(&(path.to_owned(), side.index()))
            .copied()
            .unwrap_or(0);
        capacity.saturating_sub(landed)
    }
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

    /// One 24-wide V-channel between two keep-outs (walls hard at 80 and 104).
    fn gap_graph() -> ChannelGraph {
        ChannelGraph::build(
            BOUNDS,
            &[
                Rect::new(10.0, 10.0, 80.0, 90.0),
                Rect::new(104.0, 10.0, 180.0, 90.0),
            ],
            false,
        )
    }

    fn gap_chan(g: &ChannelGraph) -> usize {
        g.v.iter()
            .position(|c| c.rect == Rect::new(80.0, 0.0, 104.0, 100.0))
            .expect("between-channel")
    }

    #[test]
    fn capacity_is_counted_at_min_pitch() {
        // Width 24, clearance 8 → min pitch 4 → floor(24/4)+1 = 7 tracks.
        let g = gap_graph();
        let chan = gap_chan(&g);
        let ledger = Ledger::new(8.0);
        assert_eq!(ledger.tracks_left(0, Axis::V, chan, (20.0, 80.0), &g), 7);
    }

    #[test]
    fn overlapping_loads_stack_disjoint_loads_do_not() {
        let g = gap_graph();
        let chan = gap_chan(&g);
        let mut ledger = Ledger::new(8.0);
        // Two k=2 runs overlapping in span: peak load 4.
        ledger.commit_run(0, Axis::V, chan, (20.0, 60.0), 2, &g);
        ledger.commit_run(0, Axis::V, chan, (40.0, 80.0), 2, &g);
        assert_eq!(ledger.tracks_left(0, Axis::V, chan, (20.0, 80.0), &g), 3);
        // A run far below the query span adds nothing there.
        ledger.commit_run(0, Axis::V, chan, (90.0, 100.0), 3, &g);
        assert_eq!(ledger.tracks_left(0, Axis::V, chan, (20.0, 80.0), &g), 3);
    }

    #[test]
    fn near_touching_spans_are_concurrent_min_pitch_apart_is_not() {
        let g = gap_graph();
        let chan = gap_chan(&g);
        let mut ledger = Ledger::new(8.0);
        ledger.commit_run(0, Axis::V, chan, (20.0, 50.0), 1, &g);
        // Gap of 2 < min pitch 4: the two runs need distinct tracks.
        ledger.commit_run(0, Axis::V, chan, (52.0, 80.0), 1, &g);
        assert_eq!(ledger.tracks_left(0, Axis::V, chan, (20.0, 80.0), &g), 5);
        // Gap of exactly 2×min-pitch: the reaches touch, ends retire before
        // starts, so the two runs never stack — they may share a track.
        let mut spaced = Ledger::new(8.0);
        spaced.commit_run(0, Axis::V, chan, (20.0, 46.0), 1, &g);
        spaced.commit_run(0, Axis::V, chan, (54.0, 80.0), 1, &g);
        assert_eq!(spaced.tracks_left(0, Axis::V, chan, (20.0, 80.0), &g), 6);
    }

    #[test]
    fn crossings_charge_certain_rails_once_k_weighted() {
        let g = gap_graph();
        let chan = gap_chan(&g);
        let mut ledger = Ledger::new(8.0);
        // A 4-rail bundle riding the gap channel, estimated at its anchor
        // (both walls are keep-outs → the midline, x = 92).
        ledger.commit_run(0, Axis::V, chan, (30.0, 70.0), 4, &g);
        // H travel across the gap whose ordinate window the span covers:
        // certain, all 4 charged.
        assert_eq!(
            ledger.crossings_covering(0, Axis::H, (60.0, 140.0), (45.0, 55.0)),
            4
        );
        // A window the span does not fully cover is dodgeable: uncharged.
        assert_eq!(
            ledger.crossings_covering(0, Axis::H, (60.0, 140.0), (2.0, 55.0)),
            0
        );
        // Travel that stops short of the anchor never crosses; half-open
        // travel charges a rail sitting exactly on the interval's start once.
        assert_eq!(
            ledger.crossings_covering(0, Axis::H, (60.0, 92.0), (45.0, 55.0)),
            0
        );
        assert_eq!(
            ledger.crossings_covering(0, Axis::H, (92.0, 140.0), (45.0, 55.0)),
            4
        );
        // Same-axis runs are never crossings.
        assert_eq!(
            ledger.crossings_covering(0, Axis::V, (0.0, 100.0), (85.0, 95.0)),
            0
        );
        // A pinned stub: charged while its window overlaps the span, tangent
        // at the span edge is contact.
        assert_eq!(
            ledger.crossings_overlapping(0, Axis::H, (60.0, 140.0), (50.0, 50.0)),
            4
        );
        assert_eq!(
            ledger.crossings_overlapping(0, Axis::H, (60.0, 140.0), (5.0, 5.0)),
            0
        );
        assert_eq!(
            ledger.crossings_overlapping(0, Axis::H, (60.0, 140.0), (30.0, 30.0)),
            0
        );
    }

    #[test]
    fn side_capacity_compresses_to_min_pitch_and_fills_up() {
        let mut ledger = Ledger::new(8.0);
        let body = Rect::new(0.0, 0.0, 60.0, 40.0);
        // Right side: length 40 − 2·8 margins = 24 window → floor(24/4)+1 = 7.
        assert_eq!(ledger.side_free("a", Side::Right, body), 7);
        ledger.commit_port("a", Side::Right, 5);
        assert_eq!(ledger.side_free("a", Side::Right, body), 2);
        ledger.commit_port("a", Side::Right, 5);
        assert_eq!(ledger.side_free("a", Side::Right, body), 0);
        // Other sides and other nodes are untouched.
        assert_eq!(ledger.side_free("a", Side::Top, body), 12);
        assert_eq!(ledger.side_free("b", Side::Right, body), 7);
    }

    #[test]
    fn a_short_side_still_holds_one_port() {
        let ledger = Ledger::new(16.0);
        let tiny = Rect::new(0.0, 0.0, 20.0, 20.0);
        assert_eq!(ledger.side_free("a", Side::Right, tiny), 1);
    }
}

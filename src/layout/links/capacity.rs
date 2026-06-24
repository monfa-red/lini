//! Hard capacity bookkeeping and the shared placement core.
//!
//! Route-time closure is judged by **simulating the assignment**: the same rail
//! packing and cluster ordinates `runs::assign` draws with decide here whether a
//! span fits — so route time and drawing can never disagree about what fits.
//! [`Occupancy`] holds the committed spans
//! per channel, [`Ports`] the per-`(node, side)` slots. Capacity is binary
//! everywhere: a closed channel or full side means a detour, lanes are never
//! squeezed (LINKING §Model step 4).

use super::graph::{Axis, Channel};
use super::rect::Rect;
use super::runs::{Chain, EPS, Pin};
use crate::ast::Side;
use std::collections::BTreeMap;

/// `(world, axis, channel)` — how occupancy and assignment address a channel.
pub(super) type ChanKey = (usize, u8, usize);

/// One committed run in a channel: travel span, fan tag, the pinned
/// ordinate of port approaches and self-loop legs (provisional until ports
/// are placed; `None` for through-runs), and for port approaches the
/// `(node, side)` unit they land on — pins of one unit may stand nearer
/// than clearance when the side compacts (LINKING Law 2).
#[derive(Clone, Copy, Debug, PartialEq)]
struct SpanRec {
    lo: f64,
    hi: f64,
    fan: Option<usize>,
    pin: Option<f64>,
    unit: Option<u32>,
    margin: bool,
}

/// Committed channel occupancy, one record per run — identical same-fan
/// records collapse (the trunk is one drawn line). Closure is judged by
/// **simulating the assignment**: the within-clearance component a span
/// would chain is packed into rails and given ordinates exactly as
/// [`assign`] will place them, so route time and drawing can never disagree
/// about what fits.
pub struct Occupancy {
    map: BTreeMap<ChanKey, Vec<SpanRec>>,
    units: BTreeMap<(String, u8), u32>,
    clearance: f64,
}

/// A channel's verdict on a run joining it (see [`Occupancy::closure`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Closure {
    /// The run fits.
    Open,
    /// Closed for width alone: this many simulated rails leave the allowed
    /// band — the lane deficit gap growth can name.
    Short(usize),
    /// Closed for reasons no width fixes: a deny wall, or conflicting
    /// immovable pins.
    Hard,
}

impl Occupancy {
    pub fn new(clearance: f64) -> Occupancy {
        Occupancy {
            map: BTreeMap::new(),
            units: BTreeMap::new(),
            clearance,
        }
    }

    /// Dense id for a `(node, side)` port unit.
    fn unit_id(&mut self, path: &str, side: Side) -> u32 {
        let next = self.units.len() as u32;
        *self
            .units
            .entry((path.to_owned(), side.index()))
            .or_insert(next)
    }

    /// [`Occupancy::closure`] as the bare yes/no the closure tests assert on.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub fn closes(
        &self,
        channel: &Channel,
        world: usize,
        axis: Axis,
        chan: usize,
        lo: f64,
        hi: f64,
        k: usize,
        exempt: Option<usize>,
        relaxed: bool,
        deny: &[Rect],
    ) -> bool {
        self.closure(
            channel, world, axis, chan, lo, hi, k, exempt, relaxed, false, deny,
        ) != Closure::Open
    }

    /// The classified verdict on a run spanning `[lo, hi]` (`k` bundle rails
    /// of it) joining this channel. The chained component is packed into
    /// rails and the rail ordinates simulated; the channel is closed when
    /// any ordinate leaves the allowed range ([`Closure::Short`], counting
    /// the rails outside — the lane deficit), two conflicting members end up
    /// nearer than `clearance` (immovable pins), or a candidate rail lands
    /// inside a `deny` region it travels through (the separation audit
    /// walling a reroute away from a conflict). `relaxed` widens the range
    /// from the usable band to the walls — the rescue pass for links that
    /// would otherwise be impossible, gated on ground truth by the caller.
    /// Records tagged `exempt` are the asking bundle's own fan trunk and
    /// never block it.
    #[allow(clippy::too_many_arguments)]
    pub fn closure(
        &self,
        channel: &Channel,
        world: usize,
        axis: Axis,
        chan: usize,
        lo: f64,
        hi: f64,
        k: usize,
        exempt: Option<usize>,
        relaxed: bool,
        margin: bool,
        deny: &[Rect],
    ) -> Closure {
        let c = self.clearance;
        let Some(spans) = self.map.get(&(world, axis.index(), chan)) else {
            return empty_closure(channel, axis, lo, hi, k, relaxed, margin, deny, c);
        };
        let mut hull = (lo, hi);
        let mut joined = vec![false; spans.len()];
        loop {
            let mut grew = false;
            for (i, s) in spans.iter().enumerate() {
                if joined[i] || (s.fan.is_some() && s.fan == exempt) {
                    continue;
                }
                if s.lo < hull.1 + c && s.hi > hull.0 - c {
                    joined[i] = true;
                    hull = (hull.0.min(s.lo), hull.1.max(s.hi));
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }
        type Member = (f64, f64, Option<f64>, Option<u32>, bool);
        let mut members: Vec<Member> = spans
            .iter()
            .zip(&joined)
            .filter(|&(_, &j)| j)
            .map(|(s, _)| (s.lo, s.hi, s.pin, s.unit, s.margin))
            .collect();
        members.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        members.extend(std::iter::repeat_n((lo, hi, None, None, margin), k));
        let packed: Vec<(f64, f64, Option<f64>)> = members
            .iter()
            .map(|&(lo, hi, pin, ..)| (lo, hi, pin))
            .collect();
        let (at, pins) = pack(&packed, c);
        let (u0, u1) = channel.usable(hull.0, hull.1, c);
        let (mut b0, mut b1) = if relaxed { channel.walls() } else { (u0, u1) };
        // An open canvas wall holds any overflow in margin mode — the
        // otherwise-impossible lever may pitch lanes outward past the
        // bound. Everywhere else the margin stays finite, and overflowing
        // it is Hard, not Short: no gap grows the canvas, so growth must
        // never chase it.
        if margin {
            if channel.outer[0] {
                b0 = f64::NEG_INFINITY;
            }
            if channel.outer[1] {
                b1 = f64::INFINITY;
            }
        }
        let anchored = margin || members.iter().any(|m| m.4);
        let mid = cluster_midline(channel, u0, u1, pins.len(), c, anchored);
        let ords = cluster_ordinates(&pins, mid, c);
        let beyond = |o: f64, wall: usize| match wall {
            0 => o < b0 - EPS,
            _ => o > b1 + EPS,
        };
        let inner_out = ords
            .iter()
            .filter(|&&o| {
                (beyond(o, 0) && !channel.outer[0]) || (beyond(o, 1) && !channel.outer[1])
            })
            .count();
        if inner_out > 0 {
            return Closure::Short(inner_out);
        }
        if ords.iter().any(|&o| beyond(o, 0) || beyond(o, 1)) {
            return Closure::Hard;
        }
        let denied = at[members.len() - k..].iter().any(|&rail| {
            deny.iter().any(|d| {
                let ((d0, d1), (t0, t1)) = match axis {
                    Axis::H => ((d.y0, d.y1), (d.x0, d.x1)),
                    Axis::V => ((d.x0, d.x1), (d.y0, d.y1)),
                };
                lo < t1 && hi > t0 && ords[rail] > d0 && ords[rail] < d1
            })
        });
        if denied {
            return Closure::Hard;
        }
        // Pins of one port unit may stand nearer than clearance — that is a
        // compacted side's row (LINKING Law 2), not a conflict.
        let pinched = members
            .iter()
            .enumerate()
            .any(|(i, &(alo, ahi, _, ua, _))| {
                members[i + 1..].iter().zip(&at[i + 1..]).any(|(b, &rb)| {
                    at[i] != rb
                        && !(ua.is_some() && ua == b.3)
                        && alo < b.1 + c
                        && ahi > b.0 - c
                        && (ords[at[i]] - ords[rb]).abs() < c - EPS
                })
            });
        if pinched {
            Closure::Hard
        } else {
            Closure::Open
        }
    }

    /// Commit one run's span. An identical same-fan record is the trunk
    /// drawn again — it occupies the one lane it already holds.
    #[allow(clippy::too_many_arguments)]
    pub fn commit(
        &mut self,
        world: usize,
        axis: Axis,
        chan: usize,
        lo: f64,
        hi: f64,
        fan: Option<usize>,
        pin: Option<f64>,
        unit: Option<u32>,
        margin: bool,
    ) {
        if hi < lo {
            return;
        }
        let rec = SpanRec {
            lo,
            hi,
            fan,
            pin,
            unit,
            margin,
        };
        let spans = self.map.entry((world, axis.index(), chan)).or_default();
        if fan.is_some() && spans.contains(&rec) {
            return;
        }
        spans.push(rec);
    }

    /// Commit every run of one chain, tagged with its fan group (if any);
    /// port approaches carry their `(node, side)` unit.
    pub fn commit_chain(&mut self, chain: &Chain) {
        let fan = chain.ends[0].fan.or(chain.ends[1].fan);
        for r in &chain.runs {
            let (lo, hi) = r.span();
            let (pin, unit) = match r.pin {
                Pin::Free => (None, None),
                Pin::Fixed(v) => (Some(v), None),
                Pin::Port(e) => {
                    let end = &chain.ends[e];
                    let unit = self.unit_id(&end.path, end.side);
                    (Some(r.ord), Some(unit))
                }
            };
            self.commit(
                chain.world,
                r.axis,
                r.chan,
                lo,
                hi,
                fan,
                pin,
                unit,
                chain.margin,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn empty_closure(
    channel: &Channel,
    axis: Axis,
    lo: f64,
    hi: f64,
    k: usize,
    relaxed: bool,
    margin: bool,
    deny: &[Rect],
    clearance: f64,
) -> Closure {
    if k == 0 {
        return Closure::Open;
    }
    let (u0, u1) = channel.usable(lo, hi, clearance);
    let (mut b0, mut b1) = if relaxed { channel.walls() } else { (u0, u1) };
    if margin {
        if channel.outer[0] {
            b0 = f64::NEG_INFINITY;
        }
        if channel.outer[1] {
            b1 = f64::INFINITY;
        }
    }
    let mid = cluster_midline(channel, u0, u1, k, clearance, margin);
    let ord = |i: usize| mid + (i as f64 - (k as f64 - 1.0) / 2.0) * clearance;
    let beyond = |o: f64, wall: usize| match wall {
        0 => o < b0 - EPS,
        _ => o > b1 + EPS,
    };
    let mut inner_out = 0;
    let mut any_out = false;
    for i in 0..k {
        let o = ord(i);
        let low = beyond(o, 0);
        let high = beyond(o, 1);
        inner_out += usize::from((low && !channel.outer[0]) || (high && !channel.outer[1]));
        any_out |= low || high;
    }
    if inner_out > 0 {
        return Closure::Short(inner_out);
    }
    if any_out {
        return Closure::Hard;
    }
    let denied = (0..k).any(|i| {
        let o = ord(i);
        deny.iter().any(|d| {
            let ((d0, d1), (t0, t1)) = match axis {
                Axis::H => ((d.y0, d.y1), (d.x0, d.x1)),
                Axis::V => ((d.x0, d.x1), (d.y0, d.y1)),
            };
            lo < t1 && hi > t0 && o > d0 && o < d1
        })
    });
    if denied { Closure::Hard } else { Closure::Open }
}

/// First-fit rail packing — the channel router's left-edge discipline.
/// Members are `(lo, hi, pin)` in placement order; each takes the first
/// rail that (a) sits at or above every rail holding an earlier member
/// whose span strictly overlaps — overlapping runs keep their given order,
/// nested, never braided; (b) it clears by ≥ `clearance` everywhere along
/// the channel; and (c) is not anchored to a different pin. Same-pin
/// members always share a rail: they are one drawn line. Returns each
/// member's rail and the rails' pins, in rail order.
pub(super) fn pack(
    members: &[(f64, f64, Option<f64>)],
    clearance: f64,
) -> (Vec<usize>, Vec<Option<f64>>) {
    struct Rail {
        pin: Option<f64>,
        spans: Vec<(f64, f64)>,
    }
    let mut rails: Vec<Rail> = Vec::new();
    let mut at: Vec<usize> = Vec::with_capacity(members.len());
    for (i, &(lo, hi, pin)) in members.iter().enumerate() {
        let same_pin = pin.and_then(|p| rails.iter().position(|r| r.pin == Some(p)));
        let slot = same_pin.or_else(|| {
            let floor = members[..i]
                .iter()
                .zip(&at)
                .filter(|&(&(l, h, _), _)| l < hi && h > lo)
                .map(|(_, &r)| r + 1)
                .max()
                .unwrap_or(0);
            (floor..rails.len()).find(|&r| {
                let pin_free = rails[r].pin.is_none() || pin.is_none();
                pin_free
                    && rails[r]
                        .spans
                        .iter()
                        .all(|&(a, b)| lo >= b + clearance || hi <= a - clearance)
            })
        });
        let r = slot.unwrap_or_else(|| {
            rails.push(Rail {
                pin: None,
                spans: Vec::new(),
            });
            rails.len() - 1
        });
        if pin.is_some() {
            rails[r].pin = pin;
        }
        rails[r].spans.push((lo, hi));
        at.push(r);
    }
    (at, rails.iter().map(|r| r.pin).collect())
}

/// Port slots per `(node, side)` — Law 2's side capacity, shared by outer
/// ends, inner (containment) ends, and self-loop legs alike.
pub struct Ports {
    used: BTreeMap<String, [usize; 4]>,
    clearance: f64,
}

/// The extent of a side along its own axis.
pub(super) fn side_len(rect: Rect, side: Side) -> f64 {
    match side {
        Side::Left | Side::Right => rect.h(),
        Side::Top | Side::Bottom => rect.w(),
    }
}

/// Law 2's side capacity: `floor((len − 2·clearance) / clearance) + 1`,
/// minimum 1 (LINKING step 4).
pub fn side_capacity(rect: Rect, side: Side, clearance: f64) -> usize {
    let free = side_len(rect, side) - 2.0 * clearance;
    if free < 0.0 {
        1
    } else {
        (free / clearance).floor() as usize + 1
    }
}

impl Ports {
    pub fn new(clearance: f64) -> Ports {
        Ports {
            used: BTreeMap::new(),
            clearance,
        }
    }

    pub fn capacity(&self, rect: Rect, side: Side) -> usize {
        side_capacity(rect, side, self.clearance)
    }

    pub fn free(&self, path: &str, side: Side, rect: Rect) -> usize {
        let used = self
            .used
            .get(path)
            .map_or(0, |sides| sides[side.index() as usize]);
        self.capacity(rect, side).saturating_sub(used)
    }

    pub fn commit(&mut self, path: &str, side: Side, n: usize) {
        self.used.entry(path.to_owned()).or_default()[side.index() as usize] += n;
    }
}

/// A pinless cluster's midline: centred in the channel while it fits — and
/// anchored at the inner wall when it overflows one whose opposite wall is
/// the open canvas bound, so the overflow pitches outward into the margin
/// and everything that fits draws exactly as it always did.
pub(super) fn cluster_midline(
    channel: &Channel,
    u0: f64,
    u1: f64,
    n: usize,
    clearance: f64,
    anchored: bool,
) -> f64 {
    let half = (n as f64 - 1.0) / 2.0 * clearance;
    if !anchored || 2.0 * half <= u1 - u0 + EPS {
        return (u0 + u1) / 2.0;
    }
    match channel.outer {
        [false, true] => u0 + half,
        [true, false] => u1 - half,
        _ => (u0 + u1) / 2.0,
    }
}

/// Ordinates for one ordered overlap cluster: pins stay, runs between two pins
/// spread evenly, runs outside the pinned range step outward at `clearance`
/// pitch, and a pinless cluster sits centred on the channel midline.
pub(super) fn cluster_ordinates(pins: &[Option<f64>], midline: f64, clearance: f64) -> Vec<f64> {
    let n = pins.len();
    let mut ords = vec![0.0; n];
    let pin_pos: Vec<usize> = (0..n).filter(|&i| pins[i].is_some()).collect();
    if pin_pos.is_empty() {
        for (i, o) in ords.iter_mut().enumerate() {
            *o = midline + (i as f64 - (n as f64 - 1.0) / 2.0) * clearance;
        }
        return ords;
    }
    for &i in &pin_pos {
        ords[i] = pins[i].unwrap();
    }
    let first = pin_pos[0];
    let last = *pin_pos.last().unwrap();
    for i in (0..first).rev() {
        ords[i] = ords[i + 1] - clearance;
    }
    for i in last + 1..n {
        ords[i] = ords[i - 1] + clearance;
    }
    for w in pin_pos.windows(2) {
        let (a, b) = (w[0], w[1]);
        let gap = (ords[b] - ords[a]) / (b - a) as f64;
        for i in a + 1..b {
            ords[i] = ords[a] + gap * (i - a) as f64;
        }
    }
    ords
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A V-channel whose walls sit at `w0`/`w1`, no soft stretches.
    fn vchan(w0: f64, w1: f64) -> Channel {
        Channel {
            rect: Rect::new(w0, 0.0, w1, 1000.0),
            axis: Axis::V,
            soft: [Vec::new(), Vec::new()],
            outer: [false, false],
        }
    }

    #[test]
    fn runs_clear_along_the_channel_share_a_lane() {
        let mut occ = Occupancy::new(8.0);
        occ.commit(0, Axis::V, 0, 0.0, 10.0, None, None, None, false);
        occ.commit(0, Axis::V, 0, 50.0, 60.0, None, None, None, false);
        // A bridging run chains all three, but the committed pair clears
        // itself by ≥ clearance and shares a lane: two rails fit two lanes.
        let two_lanes = vchan(0.0, 8.0);
        assert!(!occ.closes(&two_lanes, 0, Axis::V, 0, 8.0, 52.0, 1, None, false, &[]));
        // A second bridging run needs a third lane: closed.
        occ.commit(0, Axis::V, 0, 8.0, 52.0, None, None, None, false);
        assert!(occ.closes(&two_lanes, 0, Axis::V, 0, 12.0, 48.0, 1, None, false, &[]));
    }

    #[test]
    fn mutually_close_runs_need_distinct_lanes() {
        let mut occ = Occupancy::new(8.0);
        occ.commit(0, Axis::V, 0, 0.0, 10.0, None, None, None, false);
        occ.commit(0, Axis::V, 0, 5.0, 15.0, None, None, None, false);
        assert!(occ.closes(
            &vchan(0.0, 8.0),
            0,
            Axis::V,
            0,
            8.0,
            12.0,
            1,
            None,
            false,
            &[]
        ));
        assert!(!occ.closes(
            &vchan(0.0, 16.0),
            0,
            Axis::V,
            0,
            8.0,
            12.0,
            1,
            None,
            false,
            &[]
        ));
    }

    #[test]
    fn degenerate_jog_spans_still_count() {
        let mut occ = Occupancy::new(8.0);
        occ.commit(0, Axis::V, 0, 0.0, 0.0, None, None, None, false);
        assert!(occ.closes(
            &vchan(0.0, 4.0),
            0,
            Axis::V,
            0,
            5.0,
            5.0,
            1,
            None,
            false,
            &[]
        ));
        assert!(!occ.closes(
            &vchan(0.0, 8.0),
            0,
            Axis::V,
            0,
            5.0,
            5.0,
            1,
            None,
            false,
            &[]
        ));
    }

    #[test]
    fn close_pins_close_the_channel() {
        let mut occ = Occupancy::new(8.0);
        occ.commit(0, Axis::V, 0, 0.0, 10.0, None, Some(2.0), None, false);
        // One pin and a free run: the free rail steps a clearance out.
        assert!(!occ.closes(
            &vchan(0.0, 16.0),
            0,
            Axis::V,
            0,
            5.0,
            12.0,
            1,
            None,
            false,
            &[]
        ));
        // Two overlapping pins only 4 apart can never separate: closed for
        // anything that would chain to them.
        occ.commit(0, Axis::V, 0, 5.0, 12.0, None, Some(6.0), None, false);
        assert!(occ.closes(
            &vchan(0.0, 16.0),
            0,
            Axis::V,
            0,
            3.0,
            9.0,
            1,
            None,
            false,
            &[]
        ));
    }

    #[test]
    fn fan_trunk_counts_once_and_never_blocks_itself() {
        let mut occ = Occupancy::new(8.0);
        occ.commit(0, Axis::H, 2, 0.0, 50.0, Some(7), Some(5.0), None, false);
        occ.commit(0, Axis::H, 2, 0.0, 50.0, Some(7), Some(5.0), None, false);
        occ.commit(0, Axis::H, 2, 30.0, 80.0, Some(7), Some(5.0), None, false);
        let hchan = |w0: f64, w1: f64| Channel {
            rect: Rect::new(0.0, w0, 1000.0, w1),
            axis: Axis::H,
            soft: [Vec::new(), Vec::new()],
            outer: [false, false],
        };
        // The trunk never blocks its own bundle…
        assert!(!occ.closes(
            &hchan(0.0, 8.0),
            0,
            Axis::H,
            2,
            0.0,
            80.0,
            1,
            Some(7),
            false,
            &[]
        ));
        // …and is one line (one shared-pin rail) for everyone else.
        assert!(!occ.closes(
            &hchan(0.0, 16.0),
            0,
            Axis::H,
            2,
            0.0,
            80.0,
            1,
            None,
            false,
            &[]
        ));
        assert!(occ.closes(
            &hchan(0.0, 16.0),
            0,
            Axis::H,
            2,
            0.0,
            80.0,
            2,
            None,
            false,
            &[]
        ));
    }

    #[test]
    fn pack_shares_lanes_and_keeps_overlapping_members_ordered() {
        // a and c clear everything in rail 0 by ≥ clearance and share it;
        // b overlaps a and must take a later rail.
        let (at, pins) = pack(
            &[(0.0, 10.0, None), (8.0, 52.0, None), (60.0, 70.0, None)],
            8.0,
        );
        assert_eq!(at, vec![0, 1, 0]);
        assert_eq!(pins, vec![None, None]);
    }

    #[test]
    fn pack_anchors_same_pin_members_to_one_rail() {
        let (at, pins) = pack(
            &[
                (0.0, 50.0, Some(5.0)),
                (30.0, 80.0, Some(5.0)),
                (20.0, 40.0, None),
            ],
            8.0,
        );
        assert_eq!(at, vec![0, 0, 1]);
        assert_eq!(pins, vec![Some(5.0), None]);
    }

    #[test]
    fn port_capacity_floors_at_one() {
        let ports = Ports::new(8.0);
        let r = Rect::new(0.0, 0.0, 60.0, 10.0);
        assert_eq!(ports.capacity(r, Side::Top), 6);
        assert_eq!(ports.capacity(r, Side::Right), 1);
    }

    #[test]
    fn port_slots_run_out() {
        let mut ports = Ports::new(8.0);
        let r = Rect::new(0.0, 0.0, 30.0, 30.0);
        assert_eq!(ports.capacity(r, Side::Left), 2);
        assert_eq!(ports.free("a", Side::Left, r), 2);
        ports.commit("a", Side::Left, 2);
        assert_eq!(ports.free("a", Side::Left, r), 0);
        assert_eq!(ports.free("a", Side::Right, r), 2);
    }

    #[test]
    fn pinless_cluster_centres_at_clearance_pitch() {
        let ords = cluster_ordinates(&[None, None, None], 50.0, 8.0);
        assert_eq!(ords, vec![42.0, 50.0, 58.0]);
    }

    #[test]
    fn pins_anchor_and_frees_step_outward() {
        let ords = cluster_ordinates(&[None, Some(40.0), None, None], 99.0, 8.0);
        assert_eq!(ords, vec![32.0, 40.0, 48.0, 56.0]);
    }

    #[test]
    fn frees_between_two_pins_spread_evenly() {
        let ords = cluster_ordinates(&[Some(10.0), None, None, Some(40.0)], 0.0, 8.0);
        assert_eq!(ords, vec![10.0, 20.0, 30.0, 40.0]);
    }

    #[test]
    fn pins_at_extremes_hold_their_ordinates() {
        let ords = cluster_ordinates(&[Some(0.0), None, Some(16.0)], 100.0, 8.0);
        assert_eq!(ords, vec![0.0, 8.0, 16.0]);
    }
}

//! Per-channel run assignment — the global stage (ROUTING §Model step 5).
//!
//! A routed link is a **chain** of channel runs. This module owns port
//! placement and the ordinate assignment that mixes port-pinned approach
//! runs with through-runs at `clearance` pitch, over all chains at once.
//! Pairwise ordering lives in [`super::order`]; capacity bookkeeping and the
//! shared placement core in [`super::capacity`]; chains are built in
//! [`super::geometry`].

use super::capacity::{ChanKey, cluster_midline, cluster_ordinates, pack, side_capacity, side_len};
use super::graph::{Axis, Channel, ChannelGraph};
use super::order;
use super::rect::Rect;
use crate::ast::Side;
use std::collections::BTreeMap;

pub(super) const EPS: f64 = 1e-6;

/// One routing world: the interior of a container (`""` = the scene root) and
/// its channel decomposition. Every chain lives in exactly one world.
pub struct World {
    pub path: String,
    pub graph: ChannelGraph,
}

/// How a run's ordinate is decided.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pin {
    /// A through-run: the assignment picks the ordinate.
    Free,
    /// Final approach into the port at this chain end (0 = start, 1 = goal):
    /// the ordinate is the port's, set when ports are placed.
    Port(usize),
    /// Structurally fixed: self-loop legs on the keep-out boundary, and a
    /// straight-shot chain's middle runs riding its ports' shared ordinate.
    Fixed(f64),
}

/// A run's connection at one chain end.
#[derive(Clone, Copy, Debug)]
pub enum Conn {
    /// Turns into the neighbouring run inside this cell.
    Junction { cell: usize, q: f64 },
    /// The run ends here (stub tip / port approach).
    Terminal { q: f64 },
}

impl Conn {
    pub fn q(&self) -> f64 {
        match self {
            Conn::Junction { q, .. } | Conn::Terminal { q } => *q,
        }
    }
}

/// One channel run: travel along `axis` in channel `chan` of the chain's
/// world, at the perpendicular ordinate `ord`.
#[derive(Clone, Debug)]
pub struct Run {
    pub axis: Axis,
    pub chan: usize,
    pub pin: Pin,
    /// Assigned ordinate; meaningful after [`assign`].
    pub ord: f64,
    /// Connections in chain order: `[toward start, toward goal]`.
    pub conn: [Conn; 2],
}

impl Run {
    pub fn span(&self) -> (f64, f64) {
        let (a, b) = (self.conn[0].q(), self.conn[1].q());
        (a.min(b), a.max(b))
    }
}

/// One chain end: which side of which body it lands on.
#[derive(Clone, Debug)]
pub struct EndInfo {
    pub path: String,
    pub side: Side,
    pub rect: Rect,
    /// Port point on the side; provisional (side centre) until [`assign`].
    pub port: (f64, f64),
    pub fan: Option<usize>,
}

/// A routed link: its world, its runs in start→goal order, and its two ends.
#[derive(Clone)]
pub struct Chain {
    pub world: usize,
    pub runs: Vec<Run>,
    pub ends: [EndInfo; 2],
    /// Declaration rank of the request — the final tie-break.
    pub req: usize,
    /// Routed in margin mode (the otherwise-impossible lever): this chain's
    /// overflow in an open outer channel pitches outward past the canvas
    /// bound rather than centring over the scene.
    pub margin: bool,
}

/// Port-group slides per `(node, side)`: Law 2 lets a whole group slide
/// along its side, spacing and order intact, when the centred rows would
/// break Law 1 (ROUTING §Ports). Owned by the routing pass; the separation
/// audit proposes entries, ground truth accepts them.
pub type Slides = BTreeMap<(String, u8), f64>;

/// Assign every ordinate: ports first (evenly spaced at `clearance` pitch,
/// median on the side centre plus any accepted slide, comparator-ordered),
/// then per-channel runs (pins fixed, through-runs fitted around them ≥
/// `clearance` apart, leftover slack centred). A pair that the assignment
/// actually makes cross and whose end orders conflict is an inversion: its
/// swap is realised mid-channel and ordinates run once more — then judged
/// on the drawn truth: a swap whose jog crosses nothing reverts. Chains are
/// complete except for ordinates when this runs.
pub fn assign(worlds: &[World], chains: &mut [Option<Chain>], clearance: f64, slides: &Slides) {
    place_ports(chains, clearance, slides);
    align_straight_chains(worlds, chains, clearance, slides);
    snap_junctions(chains);
    assign_ordinates(worlds, chains, clearance);
    let mut swaps = split_inversions(worlds, chains);
    if !swaps.is_empty() {
        assign_ordinates(worlds, chains, clearance);
        while revert_failed_swaps(chains, &mut swaps) {
            assign_ordinates(worlds, chains, clearance);
        }
    }
}

/// Law 2's lone-port freedom (ROUTING §Ports): a chain whose runs all share
/// one axis is a **straight shot** — one drawn line, its lateral position
/// owned by its two ports. When the port ordinates differ, the goal end
/// re-pins to the start's ordinate (or the start to the goal's): an end may
/// move iff its side holds no other port, it is not a fan trunk, no
/// accepted slide owns the side, the target keeps the corner margins, and
/// the line stays inside every run's channel. With the ports agreed, every
/// middle run fixes on the shared ordinate — the pair of stub-side turns
/// that paid for centre misalignment never forms. Ends that cannot move
/// keep the centred jog.
fn align_straight_chains(
    worlds: &[World],
    chains: &mut [Option<Chain>],
    clearance: f64,
    slides: &Slides,
) {
    let mut units: BTreeMap<(String, u8), usize> = BTreeMap::new();
    let mut fans_seen: std::collections::BTreeSet<(String, u8, usize)> = Default::default();
    for chain in chains.iter().flatten() {
        for e in &chain.ends {
            let key = (e.path.clone(), e.side.index());
            match e.fan {
                Some(g) => {
                    if fans_seen.insert((key.0.clone(), key.1, g)) {
                        *units.entry(key).or_insert(0) += 1;
                    }
                }
                None => *units.entry(key).or_insert(0) += 1,
            }
        }
    }

    for chain in chains.iter_mut().flatten() {
        let Some(axis) = chain
            .runs
            .iter()
            .find(|r| matches!(r.pin, Pin::Port(_)))
            .map(|r| r.axis)
        else {
            continue;
        };
        // Straight modulo jogs: every run rides the travel axis, except
        // free perpendicular jog runs — those collapse to zero width once
        // the ports agree (route-time straight-shot jogs bridge the two
        // provisional side centres).
        let straight = chain.runs.iter().all(|r| {
            (r.axis == axis && !matches!(r.pin, Pin::Fixed(_)))
                || (r.axis != axis && r.pin == Pin::Free)
        });
        if !straight {
            continue;
        }
        let ord = |p: (f64, f64)| if axis == Axis::V { p.0 } else { p.1 };
        let o = [ord(chain.ends[0].port), ord(chain.ends[1].port)];
        let graph = &worlds[chain.world].graph;
        // The straightened line must stay inside every travel channel's
        // walls, and cross every jog's cell inside that channel's own free
        // extent — clearance against bodies is the corridor's by
        // construction, never re-judged here.
        let in_channels = |t: f64| {
            chain.runs.iter().all(|r| {
                let ch = match r.axis {
                    Axis::H => &graph.h[r.chan],
                    Axis::V => &graph.v[r.chan],
                };
                let (lo, hi) = if r.axis == axis {
                    ch.walls()
                } else {
                    match r.axis {
                        Axis::H => (ch.rect.x0, ch.rect.x1),
                        Axis::V => (ch.rect.y0, ch.rect.y1),
                    }
                };
                t >= lo - EPS && t <= hi + EPS
            })
        };
        let movable = |e: &EndInfo, t: f64| {
            if e.fan.is_some()
                || slides.contains_key(&(e.path.clone(), e.side.index()))
                || units.get(&(e.path.clone(), e.side.index())) != Some(&1)
            {
                return false;
            }
            let (lo, hi) = match e.side {
                Side::Top | Side::Bottom => (e.rect.x0, e.rect.x1),
                Side::Left | Side::Right => (e.rect.y0, e.rect.y1),
            };
            t >= lo + clearance - EPS && t <= hi - clearance + EPS
        };
        let aligned = (o[0] - o[1]).abs() <= EPS;
        let t = if aligned || (movable(&chain.ends[1], o[0]) && in_channels(o[0])) {
            o[0]
        } else if movable(&chain.ends[0], o[1]) && in_channels(o[1]) {
            o[1]
        } else {
            continue;
        };
        for e in &mut chain.ends {
            e.port = match axis {
                Axis::V => (t, e.port.1),
                Axis::H => (e.port.0, t),
            };
        }
        for r in &mut chain.runs {
            if r.axis != axis {
                continue; // a jog: its width collapses via the snapped conns
            }
            if r.pin == Pin::Free {
                r.pin = Pin::Fixed(t);
            }
            r.ord = t;
        }
    }
}

/// One realised inversion: the split chain, its jog's run index, and the
/// partner chain the swap exists to cross.
struct Swap {
    ci: usize,
    jog: usize,
    partner: usize,
}

/// The realisation's own ground-truth gate (ROUTING §Model step 5: "the
/// halves flank the partner"). Ordinates settle only after the split, and
/// the assignment may land both halves on one side of the partner's run —
/// a swap that cannot cross it: two turns and an extra lane that buy no
/// crossing (the pair's crossing, if any, is drawn elsewhere), and at
/// worst a chain crossing itself. Every such swap merges back into one
/// run, all of a pass's failures in one batch — the caller re-assigns and
/// re-checks until stable. A trio mangled by a nested split is left
/// alone, conservatively.
fn revert_failed_swaps(chains: &mut [Option<Chain>], swaps: &mut Vec<Swap>) -> bool {
    let mut any = false;
    loop {
        let failed = swaps.iter().position(|s| {
            let Some(chain) = chains[s.ci].as_ref() else {
                return false;
            };
            if s.jog == 0 || s.jog + 1 >= chain.runs.len() {
                return false;
            }
            let (h, j, t) = (
                &chain.runs[s.jog - 1],
                &chain.runs[s.jog],
                &chain.runs[s.jog + 1],
            );
            if h.axis != t.axis || h.chan != t.chan || j.axis == h.axis {
                return false;
            }
            let flanked = chains[s.partner].as_ref().is_some_and(|p| {
                p.runs
                    .iter()
                    .filter(|r| r.axis == h.axis && r.chan == h.chan)
                    .any(|r| {
                        let (lo, hi) = r.span();
                        lo <= j.ord && j.ord <= hi && (h.ord - r.ord) * (t.ord - r.ord) < 0.0
                    })
            });
            !flanked
        });
        let Some(k) = failed else {
            return any;
        };
        any = true;
        let Swap { ci, jog, .. } = swaps.remove(k);
        let chain = chains[ci].as_mut().unwrap();
        let tail = chain.runs.remove(jog + 1);
        chain.runs.remove(jog);
        let head = &mut chain.runs[jog - 1];
        head.conn[1] = tail.conn[1];
        if head.pin == Pin::Free {
            head.pin = tail.pin;
            head.ord = tail.ord;
        }
        for s in swaps.iter_mut() {
            if s.ci == ci && s.jog > jog {
                s.jog -= 2;
            }
        }
    }
}

/// All permutations of `0..n` in lexicographic order, identity first.
fn permutations(n: usize) -> Vec<Vec<usize>> {
    fn build(prefix: &mut Vec<usize>, rest: &[usize], out: &mut Vec<Vec<usize>>) {
        if rest.is_empty() {
            out.push(prefix.clone());
            return;
        }
        for (i, &x) in rest.iter().enumerate() {
            prefix.push(x);
            let mut rem = rest.to_vec();
            rem.remove(i);
            build(prefix, &rem, out);
            prefix.pop();
        }
    }
    let mut out = Vec::new();
    build(&mut Vec::new(), &(0..n).collect::<Vec<_>>(), &mut out);
    out
}

/// The per-channel ordinate pass over the current run set.
fn assign_ordinates(worlds: &[World], chains: &mut [Option<Chain>], clearance: f64) {
    for ((world, axis, chan), mut runs) in channel_map(chains) {
        runs.sort_by(|&a, &b| order::cmp_runs(chains, a, b));
        let g = &worlds[world].graph;
        let channel = if axis == 0 { &g.h[chan] } else { &g.v[chan] };
        assign_channel(chains, &runs, channel, clearance);
    }
}

/// Re-snap junction span-ends to their pinned neighbours' ordinates.
/// `geometry::chain` snaps to the provisional (side-centre) ports it builds
/// with; once `place_ports` fixes the real ones, spans must follow — a stale
/// span collapses straight shots to zero length and breaks both overlap
/// clustering and the comparator's turn positions.
fn snap_junctions(chains: &mut [Option<Chain>]) {
    for chain in chains.iter_mut().flatten() {
        for i in 0..chain.runs.len() {
            for e in 0..2 {
                let nb = if e == 1 { i + 1 } else { i.wrapping_sub(1) };
                if nb >= chain.runs.len() || chain.runs[nb].pin == Pin::Free {
                    continue;
                }
                if let Conn::Junction { cell, .. } = chain.runs[i].conn[e] {
                    let q = chain.runs[nb].ord;
                    chain.runs[i].conn[e] = Conn::Junction { cell, q };
                }
            }
        }
    }
}

/// Every run of every chain, addressed per channel — the unit run ordering
/// and ordinate assignment work over.
fn channel_map(chains: &[Option<Chain>]) -> BTreeMap<ChanKey, Vec<(usize, usize)>> {
    let mut channels: BTreeMap<ChanKey, Vec<(usize, usize)>> = BTreeMap::new();
    for (ci, chain) in chains.iter().enumerate() {
        let Some(chain) = chain.as_ref() else {
            continue;
        };
        for (ri, r) in chain.runs.iter().enumerate() {
            channels
                .entry((chain.world, r.axis.index(), r.chan))
                .or_default()
                .push((ci, ri));
        }
    }
    channels
}

/// Realise inversions (ROUTING §Model step 5): when two overlapping runs'
/// ends demand opposite orders, the later link swaps sides mid-channel — its
/// run splits at the overlap midpoint, and the perpendicular jog between the
/// halves crosses the partner square-on, both links locally straight. The
/// jog is a **real run** in the split cell's perpendicular channel, so lane
/// spacing, soft-wall margins, and occupancy govern it like any other run.
/// Only pairs the current assignment **actually draws crossing** are
/// realised — the walk verdict alone misfires on tied shared-port geometry —
/// and an inverted pair crosses exactly once, so each chain pair splits at
/// most once.
fn split_inversions(worlds: &[World], chains: &mut [Option<Chain>]) -> Vec<Swap> {
    let mut swaps: Vec<Swap> = Vec::new();
    let crossing: std::collections::BTreeSet<(usize, usize)> = super::audit::collect(chains)
        .iter()
        .map(|c| c.pair)
        .collect();
    if crossing.is_empty() {
        return swaps;
    }
    let mut done: std::collections::BTreeSet<(usize, usize)> = std::collections::BTreeSet::new();
    loop {
        let Some((pair, (ci, ri), mid, cell)) = find_inversion(worlds, chains, &crossing, &done)
        else {
            return swaps;
        };
        done.insert(pair);
        for s in &mut swaps {
            if s.ci == ci && s.jog >= ri {
                s.jog += 2;
            }
        }
        swaps.push(Swap {
            ci,
            jog: ri + 1,
            partner: if pair.0 == ci { pair.1 } else { pair.0 },
        });
        let chain = chains[ci].as_mut().unwrap();
        let mut head = chain.runs[ri].clone();
        let mut tail = head.clone();
        head.conn[1] = Conn::Junction { cell, q: mid };
        tail.conn[0] = Conn::Junction { cell, q: mid };
        match head.pin {
            Pin::Port(0) => tail.pin = Pin::Free,
            Pin::Port(1) => head.pin = Pin::Free,
            _ => {}
        }
        let (jog_axis, jog_chan) = {
            let c = &worlds[chain.world].graph.cells[cell];
            match head.axis {
                Axis::H => (Axis::V, c.v),
                Axis::V => (Axis::H, c.h),
            }
        };
        let jog = Run {
            axis: jog_axis,
            chan: jog_chan,
            pin: Pin::Free,
            ord: mid,
            conn: [
                Conn::Junction { cell, q: head.ord },
                Conn::Junction { cell, q: tail.ord },
            ],
        };
        chain.runs.splice(ri..=ri, [head, jog, tail]);
    }
}

type Inversion = ((usize, usize), (usize, usize), f64, usize);

/// The first inverted overlapping pair among `crossing` chain pairs not yet
/// realised, in channel order: the chain pair, the run to split (the later
/// link's), the overlap midpoint, and the cell containing it.
fn find_inversion(
    worlds: &[World],
    chains: &[Option<Chain>],
    crossing: &std::collections::BTreeSet<(usize, usize)>,
    done: &std::collections::BTreeSet<(usize, usize)>,
) -> Option<Inversion> {
    for ((world, axis, chan), runs) in channel_map(chains) {
        for (i, &a) in runs.iter().enumerate() {
            for &b in &runs[i + 1..] {
                let pair = (a.0.min(b.0), a.0.max(b.0));
                if !crossing.contains(&pair) || done.contains(&pair) {
                    continue;
                }
                let span = |(ci, ri): (usize, usize)| chains[ci].as_ref().unwrap().runs[ri].span();
                let (lo, hi) = (span(a).0.max(span(b).0), span(a).1.min(span(b).1));
                if lo >= hi || !order::inverted(chains, a, b) {
                    continue;
                }
                let rank = |(ci, _): (usize, usize)| (chains[ci].as_ref().unwrap().req, ci);
                let later = if rank(b) > rank(a) { b } else { a };
                if matches!(
                    chains[later.0].as_ref().unwrap().runs[later.1].pin,
                    Pin::Fixed(_)
                ) {
                    continue;
                }
                let mid = (lo + hi) / 2.0;
                let cell = worlds[world].graph.cells.iter().position(|c| {
                    let (own, e0, e1) = if axis == 0 {
                        (c.h, c.rect.x0, c.rect.x1)
                    } else {
                        (c.v, c.rect.y0, c.rect.y1)
                    };
                    own == chan && e0 <= mid && mid <= e1
                });
                if let Some(cell) = cell {
                    return Some((pair, later, mid, cell));
                }
            }
        }
    }
    None
}

/// Port placement: group every chain end (and fan groups as one unit) per
/// `(node, side)`, order units with the end comparator, spread ordinates at
/// `clearance` pitch centred on the side — shifted by the side's accepted
/// slide, if any. A side past its capacity **compacts** (ROUTING Law 2): all
/// of its units re-space evenly at the widest pitch the side allows,
/// `usable / (units − 1)`, corner margins intact at full clearance — pitch
/// zero, ports coinciding, when the side is too short for distinct points.
fn place_ports(chains: &mut [Option<Chain>], clearance: f64, slides: &Slides) {
    #[derive(PartialEq)]
    enum UnitKey {
        Fan(usize),
        Single(usize, usize),
    }
    type Unit = (UnitKey, Vec<(usize, usize)>);
    let mut sides: BTreeMap<(String, u8), Vec<Unit>> = BTreeMap::new();
    for (ci, chain) in chains.iter().enumerate() {
        let Some(chain) = chain.as_ref() else {
            continue;
        };
        for end in 0..2 {
            let e = &chain.ends[end];
            let key = (e.path.clone(), e.side.index());
            let unit = match e.fan {
                Some(g) => UnitKey::Fan(g),
                None => UnitKey::Single(ci, end),
            };
            let units = sides.entry(key).or_default();
            match units.iter_mut().find(|(k, _)| *k == unit) {
                Some((_, members)) => members.push((ci, end)),
                None => units.push((unit, vec![(ci, end)])),
            }
        }
    }

    for ((path, side), mut units) in sides {
        let slide = slides.get(&(path, side)).copied().unwrap_or(0.0);
        let side = Side::ALL[side as usize];
        units.sort_by(|a, b| order::cmp_ends(chains, a.1[0], b.1[0]));
        let n = units.len();
        let e0 = {
            let (ci, end) = units[0].1[0];
            chains[ci].as_ref().unwrap().ends[end].clone()
        };
        let centre = slide
            + match side {
                Side::Left | Side::Right => (e0.rect.y0 + e0.rect.y1) / 2.0,
                Side::Top | Side::Bottom => (e0.rect.x0 + e0.rect.x1) / 2.0,
            };
        let pitch = if n > side_capacity(e0.rect, side, clearance) {
            (side_len(e0.rect, side) - 2.0 * clearance).max(0.0) / (n as f64 - 1.0)
        } else {
            clearance
        };
        for (i, (_, members)) in units.iter().enumerate() {
            let ord = centre + (i as f64 - (n as f64 - 1.0) / 2.0) * pitch;
            for &(ci, end) in members {
                let chain = chains[ci].as_mut().unwrap();
                let e = &mut chain.ends[end];
                e.port = match side {
                    Side::Right => (e.rect.x1, ord),
                    Side::Left => (e.rect.x0, ord),
                    Side::Top => (ord, e.rect.y0),
                    Side::Bottom => (ord, e.rect.y1),
                };
                for r in &mut chain.runs {
                    if r.pin == Pin::Port(end) {
                        r.ord = ord;
                    }
                }
            }
        }
    }
}

/// Ordinates for one channel's runs, already in cross order. Overlap clusters
/// are independent; within one, pins anchor and through-runs fit around them.
fn assign_channel(
    chains: &mut [Option<Chain>],
    runs: &[(usize, usize)],
    channel: &super::graph::Channel,
    clearance: f64,
) {
    let info = |chains: &[Option<Chain>], (ci, ri): (usize, usize)| {
        let r = &chains[ci].as_ref().unwrap().runs[ri];
        let pinned = match r.pin {
            Pin::Free => None,
            Pin::Fixed(v) => Some(v),
            Pin::Port(_) => Some(r.ord),
        };
        (r.span(), pinned)
    };

    // Overlap clusters over the ordered list (union spans). Spans nearer
    // than `clearance` count as overlapping: runs that share an ordinate sit
    // tip-to-tip on one line, so their gap is link–link distance — and a run
    // pinned elsewhere must not start inside a stranger's clearance band.
    let mut clusters: Vec<(f64, f64, Vec<usize>)> = Vec::new();
    for (i, &run) in runs.iter().enumerate() {
        let ((lo, hi), _) = info(chains, run);
        match clusters
            .iter_mut()
            .find(|(clo, chi, _)| lo < *chi + clearance && hi > *clo - clearance)
        {
            Some((clo, chi, members)) => {
                *clo = clo.min(lo);
                *chi = chi.max(hi);
                members.push(i);
            }
            None => clusters.push((lo, hi, vec![i])),
        }
    }
    // Growing a cluster can swallow a later one; merge until stable.
    loop {
        let mut merged = false;
        let mut i = 0;
        while i < clusters.len() {
            let mut j = i + 1;
            while j < clusters.len() {
                if clusters[i].0 < clusters[j].1 + clearance
                    && clusters[i].1 > clusters[j].0 - clearance
                {
                    let (lo, hi, members) = clusters.remove(j);
                    clusters[i].0 = clusters[i].0.min(lo);
                    clusters[i].1 = clusters[i].1.max(hi);
                    clusters[i].2.extend(members);
                    merged = true;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
        if !merged {
            break;
        }
    }

    for (clo, chi, mut members) in clusters {
        members.sort_unstable();
        let packed: Vec<(f64, f64, Option<f64>)> = members
            .iter()
            .map(|&i| {
                let ((lo, hi), pin) = info(chains, runs[i]);
                (lo, hi, pin)
            })
            .collect();
        let (mut at, mut pins) = pack(&packed, clearance);
        let (u0, u1) = channel.usable(clo, chi, clearance);
        let anchored = members
            .iter()
            .any(|&i| chains[runs[i].0].as_ref().unwrap().margin);
        let mid = cluster_midline(channel, u0, u1, pins.len(), clearance, anchored);
        let mut ords = cluster_ordinates(&pins, mid, clearance);
        let cluster: Vec<(usize, usize)> = members.iter().map(|&i| runs[i]).collect();
        nest_rails(
            chains,
            channel,
            &cluster,
            (&mut at, &mut pins, &mut ords),
            (u0 + u1) / 2.0,
            clearance,
        );
        for (&i, &rail) in members.iter().zip(&at) {
            let (ci, ri) = runs[i];
            chains[ci].as_mut().unwrap().runs[ri].ord = ords[rail];
        }
    }
}

/// One cluster's rail layout, mutated in place by [`nest_rails`]:
/// member → rail, rail → pin, rail → ordinate.
type RailLayout<'a> = (
    &'a mut Vec<usize>,
    &'a mut Vec<Option<f64>>,
    &'a mut Vec<f64>,
);

/// An owned rail layout — a [`nest_rails`] candidate.
type OwnedLayout = (Vec<usize>, Vec<Option<f64>>, Vec<f64>);

/// Pinless rails in one cluster ladder have a topologically free order:
/// the comparator's pick is planar either way, and `pack`'s lane sharing
/// is decided by span clearance alone — yet both choices matter across
/// channels, where staircase links can interleave their steps nearer than
/// clearance while another arrangement nests them cleanly (ROUTING §Model
/// step 5 — nested, never braided). When the packed layout draws the
/// cluster into conflict, try every order of its free rails, on the
/// packed sharing and on variants giving one shared-rail member its own
/// rail; score by the cluster's drawn proximities — each member's run
/// plus the connecting rows that move with it — and keep the first strict
/// improvement. A conflict-free packing is never touched, so clean scenes
/// are byte-identical with or without this pass. Strict-overlap pairs
/// never reorder (Jordan; `pack` rule (a)), pins never move, and no
/// variant may leave the channel walls.
fn nest_rails(
    chains: &[Option<Chain>],
    channel: &Channel,
    cluster: &[(usize, usize)],
    layout: RailLayout,
    midline: f64,
    clearance: f64,
) {
    let (at, pins, ords) = layout;
    let mut best_score = nest_conflicts(chains, channel.axis, cluster, at, ords, clearance);
    if best_score == 0 {
        return;
    }
    let span = |m: (usize, usize)| chains[m.0].as_ref().unwrap().runs[m.1].span();
    // Strict-overlap pairs keep the order the packed ladder gave them.
    let mut keep_order: Vec<(usize, usize, bool)> = Vec::new();
    for (i, &a) in cluster.iter().enumerate() {
        for (j, &b) in cluster.iter().enumerate().skip(i + 1) {
            let (sa, sb) = (span(a), span(b));
            if at[i] != at[j] && sa.0 < sb.1 && sa.1 > sb.0 {
                keep_order.push((i, j, ords[at[i]] < ords[at[j]]));
            }
        }
    }

    let mut layouts: Vec<(Vec<usize>, Vec<Option<f64>>)> = vec![(at.clone(), pins.clone())];
    for (m, &rail) in at.iter().enumerate() {
        if pins[rail].is_none() && at.iter().filter(|&&r| r == rail).count() >= 2 {
            let mut alt = at.clone();
            alt[m] = pins.len();
            let mut alt_pins = pins.clone();
            alt_pins.push(None);
            layouts.push((alt, alt_pins));
        }
    }

    let (w0, w1) = channel.walls();
    let mut best: Option<OwnedLayout> = None;
    for (alt, alt_pins) in layouts {
        let ladder = cluster_ordinates(&alt_pins, midline, clearance);
        let free: Vec<usize> = (0..ladder.len())
            .filter(|&r| alt_pins[r].is_none())
            .collect();
        if free.len() > 4 {
            continue;
        }
        let base: Vec<f64> = free.iter().map(|&r| ladder[r]).collect();
        for perm in permutations(free.len()) {
            let mut cand = ladder.clone();
            for (k, &r) in free.iter().enumerate() {
                cand[r] = base[perm[k]];
            }
            let legal = cand.iter().all(|&o| o >= w0 - EPS && o <= w1 + EPS)
                && keep_order
                    .iter()
                    .all(|&(i, j, less)| (cand[alt[i]] < cand[alt[j]]) == less);
            if !legal {
                continue;
            }
            let score = nest_conflicts(chains, channel.axis, cluster, &alt, &cand, clearance);
            if score < best_score {
                best_score = score;
                best = Some((alt.clone(), alt_pins.clone(), cand));
            }
        }
    }
    if let Some((alt, alt_pins, cand)) = best {
        *at = alt;
        *pins = alt_pins;
        *ords = cand;
    }
}

/// The segments that move with one member: `(chain, segments)`.
type MemberSegs = (usize, Vec<[(f64, f64); 2]>);

/// Sub-clearance proximities among one cluster's members at candidate
/// ordinates, square-on crossings free. Different chains only — a chain
/// never conflicts with itself.
fn nest_conflicts(
    chains: &[Option<Chain>],
    axis: Axis,
    cluster: &[(usize, usize)],
    at: &[usize],
    ords: &[f64],
    clearance: f64,
) -> usize {
    let segs: Vec<MemberSegs> = cluster
        .iter()
        .zip(at)
        .map(|(&(ci, ri), &rail)| (ci, member_segments(chains, axis, ci, ri, ords[rail])))
        .collect();
    let mut n = 0;
    for (i, (ca, sa)) in segs.iter().enumerate() {
        for (cb, sb) in segs.iter().skip(i + 1) {
            if ca == cb {
                continue;
            }
            for a in sa {
                for b in sb {
                    if super::audit::cross(a, b).is_none()
                        && super::audit::seg_dist(a, b) < clearance - EPS
                    {
                        n += 1;
                    }
                }
            }
        }
    }
    n
}

/// The drawn segments that move with one member at ordinate `o`: its run
/// along the channel, and each connecting row from the corner at `o` out
/// to the row's far end.
fn member_segments(
    chains: &[Option<Chain>],
    axis: Axis,
    ci: usize,
    ri: usize,
    o: f64,
) -> Vec<[(f64, f64); 2]> {
    let chain = chains[ci].as_ref().unwrap();
    let (lo, hi) = chain.runs[ri].span();
    let pt = |q: f64, ord: f64| match axis {
        Axis::H => (q, ord),
        Axis::V => (ord, q),
    };
    let mut out = vec![[pt(lo, o), pt(hi, o)]];
    for (e, nb) in [(0usize, ri.wrapping_sub(1)), (1, ri + 1)] {
        let Some(row) = chain.runs.get(nb) else {
            continue;
        };
        let far = row.conn[e].q();
        let (a, b) = (o.min(far), o.max(far));
        out.push(match axis {
            Axis::H => [(row.ord, a), (row.ord, b)],
            Axis::V => [(a, row.ord), (b, row.ord)],
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hand-built realised swap: chain 0 is the partner (one straight run
    /// at `partner_ord`), chain 1 is split into head (ord 26) / jog (at
    /// q 50) / tail (ord 18) in the same channel.
    fn swapped_pair(partner_ord: f64) -> (Vec<Option<Chain>>, Vec<Swap>) {
        let end = |side| EndInfo {
            path: String::new(),
            side,
            rect: Rect::new(0.0, 0.0, 40.0, 40.0),
            port: (0.0, 0.0),
            fan: None,
        };
        let run = |ord: f64, conn: [Conn; 2]| Run {
            axis: Axis::V,
            chan: 0,
            pin: Pin::Free,
            ord,
            conn,
        };
        let chain = |runs: Vec<Run>, req: usize| {
            Some(Chain {
                world: 0,
                runs,
                ends: [end(crate::ast::Side::Top), end(crate::ast::Side::Bottom)],
                req,
                margin: false,
            })
        };
        let partner = chain(
            vec![run(
                partner_ord,
                [Conn::Terminal { q: 0.0 }, Conn::Terminal { q: 100.0 }],
            )],
            0,
        );
        let head = run(
            26.0,
            [
                Conn::Terminal { q: 0.0 },
                Conn::Junction { cell: 0, q: 50.0 },
            ],
        );
        let jog = Run {
            axis: Axis::H,
            chan: 0,
            pin: Pin::Free,
            ord: 50.0,
            conn: [
                Conn::Junction { cell: 0, q: 26.0 },
                Conn::Junction { cell: 0, q: 18.0 },
            ],
        };
        let tail = run(
            18.0,
            [
                Conn::Junction { cell: 0, q: 50.0 },
                Conn::Terminal { q: 100.0 },
            ],
        );
        let split = chain(vec![head, jog, tail], 1);
        let swaps = vec![Swap {
            ci: 1,
            jog: 1,
            partner: 0,
        }];
        (vec![partner, split], swaps)
    }

    #[test]
    fn swap_jog_that_crosses_nothing_merges_back() {
        // Both halves (26, 18) land on one side of the partner (10): the jog
        // crosses nothing — two turns and an extra lane that buy no crossing.
        let (mut chains, mut swaps) = swapped_pair(10.0);
        assert!(revert_failed_swaps(&mut chains, &mut swaps));
        let merged = chains[1].as_ref().unwrap();
        assert_eq!(merged.runs.len(), 1, "the pointless swap must merge back");
        assert_eq!(merged.runs[0].span(), (0.0, 100.0));
        assert!(swaps.is_empty());
        assert!(!revert_failed_swaps(&mut chains, &mut swaps));
    }

    #[test]
    fn swap_jog_that_crosses_its_partner_is_kept() {
        // The partner (22) sits between the halves (18, 26) inside the jog's
        // span: the jog realises the crossing — it must stay.
        let (mut chains, mut swaps) = swapped_pair(22.0);
        assert!(!revert_failed_swaps(&mut chains, &mut swaps));
        assert_eq!(chains[1].as_ref().unwrap().runs.len(), 3);
        assert_eq!(swaps.len(), 1);
    }

    #[test]
    fn a_lone_port_meets_its_straight_link() {
        // Two stacked boxes whose centres miss by 6: the straight shot
        // between them must ride one ordinate (the start's), not jog
        // mid-corridor to bridge the centres (ROUTING §Ports, lone-port
        // freedom).
        use crate::layout::links::{geometry, graph::ChannelGraph, path};

        let bounds = Rect::new(0.0, 0.0, 300.0, 200.0);
        let a = Rect::new(100.0, 20.0, 160.0, 40.0);
        let b = Rect::new(106.0, 120.0, 166.0, 140.0);
        let keepouts = [a.inflate(8.0), b.inflate(8.0)];
        let graph = ChannelGraph::build(bounds, &keepouts, false);
        let starts = path::entries(&graph, a, 8.0, None, &[], false);
        let goals = path::entries(&graph, b, 8.0, None, &[], false);
        let r = path::shortest(&graph, &starts, &goals, &|_, _, _, _| false, path::FREE)
            .expect("route");
        let (se, ge) = (&starts[r.start], &goals[r.goal]);
        let ends = [("a", a, se), ("b", b, ge)].map(|(name, rect, e)| EndInfo {
            path: name.to_owned(),
            side: e.side,
            rect,
            port: e.port,
            fan: None,
        });
        let mut chains = vec![Some(geometry::chain(
            &graph, 0, &r.cells, se, ge, ends, 0, false,
        ))];
        let worlds = [World {
            path: String::new(),
            graph,
        }];
        assign(&worlds, &mut chains, 8.0, &Default::default());

        let poly = geometry::polyline(chains[0].as_ref().unwrap());
        let x0 = (a.x0 + a.x1) / 2.0;
        assert!(
            poly.iter().all(|p| (p.0 - x0).abs() < EPS),
            "the goal port aligns to the start: {poly:?}"
        );
    }

    #[test]
    fn inverted_pair_swaps_mid_channel_with_one_jog() {
        // The west-column interleave: a (top) → c (lower middle) and
        // b (upper middle) → d (bottom), all ends forced left — the runs
        // share the margin channel and their end orders conflict (Jordan),
        // so the later link must swap sides at the overlap midpoint.
        use crate::ast::Side;
        use crate::layout::links::{geometry, graph::ChannelGraph, path};

        let bounds = Rect::new(0.0, 0.0, 200.0, 400.0);
        let names = ["a", "b", "c", "d"];
        let bodies: Vec<Rect> = (0..4)
            .map(|i| {
                Rect::new(
                    100.0,
                    20.0 + 100.0 * i as f64,
                    160.0,
                    40.0 + 100.0 * i as f64,
                )
            })
            .collect();
        let keepouts: Vec<Rect> = bodies.iter().map(|b| b.inflate(8.0)).collect();
        let graph = ChannelGraph::build(bounds, &keepouts, false);
        let route = |from: usize, to: usize, req: usize| {
            let starts = path::entries(&graph, bodies[from], 8.0, Some(Side::Left), &[], false);
            let goals = path::entries(&graph, bodies[to], 8.0, Some(Side::Left), &[], false);
            let r = path::shortest(&graph, &starts, &goals, &|_, _, _, _| false, path::FREE)
                .expect("route");
            let (se, ge) = (&starts[r.start], &goals[r.goal]);
            let ends = [(from, se), (to, ge)].map(|(i, e)| EndInfo {
                path: names[i].to_owned(),
                side: e.side,
                rect: bodies[i],
                port: e.port,
                fan: None,
            });
            geometry::chain(&graph, 0, &r.cells, se, ge, ends, req, false)
        };
        let mut chains = vec![Some(route(0, 2, 0)), Some(route(1, 3, 1))];
        assert!(order::inverted(&chains, (0, 1), (1, 1)), "pair must invert");

        let worlds = [World {
            path: String::new(),
            graph,
        }];
        assign(&worlds, &mut chains, 8.0, &Default::default());

        let (early, late) = (chains[0].as_ref().unwrap(), chains[1].as_ref().unwrap());
        assert_eq!(early.runs.len(), 3, "the earlier link stays whole");
        assert_eq!(
            late.runs.len(),
            5,
            "the later link splits around a real jog"
        );
        let (head, jog, tail) = (&late.runs[1], &late.runs[2], &late.runs[3]);
        assert_eq!(head.axis, Axis::V);
        assert_eq!(tail.axis, Axis::V);
        assert_eq!(head.chan, tail.chan);
        assert_eq!(jog.axis, Axis::H, "the swap jog is a managed run");
        // Split at the overlap midpoint: spans [30, 230] ∩ [130, 330] → 180.
        assert_eq!(head.conn[1].q(), 180.0);
        assert_eq!(tail.conn[0].q(), 180.0);
        // The halves flank the partner: the jog crosses it square-on.
        let partner = early.runs[1].ord;
        assert!(
            tail.ord < partner && partner < head.ord,
            "tail {} < partner {} < head {}",
            tail.ord,
            partner,
            head.ord
        );
        let poly = geometry::polyline(late);
        assert!(poly.contains(&(head.ord, jog.ord)) && poly.contains(&(tail.ord, jog.ord)));
    }
}

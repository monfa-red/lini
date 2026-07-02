//! Placement (ROUTING.md model step 5): every run's ordinate, decided once,
//! per channel, by one mechanism — cluster, order, ladder.
//!
//! Runs whose spans come within a clearance of one another form a cluster; a
//! cluster's pitch is `min(clearance, usable/(n−1))` floored at half the
//! clearance (the search guaranteed fit). Within a cluster runs order so
//! wires leave in the order they arrive — nested, never braided — and take
//! the order-preserving ordinates nearest their preferences ([`ladder`]).
//! Preferences are the aesthetic law: interior runs want their channel's
//! anchor (the midline between two nodes, the keep-out wall at the canvas
//! edge); end runs want the straightest lawful line to their port. Ports
//! *are* end-run ordinates — fan siblings merge into one item and share one
//! port — so a port can never disagree with the wire it serves.

// Scaffold: consumed by the pipeline driver (ROUTING-V2.md stage 4);
// the allow leaves with it.
#![allow(dead_code)]

use std::collections::BTreeMap;

use super::cost::min_pitch;
use super::graph::{Axis, Channel};
use super::ladder::ladder;
use super::{Chain, World};

/// One ladder item: a run (or a fan's merged end runs) awaiting its
/// ordinate.
struct Item {
    /// `(chain index, run index)` of every run taking this ordinate.
    members: Vec<(usize, usize)>,
    span: (f64, f64),
    pref: f64,
    /// Hard bounds from the port window; `None` for interior runs (the
    /// channel's usable range applies alone).
    window: Option<(f64, f64)>,
    /// Nesting keys toward the chain's two neighbours (see [`keys`]).
    key: [Option<(i8, i8, f64)>; 2],
    /// Declaration-order tie break.
    link: usize,
    run: usize,
}

/// Assign every `Run::ord` in every chain. Channels are processed in fixed
/// (world, axis, channel) order; preferences and nesting keys are static, so
/// the outcome is independent of that order — and deterministic.
pub(crate) fn place(worlds: &[World], chains: &mut [Option<Chain>], clearance: f64) {
    let mut by_channel: BTreeMap<(usize, u8, usize), Vec<Item>> = BTreeMap::new();
    for (ci, chain) in chains.iter().enumerate() {
        let Some(chain) = chain else { continue };
        let prefs = chain_prefs(chain, worlds);
        let keys = chain_keys(chain, &prefs);
        for (ri, run) in chain.runs.iter().enumerate() {
            by_channel
                .entry((chain.world, run.axis.index(), run.chan))
                .or_default()
                .push(Item {
                    members: vec![(ci, ri)],
                    span: (run.span.0.min(run.span.1), run.span.0.max(run.span.1)),
                    pref: prefs[ri].0,
                    window: prefs[ri].1,
                    key: keys[ri],
                    link: chain.link,
                    run: ri,
                });
        }
    }

    for ((world, axis, chan), mut items) in by_channel {
        let channel = match [Axis::H, Axis::V][axis as usize] {
            Axis::H => &worlds[world].graph.h[chan],
            Axis::V => &worlds[world].graph.v[chan],
        };
        merge_fans(&mut items, chains);
        items.sort_by(|a, b| a.span.0.total_cmp(&b.span.0).then(a.link.cmp(&b.link)));
        // Chain spans within a clearance of each other into clusters.
        let mut cluster: Vec<Item> = Vec::new();
        let mut reach = f64::MIN;
        for item in items {
            if !cluster.is_empty() && item.span.0 >= reach + clearance {
                settle(cluster, channel, clearance, chains);
                cluster = Vec::new();
                reach = f64::MIN;
            }
            reach = reach.max(item.span.1);
            cluster.push(item);
        }
        if !cluster.is_empty() {
            settle(cluster, channel, clearance, chains);
        }
    }
}

/// Per-run `(preference, port window)` for one chain (ROUTING.md step 5):
/// a single run serving both ports prefers the straightest lawful line —
/// the two side centres' midpoint clamped into the shared window; an end
/// run prefers its own side's centre inside its window; an interior run
/// prefers its channel's anchor.
fn chain_prefs(chain: &Chain, worlds: &[World]) -> Vec<(f64, Option<(f64, f64)>)> {
    let last = chain.runs.len() - 1;
    chain
        .runs
        .iter()
        .enumerate()
        .map(|(ri, run)| {
            let (a, b) = (&chain.ends[0], &chain.ends[1]);
            if ri == 0 && ri == last {
                let shared = (a.window.0.max(b.window.0), a.window.1.min(b.window.1));
                debug_assert!(
                    shared.0 <= shared.1,
                    "a straight run needs overlapping windows (the search jogs otherwise)"
                );
                let mid = (a.centre() + b.centre()) / 2.0;
                (mid.max(shared.0).min(shared.1), Some(shared))
            } else if ri == 0 {
                (a.centre(), Some(a.window))
            } else if ri == last {
                (b.centre(), Some(b.window))
            } else {
                let anchor = match run.axis {
                    Axis::H => worlds[chain.world].graph.h[run.chan].anchor(),
                    Axis::V => worlds[chain.world].graph.v[run.chan].anchor(),
                };
                (anchor, None)
            }
        })
        .collect()
}

/// Nesting keys — how a run's chain neighbours order it against cluster
/// mates with equal preferences (nested, never braided). Toward each
/// neighbour: the two arm directions away from their shared corner (the
/// neighbour's, then this run's, as ±1) and the neighbour's own ordinate
/// preference signed by their product — for corners turning the same way,
/// ascending key is ascending ordinate, so wires leave in the order they
/// arrive. Ends have no key on the port side: the far side decides.
fn chain_keys(
    chain: &Chain,
    prefs: &[(f64, Option<(f64, f64)>)],
) -> Vec<[Option<(i8, i8, f64)>; 2]> {
    let n = chain.runs.len();
    // Each run's provisional endpoints along its travel axis: the chain-side
    // neighbour's preference, or the side line at the ports.
    let lo_est: Vec<f64> = (0..n)
        .map(|i| {
            if i == 0 {
                chain.ends[0].side_coord()
            } else {
                prefs[i - 1].0
            }
        })
        .collect();
    let hi_est: Vec<f64> = (0..n)
        .map(|i| {
            if i == n - 1 {
                chain.ends[1].side_coord()
            } else {
                prefs[i + 1].0
            }
        })
        .collect();
    let sign = |v: f64| if v < 0.0 { -1i8 } else { 1i8 };
    (0..n)
        .map(|i| {
            let towards_prev = (i > 0).then(|| {
                let arm_n = sign(lo_est[i - 1] - hi_est[i - 1]);
                let arm_r = sign(hi_est[i] - lo_est[i]);
                (arm_n, arm_r, f64::from(arm_n * arm_r) * prefs[i - 1].0)
            });
            let towards_next = (i + 1 < n).then(|| {
                let arm_n = sign(hi_est[i + 1] - lo_est[i + 1]);
                let arm_r = sign(lo_est[i] - hi_est[i]);
                (arm_n, arm_r, f64::from(arm_n * arm_r) * prefs[i + 1].0)
            });
            [towards_prev, towards_next]
        })
        .collect()
}

/// Fan siblings' end runs share one port: merge same-group items into one,
/// spans united, windows intersected.
fn merge_fans(items: &mut Vec<Item>, chains: &[Option<Chain>]) {
    let mut merged: Vec<Item> = Vec::new();
    for item in items.drain(..) {
        let (ci, ri) = item.members[0];
        let chain = chains[ci].as_ref().expect("placed chain");
        let fan = fan_of(chain, ri);
        let twin = fan.and_then(|f| {
            merged.iter_mut().find(|m| {
                let (mc, mr) = m.members[0];
                fan_of(chains[mc].as_ref().expect("placed chain"), mr) == Some(f)
            })
        });
        match twin {
            Some(m) => {
                m.span = (m.span.0.min(item.span.0), m.span.1.max(item.span.1));
                m.window = match (m.window, item.window) {
                    (Some(a), Some(b)) => Some((a.0.max(b.0), a.1.min(b.1))),
                    (w, None) | (None, w) => w,
                };
                m.link = m.link.min(item.link);
                m.members.extend(item.members);
            }
            None => merged.push(item),
        }
    }
    *items = std::mem::take(&mut merged);
}

/// The fan group of an **end** run, if any — interior runs never merge.
fn fan_of(chain: &Chain, ri: usize) -> Option<usize> {
    let last = chain.runs.len() - 1;
    match (ri == 0, ri == last) {
        (true, true) => chain.ends[0].fan.or(chain.ends[1].fan),
        (true, false) => chain.ends[0].fan,
        (false, true) => chain.ends[1].fan,
        _ => None,
    }
}

/// Order one cluster and ladder it into ordinates.
fn settle(mut cluster: Vec<Item>, channel: &Channel, clearance: f64, chains: &mut [Option<Chain>]) {
    let lo = cluster.iter().map(|i| i.span.0).fold(f64::MAX, f64::min);
    let hi = cluster.iter().map(|i| i.span.1).fold(f64::MIN, f64::max);
    let (u0, u1) = channel.usable(lo, hi, clearance);
    let n = cluster.len();
    let pitch = if n > 1 {
        (clearance.min((u1 - u0) / (n - 1) as f64)).max(min_pitch(clearance))
    } else {
        clearance
    };

    // Nested, never braided: preference first, then the nesting keys, then
    // declaration order.
    let key_cmp = |a: &Option<(i8, i8, f64)>, b: &Option<(i8, i8, f64)>| match (a, b) {
        (Some((an, ar, ak)), Some((bn, br, bk))) => {
            an.cmp(bn).then(ar.cmp(br)).then(ak.total_cmp(bk))
        }
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
    };
    cluster.sort_by(|a, b| {
        a.pref
            .total_cmp(&b.pref)
            .then_with(|| key_cmp(&a.key[0], &b.key[0]))
            .then_with(|| key_cmp(&a.key[1], &b.key[1]))
            .then(a.link.cmp(&b.link))
            .then(a.run.cmp(&b.run))
    });

    let prefs: Vec<f64> = cluster.iter().map(|i| i.pref).collect();
    let bounds: Vec<(f64, f64)> = cluster
        .iter()
        .map(|i| match i.window {
            // A port window bounds its run hard; the channel's usable range
            // tightens it where it can, but the lawful window always wins.
            Some(w) => {
                let tight = (w.0.max(u0), w.1.min(u1));
                if tight.0 <= tight.1 { tight } else { w }
            }
            None => (u0, u1),
        })
        .collect();
    // Two pieces of one wire owe each other nothing unless their spans
    // overlap (a U's doubled-back legs); different wires always keep pitch.
    let seps: Vec<f64> = cluster
        .windows(2)
        .map(|w| {
            let same_wire = w[0]
                .members
                .iter()
                .any(|(c0, _)| w[1].members.iter().any(|(c1, _)| c0 == c1));
            let spans_overlap = w[0].span.0.max(w[1].span.0) < w[0].span.1.min(w[1].span.1);
            if same_wire && !spans_overlap {
                0.0
            } else {
                pitch
            }
        })
        .collect();
    let ords = ladder(&prefs, &bounds, &seps);
    for (item, ord) in cluster.iter().zip(ords) {
        for &(ci, ri) in &item.members {
            chains[ci].as_mut().expect("placed chain").runs[ri].ord = Some(ord);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::graph::ChannelGraph;
    use super::super::rect::Rect;
    use super::super::{EndInfo, Run};
    use super::*;
    use crate::ast::Side;

    const C: f64 = 8.0;

    fn world(bounds: Rect, keepouts: &[Rect]) -> World {
        World {
            path: String::new(),
            graph: ChannelGraph::build(bounds, keepouts, false),
        }
    }

    fn end(side: Side, rect: Rect) -> EndInfo {
        let window = match side {
            Side::Left | Side::Right => (rect.y0 + C, rect.y1 - C),
            Side::Top | Side::Bottom => (rect.x0 + C, rect.x1 - C),
        };
        EndInfo {
            side,
            rect,
            window,
            fan: None,
        }
    }

    /// The facing scene: two tall nodes (windows 44 high — room for a
    /// 4-bundle at clearance pitch) across an open corridor in a 200×100
    /// world.
    fn facing() -> (World, Rect, Rect) {
        let a = Rect::new(20.0, 20.0, 40.0, 80.0);
        let b = Rect::new(160.0, 20.0, 180.0, 80.0);
        let w = world(
            Rect::new(0.0, 0.0, 200.0, 100.0),
            &[a.inflate(C), b.inflate(C)],
        );
        (w, a, b)
    }

    fn h_chan(w: &World, x: f64, y: f64) -> usize {
        w.graph
            .h
            .iter()
            .position(|c| x >= c.rect.x0 && x <= c.rect.x1 && y >= c.rect.y0 && y <= c.rect.y1)
            .expect("h channel at point")
    }

    fn straight(link: usize, a: Rect, b: Rect, chan: usize) -> Chain {
        Chain {
            link,
            world: 0,
            runs: vec![Run {
                axis: Axis::H,
                chan,
                span: (a.x1, b.x0),
                ord: None,
            }],
            ends: [end(Side::Right, a), end(Side::Left, b)],
        }
    }

    #[test]
    fn a_lone_straight_takes_the_shared_centre() {
        let (w, a, b) = facing();
        let chan = h_chan(&w, 100.0, 50.0);
        let mut chains = vec![Some(straight(0, a, b, chan))];
        place(&[w], &mut chains, C);
        assert_eq!(chains[0].as_ref().unwrap().runs[0].ord, Some(50.0));
    }

    #[test]
    fn a_bundle_ladders_centred_on_the_shared_centre() {
        let (w, a, b) = facing();
        let chan = h_chan(&w, 100.0, 50.0);
        let mut chains: Vec<Option<Chain>> =
            (0..4).map(|i| Some(straight(i, a, b, chan))).collect();
        place(&[w], &mut chains, C);
        let ords: Vec<f64> = chains
            .iter()
            .map(|c| c.as_ref().unwrap().runs[0].ord.unwrap())
            .collect();
        // Four rails at clearance pitch, median on the aligned centres, in
        // declaration order.
        assert_eq!(ords, vec![38.0, 46.0, 54.0, 62.0]);
    }

    #[test]
    fn an_interior_run_rests_on_the_channel_midline() {
        // A three-run Z through the corridor: the jog's V run prefers the
        // anchor of the V channel between the keep-outs.
        let (w, a, b) = facing();
        let hchan = h_chan(&w, 100.0, 50.0);
        let vchan = w
            .graph
            .v
            .iter()
            .position(|c| c.rect == Rect::new(48.0, 0.0, 152.0, 100.0))
            .expect("middle V channel");
        let mut chains = vec![Some(Chain {
            link: 0,
            world: 0,
            runs: vec![
                Run {
                    axis: Axis::H,
                    chan: hchan,
                    span: (40.0, 100.0),
                    ord: None,
                },
                Run {
                    axis: Axis::V,
                    chan: vchan,
                    span: (48.0, 52.0),
                    ord: None,
                },
                Run {
                    axis: Axis::H,
                    chan: hchan,
                    span: (100.0, 160.0),
                    ord: None,
                },
            ],
            ends: [end(Side::Right, a), end(Side::Left, b)],
        })];
        place(&[w], &mut chains, C);
        let runs = &chains[0].as_ref().unwrap().runs;
        // End runs take their side centres; the jog takes the V anchor
        // (both walls are keep-out edges → their midline, x = 100).
        assert_eq!(runs[0].ord, Some(50.0));
        assert_eq!(runs[1].ord, Some(100.0));
        assert_eq!(runs[2].ord, Some(50.0));
    }

    #[test]
    fn turning_wires_nest_in_arrival_order() {
        // Two L-wires from stacked sources in the west turn south in one V
        // channel: the upper wire turns outside (east of) the lower — nested,
        // never braided (an east-then-south corner pair).
        let a1 = Rect::new(20.0, 10.0, 40.0, 26.0);
        let a2 = Rect::new(20.0, 34.0, 40.0, 50.0);
        let b = Rect::new(80.0, 160.0, 120.0, 180.0);
        let w = world(
            Rect::new(0.0, 0.0, 200.0, 200.0),
            &[a1.inflate(C), a2.inflate(C), b.inflate(C)],
        );
        // The V channel the wires descend in: the one over b, containing
        // its top window (x 88..112).
        let vchan = w
            .graph
            .v
            .iter()
            .position(|c| {
                c.rect.x0 <= 88.0 && c.rect.x1 >= 112.0 && c.rect.y0 <= 60.0 && c.rect.y1 >= 140.0
            })
            .expect("V channel above b");
        let l_chain = |link: usize, src: Rect, hchan: usize| Chain {
            link,
            world: 0,
            runs: vec![
                Run {
                    axis: Axis::H,
                    chan: hchan,
                    span: (src.x1, 100.0),
                    ord: None,
                },
                Run {
                    axis: Axis::V,
                    chan: vchan,
                    span: ((src.y0 + src.y1) / 2.0, 160.0),
                    ord: None,
                },
            ],
            ends: [end(Side::Right, src), end(Side::Top, b)],
        };
        let h1 = h_chan(&w, 60.0, 18.0);
        let h2 = h_chan(&w, 60.0, 42.0);
        let mut chains = vec![Some(l_chain(0, a1, h1)), Some(l_chain(1, a2, h2))];
        place(&[w], &mut chains, C);
        let x1 = chains[0].as_ref().unwrap().runs[1].ord.unwrap();
        let x2 = chains[1].as_ref().unwrap().runs[1].ord.unwrap();
        assert!(
            x1 > x2,
            "upper wire turns outside the lower: x1={x1} x2={x2}"
        );
    }

    #[test]
    fn fan_siblings_share_one_port_ordinate() {
        let (w, a, b) = facing();
        let chan = h_chan(&w, 100.0, 50.0);
        let mut c1 = straight(0, a, b, chan);
        let mut c2 = straight(1, a, b, chan);
        c1.ends[0].fan = Some(0);
        c2.ends[0].fan = Some(0);
        let mut chains = vec![Some(c1), Some(c2)];
        place(&[w], &mut chains, C);
        let o1 = chains[0].as_ref().unwrap().runs[0].ord.unwrap();
        let o2 = chains[1].as_ref().unwrap().runs[0].ord.unwrap();
        assert_eq!(o1, o2, "one fan, one port");
    }

    #[test]
    fn disjoint_clusters_both_take_the_midline() {
        // Two runs far apart along one channel never cluster: each sits on
        // the channel anchor independently.
        let w = world(Rect::new(0.0, 0.0, 400.0, 100.0), &[]);
        let interior = |link: usize, span: (f64, f64)| Chain {
            link,
            world: 0,
            runs: vec![
                Run {
                    axis: Axis::V,
                    chan: 0,
                    span: (10.0, 20.0),
                    ord: None,
                },
                Run {
                    axis: Axis::H,
                    chan: 0,
                    span,
                    ord: None,
                },
                Run {
                    axis: Axis::V,
                    chan: 0,
                    span: (80.0, 90.0),
                    ord: None,
                },
            ],
            ends: [
                end(Side::Bottom, Rect::new(span.0 - 20.0, 0.0, span.0, 10.0)),
                end(Side::Bottom, Rect::new(span.1, 0.0, span.1 + 20.0, 10.0)),
            ],
        };
        let mut chains = vec![
            Some(interior(0, (40.0, 120.0))),
            Some(interior(1, (240.0, 320.0))),
        ];
        place(&[w], &mut chains, C);
        let m1 = chains[0].as_ref().unwrap().runs[1].ord.unwrap();
        let m2 = chains[1].as_ref().unwrap().runs[1].ord.unwrap();
        assert_eq!(m1, 50.0, "the empty world's H anchor is its midline");
        assert_eq!(m1, m2, "disjoint spans share the midline in peace");
    }

    #[test]
    fn place_is_deterministic() {
        let (w, a, b) = facing();
        let chan = h_chan(&w, 100.0, 50.0);
        let run = |chains: &mut Vec<Option<Chain>>| {
            place(
                &[world(
                    Rect::new(0.0, 0.0, 200.0, 100.0),
                    &[a.inflate(C), b.inflate(C)],
                )],
                chains,
                C,
            );
            chains
                .iter()
                .map(|c| c.as_ref().unwrap().runs[0].ord.unwrap())
                .collect::<Vec<f64>>()
        };
        let mut first = (0..4)
            .map(|i| Some(straight(i, a, b, chan)))
            .collect::<Vec<_>>();
        let baseline = run(&mut first);
        for _ in 0..50 {
            let mut again = (0..4)
                .map(|i| Some(straight(i, a, b, chan)))
                .collect::<Vec<_>>();
            assert_eq!(run(&mut again), baseline);
        }
    }
}

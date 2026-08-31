use super::super::graph::ChannelGraph;
use super::super::rect::Rect;
use super::super::{EndInfo, Run};
use super::*;
use crate::ast::Side;

const C: f64 = 8.0;

fn world(bounds: Rect, keepouts: &[Rect]) -> World {
    World {
        key: None,
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
fn a_fan_whose_legs_windows_cannot_meet_still_places() {
    // [ROUTING.md step 5] `cluster::merge_fans` intersects fan siblings'
    // port windows, and the intersection can be **empty** without anything
    // being wrong: a straight leg carries the pair of its own two ends'
    // windows, so a tall node fanning to two far nodes at opposite
    // extremes offers two disjoint slices of the one shared side. Only a
    // fixed port guarantees the meet (a fan over two different ones strays
    // whole, upstream); a free window never did. The merged item must
    // still hand the ladder one valid interval — the first window — rather
    // than an inverted one no corridor can satisfy.
    let a = Rect::new(20.0, 10.0, 40.0, 90.0);
    let (top, bot) = (
        Rect::new(160.0, 10.0, 180.0, 30.0),
        Rect::new(160.0, 70.0, 180.0, 90.0),
    );
    let w = world(
        Rect::new(0.0, 0.0, 200.0, 100.0),
        &[a.inflate(C), top.inflate(C), bot.inflate(C)],
    );
    let leg = |link: usize, far: Rect, y: f64| {
        let mut chain = straight(link, a, far, h_chan(&w, 100.0, y));
        chain.ends[0].fan = Some(0);
        Some(chain)
    };
    // Windows: a's side is (18, 82); the two far sides (18, 22) and
    // (78, 82) — disjoint, so the legs share no lawful ordinate.
    let mut chains = vec![leg(0, top, 20.0), leg(1, bot, 80.0)];
    place(&[w], &mut chains, C);
    let ords: Vec<f64> = chains
        .iter()
        .map(|c| c.as_ref().unwrap().runs[0].ord.unwrap())
        .collect();
    assert!(
        ords.iter().all(|o| (18.0..=22.0).contains(o)),
        "the first leg's window stands for the merged fan: {ords:?}"
    );
}

#[test]
fn a_bundle_ladders_centred_on_the_shared_centre() {
    let (w, a, b) = facing();
    let chan = h_chan(&w, 100.0, 50.0);
    let mut chains: Vec<Option<Chain>> = (0..4).map(|i| Some(straight(i, a, b, chan))).collect();
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
                span: (src.centre().1, 160.0),
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

/// A three-run dogleg whose ends leave **opposite** ways: `span` is the
/// interior run's reach, the source at the world's top edge and the target
/// at its bottom, so the interior run wants the channel anchor.
fn dogleg(link: usize, span: (f64, f64)) -> Chain {
    let v = |span| Run {
        axis: Axis::V,
        chan: 0,
        span,
        ord: None,
    };
    Chain {
        link,
        world: 0,
        runs: vec![
            v((10.0, 20.0)),
            Run {
                axis: Axis::H,
                chan: 0,
                span,
                ord: None,
            },
            v((80.0, 90.0)),
        ],
        ends: [
            end(Side::Bottom, Rect::new(span.0 - 20.0, 0.0, span.0, 10.0)),
            end(Side::Top, Rect::new(span.1, 90.0, span.1 + 20.0, 100.0)),
        ],
    }
}

#[test]
fn disjoint_clusters_both_take_the_midline() {
    // Two runs far apart along one channel never cluster: each sits on
    // the channel anchor independently.
    let w = world(Rect::new(0.0, 0.0, 400.0, 100.0), &[]);
    let mut chains = vec![
        Some(dogleg(0, (40.0, 120.0))),
        Some(dogleg(1, (240.0, 320.0))),
    ];
    place(&[w], &mut chains, C);
    let m1 = chains[0].as_ref().unwrap().runs[1].ord.unwrap();
    let m2 = chains[1].as_ref().unwrap().runs[1].ord.unwrap();
    assert_eq!(m1, 50.0, "the empty world's H anchor is its midline");
    assert_eq!(m1, m2, "disjoint spans share the midline in peace");
}

#[test]
fn a_free_branch_forks_where_its_sibling_already_does() {
    // ROUTING.md Special nodes: a fan forks at as few points as it can.
    // One sibling's branch is a jog free to sit anywhere — its corridor's
    // anchor, x = 100 — and the other's is an end run its port pins at
    // x = 130. Law 3 is indifferent (a monotone route costs the same
    // length and turns wherever it bends), so the free branch joins the
    // fixed split: one T, not a split and a turn beside it.
    let a = Rect::new(20.0, 20.0, 40.0, 80.0);
    let b = Rect::new(160.0, 20.0, 180.0, 80.0);
    let c = Rect::new(120.0, 130.0, 140.0, 150.0);
    let w = world(
        Rect::new(0.0, 0.0, 200.0, 160.0),
        &[a.inflate(C), b.inflate(C), c.inflate(C)],
    );
    let hchan = h_chan(&w, 100.0, 50.0);
    // Both branches descend the corridor between a and b — one corridor,
    // sliced into channels by c's keep-out; the run reads the reassembled
    // walls (48, 152), whose midline is x = 100.
    let vchan = w
        .graph
        .v
        .iter()
        .position(|v| v.rect.x0 <= 130.0 && v.rect.x1 >= 130.0 && v.rect.y1 >= 100.0)
        .expect("the V channel over c");
    let h = |span| Run {
        axis: Axis::H,
        chan: hchan,
        span,
        ord: None,
    };
    let v = |span| Run {
        axis: Axis::V,
        chan: vchan,
        span,
        ord: None,
    };
    let mut zig = Chain {
        link: 0,
        world: 0,
        runs: vec![h((40.0, 100.0)), v((48.0, 52.0)), h((100.0, 160.0))],
        ends: [end(Side::Right, a), end(Side::Left, b)],
    };
    let mut drop = Chain {
        link: 1,
        world: 0,
        runs: vec![h((40.0, 130.0)), v((50.0, 130.0))],
        ends: [end(Side::Right, a), end(Side::Top, c)],
    };
    zig.ends[0].fan = Some(0);
    drop.ends[0].fan = Some(0);
    let mut chains = vec![Some(zig), Some(drop)];
    place(&[w], &mut chains, C);
    let fork = chains[1].as_ref().unwrap().runs[1].ord.unwrap();
    assert_eq!(fork, 130.0, "the pinned branch leaves at its own port");
    assert_eq!(
        chains[0].as_ref().unwrap().runs[1].ord,
        Some(fork),
        "and the free branch leaves with it, not on the anchor at x = 100"
    );
}

#[test]
fn a_same_side_u_turns_at_the_outer_side_line_not_the_void() {
    // Both ends leave the same way, so the turn has no reason to travel
    // past the outermost of the two side lines — the anchor would centre
    // it in whatever void the world happens to have, and the drawn bight
    // would then move with the empty space around the pair. One body or
    // two reads the same (ROUTING.md step 5).
    let w = world(Rect::new(0.0, 0.0, 400.0, 100.0), &[]);
    // Two bodies whose bottoms sit at different depths, wired bottom to
    // bottom: the U clears the deeper one and stops.
    let u = |shallow: f64, deep: f64| Chain {
        link: 0,
        world: 0,
        runs: vec![
            Run {
                axis: Axis::V,
                chan: 0,
                span: (shallow, 50.0),
                ord: None,
            },
            Run {
                axis: Axis::H,
                chan: 0,
                span: (40.0, 120.0),
                ord: None,
            },
            Run {
                axis: Axis::V,
                chan: 0,
                span: (deep, 50.0),
                ord: None,
            },
        ],
        ends: [
            end(Side::Bottom, Rect::new(20.0, 0.0, 40.0, shallow)),
            end(Side::Bottom, Rect::new(120.0, 0.0, 140.0, deep)),
        ],
    };
    let mut chains = vec![Some(u(10.0, 30.0))];
    place(&[w], &mut chains, C);
    let ord = chains[0].as_ref().unwrap().runs[1].ord.unwrap();
    assert_eq!(ord, 30.0, "the U turns at the deeper body's own side line");
    // The world's void is not the U's business: widen it and nothing moves.
    let wide = world(Rect::new(0.0, 0.0, 400.0, 4000.0), &[]);
    let mut chains = vec![Some(u(10.0, 30.0))];
    place(&[wide], &mut chains, C);
    assert_eq!(
        chains[0].as_ref().unwrap().runs[1].ord.unwrap(),
        ord,
        "the bight never moves with the empty space around it"
    );
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

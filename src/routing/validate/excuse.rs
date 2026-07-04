//! Law 1's one excuse (ROUTING.md The Four Laws), re-derived from the
//! output alone: a sub-clearance gap stands only where the contested
//! pair's **contention component** — the parallel wires transitively owing
//! each other pitch — demonstrably cannot spread to full clearance. Port
//! windows and the contract's channel model, rebuilt from the placed nodes,
//! grant each wire its lawful ordinate range; feasibility runs in drawn
//! order as a longest-path reach over the contention edges.

use super::{EPS, landing, seg_box};
use crate::ast::Side;
use crate::layout::ir::RoutedLink;
use crate::routing::ortho::cost::min_pitch;
use crate::routing::ortho::graph::{Axis, ChannelGraph};
use crate::routing::ortho::rect::Rect;
use crate::routing::ortho::scene::SceneIndex;

/// Law 1's one excuse, judged on the output alone: the drawn compression is
/// lawful only where the contested pair's **contention component** — the
/// parallel wires transitively owing each other pitch — cannot spread to
/// full clearance. Each wire's lawful ordinate range is re-derived exactly
/// as the law grants it: an end segment answers to its port window
/// tightened by its corridor, an interior run to its corridor's usable
/// width (the contract's channel model rebuilt from the placed nodes; soft
/// shared boundaries surrender half a clearance, ROUTING.md Vocabulary),
/// and a wire outside this world's channels holds its drawn ordinate.
/// Feasibility runs in the drawn order — wires never reorder; braids are
/// unlawful — as a longest-path reach over the contention edges, so a chain
/// pinched at *any* cross-section excuses the group it compresses with. A
/// component that fits at clearance pitch had room to spare: a breach.
pub(super) fn excused(
    index: &SceneIndex,
    links: &[&RoutedLink],
    a: (usize, usize, &[(f64, f64)]),
    b: (usize, usize, &[(f64, f64)]),
    c: f64,
) -> bool {
    let (ai, sk, sa) = a;
    let (bi, tk, sb) = b;
    let (wa, wb) = (links[ai], links[bi]);
    // Only parallel wires with overlapping travel contend for ordinate
    // space; a perpendicular approach in the same compressed group rides
    // its parallel legs' verdict.
    let (abox, bbox) = (seg_box(sa), seg_box(sb));
    let (a_horiz, b_horiz) = (abox.1 == abox.3, bbox.1 == bbox.3);
    if a_horiz != b_horiz {
        return true;
    }
    let horizontal = a_horiz;
    let axis = if horizontal { Axis::H } else { Axis::V };
    let (t0, t1) = if horizontal {
        (abox.0.max(bbox.0), abox.2.min(bbox.2))
    } else {
        (abox.1.max(bbox.1), abox.3.min(bbox.3))
    };
    if t1 <= t0 + EPS {
        return true;
    }
    let mid_t = (t0 + t1) / 2.0;
    let ord_of = |s: &[(f64, f64)]| if horizontal { s[0].1 } else { s[0].0 };
    let mid_ord = (ord_of(sa) + ord_of(sb)) / 2.0;

    // The engine's channel model for the pair's world — bounds minus the
    // world's children, collapsed, exactly as model step 2 builds it —
    // retried one world up while the contested spot lies outside its
    // channels (a link may have routed up the ladder).
    let mut world = common_world(
        &SceneIndex::world_of(&wa.seg_from, &wa.seg_to),
        &SceneIndex::world_of(&wb.seg_from, &wb.seg_to),
    );
    let graph = loop {
        let bounds = if world.is_empty() {
            index.bounds().inflate(2.0 * c + 20.0)
        } else {
            match index.rect(&world) {
                Some(r) => r,
                None => return true,
            }
        };
        let keepouts: Vec<Rect> = index
            .child_rects(&world)
            .iter()
            .map(|r| r.inflate(c))
            .collect();
        let graph = ChannelGraph::build(bounds, &keepouts, world.is_empty());
        if channel_at(&graph, axis, mid_t, mid_ord).is_some() {
            break graph;
        }
        if world.is_empty() {
            // The contested spot lies inside every world's keep-outs — a
            // clearance breach reports it; nothing to judge here.
            return true;
        }
        world = world
            .rfind('.')
            .map_or(String::new(), |i| world[..i].to_owned());
    };

    // Wire nodes: every parallel segment of every link, with its drawn
    // ordinate, travel extent, and lawful range.
    struct Wire {
        link: usize,
        seg: usize,
        ord: f64,
        ext: (f64, f64),
        range: (f64, f64),
    }
    let mut wires: Vec<Wire> = Vec::new();
    for (li, w) in links.iter().enumerate() {
        if w.path.len() < 2 {
            continue;
        }
        let last = w.path.len() - 2;
        for (k, s) in w.path.windows(2).enumerate() {
            let sbox = seg_box(s);
            if (sbox.1 == sbox.3) != horizontal || (sbox.0, sbox.1) == (sbox.2, sbox.3) {
                continue;
            }
            let (lo, hi, o) = if horizontal {
                (sbox.0, sbox.2, sbox.1)
            } else {
                (sbox.1, sbox.3, sbox.0)
            };
            let mut window: Option<(f64, f64)> = None;
            let mut ends = Vec::new();
            if k == 0 {
                ends.push((w.seg_from.as_str(), w.path[0], w.path[1]));
            }
            if k == last {
                ends.push((w.seg_to.as_str(), w.path[last + 1], w.path[last]));
            }
            for (path, port, inward) in ends {
                if let Some(rect) = index.rect(path)
                    && let Ok(side) = landing(rect, port, inward, c)
                {
                    let win = port_window(rect, side, c);
                    window = Some(match window {
                        Some(w) => (w.0.max(win.0), w.1.min(win.1)),
                        None => win,
                    });
                }
            }
            let usable = channel_of(&graph, axis, (lo, hi), o).map(|chan| {
                let corr = graph.corridor(axis, chan, lo, hi);
                let u = corr.usable();
                if u.0 <= u.1 { u } else { corr.walls }
            });
            // Mirror placement's bounds: the port window wins where the
            // corridor's tightening would invert it; a wire this world's
            // channels don't hold keeps its drawn ordinate.
            let range = match (window, usable) {
                (Some(w), Some(u)) => {
                    let tight = (w.0.max(u.0), w.1.min(u.1));
                    if tight.0 <= tight.1 { tight } else { w }
                }
                (Some(w), None) => w,
                (None, Some(u)) => u,
                (None, None) => (o, o),
            };
            wires.push(Wire {
                link: li,
                seg: k,
                ord: o,
                ext: (lo, hi),
                range,
            });
        }
    }
    wires.sort_by(|x, y| {
        x.ord
            .total_cmp(&y.ord)
            .then(x.link.cmp(&y.link))
            .then(x.seg.cmp(&y.seg))
    });

    // Contention edges, exactly the pitch the law charges: overlapping
    // travel, or tips flanking within a clearance — same-wire pieces only
    // when they overlap, fan siblings never (the trunk is one line), and
    // wires already glued below the pitch floor never (they are their own
    // breach, not an excuse for the neighbours').
    let fan_pair = |x: &RoutedLink, y: &RoutedLink| {
        [x.fan_from, x.fan_to]
            .iter()
            .flatten()
            .any(|g| [y.fan_from, y.fan_to].contains(&Some(*g)))
    };
    let m = wires.len();
    let edge = |i: usize, j: usize| -> bool {
        let (x, y) = (&wires[i], &wires[j]);
        if (y.ord - x.ord).abs() < min_pitch(c) - EPS {
            return false;
        }
        if x.link != y.link && fan_pair(links[x.link], links[y.link]) {
            return false;
        }
        let overlap = x.ext.0.max(y.ext.0) < x.ext.1.min(y.ext.1);
        // Inclusive at exactly a clearance, as the engine charges it: a tip
        // gap is a corner until placement settles, so wires flanking at
        // precisely the pitch still owe it.
        let near = y.ext.0 <= x.ext.1 + c + EPS && x.ext.0 <= y.ext.1 + c + EPS;
        if x.link == y.link {
            overlap
        } else {
            overlap || near
        }
    };

    // The contested pair's component over those edges.
    let seed = |li: usize, si: usize| wires.iter().position(|w| w.link == li && w.seg == si);
    let (Some(s0), Some(s1)) = (seed(ai, sk), seed(bi, tk)) else {
        return true;
    };
    let mut in_comp = vec![false; m];
    let mut queue = vec![s0, s1];
    in_comp[s0] = true;
    in_comp[s1] = true;
    while let Some(i) = queue.pop() {
        let grow: Vec<usize> = (0..m)
            .filter(|&j| !in_comp[j] && edge(i.min(j), i.max(j)))
            .collect();
        for j in grow {
            in_comp[j] = true;
            queue.push(j);
        }
    }

    // Feasibility at full clearance: each wire's minimal lawful ordinate is
    // its range floor pushed up by every contending wire below it — exact
    // for interval feasibility at fixed order. One overfull wire anywhere
    // excuses the component it compresses with.
    let mut reach = vec![f64::NEG_INFINITY; m];
    for j in 0..m {
        if !in_comp[j] {
            continue;
        }
        let mut x = wires[j].range.0;
        for i in 0..j {
            if in_comp[i] && edge(i, j) {
                x = x.max(reach[i] + c);
            }
        }
        if x > wires[j].range.1 + EPS {
            return true;
        }
        reach[j] = x;
    }
    false
}

/// The innermost container shared by two world paths (`""` = the root).
fn common_world(a: &str, b: &str) -> String {
    let mut out = String::new();
    for (x, y) in a.split('.').zip(b.split('.')) {
        if x != y || x.is_empty() {
            break;
        }
        if !out.is_empty() {
            out.push('.');
        }
        out.push_str(x);
    }
    out
}

/// The channel of `axis` containing the point `(travel, ordinate)`.
fn channel_at(graph: &ChannelGraph, axis: Axis, travel: f64, ordinate: f64) -> Option<usize> {
    let chans = match axis {
        Axis::H => &graph.h,
        Axis::V => &graph.v,
    };
    chans.iter().position(|ch| {
        let (w0, w1) = ch.walls();
        let (v0, v1) = ch.travel();
        v0 - EPS <= travel && travel <= v1 + EPS && w0 - EPS <= ordinate && ordinate <= w1 + EPS
    })
}

/// The channel a drawn run of `axis` actually rides: walls containing its
/// ordinate, travel containing its midpoint — preferring one whose travel
/// covers the **whole** extent. A wire hugging a wall (walls charge no
/// margin) sits on two channels' shared coordinate, and the neighbour's
/// shorter travel would clamp the corridor query past the very stretch
/// that pins the wire, overstating its lawful range (links_hard at
/// clearance 9: a wall-hugger's tail beside hub read as free to slide
/// west, and a genuinely pinched component was flagged instead of
/// excused).
fn channel_of(graph: &ChannelGraph, axis: Axis, ext: (f64, f64), ordinate: f64) -> Option<usize> {
    let chans = match axis {
        Axis::H => &graph.h,
        Axis::V => &graph.v,
    };
    let mid = (ext.0 + ext.1) / 2.0;
    let holds = |ch: &crate::routing::ortho::graph::Channel| {
        let (w0, w1) = ch.walls();
        let (v0, v1) = ch.travel();
        v0 - EPS <= mid && mid <= v1 + EPS && w0 - EPS <= ordinate && ordinate <= w1 + EPS
    };
    let covers = |i: &usize| {
        let (v0, v1) = chans[*i].travel();
        v0 - EPS <= ext.0 && ext.1 <= v1 + EPS
    };
    let candidates: Vec<usize> = (0..chans.len()).filter(|&i| holds(&chans[i])).collect();
    candidates
        .iter()
        .find(|i| covers(i))
        .or(candidates.first())
        .copied()
}

/// The lawful port window along `side`: the side minus a `clearance` corner
/// margin at each end, the margin relaxing to half the side on sides too
/// short for it.
fn port_window(rect: Rect, side: Side, c: f64) -> (f64, f64) {
    let (lo, hi) = match side {
        Side::Left | Side::Right => (rect.y0, rect.y1),
        _ => (rect.x0, rect.x1),
    };
    let m = c.min((hi - lo) / 2.0);
    (lo + m, hi - m)
}

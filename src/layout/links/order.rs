//! The run-order comparator (ROUTING §Model step 5, PLAN §Run ordering).
//!
//! Two links sharing a channel are ordered by where their paths diverge: walk
//! both outward from the shared run; the first divergence — a turn off the
//! channel, or a pinned terminal — decides. A link turning toward the
//! positive ordinate sweeps that side, so it must sit on it (`n > 0` ⇒
//! larger); equal turns recurse into the next channel with the nesting sign.
//! Every remaining tie breaks on declaration order, so the order is total.

use super::graph::Axis;
use super::runs::{Chain, Conn, Pin};
use crate::ast::Side;
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug)]
enum Ev {
    /// Leaves the channel at `q`, sweeping toward `n` on the ordinate axis;
    /// `cell` identifies the junction, `next` the channel it turns into
    /// (axis distinguishes a real turn from an inversion jog's same-axis
    /// continuation).
    Turn {
        q: f64,
        cell: usize,
        n: i8,
        next: (Axis, usize),
    },
    /// The run ends at `q`; `pin` is its (pinned) ordinate.
    Term { q: f64, pin: f64 },
}

/// A walk position: chain `ci`, run `ri`, and which conn (0/1) we exit by.
#[derive(Clone, Copy)]
struct Cursor {
    ci: usize,
    ri: usize,
    e: usize,
}

fn pin_value(chain: &Chain, ri: usize) -> f64 {
    match chain.runs[ri].pin {
        Pin::Fixed(v) => v,
        _ => chain.runs[ri].ord,
    }
}

fn event(chains: &[Option<Chain>], cur: Cursor) -> Ev {
    let chain = chains[cur.ci].as_ref().unwrap();
    let run = &chain.runs[cur.ri];
    match run.conn[cur.e] {
        Conn::Terminal { q } => Ev::Term {
            q,
            pin: pin_value(chain, cur.ri),
        },
        Conn::Junction { cell, q } => {
            let ni = if cur.e == 1 { cur.ri + 1 } else { cur.ri - 1 };
            let next = &chain.runs[ni];
            let (near, far) = if cur.e == 1 {
                (next.conn[0].q(), next.conn[1].q())
            } else {
                (next.conn[1].q(), next.conn[0].q())
            };
            let n = if far >= near { 1 } else { -1 };
            Ev::Turn {
                q,
                cell,
                n,
                next: (next.axis, next.chan),
            }
        }
    }
}

fn advance(cur: Cursor) -> Cursor {
    Cursor {
        ci: cur.ci,
        ri: if cur.e == 1 { cur.ri + 1 } else { cur.ri - 1 },
        e: cur.e,
    }
}

fn flip(o: Ordering, m: i8) -> Ordering {
    if m < 0 { o.reverse() } else { o }
}

/// One simultaneous outward walk. Returns the order and whether it came from
/// a geometric constraint (`true`) or a convention on unconstrained pairs.
fn walk(chains: &[Option<Chain>], a: Cursor, b: Cursor, dir: i8) -> (Ordering, bool) {
    let (mut ca, mut cb) = (a, b);
    let mut sigma = dir;
    let mut m: i8 = 1;
    loop {
        let (ea, eb) = (event(chains, ca), event(chains, cb));
        match (ea, eb) {
            (
                Ev::Turn {
                    cell: la,
                    n: na,
                    next: xa,
                    ..
                },
                Ev::Turn {
                    cell: lb,
                    n: nb,
                    next: xb,
                    ..
                },
            ) if la == lb && na == nb && xa == xb => {
                m *= -(sigma * na);
                sigma = na;
                ca = advance(ca);
                cb = advance(cb);
            }
            (Ev::Turn { q: qa, n: na, .. }, Ev::Turn { q: qb, n: nb, .. }) => {
                let (sa, sb) = (qa * sigma as f64, qb * sigma as f64);
                let o = if sa < sb {
                    if na > 0 {
                        Ordering::Greater
                    } else {
                        Ordering::Less
                    }
                } else if sb < sa {
                    if nb > 0 {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    }
                } else if na != nb {
                    na.cmp(&nb)
                } else {
                    return (flip(tie(chains, ca.ci, cb.ci), m), false);
                };
                return (flip(o, m), true);
            }
            (Ev::Turn { q, n, .. }, Ev::Term { q: qt, .. }) => {
                let o = if n > 0 {
                    Ordering::Greater
                } else {
                    Ordering::Less
                };
                return (flip(o, m), qt * sigma as f64 >= q * sigma as f64);
            }
            (Ev::Term { q: qt, .. }, Ev::Turn { q, n, .. }) => {
                let o = if n > 0 {
                    Ordering::Less
                } else {
                    Ordering::Greater
                };
                return (flip(o, m), qt * sigma as f64 >= q * sigma as f64);
            }
            (Ev::Term { pin: pa, .. }, Ev::Term { pin: pb, .. }) => {
                let o = pa.total_cmp(&pb);
                if o != Ordering::Equal {
                    return (flip(o, m), true);
                }
                return (flip(tie(chains, ca.ci, cb.ci), m), false);
            }
        }
    }
}

fn tie(chains: &[Option<Chain>], a: usize, b: usize) -> Ordering {
    let (ra, rb) = (
        chains[a].as_ref().unwrap().req,
        chains[b].as_ref().unwrap().req,
    );
    ra.cmp(&rb).then(a.cmp(&b))
}

/// Which conn of a run faces the channel's positive end.
fn plus_end(chain: &Chain, ri: usize) -> usize {
    let r = &chain.runs[ri];
    usize::from(r.conn[1].q() >= r.conn[0].q())
}

/// The walk cursor for a run, facing the channel's `dir` end.
fn cursor(chains: &[Option<Chain>], (ci, ri): (usize, usize), dir: i8) -> Cursor {
    let plus = plus_end(chains[ci].as_ref().unwrap(), ri);
    Cursor {
        ci,
        ri,
        e: if dir > 0 { plus } else { 1 - plus },
    }
}

/// Total cross order of two runs sharing a channel: the positive-end walk
/// wins when geometric, then the negative-end walk, then convention.
pub fn cmp_runs(chains: &[Option<Chain>], a: (usize, usize), b: (usize, usize)) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    let (op, rp) = walk(chains, cursor(chains, a, 1), cursor(chains, b, 1), 1);
    if rp {
        return op;
    }
    let (om, rm) = walk(chains, cursor(chains, a, -1), cursor(chains, b, -1), -1);
    if rm {
        return om;
    }
    op
}

/// Whether two runs sharing a channel are **inverted** — both outward walks
/// are geometric and demand opposite orders, so the pair must cross exactly
/// once (ROUTING §Model step 5).
pub fn inverted(chains: &[Option<Chain>], a: (usize, usize), b: (usize, usize)) -> bool {
    if a == b {
        return false;
    }
    let (op, rp) = walk(chains, cursor(chains, a, 1), cursor(chains, b, 1), 1);
    let (om, rm) = walk(chains, cursor(chains, a, -1), cursor(chains, b, -1), -1);
    rp && rm && op != om
}

/// Order of two chain ends along their shared side (Law 2: port order equals
/// lane order, so links never braid at the mouth). Walks outward from the
/// side; the first run is the approach when it runs along the stub axis, or
/// counts as an immediate turn (self-loops) when it does not.
pub fn cmp_ends(chains: &[Option<Chain>], a: (usize, usize), b: (usize, usize)) -> Ordering {
    let outward = |(ci, end): (usize, usize)| -> (Cursor, i8, bool) {
        let chain = chains[ci].as_ref().unwrap();
        let ri = if end == 0 { 0 } else { chain.runs.len() - 1 };
        let e = 1 - end;
        let stub_axis_is_run_axis = {
            let along_x = matches!(chain.ends[end].side, Side::Left | Side::Right);
            let run_h = chain.runs[ri].axis == Axis::H;
            along_x == run_h
        };
        let sigma: i8 = match chain.ends[end].side {
            Side::Right | Side::Bottom => 1,
            Side::Left | Side::Top => -1,
        };
        (Cursor { ci, ri, e }, sigma, stub_axis_is_run_axis)
    };
    let (ca, sa, aa) = outward(a);
    let (cb, sb, ab) = outward(b);
    debug_assert_eq!(sa, sb);
    match (aa, ab) {
        (true, true) => walk(chains, ca, cb, sa).0,
        // A perpendicular first run is an immediate turn at the keep-out edge:
        // it sweeps its travel direction, so it takes that flank of the side.
        (false, true) | (true, false) => {
            let perp = if aa { b } else { a };
            let chain = chains[perp.0].as_ref().unwrap();
            let ri = if perp.1 == 0 { 0 } else { chain.runs.len() - 1 };
            let (near, far) = if perp.1 == 0 {
                (chain.runs[ri].conn[0].q(), chain.runs[ri].conn[1].q())
            } else {
                (chain.runs[ri].conn[1].q(), chain.runs[ri].conn[0].q())
            };
            let n: i8 = if far >= near { 1 } else { -1 };
            let o = if n > 0 {
                Ordering::Greater
            } else {
                Ordering::Less
            };
            if aa { o.reverse() } else { o }
        }
        (false, false) => tie(chains, a.0, b.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::links::geometry;
    use crate::layout::links::graph::ChannelGraph;
    use crate::layout::links::path;
    use crate::layout::links::rect::Rect;
    use crate::layout::links::runs::EndInfo;
    use std::cmp::Ordering;

    /// Deterministic LCG — no RNG dependency, identical every run.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self, bound: usize) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 33) as usize) % bound
        }
    }

    type RunRefs = Vec<(usize, usize)>;

    /// Route many random node pairs over a 3×3 block grid and collect the
    /// chains — real paths, real channels, the comparator's actual domain.
    fn grid_chains() -> (Vec<Option<Chain>>, Vec<RunRefs>) {
        let bounds = Rect::new(0.0, 0.0, 400.0, 400.0);
        let mut bodies = Vec::new();
        for gy in 0..3 {
            for gx in 0..3 {
                let (x, y) = (60.0 + 120.0 * gx as f64, 60.0 + 120.0 * gy as f64);
                bodies.push(Rect::new(x, y, x + 60.0, y + 40.0));
            }
        }
        let keepouts: Vec<Rect> = bodies.iter().map(|b| b.inflate(8.0)).collect();
        let graph = ChannelGraph::build(bounds, &keepouts, false);

        let mut rng = Lcg(7);
        let mut chains: Vec<Option<Chain>> = Vec::new();
        for req in 0..40 {
            let (ai, bi) = (rng.next(9), rng.next(9));
            if ai == bi {
                continue;
            }
            let (a, b) = (bodies[ai], bodies[bi]);
            let starts = path::entries(&graph, a, 8.0, None, &[], false);
            let goals = path::entries(&graph, b, 8.0, None, &[], false);
            let Some(route) =
                path::shortest(&graph, &starts, &goals, &|_, _, _, _| false, path::FREE)
            else {
                continue;
            };
            let (se, ge) = (&starts[route.start], &goals[route.goal]);
            let ends = [(a, se), (b, ge)].map(|(rect, e)| EndInfo {
                path: String::new(),
                side: e.side,
                rect,
                port: e.port,
                fan: None,
            });
            chains.push(Some(geometry::chain(
                &graph,
                0,
                &route.cells,
                se,
                ge,
                ends,
                req,
                false,
            )));
        }

        let mut groups: std::collections::BTreeMap<(u8, usize), Vec<(usize, usize)>> =
            std::collections::BTreeMap::new();
        for (ci, c) in chains.iter().enumerate() {
            let c = c.as_ref().unwrap();
            for (ri, r) in c.runs.iter().enumerate() {
                let axis = match r.axis {
                    crate::layout::links::graph::Axis::H => 0u8,
                    crate::layout::links::graph::Axis::V => 1u8,
                };
                groups.entry((axis, r.chan)).or_default().push((ci, ri));
            }
        }
        (chains, groups.into_values().collect())
    }

    #[test]
    fn comparator_is_reflexive_antisymmetric_and_transitive() {
        let (chains, groups) = grid_chains();
        let mut pairs = 0;
        for runs in &groups {
            for &a in runs {
                assert_eq!(cmp_runs(&chains, a, a), Ordering::Equal);
                for &b in runs {
                    if a == b {
                        continue;
                    }
                    let ab = cmp_runs(&chains, a, b);
                    let ba = cmp_runs(&chains, b, a);
                    assert_eq!(ab, ba.reverse(), "antisymmetry {a:?} {b:?}");
                    assert_ne!(ab, Ordering::Equal, "distinct runs must order {a:?} {b:?}");
                    pairs += 1;
                }
            }
            for &a in runs {
                for &b in runs {
                    for &c in runs {
                        if a == b || b == c || a == c {
                            continue;
                        }
                        if cmp_runs(&chains, a, b) == Ordering::Less
                            && cmp_runs(&chains, b, c) == Ordering::Less
                        {
                            assert_eq!(
                                cmp_runs(&chains, a, c),
                                Ordering::Less,
                                "transitivity {a:?} {b:?} {c:?}"
                            );
                        }
                    }
                }
            }
        }
        assert!(
            pairs > 100,
            "the grid must exercise shared channels: {pairs}"
        );
    }
}

//! The run-order mechanism (ROUTING.md model step 5): wires leave in the
//! order they arrive — nested, never braided.
//!
//! Two runs sharing a channel are **judged** by where their chains diverge:
//! walk both chains outward from the shared channel; the first divergence —
//! a turn off the channel, or a terminal port — decides. A wire turning
//! toward the positive ordinate sweeps that flank, so it sits on it; wires
//! turning together into one channel recurse there, the orientation flipping
//! as corners invert. Ordinates are placement *estimates* (port preferences,
//! channel anchors — placement hasn't run yet), the same estimates the
//! search priced, so the order placement realises is the order the search
//! paid for. A pair no geometry orders — exact parallels, tied at both
//! ends — resolves by one oriented convention: the earlier-declared wire
//! keeps the left of its own travel, so every channel the pair shares
//! agrees on the same nesting (an offset curve never braids).
//!
//! Each judgment is sound for its pair, but judgments need not compose: a
//! third wire can be walked *between* the two end runs of a chain that
//! revisits the corridor, contradicting the same-chain convention, and a
//! braid-forced knot can cycle outright — no comparator over these
//! judgments is transitive in general (the unit test pins links_hard's
//! triple). So the cluster's total order is built whole, by [`ranks`]:
//! geometric judgments bind, conventions rank what geometry leaves free,
//! declaration settles ties.

use std::cmp::Ordering;

use super::Chain;
use super::graph::Axis;

/// The walk's read-only view: the routed chains and each run's ordinate
/// estimate.
pub(crate) struct Ctx<'a> {
    pub chains: &'a [Option<Chain>],
    pub ests: &'a [Vec<f64>],
}

/// A walk position: chain `ci`, run `ri`, exiting toward `ends[e]`.
#[derive(Clone, Copy)]
struct Cursor {
    ci: usize,
    ri: usize,
    e: usize,
}

#[derive(Clone, Copy, Debug)]
enum Ev {
    /// Leaves the channel at travel coordinate `q`, sweeping toward `n` on
    /// the ordinate axis, into channel `next`.
    Turn { q: f64, n: i8, next: (Axis, usize) },
    /// The chain ends at travel coordinate `q`, its port estimated at `pin`.
    Term { q: f64, pin: f64 },
}

fn neighbour(chain: &Chain, ri: usize, e: usize) -> Option<usize> {
    if e == 1 {
        (ri + 1 < chain.runs.len()).then_some(ri + 1)
    } else {
        ri.checked_sub(1)
    }
}

impl Ctx<'_> {
    fn chain(&self, ci: usize) -> &Chain {
        self.chains[ci].as_ref().expect("routed chain")
    }

    fn est(&self, ci: usize, ri: usize) -> f64 {
        self.ests[ci][ri]
    }

    /// Where run `ri` ends on side `e`, along its travel axis: the next
    /// run's estimate, or the chain's side line at a terminal.
    fn end_q(&self, ci: usize, ri: usize, e: usize) -> f64 {
        let chain = self.chain(ci);
        match neighbour(chain, ri, e) {
            Some(ni) => self.est(ci, ni),
            None => chain.ends[e].side_coord(),
        }
    }

    fn event(&self, cur: Cursor) -> Ev {
        let chain = self.chain(cur.ci);
        match neighbour(chain, cur.ri, cur.e) {
            None => Ev::Term {
                q: chain.ends[cur.e].side_coord(),
                pin: self.est(cur.ci, cur.ri),
            },
            Some(ni) => {
                let near = self.est(cur.ci, cur.ri);
                let far = self.end_q(cur.ci, ni, cur.e);
                Ev::Turn {
                    q: self.est(cur.ci, ni),
                    n: if far >= near { 1 } else { -1 },
                    next: (chain.runs[ni].axis, chain.runs[ni].chan),
                }
            }
        }
    }

    /// Declaration-order tie for unconstrained pairs.
    fn tie(&self, a: usize, b: usize) -> Ordering {
        self.chain(a).link.cmp(&self.chain(b).link).then(a.cmp(&b))
    }
}

fn advance(cur: Cursor) -> Cursor {
    Cursor {
        ri: if cur.e == 1 { cur.ri + 1 } else { cur.ri - 1 },
        ..cur
    }
}

fn flip(o: Ordering, m: i8) -> Ordering {
    if m < 0 { o.reverse() } else { o }
}

/// One simultaneous outward walk. Returns the order and whether it came from
/// a geometric constraint (`true`) or a convention on unconstrained pairs.
fn walk(ctx: &Ctx, a: Cursor, b: Cursor, dir: i8) -> (Ordering, bool) {
    let (mut ca, mut cb) = (a, b);
    let mut sigma = dir;
    let mut m: i8 = 1;
    loop {
        let (ea, eb) = (ctx.event(ca), ctx.event(cb));
        match (ea, eb) {
            (
                Ev::Turn {
                    q: qa,
                    n: na,
                    next: xa,
                },
                Ev::Turn {
                    q: qb,
                    n: nb,
                    next: xb,
                },
            ) if qa.total_cmp(&qb) == Ordering::Equal && na == nb && xa == xb => {
                m *= -(sigma * na);
                sigma = na;
                ca = advance(ca);
                cb = advance(cb);
            }
            (Ev::Turn { q: qa, n: na, .. }, Ev::Turn { q: qb, n: nb, .. }) => {
                let (sa, sb) = (qa * f64::from(sigma), qb * f64::from(sigma));
                let o = match sa.total_cmp(&sb) {
                    Ordering::Less => {
                        if na > 0 {
                            Ordering::Greater
                        } else {
                            Ordering::Less
                        }
                    }
                    Ordering::Greater => {
                        if nb > 0 {
                            Ordering::Less
                        } else {
                            Ordering::Greater
                        }
                    }
                    Ordering::Equal if na != nb => na.cmp(&nb),
                    Ordering::Equal => return (flip(ctx.tie(ca.ci, cb.ci), m), false),
                };
                return (flip(o, m), true);
            }
            (Ev::Turn { q, n, .. }, Ev::Term { q: qt, .. }) => {
                let o = if n > 0 {
                    Ordering::Greater
                } else {
                    Ordering::Less
                };
                return (flip(o, m), qt * f64::from(sigma) >= q * f64::from(sigma));
            }
            (Ev::Term { q: qt, .. }, Ev::Turn { q, n, .. }) => {
                let o = if n > 0 {
                    Ordering::Less
                } else {
                    Ordering::Greater
                };
                return (flip(o, m), qt * f64::from(sigma) >= q * f64::from(sigma));
            }
            (Ev::Term { pin: pa, .. }, Ev::Term { pin: pb, .. }) => {
                let o = pa.total_cmp(&pb);
                if o != Ordering::Equal {
                    return (flip(o, m), true);
                }
                return (flip(ctx.tie(ca.ci, cb.ci), m), false);
            }
        }
    }
}

/// The convention for pairs no geometry orders: the earlier-declared wire
/// takes the **left of its own travel** through the queried channel — a
/// V run heading down sits at greater x, heading up at lesser x; an H run
/// heading right sits at lesser y, heading left at greater y. One fixed
/// side per travel direction is the offset-curve rule: every channel an
/// exact-parallel pair shares resolves to the same nesting, where a fixed
/// declaration order flipped by each walk's parity braids the pair.
fn convention(ctx: &Ctx, a: (usize, usize), b: (usize, usize)) -> Ordering {
    let led = ctx.tie(a.0, b.0) == Ordering::Less;
    let (ci, ri) = if led { a } else { b };
    let downstream = ctx.end_q(ci, ri, 1) >= ctx.end_q(ci, ri, 0);
    let leader = match (ctx.chain(ci).runs[ri].axis, downstream) {
        (Axis::V, true) | (Axis::H, false) => Ordering::Greater,
        _ => Ordering::Less,
    };
    if led { leader } else { leader.reverse() }
}

/// The walk cursor for a run, facing the channel's `dir` end.
fn cursor(ctx: &Ctx, (ci, ri): (usize, usize), dir: i8) -> Cursor {
    let plus = usize::from(ctx.end_q(ci, ri, 1) >= ctx.end_q(ci, ri, 0));
    Cursor {
        ci,
        ri,
        e: if dir > 0 { plus } else { 1 - plus },
    }
}

/// The pairwise judgment of two runs sharing a channel, and whether it is
/// **geometric** (a real anti-braid constraint) or a convention: the
/// positive-end walk wins when geometric, then the negative-end walk, then
/// [`convention`]. Two pieces of one wire owe each other no nesting — they
/// order by estimate, then run index.
fn judge(ctx: &Ctx, a: (usize, usize), b: (usize, usize)) -> (Ordering, bool) {
    if a.0 == b.0 {
        let o = ctx
            .est(a.0, a.1)
            .total_cmp(&ctx.est(b.0, b.1))
            .then(a.1.cmp(&b.1));
        return (o, false);
    }
    let (op, gp) = walk(ctx, cursor(ctx, a, 1), cursor(ctx, b, 1), 1);
    if gp {
        return (op, true);
    }
    let (om, gm) = walk(ctx, cursor(ctx, a, -1), cursor(ctx, b, -1), -1);
    if gm {
        return (om, true);
    }
    (convention(ctx, a, b), false)
}

/// Sort positions for one cluster's items: preference first (`total_cmp`),
/// then, within each preference class, a linear extension of the pairwise
/// judgments — a genuine total order where a pairwise comparator is not.
///
/// The extension emits, repeatedly, the item no remaining item precedes
/// **geometrically**; among those free items the one preceding the most
/// (judgments of any strength), declaration order last. Where the judgments
/// are consistent — every cluster the old comparator sorted without
/// contradiction — this is their unique total order, so nothing already
/// lawful moves. Where they knot, geometry yields as little as the
/// tournament allows and the surrendered pair braids honestly (a knot *is*
/// a braid no order avoids); a contradicted convention costs nothing.
///
/// `runs[i]` is item `i`'s walk representative (a fan's merged item walks
/// as its first member), `prefs[i]` its preference. Returns each item's
/// position in the settled order.
pub(crate) fn ranks(ctx: &Ctx, runs: &[(usize, usize)], prefs: &[f64]) -> Vec<usize> {
    let n = runs.len();
    let mut by_pref: Vec<usize> = (0..n).collect();
    by_pref.sort_by(|&a, &b| prefs[a].total_cmp(&prefs[b]).then(a.cmp(&b)));
    let mut pos = vec![0; n];
    let (mut p, mut lo) = (0, 0);
    while lo < n {
        let hi = (lo..n)
            .take_while(|&i| prefs[by_pref[i]].total_cmp(&prefs[by_pref[lo]]) == Ordering::Equal)
            .count()
            + lo;
        for i in extend(ctx, runs, &by_pref[lo..hi]) {
            pos[i] = p;
            p += 1;
        }
        lo = hi;
    }
    pos
}

/// Linearly extend one preference class's judgments; returns the class's
/// item indices in emitted order.
fn extend(ctx: &Ctx, runs: &[(usize, usize)], class: &[usize]) -> Vec<usize> {
    let m = class.len();
    if m == 1 {
        return vec![class[0]];
    }
    let mut verdict = vec![vec![(Ordering::Equal, false); m]; m];
    for i in 0..m {
        for j in i + 1..m {
            let (o, g) = judge(ctx, runs[class[i]], runs[class[j]]);
            verdict[i][j] = (o, g);
            verdict[j][i] = (o.reverse(), g);
        }
    }
    let decl = |i: usize| {
        let (ci, ri) = runs[class[i]];
        (ctx.chain(ci).link, ci, ri)
    };
    let mut remaining: Vec<usize> = (0..m).collect();
    let mut out = Vec::with_capacity(m);
    while !remaining.is_empty() {
        let free: Vec<usize> = remaining
            .iter()
            .copied()
            .filter(|&i| {
                remaining
                    .iter()
                    .all(|&j| j == i || verdict[j][i] != (Ordering::Less, true))
            })
            .collect();
        // No free item means the geometric constraints themselves cycle —
        // a braid no order avoids; the whole remainder competes.
        let pool = if free.is_empty() {
            remaining.clone()
        } else {
            free
        };
        let pick = pool
            .iter()
            .copied()
            .max_by(|&a, &b| {
                let wins = |i: usize| {
                    pool.iter()
                        .filter(|&&j| j != i && verdict[i][j].0 == Ordering::Less)
                        .count()
                };
                wins(a).cmp(&wins(b)).then_with(|| decl(b).cmp(&decl(a)))
            })
            .expect("non-empty pool");
        remaining.retain(|&i| i != pick);
        out.push(class[pick]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::rect::Rect;
    use super::super::{Chain, EndInfo, Run};
    use super::*;
    use crate::ast::Side;

    fn run(axis: Axis, chan: usize, span: (f64, f64)) -> Run {
        Run {
            axis,
            chan,
            span,
            ord: None,
        }
    }

    /// An end whose `side` line sits at `coord`; the walk reads nothing
    /// else of the rect.
    fn end(side: Side, coord: f64) -> EndInfo {
        let rect = match side {
            Side::Left => Rect::new(coord, -10.0, coord + 20.0, 10.0),
            Side::Right => Rect::new(coord - 20.0, -10.0, coord, 10.0),
            Side::Top => Rect::new(-10.0, coord, 10.0, coord + 20.0),
            Side::Bottom => Rect::new(-10.0, coord - 20.0, 10.0, coord),
        };
        EndInfo {
            side,
            rect,
            window: (-10.0, 10.0),
            fan: None,
        }
    }

    /// links_hard at clearance 6, reduced: chain `u` revisits the middle
    /// corridor (both its end runs land there at ordinate 0), and chain
    /// `f` is walked geometrically *between* them — `u4 < f0` and
    /// `f0 < u0` — while the same-chain convention (est tie → run index)
    /// says `u4 > u0`. No pairwise comparator survives that triple (the
    /// old one panicked Rust's sort); the extension must honour both
    /// geometric constraints and surrender the convention.
    #[test]
    fn a_run_walked_between_a_revisiting_chains_ends_ranks_consistently() {
        let f = Chain {
            link: 0,
            world: 0,
            runs: vec![
                run(Axis::H, 25, (38.5, 69.5)),
                run(Axis::V, 22, (-237.5, 0.0)),
                run(Axis::H, 26, (34.0, 69.5)),
            ],
            ends: [end(Side::Right, 38.5), end(Side::Right, 34.0)],
        };
        let u = Chain {
            link: 17,
            world: 0,
            runs: vec![
                run(Axis::H, 25, (61.5, 84.5)),
                run(Axis::V, 21, (-72.0, 0.0)),
                run(Axis::H, 19, (4.0, 61.5)),
                run(Axis::V, 11, (-72.0, 0.0)),
                run(Axis::H, 20, (-76.5, 4.0)),
            ],
            ends: [end(Side::Left, 84.5), end(Side::Right, -76.5)],
        };
        let chains = vec![Some(f), Some(u)];
        let ests = vec![vec![0.0, 69.5, -237.5], vec![0.0, 61.5, -72.0, -53.5, 0.0]];
        let ctx = Ctx {
            chains: &chains,
            ests: &ests,
        };
        assert_eq!(judge(&ctx, (1, 4), (0, 0)), (Ordering::Less, true));
        assert_eq!(judge(&ctx, (0, 0), (1, 0)), (Ordering::Less, true));
        assert_eq!(judge(&ctx, (1, 4), (1, 0)), (Ordering::Greater, false));
        let pos = ranks(&ctx, &[(1, 4), (0, 0), (1, 0)], &[0.0; 3]);
        assert!(
            pos[0] < pos[1] && pos[1] < pos[2],
            "geometric nesting must hold: {pos:?}"
        );
    }

    /// Preference stays the primary key: items in different preference
    /// classes never consult the walk.
    #[test]
    fn preference_orders_across_classes() {
        let straight = |link: usize, y: f64| Chain {
            link,
            world: 0,
            runs: vec![run(Axis::H, 0, (20.0, 80.0))],
            ends: [end(Side::Right, 20.0), {
                let mut e = end(Side::Left, 80.0);
                e.window = (y - 10.0, y + 10.0);
                e
            }],
        };
        let chains = vec![Some(straight(0, 30.0)), Some(straight(1, 10.0))];
        let ests = vec![vec![30.0], vec![10.0]];
        let ctx = Ctx {
            chains: &chains,
            ests: &ests,
        };
        let pos = ranks(&ctx, &[(0, 0), (1, 0)], &[30.0, 10.0]);
        assert_eq!(pos, vec![1, 0]);
    }
}

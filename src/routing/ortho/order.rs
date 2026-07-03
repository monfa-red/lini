//! The run-order comparator (ROUTING.md model step 5): wires leave in the
//! order they arrive — nested, never braided.
//!
//! Two runs sharing a channel are ordered by where their chains diverge:
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
//! agrees on the same nesting (an offset curve never braids). Total either
//! way.

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

/// Total cross order of two runs sharing a channel: the positive-end walk
/// wins when geometric, then the negative-end walk, then [`convention`].
/// Two pieces of one wire owe each other no nesting — they order by estimate.
pub(crate) fn cmp_runs(ctx: &Ctx, a: (usize, usize), b: (usize, usize)) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    if a.0 == b.0 {
        return ctx
            .est(a.0, a.1)
            .total_cmp(&ctx.est(b.0, b.1))
            .then(a.1.cmp(&b.1));
    }
    let (op, gp) = walk(ctx, cursor(ctx, a, 1), cursor(ctx, b, 1), 1);
    if gp {
        return op;
    }
    let (om, gm) = walk(ctx, cursor(ctx, a, -1), cursor(ctx, b, -1), -1);
    if gm {
        return om;
    }
    convention(ctx, a, b)
}

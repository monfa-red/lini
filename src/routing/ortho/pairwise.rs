//! The general pairwise settle (ROUTING.md model step 5): the projection
//! [`place`](super::place) falls to when a cluster's contention is not a
//! chain — a zero-gap bridge dissolves the chain model's separations, so
//! each contending pair owes its pitch directly, and the ordinates are the
//! least-squares projection of the preferences onto the feasible set.

use super::cost::min_pitch;
use super::graph::Corridor;
use super::place::{Item, owed};

/// The general settle for clusters the chain cannot express: each
/// contending pair — and only those — owes its pitch ([`owed`]: the
/// distance model — full clearance alongside, the diagonal remainder past
/// each other), signed by the cluster order (nested, never braided);
/// non-contending items stay uncoupled, free to share ordinate space. Relief first makes the system feasible (the
/// same uniform compression, applied along the tightest constraint chains),
/// then the ordinates are the least-squares projection of the preferences
/// onto the feasible set (Dykstra's alternating projections — exact in the
/// limit, run to well below geometric tolerance, deterministic).
pub(super) fn pairwise(
    cluster: &[(Item, Corridor)],
    prefs: &[f64],
    bounds: &[(f64, f64)],
    clearance: f64,
) -> Vec<f64> {
    let n = cluster.len();
    let mut gaps: Vec<(usize, usize, f64)> = Vec::new();
    for i in 0..n {
        for j in i + 1..n {
            let owes = owed(&cluster[i].0, &cluster[j].0, clearance, clearance);
            if owes > 0.0 {
                gaps.push((i, j, owes));
            }
        }
    }

    // Feasibility relief, judged exactly: each item's minimal lawful
    // ordinate is its box floor pushed up by every contender below it
    // (staggered boxes lend their room — an envelope test would squeeze
    // stretches that actually fit). When a chain overruns a box, the gaps
    // riding it compress uniformly toward what fits, floored at half the
    // clearance; floors bound the loop.
    for _ in 0..64 {
        let mut reach = vec![f64::NEG_INFINITY; n];
        let mut origin: Vec<usize> = (0..n).collect();
        let mut via: Vec<Option<usize>> = vec![None; n];
        let mut violated = None;
        'feasible: for j in 0..n {
            let mut x = bounds[j].0;
            for (e, &(i, jj, g)) in gaps.iter().enumerate() {
                if jj == j && reach[i] + g > x {
                    x = reach[i] + g;
                    origin[j] = origin[i];
                    via[j] = Some(e);
                }
            }
            reach[j] = x;
            if x > bounds[j].1 + 1e-9 {
                violated = Some(j);
                break 'feasible;
            }
        }
        let Some(t) = violated else { break };
        let mut path = Vec::new();
        let mut at = t;
        while let Some(e) = via[at] {
            path.push(e);
            at = gaps[e].0;
        }
        let avail = (bounds[t].1 - bounds[origin[t]].0).max(0.0);
        let target = (avail / path.len().max(1) as f64).max(min_pitch(clearance));
        let mut lowered = false;
        for e in path {
            if gaps[e].2 > target {
                gaps[e].2 = target;
                lowered = true;
            }
        }
        if !lowered {
            break;
        }
    }

    // Dykstra: project the preferences onto the boxes and every gap
    // halfspace in turn, with per-constraint corrections, until a full
    // sweep moves nothing.
    let mut x = prefs.to_vec();
    let mut box_corr = vec![0.0; n];
    let mut gap_corr = vec![0.0; gaps.len()];
    for _ in 0..10_000 {
        let mut moved = 0.0_f64;
        for i in 0..n {
            let y = x[i] + box_corr[i];
            let p = y.max(bounds[i].0).min(bounds[i].1);
            box_corr[i] = y - p;
            moved = moved.max((p - x[i]).abs());
            x[i] = p;
        }
        for (e, &(i, j, g)) in gaps.iter().enumerate() {
            let (yi, yj) = (x[i] + gap_corr[e] / 2.0, x[j] - gap_corr[e] / 2.0);
            let short = g - (yj - yi);
            let d = short.max(0.0);
            gap_corr[e] = d;
            let (pi, pj) = (yi - d / 2.0, yj + d / 2.0);
            moved = moved.max((pi - x[i]).abs()).max((pj - x[j]).abs());
            x[i] = pi;
            x[j] = pj;
        }
        if moved < 1e-11 {
            break;
        }
    }
    // A system the relief could not make feasible even at the floors — the
    // admission's cross-window blind spot (ROUTING-LOG.md execution log) —
    // leaves Dykstra splitting the shortfall across every constraint.
    // Windows and walls are absolute law; pitch below them is at least
    // visible. Bounds win, and the gaps carry the debt.
    for i in 0..n {
        x[i] = x[i].max(bounds[i].0).min(bounds[i].1);
    }
    x
}

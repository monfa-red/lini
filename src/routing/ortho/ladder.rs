//! The placement primitive (ROUTING.md model step 5): order-preserving
//! ordinates minimizing Σ(x_i − pref_i)², subject to x_{i+1} − x_i ≥ sep_i
//! and lo_i ≤ x_i ≤ hi_i. The solution is unique (strictly convex objective
//! over a convex set), so Law 4 holds by construction.
//!
//! Substituting y_i = x_i − Σ_{j<i} sep_j turns minimum separation into plain
//! monotonicity: **bounded** isotonic regression. Pool-adjacent-violators
//! generalizes to separable convex objectives — each item's objective is
//! (y − p_i)² plus its box's indicator, so a block's minimizer is its mean
//! clipped into the intersection of its members' boxes; blocks pool while
//! the clipped values violate monotonicity. (Clipping the *unbounded* fit
//! by bound envelopes is not optimal: a bound activating inside a pooled
//! block must re-balance its neighbours — the brute-force tests below pin
//! the difference.)

/// Deterministic, unique, order-preserving ladder. `seps[i]` is the minimum
/// gap between items i and i+1 — a cluster's pitch between different wires,
/// zero between two pieces of one wire (a jog may collapse; its legs owe
/// each other nothing). The caller guarantees feasibility (the search's
/// capacity closure); an infeasible call is a routing bug, caught in debug
/// builds.
pub(crate) fn ladder(prefs: &[f64], bounds: &[(f64, f64)], seps: &[f64]) -> Vec<f64> {
    let n = prefs.len();
    debug_assert_eq!(n, bounds.len());
    if n == 0 {
        return Vec::new();
    }
    debug_assert_eq!(seps.len(), n - 1);
    let cum: Vec<f64> = std::iter::once(0.0)
        .chain(seps.iter().scan(0.0, |acc, s| {
            *acc += s;
            Some(*acc)
        }))
        .collect();
    let shift = |v: f64, i: usize| v - cum[i];

    struct Block {
        sum: f64,
        count: usize,
        lo: f64,
        hi: f64,
    }
    impl Block {
        fn value(&self) -> f64 {
            (self.sum / self.count as f64).max(self.lo).min(self.hi)
        }
    }

    let mut blocks: Vec<Block> = Vec::with_capacity(n);
    for i in 0..n {
        let (lo, hi) = (shift(bounds[i].0, i), shift(bounds[i].1, i));
        debug_assert!(
            lo <= hi + 1e-9,
            "infeasible ladder: item {i} box crosses ({lo} > {hi})"
        );
        blocks.push(Block {
            sum: shift(prefs[i], i),
            count: 1,
            lo,
            hi,
        });
        while blocks.len() >= 2
            && blocks[blocks.len() - 2].value() > blocks[blocks.len() - 1].value()
        {
            let b = blocks.pop().expect("two blocks");
            let a = blocks.last_mut().expect("two blocks");
            a.sum += b.sum;
            a.count += b.count;
            a.lo = a.lo.max(b.lo);
            a.hi = a.hi.min(b.hi);
            debug_assert!(
                a.lo <= a.hi + 1e-9,
                "infeasible ladder: pooled boxes cross ({} > {})",
                a.lo,
                a.hi
            );
        }
    }

    let mut out = Vec::with_capacity(n);
    let mut i = 0;
    for b in &blocks {
        let v = b.value();
        for _ in 0..b.count {
            out.push(v + cum[i]);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Brute-force optimum on a fine grid, for cross-checking small cases.
    fn brute(prefs: &[f64], bounds: &[(f64, f64)], seps: &[f64], step: f64) -> Vec<f64> {
        let n = prefs.len();
        let lo = bounds.iter().map(|b| b.0).fold(f64::MAX, f64::min);
        let hi = bounds.iter().map(|b| b.1).fold(f64::MIN, f64::max);
        let ticks: Vec<f64> = {
            let mut t = Vec::new();
            let mut v = lo;
            while v <= hi + 1e-9 {
                t.push(v);
                v += step;
            }
            t
        };
        let mut best: Option<(f64, Vec<f64>)> = None;
        let mut xs = vec![0.0; n];
        fn rec(
            i: usize,
            xs: &mut Vec<f64>,
            ticks: &[f64],
            prefs: &[f64],
            bounds: &[(f64, f64)],
            seps: &[f64],
            best: &mut Option<(f64, Vec<f64>)>,
        ) {
            if i == prefs.len() {
                let cost: f64 = xs.iter().zip(prefs).map(|(x, p)| (x - p) * (x - p)).sum();
                if best.as_ref().is_none_or(|(c, _)| cost < *c - 1e-12) {
                    *best = Some((cost, xs.clone()));
                }
                return;
            }
            for &t in ticks {
                if t < bounds[i].0 - 1e-9 || t > bounds[i].1 + 1e-9 {
                    continue;
                }
                if i > 0 && t - xs[i - 1] < seps[i - 1] - 1e-9 {
                    continue;
                }
                xs[i] = t;
                rec(i + 1, xs, ticks, prefs, bounds, seps, best);
            }
        }
        rec(0, &mut xs, &ticks, prefs, bounds, seps, &mut best);
        best.expect("feasible").1
    }

    fn close(a: &[f64], b: &[f64]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6)
    }

    #[test]
    fn equal_prefs_spread_centred_on_the_shared_spot() {
        // The bus ladder: four rails wanting one midline take pitch steps
        // centred on it — the median lands on the preference.
        let got = ladder(&[100.0; 4], &[(0.0, 200.0); 4], &[10.0; 3]);
        assert_eq!(got, vec![85.0, 95.0, 105.0, 115.0]);
    }

    #[test]
    fn separated_prefs_stand_exactly_where_they_ask() {
        let got = ladder(
            &[285.0, 295.0, 305.0, 315.0],
            &[(225.0, 385.0); 4],
            &[10.0; 3],
        );
        assert_eq!(got, vec![285.0, 295.0, 305.0, 315.0]);
    }

    #[test]
    fn a_blocked_flock_pools_against_its_neighbour() {
        // The two-bus shape in miniature: two rails wanting one centre,
        // ordered ahead of two spread rails wanting the same region — the
        // whole cluster pools into one pitch-spaced block balancing the
        // pulls, order and pitch intact.
        let prefs = [300.0, 300.0, 285.0, 295.0];
        let bounds = [(260.0, 340.0); 4];
        let got = ladder(&prefs, &bounds, &[10.0; 3]);
        assert_eq!(got, vec![280.0, 290.0, 300.0, 310.0]);
        let expected = brute(&prefs, &bounds, &[10.0; 3], 5.0);
        assert!(close(&got, &expected), "got {got:?} vs brute {expected:?}");
    }

    #[test]
    fn matches_brute_force_on_hard_small_cases() {
        type Case = (&'static [f64], &'static [(f64, f64)], f64);
        let cases: &[Case] = &[
            // Crossing pulls: order forced against the preferences.
            (&[30.0, 10.0, 20.0], &[(0.0, 40.0); 3], 8.0),
            // Tight walls: the ladder squeezes to the boundary.
            (&[20.0, 20.0, 20.0], &[(10.0, 34.0); 3], 12.0),
            // Uneven boxes: one item pinned high, one low.
            (
                &[15.0, 25.0, 18.0],
                &[(0.0, 16.0), (10.0, 40.0), (24.0, 40.0)],
                4.0,
            ),
            // A lone item clamps to its box.
            (&[50.0], &[(10.0, 30.0)], 8.0),
        ];
        for (prefs, bounds, pitch) in cases {
            let seps = vec![*pitch; prefs.len().saturating_sub(1)];
            let got = ladder(prefs, bounds, &seps);
            let expected = brute(prefs, bounds, &seps, 0.5);
            assert!(
                close(&got, &expected),
                "prefs {prefs:?}: got {got:?} vs brute {expected:?}"
            );
        }
    }

    #[test]
    fn bounds_and_pitch_hold_at_the_walls() {
        // Three rails in a corridor exactly three-pitches wide sit at the
        // walls and centre, whatever they preferred.
        let got = ladder(&[0.0, 0.0, 0.0], &[(40.0, 56.0); 3], &[8.0; 2]);
        assert_eq!(got, vec![40.0, 48.0, 56.0]);
        let got = ladder(&[99.0, 99.0, 99.0], &[(40.0, 56.0); 3], &[8.0; 2]);
        assert_eq!(got, vec![40.0, 48.0, 56.0]);
    }

    #[test]
    fn zero_separation_lets_one_wires_pieces_meet() {
        // A Z's two legs (sep 0) share their preferred track — the jog
        // collapses — while a stranger before them still keeps pitch.
        let got = ladder(&[50.0, 50.0], &[(28.0, 72.0); 2], &[0.0]);
        assert_eq!(got, vec![50.0, 50.0]);
        // The stranger and the welded pair balance around 50, pitch held
        // once: one pooled block at mean(50, 42, 42) = 134/3, shifted back.
        let got = ladder(&[50.0, 50.0, 50.0], &[(0.0, 100.0); 3], &[8.0, 0.0]);
        let expected = [134.0 / 3.0, 134.0 / 3.0 + 8.0, 134.0 / 3.0 + 8.0];
        assert!(close(&got, &expected), "got {got:?} vs {expected:?}");
    }

    #[test]
    fn empty_and_single_inputs_are_trivial() {
        assert_eq!(ladder(&[], &[], &[]), Vec::<f64>::new());
        assert_eq!(ladder(&[25.0], &[(0.0, 100.0)], &[]), vec![25.0]);
        assert_eq!(ladder(&[125.0], &[(0.0, 100.0)], &[]), vec![100.0]);
    }
}

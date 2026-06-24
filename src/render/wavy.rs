//! Wavy link rendering (`~>`): the routed centreline, resampled by arc length
//! and displaced by a tapered sine, emitted as a smooth cubic-Bézier path.
//!
//! The wave lives in the **geometry**, not the stroke — so a `~>` link reads as
//! wavy at any `stroke-width` and survives a host-CSS recolour, exactly like the
//! solid/dashed/dotted styles it joins. It reuses the same corner fillets as the
//! plain path ([`super::rounding`]), so a wavy link turns corners the same way
//! its neighbours do; the sine then rides that centreline.

use super::rounding::{Point, RoundedPath, Seg, round};
use super::values::num;
use std::f64::consts::TAU;
use std::fmt::Write;

/// Wave shape, in world units, tuned against the default clearance (16): the
/// wavelength reads as a clear wiggle and the amplitude stays well under a
/// corner's fillet radius, so the wave never touches itself on the inside of a
/// turn. Exposed so the label cut can widen its mask to the wave's reach.
const WAVELENGTH: f64 = 12.0;
pub const AMPLITUDE: f64 = 1.4;

/// Chord the arcs flatten to before sampling — fine enough that a fillet's
/// finite-difference tangent stays smooth.
const FLATTEN_STEP: f64 = 1.5;

/// A wavy `d` for the link centreline `pts` (already shortened for markers),
/// rounded with the same per-corner `targets` as the plain path. `None` when the
/// route is shorter than one wavelength — the caller then draws it straight,
/// since a fraction of a wave would just look like a kink.
pub fn wavy_d(pts: &[Point], targets: &[f64]) -> Option<String> {
    let line = Centerline::flatten(&round(pts, targets));
    let total = line.total();
    if total < WAVELENGTH {
        return None;
    }

    // The amplitude ramps 0 → 1 → 0 over half a wavelength at each end, so the
    // link leaves each port flat — meeting its marker and the node edge head-on
    // — yet a short run still shows real wave between the ramps.
    let taper = (WAVELENGTH / 2.0).min(total / 2.0);
    let k = TAU / WAVELENGTH;
    let steps = ((total / (WAVELENGTH / 4.0)).round() as usize).max(2);

    // One sampled point on the wave, with its unit tangent: the centreline
    // point pushed `offset` along the normal, the tangent tilted by how fast
    // that offset is changing — so consecutive Bézier handles meet smoothly.
    let sample = |s: f64| -> (Point, Point) {
        let (p, t) = line.at(s);
        let (env, denv) = envelope(s, total, taper);
        let phase = k * s;
        let offset = env * AMPLITUDE * phase.sin();
        let d_offset = denv * AMPLITUDE * phase.sin() + env * AMPLITUDE * k * phase.cos();
        let normal = (-t.1, t.0);
        let w = (p.0 + normal.0 * offset, p.1 + normal.1 * offset);
        let wt = unit((t.0 + normal.0 * d_offset, t.1 + normal.1 * d_offset));
        (w, wt)
    };

    let (mut from, mut from_t) = sample(0.0);
    let mut d = format!("M {} {}", num(from.0), num(from.1));
    for i in 1..=steps {
        let s = (total * i as f64 / steps as f64).min(total);
        let (to, to_t) = sample(s);
        // Hermite → cubic: handles a third of the chord along each end tangent.
        let h = dist(from, to) / 3.0;
        write!(
            d,
            " C {} {} {} {} {} {}",
            num(from.0 + from_t.0 * h),
            num(from.1 + from_t.1 * h),
            num(to.0 - to_t.0 * h),
            num(to.1 - to_t.1 * h),
            num(to.0),
            num(to.1),
        )
        .unwrap();
        (from, from_t) = (to, to_t);
    }
    Some(d)
}

/// A smoothstep window over arc length: amplitude 0 at both ends, 1 in the
/// middle. Returns the envelope and its derivative w.r.t. `s`, so the wave's end
/// tangents fold cleanly back onto the centreline (both the value and its slope
/// reach 0 at each port).
fn envelope(s: f64, total: f64, taper: f64) -> (f64, f64) {
    let (u, du) = if s <= total - s {
        (s / taper, 1.0 / taper)
    } else {
        ((total - s) / taper, -1.0 / taper)
    };
    if u >= 1.0 {
        return (1.0, 0.0);
    }
    (3.0 * u * u - 2.0 * u * u * u, (6.0 * u - 6.0 * u * u) * du)
}

/// The rounded centreline flattened to a dense polyline with cumulative arc
/// length, so a point and tangent can be read off at any distance `s`.
struct Centerline {
    pts: Vec<Point>,
    cum: Vec<f64>,
}

impl Centerline {
    fn flatten(rp: &RoundedPath) -> Self {
        let mut pts = vec![rp.start];
        for seg in &rp.segs {
            match seg {
                Seg::Line { to } => pts.push(*to),
                Seg::Arc {
                    to, center, radius, ..
                } => flatten_arc(*pts.last().unwrap(), *to, *center, *radius, &mut pts),
            }
        }
        pts.dedup_by(|a, b| dist(*a, *b) < 1e-9);
        let mut cum = vec![0.0];
        for w in pts.windows(2) {
            cum.push(cum.last().unwrap() + dist(w[0], w[1]));
        }
        Centerline { pts, cum }
    }

    fn total(&self) -> f64 {
        *self.cum.last().unwrap_or(&0.0)
    }

    fn at(&self, s: f64) -> (Point, Point) {
        let s = s.clamp(0.0, self.total());
        let j = self
            .cum
            .partition_point(|&c| c <= s)
            .clamp(1, self.pts.len() - 1)
            - 1;
        let (a, b) = (self.pts[j], self.pts[j + 1]);
        let span = self.cum[j + 1] - self.cum[j];
        let t = if span > 1e-9 {
            (s - self.cum[j]) / span
        } else {
            0.0
        };
        let p = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        (p, unit((b.0 - a.0, b.1 - a.1)))
    }
}

/// Append the interior samples of an arc and its exact end point, rotating the
/// start radius about the centre by the signed swept angle (≤ 90° for an
/// orthogonal turn, so the short way is always right).
fn flatten_arc(from: Point, to: Point, center: Point, radius: f64, out: &mut Vec<Point>) {
    let v0 = (from.0 - center.0, from.1 - center.1);
    let v1 = (to.0 - center.0, to.1 - center.1);
    let angle = (v0.0 * v1.1 - v0.1 * v1.0).atan2(v0.0 * v1.0 + v0.1 * v1.1);
    let steps = ((radius * angle.abs() / FLATTEN_STEP).ceil() as usize).max(1);
    for i in 1..steps {
        let a = angle * i as f64 / steps as f64;
        let (cos, sin) = (a.cos(), a.sin());
        out.push((
            center.0 + v0.0 * cos - v0.1 * sin,
            center.1 + v0.0 * sin + v0.1 * cos,
        ));
    }
    out.push(to);
}

fn unit(v: Point) -> Point {
    let len = (v.0 * v.0 + v.1 * v.1).sqrt();
    if len < 1e-12 {
        (1.0, 0.0)
    } else {
        (v.0 / len, v.1 / len)
    }
}

fn dist(a: Point, b: Point) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse the `x y` pairs out of a `d` (the M anchor plus every C end point).
    fn anchors(d: &str) -> Vec<Point> {
        let nums: Vec<f64> = d
            .split([' ', ','])
            .filter_map(|t| t.parse::<f64>().ok())
            .collect();
        // M is 1 pair; each C is 3 pairs, the last being the on-curve point.
        let mut pts = vec![(nums[0], nums[1])];
        let mut i = 2;
        while i + 6 <= nums.len() {
            pts.push((nums[i + 4], nums[i + 5]));
            i += 6;
        }
        pts
    }

    #[test]
    fn short_route_declines_to_wave() {
        assert!(wavy_d(&[(0.0, 0.0), (8.0, 0.0)], &[]).is_none());
    }

    #[test]
    fn wave_stays_on_the_line_at_both_ends() {
        // A straight run: the taper pins the first and last on-curve points to
        // the centreline (y = 0) so the link meets its ports flat.
        let d = wavy_d(&[(0.0, 0.0), (120.0, 0.0)], &[]).expect("waves");
        let pts = anchors(&d);
        assert!(pts.len() > 4, "expected several Bézier hops: {d}");
        assert!(pts.first().unwrap().1.abs() < 1e-9);
        assert!(pts.last().unwrap().1.abs() < 1e-9);
        assert!((pts.first().unwrap().0 - 0.0).abs() < 1e-9);
        assert!((pts.last().unwrap().0 - 120.0).abs() < 1e-9);
    }

    #[test]
    fn amplitude_is_bounded_and_actually_oscillates() {
        let d = wavy_d(&[(0.0, 0.0), (160.0, 0.0)], &[]).expect("waves");
        let pts = anchors(&d);
        let peak = pts.iter().map(|p| p.1.abs()).fold(0.0_f64, f64::max);
        // The mid-run reaches near full amplitude but never past it.
        assert!(peak > AMPLITUDE * 0.8, "wave too shallow: {peak}");
        assert!(peak <= AMPLITUDE + 1e-9, "wave overshoots: {peak}");
    }

    #[test]
    fn turns_a_corner_without_blowing_up() {
        // An L-route with a filleted corner stays finite and bounded near the
        // corner (no NaN from the arc walk, no runaway handle).
        let d = wavy_d(&[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)], &[16.0]).expect("waves");
        assert!(!d.contains("NaN") && !d.contains("inf"), "{d}");
        let pts = anchors(&d);
        assert!(pts.len() > 8, "expected a wave around the corner: {d}");
    }
}

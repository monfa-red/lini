//! Axis scales [SPEC 14.4]: a categorical **band** (evenly-spaced slots) or a
//! numeric **linear** domain. One type for x and value axes, so the projection and
//! every tick renderer speak one scale. (`log` follows in a later step.)

/// A position scale along one axis.
pub enum Scale {
    /// `n` evenly-spaced category slots; a datum's coordinate is its 0-based index.
    Band { n: usize },
    /// A numeric domain `[min, max]`; `rev` runs it high→low (a reversed axis).
    Linear {
        min: f64,
        max: f64,
        rev: bool,
        ticks: Vec<f64>,
    },
    /// A logarithmic domain `[min, max]` (both > 0); decade ticks labelled 1-2-5.
    Log { min: f64, max: f64, ticks: Vec<f64> },
}

impl Scale {
    pub fn band(n: usize) -> Scale {
        Scale::Band { n: n.max(1) }
    }

    pub fn linear(min: f64, max: f64, rev: bool, ticks: Vec<f64>) -> Scale {
        Scale::Linear {
            min,
            max,
            rev,
            ticks,
        }
    }

    /// A log scale over `[min, max]` (both > 0), with decade ticks at 1-2-5 × 10ⁿ.
    pub fn log(min: f64, max: f64) -> Scale {
        Scale::Log {
            min,
            max,
            ticks: decade_ticks(min, max),
        }
    }

    /// Position fraction 0..1 of `v` from the axis start. Band: the slot centre
    /// `(i + 0.5)/n`. Linear: `(v − min)/(max − min)`, flipped when reversed.
    pub fn frac(&self, v: f64) -> f64 {
        match self {
            Scale::Band { n } => (v + 0.5) / *n as f64,
            Scale::Linear { min, max, rev, .. } => {
                let span = max - min;
                let f = if span.abs() < f64::EPSILON {
                    0.0
                } else {
                    (v - min) / span
                };
                if *rev { 1.0 - f } else { f }
            }
            Scale::Log { min, max, .. } => {
                let span = max.log10() - min.log10();
                if span.abs() < f64::EPSILON || v <= 0.0 {
                    0.0
                } else {
                    (v.log10() - min.log10()) / span
                }
            }
        }
    }

    /// The fraction edges of category slot `i` (for a bar's width). Linear scales
    /// have no slots.
    pub fn slot(&self, i: usize) -> (f64, f64) {
        match self {
            Scale::Band { n } => (i as f64 / *n as f64, (i as f64 + 1.0) / *n as f64),
            _ => (0.0, 1.0),
        }
    }

    /// This scale's tick values (empty for a band — its labels are categories).
    pub fn ticks(&self) -> &[f64] {
        match self {
            Scale::Linear { ticks, .. } | Scale::Log { ticks, .. } => ticks,
            Scale::Band { .. } => &[],
        }
    }

    /// Clamp a value into the numeric domain (crop to the plot, [SPEC 14.4]); a
    /// band passes its index through.
    pub fn clamp(&self, v: f64) -> f64 {
        match self {
            Scale::Linear { min, max, .. } | Scale::Log { min, max, .. } => {
                v.clamp(min.min(*max), min.max(*max))
            }
            Scale::Band { .. } => v,
        }
    }

    /// Whether `v` lies within the drawn domain (for cropping numeric data; a band
    /// always contains its own indices).
    pub fn contains(&self, v: f64) -> bool {
        match self {
            Scale::Band { n } => v >= 0.0 && v < *n as f64,
            Scale::Linear { min, max, .. } | Scale::Log { min, max, .. } => {
                let (lo, hi) = (min.min(*max), min.max(*max));
                v >= lo - 1e-9 && v <= hi + 1e-9
            }
        }
    }
}

/// A "nice" tick step (1-2-5 × 10ⁿ) near `range / 5`.
pub fn nice_step(range: f64) -> f64 {
    if range <= 0.0 || !range.is_finite() {
        return 1.0;
    }
    let raw = range / 5.0;
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag;
    let unit = if norm <= 1.0 {
        1.0
    } else if norm <= 2.0 {
        2.0
    } else if norm <= 5.0 {
        5.0
    } else {
        10.0
    };
    unit * mag
}

/// A nice ceiling ≥ `vmax` for a zero-based value axis (bars include 0).
pub fn nice_max(vmax: f64) -> f64 {
    if vmax.is_nan() || vmax <= 0.0 {
        return 1.0;
    }
    let step = nice_step(vmax);
    (vmax / step).ceil() * step
}

/// Nice tick values across `[min, max]` (1-2-5 spacing).
pub fn nice_ticks(min: f64, max: f64) -> Vec<f64> {
    ticks_by_step(min, max, nice_step(max - min))
}

/// Tick values across `[min, max]` at a given `step`, snapped to multiples of it.
pub fn ticks_by_step(min: f64, max: f64, step: f64) -> Vec<f64> {
    let (lo, hi) = (min.min(max), min.max(max));
    if step <= 0.0 || !step.is_finite() || hi <= lo {
        return vec![lo];
    }
    let mut out = Vec::new();
    let mut t = (lo / step).ceil() * step;
    while t <= hi + step * 1e-6 {
        // Snap away float drift so a tick lands on a clean value.
        out.push((t / step).round() * step);
        t += step;
    }
    if out.is_empty() {
        out.push(lo);
    }
    out
}

/// Decade ticks for a log axis [SPEC 14.4]: 1-2-5 × 10ⁿ within `[min, max]`.
fn decade_ticks(min: f64, max: f64) -> Vec<f64> {
    if min <= 0.0 || max <= min {
        return vec![min.max(1e-9), max.max(1.0)];
    }
    let mut out = Vec::new();
    let lo = min.log10().floor() as i32;
    let hi = max.log10().ceil() as i32;
    for e in lo..=hi {
        let decade = 10f64.powi(e);
        for m in [1.0, 2.0, 5.0] {
            let t = m * decade;
            if t >= min - 1e-9 && t <= max + 1e-9 {
                out.push(t);
            }
        }
    }
    if out.is_empty() {
        out.push(min);
        out.push(max);
    }
    out
}

/// A tick label: the formatted value with an optional unit suffix [SPEC 14.4].
pub fn label(value: f64, unit: &Option<String>) -> String {
    let mut s = fmt_tick(value);
    if let Some(u) = unit {
        s.push_str(u);
    }
    s
}

/// Format a tick / data value as a clean label: integers stay integers, decimals
/// trim trailing zeros. (A value's *display* string is chart content, distinct from
/// the SVG coordinate formatting render owns.)
pub fn fmt_tick(n: f64) -> String {
    if n.is_finite() && n == n.trunc() && n.abs() < 1e15 {
        return (n as i64).to_string();
    }
    let s = format!("{:.4}", n);
    let t = s.trim_end_matches('0').trim_end_matches('.');
    if t.is_empty() || t == "-" {
        "0".to_string()
    } else {
        t.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_max_rounds_up_to_a_1_2_5_ceiling() {
        assert_eq!(nice_max(24.0), 25.0);
        assert_eq!(nice_max(9.0), 10.0);
        assert_eq!(nice_max(100.0), 100.0);
        assert_eq!(nice_max(0.0), 1.0);
    }

    #[test]
    fn nice_ticks_run_across_the_domain() {
        let t = nice_ticks(0.0, 25.0);
        assert_eq!(t.first(), Some(&0.0));
        assert_eq!(t.last(), Some(&25.0));
    }

    #[test]
    fn band_frac_is_the_slot_centre() {
        let s = Scale::band(4);
        assert!((s.frac(0.0) - 0.125).abs() < 1e-9);
        assert!((s.frac(3.0) - 0.875).abs() < 1e-9);
    }

    #[test]
    fn linear_reverse_flips_the_fraction() {
        let up = Scale::linear(0.0, 10.0, false, vec![]);
        let dn = Scale::linear(0.0, 10.0, true, vec![]);
        assert!((up.frac(2.5) - 0.25).abs() < 1e-9);
        assert!((dn.frac(2.5) - 0.75).abs() < 1e-9);
    }

    #[test]
    fn fmt_tick_trims_trailing_zeros() {
        assert_eq!(fmt_tick(5.0), "5");
        assert_eq!(fmt_tick(2.5), "2.5");
        assert_eq!(fmt_tick(0.0), "0");
    }
}

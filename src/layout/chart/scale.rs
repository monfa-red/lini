//! Axis scales [SPEC 14.4]: a categorical **band** (evenly-spaced slots) or a
//! numeric **linear** domain. One type for x and value axes, so the projection and
//! every tick renderer speak one scale. (`log` follows in a later step.)

use crate::ledger::date;
use crate::ledger::format::{self, DateUnit, Format};

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
    /// A time domain in **epoch seconds** [SPEC 14.4]: linear projection,
    /// calendar-aware ticks reading at `unit`'s granularity.
    Time {
        min: f64,
        max: f64,
        rev: bool,
        ticks: Vec<f64>,
        unit: DateUnit,
    },
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

    /// A time scale over `[min, max]` epoch seconds with calendar ticks
    /// [SPEC 14.4]; `step` is an authored calendar interval, `None` auto.
    pub fn time(min: f64, max: f64, rev: bool, step: Option<(CalUnit, u32)>) -> Scale {
        let (ticks, unit) = time_ticks(min, max, step);
        Scale::Time {
            min,
            max,
            rev,
            ticks,
            unit,
        }
    }

    /// Position fraction 0..1 of `v` from the axis start. Band: the slot centre
    /// `(i + 0.5)/n`. Linear: `(v − min)/(max − min)`, flipped when reversed.
    pub fn frac(&self, v: f64) -> f64 {
        match self {
            Scale::Band { n } => (v + 0.5) / *n as f64,
            Scale::Linear { min, max, rev, .. } | Scale::Time { min, max, rev, .. } => {
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
            Scale::Linear { ticks, .. } | Scale::Log { ticks, .. } | Scale::Time { ticks, .. } => {
                ticks
            }
            Scale::Band { .. } => &[],
        }
    }

    /// Clamp a value into the numeric domain (crop to the plot, [SPEC 14.4]); a
    /// band passes its index through.
    pub fn clamp(&self, v: f64) -> f64 {
        match self {
            Scale::Linear { min, max, .. }
            | Scale::Log { min, max, .. }
            | Scale::Time { min, max, .. } => v.clamp(min.min(*max), min.max(*max)),
            Scale::Band { .. } => v,
        }
    }

    /// Whether `v` lies within the drawn domain (for cropping numeric data; a band
    /// always contains its own indices).
    pub fn contains(&self, v: f64) -> bool {
        match self {
            Scale::Band { n } => v >= 0.0 && v < *n as f64,
            Scale::Linear { min, max, .. }
            | Scale::Log { min, max, .. }
            | Scale::Time { min, max, .. } => {
                let (lo, hi) = (min.min(*max), min.max(*max));
                v >= lo - 1e-9 && v <= hi + 1e-9
            }
        }
    }
}

/// A "nice" tick step (1-2-5 × 10ⁿ) near `range / TICK_TARGET`.
pub fn nice_step(range: f64) -> f64 {
    if range <= 0.0 || !range.is_finite() {
        return 1.0;
    }
    let raw = range / super::metrics::TICK_TARGET;
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

/// A tick label: the value under its axis's `format:` [SPEC 14.4/16], with an
/// optional unit suffix appended after (the compose order [SPEC 16]). On a
/// time scale, `auto` reads at the tick unit's granularity; an explicit date
/// preset wins; an explicit numeric family renders the epoch number, honestly.
pub fn label(scale: &Scale, value: f64, fmt: Format, unit: &Option<String>) -> String {
    let mut s = match (scale, fmt) {
        (Scale::Time { unit: u, .. }, Format::Auto) => date::render(value, *u),
        (Scale::Time { .. }, Format::Date(p)) => date::render(value, p),
        _ => format::render(value, fmt),
    };
    if let Some(u) = unit {
        s.push_str(u);
    }
    s
}

/// A value's `auto` display string [SPEC 16] — the format engine's default
/// reading. (Chart content, distinct from the SVG coordinate formatting render
/// owns.)
pub fn fmt_tick(n: f64) -> String {
    format::auto(n)
}

/// A calendar tick interval's unit [SPEC 14.4] — the `step:` idents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalUnit {
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

impl CalUnit {
    /// The average length in seconds — step *selection* only; generation
    /// walks the real calendar.
    fn approx(self) -> f64 {
        match self {
            CalUnit::Minute => 60.0,
            CalUnit::Hour => 3_600.0,
            CalUnit::Day => 86_400.0,
            CalUnit::Week => 604_800.0,
            CalUnit::Month => 2_629_746.0,
            CalUnit::Year => 31_556_952.0,
        }
    }

    /// The granularity ticks of this unit read at (weeks read as days).
    fn label(self) -> DateUnit {
        match self {
            CalUnit::Minute => DateUnit::Minute,
            CalUnit::Hour => DateUnit::Hour,
            CalUnit::Day | CalUnit::Week => DateUnit::Day,
            CalUnit::Month => DateUnit::Month,
            CalUnit::Year => DateUnit::Year,
        }
    }
}

/// The auto ladder [SPEC 14.4]: the first interval giving ≤ `TICK_TARGET`
/// ticks wins; spans beyond it climb years by 1-2-5.
const CAL_LADDER: &[(CalUnit, u32)] = &[
    (CalUnit::Minute, 1),
    (CalUnit::Minute, 2),
    (CalUnit::Minute, 5),
    (CalUnit::Minute, 10),
    (CalUnit::Minute, 15),
    (CalUnit::Minute, 30),
    (CalUnit::Hour, 1),
    (CalUnit::Hour, 2),
    (CalUnit::Hour, 3),
    (CalUnit::Hour, 6),
    (CalUnit::Hour, 12),
    (CalUnit::Day, 1),
    (CalUnit::Day, 2),
    (CalUnit::Week, 1),
    (CalUnit::Week, 2),
    (CalUnit::Month, 1),
    (CalUnit::Month, 2),
    (CalUnit::Month, 3),
    (CalUnit::Month, 6),
    (CalUnit::Year, 1),
    (CalUnit::Year, 2),
    (CalUnit::Year, 5),
];

/// Calendar-aware time ticks [SPEC 14.4]: every tick lands on a calendar
/// boundary (weeks on Mondays); auto picks the ladder interval, an authored
/// calendar `step:` overrides. Returns the ticks and their reading unit.
pub fn time_ticks(min: f64, max: f64, step: Option<(CalUnit, u32)>) -> (Vec<f64>, DateUnit) {
    let span = (max - min).max(0.0);
    let (unit, count) = step.unwrap_or_else(|| {
        let target = super::metrics::TICK_TARGET;
        for &(u, c) in CAL_LADDER {
            if span / (u.approx() * c as f64) <= target {
                return (u, c);
            }
        }
        // Beyond 5-year steps: climb years by 1-2-5.
        let years = nice_step(span / CalUnit::Year.approx()).max(1.0);
        (CalUnit::Year, years as u32)
    });
    let mut ticks = Vec::new();
    match unit {
        CalUnit::Minute | CalUnit::Hour | CalUnit::Day | CalUnit::Week => {
            let step_s = unit.approx() * count as f64;
            // Fixed-length units align to their epoch grid; weeks to Mondays
            // (epoch day 0 is a Thursday — Monday sits at day ≡ 4 mod 7).
            let offset = if unit == CalUnit::Week {
                4.0 * 86_400.0
            } else {
                0.0
            };
            let mut t = ((min - offset) / step_s).ceil() * step_s + offset;
            while t <= max + 1e-6 && ticks.len() < 4_000 {
                ticks.push(t);
                t += step_s;
            }
        }
        CalUnit::Month => {
            let days = (min / 86_400.0).floor() as i64;
            let (mut y, mut m, _) = date::civil_from_days(days);
            // The first month boundary ≥ min.
            if (date::days_from_civil(y, m, 1) as f64) * 86_400.0 < min {
                m += 1;
            }
            loop {
                y += (m as i64 - 1).div_euclid(12);
                m = ((m as i64 - 1).rem_euclid(12) + 1) as u32;
                let t = date::days_from_civil(y, m, 1) as f64 * 86_400.0;
                if t > max + 1e-6 || ticks.len() >= 4_000 {
                    break;
                }
                if t >= min - 1e-6 {
                    ticks.push(t);
                }
                m += count;
            }
        }
        CalUnit::Year => {
            let days = (min / 86_400.0).floor() as i64;
            let (mut y, _, _) = date::civil_from_days(days);
            if (date::days_from_civil(y, 1, 1) as f64) * 86_400.0 < min {
                y += 1;
            }
            // Multi-year steps land on multiples of the step (2010, 2015, …).
            if count > 1 {
                y = y.div_euclid(count as i64) * count as i64;
                if (date::days_from_civil(y, 1, 1) as f64) * 86_400.0 < min {
                    y += count as i64;
                }
            }
            loop {
                let t = date::days_from_civil(y, 1, 1) as f64 * 86_400.0;
                if t > max + 1e-6 || ticks.len() >= 4_000 {
                    break;
                }
                ticks.push(t);
                y += count as i64;
            }
        }
    }
    if ticks.is_empty() {
        ticks.push(min);
        ticks.push(max);
    }
    (ticks, unit.label())
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

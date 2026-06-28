//! A value scale: data domain → nice ticks ([CHARTS.md] §6). Step 1 is the linear,
//! zero-based case bars need; `log` and explicit `step:`/`ticks:` follow in a later
//! step, extending this one type rather than adding a parallel one.

/// A linear value scale from 0 to a nice ceiling, with 1-2-5 tick stops.
pub struct Scale {
    pub max: f64,
    pub ticks: Vec<f64>,
}

impl Scale {
    /// A nice zero-based scale covering `[0, vmax]`: the tick step is a 1-2-5 × 10ⁿ
    /// value near `vmax / 5`, and the ceiling is rounded up to a whole step.
    pub fn nice(vmax: f64) -> Scale {
        if vmax.is_nan() || vmax <= 0.0 {
            return Scale {
                max: 1.0,
                ticks: vec![0.0, 1.0],
            };
        }
        let raw = vmax / 5.0;
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
        let step = unit * mag;
        let max = (vmax / step).ceil() * step;
        let count = (max / step).round() as i64;
        let ticks = (0..=count).map(|k| k as f64 * step).collect();
        Scale { max, ticks }
    }
}

/// Format a tick / data value as a clean label: integers stay integers, decimals
/// trim trailing zeros. (A value's *display* string is chart content, distinct from
/// the SVG coordinate formatting render owns.)
pub fn fmt_tick(n: f64) -> String {
    if n.is_finite() && n == n.trunc() && n.abs() < 1e15 {
        return (n as i64).to_string();
    }
    let s = format!("{:.2}", n);
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
    fn nice_rounds_up_to_a_1_2_5_ceiling() {
        assert_eq!(Scale::nice(24.0).max, 25.0);
        assert_eq!(Scale::nice(9.0).max, 10.0);
        assert_eq!(Scale::nice(100.0).max, 100.0);
        // A non-positive domain falls back to a 0..1 axis.
        assert_eq!(Scale::nice(0.0).max, 1.0);
    }

    #[test]
    fn ticks_run_from_zero_to_the_ceiling() {
        let s = Scale::nice(24.0);
        assert_eq!(s.ticks.first(), Some(&0.0));
        assert_eq!(s.ticks.last(), Some(&25.0));
    }

    #[test]
    fn fmt_tick_trims_trailing_zeros() {
        assert_eq!(fmt_tick(5.0), "5");
        assert_eq!(fmt_tick(2.5), "2.5");
        assert_eq!(fmt_tick(0.0), "0");
    }
}

//! The `format:` engine [SPEC 16] — value **presentation**, never measurement.
//! One parse and one renderer shared by every consumer (chart ticks and
//! tooltip values today, dimension text in the drawing half), so the same
//! value can never read two ways. Composes before `unit:` suffixes, `tol:`,
//! the `⌀`/`R`/`°` glyphs, and `N×` counts.

use crate::error::Error;
use crate::resolve::{AttrMap, ResolvedValue};
use crate::span::Span;

/// A parsed `format:` [SPEC 16], default `auto`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Format {
    /// Integers stay integers; decimals trim trailing zeros (≤ 4 places).
    #[default]
    Auto,
    /// Fixed decimals, trailing zeros kept.
    Decimal(u8),
    /// Significant digits (trailing zeros within them kept: `1.50`).
    Significant(u8),
    /// `m e p` exponential, the mantissa in `[1, 10)`.
    Scientific(u8),
    /// Exponential with the exponent snapped to a multiple of 3.
    Engineering(u8),
    /// Value × 100 with fixed decimals, a `%` appended.
    Percent(u8),
    /// Nearest multiple of `1/D`, reduced — the drafting fraction.
    Fraction(u32),
    /// A date preset [SPEC 14.4] — speaks only to time-axis ticks; inherited
    /// onto a numeric consumer it says nothing ([`numeric`]), authored on one
    /// directly it errors at that owner's build.
    Date(DateUnit),
}

/// A date preset's granularity [SPEC 14.4/16].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateUnit {
    Year,
    Month,
    Day,
    Hour,
    Minute,
}

/// The SPEC 20 usage row, verbatim.
const USAGE: &str = "'format' takes auto, decimal N, significant N, scientific N, \
                     engineering N, percent N, fraction D, or a date preset";

/// Read a node's `format:`, falling back to `default` (the chart's, for an
/// axis / series that sets none) — the cascade in one read, like `tooltip:`.
pub fn read_or(attrs: &AttrMap, default: Format, span: Span) -> Result<Format, Error> {
    match attrs.get("format") {
        None => Ok(default),
        Some(v) => parse(v, span),
    }
}

/// The numeric reading of a **cascaded** format: a date preset speaks only to
/// time ticks — flowing onto a numeric consumer it says nothing (`Auto`).
pub fn numeric(f: Format) -> Format {
    if matches!(f, Format::Date(_)) {
        Format::Auto
    } else {
        f
    }
}

/// The date-preset gate [SPEC 16]: a date preset speaks only to a time axis, so
/// authored on a numeric consumer it errors. Shared by the chart's numeric axes,
/// `pie`, and the drawing's dimension text.
pub fn reject_date(f: Format, span: Span) -> Result<(), Error> {
    if matches!(f, Format::Date(_)) {
        return Err(Error::at(span, "a date preset reads a time axis"));
    }
    Ok(())
}

/// The SPEC 20 message when a one-shape series (`|line|` / `|area|`) carries a
/// per-datum paint list. Shared by the validator (lint) and the model (the
/// semantic authority), so a library compile can't slip past a matching text.
pub fn one_shape_paint(shape: &str) -> String {
    format!(
        "a '|{shape}|' is one shape with one paint — per-datum lists \
         read on '|bars|' / '|dots|'"
    )
}

fn parse(v: &ResolvedValue, span: Span) -> Result<Format, Error> {
    let err = |msg: &str| Err(Error::at(span, msg));
    match v {
        ResolvedValue::Ident(s) => match s.as_str() {
            "auto" => Ok(Format::Auto),
            "year" => Ok(Format::Date(DateUnit::Year)),
            "month" => Ok(Format::Date(DateUnit::Month)),
            "day" => Ok(Format::Date(DateUnit::Day)),
            "hour" => Ok(Format::Date(DateUnit::Hour)),
            "minute" => Ok(Format::Date(DateUnit::Minute)),
            _ => err(USAGE),
        },
        ResolvedValue::Tuple(items) => match items.as_slice() {
            [ResolvedValue::Ident(fam), ResolvedValue::Number(n)] => {
                let arg = *n;
                let digits = |lo: f64, what: &str| -> Result<u8, Error> {
                    if arg.fract() == 0.0 && (lo..=12.0).contains(&arg) {
                        Ok(arg as u8)
                    } else {
                        Err(Error::at(
                            span,
                            format!("'format: {fam} N' takes {}..12 {what}", lo as u8),
                        ))
                    }
                };
                match fam.as_str() {
                    "decimal" => Ok(Format::Decimal(digits(0.0, "decimals")?)),
                    "significant" => Ok(Format::Significant(digits(1.0, "digits")?)),
                    "scientific" => Ok(Format::Scientific(digits(0.0, "decimals")?)),
                    "engineering" => Ok(Format::Engineering(digits(0.0, "decimals")?)),
                    "percent" => Ok(Format::Percent(digits(0.0, "decimals")?)),
                    "fraction" => {
                        if arg.fract() == 0.0 && (2.0..=1024.0).contains(&arg) {
                            Ok(Format::Fraction(arg as u32))
                        } else {
                            err("'format: fraction D' takes a whole denominator 2..1024")
                        }
                    }
                    _ => err(USAGE),
                }
            }
            _ => err(USAGE),
        },
        _ => err(USAGE),
    }
}

/// Render a numeric value under a format. A `Date` preset never reaches here —
/// consumers gate it ([`numeric`] / the owner-build errors) — but reads `Auto`
/// rather than lying.
pub fn render(n: f64, f: Format) -> String {
    match f {
        Format::Auto | Format::Date(_) => auto(n),
        Format::Decimal(d) => no_neg_zero(format!("{:.*}", d as usize, n)),
        Format::Significant(s) => significant(n, s),
        Format::Scientific(d) => exponential(n, d, false),
        Format::Engineering(d) => exponential(n, d, true),
        Format::Percent(d) => format!("{}%", no_neg_zero(format!("{:.*}", d as usize, n * 100.0))),
        Format::Fraction(den) => fraction_flat(n, den),
    }
}

/// The `auto` reading (the historic tick formatter): integers stay integers,
/// decimals print to 4 places and trim trailing zeros.
pub fn auto(n: f64) -> String {
    if n.is_finite() && n == n.trunc() && n.abs() < 1e15 {
        return (n as i64).to_string();
    }
    let s = format!("{n:.4}");
    let t = s.trim_end_matches('0').trim_end_matches('.');
    if t.is_empty() || t == "-" {
        "0".to_string()
    } else {
        t.to_string()
    }
}

fn significant(n: f64, s: u8) -> String {
    if n == 0.0 || !n.is_finite() {
        return auto(n);
    }
    let exp = n.abs().log10().floor() as i32;
    let dec = s as i32 - 1 - exp;
    if dec < 0 {
        // Rounding above the decimal point: snap to the significant place.
        let scale = 10f64.powi(-dec);
        auto((n / scale).round() * scale)
    } else {
        no_neg_zero(format!("{:.*}", dec as usize, n))
    }
}

fn exponential(n: f64, d: u8, eng: bool) -> String {
    if n == 0.0 || !n.is_finite() {
        return format!("{}e0", no_neg_zero(format!("{:.*}", d as usize, 0.0)));
    }
    let round_to = 10f64.powi(d as i32);
    let mut exp = n.abs().log10().floor() as i32;
    let mut mant = n / 10f64.powi(exp);
    // Mantissa rounding may carry past the band's top (9.99 → 10.0 at d = 1).
    if (mant.abs() * round_to).round() / round_to >= 10.0 {
        exp += 1;
        mant /= 10.0;
    }
    if eng {
        let down = exp.rem_euclid(3);
        exp -= down;
        mant *= 10f64.powi(down);
        if (mant.abs() * round_to).round() / round_to >= 1000.0 {
            exp += 3;
            mant /= 1000.0;
        }
    }
    format!("{}e{exp}", no_neg_zero(format!("{:.*}", d as usize, mant)))
}

/// A fraction's parts [SPEC 16]: the nearest multiple of `1/den`, reduced —
/// `(negative, whole, numerator, denominator)`; a zero numerator is a whole
/// reading. The dimension typography arranges these; [`render`] flattens them.
pub fn fraction_parts(n: f64, den: u32) -> (bool, u64, u32, u32) {
    let total = (n.abs() * den as f64).round() as u64;
    let whole = total / den as u64;
    let mut num = (total % den as u64) as u32;
    let mut d = den;
    if num != 0 {
        let g = gcd(num, d);
        num /= g;
        d /= g;
    }
    (n < 0.0 && total > 0, whole, num, d)
}

fn fraction_flat(n: f64, den: u32) -> String {
    let (neg, whole, num, d) = fraction_parts(n, den);
    let sign = if neg { "-" } else { "" };
    match (whole, num) {
        (_, 0) => format!("{sign}{whole}"),
        (0, _) => format!("{sign}{num}/{d}"),
        _ => format!("{sign}{whole} {num}/{d}"),
    }
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// `-0.00` reads `0.00`: a sign on an all-zero run is float noise, not a value.
fn no_neg_zero(s: String) -> String {
    if s.starts_with('-') && s[1..].chars().all(|c| matches!(c, '0' | '.')) {
        s[1..].to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(v: ResolvedValue) -> Result<Format, Error> {
        parse(&v, Span::empty())
    }

    fn pair(fam: &str, n: f64) -> ResolvedValue {
        ResolvedValue::Tuple(vec![
            ResolvedValue::Ident(fam.into()),
            ResolvedValue::Number(n),
        ])
    }

    #[test]
    fn parses_every_family() {
        assert_eq!(
            fmt(ResolvedValue::Ident("auto".into())).unwrap(),
            Format::Auto
        );
        assert_eq!(fmt(pair("decimal", 2.0)).unwrap(), Format::Decimal(2));
        assert_eq!(
            fmt(pair("significant", 3.0)).unwrap(),
            Format::Significant(3)
        );
        assert_eq!(fmt(pair("scientific", 1.0)).unwrap(), Format::Scientific(1));
        assert_eq!(
            fmt(pair("engineering", 0.0)).unwrap(),
            Format::Engineering(0)
        );
        assert_eq!(fmt(pair("percent", 1.0)).unwrap(), Format::Percent(1));
        assert_eq!(fmt(pair("fraction", 8.0)).unwrap(), Format::Fraction(8));
        assert_eq!(
            fmt(ResolvedValue::Ident("month".into())).unwrap(),
            Format::Date(DateUnit::Month)
        );
    }

    #[test]
    fn rejects_bad_values() {
        assert!(fmt(ResolvedValue::Ident("decimals".into())).is_err());
        assert!(fmt(ResolvedValue::Number(2.0)).is_err());
        assert!(fmt(pair("decimal", 2.5)).is_err());
        assert!(fmt(pair("decimal", 13.0)).is_err());
        assert!(fmt(pair("significant", 0.0)).is_err());
        assert!(fmt(pair("fraction", 1.0)).is_err());
        assert!(fmt(pair("fraction", 0.5)).is_err());
    }

    #[test]
    fn auto_matches_the_historic_tick_formatter() {
        assert_eq!(render(42.0, Format::Auto), "42");
        assert_eq!(render(2.5, Format::Auto), "2.5");
        assert_eq!(render(2.5001, Format::Auto), "2.5001");
        // The historic trim keeps the sign on a rounded-to-zero fraction; `auto`
        // is pinned byte-identical to it (`no_neg_zero` is the other families').
        assert_eq!(render(-0.00001, Format::Auto), "-0");
        assert_eq!(render(-3.0, Format::Auto), "-3");
    }

    #[test]
    fn decimal_keeps_trailing_zeros() {
        assert_eq!(render(2.5, Format::Decimal(2)), "2.50");
        assert_eq!(render(2.567, Format::Decimal(2)), "2.57");
        assert_eq!(render(3.0, Format::Decimal(0)), "3");
        assert_eq!(render(-0.001, Format::Decimal(2)), "0.00");
    }

    #[test]
    fn significant_rounds_both_sides_of_the_point() {
        assert_eq!(render(1234.0, Format::Significant(3)), "1230");
        assert_eq!(render(0.012345, Format::Significant(3)), "0.0123");
        assert_eq!(render(1.5, Format::Significant(3)), "1.50");
        assert_eq!(render(999.9, Format::Significant(3)), "1000");
        assert_eq!(render(0.0, Format::Significant(3)), "0");
        assert_eq!(render(-1234.0, Format::Significant(2)), "-1200");
    }

    #[test]
    fn scientific_and_engineering() {
        assert_eq!(render(12345.0, Format::Scientific(2)), "1.23e4");
        assert_eq!(render(0.00123, Format::Scientific(1)), "1.2e-3");
        assert_eq!(render(9.99, Format::Scientific(1)), "1.0e1");
        assert_eq!(render(0.0, Format::Scientific(1)), "0.0e0");
        assert_eq!(render(12345.0, Format::Engineering(1)), "12.3e3");
        assert_eq!(render(0.00123, Format::Engineering(0)), "1e-3");
        assert_eq!(render(999.96, Format::Engineering(1)), "1.0e3");
        assert_eq!(render(-2500.0, Format::Engineering(1)), "-2.5e3");
    }

    #[test]
    fn percent_scales_by_100() {
        assert_eq!(render(0.755, Format::Percent(1)), "75.5%");
        assert_eq!(render(1.0, Format::Percent(0)), "100%");
        assert_eq!(render(-0.00001, Format::Percent(1)), "0.0%");
    }

    #[test]
    fn fraction_rounds_and_reduces() {
        assert_eq!(render(1.375, Format::Fraction(8)), "1 3/8");
        assert_eq!(render(0.75, Format::Fraction(8)), "3/4");
        assert_eq!(render(2.0, Format::Fraction(8)), "2");
        assert_eq!(render(2.99, Format::Fraction(4)), "3");
        assert_eq!(render(-1.25, Format::Fraction(4)), "-1 1/4");
        assert_eq!(render(-0.01, Format::Fraction(4)), "0");
        assert_eq!(fraction_parts(1.375, 8), (false, 1, 3, 8));
    }

    #[test]
    fn cascade_reads_like_tooltip() {
        let mut attrs = AttrMap::new();
        assert_eq!(
            read_or(&attrs, Format::Decimal(1), Span::empty()).unwrap(),
            Format::Decimal(1)
        );
        attrs.insert("format", pair("percent", 0.0));
        assert_eq!(
            read_or(&attrs, Format::Decimal(1), Span::empty()).unwrap(),
            Format::Percent(0)
        );
        assert_eq!(numeric(Format::Date(DateUnit::Day)), Format::Auto);
        assert_eq!(numeric(Format::Percent(1)), Format::Percent(1));
    }
}

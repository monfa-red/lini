//! The axis attribute readers [SPEC 14.4/16/20]: `range:`, `ticks:`, `step:`,
//! `side:`, `gridlines:`, `unit:`, `scale:`, and `format:`, plus the shared
//! domain-from-`range` resolution. `axes.rs` binds series to axes and builds the
//! scales; the parsing of each attribute lives here. The numeric and time
//! `range:` / `ticks:` readers share one envelope each, differing only in the
//! per-value reader.

use super::super::*;

/// The shared `range:` envelope [SPEC 14.4] — a two-item tuple, each end read by
/// `end_of`; the arity message lives here once for the numeric and time axes.
fn read_range_with(
    inst: &ResolvedInst,
    end_of: impl Fn(&ResolvedValue) -> Result<End, Error>,
) -> Result<Option<(End, End)>, Error> {
    let Some(v) = inst.attrs.get("range") else {
        return Ok(None);
    };
    let ResolvedValue::Tuple(items) = v else {
        return Err(Error::at(
            inst.span,
            "'range' takes two ends: 'a b', 'a auto', or 'auto b'",
        ));
    };
    if items.len() != 2 {
        return Err(Error::at(
            inst.span,
            "'range' takes two ends: 'a b', 'a auto', or 'auto b'",
        ));
    }
    Ok(Some((end_of(&items[0])?, end_of(&items[1])?)))
}

/// A value / numeric axis's `range:` — two number / `auto` ends [SPEC 14.4].
pub(super) fn read_range(inst: &ResolvedInst) -> Result<Option<(End, End)>, Error> {
    read_range_with(inst, |v| read_end(v, inst.span))
}

/// A time axis's `range:` — two ends, each a quoted date or `auto`; a plain
/// number is the mixed-domain error [SPEC 14.4/20].
pub(super) fn read_time_range(inst: &ResolvedInst) -> Result<Option<(End, End)>, Error> {
    read_range_with(inst, |v| match v {
        ResolvedValue::String(text) => date_secs(text, inst.span).map(End::Num),
        ResolvedValue::Ident(s) if s == "auto" => Ok(End::Auto),
        _ => Err(Error::at(
            inst.span,
            "the x axis mixes dates and numbers — one domain, one kind",
        )),
    })
}

/// The shared explicit-`ticks:` envelope [SPEC 2/14.4]: a comma-list (or a lone
/// value), each item read by `value_of`.
fn read_ticks_with(
    attrs: &AttrMap,
    value_of: impl Fn(&ResolvedValue) -> Result<f64, Error>,
) -> Result<Option<Vec<f64>>, Error> {
    let Some(v) = attrs.get("ticks") else {
        return Ok(None);
    };
    let items = match v {
        ResolvedValue::List(items) => items.as_slice(),
        one => std::slice::from_ref(one),
    };
    items
        .iter()
        .map(value_of)
        .collect::<Result<Vec<f64>, Error>>()
        .map(Some)
}

/// An explicit numeric `ticks:` list — comma-separated numbers [SPEC 2/14.4].
pub(super) fn read_ticks(attrs: &AttrMap, span: Span) -> Result<Option<Vec<f64>>, Error> {
    read_ticks_with(attrs, |it| {
        it.as_number().ok_or_else(|| {
            Error::at(
                span,
                "'ticks' takes comma-separated numbers — 'ticks: 0, 50, 100'",
            )
        })
    })
}

/// A time axis's explicit `ticks:` — comma-separated quoted dates.
pub(super) fn read_time_ticks(inst: &ResolvedInst) -> Result<Option<Vec<f64>>, Error> {
    read_ticks_with(&inst.attrs, |it| match it {
        ResolvedValue::String(text) => date_secs(text, inst.span),
        _ => Err(Error::at(
            inst.span,
            "the x axis mixes dates and numbers — one domain, one kind",
        )),
    })
}

/// A calendar `step:` [SPEC 14.4] — a unit ident with an optional count
/// (`step: month`, `step: 2 week`); a plain number points at the calendar form.
pub(super) fn read_cal_step(inst: &ResolvedInst) -> Result<Option<(scale::CalUnit, u32)>, Error> {
    const CAL: &str = "a time axis steps by calendar — 'step: month', 'step: 2 week'";
    let unit = |s: &str| -> Option<scale::CalUnit> {
        Some(match s {
            "minute" => scale::CalUnit::Minute,
            "hour" => scale::CalUnit::Hour,
            "day" => scale::CalUnit::Day,
            "week" => scale::CalUnit::Week,
            "month" => scale::CalUnit::Month,
            "year" => scale::CalUnit::Year,
            _ => return None,
        })
    };
    match inst.attrs.get("step") {
        None => Ok(None),
        Some(ResolvedValue::Ident(s)) => match unit(s) {
            Some(u) => Ok(Some((u, 1))),
            None => Err(Error::at(inst.span, CAL)),
        },
        Some(ResolvedValue::Tuple(items)) => match items.as_slice() {
            [ResolvedValue::Number(n), ResolvedValue::Ident(s)]
                if n.fract() == 0.0 && (1.0..=1000.0).contains(n) =>
            {
                match unit(s) {
                    Some(u) => Ok(Some((u, *n as u32))),
                    None => Err(Error::at(inst.span, CAL)),
                }
            }
            _ => Err(Error::at(inst.span, CAL)),
        },
        Some(_) => Err(Error::at(inst.span, CAL)),
    }
}

/// A quoted date literal to epoch seconds, with the SPEC 20 message.
fn date_secs(text: &str, span: Span) -> Result<f64, Error> {
    date::parse(text).ok_or_else(|| {
        Error::at(
            span,
            format!("'{text}' is not a date — ISO-8601: '2026-01-31', optionally 'T09:30' and 'Z'"),
        )
    })
}

/// The domain from a value list, an explicit `range:` window, and the empty-data
/// fallback: `(min, max, reversed)`. A `range:` end of `auto` takes the data bound;
/// a high→low range reverses. Shared by the numeric x, time, and value scales (the
/// value scale supplies its own bars-include-zero `None` branch instead).
pub(super) fn resolve_domain(
    xs: &[f64],
    range: Option<&(End, End)>,
    empty: (f64, f64),
) -> (f64, f64, bool) {
    let data_min = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let data_max = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (dmin, dmax) = if xs.is_empty() {
        empty
    } else {
        (data_min, data_max)
    };
    match range {
        Some((a, b)) => {
            let lo = end(a, dmin);
            let hi = end(b, dmax);
            (lo.min(hi), lo.max(hi), lo > hi)
        }
        None => (dmin, dmax, false),
    }
}

/// A numeric consumer's `format:` [SPEC 16]: its own (a date preset authored
/// here errors — it reads a time axis), else the chart's numeric reading.
pub(crate) fn numeric_fmt(inst: &ResolvedInst, chart_fmt: Format) -> Result<Format, Error> {
    let f = format::read_or(&inst.attrs, format::numeric(chart_fmt), inst.span)?;
    if inst.attrs.get("format").is_some() {
        format::reject_date(f, inst.span)?;
    }
    Ok(format::numeric(f))
}

/// An axis's `scale:` kind [SPEC 14.4]: `linear` (default), `log`, or `time`.
#[derive(PartialEq, Clone, Copy)]
pub(super) enum ScaleKind {
    Linear,
    Log,
    Time,
}

pub(super) fn read_scale_kind(inst: &ResolvedInst) -> Result<ScaleKind, Error> {
    match inst.attrs.get("scale") {
        None => Ok(ScaleKind::Linear),
        Some(ResolvedValue::Ident(s)) if s == "linear" => Ok(ScaleKind::Linear),
        Some(ResolvedValue::Ident(s)) if s == "log" => Ok(ScaleKind::Log),
        Some(ResolvedValue::Ident(s)) if s == "time" => Ok(ScaleKind::Time),
        _ => Err(Error::at(inst.span, "'scale' is linear, log, or time")),
    }
}

pub(super) fn read_log(inst: &ResolvedInst) -> Result<bool, Error> {
    Ok(read_scale_kind(inst)? == ScaleKind::Log)
}

pub(crate) fn read_side(inst: &ResolvedInst) -> Result<Option<Side>, Error> {
    match inst.attrs.get("side") {
        None => Ok(None),
        Some(ResolvedValue::Ident(s)) => match s.as_str() {
            "bottom" => Ok(Some(Side::Bottom)),
            "top" => Ok(Some(Side::Top)),
            "left" => Ok(Some(Side::Left)),
            "right" => Ok(Some(Side::Right)),
            _ => Err(Error::at(
                inst.span,
                "'side' is bottom, top, left, or right",
            )),
        },
        _ => Err(Error::at(
            inst.span,
            "'side' is bottom, top, left, or right",
        )),
    }
}

pub(super) fn read_grid(inst: &ResolvedInst) -> Result<Grid, Error> {
    match inst.attrs.get("gridlines") {
        None => Ok(Grid::Default),
        Some(ResolvedValue::Ident(s)) if s == "none" => Ok(Grid::Off),
        Some(v) => Ok(Grid::Color(v.clone())),
    }
}

fn read_end(v: &ResolvedValue, span: Span) -> Result<End, Error> {
    match v {
        ResolvedValue::Number(n) => Ok(End::Num(*n)),
        ResolvedValue::Ident(s) if s == "auto" => Ok(End::Auto),
        _ => Err(Error::at(span, "a 'range' end is a number or 'auto'")),
    }
}

pub(super) fn end(e: &End, auto: f64) -> f64 {
    match e {
        End::Num(n) => *n,
        End::Auto => auto,
    }
}

pub(super) fn read_unit(inst: &ResolvedInst) -> Result<Option<String>, Error> {
    match inst.attrs.get("unit") {
        None => Ok(None),
        Some(ResolvedValue::String(s)) => Ok(Some(s.clone())),
        _ => Err(Error::at(inst.span, "'unit' is a quoted string")),
    }
}

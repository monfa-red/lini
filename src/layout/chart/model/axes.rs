//! The x (domain) and value axes: binding, sides, ranges, scales, and ticks.

use super::*;

/// Bind a series to a value axis by its `axis:` id, defaulting to the first value
/// axis. An unknown id reports the chart's own axis ids [SPEC 20].
pub(super) fn bind_axis(inst: &ResolvedInst, specs: &[AxisSpec]) -> Result<usize, Error> {
    let Some(id) = axis_id(inst) else {
        return Ok(0);
    };
    if let Some(pos) = specs.iter().position(|a| a.id == Some(id)) {
        return Ok(pos);
    }
    let known: Vec<&str> = specs.iter().filter_map(|a| a.id).collect();
    Err(no_axis(id, &known, inst.span))
}

/// A node's `axis:` binding id, if any.
pub(super) fn axis_id(inst: &ResolvedInst) -> Option<&str> {
    match inst.attrs.get("axis") {
        Some(ResolvedValue::Ident(s)) => Some(s.as_str()),
        _ => None,
    }
}

/// The "axis 'X' not found; did you mean 'Y'?" error [SPEC 20], shared by
/// series, band, and mark binding. Axes are chart-local (not in the global index),
/// so the suggestion ranges over the chart's own `|axis|` ids.
fn no_axis(id: &str, known: &[&str], span: Span) -> Error {
    Error::at(
        span,
        format!(
            "axis '{id}' not found{}",
            crate::suggest::did_you_mean(known)
        ),
    )
}

/// Resolve a band / mark `axis:` id to the x axis or a value axis [SPEC 14.5].
pub(super) fn lookup_axis(
    id: &str,
    x_id: Option<&str>,
    specs: &[AxisSpec],
    span: Span,
) -> Result<AxisRef, Error> {
    if x_id == Some(id) {
        return Ok(AxisRef::X);
    }
    if let Some(pos) = specs.iter().position(|a| a.id == Some(id)) {
        return Ok(AxisRef::Value(pos));
    }
    let mut known: Vec<&str> = Vec::new();
    known.extend(x_id);
    known.extend(specs.iter().filter_map(|a| a.id));
    Err(no_axis(id, &known, span))
}

pub(super) fn build_x_axis(
    x_inst: Option<&ResolvedInst>,
    categories: &Option<Vec<String>>,
    series: &[Series],
    segments: &[(f64, f64)],
    bubbles: &[Bubble],
    span: Span,
) -> Result<XAxis, Error> {
    let (title, unit, grid) = match x_inst {
        Some(a) => (label_of(a), read_unit(a)?, read_grid(a)?),
        None => (None, None, Grid::Default),
    };
    // Categorical when categories are set or every series is categorical; numeric when
    // the data is points / a formula / bubbles, or a bottom axis fixes a range.
    let any_numeric = !bubbles.is_empty()
        || series
            .iter()
            .any(|s| matches!(s.data, Data::Points(_) | Data::Formula(_)));
    if let Some(cats) = categories {
        return Ok(XAxis {
            scale: Scale::band(cats.len()),
            labels: cats.clone(),
            title,
            unit,
            grid,
        });
    }
    if !any_numeric {
        let n = series
            .iter()
            .map(|s| match &s.data {
                Data::Categorical(v) => v.len(),
                _ => 0,
            })
            .max()
            .unwrap_or(0);
        if n == 0 {
            return Err(Error::at(
                span,
                "a chart series needs at least one data value",
            ));
        }
        return Ok(XAxis {
            scale: Scale::band(n),
            labels: Vec::new(),
            title,
            unit,
            grid,
        });
    }
    // Numeric x: domain from a bottom axis `range:`, else the union of point x's (a
    // formula contributes none — it samples over whatever domain this fixes). With no
    // point data, x-bound bands define the domain (the segmentation case, [SPEC 14.5]).
    let mut xs: Vec<f64> = series
        .iter()
        .flat_map(|s| match &s.data {
            Data::Points(p) => p.iter().map(|(x, _)| *x).collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect();
    xs.extend(bubbles.iter().map(|b| b.at.0));
    if xs.is_empty() {
        for &(a, b) in segments {
            xs.push(a);
            xs.push(b);
        }
    }
    let range = x_inst.map(read_range).transpose()?.flatten();
    // Bubbles have a drawn radius, so pad the auto domain to keep edge bubbles inside.
    if range.is_none() && !bubbles.is_empty() {
        let lo = xs.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let pad = ((hi - lo) * 0.1).max(1.0);
        xs.push(lo - pad);
        xs.push(hi + pad);
    }
    let scale = numeric_scale(&xs, range, x_inst)?;
    Ok(XAxis {
        scale,
        labels: Vec::new(),
        title,
        unit,
        grid,
    })
}

pub(super) fn build_value_axes(
    specs: Vec<AxisSpec>,
    series: &[Series],
    bars: &BarMode,
    bubbles: &[Bubble],
) -> Result<Vec<ValueAxis>, Error> {
    let mut out = Vec::with_capacity(specs.len());
    for (i, spec) in specs.iter().enumerate() {
        // The value range bound to this axis. Non-bar series contribute their values
        // (`Points` their y; formulas were sampled to `Points` before this runs). Bar
        // series contribute their values too — except stacked bars, whose envelope is
        // the per-category sum (the top of the pile, [SPEC 14.2]).
        let mut vals: Vec<f64> = Vec::new();
        let bar_data: Vec<&[f64]> = series
            .iter()
            .filter(|s| s.axis == i && matches!(s.kind, SeriesKind::Bars))
            .filter_map(|s| match &s.data {
                Data::Categorical(v) => Some(v.as_slice()),
                _ => None,
            })
            .collect();
        for s in series
            .iter()
            .filter(|s| s.axis == i && !matches!(s.kind, SeriesKind::Bars))
        {
            match &s.data {
                Data::Categorical(v) => vals.extend(v),
                Data::Points(p) => vals.extend(p.iter().map(|(_, y)| *y)),
                Data::Formula(_) => {}
            }
        }
        vals.extend(bubbles.iter().filter(|b| b.axis == i).map(|b| b.at.1));
        if matches!(bars, BarMode::Stacked) {
            let n = bar_data.iter().map(|v| v.len()).max().unwrap_or(0);
            for c in 0..n {
                vals.push(
                    bar_data
                        .iter()
                        .map(|v| v.get(c).copied().unwrap_or(0.0))
                        .sum(),
                );
            }
        } else {
            for v in &bar_data {
                vals.extend(*v);
            }
        }
        let scale = value_scale(&vals, !bar_data.is_empty(), spec)?;
        out.push(ValueAxis {
            side: matches!(spec.side, Side::Right)
                .then_some(Side::Right)
                .unwrap_or(Side::Left),
            scale,
            title: spec.title.clone(),
            unit: spec.unit.clone(),
            grid: clone_grid(&spec.grid),
            primary: i == 0,
        });
    }
    Ok(out)
}

/// A value axis's scale: its data domain (bars include zero), honouring an explicit
/// `range:` window / reverse and `step:` / `ticks:` [SPEC 14.4].
fn value_scale(vals: &[f64], has_bars: bool, spec: &AxisSpec) -> Result<Scale, Error> {
    let data_min = vals.iter().copied().fold(f64::INFINITY, f64::min);
    let data_max = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (dmin, dmax) = if vals.is_empty() {
        (0.0, 1.0)
    } else {
        (data_min, data_max)
    };
    if spec.log {
        let lo = spec.range.as_ref().map_or(dmin, |(a, _)| end(a, dmin));
        let hi = spec.range.as_ref().map_or(dmax, |(_, b)| end(b, dmax));
        return log_scale(lo, hi, spec.range.is_some(), Span::empty());
    }
    let (min, max, rev) = match &spec.range {
        Some((a, b)) => {
            let lo = end(a, dmin);
            let hi = end(b, dmax);
            if (lo - hi).abs() < f64::EPSILON {
                return Err(Error::at(Span::empty(), "'range' needs distinct ends"));
            }
            (lo.min(hi), lo.max(hi), lo > hi)
        }
        None => {
            let lo = if has_bars || dmin >= 0.0 {
                0.0
            } else {
                -scale::nice_max(-dmin)
            };
            let hi = scale::nice_max(dmax.max(0.0));
            (lo, hi, false)
        }
    };
    let ticks = axis_ticks(min, max, spec);
    Ok(Scale::linear(min, max, rev, ticks))
}

/// A numeric x scale (a scatter's x, a formula's domain, or a `range:`-fixed bottom
/// axis). Empty data (a formula-only chart with no range) defaults to `[0, 1]`.
fn numeric_scale(
    xs: &[f64],
    range: Option<(End, End)>,
    spec_src: Option<&ResolvedInst>,
) -> Result<Scale, Error> {
    let data_min = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let data_max = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (dmin, dmax) = if xs.is_empty() {
        (0.0, 1.0)
    } else {
        (data_min, data_max)
    };
    if spec_src.is_some_and(|a| read_log(a).unwrap_or(false)) {
        let lo = range.as_ref().map_or(dmin, |(a, _)| end(a, dmin));
        let hi = range.as_ref().map_or(dmax, |(_, b)| end(b, dmax));
        let span = spec_src.map_or(Span::empty(), |a| a.span);
        return log_scale(lo, hi, range.is_some(), span);
    }
    let (min, max, rev) = match range {
        Some((a, b)) => {
            let lo = end(&a, dmin);
            let hi = end(&b, dmax);
            (lo.min(hi), lo.max(hi), lo > hi)
        }
        None => (dmin, dmax, false),
    };
    let step = spec_src.and_then(|a| a.attrs.number("step"));
    let explicit_ticks = match spec_src {
        Some(a) => read_ticks(&a.attrs, a.span)?,
        None => None,
    };
    let ticks = if let Some(t) = explicit_ticks {
        t
    } else if let Some(st) = step {
        scale::ticks_by_step(min, max, st)
    } else {
        scale::nice_ticks(min, max)
    };
    Ok(Scale::linear(min, max, rev, ticks))
}

/// A log scale over a positive domain [SPEC 14.4]: the data domain is rounded
/// out to whole decades unless an explicit `range:` fixes it. A non-positive domain
/// is an error.
fn log_scale(lo: f64, hi: f64, has_range: bool, span: Span) -> Result<Scale, Error> {
    if lo <= 0.0 || hi <= 0.0 {
        return Err(Error::at(
            span,
            "a 'scale: log' axis needs a domain above 0",
        ));
    }
    let (a, b) = (lo.min(hi), lo.max(hi));
    let (min, max) = if has_range {
        (a, b)
    } else {
        (10f64.powf(a.log10().floor()), 10f64.powf(b.log10().ceil()))
    };
    Ok(Scale::log(min, max))
}

/// An explicit `ticks:` list — comma-separated numbers [SPEC 2/14.4].
fn read_ticks(attrs: &AttrMap, span: Span) -> Result<Option<Vec<f64>>, Error> {
    let Some(v) = attrs.get("ticks") else {
        return Ok(None);
    };
    let items = match v {
        ResolvedValue::List(items) => items.as_slice(),
        one => std::slice::from_ref(one),
    };
    items
        .iter()
        .map(|it| {
            it.as_number().ok_or_else(|| {
                Error::at(
                    span,
                    "'ticks' takes comma-separated numbers — 'ticks: 0, 50, 100'",
                )
            })
        })
        .collect::<Result<Vec<f64>, Error>>()
        .map(Some)
}

fn axis_ticks(min: f64, max: f64, spec: &AxisSpec) -> Vec<f64> {
    if let Some(t) = &spec.ticks {
        t.clone()
    } else if let Some(step) = spec.step {
        scale::ticks_by_step(min, max, step)
    } else {
        scale::nice_ticks(min, max)
    }
}

pub(super) fn axis_spec(inst: &ResolvedInst, side: Side) -> Result<AxisSpec<'_>, Error> {
    Ok(AxisSpec {
        id: inst.id.as_deref(),
        side,
        title: label_of(inst),
        unit: read_unit(inst)?,
        grid: read_grid(inst)?,
        range: read_range(inst)?,
        step: inst.attrs.number("step"),
        ticks: read_ticks(&inst.attrs, inst.span)?,
        log: read_log(inst)?,
    })
}

/// Whether an axis is `scale: log` [SPEC 14.4]; `scale:` accepts only
/// `linear` / `log`.
fn read_log(inst: &ResolvedInst) -> Result<bool, Error> {
    match inst.attrs.get("scale") {
        None => Ok(false),
        Some(ResolvedValue::Ident(s)) if s == "linear" => Ok(false),
        Some(ResolvedValue::Ident(s)) if s == "log" => Ok(true),
        _ => Err(Error::at(inst.span, "'scale' is linear or log")),
    }
}

// ───────────────────────────── attribute readers ─────────────────────────────

pub(super) fn read_side(inst: &ResolvedInst) -> Result<Option<Side>, Error> {
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

fn read_grid(inst: &ResolvedInst) -> Result<Grid, Error> {
    match inst.attrs.get("gridlines") {
        None => Ok(Grid::Default),
        Some(ResolvedValue::Ident(s)) if s == "none" => Ok(Grid::Off),
        Some(v) => Ok(Grid::Color(v.clone())),
    }
}

fn read_range(inst: &ResolvedInst) -> Result<Option<(End, End)>, Error> {
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
    Ok(Some((
        read_end(&items[0], inst.span)?,
        read_end(&items[1], inst.span)?,
    )))
}

fn read_end(v: &ResolvedValue, span: Span) -> Result<End, Error> {
    match v {
        ResolvedValue::Number(n) => Ok(End::Num(*n)),
        ResolvedValue::Ident(s) if s == "auto" => Ok(End::Auto),
        _ => Err(Error::at(span, "a 'range' end is a number or 'auto'")),
    }
}

fn end(e: &End, auto: f64) -> f64 {
    match e {
        End::Num(n) => *n,
        End::Auto => auto,
    }
}

fn read_unit(inst: &ResolvedInst) -> Result<Option<String>, Error> {
    match inst.attrs.get("unit") {
        None => Ok(None),
        Some(ResolvedValue::String(s)) => Ok(Some(s.clone())),
        _ => Err(Error::at(inst.span, "'unit' is a quoted string")),
    }
}

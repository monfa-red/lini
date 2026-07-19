//! Reading each series' data, labels, and markers, and sampling deferred formulas.

use super::*;
use crate::error::Code;

/// Parse a `|bubble|` [SPEC 14.2]: a labelled point `at: x y`, its `value` (area
/// size), the bound value axis, and its colour (explicit `fill`/`stroke`, else palette).
pub(super) fn read_bubble(
    inst: &ResolvedInst,
    index: usize,
    specs: &[AxisSpec],
    chart_tip: Tooltip,
) -> Result<Bubble, Error> {
    let needs = || Error::at(inst.span, "a '|bubble|' needs 'at:' (x y) and 'value:'");
    let MarkAt::Point(x, y) = read_at(inst).map_err(|_| needs())? else {
        return Err(needs());
    };
    let value = inst.attrs.number("value").ok_or_else(needs)?;
    let color = fill_color(&inst.attrs).unwrap_or_else(|| live(palette::hue(index)));
    Ok(Bubble {
        at: (x, y),
        value,
        axis: bind_axis(inst, specs)?,
        label: label_of(inst),
        color,
        outline: outline(&inst.attrs),
        tooltip: super::tooltip::read_or(&inst.attrs, chart_tip)?,
    })
}

pub(super) fn read_series(
    inst: &ResolvedInst,
    index: usize,
    value_specs: &[AxisSpec],
    categories: &Option<Vec<String>>,
    chart_tip: Tooltip,
    chart_fmt: Format,
    _chart_span: Span,
) -> Result<Series, Error> {
    let kind = match tag(inst) {
        Some("bars") => SeriesKind::Bars,
        Some("dots") => SeriesKind::Dots,
        Some("area") => SeriesKind::Area,
        _ => SeriesKind::Line,
    };
    let has_data = inst.attrs.get("data").is_some();
    let has_fn = matches!(inst.attrs.get("fn"), Some(ResolvedValue::Deferred(_)));
    let (data, time_x) = match (has_data, has_fn) {
        (true, true) => {
            return Err(
                Error::at(inst.span, "a series takes 'data' or 'fn', not both")
                    .code(Code::CHART_DATA),
            );
        }
        (false, false) => {
            return Err(
                Error::at(inst.span, "a series needs 'data' or 'fn'").code(Code::CHART_DATA)
            );
        }
        (false, true) => match inst.attrs.get("fn") {
            Some(ResolvedValue::Deferred(exprs)) => (Data::Formula(exprs.clone()), false),
            _ => {
                return Err(
                    Error::at(inst.span, "a series needs 'data' or 'fn'").code(Code::CHART_DATA)
                );
            }
        },
        (true, false) => read_data(inst, &kind)?,
    };
    if categories.is_some() && !matches!(data, Data::Categorical(_)) {
        return Err(Error::at(
            inst.span,
            "point / formula data needs a numeric x axis, not 'categories'",
        ));
    }
    let axis = bind_axis(inst, value_specs)?;
    // Paint by role [SPEC 14.6]: a fill shape (bars / area) takes its body from
    // `fill:`, a line its colour from `stroke:`, dots from either. An explicit `stroke:`
    // is a separate outline (read into `outline` below), never the body — so a stroke on
    // a bar no longer leaks into its fill. No explicit paint → walk the palette at the
    // role's tier (a line the deep stroke, dots the ink, a bar the base fill).
    // A comma list is per-datum paint [SPEC 14.6], not the series base — the
    // base derivation (and the legend swatch) reads as if the key were unset.
    let listed = |k: &str| matches!(inst.attrs.get(k), Some(ResolvedValue::List(_)));
    let fill = (!listed("fill")).then(|| fill_color(&inst.attrs)).flatten();
    let stroke = (!listed("stroke"))
        .then(|| real_color(inst.attrs.get("stroke")))
        .flatten();
    let color = match kind {
        SeriesKind::Bars | SeriesKind::Area => fill,
        SeriesKind::Line => stroke.or(fill),
        SeriesKind::Dots => fill.or(stroke),
    }
    .unwrap_or_else(|| {
        // The outlined look [SPEC 14.6]: a bar / area fills with the **soft** tier
        // and gains a **deep** edge below; a line takes the deep stroke, dots the ink.
        let suffix = match kind {
            SeriesKind::Line => "-deep",
            SeriesKind::Dots => "-ink",
            SeriesKind::Bars | SeriesKind::Area => "-soft",
        };
        live(&format!("{}{}", palette::hue(index), suffix))
    });
    let dot_w = inst.attrs.number("width").unwrap_or(7.0);
    let dot_h = inst.attrs.number("height").unwrap_or(dot_w);
    // A `|dots|` *is* markers, so an unset marker draws a round `dot` (sized by `width`);
    // every other series draws vertex markers only when `marker:` asks for them.
    let marker = chart_marker(inst)?;
    let marker = if matches!(kind, SeriesKind::Dots) && marker == MarkerKind::None {
        MarkerKind::Dot
    } else {
        marker
    };
    let labels = read_labels(inst, &data)?;
    let tooltip = super::tooltip::read_or(&inst.attrs, chart_tip)?;
    let tag_color = real_color(inst.attrs.get("color")).unwrap_or_else(muted);
    // `|bars|` default to a deep edge of their soft fill (the outlined look, [SPEC 14.6]); an
    // `|area|` reads its explicit `stroke` here and otherwise deepens its fill at draw.
    let edge = match kind {
        SeriesKind::Bars if listed("stroke") => {
            // The listed stroke rides `per_datum`; the base edge is the default
            // deep tier (what an unset stroke gives).
            let width = inst.attrs.number("stroke-width").unwrap_or(1.5);
            Some((palette::deepen(&color), width))
        }
        SeriesKind::Bars => fill_outline(&inst.attrs, &color),
        _ if listed("stroke") => None,
        _ => outline(&inst.attrs),
    };
    let per_datum = paint_lists(inst, &kind, &color, &data)?;
    Ok(Series {
        kind,
        data,
        label: label_of(inst),
        color,
        axis,
        marker,
        labels,
        tooltip,
        tag_color,
        curve: read_curve(&inst.attrs)?,
        stroke_style: inst.attrs.get("stroke-style").cloned(),
        outline: edge,
        thickness: inst.attrs.number("stroke-width").unwrap_or(2.0),
        radius: inst.attrs.number("radius").unwrap_or(0.0),
        dot: (dot_w, dot_h),
        baseline: inst.attrs.number("baseline"),
        fmt: axes::numeric_fmt(inst, chart_fmt)?,
        per_datum,
        time_x,
    })
}

/// Sample a `fn:` over the x-domain → points [SPEC 14.3]. A single expr is the
/// whole-domain form: bind `x` at `samples` steps over the numeric domain. A per-band
/// list samples each expr in band-local `u` (0→1) across its segment's x-span, the
/// segments connecting end-to-start [SPEC 14.5] — one continuous polyline whose
/// boundary risers are drawn. A list length ≠ the band count is an error ([SPEC 20]).
pub(super) fn sample_formula(
    exprs: &[Expr],
    x: &Scale,
    samples: usize,
    funcs: &FuncTable,
    span: Span,
    segments: &[(f64, f64)],
) -> Result<Data, Error> {
    let n = samples.max(2);
    if exprs.len() == 1 {
        let (min, max) = match x {
            Scale::Linear { min, max, .. }
            | Scale::Log { min, max, .. }
            | Scale::Time { min, max, .. } => (*min, *max),
            Scale::Band { .. } => {
                return Err(Error::at(span, "a 'fn:' series needs a numeric x axis"));
            }
        };
        let xs: Vec<f64> = (0..n)
            .map(|i| min + (max - min) * i as f64 / (n - 1) as f64)
            .collect();
        let ys = expr::sample(&exprs[0], "x", &xs, funcs).map_err(|e| Error::at(span, e.0))?;
        return Ok(Data::Points(points_from(&xs, ys, span)?));
    }
    if exprs.len() != segments.len() {
        return Err(Error::at(
            span,
            format!(
                "'fn' has {} formulas but the chart has {} bands",
                exprs.len(),
                segments.len()
            ),
        ));
    }
    let us: Vec<f64> = (0..n).map(|i| i as f64 / (n - 1) as f64).collect();
    let mut pts = Vec::new();
    for (expr, &(a, b)) in exprs.iter().zip(segments) {
        let ys = expr::sample(expr, "u", &us, funcs).map_err(|e| Error::at(span, e.0))?;
        let xs: Vec<f64> = us.iter().map(|u| a + (b - a) * u).collect();
        pts.extend(points_from(&xs, ys, span)?);
    }
    Ok(Data::Points(pts))
}

/// Zip sampled xs with their evaluated ys into points; a point-valued result is an
/// error (a series `fn:` must return a number, [SPEC 14.3]).
fn points_from(xs: &[f64], ys: Vec<ExprValue>, span: Span) -> Result<Vec<(f64, f64)>, Error> {
    let mut pts = Vec::with_capacity(xs.len());
    for (&xv, yv) in xs.iter().zip(ys) {
        match yv {
            ExprValue::Number(y) => pts.push((xv, y)),
            ExprValue::Point(..) => {
                return Err(Error::at(span, "a 'fn:' expression must return a number"));
            }
        }
    }
    Ok(pts)
}

/// `data:` reads across comma-groups [SPEC 2/14.3]: values (`data: 9, 15, 24`)
/// → categorical, `x y` pairs (`data: 10 20, 30 40`) → points — so a lone
/// `data: 10 20` is one point, never two values. A point's x may be a quoted
/// ISO-8601 date (epoch seconds — the second return); dates and numbers never
/// mix in one domain [SPEC 14.3/20]. Bars are categorical only.
fn read_data(inst: &ResolvedInst, kind: &SeriesKind) -> Result<(Data, bool), Error> {
    let Some(ResolvedValue::List(items)) = inst.attrs.get("data") else {
        return Err(Error::at(inst.span, "'data' must be a list of numbers").code(Code::CHART_DATA));
    };
    if items.iter().all(|it| it.as_number().is_some()) {
        return Ok((Data::Categorical(numbers(items, inst.span)?), false));
    }
    let mut pts = Vec::with_capacity(items.len());
    let mut dates = 0usize;
    for it in items {
        match it {
            ResolvedValue::Tuple(pair) if pair.len() == 2 => {
                let x = match &pair[0] {
                    ResolvedValue::String(text) => {
                        dates += 1;
                        date::parse(text).ok_or_else(|| {
                            Error::at(
                                inst.span,
                                format!(
                                    "'{text}' is not a date — ISO-8601: '2026-01-31', \
                                     optionally 'T09:30' and 'Z'"
                                ),
                            )
                        })?
                    }
                    v => number(v, inst.span)?,
                };
                pts.push((x, number(&pair[1], inst.span)?));
            }
            // A longer space run is the pre-0.21 value list.
            ResolvedValue::Tuple(_) => {
                return Err(Error::at(
                    inst.span,
                    "'data' takes comma-separated values — 'data: 9, 15, 24'",
                )
                .code(Code::CHART_DATA));
            }
            _ => {
                return Err(
                    Error::at(inst.span, "point data is 'x y' pairs").code(Code::CHART_DATA)
                );
            }
        }
    }
    if dates > 0 && dates < pts.len() {
        return Err(Error::at(
            inst.span,
            "'data' mixes dates and numbers — one domain, one kind",
        )
        .code(Code::CHART_DATA));
    }
    if matches!(kind, SeriesKind::Bars) {
        return Err(Error::at(
            inst.span,
            "'|bars|' takes categorical data ('data: 9, 15, 24'), not 'x y' points",
        )
        .code(Code::CHART_DATA));
    }
    Ok((Data::Points(pts), dates > 0))
}

/// Parse a series' `labels:` [SPEC 14.3]: a quoted-string list, one per datum,
/// validated against the data count. A `fn:` series has no authored points to label, so
/// `labels:` on one is an error ([SPEC 20]). Reuses [`collect_strings`] (the `categories:`
/// reader), so the list parses exactly like the chart's category list.
fn read_labels(inst: &ResolvedInst, data: &Data) -> Result<Vec<String>, Error> {
    let Some(v) = inst.attrs.get("labels") else {
        return Ok(Vec::new());
    };
    let mut labels = Vec::new();
    collect_strings("labels", v, &mut labels, inst.span)?;
    let n = match data {
        Data::Categorical(values) => values.len(),
        Data::Points(p) => p.len(),
        Data::Formula(_) => {
            return Err(Error::at(
                inst.span,
                "'labels' needs explicit 'data' — a sampled 'fn' has no points to label",
            ));
        }
    };
    if labels.len() != n {
        return Err(Error::at(
            inst.span,
            format!(
                "'labels' has {} entries but the series has {} data points",
                labels.len(),
                n
            ),
        )
        .code(Code::CHART_DATA));
    }
    Ok(labels)
}

fn read_curve(attrs: &AttrMap) -> Result<Curve, Error> {
    match attrs.get("curve") {
        None => Ok(Curve::Linear),
        Some(ResolvedValue::Ident(s)) => match s.as_str() {
            "linear" => Ok(Curve::Linear),
            "smooth" => Ok(Curve::Smooth),
            "step" => Ok(Curve::Step),
            _ => Err(Error::at(
                Span::empty(),
                "'curve' is linear, smooth, or step",
            )),
        },
        _ => Err(Error::at(
            Span::empty(),
            "'curve' is linear, smooth, or step",
        )),
    }
}

/// The effective **centred** marker for a chart node [SPEC 14.2]: a line's
/// vertex, a `|dots|`, or a `|mark|` point. The `marker:` shorthand is extracted to the
/// resolved [`Markers`] (and dropped from the attr map), so this reads the resolved kind
/// (`start`, else `end` — `marker:` sets both; the directional ends have no chart
/// meaning). `marker: none` resolves to `None`; a `|mark|`'s template default `marker:
/// dot` separates an explicit `none` (label only) from a plain point (a dot). A chart
/// marker is centred, so the directional `arrow` / `crow` are rejected here ([SPEC 20]).
pub(super) fn chart_marker(inst: &ResolvedInst) -> Result<MarkerKind, Error> {
    let kind = if inst.markers.start != MarkerKind::None {
        inst.markers.start
    } else {
        inst.markers.end
    };
    match kind {
        MarkerKind::Arrow | MarkerKind::Crow => {
            let name = if kind == MarkerKind::Arrow {
                "arrow"
            } else {
                "crow"
            };
            Err(Error::at(
                inst.span,
                format!(
                    "'marker: {name}' has no centred form on a chart — use dot, circle, or diamond"
                ),
            ))
        }
        k => Ok(k),
    }
}

pub(super) fn collect_strings(
    name: &str,
    v: &ResolvedValue,
    out: &mut Vec<String>,
    span: Span,
) -> Result<(), Error> {
    let bad = || {
        Error::at(
            span,
            format!("'{name}' takes comma-separated quoted strings — '{name}: \"a\", \"b\"'"),
        )
    };
    let ResolvedValue::List(items) = v else {
        return Err(bad());
    };
    for it in items {
        match it {
            ResolvedValue::String(s) => out.push(s.clone()),
            _ => return Err(bad()),
        }
    }
    Ok(())
}

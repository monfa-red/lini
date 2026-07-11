//! Reading each series' data, tags, and markers, and sampling deferred formulas.

use super::*;

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
    let data = match (has_data, has_fn) {
        (true, true) => {
            return Err(Error::at(
                inst.span,
                "a series takes 'data' or 'fn', not both",
            ));
        }
        (false, false) => return Err(Error::at(inst.span, "a series needs 'data' or 'fn'")),
        (false, true) => match inst.attrs.get("fn") {
            Some(ResolvedValue::Deferred(exprs)) => Data::Formula(exprs.clone()),
            _ => return Err(Error::at(inst.span, "a series needs 'data' or 'fn'")),
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
    let fill = fill_color(&inst.attrs);
    let stroke = real_color(inst.attrs.get("stroke"));
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
    let tags = read_tags(inst, &data)?;
    let tooltip = super::tooltip::read_or(&inst.attrs, chart_tip)?;
    let tag_color = real_color(inst.attrs.get("color")).unwrap_or_else(muted);
    // `|bars|` default to a deep edge of their soft fill (the outlined look, [SPEC 14.6]); an
    // `|area|` reads its explicit `stroke` here and otherwise deepens its fill at draw.
    let edge = match kind {
        SeriesKind::Bars => fill_outline(&inst.attrs, &color),
        _ => outline(&inst.attrs),
    };
    Ok(Series {
        kind,
        data,
        label: label_of(inst),
        color,
        axis,
        marker,
        tags,
        tooltip,
        tag_color,
        curve: read_curve(&inst.attrs)?,
        stroke_style: inst.attrs.get("stroke-style").cloned(),
        outline: edge,
        thickness: inst.attrs.number("stroke-width").unwrap_or(2.0),
        radius: inst.attrs.number("radius").unwrap_or(0.0),
        dot: (dot_w, dot_h),
        baseline: inst.attrs.number("baseline"),
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
            Scale::Linear { min, max, .. } | Scale::Log { min, max, .. } => (*min, *max),
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

/// Categorical `data:` → values; comma-grouped `data:` → `x y` points. Bars are
/// categorical only.
fn read_data(inst: &ResolvedInst, kind: &SeriesKind) -> Result<Data, Error> {
    match inst.attrs.get("data") {
        Some(ResolvedValue::Number(n)) => Ok(Data::Categorical(vec![*n])),
        Some(ResolvedValue::Tuple(items)) => Ok(Data::Categorical(numbers(items, inst.span)?)),
        Some(ResolvedValue::List(items)) => {
            if matches!(kind, SeriesKind::Bars) {
                return Err(Error::at(
                    inst.span,
                    "'|bars|' takes categorical data ('data: 9 15 24'), not 'x y' points",
                ));
            }
            let mut pts = Vec::with_capacity(items.len());
            for it in items {
                match it {
                    ResolvedValue::Tuple(pair) if pair.len() == 2 => {
                        pts.push((number(&pair[0], inst.span)?, number(&pair[1], inst.span)?));
                    }
                    _ => return Err(Error::at(inst.span, "point data is 'x y' pairs")),
                }
            }
            Ok(Data::Points(pts))
        }
        _ => Err(Error::at(inst.span, "'data' must be a list of numbers")),
    }
}

/// Parse a series' `tags:` [SPEC 14.3]: a quoted-string list, one per datum,
/// validated against the data count. A `fn:` series has no authored points to label, so
/// `tags:` on one is an error ([SPEC 20]). Reuses [`collect_strings`] (the `categories:`
/// reader), so a tag list parses exactly like the chart's category list.
fn read_tags(inst: &ResolvedInst, data: &Data) -> Result<Vec<String>, Error> {
    let Some(v) = inst.attrs.get("tags") else {
        return Ok(Vec::new());
    };
    let mut tags = Vec::new();
    collect_strings(v, &mut tags, inst.span)?;
    let n = match data {
        Data::Categorical(values) => values.len(),
        Data::Points(p) => p.len(),
        Data::Formula(_) => {
            return Err(Error::at(
                inst.span,
                "'tags' needs explicit 'data' — a sampled 'fn' has no points to label",
            ));
        }
    };
    if tags.len() != n {
        return Err(Error::at(
            inst.span,
            format!(
                "'tags' has {} labels but the series has {} data points",
                tags.len(),
                n
            ),
        ));
    }
    Ok(tags)
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
    v: &ResolvedValue,
    out: &mut Vec<String>,
    span: Span,
) -> Result<(), Error> {
    match v {
        ResolvedValue::String(s) => out.push(s.clone()),
        ResolvedValue::Tuple(items) | ResolvedValue::List(items) => {
            for it in items {
                collect_strings(it, out, span)?;
            }
        }
        _ => return Err(Error::at(span, "'categories' is a list of quoted strings")),
    }
    Ok(())
}

//! `build` — read a chart's children into a [`Chart`], plus the orchestration helpers.

use super::*;

/// The children of a chart, split by role: series, axes, bands, marks, bubbles, and the
/// harvested title.
type Split<'a> = (
    Vec<&'a ResolvedInst>,
    Vec<&'a ResolvedInst>,
    Vec<&'a ResolvedInst>,
    Vec<&'a ResolvedInst>,
    Vec<&'a ResolvedInst>,
    Option<String>,
);

pub fn build(inst: &ResolvedInst, funcs: &FuncTable) -> Result<Chart, Error> {
    let span = inst.span;
    let dir = read_direction(&inst.attrs)?;
    let samples = sample_count(&inst.attrs);
    let bars = read_bars(&inst.attrs)?;
    let chart_tip = super::tooltip::read(&inst.attrs)?;
    let (series_insts, axis_insts, band_insts, mark_insts, bubble_insts, title) = partition(inst)?;
    if series_insts.is_empty() && bubble_insts.is_empty() {
        return Err(Error::at(span, "a chart needs at least one series"));
    }

    let categories = read_categories(&inst.attrs, span)?;

    // Split declared axes into the one domain (x) axis and the value axes, by which
    // screen edge plays the domain in this direction [SPEC 14.7]: the bottom/top
    // in a column, the left/right in a row. A radial chart has no sides — one radius
    // (value) axis, the domain being the spokes ([SPEC 14.7]).
    let mut x_inst: Option<&ResolvedInst> = None;
    let mut value_specs: Vec<AxisSpec> = Vec::new();
    let default_value_side = if dir == Dir::Row {
        Side::Bottom
    } else {
        Side::Left
    };
    for ax in &axis_insts {
        let side = read_side(ax)?;
        match dir {
            Dir::Radial => {
                if side.is_some() {
                    return Err(Error::at(
                        ax.span,
                        "'side' has no meaning in a radial chart — it has one radius axis",
                    ));
                }
                value_specs.push(axis_spec(ax, Side::Left)?);
            }
            Dir::Row => match side {
                Some(Side::Left | Side::Right) => x_inst = Some(ax),
                _ => value_specs.push(axis_spec(ax, side.unwrap_or(Side::Bottom))?),
            },
            Dir::Column => match side {
                Some(Side::Bottom | Side::Top) => x_inst = Some(ax),
                _ => value_specs.push(axis_spec(ax, side.unwrap_or(Side::Left))?),
            },
        }
    }
    if categories.is_some() && x_inst.is_some_and(|a| a.attrs.get("labels").is_some()) {
        return Err(Error::at(
            span,
            "set 'categories' or an axis 'labels', not both",
        ));
    }
    if value_specs.is_empty() {
        value_specs.push(AxisSpec {
            id: None,
            side: default_value_side,
            title: None,
            unit: None,
            grid: Grid::Default,
            range: None,
            step: None,
            ticks: None,
            log: false,
        });
    }

    // Read each series' data + style, binding it to a value axis by index.
    let mut series = Vec::with_capacity(series_insts.len());
    for (i, si) in series_insts.iter().enumerate() {
        series.push(read_series(
            si,
            i,
            &value_specs,
            &categories,
            chart_tip,
            span,
        )?);
    }

    // Bands and marks bind to an axis by id (the x axis or a value axis), so resolve
    // them while both id sources are in scope.
    let x_id = x_inst.and_then(|a| a.id.as_deref());
    let bands: Vec<Band> = band_insts
        .iter()
        .map(|b| read_band(b, x_id, &value_specs))
        .collect::<Result<_, _>>()?;
    let marks: Vec<Mark> = mark_insts
        .iter()
        .map(|m| read_mark(m, x_id, &value_specs, chart_tip))
        .collect::<Result<_, _>>()?;
    let bubbles: Vec<Bubble> = bubble_insts
        .iter()
        .enumerate()
        .map(|(i, b)| read_bubble(b, i, &value_specs, chart_tip))
        .collect::<Result<_, _>>()?;
    // The segmentation partition: x-bound bands' spans, in source order.
    let segments: Vec<(f64, f64)> = bands
        .iter()
        .filter(|b| matches!(b.axis, AxisRef::X))
        .map(|b| b.span)
        .collect();

    // The x scale: a band for categorical data (categories or indices), or a numeric
    // domain when the data is points / a formula / a bottom axis range / bands / bubbles.
    let x = build_x_axis(x_inst, &categories, &series, &segments, &bubbles, span)?;

    // Sample any deferred `fn:` over the now-fixed x-domain → concrete points
    // [SPEC 14.3]; after this, every series carries data feeding the value axes.
    for (si, s) in series_insts.iter().zip(series.iter_mut()) {
        if let Data::Formula(exprs) = &s.data {
            s.data = sample_formula(exprs, &x.scale, samples, funcs, si.span, &segments)?;
        }
    }

    // Re-bind categorical series length to the band, validating against categories.
    if let Some(cats) = &categories {
        for (si, s) in series_insts.iter().zip(&series) {
            if let Data::Categorical(v) = &s.data
                && v.len() != cats.len()
            {
                return Err(Error::at(
                    si.span,
                    format!(
                        "series data has {} values but the chart has {} categories",
                        v.len(),
                        cats.len()
                    ),
                ));
            }
        }
    }

    let values = build_value_axes(value_specs, &series, &bars, &bubbles)?;

    Ok(Chart {
        title,
        x,
        values,
        series,
        bands,
        marks,
        bubbles,
        bars,
        dir,
        gap: read_gap(&inst.attrs),
        tooltip: chart_tip,
    })
}

/// The chart's title / legend gutter [SPEC 14.6], from the resolved `gap:` (the
/// `.lini-chart` / `.lini-pie` class defaults it to 10, overriding the `|block|` 20).
pub(crate) fn read_gap(attrs: &AttrMap) -> f64 {
    attrs.number("gap").unwrap_or(10.0)
}

/// The chart's `direction` [SPEC 14.7] — its orientation / projection.
fn read_direction(attrs: &AttrMap) -> Result<Dir, Error> {
    match attrs.get("direction") {
        None => Ok(Dir::Column),
        Some(ResolvedValue::Ident(s)) => match s.as_str() {
            "column" => Ok(Dir::Column),
            "row" => Ok(Dir::Row),
            "radial" => Ok(Dir::Radial),
            _ => Err(Error::at(
                Span::empty(),
                "'direction' is column, row, or radial",
            )),
        },
        _ => Err(Error::at(
            Span::empty(),
            "'direction' is column, row, or radial",
        )),
    }
}

/// Split children into series, axes, bands, marks, and the harvested title; reject
/// non-chart children and the constructs that arrive in later steps [SPEC 20].
fn partition(inst: &ResolvedInst) -> Result<Split<'_>, Error> {
    let mut series = Vec::new();
    let mut axes = Vec::new();
    let mut bands = Vec::new();
    let mut marks = Vec::new();
    let mut bubbles = Vec::new();
    let mut title = None;
    for child in &inst.children {
        if child.kind == NodeKind::Text {
            if title.is_none() {
                title = child
                    .label
                    .as_deref()
                    .filter(|t| !t.is_empty())
                    .map(str::to_string);
            }
            continue;
        }
        match tag(child) {
            Some("bars") | Some("dots") | Some("line") | Some("area") => series.push(child),
            Some("axis") => axes.push(child),
            Some("band") => bands.push(child),
            Some("mark") => marks.push(child),
            Some("bubble") => bubbles.push(child),
            Some("slice") => {
                return Err(Error::at(
                    child.span,
                    "'|slice|' belongs in a 'layout: pie'",
                ));
            }
            Some(other) => {
                return Err(Error::at(
                    child.span,
                    format!("'|{other}|' arrives in a later charts step"),
                ));
            }
            None => {
                return Err(Error::at(
                    child.span,
                    "a chart's children are series (e.g. '|bars|', '|line|') and '|axis|'",
                ));
            }
        }
    }
    Ok((series, axes, bands, marks, bubbles, title))
}

/// The chart type tag a child carries — its `type_chain` entry, or `line` for the
/// reused `|line|` primitive.
pub(crate) fn tag(inst: &ResolvedInst) -> Option<&str> {
    const TAGS: &[&str] = &[
        "line", "area", "bars", "dots", "bubble", "slice", "axis", "band", "mark",
    ];
    if inst.kind == NodeKind::Line {
        return Some("line");
    }
    inst.type_chain
        .iter()
        .rev()
        .find_map(|t| TAGS.iter().copied().find(|&tag| tag == t))
}

/// The chart's `fn:` sample count [SPEC 14.1], default 24.
fn sample_count(attrs: &AttrMap) -> usize {
    attrs
        .number("samples")
        .filter(|n| *n >= 2.0)
        .map(|n| n as usize)
        .unwrap_or(24)
}

/// The chart's `bars:` mode [SPEC 14.2] — how multiple `|bars|` series combine.
fn read_bars(attrs: &AttrMap) -> Result<BarMode, Error> {
    match attrs.get("bars") {
        None => Ok(BarMode::Grouped),
        Some(ResolvedValue::Ident(s)) => match s.as_str() {
            "grouped" => Ok(BarMode::Grouped),
            "stacked" => Ok(BarMode::Stacked),
            "overlay" => Ok(BarMode::Overlay),
            _ => Err(Error::at(
                Span::empty(),
                "'bars' is grouped, stacked, or overlay",
            )),
        },
        _ => Err(Error::at(
            Span::empty(),
            "'bars' is grouped, stacked, or overlay",
        )),
    }
}

fn read_categories(attrs: &AttrMap, span: Span) -> Result<Option<Vec<String>>, Error> {
    let Some(v) = attrs.get("categories") else {
        return Ok(None);
    };
    let mut out = Vec::new();
    collect_strings(v, &mut out, span)?;
    Ok(Some(out))
}

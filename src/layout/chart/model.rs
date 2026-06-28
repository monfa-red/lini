//! Parse a chart's resolved children into a typed model: the x (domain) axis, the
//! value axes, and the series bound to them ([CHARTS.md] §3–§6). All chart-shape
//! validation (§18) lives here; the geometry is the renderers' job.

use super::palette;
use super::scale::{self, Scale};
use crate::error::Error;
use crate::resolve::{AttrMap, NodeKind, ResolvedInst, ResolvedValue};
use crate::span::Span;

pub enum Side {
    Bottom,
    Top,
    Left,
    Right,
}

/// A node's gridline setting ([CHARTS.md] §5): the default (drawn for the primary
/// value axis and a numeric x axis), off, or an explicit tint.
pub enum Grid {
    Default,
    Off,
    Color(ResolvedValue),
}

pub enum SeriesKind {
    Bars,
    Line,
    Dots,
}

pub enum Data {
    /// One value per category (a categorical series).
    Categorical(Vec<f64>),
    /// `x y` pairs (scatter / irregular).
    Points(Vec<(f64, f64)>),
}

pub enum Curve {
    Linear,
    Step,
}

pub struct Series {
    pub kind: SeriesKind,
    pub data: Data,
    pub label: Option<String>,
    pub color: ResolvedValue,
    /// Index into [`Chart::values`] — the value axis this series is read against.
    pub axis: usize,
    pub marker: bool,
    pub curve: Curve,
    pub stroke_style: Option<ResolvedValue>,
    /// A line's `stroke-width` (default 2).
    pub thickness: f64,
    /// A dot's diameter `width` × `height` (default a small circle).
    pub dot: (f64, f64),
}

pub struct ValueAxis {
    pub side: Side,
    pub scale: Scale,
    pub title: Option<String>,
    pub unit: Option<String>,
    pub grid: Grid,
    pub primary: bool,
}

pub struct XAxis {
    pub scale: Scale,
    pub labels: Vec<String>,
    pub title: Option<String>,
    pub unit: Option<String>,
    pub grid: Grid,
}

pub struct Chart {
    pub title: Option<String>,
    pub x: XAxis,
    pub values: Vec<ValueAxis>,
    pub series: Vec<Series>,
}

/// One end of a `range:` window: a fixed number, or `auto` (fit from data).
enum End {
    Num(f64),
    Auto,
}

/// The children of a chart, split by role: series, axes, and the harvested title.
type Split<'a> = (Vec<&'a ResolvedInst>, Vec<&'a ResolvedInst>, Option<String>);

/// Raw value-axis metadata, parsed before the data domains that build its scale.
struct AxisSpec<'a> {
    id: Option<&'a str>,
    side: Side,
    title: Option<String>,
    unit: Option<String>,
    grid: Grid,
    range: Option<(End, End)>,
    step: Option<f64>,
    ticks: Option<Vec<f64>>,
}

pub fn build(inst: &ResolvedInst) -> Result<Chart, Error> {
    let span = inst.span;
    let (series_insts, axis_insts, title) = partition(inst)?;
    if series_insts.is_empty() {
        return Err(Error::at(span, "a chart needs at least one series"));
    }

    let categories = read_categories(&inst.attrs, span)?;

    // Split declared axes into the one domain (x) axis and the value axes.
    let mut x_inst: Option<&ResolvedInst> = None;
    let mut value_specs: Vec<AxisSpec> = Vec::new();
    for ax in &axis_insts {
        match read_side(ax)? {
            Some(s @ (Side::Bottom | Side::Top)) => {
                x_inst = Some(ax);
                let _ = s;
            }
            side => value_specs.push(axis_spec(ax, side.unwrap_or(Side::Left))?),
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
            side: Side::Left,
            title: None,
            unit: None,
            grid: Grid::Default,
            range: None,
            step: None,
            ticks: None,
        });
    }

    // Read each series' data + style, binding it to a value axis by index.
    let mut series = Vec::with_capacity(series_insts.len());
    for (i, si) in series_insts.iter().enumerate() {
        series.push(read_series(si, i, &value_specs, &categories, span)?);
    }

    // The x scale: a band for categorical data (categories or indices), or a numeric
    // domain when the data is points / an explicit bottom axis sets a range.
    let x = build_x_axis(x_inst, &categories, &series, span)?;
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

    let values = build_value_axes(value_specs, &series)?;

    Ok(Chart {
        title,
        x,
        values,
        series,
    })
}

/// Split children into series, axes, and the harvested title; reject non-chart
/// children and the constructs that arrive in later steps ([CHARTS.md] §18).
fn partition(inst: &ResolvedInst) -> Result<Split<'_>, Error> {
    let mut series = Vec::new();
    let mut axes = Vec::new();
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
            Some("bars") | Some("dots") | Some("line") => series.push(child),
            Some("axis") => axes.push(child),
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
    Ok((series, axes, title))
}

/// The chart type tag a child carries — its `type_chain` entry, or `line` for the
/// reused `|line|` primitive.
fn tag(inst: &ResolvedInst) -> Option<&str> {
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

fn read_series(
    inst: &ResolvedInst,
    index: usize,
    value_specs: &[AxisSpec],
    categories: &Option<Vec<String>>,
    _chart_span: Span,
) -> Result<Series, Error> {
    let kind = match tag(inst) {
        Some("bars") => SeriesKind::Bars,
        Some("dots") => SeriesKind::Dots,
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
        (false, true) => {
            return Err(Error::at(
                inst.span,
                "a computed 'fn:' series needs an axis — added in a later charts step",
            ));
        }
        (true, false) => read_data(inst, &kind)?,
    };
    if categories.is_some() && matches!(data, Data::Points(_)) {
        return Err(Error::at(
            inst.span,
            "'x y' point data needs a numeric x axis, not 'categories'",
        ));
    }
    let axis = bind_axis(inst, value_specs)?;
    let color = series_color(&inst.attrs).unwrap_or_else(|| {
        // No explicit paint → walk the palette at the tier the role wants
        // ([CHARTS.md] §10): a line the deep stroke, dots the ink, a bar the base.
        let suffix = match kind {
            SeriesKind::Line => "-deep",
            SeriesKind::Dots => "-ink",
            SeriesKind::Bars => "",
        };
        live(&format!("{}{}", palette::hue(index), suffix))
    });
    let dot_w = inst.attrs.number("width").unwrap_or(7.0);
    let dot_h = inst.attrs.number("height").unwrap_or(dot_w);
    Ok(Series {
        kind,
        data,
        label: label_of(inst),
        color,
        axis,
        marker: marker_on(&inst.attrs),
        curve: read_curve(&inst.attrs)?,
        stroke_style: inst.attrs.get("stroke-style").cloned(),
        thickness: inst.attrs.number("stroke-width").unwrap_or(2.0),
        dot: (dot_w, dot_h),
    })
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

/// Bind a series to a value axis by its `axis:` id, defaulting to the first value
/// axis. An unknown id reports the chart's own axis ids ([CHARTS.md] §18).
fn bind_axis(inst: &ResolvedInst, specs: &[AxisSpec]) -> Result<usize, Error> {
    let Some(ResolvedValue::Ident(id)) = inst.attrs.get("axis") else {
        return Ok(0);
    };
    if let Some(pos) = specs.iter().position(|a| a.id == Some(id.as_str())) {
        return Ok(pos);
    }
    let known: Vec<String> = specs
        .iter()
        .filter_map(|a| a.id.map(|s| format!("'{s}'")))
        .collect();
    let hint = if known.is_empty() {
        String::new()
    } else {
        format!("; did you mean {}?", known.join(", "))
    };
    Err(Error::at(inst.span, format!("axis '{id}' not found{hint}")))
}

fn build_x_axis(
    x_inst: Option<&ResolvedInst>,
    categories: &Option<Vec<String>>,
    series: &[Series],
    span: Span,
) -> Result<XAxis, Error> {
    let (title, unit, grid) = match x_inst {
        Some(a) => (label_of(a), read_unit(a)?, read_grid(a)?),
        None => (None, None, Grid::Default),
    };
    // Categorical when categories are set or every series is categorical; numeric
    // when the data is points (a scatter) or a bottom axis fixes a range.
    let any_points = series.iter().any(|s| matches!(s.data, Data::Points(_)));
    if let Some(cats) = categories {
        return Ok(XAxis {
            scale: Scale::band(cats.len()),
            labels: cats.clone(),
            title,
            unit,
            grid,
        });
    }
    if !any_points {
        let n = series
            .iter()
            .map(|s| match &s.data {
                Data::Categorical(v) => v.len(),
                Data::Points(_) => 0,
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
    // Numeric x: domain from a bottom axis `range:`, else the union of point x's.
    let xs: Vec<f64> = series
        .iter()
        .flat_map(|s| match &s.data {
            Data::Points(p) => p.iter().map(|(x, _)| *x).collect::<Vec<_>>(),
            Data::Categorical(_) => Vec::new(),
        })
        .collect();
    let range = x_inst.map(read_range).transpose()?.flatten();
    let scale = numeric_scale(&xs, range, x_inst, false)?;
    Ok(XAxis {
        scale,
        labels: Vec::new(),
        title,
        unit,
        grid,
    })
}

fn build_value_axes(specs: Vec<AxisSpec>, series: &[Series]) -> Result<Vec<ValueAxis>, Error> {
    let mut out = Vec::with_capacity(specs.len());
    for (i, spec) in specs.iter().enumerate() {
        // Values + whether any bound series is bars (which forces zero into the
        // domain). `Categorical` contributes its values; `Points` their y.
        let mut vals: Vec<f64> = Vec::new();
        let mut has_bars = false;
        for s in series.iter().filter(|s| s.axis == i) {
            has_bars |= matches!(s.kind, SeriesKind::Bars);
            match &s.data {
                Data::Categorical(v) => vals.extend(v),
                Data::Points(p) => vals.extend(p.iter().map(|(_, y)| *y)),
            }
        }
        let scale = value_scale(&vals, has_bars, spec)?;
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
/// `range:` window / reverse and `step:` / `ticks:` ([CHARTS.md] §6).
fn value_scale(vals: &[f64], has_bars: bool, spec: &AxisSpec) -> Result<Scale, Error> {
    let data_min = vals.iter().copied().fold(f64::INFINITY, f64::min);
    let data_max = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (dmin, dmax) = if vals.is_empty() {
        (0.0, 1.0)
    } else {
        (data_min, data_max)
    };
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

/// A numeric x scale (a scatter's x, or a `range:`-fixed bottom axis).
fn numeric_scale(
    xs: &[f64],
    range: Option<(End, End)>,
    spec_src: Option<&ResolvedInst>,
    _value: bool,
) -> Result<Scale, Error> {
    let data_min = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let data_max = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (dmin, dmax) = if xs.is_empty() {
        (0.0, 1.0)
    } else {
        (data_min, data_max)
    };
    let (min, max, rev) = match range {
        Some((a, b)) => {
            let lo = end(&a, dmin);
            let hi = end(&b, dmax);
            (lo.min(hi), lo.max(hi), lo > hi)
        }
        None => (dmin.min(0.0).max(dmin), dmax, false),
    };
    let step = spec_src.and_then(|a| a.attrs.number("step"));
    let explicit_ticks = spec_src.and_then(|a| number_list(a.attrs.get("ticks")));
    let ticks = if let Some(t) = explicit_ticks {
        t
    } else if let Some(st) = step {
        scale::ticks_by_step(min, max, st)
    } else {
        scale::nice_ticks(min, max)
    };
    Ok(Scale::linear(min, max, rev, ticks))
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

fn axis_spec(inst: &ResolvedInst, side: Side) -> Result<AxisSpec<'_>, Error> {
    if let Some(ResolvedValue::Ident(s)) = inst.attrs.get("scale")
        && s == "log"
    {
        return Err(Error::at(
            inst.span,
            "'scale: log' arrives in a later charts step",
        ));
    }
    Ok(AxisSpec {
        id: inst.id.as_deref(),
        side,
        title: label_of(inst),
        unit: read_unit(inst)?,
        grid: read_grid(inst)?,
        range: read_range(inst)?,
        step: inst.attrs.number("step"),
        ticks: number_list(inst.attrs.get("ticks")),
    })
}

// ───────────────────────────── attribute readers ─────────────────────────────

fn read_side(inst: &ResolvedInst) -> Result<Option<Side>, Error> {
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

fn read_curve(attrs: &AttrMap) -> Result<Curve, Error> {
    match attrs.get("curve") {
        None => Ok(Curve::Linear),
        Some(ResolvedValue::Ident(s)) => match s.as_str() {
            "linear" => Ok(Curve::Linear),
            "step" => Ok(Curve::Step),
            "smooth" => Err(Error::at(
                Span::empty(),
                "'curve: smooth' arrives in a later charts step",
            )),
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

/// A line draws a vertex marker when `marker:` is set to anything but `none`.
fn marker_on(attrs: &AttrMap) -> bool {
    matches!(attrs.get("marker"), Some(ResolvedValue::Ident(s)) if s != "none")
}

fn read_categories(attrs: &AttrMap, span: Span) -> Result<Option<Vec<String>>, Error> {
    let Some(v) = attrs.get("categories") else {
        return Ok(None);
    };
    let mut out = Vec::new();
    collect_strings(v, &mut out, span)?;
    Ok(Some(out))
}

fn collect_strings(v: &ResolvedValue, out: &mut Vec<String>, span: Span) -> Result<(), Error> {
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

fn number_list(v: Option<&ResolvedValue>) -> Option<Vec<f64>> {
    match v? {
        ResolvedValue::Number(n) => Some(vec![*n]),
        ResolvedValue::Tuple(items) | ResolvedValue::List(items) => {
            items.iter().map(ResolvedValue::as_number).collect()
        }
        _ => None,
    }
}

fn numbers(items: &[ResolvedValue], span: Span) -> Result<Vec<f64>, Error> {
    items.iter().map(|it| number(it, span)).collect()
}

fn number(v: &ResolvedValue, span: Span) -> Result<f64, Error> {
    v.as_number()
        .ok_or_else(|| Error::at(span, "'data' values must be numbers"))
}

/// A series' legend label, harvested from the smart label ([CHARTS.md] §9): a
/// geometry series (`|line|`) keeps it on the node; a block series (`|bars|` /
/// `|dots|`) lowered it to a centred text child.
fn label_of(inst: &ResolvedInst) -> Option<String> {
    inst.label.clone().filter(|t| !t.is_empty()).or_else(|| {
        inst.children
            .iter()
            .find(|c| c.kind == NodeKind::Text)
            .and_then(|c| c.label.as_deref())
            .filter(|t| !t.is_empty())
            .map(str::to_string)
    })
}

/// A series' user-chosen colour ([CHARTS.md] §10): an explicit `fill` (areas / bars)
/// or `stroke` (lines / dots). The inherited primitive defaults — `none`, or the
/// bare `--stroke` / `--fill` role vars a `|line|`/`|block|` carries — are **not** a
/// choice, so they fall through to the palette walk.
fn series_color(attrs: &AttrMap) -> Option<ResolvedValue> {
    real_color(attrs.get("fill")).or_else(|| real_color(attrs.get("stroke")))
}

fn real_color(v: Option<&ResolvedValue>) -> Option<ResolvedValue> {
    match v {
        Some(ResolvedValue::Ident(s)) if s == "none" => None,
        Some(ResolvedValue::LiveVar { name, .. }) if name == "stroke" || name == "fill" => None,
        Some(other) => Some(other.clone()),
        None => None,
    }
}

fn live(name: &str) -> ResolvedValue {
    ResolvedValue::LiveVar {
        name: name.to_string(),
        raw: false,
    }
}

fn clone_grid(g: &Grid) -> Grid {
    match g {
        Grid::Default => Grid::Default,
        Grid::Off => Grid::Off,
        Grid::Color(c) => Grid::Color(c.clone()),
    }
}

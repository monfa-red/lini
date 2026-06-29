//! Parse a chart's resolved children into a typed model: the x (domain) axis, the
//! value axes, and the series bound to them ([CHARTS.md] §3–§6). All chart-shape
//! validation (§18) lives here; the geometry is the renderers' job.

use super::palette;
use super::project::Dir;
use super::scale::{self, Scale};
use super::tooltip::Tooltip;
use crate::error::Error;
use crate::expr::{self, Expr, FuncTable, Value as ExprValue};
use crate::resolve::{AttrMap, MarkerKind, NodeKind, ResolvedInst, ResolvedValue};
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
    Area,
}

pub enum Data {
    /// One value per category (a categorical series).
    Categorical(Vec<f64>),
    /// `x y` pairs (scatter / irregular).
    Points(Vec<(f64, f64)>),
    /// A `fn:` formula, held unevaluated until the x-domain is fixed, then sampled to
    /// `Points` ([CHARTS.md] §4). One expr is a whole-domain `fn:`.
    Formula(Vec<Expr>),
}

pub enum Curve {
    Linear,
    /// A monotone cubic — curved, through every point, never overshooting.
    Smooth,
    Step,
}

/// How multiple `|bars|` series combine ([CHARTS.md] §3): side-by-side, piled, or
/// translucently on top.
pub enum BarMode {
    Grouped,
    Stacked,
    Overlay,
}

/// The axis a band / annotation is measured against ([CHARTS.md] §8): the x (domain)
/// axis, or a value axis by index into [`Chart::values`].
pub enum AxisRef {
    X,
    Value(usize),
}

/// A `|mark|`'s placement ([CHARTS.md] §8): a reference line at one value, or a point
/// at `(x, value)`.
pub enum MarkAt {
    Line(f64),
    Point(f64, f64),
}

/// A `|band|` ([CHARTS.md] §7): a shaded zone over `span` on its bound axis, a tick
/// (its label), and — for an x-bound band — a boundary in the shared segmentation
/// partition. `fill: none` (or no fill) makes it a divider, not a shade.
pub struct Band {
    pub axis: AxisRef,
    pub span: (f64, f64),
    pub label: Option<String>,
    /// The shade tint; `None` draws dividers at the span edges instead.
    pub fill: Option<ResolvedValue>,
    /// The colour of the tick label (and, for an unfilled band, its dividers): the
    /// `fill` tint ([CHARTS.md] §9), or muted when there is none.
    pub tick: ResolvedValue,
}

/// A `|mark|` annotation ([CHARTS.md] §8): a reference line or a labelled point,
/// placed by value on a named axis (so it survives a `direction` flip).
pub struct Mark {
    pub axis: AxisRef,
    pub at: MarkAt,
    pub label: Option<String>,
    /// A point's centred marker ([CHARTS.md] §8): `dot` by default (the `|mark|`
    /// template), `circle` / `diamond` to enlarge it, `None` (from `marker: none`) for a
    /// label-only mark. Validated against `arrow` / `crow` at parse ([§18]).
    pub marker: MarkerKind,
    /// The accent for the line / dot / label: an explicit `stroke` / `fill`, else muted.
    pub color: ResolvedValue,
    pub stroke_style: Option<ResolvedValue>,
    /// How the mark's label presents ([CHARTS.md] §14) — the cascaded `tooltip:`. A mark
    /// is a deliberate annotation, so its label is forced (always placed) unless `none`.
    pub tooltip: Tooltip,
}

/// A `|bubble|` ([CHARTS.md] §3): one labelled mark at a data point `(x, y)`, sized by
/// `value` (area-scaled across the chart) — its own colour (explicit or palette walk).
pub struct Bubble {
    pub at: (f64, f64),
    pub value: f64,
    /// Index into [`Chart::values`] — the value axis its `y` is read against.
    pub axis: usize,
    pub label: Option<String>,
    pub color: ResolvedValue,
    /// An explicit `stroke:` outline ([CHARTS.md] §10): colour + `stroke-width`, drawn
    /// around the bubble. `None` → no outline.
    pub outline: Option<(ResolvedValue, f64)>,
    /// How the bubble's label presents ([CHARTS.md] §14) — the cascaded `tooltip:`.
    /// `auto` sits it inside when it fits, else beside, else on hover; `none` hover-only.
    pub tooltip: Tooltip,
}

pub struct Series {
    pub kind: SeriesKind,
    pub data: Data,
    pub label: Option<String>,
    pub color: ResolvedValue,
    /// Index into [`Chart::values`] — the value axis this series is read against.
    pub axis: usize,
    /// The centred marker at each vertex ([CHARTS.md] §3): `None` draws none; `dot` /
    /// `circle` / `diamond` are the centred shapes. A `|dots|` is never `None` (it *is*
    /// markers). Validated against `arrow` / `crow` at parse ([§18]).
    pub marker: MarkerKind,
    /// Per-datum label text ([CHARTS.md] §4), parallel to the data — one tag per value /
    /// point, or empty. Drawn inline / on hover per [`tooltip`](Self::tooltip).
    pub tags: Vec<String>,
    /// How this series' labels present ([CHARTS.md] §14) — the cascaded `tooltip:` (its
    /// own, else the chart's). Governs whether the `tags` draw inline.
    pub tooltip: Tooltip,
    /// The tint for this series' inline tag labels ([CHARTS.md] §14): an explicit
    /// `color:`, else the muted role.
    pub tag_color: ResolvedValue,
    pub curve: Curve,
    pub stroke_style: Option<ResolvedValue>,
    /// An explicit `stroke:` outline ([CHARTS.md] §10): its colour and `stroke-width`.
    /// An `|area|` draws it as its top edge (defaulting to a deep tier of the fill when
    /// absent); `|bars|` draw it as the rect / wedge outline. `None` → no outline. The
    /// fill is read separately (from `fill:`), so a stroke never bleeds into the body.
    pub outline: Option<(ResolvedValue, f64)>,
    /// A line's `stroke-width` (default 2).
    pub thickness: f64,
    /// A `|bars|` corner radius ([CHARTS.md] §3), from the resolved `radius:` (default 2
    /// via the `.lini-bars` class). Rounds a rectangular bar; a radial wedge ignores it.
    pub radius: f64,
    /// A dot's diameter `width` × `height` (default a small circle).
    pub dot: (f64, f64),
    /// An `|area|`'s fill target ([CHARTS.md] §16) — the axis zero / range floor by
    /// default.
    pub baseline: Option<f64>,
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
    pub bands: Vec<Band>,
    pub marks: Vec<Mark>,
    pub bubbles: Vec<Bubble>,
    pub bars: BarMode,
    pub dir: Dir,
    /// The clear space between the plot and the title / legend outside it ([CHARTS.md]
    /// §9), from the resolved `gap:` (default 10 via the `.lini-chart` class).
    pub gap: f64,
    /// The chart-level label presentation ([CHARTS.md] §14), default `auto` — the hover
    /// card driver and each series' `tooltip:` fallback.
    pub tooltip: Tooltip,
}

/// One wedge of a `layout: pie` ([CHARTS.md] §13): its magnitude, legend label, and
/// colour (an explicit `fill` / `stroke`, else the per-slice palette walk).
pub struct Slice {
    pub value: f64,
    pub label: Option<String>,
    pub color: ResolvedValue,
    /// An explicit `stroke:` outline ([CHARTS.md] §10): colour + `stroke-width`, drawn
    /// around the wedge. `None` → no outline.
    pub outline: Option<(ResolvedValue, f64)>,
}

/// A parsed pie ([CHARTS.md] §13): its slices (source order, clockwise from the top),
/// title, and `hole` fraction (`0` a pie, `0 < n < 1` a donut).
pub struct Pie {
    pub slices: Vec<Slice>,
    pub title: Option<String>,
    pub hole: f64,
    /// The clear space between the pie and its title / legend ([CHARTS.md] §9), from the
    /// resolved `gap:` (default 10 via the `.lini-pie` class).
    pub gap: f64,
}

/// One end of a `range:` window: a fixed number, or `auto` (fit from data).
enum End {
    Num(f64),
    Auto,
}

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
    log: bool,
}

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
    // screen edge plays the domain in this direction ([CHARTS.md] §11): the bottom/top
    // in a column, the left/right in a row. A radial chart has no sides — one radius
    // (value) axis, the domain being the spokes ([§12]).
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
    // ([CHARTS.md] §4); after this, every series carries data feeding the value axes.
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

/// The chart's title / legend gutter ([CHARTS.md] §9), from the resolved `gap:` (the
/// `.lini-chart` / `.lini-pie` class defaults it to 10, overriding the `|block|` 20).
fn read_gap(attrs: &AttrMap) -> f64 {
    attrs.number("gap").unwrap_or(10.0)
}

/// Parse a `|bubble|` ([CHARTS.md] §3): a labelled point `at: x y`, its `value` (area
/// size), the bound value axis, and its colour (explicit `fill`/`stroke`, else palette).
fn read_bubble(
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

/// Parse a `layout: pie` into its slices ([CHARTS.md] §13). All pie validation (§18)
/// lives here; the wedge geometry is the renderer's job. Reuses the chart's `tag`,
/// `label_of`, the `fill:` / `outline:` paint readers, and the palette walk (per
/// slice — [§10]).
pub fn build_pie(inst: &ResolvedInst) -> Result<Pie, Error> {
    let span = inst.span;
    let hole = read_hole(&inst.attrs)?;
    let mut title = None;
    let mut slice_insts = Vec::new();
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
            Some("slice") => slice_insts.push(child),
            _ => return Err(Error::at(child.span, "a pie's children are '|slice|' only")),
        }
    }
    if slice_insts.is_empty() {
        return Err(Error::at(span, "a pie needs at least one '|slice|'"));
    }
    let mut slices = Vec::with_capacity(slice_insts.len());
    for (i, s) in slice_insts.iter().enumerate() {
        let value = s
            .attrs
            .number("value")
            .ok_or_else(|| Error::at(s.span, "a '|slice|' needs a 'value:'"))?;
        if value < 0.0 {
            return Err(Error::at(s.span, "a '|slice|' value must be ≥ 0"));
        }
        let color =
            fill_color(&s.attrs).unwrap_or_else(|| live(&format!("{}-soft", palette::hue(i))));
        let edge = fill_outline(&s.attrs, &color);
        slices.push(Slice {
            value,
            label: label_of(s),
            color,
            outline: edge,
        });
    }
    if slices.iter().map(|s| s.value).sum::<f64>() <= 0.0 {
        return Err(Error::at(span, "a pie's slice values sum to zero"));
    }
    Ok(Pie {
        slices,
        title,
        hole,
        gap: read_gap(&inst.attrs),
    })
}

/// A pie's `hole:` fraction ([CHARTS.md] §13) — `0` a pie, `0 < n < 1` a donut.
fn read_hole(attrs: &AttrMap) -> Result<f64, Error> {
    match attrs.get("hole") {
        None => Ok(0.0),
        Some(ResolvedValue::Number(n)) if *n >= 0.0 && *n < 1.0 => Ok(*n),
        _ => Err(Error::at(Span::empty(), "'hole' is a fraction 0..1")),
    }
}

/// The chart's `direction` ([CHARTS.md] §11) — its orientation / projection.
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
/// non-chart children and the constructs that arrive in later steps ([CHARTS.md] §18).
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
    // Paint by role ([CHARTS.md] §10): a fill shape (bars / area) takes its body from
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
        // The outlined look ([CHARTS.md] §10): a bar / area fills with the **soft** tier
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
    // `|bars|` default to a deep edge of their soft fill (the outlined look, [§10]); an
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

/// The chart's `fn:` sample count ([CHARTS.md] §2/§4), default 24.
fn sample_count(attrs: &AttrMap) -> usize {
    attrs
        .number("samples")
        .filter(|n| *n >= 2.0)
        .map(|n| n as usize)
        .unwrap_or(24)
}

/// Sample a `fn:` over the x-domain → points ([CHARTS.md] §4). A single expr is the
/// whole-domain form: bind `x` at `samples` steps over the numeric domain. A per-band
/// list samples each expr in band-local `u` (0→1) across its segment's x-span, the
/// segments connecting end-to-start ([CHARTS.md] §7) — one continuous polyline whose
/// boundary risers are drawn. A list length ≠ the band count is an error ([§18]).
fn sample_formula(
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
/// error (a series `fn:` must return a number, [CHARTS.md] §4).
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

/// Parse a series' `tags:` ([CHARTS.md] §4): a quoted-string list, one per datum,
/// validated against the data count. A `fn:` series has no authored points to label, so
/// `tags:` on one is an error ([§18]). Reuses [`collect_strings`] (the `categories:`
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

/// Bind a series to a value axis by its `axis:` id, defaulting to the first value
/// axis. An unknown id reports the chart's own axis ids ([CHARTS.md] §18).
fn bind_axis(inst: &ResolvedInst, specs: &[AxisSpec]) -> Result<usize, Error> {
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
fn axis_id(inst: &ResolvedInst) -> Option<&str> {
    match inst.attrs.get("axis") {
        Some(ResolvedValue::Ident(s)) => Some(s.as_str()),
        _ => None,
    }
}

/// The "axis 'X' not found; did you mean 'Y'?" error ([CHARTS.md] §18), shared by
/// series, band, and mark binding. Axes are chart-local (not in the global index),
/// so the suggestion ranges over the chart's own `|axis|` ids.
fn no_axis(id: &str, known: &[&str], span: Span) -> Error {
    let listed: Vec<String> = known.iter().map(|s| format!("'{s}'")).collect();
    let hint = if listed.is_empty() {
        String::new()
    } else {
        format!("; did you mean {}?", listed.join(", "))
    };
    Error::at(span, format!("axis '{id}' not found{hint}"))
}

/// Resolve a band / mark `axis:` id to the x axis or a value axis ([CHARTS.md] §8).
fn lookup_axis(
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

/// Parse a `|band|` ([CHARTS.md] §7): its bound axis (default the x/domain axis), its
/// `span`, label, and fill (a real fill shades; `none` / unset makes it a divider).
fn read_band(inst: &ResolvedInst, x_id: Option<&str>, specs: &[AxisSpec]) -> Result<Band, Error> {
    let axis = match axis_id(inst) {
        Some(id) => lookup_axis(id, x_id, specs, inst.span)?,
        None => AxisRef::X,
    };
    let fill = real_color(inst.attrs.get("fill"));
    let tick = fill.clone().unwrap_or_else(muted);
    Ok(Band {
        axis,
        span: read_span(inst)?,
        label: label_of(inst),
        fill,
        tick,
    })
}

/// Parse a `|mark|` ([CHARTS.md] §8): a required bound axis, its `at` placement, the
/// label, whether a point shows its dot, and the accent (`stroke` / `fill`, else muted).
fn read_mark(
    inst: &ResolvedInst,
    x_id: Option<&str>,
    specs: &[AxisSpec],
    chart_tip: Tooltip,
) -> Result<Mark, Error> {
    let axis = match axis_id(inst) {
        Some(id) => lookup_axis(id, x_id, specs, inst.span)?,
        None => return Err(Error::at(inst.span, "a '|mark|' needs 'axis:' to place it")),
    };
    let color = real_color(inst.attrs.get("stroke"))
        .or_else(|| real_color(inst.attrs.get("fill")))
        .unwrap_or_else(muted);
    Ok(Mark {
        axis,
        at: read_at(inst)?,
        label: label_of(inst),
        marker: chart_marker(inst)?,
        color,
        stroke_style: inst.attrs.get("stroke-style").cloned(),
        tooltip: super::tooltip::read_or(&inst.attrs, chart_tip)?,
    })
}

/// A `|band|`'s `span: a b` — its data range on the bound axis ([CHARTS.md] §7).
fn read_span(inst: &ResolvedInst) -> Result<(f64, f64), Error> {
    match inst.attrs.get("span") {
        Some(ResolvedValue::Tuple(items)) if items.len() == 2 => {
            Ok((number(&items[0], inst.span)?, number(&items[1], inst.span)?))
        }
        _ => Err(Error::at(inst.span, "a '|band|' needs 'span: a b'")),
    }
}

/// A `|mark|`'s `at:` — one value (a reference line) or two (a point) ([CHARTS.md] §8).
fn read_at(inst: &ResolvedInst) -> Result<MarkAt, Error> {
    match inst.attrs.get("at") {
        Some(ResolvedValue::Number(v)) => Ok(MarkAt::Line(*v)),
        Some(ResolvedValue::Tuple(items)) if items.len() == 2 => Ok(MarkAt::Point(
            number(&items[0], inst.span)?,
            number(&items[1], inst.span)?,
        )),
        _ => Err(Error::at(
            inst.span,
            "'at' takes one value (a line) or two (a point)",
        )),
    }
}

/// The chart's `bars:` mode ([CHARTS.md] §3) — how multiple `|bars|` series combine.
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

fn build_x_axis(
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
    // point data, x-bound bands define the domain (the segmentation case, [§7]).
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

fn build_value_axes(
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
        // the per-category sum (the top of the pile, [CHARTS.md] §3).
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
/// `range:` window / reverse and `step:` / `ticks:` ([CHARTS.md] §6).
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

/// A log scale over a positive domain ([CHARTS.md] §6): the data domain is rounded
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
    Ok(AxisSpec {
        id: inst.id.as_deref(),
        side,
        title: label_of(inst),
        unit: read_unit(inst)?,
        grid: read_grid(inst)?,
        range: read_range(inst)?,
        step: inst.attrs.number("step"),
        ticks: number_list(inst.attrs.get("ticks")),
        log: read_log(inst)?,
    })
}

/// Whether an axis is `scale: log` ([CHARTS.md] §6); `scale:` accepts only
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

/// The effective **centred** marker for a chart node ([CHARTS.md] §3/§8): a line's
/// vertex, a `|dots|`, or a `|mark|` point. The `marker:` shorthand is extracted to the
/// resolved [`Markers`] (and dropped from the attr map), so this reads the resolved kind
/// (`start`, else `end` — `marker:` sets both; the directional ends have no chart
/// meaning). `marker: none` resolves to `None`; a `|mark|`'s template default `marker:
/// dot` separates an explicit `none` (label only) from a plain point (a dot). A chart
/// marker is centred, so the directional `arrow` / `crow` are rejected here ([§18]).
fn chart_marker(inst: &ResolvedInst) -> Result<MarkerKind, Error> {
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

/// The explicit **fill** of a fill shape ([CHARTS.md] §10) — `fill:` only, never the
/// stroke (a stroke is a separate outline, read by [`outline`]). The inherited primitive
/// defaults — `none`, or the bare `--fill` role var a `|block|` carries — are **not** a
/// choice, so they fall through to the palette walk.
fn fill_color(attrs: &AttrMap) -> Option<ResolvedValue> {
    real_color(attrs.get("fill"))
}

/// A fill shape's explicit `stroke:` outline ([CHARTS.md] §10): its colour paired with
/// `stroke-width` (default 1.5), or `None` for no outline. Used by `|area|` and `|bubble|`
/// (explicit-only); `|bars|` / `|slice|` use [`fill_outline`] for the default deep edge.
fn outline(attrs: &AttrMap) -> Option<(ResolvedValue, f64)> {
    real_color(attrs.get("stroke")).map(|c| (c, attrs.number("stroke-width").unwrap_or(1.5)))
}

/// A fill *series'* outline — the outlined look ([CHARTS.md] §10). An explicit `stroke:`
/// colour wins; the class default `stroke: auto` (or a bare role var / unset) draws a
/// **deep** edge of the `fill`; `stroke: none` removes it. The `auto` sentinel on the
/// `.lini-bars` / `.lini-slice` class is what separates an unset stroke (→ a default edge)
/// from an explicit `none` (→ no edge). Shared by `|bars|` (here) and `|slice|`
/// ([`build_pie`]), so the default edge derives in one place.
fn fill_outline(attrs: &AttrMap, fill: &ResolvedValue) -> Option<(ResolvedValue, f64)> {
    let width = attrs.number("stroke-width").unwrap_or(1.5);
    match attrs.get("stroke") {
        Some(ResolvedValue::Ident(s)) if s == "none" => None,
        Some(ResolvedValue::Ident(s)) if s == "auto" => Some((palette::deepen(fill), width)),
        other => match real_color(other) {
            Some(c) => Some((c, width)),
            None => Some((palette::deepen(fill), width)),
        },
    }
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

/// The muted role tint — a band tick / mark accent's default when unpainted.
fn muted() -> ResolvedValue {
    live("muted")
}

fn clone_grid(g: &Grid) -> Grid {
    match g {
        Grid::Default => Grid::Default,
        Grid::Off => Grid::Off,
        Grid::Color(c) => Grid::Color(c.clone()),
    }
}

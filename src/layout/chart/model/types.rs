//! The typed chart model — every enum and struct `build` fills in.

use super::*;

pub enum Side {
    Bottom,
    Top,
    Left,
    Right,
}

/// A node's gridline setting [SPEC 14.4]: the default (drawn for the primary
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
    /// `Points` [SPEC 14.3]. One expr is a whole-domain `fn:`.
    Formula(Vec<Expr>),
}

pub enum Curve {
    Linear,
    /// A monotone cubic — curved, through every point, never overshooting.
    Smooth,
    Step,
}

/// How multiple `|bars|` series combine [SPEC 14.2]: side-by-side, piled, or
/// translucently on top.
pub enum BarMode {
    Grouped,
    Stacked,
    Overlay,
}

/// The axis a band / annotation is measured against [SPEC 14.5]: the x (domain)
/// axis, or a value axis by index into [`Chart::values`].
pub enum AxisRef {
    X,
    Value(usize),
}

/// A `|mark|`'s placement [SPEC 14.5]: a reference line at one value, or a point
/// at `(x, value)`.
pub enum MarkAt {
    Line(f64),
    Point(f64, f64),
}

/// A `|band|` [SPEC 14.5]: a shaded zone over `span` on its bound axis, a tick
/// (its label), and — for an x-bound band — a boundary in the shared segmentation
/// partition. `fill: none` (or no fill) makes it a divider, not a shade.
pub struct Band {
    pub axis: AxisRef,
    pub span: (f64, f64),
    pub label: Option<String>,
    /// The shade tint; `None` draws dividers at the span edges instead.
    pub fill: Option<ResolvedValue>,
    /// The colour of the tick label (and, for an unfilled band, its dividers): the
    /// `fill` tint [SPEC 14.6], or muted when there is none.
    pub tick: ResolvedValue,
}

/// A `|mark|` annotation [SPEC 14.5]: a reference line or a labelled point,
/// placed by value on a named axis (so it survives a `direction` flip).
pub struct Mark {
    pub axis: AxisRef,
    pub at: MarkAt,
    pub label: Option<String>,
    /// A point's centred marker [SPEC 14.5]: `dot` by default (the `|mark|`
    /// template), `circle` / `diamond` to enlarge it, `None` (from `marker: none`) for a
    /// label-only mark. Validated against `arrow` / `crow` at parse ([SPEC 20]).
    pub marker: MarkerKind,
    /// The accent for the line / dot / label: an explicit `stroke` / `fill`, else muted.
    pub color: ResolvedValue,
    pub stroke_style: Option<ResolvedValue>,
    /// How the mark's label presents [SPEC 14.8] — the cascaded `tooltip:`. A mark
    /// is a deliberate annotation, so its label is forced (always placed) unless `none`.
    pub tooltip: Tooltip,
}

/// A `|bubble|` [SPEC 14.2]: one labelled mark at a data point `(x, y)`, sized by
/// `value` (area-scaled across the chart) — its own colour (explicit or palette walk).
pub struct Bubble {
    pub at: (f64, f64),
    pub value: f64,
    /// Index into [`Chart::values`] — the value axis its `y` is read against.
    pub axis: usize,
    pub label: Option<String>,
    pub color: ResolvedValue,
    /// An explicit `stroke:` outline [SPEC 14.6]: colour + `stroke-width`, drawn
    /// around the bubble. `None` → no outline.
    pub outline: Option<(ResolvedValue, f64)>,
    /// How the bubble's label presents [SPEC 14.8] — the cascaded `tooltip:`.
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
    /// The centred marker at each vertex [SPEC 14.2]: `None` draws none; `dot` /
    /// `circle` / `diamond` are the centred shapes. A `|dots|` is never `None` (it *is*
    /// markers). Validated against `arrow` / `crow` at parse ([SPEC 20]).
    pub marker: MarkerKind,
    /// Per-datum label text [SPEC 14.3], parallel to the data — one tag per value /
    /// point, or empty. Drawn inline / on hover per [`tooltip`](Self::tooltip).
    pub tags: Vec<String>,
    /// How this series' labels present [SPEC 14.8] — the cascaded `tooltip:` (its
    /// own, else the chart's). Governs whether the `tags` draw inline.
    pub tooltip: Tooltip,
    /// The tint for this series' inline tag labels [SPEC 14.8]: an explicit
    /// `color:`, else the muted role.
    pub tag_color: ResolvedValue,
    pub curve: Curve,
    pub stroke_style: Option<ResolvedValue>,
    /// An explicit `stroke:` outline [SPEC 14.6]: its colour and `stroke-width`.
    /// An `|area|` draws it as its top edge (defaulting to a deep tier of the fill when
    /// absent); `|bars|` draw it as the rect / wedge outline. `None` → no outline. The
    /// fill is read separately (from `fill:`), so a stroke never bleeds into the body.
    pub outline: Option<(ResolvedValue, f64)>,
    /// A line's `stroke-width` (default 2).
    pub thickness: f64,
    /// A `|bars|` corner radius [SPEC 14.2], from the resolved `radius:` (default 2
    /// via the `.lini-bars` class). Rounds a rectangular bar; a radial wedge ignores it.
    pub radius: f64,
    /// A dot's diameter `width` × `height` (default a small circle).
    pub dot: (f64, f64),
    /// An `|area|`'s fill target [SPEC 16] — the axis zero / range floor by
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
    /// The clear space between the plot and the title / legend outside it
    /// [SPEC 14.6], from the resolved `gap:` (default 10 via the `.lini-chart` class).
    pub gap: f64,
    /// The chart-level label presentation [SPEC 14.8], default `auto` — the hover
    /// card driver and each series' `tooltip:` fallback.
    pub tooltip: Tooltip,
}

/// One wedge of a `layout: pie` [SPEC 14.7]: its magnitude, legend label, and
/// colour (an explicit `fill` / `stroke`, else the per-slice palette walk).
pub struct Slice {
    pub value: f64,
    pub label: Option<String>,
    pub color: ResolvedValue,
    /// An explicit `stroke:` outline [SPEC 14.6]: colour + `stroke-width`, drawn
    /// around the wedge. `None` → no outline.
    pub outline: Option<(ResolvedValue, f64)>,
}

/// A parsed pie [SPEC 14.7]: its slices (source order, clockwise from the top),
/// title, and `hole` fraction (`0` a pie, `0 < n < 1` a donut).
pub struct Pie {
    pub slices: Vec<Slice>,
    pub title: Option<String>,
    pub hole: f64,
    /// The clear space between the pie and its title / legend [SPEC 14.6], from the
    /// resolved `gap:` (default 10 via the `.lini-pie` class).
    pub gap: f64,
}

/// One end of a `range:` window: a fixed number, or `auto` (fit from data).
pub(super) enum End {
    Num(f64),
    Auto,
}

/// Raw value-axis metadata, parsed before the data domains that build its scale.
pub(super) struct AxisSpec<'a> {
    pub(super) id: Option<&'a str>,
    pub(super) side: Side,
    pub(super) title: Option<String>,
    pub(super) unit: Option<String>,
    pub(super) grid: Grid,
    pub(super) range: Option<(End, End)>,
    pub(super) step: Option<f64>,
    pub(super) ticks: Option<Vec<f64>>,
    pub(super) log: bool,
}

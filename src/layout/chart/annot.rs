//! Bands and annotations [SPEC 14.5] — children that paint the shared plane
//! in *data* coordinates. A `|band|` shades (or divides) a span on its axis and labels
//! it; a `|mark|` draws a reference line or a labelled point. Both place by value on a
//! named axis through one projector (`axis_px`), so they survive a `direction` flip and
//! lower to the same `prim::*` primitives the renderer already draws.

use super::labels;
use super::marks::marker_diameter;
use super::metrics::LABEL_SIZE;
use super::model::{AxisRef, Chart, Mark, MarkAt};
use super::project::Plot;
use super::tooltip::Tooltip;
use crate::layout::PlacedNode;
use crate::layout::prim;
use crate::resolve::{MarkerKind, ResolvedValue};
/// A band wash's opacity, so the data reads clearly over it.
const SHADE: f64 = 0.15;
/// A reference line's stroke width (heavier than a 1px gridline, to stand out).
const LINE_W: f64 = 1.5;

/// The pixel coordinate of value `v` measured on `axis` — an x-pixel on the domain
/// axis, a y-pixel on a value axis — clamped into the drawn domain (crop, [SPEC 14.4]).
fn axis_px(plot: &Plot, chart: &Chart, axis: &AxisRef, v: f64) -> f64 {
    match axis {
        AxisRef::X => plot.x_at(&chart.x.scale, chart.x.scale.clamp(v)),
        AxisRef::Value(i) => {
            let s = &chart.values[*i].scale;
            plot.y_at(s, s.clamp(v))
        }
    }
}

/// Whether `v` lies within `axis`'s drawn domain (an off-plot annotation is cropped).
fn contains(chart: &Chart, axis: &AxisRef, v: f64) -> bool {
    match axis {
        AxisRef::X => chart.x.scale.contains(v),
        AxisRef::Value(i) => chart.values[*i].scale.contains(v),
    }
}

/// Band shades and dividers, drawn first so the data sits over them [SPEC 14.5].
pub fn band_shades(plot: &Plot, chart: &Chart, out: &mut Vec<PlacedNode>) {
    let mut drawn: Vec<f64> = Vec::new(); // divider positions, to dedup a shared edge
    for b in &chart.bands {
        let p0 = axis_px(plot, chart, &b.axis, b.span.0);
        let p1 = axis_px(plot, chart, &b.axis, b.span.1);
        if (p1 - p0).abs() < 0.5 {
            continue; // clamped to a sliver — the band lies outside the domain
        }
        match &b.fill {
            Some(fill) => out.push(shade(plot, &b.axis, p0, p1, fill.clone())),
            None => dividers(plot, &b.axis, &b.tick, p0, p1, &mut drawn, out),
        }
    }
}

/// A filled band: a faint wash over the span × the full perpendicular plot extent.
fn shade(plot: &Plot, axis: &AxisRef, p0: f64, p1: f64, fill: ResolvedValue) -> PlacedNode {
    match axis {
        AxisRef::X => prim::rect(
            (p0 + p1) / 2.0,
            (plot.y0 + plot.y1) / 2.0,
            (p1 - p0).abs(),
            plot.h(),
            fill,
            SHADE,
        ),
        AxisRef::Value(_) => prim::rect(
            (plot.x0 + plot.x1) / 2.0,
            (p0 + p1) / 2.0,
            plot.w(),
            (p1 - p0).abs(),
            fill,
            SHADE,
        ),
    }
}

/// An unfilled band's dividers: a line at each interior span edge, deduped (a shared
/// boundary in a contiguous partition draws once) and skipping the plot border.
fn dividers(
    plot: &Plot,
    axis: &AxisRef,
    color: &ResolvedValue,
    p0: f64,
    p1: f64,
    drawn: &mut Vec<f64>,
    out: &mut Vec<PlacedNode>,
) {
    for p in [p0, p1] {
        let edge = match axis {
            AxisRef::X => near(p, plot.x0) || near(p, plot.x1),
            AxisRef::Value(_) => near(p, plot.y0) || near(p, plot.y1),
        };
        if edge || drawn.iter().any(|&q| near(q, p)) {
            continue;
        }
        drawn.push(p);
        let pts = match axis {
            AxisRef::X => vec![(p, plot.y0), (p, plot.y1)],
            AxisRef::Value(_) => vec![(plot.x0, p), (plot.x1, p)],
        };
        out.push(prim::line(pts, color.clone(), 1.0));
    }
}

/// Band tick labels along each band's axis, tinted its fill [SPEC 14.6]. An x-band
/// tick sits a row below the x labels so band names and numeric ticks don't collide.
pub fn band_ticks(plot: &Plot, chart: &Chart, out: &mut Vec<PlacedNode>) {
    for b in &chart.bands {
        let Some(label) = &b.label else { continue };
        let mid = axis_px(plot, chart, &b.axis, (b.span.0 + b.span.1) / 2.0);
        let color = Some(b.tick.clone());
        let node = match &b.axis {
            AxisRef::X => prim::text(
                label,
                mid,
                plot.y1 + 4.0 + LABEL_SIZE * 1.7,
                LABEL_SIZE,
                color,
                false,
            ),
            AxisRef::Value(_) => prim::text_right(label, plot.x0 - 6.0, mid, LABEL_SIZE, color),
        };
        out.push(node);
    }
}

/// The extra bottom gutter an x-bound band's tick row needs (its labels sit a row below
/// the x labels), or 0 when no x band is labelled. Shared by the plot-rect inset and the
/// x-axis-title placement so the three rows stack without overlap.
pub fn x_band_row(chart: &Chart) -> f64 {
    let labelled = chart
        .bands
        .iter()
        .any(|b| matches!(b.axis, AxisRef::X) && b.label.is_some());
    if labelled { LABEL_SIZE } else { 0.0 }
}

/// `|mark|` annotations [SPEC 14.5]: a reference line at a value, or a labelled
/// point. Drawn after the series, before the axes / labels ([SPEC 14.9]).
pub fn marks(plot: &Plot, chart: &Chart, out: &mut Vec<PlacedNode>, reqs: &mut Vec<labels::Req>) {
    for m in &chart.marks {
        match m.at {
            MarkAt::Line(v) => ref_line(plot, chart, m, v, out),
            MarkAt::Point(x, y) => point(plot, chart, m, x, y, out, reqs),
        }
    }
}

/// A reference line at value `v`, across the plot perpendicular to the bound axis.
fn ref_line(plot: &Plot, chart: &Chart, m: &Mark, v: f64, out: &mut Vec<PlacedNode>) {
    if !contains(chart, &m.axis, v) {
        return; // off-plot — cropped
    }
    let p = axis_px(plot, chart, &m.axis, v);
    let pts = match &m.axis {
        AxisRef::X => vec![(p, plot.y0), (p, plot.y1)],
        AxisRef::Value(_) => vec![(plot.x0, p), (plot.x1, p)],
    };
    let mut ln = prim::line(pts, m.color.clone(), LINE_W);
    if let Some(ss) = &m.stroke_style {
        ln.attrs.insert("stroke-style", ss.clone());
    }
    out.push(ln);
    if let Some(text) = &m.label {
        let color = Some(m.color.clone());
        let node = match &m.axis {
            // A vertical line: the label centred just inside the top.
            AxisRef::X => prim::text(
                text,
                p,
                plot.y0 + LABEL_SIZE * 0.9,
                LABEL_SIZE,
                color,
                false,
            ),
            // A horizontal line: the label at the left end, just above the line (clear
            // of the data, which usually grows to the right).
            AxisRef::Value(_) => {
                prim::text_left(text, plot.x0 + 3.0, p - LABEL_SIZE * 0.6, LABEL_SIZE, color)
            }
        };
        out.push(node);
    }
}

/// A labelled point at `(x, y)` — `x` on the domain axis, `y` on the bound value axis
/// (the primary axis if the mark binds the x axis). `marker: none` drops the dot. The
/// label joins the shared placement pass [SPEC 14.8] — forced (a mark is a
/// deliberate annotation, so its label is always placed, never dropped) yet registered, so
/// a series' labels fan out around it. `tooltip: none` suppresses it.
fn point(
    plot: &Plot,
    chart: &Chart,
    m: &Mark,
    x: f64,
    y: f64,
    out: &mut Vec<PlacedNode>,
    reqs: &mut Vec<labels::Req>,
) {
    let vi = match &m.axis {
        AxisRef::Value(i) => *i,
        AxisRef::X => 0,
    };
    if !chart.x.scale.contains(x) || !chart.values[vi].scale.contains(y) {
        return;
    }
    let xp = plot.x_at(&chart.x.scale, x);
    let yp = plot.y_at(&chart.values[vi].scale, y);
    let radius = if m.marker != MarkerKind::None {
        let d = marker_diameter(m.marker, 2.0);
        out.push(prim::marker(m.marker, xp, yp, d, d, m.color.clone()));
        d / 2.0
    } else {
        0.0
    };
    if let Some(text) = &m.label
        && m.tooltip != Tooltip::None
    {
        reqs.push(labels::Req {
            anchor: (xp, yp),
            radius,
            text: text.clone(),
            color: m.color.clone(),
            forced: true,
            inside: None,
        });
    }
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.5
}

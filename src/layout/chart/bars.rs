//! Bar geometry [SPEC 14.2]: one mark per datum, combined by the chart's
//! `bars:` mode. [`visit_bars`] is the one place the modes branch — grouped splits the
//! category slot, stacked piles on the running total, overlay draws translucently on
//! top — and each direction consumes it: a `column` grows the value up (a rect), a
//! `row` grows it right (a rect), a `radial` chart grows it outward (a wedge). Every bar
//! carries its `<title>` [SPEC 14.8].

use super::model::{BarMode, Chart, Data, Series, SeriesKind};
use super::project::{Dir, Plot};
use super::scale::Scale;
use crate::layout::PlacedNode;
use crate::layout::prim;
use crate::ledger::format;
use std::f64::consts::TAU;

/// The bar group's share of a category slot (~14% padding each side).
const GROUP: f64 = 0.72;
/// An `overlay` bar's translucency, so a bar behind it reads through.
const OVERLAY: f64 = 0.6;

/// Emit the bars for every bar series, in the chart's direction.
pub fn lay_out(plot: &Plot, chart: &Chart, out: &mut Vec<PlacedNode>) {
    let bars: Vec<&Series> = chart
        .series
        .iter()
        .filter(|s| matches!(s.kind, SeriesKind::Bars))
        .collect();
    if bars.is_empty() {
        return;
    }
    let Scale::Band { n } = chart.x.scale else {
        return; // bars are categorical; a numeric x carries no slots
    };
    match plot.dir {
        Dir::Column => column_bars(plot, chart, &bars, n, out),
        Dir::Row => row_bars(plot, chart, &bars, n, out),
        Dir::Radial => radial_bars(plot, chart, &bars, n, out),
    }
}

/// Visit each bar of category `i` per the `bars:` mode, calling `emit` with the bar's
/// sub-slot (`k` of `s`, for splitting a grouped slot), its value span `[lo, hi]`
/// (stacked piles via the running total), the datum `value`, and the opacity. The one
/// place the modes branch; the geometry per direction is the callers' job.
fn visit_bars(
    bars: &[&Series],
    i: usize,
    mode: &BarMode,
    mut emit: impl FnMut(&Series, usize, usize, f64, f64, f64, f64),
) {
    match mode {
        BarMode::Grouped => {
            let s = bars.len();
            for (k, ser) in bars.iter().copied().enumerate() {
                if let Some(v) = datum(ser, i) {
                    emit(ser, k, s, 0.0, v, v, 1.0);
                }
            }
        }
        BarMode::Stacked => {
            let mut cum = 0.0;
            for ser in bars.iter().copied() {
                if let Some(v) = datum(ser, i) {
                    emit(ser, 0, 1, cum, cum + v, v, 1.0);
                    cum += v;
                }
            }
        }
        BarMode::Overlay => {
            for ser in bars.iter().copied() {
                if let Some(v) = datum(ser, i) {
                    emit(ser, 0, 1, 0.0, v, v, OVERLAY);
                }
            }
        }
    }
}

/// Column bars: the slot runs along x, the value grows up (a rect).
fn column_bars(plot: &Plot, chart: &Chart, bars: &[&Series], n: usize, out: &mut Vec<PlacedNode>) {
    for i in 0..n {
        let (sx0, sx1) = plot.slot_px(&chart.x.scale, i);
        let group_w = (sx1 - sx0) * GROUP;
        let group0 = sx0 + (sx1 - sx0 - group_w) / 2.0;
        let cat = chart.x.labels.get(i);
        visit_bars(bars, i, &chart.bars, |ser, k, s, lo, hi, value, op| {
            let bar_w = group_w / s as f64;
            let bx = group0 + (k as f64 + 0.5) * bar_w;
            let scale = &chart.values[ser.axis].scale;
            let y0 = plot.y_at(scale, scale.clamp(lo));
            let y1 = plot.y_at(scale, scale.clamp(hi));
            emit_rect(
                bx,
                (y0 + y1) / 2.0,
                bar_w * 0.9,
                (y0 - y1).abs(),
                ser,
                value,
                cat,
                op,
                out,
            );
        });
    }
}

/// Row bars: the slot runs down y, the value grows right (a rect) — the column flip.
fn row_bars(plot: &Plot, chart: &Chart, bars: &[&Series], n: usize, out: &mut Vec<PlacedNode>) {
    for i in 0..n {
        let sy0 = plot.y0 + (i as f64 / n as f64) * plot.h();
        let slot_h = plot.h() / n as f64;
        let group_h = slot_h * GROUP;
        let group0 = sy0 + (slot_h - group_h) / 2.0;
        let cat = chart.x.labels.get(i);
        visit_bars(bars, i, &chart.bars, |ser, k, s, lo, hi, value, op| {
            let bar_h = group_h / s as f64;
            let by = group0 + (k as f64 + 0.5) * bar_h;
            let scale = &chart.values[ser.axis].scale;
            let x0 = plot.x0 + scale.frac(scale.clamp(lo)) * plot.w();
            let x1 = plot.x0 + scale.frac(scale.clamp(hi)) * plot.w();
            emit_rect(
                (x0 + x1) / 2.0,
                by,
                (x1 - x0).abs(),
                bar_h * 0.9,
                ser,
                value,
                cat,
                op,
                out,
            );
        });
    }
}

/// Radial bars are wedges [SPEC 14.7]: each category owns the angular slot around
/// its spoke, grouped splits it angularly, stacked piles outward in radius.
fn radial_bars(plot: &Plot, chart: &Chart, bars: &[&Series], n: usize, out: &mut Vec<PlacedNode>) {
    let xs = &chart.x.scale;
    let slot = TAU / n as f64;
    let pad = slot * 0.14;
    for i in 0..n {
        let center = plot.spoke_angle(xs, i as f64);
        let (a_lo0, a_hi0) = (center - slot / 2.0 + pad, center + slot / 2.0 - pad);
        let cat = chart.x.labels.get(i);
        visit_bars(bars, i, &chart.bars, |ser, k, s, lo, hi, value, op| {
            let step = (a_hi0 - a_lo0) / s as f64;
            let a_lo = a_lo0 + step * k as f64;
            emit_wedge(
                plot,
                chart,
                ser,
                lo,
                hi,
                a_lo,
                a_lo + step,
                value,
                cat,
                op,
                out,
            );
        });
    }
}

/// A bar series' value at category `i`.
fn datum(ser: &Series, i: usize) -> Option<f64> {
    match &ser.data {
        Data::Categorical(v) => v.get(i).copied(),
        _ => None,
    }
}

/// A rectangular bar centred at (cx, cy) with the datum's `<title>`. Skips a flat bar.
#[allow(clippy::too_many_arguments)]
fn emit_rect(
    cx: f64,
    cy: f64,
    w: f64,
    h: f64,
    ser: &Series,
    value: f64,
    category: Option<&String>,
    opacity: f64,
    out: &mut Vec<PlacedNode>,
) {
    if w.min(h) <= 0.0 {
        return;
    }
    let mut bar = prim::rect(cx, cy, w, h, ser.color.clone(), opacity);
    prim::round(&mut bar, ser.radius);
    if let Some((color, width)) = &ser.outline {
        prim::outline(&mut bar, color.clone(), *width);
    }
    prim::set_hint(
        &mut bar,
        title(category, ser.label.as_deref(), value, ser.fmt),
    );
    out.push(bar);
}

/// A radial bar: an annular sector from value `lo` to `hi` (mapped to radius on the
/// series' axis) spanning `[a_lo, a_hi]`, with the datum's `<title>`.
#[allow(clippy::too_many_arguments)]
fn emit_wedge(
    plot: &Plot,
    chart: &Chart,
    ser: &Series,
    lo: f64,
    hi: f64,
    a_lo: f64,
    a_hi: f64,
    value: f64,
    category: Option<&String>,
    opacity: f64,
    out: &mut Vec<PlacedNode>,
) {
    let vs = &chart.values[ser.axis].scale;
    let (cx, cy) = plot.center();
    let r0 = vs.frac(lo) * plot.radius();
    let r1 = vs.frac(hi) * plot.radius();
    if (r1 - r0).abs() < 0.5 {
        return;
    }
    let mut wedge = prim::wedge(cx, cy, r0, r1, a_lo, a_hi, ser.color.clone(), opacity);
    if let Some((color, width)) = &ser.outline {
        prim::outline(&mut wedge, color.clone(), *width);
    }
    prim::set_hint(
        &mut wedge,
        title(category, ser.label.as_deref(), value, ser.fmt),
    );
    out.push(wedge);
}

/// The `<title>` text for a bar: category and/or series name, then the value
/// under the series' `format:` [SPEC 16].
fn title(category: Option<&String>, name: Option<&str>, value: f64, fmt: format::Format) -> String {
    let v = format::render(value, fmt);
    match (category.map(String::as_str), name) {
        (Some(c), Some(n)) => format!("{c} · {n}: {v}"),
        (Some(c), None) => format!("{c}: {v}"),
        (None, Some(n)) => format!("{n}: {v}"),
        (None, None) => v,
    }
}

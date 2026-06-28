//! Bar geometry ([CHARTS.md] §3): one rect per datum. Multiple `|bars|` series combine
//! by the chart's `bars:` mode — `grouped` side-by-side (the default), `stacked` piled
//! (each sits on the running total), or `overlay` translucently on top. Every mode
//! emits its rects through one `emit_bar`, and each bar carries its `<title>` (§14).

use super::model::{BarMode, Chart, Data, Series, SeriesKind};
use super::prim;
use super::project::Plot;
use super::scale::{Scale, fmt_tick};
use crate::layout::PlacedNode;

/// The bar group's share of a category slot (~14% padding each side).
const GROUP: f64 = 0.72;
/// An `overlay` bar's translucency, so a bar behind it reads through.
const OVERLAY: f64 = 0.6;

/// Emit the bars for every bar series, combined per the chart's `bars:` mode.
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
    for i in 0..n {
        let (sx0, sx1) = plot.slot_px(&chart.x.scale, i);
        let slot_w = sx1 - sx0;
        let group_w = slot_w * GROUP;
        let cx = (sx0 + sx1) / 2.0;
        let cat = chart.x.labels.get(i);
        match chart.bars {
            // Side-by-side: split the group into one column per series.
            BarMode::Grouped => {
                let bar_w = group_w / bars.len() as f64;
                for (k, ser) in bars.iter().copied().enumerate() {
                    let Some(value) = datum(ser, i) else { continue };
                    let bx = sx0 + (slot_w - group_w) / 2.0 + (k as f64 + 0.5) * bar_w;
                    emit_bar(
                        plot, chart, ser, bx, bar_w, 0.0, value, value, cat, 1.0, out,
                    );
                }
            }
            // Piled: each bar starts at the running total of the bars below it.
            BarMode::Stacked => {
                let mut cum = 0.0;
                for ser in bars.iter().copied() {
                    let Some(value) = datum(ser, i) else { continue };
                    emit_bar(
                        plot,
                        chart,
                        ser,
                        cx,
                        group_w,
                        cum,
                        cum + value,
                        value,
                        cat,
                        1.0,
                        out,
                    );
                    cum += value;
                }
            }
            // Overlaid: one full-width column per series, translucent, in order.
            BarMode::Overlay => {
                for ser in bars.iter().copied() {
                    let Some(value) = datum(ser, i) else { continue };
                    emit_bar(
                        plot, chart, ser, cx, group_w, 0.0, value, value, cat, OVERLAY, out,
                    );
                }
            }
        }
    }
}

/// A bar series' value at category `i`.
fn datum(ser: &Series, i: usize) -> Option<f64> {
    match &ser.data {
        Data::Categorical(v) => v.get(i).copied(),
        _ => None,
    }
}

/// One bar: the rect from value `lo` to `hi` on the series' axis, at horizontal centre
/// `bx`, carrying the datum `value`'s `<title>`. A zero-height bar is skipped.
#[allow(clippy::too_many_arguments)]
fn emit_bar(
    plot: &Plot,
    chart: &Chart,
    ser: &Series,
    bx: f64,
    bar_w: f64,
    lo: f64,
    hi: f64,
    value: f64,
    category: Option<&String>,
    opacity: f64,
    out: &mut Vec<PlacedNode>,
) {
    let scale = &chart.values[ser.axis].scale;
    let y0 = plot.y_at(scale, scale.clamp(lo));
    let y1 = plot.y_at(scale, scale.clamp(hi));
    let height = (y0 - y1).abs();
    if height <= 0.0 {
        return;
    }
    let mut bar = prim::rect(
        bx,
        (y0 + y1) / 2.0,
        bar_w * 0.9,
        height,
        ser.color.clone(),
        opacity,
    );
    prim::set_title(&mut bar, title(category, ser.label.as_deref(), value));
    out.push(bar);
}

/// The `<title>` text for a bar: category and/or series name, then the value.
fn title(category: Option<&String>, name: Option<&str>, value: f64) -> String {
    let v = fmt_tick(value);
    match (category.map(String::as_str), name) {
        (Some(c), Some(n)) => format!("{c} · {n}: {v}"),
        (Some(c), None) => format!("{c}: {v}"),
        (None, Some(n)) => format!("{n}: {v}"),
        (None, None) => v,
    }
}

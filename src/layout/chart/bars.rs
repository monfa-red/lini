//! Bar geometry ([CHARTS.md] §3): one rect per datum, grouped side-by-side across
//! the bar series within each category slot (the default — `bars: stacked | overlay`
//! arrive in a later step). Each bar carries a `<title>` (the tooltip floor, §14).

use super::model::{Chart, Data, SeriesKind};
use super::prim;
use super::project::Plot;
use super::scale::{Scale, fmt_tick};
use crate::layout::PlacedNode;

/// Emit the bars for every bar series, grouped within each category slot.
pub fn lay_out(plot: &Plot, chart: &Chart, out: &mut Vec<PlacedNode>) {
    let bars: Vec<usize> = chart
        .series
        .iter()
        .enumerate()
        .filter(|(_, s)| matches!(s.kind, SeriesKind::Bars))
        .map(|(i, _)| i)
        .collect();
    let s = bars.len();
    if s == 0 {
        return;
    }
    let Scale::Band { n } = chart.x.scale else {
        return; // bars are categorical; a numeric x carries no slots
    };
    for i in 0..n {
        let (sx0, sx1) = plot.slot_px(&chart.x.scale, i);
        let slot_w = sx1 - sx0;
        let group_w = slot_w * 0.72; // ~14% padding each side of a slot
        let bar_w = group_w / s as f64;
        for (k, &si) in bars.iter().enumerate() {
            let ser = &chart.series[si];
            let Data::Categorical(v) = &ser.data else {
                continue;
            };
            let Some(&value) = v.get(i) else { continue };
            let scale = &chart.values[ser.axis].scale;
            let base = plot.y_at(scale, scale.clamp(0.0));
            let top = plot.y_at(scale, scale.clamp(value));
            let height = (base - top).abs();
            if height <= 0.0 {
                continue;
            }
            let bx = sx0 + (slot_w - group_w) / 2.0 + (k as f64 + 0.5) * bar_w;
            let mut bar = prim::rect(
                bx,
                (base + top) / 2.0,
                bar_w * 0.9,
                height,
                ser.color.clone(),
            );
            prim::set_title(
                &mut bar,
                title(chart.x.labels.get(i), ser.label.as_deref(), value),
            );
            out.push(bar);
        }
    }
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

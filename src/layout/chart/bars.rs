//! Bar geometry ([CHARTS.md] §3): one rect per datum, grouped side-by-side across
//! series within each category slot (the default — `bars: stacked | overlay` arrive
//! in a later step). Each bar carries a `<title>` (the tooltip floor, §14).

use super::prim;
use super::project::Plot;
use super::scale::fmt_tick;
use crate::layout::PlacedNode;
use crate::resolve::ResolvedValue;

/// A resolved bar series: its values, optional legend label, and resolved colour.
pub struct Series {
    pub values: Vec<f64>,
    pub label: Option<String>,
    pub color: ResolvedValue,
}

/// Emit the bars for every series, grouped within each category slot.
pub fn lay_out(plot: &Plot, series: &[Series], categories: &[String], out: &mut Vec<PlacedNode>) {
    let s = series.len();
    if s == 0 {
        return;
    }
    let group_w = plot.slot_w() * 0.72; // ~14% padding each side of a slot
    let bar_w = group_w / s as f64;
    let baseline = plot.baseline();
    for i in 0..plot.n {
        let center = plot.slot_center(i);
        for (si, ser) in series.iter().enumerate() {
            let Some(&v) = ser.values.get(i) else {
                continue;
            };
            let top = plot.y(v);
            let height = (baseline - top).abs();
            if height <= 0.0 {
                continue;
            }
            let bx = center - group_w / 2.0 + (si as f64 + 0.5) * bar_w;
            let cy = (baseline + top) / 2.0;
            let mut bar = prim::rect(bx, cy, bar_w * 0.9, height, ser.color.clone());
            prim::set_title(&mut bar, title(categories.get(i), ser.label.as_deref(), v));
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

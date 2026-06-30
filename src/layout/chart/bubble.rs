//! `|bubble|` rendering ([CHARTS.md] §3): one labelled oval per node, area-scaled across
//! the chart (area ∝ value, so the largest fits) and projected through `Plot` like any
//! datum. The label rides the shared placement pass ([`super::labels`], §14): centred in
//! the bubble when it fits, else beside it, else on hover. Reuses `prim::oval` / the
//! projection / the palette.

use super::labels;
use super::model::{Bubble, Chart};
use super::project::Plot;
use super::scale::fmt_tick;
use crate::layout::PlacedNode;
use crate::layout::prim;
use crate::resolve::ResolvedValue;

/// The largest bubble's radius, as a fraction of the smaller plot dimension.
const MAX_R: f64 = 0.14;

pub fn lay_out(plot: &Plot, chart: &Chart, out: &mut Vec<PlacedNode>, reqs: &mut Vec<labels::Req>) {
    let vmax = chart
        .bubbles
        .iter()
        .map(|b| b.value)
        .fold(0.0_f64, f64::max);
    if vmax <= 0.0 {
        return;
    }
    let rmax = plot.w().min(plot.h()) * MAX_R;
    let xs = &chart.x.scale;
    for b in &chart.bubbles {
        let vs = &chart.values[b.axis].scale;
        if !xs.contains(b.at.0) || !vs.contains(b.at.1) {
            continue; // off-plot — cropped
        }
        let (cx, cy) = plot.project(xs, b.at.0, vs, b.at.1);
        let d = ((b.value / vmax).sqrt() * rmax * 2.0).max(2.0); // area ∝ value
        let mut bubble = prim::oval(cx, cy, d, d, b.color.clone());
        if let Some((color, width)) = &b.outline {
            prim::outline(&mut bubble, color.clone(), *width);
        }
        prim::set_title(&mut bubble, bubble_title(b));
        out.push(bubble);
        // The label joins the shared pass ([CHARTS.md] §14): its `inside` seat centres it
        // on the bubble when the text fits (the on-fill tint), else it sits beside (muted);
        // a `pointer-events: none` keeps the bubble's hover working through it.
        if let Some(label) = &b.label
            && !label.is_empty()
            && b.tooltip.inline()
        {
            reqs.push(labels::Req {
                anchor: (cx, cy),
                radius: d / 2.0,
                text: label.clone(),
                color: muted(),
                forced: b.tooltip.forced(),
                inside: Some(labels::Inside {
                    fit: d,
                    color: on_fill(),
                }),
            });
        }
    }
}

/// The muted role tint a bubble label takes when it sits *beside* the bubble.
fn muted() -> ResolvedValue {
    ResolvedValue::LiveVar {
        name: "muted".into(),
        raw: false,
    }
}

/// A bubble's `<title>` — its name and value (the area metric).
fn bubble_title(b: &Bubble) -> String {
    let v = fmt_tick(b.value);
    match &b.label {
        Some(l) => format!("{l}: {v}"),
        None => v,
    }
}

/// The label tint for text centred on a coloured bubble — the light accent-text role.
fn on_fill() -> ResolvedValue {
    ResolvedValue::LiveVar {
        name: "accent-text".into(),
        raw: false,
    }
}

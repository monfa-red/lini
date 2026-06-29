//! `|bubble|` rendering ([CHARTS.md] §3): one labelled oval per node, area-scaled across
//! the chart (area ∝ value, so the largest fits) and projected through `Plot` like any
//! datum. The smart label sits centred in the bubble when it fits, else only as the
//! `<title>` floor (§14). Reuses `prim::oval` / the projection / the palette.

use super::LABEL_SIZE;
use super::model::{Bubble, Chart};
use super::prim;
use super::project::Plot;
use super::scale::fmt_tick;
use crate::layout::PlacedNode;
use crate::resolve::ResolvedValue;

/// The largest bubble's radius, as a fraction of the smaller plot dimension.
const MAX_R: f64 = 0.14;

pub fn lay_out(plot: &Plot, chart: &Chart, out: &mut Vec<PlacedNode>) {
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
        // The centred label rides *inside* the bubble's `<g>` (at its centre, so relative
        // to the group's translate), not as a sibling on top: hovering the label then
        // still counts as hovering the bubble, so its `:hover` tooltip stays put — and the
        // label keeps default pointer events, so it remains selectable ([CHARTS.md] §14).
        if let Some(label) = &b.label
            && prim::text_width(label, LABEL_SIZE) <= d
        {
            bubble.children.push(prim::text(
                label,
                0.0,
                0.0,
                LABEL_SIZE,
                Some(on_fill()),
                false,
            ));
        }
        out.push(bubble);
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

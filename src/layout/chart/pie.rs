//! `layout: pie` ([CHARTS.md] §13) — value as **angle**. Each `|slice|` is a wedge whose
//! angle is its share of the total, filled clockwise from the top; `hole:` cuts a donut.
//! Like a chart, a pie owns its subtree and lowers to primitives (`prim::wedge`), reusing
//! the chart's box, palette, legend, and `<title>` machinery — the renderer learns nothing.

use super::model::{self, Slice};
use super::prim;
use super::scale::fmt_tick;
use super::{LABEL_SIZE, TITLE_SIZE, chart_box, lay_out_legend};
use crate::error::Error;
use crate::layout::PlacedNode;
use crate::resolve::{AttrMap, ResolvedInst, ResolvedValue};
use std::f64::consts::TAU;

/// Is this node a pie container ([CHARTS.md] §2)? Detected by its `layout:` attr.
pub fn is_pie(attrs: &AttrMap) -> bool {
    matches!(attrs.get("layout"), Some(ResolvedValue::Ident(s)) if s == "pie")
}

/// Lay a pie out into one `PlacedNode`: the chart box carrying the slice wedges, title,
/// and legend.
pub fn layout_pie(inst: &ResolvedInst) -> Result<PlacedNode, Error> {
    let pie = model::build_pie(inst)?;
    // A pie is square ([CHARTS.md] §2).
    let w = inst.attrs.number("width").unwrap_or(280.0);
    let h = inst.attrs.number("height").unwrap_or(280.0);

    // The circle is centred in the box below the title, above the legend.
    let title_h = if pie.title.is_some() {
        TITLE_SIZE * 2.0
    } else {
        0.0
    };
    let entries = legend_entries(&pie.slices);
    let legend_h = if entries.len() >= 2 {
        LABEL_SIZE * 1.6
    } else {
        0.0
    };
    let margin = 8.0;
    let top = title_h + margin;
    let avail_h = h - top - legend_h - margin;
    let r = avail_h.min(w - 2.0 * margin).max(0.0) / 2.0;
    let cy = -h / 2.0 + top + avail_h / 2.0;
    let inner = pie.hole * r;
    let total: f64 = pie.slices.iter().map(|s| s.value).sum();

    let mut kids = Vec::new();
    let mut a = 0.0;
    for s in &pie.slices {
        let span = s.value / total * TAU;
        if span > 0.0 {
            let mut wedge = prim::wedge(0.0, cy, inner, r, a, a + span, s.color.clone(), 1.0);
            prim::set_title(&mut wedge, slice_title(s, total));
            kids.push(wedge);
        }
        a += span;
    }
    if let Some(t) = &pie.title {
        kids.push(prim::text(
            t,
            0.0,
            -h / 2.0 + 8.0 + TITLE_SIZE * 0.7,
            TITLE_SIZE,
            None,
            true,
        ));
    }
    if entries.len() >= 2 {
        lay_out_legend(&entries, h / 2.0 - LABEL_SIZE * 0.9, &mut kids);
    }
    let kids = super::tooltip::apply(kids, super::tooltip::read(&inst.attrs)?, w, h);
    Ok(chart_box(inst, w, h, kids))
}

/// The legend entries — one per slice that carries a label ([CHARTS.md] §9).
fn legend_entries(slices: &[Slice]) -> Vec<(String, ResolvedValue)> {
    slices
        .iter()
        .filter_map(|s| s.label.clone().map(|l| (l, s.color.clone())))
        .collect()
}

/// A slice's `<title>` ([CHARTS.md] §14): its name, value, and percent of the total.
fn slice_title(s: &Slice, total: f64) -> String {
    let pct = fmt_tick((s.value / total * 100.0).round());
    let v = fmt_tick(s.value);
    match &s.label {
        Some(l) => format!("{l}: {v} ({pct}%)"),
        None => format!("{v} ({pct}%)"),
    }
}

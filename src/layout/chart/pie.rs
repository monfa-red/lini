//! `layout: pie` [SPEC 14.7] — value as **angle**. Each `|slice|` is a wedge whose
//! angle is its share of the total, filled clockwise from the top; `hole:` cuts a donut.
//! Like a chart, a pie owns its subtree and lowers to primitives (`prim::wedge`), reusing
//! the chart's box, palette, legend, and `<title>` machinery — the renderer learns nothing.

use super::metrics::{LABEL_SIZE, TITLE_SIZE};
use super::model::{Pie, Slice, fill_color, fill_outline, label_of, live, read_gap, tag};
use super::palette;
use super::scale::fmt_tick;
use super::{chart_box, lay_out_legend, legend_reserve, title_reserve};
use crate::error::Error;
use crate::layout::PlacedNode;
use crate::layout::prim;
use crate::resolve::{AttrMap, NodeKind, ResolvedInst, ResolvedValue};
use crate::span::Span;
use std::f64::consts::TAU;

/// Is this node a pie container [SPEC 14.1]? Detected by its `layout:` attr.
pub fn is_pie(attrs: &AttrMap) -> bool {
    matches!(attrs.get("layout"), Some(ResolvedValue::Ident(s)) if s == "pie")
}

/// Lay a pie out into one `PlacedNode`: the chart box carrying the slice wedges, title,
/// and legend.
pub fn layout_pie(inst: &ResolvedInst) -> Result<PlacedNode, Error> {
    let pie = build_pie(inst)?;
    // A pie is square [SPEC 14.1].
    let w = inst.attrs.number("width").unwrap_or(280.0);
    let h = inst.attrs.number("height").unwrap_or(280.0);

    // The circle is centred in the box below the title, above the legend.
    let title_h = title_reserve(pie.title.is_some(), pie.gap);
    let entries = legend_entries(&pie.slices);
    let legend_h = legend_reserve(entries.len(), pie.gap);
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
            if let Some((color, width)) = &s.outline {
                prim::outline(&mut wedge, color.clone(), *width);
            }
            prim::set_hint(&mut wedge, slice_title(s, total));
            kids.push(wedge);
        }
        a += span;
    }
    if let Some(t) = &pie.title {
        // Same `.lini-chart-title` rule as a chart's [SPEC 14.6/17].
        kids.push(prim::text_classed(
            t,
            0.0,
            -h / 2.0 + 8.0 + TITLE_SIZE * 0.7,
            TITLE_SIZE,
            "chart-title",
            crate::font::Font::semibold(pie.font_kind),
        ));
    }
    if entries.len() >= 2 {
        lay_out_legend(
            &entries,
            h / 2.0 - LABEL_SIZE * 0.9,
            pie.font_kind,
            &mut kids,
        );
    }
    let kids = super::tooltip::apply(
        kids,
        super::tooltip::read(&inst.attrs)?,
        w,
        h,
        pie.font_kind,
    );
    Ok(chart_box(inst, w, h, kids))
}

/// The legend entries — one per slice that carries a label [SPEC 14.6]. The swatch
/// mirrors the slice's fill and its (default-deep or explicit) edge.
fn legend_entries(slices: &[Slice]) -> Vec<super::LegendEntry> {
    slices
        .iter()
        .filter_map(|s| {
            let edge = s.outline.as_ref().map(|(c, _)| c.clone());
            s.label.clone().map(|l| (l, s.color.clone(), edge))
        })
        .collect()
}

/// A slice's `<title>` [SPEC 14.8]: its name, value, and percent of the total.
fn slice_title(s: &Slice, total: f64) -> String {
    let pct = fmt_tick((s.value / total * 100.0).round());
    let v = fmt_tick(s.value);
    match &s.label {
        Some(l) => format!("{l}: {v} ({pct}%)"),
        None => format!("{v} ({pct}%)"),
    }
}

/// Parse a `layout: pie` into its slices [SPEC 14.7]. All pie validation [SPEC 20]
/// lives here; the wedge geometry is the renderer's job. Reuses the chart's `tag`,
/// `label_of`, the `fill:` / `outline:` paint readers, and the palette walk (per
/// slice — [SPEC 14.6]).
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
        font_kind: inst.font.kind,
    })
}

/// A pie's `hole:` fraction [SPEC 14.7] — `0` a pie, `0 < n < 1` a donut.
fn read_hole(attrs: &AttrMap) -> Result<f64, Error> {
    match attrs.get("hole") {
        None => Ok(0.0),
        Some(ResolvedValue::Number(n)) if *n >= 0.0 && *n < 1.0 => Ok(*n),
        _ => Err(Error::at(Span::empty(), "'hole' is a fraction 0..1")),
    }
}

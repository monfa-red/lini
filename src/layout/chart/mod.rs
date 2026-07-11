//! `layout: chart` [SPEC 14] — a container that reads all its children, fixes a
//! shared data→pixel scale, and **lowers to primitive `PlacedNode`s**. The renderer,
//! cascade, palette, theming, and `--static` are all reused unchanged; the chart
//! adds only the scale-and-place algorithm here.
//!
//! `|bars|` / `|line|` / `|dots|` / `|area|` / `|bubble|` over a categorical band or a
//! numeric x, with explicit `|axis|` children, nice/log scales, formulas, bands,
//! annotations, a legend, and a title. Every series lowers through one `Plot::project`,
//! so the `direction: column | row | radial` flip (a radar reusing the cartesian
//! builders) is a projector change, not a rewrite. `layout: pie` is the sibling layout
//! (`pie.rs`). Labels present per `tooltip: none | hover | auto | always` (`tooltip.rs`):
//! a native `<title>` floor + a live `:hover` card, and — for `auto` / `always` — inline
//! `labels:` placed by one greedy pass (`labels.rs`).

mod annot;
mod axis;
mod bars;
mod bubble;
mod frame;
mod labels;
mod legend;
mod marks;
pub(crate) mod metrics;
mod model;
mod palette;
mod pie;
use super::prim;
mod project;
mod radial;
mod scale;
mod tooltip;

pub(super) use pie::{is_pie, layout_pie};

use crate::error::Error;
use crate::layout::{Bbox, PlacedNode};
use crate::resolve::{AttrMap, ResolvedInst, ResolvedValue};
use metrics::{AXIS_TITLE_SIZE, LABEL_SIZE, TITLE_SIZE};
use model::{Chart, Series, SeriesKind, Side};
use project::{Dir, Plot};

use frame::{plot_rect, title_reserve};
use legend::{LegendEntry, lay_out_legend, legend_entries, legend_reserve};

/// Is this node a chart container [SPEC 14.1]? Detected by its `layout:` attr —
/// the same key `read_layout_mode` owns — so a chart is intercepted before the generic
/// container path. (`layout: pie` is the sibling layout, `pie::is_pie`.)
pub(super) fn is_chart(attrs: &AttrMap) -> bool {
    matches!(attrs.get("layout"), Some(ResolvedValue::Ident(s)) if s == "chart")
}

/// Lay a chart out into one `PlacedNode`: the chart box, carrying the lowered
/// gridlines / series / labels / title / legend as its pre-positioned children.
pub(super) fn layout_chart(
    inst: &ResolvedInst,
    funcs: &crate::expr::FuncTable,
) -> Result<PlacedNode, Error> {
    let chart = model::build(inst, funcs)?;
    // A radial (or pie) chart is square; a cartesian one is wide [SPEC 14.1].
    let square = chart.dir == Dir::Radial;
    let w = inst
        .attrs
        .number("width")
        .unwrap_or(if square { 280.0 } else { 360.0 });
    let h = inst
        .attrs
        .number("height")
        .unwrap_or(if square { 280.0 } else { 220.0 });
    let plot = plot_rect(&chart, w, h);

    // Semantic draw order [SPEC 14.9]: gridlines/web behind, then areas, bars,
    // lines, dots, then annotations and labels — pushed in order, so a later child
    // paints over an earlier one. The series builders project through `Plot`, so a
    // radial chart reuses the exact `areas`/`lines`/`dots`/`bars` passes; only the
    // gridlines, labels, and annotations differ by direction.
    let mut kids = Vec::new();
    // Inline-label requests accrue as the marks lay out (bubbles, `|mark|` points), then
    // join the series' `labels:` for one placement pass [SPEC 14.8].
    let mut reqs = Vec::new();
    match plot.dir {
        Dir::Radial => radial::gridlines(&plot, &chart, &mut kids),
        Dir::Column | Dir::Row => {
            annot::band_shades(&plot, &chart, &mut kids);
            axis::gridlines(&plot, &chart, &mut kids);
        }
    }
    marks::areas(&plot, &chart, &mut kids);
    bars::lay_out(&plot, &chart, &mut kids);
    marks::lines(&plot, &chart, &mut kids);
    marks::dots(&plot, &chart, &mut kids);
    bubble::lay_out(&plot, &chart, &mut kids, &mut reqs);
    match plot.dir {
        Dir::Radial => radial::labels(&plot, &chart, &mut kids),
        Dir::Column | Dir::Row => {
            annot::marks(&plot, &chart, &mut kids, &mut reqs);
            axis::labels(&plot, &chart, &mut kids);
            annot::band_ticks(&plot, &chart, &mut kids);
        }
    }
    if let Some(t) = &chart.title {
        // The title rides its own `.lini-chart-title` rule (14px semibold) —
        // no inline font [SPEC 14.6/17].
        kids.push(prim::text_classed(
            t,
            0.0,
            -h / 2.0 + 8.0 + TITLE_SIZE * 0.7,
            TITLE_SIZE,
            "chart-title",
            crate::font::Font::semibold(chart.font_kind),
        ));
    }
    let legend = legend_entries(&chart);
    if legend.len() >= 2 {
        lay_out_legend(
            &legend,
            h / 2.0 - LABEL_SIZE * 0.9,
            chart.font_kind,
            &mut kids,
        );
    }

    // Inline data labels [SPEC 14.8]: the series' `labels:` join the bubble / mark
    // reqs gathered above, all placed by one greedy pass after the series / axes / title
    // so they sit above them and below the hover cards.
    labels::collect_series(&plot, &chart, &mut reqs);
    let lines = labels::series_lines(&plot, &chart);
    kids.extend(labels::place(&reqs, &plot, &lines, chart.font_kind));

    let kids = tooltip::apply(kids, chart.tooltip, w, h, chart.font_kind);
    Ok(chart_box(inst, w, h, kids))
}

/// The chart container `PlacedNode`: a `Block` of the chart box carrying the lowered
/// primitives as pre-positioned children. Shared by `layout: chart` and `layout: pie`,
/// so both place as one unit and keep the node's id / classes / paint.
pub(super) fn chart_box(inst: &ResolvedInst, w: f64, h: f64, kids: Vec<PlacedNode>) -> PlacedNode {
    prim::container(inst, Bbox::centered(w, h), kids)
}

#[cfg(test)]
mod tests;

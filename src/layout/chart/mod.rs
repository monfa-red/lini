//! `layout: chart` ([CHARTS.md]) — a container that reads all its children, fixes a
//! shared data→pixel scale, and **lowers to primitive `PlacedNode`s**. The renderer,
//! cascade, palette, theming, and `--bake-vars` are all reused unchanged; the chart
//! adds only the scale-and-place algorithm here.
//!
//! The cartesian toolkit (steps 1–2): `|bars|` / `|line|` / `|dots|` over a
//! categorical band or a numeric x, with explicit `|axis|` children, nice scales,
//! gridlines, titles, and a legend. Formulas, bands, annotations, radial, and pie
//! follow in later steps (see `PLAN.md`).

mod axis;
mod bars;
mod marks;
mod model;
mod palette;
mod prim;
mod project;
mod scale;

use crate::error::Error;
use crate::layout::{Bbox, PlacedNode};
use crate::resolve::{AttrMap, NodeKind, ResolvedInst, ResolvedValue};
use model::{Chart, Side};
use project::Plot;

const TITLE_SIZE: f64 = 13.0;
const AXIS_TITLE_SIZE: f64 = 11.0;
const LABEL_SIZE: f64 = 11.0;

/// Is this node a chart container ([CHARTS.md] §2)? Detected by its `layout:` attr —
/// the same key `read_layout_mode` owns — so a chart is intercepted before the
/// generic container path. (`layout: pie` is recognised in a later step.)
pub(super) fn is_chart(attrs: &AttrMap) -> bool {
    matches!(attrs.get("layout"), Some(ResolvedValue::Ident(s)) if s == "chart")
}

/// Lay a chart out into one `PlacedNode`: the chart box, carrying the lowered
/// gridlines / series / labels / title / legend as its pre-positioned children.
pub(super) fn layout_chart(
    inst: &ResolvedInst,
    funcs: &crate::expr::FuncTable,
) -> Result<PlacedNode, Error> {
    let span = inst.span;
    let w = inst.attrs.number("width").unwrap_or(360.0);
    let h = inst.attrs.number("height").unwrap_or(220.0);

    let chart = model::build(inst, funcs)?;
    let plot = plot_rect(&chart, w, h);

    // Semantic draw order ([CHARTS.md] §15): gridlines, bars, lines/dots, labels,
    // title, legend — pushed in order, so a later child paints over an earlier one.
    let mut kids = Vec::new();
    axis::gridlines(&plot, &chart, &mut kids);
    bars::lay_out(&plot, &chart, &mut kids);
    marks::lay_out(&plot, &chart, &mut kids);
    axis::labels(&plot, &chart, &mut kids);
    if let Some(t) = &chart.title {
        kids.push(prim::text(
            t,
            0.0,
            -h / 2.0 + 8.0 + TITLE_SIZE * 0.7,
            TITLE_SIZE,
            None,
            true,
        ));
    }
    let legend = legend_entries(&chart);
    if legend.len() >= 2 {
        lay_out_legend(&legend, h / 2.0 - LABEL_SIZE * 0.9, &mut kids);
    }

    Ok(PlacedNode {
        id: inst.id.clone(),
        kind: NodeKind::Block,
        type_chain: inst.type_chain.clone(),
        applied_styles: inst.applied_styles.clone(),
        label: None,
        attrs: inst.attrs.clone(),
        own_style: AttrMap::new(),
        markers: inst.markers.clone(),
        cx: 0.0,
        cy: 0.0,
        bbox: Bbox::centered(w, h),
        rotation: inst.attrs.number("rotate").unwrap_or(0.0),
        children: kids,
        dividers: Vec::new(),
        span,
    })
}

/// The plot rect = the chart box inset by the gutters its labels / titles / legend
/// need, all measured at compile time (SPEC §6).
fn plot_rect(chart: &Chart, w: f64, h: f64) -> Plot {
    let left = nonzero(side_gutter(chart, false), 12.0);
    let right = nonzero(side_gutter(chart, true), 12.0);
    let title_h = if chart.title.is_some() {
        TITLE_SIZE * 1.4
    } else {
        0.0
    };
    let value_title_h = if chart.values.iter().any(|a| a.title.is_some()) {
        AXIS_TITLE_SIZE * 1.4
    } else {
        0.0
    };
    let x_title_h = if chart.x.title.is_some() {
        AXIS_TITLE_SIZE * 1.4
    } else {
        0.0
    };
    let legend_h = if legend_entries(chart).len() >= 2 {
        LABEL_SIZE * 1.6
    } else {
        0.0
    };
    Plot {
        x0: -w / 2.0 + left,
        x1: w / 2.0 - right,
        y0: -h / 2.0 + 8.0 + title_h + value_title_h,
        y1: h / 2.0 - 6.0 - LABEL_SIZE * 1.4 - x_title_h - legend_h,
    }
}

/// The label-width gutter for the value axes on one side (`right` = right edge), or
/// 0 if no value axis sits there.
fn side_gutter(chart: &Chart, right: bool) -> f64 {
    let mut maxw = 0.0_f64;
    let mut any = false;
    for axis in chart
        .values
        .iter()
        .filter(|a| matches!(a.side, Side::Right) == right)
    {
        any = true;
        for &t in axis.scale.ticks() {
            maxw = maxw.max(prim::text_width(&scale::label(t, &axis.unit), LABEL_SIZE));
        }
    }
    if any { maxw + 10.0 } else { 0.0 }
}

fn nonzero(v: f64, fallback: f64) -> f64 {
    if v > 0.0 { v } else { fallback }
}

/// The legend entries — one per series that carries a label (no label → no entry,
/// [CHARTS.md] §9).
fn legend_entries(chart: &Chart) -> Vec<(String, ResolvedValue)> {
    chart
        .series
        .iter()
        .filter_map(|s| s.label.clone().map(|l| (l, s.color.clone())))
        .collect()
}

/// A centred row of swatch + label entries at vertical `cy`.
fn lay_out_legend(entries: &[(String, ResolvedValue)], cy: f64, out: &mut Vec<PlacedNode>) {
    const SW: f64 = 11.0; // swatch side
    const GAP: f64 = 5.0; // swatch → label
    const ITEM_GAP: f64 = 16.0; // entry → entry
    let widths: Vec<f64> = entries
        .iter()
        .map(|(l, _)| prim::text_width(l, LABEL_SIZE))
        .collect();
    let per: f64 = widths.iter().map(|w| SW + GAP + w).sum();
    let total = per + ITEM_GAP * widths.len().saturating_sub(1) as f64;
    let mut x = -total / 2.0;
    for ((label, color), &tw) in entries.iter().zip(&widths) {
        out.push(prim::rect(x + SW / 2.0, cy, SW, SW, color.clone()));
        out.push(prim::text(
            label,
            x + SW + GAP + tw / 2.0,
            cy,
            LABEL_SIZE,
            None,
            false,
        ));
        x += SW + GAP + tw + ITEM_GAP;
    }
}

#[cfg(test)]
mod tests {
    /// Live-mode SVG for a source (palette vars stay `var(--lini-…)`).
    fn svg(src: &str) -> String {
        crate::compile_str(src).expect("compile")
    }

    /// The layout-phase error message for a chart that resolves but won't lay out.
    fn layout_err(src: &str) -> String {
        let toks = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(&toks).expect("parse");
        let lowered = crate::desugar::desugar(&file).expect("desugar");
        let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
        crate::layout::layout(&program)
            .err()
            .expect("expected a layout error")
            .to_string()
    }

    #[test]
    fn bars_chart_lowers_to_axis_bars_legend_and_title() {
        let s = svg(
            "|chart| \"T\" { categories: \"a\" \"b\" } [\n  |bars| \"S1\" { data: 3 6 }\n  |bars| \"S2\" { data: 4 2 }\n]\n",
        );
        assert!(s.contains("lini-chart"), "chart container class: {s}");
        // Palette walk: series 0 rose, series 1 orange — red skipped.
        assert!(s.contains("var(--lini-rose)"), "series 0 hue: {s}");
        assert!(s.contains("var(--lini-orange)"), "series 1 hue: {s}");
        assert!(!s.contains("var(--lini-red)"), "red is reserved: {s}");
        assert!(s.contains("var(--lini-grid)"), "gridlines: {s}");
        assert!(s.contains("<title>a · S1: 3</title>"), "bar title: {s}");
        assert!(s.contains(">T</text>"), "chart title text: {s}");
    }

    #[test]
    fn a_line_series_draws_a_polyline() {
        let s = svg("|chart| { categories: \"a\" \"b\" \"c\" } [\n  |line| { data: 3 6 4 }\n]\n");
        assert!(s.contains("<polyline"), "polyline: {s}");
    }

    #[test]
    fn a_dots_series_over_points_draws_ellipses() {
        let s = svg(
            "|chart| [\n  |axis| { side: bottom }\n  |axis| { side: left }\n  |dots| { data: 1 5, 2 3, 3 8 }\n]\n",
        );
        assert!(s.contains("<ellipse"), "dots render as ellipses: {s}");
    }

    #[test]
    fn an_explicit_fill_overrides_the_palette_walk() {
        let s = svg("|chart| { categories: \"a\" } [\n  |bars| { data: 5; fill: --teal }\n]\n");
        assert!(s.contains("var(--lini-teal)"), "explicit fill kept: {s}");
        assert!(!s.contains("var(--lini-rose)"), "palette not walked: {s}");
    }

    #[test]
    fn a_dual_axis_chart_binds_series_by_id() {
        let s = svg(
            "|chart| { categories: \"a\" \"b\" } [\n  |axis#n| { side: left }\n  |axis#p| { side: right }\n  |bars| { data: 10 20; axis: n }\n  |line| { data: 4 9; axis: p }\n]\n",
        );
        assert!(s.contains("<line "), "the 2-point line: {s}");
        // Each axis's domain comes from its bound series: bars 10/20 → a left axis to
        // 20, line 4/9 → a right axis to 10 (whose 1-2 ticks include 8; the left's
        // 0-5-10-15-20 do not). Distinct domains prove the by-id binding.
        assert!(s.contains(">20</text>"), "left axis from bars: {s}");
        assert!(s.contains(">8</text>"), "right axis from line: {s}");
    }

    #[test]
    fn an_unknown_axis_id_errors_with_a_suggestion() {
        let e = layout_err(
            "|chart| { categories: \"a\" } [\n  |axis#v| { side: left }\n  |line| { data: 1; axis: nope }\n]\n",
        );
        assert!(e.contains("axis 'nope' not found"), "{e}");
        assert!(e.contains("'v'"), "suggests the known id: {e}");
    }

    #[test]
    fn empty_chart_errors() {
        assert!(layout_err("|chart| \"T\"\n").contains("at least one series"));
    }

    #[test]
    fn data_count_must_match_categories() {
        let e = layout_err("|chart| { categories: \"a\" \"b\" } [\n  |bars| { data: 1 2 3 }\n]\n");
        assert!(e.contains("3 values but the chart has 2 categories"), "{e}");
    }

    #[test]
    fn data_and_fn_together_error() {
        let e = layout_err("|chart| { categories: \"a\" } [\n  |bars| { data: 1; fn: `2` }\n]\n");
        assert!(e.contains("not both"), "{e}");
    }

    #[test]
    fn a_non_series_child_is_rejected() {
        let e = layout_err("|chart| [\n  |box| \"x\"\n]\n");
        assert!(e.contains("series"), "{e}");
    }

    #[test]
    fn a_fn_series_samples_a_curve_over_the_x_domain() {
        let s = svg(
            "|chart| [\n  |axis| { side: bottom; range: 0 10 }\n  |axis| { side: left }\n  |line| { fn: `x*x`; samples: 12 }\n]\n",
        );
        assert!(s.contains("<polyline"), "sampled fn polyline: {s}");
        // x² over 0..10 peaks at 100 → the value axis auto-fits to 100.
        assert!(
            s.contains(">100</text>"),
            "value axis fits the sampled data: {s}"
        );
    }

    #[test]
    fn an_area_series_fills_a_polygon() {
        let s = svg("|chart| { categories: \"a\" \"b\" \"c\" } [\n  |area| { data: 3 6 4 }\n]\n");
        assert!(s.contains("<polygon"), "area fill: {s}");
    }

    #[test]
    fn a_log_axis_draws_decade_ticks() {
        let s = svg(
            "|chart| { categories: \"a\" \"b\" } [\n  |axis| { side: left; scale: log }\n  |bars| { data: 10 1000 }\n]\n",
        );
        assert!(s.contains(">100</text>"), "decade tick: {s}");
        assert!(s.contains(">1000</text>"), "decade tick: {s}");
    }

    #[test]
    fn a_log_axis_over_a_non_positive_domain_errors() {
        let e = layout_err(
            "|chart| { categories: \"a\" } [\n  |axis| { side: left; scale: log; range: -1 10 }\n  |bars| { data: 5 }\n]\n",
        );
        assert!(e.contains("domain above 0"), "{e}");
    }

    #[test]
    fn a_smooth_curve_resamples_densely() {
        let s = svg(
            "|chart| { categories: \"a\" \"b\" \"c\" \"d\" } [\n  |line| { data: 1 8 2 6; curve: smooth }\n]\n",
        );
        // The monotone cubic is resampled into a many-point polyline, not 4 segments.
        let pts = s
            .split("<polyline points=\"")
            .nth(1)
            .and_then(|t| t.split('"').next())
            .unwrap_or("");
        assert!(
            pts.split(' ').count() > 20,
            "smooth resamples densely, got {} points",
            pts.split(' ').count()
        );
    }

    #[test]
    fn a_per_band_fn_list_needs_bands() {
        let e = layout_err(
            "|chart| [\n  |axis| { side: bottom; range: 0 1 }\n  |axis| { side: left }\n  |line| { fn: `1` `2` }\n]\n",
        );
        assert!(e.contains("per-band"), "{e}");
    }
}

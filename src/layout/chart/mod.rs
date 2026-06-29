//! `layout: chart` ([CHARTS.md]) — a container that reads all its children, fixes a
//! shared data→pixel scale, and **lowers to primitive `PlacedNode`s**. The renderer,
//! cascade, palette, theming, and `--bake-vars` are all reused unchanged; the chart
//! adds only the scale-and-place algorithm here.
//!
//! `|bars|` / `|line|` / `|dots|` / `|area|` / `|bubble|` over a categorical band or a
//! numeric x, with explicit `|axis|` children, nice/log scales, formulas, bands,
//! annotations, a legend, and a title. Every series lowers through one `Plot::project`,
//! so the `direction: column | row | radial` flip (a radar reusing the cartesian
//! builders) is a projector change, not a rewrite. `layout: pie` is the sibling layout
//! (`pie.rs`). Rich `:hover` tooltips + `fmt` polish are the remaining step (`PLAN.md`).

mod annot;
mod axis;
mod bars;
mod bubble;
mod marks;
mod model;
mod palette;
mod pie;
mod prim;
mod project;
mod radial;
mod scale;

pub(super) use pie::{is_pie, layout_pie};

use crate::error::Error;
use crate::layout::{Bbox, PlacedNode};
use crate::resolve::{AttrMap, NodeKind, ResolvedInst, ResolvedValue};
use model::{Chart, Side};
use project::{Dir, Plot};

pub(super) const TITLE_SIZE: f64 = 13.0;
const AXIS_TITLE_SIZE: f64 = 11.0;
pub(super) const LABEL_SIZE: f64 = 11.0;

/// Is this node a chart container ([CHARTS.md] §2)? Detected by its `layout:` attr —
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
    // A radial (or pie) chart is square; a cartesian one is wide ([CHARTS.md] §2).
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

    // Semantic draw order ([CHARTS.md] §15): gridlines/web behind, then areas, bars,
    // lines, dots, then annotations and labels — pushed in order, so a later child
    // paints over an earlier one. The series builders project through `Plot`, so a
    // radial chart reuses the exact `areas`/`lines`/`dots`/`bars` passes; only the
    // gridlines, labels, and annotations differ by direction.
    let mut kids = Vec::new();
    match plot.dir {
        Dir::Radial => radial::gridlines(&plot, &chart, &mut kids),
        Dir::Column => {
            annot::band_shades(&plot, &chart, &mut kids);
            axis::gridlines(&plot, &chart, &mut kids);
        }
        Dir::Row => axis::gridlines(&plot, &chart, &mut kids),
    }
    marks::areas(&plot, &chart, &mut kids);
    bars::lay_out(&plot, &chart, &mut kids);
    marks::lines(&plot, &chart, &mut kids);
    marks::dots(&plot, &chart, &mut kids);
    bubble::lay_out(&plot, &chart, &mut kids);
    match plot.dir {
        Dir::Radial => radial::labels(&plot, &chart, &mut kids),
        // Bands / annotations are column-oriented today; in a row they are deferred.
        Dir::Row => axis::labels(&plot, &chart, &mut kids),
        Dir::Column => {
            annot::marks(&plot, &chart, &mut kids);
            axis::labels(&plot, &chart, &mut kids);
            annot::band_ticks(&plot, &chart, &mut kids);
        }
    }
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

    Ok(chart_box(inst, w, h, kids))
}

/// The chart container `PlacedNode`: a `Block` of the chart box carrying the lowered
/// primitives as pre-positioned children. Shared by `layout: chart` and `layout: pie`,
/// so both place as one unit and keep the node's id / classes / paint.
pub(super) fn chart_box(inst: &ResolvedInst, w: f64, h: f64, kids: Vec<PlacedNode>) -> PlacedNode {
    PlacedNode {
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
        span: inst.span,
    }
}

/// The plot rect = the chart box inset by the gutters its labels / titles / legend
/// need, all measured at compile time (SPEC §6).
fn plot_rect(chart: &Chart, w: f64, h: f64) -> Plot {
    if chart.dir == Dir::Radial {
        return radial_plot(chart, w, h);
    }
    if chart.dir == Dir::Row {
        return row_plot(chart, w, h);
    }
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
    let band_row = annot::x_band_row(chart);
    Plot {
        x0: -w / 2.0 + left,
        x1: w / 2.0 - right,
        y0: -h / 2.0 + 8.0 + title_h + value_title_h,
        y1: h / 2.0 - 6.0 - LABEL_SIZE * 1.4 - band_row - x_title_h - legend_h,
        dir: chart.dir,
    }
}

/// A row chart's plot rect: the cartesian flip ([CHARTS.md] §11). The domain
/// (category) labels sit on the left and the value labels along the bottom, so the
/// gutters swap — a left gutter sized to the widest category, a bottom gutter for the
/// value labels and axis title.
fn row_plot(chart: &Chart, w: f64, h: f64) -> Plot {
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
    let legend_h = if legend_entries(chart).len() >= 2 {
        LABEL_SIZE * 1.6
    } else {
        0.0
    };
    let left = nonzero(domain_gutter(chart), 12.0);
    Plot {
        x0: -w / 2.0 + left,
        x1: w / 2.0 - 12.0,
        y0: -h / 2.0 + 8.0 + title_h,
        y1: h / 2.0 - 6.0 - LABEL_SIZE * 1.4 - value_title_h - legend_h,
        dir: Dir::Row,
    }
}

/// The left-gutter width for a row chart's domain (category) labels.
fn domain_gutter(chart: &Chart) -> f64 {
    let maxw = chart
        .x
        .labels
        .iter()
        .map(|l| prim::text_width(l, LABEL_SIZE))
        .fold(0.0_f64, f64::max);
    if maxw > 0.0 { maxw + 8.0 } else { 0.0 }
}

/// A radial chart's plot rect: a centred square (the spoke-circle's bounding box),
/// inset from the chart box by the title (top), legend (bottom), and a margin all
/// round for the spoke labels that sit just outside the rim ([CHARTS.md] §12).
fn radial_plot(chart: &Chart, w: f64, h: f64) -> Plot {
    let title_h = if chart.title.is_some() {
        TITLE_SIZE * 1.4
    } else {
        0.0
    };
    let legend_h = if legend_entries(chart).len() >= 2 {
        LABEL_SIZE * 1.6
    } else {
        0.0
    };
    let margin = LABEL_SIZE * 2.0;
    let top = title_h + margin;
    let avail_h = h - top - legend_h - margin;
    let side = avail_h.min(w - 2.0 * margin).max(0.0);
    let cy = -h / 2.0 + top + avail_h / 2.0;
    Plot {
        x0: -side / 2.0,
        x1: side / 2.0,
        y0: cy - side / 2.0,
        y1: cy + side / 2.0,
        dir: Dir::Radial,
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

/// A centred row of swatch + label entries at vertical `cy`. Shared by chart and pie.
pub(super) fn lay_out_legend(
    entries: &[(String, ResolvedValue)],
    cy: f64,
    out: &mut Vec<PlacedNode>,
) {
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
        out.push(prim::rect(x + SW / 2.0, cy, SW, SW, color.clone(), 1.0));
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
    fn a_fn_list_without_bands_reports_the_mismatch() {
        let e = layout_err(
            "|chart| [\n  |axis| { side: bottom; range: 0 1 }\n  |axis| { side: left }\n  |line| { fn: `1` `2` }\n]\n",
        );
        assert!(e.contains("2 formulas"), "{e}");
        assert!(e.contains("0 bands"), "{e}");
    }

    #[test]
    fn a_filled_band_shades_the_plot_and_labels_it() {
        let s = svg(
            "|chart| { categories: \"a\" \"b\" } [\n  |bars| { data: 5 8 }\n  |band| \"zone\" { span: 0 1; fill: --amber }\n]\n",
        );
        // Amber is unused by the palette walk, so it is unambiguously the band.
        assert!(s.contains("var(--lini-amber)"), "band shade tint: {s}");
        assert!(s.contains("opacity"), "the shade is translucent: {s}");
        assert!(s.contains(">zone</text>"), "band tick label: {s}");
    }

    #[test]
    fn an_unfilled_band_draws_a_divider_not_a_shade() {
        let s = svg(
            "|chart| { categories: \"a\" \"b\" \"c\" } [\n  |bars| { data: 5 8 6 }\n  |band| \"L\" { span: 0 1 }\n  |band| \"R\" { span: 1 3 }\n]\n",
        );
        assert!(
            s.contains(">L</text>") && s.contains(">R</text>"),
            "band ticks: {s}"
        );
        assert!(
            !s.contains("opacity"),
            "no shade is drawn for an unfilled band: {s}"
        );
    }

    #[test]
    fn a_segmented_fn_draws_one_polyline_across_the_bands() {
        let s = svg(
            "|chart| [\n  |axis| { side: bottom }\n  |axis| { side: left }\n  |band| { span: 0 1 }\n  |band| { span: 1 2 }\n  |line| { fn: `u` `1-u` }\n]\n",
        );
        assert!(s.contains("<polyline"), "segmented curve polyline: {s}");
    }

    #[test]
    fn a_fn_list_length_must_match_the_band_count() {
        let e = layout_err(
            "|chart| [\n  |axis| { side: bottom }\n  |axis| { side: left }\n  |band| { span: 0 1 }\n  |line| { fn: `1` `2` `3` }\n]\n",
        );
        assert!(e.contains("3 formulas"), "{e}");
        assert!(e.contains("1 bands"), "{e}");
    }

    #[test]
    fn a_mark_draws_a_reference_line_with_its_label() {
        let s = svg(
            "|chart| { categories: \"a\" \"b\" } [\n  |axis#v| { side: left }\n  |bars| { data: 5 8 }\n  |mark| \"max\" { at: 6; axis: v; stroke: --red }\n]\n",
        );
        assert!(
            s.contains("var(--lini-red)"),
            "the reference line is the mark's stroke: {s}"
        );
        assert!(s.contains(">max</text>"), "the mark label: {s}");
    }

    #[test]
    fn a_mark_point_draws_a_dot_and_a_label() {
        let s = svg(
            "|chart| { categories: \"a\" \"b\" } [\n  |axis#v| { side: left }\n  |bars| { data: 5 8 }\n  |mark| \"pt\" { at: 1 6; axis: v }\n]\n",
        );
        assert!(s.contains("<ellipse"), "the point's dot: {s}");
        assert!(s.contains(">pt</text>"), "the point's label: {s}");
    }

    #[test]
    fn marker_none_suppresses_the_point_dot() {
        let s = svg(
            "|chart| { categories: \"a\" \"b\" } [\n  |axis#v| { side: left }\n  |bars| { data: 5 8 }\n  |mark| \"lbl\" { at: 1 6; axis: v; marker: none }\n]\n",
        );
        assert!(s.contains(">lbl</text>"), "the label still draws: {s}");
        assert!(!s.contains("<ellipse"), "no dot when 'marker: none': {s}");
    }

    #[test]
    fn a_mark_needs_an_axis() {
        let e = layout_err(
            "|chart| { categories: \"a\" } [\n  |bars| { data: 5 }\n  |mark| \"x\" { at: 3 }\n]\n",
        );
        assert!(e.contains("needs 'axis:'"), "{e}");
    }

    #[test]
    fn a_mark_at_takes_one_or_two_values() {
        let e = layout_err(
            "|chart| { categories: \"a\" } [\n  |axis#v| { side: left }\n  |bars| { data: 5 }\n  |mark| \"x\" { at: 1 2 3; axis: v }\n]\n",
        );
        assert!(e.contains("one value"), "{e}");
    }

    #[test]
    fn stacked_bars_fit_the_per_category_sum() {
        let s = svg(
            "|chart| { categories: \"a\" \"b\"; bars: stacked } [\n  |bars| { data: 3 4 }\n  |bars| { data: 5 6 }\n]\n",
        );
        // Category b sums to 10, so the value axis reaches a 10 tick (grouped tops out
        // at 6). The 10 proves the stacked envelope drove the domain.
        assert!(
            s.contains(">10</text>"),
            "value axis fits the stack sum: {s}"
        );
    }

    #[test]
    fn overlay_bars_are_translucent() {
        let s = svg(
            "|chart| { categories: \"a\" \"b\"; bars: overlay } [\n  |bars| { data: 3 4 }\n  |bars| { data: 7 6 }\n]\n",
        );
        assert!(s.contains("opacity"), "overlay bars carry an opacity: {s}");
    }

    #[test]
    fn a_radial_line_draws_a_closed_radar_with_spoke_labels() {
        let s = svg(
            "|chart| { direction: radial; categories: \"a\" \"b\" \"c\" } [\n  |axis| { range: 0 5 }\n  |line| { data: 5 3 4 }\n]\n",
        );
        assert!(s.contains("<polyline"), "the radar loop: {s}");
        assert!(s.contains(">a</text>"), "a spoke (category) label: {s}");
    }

    #[test]
    fn radial_bars_draw_wedge_polygons() {
        let s = svg(
            "|chart| { direction: radial; categories: \"a\" \"b\" \"c\" } [\n  |axis| { range: 0 10 }\n  |bars| { data: 8 5 9 }\n]\n",
        );
        assert!(s.contains("<polygon"), "wedge polygons: {s}");
    }

    #[test]
    fn a_side_on_a_radial_axis_errors() {
        let e = layout_err(
            "|chart| { direction: radial; categories: \"a\" \"b\" } [\n  |axis| { side: left; range: 0 5 }\n  |line| { data: 3 4 }\n]\n",
        );
        assert!(e.contains("radial"), "{e}");
    }

    #[test]
    fn a_row_chart_lays_categories_left_and_values_below() {
        let s = svg(
            "|chart| { direction: row; categories: \"a\" \"b\" } [\n  |axis| \"v\" { side: bottom }\n  |bars| { data: 5 10 }\n]\n",
        );
        assert!(s.contains("<rect"), "horizontal bars: {s}");
        assert!(s.contains(">a</text>"), "a category label (left): {s}");
        assert!(s.contains(">10</text>"), "a value tick (below): {s}");
    }

    #[test]
    fn a_row_line_projects_through_the_same_builder() {
        let s = svg(
            "|chart| { direction: row; categories: \"a\" \"b\" \"c\" } [\n  |line| { data: 3 6 4 }\n]\n",
        );
        assert!(
            s.contains("<polyline"),
            "the row line reuses the cartesian builder: {s}"
        );
    }

    #[test]
    fn an_unknown_direction_errors() {
        let e = layout_err(
            "|chart| { direction: sideways; categories: \"a\" } [\n  |bars| { data: 5 }\n]\n",
        );
        assert!(e.contains("column, row, or radial"), "{e}");
    }

    #[test]
    fn a_pie_draws_slice_wedges_and_a_legend() {
        let s =
            svg("|pie| \"T\" [\n  |slice| \"a\" { value: 3 }\n  |slice| \"b\" { value: 1 }\n]\n");
        assert!(s.contains("<polygon"), "slice wedges: {s}");
        assert!(
            s.contains("var(--lini-rose)"),
            "slice 0 walks the palette: {s}"
        );
        assert!(
            s.contains("var(--lini-orange)"),
            "slice 1 walks the palette: {s}"
        );
        assert!(s.contains(">a</text>"), "a legend label: {s}");
    }

    #[test]
    fn a_non_slice_child_of_a_pie_errors() {
        let e = layout_err("|pie| [\n  |bars| { data: 1 }\n]\n");
        assert!(e.contains("'|slice|' only"), "{e}");
    }

    #[test]
    fn an_empty_pie_errors() {
        assert!(layout_err("|pie| \"T\"\n").contains("at least one '|slice|'"));
    }

    #[test]
    fn a_negative_slice_value_errors() {
        let e = layout_err("|pie| [\n  |slice| { value: -1 }\n]\n");
        assert!(e.contains("≥ 0"), "{e}");
    }

    #[test]
    fn a_pie_summing_to_zero_errors() {
        let e = layout_err("|pie| [\n  |slice| { value: 0 }\n  |slice| { value: 0 }\n]\n");
        assert!(e.contains("sum to zero"), "{e}");
    }

    #[test]
    fn a_hole_out_of_range_errors() {
        let e = layout_err("|pie| { hole: 1.5 } [\n  |slice| { value: 1 }\n]\n");
        assert!(e.contains("fraction 0..1"), "{e}");
    }

    #[test]
    fn bubbles_render_as_ovals_with_a_title_floor() {
        let s = svg(
            "|chart| [\n  |axis| { side: bottom }\n  |axis| { side: left }\n  |bubble| \"A\" { at: 1 2; value: 4 }\n  |bubble| \"B\" { at: 3 4; value: 16 }\n]\n",
        );
        assert!(s.contains("<ellipse"), "bubbles are ovals: {s}");
        assert!(
            s.contains("<title>B: 16</title>"),
            "the bubble <title> floor: {s}"
        );
    }

    #[test]
    fn a_bubble_needs_at_and_value() {
        let e = layout_err(
            "|chart| [\n  |axis| { side: bottom }\n  |axis| { side: left }\n  |bubble| \"A\" { at: 1 2 }\n]\n",
        );
        assert!(e.contains("needs 'at:' (x y) and 'value:'"), "{e}");
    }
}

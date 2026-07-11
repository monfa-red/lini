/// Live-mode SVG for a source (palette vars stay `var(--lini-…)`).
fn svg(src: &str) -> String {
    crate::compile_str(src).expect("compile")
}

/// The layout-phase error message for a chart that resolves but won't lay out.
fn layout_err(src: &str) -> String {
    let toks = crate::lexer::lex(src).expect("lex");
    let file = crate::syntax::parser::parse(src, &toks).expect("parse");
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
        "|chart| \"T\" { categories: \"a\", \"b\" } [\n  |bars| \"S1\" { data: 3, 6 }\n  |bars| \"S2\" { data: 4, 2 }\n]\n",
    );
    assert!(s.contains("lini-chart"), "chart container class: {s}");
    // Palette walk: series 0 rose, series 1 teal — red skipped. Bars fill with the
    // soft tier (the outlined look, [SPEC 14.6]).
    assert!(s.contains("var(--lini-rose-soft)"), "series 0 hue: {s}");
    assert!(s.contains("var(--lini-teal-soft)"), "series 1 hue: {s}");
    assert!(!s.contains("var(--lini-red)"), "red is reserved: {s}");
    assert!(s.contains("var(--lini-grid)"), "gridlines: {s}");
    assert!(s.contains("<title>a · S1: 3</title>"), "bar title: {s}");
    assert!(s.contains(">T</text>"), "chart title text: {s}");
}

#[test]
fn a_line_series_draws_a_polyline() {
    let s = svg("|chart| { categories: \"a\", \"b\", \"c\" } [\n  |line| { data: 3, 6, 4 }\n]\n");
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
fn a_bar_radius_rounds_the_rect() {
    // The desugar defaults |bars| to radius 2; an explicit `radius:` overrides it.
    let s = svg("|chart| { categories: \"a\" } [\n  |bars| { data: 5; radius: 6 }\n]\n");
    assert!(
        s.contains("rx=\"6\""),
        "explicit bar radius rounds the rect: {s}"
    );
    let d = svg("|chart| { categories: \"a\" } [\n  |bars| { data: 5 }\n]\n");
    assert!(
        d.contains("rx=\"2\""),
        "the default bar radius rounds the rect: {d}"
    );
}

#[test]
fn a_bar_stroke_draws_an_outline_without_recoloring_the_fill() {
    // A `stroke:` on a fill shape is a separate outline [SPEC 14.6] — it must
    // not become the fill. With no `fill:`, the body stays the palette soft tier
    // (rose) and the stroke is the outline (sky); the old bug made the body sky.
    let s = svg("|chart| { categories: \"a\" } [\n  |bars| { data: 5; stroke: --sky }\n]\n");
    assert!(
        s.contains("var(--lini-rose-soft)"),
        "the fill stays the palette soft tier: {s}"
    );
    assert!(
        s.contains("var(--lini-sky)"),
        "the stroke draws as an outline: {s}"
    );
}

#[test]
fn bars_default_to_an_outlined_look() {
    // A default bar fills with the soft tier and gains a deep edge [SPEC 14.6].
    let s = svg("|chart| { categories: \"a\" } [\n  |bars| { data: 5 }\n]\n");
    assert!(s.contains("var(--lini-rose-soft)"), "soft fill: {s}");
    assert!(s.contains("var(--lini-rose-deep)"), "deep edge: {s}");
}

#[test]
fn a_bar_stroke_none_opts_out_of_the_edge() {
    // `stroke: none` overrides the class `auto` sentinel — a flat bar, no edge.
    let s = svg("|chart| { categories: \"a\" } [\n  |bars| { data: 5; stroke: none }\n]\n");
    assert!(s.contains("var(--lini-rose-soft)"), "soft fill stays: {s}");
    assert!(!s.contains("var(--lini-rose-deep)"), "no deep edge: {s}");
}

#[test]
fn a_slice_stroke_outlines_without_recoloring_the_fill() {
    // The pie bug [SPEC 14.6]: `stroke:` on a slice recoloured its fill and
    // drew no outline. Now slice 0's fill walks the palette soft tier (rose) and the
    // stroke is a separate outline (sky).
    let s = svg(
        "|pie| [\n  |slice| \"a\" { value: 1; stroke: --sky }\n  |slice| \"b\" { value: 1 }\n]\n",
    );
    assert!(
        s.contains("var(--lini-rose-soft)"),
        "slice 0 fill stays the palette soft walk: {s}"
    );
    assert!(
        s.contains("var(--lini-sky)"),
        "slice 0 stroke draws as an outline: {s}"
    );
}

#[test]
fn the_chart_gap_tunes_the_title_inset() {
    // `gap:` sets the title→plot space [SPEC 14.6], so different gaps shift the
    // plot geometry; the default (10) is set on the .lini-chart class at desugar.
    let tight = svg("|chart| \"T\" { categories: \"a\"; gap: 0 } [\n  |bars| { data: 5 }\n]\n");
    let loose = svg("|chart| \"T\" { categories: \"a\"; gap: 60 } [\n  |bars| { data: 5 }\n]\n");
    assert_ne!(
        tight, loose,
        "the chart 'gap' changes the title / plot spacing"
    );
}

#[test]
fn a_dual_axis_chart_binds_series_by_id() {
    let s = svg(
        "|chart| { categories: \"a\", \"b\" } [\n  |axis#n| { side: left }\n  |axis#p| { side: right }\n  |bars| { data: 10, 20; axis: n }\n  |line| { data: 4, 9; axis: p }\n]\n",
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
    let e = layout_err("|chart| { categories: \"a\", \"b\" } [\n  |bars| { data: 1, 2, 3 }\n]\n");
    assert!(e.contains("3 values but the chart has 2 categories"), "{e}");
}

#[test]
fn data_and_fn_together_error() {
    let e = layout_err("|chart| { categories: \"a\" } [\n  |bars| { data: 1; fn: (2) }\n]\n");
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
        "|chart| [\n  |axis| { side: bottom; range: 0 10 }\n  |axis| { side: left }\n  |line| { fn: (x*x); samples: 12 }\n]\n",
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
    let s = svg("|chart| { categories: \"a\", \"b\", \"c\" } [\n  |area| { data: 3, 6, 4 }\n]\n");
    assert!(s.contains("<polygon"), "area fill: {s}");
}

#[test]
fn a_log_axis_draws_decade_ticks() {
    let s = svg(
        "|chart| { categories: \"a\", \"b\" } [\n  |axis| { side: left; scale: log }\n  |bars| { data: 10, 1000 }\n]\n",
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
        "|chart| { categories: \"a\", \"b\", \"c\", \"d\" } [\n  |line| { data: 1, 8, 2, 6; curve: smooth }\n]\n",
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
        "|chart| [\n  |axis| { side: bottom; range: 0 1 }\n  |axis| { side: left }\n  |line| { fn: (1), (2) }\n]\n",
    );
    assert!(e.contains("2 formulas"), "{e}");
    assert!(e.contains("0 bands"), "{e}");
}

#[test]
fn a_filled_band_shades_the_plot_and_labels_it() {
    let s = svg(
        "|chart| { categories: \"a\", \"b\" } [\n  |bars| { data: 5, 8 }\n  |band| \"zone\" { span: 0 1; fill: --amber }\n]\n",
    );
    // Amber is unused by the palette walk, so it is unambiguously the band.
    assert!(s.contains("var(--lini-amber)"), "band shade tint: {s}");
    assert!(s.contains("opacity"), "the shade is translucent: {s}");
    assert!(s.contains(">zone</text>"), "band tick label: {s}");
}

#[test]
fn an_unfilled_band_draws_a_divider_not_a_shade() {
    let s = svg(
        "|chart| { categories: \"a\", \"b\", \"c\" } [\n  |bars| { data: 5, 8, 6 }\n  |band| \"L\" { span: 0 1 }\n  |band| \"R\" { span: 1 3 }\n]\n",
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
        "|chart| [\n  |axis| { side: bottom }\n  |axis| { side: left }\n  |band| { span: 0 1 }\n  |band| { span: 1 2 }\n  |line| { fn: (u), (1-u) }\n]\n",
    );
    assert!(s.contains("<polyline"), "segmented curve polyline: {s}");
}

#[test]
fn a_fn_list_length_must_match_the_band_count() {
    let e = layout_err(
        "|chart| [\n  |axis| { side: bottom }\n  |axis| { side: left }\n  |band| { span: 0 1 }\n  |line| { fn: (1), (2), (3) }\n]\n",
    );
    assert!(e.contains("3 formulas"), "{e}");
    assert!(e.contains("1 bands"), "{e}");
}

#[test]
fn a_mark_draws_a_reference_line_with_its_label() {
    let s = svg(
        "|chart| { categories: \"a\", \"b\" } [\n  |axis#v| { side: left }\n  |bars| { data: 5, 8 }\n  |mark| \"max\" { at: 6; axis: v; stroke: --red }\n]\n",
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
        "|chart| { categories: \"a\", \"b\" } [\n  |axis#v| { side: left }\n  |bars| { data: 5, 8 }\n  |mark| \"pt\" { at: 1 6; axis: v }\n]\n",
    );
    assert!(s.contains("<ellipse"), "the point's dot: {s}");
    assert!(s.contains(">pt</text>"), "the point's label: {s}");
}

#[test]
fn marker_none_suppresses_the_point_dot() {
    let s = svg(
        "|chart| { categories: \"a\", \"b\" } [\n  |axis#v| { side: left }\n  |bars| { data: 5, 8 }\n  |mark| \"lbl\" { at: 1 6; axis: v; marker: none }\n]\n",
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
        "|chart| { categories: \"a\", \"b\"; bars: stacked } [\n  |bars| { data: 3, 4 }\n  |bars| { data: 5, 6 }\n]\n",
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
        "|chart| { categories: \"a\", \"b\"; bars: overlay } [\n  |bars| { data: 3, 4 }\n  |bars| { data: 7, 6 }\n]\n",
    );
    assert!(s.contains("opacity"), "overlay bars carry an opacity: {s}");
}

#[test]
fn a_radial_line_draws_a_closed_radar_with_spoke_labels() {
    let s = svg(
        "|chart| { direction: radial; categories: \"a\", \"b\", \"c\" } [\n  |axis| { range: 0 5 }\n  |line| { data: 5, 3, 4 }\n]\n",
    );
    assert!(s.contains("<polyline"), "the radar loop: {s}");
    assert!(s.contains(">a</text>"), "a spoke (category) label: {s}");
}

#[test]
fn radial_bars_draw_wedge_polygons() {
    let s = svg(
        "|chart| { direction: radial; categories: \"a\", \"b\", \"c\" } [\n  |axis| { range: 0 10 }\n  |bars| { data: 8, 5, 9 }\n]\n",
    );
    assert!(s.contains("<polygon"), "wedge polygons: {s}");
}

#[test]
fn a_side_on_a_radial_axis_errors() {
    let e = layout_err(
        "|chart| { direction: radial; categories: \"a\", \"b\" } [\n  |axis| { side: left; range: 0 5 }\n  |line| { data: 3, 4 }\n]\n",
    );
    assert!(e.contains("radial"), "{e}");
}

#[test]
fn a_row_chart_lays_categories_left_and_values_below() {
    let s = svg(
        "|chart| { direction: row; categories: \"a\", \"b\" } [\n  |axis| \"v\" { side: bottom }\n  |bars| { data: 5, 10 }\n]\n",
    );
    assert!(s.contains("<rect"), "horizontal bars: {s}");
    assert!(s.contains(">a</text>"), "a category label (left): {s}");
    assert!(s.contains(">10</text>"), "a value tick (below): {s}");
}

#[test]
fn a_row_line_projects_through_the_same_builder() {
    let s = svg(
        "|chart| { direction: row; categories: \"a\", \"b\", \"c\" } [\n  |line| { data: 3, 6, 4 }\n]\n",
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
    let s = svg("|pie| \"T\" [\n  |slice| \"a\" { value: 3 }\n  |slice| \"b\" { value: 1 }\n]\n");
    assert!(s.contains("<polygon"), "slice wedges: {s}");
    assert!(
        s.contains("var(--lini-rose-soft)"),
        "slice 0 walks the palette (soft): {s}"
    );
    assert!(
        s.contains("var(--lini-teal-soft)"),
        "slice 1 walks the palette (soft): {s}"
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

#[test]
fn auto_tooltips_add_a_hover_card_over_the_title_floor() {
    // The default mode is `auto`: the <title> floor plus the live hover card.
    let s = svg("|chart| { categories: \"a\" } [\n  |bars| { data: 5 }\n]\n");
    assert!(s.contains("lini-chart-tip"), "the hover card: {s}");
    assert!(
        s.contains("<title>a: 5</title>"),
        "the title floor stays: {s}"
    );
    assert!(
        s.contains(":hover ~ .lini-tip-0"),
        "the reveal rule links the mark to its card: {s}"
    );
    assert!(s.contains("lini-hit-0"), "the hovered mark is tagged: {s}");
}

#[test]
fn tooltip_none_drops_the_floor() {
    let s = svg("|chart| { categories: \"a\"; tooltip: none } [\n  |bars| { data: 5 }\n]\n");
    assert!(!s.contains("<title>"), "no title floor: {s}");
    assert!(!s.contains("lini-chart-tip"), "no card: {s}");
}

#[test]
fn tags_draw_inline_labels_under_auto() {
    // A series' `tags:` show on the plot (default auto) as `.lini-chart-label` text,
    // over the hover card the value still rides.
    let s = svg(
        "|chart| { categories: \"a\", \"b\" } [\n  |line| { data: 3, 6; tags: \"lo\", \"hi\" }\n]\n",
    );
    assert!(s.contains("lini-chart-label"), "inline label class: {s}");
    assert!(
        s.contains(">lo</text>") && s.contains(">hi</text>"),
        "tag text: {s}"
    );
    assert!(
        s.contains("pointer-events: none"),
        "inline labels pass hover through: {s}"
    );
}

#[test]
fn tooltip_hover_keeps_tags_off_the_plot() {
    // `tooltip: hover` keeps the card (bars are hit targets) but draws no inline label,
    // even with tags.
    let s = svg(
        "|chart| { categories: \"a\", \"b\"; tooltip: hover } [\n  |bars| { data: 3, 6; tags: \"lo\", \"hi\" }\n]\n",
    );
    assert!(!s.contains("lini-chart-label"), "no inline label: {s}");
    assert!(s.contains("lini-chart-tip"), "the hover card stays: {s}");
}

#[test]
fn a_series_tooltip_overrides_the_chart_default() {
    // The chart says hover (no inline); the series opts back into always.
    let s = svg(
        "|chart| { categories: \"a\", \"b\"; tooltip: hover } [\n  |line| { data: 3, 6; tags: \"lo\", \"hi\"; tooltip: always }\n]\n",
    );
    assert!(
        s.contains("lini-chart-label"),
        "series override shows inline: {s}"
    );
}

#[test]
fn an_arrow_marker_on_a_series_errors() {
    let e = layout_err(
        "|chart| { categories: \"a\", \"b\" } [\n  |line| { data: 3, 6; marker: arrow }\n]\n",
    );
    assert!(e.contains("no centred form"), "{e}");
    assert!(e.contains("dot, circle, or diamond"), "{e}");
}

#[test]
fn a_circle_marker_is_bigger_than_a_dot() {
    // A line vertex `circle` is a hover-sized point; `dot` stays small.
    let c =
        svg("|chart| { categories: \"a\", \"b\" } [\n  |line| { data: 3, 6; marker: circle }\n]\n");
    let d =
        svg("|chart| { categories: \"a\", \"b\" } [\n  |line| { data: 3, 6; marker: dot }\n]\n");
    assert!(c.contains("rx=\"5.5\""), "circle marker radius: {c}");
    assert!(d.contains("rx=\"2.5\""), "dot marker radius: {d}");
}

#[test]
fn a_diamond_marker_draws_a_rhombus() {
    let s = svg(
        "|chart| { categories: \"a\", \"b\" } [\n  |line| { data: 3, 6; marker: diamond }\n]\n",
    );
    assert!(s.contains("<polygon"), "diamond marker is a polygon: {s}");
}

#[test]
fn data_text_is_normal_weight_chrome_is_bold() {
    // The diagram-wide default is bold; a chart keeps it for the title and legend but
    // states `normal` for its axis ticks (and tags) [SPEC 14.6].
    let s = svg(
        "|chart| \"Cost\" { categories: \"a\", \"b\" } [\n  |bars| \"A\" { data: 5, 8 }\n  |bars| \"B\" { data: 3, 4 }\n]\n",
    );
    assert!(
        s.contains("font-size: 13px; font-weight: bold\">Cost</text>"),
        "title bold: {s}"
    );
    assert!(
        s.contains("font-size: 11px; font-weight: bold\">A</text>"),
        "legend bold: {s}"
    );
    assert!(
        s.contains("font-size: 11px; font-weight: normal\">a</text>"),
        "axis tick normal: {s}"
    );
}

#[test]
fn tags_count_must_match_the_data() {
    let e = layout_err(
        "|chart| { categories: \"a\", \"b\" } [\n  |line| { data: 3, 6; tags: \"only\" }\n]\n",
    );
    assert!(e.contains("1 labels but the series has 2"), "{e}");
}

#[test]
fn tags_on_a_fn_series_error() {
    let e = layout_err(
        "|chart| [\n  |axis| { side: bottom; range: 0 10 }\n  |axis| { side: left }\n  |line| { fn: (x); tags: \"a\", \"b\" }\n]\n",
    );
    assert!(e.contains("needs explicit 'data'"), "{e}");
}

#[test]
fn a_legacy_space_data_list_errors_with_the_comma_spelling() {
    // The 0.21 comma law [SPEC 2/20]: `data: 9 15 24` is the pre-law spelling.
    let e = layout_err("|chart| [\n  |bars| { data: 9 15 24 }\n]\n");
    assert!(
        e.contains("'data' takes comma-separated values — 'data: 9, 15, 24'"),
        "{e}"
    );
}

#[test]
fn a_lone_space_pair_is_one_point_never_two_values() {
    // `data: 10 20` [SPEC 2]: one `x y` point — it draws a dot, not two bars.
    let s = svg("|chart| [\n  |axis| { side: bottom }\n  |dots| { data: 10 20 }\n]\n");
    assert!(s.contains("lini-chart"), "{s}");
    let e = layout_err("|chart| { categories: \"a\", \"b\" } [\n  |bars| { data: 10 20 }\n]\n");
    assert!(e.contains("not 'x y' points"), "{e}");
}

/// Live-mode SVG for a source (palette vars stay `var(--lini-…)`).
fn svg(src: &str) -> String {
    crate::compile_str(src).expect("compile")
}

/// The layout-phase error message for a chart that resolves but won't lay out.
use crate::testutil::layout_err;

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

/// Each series type's SVG element [SPEC 14]: the mark a series lowers to,
/// one row per type, column and radial alike.
#[test]
fn every_series_draws_its_element() {
    for (what, src, wants) in [
        (
            "line",
            "|chart| { categories: \"a\", \"b\", \"c\" } [\n  |line| { data: 3, 6, 4 }\n]\n",
            &["<polyline"][..],
        ),
        (
            "dots over points",
            "|chart| [\n  |axis| { side: bottom }\n  |axis| { side: left }\n  |dots| { data: 1 5, 2 3, 3 8 }\n]\n",
            &["<ellipse"],
        ),
        (
            "area",
            "|chart| { categories: \"a\", \"b\", \"c\" } [\n  |area| { data: 3, 6, 4 }\n]\n",
            &["<polygon"],
        ),
        // Radial: the radar loop closes and the spokes wear their category labels.
        (
            "radial line",
            "|chart| { direction: radial; categories: \"a\", \"b\", \"c\" } [\n  |axis| { range: 0 5 }\n  |line| { data: 5, 3, 4 }\n]\n",
            &["<polyline", ">a</text>"],
        ),
        (
            "radial bars",
            "|chart| { direction: radial; categories: \"a\", \"b\", \"c\" } [\n  |axis| { range: 0 10 }\n  |bars| { data: 8, 5, 9 }\n]\n",
            &["<polygon"],
        ),
    ] {
        let s = svg(src);
        for want in wants {
            assert!(s.contains(want), "a {what} series must draw {want:?}: {s}");
        }
    }
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

/// `baseline:` on an `|area|` [SPEC 14.2]: the fill closes on that value
/// rather than zero, so raising the baseline lifts the polygon's foot.
#[test]
fn an_area_baseline_lifts_the_fill_foot() {
    let foot = |src: &str| {
        let s = svg(src);
        let at = s.find("<polygon points=\"").expect("the area fill");
        let pts = &s[at + 17..];
        let pts = &pts[..pts.find('"').unwrap()];
        pts.split_whitespace()
            .filter_map(|t| t.split(',').nth(1))
            .filter_map(|y| y.parse::<f64>().ok())
            .fold(f64::MIN, f64::max)
    };
    let zero =
        foot("|chart| { categories: \"a\", \"b\", \"c\" } [\n  |area| { data: 4, 8, 6 }\n]\n");
    let raised = foot(
        "|chart| { categories: \"a\", \"b\", \"c\" } [\n  |area| { data: 4, 8, 6; baseline: 2 }\n]\n",
    );
    assert!(
        raised < zero - 1.0,
        "baseline: 2 lifts the foot off zero: {raised} vs {zero}"
    );
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
fn a_filled_band_shades_the_plot_and_labels_it() {
    let s = svg(
        "|chart| { categories: \"a\", \"b\" } [\n  |bars| { data: 5, 8 }\n  |band| \"zone\" { range: 0 1; fill: --amber }\n]\n",
    );
    // Amber is unused by the palette walk, so it is unambiguously the band.
    assert!(s.contains("var(--lini-amber)"), "band shade tint: {s}");
    assert!(s.contains("opacity"), "the shade is translucent: {s}");
    assert!(s.contains(">zone</text>"), "band tick label: {s}");
}

#[test]
fn an_unfilled_band_draws_a_divider_not_a_shade() {
    let s = svg(
        "|chart| { categories: \"a\", \"b\", \"c\" } [\n  |bars| { data: 5, 8, 6 }\n  |band| \"L\" { range: 0 1 }\n  |band| \"R\" { range: 1 3 }\n]\n",
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
        "|chart| [\n  |axis| { side: bottom }\n  |axis| { side: left }\n  |band| { range: 0 1 }\n  |band| { range: 1 2 }\n  |line| { fn: (u), (1-u) }\n]\n",
    );
    assert!(s.contains("<polyline"), "segmented curve polyline: {s}");
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
fn labels_draw_inline_under_auto() {
    // A series' `labels:` show on the plot (default auto) as `.lini-chart-label` text,
    // over the hover card the value still rides.
    let s = svg(
        "|chart| { categories: \"a\", \"b\" } [\n  |line| { data: 3, 6; labels: \"lo\", \"hi\" }\n]\n",
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
    // even with labels.
    let s = svg(
        "|chart| { categories: \"a\", \"b\"; tooltip: hover } [\n  |bars| { data: 3, 6; labels: \"lo\", \"hi\" }\n]\n",
    );
    assert!(!s.contains("lini-chart-label"), "no inline label: {s}");
    assert!(s.contains("lini-chart-tip"), "the hover card stays: {s}");
}

#[test]
fn a_series_tooltip_overrides_the_chart_default() {
    // The chart says hover (no inline); the series opts back into always.
    let s = svg(
        "|chart| { categories: \"a\", \"b\"; tooltip: hover } [\n  |line| { data: 3, 6; labels: \"lo\", \"hi\"; tooltip: always }\n]\n",
    );
    assert!(
        s.contains("lini-chart-label"),
        "series override shows inline: {s}"
    );
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
fn data_text_is_normal_weight_chrome_is_semibold() {
    // Chrome reads semibold [SPEC 14.6]: the title through its own
    // `.lini-chart-title` rule (14px/600, nothing inlined), the legend inline
    // (semibold emits as CSS 600); data text — axis ticks, labels — states
    // `normal` so the numbers never shout.
    let s = svg(
        "|chart| \"Cost\" { categories: \"a\", \"b\" } [\n  |bars| \"A\" { data: 5, 8 }\n  |bars| \"B\" { data: 3, 4 }\n]\n",
    );
    assert!(
        s.contains(" .lini-chart-title { font-size: 14px; font-weight: 600; }"),
        "title rule: {s}"
    );
    assert!(
        s.contains("<text class=\"lini-text lini-chart-title\" x=\"0\"")
            && s.contains(">Cost</text>"),
        "title classed, no inline font: {s}"
    );
    assert!(
        s.contains("font-size: 11px; font-weight: 600\">A</text>"),
        "legend semibold: {s}"
    );
    assert!(
        s.contains("font-size: 11px; font-weight: normal\">a</text>"),
        "axis tick normal: {s}"
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

#[test]
fn row_bands_and_marks_flip_with_the_direction() {
    // [SPEC 14.5/14.7]: in a row chart the value axis runs along the bottom, so
    // a value-bound band shades a vertical strip and a value-bound mark draws a
    // vertical dashed line — same declarations, flipped projection.
    let s = svg(
        "|chart| { direction: row; categories: \"a\", \"b\" } [\n  |axis#v| { side: bottom; range: 0 20 }\n  |bars| { data: 12, 8 }\n  |band| \"hot\" { range: 10 15; axis: v; fill: --amber }\n  |mark| \"target\" { at: 10; axis: v; stroke-style: dashed }\n]\n",
    );
    // The band's wash: full plot height, x-span 10..15 — a rect taller than wide.
    let band = s.find("opacity: 0.15").expect("the band wash");
    let after = &s[band..];
    let rect = &after[..after.find("/>").unwrap()];
    let dim = |k: &str| {
        let i = rect.find(k).unwrap() + k.len() + 2;
        rect[i..i + rect[i..].find('"').unwrap()]
            .parse::<f64>()
            .unwrap()
    };
    assert!(
        dim("height") > dim("width"),
        "a row value-band is a vertical strip: {rect}"
    );
    // The mark's reference line: vertical (both endpoints share one x).
    let mark = s.find("stroke-dasharray: 5").expect("the dashed mark line");
    let after = &s[mark..];
    let line = &after[after.find("<line ").unwrap()..];
    let line = &line[..line.find("/>").unwrap()];
    let coord = |k: &str| {
        let i = line.find(k).unwrap() + k.len() + 2;
        line[i..i + line[i..].find('"').unwrap()]
            .parse::<f64>()
            .unwrap()
    };
    assert!(
        (coord("x1") - coord("x2")).abs() < 1e-9 && coord("y1") != coord("y2"),
        "vertical reference line: {line}"
    );
    assert!(s.contains(">target<"), "the mark label draws: {s}");
    assert!(s.contains(">hot<"), "the band tick draws: {s}");
}

#[test]
fn a_series_tooltip_none_strips_its_titles() {
    // [SPEC 14.8]: only `tooltip: none` strips the native <title> floor — and
    // a series' own mode overrides the chart's.
    let s = svg("|chart| { categories: \"a\" } [\n  |bars| { data: 3; tooltip: none }\n]\n");
    assert!(!s.contains("<title>"), "no hover floor: {s}");
    let s = svg("|chart| { categories: \"a\" } [\n  |bars| { data: 3 }\n]\n");
    assert!(s.contains("<title>"), "the default keeps it: {s}");
}

#[test]
fn a_row_chart_with_a_numeric_domain_draws_its_domain_ticks() {
    // The row projection used to gate domain labels on a categorical scale,
    // so a numeric x lost its ticks, gridlines and title entirely.
    let s = svg("|chart| { direction: row } [\n  |line| { data: 1 5, 2 3, 4 8 }\n]\n");
    for tick in [">1</text>", ">2</text>", ">4</text>"] {
        assert!(s.contains(tick), "domain tick {tick}: {s}");
    }
}

#[test]
fn a_log_axis_reverses_like_a_linear_one() {
    // [SPEC 14.4]: `range: a b` with a > b reverses unconditionally.
    let s = svg(
        "|chart| [\n  |axis| { side: left; scale: log; range: 1000 1 }\n  |line| { data: 1 5, 2 500, 3 900 }\n]\n",
    );
    let y_of = |label: &str| -> f64 {
        let needle = format!(">{label}</text>");
        let i = s
            .find(&needle)
            .unwrap_or_else(|| panic!("tick {label}: {s}"));
        let head = &s[..i];
        let y = head.rfind(" y=\"").expect("a y attr") + 4;
        head[y..head[y..].find('"').map(|e| y + e).expect("closing quote")]
            .parse()
            .expect("a number")
    };
    assert!(
        y_of("1000") > y_of("10"),
        "1000 sits at the bottom when the range runs high to low"
    );
}

#[test]
fn a_log_domain_axis_keeps_its_gridlines() {
    // Default x gridlines were an allow-list of linear — a log (or time) x
    // silently lost its grid.
    let s = svg(
        "|chart| [\n  |axis| { side: left; gridlines: none }\n  |axis#x| { side: bottom; scale: log }\n  |dots| { data: 1 5, 100 3, 1000 8 }\n]\n",
    );
    assert!(
        s.contains("var(--lini-grid)"),
        "the domain grid stands: {s}"
    );
}

#[test]
fn a_bands_old_span_points_at_range() {
    // The one-time migration pointer [SPEC 14.5]: a band's extent renamed to
    // `range:` — the axis's own interval shape.
    let e = crate::lint_str(
        "|chart| { categories: \"a\" } [\n  |bars| { data: 5 }\n  |band| \"z\" { span: 0 1 }\n]\n",
    )
    .expect("parse")
    .iter()
    .map(|d| d.message.clone())
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        e.contains("a band's extent is 'range: a b'"),
        "the migration pointer: {e}"
    );
}

// ── Every chart diagnostic, one row per refusal [SPEC 14/17] ──

/// The chart engine's whole refusal surface, in one table: a source the
/// engine must reject and the substrings its message must carry. Following
/// `dim_errors_speak_spec` — a diagnostic is a contract with the author, so
/// each row pins the words, not just the failure.
#[test]
fn chart_errors_speak_spec() {
    for (src, wants) in [
        ("|chart| \"T\"\n", &["at least one series"][..]),
        (
            "|chart| { categories: \"a\", \"b\" } [\n  |bars| { data: 1, 2, 3 }\n]\n",
            &["3 values but the chart has 2 categories"],
        ),
        (
            "|chart| { categories: \"a\" } [\n  |bars| { data: 1; fn: (2) }\n]\n",
            &["not both"],
        ),
        ("|chart| [\n  |box| \"x\"\n]\n", &["series"]),
        (
            "|chart| { categories: \"a\" } [\n  |axis#v| { side: left }\n  |line| { data: 1; axis: nope }\n]\n",
            &["axis 'nope' not found", "'v'"],
        ),
        (
            "|chart| { categories: \"a\" } [\n  |axis| { side: left; scale: log; range: -1 10 }\n  |bars| { data: 5 }\n]\n",
            &["domain above 0"],
        ),
        (
            "|chart| [\n  |axis| { side: bottom; range: 0 1 }\n  |axis| { side: left }\n  |line| { fn: (1), (2) }\n]\n",
            &["2 formulas", "0 bands"],
        ),
        (
            "|chart| [\n  |axis| { side: bottom }\n  |axis| { side: left }\n  |band| { range: 0 1 }\n  |line| { fn: (1), (2), (3) }\n]\n",
            &["3 formulas", "1 bands"],
        ),
        (
            "|chart| { categories: \"a\" } [\n  |bars| { data: 5 }\n  |mark| \"x\" { at: 3 }\n]\n",
            &["needs 'axis:'"],
        ),
        (
            "|chart| { categories: \"a\" } [\n  |axis#v| { side: left }\n  |bars| { data: 5 }\n  |mark| \"x\" { at: 1 2 3; axis: v }\n]\n",
            &["one value"],
        ),
        (
            "|chart| { direction: radial; categories: \"a\", \"b\" } [\n  |axis| { side: left; range: 0 5 }\n  |line| { data: 3, 4 }\n]\n",
            &["radial"],
        ),
        (
            "|chart| { direction: sideways; categories: \"a\" } [\n  |bars| { data: 5 }\n]\n",
            &["column, row, or radial"],
        ),
        (
            "|chart| [\n  |axis| { side: bottom }\n  |axis| { side: left }\n  |bubble| \"A\" { at: 1 2 }\n]\n",
            &["needs 'at:' (x y) and 'value:'"],
        ),
        (
            "|chart| { categories: \"a\", \"b\" } [\n  |line| { data: 3, 6; marker: arrow }\n]\n",
            &["no centred form", "dot, circle, or diamond"],
        ),
        (
            "|chart| { categories: \"a\", \"b\" } [\n  |line| { data: 3, 6; labels: \"only\" }\n]\n",
            &["1 entries but the series has 2"],
        ),
        (
            "|chart| [\n  |axis| { side: bottom; range: 0 10 }\n  |axis| { side: left }\n  |line| { fn: (x); labels: \"a\", \"b\" }\n]\n",
            &["needs explicit 'data'"],
        ),
        // The 0.21 comma law [SPEC 2/20]: `data: 9 15 24` is the pre-law spelling.
        (
            "|chart| [\n  |bars| { data: 9 15 24 }\n]\n",
            &["'data' takes comma-separated values — 'data: 9, 15, 24'"],
        ),
        // The radial flip is never silently lossy [SPEC 14.7/20].
        (
            "|chart| { direction: radial; categories: \"a\", \"b\", \"c\" } [\n  |line| { data: 1, 2, 3 }\n  |mark| \"x\" { at: 2 }\n]\n",
            &["a radial chart draws no bands / marks yet — remove it or change 'direction'"],
        ),
        // `format:` [SPEC 14.4/16] — a date preset needs a time axis, and a
        // misspelling names the usage.
        (
            "|chart| [\n|axis| { side: left; format: month }\n|bars| { data: 1, 2 }\n]\n",
            &["a date preset reads a time axis"],
        ),
        (
            "|chart| { format: decimals } [ |bars| { data: 1 } ]\n",
            &["'format' takes auto"],
        ),
        // Per-datum paint lists [SPEC 14.6]: one paint per datum, on a
        // per-datum shape, over explicit data.
        (
            "|chart| [\n|bars| { data: 9, 15, 24; fill: auto, --red }\n]\n",
            &["'fill' lists 2 paints but the series has 3 data points"],
        ),
        (
            "|chart| [\n|line| { data: 9, 15; stroke: red, blue }\n]\n",
            &["one shape with one paint"],
        ),
        (
            "|chart| [\n|bars| { fn: (x); fill: auto, --red }\n]\n",
            &["needs explicit 'data'"],
        ),
        // Time axes [SPEC 14.3/14.4].
        (
            "|chart| [\n|axis| { side: bottom; step: 5 }\n|line| { data: \"2026-01-01\" 1, \"2026-06-01\" 2 }\n]\n",
            &["steps by calendar"],
        ),
        (
            "|chart| [\n|line| { data: \"2026-01-01\" 1, \"2026-06-01\" 2 }\n|dots| { data: 3 4, 5 6 }\n]\n",
            &["mixes dates and numbers"],
        ),
        (
            "|chart| [\n|line| { data: \"2026-13-01\" 1, \"2026-06-01\" 2 }\n]\n",
            &["'2026-13-01' is not a date"],
        ),
        (
            "|chart| [\n|axis| { side: left; scale: time }\n|bars| { data: 1, 2 }\n]\n",
            &["a value axis is numeric"],
        ),
    ] {
        let e = layout_err(src);
        for want in wants {
            assert!(e.contains(want), "{src:?}\n  wanted {want:?}, got {e:?}");
        }
    }
}

/// The pie's own refusals [SPEC 14.6] — the same table, one scope over.
#[test]
fn pie_errors_speak_spec() {
    for (src, want) in [
        ("|pie| \"T\"\n", "at least one '|slice|'"),
        ("|pie| [\n  |bars| { data: 1 }\n]\n", "'|slice|' only"),
        ("|pie| [\n  |slice| { value: -1 }\n]\n", "≥ 0"),
        (
            "|pie| [\n  |slice| { value: 0 }\n  |slice| { value: 0 }\n]\n",
            "sum to zero",
        ),
        (
            "|pie| { hole: 1.5 } [\n  |slice| { value: 1 }\n]\n",
            "fraction 0..1",
        ),
    ] {
        let e = layout_err(src);
        assert!(e.contains(want), "{src:?}\n  wanted {want:?}, got {e:?}");
    }
}

/// **The out-of-scope type gate** [SPEC 21], the schematic family's twin: every
/// chart type is an error outside its layout, reported by the type the author
/// wrote — and legal the moment its scope encloses it.
#[test]
fn every_chart_type_is_gated_and_named_as_written() {
    for (part, message) in [
        (
            "|bars| { data: 1, 2 }",
            "'|bars|' is a chart series — it belongs in a 'layout: chart'",
        ),
        (
            "|dots| { data: 1, 2 }",
            "'|dots|' is a chart series — it belongs in a 'layout: chart'",
        ),
        (
            "|area| { data: 1, 2 }",
            "'|area|' is a chart series — it belongs in a 'layout: chart'",
        ),
        (
            "|bubble| { at: 1 2; value: 3 }",
            "'|bubble|' is a chart series — it belongs in a 'layout: chart'",
        ),
        ("|axis| \"x\"", "'|axis|' belongs in a 'layout: chart'"),
        (
            "|band| { range: 0 1 }",
            "'|band|' belongs in a 'layout: chart'",
        ),
        ("|mark| { at: 1 }", "'|mark|' belongs in a 'layout: chart'"),
    ] {
        // Bare on the canvas, and two ordinary containers deeper — the gate is
        // carried down the walk, not read off the parent. (Inside a chart or a
        // pie the type exists; what it may sit *among* is that layout's own
        // reading — `|bars|` in a pie is "a pie's children are '|slice|' only".)
        for src in [
            format!("{part}\n"),
            format!("|group#g| [\n  {part}\n]\n"),
            format!("|group#g| [\n  |row#r| [\n    {part}\n  ]\n]\n"),
        ] {
            assert_eq!(layout_err(&src), message, "{src}");
        }
    }
    // A `|slice|` belongs in a pie — including inside a chart, which is not it.
    for src in [
        "|slice| { value: 1 }\n",
        "|group#g| [\n  |slice| { value: 1 }\n]\n",
        "|chart| [\n  |slice| { value: 1 }\n]\n",
    ] {
        assert_eq!(
            layout_err(src),
            "'|slice|' belongs in a 'layout: pie'",
            "{src}"
        );
    }
    // …and each is legal in its own scope.
    for src in [
        "|chart| { categories: \"a\", \"b\" } [\n  |bars| { data: 1, 2 }\n  |axis#v| { side: left }\n  |band| { range: 0 1 }\n  |mark| { axis: v; at: 1 }\n]\n",
        "|chart| [\n  |axis#v| { side: left }\n  |bubble| { at: 1 2; value: 3 }\n]\n",
        "|pie| [\n  |slice| { value: 1 }\n]\n",
    ] {
        crate::layout::layout(&crate::testutil::program(src))
            .unwrap_or_else(|e| panic!("{src}: {}", e.message));
    }
    // A define over a chart type reports the name its author wrote.
    assert_eq!(
        layout_err("{ |revenue::bars| { } }\n|revenue| { data: 1, 2 }\n"),
        "'|revenue|' is a chart series — it belongs in a 'layout: chart'"
    );
    // `|line|` is the core primitive [SPEC 7] — never gated.
    crate::layout::layout(&crate::testutil::program("|line| { points: 0 0, 10 10 }\n"))
        .expect("a standalone line");
}

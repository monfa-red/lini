use super::*;

// ── `format:` on ticks & tooltips [SPEC 14.4/16, CHART-DRAW Stage 1] ──

#[test]
fn format_percent_formats_value_axis_ticks() {
    let svg = render_live(
        "|chart| [\n|axis| { side: left; range: 0 1; format: percent 0 }\n|bars| { data: 0.25, 0.5, 1 }\n]\n",
    );
    assert!(svg.contains(">100%<"), "formatted tick missing:\n{svg}");
    assert!(svg.contains(">20%<"), "formatted tick missing:\n{svg}");
}

#[test]
fn format_inherits_from_the_chart_and_reaches_titles() {
    // Chart-level `format:` defaults the axis ticks and the bar `<title>` values.
    let svg = render_live("|chart| { format: decimal 1 } [ |bars| \"S\" { data: 2, 4 } ]\n");
    assert!(svg.contains(">4.0<"), "formatted tick missing:\n{svg}");
    assert!(svg.contains("S: 4.0"), "formatted title missing:\n{svg}");
}

#[test]
fn format_date_preset_errors_off_a_time_axis() {
    let err = lini::compile_str(
        "|chart| [\n|axis| { side: left; format: month }\n|bars| { data: 1, 2 }\n]\n",
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("a date preset reads a time axis"),
        "got: {err}"
    );
}

#[test]
fn format_bad_value_errors_with_the_usage() {
    let err =
        lini::compile_str("|chart| { format: decimals } [ |bars| { data: 1 } ]\n").unwrap_err();
    assert!(
        err.to_string().contains("'format' takes auto"),
        "got: {err}"
    );
}

// ── Per-datum paint [SPEC 14.6, CHART-DRAW Stage 2] ──

#[test]
fn per_datum_fill_list_highlights_one_bar() {
    let svg =
        render_live("|chart| [\n|bars| { data: 9, 15, 24; fill: auto, auto, --red-soft }\n]\n");
    // Two bars keep the palette walk's first hue, the third takes the listed paint.
    assert_eq!(svg.matches("var(--lini-rose-soft)").count(), 2, "{svg}");
    assert!(
        svg.contains("var(--lini-red-deep)"),
        "the listed fill deepens its own edge:\n{svg}"
    );
    assert!(svg.contains("--lini-red-soft"), "{svg}");
}

#[test]
fn per_datum_stroke_auto_deepens_each_datums_own_fill() {
    let svg = render_live(
        "|chart| [\n|bars| { data: 9, 15; fill: auto, --red-soft; stroke: auto, auto }\n]\n",
    );
    // Each datum's edge is the deep tier of its *own* fill.
    assert!(svg.contains("--lini-rose-deep"), "{svg}");
    assert!(svg.contains("--lini-red-deep"), "{svg}");
}

#[test]
fn per_datum_list_count_mismatch_errors_with_both_counts() {
    let err = lini::compile_str("|chart| [\n|bars| { data: 9, 15, 24; fill: auto, --red }\n]\n")
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("'fill' lists 2 paints but the series has 3 data points"),
        "got: {err}"
    );
}

#[test]
fn per_datum_list_on_a_line_errors() {
    let err =
        lini::compile_str("|chart| [\n|line| { data: 9, 15; stroke: red, blue }\n]\n").unwrap_err();
    assert!(
        err.to_string().contains("one shape with one paint"),
        "got: {err}"
    );
}

#[test]
fn per_datum_list_with_fn_errors() {
    let err =
        lini::compile_str("|chart| [\n|bars| { fn: (x); fill: auto, --red }\n]\n").unwrap_err();
    assert!(
        err.to_string().contains("needs explicit 'data'"),
        "got: {err}"
    );
}

#[test]
fn per_datum_fill_list_reaches_dots() {
    let svg = render_live("|chart| [\n|dots| { data: 1 2, 3 4; fill: --blue-ink, auto }\n]\n");
    assert!(svg.contains("--lini-blue-ink"), "{svg}");
}

// ── Time axes [SPEC 14.3/14.4, CHART-DRAW Stage 3] ──

#[test]
fn time_ticks_across_zoomy_domains() {
    // One chart per span — minutes → hours → days → months → years; the tick
    // text (unit choice + calendar boundaries + rendering) is pinned whole.
    let spans = [
        ("minutes", "2026-03-04T09:07", "2026-03-04T09:26"),
        ("hours", "2026-03-04T03:12", "2026-03-04T21:40"),
        ("days", "2026-03-04", "2026-03-09T12:00"),
        ("weeks", "2026-03-04", "2026-04-20"),
        ("months", "2026-01-15", "2026-11-02"),
        ("years", "2019-06-01", "2026-02-01"),
        ("decades", "1985-01-01", "2026-01-01"),
    ];
    let mut out = String::new();
    for (name, a, b) in spans {
        let src = format!("|chart| [\n|line| {{ data: \"{a}\" 1, \"{b}\" 2 }}\n]\n");
        let svg = render_live(&src);
        out.push_str(&format!("{name}: {}\n", x_tick_texts(&svg).join(" | ")));
    }
    insta::assert_snapshot!(out);
}

#[test]
fn calendar_step_overrides_the_auto_unit() {
    let svg = render_live(
        "|chart| [\n|axis#t| { side: bottom; step: 2 month }\n|line| { data: \"2026-01-10\" 1, \"2026-07-20\" 2 }\n]\n",
    );
    let ticks = x_tick_texts(&svg);
    assert!(
        ticks.iter().any(|t| t == "Feb 2026") && ticks.iter().any(|t| t == "Apr 2026"),
        "{ticks:?}"
    );
    assert!(!ticks.iter().any(|t| t == "Mar 2026"), "{ticks:?}");
}

#[test]
fn time_axis_error_rows() {
    // Numeric step on a time axis.
    let err = lini::compile_str(
        "|chart| [\n|axis| { side: bottom; step: 5 }\n|line| { data: \"2026-01-01\" 1, \"2026-06-01\" 2 }\n]\n",
    )
    .unwrap_err();
    assert!(err.to_string().contains("steps by calendar"), "got: {err}");
    // Mixed date/numeric series.
    let err = lini::compile_str(
        "|chart| [\n|line| { data: \"2026-01-01\" 1, \"2026-06-01\" 2 }\n|dots| { data: 3 4, 5 6 }\n]\n",
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("mixes dates and numbers"),
        "got: {err}"
    );
    // An invalid date carries the literal.
    let err =
        lini::compile_str("|chart| [\n|line| { data: \"2026-13-01\" 1, \"2026-06-01\" 2 }\n]\n")
            .unwrap_err();
    assert!(
        err.to_string().contains("'2026-13-01' is not a date"),
        "got: {err}"
    );
    // scale: time belongs to the x axis.
    let err = lini::compile_str(
        "|chart| [\n|axis| { side: left; scale: time }\n|bars| { data: 1, 2 }\n]\n",
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("a value axis is numeric"),
        "got: {err}"
    );
}

#[test]
fn time_axis_date_preset_and_range_and_hover() {
    let svg = render_live(
        "|chart| [\n|axis#t| { side: bottom; format: year; range: \"2024-01-01\" \"2027-01-01\" }\n|line| \"S\" { data: \"2024-06-01\" 1, \"2026-06-01\" 2; marker: circle }\n]\n",
    );
    let ticks = x_tick_texts(&svg);
    assert!(ticks.iter().any(|t| t == "2025"), "{ticks:?}");
    // The authored date preset wins everywhere — ticks and hover alike.
    assert!(svg.contains("S: 2026, 2"), "{svg}");
    // Without a preset, hover shows the full instant.
    let svg = render_live(
        "|chart| [\n|line| \"S\" { data: \"2026-01-01\" 1, \"2026-06-01T09:30\" 2; marker: circle }\n]\n",
    );
    assert!(svg.contains("S: Jun 1 2026, 09:30, 2"), "{svg}");
}

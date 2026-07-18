use super::super::testutil::{by_id, laid, layout_err, text_at, texts};
use crate::ledger::consts::DIM_CLEARANCE;
use crate::resolve::{MarkerKind, NodeKind, ResolvedValue};

/// A bottom row's painted band [SPEC 15.6]: text reach `fs + 2` above the
/// line, extension overshoot 3 below — what a next row stands `clearance`
/// off.
const BAND_NEG: f64 = 14.0;
const BAND_POS: f64 = 3.0;

// ── Linear dims & chains [SPEC 15.6] ──

#[test]
fn a_chain_shares_one_row_and_the_next_dim_packs_outside() {
    // Plate −75..75, holes at −50 and 10, at 2 px/unit: hops 25 · 60 · 65
    // all fit their spans and share one row; the overall 150 overlaps
    // them and stands `clearance` off their band.
    let l = laid(
        "{ layout: drawing; scale: 2; density: 1 }\n|rect#plate| { width: 150; height: 40 }\n|hole#a| { width: 8; translate: -50 0 }\n|hole#b| { width: 8; translate: 10 0 }\nplate:left (-) a (-) b (-) plate:right { side: bottom }\nplate:left (-) plate:right { side: bottom }\n",
    );
    let (_, y25, _) = text_at(&l.nodes, "25");
    let (_, y60, _) = text_at(&l.nodes, "60");
    let (_, y65, _) = text_at(&l.nodes, "65");
    let (_, y150, _) = text_at(&l.nodes, "150");
    assert!(
        (y25 - y60).abs() < 1e-6 && (y60 - y65).abs() < 1e-6,
        "hops share the row: {y25} / {y60} / {y65}"
    );
    // The next row's text reach clears the first band by the clearance:
    // line-to-line = band + clearance + text reach.
    let pitch = BAND_POS + DIM_CLEARANCE + BAND_NEG;
    assert!(
        (y150 - y60 - pitch).abs() < 0.01,
        "the 150 stands clearance off the first band: {y150} vs {y60}"
    );
    // First row: value text `clearance` off the plate's paint extent (41),
    // the line `BAND_NEG` past that — text centre lifted 8, half-height ~6.
    assert!(
        (y60 - (41.0 + DIM_CLEARANCE + BAND_NEG - 7.5)).abs() < 0.6,
        "y60={y60}"
    );
}

#[test]
fn clearance_cascades_and_a_per_dim_value_is_honored_independently() {
    let first_row_y = |src: &str| text_at(&laid(src).nodes, "40").1;
    let geometry = "|rect#a| { width: 40; height: 20 }\n";
    let dim = "a:left (-) a:right { side: bottom }\n";
    // The drawing default: text stands DIM_CLEARANCE off the extent (y = 11).
    let base = first_row_y(&format!(
        "{{ layout: drawing; density: 1 }}\n{geometry}{dim}"
    ));
    // The scope's own `clearance:` (scene config) moves the row out…
    let scoped = first_row_y(&format!(
        "{{ layout: drawing; density: 1; clearance: 10 }}\n{geometry}{dim}"
    ));
    // …so does a `(-)` family rule…
    let ruled = first_row_y(&format!(
        "{{ layout: drawing; density: 1;\n  (-) {{ clearance: 10 }}\n}}\n{geometry}{dim}"
    ));
    // …and the dim's own block.
    let owned = first_row_y(&format!(
        "{{ layout: drawing; density: 1 }}\n{geometry}a:left (-) a:right {{ side: bottom; clearance: 10 }}\n"
    ));
    assert!((base - (11.0 + DIM_CLEARANCE + BAND_NEG - 7.5)).abs() < 0.6);
    for (who, y) in [("scope", scoped), ("rule", ruled), ("block", owned)] {
        assert!(
            (y - base - (10.0 - DIM_CLEARANCE)).abs() < 0.01,
            "{who} clearance moves the row: {y} vs {base}"
        );
    }
    // Independent: a widened dim leaves its sibling at the default seat, and
    // the sibling's second row stands off the widened band it packs past.
    let two = laid(
        "{ layout: drawing; density: 1 }\n|rect#a| { width: 40; height: 20 }\na:left (-) a:right { side: bottom; clearance: 10 }\na:left (-) a:right { side: bottom }\n",
    );
    let rows: Vec<f64> = texts(&two.nodes)
        .iter()
        .filter(|(t, ..)| t == "40")
        .map(|(_, _, y, _)| *y)
        .collect();
    assert!(
        (rows[0] - (11.0 + 10.0 + BAND_NEG - 7.5)).abs() < 0.6,
        "the widened dim stands 10 off: {rows:?}"
    );
    assert!(
        (rows[1] - rows[0] - (BAND_POS + DIM_CLEARANCE + BAND_NEG)).abs() < 0.01,
        "the default dim packs its own clearance off the first band: {rows:?}"
    );
}

#[test]
fn no_annotation_text_lands_on_another_across_the_drawing_samples() {
    // The packing oracle [SPEC 15.6]: a row stands `clearance` off everything
    // painted, so no dim value may overlap any other annotation text —
    // another row's, a callout's, an angle's — in any drawing sample.
    use crate::layout::ir::Bbox;
    fn collect(nodes: &[crate::layout::PlacedNode], ox: f64, oy: f64, out: &mut Vec<Bbox>) {
        for n in nodes {
            if n.type_chain.iter().any(|t| t == "dim-text") {
                out.push(Bbox::extent_of(std::slice::from_ref(n), |_| true).shifted(ox, oy));
            }
            collect(&n.children, ox + n.cx, oy + n.cy, out);
        }
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("samples");
    let mut seen = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("lini") {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        if !src.contains("drawing") {
            continue;
        }
        let mut boxes = Vec::new();
        collect(&laid(&src).nodes, 0.0, 0.0, &mut boxes);
        seen += 1;
        for (i, a) in boxes.iter().enumerate() {
            for b in &boxes[i + 1..] {
                assert!(
                    !a.inflate(-0.5).overlaps(b.inflate(-0.5)),
                    "{}: annotation texts overlap: {a:?} vs {b:?}",
                    path.display()
                );
            }
        }
    }
    assert!(seen >= 6, "the drawing samples compiled: {seen}");
}

#[test]
fn gap_on_a_dimension_points_at_clearance() {
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|rect#a| { width: 40; height: 20 }\na:left (-) a:right { gap: 30 }\n"
        ),
        "a dimension stands off by 'clearance' — 'gap' is a mate's separation"
    );
    // The `(o)` reading is a dimension too.
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|hole#h| { width: 12 }\nh:top (o) { gap: 30 }\n"
        ),
        "a dimension stands off by 'clearance' — 'gap' is a mate's separation"
    );
}

#[test]
fn iso_text_turns_with_a_vertical_dim() {
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#a| { width: 60; height: 40 }\na:top (-) a:bottom { side: right }\n",
    );
    let (x, _, rot) = text_at(&l.nodes, "40");
    assert_eq!(rot, -90.0, "reads from the right");
    let a = by_id(&l.nodes, "a");
    assert!(x > a.cx + 30.0, "stacked right of the geometry: x={x}");
}

#[test]
fn a_narrow_span_flips_arrows_out_but_keeps_a_fitting_value_inside() {
    // A 20-wide span can't hold text + arrows, but the bare "20" still
    // reads between the extension lines: arrows flip out, value centred
    // inside — drafting's middle form [SPEC 15.6].
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#a| { width: 20; height: 10 }\na:left (-) a:right { side: bottom }\n",
    );
    let (x, _, _) = text_at(&l.nodes, "20");
    assert!(x.abs() < 1e-6, "value centred inside the span: x={x}");
}

#[test]
fn a_span_too_tight_for_its_value_slides_the_text_past() {
    // A 10-wide span can't even hold "10" — the text slides rightward,
    // past the higher-u extension line.
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#a| { width: 10; height: 8 }\na:left (-) a:right { side: bottom }\n",
    );
    let (x, _, _) = text_at(&l.nodes, "10");
    assert!(x > 5.0, "text outside the span: x={x}");
}

#[test]
fn corner_anchors_on_one_edge_pull_the_dim_there() {
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#a| { width: 40; height: 20 }\n|rect#b| { width: 40; height: 20; translate: 70 0 }\na:top-left (-) b:top-right\n",
    );
    let (_, y, _) = text_at(&l.nodes, "110");
    let a = by_id(&l.nodes, "a");
    assert!(y < a.cy - 10.0, "pulled to the top: y={y}");
}

#[test]
fn a_two_ended_label_replaces_the_number() {
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#a| { width: 40; height: 20 }\na:left (-) a:right \"180\"\n",
    );
    text_at(&l.nodes, "180");
    assert!(
        !texts(&l.nodes).iter().any(|(t, ..)| t == "40"),
        "the measured 40 is replaced"
    );
}

#[test]
fn measured_values_read_bare_pre_scale_numbers() {
    // `unit:` is the semantic quantity only [SPEC 15.1]: no per-value suffix
    // (drafting states units once, in the title block), and the value is
    // pre-scale — 40 units at `unit: mm` reads `40`, whatever the density.
    let l = laid(
        "{ layout: drawing; unit: mm }\n|rect#a| { width: 40; height: 20 }\n|hole#h| { width: 12 }\na:left (-) a:right { side: bottom }\nh (o)\n",
    );
    text_at(&l.nodes, "40");
    text_at(&l.nodes, "⌀12");
}

#[test]
fn dim_errors_speak_spec() {
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|rect#a| { width: 40; height: 20 }\n|rect#b| { width: 40; height: 20 }\na:left (-) b:top\n"
        ),
        "'a:left (-) b:top' — perpendicular faces have no shared normal; the angle between edges is '(<)'"
    );
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|rect#a| { width: 40; height: 20 }\na:left (-) a:right { side: left }\n"
        ),
        "a horizontal dimension stacks on top or bottom"
    );
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|rect#a| { width: 40; height: 20 }\na:top (-) a:bottom { side: top }\n"
        ),
        "a vertical dimension stacks on left or right"
    );
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|rect#a| { width: 40; height: 20 }\na:left (-) a:right { tol: \"x\" }\n"
        ),
        "'tol' takes a number, '+upper -lower', or a fit ident"
    );
}

// ── Axis inference, `project:` & aligned dims [SPEC 15.6] ──

#[test]
fn two_point_anchors_read_the_true_aligned_distance() {
    // A 80 × 60 right triangle: corner anchors are point anchors, so the dim
    // reads the aligned 100 along the hypotenuse, ISO text turned with it.
    let l = laid(
        "{ layout: drawing; density: 1 }\n|sketch#t| { draw: move(0, 0) right(80) up(60) close() }\nt:bottom-left (-) t:top-right\n",
    );
    let (x, y, rot) = text_at(&l.nodes, "100");
    assert!((rot - -36.87).abs() < 0.01, "turned with the span: {rot}");
    // The hypotenuse runs bottom-left → top-right; its midpoint ties with the
    // bbox centre, so the dim falls to the ISO-above side — up-left, outside
    // the material.
    let t = by_id(&l.nodes, "t");
    let mid = (t.cx + 40.0, t.cy - 30.0);
    assert!(
        0.6 * (x - mid.0) + 0.8 * (y - mid.1) < -10.0,
        "stands off the up-left side: ({x}, {y}) vs {mid:?}"
    );
}

#[test]
fn a_point_and_a_directed_anchor_read_along_the_normal() {
    // `plate:left` directs the axis horizontal; the diagonal hole offset
    // projects flat — 75 + 10, never the aligned distance.
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#plate| { width: 150; height: 70 }\n|hole#h| { width: 10; translate: 10 15 }\nplate:left (-) h { side: bottom }\n",
    );
    text_at(&l.nodes, "85");
}

#[test]
fn project_overrides_the_point_readings() {
    let base = "{ layout: drawing; density: 1 }\n|sketch#t| { draw: move(0, 0) right(80) up(60) close() }\n";
    let h = laid(&format!(
        "{base}t:bottom-left (-) t:top-right {{ project: horizontal }}\n"
    ));
    text_at(&h.nodes, "80");
    let v = laid(&format!(
        "{base}t:bottom-left (-) t:top-right {{ project: vertical }}\n"
    ));
    text_at(&v.nodes, "60");
    // `aligned` confirms the default point reading.
    let a = laid(&format!(
        "{base}t:bottom-left (-) t:top-right {{ project: aligned }}\n"
    ));
    text_at(&a.nodes, "100");
}

#[test]
fn project_and_axis_errors_speak_spec() {
    // `project:` must agree with a directed anchor [SPEC 20].
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|rect#a| { width: 40; height: 20 }\na:left (-) a:right { project: vertical }\n"
        ),
        "'project: vertical' conflicts with 'a:left' — the directed anchor reads horizontal"
    );
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|rect#a| { width: 40; height: 20 }\na:left (-) a:right { project: sideways }\n"
        ),
        "'project' takes horizontal, vertical, or aligned"
    );
    // An agreeing override is a no-op, not a conflict.
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#a| { width: 40; height: 20 }\na:left (-) a:right { project: horizontal }\n",
    );
    text_at(&l.nodes, "40");
}

#[test]
fn an_aligned_dim_takes_side_left_or_right_along_its_span() {
    // Walking bottom-left → top-right (up-right), the walker's left is the
    // up-left side, right the down-right side.
    let base = "{ layout: drawing; density: 1 }\n|sketch#t| { draw: move(0, 0) right(80) up(60) close() }\n";
    let left = laid(&format!(
        "{base}t:bottom-left (-) t:top-right {{ side: left }}\n"
    ));
    let right = laid(&format!(
        "{base}t:bottom-left (-) t:top-right {{ side: right }}\n"
    ));
    let t = by_id(&left.nodes, "t");
    let mid = (t.cx + 40.0, t.cy - 30.0);
    let side_of = |l: &crate::layout::LaidOut| {
        let (x, y, _) = text_at(&l.nodes, "100");
        0.6 * (x - mid.0) + 0.8 * (y - mid.1)
    };
    assert!(side_of(&left) < 0.0, "left = the up-left side");
    assert!(side_of(&right) > 0.0, "right = the down-right side");
    assert_eq!(
        layout_err(&format!(
            "{base}t:bottom-left (-) t:top-right {{ side: top }}\n"
        )),
        "an aligned dimension sits left or right of its span — read along it, first anchor to second"
    );
}

// ── Pattern-copy addressing [SPEC 15.4] ──

#[test]
fn a_copy_index_measures_that_copy_grid_row_major_radial_clockwise() {
    // Grid(2, 2, 100, 30) from the seed at (−50, −15): row-major, so copy 2
    // is (50, −15) and copy 3 is (−50, 15).
    let grid = "{ layout: drawing; density: 1 }\n|rect#plate| { width: 150; height: 70 } [\n  |hole#bolt| { width: 10; translate: -50 -15; pattern: grid(2, 2, 100, 30) }\n]\n";
    let l = laid(&format!(
        "{grid}plate:left (-) plate.bolt.2 {{ side: bottom }}\n"
    ));
    text_at(&l.nodes, "125");
    let l = laid(&format!(
        "{grid}plate:top (-) plate.bolt.3 {{ side: left }}\n"
    ));
    text_at(&l.nodes, "50");
    // Radial copies run clockwise from bearing 0 — copy 2 sits at 90°, the
    // ring's right; no `N×` prefix on an indexed copy.
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#plate| { width: 120; height: 80 } [\n  |hole#b| { width: 8; pattern: radial(4, 25) }\n]\nplate:left (-) plate.b.2 { side: bottom }\n",
    );
    text_at(&l.nodes, "85");
}

#[test]
fn copy_index_errors_carry_the_count() {
    let grid = "{ layout: drawing; density: 1 }\n|rect#plate| { width: 150; height: 70 } [\n  |hole#bolt| { width: 10; translate: -50 -15; pattern: grid(2, 2, 100, 30) }\n]\n";
    assert_eq!(
        layout_err(&format!("{grid}plate:left (-) plate.bolt.5\n")),
        "no copy 'bolt.5' — the pattern places 4"
    );
    // 1-based: copy 0 is the same unknown-index error.
    assert_eq!(
        layout_err(&format!("{grid}plate:left (-) plate.bolt.0\n")),
        "no copy 'bolt.0' — the pattern places 4"
    );
    // An index needs a pattern to pick from.
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|rect#plate| { width: 150; height: 70 } [\n  |hole#d| { width: 6 }\n]\nplate:left (-) plate.d.2\n"
        ),
        "'d' has no 'pattern:' — a numeric segment picks a pattern copy"
    );
}

#[test]
fn a_copy_dim_measures_model_truth_through_a_break() {
    // Copies at model x −80 / 0 / 80 on a broken bar: the dim to copy 3
    // still reads the true 180, while the displayed drawing is compressed.
    let src = "{ layout: drawing; density: 1 }\n|sketch#bar| { draw: move(-100, 0) up(10) right(200) down(10); revolve: x-axis; break: 20 60 } [\n  |hole#m| { width: 8; translate: -80 0; pattern: grid(3, 1, 80, 0) }\n]\nbar:left (-) bar.m.3 { side: bottom }\n";
    let l = laid(src);
    text_at(&l.nodes, "180");
    let bar = by_id(&l.nodes, "bar");
    assert!(
        bar.bbox.w() < 180.0,
        "the displayed bar is compressed: {}",
        bar.bbox.w()
    );
}

// ── Tolerances [SPEC 15.6] ──

#[test]
fn tol_composes_its_three_forms() {
    let sym = laid(
        "{ layout: drawing; density: 1 }\n|rect#a| { width: 40; height: 20 }\na:left (-) a:right { tol: 0.1 }\n",
    );
    text_at(&sym.nodes, "40±0.1");

    let fit = laid(
        "{ layout: drawing; density: 1 }\n|rect#a| { width: 40; height: 20 }\na:left (-) a:right { tol: H7 }\n",
    );
    text_at(&fit.nodes, "40 H7");

    // Stacked deviations: raised / lowered beside the value, 0.7 × font.
    let dev = laid(
        "{ layout: drawing; density: 1 }\n|rect#a| { width: 40; height: 20 }\na:left (-) a:right { tol: +0.2 -0.05 }\n",
    );
    let (_, yu, _) = text_at(&dev.nodes, "+0.2");
    let (_, yl, _) = text_at(&dev.nodes, "-0.05");
    let (_, ym, _) = text_at(&dev.nodes, "40");
    assert!(
        yu < ym && ym < yl,
        "raised {yu} / value {ym} / lowered {yl}"
    );
}

// ── `format:` on dimensions [SPEC 15.6/16, CHART-DRAW Stage 8] ──

#[test]
fn an_unformatted_dim_keeps_the_drafting_two_decimal_trim() {
    // The `auto` default is the historic ≤ 2-decimals trim, byte-identical.
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#a| { width: 40.456; height: 20 }\na:left (-) a:right { side: bottom }\n",
    );
    text_at(&l.nodes, "40.46");
}

#[test]
fn format_shapes_the_number_and_the_rest_composes_around_it() {
    // Decision 2's order: `N×` count + `⌀` glyph + formatted number +
    // following label + `tol:` — `format:` touches the number alone.
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#plate| { width: 120; height: 60 } [\n  |hole#pin| { width: 10; translate: -35 0; pattern: grid(2, 1, 70, 0) }\n]\nplate.pin (o) \"H7\" { format: decimal 1; tol: 0.1 }\n",
    );
    text_at(&l.nodes, "2× ⌀10.0 H7±0.1");
}

#[test]
fn format_cascades_scope_rule_and_the_dims_block() {
    let geometry = "|rect#a| { width: 40; height: 20 }\n";
    let dim = "a:left (-) a:right { side: bottom }\n";
    // The drawing scope's config…
    let scoped = laid(&format!(
        "{{ layout: drawing; density: 1; format: decimal 1 }}\n{geometry}{dim}"
    ));
    text_at(&scoped.nodes, "40.0");
    // …a `(-)` family rule…
    let ruled = laid(&format!(
        "{{ layout: drawing; density: 1;\n  (-) {{ format: decimal 1 }}\n}}\n{geometry}{dim}"
    ));
    text_at(&ruled.nodes, "40.0");
    // …and the dim's own block wins over the rule.
    let owned = laid(&format!(
        "{{ layout: drawing; density: 1;\n  (-) {{ format: decimal 1 }}\n}}\n{geometry}a:left (-) a:right {{ side: bottom; format: decimal 3 }}\n"
    ));
    text_at(&owned.nodes, "40.000");
}

#[test]
fn a_fraction_dim_stacks_numerator_over_denominator() {
    // `fraction D` rides the raised / lowered run machinery [SPEC 15.6]:
    // whole leading, raised numerator, slash, lowered denominator.
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#a| { width: 40.375; height: 20 }\na:left (-) a:right { side: bottom; format: fraction 8 }\n",
    );
    let (xw, yw, _) = text_at(&l.nodes, "40");
    let (xn, yn, _) = text_at(&l.nodes, "3");
    let (xs, ys, _) = text_at(&l.nodes, "/");
    let (xd, yd, _) = text_at(&l.nodes, "8");
    assert!(
        xw < xn && xn < xs && xs < xd,
        "runs read left to right: {xw} / {xn} / {xs} / {xd}"
    );
    assert!(
        yn < yw && yn < ys && yd > yw && yd > ys,
        "numerator raised, denominator lowered: {yn} / {yw} / {ys} / {yd}"
    );
}

#[test]
fn a_date_preset_on_a_dimension_errors() {
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|rect#a| { width: 40; height: 20 }\na:left (-) a:right { format: month }\n"
        ),
        "a date preset reads a time axis"
    );
}

#[test]
fn a_fraction_stack_cannot_hold_stacked_deviations() {
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|rect#a| { width: 40.375; height: 20 }\na:left (-) a:right { format: fraction 8; tol: +0.2 -0.05 }\n"
        ),
        "'tol: +upper -lower' stacks where 'format: fraction' already stacks — use a numeric 'tol' or a decimal format"
    );
}

// ── The `(o)` readings [SPEC 15.6] ──

#[test]
fn a_named_arc_reads_its_radius() {
    let l = laid(
        "{ layout: drawing; scale: 2; density: 1 }\n|sketch#s| { draw: move(0, 0) right(30) fillet(3):r1 up(20) left(30) down(20) close() }\ns:r1 (o)\n",
    );
    text_at(&l.nodes, "R3");
}

#[test]
fn a_circle_segment_reads_its_diameter() {
    let l = laid(
        "{ layout: drawing; density: 1 }\n|sketch#s| { draw: move(0, 0) right(40) up(20) left(40) close() move(20, -10) circle(5):c }\ns:c (o)\n",
    );
    text_at(&l.nodes, "⌀10");
}

#[test]
fn a_bare_round_node_leaders_its_diameter_with_the_copy_count() {
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#plate| { width: 120; height: 60 } [\n  |hole#pin| { width: 10; translate: -35 0; pattern: grid(2, 1, 70, 0) }\n]\nplate.pin (o) \"H7\"\n",
    );
    text_at(&l.nodes, "2× ⌀10 H7");
}

#[test]
fn a_side_anchor_on_a_round_node_draws_the_diametral_line() {
    // The value doesn't fit inside ⌀16 — the line overruns the anchored
    // rim and the text spills upward, turned with the vertical line.
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#plate| { width: 80; height: 40 }\n|hole#eye| { width: 16 }\neye:top (o)\n",
    );
    let (_, y, rot) = text_at(&l.nodes, "⌀16");
    assert_eq!(rot, -90.0, "turned with the line");
    assert!(y < -8.0, "spills past the top rim: y={y}");
}

#[test]
fn a_side_anchor_on_any_node_spans_to_the_opposite_side() {
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#bore| { width: 60; height: 16 }\nbore:top (o) { side: right }\n",
    );
    let (x, _, _) = text_at(&l.nodes, "⌀16");
    assert!(x > 30.0, "stacked on the right: x={x}");
}

#[test]
fn a_revolved_name_spans_its_station_across_the_axis() {
    let l = laid(
        "{ layout: drawing; scale: 2; density: 1 }\n|sketch#bar| { draw: move(-150, 0) up(10) right(40):thread right(260) down(10); revolve: x-axis }\nbar:thread (o) { side: left; tol: h6 }\n",
    );
    text_at(&l.nodes, "⌀20 h6");
}

#[test]
fn a_station_diameter_requires_a_revolve() {
    // A merely mirrored profile's span is a width, not a diameter
    // [SPEC 15.6] — the reading asks for the revolve.
    assert_eq!(
        layout_err(
            "{ layout: drawing; scale: 2; density: 1 }\n|sketch#bar| { draw: move(-150, 0) up(10) right(40):thread right(260) down(10); mirror: x-axis }\nbar:thread (o) { side: left }\n"
        ),
        "a station '⌀' reads a revolved profile — 'revolve: x-axis'"
    );
}

#[test]
fn a_bare_round_measure_needs_an_axis() {
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|rect#block| { width: 40; height: 20 }\nblock (o)\n"
        ),
        "'(o)' can't pick an axis on 'block' — anchor a side ('block:top (o)') or a segment"
    );
}

// ── `(<)` — the angle [SPEC 15.6] ──

#[test]
fn an_angle_reads_two_edges_and_rides_its_arc() {
    // rise 120 over run 160 → atan = 36.87°.
    let l = laid(
        "{ layout: drawing; scale: 2; density: 1 }\n|sketch#g| { draw: move(-40, 30) right(80):base up(60) line(-80, 60):flank close() }\ng:flank (<) g:base\n",
    );
    text_at(&l.nodes, "36.87°");
}

#[test]
fn a_unary_angle_measures_the_included_taper() {
    // A 10-in-40 taper mirrored about x: included angle = 2 · atan(10/40)
    // = 28.07°.
    let l = laid(
        "{ layout: drawing; density: 1 }\n|sketch#cone| { draw: move(0, 0) line(40, -10):taper; mirror: x-axis }\ncone:taper (<)\n",
    );
    text_at(&l.nodes, "28.07°");
}

#[test]
fn angle_errors_speak_spec() {
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|oval#a| { width: 20; height: 20 }\n|oval#b| { width: 20; height: 20 }\na (<) b\n"
        ),
        "an angle reads two edges — a named segment, a '|line|', or a side"
    );
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|sketch#s| { draw: move(0, 0) line(40, -10):taper up(10) close() }\ns:taper (<)\n"
        ),
        "'(<)' on ':taper' needs 'mirror:' — no twin to measure against"
    );
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|rect#a| { width: 40; height: 20 }\n|rect#b| { width: 40; height: 20 }\na:top (<) b:bottom\n"
        ),
        "the angle's edges are parallel — they never meet"
    );
}

// ── Leaders [SPEC 15.7] ──

/// The slender arrowhead's exact tip — its poly's first point
/// (`dims::arrow` builds tip-first); word leaders carry it since the
/// ISO 129 unification [SPEC 15.7].
fn arrow_tip(nodes: &[crate::layout::PlacedNode]) -> (f64, f64) {
    nodes
        .iter()
        .find(|n| n.type_chain.iter().any(|t| t == "marker-dim"))
        .map(|n| {
            crate::layout::primitives::attr_points(&n.attrs, "points", n.span)
                .unwrap()
                .unwrap()[0]
        })
        .expect("a slender arrowhead")
}

#[test]
fn a_leader_tip_ray_casts_onto_the_outline_with_a_landing_elbow() {
    let l = laid(
        "{ layout: drawing; density: 1 }\n|oval#disc| { width: 40; height: 40 }\ndisc:top-right <- \"THRU\"\n",
    );
    let line = l
        .nodes
        .iter()
        .find(|n| n.type_chain.iter().any(|t| t == "dim-line"))
        .expect("the leader line");
    let pts = crate::layout::primitives::attr_points(&line.attrs, "points", line.span)
        .unwrap()
        .unwrap();
    assert_eq!(pts.len(), 3, "tip, elbow, landing");
    // The slender arrowhead's own tip sits on the circle (r = 20), not
    // the bbox corner — the line stops a stub short of it [SPEC 15.6/7].
    let tip = arrow_tip(&l.nodes);
    assert!(
        (tip.0.hypot(tip.1) - 20.0).abs() < 0.75,
        "tip on the rim: {tip:?}"
    );
    // The landing is horizontal.
    assert!((pts[1].1 - pts[2].1).abs() < 1e-9, "horizontal landing");
    // Text past the landing.
    let (tx, ty, _) = text_at(&l.nodes, "THRU");
    assert!(tx > pts[2].0, "text past the landing");
    assert!((ty - pts[2].1).abs() < 1e-6, "text rides the landing");
}

#[test]
fn a_word_leader_tips_the_rim_of_a_patterned_hole() {
    // The carrier's ray-cast recurses into a copy — the copy must not
    // still look like a carrier (the pattern attr made it return None
    // and the tip fell back to the hole's centre).
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#plate| { width: 120; height: 60 } [\n  |hole#pin| { width: 10; translate: -35 0; pattern: grid(2, 1, 70, 0) }\n]\nplate.pin <- \"THRU\" { side: top }\n",
    );
    let tip = arrow_tip(&l.nodes);
    let d = ((tip.0 - -35.0).powi(2) + tip.1.powi(2)).sqrt();
    assert!((d - 5.0).abs() < 0.75, "tip on the seed's rim: {tip:?}");
}

#[test]
fn a_circle_diameter_runs_across_with_both_arrows() {
    // The ⌀ line is a diameter, not a word leader [SPEC 15.6]: it crosses
    // the circle, overshoots the far rim, and presses both rims inward.
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#plate| { width: 80; height: 40 }\n|hole#eye| { width: 12 }\neye (o)\n",
    );
    let arrows: Vec<_> = l
        .nodes
        .iter()
        .filter(|n| n.type_chain.iter().any(|t| t == "marker-dim"))
        .collect();
    assert_eq!(arrows.len(), 2, "an arrowhead on each rim");
    let line = l
        .nodes
        .iter()
        .find(|n| n.type_chain.iter().any(|t| t == "dim-line"))
        .expect("the ⌀ line");
    let pts = crate::layout::primitives::attr_points(&line.attrs, "points", line.span)
        .unwrap()
        .unwrap();
    let start_r = (pts[0].0.powi(2) + pts[0].1.powi(2)).sqrt();
    assert!(
        start_r > 6.0 && start_r < 21.0,
        "the line overshoots the far rim: {pts:?}"
    );
}

#[test]
fn side_steers_a_leader() {
    let l = laid(
        "{ layout: drawing; density: 1 }\n|oval#disc| { width: 40; height: 40 }\ndisc <- \"A\" { side: left }\n",
    );
    let (tx, _, _) = text_at(&l.nodes, "A");
    assert!(tx < -20.0, "text left of the disc: {tx}");
}

#[test]
fn the_datum_triangle_seats_on_the_surface() {
    // `>-` on a directed feature: the GD&T triangle's base lies flush
    // with the drawn edge (y = 15), its apex out along the surface
    // normal — never tilted by the leader's approach angle [SPEC 15.7].
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#block| { width: 60; height: 30 }\nblock:bottom >- \"A\"\n",
    );
    let tri = l
        .nodes
        .iter()
        .find(|n| n.type_chain.iter().any(|t| t == "marker-datum"))
        .expect("the seated datum triangle");
    let pts = crate::layout::primitives::attr_points(&tri.attrs, "points", tri.span)
        .unwrap()
        .unwrap();
    assert!(
        (pts[0].1 - 15.0).abs() < 1e-6 && (pts[1].1 - 15.0).abs() < 1e-6,
        "base on the bottom face: {pts:?}"
    );
    assert!(pts[2].1 > 15.0, "apex out along the normal: {pts:?}");
    // …and its leader leaves straight off the face — vertical to the
    // elbow, then the horizontal landing [SPEC 15.7].
    let line = l
        .nodes
        .iter()
        .find(|n| n.type_chain.iter().any(|t| t == "dim-line"))
        .expect("the datum leader");
    let lp = crate::layout::primitives::attr_points(&line.attrs, "points", line.span)
        .unwrap()
        .unwrap();
    assert!(
        (lp[0].0 - lp[1].0).abs() < 1e-6,
        "straight off the surface: {lp:?}"
    );
    // A point-anchored datum keeps the core marker, oriented by the line.
    let l = laid(
        "{ layout: drawing; density: 1 }\n|oval#pin| { width: 20; height: 20 }\npin >- \"B\"\n",
    );
    assert!(
        l.nodes
            .iter()
            .any(|n| n.kind == NodeKind::Line && n.markers.start == MarkerKind::Datum),
        "the fallback datum marker"
    );
}

#[test]
fn a_two_ended_arrow_trims_at_the_rim_and_dots_within() {
    // `b1 -* part`: the line springs from the balloon's rim (default
    // anchor → trimmed) and its dot lands at the part's origin (within).
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#part| { width: 60; height: 30 }\n|balloon#b1| \"1\" { translate: 60 -40 }\nb1 -* part\n",
    );
    let line = l
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Line && n.markers.end == MarkerKind::Dot)
        .expect("the wire");
    let pts = crate::layout::primitives::attr_points(&line.attrs, "points", line.span)
        .unwrap()
        .unwrap();
    let b1 = by_id(&l.nodes, "b1");
    assert!(
        (pts[0].0 - b1.cx).hypot(pts[0].1 - b1.cy) > 7.0,
        "start off the balloon's centre: {:?}",
        pts[0]
    );
    assert_eq!(pts[1], (0.0, 0.0), "the dot lands on the part's origin");
}

#[test]
fn a_leader_tip_lands_on_a_recessed_edge_not_the_box() {
    // The thread section sits below the profile's outer surface: the tip
    // must ray-cast onto the drawn edge (y = −63), not stop at the
    // geometry box (y = −75) — the floating-datum bug (`ray_line`'s
    // segment parameter accepted each segment's mirror about its start).
    let l = laid(
        "{ layout: drawing; scale: 3; density: 1 }\n|sketch#body| { draw: move(-80, 0) up(21) right(38):thread right(32):land up(4) right(90) down(25); mirror: x-axis }\nbody:thread <- \"M42\" { side: top }\nbody:land >- \"A\"\n",
    );
    let arrow_tip = arrow_tip(&l.nodes);
    assert!(
        (arrow_tip.1 + 63.0).abs() < 1e-6,
        "the arrow touches the drawn surface: {arrow_tip:?}"
    );
    let tri = l
        .nodes
        .iter()
        .find(|n| n.type_chain.iter().any(|t| t == "marker-datum"))
        .expect("the seated datum triangle");
    let pts = crate::layout::primitives::attr_points(&tri.attrs, "points", tri.span)
        .unwrap()
        .unwrap();
    assert!(
        (pts[0].1 + 63.0).abs() < 1e-6 && (pts[1].1 + 63.0).abs() < 1e-6,
        "the datum base sits on the drawn surface: {pts:?}"
    );
}

#[test]
fn a_dim_row_clears_leader_texts() {
    // A callout's text registers as an obstacle: a dim stacked on the
    // same side seats its row past it, never on top of it [SPEC 15.6].
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#bar| { width: 200; height: 30 }\nbar:top <- \"M42\"\nbar:left (-) bar:right { side: top }\n",
    );
    let (_, ty, _) = text_at(&l.nodes, "M42");
    let (_, dy, _) = text_at(&l.nodes, "200");
    assert!(
        dy < ty - 8.0,
        "the 200 climbs past the callout text: dim {dy} vs callout {ty}"
    );
}

// ── The anatomy's class hooks [SPEC 17] ──

#[test]
fn dimension_anatomy_wears_its_classes() {
    // Paint states once per class: the dim line, the light extension
    // lines, the marker-classed arrowheads — no per-element inline style.
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#a| { width: 60; height: 20 }\na:left (-) a:right { side: bottom }\n",
    );
    let with_chain = |name: &str| {
        l.nodes
            .iter()
            .filter(|n| n.type_chain.iter().any(|t| t == name))
            .collect::<Vec<_>>()
    };
    assert_eq!(with_chain("ext-line").len(), 2, "two extension springs");
    assert_eq!(with_chain("dim-line").len(), 1, "the dim line");
    assert_eq!(with_chain("marker-dim").len(), 2, "two arrowheads");
    // Extension lines take the light support tone [SPEC 15.6]…
    assert!(
        matches!(
            with_chain("ext-line")[0].attrs.get("stroke"),
            Some(ResolvedValue::LiveVar { name, .. }) if name == "stroke-light"
        ),
        "--stroke-light by default"
    );
    // …until the statement recolours — then the whole dim follows.
    let red = laid(
        "{ layout: drawing; density: 1 }\n|rect#a| { width: 60; height: 20 }\na:left (-) a:right { side: bottom; stroke: red }\n",
    );
    let ext = red
        .nodes
        .iter()
        .find(|n| n.type_chain.iter().any(|t| t == "ext-line"))
        .expect("extension line");
    assert!(
        matches!(ext.attrs.get("stroke"), Some(ResolvedValue::Ident(c)) if c == "red"),
        "a recoloured statement recolours its extension lines too"
    );
}

// ── The drawing's `|-|` weight [SPEC 15.1] ──

#[test]
fn drawing_links_thin_to_stroke_width_1() {
    let width_of = |src: &str| {
        let l = laid(src);
        let dim_line = l
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Line)
            .expect("a dim line");
        match dim_line.attrs.get("stroke-width") {
            Some(ResolvedValue::Number(w)) => *w,
            other => panic!("stroke-width: {other:?}"),
        }
    };
    assert_eq!(
        width_of(
            "{ layout: drawing; density: 1 }\n|rect#a| { width: 40; height: 20 }\na:left (-) a:right\n"
        ),
        1.0,
        "the drawing-scope link default"
    );
    // A scope default, not a rule — a plain `|-|` rule overrides it.
    // And it is the *immediate* scope's default: a flow container nested
    // in a drawing owns ordinary routed links, weight 2.
    let l = laid(
        "|drawing#d| { scale: 0.25 } [\n  |rect#part| { width: 40; height: 20 }\n  |row#legend| { translate: 0 60 } [\n    |box#a| \"a\"\n    |box#b| \"b\"\n    a -> b\n  ]\n]\n",
    );
    let wire = l.links.first().expect("the routed flow link");
    assert!(
        wire.attrs.number("stroke-width").is_none_or(|w| w != 1.0),
        "a nested flow's links keep the flow weight: {:?}",
        wire.attrs.get("stroke-width")
    );
    assert_eq!(
        width_of(
            "{ layout: drawing; scale: 1;\n  |-| { stroke-width: 2 }\n}\n|rect#a| { width: 40; height: 20 }\na:left (-) a:right\n"
        ),
        2.0,
        "a user '|-|' rule wins over the scope default"
    );
}

// ── Datum boxes & fan leaders [SPEC 15.7] ──

/// Every node (self + descendants, world frame) whose type chain carries
/// `class`, as world bboxes.
fn boxes_classed(nodes: &[crate::layout::PlacedNode], class: &str) -> Vec<crate::layout::ir::Bbox> {
    fn walk(
        nodes: &[crate::layout::PlacedNode],
        ox: f64,
        oy: f64,
        class: &str,
        out: &mut Vec<crate::layout::ir::Bbox>,
    ) {
        for n in nodes {
            if n.type_chain.iter().any(|t| t == class) {
                out.push(
                    crate::layout::ir::Bbox::extent_of(std::slice::from_ref(n), |_| true)
                        .shifted(ox, oy),
                );
            }
            walk(&n.children, ox + n.cx, oy + n.cy, class, out);
        }
    }
    let mut out = Vec::new();
    walk(nodes, 0.0, 0.0, class, &mut out);
    out
}

#[test]
fn a_datum_letter_seats_in_a_framed_box_the_rows_stand_off() {
    // The letter lowers to the standard framed box at the landing
    // [SPEC 15.7], registered as painted bounds — a dim row whose span
    // crosses it packs past the frame, not just the letter.
    let geometry = "|rect#a| { width: 80; height: 20 }\n";
    let dim = "a:left (-) a:right { side: bottom }\n";
    let bare = text_at(
        &laid(&format!(
            "{{ layout: drawing; density: 1 }}\n{geometry}{dim}"
        ))
        .nodes,
        "80",
    )
    .1;
    let l = laid(&format!(
        "{{ layout: drawing; density: 1 }}\n{geometry}a:bottom >- \"A\"\n{dim}"
    ));
    let frames = boxes_classed(&l.nodes, "datum-frame");
    assert_eq!(frames.len(), 1, "one framed box");
    let frame = frames[0];
    // The frame squares around the letter and meets the leader's landing.
    let (lx, ly, _) = text_at(&l.nodes, "A");
    assert!(
        frame.min_x < lx && lx < frame.max_x && frame.min_y < ly && ly < frame.max_y,
        "the letter sits inside its frame"
    );
    let (_, dim_y, _) = text_at(&l.nodes, "80");
    assert!(
        dim_y > frame.max_y,
        "the row packs past the frame: text y {dim_y} vs frame bottom {}",
        frame.max_y
    );
    assert!(
        dim_y > bare + 15.0,
        "the box moved the row: {dim_y} vs bare {bare}"
    );
}

#[test]
fn a_fan_leader_shares_one_note_across_independent_legs() {
    // `a & b <- "2× R5"`: one text, one landing (the first endpoint
    // steers), a ray-cast leg per feature [SPEC 15.7].
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#a| { width: 40; height: 20; translate: -40 0 }\n|rect#b| { width: 40; height: 20; translate: 40 0 }\na & b <- \"2× R5\"\n",
    );
    let (_, _, _) = text_at(&l.nodes, "2× R5"); // exactly one text
    let arrows = boxes_classed(&l.nodes, "marker-dim");
    assert_eq!(arrows.len(), 2, "a leg tips each feature");
    let lines: Vec<_> = boxes_classed(&l.nodes, "dim-line");
    assert_eq!(lines.len(), 2, "the steering leg + one fan leg");
}

#[test]
fn an_unroutable_fan_leg_is_reported() {
    // The text lands left (steered by `a`); `b:right` faces away — the ray
    // from the shared landing strikes b's near face first, so the leg
    // cannot reach its anchor [SPEC 15.7]. Reported, never dropped.
    let e = layout_err(
        "{ layout: drawing; density: 1 }\n|rect#a| { width: 40; height: 20; translate: -60 0 }\n|rect#b| { width: 40; height: 20; translate: 60 0 }\na & b:right <- \"X\"\n",
    );
    assert!(e.contains("a fan leg cannot reach 'b:right'"), "got: {e}");
}

// ── Carried annotation nodes [SPEC 15.9] ──

#[test]
fn a_carried_frame_stacks_under_the_dim_value_and_rides_its_row() {
    // A `[ ]` frame lowers at the statement's text seat: under the value,
    // centred on it, below the dim line on the packed row [SPEC 15.9].
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#p| { width: 120; height: 40 }\np:bottom >- \"A\"\np:left (-) p:right { side: bottom } [\n  |feature-control#fcf| \"flatness\" { tol: 0.2 }\n]\n",
    );
    let (tx, ty, _) = text_at(&l.nodes, "120");
    let f = by_id(&l.nodes, "fcf");
    let fb = crate::layout::ir::Bbox::extent_of(std::slice::from_ref(f), |_| true);
    assert!(
        fb.min_y > ty,
        "the frame stacks under the value: {fb:?} vs {ty}"
    );
    let fcx = (fb.min_x + fb.max_x) / 2.0;
    assert!((fcx - tx).abs() < 0.5, "centred on the seat: {fcx} vs {tx}");
}

#[test]
fn a_later_row_packs_past_a_carried_frame() {
    // The carried box registers as a packing obstacle — the next dim on the
    // same side seats its row past it, never on top [SPEC 15.6/15.9].
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#p| { width: 120; height: 40 } [\n  |hole#h| { width: 10; translate: -30 0 }\n]\np:left (-) p:right { side: bottom } [\n  |feature-control#fcf| \"flatness\" { tol: 0.2 }\n]\np:left (-) p.h:center { side: bottom }\n",
    );
    let f = by_id(&l.nodes, "fcf");
    let fb = crate::layout::ir::Bbox::extent_of(std::slice::from_ref(f), |_| true);
    let (_, ty2, _) = text_at(&l.nodes, "30");
    assert!(
        ty2 > fb.max_y,
        "the second row cleared the frame: text at {ty2}, frame to {}",
        fb.max_y
    );
}

#[test]
fn a_carried_frame_rides_a_round_leader_under_its_value() {
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#p| { width: 120; height: 40 } [\n  |hole#h| { width: 10 }\n]\np:bottom >- \"A\"\np.h (o) [\n  |feature-control#fcf| \"position\" { tol: 0.1; zone: diameter; datums: A }\n]\n",
    );
    let (tx, ty, _) = text_at(&l.nodes, "⌀10");
    let f = by_id(&l.nodes, "fcf");
    let fb = crate::layout::ir::Bbox::extent_of(std::slice::from_ref(f), |_| true);
    assert!(fb.min_y > ty, "under the callout line: {fb:?} vs {ty}");
    let fcx = (fb.min_x + fb.max_x) / 2.0;
    assert!((fcx - tx).abs() < 0.5, "centred on the seat: {fcx} vs {tx}");
}

#[test]
fn a_carried_datum_states_the_axis_and_feeds_the_alphabet() {
    // `|datum| "B"` in a dimension's `[ ]` lowers the framed letter at the
    // seat and joins the identity set — a frame may reference it [SPEC 15.9].
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#p| { width: 120; height: 40 }\np:left (-) p:right { side: bottom } [ |datum#axis| \"B\" ]\n|feature-control#fr| \"perpendicularity\" { tol: 0.1; datums: B; translate: 0 -60 }\n",
    );
    let d = by_id(&l.nodes, "axis");
    let db = crate::layout::ir::Bbox::extent_of(std::slice::from_ref(d), |_| true);
    let (_, ty, _) = text_at(&l.nodes, "120");
    assert!(db.min_y > ty, "the framed letter stacks under the value");
}

#[test]
fn a_carried_frame_validates_datums_against_the_scope() {
    let e = layout_err(
        "{ layout: drawing; density: 1 }\n|rect#p| { width: 120; height: 40 }\np:bottom >- \"A\"\np:left (-) p:right [ |feature-control| \"position\" { tol: 0.1; datums: Z } ]\n",
    );
    assert_eq!(e, "no datum 'Z' in this drawing — declared: A");
}

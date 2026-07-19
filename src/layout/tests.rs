use super::*;

fn lay_out(src: &str) -> LaidOut {
    let tokens = crate::lexer::lex(src).expect("lex");
    let file = crate::syntax::parser::parse(src, &tokens).expect("parse");
    let lowered = crate::desugar::desugar(&file).expect("desugar");
    let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
    layout(&program).expect("layout")
}

// ── Sizing [SPEC 5] ──

#[test]
fn empty_closed_primitive_is_two_paddings() {
    // padding 20 each side → 40 drawn; + stroke 2 → 42 bbox.
    let n = &lay_out("|box|\n").nodes[0];
    assert!((n.bbox.w() - 42.0).abs() < 0.01, "w={}", n.bbox.w());
    assert!((n.bbox.h() - 42.0).abs() < 0.01, "h={}", n.bbox.h());
}

#[test]
fn explicit_dims_are_border_box() {
    let n = &lay_out("|box| { width: 100; height: 50; }\n").nodes[0];
    assert!((n.bbox.w() - 102.0).abs() < 0.01, "w={}", n.bbox.w());
    assert!((n.bbox.h() - 52.0).abs() < 0.01, "h={}", n.bbox.h());
}

#[test]
fn stroke_width_counts_toward_the_bbox() {
    // [SPEC 5]: width 100 height 50 stroke-width 4 → 104×54.
    let n = &lay_out("|box| { width: 100; height: 50; stroke-width: 4; }\n").nodes[0];
    assert!((n.bbox.w() - 104.0).abs() < 0.01, "w={}", n.bbox.w());
    assert!((n.bbox.h() - 54.0).abs() < 0.01, "h={}", n.bbox.h());
}

#[test]
fn label_auto_sizes_to_content_plus_padding() {
    // text ~18 + 2×20 padding + 2 stroke → ~60.
    let n = &lay_out("|box| \"hi\"\n").nodes[0];
    assert!(n.bbox.w() > 55.0 && n.bbox.w() < 65.0, "w={}", n.bbox.w());
}

#[test]
fn dims_are_independent_per_axis() {
    let n = &lay_out("|box| \"hi\" { width: 200 }\n").nodes[0];
    assert!((n.bbox.w() - 202.0).abs() < 0.01, "w={}", n.bbox.w());
    // height auto = one text line (15) + 40 padding + 2 stroke = 57.
    assert!((n.bbox.h() - 57.0).abs() < 0.01, "h={}", n.bbox.h());
}

#[test]
fn explicit_size_is_a_floor_not_a_clip() {
    // Content wider than the declared width grows the box instead of spilling.
    let grown = &lay_out("|box| \"a long label\" { width: 40 }\n").nodes[0];
    assert!(
        grown.bbox.w() > 60.0,
        "floor grows to content: w={}",
        grown.bbox.w()
    );
    // A width the content fits within is honoured exactly (border-box + stroke).
    let kept = &lay_out("|box| \"hi\" { width: 300 }\n").nodes[0];
    assert!((kept.bbox.w() - 302.0).abs() < 0.01, "w={}", kept.bbox.w());
}

#[test]
fn asymmetric_padding_offsets_the_content() {
    // padding t r b l = 0 0 0 20 → 20 on the left, 0 on the right, so the
    // content shifts right by (20 − 0)/2 = 10.
    let off = &lay_out("|box| \"x\" { padding: 0 0 0 20 }\n").nodes[0];
    assert!(
        (off.children[0].cx - 10.0).abs() < 0.01,
        "cx={}",
        off.children[0].cx
    );
    // Symmetric padding keeps it centred.
    let mid = &lay_out("|box| \"x\" { padding: 8 }\n").nodes[0];
    assert!(
        mid.children[0].cx.abs() < 0.01,
        "centred: cx={}",
        mid.children[0].cx
    );
}

#[test]
fn oval_uses_width_height() {
    let n = &lay_out("|oval| { width: 100; height: 50; }\n").nodes[0];
    assert!((n.bbox.w() - 102.0).abs() < 0.01, "w={}", n.bbox.w());
    assert!((n.bbox.h() - 52.0).abs() < 0.01, "h={}", n.bbox.h());
}

#[test]
fn text_sizes_to_its_glyphs_without_padding() {
    let n = &lay_out("\"hi\"\n").nodes[0];
    assert!((n.bbox.w() - 18.0).abs() < 0.5, "w={}", n.bbox.w()); // 2 × 15 × 0.6
    assert!((n.bbox.h() - 15.0).abs() < 0.5, "h={}", n.bbox.h());
}

// ── Basic flow (full align/justify/stretch/evenly land in the flex chunk) ──

#[test]
fn row_layout_stacks_horizontally() {
    let l = lay_out(
        "{ direction: row; gap: 10; }\n\
             |box| { width: 100; height: 40; }\n\
             |box| { width: 60; height: 40; }\n",
    );
    assert_eq!(l.nodes.len(), 2);
    // half (51) + gap (10) + half (31) = 92.
    let dx = l.nodes[1].cx - l.nodes[0].cx;
    assert!((dx - 92.0).abs() < 0.5, "dx={}", dx);
    assert!((l.nodes[0].cy - l.nodes[1].cy).abs() < 0.01);
}

#[test]
fn column_layout_stacks_vertically() {
    let l = lay_out(
        "{ direction: column; gap: 20; }\n\
             |box| { width: 100; height: 40; }\n\
             |box| { width: 100; height: 60; }\n",
    );
    // half (21) + gap (20) + half (31) = 72.
    let dy = l.nodes[1].cy - l.nodes[0].cy;
    assert!((dy - 72.0).abs() < 0.5, "dy={}", dy);
    assert!((l.nodes[0].cx - l.nodes[1].cx).abs() < 0.01);
}

fn lay_out_err(src: &str) -> Error {
    let tokens = crate::lexer::lex(src).expect("lex");
    let file = crate::syntax::parser::parse(src, &tokens).expect("parse");
    let lowered = crate::desugar::desugar(&file).expect("desugar");
    let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
    match layout(&program) {
        Ok(_) => panic!("expected a layout error"),
        Err(e) => e,
    }
}

#[test]
fn layout_row_and_column_are_removed() {
    for dir in ["row", "column"] {
        let err = lay_out_err(&format!("{{ layout: {dir}; }}\n|box|\n|box|\n"));
        assert!(
            err.message.contains(&format!("direction: {dir}")),
            "msg={}",
            err.message
        );
    }
}

#[test]
fn direction_radial_is_rejected_in_a_flow() {
    let err = lay_out_err("{ direction: radial; }\n|box|\n|box|\n");
    assert!(err.message.contains("chart"), "msg={}", err.message);
}

#[test]
fn viewbox_wraps_content_with_scene_padding() {
    // bbox 102×42, + the scene's 20 padding each side → 142×82.
    let l = lay_out("|box| { width: 100; height: 40; }\n");
    assert!((l.viewbox.w - 142.0).abs() < 0.01, "w={}", l.viewbox.w);
    assert!((l.viewbox.h - 82.0).abs() < 0.01, "h={}", l.viewbox.h);
}

#[test]
fn a_pages_only_scene_carries_its_physical_mm() {
    // An A5-landscape sheet (210×148 mm) prints true-scale [SPEC 15.8]:
    // the physical size is the viewBox over the page's px-per-mm scale.
    let l = lay_out(
        "|page| { sheet: a5 landscape } [ |drawing| { scale: 4 } [ |rect#r| { width: 10; height: 10 } ] ]\n",
    );
    let (w, h) = l.physical.expect("a pages-only scene prints true-scale");
    assert!(
        (w - 210.0).abs() < 0.5 && (h - 148.0).abs() < 0.5,
        "{w}×{h} mm"
    );
    // A non-sheet scene sizes in pixels.
    let d = lay_out("{ layout: drawing; density: 1 }\n|rect#r| { width: 10; height: 10 }\n");
    assert_eq!(d.physical, None);
}

// ── Captions: ordinary flow children [SPEC 8] ──

#[test]
fn caption_overlay_does_not_grow_the_group() {
    // A caption pins to the top edge (an overlay), so it reserves no flow
    // row — the group sizes to its content alone, with or without it.
    let h = |src: &str| lay_out(src).nodes[0].bbox.h();
    let plain = h("|group#g| [\n  |box#a| { width: 80; height: 30; }\n]\n");
    let capped = h("|group#g| [\n  |caption| \"Cap\"\n  |box#a| { width: 80; height: 30; }\n]\n");
    assert!(
        (capped - plain).abs() < 0.01,
        "caption is an overlay, no extra height: plain={plain} capped={capped}"
    );
}

#[test]
fn caption_sits_above_the_content() {
    let l = lay_out("|group#g| [\n  |caption| \"Cap\"\n  |box#a| { width: 80; height: 30; }\n]\n");
    let g = &l.nodes[0];
    let cap = g
        .children
        .iter()
        .find(|c| c.type_chain.iter().any(|t| t == "caption"))
        .expect("caption child");
    let a = g
        .children
        .iter()
        .find(|c| c.id.as_deref() == Some("a"))
        .expect("box child");
    assert!(cap.cy < a.cy, "cap.cy={} a.cy={}", cap.cy, a.cy);
}

// ── Flex distribution with slack [SPEC 12] ──

#[test]
fn justify_orders_children_start_center_end() {
    let first_cx = |j: &str| {
        let src = format!(
            "|row#g| {{ width: 300; justify: {j} }} [\n  |box#a| {{ width: 40; height: 20; }}\n  |box#b| {{ width: 40; height: 20; }}\n]\n"
        );
        lay_out(&src).nodes[0].children[0].cx
    };
    let (start, center, end) = (first_cx("start"), first_cx("center"), first_cx("end"));
    assert!(
        start < center && center < end,
        "start={start} center={center} end={end}"
    );
}

#[test]
fn justify_evenly_spaces_children_equally() {
    let l = lay_out(
        "|row#g| { width: 300; justify: evenly } [\n  |box#a| { width: 20; height: 20; }\n  |box#b| { width: 20; height: 20; }\n  |box#c| { width: 20; height: 20; }\n]\n",
    );
    let cx: Vec<f64> = l.nodes[0].children.iter().map(|c| c.cx).collect();
    assert!(
        ((cx[1] - cx[0]) - (cx[2] - cx[1])).abs() < 0.01,
        "centers {cx:?}"
    );
}

#[test]
fn align_stretch_fills_the_cross_axis() {
    // An unsized child grows to the row's content height (row pads 0).
    let l = lay_out("|row#g| { height: 80; align: stretch } [\n  |box#a| { width: 40; }\n]\n");
    let a = &l.nodes[0].children[0];
    assert!((a.bbox.h() - 80.0).abs() < 1.0, "a.h={}", a.bbox.h());
}

#[test]
fn no_slack_means_no_distribution() {
    // An auto-width row ignores justify — children stay packed at the gap.
    let span = |j: &str| {
        let src = format!(
            "|row#g| {{ justify: {j} }} [\n  |box#a| {{ width: 40; height: 20; }}\n  |box#b| {{ width: 40; height: 20; }}\n]\n"
        );
        let l = lay_out(&src);
        l.nodes[0].children[1].cx - l.nodes[0].children[0].cx
    };
    assert!(
        (span("start") - span("end")).abs() < 0.01,
        "auto row: justify is a no-op"
    );
}

// ── Grid [SPEC 12] ──

#[test]
fn grid_fixed_columns_place_children_in_order() {
    let l = lay_out(
        "{ layout: grid; columns: 80, 80, 80; gap: 0; }\n\
             |box#a| { width: 40; height: 40; }\n\
             |box#b| { width: 40; height: 40; }\n\
             |box#c| { width: 40; height: 40; }\n",
    );
    let cx: Vec<f64> = l.nodes.iter().map(|n| n.cx).collect();
    assert!((cx[1] - cx[0] - 80.0).abs() < 0.5, "dx={}", cx[1] - cx[0]);
    assert!((cx[2] - cx[1] - 80.0).abs() < 0.5);
    assert!((l.nodes[0].cy - l.nodes[1].cy).abs() < 0.01);
}

#[test]
fn grid_repeat_makes_auto_columns_and_wraps() {
    let l = lay_out(
        "{ layout: grid; columns: repeat(2); }\n\
             |box#a| { width: 30; height: 30; }\n\
             |box#b| { width: 30; height: 30; }\n\
             |box#c| { width: 30; height: 30; }\n",
    );
    // 2 columns, 3 children → c wraps to the second row.
    assert!(l.nodes[2].cy > l.nodes[0].cy, "c below a");
}

#[test]
fn grid_cell_pins_placement() {
    let l = lay_out(
        "{ layout: grid; columns: repeat(3); }\n\
             |box#a| { cell: 3 1; }\n\
             |box#b|\n",
    );
    // a pins to column 3; b auto-flows to the first free cell (column 1).
    assert!(
        l.nodes[0].cx > l.nodes[1].cx,
        "a (col 3) right of b (col 1)"
    );
}

// ── Title block: authored cells after the generated fields [SPEC 15.8] ──

/// The `|title-block|` node inside a laid-out page.
fn find_title_block(nodes: &[PlacedNode]) -> &PlacedNode {
    fn walk(nodes: &[PlacedNode]) -> Option<&PlacedNode> {
        nodes.iter().find_map(|n| {
            if n.type_chain.iter().any(|t| t == "title-block") {
                Some(n)
            } else {
                walk(&n.children)
            }
        })
    }
    walk(nodes).expect("a title block")
}

#[test]
fn title_block_authored_cells_follow_the_generated_fields() {
    let l = lay_out(
        "|page| { sheet: a4 landscape } [\n\
           |drawing#v| [ |rect#r| { width: 10; height: 10 } ]\n\
           |title-block| { title: \"T\"; drawing-number: \"D\"; revision: \"A\";\n\
             sheet-number: \"1/1\"; date: \"2026\"; author: \"AM\"; } [\n\
             |box#logo| { cell: 3 3; width: 8; height: 8; }\n\
             |box#note| { span: 2 1; width: 8; height: 8; }\n\
           ]\n\
         ]\n",
    );
    let tb = find_title_block(&l.nodes);
    let by_id = |id: &str| {
        tb.children
            .iter()
            .find(|c| c.id.as_deref() == Some(id))
            .expect(id)
    };
    let date = tb
        .children
        .iter()
        .find(|c| matches!(c.attrs.get("field"), Some(ResolvedValue::String(s)) if s == "Date"))
        .expect("the Date field cell");
    // `cell: 3 3` seats the logo in the fields' last free slot: the Date row
    // (row 3), right of the Author column.
    let logo = by_id("logo");
    assert!((logo.cy - date.cy).abs() < 1e-6, "logo shares the Date row");
    assert!(logo.cx > date.cx, "logo right of Date (column 3)");
    // The unaddressed 2-wide note can't fit a remaining slot: it flows into
    // the next row, below every generated field.
    let note = by_id("note");
    assert!(note.cy > date.cy, "note flows below the generated rows");
}

#[test]
fn title_block_authored_cell_on_a_generated_field_errors() {
    let err = lay_out_err(
        "|page| { sheet: a4 landscape } [\n\
           |drawing#v| [ |rect#r| { width: 10; height: 10 } ]\n\
           |title-block| { title: \"T\"; drawing-number: \"D\"; revision: \"A\"; } [\n\
             |box#x| { cell: 2 2; width: 8; height: 8; }\n\
           ]\n\
         ]\n",
    );
    assert!(
        err.to_string()
            .contains("cell 2 2 is taken by the generated 'Rev' field — place it after the fields"),
        "{err}"
    );
}

#[test]
fn title_block_authored_span_crossing_a_field_errors() {
    // Pinned beside the fields but spanning left onto the 'Rev' slot.
    let err = lay_out_err(
        "|page| { sheet: a4 landscape } [\n\
           |drawing#v| [ |rect#r| { width: 10; height: 10 } ]\n\
           |title-block| { title: \"T\"; drawing-number: \"D\"; revision: \"A\"; } [\n\
             |box#x| { cell: 2 2; span: 2 1; width: 8; height: 8; }\n\
           ]\n\
         ]\n",
    );
    assert!(
        err.to_string().contains("is taken by the generated"),
        "{err}"
    );
}

#[test]
fn grid_cell_fills_its_track_under_stretch() {
    let l = lay_out(
        "{ layout: grid; columns: 120, 120; gap: 0; }\n\
             |box#a| { justify: stretch; align: stretch; }\n\
             |box#b|\n",
    );
    assert!(
        (l.nodes[0].bbox.w() - 120.0).abs() < 1.0,
        "a.w={}",
        l.nodes[0].bbox.w()
    );
}

#[test]
fn grid_rows_track_list_is_a_floor_implicit_rows_overflow() {
    // [SPEC 12/18]: a declared `rows` track list sizes the first rows; extra
    // children flow into implicit auto rows (CSS grid) rather than erroring.
    // Here 2 cols × 1 declared row track, 4 children → a second, implicit row.
    let l = lay_out(
        "{ layout: grid; columns: 40, 40; rows: auto; }\n\
             |box#a| { width: 30; height: 30; }\n\
             |box#b| { width: 30; height: 30; }\n\
             |box#c| { width: 30; height: 30; }\n\
             |box#d| { width: 30; height: 30; }\n",
    );
    assert!(l.nodes[2].cy > l.nodes[0].cy, "c (row 2) below a (row 1)");
    assert!(
        (l.nodes[2].cy - l.nodes[3].cy).abs() < 0.01,
        "c, d share row 2"
    );
}

#[test]
fn grid_without_columns_is_an_error() {
    let src = "{ layout: grid; }\n|box#a|\n|box#b|\n";
    let tokens = crate::lexer::lex(src).expect("lex");
    let file = crate::syntax::parser::parse(src, &tokens).expect("parse");
    let lowered = crate::desugar::desugar(&file).expect("desugar");
    let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
    assert!(layout(&program).is_err());
}

// ── Gutters [SPEC 11] ──

#[test]
fn table_fills_interior_gutters_no_frame() {
    let l = lay_out("|table#t| { columns: 40, 40 } [\n  \"a\" \"b\" \"c\" \"d\"\n]\n");
    // The table's `gap-fill: --stroke` fills the interior gutters.
    assert!(!l.nodes[0].gutters.is_empty(), "table has interior gutters");
    // A plain group has no `gap-fill`, so no gutters.
    assert!(
        lay_out("|group#g| [ |box#x| ]\n").nodes[0]
            .gutters
            .is_empty()
    );
}

#[test]
fn grid_gutters_stay_within_the_content_box() {
    // Interior gutter rects must not overshoot the frame: every rect sits fully
    // inside the grid's own content box.
    let l = lay_out(
        "|table#t| { columns: 40, 40; gap: 20 } [\n  \"a\"\n  \"b\"\n  \"c\"\n  \"d\"\n]\n",
    );
    let t = &l.nodes[0];
    let (hw, hh) = (t.bbox.w() / 2.0 + 0.01, t.bbox.h() / 2.0 + 0.01);
    for (cx, cy, w, h) in &t.gutters {
        assert!(cx.abs() + w / 2.0 <= hw, "gutter x {cx}±{} > {hw}", w / 2.0);
        assert!(cy.abs() + h / 2.0 <= hh, "gutter y {cy}±{} > {hh}", h / 2.0);
    }
}

#[test]
fn one_d_gutter_falls_between_flow_children() {
    let l = lay_out(
        "|row#g| { gap-fill: --stroke } [\n  |box#a| { width: 30; height: 30; }\n  |box#b| { width: 30; height: 30; }\n  |box#c| { width: 30; height: 30; }\n]\n",
    );
    assert_eq!(
        l.nodes[0].gutters.len(),
        2,
        "two gutters between three children"
    );
}

#[test]
fn gap_fill_per_axis_selects_gutters() {
    // `gap: row col` [SPEC 11]: `4 0` paints row rules (horizontal gutters), `0 4`
    // column rules (vertical). A 2×2 grid has one interior boundary each way.
    let rows_only = lay_out(
        "|grid#g| { columns: 40, 40; gap: 4 0; gap-fill: --stroke } [\n  \"a\" \"b\"\n  \"c\" \"d\"\n]\n",
    );
    let (_, _, w, h) = rows_only.nodes[0].gutters[0];
    assert_eq!(rows_only.nodes[0].gutters.len(), 1, "row gap → one gutter");
    assert!(w > h, "horizontal gutter is wide: w={w} h={h}");

    let cols_only = lay_out(
        "|grid#g| { columns: 40, 40; gap: 0 4; gap-fill: --stroke } [\n  \"a\" \"b\"\n  \"c\" \"d\"\n]\n",
    );
    let (_, _, w2, h2) = cols_only.nodes[0].gutters[0];
    assert_eq!(cols_only.nodes[0].gutters.len(), 1, "col gap → one gutter");
    assert!(h2 > w2, "vertical gutter is tall: w={w2} h={h2}");
}

// ── `scale:` — a global node transform [SPEC 15.1] ──

#[test]
fn scale_multiplies_the_shape_never_text_or_stroke() {
    let plain = &lay_out("|box#a| \"hi\" { width: 100; height: 40 }\n").nodes[0];
    let scaled = &lay_out("|box#a| \"hi\" { width: 100; height: 40; scale: 2 }\n").nodes[0];
    assert!(
        (scaled.bbox.w() - 202.0).abs() < 0.01,
        "w={}",
        scaled.bbox.w()
    );
    assert!(
        (scaled.bbox.h() - 82.0).abs() < 0.01,
        "h={}",
        scaled.bbox.h()
    );
    // The text child keeps its size — text never scales.
    assert!((scaled.children[0].bbox.w() - plain.children[0].bbox.w()).abs() < 0.01);
}

#[test]
fn scale_inherits_nearest_ancestor_wins() {
    // The root's scale reaches the child; the note's own `scale: 1` opts out.
    let l = lay_out(
        "{ scale: 2 }\n|rect#a| { width: 50; height: 20 }\n|note#n| { width: 50; height: 20 }\n",
    );
    let a = &l.nodes[0];
    assert!(
        (a.bbox.w() - 102.0).abs() < 0.01,
        "inherited: w={}",
        a.bbox.w()
    );
    let n = &l.nodes[1];
    assert!(
        n.bbox.w() < 60.0,
        "the note is sheet chrome: w={}",
        n.bbox.w()
    );
}

#[test]
fn translate_scales_by_the_parent() {
    // A column flow: the x offset between the boxes is the translate alone,
    // in drawing units × the parent's scale [SPEC 15.1].
    let nudge = |src: &str| {
        let l = lay_out(src);
        l.nodes[1].cx - l.nodes[0].cx
    };
    let plain = nudge(
        "|rect#a| { width: 10; height: 10 }\n|rect#b| { width: 10; height: 10; translate: 5 0 }\n",
    );
    let scaled = nudge(
        "{ scale: 3 }\n|rect#a| { width: 10; height: 10 }\n|rect#b| { width: 10; height: 10; translate: 5 0 }\n",
    );
    assert!((plain - 5.0).abs() < 0.01, "plain={plain}");
    assert!((scaled - 15.0).abs() < 0.01, "scaled={scaled}");
}

#[test]
fn a_scaled_sketch_in_a_flow_doubles_its_geometry() {
    let one =
        &lay_out("|sketch#s| { draw: move(0, 0) right(40) down(20) left(40) close() }\n").nodes[0];
    let two =
        &lay_out("|sketch#s| { draw: move(0, 0) right(40) down(20) left(40) close(); scale: 2 }\n")
            .nodes[0];
    assert!((two.bbox.w() - one.bbox.w() - 40.0).abs() < 0.01);
    // The folded d carries the scaled coordinates for render.
    assert!(
        matches!(two.attrs.get("path"), Some(ResolvedValue::String(d)) if d.contains("80")),
        "scaled path"
    );
}

#[test]
fn scale_must_be_positive() {
    let err = lay_out_err("|box#a| { scale: 0 }\n");
    assert_eq!(err.message, "'scale' must be > 0");
}

// ── `pattern:` — replicate in any layout [SPEC 15.4] ──

#[test]
fn a_patterned_box_in_a_flow_unions_its_copies() {
    let l = lay_out("|rect#a| { width: 20; height: 20; pattern: grid(3, 1, 30, 0) }\n");
    let a = &l.nodes[0];
    // Seed at 0, copies at 30 and 60 → 20 + 60 + stroke.
    assert!((a.bbox.w() - 82.0).abs() < 0.01, "w={}", a.bbox.w());
    assert_eq!(a.children.len(), 3, "three copies");
    assert!(a.id.as_deref() == Some("a"), "the carrier keeps the id");
}

#[test]
fn a_filled_grid_cell_aligns_its_text_by_its_own_align() {
    // A grid cell filled by the container's `align: stretch`, then, honours, its
    // own `align` (↔) to place its text [SPEC 12] — the generic rule tables use.
    let text_cx = |a: &str| {
        let src = format!(
            "|grid#g| {{ columns: 200; align: stretch }} [\n  |block#c| \"x\" {{ align: {a} }}\n]\n"
        );
        let l = lay_out(&src);
        let text = &l.nodes[0].children[0].children[0];
        assert_eq!(text.kind, NodeKind::Text);
        text.cx
    };
    // The cell fills the 200-wide track; `start` hugs the text left of centre,
    // `end` right, `center` stays centred.
    assert!(text_cx("start") < -50.0, "start: {}", text_cx("start"));
    assert!(text_cx("end") > 50.0, "end: {}", text_cx("end"));
    assert!(
        text_cx("center").abs() < 5.0,
        "center: {}",
        text_cx("center")
    );
}

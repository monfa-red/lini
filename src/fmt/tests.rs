use super::format;

fn fmt(src: &str) -> String {
    format(src).expect("format")
}

/// fmt output must re-parse cleanly (it is valid).
fn reparses(src: &str) {
    let out = fmt(src);
    let toks = crate::lexer::lex(&out).expect("lex fmt output");
    crate::syntax::parser::parse(src, &toks).expect("parse fmt output");
}

/// The core invariant: a second pass changes nothing.
fn idempotent(src: &str) {
    let once = fmt(src);
    let twice = fmt(&once);
    assert_eq!(
        once, twice,
        "not idempotent:\n--- once ---\n{once}\n--- twice ---\n{twice}"
    );
}

#[test]
fn node_head_label() {
    // A head label is preserved; a `[ ]` text child is left as written (fmt resolves
    // no types, and the head label's meaning is type-dependent — [SPEC 3]).
    assert_eq!(fmt("|box#x| \"hi\"\n"), "|box#x| \"hi\"\n");
    assert_eq!(fmt("|box#x|[ \"hi\" ]\n"), "|box#x| [ \"hi\" ]\n");
}

#[test]
fn id_only_bars() {
    assert_eq!(fmt("|#cat|\n"), "|#cat|\n");
}

#[test]
fn bare_label_node() {
    assert_eq!(fmt("\"Apple\"\n"), "\"Apple\"\n");
}

#[test]
fn root_declaration() {
    assert_eq!(fmt("{layout:grid}\n"), "{\n  layout: grid;\n}\n");
}

#[test]
fn variable_declaration() {
    assert_eq!(fmt("{--brand:#ff6600}\n"), "{\n  --brand: #ff6600;\n}\n");
}

#[test]
fn element_rule() {
    assert_eq!(fmt("{|box|{radius:6}}\n"), "{\n  |box| { radius: 6; }\n}\n");
}

#[test]
fn class_rule() {
    assert_eq!(
        fmt("{.hot{stroke-width:2}}\n"),
        "{\n  .hot { stroke-width: 2; }\n}\n"
    );
}

#[test]
fn id_rule() {
    assert_eq!(
        fmt("{#hero{fill:gold}}\n"),
        "{\n  #hero { fill: gold; }\n}\n"
    );
}

#[test]
fn descendant_rule() {
    assert_eq!(
        fmt("{|table| |box|{padding:4 8}}\n"),
        "{\n  |table| |box| { padding: 4 8; }\n}\n"
    );
}

#[test]
fn id_pinned_descendant_rule() {
    assert_eq!(
        fmt("{|table#main| |box|{fill:white}}\n"),
        "{\n  |table#main| |box| { fill: white; }\n}\n"
    );
}

#[test]
fn define() {
    assert_eq!(
        fmt("{|treat::box|{radius:5}}\n"),
        "{\n  |treat::box| { radius: 5; }\n}\n"
    );
}

#[test]
fn multi_group_value_list() {
    assert_eq!(
        fmt("|line#dim|{points:0 0,10 10}\n"),
        "|line#dim| { points: 0 0, 10 10; }\n"
    );
}

#[test]
fn function_value() {
    assert_eq!(
        fmt("{layout:grid;\ncolumns:repeat(3)}\n"),
        "{\n  layout: grid; columns: repeat(3);\n}\n"
    );
}

#[test]
fn bindings_and_group_expressions() {
    // A scalar binding reads bare; a function's body is a group [SPEC 10.7].
    assert_eq!(fmt("{my_r=5}\n"), "{\n  my_r = 5;\n}\n");
    assert_eq!(
        fmt("{scale(n)=(100 * 1.2 ^ n)}\n"),
        "{\n  scale(n) = (100 * 1.2 ^ n);\n}\n"
    );
    // A direct group value wears its parens; a call argument sheds them (it is
    // already inside the call's own parens).
    assert_eq!(
        fmt("|box#a| { padding: (8 * 2); width: gain(2 * n) }\n"),
        "|box#a| { padding: (8 * 2); width: gain(2 * n); }\n"
    );
    idempotent(
        "{ my_r = 5; scale(n) = (100 * 1.2 ^ n); }\n|box#a| { padding: (8 * 2); width: gain(2 * n) }\n",
    );
}

#[test]
fn node_class_follows_the_bars() {
    assert_eq!(fmt("|box#x| .hot.loud\n"), "|box#x| .hot.loud\n");
    // A spaced class chain normalizes to glued.
    assert_eq!(fmt("|box#x| .hot .loud\n"), "|box#x| .hot.loud\n");
    // A class on a default box (id only).
    assert_eq!(fmt("|#x| .hot\n"), "|#x| .hot\n");
}

#[test]
fn head_label_before_classes_and_style() {
    assert_eq!(
        fmt("|box#api| \"API\" .hot{fill:red}\n"),
        "|box#api| \"API\" .hot { fill: red; }\n"
    );
}

#[test]
fn node_with_style_and_children() {
    assert_eq!(
        fmt("|group#g|{direction:column}[\n|box#a|\n|box#b|\n]\n"),
        "|group#g| { direction: column; } [\n  |box#a|\n  |box#b|\n]\n"
    );
}

#[test]
fn block_declarations_group_on_one_line() {
    // [SPEC 19]: config decls share a line in the style block, off the head.
    assert_eq!(
        fmt("|group#g| { cell: 1 2; direction: column; gap: 16 } [\n|box#a|\n]\n"),
        "|group#g| { cell: 1 2; direction: column; gap: 16; } [\n  |box#a|\n]\n"
    );
}

#[test]
fn a_comment_breaks_a_declaration_group_and_forces_a_block() {
    assert_eq!(
        fmt("|group#g| {\n  direction: row;\n  // note\n  gap: 10;\n} [\n  |box#a|\n]\n"),
        "|group#g| {\n  direction: row;\n  // note\n  gap: 10;\n} [\n  |box#a|\n]\n"
    );
}

#[test]
fn simple_link() {
    assert_eq!(fmt("a -> b\n"), "a -> b\n");
}

#[test]
fn link_label_trails() {
    assert_eq!(fmt("a -> b \"x\"\n"), "a -> b \"x\"\n");
}

#[test]
fn link_fan_and_chain() {
    assert_eq!(fmt("a & b -> c\n"), "a & b -> c\n");
    assert_eq!(fmt("a -> b -> c\n"), "a -> b -> c\n");
}

#[test]
fn link_line_ops() {
    assert_eq!(fmt("a --> b\n"), "a --> b\n");
    assert_eq!(fmt("a ---> b\n"), "a ---> b\n");
    assert_eq!(fmt("a ~> b\n"), "a ~> b\n");
}

#[test]
fn link_class_and_labels_with_along() {
    assert_eq!(
        fmt("a -> b {along:0.3, 0.7}[ \"near a\" \"near b\" ]\n"),
        "a -> b { along: 0.3, 0.7; } [ \"near a\" \"near b\" ]\n"
    );
    assert_eq!(fmt("a -> b .loud\n"), "a -> b .loud\n");
    // A spaced link-class chain normalizes to glued, like a node's.
    assert_eq!(fmt("a -> b .c1 .c2\n"), "a -> b .c1.c2\n");
    // A head label precedes the class (the tail order, re-parseable).
    assert_eq!(fmt("a -> b \"flows\" .loud\n"), "a -> b \"flows\" .loud\n");
}

#[test]
fn endpoint_dot_path_and_side() {
    assert_eq!(fmt("a.b:left -> c\n"), "a.b:left -> c\n");
}

#[test]
fn phases_separated_by_a_blank_line() {
    assert_eq!(
        fmt("{|box|{radius:4}}\n|box#x|\na -> b\n"),
        "{\n  |box| { radius: 4; }\n}\n\n|box#x|\n\na -> b\n"
    );
}

#[test]
fn interleaved_body_keeps_source_order() {
    // [SPEC 3]: a child after a link in a body stays put (a `layout: sequence`
    // reads this order as time) — the formatter must not reorder to children-then-links.
    assert_eq!(
        fmt("|group#g| [\n  a -> b\n  |box#m|\n  m -> a\n]\n"),
        "|group#g| [\n  a -> b\n  |box#m|\n  m -> a\n]\n"
    );
}

#[test]
fn interleaved_root_keeps_source_order_no_phase_break() {
    // A root `layout: sequence` interleaves participants and messages; the
    // canvas/links blank-line split applies only to a cleanly phased file.
    assert_eq!(
        fmt("{layout:sequence}\n|box#a|\na -> b\n|loop#l| [ b -> a ]\n"),
        "{\n  layout: sequence;\n}\n\n|box#a|\na -> b\n|loop#l| [\n  b -> a\n]\n"
    );
}

#[test]
fn comments_are_preserved() {
    assert_eq!(fmt("// header\n|box#x|\n"), "// header\n|box#x|\n");
}

#[test]
fn a_blank_line_grouping_survives() {
    assert_eq!(fmt("|box#a|\n\n|box#b|\n"), "|box#a|\n\n|box#b|\n");
}

#[test]
fn runs_of_blank_lines_collapse_to_one() {
    assert_eq!(fmt("|box#a|\n\n\n\n|box#b|\n"), "|box#a|\n\n|box#b|\n");
}

#[test]
fn table_cells_align_into_columns() {
    // [SPEC 8/16]: a |table|'s bare-text cells align, each column padded to its
    // widest cell; the track list lives in the style block.
    let out = "|table#t| { columns: 80, 80; } [\n  \"A\"     \"Quantity\"\n  \"Apple\" \"3\"\n]\n";
    assert_eq!(
        fmt("|table#t|{columns:80, 80}[\n\"A\" \"Quantity\"\n\"Apple\" \"3\"\n]\n"),
        out
    );
    idempotent(out);
}

#[test]
fn a_comma_data_list_prints_the_law() {
    // [SPEC 2]: comma-groups re-emit comma-separated, spaces within a group —
    // `data: 9, 15, 24` round-trips; point pairs keep their internal space.
    idempotent("|chart#c| [\n  |bars| { data: 9, 15, 24; }\n  |dots| { data: 10 20, 30 40; }\n]\n");
    assert_eq!(
        fmt("|bars#b|{data:9,15,24}\n"),
        "|bars#b| { data: 9, 15, 24; }\n"
    );
}

#[test]
fn a_styled_table_cell_keeps_its_block_and_breaks_its_row_out() {
    // [SPEC 19]: a cell's `{ }` must survive fmt (dropping it is silent data loss);
    // its whole row leaves the alignment grid, while the plain rows stay aligned.
    let out = "|table#t| { columns: 80, 80; } [\n  \"A\"     \"Qty\"\n  \"Apple\" { color: red; } \"3\"\n  \"Mango\" \"5\"\n]\n";
    assert_eq!(
        fmt(
            "|table#t|{columns:80, 80}[\n\"A\" \"Qty\"\n\"Apple\"{color:red} \"3\"\n\"Mango\" \"5\"\n]\n"
        ),
        out
    );
    idempotent(out);
}

#[test]
fn a_comment_between_style_and_children_lands_in_the_children() {
    // The style block ends at its own `}`; trivia after it belongs to the `[ ]`.
    assert_eq!(
        fmt("|box#p| { fill: red } [\n  // kids\n  |oval#a|\n]\n"),
        "|box#p| { fill: red; } [\n  // kids\n  |oval#a|\n]\n"
    );
}

#[test]
fn idempotence_and_reparse_over_a_rich_file() {
    let src = "\
{
layout: grid;  columns: repeat(3);  gap: 40;
--accent: #0a84ff;
|box| { radius: 4; }
|treat::box| { radius: 5; }
.loud { stroke: red; stroke-width: 2; }
}

|oval#cat| \"Cat\" { cell: 1 1 }
|group#kitchen| { direction: column } [
|caption| \"Kitchen\"
|treat#bowl| \"Bowl\"
|box#water| \"Water\"
bowl -> water \"flows\"
]

cat -> kitchen.bowl .loud
";
    idempotent(src);
    reparses(src);
}

// ───────── The drawing statements [SPEC 15/19] ─────────

#[test]
fn draw_gets_its_own_paragraph_and_wraps_at_the_budget() {
    // The pen never shares a line with another declaration; calls flow to the
    // line budget and continuations align under the first call.
    let src = "|sketch#bar| { draw: move(-150, 0) up(10) chamfer(1.5) right(40):thread point():a right(260) chamfer(1.5) down(10); mirror: x-axis; }\n";
    let out = fmt(src);
    assert_eq!(
        out,
        "|sketch#bar| {\n  draw: move(-150, 0) up(10) chamfer(1.5) right(40):thread point():a right(260)\n        chamfer(1.5) down(10);\n  mirror: x-axis;\n}\n"
    );
    idempotent(src);
}

#[test]
fn each_move_starts_its_own_subpath_line() {
    let src = "|sketch#plate| { draw: move(0, 0) right(60) close() move(20, 15) circle(6); }\n";
    let out = fmt(src);
    assert_eq!(
        out,
        "|sketch#plate| {\n  draw: move(0, 0) right(60) close()\n        move(20, 15) circle(6);\n}\n"
    );
    idempotent(src);
}

#[test]
fn a_short_single_subpath_draw_still_inlines() {
    let src = "|sketch#s| { draw: move(0, 0) right(10); }\n";
    assert_eq!(fmt(src), "|sketch#s| { draw: move(0, 0) right(10); }\n");
}

#[test]
fn mates_and_measures_format_like_links() {
    // The drawing ops are ordinary link statements to the formatter: the op
    // between two-ended groups, after a one-ended group [SPEC 15.6/21].
    let src = "a:left||b:right{gap:-10}\nbar:thread   (o)   { side: left; tol: h6 }\nbar:left (-) bar:right{side:bottom}\nbolt <- \"THRU\"\n";
    let out = fmt(src);
    assert_eq!(
        out,
        "a:left || b:right { gap: -10; }\nbar:thread (o) { side: left; tol: h6; }\nbar:left (-) bar:right { side: bottom; }\nbolt <- \"THRU\"\n"
    );
    idempotent(src);
}

#[test]
fn a_carried_annotation_node_rides_the_label_block_multi_line() {
    // A `[ ]` holding a node goes multi-line [SPEC 15.9/21]; texts and nodes
    // keep source order, and the round-trip is idempotent.
    let src = "a:left (-) a:right [ \"W\" |feature-control| \"flatness\" { tol: 0.1 } ]\n";
    let out = fmt(src);
    assert_eq!(
        out,
        "a:left (-) a:right [\n  \"W\"\n  |feature-control| \"flatness\" { tol: 0.1; }\n]\n"
    );
    idempotent(src);
    reparses(src);
}

#[test]
fn a_text_only_label_block_stays_inline() {
    let src = "a -> b [ \"x\" \"y\" ]\n";
    assert_eq!(fmt(src), "a -> b [ \"x\" \"y\" ]\n");
    idempotent(src);
}

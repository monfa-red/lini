use super::format;

fn fmt(src: &str) -> String {
    format(src).expect("format")
}

/// fmt output must re-parse cleanly (it is valid).
fn reparses(src: &str) {
    let out = fmt(src);
    let toks = crate::lexer::lex(&out).expect("lex fmt output");
    crate::syntax::parser::parse(&toks).expect("parse fmt output");
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
fn node_label_trails_in_terse_form() {
    // A text `[ ]` contracts to a trailing label (SPEC §3); a trailing label stays.
    assert_eq!(fmt("x|box|[\"hi\"]\n"), "x |box| \"hi\"\n");
    assert_eq!(fmt("x|box| \"hi\"\n"), "x |box| \"hi\"\n");
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
fn descendant_rule() {
    assert_eq!(
        fmt("{|table box|{padding:4 8}}\n"),
        "{\n  |table box| { padding: 4 8; }\n}\n"
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
        fmt("dim|line|{points:0 0,10 10}\n"),
        "dim |line| { points: 0 0, 10 10; }\n"
    );
}

#[test]
fn function_value() {
    assert_eq!(
        fmt("{layout:grid\ncolumns:repeat(3)}\n"),
        "{\n  layout: grid; columns: repeat(3);\n}\n"
    );
}

#[test]
fn classes_glue_into_the_bars() {
    assert_eq!(fmt("x|box.hot.loud|\n"), "x |box.hot.loud|\n");
}

#[test]
fn node_with_style_and_children() {
    assert_eq!(
        fmt("g|group|{layout:column}[\na|box|\nb|box|\n]\n"),
        "g |group| { layout: column; } [\n  a |box|\n  b |box|\n]\n"
    );
}

#[test]
fn block_declarations_group_on_one_line() {
    // SPEC §20: config decls share a line in the style block, off the head.
    assert_eq!(
        fmt("g |group| { cell: 1 2; layout: column; gap: 16 } [\na |box|\n]\n"),
        "g |group| { cell: 1 2; layout: column; gap: 16; } [\n  a |box|\n]\n"
    );
}

#[test]
fn a_comment_breaks_a_declaration_group_and_forces_a_block() {
    assert_eq!(
        fmt("g |group| {\n  layout: row;\n  // note\n  gap: 10;\n} [\n  a |box|\n]\n"),
        "g |group| {\n  layout: row;\n  // note\n  gap: 10;\n} [\n  a |box|\n]\n"
    );
}

#[test]
fn simple_wire() {
    assert_eq!(fmt("a -> b\n"), "a -> b\n");
}

#[test]
fn wire_label_trails() {
    assert_eq!(fmt("a -> b \"x\"\n"), "a -> b \"x\"\n");
}

#[test]
fn wire_fan_and_chain() {
    assert_eq!(fmt("a & b -> c\n"), "a & b -> c\n");
    assert_eq!(fmt("a -> b -> c\n"), "a -> b -> c\n");
}

#[test]
fn dotted_wire_op() {
    assert_eq!(fmt("a ..> b\n"), "a ..> b\n");
    assert_eq!(fmt("a .. b\n"), "a .. b\n");
}

#[test]
fn wire_defaults_rule_uses_the_arrow_glyph() {
    assert_eq!(
        fmt("{-> {clearance:8}}\na -> b\n"),
        "{\n  -> { clearance: 8; }\n}\n\na -> b\n"
    );
}

#[test]
fn wire_class_and_labels_with_along() {
    assert_eq!(
        fmt("a -> b {along:0.3 0.7}\"near a\" \"near b\"\n"),
        "a -> b { along: 0.3 0.7; } \"near a\" \"near b\"\n"
    );
    assert_eq!(fmt("a -> b .loud\n"), "a -> b .loud\n");
}

#[test]
fn endpoint_dot_path_and_side() {
    assert_eq!(fmt("a.b.left -> c\n"), "a.b.left -> c\n");
}

#[test]
fn phases_separated_by_a_blank_line() {
    assert_eq!(
        fmt("{|box|{radius:4}}\nx|box|\na -> b\n"),
        "{\n  |box| { radius: 4; }\n}\n\nx |box|\n\na -> b\n"
    );
}

#[test]
fn comments_are_preserved() {
    assert_eq!(fmt("// header\nx|box|\n"), "// header\nx |box|\n");
}

#[test]
fn a_blank_line_grouping_survives() {
    assert_eq!(fmt("a|box|\n\nb|box|\n"), "a |box|\n\nb |box|\n");
}

#[test]
fn runs_of_blank_lines_collapse_to_one() {
    assert_eq!(fmt("a|box|\n\n\n\nb|box|\n"), "a |box|\n\nb |box|\n");
}

#[test]
fn sibling_id_and_type_columns_align() {
    assert_eq!(
        fmt("g|group|[\nbowl|treat| \"Bowl\"\nwater|box| \"Water\"\n]\n"),
        "g |group| [\n  bowl  |treat| \"Bowl\"\n  water |box|   \"Water\"\n]\n"
    );
}

#[test]
fn a_blank_line_breaks_an_alignment_group() {
    assert_eq!(
        fmt("bowl|box|\n\nwater|box|\n"),
        "bowl |box|\n\nwater |box|\n"
    );
}

#[test]
fn table_cells_align_into_columns() {
    // SPEC §8/§14: a |table|'s bare-text cells align, each column padded to its
    // widest cell; the track list lives in the style block.
    let out = "t |table| { columns: 80 80; } [\n  \"A\"     \"Quantity\"\n  \"Apple\" \"3\"\n]\n";
    assert_eq!(
        fmt("t|table|{columns:80 80}[\n\"A\" \"Quantity\"\n\"Apple\" \"3\"\n]\n"),
        out
    );
    idempotent(out);
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

cat |oval| { cell: 1 1 } \"Cat\"
kitchen |group| { layout: column } [
|caption| \"Kitchen\"
bowl |treat| \"Bowl\"
water |box| \"Water\"
bowl -> water \"flows\"
]

cat -> kitchen.bowl .loud
";
    idempotent(src);
    reparses(src);
}

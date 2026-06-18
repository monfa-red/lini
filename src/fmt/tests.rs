use super::format;

fn fmt(src: &str) -> String {
    format(src).expect("format")
}

/// fmt output must re-parse cleanly (it is valid v4).
fn reparses(src: &str) {
    let out = fmt(src);
    let toks = crate::lexer::lex(&out).expect("lex fmt output");
    crate::syntax::parser::parse(&toks).expect("parse fmt output");
}

/// The core invariant: a second pass changes nothing.
fn idempotent(src: &str) {
    let once = fmt(src);
    let twice = fmt(&once);
    assert_eq!(once, twice, "not idempotent:\n--- once ---\n{once}\n--- twice ---\n{twice}");
}

#[test]
fn node_id_type_and_label_block() {
    // The label is a text child in the block; a short block collapses inline.
    assert_eq!(fmt("x|box|{\"hi\"}\n"), "x |box| { \"hi\" }\n");
}

#[test]
fn bare_label_node() {
    assert_eq!(fmt("\"Apple\"\n"), "\"Apple\"\n");
}

#[test]
fn root_declaration() {
    assert_eq!(fmt("layout:grid\n"), "layout: grid;\n");
}

#[test]
fn variable_declaration() {
    assert_eq!(fmt("--brand:#ff6600\n"), "--brand: #ff6600;\n");
}

#[test]
fn element_rule() {
    assert_eq!(fmt("box{radius:6}\n"), "box { radius: 6; }\n");
}

#[test]
fn class_rule() {
    assert_eq!(fmt(".hot{stroke-width:2}\n"), ".hot { stroke-width: 2; }\n");
}

#[test]
fn descendant_rule() {
    assert_eq!(fmt("table box{padding:4 8}\n"), "table box { padding: 4 8; }\n");
}

#[test]
fn define() {
    assert_eq!(fmt("treat::box{radius:5}\n"), "treat::box { radius: 5; }\n");
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
        fmt("layout:grid\ncolumns:repeat(3)\n"),
        "layout: grid; columns: repeat(3);\n"
    );
}

#[test]
fn block_declarations_group_on_one_line() {
    // SPEC §20: leading config decls share a line, off the opening brace.
    assert_eq!(
        fmt("g |group| { cell: 1 2; layout: column; gap: 16;\n  a |box|\n}\n"),
        "g |group| {\n  cell: 1 2; layout: column; gap: 16;\n  a |box|\n}\n"
    );
}

#[test]
fn a_comment_breaks_a_declaration_group() {
    assert_eq!(
        fmt("g |group| {\n  layout: row;\n  // note\n  gap: 10;\n  a |box|\n}\n"),
        "g |group| {\n  layout: row;\n  // note\n  gap: 10;\n  a |box|\n}\n"
    );
}

#[test]
fn node_with_block_and_children() {
    assert_eq!(
        fmt("g|group|{layout:column\na|box|\nb|box|}\n"),
        "g |group| {\n  layout: column;\n  a |box|\n  b |box|\n}\n"
    );
}

#[test]
fn classes_on_a_node() {
    assert_eq!(fmt("x|box|.hot.loud\n"), "x |box| .hot .loud\n");
}

#[test]
fn simple_wire() {
    assert_eq!(fmt("a -> b\n"), "a -> b\n");
}

#[test]
fn wire_label_collapses_inline() {
    assert_eq!(fmt("a -> b {\"x\"}\n"), "a -> b { \"x\" }\n");
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
    assert_eq!(fmt("-> {clearance:8}\na -> b\n"), "-> { clearance: 8; }\n\na -> b\n");
}

#[test]
fn wire_labels_with_along() {
    assert_eq!(
        fmt("a -> b {along:0.3 0.7\n\"near a\" \"near b\"}\n"),
        "a -> b { along: 0.3 0.7; \"near a\" \"near b\" }\n"
    );
}

#[test]
fn endpoint_dot_path_and_side() {
    assert_eq!(fmt("a.b.left -> c\n"), "a.b.left -> c\n");
}

#[test]
fn phases_separated_by_a_blank_line() {
    assert_eq!(
        fmt("box{radius:4}\nx|box|\na -> b\n"),
        "box { radius: 4; }\n\nx |box|\n\na -> b\n"
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
        fmt("g|group|{bowl|treat|{\"Bowl\"}\nwater|box|{\"Water\"}}\n"),
        "g |group| {\n  bowl  |treat| { \"Bowl\" }\n  water |box|   { \"Water\" }\n}\n"
    );
}

#[test]
fn a_blank_line_breaks_an_alignment_group() {
    // The two nodes are in separate groups, so their columns don't align.
    assert_eq!(
        fmt("bowl|box|\n\nwater|box|\n"),
        "bowl |box|\n\nwater |box|\n"
    );
}

#[test]
fn table_cells_align_into_columns() {
    // SPEC §8/§14: a multi-row |table| breaks into rows with each column padded
    // to its widest cell.
    assert_eq!(
        fmt("t|table|{columns:80 80\n\"A\" \"Quantity\"\n\"Apple\" \"3\"}\n"),
        "t |table| {\n  columns: 80 80;\n  \"A\"     \"Quantity\"\n  \"Apple\" \"3\"\n}\n"
    );
    idempotent("t |table| {\n  columns: 80 80;\n  \"A\"     \"Quantity\"\n  \"Apple\" \"3\"\n}\n");
}

#[test]
fn idempotence_and_reparse_over_a_rich_file() {
    let src = "\
layout: grid;  columns: repeat(3);  gap: 40;
--accent: #0a84ff;
box { radius: 4; }
treat::box { radius: 5; }
.loud { stroke: red; stroke-width: 2; }

cat |oval| { cell: 1 1; \"Cat\" }
kitchen |group| {
  |caption| { \"Kitchen\" }
  bowl |treat| { \"Bowl\" }
  water |box| { \"Water\" }
  bowl -> water { \"flows\" }
}

cat -> kitchen.bowl .loud
";
    idempotent(src);
    reparses(src);
}

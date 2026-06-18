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
fn node_id_type_label() {
    assert_eq!(fmt("x|rect|\"hi\"\n"), "x |rect| \"hi\"\n");
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
    assert_eq!(fmt("rect{radius:6}\n"), "rect { radius: 6; }\n");
}

#[test]
fn class_rule() {
    assert_eq!(fmt(".hot{stroke-width:2}\n"), ".hot { stroke-width: 2; }\n");
}

#[test]
fn descendant_rule() {
    assert_eq!(fmt("table rect{padding:4 8}\n"), "table rect { padding: 4 8; }\n");
}

#[test]
fn define() {
    assert_eq!(fmt("treat::rect{radius:5}\n"), "treat::rect { radius: 5; }\n");
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
        fmt("g |group| { cell: 1 2; layout: column; gap: 16;\n  a |rect|\n}\n"),
        "g |group| {\n  cell: 1 2; layout: column; gap: 16;\n  a |rect|\n}\n"
    );
}

#[test]
fn a_comment_breaks_a_declaration_group() {
    assert_eq!(
        fmt("g |group| {\n  layout: row;\n  // note\n  gap: 10;\n  a |rect|\n}\n"),
        "g |group| {\n  layout: row;\n  // note\n  gap: 10;\n  a |rect|\n}\n"
    );
}

#[test]
fn node_with_block_and_children() {
    assert_eq!(
        fmt("g|group|{layout:column\na|rect|\nb|rect|}\n"),
        "g |group| {\n  layout: column;\n  a |rect|\n  b |rect|\n}\n"
    );
}

#[test]
fn classes_on_a_node() {
    assert_eq!(fmt("x|rect|.hot.loud\n"), "x |rect| .hot .loud\n");
}

#[test]
fn simple_wire() {
    assert_eq!(fmt("a -> b \"x\"\n"), "a -> b \"x\"\n");
}

#[test]
fn wire_fan_and_chain() {
    assert_eq!(fmt("a & b -> c\n"), "a & b -> c\n");
    assert_eq!(fmt("a -> b -> c\n"), "a -> b -> c\n");
}

#[test]
fn wire_with_text_children() {
    assert_eq!(
        fmt("a -> b {|text|\"watches\"{at:0.5}}\n"),
        "a -> b {\n  |text| \"watches\" { at: 0.5; }\n}\n"
    );
}

#[test]
fn endpoint_dot_path_and_side() {
    assert_eq!(fmt("a.b.left -> c\n"), "a.b.left -> c\n");
}

#[test]
fn phases_separated_by_a_blank_line() {
    assert_eq!(
        fmt("rect{radius:4}\nx|rect|\na -> b\n"),
        "rect { radius: 4; }\n\nx |rect|\n\na -> b\n"
    );
}

#[test]
fn comments_are_preserved() {
    assert_eq!(fmt("// header\nx|rect|\n"), "// header\nx |rect|\n");
}

#[test]
fn a_blank_line_grouping_survives() {
    assert_eq!(fmt("a|rect|\n\nb|rect|\n"), "a |rect|\n\nb |rect|\n");
}

#[test]
fn runs_of_blank_lines_collapse_to_one() {
    assert_eq!(fmt("a|rect|\n\n\n\nb|rect|\n"), "a |rect|\n\nb |rect|\n");
}

#[test]
fn sibling_id_and_type_columns_align() {
    assert_eq!(
        fmt("g|group|{bowl|treat|\"Bowl\"\nwater|rect|\"Water\"}\n"),
        "g |group| {\n  bowl  |treat| \"Bowl\"\n  water |rect|  \"Water\"\n}\n"
    );
}

#[test]
fn a_blank_line_breaks_an_alignment_group() {
    // The two nodes are in separate groups, so their columns don't align.
    assert_eq!(
        fmt("bowl|rect|\n\nwater|rect|\n"),
        "bowl |rect|\n\nwater |rect|\n"
    );
}

#[test]
fn idempotence_and_reparse_over_a_rich_file() {
    let src = "\
layout: grid;  columns: repeat(3);  gap: 40;
--accent: #0a84ff;
rect { radius: 4; }
treat::rect { radius: 5; }
.loud { stroke: red; stroke-width: 2; }

cat |oval| \"Cat\" { cell: 1 1; }
kitchen |group| \"Kitchen\" {
  bowl |treat| \"Bowl\"
  water |rect| \"Water\"
  bowl -> water \"flows\"
}

cat -> kitchen.bowl .loud
";
    idempotent(src);
    reparses(src);
}

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
fn node_head_label() {
    // A lone text `[ ]` contracts to the head label (SPEC §3); a head label stays.
    assert_eq!(fmt("|box#x|[\"hi\"]\n"), "|box#x| \"hi\"\n");
    assert_eq!(fmt("|box#x| \"hi\"\n"), "|box#x| \"hi\"\n");
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
        fmt("{layout:grid\ncolumns:repeat(3)}\n"),
        "{\n  layout: grid; columns: repeat(3);\n}\n"
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
        fmt("|group#g|{layout:column}[\n|box#a|\n|box#b|\n]\n"),
        "|group#g| { layout: column; } [\n  |box#a|\n  |box#b|\n]\n"
    );
}

#[test]
fn block_declarations_group_on_one_line() {
    // SPEC §20: config decls share a line in the style block, off the head.
    assert_eq!(
        fmt("|group#g| { cell: 1 2; layout: column; gap: 16 } [\n|box#a|\n]\n"),
        "|group#g| { cell: 1 2; layout: column; gap: 16; } [\n  |box#a|\n]\n"
    );
}

#[test]
fn a_comment_breaks_a_declaration_group_and_forces_a_block() {
    assert_eq!(
        fmt("|group#g| {\n  layout: row;\n  // note\n  gap: 10;\n} [\n  |box#a|\n]\n"),
        "|group#g| {\n  layout: row;\n  // note\n  gap: 10;\n} [\n  |box#a|\n]\n"
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
        fmt("a -> b {along:0.3 0.7}[ \"near a\" \"near b\" ]\n"),
        "a -> b { along: 0.3 0.7; } [ \"near a\" \"near b\" ]\n"
    );
    assert_eq!(fmt("a -> b .loud\n"), "a -> b .loud\n");
    // A spaced link-class chain normalizes to glued, like a node's.
    assert_eq!(fmt("a -> b .c1 .c2\n"), "a -> b .c1.c2\n");
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
    // SPEC §8/§14: a |table|'s bare-text cells align, each column padded to its
    // widest cell; the track list lives in the style block.
    let out = "|table#t| { columns: 80 80; } [\n  \"A\"     \"Quantity\"\n  \"Apple\" \"3\"\n]\n";
    assert_eq!(
        fmt("|table#t|{columns:80 80}[\n\"A\" \"Quantity\"\n\"Apple\" \"3\"\n]\n"),
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
|group#kitchen| { layout: column } [
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

use super::format;

fn fmt(src: &str) -> String {
    format(src).expect("format")
}

/// fmt output must re-parse cleanly (it is valid).
fn reparses(src: &str) {
    let out = fmt(src);
    let toks = crate::lexer::lex(&out).expect("lex fmt output");
    crate::syntax::parser::parse(&out, &toks).expect("parse fmt output");
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

/// A canonical spelling is a contract: every form the formatter owns, as
/// `(written, canonical)`. Each row is also re-parsed and run a second time —
/// valid output that a save-loop cannot churn.
#[track_caller]
fn canonicalizes(rows: &[(&str, &str)]) {
    for (src, want) in rows {
        assert_eq!(&fmt(src), want, "canonical form of {src:?}");
        reparses(src);
        idempotent(src);
    }
}

#[test]
fn fmt_canonicalizes_a_node() {
    canonicalizes(&[
        // A head label is preserved; a `[ ]` text child is left as written (fmt
        // resolves no types, and the head label's meaning is type-dependent
        // [SPEC 3]).
        ("|box#x| \"hi\"\n", "|box#x| \"hi\"\n"),
        ("|box#x|[ \"hi\" ]\n", "|box#x| [ \"hi\" ]\n"),
        ("|#cat|\n", "|#cat|\n"),
        ("\"Apple\"\n", "\"Apple\"\n"),
        ("|box#x| .hot.loud\n", "|box#x| .hot.loud\n"),
        // A class on a default box (id only).
        ("|#x| .hot\n", "|#x| .hot\n"),
        (
            "|box#api| \"API\" .hot{fill:red}\n",
            "|box#api| \"API\" .hot { fill: red; }\n",
        ),
        (
            "|group#g|{direction:column}[\n|box#a|\n|box#b|\n]\n",
            "|group#g| { direction: column; } [\n  |box#a|\n  |box#b|\n]\n",
        ),
        // [SPEC 20]: config decls share a line in the style block, off the head.
        (
            "|group#g| { cell: 1 2; direction: column; gap: 16 } [\n|box#a|\n]\n",
            "|group#g| { cell: 1 2; direction: column; gap: 16; } [\n  |box#a|\n]\n",
        ),
        // …but a comment breaks the group and forces a block.
        (
            "|group#g| {\n  direction: row;\n  // note\n  gap: 10;\n} [\n  |box#a|\n]\n",
            "|group#g| {\n  direction: row;\n  // note\n  gap: 10;\n} [\n  |box#a|\n]\n",
        ),
    ]);
}

#[test]
fn fmt_canonicalizes_a_stylesheet() {
    canonicalizes(&[
        ("{layout:grid}\n", "{\n  layout: grid;\n}\n"),
        ("{--brand:#ff6600}\n", "{\n  --brand: #ff6600;\n}\n"),
        ("{|box|{radius:6}}\n", "{\n  |box| { radius: 6; }\n}\n"),
        (
            "{.hot{stroke-width:2}}\n",
            "{\n  .hot { stroke-width: 2; }\n}\n",
        ),
        ("{#hero{fill:gold}}\n", "{\n  #hero { fill: gold; }\n}\n"),
        (
            "{|table| |box|{padding:4 8}}\n",
            "{\n  |table| |box| { padding: 4 8; }\n}\n",
        ),
        (
            "{|table#main| |box|{fill:white}}\n",
            "{\n  |table#main| |box| { fill: white; }\n}\n",
        ),
        (
            "{|treat::box|{radius:5}}\n",
            "{\n  |treat::box| { radius: 5; }\n}\n",
        ),
        // v4 values are space-separated within a group, comma between groups.
        (
            "|line#dim|{points:0 0,10 10}\n",
            "|line#dim| { points: 0 0, 10 10; }\n",
        ),
        (
            "{layout:grid;\ncolumns:repeat(3)}\n",
            "{\n  layout: grid; columns: repeat(3);\n}\n",
        ),
        // A scalar binding reads bare; a function's body is a group [SPEC 10.7].
        ("{my_r=5}\n", "{\n  my_r = 5;\n}\n"),
        (
            "{scale(n)=(100 * 1.2 ^ n)}\n",
            "{\n  scale(n) = (100 * 1.2 ^ n);\n}\n",
        ),
        // A direct group value wears its parens; a call argument sheds them (it
        // is already inside the call's own parens).
        (
            "|box#a| { padding: (8 * 2); width: gain(2 * n) }\n",
            "|box#a| { padding: (8 * 2); width: gain(2 * n); }\n",
        ),
    ]);
    idempotent(
        "{ my_r = 5; scale(n) = (100 * 1.2 ^ n); }\n|box#a| { padding: (8 * 2); width: gain(2 * n) }\n",
    );
}

#[test]
fn fmt_canonicalizes_a_link() {
    canonicalizes(&[
        ("a -> b\n", "a -> b\n"),
        ("a -> b \"x\"\n", "a -> b \"x\"\n"),
        ("a & b -> c\n", "a & b -> c\n"),
        ("a -> b -> c\n", "a -> b -> c\n"),
        ("a --> b\n", "a --> b\n"),
        ("a ---> b\n", "a ---> b\n"),
        ("a ~> b\n", "a ~> b\n"),
        (
            "a -> b {along:0.3, 0.7}[ \"near a\" \"near b\" ]\n",
            "a -> b { along: 0.3, 0.7; } [ \"near a\" \"near b\" ]\n",
        ),
        ("a -> b .loud\n", "a -> b .loud\n"),
        ("a -> b .c1.c2\n", "a -> b .c1.c2\n"),
        // A head label precedes the class (the tail order, re-parseable).
        ("a -> b \"flows\" .loud\n", "a -> b \"flows\" .loud\n"),
        ("a.b:left -> c\n", "a.b:left -> c\n"),
    ]);
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
fn a_trailing_comment_stays_on_its_own_statement() {
    // It annotates the item it follows; replaying it on a fresh line would
    // silently re-point it at the next one [SPEC 20].
    assert_eq!(
        fmt("|box#a| \"A\"   // the hero\n|box#b| \"B\"\n"),
        "|box#a| \"A\"  // the hero\n|box#b| \"B\"\n"
    );
    // A declaration keeps its own annotation, so a constants table survives.
    assert_eq!(
        fmt("{\n  w = 10;   // the width\n}\n|box#a|\n"),
        "{\n  w = 10;  // the width\n}\n\n|box#a|\n"
    );
}

#[test]
fn a_trailing_comment_survives_a_phase_break() {
    // The blank line between the instances and the links is pushed *after* the
    // statement, so reattaching by trimming what came since would eat the break
    // and re-point the comment at the next phase — the bug it was fixing.
    let once = fmt("|box#a| \"A\"\n|box#b| \"B\"  // hero\na -> b\n");
    assert_eq!(once, "|box#a| \"A\"\n|box#b| \"B\"  // hero\n\na -> b\n");
    assert_eq!(fmt(&once), once, "and formatting is idempotent");

    // Same across the stylesheet/canvas break.
    let sheet = fmt("{\n  direction: column;  // config\n}\n|box#a| \"A\"\n");
    assert_eq!(
        sheet,
        "{\n  direction: column;  // config\n}\n\n|box#a| \"A\"\n"
    );
    assert_eq!(fmt(&sheet), sheet);
}

#[test]
fn a_comment_opening_its_line_stays_leading() {
    assert_eq!(
        fmt("|box#a| \"A\"\n// about b\n|box#b| \"B\"\n"),
        "|box#a| \"A\"\n// about b\n|box#b| \"B\"\n"
    );
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
fn an_entity_aligns_on_its_bundles_columns() {
    // [SPEC 8/16]: the column count fmt aligns on is the one the grid sugar
    // reads — the node's own `columns:` (last wins), else its template chain's
    // bundle. An |entity| never spells out `columns: auto, auto`, so counting
    // only its own style left its cells unaligned while an identical |table|
    // aligned.
    let out = "|entity#e| \"Users\" [\n  \"id\"   \"int\"\n  \"name\" \"text\"\n]\n";
    assert_eq!(
        fmt("|entity#e| \"Users\" [\n\"id\" \"int\"\n\"name\" \"text\"\n]\n"),
        out
    );
    idempotent(out);
    // A define over |entity| inherits the same count through its chain.
    let out =
        "{\n  |myent::entity|\n}\n\n|myent#f| [\n  \"id\"   \"int\"\n  \"name\" \"text\"\n]\n";
    assert_eq!(
        fmt("{ |myent::entity| {} }\n|myent#f| [\n\"id\" \"int\"\n\"name\" \"text\"\n]\n"),
        out
    );
    idempotent(out);
    // A repeated `columns:` counts the last, as the cascade does.
    let out =
        "|table#t| { columns: 80; columns: 80, 80; } [\n  \"a\"   \"b\"\n  \"ccc\" \"d\"\n]\n";
    assert_eq!(
        fmt("|table#t| { columns: 80; columns: 80, 80 } [\n\"a\" \"b\"\n\"ccc\" \"d\"\n]\n"),
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
    // [SPEC 20]: a cell's `{ }` must survive fmt (dropping it is silent data loss);
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

#[test]
fn a_comment_inside_a_link_label_block_survives() {
    // [SPEC 20]: comments are preserved everywhere — a link's `[ ]` is a body
    // like a node's, so it breaks multi-line to keep the note…
    let src = "a -> b [\n  // why\n  \"x\"\n  \"y\"\n]\n";
    assert_eq!(fmt(src), src);
    idempotent(src);
    // …and a lone label does not contract to the head label and swallow it.
    let one = "a -> b [\n  // why\n  \"x\"\n]\n";
    assert_eq!(fmt(one), one);
    idempotent(one);
}

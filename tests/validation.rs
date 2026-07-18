//! The property-validation pass and the lint warnings [SPEC 16/20]: one insta
//! family per diagnostic, snapshotting the exact CLI-rendered message, plus
//! the silent cases that gate false positives.

fn diags(src: &str) -> String {
    lini::lint_str(src)
        .expect("parse")
        .iter()
        .map(|d| d.display_with_source(src, "test.lini").to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

#[track_caller]
fn assert_silent(src: &str) {
    let d = diags(src);
    assert!(d.is_empty(), "expected no diagnostics, got:\n{d}");
}

// ── Unknown property name — an error, everywhere ──

#[test]
fn unknown_property_suggests_the_nearest_name() {
    insta::assert_snapshot!(
        diags("|box#a| { colr: red; }\n"),
        @"test.lini:1:11: error: unknown property 'colr'; did you mean 'color'?"
    );
}

#[test]
fn unknown_property_errors_even_in_a_class_rule() {
    insta::assert_snapshot!(
        diags("{ .hot { colr: red; } }\n|box#a| .hot\n"),
        @"test.lini:1:10: error: unknown property 'colr'; did you mean 'color'?"
    );
}

// ── Misused property, wearer statically known — an error with a correction ──

#[test]
fn type_owned_property_on_the_wrong_type_errors() {
    insta::assert_snapshot!(
        diags("|box#a| { points: 0 0, 1 1; }\n"),
        @"test.lini:1:11: error: 'points' has no meaning on '|box|' — it reads on '|line|' / '|poly|'"
    );
}

#[test]
fn misuse_errors_in_an_element_rule_too() {
    insta::assert_snapshot!(
        diags("{ |box| { symbol: heart; } }\n|box#a|\n"),
        @"test.lini:1:11: error: 'symbol' has no meaning on '|box|' — it reads on '|icon|'"
    );
}

#[test]
fn series_data_off_a_series_errors() {
    insta::assert_snapshot!(
        diags("|box#a| { data: 1, 2; }\n"),
        @"test.lini:1:11: error: 'data' has no meaning on '|box|' — it reads on a chart series"
    );
}

#[test]
fn cell_off_a_grid_errors_with_the_context() {
    insta::assert_snapshot!(
        diags("|row#r| [ |box#a| { cell: 2 1; } ]\n"),
        @"test.lini:1:21: error: 'cell' places a grid child — this box sits in a 'layout: flow'"
    );
}

#[test]
fn cell_in_a_grid_is_silent() {
    assert_silent("{ layout: grid; columns: 80, 80; }\n|box#a| { cell: 2 1; }\n");
}

#[test]
fn cell_stays_silent_when_a_rule_could_set_the_layout() {
    // A stylesheet rule injects `layout:` — the parent's layout is no longer
    // statically known, so the strict check stands down.
    assert_silent(
        "{ |row| { layout: grid; columns: 80, 80; } }\n|row#r| [ |box#a| { cell: 2 1; } ]\n",
    );
}

#[test]
fn sequence_placement_off_a_sequence_errors() {
    insta::assert_snapshot!(
        diags("|row#r| [ |note#n| \"hi\" { place: over a; } ]\n"),
        @"test.lini:1:27: error: 'place' is valid only in a 'layout: sequence'"
    );
}

#[test]
fn density_off_the_root_errors() {
    insta::assert_snapshot!(
        diags("|box#a| { density: 2; }\n"),
        @"test.lini:1:11: error: 'density' is scene config — set it in the root block"
    );
}

#[test]
fn container_scoped_property_reads_on_a_matching_root() {
    // `unit:` is `|drawing|`-owned; a `layout: drawing` root is that scope.
    assert_silent("{ layout: drawing; unit: mm; }\n|rect#p| { width: 40; height: 20; }\n");
}

// ── Class rules: CSS semantics — inert is fine, dead-everywhere warns ──

#[test]
fn a_class_dead_on_every_wearer_warns() {
    insta::assert_snapshot!(
        diags("{ .geo { points: 0 0, 5 5; } }\n|box#a| .geo\n"),
        @"test.lini:1:10: warning: '.geo { points: … }' is inert on every wearer"
    );
}

#[test]
fn a_class_usable_by_one_wearer_is_silent() {
    assert_silent("{ .geo { points: 0 0, 5 5; } }\n|box#a| .geo\n|line#l| .geo\n");
}

#[test]
fn a_text_valid_class_worn_only_by_text_is_live() {
    // A text leaf is a wearer [SPEC 3]: a text-valid property is live on it, so
    // the class is neither dead nor never-worn.
    assert_silent("{ .card-title { font-size: 17; } }\n\"Starter\" .card-title\n");
}

#[test]
fn a_box_only_class_worn_only_by_text_warns_dead() {
    // The class-polymorphism law: `padding` is inert on the only wearer (text),
    // so it warns dead-on-every-wearer.
    insta::assert_snapshot!(
        diags("{ .card { padding: 40; } }\n\"Starter\" .card\n"),
        @"test.lini:1:11: warning: '.card { padding: … }' is inert on every wearer"
    );
}

#[test]
fn a_never_worn_class_warns() {
    insta::assert_snapshot!(
        diags("{ .hot { fill: red; } }\n|box#a|\n"),
        @"test.lini:1:3: warning: class '.hot' is never worn"
    );
}

// ── Malformed values the ledger judges statically ──

#[test]
fn opacity_out_of_range_errors() {
    insta::assert_snapshot!(
        diags("|box#a| { opacity: 3; }\n"),
        @"test.lini:1:11: error: 'opacity' is a fraction 0..1"
    );
}

#[test]
fn wavy_on_a_node_errors() {
    // `wavy` is link-only by design [SPEC 16]; a link keeps it.
    insta::assert_snapshot!(
        diags("|box#a| { stroke-style: wavy; }\n"),
        @"test.lini:1:11: error: 'wavy' waves a link's wire — a shape's outline takes solid, dashed, dotted, center, or phantom"
    );
    assert_eq!(
        diags("|box#a|\n|box#b|\na -> b { stroke-style: wavy; }\n"),
        ""
    );
}

#[test]
fn translate_arity_errors() {
    insta::assert_snapshot!(
        diags("|box#a| { translate: 5; }\n"),
        @"test.lini:1:11: error: 'translate' takes 'x y'"
    );
}

#[test]
fn a_comma_list_on_a_one_value_property_errors() {
    insta::assert_snapshot!(
        diags("|box#a| { padding: 4, 5; }\n"),
        @"test.lini:1:11: error: 'padding' takes one value, not a comma list"
    );
}

// ── The auto-create near-miss warning [SPEC 3/20] ──

#[test]
fn a_near_miss_auto_create_warns_toward_the_target() {
    insta::assert_snapshot!(
        diags("|box#cat| \"cat\"\ncta -> bird\n"),
        @"test.lini:2:1: warning: 'cta' auto-creates a new box; did you mean 'cat'?"
    );
}

#[test]
fn a_case_slip_warns_even_past_the_typo_distance() {
    insta::assert_snapshot!(
        diags("|box#cat|\nCAT -> dog\n"),
        @"test.lini:2:1: warning: 'CAT' auto-creates a new box; did you mean 'cat'?"
    );
}

#[test]
fn distinct_implicit_names_stay_silent() {
    // The all-implicit hello-world draws no noise [SPEC 3].
    assert_silent("cat -> dog -> bird\n");
}

#[test]
fn short_distinct_ids_stay_silent() {
    // `a`/`b` are within edit distance 2 but not typos of each other — the
    // near-miss distance must be shorter than the id itself.
    assert_silent("|box#a| \"A\"\na -> b\n");
}

#[test]
fn a_near_miss_of_a_previously_created_name_warns() {
    // `serverr` is a typo of the previously auto-created `server`; the
    // numbered sibling `server2` is a family, not a typo — it stays silent.
    insta::assert_snapshot!(
        diags("server -> db\nserver2 -> serverr\n"),
        @"test.lini:2:12: warning: 'serverr' auto-creates a new box; did you mean 'server'?"
    );
}

// ── The CLI contract: errors always fail; --strict promotes warnings ──

#[test]
fn strict_turns_warnings_into_exit_1_and_no_warn_silences() {
    use std::process::Command;
    let bin = env!("CARGO_BIN_EXE_lini");
    let dir = std::env::temp_dir().join("lini-strict-test");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("warns.lini");
    std::fs::write(&file, "|box#cat| \"cat\"\ncta -> bird\n").unwrap();
    let run = |args: &[&str]| {
        let out = Command::new(bin).args(args).output().expect("spawn lini");
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    };
    let f = file.to_str().unwrap();

    // A warning alone: exit 0, message on stderr.
    let (code, err) = run(&[f, "-o", "/dev/null"]);
    assert_eq!(code, Some(0), "warnings don't fail a normal run: {err}");
    assert!(err.contains("did you mean 'cat'?"), "{err}");

    // --strict: the same warning is exit 1.
    let (code, err) = run(&["--strict", f, "-o", "/dev/null"]);
    assert_eq!(code, Some(1), "--strict promotes warnings: {err}");

    // --no-warn: silent, exit 0.
    let (code, err) = run(&["--no-warn", f, "-o", "/dev/null"]);
    assert_eq!(code, Some(0));
    assert!(err.is_empty(), "--no-warn silences warnings: {err}");

    // A validation error fails even under --no-warn.
    let bad = dir.join("bad.lini");
    std::fs::write(&bad, "|box#a| { colr: red; }\n").unwrap();
    let (code, err) = run(&["--no-warn", bad.to_str().unwrap(), "-o", "/dev/null"]);
    assert_eq!(code, Some(1), "validation errors always fail: {err}");
    assert!(err.contains("unknown property 'colr'"), "{err}");
}

// ── Tree structure errors [SPEC 12/20] — hard compile errors on the nested AST ──

fn tree_err(src: &str) -> String {
    lini::check(src)
        .expect_err("expected a tree structure error")
        .message
}

#[test]
fn topic_outside_a_tree_errors() {
    insta::assert_snapshot!(
        tree_err("|box#b| [\n  |topic#a| \"A\"\n]\n"),
        @"'|topic|' builds a tree — it belongs in a 'layout: tree'"
    );
}

#[test]
fn a_tree_with_no_root_topic_errors() {
    insta::assert_snapshot!(
        tree_err("|column#o| { layout: tree } [\n  |box#x|\n]\n"),
        @"a tree needs exactly one root '|topic|'"
    );
}

#[test]
fn a_tree_with_two_roots_errors_on_the_second() {
    insta::assert_snapshot!(
        tree_err("|column#o| { layout: tree } [\n  |topic#a| \"A\"\n  |topic#b| \"B\"\n]\n"),
        @"a tree has one root — '|topic|' 'B' is a second"
    );
}

#[test]
fn a_bilateral_top_side_errors() {
    // `top`/`bottom` has no meaning in a left/right split [SPEC 12/20].
    insta::assert_snapshot!(
        tree_err(
            "|column#o| { layout: tree; direction: bilateral } [\n  |topic#r| \"R\" [\n    |topic#a| \"A\" { side: top }\n  ]\n]\n"
        ),
        @"a bilateral tree grows left and right — 'side' takes left or right"
    );
}

#[test]
fn a_side_on_a_row_tree_errors() {
    // `side:` picks a bilateral half; a one-direction tree has none [SPEC 12/20].
    insta::assert_snapshot!(
        tree_err(
            "|column#o| { layout: tree; direction: row } [\n  |topic#r| \"R\" [\n    |topic#a| \"A\" { side: left }\n  ]\n]\n"
        ),
        @"'side' picks a bilateral branch's half — this tree has one growth direction"
    );
}

#[test]
fn a_deeper_bilateral_side_errors() {
    // The override is a first-level topic's; a deeper `side:` cannot repick its
    // half [SPEC 12/20].
    insta::assert_snapshot!(
        tree_err(
            "|column#o| { layout: tree; direction: bilateral } [\n  |topic#r| \"R\" [\n    |topic#a| \"A\" [ |topic#b| \"B\" { side: left } ]\n  ]\n]\n"
        ),
        @"'side' picks a bilateral branch's half — this tree has one growth direction"
    );
}

#[test]
fn a_first_level_bilateral_side_override_is_silent() {
    let src = "|column#o| { layout: tree; direction: bilateral } [\n  |topic#r| \"R\" [\n    |topic#a| \"A\" { side: left }\n    |topic#b| \"B\"\n  ]\n]\n";
    assert!(
        lini::check(src).is_ok(),
        "a first-level side override should compile"
    );
    assert_silent(src);
}

#[test]
fn a_well_formed_tree_is_silent() {
    let src = "|column#o| { layout: tree } [\n  |topic#a| \"A\" [\n    |topic#b| \"B\"\n  ]\n]\n";
    assert!(lini::check(src).is_ok(), "a valid tree should compile");
    assert_silent(src);
}

#[test]
fn scoped_topic_ids_stay_legal_across_sealed_bodies() {
    // Topic nesting is preserved, so two `#note`s in different topic bodies are
    // distinct by their dot-path — no duplicate-id error [SPEC 9/12].
    let src = "{ layout: tree; }\n\
        |topic#root| \"R\" [\n\
          |topic#a| \"A\" [ |topic#note| \"n1\" ]\n\
          |topic#b| \"B\" [ |topic#note| \"n2\" ]\n\
        ]\n";
    assert!(
        lini::check(src).is_ok(),
        "scoped duplicate ids should compile"
    );
}

#[test]
fn an_authored_id_may_not_begin_lini() {
    // The `lini-` prefix is reserved for generated names, mirroring the
    // `.lini-*` class reservation [SPEC 20/22].
    insta::assert_snapshot!(
        lini::check("|box#lini-foo|\n").expect_err("reserved id prefix").message,
        @"an id may not begin 'lini-' — the prefix is reserved for generated names"
    );
}

// ── `format:` ownership [SPEC 16, CHART-DRAW Stage 1] ──

#[test]
fn format_off_a_chart_owner_errors() {
    insta::assert_snapshot!(
        diags("|box#a| { format: decimal 2; }\n"),
        @r"
    test.lini:1:11: error: 'format' has no meaning on '|box|' — it reads on '|chart|' / '|pie|' / '|axis|' / a chart series
    "
    );
}

#[test]
fn format_on_an_axis_is_silent() {
    assert_silent("|chart| [\n|axis| { format: decimal 2 }\n|bars| { data: 1, 2 }\n]\n");
}

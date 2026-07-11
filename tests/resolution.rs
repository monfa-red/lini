use std::ffi::OsStr;
use std::path::PathBuf;

/// Every sample must lex, parse, and resolve cleanly.
#[test]
fn all_samples_resolve() {
    let samples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples");
    let mut failures = Vec::new();

    for entry in std::fs::read_dir(&samples_dir).expect("read samples dir") {
        let path = entry.expect("readdir entry").path();
        if path.extension() != Some(OsStr::new("lini")) {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("read sample");
        // Icons need the `icons` feature; skip icon-using samples when it's off.
        if !cfg!(feature = "icons") && src.contains("|icon|") {
            continue;
        }
        if let Err(e) = lini::check(&src) {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            failures.push(format!("{}: {}", name, e));
        }
    }

    assert!(
        failures.is_empty(),
        "the following samples failed to resolve:\n  {}",
        failures.join("\n  ")
    );
}

// ─────────────────────────── Invalid-input cases ───────────────────────────

#[track_caller]
fn assert_resolve_error(src: &str, expect_msg_substr: &str) {
    let err = lini::check(src).expect_err("expected resolve error");
    let msg = err.to_string();
    assert!(
        msg.contains(expect_msg_substr),
        "expected error containing {:?}, got {:?}",
        expect_msg_substr,
        msg
    );
}

#[test]
fn err_duplicate_scene_id() {
    assert_resolve_error("|box#cat| \"1\"\n|box#cat| \"2\"\n", "duplicate id 'cat'");
}

#[test]
fn err_duplicate_id_reports_previous_location() {
    let src = "|box#cat| \"1\"\n|box#cat| \"2\"\n";
    let err = lini::check(src).expect_err("expected resolve error");
    let shown = err.display_with_source(src, "<test>").to_string();
    assert!(
        shown.contains("(previously at 1:1)"),
        "expected the prior location, got {:?}",
        shown
    );
}

#[test]
fn err_duplicate_id_nested_in_container() {
    // SPEC §15: a duplicate is an error in any scope — the path index requires
    // unique paths.
    assert_resolve_error("|group#g| [ |box#a|\n|box#a| ]\n", "duplicate id 'a'");
}

#[test]
fn same_local_id_across_instances_is_ok() {
    // Two instances of a define share the local id `inlet`, but their full paths
    // differ (a.inlet vs b.inlet), so this must not collide.
    lini::check("{\n  |room::group| [ |box#inlet| ]\n}\n|room#a|\n|room#b|\n")
        .expect("distinct instance paths must not collide");
}

#[test]
fn err_slant_skew_out_of_range() {
    // SPEC §7/§15: skew must be in (-89, 89). 90° gives tan→huge, an absurd
    // shift, so it must be rejected, not silently rendered off-canvas.
    assert_resolve_error(
        "|slant#a| \"x\" { skew: 90 }\n",
        "skew: 90 must be in (-89, 89)",
    );
}

#[test]
fn err_unknown_shape_type() {
    assert_resolve_error("|nosuch#cat| \"x\"\n", "unknown type 'nosuch'");
}

#[test]
fn err_unknown_class() {
    assert_resolve_error("|box#cat| \"x\" .nope\n", "unknown class '.nope'");
}

#[test]
fn err_define_cycle() {
    assert_resolve_error("{\n  |looper::looper| { }\n}\n|box#cat|\n", "cycle in");
}

#[test]
fn err_define_name_collides_with_primitive() {
    assert_resolve_error(
        "{\n  |rect::oval| { }\n}\n|box#cat|\n",
        "'rect' shadows a built-in type",
    );
}

#[test]
fn err_define_name_collides_with_template() {
    assert_resolve_error(
        "{\n  |badge::box| { }\n}\n|box#cat|\n",
        "'badge' shadows a built-in type",
    );
}

#[test]
fn side_name_is_a_free_scene_id() {
    // SPEC §18: `top`/`bottom`/`left`/`right` are keywords only after an endpoint's
    // ':' — so a node may be named `|box#left|`, and that id is reachable as an
    // ordinary endpoint (no longer a reserved-id error).
    lini::check("|box#left| \"x\"\n|box#b|\nleft -> b\n").expect("a side name is a free node id");
}

#[test]
fn link_endpoint_dotpath_navigates_into_groups() {
    lini::check("|group#garden| [ |box#frog| ]\n|box#outside|\ngarden.frog -> outside\n")
        .expect("dot-path resolves");
}

#[test]
fn element_rule_applies_to_every_instance() {
    // `|box| { radius: 5; }` gives every box a default radius of 5.
    lini::check("{\n  |box| { radius: 5; }\n}\n|box#cat| \"Cat\"\n").expect("box defaults");
}

#[test]
fn selector_unknown_type_errors() {
    // A descendant selector's parts must each name a known type; `frog` is
    // unknown, so the rule is rejected at resolve.
    let err =
        lini::check("{\n  |table| |frog| { fill: green; }\n}\n|box#cat|\n").expect_err("unknown");
    assert!(err.to_string().contains("unknown type"), "got: {}", err);
}

#[test]
fn duplicate_define_errors() {
    let err = lini::check(
        "{\n  |treat::box| { radius: 5; }\n  |treat::box| { radius: 9; }\n}\n|treat#cat|\n",
    )
    .expect_err("duplicate");
    assert!(err.to_string().contains("duplicate type"), "got: {}", err);
}

#[test]
fn link_endpoint_path_errors_with_suggestions() {
    // A multi-segment path never auto-creates (SPEC §9); when it misses, the error
    // suggests same-named full paths. (A bare name would auto-create instead.)
    let err = lini::check(
        "|group#kitchen| [ |box#mouse| ]\n|group#garden| [ |box#mouse| ]\nden.mouse -> kitchen\n",
    )
    .expect_err("not found");
    let msg = err.to_string();
    assert!(
        msg.contains("not found at scene root")
            && msg.contains("'garden.mouse'")
            && msg.contains("'kitchen.mouse'"),
        "got: {}",
        msg
    );
}

#[test]
fn body_link_suggestion_is_scope_relative() {
    // A body link resolves from its container; the suggestion must be the path
    // the user can actually type there (shelf.bowl), not the un-typeable
    // root-absolute one (garden.shelf.bowl).
    let err = lini::check(
        "|group#garden| [ |group#shelf| [ |box#bowl| ]\n|box#pot|\nden.bowl -> pot ]\n",
    )
    .expect_err("not found");
    let msg = err.to_string();
    assert!(
        msg.contains("'shelf.bowl'"),
        "scope-relative suggestion: {}",
        msg
    );
    assert!(
        !msg.contains("garden.shelf.bowl"),
        "must not suggest the un-typeable root path: {}",
        msg
    );
}

#[test]
fn body_link_suggestion_stays_in_scope() {
    // Sealed body: a sibling container's node is unreachable, so it must not be
    // suggested at all. A multi-segment path never auto-creates (SPEC §3), so it
    // surfaces the not-found error a bare id would now silently create.
    let err = lini::check(
        "|group#kitchen| [ |box#mouse| ]\n|group#garden| [ |box#cat|\ncat.mouse -> cat ]\n",
    )
    .expect_err("not found");
    let msg = err.to_string();
    assert!(msg.contains("not found"), "{}", msg);
    assert!(
        !msg.contains("kitchen.mouse"),
        "sealed body must not suggest an unreachable sibling: {}",
        msg
    );
}

#[test]
fn a_link_is_styled_with_the_ordinary_vocabulary() {
    // A link is styled like a node (SPEC §9): `stroke` is its wire, `stroke-width` /
    // `stroke-style` its weight and dash, `font-*` / `color` its labels — no `link-*`
    // family. Valid on the link's own block, via a worn class, or globally via `|-|`.
    lini::check("|box#a|\n|box#b|\na -> b { stroke: red; stroke-width: 3; font-size: 14 }\n")
        .expect("stroke/font on a link's own block");
    // A stroke class paints a link's wire and a box's outline alike — one class, one
    // vocabulary.
    lini::check("{ .x { stroke: red } }\n|box#a| .x\n|box#b|\na -> b .x\n")
        .expect("a stroke class worn by a link and a box");
    // `|-|` styles every link at once.
    lini::check("{ |-| { stroke: red; color: blue; font-weight: bold } }\na -> b\n")
        .expect("|-| styles every link");
    // `|-|` is selector-only — a link is drawn by an operator, never instantiated.
    assert!(
        lini::check("|-| \"x\"\n")
            .expect_err("'|-|' as an instance")
            .to_string()
            .contains("only styles links")
    );
}

// SPEC §2: a string-valued property (`hint` / `href` / `src` / `path`) takes a
// quoted string — a bare word there is an identifier, so it is an error.
#[test]
fn unquoted_text_value_is_rejected() {
    for src in [
        "|box#x| { hint: hello }\n",
        "|image#x| { src: photo; width: 40; height: 40 }\n",
        "|path#p| { path: data }\n",
    ] {
        assert_resolve_error(src, "takes a quoted string");
    }
}

#[test]
fn unquoted_text_value_in_a_rule_is_rejected() {
    // The rule cascade resolves the same way — `.link { href: page }` is caught too.
    assert_resolve_error(
        "{ .link { href: page } }\n|box#x| .link\n",
        "takes a quoted string",
    );
}

#[test]
fn quoted_text_value_resolves() {
    lini::check("|box#x| { hint: \"hello\" }\n").expect("a quoted hint resolves");
}

#[test]
fn a_name_value_stays_a_bare_ident() {
    // A *name* (`font-family`, a colour, a `symbol`) is not text: bare is fine,
    // quoted only when it has spaces (SPEC §2). The rule must not over-restrict it.
    lini::check("|box#x| { font-family: monospace; fill: red }\n")
        .expect("a bare name value resolves");
}

// ─────────────────────────── Drawing gates [SPEC 15, 20] ───────────────────────────
//
// Stage 1 (DRAWING-0.16.md): the whole drawing vocabulary parses and resolves; the ops,
// `tol:`, and the wider anchor set are gated to a `layout: drawing` scope; a
// drawing never auto-creates. The engine itself lands in stage 3, so a valid
// drawing file passes `check` (resolve) and errors only at full compile.

#[test]
fn drawing_ops_need_a_drawing_scope() {
    assert_resolve_error("pin (-)\n", "'(-)' draws a dimension");
    assert_resolve_error("|box#a|\n|box#b|\na (<) b\n", "'(<)' draws a dimension");
    assert_resolve_error(
        "|box#a|\n|box#b|\na || b\n",
        "'||' belongs in a 'layout: drawing'",
    );
    // Inside a layout-owning child of a drawing the flow already decided every
    // position — the gate names the container [SPEC 20].
    assert_resolve_error(
        "{ layout: drawing }\n|rect#part| { width: 20; height: 20 }\n|row#r| [\n  |box#a|\n  |box#b|\n  a || b\n]\n",
        "a '|row|' places its own children — mates seat a drawing's",
    );
    assert_resolve_error(
        "a <-> b { tol: 0.1 }\n",
        "'tol' composes a dimension's text",
    );
    assert_resolve_error("a -> b:top-left\n", "':top-left' is a drawing anchor");
    assert_resolve_error("a -> b:middle\n", "':middle' is not a side");
    assert_resolve_error("a -> b:right-top\n", "did you mean ':top-right'?");
    // One-ended wires stay two-ended outside a drawing.
    assert_resolve_error("bolt <- \"THRU\"\n", "at least two endpoints");
}

#[test]
fn a_drawing_scope_resolves_its_statements() {
    lini::check(concat!(
        "{ layout: drawing; scale: 2; unit: mm }\n",
        "|rect#plate| { width: 120; height: 70 } [\n",
        "  |hole#pin| { width: 10; translate: -35 20; pattern: grid(2, 1, 70, 0) }\n",
        "]\n",
        "|rect#bore| { width: 60; height: 16 }\n",
        "plate:left (-) plate:right { side: bottom }\n",
        "plate:left (-) plate.pin { side: top }\n",
        "plate.pin (o) { tol: H7 }\n",
        "bore:top (o) { side: right }\n",
        "plate:top-left <- \"C1.5\"\n",
        "bore:left || plate:right { gap: 4 }\n",
    ))
    .expect("a valid drawing resolves (the engine gates at layout, not here)");
}

#[test]
fn drawing_statement_shapes_are_gated() {
    let in_drawing = |stmts: &str| {
        format!(
            "{{ layout: drawing; }}\n|rect#a| {{ width: 10 }}\n|rect#b| {{ width: 10 }}\n{stmts}"
        )
    };
    assert_resolve_error(&in_drawing("a || b \"x\"\n"), "a mate takes no label");
    assert_resolve_error(&in_drawing("a ||\n"), "a mate seats two parts");
    assert_resolve_error(
        &in_drawing("a (-)\n"),
        "a linear dimension measures two anchors",
    );
    assert_resolve_error(
        &in_drawing("a:top (o) b:bottom\n"),
        "'(o)' measures one round feature",
    );
    assert_resolve_error(&in_drawing("a ->\n"), "a leader points back at its feature");
    // A bare `<-` may compose its text from a threaded segment, so its
    // empty-text gate lives at layout [SPEC 15.7]; the dot leader keeps the
    // resolve-time gate.
    assert_resolve_error(&in_drawing("a *-\n"), "a leader needs its text");
    // No auto-create in a drawing scope [SPEC 15]: unknown endpoints stay unknown.
    assert_resolve_error(
        &in_drawing("a (-) ghost\n"),
        "dimension endpoint 'ghost' not found",
    );
}

#[test]
fn note_is_a_core_template() {
    // Legal anywhere now [SPEC 8]; a sequence still requires its placement.
    lini::check("|note#n| \"anywhere\"\n").expect("a flow-scope note resolves");
    let seq = "{ layout: sequence }\n|box#a| \"A\"\n|note| \"hi\"\n";
    let err = lini::compile_str(seq).expect_err("sequence note needs placement");
    assert!(
        err.to_string().contains("a sequence '|note|' needs"),
        "got: {err}"
    );
}

#[test]
fn scalar_bindings_read_bare_in_groups() {
    lini::check("{ w = 42; }\n|box#x| { width: (w * 2); padding: (w) }\n")
        .expect("a scalar binding reads bare inside a group");
    // Recursion — a binding referencing itself — is the existing static cycle check's
    // job: one mechanism, caught at build, before any evaluation.
    assert_resolve_error("{ a = (a); }\n|box#x| { width: (a) }\n", "cycle");
    assert_resolve_error("{ a = (b); b = (a); }\n|box#x| { width: (a) }\n", "cycle");
}

// ── The comma law [SPEC 2/20]: legacy space-separated lists error with the
//    migration spelling at resolve (scalar-kind lists — strings, keywords,
//    tracks); number lists are judged by their readers (see chart tests). ──

#[test]
fn err_legacy_space_categories() {
    let err = lini::check("|chart#c| { categories: \"a\" \"b\" } [\n  |bars| { data: 1, 2 }\n]\n")
        .expect_err("expected resolve error");
    assert!(
        err.to_string()
            .contains("'categories' takes comma-separated values — 'categories: \"a\", \"b\"'"),
        "{err}"
    );
}

#[test]
fn err_legacy_space_columns() {
    let err = lini::check("{ layout: grid; columns: 80 140 auto; }\n|box#a|\n")
        .expect_err("expected resolve error");
    assert!(
        err.to_string()
            .contains("'columns' takes comma-separated values — 'columns: 80, 140, auto'"),
        "{err}"
    );
}

#[test]
fn err_legacy_space_align() {
    let err = lini::check("|table#t| { columns: 80, 80; align: start end; } [\n  \"a\" \"b\"\n]\n")
        .expect_err("expected resolve error");
    assert!(
        err.to_string()
            .contains("'align' takes comma-separated values — 'align: start, center, end'"),
        "{err}"
    );
}

#[test]
fn err_legacy_space_along() {
    let err = lini::check("|box#a|\n|box#b|\na -> b \"x\" { along: 0.2 0.8; }\n")
        .expect_err("expected resolve error");
    assert!(
        err.to_string()
            .contains("'along' takes comma-separated fractions — 'along: 0.2, 0.5, 0.8'"),
        "{err}"
    );
}

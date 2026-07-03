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
fn stroke_props_on_a_link_are_rejected() {
    // A link is painted by the `link-*` family, never `stroke*` (SPEC §9) — it is a
    // link, not a stroked shape. The error names the `link-*` replacement.
    let cases = [
        ("a -> b { stroke: red }\n", "link-color"),
        ("a -> b { stroke-width: 3 }\n", "link-width"),
        ("a -> b { stroke-style: dashed }\n", "link-style"),
    ];
    for (link, equiv) in cases {
        let src = format!("|box#a|\n|box#b|\n{link}");
        let msg = lini::check(&src).expect_err("stroke on a link").to_string();
        assert!(msg.contains("paints a shape's outline"), "{src} → {msg}");
        assert!(
            msg.contains(equiv),
            "should suggest '{equiv}': {src} → {msg}"
        );
    }
    // A stroke property reaching a link through a worn class is rejected too.
    let msg = lini::check("{ .x { stroke: red } }\n|box#a|\n|box#b|\na -> b .x\n")
        .expect_err("stroke via class on a link")
        .to_string();
    assert!(msg.contains("paints a shape's outline"), "{msg}");
    // The link family is valid on a link; a stroke class on a box still is too.
    lini::check(
        "{ .x { stroke: red } }\n|box#a| .x\n|box#b|\na -> b { link-color: red; link-width: 3 }\n",
    )
    .expect("link family on a link, stroke class on a box");
}

// SPEC §2: a string-valued property (`title` / `href` / `src` / `path`) takes a
// quoted string — a bare word there is an identifier, so it is an error.
#[test]
fn unquoted_text_value_is_rejected() {
    for src in [
        "|box#x| { title: hello }\n",
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
    lini::check("|box#x| { title: \"hello\" }\n").expect("a quoted title resolves");
}

#[test]
fn a_name_value_stays_a_bare_ident() {
    // A *name* (`font-family`, a colour, a `symbol`) is not text: bare is fine,
    // quoted only when it has spaces (SPEC §2). The rule must not over-restrict it.
    lini::check("|box#x| { font-family: monospace; fill: red }\n")
        .expect("a bare name value resolves");
}

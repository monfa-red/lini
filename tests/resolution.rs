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
    assert_resolve_error("cat |box| \"1\"\ncat |box| \"2\"\n", "duplicate id 'cat'");
}

#[test]
fn err_duplicate_id_reports_previous_location() {
    let src = "cat |box| \"1\"\ncat |box| \"2\"\n";
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
    assert_resolve_error("g |group| [ a |box|\na |box| ]\n", "duplicate id 'a'");
}

#[test]
fn same_local_id_across_instances_is_ok() {
    // Two instances of a define share the local id `inlet`, but their full paths
    // differ (a.inlet vs b.inlet), so this must not collide.
    lini::check("{\n  |room::group| [ inlet |box| ]\n}\na |room|\nb |room|\n")
        .expect("distinct instance paths must not collide");
}

#[test]
fn err_slant_skew_out_of_range() {
    // SPEC §7/§15: skew must be in (-89, 89). 90° gives tan→huge, an absurd
    // shift, so it must be rejected, not silently rendered off-canvas.
    assert_resolve_error(
        "a |slant| { skew: 90 } \"x\"\n",
        "skew: 90 must be in (-89, 89)",
    );
}

#[test]
fn err_unknown_shape_type() {
    assert_resolve_error("cat |nosuch| \"x\"\n", "unknown type 'nosuch'");
}

#[test]
fn err_unknown_class() {
    assert_resolve_error("cat |box| .nope \"x\"\n", "unknown class '.nope'");
}

#[test]
fn err_define_cycle() {
    assert_resolve_error("{\n  |looper::looper| { }\n}\ncat |box|\n", "cycle in");
}

#[test]
fn err_define_name_collides_with_primitive() {
    assert_resolve_error(
        "{\n  |rect::oval| { }\n}\ncat |box|\n",
        "'rect' shadows a built-in type",
    );
}

#[test]
fn err_define_name_collides_with_template() {
    assert_resolve_error(
        "{\n  |note::box| { }\n}\ncat |box|\n",
        "'note' shadows a built-in type",
    );
}

#[test]
fn err_reserved_scene_id() {
    // Type names are free as ids now; only endpoint sides and `link` stay
    // reserved — `left` is a side, so it cannot be a node id.
    assert_resolve_error("left |box| \"x\"\n", "'left' is reserved");
}

#[test]
fn link_endpoint_dotpath_navigates_into_groups() {
    lini::check("garden |group| [ frog |box| ]\noutside |box|\ngarden.frog -> outside\n")
        .expect("dot-path resolves");
}

#[test]
fn element_rule_applies_to_every_instance() {
    // `|box| { radius: 5; }` gives every box a default radius of 5.
    lini::check("{\n  |box| { radius: 5; }\n}\ncat |box| \"Cat\"\n").expect("box defaults");
}

#[test]
fn selector_unknown_type_errors() {
    // A descendant selector's parts must each name a known type; `frog` is
    // unknown, so the rule is rejected at resolve.
    let err =
        lini::check("{\n  |table frog| { fill: green; }\n}\ncat |box|\n").expect_err("unknown");
    assert!(err.to_string().contains("unknown type"), "got: {}", err);
}

#[test]
fn duplicate_define_errors() {
    let err = lini::check(
        "{\n  |treat::box| { radius: 5; }\n  |treat::box| { radius: 9; }\n}\ncat |treat|\n",
    )
    .expect_err("duplicate");
    assert!(err.to_string().contains("duplicate type"), "got: {}", err);
}

#[test]
fn link_endpoint_bare_nested_name_errors_with_suggestions() {
    let err = lini::check(
        "kitchen |group| [ mouse |box| ]\ngarden |group| [ mouse |box| ]\nmouse -> kitchen\n",
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
    let err =
        lini::check("garden |group| [ shelf |group| [ bowl |box| ]\npot |box|\nbowl -> pot ]\n")
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
    // suggested at all.
    let err = lini::check(
        "kitchen |group| [ mouse |box| ]\ngarden |group| [ cat |box|\nmouse -> cat ]\n",
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
    // A link is painted by the `link` family, never `stroke*` (SPEC §9) — it is a
    // link, not a stroked shape. The error names the `link*` replacement.
    let cases = [
        ("a -> b { stroke: red }\n", "link"),
        ("a -> b { stroke-width: 3 }\n", "link-width"),
        ("a -> b { stroke-style: dashed }\n", "link-style"),
    ];
    for (link, equiv) in cases {
        let src = format!("a |box|\nb |box|\n{link}");
        let msg = lini::check(&src).expect_err("stroke on a link").to_string();
        assert!(msg.contains("paints a shape's outline"), "{src} → {msg}");
        assert!(
            msg.contains(equiv),
            "should suggest '{equiv}': {src} → {msg}"
        );
    }
    // A stroke property reaching a link through a worn class is rejected too.
    let msg = lini::check("{ .x { stroke: red } }\na |box|\nb |box|\na -> b .x\n")
        .expect_err("stroke via class on a link")
        .to_string();
    assert!(msg.contains("paints a shape's outline"), "{msg}");
    // The link family is valid on a link; a stroke class on a box still is too.
    lini::check(
        "{ .x { stroke: red } }\na |box| .x\nb |box|\na -> b { link: red; link-width: 3 }\n",
    )
    .expect("link family on a link, stroke class on a box");
}

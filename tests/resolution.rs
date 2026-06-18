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
    assert_resolve_error("g |group| { a |box|\na |box| }\n", "duplicate id 'a'");
}

#[test]
fn same_local_id_across_instances_is_ok() {
    // Two instances of a define share the local id `inlet`, but their full paths
    // differ (a.inlet vs b.inlet), so this must not collide.
    lini::check("room::group { inlet |box| }\na |room|\nb |room|\n")
        .expect("distinct instance paths must not collide");
}

#[test]
fn err_slant_skew_out_of_range() {
    // SPEC §7/§15: skew must be in (-89, 89). 90° gives tan→huge, an absurd
    // shift, so it must be rejected, not silently rendered off-canvas.
    assert_resolve_error(
        "a |slant| \"x\" { skew: 90; }\n",
        "skew: 90 must be in (-89, 89)",
    );
}

#[test]
fn err_unknown_shape_type() {
    assert_resolve_error("cat |nosuch| \"x\"\n", "unknown type 'nosuch'");
}

#[test]
fn err_unknown_class() {
    assert_resolve_error("cat |box| \"x\" .nope\n", "unknown class '.nope'");
}

#[test]
fn err_define_cycle() {
    assert_resolve_error("looper::looper { }\ncat |box|\n", "cycle in");
}

#[test]
fn err_define_name_collides_with_primitive() {
    assert_resolve_error("rect::oval { }\ncat |box|\n", "'rect' shadows a built-in type");
}

#[test]
fn err_define_name_collides_with_template() {
    assert_resolve_error("note::box { }\ncat |box|\n", "'note' shadows a built-in type");
}

#[test]
fn err_reserved_scene_id() {
    assert_resolve_error("rect |box| \"x\"\n", "'rect' is reserved");
}

#[test]
fn wire_endpoint_dotpath_navigates_into_groups() {
    lini::check("garden |group| { frog |box| }\noutside |box|\ngarden.frog -> outside\n")
        .expect("dot-path resolves");
}

#[test]
fn element_rule_applies_to_every_instance() {
    // `box { radius: 5; }` gives every rect a default radius of 5.
    lini::check("box { radius: 5; }\ncat |box| \"Cat\"\n").expect("rect defaults");
}

#[test]
fn selector_unknown_type_errors() {
    // A lone `frog { }` with an unknown name is a *node*, not a rule (SPEC §16);
    // an unknown type surfaces in a descendant selector, whose parts must name
    // known types.
    let err = lini::check("table frog { fill: green; }\ncat |box|\n").expect_err("unknown");
    assert!(err.to_string().contains("unknown type"), "got: {}", err);
}

#[test]
fn duplicate_define_errors() {
    let err = lini::check("treat::box { radius: 5; }\ntreat::box { radius: 9; }\ncat |treat|\n")
        .expect_err("duplicate");
    assert!(err.to_string().contains("duplicate type"), "got: {}", err);
}

#[test]
fn wire_endpoint_bare_nested_name_errors_with_suggestions() {
    let err = lini::check(
        "kitchen |group| { mouse |box| }\ngarden |group| { mouse |box| }\nmouse -> kitchen\n",
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
fn body_wire_suggestion_is_scope_relative() {
    // A body wire resolves from its container; the suggestion must be the path
    // the user can actually type there (shelf.bowl), not the un-typeable
    // root-absolute one (garden.shelf.bowl).
    let err =
        lini::check("garden |group| { shelf |group| { bowl |box| }\npot |box|\nbowl -> pot }\n")
            .expect_err("not found");
    let msg = err.to_string();
    assert!(msg.contains("'shelf.bowl'"), "scope-relative suggestion: {}", msg);
    assert!(
        !msg.contains("garden.shelf.bowl"),
        "must not suggest the un-typeable root path: {}",
        msg
    );
}

#[test]
fn body_wire_suggestion_stays_in_scope() {
    // Sealed body: a sibling container's node is unreachable, so it must not be
    // suggested at all.
    let err = lini::check(
        "kitchen |group| { mouse |box| }\ngarden |group| { cat |box|\nmouse -> cat }\n",
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

//! Formatter conformance + invariants.

use std::ffi::OsStr;

#[test]
fn fmt_every_sample_is_idempotent() {
    // Running fmt twice on the same input must produce the same output. This is
    // the core invariant for any formatter — without it, editor-on-save loops
    // would diff every time.
    let samples_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("samples");
    let mut failures = Vec::new();
    for entry in std::fs::read_dir(&samples_dir).expect("read samples dir") {
        let path = entry.expect("readdir").path();
        if path.extension() != Some(OsStr::new("lini")) {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("read sample");
        let pass1 = lini::format_source(&src).expect("fmt pass 1");
        let pass2 = lini::format_source(&pass1).expect("fmt pass 2");
        if pass1 != pass2 {
            failures.push(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    assert!(failures.is_empty(), "not idempotent: {:?}", failures);
}

#[test]
fn formatted_output_resolves_identically() {
    // Formatting must not change semantics. Compile the original sample,
    // compile the formatted version, and require identical SVG output.
    let opts = lini::Options {
        bake_vars: true,
        ..Default::default()
    };
    let samples_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("samples");
    let mut failures = Vec::new();
    for entry in std::fs::read_dir(&samples_dir).expect("read samples dir") {
        let path = entry.expect("readdir").path();
        if path.extension() != Some(OsStr::new("lini")) {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        // Skip the user's untracked scratch file if it exists.
        if name == "test.lini" {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("read sample");
        let formatted = lini::format_source(&src).expect("format");

        let svg_orig = lini::compile_str_with(&src, &opts).expect("compile original");
        let svg_fmt = lini::compile_str_with(&formatted, &opts).expect("compile formatted");
        if svg_orig != svg_fmt {
            failures.push(name);
        }
    }
    assert!(failures.is_empty(), "semantic divergence: {:?}", failures);
}

#[test]
fn fmt_preserves_section_comments_and_blank_lines() {
    let src = "\
--gap: 24;

// Top-level comment.
// Comment on root statement.
cat |box|

dog |box|
";
    let formatted = lini::format_source(src).expect("fmt");
    assert!(
        formatted.contains("// Top-level comment."),
        "missing top-level comment in:\n{}",
        formatted
    );
    // Blank line between cat and dog should be preserved.
    assert!(
        formatted.contains("|box|\n\ndog"),
        "blank line not preserved between siblings:\n{}",
        formatted
    );
}

#[test]
fn fmt_canonicalizes_numeric_forms() {
    // `+3` and `.5` are legal but non-canonical; the formatter normalizes.
    let src = "--a: +3;\n--b: .5;\n";
    let formatted = lini::format_source(src).expect("fmt");
    assert!(
        formatted.contains("--a: 3;"),
        "expected +3 → 3, got:\n{}",
        formatted
    );
    assert!(
        formatted.contains("--b: 0.5;"),
        "expected .5 → 0.5, got:\n{}",
        formatted
    );
}

#[test]
fn fmt_normalizes_value_group_spacing() {
    // v4 values are space-separated within a group, comma between groups.
    let src = "dim |line| {points:0 0,10 10}\n";
    let formatted = lini::format_source(src).expect("fmt");
    assert!(
        formatted.contains("points: 0 0, 10 10;"),
        "expected canonical value-group spacing, got:\n{}",
        formatted
    );
}

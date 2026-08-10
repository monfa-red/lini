//! Formatter conformance + invariants.

use lini::testing::{read_sample, sample_opts, samples};

#[test]
fn fmt_every_sample_is_idempotent() {
    // Running fmt twice on the same input must produce the same output. This is
    // the core invariant for any formatter — without it, editor-on-save loops
    // would diff every time.
    let mut failures = Vec::new();
    for path in samples() {
        let pass1 = lini::format_source(&read_sample(&path)).expect("fmt pass 1");
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
        static_mode: true,
        ..sample_opts()
    };
    let mut failures = Vec::new();
    for path in samples() {
        let src = read_sample(&path);
        let formatted = lini::format_source(&src).expect("format");

        let svg_orig = lini::compile_str_with(&src, &opts).expect("compile original");
        let svg_fmt = lini::compile_str_with(&formatted, &opts).expect("compile formatted");
        if svg_orig != svg_fmt {
            failures.push(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    assert!(failures.is_empty(), "semantic divergence: {:?}", failures);
}

#[test]
fn fmt_preserves_section_comments_and_blank_lines() {
    let src = "\
{ --gap: 24; }

// Top-level comment.
// Comment on root statement.
|box#cat|

|box#dog|
";
    let formatted = lini::format_source(src).expect("fmt");
    assert!(
        formatted.contains("// Top-level comment."),
        "missing top-level comment in:\n{}",
        formatted
    );
    // Blank line between cat and dog should be preserved.
    assert!(
        formatted.contains("|box#cat|\n\n|box#dog|"),
        "blank line not preserved between siblings:\n{}",
        formatted
    );
}

#[test]
fn fmt_canonicalizes_numeric_forms() {
    // `+3` and `.5` are legal but non-canonical; the formatter normalizes.
    let src = "{ --a: +3; --b: .5; }\n";
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
fn fmt_round_trips_a_classed_text_leaf() {
    // A string in content position wears its class chain canonically, spaced off
    // the string then glued, ahead of its `{ }` block [SPEC 3].
    let src = "{ .card-title { font-size: 17; } }\n\"Starter\" .card-title { color: red }\n";
    let formatted = lini::format_source(src).expect("fmt");
    assert!(
        formatted.contains("\"Starter\" .card-title { color: red; }"),
        "expected canonical classed text, got:\n{}",
        formatted
    );
    // Idempotent.
    assert_eq!(
        formatted,
        lini::format_source(&formatted).expect("fmt pass 2")
    );
}

// ─────────────────────────── Capsule endpoints [SPEC 9] ───────────────────────────

#[test]
fn fmt_round_trips_capsule_endpoints() {
    // The canonical spellings survive fmt byte-for-byte.
    for src in [
        "cat -> |cyl#db|\n",
        "|cyl#db| -> cat\n",
        "a -> |box| -> c\n",
        "a & b -> |gnd|\n",
        "a - |gnd| - b\n",
        "x - |component#U9|.p4\n",
        "|cyl#db|:left -> x\n",
        "a -> |#cat|\n",
        "a -> |cyl#db| \"watches\" { stroke: red; }\n",
    ] {
        let once = lini::format_source(src).expect("fmt");
        assert_eq!(once, src, "canonical form changed");
        let twice = lini::format_source(&once).expect("fmt twice");
        assert_eq!(twice, once, "not idempotent");
    }
}

#[test]
fn fmt_round_trips_one_ended_label_wires() {
    // The schematic scope's one-ended statement [SPEC 16.5/22]: the op trails
    // its single endpoint, the net text trails the op, and every marker
    // spelling survives byte-for-byte.
    for src in [
        "u7.diag - \"NSTDBY\"\n",
        "u7.diag -> \"NSTDBY\"\n",
        "u7.diag -< \"NSTDBY\"\n",
        "u7.diag -<> \"NSTDBY\"\n",
        "u7.diag -* \"NSTDBY\"\n",
        "u7.diag -- \"NSTDBY\"\n",
        "u7.diag - \"NSTDBY\" { stroke: red; }\n",
    ] {
        let once = lini::format_source(src).expect("fmt");
        assert_eq!(once, src, "canonical form changed");
        assert_eq!(lini::format_source(&once).expect("fmt twice"), once);
    }
}

#[test]
fn fmt_spaces_a_glued_capsule_op() {
    // `a -|gnd|- b` canonicalizes to spaced ops, like every link op.
    let out = lini::format_source("a -|gnd|- b\n").expect("fmt");
    assert_eq!(out, "a - |gnd| - b\n");
}

//! `lini desugar` — the surface sugar expanded to its explicit form (SPEC §14):
//! a box's id-as-label into a `{ "id" }` text child, a wire's auto-distributed
//! labels into an explicit `along:`. Types/vars/properties are left as written.

use lini::desugar_source;

#[test]
fn id_as_label_becomes_an_explicit_text() {
    let out = desugar_source("cat |box|\n").unwrap();
    assert!(out.contains("cat |box| { \"cat\" }"), "{out}");
}

#[test]
fn an_explicit_label_is_left_alone() {
    let out = desugar_source("cat |box| { \"Cat\" }\n").unwrap();
    assert!(out.contains("\"Cat\""), "{out}");
    // No second, id-derived label is added.
    assert!(!out.contains("\"cat\""), "{out}");
}

#[test]
fn a_container_keeps_its_children_and_takes_no_label() {
    let out = desugar_source("g |group| {\n  a |box|\n}\n").unwrap();
    assert!(!out.contains("\"g\""), "a group's id is not a label: {out}");
    assert!(
        out.contains("a |box| { \"a\" }"),
        "the leaf child gains its id label: {out}"
    );
}

#[test]
fn icon_glyph_is_not_expanded() {
    let out = desugar_source("home |icon|\n").unwrap();
    assert!(
        !out.contains("{"),
        "an icon's glyph stays on the line: {out}"
    );
}

#[test]
fn wire_labels_gain_an_explicit_along() {
    let out = desugar_source("a |box|\nb |box|\na -> b { \"x\" }\n").unwrap();
    assert!(
        out.contains("along: 0.5;"),
        "one label centres at 0.5: {out}"
    );
    assert!(out.contains("\"x\""), "{out}");
}

#[test]
fn desugar_is_idempotent() {
    let src =
        "g |group| {\n  |caption| { \"T\" }\n  a |box|\n}\nx |box|\ny |box|\nx -> y { \"w\" }\n";
    let once = desugar_source(src).unwrap();
    let twice = desugar_source(&once).unwrap();
    assert_eq!(once, twice, "desugar must be idempotent");
}

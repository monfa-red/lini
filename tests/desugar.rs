//! `lini desugar` — label and wire-label sugar expanded to explicit children,
//! with types/vars/properties left as written.

use lini::desugar_source;

#[test]
fn group_first_label_becomes_a_top_caption() {
    let out = desugar_source("g |group| \"Hi\" {\n  a |rect| \"A\"\n}\n").unwrap();
    // A group's first label is a |caption| (mount: in via the caption template).
    assert!(out.contains("|caption| \"Hi\""), "{out}");
}

#[test]
fn group_second_label_becomes_a_bottom_footer() {
    let out = desugar_source("g |group| \"Top\" \"Bot\" {}\n").unwrap();
    assert!(out.contains("|caption| \"Top\""), "{out}");
    assert!(out.contains("|caption| \"Bot\""), "{out}");
    assert!(out.contains("side: bottom;"), "the footer carries side: bottom: {out}");
}

#[test]
fn plain_shape_label_is_a_centred_text_child() {
    let out = desugar_source("cat |rect| \"Cat\"\n").unwrap();
    assert!(out.contains("|text| \"Cat\""), "{out}");
    assert!(!out.contains("|caption|"), "a plain rect has no caption: {out}");
}

#[test]
fn inline_wire_label_becomes_a_text_child() {
    let out = desugar_source("a |rect|\nb |rect|\na -> b \"x\"\n").unwrap();
    assert!(out.contains("a -> b {"), "{out}");
    assert!(out.contains("|text| \"x\""), "{out}");
}

#[test]
fn user_shape_extending_group_still_promotes_its_caption() {
    // The group-ness comes from the type chain, not a literal `|group|`.
    let out = desugar_source("panel::group { }\np |panel| \"Title\" {}\n").unwrap();
    assert!(out.contains("|caption| \"Title\""), "{out}");
}

#[test]
fn desugar_is_idempotent() {
    let src = "g |group| \"T\" \"F\" {\n  a |rect| \"A\"\n}\nx |rect|\ny |rect|\nx -> y \"w\"\n";
    let once = desugar_source(src).unwrap();
    let twice = desugar_source(&once).unwrap();
    assert_eq!(once, twice, "desugar must be idempotent");
}

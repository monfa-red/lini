//! `lini desugar` lowers ALL sugar to primitives + `.lini-*` classes: typed
//! instances become primitives wearing their `.lini-*` chain, templates/defines
//! collapse into generated class defs, scene/wire defaults fill the global block,
//! and labels / `along:` become explicit. The lowered form is a fixed point.

use lini::desugar_source;

#[test]
fn a_plain_box_wears_its_lini_class_and_explicit_label() {
    let out = desugar_source("cat |box|\n").unwrap();
    assert!(out.contains("cat |box| .lini-box [ \"cat\" ]"), "{out}");
    assert!(
        out.contains(".lini-box {"),
        "the box bundle is a generated class: {out}"
    );
}

#[test]
fn a_group_lowers_to_box_plus_chain_and_a_generated_class() {
    let out = desugar_source("g |group| [\n  a |box|\n]\n").unwrap();
    // derived → base → primitive (matches the pre-desugar SVG class order).
    assert!(out.contains("|box| .lini-group.lini-box"), "{out}");
    assert!(
        out.contains(".lini-group {") && out.contains("stroke-style: dashed;"),
        "{out}"
    );
}

#[test]
fn element_rule_merges_into_the_generated_class() {
    let out = desugar_source("{ |box| { radius: 4; } }\nx |box|\n").unwrap();
    assert!(
        out.contains("radius: 4;"),
        "element rule lands in .lini-box: {out}"
    );
    assert!(
        !out.contains("radius: 6;"),
        "the bundle's radius is overridden in place, not duplicated: {out}"
    );
}

#[test]
fn descendant_rule_rewrites_types_to_lini_classes() {
    let out =
        desugar_source("{ |group box| { fill: gray; } }\ng |group| [\n  a |box|\n]\n").unwrap();
    assert!(out.contains("|.lini-group .lini-box|"), "{out}");
}

#[test]
fn define_body_inlines_and_the_define_vanishes() {
    let src = "{ |room::group| { gap: 10; } [\n  inlet |box|\n] }\nr |room|\n";
    let out = desugar_source(src).unwrap();
    assert!(out.contains(".lini-room { gap: 10; }"), "{out}");
    assert!(
        out.contains("inlet |box| .lini-box [ \"inlet\" ]"),
        "define body inlined per instance: {out}"
    );
    assert!(!out.contains("::"), "no defines remain: {out}");
}

#[test]
fn scene_and_wire_defaults_land_in_the_global_block() {
    let out = desugar_source("a -> b \"w\"\n").unwrap();
    assert!(out.contains("padding: 20;"), "scene defaults: {out}");
    assert!(out.contains("clearance: 16;"), "wire defaults: {out}");
    assert!(
        out.contains("a |box| .lini-box [ \"a\" ]"),
        "auto-create: {out}"
    );
    assert!(out.contains("along: 0.5;"), "auto-along: {out}");
}

#[test]
fn the_wire_block_appears_only_when_a_wire_exists() {
    // A wireless diagram carries no `-> { }` block — nothing would consume it.
    let wireless = desugar_source("\"hello\"\n").unwrap();
    assert!(
        !wireless.contains("->"),
        "no wire → no wire block: {wireless}"
    );
    assert!(
        !wireless.contains("clearance"),
        "no wire defaults: {wireless}"
    );
    // The moment a wire appears, the defaults return.
    let wired = desugar_source("a -> b\n").unwrap();
    assert!(wired.contains("clearance: 16;"), "wired keeps it: {wired}");
}

#[test]
fn an_icon_keeps_its_glyph_on_the_line() {
    let out = desugar_source("home |icon|\n").unwrap();
    assert!(out.contains("home |icon| .lini-icon"), "{out}");
    assert!(
        !out.contains("[ \"home\" ]"),
        "an icon's glyph stays its id, never a text child: {out}"
    );
}

#[test]
fn desugar_is_idempotent() {
    let src = "g |group| [\n  |caption| \"T\"\n  a |box|\n]\nx -> y \"w\"\n";
    let once = desugar_source(src).unwrap();
    assert_eq!(desugar_source(&once).unwrap(), once, "idempotent");
}

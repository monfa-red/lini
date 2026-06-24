//! `lini desugar` lowers ALL sugar to primitives + `.lini-*` classes: typed
//! instances become primitives wearing their `.lini-*` chain, templates/defines
//! collapse into generated class defs, scene/link defaults fill the global block,
//! and labels / `along:` become explicit. The lowered form is a fixed point.

use lini::desugar_source;

#[test]
fn a_plain_box_wears_its_lini_class_and_explicit_label() {
    let out = desugar_source("cat |box|\n").unwrap();
    assert!(
        out.contains("cat |block| .lini-box.lini-block [ \"cat\" ]"),
        "{out}"
    );
    assert!(
        out.contains(".lini-box {"),
        "the box bundle is a generated class: {out}"
    );
}

#[test]
fn a_group_lowers_to_block_plus_chain_and_a_generated_class() {
    let out = desugar_source("g |group| [\n  a |box|\n]\n").unwrap();
    // derived → base → primitive (matches the pre-desugar SVG class order).
    assert!(out.contains("|block| .lini-group.lini-block"), "{out}");
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
        out.contains("inlet |block| .lini-box.lini-block [ \"inlet\" ]"),
        "define body inlined per instance: {out}"
    );
    assert!(!out.contains("::"), "no defines remain: {out}");
}

#[test]
fn scene_defaults_and_auto_create_land_in_the_global_block() {
    let out = desugar_source("a -> b \"w\"\n").unwrap();
    assert!(out.contains("padding: 20;"), "scene defaults: {out}");
    assert!(
        out.contains("a |block| .lini-box.lini-block [ \"a\" ]"),
        "auto-create: {out}"
    );
    assert!(out.contains("along: 0.5;"), "auto-along: {out}");
}

#[test]
fn desugar_emits_no_link_defaults_block() {
    // Link defaults are a resolve-time cascade now (SPEC §9), not a `-> { }`
    // rule — desugar never emits one, and its output stays re-parseable.
    let linked = desugar_source("a -> b\n").unwrap();
    assert!(!linked.contains("-> {"), "no link-defaults block: {linked}");
    assert!(
        !linked.contains("clearance"),
        "no clearance in desugar: {linked}"
    );
    assert!(
        linked.contains("a -> b"),
        "the link statement remains: {linked}"
    );
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

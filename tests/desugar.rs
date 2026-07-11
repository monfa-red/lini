//! `lini desugar` lowers ALL sugar to primitives + `.lini-*` classes: typed
//! instances become primitives wearing their `.lini-*` chain, templates/defines
//! collapse into generated class defs, scene/link defaults fill the global block,
//! and labels / `along:` become explicit. The lowered form is a fixed point.

use lini::desugar_source;

#[test]
fn a_plain_box_wears_its_lini_class_and_explicit_label() {
    let out = desugar_source("|box#cat| \"cat\"\n").unwrap();
    assert!(
        out.contains("|block#cat| .lini-box.lini-block [ \"cat\" ]"),
        "{out}"
    );
    assert!(
        out.contains(".lini-box {"),
        "the box bundle is a generated class: {out}"
    );
}

#[test]
fn a_group_lowers_to_block_plus_chain_and_a_generated_class() {
    let out = desugar_source("|group#g| [\n  |box#a|\n]\n").unwrap();
    // derived → base → primitive (matches the pre-desugar SVG class order).
    assert!(out.contains("|block#g| .lini-group.lini-block"), "{out}");
    assert!(
        out.contains(".lini-group {") && out.contains("stroke-style: dashed;"),
        "{out}"
    );
}

#[test]
fn element_rule_merges_into_the_generated_class() {
    let out = desugar_source("{ |box| { radius: 4; } }\n|box#x|\n").unwrap();
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
        desugar_source("{ |group| |box| { fill: gray; } }\n|group#g| [\n  |box#a|\n]\n").unwrap();
    assert!(out.contains(".lini-group .lini-box {"), "{out}");
}

#[test]
fn define_body_inlines_and_the_define_vanishes() {
    let src = "{ |room::group| { gap: 10; } [\n  |box#inlet| \"inlet\"\n] }\n|room#r|\n";
    let out = desugar_source(src).unwrap();
    assert!(out.contains(".lini-room { gap: 10; }"), "{out}");
    assert!(
        out.contains("|block#inlet| .lini-box.lini-block [ \"inlet\" ]"),
        "define body inlined per instance: {out}"
    );
    assert!(!out.contains("::"), "no defines remain: {out}");
}

#[test]
fn scene_defaults_and_auto_create_land_in_the_global_block() {
    let out = desugar_source("a -> b \"w\"\n").unwrap();
    assert!(out.contains("padding: 20;"), "scene defaults: {out}");
    assert!(
        out.contains("|block#a| .lini-box.lini-block [ \"a\" ]"),
        "auto-create: {out}"
    );
    assert!(out.contains("along: 0.5;"), "auto-along: {out}");
}

#[test]
fn link_labels_lower_to_an_explicit_bracket() {
    // SPEC §9/§14: a link's head-label sugar lowers to the explicit [ ] form (the
    // dumb core's input), exactly as a node's smart label does. The head shape is
    // pretty-fmt sugar only — the core never sees it.
    let out = desugar_source("a -> b \"flows\"\n").unwrap();
    assert!(out.contains("[ \"flows\" ]"), "link label in [ ]: {out}");
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
fn an_icon_has_no_id_label_child() {
    // An |icon| is named by `symbol`; its id never becomes a text child.
    let out = desugar_source("|icon#home| { symbol: house }\n").unwrap();
    assert!(out.contains("|icon#home| .lini-icon"), "{out}");
    assert!(
        !out.contains("[ \"home\" ]"),
        "an icon's id never becomes a text child: {out}"
    );
}

#[test]
fn desugar_is_idempotent() {
    let src = "|group#g| [\n  |caption| \"T\"\n  |box#a|\n]\nx -> y \"w\"\n";
    let once = desugar_source(src).unwrap();
    assert_eq!(desugar_source(&once).unwrap(), once, "idempotent");
}

#[test]
fn the_scale_fold_stamps_px_per_unit() {
    // ratio × unit-mm × density → the engine's one internal number [SPEC 15.1/18].
    let out = desugar_source("{ layout: drawing }\n|rect#r| { width: 10; height: 5 }\n").unwrap();
    assert!(out.contains("px-per-unit: 4"), "defaults 1 × mm × 4: {out}");

    let out = desugar_source(
        "{ density: 8; }\n|drawing#v| { scale: 2; unit: cm; } [ |rect#r| { width: 4; height: 2 } ]\n",
    )
    .unwrap();
    assert!(
        out.contains("px-per-unit: 160"),
        "2 × 10 mm × 8 px/mm: {out}"
    );
    // The authored ratio stays visible beside the fold — titles read it.
    assert!(out.contains("scale: 2"), "{out}");
}

#[test]
fn the_scale_fold_is_idempotent() {
    let src = "{ density: 8; }\n|drawing#v| { scale: 2; unit: cm; } [ |rect#r| { width: 4; height: 2 } ]\n";
    let once = desugar_source(src).unwrap();
    let twice = desugar_source(&once).unwrap();
    assert_eq!(once, twice, "re-desugar must not fold the fold");
}

#[test]
fn a_page_folds_the_density_alone_and_rejects_its_own_scale() {
    let out = desugar_source("|page#p| { sheet: a5 }\n").unwrap();
    assert!(out.contains("px-per-unit: 4"), "paper mm × density: {out}");

    let err = lini::check("|page#p| { sheet: a5; scale: 2 }\n").expect_err("page scale");
    assert!(
        err.to_string().contains("a '|page|' carries no 'scale:'"),
        "{err}"
    );
}

#[test]
fn unit_is_an_ident_enum_and_density_positive() {
    let err = lini::check("{ layout: drawing; unit: \"mm\" }\n|rect#r| { width: 4; height: 2 }\n")
        .expect_err("quoted unit");
    assert!(
        err.to_string().contains("'unit' is mm, cm, m, or in"),
        "{err}"
    );

    let err = lini::check("{ layout: drawing; density: 0 }\n|rect#r| { width: 4; height: 2 }\n")
        .expect_err("zero density");
    assert!(err.to_string().contains("'density' must be > 0"), "{err}");
}

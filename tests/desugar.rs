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
fn every_sample_is_a_byte_identical_desugar_fixed_point() {
    // The source fixed point, swept over the showroom [SPEC 19]: for every
    // sample, re-desugaring the lowered text reproduces it byte for byte — the
    // guard that every generated node/link is idempotently detected and
    // span-seated so `lini desugar` output is stable.
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/samples");
    let mut swept = 0;
    for entry in std::fs::read_dir(dir).expect("samples dir") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("lini") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let src = std::fs::read_to_string(&path).expect("read sample");
        let once = desugar_source(&src).unwrap_or_else(|e| panic!("{name}: desugar failed: {e}"));
        let twice =
            desugar_source(&once).unwrap_or_else(|e| panic!("{name}: re-desugar failed: {e}"));
        assert_eq!(once, twice, "{name}: desugar is not a fixed point");
        swept += 1;
    }
    assert!(swept > 20, "the sweep found only {swept} samples");
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

#[test]
fn a_wire_chain_expands_to_one_link_per_hop() {
    // [SPEC 9/18]: `a -> b -> c` is exactly `a -> b; b -> c` — every hop
    // carries the operator's full markers, and `lini desugar` shows both.
    let out = desugar_source("|box#a|\n|box#b|\n|box#c|\na -> b -> c\n").unwrap();
    assert!(out.contains("a -> b\n"), "{out}");
    assert!(out.contains("b -> c\n"), "{out}");
    assert!(!out.contains("a -> b -> c"), "{out}");
    // The statement's label rides every hop [SPEC 9].
    let out = desugar_source("|box#a|\n|box#b|\n|box#c|\na -> b -> c \"step\"\n").unwrap();
    assert_eq!(out.matches("\"step\"").count(), 2, "{out}");
}

#[test]
fn chain_hops_keep_their_own_operators() {
    // The bare-first-hop spelling [SPEC 9]: `a - b -> c` — and a fan hop
    // stays a fan (`&` is routing geometry, not sugar [SPEC 19]).
    let out = desugar_source("|box#a|\n|box#b|\n|box#c|\na - b <-> c\n").unwrap();
    assert!(out.contains("a - b\n"), "{out}");
    assert!(out.contains("b <-> c\n"), "{out}");
    let out = desugar_source("|box#a|\n|box#b|\n|box#c|\n|box#d|\na -> b -> c & d\n").unwrap();
    assert!(out.contains("a -> b\n"), "{out}");
    assert!(out.contains("b -> c & d\n"), "{out}");
}

#[test]
fn a_chain_auto_creates_every_hops_endpoints_once() {
    // Auto-created ids ride the expansion [SPEC 19]: the shared middle id is
    // created once, at the root.
    let out = desugar_source("x -> y -> z\n").unwrap();
    for id in ["x", "y", "z"] {
        assert_eq!(
            out.matches(&format!("|block#{id}| ")).count(),
            1,
            "{id} created once: {out}"
        );
    }
}

#[test]
fn mixing_op_kinds_in_a_chain_stays_a_parse_error() {
    let e = desugar_source("|box#a|\n|box#b|\n|box#c|\na -> b (-) c\n")
        .expect_err("mixed kinds error")
        .to_string();
    assert!(e.contains("mixes operators"), "{e}");
}

#[test]
fn a_tree_keeps_topic_nesting_wears_level_classes_and_fans_branches() {
    // Topic nesting is preserved; each topic wears its depth class, and each
    // parent's edges become one dotted branch fan on the parent's port,
    // generated in the scope that contains the parent [SPEC 12].
    let out = desugar_source(
        "|column#o| { layout: tree } [\n  |topic#a| \"A\" [\n    |topic#b| \"B\"\n    |topic#c| \"C\"\n  ]\n]\n",
    )
    .unwrap();
    assert!(
        out.contains("|block#a| .lini-topic.lini-block.lini-level-0"),
        "{out}"
    );
    assert!(
        out.contains("|block#b| .lini-topic.lini-block.lini-level-1"),
        "{out}"
    );
    assert!(
        out.contains("|block#c| .lini-topic.lini-block.lini-level-1"),
        "{out}"
    );
    // One fan per parent, endpoints dotted from the parent's scope, with the
    // column direction's forced sides.
    assert!(
        out.contains("a:bottom - a.b:top & a.c:top"),
        "branch fan: {out}"
    );
    // The default gap is injected (the generic 20 is unroutable at clearance 16).
    assert!(out.contains("gap: 64 48"), "{out}");
    // The topic template is a generated class.
    assert!(out.contains(".lini-topic {"), "{out}");
}

#[test]
fn a_row_tree_fans_on_the_right_side() {
    let out = desugar_source(
        "|column#o| { layout: tree; direction: row } [\n  |topic#a| \"A\" [\n    |topic#b| \"B\"\n  ]\n]\n",
    )
    .unwrap();
    assert!(
        out.contains("a:right - a.b:left"),
        "row fan on right side: {out}"
    );
}

#[test]
fn a_bilateral_tree_splits_the_first_level_and_fans_both_sides() {
    // First ⌈n/2⌉ first-level topics fill the right, the rest the left; an
    // authored `side:` overrides its half; the root emits one fan per half with
    // mirrored sides, and each half grows on that side [SPEC 12].
    let out = desugar_source(
        "|column#o| { layout: tree; direction: bilateral } [\n  |topic#r| \"R\" [\n    |topic#a| \"A\" [ |topic#a1| \"A1\" ]\n    |topic#b| \"B\"\n    |topic#c| \"C\"\n    |topic#d| \"D\" { side: right }\n  ]\n]\n",
    )
    .unwrap();
    // n = 4: a,b default right, c,d default left; d overridden back to right.
    assert!(
        out.contains("|block#a| .lini-topic.lini-block.lini-side-right.lini-level-1"),
        "a right: {out}"
    );
    assert!(
        out.contains("|block#c| .lini-topic.lini-block.lini-side-left.lini-level-1"),
        "c left: {out}"
    );
    assert!(
        out.contains("|block#d| .lini-topic.lini-block.lini-side-right.lini-level-1"),
        "d overridden right: {out}"
    );
    // The authored `side:` is consumed — no raw property survives to resolve.
    assert!(!out.contains("side:"), "side consumed: {out}");
    // The root's two fans, mirrored per half.
    assert!(
        out.contains("r:right - r.a:left & r.b:left & r.d:left"),
        "right fan: {out}"
    );
    assert!(out.contains("r:left - r.c:right"), "left fan: {out}");
    // A deeper right-half subtree keeps the right orientation.
    assert!(out.contains("a:right - a.a1:left"), "deep right fan: {out}");
}

#[test]
fn a_bilateral_tree_is_a_desugar_fixed_point() {
    let src = "|column#o| { layout: tree; direction: bilateral } [\n  |topic#r| \"R\" [\n    |topic#a| \"A\"\n    |topic#b| \"B\"\n    |topic#c| \"C\" { side: right }\n  ]\n]\n";
    let once = desugar_source(src).unwrap();
    let twice = desugar_source(&once).unwrap();
    assert_eq!(
        once, twice,
        "re-desugaring the lowered bilateral tree changes it"
    );
}

#[test]
fn a_tree_is_a_desugar_fixed_point() {
    let src = "|column#o| { layout: tree } [\n  |topic#a| \"A\" [\n    |topic#b| \"B\"\n    |topic#c| \"C\"\n  ]\n]\n";
    let once = desugar_source(src).unwrap();
    let twice = desugar_source(&once).unwrap();
    assert_eq!(once, twice, "re-desugaring the lowered tree changes it");
}

#[test]
fn a_root_tree_is_a_byte_fixed_point() {
    // The generated root fan's span seats past the instances, so fmt's
    // phase split prints identically on first lowering and re-lowering.
    let src = "{ layout: tree; }\n|topic#r| \"R\" [\n  |topic#a| \"A\"\n  |topic#b| \"B\"\n]\n";
    let once = desugar_source(src).unwrap();
    let twice = desugar_source(&once).unwrap();
    assert_eq!(
        once, twice,
        "re-desugaring the lowered root tree changes it"
    );
}

#[test]
fn a_mindmap_seats_its_scene_and_lowers_the_preset() {
    // The |mindmap| preset [SPEC 8]: the node is the visible root topic; its
    // scene becomes the generated tree scope (`layout: tree; direction:
    // bilateral; routing: natural`), and the three garnishes lower as ordinary
    // rules — the wrap cap + weight reset, the depth ramp, and the palette
    // walk's tints — all visible in `lini desugar`.
    let out = desugar_source(
        "|mindmap#m| \"M\" [\n  |topic#a| \"A\" [ |topic#a1| \"A1\" ]\n  |topic#b| \"B\"\n  |topic| \"C\"\n]\n",
    )
    .unwrap();
    for decl in [
        "layout: tree;",
        "direction: bilateral;",
        "routing: natural;",
    ] {
        assert!(out.contains(decl), "scope trio on the root: {decl}: {out}");
    }
    assert!(
        out.contains(".lini-mindmap .lini-topic { max-width: 160; font-weight: medium; }"),
        "wrap cap + weight reset: {out}"
    );
    assert!(
        out.contains(".lini-mindmap .lini-level-1 { font-size: 15; }")
            && out.contains(".lini-mindmap .lini-level-2 { font-size: 14; }"),
        "the depth ramp: {out}"
    );
    assert!(
        out.contains(".lini-mindmap .lini-hue-rose {")
            && out.contains("fill: --rose-wash; stroke: --rose-deep; color: --rose-ink;"),
        "a hue tint at the tiers: {out}"
    );
    // The root stays neutral — level 0, no hue class.
    assert!(
        out.contains("|block#m| .lini-mindmap.lini-topic.lini-block.lini-level-0 ["),
        "neutral root: {out}"
    );
    // Per-branch tinted root arms (declaration order: a rose, b orange, the
    // anonymous branch amber on the left half), and the subtree wire wears its
    // branch's hue.
    assert!(out.contains("m:right - m.a:left .lini-hue-rose"), "{out}");
    assert!(out.contains("m:right - m.b:left .lini-hue-orange"), "{out}");
    assert!(
        out.contains("m:left - m.lini-topic-3:right .lini-hue-amber"),
        "anonymous branch arm: {out}"
    );
    assert!(
        out.contains("a:right - a.a1:left .lini-hue-rose"),
        "subtree wire tinted: {out}"
    );
}

#[test]
fn the_palette_walk_skips_red_and_grey_and_wraps_past_nine() {
    // Ten branches: the walk order is the HUES table with red and grey
    // skipped — rose orange amber lime green teal sky blue purple — and the
    // tenth branch wraps back to rose [SPEC 8].
    let branches: String = (1..=10)
        .map(|i| format!("  |topic#b{i}| \"B{i}\"\n"))
        .collect();
    let out = desugar_source(&format!("|mindmap#m| \"M\" [\n{branches}]\n")).unwrap();
    let order = [
        "rose", "orange", "amber", "lime", "green", "teal", "sky", "blue", "purple", "rose",
    ];
    for (i, hue) in order.iter().enumerate() {
        assert!(
            out.contains(&format!(
                "|block#b{}| .lini-topic.lini-block.lini-side-",
                i + 1
            )) && out.contains(&format!(
                ".lini-level-1.lini-hue-{hue} [\n    \"B{}\"",
                i + 1
            )),
            "branch {} wears {hue}: {out}",
            i + 1
        );
    }
    assert!(
        !out.contains("hue-red") && !out.contains("hue-gray"),
        "red and grey never assigned: {out}"
    );
}

#[test]
fn a_mindmap_is_a_desugar_fixed_point() {
    let src = "|mindmap#m| \"M\" [\n  |topic#a| \"A\" [ |topic| \"A1\" ]\n  |topic#b| \"B\" { side: left }\n  |topic| \"C\"\n]\n";
    let once = desugar_source(src).unwrap();
    let twice = desugar_source(&once).unwrap();
    assert_eq!(once, twice, "re-desugaring the lowered mindmap changes it");
}

#[test]
fn a_mindmap_hoists_its_own_routing_to_the_scope() {
    // `|mindmap| { routing: orthogonal }` must govern the WHOLE tree — the
    // root's arms live in the generated scope, not the root card's body, so a
    // routing left on the node would split the tree across two strategies.
    let out = desugar_source("|mindmap#m| \"M\" { routing: orthogonal } [\n  |topic#a| \"A\"\n]\n")
        .unwrap();
    assert!(out.contains("routing: orthogonal;"), "hoisted: {out}");
    assert!(
        !out.contains("routing: natural"),
        "the preset does not fight the authored value: {out}"
    );
}

#[test]
fn a_mindmap_hoists_its_own_direction_to_the_scope() {
    // `|mindmap| { direction: row }` steers the generated tree scope, not the
    // root card's own content [SPEC 8]; authored scene config still wins.
    let out =
        desugar_source("|mindmap#m| \"M\" { direction: row } [\n  |topic#a| \"A\"\n]\n").unwrap();
    assert!(out.contains("direction: row;"), "hoisted: {out}");
    assert!(
        out.contains("m:right - m.a:left .lini-hue-rose"),
        "a row mindmap fans rightward, arm tinted: {out}"
    );
}

// ── Capsule endpoints [SPEC 9/19] ──

#[test]
fn a_capsule_endpoint_hoists_a_declaration_and_references_it() {
    let out = desugar_source("cat -> |cyl#db| \"watches\" { stroke: red }\n").unwrap();
    // The declaration, at the statement's position; the tail is the LINK's.
    assert!(out.contains("|cyl#db| .lini-cyl"), "{out}");
    assert!(out.contains("cat -> db"), "{out}");
    assert!(out.contains("stroke: red"), "{out}");
    assert!(
        out.contains("[ \"watches\" ]"),
        "label is the link's: {out}"
    );
}

#[test]
fn a_statement_head_capsule_hoists_too() {
    let out = desugar_source("|cyl#db| -> cat\n").unwrap();
    assert!(out.contains("|cyl#db| .lini-cyl"), "{out}");
    assert!(out.contains("db -> cat"), "{out}");
}

#[test]
fn an_anonymous_capsule_mints_a_reserved_id_once_per_chain() {
    let out = desugar_source("a -> |box| -> c\n").unwrap();
    assert!(out.contains("a -> lini-cap-1"), "{out}");
    assert!(out.contains("lini-cap-1 -> c"), "{out}");
    assert_eq!(
        out.matches("|block#lini-cap-1| ").count(),
        1,
        "one instance for the mid-chain capsule: {out}"
    );
}

#[test]
fn a_fan_into_a_capsule_is_one_instance() {
    let out = desugar_source("a & b -> |cyl#store|\n").unwrap();
    assert!(out.contains("a & b -> store"), "{out}");
    assert_eq!(out.matches("|cyl#store|").count(), 1, "{out}");
}

#[test]
fn minting_skips_taken_names() {
    // A lowered scope already holding lini-cap-1 gains a new anonymous
    // capsule: the mint skips to lini-cap-2 instead of colliding.
    let src = "|block#lini-cap-1| .lini-box.lini-block\nx -> |box|\n";
    let out = desugar_source(src).unwrap();
    assert!(out.contains("x -> lini-cap-2"), "{out}");
}

#[test]
fn a_capsule_statement_is_a_byte_fixed_point() {
    for src in [
        "cat -> |cyl#db|\n",
        "a -> |box| -> c\n",
        "a & b -> |cyl#store| \"s\"\n",
        "|group#g| [\n  a -> |cyl#db|\n]\n",
    ] {
        let once = desugar_source(src).unwrap();
        let twice = desugar_source(&once).unwrap();
        assert_eq!(once, twice, "not a fixed point for: {src}");
    }
}

#[test]
fn a_body_capsule_declares_inside_its_scope() {
    let out = desugar_source("|group#g| [\n  a -> |cyl#db|\n]\n").unwrap();
    // The declaration and the auto-created `a` both live in g's body.
    let body = out.split("|block#g|").nth(1).expect("g's body");
    assert!(body.contains("|cyl#db| .lini-cyl"), "{out}");
    assert!(body.contains("a -> db"), "{out}");
}

#[test]
fn a_define_body_capsule_materializes_per_instance() {
    let src = "{ |room::group| [ a -> |cyl#db| ] }\n|room#r1|\n|room#r2|\n";
    let out = desugar_source(src).unwrap();
    assert_eq!(
        out.matches("|cyl#db| .lini-cyl").count(),
        2,
        "one declaration per materialized body: {out}"
    );
}

#[test]
fn a_capsule_in_a_drawing_scope_errors() {
    let err = desugar_source("{ layout: drawing }\n|rect#r| { width: 4; height: 2 }\nr -> |box|\n")
        .expect_err("drawing capsule");
    assert!(
        err.to_string()
            .contains("a drawing never invents an endpoint"),
        "{err}"
    );
}

#[test]
fn a_capsule_id_never_reauto_creates_elsewhere() {
    // A later bare reference to a capsule-declared id uses the declaration.
    let out = desugar_source("a -> |cyl#db|\ndb -> c\n").unwrap();
    assert_eq!(out.matches("|cyl#db|").count(), 1, "{out}");
    assert!(!out.contains("|block#db|"), "no auto-created twin: {out}");
}

// ── Schematic types [SPEC 16] ──

#[test]
fn a_component_splits_pins_bilaterally_into_anonymous_rails() {
    // 3 auto pins: ⌈3/2⌉ = 2 left, 1 right; an explicit `side:` is excluded
    // from the count [SPEC 16.2]. Rails are anonymous — scope-transparent.
    let out = desugar_source(
        "|component#U7| \"IC\" [\n  |pin#a| { number: 1 }\n  |pin#b| { number: 2 }\n  |pin#c| { number: 3 }\n  |pin#d| { number: 4; side: right }\n]\n",
    )
    .unwrap();
    let left = out.split("align: start").nth(1).expect("left rail");
    assert!(
        left.contains("|block#a|") && left.contains("|block#b|"),
        "{out}"
    );
    let right = out.split("align: end").nth(1).expect("right rail");
    assert!(
        right.contains("|block#c|") && right.contains("|block#d|"),
        "{out}"
    );
    // Pin chrome: stubs and number readouts wear the one class each.
    assert!(out.contains(".lini-pin-stub {"), "{out}");
    assert!(out.contains(".lini-pin-number {"), "{out}");
    // The unlabelled pin displays its id; the value readout sits above.
    assert!(out.contains("[ \"a\" ") || out.contains("\"a\"\n"), "{out}");
    assert!(out.contains(".lini-part-value {"), "{out}");
}

#[test]
fn a_connector_generates_numbered_nameless_pins() {
    let out = desugar_source("|J#J3| \"header\" { pins: 3 }\n").unwrap();
    for p in ["|block#p1|", "|block#p2|", "|block#p3|"] {
        assert!(out.contains(p), "{p}: {out}");
    }
    assert!(out.contains("number: 3"), "{out}");
}

#[test]
fn a_discrete_lowers_its_symbol_ports_and_readouts() {
    let out = desugar_source("|R#R5| \"470\"\n").unwrap();
    assert!(out.contains(".lini-sch-line {"), "one symbol rule: {out}");
    assert!(out.contains("|path| .lini-sch-line.lini-path"), "{out}");
    assert!(
        out.contains("|block#p1|") && out.contains("|block#p2|"),
        "{out}"
    );
    assert!(out.contains("[ \"R5\" ]"), "id as the drawn ref: {out}");
    assert!(out.contains("\"470\""), "value readout: {out}");
}

#[test]
fn discrete_variants_pick_glyph_and_pin_ids() {
    let out = desugar_source("|Q#q1| { symbol: nfet }\n").unwrap();
    for p in ["|block#g|", "|block#d|", "|block#s|"] {
        assert!(out.contains(p), "{p}: {out}");
    }
    let err = desugar_source("|D#d1| { symbol: zenr }\n").expect_err("unknown variant");
    assert!(err.to_string().contains("unknown symbol 'zenr'"), "{err}");
    assert!(err.to_string().contains("zener"), "{err}");
}

#[test]
fn labels_lower_text_symbol_and_shape() {
    let out = desugar_source("|gnd|\n").unwrap();
    assert!(out.contains(".lini-sch-tag-line {"), "{out}");
    let out = desugar_source("|label#run| \"RUN\" { shape: round }\n").unwrap();
    assert!(
        out.contains(".lini-tag-outline.lini-tag-round.lini-label.lini-block"),
        "shape classes lead the chain: {out}"
    );
    let err = desugar_source("|label#x| \"X\" { symbol: gnb }\n").expect_err("unknown symbol");
    assert!(err.to_string().contains("did you mean 'gnd'?"), "{err}");
}

#[test]
fn a_power_flag_define_reads_its_symbol_through_the_chain() {
    // [SPEC 16.4]: a power net is a one-line define with intrinsic text.
    let out = desugar_source("{ |vm::label| { symbol: power } [ \"VM\" ] }\n|vm#v1|\n").unwrap();
    assert!(
        out.contains("M 8 14 L 8 5"),
        "the power glyph lowered: {out}"
    );
}

#[test]
fn anonymous_parts_mint_display_refs_skipping_taken_ids() {
    let out = desugar_source("|R#R2| \"a\"\n|R| \"b\"\n|R| \"c\"\n").unwrap();
    // R2 authored; anonymous parts mint R1 then R3.
    assert!(out.contains("[ \"R1\" ]"), "{out}");
    assert!(out.contains("[ \"R3\" ]"), "{out}");
    // `prefix:` overrides.
    let out = desugar_source("|component| \"x\" { prefix: \"IC\" }\n").unwrap();
    assert!(out.contains("[ \"IC1\" ]"), "{out}");
}

#[test]
fn schematic_lowerings_are_byte_fixed_points() {
    for src in [
        "|component#U7| \"IC\" [\n  |pin#a| { number: 1 }\n  |pin#b| { number: 2 }\n]\n",
        "|R#R5| \"470\"\n|R| \"1k\"\n",
        "|J#J3| { pins: 2 }\n",
        "|gnd|\n|label#run| \"RUN\" { shape: round }\n",
        "|opamp#u1|\n",
    ] {
        let once = desugar_source(src).unwrap();
        let twice = desugar_source(&once).unwrap();
        assert_eq!(once, twice, "not a fixed point for: {src}");
    }
}

#[test]
fn anonymous_parts_generate_no_pin_terminals() {
    // An anonymous part is scope-transparent [SPEC 9] — generated `p1` ids
    // would collide across two anonymous |R|s — and unwirable (no dot-path),
    // so only an id'd part generates its port nodes.
    let out = desugar_source("|R| \"a\"\n|R| \"b\"\n").unwrap();
    assert!(!out.contains("|block#p1|"), "{out}");
    let out = desugar_source("|R#r1| \"a\"\n").unwrap();
    assert!(out.contains("|block#p1|"), "{out}");
}

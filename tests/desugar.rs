//! `lini desugar` lowers ALL sugar to primitives + `.lini-*` classes: typed
//! instances become primitives wearing their `.lini-*` chain, templates/defines
//! collapse into generated class defs, scene/link defaults fill the global block,
//! and labels / `along:` become explicit. The lowered form is a fixed point.

use lini::desugar_source;

/// The first lowered `path:` value in a desugared source — a schematic
/// symbol's linework, as the glyph pass wrote it.
fn lowered_path(out: &str) -> String {
    let at = out.find("path: \"").expect("a lowered symbol path");
    let rest = &out[at + 7..];
    rest[..rest.find('"').expect("closing quote")].to_string()
}

/// Every `(x, y)` in a path `d`, in order — the command letters parse away.
fn points(d: &str) -> Vec<(f64, f64)> {
    let n: Vec<f64> = d
        .split_whitespace()
        .filter_map(|t| t.parse().ok())
        .collect();
    n.as_chunks::<2>().0.iter().map(|c| (c[0], c[1])).collect()
}

/// A path's `(width, height)`.
fn extent(d: &str) -> (f64, f64) {
    let pts = points(d);
    let (xs, ys): (Vec<f64>, Vec<f64>) = pts.iter().copied().unzip();
    let span = |v: &[f64]| {
        v.iter().copied().fold(f64::MIN, f64::max) - v.iter().copied().fold(f64::MAX, f64::min)
    };
    (span(&xs), span(&ys))
}

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
    let mut swept = 0;
    for path in lini::testing::samples() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let src = lini::testing::read_sample(&path);
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
fn an_explicit_layout_drawing_opens_a_scope_it_does_not_seal() {
    // [SPEC 15.1]: the `layout:` that *opens* a drawing scope is the very decl
    // the seal reads, so it must not seal the scope against its own children —
    // `|group| { layout: drawing }` folds a child's `scale:` exactly as
    // `|drawing|` does.
    let explicit =
        desugar_source("|group#v| { layout: drawing } [ |rect#r| { scale: 2; width: 4 } ]\n")
            .unwrap();
    assert!(
        explicit.contains("px-per-unit: 8"),
        "2 × mm × 4: {explicit}"
    );
    let typed = desugar_source("|drawing#v| [ |rect#r| { scale: 2; width: 4 } ]\n").unwrap();
    assert!(typed.contains("px-per-unit: 8"), "{typed}");
    // A child that owns a layout of its own still seals the inherited scope.
    let sealed =
        desugar_source("|drawing#v| [ |row#r| [ |rect#q| { scale: 2; width: 4 } ] ]\n").unwrap();
    assert!(!sealed.contains("px-per-unit: 8"), "{sealed}");
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
fn desugar_runs_the_same_gates_the_compiler_does() {
    // [SPEC 20]: the lowered form re-renders identically, so what `lini desugar`
    // accepts is what `lini build` accepts — a two-root tree fails both, with
    // the same error, rather than lowering happily and failing at compile.
    let src = "{ layout: tree }\n|topic#a|\n|topic#b|\n";
    let desugared = desugar_source(src).expect_err("two roots");
    let compiled = lini::check(src).expect_err("two roots");
    assert!(
        desugared.to_string().contains("a tree has one root"),
        "{desugared}"
    );
    assert_eq!(desugared.to_string(), compiled.to_string());
}

#[test]
fn a_repeated_declaration_is_read_last_wins_everywhere() {
    // [SPEC 4]: a later declaration overrides an earlier one, and the lowered
    // form keeps no sugar — the second `sheet:` sizes the page and neither
    // survives as a declaration.
    let out = desugar_source("|page#p| { sheet: a3; sheet: a5 }\n").unwrap();
    assert!(out.contains("width: 148; height: 210;"), "{out}");
    assert!(!out.contains("sheet:"), "the sugar is gone: {out}");
    // A losing value is still read, so a typo is never silently overridden.
    assert!(lini::check("|page#p| { sheet: zz; sheet: a5 }\n").is_err());
    // And the gates read the same winner desugar lowered by: a root that ends
    // in `layout: tree` **is** a tree, so `|topic|` belongs in it.
    lini::check("{ layout: flow; layout: tree }\n|topic#a| [ |topic#b| ]\n")
        .expect("the last layout wins");
    let err = lini::check("{ layout: tree; layout: flow }\n|topic#a|\n")
        .expect_err("a flow root takes no topic");
    assert!(err.to_string().contains("builds a tree"), "{err}");
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
    // The default gap is injected (the generic 36 is too tight for a tree's generations).
    assert!(out.contains("gap: 64 48"), "{out}");
    // The topic template is a generated class.
    assert!(out.contains(".lini-topic {"), "{out}");
}

#[test]
fn a_minted_topic_id_keeps_its_ordinal_and_steps_over_a_taken_one() {
    // [SPEC 12]: `lini-topic-N` is 1-based among the scope's topics, so an
    // authored sibling spends its ordinal — the anonymous topic after `#b` is 2.
    let out = desugar_source(
        "|column#o| { layout: tree } [\n  |topic#a| \"A\" [\n    |topic#b| \"B\"\n    |topic| \"C\"\n  ]\n]\n",
    )
    .unwrap();
    assert!(out.contains("|block#lini-topic-2|"), "{out}");
    assert!(
        out.contains("a:bottom - a.b:top & a.lini-topic-2:top"),
        "{out}"
    );

    // [SPEC 19/23]: the lowered form legitimately carries `lini-topic-N` ids, so
    // a scope that mixes one with an anonymous topic must be stepped over, never
    // minted onto twice (the duplicate broke both the fan and the id table).
    let src = "|column#o| { layout: tree } [\n  |topic#a| \"A\" [\n    |block#lini-topic-2| .lini-topic.lini-block [ \"kept\" ]\n    |topic| \"new\"\n  ]\n]\n";
    let once = desugar_source(src).unwrap();
    assert_eq!(
        once.matches("|block#lini-topic-2|").count(),
        1,
        "the taken id is minted onto: {once}"
    );
    assert!(once.contains("|block#lini-topic-3|"), "{once}");
    assert!(
        lini::compile_str(&once).is_ok(),
        "the lowered form must compile: {once}"
    );
    assert_eq!(
        desugar_source(&once).unwrap(),
        once,
        "a fixed point: {once}"
    );
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

/// The lowered form is a fixed point, byte for byte — re-lowering it changes
/// nothing. One row per sugar the desugar pass mints something for: a capsule
/// endpoint, a tree's generated fan, a mindmap's arms. (The schematic family
/// has its own table below; the whole showroom is swept by
/// `every_sample_is_a_byte_identical_desugar_fixed_point`.)
#[test]
fn every_sugar_lowers_to_a_byte_fixed_point() {
    for src in [
        // Capsule endpoints [SPEC 9].
        "cat -> |cyl#db|\n",
        "a -> |box| -> c\n",
        "a & b -> |cyl#store| \"s\"\n",
        "|group#g| [\n  a -> |cyl#db|\n]\n",
        // Trees [SPEC 12] — nested, bilateral, and the root form whose
        // generated fan seats its span past the instances, so fmt's phase
        // split prints identically on first lowering and re-lowering.
        "|column#o| { layout: tree } [\n  |topic#a| \"A\" [\n    |topic#b| \"B\"\n    |topic#c| \"C\"\n  ]\n]\n",
        "|column#o| { layout: tree; direction: bilateral } [\n  |topic#r| \"R\" [\n    |topic#a| \"A\"\n    |topic#b| \"B\"\n    |topic#c| \"C\" { side: right }\n  ]\n]\n",
        "{ layout: tree; }\n|topic#r| \"R\" [\n  |topic#a| \"A\"\n  |topic#b| \"B\"\n]\n",
        // A mindmap's own arms.
        "|mindmap#m| \"M\" [\n  |topic#a| \"A\" [ |topic| \"A1\" ]\n  |topic#b| \"B\" { side: left }\n  |topic| \"C\"\n]\n",
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
fn a_symbol_part_lowers_its_glyph_ahead_of_authored_text() {
    // [SPEC 16.3/16.4] "text beside it like an icon's": the glyph leads,
    // authored content follows — and unlike a part's readouts (`pin:`
    // overlays), both are **in flow**, so their order is layout. A generated
    // node carries an empty span and `fmt` emits a body in span order, so the
    // glyph must be *inserted* ahead of the authored text, not appended:
    // appending printed it first and made the lowered source a different
    // program from its source. All three symbol lowerings — label, discrete,
    // opamp — seat it through the one `seat_glyph`, so all three are pinned
    // here. (`tests/oracle.rs` sweeps the same property over the samples; the
    // `[ ]` content form no sample uses is why the discretes escaped it.)
    for (part, class, text) in [
        (
            "|label#pwr| { symbol: power } [ \"5V\" ]\n",
            "sch-tag-line",
            "5V",
        ),
        ("|R#r1| [ \"1k\" ]\n", "sch-line", "1k"),
        ("|opamp#o1| [ \"amp\" ]\n", "sch-line", "amp"),
    ] {
        // The part lives in its scope — a schematic type outside one is the
        // out-of-scope gate [SPEC 21], not a lowering.
        let src = &format!("{{ layout: schematic }}\n{part}");
        let out = desugar_source(src).unwrap();
        let glyph = out
            .find(&format!("lini-{class}.lini-path"))
            .unwrap_or_else(|| panic!("no glyph in: {out}"));
        let text = out
            .find(&format!("\"{text}\""))
            .unwrap_or_else(|| panic!("no authored text in: {out}"));
        assert!(glyph < text, "the glyph leads the text: {out}");
        assert_eq!(
            lini::compile_str(src).unwrap(),
            lini::compile_str(&out).unwrap(),
            "the lowered part renders identically: {src}"
        );
    }
}

#[test]
fn a_power_flag_define_reads_its_symbol_through_the_chain() {
    // [SPEC 16.4]: a power net is a one-line define with intrinsic text.
    let through_the_chain =
        desugar_source("{ |vm::label| { symbol: power } [ \"VM\" ] }\n|vm#v1|\n").unwrap();
    let stated_directly = desugar_source("|label#v1| { symbol: power } [ \"VM\" ]\n").unwrap();
    // The define chain reaches the same glyph the direct spelling lowers —
    // the geometry itself is pinned by the conformance snapshots, not here.
    assert_eq!(
        lowered_path(&through_the_chain),
        lowered_path(&stated_directly),
        "the chain lowered a different glyph:\n{through_the_chain}"
    );
    assert!(!lowered_path(&through_the_chain).is_empty());
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
        // Posed parts: the turn is consumed, so re-lowering must not turn again.
        "|component#U7| { rotate: 90 } [\n  |pin#a|; |pin#b|; |pin#c|\n]\n",
        "|R#r5| \"470\" { rotate: 270 }\n|gnd#g1| { rotate: 180 }\n",
        "{ |vert::R| { rotate: 90 } }\n|vert#r9|\n",
        // A posed sheet whose tags and grounds are all minted at desugar — the
        // label wire, its shaping marker, and the capsule form [SPEC 16.5].
        "|schematic#s| [\n  |component#U7| [ |pin#a| { side: left }; |pin#b|; |pin#c| ]\n  U7.a -> \"NSTDBY\"\n  U7.b - |gnd|\n]\n",
        "{ |sig::pin| { translate: 6 0; side: top } }\n|component#U7| { rotate: 90 } [ |sig#a| ]\n",
        // The **carrier**: a tag minted in a nested ordinary container inside a
        // sheet must re-lower unchanged, which is only true if the lowered
        // sheet still answers "schematic" — a nested one states it as a class,
        // not as its type [SPEC 16].
        "|schematic#s| [\n  |row#r| [\n    |component#U7| [ |pin#a| ]\n    U7.a -> \"NET\"\n  ]\n]\n",
        "{ layout: schematic; }\n|group#g| [\n  |component#U8| [ |pin#z| ]\n  U8.z - \"NET\"\n]\n",
        // The **landings** [SPEC 16.5]: a resolved pin prints as an ordinary
        // named one, so re-lowering has nothing left to resolve.
        "{ layout: schematic; }\n|component#u1| [ |pin#a| ]\n|D#d1|\n|gnd#g1|\nu1 - d1.k - g1\n",
        "{ layout: schematic; }\n|R#r1|\n|gnd#g1|\nr1 - r1 - g1\n",
        "{ layout: schematic; }\n|component#a1| { cell: 1 1 } [ |pin#p1| { side: left }; |pin#p2| { side: right } ]\n|gnd#g0|\n|C#c1|\na1.p1 - g0\na1 - c1\n",
        // …and one the sheet cannot resolve — a part inside a nested container
        // is resolve's to land, so the lowered form still says `r.r1`.
        "{ layout: schematic; }\n|component#u1| [ |pin#a|; |pin#b|; |pin#c| ]\n|row#r| [ |R#r1| ]\nu1.a - r.r1\nu1.b - r.r1\n",
    ] {
        let once = desugar_source(src).unwrap();
        let twice = desugar_source(&once).unwrap();
        assert_eq!(once, twice, "not a fixed point for: {src}");
        // …and the lowering itself is deterministic.
        assert_eq!(once, desugar_source(src).unwrap(), "unstable: {src}");
    }
}

#[test]
fn a_resolved_landing_means_the_same_program_after_lowering() {
    // [SPEC 16.5, tests/oracle.rs] **The** reason the arity law resolves at
    // desugar: `split_chain` states a chain as one link per hop, and a hop is
    // exactly the statement a pass-through is defined over. Byte-stability
    // cannot catch this on its own — the lowered text was stable all along, it
    // simply meant a different circuit — so each form is *compiled* and
    // compared.
    for src in [
        // SPEC 16.5's own example: enters the cathode, leaves by the anode.
        "{ layout: schematic }\n|component#u1| [ |pin#a| ]\n|D#d1|\n|gnd#g1|\nu1 - d1.k - g1\n",
        // The degenerate chain: it compiled from source and failed (R021) from
        // its own lowered form.
        "{ layout: schematic }\n|R#r1|\n|gnd#g1|\nr1 - r1 - g1\n",
        // A series run through two parts, and a reserved pin pushing a pinless
        // landing onto p2.
        "{ layout: schematic }\n|component#u1| [ |pin#a| ]\n|R#r1|\n|LED#d1|\n|gnd#g1|\nu1 - r1 - d1 - g1\n",
        "{ layout: schematic }\n|component#u1| [ |pin#a|; |pin#b|; |pin#c| ]\n|R#r1|\n|gnd#g1|\nu1.a - r1.p2\nu1.b - r1\n",
        // A landing only resolve can make (into a nested container) — named,
        // and through an **anonymous** one, which is scope-transparent to a
        // path but still runs its own gather.
        "{ layout: schematic }\n|component#u1| [ |pin#a|; |pin#b|; |pin#c| ]\n|row#r| [ |R#r1| ]\nu1.a - r.r1\nu1.b - r.r1\n",
        "{ layout: schematic }\n|group| [\n  |R#r1|\n  |gnd#gi|\n  r1 - gi\n]\n|gnd#go|\ngo - r1\n",
        // …and a chain **through** such a part — both container shapes, both
        // spellings. Desugar cannot resolve the landing, so it must leave the
        // chain whole: hops would say "a junction at that pin" instead, which
        // is a different circuit.
        "{ layout: schematic }\n|component#u1| [ |pin#a|; |pin#b|; |pin#c| ]\n|group| [ |R#r1| ]\n|gnd#g1|\nu1.a - r1 - g1\n",
        "{ layout: schematic }\n|component#u1| [ |pin#a|; |pin#b|; |pin#c| ]\n|group#gp| [ |R#r1| ]\n|gnd#g1|\nu1.a - gp.r1 - g1\n",
        "{ layout: schematic }\n|component#u1| [ |pin#a|; |pin#b|; |pin#c| ]\n|group| [ |D#d1| ]\n|gnd#g1|\nu1.a - d1.k - g1\n",
        "{ layout: schematic }\n|component#u1| [ |pin#a|; |pin#b|; |pin#c| ]\n|group#gp| [ |D#d1| ]\n|gnd#g1|\nu1.a - gp.d1.k - g1\n",
        // …and an `&` fan in such a chain [SPEC 9]: the legs share an end, so
        // the hops away from the fan are one wire each, at both stages.
        "{ layout: schematic }\n|box#a|\n|box#b|\n|box#x|\n|box#c|\na & b - x - c\n",
        "{ layout: schematic }\n|component#u1| [ |pin#a|; |pin#b|; |pin#c| ]\n|group| [ |R#r1| ]\n|gnd#g1|\nu1.a & u1.b - r1 - g1\n",
        "{ layout: schematic }\n|component#u1| [ |pin#a|; |pin#b|; |pin#c| ]\n|group| [ |R#r1| ]\nu1.a - r1 - u1.b & u1.c\n",
    ] {
        let lowered = desugar_source(src).unwrap();
        assert_eq!(
            lini::compile_str(src).expect("source compiles"),
            lini::compile_str(&lowered).expect("its lowered form compiles"),
            "compile(src) != compile(desugar(src)) for: {src}"
        );
    }
}

#[test]
fn a_pinless_landing_poses_the_satellite_it_actually_reaches() {
    // [SPEC 16.5/16.1] The landing resolves *before* the pose chooser reads
    // it, so how the author spelled it cannot change which pin a satellite is
    // turned to face: `a1 - c1` (p1 already taken, so the cap lands on p2) is
    // the same program as writing `a1.p2 - c1` by hand.
    let sheet = |wire: &str| {
        format!(
            "{{ layout: schematic }}\n\
             |component#a1| {{ cell: 1 1 }} [ |pin#p1| {{ side: left }}; |pin#p2| {{ side: right }} ]\n\
             |gnd#g0|\n|C#c1|\na1.p1 - g0\n{wire}\n"
        )
    };
    assert_eq!(
        desugar_source(&sheet("a1 - c1")).unwrap(),
        desugar_source(&sheet("a1.p2 - c1")).unwrap(),
        "the pinless spelling lowers to the pin-named one"
    );
    // …and the pin it names is the free one, stated in the lowered source.
    let out = desugar_source(&sheet("a1 - c1")).unwrap();
    assert!(out.contains("a1.p2 - c1.p1"), "{out}");
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

// ── Pose [SPEC 16.1] — `rotate:` is read at lowering, never painted ──

/// Each pin's id paired with the side its stub points out of, in lowered
/// document order — a pin's landed side, read the way the router will read it.
fn pin_sides(out: &str) -> Vec<(String, String)> {
    let mut seen: Vec<(String, String)> = Vec::new();
    let mut id = String::new();
    for line in out.lines() {
        if let Some(rest) = line.split("|block#").nth(1)
            && line.contains(".lini-pin.")
        {
            id = rest.split('|').next().unwrap_or_default().to_string();
        }
        if line.contains("points: 0 0")
            && let Some(side) = line.split("pin: ").nth(1).and_then(|l| l.split(';').next())
        {
            seen.push((id.clone(), side.to_string()));
        }
    }
    seen
}

/// One pin's **own** declarations, whitespace-normalised — what the lowering
/// wrote onto the instance, where it beats the rule the value may have come
/// from. `fmt` wraps a long block, so the reader joins the continuations
/// rather than pinning one line's exact shape.
fn pin_own(out: &str, id: &str) -> String {
    let head = format!("|block#{id}|");
    let mut lines = out
        .lines()
        .skip_while(|l| !(l.contains(&head) && l.contains(".lini-pin.")));
    let mut body = match lines.next().and_then(|l| l.split_once('{')) {
        Some((_, rest)) => rest.to_string(),
        None => return String::new(),
    };
    while !body.contains('}') {
        match lines.next() {
            Some(l) => body.push_str(l),
            None => break,
        }
    }
    body.split('}')
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// `pin_sides` as `id:side` strings, for a compact expectation.
fn sided(out: &str) -> Vec<String> {
    pin_sides(out)
        .into_iter()
        .map(|(i, s)| format!("{i}:{s}"))
        .collect()
}

#[test]
fn a_pose_re_sides_a_components_pins_rigidly() {
    let src = |deg: &str| {
        format!("|component#U7| {{ rotate: {deg} }} [\n  |pin#a|; |pin#b|; |pin#c|; |pin#d|\n]\n")
    };
    // Unposed: the bilateral split — a, b left; c, d right, each rail read
    // top-to-bottom.
    let out = desugar_source(&src("0")).unwrap();
    assert_eq!(
        sided(&out),
        ["a:left", "b:left", "c:right", "d:right"],
        "{out}"
    );
    // A quarter turn clockwise: the left column swings to the top row, the
    // right column to the bottom — and each reads backwards, because the
    // topmost left pin lands rightmost (a rigid turn, not a re-split).
    let out = desugar_source(&src("90")).unwrap();
    assert_eq!(
        sided(&out),
        ["b:top", "a:top", "d:bottom", "c:bottom"],
        "{out}"
    );
    // A half turn swaps every side and reverses every rail.
    let out = desugar_source(&src("180")).unwrap();
    assert_eq!(
        sided(&out),
        ["d:left", "c:left", "b:right", "a:right"],
        "{out}"
    );
    // Three quarters: the left column swings down, keeping its reading order.
    let out = desugar_source(&src("270")).unwrap();
    assert_eq!(
        sided(&out),
        ["c:top", "d:top", "a:bottom", "b:bottom"],
        "{out}"
    );
}

#[test]
fn an_explicitly_sided_pin_rides_the_turn_too() {
    let out =
        desugar_source("|component#U7| { rotate: 90 } [\n  |pin#a| { side: top }\n]\n").unwrap();
    assert_eq!(sided(&out), ["a:right"], "{out}");
}

#[test]
fn a_pose_re_lays_a_symbol_and_its_ports() {
    // The 64×12 resistor stands up: its `d` is turned geometry (no transform)
    // and its ports move by the same map — p1 to the top, p2 to the bottom.
    // The two leading moves are the glyph's own box, which every fragment
    // carries so the linework and any solid detail lay out as one rectangle
    // ([`crate::desugar::schematic`]); they turn with the rest.
    let upright = lowered_path(&desugar_source("|R#r1| \"1k\"\n").unwrap());
    let out = desugar_source("|R#r1| \"1k\" { rotate: 90 }\n").unwrap();
    let turned = lowered_path(&out);
    assert_eq!(extent(&upright), (64.0, 12.0), "the resistor lies down");
    assert_eq!(
        extent(&turned),
        (12.0, 64.0),
        "…and stands up turned: {out}"
    );
    assert_eq!(
        points(&turned).len(),
        points(&upright).len(),
        "the same linework, turned — not redrawn: {out}"
    );
    assert!(
        out.contains("|block#p1| .lini-block { pin: center; translate: 0 -32; }"),
        "{out}"
    );
    assert!(
        out.contains("|block#p2| .lini-block { pin: center; translate: 0 32; }"),
        "{out}"
    );
    // A label's symbol turns the same way — the gnd's connection point (its
    // stem, at the top) swings to the bottom.
    let upright_d = lowered_path(&desugar_source("|gnd#g1|\n").unwrap());
    let upright = points(&upright_d);
    let out = desugar_source("|gnd#g1| { rotate: 180 }\n").unwrap();
    let turned_d = lowered_path(&out);
    let turned = points(&turned_d);
    let (w, h) = extent(&upright_d);
    assert_eq!(
        extent(&turned_d),
        (w, h),
        "a half turn keeps the box: {out}"
    );
    for (x, y) in &upright {
        let mirrored = (w - x, h - y);
        assert!(
            turned
                .iter()
                .any(|p| (p.0 - mirrored.0).abs() < 1e-9 && (p.1 - mirrored.1).abs() < 1e-9),
            "{mirrored:?} missing — the half turn is not a point reflection: {out}"
        );
    }
}

#[test]
fn a_turn_never_reaches_the_paint() {
    // [SPEC 16.1]: rotation is structural, so every text — pin names and
    // numbers, ref, value, net text — stands upright. Nothing in the lowered
    // scene carries a `rotate:` at all.
    let out = desugar_source(
        "|component#U7| \"IC\" { rotate: 90 } [ |pin#a| { number: 1 } ]\n|R#r1| \"1k\" { rotate: 270 }\n|label#n| \"NET\" { rotate: 180 }\n",
    )
    .unwrap();
    assert!(!out.contains("rotate"), "{out}");
    // The pose rides a class instead, so the engine can read it back.
    assert!(out.contains(".lini-pose-90"), "{out}");
    assert!(out.contains(".lini-pose-270"), "{out}");
    assert!(out.contains(".lini-pose-180"), "{out}");
}

#[test]
fn a_pin_behind_a_wrapper_still_counts_as_a_pin() {
    // [SPEC 16.1/16.2]: one pin walk. A `|component|`'s pins are found *through*
    // whatever wraps them, so the pose chooser and the engine agree on a part's
    // arity — two wrapped pins make U1 a two-pin satellite that turns to face
    // its anchor, exactly as two direct ones do.
    let wrapped = desugar_source(
        "{ layout: schematic }\n|component#U0| { cell: 1 1 } [ |pin#x| ]\n|component#U1| [ |row| [ |pin#a|; |pin#b| ] ]\nU0.x - U1.a\n",
    )
    .unwrap();
    assert!(wrapped.contains(".lini-pose-180"), "{wrapped}");
    let direct = desugar_source(
        "{ layout: schematic }\n|component#U0| { cell: 1 1 } [ |pin#x| ]\n|component#U1| [ |pin#a|; |pin#b| ]\nU0.x - U1.a\n",
    )
    .unwrap();
    assert!(direct.contains(".lini-pose-180"), "{direct}");
}

#[test]
fn a_pose_off_the_chain_is_consumed_too() {
    // A define (or element rule) can pose a part; the turn is still structural,
    // and the class rule it came from is neutralized on the instance.
    let out = desugar_source("{ |vert::R| { rotate: 90 } }\n|vert#r1|\n").unwrap();
    assert!(out.contains(".lini-pose-90"), "{out}");
    assert!(out.contains("translate: 0 32"), "the ports turned: {out}");
    assert!(
        out.contains("|block#r1| .lini-vert.lini-R.lini-block.lini-pose-90 { rotate: 0; }"),
        "the class's turn is cancelled on the instance: {out}"
    );
    // The instance restating the rule's turn must cancel it just the same —
    // dropping its own decl would leave the rule standing.
    let out = desugar_source("{ |vert::R| { rotate: 90 } }\n|vert#r1| { rotate: 90 }\n").unwrap();
    assert!(
        out.contains("|block#r1| .lini-vert.lini-R.lini-block.lini-pose-90 { rotate: 0; }"),
        "{out}"
    );
}

#[test]
fn only_a_right_angle_poses_a_part() {
    let err = desugar_source("|R#r1| { rotate: 45 }\n").expect_err("a non-90° pose");
    assert!(
        err.to_string()
            .ends_with("a schematic part rotates in 90° steps — 0, 90, 180, or 270"),
        "{err}"
    );
    // A plain box is not connection-bearing — it turns as paint, as ever.
    let out = desugar_source("|box#b| { rotate: 45 }\n").unwrap();
    assert!(out.contains("rotate: 45"), "{out}");
}

#[test]
fn a_pin_translate_slides_along_its_side() {
    // [SPEC 16.2] the along-side component moves the pin — chrome and all,
    // since the stub / name / number are its children.
    let out = desugar_source("|component#U7| [ |pin#a| { translate: 0 6 } ]\n").unwrap();
    assert!(out.contains("translate: 0 6"), "{out}");
    // Under a turn the slide turns with the pin: down the left edge becomes
    // leftward along the top edge.
    let out =
        desugar_source("|component#U7| { rotate: 90 } [ |pin#a| { translate: 0 6 } ]\n").unwrap();
    assert!(out.contains("translate: -6 0"), "{out}");
    assert_eq!(sided(&out), ["a:top"], "{out}");
}

#[test]
fn a_cross_axis_pin_translate_errors() {
    let err = desugar_source("|component#U7| [ |pin#a| { translate: 4 0 } ]\n")
        .expect_err("a cross-axis slide");
    assert!(
        err.to_string().ends_with(
            "a pin lives on its side — 'translate' slides it along the left edge; drop the x component"
        ),
        "{err}"
    );
    // The axis is read on the side the pin **landed** on.
    let err = desugar_source("|component#U7| { rotate: 90 } [ |pin#a| { translate: 4 0 } ]\n")
        .expect_err("a cross-axis slide");
    assert!(
        err.to_string()
            .contains("along the top edge; drop the y component"),
        "{err}"
    );
}

#[test]
fn a_pins_side_and_slide_read_the_same_cascade_the_pose_does() {
    // [SPEC 16.2] a `side:` / `translate:` off a define is a pin's side and a
    // pin's slide exactly as an authored one is — otherwise the turn misses it
    // and the cross-axis gate never fires.
    let sheet = "{ |sig::pin| { translate: 0 6 } }\n";
    let out = desugar_source(&format!("{sheet}|component#U7| [ |sig#a|; |pin#b| ]\n")).unwrap();
    assert!(
        out.contains("translate: 0 6"),
        "the rule still states it: {out}"
    );
    // Under a turn the slide turns with the pin and lands **on** it, beating
    // the rule it came from.
    let out = desugar_source(&format!(
        "{sheet}|component#U7| {{ rotate: 90 }} [ |sig#a|; |pin#b| ]\n"
    ))
    .unwrap();
    assert!(
        pin_own(&out, "a").contains("translate: -6 0;"),
        "the turned slide lands on the pin itself: {out}"
    );
    // And a cross-axis one errors wherever it was stated, in either frame.
    let cross = "{ |sig::pin| { translate: 4 0 } }\n";
    for src in [
        format!("{cross}|component#U7| [ |sig#a| ]\n"),
        format!("{cross}|component#U7| {{ rotate: 90 }} [ |sig#a| ]\n"),
    ] {
        let err = desugar_source(&src).expect_err("a cross-axis slide");
        assert!(
            err.to_string().contains("a pin lives on its side"),
            "{err} for {src}"
        );
    }
}

#[test]
fn a_re_sided_pin_says_where_it_landed() {
    // The lowered tree is what the engine reads a forced side back off
    // [SPEC 16.7]: a turned pin's `side:` must agree with its rail and stub.
    let out = desugar_source("|component#U7| { rotate: 90 } [ |pin#a| { side: top } ]\n").unwrap();
    assert!(out.contains("side: right"), "{out}");
    assert!(!out.contains("side: top"), "no stale ident: {out}");
    assert_eq!(sided(&out), ["a:right"], "{out}");
    // A side stated by a rule is answered the same way — on the pin, where it
    // beats the rule.
    let out =
        desugar_source("{ |up::pin| { side: top } }\n|component#U7| { rotate: 180 } [ |up#a| ]\n")
            .unwrap();
    assert!(
        pin_own(&out, "a").contains("side: bottom;"),
        "the landed side is written on the pin itself: {out}"
    );
    // Unturned, nothing is rewritten — the pin keeps exactly what it stated.
    let out = desugar_source("|component#U7| [ |pin#a| { side: top } ]\n").unwrap();
    assert!(out.contains("side: top"), "{out}");
}

#[test]
fn a_lowered_chain_reads_derived_first_like_the_authored_one() {
    // The cascade's tiers: a define's own decl beats the base it derives
    // from, whether the reader walks the *authored* chain or a lowered node's
    // classes. The classes are worn most-derived first, so the read must
    // reverse them — un-reversed, a define lost every property to its base.
    let out = desugar_source(
        "{\n  |up::pin| { side: top }\n  |down::up| { side: bottom }\n}\n|component#U7| [ |down#a| ]\n",
    )
    .unwrap();
    assert_eq!(sided(&out), ["a:bottom"], "the derived tier wins: {out}");
    // The same read mints a display ref [SPEC 16.2] — a define's `prefix:`
    // beats the family default its base carries.
    let out =
        desugar_source("{ |amp::opamp| { prefix: \"A\" } }\n|schematic#s| [ |amp| ]\n").unwrap();
    assert!(
        out.contains("[ \"A1\" ]"),
        "the define's prefix mints: {out}"
    );
}

#[test]
fn a_define_carried_layout_is_a_schematic_scope_too() {
    // The pose chooser asks desugar's cascade slice, not the written type, so
    // `{ |sheet::group| { layout: schematic } }` seats and poses its
    // satellites exactly as a `|schematic|` does [SPEC 16.1].
    let body = "[\n  |component#U7| [ |pin#a|; |pin#b|; |pin#c| ]\n  |R#r1|\n  U7.a - r1.p1\n]\n";
    let posed = |scope: &str| {
        desugar_source(&format!(
            "{{ |sheet::group| {{ layout: schematic }} }}\n|{scope}#s| {body}"
        ))
        .unwrap()
        .contains("lini-pose-180")
    };
    assert!(posed("sheet"), "a define carrying `layout: schematic`");
    assert!(posed("schematic"), "and the written type, unchanged");
}

#[test]
fn a_define_body_contributes_satellites_the_pose_chooser_sees() {
    // Hoist-then-pose [SPEC 16.1/19]: a `define` body's children and links
    // reach the scope only when the instance expands, and the gather does that
    // **before** the chooser runs — so a satellite written in a define poses
    // exactly like one written in the sheet.
    let body = "[\n  |component#U7| [ |pin#a| { side: left }; |pin#b|; |pin#c| ]\n  |R#r1|\n  U7.a - r1.p1\n]";
    let from_define = desugar_source(&format!(
        "{{\n  |sheet::group| {{ layout: schematic; }} {body}\n}}\n|sheet#s|\n"
    ))
    .unwrap();
    let written = desugar_source(&format!("|schematic#s| {body}\n")).unwrap();
    assert!(from_define.contains("lini-pose-180"), "{from_define}");
    assert!(written.contains("lini-pose-180"), "{written}");
}

// ── Label wires [SPEC 16.5] ──

/// A one-part schematic sheet carrying `wires` — the shape every label-wire
/// test is written over. `u7` has a pin on three sides, so a tag can be seated
/// anywhere.
fn sheet(wires: &str) -> String {
    format!(
        "{{ layout: schematic; }}\n\
         |component#u7| [\n  |pin#a| {{ side: right }}\n  |pin#b| {{ side: left }}\n  |pin#c| {{ side: bottom }}\n]\n\
         {wires}\n"
    )
}

/// The first minted tag's declaration head and `{ }` style (everything before
/// its `[ ]` body) — the one place a minted `shape:` can show, since the
/// `.lini-label` class def carries the built-in `shape: plain` too.
fn tag_line(out: &str) -> String {
    let at = out.find("|block#lini-label-1|").expect("a minted tag");
    let rest = &out[at..];
    rest[..rest.find('[').unwrap_or(rest.len())].to_string()
}

#[test]
fn a_label_wire_mints_its_tag_and_wires_to_it() {
    // The one-ended statement is read here, before the resolve gates that
    // reject the shape from either side [SPEC 16.5].
    let out = desugar_source(&sheet("u7.a - \"NSTDBY\"")).unwrap();
    assert!(out.contains("u7.a - lini-label-1"), "{out}");
    assert!(
        out.contains("|block#lini-label-1| .lini-net-run.lini-label.lini-block"),
        "the tag carries the net text as its smart label, on a net run: {out}"
    );
    // The text moved onto the tag — the wire keeps no label of its own.
    assert!(!out.contains("[ \"NSTDBY\" ]\n\n"), "{out}");
}

#[test]
fn a_label_wires_end_marker_shapes_the_tag() {
    for (op, shape) in [
        ("->", "right"),
        ("-<", "left"),
        ("-<>", "both"),
        ("-*", "round"),
    ] {
        let out = desugar_source(&sheet(&format!("u7.a {op} \"N\""))).unwrap();
        assert!(
            tag_line(&out).contains(&format!("shape: {shape};")),
            "'{op}' shapes '{shape}': {out}"
        );
        // Consumed off the wire — a tag is drawn, never an arrowhead.
        assert!(out.contains("u7.a - lini-label-1"), "{out}");
    }
    // The bare `-` leaves the tag its default; nothing is written on it.
    let out = desugar_source(&sheet("u7.a - \"N\"")).unwrap();
    assert!(!tag_line(&out).contains("shape:"), "{out}");
}

#[test]
fn a_label_wires_line_part_stays_free() {
    // The op's *line* means what it always means [SPEC 9] — only the marker is
    // the scope's tag sugar.
    let out = desugar_source(&sheet("u7.a -- \"N\"")).unwrap();
    assert!(out.contains("u7.a -- lini-label-1"), "a dashed wire: {out}");
    assert!(!tag_line(&out).contains("shape:"), "{out}");
}

#[test]
fn an_authored_shape_outranks_the_markers() {
    // The marker fills the built-in default; an element rule (or the tag's own
    // style) wins [SPEC 16.5].
    let out = desugar_source(&sheet("u7.a -> \"N\"").replace(
        "layout: schematic;",
        "layout: schematic; |label| { shape: round; }",
    ))
    .unwrap();
    assert!(!tag_line(&out).contains("shape: right"), "{out}");
    assert!(
        out.contains("lini-tag-round"),
        "the rule's shape stands: {out}"
    );
}

#[test]
fn a_label_wires_side_rides_onto_the_tag_it_mints() {
    // A one-ended label wire has no node of its own to carry `side:` — the
    // block is the *link's* tail [SPEC 9] — so the mint moves it onto the tag,
    // the way it already moves the op's marker [SPEC 16.4]. Without this the
    // commonest spelling of the override is silently inert.
    let out = desugar_source(&sheet("u7.a - \"N\" { side: bottom }")).unwrap();
    assert!(
        out.contains("side: bottom"),
        "the side reaches the minted tag: {out}"
    );
    // …and it is *moved*, not copied: the wire keeps no `side:` it cannot read.
    let wire = out
        .lines()
        .find(|l| l.contains("u7.a -"))
        .expect("the label wire");
    assert!(!wire.contains("side"), "the wire keeps none of it: {wire}");
}

#[test]
fn a_marker_shapes_a_tag_however_it_was_written() {
    // "Markers shape labels" [SPEC 16.5] is one law, not three: a net tag can
    // be minted from text, referenced, or declared inline as a capsule, and the
    // end marker shapes it the same way in each. The gather hoists capsules
    // before the mint runs, so all three arrive as one lookup.
    for wires in [
        "u7.a -> \"N\"",
        "|label#n1| \"N\"\nu7.a -> n1",
        "u7.a -> |label#n1|",
    ] {
        let out = desugar_source(&sheet(wires)).unwrap();
        // The selector and its one declaration, checked apart: the rule is long
        // enough that `fmt` may break it over two lines.
        assert!(
            out.contains(".lini-tag-flag-right.lini-label.lini-block")
                && out.contains("shape: right;"),
            "'{wires}' shapes its tag: {out}"
        );
        assert!(
            !out.contains("-> lini-label") && !out.contains("-> n1"),
            "the marker is consumed: {out}"
        );
    }
}

#[test]
fn the_first_statement_to_shape_a_tag_wins() {
    // A tag's shape is settled once [SPEC 16.5]: an authored `shape:` outranks
    // every marker, and among markers the first to land holds — a later hop
    // reads the shape already there and leaves it. Pinned so the rule is a
    // decision, not an accident.
    let out = desugar_source(&sheet("|label#n1| \"N\"\nu7.a -> n1\nu7.b -* n1")).unwrap();
    assert!(out.contains("{ shape: right; }"), "the first marker: {out}");
    assert!(!out.contains("shape: round"), "{out}");
}

#[test]
fn a_marker_still_shapes_in_a_lowered_file() {
    // `lini desugar` emits `.lini-label { shape: plain; … }` — the compiler's
    // own echo of the bundle default, folded back as an element rule on
    // re-desugar. It must not read as an authored choice, or every marker in a
    // lowered file would be silently inert.
    let lowered = desugar_source(&sheet("u7.a -> \"N\"")).unwrap();
    assert!(lowered.contains(".lini-label { shape: plain;"), "{lowered}");
    let grown = desugar_source(&format!("{lowered}u7.b -* \"Q\"\n")).unwrap();
    assert!(
        grown.contains("lini-tag-round"),
        "the marker still bites: {grown}"
    );
    // A rule stating anything *other* than the default is the author's —
    // `an_authored_shape_outranks_the_markers` holds that half.
}

#[test]
fn a_marked_part_to_part_wire_errors() {
    let err = lini::check(&sheet("u7.a -> u7.b")).expect_err("a marked wire");
    assert!(
        err.to_string().contains("a schematic wire is plain"),
        "{err}"
    );
    assert!(err.to_string().contains("write 'a - b'"), "{err}");
    // A start marker shapes nothing either.
    let err = lini::check(&sheet("u7.a <- \"N\"")).expect_err("a start marker");
    assert!(
        err.to_string().contains("a schematic wire is plain"),
        "{err}"
    );
    // A marked statement with no far end at all wanted a label wire — the
    // suggestion points at that, not at a plain part-to-part wire.
    let err = lini::check(&sheet("u7.a ->")).expect_err("a marker with no tag");
    assert!(err.to_string().contains("write 'a -> \"NET\"'"), "{err}");
}

#[test]
fn a_marker_at_a_symbol_form_label_errors() {
    // `- |gnd|` is the symbol form of the same statement — the symbol *is* the
    // drawing, so there is no tag for a marker to shape [SPEC 16.5].
    let err = lini::check(&sheet("u7.a -> |gnd|")).expect_err("a marked symbol tag");
    assert!(
        err.to_string()
            .contains("'|gnd|' draws its symbol — there is no tag to shape"),
        "{err}"
    );
    // …and an authored id is part of that spelling — the message names what the
    // author wrote, not the head of the rewritten path.
    let named = lini::check(&sheet("u7.a -> |gnd#g1|")).expect_err("a marked symbol tag");
    assert!(
        named
            .to_string()
            .contains("'|gnd#g1|' draws its symbol — there is no tag to shape"),
        "{named}"
    );
}

#[test]
fn the_capsule_form_poses_like_the_declared_one() {
    // Hoist-then-pose [SPEC 16.1]: `u7.b - |R#rx|` reaches the chooser as a
    // child with its minted id and a wire rewritten to it, so it turns exactly
    // as the two-statement spelling does.
    let capsule = desugar_source(&sheet("u7.b - |R#rx|")).unwrap();
    let declared = desugar_source(&sheet("|R#rx|\nu7.b - rx")).unwrap();
    let pose = |out: &str| {
        out.lines()
            .find(|l| l.contains(".lini-R."))
            .unwrap_or_default()
            .to_string()
    };
    assert!(pose(&capsule).contains("lini-pose-180"), "{capsule}");
    assert_eq!(
        pose(&capsule).replace("lini-cap-1", "gx"),
        pose(&declared),
        "the two spellings lower alike"
    );
}

#[test]
fn label_wire_lowering_is_a_byte_fixed_point_and_the_mint_skips_taken_names() {
    let once = desugar_source(&sheet("u7.a -> \"N\"\nu7.c -* \"P\"")).unwrap();
    assert_eq!(desugar_source(&once).unwrap(), once, "a fixed point");
    // A lowered sheet gaining a wire mints past the ids already there — the
    // `lini-cap-N` discipline [SPEC 9].
    let grown = desugar_source(&format!("{once}u7.b - \"Q\"\n")).unwrap();
    assert!(grown.contains("u7.b - lini-label-3"), "{grown}");
}

#[test]
fn only_a_schematic_scope_reads_a_one_ended_wire_as_a_label() {
    // Elsewhere the statement means nothing and resolve says so, unchanged.
    let err = lini::check("a -> b\nb - \"N\"\n").expect_err("no scope reads it");
    assert!(err.to_string().contains("at least two endpoints"), "{err}");
    // A sibling scope beside a schematic is untouched — the carrier reaches
    // *inside*, never across.
    let beside = "|schematic#s| [\n  |component#u8| [ |pin#z| ]\n]\n|group#g| [\n  |box#b|\n  b - \"N\"\n]\n";
    let err = lini::check(beside).expect_err("a sibling scope reads nothing");
    assert!(err.to_string().contains("at least two endpoints"), "{err}");
}

#[test]
fn the_scopes_link_laws_reach_a_nested_ordinary_container() {
    // [SPEC 16] **the carrier**: placement never cascades, but the scope's
    // *reading of a statement* does. A `|group|` inside a schematic places its
    // own children and still mints the label wire written in it — the boundary
    // Task 5.1 pinned as an error, flipped here deliberately.
    let nested =
        "{ layout: schematic; }\n|group#g| [\n  |component#u8| [ |pin#z| ]\n  u8.z - \"NET\"\n]\n";
    let out = desugar_source(nested).unwrap();
    assert!(
        out.contains("|block#lini-label-1| .lini-net-run.lini-label.lini-block"),
        "the nested group mints its tag: {out}"
    );
    assert!(out.contains("u8.z - lini-label-1"), "{out}");
    assert!(lini::check(nested).is_ok(), "and the sheet compiles");
    // The same statement one container deeper, and inside an anonymous one.
    for body in [
        "|group#g| [\n  |row#r| [\n    |component#u8| [ |pin#z| ]\n    u8.z - \"NET\"\n  ]\n]\n",
        "|group| [\n  |component#u8| [ |pin#z| ]\n  u8.z - \"NET\"\n]\n",
    ] {
        let out = desugar_source(&format!("{{ layout: schematic; }}\n{body}")).unwrap();
        assert!(out.contains("u8.z - lini-label-1"), "{body}: {out}");
    }
}

#[test]
fn a_nested_container_poses_nothing_though_its_wires_are_the_scopes() {
    // The other half of the split [SPEC 16]: a pose is **placement**, so the
    // chooser answers off the immediate container. The same two statements
    // written directly in the sheet turn the ground to face the pin; written
    // inside a `|row|` — which runs its own engine and seats nothing — they
    // must not.
    let sheet = |body: &str| format!("{{ layout: schematic }}\n{body}");
    let direct = desugar_source(&sheet(
        "|component#u1| [ |pin#a|; |pin#b|; |pin#c| ]\n|R#r1|\nu1.a - r1.p1\n",
    ))
    .unwrap();
    assert!(
        direct.contains("lini-pose-"),
        "the sheet poses (the turn is consumed into its class): {direct}"
    );
    let nested = desugar_source(&sheet(
        "|row#r| [\n  |component#u1| [ |pin#a|; |pin#b|; |pin#c| ]\n  |R#r1|\n  u1.a - r1.p1\n]\n",
    ))
    .unwrap();
    assert!(
        !nested.contains("lini-pose-"),
        "a nested row seats nothing, so it turns nothing: {nested}"
    );
}

#[test]
fn a_schematic_scope_never_invents_a_box() {
    // [SPEC 16.5/21] auto-create is off inside the scope, and the message
    // names the net label the bare id most likely meant.
    let err = lini::check("{ layout: schematic }\n|component#u7| [ |pin#a| ]\nu7.a - NSTDBY\n")
        .expect_err("no invented box");
    assert_eq!(
        err.message,
        "'NSTDBY' is unknown — a schematic never invents a box; \
         did you mean '- \"NSTDBY\"' (a net label)?"
    );
    // The carrier again: a nested ordinary container refuses too.
    let nested = lini::check(
        "|schematic#s| [\n  |group#g| [\n    |component#u7| [ |pin#a| ]\n    u7.a - NSTDBY\n  ]\n]\n",
    )
    .expect_err("the law reaches the nested group");
    assert!(
        nested.to_string().contains("never invents a box"),
        "{nested}"
    );
    // The suggested spelling is the one that works.
    assert!(
        lini::check("{ layout: schematic }\n|component#u7| [ |pin#a| ]\nu7.a - \"NSTDBY\"\n")
            .is_ok()
    );
}

#[test]
fn a_minted_ref_is_not_an_endpoint() {
    // [SPEC 16.2/21] an anonymous part's display ref is drawn, never an id, so
    // wiring one is an unknown endpoint — and says why, rather than reading as
    // a stray net name.
    let err = lini::check("{ layout: schematic }\n|R| \"1k\"\n|C| \"100n\"\nR1 - C1\n")
        .expect_err("a minted ref is display-only");
    assert_eq!(
        err.message,
        "link endpoint 'R1' not found — a minted ref is display-only; \
         give the part an id to wire it"
    );
    // Giving the part an id is the fix the message names.
    lini::check("{ layout: schematic }\n|R#r1| \"1k\"\n|C#c1| \"100n\"\nr1 - c1\n")
        .expect("an id is wirable");
}

#[test]
fn a_mindmap_inside_a_sheet_seals_the_carrier_by_its_type() {
    // [SPEC 8/16] A `|mindmap|` declares no `layout:` of its own — the tree
    // seat stamps `layout: tree` on its scope *after* this body lowers — so
    // it is the one engine `seals_schematic_scope` has to name by type.
    // Without that clause the sheet's reading reaches into the mindmap's body
    // and its cross-link's unknown id becomes the invent refusal instead of an
    // ordinary auto-created topic.
    let out = desugar_source(
        "|schematic#s| [\n  |mindmap#m| [\n    |topic#a| [ |topic#b| ]\n    a - c\n  ]\n]\n",
    )
    .expect("the mindmap reads its own body");
    assert!(
        out.contains("|block#c| .lini-box.lini-block [ \"c\" ]"),
        "the cross-link's endpoint auto-created inside the mindmap: {out}"
    );
}

#[test]
fn a_define_bodys_links_stay_out_of_the_hosts_own_slice_however_they_land() {
    // Auto-create reads the host body's **own** statements — a define's ids
    // are the define's affair — and the boundary between the two used to be an
    // index taken before the schematic landing step, which cuts a chain into
    // hops and moves it. So an unrelated statement decided the diagnostic: the
    // `- r1 -` row leaked the next define link into the host's slice and made
    // its unknown `x` the sheet's invent refusal, while the `- g1` row left it
    // for resolve. Same semantics, two answers; now one.
    let body = |wire: &str| {
        format!(
            "{{\n  layout: schematic;\n  |blk::group| {{ layout: schematic; }} [\n    \
             |component#u1| [ |pin#a| ]\n    |R#r1|\n    |gnd#g1|\n    {wire}\n    x - g1\n  ]\n}}\n\
             |blk#c1|\n"
        )
    };
    for wire in ["u1.a - r1 - g1", "u1.a - g1"] {
        let err = lini::check(&body(wire)).expect_err("the define's own unknown id");
        assert_eq!(
            err.message, "link endpoint 'x' not found in 'c1'",
            "the cut moved the boundary: {wire}"
        );
    }
}

#[test]
fn a_lowered_nested_sheet_is_still_a_sheet_to_the_carrier() {
    // A **nested** scope states itself as a class after lowering
    // (`|block#s| .lini-schematic`), never as its type — so a carrier reading
    // only the written chain would answer `false` on `lini desugar`'s own
    // output, where the layout gate answers `true`. Two stages, one law: a
    // statement hand-added to a lowered sheet reads exactly as it did in the
    // source.
    let lowered = desugar_source(
        "|schematic#s| [\n  |component#u7| [ |pin#a|\n |pin#b| ]\n  u7.a - \"N\"\n]\n",
    )
    .unwrap();
    let grown = lowered.replace(
        "  u7.a - lini-label-1",
        "  u7.a - lini-label-1\n  u7.b - \"Q\"",
    );
    let out = desugar_source(&grown).unwrap();
    assert!(
        out.contains("u7.b - lini-label-2"),
        "the lowered sheet still mints: {out}"
    );
}

#[test]
fn auto_create_is_untouched_outside_a_schematic_scope() {
    // [SPEC 3] every other scope still invents its box — flow, grid, tree,
    // sequence, and a plain container beside a schematic.
    for src in [
        "a -> b\n",
        "{ layout: grid; cols: 2 }\na -> b\n",
        "{ layout: tree }\n|topic#t| \"T\"\n",
        "{ layout: sequence }\na -> b \"hi\"\n",
        "|schematic#s| [\n  |component#u7| [ |pin#a| ]\n]\n|group#g| [\n  x -> y\n]\n",
    ] {
        lini::check(src).unwrap_or_else(|e| panic!("{src}: {e}"));
    }
    let out = desugar_source("|group#g| [\n  x -> y\n]\n").unwrap();
    assert!(out.contains("|block#x| .lini-box.lini-block"), "{out}");
}

/// A sheet holding `body`, with an anchor so the scope has something to seat.
fn sheet_holding(body: &str) -> String {
    format!("{{ layout: schematic }}\n|component#u1| [\n  |pin#a|; |pin#b|; |pin#c|\n]\n{body}")
}

#[test]
fn another_engine_nested_in_a_sheet_seals_the_carrier() {
    // [SPEC 16] the carrier reaches a container that reads no statement of its
    // own (`|row|`, `|group|`, anonymous) and **stops** at one that reads its
    // own — otherwise the sheet silently rewrites another engine's statements.
    // The axis the flow/grid/tree root sweep above cannot test: here every
    // case is genuinely *inside* a schematic scope.

    // A nested drawing's leader stays a leader [SPEC 15.7] — it must not mint
    // a net tag, and its own direction law must still be the one that speaks.
    let drawing = "|drawing#d| [\n  |rect#r1| { width: 40; height: 20 }\n  r1 <- \"a note\"\n]\n";
    let out = desugar_source(&sheet_holding(drawing)).unwrap();
    assert!(
        !out.contains("lini-label-"),
        "a leader is not a label wire: {out}"
    );
    assert!(out.contains("[ \"a note\" ]"), "it stays the leader's text");
    lini::check(&sheet_holding(drawing)).expect("a drawing nested in a sheet compiles");
    let wrong = sheet_holding(&drawing.replace("r1 <- ", "r1 -> "));
    let err = lini::check(&wrong).expect_err("the drawing's own direction law speaks");
    assert!(err.to_string().contains("a leader points back"), "{err}");

    // A nested sequence still declares its participants: the marker gate never
    // sees `->`, and no-auto-create never sees `x`.
    let seq = "|sequence#q| [\n  x -> y \"hi\"\n]\n";
    let out = desugar_source(&sheet_holding(seq)).unwrap();
    assert!(out.contains("|block#x| .lini-box.lini-block"), "{out}");
    assert!(out.contains("|block#y| .lini-box.lini-block"), "{out}");
    lini::check(&sheet_holding(seq)).expect("a sequence nested in a sheet compiles");

    // A nested tree still builds its branches from its topics.
    let tree =
        "|box#t| { layout: tree } [\n  |topic#root| \"R\" [\n    |topic#kid| \"K\"\n  ]\n]\n";
    lini::check(&sheet_holding(tree)).expect("a tree nested in a sheet compiles");

    // …and the seal is not one grain too wide: a flow wrapper still reaches.
    let row = "|row#r| [\n  |gnd#g1|\n  u1.c - g1\n  u1.b - \"NET\"\n]\n";
    let out = desugar_source(&sheet_holding(row)).unwrap();
    assert!(
        out.contains("u1.b - lini-label-1"),
        "the row still mints: {out}"
    );
}

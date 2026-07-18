use super::*;
use crate::resolve::{MarkerKind, NodeKind};

fn rv4(src: &str) -> Program {
    let toks = crate::lexer::lex(src).expect("lex");
    let file = crate::syntax::parser::parse(src, &toks).expect("parse");
    let lowered = crate::desugar::desugar(&file).expect("desugar");
    resolve(&lowered, &[]).expect("resolve")
}

fn rv4_err(src: &str) -> String {
    let toks = crate::lexer::lex(src).expect("lex");
    let file = crate::syntax::parser::parse(src, &toks).expect("parse");
    // The error may surface in desugar (unknown type, cycle) or in resolve.
    let result = crate::desugar::desugar(&file).and_then(|f| resolve(&f, &[]));
    match result {
        Err(e) => e.message,
        Ok(_) => panic!("expected an error resolving {src:?}"),
    }
}

fn num(p: &Program, node: usize, attr: &str) -> Option<f64> {
    p.scene.nodes[node].attrs.number(attr)
}
fn ident<'a>(p: &'a Program, node: usize, attr: &str) -> Option<&'a str> {
    match p.scene.nodes[node].attrs.get(attr) {
        Some(ResolvedValue::Ident(s)) => Some(s.as_str()),
        _ => None,
    }
}

#[test]
fn bare_node_resolves() {
    let p = rv4("|box#x|\n");
    assert_eq!(p.scene.nodes.len(), 1);
    assert_eq!(p.scene.nodes[0].id.as_deref(), Some("x"));
    assert_eq!(p.scene.nodes[0].kind, NodeKind::Block);
}

#[test]
fn dumb_core_has_no_hidden_defaults() {
    // Resolve `|block#x|` WITHOUT desugaring (input that bypassed the lowering):
    // a bare primitive with no `.lini-*` class carries no radius/padding/gap. The
    // defaults live only in the `.lini-*` classes desugar injects.
    let src = "|block#x|\n";
    let toks = crate::lexer::lex(src).expect("lex");
    let file = crate::syntax::parser::parse(src, &toks).expect("parse");
    let p = resolve(&file, &[]).expect("resolve");
    let attrs = &p.scene.nodes[0].attrs;
    assert!(
        attrs.get("radius").is_none(),
        "no default radius in the core"
    );
    assert!(
        attrs.get("padding").is_none(),
        "no default padding in the core"
    );
    assert!(attrs.get("gap").is_none(), "no default gap in the core");
}

#[test]
fn element_rule_reaches_the_node() {
    let p = rv4("{ |box| { radius: 4; } }\n|box#x|\n");
    assert_eq!(num(&p, 0, "radius"), Some(4.0));
}

#[test]
fn descendant_rule_matches_a_nested_node() {
    let p = rv4("{ |group| |box| { fill: gray; } }\n|group#g| [\n  |box#a|\n]\n");
    // `a` is a box inside the group; the descendant rule paints it.
    let a = &p.scene.nodes[0].children[0];
    assert!(matches!(a.attrs.get("fill"), Some(ResolvedValue::Ident(s)) if s == "gray"));
}

#[test]
fn id_rule_targets_one_node() {
    // [SPEC 4]: `#hero { }` paints only the node with that id, and the instance
    // block still beats it.
    let p = rv4("{ #hero { fill: gold; } }\n|box#hero|\n|box#other|\n");
    assert_eq!(ident(&p, 0, "fill"), Some("gold"));
    assert_eq!(ident(&p, 1, "fill"), None);
}

#[test]
fn instance_block_beats_id_rule() {
    let p = rv4("{ #hero { fill: gold; } }\n|box#hero| { fill: red }\n");
    assert_eq!(ident(&p, 0, "fill"), Some("red"));
}

#[test]
fn class_rule_applies() {
    let p = rv4("{ .hot { stroke: red; } }\n|box#x| .hot\n");
    assert_eq!(ident(&p, 0, "stroke"), Some("red"));
    assert_eq!(p.scene.nodes[0].applied_styles, vec!["hot"]);
}

#[test]
fn instance_block_beats_element_rule() {
    let p = rv4("{ |box| { fill: white; } }\n|box#x| { fill: red; }\n");
    assert_eq!(ident(&p, 0, "fill"), Some("red"));
}

#[test]
fn label_becomes_a_centred_text_child() {
    // [SPEC 3]: a box's smart label lowers to a centred text child.
    let p = rv4("|box#cat| \"cat\"\n");
    let label = &p.scene.nodes[0].children[0];
    assert_eq!(label.kind, NodeKind::Text);
    assert_eq!(label.label.as_deref(), Some("cat"));
}

#[test]
fn an_empty_label_draws_nothing() {
    // [SPEC 3]: `""` is an empty string — nothing in flow.
    let p = rv4("|box#cat| \"\"\n");
    assert!(p.scene.nodes[0].children.is_empty());
}

#[test]
fn caption_is_a_small_text_plain_title() {
    // [SPEC 8]: a caption is a `|block|`-based title, pinned to the top edge
    // with a smaller font (`mount` is gone entirely).
    let p = rv4("|group#g| [\n  |caption| \"Title\"\n]\n");
    let cap = &p.scene.nodes[0].children[0];
    assert!(cap.type_chain.iter().any(|t| t == "caption"));
    assert!(matches!(
        cap.attrs.get("pin"),
        Some(ResolvedValue::Tuple(_))
    ));
    assert!(cap.attrs.get("mount").is_none());
    assert!(matches!(cap.attrs.get("font-size"), Some(ResolvedValue::Number(n)) if *n == 12.0));
    assert_eq!(cap.children[0].label.as_deref(), Some("Title"));
}

#[test]
fn group_label_lowers_to_a_caption() {
    // [SPEC 3/8]: a group's smart label is its caption.
    let p = rv4("|group#k| \"Kitchen\"\n");
    let cap = &p.scene.nodes[0].children[0];
    assert!(cap.type_chain.iter().any(|t| t == "caption"));
    assert_eq!(cap.children[0].label.as_deref(), Some("Kitchen"));
}

#[cfg(feature = "icons")]
#[test]
fn icon_named_by_symbol_with_optional_text() {
    // [SPEC 7]: `symbol` names the icon; a bare string in `[ ]` is an ordinary
    // centred-text **child** (so `translate` / styling reach it like any node's
    // text), not folded onto the node.
    let p = rv4("|icon#i| { symbol: house } [ \"3\" ]\n");
    assert_eq!(p.scene.nodes[0].kind, NodeKind::Icon);
    assert_eq!(ident(&p, 0, "symbol"), Some("house"));
    assert_eq!(p.scene.nodes[0].label, None);
    let child = &p.scene.nodes[0].children[0];
    assert_eq!(child.kind, NodeKind::Text);
    assert_eq!(child.label.as_deref(), Some("3"));
}

#[cfg(feature = "icons")]
#[test]
fn icon_label_sets_the_symbol() {
    // [SPEC 7]: the smart label of an icon is its symbol.
    let p = rv4("|icon#i| \"house\"\n");
    assert_eq!(ident(&p, 0, "symbol"), Some("house"));
}

#[cfg(feature = "icons")]
#[test]
fn icon_symbol_set_twice_errors() {
    assert!(
        rv4_err("|icon#i| \"house\" { symbol: heart }\n")
            .contains("symbol is its label or 'symbol:', not both")
    );
}

#[test]
fn text_properties_inherit_to_descendants() {
    let p = rv4("|group#g| { font-size: 10 } [\n  \"hi\"\n]\n");
    let t = &p.scene.nodes[0].children[0];
    assert_eq!(t.kind, NodeKind::Text);
    assert_eq!(t.attrs.number("font-size"), Some(10.0));
}

#[test]
fn define_body_materializes_per_instance() {
    let p = rv4("{ |room::group| [\n  |box#inlet|\n] }\n|room#r|\n");
    let inlet = &p.scene.nodes[0].children[0];
    assert_eq!(inlet.id.as_deref(), Some("inlet"));
}

#[test]
fn root_link_auto_creates_undeclared_endpoints() {
    let p = rv4("cat -> dog\n");
    let ids: Vec<&str> = p
        .scene
        .nodes
        .iter()
        .filter_map(|n| n.id.as_deref())
        .collect();
    assert!(ids.contains(&"cat") && ids.contains(&"dog"));
    assert_eq!(p.links.len(), 1);
}

#[test]
fn link_selector_styles_every_link() {
    // [SPEC 9]: `|-| { stroke; stroke-width }` styles every link's wire — the
    // ordinary node vocabulary, scoped by the selector, no `link-*` family.
    let p = rv4("{ |-| { stroke: red; stroke-width: 3 } }\na -> b\n");
    assert!(matches!(p.links[0].attrs.get("stroke"), Some(ResolvedValue::Ident(s)) if s == "red"));
    assert_eq!(p.links[0].attrs.number("stroke-width"), Some(3.0));
}

#[test]
fn scoped_link_rule_overrides_the_root_one() {
    // [SPEC 4]: a descendant `#g |-|` styles the links written in `g`'s body; a
    // root-scope link keeps the bare `|-|` value. Root links resolve before
    // lifted (body) links, so [0] is `a -> g` and [1] is the internal `x -> y`.
    let p = rv4(
        "{ |-| { stroke: --gray }\n#g |-| { stroke: --red-ink } }\n|box#a|\n|group#g| [\n  |box#x|\n  |box#y|\n  x -> y\n]\na -> g\n",
    );
    let stroke_var = |i: usize| match p.links[i].attrs.get("stroke") {
        Some(ResolvedValue::LiveVar { name, .. }) => name.clone(),
        other => panic!("expected a var stroke, got {other:?}"),
    };
    assert_eq!(stroke_var(0), "gray");
    assert_eq!(stroke_var(1), "red-ink");
}

#[test]
fn clearance_cascades_from_a_container_block() {
    // [SPEC 9]: `clearance` / `routing` stay scene config — set on a container's
    // own block, they cascade to that scope's links, nearest winning.
    let p = rv4(
        "{ clearance: 8 }\n|box#a|\n|group#g| { clearance: 20 } [\n  |box#x|\n  |box#y|\n  x -> y\n]\na -> g\n",
    );
    assert_eq!(p.links[0].attrs.number("clearance"), Some(8.0)); // a -> g (root)
    assert_eq!(p.links[1].attrs.number("clearance"), Some(20.0)); // x -> y (in g)
}

#[test]
fn removed_routing_is_rejected() {
    // `curved` was replaced by `natural`, not aliased — SPEC 20's exact row.
    assert_eq!(
        rv4_err("{ routing: curved }\na -> b\n"),
        "routing takes orthogonal, natural, or straight — 'curved' was replaced by 'natural'"
    );
    rv4("{ routing: orthogonal }\na -> b\n"); // the built modes are accepted
    let p = rv4("{ routing: natural }\na -> b\n");
    assert_eq!(p.links[0].routing, crate::resolve::Strategy::Natural);
    let p = rv4("{ routing: straight }\na -> b\n");
    assert_eq!(p.links[0].routing, crate::resolve::Strategy::Straight);
}

#[test]
fn operator_sets_markers_and_line_style() {
    let p = rv4("|box#a|\n|box#b|\na --> b\n");
    let w = &p.links[0];
    assert_eq!(w.markers.end, MarkerKind::Arrow);
    assert!(matches!(w.attrs.get("stroke-style"), Some(ResolvedValue::Ident(s)) if s == "dashed"));
}

#[test]
fn fan_expands_to_one_link_per_pair() {
    let p = rv4("|box#a|\n|box#b|\n|box#c|\na & b -> c\n");
    assert_eq!(p.links.len(), 2);
}

#[test]
fn a_sequence_frame_is_scope_transparent() {
    // [SPEC 13]: a message inside a frame resolves against the sequence's participants,
    // not the frame body — it hoists to the scene scope and auto-creates nothing local.
    let p = rv4("{ layout: sequence }\n|box#api|\n|cyl#db|\napi -> db\n|alt| [\n  db --> api\n]\n");
    // Both messages live at scene scope; the frame opened none.
    assert_eq!(p.links.len(), 2);
    assert!(
        p.links.iter().all(|w| w.scope.is_empty()),
        "frame message hoisted to scene scope"
    );
    // The frame-body return wires the outer db → api.
    let ret = &p.links[1];
    assert_eq!(ret.endpoints[0].path, "db");
    assert_eq!(ret.endpoints[1].path, "api");
    // No phantom frame-local participants: the alt holds no boxes.
    let alt = p
        .scene
        .nodes
        .iter()
        .find(|n| n.type_chain.iter().any(|t| t == "alt"))
        .expect("the alt frame");
    assert!(
        alt.children.iter().all(|c| c.id.is_none()),
        "no phantom boxes inside the frame"
    );
}

#[test]
fn internal_link_resolves_with_scoped_paths() {
    let p =
        rv4("{ |room::group| [\n  |box#inlet|\n  |box#outlet|\n  inlet -> outlet\n] }\n|room#r|\n");
    let w = &p.links[0];
    assert_eq!(w.endpoints[0].path, "r.inlet");
    assert_eq!(w.endpoints[1].path, "r.outlet");
}

// ── Errors [SPEC 20] ──

#[test]
fn unknown_type_errors() {
    assert!(rv4_err("|ghost#x|\n").contains("unknown type 'ghost'"));
}

#[test]
fn unknown_class_errors() {
    assert!(rv4_err("|box#x| .nope\n").contains("unknown class '.nope'"));
}

#[test]
fn duplicate_id_errors() {
    assert!(rv4_err("|box#a|\n|oval#a|\n").contains("duplicate id 'a'"));
}

#[test]
fn side_names_are_free_ids() {
    // [SPEC 22]: sides are keywords only after an endpoint `:`, so a node may be
    // named `|box#top|` — no longer a reserved-id error.
    let p = rv4("|box#top|\n");
    assert_eq!(p.scene.nodes[0].id.as_deref(), Some("top"));
}

#[test]
fn body_link_endpoint_not_found_suggests() {
    let e = rv4_err("|group#g| [\n  |box#x|\n  g.y -> x\n]\n");
    assert!(e.contains("not found"), "got: {e}");
}

#[test]
fn a_copy_index_leaks_no_ids_and_needs_a_drawing() {
    // `bolt.2` without its carrier path stays an unknown endpoint
    // [SPEC 15.4] — copies leak no ids into the scope.
    let e = rv4_err(
        "{ layout: drawing }\n|rect#plate| { width: 120; height: 60 } [\n  |hole#bolt| { width: 10; translate: -35 0; pattern: grid(2, 2, 70, 30) }\n]\nplate:left (-) bolt.2\n",
    );
    assert!(e.contains("endpoint 'bolt.2' not found"), "got: {e}");
    // The numeric segment is drawing grammar [SPEC 21].
    let e = rv4_err("|box#a|\n|box#b|\na.2 -> b\n");
    assert_eq!(
        e,
        "a numeric path segment picks a pattern copy — it belongs in a 'layout: drawing'"
    );
}

#[test]
fn a_duplicate_datum_letter_errors_per_drawing_scope() {
    // Letters are identities, collected per drawing scope [SPEC 15.7/20].
    let geometry = "|rect#a| { width: 40; height: 20 }\n|rect#b| { width: 40; height: 20 }\n";
    let e = rv4_err(&format!(
        "{{ layout: drawing }}\n{geometry}a:bottom >- \"A\"\nb:bottom >- \"A\"\n"
    ));
    assert_eq!(e, "datum 'A' is already placed");
    // Sibling drawings each carry their own alphabet.
    let one = "|drawing#v| [ |rect#a| { width: 40; height: 20 }\n  a:bottom >- \"A\" ]\n";
    let two = "|drawing#w| [ |rect#a| { width: 40; height: 20 }\n  a:bottom >- \"A\" ]\n";
    rv4(&format!("{one}{two}"));
}

#[test]
fn a_one_ended_fan_stays_one_link_and_measures_never_fan() {
    // `&` on a one-ended leader keeps one link — one text, every endpoint
    // [SPEC 15.7]; on a measure or mate it errors [SPEC 20].
    let geometry = "|rect#a| { width: 40; height: 20 }\n|rect#b| { width: 40; height: 20 }\n";
    let p = rv4(&format!(
        "{{ layout: drawing }}\n{geometry}a & b <- \"2× R5\"\n"
    ));
    let fan: Vec<_> = p.links.iter().filter(|w| w.one_ended).collect();
    assert_eq!(fan.len(), 1);
    assert_eq!(fan[0].endpoints.len(), 2);
    assert_eq!(fan[0].texts.len(), 1);
    let misuse = "'&' fans one-ended leaders — chain dimensions instead ('a (-) b (-) c')";
    let e = rv4_err(&format!(
        "{{ layout: drawing }}\n{geometry}|rect#c| {{ width: 40; height: 20 }}\na & b (-) c\n"
    ));
    assert_eq!(e, misuse);
    let e = rv4_err(&format!(
        "{{ layout: drawing }}\n{geometry}|rect#c| {{ width: 40; height: 20 }}\na:right & b:right || c:left\n"
    ));
    assert_eq!(e, misuse);
}

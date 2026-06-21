//! The resolve orchestrator: variables → stylesheet → scene tree → wires →
//! render inputs, assembled into a [`Program`] (SPEC §17). Types, templates,
//! defines, labels, and auto-create are lowered upstream by `desugar`, so resolve
//! only ever sees primitives and `.lini-*` classes.
//!
//! [`lib.rs`]'s compile pipeline enters resolution here (after `desugar`).

use super::cascade::Stylesheet;
use super::defaults;
use super::ir::{AttrMap, Program, ResolvedScene, ResolvedValue, SheetInputs, VarTable};
use super::merge::collapse;
use super::scene::{self, PathIndex, SceneCtx};
use super::value::resolve_groups;
use super::wires;
use crate::error::Error;
use crate::syntax::ast::{Decl, File, Rule, SelPart, StyleItem};
use std::collections::HashMap;

/// Resolve a parsed file into a [`Program`].
pub fn resolve(file: &File, theme: &[(String, String)]) -> Result<Program, Error> {
    // ── Variables: built-in visual-var defaults ← theme ← `--name` decls ──
    let mut vars = defaults::built_in_defaults();
    apply_theme(&mut vars, theme);
    apply_var_decls(&mut vars, file)?;

    // ── Stylesheet: the desugared file's rules (generated `.lini-*` type classes,
    //    the `-> { }` wire defaults, descendant + user-class rules) ──
    let rules: Vec<&Rule> = file
        .stylesheet
        .iter()
        .filter_map(|it| match it {
            StyleItem::Rule(r) => Some(r),
            _ => None,
        })
        .collect();
    let sheet = Stylesheet::build(&rules, &vars)?;

    // ── Root configuration + the text props it cascades ──
    let root_attrs = root_attrs(file, &vars)?;
    let mut root_text_ctx = AttrMap::new();
    for name in scene::INHERITED_TEXT {
        if let Some(v) = root_attrs.get(name) {
            root_text_ctx.insert(*name, v.clone());
        }
    }

    // ── Scene tree (types/templates/defines, labels, and auto-create were all
    //    lowered by desugar — resolve only sees primitives + classes) ──
    let ctx = SceneCtx {
        sheet: &sheet,
        vars: &vars,
    };
    let mut id_seen = HashMap::new();
    let mut lifted = Vec::new();
    let nodes = scene::resolve_instances(
        &file.instances,
        &ctx,
        &root_attrs,
        &root_text_ctx,
        &mut id_seen,
        &mut lifted,
    )?;
    let index = PathIndex::build(&nodes);

    // ── Wires: root statements then lifted internal wires ──
    let wire_defaults = wire_rule_decls(file, &vars)?;
    let mut wire_list = Vec::new();
    for w in &file.wires {
        wire_list.extend(wires::resolve_wire(w, &ctx, &index, &[], &wire_defaults)?);
    }
    for lw in &lifted {
        wire_list.extend(wires::resolve_wire(
            &lw.wire,
            &ctx,
            &index,
            &lw.prefix,
            &wire_defaults,
        )?);
    }

    let sheet_inputs = build_sheet_inputs(file, &vars, &root_attrs)?;

    Ok(Program {
        vars,
        scene: ResolvedScene {
            attrs: root_attrs,
            nodes,
        },
        wires: wire_list,
        sheet: sheet_inputs,
    })
}

// ─────────────────────────── Variables ───────────────────────────

fn apply_theme(vars: &mut VarTable, theme: &[(String, String)]) {
    for (name, raw) in theme {
        vars.set(name.clone(), parse_theme_value(raw));
    }
}

/// Parse a `--theme` value: a number, a `#hex`, a bare ident, else raw CSS
/// (a font stack stays verbatim, never quote-wrapped).
fn parse_theme_value(raw: &str) -> ResolvedValue {
    let s = raw.trim();
    if let Ok(n) = s.parse::<f64>() {
        return ResolvedValue::Number(n);
    }
    if let Some(hex) = s.strip_prefix('#')
        && matches!(hex.len(), 3 | 6 | 8)
        && hex.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return ResolvedValue::Hex(hex.to_string());
    }
    if !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return ResolvedValue::Ident(s.to_string());
    }
    ResolvedValue::RawCss(s.to_string())
}

/// Apply `--name: value` declarations in source order (each sees the prior).
/// All vars are visual (SPEC §11.2); a built-in `--lini-*` name keeps its
/// meaning, a new name is the author's.
fn apply_var_decls(vars: &mut VarTable, file: &File) -> Result<(), Error> {
    for item in &file.stylesheet {
        if let StyleItem::Var(d) = item {
            let value = resolve_groups(&d.groups, d.span, vars)?;
            vars.set(d.name.clone(), value);
        }
    }
    Ok(())
}

// ─────────────────────────── Root config ───────────────────────────

/// Root container attributes — read straight from the global block. Desugar
/// injects the scene defaults (`layout: column`, `padding: 20` — the scene's
/// frame — `gap`, the inherited-text baseline), so there is nothing to seed here.
fn root_attrs(file: &File, vars: &VarTable) -> Result<AttrMap, Error> {
    let mut ordered: Vec<(String, ResolvedValue)> = Vec::new();
    for item in &file.stylesheet {
        if let StyleItem::RootDecl(d) = item {
            ordered.push((d.name.clone(), resolve_groups(&d.groups, d.span, vars)?));
        }
    }
    Ok(collapse(&ordered))
}

// ─────────────────────────── Render inputs ───────────────────────────

/// The `-> { }` wire defaults as ordered decls (lowest specificity for the wire
/// cascade) — the wire glyph is the reserved `wire` element selector.
fn wire_rule_decls(file: &File, vars: &VarTable) -> Result<Vec<(String, ResolvedValue)>, Error> {
    for item in &file.stylesheet {
        if let StyleItem::Rule(r) = item
            && let [SelPart::Type(t)] = r.selector.parts.as_slice()
            && t == "wire"
        {
            let mut out = Vec::with_capacity(r.decls.len());
            for d in &r.decls {
                out.push((d.name.clone(), resolve_groups(&d.groups, d.span, vars)?));
            }
            return Ok(out);
        }
    }
    Ok(Vec::new())
}

/// The renderer's [`SheetInputs`]: every single-class rule's attrs (the generated
/// `.lini-*` type classes and the user `.style` classes, in source order), the
/// wire defaults, and the root inherited-text font size. Descendant rules
/// (`|.lini-table .lini-box| { }`) bake inline via the cascade and carry no entry.
fn build_sheet_inputs(
    file: &File,
    vars: &VarTable,
    root_attrs: &AttrMap,
) -> Result<SheetInputs, Error> {
    let mut class_rules = Vec::new();
    let mut wire_defaults = AttrMap::new();
    for item in &file.stylesheet {
        if let StyleItem::Rule(r) = item {
            match r.selector.parts.as_slice() {
                [SelPart::Class(c)] => {
                    class_rules.push((c.clone(), decls_attrmap(&r.decls, vars)?))
                }
                [SelPart::Type(t)] if t == "wire" => wire_defaults = decls_attrmap(&r.decls, vars)?,
                _ => {}
            }
        }
    }
    let root_font_size = root_attrs.number("font-size").unwrap_or(15.0);
    // Live-CSS text styling the global block sets — rides the `.lini` rule so it
    // applies scene-wide (SPEC §10). No default: present only when authored.
    let mut root_text = AttrMap::new();
    for name in ["font-style", "text-transform"] {
        if let Some(v) = root_attrs.get(name) {
            root_text.insert(name, v.clone());
        }
    }
    Ok(SheetInputs {
        class_rules,
        wire_defaults,
        root_font_size,
        root_text,
    })
}

fn decls_attrmap(decls: &[Decl], vars: &VarTable) -> Result<AttrMap, Error> {
    let mut ordered = Vec::with_capacity(decls.len());
    for d in decls {
        ordered.push((d.name.clone(), resolve_groups(&d.groups, d.span, vars)?));
    }
    Ok(collapse(&ordered))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::{MarkerKind, ShapeKind};

    fn rv4(src: &str) -> Program {
        let toks = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(&toks).expect("parse");
        let lowered = crate::desugar::desugar(&file).expect("desugar");
        resolve(&lowered, &[]).expect("resolve")
    }

    fn rv4_err(src: &str) -> String {
        let toks = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(&toks).expect("parse");
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
        let p = rv4("x |box|\n");
        assert_eq!(p.scene.nodes.len(), 1);
        assert_eq!(p.scene.nodes[0].id.as_deref(), Some("x"));
        assert_eq!(p.scene.nodes[0].shape, ShapeKind::Box);
    }

    #[test]
    fn dumb_core_has_no_hidden_defaults() {
        // Resolve `x |box|` WITHOUT desugaring (input that bypassed the lowering):
        // a primitive with no `.lini-box` class carries no radius/padding/gap. The
        // defaults live only in the `.lini-*` classes desugar injects.
        let toks = crate::lexer::lex("x |box|\n").expect("lex");
        let file = crate::syntax::parser::parse(&toks).expect("parse");
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
        let p = rv4("{ |box| { radius: 4; } }\nx |box|\n");
        assert_eq!(num(&p, 0, "radius"), Some(4.0));
    }

    #[test]
    fn descendant_rule_matches_a_nested_node() {
        let p = rv4("{ |group box| { fill: gray; } }\ng |group| [\n  a |box|\n]\n");
        // `a` is a box inside the group; the descendant rule paints it.
        let a = &p.scene.nodes[0].children[0];
        assert!(matches!(a.attrs.get("fill"), Some(ResolvedValue::Ident(s)) if s == "gray"));
    }

    #[test]
    fn class_rule_applies() {
        let p = rv4("{ .hot { stroke: red; } }\nx |box| .hot\n");
        assert_eq!(ident(&p, 0, "stroke"), Some("red"));
        assert_eq!(p.scene.nodes[0].applied_styles, vec!["hot"]);
    }

    #[test]
    fn instance_block_beats_element_rule() {
        let p = rv4("{ |box| { fill: white; } }\nx |box| { fill: red; }\n");
        assert_eq!(ident(&p, 0, "fill"), Some("red"));
    }

    #[test]
    fn id_becomes_a_centred_label() {
        // SPEC §3: a leaf box with no block text shows its id as a text child.
        let p = rv4("cat |box|\n");
        let label = &p.scene.nodes[0].children[0];
        assert_eq!(label.shape, ShapeKind::Text);
        assert_eq!(label.label.as_deref(), Some("cat"));
    }

    #[test]
    fn an_empty_label_suppresses_the_id() {
        // SPEC §3: `""` is content, so it overrides id-as-label with nothing.
        let p = rv4("cat |box| \"\"\n");
        assert!(p.scene.nodes[0].children.is_empty());
    }

    #[test]
    fn caption_is_a_small_text_plain_title() {
        // SPEC §8: a caption is a `|plain|`-based title, pinned to the top edge
        // with a smaller font (`mount` is gone entirely).
        let p = rv4("g |group| [\n  |caption| \"Title\"\n]\n");
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
    fn icon_label_is_the_glyph_name_not_a_child() {
        // SPEC §7: an icon's text is its glyph name, carried on the node — never a
        // rendered text child.
        let p = rv4("i |icon| \"home\"\n");
        assert_eq!(p.scene.nodes[0].shape, ShapeKind::Icon);
        assert_eq!(p.scene.nodes[0].label.as_deref(), Some("home"));
        assert!(p.scene.nodes[0].children.is_empty());
    }

    #[test]
    fn text_properties_inherit_to_descendants() {
        let p = rv4("g |group| { font-size: 10 } [\n  \"hi\"\n]\n");
        let t = &p.scene.nodes[0].children[0];
        assert_eq!(t.shape, ShapeKind::Text);
        assert_eq!(t.attrs.number("font-size"), Some(10.0));
    }

    #[test]
    fn define_body_materializes_per_instance() {
        let p = rv4("{ |room::group| [\n  inlet |box|\n] }\nr |room|\n");
        let inlet = &p.scene.nodes[0].children[0];
        assert_eq!(inlet.id.as_deref(), Some("inlet"));
    }

    #[test]
    fn root_wire_auto_creates_undeclared_endpoints() {
        let p = rv4("cat -> dog\n");
        let ids: Vec<&str> = p
            .scene
            .nodes
            .iter()
            .filter_map(|n| n.id.as_deref())
            .collect();
        assert!(ids.contains(&"cat") && ids.contains(&"dog"));
        assert_eq!(p.wires.len(), 1);
    }

    #[test]
    fn wire_rule_sets_routing_defaults() {
        // SPEC §9: `-> { }` is the routing layer's element selector (the wire
        // glyph), carrying the reserved `wire` element rule internally.
        let p = rv4("{ -> { stroke: red; stroke-width: 2; } }\na -> b\n");
        assert!(
            matches!(p.wires[0].attrs.get("stroke"), Some(ResolvedValue::Ident(s)) if s == "red")
        );
        assert_eq!(p.wires[0].attrs.number("stroke-width"), Some(2.0));
    }

    #[test]
    fn operator_sets_markers_and_line_style() {
        let p = rv4("a |box|\nb |box|\na --> b\n");
        let w = &p.wires[0];
        assert_eq!(w.markers.end, MarkerKind::Arrow);
        assert!(
            matches!(w.attrs.get("stroke-style"), Some(ResolvedValue::Ident(s)) if s == "dashed")
        );
    }

    #[test]
    fn fan_expands_to_one_wire_per_pair() {
        let p = rv4("a |box|\nb |box|\nc |box|\na & b -> c\n");
        assert_eq!(p.wires.len(), 2);
    }

    #[test]
    fn internal_wire_resolves_with_scoped_paths() {
        let p = rv4(
            "{ |room::group| [\n  inlet |box|\n  outlet |box|\n  inlet -> outlet\n] }\nr |room|\n",
        );
        let w = &p.wires[0];
        assert_eq!(w.endpoints[0].path, "r.inlet");
        assert_eq!(w.endpoints[1].path, "r.outlet");
    }

    // ── Errors (SPEC §15) ──

    #[test]
    fn unknown_type_errors() {
        assert!(rv4_err("x |ghost|\n").contains("unknown type 'ghost'"));
    }

    #[test]
    fn unknown_class_errors() {
        assert!(rv4_err("x |box| .nope\n").contains("unknown class '.nope'"));
    }

    #[test]
    fn duplicate_id_errors() {
        assert!(rv4_err("a |box|\na |oval|\n").contains("duplicate id 'a'"));
    }

    #[test]
    fn reserved_id_errors_with_capitalized_hint() {
        // `top` is a side — reserved as an id.
        assert!(rv4_err("top |box|\n").contains("'Top' is free"));
    }

    #[test]
    fn body_wire_endpoint_not_found_suggests() {
        let e = rv4_err("g |group| [\n  x |box|\n  g.y -> x\n]\n");
        assert!(e.contains("not found"), "got: {e}");
    }
}

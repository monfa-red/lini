//! The v4 resolve orchestrator: variables → stylesheet → types → scene tree →
//! auto-create → wires → render inputs, assembled into a [`Program`] (SPEC §17).
//!
//! [`lib.rs`]'s compile pipeline enters resolution here.

use super::cascade::Stylesheet;
use super::ir::{
    AttrMap, Program, ResolvedScene, ResolvedValue, SheetInputs, VarKind, VarTable,
};
use super::merge::collapse;
use super::scene::{self, PathIndex, SceneCtx};
use super::types::{self, Types};
use super::value::resolve_groups;
use super::wires;
use super::defaults;
use crate::error::Error;
use crate::span::Span;
use crate::syntax::ast::{Decl, Define, File, Node, Rule, SelPart, Selector, StyleItem, Value};
use std::collections::{HashMap, HashSet};

/// Resolve a parsed v4 file into a [`Program`].
pub fn resolve(file: &File, theme: &[(String, String)]) -> Result<Program, Error> {
    // ── Variables: built-in defaults ← theme ← `--name` declarations ──
    let mut vars = defaults::built_in_defaults();
    apply_theme(&mut vars, theme);
    apply_var_decls(&mut vars, file)?;

    // ── Stylesheet: built-in rules then the file's rules ──
    let builtins = builtin_rules();
    let mut rule_refs: Vec<&Rule> = builtins.iter().collect();
    for item in &file.stylesheet {
        if let StyleItem::Rule(r) = item {
            rule_refs.push(r);
        }
    }
    let sheet = Stylesheet::build(&rule_refs, &vars)?;

    // ── Types: user defines over templates/primitives ──
    let defines: Vec<&Define> = file
        .stylesheet
        .iter()
        .filter_map(|it| match it {
            StyleItem::Define(d) => Some(d),
            _ => None,
        })
        .collect();
    let types = Types::build(&defines, &sheet, &vars)?;

    // Selector type parts must name known types (SPEC §17 step 1).
    for t in sheet.referenced_types() {
        if !types.is_known(t) {
            return Err(Error::at(Span::empty(), format!("unknown type '{}' in selector", t)));
        }
    }

    // ── Root configuration + the text props it cascades ──
    let root_attrs = root_attrs(file, &vars)?;
    let mut root_text_ctx = AttrMap::new();
    for name in scene::INHERITED_TEXT {
        if let Some(v) = root_attrs.get(name) {
            root_text_ctx.insert(*name, v.clone());
        }
    }

    // ── Scene tree ──
    let ctx = SceneCtx {
        types: &types,
        sheet: &sheet,
        vars: &vars,
    };
    let mut id_seen = HashMap::new();
    let mut lifted = Vec::new();
    let mut nodes =
        scene::resolve_instances(&file.instances, &ctx, &root_text_ctx, &mut id_seen, &mut lifted)?;

    // ── Auto-create: a root wire's single-segment id absent everywhere becomes
    // an empty |rect| at the scene root (SPEC §3). ──
    let mut index = PathIndex::build(&nodes);
    let auto = auto_created(&index, file);
    if !auto.is_empty() {
        let mut ancestors = Vec::new();
        for node in &auto {
            nodes.push(scene::resolve_node(
                node,
                &ctx,
                &mut ancestors,
                &[],
                &root_text_ctx,
                &mut id_seen,
                &mut Vec::new(),
            )?);
        }
        index = PathIndex::build(&nodes);
    }

    // ── Wires: root statements then lifted internal wires ──
    let wire_defaults = sheet.element_decls("wire");
    let mut wire_list = Vec::new();
    for w in &file.wires {
        wire_list.extend(wires::resolve_wire(w, &ctx, &index, &[], &wire_defaults)?);
    }
    for lw in &lifted {
        wire_list.extend(wires::resolve_wire(&lw.wire, &ctx, &index, &lw.prefix, &wire_defaults)?);
    }

    let sheet_inputs = build_sheet_inputs(file, &defines, &vars)?;

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
        let value = parse_theme_value(raw);
        let kind = vars.get(name).map_or(VarKind::Visual, |e| e.kind);
        vars.set(name.clone(), kind, value);
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
    if !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
        return ResolvedValue::Ident(s.to_string());
    }
    ResolvedValue::RawCss(s.to_string())
}

/// Apply `--name: value` declarations in source order (each sees the prior). A
/// built-in keeps its kind; a new name is Visual.
fn apply_var_decls(vars: &mut VarTable, file: &File) -> Result<(), Error> {
    for item in &file.stylesheet {
        if let StyleItem::Var(d) = item {
            let value = resolve_groups(&d.groups, d.span, vars)?;
            let kind = vars.get(&d.name).map_or(VarKind::Visual, |e| e.kind);
            vars.set(d.name.clone(), kind, value);
        }
    }
    Ok(())
}

// ─────────────────────────── Root config ───────────────────────────

/// Root container attributes: the defaults (`layout: column`, `padding: 0` —
/// the root's margin is the fixed canvas-pad, not padding) overlaid by the
/// file's bare top-level declarations.
fn root_attrs(file: &File, vars: &VarTable) -> Result<AttrMap, Error> {
    let mut ordered: Vec<(String, ResolvedValue)> = vec![
        ("layout".into(), ResolvedValue::Ident("column".into())),
        ("padding".into(), ResolvedValue::Number(0.0)),
    ];
    for item in &file.stylesheet {
        if let StyleItem::RootDecl(d) = item {
            ordered.push((d.name.clone(), resolve_groups(&d.groups, d.span, vars)?));
        }
    }
    Ok(collapse(&ordered))
}

// ─────────────────────────── Auto-create ───────────────────────────

fn auto_created(index: &PathIndex, file: &File) -> Vec<Node> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for w in &file.wires {
        for group in &w.chain {
            for ep in &group.endpoints {
                if ep.path.len() != 1 {
                    continue; // multi-segment paths navigate, never create
                }
                let id = &ep.path[0];
                if index.has_final_segment(id) || seen.contains(id) {
                    continue;
                }
                seen.insert(id.clone());
                out.push(auto_rect(id, ep.span));
            }
        }
    }
    out
}

fn auto_rect(id: &str, span: Span) -> Node {
    Node {
        id: Some(id.to_string()),
        ty: Some("rect".to_string()),
        labels: vec![id.to_string()],
        classes: Vec::new(),
        block: None,
        span,
    }
}

// ─────────────────────────── Built-in rules ───────────────────────────

/// The rules that ship with the language. Today: `table rect { … }` makes table
/// cells borderless, padded, and track-filling (SPEC §8).
fn builtin_rules() -> Vec<Rule> {
    let sp = Span::empty();
    let decl = |name: &str, groups: Vec<Vec<Value>>| Decl {
        name: name.into(),
        groups,
        span: sp,
    };
    vec![Rule {
        selector: Selector {
            parts: vec![SelPart::Type("table".into()), SelPart::Type("rect".into())],
            span: sp,
        },
        decls: vec![
            decl("stroke-width", vec![vec![Value::Number(0.0)]]),
            decl("padding", vec![vec![Value::Number(4.0), Value::Number(8.0)]]),
            decl("align", vec![vec![Value::Ident("stretch".into())]]),
            decl("justify", vec![vec![Value::Ident("stretch".into())]]),
        ],
        span: sp,
    }]
}

// ─────────────────────────── Render inputs ───────────────────────────

/// The renderer's [`SheetInputs`]: the stylesheet sorted into the layers it
/// restates as CSS class rules (SPEC §13). Single-part selectors map directly;
/// descendant rules (`table rect { }`) bake inline via the cascade and carry no
/// entry here.
fn build_sheet_inputs(file: &File, defines: &[&Define], vars: &VarTable) -> Result<SheetInputs, Error> {
    let mut class_rules = Vec::new();
    let mut element_rules = Vec::new();
    let mut wire_defaults = AttrMap::new();
    for item in &file.stylesheet {
        if let StyleItem::Rule(r) = item {
            match r.selector.parts.as_slice() {
                [SelPart::Class(c)] => class_rules.push((c.clone(), decls_attrmap(&r.decls, vars)?)),
                [SelPart::Type(t)] if t == "wire" => wire_defaults = decls_attrmap(&r.decls, vars)?,
                [SelPart::Type(t)] => element_rules.push((t.clone(), decls_attrmap(&r.decls, vars)?)),
                _ => {}
            }
        }
    }
    let mut defines_out = Vec::new();
    for d in defines {
        defines_out.push((d.name.clone(), decls_attrmap(&d.body.decls, vars)?));
    }
    let templates = types::TEMPLATES
        .iter()
        .map(|(n, _)| (n.to_string(), collapse(&types::template_attrs(n))))
        .filter(|(_, a)| !a.map.is_empty())
        .collect();
    Ok(SheetInputs {
        class_rules,
        element_rules,
        defines: defines_out,
        templates,
        wire_defaults,
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
        resolve(&file, &[]).expect("resolve")
    }

    fn rv4_err(src: &str) -> String {
        let toks = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(&toks).expect("parse");
        match resolve(&file, &[]) {
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
        let p = rv4("x |rect|\n");
        assert_eq!(p.scene.nodes.len(), 1);
        assert_eq!(p.scene.nodes[0].id.as_deref(), Some("x"));
        assert_eq!(p.scene.nodes[0].shape, ShapeKind::Rect);
    }

    #[test]
    fn element_rule_reaches_the_node() {
        let p = rv4("rect { radius: 4; }\nx |rect|\n");
        assert_eq!(num(&p, 0, "radius"), Some(4.0));
    }

    #[test]
    fn descendant_rule_matches_a_nested_node() {
        let p = rv4("table rect { fill: gray; }\nt |table| {\n  \"A\"\n}\n");
        // The cell `"A"` is a rect inside the table; the descendant rule paints it.
        let cell = &p.scene.nodes[0].children[0];
        assert!(matches!(cell.attrs.get("fill"), Some(ResolvedValue::Ident(s)) if s == "gray"));
    }

    #[test]
    fn class_rule_applies() {
        let p = rv4(".hot { stroke: red; }\nx |rect| .hot\n");
        assert_eq!(ident(&p, 0, "stroke"), Some("red"));
        assert_eq!(p.scene.nodes[0].applied_styles, vec!["hot"]);
    }

    #[test]
    fn instance_block_beats_element_rule() {
        let p = rv4("rect { fill: white; }\nx |rect| { fill: red; }\n");
        assert_eq!(ident(&p, 0, "fill"), Some("red"));
    }

    #[test]
    fn group_caption_sugar_makes_a_caption_child() {
        let p = rv4("g |group| \"Title\"\n");
        let cap = &p.scene.nodes[0].children[0];
        assert_eq!(cap.shape, ShapeKind::Text);
        assert!(cap.type_chain.iter().any(|t| t == "caption"));
        assert_eq!(cap.label.as_deref(), Some("Title"));
        assert!(matches!(cap.attrs.get("mount"), Some(ResolvedValue::Ident(s)) if s == "in"));
    }

    #[test]
    fn group_second_label_is_a_bottom_footer() {
        let p = rv4("g |group| \"Head\" \"Foot\"\n");
        let foot = &p.scene.nodes[0].children[1];
        assert_eq!(foot.label.as_deref(), Some("Foot"));
        assert!(matches!(foot.attrs.get("side"), Some(ResolvedValue::Ident(s)) if s == "bottom"));
    }

    #[test]
    fn icon_label_is_the_glyph_name_not_a_child() {
        // SPEC §7: an icon's positional label is its glyph name, carried on the
        // node — never expanded into a stacked |text| child.
        let p = rv4("i |icon| \"home\"\n");
        assert_eq!(p.scene.nodes[0].shape, ShapeKind::Icon);
        assert_eq!(p.scene.nodes[0].label.as_deref(), Some("home"));
        assert!(p.scene.nodes[0].children.is_empty());
    }

    #[test]
    fn text_properties_inherit_to_descendants() {
        let p = rv4("g |group| {\n  font-size: 10;\n  t |text| \"hi\"\n}\n");
        let t = &p.scene.nodes[0].children[0];
        assert_eq!(t.shape, ShapeKind::Text);
        assert_eq!(t.attrs.number("font-size"), Some(10.0));
    }

    #[test]
    fn define_body_materializes_per_instance() {
        let p = rv4("room::group {\n  inlet |rect| \"In\"\n}\nr |room|\n");
        let inlet = &p.scene.nodes[0].children[0];
        assert_eq!(inlet.id.as_deref(), Some("inlet"));
    }

    #[test]
    fn root_wire_auto_creates_undeclared_endpoints() {
        let p = rv4("cat -> dog\n");
        let ids: Vec<&str> = p.scene.nodes.iter().filter_map(|n| n.id.as_deref()).collect();
        assert!(ids.contains(&"cat") && ids.contains(&"dog"));
        assert_eq!(p.wires.len(), 1);
    }

    #[test]
    fn operator_sets_markers_and_line_style() {
        let p = rv4("a |rect|\nb |rect|\na --> b\n");
        let w = &p.wires[0];
        assert_eq!(w.markers.end, MarkerKind::Arrow);
        assert!(matches!(w.attrs.get("stroke-style"), Some(ResolvedValue::Ident(s)) if s == "dashed"));
    }

    #[test]
    fn fan_expands_to_one_wire_per_pair() {
        let p = rv4("a |rect|\nb |rect|\nc |rect|\na & b -> c\n");
        assert_eq!(p.wires.len(), 2);
    }

    #[test]
    fn internal_wire_resolves_with_scoped_paths() {
        let p = rv4("room::group {\n  inlet |rect|\n  outlet |rect|\n  inlet -> outlet\n}\nr |room|\n");
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
        assert!(rv4_err("x |rect| .nope\n").contains("unknown class '.nope'"));
    }

    #[test]
    fn duplicate_id_errors() {
        assert!(rv4_err("a |rect|\na |oval|\n").contains("duplicate id 'a'"));
    }

    #[test]
    fn reserved_id_errors_with_capitalized_hint() {
        // `top` is a side — reserved as an id.
        assert!(rv4_err("top |rect|\n").contains("'Top' is free"));
    }

    #[test]
    fn body_wire_endpoint_not_found_suggests() {
        let e = rv4_err("g |group| {\n  x |rect|\n  g.y -> x\n}\n");
        assert!(e.contains("not found"), "got: {e}");
    }
}

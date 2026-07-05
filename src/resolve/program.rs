//! The resolve orchestrator: variables → stylesheet → scene tree → links →
//! render inputs, assembled into a [`Program`] [SPEC 18]. Types, templates,
//! defines, labels, and auto-create are lowered upstream by `desugar`, so resolve
//! only ever sees primitives and `.lini-*` classes.
//!
//! [`lib.rs`]'s compile pipeline enters resolution here (after `desugar`).

use super::cascade::{NodeFacts, Stylesheet};
use super::defaults;
use super::ir::{
    AttrMap, Program, ResolvedCall, ResolvedInst, ResolvedScene, ResolvedValue, SheetInputs,
    VarTable,
};
use super::links;
use super::merge::collapse;
use super::scene::{self, PathIndex, SceneCtx};
use super::value::{resolve_groups, resolve_property};
use crate::error::Error;
use crate::expr::{Expr, FuncTable};
use crate::syntax::ast::{Decl, File, Rule, SelUnit, StyleItem};
use std::collections::{HashMap, HashSet};

/// Resolve a parsed file into a [`Program`].
pub fn resolve(file: &File, theme: &[(String, String)]) -> Result<Program, Error> {
    // ── Variables: built-in visual-var defaults ← theme ← `--name` decls ──
    let mut vars = defaults::built_in_defaults();
    apply_theme(&mut vars, theme);

    // ── Functions: parse each `name(params) `body`` and reject cycles [SPEC 10.7];
    //    every numeric value folds against this table. ──
    let funcs = build_funcs(file)?;
    apply_var_decls(&mut vars, file, &funcs)?;

    // ── Stylesheet: the desugared file's rules (generated `.lini-*` type classes,
    //    descendant + user-class rules) ──
    let rules: Vec<&Rule> = file
        .stylesheet
        .iter()
        .filter_map(|it| match it {
            StyleItem::Rule(r) => Some(r),
            _ => None,
        })
        .collect();
    let sheet = Stylesheet::build(&rules, &vars, &funcs)?;

    // ── Root configuration + the text props it cascades ──
    let root_attrs = root_attrs(file, &vars, &funcs)?;
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
        funcs: &funcs,
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

    // ── Links: root statements then lifted internal links. A link resolves through
    //    the node cascade [SPEC 9, 4]: `resolve_link` layers the `|-|` rules over
    //    a base of the baked defaults plus the scope's `clearance` / `routing`, and
    //    matches descendant rules against the scope's container chain. ──
    let baked = baked_link_defaults(&vars, &funcs)?;
    let mut link_list = Vec::new();
    for w in &file.links {
        let (base, ancestors) = link_scope(&baked, &nodes, &root_attrs, &[]);
        link_list.extend(links::resolve_link(
            w,
            &ctx,
            &index,
            &[],
            &ancestors,
            &base,
        )?);
    }
    for lw in &lifted {
        let (base, ancestors) = link_scope(&baked, &nodes, &root_attrs, &lw.prefix);
        link_list.extend(links::resolve_link(
            &lw.link, &ctx, &index, &lw.prefix, &ancestors, &base,
        )?);
    }

    let sheet_inputs = build_sheet_inputs(file, &vars, &funcs, &root_attrs, &baked, &sheet)?;

    Ok(Program {
        vars,
        scene: ResolvedScene {
            attrs: root_attrs,
            nodes,
        },
        links: link_list,
        sheet: sheet_inputs,
        // Carried to the layout phase for deferred `fn:` sampling [SPEC 14.3];
        // every borrow of it above (scene ctx, sheet inputs) has ended by here.
        funcs,
    })
}

// ─────────────────────────── Variables ───────────────────────────

fn apply_theme(vars: &mut VarTable, theme: &[(String, String)]) {
    for (name, raw) in theme {
        vars.set(name.clone(), parse_theme_value(raw));
    }
}

/// Parse a `--theme` value: a `light-dark()` / `rgba()` / `var()` call, a number,
/// a `#hex`, a bare ident, else raw CSS (a font stack stays verbatim).
fn parse_theme_value(raw: &str) -> ResolvedValue {
    let s = raw.trim();
    // Function form: NAME( ARGS ) — light-dark(), rgb/rgba/hsl/hsla(), var().
    if let Some(open) = s.find('(')
        && s.ends_with(')')
        && is_func_name(&s[..open])
    {
        let name = &s[..open];
        let inner = &s[open + 1..s.len() - 1];
        if name == "var" {
            let v = inner.trim();
            if let Some(rest) = v.strip_prefix("--lini-") {
                return ResolvedValue::LiveVar {
                    name: rest.to_string(),
                    raw: false,
                };
            }
            if let Some(rest) = v.strip_prefix("--") {
                return ResolvedValue::LiveVar {
                    name: rest.to_string(),
                    raw: true,
                };
            }
            return ResolvedValue::RawCss(s.to_string());
        }
        let args = split_top_commas(inner)
            .iter()
            .map(|a| parse_theme_value(a))
            .collect();
        return ResolvedValue::Call(ResolvedCall {
            name: name.to_string(),
            args,
        });
    }
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

/// A CSS function name: letters/digits/`-`, starting with a letter (so a value
/// like `translate(…)` is a call, but a `#hex` or font stack is not).
fn is_func_name(s: &str) -> bool {
    s.bytes().next().is_some_and(|b| b.is_ascii_alphabetic())
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Split on top-level commas (ignoring commas inside nested parens), for the
/// arguments of a `light-dark()` / `rgba()` value.
fn split_top_commas(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

/// Apply `--name: value` declarations in source order (each sees the prior).
/// All vars are visual [SPEC 10.2]; a built-in `--lini-*` name keeps its
/// meaning, a new name is the author's.
fn apply_var_decls(vars: &mut VarTable, file: &File, funcs: &FuncTable) -> Result<(), Error> {
    for item in &file.stylesheet {
        if let StyleItem::Var(d) = item {
            let value = resolve_groups(&d.groups, d.span, vars, funcs)?;
            vars.set(d.name.clone(), value);
        }
    }
    Ok(())
}

/// Parse the stylesheet's `funcdef`s into a [`FuncTable`] and reject reference
/// cycles [SPEC 10.7]. Arity and unknown-name errors surface at fold time.
fn build_funcs(file: &File) -> Result<FuncTable, Error> {
    let mut parsed = Vec::new();
    for item in &file.stylesheet {
        if let StyleItem::Func(f) = item {
            let body = Expr::parse(&f.body).map_err(|e| Error::at(f.span, e.0))?;
            parsed.push((f, body));
        }
    }
    let names: HashSet<&str> = parsed.iter().map(|(f, _)| f.name.as_str()).collect();
    // Edges to other user functions only (math builtins / params are not nodes).
    let graph: HashMap<&str, Vec<String>> = parsed
        .iter()
        .map(|(f, body)| {
            let refs = body
                .referenced_names()
                .into_iter()
                .filter(|n| names.contains(n.as_str()))
                .collect();
            (f.name.as_str(), refs)
        })
        .collect();
    for (f, _) in &parsed {
        detect_cycle(&f.name, &graph, &mut Vec::new())?;
    }

    let mut funcs = FuncTable::new();
    for (f, body) in parsed {
        funcs.insert(f.name.clone(), f.params.clone(), body);
    }
    Ok(funcs)
}

/// Depth-first cycle check over the function reference graph.
fn detect_cycle(
    name: &str,
    graph: &HashMap<&str, Vec<String>>,
    stack: &mut Vec<String>,
) -> Result<(), Error> {
    if stack.iter().any(|n| n == name) {
        stack.push(name.to_string());
        return Err(Error::at(
            crate::span::Span::empty(),
            format!("cycle in '{}'", stack.join(" → ")),
        ));
    }
    stack.push(name.to_string());
    if let Some(refs) = graph.get(name) {
        for r in refs {
            detect_cycle(r, graph, stack)?;
        }
    }
    stack.pop();
    Ok(())
}

// ─────────────────────────── Root config ───────────────────────────

/// Root container attributes — read straight from the global block. Desugar
/// injects the scene defaults (`layout: flow`, `padding: 20` — the scene's
/// frame — `gap`, the inherited-text baseline), so there is nothing to seed here.
fn root_attrs(file: &File, vars: &VarTable, funcs: &FuncTable) -> Result<AttrMap, Error> {
    let mut ordered: Vec<(String, ResolvedValue)> = Vec::new();
    for item in &file.stylesheet {
        if let StyleItem::RootDecl(d) = item {
            ordered.push((
                d.name.clone(),
                resolve_property(&d.name, &d.groups, d.span, vars, funcs)?,
            ));
        }
    }
    Ok(collapse(&ordered))
}

// ─────────────────────────── Render inputs ───────────────────────────

/// The baked link base [SPEC 10.5] — a link's lowest-specificity layer, below
/// the scope cascade, class rules, and its own block. The values live in the one
/// tuning home (`desugar::bundles`).
fn baked_link_defaults(
    vars: &VarTable,
    funcs: &FuncTable,
) -> Result<Vec<(String, ResolvedValue)>, Error> {
    let mut out = Vec::new();
    for d in crate::desugar::bundles::link_defaults() {
        out.push((
            d.name.clone(),
            resolve_groups(&d.groups, d.span, vars, funcs)?,
        ));
    }
    Ok(out)
}

/// The scene-config properties a link takes from its scope [SPEC 9]: geometry, not
/// paint, so they live on a container's own block and cascade nearest-wins — unlike
/// the wire and label look, which come from `|-|` rules. `clearance` is respected
/// between links *and* nodes; `routing` pairs with `layout`.
const SCOPE_LINK_PROPS: &[&str] = &["clearance", "routing"];

/// The container chain from the scene root down to `scope` (each segment an id),
/// stopping at the first missing segment. The root is not a node, so it is absent —
/// [`link_scope`] folds it in for the config cascade, and a bare `|-|` matches every
/// link with no ancestor needed.
fn scope_chain<'a>(nodes: &'a [ResolvedInst], scope: &[String]) -> Vec<&'a ResolvedInst> {
    let mut out = Vec::new();
    let mut cur = nodes;
    for seg in scope {
        match cur.iter().find(|n| n.id.as_deref() == Some(seg)) {
            Some(n) => {
                out.push(n);
                cur = &n.children;
            }
            None => break,
        }
    }
    out
}

/// The selector identity of a resolved container [SPEC 4]: its worn `.lini-*` type
/// classes (the type chain plus its primitive) and user classes, and its id — what a
/// descendant `|table| |-|` matches against.
fn inst_facts(inst: &ResolvedInst) -> NodeFacts {
    let mut classes: Vec<String> = inst
        .type_chain
        .iter()
        .map(|t| format!("lini-{t}"))
        .collect();
    classes.push(format!("lini-{}", inst.kind.as_str()));
    classes.extend(inst.applied_styles.iter().cloned());
    NodeFacts {
        classes,
        id: inst.id.clone(),
    }
}

/// A link's scope inputs: its `base` layer — the baked defaults plus the nearest
/// scope's [`SCOPE_LINK_PROPS`] (root → container chain, nearest winning) — and the
/// `ancestors` its descendant `|…| |-|` rules match against. A root-scope link
/// passes `scope: &[]`.
fn link_scope(
    baked: &[(String, ResolvedValue)],
    nodes: &[ResolvedInst],
    root_attrs: &AttrMap,
    scope: &[String],
) -> (Vec<(String, ResolvedValue)>, Vec<NodeFacts>) {
    let chain = scope_chain(nodes, scope);
    let mut base = baked.to_vec();
    for prop in SCOPE_LINK_PROPS {
        let nearest = chain
            .iter()
            .rev()
            .find_map(|n| n.attrs.get(prop))
            .or_else(|| root_attrs.get(prop));
        if let Some(v) = nearest {
            base.push((prop.to_string(), v.clone()));
        }
    }
    let ancestors = chain.iter().map(|n| inst_facts(n)).collect();
    (base, ancestors)
}

/// The renderer's [`SheetInputs`]: every single-class rule's attrs (the generated
/// `.lini-*` type classes and the user `.style` classes, in source order), the
/// link defaults, and the root inherited-text font size. Descendant rules
/// (`|.lini-table .lini-box| { }`) bake inline via the cascade and carry no entry.
fn build_sheet_inputs(
    file: &File,
    vars: &VarTable,
    funcs: &FuncTable,
    root_attrs: &AttrMap,
    baked: &[(String, ResolvedValue)],
    sheet: &Stylesheet,
) -> Result<SheetInputs, Error> {
    let mut class_rules = Vec::new();
    for item in &file.stylesheet {
        if let StyleItem::Rule(r) = item
            && let [SelUnit::Class(c)] = r.selector.units.as_slice()
        {
            class_rules.push((c.clone(), decls_attrmap(&r.decls, vars, funcs)?));
        }
    }
    // The `.lini-link` rule's defaults: a root-scope link — the baked base plus the
    // scope config, then the root `|-|` element rule [SPEC 9, 16]. Its paint states
    // the `.lini-link` CSS rule; a link that differs inlines the difference.
    let (base, _) = link_scope(baked, &[], root_attrs, &[]);
    let mut link_defaults = base;
    link_defaults.extend(sheet.class_decls(links::LINK_CLASS));
    let link_defaults = collapse(&link_defaults);
    let root_font_size = root_attrs.number("font-size").unwrap_or(15.0);
    // Inherited-text props the global block set, for the `.lini` rule [SPEC 6].
    // `font-family` / `font-weight` / `color` override their themeable var when set
    // globally; the rest are live CSS with no default, present only when authored.
    let mut root_text = AttrMap::new();
    for name in [
        "font-family",
        "font-weight",
        "color",
        "font-style",
        "text-transform",
        "text-decoration",
        "text-shadow",
    ] {
        if let Some(v) = root_attrs.get(name) {
            root_text.insert(name, v.clone());
        }
    }
    Ok(SheetInputs {
        class_rules,
        link_defaults,
        root_font_size,
        root_text,
    })
}

fn decls_attrmap(decls: &[Decl], vars: &VarTable, funcs: &FuncTable) -> Result<AttrMap, Error> {
    let mut ordered = Vec::with_capacity(decls.len());
    for d in decls {
        ordered.push((
            d.name.clone(),
            resolve_property(&d.name, &d.groups, d.span, vars, funcs)?,
        ));
    }
    Ok(collapse(&ordered))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::{MarkerKind, NodeKind};

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
        let toks = crate::lexer::lex("|block#x|\n").expect("lex");
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
        assert!(
            matches!(p.links[0].attrs.get("stroke"), Some(ResolvedValue::Ident(s)) if s == "red")
        );
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
    fn deferred_routing_is_rejected() {
        assert!(rv4_err("{ routing: curved }\na -> b\n").contains("'curved' is deferred"));
        rv4("{ routing: orthogonal }\na -> b\n"); // the built modes are accepted
        let p = rv4("{ routing: straight }\na -> b\n");
        assert_eq!(p.links[0].routing, crate::resolve::Strategy::Straight);
    }

    #[test]
    fn operator_sets_markers_and_line_style() {
        let p = rv4("|box#a|\n|box#b|\na --> b\n");
        let w = &p.links[0];
        assert_eq!(w.markers.end, MarkerKind::Arrow);
        assert!(
            matches!(w.attrs.get("stroke-style"), Some(ResolvedValue::Ident(s)) if s == "dashed")
        );
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
        let p =
            rv4("{ layout: sequence }\n|box#api|\n|cyl#db|\napi -> db\n|alt| [\n  db --> api\n]\n");
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
        let p = rv4(
            "{ |room::group| [\n  |box#inlet|\n  |box#outlet|\n  inlet -> outlet\n] }\n|room#r|\n",
        );
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
}

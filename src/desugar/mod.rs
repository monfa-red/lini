//! Desugar: lower all surface sugar (types, templates, defines, element/descendant
//! rules, labels, scene defaults) to primitive shapes + `.lini-*` classes, so the
//! core only ever sees primitives. Design:
//! `docs/superpowers/specs/2026-06-20-desugar-to-primitives-design.md`.
//!
//! After lowering, every instance is a `|primitive|` wearing its `.lini-*` chain,
//! define bodies are inlined, element/descendant rules collapse into the `.lini-*`
//! class namespace, and the global block carries the scene + link defaults plus the
//! generated class defs. The pass is **idempotent**: every injection is an
//! override-in-place merge, and an already-lowered node is passed through.

pub(crate) mod bundles;
mod classes;
mod labels;
pub(crate) mod scene;
mod types;

use crate::error::Error;
use crate::resolve::NodeKind;
use crate::span::Span;
use crate::syntax::ast::{
    Child, Decl, File, Link, Node, Rule, SelUnit, Selector, StyleItem, TextNode, Value,
};
use bundles::root_defaults;
use classes::{class_defs, is_lini_class, lini_class, merge_decls, worn_classes};
use std::collections::{BTreeSet, HashMap};
use types::{Types, is_template};

type Bodies = HashMap<String, (Vec<Child>, Vec<Link>)>;

/// Lower a parsed file to primitives + `.lini-*` classes.
pub fn desugar(file: &File) -> Result<File, Error> {
    let types = Types::build(file)?;

    // ── Stylesheet walk: element-rule decls per type, define bodies, the extra
    //    class order, and user vars / root decls / rules. Link defaults are a
    //    resolve-time cascade now (SPEC §9), not a desugared rule. ──
    let mut element_rules: HashMap<String, Vec<Decl>> = HashMap::new();
    let mut bodies: Bodies = HashMap::new();
    let mut extra_order: Vec<String> = Vec::new();
    let mut user_root: Vec<Decl> = Vec::new();
    let mut user_vars: Vec<Decl> = Vec::new();
    let mut user_rules: Vec<Rule> = Vec::new();
    let mut user_funcs: Vec<crate::syntax::ast::FuncDef> = Vec::new();

    for item in &file.stylesheet {
        match item {
            StyleItem::RootDecl(d) => user_root.push(d.clone()),
            // Functions are compile-time (SPEC §11.7); pass them through so resolve
            // can fold values against them.
            StyleItem::Func(f) => user_funcs.push(f.clone()),
            StyleItem::Var(d) => user_vars.push(d.clone()),
            StyleItem::Define(d) => {
                element_rules
                    .entry(d.name.clone())
                    .or_default()
                    .extend(d.style.iter().cloned());
                bodies.insert(d.name.clone(), (d.children.clone(), d.links.clone()));
                push_unique(&mut extra_order, &d.name);
            }
            StyleItem::Rule(r) => match r.selector.units.as_slice() {
                // `|box| { }` — a bare element rule folds into the type's class def.
                // `|table#main| { }` (id-pinned) is an id rule, kept as a user rule.
                [SelUnit::Type { name, id: None }] => element_rules
                    .entry(name.clone())
                    .or_default()
                    .extend(r.decls.iter().cloned()),
                // A pre-lowered type class (`.lini-X`, on re-desugar): fold it back
                // as an element rule so the regenerated class is byte-identical.
                [SelUnit::Class(c)] if is_lini_class(c) => {
                    let x = c.strip_prefix("lini-").unwrap().to_string();
                    element_rules
                        .entry(x.clone())
                        .or_default()
                        .extend(r.decls.iter().cloned());
                    if NodeKind::parse(&x).is_none() && !is_template(&x) {
                        push_unique(&mut extra_order, &x);
                    }
                }
                // Descendant rules and user single-class rules keep their place.
                _ => user_rules.push(rewrite_selector(r, &types)?),
            },
        }
    }

    // ── Lower instances, then auto-create root boxes for undeclared link ids — counting
    //    messages inside any root-sequence frame, since a frame opens no scope and its
    //    endpoints resolve against the scene's participants (SPEC §10). ──
    let mut instances = Vec::new();
    for child in &file.instances {
        instances.push(lower_child(child, &types, &bodies)?);
    }
    let declared = scene::declared_ids(&instances);
    let mut root_msgs: Vec<&Link> = file.links.iter().collect();
    root_msgs.extend(gather_frame_messages(&instances));
    for (id, span) in scene::auto_created_ids(&root_msgs, &declared) {
        instances.push(Child::Box(lower_node(
            &scene::auto_box(&id, span),
            &types,
            &bodies,
        )?));
    }

    // ── Present types = every `.lini-X` class worn anywhere. ──
    let mut present: BTreeSet<String> = BTreeSet::new();
    for c in &instances {
        mark_present(c, &mut present);
    }

    // ── Assemble the new stylesheet (a canonical order, so re-desugar is stable):
    //    scene config, vars, the generated `.lini-*` defs, then the user
    //    descendant/class rules. ──
    let mut stylesheet: Vec<StyleItem> = Vec::new();
    // The scene defaults, plus any root-engine defaults (a root `{ layout: sequence }` gets
    // the sequence `gap`), then the user's own decls on top.
    let base = merge_decls(
        root_defaults(),
        &bundles::root_layout_defaults(root_layout(&user_root)),
    );
    for d in merge_decls(base, &user_root) {
        stylesheet.push(StyleItem::RootDecl(d));
    }
    for d in user_vars {
        stylesheet.push(StyleItem::Var(d));
    }
    for f in user_funcs {
        stylesheet.push(StyleItem::Func(f));
    }
    for r in class_defs(&present, &element_rules, &extra_order) {
        stylesheet.push(StyleItem::Rule(r));
    }
    for r in user_rules {
        stylesheet.push(StyleItem::Rule(r));
    }

    Ok(File {
        stylesheet,
        stylesheet_span: Span::empty(),
        instances,
        links: file.links.iter().map(labels::lower_link).collect(),
    })
}

/// The sequence frame types (SPEC §10): they open no scope, so their `[ ]` messages resolve
/// against the enclosing sequence — counted for its auto-create here, kept in place for the
/// layout (which anchors each message to its frame by source position). Shared with resolve
/// (frame transparency) and the layout engine.
pub(crate) const FRAME_TYPES: [&str; 4] = ["loop", "opt", "alt", "else"];

/// Whether a (lowered) node wears a frame type's `.lini-*` class.
fn is_frame_classes(classes: &[String]) -> bool {
    classes.iter().any(|c| {
        c.strip_prefix("lini-")
            .is_some_and(|x| FRAME_TYPES.contains(&x))
    })
}

/// The messages inside a scope's frames (SPEC §10 — a frame opens no scope, so its endpoints
/// belong to the enclosing sequence's auto-create), descending through nested frames.
/// Read-only: the frames keep their links in place, so desugar stays a fixed point.
fn gather_frame_messages(children: &[Child]) -> Vec<&Link> {
    let mut out = Vec::new();
    for c in children {
        if let Child::Box(n) = c
            && is_frame_classes(&n.classes)
        {
            out.extend(n.links.iter());
            out.extend(gather_frame_messages(&n.children));
        }
    }
    out
}

fn lower_child(child: &Child, types: &Types, bodies: &Bodies) -> Result<Child, Error> {
    match child {
        Child::Box(n) => Ok(Child::Box(lower_node(n, types, bodies)?)),
        Child::Text(t) => Ok(Child::Text(t.clone())),
    }
}

fn decl(name: &str, values: Vec<Value>) -> Decl {
    Decl {
        name: name.into(),
        groups: vec![values],
        span: Span::empty(),
    }
}

/// The grid column count for a table / entity node (SPEC §8): its own `columns:` decl,
/// else a bundle default in its chain (`entity` carries `columns: auto auto`). `None`
/// when undeterminable — the auto-header and title-span then no-op.
fn column_count(style: &[Decl], chain: &[String]) -> Option<usize> {
    if let Some(d) = style.iter().find(|d| d.name == "columns") {
        let n = count_tracks(d);
        if n > 0 {
            return Some(n);
        }
    }
    chain.iter().rev().find_map(|name| {
        let n = count_tracks(
            bundles::template_bundle(name)
                .iter()
                .find(|d| d.name == "columns")?,
        );
        (n > 0).then_some(n)
    })
}

/// Tracks a `columns:` value declares — each token is one track, `repeat(N)` is N.
fn count_tracks(d: &Decl) -> usize {
    d.groups
        .iter()
        .flatten()
        .map(|v| match v {
            Value::Call(c) if c.name == "repeat" => match c.args.first() {
                Some(Value::Number(n)) if *n >= 1.0 => *n as usize,
                _ => 1,
            },
            _ => 1,
        })
        .sum()
}

/// A `|header|` node carrying `text` (SPEC §8). With `span`, it is an `|entity|`'s title
/// at the grid top-left; without, it wraps one bare-text table cell (the auto-header).
fn header_node(text: &TextNode, span: Option<usize>) -> Node {
    let style = match span {
        Some(cols) => vec![
            decl("cell", vec![Value::Number(1.0), Value::Number(1.0)]),
            decl("span", vec![Value::Number(cols as f64)]),
        ],
        None => Vec::new(),
    };
    Node {
        id: None,
        ty: Some("header".into()),
        label: Some(text.clone()),
        classes: Vec::new(),
        style,
        style_span: None,
        children: Vec::new(),
        links: Vec::new(),
        span: text.span,
    }
}

/// Auto-header a `|table|`'s first row (SPEC §8): wrap the first `cols` children as
/// `|header|` cells when they are all bare text. A first row holding a box or an
/// explicit `cell:` is left alone — that is a custom layout, not a header.
fn wrap_header_row(
    children: &mut [Child],
    cols: usize,
    types: &Types,
    bodies: &Bodies,
) -> Result<(), Error> {
    let row_end = cols.min(children.len());
    if row_end == 0
        || !children[..row_end]
            .iter()
            .all(|c| matches!(c, Child::Text(_)))
    {
        return Ok(());
    }
    for c in &mut children[..row_end] {
        if let Child::Text(t) = c {
            *c = Child::Box(lower_node(&header_node(t, None), types, bodies)?);
        }
    }
    Ok(())
}

fn lower_node(node: &Node, types: &Types, bodies: &Bodies) -> Result<Node, Error> {
    let ty = node.ty.as_deref().unwrap_or("box");
    let info = types.resolve(ty, node.span)?;
    let kind = info.kind;

    // Idempotency: a node already at a primitive type and wearing its `.lini-<kind>`
    // class is already lowered — keep its classes and type verbatim (re-prepending
    // worn classes would duplicate them, and a lowered define's `.lini-<name>` is
    // unrecoverable from the now-primitive type).
    let already = NodeKind::parse(ty).is_some()
        && node.classes.iter().any(|c| *c == lini_class(kind.as_str()));

    let classes = if already {
        node.classes.clone()
    } else {
        let mut cs = worn_classes(&info);
        cs.extend(node.classes.iter().cloned());
        cs
    };
    let new_ty = if already {
        node.ty.clone()
    } else {
        Some(kind.as_str().to_string())
    };

    // Define bodies (base→derived) materialize ahead of the node's own children;
    // an already-lowered node has no define in its chain, so this is a no-op there.
    let mut children = Vec::new();
    if !already {
        for name in &info.chain {
            if let Some((body, _)) = bodies.get(name) {
                for c in body {
                    children.push(lower_child(c, types, bodies)?);
                }
            }
        }
    }
    for c in &node.children {
        children.push(lower_child(c, types, bodies)?);
    }

    // Table / entity structure (SPEC §8). `cols` is the grid column count, driving both
    // a `|table|`'s auto-header (its first row → `|header|` cells, below) and an
    // `|entity|`'s title span (its label → a spanning header, in the smart-label block).
    let is_entity = info.chain.iter().any(|n| n == "entity");
    let is_table = !is_entity && info.chain.iter().any(|n| n == "table");
    let cols = column_count(&node.style, &info.chain);
    if is_table && let Some(cols) = cols {
        wrap_header_row(&mut children, cols, types, bodies)?;
    }

    // The smart label, lowered per type (SPEC §3/§7) — the single shared lowering
    // for a node's text (a link's labels go through the same `TextNode`). A box-like
    // type → centred text prepended; a group/table → a `|caption|` child; an
    // icon/sign → the `symbol`. An empty `""` lowers to nothing. Geometry-only
    // shapes (line/poly/path/image) hold no text.
    let text_capable = !matches!(
        kind,
        NodeKind::Line | NodeKind::Poly | NodeKind::Path | NodeKind::Image
    );
    let is_icon = kind == NodeKind::Icon;
    let is_container = info.chain.iter().any(|n| n == "group");
    let mut style = node.style.clone();
    let mut kept_label = None;
    if let Some(label) = node.label.as_ref().filter(|l| !l.text.is_empty()) {
        if is_icon {
            if style.iter().any(|d| d.name == "symbol") {
                return Err(Error::at(
                    node.span,
                    "an icon's symbol is its label or 'symbol:', not both",
                ));
            }
            style.push(labels::symbol_decl(&label.text, node.span));
        } else if is_entity {
            // An entity's label is its title: a `|header|` spanning every column (SPEC §8).
            let title = header_node(label, Some(cols.unwrap_or(2)));
            children.insert(0, Child::Box(lower_node(&title, types, bodies)?));
        } else if is_container {
            let caption = lower_node(&labels::caption_node(label), types, bodies)?;
            children.insert(0, Child::Box(caption));
        } else if text_capable {
            children.insert(0, Child::Text(label.clone()));
        } else {
            // Geometry primitives (line/poly/path/image) draw no text, but a label
            // still *names* the node — keep it so a chart can read a `|line|` series'
            // legend name. Inert for a standalone primitive (render ignores it).
            kept_label = Some(label.clone());
        }
    }

    // In an entity, header / footer cells span every column (SPEC §8): the title above
    // carries its own span; a hand-written `|footer|` (or `|header|`) gets one here.
    if is_entity && let Some(cols) = cols {
        for child in &mut children {
            if let Child::Box(n) = child
                && n.classes
                    .iter()
                    .any(|c| c == "lini-header" || c == "lini-footer")
                && !n.style.iter().any(|d| d.name == "span")
            {
                n.style.push(decl("span", vec![Value::Number(cols as f64)]));
            }
        }
    }

    // Links: define-body links (base→derived) then the node's own, each lowered
    // (head label folded into the label list, auto-`along:` filled).
    let mut links = Vec::new();
    if !already {
        for name in &info.chain {
            if let Some((_, body)) = bodies.get(name) {
                for w in body {
                    links.push(labels::lower_link(w));
                }
            }
        }
    }
    for w in &node.links {
        links.push(labels::lower_link(w));
    }

    // Auto-create undeclared body-link endpoints among this body's own children (SPEC §3 —
    // auto-create runs in any scope, not just the root), counting messages inside any frame
    // child so a participant first named inside a frame is created on the sequence, not the
    // frame. A frame (`loop`/`opt`/`alt`/`else`) opens no scope, so it never auto-creates —
    // its endpoints resolve against the enclosing sequence's participants (SPEC §10).
    if !already && !is_frame_classes(&classes) {
        let declared = scene::declared_ids(&children);
        // Scope the message borrows of `children` so the auto-create push below is free.
        let to_create = {
            let mut msgs: Vec<&Link> = node.links.iter().collect();
            msgs.extend(gather_frame_messages(&children));
            scene::auto_created_ids(&msgs, &declared)
        };
        for (auto_id, auto_span) in to_create {
            let created = lower_node(&scene::auto_box(&auto_id, auto_span), types, bodies)?;
            children.push(Child::Box(created));
        }
    }

    Ok(Node {
        id: node.id.clone(),
        ty: new_ty,
        // A box / container / icon label is lowered into `children` / `style` (so the
        // output carries none); a geometry primitive's label is kept verbatim. Both
        // are idempotent — re-desugaring lowers nothing further.
        label: kept_label,
        classes,
        style,
        style_span: node.style_span,
        children,
        links,
        span: node.span,
    })
}

/// Rewrite a non-element rule's selector into the class / id namespace: a bare
/// `|type|` unit becomes a `.lini-<type>` **class** (so it prints as `.lini-type`
/// and re-desugars unchanged); a `|type#id|` keeps a single unit that matches both
/// the type class and the id; `.class` / `#id` units are kept. Already-lowered
/// `.lini-*` names pass through (re-desugar idempotency). Element rules
/// (`|box| { }`, no id) fold into the type's class def separately, not here.
fn rewrite_selector(rule: &Rule, types: &Types) -> Result<Rule, Error> {
    let mut units = Vec::with_capacity(rule.selector.units.len());
    for unit in &rule.selector.units {
        match unit {
            SelUnit::Type { name, id } => {
                let class = if is_lini_class(name) {
                    name.clone()
                } else if types.is_known(name) {
                    lini_class(name)
                } else {
                    return Err(Error::at(
                        rule.span,
                        format!("unknown type '{}' in selector", name),
                    ));
                };
                match id {
                    Some(_) => units.push(SelUnit::Type {
                        name: class,
                        id: id.clone(),
                    }),
                    None => units.push(SelUnit::Class(class)),
                }
            }
            SelUnit::Class(c) => units.push(SelUnit::Class(c.clone())),
            SelUnit::Id(i) => units.push(SelUnit::Id(i.clone())),
        }
    }
    Ok(Rule {
        selector: Selector { units },
        decls: rule.decls.clone(),
        span: rule.span,
    })
}

/// Record every `.lini-X` class worn anywhere as the bare type name `X` (the gate
/// for which class defs to emit).
fn mark_present(child: &Child, present: &mut BTreeSet<String>) {
    if let Child::Box(n) = child {
        for c in &n.classes {
            if let Some(x) = c.strip_prefix("lini-") {
                present.insert(x.to_string());
            }
        }
        for ch in &n.children {
            mark_present(ch, present);
        }
    }
}

/// The `layout:` ident set on the root, if any — picks the root-engine defaults.
fn root_layout(user_root: &[Decl]) -> Option<&str> {
    user_root
        .iter()
        .rev()
        .find(|d| d.name == "layout")
        .and_then(|d| match d.groups.as_slice() {
            [group] => match group.as_slice() {
                [Value::Ident(s)] => Some(s.as_str()),
                _ => None,
            },
            _ => None,
        })
}

fn push_unique(v: &mut Vec<String>, name: &str) {
    if !v.iter().any(|x| x == name) {
        v.push(name.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lower(src: &str) -> File {
        let toks = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(&toks).expect("parse");
        desugar(&file).expect("desugar")
    }
    fn root_box<'a>(f: &'a File, id: &str) -> &'a Node {
        f.instances
            .iter()
            .find_map(|c| match c {
                Child::Box(n) if n.id.as_deref() == Some(id) => Some(n),
                _ => None,
            })
            .expect("node")
    }
    fn is_header(c: &Child) -> bool {
        matches!(c, Child::Box(n) if n.classes.iter().any(|x| x == "lini-header"))
    }

    #[test]
    fn table_first_row_becomes_header_cells() {
        let f = lower("|table#t| { columns: 30 30 } [\n\"a\"\n\"b\"\n\"c\"\n\"d\"\n]\n");
        let t = root_box(&f, "t");
        // Row 0 (the first `cols` cells) are header boxes; the body stays bare text.
        assert!(
            is_header(&t.children[0]) && is_header(&t.children[1]),
            "first row is header"
        );
        assert!(
            matches!(t.children[2], Child::Text(_)) && matches!(t.children[3], Child::Text(_)),
            "body stays bare text"
        );
    }

    #[test]
    fn entity_label_is_a_spanning_header_fields_stay_text() {
        let f = lower("|entity#e| \"Users\" [\n\"id\"\n\"int\"\n]\n");
        let e = root_box(&f, "e");
        let Child::Box(title) = &e.children[0] else {
            panic!("the entity title is a box");
        };
        assert!(title.classes.iter().any(|c| c == "lini-header"));
        assert!(
            title.style.iter().any(|d| d.name == "span"),
            "the title spans its columns"
        );
        // Field rows are not auto-headered — only the label is the title.
        assert!(matches!(e.children[1], Child::Text(_)) && matches!(e.children[2], Child::Text(_)));
    }

    #[test]
    fn bare_grid_does_not_auto_header() {
        let f = lower("|grid#g| { columns: 30 30 } [\n\"a\"\n\"b\"\n]\n");
        let g = root_box(&f, "g");
        assert!(
            g.children.iter().all(|c| !is_header(c)),
            "a bare grid is not a table — no auto-header"
        );
    }
}

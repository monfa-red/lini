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

mod bundles;
mod classes;
mod labels;
mod scene;
mod types;

use crate::error::Error;
use crate::resolve::ShapeKind;
use crate::span::Span;
use crate::syntax::ast::{Child, Decl, File, Link, Node, Rule, SelPart, Selector, StyleItem};
use bundles::{link_defaults, root_defaults};
use classes::{class_defs, is_lini_class, lini_class, merge_decls, worn_classes};
use std::collections::{BTreeSet, HashMap};
use types::{Types, is_template};

type Bodies = HashMap<String, (Vec<Child>, Vec<Link>)>;

/// Lower a parsed file to primitives + `.lini-*` classes.
pub fn desugar(file: &File) -> Result<File, Error> {
    let types = Types::build(file)?;

    // ── Stylesheet walk: element-rule decls per type, define bodies, the extra
    //    class order, user vars / root decls / rules, and link-default overrides. ──
    let mut element_rules: HashMap<String, Vec<Decl>> = HashMap::new();
    let mut bodies: Bodies = HashMap::new();
    let mut extra_order: Vec<String> = Vec::new();
    let mut user_root: Vec<Decl> = Vec::new();
    let mut user_vars: Vec<Decl> = Vec::new();
    let mut user_rules: Vec<Rule> = Vec::new();
    let mut link_user: Vec<Decl> = Vec::new();

    for item in &file.stylesheet {
        match item {
            StyleItem::RootDecl(d) => user_root.push(d.clone()),
            StyleItem::Var(d) => user_vars.push(d.clone()),
            StyleItem::Define(d) => {
                element_rules
                    .entry(d.name.clone())
                    .or_default()
                    .extend(d.style.iter().cloned());
                bodies.insert(d.name.clone(), (d.children.clone(), d.links.clone()));
                push_unique(&mut extra_order, &d.name);
            }
            StyleItem::Rule(r) => match r.selector.parts.as_slice() {
                [SelPart::Type(t)] if t == "link" => link_user.extend(r.decls.iter().cloned()),
                [SelPart::Type(t)] => element_rules
                    .entry(t.clone())
                    .or_default()
                    .extend(r.decls.iter().cloned()),
                // A pre-lowered type class (`.lini-X`, on re-desugar): fold it back
                // as an element rule so the regenerated class is byte-identical.
                [SelPart::Class(c)] if is_lini_class(c) => {
                    let x = c.strip_prefix("lini-").unwrap().to_string();
                    element_rules
                        .entry(x.clone())
                        .or_default()
                        .extend(r.decls.iter().cloned());
                    if ShapeKind::parse(&x).is_none() && !is_template(&x) {
                        push_unique(&mut extra_order, &x);
                    }
                }
                // Descendant rules and user single-class rules keep their place.
                _ => user_rules.push(rewrite_selector(r, &types)?),
            },
        }
    }

    // ── Lower instances, then auto-create root boxes for undeclared link ids. ──
    let mut instances = Vec::new();
    for child in &file.instances {
        instances.push(lower_child(child, &types, &bodies)?);
    }
    let declared = scene::declared_ids(&instances);
    for (id, span) in scene::auto_created_ids(&file.links, &declared) {
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
    //    scene config, vars, link defaults, the generated `.lini-*` defs, then the
    //    user descendant/class rules. ──
    let mut stylesheet: Vec<StyleItem> = Vec::new();
    for d in merge_decls(root_defaults(), &user_root) {
        stylesheet.push(StyleItem::RootDecl(d));
    }
    for d in user_vars {
        stylesheet.push(StyleItem::Var(d));
    }
    // The `-> { }` link defaults are the link layer's config; emit them only when
    // the scene actually has a link, so a linkless diagram carries no link block.
    let has_link = !file.links.is_empty() || instances.iter().any(child_has_link);
    if has_link {
        stylesheet.push(StyleItem::Rule(link_rule(merge_decls(
            link_defaults(),
            &link_user,
        ))));
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
        links: file.links.iter().map(labels::auto_along).collect(),
    })
}

fn lower_child(child: &Child, types: &Types, bodies: &Bodies) -> Result<Child, Error> {
    match child {
        Child::Box(n) => Ok(Child::Box(lower_node(n, types, bodies)?)),
        Child::Text(t) => Ok(Child::Text(t.clone())),
    }
}

fn lower_node(node: &Node, types: &Types, bodies: &Bodies) -> Result<Node, Error> {
    let ty = node.ty.as_deref().unwrap_or("box");
    let info = types.resolve(ty, node.span)?;
    let kind = info.kind;

    // Idempotency: a node already at a primitive type and wearing its `.lini-<kind>`
    // class is already lowered — keep its classes and type verbatim (re-prepending
    // worn classes would duplicate them, and a lowered define's `.lini-<name>` is
    // unrecoverable from the now-primitive type).
    let already = ShapeKind::parse(ty).is_some()
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

    // id-as-label for a text-capable leaf box (SPEC §3). Geometry-only shapes hold
    // no centred text; an icon consumes its text; a container holds its children.
    let text_capable = !matches!(
        kind,
        ShapeKind::Line | ShapeKind::Poly | ShapeKind::Path | ShapeKind::Image
    );
    let is_icon = kind == ShapeKind::Icon;
    let is_container = info.chain.iter().any(|n| n == "group");
    if children.is_empty()
        && text_capable
        && let Some(label) = labels::label_child_for(node, is_icon, is_container)
    {
        children.push(label);
    }

    // Links: define-body links (base→derived) then the node's own, each auto-along'd.
    let mut links = Vec::new();
    if !already {
        for name in &info.chain {
            if let Some((_, body)) = bodies.get(name) {
                for w in body {
                    links.push(labels::auto_along(w));
                }
            }
        }
    }
    for w in &node.links {
        links.push(labels::auto_along(w));
    }

    Ok(Node {
        id: node.id.clone(),
        ty: new_ty,
        classes,
        style: node.style.clone(),
        style_span: node.style_span,
        children,
        links,
        span: node.span,
    })
}

/// Rewrite a non-element rule's selector to the class namespace: each `|type|`
/// part becomes `.lini-<type>` (validated as a known type), each `.class` part is
/// kept. Element rules (`|box| { }`) are handled separately, not here.
fn rewrite_selector(rule: &Rule, types: &Types) -> Result<Rule, Error> {
    let mut parts = Vec::with_capacity(rule.selector.parts.len());
    for part in &rule.selector.parts {
        match part {
            SelPart::Type(t) => {
                if !types.is_known(t) {
                    return Err(Error::at(
                        rule.span,
                        format!("unknown type '{}' in selector", t),
                    ));
                }
                parts.push(SelPart::Class(lini_class(t)));
            }
            SelPart::Class(c) => parts.push(SelPart::Class(c.clone())),
        }
    }
    Ok(Rule {
        selector: Selector { parts },
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

/// Whether this child (or any descendant) carries an internal link — define-body
/// links are already materialized onto the node by lowering, so this sees every
/// drawn link below the root.
fn child_has_link(child: &Child) -> bool {
    match child {
        Child::Box(n) => !n.links.is_empty() || n.children.iter().any(child_has_link),
        Child::Text(_) => false,
    }
}

/// The `-> { }` link-defaults rule (the link glyph is the reserved `link` element).
fn link_rule(decls: Vec<Decl>) -> Rule {
    Rule {
        selector: Selector {
            parts: vec![SelPart::Type("link".to_string())],
        },
        decls,
        span: Span::empty(),
    }
}

fn push_unique(v: &mut Vec<String>, name: &str) {
    if !v.iter().any(|x| x == name) {
        v.push(name.to_string());
    }
}

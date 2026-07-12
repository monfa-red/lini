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

mod chrome;
mod classes;
mod drawing;
mod labels;
mod page;
mod scale;
pub(crate) mod scene;
mod tables;
mod titleblock;
pub(crate) mod tree;
pub(crate) mod types;

use crate::error::Error;
use crate::ledger::defaults::root_defaults;
use crate::resolve::NodeKind;
use crate::span::Span;
use crate::syntax::ast::{
    Child, Decl, File, Link, Node, Rule, SelUnit, Selector, StyleItem, Value,
};
use classes::{class_defs, is_lini_class, lini_class, merge_decls, worn_classes};
use std::collections::{BTreeSet, HashMap};
use tables::{
    column_count, distribute_cell_alignment, header_node, wrap_body_cells, wrap_header_row,
};
use types::{Types, is_template};

type Bodies = HashMap<String, (Vec<Child>, Vec<Link>)>;

/// Lower a parsed file to primitives + `.lini-*` classes.
pub fn desugar(file: &File) -> Result<File, Error> {
    let types = Types::build(file)?;

    // ── Stylesheet walk: element-rule decls per type, define bodies, the extra
    //    class order, and user vars / root decls / rules. The baked link base stays
    //    a resolve-time layer [SPEC 9]; a `|-|` rule lowers to `.lini-link` like any
    //    selector, so the link cascade is the node cascade. ──
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
            // Functions are compile-time [SPEC 10.7]; pass them through so resolve
            // can fold values against them.
            StyleItem::Binding(f) => user_funcs.push(f.clone()),
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
                // `.lini-link` / `.lini-dimension` are the lowered `|-|` / `(-)`
                // [SPEC 9, 15.6], not instance types: no node wears them (links wear
                // them at resolve), so keep them plain rules the link cascade reads —
                // folding either as a type class would drop it on re-desugar. Every
                // other `.lini-X` is a real type.
                [SelUnit::Class(c)]
                    if is_lini_class(c) && (c == "lini-link" || c == "lini-dimension") =>
                {
                    user_rules.push(rewrite_selector(r, &types)?)
                }
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
    //    endpoints resolve against the scene's participants [SPEC 13]. ──
    let root_drawing = root_layout(&user_root) == Some("drawing");
    let mut instances = Vec::new();
    for child in &file.instances {
        instances.push(lower_child(child, &types, &bodies, root_drawing)?);
    }
    // A root `{ layout: tree }` scene flattens its own topic nesting [SPEC 12],
    // like a node tree does in `lower_node` — before auto-create below. Its gap
    // default rides `root_layout_defaults`, not `ensure_gap`.
    let mut root_branch_links: Vec<Link> = Vec::new();
    if tree::is_tree_scope(&user_root) {
        tree::flatten(&mut instances, &mut root_branch_links, &user_root);
    }
    // A drawing scope never auto-creates [SPEC 15]: an annotation must point at
    // real geometry, so an unknown endpoint stays unknown and errors at resolve.
    if !root_drawing {
        let declared = scene::declared_ids(&instances);
        let mut root_msgs: Vec<&Link> = file.links.iter().collect();
        root_msgs.extend(gather_frame_messages(&instances));
        for (id, span) in scene::auto_created_ids(&root_msgs, &declared) {
            instances.push(Child::Box(lower_node(
                &scene::auto_box(&id, span),
                &types,
                &bodies,
                false,
            )?));
        }
    }

    // ── The scale fold [SPEC 15.1/18]: drawing scopes and pages gain their
    //    generated internal `px-per-unit:` from ratio × unit × density. ──
    scale::fold(&mut instances, &mut user_root, root_drawing)?;

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
    let mut layout_defaults =
        crate::ledger::defaults::root_layout_defaults(root_layout(&user_root));
    // A file whose drawn content is only `|page|` sheets hugs them — the
    // paper is the margin, so the root's padding defaults to 0 [SPEC 15.8];
    // the user's own padding still wins.
    let only_pages = !instances.is_empty()
        && instances
            .iter()
            .all(|c| matches!(c, Child::Box(n) if n.classes.iter().any(|k| k == "lini-page")));
    if only_pages {
        layout_defaults.push(decl("padding", vec![Value::Number(0.0)]));
    }
    let base = merge_decls(root_defaults(), &layout_defaults);
    for d in merge_decls(base, &user_root) {
        stylesheet.push(StyleItem::RootDecl(d));
    }
    for d in user_vars {
        stylesheet.push(StyleItem::Var(d));
    }
    for f in user_funcs {
        stylesheet.push(StyleItem::Binding(f));
    }
    // The chart / sequence engines synthesize `|line|` / `|block|` shapes at layout
    // (with no source node), so their primitive class rules must exist even unworn —
    // a plain scene synthesizes nothing and skips them [SPEC 17].
    let synthesizes_shapes = ["chart", "pie", "sequence"]
        .iter()
        .any(|t| present.contains(*t))
        || root_layout(&user_root) == Some("sequence");
    for r in class_defs(&present, &element_rules, &extra_order, synthesizes_shapes) {
        stylesheet.push(StyleItem::Rule(r));
    }
    for r in classes::scoped_note_rules(&present, &user_rules) {
        stylesheet.push(StyleItem::Rule(r));
    }
    for r in user_rules {
        stylesheet.push(StyleItem::Rule(r));
    }

    Ok(File {
        stylesheet,
        stylesheet_span: Span::empty(),
        instances,
        links: file
            .links
            .iter()
            .chain(&root_branch_links)
            .flat_map(labels::split_chain)
            .map(|w| labels::lower_link(&w))
            .collect(),
    })
}

/// The sequence frame types [SPEC 13]: they open no scope, so their `[ ]` messages resolve
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

/// The messages inside a scope's frames ([SPEC 13] — a frame opens no scope, so its endpoints
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

/// `in_drawing`: whether this child sits in a drawing scope [SPEC 15] — the
/// gate for the generated chrome. Class-detected, like frames: a container
/// made a drawing only by an element rule is not seen here (the accepted
/// stage-1 edge; resolve's gates still hold).
fn lower_child(
    child: &Child,
    types: &Types,
    bodies: &Bodies,
    in_drawing: bool,
) -> Result<Child, Error> {
    match child {
        Child::Box(n) => Ok(Child::Box(lower_node(n, types, bodies, in_drawing)?)),
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

fn lower_node(
    node: &Node,
    types: &Types,
    bodies: &Bodies,
    in_drawing: bool,
) -> Result<Node, Error> {
    let ty = node.ty.as_deref().unwrap_or("box");
    let info = types.resolve(ty, node.span)?;
    let kind = info.kind;

    // The drawing scope [SPEC 15]: opened by a drawing node, carried through
    // its parts and their features, sealed by a child that owns its own layout
    // (a |row|, a |table|, a chart — it "lays out as one box", [SPEC 15.1]).
    let is_drawing = is_drawing_body(&info.chain, &node.style);
    let child_in_drawing =
        is_drawing || (in_drawing && !seals_drawing_scope(&info.chain, &node.style));

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
                    children.push(lower_child(c, types, bodies, child_in_drawing)?);
                }
            }
        }
    }
    for c in &node.children {
        children.push(lower_child(c, types, bodies, child_in_drawing)?);
    }
    // The generated chrome [SPEC 15.7] — real children, so the cascade styles
    // or removes them. Only for a node in a drawing scope, and only on first
    // lowering (re-desugar keeps the ones already there).
    if !already && in_drawing {
        for ch in drawing::chrome_children(node, kind, &info.chain) {
            children.push(Child::Box(lower_node(&ch, types, bodies, false)?));
        }
    }
    // The sheet's furniture [SPEC 15.8]: `sheet:` desugars in place to
    // `width` / `height` in mm first (the zone counts derive from the final
    // numbers), then the pinned chrome children (frame, zone grid, centring
    // marks) are generated, positioned by the layout once the page is sized;
    // a `|title-block|` child is pulled out of the flow here so the page can
    // seat it flush inside the frame's bottom-right corner.
    let is_page = info.chain.iter().any(|n| n == "page");
    let mut page_style: Option<Vec<Decl>> = None;
    if is_page {
        let mut s = node.style.clone();
        page::expand_sheet(&mut s)?;
        page::default_direction(&mut s, node.span);
        page_style = Some(s);
    }
    if !already && is_page {
        for ch in page::chrome_children(page_style.as_deref().expect("a page"), node.span) {
            children.push(Child::Box(lower_node(&ch, types, bodies, false)?));
        }
    }
    if is_page {
        for child in &mut children {
            if let Child::Box(n) = child
                && n.classes.iter().any(|c| c == "lini-title-block")
                && !n.style.iter().any(|d| d.name == "pin")
            {
                n.style.push(decl(
                    "pin",
                    vec![Value::Ident("bottom".into()), Value::Ident("right".into())],
                ));
            }
        }
    }

    // Table / entity structure [SPEC 8]. `cols` is the grid column count, driving both
    // a `|table|`'s auto-header (its first row → `|header|` cells, below) and an
    // `|entity|`'s title span (its label → a spanning header, in the smart-label block).
    let is_entity = info.chain.iter().any(|n| n == "entity");
    let is_table = !is_entity && info.chain.iter().any(|n| n == "table");
    let cols = column_count(&node.style, &info.chain);
    if is_table && let Some(cols) = cols {
        wrap_header_row(&mut children, cols, types, bodies)?;
    }
    // Wrap every remaining bare-text body cell in a `|cell|` (the box that carries
    // the cell padding, [SPEC 8]). The entity title (a spanning header) is inserted
    // after this, already a box.
    if is_table || is_entity {
        wrap_body_cells(&mut children, types, bodies)?;
    }
    // Distribute the table's per-column `align`/`justify` onto its cells [SPEC 8]:
    // every cell fills its track (the |table| bundle forces `stretch`), so the
    // user's align/justify instead place each cell's text — carried to the cell in
    // its own column. The table's own align/justify are dropped below so `stretch`
    // stands. Only auto-flow cells are covered (the assumption the header sugar
    // already makes); `cell:`/`span:` cells keep the column default.
    if (is_table || is_entity)
        && let Some(cols) = cols
    {
        distribute_cell_alignment(&mut children, &node.style, cols, is_entity)?;
    }

    // The smart label, lowered per type [SPEC 3/7] — the single shared lowering
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
    // A table/entity's own `align`/`justify` are consumed above (distributed to its
    // cells), so drop them here — the bundle's `stretch` fills the cells.
    let mut style: Vec<Decl> = if is_table || is_entity {
        node.style
            .iter()
            .filter(|d| d.name != "align" && d.name != "justify")
            .cloned()
            .collect()
    } else if let Some(expanded) = page_style {
        expanded
    } else {
        node.style.clone()
    };
    // An authored |plane| in a drawing scope is chrome [SPEC 15.8]: its
    // ISO anatomy — thick ends, viewing arrows, the letter — fills from the
    // view's extent at layout, so mark it and layout intercepts it as a
    // placeholder (like the generated chrome types).
    if in_drawing && info.chain.iter().any(|t| t == "plane") {
        style.push(decl("chrome", vec![Value::Ident("plane".into())]));
    }
    // A `|title-block|`'s smart label is its `title` field [SPEC 15.8]: a
    // label — like any field property — selects the structured-field mode.
    let is_title_block = info.chain.iter().any(|t| t == "title-block");
    let mut label = node.label.as_ref().filter(|l| !l.text.is_empty());
    if is_title_block
        && let Some(l) = label.take()
        && !style.iter().any(|d| d.name == "title")
    {
        style.push(Decl {
            name: "title".into(),
            groups: vec![vec![Value::String(l.text.clone())]],
            span: l.span,
        });
    }
    // A `|title-block|` with ISO 7200 field properties builds its grid
    // [SPEC 15.8]; with none, its cells stay authored (the plain-table form).
    // The generated cells are `|cell|` boxes, so the table auto-header skips
    // them and the field grid stands as built.
    if is_title_block && titleblock::has_fields(&style) {
        for cell in titleblock::expand_fields(&mut style, node.span) {
            children.push(Child::Box(lower_node(&cell, types, bodies, false)?));
        }
    }
    let mut kept_label = None;
    if let Some(label) = label {
        if is_icon {
            if style.iter().any(|d| d.name == "symbol") {
                return Err(Error::at(
                    node.span,
                    "an icon's symbol is its label or 'symbol:', not both",
                ));
            }
            style.push(labels::symbol_decl(&label.text, node.span));
        } else if is_entity {
            // An entity's label is its title: a `|header|` spanning every column [SPEC 8].
            let title = header_node(label, Some(cols.unwrap_or(2)));
            children.insert(0, Child::Box(lower_node(&title, types, bodies, false)?));
        } else if is_drawing {
            // A drawing's smart label is its title, lowered to a |footnote|
            // under the view [SPEC 15.8].
            let title = lower_node(&labels::footnote_node(label), types, bodies, false)?;
            children.insert(0, Child::Box(title));
        } else if is_container {
            let caption = lower_node(&labels::caption_node(label), types, bodies, false)?;
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
    // A view sourced from a marker (`of:`) with no authored label composes its
    // title [SPEC 15.8]: seed a placeholder |footnote| the engine fills where it
    // pins the title — the marker (a `|plane|` → `A-A`, a `|magnifier|` → `C`)
    // and the scale ratio are both known there.
    if is_drawing
        && node.style.iter().any(|d| d.name == "of")
        && node.label.as_ref().filter(|l| !l.text.is_empty()).is_none()
    {
        let foot = labels::of_footnote(node.span);
        children.insert(0, Child::Box(lower_node(&foot, types, bodies, false)?));
    }

    // In an entity, header / footer cells span every column [SPEC 8]: the title above
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
                    links.extend(labels::split_chain(w).iter().map(labels::lower_link));
                }
            }
        }
    }
    for w in &node.links {
        links.extend(labels::split_chain(w).iter().map(labels::lower_link));
    }

    // Flatten a `layout: tree` scope's topic nesting into a depth-classed flat
    // list + generated branch links [SPEC 12], **before** this body's
    // auto-create so a branch / cross-link endpoint sees the lifted topics as
    // declared siblings, and before the paint cascade so the level classes are worn.
    if !already && tree::is_tree_scope(&style) {
        tree::ensure_gap(&mut style);
        tree::flatten(&mut children, &mut links, &style);
    }

    // Auto-create undeclared body-link endpoints among this body's own children ([SPEC 3] —
    // auto-create runs in any scope, not just the root), counting messages inside any frame
    // child so a participant first named inside a frame is created on the sequence, not the
    // frame. A frame (`loop`/`opt`/`alt`/`else`) opens no scope, so it never auto-creates —
    // its endpoints resolve against the enclosing sequence's participants [SPEC 13]. A
    // drawing body never auto-creates either [SPEC 15]: its links point at real geometry.
    if !already && !is_frame_classes(&classes) && !is_drawing_body(&info.chain, &node.style) {
        let declared = scene::declared_ids(&children);
        // Scope the message borrows of `children` so the auto-create push below is free.
        let to_create = {
            let mut msgs: Vec<&Link> = node.links.iter().collect();
            msgs.extend(gather_frame_messages(&children));
            scene::auto_created_ids(&msgs, &declared)
        };
        for (auto_id, auto_span) in to_create {
            let created = lower_node(&scene::auto_box(&auto_id, auto_span), types, bodies, false)?;
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
            // `|-|` — the link type [SPEC 9]: every link wears `.lini-link`, so the
            // selector lowers to that class and the node cascade matches it unchanged.
            SelUnit::Link => units.push(SelUnit::Class(lini_class("link"))),
            // `(-)` — the dimension type [SPEC 15.6]: every dimension wears
            // `.lini-dimension`, the `|-|` subtype, layered above `.lini-link`.
            SelUnit::Dimension => units.push(SelUnit::Class(lini_class("dimension"))),
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
/// A drawing scope, detected as frames are — by type chain (`|drawing|` or a
/// define over it) or an explicit `layout: drawing` on the instance [SPEC 15].
fn is_drawing_body(chain: &[String], style: &[Decl]) -> bool {
    chain.iter().any(|t| t == "drawing") || root_layout(style) == Some("drawing")
}

/// Whether a node **seals** an enclosing drawing scope [SPEC 15.1]: it owns a
/// layout (a flow wrapper, a grid, an engine) and arranges its interior as
/// usual — its children are not the drawing's features. The layout-side twin
/// is `layout::owns_layout` (attr-based, post-cascade).
fn seals_drawing_scope(chain: &[String], style: &[Decl]) -> bool {
    chain.iter().any(|t| {
        matches!(
            t.as_str(),
            "row" | "column" | "grid" | "table" | "entity" | "chart" | "pie" | "sequence"
        )
    }) || style
        .iter()
        .any(|d| d.name == "layout" || d.name == "direction")
}

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

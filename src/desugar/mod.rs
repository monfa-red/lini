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

mod autopose;
mod capsule;
pub(crate) mod classes;
mod drawing;
mod gather;
mod labels;
mod labelwire;
mod mint;
mod nest;
mod page;
pub(crate) mod pose;
pub(crate) mod scale;
pub(crate) mod scene;
pub(crate) mod schematic;
mod synth;
mod tables;
mod titleblock;
pub(crate) mod tree;
pub(crate) mod types;

use crate::error::{Code, Error};
use crate::ledger::defaults::root_defaults;
use crate::resolve::NodeKind;
use crate::span::Span;
use crate::syntax::ast::{
    Child, Decl, File, Link, Node, Rule, SelUnit, Selector, StyleItem, Value, layout_of,
};
use classes::{class_defs, is_lini_class, lini_class, merge_decls, worn_classes};
pub(crate) use nest::{Nest, STATEMENT_ENGINES};
use nest::{in_drawing_scope, is_drawing_body, is_schematic_body, seals_schematic_scope};
use std::collections::{BTreeSet, HashMap};
pub(crate) use tables::declared_column_count;
use tables::{header_node, wrap_body_cells};
use types::{Types, is_template};

type Bodies = HashMap<String, (Vec<Child>, Vec<Link>)>;

/// The lowering context threaded through the walk: the type table, the
/// define bodies, and the element-rule decls (define styles + `|type| { }`
/// rules) — the tiers a desugar-time property read (`symbol:`, `prefix:`,
/// `pins:`) can see [SPEC 16]. Descendant / class rules are resolve's; a
/// schematic default reached only through one is out of desugar's sight.
pub(crate) struct Lower<'a> {
    types: &'a Types,
    bodies: &'a Bodies,
    rules: &'a HashMap<String, Vec<Decl>>,
}

impl Lower<'_> {
    /// The effective declaration for `name`: the instance's own style (last
    /// wins), else the chain's element rules / template bundles, derived
    /// first — desugar's slice of the cascade.
    fn chain_decl(&self, chain: &[String], style: &[Decl], name: &str) -> Option<Decl> {
        if let Some(d) = style.iter().rev().find(|d| d.name == name) {
            return Some(d.clone());
        }
        for t in chain.iter().rev() {
            if let Some(d) = self
                .rules
                .get(t)
                .and_then(|ds| ds.iter().rev().find(|d| d.name == name))
            {
                return Some(d.clone());
            }
            if let Some(d) = crate::ledger::defaults::template_bundle(t)
                .into_iter()
                .rev()
                .find(|d| d.name == name)
            {
                return Some(d);
            }
        }
        None
    }
    /// An **authored** node's type chain, base→derived — the argument every
    /// reader above takes. An unresolvable type carries none: the error is
    /// [`lower_node`]'s to raise.
    fn authored_chain(&self, node: &Node) -> Vec<String> {
        self.types
            .resolve(node.ty.as_deref().unwrap_or("box"), node.span)
            .map(|i| i.chain)
            .unwrap_or_default()
    }
    fn chain_ident(&self, chain: &[String], style: &[Decl], name: &str) -> Option<String> {
        self.chain_decl(chain, style, name)?
            .ident()
            .map(str::to_string)
    }
    fn chain_number(&self, chain: &[String], style: &[Decl], name: &str) -> Option<f64> {
        match self
            .chain_decl(chain, style, name)?
            .groups
            .first()?
            .first()?
        {
            Value::Number(v) => Some(*v),
            _ => None,
        }
    }
    fn chain_str(&self, chain: &[String], style: &[Decl], name: &str) -> Option<String> {
        match self
            .chain_decl(chain, style, name)?
            .groups
            .first()?
            .first()?
        {
            Value::String(s) => Some(s.clone()),
            _ => None,
        }
    }
}

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
                // Generated utility classes (cell alignment, the schematic
                // chrome looks) regenerate from the worn set [SPEC 8/16] —
                // drop the incoming copy (folding it back would emit it twice
                // on re-desugar).
                [SelUnit::Class(c)] if classes::is_generated_class(c) => {}
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
    let root_nest = Nest {
        drawing: layout_of(&user_root).is_some_and(crate::resolve::is_drawing_layout),
        schematic: layout_of(&user_root) == Some("schematic"),
    };
    let cx = Lower {
        types: &types,
        bodies: &bodies,
        rules: &element_rules,
    };
    // The scene's statements settle before any of them lower [SPEC 19] — label
    // wires mint their tags, capsules hoist their declarations, and only then
    // does the pose chooser turn the satellites it can now see (`gather`). The
    // root scope is one like any other.
    let root = gather::Scope::gather(
        &cx,
        file.instances.clone(),
        file.links.clone(),
        0,
        root_nest,
        root_nest.schematic,
    )?;
    let mut instances = root.lower(&cx)?;
    let root_links = root.links;
    // A scene owned by a stand-alone `|mindmap|` seats it first [SPEC 8]: the
    // root decls become its generated tree scope (`layout: tree; direction:
    // bilateral; routing: natural`, authored decls winning), so the tree build
    // below runs unchanged.
    tree::seat_mindmap(&mut user_root, &mut instances);
    // A root `{ layout: tree }` scene builds its own topic tree [SPEC 12], like a
    // node tree does in `lower_node` — before auto-create below. Its gap default
    // rides `root_layout_defaults`, not `ensure_gap`.
    let mut root_branch_links: Vec<Link> = Vec::new();
    if tree::is_tree_scope(&user_root) {
        tree::build_tree(&mut instances, &mut root_branch_links, &user_root);
        // The generated fan prints *after* the instances, so it must sort (and
        // phase-split, in fmt) as if written there — seat its span past the last
        // instance, so `lini desugar` is byte-idempotent from the first pass.
        let end = instances.iter().map(|c| c.span().end).max();
        if let Some(end) = end {
            for (i, l) in root_branch_links.iter_mut().enumerate() {
                l.span = Span {
                    start: end + i,
                    end: end + i,
                };
            }
        }
    }
    // Display refs [SPEC 16.2]: parts read their id as the drawn ref;
    // anonymous ones mint prefix + N, per scope.
    let minted_refs = schematic::mint_refs(&cx, &mut instances)?;
    // A drawing scope never auto-creates [SPEC 15]: an annotation must point at
    // real geometry, so an unknown endpoint stays unknown and errors at resolve.
    if !root_nest.drawing {
        let declared = scene::declared_ids(&instances);
        let mut root_msgs: Vec<&Link> = root_links.iter().collect();
        root_msgs.extend(gather_frame_messages(&instances));
        for (id, span) in
            scene::to_create(&root_msgs, &declared, root_nest.schematic, &minted_refs)?
        {
            instances.push(Child::Box(lower_node(
                &cx,
                &scene::auto_box(&id, span),
                Nest::NONE,
            )?));
        }
    }

    // ── The scale fold [SPEC 15.1/18]: drawing scopes and pages gain their
    //    generated internal `px-per-unit:` from ratio × unit × density. ──
    scale::fold(&mut instances, &mut user_root, root_nest.drawing)?;

    // ── Root links, lowered before the present walk: a carried `[ ]`
    //    annotation node [SPEC 15.9] wears its `.lini-*` chain like any child
    //    and its class defs must emit. ──
    let mut links = Vec::new();
    for w in root_links.iter().chain(&root_branch_links) {
        for hop in split_statement(w, root_nest) {
            links.push(labels::lower_link(&hop, &cx, root_nest)?);
        }
    }

    // ── Present types = every `.lini-X` class worn anywhere. ──
    let mut present: BTreeSet<String> = BTreeSet::new();
    for c in &instances {
        mark_present(c, &mut present);
    }
    for w in &links {
        mark_present_link(w, &mut present);
    }
    // The junction dot is generated at layout from the routed geometry
    // [SPEC 16.5], so no source node ever wears `.lini-junction` — but its one
    // rule must exist for the dots to paint through (and for `|junction| { … }`
    // to reach them). A sheet's *parts* are what makes it live: a meet is only
    // ever at a part's landing, so wherever a dot can appear, a part is present.
    if present
        .iter()
        .any(|t| schematic::schematic_type(std::slice::from_ref(t)).is_some())
    {
        present.insert("junction".to_string());
    }
    // A `|table|`'s header band and its per-column alignment are worn at
    // resolve, from the resolved column count [SPEC 8] — so no source node
    // wears `.lini-header` / `.lini-align-*` here, yet their rules must exist
    // for that pass to have anything to apply (and for `|header| { … }` to
    // reach the band). A table is what makes them live.
    if present.contains("table") {
        present.insert("header".to_string());
        for (name, ..) in classes::ALIGN_CLASSES {
            present.insert(name.to_string());
        }
    }

    // ── Assemble the new stylesheet (a canonical order, so re-desugar is stable):
    //    scene config, vars, the generated `.lini-*` defs, then the user
    //    descendant/class rules. ──
    let mut stylesheet: Vec<StyleItem> = Vec::new();
    // The scene defaults, plus any root-engine defaults (a root `{ layout: sequence }` gets
    // the sequence `gap`), then the user's own decls on top.
    let mut layout_defaults = crate::ledger::defaults::root_layout_defaults(layout_of(&user_root));
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
    // Desugar's output is itself a legal file — `lini desugar` re-renders
    // byte-identically [SPEC 20] — so a default it stamps must be one the
    // root's own engine reads: a drawing root honours no `gap` [SPEC 17], and
    // validation says so. The predicate is the root validator's own.
    let root_layout = layout_of(&user_root).unwrap_or("flow");
    let base: Vec<Decl> = merge_decls(root_defaults(), &layout_defaults)
        .into_iter()
        .filter(|d| {
            crate::ledger::properties::get(&d.name)
                .is_none_or(|p| crate::validate::root_reads(root_layout, p))
        })
        .collect();
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
    // a plain scene synthesizes nothing and skips them [SPEC 18].
    let synthesizes_shapes = ["chart", "pie", "sequence"]
        .iter()
        .any(|t| present.contains(*t))
        || layout_of(&user_root) == Some("sequence");
    for r in class_defs(&present, &element_rules, &extra_order, synthesizes_shapes) {
        stylesheet.push(StyleItem::Rule(r));
    }
    for r in classes::scoped_note_rules(&present, &user_rules) {
        stylesheet.push(StyleItem::Rule(r));
    }
    for r in tree::mindmap_rules(&present, &user_rules) {
        stylesheet.push(StyleItem::Rule(r));
    }
    for r in user_rules {
        stylesheet.push(StyleItem::Rule(r));
    }

    Ok(File {
        stylesheet,
        stylesheet_span: Span::empty(),
        instances,
        links,
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

fn lower_child(cx: &Lower, child: &Child, nest: Nest) -> Result<Child, Error> {
    match child {
        Child::Box(n) => Ok(Child::Box(lower_node(cx, n, nest)?)),
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

fn lower_node(cx: &Lower, node: &Node, nest: Nest) -> Result<Node, Error> {
    let (types, bodies) = (cx.types, cx.bodies);
    let ty = node.ty.as_deref().unwrap_or("box");
    let info = types.resolve(ty, node.span)?;
    let kind = info.kind;

    // Idempotency: a node already at a primitive type and wearing its `.lini-<kind>`
    // class is already lowered — keep its classes and type verbatim (re-prepending
    // worn classes would duplicate them, and a lowered define's `.lini-<name>` is
    // unrecoverable from the now-primitive type).
    let already = NodeKind::parse(ty).is_some()
        && node.classes.iter().any(|c| *c == lini_class(kind.as_str()));

    // An authored id may not begin `lini-` — the prefix is reserved for
    // generated names [SPEC 21/23], mirroring the `.lini-*` class reservation.
    // Only first-lowering nodes are checked: a re-desugared node (`already`)
    // carries the compiler's own minted `lini-topic-N` ids, which must round-trip.
    if !already
        && let Some(id) = &node.id
        && id.starts_with("lini-")
    {
        return Err(Error::at(
            node.span,
            "an id may not begin 'lini-' — the prefix is reserved for generated names",
        )
        .code(Code::RESERVED_ID));
    }

    // The two nested scopes this body opens or inherits. A **lowered** node
    // states its type as a class, not as its `ty` (`|block| .lini-schematic`),
    // so the scope read walks that chain instead — otherwise a re-desugared
    // sheet would answer `false` where the resolve-side gate answers `true`,
    // and the two stages' laws would disagree on the compiler's own output.
    let scope_chain = if already {
        schematic::lowered_chain(node)
    } else {
        info.chain.clone()
    };
    // The drawing scope [SPEC 15]: opened by a drawing node, carried through
    // its parts and their features, sealed by a child that owns its own layout
    // (a |row|, a |table|, a chart — it "lays out as one box", [SPEC 15.1]).
    let is_drawing = is_drawing_body(&info.chain, &node.style);
    // The schematic scope [SPEC 16]: opened by any container the cascade makes
    // one, and sealed one grain later than the drawing — a flow wrapper reads
    // no statement of its own, so the laws reach through it; another engine
    // that reads its own body's statements stops them dead.
    let is_schematic = is_schematic_body(cx, &scope_chain, &node.style);
    let child_nest = Nest {
        drawing: in_drawing_scope(is_drawing, nest.drawing, &info.chain, &node.style),
        schematic: is_schematic
            || (nest.schematic && !seals_schematic_scope(cx, &scope_chain, &node.style)),
    };

    let mut classes = if already {
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

    // This body's statements, gathered raw: the define bodies in the type chain
    // (base→derived) ahead of the node's own children and links — an
    // already-lowered node has no define in its chain, so that half is a no-op
    // there. `gather` then mints, hoists, poses and lowers, in that order
    // [SPEC 16.1/19].
    let mut raw_kids: Vec<Child> = Vec::new();
    let mut raw_links: Vec<Link> = Vec::new();
    if !already {
        for name in &info.chain {
            if let Some((body, body_links)) = bodies.get(name) {
                raw_kids.extend(body.iter().cloned());
                raw_links.extend(body_links.iter().cloned());
            }
        }
    }
    // Where this body's **own** statements begin — the gather hands the index
    // back, because its landing step cuts a chain into hops and moves it.
    let own_links_at = raw_links.len();
    raw_kids.extend(node.children.iter().cloned());
    raw_links.extend(node.links.iter().cloned());
    // The gather takes both readings: the **carrier** (`child_nest`, which
    // reaches) mints this scope's label wires, while the pose chooser turns
    // satellites only when this very container is the schematic — placement
    // never cascades [SPEC 16].
    let scope = gather::Scope::gather(
        cx,
        raw_kids,
        raw_links,
        own_links_at,
        child_nest,
        is_schematic,
    )?;
    let mut children = scope.lower(cx)?;
    // A schematic part [SPEC 16] lowers structurally here — rails, symbol
    // bodies, readouts — dispatched on the chain; Phase 4 adds placement.
    let sch = if already {
        None
    } else {
        schematic::sch_kind(&info.chain)
    };
    // `|J| { pins: N }` [SPEC 16.2]: the generated numbered pins lead the
    // authored children.
    if sch == Some(schematic::SchKind::Component) {
        for (i, pin) in schematic::expand_connector_pins(cx, &info.chain, &node.style)
            .into_iter()
            .enumerate()
        {
            // A generated pin is inside the part, so inside the scope — but
            // never in the drawing one (it grows no chrome of its own).
            let pin_nest = Nest {
                drawing: false,
                schematic: child_nest.schematic,
            };
            children.insert(i, Child::Box(lower_node(cx, &pin, pin_nest)?));
        }
    }
    // The generated chrome [SPEC 15.7] — real children, so the cascade styles
    // or removes them. Only for a node in a drawing scope, and only on first
    // lowering (re-desugar keeps the ones already there).
    if !already && nest.drawing {
        for ch in drawing::chrome_children(node, kind, &info.chain) {
            children.push(Child::Box(lower_node(cx, &ch, Nest::NONE)?));
        }
    }
    // The sheet's furniture [SPEC 15.8]: `sheet:` desugars in place to
    // `width` / `height` in mm first (the zone counts derive from the final
    // numbers), then the pinned chrome children (frame, zone grid, centring
    // marks) are generated, positioned by the layout once the page is sized;
    // a `|title-block|` child is pulled out of the flow here so the page can
    // seat it flush inside the frame's bottom-right corner.
    let is_page = nest::is_page_body(&info.chain);
    let mut page_style: Option<Vec<Decl>> = None;
    if is_page {
        let mut s = node.style.clone();
        page::expand_sheet(&mut s)?;
        page::default_direction(&mut s, node.span);
        page_style = Some(s);
    }
    if !already && is_page {
        for ch in page::chrome_children(page_style.as_deref().expect("a page"), node.span) {
            children.push(Child::Box(lower_node(cx, &ch, Nest::NONE)?));
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

    // Table / entity structure [SPEC 8], the count-free half: wrap every
    // bare-text body cell in a `|cell|` (the box that carries the cell
    // padding). Everything the grid's **column count** decides — the
    // auto-header row, the per-column alignment, an entity's full-width
    // bands — waits for the cascade to settle `columns:` and runs at resolve
    // (`crate::resolve::tables`).
    let is_entity = info.chain.iter().any(|n| n == "entity");
    let is_table = is_entity || info.chain.iter().any(|n| n == "table");
    if is_table {
        wrap_body_cells(cx, &mut children)?;
    }

    let mut style: Vec<Decl> = if let Some(expanded) = page_style {
        expanded
    } else {
        node.style.clone()
    };
    // Opting into the engine is **one** decision [SPEC 16.6]: a container the
    // cascade makes a schematic scope takes the sheet's own track spacing and
    // clearance, whether it wrote `layout: schematic` itself or wears
    // `|schematic|` (whose bundle already states them). The engine's baked
    // constants — pin pitch, stub, seat gap — are tuned to that clearance, so a
    // scope routing at the diagram's 16 would stray every lead it seats. An
    // already-lowered node carries the first pass's answer — in its own block,
    // or in the `.lini-*` rule its define became — and skips, so desugar stays
    // a fixed point.
    //
    // The config lands on the instance's own block — tier 5, which outranks
    // everything — so it may only be added where nothing states it *anywhere
    // the type chain reaches*: the block itself, an element rule, or a
    // define's defaults ([`Cx::chain_decl`], the same reader every other
    // chain-aware property uses). Asking the block alone made a scope's own
    // `|region::group| { gap: 100 }` inert — the injected 60 sat a tier above
    // the define that asked for it.
    if is_schematic && !already && !scope_chain.iter().any(|t| t == "schematic") {
        let owned = |name: &str| cx.chain_decl(&info.chain, &style, name).is_some();
        let add: Vec<Decl> = crate::ledger::defaults::schematic_scope_config()
            .into_iter()
            .filter(|d| !owned(&d.name))
            .collect();
        style.splice(0..0, add);
    }
    // An authored |plane| in a drawing scope is chrome [SPEC 15.8]: its
    // ISO anatomy — thick ends, viewing arrows, the letter — fills from the
    // view's extent at layout, so mark it and layout intercepts it as a
    // placeholder (like the generated chrome types).
    if nest.drawing && info.chain.iter().any(|t| t == "plane") {
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
    // them and the field grid stands as built. Generated cells **lead**:
    // authored children follow as ordinary cells in the same grid, flowing
    // into the remaining slots (or pinned by their own `cell:` / `span:`).
    if is_title_block && titleblock::has_fields(&style) {
        for (i, cell) in titleblock::expand_fields(&mut style, node.span)
            .into_iter()
            .enumerate()
        {
            children.insert(i, Child::Box(lower_node(cx, &cell, Nest::NONE)?));
        }
    }
    // The part's **pose** [SPEC 16.1] — consumed here, before anything it
    // shapes: the value readout reads it for its seat, and the bodies below
    // are built turned, so no text ever rides a paint transform.
    let pose = if sch.is_some() {
        let p = pose::take(cx, &info.chain, &mut style, node.span)?;
        pose::mark(p, &mut classes);
        p
    } else {
        pose::Pose::NONE
    };
    let kept_label = labels::lower_smart(
        cx,
        node,
        label,
        &labels::Smart::read(kind, &info.chain, is_entity, is_drawing, sch, pose),
        &mut style,
        &mut children,
    )?;

    // The schematic bodies [SPEC 16]: rails + per-pin chrome for a
    // component, the registry symbol + wirable ports for a symbol-bodied
    // part, the tag drawing / outline classes for a label.
    if sch.is_some() {
        match sch {
            Some(schematic::SchKind::Component) => {
                schematic::assemble_component(cx, pose, &info.chain, &mut style, &mut children)?;
            }
            Some(k @ (schematic::SchKind::Opamp | schematic::SchKind::Discrete(_))) => {
                schematic::symbol_body(cx, k, pose, &info.chain, node, &mut children)?;
            }
            Some(schematic::SchKind::Label) => {
                schematic::label_body(
                    cx,
                    pose,
                    &info.chain,
                    node,
                    &mut style,
                    &mut classes,
                    &mut children,
                )?;
            }
            None => {}
        }
    }

    // Display refs [SPEC 16.2], per scope — after the gather, so a capsule
    // part reads its declaration.
    let minted_refs = schematic::mint_refs(cx, &mut children)?;
    // The body's links, rewritten by the gather (capsules hoisted, label wires
    // minted), each lowering as before: head label folded into the label list,
    // auto-`along:` filled.
    let mut links = Vec::new();
    for w in &scope.links {
        for hop in split_statement(w, child_nest) {
            links.push(labels::lower_link(&hop, cx, child_nest)?);
        }
    }

    // Seat a stand-alone `|mindmap|` child [SPEC 8]: this body becomes its
    // generated tree scope — never inside a topic, whose body belongs to the
    // enclosing tree (a nested mindmap is an ordinary topic there).
    if !already && !classes.iter().any(|c| c == "lini-topic") {
        tree::seat_mindmap(&mut style, &mut children);
    }
    // Build a `layout: tree` scope [SPEC 12]: mint anonymous topic ids, wear the
    // depth classes, and generate the branch fans — **before** this body's
    // auto-create so a branch / cross-link endpoint sees the topics as declared,
    // and before the paint cascade so the level classes are worn.
    if !already && tree::is_tree_scope(&style) {
        tree::ensure_gap(&mut style);
        tree::build_tree(&mut children, &mut links, &style);
    }

    // Auto-create undeclared body-link endpoints among this body's own children ([SPEC 3] —
    // auto-create runs in any scope, not just the root), counting messages inside any frame
    // child so a participant first named inside a frame is created on the sequence, not the
    // frame. A frame (`loop`/`opt`/`alt`/`else`) opens no scope, so it never auto-creates —
    // its endpoints resolve against the enclosing sequence's participants [SPEC 13]. A
    // drawing body never auto-creates either [SPEC 15]: its links point at real geometry.
    // A schematic scope does not decline but **refuses** — see [`scene::to_create`].
    if !already && !is_frame_classes(&classes) && !is_drawing_body(&info.chain, &node.style) {
        let declared = scene::declared_ids(&children);
        // Scope the message borrows of `children` so the auto-create push below is free.
        // The node's **own** links, post-hoist — define-body links stay out,
        // as before (their ids are the define's own affair).
        let to_create = {
            let mut msgs: Vec<&Link> = scope.own_links().iter().collect();
            msgs.extend(gather_frame_messages(&children));
            scene::to_create(&msgs, &declared, child_nest.schematic, &minted_refs)?
        };
        for (auto_id, auto_span) in to_create {
            let created = lower_node(cx, &scene::auto_box(&auto_id, auto_span), Nest::NONE)?;
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
                    )
                    .code(Code::UNKNOWN_TYPE));
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
        mark_present_node(n, present);
    }
}

fn mark_present_node(n: &Node, present: &mut BTreeSet<String>) {
    for c in &n.classes {
        if let Some(x) = c.strip_prefix("lini-") {
            present.insert(x.to_string());
        }
    }
    for ch in &n.children {
        mark_present(ch, present);
    }
    for w in &n.links {
        mark_present_link(w, present);
    }
}

/// A link's carried `[ ]` annotation nodes wear `.lini-*` chains like any
/// child [SPEC 15.9] — their class defs must emit too.
fn mark_present_link(w: &Link, present: &mut BTreeSet<String>) {
    for n in w.label_nodes() {
        mark_present_node(n, present);
    }
}

/// A statement, as the scope states its wires [SPEC 9/18] — `a -> b -> c` is
/// exactly `a -> b; b -> c`, so an ordinary chain lowers as one link per hop.
///
/// **A schematic scope states its own** [SPEC 16.5]: a chain through a two-pin
/// part is a *series circuit*, not two statements, so the equivalence does not
/// hold there — `schematic::arity` has already cut the chain wherever it
/// resolved that reading (writing the entry and exit pins), and what it left
/// whole it left whole on purpose: only the chain itself still says what a
/// landing it could not resolve means, and that has to survive printing.
fn split_statement(w: &Link, nest: Nest) -> Vec<Link> {
    if nest.schematic {
        return vec![w.clone()];
    }
    labels::split_chain(w)
}

fn push_unique(v: &mut Vec<String>, name: &str) {
    if !v.iter().any(|x| x == name) {
        v.push(name.to_string());
    }
}

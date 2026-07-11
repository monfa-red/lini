//! The owner-aware property validation pass [SPEC 16/20], reading the ledger.
//! Strict where the wearer is statically known, lenient where a class is
//! polymorphic:
//!
//! - an **unknown property name** is an error, everywhere — the message
//!   suggests the nearest name;
//! - a known property **misused where its wearer is statically known** (an
//!   instance's own block, an element / id / descendant rule's tail, the root
//!   block) is an error with a contextual correction;
//! - in a **class rule** a property is inert on wearers that can't use it — it
//!   warns only when it is dead for *every* wearer, and a defined class no one
//!   wears warns too;
//! - a **malformed value** the ledger shape can judge statically (arity,
//!   range) is an error, wearer-independent.
//!
//! The pass runs on the parsed file, before desugar, so it sees exactly what
//! the user wrote; the handful of attr names desugar/layout generate
//! internally are whitelisted for the lowered-form round-trip.

use crate::desugar::types::{self, Types};
use crate::error::Diagnostic;
use crate::ledger::properties::{self, Inherit, Owner, Property, Shape};
use crate::suggest;
use crate::syntax::ast::{
    Child, Decl, Define, File, Link, Node, Rule, SelUnit, StyleItem, TextNode, Value,
};
use std::collections::{HashMap, HashSet};

/// Attr names desugar/layout write internally (view chrome, detail clips, the
/// sourced-view title, mate seating) — never user properties, but present when
/// a lowered file (`lini desugar` output) is compiled back.
const INTERNAL: &[&str] = &["chrome", "clip", "of-title", "mount", "px-per-unit"];

pub fn validate(file: &File) -> Vec<Diagnostic> {
    // A broken type table (cycle, shadowing) is desugar's error to report.
    let Ok(types) = Types::build(file) else {
        return Vec::new();
    };
    let ctx = Ctx::new(file, &types);
    let mut out = Vec::new();

    // The stylesheet: root config, rules, define bodies.
    for item in &file.stylesheet {
        match item {
            StyleItem::RootDecl(d) => ctx.check_decl(d, &Wearer::Root, &mut out),
            StyleItem::Rule(r) => ctx.check_rule(r, &mut out),
            StyleItem::Define(d) => ctx.check_define(d, &mut out),
            StyleItem::Var(_) | StyleItem::Binding(_) => {}
        }
    }
    ctx.check_unworn_classes(file, &mut out);

    // The canvas: every instance block, text style, and link block, with the
    // parent's statically-known layout as context.
    let root_layout = ctx.root_layout.clone();
    for c in &file.instances {
        ctx.check_child(c, Some(root_layout.as_str()), &mut out);
    }
    for w in &file.links {
        ctx.check_link(w, &mut out);
    }
    out
}

/// What a declaration is written on — decides which owners satisfy it.
enum Wearer<'a> {
    /// The scene root (its `layout:` is always statically known).
    Root,
    /// A node with a resolved type: primitive kind name, template/define chain
    /// (base→derived), its own static layout, and the parent's static layout.
    Node {
        /// The written type name, for messages (`'|box|'`).
        shown: &'a str,
        kind: &'a str,
        chain: &'a [String],
        own_layout: Option<&'a str>,
        parent_layout: Option<&'a str>,
    },
    /// A link (`|-|` / `(-)` rules, a link's own block) — polymorphic between
    /// wires, dimensions, and mates, so only name/value checks apply.
    Link,
    /// A bare text leaf — resolve enforces text validity with its own message;
    /// only name/value checks apply here.
    Text,
}

struct Ctx<'a> {
    types: &'a Types,
    /// Define name → its own style decls, for chain-walking static layouts.
    define_styles: HashMap<&'a str, &'a [Decl]>,
    /// Whether any stylesheet rule sets `layout:` — if so, a node's layout can
    /// come from a class/id/element rule and is never statically known.
    rules_set_layout: bool,
    root_layout: String,
}

impl<'a> Ctx<'a> {
    fn new(file: &'a File, types: &'a Types) -> Self {
        let define_styles = file
            .stylesheet
            .iter()
            .filter_map(|it| match it {
                StyleItem::Define(d) => Some((d.name.as_str(), d.style.as_slice())),
                _ => None,
            })
            .collect();
        let rules_set_layout = file.stylesheet.iter().any(
            |it| matches!(it, StyleItem::Rule(r) if r.decls.iter().any(|d| d.name == "layout")),
        );
        let root_layout = file
            .stylesheet
            .iter()
            .find_map(|it| match it {
                StyleItem::RootDecl(d) if d.name == "layout" => decl_ident(d),
                _ => None,
            })
            .unwrap_or("flow")
            .to_string();
        Self {
            types,
            define_styles,
            rules_set_layout,
            root_layout,
        }
    }

    // ── The canvas walk ──

    fn check_child(&self, child: &Child, parent_layout: Option<&str>, out: &mut Vec<Diagnostic>) {
        match child {
            Child::Text(t) => self.check_text(t, out),
            Child::Box(n) => self.check_node(n, parent_layout, out),
        }
    }

    fn check_node(&self, n: &Node, parent_layout: Option<&str>, out: &mut Vec<Diagnostic>) {
        let ty = n.ty.as_deref().unwrap_or("box");
        let info = self.types.resolve(ty, n.span).ok();
        let own_layout = self.static_layout(n, info.as_ref());
        if let Some(info) = &info {
            // A lowered file (`lini desugar` output) carries its type chain as
            // worn `.lini-*` classes — fold them in, so the round-trip
            // validates like the sugar it came from.
            let chain = with_worn_types(&info.chain, &n.classes);
            let wearer = Wearer::Node {
                shown: ty,
                kind: info.kind.as_str(),
                chain: &chain,
                own_layout: own_layout.as_deref(),
                parent_layout,
            };
            for d in &n.style {
                self.check_decl(d, &wearer, out);
            }
        }
        if let Some(label) = &n.label {
            self.check_text(label, out);
        }
        for c in &n.children {
            self.check_child(c, own_layout.as_deref(), out);
        }
        for w in &n.links {
            self.check_link(w, out);
        }
    }

    fn check_text(&self, t: &TextNode, out: &mut Vec<Diagnostic>) {
        for d in &t.style {
            self.check_decl(d, &Wearer::Text, out);
        }
    }

    fn check_link(&self, w: &Link, out: &mut Vec<Diagnostic>) {
        for d in &w.style {
            self.check_decl(d, &Wearer::Link, out);
        }
        for label in &w.labels {
            self.check_text(label, out);
        }
    }

    // ── The stylesheet walk ──

    fn check_rule(&self, r: &Rule, out: &mut Vec<Diagnostic>) {
        let wearer = match r.selector.units.last() {
            Some(SelUnit::Type { name, .. }) => match self.types.resolve(name, r.span).ok() {
                Some(info) => Some((info.kind.as_str().to_string(), info.chain)),
                None => None, // unknown type — desugar's error
            },
            Some(SelUnit::Link | SelUnit::Dimension) => {
                for d in &r.decls {
                    self.check_decl(d, &Wearer::Link, out);
                }
                return;
            }
            // A class rule is judged wearer-set-wide in `check_unworn_classes`;
            // an id rule's node is checked where it is declared (the instance
            // block) — here both get the wearer-independent checks.
            Some(SelUnit::Class(_) | SelUnit::Id(_)) | None => None,
        };
        let shown = match r.selector.units.last() {
            Some(SelUnit::Type { name, .. }) => name.as_str(),
            _ => "",
        };
        match wearer {
            Some((kind, chain)) => {
                let wearer = Wearer::Node {
                    shown,
                    kind: &kind,
                    chain: &chain,
                    own_layout: None,
                    parent_layout: None,
                };
                for d in &r.decls {
                    self.check_decl(d, &wearer, out);
                }
            }
            None => {
                for d in &r.decls {
                    self.check_decl(d, &Wearer::Text, out); // name + value checks only
                }
            }
        }
    }

    fn check_define(&self, def: &Define, out: &mut Vec<Diagnostic>) {
        if let Ok(info) = self.types.resolve(&def.name, def.span) {
            let wearer = Wearer::Node {
                shown: &def.name,
                kind: info.kind.as_str(),
                chain: &info.chain,
                own_layout: None,
                parent_layout: None,
            };
            for d in &def.style {
                self.check_decl(d, &wearer, out);
            }
        }
        for c in &def.children {
            self.check_child(c, None, out);
        }
        for w in &def.links {
            self.check_link(w, out);
        }
    }

    // ── One declaration ──

    fn check_decl(&self, d: &Decl, wearer: &Wearer, out: &mut Vec<Diagnostic>) {
        if INTERNAL.contains(&d.name.as_str()) {
            return;
        }
        let Some(prop) = properties::get(&d.name) else {
            let near = suggest::nearest(&d.name, properties::PROPERTIES.iter().map(|p| p.name), 1);
            out.push(Diagnostic::error(
                d.span,
                format!(
                    "unknown property '{}'{}",
                    d.name,
                    suggest::did_you_mean(&near)
                ),
            ));
            return;
        };
        self.check_value(d, prop, out);
        let Wearer::Node {
            shown,
            kind,
            chain,
            own_layout,
            parent_layout,
        } = wearer
        else {
            if matches!(wearer, Wearer::Root) {
                self.check_root_decl(d, prop, out);
            }
            return;
        };
        if !node_accepts(prop, kind, chain, *own_layout) {
            out.push(Diagnostic::error(
                d.span,
                misuse_message(&d.name, shown, prop),
            ));
            return;
        }
        // Layout-owned placement props, gated on the statically-known context
        // [SPEC 16]: `cell`/`span` need a grid parent; the sequence placement
        // props need a sequence [SPEC 20].
        match d.name.as_str() {
            "cell" | "span" => {
                let is_band = chain.iter().any(|c| c == "band");
                if let Some(parent) = parent_layout
                    && *parent != "grid"
                    && !(d.name == "span" && is_band)
                {
                    let verb = if d.name == "cell" {
                        "places a grid child"
                    } else {
                        "spans grid tracks"
                    };
                    out.push(Diagnostic::error(
                        d.span,
                        format!(
                            "'{}' {verb} — this box sits in a 'layout: {parent}'",
                            d.name
                        ),
                    ));
                }
            }
            "place" => {
                if let Some(parent) = parent_layout
                    && *parent != "sequence"
                {
                    out.push(Diagnostic::error(
                        d.span,
                        "'place' is valid only in a 'layout: sequence'",
                    ));
                }
            }
            "activation" => {
                if let Some(own) = own_layout
                    && *own != "sequence"
                {
                    out.push(Diagnostic::error(
                        d.span,
                        "'activation' is valid only in a 'layout: sequence'",
                    ));
                }
            }
            _ => {}
        }
    }

    /// Root-block misuse: the root accepts scene config (universal, root,
    /// layout-owned for its own layout) — never a type-/role-owned property.
    fn check_root_decl(&self, d: &Decl, prop: &Property, out: &mut Vec<Diagnostic>) {
        if prop.inherit != Inherit::No {
            return;
        }
        let ok = prop.owners.iter().any(|o| match o {
            Owner::Universal | Owner::Root | Owner::Layout(_) => true,
            Owner::Link => false,
            Owner::Type(t) => container_layout(t) == Some(self.root_layout.as_str()),
            Owner::Role(_) => false,
        });
        if !ok {
            out.push(Diagnostic::error(
                d.span,
                misuse_message(&d.name, "the root block", prop),
            ));
        }
    }

    // ── Value shapes the ledger can judge statically [SPEC 20] ──

    fn check_value(&self, d: &Decl, prop: &Property, out: &mut Vec<Diagnostic>) {
        if matches!(prop.shape, Shape::One(_)) && d.groups.len() > 1 {
            out.push(Diagnostic::error(
                d.span,
                format!("'{}' takes one value, not a comma list", d.name),
            ));
        }
        match d.name.as_str() {
            "opacity" => {
                if let Some(Value::Number(n)) = single_value(d)
                    && !(0.0..=1.0).contains(n)
                {
                    out.push(Diagnostic::error(d.span, "'opacity' is a fraction 0..1"));
                }
            }
            "translate" => {
                // `translate: x y` — flag a bare scalar or a longer run; a
                // single `(…)` group may fold to a point, so it passes.
                let bad = match d.groups.first().map(Vec::as_slice) {
                    Some([Value::Number(_)]) => true,
                    Some(g) if g.len() > 2 => true,
                    _ => false,
                };
                if bad {
                    out.push(Diagnostic::error(d.span, "'translate' takes 'x y'"));
                }
            }
            _ => {}
        }
    }

    // ── Class rules: wearer-set-wide judgment [SPEC 16] ──

    fn check_unworn_classes(&self, file: &File, out: &mut Vec<Diagnostic>) {
        let mut node_wearers: HashMap<&str, Vec<(String, Vec<String>)>> = HashMap::new();
        let mut link_wearers: HashSet<&str> = HashSet::new();
        collect_wearers(file, self.types, &mut node_wearers, &mut link_wearers);

        for item in &file.stylesheet {
            let StyleItem::Rule(r) = item else { continue };
            let [SelUnit::Class(name)] = r.selector.units.as_slice() else {
                continue;
            };
            // Generated `.lini-*` classes (a lowered file's type bundles) are
            // worn implicitly at resolve — the compiler's, not the user's.
            if name.starts_with("lini-") {
                continue;
            }
            let nodes = node_wearers.get(name.as_str());
            let on_links = link_wearers.contains(name.as_str());
            if nodes.is_none() && !on_links {
                out.push(Diagnostic::warn(
                    r.span,
                    format!("class '.{name}' is never worn"),
                ));
                continue;
            }
            // CSS semantics: a property inert on one wearer is fine; dead on
            // every wearer it warns.
            for d in &r.decls {
                let Some(prop) = properties::get(&d.name) else {
                    continue; // unknown-name already reported
                };
                let node_ok = nodes.is_some_and(|ws| {
                    ws.iter()
                        .any(|(kind, chain)| node_accepts(prop, kind, chain, None))
                });
                let link_ok = on_links && link_accepts(prop);
                if !node_ok && !link_ok {
                    out.push(Diagnostic::warn(
                        d.span,
                        format!("'.{name} {{ {}: … }}' is inert on every wearer", d.name),
                    ));
                }
            }
        }
    }

    /// A node's statically-known layout: its own `layout:` decl; else — when no
    /// stylesheet rule can inject one — the nearest layout default in its
    /// define/template chain; else `flow`. `None` when it can't be known.
    fn static_layout(&self, n: &Node, info: Option<&types::TypeInfo>) -> Option<String> {
        if let Some(l) = n.style.iter().find(|d| d.name == "layout") {
            return decl_ident(l).map(str::to_string);
        }
        if self.rules_set_layout {
            return None;
        }
        let info = info?;
        for name in info.chain.iter().rev() {
            if let Some(style) = self.define_styles.get(name.as_str())
                && let Some(l) = style.iter().find(|d| d.name == "layout")
            {
                return decl_ident(l).map(str::to_string);
            }
            if let Some(l) = container_layout(name) {
                return Some(l.to_string());
            }
        }
        Some("flow".to_string())
    }
}

/// The layout a built-in container type owns [SPEC 8] — how a `Type` owner is
/// satisfied by a scope whose `layout:` matches it.
fn container_layout(t: &str) -> Option<&'static str> {
    Some(match t {
        "drawing" => "drawing",
        "chart" => "chart",
        "pie" => "pie",
        "sequence" => "sequence",
        "table" | "entity" | "title-block" | "grid" => "grid",
        _ => return None,
    })
}

/// Whether a node wearer can use the property at all [SPEC 16].
fn node_accepts(prop: &Property, kind: &str, chain: &[String], own_layout: Option<&str>) -> bool {
    // The inheriting channels reach every node (text props, scope link config).
    if prop.inherit != Inherit::No {
        return true;
    }
    prop.owners.iter().any(|o| match o {
        Owner::Universal => true,
        Owner::Root | Owner::Link => false,
        // Layout-owned properties read on any container (its layout may be
        // set later in the cascade); `cell`/`span` gate on the parent instead.
        Owner::Layout(_) => true,
        // A container type's own properties also read on a scope whose
        // `layout:` is that type's layout (`{ layout: drawing; unit: "mm" }`).
        Owner::Type(t) => {
            *t == kind
                || chain.iter().any(|c| c == t)
                || (own_layout.is_some() && container_layout(t) == own_layout)
        }
        Owner::Role(r) => role_accepts(r, kind, chain),
    })
}

/// Whether a link wearer can use the property (a class's link side).
fn link_accepts(prop: &Property) -> bool {
    if prop.inherit != Inherit::No {
        return true;
    }
    prop.owners.iter().any(|o| match o {
        Owner::Link => true,
        Owner::Role("dimension" | "mate") => true,
        // Links are styled with the node paint vocabulary [SPEC 9].
        Owner::Universal => true,
        _ => false,
    })
}

fn role_accepts(role: &str, kind: &str, chain: &[String]) -> bool {
    let in_chain = |names: &[&str]| {
        names
            .iter()
            .any(|n| *n == kind || chain.iter().any(|c| c == n))
    };
    match role {
        "series" => in_chain(&["line", "bars", "area", "dots", "bubble"]),
        "title-block" => in_chain(&["title-block"]),
        // Closed shapes [SPEC 7]: everything that has a body to duplicate.
        "closed" => !in_chain(&["line", "image"]),
        // Dimensions and mates are links, never nodes.
        "dimension" | "mate" => false,
        _ => false,
    }
}

/// The contextual correction for a misused property [SPEC 20]: where it *does*
/// read, phrased per owner kind.
fn misuse_message(name: &str, wearer: &str, prop: &Property) -> String {
    if name == "density" {
        return "'density' is scene config — set it in the root block".to_string();
    }
    let mut homes: Vec<String> = Vec::new();
    for o in prop.owners {
        let home = match o {
            Owner::Type(t) => format!("'|{t}|'"),
            Owner::Role("series") => "a chart series".to_string(),
            Owner::Role("dimension") => "a '(-)' dimension".to_string(),
            Owner::Role("mate") => "a '||' mate".to_string(),
            Owner::Role("title-block") => "the '|title-block|' fields".to_string(),
            Owner::Role("closed") => "closed shapes".to_string(),
            Owner::Role(r) => format!("'{r}'"),
            Owner::Link => "links".to_string(),
            Owner::Layout(l) => format!("a 'layout: {l}'"),
            Owner::Root => "the root block".to_string(),
            Owner::Universal => continue,
        };
        if !homes.contains(&home) {
            homes.push(home);
        }
    }
    let wearer = if wearer.starts_with("the ") {
        wearer.to_string()
    } else {
        format!("'|{wearer}|'")
    };
    format!(
        "'{name}' has no meaning on {wearer} — it reads on {}",
        homes.join(" / ")
    )
}

fn collect_wearers<'a>(
    file: &'a File,
    types: &Types,
    nodes: &mut HashMap<&'a str, Vec<(String, Vec<String>)>>,
    links: &mut HashSet<&'a str>,
) {
    fn walk_children<'a>(
        children: &'a [Child],
        child_links: &'a [Link],
        types: &Types,
        nodes: &mut HashMap<&'a str, Vec<(String, Vec<String>)>>,
        links: &mut HashSet<&'a str>,
    ) {
        for c in children {
            let Child::Box(n) = c else { continue };
            if !n.classes.is_empty()
                && let Ok(info) = self_resolve(types, n)
            {
                let chain = with_worn_types(&info.1, &n.classes);
                for class in &n.classes {
                    nodes
                        .entry(class.as_str())
                        .or_default()
                        .push((info.0.clone(), chain.clone()));
                }
            }
            walk_children(&n.children, &n.links, types, nodes, links);
        }
        for w in child_links {
            for class in &w.classes {
                links.insert(class.as_str());
            }
        }
    }
    fn self_resolve(types: &Types, n: &Node) -> Result<(String, Vec<String>), ()> {
        let ty = n.ty.as_deref().unwrap_or("box");
        types
            .resolve(ty, n.span)
            .map(|i| (i.kind.as_str().to_string(), i.chain))
            .map_err(|_| ())
    }
    walk_children(&file.instances, &file.links, types, nodes, links);
    for item in &file.stylesheet {
        if let StyleItem::Define(d) = item {
            walk_children(&d.children, &d.links, types, nodes, links);
        }
    }
}

/// The chain plus any worn `.lini-<type>` classes' names — how a lowered
/// (`lini desugar`) instance still reads as its sugared type.
fn with_worn_types(chain: &[String], classes: &[String]) -> Vec<String> {
    let mut out = chain.to_vec();
    for c in classes {
        if let Some(name) = c.strip_prefix("lini-")
            && !out.iter().any(|n| n == name)
        {
            out.push(name.to_string());
        }
    }
    out
}

fn decl_ident(d: &Decl) -> Option<&str> {
    match d.groups.first().and_then(|g| g.first()) {
        Some(Value::Ident(s)) => Some(s),
        _ => None,
    }
}

fn single_value(d: &Decl) -> Option<&Value> {
    match d.groups.as_slice() {
        [group] => match group.as_slice() {
            [v] => Some(v),
            _ => None,
        },
        _ => None,
    }
}

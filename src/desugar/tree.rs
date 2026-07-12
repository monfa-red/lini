//! Tree lowering [SPEC 12]: a `layout: tree` scope authors a rooted hierarchy
//! with `|topic|` nesting; this pass keeps that nesting and adds the generated
//! branch links + depth classes.
//!
//! Topic nesting is preserved through the whole front half (scoped ids, sealed
//! bodies, dot-paths all stay true to SPEC 9). For each topic that has
//! topic-children, one unmarked `|-|` **fan** is generated **in the scope that
//! contains the parent topic** — the root topic's fan in the tree scope
//! (`ceo:bottom - ceo.cto:top & ceo.coo:top`), a deeper topic's fan inside its
//! parent's body (`cto:bottom - cto.backend:top & cto.frontend:top`) — with the
//! direction's forced sides (`column`: `bottom` → `top`; `row`: `right` →
//! `left`). The endpoints are dotted from the fan's own scope so they resolve by
//! the ordinary path-walk. Every topic also wears its depth class
//! `.lini-level-N` (root 0), and an anonymous topic gets a deterministic minted
//! id `lini-topic-N` (1-based among its scope's topic children).
//!
//! The pass is a fixed point: an already-levelled scope regenerates nothing, and
//! a fan the scope already carries is not duplicated.

use super::classes::lini_class;
use super::types::Types;
use crate::ast::{ChainOp, LineStyle, LinkMarker, LinkOp};
use crate::error::Error;
use crate::span::Span;
use crate::syntax::ast::{
    Child, Decl, Endpoint, EndpointGroup, File, Link, Node, PointRef, StyleItem, Value,
};

/// A tree's growth axis [SPEC 12] — only `row`/`column` drive Stage 1; a
/// `bilateral` value flows through as `column` here (the split is Stage 2).
#[derive(Clone, Copy)]
enum Growth {
    Column,
    Row,
}

impl Growth {
    /// The forced `(parent, child)` sides the branch link wears [SPEC 12].
    fn sides(self) -> (&'static str, &'static str) {
        match self {
            Growth::Column => ("bottom", "top"),
            Growth::Row => ("right", "left"),
        }
    }
}

fn growth_of(style: &[Decl]) -> Growth {
    match style
        .iter()
        .rev()
        .find(|d| d.name == "direction")
        .and_then(decl_ident)
    {
        Some("row") => Growth::Row,
        _ => Growth::Column,
    }
}

// ───────────────────────── structure validation ─────────────────────────

/// Structure errors [SPEC 20], on the parsed (still-nested) AST. Runs
/// pre-desugar, the `lint::is_drawing_node` precedent (written type + style, no
/// cascade).
pub(crate) fn validate(file: &File) -> Result<(), Error> {
    // A broken type table is desugar's error to report; skip here.
    let Ok(types) = Types::build(file) else {
        return Ok(());
    };
    let root_tree = matches!(root_layout(&file.stylesheet), Some(l) if l == "tree");
    if root_tree {
        check_root_count(&file.instances, Span::empty())?;
    }
    for c in &file.instances {
        check(c, root_tree, &types)?;
    }
    Ok(())
}

fn check(child: &Child, in_tree: bool, types: &Types) -> Result<(), Error> {
    let Child::Box(n) = child else {
        return Ok(());
    };
    let topic = is_topic_ast(n, types);
    if topic && !in_tree {
        return Err(Error::at(
            n.span,
            "'|topic|' builds a tree — it belongs in a 'layout: tree'",
        ));
    }
    let tree = is_tree_style(&n.style);
    if tree {
        check_root_count(&n.children, n.span)?;
    }
    let child_in_tree = tree || (topic && in_tree);
    for c in &n.children {
        check(c, child_in_tree, types)?;
    }
    Ok(())
}

/// A tree scope holds exactly one root topic [SPEC 20]: none or a second errors.
/// Counts *root* topics — direct topic children of the scope.
fn check_root_count(children: &[Child], scope_span: Span) -> Result<(), Error> {
    let mut roots = children.iter().filter_map(|c| match c {
        Child::Box(n)
            if (n.classes.iter().any(|k| k == "lini-topic") || is_topic_node(n))
                && !wears_deeper_level(n) =>
        {
            Some(n)
        }
        _ => None,
    });
    if roots.next().is_none() {
        return Err(Error::at(
            scope_span,
            "a tree needs exactly one root '|topic|'",
        ));
    }
    if let Some(second) = roots.next() {
        let label = second
            .label
            .as_ref()
            .map(|l| l.text.as_str())
            .unwrap_or_default();
        return Err(Error::at(
            second.span,
            format!("a tree has one root — '|topic|' '{label}' is a second"),
        ));
    }
    Ok(())
}

/// Whether a (re-desugared) topic wears a level class deeper than 0 — never true
/// for a direct child of the tree scope, kept as a guard for the lowered form.
fn wears_deeper_level(n: &Node) -> bool {
    n.classes
        .iter()
        .any(|c| matches!(c.strip_prefix("lini-level-"), Some(d) if d != "0"))
}

/// Whether the node's written type resolves to a topic-derived chain — the
/// pre-desugar test (`|person::topic|` counts).
fn is_topic_ast(n: &Node, types: &Types) -> bool {
    let ty = n.ty.as_deref().unwrap_or("box");
    types
        .resolve(ty, n.span)
        .map(|i| i.chain.iter().any(|c| c == "topic"))
        .unwrap_or(false)
}

/// The bare-name test for the root count (no `Types` in scope): a written
/// `|topic|`, or a `|x::topic|` whose base is spelled `topic`.
fn is_topic_node(n: &Node) -> bool {
    matches!(n.ty.as_deref(), Some(t) if t == "topic" || t.ends_with("::topic"))
}

// ───────────────────────── build the tree ─────────────────────────

/// Whether a scope's style opens a tree [SPEC 12] — the gate `lower_node` reads
/// and the desugar root reads for the scene scope.
pub(crate) fn is_tree_scope(style: &[Decl]) -> bool {
    is_tree_style(style)
}

/// Build a `layout: tree` scope: mint ids for anonymous topics, wear each topic
/// its depth class, and generate one branch fan per parent into the scope that
/// contains the parent [SPEC 12]. Called from `lower_node` (a node tree) and the
/// desugar root (a scene tree) on the lowered, still-nested topics. Idempotent:
/// an already-levelled scope regenerates nothing.
pub(crate) fn build_tree(children: &mut [Child], links: &mut Vec<Link>, style: &[Decl]) {
    let already = children
        .iter()
        .any(|c| matches!(c, Child::Box(n) if is_topic(n) && has_level_class(n)));
    if already {
        return;
    }
    build_scope(children, links, 0, growth_of(style));
}

/// For one scope (its direct `children` and its own `links`): mint anonymous
/// topic ids, stamp each topic's level class, generate the fan for every topic
/// that has topic-children into `links`, then recurse into each topic's body
/// (its children a generation deeper, their fans landing in the topic's links).
fn build_scope(children: &mut [Child], links: &mut Vec<Link>, level: usize, g: Growth) {
    mint_ids(children);
    let mut fans: Vec<Link> = Vec::new();
    for c in children.iter_mut() {
        let Child::Box(t) = c else { continue };
        if !is_topic(t) {
            continue;
        }
        t.classes.push(lini_class(&format!("level-{level}")));
        // Mint the children now so the fan can name them (the recursion below
        // re-mints them idempotently).
        mint_ids(&mut t.children);
        let kids: Vec<String> = t
            .children
            .iter()
            .filter_map(|cc| match cc {
                Child::Box(k) if is_topic(k) => k.id.clone(),
                _ => None,
            })
            .collect();
        if let Some(pid) = t.id.clone()
            && !kids.is_empty()
        {
            let link = branch_fan(&pid, &kids, g, t.span);
            if !links.iter().chain(fans.iter()).any(|l| same_link(l, &link)) {
                fans.push(link);
            }
        }
        build_scope(&mut t.children, &mut t.links, level + 1, g);
    }
    links.extend(fans);
    // Order by span so the lowered order matches the fmt-printed (span-sorted)
    // desugar output — desugar transparency; the router's declaration-order
    // tie-break is the same whether source or its lowering is compiled.
    links.sort_by_key(|l| l.span.start);
}

/// Mint deterministic `lini-topic-N` ids for a scope's anonymous topic children
/// (1-based among the scope's topics) [SPEC 12]. Idempotent: an already-id'd
/// topic keeps its id.
fn mint_ids(children: &mut [Child]) {
    let mut nth = 0usize;
    for c in children.iter_mut() {
        if let Child::Box(t) = c
            && is_topic(t)
        {
            nth += 1;
            if t.id.is_none() {
                t.id = Some(format!("lini-topic-{nth}"));
            }
        }
    }
}

/// One unmarked `|-|` branch fan `parent:side - parent.c1:side & parent.c2:side …`
/// [SPEC 12], generated in the scope that contains `parent`: the parent is a bare
/// sibling id there, each child a dotted path into it. A single statement so the
/// children share the parent's port — the classic single-trunk tree connector. It
/// carries the parent's span so the lowered link order is stable across the
/// desugar round-trip.
fn branch_fan(parent: &str, children: &[String], g: Growth, span: Span) -> Link {
    let (ps, cs) = g.sides();
    Link {
        chain: vec![
            EndpointGroup {
                endpoints: vec![endpoint(vec![parent.to_string()], ps)],
            },
            EndpointGroup {
                endpoints: children
                    .iter()
                    .map(|c| endpoint(vec![parent.to_string(), c.clone()], cs))
                    .collect(),
            },
        ],
        ops: vec![ChainOp::Wire(LinkOp {
            line: LineStyle::Solid,
            start: LinkMarker::None,
            end: LinkMarker::None,
        })],
        classes: Vec::new(),
        style: Vec::new(),
        style_span: None,
        label: None,
        labels: Vec::new(),
        span,
    }
}

fn endpoint(path: Vec<String>, side: &str) -> Endpoint {
    Endpoint {
        path,
        point: Some(PointRef {
            name: side.to_string(),
            span: Span::empty(),
        }),
        span: Span::empty(),
    }
}

/// Structural equality of two links — the fixed-point guard: same ops, same
/// endpoint paths and `:point` names.
fn same_link(a: &Link, b: &Link) -> bool {
    a.ops == b.ops
        && a.chain.len() == b.chain.len()
        && a.chain.iter().zip(&b.chain).all(|(x, y)| {
            x.endpoints.len() == y.endpoints.len()
                && x.endpoints.iter().zip(&y.endpoints).all(|(p, q)| {
                    p.path == q.path
                        && p.point.as_ref().map(|r| &r.name) == q.point.as_ref().map(|r| &r.name)
                })
        })
}

// ───────────────────────── shared helpers ─────────────────────────

fn is_topic(n: &Node) -> bool {
    n.classes.iter().any(|c| c == "lini-topic")
}

fn has_level_class(n: &Node) -> bool {
    n.classes.iter().any(|c| c.starts_with("lini-level-"))
}

/// Inject the tree's default `gap` (generation, sibling) when the scope authors
/// none — the generic `20` is too tight for the branch connectors [SPEC 12].
pub(crate) fn ensure_gap(style: &mut Vec<Decl>) {
    if style.iter().any(|d| d.name == "gap") {
        return;
    }
    style.push(Decl {
        name: "gap".into(),
        groups: vec![vec![
            Value::Number(crate::ledger::defaults::TREE_GAP_GEN),
            Value::Number(crate::ledger::defaults::TREE_GAP_SIB),
        ]],
        span: Span::empty(),
    });
}

fn is_tree_style(style: &[Decl]) -> bool {
    style
        .iter()
        .any(|d| d.name == "layout" && decl_ident(d) == Some("tree"))
}

fn decl_ident(d: &Decl) -> Option<&str> {
    match d.groups.first().and_then(|g| g.first()) {
        Some(Value::Ident(s)) => Some(s),
        _ => None,
    }
}

fn root_layout(stylesheet: &[StyleItem]) -> Option<&str> {
    stylesheet.iter().find_map(|it| match it {
        StyleItem::RootDecl(d) if d.name == "layout" => decl_ident(d),
        _ => None,
    })
}

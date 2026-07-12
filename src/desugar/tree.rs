//! Tree lowering [SPEC 12]: the structure a `layout: tree` scope authors with
//! `|topic|` nesting becomes a **flat** set of topics + generated branch links.
//!
//! Authoring nests topics (a direct `|topic|`-derived child is a branch); this
//! pass **flattens** each tree scope's topic subtree into a pre-order list of
//! direct children, wears each topic its depth class `.lini-level-N` (root 0),
//! and generates one unmarked `|-|` branch link per parent→child edge with the
//! direction's forced sides (`column`: `bottom` → `top`; `row`: `right` →
//! `left`). Flattening makes the branch links **ordinary sibling wires** the
//! orthogonal router routes in the scope's world — a nested card never contains
//! its child, so string-path containment must not imply it. The tree engine
//! reconstructs the hierarchy from the flat list's level classes + source order
//! ([`crate::layout::tree`]).
//!
//! The pass is a fixed point: a re-desugared (already-flat, already-levelled)
//! scope is left untouched, and a branch link the scope already carries is not
//! regenerated.

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

/// Structure errors [SPEC 20], on the parsed (still-nested) AST — before the
/// flatten below erases the nesting the root count reads. Runs pre-desugar, the
/// `lint::is_drawing_node` precedent (written type + style, no cascade).
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
/// Counts *root* topics — direct topic children not wearing a deeper level class
/// — so it reads the same on a re-desugared (already-flattened) scope, where
/// every topic is a direct child but only one is level 0.
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

/// Whether a (re-desugared) topic wears a level class deeper than 0 — a lifted
/// branch, not a root.
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

// ───────────────────────── flatten + generate ─────────────────────────

/// Whether a scope's style opens a tree [SPEC 12] — the gate `lower_node` reads
/// to flatten a node scope and the desugar root reads for the scene scope.
pub(crate) fn is_tree_scope(style: &[Decl]) -> bool {
    is_tree_style(style)
}

/// Flatten a `layout: tree` scope's topic subtree into a depth-classed pre-order
/// flat list, appending one branch link per edge to `links` [SPEC 12]. Called
/// from `lower_node` (a node tree) and the desugar root (a scene tree), **before
/// that scope's auto-create**, so a branch / cross-link endpoint sees the lifted
/// topics as declared siblings. Idempotent: an already-flat, already-levelled
/// scope is left as-is, and a branch link the scope already carries is not
/// regenerated.
pub(crate) fn flatten(children: &mut Vec<Child>, links: &mut Vec<Link>, style: &[Decl]) {
    flatten_scope(children, links, growth_of(style));
}

/// Replace a tree scope's root-topic subtree with a pre-order flat list of
/// topics, each depth-classed, and append one branch link per edge to `links`.
/// Idempotent: an already-flattened scope (its topics wear level classes) is
/// left as-is.
fn flatten_scope(children: &mut Vec<Child>, links: &mut Vec<Link>, g: Growth) {
    let already = children
        .iter()
        .any(|c| matches!(c, Child::Box(n) if has_level_class(n)));
    if already {
        return;
    }
    let mut flat: Vec<Node> = Vec::new();
    let mut kept: Vec<Child> = Vec::new();
    for c in std::mem::take(children) {
        match c {
            Child::Box(n) if is_topic(&n) => flatten_topic(n, 0, links, g, &mut flat),
            other => kept.push(other),
        }
    }
    kept.extend(flat.into_iter().map(Child::Box));
    *children = kept;
    // Order the scope's links by span so the lowered order matches the
    // fmt-printed (span-sorted) desugar output — desugar transparency, so the
    // router's declaration-order tie-break is the same whether the source or its
    // lowering is compiled. Stable: branch fans carry their parent's span
    // (distinct, source order), authored links their own.
    links.sort_by_key(|l| l.span.start);
}

/// Depth-class `topic`, split off its topic children (its content stays), emit
/// one branch **fan** for its edges, then recurse pre-order.
fn flatten_topic(
    mut topic: Node,
    level: usize,
    links: &mut Vec<Link>,
    g: Growth,
    flat: &mut Vec<Node>,
) {
    topic.classes.push(lini_class(&format!("level-{level}")));
    let mut branches: Vec<Node> = Vec::new();
    let mut content: Vec<Child> = Vec::new();
    for c in std::mem::take(&mut topic.children) {
        match c {
            Child::Box(n) if is_topic(&n) => branches.push(n),
            other => content.push(other),
        }
    }
    topic.children = content;
    if let Some(pid) = topic.id.clone() {
        let child_ids: Vec<String> = branches.iter().filter_map(|c| c.id.clone()).collect();
        if !child_ids.is_empty() {
            let link = branch_fan(&pid, &child_ids, g, topic.span);
            if !links.iter().any(|l| same_link(l, &link)) {
                links.push(link);
            }
        }
    }
    flat.push(topic);
    for ch in branches {
        flatten_topic(ch, level + 1, links, g, flat);
    }
}

/// One unmarked `|-|` branch fan `parent:side - c1:side & c2:side …` [SPEC 12].
/// A single statement so the children share the parent's port — the classic
/// single-trunk tree connector, and the only form that lands on a narrow side
/// (a row tree's parent right edge). It carries the parent's span so the lowered
/// link order is stable across the desugar round-trip.
fn branch_fan(parent: &str, children: &[String], g: Growth, span: Span) -> Link {
    let (ps, cs) = g.sides();
    Link {
        chain: vec![
            EndpointGroup {
                endpoints: vec![endpoint(parent, ps)],
            },
            EndpointGroup {
                endpoints: children.iter().map(|c| endpoint(c, cs)).collect(),
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

fn endpoint(id: &str, side: &str) -> Endpoint {
    Endpoint {
        path: vec![id.to_string()],
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

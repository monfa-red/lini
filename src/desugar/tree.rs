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

/// A branch's outward growth [SPEC 12] — the direction its fan leaves the
/// parent. `column`/`row` are whole-tree; a `bilateral` tree splits into a
/// right half (`BiRight`, growing rightward like `row`) and a mirrored left
/// half (`BiLeft`).
#[derive(Clone, Copy)]
enum Growth {
    Column,
    Row,
    BiRight,
    BiLeft,
}

impl Growth {
    /// The forced `(parent, child)` sides the branch link wears [SPEC 12].
    fn sides(self) -> (&'static str, &'static str) {
        match self {
            Growth::Column => ("bottom", "top"),
            Growth::Row | Growth::BiRight => ("right", "left"),
            Growth::BiLeft => ("left", "right"),
        }
    }
}

/// A tree's declared growth [SPEC 12], default `column`.
#[derive(Clone, Copy, PartialEq)]
enum Dir {
    Column,
    Row,
    Bilateral,
}

fn dir_of(style: &[Decl]) -> Dir {
    dir_from(
        style
            .iter()
            .rev()
            .find(|d| d.name == "direction")
            .and_then(decl_ident),
    )
}

fn dir_from(ident: Option<&str>) -> Dir {
    match ident {
        Some("row") => Dir::Row,
        Some("bilateral") => Dir::Bilateral,
        _ => Dir::Column,
    }
}

/// Which bilateral half a first-level topic fills [SPEC 12].
#[derive(Clone, Copy, PartialEq)]
enum Half {
    Right,
    Left,
}

impl Half {
    /// The generated marker class the engine reads to place the half.
    fn class(self) -> &'static str {
        match self {
            Half::Right => "side-right",
            Half::Left => "side-left",
        }
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
    // The scene's own topic children are the root (depth 0) of a root tree.
    let ctx = root_tree.then(|| TreeCtx {
        dir: root_direction(&file.stylesheet),
        depth: 0,
    });
    for c in &file.instances {
        check(c, ctx, &types)?;
    }
    Ok(())
}

/// A topic's tree context: the enclosing tree's direction and this topic's
/// depth (root 0, first-level 1) — enough to judge a `side:` [SPEC 12/20].
#[derive(Clone, Copy)]
struct TreeCtx {
    dir: Dir,
    depth: usize,
}

fn check(child: &Child, ctx: Option<TreeCtx>, types: &Types) -> Result<(), Error> {
    let Child::Box(n) = child else {
        return Ok(());
    };
    let topic = is_topic_ast(n, types);
    if topic && ctx.is_none() {
        return Err(Error::at(
            n.span,
            "'|topic|' builds a tree — it belongs in a 'layout: tree'",
        ));
    }
    if let (true, Some(c)) = (topic, ctx) {
        check_side(n, c)?;
    }
    let tree = is_tree_style(&n.style);
    if tree {
        check_root_count(&n.children, n.span)?;
    }
    // A tree container opens depth 0; a topic deepens its own context; anything
    // else drops out of the tree.
    let child_ctx = if tree {
        Some(TreeCtx {
            dir: dir_of(&n.style),
            depth: 0,
        })
    } else if let (true, Some(c)) = (topic, ctx) {
        Some(TreeCtx {
            dir: c.dir,
            depth: c.depth + 1,
        })
    } else {
        None
    };
    for c in &n.children {
        check(c, child_ctx, types)?;
    }
    Ok(())
}

/// `side:` is a bilateral first-level half-picker [SPEC 12/20]: `left`/`right`
/// on a first-level topic (the override) is legal; `top`/`bottom` there, a
/// `side:` on a deeper bilateral topic, or any `side:` under `row`/`column`
/// errors.
fn check_side(n: &Node, c: TreeCtx) -> Result<(), Error> {
    let Some(val) = n
        .style
        .iter()
        .rev()
        .find(|d| d.name == "side")
        .and_then(decl_ident)
    else {
        return Ok(());
    };
    match c.dir {
        Dir::Row | Dir::Column => Err(Error::at(
            n.span,
            "'side' picks a bilateral branch's half — this tree has one growth direction",
        )),
        Dir::Bilateral => match val {
            "left" | "right" if c.depth == 1 => Ok(()),
            "left" | "right" => Err(Error::at(
                n.span,
                "'side' picks a bilateral branch's half — this tree has one growth direction",
            )),
            _ => Err(Error::at(
                n.span,
                "a bilateral tree grows left and right — 'side' takes left or right",
            )),
        },
    }
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
    match dir_of(style) {
        Dir::Bilateral => build_bilateral(children, links),
        Dir::Row => build_scope(children, links, 0, Growth::Row),
        Dir::Column => build_scope(children, links, 0, Growth::Column),
    }
}

/// For one scope (its direct `children` and its own `links`): mint anonymous
/// topic ids and process each topic with the scope-wide growth `g`.
fn build_scope(children: &mut [Child], links: &mut Vec<Link>, level: usize, g: Growth) {
    mint_ids(children);
    for c in children.iter_mut() {
        if let Child::Box(t) = c
            && is_topic(t)
        {
            build_topic(t, links, level, g);
        }
    }
    // Order by span so the lowered order matches the fmt-printed (span-sorted)
    // desugar output — desugar transparency; the router's declaration-order
    // tie-break is the same whether source or its lowering is compiled.
    links.sort_by_key(|l| l.span.start);
}

/// One topic `t` growing with `g`: stamp its level class, generate its fan into
/// the scope's `links`, and recurse into its body a generation deeper (its
/// children's fans landing in the topic's own links). Shared by the uniform
/// (`row`/`column`) scope walk and the per-half bilateral walk — one mechanism.
fn build_topic(t: &mut Node, links: &mut Vec<Link>, level: usize, g: Growth) {
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
        push_unique(links, branch_fan(&pid, &kids, g, t.span));
    }
    build_scope(&mut t.children, &mut t.links, level + 1, g);
}

/// A bilateral tree [SPEC 12]: the single root fans to both sides. Its
/// first-level topics split — the first ⌈n/2⌉ (declaration order) right, the
/// rest left, an authored `side: left|right` overriding its half — and each
/// half then grows as a `row` tree (the left mirrored). The root emits one fan
/// per non-empty half; each first-level subtree carries its half's orientation
/// down.
fn build_bilateral(children: &mut [Child], links: &mut Vec<Link>) {
    mint_ids(children);
    for c in children.iter_mut() {
        let Child::Box(root) = c else { continue };
        if !is_topic(root) {
            continue;
        }
        root.classes.push(lini_class("level-0"));
        mint_ids(&mut root.children);

        // First-level topic positions (indices into the root's children).
        let first: Vec<usize> = root
            .children
            .iter()
            .enumerate()
            .filter_map(|(i, cc)| match cc {
                Child::Box(k) if is_topic(k) => Some(i),
                _ => None,
            })
            .collect();
        let n = first.len();
        // Base split, then per-topic `side:` override (read + consume) and the
        // generated half class the engine places on.
        let mut halves: Vec<Half> = (0..n)
            .map(|i| {
                if i < n.div_ceil(2) {
                    Half::Right
                } else {
                    Half::Left
                }
            })
            .collect();
        for (pos, &idx) in first.iter().enumerate() {
            let Child::Box(k) = &mut root.children[idx] else {
                continue;
            };
            if let Some(s) = take_side(&mut k.style) {
                halves[pos] = if s == "left" { Half::Left } else { Half::Right };
            }
            k.classes.push(lini_class(halves[pos].class()));
        }

        // The root emits both sides — one fan per non-empty half.
        if let Some(pid) = root.id.clone() {
            for (half, g) in [(Half::Right, Growth::BiRight), (Half::Left, Growth::BiLeft)] {
                let kids: Vec<String> = first
                    .iter()
                    .zip(&halves)
                    .filter(|&(_, &h)| h == half)
                    .filter_map(|(&i, _)| match &root.children[i] {
                        Child::Box(k) => k.id.clone(),
                        _ => None,
                    })
                    .collect();
                if !kids.is_empty() {
                    push_unique(links, branch_fan(&pid, &kids, g, root.span));
                }
            }
        }

        // Each first-level subtree grows uniformly in its half's orientation.
        for (pos, &idx) in first.iter().enumerate() {
            let g = match halves[pos] {
                Half::Right => Growth::BiRight,
                Half::Left => Growth::BiLeft,
            };
            let Child::Box(k) = &mut root.children[idx] else {
                continue;
            };
            build_topic(k, &mut root.links, 1, g);
        }
        root.links.sort_by_key(|l| l.span.start);
    }
    links.sort_by_key(|l| l.span.start);
}

/// Read and remove a topic's inline `side:` (the last wins) [SPEC 12] — the
/// half is re-expressed as a generated class, so the raw property never reaches
/// resolve.
fn take_side(style: &mut Vec<Decl>) -> Option<String> {
    let val = style
        .iter()
        .rev()
        .find(|d| d.name == "side")
        .and_then(decl_ident)
        .map(str::to_string);
    style.retain(|d| d.name != "side");
    val
}

/// Push a generated link unless the scope already carries an identical one —
/// the fixed-point guard.
fn push_unique(links: &mut Vec<Link>, link: Link) {
    if !links.iter().any(|l| same_link(l, &link)) {
        links.push(link);
    }
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

fn root_direction(stylesheet: &[StyleItem]) -> Dir {
    dir_from(stylesheet.iter().rev().find_map(|it| match it {
        StyleItem::RootDecl(d) if d.name == "direction" => decl_ident(d),
        _ => None,
    }))
}

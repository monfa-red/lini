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

use super::classes::{has_two_class_rule, lini_class};
use super::types::Types;
use crate::ast::{ChainOp, LineStyle, LinkMarker, LinkOp};
use crate::error::Error;
use crate::ledger::defaults::{
    MINDMAP_BRANCH_FONT, MINDMAP_LEAF_FONT, MINDMAP_MAX_WIDTH, MINDMAP_SUB_FONT,
};
use crate::span::Span;
use crate::syntax::ast::{
    Child, Decl, Endpoint, EndpointGroup, File, Link, Node, PointRef, Rule, SelUnit, Selector,
    StyleItem, Value,
};
use std::collections::BTreeSet;

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

    /// The half's outward growth.
    fn growth(self) -> Growth {
        match self {
            Half::Right => Growth::BiRight,
            Half::Left => Growth::BiLeft,
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
    // A `|mindmap|` is its own root topic [SPEC 8]: outside a tree it opens
    // one (desugar seats it in a generated tree scope), so it is legal
    // anywhere; inside a tree it is an ordinary topic.
    let mindmap = topic && is_mindmap_ast(n, types);
    if topic && !mindmap && ctx.is_none() {
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
    // A tree container opens depth 0; a topic deepens its own context (a
    // stand-alone mindmap *is* depth 0, so its topic children are first-level
    // branches); anything else drops out of the tree.
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
    } else if mindmap {
        Some(TreeCtx {
            dir: mindmap_dir_of(&n.style),
            depth: 1,
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

/// Whether the node's written type resolves through `mindmap` [SPEC 8].
fn is_mindmap_ast(n: &Node, types: &Types) -> bool {
    let ty = n.ty.as_deref().unwrap_or("box");
    types
        .resolve(ty, n.span)
        .map(|i| i.chain.iter().any(|c| c == "mindmap"))
        .unwrap_or(false)
}

/// A stand-alone mindmap's growth for `side:` validation — its inline
/// `direction:` if authored, else the preset's `bilateral` [SPEC 8].
fn mindmap_dir_of(style: &[Decl]) -> Dir {
    match style
        .iter()
        .rev()
        .find(|d| d.name == "direction")
        .and_then(decl_ident)
    {
        Some("row") => Dir::Row,
        Some("column") => Dir::Column,
        _ => Dir::Bilateral,
    }
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
/// an already-levelled scope regenerates nothing. A root topic that is a
/// `|mindmap|` additionally runs the palette walk [SPEC 8].
pub(crate) fn build_tree(children: &mut [Child], links: &mut Vec<Link>, style: &[Decl]) {
    let already = children
        .iter()
        .any(|c| matches!(c, Child::Box(n) if is_topic(n) && has_level_class(n)));
    if already {
        return;
    }
    match dir_of(style) {
        Dir::Bilateral => build_bilateral(children, links),
        Dir::Row => build_scope(children, links, 0, Growth::Row, None),
        Dir::Column => build_scope(children, links, 0, Growth::Column, None),
    }
}

/// For one scope (its direct `children` and its own `links`): mint anonymous
/// topic ids and process each topic with the scope-wide growth `g`. `hue` is
/// the enclosing branch's palette class, `None` outside a mindmap.
fn build_scope(
    children: &mut [Child],
    links: &mut Vec<Link>,
    level: usize,
    g: Growth,
    hue: Option<&str>,
) {
    mint_ids(children);
    for c in children.iter_mut() {
        if let Child::Box(t) = c
            && is_topic(t)
        {
            build_topic(t, links, level, g, hue);
        }
    }
    // Order by span so the lowered order matches the fmt-printed (span-sorted)
    // desugar output — desugar transparency; the router's declaration-order
    // tie-break is the same whether source or its lowering is compiled.
    links.sort_by_key(|l| l.span.start);
}

/// One topic `t` growing with `g`: stamp its level (and branch hue) class,
/// generate its fan into the scope's `links` — the fan wearing the hue too, so
/// one generated `.lini-mindmap .lini-hue-*` rule tints cards and wires alike —
/// and recurse into its body a generation deeper (its children's fans landing
/// in the topic's own links). Shared by the uniform (`row`/`column`) scope walk
/// and the per-half bilateral walk — one mechanism. A root-level `|mindmap|`
/// routes to [`build_mindmap_root`] instead.
fn build_topic(t: &mut Node, links: &mut Vec<Link>, level: usize, g: Growth, hue: Option<&str>) {
    if level == 0 && is_mindmap(t) {
        return build_mindmap_root(t, links, g);
    }
    t.classes.push(lini_class(&format!("level-{level}")));
    if let Some(h) = hue {
        t.classes.push(h.to_string());
    }
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
        let mut fan = branch_fan(&pid, &kids, g, t.span);
        if let Some(h) = hue {
            fan.classes.push(h.to_string());
        }
        push_unique(links, fan);
    }
    build_scope(&mut t.children, &mut t.links, level + 1, g, hue);
}

/// The palette walk [SPEC 8]: hue classes in declaration order per first-level
/// branch — the palette's hue order, red and grey skipped, wrapping past nine.
fn walk_hue(pos: usize) -> String {
    let hues: Vec<&str> = crate::palette::walk_hues().collect();
    lini_class(&format!("hue-{}", hues[pos % hues.len()]))
}

/// A mindmap root in a `row`/`column` tree [SPEC 8]: the root stays neutral;
/// each first-level branch takes the next hue and its **own** root arm (a
/// one-child fan wearing the hue, spanned to the branch so declaration order
/// holds), since arms of one shared fan could not be tinted apart — the root
/// port fans no trunk, each tinted arm lands its own port. The bilateral twin
/// lives in [`build_bilateral`].
fn build_mindmap_root(t: &mut Node, links: &mut Vec<Link>, g: Growth) {
    t.classes.push(lini_class("level-0"));
    mint_ids(&mut t.children);
    let first: Vec<usize> = topic_positions(&t.children);
    let pid = t.id.clone();
    for (pos, &idx) in first.iter().enumerate() {
        let hue = walk_hue(pos);
        let Child::Box(k) = &mut t.children[idx] else {
            continue;
        };
        if let (Some(pid), Some(kid)) = (&pid, k.id.clone()) {
            let mut arm = branch_fan(pid, &[kid], g, k.span);
            arm.classes.push(hue.clone());
            push_unique(links, arm);
        }
        build_topic(k, &mut t.links, 1, g, Some(&hue));
    }
    t.links.sort_by_key(|l| l.span.start);
}

/// First-level topic positions (indices into a root's children).
fn topic_positions(children: &[Child]) -> Vec<usize> {
    children
        .iter()
        .enumerate()
        .filter_map(|(i, cc)| match cc {
            Child::Box(k) if is_topic(k) => Some(i),
            _ => None,
        })
        .collect()
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
        let mindmap = is_mindmap(root);
        root.classes.push(lini_class("level-0"));
        mint_ids(&mut root.children);

        // First-level topic positions (indices into the root's children).
        let first: Vec<usize> = topic_positions(&root.children);
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
            // Read + consume the authored `side:` [SPEC 12] — the half is
            // re-expressed as a generated class, so the raw property never
            // reaches resolve.
            if let Some(s) = take_ident(&mut k.style, "side") {
                halves[pos] = if s == "left" { Half::Left } else { Half::Right };
            }
            k.classes.push(lini_class(halves[pos].class()));
        }

        // The root emits both sides — one fan per non-empty half. A mindmap
        // root instead emits one **tinted arm per branch** [SPEC 8] (arms of a
        // shared fan could not be tinted apart), each spanned to its branch so
        // declaration order holds — the row/column twin is
        // [`build_mindmap_root`].
        if let Some(pid) = root.id.clone() {
            if mindmap {
                for (pos, &idx) in first.iter().enumerate() {
                    let Child::Box(k) = &root.children[idx] else {
                        continue;
                    };
                    if let Some(kid) = k.id.clone() {
                        let mut arm = branch_fan(&pid, &[kid], halves[pos].growth(), k.span);
                        arm.classes.push(walk_hue(pos));
                        push_unique(links, arm);
                    }
                }
            } else {
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
        }

        // Each first-level subtree grows uniformly in its half's orientation —
        // in a mindmap, carrying its branch hue down [SPEC 8].
        for (pos, &idx) in first.iter().enumerate() {
            let hue = mindmap.then(|| walk_hue(pos));
            let Child::Box(k) = &mut root.children[idx] else {
                continue;
            };
            build_topic(k, &mut root.links, 1, halves[pos].growth(), hue.as_deref());
        }
        root.links.sort_by_key(|l| l.span.start);
    }
    links.sort_by_key(|l| l.span.start);
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
        copy: None,
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

// ───────────────────────── the |mindmap| seat & rules ─────────────────────────

/// Seat a stand-alone `|mindmap|` [SPEC 8]: the node is the visible root topic,
/// so the scope that contains it becomes the tree scope — `layout: tree` unless
/// the scope authors a layout, the mindmap's own inline `direction:` hoisted
/// (consumed; default `bilateral`) unless the scope authors one, and `routing:
/// natural` unless authored. Every injected decl prints in `lini desugar`, and
/// the absence gates make authored config win and re-desugar a fixed point.
/// Called on the scene root's decls and on a non-topic node's style (a topic's
/// body belongs to the enclosing tree, where a nested mindmap is an ordinary
/// topic).
pub(crate) fn seat_mindmap(style: &mut Vec<Decl>, children: &mut [Child]) {
    let Some(mm) = children.iter_mut().find_map(|c| match c {
        Child::Box(n) if is_mindmap(n) => Some(n),
        _ => None,
    }) else {
        return;
    };
    if !style.iter().any(|d| d.name == "layout") {
        style.push(ident_decl("layout", "tree"));
    }
    // An authored non-tree layout wins outright — the mindmap stays a card.
    if !is_tree_style(style) {
        return;
    }
    if !style.iter().any(|d| d.name == "direction") {
        let dir = take_ident(&mut mm.style, "direction").unwrap_or_else(|| "bilateral".into());
        style.push(ident_decl("direction", &dir));
    }
    if !style.iter().any(|d| d.name == "routing") {
        // Hoisted (consumed) like `direction:` — the root's arms live in THIS
        // scope, so a routing left on the node would govern only its body's
        // fans and split the tree across two strategies.
        let r = take_ident(&mut mm.style, "routing").unwrap_or_else(|| "natural".into());
        style.push(ident_decl("routing", &r));
    }
}

/// The `|mindmap|` garnish rules [SPEC 8], generated beside the scoped note
/// rules — each an ordinary `.lini-mindmap .lini-*` descendant rule, so it
/// sits below the authored class/id/inline tiers and `lini desugar` shows it;
/// a rule the (re-desugared) file already carries is not re-generated. Three
/// garnishes: the topic wrap cap + the root-weight reset, the depth ramp per
/// present level, and the palette walk's tint per present hue (`wash` fill,
/// `deep` stroke — cards and their branch wires wear the same hue class —
/// `ink` text; dark mode rides the tiers' `light-dark()` pairs).
pub(crate) fn mindmap_rules(present: &BTreeSet<String>, user_rules: &[Rule]) -> Vec<Rule> {
    if !present.contains("mindmap") {
        return Vec::new();
    }
    let mut out: Vec<Rule> = Vec::new();
    let mut push = |class: &str, decls: Vec<Decl>| {
        if !has_two_class_rule(user_rules, "lini-mindmap", class) {
            out.push(Rule {
                selector: Selector {
                    units: vec![
                        SelUnit::Class("lini-mindmap".to_string()),
                        SelUnit::Class(class.to_string()),
                    ],
                },
                decls,
                span: Span::empty(),
            });
        }
    };
    // Branch topics wrap at the cap and shed the root card's inherited
    // semibold down to medium — the box text baseline (500), the mindmap's
    // floor; the root itself is no descendant, so its bundle tier stands.
    push(
        "lini-topic",
        vec![
            number_decl("max-width", MINDMAP_MAX_WIDTH),
            ident_decl("font-weight", "medium"),
        ],
    );
    // The depth ramp [SPEC 8]: 15 / 14 / 13 down the generations, one rule per
    // level the scene actually wears (root 0 rides the |mindmap| bundle).
    let mut levels: Vec<usize> = present
        .iter()
        .filter_map(|p| p.strip_prefix("level-")?.parse().ok())
        .filter(|&n| n >= 1)
        .collect();
    levels.sort_unstable();
    for n in levels {
        let size = match n {
            1 => MINDMAP_BRANCH_FONT,
            2 => MINDMAP_SUB_FONT,
            _ => MINDMAP_LEAF_FONT,
        };
        push(
            &format!("lini-level-{n}"),
            vec![number_decl("font-size", size)],
        );
    }
    // The palette walk's tints, in walk order for the hues actually assigned.
    for hue in crate::palette::walk_hues() {
        if present.contains(&format!("hue-{hue}")) {
            push(
                &format!("lini-hue-{hue}"),
                vec![
                    var_decl("fill", &format!("{hue}-wash")),
                    var_decl("stroke", &format!("{hue}-deep")),
                    var_decl("color", &format!("{hue}-ink")),
                ],
            );
        }
    }
    out
}

/// Read (the last wins) and remove a `name:` ident from a style block — shared
/// by the `side:` consume (re-expressed as a half class) and [`seat_mindmap`]'s
/// `direction:` hoist (moved to the generated tree scope instead of steering
/// the root card's own content).
fn take_ident(style: &mut Vec<Decl>, name: &str) -> Option<String> {
    let val = style
        .iter()
        .rev()
        .find(|d| d.name == name)
        .and_then(decl_ident)
        .map(str::to_string);
    if val.is_some() {
        style.retain(|d| d.name != name);
    }
    val
}

fn ident_decl(name: &str, v: &str) -> Decl {
    Decl {
        name: name.into(),
        groups: vec![vec![Value::Ident(v.into())]],
        span: Span::empty(),
    }
}

fn number_decl(name: &str, v: f64) -> Decl {
    Decl {
        name: name.into(),
        groups: vec![vec![Value::Number(v)]],
        span: Span::empty(),
    }
}

fn var_decl(name: &str, v: &str) -> Decl {
    Decl {
        name: name.into(),
        groups: vec![vec![Value::Var(v.into())]],
        span: Span::empty(),
    }
}

// ───────────────────────── shared helpers ─────────────────────────

fn is_topic(n: &Node) -> bool {
    n.classes.iter().any(|c| c == "lini-topic")
}

/// Whether a (lowered) node wears the `|mindmap|` type class [SPEC 8].
fn is_mindmap(n: &Node) -> bool {
    n.classes.iter().any(|c| c == "lini-mindmap")
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

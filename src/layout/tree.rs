//! `layout: tree` [SPEC 12] — a rooted hierarchy laid out in generations.
//!
//! Desugar keeps the topic **nesting** (scoped ids, sealed bodies, dot-paths)
//! and generates the parent→child branch links as ordinary `|-|` wires
//! ([`crate::desugar::tree`]). This engine reads that nested topic tree, lays
//! each topic's **card** from its own non-topic content only (branches never
//! grow the card), places the cards by generation, and emits them **nested** —
//! each topic a placed child of its parent, its card box its own extent, its
//! subtree overhanging (the drawing-features precedent). The routing scene index
//! then sees a topic's keep-out as its own card (never its subtree's hull), the
//! dot-paths address the placed nodes unchanged, and the branch links route as
//! ordinary side-by-side wires once the containment special case is gated on
//! geometry ([`crate::routing::ortho::scene::SceneIndex::geo_contains`]).
//!
//! `direction` picks the growth axis (`column` down — the org chart — or `row`
//! rightward); `gap: g s` is the generation distance then the sibling
//! separation (a scalar sets both). `bilateral` is Stage 2 — treated as
//! `column` here.

use super::flex::Axis;
use super::ir::{Bbox, PlacedNode};
use super::{Ctx, child_path, layout_inst, prim, primitives};
use crate::error::Error;
use crate::resolve::{AttrMap, Program, ResolvedInst, ResolvedValue};
use crate::span::Span;

/// Is this node a tree container [SPEC 12]? Detected by its `layout:` attr — the
/// same key the chart / sequence dispatch reads, so it is intercepted before the
/// generic container path.
pub(super) fn is_tree(attrs: &AttrMap) -> bool {
    matches!(attrs.get("layout"), Some(ResolvedValue::Ident(s)) if s == "tree")
}

/// The growth axis from `direction` [SPEC 12], default `column`. `bilateral`
/// (Stage 2) falls back to `column`.
fn growth(attrs: &AttrMap) -> Axis {
    match attrs.get("direction") {
        Some(ResolvedValue::Ident(s)) if s == "row" => Axis::Row,
        _ => Axis::Column,
    }
}

/// Whether a resolved instance is a topic (a structural node) rather than the
/// container's own content — the type chain wears `topic`.
fn is_topic_inst(inst: &ResolvedInst) -> bool {
    inst.type_chain.iter().any(|t| t == "topic")
}

/// A topic card's depth from its `.lini-level-N` class (desugar-generated); root 0.
fn level_of(n: &PlacedNode) -> usize {
    n.type_chain
        .iter()
        .find_map(|t| t.strip_prefix("level-").and_then(|d| d.parse().ok()))
        .unwrap_or(0)
}

/// A `|tree|` **node** [SPEC 12]: lay each topic's card out with its content,
/// place the generations, and return the container carrying the nested topics.
pub(super) fn layout_node(
    inst: &ResolvedInst,
    path: &str,
    program: &Program,
) -> Result<PlacedNode, Error> {
    let (children, bbox) = arrange(&inst.attrs, &inst.children, path, program, inst.span)?;
    Ok(prim::container(inst, bbox, children))
}

/// A root `{ layout: tree }` scene [SPEC 12]: the scene itself is the tree
/// container. The router routes its branch links after (they are ordinary
/// wires) — the caller wires that up like any scene.
pub(super) fn layout_root(program: &Program) -> Result<(Vec<PlacedNode>, Bbox), Error> {
    arrange(
        &program.scene.attrs,
        &program.scene.nodes,
        "",
        program,
        Span::empty(),
    )
}

/// The tree arrangement shared by the node and root entries: flatten the nested
/// topics into cards (each sized from its own content only), place the
/// generations, re-nest, and seat the scope's non-topic content around the
/// finished cluster. Returns the placed children and the padded bbox.
fn arrange(
    attrs: &AttrMap,
    inst_children: &[ResolvedInst],
    path: &str,
    program: &Program,
    span: Span,
) -> Result<(Vec<PlacedNode>, Bbox), Error> {
    // A tree's interior is sheet-space [SPEC 15.1] — topics never inherit an
    // enclosing drawing's view scale.
    let axis = growth(attrs);
    // `gap: generation sibling` [SPEC 12]; a scalar sets both. `primitives::gap`
    // reads the two values in order — first is the generation distance.
    let (generation, sibling) = primitives::gap(attrs, span)?;

    // Flatten the nested topics into a pre-order card list (each card sized from
    // its own content only), keeping the container's non-topic content aside.
    let mut cards: Vec<PlacedNode> = Vec::new();
    let mut content: Vec<PlacedNode> = Vec::new();
    for c in inst_children {
        if is_topic_inst(c) {
            flatten_cards(c, &child_path(path, c), program, &mut cards)?;
        } else {
            content.push(layout_inst(c, &child_path(path, c), program, Ctx::sheet())?);
        }
    }

    // Place every card (post-order packing), then re-nest so each topic is a
    // placed child of its parent with its card its own overhang box.
    let (nodes, roots) = place_generations(&mut cards, axis, generation, sibling);
    let mut children = nest(cards, &nodes, &roots);

    // Content (a caption / free text) is not part of the tree structure: pin an
    // out-of-flow overlay onto the finished cluster, stack any flow content
    // above it — the honest minimum (SPEC does not specify container content).
    let cluster = union_all(&children);
    place_content(&mut children, content, cluster, axis, sibling)?;

    let pad = primitives::padding(attrs, span)?;
    let body = union_all(&children);
    let bbox = body.expand(pad.top, pad.right, pad.bottom, pad.left);
    Ok((children, bbox))
}

/// Recursively push each topic's card (pre-order), sizing the card from its
/// non-topic content only — the branches are laid out as their own cards.
fn flatten_cards(
    inst: &ResolvedInst,
    path: &str,
    program: &Program,
    cards: &mut Vec<PlacedNode>,
) -> Result<(), Error> {
    cards.push(layout_card(inst, path, program)?);
    for c in &inst.children {
        if is_topic_inst(c) {
            flatten_cards(c, &child_path(path, c), program, cards)?;
        }
    }
    Ok(())
}

/// One topic's card [SPEC 12]: the topic laid out with its **non-topic** content
/// only. Reuse the generic layout on a copy sans branches — one mechanism, so the
/// card sizes exactly like any block (padding, radius, wrap, the label leaf).
fn layout_card(inst: &ResolvedInst, path: &str, program: &Program) -> Result<PlacedNode, Error> {
    let mut card = inst.clone();
    card.children.retain(|c| !is_topic_inst(c));
    layout_inst(&card, path, program, Ctx::sheet())
}

/// Recursive union of a placed subtree's card boxes, in the container's frame.
fn union_all(nodes: &[PlacedNode]) -> Bbox {
    fn go(nodes: &[PlacedNode], ox: f64, oy: f64, acc: &mut Bbox) {
        for n in nodes {
            let (x, y) = (ox + n.cx, oy + n.cy);
            *acc = acc.union(n.bbox.shifted(x, y));
            go(&n.children, x, y, acc);
        }
    }
    let mut acc = Bbox::empty();
    go(nodes, 0.0, 0.0, &mut acc);
    acc
}

/// One reconstructed topic: which card it is, its children (indices into this
/// list), and the cross-axis span its subtree occupies.
struct Node {
    card: usize,
    children: Vec<usize>,
    subtree_cross: f64,
}

/// Reconstruct the hierarchy from level classes + source order and place every
/// card: generations one `generation` gap apart along the main axis (a level's
/// cards share a line), siblings packed `sibling` apart across it, each parent
/// centred over its subtree's cross span [SPEC 12]. Card positions are absolute
/// in the container frame; the returned tree drives the re-nesting.
fn place_generations(
    cards: &mut [PlacedNode],
    axis: Axis,
    generation: f64,
    sibling: f64,
) -> (Vec<Node>, Vec<usize>) {
    let mut nodes: Vec<Node> = (0..cards.len())
        .map(|card| Node {
            card,
            children: Vec::new(),
            subtree_cross: 0.0,
        })
        .collect();
    let mut roots: Vec<usize> = Vec::new();
    if cards.is_empty() {
        return (nodes, roots);
    }
    // Card extents along the two axes.
    let main = |c: &PlacedNode| match axis {
        Axis::Column => c.bbox.h(),
        Axis::Row => c.bbox.w(),
    };
    let cross = |c: &PlacedNode| match axis {
        Axis::Column => c.bbox.w(),
        Axis::Row => c.bbox.h(),
    };

    // Rebuild the tree: a topic at level L is a child of the most recent topic
    // at level L-1 (inverting the pre-order flatten).
    let levels: Vec<usize> = cards.iter().map(level_of).collect();
    let mut stack: Vec<usize> = Vec::new();
    for (i, &lvl) in levels.iter().enumerate() {
        stack.truncate(lvl);
        match stack.last() {
            Some(&parent) if lvl > 0 => nodes[parent].children.push(i),
            _ => roots.push(i),
        }
        stack.push(i);
    }

    // Post-order: each subtree's cross span is the wider of its own card and its
    // children packed with sibling gaps.
    fn measure(
        i: usize,
        nodes: &mut [Node],
        cards: &[PlacedNode],
        cross: &dyn Fn(&PlacedNode) -> f64,
        gap: f64,
    ) {
        let kids = nodes[i].children.clone();
        for &k in &kids {
            measure(k, nodes, cards, cross, gap);
        }
        let block: f64 = if kids.is_empty() {
            0.0
        } else {
            kids.iter().map(|&k| nodes[k].subtree_cross).sum::<f64>()
                + gap * (kids.len() - 1) as f64
        };
        nodes[i].subtree_cross = cross(&cards[nodes[i].card]).max(block);
    }

    // Pre-order: main position by level (cumulative band centres), cross position
    // by packing children about their parent's centre.
    let max_level = *levels.iter().max().unwrap_or(&0);
    let mut band_size = vec![0.0_f64; max_level + 1];
    for (i, &lvl) in levels.iter().enumerate() {
        band_size[lvl] = band_size[lvl].max(main(&cards[i]));
    }
    let mut band_centre = vec![0.0_f64; max_level + 1];
    for d in 1..=max_level {
        band_centre[d] =
            band_centre[d - 1] + band_size[d - 1] / 2.0 + generation + band_size[d] / 2.0;
    }

    #[allow(clippy::too_many_arguments)]
    fn assign(
        i: usize,
        centre: f64,
        nodes: &[Node],
        cards: &mut [PlacedNode],
        levels: &[usize],
        band_centre: &[f64],
        axis: Axis,
        gap: f64,
    ) {
        let card = nodes[i].card;
        let main_c = band_centre[levels[card]];
        match axis {
            Axis::Column => {
                cards[card].cx = centre;
                cards[card].cy = main_c;
            }
            Axis::Row => {
                cards[card].cx = main_c;
                cards[card].cy = centre;
            }
        }
        let block: f64 = if nodes[i].children.is_empty() {
            0.0
        } else {
            nodes[i]
                .children
                .iter()
                .map(|&k| nodes[k].subtree_cross)
                .sum::<f64>()
                + gap * (nodes[i].children.len() - 1) as f64
        };
        let mut cursor = centre - block / 2.0;
        for &k in &nodes[i].children {
            let slot = nodes[k].subtree_cross;
            assign(
                k,
                cursor + slot / 2.0,
                nodes,
                cards,
                levels,
                band_centre,
                axis,
                gap,
            );
            cursor += slot + gap;
        }
    }

    // Multiple roots never reach here (structure validation caught them); place
    // each in its own cross band for safety.
    let mut cursor = 0.0;
    for &r in &roots {
        measure(r, &mut nodes, cards, &cross, sibling);
        let span = nodes[r].subtree_cross;
        assign(
            r,
            cursor + span / 2.0,
            &nodes,
            cards,
            &levels,
            &band_centre,
            axis,
            sibling,
        );
        cursor += span + sibling;
    }
    (nodes, roots)
}

/// Re-nest the placed cards into a hierarchy [SPEC 12]: each topic becomes a
/// placed child of its parent, its `cx`/`cy` made relative to the parent's
/// centre so the scene index accumulates absolute positions unchanged. A card
/// keeps its own content children; its subtopics overhang its card box.
fn nest(cards: Vec<PlacedNode>, nodes: &[Node], roots: &[usize]) -> Vec<PlacedNode> {
    fn build(i: usize, nodes: &[Node], slots: &mut [Option<PlacedNode>]) -> PlacedNode {
        let mut me = slots[nodes[i].card].take().expect("card placed once");
        let (mx, my) = (me.cx, me.cy);
        for &k in &nodes[i].children {
            let mut child = build(k, nodes, slots);
            child.cx -= mx;
            child.cy -= my;
            me.children.push(child);
        }
        me
    }
    let mut slots: Vec<Option<PlacedNode>> = cards.into_iter().map(Some).collect();
    roots.iter().map(|&r| build(r, nodes, &mut slots)).collect()
}

/// Seat the container's own (non-topic) content around the finished tree
/// cluster: a `pin`ned overlay onto the cluster box, flow content stacked one
/// `sibling` gap before it on the cross axis.
fn place_content(
    children: &mut Vec<PlacedNode>,
    content: Vec<PlacedNode>,
    cluster: Bbox,
    axis: Axis,
    sibling: f64,
) -> Result<(), Error> {
    let mut flow_cursor = match axis {
        Axis::Column => cluster.min_y,
        Axis::Row => cluster.min_x,
    };
    for mut c in content {
        if let Some(pin) = super::anchors::read_pin(&c.attrs, c.span)? {
            let (cx, cy) = pin.target(cluster, c.bbox);
            c.cx = cx;
            c.cy = cy;
        } else {
            match axis {
                Axis::Column => {
                    c.cx = (cluster.min_x + cluster.max_x) / 2.0;
                    flow_cursor -= sibling + c.bbox.h() / 2.0;
                    c.cy = flow_cursor;
                    flow_cursor -= c.bbox.h() / 2.0;
                }
                Axis::Row => {
                    c.cy = (cluster.min_y + cluster.max_y) / 2.0;
                    flow_cursor -= sibling + c.bbox.w() / 2.0;
                    c.cx = flow_cursor;
                    flow_cursor -= c.bbox.w() / 2.0;
                }
            }
        }
        if let Some((dx, dy)) = super::anchors::translate(&c.attrs, c.span)? {
            c.cx += dx;
            c.cy += dy;
        }
        children.push(c);
    }
    Ok(())
}

#[cfg(test)]
mod tests;

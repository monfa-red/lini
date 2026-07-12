//! `layout: tree` [SPEC 12] — a rooted hierarchy laid out in generations.
//!
//! Desugar has already **flattened** the scope: every topic is a direct child
//! wearing its depth class `.lini-level-N` (root 0), and the parent→child edges
//! are ordinary sibling `|-|` links the orthogonal router draws
//! ([`crate::desugar::tree`]). This engine reconstructs the hierarchy from the
//! flat list's levels + source order, lays each topic's card out with its own
//! content (never grown by its branches), places the cards by generation, and
//! emits them **flat** — the container's direct placed children — so the routing
//! scene index stays honest (no ancestor rect that fails to contain a
//! descendant) and the branch links route as sibling wires.
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

/// Whether a placed child is a topic (structural node) rather than the
/// container's own content — the flattened form wears `lini-topic`.
fn is_topic(n: &PlacedNode) -> bool {
    n.type_chain.iter().any(|t| t == "topic")
}

/// A topic's depth from its `.lini-level-N` class (desugar-generated); root 0.
fn level_of(n: &PlacedNode) -> usize {
    n.type_chain
        .iter()
        .find_map(|t| t.strip_prefix("level-").and_then(|d| d.parse().ok()))
        .unwrap_or(0)
}

/// A `|tree|` **node** [SPEC 12]: lay each topic's card out with its content,
/// place the generations, and return the container carrying the flat topic set.
pub(super) fn layout_node(
    inst: &ResolvedInst,
    path: &str,
    program: &Program,
) -> Result<PlacedNode, Error> {
    // A tree's interior is sheet-space [SPEC 15.1] — topics never inherit an
    // enclosing drawing's view scale.
    let mut cards: Vec<PlacedNode> = Vec::new();
    let mut content: Vec<PlacedNode> = Vec::new();
    for c in &inst.children {
        let placed = layout_inst(c, &child_path(path, c), program, Ctx::sheet())?;
        if is_topic(&placed) {
            cards.push(placed);
        } else {
            content.push(placed);
        }
    }

    let axis = growth(&inst.attrs);
    // `gap: generation sibling` [SPEC 12]; a scalar sets both. `primitives::gap`
    // reads the two values in order — first is the generation distance.
    let (generation, sibling) = primitives::gap(&inst.attrs, inst.span)?;

    place_generations(&mut cards, axis, generation, sibling);

    // Content (a caption / free text) is not part of the tree structure: pin an
    // out-of-flow overlay onto the finished cluster, stack any flow content
    // above it — the honest minimum (SPEC does not specify container content).
    let cluster = union_of(&cards);
    let mut children = cards;
    place_content(&mut children, content, cluster, axis, sibling)?;

    let pad = primitives::padding(&inst.attrs, inst.span)?;
    let body = union_of(&children);
    let bbox = body.expand(pad.top, pad.right, pad.bottom, pad.left);
    Ok(prim::container(inst, bbox, children))
}

/// The union of the placed nodes' bboxes, each in the container's frame
/// (shifted by its `cx`/`cy`). Empty when there are none.
fn union_of(nodes: &[PlacedNode]) -> Bbox {
    let mut it = nodes.iter();
    let Some(first) = it.next() else {
        return Bbox::empty();
    };
    let mut b = first.bbox.shifted(first.cx, first.cy);
    for n in it {
        b = b.union(n.bbox.shifted(n.cx, n.cy));
    }
    b
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
/// centred over its subtree's cross span [SPEC 12].
fn place_generations(cards: &mut [PlacedNode], axis: Axis, generation: f64, sibling: f64) {
    if cards.is_empty() {
        return;
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
    // at level L-1 (inverting desugar's pre-order flatten).
    let levels: Vec<usize> = cards.iter().map(level_of).collect();
    let mut nodes: Vec<Node> = (0..cards.len())
        .map(|card| Node {
            card,
            children: Vec::new(),
            subtree_cross: 0.0,
        })
        .collect();
    let mut stack: Vec<usize> = Vec::new();
    let mut roots: Vec<usize> = Vec::new();
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

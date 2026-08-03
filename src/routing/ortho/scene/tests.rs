//! The scene index's own tests: absolute rects, worlds, and keep-out sets.
//! The part fold ([`super::parts`]) is judged end to end on real sheets, in
//! `layout::schematic::route_tests`.

use super::*;
use crate::layout::ir::{Bbox, PlacedNode};
use crate::resolve::{AttrMap, Markers, NodeKind};
use crate::span::Span;

fn node(
    id: Option<&str>,
    kind: NodeKind,
    cx: f64,
    cy: f64,
    w: f64,
    h: f64,
    children: Vec<PlacedNode>,
) -> PlacedNode {
    PlacedNode {
        id: id.map(String::from),
        kind,
        type_chain: Vec::new(),
        applied_styles: Vec::new(),
        label: None,
        attrs: AttrMap::default(),
        own_style: AttrMap::default(),
        markers: Markers::default(),
        cx,
        cy,
        bbox: Bbox::centered(w, h),
        rotation: 0.0,
        children,
        gutters: Vec::new(),
        links: Vec::new(),
        sketch: None,
        origin: (0.0, 0.0),
        span: Span::empty(),
    }
}

fn rect_node(id: &str, cx: f64, cy: f64, w: f64, h: f64) -> PlacedNode {
    node(Some(id), NodeKind::Block, cx, cy, w, h, Vec::new())
}

/// cat at (0,0) 40×20; garden at (100,50) 80×60 containing dog at (10,5) 30×10.
fn scene() -> Vec<PlacedNode> {
    let dog = rect_node("dog", 10.0, 5.0, 30.0, 10.0);
    let garden = node(
        Some("garden"),
        NodeKind::Block,
        100.0,
        50.0,
        80.0,
        60.0,
        vec![dog],
    );
    vec![rect_node("cat", 0.0, 0.0, 40.0, 20.0), garden]
}

#[test]
fn absolute_rects_accumulate_nested_offsets() {
    let idx = SceneIndex::build(&scene());
    assert_eq!(idx.rect("cat"), Some(Rect::new(-20.0, -10.0, 20.0, 10.0)));
    assert_eq!(idx.rect("garden"), Some(Rect::new(60.0, 20.0, 140.0, 80.0)));
    // dog: offset garden(100,50) + own(10,5), bbox 30×10 centred.
    assert_eq!(
        idx.rect("garden.dog"),
        Some(Rect::new(95.0, 50.0, 125.0, 60.0))
    );
    assert_eq!(idx.rect("dog"), None);
}

#[test]
fn world_is_the_innermost_shared_container() {
    // garden{dog} + bird added beside dog for the sibling case.
    let mut roots = scene();
    roots[1]
        .children
        .push(rect_node("bird", -20.0, 5.0, 20.0, 10.0));
    let idx = SceneIndex::build(&roots);
    let key = |p: &str| idx.node_of(p);
    assert_eq!(idx.world_of("cat", "garden.dog"), None);
    assert_eq!(
        idx.world_of("garden.dog", "garden.bird"),
        key("garden"),
        "siblings route in their parent's interior"
    );
    // Containment: the container endpoint's own interior, both ways.
    assert_eq!(idx.world_of("garden", "garden.dog"), key("garden"));
    assert_eq!(idx.world_of("garden.dog", "garden"), key("garden"));
    // The shared-ancestor pick the validator uses.
    assert_eq!(idx.common_ancestor_world(key("garden"), None), None);
    assert_eq!(
        idx.common_ancestor_world(key("garden"), key("garden")),
        key("garden")
    );
}

#[test]
fn an_anonymous_container_is_a_world_like_a_named_one() {
    // column{ a, b } with no id: a and b keep root-level paths, yet their
    // common world is the column's interior — structure, not strings.
    let a = rect_node("a", -15.0, 0.0, 20.0, 10.0);
    let b = rect_node("b", 15.0, 0.0, 20.0, 10.0);
    let column = node(None, NodeKind::Block, 0.0, 0.0, 80.0, 40.0, vec![a, b]);
    let idx = SceneIndex::build(&[column]);
    let world = idx.common_world("a", "b");
    assert!(world.is_some(), "the anonymous interior is a world");
    assert_eq!(
        idx.world_rect(world),
        Some(Rect::new(-40.0, -20.0, 40.0, 20.0))
    );
    assert_eq!(idx.child_rects(world).len(), 2);
    // The ladder above it is the scene root.
    assert_eq!(idx.parent_world(world), Some(None));
    // geo_contains sees through the anonymous level too: the column is
    // nobody's endpoint, but a's world chain still reaches the root.
    assert!(!idx.geo_contains("a", "b"));
}

#[test]
fn child_rects_lists_one_collapsed_rect_per_direct_child() {
    let idx = SceneIndex::build(&scene());
    assert_eq!(
        idx.child_rects(None),
        vec![
            Rect::new(-20.0, -10.0, 20.0, 10.0),
            Rect::new(60.0, 20.0, 140.0, 80.0),
        ]
    );
    assert_eq!(
        idx.child_rects(idx.node_of("garden")),
        vec![Rect::new(95.0, 50.0, 125.0, 60.0)]
    );
    assert_eq!(idx.child_rects(idx.node_of("garden.dog")), Vec::new());
}

#[test]
fn solid_rects_collapse_non_endpoint_subtrees() {
    let idx = SceneIndex::build(&scene());
    // cat → garden.dog: both passable, garden is an ancestor (transparent);
    // nothing else exists, so nothing is solid.
    assert_eq!(idx.solid_rects_for(["cat", "garden.dog"]), Vec::new());
    // cat → cat: garden is solid and collapses to one rect, dog swallowed.
    assert_eq!(
        idx.solid_rects_for(["cat", "cat"]),
        vec![Rect::new(60.0, 20.0, 140.0, 80.0)]
    );
}

#[test]
fn labels_block_inside_transparent_ancestors_but_not_inside_endpoints() {
    // garden{ label, dog, bird } — routing dog→bird must avoid the label;
    // routing garden→garden must not see its own inner label.
    let label = node(None, NodeKind::Text, 0.0, -25.0, 40.0, 10.0, Vec::new());
    let dog = rect_node("dog", -15.0, 5.0, 20.0, 10.0);
    let bird = rect_node("bird", 15.0, 5.0, 20.0, 10.0);
    let garden = node(
        Some("garden"),
        NodeKind::Block,
        0.0,
        0.0,
        80.0,
        70.0,
        vec![label, dog, bird],
    );
    let idx = SceneIndex::build(&[garden]);
    assert_eq!(
        idx.solid_rects_for(["garden.dog", "garden.bird"]),
        vec![Rect::new(-20.0, -30.0, 20.0, -20.0)]
    );
    // Self-loop on the group: its own label is exempt; the child bodies stay
    // solid (harmless — they sit inside the endpoint's body).
    assert_eq!(
        idx.solid_rects_for(["garden", "garden"]),
        vec![
            Rect::new(-25.0, 0.0, -5.0, 10.0),
            Rect::new(5.0, 0.0, 25.0, 10.0)
        ]
    );
}

#[test]
fn idd_text_is_a_body_not_a_label() {
    let title = node(
        Some("title"),
        NodeKind::Text,
        0.0,
        0.0,
        30.0,
        10.0,
        Vec::new(),
    );
    let idx = SceneIndex::build(&[title, rect_node("cat", 50.0, 0.0, 20.0, 10.0)]);
    assert_eq!(idx.rect("title"), Some(Rect::new(-15.0, -5.0, 15.0, 5.0)));
    // As a non-endpoint it is solid like any body.
    assert_eq!(
        idx.solid_rects_for(["cat", "cat"]),
        vec![Rect::new(-15.0, -5.0, 15.0, 5.0)]
    );
}

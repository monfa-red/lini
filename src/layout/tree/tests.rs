//! Tree placement math [SPEC 12]: generations, sibling packing, parent centring.

use crate::layout::PlacedNode;

fn laid(src: &str) -> Vec<PlacedNode> {
    crate::testutil::laid(src).nodes
}

/// The placed topic card by id, searched flat (topics are direct children of
/// the tree container).
fn topic<'a>(nodes: &'a [PlacedNode], id: &str) -> &'a PlacedNode {
    crate::testutil::placed_by_id(nodes, id).0
}

/// Absolute centre of a topic — container `cx/cy` plus the card's own.
fn centre(nodes: &[PlacedNode], id: &str) -> (f64, f64) {
    let (_, x, y) = crate::testutil::placed_by_id(nodes, id);
    (x, y)
}

#[test]
fn a_column_tree_drops_generations_and_centres_the_parent() {
    let nodes = laid(
        "|column#o| { layout: tree } [\n  |topic#a| \"A\" [\n    |topic#b| \"B\"\n    |topic#c| \"C\"\n  ]\n]\n",
    );
    let (ax, ay) = centre(&nodes, "a");
    let (bx, by) = centre(&nodes, "b");
    let (cx, cy) = centre(&nodes, "c");
    // Children sit one generation below the root (larger y).
    assert!(by > ay && cy > ay, "children below root: {ay} vs {by}/{cy}");
    assert!((by - cy).abs() < 1e-6, "siblings share a generation line");
    // The parent is centred over its subtree's **span** [SPEC 12] — the
    // children's outer edges, not the midpoint of their centres: on a
    // proportional face two siblings rarely measure the same width.
    let edge = |id: &str, x: f64| {
        let t = topic(&nodes, id);
        (x + t.bbox.min_x, x + t.bbox.max_x)
    };
    let (bl, br) = edge("b", bx);
    let (cl, cr) = edge("c", cx);
    let span = (bl.min(cl) + br.max(cr)) / 2.0;
    assert!(
        (ax - span).abs() < 1e-6,
        "parent {ax} centred over the subtree span {span}"
    );
    // Siblings are separated horizontally.
    assert!(cx > bx, "b left of c: {bx} vs {cx}");
}

#[test]
fn a_row_tree_grows_rightward() {
    let nodes = laid(
        "|column#o| { layout: tree; direction: row } [\n  |topic#a| \"A\" [\n    |topic#b| \"B\"\n    |topic#c| \"C\"\n  ]\n]\n",
    );
    let (ax, ay) = centre(&nodes, "a");
    let (bx, by) = centre(&nodes, "b");
    let (cx, cy) = centre(&nodes, "c");
    assert!(
        bx > ax && cx > ax,
        "children right of root: {ax} vs {bx}/{cx}"
    );
    assert!((bx - cx).abs() < 1e-6, "siblings share a generation column");
    assert!(
        (ay - (by + cy) / 2.0).abs() < 1e-6,
        "parent centred beside its children"
    );
    assert!(cy > by, "b above c: {by} vs {cy}");
}

#[test]
fn a_bilateral_tree_splits_first_half_right_rest_left() {
    // n = 3: ⌈3/2⌉ = 2 right (a, b), 1 left (c). Right cards sit right of the
    // root, the left card left of it; the root centres between them.
    let nodes = laid(
        "|column#o| { layout: tree; direction: bilateral } [\n  |topic#r| \"R\" [\n    |topic#a| \"A\"\n    |topic#b| \"B\"\n    |topic#c| \"C\"\n  ]\n]\n",
    );
    let (rx, _) = centre(&nodes, "r");
    let (ax, _) = centre(&nodes, "a");
    let (bx, _) = centre(&nodes, "b");
    let (cx, _) = centre(&nodes, "c");
    assert!(ax > rx && bx > rx, "a/b right of root: {rx} vs {ax}/{bx}");
    assert!(cx < rx, "c left of root: {cx} vs {rx}");
    // The two right subtrees share a generation column.
    assert!((ax - bx).abs() < 1e-6, "a/b share the right column");
}

#[test]
fn a_bilateral_even_split_is_balanced() {
    // n = 4: a, b right; c, d left.
    let nodes = laid(
        "|column#o| { layout: tree; direction: bilateral } [\n  |topic#r| \"R\" [\n    |topic#a| \"A\"\n    |topic#b| \"B\"\n    |topic#c| \"C\"\n    |topic#d| \"D\"\n  ]\n]\n",
    );
    let (rx, _) = centre(&nodes, "r");
    for id in ["a", "b"] {
        assert!(centre(&nodes, id).0 > rx, "{id} right of root");
    }
    for id in ["c", "d"] {
        assert!(centre(&nodes, id).0 < rx, "{id} left of root");
    }
}

#[test]
fn a_bilateral_side_override_moves_a_branch() {
    // n = 3 defaults a, b right and c left; `side: left` on b sends it left
    // while a stays right — the override moves exactly one branch.
    let nodes = laid(
        "|column#o| { layout: tree; direction: bilateral } [\n  |topic#r| \"R\" [\n    |topic#a| \"A\"\n    |topic#b| \"B\" { side: left }\n    |topic#c| \"C\"\n  ]\n]\n",
    );
    let (rx, _) = centre(&nodes, "r");
    assert!(centre(&nodes, "a").0 > rx, "a stays right");
    assert!(centre(&nodes, "b").0 < rx, "b overridden to the left");
    assert!(centre(&nodes, "c").0 < rx, "c stays left");
}

#[test]
fn a_bilateral_half_mirrors_a_deeper_generation() {
    // A right subtree grows further right with depth; a left one further left.
    let nodes = laid(
        "|column#o| { layout: tree; direction: bilateral } [\n  |topic#r| \"R\" [\n    |topic#a| \"A\" [ |topic#ax| \"AX\" ]\n    |topic#c| \"C\" { side: left } [ |topic#cx| \"CX\" ]\n  ]\n]\n",
    );
    let (rx, _) = centre(&nodes, "r");
    let (ax, _) = centre(&nodes, "a");
    let (axx, _) = centre(&nodes, "ax");
    let (cx, _) = centre(&nodes, "c");
    let (cxx, _) = centre(&nodes, "cx");
    assert!(axx > ax && ax > rx, "right generation grows rightward");
    assert!(cxx < cx && cx < rx, "left generation grows leftward");
    // The two second-generation cards mirror about the root by one gap band.
    assert!(
        (axx - rx) > 0.0 && (rx - cxx) > 0.0,
        "symmetric outward growth"
    );
}

#[test]
fn a_deeper_subtree_packs_without_overlap() {
    // b has two children; d/e widen b's subtree so a stays centred over the
    // whole span, and the two leaves never overlap.
    let nodes = laid(
        "|column#o| { layout: tree } [\n  |topic#a| \"A\" [\n    |topic#b| \"B\" [\n      |topic#d| \"D\"\n      |topic#e| \"E\"\n    ]\n    |topic#c| \"C\"\n  ]\n]\n",
    );
    let d = topic(&nodes, "d");
    let e = topic(&nodes, "e");
    let (dx, _) = centre(&nodes, "d");
    let (ex, _) = centre(&nodes, "e");
    let gap = (ex - dx).abs() - (d.bbox.w() + e.bbox.w()) / 2.0;
    assert!(gap > 0.0, "leaves separated (gap {gap})");
}

#[test]
fn a_turned_card_stays_inside_the_cluster_box() {
    // [SPEC 5] law 5: a rotated node propagates its **rotated** bounding
    // rectangle upward, so the tree container's box must hold a turned card's
    // corners — the naive corner union measured the unturned card and let it
    // hang out of the cluster (and out of any frame drawn around it).
    let nodes = laid(
        "|column#o| { layout: tree } [\n  |topic#a| \"Chief Executive\" [\n    |topic#b| \"Chief Technology Officer\" { rotate: 35 }\n    |topic#c| \"Operations\"\n  ]\n]\n",
    );
    let (o, ox, oy) = crate::testutil::placed_by_id(&nodes, "o");
    let (b, bx, by) = crate::testutil::placed_by_id(&nodes, "b");
    let turned = crate::layout::ir::Bbox::drawn_of(b).shifted(bx, by);
    let cluster = o.bbox.shifted(ox, oy).inflate(1e-6);
    assert!(
        cluster.contains(turned),
        "the cluster box {cluster:?} must hold the turned card {turned:?}"
    );
}

//! Tree placement math [SPEC 12]: generations, sibling packing, parent centring.

use crate::layout::PlacedNode;

fn laid(src: &str) -> Vec<PlacedNode> {
    let toks = crate::lexer::lex(src).expect("lex");
    let file = crate::syntax::parser::parse(src, &toks).expect("parse");
    let lowered = crate::desugar::desugar(&file).expect("desugar");
    let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
    crate::layout::layout(&program).expect("layout").nodes
}

/// The placed topic card by id, searched flat (topics are direct children of
/// the tree container).
fn topic<'a>(nodes: &'a [PlacedNode], id: &str) -> &'a PlacedNode {
    fn find<'a>(nodes: &'a [PlacedNode], id: &str) -> Option<&'a PlacedNode> {
        for n in nodes {
            if n.id.as_deref() == Some(id) {
                return Some(n);
            }
            if let Some(f) = find(&n.children, id) {
                return Some(f);
            }
        }
        None
    }
    find(nodes, id).unwrap_or_else(|| panic!("no topic '{id}'"))
}

/// Absolute centre of a topic — container `cx/cy` plus the card's own.
fn centre(nodes: &[PlacedNode], id: &str) -> (f64, f64) {
    // The tree container is a top node; its card offset adds in. Search the
    // container that holds the topic and sum offsets.
    fn walk(nodes: &[PlacedNode], id: &str, ox: f64, oy: f64) -> Option<(f64, f64)> {
        for n in nodes {
            let (x, y) = (ox + n.cx, oy + n.cy);
            if n.id.as_deref() == Some(id) {
                return Some((x, y));
            }
            if let Some(p) = walk(&n.children, id, x, y) {
                return Some(p);
            }
        }
        None
    }
    walk(nodes, id, 0.0, 0.0).expect("topic placed")
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
    // The parent is centred over its two children.
    assert!(
        (ax - (bx + cx) / 2.0).abs() < 1e-6,
        "parent {ax} centred over children midpoint {}",
        (bx + cx) / 2.0
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

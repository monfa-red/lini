//! Scene model — dot-path → absolute rect, and per-link solidity.
//!
//! `PlacedNode.cx/cy` are parent-relative and `bbox` is node-local, so absolute
//! rects accumulate offsets down the tree. Every node is indexed: id'd shapes
//! are **bodies** (addressable, endpoint-able); anonymous children — labels
//! first among them — are **labels** (obstacles owned by their enclosing body,
//! never endpoints).

use super::rect::Rect;
use crate::layout::ir::PlacedNode;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Body,
    Label,
}

#[derive(Clone, Debug)]
pub struct SceneNode {
    pub path: String,
    pub rect: Rect,
    /// Descendant rects poking out of `rect` — a group's caption, an
    /// absolute overlay. A collapsed keep-out is `rect` plus these: what is
    /// drawn must be avoided, and only the overflow itself blocks (a hull
    /// would wall off free space beside a narrow caption).
    pub overflow: Vec<Rect>,
    pub kind: NodeKind,
    /// The enclosing scene node (`None` for a top-level node). Containment
    /// and worlds walk this chain — **structure, not paths** — so an
    /// anonymous container is as real a container as a named one.
    parent: Option<usize>,
    children: Vec<usize>,
}

/// A routing world's identity: a container's scene-node index, or `None` for
/// the scene root. Node indices are assigned in build (walk) order, so keys
/// are deterministic — Law 4 holds.
pub type WorldKey = Option<usize>;

pub struct SceneIndex {
    nodes: Vec<SceneNode>,
    roots: Vec<usize>,
    by_path: BTreeMap<String, usize>,
}

impl SceneIndex {
    pub fn build(roots: &[PlacedNode]) -> SceneIndex {
        let mut idx = SceneIndex {
            nodes: Vec::new(),
            roots: Vec::new(),
            by_path: BTreeMap::new(),
        };
        for r in roots {
            let i = idx.walk(r, "", None, 0.0, 0.0);
            idx.roots.push(i);
        }
        idx
    }

    fn walk(
        &mut self,
        n: &PlacedNode,
        prefix: &str,
        parent: Option<usize>,
        ox: f64,
        oy: f64,
    ) -> usize {
        let (cx, cy) = (ox + n.cx, oy + n.cy);
        let rect = Rect::new(
            n.bbox.min_x + cx,
            n.bbox.min_y + cy,
            n.bbox.max_x + cx,
            n.bbox.max_y + cy,
        );
        let (path, kind) = match &n.id {
            Some(id) if prefix.is_empty() => (id.clone(), NodeKind::Body),
            Some(id) => (format!("{prefix}.{id}"), NodeKind::Body),
            None => (prefix.to_owned(), NodeKind::Label),
        };
        let i = self.nodes.len();
        self.nodes.push(SceneNode {
            path: path.clone(),
            rect,
            overflow: Vec::new(),
            kind,
            parent,
            children: Vec::new(),
        });
        if kind == NodeKind::Body {
            self.by_path.insert(path.clone(), i);
        }
        let inside = |outer: Rect, r: Rect| {
            r.x0 >= outer.x0 && r.y0 >= outer.y0 && r.x1 <= outer.x1 && r.y1 <= outer.y1
        };
        for c in &n.children {
            let ci = self.walk(c, &path, Some(i), cx, cy);
            self.nodes[i].children.push(ci);
            let pokes: Vec<Rect> = std::iter::once(self.nodes[ci].rect)
                .chain(self.nodes[ci].overflow.iter().copied())
                .filter(|&r| !inside(rect, r))
                .collect();
            self.nodes[i].overflow.extend(pokes);
        }
        i
    }

    /// A body's absolute rect by full dot-path.
    pub fn rect(&self, path: &str) -> Option<Rect> {
        self.by_path.get(path).map(|&i| self.nodes[i].rect)
    }

    /// The union of every node's rect — the scene extent.
    pub fn bounds(&self) -> Rect {
        let mut rects = self.nodes.iter().map(|n| n.rect);
        let first = rects.next().unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
        rects.fold(first, |a, r| {
            Rect::new(
                a.x0.min(r.x0),
                a.y0.min(r.y0),
                a.x1.max(r.x1),
                a.y1.max(r.y1),
            )
        })
    }

    /// A body's scene-node index by full dot-path.
    pub(crate) fn node_of(&self, path: &str) -> Option<usize> {
        self.by_path.get(path).copied()
    }

    /// The enclosing container of a world (`None` = the scene root's world).
    /// The root world has no parent.
    pub(crate) fn parent_world(&self, key: WorldKey) -> Option<WorldKey> {
        key.map(|i| self.nodes[i].parent)
    }

    /// Whether the scene node `outer` is a strict structural ancestor of
    /// `inner` — named or anonymous; containment is the placed tree's, never
    /// the path string's.
    fn is_ancestor(&self, outer: usize, inner: usize) -> bool {
        let mut p = self.nodes[inner].parent;
        while let Some(i) = p {
            if i == outer {
                return true;
            }
            p = self.nodes[i].parent;
        }
        false
    }

    /// Whether `outer` **geometrically** contains `inner`: structural ancestry
    /// AND its placed rect actually enclosing the inner rect. Everywhere but a
    /// tree, nesting implies geometric containment — but a tree's branch child
    /// is a descendant placed *beside* its parent, so its parent does not
    /// enclose it, and the containment special case (world truncation, the
    /// inward port flip) must not fire for it. The conservative gate:
    /// ancestry AND geometry.
    pub fn geo_contains(&self, outer: &str, inner: &str) -> bool {
        match (self.node_of(outer), self.node_of(inner)) {
            (Some(o), Some(i)) => {
                self.is_ancestor(o, i) && {
                    let (or, ir) = (self.nodes[o].rect, self.nodes[i].rect);
                    or.x0 <= ir.x0 && or.y0 <= ir.y0 && or.x1 >= ir.x1 && or.y1 >= ir.y1
                }
            }
            _ => false,
        }
    }

    /// The routing world of a link `a → b`: the innermost container whose
    /// interior holds both ends (`None` = the scene root). An endpoint that is
    /// itself the container maps to its own interior (containment links).
    pub(crate) fn world_of(&self, a: &str, b: &str) -> WorldKey {
        let (na, nb) = match (self.node_of(a), self.node_of(b)) {
            (Some(na), Some(nb)) => (na, nb),
            _ => return None,
        };
        if self.is_ancestor(na, nb) {
            return Some(na);
        }
        if self.is_ancestor(nb, na) {
            return Some(nb);
        }
        self.common_world(a, b)
    }

    /// The innermost world equal to or enclosing both given worlds (`None` =
    /// the scene root) — the validator's shared-graph pick when two wires
    /// routed in different worlds.
    pub(crate) fn common_ancestor_world(&self, a: WorldKey, b: WorldKey) -> WorldKey {
        let mut w = a;
        loop {
            let holds_b = match (w, b) {
                (None, _) => true,
                (Some(x), Some(y)) => x == y || self.is_ancestor(x, y),
                (Some(_), None) => false,
            };
            if holds_b {
                return w;
            }
            w = self.parent_world(w).expect("Some(_) has a parent world");
        }
    }

    /// The innermost shared *ancestor* container of two endpoints (`None` =
    /// the scene root) — the world logic without the containment early-return,
    /// so a descendant its ancestor does not geometrically enclose (a tree's
    /// branch) routes in the ancestor's world, not its parent's. Anonymous
    /// ancestors count: their interiors are worlds like any container's.
    pub(super) fn common_world(&self, a: &str, b: &str) -> WorldKey {
        let (na, nb) = match (self.node_of(a), self.node_of(b)) {
            (Some(na), Some(nb)) => (na, nb),
            _ => return None,
        };
        let mut p = self.nodes[na].parent;
        while let Some(i) = p {
            if i == nb {
                // The shared ancestor is the endpoint itself only on a
                // containment pair — handled by the caller; its world is one
                // container up (equal full paths never reach here: self-loops
                // are handled before worlds).
                return self.nodes[i].parent;
            }
            if self.is_ancestor(i, nb) {
                return Some(i);
            }
            p = self.nodes[i].parent;
        }
        None
    }

    /// Every visually solid rect — labels, and bodies without body
    /// children. A container's rect covers its open interior, where links
    /// (and their labels) legitimately live, so containers are excluded
    /// while their own title labels still count. The obstacle set a link
    /// label dodges.
    pub fn obstacle_rects(&self) -> Vec<Rect> {
        self.nodes
            .iter()
            .filter(|n| {
                !n.children
                    .iter()
                    .any(|&c| self.nodes[c].kind == NodeKind::Body)
            })
            .map(|n| n.rect)
            .collect()
    }

    /// A world's own placed body (`None` for the scene root, which spans the
    /// canvas instead).
    pub(crate) fn world_rect(&self, key: WorldKey) -> Option<Rect> {
        key.map(|i| self.nodes[i].rect)
    }

    /// Direct-child rects of a world's container (`None` = the scene roots) —
    /// the keep-out set of that interior: bodies collapse their subtrees
    /// (rect plus drawn overflow), anonymous labels count as nodes.
    pub fn child_rects(&self, world: WorldKey) -> Vec<Rect> {
        let ids: &[usize] = match world {
            None => &self.roots,
            Some(i) => &self.nodes[i].children,
        };
        ids.iter()
            .flat_map(|&i| {
                std::iter::once(self.nodes[i].rect).chain(self.nodes[i].overflow.iter().copied())
            })
            .collect()
    }

    /// The solid rects a link between `endpoints` must avoid. Endpoints and
    /// their ancestors are passable (ancestors expose their interiors — labels
    /// included); every other body is solid and collapses to one rect, its
    /// subtree swallowed. A label inside an endpoint's own body is exempt.
    pub fn solid_rects_for(&self, endpoints: [&str; 2]) -> Vec<Rect> {
        let mut out = Vec::new();
        for &r in &self.roots {
            self.gather(r, endpoints, false, &mut out);
        }
        out
    }

    /// Returns whether this subtree contained a passable region, so the caller
    /// exposes its interior rather than collapsing it to one solid rect.
    fn gather(
        &self,
        i: usize,
        endpoints: [&str; 2],
        inside_endpoint: bool,
        out: &mut Vec<Rect>,
    ) -> bool {
        let n = &self.nodes[i];
        let is_endpoint =
            n.kind == NodeKind::Body && (n.path == endpoints[0] || n.path == endpoints[1]);
        let mut inner = Vec::new();
        let mut any_passable = false;
        for &c in &n.children {
            any_passable |= self.gather(c, endpoints, inside_endpoint || is_endpoint, &mut inner);
        }
        if is_endpoint || any_passable {
            out.extend(inner);
            return true;
        }
        if !(n.kind == NodeKind::Label && inside_endpoint) {
            out.push(n.rect);
            out.extend(n.overflow.iter().copied());
        }
        false
    }
}

#[cfg(test)]
mod tests {
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
}

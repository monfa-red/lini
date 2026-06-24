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
    pub kind: NodeKind,
    children: Vec<usize>,
}

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
            let i = idx.walk(r, "", 0.0, 0.0);
            idx.roots.push(i);
        }
        idx
    }

    fn walk(&mut self, n: &PlacedNode, prefix: &str, ox: f64, oy: f64) -> usize {
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
            kind,
            children: Vec::new(),
        });
        if kind == NodeKind::Body {
            self.by_path.insert(path.clone(), i);
        }
        for c in &n.children {
            let ci = self.walk(c, &path, cx, cy);
            self.nodes[i].children.push(ci);
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

    /// Whether the body at `outer` strictly contains the body at `inner`,
    /// by dot-path ancestry.
    pub fn contains(outer: &str, inner: &str) -> bool {
        inner.len() > outer.len()
            && inner.starts_with(outer)
            && inner.as_bytes()[outer.len()] == b'.'
    }

    /// The routing world of a link `a → b`: the innermost container whose
    /// interior holds both ends (`""` = the scene root). An endpoint that is
    /// itself the container maps to its own interior (containment links).
    pub fn world_of(a: &str, b: &str) -> String {
        if Self::contains(a, b) {
            return a.to_owned();
        }
        if Self::contains(b, a) {
            return b.to_owned();
        }
        let mut world = String::new();
        for (sa, sb) in a.split('.').zip(b.split('.')) {
            if sa != sb {
                break;
            }
            if !world.is_empty() {
                world.push('.');
            }
            world.push_str(sa);
        }
        // The innermost shared *segment* may be the endpoints' own parent only
        // if it is a proper ancestor of both; equal full paths never reach here
        // (self-loops are handled before worlds).
        if world == a || world == b {
            match world.rfind('.') {
                Some(i) => world.truncate(i),
                None => world.clear(),
            }
        }
        world
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

    /// Direct-child rects of the body at `path` (`""` = the scene roots) —
    /// the keep-out set of that interior: bodies collapse their subtrees,
    /// anonymous labels count as nodes.
    pub fn child_rects(&self, path: &str) -> Vec<Rect> {
        let ids: &[usize] = if path.is_empty() {
            &self.roots
        } else {
            match self.by_path.get(path) {
                Some(&i) => &self.nodes[i].children,
                None => &[],
            }
        };
        ids.iter().map(|&i| self.nodes[i].rect).collect()
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
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::ir::{Bbox, PlacedNode};
    use crate::resolve::{AttrMap, Markers, ShapeKind};
    use crate::span::Span;

    fn node(
        id: Option<&str>,
        shape: ShapeKind,
        cx: f64,
        cy: f64,
        w: f64,
        h: f64,
        children: Vec<PlacedNode>,
    ) -> PlacedNode {
        PlacedNode {
            id: id.map(String::from),
            shape,
            type_chain: Vec::new(),
            applied_styles: Vec::new(),
            label: None,
            attrs: AttrMap::default(),
            markers: Markers::default(),
            cx,
            cy,
            bbox: Bbox::centered(w, h),
            rotation: 0.0,
            children,
            dividers: Vec::new(),
            span: Span::empty(),
        }
    }

    fn rect_node(id: &str, cx: f64, cy: f64, w: f64, h: f64) -> PlacedNode {
        node(Some(id), ShapeKind::Block, cx, cy, w, h, Vec::new())
    }

    /// cat at (0,0) 40×20; garden at (100,50) 80×60 containing dog at (10,5) 30×10.
    fn scene() -> Vec<PlacedNode> {
        let dog = rect_node("dog", 10.0, 5.0, 30.0, 10.0);
        let garden = node(
            Some("garden"),
            ShapeKind::Block,
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
    fn containment_follows_dot_path_ancestry() {
        assert!(SceneIndex::contains("garden", "garden.dog"));
        assert!(SceneIndex::contains("a.b", "a.b.c.d"));
        assert!(!SceneIndex::contains("garden", "garden"));
        assert!(!SceneIndex::contains("garden", "gardenia.dog"));
    }

    #[test]
    fn world_is_the_innermost_shared_container() {
        assert_eq!(SceneIndex::world_of("cat", "garden.dog"), "");
        assert_eq!(SceneIndex::world_of("kitchen.bowl", "garden.dog"), "");
        assert_eq!(SceneIndex::world_of("garden.dog", "garden.bird"), "garden");
        assert_eq!(SceneIndex::world_of("a.b.x", "a.b.y"), "a.b");
        assert_eq!(SceneIndex::world_of("garden", "garden.dog"), "garden");
        assert_eq!(SceneIndex::world_of("garden.dog", "garden"), "garden");
        assert_eq!(SceneIndex::world_of("x.y", "x.yz"), "x");
    }

    #[test]
    fn child_rects_lists_one_collapsed_rect_per_direct_child() {
        let idx = SceneIndex::build(&scene());
        assert_eq!(
            idx.child_rects(""),
            vec![
                Rect::new(-20.0, -10.0, 20.0, 10.0),
                Rect::new(60.0, 20.0, 140.0, 80.0),
            ]
        );
        assert_eq!(
            idx.child_rects("garden"),
            vec![Rect::new(95.0, 50.0, 125.0, 60.0)]
        );
        assert_eq!(idx.child_rects("garden.dog"), Vec::new());
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
        // garden{ label, dog, bird } — linking dog→bird must avoid the label;
        // linking garden→garden must not see its own inner label.
        let label = node(None, ShapeKind::Text, 0.0, -25.0, 40.0, 10.0, Vec::new());
        let dog = rect_node("dog", -15.0, 5.0, 20.0, 10.0);
        let bird = rect_node("bird", 15.0, 5.0, 20.0, 10.0);
        let garden = node(
            Some("garden"),
            ShapeKind::Block,
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
            ShapeKind::Text,
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

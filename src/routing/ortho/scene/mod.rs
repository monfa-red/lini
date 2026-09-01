//! Scene model — dot-path → absolute rect, and per-link solidity.
//!
//! `PlacedNode.cx/cy` are parent-relative and `bbox` is node-local, so absolute
//! rects accumulate offsets down the tree. Every node is indexed: id'd shapes
//! are **bodies** (addressable, endpoint-able); anonymous children — labels
//! first among them — are **labels** (obstacles owned by their enclosing body,
//! never endpoints).
//!
//! A **schematic part** [SPEC 16.2] is the one exception, and it is an
//! identity, not a special case: a part is a scene **leaf**, folded with its
//! pins, its chrome and its fixed ports by [`parts`].
//!
//! **Generated chrome is drawn, never solid.** A `|page|`'s `|frame|`,
//! `|zone|` and `|tick|` furniture, a `|centerline|`, a `|pitch-circle|` — the
//! lines a standard always draws [SPEC 15.7] — are scaffolding *over* the
//! sheet, not bodies on it: the frame is a border the content sits inside, so
//! reading it as an obstacle would wall in every wire on the page. They still
//! count toward [`SceneIndex::bounds`] (ink the canvas must hold); they are
//! never a keep-out, a blocker, or a label's dodge.

use super::rect::Rect;
use crate::layout::ir::PlacedNode;
use crate::ledger::consts::PIN_PITCH;
use parts::Parts;
use std::collections::BTreeMap;

mod parts;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Body,
    Label,
}

#[derive(Clone, Debug)]
pub struct SceneNode {
    pub rect: Rect,
    /// Descendant rects poking out of `rect` — a group's caption, an
    /// absolute overlay. A collapsed keep-out is `rect` plus these: what is
    /// drawn must be avoided, and only the overflow itself blocks (a hull
    /// would wall off free space beside a narrow caption).
    pub overflow: Vec<Rect>,
    pub kind: NodeKind,
    /// Generated chrome [SPEC 15.7] — drawn, never solid (see the module
    /// header). Its subtree is invisible to every solidity question.
    chrome: bool,
    /// The track quantum this container's interior states, if any
    /// (ROUTING.md §Vocabulary) — a schematic scope's fine pitch [SPEC 16.1].
    quantum: Option<f64>,
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
    /// The scene root's own track quantum — the root world is keyed `None`
    /// and has no scene node to read one off ([`SceneIndex::with_root_quantum`]).
    root_quantum: Option<f64>,
    /// The sheet's fixed ports and terminal addresses ([`parts`]).
    parts: Parts,
}

impl SceneIndex {
    pub fn build(roots: &[PlacedNode]) -> SceneIndex {
        let mut idx = SceneIndex {
            nodes: Vec::new(),
            roots: Vec::new(),
            by_path: BTreeMap::new(),
            root_quantum: None,
            parts: Parts::default(),
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
        // A schematic part is a leaf whose rect is its connection frame
        // [SPEC 16.2]; everything inside it addresses the part.
        let part = crate::layout::schematic::part_ports(n);
        let box_ = part.as_ref().map_or(n.bbox, |p| p.frame);
        let rect = Rect::new(
            box_.min_x + cx,
            box_.min_y + cy,
            box_.max_x + cx,
            box_.max_y + cy,
        );
        let (path, kind) = match &n.id {
            Some(id) if prefix.is_empty() => (id.clone(), NodeKind::Body),
            Some(id) => (format!("{prefix}.{id}"), NodeKind::Body),
            None => (prefix.to_owned(), NodeKind::Label),
        };
        let i = self.nodes.len();
        self.nodes.push(SceneNode {
            rect,
            overflow: Vec::new(),
            kind,
            chrome: crate::layout::drawing::chrome::is_chrome(&n.attrs),
            quantum: crate::resolve::is_schematic(&n.attrs).then_some(PIN_PITCH),
            parent,
            children: Vec::new(),
        });
        if kind == NodeKind::Body {
            self.by_path.insert(path.clone(), i);
        }
        if let Some(part) = part {
            self.fold_part(part, n, &path, i, cx, cy);
            return i;
        }
        for c in &n.children {
            let ci = self.walk(c, &path, Some(i), cx, cy);
            self.nodes[i].children.push(ci);
            if self.nodes[ci].chrome {
                continue;
            }
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

    /// The union of every node's rect and its drawn overflow — the scene
    /// extent. Overflow counts because a folded part's chrome is no longer a
    /// node of its own; for every other node its poking descendants are nodes
    /// already, so this changes nothing there.
    pub fn bounds(&self) -> Rect {
        let mut rects = self
            .nodes
            .iter()
            .flat_map(|n| std::iter::once(n.rect).chain(n.overflow.iter().copied()));
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
                !n.chrome
                    && !n
                        .children
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

    /// The root scene's own track quantum — the root world has no scene node
    /// to read it off, so the caller supplies it.
    pub(crate) fn with_root_quantum(mut self, q: Option<f64>) -> Self {
        self.root_quantum = q;
        self
    }

    /// A world's track quantum (ROUTING.md §Vocabulary), if its scope states one.
    pub(crate) fn quantum(&self, key: WorldKey) -> Option<f64> {
        key.map_or(self.root_quantum, |i| self.nodes[i].quantum)
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
            .filter(|&&i| !self.nodes[i].chrome)
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
        // Scene-node identity, not the path string: a folded part answers to
        // every address inside it, so a wire off `u7.vs` sees its own
        // component as passable [SPEC 16.2].
        let ends = endpoints.map(|p| self.node_of(p));
        for &r in &self.roots {
            self.gather(r, ends, false, &mut out);
        }
        out
    }

    /// Returns whether this subtree contained a passable region, so the caller
    /// exposes its interior rather than collapsing it to one solid rect.
    fn gather(
        &self,
        i: usize,
        endpoints: [Option<usize>; 2],
        inside_endpoint: bool,
        out: &mut Vec<Rect>,
    ) -> bool {
        let n = &self.nodes[i];
        if n.chrome {
            return false;
        }
        let is_endpoint = endpoints[0] == Some(i) || endpoints[1] == Some(i);
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

/// A placed node's absolute rect, given its accumulated centre.
pub(super) fn abs_rect(n: &PlacedNode, cx: f64, cy: f64) -> Rect {
    Rect::new(
        n.bbox.min_x + cx,
        n.bbox.min_y + cy,
        n.bbox.max_x + cx,
        n.bbox.max_y + cy,
    )
}

/// Whether `r` sits wholly within `outer`.
pub(super) fn inside(outer: Rect, r: Rect) -> bool {
    r.x0 >= outer.x0 && r.y0 >= outer.y0 && r.x1 <= outer.x1 && r.y1 <= outer.y1
}

//! The drawing layout family [SPEC 15]. The **sketch pen** (`pen`, `geometry`)
//! folds `draw:` profiles in any layout; the **engine** (`engine`, `mates`,
//! `anchors`, `chrome`) is the `layout: drawing` scope itself — datum
//! placement, features, mates, the generated chrome; the **annotations**
//! (`annotate`, `dims`, `round`, `angle`, `leaders`, `compose`, `outline`) lower the
//! scope's measuring and leader links onto the seated geometry.

pub(crate) mod anchors;
mod angle;
mod annotate;
pub(crate) mod breaks;
pub(crate) mod chrome;
mod compose;
mod corner;
mod dims;
pub(crate) mod edges;
mod engine;
pub(crate) mod geometry;
mod leaders;
mod mates;
mod outline;
pub(crate) mod pen;
mod round;
mod threads;

pub(super) use engine::{layout_node, layout_root};

use super::ir::Bbox;
use crate::error::Error;
use crate::resolve::{AttrMap, Program, ResolvedInst, ResolvedValue};
use geometry::P;

/// A folded sketch's annotation geometry, carried on its placed node
/// [SPEC 15.2/15.6]: the authored `:segment`s (model coordinates — a
/// `break:` never moves them), the applied `mirror:` axes (the unary
/// mirrored readings), the drawn outline (leader tips ray-cast onto it —
/// displayed, clipped at any break), and the break **view map** (identity
/// without one). Everything is in the node's local frame, scaled.
pub struct SketchGeo {
    pub segments: Vec<(String, Segment)>,
    pub mirrors: Vec<geometry::MirrorAxis>,
    /// Whether the profile is a `revolve:` — the `⌀` station readings gate on
    /// it ([SPEC 15.6]; a merely mirrored span is a width, not a diameter).
    pub revolved: bool,
    /// `(segment, pitch)` per `thread:` group — a bare leader onto one of
    /// these composes its `M⌀×pitch` spec ([SPEC 15.7]).
    pub threads: Vec<(String, f64)>,
    pub outline: Vec<geometry::Subpath>,
    pub view: breaks::ViewMap,
}

/// What an authored `:segment` addresses [SPEC 15.2] — the pen's output
/// vocabulary, produced by the fold and consumed by the anchors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Segment {
    /// A freestanding name — the pen's point there.
    Point(P),
    /// A straight run (or a chamfer bevel, or a `close()` seam) — carries its
    /// direction for dimension axes.
    Edge(P, P),
    /// An arc (drawn, tangent, or a fillet) — a point on it plus its radius,
    /// the `R` reading.
    Arc { mid: P, r: f64 },
    /// A `circle(r)` subpath — round by construction, the `⌀` reading.
    Circle { center: P, r: f64 },
}

impl Segment {
    /// The segment under the node's own `scale:` — a uniform coordinate map,
    /// so directions survive and radii multiply.
    pub(super) fn scaled(self, s: f64) -> Self {
        let m = |p: P| (p.0 * s, p.1 * s);
        match self {
            Segment::Point(p) => Segment::Point(m(p)),
            Segment::Edge(a, b) => Segment::Edge(m(a), m(b)),
            Segment::Arc { mid, r } => Segment::Arc {
                mid: m(mid),
                r: r * s,
            },
            Segment::Circle { center, r } => Segment::Circle {
                center: m(center),
                r: r * s,
            },
        }
    }
}

/// `layout: drawing` [SPEC 15] — the drawing engine's dispatch check, the
/// `is_sequence` twin.
pub(crate) fn is_drawing(attrs: &AttrMap) -> bool {
    matches!(attrs.get("layout"), Some(ResolvedValue::Ident(l)) if l == "drawing")
}

/// Whether the container at `scope` is a `layout: drawing` — its links are the
/// engine's (dimensions, leaders, mates), so the router and the declared-edge
/// count skip them, exactly as a sequence scope's messages are skipped.
pub(crate) fn is_drawing_scope(program: &Program, scope: &str) -> bool {
    super::scope_attrs(program, scope).is_some_and(is_drawing)
}

/// A scene-rooted endpoint path relative to its drawing scope (`""` = root).
/// Resolve always qualifies endpoints under their scope, so the prefix is
/// exact — shared by the anchor walk and the mate error spellings.
pub(super) fn rel_path<'a>(path: &'a str, scope: &str) -> &'a str {
    path.strip_prefix(scope)
        .map(|p| p.trim_start_matches('.'))
        .unwrap_or(path)
}

/// Sheet content [SPEC 15]: placed and styled per its own type, never a part —
/// text, notes, balloons, the title footnote (tables seal via `layout: grid`).
pub(super) fn is_sheet(kind: crate::resolve::NodeKind, type_chain: &[String]) -> bool {
    kind == crate::resolve::NodeKind::Text
        || type_chain.iter().any(|t| {
            matches!(
                t.as_str(),
                "note" | "balloon" | "table" | "footnote" | "caption" | "page"
            )
        })
}

/// A part's own bbox in a drawing scope [SPEC 15.4]: `|hole|` / `|pitch-circle|`
/// are round — `width:` (required) is the diameter — and every other shape
/// sizes as a leaf: a part's features never grow it, they overhang.
pub(super) fn part_bbox(inst: &ResolvedInst, own: f64) -> Result<Bbox, Error> {
    if let Some(ty) = inst
        .type_chain
        .iter()
        .find(|t| *t == "hole" || *t == "pitch-circle")
        && !chrome::is_chrome(&inst.attrs)
    {
        let Some(w) = inst.attrs.number("width") else {
            return Err(Error::at(
                inst.span,
                format!("'|{ty}|' requires 'width' — its diameter"),
            ));
        };
        let sw = inst.attrs.number("stroke-width").unwrap_or(0.0);
        return Ok(Bbox::centered(w * own, w * own).inflate(sw / 2.0));
    }
    super::primitives::leaf_bbox(inst, own)
}

/// Datum placement [SPEC 15.1/15.4]: every child's **origin** lands on the
/// parent's datum, offset only by `translate:` (drawing units × the parent's
/// scale). A broken parent's view map acts on **every position in its
/// frame** — features, their sub-features, a pattern's copies — so the whole
/// population rides the displayed pieces [SPEC 15.3] (model positions unmap
/// in the anchor walk). Chrome children stay at the datum (their geometry is
/// filled by [`chrome::fill`]); pinned sheet content is re-placed by the
/// engine after the extent is known.
pub(super) fn place_features(
    children: &mut [super::PlacedNode],
    scale: f64,
    view: Option<&breaks::ViewMap>,
) -> Result<(), Error> {
    for c in children.iter_mut() {
        if chrome::is_chrome(&c.attrs) {
            continue;
        }
        let (dx, dy) = super::anchors::translate(&c.attrs, c.span)?.unwrap_or((0.0, 0.0));
        let m = (dx * scale, dy * scale);
        let p = match view {
            Some(v) => v.map(m),
            None => m,
        };
        c.cx = p.0;
        c.cy = p.1;
        if let Some(v) = view {
            ride_view(c, v, m, p);
        }
    }
    Ok(())
}

/// Slide a subtree's descendant positions through the broken ancestor's view
/// map: a descendant at model offset `d` in the ancestor's frame displays at
/// `map(base + d)`. Recursion stops where positions leave that frame — a
/// turned child (its interior axes no longer align with the break axis), a
/// layout-owning child (sealed, arranged by its own engine), or a child with
/// its own break (its own view world, rigid from outside); each still rides
/// as one box. Chrome stays with whatever generated it.
fn ride_view(node: &mut super::PlacedNode, v: &breaks::ViewMap, base_model: P, base_disp: P) {
    if node.rotation != 0.0
        || super::owns_layout(&node.attrs)
        || node.sketch.as_ref().is_some_and(|g| !g.view.is_identity())
    {
        return;
    }
    for c in node.children.iter_mut() {
        if chrome::is_chrome(&c.attrs) {
            continue;
        }
        let m = (base_model.0 + c.cx, base_model.1 + c.cy);
        let d = v.map(m);
        c.cx = d.0 - base_disp.0;
        c.cy = d.1 - base_disp.1;
        ride_view(c, v, m, d);
    }
    // A pattern carrier's bbox is its copies' union (`pattern::expand`) —
    // re-union it around the ridden positions.
    if node.attrs.get("pattern").is_some() {
        let mut bbox = Bbox::empty();
        for (i, c) in node.children.iter().enumerate() {
            let b = c.bbox.shifted(c.cx, c.cy);
            bbox = if i == 0 { b } else { bbox.union(b) };
        }
        node.bbox = bbox;
    }
}

/// The one compile pipeline the drawing tests drive — source → `LaidOut` (or
/// the layout error), plus the tree lookups every assertion needs.
#[cfg(test)]
pub(super) mod testutil {
    use super::super::{LaidOut, PlacedNode};
    use crate::resolve::NodeKind;

    pub fn laid(src: &str) -> LaidOut {
        let toks = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(&toks).expect("parse");
        let lowered = crate::desugar::desugar(&file).expect("desugar");
        let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
        crate::layout::layout(&program).expect("layout")
    }

    pub fn layout_err(src: &str) -> String {
        let toks = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(&toks).expect("parse");
        let lowered = crate::desugar::desugar(&file).expect("desugar");
        let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
        match crate::layout::layout(&program) {
            Ok(_) => panic!("expected a layout error"),
            Err(e) => e.message,
        }
    }

    pub fn by_id<'a>(nodes: &'a [PlacedNode], id: &str) -> &'a PlacedNode {
        fn walk<'a>(nodes: &'a [PlacedNode], id: &str) -> Option<&'a PlacedNode> {
            for n in nodes {
                if n.id.as_deref() == Some(id) {
                    return Some(n);
                }
                if let Some(hit) = walk(&n.children, id) {
                    return Some(hit);
                }
            }
            None
        }
        walk(nodes, id).unwrap_or_else(|| panic!("node '{id}' placed"))
    }

    /// Every placed text leaf, depth-first: (content, world cx, world cy,
    /// rotation).
    pub fn texts(nodes: &[PlacedNode]) -> Vec<(String, f64, f64, f64)> {
        fn walk(nodes: &[PlacedNode], ox: f64, oy: f64, out: &mut Vec<(String, f64, f64, f64)>) {
            for n in nodes {
                if n.kind == NodeKind::Text
                    && let Some(t) = &n.label
                {
                    out.push((t.clone(), ox + n.cx, oy + n.cy, n.rotation));
                }
                walk(&n.children, ox + n.cx, oy + n.cy, out);
            }
        }
        let mut out = Vec::new();
        walk(nodes, 0.0, 0.0, &mut out);
        out
    }

    /// The one text node with this content — its (cx, cy, rotation).
    pub fn text_at(nodes: &[PlacedNode], content: &str) -> (f64, f64, f64) {
        let all = texts(nodes);
        let hits: Vec<_> = all.iter().filter(|(t, ..)| t == content).collect();
        match hits.as_slice() {
            [one] => (one.1, one.2, one.3),
            _ => panic!(
                "expected one '{content}', found {}: {:?}",
                hits.len(),
                all.iter().map(|(t, ..)| t).collect::<Vec<_>>()
            ),
        }
    }
}

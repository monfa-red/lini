//! `pattern:` — replicate a node about its own position [SPEC 15.4]. A node
//! property, legal in any layout: `grid(cols, rows, dx, dy)` copies at offsets
//! (the **seed is copy one** and keeps the node's position), `radial(count,
//! radius)` puts every copy **on** the circle (the node's position is the ring
//! centre; nothing is drawn there). The call's own law — the names, arities
//! and ranges — is [`crate::resolve::pattern`]'s; what lives here is the
//! geometry and the rewrite.
//!
//! Expansion rewrites the placed node into an unpainted **carrier** that keeps
//! its identity (id, position props) and holds the copies as children — each
//! copy the node's full drawn body, children included, so a patterned `|hole|`
//! punches and centre-marks per copy with no special case. A `chrome: ring`
//! child (the radial pattern's generated `|pitch-circle|`, [SPEC 15.7]) is
//! hoisted out of the body first — one ring through the copies, not one per.
//!
//! [`carry`] is that rewrite, and it is the **only** one: `mirror:`'s reflected
//! features ([`crate::layout::mirror`]) are the same carrier from the same
//! builder, their placements simply carrying a reflection each.

use super::drawing::geometry::{MirrorAxis, P};
use super::ir::{Bbox, PlacedNode};
use crate::error::Error;
use crate::resolve::ResolvedValue;
use crate::resolve::pattern::Pattern;

/// The **replication carrier** mark [SPEC 15.4]: the copy count, written onto
/// a node whose children *are* its copies. Read through [`replicas`] alone —
/// the attr name is this module's business.
const MARK: &str = "replicas";

/// Whether a placed node is a replication carrier, and by how many copies
/// [SPEC 15.4/15.6]: a carrier **draws nothing itself** — its copies are the
/// geometry every outline, halo, anchor and bbox pass reads — and the count is
/// the `N×` prefix a dimension composes. `None` for an ordinary node.
///
/// One home, because six passes ask the question and a second producer of
/// copies (a `mirror:` reflecting a feature, [SPEC 15.3]) must answer it the
/// same way rather than teach each pass a second spelling.
pub(crate) fn replicas(node: &PlacedNode) -> Option<usize> {
    Some(node.attrs.number(MARK)? as usize)
}

/// Where one copy of a replication sits, and what its body takes on the way
/// there: the offset from the carrier's own position, plus the reflections its
/// content is turned by — none for a `pattern:` copy, one per `mirror:` item
/// for a reflected one [SPEC 15.3/15.4].
pub(super) struct Placement {
    pub at: P,
    pub reflect: Vec<MirrorAxis>,
}

/// Expand a placed node's `pattern:` in place. `scale` is the node's **own**
/// effective `scale:` — pattern offsets are part of its shape [SPEC 15.1]. The
/// carrier keeps the authored `pattern` attr and gains the [`MARK`] count;
/// expansion runs once, from `layout_inst`.
pub(super) fn expand(placed: &mut PlacedNode, scale: f64) -> Result<(), Error> {
    let Some(ResolvedValue::Call(call)) = placed.attrs.get("pattern").cloned() else {
        return Ok(());
    };
    let pattern = Pattern::read(&call, placed.span)?;
    let places: Vec<Placement> = offsets(pattern, scale)
        .into_iter()
        .map(|at| Placement {
            at,
            reflect: Vec::new(),
        })
        .collect();

    // The ring chrome stays at pattern level; everything else rides per copy.
    let (mut ring, rest): (Vec<PlacedNode>, Vec<PlacedNode>) = placed.children.drain(..).partition(
        |c| matches!(c.attrs.get("chrome"), Some(ResolvedValue::Ident(k)) if k == "ring"),
    );
    if let Some(r) = pattern.ring_radius() {
        let sw = ring
            .first()
            .and_then(|c| c.attrs.number("stroke-width"))
            .unwrap_or(0.0);
        for pc in &mut ring {
            pc.bbox = Bbox::centered(2.0 * r * scale + sw, 2.0 * r * scale + sw);
        }
    }
    placed.children = rest;
    carry(placed, &places, ring);
    Ok(())
}

/// Rewrite `placed` into a replication carrier holding one copy per placement
/// [SPEC 15.4] — the one copy loop, whichever property produced the
/// placements. `level` rides at carrier level rather than per copy (a radial
/// pattern's hoisted `|pitch-circle|`: one ring through the copies, not one
/// per). The carrier keeps its identity and position and paints nothing; the
/// copies are the geometry every later pass reads through [`replicas`].
pub(super) fn carry(placed: &mut PlacedNode, places: &[Placement], level: Vec<PlacedNode>) {
    // The drawn body: the node's own shape and paint, its features, its name —
    // everything except identity and position, which the carrier keeps.
    let mut body = PlacedNode {
        id: None,
        ..placed.clone()
    };
    body.attrs.remove("translate");
    body.attrs.remove("pin");
    body.attrs.remove("layer");
    // A copy is not a carrier: keeping `pattern` re-entered the carrier arms
    // downstream (a leader's ray-cast recursed into the copy, found only
    // chrome, and missed the rim entirely).
    body.attrs.remove("pattern");
    body.rotation = 0.0;

    let mut copies = Vec::with_capacity(places.len() + level.len());
    copies.extend(level);
    for (i, place) in places.iter().enumerate() {
        let mut copy = if i + 1 == places.len() {
            std::mem::replace(
                &mut body,
                PlacedNode {
                    ..empty_like(placed)
                },
            )
        } else {
            body.clone()
        };
        copy.cx = place.at.0;
        copy.cy = place.at.1;
        // A reflected copy is a copy whose coordinates are reflected — never a
        // node wearing a flip [SPEC 15.3], so its labels read forward and its
        // anchors stay handedness-free.
        for axis in &place.reflect {
            super::mirror::reflect_content(&mut copy, axis.dir());
        }
        copies.push(copy);
    }
    let bbox = carrier_bbox(&copies);

    // The carrier: identity + position, no paint of its own (inline `none`
    // beats the type's class rule, so the union box never draws).
    placed.children = copies;
    placed.bbox = bbox;
    placed.markers = Default::default();
    placed
        .attrs
        .insert(MARK, ResolvedValue::Number(places.len() as f64));
    placed
        .attrs
        .insert("fill", ResolvedValue::Ident("none".into()));
    placed
        .attrs
        .insert("stroke", ResolvedValue::Ident("none".into()));
    placed
        .attrs
        .insert("stroke-width", ResolvedValue::Number(0.0));
    placed.attrs.remove("path");
    placed.attrs.remove("points");
}

/// A pattern carrier's bbox — the union of its **copies**, each shifted to
/// where it sits [SPEC 15.4]. Generated chrome among the children (the radial
/// pattern's hoisted `|pitch-circle|`, sized to the ring, not to anything
/// drawn) is not a copy and never widens the box. Read at expansion and again
/// whenever the copies move (a broken ancestor slides them;
/// `drawing::ride_view`) — one reading, so the box cannot grow mid-flight.
pub(super) fn carrier_bbox<'a>(children: impl IntoIterator<Item = &'a PlacedNode>) -> Bbox {
    children
        .into_iter()
        .filter(|c| !super::drawing::chrome::is_chrome(&c.attrs))
        .fold(None, |acc: Option<Bbox>, c| {
            let b = c.bbox.shifted(c.cx, c.cy);
            Some(match acc {
                Some(a) => a.union(b),
                None => b,
            })
        })
        .unwrap_or_else(Bbox::empty)
}

/// The copy offsets from the node's own position, in px. Grid: `(i·dx, j·dy)`,
/// the seed at (0, 0); radial: on the circle, first at bearing 0, clockwise —
/// the drafting datums [SPEC 15.4]. The call itself was read into a
/// [`Pattern`] already, so this is pure geometry.
fn offsets(pattern: Pattern, scale: f64) -> Vec<(f64, f64)> {
    match pattern {
        Pattern::Grid { cols, rows, dx, dy } => {
            let mut out = Vec::with_capacity(cols * rows);
            for j in 0..rows {
                for i in 0..cols {
                    out.push((i as f64 * dx * scale, j as f64 * dy * scale));
                }
            }
            out
        }
        Pattern::Radial { count, radius } => (0..count)
            .map(|k| {
                let dir = super::drawing::geometry::bearing_dir(k as f64 * 360.0 / count as f64);
                (dir.0 * radius * scale, dir.1 * radius * scale)
            })
            .collect(),
    }
}

/// A hollow node used only as the `mem::replace` filler for the last copy.
fn empty_like(n: &PlacedNode) -> PlacedNode {
    PlacedNode {
        id: None,
        kind: n.kind,
        type_chain: Vec::new(),
        applied_styles: Vec::new(),
        label: None,
        attrs: Default::default(),
        own_style: Default::default(),
        markers: Default::default(),
        cx: 0.0,
        cy: 0.0,
        bbox: Bbox::empty(),
        rotation: 0.0,
        children: Vec::new(),
        gutters: Vec::new(),
        links: Vec::new(),
        sketch: None,
        origin: (0.0, 0.0),
        span: n.span,
    }
}

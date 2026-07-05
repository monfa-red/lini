//! Drawing anchors [SPEC 15.2], against **seated** placed geometry: a side or
//! corner sits on the node's geometry bbox (stroke excluded), `center` is its
//! centre, no point is the node's **origin** (`cap || barrel` is
//! origin-to-origin), and an authored `:name` reads the pen product the sketch
//! collected. Every hit reduces to a representative point in the drawing's
//! frame; sides and named edges additionally carry an **outward** unit normal —
//! the directed anchors a mate seats along. Rotation is honoured: a part's
//! `rotate:` turns its anchors with it.

use super::super::ir::{Bbox, PlacedNode};
use super::Product;
use super::geometry::P;
use crate::ast::Side;
use crate::error::Error;
use crate::resolve::ResolvedEndpoint;

/// A resolved anchor: the scope-level child it belongs to (what a mate moves),
/// its representative point in the drawing frame, and — for a directed
/// anchor — the outward unit normal.
pub(super) struct Hit {
    pub child: usize,
    pub point: P,
    pub outward: Option<P>,
}

/// Resolve an endpoint against the drawing's placed children. `scope` is the
/// drawing's dot-path (`""` at the root); the endpoint's path is scene-rooted.
pub(super) fn resolve(
    kids: &[PlacedNode],
    scope: &str,
    ep: &ResolvedEndpoint,
) -> Result<Hit, Error> {
    let rel = super::rel_path(&ep.path, scope);
    let mut segs = rel.split('.');
    let first = segs.next().expect("an endpoint path is non-empty");
    let child = kids
        .iter()
        .position(|k| k.id.as_deref() == Some(first))
        .ok_or_else(|| Error::at(ep.span, format!("mate endpoint '{rel}' not placed")))?;

    // Walk into features, accumulating origin and rotation — each level renders
    // as translate(cx, cy) rotate(deg), so a parent's turn carries its subtree.
    let mut node = &kids[child];
    let mut origin = (node.cx, node.cy);
    let mut rot = node.rotation;
    for seg in segs {
        let next = node
            .children
            .iter()
            .find(|c| c.id.as_deref() == Some(seg))
            .ok_or_else(|| {
                // The path resolved against the source tree, so the only placed
                // divergence is a pattern's copies [SPEC 15.4/23].
                Error::at(
                    ep.span,
                    format!("'{rel}' sits inside a 'pattern:' — per-copy features are deferred (SPEC 23)"),
                )
            })?;
        let local = rotated((next.cx, next.cy), rot);
        origin = (origin.0 + local.0, origin.1 + local.1);
        rot += next.rotation;
        node = next;
    }

    let last = rel.rsplit('.').next().expect("non-empty");
    let (local, outward) = local_anchor(node, ep, last)?;
    Ok(Hit {
        child,
        point: {
            let p = rotated(local, rot);
            (origin.0 + p.0, origin.1 + p.1)
        },
        outward: outward.map(|n| rotated(n, rot)),
    })
}

/// The anchor in the node's own frame: a representative point plus, for the
/// directed anchors (sides, named edges), the outward unit normal.
fn local_anchor(
    node: &PlacedNode,
    ep: &ResolvedEndpoint,
    node_name: &str,
) -> Result<(P, Option<P>), Error> {
    let g = geometry_box(node);
    let (cx, cy) = ((g.min_x + g.max_x) / 2.0, (g.min_y + g.max_y) / 2.0);
    if let Some(side) = ep.side {
        return Ok(match side {
            Side::Top => ((cx, g.min_y), Some((0.0, -1.0))),
            Side::Bottom => ((cx, g.max_y), Some((0.0, 1.0))),
            Side::Left => ((g.min_x, cy), Some((-1.0, 0.0))),
            Side::Right => ((g.max_x, cy), Some((1.0, 0.0))),
        });
    }
    let Some(point) = &ep.point else {
        // No anchor — the node's origin: primitives are concentric by default,
        // a sketch keeps its pen origin [SPEC 15.1].
        return Ok(((0.0, 0.0), None));
    };
    let hit = match point.as_str() {
        "center" => Some(((cx, cy), None)),
        "top-left" => Some(((g.min_x, g.min_y), None)),
        "top-right" => Some(((g.max_x, g.min_y), None)),
        "bottom-left" => Some(((g.min_x, g.max_y), None)),
        "bottom-right" => Some(((g.max_x, g.max_y), None)),
        _ => None,
    };
    if let Some(hit) = hit {
        return Ok(hit);
    }
    // An authored `:name` [SPEC 15.3]: the pen product's representative point;
    // a named edge is directed — its normal points away from the profile.
    let Some((_, product)) = node.names.iter().find(|(n, _)| n == point) else {
        let mut msg = format!("no point ':{point}' on '{node_name}'");
        let mut names: Vec<&str> = node.names.iter().map(|(n, _)| n.as_str()).collect();
        names.sort_by_key(|n| usize::abs_diff(n.len(), point.len()));
        let near: Vec<String> = names.iter().take(2).map(|n| format!("':{n}'")).collect();
        if !near.is_empty() {
            msg.push_str(&format!("; did you mean {}?", near.join(", ")));
        }
        return Err(Error::at(ep.span, msg));
    };
    Ok(match *product {
        Product::Point(p) => (p, None),
        Product::Arc { mid, .. } => (mid, None),
        Product::Circle { center, .. } => (center, None),
        Product::Edge(a, b) => {
            let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
            let len = super::geometry::dist(a, b).max(1e-9);
            let t = ((b.0 - a.0) / len, (b.1 - a.1) / len);
            // Outward = the **left of the pen's travel**: a profile drawn the
            // natural way (material on the pen's right — axis, up, across,
            // down) faces every edge outward, interior shoulders included —
            // where an away-from-centre guess would flip them.
            (mid, Some((t.1, -t.0)))
        }
    })
}

/// The node's geometry bbox — the drawn shape, stroke excluded [SPEC 15.1].
fn geometry_box(node: &PlacedNode) -> Bbox {
    let half = node.attrs.number("stroke-width").unwrap_or(0.0) / 2.0;
    node.bbox.inflate(-half)
}

fn rotated(p: P, deg: f64) -> P {
    if deg == 0.0 {
        return p;
    }
    let (s, c) = deg.to_radians().sin_cos();
    (p.0 * c - p.1 * s, p.0 * s + p.1 * c)
}

//! `pattern:` — replicate a node about its own position [SPEC 15.4]. A node
//! property, legal in any layout: `grid(cols, rows, dx, dy)` copies at offsets
//! (the **seed is copy one** and keeps the node's position), `radial(count,
//! radius)` puts every copy **on** the circle (the node's position is the ring
//! centre; nothing is drawn there).
//!
//! Expansion rewrites the placed node into an unpainted **carrier** that keeps
//! its identity (id, position props) and holds the copies as children — each
//! copy the node's full drawn body, children included, so a patterned `|hole|`
//! punches and centre-marks per copy with no special case. A `chrome: ring`
//! child (the radial pattern's generated `|pitch-circle|`, [SPEC 15.7]) is
//! hoisted out of the body first — one ring through the copies, not one per.

use super::ir::{Bbox, PlacedNode};
use crate::error::Error;
use crate::resolve::{ResolvedCall, ResolvedValue};

/// Expand a placed node's `pattern:` in place. `scale` is the node's **own**
/// effective `scale:` — pattern offsets are part of its shape [SPEC 15.1]. The
/// `pattern` attr stays on the carrier (the dimension text's `2×` count prefix
/// reads it later); expansion runs once, from `layout_inst`.
pub(super) fn expand(placed: &mut PlacedNode, scale: f64) -> Result<(), Error> {
    let Some(ResolvedValue::Call(call)) = placed.attrs.get("pattern").cloned() else {
        return Ok(());
    };
    let offsets = offsets(&call, scale, placed)?;

    // The ring chrome stays at pattern level; everything else rides per copy.
    let (mut ring, rest): (Vec<PlacedNode>, Vec<PlacedNode>) = placed.children.drain(..).partition(
        |c| matches!(c.attrs.get("chrome"), Some(ResolvedValue::Ident(k)) if k == "ring"),
    );
    if call.name == "radial"
        && let Some(r) = call.args.get(1).and_then(ResolvedValue::as_number)
    {
        let sw = ring
            .first()
            .and_then(|c| c.attrs.number("stroke-width"))
            .unwrap_or(0.0);
        for pc in &mut ring {
            pc.bbox = Bbox::centered(2.0 * r * scale + sw, 2.0 * r * scale + sw);
        }
    }

    // The drawn body: the node's own shape and paint, its features, its name —
    // everything except identity and position, which the carrier keeps.
    let mut body = PlacedNode {
        id: None,
        children: rest,
        ..placed.clone()
    };
    body.attrs.remove("translate");
    body.attrs.remove("pin");
    body.attrs.remove("layer");
    body.rotation = 0.0;

    let mut bbox = Bbox::empty();
    let mut copies = Vec::with_capacity(offsets.len() + ring.len());
    copies.extend(ring);
    for (i, &(dx, dy)) in offsets.iter().enumerate() {
        let mut copy = if i + 1 == offsets.len() {
            std::mem::replace(
                &mut body,
                PlacedNode {
                    ..empty_like(placed)
                },
            )
        } else {
            body.clone()
        };
        copy.cx = dx;
        copy.cy = dy;
        bbox = if i == 0 {
            copy.bbox.shifted(dx, dy)
        } else {
            bbox.union(copy.bbox.shifted(dx, dy))
        };
        copies.push(copy);
    }

    // The carrier: identity + position, no paint of its own (inline `none`
    // beats the type's class rule, so the union box never draws).
    placed.children = copies;
    placed.bbox = bbox;
    placed.markers = Default::default();
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
    Ok(())
}

/// The copy offsets from the node's own position, in px. Grid: `(i·dx, j·dy)`,
/// the seed at (0, 0); radial: on the circle, first at bearing 0, clockwise —
/// the drafting datums [SPEC 15.4].
fn offsets(call: &ResolvedCall, scale: f64, placed: &PlacedNode) -> Result<Vec<(f64, f64)>, Error> {
    let num = |i: usize| call.args.get(i).and_then(ResolvedValue::as_number);
    let usage = || {
        Error::at(
            placed.span,
            "'pattern' takes grid(cols, rows, dx, dy) or radial(count, radius)",
        )
    };
    match call.name.as_str() {
        "grid" => {
            let (Some(cols), Some(rows), Some(dx), Some(dy)) = (num(0), num(1), num(2), num(3))
            else {
                return Err(usage());
            };
            if cols < 1.0 || rows < 1.0 {
                return Err(Error::at(placed.span, "'grid' needs cols ≥ 1 and rows ≥ 1"));
            }
            let (cols, rows) = (cols as usize, rows as usize);
            let mut out = Vec::with_capacity(cols * rows);
            for j in 0..rows {
                for i in 0..cols {
                    out.push((i as f64 * dx * scale, j as f64 * dy * scale));
                }
            }
            Ok(out)
        }
        "radial" => {
            let (Some(count), Some(radius)) = (num(0), num(1)) else {
                return Err(usage());
            };
            if count < 2.0 || radius <= 0.0 {
                return Err(Error::at(
                    placed.span,
                    "'radial' needs count ≥ 2 and radius > 0",
                ));
            }
            let n = count as usize;
            Ok((0..n)
                .map(|k| {
                    let dir = super::drawing::geometry::bearing_dir(k as f64 * 360.0 / n as f64);
                    (dir.0 * radius * scale, dir.1 * radius * scale)
                })
                .collect())
        }
        _ => Err(usage()),
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
        names: Vec::new(),
        span: n.span,
    }
}

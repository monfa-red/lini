//! Mates & seats [SPEC 15.5] — one `||` machinery, two semantic arms split by
//! what the ends are. **Geometry ↔ geometry is a mate**: after datum
//! placement the walk moves outward from the **ground** (the first-declared
//! geometry child), each mate translating the side not yet connected, whole
//! and rigid; directed anchors (sides, named edges) must face along one axis
//! and seat flush, `gap:` apart along the shared normal (negative =
//! inserted); point anchors coincide. **A sheet-content end makes it a
//! seat**: the annotation always moves — after every mate, outside the
//! grounding graph — its seat anchor landing on the target face's point,
//! both axes. Either arm: `rotate:` turned the anchors already; the mover's
//! own `translate:` re-applies **after** — the universal post-placement
//! nudge.

use super::super::ir::PlacedNode;
use super::anchors;
use super::geometry::P;
use crate::error::Error;
use crate::resolve::{ResolvedEndpoint, ResolvedLink};
use crate::span::Span;
use std::collections::HashMap;

/// A mate-side anchor, reduced to what the seat needs — plain data, so the
/// walk can mutate `kids` after resolving both ends.
struct Hit {
    child: usize,
    point: P,
    outward: Option<P>,
}

fn hit(kids: &[PlacedNode], scope: &str, ep: &ResolvedEndpoint) -> Result<Hit, Error> {
    let a = anchors::resolve(kids, scope, ep, "mate")?;
    Ok(Hit {
        child: a.child,
        point: a.point(),
        outward: a.outward(),
    })
}

/// Run a scope's `||` statements: the mate walk, then the seats — returning
/// the seated annotation children, in seat order, for the packer's
/// annotation-obstacle registration [SPEC 15.5/15.6].
pub(super) fn seat(
    kids: &mut [PlacedNode],
    ground: usize,
    mates: &[&ResolvedLink],
    scope: &str,
    scale: f64,
) -> Result<Vec<usize>, Error> {
    // A chain (`a || b || c`) is one statement per hop, in source order; each
    // hop classifies by its ends [SPEC 15.5]: geometry ↔ geometry is a mate,
    // a sheet-content end makes it a seat, two sheet ends seat nothing.
    let mut pending: Vec<(&ResolvedLink, usize)> = Vec::new();
    let mut seats: Vec<(&ResolvedLink, usize)> = Vec::new();
    for w in mates {
        for i in 0..w.endpoints.len() - 1 {
            let (ea, eb) = (&w.endpoints[i], &w.endpoints[i + 1]);
            let a = hit(kids, scope, ea)?;
            let b = hit(kids, scope, eb)?;
            match (
                super::sheet_node(&kids[a.child]),
                super::sheet_node(&kids[b.child]),
            ) {
                (false, false) => pending.push((w, i)),
                (true, true) => {
                    return Err(Error::at(
                        w.span,
                        format!(
                            "a seat stands an annotation on geometry — '{}' seats nothing",
                            spell_pair(ea, eb, scope)
                        ),
                    ));
                }
                _ => seats.push((w, i)),
            }
        }
    }

    // Who is positioned, and by which mate (the ground by none) — the walk's
    // frontier and the over-constraint report's evidence.
    let mut seated: HashMap<usize, (Option<String>, usize)> = HashMap::new();
    seated.insert(ground, (None, 0));
    let mut seq = 1;

    while !pending.is_empty() {
        let mut progressed = false;
        let mut i = 0;
        while i < pending.len() {
            let (w, hop) = pending[i];
            let (ea, eb) = (&w.endpoints[hop], &w.endpoints[hop + 1]);
            let a = hit(kids, scope, ea)?;
            let b = hit(kids, scope, eb)?;
            if a.child == b.child {
                return Err(Error::at(
                    w.span,
                    format!(
                        "'{}' and '{}' are features of one part — a part is rigid",
                        rel(ea, scope),
                        rel(eb, scope)
                    ),
                ));
            }
            match (seated.contains_key(&a.child), seated.contains_key(&b.child)) {
                (true, true) => {
                    // Over-constrained: name the end a mate already positioned
                    // (the later seat; the ground itself is never the culprit).
                    let (xa, xb) = (&seated[&a.child], &seated[&b.child]);
                    let (child, via) = if xb.1 > xa.1 {
                        (b.child, xb)
                    } else {
                        (a.child, xa)
                    };
                    let who = kids[child].id.clone().unwrap_or_default();
                    let via = via.0.clone().unwrap_or_else(|| spell_pair(ea, eb, scope));
                    return Err(Error::at(
                        w.span,
                        format!("mate over-constrains '{who}' — already positioned via '{via}'"),
                    ));
                }
                (true, false) => {
                    let pair = spell_pair(ea, eb, scope);
                    let d = delta(kids, &a, &b, &pair, w, scale)?;
                    let moved = b.child;
                    kids[moved].cx += d.0;
                    kids[moved].cy += d.1;
                    seated.insert(moved, (Some(pair), seq));
                }
                (false, true) => {
                    let pair = spell_pair(ea, eb, scope);
                    let d = delta(kids, &b, &a, &pair, w, scale)?;
                    let moved = a.child;
                    kids[moved].cx += d.0;
                    kids[moved].cy += d.1;
                    seated.insert(moved, (Some(pair), seq));
                }
                (false, false) => {
                    i += 1;
                    continue;
                }
            }
            seq += 1;
            pending.remove(i);
            progressed = true;
        }
        if !progressed {
            // Unconnected islands ground their own first-declared node
            // [SPEC 15.5] — deterministic, source-ordered.
            let island = pending
                .iter()
                .flat_map(|(w, hop)| [&w.endpoints[*hop], &w.endpoints[*hop + 1]])
                .filter_map(|ep| hit(kids, scope, ep).ok().map(|h| h.child))
                .min()
                .expect("pending mates have endpoints");
            seated.insert(island, (None, seq));
            seq += 1;
        }
    }

    // Seats run **after** every mate, outside the grounding graph [SPEC 15.5]:
    // the annotation always moves, never grounds anything, and seats once.
    let mut placed: HashMap<usize, Span> = HashMap::new();
    let mut order = Vec::with_capacity(seats.len());
    for (w, hop) in seats {
        order.push(place_seat(kids, scope, scale, w, hop, &mut placed)?);
    }
    Ok(order)
}

/// Seat one annotation on a face [SPEC 15.5], returning the moved child. The
/// **seat anchor** — the endpoint's own, or the type default: a
/// `|surface-finish|`'s vee **tip**, everything else's **facing side** —
/// lands on the target's representative point, both axes (a seat places; a
/// mate aligns). `gap:` offsets along the target's outward normal, positive
/// = daylight; `rotate:` already turned the annotation, so the rotated
/// anchor aligns; its own `translate:` re-applies after — the lateral nudge.
fn place_seat(
    kids: &mut [PlacedNode],
    scope: &str,
    scale: f64,
    w: &ResolvedLink,
    hop: usize,
    placed: &mut HashMap<usize, Span>,
) -> Result<usize, Error> {
    let (ea, eb) = (&w.endpoints[hop], &w.endpoints[hop + 1]);
    let a = anchors::resolve(kids, scope, ea, "seat")?;
    let b = anchors::resolve(kids, scope, eb, "seat")?;
    // Operand order is irrelevant — the sheet-content end is the annotation.
    let (ann_ep, geo_ep, ann, geo) = if super::sheet_node(&kids[a.child]) {
        (ea, eb, a, b)
    } else {
        (eb, ea, b, a)
    };
    if let Some(prev) = placed.get(&ann.child) {
        let who = kids[ann.child]
            .id
            .clone()
            .unwrap_or_else(|| rel(ann_ep, scope).to_string());
        return Err(Error::at(w.span, format!("'{who}' is already seated")).with_related(*prev));
    }
    // The target supplies the face [SPEC 15.5] — a point target has no
    // outward to seat along.
    let Some(n) = geo.outward() else {
        return Err(Error::at(
            w.span,
            format!(
                "a seat needs a face — anchor a side or a named edge ('{} || {}:top')",
                rel(ann_ep, scope),
                rel(geo_ep, scope)
            ),
        ));
    };
    let gap = gap_attr(w)?.unwrap_or(0.0) * scale;
    let face = geo.point();
    let target = (face.0 + n.0 * gap, face.1 + n.1 * gap);
    let explicit = ann_ep.side.is_some() || ann_ep.point.is_some();
    let tip = super::symbols::drafting_type(&ann.node.type_chain) == Some("surface-finish");
    let seat_pt = if explicit || tip {
        // The endpoint's own anchor — or the finish default, the vee tip,
        // which *is* the node's origin (the bare endpoint's point).
        ann.point()
    } else {
        ann.facing_side_point(n)
    };
    let child = ann.child;
    let t = super::super::anchors::translate(&kids[child].attrs, w.span)?
        .map(|(x, y)| (x * scale, y * scale))
        .unwrap_or((0.0, 0.0));
    kids[child].cx += target.0 - (seat_pt.0 - t.0);
    kids[child].cy += target.1 - (seat_pt.1 - t.1);
    placed.insert(child, w.span);
    Ok(child)
}

/// The translation that seats `mover` against `fixed` [SPEC 15.5]. The seat is
/// computed from the mover's **datum-pure** position (its own `translate:`
/// re-applies after — subtracting it here and moving the placed node is the
/// same thing). `pair` is the mate as the author wrote it, for the errors.
fn delta(
    kids: &[PlacedNode],
    fixed: &Hit,
    mover: &Hit,
    pair: &str,
    w: &ResolvedLink,
    scale: f64,
) -> Result<P, Error> {
    let gap = gap_attr(w)?;
    let t = super::super::anchors::translate(&kids[mover.child].attrs, w.span)?
        .map(|(x, y)| (x * scale, y * scale))
        .unwrap_or((0.0, 0.0));
    let pm = (mover.point.0 - t.0, mover.point.1 - t.1);
    let pf = fixed.point;
    match (fixed.outward, mover.outward) {
        (Some(nf), Some(nm)) => {
            if (nf.0 * nm.1 - nf.1 * nm.0).abs() > 1e-6 {
                return Err(Error::at(
                    w.span,
                    format!(
                        "mated anchors must face along one axis — '{pair}' has no shared normal"
                    ),
                ));
            }
            let gap_px = gap.unwrap_or(0.0) * scale;
            let d = gap_px - ((pm.0 - pf.0) * nf.0 + (pm.1 - pf.1) * nf.1);
            Ok((nf.0 * d, nf.1 * d))
        }
        (None, None) => {
            if gap.is_some() {
                return Err(Error::at(
                    w.span,
                    "a point mate coincides — 'gap' needs directed anchors (sides or named edges)",
                ));
            }
            Ok((pf.0 - pm.0, pf.1 - pm.1))
        }
        _ => Err(Error::at(
            w.span,
            format!("mated anchors must face along one axis — '{pair}' has no shared normal"),
        )),
    }
}

/// The statement's `gap:` — the signed separation along the normal
/// [SPEC 15.5], one law for the mate and the seat arms.
fn gap_attr(w: &ResolvedLink) -> Result<Option<f64>, Error> {
    match w.attrs.get("gap") {
        None => Ok(None),
        Some(v) => v
            .as_number()
            .map(Some)
            .ok_or_else(|| Error::at(w.span, "a mate's 'gap' is a number")),
    }
}

fn spell_pair(a: &ResolvedEndpoint, b: &ResolvedEndpoint, scope: &str) -> String {
    format!(
        "{} || {}",
        anchors::spell(a, scope),
        anchors::spell(b, scope)
    )
}

fn rel<'a>(ep: &'a ResolvedEndpoint, scope: &str) -> &'a str {
    super::rel_path(&ep.path, scope)
}

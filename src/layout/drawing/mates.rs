//! Mates [SPEC 15.5] — `a:anchor || b:anchor` seats parts after datum
//! placement, walking outward from the **ground** (the first-declared geometry
//! child): each mate translates the side not yet connected to the ground,
//! whole and rigid. Directed anchors (sides, named edges) must face along one
//! axis and seat flush, `gap:` apart along the shared normal (negative =
//! inserted); point anchors coincide. A part's `rotate:` turned its anchors
//! already; its own `translate:` re-applies **after** the seat — the universal
//! post-placement nudge, here a lateral slide along the face.

use super::super::ir::PlacedNode;
use super::anchors;
use super::geometry::P;
use crate::error::Error;
use crate::resolve::{ResolvedEndpoint, ResolvedLink};
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

pub(super) fn seat(
    kids: &mut [PlacedNode],
    ground: usize,
    mates: &[&ResolvedLink],
    scope: &str,
    scale: f64,
) -> Result<(), Error> {
    // A chain (`a || b || c`) is one mate per hop, in source order.
    let mut pending: Vec<(&ResolvedLink, usize)> = Vec::new();
    for w in mates {
        for i in 0..w.endpoints.len() - 1 {
            pending.push((w, i));
        }
    }
    if pending.is_empty() {
        return Ok(());
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
            // A mate seats two **geometry** nodes [SPEC 15.5] — sheet content
            // (a note, a balloon, the title) is placed by its own rules.
            for hit in [&a, &b] {
                let k = &kids[hit.child];
                if super::is_sheet(k.kind, &k.type_chain) {
                    let ty = k.type_chain.first().map(String::as_str).unwrap_or("text");
                    return Err(Error::at(
                        w.span,
                        format!("a mate seats geometry — '|{ty}|' is sheet content"),
                    ));
                }
            }
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
    Ok(())
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
    let gap = match w.attrs.get("gap") {
        None => None,
        Some(v) => Some(
            v.as_number()
                .ok_or_else(|| Error::at(w.span, "a mate's 'gap' is a number"))?,
        ),
    };
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

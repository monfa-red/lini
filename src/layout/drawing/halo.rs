//! Crossing halos [SPEC 15.7]: annotation linework — dimension, extension,
//! and leader lines — **breaks** where it crosses geometry, a sheet-space
//! knockout `halo-margin` wide each side of the crossed line, mask-based so
//! the break holds over hatching and in every theme. The exclusions hold by
//! construction: the mask attaches to the **line element alone** (arrowheads,
//! text, and frames are separate nodes it never touches), a datum frame's own
//! linework is skipped, crossings are cast against **geometry only** (never
//! chrome, text, or other annotations), and a cut inside the contact zone at
//! either end of the line — a tip, a landing, an arrowhead's seat — is
//! dropped, not trimmed. The cuts lower to `lini-halo`-classed mask shapes,
//! so the `|halo|` chrome rule restyles or removes them like all chrome.

use super::super::ir::{Bbox, PlacedNode};
use super::anchors::rotated;
use super::annotate::Ctx;
use super::chrome;
use super::geometry::{P, dist, unit};
use super::outline;
use crate::ledger::consts::{DRAWING_LINK_STROKE_WIDTH, HALO_MARGIN};
use crate::resolve::{NodeKind, ResolvedValue};

/// Below this crossing-angle sine the annotation line runs along the
/// geometry rather than across it — no clean crossing, no halo.
const GRAZE: f64 = 0.1;

/// One crossing along a polyline: its station and the cut's half-length.
struct Crossing {
    s: f64,
    half: f64,
}

/// Cut every dim / extension / leader line of a lowered scope where it
/// crosses drawn geometry [SPEC 15.7]. The cuts bake into a `halo` attr (a
/// list of cut polylines in the line's own coordinates) plus a document-
/// unique `halo-id`; the renderer folds them into the line's knockout mask.
pub(super) fn apply(ctx: &Ctx, nodes: &mut [PlacedNode]) {
    let geometry: Vec<&PlacedNode> = ctx
        .kids
        .iter()
        .filter(|k| {
            !super::is_sheet(k.kind, &k.type_chain)
                && !super::super::anchors::is_pinned(&k.attrs)
                && drawn(k)
        })
        .collect();
    if geometry.is_empty() {
        return;
    }
    // The contact set [SPEC 15.7]: every arrowhead and datum triangle already
    // lowered — a tip pressing a rim, a dim arrow seated on an extension line.
    // A cut touching one's ink is dropped, whichever statement placed it.
    let contacts: Vec<Bbox> = nodes
        .iter()
        .filter(|n| n.type_chain.iter().any(|t| t == "marker"))
        .map(|n| n.bbox.shifted(n.cx, n.cy))
        .collect();
    let mut serial = 0usize;
    for node in nodes.iter_mut().filter(|n| haloable(n)) {
        let Some(points) = super::super::primitives::attr_points(&node.attrs, "points", node.span)
            .ok()
            .flatten()
        else {
            continue;
        };
        if points.len() < 2 {
            continue;
        }
        let sw = node
            .attrs
            .number("stroke-width")
            .unwrap_or(DRAWING_LINK_STROKE_WIDTH);
        let cuts = cut_chains(&points, &geometry, &contacts, sw);
        if cuts.is_empty() {
            continue;
        }
        node.attrs.insert("halo", encode(&cuts));
        node.attrs
            .insert("halo-id", ResolvedValue::String(halo_id(ctx.scope, serial)));
        serial += 1;
    }
}

/// Dim and extension linework take halos; a datum's framed box is a frame —
/// excluded by the SPEC 15.7 list — and text / markers are never line nodes.
fn haloable(n: &PlacedNode) -> bool {
    n.kind == NodeKind::Line
        && n.type_chain
            .iter()
            .any(|t| t == "dim-line" || t == "ext-line")
        && !n.type_chain.iter().any(|t| t == "datum-frame")
}

/// Whether a scope child is drawn **geometry** an annotation breaks over —
/// not chrome (a centerline, a pitch circle, a thread dressing) and not text.
fn drawn(n: &PlacedNode) -> bool {
    !chrome::is_chrome(&n.attrs)
        && n.kind != NodeKind::Text
        && !n
            .type_chain
            .iter()
            .any(|t| t == "centerline" || t == "pitch-circle")
}

/// The merged cut sub-polylines along `points` where geometry crossings land
/// clear of the line's ends and of every contact box.
fn cut_chains(points: &[P], geometry: &[&PlacedNode], contacts: &[Bbox], sw: f64) -> Vec<Vec<P>> {
    let mut stations = Vec::with_capacity(points.len());
    let mut total = 0.0;
    stations.push(0.0);
    for w in points.windows(2) {
        total += dist(w[0], w[1]);
        stations.push(total);
    }
    let mut crossings: Vec<Crossing> = Vec::new();
    for (i, w) in points.windows(2).enumerate() {
        let len = dist(w[0], w[1]);
        if len < 1e-9 {
            continue;
        }
        let d = unit((w[1].0 - w[0].0, w[1].1 - w[0].1));
        let mut hits = Vec::new();
        for g in geometry {
            node_crossings(g, w[0], d, &mut hits);
        }
        for (k, &(t, tangent, gsw, graze)) in hits.iter().enumerate() {
            if t >= len {
                continue;
            }
            // A lone endpoint graze is a corner touch, not a crossing — an
            // extension line riding an edge meets the adjoining edge's end.
            // A true vertex crossing keeps its twin (one graze per adjoining
            // segment, same station) [SPEC 15.7].
            if graze
                && !hits
                    .iter()
                    .enumerate()
                    .any(|(j, h)| j != k && (h.0 - t).abs() < 1e-6)
            {
                continue;
            }
            let sin = (d.0 * tangent.1 - d.1 * tangent.0).abs();
            if sin < GRAZE {
                continue;
            }
            crossings.push(Crossing {
                s: stations[i] + t,
                half: (gsw / 2.0 + HALO_MARGIN) / sin,
            });
        }
    }
    if crossings.is_empty() {
        return Vec::new();
    }
    crossings.sort_by(|a, b| a.s.total_cmp(&b.s));
    // Merge overlapping cuts (a corner crossing reports once per adjoining
    // segment — the merge folds the twins); a cut must lie whole within the
    // line and clear of every contact box — never over an arrowhead, a tip,
    // or a landing [SPEC 15.7].
    let mut intervals: Vec<(f64, f64)> = Vec::new();
    for c in crossings {
        let (lo, hi) = (c.s - c.half, c.s + c.half);
        match intervals.last_mut() {
            Some(last) if lo <= last.1 => last.1 = last.1.max(hi),
            _ => intervals.push((lo, hi)),
        }
    }
    intervals.retain(|&(lo, hi)| lo > HALO_MARGIN && hi < total - HALO_MARGIN);
    intervals
        .into_iter()
        .map(|(lo, hi)| chain(points, &stations, lo, hi))
        .filter(|c| {
            let reach = Bbox::from_points(c).inflate(sw / 2.0);
            !contacts.iter().any(|b| b.overlaps(reach))
        })
        .collect()
}

/// Every crossing of the world ray with a geometry node's drawn path (its
/// features and pattern copies included), as `(t, world tangent, geometry
/// stroke-width, endpoint graze)` — the ray walks each child frame like the
/// anchors do.
fn node_crossings(node: &PlacedNode, o: P, d: P, out: &mut Vec<(f64, P, f64, bool)>) {
    if !drawn(node) {
        return;
    }
    let local_o = rotated((o.0 - node.cx, o.1 - node.cy), -node.rotation);
    let local_d = rotated(d, -node.rotation);
    let sw = node.attrs.number("stroke-width").unwrap_or(2.0);
    let mut hits = Vec::new();
    outline::crossings(node, local_o, local_d, &mut hits);
    out.extend(
        hits.into_iter()
            .map(|h| (h.t, rotated(h.tangent, node.rotation), sw, h.graze)),
    );
    // A pattern carrier's copies were its own path above; anything else walks
    // its children — a part's holes, a shaft's hidden bore.
    if node.attrs.get("pattern").is_none() {
        for c in &node.children {
            let mut sub = Vec::new();
            node_crossings(c, local_o, local_d, &mut sub);
            out.extend(
                sub.into_iter()
                    .map(|(t, tan, gsw, g)| (t, rotated(tan, node.rotation), gsw, g)),
            );
        }
    }
}

/// The polyline sub-chain between stations `lo..hi`.
fn chain(points: &[P], stations: &[f64], lo: f64, hi: f64) -> Vec<P> {
    let at = |s: f64| -> P {
        for (i, w) in points.windows(2).enumerate() {
            if s <= stations[i + 1] || i + 2 == points.len() {
                let len = (stations[i + 1] - stations[i]).max(1e-9);
                let f = ((s - stations[i]) / len).clamp(0.0, 1.0);
                return (
                    w[0].0 + (w[1].0 - w[0].0) * f,
                    w[0].1 + (w[1].1 - w[0].1) * f,
                );
            }
        }
        *points.last().expect("non-empty")
    };
    let mut out = vec![at(lo)];
    for (i, p) in points.iter().enumerate().skip(1) {
        if stations[i] > lo && stations[i] < hi {
            out.push(*p);
        }
    }
    out.push(at(hi));
    out
}

/// The `halo` attr: a list of cut polylines, each a list of `(x, y)` points.
fn encode(cuts: &[Vec<P>]) -> ResolvedValue {
    ResolvedValue::List(
        cuts.iter()
            .map(|c| {
                ResolvedValue::List(
                    c.iter()
                        .map(|p| {
                            ResolvedValue::Tuple(vec![
                                ResolvedValue::Number(p.0),
                                ResolvedValue::Number(p.1),
                            ])
                        })
                        .collect(),
                )
            })
            .collect(),
    )
}

/// A document-unique mask id: the scope path (unique per drawing scope)
/// plus a per-scope serial — `lini-` reserved, so no authored id collides.
fn halo_id(scope: &str, serial: usize) -> String {
    if scope.is_empty() {
        format!("lini-halo-{serial}")
    } else {
        format!("lini-halo-{scope}-{serial}")
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::laid;
    use crate::layout::PlacedNode;
    use crate::resolve::ResolvedValue;

    /// One halo'd line: its type class and its cut chains.
    type Halo = (String, Vec<Vec<(f64, f64)>>);

    /// Every halo'd line as `(type class, its cut chains)`, walked deep.
    fn halos(nodes: &[PlacedNode]) -> Vec<Halo> {
        fn walk(nodes: &[PlacedNode], out: &mut Vec<Halo>) {
            for n in nodes {
                if let Some(ResolvedValue::List(cuts)) = n.attrs.get("halo") {
                    let chains = cuts
                        .iter()
                        .map(|c| {
                            let ResolvedValue::List(pts) = c else {
                                panic!("a cut chain");
                            };
                            pts.iter()
                                .map(|p| {
                                    let ResolvedValue::Tuple(xy) = p else {
                                        panic!("a point");
                                    };
                                    (xy[0].as_number().unwrap(), xy[1].as_number().unwrap())
                                })
                                .collect()
                        })
                        .collect();
                    out.push((n.type_chain[0].clone(), chains));
                    assert!(
                        n.attrs.get("halo-id").is_some(),
                        "a halo'd line carries its mask id"
                    );
                }
                walk(&n.children, out);
            }
        }
        let mut out = Vec::new();
        walk(nodes, &mut out);
        out
    }

    #[test]
    fn a_crossing_breaks_the_linework_and_a_clear_dim_does_not() {
        // The hole's extension line rises through the plate's top edge
        // (y = −20, geometry stroke 2): one cut, margin 2 each side of the
        // stroke — y −23..−17. The plate-side extension line rides the
        // plate's own left edge (collinear — no crossing), and the top-row
        // dim line crosses nothing.
        let l = laid(
            "{ layout: drawing; density: 1 }\n|rect#plate| { width: 100; height: 40 }\n|hole#h| { width: 8 }\nplate:left (-) h { side: top }\n",
        );
        let hs = halos(&l.nodes);
        assert_eq!(hs.len(), 1, "one crossed line: {hs:?}");
        let (class, chains) = &hs[0];
        assert_eq!(class, "ext-line");
        assert_eq!(chains.len(), 1, "{chains:?}");
        // The cut spans margin 2 each side of the stroke, in the line's own
        // travel order (anchor outward here).
        let (a, b) = (chains[0][0], chains[0][chains[0].len() - 1]);
        assert!((a.0 - 0.0).abs() < 1e-6 && (b.0 - 0.0).abs() < 1e-6);
        assert!(
            (a.1.min(b.1) - -23.0).abs() < 1e-6 && (a.1.max(b.1) - -17.0).abs() < 1e-6,
            "cut span: {a:?}..{b:?}"
        );

        // Clear of geometry: no halos at all.
        let clear = laid(
            "{ layout: drawing; density: 1 }\n|rect#plate| { width: 100; height: 40 }\nplate:left (-) plate:right { side: bottom }\n",
        );
        assert!(halos(&clear.nodes).is_empty());
    }

    #[test]
    fn no_cut_lands_on_an_arrowheads_contact() {
        // A hole's ⌀ leader runs the diameter and out through the plate's
        // right edge: the rim crossings (x = ±4) sit under the pressing
        // arrowheads — dropped — while the plate-edge crossing (x = 50)
        // breaks. [SPEC 15.7: never over arrowheads or the contact region.]
        let l = laid(
            "{ layout: drawing; density: 1 }\n|rect#plate| { width: 100; height: 40 }\n|hole#h| { width: 8 }\nh (o) { side: right }\n",
        );
        let hs = halos(&l.nodes);
        assert_eq!(hs.len(), 1, "{hs:?}");
        let chains = &hs[0].1;
        assert_eq!(chains.len(), 1, "only the plate edge breaks: {chains:?}");
        let mid = (chains[0][0].0 + chains[0][chains[0].len() - 1].0) / 2.0;
        assert!((mid - 50.0).abs() < 1e-6, "cut centred on the edge: {mid}");
    }

    #[test]
    fn chrome_and_annotations_are_not_crossed_geometry() {
        // A dim line across a centerline (chrome) and another dim's
        // extension line: neither is geometry — no halo anywhere.
        let l = laid(
            "{ layout: drawing; density: 1 }\n|rect#a| { width: 40; height: 20 }\n|rect#b| { width: 40; height: 20; translate: 60 0 }\n|centerline| { points: 30 -30, 30 30 }\na:right (-) b:left { side: bottom }\na:left (-) b:right { side: bottom }\n",
        );
        assert!(halos(&l.nodes).is_empty(), "{:?}", halos(&l.nodes));
    }
}

//! The edge-line law [SPEC 15.3]: a turned part's every **sharp** circular
//! edge — a shoulder, a groove lip, a chamfer's two edge circles — projects to
//! a straight line across the diameter. `revolve:` computes them from the
//! folded profile: at every vertex where two segments meet with a tangent
//! break, off the axis, a line runs perpendicular to the axis to the vertex's
//! reflected twin. A `fillet()` joins tangent-continuously and generates
//! nothing; vertices sharing a station draw once, at the widest span; a span
//! the profile already draws whole is skipped.
//!
//! The lines land on the `|shoulder|` chrome seed desugar generated — one clone
//! per span, so the cascade's resolved style (`|sketch| |shoulder| { … }`) rides
//! every line.

use super::super::ir::{Bbox, PlacedNode};
use super::geometry::{MirrorAxis, P, PathSeg, Subpath, arc_center, unit};
use crate::resolve::ResolvedValue;

/// Positional agreement finer than any drafting feature (px).
const EPS: f64 = 1e-3;
/// Tangency: unit directions whose dot falls below this break the tangent.
const TANGENT_DOT: f64 = 1.0 - 1e-7;

/// The edge-line spans of a revolved profile, as drawn point pairs —
/// computed on the displayed (scaled, break-clipped) subpaths, so the lines
/// ride a `break:` like everything in the sketch's frame.
pub(super) fn spans(subs: &[Subpath], axis: MirrorAxis) -> Vec<(P, P)> {
    let u = axis.dir();
    let perp = (-u.1, u.0);
    let station = |p: P| p.0 * u.0 + p.1 * u.1;
    let off = |p: P| p.0 * perp.0 + p.1 * perp.1;

    // Every tangent-break vertex off the axis, as (station, |offset|).
    let mut sharp: Vec<(f64, f64)> = Vec::new();
    for sub in subs {
        let n = sub.segs.len();
        for i in 0..n {
            if i == 0 && !sub.closed {
                continue; // an open run's free end is no joint
            }
            let prev = &sub.segs[(i + n - 1) % n];
            let next = &sub.segs[i];
            if super::geometry::dist(prev.to(), next.from()) > EPS {
                continue; // a stitched gap (a break cut) is no joint
            }
            let (a, b) = (dir_at_end(prev), dir_at_start(next));
            if a.0 * b.0 + a.1 * b.1 >= TANGENT_DOT {
                continue; // tangent — a fillet, a smooth arc join
            }
            let o = off(next.from());
            if o.abs() > EPS {
                sharp.push((station(next.from()), o.abs()));
            }
        }
    }

    // One line per station, at the widest span [SPEC 15.3].
    sharp.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    let mut grouped: Vec<(f64, f64)> = Vec::new();
    for (s, m) in sharp {
        match grouped.last_mut() {
            Some((gs, gm)) if (s - *gs).abs() <= EPS => *gm = gm.max(m),
            _ => grouped.push((s, m)),
        }
    }

    grouped
        .into_iter()
        .filter(|&(s, m)| !covered(subs, u, perp, s, m))
        .map(|(s, m)| {
            (
                (u.0 * s - perp.0 * m, u.1 * s - perp.1 * m),
                (u.0 * s + perp.0 * m, u.1 * s + perp.1 * m),
            )
        })
        .collect()
}

/// Whether the profile's own straight segments at this station already draw
/// the whole `[-m, m]` span — an end face, a fuse seam.
fn covered(subs: &[Subpath], u: P, perp: P, s: f64, m: f64) -> bool {
    let station = |p: P| p.0 * u.0 + p.1 * u.1;
    let off = |p: P| p.0 * perp.0 + p.1 * perp.1;
    let mut ivals: Vec<(f64, f64)> = Vec::new();
    for sub in subs {
        for seg in &sub.segs {
            if let PathSeg::Line { from, to } = *seg
                && (station(from) - s).abs() <= EPS
                && (station(to) - s).abs() <= EPS
            {
                let (a, b) = (off(from), off(to));
                ivals.push((a.min(b), a.max(b)));
            }
        }
    }
    ivals.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    let mut reach = -m;
    for (lo, hi) in ivals {
        if lo > reach + EPS {
            return false;
        }
        reach = reach.max(hi);
    }
    reach >= m - EPS
}

/// The outgoing unit direction at a segment's end.
fn dir_at_end(seg: &PathSeg) -> P {
    match *seg {
        PathSeg::Line { from, to } => unit((to.0 - from.0, to.1 - from.1)),
        PathSeg::Arc {
            from,
            to,
            r,
            large,
            sweep,
        } => arc_tangent(to, arc_center(from, to, r, large, sweep), sweep),
        PathSeg::Cubic { from, c1, c2, to } => {
            unit(first_nonzero(&[(to, c2), (to, c1), (to, from)]))
        }
    }
}

/// The incoming unit direction at a segment's start.
fn dir_at_start(seg: &PathSeg) -> P {
    match *seg {
        PathSeg::Line { from, to } => unit((to.0 - from.0, to.1 - from.1)),
        PathSeg::Arc {
            from,
            to,
            r,
            large,
            sweep,
        } => arc_tangent(from, arc_center(from, to, r, large, sweep), sweep),
        PathSeg::Cubic { from, c1, c2, to } => {
            unit(first_nonzero(&[(c1, from), (c2, from), (to, from)]))
        }
    }
}

/// The circle's unit tangent at `p` about `c` — SVG sweep 1 walks the
/// increasing angle (clockwise on screen, y down).
fn arc_tangent(p: P, c: P, sweep: bool) -> P {
    let v = (p.0 - c.0, p.1 - c.1);
    unit(if sweep { (-v.1, v.0) } else { (v.1, -v.0) })
}

fn first_nonzero(pairs: &[(P, P)]) -> P {
    for (a, b) in pairs {
        let d = (a.0 - b.0, a.1 - b.1);
        if d.0.hypot(d.1) > 1e-9 {
            return d;
        }
    }
    (0.0, 0.0)
}

/// Land spans on a chrome seed among a sketch's children — one clone per
/// span (each carrying the seed's cascade-resolved style); the seed itself
/// is removed, so no empty placeholder survives. `marker` picks the seed:
/// `edges` (the `|shoulder|` lines) or `thread` (the `|threadline|` minors).
pub(in crate::layout) fn fill(children: &mut Vec<PlacedNode>, marker: &str, spans: &[(P, P)]) {
    let Some(at) = children.iter().position(
        |c| matches!(c.attrs.get("chrome"), Some(ResolvedValue::Ident(k)) if k == marker),
    ) else {
        return;
    };
    let seed = children.remove(at);
    let half = seed.attrs.number("stroke-width").unwrap_or(0.0) / 2.0;
    for &(a, b) in spans.iter().rev() {
        let mut line = seed.clone();
        let point = |p: P| {
            ResolvedValue::Tuple(vec![ResolvedValue::Number(p.0), ResolvedValue::Number(p.1)])
        };
        line.attrs
            .insert("points", ResolvedValue::List(vec![point(a), point(b)]));
        line.bbox = Bbox::from_points(&[a, b]).inflate(half);
        children.insert(at, line);
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{by_id, laid, layout_err};
    use super::*;
    use crate::resolve::ResolvedValue;

    /// The placed `|shoulder|` lines under a node, as their `points:` pairs.
    fn edge_lines(nodes: &[crate::layout::PlacedNode], id: &str) -> Vec<(P, P)> {
        by_id(nodes, id)
            .children
            .iter()
            .filter(|c| c.type_chain.iter().any(|t| t == "shoulder"))
            .map(|c| {
                let Some(ResolvedValue::List(pts)) = c.attrs.get("points") else {
                    panic!("edge line has points");
                };
                let p = |v: &ResolvedValue| {
                    let ResolvedValue::Tuple(xy) = v else {
                        panic!("a point");
                    };
                    (xy[0].as_number().expect("x"), xy[1].as_number().expect("y"))
                };
                (p(&pts[0]), p(&pts[1]))
            })
            .collect()
    }

    #[test]
    fn a_step_completes_its_shoulder_line_and_a_fillet_draws_none() {
        // A stepped shaft: ⌀20 → ⌀30 at x = -10, the step corners sharp on one
        // side, filleted on the other — only the sharp station draws.
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|sketch#s| { draw: move(-50, 0) up(10) right(40) up(5) right(60) fillet(4) down(15); revolve: x-axis }\n",
        );
        let lines = edge_lines(&l.nodes, "s");
        // The step at x = -10 has two sharp vertices (offsets 10 and 15) — one
        // line at the widest span; the filleted right end generates nothing.
        assert_eq!(lines.len(), 1, "one shoulder line: {lines:?}");
        let (a, b) = lines[0];
        assert!((a.0 - -10.0).abs() < 1e-6 && (b.0 - -10.0).abs() < 1e-6);
        assert!((a.1 - -15.0).abs() < 1e-6 && (b.1 - 15.0).abs() < 1e-6);
    }

    #[test]
    fn a_chamfer_draws_its_edge_circle_and_the_face_span_is_skipped() {
        // The tie-bar end: the chamfer's big-end circle draws (nothing covers
        // that station); its small-end span coincides with the drawn end face
        // and is skipped [SPEC 15.3].
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|sketch#s| { draw: move(-50, 0) up(10) right(100) chamfer(2) down(10); revolve: x-axis }\n",
        );
        let lines = edge_lines(&l.nodes, "s");
        assert_eq!(lines.len(), 1, "only the big-end circle: {lines:?}");
        assert!((lines[0].0.0 - 48.0).abs() < 1e-6, "at the chamfer start");
    }

    #[test]
    fn a_groove_draws_both_lips_full_span() {
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|sketch#s| { draw: move(-50, 0) up(10) right(40) down(3) right(6) up(3) right(54) down(10); revolve: x-axis }\n",
        );
        let lines = edge_lines(&l.nodes, "s");
        let full: Vec<_> = lines
            .iter()
            .filter(|(a, b)| (a.1 - -10.0).abs() < 1e-6 && (b.1 - 10.0).abs() < 1e-6)
            .collect();
        assert_eq!(full.len(), 2, "both lips at full ⌀: {lines:?}");
    }

    #[test]
    fn edge_lines_scale_and_the_cascade_removes_them() {
        let l = laid(
            "{ layout: drawing; scale: 2 }\n|sketch#s| { draw: move(-50, 0) up(10) right(40) up(5) right(60) down(15); revolve: x-axis }\n",
        );
        let lines = edge_lines(&l.nodes, "s");
        assert!(
            lines.iter().any(|(a, _)| (a.0 - -20.0).abs() < 1e-6),
            "stations scale with the shape: {lines:?}"
        );
        let styled = laid(
            "{ layout: drawing; scale: 2; |sketch| |shoulder| { stroke: none } }\n|sketch#s| { draw: move(-50, 0) up(10) right(40) up(5) right(60) down(15); revolve: x-axis }\n",
        );
        let s = by_id(&styled.nodes, "s");
        let edge = s
            .children
            .iter()
            .find(|c| c.type_chain.iter().any(|t| t == "shoulder"))
            .expect("edge chrome");
        assert!(
            matches!(edge.attrs.get("stroke"), Some(ResolvedValue::Ident(k)) if k == "none"),
            "the descendant rule reaches generated chrome"
        );
    }

    #[test]
    fn revolve_and_mirror_exclude_each_other_and_the_value_is_gated() {
        assert_eq!(
            layout_err(
                "{ layout: drawing }\n|sketch#s| { draw: move(0, 0) up(5) right(10) down(5); revolve: x-axis; mirror: y-axis }\n"
            ),
            "a sketch takes 'revolve:' or 'mirror:', not both"
        );
        assert_eq!(
            layout_err(
                "{ layout: drawing }\n|sketch#s| { draw: move(0, 0) up(5) right(10) down(5); revolve: 45 }\n"
            ),
            "'revolve' takes x-axis or y-axis"
        );
    }

    #[test]
    fn a_hidden_child_is_a_dashed_unfilled_sketch() {
        // The |hidden| template [SPEC 8]: interior geometry on its own dashed
        // child, dimensionable like any feature.
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|sketch#s| { draw: move(0, 0) up(10) right(40) down(10); revolve: x-axis } [\n  |hidden#socket| { draw: move(0, 3) right(4) line(3, -3); mirror: x-axis }\n]\n",
        );
        let socket = by_id(&l.nodes, "socket");
        assert_eq!(socket.kind, crate::resolve::NodeKind::Sketch);
        assert!(
            matches!(socket.attrs.get("stroke-style"), Some(ResolvedValue::Ident(k)) if k == "dashed")
        );
        assert!(matches!(socket.attrs.get("fill"), Some(ResolvedValue::Ident(k)) if k == "none"));
        assert_eq!(socket.attrs.number("stroke-width"), Some(1.0));
    }
}

//! The independent law checker (ROUTING.md The Four Laws) — a test oracle
//! judging the drawn output alone: routed polylines, placed nodes, and the
//! engine's report. No router state is consulted. Contact is side
//! arithmetic; clearance is segment–box distance; separation is pairwise
//! segment distance, with Law 1's one excuse re-derived from scratch
//! ([`excuse::excused`]): a sub-clearance gap stands only where the contention
//! component it belongs to demonstrably cannot spread to full clearance —
//! port windows and the contract's channel model ([`ChannelGraph`] is pure
//! node geometry) grant each wire its lawful range — and nothing may ever
//! fall below half the clearance. Crossings reconcile against the report
//! both ways — every drawn crossing named, every named crossing drawn.
//!
//! Only orthogonal wires are judged: the four laws are the orthogonal
//! contract, and a `straight` wire is lawfully oblique and avoids nothing.
//! Anything found here is an engine bug (`Severity::Warning`); the checker
//! exists to catch it in CI, never to repair the routing.

use super::ortho::cost::min_pitch;
use super::ortho::rect::Rect;
use super::ortho::request::link_clearance;
use super::ortho::scene::SceneIndex;
use super::report::{Rule, Severity, Violation, cross};
use crate::ast::Side;
use crate::layout::ir::{PlacedNode, RoutedLink};
use crate::resolve::Strategy;
use crate::span::Span;
use std::collections::BTreeMap;

mod excuse;

const EPS: f64 = 1e-6;

pub fn check(nodes: &[PlacedNode], links: &[RoutedLink], report: &[Violation]) -> Vec<Violation> {
    let links: Vec<&RoutedLink> = links
        .iter()
        .filter(|w| match w.strategy {
            Strategy::Orthogonal => true,
            Strategy::Straight => false,
        })
        .collect();
    if links.is_empty() {
        return Vec::new();
    }
    // The diagram routes at the maximum clearance any link carries
    // (ROUTING.md Vocabulary), so every link is judged at that number.
    let c = links
        .iter()
        .map(|w| link_clearance(&w.attrs))
        .fold(0.0_f64, f64::max);
    let index = SceneIndex::build(nodes);
    let mut out = Vec::new();
    contact(&index, &links, c, &mut out);
    clearance(&index, &links, c, &mut out);
    separation(&index, &links, c, report, &mut out);
    self_crossing(&links, &mut out);
    out
}

fn name(w: &RoutedLink) -> String {
    format!("{} -> {}", w.seg_from, w.seg_to)
}

fn breach(rule: Rule, w: &RoutedLink, detail: String) -> Violation {
    Violation {
        rule,
        severity: Severity::Warning,
        links: vec![name(w)],
        detail,
        span: w.decl_span,
    }
}

/// Distance between two axis-aligned boxes; segments degenerate to boxes.
fn box_dist(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> f64 {
    let dx = (b.0 - a.2).max(a.0 - b.2).max(0.0);
    let dy = (b.1 - a.3).max(a.1 - b.3).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

fn seg_box(s: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    (
        s[0].0.min(s[1].0),
        s[0].1.min(s[1].1),
        s[0].0.max(s[1].0),
        s[0].1.max(s[1].1),
    )
}

fn rect_box(r: Rect) -> (f64, f64, f64, f64) {
    (r.x0, r.y0, r.x1, r.y1)
}

/// Which side the port lands on — or why the landing is illegal (Law 2:
/// on a side, perpendicular, ≥ clearance from the corners).
fn landing(rect: Rect, port: (f64, f64), inward: (f64, f64), c: f64) -> Result<Side, String> {
    let (x, y) = port;
    let on_x = x > rect.x0 + EPS && x < rect.x1 - EPS;
    let on_y = y > rect.y0 + EPS && y < rect.y1 - EPS;
    let side = if (y - rect.y0).abs() <= EPS && on_x {
        Side::Top
    } else if (x - rect.x1).abs() <= EPS && on_y {
        Side::Right
    } else if (y - rect.y1).abs() <= EPS && on_x {
        Side::Bottom
    } else if (x - rect.x0).abs() <= EPS && on_y {
        Side::Left
    } else {
        return Err("end is not on a side".to_owned());
    };
    let (margin, len, perpendicular) = match side {
        Side::Top | Side::Bottom => (
            (x - rect.x0).min(rect.x1 - x),
            rect.w(),
            (inward.0 - x).abs() <= EPS,
        ),
        _ => (
            (y - rect.y0).min(rect.y1 - y),
            rect.h(),
            (inward.1 - y).abs() <= EPS,
        ),
    };
    if margin < c.min(len / 2.0) - EPS {
        return Err(format!("end {margin} from a corner, needs ≥ {c}"));
    }
    if !perpendicular {
        return Err("oblique attachment".to_owned());
    }
    Ok(side)
}

/// One drawn end: its endpoint path, the port, and the inward point.
type End<'a> = (&'a str, (f64, f64), (f64, f64));

/// The two ends of a drawn polyline.
fn ends(w: &RoutedLink) -> [End<'_>; 2] {
    let n = w.path.len();
    [
        (w.seg_from.as_str(), w.path[0], w.path[1]),
        (w.seg_to.as_str(), w.path[n - 1], w.path[n - 2]),
    ]
}

/// Law 2 — Contact: every end on a side of its own endpoint, perpendicular,
/// clear of the corners; the polyline orthogonal throughout. Port spacing is
/// separation's job — end segments hug like any other wires.
fn contact(index: &SceneIndex, links: &[&RoutedLink], c: f64, out: &mut Vec<Violation>) {
    for w in links {
        if w.path.len() < 2 {
            out.push(breach(Rule::Contact, w, "degenerate path".to_owned()));
            continue;
        }
        if let Some(s) = w
            .path
            .windows(2)
            .find(|s| s[0].0 != s[1].0 && s[0].1 != s[1].1)
        {
            out.push(breach(Rule::Contact, w, format!("diagonal segment {s:?}")));
        }
        for (path, port, inward) in ends(w) {
            let Some(rect) = index.rect(path) else {
                out.push(breach(
                    Rule::Contact,
                    w,
                    format!("endpoint '{path}' has no placed body"),
                ));
                continue;
            };
            if let Err(why) = landing(rect, port, inward, c) {
                out.push(breach(
                    Rule::Contact,
                    w,
                    format!("{why} at {port:?} on '{path}'"),
                ));
            }
        }
    }
}

/// Law 1 — Clearance from bodies: ≥ clearance from every solid rect, and
/// from the link's own endpoints on every segment but the adjoining end
/// segment. A containment link runs inside its outer endpoint by design
/// (ROUTING.md Special nodes), so that body is skipped.
fn clearance(index: &SceneIndex, links: &[&RoutedLink], c: f64, out: &mut Vec<Violation>) {
    for w in links {
        if w.path.len() < 2 {
            continue;
        }
        let segs = w.path.len() - 1;
        let solids = index.solid_rects_for([&w.seg_from, &w.seg_to]);
        'solids: for r in &solids {
            for s in w.path.windows(2) {
                let d = box_dist(seg_box(s), rect_box(*r));
                if d < c - EPS {
                    out.push(breach(
                        Rule::Clearance,
                        w,
                        format!("segment {s:?} is {d} from a body at {r:?}, needs ≥ {c}"),
                    ));
                    break 'solids;
                }
            }
        }
        let mut bodies = vec![w.seg_from.as_str()];
        if w.seg_to != w.seg_from {
            bodies.push(w.seg_to.as_str());
        }
        for body in bodies {
            let partner: &str = if body == w.seg_from {
                &w.seg_to
            } else {
                &w.seg_from
            };
            // A geometric containment link runs inside its outer endpoint by
            // design (ROUTING.md Special nodes); a tree branch merely nested by
            // path is an ordinary wire and keeps clearance from its endpoints.
            if index.geo_contains(body, partner) {
                continue;
            }
            let Some(rect) = index.rect(body) else {
                continue;
            };
            for (k, s) in w.path.windows(2).enumerate() {
                if (k == 0 && body == w.seg_from) || (k == segs - 1 && body == w.seg_to) {
                    continue;
                }
                let d = box_dist(seg_box(s), rect_box(rect));
                if d < c - EPS {
                    out.push(breach(
                        Rule::Clearance,
                        w,
                        format!("segment {s:?} is {d} from its own endpoint '{body}', needs ≥ {c}"),
                    ));
                    break;
                }
            }
        }
    }
}

/// Law 1 (link–link) and Law 3's promise, pairwise: every segment pair of
/// two links keeps clearance, except the sanctioned contacts — transversal
/// crossings (reconciled against the report) and fan-sibling trunks (one
/// drawn line until the split). A gap below clearance stands only on a
/// scarcity excuse ([`excused`]); a gap below half the clearance never
/// stands (the relief valve's floor).
fn separation(
    index: &SceneIndex,
    links: &[&RoutedLink],
    c: f64,
    report: &[Violation],
    out: &mut Vec<Violation>,
) {
    let mut drawn: BTreeMap<(String, String), Vec<(f64, f64)>> = BTreeMap::new();
    for i in 0..links.len() {
        for j in i + 1..links.len() {
            let (a, b) = (links[i], links[j]);
            let fan_pair = [a.fan_from, a.fan_to]
                .iter()
                .flatten()
                .any(|g| [b.fan_from, b.fan_to].contains(&Some(*g)));
            let mut offence: Option<String> = None;
            for (sk, sa) in a.path.windows(2).enumerate() {
                for (tk, sb) in b.path.windows(2).enumerate() {
                    if let Some(at) = cross(sa, sb) {
                        drawn.entry(pair_key(a, b)).or_default().push(at);
                        continue;
                    }
                    let d = box_dist(seg_box(sa), seg_box(sb));
                    if d >= c - EPS || (fan_pair && trunk_contact(sa, sb, a, b, d)) {
                        continue;
                    }
                    if d < min_pitch(c) - EPS {
                        offence.get_or_insert_with(|| {
                            format!(
                                "segments {sa:?} and {sb:?} are {d} apart, \
                                 below the half-clearance floor {}",
                                min_pitch(c)
                            )
                        });
                    } else if !excuse::excused(index, links, (i, sk, sa), (j, tk, sb), c) {
                        offence.get_or_insert_with(|| {
                            format!(
                                "segments {sa:?} and {sb:?} are {d} apart with room \
                                 for full clearance {c}"
                            )
                        });
                    }
                }
            }
            if let Some(detail) = offence {
                out.push(Violation {
                    rule: Rule::Separation,
                    severity: Severity::Warning,
                    links: vec![name(a), name(b)],
                    detail,
                    span: b.decl_span,
                });
            }
        }
    }
    reconcile(links, &drawn, report, out);
}

/// Fan-sibling contact that is the shared trunk rather than a braid: an
/// outright overlap or touch, a segment lying on the partner's polyline
/// (the trunk is one drawn line, so the partner extends it), or corner
/// adjacency where staggered branch points peel off the trunk. Siblings
/// running alongside each other past the split still breach.
fn trunk_contact(
    sa: &[(f64, f64)],
    sb: &[(f64, f64)],
    a: &RoutedLink,
    b: &RoutedLink,
    d: f64,
) -> bool {
    let on = |s: &[(f64, f64)], path: &[(f64, f64)]| path.windows(2).any(|t| lies_on(t, s));
    d <= EPS || on(sb, &a.path) || on(sa, &b.path) || break_out(sa, sb)
}

/// Whether `s` is collinear with and contained in `t`.
fn lies_on(t: &[(f64, f64)], s: &[(f64, f64)]) -> bool {
    let (tb, sb) = (seg_box(t), seg_box(s));
    let along_x = tb.1 == tb.3 && sb.1 == sb.3 && (tb.1 - sb.1).abs() <= EPS;
    let along_y = tb.0 == tb.2 && sb.0 == sb.2 && (tb.0 - sb.0).abs() <= EPS;
    (along_x || along_y)
        && sb.0 >= tb.0 - EPS
        && sb.1 >= tb.1 - EPS
        && sb.2 <= tb.2 + EPS
        && sb.3 <= tb.3 + EPS
}

/// Parallel segments whose extents along the travel axis at most touch —
/// branch runs leaving a shared trunk in opposite or staggered directions.
fn break_out(sa: &[(f64, f64)], sb: &[(f64, f64)]) -> bool {
    let (ab, bb) = (seg_box(sa), seg_box(sb));
    let (a_horizontal, b_horizontal) = (ab.1 == ab.3, bb.1 == bb.3);
    if a_horizontal != b_horizontal {
        return false;
    }
    if a_horizontal {
        ab.2.min(bb.2) - ab.0.max(bb.0) <= EPS
    } else {
        ab.3.min(bb.3) - ab.1.max(bb.1) <= EPS
    }
}

/// Law 3's blind spot made checkable: a link crossing **itself** is a
/// crossing the report cannot even name — always an engine bug.
fn self_crossing(links: &[&RoutedLink], out: &mut Vec<Violation>) {
    for w in links {
        let segs: Vec<_> = w.path.windows(2).collect();
        for (i, sa) in segs.iter().enumerate() {
            for sb in segs.iter().skip(i + 1) {
                if let Some(at) = cross(sa, sb) {
                    out.push(breach(
                        Rule::Crossing,
                        w,
                        format!("link crosses itself at {at:?}"),
                    ));
                }
            }
        }
    }
}

fn pair_key(a: &RoutedLink, b: &RoutedLink) -> (String, String) {
    let (x, y) = (name(a), name(b));
    if x <= y { (x, y) } else { (y, x) }
}

/// Law 3: "the report counts every drawn crossing" — a crossing the report
/// doesn't name is a bug, and a named crossing that is not drawn is the
/// same bug mirrored.
fn reconcile(
    links: &[&RoutedLink],
    drawn: &BTreeMap<(String, String), Vec<(f64, f64)>>,
    report: &[Violation],
    out: &mut Vec<Violation>,
) {
    let mut reported: BTreeMap<(String, String), (usize, Span)> = BTreeMap::new();
    for v in report.iter().filter(|v| v.rule == Rule::Crossing) {
        let [a, b] = v.links.as_slice() else {
            continue;
        };
        let key = if a <= b {
            (a.clone(), b.clone())
        } else {
            (b.clone(), a.clone())
        };
        reported.entry(key).or_insert((0, v.span)).0 += 1;
    }
    let span_of = |key: &(String, String)| {
        links
            .iter()
            .find(|w| name(w) == key.0 || name(w) == key.1)
            .map_or(Span::empty(), |w| w.decl_span)
    };
    for (key, points) in drawn {
        let named = reported.get(key).map_or(0, |(n, _)| *n);
        if points.len() > named {
            out.push(Violation {
                rule: Rule::Crossing,
                severity: Severity::Warning,
                links: vec![key.0.clone(), key.1.clone()],
                detail: format!(
                    "{} crossing(s) drawn but {named} named in the report (first at {:?})",
                    points.len(),
                    points[0],
                ),
                span: span_of(key),
            });
        }
    }
    for (key, (named, span)) in &reported {
        let count = drawn.get(key).map_or(0, Vec::len);
        if *named > count {
            out.push(Violation {
                rule: Rule::Crossing,
                severity: Severity::Warning,
                links: vec![key.0.clone(), key.1.clone()],
                detail: format!("{named} crossing(s) named in the report but {count} drawn"),
                span: *span,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::ir::Bbox;
    use crate::resolve::{AttrMap, Markers, NodeKind, ResolvedValue};

    fn sized(id: &str, cx: f64, cy: f64, w: f64, h: f64) -> PlacedNode {
        PlacedNode {
            id: Some(id.to_owned()),
            kind: NodeKind::Block,
            type_chain: Vec::new(),
            applied_styles: Vec::new(),
            label: None,
            attrs: AttrMap::default(),
            own_style: AttrMap::default(),
            markers: Markers::default(),
            cx,
            cy,
            bbox: Bbox::centered(w, h),
            rotation: 0.0,
            children: Vec::new(),
            gutters: Vec::new(),
            links: Vec::new(),
            sketch: None,
            origin: (0.0, 0.0),
            span: Span::empty(),
        }
    }

    fn body(id: &str, cx: f64, cy: f64) -> PlacedNode {
        sized(id, cx, cy, 40.0, 40.0)
    }

    fn link(from: &str, to: &str, path: Vec<(f64, f64)>) -> RoutedLink {
        let mut attrs = AttrMap::default();
        attrs.insert("clearance", ResolvedValue::Number(8.0));
        RoutedLink {
            path,
            strategy: Strategy::Orthogonal,
            markers: Markers::default(),
            attrs,
            applied_styles: Vec::new(),
            texts: Vec::new(),
            data_from: from.to_owned(),
            data_to: to.to_owned(),
            seg_from: from.to_owned(),
            seg_to: to.to_owned(),
            decl_span: Span::empty(),
            fan_from: None,
            fan_to: None,
        }
    }

    fn rules(violations: &[Violation]) -> Vec<Rule> {
        violations.iter().map(|v| v.rule).collect()
    }

    /// a at origin, b to the right, both 40×40.
    fn pair() -> Vec<PlacedNode> {
        vec![body("a", 0.0, 0.0), body("b", 200.0, 0.0)]
    }

    #[test]
    fn a_clean_straight_link_is_silent() {
        let w = link("a", "b", vec![(20.0, 0.0), (180.0, 0.0)]);
        let out = check(&pair(), &[w], &[]);
        assert_eq!(out.len(), 0, "{out:?}");
    }

    #[test]
    fn a_straight_strategy_wire_is_exempt_from_the_laws() {
        // Oblique, corner-grazing, avoidance-free — lawful for `straight`
        // (ROUTING.md Strategies), so the orthogonal checker keeps silent.
        let mut w = link("a", "b", vec![(20.0, 20.0), (180.0, -20.0)]);
        w.strategy = Strategy::Straight;
        let out = check(&pair(), &[w], &[]);
        assert_eq!(out.len(), 0, "{out:?}");
    }

    #[test]
    fn clearance_fires_on_a_grazing_segment() {
        // The detour passes 4 over the blocking body — clearance is 8.
        let nodes = vec![
            body("a", 0.0, 0.0),
            body("b", 200.0, 0.0),
            body("wall", 100.0, 0.0),
        ];
        let w = link(
            "a",
            "b",
            vec![
                (20.0, 0.0),
                (50.0, 0.0),
                (50.0, -24.0),
                (150.0, -24.0),
                (150.0, 0.0),
                (180.0, 0.0),
            ],
        );
        let out = check(&nodes, &[w], &[]);
        assert!(rules(&out).contains(&Rule::Clearance), "{out:?}");
    }

    #[test]
    fn clearance_fires_inside_the_links_own_keepout() {
        // A middle segment sweeps back 4 over its own source body.
        let w = link(
            "a",
            "b",
            vec![
                (20.0, 0.0),
                (60.0, 0.0),
                (60.0, -24.0),
                (0.0, -24.0),
                (0.0, -60.0),
                (240.0, -60.0),
                (240.0, 0.0),
                (220.0, 0.0),
            ],
        );
        let out = check(&pair(), &[w], &[]);
        assert!(rules(&out).contains(&Rule::Clearance), "{out:?}");
    }

    #[test]
    fn contact_fires_on_corner_oblique_and_diagonal_landings() {
        let corner = link("a", "b", vec![(20.0, -20.0), (180.0, -20.0)]);
        let graze = link("a", "b", vec![(20.0, -15.0), (180.0, -15.0)]);
        let oblique = link(
            "a",
            "b",
            vec![(20.0, 0.0), (20.0, -40.0), (180.0, -40.0), (180.0, 0.0)],
        );
        let diagonal = link("a", "b", vec![(20.0, 0.0), (170.0, -10.0), (180.0, 0.0)]);
        for w in [corner, graze, oblique, diagonal] {
            let out = check(&pair(), &[w], &[]);
            assert!(rules(&out).contains(&Rule::Contact), "{out:?}");
        }
    }

    #[test]
    fn separation_fires_below_the_half_clearance_floor() {
        // Two rails 3 apart: below clearance/2 = 4 — no excuse exists.
        let nodes = vec![
            sized("a", 0.0, 0.0, 40.0, 100.0),
            sized("b", 200.0, 0.0, 40.0, 100.0),
        ];
        let w1 = link("a", "b", vec![(20.0, 0.0), (180.0, 0.0)]);
        let w2 = link("a", "b", vec![(20.0, 3.0), (180.0, 3.0)]);
        let out = check(&nodes, &[w1, w2], &[]);
        assert!(
            out.iter()
                .any(|v| v.rule == Rule::Separation && v.detail.contains("floor")),
            "{out:?}"
        );
    }

    #[test]
    fn a_squeeze_with_room_to_spare_is_flagged() {
        // Five rails at pitch 5 between 100-tall boxes: the shared window
        // (84) and the corridor both hold five wires at full clearance, so
        // the sub-clearance hug has no excuse.
        let nodes = vec![
            sized("a", 0.0, 0.0, 40.0, 100.0),
            sized("b", 200.0, 0.0, 40.0, 100.0),
        ];
        let links: Vec<RoutedLink> = (0..5)
            .map(|i| {
                let y = -10.0 + 5.0 * i as f64;
                link("a", "b", vec![(20.0, y), (180.0, y)])
            })
            .collect();
        let out = check(&nodes, &links, &[]);
        assert!(rules(&out).contains(&Rule::Separation), "{out:?}");
    }

    #[test]
    fn a_full_side_excuses_its_compressed_ports() {
        // Four rails at the pitch floor between 28-tall boxes: the lawful
        // window is 28 − 2·8 = 12, four ports at full clearance need 24 —
        // the side cannot hold them, so the compression stands.
        let nodes = vec![
            sized("a", 0.0, 0.0, 40.0, 28.0),
            sized("b", 200.0, 0.0, 40.0, 28.0),
        ];
        let links: Vec<RoutedLink> = (0..4)
            .map(|i| {
                let y = -6.0 + 4.0 * i as f64;
                link("a", "b", vec![(20.0, y), (180.0, y)])
            })
            .collect();
        let out = check(&nodes, &links, &[]);
        assert_eq!(out.len(), 0, "{out:?}");
    }

    #[test]
    fn a_pinched_corridor_excuses_the_compression() {
        // Two wires drop through a 4-wide slot between two tall walls —
        // their vertical legs 4 apart (the floor, exactly). The corridor's
        // usable width cannot hold two wires at clearance 8: excused.
        let mut nodes = vec![
            sized("ww", -35.0, 0.0, 50.0, 200.0),
            sized("we", 35.0, 0.0, 50.0, 200.0),
        ];
        nodes.push(sized("a1", -60.0, -150.0, 40.0, 20.0));
        nodes.push(sized("a2", 50.0, -150.0, 40.0, 20.0));
        nodes.push(sized("b1", -60.0, 150.0, 40.0, 20.0));
        nodes.push(sized("b2", 50.0, 150.0, 40.0, 20.0));
        let w1 = link(
            "a1",
            "b1",
            vec![
                (-40.0, -150.0),
                (-2.0, -150.0),
                (-2.0, 150.0),
                (-40.0, 150.0),
            ],
        );
        let w2 = link(
            "a2",
            "b2",
            vec![(30.0, -150.0), (2.0, -150.0), (2.0, 150.0), (30.0, 150.0)],
        );
        let out = check(&nodes, &[w1, w2], &[]);
        assert_eq!(out.len(), 0, "{out:?}");
    }

    #[test]
    fn the_same_hug_in_a_roomy_corridor_is_flagged() {
        // Identical wires, walls pulled apart to a 36-wide slot: room for
        // both at clearance, so the 4-gap is an engine bug.
        let mut nodes = vec![
            sized("ww", -51.0, 0.0, 50.0, 200.0),
            sized("we", 51.0, 0.0, 50.0, 200.0),
        ];
        nodes.push(sized("a1", -60.0, -150.0, 40.0, 20.0));
        nodes.push(sized("a2", 50.0, -150.0, 40.0, 20.0));
        nodes.push(sized("b1", -60.0, 150.0, 40.0, 20.0));
        nodes.push(sized("b2", 50.0, 150.0, 40.0, 20.0));
        let w1 = link(
            "a1",
            "b1",
            vec![
                (-40.0, -150.0),
                (-2.0, -150.0),
                (-2.0, 150.0),
                (-40.0, 150.0),
            ],
        );
        let w2 = link(
            "a2",
            "b2",
            vec![(30.0, -150.0), (2.0, -150.0), (2.0, 150.0), (30.0, 150.0)],
        );
        let out = check(&nodes, &[w1, w2], &[]);
        assert!(rules(&out).contains(&Rule::Separation), "{out:?}");
    }

    #[test]
    fn crossings_reconcile_against_the_report_both_ways() {
        let nodes = vec![
            body("a", 0.0, 0.0),
            body("b", 200.0, 0.0),
            body("c", 100.0, -100.0),
            body("d", 100.0, 100.0),
        ];
        let w1 = link("a", "b", vec![(20.0, 0.0), (180.0, 0.0)]);
        let w2 = link("c", "d", vec![(100.0, -80.0), (100.0, 80.0)]);
        let entry = |links: Vec<String>| Violation {
            rule: Rule::Crossing,
            severity: Severity::Info,
            links,
            detail: String::new(),
            span: Span::empty(),
        };

        // Drawn but unnamed: the checker flags the crossing.
        let out = check(&nodes, &[w1.clone(), w2.clone()], &[]);
        assert!(
            out.iter()
                .any(|v| v.rule == Rule::Crossing && v.severity == Severity::Warning),
            "{out:?}"
        );

        // Named exactly once: silent.
        let named = entry(vec!["a -> b".to_owned(), "c -> d".to_owned()]);
        let out = check(
            &nodes,
            &[w1.clone(), w2.clone()],
            std::slice::from_ref(&named),
        );
        assert_eq!(out.len(), 0, "{out:?}");

        // Named but not drawn: the phantom is flagged.
        let phantom = entry(vec!["a -> b".to_owned(), "x -> y".to_owned()]);
        let out = check(&nodes, &[w1, w2], &[named, phantom]);
        assert!(
            out.iter()
                .any(|v| v.detail.contains("named in the report but")),
            "{out:?}"
        );
    }

    #[test]
    fn a_link_crossing_itself_is_flagged() {
        // A hook whose final approach sweeps back through the link's own run.
        let w = link(
            "a",
            "b",
            vec![
                (20.0, 0.0),
                (60.0, 0.0),
                (60.0, 60.0),
                (230.0, 60.0),
                (230.0, -9.0),
                (239.0, -9.0),
                (239.0, 0.0),
                (180.0, 0.0),
            ],
        );
        let out = check(&pair(), &[w], &[]);
        assert!(
            out.iter().any(|v| v.detail.contains("crosses itself")),
            "{out:?}"
        );
    }

    #[test]
    fn fan_siblings_share_their_trunk_without_separation_noise() {
        let nodes = vec![
            body("a", 0.0, 0.0),
            body("b", 200.0, 0.0),
            body("c", 100.0, 160.0),
        ];
        let mut w1 = link("a", "b", vec![(20.0, 0.0), (180.0, 0.0)]);
        let mut w2 = link("a", "c", vec![(20.0, 0.0), (100.0, 0.0), (100.0, 140.0)]);
        // Untagged, the trunk overlap and split T-joint breach separation…
        let out = check(&nodes, &[w1.clone(), w2.clone()], &[]);
        assert!(rules(&out).contains(&Rule::Separation), "{out:?}");
        // …as fan siblings they are one drawn line.
        w1.fan_from = Some(0);
        w2.fan_from = Some(0);
        let out = check(&nodes, &[w1, w2], &[]);
        assert_eq!(out.len(), 0, "{out:?}");
    }
}

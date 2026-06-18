//! The independent four-law checker (WIRING §The Four Laws) — judgeable on
//! the output alone: routed polylines, placed nodes, and the engine's report.
//!
//! No router knowledge: clearance and separation are segment-distance
//! arithmetic, contact is side arithmetic, and crossings are reconciled
//! against the report — every drawn crossing named, every named crossing
//! drawn. Anything found here is an engine bug (`Severity::Warning`); the
//! checker exists to catch it in CI, never to patch the routing.

use super::audit;
use super::bundle::wire_clearance;
use super::rect::Rect;
use super::scene::SceneIndex;
use super::{Rule, Severity, Violation};
use crate::layout::ir::{PlacedNode, RoutedWire};
use crate::resolve::VarTable;
use crate::span::Span;
use std::collections::BTreeMap;

const EPS: f64 = 1e-6;

pub fn check(
    nodes: &[PlacedNode],
    wires: &[RoutedWire],
    report: &[Violation],
    vars: &VarTable,
) -> Vec<Violation> {
    if wires.is_empty() {
        return Vec::new();
    }
    let c = wires
        .iter()
        .map(|w| wire_clearance(&w.attrs, vars))
        .fold(0.0_f64, f64::max);
    let index = SceneIndex::build(nodes);
    let mut out = Vec::new();
    contact(&index, wires, c, &mut out);
    clearance(&index, wires, c, &mut out);
    separation(&index, wires, c, report, &mut out);
    self_crossing(wires, &mut out);
    out
}

/// Law 3's blind spot made checkable: a wire crossing **itself** is a
/// crossing the report cannot even name — always an engine bug, never
/// counted output.
fn self_crossing(wires: &[RoutedWire], out: &mut Vec<Violation>) {
    for w in wires {
        let segs: Vec<_> = w.path.windows(2).collect();
        for (i, sa) in segs.iter().enumerate() {
            for sb in segs.iter().skip(i + 1) {
                if let Some(at) = audit::cross(sa, sb) {
                    out.push(breach(
                        Rule::Crossing,
                        w,
                        format!("wire crosses itself at {at:?}"),
                    ));
                }
            }
        }
    }
}

fn name(w: &RoutedWire) -> String {
    format!("{} -> {}", w.seg_from, w.seg_to)
}

fn breach(rule: Rule, w: &RoutedWire, detail: String) -> Violation {
    Violation {
        rule,
        severity: Severity::Warning,
        wires: vec![name(w)],
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

/// Law 2 — Contact: every end on a side, perpendicular, clear of corners;
/// ports sharing a side evenly spaced ≥ clearance apart, median on the side's
/// centre. A side past its capacity **compacts** (the compaction clause): a
/// sub-clearance pitch is excused only by genuine overflow — more ports
/// than the side holds at clearance — and must be uniform at exactly the
/// widest pitch the side allows. Orthogonality rides along: an oblique
/// segment voids every law.
fn contact(index: &SceneIndex, wires: &[RoutedWire], c: f64, out: &mut Vec<Violation>) {
    let mut sides: BTreeMap<(String, u8), Vec<(f64, usize)>> = BTreeMap::new();
    for (wi, w) in wires.iter().enumerate() {
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
        let n = w.path.len();
        let ends = [
            (&w.seg_from, w.path[0], w.path[1]),
            (&w.seg_to, w.path[n - 1], w.path[n - 2]),
        ];
        for (path, port, inward) in ends {
            let Some(rect) = index.rect(path) else {
                out.push(breach(
                    Rule::Contact,
                    w,
                    format!("endpoint '{path}' has no placed body"),
                ));
                continue;
            };
            match landing(rect, port, inward, c) {
                Ok(side) => sides
                    .entry((path.clone(), side))
                    .or_default()
                    .push((ord_on(side, port), wi)),
                Err(why) => out.push(breach(
                    Rule::Contact,
                    w,
                    format!("{why} at {port:?} on '{path}'"),
                )),
            }
        }
    }

    for ((path, side), mut ports) in sides {
        ports.sort_by(|a, b| a.0.total_cmp(&b.0));
        let members: Vec<usize> = ports.iter().map(|p| p.1).collect();
        // Fan siblings share one port; identical ordinates are one slot.
        ports.dedup_by(|a, b| (a.0 - b.0).abs() <= EPS);
        let rect = index.rect(&path).expect("landed ends have a body");
        let centre = match side {
            0 | 2 => (rect.x0 + rect.x1) / 2.0,
            _ => (rect.y0 + rect.y1) / 2.0,
        };
        let mut flag = |wi: usize, detail: String| {
            out.push(breach(
                Rule::Contact,
                &wires[wi],
                format!("{detail} on '{path}'"),
            ));
        };
        let gaps: Vec<f64> = ports.windows(2).map(|p| p[1].0 - p[0].0).collect();
        if ports.len() > side_capacity(rect, side, c) {
            let pitch = side_usable(rect, side, c) / (ports.len() as f64 - 1.0);
            if let Some(g) = gaps.iter().find(|g| (**g - pitch).abs() > EPS) {
                flag(
                    ports[0].1,
                    format!("compacted ports {g} apart, the side's even pitch is {pitch}"),
                );
            }
        } else if let Some(g) = gaps.iter().find(|g| **g < c - EPS) {
            flag(ports[0].1, format!("ports {g} apart, need ≥ {c}"));
        } else if gaps.windows(2).any(|g| (g[1] - g[0]).abs() > EPS) {
            flag(ports[0].1, "ports unevenly spaced".to_owned());
        }
        // Law 2's centred median binds sides holding two or more ports; a
        // lone port is free along its side (it aligns with its wire).
        let median = (ports[0].0 + ports[ports.len() - 1].0) / 2.0;
        if ports.len() > 1
            && (median - centre).abs() > EPS
            && !slide_excused(wires, &ports, &members, rect, side, centre - median, c)
        {
            flag(
                ports[0].1,
                format!("port median {median} off the side's centre {centre}"),
            );
        }
    }
}

/// Law 2's slide clause: a port group may sit off its side's centre only
/// when the centred rows are unavailable — some port, slid back by `shift`,
/// would come nearer than clearance to a wire outside the group. Checkable
/// on the output alone.
fn slide_excused(
    wires: &[RoutedWire],
    ports: &[(f64, usize)],
    members: &[usize],
    rect: Rect,
    side: u8,
    shift: f64,
    c: f64,
) -> bool {
    ports.iter().any(|&(o, _)| {
        let p = match side {
            0 => (o + shift, rect.y0),
            1 => (rect.x1, o + shift),
            2 => (o + shift, rect.y1),
            _ => (rect.x0, o + shift),
        };
        wires.iter().enumerate().any(|(wj, w)| {
            !members.contains(&wj)
                && w.path
                    .windows(2)
                    .any(|s| box_dist((p.0, p.1, p.0, p.1), seg_box(s)) < c - EPS)
        })
    })
}

/// The port's ordinate along its side: x on horizontal sides, y on vertical.
fn ord_on(side: u8, port: (f64, f64)) -> f64 {
    match side {
        0 | 2 => port.0,
        _ => port.1,
    }
}

/// A side's extent along its own axis (0/2 horizontal → width).
fn side_extent(rect: Rect, side: u8) -> f64 {
    match side {
        0 | 2 => rect.w(),
        _ => rect.h(),
    }
}

fn side_usable(rect: Rect, side: u8, c: f64) -> f64 {
    (side_extent(rect, side) - 2.0 * c).max(0.0)
}

/// Law 2's side capacity: `floor((len − 2c)/c) + 1`, minimum 1.
fn side_capacity(rect: Rect, side: u8, c: f64) -> usize {
    let free = side_extent(rect, side) - 2.0 * c;
    if free < 0.0 {
        1
    } else {
        (free / c).floor() as usize + 1
    }
}

/// The compacted rows, established independently of the router from Law
/// 2's excuse: every `(node, side)` carrying more distinct ports than its
/// capacity — with the wires that land there and the band its outermost
/// ports bound.
struct Row {
    vertical: bool,
    lo: f64,
    hi: f64,
    members: Vec<usize>,
}

fn compacted_rows(index: &SceneIndex, wires: &[RoutedWire], c: f64) -> Vec<Row> {
    let mut rows: BTreeMap<(String, u8), Vec<(usize, f64)>> = BTreeMap::new();
    for (wi, w) in wires.iter().enumerate() {
        if w.path.len() < 2 {
            continue;
        }
        let n = w.path.len();
        let ends = [
            (&w.seg_from, w.path[0], w.path[1]),
            (&w.seg_to, w.path[n - 1], w.path[n - 2]),
        ];
        for (path, port, inward) in ends {
            let Some(rect) = index.rect(path) else {
                continue;
            };
            let Ok(side) = landing(rect, port, inward, c) else {
                continue;
            };
            rows.entry((path.clone(), side))
                .or_default()
                .push((wi, ord_on(side, port)));
        }
    }
    let mut out = Vec::new();
    for ((path, side), ends) in rows {
        let rect = index.rect(&path).expect("landed ends have a body");
        let mut ords: Vec<f64> = ends.iter().map(|t| t.1).collect();
        ords.sort_by(f64::total_cmp);
        ords.dedup_by(|a, b| (*a - *b).abs() <= EPS);
        if ords.len() <= side_capacity(rect, side, c) {
            continue;
        }
        let mut members: Vec<usize> = ends.iter().map(|t| t.0).collect();
        members.sort_unstable();
        members.dedup();
        out.push(Row {
            vertical: side == 1 || side == 3,
            lo: ords[0],
            hi: *ords.last().unwrap(),
            members,
        });
    }
    out
}

/// Which side (0 top, 1 right, 2 bottom, 3 left) the port lands on — or why
/// the landing is illegal. Corner margin relaxes to half the side on sides
/// too short for full clearance (port capacity bottoms out at one).
fn landing(rect: Rect, port: (f64, f64), inward: (f64, f64), c: f64) -> Result<u8, String> {
    let (x, y) = port;
    let on_x = x > rect.x0 + EPS && x < rect.x1 - EPS;
    let on_y = y > rect.y0 + EPS && y < rect.y1 - EPS;
    let side = if (y - rect.y0).abs() <= EPS && on_x {
        0
    } else if (x - rect.x1).abs() <= EPS && on_y {
        1
    } else if (y - rect.y1).abs() <= EPS && on_x {
        2
    } else if (x - rect.x0).abs() <= EPS && on_y {
        3
    } else {
        return Err("end is not on a side".to_owned());
    };
    let (margin, len, perpendicular) = match side {
        0 | 2 => (
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

/// Law 1 — Clearance from bodies: ≥ clearance from every solid rect, and
/// from the wire's own endpoints on every segment but the adjoining stub.
/// A containment wire runs inside its outer endpoint by design (WIRING
/// §Special shapes), so that body is skipped.
fn clearance(index: &SceneIndex, wires: &[RoutedWire], c: f64, out: &mut Vec<Violation>) {
    for w in wires {
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
            if SceneIndex::contains(body, partner) {
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

/// Law 1 (wire–wire) and Law 3's audit promise: every segment pair of two
/// wires keeps clearance, except the sanctioned contacts — transversal
/// crossings (reconciled against the report), fan-sibling trunks (drawn as
/// one line), and the port rows of compacted sides (Law 1's third
/// surrender): among one such side's wires, the final approach legs and
/// each leg's feeding corner may under-clear one another.
fn separation(
    index: &SceneIndex,
    wires: &[RoutedWire],
    c: f64,
    report: &[Violation],
    out: &mut Vec<Violation>,
) {
    let rows = compacted_rows(index, wires, c);
    let mut drawn: BTreeMap<(String, String), Vec<(f64, f64)>> = BTreeMap::new();
    for i in 0..wires.len() {
        for j in i + 1..wires.len() {
            let (a, b) = (&wires[i], &wires[j]);
            let fan_pair = [a.fan_from, a.fan_to]
                .iter()
                .flatten()
                .any(|g| [b.fan_from, b.fan_to].contains(&Some(*g)));
            let bands: Vec<(bool, f64, f64)> = rows
                .iter()
                .filter(|r| r.members.contains(&i) && r.members.contains(&j))
                .map(|r| (r.vertical, r.lo, r.hi))
                .collect();
            let mut offence: Option<String> = None;
            for sa in a.path.windows(2) {
                for sb in b.path.windows(2) {
                    if let Some(at) = audit::cross(sa, sb) {
                        drawn.entry(pair_key(a, b)).or_default().push(at);
                        continue;
                    }
                    let d = box_dist(seg_box(sa), seg_box(sb));
                    if d >= c - EPS
                        || (fan_pair && trunk_contact(sa, sb, a, b, d))
                        || bands.iter().any(|&band| audit::band_contact(band, sa, sb))
                    {
                        continue;
                    }
                    offence.get_or_insert_with(|| {
                        format!("segments {sa:?} and {sb:?} are {d} apart, need ≥ {c}")
                    });
                }
            }
            if let Some(detail) = offence {
                out.push(Violation {
                    rule: Rule::Separation,
                    severity: Severity::Warning,
                    wires: vec![name(a), name(b)],
                    detail,
                    span: b.decl_span,
                });
            }
        }
    }
    reconcile(wires, &drawn, report, out);
}

/// Fan-sibling contact that is the shared trunk rather than a braid: an
/// outright overlap or touch, a segment that lies on the partner's polyline
/// (the trunk is one drawn line, so the partner extends it), or corner
/// adjacency where staggered branch points peel off the trunk — parallel
/// segments whose travel extents only touch. Siblings running alongside each
/// other past the split still breach.
fn trunk_contact(
    sa: &[(f64, f64)],
    sb: &[(f64, f64)],
    a: &RoutedWire,
    b: &RoutedWire,
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

fn pair_key(a: &RoutedWire, b: &RoutedWire) -> (String, String) {
    let (x, y) = (name(a), name(b));
    if x <= y { (x, y) } else { (y, x) }
}

/// Law 3: "a crossing the report doesn't name is a bug" — and a named
/// crossing that is not drawn is the same bug mirrored.
fn reconcile(
    wires: &[RoutedWire],
    drawn: &BTreeMap<(String, String), Vec<(f64, f64)>>,
    report: &[Violation],
    out: &mut Vec<Violation>,
) {
    let mut reported: BTreeMap<(String, String), (usize, Span)> = BTreeMap::new();
    for v in report.iter().filter(|v| v.rule == Rule::Crossing) {
        let [a, b] = v.wires.as_slice() else {
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
        wires
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
                wires: vec![key.0.clone(), key.1.clone()],
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
                wires: vec![key.0.clone(), key.1.clone()],
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
    use crate::resolve::{AttrMap, Markers, ResolvedValue, ShapeKind};

    fn body(id: &str, cx: f64, cy: f64) -> PlacedNode {
        PlacedNode {
            id: Some(id.to_owned()),
            shape: ShapeKind::Box,
            type_chain: Vec::new(),
            applied_styles: Vec::new(),
            label: None,
            attrs: AttrMap::default(),
            markers: Markers::default(),
            cx,
            cy,
            bbox: Bbox::centered(40.0, 40.0),
            frame: None,
            rotation: 0.0,
            children: Vec::new(),
            dividers: Vec::new(),
            span: Span::empty(),
        }
    }

    fn wire(from: &str, to: &str, path: Vec<(f64, f64)>) -> RoutedWire {
        let mut attrs = AttrMap::default();
        attrs.insert("clearance", ResolvedValue::Number(8.0));
        RoutedWire {
            path,
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
    fn a_clean_straight_wire_is_silent() {
        let w = wire("a", "b", vec![(20.0, 0.0), (180.0, 0.0)]);
        let out = check(&pair(), &[w], &[], &VarTable::new());
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
        let w = wire(
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
        let out = check(&nodes, &[w], &[], &VarTable::new());
        assert!(rules(&out).contains(&Rule::Clearance), "{out:?}");
    }

    #[test]
    fn clearance_fires_inside_the_wires_own_keepout() {
        // A middle segment sweeps back 4 over its own source body.
        let w = wire(
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
        let out = check(&pair(), &[w], &[], &VarTable::new());
        assert!(rules(&out).contains(&Rule::Clearance), "{out:?}");
    }

    #[test]
    fn contact_fires_on_corner_oblique_and_diagonal_landings() {
        let corner = wire("a", "b", vec![(20.0, -20.0), (180.0, -20.0)]);
        let near_corner = wire("a", "b", vec![(20.0, -15.0), (180.0, -15.0)]);
        let oblique = wire(
            "a",
            "b",
            vec![(20.0, 0.0), (20.0, -40.0), (180.0, -40.0), (180.0, 0.0)],
        );
        let diagonal = wire("a", "b", vec![(20.0, 0.0), (170.0, -10.0), (180.0, 0.0)]);
        for w in [corner, near_corner, oblique, diagonal] {
            let out = check(&pair(), &[w], &[], &VarTable::new());
            assert!(rules(&out).contains(&Rule::Contact), "{out:?}");
        }
    }

    #[test]
    fn contact_fires_when_ports_cram_or_drift_off_centre() {
        let nodes = vec![
            body("a", 0.0, 0.0),
            body("b", 200.0, 0.0),
            body("c", 0.0, 100.0),
        ];
        // Two ports on b's left side 4 apart (need ≥ 8), median off centre.
        let w1 = wire("a", "b", vec![(20.0, 0.0), (180.0, 0.0)]);
        let w2 = wire(
            "c",
            "b",
            vec![(20.0, 100.0), (100.0, 100.0), (100.0, 4.0), (180.0, 4.0)],
        );
        let out = check(&nodes, &[w1, w2], &[], &VarTable::new());
        assert!(
            out.iter()
                .any(|v| v.rule == Rule::Contact && v.detail.contains("ports")),
            "{out:?}"
        );
    }

    #[test]
    fn separation_fires_on_a_parallel_hug() {
        let nodes = vec![
            body("a", 0.0, 0.0),
            body("b", 200.0, 0.0),
            body("c", 0.0, 100.0),
            body("d", 200.0, 100.0),
        ];
        let w1 = wire(
            "a",
            "b",
            vec![
                (20.0, 0.0),
                (60.0, 0.0),
                (60.0, 40.0),
                (160.0, 40.0),
                (160.0, 0.0),
                (180.0, 0.0),
            ],
        );
        let w2 = wire(
            "c",
            "d",
            vec![
                (20.0, 100.0),
                (60.0, 100.0),
                (60.0, 44.0),
                (160.0, 44.0),
                (160.0, 100.0),
                (180.0, 100.0),
            ],
        );
        let out = check(&nodes, &[w1, w2], &[], &VarTable::new());
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
        let w1 = wire("a", "b", vec![(20.0, 0.0), (180.0, 0.0)]);
        let w2 = wire("c", "d", vec![(100.0, -80.0), (100.0, 80.0)]);
        let entry = |wires: Vec<String>| Violation {
            rule: Rule::Crossing,
            severity: Severity::Info,
            wires,
            detail: String::new(),
            span: Span::empty(),
        };

        // Drawn but unnamed: the checker flags the crossing.
        let out = check(&nodes, &[w1.clone(), w2.clone()], &[], &VarTable::new());
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
            &VarTable::new(),
        );
        assert_eq!(out.len(), 0, "{out:?}");

        // Named but not drawn: the phantom is flagged.
        let phantom = entry(vec!["a -> b".to_owned(), "x -> y".to_owned()]);
        let out = check(&nodes, &[w1, w2], &[named, phantom], &VarTable::new());
        assert!(
            out.iter()
                .any(|v| v.detail.contains("named in the report but")),
            "{out:?}"
        );
    }

    #[test]
    fn a_wire_crossing_itself_is_flagged() {
        // A hook past the port row whose final approach sweeps back through
        // the wire's own run — the failed-inversion bubble shape.
        let w = wire(
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
        let out = check(&pair(), &[w], &[], &VarTable::new());
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
        let mut w1 = wire("a", "b", vec![(20.0, 0.0), (180.0, 0.0)]);
        let mut w2 = wire("a", "c", vec![(20.0, 0.0), (100.0, 0.0), (100.0, 140.0)]);
        // Untagged, the trunk overlap and split T-joint breach separation…
        let out = check(&nodes, &[w1.clone(), w2.clone()], &[], &VarTable::new());
        assert!(rules(&out).contains(&Rule::Separation), "{out:?}");
        // …as fan siblings they are one drawn line.
        w1.fan_from = Some(0);
        w2.fan_from = Some(0);
        let out = check(&nodes, &[w1, w2], &[], &VarTable::new());
        assert_eq!(out.len(), 0, "{out:?}");
    }
}

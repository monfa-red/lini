//! The natural strategy's law arm (ROUTING.md The natural strategy) —
//! judged on drawn output alone: the sampled path, the exact cubics, the
//! placed nodes, and the engine's own report. Five judgments:
//!
//! - **Contact** — each end on its own side inside the port window,
//!   arriving perpendicular ([`super::contact_ends`], shared with the
//!   orthogonal arm), its stub dead straight and long enough for its marker.
//! - **Smoothness** — one G1 chain: at every knot the incoming and outgoing
//!   handles are parallel, and each stub is tangent to its end cubic — no
//!   corner at any point, ever.
//! - **Directness** — where both drawn stub directions advance along the
//!   chord (stub tip to stub tip), path progress along that chord is
//!   monotone: no doubling back, no orbits. Self-loops excepted.
//! - **Respect** — every sample ≥ margin (`clearance / 2`) from every solid
//!   body — the same [`Keepouts`] the engine dodged with — *or* the wire
//!   carries the engine's own Clearance report naming it: reported overlaps
//!   are the contract, silent ones are breaches.
//! - **Duplicate separation** — same-pair wires never pinch below the
//!   pitch floor; members fanned together lawfully ride one drawn line.
//!
//! The orthogonal-only laws are explicitly skipped per ROUTING.md: run/track
//! discipline has no meaning off the channel grid, and crossings involving a
//! natural wire are lawfully oblique (reconciled in [`super::check`] with
//! the same generic intersection the engine counts them with, never judged
//! square-on). Natural produces no strays to check.

use super::super::natural::{curve, dodge::Keepouts};
use super::super::ortho::cost::min_pitch;
use super::super::ortho::rect::{box_dist, seg_box};
use super::super::ortho::scene::SceneIndex;
use super::{EPS, Rule, Severity, Violation, breach, contact_ends, fan_pair, name};
use crate::layout::ir::RoutedLink;
use crate::render::markers::marker_size;

/// Spline overshoot slack for the directness judgment, px: a Catmull blend
/// may wobble a hair against the chord without reading as a double-back.
const BACKTRACK_SLACK: f64 = 0.5;

pub(super) fn check(
    index: &SceneIndex,
    links: &[&RoutedLink],
    c: f64,
    report: &[Violation],
    out: &mut Vec<Violation>,
) {
    contact(index, links, c, out);
    smoothness(links, out);
    directness(links, out);
    respect(index, links, c, report, out);
    duplicates(links, c, out);
}

/// Segment length.
fn seg_len(s: &[(f64, f64)]) -> f64 {
    (s[1].0 - s[0].0).hypot(s[1].1 - s[0].1)
}

/// Contact: the shared landing judgment, plus the natural stub law — each
/// end's first drawn piece is a dead-straight axis-aligned run-up at least
/// its marker long (the curve begins only past the stub tip).
fn contact(index: &SceneIndex, links: &[&RoutedLink], c: f64, out: &mut Vec<Violation>) {
    for w in links {
        if w.path.len() < 2 {
            out.push(breach(Rule::Contact, w, "degenerate path".to_owned()));
            continue;
        }
        contact_ends(index, w, c, out);
        let n = w.path.len();
        let thickness = w.attrs.number("stroke-width").unwrap_or(0.0);
        for (kind, stub) in [
            (w.markers.start, [w.path[0], w.path[1]]),
            (w.markers.end, [w.path[n - 1], w.path[n - 2]]),
        ] {
            if stub[0].0 != stub[1].0 && stub[0].1 != stub[1].1 {
                out.push(breach(
                    Rule::Contact,
                    w,
                    format!("stub {stub:?} is not straight on its side's axis"),
                ));
            } else if kind != crate::resolve::MarkerKind::None
                && seg_len(&stub) < marker_size(thickness) - EPS
            {
                out.push(breach(
                    Rule::Contact,
                    w,
                    format!("stub {stub:?} is shorter than its marker's run-up"),
                ));
            }
        }
    }
}

fn unit(v: (f64, f64)) -> Option<(f64, f64)> {
    let l = v.0.hypot(v.1);
    (l > EPS).then(|| (v.0 / l, v.1 / l))
}

/// Smoothness: the cubics chain G1 — every knot's incoming and outgoing
/// handles parallel and same-signed, each stub tangent to its end cubic.
fn smoothness(links: &[&RoutedLink], out: &mut Vec<Violation>) {
    type Joint<'a> = ((f64, f64), (f64, f64), &'a str);
    for w in links {
        if w.curve.is_empty() || w.path.len() < 2 {
            continue;
        }
        let n = w.path.len();
        let first = w.curve.first().expect("non-empty");
        let last = w.curve.last().expect("non-empty");
        let mut joints: Vec<Joint> = vec![
            (
                (w.path[1].0 - w.path[0].0, w.path[1].1 - w.path[0].1),
                (first[1].0 - first[0].0, first[1].1 - first[0].1),
                "start stub",
            ),
            (
                (last[3].0 - last[2].0, last[3].1 - last[2].1),
                (
                    w.path[n - 1].0 - w.path[n - 2].0,
                    w.path[n - 1].1 - w.path[n - 2].1,
                ),
                "end stub",
            ),
        ];
        for pair in w.curve.windows(2) {
            joints.push((
                (pair[0][3].0 - pair[0][2].0, pair[0][3].1 - pair[0][2].1),
                (pair[1][1].0 - pair[1][0].0, pair[1][1].1 - pair[1][0].1),
                "knot",
            ));
        }
        for (inn, outv, what) in joints {
            let (Some(a), Some(b)) = (unit(inn), unit(outv)) else {
                continue; // a zero handle carries no direction to disagree
            };
            let cross = a.0 * b.1 - a.1 * b.0;
            let dot = a.0 * b.0 + a.1 * b.1;
            if cross.abs() > 1e-3 || dot < 0.0 {
                out.push(breach(
                    Rule::Contact,
                    w,
                    format!("kink at a {what}: tangents {a:?} vs {b:?}"),
                ));
                break;
            }
        }
    }
}

/// Directness: where both drawn stub directions advance along the chord —
/// every heuristic side does, and so do a tree's stamped sides — the
/// undodged curve's progress along the chord is monotone. A dodge (more
/// than one cubic) swings exactly as wide as its vias require, a side
/// forced away from the chord swings out, and a self-loop is a hook — all
/// exempt.
fn directness(links: &[&RoutedLink], out: &mut Vec<Violation>) {
    for w in links {
        if w.path.len() < 4 || w.curve.len() > 1 || w.seg_from == w.seg_to {
            continue;
        }
        let n = w.path.len();
        let (ta, tb) = (w.path[1], w.path[n - 2]);
        let chord = (tb.0 - ta.0, tb.1 - ta.1);
        let da = (w.path[1].0 - w.path[0].0, w.path[1].1 - w.path[0].1);
        let db = (
            w.path[n - 2].0 - w.path[n - 1].0,
            w.path[n - 2].1 - w.path[n - 1].1,
        );
        let faces = |d: (f64, f64)| d.0 * chord.0 + d.1 * chord.1 >= -EPS;
        if !faces(da) || !faces((-db.0, -db.1)) {
            continue;
        }
        let l = chord.0.hypot(chord.1);
        if l <= EPS {
            continue;
        }
        let u = (chord.0 / l, chord.1 / l);
        let mut hi = f64::NEG_INFINITY;
        for p in &w.path[1..n - 1] {
            let t = (p.0 - ta.0) * u.0 + (p.1 - ta.1) * u.1;
            if t < hi - BACKTRACK_SLACK {
                out.push(breach(
                    Rule::Contact,
                    w,
                    format!("doubles back along its chord near ({}, {})", p.0, p.1),
                ));
                break;
            }
            hi = hi.max(t);
        }
    }
}

/// Respect: every piece ≥ margin from every solid — or the engine reported
/// the overlap on this wire. The stubs may enter only their own endpoint's
/// keep-out (span-granular, as the engine judges).
fn respect(
    index: &SceneIndex,
    links: &[&RoutedLink],
    c: f64,
    report: &[Violation],
    out: &mut Vec<Violation>,
) {
    let m = min_pitch(c);
    for w in links {
        if w.path.len() < 2 || w.curve.is_empty() {
            continue;
        }
        let excused = report.iter().any(|v| {
            v.rule == Rule::Clearance && v.severity == Severity::Warning && v.links == [name(w)]
        });
        if excused {
            continue;
        }
        let (Some(ra), Some(rb)) = (index.rect(&w.seg_from), index.rect(&w.seg_to)) else {
            continue; // contact already flagged the missing body
        };
        let keep = Keepouts::build(index, [(&w.seg_from, ra), (&w.seg_to, rb)], m);
        let n = w.path.len();
        let pieces =
            std::iter::once((vec![w.path[0], w.path[1]], [true, false]))
                .chain(std::iter::once((
                    vec![w.path[n - 2], w.path[n - 1]],
                    [false, true],
                )))
                .chain(w.curve.iter().enumerate().map(|(i, cubic)| {
                    (curve::sample_span(cubic), [i == 0, i == w.curve.len() - 1])
                }));
        for (pts, exc) in pieces {
            if let Some((s, r, d)) = keep.offence(&pts, exc) {
                out.push(breach(
                    Rule::Clearance,
                    w,
                    format!(
                        "sample {s:?} is {d} from a body at {r:?}, needs ≥ {m} — \
                         and the engine reported no overlap for this wire"
                    ),
                ));
                break;
            }
        }
    }
}

/// Duplicate separation: two natural wires of one endpoint pair are rails
/// off one port ladder and must never pinch below the pitch floor anywhere
/// along their curves. Members fanned together are one drawn line — the
/// trunk excuse. The full-clearance scarcity excuses are channel arithmetic,
/// orthogonal-only; the natural arm judges the absolute floor.
fn duplicates(links: &[&RoutedLink], c: f64, out: &mut Vec<Violation>) {
    for i in 0..links.len() {
        for j in i + 1..links.len() {
            let (a, b) = (links[i], links[j]);
            let same_pair = (a.seg_from == b.seg_from && a.seg_to == b.seg_to)
                || (a.seg_from == b.seg_to && a.seg_to == b.seg_from);
            if !same_pair || a.seg_from == a.seg_to || fan_pair(a, b) {
                continue;
            }
            let floor = min_pitch(c);
            'gap: for sa in a.path.windows(2) {
                for sb in b.path.windows(2) {
                    let d = box_dist(seg_box(sa), seg_box(sb));
                    if d < floor - EPS {
                        out.push(Violation {
                            rule: Rule::Separation,
                            severity: Severity::Warning,
                            links: vec![name(a), name(b)],
                            detail: format!(
                                "duplicate curves pinch to {d} at {sa:?} / {sb:?}, \
                                 below the half-clearance floor {floor}"
                            ),
                            span: b.decl_span,
                        });
                        break 'gap;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{body, link, pair, rules};
    use super::super::{Rule, check};
    use crate::layout::ir::{Cubic, RoutedLink};
    use crate::resolve::Strategy;

    /// A natural wire: straight stubs, one straight cubic between the tips.
    fn natural(from: &str, to: &str, path: Vec<(f64, f64)>, curve: Vec<Cubic>) -> RoutedLink {
        let mut w = link(from, to, path);
        w.strategy = Strategy::Natural;
        w.curve = curve;
        w
    }

    fn straight(y: f64) -> RoutedLink {
        natural(
            "a",
            "b",
            vec![(20.0, y), (30.0, y), (170.0, y), (180.0, y)],
            vec![[(30.0, y), (76.0, y), (124.0, y), (170.0, y)]],
        )
    }

    #[test]
    fn a_clean_natural_straight_is_silent() {
        let out = check(&pair(), &[straight(0.0)], &[]);
        assert_eq!(out.len(), 0, "{out:?}");
    }

    #[test]
    fn an_oblique_natural_stub_breaches_contact() {
        let w = natural(
            "a",
            "b",
            vec![(20.0, 0.0), (30.0, 5.0), (170.0, 0.0), (180.0, 0.0)],
            vec![[(30.0, 5.0), (76.0, 5.0), (124.0, 0.0), (170.0, 0.0)]],
        );
        let out = check(&pair(), &[w], &[]);
        assert!(rules(&out).contains(&Rule::Contact), "{out:?}");
    }

    #[test]
    fn a_kinked_knot_breaches_smoothness() {
        // Two cubics meeting at a right angle: G1 broken at the knot.
        let w = natural(
            "a",
            "b",
            vec![
                (20.0, 0.0),
                (30.0, 0.0),
                (100.0, -40.0),
                (170.0, 0.0),
                (180.0, 0.0),
            ],
            vec![
                [(30.0, 0.0), (60.0, 0.0), (100.0, -70.0), (100.0, -40.0)],
                [(100.0, -40.0), (130.0, -40.0), (150.0, 0.0), (170.0, 0.0)],
            ],
        );
        let out = check(&pair(), &[w], &[]);
        assert!(rules(&out).contains(&Rule::Contact), "{out:?}");
    }

    #[test]
    fn a_double_back_breaches_directness() {
        // Facing stubs, but the path retreats 30 px mid-flight.
        let w = natural(
            "a",
            "b",
            vec![
                (20.0, 0.0),
                (30.0, 0.0),
                (120.0, -20.0),
                (90.0, -25.0),
                (170.0, 0.0),
                (180.0, 0.0),
            ],
            vec![[(30.0, 0.0), (76.0, 0.0), (124.0, 0.0), (170.0, 0.0)]],
        );
        let out = check(&pair(), &[w], &[]);
        assert!(rules(&out).contains(&Rule::Contact), "{out:?}");
    }

    #[test]
    fn an_unreported_body_overlap_breaches_respect() {
        // The straight cubic runs level through `wall` with no engine
        // report naming the wire.
        let nodes = vec![
            body("a", 0.0, 0.0),
            body("b", 200.0, 0.0),
            body("wall", 100.0, 0.0),
        ];
        let out = check(&nodes, &[straight(0.0)], &[]);
        assert!(rules(&out).contains(&Rule::Clearance), "{out:?}");
    }

    #[test]
    fn a_reported_overlap_is_the_contract_not_a_breach() {
        use crate::routing::{Severity, Violation};
        use crate::span::Span;
        let nodes = vec![
            body("a", 0.0, 0.0),
            body("b", 200.0, 0.0),
            body("wall", 100.0, 0.0),
        ];
        let report = vec![Violation {
            rule: Rule::Clearance,
            severity: Severity::Warning,
            links: vec!["a -> b".to_owned()],
            detail: "natural wire passes 0.0 px from the body".to_owned(),
            span: Span::empty(),
        }];
        let out = check(&nodes, &[straight(0.0)], &report);
        assert!(!rules(&out).contains(&Rule::Clearance), "{out:?}");
    }

    #[test]
    fn duplicate_curves_pinching_below_the_floor_are_flagged() {
        // Two rails 3 apart: below clearance/2 = 4 — never lawful…
        let out = check(&pair(), &[straight(0.0), straight(3.0)], &[]);
        assert!(rules(&out).contains(&Rule::Separation), "{out:?}");
        // …at 4 (the floor, exactly) the compressed bundle stands.
        let out = check(&pair(), &[straight(0.0), straight(4.0)], &[]);
        assert_eq!(out.len(), 0, "{out:?}");
    }

    #[test]
    fn fanned_duplicates_ride_one_drawn_line() {
        // Members fanned at both ends draw as a single track (ROUTING.md
        // request model): zero separation, lawfully.
        let mut w1 = straight(0.0);
        let mut w2 = straight(0.0);
        w1.fan_from = Some(0);
        w2.fan_from = Some(0);
        let out = check(&pair(), &[w1, w2], &[]);
        assert_eq!(out.len(), 0, "{out:?}");
    }
}

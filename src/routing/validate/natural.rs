//! The natural strategy's law arm (ROUTING.md The natural strategy) —
//! judged on drawn output alone: the sampled path, the exact cubics, the
//! placed nodes. Three judgments:
//!
//! - **Contact** — each end on its own side inside the port window,
//!   arriving perpendicular ([`super::contact_ends`], shared with the
//!   orthogonal arm), its stub dead straight and long enough for its marker.
//! - **Sampled clearance** — every sample ≥ clearance from every solid body,
//!   the own-endpoint excuse limited to each end's stub and own end span:
//!   the same keep-out construction and the same offence predicate the
//!   router's tightening pass draws by ([`corridor::Keepouts`] — one
//!   mechanism, so the checker passes exactly what the engine guarantees).
//! - **Duplicate separation** — bundle members' curves never pinch below the
//!   half-clearance pitch floor; members fanned together lawfully ride one
//!   drawn line (the trunk excuse).
//!
//! The orthogonal-only laws are explicitly skipped per ROUTING.md: run/track
//! discipline has no meaning off the channel grid, and crossings involving a
//! natural wire are lawfully oblique (reconciled in [`super::check`] with
//! the same generic intersection the engine counts them with, never judged
//! square-on).

use super::super::natural::{corridor::Keepouts, curve};
use super::super::ortho::cost::min_pitch;
use super::super::ortho::rect::{box_dist, seg_box};
use super::super::ortho::scene::SceneIndex;
use super::{EPS, Rule, Severity, Violation, breach, contact_ends, fan_pair, name};
use crate::layout::ir::RoutedLink;
use crate::render::markers::marker_size;

pub(super) fn check(index: &SceneIndex, links: &[&RoutedLink], c: f64, out: &mut Vec<Violation>) {
    contact(index, links, c, out);
    clearance(index, links, c, out);
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

/// Sampled clearance: the stubs may enter only their own endpoint's
/// keep-out; the curve, span by span, holds clearance from every solid and
/// from its own endpoints past each end's own span.
fn clearance(index: &SceneIndex, links: &[&RoutedLink], c: f64, out: &mut Vec<Violation>) {
    for w in links {
        if w.path.len() < 2 || w.curve.is_empty() {
            continue;
        }
        let (Some(ra), Some(rb)) = (index.rect(&w.seg_from), index.rect(&w.seg_to)) else {
            continue; // contact already flagged the missing body
        };
        let keep = Keepouts::build(index, [(&w.seg_from, ra), (&w.seg_to, rb)], c);
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
        for (pts, excused) in pieces {
            if let Some((s, r, d)) = keep.offence(&pts, excused) {
                out.push(breach(
                    Rule::Clearance,
                    w,
                    format!("sample {s:?} is {d} from a body at {r:?}, needs ≥ {c}"),
                ));
                break;
            }
        }
    }
}

/// Duplicate separation: two natural wires of one endpoint pair are bundle
/// rails and must never pinch below the half-clearance pitch floor anywhere
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
    fn a_curve_sample_inside_a_keepout_breaches_clearance() {
        // The straight cubic runs level through `wall` — clearance is 8.
        let nodes = vec![
            body("a", 0.0, 0.0),
            body("b", 200.0, 0.0),
            body("wall", 100.0, 0.0),
        ];
        let out = check(&nodes, &[straight(0.0)], &[]);
        assert!(rules(&out).contains(&Rule::Clearance), "{out:?}");
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

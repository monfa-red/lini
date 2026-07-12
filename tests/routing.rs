//! The v2 routing contract tests (ROUTING.md, ROUTING-LOG.md stage 4):
//! geometry assertions over routed polylines — turn counts, ordinates,
//! crossings — never images. The Consequences table pins one test per row.

use lini::testing::{node_rect, route_sample, routes_str};
use lini::{Rule, Severity};

const PCB: &str = include_str!("../samples/pcb.lini");

type Routes = Vec<((String, String), Vec<(f64, f64)>)>;

fn routes(src: &str) -> Routes {
    routes_str(src).expect("compiles and routes")
}

fn path<'a>(routes: &'a Routes, from: &str, to: &str) -> &'a [(f64, f64)] {
    &routes
        .iter()
        .find(|((a, b), _)| a == from && b == to)
        .unwrap_or_else(|| panic!("no drawn route {from} -> {to}"))
        .1
}

fn paths<'a>(routes: &'a Routes, from: &str, to: &str) -> Vec<&'a [(f64, f64)]> {
    routes
        .iter()
        .filter(|((a, b), _)| a == from && b == to)
        .map(|(_, p)| p.as_slice())
        .collect()
}

/// Direction changes along an orthogonal polyline.
fn turns(path: &[(f64, f64)]) -> usize {
    let sign = |v: f64| {
        if v > 0.0 {
            1
        } else if v < 0.0 {
            -1
        } else {
            0
        }
    };
    let dir = |a: (f64, f64), b: (f64, f64)| (sign(b.0 - a.0), sign(b.1 - a.1));
    path.windows(3)
        .filter(|w| dir(w[0], w[1]) != dir(w[1], w[2]))
        .count()
}

fn orthogonal(p: &[(f64, f64)]) {
    for s in p.windows(2) {
        assert!(
            s[0].0 == s[1].0 || s[0].1 == s[1].1,
            "diagonal segment {s:?} in {p:?}"
        );
        assert!(s[0] != s[1], "zero-length segment in {p:?}");
    }
}

fn report(src: &str) -> Vec<lini::Violation> {
    lini::validate_str(src).expect("compiles")
}

fn crossings(src: &str) -> usize {
    report(src)
        .iter()
        .filter(|v| v.rule == Rule::Crossing)
        .count()
}

fn impossibles(src: &str) -> usize {
    report(src)
        .iter()
        .filter(|v| v.rule == Rule::Impossible && v.severity == Severity::Warning)
        .count()
}

// ── The Consequences table, one test per row (ROUTING.md §Consequences) ──

#[test]
fn facing_aligned_centres_draw_one_straight_wire() {
    let r = routes(
        "{ direction: row; gap: 100; clearance: 10 }\n\
         |box#a| { width: 60; height: 60 }\n\
         |box#b| { width: 60; height: 60 }\n\
         a -> b\n",
    );
    let p = path(&r, "a", "b");
    assert_eq!(p.len(), 2, "straight: {p:?}");
    assert_eq!(p[0].1, p[1].1, "horizontal: {p:?}");
}

#[test]
fn offset_within_windows_still_draws_straight() {
    // b sits 20 lower: the centres differ but the port windows still meet,
    // so the wire rides the shared window — straight, no jog.
    let r = routes(
        "{ direction: row; gap: 100; clearance: 10 }\n\
         |box#a| { width: 60; height: 60 }\n\
         |box#b| { width: 60; height: 60; translate: 0 20 }\n\
         a -> b\n",
    );
    let p = path(&r, "a", "b");
    assert_eq!(p.len(), 2, "windows overlap → straight: {p:?}");
    assert_eq!(p[0].1, p[1].1);
}

#[test]
fn offset_past_windows_jogs_once_on_the_gap_midline() {
    // b sits 60 lower and the facing sides are held (unforced, the cheaper
    // one-turn L via a's bottom wins — straight beats dogleg beats
    // staircase): the windows can't meet, so the route doglegs once and the
    // perpendicular run lands on the midline of the corridor between the
    // two keep-outs.
    let src = "{ direction: row; gap: 100; clearance: 10 }\n\
               |box#a| { width: 60; height: 60 }\n\
               |box#b| { width: 60; height: 60; translate: 0 60 }\n\
               a:right -> b:left\n";
    let r = routes(src);
    let p = path(&r, "a", "b");
    orthogonal(p);
    assert_eq!(p.len(), 4, "one dogleg: {p:?}");
    let laid = route_sample(src, 10.0);
    let a = node_rect(&laid, "a").expect("a placed");
    let b = node_rect(&laid, "b").expect("b placed");
    let midline = ((a.2 + 10.0) + (b.0 - 10.0)) / 2.0;
    assert!(
        (p[1].0 - midline).abs() < 1e-9 && (p[2].0 - midline).abs() < 1e-9,
        "jog on the gap midline {midline}: {p:?}"
    );
}

#[test]
fn offset_past_windows_without_forced_sides_takes_the_one_turn_l() {
    // The same scene unforced: Law 3 prefers the single corner over the
    // dogleg — turns cost real length.
    let r = routes(
        "{ direction: row; gap: 100; clearance: 10 }\n\
         |box#a| { width: 60; height: 60 }\n\
         |box#b| { width: 60; height: 60; translate: 0 60 }\n\
         a -> b\n",
    );
    let p = path(&r, "a", "b");
    orthogonal(p);
    assert_eq!(turns(p), 1, "one corner beats the dogleg: {p:?}");
}

#[test]
fn a_bundle_draws_parallel_rails_at_pitch_centred_on_the_midline() {
    let src = "{ direction: row; gap: 100; clearance: 10 }\n\
               |box#a| { width: 60; height: 60 }\n\
               |box#b| { width: 60; height: 60 }\n\
               a -> b\na -> b\na -> b\na -> b\n";
    let r = routes(src);
    let rails = paths(&r, "a", "b");
    assert_eq!(rails.len(), 4);
    let mut ys: Vec<f64> = rails
        .iter()
        .map(|p| {
            assert_eq!(p.len(), 2, "each rail straight: {p:?}");
            p[0].1
        })
        .collect();
    ys.sort_by(f64::total_cmp);
    for w in ys.windows(2) {
        assert!((w[1] - w[0] - 10.0).abs() < 1e-9, "rails at pitch: {ys:?}");
    }
    let laid = route_sample(src, 10.0);
    let a = node_rect(&laid, "a").expect("a placed");
    let centre = (a.1 + a.3) / 2.0;
    let mean = ys.iter().sum::<f64>() / 4.0;
    assert!(
        (mean - centre).abs() < 1e-9,
        "ladder centred on the aligned centres {centre}: {ys:?}"
    );
}

#[test]
fn a_wire_over_the_top_hugs_the_keepout_not_the_margin() {
    // The wall forces a→b over the top; the run along the canvas edge rides
    // exactly one clearance off the wall's body, not out in the margin.
    let src = "{ direction: row; gap: 80; clearance: 10 }\n\
               |box#a| { width: 60; height: 60 }\n\
               |box#w| { width: 20; height: 200 }\n\
               |box#b| { width: 60; height: 60 }\n\
               a -> b\n";
    let r = routes(src);
    let p = path(&r, "a", "b");
    orthogonal(p);
    let laid = route_sample(src, 10.0);
    let w = node_rect(&laid, "w").expect("w placed");
    // Bottom outranks top on the side rank, so the wire passes under; it
    // rides the wall's keep-out edge exactly — walls charge no margin
    // (near neighbours cluster instead), so the wire hugs the diagram,
    // never the canvas margin.
    let hug = w.3 + 10.0;
    assert!(
        p.windows(2)
            .any(|s| s[0].1 == s[1].1 && (s[0].1 - hug).abs() < 1e-9),
        "the crossing run hugs the keep-out at {hug}: {p:?}"
    );
}

#[test]
fn a_crossing_beats_the_orbit() {
    // n→s commits a vertical rail through the middle; a→b still draws dead
    // straight across it — one reported crossing, never a detour around the
    // diagram.
    let src = "{ layout: grid; columns: repeat(3, 60); rows: repeat(3, 60); gap: 30; clearance: 10 }\n\
               |box#n| { cell: 2 1; width: 60; height: 60 }\n\
               |box#a| { cell: 1 2; width: 60; height: 60 }\n\
               |box#b| { cell: 3 2; width: 60; height: 60 }\n\
               |box#s| { cell: 2 3; width: 60; height: 60 }\n\
               n -> s\n\
               a -> b\n";
    let r = routes(src);
    assert_eq!(path(&r, "n", "s").len(), 2, "n→s straight");
    assert_eq!(path(&r, "a", "b").len(), 2, "a→b crosses, never orbits");
    assert_eq!(crossings(src), 1, "one drawn crossing, reported");
}

// ── Forced sides, fans, self-loops, containment ──

#[test]
fn a_forced_side_is_honored_when_reachable() {
    let src = "{ direction: row; gap: 60; clearance: 10 }\n\
               |box#w| { width: 20; height: 200 }\n\
               |box#a| { width: 60; height: 60 }\n\
               |box#b| { width: 60; height: 60 }\n\
               a:left -> b\n";
    let r = routes(src);
    let p = path(&r, "a", "b");
    let laid = route_sample(src, 10.0);
    let a = node_rect(&laid, "a").expect("a placed");
    assert_eq!(p[0].0, a.0, "leaves a's left side: {p:?}");
}

#[test]
fn a_walled_forced_side_strays() {
    // w sits 8px from a — inside a's clearance — so a's left punch is
    // blocked and the forced link is reported and drawn as a stray.
    let src = "{ layout: grid; columns: repeat(2, 60); rows: repeat(2, 60); gap: 8; clearance: 10 }\n\
               |box#w| { cell: 1 1; width: 60; height: 60 }\n\
               |box#a| { cell: 2 1; width: 60; height: 60 }\n\
               |box#b| { cell: 2 2; width: 60; height: 60 }\n\
               a:left -> b\n";
    let r = routes(src);
    assert!(
        paths(&r, "a", "b").is_empty(),
        "no lawful-looking wire for a blocked forced side"
    );
    assert_eq!(impossibles(src), 1);
}

#[test]
fn fan_siblings_share_their_first_point() {
    let src = "{ layout: grid; columns: repeat(2, 60); rows: repeat(2, 60); gap: 40; clearance: 10 }\n\
               |box#a| { cell: 1 1; span: 1 2; width: 60; height: 60 }\n\
               |box#b| { cell: 2 1; width: 60; height: 60 }\n\
               |box#c| { cell: 2 2; width: 60; height: 60 }\n\
               a -> b & c\n";
    let r = routes(src);
    let (pb, pc) = (path(&r, "a", "b"), path(&r, "a", "c"));
    assert_eq!(pb[0], pc[0], "one fan, one port: {pb:?} vs {pc:?}");
}

#[test]
fn a_self_loop_wraps_the_top_right_corner() {
    let src = "{ clearance: 10 }\n|box#a| { width: 80; height: 40 }\na -> a\n";
    let r = routes(src);
    let p = path(&r, "a", "a");
    orthogonal(p);
    let laid = route_sample(src, 10.0);
    let a = node_rect(&laid, "a").expect("a placed");
    let (cx, cy) = ((a.0 + a.2) / 2.0, (a.1 + a.3) / 2.0);
    // Out the right side, around the keep-out corner, back in the top —
    // both wall runs riding exactly one clearance off the body.
    assert_eq!(
        p,
        &[
            (a.2, cy),
            (a.2 + 10.0, cy),
            (a.2 + 10.0, a.1 - 10.0),
            (cx, a.1 - 10.0),
            (cx, a.1)
        ],
        "right → top around the keep-out corner"
    );
}

#[test]
fn a_self_loop_forced_onto_one_side_is_reported() {
    let src = "{ clearance: 10 }\n|box#a| { width: 80; height: 40 }\na:top -> a:top\n";
    assert!(paths(&routes(src), "a", "a").is_empty());
    let rep = report(src);
    assert!(
        rep.iter()
            .any(|v| v.rule == Rule::Impossible && v.detail.contains("one side")),
        "{rep:?}"
    );
}

#[test]
fn a_containment_link_lands_on_the_parents_inner_side() {
    let src = "{ clearance: 10 }\n\
               |group#p| { padding: 30 } [ |box#c| { width: 40; height: 40 } ]\n\
               p -> p.c\n";
    let r = routes(src);
    let p = path(&r, "p", "p.c");
    orthogonal(p);
    let laid = route_sample(src, 10.0);
    let pr = node_rect(&laid, "p").expect("p placed");
    let on_side = p[0].0 == pr.0 || p[0].0 == pr.2 || p[0].1 == pr.1 || p[0].1 == pr.3;
    assert!(on_side, "p's end sits on p's own boundary: {p:?} vs {pr:?}");
    for pt in p {
        assert!(
            pt.0 >= pr.0 && pt.0 <= pr.2 && pt.1 >= pr.1 && pt.1 <= pr.3,
            "the wire stays inside the parent: {p:?}"
        );
    }
}

// ── The straight strategy (ROUTING.md §Strategies) ──

#[test]
fn routing_straight_draws_one_trimmed_oblique_segment() {
    // `routing: straight` (SPEC §9): one segment between the body centres,
    // trimmed to the boundaries — oblique is lawful here, and nothing is
    // avoided or reported.
    let src = "{ layout: grid; columns: repeat(2, 60); rows: repeat(2, 60); gap: 40; \
               clearance: 10; routing: straight }\n\
               |box#a| { cell: 1 1; width: 60; height: 60 }\n\
               |box#b| { cell: 2 2; width: 60; height: 60 }\n\
               a -> b\n";
    let r = routes(src);
    let p = path(&r, "a", "b");
    assert_eq!(p.len(), 2, "one segment: {p:?}");
    assert!(
        p[0].0 != p[1].0 && p[0].1 != p[1].1,
        "oblique, no avoidance: {p:?}"
    );
    let laid = route_sample(src, 10.0);
    let a = node_rect(&laid, "a").expect("a placed");
    let b = node_rect(&laid, "b").expect("b placed");
    // The diagonal centres sit 45° apart, so the trim lands exactly on the
    // facing corners — the stray's trim math, shared.
    let close = |p: (f64, f64), q: (f64, f64)| (p.0 - q.0).abs() < 1e-9 && (p.1 - q.1).abs() < 1e-9;
    assert!(close(p[0], (a.2, a.3)), "trimmed to a's corner: {p:?}");
    assert!(close(p[1], (b.0, b.1)), "trimmed to b's corner: {p:?}");
    assert_eq!(impossibles(src), 0);
    assert_eq!(crossings(src), 0);
}

#[test]
fn routing_straight_self_link_draws_the_rectangular_hook() {
    let src = "{ clearance: 16; routing: straight }\n|box#a| { width: 80; height: 40 }\na -> a\n";
    let r = routes(src);
    let p = path(&r, "a", "a");
    assert_eq!(p.len(), 4, "the rectangular self-hook: {p:?}");
    let laid = route_sample(src, 16.0);
    let a = node_rect(&laid, "a").expect("a placed");
    assert!(
        p.iter().all(|pt| pt.0 >= a.2),
        "the hook hangs off the right side: {p:?}"
    );
    assert_eq!(p[0].0, a.2);
    assert_eq!(p[3].0, a.2);
}

// ── The tightness sweep: right before impossible (ROUTING-LOG.md stage 6) ──

/// A facing 4-bundle with the shared port window swept from roomy down past
/// capacity: full clearance while it fits, uniform compression only when it
/// cannot (never below half the clearance), and the stray appears exactly
/// at the capacity boundary — no silent squeeze, no ugly detour.
#[test]
fn a_bundle_compresses_uniformly_then_strays_exactly_at_capacity() {
    // Sides forced: unforced, the router lawfully spills the whole bundle
    // over the top once the facing sides fill — a better outcome, but this
    // sweep pins the capacity boundary itself.
    let src = |h: u32| {
        format!(
            "{{ direction: row; gap: 100; clearance: 10 }}\n\
             |box#a| {{ width: 60; height: {h} }}\n\
             |box#b| {{ width: 60; height: {h} }}\n\
             a:right -> b:left\na:right -> b:left\na:right -> b:left\na:right -> b:left\n"
        )
    };
    for h in (30..=60).rev() {
        let text = src(h);
        // The drawn bbox carries the stroke, so the lawful window comes off
        // the placed rect, not the declared height.
        let a = node_rect(&route_sample(&text, 10.0), "a").expect("a placed");
        let window = (a.3 - a.1) - 20.0;
        let rails = routes(&text);
        let report = report(&text);
        let breaches: Vec<_> = report
            .iter()
            .filter(|v| v.severity == Severity::Warning && v.rule != Rule::Impossible)
            .collect();
        assert!(breaches.is_empty(), "h={h}: {breaches:?}");
        if window < 15.0 {
            // Fewer than 4 tracks at half clearance: the whole bundle
            // strays (a bundle routes whole or not at all).
            assert!(rails.is_empty(), "h={h}: past capacity, all stray");
            assert_eq!(impossibles(&text), 4, "h={h}");
            continue;
        }
        assert_eq!(rails.len(), 4, "h={h}: the bundle routes whole");
        let mut ys: Vec<f64> = rails
            .iter()
            .map(|(_, p)| {
                assert_eq!(p.len(), 2, "h={h}: rails stay straight: {p:?}");
                p[0].1
            })
            .collect();
        ys.sort_by(f64::total_cmp);
        let expect = (window / 3.0).min(10.0);
        for w in ys.windows(2) {
            assert!(
                (w[1] - w[0] - expect).abs() < 1e-9,
                "h={h}: uniform pitch {expect}, got {ys:?}"
            );
        }
    }
}

// ── The side-capacity sweep: a full side spills, then strays ──

/// n wires from a west column onto one 30×30 target: its left side holds 3
/// ports at half clearance, every side holds 12 — excess routes to other
/// sides, then strays, and the ports that share the left side land in their
/// wires' vertical order (no braid at the mouth).
#[test]
fn a_full_side_spills_to_other_sides_then_strays() {
    let src = |n: usize| {
        let mut s = String::from(
            "{ layout: grid; columns: 40, 200, 40; rows: repeat(15, 34); gap: 10; clearance: 10 }\n\
             |box#t| { cell: 2 8; width: 30; height: 30 }\n",
        );
        for i in 0..n {
            s.push_str(&format!(
                "|box#s{i}| {{ cell: 1 {}; width: 30; height: 30 }}\n",
                i + 1
            ));
        }
        for i in 0..n {
            s.push_str(&format!("s{i} -> t\n"));
        }
        s
    };
    for n in [1, 3, 6, 12, 13, 14] {
        let text = src(n);
        let r = routes(&text);
        let breaches: Vec<_> = report(&text)
            .into_iter()
            .filter(|v| v.severity == Severity::Warning && v.rule != Rule::Impossible)
            .collect();
        assert!(breaches.is_empty(), "n={n}: {breaches:?}");
        let drawn = r.len();
        assert_eq!(
            drawn + impossibles(&text),
            n,
            "n={n}: every wire drawn or honestly reported"
        );
        assert_eq!(drawn, n.min(12), "n={n}: capacity is 12 ports");
        // No braid at the mouth: ports on t's left side keep their wires'
        // vertical order (source i sits above source j for i < j).
        let laid = route_sample(&text, 10.0);
        let t = node_rect(&laid, "t").expect("t placed");
        let mut left: Vec<(f64, usize)> = r
            .iter()
            .filter(|((_, to), p)| to == "t" && p.last().unwrap().0 == t.0)
            .map(|((from, _), p)| {
                let i: usize = from[1..].parse().expect("source index");
                (p.last().unwrap().1, i)
            })
            .collect();
        left.sort_by(|a, b| a.0.total_cmp(&b.0));
        let order: Vec<usize> = left.iter().map(|&(_, i)| i).collect();
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(order, sorted, "n={n}: left ports braid");
    }
}

// ── The crossing torture: crossings arrive one at a time, never a wrap ──

/// The corridor across the middle is progressively walled by longer and
/// more committed rails: while a dodge over a short rail is cheaper than
/// `4·clearance` the wire dodges, then crossings appear exactly one per
/// rail — each drawn square-on and each reported — and the wire never orbits
/// the diagram.
#[test]
fn crossings_appear_one_at_a_time_and_never_wrap() {
    // `h` sets each rail's node height: 40 leaves a cheap hop over the rail;
    // 160 walls the hop off (the orbit costs far more than a crossing).
    let src = |rails: &[u32]| {
        let cols = rails.len() + 2;
        let mut s = format!(
            "{{ layout: grid; columns: repeat({cols}, 60); rows: 160, 60, 160; \
             gap: 30; clearance: 10 }}\n\
             |box#a| {{ cell: 1 2; width: 60; height: 60 }}\n\
             |box#b| {{ cell: {cols} 2; width: 60; height: 60 }}\n"
        );
        for (i, h) in rails.iter().enumerate() {
            s.push_str(&format!(
                "|box#n{i}| {{ cell: {c} 1; width: 60; height: {h} }}\n\
                 |box#s{i}| {{ cell: {c} 3; width: 60; height: {h} }}\n\
                 n{i} -> s{i}\n",
                c = i + 2
            ));
        }
        s.push_str("a -> b\n");
        s
    };
    // Every rail costs exactly one crossing: a dodge over even a short rail
    // needs two extra turns — already the crossing's whole price — plus
    // length, so Law 3 crosses square-on and counts it (the consequences
    // table's "crossing beats orbit", one rail at a time).
    for (rails, expected) in [
        (vec![], 0),
        (vec![40], 1),
        (vec![160], 1),
        (vec![160, 40], 2),
        (vec![160, 160], 2),
        (vec![160, 160, 160], 3),
    ] {
        let text = src(&rails);
        assert_eq!(
            crossings(&text),
            expected,
            "rails {rails:?} force exactly {expected} crossing(s)"
        );
        assert_eq!(impossibles(&text), 0, "rails {rails:?}");
        let breaches: Vec<_> = report(&text)
            .into_iter()
            .filter(|v| v.severity == Severity::Warning)
            .collect();
        assert!(breaches.is_empty(), "rails {rails:?}: {breaches:?}");
        // Never a wrap: a → b stays inside the lane band, clear of the
        // canvas margin above and below.
        let r = routes(&text);
        let laid = route_sample(&text, 10.0);
        let a = node_rect(&laid, "a").expect("a placed");
        for &(x, y) in path(&r, "a", "b") {
            assert!(
                y >= a.1 - 40.0 && y <= a.3 + 40.0 && x >= a.0,
                "rails {rails:?}: the wire wraps: ({x}, {y})"
            );
        }
    }
}

// ── Duplicate wires nest, never braid (ROUTING.md model step 5) ──

/// Two wires between one endpoint pair detouring around a wall are exact
/// parallels — no geometry forces an order, so the convention must pick one
/// consistently at all three shared channels. A braid shows up as a
/// self-inflicted crossing, whichever way the pair is declared.
#[test]
fn duplicate_detours_nest_without_crossing() {
    let dims = "{ direction: row; gap: 50; clearance: 10 }\n\
                |box#a| { width: 60; height: 60 }\n\
                |box#wall| { width: 60; height: 140 }\n\
                |box#b| { width: 60; height: 60 }\n";
    for pair in ["a -> b\nb -> a\n", "a -> b\na -> b\n"] {
        let src = format!("{dims}{pair}");
        assert_eq!(crossings(&src), 0, "the pair braids: {pair:?}");
        assert_eq!(impossibles(&src), 0);
    }
}

/// The development sample keeps its header's promise: every routing pattern
/// it stages — duplicates, detours, a fan, a self-loop — draws with zero
/// crossings.
#[test]
fn links_simple_reports_zero_crossings() {
    let src = include_str!("../samples/links_simple.lini");
    assert_eq!(crossings(src), 0);
}

/// A bundle whose rails S-curve between two corridors keeps its pitch the
/// whole way (user-reported: the round-two corridor read handed the second
/// legs an anchor outside their lawful bounds, the preference-first order
/// interleaved the trunk with the pocket, and the pairwise clamp collapsed
/// all three trunk rails onto one ordinate).
#[test]
fn a_bundle_of_s_curves_keeps_pitch_on_both_legs() {
    let src = "{ layout: grid; columns: repeat(3); gap: 35; clearance: 12; }\n\
        |box#alpha| \"Alpha\" { cell: 1 1; }\n\
        |group#north| { cell: 2 1; gap: 16; } [\n\
          |caption| \"North\"\n\
          |box#nn1| \"N1\"\n\
          |box#nn2| \"N2\"\n\
        ]\n\
        |box#beta| \"Beta\" { cell: 3 1; }\n\
        |group#west| { cell: 1 2; gap: 16; } [\n\
          |caption| \"West\"\n\
          |box#ww1| \"W1\"\n\
          |box#ww2| \"W2\"\n\
        ]\n\
        |group#east| { cell: 3 2; padding: 16; gap: 16; } [\n\
          |caption| \"East\"\n\
          |box#ee1| \"E1\"\n\
          |box#ee2| \"E2\"\n\
        ]\n\
        |group#south| { cell: 2 3; gap: 16; } [\n\
          |caption| \"South\"\n\
          |box#ss1| \"S1\"\n\
        ]\n\
        hub -> south.ss1 & west.ww1\n\
        north.nn2 -> east.ee1\n\
        north.nn2 -> east.ee1\n\
        north.nn2 -> east.ee1\n";
    let breaches: Vec<_> = report(src)
        .into_iter()
        .filter(|v| v.severity == Severity::Warning)
        .collect();
    assert!(breaches.is_empty(), "{breaches:?}");
}

/// Pitch never compresses where the void holds full clearance
/// (user-reported: links_hard's w2 → s1 pair drew its middle legs 7.5
/// apart beside gamma — the wall behind them is keep-out-backed, a rail
/// may hug it, and 12 fits; the retired soft-boundary margin charged the
/// pair half a clearance for free space no wire could reach).
#[test]
fn a_duplicate_pair_keeps_full_pitch_beside_a_keepout() {
    let src = include_str!("../samples/links_hard.lini");
    let r = routes(src);
    let pair = paths(&r, "west.w2", "south.s1");
    assert_eq!(pair.len(), 2, "both rails draw");
    assert_eq!(pair[0].len(), pair[1].len(), "rails share one shape");
    // Corresponding legs of the two rails stay a full clearance apart —
    // every leg, not just the straights near the ports.
    for (i, (a, b)) in pair[0].windows(2).zip(pair[1].windows(2)).enumerate() {
        let (da, db) = (
            (a[1].0 - a[0].0, a[1].1 - a[0].1),
            (b[1].0 - b[0].0, b[1].1 - b[0].1),
        );
        let gap = if da.0 == 0.0 && db.0 == 0.0 {
            (a[0].0 - b[0].0).abs()
        } else if da.1 == 0.0 && db.1 == 0.0 {
            (a[0].1 - b[0].1).abs()
        } else {
            continue;
        };
        assert!(
            gap >= 12.0 - 1e-6,
            "leg {i} of the pair compressed to {gap}: {:?} vs {:?}",
            pair[0],
            pair[1]
        );
    }
}

// ── Determinism (Law 4) ──

#[test]
fn identical_input_routes_identically() {
    let first = routes(PCB);
    for _ in 0..3 {
        assert_eq!(routes(PCB), first);
    }
}

/// A fan's ports land on their sides' centres when nothing contends there
/// (user-reported: links_medium's `cat -> bowl & water` ports sat pinned
/// at the top of their windows — the packed bowl↔dog band transmitted its
/// pressure through the ladder's total order across a zero-sep boundary,
/// an order two span-disjoint wires never owed each other).
#[test]
fn uncontended_fan_ports_take_their_side_centres() {
    let src = include_str!("../samples/links_medium.lini");
    let r = routes(src);
    let laid = route_sample(src, 12.0);
    for to in ["kitchen.bowl", "kitchen.water"] {
        let rect = node_rect(&laid, to).expect("placed");
        let centre = (rect.1 + rect.3) / 2.0;
        let p = paths(&r, "cat", to)[0];
        let port = p.last().unwrap().1;
        assert!(
            (port - centre).abs() < 1e-9,
            "{to}: port {port} != side centre {centre}: {p:?}"
        );
    }
}

// ── Root-layout scopes: nested ordinary wires route [SPEC 11/13/15, M6] ──

#[test]
fn a_nested_row_under_a_root_drawing_routes_its_wire() {
    // The root drawing owns its own links (measures, mates), but a nested
    // ordinary scope's wires belong to the router — they used to vanish.
    let r = routes_str(
        "{ layout: drawing; }\n\
         |sketch#part| { draw: move(0, 0) right(60) down(30) left(60) close(); }\n\
         |row#legend| { translate: 0 80; gap: 30; } [\n\
           |box#a| \"A\"\n  |box#b| \"B\"\n  a -> b\n]\n",
    )
    .expect("compiles");
    assert_eq!(r.len(), 1, "the nested wire routes: {r:?}");
    assert_eq!(r[0].0, ("legend.a".to_string(), "legend.b".to_string()));
}

#[test]
fn a_nested_row_under_a_root_sequence_routes_its_wire_with_its_own_label() {
    // Routes, and wears its *own* label: the label pass must filter
    // program links exactly like the request pass, or the sequence message's
    // label lands on the routed wire (request::is_routed).
    let src = "{ layout: sequence; }\n\
         |box#a| \"A\"\n|box#b| \"B\"\na -> b \"hello\"\n\
         |row#legend| { gap: 20; } [\n\
           |box#x| \"X\"\n  |box#y| \"Y\"\n  x -> y \"wired\"\n]\n";
    let r = routes_str(src).expect("compiles");
    // Both draw: the engine-lowered message and the routed nested wire.
    assert!(
        r.iter()
            .any(|(ep, _)| ep == &("legend.x".to_string(), "legend.y".to_string())),
        "the nested wire routes: {r:?}"
    );
    let svg = lini::compile_str(src).expect("compile");
    let link = svg
        .find("data-from=\"legend.x\"")
        .expect("the routed link group");
    let group = &svg[link..link + svg[link..].find("</g>").unwrap()];
    assert!(
        group.contains("wired") && !group.contains("hello"),
        "the routed wire wears its own label: {group}"
    );
}

/// A tree of **anonymous** topics [SPEC 12]: desugar mints deterministic
/// `lini-topic-N` ids and generates one branch fan per parent, and the scene
/// routes every branch wire — silent wire loss (a fan naming an unminted child)
/// is the bug this pins.
#[test]
fn anonymous_topics_mint_ids_and_wire_up() {
    let src = "{ layout: tree; }\n\
        |topic| \"Root\" [\n\
          |topic| \"Goals\" [\n\
            |topic| \"Q3\"\n\
          ]\n\
          |topic| \"Risks\"\n\
        ]\n";
    let out = lini::desugar_source(src).expect("desugar");
    assert!(out.contains("#lini-topic-1"), "minted ids: {out}");
    // The branch fans are present, endpoints dotted from each parent's scope.
    assert!(
        out.contains(
            "lini-topic-1:bottom - lini-topic-1.lini-topic-1:top & lini-topic-1.lini-topic-2:top"
        ),
        "root fan: {out}"
    );
    // Three routed wires: Root→Goals, Root→Risks, Goals→Q3.
    let routes = routes(src);
    assert_eq!(routes.len(), 3, "three branch wires drawn: {routes:?}");
}

// ── The natural strategy: corridors, obstacles, laws (ROUTING.md The natural strategy) ──

/// Law breaches above counted output and honest strays — what the natural
/// arm (and the four laws) must keep silent on.
fn breaches(src: &str) -> Vec<lini::Violation> {
    report(src)
        .into_iter()
        .filter(|v| v.severity != Severity::Info && v.rule != Rule::Impossible)
        .collect()
}

#[test]
fn a_natural_wire_dodges_an_obstacle_inside_its_corridor() {
    // A wall between the endpoints forces the dogleg corridor; the curve
    // rides it — every sample keeps clearance from the wall — and the
    // natural law arm passes the scene.
    let src = "{ direction: row; gap: 60; clearance: 10; routing: natural }\n\
               |box#a| { width: 60; height: 60 }\n\
               |box#wall| { width: 60; height: 200 }\n\
               |box#b| { width: 60; height: 60 }\n\
               a -> b\n";
    let found = breaches(src);
    assert!(found.is_empty(), "{found:?}");
    // And at every knob value the natural arm stays silent (the laws.rs
    // sweep pattern, on the obstacle corridor).
    for c in [6.0, 10.0, 16.0] {
        let swept = lini::testing::laws(&route_sample(src, c));
        let found: Vec<_> = swept
            .iter()
            .filter(|v| v.severity != Severity::Info && v.rule != Rule::Impossible)
            .collect();
        assert!(found.is_empty(), "at clearance {c}: {found:?}");
    }
    let r = routes(src);
    let p = path(&r, "a", "b");
    let laid = route_sample(src, 10.0);
    let (x0, y0, x1, y1) = node_rect(&laid, "wall").expect("wall placed");
    for s in p.windows(2) {
        let dx = (x0 - s[0].0.max(s[1].0))
            .max(s[0].0.min(s[1].0) - x1)
            .max(0.0);
        let dy = (y0 - s[0].1.max(s[1].1))
            .max(s[0].1.min(s[1].1) - y1)
            .max(0.0);
        assert!(
            (dx * dx + dy * dy).sqrt() >= 10.0 - 1e-6,
            "sample window {s:?} inside the wall's clearance"
        );
    }
    // It actually dodges: the ports are level with the wall, so some sample
    // clears the wall's top or bottom edge.
    assert!(
        p.iter().any(|q| q.1 < y0 || q.1 > y1),
        "the curve never left the blocked straight line: {p:?}"
    );
}

#[test]
fn a_natural_bundle_draws_two_separated_parallel_curves() {
    let src = "{ direction: row; gap: 80; clearance: 10; routing: natural }\n\
               |box#a| { width: 60; height: 60 }\n\
               |box#b| { width: 60; height: 60 }\n\
               a -> b\n\
               a -> b\n";
    let found = breaches(src);
    assert!(found.is_empty(), "{found:?}");
    let r = routes(src);
    let rails = paths(&r, "a", "b");
    assert_eq!(rails.len(), 2, "both members drawn");
    assert_ne!(
        rails[0], rails[1],
        "members ride their own placed ordinates"
    );
    // Adjacent rails at pitch: no two sample windows pinch below clearance.
    let mut min = f64::INFINITY;
    for sa in rails[0].windows(2) {
        for sb in rails[1].windows(2) {
            let (ax0, ax1) = (sa[0].0.min(sa[1].0), sa[0].0.max(sa[1].0));
            let (ay0, ay1) = (sa[0].1.min(sa[1].1), sa[0].1.max(sa[1].1));
            let (bx0, bx1) = (sb[0].0.min(sb[1].0), sb[0].0.max(sb[1].0));
            let (by0, by1) = (sb[0].1.min(sb[1].1), sb[0].1.max(sb[1].1));
            let dx = (bx0 - ax1).max(ax0 - bx1).max(0.0);
            let dy = (by0 - ay1).max(ay0 - by1).max(0.0);
            min = min.min((dx * dx + dy * dy).sqrt());
        }
    }
    assert!(min >= 10.0 - 1e-6, "rails pinch to {min}");
}

#[test]
fn a_natural_fan_shares_its_trunk_stub_exactly() {
    // `a -> b & c`: the trunk — the shared port and stub — is one drawn
    // line; past the tip the siblings split smoothly, each a lawful wire.
    let src = "{ layout: grid; columns: repeat(2, 80); rows: repeat(2, 80); gap: 40; \
                 clearance: 10; routing: natural }\n\
               |box#a| { cell: 1 1; span: 1 2; width: 60; height: 60 }\n\
               |box#b| { cell: 2 1; width: 60; height: 60 }\n\
               |box#c| { cell: 2 2; width: 60; height: 60 }\n\
               a -> b & c\n";
    let found = breaches(src);
    assert!(found.is_empty(), "{found:?}");
    let r = routes(src);
    let (to_b, to_c) = (path(&r, "a", "b"), path(&r, "a", "c"));
    assert_eq!(to_b[0], to_c[0], "one shared port");
    assert_eq!(to_b[1], to_c[1], "one shared stub tip — the trunk is exact");
    assert_ne!(to_b.last(), to_c.last(), "siblings split to their own ends");
}

#[test]
fn a_natural_self_link_draws_a_smooth_hook() {
    let src = "{ direction: row; gap: 60; clearance: 10; routing: natural }\n\
               |box#a| { width: 80; height: 60 }\n\
               a -> a\n";
    let found = breaches(src);
    assert!(found.is_empty(), "{found:?}");
    let r = routes(src);
    let p = path(&r, "a", "a");
    let laid = route_sample(src, 10.0);
    let (x0, y0, x1, _) = node_rect(&laid, "a").expect("a placed");
    // Default sides right → top (ROUTING.md Special nodes).
    assert_eq!(p[0].0, x1, "leaves the right side");
    assert_eq!(p.last().unwrap().1, y0, "returns to the top side");
    // A smooth hook, not a rounded rectangle: dense curve samples between
    // the stubs, and never a doubling-back kink sharper than a right angle.
    assert!(p.len() > 10, "sampled curve: {p:?}");
    for w in p.windows(3) {
        let u = (w[1].0 - w[0].0, w[1].1 - w[0].1);
        let v = (w[2].0 - w[1].0, w[2].1 - w[1].1);
        assert!(
            u.0 * v.0 + u.1 * v.1 >= 0.0,
            "kink at {:?} in the hook",
            w[1]
        );
    }
    let _ = x0;
}

#[test]
fn natural_wires_cross_obliquely_and_the_report_counts_it() {
    // Two natural wires forced across each other: the crossing is oblique
    // (no square-on law for natural), counted once by the generic
    // intersection — and the checker reconciles it, so the scene stays
    // breach-free. The uneven widths keep the meet off the sample grid —
    // a crossing exactly on a shared sample point is a touch, not counted,
    // by engine and checker alike.
    let src = "{ layout: grid; columns: repeat(3, 60); rows: repeat(3, 70); gap: 30; \
                 clearance: 10; routing: natural }\n\
               |box#n| { cell: 2 1; width: 60; height: 60 }\n\
               |box#a| { cell: 1 2; width: 60; height: 50 }\n\
               |box#b| { cell: 3 2; width: 80; height: 60 }\n\
               |box#s| { cell: 2 3; width: 60; height: 80 }\n\
               n -> s\n\
               a -> b\n";
    let found = breaches(src);
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(crossings(src), 1, "one oblique crossing, reported");
}

#[test]
fn a_natural_obstacle_scene_renders_byte_identically() {
    let src = "{ direction: row; gap: 60; clearance: 10; routing: natural }\n\
               |box#a| { width: 60; height: 60 }\n\
               |box#wall| { width: 60; height: 200 }\n\
               |box#b| { width: 60; height: 60 }\n\
               a -> b\n";
    let svg = lini::compile_str(src).expect("compiles");
    let routes = routes_str(src).expect("routes");
    for _ in 0..2 {
        assert_eq!(lini::compile_str(src).expect("recompile"), svg);
        assert_eq!(routes_str(src).expect("reroute"), routes);
    }
}

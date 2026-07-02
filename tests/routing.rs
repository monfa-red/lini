//! The v2 routing contract tests (ROUTING.md, ROUTING-V2.md stage 4):
//! geometry assertions over routed polylines — turn counts, ordinates,
//! crossings — never images. The Consequences table pins one test per row;
//! the pcb_fail pins prove the two v1 bugs (the orbit, the braid) dead.

use lini::testing::{node_rect, route_sample, routes_str};
use lini::{Rule, Severity};

const PCB_FAIL: &str = include_str!("../samples/pcb_fail.lini");
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
    // rides the wall's keep-out edge plus the half-clearance the channel
    // surrenders where its far stretches face free space — one wire-width
    // off the diagram, never out in the canvas margin.
    let hug = w.3 + 10.0 + 5.0;
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

// ── The pcb_fail pins: the two v1 bugs stay dead ──

#[test]
fn pcb_fail_pwr_rails_run_dead_straight() {
    let r = routes(PCB_FAIL);
    let rails = paths(&r, "pwr", "mcu");
    assert_eq!(rails.len(), 4);
    for p in rails {
        assert_eq!(turns(p), 0, "pwr → mcu straight: {p:?}");
    }
}

#[test]
fn pcb_fail_flash_lands_with_two_turns_and_never_orbits() {
    let laid = route_sample(PCB_FAIL, 10.0);
    let mcu = node_rect(&laid, "mcu").expect("mcu placed");
    let r = routes(PCB_FAIL);
    let rails = paths(&r, "flash", "mcu");
    assert_eq!(rails.len(), 4);
    for p in rails {
        assert_eq!(turns(p), 2, "flash → mcu doglegs: {p:?}");
        for pt in p {
            assert!(
                pt.0 <= mcu.2,
                "no point may orbit past mcu's right edge {}: {p:?}",
                mcu.2
            );
        }
    }
}

#[test]
fn pcb_fail_reports_zero_crossings() {
    assert_eq!(crossings(PCB_FAIL), 0);
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

// ── Determinism (Law 4) ──

#[test]
fn identical_input_routes_identically() {
    let first = routes(PCB);
    for _ in 0..3 {
        assert_eq!(routes(PCB), first);
    }
}

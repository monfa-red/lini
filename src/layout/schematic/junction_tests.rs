//! Junction dots [SPEC 16.5]: a fan's lead is dotted **where it splits**, one
//! dot per meet of three or more conductors, and nowhere else.
//!
//! Every expected point is stated by an **independent oracle** — the corner a
//! member's own drawn polyline turns at, read straight off `laid.links` here —
//! never by re-running the pass that produced it.

use super::super::tests::{anchor, close, program};
use crate::layout::PlacedNode;
use crate::layout::ir::LaidOut;

fn sheet(body: &str) -> String {
    format!("{{ layout: schematic }}\n{body}")
}

fn routed(src: &str) -> LaidOut {
    crate::layout::layout(&program(src)).expect("layout")
}

/// The generated dots' centres, in scene coordinates.
fn dots(laid: &LaidOut) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = laid
        .junctions
        .iter()
        .map(|d: &PlacedNode| (d.cx, d.cy))
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    out
}

/// The oracle: the first corner of the drawn wire between two endpoints — the
/// point at which that leg leaves the shared lead.
fn first_corner(laid: &LaidOut, from: &str, to: &str) -> (f64, f64) {
    let w = laid
        .links
        .iter()
        .find(|w| w.seg_from == from && w.seg_to == to)
        .unwrap_or_else(|| panic!("no drawn wire {from} -> {to}"));
    w.path[1]
}

fn has(dots: &[(f64, f64)], p: (f64, f64)) -> bool {
    dots.iter().any(|d| close(d.0, p.0) && close(d.1, p.1))
}

#[test]
fn a_shared_pin_is_dotted_where_its_one_lead_splits() {
    // Two statements land on u1.c: ROUTING.md's implicit fan, drawn as one lead
    // until the split [SPEC 16.5]. The dot is at the split — **not** at the pin,
    // where only the stub and the single lead meet.
    let src = sheet(&(anchor("u1", "") + &anchor("u2", "") + "u1.c - u2.a\nu1.c - u2.b\n"));
    let laid = routed(&src);
    let dots = dots(&laid);
    assert_eq!(dots.len(), 1, "one meet, one dot: {dots:?}");
    // Whichever leg peels off first owns the split; the other is still on the
    // lead there, so the far corner is not a meet.
    let (a, b) = (
        first_corner(&laid, "u1.c", "u2.a"),
        first_corner(&laid, "u1.c", "u2.b"),
    );
    let landing = laid.links[0].path[0];
    let inner = if (a.0 - landing.0).abs() <= (b.0 - landing.0).abs() {
        a
    } else {
        b
    };
    assert!(has(&dots, inner), "at the near split {inner:?}: {dots:?}");
    assert!(!has(&dots, landing), "never at the pin: {dots:?}");
}

#[test]
fn a_three_way_fan_dots_every_split_and_never_its_terminus() {
    // Three legs off u1.c: two turn away and one runs straight into r1's pin.
    // Each turn is a meet (lead in, branch out, lead on); the straight leg's
    // landing is not — one lead arrives and stops on a pin.
    let src = sheet(
        &(anchor("u1", "") + &anchor("u2", "") + "|R#r1|\nu1.c - u2.a\nu1.c - u2.b\nu1.c - r1\n"),
    );
    let laid = routed(&src);
    let dots = dots(&laid);
    assert_eq!(dots.len(), 2, "two splits, two dots: {dots:?}");
    for (from, to) in [("u1.c", "u2.a"), ("u1.c", "u2.b")] {
        let c = first_corner(&laid, from, to);
        assert!(has(&dots, c), "{from} -> {to} splits at {c:?}: {dots:?}");
    }
    let straight = laid
        .links
        .iter()
        .find(|w| w.seg_to == "r1.p1")
        .expect("the straight leg");
    assert_eq!(straight.path.len(), 2, "it really is straight");
    assert!(
        !has(&dots, *straight.path.last().expect("an end")),
        "a terminus is no meet: {dots:?}"
    );
}

#[test]
fn an_and_fan_dots_its_trunk_split() {
    // The written fan (`&`), not the merged one — same law, same dot.
    let src = sheet(&(anchor("u1", "") + &anchor("u2", "") + "u1.c - u2.a & u2.b\n"));
    let laid = routed(&src);
    assert_eq!(dots(&laid).len(), 1, "{:?}", dots(&laid));
}

#[test]
fn a_labels_stub_never_counts() {
    // [SPEC 16.5]: a net tag hangs off the wire; it is not a third conductor.
    // Both spellings — the minted text tag and the capsule symbol — are one
    // `|label|` at the far end, so neither adds a dot.
    for tail in ["u1.c - \"NET\"\n", "u1.c - |gnd|\n"] {
        let src = sheet(&(anchor("u1", "") + &anchor("u2", "") + "u1.c - u2.a\n" + tail));
        let laid = routed(&src);
        assert!(laid.links.len() >= 2, "both wires drew: {tail}");
        assert!(dots(&laid).is_empty(), "{tail}: {:?}", dots(&laid));
    }
    // …and with a *second* real conductor the meet is back: the tag is excluded,
    // the two wires still split.
    let src = sheet(
        &(anchor("u1", "") + &anchor("u2", "") + "u1.c - u2.a\nu1.c - u2.b\nu1.c - \"NET\"\n"),
    );
    assert_eq!(dots(&routed(&src)).len(), 1);
}

#[test]
fn a_crossing_stays_clean_and_dotless() {
    // Two wires that must cross share no landing, so no fan names the point —
    // and the pass never looks for coincident geometry, which is why a crossing
    // cannot become a dot [SPEC 16.5].
    let src = sheet(
        &(anchor("u1", " { cell: 1 1 }")
            + &anchor("u2", " { cell: 3 1 }")
            + &anchor("u3", " { cell: 1 3 }")
            + &anchor("u4", " { cell: 3 3 }")
            + "u1.c - u4.a\nu3.c - u2.a\n"),
    );
    let laid = routed(&src);
    assert!(
        laid.link_report
            .iter()
            .any(|v| matches!(v.rule, crate::routing::Rule::Crossing)),
        "the sheet really does cross"
    );
    assert!(dots(&laid).is_empty(), "{:?}", dots(&laid));
}

#[test]
fn a_series_chain_draws_no_dot() {
    // A pass-through lands on two *different* pins [SPEC 16.5], so nothing
    // shares a port and the run is dotless end to end.
    let src = sheet(&(anchor("u1", "") + "|C#c1|\n|gnd#g1|\nu1.c - c1 - g1\n"));
    let laid = routed(&src);
    assert_eq!(laid.links.len(), 2, "one statement, two wires");
    assert!(dots(&laid).is_empty(), "{:?}", dots(&laid));
}

#[test]
fn an_ordinary_scene_generates_no_junctions() {
    // The dot is the schematic's [SPEC 16.5]: a flow scene's fan is a fan, not a
    // net, and the pass answers only where a part landed the wire.
    let src = "{ direction: row; gap: 80 }\n|box#a|\n|box#b|\n|box#c|\na -> b & c\n";
    let laid = routed(src);
    assert!(laid.links.len() == 2 && laid.junctions.is_empty());
}

#[test]
fn a_natural_fan_neither_erases_nor_moves_the_orthogonal_dots() {
    // Fan group ids are numbered per **driver** — each strategy calls
    // `fan_groups` for itself and counts its kept groups from zero — so an
    // orthogonal fan and a natural one both answer to 0. Bucketing on the id
    // alone merged them into one group whose members share no origin, and every
    // dot on the sheet vanished with no diagnostic.
    //
    // Two ortho fans, then the same sheet with one of them turned natural: the
    // surviving dot must be at the *identical* coordinate, not merely present.
    let body = |routing: &str| {
        anchor("u1", " { cell: 1 1 }")
            + &anchor("u2", " { cell: 2 1 }")
            + &anchor("u3", " { cell: 1 3 }")
            + &anchor("u4", " { cell: 2 3 }")
            + "u1.c - u2.a\nu1.c - u2.b\n"
            + "u3.c - u4.a & u4.b"
            + routing
            + "\n"
    };
    let all_ortho = dots(&routed(&sheet(&body(""))));
    assert_eq!(all_ortho.len(), 2, "two fans, two dots: {all_ortho:?}");
    let mixed = routed(&sheet(&body(" { routing: natural }")));
    let mixed_dots = dots(&mixed);
    assert_eq!(mixed.links.len(), 4, "the sheet still draws four wires");
    // The natural fan contributes none — its legs are a sampled curve, not the
    // straight lead the split arithmetic reads — and the orthogonal one is
    // untouched.
    assert_eq!(mixed_dots.len(), 1, "{mixed_dots:?}");
    assert!(
        all_ortho.contains(&mixed_dots[0]),
        "the ortho meet keeps its exact point: {mixed_dots:?} vs {all_ortho:?}"
    );
    // …and a sheet wired entirely by another driver is dotless. That is the
    // second half of the fix and it is not the same half: a `natural` wire's
    // `path` is a dense *sampling* of a curve, so the walk that finds where a
    // leg leaves the shared lead finds where the **sampling** first bends —
    // which is a couple of pixels off the pin. Read ungated, this exact sheet
    // dots one leg 2px from its own landing, the one placement SPEC 16.5 rules
    // out, and another mid-curve where nothing meets.
    let natural = "{ layout: schematic; routing: natural }\n".to_string()
        + &anchor("u1", " { cell: 1 1 }")
        + &anchor("u2", " { cell: 3 1 }")
        + &anchor("u3", " { cell: 3 5 }")
        + "u1.c - u2.a\nu1.c - u3.b\nu1.c - u2.c\n";
    let laid = routed(&natural);
    assert_eq!(laid.links.len(), 3, "the wires drew");
    assert!(dots(&laid).is_empty(), "{:?}", dots(&laid));
    // The two-leg shape too, which the collision fix alone already handles —
    // both must stay clean.
    let pair = "{ layout: schematic; routing: natural }\n".to_string()
        + &anchor("u1", "")
        + &anchor("u2", "")
        + "u1.c - u2.a\nu1.c - u2.b\n";
    assert!(dots(&routed(&pair)).is_empty());
}

#[test]
fn the_dots_are_deterministic() {
    let src = sheet(
        &(anchor("u1", "") + &anchor("u2", "") + "|R#r1|\nu1.c - u2.a\nu1.c - u2.b\nu1.c - r1\n"),
    );
    let once = dots(&routed(&src));
    assert_eq!(once.len(), 2);
    for _ in 0..3 {
        assert_eq!(once, dots(&routed(&src)));
    }
}

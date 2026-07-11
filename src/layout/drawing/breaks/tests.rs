use super::super::testutil::{by_id, laid, layout_err, text_at};
use super::ViewMap;
use crate::ledger::consts::BREAK_GAP;
use crate::resolve::NodeKind;

#[test]
fn the_view_map_squashes_the_span_and_round_trips() {
    // One cut, 100..200 on x: 100 px folds into the 12 px gap.
    let v = ViewMap {
        x: vec![(100.0, 100.0), (200.0, 100.0 + BREAK_GAP)],
        ..Default::default()
    };
    assert_eq!(v.map((50.0, 7.0)), (50.0, 7.0), "near is identity");
    assert_eq!(v.map((300.0, 0.0)).0, 300.0 - 100.0 + BREAK_GAP);
    assert_eq!(
        v.map((150.0, 0.0)).0,
        100.0 + BREAK_GAP / 2.0,
        "mid-span squashes"
    );
    for t in [-20.0, 100.0, 137.0, 200.0, 450.0] {
        let d = v.map((t, 0.0));
        assert!((v.unmap(d).0 - t).abs() < 1e-9, "round-trip at {t}");
    }
}

#[test]
fn a_break_compresses_the_view_and_the_dim_stays_true() {
    // 300 long, break −80..60: 140 removed, the gap left — 172 displayed;
    // the dimension still reads the unbroken 300 [SPEC 15.3].
    let l = laid(
        "{ layout: drawing; density: 1 }\n|sketch#bar| { draw: move(-150, 0) up(10) right(300) down(10); mirror: x-axis; break: -80 60 }\nbar:left (-) bar:right { side: bottom }\n",
    );
    let bar = by_id(&l.nodes, "bar");
    assert!(
        (bar.bbox.w() - (172.0 + 2.0)).abs() < 1e-6,
        "compressed + stroke: {}",
        bar.bbox.w()
    );
    text_at(&l.nodes, "300");
}

#[test]
fn break_defaults_to_the_longer_axis() {
    // A tall profile with unnamed stations cuts on y.
    let l = laid(
        "{ layout: drawing; density: 1 }\n|sketch#post| { draw: move(-10, -60) right(20) down(120) left(20) close(); break: -30 30 }\n",
    );
    let post = by_id(&l.nodes, "post");
    assert!(
        (post.bbox.h() - (72.0 + 2.0)).abs() < 1e-6,
        "120 − 60 + gap: {}",
        post.bbox.h()
    );
    assert!((post.bbox.w() - 22.0).abs() < 1e-6, "x untouched");
}

#[test]
fn every_cut_edge_draws_the_jogged_break_line() {
    // One convention [SPEC 15.7]: the thin line across the profile with
    // the sharp jog mid-span — a polyline pair per group.
    let l = laid(
        "{ layout: drawing; density: 1 }\n|sketch#plate| { draw: move(-100, -12) right(200) down(24) left(200) close(); break: -50 50 }\n",
    );
    let cuts: Vec<_> = by_id(&l.nodes, "plate")
        .children
        .iter()
        .filter(|c| c.type_chain.iter().any(|t| t == "breakline"))
        .collect();
    assert_eq!(cuts.len(), 2, "one pair per group");
    assert!(cuts.iter().all(|c| c.kind == NodeKind::Line));
    // The near edge stands at the displayed station, jog amplitude
    // min(4.5, h/5) = 4.5 to its left, half a stroke more of paint.
    assert!(
        cuts.iter().any(|c| (c.bbox.min_x - -55.0).abs() < 1e-6),
        "near cut at −50 − amp − half stroke: {}",
        cuts[0].bbox.min_x
    );
    assert!(
        cuts.iter().all(|c| (c.bbox.h() - 33.0).abs() < 1e-6),
        "24 + 2 × overhang + stroke: {}",
        cuts[0].bbox.h()
    );
}

#[test]
fn a_station_in_the_removed_span_still_measures_true() {
    // `:mid` sits at x = 0, inside the cut — displayed it squashes into
    // the gap, but the dimension reads the model's 150.
    let l = laid(
        "{ layout: drawing; density: 1 }\n|sketch#bar| { draw: move(-150, 0) up(10) right(150):half point():mid right(150) down(10); mirror: x-axis; break: -80 60 }\nbar:left (-) bar:mid { side: bottom }\n",
    );
    text_at(&l.nodes, "150");
}

#[test]
fn a_revolved_station_span_reads_true_across_a_break() {
    // The ⌀ station reading reflects on the model, so a break never
    // narrows it [SPEC 15.6].
    let l = laid(
        "{ layout: drawing; density: 1 }\n|sketch#bar| { draw: move(-150, 0) up(10) right(40):thread right(260) down(10); revolve: x-axis; break: -80 60 }\nbar:thread (o) { side: left }\n",
    );
    text_at(&l.nodes, "⌀20");
}

#[test]
fn features_ride_the_broken_view_and_dims_to_them_stay_true() {
    // A hole at x = 100 sits past the cut: displayed it slides with the
    // far piece (100 − 140 + 12 = −28); a dim to it reads the model 250.
    let l = laid(
        "{ layout: drawing; density: 1 }\n|sketch#bar| { draw: move(-150, -15) right(300) down(30) left(300) close(); break: -80 60 } [\n  |hole#vent| { width: 10; translate: 100 0 }\n]\nbar:left (-) bar.vent { side: bottom }\n",
    );
    let vent = by_id(&l.nodes, "vent");
    assert!(
        (vent.cx - -28.0).abs() < 1e-9,
        "rigid with the far piece: {}",
        vent.cx
    );
    text_at(&l.nodes, "250");
}

#[test]
fn pattern_copies_ride_the_broken_view_too() {
    // The barrel bug: the view map is a black hole for every position in
    // the broken frame — a patterned hole's far-side copies slide with
    // the far piece, not just the carrier [SPEC 15.3].
    let l = laid(
        "{ layout: drawing; density: 1 }\n|sketch#bar| { draw: move(-150, -15) right(300) down(30) left(300) close(); break: -30 30 } [\n  |hole#vent| { width: 10; translate: -120 0; pattern: grid(3, 1, 80, 0) }\n]\n",
    );
    let vent = by_id(&l.nodes, "vent");
    assert_eq!((vent.cx, vent.cy), (-120.0, 0.0), "the seed stays put");
    let copies: Vec<f64> = vent
        .children
        .iter()
        .filter(|c| c.attrs.get("chrome").is_none() && c.kind == NodeKind::Oval)
        .map(|c| c.cx)
        .collect();
    // Model copies at −120, −40, 40 in the bar frame; 40 sits past the
    // cut → displayed 40 − 60 + 12 = −8, carrier-relative 112.
    assert_eq!(copies, vec![0.0, 80.0, 112.0], "the far copy slides");
}

#[test]
fn break_errors_speak_spec() {
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|rect#a| { width: 40; height: 20; break: -5 5 }\n"
        ),
        "'break' cuts a '|sketch|' — draw the profile with the pen"
    );
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|sketch#s| { draw: move(0, 0) right(40) down(20) left(40) close(); break: 30 10 }\n"
        ),
        "'break' takes two stations 'a b' — a < b — and an optional x-axis / y-axis"
    );
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|sketch#s| { draw: move(0, 0) right(40) down(20) left(40) close(); break: 90 100 }\n"
        ),
        "'break' at 90 misses the profile"
    );
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|sketch#s| { draw: move(0, 0) right(40) down(20) left(40) close(); break: 5 20, 15 35 }\n"
        ),
        "'break' spans overlap — merge them"
    );
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|sketch#s| { draw: move(0, 0) curve(20, -30, 40, 30, 60, 0) down(20) left(60) close(); break: 20 40 }\n"
        ),
        "a 'break' can't cut a 'curve()' — move the stations"
    );
}

#[test]
fn a_cut_through_an_arc_splits_it_clean() {
    // A half-round profile cut through its arc: both cut edges cross the
    // curve, the kept ends stay on the circle.
    let l = laid(
        "{ layout: drawing; density: 1 }\n|sketch#dome| { draw: move(-50, 0) arc(100, 0, 50) close(); break: -20 20 }\n",
    );
    let dome = by_id(&l.nodes, "dome");
    // 100 − 40 + 12 = 72 displayed.
    assert!(
        (dome.bbox.w() - 74.0).abs() < 0.1,
        "arc clipped and compressed: {}",
        dome.bbox.w()
    );
}

//! Openings [SPEC 15.11]: the station on a wall segment, the gap it clips in
//! the outline, the jamb-to-jamb box it measures as, and its generated chrome —
//! leaf, quarter swing arc, slider panels, window sills.

use super::wall_path;
use crate::layout::PlacedNode;
use crate::layout::drawing::testutil::{by_id, text_at};
use crate::resolve::ResolvedValue;
use crate::testutil::{laid, layout_err, try_laid};

/// A one-door wall: the gap runs `at` … `at + width` on `run`.
fn one(opening: &str) -> String {
    format!(
        "{{ layout: floorplan; density: 1; thickness: 2 }}\n\
         |wall#w| {{ draw: move(0, 0) right(10):run; }} [\n  {opening}\n]\n"
    )
}

/// Every chrome leaf under a node, as (class, geometry) — a line's two points
/// stringified, an arc's `d`.
fn chrome(node: &PlacedNode) -> Vec<(String, String)> {
    node.children
        .iter()
        .filter_map(|c| {
            let class = c
                .type_chain
                .iter()
                .find(|t| matches!(t.as_str(), "door-leaf" | "door-swing" | "window-sill"))?;
            let geo = match (c.attrs.get("path"), c.attrs.get("points")) {
                (Some(ResolvedValue::String(d)), _) => d.clone(),
                (_, Some(ResolvedValue::List(pts))) => pts
                    .iter()
                    .map(|p| match p {
                        ResolvedValue::Tuple(xy) => format!(
                            "{} {}",
                            xy[0].as_number().expect("x"),
                            xy[1].as_number().expect("y")
                        ),
                        _ => String::new(),
                    })
                    .collect::<Vec<_>>()
                    .join(" → "),
                _ => "unfilled".into(),
            };
            Some((class.clone(), geo))
        })
        .collect()
}

/// The gap **clips** the wall outline [SPEC 15.11]: the run keeps its length
/// and its stations, and each jamb closes flat across the thickness — the
/// open-run butt cap, which is why the cut runs on the centreline.
#[test]
fn a_door_clips_the_outline_and_caps_both_jambs_flat() {
    let l = laid(&one("|door#d| { on: run; at: 3; width: 4 }"));
    assert_eq!(
        wall_path(&l, "w"),
        "M 0 -1 L 3 -1 L 3 1 L 0 1 Z M 7 -1 L 10 -1 L 10 1 L 7 1 Z"
    );
    // …and the wall still measures its whole length: the outline bbox is the
    // run plus the two caps, not the two stumps.
    assert_eq!(by_id(&l.nodes, "w").bbox.w(), 10.0);
}

/// Two openings on one segment each cut their own gap, and a gap running to a
/// segment's very end simply leaves no stump behind it.
#[test]
fn two_openings_on_one_segment_cut_two_gaps() {
    let l = laid(
        "{ layout: floorplan; density: 1; thickness: 2 }\n\
         |wall#w| { draw: move(0, 0) right(10):run; } [\n\
           |door#a| { on: run; at: 2; width: 2 }\n\
           |door#b| { on: run; at: 8; width: 2 }\n\
         ]\n",
    );
    assert_eq!(
        wall_path(&l, "w"),
        "M 0 -1 L 2 -1 L 2 1 L 0 1 Z M 4 -1 L 8 -1 L 8 1 L 4 1 Z"
    );
}

/// A closed run's gap opens the ring: the seam is no longer a corner, so the
/// pieces either side of it rejoin into **one** band rather than two loops.
#[test]
fn a_gap_in_a_closed_run_opens_the_ring_into_one_band() {
    let l = laid(
        "{ layout: floorplan; density: 1; thickness: 2 }\n\
         |wall#w| { draw: move(0, 0) right(10):n down(6):e left(10):s close():x; } [\n\
           |door#d| { on: n; at: 4; width: 2 }\n\
         ]\n",
    );
    let d = wall_path(&l, "w");
    assert_eq!(d.matches('M').count(), 1, "one loop, not two: {d}");
    assert!(
        d.starts_with("M 6 -1 L 11 -1 L 11 7 L -1 7 L -1 -1 L 4 -1"),
        "{d}"
    );
}

/// The true-size defaults [SPEC 15.11]: 900 mm clear for a door, 1200 mm for a
/// window, converted through the scope's `unit:` — and an authored `width:` is
/// drawing units, untouched.
#[test]
fn opening_widths_default_to_their_physical_millimetres() {
    let l = laid(
        "{ layout: floorplan; unit: m; density: 1; thickness: 0.2 }\n\
         |wall#w| { draw: move(0, 0) right(10):run; } [\n\
           |door#d| { on: run; at: 1 }\n\
           |window#v| { on: run; at: 4 }\n\
           |door#e| { on: run; at: 7; width: 2 }\n\
         ]\n",
    );
    // own = 1 ratio × 1000 mm/unit × 1 density = 1000 px per drawing unit.
    assert_eq!(by_id(&l.nodes, "d").bbox.w(), 900.0);
    assert_eq!(by_id(&l.nodes, "v").bbox.w(), 1200.0);
    assert_eq!(by_id(&l.nodes, "e").bbox.w(), 2000.0);
    // The box's other side is the wall's thickness — the jamb-to-jamb box.
    assert_eq!(by_id(&l.nodes, "d").bbox.h(), 200.0);
}

/// The station laws [SPEC 21], one message each.
#[test]
fn the_station_laws_reject_bad_segments_overruns_and_overlaps() {
    assert_eq!(
        layout_err(&one("|door#d| { on: ru; at: 1 }")),
        "'ru' is not a segment of this wall; did you mean 'run'?"
    );
    assert_eq!(
        layout_err(
            "{ layout: floorplan; density: 1; thickness: 2 }\n\
             |wall#w| { draw: move(0, 0) arc(20, 0, 12):bay; } [\n\
               |door#d| { on: bay; at: 1 }\n\
             ]\n"
        ),
        "an opening sits on a straight run — ':bay' is an arc"
    );
    assert_eq!(
        layout_err(
            "{ layout: floorplan; density: 1; thickness: 2 }\n\
             |wall#w| { draw: move(0, 0) right(10) point():corner up(4); } [\n\
               |door#d| { on: corner; at: 1 }\n\
             ]\n"
        ),
        "an opening sits on a straight run — ':corner' is a point"
    );
    // The overrun message states the arithmetic, in drawing units.
    assert_eq!(
        layout_err(&one("|door#d2| { on: run; at: 8; width: 4 }")),
        "'d2' at 8 + width 4 overruns 'run' (length 10)"
    );
    // …and an anonymous opening names its type instead of an id.
    assert_eq!(
        layout_err(&one("|window| { on: run; at: 9.5 }")),
        "'|window|' at 9.5 + width 1200 overruns 'run' (length 10)"
    );
    assert_eq!(
        layout_err(
            "{ layout: floorplan; density: 1; thickness: 2 }\n\
             |wall#w| { draw: move(0, 0) right(10):south; } [\n\
               |door#entry| { on: south; at: 2; width: 3 }\n\
               |window#w1| { on: south; at: 4; width: 3 }\n\
             ]\n"
        ),
        "'entry' and 'w1' overlap on 'south'"
    );
    // Openings that merely touch do not overlap.
    assert!(
        try_laid(
            "{ layout: floorplan; density: 1; thickness: 2 }\n\
             |wall#w| { draw: move(0, 0) right(10):run; } [\n\
               |door#a| { on: run; at: 2; width: 2 }\n\
               |door#b| { on: run; at: 4; width: 2 }\n\
             ]\n"
        )
        .is_ok()
    );
}

/// A `fillet()` trims the **drawn** run back from the corner while the pen
/// keeps naming the segment at its theoretical one [SPEC 15.3] — so `at:` still
/// measures from that corner, and the gap still clips the run it names.
#[test]
fn a_filleted_run_still_takes_its_gap() {
    let l = laid(
        "{ layout: floorplan; density: 1; thickness: 2 }\n\
         |wall#w| { draw: move(0, 0) down(6) fillet(2) right(10):run; } [\n\
           |door#d| { on: run; at: 4; width: 3 }\n\
         ]\n",
    );
    // The run's own start is (0, 6); the fillet draws from x = 2, and `at: 4`
    // seats the near jamb at x = 4 all the same.
    let d = wall_path(&l, "w");
    assert!(d.contains("L 4 5 L 4 7") && d.contains("M 7 5"), "{d}");
    assert_eq!(
        (by_id(&l.nodes, "d").cx, by_id(&l.nodes, "d").cy),
        (5.5, 6.0)
    );
}

/// A named run with no length is a point wearing an edge's name — the same
/// law, the same message [SPEC 21], never a panic in the station arithmetic.
#[test]
fn a_zero_length_named_run_stations_nothing() {
    assert_eq!(
        layout_err(
            "{ layout: floorplan; density: 1; thickness: 2 }\n\
             |wall#w| { draw: move(0, 0) right(4) right(0):zero right(4); } [\n\
               |door#d| { on: zero; at: 0; width: 0 }\n\
             ]\n"
        ),
        "an opening sits on a straight run — ':zero' is a point"
    );
}

/// A door's `symbol:` is a closed variant set like every other, so an unknown
/// one says so in the **one** unknown-symbol wording the fixtures and the
/// schematic discretes share.
#[test]
fn an_unknown_door_symbol_names_the_variants() {
    assert_eq!(
        layout_err(&one("|door#d| { on: run; at: 3; symbol: barn }")),
        "unknown symbol 'barn' on '|door|' — its variants are single, double, sliding"
    );
}

/// **All four poses** [SPEC 15.11/15.5]: `hinge:` picks the jamb by the
/// segment's draw direction, `swing: left` is the left of the pen's travel —
/// which in the opening's own frame is `−y`, at every bearing. The leaf stands
/// 90° open on the swing-side face; the arc sweeps it back to closed.
#[test]
fn the_four_hinge_swing_poses_hang_the_leaf_and_sweep_the_arc() {
    let pose = |style: &str| {
        let l = laid(&one(&format!(
            "|door#d| {{ on: run; at: 3; width: 4; {style} }}"
        )));
        chrome(by_id(&l.nodes, "d"))
    };
    // hinge: start (the −x jamb), swing: left (−y): leaf up off the near jamb,
    // arc sweeping to the far jamb.
    assert_eq!(
        pose(""),
        vec![
            ("door-leaf".into(), "-2 -1 → -2 -5".to_string()),
            ("door-swing".into(), "M -2 -5 A 4 4 0 0 1 2 -1".to_string()),
        ]
    );
    assert_eq!(
        pose("swing: right"),
        vec![
            ("door-leaf".into(), "-2 1 → -2 5".to_string()),
            ("door-swing".into(), "M -2 5 A 4 4 0 0 0 2 1".to_string()),
        ]
    );
    assert_eq!(
        pose("hinge: end"),
        vec![
            ("door-leaf".into(), "2 -1 → 2 -5".to_string()),
            ("door-swing".into(), "M 2 -5 A 4 4 0 0 0 -2 -1".to_string()),
        ]
    );
    assert_eq!(
        pose("hinge: end; swing: right"),
        vec![
            ("door-leaf".into(), "2 1 → 2 5".to_string()),
            ("door-swing".into(), "M 2 5 A 4 4 0 0 1 -2 1".to_string()),
        ]
    );
}

/// The three door symbols and the window [SPEC 15.11]: `double` splits two
/// half-width leaves + arcs about the gap centre, `sliding` trades the arc for
/// a second panel offset to the other face, a window draws two sills at the
/// thickness's thirds.
#[test]
fn each_opening_symbol_draws_its_own_chrome() {
    let of = |opening: &str| {
        let l = laid(&one(opening));
        chrome(by_id(&l.nodes, "d"))
    };
    assert_eq!(
        of("|door#d| { on: run; at: 3; width: 4; symbol: double }"),
        vec![
            ("door-leaf".into(), "-2 -1 → -2 -3".to_string()),
            ("door-swing".into(), "M -2 -3 A 2 2 0 0 1 0 -1".to_string()),
            ("door-leaf".into(), "2 -1 → 2 -3".to_string()),
            ("door-swing".into(), "M 2 -3 A 2 2 0 0 0 0 -1".to_string()),
        ]
    );
    assert_eq!(
        of("|door#d| { on: run; at: 3; width: 4; symbol: sliding }"),
        vec![
            ("door-leaf".into(), "-2 -1 → 0 -1".to_string()),
            ("door-leaf".into(), "2 1 → 0 1".to_string()),
        ]
    );
    assert_eq!(
        of("|window#d| { on: run; at: 3; width: 4 }"),
        vec![
            (
                "window-sill".into(),
                "-2 -0.3333333333333333 → 2 -0.3333333333333333".to_string()
            ),
            (
                "window-sill".into(),
                "-2 0.3333333333333333 → 2 0.3333333333333333".to_string()
            ),
        ]
    );
}

/// The pose reads the segment's **draw** direction, not the screen [SPEC
/// 15.11]. §25's `south` runs right-to-left, so its `swing: right` opens
/// **north** — into the flat — and `at:` measures from the east end.
#[test]
fn a_right_to_left_segment_reverses_what_right_means_on_screen() {
    let l = laid(
        "{ layout: floorplan; density: 1; thickness: 2 }\n\
         |wall#w| { draw: move(0, 0) right(10):north down(6):east left(10):south close():west; } [\n\
           |door#entry| { on: south; at: 3; width: 4; swing: right }\n\
         ]\n",
    );
    let d = by_id(&l.nodes, "entry");
    // The gap centre sits 5 units from the *east* end — x = 10 − 5.
    assert_eq!((d.cx, d.cy), (5.0, 6.0));
    assert_eq!(d.rotation, 180.0);
    // In the turned frame the leaf runs to +y; on screen that is −y, north.
    assert_eq!(
        chrome(d)[0],
        ("door-leaf".to_string(), "-2 1 → -2 5".to_string())
    );
}

/// The §25 location chain [SPEC 15.11/15.6]: an id'd opening's geometry is the
/// jamb-to-jamb box, so a dimension anchors at its **centre** — the two hops
/// read 3.75 and 3.45 and sum to the wall's 7.2.
#[test]
fn an_opening_anchors_the_location_chain_at_its_centre() {
    let l = laid(
        "{ layout: floorplan; unit: m; scale: 0.02 }\n\
         |wall#outer| {\n\
           draw: move(0, 0) right(7.2):north down(4.8):east left(7.2):south close():west;\n\
         } [\n\
           |door#entry| { on: south; at: 3.0; swing: right }\n\
         ]\n\
         outer:west (-) outer.entry (-) outer:east { side: bottom }\n",
    );
    // `south` is drawn east → west, so `at: 3.0` puts the near jamb 3 m from
    // the east end and the centre 3.45 m from it — 3.75 m from the west.
    text_at(&l.nodes, "3.75");
    text_at(&l.nodes, "3.45");
}

/// An opening's smart label is its **schedule tag beside the gap** [SPEC
/// 15.11] — the fixture label's own seat, shared: clear of the wall face by
/// the readout gap, on the face the leaf never sweeps.
#[test]
fn a_schedule_tag_seats_beside_the_gap() {
    let tag = |style: &str| {
        let src = one(&format!(
            "|door#d| \"D1\" {{ on: run; at: 3; width: 4; {style} }}"
        ));
        let l = laid(&src);
        let d = by_id(&l.nodes, "d");
        let t = d
            .children
            .iter()
            .find(|c| c.kind == crate::resolve::NodeKind::Text)
            .expect("the schedule tag");
        (t.cy, t.bbox.h())
    };
    // The wall is 2 units thick, so its faces sit at ±1; the tag's near edge
    // stands one `READOUT_GAP` (8) clear of the far face — never on the wall
    // line, where `|block|`'s centred read would put it.
    let (cy, h) = tag("");
    assert_eq!(cy - h / 2.0, 9.0, "default swing: left opens −y, tag at +y");
    let (cy, h) = tag("swing: right");
    assert_eq!(
        cy + h / 2.0,
        -9.0,
        "…and the tag changes face with the leaf"
    );
}

/// A schedule tag stays **readable like dimension text** [SPEC 15.11] —
/// ISO-aligned, from the bottom or the right, never upside-down: on a wall
/// drawn east → west the opening's frame is turned a half turn, and the tag
/// takes that half turn back.
#[test]
fn a_schedule_tag_on_a_right_to_left_wall_still_reads_upright() {
    let turn = |draw: &str| {
        let l = laid(&format!(
            "{{ layout: floorplan; density: 1; thickness: 2 }}\n\
             |wall#w| {{ draw: {draw} }} [\n  |door#d| \"D1\" {{ on: run; at: 3; width: 4 }}\n]\n"
        ));
        let d = by_id(&l.nodes, "d");
        let t = d
            .children
            .iter()
            .find(|c| c.kind == crate::resolve::NodeKind::Text)
            .expect("the schedule tag");
        (d.rotation, t.attrs.number("rotate").unwrap_or(0.0))
    };
    assert_eq!(
        turn("move(0, 0) right(10):run;"),
        (0.0, 0.0),
        "a wall drawn west → east turns nothing"
    );
    let (frame, back) = turn("move(10, 0) left(10):run;");
    assert_eq!(frame, 180.0, "the opening rides the segment's bearing");
    assert_eq!(back, -180.0, "and the tag reads the other way up");
}

/// The chrome **count** is the authored decls' [SPEC 15.7], so a filler reads
/// the children it was given and never re-derives the count from the cascade:
/// a rule-borne `symbol: double` draws the single door desugar generated, not
/// half of a double one.
#[test]
fn a_rule_borne_symbol_never_half_draws_a_door() {
    let ruled = laid(
        "{ layout: floorplan; density: 1; thickness: 2;\n\
         \x20 |door| { symbol: double; }\n\
         }\n\
         |wall#w| { draw: move(0, 0) right(10):run; } [\n\
         \x20 |door#d| { on: run; at: 3; width: 4 }\n\
         ]\n",
    );
    let plain = laid(&one("|door#d| { on: run; at: 3; width: 4 }"));
    assert_eq!(
        chrome(by_id(&ruled.nodes, "d")),
        chrome(by_id(&plain.nodes, "d"))
    );
}

/// The clip is **geometry**, so it holds under every wall paint [SPEC 15.11]:
/// the hollow double-line and the hatched section cut open the same gap the
/// solid poché does.
#[test]
fn the_gap_is_paint_independent() {
    let solid = laid(&one("|door#d| { on: run; at: 3; width: 4 }"));
    for paint in ["fill: --bg; stroke: --stroke-dark", "fill: hatch(45)"] {
        let l = laid(&format!(
            "{{ layout: floorplan; density: 1; thickness: 2 }}\n\
             |wall#w| {{ {paint}; draw: move(0, 0) right(10):run; }} [\n\
               |door#d| {{ on: run; at: 3; width: 4 }}\n\
             ]\n"
        ));
        assert_eq!(wall_path(&l, "w"), wall_path(&solid, "w"), "{paint}");
    }
}

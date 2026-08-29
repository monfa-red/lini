//! The derived face anchors [SPEC 15.11]: `name-in` / `name-out` on every
//! named wall segment — the clear-span reading a listing plan dimensions.

use crate::layout::LaidOut;
use crate::layout::drawing::Segment;
use crate::layout::drawing::geometry::P;
use crate::layout::drawing::testutil::{by_id, laid, layout_err, text_at};

/// A derived face, as the segment table carries it — the edge's two ends in
/// the wall's own frame.
fn face(l: &LaidOut, id: &str, name: &str) -> (P, P) {
    let geo = by_id(&l.nodes, id).sketch.as_ref().expect("a folded wall");
    match geo.segments.iter().find(|(n, _)| n == name) {
        Some((_, Segment::Edge(a, b))) => (*a, *b),
        other => panic!(
            "no face ':{name}' on '{id}': {other:?} — has {:?}",
            geo.segments.iter().map(|(n, _)| n).collect::<Vec<_>>()
        ),
    }
}

/// The clear room span [SPEC 15.11] — face to face, what a listing plan
/// dimensions: a 4.0 m centreline room inside a 200 mm shell reads **3.85**
/// against a 100 mm partition's near face, while the centreline `:segment`
/// keeps the 4.0 it always read and the outer faces give the overall.
#[test]
fn a_clear_span_measures_face_to_face() {
    let l = laid(
        "{ layout: floorplan; unit: m; scale: 0.02; density: 5 }\n\
         |wall#outer| { draw: move(0, 0) right(6):north down(8):east left(6):south close():west; }\n\
         |partition#part| { draw: move(0, 4) right(6):mid; }\n\
         outer:north-in (-) part:mid-in { side: left }\n\
         outer:north (-) part:mid { side: right }\n\
         outer:west-out (-) outer:east-out { side: top }\n",
    );
    text_at(&l.nodes, "3.85"); // the clear span — panics unless exactly one
    text_at(&l.nodes, "4"); // the centreline, untouched
    text_at(&l.nodes, "6.2"); // the overall, outer face to outer face
}

/// Which face is `-in` [SPEC 15.11]: on a **closed** run the enclosed side —
/// whichever way the pen went round — and on an **open** one the left of its
/// travel. Every face is walked with the material on its right, so its outward
/// points off the wall (what a mate seats against, [SPEC 15.5]).
#[test]
fn in_is_the_enclosed_side_on_a_closed_run_and_the_left_of_travel_on_an_open_one() {
    let l = laid(
        "{ layout: floorplan; density: 1; thickness: 2 }\n\
         |wall#cw| { draw: move(0, 0) right(10):north down(10) left(10) close(); }\n\
         |wall#ccw| { draw: move(0, 40) down(10) right(10) up(10) close():north; }\n\
         |wall#open| { draw: move(0, 80) right(10):run; }\n",
    );
    // Both loops enclose the ground below their north run — one drawn
    // left-to-right, one right-to-left — and both call that side `-in`.
    let (a, b) = face(&l, "cw", "north-in");
    assert_eq!((a.1, b.1), (1.0, 1.0), "the enclosed side");
    assert!(
        b.0 < a.0,
        "the right offset reverses: material on its right"
    );
    assert_eq!(face(&l, "cw", "north-out").0.1, -1.0);
    let (a, b) = face(&l, "ccw", "north-in");
    assert_eq!((a.1, b.1), (41.0, 41.0), "enclosed, drawn the other way");
    assert!(b.0 < a.0, "the seam runs east → west and stays that way");
    assert_eq!(face(&l, "ccw", "north-out").0.1, 39.0);
    // An open run has no inside: `-in` is the left of the pen's travel.
    assert_eq!(face(&l, "open", "run-in").0.1, 79.0);
    assert_eq!(face(&l, "open", "run-out").0.1, 81.0);
}

/// A face anchor spans the **whole** segment [SPEC 15.11] — an opening clips
/// the drawn outline, never the face a dimension names, so a wall carrying a
/// window still reads one clear span, off the face's own midpoint.
#[test]
fn a_face_spans_its_whole_segment_through_the_openings_in_it() {
    let l = laid(
        "{ layout: floorplan; unit: m; scale: 0.02; density: 5 }\n\
         |wall#outer| { draw: move(0, 0) right(6):north down(4):east left(6):south close():west; } [\n\
           |window#w1| { on: north; at: 2; width: 2 }\n\
         ]\n\
         |partition#part| { draw: move(0, 2) right(6):mid; }\n\
         outer:north-in (-) part:mid-in { side: left }\n",
    );
    let own = 0.02 * 1000.0 * 5.0;
    let (a, b) = face(&l, "outer", "north-in");
    assert!(
        (a.0 - 6.0 * own).abs() < 1e-6 && b.0.abs() < 1e-6,
        "the face runs the segment's whole length: {a:?} → {b:?}"
    );
    text_at(&l.nodes, "1.85");
}

/// A face-anchored dimension's witness line springs from the **corner**
/// nearest the dim line, like any edge anchor's [SPEC 15.2] — never the face's
/// midpoint, which would travel the wall.
#[test]
fn a_face_anchored_dimensions_witness_line_leaves_the_corner() {
    let springs = |side: &str| {
        let l = laid(&format!(
            "{{ layout: floorplan; density: 1; thickness: 2 }}\n\
             |wall#w| {{ draw: move(0, 0) right(10):north down(10):east left(10):south close():west; }}\n\
             w:west-in (-) w:east-in {{ side: {side} }}\n"
        ));
        text_at(&l.nodes, "8"); // the clear span, midpoint to midpoint
        let ys: Vec<f64> = l
            .nodes
            .iter()
            .filter(|n| n.type_chain.iter().any(|t| t == "ext-line"))
            .map(|n| {
                crate::layout::primitives::attr_points(&n.attrs, "points", n.span)
                    .unwrap()
                    .unwrap()[0]
                    .1
            })
            .collect();
        assert_eq!(ys.len(), 2, "two extension springs");
        assert!(
            (ys[0] - ys[1]).abs() < 1e-9,
            "both feet on one line: {ys:?}"
        );
        ys[0]
    };
    // Foot to foot is the face's whole 10 plus a gap either way — a midpoint
    // spring would have left only the two gaps between them.
    let (top, bottom) = (springs("top"), springs("bottom"));
    assert!(
        (bottom - top - 16.0).abs() < 1e-6,
        "each witness line leaves its own end: top {top}, bottom {bottom}"
    );
}

/// The derived names are the wall's [SPEC 21], as the built-in point names are
/// the pen's — an authored one collides rather than silently losing its face.
#[test]
fn an_authored_face_name_collides_with_the_derived_one() {
    assert_eq!(
        layout_err(
            "{ layout: floorplan; density: 1 }\n|wall#w| { draw: move(0, 0) right(6):north-in; }\n"
        ),
        "':north-in' collides with the derived face anchor — rename the segment"
    );
}

/// The unknown-segment did-you-mean reads the face names too — they are
/// segments of the wall like any other [SPEC 15.2].
#[test]
fn an_unknown_segment_suggests_the_faces() {
    assert_eq!(
        layout_err(
            "{ layout: floorplan; density: 1 }\n\
             |wall#w| { draw: move(0, 0) right(6):north down(4):east; }\n\
             w:north-i (-) w:east { side: top }\n"
        ),
        "no segment ':north-i' on 'w'; did you mean ':north-in', ':north'?"
    );
}

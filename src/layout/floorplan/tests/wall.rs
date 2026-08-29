//! Walls [SPEC 15.11]: `thickness:` inheritance, the offset outline, poché
//! paint — and an independent oracle over the placed band.

use super::wall_path;
use crate::layout::PlacedNode;
use crate::layout::drawing::geometry::{P, PathSeg, Subpath, arc_center};
use crate::layout::drawing::testutil::{by_id, text_at};
use crate::math;
use crate::testutil::{laid, layout_err, try_laid};

/// An L-corner run [SPEC 15.11]: the outer corner mitres, the inner corner
/// trims to the true crossing (a crisp square notch, never a backtrack), and
/// both open ends butt-cap flat at their endpoints.
#[test]
fn a_wall_offsets_its_centreline_to_the_poche_outline() {
    let l = laid(
        "{ layout: floorplan; density: 1 }\n|wall#w| { thickness: 2; draw: move(0, 0) right(10) down(10); }\n",
    );
    assert_eq!(
        wall_path(&l, "w"),
        "M 0 -1 L 11 -1 L 11 10 L 9 10 L 9 1 L 0 1 Z"
    );
}

/// A closed run grows the two concentric loops — even-odd fills the band
/// between them — and the `close()` seam mitres like any corner; the outline
/// is the geometry bbox [SPEC 15.11/15.1].
#[test]
fn a_closed_run_grows_two_concentric_loops_and_the_seam_mitres() {
    let l = laid(
        "{ layout: floorplan; density: 1 }\n|wall#w| { thickness: 2; draw: move(0, 0) right(10) down(6) left(10) close(); }\n",
    );
    assert_eq!(
        wall_path(&l, "w"),
        "M -1 -1 L 11 -1 L 11 7 L -1 7 Z M 1 1 L 1 5 L 9 5 L 9 1 Z"
    );
    let w = by_id(&l.nodes, "w");
    assert_eq!((w.bbox.w(), w.bbox.h()), (12.0, 8.0), "outline bbox");
}

/// Two solid walls tee by **paint order** [SPEC 15.11/15.1]: each node
/// offsets alone — no boolean union exists — and the shared poché fill makes
/// the junction read seamless (the visual pass pins the look).
#[test]
fn two_solid_walls_tee_seamlessly_by_paint_order() {
    let l = laid(
        "{ layout: floorplan; density: 1; thickness: 2 }\n\
         |wall#a| { draw: move(0, 0) right(20); }\n\
         |wall#b| { draw: move(10, 0) down(12); }\n",
    );
    assert_eq!(wall_path(&l, "a"), "M 0 -1 L 20 -1 L 20 1 L 0 1 Z");
    assert_eq!(wall_path(&l, "b"), "M 11 0 L 11 12 L 9 12 L 9 0 Z");
}

/// An acute corner's miter would spike past limit 4 — it bevels [SPEC 15.11].
#[test]
fn an_acute_spike_bevels_at_the_miter_limit() {
    // A ~12° hairpin: the miter point would sit ~9.5 units past the corner.
    let l = laid(
        "{ layout: floorplan; density: 1 }\n|wall#w| { thickness: 2; draw: move(0, 0) right(10) angle(282, 10); }\n",
    );
    let w = by_id(&l.nodes, "w");
    assert!(w.bbox.w() < 13.0, "bevelled, not spiked: w={}", w.bbox.w());
}

/// An arc offsets to its concentric pair, radially butt-capped [SPEC 15.11].
#[test]
fn an_arc_wall_offsets_to_concentric_faces() {
    let l = laid(
        "{ layout: floorplan; density: 1 }\n|wall#w| { thickness: 2; draw: move(0, 0) arc(10, 0, 5); }\n",
    );
    assert_eq!(
        wall_path(&l, "w"),
        "M -1 0 A 6 6 0 0 1 11 0 L 9 0 A 4 4 0 0 0 1 0 Z"
    );
}

/// The centreline laws [SPEC 21]: `curve()` has no offset; an arc under
/// thickness ∕ 2 has no inner face.
#[test]
fn wall_laws_reject_curve_and_thin_arcs() {
    assert_eq!(
        layout_err(
            "{ layout: floorplan; density: 1 }\n|wall#w| { draw: move(0, 0) curve(5, 0, 10, 5, 15, 0); }\n"
        ),
        "a wall bends with 'arc()' — 'curve()' has no offset"
    );
    assert_eq!(
        layout_err(
            "{ layout: floorplan; density: 1 }\n|wall#w| { thickness: 100; draw: move(0, 0) arc(80, 0, 40); }\n"
        ),
        "arc radius 40 is under thickness/2 — the inner face vanishes"
    );
    // r == thickness/2 stays legal: the inner face degenerates to a point.
    assert!(
        try_laid(
            "{ layout: floorplan; density: 1 }\n|wall#w| { thickness: 80; draw: move(0, 0) arc(80, 0, 40); }\n"
        )
        .is_ok()
    );
}

/// `thickness:` inherits nearest-wins, scope → wall [SPEC 15.11]: the wall's
/// own value first, then the scope's, then the reader's true-size default —
/// and a `|partition|`'s 100 mm is its define's, **at** the node, so the
/// scope's inherited slot never reaches it [SPEC 8].
#[test]
fn thickness_inherits_scope_to_wall_and_partition_keeps_its_define() {
    let l = laid(
        "{ layout: floorplan; density: 1; thickness: 6 }\n\
         |wall#a| { draw: move(0, 0) right(10); }\n\
         |wall#b| { thickness: 2; draw: move(0, 20) right(10); }\n\
         |partition#p| { draw: move(0, 40) right(10); }\n",
    );
    assert_eq!(by_id(&l.nodes, "a").bbox.h(), 6.0, "the scope's value");
    assert_eq!(by_id(&l.nodes, "b").bbox.h(), 2.0, "the wall's own value");
    assert_eq!(by_id(&l.nodes, "p").bbox.h(), 100.0, "the define's 100 mm");
}

/// The true-size law [SPEC 15.11]: the mm defaults convert through the
/// scope's `unit:` — 200 mm is 0.2 drawing units at `unit: m` — while an
/// authored value is drawing units untouched.
#[test]
fn thickness_defaults_are_physical_millimetres() {
    let l = laid(
        "{ layout: floorplan; unit: m; scale: 0.25; density: 1 }\n\
         |wall#w| { draw: move(0, 0) right(4); }\n\
         |partition#p| { draw: move(0, 2) right(4); }\n",
    );
    // own = 0.25 ratio × 1000 mm/unit × 1 density = 250 px per drawing unit.
    assert!((by_id(&l.nodes, "w").bbox.h() - 0.2 * 250.0).abs() < 1e-9);
    assert!((by_id(&l.nodes, "p").bbox.h() - 0.1 * 250.0).abs() < 1e-9);
}

/// The §25 read: `:segment`s stay **centreline** stations — a dimension
/// across the closed rectangle reads 7.2, wall-axis to wall-axis — while
/// bbox anchors read the **outline**, so a mate seats flush on the face
/// [SPEC 15.11/15.2].
#[test]
fn dimensions_station_on_the_centreline_and_bbox_anchors_on_the_outline() {
    let l = laid(
        "{ layout: floorplan; unit: m; scale: 0.02 }\n\
         |wall#outer| {\n\
           draw: move(0, 0) right(7.2):north down(4.8):east left(7.2):south close():west;\n\
         }\n\
         |rect#r| { width: 0.6; height: 0.4 }\n\
         r:bottom || outer:top\n\
         outer:west (-) outer:east { side: top }\n",
    );
    text_at(&l.nodes, "7.2"); // panics unless exactly one such value renders
    // own = 0.02 × 1000 × 4 = 80 px/unit; the outline's top face sits at
    // −0.1 units; the 0.4-unit-high rect seats flush above it.
    let r = by_id(&l.nodes, "r");
    assert!(
        (r.cy - (-0.1 * 80.0 - 0.2 * 80.0)).abs() < 1e-6,
        "flush on the outline face: cy={}",
        r.cy
    );
}

/// *The anchor aims; the outline lands* [SPEC 15.2]: a leader against a wall
/// tips on the offset outline, not the centreline.
#[test]
fn a_leader_tip_lands_on_the_wall_outline() {
    let l = laid(
        "{ layout: floorplan; density: 1 }\n\
         |wall#w| { thickness: 2; draw: move(0, 0) right(10):north down(6); }\n\
         w:north <- \"W1\"\n",
    );
    let tip = l
        .nodes
        .iter()
        .find(|n| n.type_chain.iter().any(|t| t == "marker-dim"))
        .map(|n| {
            crate::layout::primitives::attr_points(&n.attrs, "points", n.span)
                .unwrap()
                .unwrap()[0]
        })
        .expect("the leader's arrowhead");
    assert!(
        (tip.1 - (-1.0)).abs() < 1e-6 && (tip.0 - 5.0).abs() < 1e-6,
        "tip on the outline face above the segment midpoint: {tip:?}"
    );
}

// ── The band properties [SPEC 15.11] — an independent oracle over the
// placed outline: sampled Green's-theorem area and perimeter. ──

/// Sample an outline element into a dense polyline (arcs at 512 steps).
fn sample(seg: &PathSeg) -> Vec<P> {
    match *seg {
        PathSeg::Line { from, to } => vec![from, to],
        PathSeg::Arc {
            from,
            to,
            r,
            large,
            sweep,
        } => {
            let c = arc_center(from, to, r, large, sweep);
            let ang = |p: P| math::atan2(p.1 - c.1, p.0 - c.0);
            let (a0, a1) = (ang(from), ang(to));
            let tau = 2.0 * std::f64::consts::PI;
            let d = if sweep {
                (a1 - a0).rem_euclid(tau)
            } else {
                -((a0 - a1).rem_euclid(tau))
            };
            (0..=512)
                .map(|i| {
                    let a = a0 + d * (i as f64 / 512.0);
                    (c.0 + r * math::cos(a), c.1 + r * math::sin(a))
                })
                .collect()
        }
        PathSeg::Cubic { .. } => unreachable!("no cubics in a wall outline"),
    }
}

/// Shoelace area (absolute) and perimeter of the sampled outline loops.
fn band_metrics(node: &PlacedNode) -> (f64, f64) {
    let outline: &[Subpath] = &node.sketch.as_ref().expect("a folded wall").outline;
    let (mut area, mut per) = (0.0, 0.0);
    for sub in outline {
        let mut loop_area = 0.0;
        for seg in &sub.segs {
            let pts = sample(seg);
            for pair in pts.windows(2) {
                let (p, q) = (pair[0], pair[1]);
                loop_area += p.0 * q.1 - q.0 * p.1;
                per += math::hypot(q.0 - p.0, q.1 - p.1);
            }
        }
        // Loops are closed; even-odd makes the signed loops subtract.
        area += loop_area / 2.0;
    }
    (area.abs(), per)
}

/// A straight run at any bearing offsets to an exact L × t rectangle:
/// area L·t, perimeter 2(L + t) — the parallel-offset distance law.
#[test]
fn a_straight_run_at_any_bearing_is_an_exact_band() {
    for bearing in (0..360).step_by(7) {
        let l = laid(&format!(
            "{{ layout: floorplan; density: 1 }}\n|wall#w| {{ thickness: 2; draw: move(0, 0) angle({bearing}, 10); }}\n"
        ));
        let (area, per) = band_metrics(by_id(&l.nodes, "w"));
        assert!((area - 20.0).abs() < 1e-6, "bearing {bearing}: area {area}");
        assert!((per - 24.0).abs() < 1e-6, "bearing {bearing}: per {per}");
    }
}

/// Mitred corners conserve the band: the outer kite equals the inner trim,
/// so a gentle run's area is exactly centreline length × thickness — and an
/// arc's band is (r+h)²−(r−h)² about the same angle, the same product.
#[test]
fn a_wall_band_keeps_length_times_thickness_area() {
    // A zig-zag of lines, mitre joins only (wedges of 135°).
    let l = laid(
        "{ layout: floorplan; density: 1 }\n|wall#w| { thickness: 3; draw: move(0, 0) right(20) angle(135, 10) right(20); }\n",
    );
    let (area, _) = band_metrics(by_id(&l.nodes, "w"));
    assert!((area - 50.0 * 3.0).abs() < 1e-6, "zig-zag area {area}");
    // A semicircular arc: L = 5π, t = 2.
    let l = laid(
        "{ layout: floorplan; density: 1 }\n|wall#w| { thickness: 2; draw: move(0, 0) arc(10, 0, 5); }\n",
    );
    let (area, per) = band_metrics(by_id(&l.nodes, "w"));
    let want = 5.0 * std::f64::consts::PI * 2.0;
    assert!((area - want).abs() / want < 1e-3, "arc band area {area}");
    let want_per = 10.0 * std::f64::consts::PI + 4.0;
    assert!(
        (per - want_per).abs() / want_per < 1e-3,
        "arc band per {per}"
    );
}

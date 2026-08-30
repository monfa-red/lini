use super::super::testutil::{by_id, laid, layout_err, texts};

#[test]
fn a_plane_spans_the_view_and_names_its_ends() {
    // A 120-wide plate; the plane A–A at the centre (longer axis x → a
    // vertical line), two letters, arrows facing right by default.
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#plate| { width: 120; height: 40 }\n|plane| \"A\" { at: 0 }\n",
    );
    let cp = by_id(&l.nodes, "plate"); // the plane is a sibling; find its texts
    let _ = cp;
    let letters: Vec<_> = texts(&l.nodes)
        .into_iter()
        .filter(|(t, ..)| t == "A")
        .collect();
    assert_eq!(letters.len(), 2, "a letter beside each end: {letters:?}");
}

#[test]
fn a_planes_thick_ends_stand_past_the_geometry() {
    // The ISO anatomy [SPEC 15.8]: the chain line crosses the view and
    // overhangs it, and the thick end strokes sit **just past** each end —
    // clear of the outline, never on it. Both lengths are baked sheet
    // constants: a plane crosses what it cuts, so the packer's `clearance`
    // never moves it.
    use crate::layout::drawing::section::plane::PLANE_END;
    use crate::ledger::consts::{PLANE_OVERHANG, PLANE_THICK_END};

    let span = |clearance: f64| {
        let l = laid(&format!(
            "{{ layout: drawing; density: 1; clearance: {clearance} }}\n             |rect#plate| {{ width: 120; height: 40 }}\n|plane#a| \"A\" {{ at: 0 }}\n",
        ));
        let plate = by_id(&l.nodes, "plate").bbox.max_y;
        let cp = by_id(&l.nodes, "a");
        let line = ends(cp);
        let thick: Vec<(f64, f64)> = cp
            .children
            .iter()
            .filter(|c| c.type_chain.iter().any(|t| t == PLANE_END))
            .map(ends)
            .collect();
        (plate, line, thick)
    };

    let (plate, line, thick) = span(4.0);
    assert!(
        (line.1 - (plate + PLANE_OVERHANG + PLANE_THICK_END)).abs() < 0.01,
        "the line overhangs the geometry by the stand-off and the end: {line:?} past {plate}"
    );
    assert_eq!(thick.len(), 2, "one thick end per end: {thick:?}");
    for (near, far) in &thick {
        let (inner, outer) = (near.abs().min(far.abs()), near.abs().max(far.abs()));
        assert!(
            (inner - (plate + PLANE_OVERHANG)).abs() < 0.01 && (outer - line.1).abs() < 0.01,
            "the end stroke stands entirely past the outline: {inner}..{outer} vs {plate}"
        );
    }
    // `clearance` packs annotation rows [SPEC 15.6]; the plane's anatomy is
    // sheet-space chrome and does not read it.
    assert_eq!(span(50.0).1, line, "the line is clearance-independent");
    assert_eq!(span(200.0).2, thick, "so are its ends");
}

/// A placed `|plane|` piece's two `points:` stations along the line — the
/// signed y of its ends (the test's planes all run vertically).
#[cfg(test)]
fn ends(n: &crate::layout::PlacedNode) -> (f64, f64) {
    let Some(crate::resolve::ResolvedValue::List(pts)) = n.attrs.get("points") else {
        panic!("a plane piece carries its points");
    };
    let y = |v: &crate::resolve::ResolvedValue| match v {
        crate::resolve::ResolvedValue::Tuple(xy) => xy[1].as_number().expect("a number"),
        _ => panic!("a point is a pair"),
    };
    (y(&pts[0]), y(&pts[1]))
}

#[test]
fn at_off_the_model_errors() {
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|rect#plate| { width: 40; height: 40 }\n|plane| \"A\" { at: 90 }\n",
        ),
        "a 'plane' at 90 sits off the model"
    );
}

#[test]
fn bad_facing_errors() {
    assert_eq!(
        layout_err(
            "{ layout: drawing; density: 1 }\n|rect#plate| { width: 40; height: 40 }\n|plane| \"A\" { at: 0; facing: sideways }\n",
        ),
        "'facing' turns the arrows — left, right, up, or down"
    );
}

#[test]
fn a_detail_view_re_lays_the_region_titles_and_clips_and_dims_the_clone() {
    // A plate with a marker `c`; the detail magnifies it 2:1 (scale 8 over
    // the page's 4) and dimensions the **clone** (40, pre-scale, deferred
    // past resolve) — the source has no such dimension.
    let l = laid(
        "|page#p| { sheet: a5 landscape } [\n  |drawing#m| { scale: 1 } [\n    |rect#plate| { width: 40; height: 20 }\n    |magnifier#c| \"C\" { width: 30 }\n  ]\n  |drawing#d| { of: c; scale: 2 } [\n    plate:left (-) plate:right { side: bottom }\n  ]\n]\n",
    );
    let all = texts(&l.nodes);
    assert!(
        all.iter().any(|(t, ..)| t == "C (2:1)"),
        "composed detail title: {all:?}"
    );
    assert!(
        all.iter().any(|(t, ..)| t == "40"),
        "the clone's dimension: {all:?}"
    );
    let d = by_id(&l.nodes, "d");
    assert!(
        d.children.iter().any(|c| c.attrs.get("clip").is_some()),
        "the detail clips its geometry to the region circle"
    );
}

#[test]
fn of_a_missing_marker_errors() {
    assert!(
            layout_err(
                "|page#p| { sheet: a5 } [\n  |drawing#m| { scale: 1 } [ |rect#r| { width: 10; height: 10 } ]\n  |drawing#d| { of: nope }\n]\n",
            )
            .contains("'of' finds no marker 'nope'")
        );
}

#[test]
fn a_detail_circle_sets_its_letter_at_the_rim() {
    let l = laid(
        "{ layout: drawing; density: 1 }\n|rect#plate| { width: 60; height: 60 }\n|magnifier#c| \"C\" { width: 20; translate: 15 0 }\n",
    );
    let c = by_id(&l.nodes, "c");
    let letter = c
        .children
        .iter()
        .find(|t| t.label.as_deref() == Some("C"))
        .expect("the rim letter");
    // Up-and-right of the centre (positive x, negative y).
    assert!(
        letter.cx > 0.0 && letter.cy < 0.0,
        "at the 45° rim: {},{}",
        letter.cx,
        letter.cy
    );
}

use super::super::testutil::{by_id, laid, layout_err, texts};

#[test]
fn a_plane_spans_the_view_and_names_its_ends() {
    // A 120-wide plate; the plane A–A at the centre (longer axis x → a
    // vertical line), two letters, arrows facing right by default.
    let l = laid(
        "{ layout: drawing; scale: 1 }\n|rect#plate| { width: 120; height: 40 }\n|plane| \"A\" { at: 0 }\n",
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
fn at_off_the_model_errors() {
    assert_eq!(
        layout_err(
            "{ layout: drawing; scale: 1 }\n|rect#plate| { width: 40; height: 40 }\n|plane| \"A\" { at: 90 }\n",
        ),
        "a 'plane' at 90 sits off the model"
    );
}

#[test]
fn bad_facing_errors() {
    assert_eq!(
        layout_err(
            "{ layout: drawing; scale: 1 }\n|rect#plate| { width: 40; height: 40 }\n|plane| \"A\" { at: 0; facing: sideways }\n",
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
        "|page#p| { sheet: a5 landscape } [\n  |drawing#m| { scale: 4 } [\n    |rect#plate| { width: 40; height: 20 }\n    |magnifier#c| \"C\" { width: 30 }\n  ]\n  |drawing#d| { of: c; scale: 8 } [\n    plate:left (-) plate:right { side: bottom }\n  ]\n]\n",
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
                "|page#p| { sheet: a5 } [\n  |drawing#m| { scale: 4 } [ |rect#r| { width: 10; height: 10 } ]\n  |drawing#d| { of: nope }\n]\n",
            )
            .contains("'of' finds no marker 'nope'")
        );
}

#[test]
fn a_detail_circle_sets_its_letter_at_the_rim() {
    let l = laid(
        "{ layout: drawing; scale: 1 }\n|rect#plate| { width: 60; height: 60 }\n|magnifier#c| \"C\" { width: 20; translate: 15 0 }\n",
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

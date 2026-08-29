//! The dialect's two halves [SPEC 15.11]: a floorplan scope **is** a drawing
//! scope (everything SPEC 15 gives a drawing arrives unchanged), and its own
//! vocabulary is gated to it (a `|wall|` elsewhere is an error, a `|pin|` here
//! is one exactly as it is in a drawing).

use crate::testutil::{laid, layout_err, try_laid};

/// A wall long enough to be the geometry child every drawing scope needs.
const WALL: &str = "|wall#w| { draw: move(0, 0) right(4000):north down(3000):east; }\n";

fn scope(body: &str) -> String {
    format!("|floorplan#f| [\n{body}]\n")
}

/// Every floorplan type outside a `layout: floorplan` is an error, and legal
/// the moment the scope encloses it — including inside a plain `|drawing|`,
/// which shares the engine but not the vocabulary [SPEC 15.11/21].
#[test]
fn every_floorplan_type_belongs_in_a_floorplan_scope() {
    let cases: &[(&str, &str)] = &[
        ("|wall| { draw: move(0, 0) right(50); }\n", "wall"),
        ("|partition| { draw: move(0, 0) right(50); }\n", "partition"),
        ("|door| { on: north; }\n", "door"),
        ("|window| { on: north; }\n", "window"),
        ("|bed|\n", "bed"),
        ("|sofa|\n", "sofa"),
        ("|dining|\n", "dining"),
        ("|bath|\n", "bath"),
        ("|appliance|\n", "appliance"),
        ("|stairs| { steps: 12; }\n", "stairs"),
    ];
    for (part, ty) in cases {
        let want = format!("'|{ty}|' belongs in a 'layout: floorplan'");
        assert_eq!(layout_err(part), want, "{part}");
        assert_eq!(
            layout_err(&format!("|drawing#d| [\n  {part}]\n")),
            want,
            "a drawing shares the engine, not the vocabulary: {part}"
        );
    }
    // A define over one is gated by the type it builds on, and reported by the
    // name the author wrote.
    assert_eq!(
        layout_err("{ |stud::wall| { } }\n|stud| { draw: move(0, 0) right(50); }\n"),
        "'|stud|' belongs in a 'layout: floorplan'"
    );
    // …and `|floorplan|` itself is exempt: it *creates* the scope.
    assert!(try_laid(&scope(&format!("  {WALL}"))).is_ok());
}

/// The dialect is the drawing engine [SPEC 15.11]: the pen, the datum, the
/// measuring ops and `unit:` all read in a floorplan exactly as in a drawing —
/// and another layout's vocabulary is refused here just as it is there.
#[test]
fn a_floorplan_scope_is_a_drawing_scope() {
    let laid = laid(
        "{ layout: floorplan; unit: m; scale: 0.02 }\n\
         |sketch#s| { draw: move(0, 0) right(4):a down(3); }\n\
         s:left (-) s:right { side: bottom }\n",
    );
    assert!(
        !laid.nodes.is_empty(),
        "the drawing engine placed the scene"
    );
    // The scope's own children datum-place, so the dimension lowered rather
    // than routing: a drawing scope consumes its links [SPEC 11].
    assert!(laid.links.is_empty(), "a drawing scope owns its links");
    // A schematic type is out of scope here, the same error a drawing gives.
    assert_eq!(
        layout_err(&scope(&format!("  {WALL}  |pin#p|\n"))),
        "'|pin|' belongs in a 'layout: schematic'"
    );
}

/// An opening's own laws [SPEC 15.11/21], each read once by the family gate.
#[test]
fn an_opening_rides_its_wall_and_is_placed_by_its_station() {
    let wall_with = |opening: &str| {
        scope(&format!(
            "  |wall#w| {{ draw: move(0, 0) right(4000):north; }} [\n    {opening}\n  ]\n"
        ))
    };

    // It rides in a wall's `[ ]` — not beside one, and not in another part.
    assert_eq!(
        layout_err(&scope(&format!("  {WALL}  |door| {{ on: north; }}\n"))),
        "a '|door|' rides in its wall's '[ ]'"
    );
    assert_eq!(
        layout_err(&scope(&format!(
            "  {WALL}  |rect#r| {{ width: 40; height: 20 }} [ |window| {{ on: north; }} ]\n"
        ))),
        "a '|window|' rides in its wall's '[ ]'"
    );
    // `on:` is required, as `points:` is on a `|line|`.
    assert_eq!(
        layout_err(&wall_with("|door| { at: 1000; }")),
        "'|door|' requires 'on' — the wall segment it stations on"
    );
    // It is placed by `on:` / `at:` alone.
    assert_eq!(
        layout_err(&wall_with("|door| { on: north; translate: 10 0; }")),
        "an opening sits at 'on:' / 'at:' — move the station, or nudge the wall"
    );
    // A sliding door has no leaf to hang.
    for pose in ["hinge: end", "swing: right"] {
        assert_eq!(
            layout_err(&wall_with(&format!(
                "|door| {{ on: north; symbol: sliding; {pose}; }}"
            ))),
            "a sliding door has no leaf to hang — remove 'hinge:' / 'swing:'"
        );
    }
    // …while the same door without a pose is fine, and so is a hinged one that
    // is not sliding.
    assert!(try_laid(&wall_with("|door| { on: north; symbol: sliding; }")).is_ok());
    assert!(
        try_laid(&wall_with(
            "|door| { on: north; hinge: end; swing: right; }"
        ))
        .is_ok()
    );
}

/// A flight generates from its tread count, so it needs one [SPEC 15.11].
#[test]
fn stairs_need_their_tread_count() {
    assert_eq!(
        layout_err(&scope(&format!("  {WALL}  |stairs|\n"))),
        "'|stairs|' requires 'steps' — its tread count"
    );
    assert!(try_laid(&scope(&format!("  {WALL}  |stairs| {{ steps: 14 }}\n"))).is_ok());
}

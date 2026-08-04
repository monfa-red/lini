//! The anchor track grid and the role table [SPEC 16.1]: one row by default,
//! `columns:` wraps, ordinal `cell:` collapses, every track sizes to its
//! widest anchor — and which children ride a track at all.

use super::tests::{anchor, at, cell, close, laid, placed, pose_of, scope, sided, x_gap, y_gap};
use crate::ledger::defaults::SCH_GAP;

// ───────────────────────── tracks ─────────────────────────

#[test]
fn anchors_take_one_row_in_declaration_order() {
    let src = scope(
        "",
        &(anchor("u1", "") + &anchor("u2", "") + &anchor("u3", "")),
    );
    let nodes = laid(&src);
    let [(x1, y1), (x2, y2), (x3, y3)] = ["u1", "u2", "u3"].map(|id| at(&nodes, id));
    assert!(x1 < x2 && x2 < x3, "declaration order: {x1} {x2} {x3}");
    assert!(close(y1, y2) && close(y2, y3), "one row: {y1} {y2} {y3}");
    assert!(
        close(x_gap(&nodes, "u1", "u2"), SCH_GAP),
        "the track gap defaults to {SCH_GAP}: {}",
        x_gap(&nodes, "u1", "u2")
    );
}

#[test]
fn columns_wraps_the_flow() {
    let src = scope(
        " { columns: 2 }",
        &(anchor("u1", "") + &anchor("u2", "") + &anchor("u3", "")),
    );
    let nodes = laid(&src);
    let [(x1, y1), (x2, y2), (x3, y3)] = ["u1", "u2", "u3"].map(|id| at(&nodes, id));
    assert!(close(y1, y2), "the first two share row 1: {y1} {y2}");
    assert!(y3 > y1, "the third wrapped to row 2: {y3} vs {y1}");
    assert!(close(x1, x3), "and back to column 1: {x1} vs {x3}");
    assert!(x2 > x1, "column 2 is to the right: {x2}");
    assert!(
        close(y_gap(&nodes, "u1", "u3"), SCH_GAP),
        "rows are one gap apart: {}",
        y_gap(&nodes, "u1", "u3")
    );
}

#[test]
fn sparse_cell_ordinals_collapse_to_adjacent_tracks() {
    // [SPEC 16.1] tracks are **ordinal**: 10 / 20 / 30 is ordering room, not
    // 30 columns — the empty ordinals collapse entirely, so the sheet is
    // identical to 1 / 2 / 3.
    let sparse = scope(
        "",
        &(anchor("u1", " { cell: 10 1 }")
            + &anchor("u2", " { cell: 20 1 }")
            + &anchor("u3", " { cell: 30 1 }")),
    );
    let dense = scope(
        "",
        &(anchor("u1", " { cell: 1 1 }")
            + &anchor("u2", " { cell: 2 1 }")
            + &anchor("u3", " { cell: 3 1 }")),
    );
    let (a, b) = (laid(&sparse), laid(&dense));
    for id in ["u1", "u2", "u3"] {
        let (ax, ay) = at(&a, id);
        let (bx, by) = at(&b, id);
        assert!(
            close(ax, bx) && close(ay, by),
            "{id}: {ax},{ay} vs {bx},{by}"
        );
    }
    assert!(
        close(x_gap(&a, "u1", "u2"), SCH_GAP),
        "no invisible space between collapsed tracks: {}",
        x_gap(&a, "u1", "u2")
    );
}

#[test]
fn sparse_row_ordinals_collapse_too() {
    let nodes = laid(&scope(
        "",
        &(anchor("u1", " { cell: 1 5 }") + &anchor("u2", " { cell: 1 9 }")),
    ));
    let ((x1, y1), (x2, y2)) = (at(&nodes, "u1"), at(&nodes, "u2"));
    assert!(close(x1, x2), "one column: {x1} {x2}");
    assert!(y2 > y1, "row order follows the ordinals: {y1} {y2}");
    assert!(
        close(y_gap(&nodes, "u1", "u2"), SCH_GAP),
        "two adjacent rows, not nine: {}",
        y_gap(&nodes, "u1", "u2")
    );
}

#[test]
fn an_explicit_cell_beats_the_flow() {
    // `u2` claims column 1; the flowing anchors take the slots left free, in
    // declaration order — so the sheet reads u1, u3 either side of it.
    let nodes = laid(&scope(
        "",
        &(anchor("u1", "") + &anchor("u2", " { cell: 1 1 }") + &anchor("u3", "")),
    ));
    let [(x1, _), (x2, _), (x3, _)] = ["u1", "u2", "u3"].map(|id| at(&nodes, id));
    assert!(x2 < x1 && x1 < x3, "u2 owns column 1: {x2} {x1} {x3}");
}

#[test]
fn an_explicit_cell_may_reach_past_the_wrap_count() {
    // `columns:` wraps the **flow**; an explicit ordinal places, so `cell: 4 1`
    // is a fourth collapsed column beside a two-column flow.
    let nodes = laid(&scope(
        " { columns: 2 }",
        &(anchor("u1", "") + &anchor("u2", "") + &anchor("u3", " { cell: 4 1 }")),
    ));
    let [(x1, y1), (x2, y2), (x3, y3)] = ["u1", "u2", "u3"].map(|id| at(&nodes, id));
    assert!(
        close(y1, y2) && close(y2, y3),
        "all on row 1: {y1} {y2} {y3}"
    );
    assert!(x1 < x2 && x2 < x3, "column order: {x1} {x2} {x3}");
}

#[test]
fn a_track_sizes_to_its_widest_anchor() {
    // Every track size goes through the cluster seam, and a track takes the
    // **max** over every anchor in it —
    // including one in another row. `u3` (wide, tall) sits in column 1 / row 2,
    // so it is what pushes column 2 rightward and row 2 downward; nothing in
    // row 1 or column 2 could account for the offsets on its own.
    let nodes = laid(&scope(
        " { columns: 2 }",
        &(anchor("u1", " { cell: 1 1 }")
            + &anchor("u2", " { cell: 2 1 }")
            + &anchor("u3", " { cell: 1 2; width: 200; height: 120 }")),
    ));
    let (x1, y1, w1, h1) = cell(&nodes, "u1");
    let (x2, _, w2, h2) = cell(&nodes, "u2");
    let (x3, y3, w3, h3) = cell(&nodes, "u3");
    assert!(
        w3 > w1 && w3 > w2 && h3 > h1 && h3 > h2,
        "u3 really is the widest and tallest: {w3}x{h3} vs {w1}x{h1}, {w2}x{h2}"
    );
    // Column 1 is `w3` wide even though `u3` sits a row down — a rule that
    // sized the column to its own row's anchor would land `u2` at
    // `w1/2 + gap + w2/2`, a good 30 px short.
    assert!(
        close(x2 - x1, w3 / 2.0 + SCH_GAP + w2 / 2.0),
        "column 1 sized to its widest anchor: {} vs {}",
        x2 - x1,
        w3 / 2.0 + SCH_GAP + w2 / 2.0
    );
    assert!(
        close(x3, x1),
        "both column-1 anchors centre in it: {x1} {x3}"
    );
    // The row axis reads the same seam: row 1 is as tall as its tallest.
    assert!(
        close(y3 - y1, h1.max(h2) / 2.0 + SCH_GAP + h3 / 2.0),
        "row 1 sized to its tallest anchor: {} vs {}",
        y3 - y1,
        h1.max(h2) / 2.0 + SCH_GAP + h3 / 2.0
    );
}

// ───────────────────────── roles ─────────────────────────

#[test]
fn pin_arity_classifies_the_role_never_the_type() {
    // [SPEC 16.1's role table] a 3-pin part anchors; an authored **two**-pin
    // `|component|` — a jumper — is a satellite, exactly like a `|R|`, so the
    // rule is arity, not the type name. Satellites leave the tracks (today:
    // the trailing fallback row).
    let nodes = laid(&scope(
        "",
        &(anchor("u1", "")
            + "  |component#j1| [\n    |pin#a|; |pin#b|\n  ]\n"
            + "  |R#r1| \"1k\"\n"),
    ));
    let (_, uy) = at(&nodes, "u1");
    let (jx, jy) = at(&nodes, "j1");
    let (rx, ry) = at(&nodes, "r1");
    assert!(jy > uy && ry > uy, "both satellites sit off the track row");
    assert!(close(jy, ry), "in one fallback row: {jy} {ry}");
    assert!(jx < rx, "declaration order: {jx} {rx}");
}

#[test]
fn an_anonymous_parts_pins_still_count() {
    // An anonymous part generates no port nodes — they would leak into the
    // parent's scope [SPEC 9] — so arity cannot be read off the lowered tree:
    // an unnamed three-pin `|Q|` anchors, an unnamed `|R|` still seats.
    let nodes = laid(&scope("", &(anchor("u1", "") + "  |Q|\n  |R| \"1k\"\n")));
    let (s, _, _) = placed(&nodes, "s");
    // Anonymous, so measured off the scope's own children: each part's drawn
    // centre, which is what a track holds [SPEC 16.1].
    let [u1, q, r] = [0, 1, 2].map(|i| {
        let c = &s.children[i];
        super::seat::drawn(c).shifted(c.cx, c.cy).center().1
    });
    assert!(close(u1, q), "the 3-pin |Q| rides the track row: {u1} {q}");
    assert!(r > q, "the 2-pin |R| seats below: {r} vs {q}");
}

#[test]
fn a_label_is_a_satellite_and_a_cell_promotes_it() {
    let unplaced = laid(&scope("", &(anchor("u1", "") + "  |gnd#g1|\n")));
    let (_, uy) = at(&unplaced, "u1");
    let (_, gy) = at(&unplaced, "g1");
    assert!(gy > uy, "a |label| seats, never tracks: {gy} vs {uy}");

    // `cell:` promotes a satellite to an anchor [SPEC 16.1] — it rides a track.
    let promoted = laid(&scope(
        "",
        &(anchor("u1", " { cell: 1 1 }") + "  |gnd#g1| { cell: 2 1 }\n"),
    ));
    let (ux, uy, ..) = cell(&promoted, "u1");
    let (gx, gy, ..) = cell(&promoted, "g1");
    assert!(
        close(uy, gy),
        "the promoted label shares the row: {uy} {gy}"
    );
    assert!(gx > ux, "in column 2: {gx} vs {ux}");
}

#[test]
fn only_cell_promotes_a_satellite_translate_just_nudges_it() {
    // [SPEC 16.1] the two are **not** the same knob: `cell:` promotes a
    // satellite to an anchor, `translate:` only moves it off whatever seat it
    // already has. The part is **wired**, so the whole seat path runs: a nudge
    // must leave the pose and the growth direction exactly as they were and
    // add a delta — the pose chooser and the seat pass have to agree on the
    // role, or a no-op nudge silently re-seats the part somewhere else.
    let seated = |style: &str| {
        laid(&scope(
            "",
            &(sided("u1") + "  |gnd#g1|" + style + "\n  u1.a - g1\n"),
        ))
    };
    let bare = seated("");
    let (bx, by) = at(&bare, "g1");
    let moved = seated(" { translate: 7 13 }");
    let (mx, my) = at(&moved, "g1");
    assert_eq!(pose_of(&bare, "g1"), 0, "the pose its own drawing asks for");
    assert_eq!(
        pose_of(&moved, "g1"),
        0,
        "a nudge is no promotion — the pose stands"
    );
    assert!(
        close(mx, bx + 7.0) && close(my, by + 13.0),
        "an exact nudge off the seat it kept: {bx},{by} → {mx},{my}"
    );
    // Even a no-op nudge must not move it: the role must not change.
    let still = seated(" { translate: 0 0 }");
    let (sx, sy) = at(&still, "g1");
    assert!(close(sx, bx) && close(sy, by), "0 0 is nothing: {sx},{sy}");

    // `cell:`, by contrast, really does take it off the seat and onto a track.
    let celled = laid(&scope(
        "",
        &(sided("u1") + "  |gnd#g1| { cell: 2 1 }\n  u1.a - g1\n"),
    ));
    let ((ux, uy, ..), (cgx, cgy, ..)) = (cell(&celled, "u1"), cell(&celled, "g1"));
    assert!(
        close(uy, cgy) && cgx > ux,
        "`cell:` puts it on the track row"
    );
    assert_eq!(pose_of(&celled, "g1"), 0, "and an anchor is never posed");
    let (gx, gy) = at(&celled, "g1");
    let nudged = laid(&scope(
        "",
        &(sided("u1") + "  |gnd#g1| { cell: 2 1; translate: 7 13 }\n  u1.a - g1\n"),
    ));
    let (nx, ny) = at(&nudged, "g1");
    assert!(
        close(nx, gx + 7.0) && close(ny, gy + 13.0),
        "the nudge is exact from the track seat too: {gx},{gy} → {nx},{ny}"
    );
}

#[test]
fn a_pinned_child_is_an_overlay_on_the_finished_sheet() {
    // The drawing precedent [SPEC 5/15.8]: `pin:` lifts a child out of the
    // tracks onto the scope's content box, and the scope never grows for it.
    let src = scope(
        "",
        &(anchor("u1", "") + &anchor("u2", "") + "  |box#note| \"note\" { pin: top right }\n"),
    );
    let nodes = laid(&src);
    let (_, u1x, _) = placed(&nodes, "u1");
    let (_, u2x, _) = placed(&nodes, "u2");
    let (note, nx, ny) = placed(&nodes, "note");
    let (s, sx, sy) = placed(&nodes, "s");
    assert!(
        close(nx + note.bbox.max_x, sx + s.bbox.max_x)
            && close(ny + note.bbox.min_y, sy + s.bbox.min_y),
        "flush in the scope's top-right corner"
    );
    // The two anchors still own the whole row — the overlay took no track.
    let plain = laid(&scope("", &(anchor("u1", "") + &anchor("u2", ""))));
    assert!(
        close(u2x - u1x, at(&plain, "u2").0 - at(&plain, "u1").0),
        "the overlay claimed no cell"
    );
}

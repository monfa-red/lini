//! The anchor track grid and the role table [SPEC 16.1]: one row by default,
//! `columns:` wraps, ordinal `cell:` collapses, every track sizes to its
//! widest anchor — and which children ride a track at all.

use super::tests::{
    anchor, at, cell, close, laid, on_fine_grid, placed, pose_of, scope, seat, sided, x_gap,
};
use crate::layout::PlacedNode;
use crate::ledger::consts::PIN_PITCH;
use crate::ledger::defaults::SCH_GAP;

/// How many coarse cells apart two anchors stand [SPEC 16.1] — a track is
/// packed in whole cells, so this is always a whole number, and the ink
/// between two of them is whatever their own widths leave.
fn apart(nodes: &[PlacedNode], from: &str, to: &str, vertical: bool) -> f64 {
    let (a, b) = (at(nodes, from), at(nodes, to));
    (if vertical { b.1 - a.1 } else { b.0 - a.0 }) / SCH_GAP
}

/// Whether a cell distance is a whole number of them, and at least one.
fn whole_cells(d: f64) -> bool {
    close(d, d.round()) && d >= 1.0
}

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
        whole_cells(apart(&nodes, "u1", "u2", false)),
        "a whole number of coarse cells apart: {}",
        apart(&nodes, "u1", "u2", false)
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
        whole_cells(apart(&nodes, "u1", "u3", true)),
        "rows stand a whole number of coarse cells apart: {}",
        apart(&nodes, "u1", "u3", true)
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
        whole_cells(apart(&a, "u1", "u2", false)),
        "no invisible space between collapsed tracks: {}",
        apart(&a, "u1", "u2", false)
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
        whole_cells(apart(&nodes, "u1", "u2", true)),
        "two adjacent rows, not nine: {}",
        apart(&nodes, "u1", "u2", true)
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
    // A track is packed in whole coarse cells and takes the **max** over every
    // anchor in it — including one in another row. `u3` (wide and tall) sits in
    // column 1 / row 2, so it is what parts column 2 and row 2; a rule reading
    // only its own row's anchors would leave both where the bare sheet has
    // them.
    let sheet = |u3: &str| {
        laid(&scope(
            " { columns: 2 }",
            &(anchor("u1", " { cell: 1 1 }") + &anchor("u2", " { cell: 2 1 }") + u3),
        ))
    };
    let wide = sheet(&anchor("u3", " { cell: 1 2; width: 400; height: 240 }"));
    let bare = sheet(&anchor("u3", " { cell: 1 2 }"));
    let (col, row) = (
        apart(&wide, "u1", "u2", false),
        apart(&wide, "u1", "u3", true),
    );
    assert!(
        col > apart(&bare, "u1", "u2", false),
        "column 1 sized to its widest anchor, a row down: {col} vs {}",
        apart(&bare, "u1", "u2", false)
    );
    assert!(
        row > apart(&bare, "u1", "u3", true),
        "and row 1 to its tallest: {row} vs {}",
        apart(&bare, "u1", "u3", true)
    );
    assert!(
        whole_cells(col) && whole_cells(row),
        "both in whole coarse cells: {col} {row}"
    );
    assert!(
        close(at(&wide, "u3").0, at(&wide, "u1").0),
        "and both column-1 anchors stand on its line"
    );
}

#[test]
fn a_rail_spaces_its_pins_at_the_pitch_whichever_side_it_runs_along() {
    // [SPEC 16.2] the pitch is the spacing **along a rail**, so it is a pin's
    // height on a left/right rail and its width on a top/bottom one. Read as a
    // height either way, a row of bottom pins would crowd to its names' widths
    // and each name would float half a pitch of empty box off the body edge.
    let part = |side: &str| {
        format!(
            "  |component#u1| [\n    |pin#p| {{ side: {side} }}; |pin#q| {{ side: {side} }}\n    |pin#z| {{ side: {} }}\n  ]\n",
            if side == "left" { "right" } else { "left" }
        )
    };
    for (side, along) in [("left", 1), ("right", 1), ("top", 0), ("bottom", 0)] {
        let nodes = laid(&scope("", &part(side)));
        let (p, q) = (cell(&nodes, "p"), cell(&nodes, "q"));
        let step = if along == 1 { q.1 - p.1 } else { q.0 - p.0 };
        assert!(
            close(step.abs(), PIN_PITCH),
            "'{side}' pins sit one pitch apart along their rail: {step}"
        );
    }
}

#[test]
fn a_space_is_one_empty_slot_on_its_rail_and_keeps_the_lattice() {
    // [SPEC 16.2] a `|space|` is one empty slot on the rail of the pin before
    // it — a whole fine pitch, `span: N` for N — and its slots count toward
    // the odd-slot rule, so every pin still lands on a fine line.
    let whole = |v: f64| close((v / PIN_PITCH).round() * PIN_PITCH, v);
    let part = "  |component#u1| [\n    |pin#a| { side: left }; |space| { span: 2 }; |pin#b| { side: left }; |pin#c| { side: left }\n  ]\n";
    let nodes = laid(&scope("", part));
    let (_, _, cy) = placed(&nodes, "u1");
    let y = |id: &str| placed(&nodes, id).2;
    assert!(
        close(y("b") - y("a"), 3.0 * PIN_PITCH),
        "two empty slots between a and b: {} vs {}",
        y("a"),
        y("b")
    );
    assert!(
        close(y("c") - y("b"), PIN_PITCH),
        "no space between b and c"
    );
    for id in ["a", "b", "c"] {
        assert!(
            whole(y(id) - cy),
            "'{id}' is off the lattice by {}",
            y(id) - cy
        );
    }
}

#[test]
fn a_components_pins_stand_a_whole_pitch_from_its_centre() {
    // [SPEC 16.2] the rails seat so every pin lands on a **fine** lattice line,
    // whatever their count: an even rail straddles its own middle, and a part
    // carrying only one horizontal rail drags its side pins off the body's
    // centre with it. Either way the pin sits half a pitch out, and a wire to a
    // neighbour's pin can no longer run straight — the alignment shift is a
    // whole number of pitches [SPEC 16.1], so what it cannot reach, it jogs.
    let whole = |v: f64| close((v / PIN_PITCH).round() * PIN_PITCH, v);
    let stands = |what: &str, part: &str, pins: &[(String, bool)]| {
        let nodes = laid(&scope("", part));
        let (_, cx, cy) = placed(&nodes, "u1");
        for (id, vertical) in pins {
            let (_, px, py) = placed(&nodes, id);
            // Along the rail it stands on: a side pin's row, a top or bottom
            // pin's column — the coordinate its wire arrives on.
            let along = if *vertical { py - cy } else { px - cx };
            assert!(whole(along), "{what}: '{id}' sits {along} off the centre");
        }
    };
    for n in 1..=6 {
        let ids: Vec<(String, bool)> = (1..=n).map(|i| (format!("p{i}"), true)).collect();
        let pins: Vec<String> = ids.iter().map(|(id, _)| format!("|pin#{id}|")).collect();
        stands(
            &format!("{n} split pins"),
            &format!("  |component#u1| [ {} ]\n", pins.join("; ")),
            &ids,
        );
    }
    for sides in [
        ["left", "right", "bottom"],
        ["left", "right", "top"],
        ["left", "top", "bottom"],
        ["bottom", "bottom", "left"],
    ] {
        let ids: Vec<(String, bool)> = sides
            .iter()
            .enumerate()
            .map(|(i, s)| (format!("p{}", i + 1), *s == "left" || *s == "right"))
            .collect();
        let pins: Vec<String> = ids
            .iter()
            .zip(sides)
            .map(|((id, _), side)| format!("|pin#{id}| {{ side: {side} }}"))
            .collect();
        stands(
            &sides.join("/"),
            &format!("  |component#u1| [ {} ]\n", pins.join("; ")),
            &ids,
        );
    }
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
    // Anonymous, so measured off the scope's own children: each part's own
    // centre, which is what lands on a track's line [SPEC 16.1].
    let [u1, q, r] = [0, 1, 2].map(|i| {
        let c = &s.children[i];
        c.cy + c.bbox.center().1
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
    let (ux, uy) = at(&promoted, "u1");
    let (gx, gy) = at(&promoted, "g1");
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
    let ((ux, uy), (cgx, cgy)) = (at(&celled, "u1"), at(&celled, "g1"));
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

#[test]
fn a_second_part_on_a_taken_cell_errors() {
    // [SPEC 16.1/21]: two explicit `cell:`s on one ordinal used to stack the
    // parts silently and stray every wire off the buried one.
    let err = super::tests::layout_err(&scope(
        "",
        "  |R#r1| \"1k\" { cell: 1 1 }\n  |R#r2| \"2k\" { cell: 1 1 }\n",
    ));
    assert_eq!(
        err,
        "cell 1 1 already holds 'r1' — give 'r2' its own ordinal"
    );
}

#[test]
fn facing_pins_align_across_tracks_and_their_wire_runs_straight() {
    // [SPEC 16.1] anchors sharing a row seat so wired facing pins share
    // their row exactly: cluster centring alone offsets neighbouring pin
    // rows by whatever the satellite asymmetry is, and every part-to-part
    // wire jogs — two neighbouring jogs then collide below the minimum
    // pitch. A real sheet runs these wires dead straight.
    let nodes = laid(&scope(
        "",
        &("  |component#u1| [\n    |pin#vin| { side: left }; |pin#en| { side: left }; |pin#out| { side: right }\n  ]\n"
            .to_owned()
            // The asymmetric cluster: a ground hanging under u1's left pin
            // shifts u1's cluster centre off u2's.
            + "  |component#u2| [\n    |pin#in| { side: left }; |pin#nc| { side: left }; |pin#o2| { side: right }\n  ]\n"
            + "  u1.vin - |gnd|\n  u1.out - u2.in\n"),
    ));
    let (ay, by) = (cell(&nodes, "out").1, cell(&nodes, "in").1);
    assert!(
        close(ay, by),
        "the wired facing pins share a row: {ay} vs {by}"
    );
}

#[test]
fn a_defines_own_gap_and_clearance_reach_the_scope_it_opens() {
    // [SPEC 16.6] opting into the engine is one decision, so a container the
    // cascade makes a schematic scope is *given* the sheet's track spacing
    // and clearance — but only where nothing states them. The config lands on
    // the instance's own block, tier 5, so asking that block alone let it
    // outrank the very define that opened the scope: `|region::group| {
    // layout: schematic; gap: 100 }` drew at 60 and looked inert.
    let sep = |gap: f64| {
        let src = format!(
            "{{ |region::group| {{ layout: schematic; gap: {gap} }} }}\n\
             |region#r| [\n\
             {}{}  u1.b - u2.a\n]\n",
            anchor("u1", " { cell: 1 1 }"),
            anchor("u2", " { cell: 2 1 }"),
        );
        let nodes = laid(&src);
        at(&nodes, "u2").0 - at(&nodes, "u1").0
    };
    assert!(
        sep(300.0) > sep(100.0) && close(sep(100.0) % 100.0, 0.0) && close(sep(300.0) % 300.0, 0.0),
        "the define's own gap parts the tracks, in its own cells: {} vs {}",
        sep(100.0),
        sep(300.0)
    );
}

#[test]
fn a_nested_scope_stands_exactly_where_its_parent_put_it() {
    // [SPEC 16.1] a scope's parts stand on multiples of its pitch in the
    // scope's **own** frame, and the router counts its track quantum from the
    // scope's origin (ROUTING.md §Vocabulary) — so the scope never has to move
    // onto the scene's grid, and the parent's `gap` is honoured to the pixel:
    // a snap would have swallowed anything under half a pitch of it.
    let sheet = |gap: u32| {
        format!(
            "{{ |region::group| {{ layout: schematic }} }}\n\
             |row| {{ gap: {gap}; padding: 3 }} [\n\
             |block#shim| {{ width: 33; height: 11 }}\n\
             |region#r| [\n{}]\n]\n",
            sided("u1") + "  |C#c1| \"1u\"\n  |gnd#g1|\n  u1.c - c1 - g1\n"
        )
    };
    let (a, b) = (laid(&sheet(7)), laid(&sheet(11)));
    for (nodes, gap) in [(&a, 7.0), (&b, 11.0)] {
        let have = x_gap(nodes, "shim", "r");
        assert!(
            close(have, gap),
            "the row's gap stands to the pixel: {have} for {gap}"
        );
    }
    // The scope's origin is off the scene's lattice, which is fine…
    let origin = at(&b, "r").0;
    assert!(!on_fine_grid(origin), "an off-grid seat stands: {origin}");
    // …because every part inside is still on the scope's own lattice.
    for id in ["u1", "c1", "g1"] {
        let (x, y) = seat(&b, id);
        assert!(
            on_fine_grid(x - origin) && on_fine_grid(y),
            "'{id}' at {x} {y} is off the scope's lattice (origin {origin})"
        );
    }
}

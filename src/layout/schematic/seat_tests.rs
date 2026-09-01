//! The satellite seat pass [SPEC 16.1]: a chain grows outward from its pin,
//! auto-posed to face it; chains on one pin stack; two placed ends distribute;
//! no placed end flows with a warning; and every seat rides its anchor.

use super::seat::LABEL_SEAT;
use super::tests::{
    anchor, at, body, cell, close, ink, laid, placed, pose_of, scope, seat_warnings, sided,
    sided_with, tip, x_gap, y_gap,
};
use crate::layout::PlacedNode;
use crate::ledger::defaults::SCH_GAP;

#[test]
fn a_chain_grows_the_way_its_terminator_faces() {
    // [SPEC 16.1] the ray is the **terminator's** own connection geometry: a
    // `|gnd|` is drawn with its point at the top, so a chain ending in one
    // grows **down** — off a bottom pin straight down its axis, and off a side
    // pin one seat out along the wire's first leg and then down.
    let nodes = laid(&scope("", &(sided("u1") + "  |gnd#g1|\n  u1.c - g1\n")));
    let (px, py, ..) = cell(&nodes, "c");
    let (gx, gy, ..) = cell(&nodes, "g1");
    assert!(gy > py, "below the bottom pin: {gy} vs {py}");
    assert!(close(gx, px), "and centred on it: {gx} vs {px}");

    for (pin, dir) in [("a", -1.0), ("b", 1.0)] {
        let nodes = laid(&scope(
            "",
            &(sided("u1") + "  |gnd#g1|\n  u1." + pin + " - g1\n"),
        ));
        let (px, py, ..) = cell(&nodes, pin);
        let (gx, gy, ..) = cell(&nodes, "g1");
        assert!(
            (gx - px) * dir > 0.0,
            "pin {pin} hangs its chain off the wire's leg: {gx} vs {px}"
        );
        assert!(gy > py, "and the ground still hangs below: {gy} vs {py}");
    }
}

#[test]
fn auto_pose_turns_a_satellite_to_face_back_up_the_ray() {
    // The chooser walks `Pose::ALL` for the first pose presenting the
    // satellite's terminal back along the growth ray [SPEC 16.1]. The
    // terminator sets that ray from its **own** drawing, so a `|gnd|` — point
    // at the top, chain growing down — never turns, whichever pin holds it.
    for pin in ["a", "b", "c"] {
        let nodes = laid(&scope(
            "",
            &(sided("u1") + "  |gnd#g1|\n  u1." + pin + " - g1\n"),
        ));
        assert_eq!(pose_of(&nodes, "g1"), 0, "a ground hangs upright off {pin}");
    }
    // A part *inside* the chain does turn: the resistor's `p1` is drawn at its
    // left end, and the ray runs down, so it stands a quarter clockwise.
    let nodes = laid(&scope(
        "",
        &(sided("u1") + "  |R#r1| \"1k\"\n  |gnd#g1|\n  u1.a - r1.p1\n  r1.p2 - g1\n"),
    ));
    assert_eq!(pose_of(&nodes, "r1"), 90, "p1 turned up, into the wire");
}

#[test]
fn an_authored_rotate_forces_the_pose_and_the_seat_follows_it() {
    // [SPEC 16.1] an explicit `rotate:` forces the pose; the seat direction
    // then derives from the **rotated** connection point — so a ground held
    // upright on a left-facing pin hangs below it, not beside it.
    let nodes = laid(&scope(
        "",
        &(sided("u1") + "  |gnd#g1| { rotate: 0 }\n  u1.a - g1\n"),
    ));
    assert_eq!(pose_of(&nodes, "g1"), 0, "the authored pose stands");
    let ((ux, uy), (gx, gy)) = (at(&nodes, "u1"), at(&nodes, "g1"));
    assert!(gy > uy, "seated below its own connection point: {gy} {uy}");
    assert!(gx < ux, "off the left pin it hangs from: {gx} {ux}");
}

#[test]
fn a_chain_grows_link_by_link() {
    // Each link seats farther out along the same ray, in wire order: the
    // grounded chain runs **down** past the pin it leaves, the resistor above
    // its ground, and the whole column stands off to the pin's own side.
    let column = |pin: &str, dir: f64| {
        let nodes = laid(&scope(
            "",
            &(sided("u1")
                + "  |R#r1| \"1k\"\n  |gnd#g1|\n  u1."
                + pin
                + " - r1.p1\n  r1.p2 - g1\n"),
        ));
        let [(ux, uy), (rx, ry), (gx, gy)] = ["u1", "r1", "g1"].map(|id| at(&nodes, id));
        assert!(uy < ry && ry < gy, "u1 → r1 → g1 downward: {uy} {ry} {gy}");
        assert!(
            (rx - gx).abs() < 1.0,
            "one column, to a half-stroke: {rx} vs {gx}"
        );
        assert!(
            (rx - ux) * dir > 0.0,
            "off the {pin} side of the part: {rx} vs {ux}"
        );
    };
    column("a", -1.0);
    column("b", 1.0);
}

#[test]
fn chains_on_one_pin_stack_in_statement_order_one_seat_apart() {
    // [SPEC 16.1] several chains on one pin stack — through the shared
    // packer, so the second stands clear of the first by the seat gap.
    let nodes = laid(&scope(
        "",
        &(sided("u1") + "  |gnd#g1|\n  |gnd#g2|\n  u1.c - g1\n  u1.c - g2\n"),
    ));
    let ((_, y1), (_, y2)) = (at(&nodes, "g1"), at(&nodes, "g2"));
    assert!(y2 > y1, "declaration order stacks outward: {y1} {y2}");
    assert!(
        close(y_gap(&nodes, "g1", "g2"), LABEL_SEAT),
        "one seat gap apart: {}",
        y_gap(&nodes, "g1", "g2")
    );
    // The **parts'** order decides, like everything else the engine places —
    // writing the wires the other way round changes nothing.
    let rewired = laid(&scope(
        "",
        &(sided("u1") + "  |gnd#g1|\n  |gnd#g2|\n  u1.c - g2\n  u1.c - g1\n"),
    ));
    assert!(at(&rewired, "g2").1 > at(&rewired, "g1").1);
    // Declaring them the other way round does.
    let redeclared = laid(&scope(
        "",
        &(sided("u1") + "  |gnd#g2|\n  |gnd#g1|\n  u1.c - g1\n  u1.c - g2\n"),
    ));
    assert!(at(&redeclared, "g1").1 > at(&redeclared, "g2").1);
}

#[test]
fn a_chain_that_turns_onto_its_ray_takes_a_lane_of_its_own() {
    // [SPEC 16.1] a chain leaving its pin sideways gets its own lane, so its
    // lead is one square turn — out along the pin, then away. Sharing a lane
    // would stand one chain's body over the next one's leg, and the router,
    // which may not cross a body, would jog that leg into a staircase.
    let nodes = laid(&scope(
        "",
        &("  |component#u1| [\n    |pin#a| { side: left }; |pin#b| { side: left }; |pin#c| { side: right }\n  ]\n"
            .to_owned()
            + "  |gnd#ga|\n  |gnd#gb|\n  u1.a - ga\n  u1.b - gb\n"),
    ));
    let (pay, pby) = (cell(&nodes, "a").1, cell(&nodes, "b").1);
    let (tax, tbx) = (tip(&nodes, "a", false), tip(&nodes, "b", false));
    assert!(close(tax, tbx), "both stubs tip on one rail: {tax} {tbx}");
    let ((gax, gay), (gbx, gby)) = (at(&nodes, "ga"), at(&nodes, "gb"));
    assert!(!close(gax, gbx), "a lane each: {gax} {gbx}");
    // The lower pin's chain keeps its own depth; the upper one's descends
    // past the lower **wired row** — that row is a corridor [SPEC 16.1] —
    // which lands the two grounds level, the way a sheet drops both to one
    // rail height.
    assert!(
        gay - pay > gby - pby,
        "the upper chain clears the lower corridor: {} vs {}",
        gay - pay,
        gby - pby
    );
    assert!(close(gay, gby), "…landing level: {gay} {gby}");
}

#[test]
fn chains_turning_onto_one_ray_from_opposite_sides_keep_their_own_lanes() {
    // [SPEC 16.1] the lane ladder exists so no two leads cross — but a chain
    // turning **left** off a left pin and one turning **right** off a right pin
    // stand on opposite sides of the part and can never cross, so neither may
    // be stepped past the other's reach. Laddered together, the second-sorted
    // chain seated visibly farther out than its mirror for no reason a reader
    // can see.
    let nodes = laid(&scope(
        "",
        &(sided("u1") + "  |gnd#ga|\n  |gnd#gb|\n  u1.a - ga\n  u1.b - gb\n"),
    ));
    let left = tip(&nodes, "a", false) - at(&nodes, "ga").0;
    let right = at(&nodes, "gb").0 - tip(&nodes, "b", true);
    assert!(
        close(left, right),
        "mirror chains stand off their pins alike: {left} vs {right}"
    );
}

#[test]
fn two_pins_of_one_side_never_share_a_lane_however_their_chains_grow() {
    // [SPEC 16.1] one ladder per **side**, not per growth ray: an up-chain off
    // one pin and a down-chain off the pin below it leave along the one lane
    // axis, so laddered per ray they both take the innermost lane and their
    // two leads turn on one column a pin pitch apart — the drain's rail flag
    // standing over the source's return, one broken line where a reader sees
    // a short (the hero's fan drive). Only a *pin's own* pair shares.
    // A bare symbol, so a node's centre *is* its connection point and the two
    // lanes compare directly.
    let sheet = "{ |flag::label| { symbol: power } }\n";
    let part = "  |component#u1| [\n    |pin#d| { side: right }; |pin#s| { side: right }; |pin#l| { side: left }\n  ]\n";
    let nodes = laid(
        &(sheet.to_owned()
            + &scope(
                "",
                &(part.to_owned() + "  |flag#f|\n  |gnd#g|\n  u1.d - f\n  u1.s - g\n"),
            )),
    );
    let (fx, gx) = (at(&nodes, "f").0, at(&nodes, "g").0);
    assert!(
        !close(fx, gx),
        "the flag and the ground take a lane each: {fx} vs {gx}"
    );
    // The pair *on one pin* still leaves on one lead and splits once — the
    // rail up to its flag, down to its decoupling cap, one column.
    let shared = laid(
        &(sheet.to_owned()
            + &scope(
                "",
                &(part.to_owned() + "  |flag#f|\n  |gnd#g|\n  u1.d - f\n  u1.d - g\n"),
            )),
    );
    let (sfx, sgx) = (at(&shared, "f").0, at(&shared, "g").0);
    assert!(
        close(sfx, sgx),
        "one pin's up- and down-chain share one column: {sfx} vs {sgx}"
    );
}

#[test]
fn a_side_growing_one_way_ladders_along_that_ray_not_the_canonical_one() {
    // [SPEC 16.1] a chain's leg crosses every lane inside its own, so the
    // chain whose pin sits **earlier along the ray** has to step out. Read
    // canonically (down, right) that is right for a side whose chains grow
    // down and backwards for one whose chains grow up — the fan header's
    // rail flag laddering outside the tach column, which then crossed its
    // lead. The canonical reading is owed only to a side holding **both**
    // rays, where one pin's up- and down-chain share a column and the two
    // ladders have to agree.
    let sheet = "{ |flag::label| { symbol: power } }\n";
    let part = "  |component#u1| [\n    |pin#hi| { side: right }; |pin#lo| { side: right }; |pin#l| { side: left }\n  ]\n";
    let lanes = |chains: &str| {
        let nodes = laid(&(sheet.to_owned() + &scope("", &(part.to_owned() + chains))));
        let wall = {
            let (u, ux, _) = placed(&nodes, "u1");
            ux - u.cx + super::seat::drawn(u).max_x
        };
        (at(&nodes, "a").0 - wall, at(&nodes, "b").0 - wall)
    };
    // Both chains grow **up**: the upper pin is the deeper one along that
    // ray, so it keeps the inner lane and the lower pin's column steps out
    // past it — nothing crosses.
    let (hi, lo) = lanes("  |flag#a|\n  |flag#b|\n  u1.hi - a\n  u1.lo - b\n");
    assert!(
        hi < lo,
        "the upper pin's rail keeps the inner lane: {hi} vs {lo}"
    );
    // Both grow **down**, and the order mirrors: the lower pin is now the
    // deeper one and keeps the inner lane.
    let (hi, lo) = lanes("  |gnd#a|\n  |gnd#b|\n  u1.hi - a\n  u1.lo - b\n");
    assert!(
        lo < hi,
        "the lower pin's return keeps the inner lane: {lo} vs {hi}"
    );
}

/// A part whose left side holds three wired pins, `a` over `b` over `c` — the
/// canonical lane ladder, one column per pin.
fn laddered(values: [&str; 3]) -> Vec<PlacedNode> {
    let [v1, v2, v3] = values;
    laid(&scope(
        "",
        &("  |component#u1| [\n    |pin#a| { side: left }; |pin#b| { side: left }; |pin#c| { side: left }\n  ]\n"
            .to_owned()
            + &format!("  |R#r1| \"{v1}\"\n  |R#r2| \"{v2}\"\n  |R#r3| \"{v3}\"\n")
            + "  u1.a - r1 - |gnd|\n  u1.b - r2 - |gnd|\n  u1.c - r3 - |gnd|\n"),
    ))
}

#[test]
fn a_ladder_steps_its_columns_on_one_pitch() {
    // [SPEC 16.1] a ladder's columns stand on **one** pitch — the greediest
    // step any neighbouring pair of them asks, taken by them all. Stepped
    // pair by pair the gaps between the *ink* come out even and the columns
    // do not, so the pitch wobbles with nothing more meaningful than how many
    // characters each part's value happens to read: the hero's MCU core
    // stepped its five columns 92.8, 93.8, 79.7 and 107.0 apart, which a
    // reader sees as columns dropped at random rather than as a grid.
    //
    // `r2`'s long value is the greedy one: its readout runs outward past its
    // own lane, so the column after it has to clear that text — and every
    // other column now steps by the same amount.
    let nodes = laddered(["1k", "1000000000k", "1k"]);
    let lane = |id| at(&nodes, id).0;
    // The deepest pin keeps the inner lane, so the ladder reads c, b, a.
    let (inner, mid, outer) = (lane("r3"), lane("r2"), lane("r1"));
    assert!(
        close(mid - inner, outer - mid),
        "one pitch for the whole ladder: {} vs {}",
        mid - inner,
        outer - mid
    );
    // And the pitch is the greedy step itself, not some average of them: the
    // ladder is exactly as wide as two of the widest column's steps.
    let plain = laddered(["1k", "1k", "1k"]);
    let step = at(&plain, "r2").0 - at(&plain, "r3").0;
    assert!(
        (mid - inner).abs() > step.abs(),
        "the greediest pair sets it: {} vs {step}",
        mid - inner
    );
}

#[test]
fn a_span_reserves_only_what_its_leg_will_really_swallow() {
    // [SPEC 16.1] the track demand and the seat itself read one `swallow`.
    // Measured two ways they disagree: the demand parted the tracks for a
    // cluster the leg then passed clear of, and the member — struck at even
    // fractions of what was left — split the slack and stood a lane of
    // nothing beside itself (105 px of it in the hero's 24 V entry).
    //
    // The scene stages exactly that. The deep stack under `u2` rides its
    // `s` pin far above `j1`, so the landing leg runs clear over `j1`'s
    // whole cluster, ground flag and all; the member's long value makes the
    // span — not the two clusters and the gap — what parts the tracks, so
    // there *is* a reserve to be wrong about.
    let sep = |ground: &str| {
        let nodes = laid(&scope(
            "",
            &("  |J#j1| \"PH\" { pins: 2; cell: 1 1; rotate: 180 }\n".to_owned()
                + "  |component#u2| { cell: 2 1 } [\n    |pin#s| { side: left }; |pin#gp| { side: bottom }\n  ]\n"
                + "  |R#r1| \"1000000000kkkkkkkk\"\n  |gnd#gb|\n"
                + "  |R#ra| \"1k\"\n  |R#rb| \"1k\"\n  |R#rc| \"1k\"\n"
                + "  j1.p2 - r1 - u2.s\n"
                + ground
                + "  u2.gp - ra - rb - rc - |gnd|\n"),
        ));
        at(&nodes, "u2").0 - at(&nodes, "j1").0
    };
    let (with, without) = (sep("  j1.p1 - gb\n"), sep(""));
    assert!(
        close(with, without),
        "a ground the leg passes clear of costs the span nothing: {with} vs {without}"
    );
}

#[test]
fn a_lane_answers_to_the_symbol_not_to_the_name_beside_it() {
    // [SPEC 16.1] the seat gap is measured on what a wire arrives at — a
    // flag's symbol — and the name beside it only may not reach back over the
    // part. Charged on the name too, a connector wired to a flag on one side
    // and a bare ground on the other stands them off by visibly different
    // amounts, lopsided for no reason a reader can see.
    let part = "  |component#u1| [\n    |pin#l| { side: left }; |pin#r| { side: right }; |pin#d| { side: bottom }\n  ]\n";
    let lanes = |name: &str| {
        // The define is the stylesheet's, so the source is built around the
        // scope rather than through the `scope` helper's body alone.
        let sheet = format!("{{ |flag::label| {{ symbol: power }} [ \"{name}\" ] }}\n");
        let nodes = laid(
            &(sheet
                + &scope(
                    "",
                    &(part.to_owned() + "  |flag#f|\n  |gnd#g|\n  u1.l - f\n  u1.r - g\n"),
                )),
        );
        // What a reader compares: stub tip to the far tip of the symbol, the
        // whole span each side of the part takes up.
        let (lx, rx) = (tip(&nodes, "l", false), tip(&nodes, "r", true));
        (lx - ink(&nodes, "f").min_x, ink(&nodes, "g").max_x - rx)
    };
    let (flag, ground) = lanes("VM");
    assert!(
        close(flag, ground),
        "a short name stands off exactly as the bare ground does: {flag} vs {ground}"
    );
    // A name long enough to actually reach the part pushes its own lane out —
    // the case the clearance exists for.
    let (long, ground) = lanes("VM_VERY_VERY_LONG");
    assert!(
        long > ground,
        "a name that would reach the part moves out: {long} vs {ground}"
    );
}

#[test]
fn chains_on_different_pins_seat_in_the_order_their_pins_do() {
    // [SPEC 16.1] no chain overtakes another. A chain that turns onto its ray
    // competes for a lane and its leg crosses every lane inside its own, so the
    // one off the **shallower** pin has to take the outer lane; take them as
    // declared instead and the two leads cross. The order is the pins' own, so
    // the sheet reads the same however the parts are written.
    let two_left = "  |component#u1| [\n    |pin#a| { side: left }; |pin#b| { side: left }; |pin#c| { side: right }\n  ]\n";
    // Declared either way round — the order a capsule terminator (`- |gnd|`)
    // hoists in, so this is the ordinary spelling's own variation.
    for order in ["  |gnd#ga|\n  |gnd#gb|\n", "  |gnd#gb|\n  |gnd#ga|\n"] {
        let nodes = laid(&scope(
            "",
            &(two_left.to_owned() + order + "  u1.a - ga\n  u1.b - gb\n"),
        ));
        let (ay, by) = (cell(&nodes, "a").1, cell(&nodes, "b").1);
        assert!(ay < by, "pin 'a' is the upper one: {ay} {by}");
        let (ga, gb) = (at(&nodes, "ga").1, at(&nodes, "gb").1);
        assert!(
            ga < gb + 1e-6,
            "the upper pin's ground never sinks below the lower pin's, declared {order:?}: {ga} {gb}"
        );
    }
}

#[test]
fn two_placed_ends_fill_the_leg_the_tracks_parted_for_them() {
    // [SPEC 16.1] the members pack against the second end's landing, and the
    // tracks part by **exactly** what that packing takes — so between two
    // bare parts there is nothing else on the leg: one member lands on the
    // midpoint and a pair straddles it, in wire order, a seat gap apart. A
    // reserve wider than the pack (the even-fraction step asked `n+1` of the
    // greediest pair) leaves the difference as blank beside the wire coming
    // in, which is the one thing the seat can never fill.
    let ends = "  |component#u1| { cell: 1 1 } [ |pin#l| { side: right } ]\n                \x20 |component#u2| { cell: 2 1 } [ |pin#r| { side: left } ]\n";
    let one = laid(&scope(
        "",
        &(ends.to_string() + "  |R#r1| \"1k\"\n  u1.l - r1.p1\n  r1.p2 - u2.r\n"),
    ));
    let (a, b) = (tip(&one, "l", true), tip(&one, "r", false));
    let (x1, ..) = body(&one, "r1");
    assert!(
        close(x1, (a + b) / 2.0),
        "one satellite halves the span: {x1} vs {}",
        (a + b) / 2.0
    );
    let two = laid(&scope(
        "",
        &(ends.to_string()
            + "  |R#r1| \"1k\"\n  |R#r2| \"2k\"\n"
            + "  u1.l - r1.p1\n  r1.p2 - r2.p1\n  r2.p2 - u2.r\n"),
    ));
    let (a, b) = (tip(&two, "l", true), tip(&two, "r", false));
    let (x1, ..) = body(&two, "r1");
    let (x2, ..) = body(&two, "r2");
    assert!(
        a < x1 && x1 < x2 && x2 < b,
        "wire order along the leg: {a} {x1} {x2} {b}"
    );
    assert!(
        close(x1 + x2, a + b),
        "the pair centres between symmetric parts: {x1} {x2} in [{a}, {b}]"
    );
    assert!(
        close(x_gap(&two, "r1", "r2"), LABEL_SEAT),
        "packed neighbours stand exactly a seat apart: {}",
        x_gap(&two, "r1", "r2")
    );
}

#[test]
fn a_spanning_chain_sizes_the_space_it_lands_in() {
    // [SPEC 16.1] a chain held at both ends rides no track and joins no
    // cluster, so nothing but this sizes the space it is struck in: at the
    // scope's own gap its satellites dropped into the 48 between two adjacent
    // tracks, on top of the very pins they wire to. The tracks now part by
    // what the chain asks — its own extent, so a wider part parts them
    // further, which no constant bump could do.
    let ends = "  |component#u1| { cell: 1 1 } [ |pin#l| { side: right } ]\n                \x20 |component#u2| { cell: 2 1 } [ |pin#r| { side: left } ]\n";
    let span = |sat: &str, wire: &str| {
        laid(&scope(
            "",
            &(ends.to_string() + sat + "  u1.l - " + wire + ".p1\n  " + wire + ".p2 - u2.r\n"),
        ))
    };
    let narrow = span("  |R#r1| \"1k\" { rotate: 90 }\n", "r1");
    let (a, b) = (tip(&narrow, "l", true), tip(&narrow, "r", false));
    let (x, _, w, _) = body(&narrow, "r1");
    assert!(
        x - w / 2.0 - a >= LABEL_SEAT - 1e-6 && b - (x + w / 2.0) >= LABEL_SEAT - 1e-6,
        "a seat gap clear of each pin: [{a}, {b}] holds {x} ± {}",
        w / 2.0
    );
    // The same chain with the part laid along the line asks for more room,
    // and gets exactly that much more.
    let wide = span("  |R#r1| \"1k\"\n", "r1");
    let room = |n: &[PlacedNode]| tip(n, "r", false) - tip(n, "l", true);
    assert!(
        room(&wide) > room(&narrow),
        "the wider part parts the tracks further: {} vs {}",
        room(&wide),
        room(&narrow)
    );
    let (x, _, w, _) = body(&wide, "r1");
    let (a, b) = (tip(&wide, "l", true), tip(&wide, "r", false));
    assert!(
        x - w / 2.0 - a >= LABEL_SEAT - 1e-6 && b - (x + w / 2.0) >= LABEL_SEAT - 1e-6,
        "still a seat gap clear either side: [{a}, {b}] holds {x} ± {}",
        w / 2.0
    );
}

#[test]
fn a_satellite_with_no_placed_end_flows_and_says_so() {
    // [SPEC 16.1/21] two capacitors wired only to each other have nothing to
    // seat against — the flow fallback, and one warning per part.
    let src = scope(
        "",
        &(anchor("u1", "") + "  |C#c7| \"1n\"\n  |C#c8| \"2n\"\n  c7.p2 - c8.p1\n"),
    );
    let warnings = seat_warnings(&src);
    assert!(
        warnings
            .iter()
            .any(|m| m == "'c7' has no placed end — its chain falls back to the flow"),
        "{warnings:?}"
    );
    assert!(warnings.iter().any(|m| m.contains("'c8'")), "{warnings:?}");
    // …and they really do flow: one trailing row under the anchor.
    let nodes = laid(&src);
    let ((_, uy), (x7, y7), (x8, y8)) = (at(&nodes, "u1"), at(&nodes, "c7"), at(&nodes, "c8"));
    assert!(y7 > uy && close(y7, y8), "one row below the grid");
    assert!(x7 < x8, "in declaration order");
    // A seated satellite is never reported.
    let seated = scope("", &(sided("u1") + "  |gnd#g1|\n  u1.c - g1\n"));
    assert!(
        seat_warnings(&seated).is_empty(),
        "{:?}",
        seat_warnings(&seated)
    );

    // A `pin:` overlay is sheet chrome seated on the **finished** box, so it
    // holds nothing: a chain wired only to one flows — and must say so. The
    // seat pass and the warning read the one placed-end filter, so a sheet can
    // never both flow a part and stay quiet about it.
    let overlay = scope(
        "",
        &(anchor("u1", "") + "  |box#note| \"note\" { pin: top right }\n  |gnd#g1|\n  note - g1\n"),
    );
    assert_eq!(
        seat_warnings(&overlay),
        ["'g1' has no placed end — its chain falls back to the flow"],
        "an overlay holds no chain"
    );
}

#[test]
fn a_third_placed_end_is_dropped_and_named() {
    // [SPEC 16.1/21] the distribution runs between **two** pins, so a chain
    // held at three loses the third — silently, until now. The sheet names the
    // end it dropped, at that part's own span. (Distribution stays two-ended;
    // a real three-way meet is Phase 5's junction work.)
    let src = scope(
        "",
        &(sided("u1")
            + &sided("u2")
            + &sided("u3")
            + "  |gnd#g1|\n  u1.b - g1\n  g1 - u2.a\n  g1 - u3.a\n"),
    );
    assert_eq!(
        seat_warnings(&src),
        [
            "'u3.a' also holds 'g1' — a chain distributes between two placed ends, so this one is dropped"
        ]
    );
    // Two ends is the lawful shape and says nothing.
    let two = scope(
        "",
        &(sided("u1") + &sided("u2") + "  |gnd#g1|\n  u1.b - g1\n  g1 - u2.a\n"),
    );
    assert!(seat_warnings(&two).is_empty(), "{:?}", seat_warnings(&two));
}

#[test]
fn an_overlay_end_holds_nothing_for_the_pose_chooser_either() {
    // [SPEC 16.1] the pose chooser and the seat pass read the **one** placed-end
    // filter, so a `pin:` overlay on a chain's far end changes neither.
    //
    // Reaching a pin *and* an overlay is a one-end chain: counting the raw ends
    // made it look spanning, and the ground seated at the pin unturned.
    let held = scope(
        "",
        &(sided("u1")
            + "  |gnd#g1|\n  |box#note| \"note\" { pin: top right }\n  u1.a - g1\n  g1 - note\n"),
    );
    assert_eq!(
        pose_of(&laid(&held), "g1"),
        0,
        "the left pin still holds it, upright"
    );
    assert!(
        seat_warnings(&held).is_empty(),
        "{:?}",
        seat_warnings(&held)
    );
    let ((ux, _), (gx, _)) = (at(&laid(&held), "u1"), at(&laid(&held), "g1"));
    assert!(gx < ux, "and it grew out of that pin: {gx} vs {ux}");

    // The mirror: reaching **only** an overlay is no placed end at all — the
    // chain flows, so nothing may have been turned to face the overlay.
    let adrift_src = scope(
        "",
        &(anchor("u1", "") + "  |gnd#g1|\n  |gnd#note| { pin: top right }\n  g1 - note\n"),
    );
    assert_eq!(pose_of(&laid(&adrift_src), "g1"), 0, "no pin, no pose");
    assert_eq!(
        seat_warnings(&adrift_src),
        ["'g1' has no placed end — its chain falls back to the flow"]
    );
}

#[test]
fn a_cluster_widens_its_anchors_track() {
    // [SPEC 16.1] track sizing reads the anchor's **cluster** — itself plus
    // its seated satellites — so a ground hanging off a bottom pin pushes the
    // row below down, without ever taking a cell.
    let bare = laid(&scope(
        " { columns: 1 }",
        &(sided("u1") + &anchor("u2", "")),
    ));
    let clustered = laid(&scope(
        " { columns: 1 }",
        &(sided("u1") + &anchor("u2", "") + "  |gnd#g1|\n  u1.c - g1\n"),
    ));
    let span = |n: &[PlacedNode]| at(n, "u2").1 - at(n, "u1").1;
    assert!(
        span(&clustered) > span(&bare),
        "the cluster grew row 1: {} vs {}",
        span(&clustered),
        span(&bare)
    );
    // The satellite consumed space, not a cell: the grid is still one column.
    assert!(close(at(&clustered, "u1").0, at(&clustered, "u2").0));
    // And row 1 is exactly as tall as the cluster, so the gap below it is the
    // scope's own — measured from the ground, the row's real bottom.
    assert!(
        close(y_gap(&clustered, "g1", "u2"), SCH_GAP),
        "one gap under the cluster: {}",
        y_gap(&clustered, "g1", "u2")
    );
}

#[test]
fn a_seat_rides_its_anchor() {
    // Pin-relative [SPEC 16.1]: move the component and its satellites — and
    // any `translate:` nudge on them — travel along, unchanged. The ground
    // hangs off a **left** pin, so its seat is a turned one: a broken role
    // would seat it below instead, and the offset would not survive.
    let body = |u1: &str, u2: &str| {
        sided_with("u1", u1) + "  |gnd#g1| { translate: 3 5 }\n  u1.a - g1\n" + &anchor("u2", u2)
    };
    let offset = |n: &[PlacedNode]| {
        let ((ux, uy), (gx, gy)) = (at(n, "u1"), at(n, "g1"));
        (gx - ux, gy - uy)
    };
    let plain = laid(&scope("", &body("", "")));
    assert_eq!(pose_of(&plain, "g1"), 0, "a ground hangs upright");
    let base = offset(&plain);
    assert!(base.0 < 0.0, "off the left pin it hangs from: {base:?}");

    // The anchor's neighbour grows, pushing u1 along its row.
    let pushed = laid(&scope("", &body("", " { width: 300 }")));
    assert!(at(&pushed, "u1").0 != at(&plain, "u1").0, "u1 really moved");
    let moved = offset(&pushed);
    assert!(
        close(base.0, moved.0) && close(base.1, moved.1),
        "the seat is the same offset either way: {base:?} {moved:?}"
    );

    // …and the **anchor's own** `translate:` carries its satellites with it
    // [SPEC 16.1] — a nudge that left them behind would break the wire.
    let nudged = laid(&scope("", &body(" { translate: 40 -25 }", "")));
    let rode = offset(&nudged);
    assert!(
        close(base.0, rode.0) && close(base.1, rode.1),
        "the ground rode the nudge: {base:?} {rode:?}"
    );
    let ((px, py), (nx, ny)) = (at(&plain, "u1"), at(&nudged, "u1"));
    assert!(
        close(nx, px + 40.0) && close(ny, py - 25.0),
        "and u1 itself took it exactly once: {px},{py} → {nx},{ny}"
    );
    // A nudge never grows the scope [SPEC 5]: the sheet is the same size.
    let extent = |n: &[PlacedNode]| placed(n, "s").0.bbox.w();
    assert!(
        close(extent(&plain), extent(&nudged)),
        "the nudge did not resize the sheet: {} vs {}",
        extent(&plain),
        extent(&nudged)
    );
}

#[test]
fn two_ends_on_one_anchor_grow_off_it_instead_of_spanning_it() {
    // The regression carried into Phase 5: `u1.a & u1.b - r1` is a fan onto one
    // part, not a span between two. Distributing it struck the midpoint of a
    // line running down u1's own side, so the resistor seated **inside** the
    // component and every wire strayed ("fixed port blocked" — the port's
    // landing was under a body). It grows off the first pin instead
    // ([`holder`]), and the router fans the rest onto the shared landing.
    let src = crate::layout::schematic::tests::scope(
        "",
        &(anchor("u1", " { cell: 1 1 }") + "  |R#r1|\n  u1.a & u1.b - r1\n"),
    );
    let laid =
        crate::layout::layout(&crate::layout::schematic::tests::program(&src)).expect("layout");
    let (u1, ux, _) = placed(&laid.nodes, "u1");
    let (r1, rx, _) = placed(&laid.nodes, "r1");
    assert!(
        rx + r1.bbox.max_x < ux + u1.bbox.min_x,
        "clear of the anchor it hangs off: r1 at {rx}, u1 at {ux}"
    );
    assert_eq!(laid.links.len(), 2, "both wires exist");
    assert!(
        laid.strays.is_empty(),
        "and both draw: {:?}",
        laid.link_report
    );
    assert!(seat_warnings(&src).is_empty(), "{:?}", seat_warnings(&src));
    // …and the meet the fan makes is dotted [SPEC 16.5].
    assert_eq!(laid.junctions.len(), 1);
    // A third end on the same anchor is no more a dropped end than the second.
    let three = crate::layout::schematic::tests::scope(
        "",
        &(anchor("u1", " { cell: 1 1 }") + "  |R#r1|\n  u1.a & u1.b & u1.c - r1\n"),
    );
    assert!(
        seat_warnings(&three).is_empty(),
        "{:?}",
        seat_warnings(&three)
    );
}

#[test]
fn a_chain_clears_the_anchors_readouts() {
    // [SPEC 16.1]: the leg runs as far as it needs to stand clear of the part
    // it hangs from — the part's **ink**, ref/value readouts included, not its
    // box (a long value text used to sit under the chain).
    let nodes = laid(&scope(
        "",
        "  |R#r1| \"10.0kOhm-1%-0603-THICKFILM\" { cell: 1 1 }\n  |gnd#g1|\n  r1.p2 - g1\n",
    ));
    let (rx, ry, rw, rh) = cell(&nodes, "r1");
    let (gx, gy, gw, gh) = cell(&nodes, "g1");
    let clear_x = (gx - rx).abs() * 2.0 >= gw + rw - 0.01;
    let clear_y = (gy - ry).abs() * 2.0 >= gh + rh - 0.01;
    assert!(
        clear_x || clear_y,
        "the chain stands clear of the readouts: r=({rx},{ry},{rw},{rh}) g=({gx},{gy},{gw},{gh})"
    );
}

#[test]
fn a_chain_grows_monotone_past_its_own_earlier_members() {
    // [SPEC 16.1] "grows from there, link by link" is an order, not just a
    // distance: a later member may never tuck into a hole before an earlier
    // one. The trigger is a neighbouring chain on the next pin of the same
    // side, whose band pushes the first member deep and opens exactly such a
    // hole — the chain's own terminator then seated *inside* its chain,
    // and 380 px of wire looped back up for a 30 px drop.
    let nodes = laid(&scope(
        "",
        &("  |component#j1| [\n    |pin#p1| { side: left }; |pin#p2| { side: left }; |pin#p3| { side: right }\n  ]\n"
            .to_owned()
            + "  |R#r2| \"4k7\"\n  |LED#d2| \"red\"\n  j1.p1 - r2 - d2 - |gnd|\n  j1.p2 - |gnd|\n"),
    ));
    let (ry, dy) = (at(&nodes, "r2").1, at(&nodes, "d2").1);
    let gy = at(&nodes, "lini-cap-1").1;
    assert!(
        ry < dy && dy < gy,
        "r2 → d2 → gnd stay in growth order: {ry} {dy} {gy}"
    );
}

#[test]
fn same_pin_chains_on_one_ray_ladder_side_by_side() {
    // [SPEC 16.1] two chains off one pin heading the same way take adjacent
    // lanes — the reset cap and the reset button both drop off NRST. Sharing
    // one lane while the ladder steps one past the other's reach is a
    // feedback loop that walked the pair hundreds of px out (775 px on the
    // hero's MCU block); adjacent lanes are also how a real sheet draws it.
    let nodes = laid(&scope(
        "",
        &("  |component#u3| [\n    |pin#nrst| { side: right }; |pin#gnd| { side: bottom }\n  ]\n"
            .to_owned()
            + "  |C#c7| \"100n\"\n  |SW#sw1| \"RST\"\n  u3.nrst - c7 - |gnd|\n  u3.nrst - sw1 - |gnd|\n"),
    ));
    let ((cx, _), (sx, _)) = (at(&nodes, "c7"), at(&nodes, "sw1"));
    let wall = {
        let (u, ux, _) = placed(&nodes, "u3");
        ux - u.cx + super::seat::drawn(u).max_x
    };
    assert!(
        cx < sx,
        "statement order takes the inner lane first: {cx} vs {sx}"
    );
    // Both lanes stay near the part — a bound loose enough for any lane
    // arithmetic, tight enough that the runaway (hundreds of px) fails it.
    assert!(
        sx - wall < 4.0 * LABEL_SEAT,
        "the outer lane stays within a few seats of the part: {} out",
        sx - wall
    );
}

#[test]
fn a_same_side_bridge_stands_in_its_first_pins_corridor() {
    // [SPEC 16.1] two placed ends on one anchor: the member grows like a
    // one-end chain off the first-named pin — in that pin's own corridor,
    // entry terminal end-on — and the far wire is the router's, which merges
    // it into the second pin's net at a junction, the way a sheet taps a
    // pull-up into the line it feeds. Grown along the side instead, the
    // member (always taller than one pin pitch) straddled the second pin's
    // row and its return orbited the part.
    let nodes = laid(&scope(
        "",
        &("  |component#u2| { cell: 1 1 } [\n    |pin#vin| { side: left }; |pin#en| { side: left }; |pin#out| { side: right }\n  ]\n"
            .to_owned()
            + "  |R#r5| \"100k\"\n  u2.en - r5 - u2.vin\n"),
    ));
    assert_eq!(pose_of(&nodes, "r5"), 180, "horizontal, p1 end-on at EN");
    let ((ux, _), (rx, ry)) = (at(&nodes, "u2"), at(&nodes, "r5"));
    assert!(rx < ux, "out along the shared left side: {rx} vs {ux}");
    let en_y = cell(&nodes, "en").1;
    assert!(close(ry, en_y), "riding EN's own row: {ry} vs {en_y}");
}

#[test]
fn a_spanning_member_rides_the_landing_leg_between_the_clusters() {
    // [SPEC 16.1] a span's member sits on the second end's row — the wire's
    // landing leg — on the stretch clear of both ends' clusters, never on
    // the raw pin-to-pin diagonal.
    let nodes = laid(&scope(
        "",
        &("  |component#u1| { cell: 1 1 } [\n    |pin#hi| { side: right }; |pin#lo| { side: right }; |pin#x| { side: left }\n  ]\n"
            .to_owned()
            + "  |component#u2| { cell: 2 1 } [\n    |pin#in| { side: left }; |pin#nc| { side: right }; |pin#n2| { side: right }\n  ]\n"
            // The gnd under u1.lo skews u1's cluster, so u2's aligned row
            // sits off u1's centre and the raw chord would run diagonal.
            + "  u1.lo - |gnd|\n  |F#f1| \"2A\"\n  u1.hi - f1 - u2.in\n"),
    ));
    let (fx, fy) = at(&nodes, "f1");
    let in_y = cell(&nodes, "in").1;
    assert!(
        close(fy, in_y),
        "the member rides the landing row: {fy} vs {in_y}"
    );
    let (u1r, u2l) = (ink(&nodes, "u1").max_x, ink(&nodes, "u2").min_x);
    assert!(
        u1r < fx && fx < u2l,
        "…between the two clusters: {u1r} {fx} {u2l}"
    );
}

#[test]
fn a_spanning_member_stands_off_the_landing_not_adrift_in_the_leg() {
    // [SPEC 16.1] a span's members seat **off the second end's landing**, a
    // seat gap at a time: the leg runs along the very axis that anchor's own
    // satellites ladder their lanes on, so the members are that ladder's next
    // columns and read on one rhythm with them. Split evenly over the clear
    // stretch instead, a member drifts into the middle of a length nobody
    // authored — the hero's 24 V bus runs clear over the connector's whole
    // cluster, so its fuse hugged the connector and left a hundred px of
    // blank between itself and the first part hanging off the bus, which a
    // reader takes for a column of nothing.
    //
    // The `gap:` is what makes the leg longer than the member needs; the
    // resistor column off `u2.d` is what gives the landing a ladder to
    // continue, and puts the cluster edge well inside the part.
    let nodes = laid(&scope(
        " { gap: 260 }",
        &("  |component#u1| { cell: 1 1 } [ |pin#o| { side: right } ]\n".to_owned()
            + "  |component#u2| { cell: 2 1 } [\n    |pin#i| { side: left }; |pin#d| { side: left }\n  ]\n"
            + "  |F#f1| \"2A\"\n  |R#ra| \"1k\"\n  |gnd#g1|\n"
            + "  u1.o - f1 - u2.i\n  u2.d - ra - g1\n"),
    ));
    let landing = ["u2", "ra", "g1"]
        .iter()
        .map(|id| ink(&nodes, id).min_x)
        .fold(f64::INFINITY, f64::min);
    assert!(
        close(ink(&nodes, "f1").max_x, landing - LABEL_SEAT),
        "one seat gap clear of the landing's cluster: {} vs {}",
        ink(&nodes, "f1").max_x,
        landing - LABEL_SEAT
    );
    // …and the leg's slack is the bare bus coming in, not a hole in the
    // ladder: several seats' worth of it, all on the connector's side.
    let lead = ink(&nodes, "f1").min_x - tip(&nodes, "o", true);
    assert!(
        lead > 3.0 * LABEL_SEAT,
        "the surplus lies where the wire comes in: {lead}"
    );

    // A vertical leg is the same law turned: the second end's pin faces
    // **top**, so the leg is its column and the member packs upward off it.
    let down = laid(&scope(
        " { gap: 260 }",
        &("  |component#u1| { cell: 1 1 } [ |pin#o| { side: bottom } ]\n".to_owned()
            + "  |component#u2| { cell: 1 2 } [ |pin#i| { side: top } ]\n"
            + "  |R#r1| \"1k\"\n  u1.o - r1 - u2.i\n"),
    ));
    assert!(
        close(ink(&down, "r1").max_y, ink(&down, "u2").min_y - LABEL_SEAT),
        "a seat gap above the landing: {} vs {}",
        ink(&down, "r1").max_y,
        ink(&down, "u2").min_y - LABEL_SEAT
    );
    assert!(
        y_gap(&down, "u1", "r1") > 3.0 * LABEL_SEAT,
        "the slack hangs off the pin the wire leaves: {}",
        y_gap(&down, "u1", "r1")
    );
}

#[test]
fn a_spanning_member_takes_the_next_column_of_the_ladder_it_lands_on() {
    // [SPEC 16.1] a span's members *are* the next columns of the ladder its
    // landing belongs to, so they stand on that ladder's own **pitch** — the
    // fuse on a bus in the column after the last part hanging off it, not a
    // gap of its own reckoning beside them. Packed off the cluster alone the
    // member's step is whatever its own ink and the outermost column's
    // readout happen to add up to, which is a third rhythm in a picture that
    // already reads as a grid.
    //
    // The wide value sits on the **deeper** pin, so its column is the inner
    // one and it sets the pitch while leaving the outermost column's own ink
    // narrow — the one arrangement where packing off the cluster and
    // continuing the ladder part company.
    let nodes = laid(&scope(
        " { gap: 260 }",
        &("  |component#u1| { cell: 1 1 } [ |pin#o| { side: right } ]\n".to_owned()
            + "  |component#u2| { cell: 2 1 } [\n    |pin#i| { side: left }; |pin#p| { side: left }; |pin#q| { side: left }\n  ]\n"
            + "  |F#f1| \"2A\"\n  |R#rn| \"1k\"\n  |R#rw| \"1000000000k\"\n"
            + "  u2.p - rn - |gnd|\n  u2.q - rw - |gnd|\n  u1.o - f1 - u2.i\n"),
    ));
    let lane = |id| at(&nodes, id).0;
    let pitch = lane("rn") - lane("rw");
    assert!(
        close(lane("f1") - lane("rn"), pitch),
        "the member takes the next column: {} vs {pitch}",
        lane("f1") - lane("rn")
    );
    // The tracks parted for that column, not for the packing it replaced:
    // the leg still runs clear from the far pin, all its slack at that end.
    assert!(
        ink(&nodes, "f1").min_x - tip(&nodes, "o", true) > LABEL_SEAT,
        "the leg holds the column it was reserved for: {}",
        ink(&nodes, "f1").min_x - tip(&nodes, "o", true)
    );
}

#[test]
fn a_rail_flag_taps_the_trunk_instead_of_standing_in_it() {
    // [SPEC 16.1] a symbol-label leaf hanging mid-chain is a tap: it hangs
    // off its attachment member along its own convention — and steps aside
    // when that points back into the trunk, as the buck's 5 V flag does at
    // the inductor's far pin, **upright**, risen a gap so its lead turns one
    // square corner. Stacked in trunk order it stood upside-down between
    // the inductor and the divider; turned to face its aside ray it lay
    // sideways, a flag no sheet draws.
    let sheet = "{\n  |v5::label| { symbol: power } [ \"5V\" ]\n  |sch::group| { layout: schematic; gap: 100 }\n}\n";
    let nodes = laid(
        &(sheet.to_owned()
            + "|sch#s| [\n"
            + &sided("u1")
            + "  |L#l1| \"100u\"\n  |v5#f|\n  |R#r4| \"4k7\"\n"
            + "  u1.b - l1 - f\n  l1.p2 - r4 - |gnd|\n]\n"),
    );
    let (ly, ry) = (at(&nodes, "l1").1, at(&nodes, "r4").1);
    assert!(ly < ry, "the trunk keeps growing: l1 above r4: {ly} {ry}");
    let (fx, fy) = at(&nodes, "f");
    let lx = at(&nodes, "l1").0;
    assert!(
        fx > lx,
        "the flag steps aside, outward of the trunk: {fx} vs {lx}"
    );
    assert_eq!(pose_of(&nodes, "f"), 0, "the flag stays upright");
    assert!(
        fy < ry && fy > ly - 60.0,
        "…risen beside its junction, not stacked past the divider: {ly} {fy} {ry}"
    );
}

#[test]
fn a_multi_member_side_branch_grows_its_own_column() {
    // [SPEC 16.1] a subtree hanging off a mid-trunk member is a **branch**:
    // it grows from its attachment junction as a sub-chain along its own
    // ray, its lane stepped beside the trunk when the rays share an axis —
    // not tucked into the trunk's stack in walk order, where the two
    // series interleaved into one smeared column.
    let nodes = laid(&scope(
        "",
        &(sided("u1")
            + "  |R#r1| \"10k\"\n  |R#r2| \"20k\"\n  |C#c1| \"100n\"\n"
            + "  u1.b - r1 - r2 - |gnd|\n  r1.p2 - c1 - |gnd|\n"),
    ));
    let ((r1x, r1y), (r2x, r2y), (c1x, c1y)) =
        (at(&nodes, "r1"), at(&nodes, "r2"), at(&nodes, "c1"));
    assert!(
        close(c1x, r1x),
        "the trunk keeps one column: {c1x} vs {r1x}"
    );
    assert!(
        !close(r2x, r1x),
        "the branch takes a lane of its own: {r2x} vs {r1x}"
    );
    assert!(
        r2y > r1y && c1y > r1y,
        "both descend from the junction: {r1y} {r2y} {c1y}"
    );
}

#[test]
fn a_two_by_two_divider_takes_two_columns() {
    // [SPEC 16.1] each pin's up-and-down pair shares one column (one lead,
    // splitting once); two pins make two columns, ordered canonically. The
    // per-ray depth orders point opposite ways, so ordering columns along
    // each ray was a contradiction the ladder could never satisfy — every
    // chain ended in one smeared column.
    let sheet = "{\n  |vp::label| { symbol: power } [ \"V+\" ]\n  |sch::group| { layout: schematic; gap: 100 }\n}\n";
    let nodes = laid(
        &(sheet.to_owned()
            + "|sch#s| [\n  |component#u4| { cell: 1 1 } [\n    |pin#inp| { side: left }; |pin#inn| { side: left }; |pin#out| { side: right }\n  ]\n"
            + "  |R#r9| \"100k\"\n  |R#r10| \"10k\"\n  |R#r11| \"2k2\"\n  |R#r12| \"1k\"\n"
            + "  u4.inp - r9 - |vp|\n  u4.inp - r10 - |gnd|\n  u4.inn - r11 - |vp|\n  u4.inn - r12 - |gnd|\n]\n"),
    );
    let x = |id: &str| at(&nodes, id).0;
    assert!(
        close(x("r9"), x("r10")),
        "inp's pair shares a column: {} vs {}",
        x("r9"),
        x("r10")
    );
    assert!(
        close(x("r11"), x("r12")),
        "inn's pair shares a column: {} vs {}",
        x("r11"),
        x("r12")
    );
    assert!(
        (x("r9") - x("r11")).abs() > 20.0,
        "two pins, two columns: {} vs {}",
        x("r9"),
        x("r11")
    );
}

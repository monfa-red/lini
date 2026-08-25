//! The satellite seat pass [SPEC 16.1]: a chain grows outward from its pin,
//! auto-posed to face it; chains on one pin stack; two placed ends distribute;
//! no placed end flows with a warning; and every seat rides its anchor.

use super::tests::{
    anchor, at, body, cell, close, ink, laid, placed, pose_of, scope, seat_warnings, sided,
    sided_with, tip, y_gap,
};
use crate::layout::PlacedNode;
use crate::ledger::consts::LABEL_SEAT;
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
    // Neither was pushed along the ray, so each turns once and lands square.
    assert!(
        close(gay - pay, gby - pby),
        "each keeps its own pin's depth: {} vs {}",
        gay - pay,
        gby - pby
    );
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
            ga < gb,
            "the upper pin's ground stays above the lower pin's, declared {order:?}: {ga} {gb}"
        );
    }
}

#[test]
fn two_placed_ends_distribute_at_even_fractions() {
    // [SPEC 16.1] the satellites of a chain held at both ends space evenly
    // along the straight line between the two pins — the wire lands on the
    // **stub tips**, so those are the ends the fractions divide.
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
    let third = (b - a) / 3.0;
    assert!(
        close(x1, a + third) && close(x2, a + 2.0 * third),
        "two satellites take the thirds: {x1} {x2} in [{a}, {b}]"
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

//! The field pass [SPEC 16.1]: a chain takes a ray, a lane and a slot, its
//! members stand on coarse cells, a span rides its landing leg, and a chain no
//! anchor holds flows with a warning.
//!
//! The lane allocator and the walk are unit-tested inside
//! [`super::field`], against the cells themselves; this suite judges the
//! **placed sheet** — where a part actually landed.

use super::tests::{
    anchor, at, cell, close, laid, placed, pose_of, scope, seat_warnings, sided, sided_with, tip,
};
use crate::layout::PlacedNode;
use crate::ledger::consts::PIN_PITCH;
use crate::ledger::defaults::SCH_GAP;

/// A user-defined power flag — the terminator for the chains that grow **up**.
const FLAG: &str = "{ |vp::label| { symbol: power } [ \"V+\" ] }\n";

// ───────────────────────── the ray ─────────────────────────

#[test]
fn a_chain_grows_the_way_its_terminator_faces() {
    // [SPEC 16.1] the ray is the **terminator's** own connection geometry: a
    // `|gnd|` is drawn with its point at the top, so a chain ending in one
    // grows **down** — off a bottom pin straight down its own line, and off a
    // side pin out onto a lane and then down.
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
            "pin {pin} hangs its chain off its own lane: {gx} vs {px}"
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
    // [SPEC 16.1] an explicit `rotate:` forces the pose; the ray then derives
    // from the **rotated** connection point — so a ground held upright on a
    // left-facing pin hangs below it, not beside it.
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
    // Each link takes the next slot along the same ray, in wire order: the
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

// ───────────────────────── lanes and slots ─────────────────────────

#[test]
fn three_chains_off_one_pin_take_three_columns_one_coarse_pitch_apart() {
    // [SPEC 16.1] the pitch is the lattice's, never the parts' ink: three
    // values of wildly different widths still stand on one rhythm.
    let src = scope(
        "",
        &(sided("u1")
            + "  |C#c1| \"1n\"\n  |C#c2| \"100000pF\"\n  |C#c3| \"1u\"\n"
            + "  |gnd#g1|\n  |gnd#g2|\n  |gnd#g3|\n"
            + "  u1.a - c1 - g1\n  u1.a - c2 - g2\n  u1.a - c3 - g3\n"),
    );
    let nodes = laid(&src);
    let [x1, x2, x3] = ["c1", "c2", "c3"].map(|id| at(&nodes, id).0);
    assert!(
        close((x1 - x2).abs(), SCH_GAP),
        "one coarse pitch: {x1} {x2}"
    );
    assert!(
        close((x2 - x3).abs(), SCH_GAP),
        "and the same one: {x2} {x3}"
    );
}

#[test]
fn members_of_different_chains_share_a_slot_row() {
    // The reference sheet's row: a cap and a resistor hanging off one bus
    // have their bodies on one line, whatever their own lengths.
    let src = scope(
        "",
        &(sided("u1")
            + "  |C#c1| \"1u\"\n  |R#r1| \"10k\"\n  |gnd#g1|\n  |gnd#g2|\n"
            + "  u1.a - c1 - g1\n  u1.a - r1 - g2\n"),
    );
    let nodes = laid(&src);
    assert!(
        close(at(&nodes, "c1").1, at(&nodes, "r1").1),
        "one slot row"
    );
}

#[test]
fn a_second_member_stands_one_coarse_pitch_deeper() {
    let src = scope(
        "",
        &(sided("u1") + "  |R#r1| \"1k\"\n  |LED#d1| \"red\"\n  |gnd#g1|\n  u1.a - r1 - d1 - g1\n"),
    );
    let nodes = laid(&src);
    let (r, d) = (at(&nodes, "r1").1, at(&nodes, "d1").1);
    assert!(close(d - r, SCH_GAP), "slot 1 then slot 2: {r} {d}");
}

#[test]
fn an_up_chain_and_a_down_chain_off_one_pin_share_a_column() {
    // [SPEC 16.1] their cells are disjoint, so the second one's innermost
    // candidate is free — no rule, a consequence.
    let src = FLAG.replace("vp", "v3")
        + &scope(
            "",
            &(sided("u1")
                + "  |C#c1| \"1u\"\n  |gnd#g1|\n  |R#r1| \"10k\"\n  |v3#f1|\n"
                + "  u1.a - c1 - g1\n  u1.a - r1 - f1\n"),
        );
    let nodes = laid(&src);
    assert!(
        close(at(&nodes, "c1").0, at(&nodes, "r1").0),
        "one lane, two rays"
    );
    assert!(
        at(&nodes, "r1").1 < at(&nodes, "u1").1,
        "the flag chain climbs"
    );
    assert!(
        at(&nodes, "c1").1 > at(&nodes, "u1").1,
        "the ground chain drops"
    );
}

#[test]
fn every_placed_part_lands_on_the_scopes_own_fine_lattice() {
    // The invariant [SPEC 16.1], and it is **absolute**: the packer lands
    // every anchor on a coarse line, a cell is a whole number of coarse
    // pitches off its anchor, a coarse pitch is a whole number of fine ones,
    // and the sheet's own centring shift is a whole number of fine ones too.
    // So the sheet's lattice, not just each anchor's, holds every part.
    let src = scope(
        "",
        &(sided("u1")
            + &sided("u2")
            + "  |R#r1| \"1k\"\n  |C#c1| \"1u\"\n  |gnd#g1|\n  |gnd#g2|\n  |F#f1| \"2A\"\n"
            + "  u1.a - r1 - g1\n  u1.c - c1 - g2\n  u1.b - f1 - u2.a\n"),
    );
    let nodes = laid(&src);
    for id in ["u1", "u2", "r1", "c1", "g1", "g2", "f1"] {
        let (x, y) = at(&nodes, id);
        assert!(
            on_fine_grid(x) && on_fine_grid(y),
            "'{id}' off the grid: {x} {y}"
        );
    }
}

/// Whether a coordinate lands on a fine lattice point [SPEC 16.1].
fn on_fine_grid(v: f64) -> bool {
    let r = (v / PIN_PITCH).round() * PIN_PITCH;
    (v - r).abs() < 1e-6
}

#[test]
fn chains_on_one_pin_stand_one_coarse_pitch_apart_in_statement_order() {
    // [SPEC 16.1] a pin's straight corridor belongs to its **first claimant**:
    // the second chain's cells meet the first's, so it steps one coarse cell
    // beside them rather than landing on top of them.
    let nodes = laid(&scope(
        "",
        &(sided("u1") + "  |gnd#g1|\n  |gnd#g2|\n  u1.c - g1\n  u1.c - g2\n"),
    ));
    let ((x1, y1), (x2, y2)) = (at(&nodes, "g1"), at(&nodes, "g2"));
    assert!(close(y1, y2), "one slot row: {y1} {y2}");
    assert!(
        close(x2 - x1, SCH_GAP),
        "one coarse pitch, in statement order: {x1} {x2}"
    );
    // The **parts'** order decides, like everything else the engine places —
    // writing the wires the other way round changes nothing.
    let rewired = laid(&scope(
        "",
        &(sided("u1") + "  |gnd#g1|\n  |gnd#g2|\n  u1.c - g2\n  u1.c - g1\n"),
    ));
    assert!(at(&rewired, "g2").0 > at(&rewired, "g1").0);
    // Declaring them the other way round does.
    let redeclared = laid(&scope(
        "",
        &(sided("u1") + "  |gnd#g2|\n  |gnd#g1|\n  u1.c - g1\n  u1.c - g2\n"),
    ));
    assert!(at(&redeclared, "g1").0 > at(&redeclared, "g2").0);
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
    let (tax, tbx) = (tip(&nodes, "a", false), tip(&nodes, "b", false));
    assert!(close(tax, tbx), "both stubs tip on one rail: {tax} {tbx}");
    let ((gax, gay), (gbx, gby)) = (at(&nodes, "ga"), at(&nodes, "gb"));
    assert!(!close(gax, gbx), "a lane each: {gax} {gbx}");
    assert!(
        close((gax - gbx).abs(), SCH_GAP),
        "one coarse pitch apart: {gax} {gbx}"
    );
    // Both take slot 1 of the same field, so they land on one row — the way a
    // sheet drops two returns to one rail height.
    assert!(close(gay, gby), "…landing level: {gay} {gby}");
}

#[test]
fn chains_turning_onto_one_ray_from_opposite_sides_keep_their_own_lanes() {
    // [SPEC 16.1] one field per **side**: a chain turning left off a left pin
    // and one turning right off a right pin stand on opposite sides of the
    // part and can never cross, so neither is stepped past the other's reach.
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
fn a_side_growing_one_way_ladders_along_that_ray_not_the_canonical_one() {
    // [SPEC 16.1] a chain's lead crosses every lane inside its own, so the
    // chain whose pin sits **shallower along the ray** has to step out. Read
    // canonically (down, right) that is right for a side whose chains grow
    // down and backwards for one whose chains grow up. The canonical reading
    // is owed only to a side holding **both** rays, where one pin's up- and
    // down-chain share a column and the two orders have to agree.
    let part = "  |component#u1| [\n    |pin#hi| { side: right }; |pin#lo| { side: right }; |pin#l| { side: left }\n  ]\n";
    let lanes = |chains: &str| {
        let nodes = laid(&(FLAG.replace("vp", "flag") + &scope("", &(part.to_owned() + chains))));
        let wall = {
            let (u, ux, _) = placed(&nodes, "u1");
            ux - u.cx + super::field::drawn(u).max_x
        };
        (at(&nodes, "a").0 - wall, at(&nodes, "b").0 - wall)
    };
    // Both chains grow **up**: the upper pin is the deeper one along that
    // ray, so it keeps the inner lane and the lower pin's column steps out.
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

#[test]
fn chains_on_different_pins_seat_in_the_order_their_pins_do() {
    // [SPEC 16.1] no chain overtakes another: a chain's lead crosses every
    // lane inside its own, so the one off the **shallower** pin takes the
    // outer lane. The order is the pins' own, so the sheet reads the same
    // however the parts are written.
    let two_left = "  |component#u1| [\n    |pin#a| { side: left }; |pin#b| { side: left }; |pin#c| { side: right }\n  ]\n";
    for order in ["  |gnd#ga|\n  |gnd#gb|\n", "  |gnd#gb|\n  |gnd#ga|\n"] {
        let nodes = laid(&scope(
            "",
            &(two_left.to_owned() + order + "  u1.a - ga\n  u1.b - gb\n"),
        ));
        let (ay, by) = (cell(&nodes, "a").1, cell(&nodes, "b").1);
        assert!(ay < by, "pin 'a' is the upper one: {ay} {by}");
        let (lane_a, lane_b) = (at(&nodes, "ga").0, at(&nodes, "gb").0);
        assert!(
            lane_a < lane_b,
            "the lower pin keeps the inner lane, declared {order:?}: {lane_a} {lane_b}"
        );
    }
}

#[test]
fn same_pin_chains_on_one_ray_take_successive_lanes() {
    // [SPEC 16.1] two chains off one pin heading the same way take adjacent
    // lanes — the reset cap and the reset button both drop off NRST, one
    // coarse cell apart, which is also how a real sheet draws it.
    let nodes = laid(&scope(
        "",
        &("  |component#u3| [\n    |pin#nrst| { side: right }; |pin#gnd| { side: bottom }; |pin#in| { side: left }\n  ]\n"
            .to_owned()
            + "  |C#c7| \"100n\"\n  |SW#sw1| \"RST\"\n  u3.nrst - c7 - |gnd|\n  u3.nrst - sw1 - |gnd|\n"),
    ));
    let ((cx, _), (sx, _)) = (at(&nodes, "c7"), at(&nodes, "sw1"));
    assert!(
        cx < sx,
        "statement order takes the inner lane first: {cx} vs {sx}"
    );
    assert!(
        close(sx - cx, SCH_GAP),
        "successive lanes, one coarse pitch: {cx} vs {sx}"
    );
}

#[test]
fn a_chain_clears_the_anchors_readouts() {
    // [SPEC 16.1] the field origin is the first coarse line clear of the
    // anchor's own drawn ink — its ref/value readouts included, not its box
    // (a long value text used to sit under the chain).
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
    // [SPEC 16.1] member *k* takes the *k*-th slot, so a later member can never
    // tuck in behind an earlier one — the chain's own terminator once seated
    // *inside* its chain, and 380 px of wire looped back up for a 30 px drop.
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

// ───────────────────────── taps and branches ─────────────────────────

#[test]
fn a_rail_flag_taps_the_trunk_instead_of_standing_in_it() {
    // [SPEC 16.1] a symbol-label leaf hanging mid-chain is a tap: it takes no
    // slot, standing on its attachment's row one coarse cell across, the way
    // a sheet stands a flag beside the junction it taps. Stacked in trunk
    // order it stood upside-down between the inductor and the divider.
    let sheet =
        "{\n  |v5::label| { symbol: power } [ \"5V\" ]\n  |sch::group| { layout: schematic }\n}\n";
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
        fy < ry,
        "…beside its junction, not stacked past the divider: {ly} {fy} {ry}"
    );
}

#[test]
fn a_multi_member_side_branch_grows_its_own_column() {
    // [SPEC 16.1] a subtree hanging off a mid-trunk member is a **branch**: it
    // grows from its attachment junction as a sub-chain along its own ray, on
    // a lane of its own where the rays share an axis — not tucked into the
    // trunk's slots in walk order, where the two series smeared into one
    // column.
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
    // [SPEC 16.1] each pin's up-and-down pair shares one column (their cells
    // are disjoint); two pins make two columns, ordered canonically. The
    // per-ray depth orders point opposite ways, so ordering columns along each
    // ray was a contradiction no ladder could satisfy — every chain ended in
    // one smeared column.
    let sheet =
        "{\n  |vp::label| { symbol: power } [ \"V+\" ]\n  |sch::group| { layout: schematic }\n}\n";
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
        close((x("r9") - x("r11")).abs(), SCH_GAP),
        "two pins, two columns one coarse pitch apart: {} vs {}",
        x("r9"),
        x("r11")
    );
}

// ───────────────────────── spans and bridges ─────────────────────────

#[test]
fn a_span_rides_the_landing_leg_on_coarse_cells() {
    // [SPEC 16.1] the fuse between a connector and a switch stands on the
    // line joining the two pins, one coarse cell per member.
    let src = scope(
        "",
        &(sided_with("u1", "") + &sided_with("u2", "") + "  |F#f1| \"2A\"\n  u1.b - f1 - u2.a\n"),
    );
    let nodes = laid(&src);
    let (fx, fy) = at(&nodes, "f1");
    let pin = at(&nodes, "b").1;
    assert!(close(fy, pin), "on the landing leg: {fy} vs {pin}");
    assert!(
        at(&nodes, "u1").0 < fx && fx < at(&nodes, "u2").0,
        "between them"
    );
}

#[test]
fn two_span_members_stand_one_coarse_pitch_apart() {
    let src = scope(
        "",
        &(sided_with("u1", "")
            + &sided_with("u2", "")
            + "  |F#f1| \"2A\"\n  |R#r1| \"1k\"\n  u1.b - f1 - r1 - u2.a\n"),
    );
    let nodes = laid(&src);
    assert!(close(
        (at(&nodes, "r1").0 - at(&nodes, "f1").0).abs(),
        SCH_GAP
    ));
}

#[test]
fn a_bridge_grows_off_its_first_named_pin_like_any_chain() {
    // [SPEC 16.1] both ends on one anchor is a fan, not a span: the pull-up
    // stands in the first pin's own corridor and the router merges the rest.
    let src = scope(
        "",
        &(sided_with("u1", "") + "  |R#r1| \"100k\"\n  u1.a - r1 - u1.b\n"),
    );
    let nodes = laid(&src);
    let (rx, _) = at(&nodes, "r1");
    assert!(
        rx < at(&nodes, "u1").0,
        "off the left pin it was named at first"
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
    let src = scope(
        "",
        &(anchor("u1", " { cell: 1 1 }") + "  |R#r1|\n  u1.a & u1.b - r1\n"),
    );
    let laid = crate::layout::layout(&super::tests::program(&src)).expect("layout");
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
    let three = scope(
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

// ───────────────────────── the seat itself ─────────────────────────

#[test]
fn a_seat_rides_its_anchor() {
    // A cell is an offset in its anchor's own frame [SPEC 16.1]: move the
    // component and its satellites — and any `translate:` nudge on them —
    // travel along, unchanged. The ground hangs off a **left** pin, so its
    // cell is a turned chain's: a broken role would seat it below instead, and
    // the offset would not survive.
    let source = |u1: &str, u2: &str| {
        sided_with("u1", u1) + "  |gnd#g1| { translate: 3 5 }\n  u1.a - g1\n" + &anchor("u2", u2)
    };
    let offset = |n: &[PlacedNode]| {
        let ((ux, uy), (gx, gy)) = (at(n, "u1"), at(n, "g1"));
        (gx - ux, gy - uy)
    };
    let plain = laid(&scope("", &source("", "")));
    assert_eq!(pose_of(&plain, "g1"), 0, "a ground hangs upright");
    let base = offset(&plain);
    assert!(base.0 < 0.0, "off the left pin it hangs from: {base:?}");

    // The anchor's neighbour grows, pushing u1 along its row.
    let pushed = laid(&scope("", &source("", " { width: 300 }")));
    assert!(at(&pushed, "u1").0 != at(&plain, "u1").0, "u1 really moved");
    let moved = offset(&pushed);
    assert!(
        close(base.0, moved.0) && close(base.1, moved.1),
        "the cell is the same offset either way: {base:?} {moved:?}"
    );

    // …and the **anchor's own** `translate:` carries its satellites with it
    // [SPEC 16.1] — a nudge that left them behind would break the wire.
    let nudged = laid(&scope("", &source(" { translate: 40 -25 }", "")));
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

// ───────────────────────── diagnostics ─────────────────────────

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
    // field pass and the warning read the one placed-end filter, so a sheet can
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
    // [SPEC 16.1/21] a span runs between **two** pins, so a chain held at
    // three loses the third — silently, until now. The sheet names the end it
    // dropped, at that part's own span.
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
    // [SPEC 16.1] the pose chooser and the field pass read the **one**
    // placed-end filter, so a `pin:` overlay on a chain's far end changes
    // neither.
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
fn a_pins_own_pair_shares_one_lane() {
    // [SPEC 16.1] one pin's up-chain and down-chain leave on one lead and
    // split once — the rail up to its flag, down to its decoupling cap, one
    // column. A bare symbol, so a node's centre **is** its connection point.
    let sheet = "{ |flag::label| { symbol: power } }\n";
    let part = "  |component#u1| [\n    |pin#d| { side: right }; |pin#s| { side: right }; |pin#l| { side: left }\n  ]\n";
    let nodes = laid(
        &(sheet.to_owned()
            + &scope(
                "",
                &(part.to_owned() + "  |flag#f|\n  |gnd#g|\n  u1.d - f\n  u1.d - g\n"),
            )),
    );
    let (fx, gx) = (at(&nodes, "f").0, at(&nodes, "g").0);
    assert!(close(fx, gx), "one pin, one column: {fx} vs {gx}");
}

#[test]
fn two_pins_share_a_lane_only_where_their_columns_never_meet() {
    // [SPEC 16.1] a chain's cells run from its own **pin** to its outermost
    // member, so two pins of one side whose rays point *at* each other claim
    // the band between them twice and take a lane each — without that reading
    // their member cells are disjoint and two nets braid into one column.
    // Pointing *away* they never meet, and one column holds both: the rail
    // climbing off the upper pin, the return dropping off the lower, which is
    // the compact idiom a sheet draws.
    let sheet = "{ |v3::label| { symbol: power } [ \"3V3\" ] }\n";
    let part = "  |component#u1| [\n    |pin#a| { side: left }; |pin#b| { side: left }; |pin#z| { side: right }\n  ]\n";
    let parts = "  |C#c1| \"1u\"\n  |gnd#g1|\n  |R#r1| \"10k\"\n  |v3#f1|\n";
    let lanes_of = |wires: &str| {
        let nodes = laid(&(sheet.to_owned() + &scope("", &(part.to_owned() + parts + wires))));
        (at(&nodes, "c1").0, at(&nodes, "r1").0)
    };
    // `a` is the upper pin and grows **down**; `b` the lower, growing **up**.
    let (cx, rx) = lanes_of("  u1.a - c1 - g1\n  u1.b - r1 - f1\n");
    assert!(
        !close(cx, rx),
        "pointing at each other, a lane each: {cx} {rx}"
    );
    // The mirror: `a` up and `b` down, so neither column enters the other's
    // band and one lane holds both.
    let (cx, rx) = lanes_of("  u1.a - r1 - f1\n  u1.b - c1 - g1\n");
    assert!(close(cx, rx), "pointing away, one lane: {cx} {rx}");
}

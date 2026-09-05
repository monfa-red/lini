//! The field pass [SPEC 16.1]: a chain takes a ray, a lane and a slot, its
//! members stand on coarse cells, a span rides its landing leg, and a chain no
//! anchor holds flows with a warning.
//!
//! The lane allocator and the walk are unit-tested inside
//! [`super::field`], against the cells themselves; this suite judges the
//! **placed sheet** — where a part actually landed.

use super::tests::{
    anchor, at, body, cell, chrome, close, ink, laid, landing, on_fine_grid, placed, port, pose_of,
    scope, seat, seat_warnings, sided, sided_with, tip,
};
use crate::layout::PlacedNode;
use crate::ledger::consts::{PIN_PITCH, READOUT_OFFSET};
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
fn an_authored_rotate_on_a_mid_chain_part_states_the_ray() {
    // [SPEC 16.1] a forced pose states the ray for its whole chain — the
    // turned pin faces back up it. Off a left pin, a resistor stood with its
    // `p1` at the bottom (270) grows its chain **up**, the run at the end
    // posed on end above it; `p1` on top (90) grows it down.
    for (deg, dir) in [(270, -1.0), (90, 1.0)] {
        let nodes = laid(&scope(
            "",
            &(sided("u1")
                + &format!(
                    "  |R#r1| \"1k\" {{ rotate: {deg} }}\n  |label#pwm| \"PWM\"\n  u1.a - r1 - pwm\n"
                )),
        ));
        let ((_, py), (rx, ry)) = (at(&nodes, "u1"), at(&nodes, "r1"));
        assert_eq!(pose_of(&nodes, "r1"), deg, "the authored pose stands");
        assert!(
            (ry - py) * dir > 0.0,
            "rotate {deg}: the chain grows the way the turned pin faces back from: {ry} vs {py}"
        );
        let (run, runx, runy) = placed(&nodes, "pwm");
        assert!(
            run.type_chain.iter().any(|t| t == "net-run-turned"),
            "the run at its end is posed along the same ray"
        );
        assert!((runy - ry) * dir > 0.0, "past the resistor: {runy} vs {ry}");
        assert!(close(runx, rx), "on the resistor's line: {runx} vs {rx}");
    }
}

#[test]
fn a_forced_up_chain_shares_its_pins_lane_with_the_down_chain() {
    // The gate network: the series resistor forced on end grows up off the
    // pin, the pull-down's ground chain grows down, and their cells are
    // disjoint — so both ride one lane, the column a sheet draws.
    let nodes = laid(&scope(
        "",
        &(sided("u1")
            + "  |R#r1| \"100R\" { rotate: 270 }\n  |R#r2| \"100k\"\n  |gnd#g1|\n"
            + "  |label#pwm| \"PWM\"\n  u1.a - r1 - pwm\n  u1.a - r2 - g1\n"),
    ));
    let (_, py, ..) = cell(&nodes, "a");
    let ((r1x, r1y), (r2x, r2y)) = (at(&nodes, "r1"), at(&nodes, "r2"));
    assert!(
        r1y < py && py < r2y,
        "one above the pin, one below: {r1y} {py} {r2y}"
    );
    assert!(close(r1x, r2x), "one lane, two rays: {r1x} vs {r2x}");
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
fn a_flag_rides_the_outermost_lane_its_pin_feeds() {
    // [SPEC 16.1] a lone power flag off a pin carrying two cap chains stands
    // over the far cap, not on an inner lane its small cell happens to fit,
    // so the rail closes over the whole bank. Declared first, it would take
    // lane one by declaration order alone.
    let src = FLAG.replace("vp", "v3")
        + &scope(
            "",
            &(sided("u1")
                + "  |v3#f1|\n  |C#c1| \"1u\"\n  |C#c2| \"1u\"\n"
                + "  u1.a - f1\n  u1.a - c1 - |gnd|\n  u1.a - c2 - |gnd|\n"),
        );
    let nodes = laid(&src);
    let (f, c1, c2) = (seat(&nodes, "f1"), at(&nodes, "c1"), at(&nodes, "c2"));
    assert!(
        c2.0 < c1.0,
        "c2 is the far lane on a left side: {c1:?} {c2:?}"
    );
    assert!(close(f.0, c2.0), "the flag stands over it: {f:?} vs {c2:?}");
}

#[test]
fn a_satellite_seats_by_its_connection_geometry_not_its_drawn_box() {
    // [SPEC 16.1] a power flag draws its name **beside** its symbol, so its
    // box centre stands half a name off its own connection point. The lattice
    // holds the connection point: the flag's lead runs dead straight up the
    // lane its chain took and the name hangs off it, which is what a sheet
    // draws. Ink deciding where a wire goes is the one thing 16.1 forbids
    // outright.
    let src = FLAG.to_owned()
        + &scope(
            "",
            &(sided("u1") + "  |R#r1| \"10k\"\n  |vp#f1|\n  u1.a - r1 - f1\n"),
        );
    let nodes = laid(&src);
    // The resistor's own leads ride its centre line [SPEC 16.3], so its centre
    // **is** the lane the whole chain stands on.
    let lane = at(&nodes, "r1").0;
    let (fx, _) = port(&nodes, "f1");
    assert!(
        close(fx, lane),
        "the flag lands off the lane: {fx} vs {lane}"
    );
    assert!(
        !close(at(&nodes, "f1").0, lane),
        "…and its box does not stand on it, the name hanging off"
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
        let (x, y) = seat(&nodes, id);
        assert!(
            on_fine_grid(x) && on_fine_grid(y),
            "'{id}' off the grid: {x} {y}"
        );
    }
}

#[test]
fn chains_on_one_pin_step_beside_each_other_in_statement_order() {
    // [SPEC 16.1] a pin's straight corridor belongs to its **first claimant**:
    // the second chain's cells meet the first's, so it steps beside them
    // rather than landing on top of them — by what the two cells need, which
    // for two bare grounds is a fine pitch and not a column.
    let nodes = laid(&scope(
        "",
        &(sided("u1") + "  |gnd#g1|\n  |gnd#g2|\n  u1.c - g1\n  u1.c - g2\n"),
    ));
    let ((x1, y1), (x2, y2)) = (at(&nodes, "g1"), at(&nodes, "g2"));
    assert!(close(y1, y2), "one slot row: {y1} {y2}");
    assert!(
        x2 > x1 && close(x2 - x1, PIN_PITCH),
        "a fine pitch beside it, in statement order: {x1} {x2}"
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
fn a_symbol_grown_out_of_one_pin_leaves_the_rows_either_side_free() {
    // [SPEC 16.1] a member's cell across its ray is the pitch of the **line it
    // stands on** — the fine pin pitch for a chain that grew straight out —
    // widened only to what the part's own symbol needs. So the `|nc|` seated
    // off one pin covers that pin's row and no other, and the net runs on the
    // pins above and below it keep their own rows instead of each being shoved
    // a whole coarse cell aside.
    // One name on every pin, so all four stubs tip on exactly one line and the
    // lane order is the statement order it ties to.
    let pin = |id: &str| format!("    |pin#{id}| \"IO\" {{ side: left }}\n");
    let rail = format!(
        "  |component#u1| [\n{}{}{}{}  ]\n",
        pin("a"),
        pin("b"),
        pin("c"),
        pin("d")
    );
    let nodes = laid(&scope(
        "",
        &(rail
            + "  |nc#x1|\n  |label#n1| \"NA\"\n  |label#n2| \"NC\"\n  |label#n3| \"ND\"\n"
            + "  u1.b - x1\n  u1.a - n1\n  u1.c - n2\n  u1.d - n3\n"),
    ));
    for (pin, run) in [("a", "n1"), ("c", "n2"), ("d", "n3")] {
        let (py, ry) = (at(&nodes, pin).1, at(&nodes, run).1);
        assert!(
            close(py, ry),
            "'{run}' sits off '{pin}''s own row: {ry} vs {py}"
        );
    }
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
    // Their cells hold a ground symbol apiece, so the lanes stand a fine pitch
    // apart and not a whole column: a bare wire to a rail is not a part.
    assert!(
        close((gax - gbx).abs(), PIN_PITCH),
        "a fine pitch apart: {gax} {gbx}"
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
    // are disjoint), and the pair is allotted together — so two pins make two
    // columns, whichever lead has to cross the other's column. Both pins'
    // columns are live toward each other, so the crossing count ties and the
    // canonical direction (down) puts the lower pin's pair inside. Ranking
    // each chain on its own ray split every pair over three columns instead.
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

#[test]
fn a_mixed_side_puts_the_least_crossed_pins_column_inside() {
    // [SPEC 16.1] the RS-485 transceiver's right side: VCC (top) climbs to a
    // rail and drops to a decoupling cap, A (middle) climbs through a
    // pull-up, B (bottom) drops through a pull-down. B's column is live only
    // below B, so no lead crosses it: it takes the inner lane, and A's rail —
    // live only above A, crossed by nothing once B is placed — shares that
    // lane with it. VCC's pair is crossed whichever way and steps outside,
    // where its lead meets A's column on the rail's bare lead, never on the
    // resistor.
    let sheet = "{ |v3::label| { symbol: power } [ \"3V3\" ] }\n";
    let part = "  |component#u5| [\n    |pin#vcc| { side: right }; |pin#a| { side: right }; |pin#b| { side: right }; |pin#l| { side: left }\n  ]\n";
    let nodes = laid(
        &(sheet.to_owned()
            + &scope(
                "",
                &(part.to_owned()
                    + "  |C#c8| \"100n\"\n  |R#r12| \"680R\"\n  |R#r13| \"680R\"\n"
                    + "  u5.vcc - |v3|\n  u5.vcc - c8 - |gnd|\n  u5.a - r12 - |v3|\n  u5.b - r13 - |gnd|\n"),
            )),
    );
    let x = |id: &str| at(&nodes, id).0;
    assert!(
        close(x("r12"), x("r13")),
        "A's rail and B's return share the inner lane: {} vs {}",
        x("r12"),
        x("r13")
    );
    assert!(
        x("c8") > x("r12"),
        "VCC's cap steps outside: {} vs {}",
        x("c8"),
        x("r12")
    );
    // …and the rail's body clears VCC's row by the pitch a lead needs
    // [SPEC 16.1]: the slot clears the deepest wired pin of the side.
    let vcc = landing(&nodes, "u5", "vcc").1;
    let r12 = ink(&nodes, "r12");
    assert!(
        r12.max_y <= vcc - PIN_PITCH,
        "the pull-up ends a pitch above VCC's row: {} vs {vcc}",
        r12.max_y
    );
}

#[test]
fn a_slot_clears_the_deepest_wired_pin_of_its_side() {
    // [SPEC 16.1] a flag climbing off the lower of two left pins passes the
    // upper pin's row, and that row carries a wire to another part: the flag
    // stands clear of it, so the wire runs straight through under the flag's
    // column rather than ending on the flag's own port.
    let src = format!(
        "{}{}",
        FLAG,
        scope(
            "",
            &("  |component#u1| [\n    |pin#s| { side: left }; |pin#d| { side: left }; |pin#z| { side: right }\n  ]\n"
                .to_owned()
                + "  |component#j1| { cell: 2 1 } [ |pin#p3| { side: left } ]\n"
                + "  |vp#f1|\n  j1.p3 - u1.s\n  u1.d - f1\n"),
        )
    );
    let nodes = laid(&src);
    let s_row = landing(&nodes, "u1", "s").1;
    let flag = ink(&nodes, "f1");
    assert!(
        flag.max_y <= s_row - PIN_PITCH,
        "the flag stands a pitch clear of the wired row above its pin: {} vs {s_row}",
        flag.max_y
    );
}

#[test]
fn a_part_led_straight_chain_starts_past_the_lane_sharing_its_pin() {
    // [SPEC 16.1] the fan driver's gate: a series resistor runs straight out
    // and a pull-down drops off the same pin. The pull-down's lane sits
    // between the body and the resistor, so the junction lies on the gate's
    // own trace ahead of the resistor — and the resistor's cell clears it.
    let src = scope(
        "",
        &("  |component#u1| [\n    |pin#g| { side: left }; |pin#d| { side: right }; |pin#s| { side: bottom }\n  ]\n"
            .to_owned()
            + "  |R#r14| \"100R\"\n  |R#r15| \"100k\"\n  |label#pwm| \"PWM\"\n  u1.g - r14 - pwm\n  u1.g - r15 - |gnd|\n"),
    );
    let nodes = laid(&src);
    let (g, r14, r15) = (at(&nodes, "u1"), ink(&nodes, "r14"), at(&nodes, "r15"));
    assert!(
        r15.0 > r14.max_x,
        "the pull-down hangs between the body and the resistor: {} vs {}",
        r15.0,
        r14.max_x
    );
    assert!(
        r15.0 < g.0,
        "…and off the gate's side: {} vs {}",
        r15.0,
        g.0
    );
    assert!(
        r14.max_x <= r15.0 - SCH_GAP / 2.0,
        "the resistor's cell clears the lane: {} vs {}",
        r14.max_x,
        r15.0
    );
}

#[test]
fn a_bare_run_keeps_its_place_and_the_lane_steps_past_it() {
    // [SPEC 16.1] a chain led by a **declared** bare net run is the trace
    // itself: the sense pin's name lies flat beside the body and its shunt's
    // lane steps past the run to tap it at the far end, exactly as before a
    // part-led chain learned to yield. (The text form on a wired pin is
    // absorbed as the wire's name instead — SPEC 16.4, tested in desugar.)
    let src = scope(
        "",
        &("  |component#u8| [\n    |pin#bra| { side: right }; |pin#l| { side: left }; |pin#z| { side: bottom }\n  ]\n"
            .to_owned()
            + "  |R#r20| \"470m\"\n  |label#rs| \"RS_A\"\n  u8.bra - rs\n  u8.bra - r20 - |gnd|\n"),
    );
    let nodes = laid(&src);
    let run = nodes
        .iter()
        .flat_map(|n| n.children.iter())
        .find(|c| c.type_chain.iter().any(|t| t == "net-run"))
        .map(|c| ink(&nodes, c.id.as_deref().unwrap()))
        .expect("the run");
    let r20 = at(&nodes, "r20");
    assert!(
        run.min_x < r20.0 && run.max_x <= r20.0,
        "the run sits inside the lane: {run:?} vs {}",
        r20.0
    );
}

// ───────────────────────── spans and bridges ─────────────────────────

/// Two anchors a track row apart, the second's left rail three pins deep so
/// its named pin sits a pitch off the first's — the alignment has something to
/// strike.
fn offset_pair(wire: &str) -> Vec<PlacedNode> {
    laid(&scope(
        "",
        &("  |component#u1| { cell: 1 1 } [ |pin#a| { side: right } ]\n".to_owned()
            + "  |component#u2| { cell: 2 1 } [\n    |pin#p1| { side: left }\n    \
               |pin#p2| { side: left }\n    |pin#p3| { side: left }\n  ]\n"
            + wire),
    ))
}

#[test]
fn a_span_aligns_the_facing_pins_it_joins() {
    // [SPEC 16.1] "a wire — **or a span**, whose members all ride one line —
    // joining a facing pin pair aligns that pair". Both of the fuse's hops are
    // anchor-to-satellite, so only the span itself names the pair; unaligned,
    // `u2` keeps the track's own line and its pin stands a pitch off the bus.
    let nodes = offset_pair("  |F#f1| \"2A\"\n  u1.a - f1 - u2.p3\n");
    let (a, p3) = (landing(&nodes, "u1", "a").1, landing(&nodes, "u2", "p3").1);
    let f = at(&nodes, "f1").1;
    assert!(close(a, p3), "the facing pair shares a row: {a} vs {p3}");
    assert!(
        close(f, a),
        "and the span rides it dead straight: {f} vs {a}"
    );
    assert!(on_fine_grid(a), "a whole number of fine pitches: {a}");
    // …and a bare wire between the same two pins still aligns them, so the
    // span reads through the one rule rather than beside it.
    let bare = offset_pair("  u1.a - u2.p3\n");
    assert!(close(
        landing(&bare, "u1", "a").1,
        landing(&bare, "u2", "p3").1
    ));
}

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

/// A part carrying two left pins and one right, with `wire` beneath it — the
/// pull-up bridge's scope, and the two-sided control for it.
fn bridged(wire: &str) -> String {
    scope(
        "",
        &("  |component#u2| { cell: 1 1 } [\n    |pin#vin| { side: left }\n    \
           |pin#en| { side: left }\n    |pin#out| { side: right }\n  ]\n"
            .to_owned()
            + "  |R#r5| \"100k\"\n"
            + wire),
    )
}

#[test]
fn a_corridor_members_readouts_step_off_the_row_its_neighbour_needs() {
    // [SPEC 16.2] a member lying along its own pin's row straddles that row
    // with its ref and its value — and a readout line stands further off a
    // body than the one **fine** pitch the next pin's row sits at, so the pair
    // draws over a live row. Crowded on one side only, both lines step whole
    // to the free side; the row is then a row again, which is what lets a
    // same-side **bridge**'s return step onto it [SPEC 16.1].
    let nodes = laid(&bridged("  u2.en - r5 - u2.vin\n"));
    let (en, vin) = (landing(&nodes, "u2", "en"), landing(&nodes, "u2", "vin"));
    let (p1, p2) = (landing(&nodes, "r5", "p1"), landing(&nodes, "r5", "p2"));
    assert!(close(p1.1, en.1) && close(p2.1, en.1), "along EN's own row");
    let drawn = ink(&nodes, "r5");
    assert!(
        drawn.min_y > vin.1 && drawn.max_y > en.1,
        "the pair stepped below, leaving VIN's row clear: {drawn:?} vs {vin:?}"
    );
    // A middle pin is the control: with a row either side of it there is
    // nothing to choose, and the pair straddles as desugar minted it.
    let both = laid(&scope(
        "",
        &("  |component#u1| { cell: 1 1 } [\n    |pin#p1| { side: left }\n    \
           |pin#p2| { side: left }\n    |pin#p3| { side: left }\n  ]\n"
            .to_owned()
            + "  |R#r1| \"1k\"\n  u1.p2 - r1\n"),
    ));
    let straddles = ink(&both, "r1");
    let axis = landing(&both, "u1", "p2").1;
    assert!(
        straddles.min_y < axis && straddles.max_y > axis,
        "a corridor with no crowded side keeps the mint: {straddles:?}"
    );
}

#[test]
fn a_bridge_grows_off_its_first_named_pin_whichever_sides_its_pins_take() {
    // [SPEC 16.1] both ends on one anchor is a fan, not a span: the member
    // stands in the first-named pin's own corridor, and the far wire is the
    // router's — merged into the second pin's net at a junction dot, the way a
    // sheet taps a pull-up into the line it feeds. One side or two reads the
    // same; only the readouts differ ([SPEC 16.2], above).
    for wire in ["  u2.en - r5 - u2.vin\n", "  u2.en - r5 - u2.out\n"] {
        let nodes = laid(&bridged(wire));
        let en = landing(&nodes, "u2", "en");
        let (p1, p2) = (landing(&nodes, "r5", "p1"), landing(&nodes, "r5", "p2"));
        assert!(close(p1.1, p2.1), "along the row: {p1:?} {p2:?}");
        assert!(close(p1.1, en.1), "EN's own: {} vs {}", p1.1, en.1);
        assert!(
            p1.0 < at(&nodes, "u2").0,
            "out along the pin it was named at"
        );
    }
}

// ───────────────────────── the rails ─────────────────────────

#[test]
fn a_ground_ends_its_own_chain_and_equal_chains_share_the_line() {
    // [SPEC 16.1] there is no ground row: a ground stands under the member it
    // terminates. Two chains of one depth therefore land on one line — which
    // is the ground line a sheet draws when it draws one — and a deeper chain
    // ends deeper, which is what both reference sheets draw.
    let src = scope(
        "",
        &(sided("u1")
            + "  |C#c1| \"1u\"\n  |R#r1| \"1k\"\n  |LED#d1| \"red\"\n"
            + "  |gnd#g1|\n  |gnd#g2|\n"
            + "  u1.a - c1 - g1\n  u1.a - r1 - d1 - g2\n"),
    );
    let nodes = laid(&src);
    assert!(
        at(&nodes, "g2").1 > at(&nodes, "g1").1,
        "the deeper chain ends deeper: {} vs {}",
        at(&nodes, "g2").1,
        at(&nodes, "g1").1
    );
    assert!(
        at(&nodes, "g2").1 > at(&nodes, "d1").1,
        "each below the member it terminates"
    );

    // Two chains of one depth, and their grounds share the line.
    let src = scope(
        "",
        &(sided("u1")
            + "  |C#c1| \"1u\"\n  |R#r1| \"1k\"\n  |gnd#g1|\n  |gnd#g2|\n"
            + "  u1.a - c1 - g1\n  u1.b - r1 - g2\n"),
    );
    let nodes = laid(&src);
    assert!(
        close(at(&nodes, "g1").1, at(&nodes, "g2").1),
        "one line for two equal chains"
    );
}

#[test]
fn a_power_flag_keeps_its_own_slot_and_no_row_of_its_own() {
    // [SPEC 16.1] there is no flag row. One ground net earns one line and
    // reads as that net; three flags naming three nets earn none, which is
    // what every reference sheet draws. So a flag on a longer chain stands
    // deeper than one on a shorter — and two chains of one length still land
    // on one line, their slot origin being the track row's.
    let src = FLAG.replace("vp", "v3")
        + &scope(
            "",
            &(sided("u1")
                + "  |R#r1| \"1k\"\n  |R#r2| \"2k\"\n  |L#l1| \"1u\"\n  |v3#f1|\n  |v3#f2|\n"
                + "  u1.a - r1 - f1\n  u1.a - r2 - l1 - f2\n"),
        );
    let nodes = laid(&src);
    assert!(
        at(&nodes, "f2").1 < at(&nodes, "f1").1,
        "the longer chain's flag stands higher: {} vs {}",
        at(&nodes, "f2").1,
        at(&nodes, "f1").1
    );

    let src = FLAG.replace("vp", "v3")
        + &scope(
            "",
            &(sided("u1")
                + "  |R#r1| \"1k\"\n  |R#r2| \"2k\"\n  |v3#f1|\n  |v3#f2|\n"
                + "  u1.a - r1 - f1\n  u1.b - r2 - f2\n"),
        );
    let nodes = laid(&src);
    assert!(
        close(at(&nodes, "f1").1, at(&nodes, "f2").1),
        "two chains of one length share the row's own origin"
    );
}

#[test]
fn a_horizontal_chain_keeps_its_own_end() {
    // [SPEC 16.1] rails are vertical only — a chain running out along a pin's
    // row ends where it ends, as both reference sheets draw it.
    let src = scope(
        "",
        &(sided("u1")
            + "  |R#r1| \"1k\"\n  |gnd#g1| { rotate: 90 }\n  |C#c1| \"1u\"\n  |gnd#g2|\n"
            + "  u1.a - r1 - g1\n  u1.c - c1 - g2\n"),
    );
    let nodes = laid(&src);
    assert!(
        !close(at(&nodes, "g1").1, at(&nodes, "g2").1),
        "no rail across the axes"
    );
}

#[test]
fn a_tap_keeps_the_junction_it_taps() {
    // A rail symbol hanging off a mid-chain junction takes no slot [SPEC 16.1]
    // — it is the flag beside that junction, so the flag row is not its row.
    let src = FLAG.to_owned()
        + &scope(
            "",
            &(sided("u1")
                + "  |L#l1| \"100u\"\n  |vp#t1|\n  |R#r1| \"4k7\"\n  |vp#f1|\n"
                + "  u1.a - l1 - t1\n  l1.p2 - r1 - f1\n"),
        );
    let nodes = laid(&src);
    assert!(
        close(seat(&nodes, "t1").1, seat(&nodes, "l1").1),
        "the tap stays on its attachment's row"
    );
    assert!(
        seat(&nodes, "f1").1 < seat(&nodes, "l1").1,
        "and only the terminator rose"
    );
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

// ───────────────────────── the readout side ─────────────────────────

#[test]
fn a_left_field_part_wears_its_readouts_to_its_left() {
    // [SPEC 16.2] outward, away from the anchor: on the left flank the reading
    // side points back over the pin the part hangs from.
    let src = scope(
        "",
        &(sided("u1") + "  |C#c1| \"100n\"\n  |gnd#g1|\n  u1.a - c1 - g1\n"),
    );
    let nodes = laid(&src);
    let axis = body(&nodes, "c1").0;
    let (name, value) = (
        chrome(&nodes, "c1", "ref"),
        chrome(&nodes, "c1", "part-value"),
    );
    assert!(
        close(axis - value.max_x, READOUT_OFFSET) && close(axis - name.max_x, READOUT_OFFSET),
        "right aligned one offset off the axis: {name:?} {value:?} vs {axis}"
    );
    assert!(name.max_y <= value.min_y, "the ref over the value");
}

#[test]
fn a_right_field_part_wears_them_to_its_right() {
    let src = scope(
        "",
        &(sided("u1") + "  |C#c1| \"100n\"\n  |gnd#g1|\n  u1.b - c1 - g1\n"),
    );
    let nodes = laid(&src);
    let axis = body(&nodes, "c1").0;
    let (name, value) = (
        chrome(&nodes, "c1", "ref"),
        chrome(&nodes, "c1", "part-value"),
    );
    assert!(
        close(value.min_x - axis, READOUT_OFFSET) && close(name.min_x - axis, READOUT_OFFSET),
        "left aligned one offset off the axis: {name:?} {value:?} vs {axis}"
    );
}

#[test]
fn a_part_riding_a_row_wears_them_above_and_below() {
    // [SPEC 16.2] the third reading: a part lying along its pin's own row
    // stacks the pair over and under its drawing, centred — the field has no
    // side to give it and none is wanted.
    let src = scope(
        "",
        &(sided("u1") + "  |R#r1| \"1k\"\n  |label#n1| \"NET\"\n  u1.a - r1 - n1\n"),
    );
    let nodes = laid(&src);
    let (rx, ry, ..) = body(&nodes, "r1");
    let (name, value) = (
        chrome(&nodes, "r1", "ref"),
        chrome(&nodes, "r1", "part-value"),
    );
    assert!(
        name.max_y <= ry && value.min_y >= ry,
        "ref above, value below: {name:?} {value:?} vs {ry}"
    );
    assert!(
        close(name.center().0, rx) && close(value.center().0, rx),
        "centred on the part: {name:?} {value:?} vs {rx}"
    );
}

#[test]
fn a_readout_never_moves_a_part() {
    // [SPEC 16.1] ink never places: a long value overhangs its neighbour's
    // column rather than parting the columns.
    let columns = |value: &str| {
        let nodes = laid(&scope(
            "",
            &(sided("u1")
                + &format!("  |C#c1| \"1n\"\n  |C#c2| \"{value}\"\n  |gnd#g1|\n  |gnd#g2|\n")
                + "  u1.a - c1 - g1\n  u1.a - c2 - g2\n"),
        ));
        at(&nodes, "c1").0 - at(&nodes, "c2").0
    };
    assert!(
        close(columns("1n"), columns("4700000pF x7r 25V")),
        "the columns stand where the lattice put them, whatever the value reads"
    );
}

/// The schematic samples land on the fine lattice [SPEC 16.1] — the analogue
/// of the routing law checker, judged on the placed sheet alone. Measured in
/// each **scope's own frame**, which is where the invariant is stated; a
/// scope's own origin lands on the lattice too, so the scene's reading agrees.
/// A part is judged at the point the lattice holds it by [SPEC 16.1] — an
/// anchor's own origin, a satellite's connection geometry.
///
/// A three-terminal symbol's same-side pins stand a whole fine pitch either
/// side of its centre line [SPEC 16.3], so nothing a sample seats — a span
/// member riding the leg into a FET's source, a connector aligned to its
/// drain — has an excuse to leave the grid.
#[test]
fn every_sample_lands_on_the_lattice() {
    for path in [
        "samples/schematic.lini",
        "samples/schematic_hero.lini",
        "samples/schematic_blocks.lini",
        "samples/schematic_parts.lini",
    ] {
        let src = std::fs::read_to_string(path).expect("a sample");
        let laid = crate::testutil::laid_in_samples(&src);
        let parts = super::tests::scope_parts(&laid.nodes);
        for p in &parts {
            for (axis, v) in [("x", p.at.0), ("y", p.at.1)] {
                if on_fine_grid(v) {
                    continue;
                }
                panic!("{path}: '{}' is off the lattice in {axis} at {v}", p.id);
            }
        }
    }
}

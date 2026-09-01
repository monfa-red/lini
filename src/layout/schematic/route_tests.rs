//! Router integration [SPEC 16.5, ROUTING.md Fixed ports]: a placed sheet's
//! wires land **on** their terminals, a part is one obstacle, and a `:side`
//! on a terminal is an error.
//!
//! Landings are judged against an **independent** oracle — the drawn stub's
//! far end and the label symbol's own box, read straight off the placed tree
//! here — never against the reader that produced them.

use super::tests::{anchor, close, laid, placed, program, sided};
use crate::error::Code;
use crate::layout::PlacedNode;
use crate::layout::ir::LaidOut;
use crate::routing::ortho::scene::SceneIndex;

/// A root schematic sheet — the scene *is* the scope, so its wires route in
/// the root world with the canvas margin, as a real sheet does.
fn sheet(body: &str) -> String {
    format!("{{ layout: schematic }}\n{body}")
}

fn routed(src: &str) -> LaidOut {
    crate::layout::layout(&program(src)).expect("layout")
}

/// The drawn polyline of the wire between two endpoint paths.
fn wire<'a>(laid: &'a LaidOut, from: &str, to: &str) -> &'a [(f64, f64)] {
    laid.links
        .iter()
        .find(|w| w.seg_from == from && w.seg_to == to)
        .unwrap_or_else(|| {
            panic!(
                "no drawn wire {from} -> {to}; report: {:?}",
                laid.link_report
                    .iter()
                    .map(|v| v.detail.clone())
                    .collect::<Vec<_>>()
            )
        })
        .path
        .as_slice()
}

/// The oracle for a component pin: the far end of the **drawn stub**, read
/// off the placed chrome — a stub points away from its pin's body, so its tip
/// is the short edge's midpoint on that side.
fn stub_tip(nodes: &[PlacedNode], part: &str, pin: &str) -> (f64, f64) {
    let (owner, ox, oy) = placed(nodes, part);
    let (p, px, py) = placed(std::slice::from_ref(owner), pin);
    let (px, py) = (ox - owner.cx + px, oy - owner.cy + py);
    let stub = p
        .children
        .iter()
        .find(|c| c.type_chain.iter().any(|t| t == "pin-stub"))
        .expect("a pin draws a stub");
    // The wire meets the ink: the lead's true endpoint, not its paint bbox.
    let b = stub.bbox.inflate(-stub.attrs.half_stroke());
    let (sx, sy) = (px + stub.cx, py + stub.cy);
    let (dx, dy) = (sx - px, sy - py);
    if dx.abs() >= dy.abs() {
        let x = if dx < 0.0 { b.min_x } else { b.max_x };
        (sx + x, sy + (b.min_y + b.max_y) / 2.0)
    } else {
        let y = if dy < 0.0 { b.min_y } else { b.max_y };
        (sx + (b.min_x + b.max_x) / 2.0, sy + y)
    }
}

/// The oracle for a `|label|`: the midpoint of its drawn symbol's edge on the
/// side the wire arrives from.
fn tag_edge(nodes: &[PlacedNode], id: &str, side: &str) -> (f64, f64) {
    let (l, lx, ly) = placed(nodes, id);
    let sym = l
        .children
        .iter()
        .find(|c| c.type_chain.iter().any(|t| t == "sch-tag-line"))
        .expect("a symbol label draws its glyph");
    // The wire meets the ink: the glyph's drawn edge, not its paint bbox.
    let b = sym.bbox.inflate(-sym.attrs.half_stroke());
    let (cx, cy) = (lx + sym.cx, ly + sym.cy);
    match side {
        "left" => (cx + b.min_x, cy + (b.min_y + b.max_y) / 2.0),
        "right" => (cx + b.max_x, cy + (b.min_y + b.max_y) / 2.0),
        "top" => (cx + (b.min_x + b.max_x) / 2.0, cy + b.min_y),
        _ => (cx + (b.min_x + b.max_x) / 2.0, cy + b.max_y),
    }
}

fn near(a: (f64, f64), b: (f64, f64)) -> bool {
    close(a.0, b.0) && close(a.1, b.1)
}

// ───────────────────────── landings ─────────────────────────

#[test]
fn a_wire_between_two_pins_lands_on_both_stub_tips() {
    let src = sheet(&(anchor("u1", "") + &anchor("u2", "") + "u1.c - u2.a\n"));
    let laid = routed(&src);
    let path = wire(&laid, "u1.c", "u2.a");
    let (from, to) = (
        stub_tip(&laid.nodes, "u1", "c"),
        stub_tip(&laid.nodes, "u2", "a"),
    );
    assert!(
        near(path[0], from),
        "the wire leaves u1.c's stub tip {from:?}, drew {:?}",
        path[0]
    );
    assert!(
        near(path[path.len() - 1], to),
        "the wire lands on u2.a's stub tip {to:?}, drew {:?}",
        path[path.len() - 1]
    );
    // Perpendicular off the side it is pinned to (ROUTING.md Law 2).
    assert!(close(path[0].1, path[1].1), "the lead leaves along the pin");
    assert!(laid.strays.is_empty(), "a placed sheet routes whole");
}

#[test]
fn two_wires_on_one_pin_share_one_bit_exact_landing() {
    // Two statements landing on u1.c: ROUTING.md's implicit fan — one port,
    // one drawn lead until the split. Equality here is **exact**, not ±ε:
    // one connection-geometry computation feeds both wires.
    let src = sheet(&(anchor("u1", "") + &anchor("u2", "") + "u1.c - u2.a\nu1.c - u2.b\n"));
    let laid = routed(&src);
    let one = wire(&laid, "u1.c", "u2.a").to_vec();
    let two = wire(&laid, "u1.c", "u2.b").to_vec();
    assert_eq!(one[0], two[0], "one landing, bit for bit");
    assert!(
        near(one[0], stub_tip(&laid.nodes, "u1", "c")),
        "and it is the stub tip"
    );
    // One drawn lead until the split: both leave along the port's own
    // ordinate, so the shorter lead lies inside the longer one — overlapping
    // ink, not two rails. (That they may overlap at all is the fan; two
    // unmerged wires this close would breach Law 1, which
    // `a_seated_sheet_holds_the_four_laws` re-judges on the same pair.)
    assert!(close(one[1].1, one[0].1) && close(two[1].1, one[0].1));
    let (a, b) = (one[1].0 - one[0].0, two[1].0 - two[0].0);
    assert!(a * b > 0.0 && (a - b).abs() < a.abs().max(b.abs()));
}

#[test]
fn a_wire_to_a_label_lands_on_its_connection_point() {
    let src = sheet(&(anchor("u1", "") + "|gnd#g1|\nu1.a - g1\n"));
    let laid = routed(&src);
    let path = wire(&laid, "u1.a", "g1");
    // A ground is drawn with its point at the top, so its chain grows **down**
    // [SPEC 16.1]: off a left pin the lead leaves west, turns, and lands on the
    // tag's top edge.
    assert!(
        near(path[0], stub_tip(&laid.nodes, "u1", "a")),
        "off the stub tip"
    );
    assert!(
        near(path[path.len() - 1], tag_edge(&laid.nodes, "g1", "top")),
        "on the symbol's connection point, drew {:?}",
        path[path.len() - 1]
    );
    assert_eq!(path.len(), 3, "out along the pin, then down: {path:?}");
}

#[test]
fn a_rotated_parts_ports_land_on_the_sides_they_turned_onto() {
    // u2 turns a quarter: its left pins (a, b) land on its **top** edge, so
    // the wire into u2.a lands on a stub pointing up, not left.
    let src = sheet(&(anchor("u1", "") + &anchor("u2", " { rotate: 90 }") + "u1.c - u2.a\n"));
    let laid = routed(&src);
    let path = wire(&laid, "u1.c", "u2.a");
    let tip = stub_tip(&laid.nodes, "u2", "a");
    let (_, _, u2y) = placed(&laid.nodes, "u2");
    assert!(
        tip.1 < u2y,
        "the turned pin's stub points up: {tip:?} vs {u2y}"
    );
    let last = path[path.len() - 1];
    assert!(
        near(last, tip),
        "lands on the turned stub tip {tip:?}, drew {last:?}"
    );
    // …and arrives perpendicular to the side it turned onto.
    assert!(
        close(path[path.len() - 2].0, last.0),
        "vertical into a top-side port: {path:?}"
    );
}

// ───────────────────────── the obstacle identity ─────────────────────────

#[test]
fn a_pin_is_never_an_obstacle_of_its_own() {
    // The endpoint `u1.a` resolves to the **component's** connection frame:
    // the body with its pin chrome folded in, the stub tips on its side lines
    // [SPEC 16.2].
    let src = sheet(&(anchor("u1", "") + "|gnd#g1|\nu1.a - g1\n"));
    let laid = routed(&src);
    let index = SceneIndex::build(&laid.nodes);
    let part = index.rect("u1").expect("the component is placed");
    assert_eq!(
        index.rect("u1.a"),
        Some(part),
        "a pin addresses its component's rect"
    );
    let (u1, ux, _) = placed(&laid.nodes, "u1");
    let tip = stub_tip(&laid.nodes, "u1", "a");
    assert!(
        close(part.x0, tip.0),
        "the frame's left edge is the stub tip"
    );
    assert!(
        close(part.x1, stub_tip(&laid.nodes, "u1", "c").0),
        "and its right edge the right rail's"
    );
    assert!(
        part.x0 < ux + u1.bbox.min_x,
        "the stubs fold in — the frame reaches past the body"
    );
    // A label's frame edge is its own symbol's connection point.
    assert!(close(
        index.rect("g1").expect("placed").y0,
        tag_edge(&laid.nodes, "g1", "top").1
    ));
}

#[test]
fn an_ordinary_scene_is_untouched_by_the_fold() {
    // The fold is the part's, nobody else's: an ordinary scene's children
    // stay their own obstacles and take no fixed ports.
    let nodes = laid("|box#a| \"A\"\n|group#g| [ |box#b| \"B\" ]\na - g.b\n");
    let index = SceneIndex::build(&nodes);
    assert!(index.fixed_port("a").is_none());
    assert!(index.rect("g.b").is_some());
    assert_ne!(index.rect("g.b"), index.rect("g"));
}

// ───────────────────────── errors & strays ─────────────────────────

#[test]
fn a_side_on_a_terminal_is_an_error() {
    // A terminal is a **pin or a label** [SPEC 16.4] — whether or not the
    // part's drawing hands it a facing, which is a separate question.
    for (body, what) in [
        (anchor("u1", "") + "|gnd#g1|\nu1.a:left - g1\n", "a pin"),
        (
            anchor("u1", "") + "|gnd#g1|\nu1.a - g1:right\n",
            "a symbol label",
        ),
        // `sch-l`'s ports sit on its box's bottom corners, so the glyph gives
        // its pins no facing and they take no fixed port: still terminals.
        (
            anchor("u1", "") + "|L#l1|\nu1.a - l1.p1:left\n",
            "a facing-less pin",
        ),
        // A text-only `|label|` draws no symbol at all — no connection
        // geometry to read, and its own terminal all the same.
        (
            anchor("u1", "") + "|label#n1| \"NET\"\nu1.a - n1:right\n",
            "a symbol-less label",
        ),
    ] {
        let src = sheet(&body);
        let err = crate::layout::layout(&program(&src))
            .err()
            .unwrap_or_else(|| panic!("{what} takes no ':side'"));
        assert_eq!(
            err.message,
            "a terminal owns its connection — a pin or label takes no ':side'"
        );
        assert_eq!(err.code, Code::SIDE_ON_TERMINAL);
    }
    // A **part** is not a terminal, whatever its arity: a bare landing on one
    // resolves to a pin [SPEC 16.5], which is not this gate's business — the
    // forced side stands, and nothing is said about pins.
    for body in [
        anchor("u1", "") + "|R#r1|\nu1.a - r1:left\n",
        anchor("u1", "") + &anchor("u2", "") + "u1:right - u2:left\n",
    ] {
        let laid = crate::layout::layout(&program(&sheet(&body)));
        assert!(
            laid.is_ok(),
            "a part takes ':side': {}",
            laid.err().expect("checked").message
        );
    }
}

#[test]
fn a_forced_side_on_a_part_beats_its_bare_landing() {
    // A bare wire to a two-pin part lands on its first pin (the seat pass's
    // reading), but an authored `:side` is the user overruling that — the
    // convenience landing yields rather than silently contradicting it.
    let laid = routed(&sheet(&(anchor("u1", "") + "|R#r1|\nu1.a - r1:bottom\n")));
    let path = wire(&laid, "u1.a", "r1");
    let index = SceneIndex::build(&laid.nodes);
    let rect = index.rect("r1").expect("placed");
    let last = path[path.len() - 1];
    assert!(
        close(last.1, rect.y1),
        "it lands on the forced side: {last:?}"
    );
}

#[test]
fn a_self_loop_on_one_pin_is_the_one_side_loop() {
    // A fixed port forces one side onto both ends, which is ROUTING.md's
    // existing one-side loop — no new code, one honest report.
    let src = sheet(&(anchor("u1", "") + "u1.a - u1.a\n"));
    let laid = routed(&src);
    assert!(laid.links.is_empty(), "nothing lawful is drawn");
    assert!(
        laid.link_report
            .iter()
            .any(|v| v.detail == "self-loop with both ends forced onto one side"),
        "named: {:?}",
        laid.link_report
    );
}

// ───────────────────────── the whole sheet ─────────────────────────

/// A sheet with every landing shape on it: pin to pin, a seated ground, a
/// seated discrete, and two wires sharing one pin.
fn full_sheet() -> String {
    sheet(
        &(anchor("u1", "")
            + &anchor("u2", "")
            + "|gnd#g1|\n|R#r1|\nu1.a - g1\nu1.c - u2.a\nu1.c - u2.b\nu2.c - r1.p1\n"),
    )
}

#[test]
fn a_seated_sheet_holds_the_four_laws() {
    let laid = routed(&full_sheet());
    let found = crate::layout::validate_routing(&laid);
    assert!(found.is_empty(), "the laws hold on a sheet: {found:?}");
    assert!(laid.strays.is_empty(), "and every wire draws");
    assert_eq!(laid.links.len(), 4, "all four wires");
}

#[test]
fn a_chain_spanning_two_pins_draws_at_the_scopes_own_gap() {
    // [SPEC 16.1] the distribution strikes a spanning satellite between two
    // pins — and the tracks size for it, so the sheet needs no hand-set gap:
    // at the default 48 the ground landed on both landings it wires to and
    // both wires strayed with "fixed port blocked".
    let src = sheet(&(sided("u1") + &sided("u2") + "|gnd#g1|\nu1.b - g1\ng1 - u2.a\n"));
    let laid = routed(&src);
    assert!(
        laid.strays.is_empty(),
        "both wires draw: {:?}",
        laid.link_report
            .iter()
            .map(|v| v.detail.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(laid.links.len(), 2);
    let found = crate::layout::validate_routing(&laid);
    assert!(found.is_empty(), "and lawfully: {found:?}");
}

#[test]
fn a_schematic_type_outside_the_scope_is_the_gate_not_a_plain_box() {
    // [SPEC 21] Phase 3 let the family render anywhere and deferred the gate;
    // this is the gate. A `|label|` in a plain flow document is not an
    // ordinary box that happens to draw a symbol — it is an error, which is
    // what makes every downstream law able to key on the *part*.
    let doc = |part: &str| format!("{{ direction: row }}\n|box#a| \"A\"\n{part}\na - g:top\n");
    let err = crate::layout::layout(&program(&doc("|label#g| { symbol: gnd }")))
        .err()
        .expect("the gate");
    assert_eq!(err.message, "'|label|' belongs in a 'layout: schematic'");
    assert_eq!(err.code, Code::SCHEMATIC_TYPE);
    // The plain box it was compared against still routes, `:side` and all.
    let plain = routed(&doc("|box#g| \"G\""));
    let path = wire(&plain, "a", "g");
    let index = SceneIndex::build(&plain.nodes);
    let rect = index.rect("g").expect("placed");
    let last = path[path.len() - 1];
    assert!(close(last.1, rect.y0), "lands on the top side: {last:?}");
}

#[test]
fn a_wire_from_outside_lands_on_a_nested_sheets_pin() {
    // [SPEC 16] **the terminal's own scope answers, not the wire's**: a pin is
    // a pin whoever wires it. A root wire into a nested `|schematic|` lands on
    // the pin's stub tip exactly as the sheet's own wire does, and the `:side`
    // ban travels with it — while the same wire's outer end stays an ordinary
    // box with an ordinary forced side.
    let src = "{ direction: row; gap: 60 }\n|box#a| \"A\"\n|schematic#s| { padding: 30 } [\n"
        .to_string()
        + &anchor("u1", "")
        + "]\na - s.u1.a\n";
    let laid = routed(&src);
    assert!(laid.strays.is_empty(), "it draws: {:?}", laid.link_report);
    let path = wire(&laid, "a", "s.u1.a");
    let tip = stub_tip(&laid.nodes, "u1", "a");
    let last = path[path.len() - 1];
    assert!(
        near(last, tip),
        "lands on the nested pin's stub tip: {last:?} vs {tip:?}"
    );

    let err = crate::layout::layout(&program(&src.replace("- s.u1.a", "- s.u1.a:left")))
        .err()
        .expect("a nested pin still owns its connection");
    assert_eq!(
        err.message,
        "a terminal owns its connection — a pin or label takes no ':side'"
    );
    assert_eq!(err.code, Code::SIDE_ON_TERMINAL);
}

#[test]
fn a_wire_from_outside_lands_on_a_sealed_engines_pin() {
    // The corner the type gate's doc rules on ([`super::check_types`]): a
    // nested `|sequence|` seals the *reading of statements*, never the address.
    // A part drawn inside one is still an addressed part, so a wire written on
    // the sheet lands on its pin like any other — the endpoint decides.
    let src = sheet(
        &(anchor("u1", "") + "|sequence#seq| [\n" + &anchor("u2", "") + "]\nu1.c - seq.u2.a\n"),
    );
    let laid = routed(&src);
    assert!(laid.strays.is_empty(), "it draws: {:?}", laid.link_report);
    let path = wire(&laid, "u1.c", "seq.u2.a");
    let tip = stub_tip(&laid.nodes, "u2", "a");
    assert!(
        near(path[path.len() - 1], tip),
        "on the sealed part's stub tip: {:?} vs {tip:?}",
        path[path.len() - 1]
    );
}

// ───────────────────────── arity's landings [SPEC 16.5] ─────────────────────────

/// The oracle for a symbol part's pin: the placed zero-size port node desugar
/// seated on the glyph's port, read inside the part it belongs to.
fn port_at(nodes: &[PlacedNode], part: &str, pin: &str) -> (f64, f64) {
    let (owner, ox, oy) = placed(nodes, part);
    let (_, px, py) = placed(std::slice::from_ref(owner), pin);
    (ox - owner.cx + px, oy - owner.cy + py)
}

#[test]
fn a_series_chain_routes_through_its_part() {
    // [SPEC 16.5] `vm - |R| - |gnd|` is a series circuit: the router draws two
    // wires, in on p1 and out of p2, each landing on that port.
    let src = sheet(&(anchor("u1", "") + "|R#r1|\n|gnd#g1|\nu1.c - r1 - g1\n"));
    let laid = routed(&src);
    let into = wire(&laid, "u1.c", "r1.p1");
    let out = wire(&laid, "r1.p2", "g1");
    assert!(
        near(into[into.len() - 1], port_at(&laid.nodes, "r1", "p1")),
        "in on p1: {:?}",
        into[into.len() - 1]
    );
    assert!(
        near(out[0], port_at(&laid.nodes, "r1", "p2")),
        "out of p2: {:?}",
        out[0]
    );
    assert_eq!(laid.links.len(), 2, "one statement, two wires");
    assert!(laid.strays.is_empty(), "and both draw");
    let found = crate::layout::validate_routing(&laid);
    assert!(found.is_empty(), "lawfully: {found:?}");
}

#[test]
fn same_pin_landings_merge_through_the_arity_resolved_endpoints() {
    // A one-pin part never runs out of pins, so both pinless landings resolve
    // to `u1.a` — and ROUTING.md's implicit fan takes it from there: one
    // bit-exact port, one drawn lead until the split. Nothing here is new
    // routing; the point is that arity's *rewritten* addresses reach it.
    // `cell:` keeps the one-pin part an anchor, so this reads the fan and
    // not the seat pass [SPEC 16.1].
    let src = sheet(
        &("  |component#u1| { cell: 1 1 } [ |pin#a| ]\n".to_string()
            + &anchor("u2", " { cell: 2 1 }")
            + "u1 - u2.a\nu1 - u2.b\n"),
    );
    let laid = routed(&src);
    let one = wire(&laid, "u1.a", "u2.a").to_vec();
    let two = wire(&laid, "u1.a", "u2.b").to_vec();
    assert_eq!(one[0], two[0], "one landing, bit for bit");
    assert!(
        near(one[0], stub_tip(&laid.nodes, "u1", "a")),
        "and it is the pin arity chose"
    );
    // The shared lead: both leave along the port's own ordinate, the shorter
    // inside the longer — overlapping ink, not two rails.
    assert!(close(one[1].1, one[0].1) && close(two[1].1, one[0].1));
    let (a, b) = (one[1].0 - one[0].0, two[1].0 - two[0].0);
    assert!(a * b > 0.0 && (a - b).abs() < a.abs().max(b.abs()));
    assert!(
        crate::layout::validate_routing(&laid).is_empty(),
        "and the fan holds the laws"
    );
}

#[test]
fn a_pinless_landing_seats_the_satellite_at_the_pin_it_resolved() {
    // The seat pass reads the *resolved* endpoints [SPEC 16.1], so a chain
    // that entered p1 and left by p2 seats exactly as the explicit spelling
    // does — the sample's series cap and its pin-named twin are one sheet.
    let series = sheet(&(anchor("u1", "") + "|C#c1|\n|gnd#g1|\nu1.c - c1 - g1\n"));
    let spelled = sheet(&(anchor("u1", "") + "|C#c1|\n|gnd#g1|\nu1.c - c1.p1\nc1.p2 - g1\n"));
    let paths = |src: &str| -> Vec<Vec<(f64, f64)>> {
        routed(src).links.iter().map(|w| w.path.clone()).collect()
    };
    assert_eq!(paths(&series), paths(&spelled), "the same drawn sheet");
}

// ───────────── the nested sheet's margin [Phase 4 carry-over] ─────────────

/// A sheet's interior, as a body: an anchor, two seated satellites and a tag.
fn interior() -> String {
    anchor("u1", "")
        + "  |C#c1|\n  |gnd#g1|\n  |label#pw| { symbol: power }\n\
           pw - u1.a\n  u1.c - c1 - g1\n"
}

#[test]
fn a_nested_sheet_routes_its_own_interior_with_no_margin_at_all() {
    // Phase 4's carry-over, in-scope half: a `|schematic|` node sitting among
    // ordinary flow content wires its whole interior with **no** padding — the
    // seat pass grows each satellite off the pin it touches, so no in-scope
    // wire ever needs to leave the parts' bbox.
    let src = format!("|box#note| \"n\"\n|schematic#s| [\n{}]\n", interior());
    let laid = crate::layout::layout(&program(&src)).expect("layout");
    assert!(
        laid.strays.is_empty(),
        "the interior routes bare: {:?}",
        laid.link_report
    );
}

#[test]
fn a_wire_into_a_sheet_needs_a_corridor_and_that_corridor_is_the_whole_rule() {
    // The carry-over's other half, closed. A sheet's box holds its parts'
    // **ink** ([`super::field::drawn`]), so a part's stub tips cannot poke out
    // of the scope and eat the neighbouring gap from the outside: the old
    // non-monotone "band" collapses into ROUTING.md's own rule — a wire needs
    // a corridor, so the free space in front of a fixed port has to exceed
    // `2 × clearance`, and nothing else moves the answer.
    //
    // The corridor is **measured, not authored**: a scope's frame lands on the
    // fine lattice [SPEC 16.1], so the flow gap it is handed rounds by up to
    // half a pitch either way. That is the one thing between the author's
    // `gap:` and the router's rule, and the rule itself is untouched.
    let sheet = |gap: f64, clearance: f64, pad: &str| {
        format!(
            "{{ direction: row; gap: {gap}; clearance: {clearance} }}\n\
             |box#a| \"A\"\n|schematic#s|{pad} [\n{}]\na - s.u1.a\n",
            anchor("u1", "")
        )
    };
    // The free space in front of the port, and whether the wire drew.
    let corridor = |src: &str| {
        let laid = crate::layout::layout(&program(src)).expect("layout");
        let (a, ax, _) = placed(&laid.nodes, "a");
        let (s, sx, _) = placed(&laid.nodes, "s");
        (
            sx + s.bbox.min_x - (ax + a.bbox.max_x),
            laid.strays.is_empty(),
        )
    };
    for c in [16.0, 8.0] {
        for gap in [12.0, 20.0, 28.0, 33.0, 36.0, 44.0, 60.0] {
            let (free, drew) = corridor(&sheet(gap, c, ""));
            assert_eq!(
                drew,
                free > 2.0 * c,
                "clearance {c}, gap {gap}: {free} of corridor and drew {drew}"
            );
        }
    }
    // Interior padding buys nothing — it grows the sheet, and the flow moves
    // the neighbour out with it — but the scope's own gap is the honest lever.
    for pad in [" { padding: 30 }", " { padding: 120 }"] {
        let (free, drew) = corridor(&sheet(20.0, 16.0, pad));
        assert!(
            !drew && free < 32.0,
            "padding is not the corridor: {pad} left {free}"
        );
    }
}

#[test]
fn a_sheet_inside_a_page_wires_its_own_interior() {
    // A `|schematic|` nested in a `|page|` used to stray on its *own* interior
    // wire: the sheet's generated `|frame|` is a full-page `|rect|` sibling,
    // and reading it as a body walled in everything inside it. Chrome is drawn,
    // never solid ([`crate::routing::ortho::scene`]), so a page now hosts a
    // sheet exactly as any other container does.
    let sheet = |wrapper: &str, style: &str| {
        format!(
            "|{wrapper}#p|{style} [\n|schematic#s| [\n{}  |gnd#g1|\n  u1.c - g1\n]\n]\n",
            anchor("u1", "")
        )
    };
    let strays = |src: &str| {
        crate::layout::layout(&program(src))
            .expect("layout")
            .strays
            .len()
    };
    for wrapper in ["page", "group", "box", "column", "row", "block"] {
        assert_eq!(strays(&sheet(wrapper, "")), 0, "a |{wrapper}| is clean");
    }
    for style in [
        " { padding: 60 }",
        " { gap: 80 }",
        " { clearance: 2 }",
        " { width: 2000; height: 1500 }",
    ] {
        assert_eq!(strays(&sheet("page", style)), 0, "and stays clean: {style}");
    }
}

/// The **net-run convention** [SPEC 16.4]: a plain net label is a stretch of
/// trace with its name beside it, so the wire crosses the whole box and lands
/// on its far end, and the name steps clear of the centreline. Judged off the
/// drawn wire and the placed text, never off the reader that produced them.
#[test]
fn a_plain_net_label_is_a_run_the_wire_travels() {
    let laid = routed(&sheet(
        &(sided("u1").replace("  |component", "|component") + "u1.b - \"NET\"\n"),
    ));
    let nodes = &laid.nodes;
    let (run, rx, ry) = placed(nodes, "lini-label-1");
    let path = wire(&laid, "u1.b", "lini-label-1");
    let (end, start) = (path[path.len() - 1], path[0]);
    // Pin `b` faces right, so the run lies to the right of it: the wire enters
    // the box's near edge and lands on the **far** one, having crossed it
    // whole.
    let (near_x, far_x) = (rx + run.bbox.min_x, rx + run.bbox.max_x);
    assert!(near(end, (far_x, ry)), "{end:?} vs the far edge {far_x}");
    assert!(
        start.0 < near_x && near_x < end.0,
        "the wire crosses the run: {start:?} → {end:?}, near edge {near_x}"
    );
    // …and the name stands off the trace it names, above a horizontal run.
    let text = run.children.first().expect("the net text");
    assert!(
        ry + text.cy + text.bbox.max_y < ry - crate::ledger::consts::NET_LABEL_OFFSET + 1e-9,
        "the name clears the centreline: {} vs {ry}",
        ry + text.cy + text.bbox.max_y
    );
}

/// A **shaped** tag is untouched [SPEC 16.4]: it stays a body the wire ends
/// on, its landing the outline's own edge — the run reading is the plain
/// shape's alone.
#[test]
fn a_shaped_tag_still_terminates_its_wire() {
    let laid = routed(&sheet(
        &(sided("u1").replace("  |component", "|component") + "u1.b -> \"NET\"\n"),
    ));
    let (tag, tx, _) = placed(&laid.nodes, "lini-label-1");
    let path = wire(&laid, "u1.b", "lini-label-1");
    let end = path[path.len() - 1];
    assert!(
        end.0 <= tx + tag.bbox.min_x + 1e-6,
        "the wire stops at the tag: {end:?} vs {}",
        tx + tag.bbox.min_x
    );
}

/// Two distinct fixed ports on one side of one body are a lawful pair
/// (ROUTING.md Fixed ports): the wire runs out one clearance, along the
/// side, and back in — never a "fixed port blocked" stray. Tying two
/// adjacent pins (`u5.nre - u5.de`) is bread-and-butter on a sheet.
#[test]
fn two_pins_on_one_side_of_one_part_tie_with_a_u_route() {
    let src = sheet(
        "|component#u5| [\n  |pin#ro| { side: left }; |pin#nre| { side: left }; |pin#de| { side: left }; |pin#vcc| { side: right }\n]\nu5.nre - u5.de\n",
    );
    let laid = routed(&src);
    let path = wire(&laid, "u5.nre", "u5.de");
    let (from, to) = (
        stub_tip(&laid.nodes, "u5", "nre"),
        stub_tip(&laid.nodes, "u5", "de"),
    );
    assert!(
        near(path[0], from),
        "the wire leaves nre's stub tip {from:?}, drew {:?}",
        path[0]
    );
    assert!(
        near(path[path.len() - 1], to),
        "the wire lands on de's stub tip {to:?}, drew {:?}",
        path[path.len() - 1]
    );
    // Both ends leave the left side perpendicular (ROUTING.md Law 2).
    assert!(
        close(path[0].1, path[1].1),
        "the lead leaves along nre's pin"
    );
    assert!(
        close(path[path.len() - 1].1, path[path.len() - 2].1),
        "the lead arrives along de's pin"
    );
    assert!(laid.strays.is_empty(), "a same-side pin pair routes whole");
}

/// A chain hanging off a **net run** carries on along the run's own line
/// [SPEC 16.4/16.5]. The pin's wire crosses the run and lands on its far end;
/// the pull-up leaves that very point, and the two are one conductor — the
/// wire being named and its continuation — so they weld on the line rather
/// than laddering apart, which no port pinned to it could survive.
#[test]
fn a_chain_hanging_off_a_net_run_carries_on_along_its_line() {
    let src = "{ layout: schematic;\n  |v3::label| { symbol: power } [ \"3V3\" ]\n}\n\
               |J#j4| { pins: 3 }\n|R#r16| \"10k\"\n|label#tach| \"FAN_TACH\"\n\
               j4.p1 - |v3|\nj4.p2 - tach\ntach - r16 - |v3|\n";
    let laid = routed(src);
    // The chain grows up into the flag, so the wire enters the run's bottom
    // edge and lands on its top one — the end away from the pin.
    let (run, rx, ry) = placed(&laid.nodes, "tach");
    let landing = (rx, ry + run.bbox.min_y);
    let lead = wire(&laid, "j4.p2", "tach");
    assert!(
        near(lead[lead.len() - 1], landing),
        "the lead lands on the run's far end {landing:?}, drew {:?}",
        lead[lead.len() - 1]
    );
    let on = wire(&laid, "tach", "r16.p1");
    assert!(
        near(on[0], landing) && on.len() == 2,
        "the pull-up leaves that same point straight on: {on:?}"
    );
    assert!(
        close(on[1].0, landing.0),
        "…on the run's own line: {on:?} vs {landing:?}"
    );
}

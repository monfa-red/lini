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
    let b = stub.bbox;
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
    let b = sym.bbox;
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
    // The ground seats out along the pin's own axis — a left pin, so the tag
    // faces right and the lead is straight.
    assert!(
        near(path[0], stub_tip(&laid.nodes, "u1", "a")),
        "off the stub tip"
    );
    assert!(
        near(path[path.len() - 1], tag_edge(&laid.nodes, "g1", "right")),
        "on the symbol's connection point, drew {:?}",
        path[path.len() - 1]
    );
    assert_eq!(path.len(), 2, "a straight lead: {path:?}");
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
        index.rect("g1").expect("placed").x1,
        tag_edge(&laid.nodes, "g1", "right").0
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
fn a_schematic_type_outside_the_scope_routes_as_a_plain_box() {
    // [SPEC 16.7] the family renders **anywhere** — Phase 3's deliberate
    // deferral, Phase 5's gate to close — but the sheet's laws belong to the
    // scope, not to the type. In a plain flow document a `|label|` carrying a
    // symbol is an ordinary box: no fixed port forces its side, its connection
    // point is no terminal, and `:side` on it is legal (it errored once the
    // family shipped, which is a regression a flow diagram never asked for).
    let doc = |part: &str| format!("{{ direction: row }}\n|box#a| \"A\"\n{part}\na - g:top\n");
    let plain = routed(&doc("|box#g| \"G\""));
    let symbol = routed(&doc("|label#g| { symbol: gnd }"));
    assert!(symbol.strays.is_empty(), "it draws, as the plain box does");
    let on_top = |laid: &LaidOut| {
        let path = wire(laid, "a", "g");
        let index = SceneIndex::build(&laid.nodes);
        let rect = index.rect("g").expect("placed");
        let last = path[path.len() - 1];
        assert!(close(last.1, rect.y0), "lands on the top side: {last:?}");
    };
    on_top(&plain);
    on_top(&symbol);
}

#[test]
fn a_sheets_wires_are_deterministic() {
    let src = full_sheet();
    let once: Vec<Vec<(f64, f64)>> = routed(&src).links.iter().map(|w| w.path.clone()).collect();
    for _ in 0..3 {
        let again: Vec<Vec<(f64, f64)>> =
            routed(&src).links.iter().map(|w| w.path.clone()).collect();
        assert_eq!(once, again, "the same sheet routes identically");
    }
}

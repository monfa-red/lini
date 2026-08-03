//! Schematic dispatch [SPEC 16]: the engine is reached, a root scope is the
//! sheet, placement stops at a nested scope while the link scope reaches
//! through it, and the whole pass is deterministic. The helpers every
//! schematic suite shares live here — the tracks and roles are
//! [`super::place_tests`], the seats [`super::seat_tests`], the router
//! [`super::route_tests`].

use crate::layout::PlacedNode;
use crate::ledger::defaults::SCH_GAP;
use crate::resolve::Program;

pub(super) fn program(src: &str) -> Program {
    let toks = crate::lexer::lex(src).expect("lex");
    let file = crate::syntax::parser::parse(src, &toks).expect("parse");
    let lowered = crate::desugar::desugar(&file).expect("desugar");
    crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve")
}

pub(super) fn laid(src: &str) -> Vec<PlacedNode> {
    crate::layout::layout(&program(src)).expect("layout").nodes
}

/// The layout error a scene raises, for the placement diagnostics.
pub(super) fn layout_err(src: &str) -> String {
    crate::layout::layout(&program(src))
        .err()
        .expect("a layout error")
        .message
        .clone()
}

/// A three-pin `|component|` — the scope's canonical **anchor** [SPEC 16.1].
pub(super) fn anchor(id: &str, style: &str) -> String {
    format!("  |component#{id}|{style} [\n    |pin#a|; |pin#b|; |pin#c|\n  ]\n")
}

/// A schematic scope wrapping `body`.
pub(super) fn scope(style: &str, body: &str) -> String {
    format!("|schematic#s|{style} [\n{body}]\n")
}

/// A placed node by id, with its centre accumulated down the tree.
pub(super) fn placed<'a>(nodes: &'a [PlacedNode], id: &str) -> (&'a PlacedNode, f64, f64) {
    fn walk<'a>(
        nodes: &'a [PlacedNode],
        id: &str,
        ox: f64,
        oy: f64,
    ) -> Option<(&'a PlacedNode, f64, f64)> {
        for n in nodes {
            let (x, y) = (ox + n.cx, oy + n.cy);
            if n.id.as_deref() == Some(id) {
                return Some((n, x, y));
            }
            if let Some(f) = walk(&n.children, id, x, y) {
                return Some(f);
            }
        }
        None
    }
    walk(nodes, id, 0.0, 0.0).unwrap_or_else(|| panic!("no placed node '{id}'"))
}

pub(super) fn at(nodes: &[PlacedNode], id: &str) -> (f64, f64) {
    let (_, x, y) = placed(nodes, id);
    (x, y)
}

/// The clear space between two placed nodes along x, in scene coordinates.
pub(super) fn x_gap(nodes: &[PlacedNode], left: &str, right: &str) -> f64 {
    let (l, lx, _) = placed(nodes, left);
    let (r, rx, _) = placed(nodes, right);
    (rx + r.bbox.min_x) - (lx + l.bbox.max_x)
}

/// A placed node's bbox **centre** in scene coords plus its extent — the cell
/// geometry the tracks size against — what a cluster is measured from.
pub(super) fn cell(nodes: &[PlacedNode], id: &str) -> (f64, f64, f64, f64) {
    let (n, x, y) = placed(nodes, id);
    let (bx, by) = n.bbox.center();
    (x + bx, y + by, n.bbox.w(), n.bbox.h())
}

/// The clear space between two placed nodes along y.
pub(super) fn y_gap(nodes: &[PlacedNode], top: &str, bottom: &str) -> f64 {
    let (t, _, ty) = placed(nodes, top);
    let (b, _, by) = placed(nodes, bottom);
    (by + b.bbox.min_y) - (ty + t.bbox.max_y)
}

pub(super) fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// A three-pin `|component|` whose pins take one side each — `a` left, `b`
/// right, `c` bottom — so a test can name the outward direction it wants.
pub(super) fn sided(id: &str) -> String {
    sided_with(id, "")
}

pub(super) fn sided_with(id: &str, style: &str) -> String {
    format!(
        "  |component#{id}|{style} [\n    |pin#a| {{ side: left }}; |pin#b| {{ side: right }}; |pin#c| {{ side: bottom }}\n  ]\n"
    )
}

/// A pin's stub **tip** — where its wire lands [SPEC 16.2]: the far end of
/// the lead, on the side it points (`out_right` picks which end).
pub(super) fn tip(nodes: &[PlacedNode], pin: &str, out_right: bool) -> f64 {
    let (p, px, _) = placed(nodes, pin);
    let stub = p
        .children
        .iter()
        .find(|c| c.type_chain.iter().any(|t| t == "pin-stub"))
        .expect("every pin wears a stub");
    let x = px + stub.cx;
    if out_right {
        x + stub.bbox.max_x
    } else {
        x + stub.bbox.min_x
    }
}

/// The pose a placed part wears, in degrees — desugar leaves it on the chain
/// [SPEC 16.7].
pub(super) fn pose_of(nodes: &[PlacedNode], id: &str) -> u32 {
    let (n, ..) = placed(nodes, id);
    n.type_chain
        .iter()
        .find_map(|t| t.strip_prefix("pose-")?.parse().ok())
        .unwrap_or(0)
}

/// The seat pass's warnings a source raises [SPEC 21] — by their code, so a
/// test sees every one the pass reports, never only the shape it expected.
pub(super) fn seat_warnings(src: &str) -> Vec<String> {
    crate::compile_str_checked(src, &crate::Options::default())
        .expect("compile")
        .1
        .iter()
        .filter(|d| d.code == crate::error::Code::SCHEMATIC_SEAT)
        .map(|d| d.message.clone())
        .collect()
}

// ───────────────────────── errors & determinism ─────────────────────────

#[test]
fn a_malformed_schematic_cell_errors_like_a_grids() {
    let msg = layout_err(&scope("", &anchor("u1", " { cell: 0 1 }")));
    assert!(
        msg.contains("'cell column' expects a positive integer"),
        "{msg}"
    );
}

#[test]
fn schematic_columns_is_a_count_not_a_track_list() {
    let msg = layout_err(&scope(" { columns: 40, auto }", &anchor("u1", "")));
    assert!(
        msg.contains("'columns' in a schematic is the wrap count"),
        "{msg}"
    );
}

#[test]
fn cell_is_legal_in_a_schematic_and_still_off_grid_elsewhere() {
    let diags = |src: &str| {
        crate::lint_str(src)
            .expect("parse")
            .iter()
            .map(|d| d.message.clone())
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(diags(&scope("", &anchor("u1", " { cell: 2 1 }"))), "");
    // The SPEC 12 gate is untouched outside the two engines.
    let flow = diags("|box#f| [\n  |box#a| \"a\" { cell: 1 1 }\n]\n");
    assert!(flow.contains("this box sits in a 'layout: flow'"), "{flow}");
    // And a grid still places by its own laws.
    let grid = crate::compile_str(
        "|box#g| { layout: grid; columns: repeat(2) } [\n  |box#a| \"a\" { cell: 1 2 }\n]\n",
    );
    assert!(grid.is_ok(), "the grid is unchanged: {grid:?}");
}

#[test]
fn the_placement_is_deterministic() {
    // Every ordering decision — the ordinal collapse, the flow cursor, the
    // chain walk, the seat stack, the fallback row — is source-ordered, so a
    // rerun of a scene holding all of them is byte-identical.
    let src = scope(
        " { columns: 2 }",
        &(anchor("u1", " { cell: 30 1 }")
            + &anchor("u2", "")
            + &anchor("u3", " { cell: 10 2 }")
            + "  |R#r1| \"1k\"\n  |gnd#g1|\n  |gnd#g2|\n  |C#c1| \"1n\"\n"
            + "  u1.a - r1.p1\n  r1.p2 - g1\n  u1.a - g2\n  u2.b - c1.p1\n  c1.p2 - u3.a\n"),
    );
    let first = crate::compile_str(&src).expect("compile");
    for _ in 0..3 {
        assert_eq!(first, crate::compile_str(&src).expect("compile"));
    }
}

// ───────────────────────── dispatch (Task 1's pins) ─────────────────────────

#[test]
fn a_root_schematic_scene_is_the_scope() {
    // The root intercept runs before the generic child loop, so the anchors
    // take the same one row (and the same `gap` default) as in a node.
    let src = format!(
        "{{ layout: schematic }}\n\n{}",
        anchor("u1", "").trim_start().to_string() + anchor("u2", "").trim_start()
    );
    let nodes = laid(&src);
    let ((x1, y1), (x2, y2)) = (at(&nodes, "u1"), at(&nodes, "u2"));
    assert!(x1 < x2, "declaration order: {x1} {x2}");
    assert!(close(y1, y2), "one row: {y1} {y2}");
    assert!(close(x_gap(&nodes, "u1", "u2"), SCH_GAP));
}

#[test]
fn placement_does_not_cascade_into_a_nested_scope() {
    // [SPEC 16] the drawing precedent: a nested `|column|` places its own
    // children — they stack, while it rides the schematic's tracks as an
    // anchor. It is no part, so it seats nowhere *and* it does not inherit its
    // children's pin arity: a column holding a two-pin `|R|` is not a jumper.
    let nodes = laid(&scope(
        "",
        &(anchor("u1", "") + "  |column#c| [\n    |box#a| \"a\"\n    |R#b| \"1k\"\n  ]\n"),
    ));
    let ((ax, ay), (bx, by)) = (at(&nodes, "a"), at(&nodes, "b"));
    assert!(close(ax, bx), "the column stacks: {ax} vs {bx}");
    assert!(by > ay, "in declaration order: {ay} vs {by}");
    let ((ux, uy), (cx, cy)) = (at(&nodes, "u1"), at(&nodes, "c"));
    assert!(cx > ux, "the column rides the scope's track row");
    assert!(close(uy, cy), "beside the anchor, not below it");
}

#[test]
fn a_nested_row_places_its_own_children_though_the_scope_still_reaches_them() {
    // [SPEC 16] the placement/link split, both halves in one scene. **Placement
    // does not cascade**: a nested `|row|` runs its own engine, so the `|gnd|`
    // and the two-pin `|R|` inside it are ordinary row children in declaration
    // order at the *row's* gap — never the schematic's satellites, even with a
    // wire from an outer pin reaching one of them (it seats nothing, and raises
    // no adrift warning: only a direct child has a role here). **The laws do
    // cascade**: the `|R|` inside the row is legal there, which is the type
    // gate's half of the same split.
    // The wire names the row's child by its path — a named container opens a
    // scope [SPEC 9], and the sheet no longer invents the box a bare `g1`
    // would have asked for (Task 5.2's no-auto-create law).
    let row = "  |row#r| { gap: 12 } [\n    |gnd#g1|\n    |R#r1| \"1k\"\n  ]\n";
    let src = scope("", &(anchor("u1", "") + row + "  u1.a - r.g1\n"));
    let nodes = laid(&src);

    let ((gx, gy), (rx, ry)) = (at(&nodes, "g1"), at(&nodes, "r1"));
    assert!(close(gy, ry), "the row lays out row-wise: {gy} vs {ry}");
    assert!(gx < rx, "in declaration order: {gx} {rx}");
    assert!(
        close(x_gap(&nodes, "g1", "r1"), 12.0),
        "at the row's own gap, not the scope's {SCH_GAP}: {}",
        x_gap(&nodes, "g1", "r1")
    );

    // The row itself is the anchor — one child of the scope, on its track row.
    let ((ux, uy), (wx, wy)) = (at(&nodes, "u1"), at(&nodes, "r"));
    assert!(wx > ux, "the row rides the scope's track row: {ux} {wx}");
    assert!(close(uy, wy), "beside the anchor, not seated below it");

    // The wire moved nothing: g1 sits exactly where the bare row put it.
    let bare = laid(&scope("", &(anchor("u1", "") + row)));
    let (bgx, bgy) = at(&bare, "g1");
    let (bwx, bwy) = at(&bare, "r");
    assert!(
        close(gx - wx, bgx - bwx) && close(gy - wy, bgy - bwy),
        "a wire never seats a nested scope's child: {:?} vs {:?}",
        (gx - wx, gy - wy),
        (bgx - bwx, bgy - bwy)
    );
    assert!(
        seat_warnings(&src).is_empty(),
        "nor does it call one adrift"
    );
}

#[test]
fn a_schematic_layout_is_dispatched_never_read_as_a_box_mode() {
    // `layout: schematic` on any container is intercepted in `layout_inst`; it
    // must never reach the box arranger's flow/grid reader.
    let nodes = laid("|box#s| { layout: schematic } [\n  |box#a| \"a\"\n  |box#b| \"b\"\n]\n");
    let ((ax, ay), (bx, by)) = (at(&nodes, "a"), at(&nodes, "b"));
    assert!(bx > ax && close(ay, by), "one row");
}

#[test]
fn a_schematic_wire_stays_the_routers_link() {
    // [SPEC 16.7] the engine places and never consumes: unlike a sequence's
    // messages or a drawing's measures, a schematic scope's wires stay
    // requests — so neither the request filter nor the declared-edge count
    // grows a schematic clause. (Landing them on pin ports is this phase's
    // routing task; what this pins is the ownership.)
    let src = "|schematic#s| [\n  |R#r1| \"1k\"\n  |R#r2| \"2k\"\n  r1.p2 - r2.p1\n]\n";
    let p = program(src);
    assert!(
        p.links
            .iter()
            .all(|w| crate::routing::ortho::request::is_routed(&p, w)),
        "the router owns a schematic wire"
    );
    assert_eq!(crate::testing::declared_edges(src), 1);
}

#[test]
fn the_type_gate_reaches_every_nesting_the_scope_encloses() {
    // [SPEC 16/21] `|R|` belongs in a `layout: schematic` — and the scope
    // *reaches*, unlike the sequence's and drawing's immediate-scope tests.
    // Every shape that encloses a part legally, and the two that do not.
    for (what, src) in [
        ("the scope itself", "|schematic#s| [\n  |R#r1| \"1k\"\n]\n"),
        (
            "a nested ordinary container",
            "|schematic#s| [\n  |row#r| [\n    |R#r1| \"1k\"\n  ]\n]\n",
        ),
        (
            "an anonymous container, which owns no path segment",
            "|schematic#s| [\n  |row| [\n    |R| \"1k\"\n  ]\n]\n",
        ),
        ("an anonymous scope", "|schematic| [\n  |R| \"1k\"\n]\n"),
        (
            "the root scope",
            "{ layout: schematic }\n|row#r| [\n  |R#r1| \"1k\"\n]\n",
        ),
        (
            "a define that carries the layout",
            "{ |sheet::group| { layout: schematic } }\n|sheet#s| [\n  |R#r1| \"1k\"\n]\n",
        ),
    ] {
        crate::layout::layout(&program(src)).unwrap_or_else(|e| panic!("{what}: {}", e.message));
    }
    for (what, src) in [
        ("the bare root", "|R#r1| \"1k\"\n"),
        (
            "a sibling scope",
            "|schematic#s| [\n  |R#r1| \"1k\"\n]\n|box#o| [\n  |R#r2| \"2k\"\n]\n",
        ),
    ] {
        assert_eq!(
            layout_err(src),
            "'|R|' belongs in a 'layout: schematic'",
            "{what}"
        );
    }
}

#[test]
fn every_schematic_type_is_gated_and_named_as_written() {
    // The whole family [SPEC 21], each reported by the type the author wrote —
    // a `|gnd|` says `'|gnd|'`, not the `|label|` it defines over.
    let mut cases: Vec<(String, String, String)> = vec![
        (
            String::new(),
            "|component#u1| [\n    |pin#a|\n  ]\n".into(),
            "component".into(),
        ),
        (String::new(), "|pin#a|\n".into(), "pin".into()),
        (String::new(), "|label#n1| \"N\"\n".into(), "label".into()),
        (String::new(), "|junction#j|\n".into(), "junction".into()),
        (String::new(), "|J#j1| { pins: 2 }\n".into(), "J".into()),
        (String::new(), "|opamp#o1|\n".into(), "opamp".into()),
        (String::new(), "|gnd#g1|\n".into(), "gnd".into()),
        (String::new(), "|nc#n|\n".into(), "nc".into()),
        (
            "{ |vm::label| { symbol: power } }\n".into(),
            "|vm#v1|\n".into(),
            "vm".into(),
        ),
    ];
    cases.extend(
        crate::desugar::types::DISCRETES
            .iter()
            .map(|t| (String::new(), format!("|{t}#x1|\n"), (*t).to_string())),
    );
    for (prelude, part, ty) in cases {
        let outside = format!("{prelude}{part}");
        assert_eq!(
            layout_err(&outside),
            format!("'|{ty}|' belongs in a 'layout: schematic'"),
            "{outside}"
        );
        // …and legal the moment the scope encloses it.
        let inside = format!("{prelude}|schematic#s| [\n  {part}]\n");
        crate::layout::layout(&program(&inside))
            .unwrap_or_else(|e| panic!("{inside}: {}", e.message));
    }
    // `|schematic|` itself is exempt — it *creates* the scope.
    crate::layout::layout(&program("|schematic#s| [\n  |box#a|\n]\n")).expect("the scope");
}

#[test]
fn a_turned_part_is_turned_geometry_by_the_time_the_engine_sees_it() {
    // [SPEC 16.1] the pose is structural, so the placed scene — what the
    // router reads — is already turned: no paint rotation anywhere, the body
    // transposed, and the ports on the sides they landed on.
    let nodes = laid(&scope(
        "",
        "  |R#r1| \"1k\"\n  |R#r2| \"1k\" { rotate: 90 }\n",
    ));
    let (flat, ..) = placed(&nodes, "r1");
    let (stood, ..) = placed(&nodes, "r2");
    let port = |n: &PlacedNode, id: &str| {
        let c = n
            .children
            .iter()
            .find(|c| c.id.as_deref() == Some(id))
            .unwrap_or_else(|| panic!("no port '{id}'"));
        (c.cx, c.cy)
    };
    assert_eq!(stood.rotation, 0.0, "a part never paints a rotation");
    assert!(
        flat.bbox.w() > flat.bbox.h() && stood.bbox.h() > stood.bbox.w(),
        "the body transposes: {:?} vs {:?}",
        flat.bbox,
        stood.bbox
    );
    let (a, b) = (port(flat, "p1"), port(flat, "p2"));
    assert!(
        (b.0 - a.0).abs() == 64.0 && b.1 == a.1,
        "the plain resistor's ports span its length: {a:?} {b:?}"
    );
    let (a, b) = (port(stood, "p1"), port(stood, "p2"));
    assert!(
        (b.1 - a.1).abs() == 64.0 && b.0 == a.0,
        "the turned resistor's ports stand up: {a:?} {b:?}"
    );
}

#[test]
fn a_turned_components_pins_are_placed_on_the_sides_they_landed_on() {
    let src = scope(
        "",
        "  |component#u1| { rotate: 90 } [\n    |pin#a|; |pin#b|; |pin#c|\n  ]\n",
    );
    let nodes = laid(&src);
    let (_, _, body_y) = placed(&nodes, "u1");
    let (_, _, a_y) = placed(&nodes, "a");
    let (_, _, c_y) = placed(&nodes, "c");
    assert!(
        a_y < body_y,
        "the left rail swung to the top: {a_y} {body_y}"
    );
    assert!(c_y > body_y, "the right rail swung to the bottom: {c_y}");
}

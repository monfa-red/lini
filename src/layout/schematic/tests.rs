//! Schematic dispatch [SPEC 16]: the engine is reached, a root scope is the
//! sheet, placement stops at a nested scope while the link scope reaches
//! through it, and the whole pass is deterministic. The helpers every
//! schematic suite shares live here — the tracks and roles are
//! [`super::place_tests`], the cells [`super::field_tests`], the router
//! [`super::route_tests`].

use crate::layout::PlacedNode;
use crate::ledger::consts;
use crate::ledger::defaults::SCH_GAP;

pub(super) use crate::testutil::{layout_err, program};

pub(super) fn laid(src: &str) -> Vec<PlacedNode> {
    crate::testutil::laid(src).nodes
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
pub(super) use crate::testutil::placed_by_id as placed;

pub(super) fn at(nodes: &[PlacedNode], id: &str) -> (f64, f64) {
    let (_, x, y) = placed(nodes, id);
    (x, y)
}

/// A placed node's **drawn** extent in scene coordinates — the engine's one
/// extent notion ([`super::field::drawn`]), so a test measures the ink the
/// tracks reserved and never the box inside it.
pub(super) fn ink(nodes: &[PlacedNode], id: &str) -> crate::layout::ir::Bbox {
    let (n, x, y) = placed(nodes, id);
    super::field::drawn(n).shifted(x, y)
}

/// The clear space between two placed nodes along x, in scene coordinates.
pub(super) fn x_gap(nodes: &[PlacedNode], left: &str, right: &str) -> f64 {
    ink(nodes, right).min_x - ink(nodes, left).max_x
}

/// A placed node's drawn **centre** in scene coords plus its extent — the cell
/// geometry the tracks size against, which is what a cluster is measured from.
pub(super) fn cell(nodes: &[PlacedNode], id: &str) -> (f64, f64, f64, f64) {
    let b = ink(nodes, id);
    let (bx, by) = b.center();
    (bx, by, b.w(), b.h())
}

/// The point the lattice holds a placed part by [SPEC 16.1]: an **anchor** by
/// its own origin, which the packer lands on a coarse line, and a
/// **satellite** by its connection geometry, which the field pass stands on
/// the cell. One reading for every test that judges the invariant.
pub(super) fn seat(nodes: &[PlacedNode], id: &str) -> (f64, f64) {
    let (n, x, y) = placed(nodes, id);
    seat_of(n, (x, y))
}

/// …the same reading, for a walk that already holds the node and its origin.
pub(super) fn seat_of(node: &PlacedNode, at: (f64, f64)) -> (f64, f64) {
    if super::place::role(node) != crate::desugar::schematic::Role::Satellite {
        return at;
    }
    let (sx, sy) = super::terminal::seat_point(node);
    (at.0 + sx, at.1 + sy)
}

/// A placed part's own **connection point** in scene coordinates — where a
/// bare wire to it lands [SPEC 16.4], read through the one connection-geometry
/// reader the engine seats by.
pub(super) fn port(nodes: &[PlacedNode], id: &str) -> (f64, f64) {
    let (n, x, y) = placed(nodes, id);
    let at = super::terminal::terminal(n, None).at;
    (x + at.0, y + at.1)
}

/// Where a wire lands on one **named** terminal of a placed part in scene
/// coordinates — a pin's stub tip, a symbol part's port [SPEC 16.2] — through
/// the same reader the router's fixed ports come from, so a test judges the
/// point a wire really arrives at.
pub(super) fn landing(nodes: &[PlacedNode], id: &str, terminal: &str) -> (f64, f64) {
    let (n, x, y) = placed(nodes, id);
    let at = super::terminal::terminal(n, Some(terminal)).at;
    (x + at.0, y + at.1)
}

/// A placed node's **own box** in scene coords — its drawing without the
/// readout chrome hanging off it, for the assertions that are about where the
/// part itself landed.
pub(super) fn body(nodes: &[PlacedNode], id: &str) -> (f64, f64, f64, f64) {
    let (n, x, y) = placed(nodes, id);
    let (bx, by) = n.bbox.center();
    (x + bx, y + by, n.bbox.w(), n.bbox.h())
}

pub(super) fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// Whether a coordinate lands on a fine lattice point [SPEC 16.1] — the one
/// reading of the invariant, shared by the cell suite and the track suite.
pub(super) fn on_fine_grid(v: f64) -> bool {
    close(v, (v / consts::PIN_PITCH).round() * consts::PIN_PITCH)
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
    // The wire meets the ink: the lead's true endpoint, not its paint bbox.
    let b = stub.bbox.inflate(-stub.attrs.half_stroke());
    if out_right { x + b.max_x } else { x + b.min_x }
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

/// Every schematic part a scope placed, as `(id, x, y)` with the point the
/// lattice holds it by ([`seat_of`]) **in that scope's own frame** — what
/// [SPEC 16.1]'s invariant is stated in.
///
/// A part's own anatomy — pins, rails, readouts — is its business, so the walk
/// stops at the outermost schematic type it meets.
pub(super) struct ScopePart {
    pub id: String,
    pub at: (f64, f64),
}

pub(super) fn scope_parts(nodes: &[PlacedNode]) -> Vec<ScopePart> {
    fn walk(
        nodes: &[PlacedNode],
        ox: f64,
        oy: f64,
        scope: Option<(f64, f64)>,
        out: &mut Vec<ScopePart>,
    ) {
        for n in nodes {
            let (x, y) = (ox + n.cx, oy + n.cy);
            let scope = if super::is_schematic(&n.attrs) {
                Some((x, y))
            } else {
                scope
            };
            match (
                scope,
                crate::desugar::schematic::schematic_type(&n.type_chain),
            ) {
                (Some((sx, sy)), Some(ty)) => out.push(ScopePart {
                    id: n.id.clone().unwrap_or_else(|| format!("|{ty}|")),
                    at: {
                        let (px, py) = seat_of(n, (x, y));
                        (px - sx, py - sy)
                    },
                }),
                _ => walk(&n.children, x, y, scope, out),
            }
        }
    }
    let mut out = Vec::new();
    walk(nodes, 0.0, 0.0, None, &mut out);
    out
}

// ───────────────────────── the readout seats ─────────────────────────

/// The chrome box a placed part hangs off itself: the union of every
/// descendant of `id` wearing the generated class `class`, in scene
/// coordinates — a readout (`ref` / `part-value`) or a pin's lead
/// (`pin-stub` / `pin-number`).
pub(super) fn chrome(nodes: &[PlacedNode], id: &str, class: &str) -> crate::layout::ir::Bbox {
    let (part, px, py) = placed(nodes, id);
    let hits =
        crate::testutil::all_placed(&part.children, &|n| n.type_chain.iter().any(|t| t == class));
    assert!(!hits.is_empty(), "no '{class}' chrome under '{id}'");
    hits.iter()
        .map(|(n, x, y)| n.bbox.shifted(px + x, py + y))
        .reduce(|a, b| a.union(b))
        .expect("a box")
}

#[test]
fn a_top_pinned_components_readouts_clear_its_rail() {
    // [SPEC 16.2] the ref / value pair and a pin number both place
    // deterministically, with nothing to arbitrate between them — so they may
    // never want one band: a pin landing on **top** hangs its stub and its
    // number off the very edge the readouts are pinned to, and the pair
    // clears the whole band, keeping the same gap it keeps off a bare edge.
    // The pose is structural [SPEC 16.1], so a pin the turn *lands* on top is
    // the same case, not a second rule.
    for (what, part) in [
        (
            "an authored 'side: top'",
            "  |component#u1| \"LM2596S\" [\n    |pin#a| { side: top; number: 7 }\n    |pin#b| { side: left; number: 1 }\n  ]\n",
        ),
        (
            "a pose that lands one there",
            "  |component#u1| \"LM2596S\" { rotate: 90 } [\n    |pin#a| { number: 7 }\n    |pin#b| { side: bottom; number: 1 }\n  ]\n",
        ),
    ] {
        let nodes = laid(&scope("", part));
        let band = chrome(&nodes, "a", "pin-stub").union(chrome(&nodes, "a", "pin-number"));
        let value = chrome(&nodes, "u1", "part-value");
        let name = chrome(&nodes, "u1", "ref");
        assert!(
            value.max_y <= band.min_y && name.max_y <= band.min_y,
            "the readouts print over the top rail ({what}): {value:?} / {name:?} vs {band:?}"
        );
        assert!(
            close(band.min_y - value.max_y, consts::READOUT_GAP),
            "…one gap clear of it ({what}): {}",
            band.min_y - value.max_y
        );
        assert!(
            close(value.min_y - name.max_y, consts::READOUT_STACK),
            "…and the pair moved as one ({what}): {}",
            value.min_y - name.max_y
        );
    }
    // No top rail, nothing raised: the value keeps its gap off the body's own
    // edge, exactly as before.
    let nodes = laid(&scope(
        "",
        "  |component#u1| \"LM2596S\" [\n    |pin#a| { side: left; number: 7 }\n    |pin#b| { side: right; number: 1 }\n  ]\n",
    ));
    let (part, _, py) = placed(&nodes, "u1");
    let edge = py + part.bbox.min_y + part.attrs.half_stroke();
    let value = chrome(&nodes, "u1", "part-value");
    assert!(
        close(edge - value.max_y, consts::READOUT_GAP),
        "a side-pinned part's seat is untouched: {}",
        edge - value.max_y
    );
}

// ───────────────────────── the shaped tag ─────────────────────────

#[test]
fn a_shaped_tag_draws_its_flag_over_the_sized_label() {
    // [SPEC 16.4] `shape:` picks the outline, and the op's end marker sets it
    // [SPEC 16.5]. `plain` draws none; `round` is the label's own stadium; the
    // three flags lower as a chrome `|path|` the layout fills from the finished
    // box — so the point's *span* is the tag's, and its depth the constant.
    let tag_path = |op: &str| -> Option<String> {
        let src =
            format!("{{ layout: schematic }}\n|component#u1| [ |pin#a| ]\nu1.a {op} \"NET\"\n");
        let nodes = laid(&src);
        let (label, ..) = placed(&nodes, "lini-label-1");
        label.children.iter().find_map(|c| {
            (c.kind == crate::resolve::NodeKind::Path)
                .then(|| match c.attrs.get("path") {
                    Some(crate::resolve::ResolvedValue::String(d)) => Some(d.clone()),
                    _ => None,
                })
                .flatten()
        })
    };
    assert_eq!(tag_path("-"), None, "a plain tag is bare text");
    assert_eq!(tag_path("-*"), None, "a round tag is the label's own box");
    // A flag turns the box's corners into a point: five or six vertices, and
    // the tip sits on the box edge the padding already reserved.
    for (op, tips) in [("->", 1), ("-<", 1), ("-<>", 2)] {
        let d = tag_path(op).unwrap_or_else(|| panic!("{op} draws a flag"));
        // A rectangle is three drawn edges plus the closing one; each point
        // adds a vertex.
        assert_eq!(
            d.matches('L').count(),
            3 + tips,
            "{op} cuts {tips} end(s): {d}"
        );
        assert!(d.ends_with('Z'), "and closes: {d}");
    }
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
    assert!(
        close((x2 - x1) % SCH_GAP, 0.0),
        "a whole number of coarse cells apart: {}",
        x2 - x1
    );
}

#[test]
fn placement_does_not_cascade_into_a_nested_scope() {
    // [SPEC 16] the drawing precedent: a nested `|column|` places its own
    // children — they stack, while it rides the schematic's tracks as an
    // anchor. It is no part, so it seats nowhere *and* it does not inherit its
    // children's pin arity: a column holding a two-pin `|R|` is not a jumper.
    // Ids distinct from `anchor`'s own pins (a / b / c), which are declared
    // first and would otherwise be what the lookups find.
    let nodes = laid(&scope(
        "",
        &(anchor("u1", "") + "  |column#col| [\n    |box#top| \"a\"\n    |R#low| \"1k\"\n  ]\n"),
    ));
    let ((ax, ay), (bx, by)) = (at(&nodes, "top"), at(&nodes, "low"));
    assert!(close(ax, bx), "the column stacks: {ax} vs {bx}");
    assert!(by > ay, "in declaration order: {ay} vs {by}");
    let ((ux, _), (cx, _)) = (at(&nodes, "u1"), at(&nodes, "col"));
    assert!(cx > ux, "the column rides the scope's track row");
    // Beside the anchor, not below it: the two clusters share the row, so
    // their drawn extents overlap along y. (Their box centres need not line
    // up — a track seats clusters, and the anchor's carries pin stubs.)
    let (u, c) = (ink(&nodes, "u1"), ink(&nodes, "col"));
    assert!(
        u.min_y < c.max_y && c.min_y < u.max_y,
        "same row: {u:?} vs {c:?}"
    );
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
    let ((ux, uy), (cwx, cwy)) = (at(&nodes, "u1"), at(&nodes, "r"));
    assert!(cwx > ux, "the row rides the scope's track row: {ux} {cwx}");
    assert!(close(uy, cwy), "beside the anchor, not seated below it");
    let (wx, wy) = at(&nodes, "r");

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

#[test]
fn a_schematic_scope_honours_its_size_floor() {
    // [SPEC 17]'s matrix: schematic width/height are "✓ a floor".
    let nodes = laid(&scope(" { width: 600; height: 400 }", "  |R#r1| \"1k\"\n"));
    let (_, _, w, h) = body(&nodes, "s");
    assert!(w >= 600.0, "width floor: {w}");
    assert!(h >= 400.0, "height floor: {h}");
}

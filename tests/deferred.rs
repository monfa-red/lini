//! **The deferred-surface ledger** — one test per intentionally-unbuilt slot a
//! user can actually type, asserting today's refusal.
//!
//! The lens: anything that errors today stays a **free option** for post-1.0;
//! anything accidentally lenient becomes **frozen behaviour**. So every
//! [SPEC 24](../SPEC.md#24-deferred) item whose syntax is reachable in today's
//! grammar is pinned here as an error — never silently accepted, never silently
//! dropped. The sections and their order mirror SPEC 24, so a reviewer can diff
//! the two documents item by item; where a gate is new, the built form beside it
//! is asserted to still compile, so the gate can't be over-wide.
//!
//! **Messages are not the contract** — the diagnostic *codes* are ([SPEC 21]).
//! These snapshots exist so the refusal cannot silently become an acceptance;
//! improving a message is a snapshot update, and relaxing one of these tests is
//! exactly what shipping the deferred feature looks like.
//!
//! **Unreachable — no test.** SPEC 24 items whose syntax cannot be typed today,
//! so there is nothing to accept or reject:
//!
//! - *Core*: kerning-aware measurement (no surface — a measurement quality).
//! - *Tables*: arbitrary per-cell backgrounds — a `|cell|` child already takes a
//!   `fill:`, and a bare-text cell's `fill:` paints its glyphs [SPEC 6]; the
//!   deferred slot has no distinct spelling.
//! - *Sequences*: participant grouping, found / lost messages, create / destroy
//!   lifelines, explicit activation spans (`activation:` takes a mode, not a
//!   span).
//! - *Charts*: a centred donut total; per-segment styling (a segmented `fn:`
//!   list mirror — a paint list on a one-shape series already errors,
//!   `tests/validation.rs`).
//! - *Drawings*: repeated-segment counting (`4× R3` auto-prefixing); the ASME
//!   text-in-a-broken-line diametral form and its horizontal-text knob; balloon
//!   auto-numbering and auto-BOM; angled break lines.
//! - *Schematics*: wire-seating; crossing hop-over arcs; pin electrical marks;
//!   hierarchical sheets; netlist semantics; a mid-wire tag at an `along:`
//!   fraction (a link label at `along:` is built — the deferred part is the tag
//!   glyph, which has no spelling).
//! - *Beyond 1.0*: view-letter arrows on sheets; animation; native PNG / WebP
//!   export (a CLI surface, not a language one).

/// The refusal a source draws, rendered as the CLI prints it — the shared
/// `compile_verdict` read the other way round. One entry point, so the ledger
/// below reads as one table whatever phase owns the refusal, and a slot that
/// turns lenient fails here loudly rather than quietly passing.
#[track_caller]
fn refusal(src: &str) -> String {
    match lini::testing::compile_verdict(src, "test.lini") {
        Err(msg) => msg,
        Ok(()) => panic!("expected a refusal; the source compiled clean:\n{src}"),
    }
}

/// The built form beside a newly-gated one — the gate must refuse the deferred
/// slot and nothing else.
#[track_caller]
fn compiles(src: &str) {
    if let Err(msg) = lini::testing::compile_verdict(src, "test.lini") {
        panic!("expected a clean compile, got:\n{msg}");
    }
}

// ───────────────────────────────── Core ─────────────────────────────────

#[test]
fn a_standalone_hollow_circle_endpoint() {
    // The hollow ring lives only inside the ER cardinality glyphs: `o` is an
    // operator glyph only next to a max glyph [SPEC 23]…
    insta::assert_snapshot!(
        refusal("|box#a|\n|box#b|\na -o b\n"),
        @"test.lini:3:3: error: '-o' needs a max glyph — write '-o<', '-o+', or 'marker-end: circle'"
    );
    // …and there is no marker name for it either — `circle` is the larger
    // *filled* dot [SPEC 7].
    insta::assert_snapshot!(
        refusal("|box#a|\n|box#b|\na -> b { marker-end: ring }\n"),
        @"test.lini:3:1: error: invalid marker value 'ring'"
    );
    compiles("|box#a|\n|box#b|\na -> b { marker-end: circle }\n");
}

#[test]
fn gradient_fills_on_text() {
    // `color:` is the text colour of a whole subtree, so it takes a flat one…
    insta::assert_snapshot!(
        refusal("|box#a| \"hi\" { color: gradient(--red, --blue) }\n"),
        @"test.lini:1:16: error: 'color' takes a flat colour — a gradient fills a shape, and gradient-on-text is deferred"
    );
    // …and a text leaf's own paint is the same refusal.
    insta::assert_snapshot!(
        refusal("|box#a| [\n\"hi\" { fill: gradient(--red, --blue) }\n]\n"),
        @"test.lini:2:8: error: 'fill' takes a flat colour — a gradient fills a shape, and gradient-on-text is deferred"
    );
    // A shape's fill is the built case [SPEC 10.3].
    compiles("|box#a| \"hi\" { fill: gradient(--red, --blue) }\n");
}

#[test]
fn radius_on_a_non_rect_primitive() {
    // `radius` rounds a rect and a polyline's joins; hex / diamond / slant /
    // poly are deferred, so the value errors rather than being dropped.
    insta::assert_snapshot!(
        refusal("|hex#h| \"x\" { radius: 4 }\n"),
        @"test.lini:1:15: error: 'radius' rounds a rect or a polyline join — rounding a '|hex|' is deferred"
    );
    insta::assert_snapshot!(
        refusal("|poly#p| { points: 0 -30, 30 -6, -30 -6; radius: 4 }\n"),
        @"test.lini:1:42: error: 'radius' rounds a rect or a polyline join — rounding a '|poly|' is deferred"
    );
    compiles("|box#b| \"x\" { radius: 4 }\n");
    compiles("|line#l| { points: 0 0, 20 0, 20 20; radius: 4 }\n");
}

#[test]
fn arbitrary_numeric_font_weight() {
    // The metrics ship 400–700; an arbitrary 100–900 would measure at the
    // nearest built static while the emitted CSS asked for another.
    insta::assert_snapshot!(
        refusal("|box#a| \"x\" { font-weight: 250 }\n"),
        @"test.lini:1:15: error: 'font-weight' takes normal, medium, semibold, bold, or 400, 500, 600, 700"
    );
    compiles("|box#a| \"x\" { font-weight: 600 }\n");
    compiles("|box#a| \"x\" { font-weight: semibold }\n");
}

#[test]
fn a_solid_fill_weight_icon_variant() {
    // The built-in set is Phosphor duotone; there is no weight knob.
    insta::assert_snapshot!(
        refusal("|icon#i| { symbol: heart; weight: fill }\n"),
        @"test.lini:1:27: error: unknown property 'weight'; did you mean 'height'?"
    );
}

#[test]
fn aria_label() {
    insta::assert_snapshot!(
        refusal("|box#a| \"x\" { aria-label: \"a box\" }\n"),
        @"test.lini:1:15: error: unknown property 'aria-label'"
    );
}

// ─────────────────────────────── Sequences ───────────────────────────────

#[test]
fn the_deferred_sequence_fragments() {
    // `par` (with its `|and|` separator), `break`, `critical`, and `ref` — the
    // built fragments are `|loop|` / `|opt|` / `|alt|` [SPEC 13].
    for ty in ["par", "and", "break", "critical", "ref"] {
        let src = format!(
            "{{ layout: sequence }}\n|box#a| \"A\"\n|box#b| \"B\"\n|{ty}#f| [\na -> b \"x\"\n]\n"
        );
        assert_eq!(
            refusal(&src),
            format!("test.lini:4:1: error: unknown type '{ty}'"),
        );
    }
}

#[test]
fn an_actor_stick_figure_primitive() {
    // An actor is an `|icon|` today.
    insta::assert_snapshot!(
        refusal("{ layout: sequence }\n|actor#a| \"A\"\n|box#b| \"B\"\na -> b \"x\"\n"),
        @"test.lini:2:1: error: unknown type 'actor'"
    );
}

#[test]
fn dividers_and_delays() {
    // `==` / `...` are not statements — the parser reads a leading `=` / `.` as
    // a malformed node.
    insta::assert_snapshot!(
        refusal("{ layout: sequence }\n|box#a| \"A\"\n|box#b| \"B\"\na -> b \"x\"\n== \"phase two\"\n"),
        @"test.lini:5:1: error: a node leads with bars — write '|box#X|' (a bare name is a link endpoint)"
    );
}

#[test]
fn message_auto_numbering() {
    insta::assert_snapshot!(
        refusal("{ layout: sequence; numbering: auto }\n|box#a| \"A\"\n|box#b| \"B\"\na -> b \"m\"\n"),
        @"test.lini:1:21: error: unknown property 'numbering'"
    );
}

// ───────────────────────────────── Charts ─────────────────────────────────

#[test]
fn legend_placement_and_suppression() {
    // The auto legend (≥ 2 entries) is built; `top` / `right` / `bottom` /
    // `none` are the deferred reader, so the whole decl errors — writing one
    // today would freeze a no-op.
    insta::assert_snapshot!(
        refusal("|chart#c| { legend: bottom } [\n|bars| \"A\" { data: 1, 2 }\n]\n"),
        @"test.lini:1:13: error: 'legend' is named but not built yet — see SPEC 24"
    );
    insta::assert_snapshot!(
        refusal("|chart#c| { legend: none } [\n|bars| \"A\" { data: 1, 2 }\n]\n"),
        @"test.lini:1:13: error: 'legend' is named but not built yet — see SPEC 24"
    );
    compiles("|chart#c| [\n|bars| \"A\" { data: 1, 2 }\n|bars| \"B\" { data: 2, 3 }\n]\n");
}

#[test]
fn bands_and_marks_in_a_radial_chart() {
    // `column` and `row` are built; the radial flip is never silently lossy.
    insta::assert_snapshot!(
        refusal("|chart#c| { direction: radial } [\n|bars| { data: 1, 2 }\n|band| { range: 0 1; axis: x }\n]\n"),
        @"test.lini:3:1: error: a radial chart draws no bands / marks yet — remove it or change 'direction'"
    );
    insta::assert_snapshot!(
        refusal("|chart#c| { direction: radial } [\n|bars| { data: 1, 2 }\n|mark| { at: 1; axis: x }\n]\n"),
        @"test.lini:3:1: error: a radial chart draws no bands / marks yet — remove it or change 'direction'"
    );
}

#[test]
fn explicit_per_axis_tick_text() {
    // `categories:` covers the x axis; an axis carries no text of its own, so
    // the SPEC 21 "set 'categories' or an axis 'labels', not both" row stays
    // unreachable — this is the error a user actually meets.
    insta::assert_snapshot!(
        refusal("|chart#c| [\n|axis| { labels: \"a\", \"b\" }\n|bars| { data: 1, 2 }\n]\n"),
        @"test.lini:2:10: error: 'labels' has no meaning on '|axis|' — it reads on a chart series"
    );
}

#[test]
fn gauge_stacked_areas_and_the_radial_knobs() {
    // A gauge (a partial arc for one value) is not a type…
    insta::assert_snapshot!(
        refusal("|gauge#g| { data: 1 }\n"),
        @"test.lini:1:1: error: unknown type 'gauge'"
    );
    // …`bars: stacked` reaches `|bars|` alone, never `|area|`…
    insta::assert_snapshot!(
        refusal("|chart#c| [\n|area| { data: 1, 2; bars: stacked }\n]\n"),
        @"test.lini:2:22: error: 'bars' has no meaning on '|area|' — it reads on '|chart|'"
    );
    // …and the polar-area start angle / direction knob has no property.
    insta::assert_snapshot!(
        refusal("|chart#c| { direction: radial; start-angle: 90 } [\n|bars| { data: 1, 2 }\n]\n"),
        @"test.lini:1:32: error: unknown property 'start-angle'"
    );
}

#[test]
fn per_slice_explode_and_on_slice_labels() {
    insta::assert_snapshot!(
        refusal("|pie#p| [\n|slice| \"a\" { value: 1; explode: 10 }\n]\n"),
        @"test.lini:2:25: error: unknown property 'explode'"
    );
    insta::assert_snapshot!(
        refusal("|pie#p| [\n|slice| \"a\" { value: 1; labels: \"x\" }\n]\n"),
        @"test.lini:2:25: error: 'labels' has no meaning on '|slice|' — it reads on a chart series"
    );
}

#[test]
fn multi_ring_pie_and_sunburst() {
    // A slice is one wedge — a nested ring was silently dropped before.
    insta::assert_snapshot!(
        refusal("|pie#p| [\n|slice| \"a\" { value: 1 } [\n|slice| \"b\" { value: 1 }\n]\n]\n"),
        @"test.lini:2:1: error: a '|slice|' is one wedge — multi-ring pie / sunburst is deferred"
    );
    compiles("|pie#p| [\n|slice| \"a\" { value: 1 }\n|slice| \"b\" { value: 2 }\n]\n");
}

#[test]
fn mark_and_note_in_charts_at_data_coordinates() {
    // `at:` places a `|mark|` / `|bubble|` / `|plane|`; a `|note|` in a chart
    // has no data-coordinate placement.
    insta::assert_snapshot!(
        refusal("|chart#c| [\n|bars| { data: 1, 2 }\n|note#n| \"hi\" { at: 1 1 }\n]\n"),
        @"test.lini:3:17: error: 'at' has no meaning on '|note|' — it reads on '|mark|' / '|bubble|' / '|plane|' / a wall opening ('|door|' / '|window|')"
    );
}

// ──────────────────────────────── Drawings ────────────────────────────────

#[test]
fn per_kind_dimension_selectors() {
    // The family selector `(-)` reaches every dimension; `(o)` / `(<)` are
    // deferred, as is a leader-specific selector under `|-|`.
    let sketch = "|drawing#d| [\n|sketch#s| { draw: move(0,0) right(40) down(20) close() }\n]\n";
    insta::assert_snapshot!(
        refusal(&format!("{{ (o) {{ color: --red }} }}\n{sketch}")),
        @"test.lini:1:3: error: '(-)' selects every dimension — per-kind '(o)' / '(<)' selectors are deferred (SPEC 24)"
    );
    insta::assert_snapshot!(
        refusal(&format!("{{ (<) {{ color: --red }} }}\n{sketch}")),
        @"test.lini:1:3: error: '(-)' selects every dimension — per-kind '(o)' / '(<)' selectors are deferred (SPEC 24)"
    );
    compiles(&format!("{{ (-) {{ color: --red }} }}\n{sketch}"));
}

#[test]
fn explode_views() {
    insta::assert_snapshot!(
        refusal("|drawing#d| { explode: 2 } [\n|sketch#s| { draw: move(0,0) right(40) down(20) close() }\n]\n"),
        @"test.lini:1:15: error: unknown property 'explode'"
    );
}

#[test]
fn authored_segment_twins() {
    // A `mirror:` copy of a `:segment` is unaddressable — only a `pattern:`
    // copy carries an index.
    insta::assert_snapshot!(
        refusal("|drawing#d| { unit: mm } [\n|sketch#s| { draw: move(0,0) right(40):edge down(20) left(40) close(); mirror: y-axis }\ns.2:edge (-) s:edge\n]\n"),
        @"test.lini:3:1: error: 's' has no copies — a numeric segment picks a replication copy"
    );
}

#[test]
fn routed_links_to_authored_anchors() {
    // The fixed-port contract is built (schematic pins ride it); the flow /
    // grid surface syntax `a -> b:port` onto a sketch's `:segment`s is not.
    insta::assert_snapshot!(
        refusal("|box#a|\n|box#b|\na -> b:inlet\n"),
        @"test.lini:3:8: error: ':inlet' is not a side — use top, bottom, left, or right"
    );
}

#[test]
fn hole_variants() {
    // Threads are built (`thread:`); counterbore and countersink are not.
    insta::assert_snapshot!(
        refusal("|drawing#d| [\n|sketch#s| { draw: move(0,0) right(40) down(20) close(); counterbore: 10 3 }\n]\n"),
        @"test.lini:2:58: error: unknown property 'counterbore'"
    );
}

#[test]
fn deeper_sourced_view_nesting() {
    // A detail of a marker inside another detail / section.
    insta::assert_snapshot!(
        refusal(
            "|page#p| [\n|drawing#side| { unit: mm } [\n|sketch#body| { draw: move(-50,0) right(100) down(20) left(100) close() }\n|magnifier#c| \"C\" { width: 34 }\n]\n|drawing#det| { of: c; scale: 2 } [\n|magnifier#e| \"E\" { width: 10 }\n]\n|drawing#det2| { of: e; scale: 4 }\n]\n"
        ),
        @"test.lini:9:1: error: a detail magnifies a base view — 'of' can't name a marker inside another sourced view"
    );
}

#[test]
fn the_deferred_break_slots() {
    // A scope-level `break:` on the `|drawing|` itself…
    insta::assert_snapshot!(
        refusal("|drawing#d| { break: 10 20 } [\n|sketch#s| { draw: move(0,0) right(40) down(20) close() }\n]\n"),
        @"test.lini:1:15: error: 'break' has no meaning on '|drawing|' — it reads on '|sketch|'"
    );
    // …a station through a `curve()` (lines and arcs clip exactly)…
    insta::assert_snapshot!(
        refusal("|drawing#d| { unit: mm } [\n|sketch#s| { draw: move(-60,0) right(40) curve(10,0, 20,10, 30,10) right(40) down(20) left(110) close(); break: -10 10 }\n]\n"),
        @"test.lini:2:1: error: a 'break' can't cut a 'curve()' — move the stations"
    );
    // …and `break:` on non-sketch geometry.
    insta::assert_snapshot!(
        refusal("|drawing#d| [\n|rect#p| { width: 40; height: 20; break: 20 }\n]\n"),
        @"test.lini:2:35: error: 'break' has no meaning on '|rect|' — it reads on '|sketch|'"
    );
}

#[test]
fn an_ambient_w_h_bound_to_a_nodes_own_size() {
    // Circular against auto-sizing today — a named constant covers the workflow.
    insta::assert_snapshot!(
        refusal("|drawing#d| [\n|rect#p| { width: 40; height: 20 }\n|rect#q| { width: (w / 2); height: 10 }\n]\n"),
        @"test.lini:3:12: error: unknown name 'w' in an expression"
    );
}

// ─────────────────────────────── Schematics ───────────────────────────────

#[test]
fn an_ansi_symbol_standard_knob() {
    // IEC is the built-in; the scope-level swap has no property.
    insta::assert_snapshot!(
        refusal("|schematic#s| { standard: ansi } [\n|R#r1| \"1k\"\n]\n"),
        @"test.lini:1:17: error: unknown property 'standard'"
    );
}

#[test]
fn the_deferred_part_types() {
    // Logic gates, transformer (`T`), relay (`K`), motor (`M`), speaker (`LS`),
    // potentiometer (`RV`) — none is a type yet.
    for ty in ["T", "K", "M", "LS", "RV"] {
        let src = format!("|schematic#s| [\n|{ty}#p1| \"x\"\n]\n");
        assert_eq!(
            refusal(&src),
            format!("test.lini:2:1: error: unknown type '{ty}'"),
        );
    }
}

#[test]
fn buses() {
    insta::assert_snapshot!(
        refusal("|schematic#s| [\n|R#r1| \"1k\"\n|R#r2| \"2k\"\nr1 - r2 { bus: 4 }\n]\n"),
        @"test.lini:4:11: error: unknown property 'bus'; did you mean 'bars'?"
    );
}

// ────────────────────────────── Beyond 1.0 ──────────────────────────────

#[test]
fn automatic_graph_dag_layout() {
    insta::assert_snapshot!(
        refusal("{ layout: graph }\n|box#a|\na -> b\n"),
        @"test.lini:1:1: error: unknown layout 'graph' — expected flow, grid, tree, sequence, drawing, floorplan or schematic"
    );
}

#[test]
fn ring_radial_and_forest_trees() {
    // A tree grows column / row / bilateral — `radial` is a chart's word, and
    // reading it as the default would have been a silent drop.
    insta::assert_snapshot!(
        refusal("{ layout: tree; direction: radial }\n|topic#r| \"R\" [\n|topic#a| \"A\"\n]\n"),
        @"test.lini:1:1: error: unknown direction 'radial' — a tree grows column, row, or bilateral"
    );
    insta::assert_snapshot!(
        refusal("|mindmap#m| \"R\" { direction: radial } [\n|topic#a| \"A\"\n]\n"),
        @"test.lini:1:1: error: unknown direction 'radial' — a tree grows column, row, or bilateral"
    );
    // A forest (multi-root) tree is the one-root law [SPEC 12].
    insta::assert_snapshot!(
        refusal("|column#o| { layout: tree } [\n|topic#a| \"A\"\n|topic#b| \"B\"\n]\n"),
        @"test.lini:3:1: error: a tree has one root — '|topic|' 'B' is a second"
    );
    compiles("{ layout: tree; direction: bilateral }\n|topic#r| \"R\" [\n|topic#a| \"A\"\n]\n");
}

#[test]
fn imports_modules_and_namespaces() {
    insta::assert_snapshot!(
        refusal("@import \"other.lini\"\n|box#a|\n"),
        @"test.lini:1:1: error: unexpected character '@'"
    );
}

// ───────────────── Reserved syntax outside SPEC 24's list ─────────────────
//
// The pre-v1 review's own findings: surfaces the language reserves elsewhere
// whose leniency would freeze the same way.

#[test]
fn per_link_routing() {
    // `routing:` picks a **scope's** strategy — one scope, one strategy
    // [SPEC 11, ROUTING]. Per-link routing used to work by accident.
    insta::assert_snapshot!(
        refusal("|box#a|\n|box#b|\na -> b { routing: natural }\n"),
        @"test.lini:3:10: error: 'routing' is a scope's strategy — one scope, one strategy; set it on the container"
    );
    insta::assert_snapshot!(
        refusal("{ |-| { routing: natural } }\n|box#a|\n|box#b|\na -> b\n"),
        @"test.lini:1:9: error: 'routing' is a scope's strategy — one scope, one strategy; set it on the container"
    );
    // The scope's own declaration is the built form, and `clearance:` — which a
    // link really does carry [ROUTING] — is untouched.
    compiles("{ routing: natural }\n|box#a|\n|box#b|\na -> b\n");
    compiles("|box#a|\n|box#b|\na -> b { clearance: 30 }\n");
}

#[test]
fn a_flow_callout() {
    // A one-ended op toward a string is the drawing's leader [SPEC 15.7]; the
    // same shape in flow — the future callout — has no meaning…
    insta::assert_snapshot!(
        refusal("|box#a| \"A\"\na <- \"callout\"\n"),
        @"test.lini:2:1: error: link requires at least two endpoints"
    );
    // …nor in a schematic, whose wires are plain.
    insta::assert_snapshot!(
        refusal("|schematic#s| [\n|R#r1| \"1k\"\nr1 <- \"note\"\n]\n"),
        @"test.lini:3:1: error: a schematic wire is plain — markers shape a text label's tag; write 'a -> \"NET\"'"
    );
}

#[test]
fn a_balloon_capsule_leader_in_a_drawing() {
    // A drawing never invents an endpoint, so an inline capsule is refused —
    // the balloon-capsule leader stays free.
    insta::assert_snapshot!(
        refusal("|drawing#d| [\n|rect#p| { width: 40; height: 20 }\np -> |note| \"x\"\n]\n"),
        @"test.lini:3:6: error: a drawing never invents an endpoint — declare the node, then annotate it"
    );
}

#[test]
fn a_percentage_outside_a_colour_component() {
    // A trailing `%` is a colour component and nothing else [SPEC 2]; bare in
    // any other slot it used to flow through to the output unread.
    insta::assert_snapshot!(
        refusal("|box#a| { width: 50% }\n"),
        @"test.lini:1:11: error: 'width' takes a number — a '%' is a colour component"
    );
    insta::assert_snapshot!(
        refusal("|box#a| { opacity: 50% }\n"),
        @"test.lini:1:11: error: 'opacity' takes a number — a '%' is a colour component"
    );
    compiles("|box#a| { fill: hsl(0, 100%, 50%) }\n");
}

#[test]
fn fr_like_grid_tracks() {
    // There is no `fr` unit [SPEC 12] — a track is a size, `auto`, or
    // `repeat(N[, size])`, and equal tracks are `repeat(N)`.
    insta::assert_snapshot!(
        refusal("{ layout: grid; columns: 1fr, 2fr }\n|box#a|\n|box#b|\n"),
        @"test.lini:1:17: error: 'columns' takes comma-separated values — 'columns: 80, 140, auto'"
    );
    compiles("{ layout: grid; columns: repeat(2) }\n|box#a|\n|box#b|\n");
}

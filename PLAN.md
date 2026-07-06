# PLAN — `layout: drawing` (SPEC 15)

The implementation plan for engineering drawings, against **SPEC.md as of commit
`520e49c`** (drawings are SPEC 15; Part III is 16–24). SPEC 15 is the contract; this
file is the build order, the decisions that don't fit in a spec, and the **execution
log**. Every session that works on this plan reads this file first and appends to the
log — do not rely on chat memory for anything.

**Quality bar (non-negotiable):** modular, modern, maintainable, human-readable Rust.
No `unsafe`, no hacks, no patch layers, no repair passes. If an earlier stage's
decision proves wrong, fix it **at its source** and record why in the Execution log.
One mechanism per problem; no parallel implementations — shared logic is factored and
called from both sides. Split any module past ~500 LOC. `insta` snapshots for
output-shaped code; one sample per feature in `samples/`; verify SVG **visually**
(render to PNG with `resvg` and look at it). Before any push: `cargo fmt`,
`cargo test`, `cargo clippy`. Never add "Co-Authored-By" to commits.

---

## Architecture map (where things live today)

The pipeline: `lexer::lex` → `syntax::parser::parse` → `desugar::desugar` →
`resolve::resolve` (→ `resolve::Program`, `resolve/ir.rs`) → `layout::layout`
(→ `LaidOut`, `layout/ir.rs`) → `routing::route` → `render::render`.

| Concern | Where |
|---|---|
| Tokens & sigils | `src/lexer.rs` (one `match` in `Lexer::run`, ~line 72); link ops pre-composed into `LinkOp`/`LinkMarker`/`LineStyle` in `src/ast.rs` |
| Statement dispatch | `syntax/parser.rs` — `classify_setup` ~165, `classify_body` ~198; links `parse_link` ~769, `parse_endpoint` ~819 |
| Type registration | `desugar/types.rs` (`TEMPLATES` ~16); defaults as parser-shaped `Decl` bundles in `desugar/bundles.rs` (`primitive_bundle` ~48, `template_bundle` ~105, `root_defaults` ~274, `link_defaults` ~286) |
| Value resolution | `resolve/value.rs` — builder calls kept structured via `is_builder` ~22, every other call folded to a number via `fold_call` ~56; string-valued props `is_string_valued` ~118 |
| Cascade | `resolve/cascade.rs` (`node_layers` ~94, `selector_matches` ~142) — descendant rules already work |
| Layout dispatch | `layout/mod.rs` — per-node interception in `layout_inst` ~175 (`chart::is_chart` → …), root case ~40; **no trait** — engines are free functions returning `PlacedNode` |
| Scope-owns-links | `sequence::is_sequence_scope` (`layout/sequence/mod.rs` ~103) — routing and `testing::declared_edges` skip those links; drawing mirrors this |
| Scope-only validation | `sequence/mod.rs` `SEQ_PROPS` ~312, `validate` ~327 — the model for the drawing's gates |
| Primitive builders | `layout/prim.rs` — `rect` `oval` `marker` `poly` `wedge` `line` `path` (raw `d` + caller bbox) `group` `text*` `outline` `round` `set_title` |
| Sizing | `layout/primitives.rs` (`leaf_bbox`, `closed_bbox`); path extents `layout/path_bbox.rs` |
| Markers | `render/markers.rs`, `MarkerKind` in `resolve/ir.rs` ~249 — gains `Datum`; the drafting-slender dim arrow is drawn by the drawing's own lowering, not a core marker |
| Expressions | `src/expr.rs` — `Value::Number/Point`, locals, ambients `u`/`x`, `math_arity` ~624; user functions via `FuncTable` |
| Chart model (the analog) | `layout/chart/model.rs` — one `build()` parses children into a typed struct; per-kind geometry in sibling modules; copy this shape |

Closest structural precedent for the whole feature: **sequence** (scope owns links,
engine consumes them) + **chart** (typed model built from children, lowered at baked
coordinates through `prim::*`).

## Decisions ledger (settled in design review — do not relitigate)

1. **Ops.** `(-)` round measure and `(<)` angle are **glued lexer tokens**; `(>)` is a
   reserved error ("the angle op is '(<)'"). There is **no** `(-` radius op and **no**
   binary `(-)`. `||` mate is **not** a lexer token: the parser recognizes two
   **adjacent** `Pipe` tokens at operator position (after endpoints), so bars stay
   paired everywhere else; a glued `|box||cell|` selector becomes an error asking for
   a space.
2. **Call-glue rule.** In the lexer, `(` **glued to a preceding ident** (no
   whitespace) opens a call; otherwise `(-)` / `(<)` may lex as ops. `move(-2, 5)`
   keeps lexing as today (signed-number arm beats the link-op arm). A **spaced**
   `ident (` becomes the error "a call's '(' glues to its name".
3. **`(-)` readings** (SPEC 15.6): named arc → `R` leader; round-by-construction node
   bare → `⌀` leader (*amended 2026-07-05 by Abbas*: the line runs **across the
   diameter**, both arrows pressing the rims from outside, overshooting the far
   one — a single-arrow tip read as a word leader); round node + side/corner anchor → **diametral line** (text
   inside if it fits, else the line overruns the *anchored* rim and carries the text);
   any node + side anchor → span to the opposite side, ⌀-formatted, stacked; mirrored
   `:segment` → station span across the axis, stacked; bare with no inferable axis →
   error. Roundness is **by construction only** (`|oval|` lineage, `circle()` product,
   `|pitch-circle|`) — never geometric detection.
4. **One-ended relaxation**: RHS endpoints omissible for `<-` `*-` `>-` `(-)` `(<)`;
   one token of lookahead (ident → endpoint; string/`.`/`{`/`[`/EOS → tail). `<->` and
   `||` require both ends. Any **two-ended** core op in a drawing = straight
   annotation line, markers per op. One-ended `->` / `-*` → "a leader points back at
   its feature".
5. **Anchors**: geometry-bbox points; corners vertical-word-first (`:top-right`;
   reversed → did-you-mean); corners + `:center` drawing-scope only; authored `:segment`s
   from the pen (attached = product incl. its direction/radius; freestanding = pen
   point; duplicates error; built-ins win). Leader tips ray-cast onto the drawn path;
   dim extension lines spring from the anchor points exactly.
6. **Pen** (SPEC 15.3): y-down frame, visual verbs, bearings 0=up CW; heading state;
   even-odd subpaths; `fillet`/`chamfer` trim both legs and are **cyclic through
   `close()`** (allowed on either side of it); `chamfer(c)` cuts `c` back along each
   leg. `circle(r)` leaves point+heading unchanged.
7. **`mirror:`** list, left-to-right, each item reflecting the union so far; per
   subpath: open → **fused** (seam on the axis), closed → **duplicated**; fused
   generates the axis `|centerline|` chrome. Runs before `pattern:` and placement.
8. **`break: a b [x-axis|y-axis]`**, comma groups; every group defaults to the
   node's **longer axis**; `a < b` else error; stations are coordinates in the node's
   own frame. View-only compression: far piece slides toward the near piece leaving
   sheet-space `break-gap`; generated `|breakline|` chrome (*amended 2026-07-05 by
   Abbas*: **one convention** — the thin line with the sharp mid-jog; the round-stock
   S was dropped); the break is a **black hole for position** — features,
   sub-features, and pattern copies in the broken frame all ride the map (*amended
   2026-07-05, the barrel bug*); **measured values always read the unbroken model**
   (displayed positions come from a piecewise offset map).
9. **`scale:`** is a per-node property (px per drawing unit, default 1,
   nearest-ancestor-wins; *amended 2026-07-05 by Abbas*: a `|drawing|` and a root
   `{ layout: drawing }` default to **4** — ≈1 mm per unit at screen resolution): a node's **position** (`translate:`) scales by its
   *parent's* effective scale, its **own shape** (`draw:`/`points:`/`width`/`height`/
   `pattern:` offsets) by its *own*. Text, `stroke-width`, markers, hatch pitch, and
   all dim/leader constants **never** scale. `|note|`/`|balloon|`/`|table|` templates
   carry `scale: 1`. Dims report **pre-scale** drawing units.
10. **Mates**: resolve after datum placement, walking from the **ground** (first
    declared geometry child); move the ungrounded side (whole scope-level child,
    rigid); islands ground locally; both-grounded → over-constraint error. Directed
    anchors must be parallel; `gap:` along the normal, may be negative; point anchors
    coincide, `gap:` errors. Rotate-then-mate; child `translate:` applies after.
11. **Dims**: measured after mates, on unbroken geometry; ≤ 2 decimals, trailing zeros
    trimmed; text composition op-glyph + number/label + `tol:` + `pattern:` count +
    `unit:` (two-ended label **replaces** the number; one-ended label **follows** it).
    Row packing per side (`dim-pitch`/`dim-offset`, innermost free row, source order);
    ISO **aligned** text above the line; drafting-slender arrows ≈3:1 sized by the
    dim's stroke-width; narrow spans flip arrows outside. Geometry `stroke-width` 2,
    drawing links 1 (*amended 2026-07-05*: a **scope-level link default** in the
    base layer, not a built-in `|drawing| |-|` rule — the rule form outranked and
    swallowed a user's plain `|-| { stroke-width: … }`, which must win).
12. **Leaders**: text auto-places outward past the geometry (`note-offset`) with a
    horizontal **landing** (`note-landing`) elbow; `side:` picks a direction; `>-`
    lowers to the new `datum` marker; callout text is an unboxed leaf.
13. **Core promotions**: `|note|` core template (padding 20, `scale: 1`), compacted by
    **built-in scoped rules** `|sequence| |note|` / `|drawing| |note|`
    `{ padding: 6 10; font-size: 13 }`; `hatch()` core fill function (one deduped
    `<defs>` pattern per distinct hatch, fixed 0.75 line width); `stroke-style:
    center`/`phantom` global on shapes and `|line|`s; zero-arg functions read **bare
    inside fences** (lookup order: locals → `u`/`x` → `pi`/`e` → named constants;
    recursion is the existing **static cycle check**'s job at function-table build —
    one mechanism, no runtime depth guard) — outside fences the call form `w()` stands.
14. **No-roles**: `routing`/`clearance`/`along` and container `gap`/`align`/`justify`
    ignored in a drawing; `pin:` on a mated child warns; auto-create **never** happens
    in a drawing scope.
15. **Errors**: the full table is SPEC 20 "Layout — drawing" — implement messages
    verbatim from there.
16. Names authored in a `draw:` are user vocabulary — samples use obviously-generic
    names.

## Stages

Each stage ends green: `cargo fmt` + `cargo test` + `cargo clippy` clean, snapshots
reviewed, new samples rendered to PNG via `resvg` and **looked at**. Commit per stage
(one purposeful commit each, message says what and why). Append to the Execution log:
what landed, deviations from this plan and their reasons, and anything the next
session must know. The stages are sized for one `/compact`-able session each.

### Stage 1 — Language surface & core plumbing  ← DONE (see the Execution log)

Everything the rest builds on; after this stage the whole drawing vocabulary parses,
desugars, resolves, and **errors correctly outside a drawing scope** — but draws
nothing new except what's listed.

- **Lexer** (`lexer.rs`): track ident-adjacency for `(`; lex free-standing `(-)` /
  `(<)` as new tokens; `(>)` at op position → reserved error; spaced `ident (` →
  call-glue error. New `TokKind::DrawOp(DrawOpKind { RoundMeasure, Angle })` (naming
  free to improve). **No `||` token.**
- **AST + parser** (`ast.rs`, `syntax/parser.rs`, `syntax/ast.rs`): link statements
  accept `op = link_op | draw_op | mate` where mate = two adjacent `Pipe`s at operator
  position; one-ended links (RHS `None`) per decision 4 with the lookahead rule;
  endpoint `:point` widens from the `Side` enum to side-or-name (corners are single
  idents — `top-left` already lexes as one ident); `pen_item` value forms, parsed
  only when the declaration is `draw:` (a property-scoped flag keeps runaway-decl
  diagnostics sharp elsewhere); **call-argument space groups** so
  `hatch(45 -45, 6)` parses (`Value::Group`, one slot holding several values).
- **Desugar** (`desugar/types.rs`, `desugar/bundles.rs`): register `drawing`,
  `sketch`, `hole`, `centerline`, `pitch-circle`, `balloon`, `breakline`; template
  bundles per SPEC 8 (incl. `scale: 1` on `note`/`balloon`/`table`); `|note|` loses
  its sequence-only status; add the **built-in scoped rules** mechanism (a small list
  of engine-supplied descendant rules at the lowest cascade tier — the
  `|sequence| |note|` / `|drawing| |note|` compaction) — check whether a precedent
  exists before inventing one.
- **Resolve** (`resolve/value.rs`, `resolve/program.rs`, `resolve/links.rs`): keep
  `draw:` pen calls structured (a new structured-value class beside `is_builder` —
  pen calls fold to *pen items*, never to numbers); `pattern:`/`hatch()` values kept
  structured likewise; a `drawing`-scope detector (`layout: drawing` attr) and the
  **validation gates**: drawing ops/`tol:`/corner anchors/`:segment` endpoints outside a
  drawing scope error with the SPEC 20 messages; mates/dims never auto-create; the
  sequence `|note|` gate reworded per SPEC 20.
- **Expr** (`expr.rs`): bare zero-arg-function constants with the decision-13 lookup
  order and a recursion cap.
- **Tests**: lexer unit tests (`(-)` vs `move(-2,5)`, `(<)`, `(>)` error, call-glue
  error, adjacency); parser tests (one-ended forms, mate from adjacent pipes, spaced
  `|box| |cell|` still a selector, glued `|box||cell|` error, pen items in `draw:`);
  desugar snapshots (new templates); resolve error tests (each gate, verbatim
  messages); expr tests (bare constants, shadowing order, recursion cap).

**Not in stage 1**: no geometry, no layout dispatch, no rendering change. A
`layout: drawing` file should *parse and resolve* but may error "not yet built" at
layout — a single clearly-marked `todo`-style compile error, not a panic.

### Stage 2 — The sketch pen (geometry, any layout)  ← DONE (see the Execution log)

`|sketch|` becomes a real primitive usable in flow/grid too — nothing drawing-scoped
yet.

- New module family `src/layout/drawing/` (e.g. `pen.rs`, `geometry.rs`): fold
  `draw:` items to a segment list (lines, arcs, cubics, circle subpaths) with heading
  state; apply `fillet`/`chamfer` (incl. cyclic through `close()`); collect authored
  names — points, edges (with direction), arcs (with radius); `mirror:` fuse/duplicate
  per decision 7 (record the fused-axis for chrome + the mirrored-segment unary
  readings); emit an SVG `d` via `prim::path` with exact bbox (extend
  `path_bbox` if arcs need it).
- `NodeKind::Sketch` lowering (the kind is already plumbed; stage 1 errors at
  `leaf_bbox`). Two genuinely new render behaviours: the sketch's `|path|` must
  carry its **own paint** (`prim::path` hardcodes `stroke: none` — built for chart
  fills) and emit **`fill-rule="evenodd"`** for inner-subpath holes (no fill-rule
  support exists anywhere in src/render yet).
- `stroke-style: center` / `phantom` dash arrays, global (render layer; links keep
  their set). `|centerline|` / `|pitch-circle|` work as plain styled primitives.
- The `|note|` folded-corner silhouette becomes the **core** look (today only the
  sequence engine folds it via `notes::sticky` — factor that into the shared
  geometry path, one mechanism).
- **Samples + snapshots**: a profile with every call; fillet/chamfer corners incl.
  seam; mirror fused vs duplicated; a sketch inside a plain flow diagram; center/
  phantom styles. Render each to PNG and inspect.

### Stage 3 — The drawing engine (placement, mates, scale)  ← DONE (see the Execution log)

- `layout/mod.rs`: `is_drawing` interception + root case; `is_drawing_scope` link
  skip (mirror `is_sequence_scope` — routing and `declared_edges` both honour it).
- `scale:` and `pattern:` as **global node transforms** ([SPEC 15]'s global
  column) — wired here, once, alongside the drawing view-scale cascade (moved
  from stage 2: one scale context, one mechanism). A scaled / patterned sketch
  in a flow diagram must work, not just in a drawing.
- A typed model à la `chart::build`: partition children into geometry / annotation
  links / mates / sheet content; datum placement (origins, `translate:`), features in
  `[ ]` rigid with the part; nested drawings as rigid bodies; per-node effective
  `scale:` (decision 9 — position by parent, shape by self); title → `|footnote|`.
- `|hole|` punch + centre-mark chrome; `pattern:` grid/radial expansion (+
  `|pitch-circle|` chrome, bbox union, seed/centre datums).
- Mates: ground walk, directed/point, `gap:` (negative ok), rotate-then-mate,
  translate-after, all the SPEC 20 mate errors.
- Sizing: drawing bbox = paint bboxes ∪ annotations + padding (annotations come in
  stage 4 — leave the seam).
- Samples: concentric primitives, plate + patterned holes, a two-part mate with
  negative gap, a nested-drawing assembly. PNG-inspect each.

### Stage 4 — Annotations (dims, leaders, notes)  ← DONE (see the Execution log)

The largest stage; if it must split, split **measure/compose** from **place/pack**.
(Stage 3 is nearly as heavy — its natural split is **placement/features** first,
**mates** second.)

- Anchor resolution against seated geometry (sides, corners, `:center`, authored
  names, dot-paths, pattern datums); representative points + directions.
- `<->` linear dims + chains; `(-)` all five readings (decision 3) incl. the
  diametral fits-inside rule; `(<)` binary/unary; auto-measure (pre-scale, unbroken,
  ≤2 dp trimmed); text composition incl. `tol:` (three forms, stacked deviations at
  0.7×), `unit:`, `pattern:` count, label seat rules.
- Placement: side defaulting, row packing, `gap:` pin, extension lines
  (gap/overshoot), slender ≈3:1 arrows, narrow-span flip, ISO-aligned text.
- Leaders: ray-cast tips, outward text + landing elbow, `side:` directions, `datum`
  marker (`MarkerKind::Datum` + `render/markers.rs`), straight annotation arrows for
  two-ended ops, `|note|`/`|balloon|` wiring, drawing `|-|` stroke-width 1 default.
- Annotations paint above geometry; `layer:` still wins; drawing bbox now includes
  them.
- Samples: the SPEC 24 tie bar (minus `break:` until stage 5), bushing section
  (hatch arrives stage 5 — use flat fills), a fully-dimensioned plate, a leader/datum
  sheet. Snapshot + PNG-inspect. Cross-check every measured value by hand.

### Stage 5 — Conventions, break, fmt & finish  ← DONE (see the Execution log)

- `hatch()`: `<defs>` `<pattern>` emission, dedup, theming/bake parity with
  gradients.  ← DONE (see the Execution log)
- `break:`: station validation, longer-axis default, clipping the folded path at the
  cut stations, the piecewise view-offset map (annotations read displayed positions,
  values read the model), `|breakline|` zigzag + S-break chrome, `break-gap`.
- `fmt`: canonical `draw:` layout (break before each `move()`, wrap between calls at
  the column limit, continuations indented); mate/dim statements format like links.
- Samples: the three SPEC 24 examples byte-for-byte as written there; the barrel from
  DRAWING_OLD §17 as a stress sample. Visual pass over **all** drawing samples.
- Flip the SPEC 16 ledger ⌛ marks for the drawing column to ✓ in one sweep; delete
  `DRAWING_OLD.md`; update `README.md` if it lists layouts.

---

## Execution log

Append-only. Every working session adds: date, stage, what landed (commits), what
deviated from this plan and **why**, open threads for the next session.

- **2026-07-04 — plan written.** SPEC 15 landed in `520e49c` (spec rewrite; Part III
  renumbered 15–23 → 16–24, code doc-comments rewired). Design review + all decisions
  in the ledger above were settled in the same session with Abbas.

- **2026-07-04 — stage 1 landed** (same session; all suites green, clippy + fmt
  clean; both the spec and this plan were audited by independent agents and the
  findings folded in). What shipped, and the deviations a later stage must know:
  - Lexer: `TokKind::DrawOp` (`(-)` / `(<)`, exact three-char, free-standing per
    the call-glue rule — prev byte ident-continue ⇒ call-open); `(>)` reserved
    error. `move(-90, 0)` lexes untouched (signed-number arm outruns ops).
  - Parser: `ChainOp { Wire(LinkOp), Measure(DrawOp), Mate }` on `Link.op`;
    `||` recognised **only at operator position** from two adjacent `Pipe`s —
    glued selectors like `|box||cell|` therefore **stay legal**, and SPEC 21 was
    corrected to say so. One-ended chains = `chain.len() == 1`; op printed after
    the single group by `fmt`. `Endpoint.side: Option<Side>` became
    `point: Option<PointRef>` (raw name + span; resolve validates per scope).
    Pen items parse only under `draw:`; call args accept space groups
    (`Value::Group`).
  - Resolve: `resolve_link` takes `drawing_scope` (from `scope_is_drawing`,
    program.rs — immediate container / root `layout`); `validate_statement` in
    links.rs holds every statement-shape gate (ops-outside-drawing, mate
    label/one-ended, `(-)` unary-only, leader shapes, `tol:`), messages verbatim
    from SPEC 20. `ResolvedLink.kind: LinkKind` (router filters non-wires in
    request.rs); `ResolvedEndpoint.point` carries `#[expect(dead_code)]` until
    stage 4 reads it (the expect will self-report then — remove it).
    `resolve_property` keeps `draw:` (PenCall/PenPoint) and `pattern:`
    structured; `hatch` joined `is_builder`. Endpoint-not-found noun is
    "dimension"/"mate" in a drawing scope (never auto-created).
  - Desugar: templates registered (`drawing`, `hole`, `centerline`,
    `pitch-circle`, `balloon`, `breakline`; `sketch` is `NodeKind::Sketch`);
    `|note|` de-gated from sequences (core, padding 20, `scale: 1`); auto-create
    skipped for drawing scopes at both sites (root via `root_layout`, bodies via
    `is_drawing_body` — chain- or instance-decl-based; a container made a drawing
    by a bare **element rule** is not seen there, the same class-based limitation
    frame detection has — resolve's gates still hold; accepted edge).
  - Cascade: built-in scoped rules `|sequence| |note|` / `|drawing| |note|`
    `{ padding: 6 10; font-size: 13 }` are prepended in `Stylesheet::build`'s
    input (program.rs `scoped_rules`), and `resolve_instances` seeds the ancestor
    chain with synthetic root facts (`.lini-sequence` / `.lini-drawing`) for a
    root `{ layout: … }` — "the file is the root container". One snapshot shifted
    (sequence.lini: `font-size: 13px` moved from the `.lini-note` class rule to
    the note's inline diff; geometry identical — accepted).
  - Expr: **no runtime recursion guard** — planned "capped like define depth" was
    dropped after tests showed the static cycle check at function-table build
    already rejects every cycle (bare or called); one mechanism. Bare zero-arg
    constants already worked (`eval_var`); tests added.
  - Layout: `layout: drawing` (root and node) errors
    "'layout: drawing' is not built yet (PLAN.md stage 3)"; `|sketch|` errors at
    `leaf_bbox` ("stage 2") after its `draw:`-required check. A flow-scope
    `|note|` renders as a plain card until stage 2 folds the corner.
  - Spec audit fixes applied alongside: `|breakline|` template row, SPEC 6 stroke
    prose (+`center`/`phantom`), `(-)` unary-only statement + its SPEC 20 error,
    container-`gap` error scoped (`a container's 'gap' must be ≥ 0` — code
    message updated to match).

- **2026-07-05 — stage 2 landed** (same session; 14 suites green, clippy + fmt
  clean; `samples/sketch.lini` rendered to PNG via resvg and visually verified —
  chamfer/fillet corners, punched even-odd bore, fused pin, duplicated ears,
  tangent-arc hook, center/phantom dashes, folded note all read correctly).
  What shipped, and what a later stage must know:
  - `src/layout/drawing/{pen,geometry}.rs`: `pen::fold(inst) -> Folded { d,
    geometry (stroke-excluded bbox), names, mirror_axes, fused }`. Segments are
    Line/Arc/Cubic chains per subpath; bbox comes from the folded `d` through
    the existing `path_bbox::extent_points` (one mechanism). Bearings: the four
    cardinals are **exact** vectors, not sin/cos — their coordinates feed
    measured values in stage 4.
  - Corner modifiers: pending-slot model; cyclic through `close()` both ways
    (`fillet(3) close()` = last-to-seam, `close() fillet(3)` = seam-to-first).
    **Straight runs only** for now — a modifier against an arc errors
    "'fillet' joins two straight segments today" (added to SPEC 23 Deferred).
    The closing seam is a real segment (cyclic corners need it); `to_d` skips
    emitting a redundant trailing `L` that `Z` draws.
  - After `close()`, only `fillet`/`chamfer`/`circle`/`move`/`:segment` may follow
    ("after close(), start the next subpath with move()"). The tangent
    `arc(r, deg)` requires a heading; `circle(r)` appends its own closed
    subpath without disturbing the pen.
  - Duplicate `:segment` message is "':x' is already named in this 'draw:'" — pen
    items carry no spans, so SPEC 20's old "at L:C" form was amended to match.
  - `mirror:` fuse/duplicate per subpath as spec'd; reflection is applied
    chain-order (continuity preserved), reverse-then-reflect keeps an arc's
    sweep; axis-degenerate seams are skipped. `Folded.mirror_axes` / `fused`
    carry `#[expect(dead_code)]` until stages 3-4 consume them.
  - Sketch integration: `layout_inst` intercepts `NodeKind::Sketch` (geometry
    decides the bbox — never content+padding; children still arrange over it),
    stores the folded `d` as the placed node's `path` attr;
    `primitives::leaf_bbox` delegates to the same fold for any other caller.
    Render: `emit_sketch` = `<path d fill-rule="evenodd"/>`, paint riding the
    `<g>` like every closed primitive.
  - `|note|` fold moved to `layout/note.rs::fold`, applied once by the generic
    arranger (kind Block + type chain "note"); the sequence engine now only
    places the card — its snapshot stayed byte-identical, which validates the
    move.
  - `dash_pattern` gained `center` (long,gap,dot,gap) and `phantom`
    (long,gap,dot,gap,dot,gap), stroke-proportional like `dashed`; links
    reject both at resolve ("a link's stroke-style is solid, dashed, dotted,
    or wavy").
  - **Deviation:** generic `scale:`/`pattern:` wiring moved from stage 2 to
    stage 3 — they need the same effective-scale context the drawing engine
    builds, so wiring them once there is the one-mechanism play (stage 3
    bullet updated).

- **2026-07-05 — stage 3 landed** (same session as stage 2's log entry, after
  a `/compact`; all 14 suites green — 678 tests — clippy silent, fmt clean;
  four new samples PNG-rendered via resvg and visually verified: concentric
  washer at 3:1 with sheet-space centre marks, plate + grid/radial patterns
  with per-copy marks and the pitch circle, pin seated 20 deep in a socket
  (negative gap, `--bg` punch, chamfered nose), motor↔pump nested-drawing
  assembly). What shipped, and what stages 4–5 must know:
  - **One `Ctx { scale, drawing }` threads `layout_inst`** — the generic
    `scale:` transform and the drawing scope ride a single context; there is
    no separate part-layout path. `effective_scale` (nearest-ancestor, `> 0`
    gated) lives in `layout/mod.rs`; declared `width`/`height`/`points:` and
    the pen fold scale by the node's **own** scale (`leaf_bbox`/`closed_bbox`
    take a scale param; `pen::fold(inst, scale)` scales the folded output —
    per-call arg masking was rejected as pen knowledge outside the pen);
    `translate:` (and a container's declared content area) scale by the
    **parent's**, applied in `lay_out_container_children` and the engine's
    datum placement alike. Text/stroke/padding never touch the scale.
  - **Layout-owning engines are sheet-space**: chart/pie/sequence interiors
    call `layout_inst` with `Ctx::sheet()` and never read the inherited
    scale — like `|table|`'s `scale: 1`, without bundle churn. Accepted edge:
    a chart's own `width:` inside a scaled drawing does not scale.
  - **`pattern:` is a generic layout pass** (`layout/pattern.rs`, run from
    `layout_inst`): the placed node becomes an unpainted **carrier** (id +
    position props + inline `fill/stroke: none`) holding the copies — each a
    clone of the drawn body, children included, so a patterned `|hole|`
    centre-marks per copy for free. The `pattern` attr **stays on the
    carrier** — stage 4's `2×` count prefix reads it. Grid seed = copy one at
    the carrier origin; radial copies on the circle, none at the centre.
  - **Chrome is desugar-generated** (`desugar/drawing.rs`) so the cascade
    styles/removes it as SPEC 15.7 promises (`|sketch| |centerline| { stroke:
    none }` verified end-to-end): real `|centerline|` / `|pitch-circle|`
    children carrying a `chrome:` marker (`x-axis` / `y-axis` / bearing /
    `ring`) instead of geometry; `layout::drawing::chrome` fills axis lines
    from the parent's sized geometry (+ overhang 4), `pattern::expand` sizes
    and hoists the ring to pattern level. Fused-mirror detection at desugar is
    **syntactic openness** (no `close()` before the next `move()`/end) — the
    pen's `Folded.fused` is kept and a test asserts the two judgements agree.
    Chrome (and auto-create, and the drawing title) share the class-based
    `is_drawing_body` limitation — element-rule-made drawings aren't seen at
    desugar; accepted edge, resolve/layout still gate.
  - **The engine** (`layout/drawing/engine.rs`): children lay out via
    `layout_inst` under the drawing ctx (a **part** = `!owns_layout(attrs)` —
    no `layout:`/`direction:` — and not sheet content; its `[ ]` children
    datum-place as features, its bbox is its own shape via `part_bbox` —
    `|hole|`/`|pitch-circle|` are circles, ⌀ `width:` required). The engine
    then datum-places, consumes the scope's links (mates seat; anything else
    errors "a drawing's dimensions and leaders are not built yet (PLAN.md
    stage 4)" — **stage 4 replaces that gate**), takes the flow extent,
    recentres (node case), sizes border-box via `closed_bbox`, and pins sheet
    chrome (the title `|footnote|`, generated by desugar from the smart
    label) onto the finished box. Root case leaves scene coords; `finish`
    pads. `is_drawing_scope` skips the router + `declared_edges`.
  - **Anchors** (`layout/drawing/anchors.rs`, stage 4 extends): sides/corners/
    `center` on the geometry bbox (stroke-deflated), no-anchor = **origin**,
    authored segments from `PlacedNode.names` (a new field — the pen's products,
    scaled, carried on the placed sketch). Rotation accumulates through the
    dot-path walk, so rotate-then-mate is exact. **A named edge's outward
    normal is the left of the pen's travel** — the away-from-centre guess was
    replaced at its source when the socket sample flipped an interior
    shoulder; SPEC 15.5 now states the convention. Pattern-copy interiors are
    unaddressable ("per-copy features are deferred (SPEC 23)").
  - **Mates** (`layout/drawing/mates.rs`): source-ordered pair walk from the
    ground with island re-grounding (first-declared, deterministic); directed
    seats solve the **normal axis only** from the mover's datum-pure position
    (its `translate:` re-applies after — the SPEC law, so a translate along
    the normal deliberately offsets the seat; don't translate what you mate,
    see the assembly sample's fix); point mates coincide; `gap × scale` px.
    All SPEC 20 messages verbatim, incl. the over-constraint's
    "already positioned via 'a:right || b:left'" evidence.
  - **Resolve/lint**: `LinkScope { drawing, flow_in_drawing }` refines the
    mate gate to "a '|row|' places its own children — mates seat a drawing's"
    when a drawing encloses a flow scope; `lint_pinned_mates` warns
    "'pin' on 'X' is ignored — the mate seats it". `ResolvedEndpoint.point`'s
    stage-1 `#[expect(dead_code)]` is gone (mates read it).
  - No pre-existing snapshot moved — the Ctx/scale threading is a proven
    no-op outside drawings. Samples: `drawing.lini`, `drawing_pattern.lini`,
    `drawing_mate.lini`, `drawing_assembly.lini`.

- **2026-07-05 — stage-3 audit round** (same session; an independent opus
  audit + a hand probe pass over renders and edge cases; all gates green
  after). Fixed at the source:
  - **Rotated parts clipped at the canvas** — `accumulate_extent` ignored
    `rotate:`; it now swings bbox corners by the accumulated rotation (the
    one extent mechanism, so `finish` and the engine both benefit; a mated
    bar stood on end renders whole). No pre-existing snapshot moved.
  - **`pattern:` on a layout-owning node was a silent no-op** — the engine
    dispatch (chart/pie/sequence/drawing) returned before the pattern hook;
    the dispatch tail now expands too, and the `|note|` fold runs **before**
    expansion so a patterned note copies folded cards.
  - **The title gap scaled with the view** — `place_pinned` (and the generic
    container path) applied `translate × scale` to pinned overlays; a
    pin-relative nudge is chrome **anatomy** and stays sheet-space. SPEC 15.1's
    never-scales list now includes it. `drawing.lini`'s snapshot moved
    (title 99.5 → 65.5 at scale 3 — the fix, PNG-verified).
  - **Mates silently moved sheet content** — SPEC 15.5 seats *geometry*;
    now "a mate seats geometry — '|note|' is sheet content" (added to the
    SPEC 20 table).
  - Style: `pen.rs` split per the ~500-LOC law (`corner.rs` holds the
    fillet/chamfer trim; `Product` is the module-level pen↔anchors
    vocabulary; `rotate_about`/`arc_mid` joined `geometry.rs`); the
    scope-relative path helper is one `drawing::rel_path`; mate errors spell
    the pair as written (never fixed/mover order).
  - **Open question for Abbas (blocks nothing):** links inside an
    **anonymous** layout-owning container are mis-scoped — resolve's lifted
    prefix skips id-less nodes (scene.rs), so an anonymous `|drawing|`'s
    mates leak to the enclosing scope ("mate endpoint 'a' not placed" /
    "'||' belongs in a 'layout: drawing'"), and an anonymous `|sequence|`'s
    messages get **routed** instead of drawn (pre-existing core behavior,
    same root). Decide: give anonymous containers positional scope segments
    (core scoping surgery, resolve + layout agreeing), or require ids for
    link-owning bodies with a clean error.
  - **Stage-4 note:** a patterned node's **side/corner anchors** currently
    read the carrier's union bbox; SPEC 15.2 fixes only its *position*
    (grid → seed, radial → ring centre). Decide seed-vs-union for dims when
    anchoring `plate.pin:top` on a patterned hole.

- **2026-07-05 — stage 4 landed** (fresh session; all suites green — 555 lib
  tests + 4 new sample snapshots — clippy silent, fmt clean; every new sample
  PNG-rendered via resvg and inspected; every measured value cross-checked by
  hand: plate 150/70/25·125/50/2× ⌀10 H7/⌀6, tie bar 300/40/⌀20 h6, bushing
  ⌀16/⌀36/60, bracket 36.87° = atan(120/160), taper 28.07° = 2·atan(10/40)).
  What shipped, and what stage 5 must know:
  - **Modules** (`layout/drawing/`): `annotate.rs` (orchestrator: geometry
    extent, the row packer, per-link dispatch, the sheet constants),
    `dims.rs` (linear + the `(-)` readings + stacked lowering + diametral +
    the slender arrow), `angle.rs`, `leaders.rs` (leader skeleton, callouts,
    straight arrows, label leaves), `compose.rs` (glyph/number/label/tol/
    count/unit + `DimText::nodes` for ISO-rotated text with deviation
    stacks), `outline.rs` (ray-casting: sketch subpaths incl. arcs, ellipse,
    poly/line points, rect fallback, pattern-copy union; `exit_box` for
    leader text placement). The engine partitions links (mates seat first),
    lowers annotations against the seated kids, and **appends** them — later
    in source order = painted above geometry, `layer:` still wins, and the
    drawing bbox includes them for free.
  - **IR at the source**: `PlacedNode.names` became
    `sketch: Option<Arc<SketchGeo>>` — names **+ mirror axes + folded
    outline** (the pen keeps its subpaths; `arc_center` factored into
    `geometry.rs`, shared by `arc_to` and the ray-caster). `anchors.rs` was
    rebuilt around one `Anchor { child, node, origin, rot, spot }` with
    `point/outward/direction/round_diameter/mirrors/pattern_count`; mates
    consume the same model (their `Hit` is a projection of it).
  - **`<->` is typed at resolve**: `LinkKind::Measure(MeasureOp::Linear)`
    from the *operator* in a drawing scope (`MeasureOp { Linear, Round,
    Angle }` replaced the raw `DrawOp` payload) — an explicit `marker:` can
    restyle a wire but never re-type a statement.
  - **The built-in `|drawing| |-| { stroke-width: 1 }`** scoped rule (the
    drafting 2 : 1 contrast); `link_scope` now seeds the synthetic root fact
    (scene.rs `root_facts`, shared), so a **root** drawing's links match
    `|drawing| |-|` exactly like a `|drawing#x|`'s.
  - **`MarkerKind::Datum`** (parse "datum"): filled GD&T triangle, base on
    the feature, apex a full head back where the leader stops
    (`line_inset = marker_size`); a one-ended `>-`'s Crow lowers to it —
    only the leader lowering converts, a two-ended `>-` keeps core crow.
  - **Decisions taken here** (the spec was amended where noted):
    - `unit:` suffixes **linear** auto-values only — `300 mm` but `⌀20 h6`,
      matching every SPEC 24 comment (SPEC 15.1 now says so).
    - The stage-3 open question: a patterned node's anchors read **one
      copy's geometry about the pattern datum** — the copy is the feature,
      the pattern only places it (SPEC 15.2 sentence added). Mates on
      patterned nodes changed from union-bbox to the same rule.
    - SPEC 24 bushing dims **reordered** (bore first): the packing law
      (source order, innermost free row) contradicted the old "⌀16 stacks
      inside the ⌀36" comment; source order now teaches the control. The
      tie bar's thread leader gained `{ side: top }` — the default
      datum-ray grazes a long bar and lands on its end face.
    - Angle value = the drawn **wedge** at the (extended) intersection
      (`acos(u1·u2)`, ≤ 180°, legs toward the anchors) — supplement-proof
      where raw travel directions would flip; arc radius = min leg length
      clamped to [14, 40]; **parallel edges error** added to SPEC 20.
    - The off-axis `side:` message got its vertical twin ("a vertical
      dimension stacks on left or right" — SPEC 20 row extended).
    - Station / point↔point spans measure the **dominant-axis projection**
      (true aligned dims stay deferred, SPEC 23); `(-)` on an unmirrored
      name or a corner of a non-round node reuses the no-axis error.
    - `(-)` leaders (R / ⌀) tip with the **slender dim arrow** (dim
      anatomy); word callouts keep core markers. A leader whose feature
      sits on the datum has no outward ray — falls back to up-right.
    - Corner anchors both on one edge pull the dim to that side (the
      "anchors both on one edge" clause is corner-specific; side anchors
      set the axis instead, so they can never share a valid stack side).
    - Chain labels map to hops in order (`a <-> b <-> c [ "L1" "L2" ]`).
  - **Stage-5 notes**: leaders and diametral lines are not collision-packed
    against dim rows (deterministic; `side:` steers) — fine in practice,
    revisit only if the break samples collide. The angle arc draws no leg
    extension lines when its endpoints sit off the drawn edges. `break:`
    must feed annotations *displayed* anchor positions while values read
    the unbroken model — the seam is `annotate::Ctx` (one place to carry a
    view-offset map). SPEC 24's three examples must now land byte-for-byte
    **including this session's two edits** (bushing order, tie-bar `side:
    top`).

- **2026-07-05 — stage 5, first slice: `hatch()` landed** (same session as
  stage 4, directly after; all gates green — 705 tests — clippy silent, fmt
  clean; the bushing PNG-rendered and inspected: 45° section lines, exact
  ±45 cross-hatch probed separately, the `--bg` bore punching the hatch
  with no special case). What shipped:
  - `render/gradients.rs` → **`paints.rs`**: one post-layout walk interns
    both defs-backed paints (`Interner { gradients, hatches }`), rewriting
    use-sites to `url(#lini-gradient-N)` / `url(#lini-hatch-N)`;
    `HatchDef { angles, pitch, color }` rides `LaidOut.hatches` beside the
    gradients. `lower_gradients` is now `lower_paints`.
  - The tile: `pitch × pitch`, `patternTransform=rotate(first bearing)` —
    exact for **any** single bearing; a family at +90° from the first is
    the full-width line, so the standard cross-hatch tiles exactly at any
    bearing too. An oblique extra family (no shared tile period exists —
    `45 60` has none mathematically) draws through the tile centre, best
    effort. Lines mid-tile (a boundary line would lose half its stroke to
    tile clipping). Width fixed 0.75; colour via `format_value` — themes,
    flips, bakes exactly like gradient stops (live `style=`, baked attr).
  - Defaults: pitch 6, colour `--stroke`; forms `hatch(a)`, `hatch(a, p)`,
    `hatch(a, p, colour)`, `hatch(a b, p)`. A malformed first arg is not
    recognised and the call falls through to `fold_call`'s arity error —
    no bespoke message; acceptable.
  - The **fill-only gate** lives in `resolve_property` ("'hatch' is a
    fill — 'stroke' takes a colour or gradient", SPEC 20 verbatim) — any
    non-`fill` property, not just stroke.
  - `samples/drawing_bushing.lini` flipped from the flat-fill stopgap to
    `fill: hatch(45, 6)` — now the SPEC 24 example's paint.
- **2026-07-05 — review follow-ups from Abbas** (same session, after the hatch
  slice; all gates green — 705 tests — clippy silent, fmt clean; every drawing
  sample re-rendered to PNG and inspected at the new scales):
  - **`:name` → `:segment`.** The authored point sigil's concept renamed
    everywhere — SPEC (grammar, 15.2/15.3/15.6 tables, SPEC 20 rows, SPEC 21
    comment, SPEC 23), PLAN (ledger + log), and code: `Product` →
    `Segment`, `SketchGeo.names` → `segments`, `Folded.names` → `segments`,
    `PenCall.product` → `segment`, `PenPoint` → `PenSegment`,
    `Spot::Product` → `Spot::Segment`; the path-segment enum `geometry::Seg`
    became `PathSeg` so the two segment vocabularies can't be confused.
    Messages updated: "no segment ':step' on 'body'", "'move' takes no
    segment — name its landing with a freestanding ':segment'", "… anchor a
    side ('X:top (-)') or a segment".
  - **`scale:` defaults to 4 on a drawing.** `|drawing|`'s template bundle
    and the root `{ layout: drawing }` engine defaults carry `scale: 4`
    (≈1 mm per unit at screen resolution — Abbas's call; ledger decision 9
    annotated, SPEC 8/15.1/15.10 updated). Everything else still defaults
    to 1; `|note|`/`|balloon|`/`|table|` keep `scale: 1`. Nearest-wins is
    unchanged — note a *nested* `|drawing|` re-defaults to 4 rather than
    inheriting an ancestor's explicit scale (template tier beats
    inheritance, exactly like `|table|`'s `scale: 1`); sub-assembly views
    state their scale. Samples: dims drops its decl (showcases the
    default), tie bar / bushing / leaders / mate at 3, SPEC 24 examples
    updated to match (tie bar 2→3, bushing 1.6→3). Unit tests that assert
    absolute px pin `scale: 1`.
  - **The drawing link weight is a scope default, not a rule.** Abbas hit
    it: `|-| { stroke-width: 2 }` didn't restyle dim lines — the stage-4
    built-in `|drawing| |-|` *descendant* rule outranked a user's plain
    `|-|` element rule. Replaced at the source: `link_scope` pushes
    `stroke-width: 1` into the link **base layer** when the scope chain
    (or root) is a drawing — below every user rule, like the scope's
    `clearance`/`routing`; the scoped rule is gone (ledger decision 11
    annotated, SPEC 15.1 reworded). Regression test covers the exact
    override. The `|sequence|`/`|drawing| |note|` compaction rules keep the
    rule form deliberately — they must beat the note *template's* padding,
    and the descendant-selector override (`|drawing| |note| { … }`) is the
    documented escape there.

- **2026-07-05 — independent audit round** (opus agent over stages 1–5a,
  emphasis on the hatch slice and the review follow-ups; hatch tile geometry,
  the `(-)` dispatch, packing, wedge math, ray-cast frames, scale-4 fallout
  and the rename all probed clean — measured values re-verified by hand).
  Fixed at the source:
  - **The `|-|` scope default leaked into nested flow scopes** (the audit's
    one major): `link_scope` pushed `stroke-width: 1` when *any* ancestor
    was a drawing, while the mate gate classifies by the **immediate**
    scope — so a `|row|` nested in a drawing had its routed links thinned
    to 1. Both now use `scope_is_drawing` (one predicate, one mechanism);
    regression test covers a `|row|` in a `|drawing|` node.
  - Three stale `:name` doc comments swept (value.rs, syntax/ast.rs,
    parser.rs).
  - `dims.rs` (565 LOC) split per the ~500 law: the `(-)` readings moved to
    `round.rs`; `dims.rs` keeps `<->` + the shared stacked-dim anatomy
    (`Stacked`/`stacked`/`stack_side`/`arrow`/`span_on`, now `pub(super)`).
  - **New open thread** (audit-adjacent, pre-existing): a **root** drawing
    never runs the router, so a wire inside a nested flow scope (`|row|` in
    a root `{ layout: drawing }`) is silently dropped — a *node* `|drawing|`
    in a flow sheet routes it fine. Same family as the stage-3 anonymous
    -container question; decide when `break:`/stage-5 lands.
  - Deliberately not "fixed": the layout-time "{noun} endpoint 'x' not
    placed" message (only reachable via the documented anonymous-container
    edge); `pen.rs`/`engine.rs`/`annotate.rs` LOC counts (past ~500 only
    with their test modules — the law targets code, and `pen.rs` already
    ceded `corner.rs`).
  - **Still open in stage 5**: `break:` (the big one — clip the folded
    subpaths at the stations, slide the far piece, the piecewise
    view-offset map through `annotate::Ctx`, `|breakline|` zigzag +
    S-break chrome), `fmt` for `draw:`, the three SPEC 24 examples
    byte-for-byte as samples, the barrel stress sample, the SPEC 16
    ledger sweep, delete `DRAWING_OLD.md`, README check.

- **2026-07-05 — stage 5 finished** (fresh session; four commits —
  `1d7b136` break:, `dbd0124` fmt, `478a527` samples, plus the finish
  sweep; all 14 suites green — 719 tests — clippy silent, fmt clean; every
  drawing sample PNG-rendered and inspected; measured values hand-checked:
  tie bar 300 across the break, barrel 485/38/14·7/⌀42 h6/⌀45 f7/⌀50,
  pump 230 = 180 + 60 − 10 press). What shipped, and the decisions:
  - **`break:`** (`layout/drawing/breaks.rs`): stations parse from the
    resolved groups (two numbers `a < b` + optional axis; default = the
    model's longer axis), scale with the node, and clip the folded,
    scaled subpaths — segments split exactly at the station lines (lines
    and circular arcs closed-form; a **cubic** must clear both stations,
    hull-tested, else the new "can't cut a 'curve()'" error — SPEC 23
    note added). Kept pieces stitch into maximal runs that stay **open**
    at the cut: SVG's implied fill closure *is* the straight cut edge,
    the profile stroke never draws there, and the `|breakline|` chrome
    draws over it. The far piece slides to the sheet-space 12 px gap via
    a per-axis piecewise-linear **view map** — monotone, total,
    invertible (the removed span squashes into the gap, so even a
    station *inside* the cut displays sensibly). **Deviation from the
    stage-4 note**: the map rides `SketchGeo` per node, not
    `annotate::Ctx` — a break belongs to its sketch, and a drawing-wide
    map would wrongly shift other parts. Anchors resolve **displayed**
    (segments map at `spot()`, bbox spots read the clipped box);
    `Anchor::model_point`/`model_world` unmap for values — dims, station
    ⌀s, and the bare mirrored span all read the unbroken model. Mates
    seat displayed (the view stays self-consistent).
  - **Features ride the break** (found by the barrel): `place_features`
    maps a broken parent's feature positions through its view map
    (rigid with the displayed pieces — a far-side hole slides along),
    and the anchor walk accumulates a parallel **model origin**, so a
    dim to that hole still reads true. A feature's own *shape* never
    clips (only positions map) — accepted edge, same family as SPEC 23's
    angled break lines.
  - **Chrome**: desugar generates two `|breakline|` children per comma
    group (`chrome: break N`, authored order); the pen's fold fills them
    — the thin zigzag with the lightning jog mid-span, or the round-stock
    **S** (two opposed cubic bows) when a `mirror:` axis parallels the
    break axis; the S node's kind flips to `Path` (a `|line|` can't arc).
    Generated chrome now carries the parent's **tail** span — the fmt
    printer sorts a body by span, and parent-headed spans hoisted chrome
    above authored children, breaking `compile(desugar(src))`
    byte-transparency (the oracle test caught it on the barrel).
  - **Four new SPEC 20 rows**: break off a sketch ("'break' cuts a
    '|sketch|' — draw the profile with the pen" — layout gates it on any
    other node), a station that misses the profile, overlapping spans,
    the cubic cut. SPEC 15.10's `break` row narrowed to `|sketch|`.
  - **fmt**: a `draw:` is a paragraph — never shares a line with another
    declaration; calls flow to the 80-column budget, every `move()`
    after the first starts its own subpath line, continuations align
    under the first call (exactly SPEC 19's promise); a short
    single-subpath draw still inlines with its node, a multi-subpath one
    never does. Mate/dim/leader statements already formatted like links
    (stage 1); tests pin all of it now.
  - **A chain seats one row** (found by the barrel's 14·7): SPEC 15.6
    says "a chain shares one row", but hops seated independently and a
    narrow hop's flipped-arrow reach poked the neighbour's interval,
    splitting the row. `dims::stacked` split into `plan` (footprint) +
    `at_row` (anatomy); a chain whose hops agree on axis + side seats
    the **union** interval once — flipped arrows abutting tip-to-tip at
    a shared extension line are drafting-normal, not a collision. Mixed
    chains fall back to per-hop seating.
  - **Samples**: the three SPEC 24 examples land **byte-for-byte** (code
    below the header comment): `drawing_tiebar.lini` gained its
    `break: -80 60` (S-break pair), `drawing_bushing.lini` one comment
    word, `drawing_pump.lini` is new (mated assembly, balloons, BOM).
    `drawing_barrel.lini` is the DRAWING_OLD §17 stress sample,
    condensed: scale 3, no unit (the flipped narrow hops need the room),
    manual centerline dropped (the fused mirror auto-generates it), the
    m8 callout steered `side: top` (the datum ray grazed the ⌀ stack),
    and `break: 90 165` threaded between the bolt holes.
  - **Finish**: SPEC 16's drawing marks flipped to ✓ in one sweep (scale
    default noted as `|drawing|` 4; `break` on `|sketch|`; the
    "⌛ as one unit" footnote removed); `DRAWING_OLD.md` deleted; README
    gained the drawings bullet.
  - **Open threads** (all pre-existing, none blocking): the root-drawing
    router gap (a wire in a nested flow scope of a *root* drawing is
    dropped — a node `|drawing|` routes it fine) is **deferred to a core
    scoping session together with the anonymous-container question** —
    same family, and the fix should be one mechanism for both, with
    Abbas's call on anonymous scope semantics. Leaders/diametral lines
    still aren't collision-packed against dim rows (deterministic;
    `side:` steers — the break samples didn't collide). The angle arc
    still draws no leg extension lines.

- **2026-07-05 — Abbas's break review** (same day; all gates green — 720
  tests — clippy silent, fmt clean; tie bar + barrel re-rendered to PNG and
  inspected against his reference images). Two fixes at the source:
  - **The break is a black hole for position** (the barrel bug he caught:
    the third M8 hole ignored the compression). `place_features` mapped
    only a feature's *own* translate; a `pattern:`'s copies — and any
    deeper descendants — are placed by offsets inside the carrier and
    never rode the map. Now `ride_view` recurses: every non-chrome
    position in the broken frame maps through the view (`map(base + d)`),
    stopping only where positions leave that frame — a turned child, a
    layout-owning (sealed) child, a child with its own break — each of
    which still rides as one box; a carrier's bbox re-unions around the
    ridden copies. The anchor walk carries the same state (the active
    view + walked model/displayed positions) and inverts it per hop, so
    a dimension to a far-side copy still reads the unbroken model. First
    walk version used `map(base)` as the displayed base — wrong when the
    removed span contains the origin (child positions are stored absolute
    in the sketch frame); the regression test pins it. SPEC 15.3 gained
    the black-hole bullet; ledger 8 annotated.
  - **One break-line convention** (Abbas's call, reference: the standards'
    long-break line): the round-stock S is gone; both cut edges draw the
    thin line with the **sharp compact jog** mid-span — jog half-height
    min(0.28 h, 9), amplitude min(0.2 h, 4.5), safely inside the 12 px
    gap so the twin jogs never touch. `CutEdge.s_break`, the mirror
    check, and the kind-flip to `Path` are deleted (`|breakline|` is a
    plain `points:` polyline again). SPEC 8/15.3/15.7 reworded; ledger 8
    annotated; sample comments updated.

- **2026-07-05 — Abbas's paint review** (same day; all gates green — 721
  tests — clippy silent, fmt clean; dims + barrel PNG-inspected in both
  tones). Two visual-layer fixes:
  - **`--lini-stroke-light`** — a new visual var (SPEC 10.1), the secondary
    line tone for drafting's thin support lines, aliasing
    `--lini-gray-deep` (dark/light aware, tree-shaken, user-overridable).
    The `|centerline|` / `|pitch-circle|` / `|breakline|` bundles now carry
    `stroke: --stroke-light`, so the chrome class rules state it once and
    a `|centerline| { stroke: … }` rule still overrides.
  - **The dimension anatomy rides classes, not inline styles** (SPEC 17):
    dim / leader linework is `lini-dim-line`, extension lines
    `lini-ext-line` (painted `--stroke-light` unless the statement
    recolours — `Paint.light` falls back to the explicit stroke), and the
    slender arrowheads are `lini-marker lini-marker-dim` riding the
    existing `.lini-marker` rule (`prim::dim_marker` — no stray
    `stroke-width: 0; opacity: 1` inlines). The two new rules emit only
    when present, placed after the shape rules so they win the
    same-specificity tie; the repeated
    `style="fill: …; stroke: none; stroke-width: 0; opacity: 1"` and
    `style="stroke-width: 1"` inlines are gone from every drawing sample.

- **2026-07-05 — the floating datum** (Abbas's catch: `body:land >- "A"` and
  the `side: top` thread leader hovered at the bounding box instead of
  touching the drawn edge). Root cause: a **sign error in
  `outline::ray_line`** — the segment parameter `s` divided by `−denom`
  while `t` divided by `denom`, so the on-segment test accepted each
  segment's *mirror about its start point* and rejected true hits. Every
  earlier ray-cast survived by symmetry (circle rims use `ray_circle`;
  box-centre aims hit coincidentally); a **recessed** edge — the barrel's
  thread section below the tube surface — exposed it: the miss fell to the
  rect fallback, the box top. One-character fix at the source; regression
  test pins both tips on the drawn surface (y = −63, not the box's −75).
  The leaders sample's note arrow now lands exactly on the bracket corner.

- **2026-07-05 — the tilted datum** (Abbas's catch: the `>-` triangle rode
  the leader's angle — a shallow leader laid the symbol nearly flat along
  the surface). GD&T seats the datum triangle on the **feature**, not the
  line: on a directed anchor the lowering now draws it itself
  (`prim::dim_marker("datum", …)` — the marker builder gained a variant,
  classes `lini-marker lini-marker-datum`) with its base flush on the drawn
  edge and its apex out along the **surface normal**; the leader meets the
  apex at whatever angle it arrives. The surface sets the triangle's axis,
  the leader its **sign** (the apex points to the elbow) — so an edge
  authored material-on-the-left (the leaders sample's CCW bracket, where
  `outward` faces into the part) still seats right side out. A
  point-anchored datum (no normal) keeps the core line-oriented marker —
  today's fallback. Size shares `render::markers::marker_size` (one
  formula). Tests pin the seated base on the surface and the fallback.

- **2026-07-05 — circle leaders press the rims** (Abbas's round: two fixes;
  all gates green — 724 tests — clippy silent, fmt clean; dims + barrel
  PNG-inspected against his reference).
  - **A `<-` word leader on a patterned hole tipped the centre, not the
    rim**: `pattern::expand` cloned the whole node as the copy body, so
    every copy kept the `pattern` attr — the ray-cast's carrier arm
    recursed into the copy, saw only chrome children, returned `None`, and
    the tip fell back to the aim. Copies now shed `pattern` (they are not
    carriers); this also stopped `ride_view` re-unioning a copy's bbox
    over its chrome. Test pins the tip on the seed's rim.
  - **The bare `(-)` circle reading looked like a word leader** (one
    arrow): it now draws the drafting ⌀ line — along the diameter through
    the centre, overshooting the far rim by an arrow-length, with **both
    arrowheads pressing the rims inward from outside** (his reference's
    ⌀20/⌀14.5 form). The `R` arc leader keeps its single arrow. Inside
    placement (arrows out, value on the line) stays a future knob — the
    side-anchored diametral already covers the fits-inside case. Ledger 3
    annotated; SPEC 15.6's table row reworded.

- **2026-07-05 — leaders leave straight off the face** (Abbas's catch: the
  datum "A" ran a steep diagonal to the top-left — the auto direction was
  always the datum-ray). The default leader direction for a **directed**
  feature (a side, a named edge) is now its **surface normal**,
  sign-corrected away from the datum — straight off the face, then the
  horizontal elbow, exactly what `side: top` spelled by hand (the samples'
  explicit `side: top` steers are now redundant but stay as documentation).
  Point features (holes, origins, arcs) keep the datum-ray; `side:` still
  overrides everything. SPEC 15.7's placement rule reworded; the seated
  datum's test pins the vertical rise.

- **2026-07-05 — dim polish round** (Abbas's three catches on his re-cut
  barrel; all gates green — 727 tests — clippy silent, fmt clean; barrel at
  scale 2 + a `side: top` probe PNG-inspected):
  - **"14·7 read as 147"**: the narrow-span flip moved arrows *and* value
    outside, so adjacent flipped chain hops overlapped their texts. Now
    three regimes (SPEC 15.6): everything inside; arrows out, **value
    centred inside** while the bare text still fits (`tw + 4 ≤ span` —
    drafting's middle form, what the chain needed); only a span too tight
    even for the text slides it past the nearer line.
  - **Rows dodge callout texts**: `side: top` seated the 485 on top of the
    M42/A/M8 leader texts — the packer only knew other dims. Not routing:
    leaders/callouts/angles lower **first** (they are feature-anchored) and
    their text boxes register as **obstacles** in `Rows`; a candidate row
    whose band (line + the value riding above it) intersects one is
    skipped. Output keeps source order; `(-)` leader texts obstruct
    later dims too. This closes most of the stage-4 "leaders aren't
    collision-packed" thread from the dim side; leaders themselves still
    place deterministically.
  - **Slender arrows**: 9×3 → **10.5×3.5** (still 3:1, SPEC 10.5), and the
    dim line now stops `2·stroke-width` short of each tip — a butt-capped
    stroke ending exactly at the tip blunted it (same fix links carry).
    The diametral line and R-leaders trim the same way.

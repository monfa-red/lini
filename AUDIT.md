# Codebase audit — pre-v1.0

Findings and decisions from a full read of `src/` (~44.6k LOC), 2026-07-10, feeding
the v1.0 refactor stages in `PLAN-ALPHA.md`. Six subsystem reports (front-end,
desugar/resolve, layout, render, routing, periphery) were gathered by agents; this
file is the synthesis and the calls. Delete it once the refactor stages have landed.

**The oracle.** Output is byte-deterministic and snapshot-pinned, so a pure refactor
changes **zero snapshots**. Every work item below is tagged `[pure]` (snapshot-neutral)
or `[output]` (intentional byte change — its own commit, snapshots re-blessed with the
reason in the message).

**LOC convention (adopted).** Several "oversized" files are test-heavy, not
logic-heavy (`chart/mod.rs` is 600/926 test LOC; `drawing/engine.rs` 485/693). The
~500 LOC guideline counts **production lines**; big `#[cfg(test)]` blocks move to
sibling `tests.rs` files as the standing convention.

---

## Verdicts (one line per subsystem)

| Subsystem | State |
|---|---|
| Front-end (lexer/parser/expr) | Good. Comma law needs **zero parser changes**; debts: a fully duplicated expression lexer, parser.rs size. |
| Desugar/resolve | Good structure, but **no property ledger** — property knowledge scattered over 285 read-sites / 80 names / 5 mini-classifiers. |
| Layout | Engines work but there is **no engine trait** (two ad-hoc dispatch sites); drawing-chrome constants scattered across 8 files; a real crop of small parallel implementations. |
| Render | Solid. The leak-proof chokepoint (`inline_paint_diff`) exists; leaks survive only on class-less sub-elements and hand-rolled text paths. Determinism hygiene clean. |
| Routing | Recently refactored, clean, constants centralized and matching ROUTING.md. `natural` = new module + five small named touches. |
| Periphery | CLI/fmt/serve/tests in better shape than assumed: fmt already prints comma-law shape, test suite is layered (conformance + desugar oracle + routing laws + fmt fixed-points). Zero TODO/FIXME markers in `src/`. |

---

## Decisions

- **D1 — Build `src/ledger/` as the single source of truth.** Three files:
  `properties.rs` (new: name → owners, value shape, default ref, inherits?, gate),
  `defaults.rs` (today's `desugar/bundles.rs`, moved), `consts.rs` (the SPEC-10.5
  chrome constants + look tunables, gathered). Pure data, no behavior. Consumers, in
  order: the resolve classifiers, the 0.21 validation pass, schema generation, fmt,
  generated SPEC tables. Build data-first `[pure]`, migrate consumers one at a time.
- **D2 — Tree branch links are generated in desugar**, not layout. They are
  structural (parent-topic → child-topic, placement-independent), so desugar emits
  ordinary `|-|` links scoped to the tree container: the router consumes them
  unchanged, `lini desugar`/`fmt` show them, and the tree engine stays a pure
  arranger. This closes the "no layout→router request channel" gap without inventing
  one.
- **D3 — A `LayoutEngine` trait lands *with* the tree work, not before.** Today's
  two dispatch sites (lowerer if-else at `layout/mod.rs:350`, `LayoutMode` enum at
  `:736`) are fine for the current engines; tree is the forcing function that makes
  the categories explicit (`lower | arrange`), so the trait is designed against a
  real third case instead of speculation.
- **D4 — Strategy checks become exhaustive `match`es** during refactor. Every
  `Strategy` test today is `==`/`!=` (straight.rs:67, validate.rs:36,
  request.rs:174, ortho/mod.rs:154,407), so adding `Strategy::Natural` compiles
  silently past all of them. Exhaustive matches make rustc list every site that
  needs a judgment when `natural` lands.
- **D5 — Halos are masks, not understrokes.** The precedent exists at
  `render/links.rs:386` (`label_mask`, a luminance mask chosen for exactly this
  reason): an understroke paints wrong over hatching/gradients and needs the bg
  colour in dark mode. Generalize `label_mask` into a knockout-region builder.
- **D6 — Embedded SVGs need id-rewriting, not just nesting.** A nested `<svg>`
  isolates viewport and (with `.lini` scoping) CSS, but SVG ids stay
  document-global — an embedded `id="a"` collides with `url(#a)`. Prefix embedded
  ids on embed.
- **D7 — Comma-law enforcement lives in resolve, not the parser.** The parser is a
  shape-agnostic comma/space splitter (correct; keep it dumb). Legacy space-lists
  can't be rejected at parse; each list-valued reader errors with a targeted
  message ("`data` takes comma-separated values — `data: 9, 15, 24`").
- **D8 — `section.rs`'s `OVERHANG 6.0` is a different concept** from the
  center-mark `OVERHANG 4.0` (plane-line overshoot vs centre-mark overhang) — it
  gets its own name (`PLANE_OVERHANG`) in `ledger/consts.rs` rather than forced
  unification. The duplicated 4.0 (breaks.rs:22 / chrome.rs:20) does unify.
- **D9 — JSON diagnostics stay serde-free.** `Error { message, span, related }` is
  tiny and fixed; hand-roll `to_json()` reusing `serve/http.rs`'s `json_escape`.
  The real work is stable diagnostic codes (an `enum Code` + `Error::coded`),
  not the envelope.

---

## Work packages

### R1 — The ledger (foundation for validation + schema) `[pure]` · L

The de-facto ledger is `desugar/bundles.rs` (name → default, grouped by owner, for
13 primitives + 48 templates + link/root defaults) — ~80% of what's needed. Missing:
owned-but-defaultless properties (`points`, `symbol`, `data`, `cell`, `of`, `at`,
`tol`, `draw`…), value shapes, and SPEC 16's lenient-vs-hard-gate flag. Scattered
knowledge to absorb (the migration list):

- `resolve/value.rs:204 is_string_valued`, `:22 is_builder`, pen/pattern
  special-cases `:111`
- `resolve/scene.rs:19 INHERITED_TEXT` + `:443 is_text_prop`
- `resolve/merge.rs:23 is_marker_attr`
- `resolve/program.rs:319 SCOPE_LINK_PROPS` + the **independently re-listed**
  text-prop subset at `:524-532` (drift risk — derive both from one annotated list)
- 285 `.get()/.number()` sites over 80 names (layout 196, render 53, resolve 29) —
  these keep reading by name; the ledger is what *validates*, not a runtime hop.

Also folds in: `tags:`→`labels:` collision handling (`labels` is already live in
`is_string_valued` and `chart/model.rs:296` — reconcile, don't just rename), and the
keyword/type-name lists that schema generation and editor grammars will consume.

### R2 — Kill the parallel implementations `[pure]` · S–M each

Shared helpers, each replacing 2–8 copies:

| New home | Replaces |
|---|---|
| `crate::suggest::nearest()` (Levenshtein + rank + "did you mean" formatter) | 2 full Levenshteins (`desugar/page.rs:103`, `icon/mod.rs:86`), 3 length-diff rankers (`drawing/anchors.rs:200`, `threads.rs:171`, `chart/model.rs:834`), exact-only `resolve/scene.rs:584`; ≥4 hand-written `"; did you mean"` strings. Also the hook for the 0.21 implicit-node similarity warning and ledger property suggestions. |
| `Bbox::from_points` / `overlaps` / `contains` / `extent_of` (funnel through `accumulate_extent`) | 3 point-fold copies (`prim.rs:57`, `primitives.rs:220`, `breaks.rs:555`) + inline copies in chart/sequence; 3 AABB overlap re-rolls (`chart/labels.rs:211`, `annotate.rs:264`, `:220`); 3–4 child-union loops missing rotation awareness (`sequence/mod.rs:229`, `section.rs:155`, `pattern.rs:79`) |
| `geometry::unit()` | ~8 hand-rolled normalises (`leaders.rs:76,148,244`, `angle.rs:80,155`, `anchors.rs:310`, `edges.rs:157`, threads) |
| `liang_barsky()` | `chart/project.rs:136 clip_segment` ≡ `chart/labels.rs:219 seg_hits_rect` |
| one `diameter_line()` | the two independent ⌀-line drawers (`leaders.rs:132`, `round.rs:239`) + the two fits-else-spill blocks (`dims.rs:141`, `round.rs:244`) + ISO −90° text rotation ×3 (`dims.rs:235`, `round.rs:248`, `compose.rs:183`) |
| `layout/label_place.rs` (generic greedy seat-search, takes a bounding rect) | chart's placer (`chart/labels.rs:112-163`, nearly domain-agnostic today); future consumer for sequence/leader text seating — the drawing `Rows` packer and leader elbows stay separate (genuinely different geometry) |
| expr consumes `&[Token]` | expr.rs's second lexer (110-216) + diverged number scanner (sci-notation only there — keep the semantic split, one scanner implements it) |
| `dash_decl()` | `rules.rs:119` ≡ `rules.rs:629` |
| `intern_by_key()` | `FilterTable` (filters.rs) ≡ `Interner` (paints.rs); also replace the `{:?}`-Debug dedup keys (`paints.rs:41,186`) with structural keys |
| one `is_light_dark` | `style_block.rs:87` ≡ `theme.rs:173` |
| `desugar::chrome::node()` | `desugar/drawing.rs:183` ≡ `desugar/page.rs:246` |
| one AST-shaped + one IR-shaped drawing-scope predicate | 6 copies (`desugar/mod.rs:737,745`, `lint.rs:168`, `resolve/program.rs:391,413`, `layout/mod.rs:537`, `drawing/mod.rs:92`) |
| `Parser::span_from()` | ~15 `Span::new(start.start, last_span().end)` repeats |
| shared value-join (`Tuple`→space, `List`→comma) | copied 3× (`fmt.rs emit_value`, `render/values.rs format_value`, `:87 px_lengths`) — different types/stages, so share the join+number-format shape, not one function forced over both |

Plus small deletions: `defaults.rs:151 set_visual` (no-op wrapper), stale
`resolve/mod.rs:5` doc, one-variant `Level` enum (becomes real levels with D9),
`Value::Group`→`Tuple` and `StyleItem::Func`→`Binding` renames.

### R3 — Constants into one home `[pure]` (values that change are `[output]`) · M

`ledger/consts.rs` gathers what SPEC 10.5 promises lives in one place:

- Drawing chrome, today across 8 files: `annotate.rs:17-27` (DIM_OFFSET 18,
  DIM_PITCH 16, EXT_GAP/OVERSHOOT 3, ARROW 12, NOTE_OFFSET 14, NOTE_LANDING 8),
  `compose.rs:37` (TOL_STACK 0.7), `breaks.rs:20-22` (BREAK_GAP 12, OVERHANG 4),
  `chrome.rs:20` (OVERHANG 4 — dedupe), `section.rs:24-32` (plane anatomy; D8),
  `threads.rs:18` (THREAD_DEPTH — also name chrome.rs:120's unnamed 0.54125 twin),
  `markers.rs:56` (datum 11), hatch metrics in `render/paints.rs`.
- The two SPEC-named literals hiding in resolve: drawing-link stroke-width 1 /
  font-size 12 (`program.rs:470,473`) — and align `annotate.rs:71 Paint::of`'s
  11.0 fallback with SPEC's 12 `[output if reachable]`.
- `DEFAULT_CLEARANCE = 16.0` — three disagreeing unreachable fallbacks today
  (`request.rs:256` → 0.0, `render/links.rs:31` → 0.0, `messages.rs:71` → 16.0).
- Root font-size fallback `program.rs:519` reads the bundle instead of a second 15.
- A4 dims deduped (`page.rs:34` vs `bundles.rs:277`); `MAX_INHERITANCE_DEPTH`
  error string derives from the const (`types.rs:163`).
- `chart/metrics.rs`: TITLE_SIZE collision (13.0 in `chart/mod.rs:38` vs 11.0 in
  `axis.rs:13`), LABEL_SIZE ×4, area opacity 0.82 ×2, tick target `range/5`.
- Named marker ratios (`markers.rs:235` arrow 0.5, `:285` diamond 0.425 ×4,
  ring offsets), text leading 1.2 (`text.rs:56`), and the visual knobs currently
  buried in algorithms: wavy WAVELENGTH/AMPLITUDE, note fold FOLD_FRAC/MAX,
  page MARGIN/FILING (judged look-tunable by the sweep).
- Leave local: geometric EPS values, HTTP buffer sizes, fmt MAX_LINE/INDENT,
  routing's `cost.rs` (already correct and centralized — don't move it),
  `AVG_CHAR_WIDTH_RATIO 0.6` (stays in `text.rs` untouched — the Stage M5 font
  metrics supersede it; the bundled mono's advance is exactly 0.6em, asserted
  in xtask).

### R4 — Render: one paint chokepoint, close the leak list `[output]` · M

- Route the 2–3 text-leaf style computations (`mod.rs:283 text_style_attr`,
  `links.rs:447 render_link_text`'s hand-rolled diff) through `inline_paint_diff`
  against `.lini-text`/`.lini-link-label`/`.lini-sequence-message` — fixes the
  drifted `text-shadow` (works on node text, silently dropped on link labels).
- `.lini-gutter { stroke: none }` rule; hoist hatch `<pattern>` line paint onto one
  `<g>`; `messages::LABEL_SIZE` becomes `pub(crate)` and `rules.rs:491` references
  it; stray-glyph classes (low priority).
- Split `rules.rs` (827): model (`Rule`/`RuleSet`/queries) vs `stylesheet.rs`
  (the 435-LOC `build()`), then per-family sub-builders `[pure]`.
- End state: **no code path emits paint outside the diff** — the whack-a-mole ends.

### R5 — File splits (with the test-LOC convention) `[pure]` · M–L

- `syntax/parser/` — `values.rs` first (isolates the comma-law surface), then
  `nodes/links/decl/selector/classify`.
- `layout/mod.rs` (1329) → `arrange.rs` (container children, gutters, modes) +
  `frame.rs` (finish/viewbox/extent); mod.rs keeps dispatch + `layout_inst`.
- `chart/model.rs` (1473) → `chart/model/{types,build,series,axes,annot,paint}.rs`;
  pie bits join `pie.rs`. The one L-effort split (`build()` threads shared state).
- `desugar/mod.rs` (901) → `tables.rs` + the smart-label ladder joins `labels.rs`.
- `resolve/program.rs` (892) → `theme.rs` + `link_scope.rs`.
- Drawing: `annotate.rs` → `rows.rs`/`paint.rs`; `breaks.rs` → `viewmap.rs`/`clip.rs`;
  `section.rs` → `plane.rs`/`detail.rs`; `pen.rs` → `pen/parse.rs`.
- `ortho/world.rs` — extract `build_worlds()` + `world_ladder()` from
  `ortho/mod.rs:158-190` (serves the LOC rule *and* is the assembly glue `natural`
  must reuse; type-level reuse is already clean, assembly-level isn't).
- Tests move to sibling `tests.rs` files where they dominate a file.

### R6 — Behavior fixes (bundled into the 0.21 hardening round) `[output]`

- **Root-drawing router gap**: `layout/mod.rs:41-46` builds Routing from
  `owned_links` only — replace with `routing::route(program, &top_nodes)?`
  (`requests()` already excludes drawing-scope links by exact scope match;
  `route()` already appends owned links). **Same latent gap in the root-sequence
  arm** (`mod.rs:63-71`): route, then extend with the message wires. Regression
  sample: a wire inside a nested `|row|` under each root layout.
- Scoped note rules (`|sequence| |note|` compaction) move from
  `resolve/program.rs:365` into desugar as generated descendant rules — the
  teaching view stops lying (desugar snapshots re-bless; SVG unchanged). The
  drawing link-scope *base* stays at resolve — it's a deliberate below-rules
  cascade tier per SPEC 9; moving it would change specificity.
- Remove the dead `--standalone` flag (`main.rs:28,94`); unify the four hand-rolled
  subcommand arg-loops under `clap::Subcommand` (~150 LOC of repetition).
- CLI-binary test for `--strict`-turns-warnings-into-exit-1 (currently untested).

### R7 — Docs kept honest

- ROUTING.md: replace the stale `curved` row with `natural`'s contract when it
  lands (not an alias — different, obstacle-aware contract); refresh the
  "Implementation shape" file map (6 names vs 15 real files, `validate/excuse.rs`
  missing); tighten `mod.rs:1-8`'s overstated "every strategy is validated" claim.
- SPEC 10.5's "lives in one place" becomes true (R3) — reword to name the ledger.

---

## Leave alone (verified good — don't churn)

- `ortho/` core algorithms; `place.rs` stays whole (already offloaded
  `ladder`/`pairwise`/`order`; a split would fragment shared helpers).
- `cost.rs` — the Law-3 constants, single home, all consumers import it.
- `text.rs` measurement — the single-source model to copy (its API widens for
  wrapping in 0.21+ work, but it's the right home).
- Marker geometry, corner rounding, colour formatting, tree-shaking — verified
  single-mechanism.
- Render determinism (BTreeSet everywhere, lookup-only HashMaps, stable floats).
- The test suite's layering (conformance snapshots / desugar-transparency oracle /
  routing-law oracle / fmt fixed-points) — extend, don't reshape.
- Icon `lookup`/`Role`/`emit_role_group` — already the right seam for the drafting
  symbol registry (needs only a natural-units sizing path).
- `validate.rs` + `validate/excuse.rs` split; samples inventory (no stale files);
  Cargo.toml (2 deps, both justified).

## Feature-seam readiness (for the plans)

| v1 feature | Seam | Readiness |
|---|---|---|
| Comma law | parser `Vec<Vec<Value>>` → `resolve/value.rs resolve_groups:81` + list readers (`chart read_data:753`, grid tracks `desugar/mod.rs:237`, align lists, `along:`…) | parser ready; per-reader flips + targeted errors (D7) |
| Property validation + schema | R1 ledger | greenfield; bundles.rs is the seed |
| Implicit-node similarity warning | `lint.rs:127 shadow_scope` + `suggest::nearest` | S once R2 lands |
| Text wrap + align-driven lines | `text.rs` scalar API → line-list; shared line-align helper called by flex *and* grid (`grid.rs:293 align_cell_content` becomes a caller — parallel-impl trap flagged) | M–L |
| scale/unit/density fold | desugar, beside `sheet:` (`page.rs::SIZES` is the model) | green-field, low risk |
| Tree + mindmap | D2 (links in desugar) + D3 (trait with the work) + flex measurement (pure fn, reusable); new code: parent-over-subtree centring, radial sectors | M seam + L geometry |
| `natural` routing | `routing/natural/{mod,corridor,curve}` + extract `build_worlds` + widen `bundles()` filter (`request.rs:174`) + D4 matches + own validator | module-addition + 5 named touches |
| Row bands/marks | `chart/annot.rs` is the *only* column-hardwired file; `Plot::project` already transposes everything else | one-file M |
| Dim clearance packing | `Rows` already carries `obstacles` + `blocked()`; replace the fixed `DIM_OFFSET + k*DIM_PITCH` generator with painted-bounds + clearance scan | M |
| Annotation nodes in link `[ ]` | link labels hard-typed `TextNode`→`ResolvedText` end-to-end (`labels.rs:79`, `ir.rs`) | L — the one genuinely deep change |
| Datum letters | new resolve-scene pass beside `id_seen` (`scene.rs:150`); `MarkerKind::Datum` exists | M |
| Halos | D5 — generalize `label_mask` | M |
| Local images | resolve/layout reads bytes; `emit_image` (`primitives.rs:465`) switches on resolved form; D6 id-rewrite; serve boundary generalizes `dir_mode.rs:196 resolve_in_root` (drop the `.lini`-only check), file-mode needs a boundary it doesn't have | M |
| Symbol registry (GD&T/finish) | second data source behind icon `lookup`/`Role` | M |
| JSON diagnostics | D9 — code enum + 6 print sites in main.rs | M |
| oklch output flag | thread Options into `palette_vars` (`palette/mod.rs:145`) | S–M |

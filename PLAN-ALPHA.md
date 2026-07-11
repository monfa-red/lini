# PLAN-ALPHA — refactor, hardening & the 0.21 breaking round

Execution plan from today's `main` to the **`1.0.0-alpha` tag** (syntax frozen).
Decisions live in `ROADMAP.md`; code findings and the `[pure]`/`[output]` tags are
defined in `AUDIT.md`. This file is stages only.

**How to run a stage (every session):**
1. Re-orient: read `AGENTS.md`, this stage's section, its AUDIT references, and the
   SPEC sections it touches; run `cargo test` to confirm a green start.
2. Execute the stage. `[pure]` stages must end with **zero snapshot diffs** —
   `cargo insta test` (or `cargo test`) proves it. `[output]`/`[breaking]` stages
   re-bless snapshots deliberately, stating why in the commit message, and verify
   visually: render affected samples to PNG with `resvg` (light **and** dark where
   paint changed) and look at them.
3. Finish: `cargo fmt`, `cargo test`, `cargo clippy` clean. One purposeful commit
   per coherent chunk (stage = 1–3 commits). Leave a dated entry in the stage's
   **Log** line below (done / deviations / follow-ups) so the next session
   re-orients from this file alone.
4. Stages are sized for one session with room left for fixes. If one runs long,
   stop at a committed green point and note the remainder in the Log.

Sub-agent policy: Opus for anything structural or judgment-bearing; Sonnet only
for mechanical bulk edits (sample migration, test moves) and summaries.

---

## Phase R — refactor (all `[pure]` unless tagged)

### Stage R1 — shared helpers: kill the small parallel implementations

AUDIT R2. Independent items; do in any order, commit in small groups.

- [x] `[bugfix]` **fmt drops a table cell's style block** — silent data loss:
  `"Apple" { color: --red-ink }` in a table `[ ]` compiles and renders, but
  `lini fmt` deletes the `{ }` when padding cells into aligned columns. Fix:
  a styled cell always re-emits its block; a row containing one exits the
  alignment grid (unstyled rows stay aligned). Add a styled-cell case to the
  fmt semantic-preservation suite — and find why the existing
  `compile(fmt(src)) == compile(src)` sweep missed it (no sample covers the
  case?). Fix that hole in coverage, not just the bug.
- [x] `src/suggest.rs`: `edit_distance` (promote from `icon/mod.rs:86`),
  `nearest(input, candidates, k)`, and one `did_you_mean(…)` message formatter.
  Migrate all six sites (`desugar/page.rs:103`, `icon/mod.rs:75-101`,
  `layout/drawing/anchors.rs:200`, `layout/drawing/threads.rs:171`,
  `layout/chart/model.rs:834`, `resolve/scene.rs:584`). Message wording may
  change → those specific error-message tests/snapshots re-bless `[output: errors]`.
- [x] `Bbox` gains `from_points`, `overlaps`, `contains`, and an
  `extent_of(nodes, filter)` that funnels through `accumulate_extent`
  (rotation-aware). Migrate the copies listed in AUDIT R2 (prim.rs, primitives.rs,
  breaks.rs, chart/model.rs, sequence/notes.rs, frames.rs, sequence/mod.rs
  `enclosing_bbox`, section.rs, pattern.rs, annotate.rs overlap checks,
  chart/labels.rs `Rect::hits`).
- [x] `geometry::unit()`; migrate the ~8 hand-rolled normalises.
  `liang_barsky()` shared by `chart/project.rs:136` and `chart/labels.rs:219`.
- [x] One `diameter_line()` + one fits-else-spill + one ISO text-rotation helper
  for the drawing round-seams (`leaders.rs:132` / `round.rs:239` / `dims.rs:141`
  / `compose.rs:183`); one shared `translate:`-reader (`anchors::translate`
  becomes the only one).
- [x] Render dedups: `dash_decl()` (rules.rs:119 ≡ :629); one `is_light_dark`;
  `intern_by_key()` replacing `FilterTable`/`Interner` twin engines; structural
  dedup keys replacing `{:?}` keys (paints.rs:41,186).
- [x] `desugar::chrome::node()` (drawing.rs:183 ≡ page.rs:246). One AST-shaped +
  one IR-shaped drawing-scope predicate replacing the 6 copies.
- [x] `INHERITED_TEXT` single-sourced (kill the re-list at
  `resolve/program.rs:524-532`). `Parser::span_from()` (~15 sites).
- [x] Deletions/renames: `defaults.rs:151 set_visual`; stale `resolve/mod.rs:5`
  doc; `Value::Group`→`Value::Tuple`; `StyleItem::Func`→`Binding`; one-variant
  `Level` enum folded into a real severity enum (Error/Warning — Stage M2 adds
  codes).

Acceptance: zero snapshot diffs (except re-blessed error-message tests); clippy
clean; grep proves no remaining Levenshtein/AABB/normalise copies.
**Log:** 2026-07-10 — **done**, 11 commits, all acceptance met (fmt/test/clippy
clean; grep confirms one Levenshtein, one point-fold/overlap/normalise home, no
twin intern engines). Items landed:
- `[bugfix]` fmt styled-cell block drop — fixed (`emit_aligned_cells` row-aware,
  reuses `emit_text_node`); coverage hole closed with a new `samples/
  table_cell_style.lini` (now swept by fmt/oracle/conformance) + a focused unit test.
- `suggest.rs` — `edit_distance` / `nearest` / `did_you_mean`; six sites migrated.
  One `[output: errors]` re-bless: the `sheet:` hint adopts the shared
  "; did you mean" clause (test assertion, not a snapshot).
- `Bbox::from_points/overlaps/contains/extent_of` — migrated; `extent_of` also
  dedups engine/annotate accumulate_extent loops + sequence/section child-unions.
- `geometry::unit()` (9 sites incl. a follow-up grep sweep) + `liang_barsky()`
  (chart clip/hit unified, epsilon reconciled to 1e-9).
- `iso_text_angle()` (round + dims, −90 float-exact) + `anchors::translate` the
  only `translate:` reader (dims, section).
- render: `ResolvedValue::is_light_dark`, `rules::dash_value`, and one
  `IdTable<K,V>` interner (paints gradients/hatches + FilterTable shadows).
- `desugar::chrome::node()`; `resolve::is_drawing(attrs)` the one IR predicate
  (layout dispatch + resolve link pass call it; resolve stays layout-independent).
- `scene::BAKED_TEXT` single-sources the `.lini`-rule text subset off
  INHERITED_TEXT; `Parser::span_from()` (9 sites).
- renames: `Value::Group`→`Tuple`, `StyleItem::Func`→`Binding`; deleted the
  no-op `set_visual`; fixed the stale resolve module doc.

**Deviations** (genuinely-different slices kept, per AGENTS "keep only the
genuinely-different slice"): `pattern.rs`'s carrier bbox NOT funneled through
`extent_of` — the deep/rotation-aware path counts each copy's feature overhang
and diffs drawing_pattern + drawing_dims (that is the AUDIT's "missing rotation
awareness" latent-bug fix → an `[output]` change, out of a `[pure]` item); the
two ⌀-line drawers and two fits-else-spill blocks left apart (divergent fit
clearance 6.0 vs 8.0, leader-elbow vs centre framing); the AST-shaped
drawing-scope checks (`lint::is_drawing_node` pre-define vs
`desugar::is_drawing_body` post-define chain) and `owns_layout`/`seals_drawing_
scope` (any-layout, not this predicate) stay; `angle::leg` (returns length),
`threads::parallel` (normalise-then-dot), and routing's own `Rect` untouched.

**Follow-ups** (deferred, both flagged in commits): the one-variant `Level` enum
→ Error/Warning severity — deferred to **M2**, which actually constructs
`Level::Error` (adding it now is a dead variant); and the render `{:?}` Debug
dedup keys → derived-`PartialEq` structural keys — needs `PartialEq` on
`ResolvedValue`, which cascades through `Expr` (the `Deferred` variant), out of
scope for a pure dedup.

### Stage R2 — the ledger

AUDIT R1 + D1. The foundation for validation (H2), schema (beta), and docs.

- [x] Create `src/ledger/` — `properties.rs`: one row per property
  (`name, owners, shape, default-ref, inherits, gate`), covering the ~80 known
  names incl. defaultless ones (`points`, `symbol`, `data`, `cell`, `of`, `at`,
  `tol`, `draw`, …); `defaults.rs`: `desugar/bundles.rs` moved verbatim (imports
  updated); `consts.rs`: created empty here, filled in R3.
- [x] Migrate the five classifiers to ledger lookups: `is_string_valued`,
  `is_builder` (+ pen/pattern special-cases), `INHERITED_TEXT`/`is_text_prop`,
  `is_marker_attr`, `SCOPE_LINK_PROPS`. The 285 `.get()`/`.number()` read-sites
  do **not** change — the ledger validates and describes; reads stay direct.
- [x] Reconcile the `labels` collision now (data shape only): the ledger row for
  `labels` must describe both today's uses so the 0.21 rename (M1) is a pure
  swap.
- [x] Cross-check the ledger against SPEC 16's tables — every discrepancy is
  either a ledger bug or a SPEC bug; list them for Stage S1/S2.

Acceptance: zero snapshot diffs; a unit test asserts every property name that
appears in `bundles`-moved defaults and in the five classifiers exists in the
ledger; `lini desugar` output unchanged.
**Log:** 2026-07-10 — **done**, 1 commit, all acceptance met (zero snapshot
diffs, `lini desugar` unchanged, fmt/test/clippy clean; ledger tests assert the
legacy classifier sets fall out unchanged — membership and order — and every
bundled default name has a row). `src/ledger/` = `properties.rs` (~90 rows:
name, owners, shape, default-ref, inherit channel, text/baked flags, gate; plus
`BUILDER_CALLS`), `defaults.rs` (bundles moved verbatim), `consts.rs` (empty,
R3). All five classifiers are ledger lookups; the `draw:`/`pattern:`
structured-call dispatch rides the shape column. `labels` reconciled: one row
owns the axis use and the series use (today spelled `tags:`), same `List(Str)`
shape → the 0.21 rename is a pure swap. The shape column states the 0.21
comma-law target (list vs tuple) for M1 to read.

**Deviations:** the module creation and classifier migration landed as one
commit (the table is dead code without its readers — clippy-clean per commit).
One diagnostics-only nuance: the ISO 7200 title-block fields are string-valued
rows [SPEC 2/15.8], so a bare `dwg: x` now errors toward quoting instead of
dying silently (no sample/test hit it; desugar consumes quoted fields before
resolve).

**SPEC 16 cross-check** (each a ledger-vs-SPEC discrepancy, for S1/S2):
- `text-shadow` is honoured (inherited-text channel, text-valid, link labels)
  but missing from SPEC 16's text table → S1 adds the row (live kind).
- SPEC 16's padding row claims "Longhands `padding-top`/… accepted" — no reader
  exists anywhere in the code → S1 drops the claim (or S2 decides to build it).
- `legend:` is presented as honoured (SPEC 14.6/16 "positions or suppresses");
  no code reads a `legend` attr — the legend is auto-only (≥ 2 entries) → S2:
  mark ⌛ in SPEC 16 + defer in SPEC 23, or schedule the reader.
- `sheet` is a homonym (`|page|` size sugar vs the quoted ISO 7200 field) but
  SPEC 16 doesn't flag it the way it flags `scale`/`side`/`gap` → S1 wording.

**Follow-ups:** desugar/layout write internal generated attrs (`chrome`,
`clip`, `of-title`, `mount`) that are not user properties — M2's validation
pass needs them whitelisted (they are absent from the ledger by design).

### Stage R3 — constants into one home

AUDIT R3 + D8. Mostly `[pure]` moves; behavior alignments flagged.

- [x] `ledger/consts.rs` gathers the SPEC-10.5 chrome set from the 8 drawing
  files (annotate/compose/breaks/chrome/section/threads + markers datum + hatch
  metrics), the drawing-link literals from `resolve/program.rs:470,473`, and
  `DEFAULT_CLEARANCE = 16.0` (the three disagreeing fallbacks adopt it).
- [x] D8: dedupe the 4.0 `OVERHANG` (breaks/chrome); `section.rs`'s 6.0 becomes
  `PLANE_OVERHANG`. Align `annotate.rs:71`'s 11.0 text fallback with SPEC's 12
  `[output if reachable — check first]`.
- [x] Root font-size fallback reads the ledger default; A4 dims deduped;
  `MAX_INHERITANCE_DEPTH` error text derives from the const.
- [x] `chart/metrics.rs`: resolve the `TITLE_SIZE` 13-vs-11 collision (rename,
  values unchanged), single `LABEL_SIZE`, named area opacity, named tick target.
- [x] Name the marker ratios (markers.rs), text leading 1.2, and move the
  look-tunables (wavy, note fold, page margins) into `ledger/consts.rs`; leave
  algorithm-internal EPS/buffers/fmt-config local (AUDIT's judgment list).
- [x] `messages::LABEL_SIZE` → `pub(crate)`, referenced by `rules.rs:491`.

Acceptance: zero snapshot diffs (except the 11→12 item if it proves reachable —
then its own `[output]` commit); grep shows no duplicate literal for any
centralized constant.
**Log:** 2026-07-10 — **done**, 5 commits, all acceptance met (zero snapshot
diffs throughout — every suite green after each commit; fmt/clippy clean; the
sweep greps show each centralized value defined once, in `ledger/consts.rs` or
`chart/metrics.rs`). Items landed:
- `ledger/consts.rs` filled: dim/leader anatomy, TOL_STACK, BREAK_GAP, plane
  anatomy (`PLANE_*`), thread depths, DATUM_SIZE, hatch pitch/line-width, the
  drawing-link literals (`DRAWING_LINK_STROKE_WIDTH`/`_FONT_SIZE`),
  DEFAULT_CLEARANCE, ROOT_FONT_SIZE, A4, and the look tunables (wavy shape,
  note fold, sheet margins, TEXT_LEADING — layout + render leading now one
  constant).
- D8: the 4.0 twins unify as CENTER_MARK_OVERHANG; section's 6.0 is
  PLANE_OVERHANG; chrome.rs's bare 0.54125 named THREAD_DEPTH_INTERNAL.
  The `annotate.rs` 11.0 fallback proved **unreachable** (the drawing scope's
  base layer always supplies font-size 12) — aligned to
  DRAWING_LINK_FONT_SIZE inside the `[pure]` commit, zero diffs the proof.
  Same story for the three clearance fallbacks (0/0/16 → DEFAULT_CLEARANCE).
- Root font-size fallback + `root_defaults()` read ROOT_FONT_SIZE; A4 deduped
  (desugar DEFAULT, the SIZES a4 row, the page bundle all read `consts::A4`);
  the inheritance-depth error text derives from MAX_INHERITANCE_DEPTH.
- `chart/metrics.rs` (per plan — chart constants live with the chart):
  TITLE_SIZE 13 / AXIS_TITLE_SIZE 11 (axis.rs's colliding TITLE_SIZE renamed,
  values unchanged), one LABEL_SIZE 11 (was ×4: mod/axis/annot/radial),
  AREA_OPACITY 0.82, TICK_TARGET 5.
- Marker ratios named in markers.rs beside the existing family: ARROW/DATUM/
  DIAMOND_HALF_SPREAD, RING_BACK_ONE/MANY (shared by line-stop + drawn ring).
- `messages::LABEL_SIZE` pub(crate); the `.lini-sequence-message` rule derives
  its `13px` from it. Sweep bonus: prim::DIM_TEXT_SIZE and the
  `.lini-dim-text` rule's `12px` also derive from DRAWING_LINK_FONT_SIZE.

**Deviations:** none of substance. Left local by judgment (AUDIT's list +
in-stage calls): chart `TITLE_SIZE * 1.2` title reserve (spacing, not text
leading), the ER bar glyph offsets (one-off per marker arm), chart
`labels.rs` SIZE 10 (data-label knob, not the tick LABEL_SIZE), the
sequence-tab `12px` (a sequence choice, not the drawing caption), and EPS /
buffers / fmt-config / routing `cost.rs` / `AVG_CHAR_WIDTH_RATIO` per AUDIT.

### Stage R4 — front-end: parser split + one expression lexer

AUDIT R2 (expr) + R5 (parser). Medium risk — snapshots are the oracle.

- [x] Split `syntax/parser.rs` into `syntax/parser/{mod,values,nodes,links,decl,
  selector,classify}.rs` per the AUDIT table — `values.rs` first (isolates the
  comma-law surface for M1); tests to `parser/tests.rs`.
- [x] expr consumes `&[Token]`: `take_group`/`take_arg_expr` hand a token slice;
  delete expr's lexer (expr.rs:110-216) and `tok_str`; fold scientific notation
  into the one number scanner **gated to expression context** (SPEC: sci-notation
  is expression-only; preserve the ident-predicate divergence — `r-1` stays
  subtraction in expressions).

Acceptance: zero snapshot diffs; `cargo test` including every expr fold test;
parser files each < 500 production LOC.
**Log:** 2026-07-10 — **done**, 2 commits, all acceptance met (zero snapshot
diffs — every suite green; fmt/clippy clean; parser production files 84–257 LOC,
all < 500). Items landed:
- expr lexer unified: `expr.rs`'s private lexer / `Tok` / `lex_number` / `push`
  / `tok_str` (110-409) deleted; `Expr::parse` re-lexes through the main lexer's
  new `lexer::lex_expr`, and the Pratt parser consumes `lexer::TokKind`
  directly. Sci-notation folded into the one `lex_number` (gated `expr_mode`);
  the ident-predicate divergence (`r-1` = subtraction) and the signed-number
  divergence (`hatch(45 -45)` = two angles) are preserved by a lexer **mode**,
  not a second lexer — `lex_expr` sets `expr_mode`, under which `-`/`+` are
  operators, `-` isn't an ident char, and newlines are whitespace. The normal
  pass is byte-for-byte unchanged (every gate is `expr_mode`-only), so
  `Value::Expr(String)` / fmt / desugar are untouched.
- parser split: `parser.rs` (1681 LOC) → `parser/{mod,classify,values,decl,
  selector,nodes,links,tests}.rs`; each submodule an `impl Parser` block of
  `pub(super)` methods, primitives + File driver + `Tail`/`Kind`/`BarsCtx` kept
  in `mod.rs`. Faithful move verified by normalize-and-diff (only module
  boilerplate + one fmt-reflowed signature differ) plus the snapshot oracle.

**Deviation:** the expr item's literal "`take_group`/`take_arg_expr` hand a
token slice" (⇒ `Value::Expr` carries tokens) is **infeasible as `[pure]`** —
surfaced and approved before coding. Two reasons: (1) one uniform token stream
can't yield both `hatch(45 -45)` (needs `-45` a signed number) and `(r-1)`
(needs `-` an operator) — today's two lexers resolve this precisely by
re-lexing the identified expression region, so handing the main pass's tokens
would regress one case; (2) fmt and `lini desugar` emit `Value::Expr` as **raw
source verbatim** and `print_file` has no `src`, so token-carrying `Value::Expr`
would force canonical-spaced reconstruction — an `[output]` change plus a new
parallel expr-printer. Re-lexing the isolated region via `lex_expr` achieves
the AUDIT's actual goal ("kill the duplicate lexer + diverged number scanner;
one scanner implements the semantic split") while staying `[pure]`. No
follow-ups.

### Stage R5 — structural splits: layout, chart, desugar, resolve, routing

AUDIT R5. Pure moves; big but mechanical. Split across two sessions if needed
(5a: layout+drawing; 5b: chart+desugar+resolve+routing).

- [x] `layout/mod.rs` → `arrange.rs` + `frame.rs`; mod keeps dispatch +
  `layout_inst`.
- [x] Drawing: `annotate.rs` → `rows.rs`/`paint.rs`; `breaks.rs` →
  `viewmap.rs`/`clip.rs`; `section.rs` → `plane.rs`/`detail.rs`; `pen.rs` →
  `pen/parse.rs`.
- [x] `chart/model.rs` → `chart/model/{types,build,series,axes,annot,paint}.rs`
  (the one L split — `build()`'s shared state gets a small context struct); pie
  bits join `pie.rs`; `chart/mod.rs` tests → `chart/tests.rs`, plot geometry →
  `chart/frame.rs`, legend → `chart/legend.rs`.
- [x] `desugar/mod.rs` → `tables.rs` + smart-label ladder into `labels.rs`.
- [x] `resolve/program.rs` → `theme.rs` + `link_scope.rs`.
- [x] `routing/ortho/world.rs`: extract `build_worlds()` + `world_ladder()` from
  `ortho/mod.rs:158-190` (also the `natural` seam, AUDIT). Convert every
  `Strategy` `==`/`!=` to an exhaustive `match` (D4).
- [x] Adopt the test-LOC convention: big `#[cfg(test)]` blocks to sibling
  `tests.rs` files wherever they dominate (engine.rs, annotate.rs, …).

Acceptance: zero snapshot diffs; every production file ≲ 500 LOC except the
AUDIT-sanctioned holdouts (`place.rs`); module docs updated.
**Log:** 2026-07-10 — **done**, 9 commits, every split a `[pure]` move proved by
the snapshot oracle (zero diffs after each) + a per-file normalize-and-diff (only
module boilerplate / rustfmt reflow / re-export lines differ) + clippy/fmt clean.
Driven by fresh Opus sub-agents per module (one Sonnet for the engine test move),
each verified by me before commit. Every new file carries a `//!` doc. Items:
- `resolve/program.rs` (889) → `program/{mod,theme,link_scope,tests}.rs` (clean
  seam; the two halves never cross-call). `mod link_scope` shadows `fn
  link_scope` → three qualified `link_scope::link_scope(...)` call sites.
- `desugar/mod.rs`: table lowering → new `tables.rs` (307). **See deviation** —
  the label ladder was already in `labels.rs`, so `mod.rs` lands at 608.
- Drawing: `pen.rs`→`pen/{mod,parse}`, `breaks.rs`→`breaks/{mod,viewmap,clip,
  tests}`, `section.rs`→`section/{mod,plane,tests}`, `annotate.rs`→`annotate/
  {mod,rows,tests}`. `engine.rs`→`engine/{mod,tests}` (test-LOC convention).
- `layout/mod.rs` (1329) → `mod` (425, dispatch/`layout_inst`) + `arrange` (260)
  + `frame` (129) + `tests`; `accumulate_extent` re-exported for `ir.rs`.
- `routing`: `ortho/world.rs` = moved `world_ladder` + extracted `build_worlds
  (index, reqs, c)`; D4 — all six `Strategy` ==/!= → exhaustive `match` over
  {Orthogonal, Straight} (7th inside `build_worlds`), proven exhaustive (a third
  variant breaks compilation at every site; `ir.rs` left pristine).
- `chart/model.rs` (1473) → `model/{mod,types,build,series,axes,annot,paint}.rs`
  + pie helpers to `pie.rs`; `chart/mod.rs` (924) → `mod` (137) + `frame` (124)
  + `legend` (77) + `tests` (593).

**Deviations** (all flagged in commits; oracle-neutral):
1. **No context struct** for `chart::build` — the code already threads its state
   through explicit params to standalone helpers (verified by reading `build`),
   so a straight function-group move is correct and lower-risk; adding a struct
   would be unneeded churn ("trust a correct model").
2. **`section`/`annotate` split only their clean half** (`plane.rs`, `rows.rs`);
   the `detail`/`Paint` halves use `super`-relative module paths that break one
   level deeper and can't move verbatim. Both files were already < 500 production
   and every resulting file is < 500, so the LOC goal holds; `detail.rs`/
   `paint.rs` were not separately created.
3. **`desugar/mod.rs` stays at 608** (over ≲ 500). The plan's two prescribed
   extractions were tables **and** the smart-label ladder → `labels.rs`, but the
   ladder was already extracted in an earlier stage, so tables-only leaves the
   driver (`desugar` 151) + `lower_node` (274) dispatch. A `lower_node` → `lower.rs`
   move would land it ~334, but the **owner chose to accept 608** — it's the
   cohesive driver, in line with the many other >500 non-target files (fmt 867,
   scene 605, grid 543, …). Not a holdout to revisit.

**Minor** (not blocking; noted for a later polish pass): the six `model` helpers
`pie.rs` reuses are `pub(crate)` (a `pub(super)` item can't re-export up to
`chart` — E0364); `pub(in crate::layout::chart)` would be tighter.

### Stage R6 — render: one paint chokepoint `[output]`

AUDIT R4. The stage that ends the inline-style whack-a-mole.

- [x] Route text-leaf styling through `inline_paint_diff` against
  `.lini-text`/`.lini-link-label`/`.lini-sequence-message`; delete
  `render_link_text`'s hand-rolled diff and unify with `text_style_attr`. This
  *fixes* the dropped `text-shadow` on link labels — intentional output change.
- [x] `.lini-gutter { stroke: none }` rule (gated on gutters present); hoist
  hatch `<pattern>` line paint onto one `<g>`; stray-glyph classes (optional).
- [x] Split `rules.rs` → model vs `stylesheet.rs` (`build()` into per-family
  sub-builders) `[pure part]`.
- [x] Sweep: assert (grep/review) that no emit site writes paint attributes
  outside the diff or a `.lini-*` rule; add a comment-contract at the chokepoint.

Acceptance: snapshot re-bless limited to the enumerated changes; resvg renders
of `styles.lini`, `gap_fill.lini`, `sequence.lini`, one drawing sample — light +
dark — visually checked.

**Release checkpoint:** with the R phase complete, cut **0.20.1** — pure
internals plus the R1 fmt / R6 leak bugfixes; the language is unchanged.
**Log:** 2026-07-11 — **done**, 4 commits, all acceptance met (re-bless limited
to the enumerated changes — 25 samples, every diff categorized; fmt/test/clippy
clean). Items:
- **Chokepoint `[output]`** (the headline): deleted `text_style_attr` and
  `render_link_text`'s hand-rolled diffs; both node text and link labels now
  route through one shared `text_paint_attr` → `inline_paint_diff`, diffing
  against the leaf's own base class. Fixes the dropped `text-shadow` on link
  labels; `label_size` is gone (the per-role size states once in the sheet rule,
  which also fixes sequence messages over-inlining `font-size`). 15 snapshots
  re-blessed — font props reorder to `PAINT_PROPS` order (charts/text), redundant
  `font-size: 12px` drops from drawing dim text (`.lini-dim-text` provides it),
  and text.lini's link label gains `text-shadow` (coverage added — the one label
  that exercises the fix). Render-neutral proven: drawing_leaders + chart_labels
  render **pixel-identical** old→new (resvg); text.lini eyeballed light + dark.
- **Leak `[output]`**: gutter rects drop per-rect `stroke="none"` for a
  `.lini-gutter { stroke: none }` rule (gated via a `has_gutters` walk); hatch
  `<pattern>` lines drop per-line paint for one wrapping `<g>`. 11 snapshots
  re-blessed, every one a gutter or hatch change; gap_fill + drawing_bushing/
  _sheet + table_cell_style all **pixel-identical** old→new; gap_fill eyeballed.
- **`[pure]` split**: `rules.rs` (858) → model (183: Rule/RuleSet + queries) +
  `stylesheet/{mod (144), families (497), tests}.rs`; `build()` decomposed into
  ten per-family sub-builders in the exact original push order (`live` lifted
  closure→fn). Zero snapshot diffs prove byte-identical output.
- **Sweep**: grep-confirmed no node/link/text element emits paint outside the
  diff; the paint contract is documented at `inline_paint_diff` (the remaining
  inline paints are class-less structural elements — icon roles, drawing chrome,
  `<defs>`, canvas override, gutter fill, marker/stray). Dropped a stale
  `#[allow(too_many_arguments)]` (render_link is 7 args after `label_size` went).

**Deviations:** `stylesheet` landed as a **directory** (`stylesheet/`), not a
flat `stylesheet.rs` — `build()` alone is 460 lines, so a flat file can't meet
< 500 with its helpers; a directory is the house pattern (matches
`resolve/program/`, `chart/model/`). The **stray-glyph classes** item was marked
optional (AUDIT "low priority") and left as-is — the stray's diagnostic paint is
a class-less one-off, correctly in the sweep's sanctioned-exception list.

**Release checkpoint:** R phase (R1–R6) is now complete — **0.20.1 is ready to
cut** (pure internals + the R1 fmt / R6 leak bugfixes; language unchanged).
Deferred to the owner (version bump + tag + push).

---

## Phase S — SPEC work

### Stage S1 — SPEC tightening (editorial, no semantic change)

- [x] Tighten SPEC.md's prose: dedupe restated rules (Part I is authoritative;
  Part II/III reference it), compress examples that repeat a point, keep every
  normative statement. Target: meaningfully shorter with zero information loss.
- [x] Verify losslessness: work section-by-section; a second pass (or an agent
  with the old text) diffs for dropped normative content; the R2 ledger
  cross-check list feeds errata fixes here.
- [x] Update stale cross-file claims found in the audit: SPEC 10.5's "one place"
  wording (now true via `ledger/`), ROUTING.md's implementation-shape file map
  and the over-broad "every strategy is validated" line. (ROUTING.md's `curved`
  row is replaced in alpha.1, not now.)

Acceptance: SPEC builds the same language — no sample, snapshot, or test changes;
a re-read of sections 1–24 confirms every table survived.
**Log:** 2026-07-10 — **done**, 5 commits, all acceptance met (no sample /
snapshot / test change — fmt/test/clippy clean; every table survived, verified
mechanically: the only table deltas are the intentional +text-shadow row and
the deleted 15.10 table). SPEC.md 3806 → 3603 lines / 225.4 → 215.6 KB (−5.3%).
- Errata first (their own commit): the four R2 cross-check items — SPEC 16
  gains the `text-shadow` row, loses the false padding-longhand claim, flags
  the `sheet` homonym; SPEC 10.5's "one place" now points at the ledger.
  `legend:` deliberately left for S2 (build-vs-defer decision).
- Tighten, Part I: header model paras folded into SPEC 1; bullet lists →
  prose; the class-follows rule states once (was 4×); SPEC 9's
  clearance/routing scene-config law states once (was 3×); 10.7's example
  blocks merged. One correction folded in: SPEC 3's text-valid list gains
  `fill` + `text-shadow` (matches the ledger/code).
- Tighten, Parts II–III: SPEC 15.10's property table deleted — every row was a
  third statement (law in 15.x, index in SPEC 16); verified row by row before
  and after. Micro-trims 14.2/15.3; SPEC 24 keeps one worked example per
  family and points at `samples/` for the gallery (dropped blocks duplicated
  SPEC 8/9/13/15.x examples + tested samples).
- ROUTING.md: "validation" out of the shared-spine list (the checker judges
  orthogonal wires only — matches validate.rs), stated explicitly; file map
  updated to today's modules; `curved` row untouched per plan. The same
  over-broad sentence in src/routing/mod.rs's doc fixed to match.
- Losslessness verified: four Opus agents diffed old vs new per region
  (Pre–4, 5–8, 9–10, 11–24 + ROUTING), each chasing the cross-references the
  new text leans on and re-checking the 15.10 deletion row by row — all four
  returned zero dropped/weakened normative statements. Anchor/link sets
  checked mechanically (only `15.10 Properties` heading removed; nothing
  links to it).

### Stage S2 — the 0.21 SPEC amendment (Stage-0 of the breaking round)

Write all breaking-change SPEC text before any migration code, per the repo's
Stage-0 pattern. Sources: ROADMAP 3.1 (+ 3.4's row/radial items).

- [x] The comma law (SPEC 2 value grammar + every affected property's entry +
  SPEC 21 grammar notes + migration examples). Include the pipeline clause and
  the `data: 10 20`-is-a-point consequence.
- [x] Property validation (SPEC 16 rewrite: the strict/lenient rule; SPEC 20 new
  error/warning tables; SPEC 23 un-defers the warning).
- [x] Similarity-based implicit warning (SPEC 3 implicit nodes).
- [x] `max-width`/`text-wrap` + align-driven line alignment (SPEC 5/6/12; the
  no-`text-align` rationale; wrapper-block escape).
- [x] scale/unit/density (SPEC 15.1 rewrite + 15.8 page + 10.5 constants + the
  desugar fold in SPEC 18; `unit:` value shape decision: ident enum `mm cm m in`,
  suffix display noted as `format:`'s future job).
- [x] `place:` (SPEC 13); renames (`labels:`, title-block fields + smart label,
  SPEC 8/14/15.8); row bands/marks + radial error (SPEC 14.5/14.7, the "never
  silently lossy" wording); root-drawing/sequence routing fix (SPEC 11/15 notes).
- [x] The local-bug decisions (ROADMAP 3.1): chain ops mark **every hop** and
  chain expansion is a desugar job (SPEC 9 + SPEC 18; `a - b -> c` is the bare
  first-hop spelling); `|page|` direction defaults by orientation (SPEC 15.8);
  `stroke-style: wavy` link-only — delete the closed-primitive deferral
  (SPEC 7/16/23); SPEC 19's fmt paragraph notes a styled cell breaks its row
  out of the aligned grid.
- [x] Fonts (ROADMAP 3.7): SPEC 5's text-measurement paragraph rewritten
  (metrics tables, metrics-by-kind, no-kerning note, unknown-glyph fallback,
  cap-height centering); SPEC 6/10.1 — the two bundled families, the stack,
  override semantics (name changes, metrics stay by kind), the widened
  `font-weight` set (`normal|medium|semibold|bold|400|500|600|700`); SPEC 17
  output modes; SPEC 19 — `--bake-vars` → `--static`, new `--embed-font`
  (browser-only note); SPEC 23 — the embedded-font-metrics deferral comes out,
  arbitrary 100–900 weights and kerning stay in.
- [x] SPEC 23 updated: remove what 0.21 builds, add the new deferrals from
  ROADMAP 6.

Acceptance: SPEC alone is sufficient to implement M1–M4; every example in SPEC
already uses the new syntax.
**Log:** 2026-07-10 — **done**, 7 commits, all items landed; every SPEC example
uses the new syntax (swept: comma lists, `place:`, `labels:`, the title-block
field names, ratio-form scales, `--static`; anchor/link integrity re-checked;
tests untouched/green — SPEC-only stage). Per item:
- Comma law: SPEC 2 states it once (comma = repeated items, space = one item's
  components, pipelines fold space-separated, `data: 10 20` is one point);
  SPEC 21 notes reader-enforced migration errors; SPEC 14.3's discriminator is
  item width now.
- Validation: SPEC 16's four-clause strict/lenient rule replaces
  silently-ignored (hard gates fold into the strict clause); SPEC 20 gains the
  Properties & validation table; the similarity warning is in SPEC 3 + 20;
  SPEC 23 un-defers the warning. `legend:` honestly marked ⌛ (the R2
  cross-check leftover decided: defer, not build).
- Wrap/align: SPEC 5 `max-width`/`text-wrap` law block; SPEC 6 line-alignment
  law (+ the no-`text-align` rationale, wrapper-block escape); SPEC 12 tie-in;
  SPEC 16 rows; SPEC 20 the three errors.
- scale/unit/density: SPEC 15.1 three-quantity model (`unit:` = ident enum
  `mm cm m in`, suffix display noted as `format:`'s job); `|page|` loses
  `scale:`; titles read the ratio directly; SPEC 18 states the desugar fold;
  absurd-extent hint in SPEC 20; SPEC 8/10.5/16 rows updated.
- `place:` + renames: one property replaces over/left/right; `tags:`→`labels:`
  (the deferred per-axis tick text loses the colliding name); title-block
  fields to full words (kills the `sheet` homonym) + smart-label-as-title;
  SPEC 11/15 state nested ordinary scopes route their own links (M6's fix).
- Local bugs: chain-marks-every-hop + desugar expansion (`a - b -> c` bare
  first hop); `|page|` direction by orientation; `wavy` link-only by design
  (deferral deleted); fmt styled-cell rule documented.
- Fonts: SPEC 5 metrics measurement (kind-not-name, 0.6em mono invariant,
  cap-height centering); SPEC 6 the two OFL families + widened `font-weight`;
  SPEC 17 three output modes (embed browser-only, `--static` outlines);
  `--bake-vars`→`--static` everywhere, `--embed-font` added (SPEC 10.6/19).
- SPEC 23 sweep: built items out; `format:` entered as decided-additive;
  per-datum paint in comma shape; beyond-1.0 tail (DAG, imports, animation,
  raster) reserves no syntax.

**Deviations:** none from ROADMAP; two judgment calls logged — the deferred
per-axis tick text keeps no property name (avoids the new `labels:` collision;
alpha.2 names it), and SPEC 23 gained a compact beyond-1.0 tail rather than
per-item entries. Sonnet agents unused — all writing was judgment-bearing; the
S1-style verification pass was replaced by targeted greps + anchor script since
S2 is additive amendment, not lossless compression.

---

## Phase M — the 0.21 breaking migration

### Stage M0 — the sample garden `[output: snapshots renamed/regenerated]`

Consolidate ~50 samples to ~25 **before** the M-phase migrations multiply the
cost of each one (every sample gets hand-migrated at M1 and M3, re-blessed at
M5, re-reviewed at M7). Samples are the showroom: every survivor is polished,
idiomatic lini a learner can copy.

- [ ] Keep as-is: `hello`, `hero`, `chart_hero`, `entity_hero`, `icons`,
  `palette`, `sequence`, `shapes` (README asset sources), `links`,
  `links_simple`/`links_medium`/`links_hard`, `pcb` (routing-oracle scenes),
  `sketch`, `templates`, `layout`, `styles`, `expr`, `desugar`.
- [ ] Merges — final judgment in-stage; each merged file must read as **one
  coherent scene**, not a concatenation:
  - `charts.lini` ← chart_bars + chart_lines + chart_points + chart_pie;
    `chart_advanced.lini` ← chart_axes + chart_fn + chart_annotations +
    chart_labels + chart_radial.
  - `drawing_turned.lini` ← tiebar + shaft + barrel; `drawing_section.lini` ←
    bushing + ring + detail; `drawing_assembly.lini` ← assembly + mate + pump
    (**rework** — it reads as bad work today); `drawing_annotations.lini` ←
    dims + dim_style + leaders (**rework**) + pattern + drawing.lini.
    `drawing_sheet` and `drawing_screw` stay (the flagship + the stress bar).
  - `paint.lini` ← gradient + gap_fill + themes; `text_tables.lini` ← text +
    table_align + table_cell_style (**must keep a styled cell** — R1's
    regression coverage; move its focused test's path).
- [ ] Update every by-name test reference; snapshot set follows (deletes +
  renames — regenerate, never hand-edit); conformance / oracle / fmt / laws
  sweeps stay green; README images unchanged (all sources are keepers).
- [ ] AGENTS.md: replace "One sample per feature in `samples/`" with the
  cluster policy — *samples are the showroom; one sample per feature cluster;
  extend an existing sample before adding a file.*
- [ ] Finish with a PNG contact sheet (resvg, light + dark) of every surviving
  sample for owner review — the gate is "code you'd want a stranger to learn
  lini from."

Acceptance: ~25 samples; all sweeps green; contact sheet reviewed; zero README
asset drift.
**Log:** 2026-07-11 — **done**, 1 commit. 50 samples → **29** (21 keepers + 8
merged scenes; the plan's own merge table lands at 29, its "~25" was the
estimate). Every merged file written as one coherent scene and verified by
render (light + dark PNGs eyeballed): charts = one analytics dashboard;
chart_advanced = one telemetry board; drawing_turned = three revolved parts
on one board; drawing_section = an A4 sheet (front + section A–A + detail C);
drawing_assembly reworked (nested rigid body handle, dot-path mate into it,
pressed fits with negative gaps, opposite hatch angles on adjacent parts,
face-anchored balloons, BOM); drawing_annotations = one fixture's two parts
carrying the full dim/leader vocabulary + the (-) selector; paint = gradients
+ gap-fill + themed group; text_tables keeps the styled cell (R1 coverage)
and the link-label text-shadow (R6 coverage). Snapshots regenerated via
`cargo insta test --accept --unreferenced=delete`; all sweeps green
(parsing/conformance/fmt/oracle/laws are directory-driven — the only by-name
refs are keepers). AGENTS.md swapped to the cluster policy. Contact sheet
(light + dark) rendered and delivered for owner review.

**Deviations:** in-stage judgment calls — drawing_section sits on **a4
landscape, gap 24, scales 4/4/12** (a3 was sparse; 4/4/12 keeps the composed
titles at clean 1:1 / 3:1 — the ratio composes over the page's default 4);
the detail view is a **bare re-render** (`of: c` with no children): a `(o)`
on the radially-patterned port dims at the pattern's ring centre — outside
the clipped region — so the detail carries no dimension (the original
drawing_detail dimensioned a sketch *station*, which the ring's oval
geometry doesn't have). drawing.lini's washer and drawing_pattern's flange
were folded as covered features (concentric stack + radial pattern live in
drawing_section's front view; grid pattern in drawing_annotations' plate).

### Stage M1 — the comma law `[breaking]`

AUDIT D7 + seam table. The parser is ready; this is the reader flip + migration.

- [ ] `resolve/value.rs::resolve_groups` + `ResolvedValue` semantics per S2:
  list-typed properties read across comma-groups; tuple-typed stay single-group.
  Drive list-vs-tuple from the R2 ledger's `shape` column.
- [ ] Flip the readers: chart `read_data` (list-of-scalars → categorical,
  list-of-tuples → points), `categories`, `ticks`, `along`, grid `columns`/`rows`
  (`desugar/mod.rs:237`), per-column `align`/`justify`, segmented `fn:`,
  `thread:`/`break:` groups. Pipelines (`draw:`, `mirror:`) assert single-group.
- [ ] Targeted legacy errors in each list reader ("`data` takes comma-separated
  values — `data: 9, 15, 24`").
- [ ] Migrate all ~49 samples by hand (Sonnet agents fine), re-bless snapshots,
  spot-check `fmt` (`emit_decl` already prints the law; add a `data:` case to
  `tests/fmt.rs`) and the desugar oracle.

Acceptance: all tests green; `lini fmt --check` clean over samples; conformance
PNGs of the chart samples visually identical to before (the law changes syntax,
not pixels).
**Log:** 2026-07-11 — **done**, 1 commit, all acceptance met (14 suites green;
`lini fmt --check` clean over all 29 samples; charts + chart_advanced PNGs
**pixel-identical** old→new via `cmp`; conformance snapshots needed **zero**
re-bless — the SVG is byte-stable). The mechanism: `resolve_property` is the
one chokepoint — it normalizes every list-shaped property (ledger `shape`
column) to a `ResolvedValue::List` and centrally rejects legacy space lists
for scalar-kind lists (Str/Ident/Track) with the targeted migration spelling;
Number-kind lists pass through to their readers (`data:`/`points:` items are
legitimately pairs — a lone `data: 10 20` is one point, on bars an error).
Readers flipped: chart `read_data`/`collect_strings`/ticks, link `along`,
segmented `fn:` (comma per-band), grid tracks + `track_align`/`ident`
(flex + grid), desugar `per_column` (the table align/justify distributor —
an AST-level reader the resolve chokepoint can't see, caught by test),
threads/breaks groups, hole thread pitch; `mirror:`/`draw:` assert one
space-separated pipeline run. Generated decls follow the law (entity
`columns`, title-block columns, auto-`along`). Samples + test sources
migrated by a quote/paren-aware script (its overreach into Rust comments was
reverted surgically), then samples canonicalized with `lini fmt`.

**Deviations:** none of substance. The plan's "re-bless snapshots" proved
unnecessary — output is byte-identical, the stronger result. One pre-existing
fmt nit surfaced (not introduced): `tol: +0.2 -0.05` canonicalizes to
`tol: 0.2 -0.05` (same parsed value; semantic-preservation sweep guards it).

### Stage M2 — validation + the similarity warning `[diagnostics]`

- [ ] The owner-aware pass (new `src/validate.rs` or grown `lint.rs`), reading
  the ledger: unknown name → error + `suggest::nearest`; statically-known owner
  misuse → error with contextual correction; class rules → inert / dead-warning /
  unused-class warning; value-shape errors. Wire `--strict`/`--no-warn`.
- [ ] Similarity implicit-node warning at `lint.rs:127` via `suggest::nearest`
  (edit distance ≤ 2 or case-fold, vs declared + previously-created names in
  scope). Remove nothing else — shadow warning stays.
- [ ] Sweep the samples: fix any latent misuse the new pass finds (each is a
  finding — list them in the Log).
- [ ] Tests: one `insta` family per diagnostic; the CLI-binary `--strict`
  exit-code test (AUDIT R6); update SPEC 20 table ↔ implemented messages 1:1.

Acceptance: every diagnostic in SPEC 20's new tables demonstrably fires; no
false positives across samples; `cat -> dog -> bird` stays warning-free.
**Log:** 2026-07-11 — **done**, 1 commit, all acceptance met (every SPEC 20
validation diagnostic fires in `tests/validation.rs` — one inline-insta case
per diagnostic, exact CLI rendering pinned; the 29 samples **and their
`lini desugar` forms** sweep clean — zero diagnostics; `cat -> dog -> bird`
silent, pinned by test; 15 suites + fmt + clippy clean). Items:
- New `src/validate.rs` (lint.rs stays advisory-only): unknown name → error +
  ledger-driven did-you-mean; wearer-known misuse → contextual error
  (type-/role-owned, root-only config; `cell`/`span` gated on the static
  parent layout, sequence placement on a sequence — any stylesheet rule
  injecting `layout:` stands the static checks down); class rules warn only
  dead-on-every-wearer + never-worn; static value shapes (`opacity` 0..1,
  `translate` 'x y', comma list on a one-value property). Exposed through
  `lint_str` (one parse, one diagnostics stream).
- Similarity warning at the lint auto-create loop via `suggest::nearest`,
  vs declared + previously-created names; case-fold unconditional; shadow
  warning untouched.
- `Level::Error` lands (the R1-deferred severity variant); CLI: error
  diagnostics always print + exit 1, `--no-warn` silences warnings only,
  `--strict` promotes them — the AUDIT R6 CLI-binary exit-code test included.
- SPEC 20 ↔ messages reconciled 1:1 (misuse wording → "it reads on …", the
  inert-class message shortened, the one-value comma message added); SPEC 3's
  near-miss law gains the noise guards below.
- Sample sweep findings: **none** — no latent misuse in the M0 garden; the
  sweep instead surfaced three infrastructure gaps, all fixed: the ledger
  lacked a `density` row and the `marker*` rows lacked their chart owners
  (`|mark|`, series) — cross-check finding; and desugared files needed their
  worn `.lini-*` classes folded into the wearer's chain (+ the lint
  drawing-scope skip reading `.lini-drawing`) to round-trip warning-free.

**Deviations** (all toward "no false positives", logged as SPEC 3 wording
updates): the near-miss typo distance must additionally be **shorter than the
id** (so `a -> b` stays silent) and **numbered siblings are exempt**
(`server`/`server2` is a family, not a typo). Generated `.lini-*` classes are
exempt from the class-rule warnings (compiler-authored, worn implicitly at
resolve). The ledger's `gate` column stays data — the hard-gated props are
enforced by the named checks; deeper gate-driven validation can ride a later
round with schema generation.

### Stage M3 — scale/unit/density + `place:` + renames `[breaking]`

- [ ] Desugar fold (AUDIT seam): `scale:` ratio × `unit:` mm-size × root density
  (default 4) → the engine's internal px-per-unit; `|page|` loses `scale:`;
  `unit:` becomes the inheriting ident enum; pixel-space outside drawing scopes
  confirmed by test (`right(300)` = 300px in flow). Absurd-extent hint
  diagnostic. Section/detail/view title ratios read the new `scale:` directly.
- [ ] `place:` replaces `over`/`left`/`right` (desugar + resolve + sequence
  engine + errors); old names become unknown-property errors (M2 catches them).
- [ ] Renames: `tags:` → `labels:` (with the R2 reconciliation); title-block
  field renames + smart-label-as-title (lowers to the generated spanning cell).
- [ ] Migrate samples (drawing samples get ratio-form scales — e.g. old
  `scale: 6` on a default page becomes `scale: 1.5`), re-bless, and **visually
  verify every drawing sample at print scale** (resvg, mm sizes intact).

Acceptance: all drawing samples render byte-stable after migration except where
the amendment says otherwise; `lini desugar` shows folded scales; no `dwg`/
`rev`/`tags`/`over` spelling remains anywhere in repo.

**Release checkpoint:** M1–M3 land together, never separately — with them in,
the breaking core is coherent: cut **0.21.0**.
**Log:** 2026-07-11 — **done**, 1 commit, all acceptance met (15 suites +
fmt + clippy clean; `lini desugar` shows the folded number beside the
authored ratio; grep confirms no `dwg`/`rev`/`tags`/`over` property spelling
in code, samples, SPEC, or README; drawing samples byte-stable except the
enumerated dim-text change; print-scale mm attrs verified A4/A3). Items:
- **The fold**: new `desugar/scale.rs` stamps a generated internal
  `px-per-unit:` (ratio × unit-mm × density) on every drawing scope, and the
  density alone on pages (`scale:` on a page errors); recomputed per pass —
  desugar stays idempotent (pinned by test). `effective_scale` prefers the
  stamp; the drawing/page bundles and the root-drawing default lose their
  `scale: 4` (the fold owns it). Titles read the authored ratio directly;
  pages-only mm emission reads the stamp. Absurd-extent hint lands
  (`ABSURD_EXTENT_PX` in consts; root and nested views; silent at the honest
  ratio — pinned both ways). Pixel-space outside drawing scopes pinned
  (`right(300)` = 300 px in flow).
- `unit:` = inheriting ident enum (`mm cm m in`, walked by the fold through
  pages into views); the axis homonym keeps its quoted suffix, each reader
  validating its own (ledger shape → `Any`).
- **Suffix decision executed**: measured values read **bare pre-scale
  numbers** — SPEC 15.1 (S2's decision: suffix display is `format:`'s future
  job) contradicted leftovers in SPEC 15.6/15.9 ("unit: appends its suffix");
  resolved per the S2 log, the two leftovers fixed as errata. This is the
  one visible output change: three samples' dim texts lose " mm".
- `place:` replaces over/left/right (ledger row, sequence reader with the
  SPEC 20 mode error, M2 gate, tests); `tags:` → `labels:` (reader, messages
  "'labels' has N entries…", ledger row now series-only per S2 — the
  deferred axis tick text keeps no name — internal Series field renamed);
  title-block fields to full words + `sheet-number` (the `sheet` homonym row
  dies); the block's smart label lowers to its `title` field.
- Samples migrated: view scales ÷4 (byte-stable — turned/annotations at
  0.75/0.5, screw at 0.5/1.5, section/sheet land on clean 1/2/3), quoted
  units → idents, `labels:`, `place:`, full-word fields; all fmt-canonical.
  drawing_sheet + drawing_section pixel-identical to pre-M3; turned/
  annotations/screw re-blessed for the suffix drop only, eyeballed light +
  dark. Test snippets spell multiplier intent as `density: 1` + the ratio.

**Deviations / interpretations** (flagged, none silent): (1) the fold applies
to **authored decls** — a rule-borne `scale:` on a drawing stays an engine
multiplier (AST-level pass; same static philosophy as M2); (2) sheet chrome
(`|note|`/`|balloon|`/`|table|` bundles at `scale: 1`) keeps engine-multiplier
semantics per SPEC 8/15.1's own "annotations are sheet chrome" — an *authored*
node-level `scale:` inside a view folds as a ratio; (3) the 15.6/15.9-vs-15.1
suffix contradiction resolved in 15.1's favour (see above) — flagged for
owner review rather than pre-approved.

**Release checkpoint note:** M1–M3 are in — **0.21.0 is ready to cut**
(version bump + tag + push deferred to the owner, as with 0.20.1).

### Stage M4 — text wrap + line alignment `[feature]`

- [ ] `text.rs`: line-list API (wrap at whitespace within `max-width`, grapheme
  fallback), scalar API preserved as a wrapper; `leaf_bbox` sizes from lines;
  render emits per-line positions.
- [ ] One shared line-align resolver (nearest container's horizontal packing
  knob; start/center/end; others read center) called by **both** flex and grid —
  `grid::align_cell_content` becomes a caller (AUDIT's parallel-impl trap).
- [ ] Errors: `nowrap` can't-fit; non-text child wider than `max-width`;
  `width > max-width`. Wrapped sizes feed tracks/gutters/labels/routing
  obstacles (bbox-driven — verify with a routed sample).
- [ ] Samples: one wrap sample (a card grid with long labels); table alignment
  samples re-verified; snapshots for wrap + align families.

Acceptance: default output unchanged (center = today); wrap sample renders
correctly at light/dark; no measurement caller bypasses the new API.
**Log:** 2026-07-11 — **done**, 1 commit, all acceptance met (15 suites +
fmt + clippy clean; **zero snapshot diffs** except the deliberately extended
sample — default output is byte-identical, centre = today; the card grid
eyeballed light + dark; every width/height reader still funnels through
`text::approx_*`, with `text::wrap` beside them the one line-breaker and
render's per-line anchoring computed through the same API). Items:
- `text::wrap` (whitespace-preferred, in-word char-boundary fallback, ≥ 1
  glyph per line, authored `\n` lines wrap independently, blank lines
  survive); the scalar APIs stand over the `\n`-joined result, so **the
  wrapped size is the measured size**.
- New `layout/wrap.rs` pass: after children lay out, before the container
  arranges — rewrites text leaves' labels with the breaks and re-measures;
  render's existing per-line `<tspan>` path emits the positions. Wrapped
  sizes demonstrably feed auto width, grid tracks, and routing obstacles
  (route test). The three SPEC 20 errors land verbatim; pinned overlays are
  exempt from the cap (they never grow the parent). Ledger gains the
  `max-width` / `text-wrap` rows — missing since R2 (cross-check find).
- One shared line-align resolver (`layout::line_align_of` + the stamp):
  flex (row → `justify`, column → `align`), grid tracks (per column), and
  `grid::align_cell_content` — now a caller for its own slide as well, per
  the AUDIT parallel-impl trap. Non-centre resolutions ride a
  layout-generated `line-align` attr; `render/text.rs` anchors each line.
- Sample: `text_tables.lini` gains the wrapped card grid (start / centre /
  end flush visible); its tables re-verify the align families and its links
  re-judge the wrapped card as an obstacle under the laws sweep.

**Deviations:** none of substance. The plan's "line-list API" landed as
`wrap() -> Vec<String>` + the unchanged scalar measurements over the joined
lines (one measurement home, no parallel line-metrics struct); in-word
breaks use `char` boundaries until M5's real metrics (comment flags it);
the wrap sample extends `text_tables.lini` per the M0 cluster policy rather
than adding a file.

### Stage M5 — fonts: real metrics, subsets & `--static` `[output]`

ROADMAP 3.7. Raw statics are **committed** at `assets/fonts/raw/` (four roman
statics + OFL per family; excluded from the cargo package); OFL texts in
`LICENSES/`.

- [ ] `xtask extract-fonts` (mirrors `extract-icons`): read the committed raws
  (filenames pinned in xtask); generate **(a)** metrics tables —
  per-glyph advances, ascent/descent/cap-height, upem, per family × weight
  {400, 500, 600, 700} — as generated Rust, **always compiled in** (never
  behind the feature: layout must not vary by build flags); assert the mono
  advance is uniformly 0.6em at every weight; **(b)** subset TTFs — ASCII +
  Latin-1 + Latin-Ext-A + General Punctuation + the drafting symbols lini
  composes (`⌀ ° ± ×` …) — via the pure-Rust `subsetter` crate, OFL copyright
  metadata kept, committed under `assets/fonts/`. **Budget ≤ 600KB total** —
  trim charset first, weights second; record the real numbers in the Log.
- [ ] `font` cargo feature (default-on, mirrors `icons`) gating the subset
  *bytes* only; `--embed-font` / `--static` outlining error helpfully without
  it; default name-only output works under `--no-default-features`.
- [ ] Measurement: `text.rs` swaps the flat 0.6 ratio for table lookups —
  width = Σ advances(kind, resolved weight) × size/upem + letter-spacing.
  Kind = known-mono names + a "mono" substring heuristic, else proportional;
  unknown-glyph fallback (wide for CJK ranges). Unit-test: mono widths equal
  the old estimate exactly. Vertical centering moves to cap-height optical
  centering `[output — the one full re-bless; visual pass over every sample,
  light + dark]`.
- [ ] `font-weight` widens to `normal|medium|semibold|bold|400|500|600|700`
  (ledger row; SPEC landed in S2). Measurement reads the resolved weight.
  The chrome bold→semibold retune is decided here **by eye** (layout-neutral
  for mono — advances are weight-invariant).
- [ ] Emission: the default stack leads with the bundled family names
  (rules.rs, one place); `--embed-font` = base64 `@font-face` of the
  family × weights actually used, under Lini-scoped family names;
  `--static` renames `--bake-vars` (breaking, no alias) and adds text→path
  outlining via `ttf-parser` on the subsets (glyph dedupe with
  `<defs>`/`<use>`; italic = synthetic oblique).
- [ ] Samples: one proportional sample (a Google Sans card diagram); re-verify
  a text-heavy existing sample through `--static`. From this stage on, visual
  PNG reviews render via `--static` so resvg needs no installed fonts.
- [ ] Re-verify the pivotal constraint: resvg still ignores `@font-face`
  (keeps `--embed-font` documented browser-only; last verified on resvg 0.47);
  outlined PNGs pixel-stable across machines.

Acceptance: existing mono sample *widths* byte-identical (the vertical
re-bless is the only geometry delta, visually verified); payload under budget;
`cargo test` green with **and without** `--no-default-features`.
**Log:** 2026-07-11 — **done**, 3 commits, all acceptance met (mono widths
byte-identical — zero snapshot diffs on the measurement swap except the one
enumerated fix; payload **453 KB** of the 600 KB budget — mono 4 × ~28 KB,
prop 4 × ~84 KB, 432-char charset; suites green with fonts off via
`--no-default-features --features icons` — the icons-off conformance failure
predates M5, hero.lini needs icons). Items:
- **`xtask extract-fonts`**: metrics tables (`src/font/metrics.rs`, always
  compiled in) + subset TTFs from the committed raws. `subsetter` pinned at
  **0.1** deliberately — 0.2 strips `cmap` (PDF-only scope), which would break
  browser `@font-face`; and 0.1's verbatim-cmap flaw (dropped-coverage chars
  → invisible zero-advance glyphs instead of system fallback) is fixed by
  **rebuilding a minimal format-12 cmap** over exactly the kept charset, with
  table + whole-font checksums recomputed and verified. OFL copyright/license
  records verified retained. Mono 0.6 em invariant asserted in xtask **and**
  unit tests (upem 2000, advance 1200, all four weights).
- **Measurement**: `approx_width`/`wrap` take a `Font` (kind × weight);
  uniform-advance lines (all mono text) use the historic closed form —
  bit-exact, the fold only for proportional. Resolve stamps the effective
  font on `ResolvedInst` (only resolve sees the inheritance chain); engines
  thread kind through their carriers (`Chart`/`Pie.font_kind`, drawing
  `Paint.font`) with per-run weights (legend + sequence tab measure bold,
  mirroring their rules). One enumerated fix: styles.lini's `font-family:
  serif` box now measures proportional (the ROADMAP headline fix),
  re-blessed + eyeballed. `⌀` (U+2300, in neither face) measures and
  outlines as its covered twin `Ø`.
- **Cap-height centring `[output]`** (the one full re-bless): a baked
  `dy="0.358em"` (both families cap at exactly 0.716 em) replaces
  `dominant-baseline: central` — renderer-independent optical centring; the
  three class rules drop the property. All 33 snapshots re-blessed; visual
  pass over **every** sample light + dark (58 PNGs — keystones by me, the
  rest by an Opus reviewer): zero anomalies.
- **Weights**: `normal|medium|semibold|bold|400|500|600|700` — ledger row
  ident-or-number; `medium`/`semibold` emit valid CSS (500/600) at the one
  `css_value` chokepoint. Chrome decision **by eye: bold stays** (no
  semibold retune).
- **Emission**: default stack leads with "Google Sans Code"; `--static`
  renames `--bake-vars` (no alias, CLI-test-pinned) **and outlines text** —
  the one text emitter swaps `<text>` for `<use>`-deduped glyph path defs,
  resolving each leaf's font through the CSS cascade (inline → class rules →
  root), with `text-transform` baked into the glyph choice, decoration bands
  from the face's own metrics, italic as per-glyph synthetic oblique;
  `--embed-font` inlines used faces as base64 `@font-face` under Lini-scoped
  names, every bundled-family stack led with the scoped twin. Both flags
  error helpfully without the default-on `font` feature (subset *bytes* only
  — metrics always compile in).
- **Samples**: new `samples/cards.lini` (Google Sans card diagram — the
  plan named this a new file; semibold/medium hierarchy, wrapped copy),
  verified light + dark; styles + drawing_sheet re-verified through
  `--static`. PNG reviews render via `--static` from this stage on.
- **resvg re-verified** on 0.47.0: "The @font-face rule is not supported.
  Skipped." — `--embed-font` stays browser-only; outlining is deterministic
  (std float formatting), so static PNGs are machine-stable.

**Deviations:** conformance snapshots now carry the outlined text (the suite
compiles with `static_mode`, whose semantics grew per SPEC) — 26 re-blessed,
and the suite skips under fonts-off like the icons skip; six rendering tests
pinning `<text>`/tspan mechanics flipped to live mode (that path is now the
live one). The plan's "always compiled in" metrics hold: layout never varies
by build flags, proven by the fonts-off suite run.

### Stage M6 — hardening fixes + row bands/marks `[fixes]`

- [ ] Root-drawing router: `layout/mod.rs:41-46` → `routing::route(…)`; root-
  sequence arm routes + extends message wires. Regression samples: a wire in a
  nested `|row|` under each root layout.
- [ ] Scoped note rules (`|sequence| |note|`, `|drawing| |note|`) move to desugar
  as generated descendant rules; desugar snapshots re-bless; SVG byte-identical
  (oracle test proves it).
- [ ] Row-direction bands/marks: make `chart/annot.rs` direction-aware via
  `Plot` helpers (AUDIT: the only column-hardwired file); radial band/mark →
  compile error per S2. Sample: a row chart with a band + mark.
- [ ] Chain markers per hop `[output]`: `a -> b -> c` draws two fully-marked
  links; move chain expansion from resolve to **desugar** (`a -> b; b -> c`,
  auto-created ids included — verified viable by hand-splitting). Fan-out
  (`&`) is untouched — it stays a resolve/routing concept. Re-bless affected
  snapshots; regression samples: `a -> b -> c`, `a <- b <-> c`, and a chain
  mixed with a fan.
- [ ] `|page|` direction defaults by orientation `[output]` (landscape → row,
  portrait → column); re-verify the sheet samples visually.
- [ ] Wavy-on-nodes: confirm no code path exists beyond links (render/wavy.rs
  serves links only); delete any partial shape/drawing support found. SPEC-side
  removal already landed in S2.
- [ ] CLI cleanup: remove `--standalone`; unify the four hand-rolled subcommand
  parsers under `clap::Subcommand`; serve dir-mode boundary generalized past
  `.lini` (prep for alpha.5 images; file-mode boundary noted for that round).

Acceptance: nested-wire samples route lawfully (laws oracle); row-chart sample
visually correct; `lini fmt/desugar/serve/theme --help` outputs unchanged in
substance.
**Log:**

### Stage M7 — release 0.22 → tag `1.0.0-alpha`

- [ ] Full sweep: `cargo fmt` / `test` / `clippy`; `lini fmt` over every sample
  committed clean; desugar + laws oracles green; every sample rendered to PNG
  and eyeballed (Sonnet agents render, Opus/main reviews), light + dark.
- [ ] README + `lini theme`/CLI docs updated to the new syntax; AUDIT.md deleted
  (its stages landed); ROUTING-LOG.md entry for the routing-adjacent changes.
- [ ] Version: release **0.22.0** (0.21.0 was cut at the M3 checkpoint), then
  tag `1.0.0-alpha` on the same tree — the syntax-frozen marker PLAN-V1 builds
  on. Each PLAN-V1 round then publishes its prerelease (`1.0.0-alpha.N`,
  `beta.N`, `rc.N`) as part of its round-complete sweep.
- [ ] Retro pass over this file: unfinished items either done or explicitly
  moved to PLAN-V1 rounds; Log lines complete.

**Log:**

---

## Order & dependencies

```
R1 ──► R2 ──► R3        (suggest → ledger → consts)
R4, R5, R6              (independent of R1–R3 except R5's Strategy matches; any order)
S1 ──► S2               (tighten, then amend)
M0 before M1            (samples-only; halves every later migration — runnable
                         any time after R1, suggested right after S2)
S2 ──► M1 ──► M2 ──► M3 (amendment first; M2 needs M1's ledger shapes; M3 needs M2's errors)
M4 ──► M5 after S2      (M4 rewrites text.rs's line API; M5's metrics swap the advance
                         math inside it; M4 before alpha.1 — mindmap topics want wrap)
M6 after S2             (independent fixes; any time in the M phase)
M7 last                 (releases: 0.20.1 after R6 · 0.21.0 after M3 · 0.22.0 + the
                         1.0.0-alpha tag at M7)
```

R-stages and S1 can interleave with normal life; the M-phase is one continuous
push — start it only when R2 (ledger) and S2 (amendment) are done.

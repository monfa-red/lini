# BETA-tooling — schema, diagnostics, grammars & docs

The beta round, planned 2026-07-19 against the shipped alpha.5
(`SHEET-alpha5.md`). Sources: ROADMAP 3.8 (AI & tooling readiness),
PLAN-V1's beta contract (with its two carried-over PLAN-ALPHA items),
the three pre-beta audits (`AUDIT-core.md`, `AUDIT-drawing.md`,
`AUDIT-charts.md`), and the design settlements in the ledger below.
**One plan, one tag**: this round is **feature-complete** and releases
as **`v1.0.0-beta.1`** when Stage 4 closes. **Stage 0 is the SPEC pass**
— three errata that make the law match the code, plus the ledger
reconciliation the schema will read; the amended SPEC + reconciled
ledger are the contract for the stages that follow. This file holds the
settled decisions, the build order, and the execution log. The quality
bar is ROADMAP §5, verbatim. Everything is **additive** — no language
change, no existing sample or snapshot changes except where a stage
names it (a re-pinned truth, never a feature).

Scope (ROADMAP 3.8): the round **absorbs the full pre-beta audit pile**
before it publishes anything, then builds the tooling surface on the
cleaned ledger. In order — **ledger reconciliation + SPEC errata**
(Stage 0); **hardening**, consuming all three audit files (Stage 1, the
aligned-dims row-packer integration its headline); the **generated
machine-readable schema** + compact reference + CI drift check
(Stage 2); **structured diagnostics** — stable codes, a serde-free JSON
mode, machine-applicable suggestions, and the two carried-over items
(Stage 3); **editor grammars** (VS Code + Zed, keywords generated from
the ledger) + README/docs refresh + the `fmt` canon pass + round close
(Stage 4).

Stages are sized for one session at ~60 % of a context window;
sub-agents per AGENTS.md (always with an explicit `model`). At each
stage's end: fmt/test/clippy clean, a **Log** line here.

---

## Decisions ledger (settled in design review 2026-07-19 — do not relitigate)

1. **One plan, one tag; feature-complete.** The ladder's beta row stands.
   The round releases as **`v1.0.0-beta.1`** when Stage 4 closes — no
   feature is deferred out of it, and nothing beyond the round's scope is
   pulled in. Pushing and tagging stay with Abbas.
2. **Beta absorbs the full audit pile.** The three pre-beta audits are
   not a backlog to sample from — the round clears them. **Stage 0**
   consumes the ledger findings (`AUDIT-core.md` Finding 3); **Stage 1**
   consumes everything else across all three files (the aligned-dims
   packer bug and every S/M hygiene item) and **deletes the three audit
   files** once consumed, per the `AUDIT.md` precedent. Schema,
   diagnostics, and grammars are built on the cleaned ledger, never
   alongside an un-reconciled one.
3. **The three SPEC errata match the code** (Stage 0, in place):
   - **SPEC 16 `font-weight` default** row reads `normal`; the code
     default is `--lini-font-weight` = **500** (SPEC 10.1). The row's
     default becomes **medium / 500**.
   - **SPEC 12 off-grid `cell` / `span`** reads "silently ignored"; the
     validator **errors** where the container is a statically-known
     non-grid (SPEC 16's strict rule, SPEC 20's row). The prose becomes
     the strict-rule error wording, reconciled with SPEC 16/20 (the
     `span`-on-a-chart-`|band|` exception kept).
   - **SPEC 14.6 chart chrome** reads "bold"; the code sets title and
     legend **semibold** (`Font::semibold`, emits `600`). "bold" →
     "semibold".
4. **The `format` ledger row is a documented dual-channel row** (Stage 0).
   `format` has two genuinely different cascades: the **chart leg**
   (chart / pie / axis / series) is **engine-read** — the chart threads
   its own `format:` down as the axes' and series' fallback, exactly as
   `tooltip:` is read per-node (resolve channel: none); the **drawing
   leg** (drawing scope / dimension) rides the **scope-link** channel,
   exactly as `clearance` does. The row keeps its owners and its single
   resolve channel `Inherit::ScopeLink` — the only leg that uses a
   resolve channel — but stops being blanket-accepted on every node:
   validation reads the **owners** for a scope-link property that has
   node owners, so `format:` validates on a chart / axis / drawing scope
   and **errors** on a plain flow `|box|` (no more silent inert), while
   pure scene-config (`clearance` / `routing`, no node owner) stays
   valid on any container. Schema generation (Stage 2) reads both legs
   off the row by construction: owners × the documented ScopeLink
   semantics — no per-owner-inherit split, no prose-only truth.
5. **`legend` gains a `deferred` marking.** The row is honoured only as
   the auto-legend (≥ 2 entries); its placement/suppression reader is
   deferred (SPEC 23, SPEC 14 marks it `⌛`). Building the reader is
   **not** beta's job. The ledger gains a `deferred` flag (the
   `text`/`baked`/`hard` const-builder convention) and `legend` wears it
   — which is also a first-class schema field (ROADMAP 3.8 lists
   "deferred flags"), so the generated schema states it truthfully.
6. **`text-shadow` is present in SPEC 16.** The row's stale comment
   claims it is missing; SPEC 16's Text table already lists it (Stage 0
   drops the stale note — reconciliation, no SPEC change).
7. **Diagnostic codes: phase prefix + number** (Stage 3). Each
   diagnostic carries a stable code — a **phase letter** then a 3-digit
   number: **`L`** lex · **`P`** parse · **`R`** resolve · **`V`**
   validate · **`Y`** layout · **`T`** route. Codes are **stable once
   assigned** (the ROADMAP §2 promise); messages may still improve.
8. **The schema is one lean versioned JSON file, in-repo, CI-checked**
   (Stage 2). Generated from the ledger, **committed in the repo**, and
   guarded by a drift test that regenerates it and asserts
   **byte-identical** (a stale checkout fails CI, never ships). The
   **compact reference** gets the identical treatment — generated,
   committed, byte-drift-checked.
9. **The JSON diagnostic mode is serde-free** (Stage 3, AUDIT D9). The
   structured output hand-writes its JSON (the codebase carries no
   `serde`); the human LSP-style output stays the default.
10. **The two carried-over PLAN-ALPHA items ride Stage 3.** The render
    `{:?}` Debug dedup keys become **derived-`PartialEq` structural
    keys** (needs `PartialEq` on `ResolvedValue`, cascading through
    `Expr` — the R1 follow-up, structuring work the diagnostics stage
    already touches); deeper **gate-driven validation** reads the
    ledger's `gate` column (the R2/M2 follow-up — the same rows schema
    generation walks).
11. **This round only.** rc/1.0 explode into their own docs at entry;
    nothing here reserves work for them.

---

## Stages

### Stage 0 — SPEC errata & ledger reconciliation

Make the law and the ledger honest before any generator reads them. Three
SPEC errata to match the code; the `format` / `legend` / `text-shadow`
rows reconciled (the one seam schema generation will read next). No
feature, no new syntax.

- [x] **SPEC 16 `font-weight` default** `normal` → **`medium`** (500),
  matching `--lini-font-weight` (SPEC 10.1) and the code.
- [x] **SPEC 12 off-grid `cell` / `span`** "silently ignored" → the
  strict-rule error wording (SPEC 16/20 — errors where the container is a
  known non-grid; the `span`-on-a-`|band|` exception preserved).
- [x] **SPEC 14.6 chart chrome** "bold" → **"semibold"** (title + legend;
  `Font::semibold` = 600).
- [x] **`format` dual-channel row** (decision 4): the row's doc comment
  states both legs precisely (chart engine-read, drawing scope-link);
  validation stops blanket-accepting a scope-link property that has node
  owners — `node_accepts` / `check_root_decl` read the owners, so
  `format:` validates on a chart / axis / drawing scope and errors on a
  plain box, while `clearance` / `routing` stay universal scene config.
- [x] **`legend` deferred** (decision 5): the ledger gains a `deferred`
  const-builder flag; `legend` wears it; the schema-facing accessor
  lands with a pinning test.
- [x] **`text-shadow`** (decision 6): the stale "missing from SPEC 16"
  note dropped (it is present in the Text table).
- [x] Tests pin the new truths: `format` accepted on its owners / rejected
  on a flow box; `clearance` / `routing` still universal; `legend`
  deferred; `scope_link_props()` unchanged (`format, clearance, routing`).
- [x] AUDIT consumption: `AUDIT-core.md` Finding 3 (the ledger seam)
  deleted; its other findings left for Stage 1.
- [x] Sync: ROADMAP's ladder beta row points here; PLAN-V1's beta
  contract gains its round-entered note.

Acceptance: SPEC and code agree on the three errata; the `format` row is
truthful for the schema; anchors intact (every `](#…)` resolves);
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`
clean, re-pins deliberate, snapshot churn zero.
**Log:** 2026-07-19 — **done**. **Three SPEC errata**, in place: SPEC 16's
`font-weight` default `normal` → **`medium` (500, `--font-weight`)**
(matches SPEC 10.1 and `--lini-font-weight`); SPEC 12's off-grid
`cell`/`span` "silently ignored" → the strict-rule error wording (SPEC
16/20 — an error where the container is a known non-grid, the
`span`-on-a-`|band|` exception kept — the validator already errors here,
`validate.rs:302`); SPEC 14.6 chart chrome "bold" → **"semibold"**
(`Font::semibold` = 600). **Ledger reconciliation** (`properties.rs`):
`format` is now a **documented dual-channel row** — the doc comment names
both legs (chart engine-read like `tooltip:`, drawing scope-link like
`clearance`), the row keeps its owners + its one resolve channel
`Inherit::ScopeLink`, and a new `Property::has_node_owner()` lets
validation read the **owners** for a scope-link property that has node
owners, so `format:` validates on a chart / axis / drawing scope and now
**errors** on a plain box (`'format' has no meaning on '|box|' — it reads
on '|chart|' / '|pie|' / '|axis|' / a chart series / '|drawing|' / a '(-)'
dimension`) — the audit's "misuse message never fires" defect closed —
while `clearance` / `routing` (no node owner) stay universal scene config.
`node_accepts` + `check_root_decl` share the one gate
(`inherit != No && !has_node_owner`). The ledger gains a `deferred`
const-builder flag; **`legend`** wears it (the auto-legend is built; the
placement reader is SPEC 23) — a first-class schema field (ROADMAP 3.8).
**`text-shadow`**'s stale "missing from SPEC 16" note dropped (it rides
the Universal Text table, SPEC 16 line 3176). **Tests re-pinned
deliberately:** the old `format_is_inherited_scope_config` (which pinned
the inert-on-a-box **bug** as silent) became
`format_reads_on_its_owners_not_a_plain_box` (drawing-scope silent, box
errors); two new ledger tests (`legend_is_the_only_deferred_row`,
`format_is_a_dual_channel_row`); `scope_link_props()` unchanged
(`format, clearance, routing`). **AUDIT consumption:** `AUDIT-core.md`
Finding 3 replaced by a "consumed by beta Stage 0" tombstone, its Verdict
list dropped to the two remaining pre-schema items (Findings 1, 2);
Findings 4–5 and the drawing/charts audits left for Stage 1. Anchor
sweep: 537 refs, zero broken. Tests **1082 → 1084** (+2 net; one re-pin,
zero snapshot churn); fmt / clippy (`--all-targets -D warnings`) / test
clean.

### Stage 1 — hardening: consume the audit pile

Clear all three audits before the ledger becomes a published contract.
The one **L** item — the aligned-dims row-packer integration — is the
headline; everything else is shared-helper consolidation, file splits,
and pure subtraction. Ranked, headline first. Deletes the three audit
files when done.

- [x] **Aligned dims through the row packer** (`AUDIT-drawing` #1, the
  L): generalize `Rows`/`Band` to seat along an arbitrary `Frame` (today
  hard-keyed to `Side` in `line_at`/`band_box`/`past`) and route aligned
  dims through it, so `Band`, obstacle registration, and inter-row
  clearance are computed **once** — collapsing `aligned_away` /
  `aligned_line_c`, which today re-derive the packer's band and never
  register with `Rows`. SPEC 15.6's one seating law along the frame's own
  cross axis. Snapshot-verify the drawing samples (aligned dims move to
  the packed offset).
- [x] **Shared helpers, drawing** (`AUDIT-drawing` #2/#3/#4/#7/#8): one
  `PlacedNode::geometry_box` (stroke-excluded box, 2 fns + 7 inline);
  `Bbox::center` (open-coded 13×); `Frame::text_seat` (ISO text-on-a-line,
  re-derived in `round::diametral`); `geometry::project(bbox, dir)`
  (corner-projection, 3×); `Side::name` / `Side::outward` (scattered
  `Side`→str/vector).
- [x] **Shared helpers, charts** (`AUDIT-charts` F1/F2/F3/F5/F6): one home
  for `live`/`muted` (`chart/tint.rs`, 5 copies); one generic each for
  `read_range`/`read_time_range` and `read_ticks`/`read_time_ticks`;
  `edge_from` for the none/auto/deepen edge rule (+ the double-`matches!`
  `stroke_default` cleaned); `resolve_domain` for the triplicated
  domain-from-`range` block; `format::reject_date` + a shared paint-list
  message const.
- [x] **Shared helpers / dedup, core** (`AUDIT-core` #1/#2/#5): the
  projection default reads `template_bundle("projection")` through
  `paint_props`, not restated literals; one `emit_generated_default`
  helper unifies the six present-gated default emitters (three divergent
  guards today); one `find_in_scope`-based id-path walk shared by
  `inst_at_path` and `node_at`.
- [x] **The geometry-class predicate** (`AUDIT-drawing` #6): a small
  `is_geometry(n)` base for the `!sheet_node && !is_pinned` slice the six
  callers repeat, each layering only its genuine extra (halo's line-type
  exclusion, section's marker exclusion).
- [x] **File splits over ~500 LOC** (`AUDIT-core` #4, `AUDIT-drawing` #5,
  `AUDIT-charts` F4): `tests/rendering.rs` (1589, by theme — highest
  value); `src/ledger/properties.rs` tests → sibling; `resolve/links.rs`
  projection slice → `resolve/links/projection.rs`; `dims.rs` (`Frame` →
  geometry; the aligned seat folds into #1); `leaders.rs` →
  `skeleton.rs` + dispatchers; `model/axes.rs` (653) → `axes/read.rs`
  (lands the F2/F5 dedup naturally).
- [x] **Dead / vestigial** (`AUDIT-drawing` #9, `AUDIT-charts` F7): the
  stale "true aligned dims are deferred" comment (`annotate/mod.rs:25`);
  `round::spill_dir`'s dead `a` param; `geometry::geometry_bbox` /
  `plane::seg_bbox` folded onto `Bbox::from_points`; `fmt_tick` deleted
  (call `format::auto` in `pie.rs`). `AUDIT-charts` F8 (`auto` `-0`) and
  F9/F10 (notes only) recorded as no-change decisions.
- [x] Delete `AUDIT-core.md`, `AUDIT-drawing.md`, `AUDIT-charts.md` — the
  pile is consumed.

Acceptance: the aligned-dim packer never overlaps painted annotations
(the alpha.3 oracle passes on the re-routed path); every named parallel
implementation and missing-helper folded to one home; no file over the
~500 ceiling in the touched trees; fmt/clippy/test clean; snapshot churn
only where the packer legitimately moves an aligned dim (re-blessed with
a PNG check, light + dark).
**Log:** 2026-07-19 — **done**; the audit pile is consumed and the three
files deleted. **The headline** (`AUDIT-drawing` #1): the row packer is
generalized to a **`SeatLine`** — a `Frame` (u along the row, n across)
plus an outward sign and a **base** cross coordinate — with **one seating
loop** for every dim. A side row's seat line is the axis frame outward off
the extent's edge (byte-identical to the old `line_at`/`band_box`/`past`
arithmetic — zero churn on every axis-row sample); an **aligned** dim's is
its own span frame outward on the away side, based on the span's outermost
anchor. The band is computed in frame terms (`neg` = fs + 2 toward the ISO
"above", `pos` = overshoot/arrow by outward sign), the probe is an
oriented band rectangle tested against painted boxes by **separating
axes** (exactly `Bbox::overlaps` when the frame is an axis), and the
seated band registers as its world AABB — so aligned dims now clear
callouts/symbols/earlier rows and later rows clear them.
`aligned_line_c` / `Seat::Line` (the re-derived band) are deleted;
`SeatLine::away`, `stack_side`, `corner_pull` live beside the packer. One
deliberate re-bless: `drawing_annotations` (the hypotenuse dim stands
~1.3 farther out, honestly clearing a registered obstacle) — PNG-verified
light + dark, otherwise pixel-identical; a new test pins that two
identical aligned dims pack distinct rows (they used to overprint). The
oracle passes on the re-routed path. **Drawing sweep** (#2–#9):
`Bbox::center()` (13 folds); `half_stroke`/`geometry_box` (2 fns + 7
inline); `geometry::proj`/`project` (chrome, section plane, and the
packer share it — `breaks/clip.rs` left: its fold is a scalar min/max
over a cubic's hull, not a corner projection); `Frame::text_seat` (dims'
`value_texts` + `round::diametral`); `is_geometry(n)` (the
`!sheet_node && !is_pinned` base under engine/halo/annotate —
`section::is_relaid_geometry` keeps its own slice: it reads a
`ResolvedInst`, pre-placement); `Side::name`/`outward` on the enum;
dead code out (stale aligned-dims comment, `spill_dir`'s param,
`geometry_bbox`/`seg_bbox` onto `Bbox::from_points`). Splits: `Frame` →
`geometry.rs`, seat policy → `annotate/rows.rs`, `leaders` →
`leaders/{mod,skeleton}.rs` — dims 453, leaders 442+133, all under the
ceiling. **Charts sweep** (F1–F7, worktree agent): `chart/tint.rs` the
one `live`/`muted` home (5 copies deleted); generic `read_range`/
`read_ticks` taking the per-value reader (messages byte-identical);
`edge_from` owns the none/auto/explicit/unset edge table (the
double-`matches!` gone); `resolve_domain` (3 blocks); shared
`format::reject_date` + the paint-list message const; `fmt_tick` deleted
(−1 test); `model/axes.rs` → `axes/read.rs`. **Skips recorded as
no-change decisions:** F8 (`auto`'s `-0` byte-pin kept deliberately),
F9/F10 (notes only). **Core sweep** (Findings 1/2/4/5, worktree agent):
the projection default resolves `template_bundle("projection")` through
the shared bundle path (literals gone); one `emit_generated_default`
guard (present ∧ ¬authored — emission order no longer load-bearing);
`tests/rendering.rs` → `tests/rendering/{main,text,shapes,paint,links,
charts,assets}.rs`; `properties.rs` tests → `properties/tests.rs`;
`resolve/links.rs` projection slice → `links/projection.rs`; one
`walk_scope` id-path walk under `inst_at_path`/`node_at`. Slices merged
clean (charts `1a7e28b`, core `77e7ff9`, drawing `5f28aab` + `5c4f495`,
merge `8f29c17`, close `d806799`).
Gate: fmt `--check` / clippy `--all-targets -D warnings` / test clean;
**1084 tests** (+1 aligned-packer pin, −1 `fmt_tick`), zero `.snap.new`.

### Stage 2 — the generated schema + compact reference + CI drift

The ledger becomes a published contract. One lean versioned JSON schema
generated from the ledger, committed in-repo, guarded byte-identical; the
compact reference the same.

- [x] **The schema generator** (`xtask` or a `--emit-schema` path): walks
  `PROPERTIES` + the type/template/role tables and emits one JSON file —
  types, templates, roles, inheritance channels (both `format` legs per
  decision 4), properties, value shapes (`Shape`/`Kind`, list-vs-tuple
  arity), defaults (resolved from the bundles — the single tuning home),
  owners, layout/routing compatibility, required/exclusive sets,
  **deferred flags** (decision 5), and one example each.
- [x] **One versioned file, committed**: `schema/lini.json` (or the
  chosen path), carrying a schema version; regenerated deterministically.
- [x] **CI drift check**: a test regenerates the schema and asserts it is
  **byte-identical** to the committed file — a stale checkout fails.
- [x] **The compact reference**: generated from the same ledger, committed
  in-repo, the same byte-drift test — the tools/AI lookup ROADMAP 3.8
  names.
- [x] **No drift by construction**: the generator reads the same ledger
  validation reads; a new property appears in the schema the moment it
  has a row, or the drift test fails.

Acceptance: schema + reference regenerate byte-identically from the ledger
(drift = test failure); every property is described with its real
owners/shape/default/inherit/deferred; the `format` dual cascade reads
truthfully; fmt/clippy/test clean.
**Log:** 2026-07-19 — **done**. The ledger is a published contract:
`cargo xtask gen-schema` (xtask now depends on `lini`, following the
`extract-fonts` precedent) writes two committed artifacts under `schema/`,
both **generated from the ledger and nothing else** — no prose parsed, no
timestamp, so a regeneration is byte-identical. **`schema/lini.schema.json`**
(one lean file, `schemaVersion: 1` + the crate version off `CARGO_PKG_VERSION`,
stable field order) carries: an `enums` self-description (the eight `Kind`s,
four `Shape`s, four `Inherit` channels, two `Gate`s, six owner kinds); the
`layouts` / `roles` named across the owner column; `builderCalls`; the 13
**primitives** and 56 **templates**, each with its resolved default bundle
(primitive kind + base→derived chain) printed through the one canonical
value renderer (`fmt::print_decl_value`, factored out of `emit_decl` — no
parallel printer); the `sceneDefaults` / `linkDefaults` bundles; and all
**103 properties** — owners (structured), shape + scalar kind (list-vs-tuple
arity off `Shape`), `default` ref, `inherit`, `gate`, `text`/`baked`/
`deferred`, a `dualChannel` flag derived from the data
(`inherit == ScopeLink && has_node_owner()` — true for **`format`** alone,
both legs stated by owners × ScopeLink per decision 4), and one **compiled
example each**. **`schema/reference.md`** is the compact human mirror — dense
Markdown tables off the same walk. Serde-free throughout: a 60-line JSON
value + deterministic pretty-printer (`schema/json.rs`, ordered objects, never
a `HashMap`). **Examples ride the ledger** (`src/ledger/examples.rs`, keyed by
name, order-tracks `PROPERTIES`) and a test **compiles every one** through
`compile_str` (drawing/chart/sequence snippets in their owning container, the
two `|image|` ones resolving `assets/logo.svg` against `samples/` like the
conformance suite) — an example cannot rot into invalid syntax. **Drift check**
(`tests/schema.rs`): regenerate both artifacts in memory, assert byte-equality
with the committed files — a stale checkout fails CI. Coverage + orphan +
`only_format_is_dual_channel` pinned as unit tests beside the generator. Gate:
fmt `--check` / clippy `--all-targets -D warnings` / test clean; **1084 → 1089
tests** (+5: 2 unit, 3 integration), zero snapshot churn. Landed `a46c151`.

### Stage 3 — structured diagnostics + the two carried-over items

Every diagnostic gains a stable code and a machine-readable form; the
render dedup keys and gate-driven validation land alongside the
structuring work.

- [x] **Stable codes** (decision 7): assign `L`/`P`/`R`/`V`/`Y`/`T` +
  3-digit codes across the diagnostic sites; a central registry pins each
  code to its site (a test asserts uniqueness and that no code moves).
- [x] **The structured record**: code, severity, span, related span,
  suggestions, and safe **machine-applicable** replacements (an edit a
  tool can apply verbatim); the human LSP-style output stays the default.
- [x] **JSON output mode, serde-free** (decision 9, AUDIT D9): a
  `--diagnostics json` (or the chosen flag) hand-writes the structured
  record; snapshot the JSON for a diagnostic per family.
- [x] **Derived-`PartialEq` dedup keys** (decision 10): the render
  `{:?}` Debug dedup keys become structural — derive `PartialEq` on
  `ResolvedValue`, cascading through `Expr`; the dedup reads the typed
  key, not a formatted string.
- [x] **Gate-driven validation** (decision 10): the validator reads the
  ledger's `gate` column for the hard-gated properties, replacing any
  hand-kept list — one mechanism, the same rows schema generation walks.
- [x] Tests: code uniqueness + stability; JSON snapshots per family; the
  dedup key equality; the gate-driven hard-error set matches the ledger's
  `Gate::Hard` rows.

Acceptance: every emitted diagnostic carries a stable code; the JSON mode
round-trips a machine-applicable fix; the dedup keys are structural (no
`{:?}`); hard gates are ledger-driven; fmt/clippy/test clean.
**Log:** 2026-07-19 — **done**. **Stable codes** (decision 7): a `Code` catalog
(`src/error/codes.rs`) — a `Phase` letter (`L`/`P`/`R`/`V`/`Y`/`T`, plus an `E`
sentinel) + a 3-digit number, declared once in a `catalog!` macro table so the
numbers live in **one home** and construction sites name a **const**, never a
literal (no hand-numbering drift). `Error`/`Diagnostic` gained `code` (+
`suggestion`, + `related` on `Diagnostic`); the phase letter is stamped **for
free at the phase boundary** (`in_phase` / `stamp_phase` in the lib pipelines) —
a fresh diagnostic carries `Code::UNSPECIFIED` and the boundary fills the
phase's generic `x000`, so **nothing is ever codeless** and a new error family
opts into a stable number by adding one row. 45 codes catalogued and **every one
wired** to its site across all six phases (validate + the SPEC-20 property
families, resolve identity/link/asset/projection, layout missing-required /
chart-data / project-axis / drawing-measure, the lex/parse structural set, the
routing law checker); phase = **where detected** (so `reserved-id` / `unknown
-side` / `legacy-list`, detected in desugar/resolve, are R codes, not P/T).
**The structured record**: code + severity + span + related span + a
**machine-applicable** replacement (span + verbatim text) where honestly
derivable — the did-you-mean name over the misspelled token's span
(`unknown-property` → replace `colr` with `color`); the human LSP-style output
is **unchanged byte-for-byte** (codes ride the structured form only, zero
snapshot churn). **`--json`** (SPEC 19 row added) emits one serde-free document
`{ file, diagnostics: [...] }` via the **shared** `crate::json` printer (promoted
out of `schema/`, one mechanism, decision 9) — the whole pass assembly lives in
`lini::diagnostics_json`; exit 1 on any error. **Carried-over pair** (decision
10): the render `{:?}` dedup keys are now **structural** — `PartialEq` derived on
`ResolvedValue` cascading through `Expr`/`Node`, and `GradientDef`/`HatchDef` are
their own `IdTable` keys (no formatted string) — output byte-identical (every
rendering/conformance snapshot unchanged). **Gate-driven validation**: the
validator reads the ledger's `Gate::Hard` column (`cell`/`span` now marked
`.hard()` per SPEC 12) instead of a hand-kept name list — the statically
-judgeable gates (cell/span/place/activation) fire at validate, `tol`/`project`
at drawing layout; a pinning test fixes the hard set `{activation, cell, place,
project, span, tol}` and schema regenerated (the cell/span `hard-gate` flag — the
only schema drift). SPEC 20 gains a stable-codes paragraph. **Tests:** code
uniqueness + family uniqueness + per-phase generic + a `catalog` snapshot pinning
each code→family (renumber = CI fail); `--json` shape snapshots per family; the
machine-applicable fix applied by span → recompiles clean; a no-unclassified
sweep; the hard-gate pin. Gate: fmt `--check` / clippy `--all-targets -D
warnings` / test clean; **1089 → 1098 tests** (+9), zero snapshot churn outside
the two new `--json` snapshots + the catalog pin. Landed `1bbb407`.

### Stage 4 — editor grammars, docs refresh, `fmt` canon & round close

The author-facing surface: syntax highlighting in two editors with
ledger-generated keywords, the README/docs brought current, the
formatter's final canon pass, and the round tagged.

- [x] **VS Code grammar** (a TextMate/`tmLanguage` bundle): types,
  templates, properties, operators, builder calls — **keyword lists
  generated from the ledger** so they can't drift; highlights every
  sample correctly (spot-check).
- [x] **Zed grammar** (its tree-sitter/`.scm` form): the same
  ledger-generated keyword sets; the same spot-check.
- [x] **Keyword generation**: one generator feeds both grammars from
  `PROPERTIES` + `BUILDER_CALLS` + the type/template tables — a new
  property highlights the moment it has a row.
- [x] **README / docs refresh**: current feature set (through alpha.5 +
  beta tooling), the schema/reference/diagnostics surfaces documented,
  the samples showroom current.
- [x] **`fmt` final canon pass**: every sample formatter-idempotent under
  the 1.0 canon; error messages show canonical syntax.
- [ ] **Round close** (decision 1): the ladder row / version bump /
  `v1.0.0-beta.1` tag left to the main session — push stays with Abbas;
  a round-closing visual pass (ROADMAP §5) with per-sample verdicts.
  *(The visual pass ran — showroom intact, the only render-touching
  change is two SVG-identical sample reformats; only the bump/tag/push
  remain, held for the main session.)*

Acceptance: both editors highlight every sample correctly; the grammars'
keywords are ledger-generated (no hand-list); docs are current; `fmt` is
idempotent on every sample; the round is feature-complete and the tag is
cut.
**Log:** 2026-07-19 — **done** (grammars + docs + fmt canon; the version
bump / `v1.0.0-beta.1` tag / push held for the main session).
**Editor grammars, one ledger-fed generator** (`src/grammar/mod.rs`,
`cargo xtask gen-grammars`): the VS Code TextMate bundle
(`editors/vscode/syntaxes/lini.tmLanguage.json`) and the Zed tree-sitter
highlight query (`editors/zed/languages/lini/highlights.scm`) both draw
their keyword sets — types (writable primitives + every template),
properties, value builders, and the layout names — from the same
`PROPERTIES` / type / template / `BUILDER_CALLS` tables the resolver
reads, so a new row highlights on regeneration or the drift test fails.
Structure is authored once in Rust and emitted through the shared
`crate::json` printer (VS Code) and a small query writer (Zed); ledger
sets become word-bounded alternations (TextMate) and `#match?`
predicates (Zed). Comments, strings, numbers, `|type#id|` bars, `.class`,
`#id`, `--var`, the link ops, `key:` (**strong** scope for a ledger row,
**weak** for an unknown — a typo is visible), builders vs plain calls,
`( )` math, and forced endpoint sides all highlight; the property rules
**decline a colon glued to a side word** (`prop_head`, one mechanism) so
`plate:left` reads as a side, not a property named `plate` — the same
guard fixed a latent capture-group mis-map on the property head.
**Verified**: **103/103 properties** and every built-in type used across
the 33 samples are covered by the generated alternations (fall-throughs
are genuine user types/ids / endpoint sides), plus a hand-trace of
`hero` / `charts` / `drawing_gdt` constructs against the ordered
patterns. The Zed extension ships in its real tree-sitter shape
(`extension.toml`, `config.toml`, generated `highlights.scm`,
`tree-sitter-lini/grammar.js` + `package.json`); the parser build +
in-editor smoke test is the release-packaging step (no tree-sitter CLI /
Zed in-repo) — the **ledger surface, the part that can drift, is
generated and byte-guarded** here. **Drift test** (`tests/grammar.rs`):
regenerate both grammars in memory, assert byte-equality — the schema's
guarantee. **README refresh**: a new *For tools and editors* section
(schema / reference / `--json` / grammars); charts gain the time-axis +
`format:` line; drawings gain the GD&T story (`|control|` /
`|feature-control|` / `tol:` / `datums:`, projection, ISO 5457 sheet);
the CLI table gains `--json`; error prose names the stable codes (`V001`
/ `R008`); **Status** rewritten alpha → **1.0.0-beta, feature-complete**,
with the stability contract. **fmt canon pass**: `lini fmt --check` is
now clean on **all 33 samples** — `chart_advanced` and `pcb` carried
author line-wrapping the formatter collapses; re-formatted in place, SVG
byte-identical (conformance + the fmt semantic test unchanged,
PNG-verified). **Round close**: full suite + schema/reference/grammar
drift + every sample compiles & fmt-idempotent, all green; only the
ladder row / bump / tag / push remain, held for the main session. Gate:
fmt `--check` / clippy `--all-targets -D warnings` / test clean;
**1098 → 1102 tests** (+4: 2 grammar unit, 2 grammar drift), zero
snapshot churn. Landed grammars `99d35eb`, docs + fmt canon `12ffefd`.

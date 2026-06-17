# PLAN — Implementing Lini v4

How to take the codebase from the **v3 language** it implements today to the
**v4 language** specified in [`SPEC.md`](SPEC.md). Written to be executed across
several sessions (with `/compact` between), so each phase stands on its own.

`SPEC.md` is the contract. [`WIRING.md`](WIRING.md) owns wire *geometry* and is
**unchanged** by v4. Read both, plus [`AGENT.md`](AGENT.md), at the start of any
session.

---

## Ground rules

- **Clean break.** Pre-release, zero users. No aliases, no back-compat, no v3
  fallbacks. Delete v3 concepts outright.
- **Rewrite, don't patch.** This is a language change, not a tweak. If a function
  is shaped for v3, rewrite it. Move code between files, split or merge modules,
  rename freely. Do **not** layer v4 on top of v3 plumbing — that's the
  patchwork we're avoiding. Aim for code that reads as if v4 were the only
  language it ever had.
- **`SPEC.md` wins.** If the spec and your instinct disagree, follow the spec —
  or, if the spec is genuinely wrong, fix the spec in the same change with a note
  in the PR/commit. Never let code and spec drift.
- **Modern, clean Rust.** No `unsafe`. One concept per file; split past ~500 LOC.
  Don't fight `rustfmt`/`clippy`. Comments only for the non-obvious *why*.
- **Test as you go** (per AGENT.md): `insta` snapshots for output-shaped code,
  one sample per feature in `samples/`, and **verify SVG visually** — render to
  PNG with `resvg` and actually look at it; don't make the user spot-check.
- **This plan is a shape, not a straitjacket.** Reorder, merge, or split phases
  as the code demands. We don't know every constraint yet — when the plan and
  reality disagree, trust the code and note the deviation.

---

## Current state

- The repo implements **v3** end-to-end; `cargo test` is **green**.
- Pipeline: `lexer → parser → resolve → layout (+ route) → render`, plus `fmt`,
  `lint`, `theme`, `serve`, `main` (CLI). Wire routing lives under
  `src/layout/wires/` (~6k LOC) and is the part v4 leaves alone.
- `SPEC.md` already describes v4; this plan closes the gap.

### What stays vs. changes

**Stays** (preserve; don't rewrite for its own sake):
- **Wire routing geometry** — all of `src/layout/wires/` except *attribute-name
  reads* (listed in Phase 5). The router is syntax-independent; confirmed.
- **Scoping**: per-instance id scoping, sealed bodies, dot-path endpoints,
  auto-create of undeclared root-wire ids. v4 keeps these verbatim (SPEC §9).
- **Grid auto-flow** (`next_open` in `grid.rs`), the **shadow** filter, and the
  overall pipeline shape.

**Changes** (the work): the entire syntactic front end (lexer colon rules,
parser, AST, value model), the defs-block → stylesheet model, the
cascade + selectors, the sizing model, `align/justify/stretch/divider`, `fmt`,
and the property-name surface.

---

## Phase 0 — Centralized defaults

**Goal.** One module that owns every baked default, so the whole look is tuned in
one file (an explicit ask). Today defaults are scattered across
`resolve/vars.rs` (constants + visual vars) and `resolve/shapes.rs::template_attrs`.

**Key points.**
- Collect: layout constants (`font-size 14`, `wire-font-size 12`,
  `caption-font-size 13`, `stroke-width 1`, `radius 0`, `gap 20`, `padding 16`,
  `clearance 16`, `icon-size 24`, `canvas-pad 20`), the visual `--lini-*`
  defaults (SPEC §11.1), and the per-template attribute bundles (SPEC §8).
- Visual vars stay live `var()`; everything else bakes (SPEC §11). Layout
  constants are **not** `--name` variables — they are plain consts set via
  properties/rules.
- This is foundational — later phases read from here.

**Done when.** A single source for defaults exists and the rest of the code reads
it (even if some consumers land in later phases).

---

## Phase 1 — Lexer

**Goal.** Tokenize v4. Mostly small deltas from v3.

**Key points.**
- **Colon:** drop the v3 "no whitespace around `:`" rule — `key: value` is now
  canonical (space optional). `::` is the define operator; represent it however
  is cleanest (two glued colons, or a dedicated token).
- Dash-case idents, `--name`, strings, numbers, wire-ops, `&`, braces, parens,
  brackets — unchanged. Wire-op lexing is unchanged.
- Commas remain a token (now only meaningful inside value lists / call args).

**Done when.** Unit tests tokenize the SPEC's lexical examples (§2) and the
quickstart.

---

## Phase 2 — AST + Parser

**Goal.** A single-pass recursive-descent parser for the v4 grammar (SPEC §16),
and an AST that models stylesheet / instances / wires.

**Key points.**
- **Ordering contract = single pass.** Track a phase (stylesheet → canvas →
  wires) and a type set (built-ins + defines seen). The only ambiguity is
  `ident { }`: rule if `ident` is a known type, else a node. Because defines
  precede use, the type set is complete at decision time — no prescan (SPEC §16
  prose). Enforce the ordering; out-of-order statements are errors.
- **Block-only declarations.** A node/wire's properties live in its `{ }` block,
  never inline on the line (an inline `key: value` would be a root statement).
  The line carries identity (id, type, labels, classes); the block carries
  declarations + children (SPEC §3). Bodies are ordered: declarations → child
  nodes → internal wires.
- **Value model.** A declaration's value is a comma-separated list of
  space-separated scalar groups (`points: 0 0, 10 10` = two groups; `at: 100 50`
  = one group of two; `columns: 80 140 80` = one group). Functions use parens
  (`rgb(…)`, `repeat(…)`). Reshape the `Value` representation accordingly —
  v3's `Tuple`/`List` go away.
- **Selectors & defines.** Element (`rect`), class (`.hot`), descendant
  (`table rect`, `.sidebar rect`), define (`name::base`). A node requires ≥1 of
  id / `|type|` / label / block; `|wire|` as an instance is an error.
- This is a from-scratch rewrite of `ast.rs` + `parser.rs`. Don't preserve the
  v3 defs-block AST.

**Done when.** Unit tests parse every construct in SPEC §3–§9 and §20, and
reject the error cases in §15 that are syntactic (ordering, empty node,
`|wire|`, inline decl).

---

## Phase 3 — Resolve (cascade, selectors, defines, variables)

**Goal.** Turn the AST into the resolved scene + wires, applying the v4 cascade.

**Key points.**
- **No defs block.** Root bare declarations configure the scene; `wire { }` sets
  wire defaults; rules form a whole-file stylesheet. Drop `split_defs` /
  `|scene|` / `|wire|`-singleton machinery.
- **Cascade & specificity** (SPEC §4/§12): type cascade (element rule + define
  defaults, walked base→derived) → **descendant rules** → class rules → the
  instance's own block; ties by source order. Descendant-selector matching
  against the ancestor chain is **new** — the current model only has flat
  `.name` styles and `|name|` type-defaults. Build a real (small) matcher.
- **Defines** (`name::base`): like the v3 shapes table, new syntax; cycles /
  depth-16 errors stay.
- **Variables**: `--name` is visual-only and live; layout values come from the
  Phase-0 defaults, never `--name`. `--theme` still overrides `--lini-*`.
- **caption**: ship the `caption::text` type; group label sugar emits `|caption|`
  (1st top, 2nd bottom `side: bottom`, rest plain `|text|`). Reuse the existing
  label-sugar location.
- Keep wire resolution (endpoints, scope walk, fan expansion, auto-create) —
  only its property names change (Phase 5's rename list).

**Done when.** Unit tests cover the cascade ladder, descendant selectors,
defines, caption sugar, and the §15 resolve errors (unknown type/class, cycle,
duplicate id, missing `columns`, grid-prop-off-grid, etc.).

---

## Phase 4 — Layout (sizing, flex, grid, dividers)

**Goal.** Lay out the resolved scene per SPEC §5–§7.

**Key points.**
- **Sizing model** (SPEC §6, the biggest behavioural change): `width`/`height`
  default `auto` = content + `padding` (**border-box**: padding inside an
  explicit size, never added). Empty / overlay-only shape = `2 × padding`. **Drop
  the per-shape default sizes** (rect 100×40, …) and **fold `text-pad` into
  `padding`** — one knob. `|icon|` keeps `icon-size`; geometry primitives require
  their attrs. Rewrite `primitives.rs` sizing (`closed_shape_dims`,
  `auto_sized_bbox`, `read_size` → `width`/`height`).
- **Flex `align`/`justify`/`stretch`/`evenly`** (SPEC §5): currently `flex.rs`
  only does cross-axis `start/center/end`; main-axis distribution, `stretch`,
  `evenly` are **unimplemented**. Build them — but they are **no-ops without
  slack** (slack = explicit size or fixed grid tracks). This is the main new
  layout code.
- **Grid** (SPEC §5): `columns` required, `rows` optional (implicit auto rows);
  track lists with mixed `auto` + fixed + `repeat(N)`/`repeat(N,size)`; count =
  list length (not `layout:(c,r)`). Auto-flow already exists. Cell-fill becomes
  opt-in via `align`/`justify: stretch` (today it's automatic on explicit
  tracks) — decouple them.
- **Dividers** (SPEC §5): interior separators only, painted by the container's
  `stroke*`. Reuse `grid.rs::rule_segments` for the grid case; the 1-D case
  (between flex children) is new. The outer frame is the container's **own
  border** — so a `|table|` draws its border the normal way (its group rect
  stroke) **plus** interior dividers, replacing the v3 "skip the rect, draw the
  frame via rule_segments" approach. No more `is_table` special-casing of the
  frame.
- **Caption bands** (`mount: in`/`out`): `titles.rs` is reusable largely as-is
  (rename `place`→`mount` reads).

**Done when.** Unit tests pin sizes (empty = 2×padding, text = text+padding,
explicit = exact), align/justify/stretch/evenly with slack, grid track lists +
implicit rows + repeat, and dividers (1-D and grid, span-aware).

---

## Phase 5 — Render (names, canvas, title, media)

**Goal.** Emit v4 SVG (SPEC §13). The renderer already emits CSS-shaped names, so
this mostly *aligns input names with output*.

**Key points.**
- **`PAINT_PROPS`** (`render/rules.rs`) becomes near-identity: `stroke-width`,
  `font-size`, `font-family`, `font-weight` are now the input names too;
  `stroke-style` → `stroke-dasharray`. Update the table and the node `style=`
  diff.
- **Attribute-name reads to rename across the codebase** (clean find-and-fix,
  but verify each): `thickness`→`stroke-width`, `line`→`stroke-style`,
  `rotation`→`rotate`, `double`→`stack`, `text-size`→`font-size`,
  `font`→`font-family`, `weight`→`font-weight`, `size`→`width`/`height`,
  `href`→`src` (image), icon `name`→ the label, `variant`→`icon-variant`,
  `z`→`layer`, `place`→`mount`. The **wire** code reads `thickness`,
  `clearance`, `line`, `text-size`, `place`(labels), `offset`, `stroke`/`fill`/
  `color`/`font`/`weight`, `halo`(internal, keep) — rename accordingly.
- **Wire labels**: drop `WireAt::Start/Mid/End` (numeric `at` 0..1 only, plus
  `Auto`) and drop label `place: out` (use `offset` only) — SPEC §9.
- **Canvas**: a root `fill:` paints a backing rect over the viewBox (new; v3
  `background` was unimplemented).
- **`title:`**: emit a `<title>` first-child on the node `<g>` (new; v3 spec'd it
  but never emitted). `aria-label` is deferred.
- **icon/image**: glyph name from the label; image `src`→`href` attr; size from
  `width`/`height`.

**Done when.** Snapshot tests for representative nodes/wires show the v4 output;
canvas fill and `<title>` appear when set.

---

## Phase 6 — `fmt`

**Goal.** Rewrite the canonical formatter for v4 (SPEC §14).

**Key points.**
- Emit in phase order (stylesheet → instances → wires), `key: value;`
  declarations in blocks, `name::base` defines, space-separated value lists,
  2-space indent, comments/blank-lines preserved.
- **Table-cell column alignment**: align anonymous string cells into visual
  columns so the flat table form reads as a grid (SPEC §8/§14).
- Reuse the column-alignment machinery (`NodeWidths`, `split_groups`) where it
  still fits, but expect a substantial rewrite of `fmt.rs`. `desugar` still
  expands label/wire sugar to `|text|`/`|caption|` children.

**Done when.** `fmt(fmt(x)) == fmt(x)` on every sample; `fmt --check` round-trips
the canonical examples.

---

## Phase 7 — lint, CLI, theme

**Goal.** Trim the periphery to v4.

**Key points.**
- **lint**: drop the visual-attr-inline lint entirely (v4 instance blocks make
  inline paint idiomatic). The "did you mean" property-name hint table is
  deferred (SPEC §19) — leave the lint pass minimal/empty for now.
- **CLI** (`main.rs`): flags are unchanged; fix any stale help text referencing
  the defs block. `desugar`/`fmt`/`serve` subcommands stay.
- **theme** (`theme.rs`): `--lini-*` extraction is name-agnostic — unchanged
  beyond the renamed built-ins (e.g. `--lini-font-family`).

**Done when.** `cargo test` for these modules is green; CLI help reads right.

---

## Phase 8 — Samples, tests, snapshots, README

**Goal.** Bring every fixture and doc to v4 and re-green the suite.

**Key points.**
- Rewrite all `samples/*.lini` to v4 (one feature each; add samples for the new
  features: descendant selectors, divider, caption, align/justify/stretch,
  grid track lists, canvas fill, title).
- Rewrite every `.lini` source string in `tests/*.rs` to v4.
- Regenerate `insta` snapshots (`cargo insta review`) — inspect each diff, don't
  blind-accept.
- Update `README.md` (its embedded syntax examples).
- The big front-end rewrite (Phases 1–3) will red the integration tests until
  here; keep **phase-local unit tests** green meanwhile so each phase is
  verifiable, and treat this phase as the integration re-green.

**Done when.** `cargo test` fully green; conformance snapshots reviewed.

---

## Phase 9 — Visual verification & default tuning

**Goal.** Make it *look* right, and settle the eyeball-dependent defaults.

**Key points.**
- Render the showcase samples to PNG with `resvg` and read them. Check:
  captions, tables (border + dividers, no doubling), badges, stretch-filled
  cells, padding (empty = 32×32, table cells), canvas fill.
- Tune the visual-dependent defaults in the Phase-0 module — notably table-cell
  padding (`4 8` is a starting guess; the user floated `2 4`) and group/note
  padding (now the default 16, vs v3's 10/12). Adjust to taste, in one file.

**Done when.** The samples render correctly and the defaults look right to the
user.

---

## Per-session checklist

1. Read `SPEC.md` (truth) + `WIRING.md` (routing, unchanged) + `AGENT.md`.
2. `cargo test` to see the baseline.
3. Pick the lowest unfinished phase; skim its "Key points."
4. Work in clean, self-contained commits (one purposeful change each; no
   "Co-Authored-By" lines; defer pushing to the user unless asked).
5. Add tests as you go; verify any SVG change visually.

## Definition of done

All phases complete; `cargo test` green; samples render correctly (visually
verified); `README.md` current; `SPEC.md` matches actual behavior (fix drift in
place); no v3 concepts left in the code.

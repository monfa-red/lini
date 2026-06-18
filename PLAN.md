# PLAN — the box/text model

How to take the codebase from the model it implements today to the **box/text
model** specified in [`SPEC.md`](SPEC.md). Written to be executed across several
sessions (with `/compact` between), so each phase stands on its own.

`SPEC.md` is the contract. [`WIRING.md`](WIRING.md) owns wire *geometry* and is
**unchanged** by this work. Read both, plus [`AGENT.md`](AGENT.md), at the start
of any session.

---

## Ground rules

- **Clean break.** Pre-release, zero users. No aliases, no back-compat. Delete
  retired concepts outright.
- **Rewrite, don't patch.** If a function is shaped for the old model, rewrite
  it. Move code between files, split or merge modules, rename freely. Aim for
  code that reads as if the box/text model were the only one it ever had.
- **`SPEC.md` wins.** If the spec and your instinct disagree, follow the spec —
  or, if the spec is genuinely wrong, fix the spec in the same change with a note
  in the commit. Never let code and spec drift.
- **Modern, clean Rust.** No `unsafe`. One concept per file; split past ~500 LOC.
  Don't fight `rustfmt`/`clippy`. Comments only for the non-obvious *why*.
- **Test as you go** (per AGENT.md): `insta` snapshots for output-shaped code,
  one sample per feature in `samples/`, and **verify SVG visually** — render to
  PNG with `resvg` and actually look at it.
- **This plan is a shape, not a straitjacket.** Reorder, merge, or split phases
  as the code demands; trust the code and note the deviation.

---

## What changes (SPEC §1/§3/§7/§8/§9)

The language moves to **two node kinds**:

1. **Box** — `[id] [|type|] [.class…] [block]`. The line is identity; the block
   is content + config. Default type `box` (was `rect`). No positional strings.
2. **Text** — a bare `"…"`. Content only: no id, type, children, or block.
   Inside a box's block it is that box's text; on its own it is a flow/canvas
   text node. Consecutive strings are consecutive text nodes.

Plus: **id-as-label** (id is the label unless the block has a string; `{ "" }`
empties, `{}` is a no-op); **`|plain|`** frameless box and **`|caption| :: plain`**
(no 1st/2nd footer magic); **wire = 1-D container** with bare-string labels +
`along:` (so `at:` is node-position-only); **`|table|` = grid + `divider: all` +
`gap: 0`** with bare-text cells and `fmt` column alignment; text styled/positioned
only via a containing box. Reserve `rect`, `text`, `circle`, `wire`, and `'`.

Already in place (the wire-operator change, last session) and consistent with the
new spec: `..` dotted, `-> { }` wire-defaults rule, `fmt` declaration grouping +
inline-collapse. Build on them.

---

## Current state

- The repo implements the **previous model** end-to-end; `cargo test` green
  (≈328). Pipeline: `lexer → parser (src/syntax) → resolve → layout (+ route) →
  render`, plus `fmt`, `lint`, `desugar`, `theme`, `serve`, `main`.
- Text is a `|text|` primitive; a node carries positional `labels` that resolve
  expands into `|caption|`/`|text|` children; `rect` is the default; wire labels
  use `at`/`offset`. All of that is what this plan replaces.

---

## Phase 1 — `rect` → `box`

**Goal.** Rename the default/primitive rectangle from `rect` to `box`; reserve
`rect`.

**Key points.**
- `ShapeKind::Rect` → `Box`; the `"rect"` shape literal in `resolve/ir.rs`
  (`from`/`as_str`) → `"box"`; template bases in `resolve/types.rs`
  (`group`/`badge`/`note`/`row`/`column`) → `box`; the default type in
  `scene.rs`/`desugar.rs` (`unwrap_or("rect")`) → `box`; `BUILTIN_TYPES` in the
  parser. The SVG class `lini-shape-rect` → `lini-shape-box` falls out of
  `as_str()` — only the render tests assert the literal.
- **Leave the geometry `Rect` struct alone** (`src/layout/wires/rect.rs` and its
  ~500 uses across `layout/wires/*` are bounding boxes, not the shape). Touch
  only the shape `ShapeKind::Rect` / `"rect"` string, never the struct.
- Reserve `rect` (un-instantiable, un-usable id): add it to the `matches!` lists
  in `scene.rs::is_reserved_id` and `types.rs` alongside `wire`/`circle`.
- Mechanical sweep of `samples/`, `tests/`, snapshots, `README.md`.

**Done when.** `cargo test` green; `|box|` is the default and primitive; `rect`
errors as reserved. (Orthogonal, isolated — do it first to clear the way.)

---

## Phase 2 — The box/text model (front end + resolve + fmt + desugar)

**Goal.** The heart: strings are content, no positional labels, id-as-label,
`|plain|`/`|caption|`, wire `along:`, table validation. The AST changes, so every
consumer that pattern-matches it (resolve, fmt, desugar) moves together; layout
and render stay green by keeping **text a `Text` scene node** (rendered as today,
wrapped — the leaner `<text>` is Phase 4).

**Key points.**
- **AST** (`src/syntax/ast.rs`): drop `Node.labels`. A block's children become an
  ordered list of `Box(Node) | Text(TextNode)` (order matters — text interleaves
  with boxes); top-level instances likewise. `TextNode { text, span }`. Wire
  body = declarations + text labels + `|plain|` boxes.
- **Lexer**: a `'` is an error (`single quotes are not strings; use "…"`).
- **Parser** (`src/syntax/parser.rs`): default type `box`; a `string` token is a
  text node (statement / child); **consecutive strings are consecutive text
  nodes** (a string is self-delimiting — no terminator needed between them); a
  string may **not** follow the identity tokens on a node line (no positional
  labels); within a block a string is a **child**, so it comes *after* the
  declarations (`{ width: 60; "Bowl" }`) — a string before a decl is the same
  "declarations come first" error as any other child. Drop the `|text|`
  primitive (remove `text` from `BUILTIN_TYPES`) and reserve `text` as an
  un-usable id (SPEC §18). `|wire|` still errors. Wire block accepts strings +
  `along` + `|plain|`.
- **Resolve** (`src/resolve/`): synthesize the **id-as-label** text when a box's
  block carries no string; **drop the caption 1st/2nd-label magic** entirely; add
  the `plain` template (`box { stroke: none; fill: none; padding: 0 }`),
  `caption :: plain { mount: in; font-size: 13 }`, `row`/`column :: plain`;
  rebase `group`/`badge`/`note` on `box`; wire labels are bare text placed by
  `along:` (auto-distribute when unset) — retire `at`/`offset` on wire labels;
  validate **`divider` ⇒ `gap: 0`**; `|table| :: group { layout: grid; divider:
  all; gap: 0; padding: 4 8; fill: none; stroke: --stroke }`, cells are bare text
  (drop the `table rect { stretch }` rule).
- **fmt** (`src/fmt/`): emit the new forms — no positional strings, bare-text
  children, `|box|`/`|plain|`/`|caption|`, `along`. (Keep the §6/§7 grouping +
  inline-collapse already shipped. Table **column alignment** is Phase 5.)
- **desugar** (`src/desugar.rs`): id→label text; wire-label `along` distribution;
  no `|text|` expansion.

**Done when.** The new syntax compiles end-to-end to correct SVG (text still in a
`<g>` wrapper — a documented, phase-local deviation from §13); per-module unit
tests green; the SPEC §20 examples parse and resolve.

---

## Phase 3 — Layout: tables

**Goal.** Make `|table|` look right under the new model.

**Key points.**
- A `|table|`'s `padding` is the **per-cell inset** (text-to-divider): auto
  tracks size to cell content + inset; fixed tracks centre the text with the
  inset as breathing room.
- Bare-text cells auto-flow into tracks (a `Text` scene node is a grid child);
  `cell:`/`span:` apply to **box** children only.
- `divider` interior lines sit on the (now `gap: 0`) track boundaries — confirm
  the divider geometry against flush cells.

**Done when.** Unit tests pin a 3-column table's cell sizes and divider segments;
a table renders as a clean ruled grid (verified visually).

---

## Phase 4 — Render: leaner text

**Goal.** Emit text per SPEC §13.

**Key points.**
- A text scene node renders as a bare `<text class="lini-text">…</text>` at its
  placed position — **no wrapping `<g>`**. Font/colour inherit from the enclosing
  box's `<g>`. A table of N cells becomes N `<text>` elements, not N boxes.
- Box nodes keep their `<g class="lini-node lini-shape-…">`.

**Done when.** Snapshot tests show bare `<text>`; element count for a table drops
accordingly; visuals unchanged.

---

## Phase 5 — fmt: table column alignment

**Goal.** Make the flat table form read like a table (SPEC §8/§14).

**Key points.**
- `fmt` detects a grid/`|table|`, reads its `columns` count, chunks the bare-text
  cells into rows, and pads each column to its max width — markdown-style. Other
  cell kinds (a `|plain|`) break the alignment back to one-per-line.
- Idempotent and round-trips (strings are self-delimiting, so the aligned form
  re-parses to the same cells).

**Done when.** `fmt` turns one-cell-per-line into aligned columns and back
identically; `fmt --check` clean on the table samples.

---

## Phase 6 — Samples, tests, snapshots, README

**Goal.** Bring every fixture and doc to the new model and re-green the suite.

**Key points.**
- Rewrite all `samples/*.lini`: `|box|`, id-as-label, `{ "label" }`, explicit
  `|caption|`, bare-text tables, wire `along`. Add samples for the new shapes
  (`|plain|`, the flat table).
- Rewrite every `.lini` source string + assertion in `tests/*.rs`.
- Regenerate `insta` snapshots — inspect each diff, don't blind-accept.
- Update `README.md` examples.

**Done when.** `cargo test` fully green; conformance snapshots reviewed.

---

## Phase 7 — Visual verification & default tuning

**Goal.** Make it *look* right, and settle the eyeball-dependent defaults.

**Key points.**
- Render the showcase samples to PNG with `resvg` and read them: tables (ruled,
  `gap: 0`, no doubling), captions, `|plain|` labels, badges, the §20 scene.
- Tune in the one defaults file: table cell padding (`4 8` start), caption font,
  `|plain|` paint.

**Done when.** The samples render correctly and the defaults look right.

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
verified); `README.md` current; `SPEC.md` matches actual behavior; no trace of
positional labels, the `|text|` primitive, `rect` as a type, or the caption
1st/2nd magic left in the code.

# PLAN — trailing-label sugar + empty grid cells

Two surface refinements to the box/text model (already shipped; see `git log`).
`SPEC.md` is the contract — it already describes the target. This plan turns the
code to match. Both are **parser + resolve + fmt** only: the AST, the resolved
IR, layout, and render are unchanged in shape.

Read `SPEC.md` (§3 box declaration, §5 auto-flow, §9 wire labels, §16 grammar),
`WIRING.md` (unchanged), and `AGENT.md` at the start of a session.

---

## Ground rules

- **Clean break**, pre-release. No aliases. Rewrite, don't patch.
- **Modern, clean Rust.** No `unsafe`; one concept per file; don't fight
  `rustfmt`/`clippy`; comments only for the non-obvious *why*.
- **`SPEC.md` wins.** If code and spec disagree, fix whichever is wrong in the
  same change; never let them drift.
- **Test as you go**: `insta` for output, one sample per feature, **verify SVG
  with `resvg`**. `cargo test` green at each commit.

## Current state

The box/text model is complete (`cargo test` ~332 green). Today: a box/wire label
is **only** a `Child::Text` inside the block; a block-less box uses id-as-label;
an empty `""` text is always dropped (`is_blank_anon_text`). The two changes below
are the only deltas.

---

## What changes (SPEC §3 / §5 / §9 / §16)

1. **Trailing-label sugar.** A block-less box or wire may trail its label
   string(s) to the line's end: `api |box| "API"` ≡ `api |box| { "API" }`;
   `a -> b "x" "y"` ≡ two labels. A node that opens a `{ }` block keeps its label
   inside it — **trailing + block is an error**. Pure parser sugar: it desugars
   to the block form, so nothing downstream changes.
2. **Empty grid cells.** An empty `""` text is suppressed in **flow** (as today)
   but **kept in a grid**, where it is a real empty cell that holds its track —
   so a blank table cell stops collapsing and misaligning the row.

---

## Part A — trailing-label sugar (parser + fmt)

**Parser** (`src/syntax/parser.rs`). No AST change — synthesize the block.

- `parse_node`: after the classes loop, **greedily consume trailing strings**
  (`while String { … }` — it stops at the newline token on its own). Then:
  - strings **and** a `{` block follow → error `a label is the trailing string
    or the block, not both` (SPEC §15);
  - strings, no block → `block = Some(Block { children: <Text per string>, .. })`;
  - no strings → the existing block-or-nothing path.
  Drop the current "a label is a child, not positional — put it in the block"
  error (that rule is reversed now).
- `parse_wire`: the same — after the classes, trailing strings synthesize a
  `WireBlock { labels: <Child::Text per string>, decls: [] }`; strings + block is
  the same error.
- A statement that *starts* with a string is still a standalone text node
  (`parse_child`); the greedy trailing-consume only happens after a head, so
  `"a" "b"` (no head) stays two nodes while `x |box| "a" "b"` is one box with two
  labels. (The terminator already lets a following string self-delimit — keep it.)

**fmt** (`src/fmt.rs`). The inverse: contract a text-only block to trailing form.

- Add a `terse: bool` field to `Emitter`. `format` sets it `true`; `print_file`
  (desugar) sets it `false`.
- When `terse` and a box's block is **only text children** (≥1, no decls, no box
  children, no internal wires), emit `id |type| .class "a" "b"` — head then the
  bare strings, no braces. Likewise a wire block that is only text labels (no
  `along:`/decls, no `|plain|`) → trailing strings. Everything else keeps braces.
- Idempotent + round-trips: `"x"` parses back to the same `{ "x" }` AST, which
  re-emits to `"x"`. Keep table-cell alignment (a `|table|` always has `columns:`,
  so it has a block — it never hits the trailing path).
- `desugar` needs no logic change: the parser already turned a trailing label
  into a block, and `print_file` (`terse: false`) prints that block.

**Tests.** Parser: trailing single/multi on box and wire; `x |box| "a" { … }`
errors; `"a" "b"` (no head) is two nodes; `x |box| "a" "b"` is one box, two texts.
fmt: `{ "x" }` → `"x"`, multi, wire, and a box with decls **keeps** braces;
idempotence. desugar: `api |box| "API"` → `{ "API" }`.

---

## Part B — empty grid cells (resolve)

**Resolve** (`src/resolve/scene.rs`). `is_blank_anon_text` drops an empty,
id-less `Text`. Make the drop **grid-aware**: keep empties when the *container*
is a grid (`layout: grid`, which a `|table|` resolves to), drop them in flow.

- In `resolve_node`, the `children.retain(|c| !is_blank_anon_text(c))` becomes
  conditional on the container's layout — a grid keeps empty cells, flow drops
  them. Thread the same check through `resolve_instances` for a grid root.
- A box's own empty label still drops (the box isn't a grid, so its empty `Text`
  child is removed → an unlabelled box), so `cat |box| ""` is unchanged.

**Tests.** Resolve: `|table| { columns: 2; "a" "" }` keeps two cells (the empty
one holds its slot); `g |group| { layout: row; "" }` drops the empty; `cat |box|
""` is an unlabelled box.

---

## Part C — samples, snapshots, visual

- Re-`fmt` all samples: most labels go terse (`name |box| "Ada"`, `|caption|
  "Kitchen"`) — a readability win. `samples/table.lini` already has a `"Mango" ""
  "ripe"` row; it must now render a real empty middle cell.
- Regenerate `insta` snapshots — **review each diff**. Only `table.lini` (and any
  other empty-cell case) should change geometry; the rest are source-only re-fmt
  with identical SVG.
- **Verify with resvg**: `samples/table.lini` shows the empty cell holding its
  column (3×3 grid, "ripe" in column 3, not shifted left).

**Done when.** `cargo test` green; trailing labels parse and fmt round-trips them;
an empty table cell renders as a held slot (visually verified); `SPEC.md` matches
behaviour.

---

## Per-session checklist

1. Read `SPEC.md` + `WIRING.md` + `AGENT.md`; `cargo test` for the baseline.
2. Part A (parser + fmt), then Part B (resolve), then Part C (samples/snapshots).
3. Clean, self-contained commits (one purposeful change; no "Co-Authored-By";
   defer pushing to the user).
4. Tests as you go; verify the table empty cell with `resvg`.

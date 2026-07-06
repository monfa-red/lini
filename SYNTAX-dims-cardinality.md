# Syntax update — dimension bracket, ER cardinality, the circle glyphs

Status: **agreed in design; not yet implemented.** Pre-release, so breaking
changes are free. This doc is the source of truth for the change; SPEC.md is
edited from it, then the engine.

## The problem

Arrows and measurement share glyphs. `<->` is both a two-way link *and* a
linear dimension; `(-)` (diameter) then looks like a `<->` span. Two overloads,
one root cause: **measurement borrowed the arrow vocabulary.** The fix is not a
prettier sigil — it is to give dimensions their own bracket and evict linear
from the arrow world entirely. Doing that also dissolves the `|-|` question:
its only resemblance was to `<->`-as-dimension, which no longer exists.

## Decisions

### 1. Dimensions live in the `( )` measurement bracket — pictographic

`( )` already means "measure" in the drawing scope (`(-)`, `(<)`). This
finishes that half-built pattern instead of piling a fourth job on the pipe.
Each glyph is a **picture of what it measures**:

| Op | Kind | Arity | Reads |
|---|---|---|---|
| `(-)` | **linear** span | binary / chain | the dash is a length → `25` |
| `(o)` | **round** (⌀ or R) | unary / side-anchored | the circle is a diameter → `⌀10`, `R3` on a named arc |
| `(<)` | **angular** | binary (two edges) / unary (mirrored included angle) | the wedge is an angle → `40°` |

This **reassigns** today's `(-)` (round) → **linear**, and round moves to
`(o)`. Rationale: a dash pictures a length far better than roundness; a circle
pictures ⌀ far better than a dash. "The feature picks ⌀ vs R" is kept, so no
letter-abbreviation kinds (`|d|`/`|r|`) are ever needed.

**Arity disambiguates cleanly** (no scope-mode ambiguity):
- `(-)` is **always binary** — `a (-) b`, or a chain `a (-) b (-) c`. Unary `a (-)` errors ("linear needs two ends").
- `(o)` is **always unary / side-anchored** — `hole (o)`, `bore:top (o)`. Binary `a (o) b` errors.
- `(<)` binary or unary, as today.

`<->` (and `->`, `-->`, …) in a drawing scope revert to their plain meaning: a
straight **annotation arrow** (SPEC 15). Nothing in a drawing looks like a
dimension unless it sits in `( )`.

Arc-length (a possible 5th kind) stays **deferred** — the leader fallback
covers it (`arc <- "L=42"`). Aligned/ordinate dims remain deferred as before.

### 2. `|-|` (all-links selector) — keep

Once dimensions leave the arrow family, nothing resembles `|-|`. It stays the
link *type* in bars, consistent with every other type (`|box|`, `|oval|`, …).

### 3. `||` (mate) — keep

The GD&T parallel bars; draws nothing; own bracket. No change.

### 4. ER cardinality — one compositional rule, not a pile of ops

An end-marker is `[min][max]`. Three glyphs generate all six relations:

- min (inner): `o` = zero (hollow ring) · `+` = one (bar) · *(omitted)* = unspecified
- max (outer): `+` = one (bar) · `<` = many (crow)

| Op | Relation |
|---|---|
| `-+` | one |
| `-<` | many *(already exists — `<` is crow at the end)* |
| `-o+` | zero-or-one |
| `-+<` | one-or-many |
| `-o<` | zero-or-many |
| `-++` | exactly one |

Invalid combos (e.g. a lone `-o`, min with no max) error with a hint. The
start side mirrors for the simple cases (`>-` crow-start stays); **compound
start-side cardinality is deferred** — real ER annotates the target side, or
writes the relation from the other node. This replaces the SPEC 23 "operator
spellings for ER cardinality markers" deferral (now built); the existing
`marker:` property values (`crow`, `one`, `zero-or-one`, `one-or-many`,
`zero-or-many`) are the lowering targets — the ops are sugar over them.

### 5. The circle glyphs — `o` hollow, `*` filled; no `0`

- **`o` = the hollow circle**, only where it is **delimited or sandwiched**:
  `(o)` (parens delimit it) and `-o<` / `-o+` (always followed by a max glyph).
  Both are lexically unambiguous.
- **`*` = the filled dot** — the connection marker and the `*-` leader. It is
  the one round mark that must stand **alone**, which `o` cannot do safely
  (`-o` vs `-once`).
- **No `0`** enters the language. **No bare `-o` operator** — a hollow
  *endpoint* is `marker-end: circle`, a paint choice, so `o`'s safety is
  permanent.

Glyph shape = meaning: `*` solid, `o` hollow. Min-zero renders hollow, per the
crow's-foot standard.

### 6. `(-)` also selects all dimensions — a subtype of `|-|`

Parallels `|-| { }` (all links). A dimension's type chain is **`|-|` → `(-)`**
(dimension is a link subtype), so the cascade gives both behaviours for free:

- `|-| { }` still reaches dimensions (the broad link look — stroke, color).
- `(-) { }` overrides **for dimensions only** (type cascade tier 1, the more
  specific type wins), decoupling dimension styling from link/leader styling.

`(-)` is the **sole** dimension selector — the whole family, all three ops
(`(-)`/`(o)`/`(<)`), exactly as `|-|` is the one selector for every link op
(`->`/`<->`/`-*`). It is *not* "linear only": in selector position the type is
"dimension"; linear/round/angular are op variations of that one type, not
distinct types (mirroring `->` vs `<->` under `|-|`). The stylesheet-vs-canvas
section rule disambiguates it from the linear operator, exactly as `|box| .hot`
is a rule in the stylesheet and an instance on the canvas (SPEC 4). Per-kind
dim selectors (`(o) { }`, `(<) { }`) are **deferred** — one family selector
avoids three near-identical heads.

Leaders (`<-`/`*-`/`>-`) stay under `|-|` — the "link/leader" bucket; a
leader-specific selector is deferred (YAGNI).

## Lexing / grammar deltas (SPEC 21, 2)

- `draw_op = "||" | "(-)" | "(o)" | "(<)"` — add `(o)` as a free-standing
  measuring op (same glue rule: space before `(` → op; glued → call, so
  `foo(o)` is still a call).
- Link-op marker alphabet gains `o` and `+`. `+` is standalone-valid; `o` is
  **valid only immediately before `+` or `<`** — a bare `-o`/`o-` is an error
  with a did-you-mean (`-o<` / `-o+`, or `marker: circle`). `*`, `<`, `>`, `~`,
  `-` unchanged.
- One-ended relaxation list updates: `(o)` is unary-only (the slot old `(-)`
  held); `(-)` now requires both ends; `(<)` binary or unary as before.
- `(>)` stays reserved (unchanged).
- `sel_unit` gains `(-)` — a dimension-family selector at stylesheet
  statement-head (a leading `(` is unambiguous there; calls only appear in
  value position). Selector-only, like `|-|`; dimension is a `|-|` subtype in
  the type cascade.

## SPEC.md sections to edit

- **4 Selectors & Cascade** — `(-)` as a `sel_unit`; dimension is a `|-|`
  subtype in the type cascade (tier 1).
- **9 Links** — operator table: add `+`, `o`; the `[min][max]` cardinality
  rule; note `<->` etc. are plain links everywhere.
- **7 Nodes / Markers** — the cardinality set now has operator spellings;
  reconcile with the `marker:` property values.
- **15.6 Dimensions** — linear `<->` → `(-)`; round `(-)` → `(o)`; the big
  table, the diametral-line prose, auto-measure sources (`(o)` → ⌀/R), arity.
- **15 intro table, 15.9 lowering, 15.10 properties** — measuring-op list
  `(-)`, `(o)`, `(<)`.
- **21 Grammar** — `draw_op`, `link_op`/`marker`, the relaxation clause.
- **2 Lexical** — `(o)` glue note alongside `(-)`/`(<)`.
- **22 Reserved** — `o`/`+` contextual in link-op position; `0` unused.
- **23 Deferred** — remove ER-operator-spelling deferral; add start-side
  compound cardinality + bare hollow-circle *operator* (→ `marker: circle`);
  keep arc-length / aligned / ordinate.

## Implementation stages (after your WIP lands + doc review)

1. **Lexer** — `(o)` op token; `o`/`+` markers with the sandwiched-`o` rule.
2. **Parser / AST** — `(-)`→linear binary, `(o)`→round unary; cardinality
   `LinkMarker` composition (min/max → the existing marker set); `(-)` as a
   `sel_unit` + the `|-|`→`(-)` type chain in the cascade.
3. **Dims engine** — rewire `round.rs` / `dims.rs` to the new op assignment
   (`(-)` binary linear moves into the `<->` path; `(o)` takes round).
4. **Cardinality lowering** — `-o<` etc. → `crow`/`one`/`zero-or-*` markers.
5. **Samples migration** — every drawing sample's `<->`-as-dim → `(-)`, and
   `(-)`-as-round → `(o)` (barrel, dims, bushing, tiebar, pump); refresh
   snapshots; add one ER sample exercising the cardinality set.
6. **SPEC sweep + fmt** — teach `fmt` the new ops; error rows; `cargo test` /
   `clippy` / `fmt` green; re-render drawing PNGs and spot-check values.

## Migration notes

- Diagram (non-drawing) `<->` / `->` are **untouched** — they were never
  dimensions.
- The break-the-world edits are confined to drawing samples and the drawing
  engine; the core link grammar only *gains* glyphs (`o`, `+`), it removes
  nothing.

# ENTITY_PLAN — entities, table headers/footers, ER markers

Implementation plan for the database-diagram feature specified in **[SPEC.md §8](SPEC.md)**
(templates) and **[SPEC.md §7](SPEC.md)** / **[§9](SPEC.md)** (markers). This file is
scaffolding — delete it when the feature lands. Each step lists its files, changes, tests,
and a "done when" gate, and every step leaves `cargo build && cargo test` green.

> **Re-orient (every session):** read `SPEC.md` §8 (Templates — *Tables* / *Entities*),
> §7 (*Markers*), §9 (*Operators*); read this whole plan; run `cargo test` for the baseline;
> check **Progress** below. **Branch: `entity`, cut off `sequence`** (not `main`) — rebase on
> `sequence` before each commit so it merges cleanly once sequence lands. No "Co-Authored-By"
> lines; run `cargo fmt` before any commit.
> **Stale-binary trap:** the dev binary can run stale code — `cargo clean -p lini` (or a fresh
> `cargo build`) before any render-verify.
> **Verify visually:** render to PNG with `resvg` (use `--bake-vars`, else `var()` falls back
> to black) and read it — don't ask the user to spot-check.

## Progress

- [x] **Step 1 — Table headers & footers** ✓ (footer→footnote; `|header|`/`|footer|` bundles + `--header-fill`; `|table|` first row auto-wraps to `|header|`; bare `|grid|` unaffected; samples/snapshots/README/grammar updated; 452 tests + clippy + fmt clean; verified light+dark)
- [x] **Step 2 — The `|entity|` node** ✓ (built with Step 1 — same `lower_node` branch: label→spanning `|header|` title, field rows stay text, header/footer cells span; `samples/entity.lini` + snapshot; verified light+dark)
- [x] **Step 3 — ER relationship markers** ✓ (crow's-foot redrawn — splays onto the entity edge, converging back; `one`/`zero-or-one`/`one-or-many`/`zero-or-many` + `many` alias as `marker*:` values; all open markers share `.lini-marker-open`; `MarkerKind` parse + emit tests; crow snapshots re-blessed for entity/icons/links; 455 tests + clippy + fmt clean; verified light+dark)

**All three steps landed.** Acceptance gate below met; `ENTITY_PLAN.md` can be deleted once the feature merges.

---

## What we're building

Three related pieces, all proven feasible by hand-built renders during design:

1. **Table headers/footers.** A header is a **box cell that fills its grid track** and paints
   its own `fill` — no new render path, no `header-*` paint properties. A `|table|`'s first
   row becomes `|header|` cells automatically (light-gray band + bold); a `|footer|` is the
   muted opt-in for the last row. Body cells stay bare text (the "N text, not N boxes" virtue,
   [§14](SPEC.md)). The old `|footer|` (a footnote under a shape) is renamed `|footnote|`.
2. **`|entity|`.** Sugar over `|table|` (2 auto columns) whose **label lowers to a `|header|`
   spanning all columns** (the title), over `"field" "type"` rows. Relationships are ordinary
   links; field-level wires are opt-in (give a cell an id by writing it `|block#id|`).
3. **ER markers.** The crow's-foot already maps to `-<` but **draws as an inward chevron**
   (`markers.rs` `Crow`, splay points into the shape — reads like an arrow). Redraw it as a
   true crow's foot, and add the cardinality family (`one`/`zero-or-one`/`one-or-many`/
   `zero-or-many`) as `marker*:` values (no operator spellings — [§20](SPEC.md)).

**Design evidence (rendered, today's primitives):** a stretched spanning `|block|` with `fill`
produces an exact dbdiagram header band with span-aware dividers; entity↔entity links route
cleanly; the inset grid already inflates *box* cells by the table `padding` (`layout/mod.rs`
~474), so header/footer cells need **no padding of their own** — `stretch` + `fill` suffice.

## Where the changes land

| Area | File | What |
|---|---|---|
| Templates registry | `src/desugar/types.rs` (`TEMPLATES`) | add `header`/`footer`/`entity`; rename `footer`→`footnote` |
| Bundles | `src/desugar/bundles.rs` (`template_bundle`) | new bundles; `footer`→`footnote` body |
| Desugar | `src/desugar/mod.rs` (`lower_node`) | auto-header (table); entity label→span header; entity header/footer span |
| Role var | wherever `--lini-*` defaults live (theme/render) | add `--lini-header-fill` |
| Markers | `src/resolve/ir.rs` (`MarkerKind`), `src/render/markers.rs` | new kinds + `parse`; redraw crow; render new glyphs |
| Samples | `samples/templates.lini`, new `samples/entity.lini` | footer→footnote; entity sample |
| Snapshots | `tests/…` insta snaps | basket gains a header; crow change ripples; new entity snap |
| Docs | `README.md` | a short Entities mention + feature bullet |

---

## Step 1 — Table headers & footers

**Goal.** `|header|` and `|footer|` cell types exist; a `|table|`'s first row auto-becomes
`|header|`; the old footnote is renamed. No entity yet.

**Changes.**
- `types.rs`: in `TEMPLATES`, rename `("footer","caption")` → `("footnote","caption")`; add
  `("header","block")` and `("footer","block")`.
- `bundles.rs`: rename the `"footer"` arm → `"footnote"` (body unchanged); add
  `"header" => justify: stretch; align: stretch; fill: --header-fill; font-weight: bold` and
  `"footer" => justify: stretch; align: stretch; color: --footer-color`.
- Add `--lini-header-fill` (`light-dark(rgba(0,0,0,.06), rgba(255,255,255,.08))`) beside
  `--lini-group-fill` in the visual-var defaults; it tree-shakes like the rest.
- `desugar/mod.rs` `lower_node`: when the node's type chain ends in `table` **but not**
  `entity`, wrap the first row's `columns`-count bare-text cells as `|header|` blocks (text
  moves into the block's `[ ]`; a cell already a box is left as-is). Count columns from the
  resolved `columns:` track list (mirror `grid::parse_tracks`; `repeat(N)`→N).
- `samples/templates.lini`: `|footer| "since 1843"` → `|footnote| "since 1843"`.

**Tests / verify.**
- Unit: `template_bundle("header")` has `fill`+`font-weight: bold`; `footnote` keeps the
  pinned-caption body; first-row wrap produces N `|header|` children.
- Render (baked): the `basket` table — first row a gray bold band, body plain, dividers intact.
  Render `|table| |header| { fill: none; font-weight: normal }` — header reverts to plain.
- Update affected snapshots; `cargo test && cargo clippy && cargo fmt --check` clean.

**Done when:** tables show an automatic header, `|footer|` works on a last row by hand,
`|footnote|` replaces the old footer everywhere, all gates green, visually verified.

---

## Step 2 — The `|entity|` node

**Goal.** `|entity#x| "Title" [ "field" "type" … ]` renders the dbdiagram card.

**Changes.**
- `types.rs`: add `("entity","table")`.
- `bundles.rs`: `"entity" => columns: auto auto` (inherits table's grid/divider/gap/stroke).
- `desugar/mod.rs` `lower_node`: when the chain ends in `entity` —
  - lower the **label** to a `|header|` child at `cell: 1 1; span: <columns>` (the title bar),
    **instead of** the table first-row auto-wrap (entity rows are all fields);
  - give any `|header|`/`|footer|` cell in the entity `span: <columns>` when it has no explicit
    span (so a hand-written footer spans too — SPEC §8 *Entities*).
- `samples/entity.lini`: two entities + a `-<` relationship (the SPEC §21 example).
- `README.md`: a one-paragraph Entities mention + a feature-list bullet.

**Tests / verify.**
- Unit: an entity desugars to a `group`/grid whose first child is a spanning `|header|` bearing
  the title; field rows remain `Text`.
- Render (baked): single entity (title band + 2 aligned columns + dividers); two entities with
  `users -< orders` (route clean, no stray warning).
- New conformance snapshot for `entity.lini`; all gates green.

**Done when:** the §21 entity example renders as designed, light + dark, gates green.

---

## Step 3 — ER relationship markers

**Goal.** `-<` reads as a real crow's foot; `marker-end: one|zero-or-one|one-or-many|zero-or-many`
draw the standard ER glyphs.

**Changes.**
- `resolve/ir.rs`: add `MarkerKind::{One, ZeroOrOne, OneOrMany, ZeroOrMany}`; extend `parse`
  with `"one"`, `"zero-or-one"`, `"one-or-many"`, `"zero-or-many"` (and accept `"many"` as an
  alias of `Crow`). `from_marker` (operator glyphs) is unchanged — no new operator spellings.
- `render/markers.rs`:
  - **Redraw `Crow`** so the splay lands on the shape edge (three prongs fan onto the entity,
    converging back along the line) and reads at the default `link-width` — fixing the
    chevron-that-looks-like-an-arrow (current `Crow` arm).
  - Add a **bar** primitive (a short perpendicular tick) and a small **ring**; compose:
    `one` = bar; `one-or-many` = bar + crow; `zero-or-one` = ring + bar; `zero-or-many` =
    ring + crow. Reuse `marker_size` / paint; the crow paints via `.lini-marker-crow`
    (`stroke: inherit`), the ring/bar likewise stroked.
- `line_inset` / `shorten_for_markers`: ensure the line tucks under the new heads.

**Tests / verify.**
- Unit: `MarkerKind::parse` round-trips the new names; `one`≠`crow`.
- Render (baked): a strip of all six (`arrow`, `crow`, `one`, `zero-or-one`, `one-or-many`,
  `zero-or-many`) and the two-entity ER example — each reads unmistakably.
- Re-bless snapshots the crow redraw ripples (sequence/link samples that use `-<`/`crow`);
  confirm the diffs are only the intended geometry. All gates green.

**Done when:** the crow's foot reads correctly everywhere it appears and the cardinality
family renders, gates green, visually verified.

---

## Acceptance (whole feature)

`cargo build && cargo test && cargo clippy && cargo fmt --check` clean; `samples/entity.lini`
and the updated `templates.lini` render correctly in light **and** dark; SPEC §7/§8/§9/§21 match
the behaviour; no `header-*` paint properties and no new grammar were added. Rebase on
`sequence` and confirm still-green before handing back to the user for the merge.

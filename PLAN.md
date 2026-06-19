# PLAN — positioning model refactor

Implements the SPEC §6 redesign (the single source of truth). Two knobs replace
five: **`pin`** (out-of-flow named anchor) and **`translate`** (universal nudge).
Gone: `margin`, `mount`, `side` (as a positioning prop), `at`, `offset`.

Pipeline: parse → resolve → layout → render. Parsing needs no change (props are
generic `key: value`); the work is in resolve (templates), layout (placement),
render (paint order), plus samples, tests, and docs.

---

## 1. The model, restated for the implementer

- **`pin`** takes a value naming a parent anchor: `none` (default, in flow),
  `center`, the four edge midpoints (`top`/`bottom`/`left`/`right`), the four
  corners (`top left`/`top right`/`bottom left`/`bottom right`). The child's bbox
  **center** lands on that point. A pinned child is an overlay: it does **not**
  grow the parent and paints **above** the in-flow children (`layer:` overrides).
- **`translate: x y`** shifts any node (flow or pinned) by (x, y) after placement
  — no reflow, no growth. It bakes into the node's origin (`cx`/`cy`), so the
  canvas and wires follow it but siblings/parent do not. `pin: center` +
  `translate: x y` == the old `at: x y` (parent origin is its center).
- Value shapes (from `resolve/value.rs`): `pin: center` → `Ident("center")`;
  `pin: top right` → `Tuple([Ident("top"), Ident("right")])`; `translate: x y` →
  `Tuple([Number, Number])`.

---

## 2. Resolve layer

**`src/resolve/types.rs` — `template_attrs`** (the actual design change):
- `caption` → `vec![attr("font-size", num(13.0))]` (drop `mount: in`).
- `badge` → `pin: top right` + visual props, dropping `mount`/`side`/`align` and
  the explicit `layer: 10`:
  `attr("pin", Tuple[ident("top"), ident("right")])`, `radius: 999`,
  `padding: 2 8`, `shadow: 2`, `fill: --accent`, `color: --on-accent`,
  `font-size: 11`. (Add an ident-pair helper alongside the existing `pair`.)
- Keep `caption` as a template — it earns its place as the title/footer preset
  and keeps `caption { … }` global styling working. Do **not** fold into `plain`.

**`src/resolve/ir.rs`:**
- Fix the stale comment on `ShapeKind` (lines ~59–60): a caption is a small-text
  `|plain|` flow child, not a `place`-reserving title primitive.
- Clarity rename: `WireAt` → `Along`, field `ResolvedText.at` → `.along`
  (the user-facing `at` is gone; the field is driven by `along:`). Propagate to
  `resolve/wires.rs` and `layout/wires/labels.rs`.

**`src/resolve/wires.rs`:** only the `WireAt`→`Along` rename; `along:` handling is
already correct.

**`src/resolve/program.rs`:**
- Rewrite test `explicit_caption_is_a_box_mounted_in` → assert the caption carries
  **no** `mount` and has `font-size: 13` (rename to `caption_is_small_text_plain`).
- Reword the comment (~line 182) that calls the root's canvas-pad its "margin".

---

## 3. Layout layer

**`src/layout/anchors.rs` — gut and rebuild around `Pin`:**
- Remove `Place`, `Pos`, `Side`, `Align`, `read_pos`, `is_out_band`,
  `parse_offset`. (`flex.rs` parses `align`/`justify` itself, so `anchors::Align`
  has no other user; the AST `crate::ast::Side` for wire endpoints is unrelated
  and stays.)
- Add:
  ```rust
  /// A parent anchor a pinned child centers on, as fractions of the parent bbox
  /// from its center: center (0,0), top (0,-0.5), top right (0.5,-0.5), …
  pub struct Pin { pub fx: f64, pub fy: f64 }
  pub fn read_pin(attrs, span) -> Result<Option<Pin>, Error>   // None | none → None
  pub fn is_pinned(attrs) -> bool                              // pin present and != none
  ```
  `read_pin` maps `Ident("none")`→`Ok(None)`, `center`/edges→`Some`, the four
  corner `Tuple`s→`Some`, anything else→the SPEC §15 error
  (`'pin' expects none, center, an edge (top/bottom/left/right), or a corner …`).
- `Role` becomes `{ Flow, Pinned }`; `child_role` returns `Pinned` iff `read_pin`
  is `Some`.
- A `resolve(pin, parent_bbox, child_bbox) -> (cx, cy)` that puts the child's
  center on `parent.center + (fx·parent.w, fy·parent.h)`, minus the child bbox's
  own center offset (same recentre `flex::place` uses).

**`src/layout/titles.rs` — delete the file** (no bands). Drop `mod titles;` from
`layout/mod.rs` and every `titles::` call.

**`src/layout/mod.rs` — `lay_out_container_children`:**
- Remove the margin inflate (top) and deflate (bottom) loops.
- Remove `reserve_indices`/`in_indices` and the `titles::reserve_bands` /
  `place_out_bands` calls; roles are now Flow + Pinned only. `body_bbox` is just
  `flow_bbox`.
- Place pinned children against `anchor_parent_bbox` (explicit size or
  `body_bbox`) via the new `anchors::resolve` — no `offset`.
- Add a final pass over **all** children: `c.cx/cy += translate(c.attrs)` (after
  `body_bbox` is fixed, so no reflow / growth). Add a `translate()` reader
  (pair, absent → none) in `primitives.rs` or `anchors.rs`.

**`src/layout/mod.rs` — `layout_inst` / `attempt`:** remove the `place_out_bands`
calls and the `frame` plumbing; a node's drawn box is always its `bbox`.

**`src/layout/ir.rs`:**
- Remove `PlacedNode.frame` and `draw_box()`; update the two callers
  (`render/primitives.rs::dim_excluding_stroke` and the divider test) to `n.bbox`.
- Keep `Bbox::expand` (still used by the table cell inset) and `Bbox::shifted`
  (used by `accumulate_extent`); reword `expand`'s margin-referencing doc.

**`src/layout/primitives.rs`:** delete `margin()`. Keep `padding`/`gap`
(`gap` keeps its current negative allowance — see Flags).

**`src/layout/flex.rs`:** rename the internal `pinned()` /`pinned` checks (they
mean "has an explicit width/height") to `dim_set()` to avoid colliding with the
new `pin` concept. No behavioural change (flex only ever sees flow children).

**`src/layout/wires/labels.rs`:** rename `offset_of` → `translate_of`, read the
`translate` attr (tangent frame kept), update the `WireAt`→`Along` field use and
the header comment.

`accumulate_extent` needs no change — translate is already folded into `cx`/`cy`,
so the viewBox includes it for free.

---

## 4. Render layer

**`src/render/mod.rs` — `in_layer_order`:** sort key becomes the *effective*
layer — explicit `layer:` if set, else `1.0` for a pinned child and `0.0` for a
flow child — stable, ties by source order. Reuse `layout::is_pinned` (re-export
from `layout/mod.rs`).

**`src/render/primitives.rs`:** `dim_excluding_stroke` uses `n.bbox` (frame gone).
No `translate`/`pin` work here — both are baked into `cx`/`cy` already.

---

## 5. Dead-code checklist (grep must come back empty in `src/`)

`mount`, `\bmargin\b` (positioning), `is_out_band`, `place_out`, `reserve_bands`,
`Place::`, `Pos::`, `parse_offset`, `read_pos`, `frame`/`draw_box`, `titles`,
`"offset"`, `"at"` (as a positioning attr). Each removal also strips its doc
comments and tests.

---

## 6. Samples (`samples/`)

- **delete** `margin.lini`, `place_out.lini` (features removed) and their
  `tests/snapshots/*` counterparts.
- **rewrite** `anchors.lini` → the nine `pin` anchors (center / 4 edges / 4
  corners), wrapping each label in a `|plain|`. Keep the name (it is the anchor
  showcase) or rename to `pin.lini`.
- **add** `translate.lini` — a flow child and a `pin: top right` badge each
  nudged by `translate`, showing it reshapes nothing.
- **rewrite** `captions.lini` → footer is the **last** child (drop `side: bottom`).
- **rewrite** `wires_simple.lini` — wire-label `offset` → `translate`.
- **audit** `full_example.lini`, `templates_all.lini` — any `layout: row` group
  with a `|caption|` becomes a column (caption is now an in-flow child), and any
  `mount`/`at`/`margin`/`offset` is converted; badge usages need no change.

## 7. Tests & snapshots

- Update unit tests named/asserting the old model: `layout/mod.rs`
  (`group_caption_reserves_a_band…`, `caption_sits_above_the_content` → assert a
  column-flow caption sits above its siblings, no band growth), `resolve/program`
  (caption), `syntax/parser` (the `mount: on` literal in a parse fixture →
  `pin: …`), any `anchors`/`offset` assertions.
- Regenerate conformance snapshots: `cargo insta test` then review/accept; delete
  orphaned `.snap` files for removed samples.
- `tests/wiring.rs` / `wiring_sweep.rs` exclude wire-bearing samples already;
  confirm the rewritten samples still route (run the suite).

## 8. Verification (must all pass)

1. `cargo test` (unit + conformance + wiring).
2. `cargo clippy --all-targets -- -D warnings`.
3. `cargo fmt --all -- --check`.
4. Render `pin.lini`/`anchors.lini`, `translate.lini`, `captions.lini`,
   `full_example.lini` to PNG with `resvg --bake-vars` and eyeball: corners
   straddle, center sits centered, translate nudges without reflow, the badge
   tucks into the top-right, captions read as title/footer.

## 9. Sequencing

One implementation commit is fine, but the natural internal order is:
resolve (templates + rename) → layout (anchors/titles/mod) → render → samples →
tests/snapshots. Keep `cargo check` green between sub-steps. README is a separate
docs pass (it still says “`at:`, `mount`/`side`/`align` anchors”).

---

## Flags / judgment calls

- **`caption` kept** (font-size preset), not folded into `plain` — see §2.
- **Negative `gap` left as-is.** It is the one remaining overlap-via-spacing path
  and is outside the explicit change list; making `gap` non-negative (CSS-honest,
  matching the margin removal) is a clean follow-up if wanted — flagged, not done.
- **Wire-label `translate` stays in the tangent frame** (x along the wire, y to
  its left), matching the old `offset` — more useful than world axes for a label.
- **`pin` corner order** is vertical-then-horizontal (`top right`), matching the
  SPEC table; the reverse is an error.

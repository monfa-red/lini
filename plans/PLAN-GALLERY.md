# PLAN-GALLERY — the pretty-samples pass

Curated showcase figures for the lini-website range wall (and reused in the
ASCII beats, the carousel, `samples/`, and the README) before v1. **Prettiness
is the acceptance bar** — minimal source, balanced composition, feature-dense
but never crowded.

## Where pieces live

Wall/beat pieces are authored in `lini-website/design/hero/figures/src/`
(they are sized to cards); `render.sh` compiles them with **this repo's**
`target/release/lini` — build it fresh first so the schematic fixes land.
The playground wasm (`design/hero/lini_wasm_bg.wasm`) is built from this
repo's `crates/lini-wasm` and must be refreshed separately (own chunk).
Phase 3 backports the winners into `samples/`, replacing the weak showroom
files.

## The pretty bar — every piece must pass

- `--strict` clean on current main; deterministic re-render.
- **Nothing overlaps or touches**: labels clear of wires, nodes, and each
  other; dimensions never cross; air everywhere.
- Composition reads at thumbnail size: symmetric or deliberately balanced,
  near the card slot's aspect (±10%).
- Refined/light: thin strokes, wash-fill + ink-stroke pairing, ≤ 3 hues +
  gray per piece (charts may ride the palette walk); bold only for chrome.
- Source ≤ ~35 lines (wall) / ≤ ~22 (carousel beats) — simple code is part
  of the showcase.
- Checked as PNG in light mode, spot-checked dark.

## The wall — new layout (rows sum to 12 span units)

| Row | Card (file) | Span × h | Status |
|---|---|---|---|
| 1 | `mindmap` — Mindmaps | 7 × 300 | **redo, compact** |
| 1 | `chart` — Charts (grouped bars) | 5 × 300 | tighten |
| 2 | `entity_hero` — ER schemas | 4 × 250 | **redo, 3 tables** |
| 2 | `sequence` — Sequences | 4 × 250 | polish |
| 2 | `flow` — Flows and groups | 4 × 250 | **redo from scratch** |
| 3 *(new)* | `chart_line` — Line charts | 4 × 250 | new |
| 3 *(new)* | `chart_donut` — Pies & donuts | 4 × 250 | new |
| 3 *(new)* | `table` — Tables | 4 × 250 | new |
| 4 | `drawing_turned` — Engineering drawings | 5 × 320 | **redo, simpler, on a5** |
| 4 | `schematic_hero` — Circuit schematics | 7 × 320 | re-render (fixes), keep |

Plus one off-wall piece: `cmp_bush` in the ASCII section — **redo** as the
section-view beat (see below).

## Per-piece art direction

1. **`mindmap` — compact.** Root + 4 first-level branches × 2 short leaves
   (labels 1–3 words), icons on two branches only. Shows: `|mindmap|` preset —
   bilateral split, natural curves, palette walk, depth ramp. Kill the current
   density; every topic must be legible at card size.
2. **`chart` (bars) — tighter.** 2 series × 4 categories, legend, nothing
   else; trim width/height so the pane isn't airy-boring. Shows: `|bars|`,
   `categories:`, auto legend, outlined palette look.
3. **`chart_line` — new.** 3 lines: distinct hues, one `stroke-style: dashed`,
   `curve: smooth`, markers on one series only, one `|mark|` reference line
   (dashed, amber). Labels/legend clear of everything. Shows: multi-series,
   smooth curves, marks, dash meaning.
4. **`chart_donut` — new.** `|pie| { hole: 0.5 }`, 4 slices, legend. Maybe a
   short title. Shows: pie layout, palette walk per slice. (Alternative if
   preferred: a radar — `direction: radial` line+area.)
5. **`table` — new.** One handsome `|table|` ~4×4: header band, a numeric
   right-aligned column (`align:` list), a muted `|footer|` row. Shows: ruled
   grid, per-column alignment, header/footer sugar.
6. **`entity_hero` — 3 tables.** User / Order / Product triangle, one entity
   with a key-gutter column, crow's-foot ops (`-o<`, `-+<`), hue per entity.
   Balanced triangle composition, wires short and clean.
7. **`sequence` — polish.** Keep the shape (3–4 participants, loop, note);
   fix any label-touches-lifeline moments; named actor via box-wrapping-icon.
8. **`flow` — redo from scratch.** The current one fails everything: tight
   groups, colliding labels, no symmetry. New composition: one gradient hero
   node top-centre; two captioned groups mid (`direction: row`, roomy
   `gap`/`padding`); a data row bottom. ≤ 3 hues, mixed ops with meaning
   (`->` sync, `-->` cache, `~>` async), labels placed where nothing crosses.
   Symmetry is the brief.
9. **`drawing_turned` — redo, simpler, sheeted.** ONE turned part on
   `sheet: a5 landscape` with a minimal title block: `revolve:` profile with
   a chamfer + fillet, `thread:`, 3–4 dimensions + one leader — placed so
   **no dimension, marker, or callout overlaps anything**. Shows: pen,
   revolve, thread callout, auto-measured dims, page + title block.
10. **`schematic_hero` — keep.** Re-render with the fresh binary (latest
    schematic fixes); polish only if the new render shows seams.
11. **`cmp_bush` (ASCII beat) — redo.** A section-view part: hatched cut with
    a bore, `A–A` plane on a small companion view, few dims. Explicitly fix
    the current overlaps (4× hole count vs neighbours, ⌀34 vs the A–A line).
    Card-sized: 3–4 annotations max, low `density:`.

## Process

- Build `lini` release fresh; then per piece: one Opus agent (xhigh) gets
  `SKILL.md` + this plan's piece brief; it must compile `--strict`, render a
  PNG at card aspect, and self-check the pretty bar before returning. I review
  every render, art-direct 1–2 tweak rounds, commit per piece (lini-website
  repo for figures, this repo for anything showroom-bound). One–two agents at
  a time.
- After the wall: update `content.rs` WALL for the new row, re-run
  `render.sh`, view the page.
- Phase 2: refresh the playground wasm from `crates/lini-wasm`.
- Phase 3: backport winners into `samples/` (replace weak showroom files),
  `cargo test` + insta review; README picks its images from the same set.

## Log

- **2026-08-25 — wall complete.** All ten cards + the cmp_bush beat landed in
  lini-website (figures in `design/hero/figures/src/`, WALL row wired in
  `content.rs`, `render.sh` re-pointed, all SVGs regenerated on current lini —
  schematic fixes included). Page verified in the browser. Remaining: playground
  wasm refresh (phase 2), samples/ backport (phase 3), carousel touch-ups.

## Decisions (2026-08-25)

- New row: **line + donut + table** (radar and org-tree passed on).
- Carousel `hero_*`: **after the wall** — touched only where a wall winner
  directly improves one.
- Execution order: flow, mindmap (the two worst) → new chart row + table →
  ER, drawing, cmp_bush → bars tighten, sequence polish → schematic re-render
  → wire `content.rs` WALL + `render.sh`.

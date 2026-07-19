# SHEET-alpha5 — images, title blocks & projection assistance

The alpha.5 round, planned 2026-07-19 against the shipped alpha.4
(`GDT-alpha4.md`). Sources: ROADMAP 3.5 (end — projection assistance),
ROADMAP 3.6 (images & title blocks), PLAN-V1's alpha.5 contract, and the
design settlements in the ledger below. **One plan, one tag**: the ladder
renumbered when alpha.3 shipped inside `1.0.0-alpha.2` — this round
releases as **`v1.0.0-alpha.4`** when Stage 3 closes. **Stage 0 is the
SPEC pass; the amended SPEC is the contract**; this file holds the
settled decisions, the build order, and the execution log. The quality
bar is ROADMAP §5, verbatim. Everything is **additive** — no existing
sample or snapshot changes except where a stage names it.

Scope (ROADMAP 3.5 end + 3.6): **local image embedding** — `src:` paths
resolved from the source file, bytes read at resolve, SVG as a nested
id-rewritten `<svg>`, raster as base64 data URIs, the serve traversal
boundary generalized; the **title-block finish** — authored children as
ordinary cells after the generated fields; **projection construction
links** — the one legalized cross-view anchor form, lowered to
`|projection|` chrome. Two riders (the PLAN-TREE precedent): the
`drawing_section` TAPPED BUSH view crossing the sheet frame (a real
view/frame layout defect), and the `pcb` sample's board paint in dark
theme (a sample-authoring fix). View arrows are **deferred** (ledger 9).

Stages are sized for one session at ~60 % of a context window; sub-agents
per AGENTS.md (always with an explicit `model`). At each stage's end:
fmt/test/clippy clean, a **Log** line here.

---

## Decisions ledger (settled in design review — do not relitigate)

1. **One plan, one tag.** The ladder's alpha.5 row stands; versions
   renumbered — this round tags **`v1.0.0-alpha.4`** when Stage 3
   closes; pushing stays with Abbas.
2. **Local images embed at resolve.** `src:` takes a URL, a data URI, or
   a **local path** resolved against the source `.lini` file's
   directory. Local bytes are read **at resolve** (one read, one span):
   a missing or unreadable path is a source-spanned error; HTTP(S) URLs
   and authored `data:` URIs pass through untouched; the compiler never
   touches the network. Output is **deterministic from the bytes** —
   the same file and assets give byte-identical SVG on every run.
3. **The id-rewrite scheme.** An embedded SVG nests as a child `<svg>`
   — but nesting alone doesn't isolate ids (AUDIT D6), so every `id`
   the asset declares is prefixed **`lini-aN-`** (N the image's 1-based
   document order) and every internal reference follows: `url(#…)` in
   attributes and inline `style`, and fragment `href` / `xlink:href`.
   Document-order numbering is collision-free and deterministic; the
   asset's own bytes never leak an id into the host document.
4. **No `--no-embed`.** The old TODO's escape hatch dies: embedding is
   the one behavior for a local path — a self-contained SVG is the
   output contract, and a relative `<image href>` breaks the moment the
   SVG moves. An author who wants a live reference writes the URL (or
   data URI) form, which already passes through.
5. **The traversal boundary is serve's.** `lini serve DIR` already
   confines requests to the root; the boundary **generalizes past
   `.lini`** — a compile run by the server may read assets only inside
   the served root. `lini serve FILE` gains the boundary it lacks: its
   root is the file's directory. An asset escaping the boundary is a
   compile error naming it. A plain CLI compile is unbounded — you
   compile your own file.
6. **Title block: authored cells after generated fields.** A
   field-mode `|title-block|`'s authored children remain **ordinary
   cells placed after the generated ones**, in the same grid —
   `cell:` / `span:` honored; an authored cell landing on a generated
   field's slot errors. There is **no `logo:` property** — a logo is an
   `|image|` in a cell (or anywhere on the page); the BOM stays an
   ordinary `|table|`.
7. **The cross-view anchor spelling.** A **projection construction
   link** is written in the **sheet's scope** — outside every drawing,
   where both views are visible (the core sealed-body rule) — with the
   **unmarked `-` op**, each end a dot-path into a **different** view's
   geometry carrying the full drawing anchor vocabulary ([SPEC 15.2]):
   `side.screw:head - end.od:top`. This is the **one** legalized
   cross-view anchor form: it lowers at layout (never routed) to one
   **straight thin line** — sheet-space, drawn after `align: origin`
   and every seat have placed the views. Dimensions, mates, and marked
   ops across views stay errors — sealed bodies stand.
8. **`|projection|` is chrome.** The lowered line is generated
   `|projection|` chrome (a `|line|` — `--stroke-light`, weight 1, the
   support-line tone), joining the auto-chrome table as its tenth
   producer: `|projection| { … }` restyles or removes projection lines
   scope-wide, like all chrome; manual use is free.
9. **View arrows are deferred to post-1.0** (ROADMAP 6). An arrow
   marker — unlike a `|plane|` or `|magnifier|` — defines **no
   capture**: an arrow-sourced view transforms nothing, so `of: <arrow>`
   would buy only the composed "VIEW A (2:1)" string over the smart
   label an author already writes. New marker anatomy + a third `of:`
   kind for title sugar fails the cheapness test; construction lines
   ship without it, as ROADMAP sanctioned.
10. **The frame rider is a layout fix, not a sample nudge.** In
    `drawing_section`, the TAPPED BUSH view and its M12×2 callout cross
    the sheet's right inner frame, and the Ø34 text spills into the
    filing margin — view extents (annotations included) escape the
    page's content area silently, violating the no-clip/no-spill law.
    Stage 3 finds the one owner of view placement vs the frame and
    fixes it there — never a per-sample `translate:` patch.
11. **The pcb rider is sample paint.** The board's
    `gradient(--green-ink, --teal-ink)` reads as a bright mint field in
    dark theme (ink roles invert). The fix is authored paint that reads
    in both themes, keeping the sample's PCB character — a sample edit,
    no engine change.
12. **This round only.** The beta round explodes into its own doc at
    entry; nothing here reserves syntax for it.

---

## Stages

### Stage 0 — SPEC amendment: images, title blocks & projection

Write all law before any code. The SPEC alone must suffice to implement
Stages 1–3.

- [x] SPEC 7: the `|image|` row's "External URLs only" dies; a new
  **Images** subsection (beside Icons) states the source law
  (decision 2 — path resolution, resolve-time bytes, the error, the
  pass-through forms, determinism, no network) and points at SPEC 17
  for the embedded output.
- [x] SPEC 15.8: the "projection lines stay deferred" sentence is
  replaced by the **projection construction link** law (decisions 7–8
  — the sheet-scope form, the unmarked op, the one-exception anchor
  reading, straight lowering, the chrome hook, the error set); the
  title-block paragraph gains the authored-cells law (decision 6).
- [x] Surrounding law squared: 15.2's "drawing-scope only" sentence
  gains its one exception (a sheet's projection link); 15.7's
  auto-chrome table gains the tenth producer; SPEC 8 gains the
  `|projection|` template row.
- [x] SPEC 17: the **embedded assets** law — nested `<svg>` with the
  `lini-aN-` rewrite (decision 3), raster data URIs, authored URLs
  unchanged, byte-determinism.
- [x] SPEC 19: the serve boundary note (decision 5 — dir root
  generalized past `.lini`, file mode's directory root, CLI unbounded).
- [x] SPEC 16: `src` widens to URL / data URI / local path.
- [x] SPEC 20 rows: unreadable asset; asset escaping the served root;
  marked projection op; cross-view dimension / mate; projection link
  with both ends in one view; projection endpoint outside a drawing;
  authored title-block cell on a generated field's slot.
- [x] SPEC 23: the projection-lines deferred bullet dies (built);
  **view arrows** join the beyond-1.0 list (decision 9), and ROADMAP 6
  gains the same line.
- [x] Sync: ROADMAP's ladder row points here; PLAN-V1's alpha.5
  contract gains its round-entered note.

Acceptance: SPEC alone sufficient for Stages 1–3; anchors intact (every
`](#…)` resolves); `cargo test` untouched (1055).
**Log:** 2026-07-19 — **done**, one commit (SPEC + ROADMAP/PLAN-V1 + this
doc). Settled in the pass: the projection paragraph lands in 15.8 between
the multi-view story and Sections & details — the deferred sentence dies,
15.2 gains its one-exception clause, 15.7's producer table goes to ten,
SPEC 8 gains the `|projection|` row (`--stroke-light`, weight 1, manual
use free); the image law splits **source** (a new SPEC 7 Images
subsection — resolution, resolve-time bytes, no network, no opt-out)
from **output** (SPEC 17 embedded assets — the `lini-aN-` rewrite over
`id`, `url(#…)`, and fragment `href`s); the serve boundary rides the
existing root sentence in SPEC 19 (file mode's root = its directory,
plain CLI unbounded); seven SPEC 20 rows (two asset, four projection,
one title-block overlap); SPEC 23's projection bullet became the
nesting-gated remnant and view-letter arrows joined beyond-1.0 there
and in ROADMAP 6. Anchor sweep: 536 refs, zero broken; 1055 tests
untouched.

### Stage 1 — local image embedding

- [x] Resolve-time assets: an `|image|` whose `src:` is neither a URL
  nor a `data:` URI resolves against the source file's directory; the
  bytes load at resolve (the one read — layout and render reuse them),
  classified SVG / raster by content; missing / unreadable paths error
  with the `src:` span (decision 2).
- [x] The embed (decision 3): `emit_image` switches on the resolved
  form — nested `<svg>` mapped into the node box (`fit:` honored, as
  today's `preserveAspectRatio` mapping) with the `lini-aN-` id
  rewrite; raster as a base64 data-URI `<image>`; authored URLs / data
  URIs byte-identical to today.
- [x] Boundaries (decision 5): `dir_mode::resolve_in_root` generalizes
  past `.lini` for asset reads; the compile carries an optional asset
  root — serve dir mode passes the served root, serve file mode the
  file's directory, the CLI none; an escape errors naming the path.
- [x] Assets: a small repo logo (`samples/assets/`) — an SVG with
  internal ids/refs (gradient + `use`) to exercise the rewrite, plus a
  tiny raster; `samples/drawing_sheet.lini` gains the SVG logo as a
  page child (its title-block cell seat lands in Stage 2).
- [x] Tests: id-rewrite units (id, `url(#…)`, `href` forms; two assets
  don't collide); snapshot the embedded output; **byte-identical across
  two runs**; the missing-path and escape-root errors; PNG light + dark.

Acceptance: embedded output renders in resvg and a browser, byte-identical
across runs; a traversal attempt errors; remote-URL output unchanged.
**Log:** 2026-07-19 — **done** (`9bd8846`). The asset pass lives in
`resolve/assets.rs`: `embed_image` runs at the scene walk's `|image|`
branch (span = the `src:` decl), pass-through for `http(s)://`/`data:`,
else read → boundary-check → classify by content (SVG root sniff / magic
bytes); an SVG asset folds to `embed-svg` + `embed-attrs` attrs (the
sketch-`path` precedent) — placement attrs dropped, `viewBox`
synthesized from width×height when absent — and `emit_image` nests it;
rasters rewrite `src` to a data URI through the **one** base64 (fonts'
copy deduped into `assets::base64`). `Options` gained
`base_dir`/`asset_root`: CLI sets base from the input's directory (root
none — unbounded), serve dir mode anchors at the posted file's parent
inside the served root (playground compile now sends `?path=`), file
mode roots at its own directory. `resolve_in_root` was already
`.lini`-free on the read side; its test now proves an asset resolves.
Sample: `samples/assets/logo.svg` (gradient + two `use` refs) pinned
top-left inside `drawing_sheet`'s frame + `assets/mark.png` (85-byte
checker). Sample-sweeping suites (laws/oracle/fmt/conformance/
resolution + the drawing testutil) pass `base_dir: samples/` — testing
hooks gained `_with` variants; the three `fit:` unit tests now use a URL
src. One snapshot re-blessed (`drawing_sheet` — the logo). PNGs light +
dark verified in resvg: gradient + `use` dots render, nothing drifted.
Tests 1055 → **1073** (12 rewrite units + 7 integration, −1 dedup);
fmt/clippy/test clean.

### Stage 2 — title-block authored cells; the pcb rider

- [ ] The finish (decision 6): a field-mode `|title-block|`'s generated
  cells lead, authored children follow as ordinary cells in the same
  grid — `cell:` / `span:` honored against the generated rows; an
  authored cell landing on a generated field's slot errors naming the
  field; the plain-table form is untouched.
- [ ] The showroom seat: `drawing_sheet`'s logo moves into a
  title-block cell — the embedded-logo title block the round promised;
  `drawing_screw`'s block untouched (the field-only form stays
  represented).
- [ ] The pcb rider (decision 11): the board's paint re-authored to
  read in both themes, character kept; verified light + dark.
- [ ] Tests: authored-after-generated snapshot (cells, spans), the
  overlap error, fmt idempotent on the mixed block; PNG light + dark.

Acceptance: a field block with authored cells renders fields first,
authored cells where placed, or errors on overlap — never silent
stacking; the pcb board reads in both themes.
**Log:**

### Stage 3 — projection links; the frame rider; alpha.5 closes

- [ ] Resolve (decision 7): a sheet-scope link classifies as a
  **projection link** when both ends resolve through different drawing
  children — full 15.2 anchor vocabulary legal on its endpoints, the
  one exception; the error set wired (marked op, one-view ends,
  non-drawing end, cross-view dim / mate unchanged as errors).
- [ ] Layout (decision 8): after the page places its views, each end's
  anchor point maps to sheet space and the link lowers to one straight
  `|projection|` line — never routed, never an obstacle to dims (it is
  chrome, not annotation); `|projection| { }` restyles / removes.
- [ ] The frame rider (decision 10): diagnose which mechanism owns view
  extents vs the page frame (view painted bounds vs the content area);
  fix in that owner so a view's paint — annotations included — never
  crosses the frame silently; `drawing_section` re-blessed to a legal
  sheet.
- [ ] Sample: the DIN-912 sheet (`drawing_sheet.lini`) gains projection
  lines tying the side view's stations to the end view's diameters;
  snapshot + `lini desugar` fixed point.
- [ ] The round-closing visual pass (ROADMAP §5): every drawing-family
  sample light + dark at screen + print size; ladder row confirmed;
  bump `1.0.0-alpha.4`, tag `v1.0.0-alpha.4` (push deferred to Abbas).

Acceptance: projection lines land on the anchors views were aligned by,
style / remove via the cascade; a cross-view dimension still errors; no
view paint crosses the sheet frame across the page samples (oracle);
the tag is cut.
**Log:**

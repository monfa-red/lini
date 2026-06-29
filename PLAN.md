# PLAN — implementing CHARTS.md

A staged, executable plan to build the charts feature ([`CHARTS.md`](CHARTS.md), the
law) into Lini. Charts are **two new container layouts** (`layout: chart`,
`layout: pie`) that, at layout time, fix a shared data→pixel scale from their children
and **lower to existing primitives** (`line` / `poly` / `path` / `oval` / `block` /
text). The renderer, cascade, palette, gradients, theming, `--bake-vars`, `fmt`, and
determinism are all **reused, not reimplemented**.

This is a roadmap, not a line-by-line script: each step says its purpose, the files it
touches, **what it reuses**, **what is genuinely new**, the refactors it performs, what
it defers, and how it is verified. Decide the fine detail while implementing.

**Status (branch `charts`):** steps 1–7 ✓ all done — the whole `CHARTS.md` surface is
built. Merge `charts` → `main` is the user's to do.
Step 7 added `tooltip: rich | title | none` (`tooltip.rs`): a one-pass over the lowered
nodes that, for `rich` (default), drops a hidden `.lini-chart-tip` card after each titled
mark, revealed by a CSS `:hover` rule the renderer emits **live-only** — `--bake-vars`
drops both the card (skipped in `render_node`) and the rule, so baked output is byte-for-
byte unchanged. `title` keeps only the native `<title>` floor, `none` strips it. The two
tooltip role vars (`tip-bg`/`tip-fg`) and `prim::group` are the new shared pieces. The
chart samples were canonicalized with `lini fmt` (idempotent + semantic-preserving — no
snapshot change); `tests/fmt.rs` already round-trips every sample, charts included.
Deferred (CHARTS §20, unchanged): gauge, stacked areas, polar circular gridlines +
start-angle, on-slice / donut-centre labels, per-segment style list, time scale,
sunburst, per-datum style list; plus row annotations (column-oriented today).
Step 6 added `layout: pie` (the sibling layout in `pie.rs`: value→angle wedges via the
shared `prim::wedge`, `hole:` donut, per-slice palette, §18 pie errors) and `|bubble|`
(in `bubble.rs`: area-scaled ovals on the cartesian plane, label-when-it-fits-else-hover,
auto x-domain padded so edge bubbles fit). `prim::wedge` and `chart_box` /
`lay_out_legend` are factored and shared (chart + pie + radial bars), so no geometry or
box/legend logic is duplicated.
Step 5 added the projection seam `Plot::project(x, value)` with `Dir::{Column, Row,
Radial}` (column byte-identical — no cartesian churn); `direction: radial` (radar /
filled radar / radial-bar wedges / web in `radial.rs`) and `direction: row` (the
cartesian flip) both reuse the exact `areas`/`lines`/`dots`/`bars` builders — only the
projector, gridlines, and labels differ. `bars.rs` factored to one `visit_bars` mode
dispatch shared by all three directions. Row annotations (bands / `|mark|`) are deferred
(column-oriented today); radial uses one radius axis (a `side:` there is the §18 error).
Refactors done along the way: (a) a labelled **geometry primitive** (`|line|`) used to
have its label silently dropped at desugar, so a `|line|` series had no legend name —
desugar now keeps it and resolve carries it to `ResolvedInst.label` (inert for a
standalone shape); (b) the deferred-`fn:` foundation landed in step 3 with its consumer —
`FuncTable` on `Program`, threaded into `layout_inst`, and `expr::sample` factored out of
`scene::sample_points` so the parametric-`points:` and chart-`fn:` paths share one
ambient-eval seam. The palette walk picks the §10 role tier (line→`deep`, dots→`ink`,
bar/area→base). (c) Step 4 found a latent bug: `marker:` is extracted to the resolved
`Markers` (and dropped from the attr map), so the chart's `marker_on` read of
`attrs.get("marker")` was always false — line vertex markers never drew. Fixed to read
`inst.markers` (one `has_marker`), and the `|mark|` template carries a default
`marker: dot` so the marker cascade separates that point default from an explicit
`marker: none` (which resolve would otherwise collapse together). `prim::rect` gained an
`opacity` arg (omitted at 1, so opaque rects don't churn); `marks.rs` split into
`areas`/`lines`/`dots` passes for the §15 order; bands + `|mark|` share one `axis_px`
projector in `annot.rs`.

---

## 0. The overriding rule: reuse the core, never duplicate logic

The one failure mode to avoid: two places doing the same job, so a later fix lands in one
and the bug lives on in the copy. Every step below states its reuse explicitly. Before
adding code, find the function that already does the job and call it. The factored,
shared mechanisms this plan establishes — each built **once** (timing noted) and reused
everywhere after:

- **`prim::*` primitive builders** *(step 1)* — one set of constructors that build a
  `PlacedNode` of each kind (`line(points, paint)`, `rect(rect, paint)`,
  `poly(points, paint)`, `path(d, paint)`, `oval(rect, paint)`, `text(s, at, style)`).
  **Every** series, axis, gridline, tick, legend swatch, band, annotation, slice, and
  bubble builds its output through these — never an open-coded `PlacedNode { … }`. (Reuse
  target: the existing `PlacedNode` shape and the render emitters that already draw each
  kind.)
- **`Scale`** *(step 1)* — one data-domain→pixel-range map (linear, then log in step 3),
  with nice-tick generation, shared by every cartesian axis **and** the radial radius
  axis (§12).
- **`palette_walk`** *(step 1)* — one deterministic hue iterator (the §10 sequence,
  skipping `red`); it yields the **hue**, and each caller picks the role tier (a line the
  `deep`, an area/bar the base fill + `deep` edge, dots the `ink` — CHARTS §10). Shared by
  series colour assignment **and** pie/bubble per-datum colour; one shared helper also
  derives the legend-swatch / tooltip accent from a node's dominant paint (its `fill` else
  `stroke`).
- **`Projection`** *(cartesian seam in step 1; polar variant in step 5)* — one data→pixel
  projector. Built minimal in step 1 (cartesian column) and **every series/axis builder in
  steps 1–4 is written against it from the start**, so step 5 only *adds* the `row` and
  polar variants — there is **no retrofit** of the earlier steps. This is what lets radial
  **reuse every cartesian series** instead of cloning it.
- **one tick/label renderer** *(step 1, generalised through step 4)* — value-axis ticks,
  x-category labels, and band ticks are all "a label at a scaled position"; they share one
  renderer rather than three look-alikes.
- **`sample_expr`** *(factored in step 3, when `fn:` first needs it)* — one "evaluate an
  `Expr` over an ambient `Env`" helper, shared by resolve's existing parametric `points:`
  sampling (`scene::sample_points`) **and** the chart's `fn:` sampling (§4). Factor the
  common evaluator out of `sample_points` so neither copies the other.

If a step is tempted to copy logic from another, stop and factor the shared part here.

---

## Architecture decisions (verified against the code)

- **A chart is detected by its `layout:` attr** (`chart` / `pie`). The check is the
  **first statement in `layout::layout_inst`, with an early return** — *before* the
  child-recursion that runs today (which would call `leaf_bbox` on a `|line|`/`|area|`
  series, erroring for want of `points:`), and before the generic
  `lay_out_container_children`/`read_layout_mode` path (which would hit its "unknown
  layout" arm on `chart`). The chart branch in `layout_inst` is genuinely new. `type_chain`
  (`["chart"]`, `["bars"]`, …) names the series for dispatch and for error messages. The
  chart **owns its whole subtree**: it reads the children's resolved attrs and emits
  pre-positioned primitive `PlacedNode`s in plot-local `cx`/`cy`, with the chart's own
  fixed `bbox`. The parent scene then places the chart as one unit (no new code).
- **Series lower to primitives that already render.** A `|bars|`/`|area|`/`|dots|`/
  `|bubble|`/`|slice|` desugars to a `|block|` base carrying its type in `type_chain`;
  the chart reads `type_chain` and emits `Block`/`Poly`/`Path`/`Oval`/`Line`/`Text`
  `PlacedNode`s. `|line|` **reuses `NodeKind::Line`** — inside a chart it reads
  `data:`/`fn:`; standalone it reads `points:` (the chart branch decides).
- **Data parses on the existing value grammar — no grammar/lexer changes.** `data: 9 15`
  is one space-group → `Tuple([9,15])` (categorical); `data: 0 1, 2 3` is comma-groups →
  `List([Tuple,Tuple])` (points). List ⇒ points, non-List ⇒ categorical values is the
  discriminator (`resolve::value::resolve_groups` produces exactly this). Note the edge
  case the bars reader must handle: a **single** value `data: 9` collapses to a bare
  `Number(9)`, not `Tuple([9])` (`resolve_group`), so read "categorical" as
  *Number or Tuple*.
- **Series are styled by the cascade for free.** Because a series is a node, a rule
  `|bars| { fill: red }` (→ `.lini-bars { fill: red }`) reaches it through the normal
  resolve cascade. The chart reads the **resolved** `fill`/`stroke` and only assigns a
  palette colour when none is set.
- **Paint themes/flips/bakes for free.** The chart sets `fill`/`stroke` as
  `ResolvedValue::LiveVar` palette refs on the lowered nodes; `render::node_style_attr`
  diffs them to an inline `style=`, and `used_vars` tree-shakes them — so dark/light and
  `--bake-vars` work with no chart code. The new `--lini-grid` role var (added in
  defaults) is picked up the same way.
- **`<title>` is the baked-safe tooltip floor, already emitted.** `render_node` emits
  `<title>` from a node's `title:` attr — the chart sets `title:` on hit targets and gets
  a tooltip in every renderer, baked-safe, with no new code. The *rich* `:hover` card is
  the only tooltip piece that needs a new CSS path (§14, step 7).
- **Smart labels are already lowered.** Desugar turns `|chart| "T"` / `|bars| "S"` into a
  centred `Text` child (chart/series are block-based). The chart **harvests the label
  string** from that child and re-places it as title / legend / tick / axis-title; it does
  not render the generic centred placement. (Keeps desugar dumb; the chart owns chart
  semantics. Confirmed: post-resolve a box has `label: None` and the text is a child.)

### Two foundational refactors, each justified by reuse

1. **Thread `FuncTable` to layout (step 3, with its consumer).** Today `FuncTable` is
   built in `resolve::program` and dropped; `Program` does not carry it (verified). The
   chart samples `fn:` at layout time — so `pub funcs: FuncTable` on `Program`, its borrow
   into `layout_inst` (a small `LayoutCtx`), and the first read all land together in
   **step 3**, the fn step. Step 1 deliberately does *not* add the field: `data:`
   (including constant backticks) folds to numbers at resolve, so layout has no consumer
   yet, and a set-but-never-read field is dead code. `Program` is never cloned (`finish`
   clones only `vars`/`sheet`), so no `Clone` derive is needed.
2. **A deferred-expression carrier.** A `fn:` backtick references `x` (and `u`), unbound
   at resolve — folding it now (`resolve_scalar` → `fold_expr` with an empty env) errors
   *"unknown name 'x'"*. So `fn:` must be **held unevaluated** and sampled at layout once
   the x-domain is known (the same defer the spec calls out). Add
   `ResolvedValue::Deferred(...)` holding the parsed `Expr`(s) (`Expr` is `Clone+Debug`),
   built by intercepting `d.name == "fn"` in `resolve::scene::resolve_node`'s decl loop —
   beside the existing `points:` interception, the same mechanism, deferred to layout.
   It never escapes the chart (the chart evaluates it to baked numbers before emitting),
   so the two **exhaustive** `match`es on `ResolvedValue` — `render::values::format_value`
   and `layout::values::describe` — get an `unreachable!()` / diagnostic arm. (All other
   sites use `_ =>`.)

### Spec/impl gaps found while reading — resolve these as noted

- **Quoted-text rule for chart text props.** `is_string_valued` (resolve/value.rs)
  currently lists only `title|href|src|path`. CHARTS text props that carry user text —
  `categories`, `labels`, `unit` — must require quotes too (SPEC §2). Extend
  `is_string_valued` with them in step 1 (keyword props like `direction`/`scale`/`side`
  stay bare).
- **Chart/series label placement.** Confirmed above: harvest the label string from the
  lowered `Text` child; do not special-case desugar.
- **`fmt` round-trip.** Charts are ordinary nodes + decls, so `lini fmt` should
  canonicalize them already; step 7 verifies and adds a chart to the fmt tests. (The
  known, separate `lini fmt` table-cell `{ style }` drop bug does not touch charts —
  charts use no bare table cells — so it stays out of scope.)
- **`lini desugar` cannot show the lowered chart geometry (CHARTS §15 contradiction —
  surface to the user).** `lini desugar` runs lex→parse→desugar→print with **no layout**
  (`lib.rs::desugar_source`), but charts lower to bars/axes/slices **at layout**. So
  `lini desugar` can only show the *type* desugaring — `|chart|` → `|block| .lini-chart`
  with series as `.lini-bars` blocks carrying `data:` — never the geometric primitive
  tree CHARTS §15 ("`lini desugar` prints the lowered tree") implies. **Proposed fix:**
  amend CHARTS §15 to state that a chart's *geometric* lowering exists only post-layout
  (so `lini desugar` shows the type lowering; the full primitive tree is a render-time
  artefact, like routed link geometry); a diffable post-layout dump would be its own
  deferred tool. Get the user's nod on the wording before editing CHARTS.md.
- **Unknown `axis:` id suggestion (CHARTS §18).** `|axis#id|` children are **not** in the
  global `PathIndex` (it indexes scene nodes, not a chart's internal axes), so the
  `axis 'X' not found; did you mean 'Y'?` error needs a small chart-local suggestion over
  the chart's own `|axis|` ids (step 2), not the existing endpoint machinery.

---

## Steps

Dependencies are noted so independent steps can run in separate sessions (some by an
agent). **Step 1 is the hard prerequisite for all.** After it: 2→3→4 are the cartesian
spine; 5 (radial) needs 1–4; 6 (pie/bubble) needs only 1 (pie) and 2 (bubble) and can run
in parallel with 5; 7 is last (it touches all outputs).

### Step 1 — Foundation + minimal cartesian bars *(no dep)*

**Purpose.** Stand up every load-bearing seam and prove it end-to-end with the simplest
real chart: categorical vertical `|bars|` with an auto value axis, rendering to real
primitives.

**New.**
- `ResolvedValue::Deferred` carrier + the two match arms (above).
- `src/layout/chart/` module: `mod.rs` (orchestrator + interception; bakes the default
  **360 × 220** chart box here, not in the template), `scale.rs` (`Scale`: linear
  domain→range + nice ticks, including-zero for bars), `project.rs` (the minimal
  cartesian-column `Projection` seam — bars and axes lower **through it** so step 5 adds
  `row`/polar with no retrofit), `prim.rs` (the `prim::*` builders), `bars.rs`
  (categorical bars → `Block` nodes, with the **default grouped** side-by-side placement
  for ≥2 series — the §3 default; step 4 adds only `stacked`/`overlay`), `axis.rs` (the
  one tick/label renderer: value-axis ticks/gridlines/title + x category labels →
  `Line`/`Text`), `legend.rs` + `title.rs` (harvest labels → `Text`/swatch), `palette.rs`
  (`palette_walk` + the dominant-paint accent helper). Keep each file < ~500 LOC.
- `--lini-grid` role var in `resolve::defaults::built_in_defaults`.
- The CHARTS §14 **`<title>` floor**, set via `prim::*` on each hit target (bars here; the
  same one-liner is reused on line/dots/area/slice/bubble in steps 2–6).
- Chart errors (CHARTS §18): series-outside-chart, empty chart, `data`+`fn` both,
  neither, data-vs-categories count mismatch, `categories` + axis `labels` both. (Each
  later step enforces its own slice of §18 — axis/band/mark-outside-chart in 2/4,
  slice/pie errors in 6.)

**Reuse.** `resolve_groups` (data → Tuple/List); the cascade (series paint); `PlacedNode`
+ the render emitters (`emit_rect`/`emit_line`/text) draw the lowered nodes unchanged;
`format_value`/`used_vars` (palette vars theme + tree-shake); `render_node`'s `<title>`
floor (set `title:` on each bar); `text::approx_width`/`approx_height` for gutter sizing.

**Refactor.** Register chart types in desugar: add to `TEMPLATES`
(`chart`→block, `pie`→block, `area`/`bars`/`dots`/
`bubble`/`slice`/`axis`/`band`/`mark`→block; `line` already a primitive — **not** added);
`template_bundle("chart")`/`("pie")` set `layout: chart`/`pie`. Extend `is_string_valued`
with `categories`/`labels`/`unit`. Intercept `fn:` → `Deferred` in `resolve_node` (no
eval yet — nothing samples it until step 3, but the carrier must exist).

**Defer.** Lines/dots/axes-as-children (step 2); `fn:` sampling, area, log, smooth
(step 3); everything else.

**Verify.** Unit insta snapshots in the chart module; `samples/chart_bars.lini`
(snapshotted by `tests/conformance.rs`, must also pass `tests/resolution.rs`
all-samples-resolve); render it to PNG with `resvg` and read it. `cargo fmt && cargo test
&& cargo clippy` clean. **Acceptance:** a 1-series and a 2-series (grouped, side-by-side)
categorical bars chart renders correct, themed, baked SVG with a value axis, gridlines, x
labels, title, and legend; all existing snapshots unchanged.

### Step 2 — Lines, dots, axes, scales *(dep: 1)*

**Purpose.** The full cartesian static-data toolkit.

**New.** `|line|` (via `NodeKind::Line`, `data:` categorical + `x y` points → numeric x
scale) with `marker:` at every vertex and `curve: linear|step`; `|dots|`
(`Oval`/markers, diameter = `width`); `|axis|` child honouring
`side`/`range`/`scale: linear`/`step`/`ticks`/`unit`/`labels`/`gridlines`; `range: a b`
window + crop-to-plot + reverse (`a>b`); multiple value axes with the primary-axis-only
default gridline rule; `axis:` series→axis binding, with a **chart-local** unknown-id
suggestion (`axis 'X' not found; did you mean 'Y'?` — axes are not in the global
`PathIndex`, so match over the chart's own `|axis#id|` children). The `<title>` floor on
line vertices and dots (reusing step 1's pattern).

**Reuse.** `Scale` and `prim::*` and `palette_walk` from step 1; the **marker family**
(`MarkerKind`, `render::markers`, generalised from line ends to vertices — reuse, don't
re-emit); `Oval`/`Line` emitters; crop = clip points to the plot rect (plain geometry).

**Refactor.** Generalise step 1's value-axis code into `axis.rs` serving any
side/orientation; `Scale` gains `log` here only if convenient, else step 3.

**Defer.** `fn:`/area/log/smooth (3); bands/marks (4); radial (5).

**Verify.** Samples: `chart_line`, `chart_dots`, `chart_scatter`, `chart_dual_axis`,
`chart_reversed`. Snapshots + a PNG read each. Tests green.

### Step 3 — Formulas, area, log, smooth curve *(dep: 1, 2)*

**Purpose.** Computed series and the remaining scale/curve kinds.

**New.** `fn:` **sampling**: a single backtick over the x-domain (bind `x`) at `samples:`
steps; the per-band list form is wired but exercised in step 4. `|area|` →
`Poly`/`Path` + edge `Line`, with `baseline`. `scale: log` (decade ticks 1-2-5,
domain>0 error). `curve: smooth` (monotone cubic → `Path`, no overshoot).

**Reuse.** `sample_expr` (the **factored** evaluator shared with `scene::sample_points` —
do the refactor here so neither copies the other); the `Deferred` carrier from step 1
(now given its consumer: add `funcs` to `Program` + thread it into `layout_inst`);
`Scale`; `prim::path`/`prim::poly`; `path_bbox` already sizes `A`/`C` paths.

**Refactor.** Pull the shared ambient-eval loop out of `scene::sample_points` into
`sample_expr`; resolve's `points:` path and the chart's `fn:` path both call it. Thread
`&program.funcs` into `layout_inst` (the `LayoutCtx`) — its first consumer is here.

**Verify.** Samples: `chart_fn`, `chart_area`, `chart_log`, `chart_smooth`. The dual-axis
formula example from CHARTS §19. Snapshots + PNG reads.

### Step 4 — Bands, segmentation, annotations *(dep: 3)*

**Purpose.** Zones, segmented formulas, reference lines/points, multi-bar modes.

**New.** `|band|` (`span: a b` + `axis:` → background shade + tick + shared segment
boundaries); per-band `fn:` **list** sampled in band-local `u` (segments connect
end-to-start); `|mark|` (`at: V` → reference line; `at: X Y` → point; `marker: none` →
label-only; `axis:` required); the `bars:` switch adding `stacked` and `overlay` (the
`grouped` default placement already landed in step 1).

**Reuse.** `prim::*`; `Scale`; the `u`-ambient path of `sample_expr` (same seam as
parametric `points:`); palette/paint; the band shade is a `prim::rect` behind the plot.

**Verify.** Samples: `chart_bands`, `chart_segmented`, `chart_threshold`,
`chart_stacked`. Snapshots + PNG. Confirm an annotation survives a later `direction` flip
unchanged (it is value-on-named-axis).

### Step 5 — Direction (row) + radial (radar / radial-bar) *(dep: 1–4)*

**Purpose.** Orientation flip and polar charts, reusing every cartesian series. (Depends
on 3 for filled-radar `|area|` and on 4 for stacked radial-bar, hence 1–4.)

**New.** Two new `Projection` variants — `row` (value axis to bottom, x axis to left) and
polar; `direction: radial` (spokes from `categories`, one radius axis, polygon-web
gridlines; closed radar `|line|`, filled radar `|area|`, radial-bar wedges `|bars|`→
`Poly`/`Path`, spoke `|dots|`); `side:` in radial is an error.

**Reuse.** The series/axis builders from steps 1–4 are **unchanged** — they already lower
through `Projection` (built in step 1), so radar/radial-bar reuse the exact `|line|`/
`|area|`/`|bars|`/`|dots|` builders; only the projector differs. This is the central
no-duplication win: **no parallel radial series implementations, and no retrofit.**

**Refactor.** None on steps 1–4 (they were written against `Projection` from the start);
step 5 adds the projector variants plus the radial-only gridlines (polygon web) and the
closed-loop / angular-wedge geometry.

**Defer.** Polar circular gridlines + configurable start angle (CHARTS §20).

**Verify.** Samples: `chart_row`, `chart_radar`, `chart_radial_bar`. Cartesian snapshots
**unchanged** by the projection refactor; new snapshots + PNG.

### Step 6 — Pie, donut, bubble *(dep: 1 for pie; 2 for bubble — parallelizable with 5)*

**Purpose.** Part-to-whole and per-node bubbles.

**New.** `layout: pie`: value→angle, slices → `Path` arcs (clockwise from top), `hole:`
donut, per-slice `palette_walk`, legend from slice labels; pie errors (negative value,
zero total, non-`slice` child). `|bubble|` per-node mark (`at: x y`, `value:`
area-scaled, smart label centred-when-it-fits-else-hover) → `Oval`.

**Reuse.** `Path` arcs + `path_bbox` arc sizing; `palette_walk` (per datum); `prim::path`/
`prim::oval`; `Oval` emitter; the title/legend/`<title>` machinery.

**Verify.** Samples: `chart_pie`, `chart_donut`, `chart_bubbles` (CHARTS §19). Snapshots +
PNG. Self-contained (its own layout / per-node mark), so a good candidate to run as an
isolated session or agent in parallel with step 5.

### Step 7 — Rich tooltips, fmt, polish *(dep: all)*

**Purpose.** Live hover cards, formatter round-trip, final cosmetics.

**New.** `tooltip: rich|title|none`: keep the `<title>` floor (already there); add the
rich `:hover` card — a hidden `<g class="lini-chart-tip">` revealed by a `:hover` rule,
**live-only (dropped under `--bake-vars`)**. This needs the **one new render seam**:
`RuleSet`/`style_block` emit only flat `.lini .class {}` today, so add a minimal path for
a `:hover`-bearing rule + the reserved `.lini-chart-tip` class. Hit targets stay sparse
(~10–20/series).

**Reuse.** The existing `<style>`/`@layer`/bake machinery; the card body is `prim::*`
primitives; the `<title>` path.

**Refactor.** The `:hover` CSS seam (small, generic — usable by any future interactive
rule). Verify `lini fmt` round-trips every chart sample; add one chart to `tests/fmt.rs`.

**Verify.** Live-mode snapshot shows the `:hover` rule; baked mode omits it but keeps
`<title>`. `lini fmt --check` clean on all chart samples. Full `cargo fmt && cargo test
&& cargo clippy`. Final PNG pass over every sample.

---

## Cross-cutting verification (every step)

- Keep **all** existing tests green; existing snapshots must not churn (a chart adds
  snapshots; it must not change non-chart output).
- One `samples/*.lini` per feature (auto-snapshotted by `tests/conformance.rs` in
  `--bake-vars` mode; must also pass `tests/resolution.rs`).
- **Render to PNG with `resvg` and read it** — never ask the user to spot-check.
- Determinism: byte-identical output; the palette walk and draw order are fixed.
- `cargo clean -p lini` before a build when in doubt (the stale-binary trap: a
  "Finished in 0.0s" with no "Compiling" line means stale).
- Run `cargo fmt && cargo test && cargo clippy` before a step is "done". Descriptive
  commit per step; **never** a `Co-Authored-By` line; do not push (defer to the user).

## Out of scope (CHARTS §20 deferred)

Gauge; stacked areas; polar circular gridlines + configurable start angle; per-slice
explode / on-slice labels / centred donut total; per-segment style list; time scale;
sunburst; per-datum style list.

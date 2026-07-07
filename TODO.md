# TODO

## Diagnostics / error reporting

Many misuses are **silently ignored** instead of reported — the value just does
nothing. Build one context-aware pass, keyed by node kind (box / text / wire /
wire-label) → its valid properties; anything else warns (errors under
`--strict`), LSP-formatted (`file:line:col`).

- **Property on the wrong node** — `translate`/`pin`/`padding`/`width` on a wire
  do nothing; error and suggest a `|block|` label.
- **Unknown property**, with a "did you mean" hint (`paddding:` → `padding`).
- **Out-of-range / wrong-shape value** — `translate: 0 -10 0`, `pin: middle` —
  point at the offending token.
- Fold in the existing grid-prop checks (`cell` / `span` / `columns`).

## Animation (idea — later, very light)

Small, native CSS/SVG effects so the browser does the work and it degrades to a
static frame when baked: moving dashes/dots on wires (cf. d2), a gentle wobble, a
colour/shadow pulse, maybe animating a gradient. Live-only; currently a SPEC §19
non-goal to revisit.

## OKLCH colour output (idea)

The palette is generated from OKLCH but emitted as hex for renderer compatibility
(resvg / librsvg / email don't parse `oklch()`). Add an opt-in — a
`--color-space oklch` flag or similar — that emits the `--lini-*` palette and
gradient stops as `oklch(L C H)` for users who target modern browsers only. Hex
stays the default; oklch is the wide-gamut, perceptual path for those who can use
it. `oklch()` *input* already works (`fill: oklch(0.7, 0.14, 200)`).

## Image export — PNG / WebP (idea)

`lini x.lini -o x.png` / `-o x.webp` straight from the CLI (format from the
extension), so people don't have to pipe through an external resvg. Probably behind
a cargo `raster` feature so the default binary stays lean — opt in for raster.

Path (all pure Rust, no C):

- rasterize the (baked) SVG with **`resvg`** → a `tiny-skia` Pixmap. Same renderer we
  already point users at, so output matches it.
- **PNG** is free — `Pixmap::encode_png()` is built in.
- **WebP** via **`image-webp`**, lossless — the right mode for flat diagrams (lossy
  smears edges/text, and lossy is also the variant that needs C/libwebp).

Open, not settled:

- **binary size** — measure, don't guess (`cargo bloat`, or build with/without the
  feature and diff). The weight is `tiny-skia` + the font/shaping crates, not resvg
  itself (its 76 KiB crate page is just source). Could be small — decide from the number.
- **fonts** — resvg needs a font to draw text: bundle the monospace (a few hundred KB,
  byte-reproducible) or use system fonts (no size, not reproducible). Reproducible
  output leans bundle.
- **one mode only** — a raster can't carry `light-dark()`, so it bakes to a single
  theme (light default; `--theme dark` for dark). Auto dark/light is lost in a PNG.
- **resolution** — needs a `--scale 2` / `--width N` knob (today's `resvg --zoom`).
- **leaner alt** — draw straight to a tiny-skia Pixmap from the scene model (skip
  resvg/usvg): lighter, fully reproducible, but reimplements the render backend. Reuse
  the SVG via resvg for a first cut.

Nice-to-have, not urgent. Build later.

## Auto-layout (idea — `layout: auto`)

Opt-in modes where the *layout* places the nodes and connectors stay dumb (a straight
line or one curve), instead of the router doing the hard work. It inverts the default
— here Lini places, you don't — and lives alongside `sequence` / `chart` as another
per-container layout. Names and shape all open to explore.

Sketch (tentative):

- one `layout: auto` (or `layout: dag`), with a second rule picking the flavour, the
  way charts might take a `direction`: `direction: radial | flow | column` —
  - `radial` → the mindmap fan
  - `flow` → layered DAG / flowchart (Sugiyama)
  - `column` → tidy tree / org chart
- under it, `[ ]` nesting reads as **structure** (tree/graph edges), not box
  containment — a node's own child-layout (box → column) is superseded by the
  auto-layout, you just pick the node's *kind*.
- (a Sugiyama-ish DAG is roughly possible with a grid today, just manual and fiddly;
  auto-layout would do the placement for you.)

Two things here are **not** speculative — already true / already in the model, and the
real foundation (could just as well be noted outside auto-layout):

- **Composability.** Every node is a container, layout is per-container, and wires
  cross containers by dot-path. So a `flow` DAG with a `|chart|` node, next to a
  `radial` mindmap, wired together — one SVG. Heterogeneous diagrams composed and
  interconnected; nothing else does this.
- **Context-aware routing.** Wires already route *inside* a group (root is just a
  container — routing doesn't have to live at the top). The new bit is choosing the
  routing *strategy* from where the wire sits: orthogonal in a flow/manual container,
  a dumb straight/curve inside an auto-layout. So a mindmap can hold a child with
  orthogonally-wired internals, or vice versa — the sky's the limit.

Build later.

## Drawing features — next round (scored 1 easy → 5 hard)

Gap analysis against the ramjet drawings (`screw_tip_spike`, `cooling_ring`,
`screw` — the injection-screw sheet is the bar). The principle throughout: lini
is 2D — a *section is authored* (drawn with the pen + `hatch()`, as the bushing
already is), but everything *about* a section that is bookkeeping — the cutting
plane, the letters, the title, the scale ratio — composes from facts the engine
already has.

### Sections & details

- **Cutting-plane chrome (2).** A `|cutting-plane| "A"` child on the source view:
  the ISO line (thick ends, thin dash-dot middle), viewing-direction arrows,
  letters at both ends. Anchored at a station (`at:` + axis), drawn from the
  parent's sized geometry like `|centerline|` — same generated-chrome machinery.
  *How:* a template row (`::line`, `stroke-style: center`) in
  `desugar/{types,bundles}.rs`; its smart label is the letter. Layout fills
  geometry in `drawing/chrome.rs::fill` from the parent's geometry box — the
  line across it at `at:` (+ overhang), two short thick end strokes (a second
  `|line|` child pair or inline `stroke-width` segments), arrow markers via
  `prim::dim_marker`, letter text leaves beside the ends via `prim::text`.
  Reuses the `chrome:` marker plumbing end to end; only the multi-part lowering
  (line + arrows + letters from one chrome child) is new — follow `edges::fill`'s
  seed-clone play, cloning typed pieces from the seed.
- **Section title composition (2).** A view declaring `section: a` composes its
  title — `A-A (1:1.5)` — from the letter and its scale over the page's
  (`drawing scale ÷ page scale` is already the drafting ratio). Same for
  `detail: c` → `C (1:1)`. *How:* in `desugar/mod.rs` where a drawing's smart
  label lowers to the `|footnote|` (`labels::footnote_node`), a `section:` /
  `detail:` decl with **no label** synthesizes one: uppercase the ident,
  `A-A` vs `C`, then append the ratio at layout… the scale isn't known at
  desugar — so lower a placeholder footnote carrying a `section:` attr and let
  the drawing engine (`engine.rs`, where the title footnote is pinned) append
  `" (1:1.5)"` from `own ÷ parent ctx.scale`, formatted via `compose::fmt`
  (trim `1:1` to `1:1`, `1.5` decimals per the ≤2-dp law).
- **Detail marker (1).** `|detail-circle| "C"` on the source view — the thin
  circle over a region + the letter beside it. *How:* a template over `|oval|`
  (`stroke: --stroke-light; stroke-width: 1; fill: none`, `width:` the ⌀,
  smart label → a text leaf placed at the rim's upper-right). Not even chrome —
  a plain part-frame child with `translate:`, like `|balloon|`; one template
  row + a label-placement rule in the engine (letter at 45° on the rim,
  `note-offset` out).
- **Auto detail view (4).** The zoomed view *is* derivable — it's 2D: re-lay the
  source drawing's geometry at the detail's scale, clip to the circle, skip the
  source's annotations; the detail's own `[ ]` dims anchor into the re-laid
  clones. The screw's C/D/E details are exactly this. *How:* a `|detail|`
  template (`::drawing`) with `of: <source id>` + `at: x y` + `width:` (region ⌀,
  source units). At engine dispatch, `drawing::layout_node` for a `|detail|`
  resolves `of:` via `layout::node_at`, lays out the source's **geometry
  children** again under its own `Ctx { scale: own }` (layout_inst is already
  re-entrant — the same inst tree, a different ctx), shifts them by `−at × own`,
  and wraps them in a clip: new `PlacedNode` field or attr `clip: circle r` that
  `render/primitives.rs` emits as one `<clipPath>` in `<defs>` (id interned like
  gradients in `render/paints.rs`) + `clip-path=` on the group. The detail's own
  annotation links then lower against the re-laid kids — `annotate::Ctx.kids`
  already takes whatever slice it's given, so anchors (`screw:r1 (o)`) resolve in
  the clone for free. Source annotations are skipped by partitioning only
  geometry (the engine's `is_sheet` / link split already separates them). The
  border circle + letter reuse the detail-marker template. Risks: `break:` in
  the source (clone rides it — positions are per-inst, fine) and id collisions
  (the clone's kids are *placed* copies, never re-registered in the scene — only
  the detail's own links may address them, which is the point).

### From the ramjet sheets

- **Leader label follows the composed thread spec (1).** `bar:m10 <- "LH"` should
  read `M10×1.5 LH` — one-ended labels *follow* the value everywhere else
  (SPEC 15.6); today a label suppresses the auto-compose. *How:* in
  `leaders::callout`, when the anchor is threaded (`thread_spec` hits) and
  `w.texts` is non-empty, prepend the composed spec to the first text instead of
  skipping composition; SPEC 15.3's bare-leader sentence gains the follows
  clause.
- **Boxed datum letter (1).** ISO frames the `>- "A"` letter in a small square;
  ours is bare text. *How:* in `leaders::callout`'s datum arm, wrap the text
  leaf with a `prim::rect` sized `text width + 2×pad` (the `Stacked` text-box
  math in `dims.rs` already measures this), classes `lini-dim-line`; place at
  the landing.
- **Physical-size emission (1–2).** A `|page|` knows its millimetres — emit
  `width="210mm" height="148mm"` on the SVG for true-scale printing. *How:* the
  root `finish` (layout/mod.rs) knows the lone-page case (same predicate as the
  padding rule, moved to a shared helper); carry `physical: Option<(f64, f64)>`
  mm on `LaidOut`, and `render` emits the `width`/`height` attrs beside the
  viewBox (they don't affect on-screen scaling when a host sets CSS size).
  Guard: only when the viewbox *is* the sheet (lone page, padding 0).
- **Surface-finish symbol (3).** The Ra checkmark flag (`0.8` on a bent leader).
  *How:* SPEC 23's stated direction — glyphs as paths, like icons: a small
  built-in path set (`finish`, `finish-machined`, `finish-any`) drawn by a new
  `prim::glyph` at `marker_size × k`, placed by the callout machinery — a new
  leader head or, simpler, a `|finish| "0.8"` template the engine seats on a
  face like the datum triangle (surface normal from `Anchor::outward`, which
  already exists). Start with the seated-on-face form; the flag-on-leader
  variant after.
- **GD&T feature-control frames (3–4).** The boxed `⌰ 0.02 A` runout frames on
  the screw. *How:* a `|fcf|`-ish template over `|table|` (one row, hairline
  gutters — the table machinery *is* the frame) whose first cell is a glyph
  named by ident (`runout`, `flatness`, `position`, … — the same path-set
  mechanism as surface finish); authored
  `|fcf| { tol: 0.02; datum: a; kind: runout }` or cells. Wire to a feature
  with the ordinary two-ended link or seat like a note. Glyph set is the only
  real work; no new grammar (SPEC 23's promise).
- **Internal thread in section (3).** The screw's `E` detail: an internal
  `M10×1.5 LH` bore in a hatched section — a drawn bore should dress like the
  outside of a stud. *How:* `threads.rs` already finds the run and offsets
  toward the material by `level.signum()`; an inner-subpath segment needs the
  sign flipped — decide the material side from the subpath's winding (the pen
  knows even-odd nesting: a segment on an inner subpath flips). One sign
  decision + the major-vs-minor role swap in what draws thin.
- **Counterbore / countersink (2).** The remaining hole variants. *How:* like
  `thread:` on a hole — properties (`cbore: ⌀ depth`, `csink: ⌀ angle`) whose
  top-view chrome is one/two extra circles (`chrome::fill` arm, concentric at
  the datum), lowered exactly as the ¾ arc; `pattern:` replicates per copy for
  free. Side-view forms stay authored (they're section geometry).
- **Title-block fields (2–3).** The real ISO 7200 block is a fixed nested grid
  (Dept / Created by / Approved / Doc type / status / Title / DWG No. / Rev /
  Date / Sheet + a logo cell). *How:* keep `|title-block|` a table; add an
  ISO 7200 **define body** in the template registry — intrinsic children
  (`bodies` in desugar already materialize define bodies per instance) forming
  the fixed grid, each field cell carrying an id (`dept`, `title`, `dwg`,
  `rev`, `sheet`) so users fill them by path or by a `fields:`-style shorthand.
  Smart fills (sheet count, view-scale list) read at the page's `finish`, where
  both are known.
- **Local / data-URI images (2).** `|image|` is external-URL only — a title-block
  logo needs `src: "./logo.svg"` embedded for a self-contained SVG. *How:* at
  render, a relative/`file:` src reads the file (svg → inline `<g>` or nested
  `<svg>`; png/jpg/webp → base64 `data:` href), size from `width`/`height` as
  today; a `--no-embed` escape keeps URLs. CLI knows the input dir for
  resolution; the LSP/server path needs the same root. Determinism: bytes come
  from the file, so snapshots stay stable.
- **The injection-screw sample (3).** Mostly composable *today*: the sectioned
  barrel is a revolved root profile + two `pattern: grid` rows of flight-section
  sketches (bottom row offset half a pitch), hatched; the unsectioned overview's
  helix silhouettes are parametric `points:` curves (`points:` already takes a
  `u` expr). A stress sample to prove it and catch what breaks — likely
  suspects: hatch continuity across pattern copies, dim rows over 500-unit
  spans, the FEED/TRANSITION/METERING band. A `helix()`-ish pen helper only if
  the sample shows the need.
- **Zone-band annotations (1).** FEED / TRANSITION / METERING over 190/110/100 —
  works today (a chain dim + labels); a sample corner, not a feature. Verify in
  the screw sample.

### Deferred-list promotions (SPEC 23 already names these)

- **Repeated-segment counting (2)** — one `:segment` on several corners reading
  `4× R3`, as `pattern:` already prefixes counts. *How:* lift the
  duplicate-segment error into a per-name **list** in the pen
  (`Folded.segments` keyed name → Vec); anchors read the first, `compose`
  prefixes `N× ` when the list is longer — the same seat as the `pattern:`
  count.
- **Fan leaders (3)** — `a & b <- "2× R5"`, one note, two tips. *How:* the `&`
  fan already parses for links; resolve currently rejects it one-ended — allow
  it for leader ops, and `leaders::callout` draws one text + landing with a tip
  ray per endpoint (share the elbow, first anchor steers the direction).
- **Per-copy pattern anchors + pitch dims (3)** — `bolt.2`, hole-to-hole
  pitches. *How:* the carrier already holds real copy children; give copies
  synthetic ids (`m8.1`, `m8.2` — `pattern::expand` numbers them) and let the
  endpoint dot-path walk match them (anchors walk `PlacedNode.children` by id
  already); the SPEC 23 "unaddressable" gates come out.
- **Aligned (point-to-point) dims (3)** — today horizontal/vertical only.
  *How:* `dims.rs` gains an `Axis::Along(P)` — the dim line parallel to the
  anchor-to-anchor direction, extension lines perpendicular to it; the row
  packer treats it as its own side-less band (no packing against H/V rows —
  seat at `dim-offset` along the normal, `gap:` steers). The ISO-aligned text
  rotation machinery already handles arbitrary angles (the diametral line
  proves it).
- **`fillet` / `chamfer` against an arc (3)** — the pen joins straight runs
  only. *How:* `corner.rs` grows line↔arc and arc↔arc trim: solve the tangent
  circle of radius `r` against a line and a circle (offset-curve intersection —
  closed-form quadratics, no iteration); chamfer cuts back along each curve by
  arclength. The pending-slot and cyclic plumbing is untouched — only
  `apply_mod`'s geometry cases grow.
- **Dim-line breaks / halos (3)** — where extension lines cross geometry (the
  spike's ⌀ stack wants it). *How:* render-side halo, not layout: extension /
  dim lines get `paint-order`-style masking — emit each `lini-ext-line` with a
  wider background-colour understroke (`stroke: --bg; stroke-width: 3×`)
  beneath the line, one extra element, no geometry maths; drafting calls this
  the gap convention. Opt-out via cascade.
- **`explode:` (3)** — scale directed mates' separations for exploded views.
  *How:* `mates.rs` seat solves along the normal; an `explode: k` on the
  drawing multiplies each directed mate's resolved offset (including `gap:`)
  by `k` at seat time — one factor threaded through `Ctx`; balloons follow
  because they're placed after seating.

### Core debts the drawings keep hitting

- **The root-drawing router gap (3)** — a wire inside a nested flow scope of a
  *root* `{ layout: drawing }` is silently dropped (a node `|drawing|` routes
  it fine). The anonymous-container half of this family is **fixed**
  (2026-07-07: scope-transparency everywhere — lookups, engine filters, the
  anchor walk, auto-create; SPEC 9 states the model). What remains: the root
  drawing's `layout()` interception skips `routing::route` entirely — it
  should still route the links of nested non-drawing scopes. *How:* in the
  root-drawing arm of `layout()`, partition `program.links` by
  `is_drawing_scope` of each link's scope and hand the rest to the router
  (the router already routes inside containers).
- **`w` / `h` ambient in expressions (3)** — profiles full of `83.63`-style
  derived spans want `right(`l - 2*tip`)` against named constants today; fine,
  but the ambient keeps recurring in real profiles. *How:* still blocked on the
  circularity SPEC 10.7 notes (size needs the expression, the expression wants
  the size); scope it to the **pen only** — `w`/`h` bound to the profile's
  *declared-so-far* extent is still circular, so the honest cut is stylesheet
  constants per part (status quo) or `w`/`h` meaning the *parent drawing's*
  content box, which is known. Decide at spec time; don't build first.

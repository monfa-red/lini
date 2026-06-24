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

## Sequence diagrams (idea — `layout: sequence`)

A mode where wire *order* reads as time instead of spatial routing: named nodes
become participants across the top (each with a lifeline), and messages lay out
top-to-bottom in source order. The one part worth pinning down early is the entry
point — `layout: sequence;`. Everything below is just a sketch; better shapes are
worth exploring first. Guiding rule: **every feature reuses the existing syntax** —
this is the same node + wire grammar, just read on a time axis.

A sketch (open to better shapes):

```
{ layout: sequence }

user    |actor| "User"
browser |box|   "Browser"
server  |box|   "Server"

user    ->  browser "click login"   // -> call · --> return · ~> async · a->a self
browser ->  server  "POST /login"
loop |until valid| [                 // frames: loop · alt/else · opt
  server  --> browser "401"
  browser ->  server  "retry"
]
server  --> browser "200 OK"
browser ->  user    "dashboard"
note over server "validates token"
```

Renders to (the `loop` frame is drawn once, not unrolled):

```
 User        Browser       Server
  │             │             │
  │ click login │             │
  │────────────>│             │
  │             │ POST /login │
  │             │────────────>│
  │      ┌──────┼─────────────┼──┐
  │      │ loop: until valid  │  │
  │      │      │     401     │  │
  │      │      │<╌╌╌╌╌╌╌╌╌╌╌╌│  │
  │      │      │   retry     │  │
  │      │      │────────────>│  │
  │      └──────┼─────────────┼──┘
  │             │   200 OK    │
  │             │<╌╌╌╌╌╌╌╌╌╌╌╌│
  │  dashboard  │             │
  │<────────────│             │
```

Messages reuse the wire operators (`->` call, `-->` return, `~>` async, `a -> a`
self). Frames are static boxes around a span (not control flow — no repetition).
Notes (`note over a, b "…"`) sit over their lifelines; activation bars are "busy"
rects on a lifeline; `|actor|` is a stick-figure participant.

Open questions: frame syntax could reuse the `[ ]` children convention
(`loop |until valid| [ … ]`) or take another form (cf. Mermaid's `loop … end`);
participants explicit vs inferred from first use; a message as a wire vs its own
node kind. Build later as a full feature — it would inherit the palette/theming
for free.

## Charts (idea — `layout: chart` / `layout: pie`)

Rough and unfinished — a starting point, not a decision; needs real brainstorming.
Same guiding rule as elsewhere: reuse the existing grammar — a chart is a container,
its series are `[ ]` children (like `table`).

Probably two layouts, since two coordinate systems (but open):

- **`layout: chart`** — Cartesian, one shared auto-scaled x/y. Children choose how
  they paint the same plane: `|bars|`, `|line|`, `|area|`, `|scatter|`. Overlay /
  combo = more (or mixed) children.
- **`layout: pie`** — polar (value → angle); children are `|slice|`.

Tentative sketch:

```
{ layout: chart; x: Q1 Q2 Q3 Q4 } [
  revenue |bars| { data: 3 7 5 8; fill: --teal }
  costs   |bars| { data: 2 4 3 5; fill: --rose }    // 2 series → grouped, or `stack`
  target  |line| { data: 4 6 6 7; stroke: --amber }  // combo via mixed children
]

{ layout: pie } [
  ads |slice| { value: 40 }
  seo |slice| { value: 30 }                          // slices walk the palette
]
```

Loose suggestions, none settled:

- series ids could double as the **legend**, for free.
- `direction: row | column` maybe, to flip bars horizontal/vertical? (unsure — only
  bars flip; lines/areas don't.)
- a `stack` / `group` toggle on the parent for multi-series bars.
- the one real new mechanism: a chart reads *all* children's data first to set a
  shared scale (unlike row/column/grid) — a dedicated layout, like `table`'s grid.

Probably best kept small (inline data, categorical x, auto y, palette). Build later.

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
  auto-layout, you just pick the node's *shape*.
- (a Sugiyama-ish DAG is roughly possible with a grid today, just manual and fiddly;
  auto-layout would do the placement for you.)

Two things here are **not** speculative — already true / already in the model, and the
real foundation (could just as well be noted outside auto-layout):

- **Composability.** Every shape is a container, layout is per-container, and wires
  cross containers by dot-path. So a `flow` DAG with a `|chart|` node, next to a
  `radial` mindmap, wired together — one SVG. Heterogeneous diagrams composed and
  interconnected; nothing else does this.
- **Context-aware routing.** Wires already route *inside* a group (root is just a
  container — linking doesn't have to live at the top). The new bit is choosing the
  routing *strategy* from where the wire sits: orthogonal in a flow/manual container,
  a dumb straight/curve inside an auto-layout. So a mindmap can hold a child with
  orthogonally-wired internals, or vice versa — the sky's the limit.

Build later.

## Icons — shipped; follow-ups (`|icon|`)

`|icon| { symbol: … }` is built — **Phosphor** duotone, painted like a shape (SPEC
§7), behind the default-on `icons` feature; geometry vendored in
`assets/phosphor-duotone.txt` (regenerate with `cargo xtask extract-icons`). Still
open:

- a **solid (`fill`-weight) variant** for filled glyphs (today's set is the duotone
  line art; a true solid silhouette needs Phosphor's `fill` weight).
- **symbol-shapes as aliases** — let `cloud` / `database` resolve to icons directly,
  keeping the parametric container primitives (box / oval / hex / cyl / …).
- **user-supplied icons** via `|image|` — link a local or remote SVG/PNG.

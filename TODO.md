# TODO

## Diagnostics / error reporting

Many misuses are **silently ignored** instead of reported — the value just does
nothing. Build one context-aware pass, keyed by node kind (box / text / wire /
wire-label) → its valid properties; anything else warns (errors under
`--strict`), LSP-formatted (`file:line:col`).

- **Property on the wrong node** — `translate`/`pin`/`padding`/`width` on a wire
  do nothing; error and suggest a `|plain|` label.
- **Unknown property**, with a "did you mean" hint (`paddding:` → `padding`).
- **Out-of-range / wrong-shape value** — `translate: 0 -10 0`, `pin: middle` —
  point at the offending token.
- Fold in the existing grid-prop checks (`cell` / `span` / `columns`).

## Colour palette + gradients (idea — explore later)

Make lini *pretty by default*: a curated colour system where the easy path is
the pretty path and making things ugly takes more syntax. One mechanism, layered
on the existing `light-dark()` theme vars — so everything themes and flips
dark/light for free, and bakes to literals for resvg/email.

**Palette** — built-in `--lini-*` colours, each a `light-dark()` pair, native
`--name` (themeable, host-overridable):

- *Roles*: `--accent` + `--accent-2` (brand pair), plus today's
  `--fg` / `--bg` / `--fill` / `--stroke` / `--muted` / `--danger` / `--warn`.
- *Hues*: ~14 hand-tuned hues (`--red --orange --amber --yellow --green --teal
  --cyan --blue --indigo --violet --purple --pink --rose --gray`) — one tuned
  base each, with an optional `-100…-900` ramp reachable when wanted.
- Emit only the vars actually used (tree-shake the `@layer` block) so a big
  palette never bloats a small diagram.

**Gradients** — a `gradient()` paint value compiling to a deduped `<defs>`
`<linearGradient>` (a `GradientTable`, twin of the shadow `FilterTable`); the
node's paint becomes `url(#…)`. Stops are `var(--lini-*)`, so gradients theme and
flip dark/light. `objectBoundingBox` units fill any shape; works on **fill and
stroke**.

- `gradient` → the brand pair · `gradient(pink, purple)` → an auto-angled blend
  (pretty for any two hues) · `gradient(sunset)` → a curated preset ·
  `linear-gradient(135, …)` → full control (a custom angle is the "more syntax"
  gate; `gradient()` itself is angle-less and always lands on a flattering 135°).
- ~10 curated presets built from hue vars (sunset, ocean, candy, mint, aurora,
  ember, grape, dusk, sky, slate) for the fancy multi-stop look.
- Simple linear is plenty; radial comes ~free. Mesh isn't native SVG, so
  "multi-colour" = multi-stop. Gradient-on-text is a later step.

The taste lives in the system, not the user: curated hues + presets + an
auto-angle so *any* two colours look good.

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

<p align="center">
  <img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/logo/lini.svg" alt="Lini" width="256">
</p>

<p align="center"><strong>From a mindmap to a blueprint.</strong></p>

<p align="center">One small language for every kind of figure — pretty by default, precise when it has to be.</p>

<p align="center">
  <a href="https://crates.io/crates/lini"><img src="https://img.shields.io/crates/v/lini.svg" alt="crates.io"></a>
  <a href="https://docs.rs/lini"><img src="https://img.shields.io/docsrs/lini" alt="docs.rs"></a>
  <a href="https://github.com/monfa-red/lini/actions/workflows/ci.yml"><img src="https://github.com/monfa-red/lini/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/monfa-red/lini/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="license: MIT"></a>
</p>

```
cat -> dog -> bird
```

One line is a complete figure: three boxes, two arrows, sensible spacing. You place the boxes; Lini routes the links.

<p align="center">
  <img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/hero.svg" alt="A colourful service map rendered by Lini" width="440">
</p>

<p align="center"><em>Thirty-odd lines of Lini — <a href="https://github.com/monfa-red/lini/blob/main/samples/hero.lini"><code>samples/hero.lini</code></a>.</em></p>

<p align="center"><sub>flowcharts · mindmaps · charts · sequences · ER schemas · engineering drawings · floor plans · circuit schematics</sub></p>

---

## Why Lini

A compiler: plain text in, clean themeable SVG out. You decide where things go, and the parts you'd rather not do by hand — routing a wire through the gaps, measuring a dimension, picking a palette — are done for you.

- **You place, Lini routes.** Arrange nodes in rows, grids, or by anchor; name any two and it finds an orthogonal path between them, clear of everything in the way, keeping a clearance it won't cross — force a side to steer one.
- **One language, every kind of figure.** Charts, sequences, trees, mindmaps, ER schemas, engineering drawings, floor plans, and schematics are layouts over the same nodes and links. The same grammar draws all of them, so theming, baking, and diffing work identically in each.
- **The look is yours.** Sizes, anchors, strokes, shadows, rotation, gradients, and raw SVG paths render exactly as set, never filtered through a theme.
- **Measured, not drawn.** In a drawing, a dimension reads its value from the geometry: change the model and the numbers stay true.
- **A small language.** `{ }` for style, `[ ]` for children, a few sigils, and `cat -> dog` is already a figure. `(…)` adds compile-time math, baked to literals.
- **One binary.** 3.7 MB, no Node or browser, a typical figure in ~2 ms — byte-identical each run, so SVGs diff cleanly in CI.
- **Eleven tuned hues, and dark mode.** OKLCH ramps in five job-named tiers, gradients at a flattering angle, and one SVG that follows the viewer's OS.

---

## Install

```bash
cargo install lini            # or, from a clone: cargo install --path .
```

```bash
lini diagram.lini -o diagram.svg     # compile to SVG
lini serve diagram.lini              # live-reloading preview
lini fmt diagram.lini                # canonical formatting (--check for CI)
echo "a -> b -> c" | lini -          # stdin to stdout
```

---

## A tour

**A diagram reads like a CSS file.** A `{ }` stylesheet sets defaults, declares classes, and extends types; then come the instances, then the links.

```
{                                   // the stylesheet — pure setup, draws nothing
  clearance: 10;                     // cascades to every link
  |-|   { stroke: --gray-deep; }     // |-| styles every link's wire
  .hot  { fill: --red-wash; stroke: --red-ink; }
  |db::cyl| { fill: lightyellow; }   // a new type from the cylinder primitive
}

|box#api|   "API"
|box#queue| "Queue" .hot            // a node wears its class after the label
|db#store|  "Postgres"

api   -> queue "enqueue"
queue -> store "persist"
store ---> api "ack"                 // dotted arrow
```

**Containers lay their children out** — style in `{ }`, children in `[ ]`:

```
|group#services| "Services" { direction: row; gap: 24 } [
  |box#api|  "API"
  |box#auth| "Auth"
]
```

A flow orients with `direction: row` / `column`; a `grid` is sized by `columns` / `rows` and placed with `cell:` / `span:`; `pin` and `translate` lift a child out of the flow.

---

## Nodes

<p align="center"><img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/shapes.svg" alt="Lini's primitives and templates" width="480"></p>

Block, oval, hex, slant, cylinder, diamond, polygon, line, icon, and image — plus `path` for anything else.

```
|hex|  "hex" { width: 82; height: 72 }
|poly| { points: 0 -34, 32 11, 20 34, -20 34, -32 11; }
|path| { path: "M -34 6 C -34 -34 34 -34 34 6 C 20 34 -20 34 -34 6 Z"; }
```

Text is not a primitive: a bare `"…"` is content, styleable in place (`"x" { color: red }`), and `|block|` is the frameless box for a label needing an id or a link. Templates (`box`, `rect`, `group`, `caption`, `badge`, `row`, `column`, `grid`, `table`, `cell`, `header`, `entity`, `sign`, …) bundle common patterns, and you can define your own from any base: `|panel::group| { stroke: --accent; }`. A `|table|`'s first row becomes its header automatically.

---

## Icons

<p align="center"><img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/icons.svg" alt="Lini's built-in Phosphor icons and signs" width="520"></p>

Built-in **[Phosphor](https://phosphoricons.com/)** icons as inline SVG paths — no icon font, no external files. An icon paints like any node: `fill` is the body, `stroke` the line, `stroke-width` counter-scaled so the weight holds at any size.

```
|icon| .teal { symbol: user }                            // two-tone
|icon| { symbol: cloud; fill: none; stroke: --sky-deep } // single-tone line
|icon| "bell" .amber [ "3" ]                             // symbol via label, "3" a badge
|sign#svc| "gear" .purple [ "Service" ]                  // larger, labelled, linkable
```

Only the symbols a diagram uses are embedded.

---

## Charts

<p align="center"><img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/charts.svg" alt="Four Lini charts: grouped bars, smooth lines, a radar, and a banded area" width="700"></p>

`layout: chart` plots from data instead of pixels, working out the scale, ticks, gridlines, and a colour per series.

```
|chart| "Revenue ($M)" { categories: "Q1", "Q2", "Q3", "Q4" } [
  |bars| "2023" { data: 12, 19, 15, 25 }
  |bars| "2024" { data: 18, 24, 22, 31 }
]
```

`|bars|`, `|line|`, `|area|`, `|dots|`, and `|bubble|` share one x/value plane; `|slice|` makes a pie or donut. `direction: radial` bends the plane into a radar, `direction: row` lays it on its side — no change to the data. A series reads `data:` (numbers, or `x y` points) or `fn:`, a formula sampled over the domain. Axes auto-fit or take a `range:`, run linear / `log` / **time** (from date-valued points), and `format:` shapes tick labels. Shade a zone with `|band|`, mark a threshold with `|mark|`, label points with `labels:` — they place themselves, falling to hover where they don't fit. [`SPEC.md` §14](https://github.com/monfa-red/lini/blob/main/SPEC.md#14-charts)

---

## Sequences

<p align="center"><img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/sequence.svg" alt="A Lini sequence diagram: a checkout flow with activation bars, a loop frame, a self-message, and a note" width="530"></p>

`layout: sequence` reads the diagram on a **time axis** — participants across the top, messages top-to-bottom **in the order you write them**. No new syntax: participants are nodes, messages are links.

```
{ layout: sequence }
|box#user| "User"
|box#api|  "API"
user -> api  "POST /login"   // a call — solid arrow, opens an activation bar
api --> user "200 + token"   // a return — dashed, closes it
|alt| "valid" [              // a frame; |else| "…" splits compartments
  user ~> api "log event"    // async — wavy
]
```

The operator picks the message: `->` a call, `-->` a return, `~>` async, `a -> a` a self-message. Calls open **activation bars** and returns close them, nesting automatically. `|loop|` / `|opt|` / `|alt|` frame a span without opening a scope, and `|note| "…" { place: over a }` annotates. [`SPEC.md` §13](https://github.com/monfa-red/lini/blob/main/SPEC.md#13-sequence)

---

## Trees & mindmaps

<p align="center"><img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/mindmap.svg" alt="A Lini mindmap: a centred root with six colour-tinted branches on smooth curves" width="700"></p>

`layout: tree` makes **nesting the hierarchy** — every `|topic|` child is a branch, source order is sibling order, and branch wires are generated for you.

```
|mindmap#product| "Product Vision" [
  |topic#ship| "Shipping cadence" [
    |topic| "Weekly release train"
    |topic| "Feature flags gate the risky work"
  ]
  |topic#team| "Team" [ |topic| "Small and senior" ]   // side: left|right overrides
]
```

`direction: bilateral` fans first-level topics around a centred root; `|mindmap|` presets the look — bilateral, `routing: natural` smooth curves, a palette walk tinting each branch (dark mode included), and a depth ramp. Branch wires are ordinary generated links, so `lini desugar` shows them and `#ship |-| { … }` restyles one arm. Plain `layout: tree` stays a neutral org chart. [`SPEC.md` §12](https://github.com/monfa-red/lini/blob/main/SPEC.md#12-flow-grid--tree)

---

## Links

Name two nodes and Lini finds an orthogonal path through the free space, keeping a configurable `clearance` (default 16) from every node and link, rounding corners, landing the arrowhead on the edge.

The operator is the link's look, written `[start][line][end]` with no spaces:

| Line | | Markers | |
|---|---|---|---|
| `-` solid | `--` dashed | `>` arrow | `*` dot |
| `---` dotted | `~` wavy | `<` crow | `<>` diamond |

So `->` is a solid arrow, `<->` bidirectional, `--*` a dashed line ending in a dot. Endpoints support fan-out, fan-in, and cartesian fans with `&`, and dot-paths into containers (`closet.outlet -> fridge.inlet`). Force a side to steer one (`a:right -> b:left`). Labels ride the link and slide to clear nodes; the link never moves for a label.

Full routing contract — crossings, priority, self-loops, starvation: [`ROUTING.md`](https://github.com/monfa-red/lini/blob/main/ROUTING.md)

---

## Entities

<p align="center"><img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/entity_hero.svg" alt="An e-commerce ER schema in Lini: six entities wired with crow's-foot cardinality" width="470"></p>

An `|entity|` is an ER / database card built on `|table|` — its label is the title, each row `"field" "type"` (add a third column for a `PK` / `FK` gutter). Relationships are ordinary links with the **crow's-foot** operators:

| Op | Reads | Op | Reads |
|---|---|---|---|
| `-+` | one | `-o+` | zero-or-one |
| `-<` | many | `-+<` | one-or-many |
| `-++` | exactly one | `-o<` | zero-or-many |

Each marker composes `[min][max]` — the ring `o` (zero) or bar `+` (one) hugs the line, the crow `<` (many) sits at the entity — and works on **either end** (`a +-< b` is one-to-many). Entities lay out in any container: grid, flow, or free-positioned with `pin` / `translate`.

---

## Engineering drawings

`layout: drawing` turns a profile drawn with a pen into a **dimensioned technical sheet** — and every dimension's value is **measured from the geometry**, so the numbers stay true when the model changes.

Parts *mate* against each other, holes and patterns punch through, a half-profile *revolves* into a turned part (every shoulder drawing its edge line), `thread:` dresses a surface with ISO minor lines and composes its own `M20×1.5` callout, and a long bar *breaks* to fit. Dimensions live in the `( )` bracket — `(-)` a linear span, `(o)` a diameter or radius, `(<)` an angle — with leaders, datums (`>-`), and hatched sections for the rest. **GD&T** is first-class: `|control|` rows carry `tol:` / `zone:` / `datums:` with the ISO modifier glyphs, plus `|feature-control|`, `|surface-finish|`, and datum triangles that plant against the geometry. Views project from one another, and an ISO 5457 `|page|` (frame, zone references, seated ISO 7200 `|title-block|`) hosts them at true millimetre scale. [`SPEC.md` §15](https://github.com/monfa-red/lini/blob/main/SPEC.md#15-drawing)

---

## Floor plans

`layout: floorplan` is the same drawing engine under an architect's vocabulary — the datum, `scale:` / `unit:`, anchors, dimensions and leaders all work unchanged, and the dialect adds the words a plan needs.

```
{ layout: floorplan; unit: m; scale: 0.02 }      // 1 : 50

|wall#outer| {
  draw: move(0, 0) right(7.2):north down(4.8):east left(7.2):south close():west;
} [
  |door#entry| { on: south; at: 3; swing: right }   // 900 mm, the default
  |window|     { on: north; at: 0.9; width: 1.8 }
]
|bed| { translate: 1.2 1.2; rotate: 90 }
outer:west (-) outer:east { side: top }             // → 7.2, centreline to centreline
outer:west-in (-) outer:east-in { side: bottom }    // → 7.0, the clear span
```

A `|wall|` traces its **centreline** and `thickness:` (200 mm; a `|partition|` is the 100 mm interior define) grows it into the mitred poché outline that takes the paint. `|door|` / `|window|` ride the wall's `[ ]`, stationed `on:` a named segment `at:` a distance — they clip the outline at the jambs and draw their own leaf, quarter swing arc (`hinge:` × `swing:`, `symbol: single | double | sliding`) and sill pair. Six symbol-bodied fixtures — `|bed| |sofa| |dining| |bath| |appliance| |stairs|` — draw at **true size in physical millimetres**, converted through the scope's `unit:`, so a tub is 1700 × 750 mm whether the file drafts in `m` or `mm`. Counters and islands are plain `|rect|`s; anything else is a `|sketch|` define. Every named wall run also derives its two **face anchors**, `name-in` / `name-out`, so a dimension reads the structural centreline or the **clear** span an architect publishes, face to face. [`SPEC.md` §15.11](https://github.com/monfa-red/lini/blob/main/SPEC.md#1511-floorplan--the-architectural-dialect)

---

## Schematics

<p align="center"><img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/schematic_hero.svg" alt="A Lini schematic sheet: an ISO A4 page with two captioned regions, a TMC2300 driver with pins, a decoupling capacitor and ground, net labels, connectors, and a title block" width="760"></p>

`layout: schematic` reads a diagram as a **circuit sheet**. It places the parts and lets the ordinary router draw the wires — landing them on **fixed ports** (a pin's stub tip, a label's connection point), bending square, and dotting the junctions.

```
{ layout: schematic }

|component#u1| "AMS1117-3.3" [
  |pin#vin| { side: left; number: 3 }
  |pin#vout| { side: right; number: 2 }
  |pin#gnd| { side: bottom; number: 1 }
]
|C#c1| "22u"

u1.vout - c1 - |gnd|      // a chain passes *through* the cap: p1 in, p2 out
u1.gnd  - |gnd|           // a capsule declares where it is wired
u1.vin  -> "5V"           // a one-ended wire mints the tag it points at
```

Parts split by role: a 3+-pin part is an **anchor** on the scope's own track grid (`cell:` places it, ordinal, empty tracks collapsing), and a label or unplaced 1–2-pin part is a **satellite** — seated at the pin its wire touches, hanging off that wire's first leg and growing the way its terminator is *drawn*, so a ground always points down and a power flag always up. Uppercase types are the discretes (`|R| |C| |L| |D| |LED| |Q| |Y| |F| |FB| |SW| |BT| |V| |I|`, plus `|opamp|` and `|J|`), each with generated pins and `symbol:` variants; the type is the reference family, so an anonymous one mints R1, C1, D1… `|label|` is the net tag — text, a symbol (`gnd`, `power`, `earth`, `antenna`, …), or both, its `shape:` drawn plain, pointed, or round. Wire it all on an ISO `|page|` with a `|title-block|`. [`SPEC.md` §16](https://github.com/monfa-red/lini/blob/main/SPEC.md#16-schematic)

---

## Colour & theming

<p align="center">
  <img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/palette.png" alt="Lini's 11-hue palette in five tiers, light mode" width="320">
  <img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/palette-dark.png" alt="The same palette under dark mode" width="320">
</p>

Eleven named hues — `red rose orange amber lime green teal sky blue purple gray` — each in five **job-named** tiers: `wash` (palest, backgrounds), `soft`, the bare name (the everyday pastel), `deep` (borders and strokes), `ink` (text and emphasis). The names hold across the dark flip — `--teal-ink` is the high-contrast tone in *both* modes, where a `light`/`dark` name would invert.

```
{ |card::box| { fill: --teal-wash; stroke: --teal-ink } }   // a soft card, one line
|box#hero| { fill: gradient(--rose, --amber, --sky) }       // a three-colour blend
```

OKLCH under the hood, so the ramp is perceptually even; pick any colour the same way (`fill: oklch(0.7, 0.14, 200)`). `gradient(--rose, --sky)` blends two hues at a flattering angle, on **fill and stroke**.

**One SVG, both palettes.** Every colour is a `light-dark()` pair, so an export follows the viewer's OS with no script — and `data-theme="dark"`/`"light"` on any ancestor overrides it. Defaults sit in `@layer lini.defaults`, so unlayered host CSS wins with no `!important`:

```css
.lini { --lini-accent: #ff6600; }   /* recolour every diagram on the page */
```

Geometry is always baked, so a theme only ever changes colour. `--theme light|dark|high-contrast|blueprint` pins one palette at export
(`blueprint` is the white-on-cyanotype-blue print, for any diagram); `lini theme NAME` prints it as an editable `--lini-*` file; `--static` flattens to literals and outlines text to paths for non-browser renderers and email.

**Backgrounds are opt-in.** An SVG drops straight into a page: it paints no backdrop unless the scene asks for one, so the page shows through. Give the root a `fill:` when you want a plate — `fill: --bg` for the themed one, any colour or `gradient(…)` for your own. `--static` is the exception: that output is a standalone file, so it bakes `--lini-bg` in.

Text is **Google Sans** (SIL OFL), the bundled proportional family Lini also measures with — real per-glyph metrics, so boxes hug their text exactly. `font-family: "Google Sans Code"` swaps to the bundled mono face. SVGs carry font *names* by default; `--embed-font` inlines the used weights for browsers, `--static` outlines them for anything else.

---

## The CLI

```
lini [options] <input.lini>
lini fmt     [--check] [--stdout] <input.lini>
lini serve   [--port N] [--static] [PATH]
lini desugar <input.lini>
lini theme   [NAME]
```

| Flag | Meaning |
|---|---|
| `-o, --output FILE` | Output path (default: stdout). |
| `--format svg\|html` | Raw SVG (default), or a minimal HTML page. |
| `--static` | Inline `var()` references and outline text to paths — resvg, librsvg, raster, email. |
| `--embed-font` | Embed the used font weights as base64 `@font-face` — browser-only. |
| `--theme NAME\|FILE` | A built-in theme, a CSS file, or a `light/dark` pair. |
| `--check` | Parse and validate only. |
| `--json` | Diagnostics as JSON — stable codes, spans, machine-applicable fixes. |
| `--watch` | Recompile on every change (with `-o`). |
| `--no-warn` / `--strict` | Silence warnings, or promote them to errors. |

Errors are LSP-formatted (`file:line:col: error: …`), carry a **stable code** (`V001` unknown-property, `R008` unknown-endpoint, …), and suggest fixes — an unknown endpoint asks *did you mean `kitchen.counter.bowl`?*, and `--json` hands a tool the exact edit. `lini desugar` prints a file with its sugar expanded.

**`lini serve`** is a live preview at `localhost:7700` — and a browser playground. Point it at a folder and it lists the `.lini` files inside; pick one to open it in a small editor, source left, diagram rendering live right. Syntax highlighting, a draggable split, light/dark following your system. `Ctrl`/`Cmd`-`S` renders; **Save** writes back.

```bash
lini serve samples/        # browse, edit, and render the bundled examples
lini serve diagram.lini    # a single file — live-reloads on every save
```

---

## Performance

End-to-end on a modern laptop, including process startup (`--static`, output discarded):

| Diagram | Time |
|---|---|
| One node | ~1.6 ms |
| Realistic service diagram (9 nodes, 5 links) | ~2.2 ms |
| Dense scene (100 nodes, 90 routed links) | ~50 ms |

---

## Where Lini fits

| | Lini | Auto-layout tools* |
|---|---|---|
| Placement | **you control** (flex / grid / anchors) | automatic |
| Link routing | automatic, orthogonal — **steerable sides** | automatic |
| Visual control | **full SVG** (CSS vars + classes) | theme presets |
| Runtime | **single native binary**, in Rust | varies (Node, browser, JVM, …) |

<sub>*the common auto-layout diagram generators that place nodes for you from a text description</sub>

Reach for Lini when you already have a layout in mind — a grid, a top-down flow, framed groups — and want it to look that way without drawing the connectors by hand.

---

## For tools and editors

The whole language is published as a **machine-readable contract, generated from the same property ledger the compiler reads** — so it can't drift from the code:

- **[`schema/lini.schema.json`](https://github.com/monfa-red/lini/blob/main/schema/lini.schema.json)** — every primitive, template, role, and property, with owners, value shape, resolved default, inheritance channel, deferred flags, and a compiled example each. [`schema/reference.md`](https://github.com/monfa-red/lini/blob/main/schema/reference.md) is the compact human mirror.
- **`lini … --json`** — diagnostics with stable codes, spans, and fixes an editor can apply verbatim.
- **Editor grammars** — VS Code and Zed highlighting under [`editors/`](https://github.com/monfa-red/lini/tree/main/editors), keyword lists generated from the ledger so a new property highlights the moment it lands.

A CI drift test regenerates all three and asserts byte-equality, so a stale checkout fails rather than shipping.

---

## Status

**1.0.0-beta — feature-complete for 1.0.** The language in [`SPEC.md`](https://github.com/monfa-red/lini/blob/main/SPEC.md) is frozen and the [stability contract](https://github.com/monfa-red/lini/blob/main/ROADMAP.md) holds: syntax, property names, value shapes, defaults, diagnostic codes, and the theming surface (`--lini-*` vars, `.lini-*` classes, SVG structure) don't break before a 2.0 — growth is additive.

Every layout lowers to the same primitives, so theming, baking, and diffing work everywhere. The formatter, dev server, schema, and structured diagnostics ship in the one binary. What remains before 1.0 is soak and polish — the ladder is in [`ROADMAP.md`](https://github.com/monfa-red/lini/blob/main/ROADMAP.md).

---

## Development

```bash
cargo test                          # full suite: unit, snapshot, routing laws
cargo run -- samples/hello.lini
cargo run -- serve samples/hero.lini
```

A linear pipeline, each stage independently testable:

```
lex → parse → desugar → resolve → layout → route → render
```

Recursive-descent parsing over an LL(1) grammar; **desugar** lowers every bit of surface sugar to primitives and classes — templates, defines, smart labels, chain expansion, tree branch links, implicit nodes — so the engine's true input is inspectable (`lini desugar` prints exactly this, and the pass is idempotent); resolve applies CSS-like specificity; layout sizes bottom-up, and a layout-owning container (chart, sequence, drawing) lowers its whole subtree to primitives here; the router solves the remaining links against a clearance contract; render emits semantic SVG. `samples/` holds a `.lini` per feature area — `tests/conformance.rs` snapshots their SVG with `insta`, `tests/laws.rs` asserts the router's laws on every scene.

## License

MIT — see [LICENSE](https://github.com/monfa-red/lini/blob/main/LICENSE).

# Lini

**Pretty diagrams from plain text, with fine-grained control.**

[![crates.io](https://img.shields.io/crates/v/lini.svg)](https://crates.io/crates/lini)
[![docs.rs](https://img.shields.io/docsrs/lini)](https://docs.rs/lini)
[![CI](https://github.com/monfa-red/lini/actions/workflows/ci.yml/badge.svg)](https://github.com/monfa-red/lini/actions/workflows/ci.yml)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/monfa-red/lini/blob/main/LICENSE)

```
cat -> dog -> bird
```

One line is a complete diagram: three boxes, two arrows, sensible spacing. You place the boxes; Lini routes the links. The same syntax scales to the polished scene below.

<p align="center">
  <img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/hero.png" alt="A colourful service map rendered by Lini" width="450">
</p>

Thirty-odd lines of Lini ([`samples/hero.lini`](https://github.com/monfa-red/lini/blob/main/samples/hero.lini)).

---

## Why Lini

Lini handles the fiddly part of a diagram — drawing the connectors — and leaves the layout to you. Arrange nodes in rows, grids, or by anchor; name any two and Lini routes a clean orthogonal path between them, staying clear of everything in the way.

- **You place, Lini connects.** Routing is automatic, orthogonal, and rounded, with a clearance it won't cross. Force a side when you want to steer one.
- **The look is yours.** Sizes, anchors, strokes, shadows, rotation, gradients, and raw SVG paths render exactly as set — never filtered through a theme.
- **Charts from data.** `layout: chart` plots bars, lines, areas, scatter, radar, and pie straight from numbers, working out the scales, ticks, and colours for you.
- **Sequence diagrams.** `layout: sequence` reads your wires as time — participants across the top, messages top-to-bottom, with activation bars, `loop` / `opt` / `alt` frames, and notes, all from the links you already write.
- **Small, and quick to learn.** `{ }` for style, `[ ]` for children, a few sigils, and `cat -> dog` is already a diagram. Backtick expressions add compile-time math, baked to literals.
- **One fast binary.** About 1.5 MB, no Node or browser, compiling a typical diagram in a couple of milliseconds — and byte-identically each run, so SVGs diff cleanly in CI. Hundreds of tests, property tests on the router included, keep it honest.
- **Good colour for free.** Eleven OKLCH-tuned hues in five tiers, gradients at a flattering angle, and automatic dark mode — every colour a `light-dark()` variable, no hex to pick.

---

## Install

```bash
cargo install lini
```

```bash
lini diagram.lini -o diagram.svg     # compile to SVG
lini serve diagram.lini              # live-reloading preview in your browser
lini fmt diagram.lini                # canonical formatting (--check for CI)
echo "a -> b -> c" | lini -          # read stdin, write stdout
```

Building from a clone instead? `cargo install --path .`

---

## A tour of Lini

**A diagram reads like a CSS file.** A `{ }` stylesheet at the top sets defaults, declares reusable classes, and extends nodes; then come the instances, then the links:

```
{                                   // the stylesheet — pure setup, draws nothing
  link: --gray-deep; clearance: 10;   // link defaults cascade to every link
  .hot  { fill: --red-wash; stroke: --red-ink; }  // a node class
  .loud { link: red; link-width: 2; }       // a link class
  |db::cyl| { fill: lightyellow; }    // a new type from the cylinder primitive
}

|box#api|   "API"
|box#queue| "Queue" .hot { radius: 8 }    // a node wears its class after the label
|db#store|  "Postgres"

api   -> queue "enqueue"
queue -> store .loud "persist"            // a link wears one after its endpoints
store ---> api "ack"                       // dotted arrow
```

**Containers lay their children out.** Style sits in `{ }`, children in `[ ]`; pick a mode and they flow, grid, or anchor:

```
|group#services| "Services" { direction: row; gap: 24 } [
  |box#api|  "API"
  |box#auth| "Auth"
]
```

A flow orients with `direction: row` or `column`; a `grid` is sized by `columns` / `rows` and placed with `cell:` / `span:`; plus `pin` and `translate` to lift a child out of the flow.

---

## Nodes

<p align="center"><img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/shapes.png" alt="Lini's primitives and templates" width="480"></p>

```
|hex|  "hex" { width: 82; height: 72 }
|cyl|  "db"  { width: 78; height: 78 }
|poly| { points: 0 -34, 32 11, 20 34, -20 34, -32 11; }
|path| { path: "M -34 6 C -34 -34 34 -34 34 6 C 20 34 -20 34 -34 6 Z"; }
```

Block (the bare frameless rectangle), oval, hex, slant, cylinder, diamond, polygon, line, icon (a Phosphor symbol — `|icon| { symbol: heart }`, painted like a node), and image, plus `path` for anything else. Text is not a primitive: a bare `"…"` is content — styleable in place (`"x" { color: red }`) — and `|block|` is the frameless box for a label that needs an id or a link. Templates (`box`, `rect`, `group`, `caption`, `footer`, `badge`, `row`, `column`, `grid`, `table`, `sign`) bundle common patterns over a base type, and you can define your own from any base: `|panel::group| { stroke: --accent; }`.

---

## Charts

Give a node `layout: chart` and it becomes a plot, drawn from data instead of pixels. Hand it some numbers and it sorts out the scale, the ticks, the gridlines, and a colour per series, then lowers the whole thing to the same primitives as everything else — so a chart themes, bakes, and diffs exactly like the rest of a diagram.

<p align="center"><img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/charts.png" alt="Four Lini charts: grouped bars, smooth lines, a radar, and a banded area" width="680"></p>

```
|chart| "Revenue ($M)" { categories: "Q1" "Q2" "Q3" "Q4" } [
  |bars| "2023" { data: 12 19 15 25 }
  |bars| "2024" { data: 18 24 22 31 }
]
```

`|bars|`, `|line|`, `|area|`, `|dots|`, and `|bubble|` share one x/value plane; `|slice|` makes a pie or donut. `direction: radial` bends the plane into a radar and `direction: row` lays it on its side, with no change to the data. A series reads either `data:` (plain numbers, or `x y` points) or `fn:` — a formula sampled over the domain, using the language's own compile-time math. Axes auto-fit or take a `range:`, run linear or `log`, and you declare an `|axis|` only when you want to say something; shade a zone with `|band|`, drop a threshold or callout with `|mark|`. Label individual points with `tags:` and they place themselves without colliding — on the plot where they fit, on hover where they don't (`tooltip: none | hover | auto | always`); size a point for hovering with `marker: circle`. The whole chart language is in [`CHARTS.md`](https://github.com/monfa-red/lini/blob/main/CHARTS.md).

---

## Sequences

Give the scene `layout: sequence` and the diagram reads on a **time axis**: named participants line up across the top, each drops a lifeline, and the messages — ordinary links — fall top-to-bottom **in the order you write them**. No new syntax: participants are nodes, messages are links, frames and notes are nodes. Like a chart it lowers to the same primitives, so it themes, bakes, and diffs like everything else.

<p align="center"><img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/sequence.png" alt="A Lini sequence diagram: a checkout flow with activation bars, a loop frame, a self-message, and a note" width="560"></p>

```
{ layout: sequence }
|box#user| "User"
|box#api|  "API"
user -> api  "POST /login"   // a call — solid arrow, opens an activation bar
api --> user "200 + token"   // a return — dashed
|alt| "valid" [              // a branch frame; |else| "…" splits compartments
  user ~> api "log event"    // async — wavy
]
```

The operator picks the message: `->` a call, `-->` a return, `~>` async, and `a -> a` a self-message. A call opens an **activation bar** on its target and the matching return closes it — nesting stacks, automatically. Wrap a span of messages in `|loop|`, `|opt|`, or `|alt|` (with `|else|` compartments) to frame it, and a frame only groups — its messages still wire the outer participants. Drop a `|note| "…" { over: a }` (or `{ left: a }` / `{ right: a }`, or `over: a b` to span). The whole sequence language is §10 of [`SPEC.md`](https://github.com/monfa-red/lini/blob/main/SPEC.md).

---

## Icons

Built-in **[Phosphor](https://phosphoricons.com/)** icons, drawn as inline SVG paths — no icon font, no external files. `|icon| { symbol: heart }` paints like any node: `fill` is the body, `stroke` the line, `stroke-width` counter-scaled so the weight stays even at any size. `|sign|` is a larger preset and an ordinary node — it carries a label, wears a colour class, and wires up like a box.

<p align="center"><img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/icons.png" alt="Lini's built-in Phosphor icons and signs" width="520"></p>

```
|icon| .teal { symbol: user }                            // two-tone, via a colour class
|icon| { symbol: cloud; fill: none; stroke: --sky-deep } // single-tone line
|icon| "bell" .amber [ "3" ]                             // symbol via the label, "3" a badge
|sign#svc| "gear" .purple [ "Service" ]                  // larger, labelled, and linkable
```

Only the symbols a diagram uses are embedded, so the full set never bloats a small file.

---

## Links

Connect two nodes by id and Lini finds an orthogonal path through the free space, keeping a configurable `clearance` from every node and link, rounding the corners, and landing the arrowhead on the edge. One knob (`clearance`, default 16) sets spacing for the whole diagram.

The operator is the link's look, written `[start][line][end]` with no spaces:

| Line | | Markers | |
|---|---|---|---|
| `-` solid | `--` dashed | `>` arrow | `*` dot |
| `---` dotted | `~` wavy | `<` crow | `<>` diamond |

So `->` is a solid arrow, `<->` is bidirectional, `--*` a dashed line ending in a dot, `~>` a wavy arrow. Endpoints support fan-out, fan-in, and cartesian fans with `&`, and dot-paths into nested containers (`closet.outlet -> fridge.inlet`). Routing is automatic but steerable: name a side (`a:right -> b:left`) to force where a link leaves or arrives. Labels ride the link and slide to clear nodes; the link never moves for a label.

The full routing contract (crossings, priority, self-loops, starvation) lives in [`LINKING.md`](https://github.com/monfa-red/lini/blob/main/LINKING.md).

---

## Colour

**Pretty by default.** A curated palette of 11 named hues — `red rose orange amber lime green teal sky blue purple gray` — each in five job-named tiers, so the easy path is the flattering one:

<p align="center">
  <img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/palette.png" alt="Lini's 11-hue palette in five tiers, light mode" width="320">
  <img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/palette-dark.png" alt="The same palette under dark mode" width="320">
</p>
<p align="center"><em>11 hues × 5 tiers — and the same colours under dark mode, where every tier keeps its job (<code>ink</code> stays the high-contrast tone, <code>wash</code> the surface).</em></p>

```
{ |card::box| { fill: --teal-wash; stroke: --teal-ink } }   // a soft card, one line
|box#n|     { fill: --amber-soft }
|box#hero|  { fill: gradient(--rose, --amber, --sky) }      // a three-colour blend
```

- **Five tiers per hue** — `wash` (palest, for backgrounds), `soft`, the bare name (the everyday pastel), `deep` (the strong tone, for borders and strokes), and `ink` (for text and emphasis). The names hold across the dark flip: `--teal-ink` is the high-contrast tone in *both* modes, where a `light`/`dark` name would invert.
- **OKLCH under the hood**, so the ramp is perceptually even and the eleven read as a family. Pick any colour the same way — `fill: oklch(0.7, 0.14, 200)` — and conventional names still land (`--yellow` → amber, `--pink` → rose).
- **Gradients** — `gradient(--rose, --sky)` blends two hues at a flattering angle (any two look good); add stops for a multi-colour wash, `linear-gradient(135, …)` for a custom angle, or `radial-gradient(…)`. Works on **fill and stroke**.
- **Everything flips and bakes.** Hues and gradient stops are `light-dark()` variables, so a colour follows dark/light like the rest and freezes to a literal under `--bake-vars`. Only the colours a diagram uses are emitted, so a big palette never bloats a small file.

---

## Theming

**One SVG, both palettes.** Every colour is a `light-dark()` pair, so an exported SVG carries both and switches on its own — it follows the viewer's OS (`prefers-color-scheme`) with no script or `@media`, and a `data-theme="dark"`/`"light"` attribute on the SVG or any ancestor overrides it.

Defaults sit in `@layer lini.defaults`, so unlayered host CSS wins — no `!important`, no rebuild:

```css
.lini { --lini-accent: #ff6600; }   /* recolour every diagram on the page */
```

Geometry is always baked in, so a theme only ever changes colour — layout never depends on the host.

**Three built-in themes** — `light`, `dark`, and `high-contrast` — pin a single palette at export time:

```bash
lini diagram.lini --theme dark -o dark.svg            # pin the dark palette
lini diagram.lini --theme high-contrast --bake-vars   # a fixed look, inlined for resvg / email
lini theme dark > my-theme.css                        # print a theme as CSS to copy & edit
```

`lini theme NAME` prints any as a ready-to-edit `--lini-*` file; `--bake-vars` flattens it to literals for non-browser renderers (resvg, librsvg) and email. Every `lini-*` class is a stable styling hook.

The default font is a monospace stack (`ui-monospace, "SF Mono", …, monospace`): it reads crisp and keeps text sizing accurate. Swap it with `--lini-font-family` in the diagram, a theme, or the page's CSS.

---

## The CLI

```
lini [options] <input.lini>
lini fmt     [--check] [--stdout] <input.lini>
lini serve   [--port N] [--bake-vars] [PATH]
lini desugar <input.lini>
lini theme   [NAME]
```

| Flag | Meaning |
|---|---|
| `-o, --output FILE` | Output path (default: stdout). |
| `--format svg\|html` | Raw SVG (default), or wrapped in a minimal HTML page. |
| `--bake-vars` | Inline `var()` references — for resvg, librsvg, raster, email. |
| `--theme NAME\|FILE` | A built-in theme (`dark`, `high-contrast`, …), a CSS file, or a `light/dark` pair. |
| `--check` | Parse and validate only. |
| `--watch` | Recompile on every change (with `-o`). |
| `--no-warn` / `--strict` | Silence lint warnings, or promote them to errors. |

Errors are LSP-formatted (`file:line:col: error: …`) and suggest fixes: an unknown endpoint asks *did you mean `kitchen.counter.bowl`?*. `lini serve` runs a live preview at `localhost:7700` — a single file, or a folder as a [playground](#playground); `lini desugar` prints a file with its sugar expanded, for teaching and debugging.

---

## Playground

`lini serve` is also a browser playground. Point it at a folder and it lists the `.lini` files inside; pick one from the dropdown to open it in a small editor — source on the left, the diagram rendering live on the right.

```bash
lini serve samples/        # browse, edit, and render the bundled examples
lini serve                 # …or the current directory
lini serve diagram.lini    # a single file — live-reloads on every save
```

Syntax highlighting, a draggable split, and light/dark themes (it follows your system by default). `Ctrl`/`Cmd`-`S` renders the current buffer; **Save** writes it back to the file.

---

## Performance

Measured end-to-end on a modern laptop, including process startup (`--bake-vars`, output discarded):

| Diagram | Time |
|---|---|
| One node | ~1.6 ms |
| Realistic service diagram (9 nodes, 5 links) | ~2.2 ms |
| Dense scene (100 nodes, 90 routed links) | ~50 ms |

A single-pass parser, bottom-up layout, and an orthogonal router. No browser to spin up.

---

## Where Lini fits

| | Lini | Auto-layout tools* |
|---|---|---|
| Placement | **you control** (flex / grid / anchors) | automatic |
| Link routing | automatic, orthogonal — **steerable sides** | automatic |
| Visual control | **full SVG** (CSS vars + classes) | theme presets |
| Runtime | **single native binary**, written in Rust | varies (Node, browser, JVM, …) |

<sub>*the common auto-layout diagram generators that place nodes for you from a text description</sub>

Reach for Lini when you already have a layout in mind — a grid, a top-down flow, framed groups — and want it to look that way without drawing the connectors by hand.

---

## Architecture

A linear pipeline, each stage independently testable:

```
lex → parse → resolve → layout → route → render
```

Parsing is recursive-descent over an LL(1) grammar; resolve applies CSS-like specificity (inline beats class beats default) and expands user types; layout sizes bottom-up; the router solves links orthogonally against a clearance contract; render emits semantic SVG. The full language is specified in [`SPEC.md`](https://github.com/monfa-red/lini/blob/main/SPEC.md).

---

## Status

**v0.13.** The language (the box/text model in [`SPEC.md`](https://github.com/monfa-red/lini/blob/main/SPEC.md)) is stable, and the pipeline is complete and tested: links route and render, layout and theming work, charts plot from data ([`CHARTS.md`](https://github.com/monfa-red/lini/blob/main/CHARTS.md)), sequence diagrams read the wires as time ([§10](https://github.com/monfa-red/lini/blob/main/SPEC.md#10-sequences)), and the formatter and dev server ship in the same binary.

## Development

```bash
cargo test                               # full suite: unit, snapshot, linking
cargo run -- samples/hello.lini
cargo run -- serve samples/hero.lini
```

`samples/` holds a `.lini` per feature area; `tests/conformance.rs` snapshots their SVG with `insta`, and `tests/linking.rs` asserts the router's laws on every scene.

## License

MIT — see [LICENSE](https://github.com/monfa-red/lini/blob/main/LICENSE).

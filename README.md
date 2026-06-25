# Lini

**Pretty diagrams from plain text, with fine-grained control.**

[![crates.io](https://img.shields.io/crates/v/lini.svg)](https://crates.io/crates/lini)
[![docs.rs](https://img.shields.io/docsrs/lini)](https://docs.rs/lini)
[![CI](https://github.com/monfa-red/lini/actions/workflows/ci.yml/badge.svg)](https://github.com/monfa-red/lini/actions/workflows/ci.yml)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/monfa-red/lini/blob/main/LICENSE)

```
cat -> dog -> bird
```

One line is a complete diagram: three boxes, two arrows, sensible spacing. You place the boxes, Lini routes the links, and the same syntax scales up to the polished scene below.

<p align="center">
  <img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/hero.png" alt="A colourful service map rendered by Lini" width="450">
</p>

Thirty-odd lines of Lini ([`samples/hero.lini`](https://github.com/monfa-red/lini/blob/main/samples/hero.lini)).

---

## Why Lini

Most tools make you choose: **draw by hand** (precise, but tedious and hard to version) or **auto-layout everything** (fast, but you take what the algorithm gives). Lini splits the work: you keep spatial control, it automates only the links.

- **You arrange, it routes.** Place nodes with flex, grid, or anchors; Lini routes the connectors between them: orthogonal, clearance-respecting, deterministic.
- **The full range of SVG.** Sizes, anchors, strokes, shadows, rotation, opacity, raw paths: full control over the look, not a fixed house style, so a diagram can actually look good.
- **A small syntax.** Two brackets — `{ }` for style, `[ ]` for children — plus a few sigils and sensible defaults. `cat -> dog` is already a valid diagram; the whole language is small enough to learn in one sitting.
- **Any node you need.** 11 primitives and 11 templates, plus a raw `path` that accepts any SVG path string. If SVG can draw it, you can place it and link to it.
- **Fast, and one file.** A 1.5 MB native binary, one runtime dependency. No Node, JVM, or headless browser; typical diagrams compile in about 2 ms, startup included.
- **Reproducible output.** Compilation is byte-identical across runs, so renders diff cleanly and never churn in CI. 408 tests back it, including property tests on the router's laws.
- **Pretty by default.** A curated 11-hue palette in soft pastels — five OKLCH-tuned tiers each — plus angle-less gradients, all themeable and dark/light-aware. No hex codes required.
- **Dark mode, automatically.** Every colour is a `light-dark()` CSS variable, so one SVG carries both palettes and follows the viewer's light/dark OS setting on its own — or a `data-theme` toggle. Re-theme from the page with no recompile, choose a built-in (light, dark, high-contrast), or bake any palette into a standalone file.

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

**Names become boxes; `->` connects them.** Line styles and fans mix freely:

```
cat -> dog -> bird     // a chain: three boxes, two arrows
fox & owl -> mouse     // fan-in
frog ~> pond           // wavy
fish --> bowl          // dashed
```

<p align="center"><img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/flow.png" alt="Lini's link styles, in colour" width="300"></p>

**A diagram reads like a CSS file.** A `{ }` stylesheet at the top sets defaults, declares reusable classes, and extends nodes; then come the instances, then the links:

```
{                                   // the stylesheet — pure setup, draws nothing
  link: #444; clearance: 10;          // link defaults cascade to every link
  .loud { stroke: red; stroke-width: 2; }
  |db::cyl| { fill: lightyellow; }    // a new type from the cylinder primitive
}

api   |box| "API"
queue |box| .loud { radius: 8 } "Queue"   // a node wears its class after the type
store |db|  "Postgres"

api   -> queue "enqueue"
queue -> store .loud "persist"            // …and a link wears it the same way
store ..> api  "ack"                       // dotted arrow
```

**Containers lay their children out.** Style sits in `{ }`, children in `[ ]`; pick a mode and they flow, grid, or anchor:

```
services |group| { layout: row; gap: 24 } [
  |caption| "Services"
  api  |box| "API"
  auth |box| "Auth"
]
```

`row`, `column`, and `grid` (sized by `columns` / `rows`, placed with `cell:` / `span:`), plus `pin` and `translate` to lift a child out of the flow.

---

## Nodes

<p align="center"><img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/shapes.png" alt="Lini's primitives and templates" width="480"></p>

```
|hex|  { width: 82; height: 72 } "hex"
|cyl|  { width: 78; height: 78 } "db"
|poly| { points: 0 -34, 32 11, 20 34, -20 34, -32 11; }
|path| { path: "M -34 6 C -34 -34 34 -34 34 6 C 20 34 -20 34 -34 6 Z"; }
```

Block (the bare frameless rectangle), oval, hex, slant, cylinder, diamond, polygon, line, icon (a Phosphor symbol — `|icon| { symbol: heart }`, painted like a node), and image, plus `path` for anything else. Text is not a primitive: a bare `"…"` is content — styleable in place (`"x" { color: red }`) — and `|block|` is the frameless box for a label that needs an id or a link. Templates (`box`, `rect`, `group`, `caption`, `footer`, `badge`, `note`, `row`, `column`, `table`, `sign`) bundle common patterns over a base type, and you can define your own from any base: `|panel::group| { stroke: --accent; }`.

---

## Links

Connect two nodes by id and Lini finds an orthogonal path through the free space, keeping a configurable `clearance` from every node and link, rounding the corners, and landing the arrowhead on the edge. One knob (`clearance`, default 16) sets spacing for the whole diagram.

The operator is the link's look, written `[start][line][end]` with no spaces:

| Line | | Markers | |
|---|---|---|---|
| `-` solid | `--` dashed | `>` arrow | `*` dot |
| `..` dotted | `~` wavy | `<` crow | `<>` diamond |

So `->` is a solid arrow, `<->` is bidirectional, `--*` a dashed line ending in a dot, `~>` a wavy arrow. Endpoints support fan-out, fan-in, and cartesian fans with `&`, and dot-paths into nested containers (`closet.outlet -> fridge.inlet`). Routing is automatic but steerable: name a side (`a.right -> b.left`) to force where a link leaves or arrives. Labels ride the link and slide to clear nodes; the link never moves for a label.

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
note |note| { fill: --amber-soft }
hero |box|  { fill: gradient(--rose, --amber, --sky) }      // a three-colour blend
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

<sub>*the common auto-layout diagram tools (Mermaid, Graphviz, PlantUML, and the like)</sub>

Reach for Lini when you have a layout in mind (a grid, a top-down flow, framed groups) and want it to look the way you intend, without drawing the connectors by hand. By default, you arrange and Lini routes.

---

## Architecture

A linear pipeline, each stage independently testable:

```
lex → parse → resolve → layout → route → render
```

Parsing is recursive-descent over an LL(1) grammar; resolve applies CSS-like specificity (inline beats class beats default) and expands user types; layout sizes bottom-up; the router solves links orthogonally against a clearance contract; render emits semantic SVG. The full language is specified in [`SPEC.md`](https://github.com/monfa-red/lini/blob/main/SPEC.md).

---

## Status

**v0.7.** The language (the box/text model in [`SPEC.md`](https://github.com/monfa-red/lini/blob/main/SPEC.md)) is stable, and the pipeline is complete and tested: links route and render, layout and theming work, and the formatter and dev server ship in the same binary.

## Development

```bash
cargo test                               # full suite: unit, snapshot, linking
cargo run -- samples/hello.lini
cargo run -- serve samples/hero.lini
```

`samples/` holds a `.lini` per feature area; `tests/conformance.rs` snapshots their SVG with `insta`, and `tests/linking.rs` asserts the router's laws on every scene.

## License

MIT — see [LICENSE](https://github.com/monfa-red/lini/blob/main/LICENSE).

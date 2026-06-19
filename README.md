# Lini

**Pretty diagrams from plain text, with fine-grained control.**

[![crates.io](https://img.shields.io/crates/v/lini.svg)](https://crates.io/crates/lini)
[![docs.rs](https://img.shields.io/docsrs/lini)](https://docs.rs/lini)
[![CI](https://github.com/monfa-red/lini/actions/workflows/ci.yml/badge.svg)](https://github.com/monfa-red/lini/actions/workflows/ci.yml)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/monfa-red/lini/blob/main/LICENSE)

```
cat -> dog -> bird
```

One line is a complete diagram: three boxes, two arrows, sensible spacing. You place the boxes, Lini routes the wires, and the same syntax scales up to the polished scene below.

<p align="center">
  <img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/hero.png" alt="A colourful service map rendered by Lini" width="440">
</p>

Thirty-odd lines of Lini ([`assets/hero.lini`](https://github.com/monfa-red/lini/blob/main/assets/hero.lini)).

---

## Why Lini

Most tools make you choose: **draw by hand** (precise, but tedious and hard to version) or **auto-layout everything** (fast, but you take what the algorithm gives). Lini splits the work: you keep spatial control, it automates only the wires.

- **You arrange, it routes.** Place nodes with flex, grid, or anchors; Lini routes the connectors between them: orthogonal, clearance-respecting, deterministic.
- **The full range of SVG.** Sizes, anchors, strokes, shadows, rotation, opacity, raw paths: a designer's control over the look, not a fixed house style. The result can be genuinely pretty, not merely correct.
- **A genuinely small syntax.** Five sigils and sensible defaults. `cat -> dog` is a valid diagram; the whole language fits in a coffee break.
- **Any shape you need.** 12 primitives and 10 templates, plus a raw `path` that accepts any SVG path string. If SVG can draw it, you can place it and wire to it.
- **Fast, and one file.** A 1.5 MB native binary, one runtime dependency. No Node, JVM, or headless browser; typical diagrams compile in about 2 ms, startup included.
- **Reproducible output.** Compilation is byte-identical across runs, so renders diff cleanly and never churn in CI. 340 tests back it, including property tests on the router's laws.
- **Themeable like a web page.** Colours and fonts are CSS variables in an `@layer`; a host page restyles a diagram without recompiling, or you bake it into one self-contained file.

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

<p align="center"><img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/flow.png" alt="Lini's wire styles, in colour" width="420"></p>

**A diagram reads like a CSS file.** A stylesheet at the top sets defaults, declares reusable classes, and extends shapes; then come the instances, then the wires:

```
-> { stroke: #444; clearance: 10; }
.loud { stroke: red; stroke-width: 2; }
db::cyl { fill: lightyellow; }     // a new shape from the cylinder primitive

api   |box| "API"
queue |box| { radius: 8; "Queue" }     // a label rides the head; config needs a block
store |db|  "Postgres"

api   -> queue "enqueue"
queue -> store .loud "persist"
store ..> api  "ack"               // dotted arrow
```

**Containers lay their children out.** Pick a mode, and children flow, grid, or anchor:

```
services |group| {
  layout: row;  gap: 24;
  |caption| "Services"
  api  |box| "API"
  auth |box| "Auth"
}
```

`row`, `column`, and `grid` (sized by `columns` / `rows`, placed with `cell:` / `span:`), plus `pin` and `translate` to lift a child out of the flow.

---

## Shapes

<p align="center"><img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/shapes.png" alt="Eight of Lini's shape primitives" width="640"></p>

```
|hex|  { width: 82; height: 72; "hex" }
|cyl|  { width: 78; height: 78; "db" }
|poly| { points: 0 -34, 32 11, 20 34, -20 34, -32 11; }
|path| { path: "M -34 6 C -34 -34 34 -34 34 6 C 20 34 -20 34 -34 6 Z"; }
```

Box, oval, hex, slant, cylinder, diamond, cloud, polygon, line, icon (Material Symbols), and image, plus `path` for anything else. Text is not a shape: a bare `"…"` is content, and `|plain|` is a frameless box for a label that needs an id or a wire. Templates (`plain`, `rect`, `group`, `caption`, `footer`, `badge`, `note`, `row`, `column`, `table`) bundle common patterns, and you can define your own from any base: `panel::group { stroke: --accent; }`.

---

## Wires

Connect two nodes by id and Lini finds an orthogonal path through the free space, keeping a configurable `clearance` from every node and wire, rounding the corners, and landing the arrowhead on the edge. One knob (`clearance`, default 16) sets spacing for the whole diagram.

The operator is the wire's look, written `[start][line][end]` with no spaces:

| Line | | Markers | |
|---|---|---|---|
| `-` solid | `--` dashed | `>` arrow | `*` dot |
| `..` dotted | `~` wavy | `<` crow | `<>` diamond |

So `->` is a solid arrow, `<->` is bidirectional, `--*` a dashed line ending in a dot, `~>` a wavy arrow. Endpoints support fan-out, fan-in, and cartesian fans with `&`, and dot-paths into nested containers (`closet.outlet -> fridge.inlet`). Routing is automatic but steerable: name a side (`a.right -> b.left`) to force where a wire leaves or arrives. Labels ride the wire and slide to clear nodes; the wire never moves for a label.

The full routing contract (crossings, priority, self-loops, starvation) lives in [`WIRING.md`](https://github.com/monfa-red/lini/blob/main/WIRING.md).

---

## Theming

Lini's visual defaults (colours, fonts, shadow) emit as live `var(--lini-*)` references inside `@layer lini.defaults`, so **unlayered host CSS wins automatically**, with no `!important` and no rebuild:

```css
.lini { --lini-accent: #ff6600; }   /* recolour every diagram on the page */
```

Geometry is always baked in, so layout never depends on the host. For non-browser renderers (resvg, librsvg) and email, `--bake-vars` inlines every variable into a self-contained file. Every `lini-*` class is a stable styling hook.

The default font is a monospace stack (`ui-monospace, "SF Mono", …, monospace`): it reads crisp and keeps text sizing accurate. To use the host page's font instead, set `--lini-font-family: inherit` in the diagram, via `--theme`, or from the page's CSS.

---

## The CLI

```
lini [options] <input.lini>
lini fmt     [--check] [--stdout] <input.lini>
lini serve   [--port N] [--bake-vars] <input.lini>
lini desugar <input.lini>
```

| Flag | Meaning |
|---|---|
| `-o, --output FILE` | Output path (default: stdout). |
| `--format svg\|html` | Raw SVG (default), or wrapped in a minimal HTML page. |
| `--bake-vars` | Inline `var()` references — for resvg, librsvg, raster, email. |
| `--theme FILE` | A CSS file of `--lini-*` overrides. |
| `--check` | Parse and validate only. |
| `--watch` | Recompile on every change (with `-o`). |
| `--no-warn` / `--strict` | Silence lint warnings, or promote them to errors. |

Errors are LSP-formatted (`file:line:col: error: …`) and suggest fixes: an unknown endpoint asks *did you mean `kitchen.counter.bowl`?*. `lini serve` runs a live preview (default port 7700); `lini desugar` prints a file with its sugar expanded, for teaching and debugging.

---

## Performance

Measured end-to-end on a modern laptop, including process startup (`--bake-vars`, output discarded):

| Diagram | Time |
|---|---|
| One node | ~1.6 ms |
| Realistic service diagram (9 nodes, 5 wires) | ~2.2 ms |
| Dense scene (100 nodes, 90 routed wires) | ~50 ms |

A single-pass parser, bottom-up layout, and an orthogonal router: no browser to spin up, nothing to warm.

---

## Where Lini fits

| | Lini | Auto-layout tools* |
|---|---|---|
| Placement | **you control** (flex / grid / anchors) | automatic |
| Wire routing | automatic, orthogonal — **steerable sides** | automatic |
| Visual control | **full SVG** (CSS vars + classes) | theme presets |
| Runtime | **single native binary**, written in Rust | varies (Node, browser, JVM, …) |

<sub>*the common auto-layout diagram tools (Mermaid, Graphviz, PlantUML, and the like)</sub>

Reach for Lini when you have a layout in mind (a grid, a top-down flow, framed groups) and want it to look the way you intend, without drawing the connectors by hand. Placing everything for you is a non-goal: you arrange, Lini routes.

---

## Architecture

A linear pipeline, each stage independently testable:

```
lex → parse → resolve → layout → route → render
```

Parsing is recursive-descent over an LL(1) grammar; resolve applies CSS-like specificity (inline beats class beats default) and expands user shapes; layout sizes bottom-up; the router solves wires orthogonally against a clearance contract; render emits semantic SVG. The full language is specified in [`SPEC.md`](https://github.com/monfa-red/lini/blob/main/SPEC.md).

---

## Status

**v0.3.** The language (the box/text model in [`SPEC.md`](https://github.com/monfa-red/lini/blob/main/SPEC.md)) is stable, and the pipeline is complete and tested: wires route and render, layout and theming work, and the formatter and dev server ship in the same binary.

**Non-goals**, by design: automatic node *placement* (you position, Lini routes), multi-file imports, animation, and manual wire waypoints.

## Development

```bash
cargo test                               # 340 tests
cargo run -- samples/hello.lini
cargo run -- serve samples/full_example.lini
```

`samples/` holds one `.lini` per language feature; `tests/conformance.rs` snapshots their SVG with `insta`, and `tests/wiring.rs` asserts the router's laws on every scene.

## License

MIT — see [LICENSE](https://github.com/monfa-red/lini/blob/main/LICENSE).

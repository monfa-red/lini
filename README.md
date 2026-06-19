# Lini

**A small language for plain-text diagrams.** You place the boxes — Lini routes the wires — and out comes clean, themeable SVG, in milliseconds.

[![crates.io](https://img.shields.io/crates/v/lini.svg)](https://crates.io/crates/lini)
[![docs.rs](https://img.shields.io/docsrs/lini)](https://docs.rs/lini)
[![CI](https://github.com/monfa-red/lini/actions/workflows/ci.yml/badge.svg)](https://github.com/monfa-red/lini/actions/workflows/ci.yml)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/monfa-red/lini/blob/main/LICENSE)

```
cat -> dog -> bird
```

That one line is a complete diagram: three boxes, two arrows, sensible spacing. No coordinates, no XML, no mouse.

<p align="center">
  <img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/hero.png" alt="A web-service architecture diagram rendered by Lini" width="420">
</p>

…and the diagram above? Also plain text — about twenty readable lines ([`samples/full_example.lini`](https://github.com/monfa-red/lini/blob/main/samples/full_example.lini)). You wrote the structure; Lini handled the geometry, the orthogonal wire routing, and the styling.

---

## Why Lini

Most diagram tools make you pick a side: **draw by hand** (precise, but tedious and unversionable) or **auto-layout everything** (fast, but you get whatever the algorithm decides). Lini splits the difference:

- **You arrange, it routes.** Lay nodes out with flex, grid, or anchors — the parts you have an opinion about. Lini routes the connectors between them: orthogonal, clearance-respecting, deterministic. The thing you *don't* want to do by hand.
- **Genuinely small syntax.** Five sigils, sensible defaults. `cat -> dog` is a valid diagram. You can learn the whole thing in a coffee break.
- **Any shape you need.** 12 primitives and 10 templates out of the box — and a raw `path` primitive that accepts any SVG path string. If SVG can draw it, you can place it and wire to it.
- **Fast, and a single file.** A 1.5 MB native binary with one runtime dependency. No Node, no JVM, no headless browser. Typical diagrams compile in **~2 ms** — process startup included.
- **Output you can trust.** Compilation is **byte-identical across runs**, so renders diff cleanly in review and never churn in CI. 334 tests back it, including property tests that assert the router's laws on every sample.
- **Themeable like a web page.** Colors and fonts ship as CSS variables inside an `@layer`, so a host page restyles a diagram without recompiling — or bake everything to a self-contained file for email and raster renderers.

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

## A 60-second tour

**Start with nothing.** Undeclared names become boxes; `->` connects them. Mix line styles and fan-outs freely:

```
cat -> dog -> bird     // a chain: three boxes, two arrows
fox & owl -> mouse     // fan-in
frog ~> pond           // wavy
fish --> bowl          // dashed
```

<p align="center"><img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/flow.png" alt="Four lines of Lini rendered to four little flows" width="640"></p>

**Add shape, labels, and a touch of style.** Lini reads like CSS: a stylesheet at the top sets defaults, defines reusable classes, and extends shapes — then the instances, then the wires:

```
-> { stroke: #444; clearance: 10; }
.loud { stroke: red; stroke-width: 2; }
db::cyl { fill: lightyellow; }     // a new shape, based on the cylinder primitive

api   |box| "API"
queue |box| { radius: 8; "Queue" }     // a label rides the head; config needs a block
store |db|  "Postgres"

api   -> queue "enqueue"
queue -> store .loud "persist"
store ..> api  "ack"               // dotted, with an arrow
```

**Lay things out.** Containers pick a layout mode; children flow, grid, or anchor:

```
services |group| {
  layout: row;  gap: 24;
  |caption| "Services"
  api  |box| "API"
  auth |box| "Auth"
}
```

`layout: row` · `layout: column` · `layout: grid` (sized by `columns` / `rows`), with `cell:` / `span:` for grid placement and `pin` / `translate` to lift a child out of the flow.

---

## A whole vocabulary of shapes

<p align="center"><img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/shapes.png" alt="Lini's shape primitives, including a polygon and a raw SVG path" width="640"></p>

```
|hex|  { width: 82; height: 72; "hex" }
|cyl|  { width: 78; height: 78; "db" }
|poly| { points: 0 -34, 32 11, 20 34, -20 34, -32 11; }
|path| { path: "M -34 6 C -34 -34 34 -34 34 6 C 20 34 -20 34 -34 6 Z"; }
```

Box, oval, hex, slant, cylinder, diamond, cloud, polygon, line, icon (Material Symbols), image — plus `path` for anything else. Text isn't a shape: a bare `"…"` is content, and `|plain|` is a frameless box for when a label needs an id or a wire. Templates (`plain`, `rect`, `group`, `caption`, `footer`, `badge`, `note`, `row`, `column`, `table`) bundle the common patterns, and you can define your own shapes by extending any base: `panel::group { stroke: --accent; }`.

---

## Wires that route themselves

Connect any two nodes by id; Lini finds an orthogonal path through the free space, keeps a configurable `clearance` from every node and every other wire, rounds the corners, and lands the arrowhead on the edge. One knob (`clearance`, default 16) governs spacing for the whole diagram.

The operator *is* the wire's look — `[start][line][end]`, no spaces:

| Line | | Markers | |
|---|---|---|---|
| `-` solid | `--` dashed | `>` arrow | `*` dot |
| `..` dotted | `~` wavy | `<` crow | `<>` diamond |

So `->` is a solid arrow, `<->` is bidirectional, `--*` is a dashed line ending in a dot, `~>` a wavy arrow. Endpoints support fan-out, fan-in, and cartesian fans with `&`, sides with `a.right -> b.left`, and dot-paths into nested containers (`closet.outlet -> fridge.inlet`). Labels ride their wire and slide to dodge nodes — the wire never moves for them.

The full routing contract — crossings, priority, self-loops, starvation — lives in [`WIRING.md`](https://github.com/monfa-red/lini/blob/main/WIRING.md).

---

## Theming without recompiling

Visual defaults (colors, fonts, shadow) emit as live `var(--lini-*)` references wrapped in `@layer lini.defaults`, so **unlayered host CSS wins automatically** — no `!important`, no rebuild:

```css
.lini { --lini-accent: #ff6600; }   /* recolor every diagram on the page */
```

Geometry is always baked into the SVG, so layout never depends on the host. For non-browser renderers (resvg, librsvg) and email, `--bake-vars` inlines every variable into a self-contained file that renders identically anywhere. Every `lini-*` class is a stable styling hook, too.

The default font is a **monospace** stack (`ui-monospace, "SF Mono", …, monospace`) — it reads crisp, and a fixed glyph advance keeps text sizing accurate. To make an **embedded** diagram adopt the host page's font instead, set `--lini-font-family: inherit` — in the diagram (`--font-family: inherit;` at the top), via `--theme`, or from the page's own CSS.

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
| `--no-warn` / `--strict` | Silence lint warnings / promote them to errors. |

Errors are LSP-formatted (`file:line:col: error: …`) and suggest fixes — an unknown endpoint says *did you mean `kitchen.counter.bowl`?*. `lini serve` runs a live-reloading preview (default port 7700), and `lini desugar` prints a file with its sugar expanded (id-as-label, trailing labels, auto-distributed wire labels) — a teaching and debugging view.

---

## How fast?

Measured end-to-end on an Apple-silicon laptop, **including process startup** (`--bake-vars`, output discarded):

| Diagram | Time |
|---|---|
| One node | ~1.6 ms |
| Realistic service diagram (9 nodes, 5 wires) | ~2.2 ms |
| Dense scene (100 nodes, 90 routed wires) | ~50 ms |

A single-pass parser, bottom-up layout, and an orthogonal router — no browser to spin up, nothing to warm.

---

## When to reach for Lini

| | Lini | Mermaid | Graphviz | PlantUML |
|---|---|---|---|---|
| Runtime | native binary | Node / browser | native binary | JVM |
| Placement | **you control** (flex/grid/anchors) | automatic | automatic | automatic |
| Wire routing | automatic, orthogonal | automatic | automatic (splines) | automatic |
| Theming | CSS variables + classes | themes / CSS | limited | skins |

Reach for **Mermaid or Graphviz** when you want a tool to lay everything out for you and don't care exactly where things land. Reach for **Lini** when you have a layout in mind — a grid, a top-to-bottom flow, framed groups — and you just don't want to draw the connectors by hand.

---

## How it works

A clean pipeline, each stage independently testable:

```
lex → parse → resolve → layout → route → render
```

Parse is recursive-descent over an LL(1) grammar; resolve applies CSS-like specificity (inline beats class beats default) and expands user shapes; layout sizes bottom-up; the router solves wires orthogonally against a clearance contract; render emits semantic SVG. The full language is specified in [`SPEC.md`](https://github.com/monfa-red/lini/blob/main/SPEC.md) — complete enough to build a conforming engine from scratch.

---

## Status

**v0.2.** The language (the box/text model — see [`SPEC.md`](https://github.com/monfa-red/lini/blob/main/SPEC.md)) is stable, and the whole pipeline is implemented and tested — wires route and render, layout and theming are complete, and the formatter and dev server ship in the same binary.

**Non-goals**, by design: automatic node *placement* (you position; Lini routes), multi-file imports, animation, and manual wire waypoints. The syntax stays forward-compatible for all of these.

## Development

```bash
cargo test                               # 334 tests
cargo run -- samples/hello.lini
cargo run -- serve samples/full_example.lini
```

`samples/` holds one `.lini` per language feature; `tests/conformance.rs` snapshots their SVG with `insta`, and `tests/wiring.rs` asserts the router's laws on every scene.

## License

MIT — see [LICENSE](https://github.com/monfa-red/lini/blob/main/LICENSE).

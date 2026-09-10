<p align="center">
  <a href="https://lini.rs"><img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/logo/lini.svg" alt="Lini" width="256"></a>
</p>

<p align="center"><strong>From mindmap to blueprint.</strong></p>

<p align="center">One small language for every kind of figure — pretty by default, precise when it has to be.</p>

<p align="center">
  <a href="https://crates.io/crates/lini"><img src="https://img.shields.io/crates/v/lini.svg" alt="crates.io"></a>
  <a href="https://docs.rs/lini"><img src="https://img.shields.io/docsrs/lini" alt="docs.rs"></a>
  <a href="https://github.com/monfa-red/lini/actions/workflows/ci.yml"><img src="https://github.com/monfa-red/lini/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/monfa-red/lini/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="license: MIT"></a>
</p>

<h3 align="center"><a href="https://lini.rs">lini.rs</a> — the tour, the reference, the gallery, and the compiler in your browser</h3>

<p align="center">
  <a href="https://lini.rs"><img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/hero.svg" alt="A colourful service map rendered by Lini" width="440"></a>
</p>

<p align="center"><em>Thirty-odd lines of Lini — <a href="https://github.com/monfa-red/lini/blob/main/samples/hero.lini"><code>samples/hero.lini</code></a>.</em></p>

---

## What it is

A compiler: plain text in, clean themeable SVG out. You say where things go, and the parts you'd rather not do by hand — routing a wire through the gaps, measuring a dimension, picking a palette — are done for you.

- **One grammar, every family.** Flowcharts, mindmaps, org charts, tables, ER schemas, sequences, charts, engineering drawings, floor plans, and circuit schematics are all layouts over the same nodes and links. Not ten tools with ten syntaxes — one language, so theming, baking, and diffing work identically in each.
- **Themeable after export.** Every colour is a live CSS variable and a `light-dark()` pair, so one SVG follows the viewer's OS with no script and no re-render, and one line of host CSS recolours every diagram on a page.
- **Fast and deterministic.** A typical figure compiles in about 2 ms, byte-identical every run, so SVGs diff cleanly in CI.
- **No runtime.** A single native binary. No Node, no headless browser, nothing to stand up beside it.
- **Agents can write it.** [`SKILL.md`](https://github.com/monfa-red/lini/blob/main/SKILL.md) is the whole language as a working brief, and the property schema is generated from the compiler's own ledger.

## Gallery

<table>
  <tr>
    <td width="50%" align="center" valign="top">
      <sub><b>Charts</b></sub><br>
      <img width="100%" src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/charts.svg" alt="Six charts: stacked bars, grouped bars, a step-and-line plot, a banded area, a radar, and a donut">
    </td>
    <td width="50%" align="center" valign="top">
      <sub><b>ER schemas</b></sub><br>
      <img width="100%" src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/entity_hero.svg" alt="An e-commerce ER schema: six entity cards wired with crow's-foot cardinality">
    </td>
  </tr>
  <tr>
    <td width="50%" align="center" valign="top">
      <sub><b>Sequence diagrams</b></sub><br>
      <img width="100%" src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/sequence.svg" alt="A sequence diagram of a checkout flow with activation bars, a loop frame, a self-message, and an async message">
    </td>
    <td width="50%" align="center" valign="top">
      <sub><b>Icons &amp; signs</b></sub><br>
      <img width="100%" src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/icons.svg" alt="Built-in Phosphor icons in several treatments, and labelled signs wired into a flow">
    </td>
  </tr>
  <tr>
    <td width="50%" align="center" valign="top">
      <sub><b>Circuit schematics</b></sub><br>
      <img width="100%" src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/schematic_hero.svg" alt="A circuit schematic on an ISO A4 sheet: six captioned regions, an STM32 and an RS-485 transceiver with numbered pins, discretes, net labels, connectors, and a title block">
    </td>
    <td width="50%" align="center" valign="top">
      <sub><b>Engineering drawings</b></sub><br>
      <img width="100%" src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/drawing_sheet.svg" alt="An engineering drawing of a DIN 912 socket cap screw on an ISO sheet: two views sharing an axis, a thread callout, hidden lines, dimensions, and a title block">
    </td>
  </tr>
  <tr>
    <td width="50%" align="center" valign="top">
      <sub><b>Floor plans</b></sub><br>
      <img width="100%" src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/floorplan.svg" alt="A one-bedroom condo floor plan: poché walls, doors with swing arcs, windows, true-size furniture, room labels, and clear-span dimensions">
    </td>
    <td width="50%" align="center" valign="top">
      <sub><b>Mindmaps &amp; trees</b></sub><br>
      <img width="100%" src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/mindmap.svg" alt="A mindmap: a centred root with six colour-tinted branches on smooth curves">
    </td>
  </tr>
</table>

More, with their source, at [lini.rs/gallery](https://lini.rs/gallery/).

## Colour

<p align="center">
  <img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/palette.png" alt="Lini's eleven hues in five tiers — wash, soft, base, deep, ink — in light mode" width="340">
  <img src="https://raw.githubusercontent.com/monfa-red/lini/main/assets/palette-dark.png" alt="The same palette in dark mode: the tiers flip, but ink stays the high-contrast tone" width="340">
</p>

<p align="center"><em>One file, both modes — <a href="https://github.com/monfa-red/lini/blob/main/samples/palette.lini"><code>samples/palette.lini</code></a>.</em></p>

Eleven named hues — `red rose orange amber lime green teal sky blue purple gray` — each in five **job-named** tiers: `wash` for backgrounds, `soft`, the bare name, `deep` for strokes, `ink` for text. The names hold across the flip — `--teal-ink` is the high-contrast tone in *both* modes, where a `light`/`dark` name would invert — which is what lets one export serve both. OKLCH underneath, so every ramp is perceptually even, and `gradient(--rose, --sky)` blends any two at a flattering angle.

## Install

```bash
cargo install lini
```

```bash
lini diagram.lini -o diagram.svg   # compile to SVG
lini serve diagram.lini            # live-reloading preview
lini serve samples/                # browse and edit the bundled examples
```

Six lines — a type define, a container, and two links Lini routes for you:

```
{ |svc::box| { fill: --teal-wash; stroke: --teal-ink } }

|group| "Services" [ |svc#api| "API"; |svc#auth| "Auth" ]
|svc#db| "Postgres"

api  -> db "read"
auth -> db "write"
```

`{ }` is style, `[ ]` is children, and naming two nodes draws an orthogonal path between them, clear of everything in the way. The rest of the grammar is the [tour](https://lini.rs/docs/tour/language.html).

## Use it in your project

- **[mdbook-lini](https://github.com/monfa-red/mdbook-lini)** — ` ```lini ` fences in an mdBook, compiled to inline SVG at build time. On [crates.io](https://crates.io/crates/mdbook-lini).
- **[remark-lini](https://github.com/monfa-red/remark-lini)** — the same fences anywhere [remark](https://github.com/remarkjs/remark) runs: Docusaurus, Next and MDX, Gatsby, or a bare `unified()` pipeline. A toggle reveals the source that drew each figure. On [npm](https://www.npmjs.com/package/remark-lini-lang): `npm install remark-lini-lang` — npm refuses the short name as one edit from `remark-lint`.
- **[astro-lini](https://github.com/monfa-red/astro-lini)** — the Astro integration: one line in `astro.config.mjs` and every fence in the site draws. It wraps `remark-lini` and adds the front end for Astro's own Markdown processor. On [npm](https://www.npmjs.com/package/astro-lini): `npm install astro-lini`.
- **[lini-wasm](https://www.npmjs.com/package/lini-wasm)** — the compiler itself, for JavaScript. One package, two builds, so it runs in Node, Bun, Deno, a bundler or a browser. This is what `remark-lini` rides on, and what to reach for to build an integration of your own.
- **Editors** — a VS Code TextMate bundle and a Zed tree-sitter extension under [`editors/`](https://github.com/monfa-red/lini/tree/main/editors), installable from the repo. Their word lists are generated from the compiler's own tables, so a new property highlights the day it lands.
- **As a library** — `lini` is a crate as well as a binary: [docs.rs/lini](https://docs.rs/lini).

### From the community

Built and maintained by their own authors, on their own release schedules.

- **[lini-view](https://github.com/FoxMaint/lini-view)** — the Obsidian plugin: a ` ```lini ` fence draws in the note itself, live in the editor, on `lini-wasm`.

## For agents

- **[`SKILL.md`](https://github.com/monfa-red/lini/blob/main/SKILL.md)** — the language as a working brief: the write → compile → *look at the render* loop, every family, and the mistakes worth not making. Also served at [lini.rs/SKILL.md](https://lini.rs/SKILL.md).
- **[`schema/lini.schema.json`](https://github.com/monfa-red/lini/blob/main/schema/lini.schema.json)** — every primitive, template, role, and property with its owner, value shape, resolved default, inheritance channel, and a compiled example. Generated from the same ledger the compiler reads, and CI fails on drift. [`schema/reference.md`](https://github.com/monfa-red/lini/blob/main/schema/reference.md) is the compact human mirror.
- **`lini --json`** — diagnostics as JSON: stable codes, spans, and machine-applicable fixes.

## Go deeper

- [The tour](https://lini.rs/docs/tour/language.html) — fourteen pages, start to finish.
- [The reference](https://lini.rs/docs/reference/00-at-a-glance.html) — every property, split out of the spec.
- [The gallery](https://lini.rs/gallery/) — finished figures, with the source that drew them.
- [The playground](https://lini.rs/play/) — the compiler, compiled to wasm, in your browser.

In this repo: [`SPEC.md`](https://github.com/monfa-red/lini/blob/main/SPEC.md) is the language contract, frozen for 1.x — syntax, property names, defaults, diagnostic codes, and the theming surface stay compatible. [`ROUTING.md`](https://github.com/monfa-red/lini/blob/main/ROUTING.md) is the routing contract. [`samples/`](https://github.com/monfa-red/lini/tree/main/samples) holds one file per feature area, and [`RELEASING.md`](https://github.com/monfa-red/lini/blob/main/RELEASING.md) says how the pieces ship together.

## Development

```bash
cargo test                    # unit, snapshot, and routing-law suites
cargo run -- serve samples/
```

A linear pipeline, each stage independently testable: lex → parse → desugar → resolve → layout → route → render. `lini desugar` prints any file with its sugar lowered to primitives, so the engine's real input is always inspectable. Conventions are in [`AGENTS.md`](https://github.com/monfa-red/lini/blob/main/AGENTS.md).

## License

MIT — see [LICENSE](https://github.com/monfa-red/lini/blob/main/LICENSE).

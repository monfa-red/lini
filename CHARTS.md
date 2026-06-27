# Charts — a brainstorm (v0.1)

A **thinking document**, not a plan — a first exploration of how charts could work in
Lini. The **smart label** turns out to be the right backbone. Scribble on it; push
back.

Twelve real configs drive every example: `chart-cycle-composition`, `chart-shot-power`,
and the nine engineering charts (`barrel-thermal`, `booster-timeline`, `cycle-times`,
`motor-overload`, `plasticizing-duty`, `pressure-envelope`, `tcu-warmup`, `tier-power`,
`toggle`). If a proposal can't reproduce them, it's wrong. All eleven are worked in §15.

Reading order: **§1 (the smart label)** is the backbone in one screen; skim **§0
glossary**; then the design. **§15 (the real charts)**, **§16 gotchas**, and **§17 open
questions** are where your judgement matters most.

---

## Contents

0 [Glossary](#0-glossary) · 1 [The smart label](#1-the-smart-label) ·
2 [The big bet](#2-the-big-bet) · 3 [Cheat-sheet](#3-cheat-sheet) · 4 [Chart & axes](#4-chart--axes) ·
5 [Series](#5-series) · 6 [Three kinds of data](#6-three-kinds-of-data) · 7 [Formulas](#7-formulas) ·
8 [Bands & segments](#8-bands--segments) · 9 [Annotations](#9-annotations) · 10 [Areas & fills](#10-areas--fills) ·
11 [Tooltips](#11-tooltips) · 12 [Legend](#12-legend) · 13 [layout / direction](#13-the-layout--direction-refactor) ·
14 [Performance](#14-performance) · 15 [**The 11 real charts**](#15-the-11-real-charts) · 16 [Gotchas](#16-gotchas) ·
17 [Open questions](#17-open-questions) · 18 [Future: pie & mindmap](#18-future-modes)

---

## 0. Glossary

| Term | Plain meaning |
|---|---|
| **Plot area** | The inner rectangle where data is drawn — the box minus axis labels, titles, legend. |
| **Scale** | A function mapping a **data value** → a **pixel position**. Each axis owns one — **linear** or **logarithmic**. |
| **Domain / Range** | Domain = the *data* extent (`0…20 s`). Range = the *pixel* extent. We write `range:` for the data window. |
| **Axis** | The visible ruler for a scale: line, **ticks**, **tick labels**, **title**, optional **gridlines**. |
| **Series** | One dataset drawn one way — bars, a line, an area. |
| **Categorical / continuous axis** | Labels (`15 cm³ PS`) vs numbers (time, mm). Bars live categorical; lines continuous. |
| **Stacked / grouped / overlay** | Three ways bar series share a slot: piled / side-by-side / on-top semi-transparent. |
| **Interpolation** | How a line connects points: **linear**, **smooth** (spline), **step**. |
| **Area** | A line filled down to a **baseline** (axis zero, or a chosen value). |
| **Band** | A shaded (or outlined) region spanning a sub-interval of an axis. |
| **Segment / segmentation** | A piece of a series' domain with its **own formula**; lets a curve **jump** at a boundary. |
| **Annotation** | A non-data mark: a **mark** (line or point), a **band**, a free **label**. |
| **Dual / multi axis** | Two+ value axes with different ranges sharing one plot; a series picks which it reads. |
| **Smart label** | The rule that the one `"label"` after a node's head is lowered per type — text, caption, symbol… (§1). |
| **Lower** | Compiler-speak: rewrite a high-level thing into primitives. A chart **lowers** to Lini rects, paths, text. |

---

## 1. The smart label

The one `"label"` after a node's head is lowered **per type** — `|icon| "heart"` → a
symbol, `|group| "Kitchen"` → a caption, `|box| "Server"` → a text child. That single
rule carries every chart's text:

| Chart node | Its smart label becomes |
|---|---|
| `\|chart#id\|` | the chart's caption (like a group) |
| `\|axis#id\|` | the axis **title** |
| `\|bars\| \|line\| \|area\| \|scatter\|` | the series name → its **legend** entry |
| `\|band\|` | the band name → a **phase-axis** tick |
| `\|mark\|` | the annotation's **label** |
| `\|slice\|` (pie) | the slice's **label** |

One rule, every chart text — no `label:` properties, no trailing strings. And because
the label sits **right after the head**, a chart reads **name-first**:

```
|bars| "1.8 kW" { data: 9 15 24 18 30; fill: --stroke; radius: 4 }
```

Your eye hits "1.8 kW", "Motor draw", "Power (kW)" immediately.

The id lives in the bars, so binding is clean and dual / multi axis stays legible:

```
|axis#power| "Power (kW)"        { side: left;  range: 0 4.6 }
|axis#vcap|  "Booster V_cap (V)" { side: right; range: 32 49; color: --sky }
```

A series binds with `axis: vcap` — a bare id reference, the same way a link names a
node. The `toggle` chart's **four** axes (§15.9) read cleanly this way.

---

## 2. The big bet

The foundation:

> **A `|chart|` is a container that, at layout time, samples its formulas and generates
> ordinary Lini primitives — `|line|`, `|rect|`, `|path|`, `|oval|`, text — positioned
> in pixel space. The existing renderer draws them. The chart engine is a *node
> generator*, not a new SVG backend.**

Why: the renderer, theming, palette, dark/light, `--bake-vars`, and `fmt` are all
reused; formulas sample in Rust at **compile time** (the browser gets finished paths —
answers performance, §14); and the engine stays **modular like the link router** — a
clean `src/layout/chart/` with one entry point and an isolated, snapshot-testable
sampler that reuses the SPEC §11.7 expression engine.

```
src/layout/chart/
  mod.rs          orchestrator
  scale.rs        domain→range, nice + log ticks
  axis.rs         axis line + ticks + labels (+ gridlines for grid: axis)
  series_bar.rs   grouped / stacked / overlay → rects
  series_line.rs  line / area / scatter → path + dots
  bands.rs        phase partition → background bands + phase-axis + segment spans
  annot.rs        mark / band / label placed in data space (§9)
  tooltip.rs      hover hit-targets + cards (§11)
```

A chart `desugar`s to that primitive tree — the existing teaching view, for free.

---

## 3. Cheat-sheet

```
|chart#revenue| {
  bars: grouped;                // grouped | stacked | overlay
  direction: column;            // column = vertical bars; row = horizontal
  grid: val;                    // gridlines follow the `val` axis
  categories: Q1 Q2 Q3 Q4
} [
  |axis#cat| "Quarter"      { side: bottom }
  |axis#val| "Revenue ($k)" { side: left; range: 0 auto }

  |bars| "2024" { data: 3 7 5 8; fill: --teal }
  |bars| "2025" { data: 4 6 6 9; fill: --rose }
]
```

| Form | Role |
|---|---|
| `\|chart#id\|` | container = `\|block\|` + `layout: chart` + defaults (like `\|table\|`); smart label = caption. |
| `\|axis#id\| "Title"` | a scale + ruler. `side:`, `range:`, `scale:`, `unit:`. |
| `\|bars\| \|line\| \|area\| \|scatter\| "Name"` | series; smart label = legend; each **lowers** to primitives. |
| `\|band\| "Name"` | a region / the §8 phase partition. |
| `\|mark\| "Label"` | a reference line (`at: V`) or marked point (`at: X Y`) — §9. |
| `data:` | explicit values — categorical, or `x y` point pairs. |
| `fn:` | a backtick formula, or a list of them (one per band) — §7. |
| `axis: id` | bind a series / annotation to an axis. |
| `at:` / `span:` | place an annotation at a data value / over a range. |
| `grid: id` | which axis' gridlines the chart draws. |

---

## 4. Chart & axes

`|chart#id|` is a container template (`|block|` + `layout: chart` + chart defaults), like
`|table|`. Its smart label is a **caption**. Chart-level properties: `bars:`,
`direction:`, `grid:`, `categories:`, `samples:`, `legend:`.

Axes are `|axis#id|` nodes — the id binds, the smart label is the **title**:

| Property | Does |
|---|---|
| `side:` | `bottom` / `left` / `right` / `top`. Several axes can share a side — they stack outward in **source order** (deterministic). |
| `range: a b` | data window + crop. `b < a` **reverses** (`toggle`'s x runs `50 → 1`). `auto` / `a auto` auto-fit. |
| `scale: linear \| log` | linear (default) or logarithmic (`motor-overload`, `toggle`). Log auto-emits decade ticks. |
| `step:` / `ticks:` | tick spacing or explicit ticks. Omitted → nice ticks. |
| `unit: "%"` / `" °C"` | suffix appended to tick labels (and tooltips). |
| `color:` | tints the axis line, ticks, labels, title. |

A series reads one with `axis: time`. **Gridlines live on the chart** (`grid: <axis-id>`)
so dual axes don't moiré — default: the primary value axis; `grid: none` off.

**Implicit axes.** A chart with no `|axis|` gets a categorical bottom + auto-fit left, so
simple charts stay one-liners (§15.3). You declare an axis only to say something — a
title, range, side, log, colour. `toggle` declares five; a basic bar chart declares none.

---

## 5. Series

A series is a node; its smart label is its **legend entry**:

```
|line| "Motor draw" { fn: `0.5 - 0.12*u`; color: --rose }
|bars| "2025"       { data: 4 6 6 9; fill: --rose }
```

No label → no legend entry (an anonymous single series needs none).

**Kinds:** `|bars|`, `|line|`, `|area|`, `|scatter|`. Each **lowers** to primitives
(`|bars|` → rects, `|line|` → a `|line|` primitive fed sampled points, `|area|` → a
`|path|`, `|scatter|` → `|oval|` dots). `|line|` doubles as the polyline primitive —
inside a chart it takes `data:`/`fn:`, outside it takes `points:`. (A chart line *is* a
line, so the reuse is natural; the near-useless standalone `|line|` primitive could even
become `|arrow|`, freeing the name.)

**Bars: one model, three modes** — `bars: grouped | stacked | overlay` (a chart-level
knob). Plus `radius:` rounds bar corners, and `direction: row` makes bars horizontal.
`direction` is the property §13 shares across modes.

**Colour:** explicit `fill:`/`color:` wins; else series **walk the palette** in
declaration order (`--teal`, `--rose`, `--amber`, …) deterministically.

---

## 6. Three kinds of data

Data has three sources, not one.

| Source | Syntax | Used by |
|---|---|---|
| **Categorical** | `data: 9 15 24 18 30` (one per category) | every bar chart |
| **Explicit points** | `data: 0 225, 60 225, 118 221, …` (`x y` pairs) | `barrel-thermal` (measured temps) |
| **Formula** | a `fn:` backtick formula, e.g. 8/(x/100-1)^2, sampled at compile time | most line charts |

`data:` reuses Lini's value grammar (space-separated scalars; comma between point pairs).

**The formula ceiling.** `booster-timeline` is a **numeric integration** (`q +=
(prevI + iBoost)/2 * dt`) — a recurrence, not a function of `x`. **No closed-form `fn:`
can express it.** It ships as precomputed `data:` points. Don't pretend `fn:` covers
integration; chained *closed-form* derivations (`toggle`) are fine via `=` locals (§7).

---

## 7. Formulas

A chart formula is a **backtick expression** — Lini's compile-time math
([SPEC §11.7](SPEC.md)). That section owns the language (operators, `exp`/`sin`/…,
`name = expr` locals, ternary, functions); charts add two things:

- **`x` and `u`** — the formula's variable. `x` is the **x-axis value** (the domain
  position); `u` is a **per-band local** coordinate, `0 → 1` across a band (§8). A
  whole-domain formula uses `x`; a per-band one uses `u`.
- **`fn:` is a list** — one backtick formula per band, space-separated, with bare
  numbers allowed (a constant segment). A single formula (no bands) is one backtick.

```
fn: `min(8 / (x/100 - 1)^2, 2000)`       // one formula, in x
fn: `0.5 - 0.12*u`  `1.4 - 0.7*u`  0.2   // three segments; the last is a constant
```

**Reuse via a named function** (SPEC §11.7) — define once in the stylesheet, call per
series with a parameter:

```
{ ramp(s) `min(100, 25 + 1.572*(x/s) + 0.0142*(x/s)^2)`; }
…
|area| "Steel"    { fn: ramp(1) }
|line| "Aluminum" { fn: `ramp(1/0.7)` }
```

---

## 8. Bands & segments

The hard part (image 1), kept clean because **bands are ordinary nodes with smart
labels**. `chart-shot-power` has seven phases; each drives three things — a background
**band**, a **phase-axis** tick (the coloured names along the bottom), and the
**segmentation** of every series (one formula per phase, in local `u`, free to jump at a
boundary — motor draw leaps 4.5 → 0.38).

**Bands as nodes.** A band is a node whose smart label is its name:

```
|band| "Close"  { span: 0 1.4;   fill: --accent }
|band| "Inject" { span: 1.4 3.1; fill: --rose }
…
```

The chart collects its `|band|` children (in source order) as the **x-partition**. From
that one declaration come the background shading, the phase-axis ticks (each band's smart
label, tinted its `fill`), and the segment boundaries.

**A series opts in** with a per-band `fn:` list — one backtick per band, in local `u`:

```
|line| "Motor draw" {
  color: --rose; opacity: 0.8;
  fn: `0.12 + 1.2*exp(-((u-0.8)/0.12)^2)`   // Close
      `1.5 + 3.0*u^1.1`                     // Inject
      `0.5 - 0.12*u`                        // Hold
      `1.4 - 0.7*u`                         // Recovery
      `0.26 + 0.1*sin(pi*u)`                // Open
      0.12                                  // Charge   (a constant segment)
      0.2                                   // Settle
}
```

A series with a **single** `fn:` (one backtick, no list) samples the whole domain in
`x`, ignoring bands. So segmentation is **opt-in** — only a multi-formula `fn:` cares
about the partition.

- **Jumps default on** — consecutive segments connect end-to-start, drawing the riser.
- **Constants → steps for free** — "Heaters" `fn: 0.6 0 1.2 …` + connect = clean stairs.
- **`u` is a phase-local clock** (`0 → 1` across that band); `x` is the wall clock.

**The one fragility** — the `fn:` list length must equal the band count, by position.
Two mitigations: a **loud count-mismatch error**, and `fmt` annotating each formula with
its band name (the `// Close` comments above). Bands also **generalise** — set
`fill: none` and a band is a divider + label, no shading (`barrel-thermal`'s zones).

---

## 9. Annotations

Annotations are **nodes placed in data space**, each with a smart label — no annotation
subsystem, just a placement property (`at:` / `span:` + `axis:`).

| Annotation | Form | Note |
|---|---|---|
| **Mark** | `\|mark\| "label" { at: …; axis: … }` | `at: V` (one value) → a reference **line** at that level; `at: X Y` (two) → a **point** dot. Smart label rides either. |
| **Region** | `\|band\| { span: a b; axis: … }` | a shaded box over a range — the one-off cousin of §8's partition bands. |
| **Free label** | `\|block\| "…" { at: X Y }` | a text child at a data point. |

`|mark|` merges what would otherwise be two types — the placement picks the shape, the
way `pin: top` vs `pin: top left` does. A line's orientation comes from its bound axis (a
value axis → horizontal, the x-axis → vertical).

---

## 10. Areas & fills

An **area** is a line with a baseline:

```
|area| "Max indefinite duty" { fn: `1e6/(x*x)`; fill: --teal }              // to axis zero
|area| "Engineering"         { data: …; baseline: 48; fill: --accent }     // to a value
```

`|area|` = `|line|` + a fill down to **`baseline:`** (default the axis zero; `baseline:
48` fills to a level, as `booster-timeline` does). Translucency via the `fill`'s alpha or
`opacity`. A plain `|line|` never fills.

---

## 11. Tooltips

- **No JS.** Hover = CSS `:hover` revealing a hidden `<g>` card; works in inline /
  directly-opened SVG, **bakes to a clean static chart** in resvg / email / `<img>`.
- **Dots, not per-sample targets** — a curve samples ~20 pts/segment for drawing but
  emits hover **dots** only at segment boundaries + turning points (~10–20/series).
  Visible, or invisible-with-hit-radius (the configs' `pointRadius:0, pointHitRadius:6`).
- **The card** is a generated `|block|` — themeable, no special renderer; its rows reuse
  the series smart labels.
- **One limit:** a tooltip showing a *different* number than it plots (`tier-power` plots
  % but shows Watts) needs side-channel data; no-JS shows the plotted value.

---

## 12. Legend

Series smart labels **are** the legend — automatic. Bands → the phase-axis (not the
legend); annotations → neither. `legend: none` hides it (`motor-overload`,
`plasticizing`, `toggle`); a position knob (`legend: top|right|…`) later.

---

## 13. The `layout` / `direction` refactor

Worth doing *with* charts. Split **engine** from **orientation**:

| Property | Picks | Values |
|---|---|---|
| `layout:` | the **engine** | `flow` · `grid` · `chart` · `pie` · `auto` |
| `direction:` | the **orientation** | `row` · `column` · `radial` (+ engine-specific) |

`layout: row` becomes `layout: flow; direction: row`; keep `row`/`column` (and
`|row|`/`|column|`) as **shorthands** so existing diagrams survive. The win:
`direction: row` means one thing across flexbox, bars, and a future tree —
`layout: chart; direction: row` is horizontal bars, `layout: auto; direction: radial` is
a mindmap.

---

## 14. Performance

Image 1 fully drawn ≈ **100–160 SVG nodes**. resvg draws that in single-digit ms. Canvas
only wins for **thousands of points or 60fps animation** — the opposite of a static
diagram chart. Formulas sample at **compile time**, so runtime is "draw paths." The only
bloat vector is per-sample interactivity — capped by §11's turning-point dots.

---

## 15. The 11 real charts

Each notes what it **stresses**. (Long data arrays elided with `…`; structure faithful.)

### 15.1 `cycle-composition` — stacked horizontal bar (image 2)

```
|chart#cycle| {
  bars: stacked;
  direction: row;
  categories: "15 cm³ PS" "50 cm³ ABS" "50 cm³ PC"
} [
  |axis#val| "Cycle Time (s) — aluminum mold" { side: bottom }

  |bars| "Close"    { data: 1.4  1.4   1.4;  fill: --accent }
  |bars| "Inject"   { data: 0.4  1.2   1.7;  fill: --rose }
  |bars| "Hold"     { data: 1.5  2.0   2.0;  fill: --rose-ink }
  |bars| "Recovery" { data: 2.2  7.3   7.3;  fill: --amber }
  |bars| "Open"     { data: 1.1  1.1   1.1;  fill: --purple }
  |bars| "Charge"   { data: 0    0     1.4;  fill: --sky }
  |bars| "Settle"   { data: 0.4  10.9  15.3; fill: --gray }
]
```
**Stresses:** stacked + horizontal; segment names lead each line; zero segments draw
nothing. ~11 lines vs ~30.

### 15.2 `shot-power` — dual-axis segmented lines (image 1)

```
|chart#shot| { samples: 20; grid: power } [
  |axis#power| "Power (kW)"        { side: left;  range: 0 4.6 }
  |axis#vcap|  "Booster V_cap (V)" { side: right; range: 32 49; color: --sky }
  |axis#time|  "Time (s)"          { side: bottom; range: 0 20 }

  |band| "Close"    { span: 0 1.4;    fill: --accent }
  |band| "Inject"   { span: 1.4 3.1;  fill: --rose }
  |band| "Hold"     { span: 3.1 5.1;  fill: --rose-ink }
  |band| "Recovery" { span: 5.1 12.4; fill: --amber }
  |band| "Open"     { span: 12.4 13.5; fill: --purple }
  |band| "Charge"   { span: 13.5 14.9; fill: --sky }
  |band| "Settle"   { span: 14.9 20;  fill: --gray }

  |line| "Motor draw" {
    color: --rose; opacity: 0.8;
    fn: `0.12 + 1.2*exp(-((u-0.8)/0.12)^2)`  `1.5 + 3.0*u^1.1`  `0.5 - 0.12*u`
        `1.4 - 0.7*u`  `0.26 + 0.1*sin(pi*u)`  0.12  0.2
  }
  |line| "Heaters" { color: --amber; opacity: 0.8; fn: 0.6 0 1.2 0.6 1.2 0 1.2 }
  |line| "Wall draw" {
    color: --stroke; width: 3;
    fn: `0.78 + 1.22*exp(-((u-0.8)/0.12)^2)`  `2.03 - 0.33*u^1.4`  `1.66 - 0.12*u`
        `1.72 - 0.55*u`  1.6  `0.03 + 2.0*exp(-1.4*u)`  1.3
  }
  |line| "Booster V_cap" {
    color: --sky; axis: vcap; style: dashed;
    fn: 48  `48 - 8*u^1.4`  40  40  40  `48 - 8*exp(-1.4*u)`  `48 - 8*exp(-(1.4 + 2.75*u))`
  }

  |mark| "1.8 kW breaker"       { at: 1.8; axis: power; color: --stroke }
  |mark| "heater gating clears" { at: 46;  axis: vcap;  color: --sky }
  |block| "full cycle 30.2 s →" { at: 17.5 3.2; color: --muted }
]
```
**Stresses:** the keystone (bands → phase-axis → segmentation); dual axis; per-segment
`u` formulas; jumps & steps; marks + free label. ~40 lines vs ~120 — and the labels lead.

### 15.3 `cycle-times` — grouped bars (image 4)

```
|chart#cycle| {
  categories: "15 cm³ ABS" "30 cm³ ABS" "50 cm³ ABS" "50 cm³ PS" "50 cm³ PC"
} [
  |axis#val| "Cycle Time (s) — aluminum mold" { side: left }

  |bars| "1.8 kW" { data: 9 15 24 18 30; fill: --stroke; radius: 4 }
  |bars| "2.3 kW" { data: 7 13 20 14 27; fill: --amber;  radius: 4 }
  |bars| "3.6 kW" { data: 7 10 14 13 19; fill: --sky;    radius: 4 }
]
```
**Stresses:** grouped bars; rounded corners; legend-first. ~8 lines vs ~40.

### 15.4 `barrel-thermal` — multi-line, explicit points, zones (image 3)

```
|chart#barrel| { grid: temp } [
  |axis#pos|  "Position from nozzle (mm)" { side: bottom; range: 0 450 }
  |axis#temp| "Temperature (°C)"          { side: left;   range: 0 325 }

  |line| "PP"  { color: --sky;   data: 0 225, 60 225, 118 221, 180 214, 235 207,
                 300 201, 362 196, 375 88, 388 64, 447 58, 485 55 }
  |line| "ABS" { color: --amber; data: 0 250, 60 250, 118 244, 180 233, 235 222,
                 300 211, 362 201, 375 94, 388 69, 447 61, 485 57 }
  |line| "PC"  { color: --red;   data: 0 300, 60 300, 118 293, 180 282, 235 270,
                 300 261, 362 253, 375 106, 388 76, 447 67, 485 62 }

  |band| "Zone 3"       { span: 0 118;   fill: none }
  |band| "Zone 2"       { span: 118 235; fill: none }
  |band| "Zone 1"       { span: 235 362; fill: none }
  |band| "Cooling ring" { span: 362 388; fill: none }
  |band| "Feed throat"  { span: 388 450; fill: none }

  |mark| "pellets soften ≈ 130 °C" { at: 130; axis: temp; style: dashed; color: --muted }
]
```
**Stresses:** explicit `x y` points (multi-line value, ends at `}`); fill-less bands
(dividers + top ticks); a horizontal threshold.

### 15.5 `booster-timeline` — the formula ceiling + rich annotations

```
|chart#booster| { grid: vcap } [
  |axis#t|    "Time (s)"        { side: bottom; range: 0 7 }
  |axis#vcap| "Cap Voltage (V)" { side: left; range: 34 49; step: 2 }

  // numeric integration (q += …) — NOT closed-form (§6): ships as points
  |area| "Engineering — 50 cm³ at 1,000 bar" { color: --stroke; width: 3; baseline: 48; fill: --accent; data: 0 48, … }
  |line| "Commodity — 50 cm³ at 681 bar"     { color: --sky; data: 0 48, … }

  |band| { span: 34 36; axis: vcap; fill: --red }
  |mark| "36 V — driver tier floor"     { at: 36; axis: vcap; color: --red }
  |mark| "46 V — heater-gate threshold" { at: 46; axis: vcap; color: --muted }
  |mark| { at: 1.71; axis: t; style: dotted; color: --gray }
  |block| "inject ends" { at: 1.71 48.4; color: --muted }
  |mark| "V_min 38.9 V" { at: 1.71 38.9; axis: vcap; color: --stroke }
  |block| "deferred recharge\nRC · τ = 1 s" { at: 2.7 43; color: --muted }
]
```
**Stresses:** the integration ceiling (points); area-to-a-value (`baseline: 48`); region
+ line + point + multi-line label annotations — `|mark|` doing both lines (`at: V`) and
points (`at: X Y`).

### 15.6 `motor-overload` — log axis + smooth area

```
|chart#motor| { legend: none } [
  |axis#torque| "Torque (% of rated)" { side: bottom; range: 100 300; step: 50; unit: " %" }
  |axis#time|   "Max Burst Time (s)"  { side: left; scale: log; range: 1 1000 }

  |area| "Max burst time" { fn: `min(8 / (x/100 - 1)^2, 2000)`; interpolate: smooth; fill: --teal }

  |block| "300 % → 2 s"           { at: 270 5; color: --red }
  |block| "safe operating region" { at: 170 4; color: --stroke }
]
```
**Stresses:** log y (decade ticks); smooth interpolation; single `fn:` in `x`;
area-to-baseline; unit ticks; `legend: none`.

### 15.7 `plasticizing-duty` — formula + region + many point marks

```
|chart#duty| { legend: none } [
  |axis#torque| "Plasticizing Torque (% of rated)" { side: bottom; range: 100 302; step: 50; unit: " %" }
  |axis#pct|    "Max Indefinite Duty Cycle (%)"    { side: left; range: 0 105; unit: " %" }

  |area| "Max indefinite duty" { fn: `1e6 / (x*x)`; fill: --teal }

  |band| { span: 200 302; axis: torque; fill: --red }
  |mark| "200 % firmware cap" { at: 200; axis: torque; color: --red }
  |mark| "155 % sustained\n→ 42 % duty" { at: 155 41.6; color: --stroke }
  |mark| "200 % max\n→ 25 % duty"        { at: 200 25;   color: --stroke }
  |mark| "300 % bench\nblocked"          { at: 300 11.1; color: --red }
]
```
**Stresses:** scientific notation (`1e6`); region; vertical mark; multiple point marks
with labels; unit ticks.

### 15.8 `pressure-envelope` — dual axis + ternary piecewise

```
|chart#press| { grid: bar } [
  |axis#speed| "Injection Speed (mm/s)" { side: bottom; range: 0 133 }
  |axis#bar|   "Peak Pressure (bar)"    { side: left;  range: 0 1100; color: --stroke }
  |axis#flow|  "Flow (cm³/s)"           { side: right; range: 0 50;   color: --rose }

  |area| "Peak Pressure (bar)" { axis: bar;  fn: `x <= 93 ? 1000 : 1000 - 319*((x-93)/40)`; fill: --teal }
  |line| "Flow (cm³/s)"        { axis: flow; fn: `x*42/133`; color: --rose; style: dashed }

  |mark| "1,000 bar @ 93 mm/s" { at: 93; axis: speed; color: --stroke }
]
```
**Stresses:** dual axis; ternary piecewise in one backtick (no bands); area on one axis,
dashed line on the other.

### 15.9 `toggle` — four axes, log, reversed x, locals

```
|chart#toggle| { legend: none } [
  |axis#travel| "Remaining Screw Travel to Lockup (mm)" { side: bottom; range: 50 1 }   // reversed

  |axis#lockup| "Distance to Geometric Lockup (mm)" { side: left;  range: 0 10;            color: --teal }
  |axis#screw|  "Screw Force (kN)"                  { side: right; range: 0 14; step: 2;   color: --rose }
  |axis#clamp|  "Clamp Force (kN)"                  { side: right; range: 0 320; step: 40; color: --sky }
  |axis#ma|     "Mechanical Advantage"             { side: right; scale: log; range: 1 100000; color: --amber }

  |area| "Mechanical Advantage" { axis: ma; interpolate: smooth; color: --amber; fill: --amber; fn: `193800 / x^2.909` }
  |area| "Clamp Force (kN)" { axis: clamp; interpolate: smooth; color: --sky; fill: --sky;
    fn: `min(300, 366 * max(0, 0.82 - 1.32e-6*x^3.909))` }
  |area| "Screw Force (kN)" { axis: screw; interpolate: smooth; color: --rose; fill: --rose;
    fn: `ma = 193800 / x^2.909;
         platen = 1.32e-6 * x^3.909;
         clamp = min(300, 366 * max(0, 0.82 - platen));
         clamp > 0 ? min(12.6, clamp / (ma*0.95)) : 0` }
  |line| "Platen Travel to Lockup (mm)" { axis: lockup; interpolate: smooth; style: dashed; color: --teal; fn: `1.32e-6 * x^3.909` }

  |mark| "Mold touch"                 { at: 30.3; axis: travel; color: --stroke }
  |mark| "Motor 12.6 kN (30:60 belt)" { at: 12.6; axis: screw;  color: --red }
  |block| "300 kN" { at: 4 310; axis: clamp; color: --sky }
]
```
**Stresses:** four value axes (three sharing the right, source-ordered); log; reversed x
(`range: 50 1`); `=` locals in one backtick; multiple smooth areas; axis-bound marks.
The vindication of `|axis#id|` naming.

### 15.10 `tier-power` — stacked-as-increments

```
|chart#tier| {
  bars: stacked;
  direction: row;
  categories: "1.8 kW" "2.3 kW" "3.6 kW"
} [
  |axis#draw| "Wall Draw (% of breaker rating)" { side: bottom; range: 0 120; unit: " %" }

  |bars| "Cycle-average draw" { data: 78.9 67.0 58.7; fill: --teal }
  |bars| "Peak draw"          { data: 33.3 30.8 17.4; fill: --gray }   // the gap (peak − avg)

  |mark| "breaker continuous rating" { at: 100; axis: draw; color: --rose }
]
```
**Stresses:** incremental stacking (peak series is the gap, so the total lands on the
true peak, crossing 100% on tier 1); the Watts-behind-% tooltip can't be reproduced
no-JS (§11).

### 15.11 `tcu-warmup` — a named function for the twin

```
{
  ramp(s) `min(100, 25 + 1.572*(x/s) + 0.0142*(x/s)^2)`;
}

|chart#tcu| { grid: temp } [
  |axis#t|    "Time (min)"               { side: bottom; range: 0 42 }
  |axis#temp| "Supply Temperature (°C)"  { side: left; range: 20 108 }

  |area| "Steel cavity (~13 kg)"   { interpolate: smooth; fill: --teal; fn: ramp(1) }
  |line| "Aluminum cavity (~4 kg)" { interpolate: smooth; style: dashed; color: --sky; fn: `ramp(1/0.7)` }

  |mark| "100 °C max supply setpoint" { at: 100; axis: temp; color: --red }
  |mark| "60 °C — 19 min"  { at: 19 60;  color: --stroke }
  |mark| "100 °C — 36 min" { at: 36 100; color: --stroke }
  |mark| { at: 25.2 100; color: --sky }
]
```
**Stresses:** clamp via `min()`; the scaled twin via **one named function** (`ramp(1)` /
`ramp(1/0.7)` — no copy-paste); smooth; setpoint + point marks.

---

## 16. Gotchas

1. **Tick-label margin is circular** — measure tick labels first (`text::approx_width`),
   reserve the margin, then place.
2. **Nice ticks** + **log ticks** (decades 1–9, label 1-2-5) are two routines.
3. **Dual-axis grids collide** → only the chart's `grid:` axis draws gridlines.
4. **Zero-size bars** (`Charge 0 0 1.4`) — emit no rect, don't shift the stack.
5. **Horizontal bars swap axes** — `direction: row` makes the category axis vertical;
   every "which axis is categorical" check reads `direction`.
6. **Auto-domain needs a pre-pass** — `range: auto` samples every series first; bars
   force 0 into the range; **stacked** sums per category (top = sum), not max.
7. **Sampling vs straightness** — a linear `fn:` needs 2 samples, `sin` ~24. `samples:
   24` is safe; adaptive (by curvature) is a later refinement.
8. **The formula ceiling** — integration/recurrence ships as precomputed points (§6).
9. **Per-segment styling** — `booster` dashes one segment; a segmented `fn:` series can
   carry per-segment style, a points series can't easily. Decide how far to take it.
10. **Reversed axis** — `range: 50 1` reverses; scale math + tick order must honour it.
11. **`fn:` count vs bands** — a per-band `fn:` list must match the band count; loud
    error, not silent truncation (§8).
12. **Mark point + label offset** — a point `|mark|`'s label needs an auto-offset so it
    doesn't sit on the curve.
13. **Tooltip side-channel** — no-JS shows the plotted value only (§11).
14. **Draw order** — bands → grid → area → bars → line → dots → annotations → axis →
    labels → tooltip. Emit in order (or set `layer:` on generated nodes).
15. **Clipping** — data past `range:` clips to the plot area (image 1 crops at 20 s).
16. **`fmt` & formulas** — leave backtick text intact; align the `fn:` list and annotate
    each formula with its band name (§8).

---

## 17. Open questions

My lean in **bold**.

1. **Bands drive segmentation** (§8) — with a loud count error + fmt band-name comments —
   **yes**; vs per-series breakpoints (no magic, more repetition).
2. **`range:` crops** (one honest window) vs split `range:` / `clip:`.
3. **`at:` + `axis:`** vs `at-x:` / `at-y:` for marks (§9).
4. **`|area|` explicit** vs `|line| { fill: }` auto-promoting — **explicit**.
5. **The `layout`/`direction` refactor** (§13) — **do it now, with shorthands.**
6. **Free label = `|block|`** vs a dedicated `|label|` type (§9). I lean `|block|`.
7. **`samples:` default** 24, adaptive later — fine?
8. **`|line|` series reuse** + rename the primitive to `|arrow|` (§5)? I think yes.

---

## 18. Future modes

Both lean on the smart label.

**Pie (`layout: pie`)** — slices walk the palette; smart label = slice label/legend:

```
|chart#spend| { layout: pie } [
  |slice| "Ads"    { value: 40 }
  |slice| "SEO"    { value: 30 }
  |slice| "Direct" { value: 30 }
]
```

New vocabulary: `|slice|` + `value:`. Legend, palette-walk, labels all shared with §5.

**Mindmap / auto-flow (`layout: auto`)** — the engine places nodes; `[ ]` nesting is tree
structure; `direction:` picks the flavour (the §13 payoff):

```
|chart#ideas| { layout: auto; direction: radial } [
  |box#root| "Project" [
    |box#a| "Design"
    |box#b| "Build" [ |box#b1| "API"  |box#b2| "UI" ]
    |box#c| "Ship"
  ]
]
```

`radial` = mindmap fan, `row` = L-to-R tree, `column` = org chart — the same `direction`
word as bars and flexbox. If `|slice|`'s `value:` and mindmap's `direction: radial` slot
in with no new machinery, the vocabulary generalised right.

---

## 19. In one sentence

All eleven of your real charts become **8–40 lines of name-first, readable Lini** that
theme, dark/light, and bake like any other diagram — because under the hood they're
rectangles, paths, and text, and every label does the right thing for its type.

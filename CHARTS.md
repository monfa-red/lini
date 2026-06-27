# Charts — a brainstorm (v3, on v0.10 syntax)

A **thinking document**, not a plan. Rebuilt from the ground up on the new v0.10
syntax — it supersedes the v2 draft. v0.10's **smart label** turns out to be the
right backbone for charts, so this is part critique of v2, part syntax upgrade.
Scribble on it; push back.

Twelve real configs drive every example: `chart-cycle-composition`,
`chart-shot-power`, and the nine engineering charts (`barrel-thermal`,
`booster-timeline`, `cycle-times`, `motor-overload`, `plasticizing-duty`,
`pressure-envelope`, `tcu-warmup`, `tier-power`, `toggle`). If a proposal can't
reproduce them, it's wrong. All eleven are worked in §15.

Reading order: **§1 (what v0.10 changed)** is the upgrade in one screen; skim **§0
glossary**; then the design. **§15 (the real charts)**, **§16 gotchas**, and **§17
open questions** are where your judgement matters most.

---

## Contents

0 [Glossary](#0-glossary) · 1 [What v0.10 changed for charts](#1-what-v010-changed-for-charts) ·
2 [The big bet](#2-the-big-bet) · 3 [Cheat-sheet](#3-cheat-sheet) · 4 [Chart & axes](#4-chart--axes) ·
5 [Series](#5-series) · 6 [Three kinds of data](#6-three-kinds-of-data) · 7 [The formula language](#7-the-formula-language) ·
8 [Bands & segments](#8-bands--segments) · 9 [Annotations](#9-annotations) · 10 [Areas & fills](#10-areas--fills) ·
11 [Tooltips](#11-tooltips) · 12 [Legend](#12-legend) · 13 [layout / direction](#13-the-layout--direction-refactor) ·
14 [Performance](#14-performance) · 15 [**The 11 real charts**](#15-the-11-real-charts-in-v010) · 16 [Gotchas](#16-gotchas) ·
17 [Open questions](#17-open-questions) · 18 [Future: pie & mindmap](#18-future-modes) · 19 [What changed from v2](#19-what-changed-from-v2)

---

## 0. Glossary

| Term | Plain meaning |
|---|---|
| **Plot area** | The inner rectangle where data is drawn — the box minus axis labels, titles, legend. |
| **Scale** | A function mapping a **data value** → a **pixel position**. Each axis owns one — **linear** or **logarithmic**. |
| **Domain / Range** | Domain = the *data* extent (`0…20 s`). Range = the *pixel* extent. We write `range:` for the data window (what an author thinks in). |
| **Axis** | The visible ruler for a scale: line, **ticks**, **tick labels**, **title**, optional **gridlines**. |
| **Tick / nice ticks** | A marked value; "nice ticks" = the routine that picks round numbers. |
| **Series** | One dataset drawn one way — bars, a line, an area. |
| **Categorical / continuous axis** | Labels (`15 cm³ PS`) vs numbers (time, mm). Bars live on categorical; lines on continuous. |
| **Stacked / grouped / overlay** | Three ways multiple bar series share a slot: piled / side-by-side / on-top semi-transparent. |
| **Interpolation** | How a line connects points: **linear**, **smooth** (spline), **step**. |
| **Area** | A line filled down to a **baseline** (axis zero, or a chosen value). |
| **Band** | A shaded (or outlined) region spanning a sub-interval of an axis. |
| **Segment / segmentation** | A piece of a series' domain with its **own formula**; lets a curve **jump** at a boundary. |
| **Annotation** | A non-data mark: a reference **line**, a **point** dot, a **band**, a free **label**. |
| **Dual / multi axis** | Two+ value axes with different ranges sharing one plot; a series picks which it reads. |
| **Smart label** | v0.10's rule: the one `"label"` after a node's head is lowered **per type** — text, caption, symbol… (§1). |
| **Lower** | Compiler-speak: rewrite a high-level thing into primitives. A chart **lowers** to Lini rects, paths, text. |

---

## 1. What v0.10 changed for charts

The new syntax wasn't designed for charts, but two of its changes are exactly what
charts needed.

### 1.1 The smart label is the backbone

In v0.10, the **one `"label"` right after a node's head is lowered per type**. That's
already true for diagrams (`|icon| "heart"` → symbol, `|group| "Kitchen"` → caption,
`|box| "Server"` → centred text). **It generalises perfectly to chart node types:**

| Chart node | Its smart label becomes |
|---|---|
| `\|chart#id\|` | the chart's caption (like a group) |
| `\|axis#id\|` | the axis **title** |
| `\|bars\| \|line\| \|area\| \|scatter\|` | the series name → its **legend** entry |
| `\|band\|` | the band name → a **phase-axis** tick |
| `\|rule\|` | the reference line's **pill label** |
| `\|dot\|` | the marked point's **annotation** |
| `\|slice\|` (pie) | the slice's **label** |

One rule, every chart text. No `label:` properties, no trailing strings.

### 1.2 The label lands *left*, not trailing

This was v2's biggest wart. v2 had to write the legend after a long `{ }`:

```
// v2 (trailing — the label gets lost)
|bars| { data: 9 15 24 18 30; fill: --stroke; radius: 4 } "1.8 kW"

// v0.10 (smart label, right after the head — prominent)
|bars| "1.8 kW" { data: 9 15 24 18 30; fill: --stroke; radius: 4 }
```

Every series, axis, and annotation now reads **name-first**. Scanning a chart's
source, your eye hits "Motor draw", "Power (kW)", "1.8 kW breaker" immediately —
exactly the legibility you wanted.

### 1.3 Identity in the bars cleans up binding

`|axis#power|` puts the binding id where it belongs (in the bars), leaving the head
string free for the title. Dual/multi axis stops being cryptic:

```
|axis#power| "Power (kW)"        { side: left;  range: 0 4.6 }
|axis#vcap|  "Booster V_cap (V)" { side: right; range: 32 49; color: --sky }
```

A series binds with `axis: vcap` (a bare id reference in a value, the same way a link
names a node). The `toggle` chart's **four** axes (§15.9) read cleanly where
`y/y1/y2/y3` would be misery.

**Net:** v0.10 fixed the surface problems v2 was apologising for. The rest of this
doc is the model underneath, which v0.10 doesn't change — but expresses far better.

---

## 2. The big bet

Unchanged from v2, and still the foundation:

> **A `|chart|` is a container that, at layout time, samples its formulas and
> generates ordinary Lini primitives — `|line|`, `|rect|`, `|path|`, `|oval|`,
> text — positioned in pixel space. The existing renderer draws them. The chart
> engine is a *node generator*, not a new SVG backend.**

Why: the renderer, theming, palette, dark/light, `--bake-vars`, and `fmt` are all
reused; formulas sample in Rust at **compile time** (the browser gets finished paths
— answers performance, §14); and the engine stays **modular like the link router** —
a clean `src/layout/chart/` with one entry point and an isolated, snapshot-testable
`expr.rs`.

```
src/layout/chart/
  mod.rs          orchestrator
  scale.rs        domain→range, nice + log ticks
  axis.rs         axis line + ticks + labels (+ gridlines for grid: axis)
  series_bar.rs   grouped / stacked / overlay → rects
  series_line.rs  line / area / scatter → path + dots
  bands.rs        phase partition → background bands + phase-axis + segment spans
  expr.rs         STANDALONE formula language: lex → parse → eval f64  (§7)
  annot.rs        rule / dot / band / label placed in data space (§9)
  tooltip.rs      hover hit-targets + cards (§11)
```

A chart `desugar`s to that primitive tree — the existing teaching view, for free.

---

## 3. Cheat-sheet

The whole proposal on one screen (every piece argued below).

```
|chart#revenue| {
  bars: grouped                 // grouped | stacked | overlay
  direction: column             // column = vertical bars; row = horizontal
  grid: val                     // gridlines follow the `val` axis
  categories: Q1 Q2 Q3 Q4
} [
  |axis#cat| "Quarter"     { side: bottom }
  |axis#val| "Revenue ($k)" { side: left; range: 0 auto }

  |bars| "2024" { data: 3 7 5 8; fill: --teal }
  |bars| "2025" { data: 4 6 6 9; fill: --rose }
]
```

| Form | Role |
|---|---|
| `\|chart#id\|` | container = `\|block\|` + `layout: chart` + defaults (like `\|table\|`); smart label = caption. |
| `\|axis#id\| "Title"` | a scale + ruler. `side:`, `range:`, `scale:`, `unit:`. |
| `\|bars\| \|line\| \|area\| \|scatter\| "Name"` | series kinds; smart label = legend; each **lowers** to primitives. |
| `\|band\| "Name"` | a region / the §8 phase partition. |
| `\|rule\| "Label"` / `\|dot\| "Label"` | reference line / marked point (annotations, §9). |
| `data:` | explicit values — categorical, or `x y` point pairs. |
| `fn: calc(…)` | a formula (or per-segment list) for a continuous series. |
| `axis: id` | bind a series/annotation to an axis. |
| `at:` / `span:` | place an annotation at a data value / over a range. |
| `grid: id` | which axis' gridlines the chart draws. |

---

## 4. Chart & axes

### 4.1 The chart

`|chart#id|` is a container template (`|block|` + `layout: chart` + chart defaults),
like `|table|`. Its smart label is a **caption**:

```
|chart#shot| "Shot Power Profile" [ … ]    // optional caption, like a group's
```

Chart-level properties: `bars:`, `direction:`, `grid:`, `categories:`, `samples:`,
`legend:`.

### 4.2 Axes are `|axis#id|` nodes

The id (in the bars) binds; the smart label is the **title**:

```
|axis#time| "Time (s)" { side: bottom; range: 0 20 }
```

| Property | Does |
|---|---|
| `side:` | `bottom` / `left` / `right` / `top`. Several axes can share a side — they stack outward in **source order** (no `weight` needed; deterministic). |
| `range: a b` | data window + crop. `b < a` **reverses** (`toggle`'s x runs `50 → 1`). `auto` / `a auto` auto-fit. |
| `scale: linear \| log` | linear (default) or logarithmic (`motor-overload`, `toggle`). Log auto-emits decade ticks. |
| `step:` / `ticks:` | tick spacing or explicit ticks. Omitted → nice ticks. |
| `unit: "%"` / `" °C"` | suffix appended to tick labels (and tooltips). |
| `color:` | tints the axis line, ticks, labels, title (image 1's blue right axis). |

A series reads one with `axis: time`. **Gridlines live on the chart** (`grid: <axis-id>`)
so dual axes don't moiré — default: the primary value axis; `grid: none` off.

**Implicit axes.** A chart with no `|axis|` gets a categorical bottom + auto-fit
left, so simple charts stay one-liners (§15.3). You declare an axis only to *say
something* — a title, range, side, log, colour. `toggle` declares five; a basic bar
chart declares none.

---

## 5. Series

A series is a node; its smart label is its **legend entry**:

```
|line| "Motor draw" { fn: calc(…); color: --rose }
|bars| "2025"       { data: 4 6 6 9; fill: --rose }
```

No label → no legend entry (an anonymous single series needs none).

### 5.1 Kinds, and the `|line|` overlap

`|bars|`, `|line|`, `|area|`, `|scatter|`. Each **lowers** to primitives (`|bars|`
→ rects, `|line|` → a `|line|` primitive fed sampled points, `|area|` → a `|path|`,
`|scatter|` → `|oval|` dots). `|line|` doubles as the polyline primitive — inside a
chart it takes `data:`/`fn:`; outside it takes `points:`. (You okayed the reuse, and
noted the near-useless standalone `|line|` could become `|arrow|`, which frees the
name entirely.)

### 5.2 Bars: one model, three modes

```
bars: grouped     // side by side (image 4)            ← default
bars: stacked     // piled into one bar (image 2)
bars: overlay     // on top, semi-transparent
```

One chart-level knob. Plus `radius:` rounds bar corners (`cycle-times`/`tier` use it),
and `direction: row` makes bars horizontal (image 2). `direction` is the property §13
shares across modes.

### 5.3 Colour

Explicit `fill:`/`color:` wins; else series **walk the palette** in declaration order
(`--teal`, `--rose`, `--amber`, …) deterministically — a 2-series chart needs no
colours.

---

## 6. Three kinds of data

Data has three sources, not one (v2's over-assumption).

| Source | Syntax | Used by |
|---|---|---|
| **Categorical** | `data: 9 15 24 18 30` (one per category) | every bar chart |
| **Explicit points** | `data: 0 225, 60 225, 118 221, …` (`x y` pairs) | `barrel-thermal` (measured temps) |
| **Formula** | `fn: calc(8/(x/100-1)^2)` (sampled at compile time) | most line charts |

`data:` reuses Lini's value grammar exactly (space-separated scalars; comma between
point pairs, like `points:`).

### 6.1 The formula ceiling

`booster-timeline` is a **numeric integration** (`q += (prevI + iBoost)/2 * dt`) — a
recurrence, not a function of `x`. **No closed-form `fn:` can express it.** It ships
as precomputed `data:` points. Don't pretend `fn:` covers integration; chained
*closed-form* derivations (`toggle`) are fine via `let` (§7.4). One chart, one
honest fallback.

---

## 7. The formula language

The most interesting subsystem, because **bare math doesn't lex** in Lini —
`*` is a link marker, `^`/`/` throw, `+`/`-` only lex inside numbers/link-ops.

### 7.1 `calc()` confines the math (your call)

You preferred `calc()` per formula — and it resolves the lexer cleanly: **`calc(`
triggers a raw-capture** of its balanced parens, handed to the standalone `expr.rs`
which lexes operators itself. So every `*`, `^`, `=`, `;`, `1e6` lives **only inside
`calc()`**; the main lexer never sees a bare operator, and links stay safe.

```
|line| "Max burst time" { fn: calc(min(8 / (x/100 - 1)^2, 2000)); interpolate: smooth }
```

A segmented series is a **comma-list of `calc()`s** — reusing Lini's existing list
grammar — one per band (§8).

### 7.2 The language

A Pratt parser + tree-walk eval, ~200 lines:

- **Operators** `+ - * / ^` (`^` = power, right-assoc), unary `-`, comparisons
  `< <= > >= == !=`, ternary `c ? a : b`.
- **Functions** `exp ln log sqrt abs sin cos tan min max clamp floor round`, plus
  `pow(b,e)`.
- **Constants** `pi`, `e` (reserved inside expressions only — `expr.rs` is a separate
  lexer, so they stay free as ids elsewhere).
- **Variables** `u` = local `0..1` across a segment; `x` = the global domain value.
- **Scientific notation** `1e6`, `1.32e-6` (the configs use these; `expr.rs` lexes
  `e`, the main number lexer doesn't).

### 7.3 Ternary = inline piecewise

`pressure-envelope` is two pieces — no bands needed:

```
fn: calc(x <= 93 ? 1000 : 1000 - 319*((x-93)/40))
```

### 7.4 `let` for chained derivations

`toggle` computes platen → clamp → screw, each from the last. `let` keeps it readable
(`;`-separated bindings inside one `calc()`, last line is the value):

```
fn: calc(
  let ma     = 193800 / x^2.909;
  let platen = 1.32e-6 * x^3.909;
  let clamp  = min(300, 366 * max(0, 0.82 - platen));
  clamp > 0 ? min(12.6, clamp / (ma*0.95)) : 0
)
```

This is the "do we need variables?" answer: **no file-level vars, no loops — just
`let` inside a formula.**

---

## 8. Bands & segments

The hard part (image 1), now cleaner under v0.10 because **bands are ordinary nodes
with smart labels**.

### 8.1 What image 1 contains

`chart-shot-power` has seven phases, each driving three things in the JS via three
loops: a background **band**, a **phase-axis** tick (the coloured names along the
bottom), and the **segmentation** of every series (one formula per phase, in local
`u`, free to jump at boundaries — motor draw leaps 4.5 → 0.38).

### 8.2 Bands as nodes (the v0.10 upgrade)

v2 needed a bespoke `bands [ Close 0 1.4 {…} ]` mini-syntax. v0.10 doesn't — a band
is a node whose smart label is its name:

```
|band| "Close"    { span: 0 1.4;    fill: --accent }
|band| "Inject"   { span: 1.4 3.1;  fill: --rose }
|band| "Hold"     { span: 3.1 5.1;  fill: --rose-ink }
…
```

The chart collects its `|band|` children (in source order) as the **x-partition**.
From that one declaration come: the background shading, the phase-axis ticks (each
band's smart label, tinted its `fill`), and the segment boundaries.

### 8.3 A series opts into per-band formulas

`fn:` with **one `calc()` per band**, in local `u`, mapped by position:

```
|line| "Motor draw" {
  color: --rose; opacity: 0.8
  fn: calc(0.12 + 1.2*exp(-((u-0.8)/0.12)^2)),   // Close
      calc(1.5 + 3.0*u^1.1),                       // Inject
      calc(0.5 - 0.12*u),                          // Hold
      calc(1.4 - 0.7*u),                           // Recovery
      calc(0.26 + 0.1*sin(pi*u)),                  // Open
      calc(0.12),                                  // Charge
      calc(0.2)                                    // Settle
}
```

A series with a **single** `fn: calc(…)` (no commas) is sampled over the whole domain
in `x`, ignoring bands. So segmentation is **opt-in** — only multi-`calc()` series
care about the partition.

- **Jumps default on** — consecutive segments connect end-to-start, drawing the
  riser (matches the JS). `break` leaves a gap.
- **Constants → steps for free** — "Heaters" `calc(0.6), calc(0), calc(1.2), …` +
  connect = clean stairs.
- **`u` is a phase-local clock** (`0→1` across that band); `x` is the wall clock.

### 8.4 Critique: the positional coupling

The one fragility: the `fn:` list length must equal the band count, by position. Two
mitigations make it safe:

- **Loud error** on mismatch — "series 'Motor draw' has 7 segments but there are 8
  bands (added 'Cooldown')."
- **`fmt` annotates** each `calc()` line with its band's name as a trailing comment
  (shown above) — it knows the bands, so the mapping is always visible.

The alternative (each series declaring its own breakpoints) kills the coupling but
brings the JS triplication back. I'd keep bands-drive-segmentation, made safe by the
error + fmt. (§17.)

### 8.5 Bands generalise

`barrel-thermal`'s zones are the same structure with **no shading** — `|band| "Zone 3"
{ span: 0 118; fill: none }` draws just a divider + the top tick. One concept, both
the shaded phases of image 1 and the zone dividers of image 3.

---

## 9. Annotations

All four annotation kinds are **nodes placed in data space**, each with a smart label
— no annotation subsystem, just a placement property (`at:` / `span:` + `axis:`).

| Annotation | Syntax | Smart label |
|---|---|---|
| **Reference line** | `\|rule\| "1.8 kW breaker" { at: 1.8; axis: power }` | the pill label. Orientation from the bound axis (value axis → horizontal, x-axis → vertical). |
| **Marked point** | `\|dot\| "60 °C — 19 min" { at: 19 60; axis: temp }` | the label beside the dot. |
| **Region / box** | `\|band\| { span: 34 36; axis: vcap; fill: --red }` | (usually unlabelled) — the one-off cousin of §8's partition bands. |
| **Free label** | `\|block\| "full cycle 30.2 s →" { at: 17.5 3.2 }` | centred text at a data point. |

`at:` takes one value for a `|rule|`'s level, two (`x y`) for a point; `span: a b` is
a region on an axis. A free label is a `|block|` (the SPEC's "wrap text to place it"
rule). (`at:` + `axis:` vs `at-x:`/`at-y:` is a small call — §17.)

**Bonus (because charts lower to real nodes):** a `|dot#peak|` is a normal node with
an id, so you *could* draw an orthogonal Lini link from a note box to it — chart
internals wired like any diagram. Out of scope, but the door is open.

---

## 10. Areas & fills

An **area** is a line with a baseline:

```
|area| "Max indefinite duty"            { fn: calc(1e6/(x*x)); fill: --teal }   // to axis zero
|area| "Engineering — 50 cm³ at 1,000 bar" { data: …; baseline: 48; fill: --accent } // to a value
```

`|area|` = `|line|` + a fill down to **`baseline:`** (default the axis zero;
`baseline: 48` fills to a level, as `booster-timeline` does). Translucency via the
`fill`'s alpha or `opacity`. A plain `|line|` never fills. (Whether `|line| { fill: }`
should auto-promote to an area is a small call — I lean: `|area|` is explicit. §17.)

---

## 11. Tooltips

Settled, and unchanged by v0.10.

- **No JS.** Hover = CSS `:hover` revealing a hidden `<g>` card; works in inline /
  directly-opened SVG, **bakes to a clean static chart** in resvg / email / `<img>`.
- **Dots, not per-sample targets** — a curve samples ~20 pts/segment for drawing but
  emits hover **dots** only at segment boundaries + turning points (~10–20/series).
  Visible, or invisible-with-hit-radius (the configs' `pointRadius:0,
  pointHitRadius:6`).
- **The card** is a generated `|block|` — themeable, no special renderer. Its rows
  reuse the series smart labels.
- **One limit:** a tooltip showing a *different* number than it plots (`tier-power`
  plots % but shows Watts) needs side-channel data; no-JS shows the plotted value.

Only new render concern: a tiny CSS snippet when a chart has tooltips.

---

## 12. Legend

Series smart labels **are** the legend — automatic. Bands → the phase-axis (not the
legend); annotations → neither. `legend: off` hides it (`motor-overload`,
`plasticizing`, `toggle`); a position knob (`legend: top|right|…`) later.

---

## 13. The `layout` / `direction` refactor

Worth doing *with* charts (you floated it). Split **engine** from **orientation**:

| Property | Picks | Values |
|---|---|---|
| `layout:` | the **engine** | `flow` · `grid` · `chart` · `pie` · `auto` |
| `direction:` | the **orientation** | `row` · `column` · `radial` (+ engine-specific) |

`layout: row` becomes `layout: flow; direction: row`; keep `row`/`column` (and
`|row|`/`|column|`) as **shorthands** so existing diagrams survive. The win:
`direction: row` means one thing across flexbox, bars, and a future tree —
`layout: chart; direction: row` is horizontal bars, `layout: auto; direction: radial`
is a mindmap. (Breaking rename, cushioned by the shorthand — fine pre-v1.)

---

## 14. Performance

Image 1 fully drawn ≈ **100–160 SVG nodes** (7 band rects, ~6 gridlines, 4 paths,
~60 dots if tooltips on, 3 axes + ~40 ticks/labels, a few annotations, a legend).
resvg draws that in single-digit ms. Canvas only wins for **thousands of points or
60fps animation** — the opposite of a static diagram chart. Formulas sample at
**compile time**, so runtime is "draw paths." The only bloat vector is per-sample
interactivity — capped by §11's turning-point dots.

---

## 15. The 11 real charts, in v0.10

Each notes what it **stresses**. (Long data arrays elided with `…`; structure is
faithful to the config.)

### 15.1 `cycle-composition` — stacked horizontal bar (image 2)

```
|chart#cycle| {
  bars: stacked
  direction: row
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
**Stresses:** stacked + horizontal; segment names lead each line (legend = labels);
zero segments draw nothing. ~11 lines vs ~30.

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
    color: --rose; opacity: 0.8
    fn: calc(0.12 + 1.2*exp(-((u-0.8)/0.12)^2)), calc(1.5 + 3.0*u^1.1),
        calc(0.5 - 0.12*u), calc(1.4 - 0.7*u), calc(0.26 + 0.1*sin(pi*u)),
        calc(0.12), calc(0.2)
  }
  |line| "Heaters" { color: --amber; opacity: 0.8
    fn: calc(0.6), calc(0), calc(1.2), calc(0.6), calc(1.2), calc(0), calc(1.2) }
  |line| "Wall draw" { color: --stroke; width: 3
    fn: calc(0.78 + 1.22*exp(-((u-0.8)/0.12)^2)), calc(2.03 - 0.33*u^1.4),
        calc(1.66 - 0.12*u), calc(1.72 - 0.55*u), calc(1.6),
        calc(0.03 + 2.0*exp(-1.4*u)), calc(1.3) }
  |line| "Booster V_cap" { color: --sky; axis: vcap; style: dashed
    fn: calc(48), calc(48 - 8*u^1.4), calc(40), calc(40), calc(40),
        calc(48 - 8*exp(-1.4*u)), calc(48 - 8*exp(-(1.4 + 2.75*u))) }

  |rule| "1.8 kW breaker"       { at: 1.8; axis: power; color: --stroke }
  |rule| "heater gating clears" { at: 46;  axis: vcap;  color: --sky }
  |block| "full cycle 30.2 s →" { at: 17.5 3.2; color: --muted }
]
```
**Stresses:** the keystone (bands → phase-axis → segmentation); dual axis; per-segment
`u` formulas; jumps & steps; rules + free label. ~40 lines vs ~120 — and the labels
lead.

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

  |rule| "pellets soften ≈ 130 °C" { at: 130; axis: temp; style: dashed; color: --muted }
]
```
**Stresses:** explicit `x y` points; fill-less bands (dividers + top ticks); a
horizontal threshold.

### 15.5 `booster-timeline` — the formula ceiling + rich annotations

```
|chart#booster| { grid: vcap } [
  |axis#t|    "Time (s)"         { side: bottom; range: 0 7 }
  |axis#vcap| "Cap Voltage (V)"  { side: left; range: 34 49; step: 2 }

  // numeric integration (q += …) — NOT closed-form (§6.1): ships as points
  |area| "Engineering — 50 cm³ at 1,000 bar" { color: --stroke; width: 3; baseline: 48; fill: --accent; data: 0 48, … }
  |line| "Commodity — 50 cm³ at 681 bar"     { color: --sky; data: 0 48, … }

  |band| { span: 34 36; axis: vcap; fill: --red }
  |rule| "36 V — driver tier floor"     { at: 36; axis: vcap; color: --red }
  |rule| "46 V — heater-gate threshold" { at: 46; axis: vcap; color: --muted }
  |rule| { at: 1.71; axis: t; style: dotted; color: --gray }
  |block| "inject ends" { at: 1.71 48.4; color: --muted }
  |dot| "V_min 38.9 V"  { at: 1.71 38.9; axis: vcap; color: --stroke }
  |block| "deferred recharge\nRC · τ = 1 s" { at: 2.7 43; color: --muted }
]
```
**Stresses:** the integration ceiling (points); area-to-a-value (`baseline: 48`); box
+ lines + point + multi-line label annotations. Not captured: per-segment dash (one
segment dashed) on a points series — §16.

### 15.6 `motor-overload` — log axis + smooth area

```
|chart#motor| { legend: off } [
  |axis#torque| "Torque (% of rated)" { side: bottom; range: 100 300; step: 50; unit: " %" }
  |axis#time|   "Max Burst Time (s)"  { side: left; scale: log; range: 1 1000 }

  |area| "Max burst time" { fn: calc(min(8 / (x/100 - 1)^2, 2000)); interpolate: smooth; fill: --teal }

  |block| "300 % → 2 s"           { at: 270 5; color: --red }
  |block| "safe operating region" { at: 170 4; color: --stroke }
]
```
**Stresses:** log y (decade ticks); smooth interpolation; single global `fn:`;
area-to-baseline; unit ticks; `legend: off`.

### 15.7 `plasticizing-duty` — formula + box + many point markers

```
|chart#duty| { legend: off } [
  |axis#torque| "Plasticizing Torque (% of rated)" { side: bottom; range: 100 302; step: 50; unit: " %" }
  |axis#pct|    "Max Indefinite Duty Cycle (%)"    { side: left; range: 0 105; unit: " %" }

  |area| "Max indefinite duty" { fn: calc(1e6 / (x*x)); fill: --teal }

  |band| { span: 200 302; axis: torque; fill: --red }
  |rule| "200 % firmware cap" { at: 200; axis: torque; color: --red }
  |dot| "155 % sustained\n→ 42 % duty" { at: 155 41.6; color: --stroke }
  |dot| "200 % max\n→ 25 % duty"        { at: 200 25;   color: --stroke }
  |dot| "300 % bench\nblocked"          { at: 300 11.1; color: --red }
]
```
**Stresses:** scientific notation (`1e6`); box region; vertical rule; multiple
point+label markers; unit ticks.

### 15.8 `pressure-envelope` — dual axis + ternary piecewise

```
|chart#press| { grid: bar } [
  |axis#speed| "Injection Speed (mm/s)" { side: bottom; range: 0 133 }
  |axis#bar|   "Peak Pressure (bar)"    { side: left;  range: 0 1100; color: --stroke }
  |axis#flow|  "Flow (cm³/s)"           { side: right; range: 0 50;   color: --rose }

  |area| "Peak Pressure (bar)" { axis: bar;  fn: calc(x <= 93 ? 1000 : 1000 - 319*((x-93)/40)); fill: --teal }
  |line| "Flow (cm³/s)"        { axis: flow; fn: calc(x*42/133); color: --rose; style: dashed }

  |rule| "1,000 bar @ 93 mm/s" { at: 93; axis: speed; color: --stroke }
]
```
**Stresses:** dual axis; ternary piecewise (no bands); area on one axis, dashed line
on the other.

### 15.9 `toggle` — four axes, log, reversed x, `let`

```
|chart#toggle| { legend: off } [
  |axis#travel| "Remaining Screw Travel to Lockup (mm)" { side: bottom; range: 50 1 }   // reversed

  |axis#lockup| "Distance to Geometric Lockup (mm)" { side: left;  range: 0 10;            color: --teal }
  |axis#screw|  "Screw Force (kN)"                  { side: right; range: 0 14; step: 2;   color: --rose }
  |axis#clamp|  "Clamp Force (kN)"                  { side: right; range: 0 320; step: 40; color: --sky }
  |axis#ma|     "Mechanical Advantage"             { side: right; scale: log; range: 1 100000; color: --amber }

  |area| "Mechanical Advantage" { axis: ma; interpolate: smooth; color: --amber; fill: --amber; fn: calc(193800 / x^2.909) }
  |area| "Clamp Force (kN)" { axis: clamp; interpolate: smooth; color: --sky; fill: --sky;
    fn: calc(min(300, 366 * max(0, 0.82 - 1.32e-6*x^3.909))) }
  |area| "Screw Force (kN)" { axis: screw; interpolate: smooth; color: --rose; fill: --rose; fn: calc(
    let ma     = 193800 / x^2.909;
    let platen = 1.32e-6 * x^3.909;
    let clamp  = min(300, 366 * max(0, 0.82 - platen));
    clamp > 0 ? min(12.6, clamp / (ma*0.95)) : 0
  ) }
  |line| "Platen Travel to Lockup (mm)" { axis: lockup; interpolate: smooth; style: dashed; color: --teal; fn: calc(1.32e-6 * x^3.909) }

  |rule| "Mold touch"                 { at: 30.3; axis: travel; color: --stroke }
  |rule| "Motor 12.6 kN (30:60 belt)" { at: 12.6; axis: screw;  color: --red }
  |block| "300 kN" { at: 4 310; axis: clamp; color: --sky }
]
```
**Stresses:** four value axes (three sharing the right, source-ordered); log; reversed
x (`range: 50 1`); `let` bindings; multiple smooth areas; axis-bound annotations. The
vindication of `|axis#id|` naming.

### 15.10 `tier-power` — stacked-as-increments

```
|chart#tier| {
  bars: stacked
  direction: row
  categories: "1.8 kW" "2.3 kW" "3.6 kW"
} [
  |axis#draw| "Wall Draw (% of breaker rating)" { side: bottom; range: 0 120; unit: " %" }

  |bars| "Cycle-average draw" { data: 78.9 67.0 58.7; fill: --teal }
  |bars| "Peak draw"          { data: 33.3 30.8 17.4; fill: --gray }   // the gap (peak − avg)

  |rule| "breaker continuous rating" { at: 100; axis: draw; color: --rose }
]
```
**Stresses:** incremental stacking (peak series is the gap, so the total lands on the
true peak, crossing 100% on tier 1); the Watts-behind-% tooltip can't be reproduced
no-JS (§11).

### 15.11 `tcu-warmup` — clamp + scaled twin

```
|chart#tcu| { grid: temp } [
  |axis#t|    "Time (min)"              { side: bottom; range: 0 42 }
  |axis#temp| "Supply Temperature (°C)" { side: left; range: 20 108 }

  |area| "Steel cavity (~13 kg)"   { interpolate: smooth; fill: --teal;
    fn: calc(min(100, 25 + 1.572*x + 0.0142*x^2)) }
  |line| "Aluminum cavity (~4 kg)" { interpolate: smooth; style: dashed; color: --sky;
    fn: calc(min(100, 25 + 1.572*(x*0.7) + 0.0142*(x*0.7)^2)) }

  |rule| "100 °C max supply setpoint" { at: 100; axis: temp; color: --red }
  |dot| "60 °C — 19 min"  { at: 19 60;  color: --stroke }
  |dot| "100 °C — 36 min" { at: 36 100; color: --stroke }
  |dot| { at: 25.2 100; color: --sky }
]
```
**Stresses:** clamp via `min()`; the scaled twin (`x*0.7` — a `let k = 0.7` would
dedupe); smooth; setpoint + dots.

---

## 16. Gotchas

1. **Tick-label margin is circular** — measure tick labels first (`text::approx_width`
   exists), reserve the margin, then place.
2. **Nice ticks** + **log ticks** (decades 1–9, label 1-2-5) are two routines.
3. **Dual-axis grids collide** → only the chart's `grid:` axis draws gridlines.
4. **Zero-size bars** (`Charge 0 0 1.4`) — emit no rect, don't shift the stack.
5. **Horizontal bars swap axes** — `direction: row` makes the category axis vertical;
   every "which axis is categorical" check reads `direction`.
6. **Auto-domain needs a pre-pass** — `range: auto` samples every series first; bars
   force 0 into the range; **stacked** sums per category (top = sum), not max.
7. **Sampling vs straightness** — linear `fn:` needs 2 samples, `sin` ~24. `samples:
   24` is safe; adaptive (by curvature) is v2-later.
8. **The formula ceiling** — integration/recurrence ships as precomputed points (§6.1).
9. **Per-segment styling** — `booster` dashes one segment; a segmented `fn:` series can
   carry per-segment style, a points series can't easily. Decide how far to take it.
10. **Reversed axis** — `range: 50 1` reverses; scale math + tick order must honour it.
11. **`fn:` capture extent** — `calc(` raw-captures balanced parens; the comma-list of
    `calc()`s is the segment list. Bare operators outside `calc()` still error.
12. **Annotation dot + label offset** — a `|dot|`'s label needs an auto-offset so it
    doesn't sit on the curve.
13. **Tooltip side-channel** — no-JS shows the plotted value only (§11).
14. **Draw order** — bands → grid → area → bars → line → dots → annotations → axis →
    labels → tooltip. Emit in order (or set `layer:` on generated nodes).
15. **Clipping** — data past `range:` clips to the plot area (image 1 crops at 20 s).
16. **`fmt` & formulas** — leave `calc()` text intact; align the segment list and
    annotate each line with its band name (§8.4).

---

## 17. Open questions

My lean in **bold**.

1. **Bands drive segmentation** (§8.4) — with a loud count error + fmt band-name
   comments — **yes**; vs per-series breakpoints (no magic, more repetition).
2. **`|area|` explicit** vs `|line| { fill: }` auto-promoting — **explicit**.
3. **`at:` + `axis:`** vs `at-x:`/`at-y:` for annotations (§9).
4. **`range:` crops** (one honest window) vs split `range:` / `clip:`.
5. **The `layout`/`direction` refactor** (§13) — **do it now, with shorthands.**
6. **Free label = `|block|`** vs a dedicated chart `|label|` type (§9). I lean
   `|block|` (no new type), but a `|label|` reads nicer.
7. **`samples:` default** 24, adaptive later — fine?
8. **`|line|` series reuse** + rename the primitive to `|arrow|` (§5.1)? I think yes.

---

## 18. Future modes

Today's vocabulary should *fit* these. Both lean hard on the smart label.

**Pie (`layout: pie`)** — slices walk the palette; smart label = slice label/legend:

```
|chart#spend| { layout: pie } [
  |slice| "Ads"    { value: 40 }
  |slice| "SEO"    { value: 30 }
  |slice| "Direct" { value: 30 }
]
```

New vocabulary: `|slice|` + `value:`. Legend, palette-walk, labels all shared with §5.

**Mindmap / auto-flow (`layout: auto`)** — the engine places nodes; `[ ]` nesting is
tree structure; `direction:` picks the flavour (the §13 payoff):

```
|chart#ideas| { layout: auto; direction: radial } [
  |box#root| "Project" [
    |box#a| "Design"
    |box#b| "Build" [ |box#b1| "API"  |box#b2| "UI" ]
    |box#c| "Ship"
  ]
]
```

`radial` = mindmap fan, `row` = L-to-R tree, `column` = org chart — the same
`direction` word as bars and flexbox. If `|slice|`'s `value:` and mindmap's
`direction: radial` slot in with no new machinery, the vocabulary generalised right.

---

## 19. What changed from v2

The critique, made concrete — v0.10 fixed the surface, and a few ideas got cleaner:

| v2 | v3 (v0.10) | Why |
|---|---|---|
| Series label **trailed** the `{ }` (`\|bars\| { … } "1.8 kW"`) | **Smart label after the head** (`\|bars\| "1.8 kW" { … }`) | Label leads — your core ask. |
| `power \|axis\| { … } "Power (kW)"` | `\|axis#power\| "Power (kW)" { … }` | id in bars, title as the smart label. |
| Bespoke `bands [ Close 0 1.4 {…} ]` block | `\|band\| "Close" { span: 0 1.4 }` **nodes** | Bands are ordinary nodes; the partition derives from them. |
| `fn:` vs `[ ]` body was an open conflict | **`fn: calc(…)` list, settled** | `calc()` confines the math + fixes the lexer; `[ ]` stays for content. |
| Annotations described loosely | **All nodes with smart labels** (`\|rule\| "…"`, `\|dot\| "…"`) | One placement rule, one label rule. |
| `label:` property floated for axis titles | **Gone** — the smart label is the title | One mechanism. |
| Open Qs: label placement, fn capture, line reuse | **Resolved** | v0.10 + your calls closed them. |

What did **not** change: the big bet (lower to primitives), the data trichotomy, the
formula ceiling, no-JS tooltips, performance, the bands→segmentation coupling (still
the one judgement call), and pie/mindmap as future modes.

---

## 20. In one sentence

On v0.10, all eleven of your real charts become **8–40 lines of name-first, readable
Lini** that theme, dark/light, and bake like any other diagram — because under the
hood they're rectangles, paths, and text, and every label does the right thing for
its type.

Now tear into it.

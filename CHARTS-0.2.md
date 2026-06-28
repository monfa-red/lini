# Charts — design (v0.2)

A synthesis. v0.1 ([`CHARTS.md`](CHARTS.md)) proved the architecture against eleven
real engineering charts; this revision keeps that spine, **leads with the trivial
case**, **reuses Lini's real properties** instead of inventing colliding shorthands,
and **scopes a build order** around the stated focus: *x/y line / bar / dots first,
simplicity first.* The closing §20 lists every change from v0.1 and why.

Two tests govern every decision:

1. **The one-liner test.** `|chart| [ |line| { data: 3 7 4 9 6 } ]` must be a
   complete, good-looking chart — the chart equivalent of `cat -> dog -> bird`.
2. **The real-chart test.** The eleven ramjet configs must still reproduce (§17). If
   a proposal breaks either test, it's wrong.

Reading order: **§1 start simple**, **§2 the big bet**, **§3 the smart label** are
the whole idea in three screens. **§16 reuse audit**, **§18 scope**, and **§19
gotchas** are where the engineering lives.

---

## Contents

1 [Start simple](#1-start-simple) · 2 [The big bet](#2-the-big-bet) · 3 [The smart label](#3-the-smart-label) ·
4 [Cheat-sheet](#4-cheat-sheet) · 5 [Chart & axes](#5-chart--axes) · 6 [Series](#6-series) ·
7 [Data](#7-data-three-sources) · 8 [Formulas](#8-formulas) · 9 [Bands & segmentation](#9-bands--segmentation) ·
10 [Annotations](#10-annotations) · 11 [Areas & fills](#11-areas--fills) · 12 [Tooltips](#12-tooltips) ·
13 [Legend](#13-legend) · 14 [Colour](#14-colour) · 15 [`layout` / `direction`](#15-the-layout--direction-refactor) ·
16 [**Property reuse audit**](#16-property-reuse-audit) · 17 [The 11 real charts](#17-the-11-real-charts) ·
18 [**Scope & build order**](#18-scope--build-order) · 19 [Gotchas](#19-gotchas) · 20 [**What changed from v0.1**](#20-what-changed-from-v01-and-why) ·
21 [Open questions](#21-open-questions) · 22 [Future modes](#22-future-modes)

---

## 1. Start simple

A chart is a container; its children are series. Everything else is automatic until
you say otherwise — the same progressive disclosure as the rest of Lini.

```
|chart| [ |line| { data: 3 7 4 9 6 } ]          // a line chart
|chart| [ |bars| { data: 30 45 28 60 } ]         // a bar chart
|chart| [ |dots| { data: 1 2, 3 5, 4 4, 6 9 } ]  // a scatter (x y pairs)
```

Each is complete: auto domain (nicely rounded), auto ticks + tick labels, light
gridlines, a framed plot, the series in the first palette hue.

Add a title and a second series and the **legend appears for free** — series labels
*are* the legend, the chart label *is* the title (§3):

```
|chart| "Quarterly revenue" [
  |line| "2023" { data: 10 14 12 18 }
  |line| "2024" { data: 13 19 17 24 }
]
```

You reach for an axis node, a band, or a `fn:` formula **only to say something** — a
range, a unit, a log scale, a function. The eleven engineering charts in §17 are what
that looks like at full stretch; most charts never leave this section.

---

## 2. The big bet

> **A `|chart|` is a container that, at layout time, fixes a shared data→pixel scale
> from its children, samples any formulas, and generates ordinary Lini primitives —
> `|line|`, `|rect|`, `|poly|`, `|oval|`, `<text>` — in pixel space. The existing
> renderer draws them. The chart engine is a *node generator*, not a new SVG
> backend.**

Everything follows from this:

- **Theming, palette, dark/light, gradients, shadows, `--bake-vars`, `fmt`, and
  determinism are all reused** — the output is primitives the pipeline already
  handles. `desugar` prints the lowered primitive tree, so a chart is teachable and
  diffable for free.
- **Formulas sample in Rust at compile time** (SPEC §11.7's engine), so the browser
  receives finished paths — answering performance (§19) before it's asked.
- **The new mechanism is one shared scale derived from all children** — the chart's
  analogue of how `layout: grid` derives tracks from children, isolated in a
  snapshot-testable `src/layout/chart/` (§18).

This is the architecture both independent brainstorms reached; the convergence is the
strongest evidence it's right.

---

## 3. The smart label

The one `"label"` after a node's head already lowers **per type** — `|icon| "heart"`
→ a symbol, `|group| "Kitchen"` → a caption, `|box| "Server"` → centred text. That
single existing rule carries **all** chart text, so charts add no `label:` property
and no trailing-string convention:

| Chart node | Its smart label becomes |
|---|---|
| `\|chart#id\|` | the chart's **title** (a caption, like a group) |
| `\|axis#id\|` | the axis **title** |
| `\|bars\| \|line\| \|area\| \|dots\|` | the series name → its **legend** entry |
| `\|band\|` | the band name → a **phase-axis tick** (§9) |
| `\|mark\|` | the annotation's **label** |
| `\|slice\|` (pie, later) | the slice's **label** |

Because the label sits right after the head, a chart reads **name-first** — your eye
lands on "1.8 kW", "Power (kW)", "Time (s)" immediately:

```
|bars| "1.8 kW" { data: 9 15 24 18 30; fill: --sky }
```

The id lives in the bars, so axis binding stays clean even with four axes (§17.9):

```
|axis#power| "Power (kW)"        { side: left;  range: 0 4.6 }
|axis#vcap|  "Booster V_cap (V)" { side: right; range: 32 49; stroke: --sky-deep }
```

A series binds to one with `axis: vcap` — a bare id reference, exactly the way a link
names a node.

---

## 4. Cheat-sheet

Minimal first, then everything:

```
|chart| [ |bars| { data: 30 45 28 60 } ]        // the whole language, one line
```

```
|chart#revenue| {
  bars: grouped;              // grouped | stacked | overlay
  direction: column;         // column = vertical bars; row = horizontal (§15)
  grid: val;                 // gridlines follow the `val` axis (avoids dual-axis moiré)
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
| `\|chart#id\|` | container = `\|block\|` + `layout: chart` + defaults (like `\|table\|`); smart label = title. |
| `\|axis#id\| "Title"` | a scale + ruler — `side:`, `range:`, `scale:`, `step:`/`ticks:`, `unit:`. |
| `\|bars\| \|line\| \|area\| \|dots\| "Name"` | a series; smart label = legend; each **lowers** to primitives. |
| `\|band\| "Name"` | a region, or the §9 phase partition. |
| `\|mark\| "Label"` | a reference line (`at: V`) or a marked point (`at: X Y`) — §10. |
| `data:` | explicit values (one per category) or `x y` point pairs. |
| `fn:` | a backtick formula in `x`, or a per-band list in `u` (§8–9). |
| `axis: id` | bind a series / annotation to an axis. |
| `at:` / `span:` | place an annotation at a value / over a range. |
| `grid: id` / `none` | which axis' gridlines the chart draws. |

---

## 5. Chart & axes

`|chart#id|` is a container template (`|block|` + `layout: chart` + chart defaults),
like `|table|`. Chart-level properties: `bars:`, `direction:`, `grid:`,
`categories:`, `samples:`, `legend:`, `tooltip:`.

Axes are `|axis#id|` nodes — the id binds, the smart label is the **title**:

| Property | Does |
|---|---|
| `side:` | `bottom` / `left` / `right` / `top`. Several axes may share a side — they stack outward in **source order** (deterministic; `toggle` has three on the right). |
| `range: a b` | data window **and** crop. `b < a` **reverses** (`toggle`'s x runs `50 → 1`). `auto` or `a auto` auto-fits an end. |
| `scale: linear \| log` | linear (default) or logarithmic. `log` auto-emits decade ticks labelled 1-2-5. |
| `step:` / `ticks:` | tick spacing, or explicit ticks (`ticks: 0 250 500`). Omitted → nice ticks. |
| `unit: "%"` | a suffix appended to tick labels (and tooltips). |
| `stroke:` / `color:` | `stroke` tints the axis line + ticks; `color` tints the tick labels + title (Lini's real roles — §16). |

A series reads an axis with `axis: <id>` (default: the first value axis on its
orientation). **Gridlines live on the chart** (`grid: <axis-id>`), so dual axes never
moiré; `grid: none` turns them off.

**Implicit axes.** A chart with no `|axis|` gets a categorical bottom + an auto-fit
value axis, so §1's one-liners need no axis node. You declare an axis only to add a
title, range, side, log, unit, or colour. `toggle` declares five; a basic bar chart
declares none.

---

## 6. Series

A series is a node; its smart label is its **legend entry** (none → no entry).

**Kinds**, each lowering to primitives:

| Series | Paints | Lowers to | Paint (Lini roles) |
|---|---|---|---|
| `\|line\|` | a polyline through the data | a `\|line\|` primitive fed sampled points | `stroke`, `stroke-width`, `stroke-style` |
| `\|area\|` | a line filled to a baseline | a `\|poly\|` + `\|line\|` | `fill` (body), `stroke` (edge), `baseline` |
| `\|bars\|` | one bar per datum | one `\|rect\|` per datum | `fill`, `stroke`, `radius`, `width` (bar thickness) |
| `\|dots\|` | one marker per datum | one `\|oval\|`/marker per datum | `fill`, `stroke`, `marker`, `size` |

`|line|` **doubles as the polyline primitive**: outside a chart it takes `points:`
(pixels); inside a chart it takes `data:`/`fn:` (data space). This is the same
"context decides" rule that lets `.hot` be a stylesheet *rule* and a worn *class* on
the canvas — no rename, no new type. (A chart line *is* a line.)

**Markers on a line reuse the existing marker vocabulary.** `|line| { marker: dot }`
puts a dot at every datum (the end-only `marker:` generalised to every vertex);
`|dots|` is markers with no connecting line. An invisible-but-hoverable point (the
configs' `pointRadius: 0, pointHitRadius: 6`) is `|dots| { size: 0 }`, still carrying
its `<title>` (§12).

**Bars: one model, three modes** — `bars: grouped | stacked | overlay` (a
chart-level knob). `radius:` rounds corners; `direction: row` lays bars horizontal
(§15). Grouped is the default for multiple `|bars|`.

---

## 7. Data — three sources

Data has three sources, all on the **existing value grammar** (space within a group,
comma between groups — SPEC §16); no new value form is needed.

| Source | Syntax | Used by |
|---|---|---|
| **Categorical** | `data: 9 15 24 18 30` (one value per category) | every bar chart |
| **Explicit points** | `data: 0 225, 60 225, 118 221, …` (`x y` pairs) | `barrel-thermal` (measured temps) |
| **Formula** | `fn: ` `` `…x…` `` — a backtick, sampled at compile time | most line charts |

`data:` is the explicit channel; `fn:` is the computed channel (§8). They are
separate because a `fn:` can be a **list** — one formula per band — which `data:`
cannot (§9), and because reading `data:` vs `fn:` tells you at a glance whether the
points are measured or derived.

**The formula ceiling (be honest).** `booster-timeline` is a *numeric integration*
(`q += (prevI + iBoost)/2 · dt`) — a recurrence, not a closed-form function of `x`.
**No `fn:` can express it**; it ships as precomputed `data:` points. Chained
*closed-form* derivations (`toggle`) are fine — they use `=` locals (§8). `fn:`
covers functions, not integration.

---

## 8. Formulas

A chart formula is a **backtick expression** — Lini's compile-time math (SPEC §11.7
owns the language: operators, `exp`/`sin`/`min`/…, `name = expr` locals, ternary,
user functions). Charts bind two ambient variables into that engine — the same seam
that injects `u` for parametric `points:` today (`scene.rs` samples by inserting the
variable into the eval environment):

- **`x`** — the **x-axis data value** (the domain position). A whole-domain formula
  uses `x`.
- **`u`** — a **band-local** clock, `0 → 1` across one band (§9). A per-band formula
  uses `u`.

```
fn: `min(8 / (x/100 - 1)^2, 2000)`     // one formula, in x, sampled across the domain
```

**Locals** chain derivations in one backtick (the `toggle` screw-force curve):

```
fn: `ma     = 193800 / x^2.909;
     platen = 1.32e-6 * x^3.909;
     clamp  = min(300, 366 * max(0, 0.82 - platen));
     clamp > 0 ? min(12.6, clamp / (ma*0.95)) : 0`
```

**Named functions** (SPEC §11.7) keep twins DRY — define once, call per series:

```
{ ramp(s) `min(100, 25 + 1.572*(x/s) + 0.0142*(x/s)^2)`; }
…
|area| "Steel"    { fn: ramp(1) }
|line| "Aluminum" { fn: `ramp(1/0.7)` }
```

`samples:` sets the sample count (default 24 — safe for `sin`; a linear `fn:` needs
only 2; curvature-adaptive sampling is a later refinement, §19).

---

## 9. Bands & segmentation

The hard part — `shot-power`'s seven phases (§17.2) — stays clean because **bands are
ordinary nodes with smart labels**. Each band drives three things from one
declaration: a background **shade**, a **phase-axis tick** (its label, tinted its
fill), and the **segment boundary** for every series.

```
|band| "Close"  { span: 0 1.4;   fill: --accent }
|band| "Inject" { span: 1.4 3.1; fill: --rose }
…
```

The chart collects its `|band|` children, in source order, as the x-partition.

**A series opts into segmentation** with a per-band `fn:` **list** — one backtick (or
a bare constant) per band, in local `u`:

```
|line| "Motor draw" {
  stroke: --rose-deep; opacity: 0.8;
  fn: `0.12 + 1.2*exp(-((u-0.8)/0.12)^2)`   // Close
      `1.5 + 3.0*u^1.1`                      // Inject
      `0.5 - 0.12*u`                         // Hold
      `1.4 - 0.7*u`                          // Recovery
      `0.26 + 0.1*sin(pi*u)`                 // Open
      0.12                                   // Charge   (a constant segment)
      0.2                                    // Settle
}
```

- A **single** `fn:` (one backtick) samples the whole domain in `x`, ignoring bands —
  so segmentation is **opt-in**: only a multi-formula `fn:` cares about the partition.
- **Jumps default on** — consecutive segments connect end-to-start, drawing the riser
  (motor draw leaps 4.5 → 0.38 cleanly).
- **Constants → free steps** — `fn: 0.6 0 1.2 …` + connect = clean stairs (Heaters).
- **Fill-less bands generalise** — `fill: none` makes a band a divider + label, no
  shading (`barrel-thermal`'s zones, §17.4).

**The one fragility** — the `fn:` list length must equal the band count, by position.
Two guards: a **loud count-mismatch error** (never silent truncation), and `fmt`
annotating each formula with its band name (the `// Close` comments above).

This is feasible on today's grammar with no extension: a space-separated group of
backticks/numbers (`fn: \`a\` \`b\` 0.2`) parses as one value group; a lone backtick
parses as a scalar. Scalar vs group is exactly the whole-domain vs segmented switch.

---

## 10. Annotations

Annotations are **nodes placed in data space**, each with a smart label — no
annotation subsystem, just a placement property. This falls out of the big bet for
free (children already paint the shared plane in data coordinates):

| Annotation | Form | Note |
|---|---|---|
| **Mark** | `\|mark\| "label" { at: …; axis: … }` | `at: V` → a reference **line** at that level; `at: X Y` → a **point** dot. Placement picks the shape, the way `pin: top` vs `pin: top left` does. |
| **Region** | `\|band\| { span: a b; axis: … }` | a shaded box over a range — the one-off cousin of §9's partition bands. |
| **Free label** | `\|block\| "…" { at: X Y }` | a text child at a data point. |

A `|mark|` line's orientation comes from its bound axis (a value axis → horizontal;
the x-axis → vertical). chart.js needs an annotation plugin for all of this; Lini gets
it from the coordinate model it already has.

---

## 11. Areas & fills

An **area** is a line with a baseline:

```
|area| "Max duty"     { fn: `1e6/(x*x)`; fill: --teal }                 // to axis zero
|area| "Engineering"  { data: …; baseline: 48; fill: --accent }        // to a value
```

`|area|` = `|line|` + a fill down to **`baseline:`** (default the axis zero;
`baseline: 48` fills to a level, as `booster-timeline` does). Translucency is the
`fill`'s alpha or `opacity`. A plain `|line|` never fills — areas are explicit, not a
`|line| { fill: }` auto-promotion (clearer intent).

---

## 12. Tooltips

- **No JS.** Hover = a CSS `:hover` rule revealing a hidden `<g>` card; it works in
  inline / directly-opened SVG and **bakes to a clean static chart** in resvg / email
  / `<img>` (the `:hover` simply does nothing there).
- **A baked-safe floor.** Every hit-target also carries a native `<title>` (the
  existing `title:` path), so even a baked SVG and a screen reader get the value.
- **Dots, not per-sample targets.** A curve samples ~24 pts/segment for *drawing* but
  emits hover **dots** only at segment boundaries + turning points (~10–20/series) —
  visible, or invisible-with-hit-radius (`size: 0`). This caps node count (§19).
- **The card** is a generated `|block|` — themeable, no special renderer; its rows
  reuse the series smart labels.
- **One honest limit.** A tooltip that shows a *different* number than it plots
  (`tier-power` plots % but labels Watts) needs side-channel data; no-JS shows the
  plotted value. `tooltip: rich | title | none` selects the card, the native title
  only, or nothing.

---

## 13. Legend

Series smart labels **are** the legend — automatic, with a swatch in the series'
colour. Bands feed the phase-axis (not the legend); annotations feed neither.
`legend: none` hides it (`motor-overload`, `plasticizing`, `toggle`); `legend:
top | right | bottom` positions it.

---

## 14. Colour

Explicit `stroke:`/`fill:` wins. Otherwise series **walk the palette** in declaration
order — `--teal`, `--rose`, `--amber`, `--sky`, … — deterministically, at the tier
their role wants: a line takes the hue's `deep` stroke, an area/bar takes the base
fill with a `deep` edge, dots take `ink`. The legend swatch and tooltip accent follow
the series' dominant paint. This is the one delightful new behaviour, and it keeps the
easy path flattering, exactly as the palette does for diagrams (no hex to pick).

---

## 15. The `layout` / `direction` refactor

Worth doing *with* charts: split the **engine** from the **orientation**.

| Property | Picks | Values |
|---|---|---|
| `layout:` | the **engine** | `flow` · `grid` · `chart` · `pie` · `auto` |
| `direction:` | the **orientation** | `row` · `column` · `radial` (+ engine-specific) |

`layout: row` becomes `layout: flow; direction: row`, with `row`/`column` (and
`|row|`/`|column|`) kept as **shorthands** so every existing diagram survives. The
payoff is one orientation word across modes: `layout: chart; direction: row` is
horizontal bars; `layout: auto; direction: radial` is a mindmap; `direction: column`
is a tidy tree. (Lini is pre-release, so the rename is allowed — and this is exactly
the "reuse a property for more than diagrams" the brief asked for.)

---

## 16. Property reuse audit

The brief asked to reuse/rename Lini's style properties rather than invent chart-only
ones. The genuinely-new surface is small; everything else is existing Lini, used with
its **real** meaning (this is also where v0.2 corrects v0.1 — see §20):

| Chart need | Mechanism | Status |
|---|---|---|
| line / area-edge / bar-edge colour | **`stroke`** | reuse (a `\|line\|` primitive's colour *is* `stroke`) |
| area / bar / dot body fill | **`fill`** | reuse |
| line / edge thickness | **`stroke-width`** | reuse (not `width:` — that's a dimension) |
| dashed / dotted series | **`stroke-style: dashed`** | reuse (not a bare `style:`) |
| tick-label / title colour | **`color`** | reuse (text role) |
| marker at each datum | **`marker: dot/diamond/crow`**, `size` | generalise end-only marker |
| series name → legend | **smart label** | reuse |
| chart title | **smart label** on the chart | reuse |
| axis title | **smart label** on `\|axis\|` | reuse |
| plot frame / background | **`stroke`** / **`fill`** on the chart box | reuse |
| translucency | **`opacity`** / fill alpha | reuse |
| rounded bars | **`radius`** | reuse |
| horizontal bars | **`direction: row`** | reuse (generalised, §15) |
| log scale | **`scale: log`** on the axis | reuse compute (`log`/`ln`) |
| function plot | **`fn:` in `x`/`u`** | reuse the parametric seam |
| reference line / band / target | **extra children** in data space | free from the model |
| tooltip | **`<title>`** + `:hover` CSS | reuse `title:`; new hover |
| domain / crop / reverse | **`range: a b`** | new, small |
| auto colours | Nth series → Nth hue | new, trivial |
| stacked / grouped | **`bars:`** on the chart | new, small |

Net new vocabulary: `layout: chart`, the series types (`|bars|`/`|area|`/`|dots|`;
`|line|` reused), `|axis|`/`|band|`/`|mark|`, and the properties `data:`/`fn:`/
`range:`/`side:`/`at:`/`span:`/`baseline:`/`categories:`/`bars:`/`grid:`/`samples:`.
Everything visual rides Lini's existing paint, text, and marker properties — so a
chart themes, darkens, and bakes like any diagram.

---

## 17. The 11 real charts

A representative subset is worked below — between them they exercise every distinct
capability. The table maps all eleven to the patterns they stress; v0.1 §15 has each
worked in full and stays the reference corpus.

| Chart | Stresses | Worked |
|---|---|---|
| `cycle-times` | grouped bars, rounded, legend-first | §17.1 |
| `cycle-composition` | stacked + horizontal, zero segments | →row+stacked, like 17.1 |
| `tier-power` | incremental stacking (gap series), no-JS tooltip limit | →stacked |
| `shot-power` | **bands → phase-axis → segmentation**, dual axis, jumps/steps, marks | §17.2 |
| `barrel-thermal` | explicit `x y` points, fill-less zone bands, threshold | §17.3 |
| `motor-overload` | log axis, smooth area, single `fn:`, units, `legend: none` | §17.4 |
| `plasticizing-duty` | `1e6` sci-notation, region, many point marks | →§17.4 + marks |
| `pressure-envelope` | dual axis, ternary piecewise in one backtick | →dual-axis |
| `tcu-warmup` | named-function twin, smooth, setpoint marks | →§8 named fn |
| `booster-timeline` | the formula ceiling (points), area-to-value, rich annots | →§7 + §11 |
| `toggle` | four axes, log, reversed x, `=` locals | §17.5 |

### 17.1 `cycle-times` — grouped bars

```
|chart#cycle| {
  categories: "15 cm³ ABS" "30 cm³ ABS" "50 cm³ ABS" "50 cm³ PS" "50 cm³ PC"
} [
  |axis#val| "Cycle Time (s) — aluminum mold" { side: left }

  |bars| "1.8 kW" { data: 9 15 24 18 30; fill: --sky;   radius: 4 }
  |bars| "2.3 kW" { data: 7 13 20 14 27; fill: --amber; radius: 4 }
  |bars| "3.6 kW" { data: 7 10 14 13 19; fill: --teal;  radius: 4 }
]
```

### 17.2 `shot-power` — the keystone (bands → phase-axis → segmentation, dual axis)

```
|chart#shot| { samples: 20; grid: power } [
  |axis#power| "Power (kW)"        { side: left;  range: 0 4.6 }
  |axis#vcap|  "Booster V_cap (V)" { side: right; range: 32 49; stroke: --sky-deep }
  |axis#time|  "Time (s)"          { side: bottom; range: 0 20 }

  |band| "Close"    { span: 0 1.4;     fill: --accent }
  |band| "Inject"   { span: 1.4 3.1;   fill: --rose }
  |band| "Hold"     { span: 3.1 5.1;   fill: --rose-ink }
  |band| "Recovery" { span: 5.1 12.4;  fill: --amber }
  |band| "Open"     { span: 12.4 13.5; fill: --purple }
  |band| "Charge"   { span: 13.5 14.9; fill: --sky }
  |band| "Settle"   { span: 14.9 20;   fill: --gray }

  |line| "Motor draw" {
    stroke: --rose-deep; opacity: 0.8;
    fn: `0.12 + 1.2*exp(-((u-0.8)/0.12)^2)`  `1.5 + 3.0*u^1.1`  `0.5 - 0.12*u`
        `1.4 - 0.7*u`  `0.26 + 0.1*sin(pi*u)`  0.12  0.2
  }
  |line| "Heaters"   { stroke: --amber-deep; opacity: 0.8; fn: 0.6 0 1.2 0.6 1.2 0 1.2 }
  |line| "Booster V_cap" {
    stroke: --sky-deep; axis: vcap; stroke-style: dashed;
    fn: 48 `48 - 8*u^1.4` 40 40 40 `48 - 8*exp(-1.4*u)` `48 - 8*exp(-(1.4 + 2.75*u))`
  }

  |mark| "1.8 kW breaker" { at: 1.8; axis: power; color: --muted }
  |block| "full cycle 30.2 s →" { at: 17.5 3.2; color: --muted }
]
```

### 17.3 `barrel-thermal` — explicit points, fill-less zones, threshold

```
|chart#barrel| { grid: temp } [
  |axis#pos|  "Position from nozzle (mm)" { side: bottom; range: 0 450 }
  |axis#temp| "Temperature (°C)"          { side: left;   range: 0 325 }

  |line| "PP"  { stroke: --sky-deep; data: 0 225, 60 225, 118 221, 180 214, 235 207,
                 300 201, 362 196, 375 88, 388 64, 447 58, 485 55 }
  |line| "ABS" { stroke: --amber-deep; data: 0 250, 60 250, 118 244, … }
  |line| "PC"  { stroke: --red; data: 0 300, 60 300, 118 293, … }

  |band| "Zone 3"      { span: 0 118;   fill: none }
  |band| "Zone 2"      { span: 118 235; fill: none }
  |band| "Zone 1"      { span: 235 362; fill: none }
  |band| "Feed throat" { span: 388 450; fill: none }

  |mark| "pellets soften ≈ 130 °C" { at: 130; axis: temp; stroke-style: dashed; color: --muted }
]
```

### 17.4 `motor-overload` — log axis, smooth area, single formula

```
|chart#motor| { legend: none } [
  |axis#torque| "Torque (% of rated)" { side: bottom; range: 100 300; step: 50; unit: " %" }
  |axis#time|   "Max Burst Time (s)"  { side: left; scale: log; range: 1 1000 }

  |area| "Max burst time" { fn: `min(8 / (x/100 - 1)^2, 2000)`; curve: smooth; fill: --teal }

  |block| "300 % → 2 s" { at: 270 5; color: --red }
]
```

### 17.5 `toggle` — four axes, log, reversed x, locals

```
|chart#toggle| { legend: none } [
  |axis#travel| "Remaining Screw Travel (mm)" { side: bottom; range: 50 1 }      // reversed
  |axis#lockup| "Distance to Lockup (mm)"     { side: left;  range: 0 10;            color: --teal }
  |axis#screw|  "Screw Force (kN)"            { side: right; range: 0 14; step: 2;   color: --rose }
  |axis#clamp|  "Clamp Force (kN)"            { side: right; range: 0 320; step: 40; color: --sky }
  |axis#ma|     "Mechanical Advantage"        { side: right; scale: log; range: 1 100000; color: --amber }

  |area| "Mechanical Advantage" { axis: ma; curve: smooth; fill: --amber; fn: `193800 / x^2.909` }
  |area| "Screw Force (kN)" { axis: screw; curve: smooth; fill: --rose;
    fn: `ma = 193800 / x^2.909;
         platen = 1.32e-6 * x^3.909;
         clamp = min(300, 366 * max(0, 0.82 - platen));
         clamp > 0 ? min(12.6, clamp / (ma*0.95)) : 0` }
  |line| "Platen Travel (mm)" { axis: lockup; curve: smooth; stroke-style: dashed; stroke: --teal-deep; fn: `1.32e-6 * x^3.909` }

  |mark| "Mold touch" { at: 30.3; axis: travel; color: --muted }
]
```

---

## 18. Scope & build order

The brief: **x/y line / bar / dots first; simplicity first.** So build in three tiers
— each tier is shippable, each is a snapshot-tested slice of `src/layout/chart/`.

**Tier 1 — Core (the one-liner test).**
- `layout: chart` + `|chart|`; `|line|`, `|bars|`, `|dots|`; `data:` (values +
  points); auto scale (nice), auto bottom/left axes, auto gridlines, auto palette,
  auto legend; `categories:`; `<title>` tooltips.
- Modules: `mod.rs` (orchestrator), `scale.rs` (nice linear), `axis.rs`,
  `series_bar.rs`, `series_line.rs`.

**Tier 2 — Step up (most real charts).**
- `|axis#id|` control (`side`/`range`/`step`/`unit`/`scale: log`, multi-axis +
  binding); `fn:` formulas in `x` (+ named functions, locals); `|area|` + `baseline`;
  `|mark|`/`|band|` annotations; `bars: stacked`; `direction: row` (the §15 refactor);
  `curve: smooth/step`; `grid: <axis>`; the `:hover` tooltip card.
- Modules: add `bands.rs` (segmentation), `annot.rs`, `tooltip.rs`; extend `scale.rs`
  (log + reverse).

**Tier 3 — Deferred.**
- Time scale (date parse + nice time ticks); per-segment styling beyond a per-band
  `fn:`; expr-based tick formatting; `layout: pie` + `|slice|` (polar — §22); the
  integration ceiling stays "ship as `data:` points," not a feature.

---

## 19. Gotchas

The hazards that bite during implementation (carried from v0.1 — they were dead on):

1. **Tick-label margin is circular** — measure tick labels (`text::approx_width`),
   reserve the margin, *then* place. Text is already measured at compile time.
2. **Nice ticks** and **log ticks** (decades 1–9, label 1-2-5) are two routines.
3. **Dual-axis grids collide** → only the chart's `grid:` axis draws gridlines.
4. **Zero-size bars** (`Charge 0 0 1.4`) — emit no rect, don't shift the stack.
5. **Horizontal bars swap axes** — `direction: row` makes the category axis vertical;
   every "which axis is categorical" check reads `direction`.
6. **Auto-domain needs a pre-pass** — `range: auto` samples every series first; bars
   force 0 into the range; **stacked** sums per category (top = the sum, not the max).
7. **Sampling vs straightness** — linear `fn:` needs 2 samples, `sin` ~24; `samples:
   24` is the safe default, curvature-adaptive later.
8. **The formula ceiling** — integration/recurrence ships as precomputed points (§7).
9. **Per-segment styling** — a segmented `fn:` can carry per-segment style; a points
   series can't easily. Decide how far to take it.
10. **Reversed axis** — `range: 50 1` reverses; scale math *and* tick order honour it.
11. **`fn:` count vs bands** — a per-band `fn:` list must match the band count: a loud
    error, never silent truncation (§9).
12. **Mark point + label offset** — a point `|mark|`'s label needs an auto-offset so
    it doesn't sit on the curve.
13. **No-JS tooltip side-channel** — shows the plotted value only (§12).
14. **Draw order** — bands → grid → area → bars → line → dots → annotations → axis →
    labels → tooltip; emit in that order (or set `layer:` on generated nodes).
15. **Clipping** — data past `range:` clips to the plot area (image 1 crops at 20 s).
16. **`fmt` & formulas** — leave backtick text intact; align a `fn:` list and annotate
    each formula with its band name (§9).

---

## 20. What changed from v0.1, and why

The architecture (the big bet, the smart-label backbone, series-as-children, lower-
to-primitives, annotations-as-nodes, the gotchas, the real-chart corpus) is v0.1's and
is kept wholesale — it's excellent and validated. The changes:

1. **Lead with the trivial case (§1, new).** v0.1 opens on a loaded cheat-sheet and is
   validated against eleven *engineering* charts, so it reads as a power tool. The
   brief stresses simplicity and "focus on x/y line/bar/dots first." v0.2 puts the
   one-liner first and treats the engineering charts as the ceiling, not the entry —
   matching `cat -> dog -> bird`. *Why:* the simple-case on-ramp is the most on-brand
   thing about Lini and was the one thing missing.

2. **Reuse Lini's *real* paint properties; drop the colliding shorthands (§6, §16).**
   v0.1 wrote `color:` (line colour), `style: dashed`, and `width: 3` (line
   thickness) on line series. In Lini, `color` is **text** colour (currentColor,
   SPEC:1001), `width` is a **dimension**, and there is no bare `style:`. A `|line|`
   primitive's colour is **`stroke`**; dashing is **`stroke-style`**; thickness is
   **`stroke-width`**. v0.2 uses those. *Why:* correctness, and it *is* the
   property-reuse the brief asked for — a chart should style exactly like a shape.

3. **Add an explicit property-reuse audit (§16, new).** *Why:* the brief named reuse
   as a goal; a table that says "this maps to that existing property" is the
   deliverable for it, and it exposed the §2 corrections.

4. **Add an explicit scope / build order (§18, new).** Core / step-up / deferred,
   tied to the `src/layout/chart/` modules. *Why:* the brief said x/y line/bar/dots
   first; v0.1 presents the whole surface at once. Tiering it makes the simple thing
   shippable on its own and keeps each slice snapshot-testable (AGENTS.md style).

5. **Keep `|line|` as-is; do not rename the primitive to `|arrow|`** (v0.1 open Q8
   leaned yes). *Why:* the context-decides duality (`points:` outside, `data:`/`fn:`
   inside) is the same principle as `.hot` being a rule vs a worn class — a feature,
   not a wart. `|arrow|` is just `|line| { marker-end: arrow }` (add it as a template
   if wanted); removing `|line|` is churn across SPEC/samples/tests for no real gain.

6. **`|scatter|` → `|dots|`.** *Why:* the brief's own word ("dots"), it pairs with the
   plural-plain `|bars|`, and Lini favours plain words (`box`, `cyl`, `note`). Minor;
   `scatter` can be an alias.

7. **Resolve the open questions** that affect the surface: `range:` is one
   window+crop+reverse (Q2); `|area|` is explicit, not auto-promoted (Q4); the
   `layout`/`direction` refactor ships *with* charts, behind shorthands (Q5); free
   labels are `|block|` (Q6); `samples:` default 24 (Q7). *Why:* a v0.2 should decide
   what a v0.1 floated.

8. **Foreground the `x`/`u` injection seam (§8).** v0.1 mentions it; v0.2 ties it to
   the exact code path (`scene.rs` inserting the sample variable into the eval env),
   so the "charts will inject `x` the same way" comment in `expr.rs` is the literal
   implementation plan. *Why:* it makes the formula channel concrete and cheap.

Unchanged on purpose, because v0.1 got them right: the formula ceiling honesty (§7),
bands-drive-segmentation with a loud count error + fmt comments (§9), `|mark|`
placement-picks-shape (§10), `grid: <axis>` against moiré (§5), `side:` source-order
stacking for multi-axis (§5), and the no-JS hover-dots tooltip that bakes to static
(§12).

---

## 21. Open questions

Lean in **bold**.

1. **`categories:` shorthand** on the chart vs requiring `|axis| { labels: … }` —
   **keep the shorthand** for the common case, the axis child for control.
2. **`bars:` vs a per-series `stack`/`group`** — **chart-level `bars:`** (one knob;
   mixing per-series modes is rare and confusing).
3. **`at:` + `axis:` for marks** vs `at-x:` / `at-y:` — **`at:` + `axis:`** (fewer
   names; orientation from the bound axis).
4. **Marker series name** — `|dots|` (chosen) vs `|scatter|` alias — **`|dots|`,
   alias `scatter` later** if discoverability needs it.
5. **`curve:` values** — `linear` / `smooth` / `step` — enough? (chart.js `tension` is
   a number; a named set reads better and covers the configs.)
6. **`grid:` default** — the primary value axis (chosen) vs `none` — **primary value
   axis** (a chart wants gridlines by default).
7. **Tooltip card styling hook** — a reserved class (`.lini-chart-tip`) vs a `|tip|`
   template the user can restyle. Lean **class first**, template if asked.

---

## 22. Future modes

Both lean on the smart label and the big bet.

**Pie (`layout: pie`)** — polar; value → angle; slices walk the palette; smart label =
slice label/legend. New vocabulary: `|slice|` + `value:`. Easy and separate (a second
coordinate system), so it comes after x/y is solid.

```
|chart#spend| { layout: pie } [
  |slice| "Ads" { value: 40 }  |slice| "SEO" { value: 30 }  |slice| "Direct" { value: 30 }
]
```

**Mindmap / auto-flow (`layout: auto`)** — the engine places nodes; `[ ]` nesting is
tree structure; `direction:` picks the flavour (`radial` mindmap, `row` L-to-R tree,
`column` org chart) — the §15 payoff, the same `direction` word as bars and flexbox.

---

## 23. In one sentence

Charts are containers that **lower to rectangles, paths, and text** with one shared
auto-scale and the **smart label** doing every piece of text — so the one-liner
`|chart| [ |bars| { data: 30 45 28 60 } ]` is a complete chart, the eleven engineering
configs are 8–40 lines of name-first Lini, and both theme, dark/light, and bake like
any other diagram.

# Charts — Specification

An extension of [`SPEC.md`](SPEC.md): the source of truth for **charts**, written in
the same register and to the same standard. A chart is **a layout** — `layout: chart`
and `layout: pie` — so everything the core language defines (the cascade, paint roles,
the `"string"` rule, the expression engine, lower-to-primitives, theming, baking,
determinism) applies unchanged and is referenced, not restated. This document is
provisional only in placement: once charts are proven it folds into `SPEC.md` as two
more layout modes. Until then it is the law for charts.

**One bet carries the whole design.** A chart is a container that, at layout time,
fixes a shared data→pixel scale from its children, samples any formulas, and emits
ordinary Lini primitives and templates — `|line|`, `|poly|`, `|path|`, `|oval|`,
`|rect|`, `|block|`, and text. The renderer learns nothing new; charts theme,
dark/light, and bake like any diagram.

---

## Table of Contents

1 [Mental model](#1-mental-model) · 2 [The chart container](#2-the-chart-container) ·
3 [Series](#3-series) · 4 [Data & formulas](#4-data--formulas) · 5 [Axes](#5-axes) ·
6 [Scales & domain](#6-scales--domain) · 7 [Bands & segmentation](#7-bands--segmentation) ·
8 [Annotations](#8-annotations) · 9 [Legend & title](#9-legend--title) · 10 [Colour](#10-colour) ·
11 [Direction & the flip](#11-direction--the-flip) · 12 [Radial charts](#12-radial-charts-radar--radial-bar) ·
13 [Pie & donut](#13-pie--donut) · 14 [Tooltips](#14-tooltips) · 15 [Lowering & render](#15-lowering--render) ·
16 [Properties](#16-properties) · 17 [Grammar](#17-grammar) · 18 [Errors](#18-errors) ·
19 [Examples](#19-examples) · 20 [Deferred](#20-deferred)

---

## 1. Mental model

A chart is a container ([SPEC §5](SPEC.md)) whose **layout** is `chart` or `pie`. Its
children are **series** (and, for `layout: chart`, **axes**, **bands**, and
**annotations**) — drawn in *data* coordinates, not pixels. The chart's one new job
over `row`/`column`/`grid` is to read **all** children first, fix a **shared scale**
(data domain → plot pixels), then lower each child to primitives at baked pixel
coordinates. This is the chart analogue of a grid sizing tracks from its children.

Three properties of the model, each inherited from the core language:

- **The smart label carries every text** ([SPEC §3](SPEC.md)). The one `"label"` after
  a node's head lowers per type ([§9](#9-legend--title)): a chart's label is its
  **title**, a series' its **legend** entry, an axis's its **axis title**, a band's a
  **tick**, an annotation's its **label**.
- **Paint, text, and markers are the core properties** ([SPEC §10](SPEC.md)). A line's
  colour is `stroke`, an area/bar/slice body is `fill`, a dashed line is
  `stroke-style: dashed`, thickness is `stroke-width`; there are no chart-only paint
  shorthands. User text is quoted, bare words are identifiers ([SPEC §2](SPEC.md)).
- **Children paint the shared plane.** Reference lines, thresholds, shaded bands, and
  callouts are ordinary children placed in data coordinates ([§8](#8-annotations)) —
  not a separate annotation subsystem.

A chart errors only at compile time, with a span, like the rest of Lini
([§18](#18-errors)). Data outside an axis `range:` is clipped to the plot area.

---

## 2. The chart container

Two layouts, each a container template over `|block|` with the layout preset — exactly
as `|table|` is `grid + divider: all + gap: 0` ([SPEC §8](SPEC.md)):

| Layout | Template | Encodes | Children |
|---|---|---|---|
| `layout: chart` | `\|chart\|` | an x/value plane (cartesian or radial) | series, `\|axis\|`, `\|band\|`, `\|mark\|`, `\|bubble\|` |
| `layout: pie` | `\|pie\|` | part-to-whole, value → angle | `\|slice\|` |

`width` / `height` set the whole chart (plot **plus** axis gutters and legend); the
plot area is the remainder after labels are measured ([SPEC §6](SPEC.md) — text is
measured at compile time). Unset, a chart defaults to **360 × 220**; a `pie` or
`radial` chart is **square** (default **280**) — a chart cannot size to its content
(the content depends on the scale, which depends on the size), so these are baked
layout constants ([SPEC §11.5](SPEC.md)). `fill` is the chart background, `stroke` its
frame, and the cascade styles a chart like any box.

**Chart-level properties** (on the `|chart|` / `|pie|` node):

| Property | Layout | Value | Default |
|---|---|---|---|
| `direction` | chart | `column` · `row` · `radial` | `column` |
| `bars` | chart | `grouped` · `stacked` · `overlay` | `grouped` |
| `categories` | chart | quoted-string list — the x-axis (or spoke) labels | indices `1…N` |
| `samples` | chart | integer — `fn:` sample count | `24` |
| `hole` | pie | `0` ≤ n < `1` — inner-radius fraction (a donut) | `0` |
| `legend` | both | `top` · `right` · `bottom` · `none` | auto (shown when ≥ 2 entries) |
| `tooltip` | both | `none` · `hover` · `auto` · `always` — label presentation ([§14](#14-tooltips)) | `auto` |
| `gap` | both | number — the clear space between the plot and the title / legend outside it ([§9](#9-legend--title)) | `10` |

`categories` is the common-case shorthand for the **x (domain) axis's** tick labels;
an `|axis|` child's `labels:` ([§5](#5-axes)) is the general form. The two name the same
thing — setting both is an error ([§18](#18-errors)).

---

## 3. Series

A series is a child node; its smart label is its **legend** entry (no label → no
entry). Each series **lowers to primitives** ([§15](#15-lowering--render)) and is valid
only inside its layout (a series elsewhere is an error, like `cell:` off a grid —
[SPEC §5](SPEC.md)).

| Series | Layout | Draws | Lowers to | Paint |
|---|---|---|---|---|
| `\|line\|` | chart | a polyline through the data (a **closed** loop when `radial`) | `\|line\|` / `\|path\|` | `stroke`, `stroke-width`, `stroke-style` |
| `\|area\|` | chart | a line filled to a baseline | `\|poly\|` / `\|path\|` + `\|line\|` | `fill`, `stroke`, `baseline` |
| `\|bars\|` | chart | one bar per datum (a wedge when `radial`) | one `\|rect\|` / `\|poly\|` each | `fill`, `stroke`, `radius` |
| `\|dots\|` | chart | one marker per datum | one `\|oval\|` / marker each | `fill`, `stroke`, `marker` |
| `\|bubble\|` | chart | one bubble at a point, sized by `value:` | one `\|oval\|` | `fill`, `stroke` |
| `\|slice\|` | pie | one wedge | one `\|path\|` | `fill`, `stroke` |

**Singular vs. plural is the cardinality.** `|line|` and `|area|` are **one** shape, so
singular; `|bars|` and `|dots|` are a **set** of marks (one per datum), so plural —
the name states whether the series is one path or many marks. A `|slice|` is one wedge
and a `|bubble|` one bubble (singular, per node); a pie is several `|slice|` nodes and a
bubble chart several `|bubble|` nodes, as a cartesian chart is several series.

Inside a chart, `|line|` reads `data:` / `fn:` (data space); the standalone `|line|`
primitive ([SPEC §7](SPEC.md)) reads `points:` (pixels). The section decides which,
exactly as it decides a stylesheet rule from a worn class ([SPEC §4](SPEC.md)) — a
chart line *is* a line, so the name is reused, not duplicated.

**A line carries markers at every datum**, reusing the core `marker:` family
([SPEC §7](SPEC.md)) generalised from line *ends* to every vertex: `|line| { marker: circle }`
shows a marker at each point (`marker-start` / `marker-end` have no meaning on a chart
line and are ignored). A chart marker is a **centred** glyph, so only the symmetric kinds
apply — **`dot`** (a small point), **`circle`** (a larger, hover-sized point), and
**`diamond`** (a rhombus); the directional `arrow` / `crow` have no centred reading and
are an error on a series ([§18](#18-errors)). Every marker carries the datum's `<title>`
([§14](#14-tooltips)), so a marked point is a hover target — reach for `circle` when the
point is something to hover or read. `|dots|` is markers with no connecting line, **`circle`**
by default; its diameter is `width` (`height` too for an ellipse), its shape `marker:` —
there is **no** `size:` property.

**`curve:`** sets a line's / area's interpolation:

| `curve:` | Connects points by |
|---|---|
| `linear` *(default)* | straight segments |
| `smooth` | a **monotone** cubic — curved, passes through every point, **never overshoots** (no invented peak or sub-zero dip). Parameter-free; there is no tension knob. |
| `step` | a staircase — hold, then step at each datum |

**`bars:`** on the chart sets how multiple **`|bars|`** series combine: `grouped`
(side-by-side per category, the default), `stacked` (piled; the top is the sum), or
`overlay` (on top, translucent). `radius` rounds a bar's corners. (Stacked areas are
[deferred](#20-deferred); areas overlay.)

**A `|bubble|` is one mark per node** — for individually named, coloured, and sized
bubbles, like a `|slice|`: `|bubble| "Name" { at: x y; value: N; fill: … }` places a
bubble at data point (x, y), sized by `value:`. The chart reads every bubble's `value:`
and scales them **by area** (area ∝ value) so the largest fits; the smart label sits
**centred in the bubble when it fits, else on hover**. `at:` and `value:` are the very
properties a `|mark|` and a `|slice|` use ([§8](#8-annotations), [§13](#13-pie--donut)).
Reach for `|bubble|` when each bubble is a distinct labelled entity; for many uniform
points, a `|dots|` series is terser.

---

## 4. Data & formulas

A series' values come from `data:` (explicit) or `fn:` (computed) — never both
([§18](#18-errors)). Both use the core value grammar ([SPEC §16](SPEC.md)) — space
within a group, comma between groups — so charts add **no value form**. A comma is the
discriminator:

| Source | Syntax | Meaning |
|---|---|---|
| categorical | `data: 9 15 24 18 30` | **one group** → one value per category (`categories:` / indices) |
| points | `data: 0 225, 60 225, 118 221` | **comma groups** → `x y` pairs (numeric x; scatter / irregular) |
| formula | `fn: ` `` `min(8/(x/100-1)^2, 2000)` `` | a backtick expression in `x`, sampled at `samples:` |

So a comma-less `data:` is always a value list; a single point cannot be written
comma-less (`data: 9 15` is two categorical values, not one pair). A `|line|` / `|area|`
needs ≥ 2 vertices. With categorical data, the value count must match the `categories:`
count ([§18](#18-errors)).

**`tags:` labels each datum.** A series' smart label is its *legend* entry
([§9](#9-legend--title)) — one name for the whole line; **`tags:`** is the **per-datum**
text, a quoted-string list parallel to `data:` (one tag per value, or per `x y` point):

```
|line| "GLM-5.2" { data: 35 63, 42 72, 84 75; tags: "Non-Thinking" "High" "Max"; marker: circle }
```

A tag rides with its datum: it shows **on the plot** beside the point and, when there is
no room, **on hover** — the placement is `tooltip:`'s job ([§14](#14-tooltips)). The tag
count must equal the data count ([§18](#18-errors)); `tags:` needs discrete `data:` (a
sampled `fn:` has no authored points to label, so `tags:` with `fn:` is an error). A
per-node mark — `|bubble|`, `|slice|`, `|mark|` — takes no `tags:`: its one smart label
*is* its point label.

**Formulas are the core expression engine** ([SPEC §11.7](SPEC.md)): operators, the
math library, `name = expr;` locals, the ternary, and stylesheet functions. Charts bind
two ambient names into it — the same seam that injects `u` for parametric `points:`:

- **`x`** — the x-axis data value (the domain position); a whole-domain `fn:` uses it.
- **`u`** — a band-local clock, `0 → 1` across one band ([§7](#7-bands--segmentation)).

A `fn:` is therefore **not folded at resolve** (its `x` is unbound there) but held and
**sampled at chart layout**, once the x-domain is fixed, with `x` (and `u`) bound at
each step. It reuses the same sample-an-ambient seam a parametric `points:` uses for
`u` ([SPEC §11.7](SPEC.md)), only deferred to the layout phase because `x`'s domain
comes from sibling data. The sampled result bakes to literals like any geometry.
`samples:` is the step count (default 24).

Locals chain derivations in one backtick; a stylesheet function keeps twins DRY:

```
{ ramp(s) `min(100, 25 + 1.572*(x/s) + 0.0142*(x/s)^2)`; }
…
|area| "Steel"    { fn: ramp(1) }
|line| "Aluminum" { fn: `ramp(1/0.7)` }
```

**The formula ceiling.** `fn:` expresses a function of `x`, not a recurrence: a numeric
integration (a running sum) has no closed form and ships as precomputed `data:` points.
`fn:` covers functions; it does not pretend to cover integration.

---

## 5. Axes

An axis is an `|axis|` child of a `layout: chart` (an `#id` is optional, used to
**bind** — a series or annotation reads an axis with `axis:`); its smart label is the
**axis title**. A chart with no `|axis|` gets an x (domain) axis and an auto-fit value
axis, so simple charts declare none — an axis is written only to *say* something.

| Property | Value | Notes |
|---|---|---|
| `side` | `bottom` · `left` · `right` · `top` | cartesian only; several axes on one side stack outward in **source order**. |
| `range` | `a b` (each end a number or `auto`) | the data window — and crop, and reverse ([§6](#6-scales--domain)). |
| `scale` | `linear` · `log` | `log` emits decade ticks labelled 1-2-5; its domain must be above 0. |
| `step` / `ticks` | number / list | tick spacing, or explicit ticks; omitted → nice ticks ([§6](#6-scales--domain)). |
| `unit` | `"%"` | a quoted suffix appended to tick labels (and tooltips). |
| `labels` | quoted-string list | explicit tick text — the general form of `categories:` ([§2](#2-the-chart-container)). |
| `gridlines` | `none` · *colour* | this axis's gridlines: `none`, or a colour (a colour turns them on). |
| `stroke` / `color` / `font-size` | core | `stroke` tints the axis line + ticks, `color` the labels + title ([SPEC §10](SPEC.md)). |

An **x (domain) axis** is categorical when `categories:` / `labels:` give it labels (or
by default, indices `1…N`) and numeric when the data is points or a `fn:`. A **value
axis** carries series magnitudes; `axis: <id>` on a series binds it (default: the first
value axis of the series' orientation). Multiple value axes share a plot for dual-unit
charts; only the **primary** value axis (the first declared) and the x axis draw
gridlines by default — so a normal grid appears, and a second value axis adds none
(avoiding moiré). Override per axis with `gridlines:`; the x axis's (vertical) and a
value axis's (horizontal) gridlines are perpendicular and never conflict. The default
tint is `--lini-grid` — a faint role variable charts add to the palette
([SPEC §11.1](SPEC.md)), themeable and dark/light-aware like the rest.

---

## 6. Scales & domain

Each axis owns one scale: data **domain** → pixel **range**. By default the domain is
the union of the bound series' data, rounded to nice tick steps; a value axis carrying
`|bars|` includes zero.

**`range: a b`** does three jobs at once: it sets the visible **window** (`a`…`b`),
**crops** data outside it to the plot area, and **reverses** the axis when `a > b`
(`range: 50 1` runs high→low — both the scale and the tick order flip). Either end may
be `auto` to auto-fit it (`range: 0 auto`, `range: auto 100`); the two ends must be
distinct (a zero-width domain is an error — [§18](#18-errors)).

Ticks are "nice" by default (1-2-5 × 10ⁿ); `step:` sets a spacing, `ticks:` an explicit
list, `scale: log` switches to decade ticks. A `log` axis's domain — explicit or auto —
must be above 0 ([§18](#18-errors)). Tick **labels** come from `categories:` / `labels:`
(an x axis) or the formatted tick value + `unit:` (a value axis).

---

## 7. Bands & segmentation

A `|band|` is a child that partitions an axis and drives three things from one
declaration: a background **shade**, a **tick** (its smart label), and the **segment
boundaries** every series shares.

```
|band| "Inject" { span: 1.4 3.1; axis: time; fill: --rose }
```

`span: a b` is the band's data range on its bound `axis:` (the same `span:` a grid cell
uses, now valid on a chart band too — [SPEC §5](SPEC.md)); `fill: none` makes it a divider + label with
no shading (a zone marker). The chart collects its `|band|` children, in source order,
as the partition.

**A series opts into segmentation** with a per-band `fn:` **list** — one backtick (or a
bare constant) per band, evaluated in local `u`:

```
|band| "Close"  { span: 0 1.4;   fill: --accent }
|band| "Inject" { span: 1.4 3.1; fill: --rose }
|band| "Hold"   { span: 3.1 5.1; fill: --amber }
…
|line| "Motor draw" {
  stroke: --rose-deep;
  fn: `0.12 + 1.2*exp(-((u-0.8)/0.12)^2)`   // Close
      `1.5 + 3.0*u^1.1`                      // Inject
      0.5                                    // Hold (a constant segment)
}
```

A **single** `fn:` (one backtick) samples the whole domain in `x` and ignores bands —
segmentation is opt-in. Consecutive segments connect end-to-start (the riser is drawn),
so a jump between segments is explicit and a list of constants draws clean steps. A
per-band `fn:` list whose length differs from the band count is an error
([§18](#18-errors)) — never a silent truncation.

---

## 8. Annotations

Annotations are children placed in **data** coordinates; the model gives them for free.
There are two, both reusing core paint. `axis:` names the axis an annotation is measured
against (for a point, its value axis); it is required.

| Node | Form | Draws |
|---|---|---|
| `\|mark\|` | `\|mark\| "100 °C max" { at: 100; axis: temp; stroke-style: dashed }` | a reference **line** at value 100, across the plot perpendicular to `temp` |
| `\|mark\|` | `\|mark\| "60 °C — 19 min" { at: 19 60; axis: temp }` | a **point** (dot + label): `x = 19`, value `60` on `temp` |
| `\|mark\|` | `\|mark\| "safe region" { at: 170 4; axis: temp; marker: none }` | a **label** only (no dot) |
| `\|band\|` | `\|band\| { span: a b; axis: … }` | a shaded region — the one-off cousin of a partition band ([§7](#7-bands--segmentation)) |

A `|mark|`'s placement decides its shape: `at: V` (one value) is a reference **line** at
value `V` on its bound axis, drawn across the plot perpendicular to that axis (so a
value-axis mark is a level line, an x-axis mark a vertical); `at: X Y` (two values) is a
**point** — `X` on the x axis, `Y` on the bound value `axis:`. `marker: none` suppresses
a point's dot, leaving the label — so there is no separate free-label node. Because
placement is by *value* on a *named* axis, an annotation survives a `direction` flip
([§11](#11-direction--the-flip)) unchanged.

---

## 9. Legend & title

One smart-label rule ([SPEC §3](SPEC.md)), placed by where the label sits:

| Label on | Becomes |
|---|---|
| the `\|chart\|` / `\|pie\|` | the **title** (a caption above the plot) |
| a series / `\|slice\|` | a **legend** entry, with a swatch **mirroring its paint** — fill and edge, so an outlined bar / slice gets an outlined swatch ([§10](#10-colour)) |
| an `\|axis\|` | the **axis title** |
| a `\|band\|` | a **tick** along the band's axis, tinted its `fill` |
| a `\|mark\|` | the annotation's **label** |

A legend appears automatically once there are ≥ 2 entries; `legend: top | right |
bottom | none` positions or suppresses it ([§2](#2-the-chart-container)). **`gap:`** sets
the clear space between the plot and the title (above) and legend (below) that sit
outside it — the chart repurposes the core container `gap` (a chart owns its layout, so
it has no inter-child spacing of its own). It defaults to `10`; `gap: 0` ≈ touching.

The chart's **chrome** — its title and legend — stays **bold** (the diagram-wide default,
[SPEC §10](SPEC.md)); its **data text** — axis ticks, tags ([§14](#14-tooltips)), and
annotation labels — is **normal** weight, so the numbers read quietly beneath the captions.

---

## 10. Colour

Explicit `stroke:` / `fill:` wins. Otherwise series **walk the palette**
([SPEC §11.2](SPEC.md)) in declaration order, skipping `red` (reserved for danger), in
this fixed sequence — repeating if exhausted, so the result is deterministic
([§15](#15-lowering--render)):

```
--rose  --orange  --amber  --lime  --green  --teal  --sky  --blue  --purple  --gray
```

Each series takes its hue at the tier the role wants — **the outlined look**: a `|bars|` /
`|area|` / `|slice|` fills with the **`soft`** tier and gains a **`deep`** edge below it
(`stroke: none` removes the edge — a flat fill); a line takes the `deep` stroke, dots the
`ink`. An explicit `fill:` keeps its own colour and still gains a deep edge of it; an
explicit `stroke:` is that edge. The legend swatch **mirrors** the series' paint — its
fill and its edge — and the tooltip accent follows the dominant paint (its `fill` if it
has one, else its `stroke`). In `layout: pie` the walk is **per slice** (each part a
distinct colour), the one place colour walks per datum rather than per series.

---

## 11. Direction & the flip

`direction` is a chart property (new in this spec; a core `layout`/`direction` split —
generalising `layout: row`/`column` to an engine + orientation — is planned for
`SPEC.md` and would subsume it). It sets the chart's orientation:

| `direction` | Plane | Bars grow |
|---|---|---|
| `column` *(default)* | cartesian | up |
| `row` | cartesian | right |
| `radial` | polar ([§12](#12-radial-charts-radar--radial-bar)) | outward from centre |

**The flip never breaks a chart, because nothing is authored in screen coordinates.**
You write `categories:`, series `data:` (values), and annotations bound to a **named**
axis (`axis: rev`) with `at:` / `span:` in *data* values — all logical. `direction`
only changes how that logical plane is projected: `column → row` swaps the default
placement (the value axis to the bottom, the x axis to the left); a
`|mark| { at: 40; axis: rev }` stays "at rev = 40 on the rev axis", merely re-projected.
Every series follows the same projection, so a mixed bar+line chart stays coherent when
flipped. An explicit axis `side:` is a screen edge and is honoured as written, so set it
for the orientation you are in; there is no `x:`/`y:` in chart syntax to go stale.

---

## 12. Radial charts (radar & radial bar)

`layout: chart; direction: radial` projects the same cartesian model into polar
coordinates: the **x (domain) axis** bends into a ring (categories become evenly-spaced
**spokes**, starting at the top, clockwise) and the **value** axis becomes the
**radius** (centre = range minimum, rim = range maximum). The series types are unchanged
— this is the cartesian chart, drawn radially:

| Series in `radial` | Is the chart called |
|---|---|
| `\|line\|` (closed loop) | a **radar** (spider/web) |
| `\|area\|` | a **filled radar** |
| `\|bars\|` (wedges) | a **radial bar** (polar area) |
| `\|dots\|` | points on the spokes |

A radial chart has **one value (radius) axis** — `|axis| { range: 0 5 }`; writing
`side:` on it is an error ([§18](#18-errors)) — shared by every spoke, and one x axis
(the spokes, from `categories:`). The radius axis's gridlines are concentric
**polygons** through the spokes (the radar "web"); the spokes themselves are the x
gridlines. A radar `|line|` connects a series' value on every spoke and **closes** back
to the first; an `|area|` fills that polygon; `|bars|` fill their angular slot to a value
radius. `bars:` (`grouped` / `stacked`) and the palette walk behave as in cartesian.
Concentric **circular** gridlines (the polar-area look) and a configurable start
angle / direction are [deferred](#20-deferred); the polygon web is the default.

---

## 13. Pie & donut

`layout: pie` encodes value as **angle** — each slice's angle is its value over the
total — a different scale from radial's value-as-radius, hence its own layout. It has no
axes; its children are `|slice|` nodes:

```
|pie| "Spend" { hole: 0.5 } [
  |slice| "Ads"    { value: 40 }
  |slice| "SEO"    { value: 30 }
  |slice| "Direct" { value: 30 }
]
```

A `|slice|` is a single-value series ([§3](#3-series)): its `value:` is its magnitude
(`≥ 0`), its smart label its legend entry, and it walks the palette like any series — so
slices are distinctly coloured by default. Slices fill the circle in source order,
clockwise from the top, each angle = `value / Σ value × 360°`; a total of zero is an
error ([§18](#18-errors)). **`hole:`** (a `0` ≤ n < `1` fraction of the radius, on the
`|pie|`) cuts an inner hole — `hole: 0` is a pie, `hole: 0.5` a donut. On-slice
value / percent labels, a centred total in the hole, and exploded slices are
[deferred](#20-deferred); the legend (from slice labels) carries naming for now.

---

## 14. Tooltips

A datum's label has two presentations, and one property — **`tooltip:`** — sets how much
shows where. Hover is the only interactivity, with no script ([SPEC §13](SPEC.md) governs
output):

| `tooltip:` | On the plot (inline) | On hover | For |
|---|---|---|---|
| `none` | — | — | a clean static plot, no labels |
| `hover` | — | card + `<title>` | labels on demand |
| `auto` *(default)* | where it fits, else falls to hover | card + `<title>` | the printable default |
| `always` | every label, forced | card + `<title>` | export — every label must read |

The two texts **complement**: the *inline* label is the datum's own text — a series'
`tags:` entry ([§4](#4-data--formulas)), or a per-node mark's smart label (`|bubble|` /
`|slice|`) — while *hover* shows its **value**. So a point can read `Max` on the plot and
`GLM-5.2: 75%` on hover, never competing.

**The hover floor is always honest.** A labelled mark carries a native `<title>`
([SPEC §10](SPEC.md) `title:`) — its accessible name, readable in any renderer and
surviving `--bake-vars`. Over it, a live CSS `:hover` rule reveals a hidden
`<g class="lini-chart-tip">` card built from primitives — minimal by default (name and
value), positioned beside the point so it never blankets the plot; `.lini-chart-tip` is a
reserved styling hook. The card is **live-only**: a baked SVG keeps the `<title>` and
drops the `:hover` rule. Only `tooltip: none` strips the `<title>` too.

**Inline placement is one greedy pass, not a solver.** Each label tries a few offsets
around its point — above, below, beside, the diagonals; a `|bubble|` first tries *centred
inside* — and takes the first that clears the labels already placed and stays in the plot.
Under `auto` a label with nowhere to sit **drops to its hover card** (so the tag is never
lost); under `always` it is placed at its preferred offset regardless. The pass is
O(labels² + labels·segments) over a *sparse* set (data points, not samples), so it is fast
and deterministic — it never iterates to convergence like routing ([LINKING.md](LINKING.md)).
A seat must also sit **off the series lines** — a tag never lands on a `|line|` / `|area|`
stroke (bars and bubbles fill a region a tag reads fine beside, so they don't constrain
it). Inline labels are small and muted (`color:` overrides, default `--muted`) and carry
`pointer-events: none`, so a label never blocks the hover of the point beneath it.

**`tooltip:` cascades** ([SPEC §4](SPEC.md)): set on the `|chart|` it is the default for
every series; a series (or `|bubble|`) overrides it — `|bubble| { tooltip: always }` — so
one knob tunes the whole plot and any mark refines it.

**Hit targets stay sparse** — a sampled curve draws at `samples:` density but a marker
(hence a hover target) sits only at data points / turning points (~10–20 per series), so
node count stays bounded.

---

## 15. Lowering & render

`layout: chart` / `layout: pie` resolve in the layout phase ([SPEC §17](SPEC.md)),
since the shared scale needs every child's data first:

1. **Collect** series; resolve `data:` to data-space points (a `fn:`, held unfolded
   from resolve, is sampled here once the x-domain is known — [§4](#4-data--formulas)).
2. **Domain** per axis from the union of bound series (nice-rounded unless `range:` set;
   bars force zero); build each scale.
3. **Plot rect** = the chart box inset by axis-label and legend gutters (text measured
   at compile time).
4. **Lower** every series, axis, band, annotation, and the legend to primitives in baked
   pixel coordinates: line→`|line|`/`|path|`, area→`|poly|`/`|path|`+`|line|`,
   bars→`|rect|`s (or `|poly|` wedges, radial), dots→`|oval|`s/markers, slice→`|path|`,
   ticks/labels→text, gridlines→`|line|`s, the tooltip card→a `|block|`.
5. **Emit** in a **semantic draw order** — bands → gridlines → areas → bars → lines →
   dots → annotations → axes → labels → inline data labels → tooltip — so a line sits above its bars without
   hand-ordering. This is the one place a chart overrides source-order rendering
   ([SPEC §6](SPEC.md)); `layer:` on a generated node still overrides it.

The output is an ordinary primitive subtree, so route/render, theming, the palette,
gradients, shadows, `--bake-vars`, `fmt`, and byte-for-byte determinism
([SPEC §14, §17](SPEC.md)) all apply with no chart-specific code. Because the
data→pixel lowering needs the shared scale, it runs in the **layout** phase, not
desugar — so `lini desugar` ([SPEC §14](SPEC.md)) shows a chart's *type* desugaring (a
`|chart|` is a `|block|` wearing `.lini-chart`; each series a classed `|block|` carrying
its `data:` / `fn:`), while the geometric primitive subtree — the bars, paths, ticks — is
produced at layout, a render-time artefact like a link's routed geometry. That subtree
still themes, bakes, and renders like any primitive. Charts add **no lexer or parser
grammar** ([§17](#17-grammar)) — they are nodes, declarations, and children per
[SPEC §16](SPEC.md); the new surface is type names ([§3](#3-series)), properties
([§16](#16-properties)), and the two layout algorithms.

---

## 16. Properties

New properties, with the layout / node each applies to. All paint, text, geometry, and
`marker:` properties are the core ones ([SPEC §10](SPEC.md)), used with their core
meaning.

| Property | On | Value | Notes |
|---|---|---|---|
| `direction` | `\|chart\|` | `column` · `row` · `radial` | orientation ([§11](#11-direction--the-flip)). |
| `bars` | `\|chart\|` | `grouped` · `stacked` · `overlay` | multi-`\|bars\|` mode. |
| `categories` | `\|chart\|` | quoted-string list | x-axis / spoke labels. |
| `samples` | `\|chart\|` | integer | `fn:` sample count (default 24). |
| `hole` | `\|pie\|` | `0` ≤ n < `1` | donut inner radius. |
| `legend` | `\|chart\|` `\|pie\|` | `top` · `right` · `bottom` · `none` | legend placement. |
| `tooltip` | `\|chart\|` `\|pie\|` · series | `none` · `hover` · `auto` · `always` | label presentation; cascades ([§14](#14-tooltips)). |
| `gap` | `\|chart\|` `\|pie\|` | number | title / legend gutter ([§9](#9-legend--title)); `0` ≈ touching. |
| `data` | series | value list / `x y` pairs | explicit data ([§4](#4-data--formulas)). |
| `fn` | series | backtick, or a per-band list | computed data ([§4](#4-data--formulas), [§7](#7-bands--segmentation)). |
| `tags` | series | quoted-string list | per-datum labels, parallel to `data:` ([§4](#4-data--formulas)). |
| `baseline` | `\|area\|` | number | fill target — default the axis zero, or the range floor when zero is out of range. |
| `curve` | `\|line\|` `\|area\|` | `linear` · `smooth` · `step` | interpolation ([§3](#3-series)). |
| `axis` | series, `\|mark\|`, `\|band\|` | an `\|axis\|` id | the axis to read / measure against. |
| `value` | `\|slice\|` `\|bubble\|` | number ≥ 0 | slice angle / bubble size ([§13](#13-pie--donut)). |
| `side` | `\|axis\|` | `bottom` · `left` · `right` · `top` | cartesian axis edge. |
| `range` | `\|axis\|` | `a b` (ends number or `auto`) | window + crop + reverse ([§6](#6-scales--domain)). |
| `scale` | `\|axis\|` | `linear` · `log` | scale kind. |
| `step` / `ticks` | `\|axis\|` | number / list | tick spacing / explicit ticks. |
| `unit` | `\|axis\|` | quoted string | tick-label suffix. |
| `labels` | `\|axis\|` | quoted-string list | explicit tick text. |
| `gridlines` | `\|axis\|` | `none` · colour | this axis's gridlines. |
| `at` | `\|mark\|` `\|bubble\|` | `V` / `X Y` | line value / point ([§8](#8-annotations)). |
| `span` | `\|band\|` | `a b` | band range on its axis (the grid `span:`, [SPEC §5](SPEC.md)). |

---

## 17. Grammar

Charts add **no grammar** to [SPEC §16](SPEC.md). A chart is a `node` with
`layout: chart` (or the `|chart|` / `|pie|` template); series, axes, bands, marks, and
slices are child `node`s; `data:` / `fn:` / `range:` / `at:` / `span:` are ordinary
`decl`s whose values are the existing `value` forms (a list of points is comma-grouped;
a per-band `fn:` is a space-group of backticks and numbers). The additions are the
built-in **type names** — `chart`, `pie`, `line`, `area`, `bars`, `dots`, `bubble`,
`slice`, `axis`, `band`, `mark` (`line` already exists as a primitive, [SPEC §7](SPEC.md), and is
reused) — protected from shadowing like any built-in ([SPEC §15](SPEC.md)) yet free as
ids ([SPEC §18](SPEC.md)), and the properties of [§16](#16-properties).

---

## 18. Errors

Format and discipline per [SPEC §15](SPEC.md): `filename:line:col: error: <message>`,
compile-time, with a span.

| Condition | Message |
|---|---|
| Series outside a chart | `'\|bars\|' is a chart series — it belongs in a 'layout: chart'` |
| `\|slice\|` outside a pie | `'\|slice\|' belongs in a 'layout: pie'` |
| Axis / band / mark outside a chart | `'\|axis\|' belongs in a 'layout: chart'` |
| Pie given an axis or series | `a pie's children are '\|slice\|' only` |
| Empty chart | `a chart needs at least one series` |
| Empty pie | `a pie needs at least one '\|slice\|'` |
| Series with both `data:` and `fn:` | `a series takes 'data' or 'fn', not both` |
| Series with neither | `a series needs 'data' or 'fn'` |
| `arrow` / `crow` marker on a series | `'marker: arrow' has no centred form on a chart — use dot, circle, or diamond` |
| `fn:` list ≠ band count | `'fn' has N formulas but the chart has M bands` |
| Data ≠ categories count | `series data has N values but the chart has M categories` |
| `tags:` count ≠ data count | `'tags' has N labels but the series has M data points` |
| `tags:` on an `fn:` series | `'tags' needs explicit 'data' — a sampled 'fn' has no points to label` |
| `categories:` with an axis `labels:` | `set 'categories' or an axis 'labels', not both` |
| `\|mark\|` without `axis:` | `a '\|mark\|' needs 'axis:' to place it` |
| `\|mark\|` `at:` wrong arity | `'at' takes one value (a line) or two (a point)` |
| `\|bubble\|` missing `at:` / `value:` | `a '\|bubble\|' needs 'at:' (x y) and 'value:'` |
| Unknown `axis:` id | `axis 'X' not found` + `; did you mean 'Y'?` |
| `range:` not two ends | `'range' takes two ends: 'a b', 'a auto', or 'auto b'` |
| `range:` equal ends | `'range' needs distinct ends` |
| `scale: log` over a non-positive domain | `a 'scale: log' axis needs a domain above 0` |
| `side:` in `direction: radial` | `'side' has no meaning in a radial chart — it has one radius axis` |
| `hole:` out of range | `'hole' is a fraction 0..1` |
| Negative slice value | `a '\|slice\|' value must be ≥ 0` |
| Pie total of zero | `a pie's slice values sum to zero` |

---

## 19. Examples

**Grouped bars, categorical, legend-first:**
```
|chart| "Cycle time (s)" { categories: "15 cm³" "30 cm³" "50 cm³" } [
  |bars| "1.8 kW" { data: 9 15 24; fill: --sky }
  |bars| "2.3 kW" { data: 7 13 20; fill: --amber }
]
```

**Dual-axis, a formula, a band, a threshold:**
```
|chart| "Injection profile" [
  |axis#bar|  "Pressure (bar)" { side: left;  range: 0 1100 }
  |axis#flow| "Flow (cm³/s)"   { side: right; range: 0 50; gridlines: none }
  |axis#x|    "Speed (mm/s)"   { side: bottom; range: 0 133 }

  |area| "Pressure" { axis: bar;  fn: `x <= 93 ? 1000 : 1000 - 319*((x-93)/40)`; fill: --teal }
  |line| "Flow"     { axis: flow; fn: `x*42/133`; stroke: --rose-deep; stroke-style: dashed }

  |band| { span: 93 133; axis: x; fill: --red }
  |mark| "1000 bar @ 93" { at: 93; axis: x; color: --muted }
]
```

**Radar (closed lines on radial spokes):**
```
|chart| "Profiles" { direction: radial; categories: "Speed" "Range" "Armor" "Cost" "Stealth" } [
  |axis| { range: 0 5 }
  |line| "Scout"   { data: 5 4 2 3 5 }
  |area| "Cruiser" { data: 3 3 5 4 2; fill: --teal }
]
```

**Labelled points (`tags:` per datum, hover-sized markers, always shown):**
```
|chart| "Effort vs. score" [
  |axis| "tokens (k)" { side: bottom }
  |axis| "score %"    { side: left }
  |line| "GLM-5.2" { data: 35 63, 42 72, 84 75; tags: "Non-Thinking" "High" "Max"; marker: circle; tooltip: always }
]
```

**Bubbles (one node each — position, size, colour, name):**
```
|chart| "Markets" [
  |axis| "growth %" { side: bottom }
  |axis| "share %"  { side: left }
  |bubble| "EU"   { at: 12 40; value: 8;  fill: --teal }
  |bubble| "US"   { at: 25 55; value: 20; fill: --rose }
  |bubble| "APAC" { at: 38 30; value: 14; fill: --sky }
]
```

**Donut:**
```
|pie| "Spend" { hole: 0.5 } [
  |slice| "Ads" { value: 40 }  |slice| "SEO" { value: 30 }  |slice| "Direct" { value: 30 }
]
```

---

## 20. Deferred

Named, not yet built; the syntax above is stable without them.

- **Gauge** — a partial pie / arc for a single value against a total.
- **Stacked areas** — `bars: stacked` extended to `|area|` series.
- **Polar-area circular gridlines** and a configurable radial **start angle /
  direction** (the polygon web and top-clockwise are the defaults — [§12](#12-radial-charts-radar--radial-bar)).
- **Per-slice explode**, **on-slice value / percent labels**, and a **centred total**
  in a donut hole ([§13](#13-pie--donut)).
- **Per-segment styling** — a per-band style list mirroring a segmented `fn:`
  ([§7](#7-bands--segmentation)).
- **Time scale** — date domains with calendar-aware nice ticks.
- **Multi-ring pie / sunburst** — nested `|slice|` levels.
- **Per-datum styling** — a parallel *paint* list over `data:` (highlight one bar); today,
  overlay a `|mark|`. (Per-datum *text* already exists — `tags:`, [§4](#4-data--formulas).)

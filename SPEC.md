# Lini — Language Specification (v4)

A small, human-readable language for plain-text diagrams. Flex/grid layout,
composable primitives, CSS-driven theming — compiles to clean SVG.

**Lini reads like CSS.** `key: value;` declarations in `{ }` blocks, dash-case
property names, space-separated values, real selectors. Only a handful of
concepts stay Lini-specific — the ones CSS has no word for (the `|type|` sigil,
edge anchoring, wire operators).

This document is complete: an implementer can build a conforming engine from it
alone. Wire **routing** has its own contract — see [`WIRING.md`](WIRING.md).

---

## Table of Contents

**Language** — 1 [Mental Model](#1-mental-model) · 2 [Lexical Syntax](#2-lexical-syntax) ·
3 [Statements](#3-statements) · 4 [Selectors & the Cascade](#4-selectors--the-cascade) ·
5 [Layout](#5-layout) · 6 [Positioning & Anchors](#6-positioning--anchors) ·
7 [Primitives](#7-primitives) · 8 [Templates](#8-templates) · 9 [Wires](#9-wires)

**Reference** — 10 [Properties](#10-properties) · 11 [Variables & Defaults](#11-variables--defaults) ·
12 [Specificity](#12-specificity) · 13 [SVG Output](#13-svg-output) · 14 [CLI](#14-cli) ·
15 [Errors](#15-errors) · 16 [Grammar](#16-grammar-ebnf) · 17 [Implementer Algorithm](#17-implementer-algorithm) ·
18 [Reserved Words](#18-reserved-words) · 19 [Deferred & Non-Goals](#19-deferred--non-goals) · 20 [Examples](#20-examples)

---

## Quickstart

```
cat -> dog -> bird
```

That's a complete diagram: three boxes, two arrows. Lini fills in the rest.

| Form | Means |
|---|---|
| `\|name\|` | An instance — draw this type (`\|rect\|`, `\|group\|`). **Bars = canvas.** |
| `name { … }` | A rule — style every `name`. **Bare name = stylesheet.** |
| `key: value;` | A declaration — configures the thing it's in. |
| `.name` | A class — define it (`.hot { … }`) or apply it (`box .hot`). |
| `--name` | A themeable variable (`fill: --accent`). |
| `a -> b` | A wire. |

Three defaults make small diagrams trivial:

- Omit the type → `|rect|`.
- Omit the label → the node's id (`""` to suppress it).
- Name an undeclared id in a wire → it's auto-created as a `|rect|`.

**The file *is* the root container's body** — no wrapping block. The stylesheet
comes first (bare declarations configure the scene; rules and defines style it),
then the instances, then the wires:

```
layout: grid;  columns: repeat(3);  gap: 30;   // scene config — bare declarations

rect { radius: 6; }                             // a rule (stylesheet) — draws nothing
.hot { stroke-width: 2; }

server |rect| "Server"                          // an instance (canvas) — drawn
client |rect| "Client"
server -> client "requests"                     // a wire
```

---

## 1. Mental Model

A Lini file has three parts, **in order**: the **stylesheet**, the **canvas**,
then the **wires**. Every statement is one of four kinds — **scan the left
edge:**

| Starts with | Kind | Drawn? |
|---|---|---|
| `\|type\|`, an **id**, or a `"label"` | an **instance** (node) | yes — on the canvas |
| a bare **type name**, `.class`, `name::base`, or `--var` | a **rule** (stylesheet / define) | no — it styles |
| `key: value;` | a **declaration** (configures its container) | — |
| `… -> …` | a **wire** | yes |

That one split — **bars = canvas, bare names = stylesheet** — removes every "is
this drawn or just styled?" ambiguity.

**The file is the root container.** There is no defs block and no `|scene|`.
Bare declarations at the top set the root container's own properties (`layout`,
`gap`, `padding`, `fill`, `font-size`, …); inheritable ones (`font-*`, `color`)
cascade to every node.

**The order is fixed** — detailed in [§3](#3-statements), and nested the same
inside any body. It keeps the parser single-pass and reads as "configure, draw,
connect"; deliberately strict for v1, relaxable later without breaking files.

**Render order is source order; the cascade is whole-file.** Instances draw in
the order written (later on top; `layer:` overrides), and every rule applies to
every instance. Wires are the one thing that needs no declaration: naming an id
declared nowhere auto-creates it (see [§3](#3-statements)).

**Two kinds of variable.**

- *Visual* values that don't affect layout — colours, the font family, the
  shadow tint — are exposed as live CSS variables (`--lini-fill`,
  `--lini-accent`, …) so a host page can re-theme them at runtime.
- *Layout* values — sizes, gaps, paddings, thicknesses, **and font size** —
  bake into the SVG as literals. Text is measured at compile time, so its size
  can never be a runtime `var()`; a standalone SVG always looks right.

See [Variables & Defaults](#11-variables--defaults).

---

## 2. Lexical Syntax

| Property | Value |
|---|---|
| Extension | `.lini` |
| Encoding | UTF-8 (BOM ignored) |
| Line endings | LF or CRLF (normalized on read) |
| Comments | `// …` to end of line. No block comments. |
| Statement end | newline or `;` |
| Identifier | `[a-zA-Z_][a-zA-Z0-9_-]*` — case-sensitive, ASCII, dash-case |

Whitespace is insignificant except as a token separator and where a rule below
says otherwise:

| Form | Whitespace rule |
|---|---|
| `key: value` | `:` separates name and value; surrounding space is optional, canonical is one space after (`radius: 5`). |
| `name::base` | The define operator; surrounding space optional like `:` (canonical tight: `treat::rect`). |
| `\|…\|` | Opening and closing `\|` paired; whitespace allowed inside, not at an ident boundary. |
| `.name` (class) | **Space required before** when it follows an ident or `\|` (`box .hot`). `box.hot` parses as a wire endpoint dot-path. |
| `id.side` | **No space**, wire endpoints only (`cat.right`). |
| `--name` | A variable, in a value or at a statement start to declare one. |

`:` (single) always begins a declaration value; `::` always begins a define.
The two never collide, and neither depends on whitespace.

**Strings** — double-quoted UTF-8. Escapes: `\"`, `\\`, `\n`, `\t`. Single
quotes are not strings.

**Numbers** — integer or decimal, optional sign, no units (px for lengths,
degrees for angles, 0–1 for opacities/fractions). `10`, `-5`, `0.25`, `+3`.

**Values are space-separated and positional**, like CSS: `padding: 5 2 5 5`,
`shadow: 2 2 4 #0003`, `at: 100 50`, `columns: 80 140 80`. A **comma** separates
list items and appears only where a property takes a list of groups (`points:
0 0, 10 10`). **Functions** use parentheses: `rgb(…)`, `hsl(…)`, `repeat(…)`.

**Colors** — `#fff`, `#ffaa00`, `#ffaa00cc` (alpha), CSS names (`red`,
`cornflowerblue`), `rgb(…)`, `rgba(…)`, `hsl(…)`, a `--name` variable reference,
or `none`. Out-of-range channels are an error.

---

## 3. Statements

Statements come in three phases — **stylesheet → canvas → wires** ([§1](#1-mental-model)).
The stylesheet holds variable declarations, root configuration, rules, and
`name::base` defines (each type defined before its first use); the canvas holds
instances; then come the wires. A body nests the same order: declarations, then
child nodes, then internal wires.

### The three type forms

| Form | Kind | Means |
|---|---|---|
| `\|rect\|` | instance | draw a rect (bars = canvas) |
| `rect { … }` | rule | style every rect (a CSS element selector) |
| `treat::rect { … }` | define | a new type `treat`, base `rect`, with its defaults |

A bare type name can only be a rule because **type names are reserved** — `rect`
can never be a node id. `treat::rect` reads "treat **is a** rect" (and the `::`
sets it apart from a `key: value` declaration at a glance). Defines chain
(`panel::treat`) and may carry intrinsic children. Max inheritance depth 16;
cycles are an error.

### Node declaration

```
[id] [|type|] ["label" …] [.class …] [ { block } ]
```

Everything is optional; the type defaults to `rect`. The block holds, in this
order, declarations, then child nodes, then internal wires.

```
db |cyl| "Postgres" .primary {
  fill: #eef;
  badge |rect| "v16" { mount: on; side: top; align: end; }
}
```

| Form | Effect |
|---|---|
| `cat` | `\|rect\|`, label "cat". |
| `cat \|treat\|` | Type `treat`, label "cat". |
| `cat "Friendly cat"` | `\|rect\|`, label "Friendly cat". |
| `cat \|treat\| ""` | Type `treat`, **no** label. |
| `cat .bold .loud { padding: 5; }` | Type + classes + own declarations. |
| `garden \|group\| { … }` | Container with a body. |
| `\|text\| "Title"` | Anonymous primitive (can't be wired to). |

- **Labels are positional strings** — a closed shape stacks them as centred text;
  a `|group|`'s 1st and 2nd become a top **caption** and bottom **footer**
  (`|caption|` children, [§8](#8-templates)), the rest centred; `""` suppresses one.
- A `link:` declaration (not a positional string) makes a node clickable.

Multi-line labels use `\n`; the text box sizes to the widest line, spacing is
`font-size × line-height`.

### Implicit nodes

A root wire's single-segment endpoint naming an id declared nowhere in the file
auto-creates an empty `|rect|` at the scene root with the id as its label — so
`cat -> dog -> bird` is a complete three-box diagram. Declaring the id anywhere
— before or after the wire — prevents auto-creation. If the id exists only
deeper in the tree, nothing is created: the wire must use the full path, and the
error suggests it. Body wires never auto-create.

### Declarations

A declaration `key: value;` configures the statement it sits in — the root (at
file top), a node (in its block), a wire, or a wire label. Property names are
dash-case; values are space-separated and positional. See
[Properties](#10-properties).

A declaration is itself a statement, so it binds to a node or wire only **inside
that one's block** — an inline `key: value` on the line would read as a separate
root declaration. So: the line is *identity* (id, type, labels, classes); the
block is *configuration + content*. The root is the exception — its block is the
whole file, so its declarations are the bare top-level statements.

---

## 4. Selectors & the Cascade

A **rule** is `selector { declarations }`. Selectors are CSS-shaped:

```
rect { … }                  // every rect (element selector)
.hot { … }                  // every node with class .hot (class selector)
table rect { … }            // every rect inside a table (descendant)
.sidebar rect { … }         // every rect inside a .sidebar
```

In a selector the type is **bare** — `table rect`, never `table |rect|` —
because bars are only for instances. A descendant selector is two or more parts
separated by whitespace; it matches a node whose ancestor chain contains each
part in order (not necessarily adjacent), exactly like a CSS descendant
combinator.

A **define** introduces a new type from a base: `treat::rect { … }`. Its
declarations are the type's defaults; an optional body gives it intrinsic
children (materialized per instance — see [§9](#9-wires)).

**Classes** are defined by a `.name { … }` rule and **applied** by writing
`.name` on a node (`box .hot`). Each selector part is a single element *or* a
single class; compound parts (`.card.hot`, `rect.hot`) are not supported.

**Specificity** — the most specific source wins; ties break by **source order**
(the CSS cascade):

1. **Type rule** (`rect { }`) and a type's own define defaults
2. **Descendant rule** (`table rect { }`, `.sidebar rect { }`)
3. **Class** (`.hot { }`)
4. **The instance's own block** (`client |rect| { fill: white; }`) — wins

For a wire: `-> { }` defaults → descendant/class rules → the wire's own
declarations.

Complex values (`at: x y`, `padding: t r b l`) replace wholesale — the merge is
per-property, not deep.

---

## 5. Layout

A container picks a mode with `layout`:

| Value | Behavior |
|---|---|
| `layout: row` | 1D horizontal flex. |
| `layout: column` | 1D vertical flex. |
| `layout: grid` | 2D grid — sized by `columns` / `rows`. |

**Defaults:** every container — the root included — defaults to `layout: column`
with `gap: 20`. A normal container pads its content by 16; the root pads by 0
(its margin is the fixed `canvas-pad`, 20 px), as do the frameless `|row|` /
`|column|` (see [§8](#8-templates)).

### Flex — `align` / `justify`

Flexbox model: `justify` runs *along* the flow (main axis), `align` runs
*across* it (cross axis). Both default `center`.

| Value | `justify` (main axis) | `align` (cross axis) |
|---|---|---|
| `start` / `center` / `end` | pack at the edge / centre / opposite | align each child to the edge / centre / opposite |
| `stretch` | (fills children to span the main axis) | each child's **box** fills the cross axis |
| `evenly` | equal gaps between and around children | (treated as `center`) |

`stretch` fills the child's **box**, not its *content* (placed by the child's own
`align`/`justify`, also `center`) — so a stretched table cell fills its track and
centres its text for free. `evenly` needs multiple children.

All of `align`/`justify`/`stretch`/`evenly` are **no-ops unless the container is
larger than its packed children** — an auto-sized container has no slack to
distribute. Slack comes from an explicit `width`/`height`, or from a grid's
fixed tracks.

### Grid — `columns` / `rows` / `cell` / `span`

A grid is sized by its track lists:

| Property | Notes |
|---|---|
| `columns` | **Required.** A track list — `columns: 80 140 80` (3 fixed) or `columns: repeat(3)` (3 auto) or a mix (`auto 40 auto`). The grid's column count is the list length. |
| `rows` | Optional. Same track-list form. Omitted → rows are implicit and auto-sized, the count `⌈children / columns⌉`. |
| `cell` | Child placement `column row`, 1-indexed (`cell: 2 1`). |
| `span` | Child span `columns rows`, default `1 1` (`span: 2` = `2 1`). |

A **track** is a size (`80`), `auto` (sized to its widest/tallest child), or
`repeat(N)` / `repeat(N, size)` for many equal tracks (`repeat(N)` = N auto
tracks; `repeat(N, 80)` = N tracks of 80). The count comes from the list length.

**Auto-flow.** Cells without `cell:` flow into the tracks left-to-right, wrapping
at the column count; a `cell:` pins one explicitly and the rest flow around it.
A child fills its track only under `align`/`justify: stretch` — a plain grid
leaves children at their natural size, centred in each cell.

`columns`/`rows`/`cell`/`span` are valid only on a grid (`layout: grid` or
`|table|`); using them elsewhere is an error.

### Dividers

`divider` draws separators between flow children, painted by the container's
`stroke` / `stroke-width` / `stroke-style`:

| Value | Effect |
|---|---|
| `none` (default) | no separators |
| `all` | every **interior** separator — in 1-D, between children; in a grid, between rows and columns |
| `rows` / `columns` | grid only — separators along that axis |

Dividers are **interior only** — the outer frame is just the container's own
border (its `stroke`), so a frameless grid (`stroke: none`) shows only inner
lines and a bordered one is never doubled. `divider` is span-aware in grids (a
separator never crosses a spanning cell's interior, and a shared edge is never
drawn twice) and skips mounted children. This is what lets `|table|` be plain
`grid + divider: all` rather than a magic type (see [§8](#8-templates)).

### Container properties

| Property | Applies to | Notes |
|---|---|---|
| `layout` | all | `row`, `column`, `grid`. |
| `gap` | all | Space between children. `N` = both axes; `row col` (CSS order) per axis. Negative allowed. |
| `padding` | all | Inner padding. `N`, `v h`, or `t r b l`. |
| `align` / `justify` | all | Cross / main axis (above). |
| `columns` / `rows` | grid | Track lists (above). |
| `divider` | all | Separators (above). |
| `fill` | all | Body colour; on the root it is the **canvas** colour. |

---

## 6. Positioning & Anchors

A shape's **bounding box** is the smallest axis-aligned rectangle containing it,
stroke included.

1. **Center origin.** Every bbox is centered at the parent's origin by default;
   `at: x y` puts the center at (x, y).
2. **Source order = render order;** later draws on top. `layer: N` overrides;
   ties break by source order.
3. **Strokes count** toward the bbox — `width: 100 height: 50 stroke-width: 4`
   → 104×54.
4. **`|path|`** is the only center-origin exception — `path:` uses native
   top-left coordinates.
5. **Rotation** applies last as an SVG transform; the rotated bounding rectangle
   propagates upward.

### Positioning a child

**`mount` is the switch.** It says how a child relates to its parent; any value
but `none` takes it out of the flow — one model, no compound anchor names:

- **`mount: none`** *(default)* — a normal flow/grid child. `side`/`align`
  don't apply.
- **`mount: in | out | on`** — anchor to an edge. **`side: top | bottom | left |
  right`** picks it (default `top`) and **`align: start | center | end`** slides
  along it, so a corner needs no special name (`side: top align: end` =
  top-right; `start`/`end` are the low/high ends — left/top is `start`). The
  meeting is *size-aware* — the child clears the edge by its own extent at any
  size:
  - **`in`** — flush inside, and **reserves a band**: flow content shifts to
    clear it and the box grows (top/bottom only; left/right fall back to top).
    The band is separated from the content by the container's own `gap`. This is
    a group's caption; tighten or loosen it with `margin:`.
  - **`out`** — flush *outside the border*: the band sits a container `gap`
    beyond the drawn frame, and the footprint grows to reserve it. The border
    keeps hugging the content while the caption rides just outside it. Top/bottom
    only. With no border or padding (the root, `|row|`/`|column|`) it coincides
    with `in`.
  - **`on`** — centred on the edge/corner (a corner anchor straddles it); an
    **overlay**, no reserve.

  Default `align: center`; a bare `side:` with no `mount:` is an error.

**`at: x y`** is the orthogonal escape hatch — bbox center at explicit
parent-local coords (`at: 0 0` centers). It's an overlay and overrides `mount`.

**Reserve vs overlay is the whole rule.** `mount: in`/`out` *reserve* — the
parent grows to hold the band, so the child never overlaps. `mount: on` and
`at: x y` are *overlays*: positioned against the parent but they **don't grow
it** (a parent of only overlays collapses to `2 × padding`). An overlay still
draws, and the canvas always includes it (never clipped).

**`offset: x y`** nudges from `at:` or `side:` — a pure render-time shift of the
one child.

**`margin:`** is signed *outer* spacing on any child — `N` / `v h` / `t r b l`,
like `padding` but negatives allowed. It changes the room the child reserves:
positive spreads it from siblings and grows the parent; negative eats the
surrounding gap and padding, tightening the parent (and, past zero, overlapping —
nothing clips). Unlike `offset:`, it reshapes the surrounding layout.

### Auto-sizing

`width` and `height` default to **`auto`** — the bbox sizes to its content (text
or child nodes) **plus `padding` on each side** — the one padding knob (default
16; there is no separate text padding). Sizing is **border-box**: an
explicit `width` / `height` is the exact outer dimension with padding *inside* it
(never added on top), and the two are independent (set one, the other stays
`auto`). A shape with no in-flow content — empty, or holding only `at:` /
`mount: on` overlays — is therefore **`2 × padding`** on each axis, so the
default `padding` (16) sets an empty box's minimum size (32 × 32).

Exceptions: a `|text|` sizes to its glyphs (no padding); `|icon|` defaults to
`icon-size` (24); `|line|` / `|poly|` / `|image|` / `|path|` require their
geometry (`points` / `src` / `path`) and error without it.

Text width uses an approximate metric (≈ 0.55 em per character) until embedded
font metrics land (see [§19](#19-deferred--non-goals)); setting `font-family`
restyles without re-measuring.

---

## 7. Primitives

13 primitives. All accept position and visual properties; closed shapes also
accept `stack`, `rotate`, `shadow`.

**Dimensions** use `width` / `height`, each defaulting to `auto` (content +
padding, **border-box** — see [§6](#6-positioning--anchors)). They are always
**bbox dimensions**: `|oval| width: 60 height: 40` is an ellipse in a 60×40 box;
equal dimensions (or an empty `|oval|`) make a circle.

| Primitive | Required | Notes |
|---|---|---|
| `\|rect\|` | size (auto) | Rounded via `radius:`. |
| `\|oval\|` | size (auto) | Bbox ellipse; equal width/height = circle. |
| `\|hex\|` | size (auto) | Regular hex, flat top/bottom. |
| `\|slant\|` | size (auto) | Parallelogram; top edge shifted `tan(skew) × h`. `skew` in degrees, (-89, 89). |
| `\|cyl\|` | size (auto) | Cylinder; end ellipses ≈ h/10. |
| `\|diamond\|` | size (auto) | Rhombus inscribed in the bbox. |
| `\|cloud\|` | size (auto) | Cloud path scaled to fit. |
| `\|poly\|` | `points` | ≥3 points, local (center-origin) coords. Closed. |
| `\|path\|` | `path` | Raw SVG path. **Native top-left coords.** |
| `\|text\|` | label string | See [label sugar](#3-statements) and [text properties](#10-properties). |
| `\|line\|` | `points` | 2+ points. Markers via `marker*:`. |
| `\|icon\|` | label (glyph name) | Material Symbols; the glyph name is the label (`\|icon\| "home"`). `icon-variant`, size via `width`/`height`. |
| `\|image\|` | `src`, `width`, `height` | `<image href="…">`. External URLs only; both dimensions required. |

### Visual modifiers (closed shapes)

| Property | Forms | Effect |
|---|---|---|
| `stroke-style` | `solid` / `dashed` / `dotted` | Stroke pattern. Default `solid`. (`wavy` deferred — [§19](#19-deferred--non-goals).) |
| `stack` | `N` / `dx dy` | Draw an offset duplicate behind the shape. Scalar `N` = `N -N`. |
| `rotate` | `N` degrees | Rotate around the bbox center. |
| `shadow` | `N` / `dx dy` / `dx dy blur` / `dx dy blur color` | Drop shadow via SVG `<filter>`. Scalar `N` = offset `N N`, blur `N`; tint defaults to `--lini-shadow`. |

### Markers (on `|line|` and wires)

| Property | Effect |
|---|---|
| `marker: X` | Both ends. |
| `marker-start: X` | Start end (wire source). |
| `marker-end: X` | End end (wire target). |

Values: `none`, `arrow`, `dot`, `diamond`, `crow`. Markers scale with
`stroke-width`, floor 5 px; color follows the stroke. `|line|` is bare by default
— write `|line| { marker-end: arrow; }` for a one-shot arrow. For wires the operator
picks markers (see [§9](#9-wires)). Source order wins: `marker: arrow
marker-end: dot` → start arrow, end dot.

---

## 8. Templates

Built-in types — each a bundle over a primitive base, named because the pattern
is common.

| Template | Base | Defaults | For |
|---|---|---|---|
| `\|group\|` | `\|rect\|` | `stroke: --group-stroke; fill: --group-fill; radius: 6` | Frame + caption/footer (padding via the default 16). |
| `\|caption\|` | `\|text\|` | `mount: in; font-size: 13` | A group's caption/footer band. |
| `\|badge\|` | `\|rect\|` | `mount: on; side: top; align: end; radius: 999; padding: 2 8; shadow: 2; fill: --accent; color: --on-accent; font-size: 11; layer: 10` | Corner pill (straddles the corner, grows nothing). |
| `\|note\|` | `\|rect\|` | `radius: 2; shadow: 2; stroke: none; fill: --note-bg` | Sticky note (padding via the default 16). |
| `\|row\|` | `\|rect\|` | `layout: row; fill: none; stroke: none; padding: 0` | Frameless wrapper — children in a row. |
| `\|column\|` | `\|rect\|` | `layout: column; fill: none; stroke: none; padding: 0` | Frameless wrapper — children in a column. |
| `\|table\|` | `\|group\|` | `layout: grid; divider: all; padding: 0; fill: none; stroke: --stroke` | Ruled grid (see below). |

**Captions.** A group's positional labels desugar to `|caption|` children: the
1st a top caption, the 2nd a bottom footer (`side: bottom`), any beyond the 2nd
plain centred `|text|`. A caption is just a `|text|` with `mount: in` — write one
by hand (`|caption| "X"` or a `|text| mount: …`) for anything else. Style every
caption globally with `caption { font-size: 16; font-weight: bold; }`; that
targets captions without touching body text (which `group text { }` would catch).

**Tables.** A `|table|` is sugar — a `group` that is a grid and draws dividers.
Two rules ship with it:

```
table::group { layout: grid; divider: all; stroke: --stroke; fill: none; padding: 0; }
table rect  { stroke-width: 0; padding: 4 8; align: stretch; justify: stretch; }
```

So the outer frame is the group border and the inner lines are `divider: all`,
both painted by its `stroke*`; cells are borderless (the shipped `stretch` rule
fills each cell, its text centring for free), so no edge is ever doubled. There
is no `|cell|` type — a cell is the default `|rect|`, auto-flowing into the
tracks:

```
basket |table| {
  columns: 80 140 80;
  rows: auto 40;

  "Fruit" { font-weight: bold; }  "Qty" { font-weight: bold; }  "Notes"
  "Apple"                         "12"                          "fresh"
  "Mango"                         "3"                           "ripe"
}
```

`fmt` aligns the cells into visual columns, so the flat form reads like a table.
Style cells in bulk with `.my-table rect { … }`.

Extend any template: `panel::group { stroke: --accent; }`. Common shapes need no
template:

| For | Write |
|---|---|
| Circle | `\|oval\| { width: 40; }` |
| Database | `\|cyl\|` |
| Arrow | `\|line\| { marker-end: arrow; points: 0 0, 50 0; }` |

---

## 9. Wires

Wires connect scene-node ids with an operator (`a -> b`); a wire is never written
as a `|wire|` instance.

Defaults for every wire — `stroke`, `stroke-width`, `stroke-style`, `clearance`,
`marker*` — come from a **`-> { }`** rule: the wire glyph is the element selector
for the routing layer. `clearance` additionally inherits from the root.

### Operators

A wire op is `[start_marker?][line][end_marker?]`, no spaces:

| Part | Tokens |
|---|---|
| Line | `-` solid · `--` dashed · `..` dotted · `~` wavy |
| Start markers | `<` arrow · `>` crow · `*` dot · `<>` diamond |
| End markers | `>` arrow · `<` crow · `*` dot · `<>` diamond |

The same glyph differs by position (`<` is arrow at the start, crow at the end).
The dotted line is `..` — two dots, distinct from the single dot of an endpoint
path (`a.b`).

| Op | Markers / Line |
|---|---|
| `->` `<-` `<->` | arrow combinations, solid |
| `-*` `*-` `*-*` | dot combinations |
| `-<>` `<>-<>` | diamond |
| `-<` `>-<` | crow |
| `-->` `..>` `~>` | dashed / dotted / wavy |
| `-` `--` `..` `~` | no markers (each line style) |

If the operator carries no markers, there are none on both ends. Explicit
`marker:` / `marker-start:` / `marker-end:` override the operator (source order
wins). The operator's line part sets the wire's `stroke-style` (`--` ⇒ `dashed`,
`..` ⇒ `dotted`, `~` ⇒ `wavy`); an explicit `stroke-style:` overrides it.

### Syntax

```
endpoints op endpoints [op endpoints …] ["label" …] [.class …] [{ declarations & |text| children }]
```

`endpoints` is one or more endpoints joined by `&`:

```
a -> b               // 1 wire
a -> b -> c          // chain: 2 wires
a -> b & c           // fan-out: a→b, a→c
a & b -> c           // fan-in
a & b -> c & d       // cartesian: 4 wires
a -> b -> c & d      // chain + fan
```

Mixing operators in one chain is a parse error. A wire body may contain only
`|text|` children.

### Endpoints & scope

```
endpoint = ident { "." ident } [ "." side ]
side     = top | bottom | left | right
```

Every wire resolves in a **scope** — the scene root for top-level wires, the
container's body for wires written inside one. An endpoint is a path walked from
that scope: the first segment names a node in the scope, each further segment a
child of the previous, a final segment matching a side name forces that side.
**There is no search**: a name not reachable this way is an error, and the error
suggests full paths of same-named nodes —
`wire endpoint 'bowl' not found at scene root; did you mean 'kitchen.counter.bowl'?`

| Endpoint (root wire) | Resolves to |
|---|---|
| `cat` | root node `cat` |
| `kitchen.counter.bowl` | exactly that path |
| `kitchen.counter.bowl.left` | the same node, left side forced |

Bodies are **sealed**: a body wire connects nodes of its own subtree only.
Cross-container wires are written at the lowest level where both ends are visible
— usually the root. Without a side the router picks edges by geometry; with a
side, that edge is forced.

### Labels

`a -> b "x" "y"` expands to `a -> b { |text| "x"; |text| "y" }` — each inline
string is a wire label. Labels ride the wire — there is no `mount`:

| Property | Notes |
|---|---|
| `at` | `0..1` along the route; unset = auto-distribute across the hops, so one label avoids junctions and several spread out. |
| `offset` | `x y` in the route's tangent frame (`x` along the wire, `y` to its left) — lifts the label off the line. |

A label is an obstacle to nothing, and may slide along the wire to keep clear of
nodes and other labels; the wire never moves for it. Wire labels default to
`font-size: 12`.

```
cat.right -> kitchen.bowl.left {
  |text| "watches" { at: 0.5; font-size: 10; }
  |text| "note" { offset: 0 -8; }
}
```

### Internal wires in defines

A define's body may wire its own children; ids are local and materialize per
instance — the same sealed-body rule. From outside, the dot-path navigates in:

```
room::group {
  layout: column;  gap: 10;
  inlet  |rect| "Inlet"
  outlet |rect| "Outlet"
  inlet -> outlet "flows"
}

garden  |room| "Garden"
kitchen |room| "Kitchen"
garden.outlet -> kitchen.inlet "carries"
```

### Routing

Wires route **orthogonally** — horizontal and vertical runs through the free
space between nodes, corners rounded. The router picks entry/exit sides unless an
explicit `.side` forces one. `clearance` (above; default 16) is the minimum gap
every wire keeps from nodes and from other wires.

The full routing contract — clearance, spacing, crossings, fan-out, self-loops —
lives in [`WIRING.md`](WIRING.md), the source of truth for routing.

---

## 10. Properties

Every property is `name: value;`. Dash-case names; positional, space-separated
values.

### Paint

| Property | Type | Default |
|---|---|---|
| `fill` | color | `--fill` (closed shapes); `currentColor` on `\|text\|`; `--stroke` for icons; **the canvas** on the root (default transparent) |
| `color` | color | inherits — sets text/icon glyph colour for descendants; on `\|text\|`, an alias for `fill` |
| `opacity` | 0..1 | 1 |
| `radius` | number | 0 (`\|rect\|` only) |
| `rotate` | degrees | 0 |
| `skew` | degrees | 15 (`\|slant\|` only) |
| `shadow` | `N` / `dx dy` / `dx dy blur` / `dx dy blur color` | off |
| `stack` | `N` / `dx dy` | off (shapes only) |

`color` cascades through the SVG via native `currentColor`: set it on a container
to recolour every descendant `|text|` that doesn't override. Use `color` for
*labels*, `fill` for *bodies*.

### Stroke

| Property | Type | Default |
|---|---|---|
| `stroke` | color | `--stroke` (the outline/line/wire colour) |
| `stroke-width` | number | 1 |
| `stroke-style` | `solid` / `dashed` / `dotted` | `solid` |

### Geometry & placement

| Property | Type | Notes |
|---|---|---|
| `width`, `height` | number / `auto` | bbox dims, **border-box** (padding inside, not added); default `auto` = content + padding. `\|image\|` needs both. |
| `at` | `x y` | bbox center at parent-local coords; an overlay, overrides `mount`. (On a wire label, `at: 0.5` = route fraction.) |
| `mount` | `none` / `in` / `out` / `on` | The flow switch ([§6](#6-positioning--anchors)). |
| `side` | `top` / `bottom` / `left` / `right` | Which edge `mount` meets (default `top`); a bare `side:` with no `mount:` is an error. |
| `align` | `start`/`center`/`end` (+ `stretch`/`evenly` on a container) | Mounted child: slide along its edge (default `center`). Container: cross-axis alignment ([§5](#5-layout)). |
| `offset` | `x y` | Nudge from `at:` / `side:`. |
| `margin` | `N` / `v h` / `t r b l` | Signed outer spacing; negatives tighten ([§6](#6-positioning--anchors)). |
| `layer` | integer | Render order (ties break on source order). |
| `points` | `x y, x y, …` | Vertex list (`\|poly\|`, `\|line\|`). |
| `path` | string | Raw SVG path (`\|path\|`, native top-left coords). |

(`align` on a plain flow child has no effect — the container's `align` governs;
`justify` is its container-only main-axis partner; multi-line text uses
`text-align`.)

### Spacing & layout

`padding`, `margin`, `gap`, `layout`, `align`, `justify`, `columns`, `rows`,
`cell`, `span`, `divider` — see [Layout](#5-layout) and [Positioning](#6-positioning--anchors).
Longhands `padding-top`/`-right`/`-bottom`/`-left` (and the `margin-*` set) are
accepted.

### Text

| Property | Default | Notes |
|---|---|---|
| `font-family` | `--font-family` | ident, string, or `--var`. |
| `font-size` | 14 (body), 13 (caption), 12 (wire label) | px; a baked layout constant. |
| `font-weight` | `normal` | `normal` / `bold`. |
| `font-style` | `normal` | `normal` / `italic` / `oblique`. |
| `text-align` | `center` | `start` / `center` / `end` — multi-line justification (`left`/`right` = start/end). |
| `line-height` | 1.2 | baseline-to-baseline multiple; a single line's box stays snug. |
| `letter-spacing` | 0 | feeds width measurement. |

`font-family`, `font-size`, `font-weight`, `font-style`, `text-align`,
`line-height`, `letter-spacing`, and `color` cascade to descendant `|text|` —
nearest ancestor wins, like CSS. `width`/`height` on a `|text|` are an error
(`use 'font-size'`).

### Markers & routing

`marker`, `marker-start`, `marker-end` ([§7](#7-primitives)); `clearance`
([§9](#9-wires) — set on `-> {}` or the root, inherits to every wire).

### Media & accessibility

| Property | Notes |
|---|---|
| `src` | image source (`\|image\|`). |
| `link` | wraps this node or wire in `<a href>` — clickable. |
| `icon-variant` | `outlined` / `filled` / `rounded` / `sharp`. |
| `title` | emits a `<title>` child (tooltip + screen-reader name). |

### Variables

`--name: value;` declares a themeable variable; `--name` in a value references
one. Visual variables stay live `var()`; layout values bake. See
[Variables & Defaults](#11-variables--defaults).

---

## 11. Variables & Defaults

CSS variables are for **visual theming only** — colours, the font family, the
shadow tint. Everything that affects layout — including font *size* — is a baked
constant, so a standalone SVG never depends on host CSS.

### 11.1 Visual variables (live, themeable)

```
--lini-bg            white
--lini-fg            black
--lini-fill          white
--lini-stroke        #444
--lini-accent        #0a84ff
--lini-on-accent     white
--lini-muted         #888
--lini-danger        crimson
--lini-warn          orange
--lini-airwire       crimson
--lini-note-bg       #fff9c4
--lini-group-stroke  #bbb
--lini-group-fill    rgba(0, 0, 0, 0.03)
--lini-font-family   sans-serif
--lini-text-color    var(--lini-fg)
--lini-shadow        rgba(0, 0, 0, 0.2)
```

These emit as live `var(--lini-*)` references, and the compiler ships an `@layer
lini.defaults` block alongside the SVG — so unlayered host CSS wins
automatically, no `!important`.

### 11.2 `--name` references

`--name` is the **visual-variable namespace, and only that**. `--name: value;`
declares one (a built-in `--lini-*` name keeps its meaning; a new name is yours),
and `--name` in a value references it, emitting `var(--lini-name)`:

```
--brand: #ff6600;
cat |rect| { fill: --brand; }
```

Alias a host var from CSS: `.lini { --lini-accent: var(--my-brand-blue); }`.

Layout values — sizes, gaps, padding, `font-size`, `clearance` — are **not**
`--name` variables: they aren't themeable, so there is nothing to reference or
re-theme at runtime. Set them with properties and rules instead (`gap: 30;`,
`rect { radius: 4; }`, `font-size: 16;` at the root).

### 11.3 Layout constants (baked)

Baked compile-time defaults — override per-node, on `-> { }` / the root, in
rules, or in an instance block:

```
font-size 14    wire-font-size 12   caption-font-size 13
stroke-width 1  radius 0            gap 20                 padding 16
clearance 16    icon-size 24        canvas-pad 20
```

`font-size` is body text. Wire labels and captions carry their own baked
defaults (12 and 13); a global `font-size:` at the root sets body text and
cascades, `-> { font-size: … }` sets wire labels, and `caption { font-size: …
}` sets captions.

Padding defaults to 16, with the frameless `|row|` / `|column|`, the root, and
the table container at 0, and table cells at `4 8`. It doubles as the minimum
size of an empty shape (`2 × padding`; see [Auto-sizing](#6-positioning--anchors)).
**Every baked default — these constants and the template bundles — lives in one
place**, so the whole look is tuned from a single file.

### 11.4 `--bake-vars`

Class rules and inline `style=` work everywhere, but CSS *variables* don't —
resvg and librsvg fail `var()` in every position (browsers, even `<img>`-embedded,
are fine). `--bake-vars` keeps the rules but inlines every `var(--lini-name)` as
its literal: no runtime theming, but a self-contained SVG that renders anywhere.

---

## 12. Specificity

Properties on a node merge like CSS — **the more specific source wins**, ties
broken by **later wins** (source order):

1. **Type cascade** — walked from the base primitive up to the node's declared
   type, layering each type's element-rule (`rect { }`) and define defaults. A
   more-derived type overrides what it builds on.
2. **Descendant rules** — `table rect { }`, `.sidebar rect { }`, matched against
   the ancestor chain.
3. **Class rules** — `.hot { }`, applied via `.hot` on the node.
4. **The instance's own block** — `client |rect| { fill: white; }` — the most
   specific, beats everything above.

For a wire: `-> { }` defaults → descendant/class rules → the wire's own
declarations.

Complex values (`at: x y`, `padding: t r b l`) replace wholesale — the merge is
per-property, not deep. `at:` always beats `cell:`.

---

## 13. SVG Output

```svg
<svg xmlns="http://www.w3.org/2000/svg"
     viewBox="X Y W H" width="W" height="H" class="lini">
  <style>
    @layer lini.defaults { :root, .lini { /* --lini-* variables */ } }
    .lini { font-family: var(--lini-font-family); font-size: 14px; color: var(--lini-text-color); }
    .lini .lini-shape-rect { fill: var(--lini-fill); stroke: var(--lini-stroke); stroke-width: 1; }
    .lini .lini-style-hot { stroke-width: 2; }   /* one rule per class def */
    .lini .lini-wire { stroke: var(--lini-stroke); stroke-width: 1; fill: none; }
  </style>
  <defs><!-- filters, clipPaths, icon symbols --></defs>
  <g class="lini-scene"> <!-- scene tree --> </g>
  <g class="lini-wires"> <!-- wires --> </g>
</svg>
```

`viewBox` auto-sizes to content + a 20 px canvas pad. A root `fill:` paints a
backing rect over the viewBox.

**Paint compiles to CSS; geometry bakes.** Shape and wire paint defaults — and
every rule — are stated once as class rules; only the classes actually used are
emitted. A node whose resolved paint differs from those rules carries the
difference as an inline `style="…"` (inline beats class, mirroring
[Specificity](#12-specificity)). Geometry — sizes, positions, radii, points,
paths, transforms — is always baked into attributes. Inherited text properties
(`font-family`, `font-size`, `font-weight`, `color`) state on `.lini` and cascade
natively; a node's own text property emits on its `<g>` and inherits to its
subtree.

**Node:**

```svg
<g class="lini-node lini-shape-{type} lini-shape-{base} lini-style-{class}"
   data-id="ID" transform="translate(X,Y)">
  <title>…</title>            <!-- when `title:` is set -->
  <!-- geometry, then children -->
</g>
```

Auto-classes: `lini-node` (every node); `lini-shape-{name}` (the type and every
type it inherits); `lini-style-{name}` (per applied class). With rotation, the
transform becomes `translate(X,Y) rotate(N)`.

**Wire:**

```svg
<g class="lini-wire lini-style-{class}" data-from="A" data-to="B">
  <path d="…" fill="none" stroke="…"/>
  <polygon class="lini-marker lini-marker-arrow" …/>
</g>
```

Host CSS may restyle any `lini-`-prefixed class; layout is computed at compile
time, so runtime restyling (a fatter `stroke-width`) restyles without re-layout.

---

## 14. CLI

```
lini [options] <input.lini>
lini fmt [--check] [--stdout] <input.lini>
lini desugar <input.lini>
lini serve [--port N] [--bake-vars] <input.lini>
```

| Flag | Meaning |
|---|---|
| `-o FILE` | Output path (default stdout). |
| `--format svg\|html` | `svg` (default) or HTML wrapper. |
| `--check` | Parse + resolve only — layout/render errors still surface on a full compile. |
| `--theme FILE` | CSS file of `--lini-*` overrides. |
| `--no-warn` / `--strict` | Silence warnings / treat them as errors. |
| `--bake-vars` | Inline `var()`s as literals (for non-browser renderers). |
| `--watch` | Recompile on every input change (requires `-o`). |
| `-h`, `-V` | Help / version. |

`lini -` reads stdin (filename `<stdin>` in errors). **`lini serve`** runs a
local live-reloading preview (default port 7700).

**`lini fmt`** reformats to canonical style — 2-space indent, `key: value;`
declarations, column-aligned id/type/label and table cells within a block,
comments and blank lines preserved. `--check` exits 1 if it would change
anything; `--stdout` writes instead of rewriting.

**`lini desugar`** prints the file with its sugar expanded — positional labels
and inline wire labels become the explicit `|text|` / `|caption|` children they
stand for — while types, variables, and properties stay as written. A
teaching/debugging view; prints to stdout, never rewrites, comments not
preserved.

Exit codes: 0 success · 1 parse/resolution error or `--check` reformat needed · 2
I/O · 3 invalid CLI.

---

## 15. Errors

Format: `filename:line:col: error: <message>` (LSP-compatible).

| Condition | Message |
|---|---|
| Duplicate id | `duplicate id 'X' (previously at L:C)` |
| Unknown endpoint | `wire endpoint 'X' not found at <scope>` + `; did you mean 'A', 'B'?` |
| Chain mixes operators | `wire chain mixes operators 'X' and 'Y'` |
| Chain < 2 nodes | `wire requires at least two endpoints` |
| Unknown type / class | `unknown type 'X'` / `unknown class '.X'` |
| Inheritance cycle / depth | `cycle in 'X' → … → 'X'` / `'X' exceeds max inheritance depth (16)` |
| Define shadows builtin | `'X' shadows a built-in type` |
| Missing required property | `'\|line\|' requires 'points'` |
| Unknown property | `unknown property 'foo' on '\|rect\|'` (warning) |
| `width`/`height` on text | `'width' is not a text property; use 'font-size'` |
| Wire body non-text | `wire body may only contain \|text\| children` |
| Wire label anchor | `wire labels accept only 'at' (0..1) and 'offset'` |
| Invalid / out-of-range color | `invalid color 'XYZ'` / `rgb(300,0,0): component out of range` |
| Reserved identifier | `'rect' is reserved (ids are case-sensitive — 'Rect' is free)` |
| Empty statement | `a node needs an id, type, label, or block` |
| `\|wire\|` as instance | `wires are drawn by operators, not the '\|wire\|' type` |
| Grid out of range | `cell: 5 _ exceeds columns=3` |
| Grid props off a grid | `'cell' is valid only on a grid` |
| Missing `columns` | `'layout: grid' requires 'columns'` |
| `skew` out of range | `skew: N must be in (-89, 89)` |

---

## 16. Grammar (EBNF)

```
file        = stylesheet instances wires           # the three phases, in order
stylesheet  = { vardecl | decl | rule | wire_rule | define | comment | newline }
instances   = { node | comment | newline }
wires       = { wire | comment | newline }
decl        = ident ":" values end
vardecl     = css_var ":" values end               # --name : value
rule        = selector block                       # selector parts are bare type/class names
wire_rule   = "->" block                           # wire defaults — the wire glyph as selector
define      = ident "::" ident block               # name :: base
node        = [ ident ] [ "|" ident "|" ] { string } { "." ident } [ block ]
                                                   # ≥1 of id / |type| / label / block
wire        = endpoints wire_op endpoints { wire_op endpoints }
              { string } { "." ident } [ wire_block ]
selector    = sel_part { sel_part }                # whitespace-separated = descendant
sel_part    = ident | "." ident
endpoints   = endpoint { "&" endpoint }
endpoint    = ident { "." ident } [ "." side ]
side        = "top" | "bottom" | "left" | "right"
block       = "{" { decl } { node } { wire } "}"   # body: declarations, then children, then internal wires
wire_block  = "{" { decl | text_decl | comment | newline } "}"
text_decl   = "|text|" string { "." ident } [ "{" { decl } "}" ] end

values      = value_group { "," value_group }      # comma only between list items
value_group = value { value }                      # space-separated scalars
value       = number | string | hex | ident | call | css_var
call        = ident "(" [ value { "," value } ] ")"
css_var     = "--" ident { "-" ident }

wire_op     = [ marker ] line [ marker ]
line        = "-" | "--" | ".." | "~"
marker      = "<" | ">" | "*" | "<>"

ident       = ( letter | "_" ) { letter | digit | "_" | "-" }
number      = [ "+" | "-" ] ( digit+ [ "." digit+ ] | "." digit+ )
string      = '"' { char | escape } '"'
escape      = "\" ( '"' | "\" | "n" | "t" )
comment     = "//" { not-newline } newline
end         = newline | ";"
```

**Single-pass LL(1).** The three-phase order (stylesheet → instances → wires)
plus the rule that **a type is defined before it is used** make one token of
lookahead enough. Leading tokens decide every form: `--name :` → variable;
`.name` → class rule; `ident ::` → define; `ident :` → declaration; two bare
idents → descendant rule; a leading `->` then `{` → the wire-defaults rule;
`|type|` or a leading string → instance; an `ident` followed by a wire-op / `&`
/ glued `.side` → wire. The lone `ident { … }` is a
**rule** when `ident` is a known type (built-in or already-defined) and a
**node** otherwise — and because types are defined first, the type set is always
complete at that point. No prescan, no second pass.

---

## 17. Implementer Algorithm

A reference pipeline; implementations may differ if the observable output matches.

**Parse.** Lex to tokens, then a single recursive-descent pass to the AST — the
ordering contract (§16) keeps the type set complete as types are first defined,
so `ident { … }` resolves rule-vs-node with one token of lookahead, no prescan.

**Resolve** (top-to-bottom):

1. *Variables & rules:* merge visual-var defaults ← `--theme` ← `--name: value`;
   register element/descendant/class rules and defines (detect cycles / depth >
   16); validate selectors reference known types.
2. *Scene tree:* resolve each node's type and classes; layer properties per
   [Specificity](#12-specificity) (type cascade → descendant rules → class rules
   → instance block); expand defines, scoping internal ids; expand label sugar
   (group captions → `|caption|`); build the path index; auto-create root rects
   for single-segment root-wire endpoints absent from it.
3. *Wires:* resolve endpoints by scoped path walk with suggestion errors; merge
   wire properties; cartesian-expand fan groups into one resolved wire per pair;
   the operator's line sets `stroke-style` unless overridden.

**Layout** (bottom-up): leaf bbox from `width`/`height` or defaults (+
half-`stroke-width` per side); arrange flow children per `layout` honouring
`align`/`justify`/`stretch`/`evenly` when there is slack; place mounted bands
(`mount: in`/`out`) and overlays (`mount: on`/`at:`); compute dividers; apply
`padding`; `rotate` last.

**Route wires.** Per [`WIRING.md`](WIRING.md) — orthogonal, clearance-respecting,
deterministic. Place markers (sized `max(5, stroke-width × 4)`, tip on the
endpoint) and wire labels at their `at`/`offset` anchors.

**Render.** Depth-first emit SVG per [SVG Output](#13-svg-output).

---

## 18. Reserved Words

Type names cannot be node ids — that is what makes `rect { }` a rule, not a
node. The four sides are reserved too (they are peeled from endpoint paths). Ids
are case-sensitive, so a capitalized variant is always free (`Start`, `Rect`).

- **Primitives:** `rect`, `oval`, `line`, `path`, `poly`, `text`, `hex`, `slant`,
  `cyl`, `diamond`, `cloud`, `icon`, `image`.
- **Templates:** `group`, `caption`, `badge`, `note`, `row`, `column`, `table`.
- **Sides:** `top`, `bottom`, `left`, `right`.
- **Reserved for the future:** `wire` and `circle`. `wire` is not an
  instantiable type (`|wire|` is an error) and is not a usable id; wire defaults
  are set with the `-> { … }` rule, not a `wire` keyword. `circle` is reserved
  too (today a circle is `|oval|` with equal or unset dimensions).

Value keywords are **contextual**, not reserved as ids — `grid`, `start`,
`center`, `end`, `stretch`, `evenly`, `none`, `in`, `out`, `on`, `auto`, `true`,
`false` mean their keyword only after the property that expects them
(`layout: grid`, `align: stretch`). Function names `rgb`, `rgba`, `hsl`, `repeat`
are reserved only before `(`.

---

## 19. Deferred & Non-Goals

**Deferred** — named in the language, not built yet; the syntax is stable:

- `stroke-style: wavy` rendering on shapes.
- `radius` on non-rect shapes (hex / diamond / slant / poly).
- numeric `font-weight` (`100…900`).
- `|icon|` Material Symbols glyph embedding (currently a placeholder square).
- embedded font metrics (text sizing is approximate until then).
- `aria-label`, and a "did you mean" property-name hint table.

**Non-goals** — out of scope; the syntax stays forward-compatible:

- **Auto-layout** — you position nodes (flex / grid / anchors); Lini does not
  place or route them for you.
- **Multi-file imports.**
- **Animation**, and interactivity beyond `link:` (`<a href>`).
- **Manual wire waypoints.**
- **Cross-instance addressing** into a define's internals — internal wires and
  dot-path reads work, but an external wire cannot reach into and restructure
  another instance's private nodes.

---

## 20. Examples

```
layout: grid;  columns: repeat(3);  gap: 40;  padding: 20;
fill: --bg;

-> { stroke: #666; stroke-width: 1; clearance: 6; }
rect { radius: 4; }                          // every rect rounds

--accent: #0a84ff;

.thin { stroke: #444; }
.bold { font-weight: bold; }
.loud { stroke: red; stroke-width: 2; }

treat::rect  { radius: 5; }
nest::slant  { fill: gray; }
alert::oval  { stroke: red; width: 36; height: 36; }   // a circle

room::group {
  layout: column;  gap: 8;
  inlet  |rect| "Inlet"
  outlet |rect| "Outlet"
  inlet -> outlet "flows"
}

cat |oval| "Cat — patient hunter" { cell: 1 1; }

kitchen |group| "Kitchen" {
  cell: 2 1;  layout: column;  gap: 20;
  counter |group| "Counter" {
    layout: row;  gap: 10;
    bowl  |treat| "Bowl of oats"
    water |nest|  "Water"
  }
}

garden |group| "Garden" {
  cell: 3 1;  layout: column;  gap: 20;
  den |group| "Den" {
    layout: row;  gap: 15;
    rabbit |alert| "Rabbit" { badge |badge| "FAST" }
    carrot |rect|  "Carrot patch" { stack: 4; width: 80; height: 40; fill: white; }
  }
}

closet |room| "Closet" { cell: 1 2; }
fridge |room| "Fridge" { cell: 2 2; }

// wires — full paths from the wire's scope (here: the root)
cat.right -> kitchen.counter.bowl.left -> kitchen.counter.water
kitchen.counter.water -> garden.den.rabbit -> garden.den.carrot .loud
cat <-> kitchen "watches"
closet.outlet -> fridge.inlet "restocks"
```

### Table + dimension line

```
basket |table| {
  columns: 80 140 80;  rows: auto 28;
  "Fruit" { font-weight: bold; }  "Qty" { font-weight: bold; }  "Notes"
  "Apple"                         "12"                          "fresh"
  "Mango"                         "3"                           "ripe"
}

dim |line| {
  points: 0 200, 300 200;
  marker: arrow;  color: #666;
}
```

### Mermaid-fast

```
cat -> dog -> bird     // 3 implicit rects, 2 wires
fox & owl -> mouse     // fan-in
frog ~> pond           // wavy arrow
fish --> bowl          // dashed arrow
```

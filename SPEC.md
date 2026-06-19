# Lini — Language Specification

A small, human-readable language for plain-text diagrams. Flex/grid layout,
composable shapes, CSS-driven theming — compiles to clean SVG.

**Lini reads like CSS.** `key: value;` declarations in `{ }` blocks, dash-case
property names, space-separated values, real selectors. Only a handful of
concepts stay Lini-specific — the ones CSS has no word for (the `|type|` sigil,
pinning, wire operators).

**Two node kinds, like HTML.** A **box** is a box (`|box|`, `|group|`, …); a
**string** is text *content* inside one. `"…"` is never a wrapped node — it is
the text, exactly as text sits inside an element on a web page.

This document is complete: an implementer can build a conforming engine from it
alone. Wire **routing** has its own contract — see [`WIRING.md`](WIRING.md).

---

## Table of Contents

**Language** — 1 [Mental Model](#1-mental-model) · 2 [Lexical Syntax](#2-lexical-syntax) ·
3 [Statements](#3-statements) · 4 [Selectors & the Cascade](#4-selectors--the-cascade) ·
5 [Layout](#5-layout) · 6 [Positioning & Anchors](#6-positioning--anchors) ·
7 [Shapes](#7-shapes) · 8 [Templates](#8-templates) · 9 [Wires](#9-wires)

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
| `\|type\|` | An instance — draw this type (`\|box\|`, `\|group\|`). **Bars = canvas.** |
| `"…"` | Text content — a label, a cell, a note. |
| `name { … }` | A rule — style every `name`. **Bare name = stylesheet.** |
| `.name` | A class — define it (`.hot { … }`) or apply it (`box .hot`). |
| `--name` | A themeable variable (`fill: --accent`). |
| `a -> b` | A wire. |

Three defaults make small diagrams trivial:

- Omit the type → `|box|`.
- Omit the text → the box's id is its label (`{ "" }` to suppress it).
- Name an undeclared id in a wire → it's auto-created as a `|box|`.

**The file *is* the root container's body** — no wrapping block. The stylesheet
comes first (bare declarations configure the scene; rules and defines style it),
then the instances, then the wires:

```
layout: grid;  columns: repeat(3);  gap: 30;   // scene config — bare declarations

box { radius: 6; }                              // a rule (stylesheet) — draws nothing
.hot { stroke-width: 2; }

server |box|                                    // an instance (canvas) — id is its label
client |box|
server -> client "requests"                     // a wire, with a label
```

---

## 1. Mental Model

A Lini file has three parts, **in order**: the **stylesheet**, the **canvas**,
then the **wires**. Every statement is one of these — **scan the left edge:**

| Starts with | Kind | Drawn? |
|---|---|---|
| `\|type\|` or an **id** | a **box** (shape node) | yes — on the canvas |
| `"…"` | **text** (content node) | yes |
| a bare **type name**, `.class`, `name::base`, or `--var` | a **rule** (stylesheet / define) | no — it styles |
| `key: value;` | a **declaration** (configures its container) | — |
| `… -> …` | a **wire** | yes |

That split — **bars or an id = a box; quotes = text; bare names = stylesheet** —
removes every "is this drawn or just styled?" ambiguity.

**Boxes and text.** A *box* has an id, a type, classes, a block, and children. A
*string* is bare text content — no id, no type, no children, no block. A string
inside a box's block is that box's text (centred when it is the only child); a
string on its own is a free-standing text node. To style or position text, put
it in a box (a `|plain|` is the minimal one) — exactly like styling a web page's
text by styling its element.

**The file is the root container.** There is no defs block. Bare declarations at
the top set the root's own properties (`layout`, `gap`, `padding`, `fill`,
`font-size`, …); inheritable ones (`font-*`, `color`) cascade to every node.

**The order is fixed** — detailed in [§3](#3-statements), and nested the same
inside any body. It keeps the parser single-pass and reads as "configure, draw,
connect"; deliberately strict for v1, relaxable later without breaking files.

**Render order is source order; the cascade is whole-file.** Instances draw in
the order written (later on top, pinned children above the flow; `layer:`
overrides), and every rule applies to every instance. Wires are the one thing
that needs no declaration: naming an id
declared nowhere auto-creates it (see [§3](#3-statements)).

**Two kinds of variable.**

- *Visual* values that don't affect layout — colours, the font family, the
  shadow tint — are exposed as live CSS variables (`--lini-fill`,
  `--lini-accent`, …) so a host page can re-theme them at runtime.
- *Layout* values — sizes, gaps, paddings, widths, **and font size** — bake into
  the SVG as literals. Text is measured at compile time, so its size can never
  be a runtime `var()`; a standalone SVG always looks right.

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
| `name::base` | The define operator; surrounding space optional like `:` (canonical tight: `treat::box`). |
| `\|…\|` | Opening and closing `\|` paired; whitespace allowed inside, not at an ident boundary. |
| `.name` (class) | **Space required before** it when following an ident or `\|` — `\|box\| .hot` applies a class; glued `a.hot` is a wire endpoint dot-path. |
| `id.side` | **No space**, wire endpoints only (`cat.right`). |
| `--name` | A variable, in a value or at a statement start to declare one. |
| wire op | `[marker?] line [marker?]`, glued, no internal space (`->`, `..>`, `<->`). |

`:` (single) always begins a declaration value; `::` always begins a define.
The two never collide, and neither depends on whitespace.

**Strings** — double-quoted UTF-8: `"…"`. Escapes: `\"`, `\\`, `\n`, `\t`. A
double-quoted string is always **text content**. Single quotes are **not**
strings (reserved, [§18](#18-reserved-words)).

**Numbers** — integer or decimal, optional sign, no units (px for lengths,
degrees for angles, 0–1 for opacities/fractions). `10`, `-5`, `0.25`, `+3`.

**Values are space-separated and positional**, like CSS: `padding: 5 2 5 5`,
`shadow: 2 2 4 #0003`, `translate: 10 -4`, `columns: 80 140 80`. A **comma** separates
list items and appears only where a property takes a list of groups (`points:
0 0, 10 10`). **Functions** use parentheses: `rgb(…)`, `hsl(…)`, `repeat(…)`.

**Colors** — `#fff`, `#ffaa00`, `#ffaa00cc` (alpha), CSS names (`red`,
`cornflowerblue`), `rgb(…)`, `rgba(…)`, `hsl(…)`, a `--name` variable reference,
or `none`. Out-of-range channels are an error.

---

## 3. Statements

Statements come in three phases — **stylesheet → canvas → wires**
([§1](#1-mental-model)). The stylesheet holds variable declarations, root
configuration, rules, and `name::base` defines (each type defined before its
first use); the canvas holds instances (boxes and text); then come the wires. A
body nests the same order: declarations, then children, then internal wires.

### The three type forms

| Form | Kind | Means |
|---|---|---|
| `\|box\|` | instance | draw a box (bars = canvas) |
| `box { … }` | rule | style every box (a CSS element selector) |
| `treat::box { … }` | define | a new type `treat`, base `box`, with its defaults |

A bare type name can only be a rule because **type names are reserved** — `box`
can never be a node id. `treat::box` reads "treat **is a** box" (and the `::`
sets it apart from a `key: value` declaration at a glance). Defines chain
(`panel::treat`) and may carry intrinsic children. Max inheritance depth 16;
cycles are an error.

### Box declaration

```
[id] [|type|] [.class …] [ { block } ]
```

Everything is optional; the type defaults to `box`. The **line is identity** —
id, type, classes — and the **block is content + configuration**: declarations,
then child nodes (boxes and text), then internal wires, in that order. **Text is
a child**, so inside a block its label comes *after* the declarations: `{ width:
60; "Bowl" }` is correct, `{ "Bowl"; width: 60 }` an error — no declaration may
follow a child.

A **block-less node may trail its label** instead of wrapping it — `api |box|
"API"` is exactly `api |box| { "API" }`, with the strings running to the line's
end (`a |box| "x" "y"` is two text children). Once a node opens a `{ }` block the
label lives inside it: `api |box| "API" { fill: red }` is an error.

```
db |cyl| .primary {
  fill: #eef;
  "Postgres"
  |badge| "v16"
}
```

| Form | Effect |
|---|---|
| `cat` | `\|box\|`, label "cat" (the id). |
| `cat \|treat\|` | type `treat`, label "cat". |
| `cat \|treat\| "Friendly cat"` | trailing label "Friendly cat" (no block). |
| `cat \|treat\| {}` | type `treat`, label "cat" (an empty `{}` changes nothing). |
| `cat \|treat\| { "Friendly cat" }` | same label, the explicit block form. |
| `cat \|box\| ""` | box, **no** label. |
| `cat .bold .loud { padding: 5; }` | type + classes + own declarations. |
| `garden \|group\| { … }` | container with a body. |
| `\|box\| "Load balancer"` | anonymous labelled box (can't be wired to). |

### id-as-label

A box's text is its **id** unless it is given a string (trailing or in the block):

| Label | Means |
|---|---|
| no string at all | the id (`cat` → "cat"; `cat |box| { fill: red }` → still "cat") |
| `"X"` or `{ "X" }` | "X" — the trailing and block forms are identical |
| `""` or `{ "" }` | empty: suppressed in flow, an empty cell in a grid ([§5](#5-layout)) |

So styling never costs you the label — only an explicit string overrides it. A
multi-word label needs no block: `lb |box| "Load balancer"`; an *anonymous*
labelled box needs no id either: `|box| "Load balancer"`.

### Text content

A string is a **text node**:

- In a box's block (or trailing a block-less box) it is that box's text — centred
  when it is the only in-flow child, else a flow child like any other (laid out
  by the box's `layout`).
- On its own it is a free-standing flow / canvas text node.
- Several strings are several text nodes — `"a" "b" "c"` is three, on one line or
  three (a string is self-delimiting, so no `;` is needed between them).
- An empty `""` is suppressed (adds no text) — except as a **grid cell**, where
  it is a real empty cell that holds its track ([§5](#5-layout)).
- Multi-line text uses `\n`; the box sizes to the widest line, spacing is
  `font-size × line-height`.

A string carries **no block** — text is content, not a box. To style or position
it, wrap it in a box (`|plain| { color: red; "X" }`) and set the property there;
text properties then inherit down ([§10](#10-properties)).

### Implicit nodes

A root wire's single-segment endpoint naming an id declared nowhere in the file
auto-creates an empty `|box|` at the scene root with the id as its label — so
`cat -> dog -> bird` is a complete three-box diagram. Declaring the id anywhere
— before or after the wire — prevents auto-creation. If the id exists only
deeper in the tree, nothing is created: the wire must use the full path, and the
error suggests it. Body wires never auto-create.

### Declarations

A declaration `key: value;` configures the statement it sits in — the root (at
file top), a box (in its block), or a wire. Property names are dash-case; values
are space-separated and positional. See [Properties](#10-properties).

A declaration is itself a statement, so it binds to a box only **inside that
box's block** — an inline `key: value` on the line would read as a separate root
declaration. The root is the exception: its block is the whole file, so its
declarations are the bare top-level statements.

---

## 4. Selectors & the Cascade

A **rule** is `selector { declarations }`. Selectors are CSS-shaped:

```
box { … }                   // every box (element selector)
.hot { … }                  // every node with class .hot (class selector)
table box { … }             // every box inside a table (descendant)
.sidebar box { … }          // every box inside a .sidebar
-> { … }                    // wire defaults (the wire glyph is the selector)
```

In a selector the type is **bare** — `table box`, never `table |box|` — because
bars are only for instances. A descendant selector is two or more parts
separated by whitespace; it matches a node whose ancestor chain contains each
part in order (not necessarily adjacent), exactly like a CSS descendant
combinator.

A **define** introduces a new type from a base: `treat::box { … }`. Its
declarations are the type's defaults; an optional body gives it intrinsic
children (materialized per instance — see [§9](#9-wires)).

**Classes** are defined by a `.name { … }` rule and **applied** by writing
`.name` on a node (`box .hot`). Each selector part is a single element *or* a
single class; compound parts (`.card.hot`, `box.hot`) are not supported.

**Specificity** — the most specific source wins; ties break by **source order**
(the CSS cascade):

1. **Type rule** (`box { }`) and a type's own define defaults
2. **Descendant rule** (`table box { }`, `.sidebar box { }`)
3. **Class** (`.hot { }`)
4. **The instance's own block** (`client |box| { fill: white; }`) — wins

For a wire: `-> { }` defaults → descendant/class rules → the wire's own
declarations.

Complex values (`translate: x y`, `padding: t r b l`) replace wholesale — the
merge is per-property, not deep.

---

## 5. Layout

A container picks a mode with `layout`:

| Value | Behavior |
|---|---|
| `layout: row` | 1D horizontal flex. |
| `layout: column` | 1D vertical flex. |
| `layout: grid` | 2D grid — sized by `columns` / `rows`. |

**Defaults:** every container — the root included — defaults to `layout: column`
with `gap: 20`. A normal container pads its content by 20; the root pads by 0
(the fixed `canvas-pad`, 20 px, frames the whole scene), as do the frameless `|plain|` /
`|row|` / `|column|` (see [§8](#8-templates)).

### Flex — `align` / `justify`

Flexbox model: `justify` runs *along* the flow (main axis), `align` runs
*across* it (cross axis). Both default `center`.

| Value | `justify` (main axis) | `align` (cross axis) |
|---|---|---|
| `start` / `center` / `end` | pack at the edge / centre / opposite | align each child to the edge / centre / opposite |
| `stretch` | fills children to span the main axis | each child's **box** fills the cross axis |
| `evenly` | equal gaps between and around children | (treated as `center`) |

`stretch` fills the child's **box**, not its *content* (placed by the child's own
`align`/`justify`, also `center`). `evenly` needs multiple children.

All of `align`/`justify`/`stretch`/`evenly` are **no-ops unless the container is
larger than its packed children** — an auto-sized container has no slack to
distribute. Slack comes from an explicit `width`/`height`, or from a grid's
fixed tracks.

### Grid — `columns` / `rows` / `cell` / `span`

A grid is sized by its track lists:

| Property | Notes |
|---|---|
| `columns` | **Required.** A track list — `columns: 80 140 80` (3 fixed) or `columns: repeat(3)` (3 auto) or a mix (`auto 40 auto`). The grid's column count is the list length. |
| `rows` | Optional. Same track-list form. A floor, not a cap: extra children flow into implicit auto rows. Omitted → all rows implicit, the count `⌈children / columns⌉`. |
| `cell` | A **box** child's placement `column row`, 1-indexed (`cell: 2 1`). |
| `span` | A **box** child's span `columns rows`, default `1 1` (`span: 2` = `2 1`). |

A **track** is a size (`80`), `auto` (sized to its widest/tallest child), or
`repeat(N)` / `repeat(N, size)` for many equal tracks (`repeat(N)` = N auto
tracks; `repeat(N, 80)` = N tracks of 80). The count comes from the list length.

**Auto-flow.** Children without `cell:` flow into the tracks left-to-right,
wrapping at the column count; a `cell:` pins one explicitly and the rest flow
around it. Bare-text cells (a table) are pure auto-flow — `cell:` / `span:`
apply to **box** children only (a text node has no block to carry them). A grid
is positional, so an empty `""` cell is **kept** — it holds its track and keeps
the cells after it aligned (in flow, by contrast, an empty `""` is dropped).

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

Dividers are **interior only** — the outer frame is the container's own border
(its `stroke`), so a frameless grid (`stroke: none`) shows only inner lines and a
bordered one is never doubled. `divider` is span-aware in grids (a separator
never crosses a spanning cell's interior, and a shared edge is never drawn twice)
and skips pinned children.

A container with `divider` other than `none` **requires `gap: 0`** (an error
otherwise): a separator wants the cells flush against it, not floating in a gap.
This is what lets `|table|` be plain `grid + divider: all + gap: 0` rather than a
magic type (see [§8](#8-templates)).

### Container properties

| Property | Applies to | Notes |
|---|---|---|
| `layout` | all | `row`, `column`, `grid`. |
| `gap` | all | Space between children. `N` = both axes; `row col` (CSS order) per axis. Must be `≥ 0`; `0` required with `divider`. |
| `padding` | all | Inner padding. `N`, `v h`, or `t r b l`. On a `\|table\|`, the inset around each cell's text. |
| `align` / `justify` | all | Cross / main axis (above). |
| `columns` / `rows` | grid | Track lists (above). |
| `divider` | all | Separators (above). |
| `fill` | all | Body colour; on the root it is the **canvas** colour. |

---

## 6. Positioning & Anchors

A shape's **bounding box** is the smallest axis-aligned rectangle containing it,
stroke included.

1. **Center origin.** Every bbox is centered at the parent's origin by default.
2. **Source order = render order;** later draws on top, with pinned children
   above the in-flow ones. `layer: N` overrides; ties break by source order.
3. **Strokes count** toward the bbox — `width: 100 height: 50 stroke-width: 4`
   → 104×54.
4. **`|path|`** is the only center-origin exception — `path:` uses native
   top-left coordinates.
5. **Rotation** applies last as an SVG transform; the rotated bounding rectangle
   propagates upward.

### `pin` — out of the flow

Every child is **in flow** by default — laid out by its container's `layout`
([§5](#5-layout)). **`pin` is the switch that lifts a child out**, aligning the
child's **matching point** flush with a named point of the parent:

| `pin:` | The child sits… |
|---|---|
| `none` *(default)* | — in flow; nothing is pinned |
| `center` | centre on the parent's centre |
| `top` · `bottom` · `left` · `right` | flush against that parent edge |
| `top left` · `top right` · `bottom left` · `bottom right` | with its corner on that parent corner |

The child's *own* matching point lands on the parent's, so it sits **flush**:
`pin: top left` puts the child's top-left corner on the parent's, `pin: top` sits
it flush under the top edge, `pin: center` is centre-to-centre. The anchor is the
parent's **drawn box** — its border and padding included. Corners fall out of the
value, so one switch covers every anchor: no compound knobs, no `side`/`align`.

A pinned child is an **overlay**. It **does not grow the parent** — a parent of
only pinned children collapses to `2 × padding` — and it **paints above** the
in-flow children, so a badge needs no explicit `layer`. The canvas always
includes it, so an overlay is never clipped. Set `layer:` to reorder overlapping
pins, or to push one *beneath* the flow.

### `translate` — the universal nudge

**`translate: x y`** shifts a node by (x, y) *after* it is placed. It works on
**every** node — flow children, pinned children, the root alike — and is
layout-neutral: siblings don't move, the parent doesn't grow, and no size
changes. It is CSS's standalone `translate`, baked into the node's origin (so a
standalone SVG needs no transform variable); the canvas still includes the
shifted node.

There is **no numeric coordinate property**. Because the parent's origin is its
center, `pin: center` + `translate: x y` lands a child's center at parent-local
(x, y) — explicit coordinates with no shape-size arithmetic: the named anchor
skips the math, `translate` does the pixel nudge.

Positioning is a box's job — only a box carries `pin` and `translate`. To position
a piece of text, wrap it in a `|plain|`.

### Auto-sizing

`width` and `height` default to **`auto`** — the bbox sizes to its content (text
or child nodes) **plus `padding` on each side** — the one padding knob (default
16; there is no separate text padding). Sizing is **border-box**: an explicit
`width` / `height` is the exact outer dimension with padding *inside* it (never
added on top), and the two are independent (set one, the other stays `auto`). A
box with no in-flow content — empty, or holding only `pin`ned overlays — is
therefore **`2 × padding`** on each axis, so the default `padding` (20) sets an
empty box's minimum size (40 × 40).

Exceptions: a **text** node sizes to its glyphs (no padding); `|icon|` defaults
to `icon-size` (24); `|line|` / `|poly|` / `|image|` / `|path|` require their
geometry (`points` / `src` / `path`) and error without it. `|plain|` carries
`padding: 0`, so a plain box sizes to its text exactly.

Text width uses one advance per character (≈ 0.6 em). The default font is
monospace, so this is essentially exact; a proportional `font-family` override
makes it approximate until embedded font metrics land (see
[§19](#19-deferred--non-goals)).

---

## 7. Shapes

12 shape primitives. All accept position and visual properties; closed shapes
also accept `stack`, `rotate`, `shadow`. Text is **not** a shape — it is bare
content ([§3](#3-statements)); the frameless `|plain|` box ([§8](#8-templates))
is what you reach for when text needs an id, a class, or a wire.

**Dimensions** use `width` / `height`, each defaulting to `auto` (content +
padding, **border-box** — see [§6](#6-positioning--anchors)). They are always
**bbox dimensions**: `|oval| width: 60 height: 40` is an ellipse in a 60×40 box;
equal dimensions (or an empty `|oval|`) make a circle.

| Primitive | Required | Notes |
|---|---|---|
| `\|box\|` | size (auto) | The default; rounded (`radius: 6`). `\|rect\|` for sharp corners. |
| `\|oval\|` | size (auto) | Bbox ellipse; equal width/height = circle. |
| `\|hex\|` | size (auto) | Regular hex, flat top/bottom. |
| `\|slant\|` | size (auto) | Parallelogram; top edge shifted `tan(skew) × h`. `skew` in degrees, (-89, 89). |
| `\|cyl\|` | size (auto) | Cylinder; end ellipses ≈ h/10. |
| `\|diamond\|` | size (auto) | Rhombus inscribed in the bbox. |
| `\|cloud\|` | size (auto) | Cloud path scaled to fit. |
| `\|poly\|` | `points` | ≥3 points, local (center-origin) coords. Closed. |
| `\|path\|` | `path` | Raw SVG path. **Native top-left coords.** |
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
— write `|line| { marker-end: arrow; }` for a one-shot arrow. For wires the
operator picks markers (see [§9](#9-wires)). Source order wins: `marker: arrow
marker-end: dot` → start arrow, end dot.

---

## 8. Templates

Built-in types — each a bundle over a shape base, named because the pattern is
common.

| Template | Base | Defaults | For |
|---|---|---|---|
| `\|plain\|` | `\|box\|` | `stroke: none; fill: none; padding: 0` | A frameless box — shows only its text, but is a real box (id, class, wirable). |
| `\|rect\|` | `\|box\|` | `radius: 0` | A sharp-cornered box (a plain `\|box\|` rounds to `radius: 6`). |
| `\|group\|` | `\|box\|` | `stroke: --group-stroke; stroke-style: dashed; stroke-width: 1; fill: --group-fill; radius: 6` | Dashed frame for a caption + children (padding via the default 20). |
| `\|caption\|` | `\|plain\|` | `pin: top left; translate: 0 -16; color: --caption-color; font-size: 12; font-weight: normal` | A title, pinned just above the group's top-left corner. |
| `\|footer\|` | `\|caption\|` | `pin: bottom left; translate: 0 16` | A caption flipped to the bottom edge. |
| `\|badge\|` | `\|box\|` | `pin: top right; radius: 999; padding: 2 8; shadow: 2; fill: --accent; color: --on-accent; font-size: 11` | Corner pill — tucks into the top-right corner, grows nothing. |
| `\|note\|` | `\|box\|` | `radius: 2; shadow: 2; stroke: none; fill: --note-bg` | Sticky note (padding via the default 20). |
| `\|row\|` | `\|plain\|` | `layout: row` | Frameless wrapper — children in a row. |
| `\|column\|` | `\|plain\|` | `layout: column` | Frameless wrapper — children in a column. |
| `\|table\|` | `\|group\|` | `layout: grid; divider: all; gap: 0; padding: 4 8; fill: none; stroke: --stroke; stroke-style: solid` | Ruled grid (see below). |

**Captions.** A `|caption|` is a small `|plain|` **pinned** just above the
group's top-left corner; a `|footer|` is the same flipped to the bottom. Both are
out-of-flow overlays, so they never push the content and their place is fixed by
the template, not by where they sit among the children:

```
panel |group| {
  |caption| "Settings"
  a |box| "General"
  b |box| "Network"
  |footer| "v2.1"
}
```

Style every caption globally with `caption { font-size: 16; font-weight: bold; }`
— that targets captions without touching body text. Because a caption is pinned
(not in flow), a group laid out as a `row` carries its title just the same.

**Tables.** A `|table|` is sugar — a `group` that is a grid, draws dividers, and
has `gap: 0`. Cells are **bare text** that auto-flows into the tracks; there is no
`|cell|` type and no per-cell styling — spacing comes from the track sizes
(`columns` / `rows`) and the table's `padding` (the inset around each cell's
text). The outer frame is the group border and the inner lines are `divider:
all`, both painted by the table's `stroke*`; no edge is ever doubled.

```
basket |table| {
  columns: 80 140 80;
  "Fruit" "Quantity" "Notes"
  "Apple" "12"       "fresh"
  "Mango" "3"        "ripe"
}
```

`fmt` knows the column count and pads the cells into aligned columns, so the flat
form reads like the table it is. A cell that must be styled, placed, or wired is
a **box** child (`|plain| { …; "X" }` or `|box| { … cell: 2 1; }`) — its stroke
will read against the dividers, which is exactly why bare text is the default.

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
endpoints op endpoints [op endpoints …] [.class …] [{ declarations & "labels" }]
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

Mixing operators in one chain is a parse error.

### Labels — the wire is a container

A wire is a 1-D container; its content is **bare-string labels**, distributed
along the route by `along:` — the wire's track rule, exactly as `columns:` is a
grid's:

| Property | Notes |
|---|---|
| `along` | A list of `0..1` fractions along the whole drawn route, one per label (`along: 0.2 0.5 0.8`). Omitted → auto-distribute across the hops, so one label avoids junctions and several spread out. |

Like a box, a **block-less wire may trail its labels** — `a -> b "watches"` is
`a -> b { "watches" }`, and the strings run to the line's end (`a -> b "x" "y"`).
Reach for the block only to pin them with `along:` or to style one (below):

```
a -> b "watches"                           // trailing, auto-placed
a -> b { along: 0.3 0.7; "near a" "near b" }
```

A label is an obstacle to nothing, and may slide along the wire to keep clear of
nodes and other labels; the wire never moves for it. Wire labels default to
`font-size: 11`. A label that needs its own style or a `translate` nudge (world
`x y`, the same as on any node) is a **box** (`|plain|`), which carries a block:

```
a -> b { |plain| { translate: 0 -8; "watches" } }
```

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

### Internal wires in defines

A define's body may wire its own children; ids are local and materialize per
instance — the same sealed-body rule. From outside, the dot-path navigates in:

```
room::group {
  layout: column;  gap: 10;
  inlet  |box| "Inlet"
  outlet |box| "Outlet"
  inlet -> outlet "flows"
}

garden  |room| "Garden"
kitchen |room| "Kitchen"
garden.outlet -> kitchen.inlet "carries"
```

### Routing

Wires route **orthogonally** — horizontal and vertical runs through the free
space between nodes, corners rounded. The router picks entry/exit sides unless an
explicit `.side` forces one. `clearance` (default 16) is the minimum gap every
wire keeps from nodes and from other wires.

The full routing contract — clearance, spacing, crossings, fan-out, self-loops —
lives in [`WIRING.md`](WIRING.md), the source of truth for routing.

---

## 10. Properties

Every property is `name: value;`. Dash-case names; positional, space-separated
values.

### Paint

| Property | Type | Default |
|---|---|---|
| `fill` | color | `--fill` (closed shapes); `currentColor` on text; `--stroke` for icons; **the canvas** on the root (default transparent) |
| `color` | color | inherits — sets text/icon glyph colour for descendants; on text, an alias for `fill` |
| `opacity` | 0..1 | 1 |
| `radius` | number | 6 (`\|box\|` only; `\|rect\|` is `0`) |
| `rotate` | degrees | 0 |
| `skew` | degrees | 15 (`\|slant\|` only) |
| `shadow` | `N` / `dx dy` / `dx dy blur` / `dx dy blur color` | off |
| `stack` | `N` / `dx dy` | off (closed shapes only) |

`color` cascades through the SVG via native `currentColor`: set it on a container
to recolour every descendant's text that doesn't override. Use `color` for
*labels*, `fill` for *bodies*.

### Stroke

| Property | Type | Default |
|---|---|---|
| `stroke` | color | `--stroke` (the outline/line/wire colour) |
| `stroke-width` | number | 2 (`\|group\|` is `1`) |
| `stroke-style` | `solid` / `dashed` / `dotted` | `solid` |

### Geometry & placement

| Property | Type | Notes |
|---|---|---|
| `width`, `height` | number / `auto` | bbox dims, **border-box** (padding inside, not added); default `auto` = content + padding. `\|image\|` needs both. |
| `pin` | `none` / `center` / `top` / `bottom` / `left` / `right` / `top left` / `top right` / `bottom left` / `bottom right` | Out-of-flow anchor — the child's center lands on the named parent point ([§6](#6-positioning--anchors)). |
| `translate` | `x y` | Post-placement nudge of the node and its subtree; no reflow, grows nothing ([§6](#6-positioning--anchors)). |
| `layer` | integer | Paint order; default 0 in flow, 1 when `pin`ned. Ties break on source order. |
| `points` | `x y, x y, …` | Vertex list (`\|poly\|`, `\|line\|`). |
| `path` | string | Raw SVG path (`\|path\|`, native top-left coords). |

Geometry and placement are **box** properties — a bare text node carries none of
them; wrap it in a `|plain|`. (`align` on a plain flow child has no effect — the
container's `align` governs; `justify` is its container-only main-axis partner;
multi-line text uses `text-align`.)

### Spacing & layout

`padding`, `gap`, `layout`, `align`, `justify`, `columns`, `rows`, `cell`,
`span`, `divider` — see [Layout](#5-layout) and [Positioning](#6-positioning--anchors).
Longhands `padding-top`/`-right`/`-bottom`/`-left` are accepted.

### Text

| Property | Default | Notes |
|---|---|---|
| `font-family` | `--font-family` | ident, string, or `--var`. |
| `font-size` | 15 (body), 12 (caption), 11 (wire label) | px; a baked layout constant. |
| `font-weight` | `--font-weight` (body `bold`; captions / wire labels `normal`) | `normal` / `bold`. |
| `font-style` | `normal` | `normal` / `italic` / `oblique`. |
| `text-align` | `center` | `start` / `center` / `end` — multi-line justification (`left`/`right` = start/end). |
| `line-height` | 1.2 | baseline-to-baseline multiple; a single line's box stays snug. |
| `letter-spacing` | 0 | feeds width measurement. |

These all **inherit** — nearest ancestor wins, like CSS. Because a string is not
a box, you never set a text property *on* the text; you set it on a containing
box (or the root) and it cascades down. Style globally with `font-size: …` at the
root, or scope it by setting the property on a container.

### Markers & routing

`marker`, `marker-start`, `marker-end` ([§7](#7-shapes)); `along` and `clearance`
([§9](#9-wires) — `clearance` set on `-> {}` or the root, inherits to every wire).

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
--lini-group-stroke  rgba(0, 0, 0, 0.4)
--lini-group-fill    rgba(0, 0, 0, 0.03)
--lini-caption-color rgba(0, 0, 0, 0.4)
--lini-font-family   ui-monospace, "SF Mono", "Cascadia Code", "JetBrains Mono", Menlo, Consolas, "Liberation Mono", monospace
--lini-font-weight         bold
--lini-caption-font-weight normal
--lini-wire-font-weight    normal
--lini-text-color    var(--lini-fg)
--lini-shadow        rgba(0, 0, 0, 0.2)
```

The default font is a **monospace** stack: it reads crisp, and a fixed glyph
advance keeps text-width estimation accurate without embedded font metrics
([§19](#19-deferred--non-goals)). Body text is **bold**, captions and wire labels
**normal**.

These emit as live `var(--lini-*)` references, and the compiler ships an `@layer
lini.defaults` block alongside the SVG — so unlayered host CSS wins
automatically, no `!important`.

### 11.2 `--name` references

`--name` is the **visual-variable namespace, and only that**. `--name: value;`
declares one (a built-in `--lini-*` name keeps its meaning; a new name is yours),
and `--name` in a value references it, emitting `var(--lini-name)`:

```
--brand: #ff6600;
cat |box| { fill: --brand; }
```

Alias a host var from CSS: `.lini { --lini-accent: var(--my-brand-blue); }`.

Layout values — sizes, gaps, padding, `font-size`, `clearance` — are **not**
`--name` variables: they aren't themeable, so there is nothing to reference or
re-theme at runtime. Set them with properties and rules instead (`gap: 30;`,
`box { radius: 4; }`, `font-size: 16;` at the root).

### 11.3 Layout constants (baked)

Baked compile-time defaults — override per-node, on `-> { }` / the root, in
rules, or in an instance block:

```
font-size 15    wire-font-size 11   caption-font-size 12
stroke-width 2  radius 6            gap 20                 padding 20
clearance 16    icon-size 24        canvas-pad 20
```

`font-size` is body text. Wire labels and captions carry their own baked
defaults (11 and 12); a global `font-size:` at the root sets body text and
cascades, `-> { font-size: … }` sets wire labels, and `caption { font-size: … }`
sets captions. `radius` rounds a `|box|` by default; `|rect|` resets it to 0.

Padding defaults to 20, with `|plain|` / `|row|` / `|column|` and the root at 0,
and a `|table|` at `4 8` (its cell inset). It doubles as the minimum size of an
empty box (`2 × padding`; see [Auto-sizing](#6-positioning--anchors)). **Every
baked default — these constants and the template bundles — lives in one place**,
so the whole look is tuned from a single file.

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
   type, layering each type's element-rule (`box { }`) and define defaults. A
   more-derived type overrides what it builds on.
2. **Descendant rules** — `table box { }`, `.sidebar box { }`, matched against
   the ancestor chain.
3. **Class rules** — `.hot { }`, applied via `.hot` on the node.
4. **The instance's own block** — `client |box| { fill: white; }` — the most
   specific, beats everything above.

For a wire: `-> { }` defaults → descendant/class rules → the wire's own
declarations.

Complex values (`translate: x y`, `padding: t r b l`) replace wholesale — the
merge is per-property, not deep. A `pin`ned child ignores `cell:` — pinning takes
it out of the grid.

---

## 13. SVG Output

```svg
<svg xmlns="http://www.w3.org/2000/svg"
     viewBox="X Y W H" width="W" height="H" class="lini">
  <style>
    @layer lini.defaults { :root, .lini { /* --lini-* variables */ } }
    .lini { font-family: var(--lini-font-family); font-size: 15px; font-weight: var(--lini-font-weight); color: var(--lini-text-color); }
    .lini .lini-shape-box { fill: var(--lini-fill); stroke: var(--lini-stroke); stroke-width: 2; }
    .lini .lini-style-hot { stroke-width: 3; }   /* one rule per class def */
    .lini .lini-wire { stroke: var(--lini-stroke); stroke-width: 2; fill: none; }
  </style>
  <defs><!-- filters, clipPaths, icon symbols --></defs>
  <rect class="lini-canvas" .../>   <!-- only when the root has a fill: -->
  <g class="lini-scene"> <!-- scene tree --> </g>
  <g class="lini-wires"> <!-- wires --> </g>
</svg>
```

`viewBox` auto-sizes to content + a 20 px canvas pad. A root `fill:` paints a
`lini-canvas` backing rect over the viewBox.

**Paint compiles to CSS; geometry bakes.** Shape and wire paint defaults — and
every rule — are stated once as class rules; only the classes actually used are
emitted. A node whose resolved paint differs from those rules carries the
difference as an inline `style="…"` (inline beats class, mirroring
[Specificity](#12-specificity)). Geometry — sizes, positions (`pin` and
`translate` fold into the baked origin), radii, points,
paths, transforms — is always baked into attributes. Inherited text properties
(`font-family`, `font-size`, `font-weight`, `color`) state on `.lini` and cascade
natively; a node's own text property emits on its `<g>` and inherits to its
subtree.

**Box:**

```svg
<g class="lini-node lini-shape-{type} lini-shape-{base} lini-style-{class}"
   data-id="ID" transform="translate(X,Y)">
  <title>…</title>            <!-- when `title:` is set -->
  <!-- geometry, then children -->
</g>
```

Auto-classes: `lini-node` (every box); `lini-shape-{name}` (the type and every
type it inherits); `lini-style-{name}` (per applied class). With rotation, the
transform becomes `translate(X,Y) rotate(N)`.

**Text** emits a bare `<text class="lini-text">…</text>` at its placed position —
no wrapping `<g>`, so a table of N cells is N `<text>` elements, not N boxes. Its
font and colour come by inheritance from the enclosing `<g>`.

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
declarations grouped on one line, a declarations-only block collapsed onto its
opening line when it fits, a lone label trailing the head (`api |box| "API"`),
one child node per line, table cells padded into aligned columns, comments and
blank lines preserved. `--check` exits 1 if it would change anything; `--stdout`
writes instead of rewriting.

**`lini desugar`** prints the file with its sugar expanded — an id-as-label or
trailing label gains its explicit `{ "…" }` block, and a wire's auto-distributed
labels gain their explicit `along:` — while types, variables, and properties stay
as written. A teaching/debugging view; prints to stdout, never rewrites, comments
not preserved.

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
| Unknown property | `unknown property 'foo' on '\|box\|'` (warning) |
| Text carries a block | `text content takes no block — wrap it in '\|plain\|' to style or position it` |
| Wire body non-label | `a wire body holds only labels and 'along:'` |
| Divider needs flush cells | `'divider' requires 'gap: 0'` |
| Invalid / out-of-range color | `invalid color 'XYZ'` / `rgb(300,0,0): component out of range` |
| Reserved identifier | `'box' is reserved (ids are case-sensitive — 'Box' is free)` |
| Empty statement | `a node needs an id, type, or block` |
| `\|wire\|` as instance | `wires are drawn by operators, not the '\|wire\|' type` |
| Grid out of range | `cell: 5 _ exceeds columns=3` |
| Grid props off a grid | `'cell' is valid only on a grid` |
| Missing `columns` | `'layout: grid' requires 'columns'` |
| Negative `gap` | `'gap' must be ≥ 0` |
| `skew` out of range | `skew: N must be in (-89, 89)` |
| Single-quoted string | `single quotes are not strings — use "…"` |
| Invalid `pin` value | `'pin' expects none, center, an edge (top/bottom/left/right), or a corner (e.g. 'top right')` |
| Declaration after a child | `declarations must come before children in a block` |
| Trailing label and a block | `a label is the trailing string or the block, not both` |

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

node        = box | text
box         = [ ident ] [ "|" ident "|" ] { "." ident } [ string { string } | block ]
                                                   # ≥1 of id / |type| / label / block; trailing labels XOR a block
text        = string                               # bare content; never a wrapped box

wire        = endpoints wire_op endpoints { wire_op endpoints }
              { "." ident } [ string { string } | wire_block ]
selector    = sel_part { sel_part }                # whitespace-separated = descendant
sel_part    = ident | "." ident
endpoints   = endpoint { "&" endpoint }
endpoint    = ident { "." ident } [ "." side ]
side        = "top" | "bottom" | "left" | "right"

block       = "{" { decl } { node } { wire } "}"   # declarations, then children (text is a child), then internal wires
wire_block  = "{" { decl | text | box | comment | newline } "}"   # labels (strings) + along:; a |plain| for a styled label

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
idents → descendant rule; a leading `->` then `{` → the wire-defaults rule; a
`string` → a text node; `|type|` → a box; an `ident` followed by a wire-op / `&`
/ glued `.side` → wire. A `string` is self-delimiting, so consecutive strings are
consecutive text nodes with no separator; strings *trailing a box or wire head*
are instead that node's labels, to the line's end. The lone `ident { … }` is a
**rule** when `ident` is a known type (built-in or already-defined) and a **box**
otherwise — and because types are defined first, the type set is always complete
at that point. No prescan, no second pass.

---

## 17. Implementer Algorithm

A reference pipeline; implementations may differ if the observable output matches.

**Parse.** Lex to tokens, then a single recursive-descent pass to the AST — the
ordering contract (§16) keeps the type set complete as types are first defined,
so `ident { … }` resolves rule-vs-box with one token of lookahead, no prescan.

**Resolve** (top-to-bottom):

1. *Variables & rules:* merge visual-var defaults ← `--theme` ← `--name: value`;
   register element/descendant/class rules, the `-> { }` wire defaults, and
   defines (detect cycles / depth > 16); validate selectors reference known
   types.
2. *Scene tree:* resolve each box's type and classes; layer properties per
   [Specificity](#12-specificity) (type cascade → descendant rules → class rules
   → instance block); expand defines, scoping internal ids; synthesize the
   id-as-label text for a box whose block has no string; build the path index;
   auto-create root boxes for single-segment root-wire endpoints absent from it.
3. *Wires:* resolve endpoints by scoped path walk with suggestion errors; merge
   wire properties; cartesian-expand fan groups into one resolved wire per pair;
   the operator's line sets `stroke-style` unless overridden.

**Layout** (bottom-up): leaf bbox from `width`/`height` or defaults (text → its
glyphs; box → content + `padding`; + half-`stroke-width` per side); arrange flow
children per `layout` honouring `align`/`justify`/`stretch`/`evenly` when there
is slack; pin out-of-flow children to their parent anchor (the parent never grows
for them); compute dividers; apply `padding`; apply each node's `translate`;
`rotate` last.

**Route wires.** Per [`WIRING.md`](WIRING.md) — orthogonal, clearance-respecting,
deterministic. Place markers (sized `max(5, stroke-width × 4)`, tip on the
endpoint) and wire labels at their `along:` fractions (auto-distributed when
unset).

**Render.** Depth-first emit SVG per [SVG Output](#13-svg-output): a box is a
`<g>`, a string is a `<text>`.

---

## 18. Reserved Words

Type names cannot be node ids — that is what makes `box { }` a rule, not a node.
The four sides are reserved too (they are peeled from endpoint paths). Ids are
case-sensitive, so a capitalized variant is always free (`Box`, `Top`).

- **Primitives:** `box`, `oval`, `line`, `path`, `poly`, `hex`, `slant`, `cyl`,
  `diamond`, `cloud`, `icon`, `image`.
- **Templates:** `plain`, `rect`, `group`, `caption`, `footer`, `badge`, `note`,
  `row`, `column`, `table`.
- **Sides:** `top`, `bottom`, `left`, `right`.
- **Reserved for the future:** `wire`, `text`, `circle`. None is an instantiable
  type or a usable id. `wire` — wire defaults are set with `-> { }`, not a `wire`
  keyword (`|wire|` is an error). `text` is a former name kept reserved (text is
  a bare `"…"`). `circle` — today a circle is `|oval|` with equal or unset
  dimensions.

Single quotes (`'`) are reserved and are not strings.

Value keywords are **contextual**, not reserved as ids — `grid`, `start`,
`center`, `end`, `stretch`, `evenly`, `none`, `auto`, `true`,
`false` mean their keyword only after the property that expects them
(`layout: grid`, `align: stretch`). Function names `rgb`, `rgba`, `hsl`, `repeat`
are reserved only before `(`.

---

## 19. Deferred & Non-Goals

**Deferred** — named in the language, not built yet; the syntax is stable:

- `stroke-style: wavy` rendering on shapes.
- `radius` on non-box shapes (hex / diamond / slant / poly).
- numeric `font-weight` (`100…900`).
- `|icon|` Material Symbols glyph embedding (currently a placeholder square).
- `text-transform` (`uppercase` / `lowercase` / `capitalize`).
- embedded font metrics — the monospace default keeps the estimate close; a
  proportional `font-family` override is approximate until then.
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

-> { stroke: #666; clearance: 12; }
box { radius: 4; }                           // round a touch less than the default 6

--accent: #0a84ff;

.thin { stroke: #444; }
.bold { font-weight: bold; }
.loud { stroke: red; stroke-width: 2; }

treat::box   { radius: 5; }
nest::slant  { fill: gray; }
alert::oval  { stroke: red; width: 36; height: 36; }   // a circle

room::group {
  layout: column;  gap: 8;
  inlet  |box| "Inlet"
  outlet |box| "Outlet"
  inlet -> outlet "flows"
}

cat |oval| { cell: 1 1; "Cat — patient hunter" }

kitchen |group| {
  cell: 2 1;  layout: column;  gap: 20;
  |caption| "Kitchen"
  counter |group| {
    layout: column;  gap: 10;
    |caption| "Counter"
    bowl  |treat| "Bowl of oats"
    water |nest|  "Water"
  }
}

garden |group| {
  cell: 3 1;  layout: column;  gap: 20;
  |caption| "Garden"
  den |group| {
    layout: column;  gap: 15;
    |caption| "Den"
    rabbit |alert| { |badge| "FAST" }
    carrot |box|   { stack: 4; width: 80; height: 40; fill: white; "Carrot patch" }
  }
}

closet |room| { cell: 1 2; "Closet" }
fridge |room| { cell: 2 2; "Fridge" }

// wires — full paths from the wire's scope (here: the root)
cat.right -> kitchen.counter.bowl.left -> kitchen.counter.water
kitchen.counter.water -> garden.den.rabbit -> garden.den.carrot .loud
cat <-> kitchen "watches"
closet.outlet -> fridge.inlet "restocks"
```

### Table + dimension line

```
basket |table| {
  columns: 80 140 80;
  "Fruit" "Quantity" "Notes"
  "Apple" "12"       "fresh"
  "Mango" "3"        "ripe"
}

dim |line| {
  points: 0 200, 300 200;
  marker: arrow;  color: #666;
}
```

### Mermaid-fast

```
cat -> dog -> bird     // 3 implicit boxes, 2 wires
fox & owl -> mouse     // fan-in
frog ~> pond           // wavy arrow
fish --> bowl          // dashed arrow
newt ..> log           // dotted arrow
```

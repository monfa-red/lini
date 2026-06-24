# Lini — Language Specification

A small, human-readable language for plain-text diagrams. Flex/grid layout,
composable shapes, CSS-driven theming — compiles to clean SVG.

**Two brackets carry the whole language.** `{ … }` is **style** — `key: value;`
declarations, dash-case, space-separated values, exactly like CSS. `[ … ]` is
**content** — a node's children, in source order. A node is
`id |type| .class { style } [ children ]`; every part is optional. Nothing styles
outside a `{ }`; nothing is drawn outside the canvas.

**Two node kinds, like HTML.** A **box** is a drawn shape (`|block|`, `|box|`,
`|oval|`, `|group|`, …) and may hold children; a **string** is text *content*
inside or beside one. `"…"` is the text, exactly as text sits inside an element on
a web page — now stylable in place (`"x" { color: red }`), but still a leaf, never
a box.

This document is complete: an implementer can build a conforming engine from it
alone. **Link** routing has its own contract — see [`LINKING.md`](LINKING.md).

---

## Table of Contents

**Language** — 1 [Mental Model](#1-mental-model) · 2 [Lexical Syntax](#2-lexical-syntax) ·
3 [Statements](#3-statements) · 4 [Selectors & the Cascade](#4-selectors--the-cascade) ·
5 [Layout](#5-layout) · 6 [Positioning & Anchors](#6-positioning--anchors) ·
7 [Shapes](#7-shapes) · 8 [Templates](#8-templates) · 9 [Links](#9-links)

**Reference** — 10 [Properties](#10-properties) · 11 [Colour, Variables & Defaults](#11-colour-variables--defaults) ·
12 [Specificity](#12-specificity) · 13 [SVG Output](#13-svg-output) · 14 [CLI](#14-cli) ·
15 [Errors](#15-errors) · 16 [Grammar](#16-grammar-ebnf) · 17 [Implementer Algorithm](#17-implementer-algorithm) ·
18 [Reserved Words](#18-reserved-words) · 19 [Deferred](#19-deferred) · 20 [Examples](#20-examples)

---

## Quickstart

```
cat -> dog -> bird
```

That's a complete diagram: three boxes, two links. Lini fills in the rest.

| Form | Means |
|---|---|
| `\|type\|` | A type — drawn as an **instance**, matched as a **rule**, extended as a **define**. Always in bars. |
| `"…"` | Text content — a label, a cell, a note. |
| `{ … }` | A **style block** — `key: value;` declarations. |
| `[ … ]` | A **child list** — a node's contents. |
| `.name` | A class — define it (`.hot { … }`), wear it (`\|box\| .hot`). |
| `--name` | A themeable variable (`fill: --accent`). |
| `a -> b` | A link. |

Three defaults make small diagrams trivial:

- Omit the type → `|box|` (a rounded, framed card).
- Omit the text → the box's id is its label (`""` to suppress it).
- Name an undeclared id in a link → it's auto-created as a `|box|`.

**A file has three parts, in order: the stylesheet, the canvas, then the links.**
The stylesheet is one `{ }` block at the top — setup that draws nothing. After
it come the instances, then the links:

```
{                                               // the stylesheet — setup only
  layout: grid;  columns: repeat(3);  gap: 30;  // scene config
  |box| { radius: 6; }                          // a rule — style every box
  .hot { stroke-width: 2; }                     // a class
}

server |box|                                    // the canvas — id is the label
client |box|
server -> client "requests"                     // a link, with a label
```

---

## 1. Mental Model

A Lini file is the body of an implicit **root** container, in three parts —
**stylesheet → canvas → links** — and every statement belongs to exactly one:

| Part | Holds | Drawn? |
|---|---|---|
| **stylesheet** | one `{ }` block: scene config (incl. link & routing defaults), rules, classes, defines | no — it styles |
| **canvas** | instances — boxes (`\|type\|` / an id) and text (`"…"`) | yes |
| **links** | `a -> b` connections | yes |

The "is this drawn or styled?" question never arises: **styling lives in the
stylesheet block; drawing lives on the canvas.** You never re-read a
`name { … }` to learn which it was.

**Two brackets, one meaning each.**

- `{ … }` — **style**: `key: value;` declarations. The *only* place styling
  lives. A node's own `{ }`, a rule's body, the scene config — all declarations.
- `[ … ]` — **content**: a node's children (boxes and text), then its
  internal links, in source order.

A drawn node is `id |type| .class { style } [ children ]`. Each part is optional,
but a node needs at least an id, a type, or a class; bare `cat` is a default
`|box|` labelled "cat".

**Two sigils, one meaning each.**

- `|…|` — a **type**. Always in bars, whether you instantiate it (`cat |oval|`),
  match it (`|oval| { … }`), or extend it (`|cat::oval| { … }`). On an instance the
  bars hold the type alone; as a rule they hold a CSS selector over types and
  classes (`|table box|`, `|.sidebar box|`); see [§4](#4-selectors--the-cascade).
- `.name` — a **class**. Defined bare (`.hot { … }`), worn after the type
  (`|box| .hot`) or after a link's endpoints (`a -> b .hot`) — a `.class` chain,
  never inside the bars.

**Boxes and text.** A *box* has an id, a type, classes, a style block, and
children. A *string* is text content — no id, type, classes, or children, but it
**may carry a style block** (`"x" { color: red; translate: 0 -6 }`). A string in a
box's `[ ]` (or trailing it) is that box's text (centred when it is the only
child); a string on its own is a free-standing text node. Text is a leaf: to give
it children, a border, padding, a `pin`, or a wirable id, put it in a box (a
`|block|` is the minimal one) — exactly like wrapping a web page's text in an
element.

**The file is the root container.** The stylesheet `{ }` is the root's own setup
block; the canvas instances are its children (written bare — the file *is* its
`[ ]`); the links are its internal links. Scene properties (`layout`, `gap`,
`padding`, `fill`, `font-size`, `link`, `routing`, …) sit in that block;
inheritable ones (`font-*`, `color`, `link`, `clearance`, `routing`) cascade to
every node and link.

**Render order is source order; the cascade is whole-file.** Instances draw in
the order written (later on top, pinned children above the flow; `layer:`
overrides), and every rule applies to every instance. Links are the one thing
that needs no declaration: naming an id declared nowhere auto-creates it
([§3](#3-statements)).

**Two kinds of variable.**

- *Visual* values that don't affect layout — colours and the font family — are
  exposed as live CSS variables (`--lini-fill`, `--lini-accent`, …) so a host page
  can re-theme them, and each colour carries a built-in dark variant that follows
  the viewer's OS or a `data-theme` toggle ([§11.1](#111-visual-variables-live-themeable)).
- *Layout* values — sizes, gaps, paddings, widths, **and font size** — bake into
  the SVG as literals. Text is measured at compile time, so its size can never
  be a runtime `var()`; a standalone SVG always looks right.

See [Colour, Variables & Defaults](#11-colour-variables--defaults).

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
| `key: value` | `:` separates name and value; surrounding space optional, canonical is one space after (`radius: 5`). |
| `\|…\|` | A type in bars. On an instance the bars hold a type alone (`oval`); in a rule selector a space is the descendant combinator (`table box`) and a part may be a class (`.sidebar box`); `::` is the define operator (`cat::oval`). Bars are paired; surrounding space at the boundary is not allowed. |
| `.name` (class) | At the stylesheet top it is a class **definition** (`.hot { … }`). On an instance or link it is a **worn class**, following the type or endpoints — **spaced** off an id/endpoint so it isn't a path (`cat .hot`, `a -> b .loud`), the rest of the chain **glued** (`.hot.loud`). |
| `id.side` / `id.child` | **No space** — a link endpoint path (`cat.right`, `kitchen.bowl`). |
| `--name` | A variable, in a value or at a statement start to declare one. |
| link op | `[marker?] line [marker?]`, glued, no internal space (`->`, `..>`, `<->`). |
| `[ … ]` | A child list. Paired; whitespace inside is insignificant. |

`:` (single) always begins a declaration value; `::` (inside bars) always begins
a define base. The two never collide, and neither depends on whitespace.

**Strings** — double-quoted UTF-8: `"…"`. Escapes: `\"`, `\\`, `\n`, `\t`. A
double-quoted string is always **text content**. Single quotes are **not**
strings (reserved, [§18](#18-reserved-words)).

**Numbers** — integer or decimal, optional sign, no units (px for lengths,
degrees for angles, 0–1 for opacities/fractions). `10`, `-5`, `0.25`, `+3`. A
trailing `%` makes a **percentage** (`50%`), valid only in colour components.

**Values are space-separated and positional**, like CSS: `padding: 5 2 5 5`,
`shadow: 2 2 4 #0003`, `translate: 10 -4`, `columns: 80 140 80`. A **comma**
separates list items and appears only where a property takes a list of groups
(`points: 0 0, 10 10`). **Functions** use parentheses: `rgb(…)`, `hsl(…)`,
`repeat(…)`.

**Colors** — `#fff`, `#f80c`, `#ffaa00`, `#ffaa00cc` (3/4/6/8 hex digits; the 4-
and 8-digit forms carry alpha), CSS names (`red`, `cornflowerblue`), `rgb(…)`,
`rgba(…)`, `hsl(…)`, `hsla(…)` (percentages allowed — `hsl(200, 50%, 50%)`), `oklch(L, C, H[, A])` (the palette's
own space — L/A in 0–1, C the chroma, H in degrees; folded to a hex at compile time,
so it renders in every target), a `--name` variable reference, or `none`.
Out-of-range channels are an error.
Beyond a flat colour, a **paint** (`fill` / `stroke` / `link`) may be a **gradient** —
`gradient(…)`, `linear-gradient(…)`, or `radial-gradient(…)` ([§11.3](#113-gradients));
the built-in hue palette ([§11.2](#112-the-colour-palette)) is reached through
ordinary `--name` references.

---

## 3. Statements

A file is **stylesheet → canvas → links** ([§1](#1-mental-model)), and a
container's body nests the same idea: a `{ }` style block, then a `[ ]` of
children and internal links.

### The stylesheet

One `{ }` block at the very top of the file — optional, omitted when there is
nothing to set up. Unlike an ordinary style block (declarations only), it is the
root's setup block, so it additionally holds the file-global definitions:

| Item | Form | Means |
|---|---|---|
| Scene config | `layout: grid;` | a declaration on the root |
| Link / routing defaults | `link: #666;` `routing: orthogonal;` | declarations that cascade to every link ([§9](#9-links)) |
| Variable | `--brand: #f60;` | a themeable variable |
| Rule | `\|box\| { … }` | style every box (a CSS element selector, in bars) |
| Descendant rule | `\|table box\| { … }` | style every box inside a table |
| Class | `.hot { … }` | define class `hot` |
| Define | `\|treat::box\| { … }` | a new type `treat`, base `box`, with its defaults |

```
{
  layout: column;  gap: 16;  fill: --bg;  link: #666;
  --brand: #ff6600;
  |box| { radius: 6; }
  .hot { stroke-width: 2; }
  |treat::box| { radius: 5; }
}
```

A bare type name is never a statement on its own — a type only ever appears in
bars. `|treat::box|` reads "treat **is a** box"; the `::` sets a define apart
from a plain reference (`|box|`) at a glance. Defines chain (`|panel::treat|`)
and may carry intrinsic children ([§9](#9-links)). Max inheritance depth 16;
cycles are an error.

### Box declaration

```
[id] [|type|] [.class…] [ { style } ] [ "label"… | [ children ] ]
```

The **line is identity** — id, `|type|`, and the `.class`es. The **`{ }` is
style**, the **`[ ]` is content** — children (boxes and text), then internal
links. A node leads with an id, a `|type|`, or a `.class` (any combination); the
style and content are optional.

A box's **type lives in the bars**, its **classes follow them**: `|oval|`,
`|box| .hot` (a box with class `hot`), `.hot` (a default box with class `hot`),
`|box| .hot.loud` (two classes). A bare `cat` is a default `|box|`.

A block-less node may **trail its label** instead of an `[ ]` — `api |box| "API"`
is `api |box| [ "API" ]`, the strings running to the line's end (`a |box| "x" "y"`
is two text children). The style block is independent of the label:
`api |box| { fill: red } "API"` sets the fill *and* the label. Content is the
trailing label **or** the `[ ]`, never both.

```
db |cyl| .primary { fill: #eef } [
  "Postgres"
  |badge| "v16"
]
```

| Form | Effect |
|---|---|
| `cat` | `\|box\|`, label "cat" (the id). |
| `cat \|treat\|` | type `treat`, label "cat". |
| `cat \|treat\| "Friendly cat"` | trailing label "Friendly cat". |
| `cat \|treat\| { fill: red }` | type + a style block, label still "cat". |
| `cat \|box\| ""` | box, **no** label. |
| `cat \|box\| .bold.loud { padding: 5 }` | type + classes + own style. |
| `garden \|group\| { … } [ … ]` | container with style and a body. |
| `\|box\| "Load balancer"` | anonymous labelled box (can't be linked to). |

### id-as-label

A box's text is its **id** unless it is given a string (trailing or in the `[ ]`):

| Label | Means |
|---|---|
| no string at all | the id (`cat` → "cat"; `cat \|box\| { fill: red }` → still "cat") |
| `"X"` | "X" — trailing or as a child in `[ ]`, identical |
| `""` | empty: suppressed in flow, an empty cell in a grid ([§5](#5-layout)) |

So styling never costs you the label — only an explicit string overrides it. A
multi-word label needs no `[ ]`: `lb |box| "Load balancer"`; an *anonymous*
labelled box needs no id either: `|box| "Load balancer"`.

### Text content

A string is a **text node** — always a `<text>` leaf, never wrapped:

- In a box's `[ ]` (or trailing a block-less box) it is that box's text — centred
  when it is the only in-flow child, else a flow child laid out by the box's
  `layout`.
- On its own (on the canvas, or in a `[ ]`) it is a free-standing flow / canvas
  text node.
- Several strings are several text nodes — `"a" "b" "c"` is three (a string is
  self-delimiting, so no `;` is needed between them).
- An empty `""` is suppressed (adds no text) — except as a **grid cell**, where
  it holds its track ([§5](#5-layout)).
- Multi-line text uses `\n`; the box sizes to the widest line, with a
  `font-size × 1.2` leading between lines (plus any `line-spacing`).

A string carries **no children** — text is a leaf, not a box — but it **may carry
a style block** of text properties: `"X" { color: red; font-weight: bold;
translate: 0 -6; rotate: 12 }`. Only text-valid properties apply (colour, every
`font-*`, `opacity`, `letter-spacing`, `line-spacing`, `text-transform`,
`text-decoration`, `translate`, `rotate`, `layer`, `href`, `title`); a box
property (`pin`, `padding`, `width`, a border, children) needs a real box —
wrap the text in a `|block|`. Set on the string the style applies to it directly;
set on a containing box it cascades down ([§10](#10-properties)).

### Implicit nodes

A root link's single-segment endpoint naming an id declared nowhere in the file
auto-creates an empty `|box|` at the scene root with the id as its label — so
`cat -> dog -> bird` is a complete three-box diagram. Declaring the id anywhere —
before or after the link — prevents auto-creation. If the id exists only deeper
in the tree, nothing is created: the link must use the full path, and the error
suggests it. Body links never auto-create.

### Declarations

A declaration `key: value;` lives only in a `{ }` style block — the stylesheet
(configuring the root) or a node's own block. Property names are dash-case;
values are space-separated and positional. A bare `key: value` outside a `{ }` is
an error. See [Properties](#10-properties).

---

## 4. Selectors & the Cascade

A **rule** is `|selector| { declarations }` (or `.class { … }`). Selectors are
CSS-shaped, wrapped in bars whenever they name a type:

```
|box| { … }              // every box (element selector)
.hot { … }               // every node with class .hot (class selector — bare)
|table box| { … }        // every box inside a table (descendant)
|.sidebar box| { … }     // every box inside a .sidebar
```

A **descendant selector** is two or more space-separated parts inside the bars;
it matches a node whose ancestor chain contains each part in order (not
necessarily adjacent), exactly like a CSS descendant combinator. Each part is a
single type or a single class — **compounds are not selectors**: a glued
`|box.hot|` is rejected, because an instance wears its class *after* the bars
(`|box| .hot`), never inside them. To style boxes-with-a-class, style the class
(`.hot { … }`).

A **define** introduces a new type from a base: `|treat::box| { … }`. Its
declarations are the type's defaults; an optional `[ ]` gives it intrinsic
children (materialized per instance — see [§9](#9-links)).

A **class** is defined by `.name { … }` and **worn** by writing it after the
type (`|box| .hot`) or after a link's endpoints (`a -> b .hot`) — the same
`.class` slot on both, never inside the bars.

**Specificity** — the most specific source wins; ties break by **source order**
(the CSS cascade):

1. **Type rule** (`|box| { }`) and a type's own define defaults
2. **Descendant rule** (`|table box| { }`, `|.sidebar box| { }`)
3. **Class** (`.hot { }`)
4. **The instance's own block** (`client |box| { fill: white }`) — wins

For a link: cascaded `link*` / `clearance` / `routing` from its scope →
descendant/class rules → the link's own declarations.

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
with `gap: 20`. The default `|box|` pads its content by 20; so does the root, and
its padding is the margin that frames the whole rendered scene — links and
labels included — out to the SVG edge. The frameless `|block|` / `|row|` /
`|column|` pad by 0 (see [§8](#8-templates)).

### Flex — `align` / `justify`

Flexbox model: `justify` runs *along* the flow (main axis), `align` runs *across*
it (cross axis). Both default `center`.

| Value | `justify` (main axis) | `align` (cross axis) |
|---|---|---|
| `start` / `center` / `end` | pack at the edge / centre / opposite | align each child to the edge / centre / opposite |
| `stretch` | fills children to span the main axis | each child's **box** fills the cross axis |
| `evenly` | equal gaps between and around children | (treated as `center`) |

`stretch` fills the child's **box**, not its *content* (placed by the child's own
`align`/`justify`, also `center`). `evenly` needs multiple children.

All of `align`/`justify`/`stretch`/`evenly` are **no-ops unless the container is
larger than its packed children** — an auto-sized container has no slack to
distribute. Slack comes from an explicit `width`/`height`, or a grid's fixed
tracks.

### Grid — `columns` / `rows` / `cell` / `span`

A grid is sized by its track lists:

| Property | Notes |
|---|---|
| `columns` | **Required.** A track list — `columns: 80 140 80` (3 fixed), `columns: repeat(3)` (3 auto), or a mix (`auto 40 auto`). The list length is the column count. |
| `rows` | Optional. Same form. A floor, not a cap: extra children flow into implicit auto rows. Omitted → all rows implicit, count `⌈children / columns⌉`. |
| `cell` | A **box** child's placement `column row`, 1-indexed (`cell: 2 1`). |
| `span` | A **box** child's span `columns rows`, default `1 1` (`span: 2` = `2 1`). |

A **track** is a size (`80`), `auto` (sized to its widest/tallest child), or
`repeat(N)` / `repeat(N, size)` for many equal tracks. The count comes from the
list length.

**Auto-flow.** Children without `cell:` flow left-to-right, wrapping at the
column count; a `cell:` pins one explicitly and the rest flow around it. Bare-text
cells (a table) are pure auto-flow — `cell:` / `span:` apply to **box** children
only (a text node has no block to carry them). A grid is positional, so an empty
`""` cell is **kept** — it holds its track and keeps the cells after it aligned
(in flow, an empty `""` is dropped).

`columns`/`rows`/`cell`/`span` are valid only on a grid (`layout: grid` or
`|table|`); using them elsewhere is an error.

### Dividers

`divider` draws separators between flow children, painted by the container's
`stroke` / `stroke-width` / `stroke-style`:

| Value | Effect |
|---|---|
| `none` (default) | no separators |
| `all` | every **interior** separator — in 1-D between children; in a grid between rows and columns |
| `rows` / `columns` | grid only — separators along that axis |

Dividers are **interior only** — the outer frame is the container's own border
(its `stroke`), so a frameless grid (`stroke: none`) shows only inner lines and a
bordered one is never doubled. `divider` is span-aware in grids (a separator never
crosses a spanning cell's interior, and a shared edge is never drawn twice) and
skips pinned children.

A container with `divider` other than `none` **requires `gap: 0`** (an error
otherwise): a separator wants the cells flush against it. This is what lets
`|table|` be plain `grid + divider: all + gap: 0` rather than a magic type (see
[§8](#8-templates)).

### Container properties

| Property | Applies to | Notes |
|---|---|---|
| `layout` | all | `row`, `column`, `grid`. |
| `gap` | all | Space between children. `N` = both axes; `row col` per axis. Must be `≥ 0`; `0` required with `divider`. |
| `padding` | all | Inner padding. `N`, `v h`, or `t r b l`. On a `\|table\|`, the inset around each cell's text. |
| `align` / `justify` | all | Cross / main axis (above). |
| `columns` / `rows` | grid | Track lists (above). |
| `divider` | all | Separators (above). |
| `fill` | all | Body colour; on the root it is the **canvas** colour. |
| `routing` | all | Routing strategy for links in this scope ([§9](#9-links)). |

---

## 6. Positioning & Anchors

A shape's **bounding box** is the smallest axis-aligned rectangle containing it,
stroke included.

1. **Center origin.** Every bbox is centered at the parent's origin by default.
2. **Source order = render order;** later draws on top, with pinned children above
   the in-flow ones. `layer: N` overrides; ties break by source order.
3. **Strokes count** toward the bbox — `width: 100 height: 50 stroke-width: 4`
   → 104×54.
4. **`|path|`** is the only center-origin exception — `path:` uses native
   top-left coordinates.
5. **Rotation** applies last as an SVG transform; the rotated bounding rectangle
   propagates upward.

### `pin` — out of the flow

Every child is **in flow** by default — laid out by its container's `layout`
([§5](#5-layout)). **`pin` lifts a child out**, aligning the child's **matching
point** flush with a named point of the parent:

| `pin:` | The child sits… |
|---|---|
| `none` *(default)* | — in flow; nothing is pinned |
| `center` | centre on the parent's centre |
| `top` · `bottom` · `left` · `right` | flush against that parent edge |
| `top left` · `top right` · `bottom left` · `bottom right` | with its corner on that parent corner |

The child's *own* matching point lands on the parent's, so it sits **flush**. The
anchor is the parent's **drawn box** — border and padding included. Corners fall
out of the value, so one switch covers every anchor: no compound knobs.

A pinned child is an **overlay**. It **does not grow the parent** — a parent of
only pinned children collapses to `2 × padding` — and it **paints above** the
in-flow children, so a badge needs no explicit `layer`. The canvas always includes
it, so an overlay is never clipped. Set `layer:` to reorder overlapping pins, or
to push one *beneath* the flow.

### `translate` — the universal nudge

**`translate: x y`** shifts a node by (x, y) *after* it is placed. It works on
**every** node — flow children, pinned children, text nodes, the root alike — and
is layout-neutral: siblings don't move, the parent doesn't grow, no size changes.
It is CSS's standalone `translate`, baked into the node's origin (so a standalone
SVG needs no transform variable); the canvas still includes the shifted node.

There is **no numeric coordinate property**. Because the parent's origin is its
center, `pin: center` + `translate: x y` lands a child's center at parent-local
(x, y) — explicit coordinates with no shape-size arithmetic.

`translate` and `rotate` are the two positioning knobs that work on **any** node,
text included — so a link label or a stray string can be nudged or turned in
place. `pin` (which needs a parent anchor and takes a child out of the flow) is a
**box** job; to pin text, wrap it in a `|block|`.

### Auto-sizing

`width` and `height` default to **`auto`** — the bbox sizes to its content (text
or child nodes) **plus `padding` on each side** (default 20; there is no separate
text padding). Sizing is **border-box**: padding sits *inside* the box, never
added on top, and the two axes are independent. An explicit `width` / `height` is
a **floor** — the box is exactly that size when its content fits, and grows past
it (to `content + padding`) when the content is larger, so a box never clips or
spills its content. A box with no in-flow content — empty, or holding only
`pin`ned overlays — has nothing to grow for: an explicit size stands exactly as
written, and an **auto** one falls to **`2 × padding`** on each axis (the default
`padding` 20 gives a 40 × 40 minimum).

**Padding also places the content.** The content area is the box inset by
`padding`, and the content sits within it; symmetric padding centres it, while an
asymmetric `padding: t r b l` offsets it — `padding: 4 4 20 4` lifts the content
toward the top, away from the larger bottom inset, exactly like CSS.

Exceptions: a **text** node sizes to its glyphs (no padding), widened by
`letter-spacing` and given `line-spacing` between `\n` lines; `|icon|` defaults to
`icon-size` (24); `|line|` / `|poly|` / `|image|` / `|path|` require their geometry
(`points` / `src` / `path`) and error without it. `|block|` carries `padding: 0`,
so a bare block sizes to its content exactly.

Text width uses one advance per character (≈ 0.6 em). The default font is
monospace, so this is essentially exact; a proportional `font-family` override
makes it approximate until embedded font metrics land ([§19](#19-deferred)).

---

## 7. Shapes

12 shape primitives. All accept position and visual properties; closed shapes also
accept `stack`, `rotate`, `shadow`. Text is **not** a shape — it is bare content
([§3](#3-statements)); the frameless `|block|` box ([§8](#8-templates)) is what
you reach for when text needs an id, a class, a link, or box layout.

**Dimensions** use `width` / `height`, each defaulting to `auto` (content +
padding, **border-box** — see [§6](#6-positioning--anchors)). They are always
**bbox dimensions**: `|oval| { width: 60; height: 40 }` is an ellipse in a 60×40
box; equal dimensions (or an empty `|oval|`) make a circle.

| Primitive | Required | Notes |
|---|---|---|
| `\|block\|` | size (auto) | The base rectangle — frameless (no fill/stroke, `radius: 0`, `padding: 0`), like a `div`. `\|box\|` frames + rounds it, `\|rect\|` frames it sharp ([§8](#8-templates)). |
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
| `stroke-style` | `solid` / `dashed` / `dotted` | Stroke pattern. Default `solid`. (`wavy` draws on links — [§9](#9-links); on closed shapes it is deferred — [§19](#19-deferred).) |
| `stack` | `N` / `dx dy` | Draw an offset duplicate behind the shape. Scalar `N` = `N -N`. |
| `rotate` | `N` degrees | Rotate around the bbox center. |
| `shadow` | `N` / `dx dy` / `dx dy blur` / `dx dy blur color` | Drop shadow via SVG `<filter>`. Scalar `N` = offset `N N`, blur `N`; tint defaults to `--lini-shadow-color`. |

### Markers (on `|line|` and links)

| Property | Effect |
|---|---|
| `marker: X` | Both ends. |
| `marker-start: X` | Start end (link source). |
| `marker-end: X` | End end (link target). |

Values: `none`, `arrow`, `dot`, `diamond`, `crow`. Markers scale with
`stroke-width` (on a link, with `link-width`), floor 5 px; colour follows the
stroke / link colour. `|line|` is bare by default — write
`|line| { marker-end: arrow }` for a one-shot arrow. For links the operator
picks markers (see [§9](#9-links)). Source order wins: `marker: arrow;
marker-end: dot` → start arrow, end dot.

---

## 8. Templates

Built-in types — each a bundle over a shape base, named because the pattern is
common. **Every rectangular template is a bundle over `|block|`**; the non-rect
shapes ([§7](#7-shapes)) are their own primitives.

| Template | Base | Defaults | For |
|---|---|---|---|
| `\|box\|` | `\|block\|` | `fill: --fill; stroke: --stroke; stroke-width: 2; radius: 6; padding: 20` | The **default** node — a rounded, framed card. |
| `\|rect\|` | `\|box\|` | `radius: 0` | A sharp-cornered box. |
| `\|group\|` | `\|block\|` | `stroke: --group-stroke; stroke-style: dashed; stroke-width: 1; fill: --group-fill; radius: 6; padding: 20` | Dashed frame for a caption + children. |
| `\|caption\|` | `\|block\|` | `pin: top left; translate: 0 -18; color: --caption-color; font-size: 12; font-weight: normal` | A title, pinned just above the group's top-left corner. |
| `\|footer\|` | `\|caption\|` | `pin: bottom; translate: 0 17; font-size: 11; color: --footer-color` | A caption flipped to the bottom edge, centred and muted. |
| `\|badge\|` | `\|block\|` | `pin: top right; translate: 6 -6; radius: 8; padding: 2 6; shadow: 2 3 3; fill: --accent; color: --accent-text; font-size: 11; font-weight: normal` | Corner pill — nudged out over the top-right corner, grows nothing. |
| `\|note\|` | `\|block\|` | `radius: 2; shadow: 2; fill: --note-bg; padding: 20` | Sticky note. |
| `\|row\|` | `\|block\|` | `layout: row` | Frameless wrapper — children in a row. |
| `\|column\|` | `\|block\|` | `layout: column` | Frameless wrapper — children in a column. |
| `\|table\|` | `\|group\|` | `layout: grid; divider: all; gap: 0; padding: 4 8; fill: none; stroke: --stroke; stroke-style: solid; font-size: 14; font-weight: normal` | Ruled grid (see below). |

The bare `|block|` is the base everything rectangular builds on: no fill, no
stroke, `radius: 0`, `padding: 0` — a frameless box that shows only its content,
but is a real box (id, class, children, wirable, positionable). It is what you
reach for to wrap text that needs box behaviour.

**Captions.** A `|caption|` is a small `|block|` **pinned** just above the group's
top-left corner; a `|footer|` is the same flipped to the bottom. Both are
out-of-flow overlays, so they never push the content, and their place is fixed by
the template, not by where they sit among the children:

```
panel |group| [
  |caption| "Settings"
  a |box| "General"
  b |box| "Network"
  |footer| "synced"
]
```

Style every caption globally with `|caption| { font-size: 16; font-weight: bold }`
— that targets captions without touching body text. Because a caption is pinned
(not in flow), a group laid out as a `row` carries its title just the same.

**Tables.** A `|table|` is sugar — a `group` that is a grid, draws dividers, and
has `gap: 0`. Cells are **bare text** that auto-flows into the tracks; there is no
`|cell|` type and no per-cell styling beyond the text's own style block — spacing
comes from the track sizes (`columns` / `rows`) and the table's `padding`. The
outer frame is the group border and the inner lines are `divider: all`, both
painted by the table's `stroke*`; no edge is ever doubled.

```
basket |table| {
  columns: 80 140 80;
} [
  "Fruit" "Quantity" "Notes"
  "Apple" "12"       "fresh"
  "Mango" "3"        "ripe"
]
```

`fmt` knows the column count and pads the cells into aligned columns, so the flat
form reads like the table it is. A cell that must be placed or linked is a
**box** child (`|block| { … } "X"` or `|box| { cell: 2 1; … }`); a cell that just
needs a colour or weight can take its own style block (`"Apple" { color: --red-ink }`).

Extend any template: `|panel::group| { stroke: --accent }`. Common shapes need no
template:

| For | Write |
|---|---|
| Circle | `\|oval\| { width: 40 }` |
| Database | `\|cyl\|` |
| Arrow | `\|line\| { marker-end: arrow; points: 0 0, 50 0 }` |

---

## 9. Links

A link connects scene-node ids with an operator (`a -> b`). Like every shape it
has a `{ }` **style** and a `[ ]` of **content** — its content is its **labels**
(text), placed along the route by `along:`. It is never written as a `|link|`
instance; the operator draws it.

Defaults for every link — `link` (colour), `link-width`, `link-style`,
`clearance`, `marker*`, `routing` — **cascade** from the link's scope: set them on
the root or any container's `{ }` and they reach every link in that scope, exactly
as `color` reaches text. A link's own `{ }` overrides.

### Operators

A link op is `[start_marker?][line][end_marker?]`, no spaces:

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
wins). The operator's line part sets the link's `link-style` (`--` ⇒ `dashed`,
`..` ⇒ `dotted`, `~` ⇒ `wavy`); an explicit `link-style:` overrides it.

### Syntax

```
endpoints op endpoints [op endpoints …] [.class…] [{ declarations }] [ "label"… | [ children ] ]
```

`endpoints` is one or more endpoints joined by `&`:

```
a -> b               // 1 link
a -> b -> c          // chain: 2 links
a -> b & c           // fan-out: a→b, a→c
a & b -> c           // fan-in
a & b -> c & d       // cartesian: 4 links
a -> b -> c & d      // chain + fan
```

Mixing operators in one chain is a parse error.

A link's **class follows** its endpoints (`a -> b .loud`), exactly as a node's
follows its type (`|box| .loud`) — one `.class` slot, after the identity, on
both; a class never lives inside the bars. On a chain or fan, the class and the
`{ }` apply to every link the statement expands to.

### Styling

`link` / `link-width` / `link-style` are the **link paint family**, parallel to
`stroke` / `stroke-width` / `stroke-style` for shapes and `color` for text:

| Property | Type | Default | Notes |
|---|---|---|---|
| `link` | colour | `--stroke` | The line colour. A link never uses `stroke`. |
| `link-width` | number | 2 | Line thickness; markers scale with it. |
| `link-style` | `solid` / `dashed` / `dotted` / `wavy` | from the operator | The dash pattern; usually set by the op (`-->` ⇒ dashed), overridable here. |

All three cascade from the link's scope and override on the link's own `{ }`:

```
{ stroke: #444; link: #888; link-width: 1.5; clearance: 12; routing: orthogonal }
a -> b { link: red; link-style: dashed }     // one link overrides
```

`clearance` (default 16) and `routing` (default `orthogonal`) cascade the same
way; `marker*` come from the operator and override per link.

### Labels

A link's content is **text** — trailing the head (`a -> b "watches"`,
`a -> b "x" "y"`) as sugar, or the canonical `[ ]` form (`a -> b [ "watches" ]`),
distributed along the route by `along:` — the link's track rule, exactly as
`columns:` is a grid's:

| Property | Notes |
|---|---|
| `along` | A list of `0..1` fractions along the whole drawn route, one per label (`along: 0.2 0.5 0.8`). Omitted → auto-distribute across the hops, so one label avoids junctions and several spread out. |

`along:` and any link style live in the `{ }`; the labels are the content:

```
a -> b "watches"                                // trailing-label sugar, auto-placed
a -> b [ "watches" ]                            // canonical — labels in [ ], like every shape
a -> b { along: 0.3 0.7 } [ "near a" "near b" ]
a -> b .loud { link: red } [ "watches" { translate: 0 -6 } ]   // class + style + a styled label
```

Each label is an ordinary **styleable text leaf** ([§3](#3-statements)): give it
its own `{ }` to nudge or turn it — `[ "watches" { translate: 0 -6; rotate: 12 } ]`.
A label is an obstacle to nothing, and may slide along the link to keep clear of
nodes and other labels; the link never moves for it. Link labels default to
`font-size: 11`, `font-weight: normal`, and are tinted by the link's `color` —
each link carries those baked text defaults, cascading to its labels; set
`font-size` / `color` on the link to restyle them all at once, or on a label to
restyle one.

### Endpoints & scope

```
endpoint = ident { "." ident } [ "." side ]
side     = top | bottom | left | right
```

Every link resolves in a **scope** — the scene root for top-level links, the
container's body for links written inside one. An endpoint is a path walked from
that scope: the first segment names a node in the scope, each further segment a
child of the previous, a final segment matching a side name forces that side.
**There is no search**: a name not reachable this way is an error, and the error
suggests full paths of same-named nodes —
`link endpoint 'bowl' not found at scene root; did you mean 'kitchen.counter.bowl'?`

| Endpoint (root link) | Resolves to |
|---|---|
| `cat` | root node `cat` |
| `kitchen.counter.bowl` | exactly that path |
| `kitchen.counter.bowl.left` | the same node, left side forced |

Bodies are **sealed**: a body link connects nodes of its own subtree only.
Cross-container links are written at the lowest level where both ends are visible —
usually the root. Without a side the router picks edges by geometry; with a side,
that edge is forced.

### Internal links in a body

A container's (or define's) `[ ]` may link its own children — internal links come
after the children, in source order. In a define, ids are local and materialize
per instance — the same sealed-body rule. From outside, the dot-path navigates in:

```
{
  |room::group| {
    layout: column;  gap: 10;
  } [
    inlet  |box| "Inlet"
    outlet |box| "Outlet"
    inlet -> outlet "flows"
  ]
}

garden  |room| "Garden"
kitchen |room| "Kitchen"
garden.outlet -> kitchen.inlet "carries"
```

### Routing

Links route **orthogonally** by default — horizontal and vertical runs through the
free space between nodes, corners rounded. The router picks entry/exit sides unless
an explicit `.side` forces one. `clearance` (default 16) is the minimum gap every
link keeps from nodes and from other links.

`routing` selects the strategy for a scope and cascades like `clearance`:
`orthogonal` (the default and the only mode built today) routes by the contract
below; `straight` and `curved` are named but deferred ([§19](#19-deferred)). It
pairs with `layout` — `layout` places the nodes, `routing` routes the links
between them — so a group can route its internals one way while the root routes
another.

The full routing contract — clearance, spacing, crossings, fan-out, self-loops —
lives in [`LINKING.md`](LINKING.md), the source of truth for routing.

---

## 10. Properties

Every property is `name: value;`. Dash-case names; positional, space-separated
values.

### Paint

| Property | Type | Default |
|---|---|---|
| `fill` | color | `--fill` (closed shapes); `currentColor` on text; `--stroke` for icons; `--bg` on the root (the scene background) |
| `color` | color | inherits — sets text/icon glyph colour for descendants; on text, an alias for `fill` |
| `opacity` | 0..1 | 1 |
| `radius` | number | 0 (`\|block\|`); `\|box\|` rounds to 6 |
| `rotate` | degrees | 0 |
| `skew` | degrees | 15 (`\|slant\|` only) |
| `shadow` | `N` / `dx dy` / `dx dy blur` / `dx dy blur color` | off |
| `stack` | `N` / `dx dy` | off (closed shapes only) |

`color` cascades through the SVG via native `currentColor`: set it on a container
to recolour every descendant's text that doesn't override. Use `color` for
*labels*, `fill` for *bodies*. `fill`, `stroke`, and `link` all accept a
**gradient** as well as a flat colour ([§11.3](#113-gradients)).

### Stroke

| Property | Type | Default |
|---|---|---|
| `stroke` | color | `--stroke` (a shape's outline / a `\|line\|`'s colour) |
| `stroke-width` | number | 2 (`\|group\|` is `1`) |
| `stroke-style` | `solid` / `dashed` / `dotted` | `solid` |

### Links

| Property | Type | Default | Notes |
|---|---|---|---|
| `link` | color | `--stroke` | A link's line colour ([§9](#9-links)). Cascades to links in scope. |
| `link-width` | number | 2 | A link's thickness. |
| `link-style` | `solid` / `dashed` / `dotted` / `wavy` | from the operator | A link's dash pattern. |
| `clearance` | number | 16 | Min gap a link keeps from nodes and links. Cascades. |
| `routing` | `orthogonal` (+ deferred) | `orthogonal` | Routing strategy for links in scope. Cascades. |
| `along` | fraction list | auto | Label positions along the route. |
| `marker` / `marker-start` / `marker-end` | marker | from the operator | Endpoint glyphs ([§7](#7-shapes)). |

`link*`, `clearance`, and `routing` are **inheritable**: set on the root or a
container, they reach every link in that scope; a link's own block overrides.

### Geometry & placement

| Property | Type | Notes |
|---|---|---|
| `width`, `height` | number / `auto` | bbox dims, **border-box** (padding inside); a **floor** — at least this size, growing to `content + padding` when content is larger. Default `auto` = content + padding. `\|image\|` needs both. |
| `pin` | `none` / `center` / edges / corners | Out-of-flow anchor — the child's matching point lands on the named parent point ([§6](#6-positioning--anchors)). A **box** property. |
| `translate` | `x y` | Post-placement nudge of the node and its subtree; no reflow, grows nothing ([§6](#6-positioning--anchors)). Works on **any** node, text included. |
| `layer` | integer | Paint order; default 0 in flow, 1 when `pin`ned. Ties break on source order. |
| `points` | `x y, x y, …` | Vertex list (`\|poly\|`, `\|line\|`). |
| `path` | string | Raw SVG path (`\|path\|`, native top-left coords). |

`pin`, `width`/`height`, `points`, and `path` are **box** properties — a bare text
node carries none of them; `translate` and `rotate` are the exceptions and work on
text too. To pin or size a piece of text, wrap it in a `|block|`.

### Spacing & layout

`padding`, `gap`, `layout`, `align`, `justify`, `columns`, `rows`, `cell`, `span`,
`divider`, `routing` — see [Layout](#5-layout) and [Positioning](#6-positioning--anchors).
Longhands `padding-top`/`-right`/`-bottom`/`-left` are accepted.

### Text

| Property | Default | Notes |
|---|---|---|
| `font-family` | `--font-family` | ident, string, or `--var`. |
| `font-size` | 15 (body), 12 (caption), 11 (link label) | px; a baked layout constant. |
| `font-weight` | `--font-weight` (body `bold`; captions / link labels `normal`) | `normal` / `bold`. |
| `font-style` | `normal` | `normal` / `italic` / `oblique` — live CSS. |
| `text-transform` | `none` | `uppercase` / `lowercase` / `capitalize` — live CSS (browser-applied; some SVG renderers ignore it). |
| `text-decoration` | `none` | `underline` / `overline` / `line-through` — live CSS. |
| `letter-spacing` | 0 | px between characters — positive widens, negative tightens. |
| `line-spacing` | 0 | px added between the lines of a `\n` text block. |

These all **inherit** — nearest ancestor wins, like CSS. Set them on a containing
box (or the root) and they cascade down, or set them directly on a string's own
style block (`"x" { font-weight: bold }`) for that one text node. Style globally
with `font-size: …` in the stylesheet, or scope it on a container. A global
`font-family:` / `color:` works too, but for an **embeddable** diagram prefer the
`--lini-font-family` / `--lini-text-color` variables (or `--theme`) — they stay
live for a host page to re-theme, where a global property bakes its value into the
`.lini` rule.

`letter-spacing` and `line-spacing` are **baked spacing**, not CSS: they change
**layout** — the text box grows to fit the wider glyphs or taller block — and the
spacing compiles into the glyph and line positions (like `padding`), never emitted
as a style. Both default to 0, so text is unaffected until set.

`font-style`, `text-transform`, and `text-decoration` are the reverse — **live
CSS** with no baked default: they don't touch layout, ride the class / `<g>` /
`.lini` rule, and a host page can override them. Set any in the global block to
style the whole scene (it states on `.lini`), exactly like a global `font-size:`.

### Media & accessibility

| Property | Notes |
|---|---|
| `src` | image source (`\|image\|`). |
| `href` | wraps this node or link in `<a href>` — clickable. |
| `icon-variant` | `outlined` / `filled` / `rounded` / `sharp`. |
| `title` | emits a `<title>` child (tooltip + screen-reader name). |

### Variables

`--name: value;` declares a themeable variable; `--name` in a value references one.
Visual variables stay live `var()`; layout values bake. See
[Colour, Variables & Defaults](#11-colour-variables--defaults).

---

## 11. Colour, Variables & Defaults

CSS variables theme the **visual** layer — colours and the font family. Everything
that affects layout — sizes, gaps, padding, and font *size* — is a baked constant,
so a standalone SVG never depends on host CSS.

### 11.1 Visual variables (live, themeable)

Each colour is a `light-dark(LIGHT, DARK)` value, so one SVG carries both modes:

```
--lini-bg            light-dark(white, #1b1b1f)      the scene background
--lini-fg            light-dark(black, #e8e8ea)
--lini-fill          light-dark(white, #26262b)
--lini-stroke        light-dark(#444, #9aa0a6)
--lini-accent        light-dark(#0a84ff, #4aa3ff)
--lini-accent-text   white                           text on an accent fill (e.g. a badge)
--lini-muted         light-dark(#888, #9aa0a6)
--lini-danger        light-dark(crimson, #ff6b6b)
--lini-warn          light-dark(orange, #ffb454)
--lini-stray         light-dark(crimson, #ff6b6b)    the stray-link fallback (LINKING.md, §Impossible layouts)
--lini-note-bg       light-dark(#fff9c4, #4a4733)
--lini-group-stroke  light-dark(rgba(0,0,0,.4), rgba(255,255,255,.4))
--lini-group-fill    light-dark(rgba(0,0,0,.03), rgba(255,255,255,.05))
--lini-caption-color light-dark(rgba(0,0,0,.5), rgba(255,255,255,.55))
--lini-footer-color  light-dark(rgba(0,0,0,.5), rgba(255,255,255,.55))
--lini-font-family   ui-monospace, "SF Mono", "Cascadia Code", "JetBrains Mono", Menlo, Consolas, "Liberation Mono", monospace
--lini-font-weight         bold
--lini-caption-font-weight normal
--lini-link-font-weight    normal
--lini-text-color    var(--lini-fg)
--lini-shadow-color  light-dark(rgba(0,0,0,.2), rgba(0,0,0,.5))
```

`--lini-bg` paints the scene background (the canvas rect), so the diagram is
self-contained in either mode. The default font is a **monospace** stack: it reads
crisp, and a fixed glyph advance keeps text-width estimation accurate without
embedded font metrics ([§19](#19-deferred)). Body text is **bold**,
captions and link labels **normal**. A themed proportional `font-family` is allowed
but makes text width approximate until embedded metrics land.

**Dark/light is automatic.** The compiler emits `color-scheme: light dark` on
`.lini`, so `light-dark()` follows the viewer's OS (`prefers-color-scheme`) — no
script, no `@media`. A `data-theme="dark"` / `"light"` on the SVG or any ancestor
forces a mode (it flips `color-scheme`, and its higher specificity beats the OS).
All defaults sit in `@layer lini.defaults`, so unlayered host CSS still wins with no
`!important`. `--bake-vars` freezes the light arm into literals for renderers
without `light-dark()` ([§11.6](#116---bake-vars)).

### 11.2 The colour palette

Beyond the role variables, Lini ships a **named-hue palette** — pretty by default,
themeable, and dark/light-aware like everything else. Eleven hues, each a
`light-dark()` pair:

```
red  rose  orange  amber  lime  green  teal  sky  blue  purple  gray
```

Every hue carries **five tiers**, named for the job they do — not their lightness,
which would invert in dark mode:

| Tier | Example | Job |
|---|---|---|
| wash | `--teal-wash` | palest — card and section backgrounds (a faint tint; a deep, muted surface in dark mode) |
| soft | `--teal-soft` | a gentle, lighter pastel fill |
| base | `--teal` | the everyday pastel — **the bare name is the easy path** |
| deep | `--teal-deep` | the strong tone — borders and strokes |
| ink | `--teal-ink` | deepest and most saturated — text and emphasis (the high-contrast tone in dark mode) |

`fill: --teal` lands a friendly pastel; the job-names hold across the dark flip, so
`--teal-wash` is always the faint surface and `--teal-ink` always the high-contrast
detail — a promise a `light` / `dark` or numeric name could not keep.

```
{ |card::box| { fill: --teal-wash; stroke: --teal-ink } }   // a pretty card, one line
note |note| { fill: --amber-soft }
```

The tiers are generated from one **OKLCH** seed per hue, so the ramp is
perceptually even and the eleven read as a family. The same space is open to you —
`fill: oklch(0.7, 0.14, 200)` picks any colour directly ([§2](#2-lexical-syntax)).
Names are conventional — every
one is an ordinary colour word, so `--blue`, `--red`, `--green` are all there — with
aliases for muscle memory: `--yellow → --amber`, `--pink → --rose`,
`--indigo → --purple`, `--cyan → --teal`. `red` stays clear for **danger**; `rose`
is the warm pink you decorate with (its `wash` / `soft` tiers are your pinks),
`green` is tuned to an emerald, and `lime` is the lemony one.

The palette is **tree-shaken** ([§13](#13-svg-output)): only the `--lini-*`
variables a diagram references are emitted, so the full palette costs a three-box
diagram nothing.

### 11.3 Gradients

`fill`, `stroke`, and `link` accept a **gradient** in place of a flat colour. Stops
are ordinary colours — palette `--name`s flip dark/light and bake, a raw `#hex` is a
fixed literal.

| Form | Result |
|---|---|
| `gradient(--rose, --sky)` | two stops, auto-angled 135° — any two hues blend cleanly |
| `gradient(--rose, --amber, --sky)` | three or more evenly-spaced stops |
| `linear-gradient(135, --rose, --sky)` | an explicit angle in degrees — the control gate |
| `radial-gradient(--rose, --sky)` | a radial blend from the centre out |

```
hero |box|   { fill: gradient(--blue, --purple) }       // a single-family sheen
tag  |badge| { fill: gradient(--rose, --amber, --sky) } // a three-colour pop
```

The angle is the only "more syntax": `gradient(…)` is angle-less and always lands on
a flattering 135°, so the easy form can't look wrong. OKLCH stops keep the midpoint
clean rather than muddy.

Each distinct gradient is emitted once as a `<linearGradient>` / `<radialGradient>`
in `<defs>` and referenced by `url(#…)` — deduplicated and shared like the
drop-shadow `<filter>`s ([§13](#13-svg-output)). `objectBoundingBox` units fit one
definition to any shape at any size. The stops being palette vars, a gradient
themes, flips, and bakes like any other paint; gradient-on-text is deferred
([§19](#19-deferred)).

### 11.4 `--name` references

`--name` is the **visual-variable namespace, and only that**. `--name: value;`
declares one (a built-in `--lini-*` name keeps its meaning; a new name is yours),
and `--name` in a value references it, emitting `var(--lini-name)`:

```
{
  --brand: #ff6600;
}
cat |box| { fill: --brand }
```

Alias a host var from CSS: `.lini { --lini-accent: var(--my-brand-blue); }`.

Layout values — sizes, gaps, padding, `font-size`, `clearance` — are **not**
`--name` variables: they aren't themeable. Set them with properties and rules
instead (`gap: 30;`, `|box| { radius: 4 }`, `font-size: 16;` in the stylesheet).

### 11.5 Layout constants (baked)

Baked compile-time defaults — override per-node, on the root, in rules, or in an
instance / link block:

```
font-size 15    link-font-size 11   caption-font-size 12
stroke-width 2  radius 6            gap 20                 padding 20
clearance 16    icon-size 24        link-width 2
```

`font-size` is body text. Link labels and captions carry their own baked defaults
(11 and 12); a global `font-size:` in the stylesheet sets body text and cascades,
each link carries the `link-font-size` default for its labels (set `font-size:` on
a link to change them), and `|caption| { font-size: … }` sets captions. `radius`
rounds a `|box|` by default; `|block|` / `|rect|` are `0`.

Padding defaults to 20 — including the root, whose padding frames the whole
scene (the SVG margin) — with `|block|` / `|row|` / `|column|` at 0 and a
`|table|` at `4 8` (its cell inset). It doubles as the minimum size of an
empty box (`2 × padding`; see [Auto-sizing](#6-positioning--anchors)). **Every
baked default — these constants and the template bundles — lives in one place**,
so the whole look is tuned from a single file.

### 11.6 `--bake-vars`

Class rules and inline `style=` work everywhere, but CSS *variables* don't — resvg
and librsvg fail `var()` in every position (browsers, even `<img>`-embedded, are
fine). `--bake-vars` keeps the rules but inlines every `var(--lini-name)` as its
literal: no runtime theming, but a self-contained SVG that renders anywhere.

---

## 12. Specificity

Properties on a node merge like CSS — **the more specific source wins**, ties
broken by **later wins** (source order):

1. **Type cascade** — walked from the base primitive up to the node's declared
   type, layering each type's element-rule (`|box| { }`) and define defaults. A
   more-derived type overrides what it builds on.
2. **Descendant rules** — `|table box| { }`, `|.sidebar box| { }`, matched against
   the ancestor chain.
3. **Class rules** — `.hot { }`, worn via `|box| .hot` on the node.
4. **The instance's own block** — `client |box| { fill: white }` — the most
   specific, beats everything above.

For a link: cascaded `link*` / `clearance` / `routing` from its scope →
descendant/class rules → the link's own declarations.

Complex values (`translate: x y`, `padding: t r b l`) replace wholesale — the
merge is per-property, not deep. A `pin`ned child ignores `cell:` — pinning takes
it out of the grid.

---

## 13. SVG Output

```svg
<svg xmlns="http://www.w3.org/2000/svg"
     viewBox="X Y W H" width="W" height="H" class="lini">
  <style>
    @layer lini.defaults {
      :root, .lini { color-scheme: light dark; /* --lini-*: light-dark(…, …) */ }
      .lini[data-theme="dark"],  [data-theme="dark"]  .lini { color-scheme: dark; }
      .lini[data-theme="light"], [data-theme="light"] .lini { color-scheme: light; }
    }
    .lini { font-family: var(--lini-font-family); font-size: 15px; font-weight: var(--lini-font-weight); color: var(--lini-text-color); }
    .lini .lini-canvas { fill: var(--lini-bg); }
    .lini .lini-box { fill: var(--lini-fill); stroke: var(--lini-stroke); stroke-width: 2; }
    .lini .lini-style-hot { stroke-width: 3; }   /* one rule per class def */
    .lini .lini-link { stroke: var(--lini-stroke); stroke-width: 2; fill: none; }
  </style>
  <defs><!-- filters, gradients, clipPaths, icon symbols --></defs>
  <rect class="lini-canvas" .../>   <!-- the scene background (--lini-bg) -->
  <g class="lini-scene"> <!-- scene tree --> </g>
  <g class="lini-links"> <!-- links --> </g>
</svg>
```

`viewBox` auto-sizes to content + the scene's `padding` (20 px by default) on
every side. The `lini-canvas` backing rect paints the scene background
(`--lini-bg`) over the viewBox; a root `fill:` overrides it (`none` = transparent).

**Paint compiles to CSS; geometry bakes.** Shape and link paint defaults — and
every rule — are stated once as class rules; only the classes actually used are
emitted — and likewise only the `--lini-*` variables actually referenced, so the
built-in palette ([§11.2](#112-the-colour-palette)) adds nothing unless a diagram
uses it. A node whose resolved paint differs from those rules carries the
difference as an inline `style="…"` (inline beats class, mirroring
[Specificity](#12-specificity)). Geometry — sizes, positions (`pin` and
`translate` fold into the baked origin), radii, points, paths, transforms — is
always baked into attributes. Inherited text properties (`font-family`,
`font-size`, `font-weight`, `color`, and any global `font-style` / `text-transform`
/ `text-decoration`) state on `.lini` and cascade natively; a node's own text
property emits on its `<g>` (or directly on the `<text>`) and inherits to its
subtree.

**Box:**

```svg
<g class="lini-node lini-{type} lini-{base} lini-style-{class}"
   data-id="ID" transform="translate(X,Y)">
  <title>…</title>            <!-- when `title:` is set -->
  <!-- geometry, then children -->
</g>
```

Auto-classes: `lini-node` (every box); `lini-{name}` (the type and every
type it inherits, down to `lini-block`); `lini-style-{name}` (per worn class).
With rotation, the transform becomes `translate(X,Y) rotate(N)`.

**Text** emits a bare `<text class="lini-text">…</text>` at its placed position —
no wrapping `<g>`, so a table of N cells is N `<text>` elements, not N boxes. Its
font and colour come by inheritance from the enclosing `<g>`; a string's own style
block emits as a `style="…"` (and `translate` / `rotate` as a `transform`) on the
`<text>` itself.

**Link:**

```svg
<g class="lini-link lini-style-{class}" data-from="A" data-to="B">
  <path d="…" fill="none" stroke="…"/>
  <polygon class="lini-marker lini-marker-arrow" …/>
  <text class="lini-text" …>label</text>   <!-- placed by along: -->
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
lini serve [--port N] [--bake-vars] [PATH]
lini theme [NAME]
```

| Flag | Meaning |
|---|---|
| `-o FILE` | Output path (default stdout). |
| `--format svg\|html` | `svg` (default) or HTML wrapper. |
| `--check` | Parse + resolve only — layout/render errors still surface on a full compile. |
| `--theme NAME\|FILE\|A/B` | A built-in theme (`dark`, `high-contrast`, …), a CSS file of `--lini-*` overrides, or a light/dark pair (`light/dark`). |
| `--no-warn` / `--strict` | Silence warnings / treat them as errors. |
| `--bake-vars` | Inline `var()`s as literals (for non-browser renderers). |
| `--watch` | Recompile on every input change (requires `-o`). |
| `-h`, `-V` | Help / version. |

`lini -` reads stdin (filename `<stdin>` in errors). **`lini serve`** runs a local
live preview (default port 7700): a `.lini` file live-reloads that one file; a
directory (or no path → the current directory) opens the **playground** — pick,
edit, and render any `.lini` file beneath it in the browser. **`lini theme`** lists
the built-in themes; **`lini theme NAME`** prints one as a `--lini-*` CSS file — a
ready starting point for your own (`light-dark()` colours, the font commented out).

**`lini fmt`** reformats to canonical style — 2-space indent, `key: value;`
declarations grouped on one line, a style-only node collapsed onto its head line
when it fits (`api |box| { fill: red }`), a lone label trailing the head
(`api |box| "API"`), children one per line in `[ ]`, table cells padded into
aligned columns, comments and blank lines preserved. `--check` exits 1 if it would
change anything; `--stdout` writes instead of rewriting.

**`lini desugar`** prints the file fully **lowered to primitives** — every
template/define instance becomes its base `|block|` (etc.) wearing a `.lini-*` class
chain, each type's defaults become a generated `.lini-<type> { … }` class, the
scene and cascaded link defaults fill the global block, define bodies inline per
instance, and id-as-label / trailing-label / auto-`along:` become explicit. It is
the engine's true input — the rest of the pipeline only ever sees primitives, and
the lowered form re-renders byte-identically. A teaching/debugging view; prints to
stdout, never rewrites, comments not preserved.

Exit codes: 0 success · 1 parse/resolution error or `--check` reformat needed · 2
I/O · 3 invalid CLI.

---

## 15. Errors

Format: `filename:line:col: error: <message>` (LSP-compatible).

| Condition | Message |
|---|---|
| Duplicate id | `duplicate id 'X' (previously at L:C)` |
| Unknown endpoint | `link endpoint 'X' not found at <scope>` + `; did you mean 'A', 'B'?` |
| Chain mixes operators | `link chain mixes operators 'X' and 'Y'` |
| Chain < 2 nodes | `link requires at least two endpoints` |
| Unknown type / class | `unknown type 'X'` / `unknown class '.X'` |
| Inheritance cycle / depth | `cycle in 'X' → … → 'X'` / `'X' exceeds max inheritance depth (16)` |
| Define shadows builtin | `'X' shadows a built-in type` |
| Missing required property | `'\|line\|' requires 'points'` |
| Unknown property | `unknown property 'foo' on '\|box\|'` (warning) |
| Text carries children | `text content takes no '[ ]' — wrap it in '\|block\|' to give it children` |
| Box property on text | `'pin' needs a box — wrap the text in '\|block\|'` |
| Declaration outside a block | `a declaration belongs in a '{ }' block` |
| Bare type name | `a type only appears in bars — write '\|box\| { }' to style every box` |
| `->` in the stylesheet | `'->' draws a link on the canvas — set link defaults with 'link:' / 'link-width:' in a '{ }' block` |
| Deferred routing | `routing: only 'orthogonal' is built; 'straight' / 'curved' are deferred (§19)` |
| Glued compound in a rule | `a selector part can't glue a type and a class — space them (descendant) or style '.hot'` |
| Class inside instance bars | `a class follows the bars — write '\|box\| .hot', not '\|box.hot\|'` |
| Spaced class chain | `classes glue into a chain — write '.hot.loud', no space` |
| Style block holds non-decl | `a '{ }' style block holds only declarations` |
| `[ ]` holds a declaration | `declarations go in '{ }', not '[ ]'` |
| Children after internal links | `a child must come before the body's links` |
| Label and `[ ]` both | `a node's content is the trailing label or the '[ ]', not both` |
| Stylesheet after canvas | `the stylesheet '{ }' must come first, before any instance` |
| Divider needs flush cells | `'divider' requires 'gap: 0'` |
| Invalid / out-of-range color | `invalid color 'XYZ'` / `rgb(300,0,0): component out of range` |
| Invalid `oklch()` | `oklch expects (L, C, H) or (L, C, H, A) — L and A in 0..1, C ≥ 0, H in degrees` |
| Gradient with < 2 stops | `gradient() needs at least two colour stops` |
| `linear-gradient` without an angle | `linear-gradient needs an angle first, then ≥ 2 colour stops` |
| Reserved identifier | `'left' is reserved (an endpoint side)` |
| Empty statement | `a node needs an id or a type` |
| `\|link\|` / `\|node\|` as instance | `links are drawn by operators, not the '\|link\|' type` / `'node' is the umbrella concept — write '\|block\|' for the bare box` |
| Grid out of range | `cell: 5 _ exceeds columns=3` |
| Grid props off a grid | `'cell' is valid only on a grid` |
| Missing `columns` | `'layout: grid' requires 'columns'` |
| Negative `gap` | `'gap' must be ≥ 0` |
| `skew` out of range | `skew: N must be in (-89, 89)` |
| Single-quoted string | `single quotes are not strings — use "…"` |
| Invalid `pin` value | `'pin' expects none, center, an edge (top/bottom/left/right), or a corner (e.g. 'top right')` |

---

## 16. Grammar (EBNF)

```
file        = [ stylesheet ] canvas links           # the three phases, in order
stylesheet  = "{" { setup_item } "}"                # the root's setup block; omit when empty
setup_item  = decl | vardecl | rule | class_def | define | comment | newline
canvas      = { node | comment | newline }          # instances, drawn in source order
links       = { link | comment | newline }

decl        = ident ":" values end
vardecl     = css_var ":" values end                # --name : value
rule        = "|" selector "|" style                # |box| { } , |table box| { }
class_def   = "." ident style                       # .hot { } — a class definition
define      = "|" ident "::" ident "|" body         # name :: base, optional children

node        = box | text
box         = ( ident [ typeref ] [ classes ] | typeref [ classes ] | classes )
              [ style ] [ labels | children ]        # needs an id, a |type|, or a class
typeref     = "|" ident "|"                          # a type alone — |oval|, a user type
classes     = "." ident { "." ident }               # a worn class chain — .hot, .hot.loud
text        = string [ style ]                       # bare content; a styleable leaf, never a box

style       = "{" { decl } "}"                       # declarations only
children    = "[" { node } { link } "]"              # children, then internal links
body        = [ style ] [ children ]                 # define / container body
labels      = label { label }                        # trailing text → text children
label       = string [ style ]                       # a styleable text leaf

link        = endpoints link_op endpoints { link_op endpoints }
              [ classes ] [ style ] [ labels | children ]   # worn class(es), style, labels
selector    = sel_part { sel_part }                  # whitespace-separated = descendant
sel_part    = ident | "." ident                      # a single type or a single class
endpoints   = endpoint { "&" endpoint }
endpoint    = ident { "." ident } [ "." side ]
side        = "top" | "bottom" | "left" | "right"

values      = value_group { "," value_group }        # comma only between list items
value_group = value { value }                        # space-separated scalars
value       = number | percent | string | hex | ident | css_var | call
call        = ident "(" [ value { "," value } ] ")"
css_var     = "--" ident { "-" ident }

link_op     = [ marker ] line [ marker ]
line        = "-" | "--" | ".." | "~"
marker      = "<" | ">" | "*" | "<>"

ident       = ( letter | "_" ) { letter | digit | "_" | "-" }   # excludes reserved sides
number      = [ "+" | "-" ] ( digit+ [ "." digit+ ] | "." digit+ )
percent     = number "%"                             # colour components only
hex         = "#" hexdigit { hexdigit }              # 3, 4, 6, or 8 hex digits
hexdigit    = digit | "a"…"f" | "A"…"F"
string      = '"' { char | escape } '"'
escape      = "\" ( '"' | "\" | "n" | "t" )
comment     = "//" { not-newline } newline
end         = newline | ";"
```

**Single-pass LL(1).** The phase order (stylesheet → canvas → links) plus the
bracket-and-bars vocabulary make one token of lookahead enough — and *more* than
the old grammar needed, because no statement's kind depends on a type set:

- A leading `{` opens the stylesheet; inside it, `--name :` → variable, `ident :`
  → root declaration, `|…|` → a rule (or, with an inner `::`, a define), `.name` →
  a class.
- On the canvas a statement is a `node` (an `ident`, `|type|`, or `.class` head, or
  a `"…"` text) or — when an `ident` is followed by a link-op, `&`, or a glued `.`
  (an endpoint path) — a `link`.
- `{` is always style, `[` is always children, `|…|` is always a type. A string is
  self-delimiting, so consecutive strings are consecutive text nodes; strings
  trailing a node or link head are that node's labels, to the line's end. A `{`
  immediately after a string is that string's own style block.

**Adjacency tells a `.class` from a path:** a space before the `.` makes it a worn
class (`a .hot` — node `a` with class `hot`), no space makes it an endpoint path
(`a.b`). The first class is space-separated from the type or endpoints; the rest of
the chain glues (`.hot.loud`). A glued `|box.hot|` in the bars is rejected — a
class follows the bars ([§15](#15-errors)). Inside a *rule* selector a space is the
descendant combinator (`table box`) and a part may be a class (`.sidebar box`), but
a type and class never glue. Because `ident` excludes the reserved sides
([§18](#18-reserved-words)), an endpoint's trailing `.left` reads as a side.

No prescan, no second pass, no "define before use" needed for *parsing* (it is
still required for the resolve-time cascade — [§17](#17-implementer-algorithm)).

---

## 17. Implementer Algorithm

A reference pipeline; implementations may differ if the observable output matches.

**Parse.** Lex to tokens, then a single recursive-descent pass to the AST. The
bracket vocabulary (`{ }` style, `[ ]` children, `|…|` type) resolves every
statement with one token of lookahead — no type-set prescan.

**Desugar.** Lower all surface sugar to primitives + classes — the engine's true
input. Each template/define instance becomes its base primitive wearing a `.lini-*`
class chain (derived→base→primitive, down to `block` for every rectangular type); a
type's defaults and any `|type| { }` element rule fold into a generated
`.lini-<type> { … }` class; a `|table box| { }` descendant rule rewrites to
`|.lini-table .lini-box| { }`; define bodies inline per instance; the scene
defaults (`layout`, `padding`, `gap`, `font-size`) and the cascaded link defaults
(`link`, `link-width`, `clearance`, `routing`) settle on the root; id-as-label,
trailing labels, auto-`along:`, and root-link auto-create become explicit. The pass
is idempotent; type-system errors (cycle, depth > 16, a define shadowing a built-in)
surface here.

**Resolve** (top-to-bottom):

1. *Variables & rules:* merge visual-var defaults ← `--theme` ← `--name: value`;
   compile the stylesheet's class rules.
2. *Scene tree:* each box is a primitive wearing `.lini-*` (type) and user classes;
   layer properties per [Specificity](#12-specificity) — the worn `.lini-*` classes
   as the type tier (folded base→derived), then descendant rules, user-class rules,
   and the instance block; lift internal links; build the path index. (Types,
   labels, define bodies, and auto-create were all lowered by **Desugar**.)
3. *Links:* resolve endpoints by scoped path walk with suggestion errors; merge
   link properties — cascaded `link*` / `clearance` / `routing` from the scope
   chain, then class rules, then the link's own block; cartesian-expand fan groups
   into one resolved link per pair; the operator's line sets `link-style` unless
   overridden.

**Layout** (bottom-up): leaf bbox from `width`/`height` or defaults (text → its
glyphs; box → content + `padding`; + half-`stroke-width` per side); arrange flow
children per `layout` honouring `align`/`justify`/`stretch`/`evenly` when there is
slack; pin out-of-flow children to their parent anchor (the parent never grows for
them); compute dividers; apply `padding`; apply each node's `translate`; `rotate`
last.

**Route links.** Per [`LINKING.md`](LINKING.md) — orthogonal, clearance-respecting,
deterministic. Place markers (sized `max(5, link-width × 4)`, tip on the
endpoint) and link labels at their `along:` fractions (auto-distributed when
unset).

**Render.** Depth-first emit SVG per [SVG Output](#13-svg-output): a box is a
`<g>`, a string is a `<text>`.

---

## 18. Reserved Words

Because a type only ever appears in bars (`|box|`) and an id is always bare, **type
names are free as ids** — `block -> oval` is two ordinary nodes. Two small classes
of word stay reserved:

- **Sides:** `top`, `bottom`, `left`, `right` — peeled from endpoint paths
  (`a.left`), so they cannot be node ids. Ids are case-sensitive, so `Left` is free.
- **`node`, `link`,** and the structural class names **`text`, `marker`, `canvas`,
  `scene`, `cut`:** not instantiable types — `node` is the umbrella concept (write
  `|block|` for the bare box), links are drawn by operators (`|link|` is an error),
  and a define may not take one of these (its generated `.lini-<name>` would collide
  with a built-in SVG class).

The **`.lini-*` class prefix** is reserved: desugar generates the type classes
(`.lini-block`, `.lini-box`, `.lini-<define>`), so a user class may not begin
`lini-`. User classes are emitted `.lini-style-<name>`.

Single quotes (`'`) are reserved and are not strings.

Value keywords are **contextual**, not reserved as ids — `grid`, `row`, `column`,
`start`, `center`, `end`, `stretch`, `evenly`, `none`, `auto`, `orthogonal` mean
their keyword only after the property that expects them (`layout: grid`,
`routing: orthogonal`). Function names `rgb`, `rgba`, `hsl`, `repeat` are reserved
only before `(`.

---

## 19. Deferred

**Deferred** — named in the language, not built yet; the syntax is stable:

- `routing: straight` / `routing: curved` — non-orthogonal link strategies
  ([§9](#9-links); `orthogonal` is the only mode built today).
- `stroke-style: wavy` rendering on shapes.
- **gradient fills on text** — `fill: gradient(…)` on a label (gradients fill
  shapes today, [§11.3](#113-gradients)).
- `radius` on non-rect shapes (hex / diamond / slant / poly).
- numeric `font-weight` (`100…900`).
- `|icon|` Material Symbols glyph embedding (currently a placeholder square).
- embedded font metrics — the monospace default keeps the estimate close; a
  proportional `font-family` override is approximate until then.
- `aria-label`, and a "did you mean" property-name hint table.

---

## 20. Examples

```
{
  layout: grid;  columns: repeat(3);  gap: 40;  padding: 20;
  fill: --bg;  link: #666;  clearance: 12;     // link + routing defaults, cascaded

  |box| { radius: 4; }                         // round a touch less than the default 6

  --accent: #0a84ff;

  .thin { stroke: #444; }
  .bold { font-weight: bold; }
  .loud { link: red; link-width: 2; }

  |treat::box|  { radius: 5; }
  |nest::slant| { fill: gray; }
  |alert::oval| { stroke: red; width: 36; height: 36; }   // a circle

  |room::group| {
    layout: column;  gap: 8;
  } [
    inlet  |box| "Inlet"
    outlet |box| "Outlet"
    inlet -> outlet "flows"
  ]
}

cat |oval| { cell: 1 1 } "Cat — patient hunter"

kitchen |group| {
  cell: 2 1;  layout: column;  gap: 20;
} [
  |caption| "Kitchen"
  counter |group| {
    layout: column;  gap: 10;
  } [
    |caption| "Counter"
    bowl  |treat| "Bowl of oats"
    water |nest|  "Water"
  ]
]

garden |group| {
  cell: 3 1;  layout: column;  gap: 20;
} [
  |caption| "Garden"
  den |group| {
    layout: column;  gap: 15;
  } [
    |caption| "Den"
    rabbit |alert| [ |badge| "FAST" ]
    carrot |box| { stack: 4; width: 80; height: 40; fill: white } "Carrot patch"
  ]
]

closet |room| { cell: 1 2 } "Closet"
fridge |room| { cell: 2 2 } "Fridge"

// links — full paths from the link's scope (here: the root)
cat.right -> kitchen.counter.bowl.left -> kitchen.counter.water
kitchen.counter.water -> garden.den.rabbit -> garden.den.carrot .loud
cat <-> kitchen "watches"
closet.outlet -> fridge.inlet "restocks"
```

### Table + dimension line

```
basket |table| {
  columns: 80 140 80;
} [
  "Fruit" "Quantity" "Notes"
  "Apple" "12"       "fresh"
  "Mango" "3"        "ripe"
]

dim |line| {
  points: 0 200, 300 200;
  marker: arrow;  color: #666;
}
```

### Mermaid-fast

```
cat -> dog -> bird     // 3 implicit boxes, 2 links
fox & owl -> mouse     // fan-in
frog ~> pond           // wavy arrow
fish --> bowl          // dashed arrow
newt ..> log           // dotted arrow
```

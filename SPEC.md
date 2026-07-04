# Lini — Language Specification

Pretty diagrams, charts, and technical drawings from plain text, with fine-grained
control. One core — composable nodes, a CSS-driven cascade, compile-time layout —
drives a family of layouts (flow, grid, sequence, charts, with engineering drawings
next), and compiles to clean, themeable SVG.

**Two brackets and one capsule carry the whole language.** `{ … }` is **style** —
`key: value;` declarations, dash-case, space-separated, exactly like CSS. `[ … ]`
is **content** — a node's children, in source order. `|…|` is **identity** — a
node's type and id. A node is `|type#id| "label" .class { style } [ children ]`;
every part but the bars is optional. Nothing styles outside a `{ }`; nothing is
drawn outside the canvas.

**Two node kinds, like HTML.** A **box** is a drawn node (`|block|`, `|box|`,
`|oval|`, `|group|`, …) and may hold children; a **string** is text *content*
inside or beside one. `"…"` is text, exactly as it sits inside an element on a web
page — stylable in place (`"x" { color: red }`), but a leaf, never a box.

This document is complete: an implementer can build a conforming engine from it
alone. **Everything is defined once and reused** — a property, the cascade, colour,
the expression engine all apply across every node and every layout, and a layout
section states only what is *new* to that layout. **Charts and sequences are
layouts** ([Part II](#part-ii--layout)), peers of flow and grid over the same core.
A future **drawing** layout is named but not yet built ([DRAWING.md](DRAWING.md)); it
slots into the same frame. **Link routing** has its own contract —
[ROUTING.md](ROUTING.md).

---

## Table of Contents

### Part I — Core

1 [Mental Model](#1-mental-model) · 2 [Lexical Syntax](#2-lexical-syntax) ·
3 [Statements & the Label](#3-statements--the-label) ·
4 [Selectors, Cascade & Specificity](#4-selectors-cascade--specificity) ·
5 [The Box Model](#5-the-box-model) · 6 [Paint, Stroke & Text](#6-paint-stroke--text) ·
7 [Nodes](#7-nodes) · 8 [Templates](#8-templates) · 9 [Links](#9-links) ·
10 [Colour, Variables & Expressions](#10-colour-variables--expressions)

### Part II — Layout

11 [The Layout Model](#11-the-layout-model) · 12 [Flow & Grid](#12-flow--grid) ·
13 [Sequence](#13-sequence) · 14 [Charts](#14-charts)

### Part III — Reference

15 [Property Ledger & Support](#15-property-ledger--support) · 16 [SVG Output](#16-svg-output) ·
17 [Compile Pipeline](#17-compile-pipeline) · 18 [CLI](#18-cli) · 19 [Errors](#19-errors) ·
20 [Grammar](#20-grammar) · 21 [Reserved Words](#21-reserved-words) ·
22 [Deferred](#22-deferred) · 23 [Examples](#23-examples)

---

## Quickstart

```
cat -> dog -> bird
```

That's a complete diagram: three boxes, two links. Lini fills in the rest.

| Form | Means |
|---|---|
| `\|type#id\|` | **Identity** — a type, an optional `#id`. Always in bars: an **instance** (`\|oval#cat\|`), a **rule** (`\|oval\| { … }`), a **define** (`\|cat::oval\| { … }`). |
| `"…"` | The **label** — what the node is called, placed by its type (text, a caption, a symbol, a chart title). |
| `{ … }` | A **style block** — `key: value;` declarations. |
| `[ … ]` | A **content list** — a node's children. |
| `.name` | A **class** — define it (`.hot { … }`), wear it (`\|box\| .hot`). |
| `#name` | An **id** — declare it (`\|box#cat\|`), select it (`#cat { … }`), reference it bare (`cat -> b`). |
| `--name` | A themeable **variable** (`fill: --accent`). |
| `a -> b` | A **link**. |

Three defaults make small diagrams trivial:

- Omit the type → `|box|` (a rounded, framed card); `|#cat|` is a default box.
- Omit the label → the box is empty.
- Name an undeclared id in a link → it's auto-created as a labelled `|box|` (`cat -> dog` adds `|box#cat| "cat"`).

**A file is a stylesheet, then drawn statements.** The stylesheet is one `{ }` block at the
top — setup that draws nothing. After it come the instances and links, in source order
(usually instances first, then links — a `layout: sequence` reads the order as time, [SPEC 13](#13-sequence)):

```
{                                               // the stylesheet — setup only
  layout: grid;  columns: repeat(3);  gap: 30;  // scene config
  |box| { radius: 6; }                          // a rule — style every box
  .hot { stroke-width: 2; }                     // a class
}

|box#server| "Server"                           // the canvas, two instances
|box#client| "Client"
server -> client "requests"                     // a link, with a label
```

---

# Part I — Core

The language every node and every layout shares. Read top-to-bottom once; the layout
sections ([Part II](#part-ii--layout)) and the reference ([Part III](#part-iii--reference))
build on it and never restate it.

---

## 1. Mental Model

A Lini file is the body of an implicit **root** container: a **stylesheet** of setup
first, then the drawn **canvas** instances and **links** in source order — and every
statement is exactly one of the three:

| Part | Holds | Drawn? |
|---|---|---|
| **stylesheet** | one `{ }` block: scene config (incl. link & routing defaults), rules, classes, defines | no — it styles |
| **canvas** | instances — boxes (`\|type#id\|`) and text (`"…"`) | yes |
| **links** | `a -> b` connections | yes |

The "is this drawn or styled?" question never arises: **styling lives in the
stylesheet block; drawing lives on the canvas.**

**One character tells a statement's kind.** A leading `|` opens a node, a `"`
opens text, a bare name opens a link, and inside the stylesheet a `.`/`#`/`|…|`
opens a rule. There is no prescan, no ambiguity.

**Two brackets and one capsule, one meaning each.**

- `|…|` — **identity**: a type and an optional `#id`. The *only* place a type
  lives — on an instance (`|box#cat|`), a rule (`|box| { }`), or a define
  (`|treat::box| { }`).
- `{ … }` — **style**: `key: value;` declarations. The *only* place styling lives.
- `[ … ]` — **content**: a node's children (boxes and text) and its internal
  links, in source order.

A drawn node is `|type#id| "label" .class { style } [ children ]`. Only the bars
are required; everything after is optional. A link is the same tail on a different
head: `a -> b "label" .class { style } [ labels ]`.

**Three sigils, one meaning each.**

- `|…|` — a **type** (with an optional `#id`). Always in bars.
- `.name` — a **class**: a worn style bundle. Defined `.hot { … }`, worn after the
  identity (`|box| .hot`, `a -> b .hot`) — never inside the bars.
- `#name` — an **id**: a node's unique name. Declared in the bars (`|box#cat|`),
  selected as a rule (`#cat { … }`), referenced **bare** in a link (`cat -> b`).

A name goes **bare only when referenced**, and the one thing you reference is an
**id** (you link to it). Types and classes are never linked, so they are always
sigil-marked.

**Boxes and text.** A *box* has a type, an id, classes, a style block, and
children. A *string* is text content — no identity or children, but it **may carry
a style block** (`"x" { color: red; translate: 0 -6 }`). A string in a box's `[ ]`
(or trailing the head as its label) is that box's text; a string on its own is a
free-standing text node. Text is a leaf: to give it children, a border, padding, a
`pin`, or a wirable id, put it in a box (a `|block|` is the minimal one) — exactly
like wrapping a web page's text in an element.

**The file is the root container.** The stylesheet `{ }` is the root's own setup
block; the canvas instances are its children (written bare — the file *is* its
`[ ]`); the links are its internal links. Scene properties (`layout`, `gap`,
`padding`, `fill`, `font-size`, `clearance`, `routing`, …) sit in that block, alongside
rules like `|-| { stroke: … }` for link look; inheritable ones (`font-*`, `color`,
`clearance`, `routing`) cascade to every node and link.

**Render order is source order; the cascade is whole-file.** Instances draw in the
order written (later on top, pinned children above the flow; `layer:` overrides),
and every rule applies to every instance. Links are the one thing that needs no
declaration: naming an id declared nowhere auto-creates it ([SPEC 3](#3-statements--the-label)).

**Two kinds of variable.**

- *Visual* values that don't affect layout — colours and the font family — are
  exposed as live CSS variables (`--lini-fill`, `--lini-accent`, …) so a host page
  can re-theme them, and each colour carries a built-in dark variant that follows
  the viewer's OS or a `data-theme` toggle ([SPEC 10](#10-colour-variables--expressions)).
- *Layout* values — sizes, gaps, paddings, widths, **and font size** — bake into
  the SVG as literals. Text is measured at compile time, so its size can never be a
  runtime `var()`; a standalone SVG always looks right.

---

## 2. Lexical Syntax

| Property | Value |
|---|---|
| Extension | `.lini` |
| Encoding | UTF-8 (BOM ignored) |
| Line endings | LF or CRLF (normalized on read) |
| Comments | `// …` to end of line. No block comments. |
| Statement end | A node/link/text statement ends at a newline or `;`. A **declaration** ends at `;` — its value runs to that `;` (or a closing `}`), so a value may span lines. |
| Identifier | `[a-zA-Z_][a-zA-Z0-9_-]*` — case-sensitive, ASCII, dash-case |

Whitespace is insignificant except as a token separator and where a rule below
says otherwise:

| Form | Whitespace rule |
|---|---|
| `\|…\|` | Identity in bars: a type, an optional `#id` (`\|box#cat\|`), or an id alone (`\|#cat\|`). `::` is the define operator (`\|cat::oval\|`). Bars are paired; surrounding space at the boundary is not allowed. |
| `#id` | Inside the bars it names the node's id; at a rule's head it is an **id selector** (`#cat { }`). A `#` followed by hex digits in a *value* is a colour (`#f80`); the two never meet — one heads a statement or sits in bars, the other is a value. |
| `key: value` | `:` separates name and value; surrounding space optional, canonical is one space after (`radius: 5`). |
| `a:side` | A `:` after a link endpoint forces a side (`a:left`). Distinct from the declaration `:` by position — it follows an endpoint, never opens a value. |
| `.name` (class) | At a rule head it is a class **selector** / definition (`.hot { … }`). On an instance or link it is a **worn class**, following the identity — **spaced** off it (`\|box\| .hot`, `a -> b .loud`), the rest of the chain **glued** (`.hot.loud`). |
| `id.child` | **No space** — an endpoint path into a child (`kitchen.bowl`). |
| `--name` | A variable, in a value or at a statement start to declare one. |
| link op | `[marker?] line [marker?]`, glued, no internal space (`->`, `--->`, `<->`). |
| `[ … ]` | A content list. Paired; whitespace inside is insignificant. |

**Strings** — double-quoted UTF-8: `"…"`. Escapes: `\"`, `\\`, `\n`, `\t`. A
double-quoted string is always text; leading and trailing whitespace in its value is
**trimmed** (`" ABC "` is "ABC", and a spaces-only `" "` becomes `""`), so source
spacing never leaks into the render.
Single quotes are **not** strings (reserved, [SPEC 21](#21-reserved-words)).

**A bare word is an identifier, never a string.** In a value, an unquoted word is
always an identifier — a keyword, a colour or `symbol` name, a `font-family`, or an id
reference — so literal **text** is always quoted: a string-valued property (`title`,
`href`, `src`, `path`) takes a `"…"` even with no spaces. The one hybrid is a name that
may contain spaces — `font-family` — bare or quoted, quoted only when needed
(`font-family: "SF Mono"`), as in CSS. Numbers and `` `…` `` expressions are bare too;
only text is quoted.

**Expressions** — a backtick region `` `…` `` is a **compile-time math expression**:
operators and the math library, folded to a literal number (or a point) at compile
time. It is the **only place operators appear** — outside it `-` is a link line and
`<` / `>` are markers. Self-delimiting like a string, and may span lines
([SPEC 10](#10-colour-variables--expressions)).

**Numbers** — integer or decimal, optional sign, no units (px for lengths, degrees
for angles, 0–1 for opacities/fractions). `10`, `-5`, `0.25`, `+3`. A trailing `%`
makes a **percentage** (`50%`), valid only in colour components.

**Values are space-separated and positional**, like CSS: `padding: 5 2 5 5`,
`shadow: 2 2 4 #0003`, `translate: 10 -4`, `columns: 80 140 80`. A **comma**
separates list items and appears only where a property takes a list of groups
(`points: 0 0, 10 10`). **Functions** use parentheses and sit in value position —
`rgb(…)`, `hsl(…)`, `repeat(…)`, the math library, and any you define
([SPEC 10](#10-colour-variables--expressions)); a call needs no backtick (only an operator does).

**Colors** — `#fff`, `#f80c`, `#ffaa00`, `#ffaa00cc` (3/4/6/8 hex digits; the 4-
and 8-digit forms carry alpha), CSS names (`red`, `cornflowerblue`), `rgb(…)`,
`rgba(…)`, `hsl(…)`, `hsla(…)` (percentages allowed — `hsl(200, 50%, 50%)`),
`oklch(L, C, H[, A])` (the palette's own space — L/A in 0–1, C the chroma, H in
degrees; folded to a hex at compile time, so it renders in every target), a
`--name` variable reference, or `none`. Out-of-range channels are an error. Beyond
a flat colour, a **paint** (`fill` / `stroke` / `gap-color`) may be a **gradient** —
`gradient(…)`, `linear-gradient(…)`, or `radial-gradient(…)` — reached, like the
built-in hue palette, through the colour system ([SPEC 10](#10-colour-variables--expressions)).

---

## 3. Statements & the Label

A file is a **stylesheet, then drawn statements in source order** ([SPEC 1](#1-mental-model)), and
a container's body nests the same idea: a `{ }` style block, then a `[ ]` of children and
internal links.

### The stylesheet

One `{ }` block at the very top of the file — optional, omitted when there is
nothing to set up. Unlike an ordinary style block (declarations only), it is the
root's setup block, so it additionally holds the file-global definitions:

| Item | Form | Means |
|---|---|---|
| Scene config | `layout: grid;` `routing: orthogonal;` | a declaration on the root — `clearance` / `routing` cascade to every link ([SPEC 9](#9-links)) |
| Variable | `--brand: #f60;` | a themeable visual variable (colour / font) |
| Function | `scale(n) …` | a reusable compute function — a backtick body ([SPEC 10](#10-colour-variables--expressions)) |
| Rule | `\|box\| { … }` | style every box (an element selector) |
| Link rule | `\|-\| { stroke: #666; }` | style every link — the `\|-\|` selector ([SPEC 9](#9-links)) |
| Descendant rule | `\|table\| \|box\| { … }` | style every box inside a table |
| Class | `.hot { … }` | define class `hot` |
| Id rule | `#hero { … }` | style the one node with id `hero` |
| Define | `\|treat::box\| { … }` | a new type `treat`, base `box`, with its defaults |

```
{
  gap: 16;  fill: --bg;
  --brand: #ff6600;
  scale(n) `100 * 1.2^n`;
  |box| { radius: 6; }
  |-| { stroke: #666; }
  .hot { stroke-width: 2; }
  |treat::box| { radius: 5; }
}
```

`|treat::box|` reads "treat **is a** box"; the `::` sets a define apart from a
plain reference (`|box|`) at a glance. Defines chain (`|panel::treat|`) and may
carry intrinsic children ([SPEC 9](#9-links)). Max inheritance depth 16; cycles are an
error.

### Node declaration

```
|type#id| [ "label" ] [ .class… ] [ { style } ] [ [ children ] ]
```

The **bars are identity** — a type and an optional `#id`. The **`"label"`** is the
node's name; the **`.class`es** are worn styling; the **`{ }`** is style; the
**`[ ]`** is content. Only the bars are required; at least a type or an `#id` must
sit inside them.

A node's **type and id live in the bars**, its **classes follow** them:
`|oval#cat|`, `|box| .hot` (a box with class `hot`), `|box| .hot.loud` (two
classes), `|#cat|` (a default box with id `cat`).

```
|cyl#db| "Postgres" .primary { fill: #eef } [
  |badge| "v16"
]
```

| Form | Effect |
|---|---|
| `\|box#cat\|` | a box, id `cat` (empty — no label). |
| `\|treat#cat\|` | type `treat`, id `cat`. |
| `\|treat#cat\| "Friendly cat"` | + label "Friendly cat". |
| `\|treat#cat\| { fill: red }` | + a style block. |
| `\|box#cat\| ""` | same as `\|box#cat\|` — `""` is just an empty string. |
| `\|box#cat\| .bold.loud { padding: 5 }` | type + id + classes + own style. |
| `\|group#garden\| { … } [ … ]` | container with style and a body. |
| `\|box\| "Load balancer"` | anonymous labelled box (can't be linked to). |
| `\|#cat\|` | a default `\|box\|`, id `cat`. |

### The label

A node has **no label unless you give it one** — a bare `|box#cat|` is an empty box
(the `#cat` is a handle, like HTML's `id=`, not text):

| Label | Means |
|---|---|
| no string at all | nothing — an empty box |
| `"X"` | the label "X" |
| `""` | an empty string — nothing in flow, an empty cell in a grid ([SPEC 12](#12-flow--grid)) |

A link to an *undeclared* name still draws a labelled box: `cat -> dog -> bird`
desugars to three boxes labelled "cat"/"dog"/"bird" ([Implicit nodes](#implicit-nodes)). A multi-word label needs no `[ ]`: `|box#lb| "Load balancer"`; an
*anonymous* labelled box needs no id: `|box| "Load balancer"`.

**The label is smart — each type places it.** The same `"X"` does the most useful
thing for the shape it sits on. This **one rule** is extended by every layout — a
chart's label is its title, a series' its legend entry ([SPEC 14](#14-charts)) — so no type
needs a hand-written caption or symbol:

| `"X"` on | becomes |
|---|---|
| `\|box\|` and the shapes (`\|oval\|`, `\|hex\|`, `\|cyl\|`, `\|diamond\|`, …) | its centred text |
| `\|group\|` / `\|table\|` | its **caption** ([SPEC 8](#8-templates)) |
| `\|icon\|` / `\|sign\|` | its **symbol** — `\|icon\| "heart"` is `\|icon\| { symbol: heart }` |
| a **link** | a label along the route ([SPEC 9](#9-links)) |
| a `\|chart\|` / series / `\|axis\|` / participant / frame | its title / legend / axis title / header / guard ([SPEC 13](#13-sequence), [SPEC 14](#14-charts)) |

Because a group's label is its caption, `|group#kitchen| "Kitchen" [ … ]` needs no
hand-written `|caption|`; because an icon's label is its symbol, `|icon| "bell"`
needs no `{ symbol: … }`. Give no label and a type places nothing — one rule, no
per-type exception.

**The label takes no style of its own.** The `{ }` after the head is the *node's*
block, so a styled or nudged label rides the `[ ]` content form instead, where each
string is a leaf in its own right ([Text content](#text-content)):

```
|box#api| "API" .hot { fill: red }        // label + class + the node's own style
|box#api| [ "API" { translate: 0 -6 } ]   // a styled label, via content
```

**The label and `[ ]` coexist.** The label is the node's one inline item, lowered by
its type — a text or caption child prepended to the `[ ]`, or (for `|icon|`/`|sign|`)
the `symbol` — and the `[ ]` holds the rest:

```
|group#kitchen| "Kitchen" [ |box#bowl| "Bowl" ]   // caption + a child
|icon| "bell" [ "3" ]                              // symbol + a text badge
```

One inline label only — two or more strings go in the `[ ]`.

### Text content

A string is a **text node** — always a `<text>` leaf, never wrapped:

- In a box's `[ ]` (or as the box's label) it is that box's text — centred when it
  is the only in-flow child, else a flow child laid out by the box's `layout`.
- On its own (on the canvas, or in a `[ ]`) it is a free-standing flow / canvas
  text node.
- Several strings are several text nodes — `"a" "b" "c"` is three (a string is
  self-delimiting, so no `;` is needed between them).
- An empty `""` is suppressed (adds no text) — except as a **grid cell**, where it
  holds its track ([SPEC 12](#12-flow--grid)).
- Multi-line text uses `\n`; the box sizes to the widest line, with a
  `font-size × 1.2` leading between lines (plus any `line-spacing`).

A string carries **no children** — text is a leaf, not a box — but where it is
**content** (free-standing, or a child in a `[ ]`) it **may carry a style block** of
text properties: `"X" { color: red; font-weight: bold; translate: 0 -6;
rotate: 12 }`. Only text-valid properties apply (colour, every `font-*`, `opacity`,
`letter-spacing`, `line-spacing`, `text-transform`, `text-decoration`, `translate`,
`rotate`, `layer`); any other — `pin`, `padding`, `width`, a border, children, even
`href` / `title` — needs a real box, so wrap the text in a `|block|`. Set on the
string the style applies to it directly; set on a containing box it cascades down
([SPEC 6](#6-paint-stroke--text)). A string in the **label** position is the one place it is
not content but a shorthand for it, so it takes no style block — write it in `[ ]`
to style it (above).

### Implicit nodes

A link endpoint that is a **single bare id** not present in the link's **scope**
auto-creates the node `|box#cat| "cat"` in that scope — a box named `cat`, labelled
"cat" — so `cat -> dog -> bird` is a complete three-box diagram. The same holds inside
a container body: a body link auto-creates its missing endpoints among that body's own
children. Declaring the id in the scope — before or after the link — uses it instead
of creating one. A **path** endpoint (`kitchen.bowl`) is never auto-created: it must
resolve to an existing node, or it is an error. If a same-named node exists elsewhere
in the tree, the box is still created here and a warning names the other match.

### Declarations

A declaration `key: value;` lives only in a `{ }` style block — the stylesheet
(configuring the root) or a node's own block. Property names are dash-case; values
are space-separated and positional. A declaration **ends with `;`** — its value runs
to that `;` (or the block's closing `}`), so a value may span several lines (a long
expression, a per-segment list); the `;` is optional only immediately before `}`. A
bare `key: value` outside a `{ }` is an error. Every property, its value shape, and
where it applies is in the [Property Ledger](#15-property-ledger--support).

---

## 4. Selectors, Cascade & Specificity

A **rule** is `selector { declarations }`. A selector is one or more
space-separated **units**; the space is the descendant combinator. A unit is a type
`|box|` (with an optional `#id`, `|table#main|`), the **link type `|-|`**, a class
`.hot`, or an id `#hero`:

```
|box| { … }              // every box (element selector)
|-| { … }                // every link — a line in the identity capsule ([SPEC 9](#9-links))
.hot { … }               // every node with class .hot
#hero { … }              // the one node with id hero
|table| |box| { … }      // every box inside a table (descendant)
#g |-| { … }             // every link written in #g
.sidebar |box| { … }     // every box inside a .sidebar
|table| .hot { … }       // every .hot inside a table
```

A **descendant selector** matches a node (or link) whose ancestor chain contains each
unit in order (not necessarily adjacent), exactly like CSS's descendant combinator.
Every construct keeps its sigil — `|box|`, `|-|`, `.hot`, `#hero` — so a selector reads
as a run of marked units; a bare word is never a selector. `|-|` is selector-only: a
link is drawn by an operator, never instantiated ([SPEC 9](#9-links)).

A type's class never glues into its bars (`|box.hot|` is rejected): a class is
**worn**, not part of identity. To match boxes-with-a-class, style the class
(`.hot { … }`); to match within one, use a descendant (`.hot |box|`).

A **define** introduces a new type from a base: `|treat::box| { … }`. Its
declarations are the type's defaults; an optional `[ ]` gives it intrinsic children
(materialized per instance — see [SPEC 9](#9-links)).

A **class** is defined by `.name { … }` and **worn** by writing it after the
identity (`|box| .hot`) or after a link's endpoints (`a -> b .hot`) — the same
`.class` slot on both, never inside the bars.

**Selecting vs. drawing is decided by the section, not the syntax.** `|box| .hot`
in the stylesheet is a descendant *rule* (.hot inside a box); on the canvas it is
an *instance* (a box wearing .hot). One reads as a selector, the other draws —
because rules live in the stylesheet and instances on the canvas.

### The cascade

Properties on a node merge like CSS — **the more specific source wins**, ties broken
by **later wins** (source order). The tiers, low to high:

1. **Type cascade** — walked from the base primitive up to the node's declared type,
   layering each type's element-rule (`|box| { }`) and define defaults. A more-derived
   type overrides what it builds on. (This is where a template's and a define's baked
   defaults live — [SPEC 8](#8-templates).)
2. **Descendant rules** — `|table| |box| { }`, `.sidebar |box| { }`, matched against
   the ancestor chain.
3. **Class rules** — `.hot { }`, worn via `|box| .hot` on the node.
4. **Id rule** — `#hero { }`, the node's own id.
5. **The instance's own block** — `|box#client| { fill: white }` — the most specific,
   beats everything above.

A link walks the **same ladder** — its type is `|-|`, its ancestors are its scope's
container chain, it has no id: the baked link base plus the scope's `clearance` /
`routing` (tier 0), the `|-|` element rule (type), descendant `|…| |-|` and worn-class
rules, then the link's own block ([SPEC 9](#9-links)).

**Complex values replace wholesale.** The merge is per-property, not deep:
`translate: x y` or `padding: t r b l` on a higher tier replaces the whole value from a
lower one, never blending component-by-component. A `pin`ned child ignores `cell:` —
pinning takes it out of the grid ([SPEC 5](#5-the-box-model)).

Inheritable properties (the text family, `color`, `clearance`, `routing`) additionally
flow **down** the tree — nearest ancestor wins — independent of the specificity tiers
above ([SPEC 6](#6-paint-stroke--text)).

---
## 5. The Box Model

A node's **bounding box** is the smallest axis-aligned rectangle containing it,
stroke included.

1. **Center origin.** Every bbox is centered at the parent's origin by default.
2. **Source order = render order;** later draws on top, with pinned children above
   the in-flow ones. `layer: N` overrides; ties break by source order.
3. **Strokes count** toward the bbox — `width: 100 height: 50 stroke-width: 4` →
   104×54.
4. **`|path|`** is the only center-origin exception — `path:` uses native top-left
   coordinates.
5. **Rotation** applies last as an SVG transform; the rotated bounding rectangle
   propagates upward.

### `pin` — out of the flow

Every child is **in flow** by default — laid out by its container's `layout`
([SPEC 11](#11-the-layout-model)). **`pin` lifts a child out**, aligning the child's
**matching point** flush with a named point of the parent:

| `pin:` | The child sits… |
|---|---|
| `none` *(default)* | — in flow; nothing is pinned |
| `center` | centre on the parent's centre |
| `top` · `bottom` · `left` · `right` | flush against that parent edge |
| `top left` · `top right` · `bottom left` · `bottom right` | with its corner on that parent corner |

The child's *own* matching point lands on the parent's, so it sits **flush**. The
anchor is the parent's **drawn box** — border and padding included. Corners fall out
of the value, so one switch covers every anchor.

A pinned child is an **overlay**. It **does not grow the parent** — a parent of only
pinned children collapses to `2 × padding` — and it **paints above** the in-flow
children, so a badge needs no explicit `layer`. The canvas always includes it, so an
overlay is never clipped. Set `layer:` to reorder overlapping pins, or to push one
*beneath* the flow.

### `translate` and `rotate` — the universal nudge and turn

**`translate: x y`** shifts a node by (x, y) *after* it is placed. It works on
**every** node — flow children, pinned children, text nodes, the root alike — and is
layout-neutral: siblings don't move, the parent doesn't grow, no size changes. It is
CSS's standalone `translate`, baked into the node's origin (so a standalone SVG needs
no transform variable); the canvas still includes the shifted node.

There is **no numeric coordinate property**. Because the parent's origin is its
center, `pin: center` + `translate: x y` lands a child's center at parent-local
(x, y) — explicit coordinates with no node-size arithmetic.

**`rotate: N`** turns a node N degrees about its bbox center, applied last as an SVG
transform. Like `translate`, it works on **any** node, text included — so a link label
or a stray string can be nudged or turned in place. `pin` (which needs a parent anchor
and takes a child out of the flow) is a **box** job; to pin text, wrap it in a `|block|`.

### Auto-sizing

`width` and `height` default to **`auto`** — the bbox sizes to its content (text or
child nodes) **plus `padding` on each side** (default 20 on a framed box; there is no
separate text padding). Sizing is **border-box**: padding sits *inside* the box, never
added on top, and the two axes are independent. An explicit `width` / `height` is a
**floor** — the box is exactly that size when its content fits, and grows past it (to
`content + padding`) when the content is larger, so a box never clips or spills its
content. A box with no in-flow content — empty, or holding only `pin`ned overlays —
has nothing to grow for: an explicit size stands exactly as written, and an **auto**
one falls to **`2 × padding`** on each axis (the default `padding` 20 gives a 40 × 40
minimum).

**Padding also places the content.** The content area is the box inset by `padding`,
and the content sits within it; symmetric padding centres it, while an asymmetric
`padding: t r b l` offsets it — `padding: 4 4 20 4` lifts the content toward the top,
away from the larger bottom inset, exactly like CSS.

Exceptions: a **text** node sizes to its glyphs (no padding), widened by
`letter-spacing` and given `line-spacing` between `\n` lines; `|icon|` is a square
that grows with its `[ ]` text (a `32` floor) and needs a `symbol`; `|line|` / `|poly|` /
`|image|` / `|path|` require their geometry (`points` / `src` / `path`) and error
without it. `|block|` carries `padding: 0`, so a bare block sizes to its content
exactly.

Text width uses one advance per character (≈ 0.6 em). The default font is monospace,
so this is essentially exact; a proportional `font-family` override makes it
approximate until embedded font metrics land ([SPEC 22](#22-deferred)).

---

## 6. Paint, Stroke & Text

The visual vocabulary shared by every node. These are ordinary properties — the full
list, with value shapes and defaults, is the [Property Ledger](#15-property-ledger--support);
the colour system they draw on is [SPEC 10](#10-colour-variables--expressions). This section
is the *behaviour*.

### Paint

**`fill` paints a body, `color` a label.** `fill` is a closed shape's interior (and,
on text, an alias for its `fill`); `color` sets text colour for a subtree and
cascades through the SVG via native `currentColor` — set it on a container to recolour
every descendant's text that doesn't override. `opacity` (0–1) fades a node whole.
`fill`, `stroke`, and `gap-color` each accept a **gradient** as well as a flat colour
([SPEC 10](#10-colour-variables--expressions)).

### Stroke

**One stroke role paints a shape's outline and a link's wire alike** — `stroke` the
colour, `stroke-width` the thickness (markers scale with it), `stroke-style` the dash
pattern (`solid` / `dashed` / `dotted`, and `wavy` on links). There is no parallel
`link-*` family: a `.class` carrying `stroke` dresses whichever wears it, node or link
([SPEC 9](#9-links)). A closed primitive's default outline is `--stroke` at width 2; a
`|group|` softens to width 1.

### Text

The text family — `font-family`, `font-size`, `font-weight`, `font-style`,
`text-transform`, `text-decoration`, `letter-spacing`, `line-spacing`, and `color` —
**inherits**: nearest ancestor wins, like CSS. Set it on a containing box (or the root)
and it cascades down, or set it on a string's own block (`"x" { font-weight: bold }`)
for that one text node. Style globally with `font-size:` etc. in the stylesheet, or
scope it on a container. Body text defaults to `font-size` 15, `font-weight` `normal`;
captions 12 and link labels 11 carry their own baked defaults.

Two kinds of text property, split by whether they touch layout:

- **Baked spacing** — `letter-spacing`, `line-spacing`, and `font-size` — changes
  **layout** (the text box grows to fit the wider glyphs or taller block) and compiles
  into the glyph and line positions, never emitted as a style. `font-size` can never be
  a runtime `var()` — text is measured at compile time. `letter-spacing` / `line-spacing`
  default to 0, so text is unaffected until set.
- **Live CSS** — `font-style`, `text-transform`, `text-decoration` — does *not* touch
  layout: it rides the class / `<g>` / `.lini` rule and a host page can override it. Set
  any in the global block to style the whole scene.

For a global `font-family` / `color`, prefer the `--lini-font-family` /
`--lini-text-color` variables (or a `--theme`) for an **embeddable** diagram — they stay
live for a host page to re-theme, where a global property bakes its value into the
`.lini` rule ([SPEC 10](#10-colour-variables--expressions), [SPEC 16](#16-svg-output)).

---

## 7. Nodes

11 primitives. All accept position ([SPEC 5](#5-the-box-model)) and paint ([SPEC 6](#6-paint-stroke--text));
closed primitives also accept `stack`, `rotate`, `shadow`. Text is **not** a primitive —
it is bare content ([SPEC 3](#3-statements--the-label)); the frameless `|block|` box
([SPEC 8](#8-templates)) is what you reach for when text needs an id, a class, a link, or box
layout.

**Dimensions** use `width` / `height`, each defaulting to `auto` (content + padding,
**border-box** — [SPEC 5](#5-the-box-model)). They are always **bbox dimensions**:
`|oval| { width: 60; height: 40 }` is an ellipse in a 60×40 box; equal dimensions (or an
empty `|oval|`) make a circle.

| Primitive | Required | Notes |
|---|---|---|
| `\|block\|` | size (auto) | The base rectangle — frameless (no fill/stroke, `radius: 0`, `padding: 0`), like a `div`. It keeps `stroke-width: 2` (invisible while `stroke: none`), so a styled block gets a sensible border. `\|box\|` frames + rounds it, `\|rect\|` frames it sharp ([SPEC 8](#8-templates)). |
| `\|oval\|` | size (auto) | Bbox ellipse; equal width/height = circle. |
| `\|hex\|` | size (auto) | Regular hex, flat top/bottom. |
| `\|slant\|` | size (auto) | Parallelogram; top edge shifted `tan(skew) × h`. `skew` in degrees, (-89, 89), default 15. |
| `\|cyl\|` | size (auto) | Cylinder; end ellipses ≈ h/10. |
| `\|diamond\|` | size (auto) | Rhombus inscribed in the bbox. |
| `\|poly\|` | `points` | ≥3 points, local (center-origin) coords. Closed. |
| `\|path\|` | `path` | Raw SVG path. **Native top-left coords.** |
| `\|line\|` | `points` | 2+ points. Markers via `marker*:`. |
| `\|icon\|` | `symbol` | A **Phosphor** icon — `symbol:` (or the label) names it; paints two-tone like a box (`fill` body, `stroke` line, counter-scaled `stroke-width`). A square that grows with its `[ ]` text (`32` floor); `\|sign\|` is the larger preset. See [Icons](#icons). |
| `\|image\|` | `src`, `width`, `height` | `<image href="…">`. External URLs only; both dimensions required. `fit` maps it into the box — `auto` (default, letterbox), `contain`, `cover`, or `stretch`. |

**`radius`** rounds a rectangle's corners — `|box|` defaults to 8, `|block|` / `|rect|`
to 0. It is honoured on the rectangle (and on a multi-point `|line|`'s joins); `radius`
on the non-rect primitives (hex / diamond / slant / poly) is deferred ([SPEC 22](#22-deferred)).

### Visual modifiers (closed primitives)

| Property | Forms | Effect |
|---|---|---|
| `stroke-style` | `solid` / `dashed` / `dotted` | Stroke pattern. Default `solid`. (`wavy` draws on links — [SPEC 9](#9-links); on closed primitives it is deferred — [SPEC 22](#22-deferred).) |
| `stack` | `N` / `dx dy` | Draw an offset duplicate behind the node. Scalar `N` = `N -N`. |
| `rotate` | `N` degrees | Rotate around the bbox center ([SPEC 5](#5-the-box-model)). |
| `shadow` | `N` / `dx dy` / `dx dy blur` / `dx dy blur color` | Drop shadow via SVG `<filter>`. Scalar `N` = offset `N N`, blur `N`; tint defaults to `--lini-shadow-color`. |

### Markers (on `|line|` and links)

| Property | Effect |
|---|---|
| `marker: X` | Both ends. |
| `marker-start: X` | Start end (link source). |
| `marker-end: X` | End end (link target). |

Values: `none`, `arrow`, `dot`, `circle`, `diamond`, and the ER **cardinality set** —
`crow` (the "many" foot), `one` (a bar `|`), `zero-or-one`, `one-or-many`, `zero-or-many`
(a bar or `○` paired with the foot). `circle` is a larger `dot` — a filled point sized for
hovering or reading (on a chart line it marks a data point; [SPEC 14](#14-charts)). Markers scale
with `stroke-width` (a link's wire and a shape's outline alike), floor 5 px; colour follows
the stroke.
`|line|` is bare by default — write `|line| { marker-end: arrow }` for a one-shot
arrow. For links the operator picks markers (see [SPEC 9](#9-links)). Source order wins:
`marker: arrow; marker-end: dot` → start arrow, end dot.

### Icons

`|icon|` draws a **[Phosphor](https://phosphoricons.com/)** icon (MIT) as inline SVG
paths — themeable, reproducible, and renderer-agnostic (no icon font). The `symbol`
property names it — or, as the [smart label](#the-label), the string does (`|icon| "heart"` is
`|icon| { symbol: heart }`); everything else paints like a box:

```
|icon| "bell"                                          // symbol via the label
|icon| { symbol: warning-circle; stroke: --amber-ink } // the longhand
|icon| "heart" { fill: --rose-wash; stroke: --rose-ink }
|icon#tag| "bell" [ "3" ]                              // symbol bell, "3" rides as text
```

Setting the symbol twice — a label *and* `{ symbol: … }` — is an error; pick one. A
text label on an icon rides in the `[ ]` (`|icon| "bell" [ "3" ]`).

Phosphor icons are **two-tone** (a soft fill behind a line), so an icon wears Lini's
paint roles like any node: **`fill`** paints the body, **`stroke`** the line,
**`stroke-width`** its weight. The defaults make the duotone read out of the box —
`fill` a soft grey (`--icon-fill`), `stroke` the ink (`--stroke`, matching borders
and wires), `stroke-width` 2. A single-tone line icon is `fill: none`; a hued duotone
is `fill: --teal-wash; stroke: --teal-ink`, exactly like a card.

`stroke-width` is **counter-scaled**: an icon is authored on a 256-unit grid and fit
to its box, and the stroke is divided by that scale (baked at compile time), so its
line weight holds as the icon resizes and matches the diagram's other strokes.

An icon is a **square** that grows uniformly with its `[ ]` text (and `padding`): the
side is a `32` floor (`icon-size`) over the text + padding on either axis, so an
empty icon is 32×32 and a longer label scales the **whole icon up** — symbol and all
— keeping its proportion (the symbol never distorts). For a larger stand-alone icon,
reach for `|sign|` ([SPEC 8](#8-templates)).

**`fit`** controls how the symbol fills that box. By default (`fit: auto`) an icon
keeps Phosphor's authored framing — each glyph sits in the 256-grid with its own
built-in margin, so different glyphs fill the box by different amounts and a row of
mixed icons reads at an even weight. `fit: contain` scales the glyph's *own* bounds
up until they meet the box (filling it — and `|sign|` defaults to it); `cover` scales
until the box is covered (the glyph may overflow); `stretch` fits both axes (may
distort). The counter-scaled `stroke-width` follows the resulting scale, so the line
weight stays constant whichever `fit` you choose.

A missing `symbol` errors like `|poly|` without `points`; an unknown one suggests the
nearest name. Only the icons a diagram uses are embedded (a default-on `icons` feature,
[SPEC 22](#22-deferred)).

---

## 8. Templates

Built-in types — each a bundle over a primitive base, named because the pattern is
common. **Every rectangular template is a bundle over `|block|`**; the non-rect
primitives ([SPEC 7](#7-nodes)) stand on their own. A template's defaults are the low tier of
the cascade ([SPEC 4](#4-selectors-cascade--specificity)) — every value here is overridable.

| Template | Base | Defaults | For |
|---|---|---|---|
| `\|box\|` | `\|block\|` | `fill: --fill; stroke: --stroke; stroke-width: 2; radius: 8; padding: 20` | The **default** node — a rounded, framed card. |
| `\|rect\|` | `\|box\|` | `radius: 0` | A sharp-cornered box. |
| `\|group\|` | `\|block\|` | `stroke: --group-stroke; stroke-style: dashed; stroke-width: 1; fill: --group-fill; radius: 8; padding: 20` | Dashed frame for a caption + children. |
| `\|caption\|` | `\|block\|` | `pin: top left; translate: 0 -18; color: --caption-color; font-size: 12; font-weight: --caption-font-weight` | A title, pinned just above the group's top-left corner. |
| `\|footnote\|` | `\|caption\|` | `pin: bottom; translate: 0 17; font-size: 11; color: --footer-color` | A caption flipped to a shape's bottom edge — a centred, muted footnote. |
| `\|badge\|` | `\|block\|` | `pin: top right; translate: 6 -6; radius: 8; padding: 2 6; shadow: 2 3 3; fill: --accent; color: --accent-text; font-size: 11; font-weight: normal` | Corner pill — nudged out over the top-right corner, grows nothing. |
| `\|row\|` | `\|block\|` | `direction: row` | Frameless wrapper — children in a row. |
| `\|column\|` | `\|block\|` | `direction: column` | Frameless wrapper — children in a column. |
| `\|grid\|` | `\|block\|` | `layout: grid` | Frameless grid (needs `columns`). |
| `\|sign\|` | `\|icon\|` | `width: 64; height: 64; padding: 4; stroke-width: 2; fit: contain` | A larger icon as a stand-alone node, with room for a short label; `fit: contain` fills the box (unlike a bare `\|icon\|`). |
| `\|table\|` | `\|group\|` | `layout: grid; align: stretch; justify: stretch; gap: 1; gap-color: --stroke; padding: 0; fill: none; stroke: --stroke; stroke-width: 2; stroke-style: solid; font-size: 14; font-weight: normal` | Ruled grid (see below). |
| `\|cell\|` | `\|block\|` | `padding: 4 8` | A **table cell** — a frameless `\|block\|` carrying the text-to-gutter inset. Body cells wrap in it; `\|header\|` / `\|footer\|` build on it. Style all cells with `\|cell\| { … }` or, per table, `\|table\| \|cell\| { … }`. |
| `\|header\|` | `\|cell\|` | `fill: --header-fill; font-weight: bold` | A **header** cell — a filled, bold band (a `\|table\|`'s first row; an `\|entity\|`'s title spans them). |
| `\|footer\|` | `\|cell\|` | `color: --footer-color` | A **footer** cell — muted text; opt-in on the last row. |
| `\|entity\|` | `\|table\|` | `columns: auto auto` | An ER / database **entity** — a titled, two-column field list (see below). |

The bare `|block|` is the base everything rectangular builds on — frameless, yet a real
box (id, class, children, wirable, positionable): what you reach for to wrap text that
needs box behaviour.

**Captions.** A `|caption|` is a small `|block|` **pinned** just above the group's
top-left corner; a `|footnote|` is the same flipped to the bottom. Both are out-of-flow
overlays, so they never push the content, and their place is fixed by the template,
not by where they sit among the children. A group's **label is its caption** ([SPEC 3](#the-label)),
so the two forms are equal:

```
|group#panel| "Settings" [          // label → caption
  |box#a| "General"
  |box#b| "Network"
  |footnote| "synced"
]
|group#panel| [                     // the explicit form
  |caption| "Settings"
  …
]
```

Style every caption globally with `|caption| { font-size: 16; font-weight: bold }` —
that targets captions without touching body text. Because a caption is pinned (not in
flow), a group laid out as a `row` carries its title just the same.

**Tables.** A `|table|` is sugar — a `group` that is a grid with `gap: 1` and
`gap-color: --stroke`, so the 1px gaps between cells paint as hairline rules
([SPEC 12](#12-flow--grid)). Each body cell wraps in a `|cell|`, the type that
carries the text-to-gutter inset (`padding: 4 8`); `|header|` / `|footer|` build on
`|cell|`, so every cell — but not the caption, a plain `|block|` — is inset. Style all
cells with `|cell| { … }`, or per table with `|table| |cell| { … }`, exactly as you
style headers with `|table| |header| { … }`. The table sets `align: stretch;
justify: stretch`, so **every cell fills its track** — backgrounds fill and text has
room. The outer frame is the group border, the inner rules the gap paint
([SPEC 11](#11-the-layout-model)); no edge is doubled. A table's label is its caption.

**Column alignment.** `align` (↔) / `justify` (↕) on the table read per column
([SPEC 12](#12-flow--grid)) and align the *cells' text*: since the cells already fill, the
table's own `align`/`justify` are carried onto each cell — a `start`/`end` column's cells
wear a `.lini-align-*` / `.lini-justify-*` class — and a filled cell places its text at
that edge (`center` is the default). So `align: start center end` reads three columns
left / centre / right, header band and body alike.

A table's **first row becomes its header** — each cell wrapped as a `|header|`, a filled
bold band; `|table| |header| { font-weight: normal; fill: none }` reverts it. A **footer**
is opt-in: wrap a last-row cell in `|footer|`. Every cell is a box now — header/footer
carry a fill; a body cell is a frameless `|block|` wrapping its text, so the padding rule
and the column's alignment reach it ([SPEC 16](#16-svg-output)).

```
|table#basket| {
  columns: 80 140 80;
} [
  "Fruit" "Quantity" "Notes"   // the header row — filled + bold
  "Apple" "12"       "fresh"
  "Mango" "3"        "ripe"
]
```

`fmt` knows the column count and pads the cells into aligned columns, so the flat form
reads like the table it is. A cell that must be placed or linked is a **box** child
(`|cell| "X"` for a padded cell, or `|box| { cell: 2 1; … }`); a cell that just needs a
colour or weight can take its own style block (`"Apple" { color: --red-ink }`).

**Entities.** An `|entity|` is sugar over `|table|` (two auto columns) for an ER / database
card: its **label is its title** — a `|header|` spanning every column — over `"field" "type"`
rows. In an entity (not a plain table) a `|header|` / `|footer|` cell spans the full width.

```
|entity#users| "Users" [ "id" "int"  "name" "varchar" ]
```

Relationships are ordinary links ([SPEC 9](#9-links)): `users -< orders` is one-to-many, `a >-< b`
many-to-many, landing on the entity edge. To anchor a wire to one **field**, give that cell an
id (`|block#user_id| "user_id"`) and link the path (`orders.user_id -< users.id`). Keys are
plain content (`"id" { font-weight: bold }`); an entity adds no grammar.

Extend any template: `|panel::group| { stroke: --accent }`. Common nodes need no
template:

| For | Write |
|---|---|
| Circle | `\|oval\| { width: 40 }` |
| Database | `\|cyl\|` |
| Arrow | `\|line\| { marker-end: arrow; points: 0 0, 50 0 }` |

---
## 9. Links

A link connects scene-node ids with an operator (`a -> b`). Like every node it has a
`{ }` **style** and a `[ ]` of **content** — its content is its **labels** (text),
placed along the route by `along:`. It is never written as a `|link|` instance; the
operator draws it.

A link is **styled like a node**: its type is `|-|` — a line in the identity capsule,
the one selector that matches every link — so `stroke` is its wire and `color` /
`font-*` its labels, the ordinary vocabulary ([SPEC 6](#6-paint-stroke--text)) with no
parallel family. Only **`clearance`** and **`routing`** stay scene config — geometry,
not paint — set on a container's `{ }` and cascading to its links.

### Operators

A link op is `[start_marker?][line][end_marker?]`, no spaces:

| Part | Tokens |
|---|---|
| Line | `-` solid · `--` dashed · `---` dotted · `~` wavy |
| Start markers | `<` arrow · `>` crow · `*` dot · `<>` diamond |
| End markers | `>` arrow · `<` crow · `*` dot · `<>` diamond |

The line grows more broken as it lengthens — solid `-`, dashed `--`, dotted `---`.
The same marker glyph differs by position (`<` is arrow at the start, crow at the
end).

| Op | Markers / Line |
|---|---|
| `->` `<-` `<->` | arrow combinations, solid |
| `-*` `*-` `*-*` | dot combinations |
| `-<>` `<>-<>` | diamond |
| `-<` `>-<` | crow |
| `-->` `--->` `~>` | dashed / dotted / wavy |
| `-` `--` `---` `~` | no markers (each line style) |

If the operator carries no markers, there are none on both ends. Explicit `marker:` /
`marker-start:` / `marker-end:` override the operator (source order wins). The
operator's line part sets the link's `stroke-style` (`--` ⇒ `dashed`, `---` ⇒ `dotted`,
`~` ⇒ `wavy`); an explicit `stroke-style:` overrides it.

`-<` / `>-<` draw the ER **crow's-foot** ("many"); the finer cardinalities ([SPEC 7](#7-nodes)) are
set via `marker*:`, with no operator spelling ([SPEC 22](#22-deferred)).

### Syntax

```
endpoints op endpoints [op endpoints …] [ "label" ] [ .class… ] [ { style } ] [ [ labels ] ]
```

The tail is the **node tail** (`"label" .class { style } [ … ]`); only the head differs
— endpoints + operators, versus bars — and a link's `[ ]` holds only labels (text),
where a node's holds children.

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
follows its identity (`|box| .loud`) — one `.class` slot, after the head, on both; a
class never lives in the bars. On a chain or fan, the label, class, and `{ }` apply to
every link the statement expands to.

### Styling

**`stroke` is the wire; `color` / `font-*` the labels** — the same `stroke` paints a
node's outline and a link's wire ([SPEC 6](#6-paint-stroke--text)), so a class carrying it
dresses either:

| Property | Type | Default | Role |
|---|---|---|---|
| `stroke` | colour | `--stroke` | The wire's colour. |
| `stroke-width` | number | 2 | Wire thickness; markers scale with it. |
| `stroke-style` | `solid` / `dashed` / `dotted` / `wavy` | from the operator | The dash pattern; usually set by the op (`-->` ⇒ dashed), overridable here. |
| `color` · every `font-*` · `letter-spacing` · … | — | inherits / baked | The labels ([Labels](#labels)). |

`|-| { … }` styles every link; a descendant (`#g |-|`, `|table| |-|`) or a worn class
scopes it, exactly as `|box|` / `#g |box|` / `.hot` scope a node; a link's own `{ }`
overrides — the same cascade a node walks ([SPEC 4](#4-selectors-cascade--specificity)):

```
{
  |-| { stroke: #888; stroke-width: 1.5; font-size: 12 }   // every link
  #g |-| { stroke: --blue }                                // links written in #g
  .flow { stroke: --teal }                                 // a worn class — nodes or links
  clearance: 12; routing: orthogonal                       // scene config, cascades to links
}
a -> b .flow "hi" { stroke: red; stroke-style: dashed }    // one link overrides
```

`|-|` is **selector-only**: a link is drawn by an operator, so `|-|` never appears as an
instance ([SPEC 19](#19-errors)). `clearance` (default 16) and `routing` (default
`orthogonal`) are **scene config** — geometry, not paint — set on a container's `{ }`,
cascading to that scope's links, nearest winning; `marker*` come from the operator and
override per link.

### Labels

A link's label is **text**, placed along the route by `along:` — the link's track
rule, exactly as `columns:` is a grid's. One label trails the head (`a -> b
"watches"`); two or more, or a styled one, ride the `[ ]`:

| Property | Notes |
|---|---|
| `along` | A list of `0..1` fractions along the whole drawn route, one per label (`along: 0.2 0.5 0.8`). Omitted → auto-distribute across the hops, so one label avoids junctions and several spread out. |

```
a -> b "watches"                                // the common case — one label, auto-placed
a -> b "watches" .loud { stroke: red }          // + a class and wire colour
a -> b { along: 0.3 0.7 } [ "near a" "near b" ] // two labels
a -> b [ "watches" { translate: 0 -6 } ]        // a styled / nudged label
```

Each label is an ordinary **styleable text leaf** ([SPEC 3](#3-statements--the-label)): give it its
own `{ }` in the `[ ]` to nudge or turn it. The head label takes no style — the `{ }`
after a link's head is the *link's* — so a styled label rides the `[ ]`, exactly as a
node's does. A label is an obstacle to nothing, and may slide along the link to keep
clear of nodes and other labels; the link never moves for it. Link labels default to
`font-size: 11`, `font-weight: normal`, and are tinted by the link's `color` — a link's
text props cascade to its labels; set them via `|-| { font-size: 14; color: --blue }`
to restyle every link's labels at once, on one link's `{ }` to restyle its labels, or
on a label's own `{ }` to restyle one.

### Endpoints & scope

```
endpoint = ident { "." ident } [ ":" side ]
side     = top | bottom | left | right
```

A path walks with `.` into children; a final `:side` forces a side. Every link
resolves in a **scope** — the scene root for top-level links, the container's body for
links written inside one. The first segment names a node in the scope, each further
segment a child of the previous. **There is no search.** A single bare id not in the
scope auto-creates a box there ([Implicit nodes](#implicit-nodes)); a **multi-segment
path** that does not resolve is an error, and the error suggests full paths of
same-named nodes —
`link endpoint 'kitchen.bowl' not found at scene root; did you mean 'kitchen.counter.bowl'?`

| Endpoint (root link) | Resolves to |
|---|---|
| `cat` | root node `cat` |
| `kitchen.counter.bowl` | exactly that path |
| `kitchen.counter.bowl:left` | the same node, left side forced |

Bodies are **sealed**: a body link connects nodes of its own subtree only.
Cross-container links are written at the lowest level where both ends are visible —
usually the root. Without a side the router picks edges by geometry; with a `:side`,
that edge is forced.

### Internal links in a body

A container's (or define's) `[ ]` may link its own children — children and links read in
**source order**, so a wire usually trails the boxes it joins but may also sit among them
(a `layout: sequence` ([SPEC 13](#13-sequence)) relies on this — its frames interleave with its
messages). In a define, ids are local and materialize per instance — the same sealed-body
rule. From outside, the dot-path navigates in:

```
{
  |room::group| {
    gap: 10;
  } [
    |box#inlet|  "Inlet"
    |box#outlet| "Outlet"
    inlet -> outlet "flows"
  ]
}

|room#garden|  "Garden"
|room#kitchen| "Kitchen"
garden.outlet -> kitchen.inlet "carries"
```

### Routing

Links route **orthogonally** by default — horizontal and vertical runs through the
free space between nodes, corners rounded. The router picks entry/exit sides unless an
explicit `:side` forces one. `clearance` (default 16) is the minimum gap every link
keeps from nodes and from other links.

`routing` selects the strategy for a scope and cascades like `clearance`: `orthogonal`
(the default) routes by the contract below; `straight` draws each link as one segment
between the bodies, trimmed to their boundaries — it avoids nothing and reports
nothing; `curved` is named but deferred ([SPEC 22](#22-deferred)). It pairs with `layout` —
`layout` places the nodes, `routing` routes the links between them — so a group can
route its internals one way while the root routes another. Which subsystem realises a
scope's links is the scope's **wiring strategy** ([SPEC 11](#11-the-layout-model)): the
orthogonal (or `straight`) router for `flow` / `grid`, layout-time lowering for
`sequence` and `chart` / `pie`.

The full routing contract — clearance, spacing, crossings, fan-out, self-loops — lives
in [`ROUTING.md`](ROUTING.md), the source of truth for routing.

---

## 10. Colour, Variables & Expressions

CSS variables theme the **visual** layer — colours and the font family. Everything
that affects layout — sizes, gaps, padding, and font *size* — is a baked constant, so
a standalone SVG never depends on host CSS. This section also holds the **expression
engine** ([SPEC 10.7](#107-expressions--functions)), the one place operators appear.

### 10.1 Visual variables (live, themeable)

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
--lini-stray         light-dark(crimson, #ff6b6b)    the stray-link fallback (ROUTING.md, Impossible layouts)
--lini-group-stroke  light-dark(rgba(0,0,0,.4), rgba(255,255,255,.4))
--lini-group-fill    light-dark(rgba(0,0,0,.03), rgba(255,255,255,.05))
--lini-header-fill   light-dark(rgba(0,0,0,.06), rgba(255,255,255,.08))  the table / entity header band
--lini-icon-fill     light-dark(rgba(0,0,0,.16), rgba(255,255,255,.18))  the soft body behind a duotone icon
--lini-caption-color light-dark(rgba(0,0,0,.5), rgba(255,255,255,.55))
--lini-footer-color  light-dark(rgba(0,0,0,.5), rgba(255,255,255,.55))
--lini-font-family   ui-monospace, "SF Mono", "Cascadia Code", "JetBrains Mono", Menlo, Consolas, "Liberation Mono", monospace
--lini-font-weight         normal
--lini-caption-font-weight normal
--lini-link-font-weight    normal
--lini-text-color    var(--lini-fg)
--lini-shadow-color  light-dark(rgba(0,0,0,.2), rgba(0,0,0,.5))
```

`--lini-bg` paints the scene background (the canvas rect), so the diagram is
self-contained in either mode. The default font is a **monospace** stack: it reads
crisp, and a fixed glyph advance keeps text-width estimation accurate without embedded
font metrics ([SPEC 22](#22-deferred)). Body text, captions, and link labels are all
`normal` weight by default.

**Dark/light is automatic.** The compiler emits `color-scheme: light dark` on `.lini`,
so `light-dark()` follows the viewer's OS (`prefers-color-scheme`) — no script, no
`@media`. A `data-theme="dark"` / `"light"` on the SVG or any ancestor forces a mode
(it flips `color-scheme`, and its higher specificity beats the OS). All defaults sit in
`@layer lini.defaults`, so unlayered host CSS still wins with no `!important`.
`--bake-vars` freezes the light arm into literals for renderers without `light-dark()`
([SPEC 10.6](#106---bake-vars)).

### 10.2 The colour palette

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
detail.

```
{ |card::box| { fill: --teal-wash; stroke: --teal-ink } }   // a pretty card, one line
|box#n| { fill: --amber-soft }
```

The tiers are generated from one **OKLCH** seed per hue, so the ramp is perceptually
even and the eleven read as a family. The same space is open to you — `fill: oklch(0.7,
0.14, 200)` picks any colour directly ([SPEC 2](#2-lexical-syntax)). Names are conventional
— every one is an ordinary colour word, so `--blue`, `--red`, `--green` are all there —
with aliases for muscle memory: `--yellow → --amber`, `--pink → --rose`, `--indigo →
--purple`, `--cyan → --teal`. `red` stays clear for **danger**; `rose` is the warm pink
you decorate with (its `wash` / `soft` tiers are your pinks), `green` is tuned to an
emerald, and `lime` is the lemony one.

The palette is **tree-shaken** ([SPEC 16](#16-svg-output)): only the `--lini-*` variables a
diagram references are emitted, so the full palette costs a three-box diagram nothing.

### 10.3 Gradients

`fill`, `stroke` (a shape's outline or a link's wire), and `gap-color` accept a **gradient** in place of a flat colour. Stops are
ordinary colours — palette `--name`s flip dark/light and bake, a raw `#hex` is a fixed
literal.

| Form | Result |
|---|---|
| `gradient(--rose, --sky)` | two stops, auto-angled 135° — any two hues blend cleanly |
| `gradient(--rose, --amber, --sky)` | three or more evenly-spaced stops |
| `linear-gradient(135, --rose, --sky)` | an explicit angle in degrees — the control gate |
| `radial-gradient(--rose, --sky)` | a radial blend from the centre out |

```
|box#hero| { fill: gradient(--blue, --purple) }       // a single-family sheen
|badge#tag| { fill: gradient(--rose, --amber, --sky) } // a three-colour pop
```

The angle is the only "more syntax": `gradient(…)` is angle-less and always lands on a
flattering 135°. OKLCH stops keep the midpoint clean
rather than muddy.

Each distinct gradient is emitted once as a `<linearGradient>` / `<radialGradient>` in
`<defs>` and referenced by `url(#…)` — deduplicated and shared like the drop-shadow
`<filter>`s ([SPEC 16](#16-svg-output)). `objectBoundingBox` units fit one definition to
any node at any size. The stops being palette vars, a gradient themes, flips, and bakes
like any other paint; gradient-on-text is deferred ([SPEC 22](#22-deferred)).

### 10.4 `--name` references

`--name` is the **visual-variable namespace, and only that.** `--name: value;`
declares one (a built-in `--lini-*` name keeps its meaning; a new name is yours), and
`--name` in a value references it, emitting live `var(--lini-name)`:

```
{
  --brand: #ff6600;
}
|box#cat| { fill: --brand }
```

Alias a host var from CSS: `.lini { --lini-accent: var(--my-brand-blue); }`.

Layout values — sizes, gaps, padding, `font-size`, `clearance` — are **not** `--name`
variables: they bake (a runtime `var()` can't be measured at compile time). Set them
with a literal, a rule (`gap: 30;`, `|box| { radius: 4 }`), or a backtick expression /
function ([SPEC 10.7](#107-expressions--functions)).

### 10.5 Layout constants (baked)

Baked compile-time defaults — override per-node, on the root, in rules, or in an
instance / link block:

```
font-size 15     link-font-size 11   caption-font-size 12
stroke-width 2   radius 8            gap 20                 padding 20
clearance 16     icon-size 32        link-width 2           icon stroke-width 2
```

`font-size` is body text; link labels (11) and captions (12) carry their own baked
defaults — a global `font-size:` sets body text and cascades, `|-|` or a link sets link
labels, `|caption| { font-size: … }` sets captions. `radius` rounds a `|box|` by default;
`|block|` / `|rect|` are `0`.

Padding defaults to 20 — including the root, whose padding frames the whole scene (the
SVG margin) — with `|block|` / `|row|` / `|column|` at 0 and a `|table|` at `4 8` (its
cell inset). It doubles as the minimum size of an empty box (`2 × padding`; see
[Auto-sizing](#5-the-box-model)). **Every baked default — these constants and
the template bundles — lives in one place**, so the whole look is tuned from a single
file.

### 10.6 `--bake-vars`

Class rules and inline `style=` work everywhere, but CSS *variables* don't — resvg and
librsvg fail `var()` in every position (browsers, even `<img>`-embedded, are fine).
`--bake-vars` keeps the rules but inlines every `var(--lini-name)` as its literal: no
runtime theming, but a self-contained SVG that renders anywhere.

### 10.7 Expressions & functions

A **backtick expression** `` `…` `` holds compile-time math — folded to a literal (a
number, or a point `(x, y)` for geometry) when the diagram compiles. It is the **only
place operators appear**: outside a backtick `-` is a link, `<` / `>` are markers,
`/` a comment, so the fence is what lets `*` mean "times". A value stays backtick-free
until an operator does:

```
{ scale(n) `100 * 1.2^n`; }   // a function (below)

|box| {
  gap: 8;             // a literal
  width: scale(3);    // a call — no operator, no backtick
  padding: `8 * 2`;   // an operator → backtick (= 16)
}
```

Inside a backtick the language is small and total:

- **Operators** `+ - * / ^` (`^` power, right-associative), unary `-`, grouping `( )`,
  comparisons `< <= > >= == !=`, the ternary `cond ? a : b`.
- **Functions** — the math library `exp ln log sqrt abs sin cos tan min max clamp floor
  round pow`, and any you define (below); each returns a number or a point, called
  `name(args)`. (Colour / track builders like `rgb` / `repeat` make typed values, so
  they live in value position, never inside math.)
- **Constants** `pi`, `e`; **scientific notation** `1e6`, `1.32e-6`; the sample
  parameter `u` (geometry, below).
- **Locals** — `name = expr;` binds for the rest of the expression; the **final
  expression is the value** (no keyword, no `return`). `=` binds, `==` compares. Values
  are numbers and points — no strings, no loops.

```
`r = 40; n = 6; 2 * pi * r / n`   // r, n are locals; the last line is the value
```

**Functions** are defined in the stylesheet — a name, a parameter list, and a backtick
body, **juxtaposed** with no colon (which keeps a definition apart from a property:
`scale: …` is a property, `scale(n) …` a function). A zero-parameter function is a
named constant:

```
{
  scale(n)   `100 * 1.2^n`;
  unit()     `8`;                          // a named constant
  wave(a, f) `(u*300, a*sin(2*pi*f*u))`;   // a parametric point
}
```

Call a function anywhere a value goes — bare like `rgb(…)` / `repeat(…)`, or inside a
backtick; only an operator forces the fence, never the call, and a computed argument is
itself a backtick:

```
|box| { width: scale(3); padding: `scale(2) + 4`; columns: repeat(3, `80 * 2`) }
```

**Geometry.** `points:` (on `|line|` / `|poly|`) may be a **parametric expression in
`u`** — `u` sweeps `0 → 1`, sampled at `samples:` points into a vertex list, drawing
curves, waves, and spirals procedurally:

```
|line| { points: `(u*300, 20*sin(2*pi*3*u))`; samples: 60 }   // a sine wave
|line| { points: wave(20, 3); samples: 60 }                   // the same, named
```

Everything an expression touches **bakes** — a computed size, a sampled curve — so a
standalone SVG never depends on host CSS. The same sample-an-ambient seam feeds a
chart's `fn:` (with `x` bound to the domain — [SPEC 14](#14-charts)). Unknown names, wrong
arity, and out-of-range results are compile-time errors ([SPEC 19](#19-errors)).

---
# Part II — Layout

A container picks an **engine** with `layout:`. Every engine reads the same core
([Part I](#part-i--core)) — the cascade, paint, text, the box model, links, colour — and
adds only its own placement algorithm, its own child roles, and a few scoped
properties. This part is the family; each section states just its delta.

---

## 11. The Layout Model

| `layout:` | Engine | Arranges its children as | Wiring strategy | Lowers its subtree? |
|---|---|---|---|---|
| `flow` *(default)* | 1D flex | boxes / text in a row or column | orthogonal router | no — arranges in place |
| `grid` | 2D grid | boxes / text in tracks | orthogonal router | no — arranges in place |
| `sequence` | time axis | participants + messages + frames + notes ([SPEC 13](#13-sequence)) | layout-time time-rows | yes |
| `chart` | data plane | series + axes + bands + marks ([SPEC 14](#14-charts)) | layout-time data→pixels | yes |
| `pie` | part-to-whole | slices ([SPEC 14](#14-charts)) | layout-time value→angle | yes |
| *`drawing`* | datum / geometry | geometry + annotations *(future — [DRAWING.md](DRAWING.md))* | layout-time dimensions | yes |

**Defaults.** Every container — the root included — defaults to `layout: flow` with
`direction: column` and `gap: 20`. The default `|box|` pads its content by 20; so does
the root, and its padding is the margin that frames the whole rendered scene — links and
labels included — out to the SVG edge. The frameless `|block|` / `|row|` / `|column|`
pad by 0 ([SPEC 8](#8-templates)).

### Three seams every engine plugs into

The engines differ, but three contracts are shared — which is why a new layout is a
small, bounded addition ([Part III](#part-iii--reference) formalises each):

1. **The smart label extends.** The one label rule ([SPEC 3](#3-statements--the-label)) — each
   type places its `"X"` — is inherited by every layout (title, legend, axis title, header,
   guard; [SPEC 13](#13-sequence), [SPEC 14](#14-charts)). No layout invents a label syntax.

2. **The wiring strategy realises a scope's links.** `flow` / `grid` hand their links to
   the router ([SPEC 9](#9-links), [ROUTING.md](ROUTING.md)); `sequence` lowers each message to a
   time-row arrow; `chart` / `pie` have no links. One scope, one strategy — set by the
   scope's `layout` (with `routing:` selecting `orthogonal` vs `straight` for the routed
   ones). A `sequence` message is thus the one place a link's *order* is its geometry, not a
   routing problem.

3. **A layout-owning engine lowers to primitives in the layout phase.** `flow` / `grid`
   arrange their children where they sit. `sequence` / `chart` / `pie` instead **read their
   whole subtree** and emit an ordinary primitive tree — `|block|`s, `|line|`s, `|path|`s,
   text — at baked coordinates ([SPEC 17](#17-compile-pipeline)). So the cascade, palette, theming,
   gradients, `--bake-vars`, `fmt`, and determinism all apply to a chart or a sequence with
   **no engine-specific render code** — a chart *is* a diagram once lowered.

**The container is still a box.** An engine owns *where its children go*, but the
container node itself is an ordinary box: its own `fill`, `stroke`, `stroke-width`,
`radius`, `opacity`, `shadow`, `rotate`, and `href` paint in **every** layout — a chart,
a sequence, or a pie can carry a background, a frame, or a link like any `|box|`.

### Universal container properties

| Property | Role | Where it acts |
|---|---|---|
| `layout` | picks the engine (above) | any container |
| `direction` | orient a flow — `row` / `column` (default `column`); a chart adds `radial` | `flow`, `chart` |
| `gap` | space between children — `N` both axes, `row col` per axis, `≥ 0` | all (semantics per engine — below) |
| `gap-color` | paint the interior gutters (below) | `flow`, `grid` |
| `padding` | inner padding; frames and places the content ([SPEC 5](#5-the-box-model)) | `flow`, `grid` |
| `align` / `justify` | cross / main-axis packing ([SPEC 12](#12-flow--grid)) | `flow`, `grid` |
| `columns` / `rows` / `cell` / `span` | grid tracks & placement ([SPEC 12](#12-flow--grid)) | `grid` |
| `fill` … `href` | the container box's own paint (above) | any container |

`gap` is honoured everywhere but **means what the engine needs**: inter-child spacing in
flow / grid, the plot-to-title/legend gutter in a chart / pie (default 10), and the
message pitch / participant spacing in a sequence (default 32). `direction`, `align`,
`justify`, `gap-color`, and `padding` are the **flow / grid arranger's** knobs — a
`sequence`, `chart`, or `pie` container places its own children and ignores them.

**Nested boxes are unaffected.** These knobs govern a container *engine*'s placement of
its own children; an ordinary box **nested inside any layout** still lays out its own
content by the box model. So a participant box in a `sequence` — an ordinary box —
honours `padding`, `align`, `justify`, and `gap-color` on its **own** content, even
though the sequence engine placed the participant on the time axis. (A `chart` / `pie`
consumes its children into marks, so this case does not arise there — [SPEC 14](#14-charts).)
The full property-by-layout picture is the [support matrix](#15-property-ledger--support).

### Gap paint — `gap-color`

`gap-color: <color> | none` (default `none`) fills the interior **gutters** between a
flow's or grid's children — the gap regions — with a colour. The gutter's thickness is
the `gap`, so `gap: 1; gap-color: --stroke` paints hairline rules while a larger gap
paints a bold band:

| `gap-color:` | Effect |
|---|---|
| `none` (default) | no gutters painted |
| a colour | every **interior** gutter filled with it, thickness = `gap` |

Per-axis `gap` selects which rules appear: `gap: 1 0` (row gap only) paints the row
rules (horizontal), `gap: 0 1` the column rules (vertical), `gap: 1` both; a `0` gap
on an axis paints nothing there. Gutters are **interior only** — the outer frame is
the container's own border (its `stroke`), so a frameless grid (`stroke: none`) shows
only inner rules and a bordered one is never doubled. In a grid the gutters are
span-aware (a gutter never crosses a spanning cell's interior, and a shared edge is
never doubled) and skip pinned children. This is what lets `|table|` be plain
`grid + gap: 1 + gap-color: --stroke` rather than a magic type ([SPEC 8](#8-templates)).

---

## 12. Flow & Grid

The two **router-routed** layouts: they arrange boxes and text in place, then hand
their links to the router ([SPEC 9](#9-links)). `flow` is 1D flex, `grid` is 2D.

### Flex — `align` / `justify`

`layout: flow` runs its children along one axis, set by `direction` (`row` horizontal,
`column` vertical — the default). `justify` runs *along* the flow (main axis), `align`
runs *across* it (cross axis). Both default `center`.

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
list length. There is no `fr` unit.

**Auto-flow.** Children without `cell:` flow left-to-right, wrapping at the column
count; a `cell:` pins one explicitly and the rest flow around it. Bare-text cells are
pure auto-flow — `cell:` / `span:` apply to **box** children only (a text
node has no block to carry them). A grid is positional, so an empty `""` cell is
**kept** — it holds its track and keeps the cells after it aligned (in flow, an
empty `""` is dropped). `cell:` / `span:` are read only on a grid; off a grid they are
silently ignored (`span:` is also a chart band's extent — [SPEC 14](#14-charts)).

**Per-column alignment.** On a grid, `align` (horizontal ↔) and `justify`
(vertical ↕) accept a **list parallel to `columns`** (one value per track) or a
scalar for all — so `align: start center end` aligns three columns in one
declaration. Mind the axes: a grid follows **column-flow, not CSS grid**, so `align`
is horizontal — the same knob that left-aligns text in a `direction: column` box.
`stretch` fills the track; `start`/`center`/`end` pack the cell's box at natural
size; the default centres.

A cell that **fills** its track (`stretch`) then honours its **own** `align`/
`justify` to place its content: an auto cell has no slack and sits centred, but a
filled one slides its text to the aligned edge. This is what lets a `|table|` align a
whole column — every table cell fills, and the column's `align` is carried onto the
cells to place their text ([SPEC 8](#8-templates)); the core needs no notion of "table".

Both layouts paint interior gutters with `gap-color` ([SPEC 11](#11-the-layout-model)) and
frame the whole with the container's own `stroke`.

---
## 13. Sequence

A **sequence** reads a diagram on a **time axis**: `layout: sequence` places named
**participants** across the top, drops a **lifeline** from each, and lays **messages** —
ordinary links — top-to-bottom **in source order**, so the order you write the wires *is*
the order they happen. It adds **no grammar**: participants are nodes, messages are links
([SPEC 9](#9-links)), frames and notes are nodes — only the engine, six type names, and four
properties are new, and it lowers to primitives like any layout-owning engine
([SPEC 11](#11-the-layout-model), seam 3).

### The container & its children

`layout: sequence` on the root (`{ layout: sequence }`) or any node makes a sequence; the
`|sequence|` template is the preset over `|block|`. Its children **split by role,
recognised by type** — every **other** box is a participant (an open fallback, unlike a
chart's closed series set):

| Child | Is a | Drawn |
|---|---|---|
| a box (`\|box\|`, `\|cyl\|`, `\|icon\|`, …) | participant | a header at the top + a lifeline down |
| a link (`a -> b`) | message | a time-row arrow between two lifelines |
| `\|loop\|` / `\|opt\|` / `\|alt\|` | frame | a labelled rectangle around a span of messages |
| `\|else\|` | separator | a guarded compartment divider inside an `\|alt\|` |
| `\|note\|` | note | a callout over / beside lifelines |

**Nodes and links interleave in source order** — the body's "children before links"
ordering ([SPEC 20](#20-grammar)) relaxes to *source order preserved*, so a frame (a node) sits
among the messages (links) around it.

**One scope.** Every message resolves its endpoints against the **sequence's
participants**, whatever frame it sits in: a frame's `[ ]` groups messages for layout but
opens **no new scope** — it declares no participants and auto-creates none, overriding the
sealed-body and body-auto-create rules ([SPEC 3](#implicit-nodes), [SPEC 9](#endpoints--scope))
inside a sequence. So `|alt| [ db --> api … ]` wires the outer `db` / `api` lifelines,
never frame-local boxes.

`gap` sets spacing: its **column** part the space between participants, its **row** part
the gap between message rows (`gap: row col`; default `32 32`). A label wider than its span
widens it — adjacent lifelines sit `max(gap-column, widest message label between them +
margin)` apart, text measured at compile time. `width` / `height` size the whole frame and
distribute any surplus; unset, it sizes to its content.

### Participants & lifelines

A participant is an ordinary node; its **smart label** is its header, placed **by its
type** ([SPEC 3](#the-label)) — centred text for a box, the symbol for an `|icon|`.
Participants sit across the top **in declaration order** (left to right), each dropping a
**lifeline** and sharing a common foot at the last row. An **undeclared** endpoint
**auto-creates** a participant — `a -> b` with neither declared draws two — appended in
first-use order, so a quick sequence needs no header:

```
{ layout: sequence }
user   -> server "login"     // two auto-created participants, one message
server --> user  "token"
```

Declare a participant (with an `#id`, so messages can name it) to fix its order, type, or
paint: `|cyl#db| "Store"`, or `|icon#user| "user"` for an actor glyph. A participant
**lends its paint to its apparatus**: its lifeline and activation bars take its own
`fill` / `stroke` / `stroke-width`, so colouring or weighting a participant carries down
its whole timeline. Being an ordinary box, it also honours the box model on its own
content ([SPEC 11](#11-the-layout-model)).

### Messages

A message is a **link** ([SPEC 9](#9-links)) read on the time axis: its operator picks the
look, its label rides above the arrow, its order is its row.

| Write | Means |
|---|---|
| `a -> b "x"` | a **call** — solid arrow, `a` to `b` |
| `a --> b "x"` | a **return** — dashed arrow |
| `a ~> b "x"` | an **async** message — wavy arrow |
| `a -> a "x"` | a **self-message** — a hook on `a`'s own lifeline, label to the right |

Every operator, marker, class, and `{ }` is the link's own; only the *placement* differs,
so a message's label sits centred above its arrow and `along:` has no role. A chain
`a -> b -> c` is two messages on two rows; a fan `a -> b & c` likewise expands to two, in
expansion order. A forced side (`a:left`) and `routing` have no meaning on a time-row arrow
and are ignored.
Call vs. return is read from the **operator** (`->` vs `-->`), not a `stroke-style:`
override.

### Activations

A participant is **active** while it handles a call. By default (`activation: auto`) a
call (`->`) **opens** an activation bar on its target's lifeline; the next **return**
(`-->`) from that target **closes** its most recent open bar; nested calls **stack** (each
bar offset outward), and an unclosed bar runs to that participant's last row. The bar
stack is **sequence-global** — a call inside a frame may close outside it. A self-message
(`a -> a`) and an async (`~>`) open none, and a return with no open bar just draws its
arrow. `activation: none` on the sequence draws no bars. (Explicit per-message control is
deferred — [SPEC 22](#22-deferred).)

### Frames & notes

A **frame** is a node whose `[ ]` holds its messages, drawn as a dashed rectangle spanning
the lifelines those messages touch (plus a small inset) over the rows they occupy. A
top-left **tab** names the operator; the frame's **smart label** is its **guard**, drawn
as the first compartment's condition. Frames **nest** and draw **behind** the lifelines (a
`fill` tints the region without hiding the wires):

| Frame | Means |
|---|---|
| `\|loop\| "guard"` | the messages **repeat** (drawn once, not unrolled) |
| `\|opt\| "guard"` | the messages happen **only if** the guard holds (an *if*) |
| `\|alt\| "guard"` | one of several **alternatives** (an *if/else*) |

An `|alt|` holds two or more **compartments** split by `|else| "guard"` — a separator
valid only inside an `|alt|`, its label that branch's guard; the first compartment's guard
is the `|alt|`'s own label:

```
api -> db "query"
|alt| "found" [
  db --> api "row"
  |else| "missing"
  db --> api "404"
]
api --> user "done"
```

A `|note|` is a callout placed at its time row (source order), bound to lifelines by
**placement**: `{ over: api }` a box over one lifeline, `{ over: api db }` a box spanning
those (and any between), `{ left: api }` / `{ right: api }` a box beside one. Its smart
label is the text; a multi-line or styled note rides the `[ ]` like any box. `over` /
`left` / `right` are valid only in a sequence. `par` and other fragments are deferred
([SPEC 22](#22-deferred)).

### Defaults

The six sequence types are bundles over `|block|`, tuned to read with no styling; the
cascade overrides any of it, and they reuse the scene's role variables — no new ones:

| Type | Defaults over `\|block\|` |
|---|---|
| `\|sequence\|` | `layout: sequence; gap: 32 32` (a root `{ layout: sequence }` gets the same `gap`) |
| `\|note\|` | `fill: --fill; stroke: --stroke; padding: 6 10; font-size: 13` |
| `\|loop\| / \|opt\| / \|alt\|` | `fill: none; stroke: --group-stroke; stroke-style: dashed; stroke-width: 1; radius: 4; padding: 24; font-size: 12` |
| `\|else\|` | `fill: none; stroke: --group-stroke; stroke-style: dashed; stroke-width: 1; font-size: 12` |

The engine resolves in the layout phase — a message's x-ends are the lifelines' positions
(fixed once participants are placed) and its y is its row — placing participants, walking
messages/frames/notes in source order, and lowering headers → `|block|` + text, lifelines
and arrows → `|line|`, activations/frames/notes → `|block|` ([SPEC 17](#17-compile-pipeline)).
The orthogonal router never sees these links.

---

## 14. Charts

A chart is **a layout** — `layout: chart` and `layout: pie` — so the cascade, paint roles,
the `"string"` rule, the expression engine, lower-to-primitives, theming, and baking all
apply unchanged ([SPEC 11](#11-the-layout-model)). A chart's one new job over `row`/`grid` is
to read **all** children first, fix a **shared scale** (data domain → plot pixels), sample
any formulas, then lower each child to primitives at baked pixel coordinates — the chart
analogue of a grid sizing tracks from its children. Charts add **no grammar**: the new
surface is type names, properties, and the layout algorithms.

### 14.1 The chart plane

| Layout | Template | Encodes | Children |
|---|---|---|---|
| `layout: chart` | `\|chart\|` | an x/value plane (cartesian or radial) | series, `\|axis\|`, `\|band\|`, `\|mark\|`, `\|bubble\|` |
| `layout: pie` | `\|pie\|` | part-to-whole, value → angle | `\|slice\|` |

`width` / `height` set the whole chart (plot **plus** axis gutters and legend); the plot
area is the remainder after labels are measured. Unset, a chart defaults to **360 × 220**;
a `pie` or `radial` chart is **square** (default **280**) — a chart cannot size to its
content (the content depends on the scale, which depends on the size), so these are baked
constants ([SPEC 10.5](#105-layout-constants-baked)). `fill` is the chart background, `stroke`
its frame, and the cascade styles a chart like any box.

**Chart-level properties** (on the `|chart|` / `|pie|` node):

| Property | Layout | Value | Default |
|---|---|---|---|
| `direction` | chart | `column` · `row` · `radial` | `column` |
| `bars` | chart | `grouped` · `stacked` · `overlay` | `grouped` |
| `categories` | chart | quoted-string list — the x-axis (or spoke) labels | indices `1…N` |
| `samples` | chart | integer — `fn:` sample count | `24` |
| `hole` | pie | `0` ≤ n < `1` — inner-radius fraction (a donut) | `0` |
| `legend` | both | `top` · `right` · `bottom` · `none` | auto (shown when ≥ 2 entries) |
| `tooltip` | both | `none` · `hover` · `auto` · `always` ([SPEC 14.8](#148-tooltips)) | `auto` |
| `gap` | both | number — clear space between the plot and the title / legend outside it | `10` |

`categories` is the common-case shorthand for the **x (domain) axis's** tick labels; an
`|axis|` child sets them the general way, but the two name the same thing — setting both is
an error ([SPEC 19](#19-errors)).

### 14.2 Series

A series is a child node; its smart label is its **legend** entry (no label → no entry).
Each series lowers to primitives and is valid only inside its layout (a series elsewhere is
an error, like `cell:` off a grid):

| Series | Layout | Draws | Lowers to | Paint |
|---|---|---|---|---|
| `\|line\|` | chart | a polyline through the data (a **closed** loop when `radial`) | `\|line\|` / `\|path\|` | `stroke`, `stroke-width`, `stroke-style` |
| `\|area\|` | chart | a line filled to a baseline | `\|poly\|` / `\|path\|` + `\|line\|` | `fill`, `stroke`, `baseline` |
| `\|bars\|` | chart | one bar per datum (a wedge when `radial`) | one `\|rect\|` / `\|poly\|` each | `fill`, `stroke`, `radius` |
| `\|dots\|` | chart | one marker per datum | one `\|oval\|` / marker each | `fill`, `stroke`, `marker` |
| `\|bubble\|` | chart | one bubble at a point, sized by `value:` | one `\|oval\|` | `fill`, `stroke` |
| `\|slice\|` | pie | one wedge | one `\|path\|` | `fill`, `stroke` |

**Singular vs. plural is the cardinality.** `|line|` / `|area|` are **one** shape (singular);
`|bars|` / `|dots|` are a **set** of marks, one per datum (plural). A `|slice|` is one wedge
and a `|bubble|` one bubble (singular, per node).

Inside a chart, `|line|` reads `data:` / `fn:` (data space); the standalone `|line|`
primitive ([SPEC 7](#7-nodes)) reads `points:` (pixels) — the chart layout branches on which.
A chart line is *a line*, so the name is reused, not duplicated.

**A line carries markers at every datum**, reusing the core `marker:` family generalised
from line *ends* to every vertex: `|line| { marker: circle }` shows a marker at each point.
A chart marker is **centred**, so only the symmetric kinds apply — **`dot`**, **`circle`**
(a larger, hover-sized point), and **`diamond`**; the directional `arrow` / `crow` are an
error on a series ([SPEC 19](#19-errors)). Every marker carries the datum's `<title>` — a
marked point is a hover target ([SPEC 14.8](#148-tooltips)). `|dots|` is markers with no line,
**`circle`** by default; its diameter is `width` (`height` too for an ellipse), its shape
`marker:` — there is **no** `size:` property.

**`curve:`** sets a line's / area's interpolation: `linear` (default, straight segments),
`smooth` (a **monotone** cubic — curved, passes through every point, **never overshoots**;
parameter-free), or `step` (a staircase). **`bars:`** on the chart combines multiple
`|bars|` series: `grouped` (side-by-side, default), `stacked` (piled; the top is the sum),
or `overlay` (translucent, on top). `radius` rounds a bar's corners. (Stacked areas are
deferred; areas overlay.)

**A `|bubble|` is one mark per node** — `|bubble| "Name" { at: x y; value: N; fill: … }`
places a bubble at data point (x, y), sized by `value:`. The chart scales bubbles **by
area** (area ∝ value); the smart label sits centred in the bubble when it fits, else on
hover. Reach for `|bubble|` when each is a distinct labelled entity; for many uniform
points, `|dots|` is terser.

### 14.3 Data & formulas

A series' values come from `data:` (explicit) or `fn:` (computed) — never both. Both use
the core value grammar (space within a group, comma between groups), so charts add **no
value form**; a comma is the discriminator:

| Source | Syntax | Meaning |
|---|---|---|
| categorical | `data: 9 15 24 18 30` | **one group** → one value per category |
| points | `data: 0 225, 60 225, 118 221` | **comma groups** → `x y` pairs (numeric x; scatter) |
| formula | `fn:` `` `min(8/(x/100-1)^2, 2000)` `` | a backtick in `x`, sampled at `samples:` |

A comma-less `data:` is always a value list (a single point cannot be written comma-less).
A `|line|` / `|area|` needs ≥ 2 vertices; with categorical data the value count must match
the `categories:` count ([SPEC 19](#19-errors)).

**`tags:`** is the **per-datum** text — a quoted-string list parallel to `data:` (one tag
per value or `x y` point), distinct from the series' one legend label. A tag rides with its
datum: on the plot beside the point, or on hover when there's no room — the placement is
`tooltip:`'s job ([SPEC 14.8](#148-tooltips)). Tag count must equal data count; `tags:` needs
discrete `data:` (a sampled `fn:` has no authored points, so `tags:` with `fn:` is an
error). A per-node mark (`|bubble|`, `|slice|`, `|mark|`) takes no `tags:` — its one smart
label *is* its point label.

```
|line| "GLM-5.2" { data: 35 63, 42 72, 84 75; tags: "Non-Thinking" "High" "Max"; marker: circle }
```

**Formulas are the core expression engine** ([SPEC 10.7](#107-expressions--functions)):
operators, the math library, `name = expr;` locals, the ternary, and stylesheet functions.
Charts bind two ambient names — the same seam that injects `u` for parametric `points:`:
**`x`** the x-axis data value (a whole-domain `fn:` uses it) and **`u`** a band-local clock
`0 → 1` ([SPEC 14.5](#145-bands--annotations)). A `fn:` is therefore **not folded at resolve**
(its `x` is unbound there) but held and **sampled at chart layout**, once the x-domain is
fixed. Locals chain derivations in one backtick; a stylesheet function keeps twins DRY:

```
{ ramp(s) `min(100, 25 + 1.572*(x/s) + 0.0142*(x/s)^2)`; }
|area| "Steel"    { fn: ramp(1) }
|line| "Aluminum" { fn: `ramp(1/0.7)` }
```

**The formula ceiling.** `fn:` expresses a function of `x`, not a recurrence: a numeric
integration (a running sum) has no closed form and ships as precomputed `data:` points.

### 14.4 Axes, scales & domain

An axis is an `|axis|` child of a `layout: chart` (an `#id` is optional, used to **bind** —
a series or annotation reads an axis with `axis:`); its smart label is the **axis title**.
A chart with no `|axis|` gets an x (domain) axis and an auto-fit value axis, so simple
charts declare none — an axis is written only to *say* something.

| Property | Value | Notes |
|---|---|---|
| `side` | `bottom` · `left` · `right` · `top` | cartesian only; several on one side stack outward in **source order** |
| `range` | `a b` (each end a number or `auto`) | the data window — and crop, and reverse (below) |
| `scale` | `linear` · `log` | `log` emits decade ticks labelled 1-2-5; its domain must be above 0 |
| `step` / `ticks` | number / list | tick spacing, or explicit ticks; omitted → nice ticks |
| `unit` | `"%"` | a quoted suffix appended to tick labels (and tooltips) |
| `gridlines` | `none` · *colour* | this axis's gridlines: `none`, or a colour (a colour turns them on) |
| `stroke` / `color` / `font-size` | core | `stroke` tints the axis line + ticks, `color` the labels + title |

An **x (domain) axis** is categorical when `categories:` gives it labels (or by default,
indices `1…N`) and numeric when the data is points or a `fn:`. A **value axis** carries
series magnitudes; `axis: <id>` on a series binds it (default: the first value axis of the
series' orientation). Multiple value axes share a plot for dual-unit charts; only the
**primary** value axis and the x axis draw gridlines by default, so a normal grid appears
and a second value axis adds none (avoiding moiré). The default tint is `--lini-grid` — a
faint role variable charts add to the palette, themeable and dark/light-aware.

**`range: a b`** does three jobs at once: it sets the visible **window** (`a`…`b`),
**crops** data outside it to the plot area, and **reverses** the axis when `a > b`
(`range: 50 1` runs high→low — both scale and tick order flip). Either end may be `auto`
(`range: 0 auto`); the two ends must be distinct ([SPEC 19](#19-errors)). Ticks are "nice" by
default (1-2-5 × 10ⁿ); `step:` sets a spacing, `ticks:` an explicit list, `scale: log`
decade ticks (domain above 0). Tick labels come from `categories:` (an x axis) or the
formatted tick value + `unit:` (a value axis). Explicit per-axis tick text (a general
`labels:`) is deferred — use `categories:` for the x axis ([SPEC 22](#22-deferred)).

### 14.5 Bands & annotations

Both are children placed in **data** coordinates; the model gives them for free.
`axis:` names the axis they measure against and is required on a `|mark|`.

A **`|band|`** partitions an axis and drives three things from one declaration: a
background **shade**, a **tick** (its smart label), and the **segment boundaries** every
series shares. `span: a b` is its data range on its bound `axis:` (the grid `span:`, now
valid on a chart band too — [SPEC 12](#12-flow--grid)); `fill: none` makes it a divider + label
with no shading.

```
|band| "Inject" { span: 1.4 3.1; axis: time; fill: --rose }
```

**A series opts into segmentation** with a per-band `fn:` **list** — one backtick (or a
bare constant) per band, evaluated in local `u`; a **single** `fn:` samples the whole
domain in `x` and ignores bands. Consecutive segments connect end-to-start (the riser is
drawn), so a jump is explicit. A per-band list whose length ≠ the band count is an error
([SPEC 19](#19-errors)) — never a silent truncation.

A **`|mark|`** places a reference line, point, or label by *value* on a *named* axis, so it
survives a `direction` flip unchanged:

| Form | Draws |
|---|---|
| `\|mark\| "100 °C" { at: 100; axis: temp }` | a reference **line** at value 100, across the plot perpendicular to `temp` |
| `\|mark\| "60 °C — 19 min" { at: 19 60; axis: temp }` | a **point** (dot + label): `x = 19`, value `60` |
| `\|mark\| "safe" { at: 170 4; axis: temp; marker: none }` | a **label** only (no dot) |

`at: V` (one value) is a line, `at: X Y` (two) a point; `marker: none` suppresses a point's
dot, leaving the label — so there is no separate free-label node. Bands and marks render in
**column** direction today; in `row` / `radial` they are deferred ([SPEC 22](#22-deferred)).

### 14.6 Legend, title & colour

One smart-label rule, placed by where the label sits: on the `|chart|` / `|pie|` → the
**title** (a caption above the plot); on a series / `|slice|` → a **legend** entry with a
swatch **mirroring its paint** (fill and edge); on an `|axis|` → the **axis title**; on a
`|band|` → a **tick** tinted its `fill`; on a `|mark|` → the annotation's **label**. A
legend appears automatically at ≥ 2 entries; `legend:` positions or suppresses it. **`gap:`**
sets the plot-to-title/legend clearance (default 10; `gap: 0` ≈ touching). The chart sets its
**chrome** — title and legend — in **bold**, while its **data text** — axis ticks, tags,
annotation labels — stays **normal** weight, so the numbers read quietly beneath the captions.

**Colour.** Explicit `stroke:` / `fill:` wins. Otherwise series **walk the palette**
([SPEC 10.2](#102-the-colour-palette)) in declaration order, skipping `red` (reserved for
danger), repeating if exhausted — deterministic:

```
--rose  --orange  --amber  --lime  --green  --teal  --sky  --blue  --purple  --gray
```

Each series takes its hue at the tier the role wants — **the outlined look**: a `|bars|` /
`|area|` / `|slice|` fills with the **`soft`** tier and gains a **`deep`** edge (`stroke:
none` removes it — a flat fill); a line takes the `deep` stroke, dots the `ink`. An
explicit `fill:` keeps its colour and still gains a deep edge of it. In `layout: pie` the
walk is **per slice** — the one place colour walks per datum rather than per series.

### 14.7 Direction, radial & pie

`direction` orients the chart — the same property a `flow` uses to pick its axis, plus
`radial`: `column` (default, cartesian, bars grow up), `row` (cartesian, bars grow right),
`radial` (polar, bars grow outward). **The flip never breaks a chart, because nothing is
authored in screen coordinates** — `categories:`, series `data:`, and annotations bound to
a *named* axis with `at:` / `span:` are all logical; `direction` only changes how that
plane is projected. An explicit axis `side:` is a screen edge and is honoured as written.

**Radial** (`direction: radial`) projects the cartesian model into polar coordinates: the
x (domain) axis bends into a ring (categories → evenly-spaced **spokes**, from the top,
clockwise) and the value axis becomes the **radius**. A radar `|line|` connects a series'
value on every spoke and **closes** to the first; an `|area|` fills that polygon; `|bars|`
fill their angular slot. A radial chart has **one value (radius) axis** — writing `side:`
on it is an error ([SPEC 19](#19-errors)) — and one x axis (the spokes). Concentric circular
gridlines and a configurable start angle are deferred; the polygon web is the default.

**Pie** (`layout: pie`) encodes value as **angle** — each slice's angle is its value over
the total — a different scale from radial's value-as-radius, hence its own layout. No axes;
its children are `|slice|` nodes:

```
|pie| "Spend" { hole: 0.5 } [
  |slice| "Ads"    { value: 40 }
  |slice| "SEO"    { value: 30 }
  |slice| "Direct" { value: 30 }
]
```

A `|slice|`'s `value:` is its magnitude (`≥ 0`), its smart label its legend entry; slices
fill clockwise from the top, each angle = `value / Σ value × 360°`, and walk the palette
(so slices are distinctly coloured). A total of zero is an error. **`hole:`** (`0` ≤ n < `1`)
cuts an inner hole — `hole: 0` a pie, `hole: 0.5` a donut. On-slice value labels, a centred
total, and exploded slices are deferred ([SPEC 22](#22-deferred)).

### 14.8 Tooltips

A datum's label has two presentations, and one property — **`tooltip:`** — sets how much
shows where. Hover is the only interactivity, with no script:

| `tooltip:` | On the plot (inline) | On hover | For |
|---|---|---|---|
| `none` | — | — | a clean static plot, no labels |
| `hover` | — | card + `<title>` | labels on demand |
| `auto` *(default)* | where it fits, else falls to hover | card + `<title>` | the printable default |
| `always` | every label, forced | card + `<title>` | export — every label must read |

The two texts **complement**: the *inline* label is the datum's own text — a series'
`tags:` entry, or a per-node mark's smart label — while *hover* shows its **value**. So a
point can read `Max` on the plot and `GLM-5.2: 75%` on hover, never competing.

**The hover floor is always honest.** A labelled mark carries a native `<title>` — its
accessible name, readable in any renderer and surviving `--bake-vars`. Over it, a live CSS
`:hover` rule reveals a hidden `<g class="lini-chart-tip">` card built from primitives,
positioned beside the point; the card is **live-only** (a baked SVG keeps the `<title>` and
drops the `:hover`). Only `tooltip: none` strips the `<title>` too.

**Inline placement is one greedy pass**, not a solver: each label tries a few offsets and
takes the first that clears the labels already placed and stays in the plot (a seat must
also sit off the series lines). Under `auto` a label with nowhere to sit drops to its hover
card; under `always` it is placed regardless. Inline labels are small and muted (`color:`
overrides, default `--muted`) and carry `pointer-events: none`. `tooltip:` cascades: set on
the `|chart|` it defaults every series; a series overrides it. Hit targets stay sparse — a
sampled curve draws at `samples:` density but a marker sits only at data / turning points,
so node count stays bounded.

### 14.9 Lowering

`layout: chart` / `pie` resolve in the layout phase ([SPEC 17](#17-compile-pipeline)), since the
shared scale needs every child's data first: **collect** series and resolve `data:` /
sample `fn:`; fix each axis **domain** and scale (bars force zero); inset the **plot rect**
by measured label / legend gutters; **lower** every series, axis, band, annotation, and the
legend to primitives at baked pixels; **emit** in a **semantic draw order** — bands →
gridlines → areas → bars → lines → dots → annotations → axes → labels → inline labels →
tooltip — so a line sits above its bars without hand-ordering (the one place a chart
overrides source-order rendering; `layer:` still wins). The output is an ordinary primitive
subtree ([SPEC 17](#17-compile-pipeline)).

---
# Part III — Reference

Canonical, dense lookup. The narrative ([Parts I–II](#part-i--core)) teaches once; this
part is the authoritative tables — every property, the output, the pipeline, the grammar,
the errors — and never repeats the prose.

---

## 15. Property Ledger & Support

Every property is `name: value;` — dash-case, positional, space-separated values
([SPEC 3](#3-statements--the-label)). This section is the one place that answers **which
property works where.**

**A property applies everywhere by default; the exceptions are marked.** An exception is
always one of two kinds: **type-owned** — a property a primitive requires or reads
(`points` on `|line|`, `symbol` on `|icon|`, `skew` on `|slant|`) — or **layout-owned** — a
property an engine interprets (`cell` on a grid, `over` on a sequence, `data` on a chart).
An **unknown or misspelled** property is **silently ignored** — the engine reads properties
by name and never rejects an unrecognised one (an unknown-property warning is deferred —
[SPEC 22](#22-deferred)). A **wrong-context** property is usually ignored too (`cell:` off a grid
simply has no effect), but a handful of **hard gates** do error: the sequence-placement props
(`over` / `left` / `right` / `activation`) off a sequence, a box property on bare text, a grid
without `columns`, and a layout's own type names used outside it ([SPEC 19](#19-errors)).

**State marks** used below: **✓** built and honoured · **⌛** meaningful but not built, a
candidate ([SPEC 22](#22-deferred)) · **—** not applicable.

### The container × layout matrix

The high-signal grid: which **container / layout** property each engine honours. (Paint,
text, and box-model properties are universal to every node — the tables that follow.)

| Property | `flow` | `grid` | `sequence` | `chart` | `pie` |
|---|---|---|---|---|---|
| `direction` | ✓ `row`/`column` | — | — | ✓ `+radial` | — |
| `gap` | ✓ spacing | ✓ spacing | ✓ pitch / spacing | ✓ plot gutter | ✓ plot gutter |
| `gap-color` | ✓ | ✓ | ✓ᵇ | — | — |
| `padding` | ✓ | ✓ | ✓ᵇ | — | — |
| `align` / `justify` | ✓ | ✓ per-column | ✓ᵇ | — | — |
| `width` / `height` | ✓ (slack) | ✓ (slack) | — content-sized | ✓ box size | ✓ box size |
| `columns` / `rows` / `cell` / `span` | — | ✓ | — | — (`span`→band) | — |
| container paint (`fill` `stroke` `radius` `shadow` `opacity` `href`) | ✓ | ✓ | ✓ | ✓ | ✓ |

**✓ᵇ** — honoured on the participant / frame **boxes' own content** (they are ordinary
boxes), but *not* by the sequence engine's placement of them on the time axis
([SPEC 11](#11-the-layout-model)). A `chart` / `pie` consumes its children into marks, so that
case does not arise — hence `—`.

### Universal properties

Honoured on every drawn node, in every layout (a box; text takes the marked subset).

**Paint & stroke** ([SPEC 6](#6-paint-stroke--text), colour [SPEC 10](#10-colour-variables--expressions)):

| Property | Value | Default |
|---|---|---|
| `fill` | colour · `none` · gradient · `auto` | `--fill` (box) · `none` (block/line) · `--icon-fill` (icon) · `currentColor` (text) · `--bg` (root) |
| `color` | colour | inherits (`--text-color`) — text colour for the subtree |
| `opacity` | `0..1` | 1 |
| `stroke` | colour · `none` · gradient | `--stroke` (`--group-stroke` on group) |
| `stroke-width` | number | 2 (group / frame 1) |
| `stroke-style` | `solid`·`dashed`·`dotted`·`wavy` | `solid` — `wavy` on links today (closed prims ⌛) |
| `radius` | number | 0 (block/rect) · 8 (box/group) — rect + polyline join; non-rect ⌛ |
| `shadow` | `N` · `dx dy` · `dx dy blur` · `dx dy blur color` | off — tint `--shadow-color` |

**Text** — all **inherit** ([SPEC 6](#6-paint-stroke--text)); text-valid on a bare string:

| Property | Value | Default | Kind |
|---|---|---|---|
| `font-family` | ident · string · `--var` | `--font-family` | live |
| `font-size` | number | 15 (link 11, caption 12) | baked |
| `font-weight` | `normal` · `bold` | `normal` | live (numeric ⌛) |
| `font-style` | `normal` · `italic` · `oblique` | `normal` | live |
| `text-transform` | `uppercase` · `lowercase` · `capitalize` · `none` | `none` | live |
| `text-decoration` | `underline` · `overline` · `line-through` · `none` | `none` | live |
| `letter-spacing` | number | 0 | baked |
| `line-spacing` | number | 0 | baked |

**Box model & placement** ([SPEC 5](#5-the-box-model)):

| Property | Value | Default | Notes |
|---|---|---|---|
| `width` · `height` | number · `auto` | `auto` | border-box; a **floor**. `\|image\|` needs both. |
| `padding` | `N` · `v h` · `t r b l` | 0 (block) · 20 (box) | inner padding; places content. Longhands `padding-top`/… accepted. |
| `pin` | `none` · `center` · edge · corner | `none` | out-of-flow anchor; a **box** property (not text). |
| `translate` | `x y` | — | post-placement nudge; **any** node incl. text. |
| `rotate` | degrees | 0 | turn about bbox centre; **any** node incl. text. |
| `layer` | integer | 0 (flow) · 1 (pinned) | paint order; ties → source order. |

**Media & accessibility** — any node (`href` also a link):

| Property | Value | Notes |
|---|---|---|
| `href` | quoted URL | wraps the node / link in `<a href>` — clickable. |
| `title` | quoted string | emits a `<title>` child (tooltip + screen-reader name). |

### Type-owned properties

Read on the listed primitive; required where noted ([SPEC 7](#7-nodes)).

| Property | On | Value | Notes |
|---|---|---|---|
| `points` | `\|line\|` `\|poly\|` | `x y, …` · parametric `u` expr | vertex list; **required**. |
| `samples` | `\|line\|` `\|poly\|`, chart `fn:` | integer | sample count (geometry; chart default 24). |
| `path` | `\|path\|` | quoted SVG path | **required**; native top-left coords. |
| `src` | `\|image\|` | quoted URL | **required**. |
| `symbol` | `\|icon\|` | ident | Phosphor name; **required** (or via the label). |
| `fit` | `\|icon\|` `\|image\|` | `auto` · `contain` · `cover` · `stretch` | maps content into the box (size unchanged); `auto` default, `\|sign\|` `contain`. |
| `skew` | `\|slant\|` | degrees `(-89,89)` | 15. |
| `stack` | closed primitives | `N` · `dx dy` | offset duplicate behind. |
| `marker` · `marker-start` · `marker-end` | `\|line\|`, links | see [SPEC 7](#7-nodes) | endpoint / vertex glyphs; from the operator on a link. |

### Grid, chart, pie & sequence properties

Layout-owned — an error only where a hard gate exists ([SPEC 19](#19-errors)); otherwise inert
out of scope.

| Property | Owner | Value | Default | Ref |
|---|---|---|---|---|
| `layout` | any container | `flow`·`grid`·`sequence`·`chart`·`pie` | `flow` | [SPEC 11](#11-the-layout-model) |
| `direction` | flow, chart | `row`·`column`·`radial` | `column` | [SPEC 11](#11-the-layout-model) |
| `gap` · `gap-color` · `align` · `justify` · `padding` | flow, grid | — | see matrix | [SPEC 11](#11-the-layout-model), [SPEC 12](#12-flow--grid) |
| `columns` · `rows` | grid | track list | — (`columns` required) | [SPEC 12](#12-flow--grid) |
| `cell` · `span` | grid box child | `col row` / `cols rows` | `— / 1 1` | [SPEC 12](#12-flow--grid) |
| `data` · `fn` | chart series | list / pairs / backtick | — | [SPEC 14.3](#143-data--formulas) |
| `tags` | chart series | quoted-string list | — | [SPEC 14.3](#143-data--formulas) |
| `curve` | `\|line\|` `\|area\|` | `linear`·`smooth`·`step` | `linear` | [SPEC 14.2](#142-series) |
| `baseline` | `\|area\|` | number | axis zero | [SPEC 14.2](#142-series) |
| `axis` | series, `\|mark\|`, `\|band\|` | an `\|axis\|` id | — | [SPEC 14.4](#144-axes-scales--domain) |
| `bars` · `categories` · `samples` | `\|chart\|` | see [SPEC 14.1](#141-the-chart-plane) | `grouped` · indices · 24 | [SPEC 14](#14-charts) |
| `hole` | `\|pie\|` | `0` ≤ n < `1` | 0 | [SPEC 14.7](#147-direction-radial--pie) |
| `legend` · `tooltip` | `\|chart\|` `\|pie\|`, series (`tooltip`) | see [SPEC 14](#14-charts) | auto · auto | [SPEC 14](#14-charts) |
| `value` | `\|slice\|` `\|bubble\|` | number ≥ 0 | — | [SPEC 14](#14-charts) |
| `at` | `\|mark\|` `\|bubble\|` | `V` / `X Y` | — | [SPEC 14.5](#145-bands--annotations) |
| `side` · `range` · `scale` · `step` · `ticks` · `unit` · `gridlines` | `\|axis\|` | see [SPEC 14.4](#144-axes-scales--domain) | — | [SPEC 14.4](#144-axes-scales--domain) |
| `over` · `left` · `right` | sequence `\|note\|` | id(s) | — | [SPEC 13](#13-sequence) |
| `activation` | `\|sequence\|` | `auto` · `none` | `auto` | [SPEC 13](#13-sequence) |

### Link properties

A link is styled like a node ([SPEC 9](#9-links)) — its wire takes `stroke*`, its labels the
text props. Its own properties:

| Property | Value | Default | Notes |
|---|---|---|---|
| `clearance` | number | 16 | min gap from nodes and links. **Scene config** — cascades. |
| `routing` | `orthogonal` · `straight` | `orthogonal` | wiring strategy; scene config, cascades. `curved` ⌛. |
| `along` | fraction list | auto | label positions along the route. |
| `marker` · `marker-start` · `marker-end` | marker | from the operator | endpoint glyphs ([SPEC 7](#7-nodes)). |

---

## 16. SVG Output

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
  <defs><!-- filters, gradients, clipPaths --></defs>
  <rect class="lini-canvas" .../>   <!-- the scene background (--lini-bg) -->
  <g class="lini-scene"> <!-- scene tree --> </g>
  <g class="lini-links"> <!-- links --> </g>
</svg>
```

`viewBox` auto-sizes to content + the scene's `padding` (20 px by default) on every
side. The `lini-canvas` backing rect paints the scene background (`--lini-bg`) over the
viewBox; a root `fill:` overrides it (`none` = transparent).

**Paint compiles to CSS; geometry bakes.** Node and link paint defaults — and every
rule — are stated once as class rules; only the classes actually used are emitted — and
likewise only the `--lini-*` variables actually referenced, so the built-in palette
([SPEC 10.2](#102-the-colour-palette)) adds nothing unless a diagram uses it. A node whose
resolved paint differs from those rules carries the difference as an inline `style="…"`
(inline beats class, mirroring the [cascade](#4-selectors-cascade--specificity)). Geometry —
sizes, positions (`pin` and `translate` fold into the baked origin), radii, points, paths,
transforms — is always baked into attributes. Inherited text properties state on `.lini`
and cascade natively; a node's own text property emits on its `<g>` (or directly on the
`<text>`) and inherits to its subtree.

**Box:**

```svg
<g class="lini-node lini-{type} lini-{base} lini-style-{class}"
   data-id="ID" transform="translate(X,Y)">
  <title>…</title>            <!-- when `title:` is set -->
  <!-- geometry, then children -->
</g>
```

Auto-classes: `lini-node` (every box); `lini-{name}` (the type and every type it
inherits, down to `lini-block`); `lini-style-{name}` (per worn class). With rotation,
the transform becomes `translate(X,Y) rotate(N)`.

**Text** emits a bare `<text class="lini-text">…</text>` at its placed position — no
wrapping `<g>`. A table's cells are `|block|`s wrapping their text, so each renders as a
`<g class="lini-block …"><text>…</text></g>`; the header and any `|footer|` cells carry
a fill, a body cell is frameless ([SPEC 8](#8-templates)). Text's font and colour come by
inheritance from the enclosing `<g>`; a string's own style block emits as a `style="…"`
(and `translate` / `rotate` as a `transform`) on the `<text>` itself.

**Link:**

```svg
<g class="lini-link lini-style-{class}" data-from="A" data-to="B">
  <path d="…" fill="none" stroke="…"/>
  <polygon class="lini-marker lini-marker-arrow" …/>
  <text class="lini-text" …>label</text>   <!-- placed by along: -->
</g>
```

Host CSS may restyle any `lini-`-prefixed class; layout is computed at compile time, so
runtime restyling (a fatter `stroke-width`) restyles without re-layout. A chart's or
sequence's lowered primitives ([SPEC 17](#17-compile-pipeline)) emit exactly like the boxes,
text, and lines above — a chart's tooltip card is a `<g class="lini-chart-tip">`, a
reserved styling hook.

---

## 17. Compile Pipeline

A reference pipeline; implementations may differ if the observable output matches.

**Parse.** Lex to tokens, then a single recursive-descent pass to the AST. The
bracket-and-bars vocabulary (`|…|` identity, `{ }` style, `[ ]` content) resolves every
statement with one token of lookahead — no type-set prescan ([SPEC 20](#20-grammar)).

**Desugar.** Lower all surface sugar to primitives + classes — the engine's true input.
Each template/define instance becomes its base primitive wearing a `.lini-*` class
chain (derived→base→primitive, down to `block` for every rectangular type); a type's
defaults and any `|type| { }` element rule fold into a generated `.lini-<type> { … }`
class; a `|table| |box| { }` descendant rule rewrites to `.lini-table .lini-box { }`, and
`|-|` (the link type) to `.lini-link` — the class every link wears; define bodies inline
per instance; the scene defaults (`layout`, `padding`, `gap`, `font-size`, `clearance`,
`routing`) settle on the root; the per-type smart label (text / caption / symbol / link
label / chart title …), auto-`along:`, and link auto-create (an undeclared endpoint `x` →
`|box#x| "x"`) become explicit. The pass is idempotent; type-system errors (cycle,
depth > 16, a define shadowing a built-in) surface here.

**Resolve** (top-to-bottom):

1. *Variables, functions & rules:* merge visual-var defaults ← `--theme` ←
   `--name: value`; build the function table; compile the stylesheet's class / id /
   element / descendant rules. Backtick expressions and function calls fold to literal
   numbers / points ([SPEC 10.7](#107-expressions--functions)).
2. *Scene tree:* each box is a primitive wearing `.lini-*` (type) and user classes;
   layer properties per the [cascade](#4-selectors-cascade--specificity) — the worn
   `.lini-*` classes as the type tier, then descendant, class, id rules, and the instance
   block; lift internal links; build the path index.
3. *Links:* resolve endpoints by scoped path walk with suggestion errors; merge link
   properties through the node cascade — the baked base plus the scope's `clearance` /
   `routing`, the `|-|` element rule, descendant `|…| |-|` and class rules, then the
   link's own block; cartesian-expand fan groups into one resolved link per pair; the
   operator's line sets `stroke-style` unless overridden.

**Layout** (bottom-up): leaf bbox from `width`/`height` or defaults (text → its glyphs;
box → content + `padding`; + half-`stroke-width` per side); arrange flow children per
`layout` / `direction` honouring `align`/`justify`/`stretch`/`evenly` when there is slack; pin
out-of-flow children to their parent anchor (the parent never grows for them); compute
gutters; apply `padding`; apply each node's `translate`; `rotate` last. A **layout-owning**
container — `sequence` ([SPEC 13](#13-sequence)) and `chart` / `pie` ([SPEC 14](#14-charts)) — instead
reads its whole subtree here and lowers it to primitives: a sequence places participants
and walks its scope's messages / frames / notes in source order, emitting lifelines, arrows,
frames, and notes — **consuming those messages**, so the router never sees them; a chart
fixes its shared scale and lowers series / axes / bands / annotations at baked pixels.

**Route links.** Per [`ROUTING.md`](ROUTING.md) — orthogonal, clearance-respecting,
deterministic — over every link **except** those a `sequence` scope already drew as arrows.
Place markers (sized `max(5, stroke-width × 4)`, tip on the endpoint) and link labels at
their `along:` fractions (auto-distributed when unset).

**Render.** Depth-first emit SVG per [SPEC 16](#16-svg-output): a box is a `<g>`, a string is a
`<text>`. A lowered chart / sequence subtree renders as ordinary primitives — so theming,
palette, gradients, `--bake-vars`, `fmt`, and byte-for-byte determinism apply with no
layout-specific render code.

---

## 18. CLI

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
| `--bake-vars` | Inline `var()`s as literals (for non-browser renderers — [SPEC 10.6](#106---bake-vars)). |
| `--watch` | Recompile on every input change (requires `-o`). |
| `-h`, `-V` | Help / version. |

`lini -` reads stdin (filename `<stdin>` in errors). **`lini serve`** runs a local live
preview (default port 7700): a `.lini` file live-reloads that one file; a directory (or
no path → the current directory) opens the **playground** — pick, edit, and render any
`.lini` file beneath it in the browser. **`lini theme`** lists the built-in themes;
**`lini theme NAME`** prints one as a `--lini-*` CSS file — a ready starting point for
your own (`light-dark()` colours, the font commented out).

**`lini fmt`** reformats to canonical style — 2-space indent, `key: value;`
declarations grouped on one line, a style-only node collapsed onto its head line when it
fits (`|box#api| { fill: red }`), a lone label trailing the head (`|box#api| "API"`),
children one per line in `[ ]`, table cells padded into aligned columns, comments and
blank lines preserved. `--check` exits 1 if it would change anything; `--stdout` writes
instead of rewriting.

**`lini desugar`** prints the file fully **lowered to primitives** — the Desugar pass
([SPEC 17](#17-compile-pipeline)) that is the engine's true input — so the lowered form
re-renders byte-identically. A chart's or sequence's *type* desugars here (a `|chart|`
is a `|block|` wearing `.lini-chart`); its geometric primitive subtree is a layout-phase
artefact ([SPEC 17](#17-compile-pipeline)), like a routed link's geometry. A
teaching/debugging view; prints to stdout, never rewrites, comments not preserved.

Exit codes: 0 success · 1 parse/resolution error or `--check` reformat needed · 2 I/O ·
3 invalid CLI.

---
## 19. Errors

Format: `filename:line:col: error: <message>` (LSP-compatible), compile-time, with a span.

**Identity, cascade & statements**

| Condition | Message |
|---|---|
| Duplicate id | `duplicate id 'X' (previously at L:C)` |
| Unknown type / class | `unknown type 'X'` / `unknown class '.X'` |
| Inheritance cycle / depth | `cycle in 'X → … → X'` / `'X' exceeds max inheritance depth (16)` |
| Define shadows builtin | `'X' shadows a built-in type` |
| Empty bars | `'\| \|' needs a type or an '#id'` |
| Invalid id | `'#123' is not a valid id — an id starts with a letter or '_'` |
| Class inside the bars | `a class follows the bars — write '\|box\| .hot', not '\|box.hot\|'` |
| Symbol set twice | `an icon's symbol is its label or 'symbol:', not both` |
| Text carries children | `text content takes no '[ ]' — wrap it in '\|block\|' to give it children` |
| Box property on text | `'pin' needs a box — wrap the text in '\|block\|'` |
| Declaration outside a block | `a declaration belongs in a '{ }' block` |
| Bare node on the canvas | `a node leads with bars — write '\|box#X\|' (a bare name is a link endpoint)` |
| Bare type in the stylesheet | `a type only appears in bars — write '\|box\| { }' to style every box` |
| Missing declaration ';' | `a declaration ends with ';'` |
| Style block holds non-decl | `a '{ }' style block holds only declarations` |
| `[ ]` holds a declaration | `declarations go in '{ }', not '[ ]'` |
| Styled head label | `a head label takes no '{ }' — put the text in a '[ ]' to style it: [ "…" { … } ]` |
| Two head labels | `one inline label — put two or more in a '[ ]'` |
| Label after a class | `a label comes before classes — write '\|box\| "X" .hot'` |
| Stylesheet after canvas | `the stylesheet '{ }' must come first, before any instance` |
| Glued compound in a rule | `a selector unit can't glue a type and a class — space them (descendant) or style '.hot'` |
| Spaced class chain | `classes glue into a chain — write '.hot.loud', no space` |

**Links & routing**

| Condition | Message |
|---|---|
| Unknown endpoint (path) | `link endpoint 'X' not found at <scope>` + `; did you mean 'A', 'B'?` |
| Auto-create shadows a node | `endpoint 'X' auto-created at <scope> — a node 'X' also exists at 'A.B.X'` (warning) |
| Chain mixes operators | `link chain mixes operators 'X' and 'Y'` |
| Chain < 2 nodes | `link requires at least two endpoints` |
| Missing required property | `'\|line\|' requires 'points'` |
| `->` in the stylesheet | `'->' draws a link on the canvas — style every link with '\|-\| { stroke: … }' in a '{ }' block` |
| `\|-\|` / `\|link\|` as an instance | `a link is drawn by an operator — '\|-\|' only styles links (write 'a -> b')` / `links are drawn by operators, not the '\|link\|' type` |
| `\|node\|` as instance | `'node' is the umbrella concept — write '\|block\|' for the bare box` |
| Deferred routing | `routing: 'orthogonal' and 'straight' are built; 'curved' is deferred (SPEC 22)` |
| Unknown side | `':X' is not a side — use top, bottom, left, or right` |
| Link labels split | `keep a link's labels together — write 'a -> b [ "x" "y" ]'` (warning) |

**Values, colour & expressions**

| Condition | Message |
|---|---|
| Invalid / out-of-range color | `invalid color 'XYZ'` / `rgb(300,0,0): component out of range` |
| Invalid `oklch()` | `oklch expects (L, C, H) or (L, C, H, A) — L and A in 0..1, C ≥ 0, H in degrees` |
| Gradient with < 2 stops | `gradient() needs at least two colour stops` |
| `linear-gradient` without an angle | `linear-gradient needs an angle first, then ≥ 2 colour stops` |
| Single-quoted string | `single quotes are not strings — use "…"` |
| Unquoted text value | `'title' takes a quoted string — write title: "…"` |
| Invalid `pin` value | `'pin' expects none, center, an edge (top/bottom/left/right), or a corner (e.g. 'top right')` |
| Negative `gap` | `'gap' must be ≥ 0` |
| `skew` out of range | `skew: N must be in (-89, 89)` |
| Unknown name in an expression | `unknown name 'foo' in an expression` |
| Function arity | `'scale' takes 1 argument, got 2` |

**Layout — grid**

| Condition | Message |
|---|---|
| Missing `columns` | `'layout: grid' requires 'columns'` |
| Empty / bad track | `'columns' needs at least one track` / `a track is a size, 'auto', or repeat(…)` |
| Grid out of range | `cell: 5 _ exceeds columns=3` |

**Layout — sequence**

| Condition | Message |
|---|---|
| Sequence node outside a sequence | `'\|loop\|' belongs in a 'layout: sequence'` (same for `\|opt\|` / `\|alt\|` / `\|note\|`) |
| `\|else\|` outside an `\|alt\|` | `'\|else\|' separates an '\|alt\|' — write it inside one` |
| `\|note\|` with no placement | `a '\|note\|' needs 'over:', 'left:', or 'right:'` |
| Sequence property off a sequence | `'over' is valid only in a 'layout: sequence'` (same for `left` / `right` / `activation`) |

**Layout — chart & pie**

| Condition | Message |
|---|---|
| Series / axis / band / mark outside a chart | `'\|bars\|' is a chart series — it belongs in a 'layout: chart'` |
| `\|slice\|` outside a pie | `'\|slice\|' belongs in a 'layout: pie'` |
| Pie given an axis or series | `a pie's children are '\|slice\|' only` |
| Empty chart / pie | `a chart needs at least one series` / `a pie needs at least one '\|slice\|'` |
| Series with both / neither `data:` `fn:` | `a series takes 'data' or 'fn', not both` / `a series needs 'data' or 'fn'` |
| `arrow` / `crow` marker on a series | `'marker: arrow' has no centred form on a chart — use dot, circle, or diamond` |
| `fn:` list ≠ band count | `'fn' has N formulas but the chart has M bands` |
| Data ≠ categories count | `series data has N values but the chart has M categories` |
| `tags:` count ≠ data count / on `fn:` | `'tags' has N labels but the series has M data points` / `'tags' needs explicit 'data'` |
| `categories:` + an axis text | `set 'categories' or an axis's tick text, not both` |
| `\|mark\|` without `axis:` / bad `at:` | `a '\|mark\|' needs 'axis:'` / `'at' takes one value (a line) or two (a point)` |
| `\|bubble\|` missing `at:` / `value:` | `a '\|bubble\|' needs 'at:' (x y) and 'value:'` |
| Unknown `axis:` id | `axis 'X' not found` + `; did you mean 'Y'?` |
| `range:` bad / equal ends | `'range' takes two ends: 'a b', 'a auto', or 'auto b'` / `'range' needs distinct ends` |
| `scale: log` over a non-positive domain | `a 'scale: log' axis needs a domain above 0` |
| `side:` in `direction: radial` | `'side' has no meaning in a radial chart — it has one radius axis` |
| `hole:` out of range | `'hole' is a fraction 0..1` |
| Negative slice value / pie total zero | `a '\|slice\|' value must be ≥ 0` / `a pie's slice values sum to zero` |

An **unknown property name** is not currently an error — it is silently ignored; a
warning with a did-you-mean hint is deferred ([SPEC 22](#22-deferred)).

---

## 20. Grammar

```
file        = [ stylesheet ] { drawn }              # setup block, then drawn statements in source order
stylesheet  = "{" { setup_item } "}"                # the root's setup block; omit when empty
setup_item  = decl | vardecl | funcdef | rule | define | comment | newline
drawn       = node | text | link | comment | newline   # instances and links interleave; a sequence reads order as time (SPEC 13)

decl        = ident ":" values ";"                  # ';' optional before '}'
vardecl     = css_var ":" values ";"                # --name : value ;
funcdef     = ident "(" [ ident { "," ident } ] ")" expr ";"       # scale(n) `…` ;
rule        = selector style                        # |box| { } , |table| |box| { } , .hot { } , #hero { }
define      = "|" ident "::" ident "|" body         # name :: base, optional children

node        = ident_bars [ string ] [ classes ] [ style ] [ children ]
text        = string [ style ]                      # bare content; a styleable leaf, never a box
ident_bars  = "|" ( type [ "#" ident ] | "#" ident ) "|"   # |type| , |type#id| , |#id|
type        = ident
classes     = "." ident { "." ident }               # a worn class chain — .hot, .hot.loud

style       = "{" { decl } "}"                       # declarations only
children    = "[" { node | text | link } "]"         # nodes, text, links — in source order
body        = [ style ] [ children ]                 # define / container body

link        = endpoints link_op endpoints { link_op endpoints }
              [ string ] [ classes ] [ style ] [ label_block ]   # the node tail, on a link head
selector    = sel_unit { sel_unit }                 # whitespace-separated = descendant
sel_unit    = ident_bars | "|-|" | "." ident | "#" ident  # a type(+id), the link type, a class, or an id
endpoints   = endpoint { "&" endpoint }
endpoint    = ident { "." ident } [ ":" side ]
side        = "top" | "bottom" | "left" | "right"

label_block = "[" { text } "]"                       # canonical labels — styleable text leaves

values      = value_group { "," value_group }        # comma only between list items
value_group = value { value }                        # space-separated scalars
value       = number | percent | string | hex | ident | css_var | call | expr
call        = ident "(" [ value { "," value } ] ")"
css_var     = "--" ident { "-" ident }
expr        = "`" { char } "`"                       # a compile-time math expression (SPEC 10.7)

link_op     = [ marker ] line [ marker ]
line        = "-" | "--" | "---" | "~"
marker      = "<" | ">" | "*" | "<>"

ident       = ( letter | "_" ) { letter | digit | "_" | "-" }
number      = [ "+" | "-" ] ( digit+ [ "." digit+ ] | "." digit+ )
percent     = number "%"                             # colour components only
hex         = "#" hexdigit { hexdigit }              # 3, 4, 6, or 8 hex digits
hexdigit    = digit | "a"…"f" | "A"…"F"
string      = '"' { char | escape } '"'
escape      = "\" ( '"' | "\" | "n" | "t" )
comment     = "//" { not-newline } newline
```

**Single-pass LL(1).** The stylesheet-first rule plus the bracket-and-bars vocabulary make
one token of lookahead enough — the first token of every statement already tells its kind:
in the stylesheet, `|…|` → a rule or (with an inner `::`) a define, `.name` → a class rule,
`#name` → an id rule, `--name :` → a variable, `ident :` → a root declaration; after it, a
drawn statement is a `node` (`|…|`), `text` (`"…"`), or — when a bare `ident` is followed by
a link-op, `&`, or a `.` path — a `link`. A **declaration** ends with `;` (its value may
span lines); a **statement** ends at a newline or `;`.

**Adjacency tells a `.class` from a path; a `:` tells a side.** A space before the `.`
makes it a worn class (`a .hot`), no space an endpoint path (`a.b`); the first class is
spaced from the identity, the rest of the chain glues (`.hot.loud`); a `:` after an
endpoint forces a side (`a:left`), distinct from the declaration `:` by position.

**Every layout reuses this grammar.** Charts and sequences add **no** lexer or parser
grammar — they are nodes, declarations, and children, distinguished by type name and by the
scope's `layout` ([SPEC 13](#13-sequence), [SPEC 14](#14-charts)). A future `drawing` layout adds a
few operator tokens and one value form ([DRAWING.md](DRAWING.md)); it is the first layout to
extend the grammar at all.

---

## 21. Reserved Words

Because a type only ever appears in bars (`|box|`) and an id always wears a `#`, **type
names are free as ids and ids are free as type names** — `|block#oval|` is fine, and
`block -> oval` is two ordinary nodes. A small set of words stays reserved:

- **`node`, `link`,** and the structural class names **`text`, `marker`, `canvas`,
  `scene`, `cut`:** not instantiable types — `node` is the umbrella concept (write
  `|block|` for the bare box), links are drawn by operators and styled by `|-|` (`|link|`
  is an error), and a **define** may not take one of these (its generated `.lini-<name>`
  would collide with a built-in SVG class — `|-|` lowers to the reserved `.lini-link`).

The **`.lini-*` class prefix** is reserved: desugar generates the type classes
(`.lini-block`, `.lini-box`, `.lini-<define>`), so a user class may not begin `lini-`.
User classes are emitted `.lini-style-<name>`.

The side names **`top`, `bottom`, `left`, `right`** are **not** reserved — they are
keywords only after an endpoint's `:` (`a:left`), so a node may be named `|box#left|`.
Single quotes (`'`) are reserved and are not strings.

Value keywords are **contextual**, not reserved as ids — `flow`, `grid`, `sequence`,
`chart`, `pie`, `row`, `column`, `radial`, `start`, `center`, `end`, `stretch`, `evenly`,
`none`, `auto`, `orthogonal`, `straight` mean their keyword only after the property that
expects them. The layout type names (`chart`, `pie`, `axis`, `band`, `mark`, `bars`, `dots`,
`bubble`, `slice`, `area`, `line`, and the sequence `loop`, `opt`, `alt`, `else`, `note`)
are built-in types like `box` — protected from a define shadowing them, free as ids.
Function names `rgb`, `rgba`, `hsl`, `repeat` are reserved only before `(`.

Inside an expression ([SPEC 10.7](#107-expressions--functions)), `pi`, `e`, and the sample
parameter `u` are keywords, and the math-function names (`sin`, `exp`, `min`, …) are
reserved before `(` — all contextual to the expression, free as ids elsewhere.

---

## 22. Deferred

Named in the language, not built yet; the syntax is stable.

**Core**

- `routing: curved` — the curved link strategy ([SPEC 9](#9-links); `orthogonal` and `straight`
  are built).
- operator spellings for the ER cardinality markers ([SPEC 7](#7-nodes)) — `one`,
  `zero-or-one`, `one-or-many`, `zero-or-many` are set via `marker*:` today; `-<` / `>-<`
  are the only crow's-foot operators.
- `stroke-style: wavy` on **closed** primitives (`|line|` waves — it backs an async
  sequence message; a hex / oval / rect outline does not yet).
- **gradient fills on text** — gradients fill nodes today ([SPEC 10.3](#103-gradients)).
- `radius` on non-rect primitives (hex / diamond / slant / poly).
- numeric `font-weight` (`100…900`); a solid (`fill`-weight) icon variant (the built-in set
  is Phosphor duotone, behind a default-on `icons` cargo feature).
- embedded font metrics — the monospace default keeps the estimate close; a proportional
  `font-family` override is approximate until then.
- **an unknown-property warning + a "did you mean" property-name hint** — an unrecognised
  property is silently ignored today ([SPEC 15](#15-property-ledger--support), [SPEC 19](#19-errors)).
- `aria-label`.

**Tables & entities**

- arbitrary per-cell backgrounds in a `|table|` — only the header and any `|footer|` cells
  carry a fill today; a body cell that needs one is a `|block|` ([SPEC 8](#8-templates)).

**Sequences** ([SPEC 13](#13-sequence)) — fragments `par` (parallel, with an `|and|` separator),
`break`, `critical`, and `ref`; participant grouping; found / lost messages and
create / destroy lifelines; explicit activation spans; message auto-numbering;
dividers / delays (`==` / `...`); and an `|actor|` stick-figure primitive (an actor is
`|icon|` today).

**Charts** ([SPEC 14](#14-charts))

- **bands / marks in `row` and `radial` charts** — they render in `column` direction today.
- a general per-axis `labels:` (explicit tick text for any axis) — `categories:` sets the
  x-axis labels today.
- **gauge** (a partial arc for one value); **stacked areas** (`bars: stacked` extended to
  `|area|`); polar-area **circular gridlines** and a configurable radial **start angle /
  direction** (the polygon web and top-clockwise are the defaults).
- per-slice **explode**, **on-slice value / percent labels**, and a **centred total** in a
  donut hole; **per-segment styling** (a style list mirroring a segmented `fn:`).
- **time scale** (date domains with calendar-aware ticks); **multi-ring pie / sunburst**;
  **per-datum paint styling** (a parallel paint list over `data:` — today, overlay a
  `|mark|`).

**A future layout** — `layout: drawing` (dimensioned engineering drawings) is designed but
unbuilt; see [DRAWING.md](DRAWING.md). It slots into [Part II](#part-ii--layout) as one more
layout.

---

## 23. Examples

**A scene — grid, defines, groups, nested links:**

```
{
  layout: grid;  columns: repeat(3);  gap: 40;  padding: 20;
  fill: --bg;  clearance: 12;                   // clearance cascades to every link

  |box| { radius: 4; }                          // round a touch less than the default 8
  |-|  { stroke: #666; }                        // every link's wire
  --accent: #0a84ff;
  .loud { stroke: red; stroke-width: 2; }       // a link (or node) class — one vocabulary

  |treat::box|  { radius: 5; }
  |alert::oval| { stroke: red; width: 36; height: 36; }   // a circle
  |room::group| {
    gap: 8;
  } [
    |box#inlet|  "Inlet"
    |box#outlet| "Outlet"
    inlet -> outlet "flows"                      // an internal link, per-instance
  ]
}

|oval#cat| "Cat" { cell: 1 1 }
|group#kitchen| "Kitchen" { cell: 2 1; gap: 20 } [
  |treat#bowl| "Bowl of oats"
  |box#water| "Water"
]
|room#closet| "Closet" { cell: 1 2 }
|room#fridge| "Fridge" { cell: 2 2 }

cat:right -> kitchen.bowl:left "watches"
kitchen.water -> closet .loud
closet.outlet -> fridge.inlet "restocks"
```

**Table, entity, and shorthand:**

```
|table#basket| { columns: 80 140 80 } [
  "Fruit" "Quantity" "Notes"
  "Apple" "12"       "fresh"
  "Mango" "3"        "ripe"
]

|entity#users|  "Users"  [ "id" "int"  "name" "varchar" ]
|entity#orders| "Orders" [ "id" "int"  "user_id" "int" ]
users -< orders "places"     // one-to-many — crow's foot on Orders

cat -> dog -> bird           // 3 implicit boxes, 2 links
fox & owl -> mouse           // fan-in
frog ~> pond                 // wavy arrow
```

**A sequence — a login flow:**

```
{ layout: sequence }

|icon#user|   "user"            // an actor — any node is a participant
|box#browser| "Browser"
|box#api|     "API"
|cyl#db|      "Sessions"

user    ->  browser "click login"
browser ->  api     "POST /login"
api     ->  db      "lookup"
db      --> api     "record"

|alt| "password ok" [           // a frame: its [ ] holds the branch's messages
  api     --> browser "200 + cookie"
  browser --> user    "dashboard"
  |else| "wrong"
  api     --> browser "401"
]
|note| "rate-limited" { over: api db }
```

**Charts — bars, a formula with a band, and a pie:**

```
|chart| "Cycle time (s)" { categories: "15 cm³" "30 cm³" "50 cm³" } [
  |bars| "1.8 kW" { data: 9 15 24; fill: --sky }
  |bars| "2.3 kW" { data: 7 13 20; fill: --amber }
]

|chart| "Injection profile" [
  |axis#bar| "Pressure (bar)" { side: left; range: 0 1100 }
  |axis#x|   "Speed (mm/s)"   { side: bottom; range: 0 133 }
  |area| "Pressure" { axis: bar; fn: `x <= 93 ? 1000 : 1000 - 319*((x-93)/40)`; fill: --teal }
  |band| { span: 93 133; axis: x; fill: --red }
  |mark| "1000 bar @ 93" { at: 93; axis: x; color: --muted }
]

|pie| "Spend" { hole: 0.5 } [
  |slice| "Ads"    { value: 40 }
  |slice| "SEO"    { value: 30 }
  |slice| "Direct" { value: 30 }
]
```

**A radar (radial chart) and labelled scatter:**

```
|chart| "Profiles" { direction: radial; categories: "Speed" "Range" "Armor" "Cost" "Stealth" } [
  |axis| { range: 0 5 }
  |line| "Scout"   { data: 5 4 2 3 5 }
  |area| "Cruiser" { data: 3 3 5 4 2; fill: --teal }
]

|chart| "Effort vs. score" [
  |axis| "tokens (k)" { side: bottom }
  |axis| "score %"    { side: left }
  |line| "GLM-5.2" { data: 35 63, 42 72, 84 75; tags: "Base" "High" "Max"; marker: circle; tooltip: always }
]
```

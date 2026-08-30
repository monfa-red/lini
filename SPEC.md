# Lini — Language Specification

Pretty diagrams, charts, and technical drawings from plain text, with fine-grained
control. One core — composable nodes, a CSS-driven cascade, compile-time layout —
drives a family of layouts (flow, grid, tree, sequence, charts, engineering
drawings, and floor plans), and compiles to clean, themeable SVG.

This document is complete: an implementer can build a conforming engine from it
alone. **Everything is defined once and reused** — a property, the cascade, colour,
the expression engine apply across every node and every layout. [Part I](#part-i--core)
is the shared core and is **authoritative**; **charts, sequences, and drawings are
layouts** ([Part II](#part-ii--layout)), peers of flow and grid over the same core,
each section stating only what is *new* to it; [Part III](#part-iii--reference) is
reference. **Link routing** has its own contract — [ROUTING.md](ROUTING.md).

---

## Table of Contents

### Part I

1 [Mental Model](#1-mental-model) · 2 [Lexical Syntax](#2-lexical-syntax) ·
3 [Statements & the Label](#3-statements--the-label) ·
4 [Selectors, Cascade & Specificity](#4-selectors-cascade--specificity) ·
5 [The Box Model](#5-the-box-model) · 6 [Paint, Stroke & Text](#6-paint-stroke--text) ·
7 [Nodes](#7-nodes) · 8 [Templates](#8-templates) · 9 [Links](#9-links) ·
10 [Colour, Variables & Expressions](#10-colour-variables--expressions)

### Part II

11 [The Layout Model](#11-the-layout-model) · 12 [Flow, Grid & Tree](#12-flow-grid--tree) ·
13 [Sequence](#13-sequence) · 14 [Charts](#14-charts) · 15 [Drawing](#15-drawing) ·
16 [Schematic](#16-schematic)

### Part III

17 [Property Ledger & Support](#17-property-ledger--support) · 18 [SVG Output](#18-svg-output) ·
19 [Compile Pipeline](#19-compile-pipeline) · 20 [CLI](#20-cli) · 21 [Errors](#21-errors) ·
22 [Grammar](#22-grammar) · 23 [Reserved Words](#23-reserved-words) ·
24 [Deferred](#24-deferred) · 25 [Examples](#25-examples)

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

**One character tells a statement's kind** — a leading `|` opens identity (a
node — or, when a link operator follows the closed capsule, a capsule-headed
link, [SPEC 9](#9-links)), a `"` text, a bare name a link, and inside the
stylesheet a `.`/`#`/`|…|` opens a rule. The capsule is self-delimiting, so
one token after it still decides; no prescan, no ambiguity.

**Two brackets, one capsule, three sigils — one meaning each** (the Quickstart
table, [SPEC 2](#2-lexical-syntax)): `|…|` is **identity**, the *only* place a
type lives; `{ … }` is **style**, the only place declarations live; `[ … ]` is
**content**, in source order. A drawn node is
`|type#id| "label" .class { style } [ children ]`, only the bars required; a link
is the same tail on a different head: `a -> b "label" .class { style } [ labels ]`.
A name goes bare **only when referenced**, and the one thing you reference is an
id; types and classes are never linked, so they are always sigil-marked.

**Boxes and text, like HTML.** A *box* has identity, classes, a style block, and
children. A *string* is text content — a leaf with no identity or children, though
it may wear classes and carry a style block (`"x" .quiet { color: red }`,
[SPEC 3](#text-content)). A
string in a box's `[ ]` (or trailing the head as its label) is that box's text; on
its own it is a free-standing text node. To give text children, a border, padding,
a `pin`, or a wirable id, wrap it in a box (`|block|` is the minimal one) — exactly
like wrapping a web page's text in an element.

**The file is the root container.** The stylesheet `{ }` is the root's own setup
block; the canvas instances are its children (written bare — the file *is* its
`[ ]`); the links are its internal links. Scene properties (`layout`, `gap`,
`padding`, `fill`, `font-size`, `clearance`, `routing`, …) sit in that block, alongside
rules like `|-| { stroke: … }` for link look; inheritable ones (`font-*`, `color`,
`clearance`, `routing`) cascade to every node and link.

**Render order is source order; the cascade is whole-file.** Instances draw in the
order written (later on top, pinned children above the flow; `layer:` overrides),
and every rule applies to every instance. Links need no declaration: naming an id
declared nowhere auto-creates it ([SPEC 3](#implicit-nodes)).

**Two kinds of variable.** *Visual* values that don't affect layout — colours and
the font family — stay live CSS variables (`--lini-fill`, `--lini-accent`, …), so a
host page can re-theme them; each colour carries a built-in dark variant following
the viewer's OS or a `data-theme` toggle ([SPEC 10](#10-colour-variables--expressions)).
*Layout* values — sizes, gaps, paddings, widths, **and font size** — bake into the
SVG as literals: text is measured at compile time, so its size can never be a
runtime `var()`, and a standalone SVG always looks right.

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
| `.name` (class) | At a rule head it is a class **selector** / definition (`.hot { … }`). On an instance, link, or text leaf it is a **worn class**, following the identity — **spaced** off it (`\|box\| .hot`, `a -> b .loud`, `"x" .quiet`), the rest of the chain **glued** (`.hot.loud`). |
| `id.child` | **No space** — an endpoint path into a child (`kitchen.bowl`). |
| `--name` | A variable, in a value or at a statement start to declare one. |
| link op | `[marker?] line [marker?]`, glued, no internal space (`->`, `--->`, `<->`). |
| `[ … ]` | A content list. Paired; whitespace inside is insignificant. |

**Strings** — double-quoted UTF-8: `"…"`. Escapes: `\"`, `\\`, `\n`, `\t`. A
double-quoted string is always text; leading and trailing whitespace in its value is
**trimmed** (`" ABC "` is "ABC", and a spaces-only `" "` becomes `""`), so source
spacing never leaks into the render.
Single quotes are **not** strings (reserved, [SPEC 23](#23-reserved-words)).

**A bare word is an identifier, never a string.** In a value, an unquoted word is
always an identifier — a keyword, a colour or `symbol` name, a `font-family`, or an id
reference — so literal **text** is always quoted: a string-valued property (`title`,
`hint`, `href`, `src`, `path`) takes a `"…"` even with no spaces. The one hybrid is a name that
may contain spaces — `font-family` — bare or quoted, quoted only when needed
(`font-family: "SF Mono"`), as in CSS. Numbers and `(…)` expressions are bare too;
only text is quoted.

**Expressions** — a parenthesized region `(…)` is a **compile-time math expression**,
folded to a literal number (or a point); parentheses are the **only place operators
appear**, a call's own parens count (`up(5 * r, 10)`), and groups may span lines
([SPEC 10.7](#107-expressions--functions)).

**Numbers** — integer or decimal, optional sign, no units (px for lengths, degrees
for angles, 0–1 for opacities/fractions). `10`, `-5`, `0.25`, `+3`. A trailing `%`
makes a **percentage** (`50%`), valid only in colour components.

**The comma law — CSS's rule, stated once.** A **comma** separates repeated
**list items**: `data: 2, 3, 4` · `columns: 80, 140, auto` · `points: 0 0, 10 10` ·
`categories: "Q1", "Q2"` · `along: 0.2, 0.5, 0.8` · `align: start, center, end`.
A **space** separates the components of **one** item, tuple, interval, or
shorthand: `padding: 5 2 5 5` · `shadow: 2 2 4 #0003` · `translate: 10 -4` ·
`range: 0 100` · `cell: 2 1` — and so `data: 10 20, 30 40` is two `x y` points,
and a lone `data: 10 20` is **one point, never two values** (a value list is
comma-separated: `data: 9, 15, 24`). A **pipeline** of calls that folds into one
value stays space-separated, like CSS `transform` — `draw: move(0,0) up(8)
fillet(2)`, and `mirror:`, whose items each reflect the union so far.
**Functions** use parentheses and sit in value position —
`rgb(…)`, `hsl(…)`, `repeat(…)`, the math library, and any you bind
([SPEC 10](#10-colour-variables--expressions)). A call's `(` **glues to its name**
(`rgb(…)`, never `rgb (…)`); a free-standing `(…)` is a math group, and a free-standing
`(-)`, `(o)`, or `(<)` a measuring op ([SPEC 15.6](#156-dimensions)) — which is how
`move(-2, 5)`, `(8 * 2)`, and `pin (o)` never meet.

**Colours** — `#fff`, `#f80c`, `#ffaa00`, `#ffaa00cc` (3/4/6/8 hex digits; the 4-
and 8-digit forms carry alpha), CSS names (`red`, `cornflowerblue`), `rgb(…)`,
`rgba(…)`, `hsl(…)`, `hsla(…)` (percentages allowed — `hsl(200, 50%, 50%)`),
`oklch(L, C, H[, A])` (the palette's own space — L/A in 0–1, C the chroma, H in
degrees; folded to a hex at compile time, so it renders in every target), a
`--name` variable reference, or `none`. Out-of-range channels are an error. Beyond
a flat colour, a **paint** (`fill` / `stroke` / `gap-fill`) may be a **gradient** —
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
| Binding | `my_r = 5;` `scale(n) = (…)` | a compile-time value / function, bound with `=` — read in any expression ([SPEC 10](#10-colour-variables--expressions)) |
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
  scale(n) = (100 * 1.2^n);
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

Only the bars are required — and at least a type or an `#id` must sit inside
them. [SPEC 1](#1-mental-model) names the parts; classes **follow** the bars
(`|box| .hot`, `|box| .hot.loud`), never sit inside them.

```
|cyl#db| "Postgres" .primary { fill: #eef } [
  |badge| "v16"
]
```

| Form | Effect |
|---|---|
| `\|box#cat\| ""` | same as `\|box#cat\|` — `""` is just an empty string. |
| `\|box\| "Load balancer"` | anonymous labelled box (can't be linked to). |
| `\|#cat\|` | a default `\|box\|`, id `cat`. |

### The label

A node has **no label unless you give it one** — a bare `|box#cat|` is an empty box
(the `#cat` is a handle, like HTML's `id=`, not text):

| Label | Means |
|---|---|
| no string at all | nothing — an empty box |
| `"X"` | the label "X" |
| `""` | an empty string — nothing in flow, an empty cell in a grid ([SPEC 12](#12-flow-grid--tree)) |

A link to an *undeclared* name still draws a labelled box ([Implicit nodes](#implicit-nodes)).
A multi-word label needs no `[ ]` (`|box#lb| "Load balancer"`).

**The label is smart — each type places it**, and every layout extends the same
rule (a chart's label is its title, a series' its legend entry — [SPEC 14](#14-charts)),
so no type needs a hand-written caption or symbol. Give no label and a type
places nothing:

| `"X"` on | becomes |
|---|---|
| `\|box\|` and the shapes (`\|oval\|`, `\|hex\|`, `\|cyl\|`, `\|diamond\|`, …) | its centred text |
| `\|group\|` / `\|table\|` | its **caption** ([SPEC 8](#8-templates)) |
| `\|icon\|` / `\|sign\|` | its **symbol** — `\|icon\| "heart"` is `\|icon\| { symbol: heart }` |
| a **link** | a label along the route ([SPEC 9](#9-links)) |
| a `\|chart\|` / series / `\|axis\|` / participant / frame | its title / legend / axis title / header / guard ([SPEC 13](#13-sequence), [SPEC 14](#14-charts)) |

**The label takes no style of its own.** The `{ }` and classes after the head are
the *node's*, so a styled, classed, or nudged label rides the `[ ]` content form
instead, where each string is a leaf in its own right ([Text content](#text-content)):

```
|box#api| "API" .hot { fill: red }        // label + class + the node's own style
|box#api| [ "API" { translate: 0 -6 } ]   // a styled label, via content
```

**The label and `[ ]` coexist — one inline label only** (two or more strings go in
the `[ ]`). The label is the node's one inline item, lowered by its type — a text
or caption child prepended to the `[ ]`, or (for `|icon|`/`|sign|`) the `symbol` —
and the `[ ]` holds the rest:

```
|group#kitchen| "Kitchen" [ |box#bowl| "Bowl" ]   // caption + a child
|icon| "bell" [ "3" ]                              // symbol + a text badge
```

### Text content

A string is a **text node** — always a `<text>` leaf, never wrapped:

- In a box's `[ ]` (or as the box's label) it is that box's text — centred when it
  is the only in-flow child, else a flow child laid out by the box's `layout`.
- On its own (on the canvas, or in a `[ ]`) it is a free-standing flow / canvas
  text node.
- Several strings are several text nodes — `"a" "b" "c"` is three (a string is
  self-delimiting, so no `;` is needed between them).
- An empty `""` is suppressed (adds no text) — except as a **grid cell**, where it
  holds its track ([SPEC 12](#12-flow-grid--tree)).
- Multi-line text uses `\n` (or wraps at `max-width` — [SPEC 5](#5-the-box-model));
  the box sizes to the widest line, with a `font-size × 1.2` leading between lines
  (plus any `line-spacing`), lines aligned by the container's packing knob
  ([SPEC 6](#6-paint-stroke--text)).

A string carries **no children** — text is a leaf, not a box — but where it is
**content** (free-standing, or a child in a `[ ]`) it takes the node tail: it
**may wear classes and carry a style block** of text properties —
`"Starter" .card-title`, `"X" { color: red; font-weight: bold; translate: 0 -6;
rotate: 12 }`. In its own block only text-valid properties apply (`color` /
`fill`, every `font-*`, `opacity`, `letter-spacing`, `line-spacing`,
`text-transform`, `text-decoration`, `text-shadow`, `translate`, `rotate`,
`layer`); any other — `pin`, `padding`, `width`, a border, children, even
`href` / `hint` — needs a real box, so wrap the text in a `|block|`. A **worn
class** is looser, per the class law ([SPEC 4](#4-selectors-cascade--specificity)):
its text-valid declarations land, the rest are inert on the text wearer. Set on
the string the style applies to it directly; set on a containing box it cascades
down ([SPEC 6](#6-paint-stroke--text)).

### Implicit nodes

A link endpoint that is a **single bare id** not present in the link's **scope**
auto-creates the node `|box#cat| "cat"` in that scope — a box named `cat`, labelled
"cat" — so `cat -> dog -> bird` is a complete three-box diagram. The same holds inside
a container body: a body link auto-creates its missing endpoints among that body's own
children. Declaring the id in the scope — before or after the link — uses it instead
of creating one. A **path** endpoint (`kitchen.bowl`) is never auto-created: it must
resolve to an existing node, or it is an error. If a same-named node exists elsewhere
in the tree, the box is still created here and a warning names the other match.

An auto-created id that is a **near-miss** of a name already known in its scope —
a small typo (edit distance ≤ 2, and shorter than the id itself), or equal
ignoring case, against the declared *and* the previously auto-created names —
**warns** toward the likely target: `cta -> bird` warns `did you mean 'cat'?`
even in an all-implicit file. Distinct names stay silent — short ids (`a -> b`)
and numbered siblings (`server -> server2`) are families, not typos — so
legitimate mixed use draws no noise ([SPEC 21](#21-errors)). Auto-create is
box-only; the **typed** declare-at-first-use is the capsule endpoint
(`cat -> |cyl#db|` — [SPEC 9](#9-links)).

### Declarations

A declaration `key: value;` lives only in a `{ }` style block — the stylesheet
(configuring the root) or a node's own block — and **ends with `;`**, so a value may
span lines ([SPEC 2](#2-lexical-syntax)); the `;` is optional only immediately before
`}`. A bare `key: value` outside a `{ }` is an error. Every property, its value shape,
and where it applies is in the [Property Ledger](#17-property-ledger--support).

---

## 4. Selectors, Cascade & Specificity

A **rule** is `selector { declarations }`. A selector is one or more
space-separated **units**; the space is the descendant combinator. A unit is a type
`|box|` (with an optional `#id`, `|table#main|`), the **link type `|-|`**, its
drawing subtype the **dimension type `(-)`** ([SPEC 15.6](#156-dimensions)), a class
`.hot`, or an id `#hero`:

```
|box| { … }              // every box (element selector)
|-| { … }                // every link — a line in the identity capsule ([SPEC 9](#9-links))
(-) { … }                // every dimension — the |-| subtype ([SPEC 15.6](#156-dimensions))
.hot { … }               // every node with class .hot
#hero { … }              // the one node with id hero
|table| |box| { … }      // every box inside a table (descendant)
#g |-| { … }             // every link written in #g
.sidebar |box| { … }     // every box inside a .sidebar
|table| .hot { … }       // every .hot inside a table
```

A **descendant selector** matches a node (or link) whose ancestor chain contains each
unit in order (not necessarily adjacent), exactly like CSS's descendant combinator.
Every construct keeps its sigil, so a selector reads as a run of marked units; a
bare word is never a selector. `|-|` and its dimension subtype `(-)` are
selector-only: a link is drawn by an operator, never instantiated ([SPEC 9](#9-links)).

A type's class never glues into its bars (`|box.hot|` is rejected): a class is
**worn**, not part of identity. To match boxes-with-a-class, style the class
(`.hot { … }`); to match within one, use a descendant (`.hot |box|`).

A **define**'s declarations ([SPEC 3](#3-statements--the-label)) are the new
type's defaults — tier 1 below; its optional `[ ]` children materialize per
instance ([SPEC 9](#9-links)).

**Selecting vs. drawing is decided by the section, not the syntax.** `|box| .hot`
in the stylesheet is a descendant *rule* (.hot inside a box); on the canvas it is
an *instance* (a box wearing .hot).

### The cascade

Properties on a node merge by a fixed five-tier ladder — **the more specific
source wins**, ties broken by **later wins** (source order). It is CSS-shaped
but not CSS specificity: a descendant rule always loses to a class rule,
whatever units it names. The tiers, low to high:

1. **Type cascade** — walked from the base primitive up to the node's declared type,
   layering each type's element-rule (`|box| { }`) and define defaults. A more-derived
   type overrides what it builds on. (This is where a template's and a define's baked
   defaults live — [SPEC 8](#8-templates).)
2. **Descendant rules** — `|table| |box| { }`, `.sidebar |box| { }`, matched against
   the ancestor chain.
3. **Class rules** — `.hot { }`, worn via `|box| .hot` on the node (a text
   leaf wears them the same way — `"x" .hot`, [SPEC 3](#text-content)).
4. **Id rule** — `#hero { }`, the node's own id.
5. **The instance's own block** — `|box#client| { fill: white }` — the most specific,
   beats everything above.

A link walks the **same ladder** — its type is `|-|`, its ancestors are its scope's
container chain, it has no id: below tier 1 sit the baked link base plus the scope's
`clearance` / `routing`, then the `|-|` element rule (type), descendant `|…| |-|` and worn-class
rules, then the link's own block ([SPEC 9](#9-links)). One exception: a link into a
node's **own descendant** (`x → x.path` — containment, or a tree's branch fan) cascades
**as if written in `x`** — its ancestor chain is `x`'s own, so `#x |-| { }` reaches it. A **dimension** is a link
subtype — type chain `|-|` → `(-)` — so a `(-) { }` rule beats `|-| { }` for
dimensions (the more-specific type, tier 1); `(-)` matches the measuring ops
only — a leader is styled through `|-|` (a leader-specific selector is
deferred, [SPEC 24](#24-deferred)) ([SPEC 15.6](#156-dimensions)).

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

1. **Centre origin.** Every bbox is centred at the parent's origin by default.
2. **Source order = render order;** later draws on top, with pinned children above
   the in-flow ones. `layer: N` overrides; ties break by source order.
3. **Strokes count** toward the bbox — `width: 100 height: 50 stroke-width: 4` →
   104×54. Only *painted* strokes: `stroke: none` paints nothing and counts
   nothing, whatever `stroke-width` says — so a bare `|block|` (which keeps
   `stroke-width: 2` invisibly, [SPEC 7](#7-nodes)) truly sizes to its content.
4. **`|path|`** takes native top-left coordinates rather than a centred bbox. (A
   node's *origin* may also sit off its bbox centre — a `|sketch|`'s pen origin,
   a `pattern:`'s seed, a `|drawing|`'s datum — [SPEC 12](#12-flow-grid--tree),
   [SPEC 15.1](#151-the-container-the-datum--the-scale).)
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

The anchor is the parent's **drawn box** — border and padding included.

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
centre, `pin: center` + `translate: x y` lands a child's centre at parent-local
(x, y) — explicit coordinates with no node-size arithmetic.

**`rotate: N`** turns a node N degrees about its bbox centre, applied last as an SVG
transform. Like `translate`, it works on **any** node, text included. `pin` (which
needs a parent anchor and takes a child out of the flow) is a **box** job; to pin
text, wrap it in a `|block|`.

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

### `max-width` — wrap to fit

**`max-width: N`** caps a box's auto width; **`text-wrap: wrap | nowrap`** (default
`wrap`) says whether text inside breaks into lines to honour the cap — both inert
without a finite `max-width`. Wrapping prefers **whitespace** and falls back to
breaking inside a word (grapheme boundaries), so the no-clip / no-spill law holds at
any width. The wrapped size **is** the measured size — it feeds auto-sizing, grid
tracks, gutters, spacing, link labels, and routing obstacles alike. Wrapping is
decided **once**, at measurement, against `max-width` (or the content's natural
width); a later `stretch` widens the box, never the line breaks. Three errors
keep it honest ([SPEC 21](#21-errors)): `nowrap` text that cannot fit the cap, a
**non-text** child wider than the cap (only text wraps), and a `width` floor above it.

Exceptions: a **text** node sizes to its glyphs (no padding), widened by
`letter-spacing` and given `line-spacing` between `\n` lines; `|icon|` is a square
that grows with its `[ ]` text (a `32` floor) and needs a `symbol`; `|line|` / `|poly|` /
`|image|` / `|path|` require their geometry (`points` / `src` / `path`) and error
without it. `|block|` carries `padding: 0`, so a bare block sizes to its content
exactly.

**Text is measured from real metrics.** Width = Σ per-glyph advances at the
compile-resolved `font-weight`, read from the bundled metrics tables
([SPEC 6](#6-paint-stroke--text)); no kerning or shaping (≈ 1 % of a line, the
documented tolerance — [SPEC 24](#24-deferred)). **Metrics follow the kind, not
the name**: a mono `font-family` — a known-mono name, or any name containing
"mono" — measures on the mono table (exactly **0.6 em per glyph**, at every
weight), every other family, the bundled default included, on the proportional
table. An unknown glyph
falls back to a fixed advance (wide for the CJK ranges). Vertical centring is
**cap-height optical centring**, from the same tables.

---

## 6. Paint, Stroke & Text

The visual vocabulary shared by every node. These are ordinary properties — the full
list, with value shapes and defaults, is the [Property Ledger](#17-property-ledger--support);
the colour system they draw on is [SPEC 10](#10-colour-variables--expressions). This section
is the *behaviour*.

### Paint

**`fill` paints a body, `color` a label.** `fill` is a closed shape's interior (and,
on text, an alias for its `fill`); `color` sets text colour for a subtree and
cascades through the SVG via native `currentColor` — set it on a container to recolour
every descendant's text that doesn't override. `opacity` (0–1) fades a node whole.
`fill`, `stroke`, and `gap-fill` each accept a **gradient** as well as a flat colour
([SPEC 10](#10-colour-variables--expressions)).

### Stroke

**One stroke role paints a shape's outline and a link's wire alike** — `stroke` the
colour, `stroke-width` the thickness (markers scale with it), `stroke-style` the dash
pattern (`solid` / `dashed` / `dotted`, plus the drafting `center` / `phantom` on
shapes and `|line|`s and `wavy` on links — [SPEC 7](#7-nodes)). There is no parallel
`link-*` family: a `.class` carrying `stroke` dresses whichever wears it, node or link
([SPEC 9](#9-links)). A closed primitive's default outline is `--stroke` at width 2; a
`|group|` softens to width 1.

### Text

The text family — `font-family`, `font-size`, `font-weight`, `font-style`,
`text-transform`, `text-decoration`, `letter-spacing`, `line-spacing`, and `color` —
**inherits**: nearest ancestor wins, like CSS. Set it on a containing box (or the root)
and it cascades down, or on a string's own block (`"x" { font-weight: bold }`) for
that one text node. Body text defaults to `font-size` 15, `font-weight` `500`.
**Chrome text scales with the body**: a caption reads 12∕15 and a link label
11∕15 of the *inherited* `font-size` — 12 and 11 at the default — so one
`font-size:` scales the whole scene; an explicit `font-size` on either
(`|caption| { }`, `|-| { }`, a class, an own block) is absolute. (A drawing's
annotation text stays the sheet convention, 12 — [SPEC 15.1](#151-the-container-the-datum--the-scale).)

**Two bundled families** (both SIL OFL 1.1) carry the metrics ([SPEC 5](#5-the-box-model)):
**Google Sans**, the proportional default, and **Google Sans Code**, the mono one
declaration away (`font-family: "Google Sans Code"`) — four static roman weights each.
A `font-family` **override changes only the emitted name**: measurement stays by
kind (mono vs proportional), so a runtime CSS restyle keeps the compiled layout
box. **`font-weight`** takes `normal | medium | semibold | bold | 400 | 500 | 600 |
700` (`normal` = 400, `bold` = 700; arbitrary 100–900 is deferred —
[SPEC 24](#24-deferred)); measurement reads the resolved weight (mono advances are
weight-invariant). How fonts leave the compiler — names, embedded, outlined — is
[SPEC 18](#18-svg-output)'s three output modes.

**Line alignment rides the packing knob — there is no `text-align`.** A text leaf's
lines — wrapped ([SPEC 5](#5-the-box-model)) or authored `\n` lines — align per its
**nearest container box's horizontal packing knob**: `justify` in a `row` (so, by
default — [SPEC 11](#11-the-layout-model)), `align` in a `column` or grid context,
mapped `start` / `center` / `end` (`stretch` /
`evenly` / `origin` read as `center`). The knob reaches the lines even when the box
has no slack to move children; every box is a container, so the box holding the
text decides, and the default is `center` everywhere. Split intents wrap the text
in its own `|block| { justify: … }` — the table rule ([SPEC 12](#12-flow-grid--tree))
generalised to every box, which is why there is no second `text-align` knob.

Two kinds of text property, split by whether they touch layout:

- **Baked spacing** — `letter-spacing`, `line-spacing`, and `font-size` — changes
  **layout** (the text box grows to fit the wider glyphs or taller block) and compiles
  into the glyph and line positions, never emitted as a style ([SPEC 1](#1-mental-model)).
  `letter-spacing` / `line-spacing` default to 0, so text is unaffected until set.
- **Live CSS** — `font-style`, `text-transform`, `text-decoration` — does *not* touch
  layout: it rides the class / `<g>` / `.lini` rule and a host page can override it. Set
  any in the global block to style the whole scene.

For a global `font-family` / `color`, prefer the `--lini-font-family` /
`--lini-text-color` variables (or the `--theme` CLI flag, [SPEC 20](#20-cli)) for an **embeddable** diagram — they stay
live for a host page to re-theme, where a global property bakes its value into the
`.lini` rule ([SPEC 10](#10-colour-variables--expressions), [SPEC 18](#18-svg-output)).

---

## 7. Nodes

12 primitives. All accept position ([SPEC 5](#5-the-box-model)) and paint ([SPEC 6](#6-paint-stroke--text));
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
| `\|poly\|` | `points` | ≥3 points, local (centre-origin) coords. Closed. |
| `\|path\|` | `path` | Raw SVG path. **Native top-left coords.** |
| `\|line\|` | `points` | 2+ points. Markers via `marker*:`. |
| `\|icon\|` | `symbol` | A **Phosphor** icon — `symbol:` (or the label) names it; paints two-tone like a box (`fill` body, `stroke` line, counter-scaled `stroke-width`). A square that grows with its `[ ]` text (`32` floor); `\|sign\|` is the larger preset. See [Icons](#icons). |
| `\|image\|` | `src`, `width`, `height` | A picture — `src:` a URL, a data URI, or a **local path**; local files **embed** (see [Images](#images)); both dimensions required. `fit` maps it into the box — `auto` (default, letterbox), `contain`, `cover`, or `stretch`. |
| `\|sketch\|` | `draw` | A **pen** that folds to a path — profiles drawn call by call, with named points and edges, mirroring, and view breaks ([SPEC 15.3](#153-the-sketch-pen)). Closed-primitive paint; bbox from the geometry. |

**`radius`** rounds a rectangle's corners — `|box|` defaults to 8, `|block|` / `|rect|`
to 0. It is honoured on the rectangle (and on a multi-point `|line|`'s joins); `radius`
on the non-rect primitives (hex / diamond / slant / poly) is deferred ([SPEC 24](#24-deferred)).

### Visual modifiers (closed primitives)

| Property | Forms | Effect |
|---|---|---|
| `stroke-style` | `solid` / `dashed` / `dotted` / `center` / `phantom` | Stroke pattern. Default `solid`. `center` (dash-dot) and `phantom` (dash-dot-dot) are the drafting line conventions — axes and alternate positions — valid on shapes and `\|line\|`s everywhere ([SPEC 15.7](#157-leaders-notes--line-conventions)); a link's set stays `solid` / `dashed` / `dotted` / `wavy` ([SPEC 9](#9-links)). `wavy` is **link-only by design** — a wire waves, an outline never does. |
| `stack` | `N` / `dx dy` | Draw an offset duplicate behind the node. Scalar `N` = `N -N`. |
| `rotate` | `N` degrees | Rotate around the bbox centre ([SPEC 5](#5-the-box-model)). |
| `shadow` | `N` / `dx dy` / `dx dy blur` / `dx dy blur color` | Drop shadow via SVG `<filter>`. Scalar `N` = offset `N N`, blur `N`; tint defaults to `--lini-shadow-color`. |

### Markers (on `|line|` and links)

| Property | Effect |
|---|---|
| `marker: X` | Both ends. |
| `marker-start: X` | Start end (link source). |
| `marker-end: X` | End end (link target). |

Values: `none`, `arrow`, `dot`, `circle`, `diamond`, **`datum`** (the filled drafting
triangle a drawing's `>-` leader lowers to — [SPEC 15.7](#157-leaders-notes--line-conventions)), and the ER **cardinality set** —
`crow` (the "many" foot), `one` (a bar `|`), `exactly-one` (a double bar `‖`),
`zero-or-one`, `one-or-many`, `zero-or-many` (a bar or `○` paired with the foot). The
compositional operators `-+` / `-<` / `-o+` / `-+<` / `-o<` / `-++` are sugar over this
set ([SPEC 9](#9-links)). `circle` is a larger `dot` — a filled point sized for
hovering or reading (on a chart line it marks a data point; [SPEC 14](#14-charts)). Markers scale
with `stroke-width` (a link's wire and a shape's outline alike; the size law is
[SPEC 19](#19-compile-pipeline)'s); colour follows the stroke.
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

Setting the symbol twice — a label *and* `{ symbol: … }` — is an error; pick one.

Phosphor icons are **two-tone** (a soft fill behind a line), so an icon wears Lini's
paint roles like any node: **`fill`** the body (default the soft grey `--icon-fill`),
**`stroke`** the line (default `--stroke`, matching borders and wires),
**`stroke-width`** its weight (default 2). A single-tone line icon is `fill: none`.

`stroke-width` is **counter-scaled**: an icon is authored on a 256-unit grid and fit
to its box, and the stroke is divided by that scale (baked at compile time), so its
line weight holds as the icon resizes.

An icon is a **square** that grows uniformly with its `[ ]` text (and `padding`): the
side is a `32` floor (`icon-size`) over the text + padding on either axis — an empty
icon is 32×32; a longer label scales the **whole icon up**, symbol and all, never
distorting it. For a larger stand-alone icon, reach for `|sign|` ([SPEC 8](#8-templates)).

**`fit`** controls how the symbol fills that box. `auto` (default) keeps Phosphor's
authored framing — each glyph's built-in 256-grid margin — so a row of mixed icons
reads at an even weight; `contain` scales the glyph's *own* bounds up to meet the
box (`|sign|`'s default); `cover` scales until the box is covered (may overflow);
`stretch` fits both axes (may distort). The counter-scaled `stroke-width` follows
the resulting scale, so line weight is constant whichever `fit` you choose.

A missing `symbol` errors like `|poly|` without `points`; an unknown one suggests the
nearest name. Only the icons a diagram uses are embedded (a default-on `icons` feature,
[SPEC 24](#24-deferred)).

### Images

`|image|`'s `src:` takes an **HTTP(S) URL**, a **`data:` URI**, or a **local path**,
resolved against the source `.lini` file's directory. A local file's bytes are read
once, at resolve — a missing or unreadable path errors at the `src:` span — and
**embedded** in the output ([SPEC 18](#18-svg-output)): SVG as a nested, id-isolated
`<svg>`; raster (PNG / JPEG / GIF / WebP) as a base64 data URI. Embedding is the one
behaviour for a path (there is no opt-out — a self-contained SVG is the output
contract) and is **deterministic from the bytes**: the same file and assets give
byte-identical output on every run. The compiler never touches the network — URLs and
authored data URIs pass through untouched, so a URL is the authored non-embedded
form. Under `lini serve`, assets resolve inside the served root only
([SPEC 20](#20-cli)).

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
| `\|caption\|` | `\|block\|` | `pin: top left; translate: 0 -20; color: --caption-color; font-size: 12∕15 of inherited ([SPEC 6](#6-paint-stroke--text)); font-weight: --caption-font-weight` | A title, pinned just above the group's top-left corner. |
| `\|footnote\|` | `\|caption\|` | `pin: bottom; translate: 0 19; color: --footer-color` | A caption flipped to a shape's bottom edge — a centred, muted footnote. |
| `\|badge\|` | `\|block\|` | `pin: top right; translate: 6 -6; radius: 8; padding: 2 6; shadow: 2 3 3; fill: --accent; color: --accent-text; font-size: 11; font-weight: normal` | Corner pill — nudged out over the top-right corner, grows nothing. |
| `\|row\|` | `\|block\|` | `direction: row` | Frameless wrapper — children in a row. |
| `\|column\|` | `\|block\|` | `direction: column` | Frameless wrapper — children in a column. |
| `\|grid\|` | `\|block\|` | `layout: grid` | Frameless grid (needs `columns`). |
| `\|sign\|` | `\|icon\|` | `width: 64; height: 64; padding: 4; stroke-width: 2; fit: contain` | A larger icon as a stand-alone node, with room for a short label; `fit: contain` fills the box (unlike a bare `\|icon\|`). |
| `\|table\|` | `\|group\|` | `layout: grid; align: stretch; justify: stretch; gap: 1; gap-fill: --stroke; padding: 0; fill: none; stroke: --stroke; stroke-width: 2; stroke-style: solid; font-size: 14; font-weight: normal; scale: 1` | Ruled grid (see below). |
| `\|cell\|` | `\|block\|` | `padding: 4 8` | A **table cell** — a frameless `\|block\|` carrying the text-to-gutter inset (the mechanism: **Tables**, below). |
| `\|header\|` | `\|cell\|` | `fill: --header-fill; font-weight: semibold` | A **header** cell — a filled, semibold band (a `\|table\|`'s first row; an `\|entity\|`'s title spans them). |
| `\|footer\|` | `\|cell\|` | `color: --footer-color` | A **footer** cell — muted text; opt-in on the last row. |
| `\|entity\|` | `\|table\|` | `columns: auto, auto` | An ER / database **entity** — a titled field list, rows left-aligned (see below). |
| `\|topic\|` | `\|block\|` | `fill: --fill; stroke: --stroke; stroke-width: 2; radius: 8; padding: 8 14` | A tree's **structural** node — topic nesting is the hierarchy, anything else in its `[ ]` is the topic's own content ([SPEC 12](#12-flow-grid--tree)); custom structural types derive from it (`\|person::topic\|`). Tree-only. |
| `\|mindmap\|` | `\|topic\|` | `layout: tree; direction: bilateral; routing: natural` — plus the palette walk, the depth ramp, and `max-width: 160` on topics (see below) | A **mindmap** — the node is the visible **root topic**, its `[ ]` topics the first-level branches ([SPEC 12](#12-flow-grid--tree)). |
| `\|note\|` | `\|block\|` | `fill: --fill; stroke: --stroke; padding: 20; scale: 1` | A **note** — the folded-corner callout card, one type in every layout (see below). |
| `\|balloon\|` | `\|oval\|` | `width: 16; fill: --fill; stroke: --stroke; font-size: 11; scale: 1` | An item **balloon** — the numbered circle an assembly leaders to a part ([SPEC 15.8](#158-assemblies-views-sheets--titles)). |
| `\|drawing\|` | `\|block\|` | `layout: drawing; padding: 0` | An engineering **drawing** — geometry on a datum, measured annotations; `scale:` is its drafting ratio, default 1 ([SPEC 15](#15-drawing)). |
| `\|hole\|` | `\|oval\|` | `fill: --bg; stroke: --stroke-dark` — `width:` **required**, the diameter | A round **hole** — punches by paint order, draws its own centre marks ([SPEC 15.4](#154-features-holes--patterns)). |
| `\|centerline\|` | `\|line\|` | `stroke-style: center; stroke: --stroke-light; stroke-width: 1; fill: none` — needs `points:` | The dash-dot axis / symmetry line ([SPEC 15.7](#157-leaders-notes--line-conventions)). |
| `\|pitch-circle\|` | `\|oval\|` | `stroke-style: center; stroke: --stroke-light; stroke-width: 1; fill: none` — `width:` **required**, the diameter | The dash-dot bolt circle; round, so a `(o)` reads its PCD ([SPEC 15.7](#157-leaders-notes--line-conventions)). |
| `\|breakline\|` | `\|line\|` | `stroke: --stroke-light; stroke-width: 1; fill: none` — needs `points:` | A break cut's edge — the thin jogged line a `break:` generates ([SPEC 15.3](#153-the-sketch-pen)); manual use is free. |
| `\|halo\|` | `\|line\|` | generated chrome — `halo-margin` each side | An annotation line's **crossing knockout** over geometry ([SPEC 15.7](#157-leaders-notes--line-conventions)); `\|halo\| { … }` restyles or removes them scope-wide. |
| `\|threadline\|` | `\|line\|` | `stroke: --stroke-light; stroke-width: 1; fill: none` — generated chrome | A thread's ISO 6410 thin line — the minor/major run and the ¾ arc a `thread:` generates ([SPEC 15.3](#153-the-sketch-pen), [SPEC 15.4](#154-features-holes--patterns)). |
| `\|hidden\|` | `\|sketch\|` | `stroke-style: dashed; stroke: --stroke-dark; stroke-width: 1; fill: none` — needs `draw:` | **Hidden edges** — interior geometry on its own dashed child, per the one-node-one-stroke-style law ([SPEC 15.7](#157-leaders-notes--line-conventions)). |
| `\|shoulder\|` | `\|line\|` | `stroke: --stroke-dark; stroke-width: 2; fill: none` — needs `points:` | A turned part's **shoulder line** — the geometry-weight edge a `revolve:` generates at every sharp diameter change ([SPEC 15.3](#153-the-sketch-pen)); manual use is free. |
| `\|plane\|` | `\|line\|` | `stroke-style: center; stroke: --stroke-light; stroke-width: 1; fill: none` | The **section-plane** line on the source view — its label the section letter; `at:` stations it, `facing:` turns its arrows; a `\|drawing\| { of: }` sections it ([SPEC 15.8](#158-assemblies-views-sheets--titles)). |
| `\|magnifier\|` | `\|oval\|` | `stroke: --stroke-light; stroke-width: 1; fill: none` — `width:` **required**, the region diameter | The **detail marker** — rings a region on the source view, its label the detail letter at the rim; a `\|drawing\| { of: }` details it ([SPEC 15.8](#158-assemblies-views-sheets--titles)). |
| `\|projection\|` | `\|line\|` | `stroke: --stroke-light; stroke-width: 1; fill: none` — needs `points:` | A **projection construction line** — the straight thin line a sheet's cross-view link generates ([SPEC 15.8](#158-assemblies-views-sheets--titles)); manual use is free. |
| `\|surface-finish\|` | `\|block\|` | `symbol: basic; stroke: --stroke-dark; stroke-width: 1; fill: none; font-size: 12; scale: 1` | The ISO 1302 surface-**texture** symbol — its label the textual indication, `symbol:` the vee variant; drawing-scope ([SPEC 15.9](#159-drafting-symbols--annotation-composition)). |
| `\|feature-control\|` | `\|block\|` | `stroke: --stroke-dark; stroke-width: 1; fill: --bg; font-size: 12; scale: 1` | The GD&T **frame** — characteristic, tolerance, datums in ruled compartments; rows via `\|control\|`; drawing-scope ([SPEC 15.9](#159-drafting-symbols--annotation-composition)). |
| `\|control\|` | `\|block\|` | — | One **frame row** — its label the characteristic; a `\|feature-control\|` child only ([SPEC 15.9](#159-drafting-symbols--annotation-composition)). |
| `\|datum\|` | `\|block\|` | `stroke: --stroke-dark; stroke-width: 1; fill: --bg; font-size: 12; scale: 1` | The framed **datum letter** as a node — its label the letter, an identity like `>-`'s; drawing-scope ([SPEC 15.7](#157-leaders-notes--line-conventions), [SPEC 15.9](#159-drafting-symbols--annotation-composition)). |
| `\|page\|` | `\|block\|` | `layout: flow; fill: --bg` — `sheet: a4` unless sized; `direction` by orientation | An ISO 5457 drawing **sheet** — mm dimensions via `sheet:`; px per mm from the root `density:`; frame, zones, and centring marks as generated chrome ([SPEC 15.8](#158-assemblies-views-sheets--titles)). |
| `\|title-block\|` | `\|table\|` | `font-size: 14; font-weight: semibold; stroke-width: 1` | The ISO 7200 **title block** — a table the `\|page\|` seats flush inside its frame's bottom-right corner. **Field properties** (`title`, `drawing-number`, `revision`, `date`, `sheet-number`, `author`, …) build the standard grid, absent fields collapsing; its **smart label is the `title` field**; plain cells stay a fully custom block ([SPEC 15.8](#158-assemblies-views-sheets--titles)). |
| `\|frame\|` | `\|rect\|` | `fill: none; stroke: --stroke; stroke-width: 2` | A sheet's **frame** — the thick border a `\|page\|` generates at the ISO margins ([SPEC 15.8](#158-assemblies-views-sheets--titles)). |
| `\|field\|` | `\|block\|` | `font-size: 12; font-weight: normal; color: --footer-color` | A **title-block field's caption**, over its value — quieter in size, weight, and tone ([SPEC 15.8](#158-assemblies-views-sheets--titles)). |
| `\|zone\|` | `\|block\|` | `font-size: 11; color: --stroke-light` | A **zone reference** label (1, 2… / A, B…) a `\|page\|` generates in the margin band ([SPEC 15.8](#158-assemblies-views-sheets--titles)). |
| `\|tick\|` | `\|line\|` | `stroke: --stroke; stroke-width: 1; fill: none` — needs `points:` | A zone **divider** / **centring mark** a `\|page\|` generates ([SPEC 15.8](#158-assemblies-views-sheets--titles)). |
| `\|schematic\|` | `\|block\|` | `layout: schematic` | A circuit sheet's scope ([SPEC 16](#16-schematic)). |
| `\|component\|` | `\|block\|` | `fill: --component-fill; stroke: --component-stroke; stroke-width: 1.5; radius: 0; padding: 8; prefix: "U"` | The generic pin-bearing part — IC, module, relay; ref prefix U ([SPEC 16.2](#162-components--pins)). |
| `\|pin\|` | `\|block\|` | name inside, `number:` outside, stub outward — no `side:` default (the bilateral split, [SPEC 16.2](#162-components--pins)) | A component **terminal** — the wire lands on its stub tip. |
| `\|label\|` | `\|block\|` | `shape: plain; font-size: 11; color: --label-ink` | The **net tag** — text, a symbol (`gnd`, `power`, …), or both; its own terminal ([SPEC 16.4](#164-labels)). |
| `\|junction\|` | `\|oval\|` | `fill: --wire; stroke: none` — generated chrome | The connection **dot** where ≥ 3 wire ends meet ([SPEC 16.5](#165-wires)). |
| `\|J\|` | `\|component\|` | `prefix: "J"` — pins nameless, `pins: N` generates them | The **connector** ([SPEC 16.2](#162-components--pins)). |
| `\|opamp\|` | `\|component\|` | `prefix: "U"` — pins `out` `inp` `inn`; power pins hidden | The amplifier triangle ([SPEC 16.2](#162-components--pins)). |
| the **discretes** — `\|R\|` `\|C\|` `\|L\|` `\|D\|` `\|LED\|` `\|Q\|` `\|Y\|` `\|F\|` `\|FB\|` `\|SW\|` `\|BT\|` `\|V\|` `\|I\|` | `\|block\|` | symbol-bodied, generated pins, `symbol:` variants | The two/three-terminal parts; the type is the ref family ([SPEC 16.3](#163-discretes)). |
| `\|gnd\|` / `\|nc\|` | `\|label\|` | `symbol: gnd` / `symbol: nc` | Built-in ground / no-connect defines ([SPEC 16.4](#164-labels)). |
| `\|floorplan\|` | `\|drawing\|` | `layout: floorplan` | An architectural **floor plan** — the drawing engine in a dialect, so `\|drawing\|`-scoped rules dress it too ([SPEC 15.11](#1511-floorplan--the-architectural-dialect)). |
| `\|wall\|` | `\|sketch\|` | `fill: --stroke-dark; stroke: none` — `draw:` traces the **centreline**; `thickness:` inherited (200 mm) | A **wall run** — offset to a solid (poché) outline at lowering; openings ride its `[ ]` ([SPEC 15.11](#1511-floorplan--the-architectural-dialect)). |
| `\|partition\|` | `\|wall\|` | `thickness: 100` (mm — the true-size law, [SPEC 15.11](#1511-floorplan--the-architectural-dialect)) | The thinner interior wall — a define, nothing more. |
| `\|door\|` | `\|block\|` | `on:` **required**; `width: 900` (mm); `hinge: start`; `swing: left` | A wall **opening** — gap + leaf + swing arc; `symbol: single / double / sliding` ([SPEC 15.11](#1511-floorplan--the-architectural-dialect)). |
| `\|window\|` | `\|block\|` | `on:` **required**; `width: 1200` (mm) | A glazed wall opening — gap + sill lines ([SPEC 15.11](#1511-floorplan--the-architectural-dialect)). |
| the **fixtures** — `\|bed\|` `\|sofa\|` `\|dining\|` `\|bath\|` `\|appliance\|` `\|stairs\|` | `\|block\|` | `stroke: --stroke-dark; stroke-width: 1; fill: --bg` — symbol-bodied, true-size mm defaults; `symbol:` variants (`\|stairs\|` takes none — `steps: N` **required**) | The furniture set — thin outline, masking what it overlaps ([SPEC 15.11](#1511-floorplan--the-architectural-dialect)). |

The bare `|block|` is the base everything rectangular builds on — frameless, yet a real
box (id, class, children, wirable, positionable): what you reach for to wrap text that
needs box behaviour.

**Captions.** Both caption templates are out-of-flow overlays — they never push
the content, their place fixed by the template, not by where they sit among the
children, so a `row`-laid group carries its title just the same. A group's
**label is its caption** ([SPEC 3](#the-label)), so `|group#panel| "Settings" [ … ]`
and `|group#panel| [ |caption| "Settings" … ]` are equal;
`|caption| { font-size: 16 }` styles every caption without touching body text.

**Notes.** A `|note|` is the callout card — a filled block with a folded top-right
corner. It is **one type in every layout**: in a `sequence` it binds to lifelines with
`place:` ([SPEC 13](#13-sequence)); in a `drawing` it places at the
datum, usually wired by a leader ([SPEC 15.7](#157-leaders-notes--line-conventions)); in
flow / grid it is an ordinary padded card. Built-in scoped rules — `|sequence| |note|`
and `|drawing| |note|`, each `{ padding: 6 10; font-size: 13 }` — keep it compact where
convention expects; override them like any rule.

**Topics & mindmaps.** A `|topic|` is the tree's structural node
([SPEC 12](#12-flow-grid--tree)), a compact card whose label is its centred
text. `|mindmap|` is the visible root topic owning the scene — `layout: tree;
direction: bilateral; routing: natural` — plus three deterministic garnishes,
each lowered at desugar as ordinary generated rules (visible in `lini
desugar`, overridable like any rule): the **palette walk** — each first-level
branch takes the next hue ([SPEC 10.2](#102-the-colour-palette)) in
declaration order, red and grey skipped, and tints its subtree at the tiers
(`wash` fill, `deep` stroke **and branch wires**, `ink` text; the root stays
neutral, explicit paint wins, cross-links stay neutral, dark mode free); the
**depth ramp** — `.lini-level-N` rules size the tiers (root largest, level 1
medium, deeper small); and **topic wrap** — `max-width: 160`, so a long label
wraps into a card instead of stretching an arm. A plain `layout: tree`
carries none of these — org charts read monochrome.

**Tables.** A `|table|` is pure sugar over the bundle above — its 1px
`gap-fill` gutters paint as hairline rules ([SPEC 11](#11-the-layout-model)).
Each body cell wraps in a `|cell|`; `|header|` / `|footer|` build on it, so
every cell — but not the caption, a plain `|block|` — carries the inset. Style
all cells with `|cell| { … }`, or per table with `|table| |cell| { … }`. The
table's `align: stretch; justify: stretch` makes **every cell fill its track**
— backgrounds fill and text has room. A table's label is its caption.

**Column alignment.** `align` (↔) / `justify` (↕) on the table read per column
([SPEC 12](#12-flow-grid--tree)) and align the *cells' text*: since the cells already fill, the
table's own `align`/`justify` are carried onto each cell — a `start`/`end` column's cells
wear a `.lini-align-*` / `.lini-justify-*` class — and a filled cell places its text at
that edge (`center` is the default). So `align: start, center, end` reads three columns
left / centre / right, header band and body alike.

A table's **first row becomes its header** — each cell wrapped as a `|header|`, a filled
semibold band; `|table| |header| { font-weight: normal; fill: none }` reverts it. A **footer**
is opt-in: wrap a last-row cell in `|footer|`. Every cell is a box now — header/footer
carry a fill; a body cell is a frameless `|block|` wrapping its text, so the padding rule
and the column's alignment reach it ([SPEC 18](#18-svg-output)).

```
|table#basket| {
  columns: 80, 140, 80;
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

**Entities.** An `|entity|` is sugar over `|table|` (two auto columns) for an ER /
database card: its **label is its title** — a `|header|` spanning every column, centred
over left-aligned `"field" "type"` rows (an entity's field rows read left by default; the
title keeps its centred, full-span band). Add a column for a **key** gutter —
`{ columns: auto, auto, auto }` gives `"PK"/"FK" "field" "type"`. In an entity (not a plain
table) a `|header|` / `|footer|` cell spans the full width.

```
|entity#users| "Users" [ "id" "int"  "name" "varchar" ]
```

Relationships are ordinary links with the ER cardinality operators ([SPEC 9](#9-links)):
`users -< orders` is one-to-many, `a >-< b` many-to-many, landing on the entity
edge. To anchor a wire to one **field**, give that cell an id
(`|block#user_id| "user_id"`) and link the path (`orders.user_id -< users.id`).
Keys are plain content (`"id" { font-weight: bold }`); an entity adds no grammar.

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
parallel family. Only **`clearance`** and **`routing`** stay scene config
([Styling](#styling)).

### Operators

A link op is `[start_marker?][line][end_marker?]`, no spaces:

| Part | Tokens |
|---|---|
| Line | `-` solid · `--` dashed · `---` dotted · `~` wavy |
| Start markers | `<` arrow · `>` crow · `*` dot · `<>` diamond · `+`/`o` ER cardinality (below) |
| End markers | `>` arrow · `<` crow · `*` dot · `<>` diamond · `+`/`o` ER cardinality (below) |

The same marker glyph differs by position (`<` is arrow at the start, crow at the
end).

| Op | Markers / Line |
|---|---|
| `->` `<-` `<->` | arrow combinations, solid |
| `-*` `*-` `*-*` | dot combinations |
| `-<>` `<>-<>` | diamond |
| `-<` `-+` `-o<` `-+<` `-o+` `-++` `>-<` | ER cardinality (crow's-foot, below) |
| `-->` `--->` `~>` | dashed / dotted / wavy |
| `-` `--` `---` `~` | no markers (each line style) |

An operator with no marker glyphs leaves both ends bare. Explicit `marker:` /
`marker-start:` / `marker-end:` override the operator (source order wins). The
operator's line part sets the link's `stroke-style` (`--` ⇒ `dashed`, `---` ⇒ `dotted`,
`~` ⇒ `wavy`); an explicit `stroke-style:` overrides it.

**ER cardinality — a crow's-foot marker, composed.** A cardinality marker reads
`[min][max]`: the **min** ring `o` (zero) or bar `+` (one) hugs the line, the **max**
bar `+` (one) or crow (many — `<` at the end, `>` at the start) sits outermost. **Either
end takes one; the two sides mirror** — `a +-< b` is one-to-many, `a >o-o< b` zero-or-many
both ways. The six relations, shown end-side:

| Op | Relation |
|---|---|
| `-+` | one |
| `-<` | many |
| `-o+` | zero-or-one |
| `-+<` | one-or-many |
| `-o<` | zero-or-many |
| `-++` | exactly one |

A lone `-o` (no max) errors; the hollow ring exists only inside the ER
cardinality glyphs and has no standalone endpoint form. The ops are sugar over the `marker:` set (`one`,
`exactly-one`, `zero-or-one`, `one-or-many`, `zero-or-many`, `crow` —
[SPEC 7](#7-nodes)); `marker*:` overrides.

### Syntax

```
endpoints op endpoints [op endpoints …] [ "label" ] [ .class… ] [ { style } ] [ [ labels ] ]
```

The tail is the **node tail** (`"label" .class { style } [ … ]`); only the head differs
— endpoints + operators, versus bars — and a link's `[ ]` holds only labels (text),
where a node's holds children (a drawing's dimensions and leaders alone may also
carry annotation nodes there — [SPEC 15.9](#159-drafting-symbols--annotation-composition)).

`endpoints` is one or more endpoints joined by `&`:

```
a -> b               // 1 link
a -> b -> c          // chain: 2 links
a -> b & c           // fan-out: a→b, a→c
a & b -> c           // fan-in
a & b -> c & d       // cartesian: 4 links
a -> b -> c & d      // chain + fan
```

Each hop carries its own wire operator; mixing operator *kinds* — a wire op with
a measure or mate — in one chain is a parse error. On a chain or fan, the label,
class, and `{ }` apply to every link the statement expands to.

**A chain marks every hop.** `a -> b -> c` is exactly `a -> b; b -> c` — desugar
expands the chain ([SPEC 19](#19-compile-pipeline)), so each hop carries the
operator's full markers and `lini desugar` shows the two links. A bare first hop
is spelled with the bare line op: `a - b -> c`. (Fan-out `&` is not sugar — its
shared trunk is routing geometry, [ROUTING.md](ROUTING.md).) A **schematic
scope** is the one carve-out, where a chain through a 2-pin part is a series
circuit rather than two statements ([SPEC 16.5](#165-wires)).

### Styling

The vocabulary is [SPEC 6](#6-paint-stroke--text)'s, at the ordinary defaults
([SPEC 17](#17-property-ledger--support)): `stroke` / `stroke-width` /
`stroke-style` dress the wire (the style usually set by the operator, above),
`color` and the `font-*` family its labels ([Labels](#labels)).

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
a -> b "hi" .flow { stroke: red; stroke-style: dashed }    // one link overrides
```

`clearance` (default 16) and `routing` (default
`orthogonal`) are **scene config** — geometry, not paint — set on a container's `{ }`,
cascading to that scope's links, nearest winning; the router then inflates every
node's keep-out by the **maximum** clearance any link carries
([ROUTING.md](ROUTING.md)). `marker*` come from the operator and
override per link.

### Labels

A link's label is **text**, placed along the route by `along:` — the link's track
rule, exactly as `columns:` is a grid's. One label trails the head (`a -> b
"watches"`); two or more, or a styled one, ride the `[ ]`:

| Property | Notes |
|---|---|
| `along` | A list of `0..1` fractions along the whole drawn route, one per label (`along: 0.2, 0.5, 0.8`). Omitted → auto-distribute across the hops, so one label avoids junctions and several spread out. |

```
a -> b "watches"                                // the common case — one label, auto-placed
a -> b "watches" .loud { stroke: red }          // + a class and wire colour
a -> b { along: 0.3, 0.7 } [ "near a" "near b" ] // two labels
a -> b [ "watches" { translate: 0 -6 } ]        // a styled / nudged label
```

Each label is an ordinary **styleable text leaf**; the head label takes no style
([SPEC 3](#3-statements--the-label)) — a styled label rides the `[ ]`, exactly as a
node's does. Keep one link's labels in **one** `[ ]` — a head label *and* a
`[ ]` of labels on the same link warns ([SPEC 21](#21-errors)). A label is an obstacle to nothing, and may slide along the link to keep
clear of nodes and other labels; the link never moves for it. Link labels ride the
chrome size ([SPEC 6](#6-paint-stroke--text)) at `font-weight: normal`; a link's text props
cascade to its labels
(`|-| { font-size: 14; color: --blue }` restyles every link's labels at once,
absolutely).

### Endpoints & scope

```
endpoint = ( ident | ident_bars ) { "." ident } [ ":" side ]
side     = top | bottom | left | right
```

A path walks with `.` into children; a final `:side` forces a side. An
endpoint may open with an **identity capsule** instead of an id — see
[Capsule endpoints](#capsule-endpoints) below. Every link
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

### Capsule endpoints

An endpoint position may hold an **identity capsule** — bars, exactly as a
declaration writes them: `|type|` or `|type#id|`. A capsule **declares and
links in one statement**: desugar hoists it to an ordinary declaration at
the statement's position in its scope and the link references it
([SPEC 19](#19-compile-pipeline)), so it is the *typed* form of
declaration-at-first-use ([SPEC 3](#implicit-nodes)):

```
cat -> |cyl#db|                    // declare db (empty, per SPEC 3), link to it
cat -> |cyl#db| "watches" { … }    // the tail is the LINK's, as always
a -> |box| -> c                    // anonymous mid-chain — a minted internal id
a & b -> |gnd|                     // a fan into ONE instance
```

Three existing laws govern it, none new:

- **A statement's tail belongs to its head** ([SPEC 3](#3-statements--the-label)) —
  a capsule takes no label, class, style, or children; everything after it is
  the link's.
- **Identity travels, dress doesn't** ([SPEC 1](#1-mental-model)) — the id
  inside the bars comes along; classes never sat inside bars.
- **Declared nodes are empty unless labelled** ([SPEC 3](#the-label)) — a
  define supplies intrinsic content (`|vm::label| { symbol: power } [ "VM" ]`).

The capsule composes with the rest of the endpoint's anatomy (`.path`,
`.index`, `:side`) — though an **inline** capsule has no authored pins, so a
pin path on one (`|component#U9|.p4`) is an error; anonymous capsules mint
reserved internal ids (`lini-cap-N`); an id'd capsule declared twice is the
ordinary duplicate-id error. At statement head, a capsule followed by a link
operator opens a link ([SPEC 1](#1-mental-model)); followed by anything else
it is the node declaration it always was. A **drawing scope rejects
capsules** — a drawing never invents an endpoint ([SPEC 15](#15-drawing)); a
sequence accepts them (a typed participant).

Bodies are **sealed**: a body link connects nodes of its own subtree only.
Cross-container links are written at the lowest level where both ends are visible —
usually the root. Without a side the router picks edges by geometry; with a `:side`,
that edge is forced.

An **anonymous** container opens no scope: it is **scope-transparent** — its
children belong to its parent's scope (ids stay unique across it), a dot-path
never names it, and its own `[ ]` links resolve in the parent's scope. Name a
container to give its children a dot-path of their own. A sequence frame is
transparent the same way ([SPEC 13](#13-sequence)). Scope-transparency is
about **names**, not geometry: the router sees the container itself, so links
route inside an anonymous group exactly as inside a named one; its scene
config (`clearance:`, `routing:`) cascades onto the links written in it
([ROUTING.md](ROUTING.md) Model step 1); and a **layout-owning** container
realises the statements written in it whether or not it is named — an
anonymous `|drawing|` draws its own dimensions, an anonymous `|sequence|` lays
its own messages on the time axis. The wiring strategy follows the container
that *wrote* the statement ([SPEC 11](#11-the-layout-model), seam 2), never the
dot-path its endpoints resolve against.

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

`routing` selects the strategy for a scope and cascades like `clearance`:
`orthogonal` (the default) routes horizontal/vertical runs through the free space
between nodes, corners rounded; `natural` fits direct **smooth curves** —
tangent-normal at both ends, bending gently around what they would hit, free to
cross ([ROUTING.md](ROUTING.md)); `straight` draws each link as one
segment between the bodies, trimmed to their boundaries — it avoids nothing and
reports nothing. `routing` pairs with
`layout` — `layout` places the nodes, `routing` wires them — so a group can route
its internals one way while the root routes another; which subsystem realises a
scope's links is the scope's **wiring strategy** ([SPEC 11](#11-the-layout-model)).

The full routing contract — clearance, spacing, crossings, fan-out, self-loops —
lives in [`ROUTING.md`](ROUTING.md), the source of truth for routing.

---

## 10. Colour, Variables & Expressions

CSS variables theme the **visual** layer — colours and the font family. Everything
that affects layout — sizes, gaps, padding, and font *size* — is a baked constant, so
a standalone SVG never depends on host CSS. This section also holds the **expression
engine** ([10.7](#107-expressions--functions)), the one place operators appear.

### 10.1 Visual variables (live, themeable)

Each colour is a `light-dark(LIGHT, DARK)` value, so one SVG carries both modes:

```
--lini-bg            light-dark(white, #1b1b1f)      the scene background
--lini-fg            light-dark(black, #e8e8ea)
--lini-fill          light-dark(white, #26262b)
--lini-stroke        light-dark(#444, #9aa0a6)
--lini-stroke-dark   light-dark(black, white)        the primary drafting tone — pen geometry, dimension/leader linework, and their heads read full black on white (the ISO print look)
--lini-stroke-light  light-dark(#0000008b, #ffffffa3) the secondary line tone — drafting's thin support lines (centerlines, break lines, extension lines): full black/white at reduced alpha, so a support line crossing dark geometry blends toward it instead of greying it
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
--lini-grid          light-dark(rgba(0,0,0,.1), rgba(255,255,255,.14))  the chart gridline tint
--lini-tip-bg        light-dark(#333, #e8e8ea)          the chart tooltip card's surface ([SPEC 14.8](#148-tooltips))
--lini-tip-fg        light-dark(white, #1a1a1f)         …and its text
--lini-font-family   "Google Sans", system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif
--lini-font-weight         500
--lini-caption-font-weight 400
--lini-link-font-weight    400
--lini-text-color    var(--lini-fg)
--lini-shadow-color  light-dark(rgba(0,0,0,.2), rgba(0,0,0,.5))
--lini-wire              light-dark(#0a7a2f, #4cc472)   the schematic wire
--lini-component-fill    light-dark(#fdf6d8, #3a3626)   a part's body
--lini-component-stroke  light-dark(#8a1c1c, #d98f8f)   a part's outline
--lini-label-ink         light-dark(#0e6a6a, #57c4c4)   the net tag
--lini-pin-number        light-dark(#00000073, #ffffff80)
--lini-sheet             light-dark(#faf5e6, #23221c)   the schematic scene wash
```

`--lini-bg` is the **paper tone** — what a root `fill: --bg`, a `|page|` sheet, and
a punched `|hole|` paint with. It is not painted unasked: a figure carries a
background only when the scene sets one ([SPEC 18](#18-svg-output)). The default stack leads with the bundled
proportional **Google Sans** ([SPEC 6](#6-paint-stroke--text)); its advances are
what the proportional metrics table measures, so a diagram measures identically
in every output mode ([SPEC 18](#18-svg-output)).

**Dark/light is automatic.** The compiler emits `color-scheme: light dark` on `.lini`,
so `light-dark()` follows the viewer's OS (`prefers-color-scheme`) — no script, no
`@media`. A `data-theme="dark"` / `"light"` on the SVG or any ancestor forces a mode
(it flips `color-scheme`, and its higher specificity beats the OS). All defaults sit in
`@layer lini.defaults`, so unlayered host CSS still wins with no `!important`.
`--static` freezes the light arm into literals for renderers without `light-dark()`
([10.6](#106---static)).

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

The job-names hold across the dark flip — `--teal-wash` is always the faint
surface, `--teal-ink` always the high-contrast detail:

```
{ |card::box| { fill: --teal-wash; stroke: --teal-ink } }   // a pretty card, one line
|box#n| { fill: --amber-soft }
```

The tiers are generated from one **OKLCH** seed per hue, so the ramp is perceptually
even and the eleven read as a family; the same space is open directly —
`fill: oklch(0.7, 0.14, 200)` ([SPEC 2](#2-lexical-syntax)). Aliases cover muscle
memory: `--yellow → --amber`, `--pink → --rose`, `--indigo → --purple`,
`--cyan → --teal`. `red` stays clear for **danger**; `rose` is the decorating pink,
`green` an emerald, `lime` the lemony one.

The palette is **tree-shaken** — only referenced variables are emitted
([SPEC 18](#18-svg-output)).

### 10.3 Gradients

`fill`, `stroke` (a shape's outline or a link's wire), and `gap-fill` accept a **gradient** in place of a flat colour. Stops are
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

Each distinct gradient is emitted once as a `<linearGradient>` / `<radialGradient>` in
`<defs>` and referenced by `url(#…)` — deduplicated and shared like the drop-shadow
`<filter>`s ([SPEC 18](#18-svg-output)). `objectBoundingBox` units fit one definition to
any node at any size. The stops being palette vars, a gradient themes, flips, and bakes
like any other paint; gradient-on-text is deferred ([SPEC 24](#24-deferred)).

**Hatches.** `hatch()` is a paint function beside `gradient()`, valid on **`fill`**
only — the drafting section-line texture, usable in any layout:

| Form | Result |
|---|---|
| `hatch(45)` | section lines at 45°, pitch 6 |
| `hatch(45, 6)` | explicit pitch (sheet-space px — hatch never scales, [SPEC 15.1](#151-the-container-the-datum--the-scale)) |
| `hatch(45, 6, --gray-deep)` | explicit line colour (default `--stroke`) |
| `hatch(45 -45, 6)` | a space-group of angles — cross-hatch |

Angles use the drawing bearing (0 = up, clockwise — [SPEC 15.3](#153-the-sketch-pen)).
Each distinct hatch emits one `<pattern>` in `<defs>`, deduplicated like gradients; the
colour is an ordinary paint, so hatching themes, flips dark/light, and bakes. Hatch
line width is fixed (0.75) — a texture, not a stroke. `hatch()` on `stroke` is an
error — a stroke takes a colour or gradient.

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
with a literal, a rule (`gap: 30;`, `|box| { radius: 4 }`), or a `(…)` expression /
binding ([10.7](#107-expressions--functions)).

### 10.5 Layout constants (baked)

Baked compile-time defaults — override per-node, on the root, in rules, or in an
instance / link block. The per-property values and the template bundles live in
the [Property Ledger](#17-property-ledger--support) — **every baked default has
one home** (the implementation's ledger module), so the whole look is tuned from
one place. The root's `padding` (20) frames the whole scene — the SVG margin.

The chrome constants below have no ledger row — they are engine anatomy, listed
here once. So are the chart-plane sizes ([SPEC 14.1](#141-the-chart-plane)): a
chart defaults **360 × 220**, a pie / radial chart a **280** square — a chart
cannot size to its content, so these stand in for `auto`.

The drawing chrome ([SPEC 15](#15-drawing)) — sheet-space, never scaled:

```
dim clearance 4 (the drawing scope's link default)
dim-ext-gap 3    dim-ext-overshoot 3     halo-margin 2
dim-arrow 12 × 4      datum-triangle 11   note-offset 14   note-landing 8
hatch-pitch 6    hatch line-width 0.75   break-gap 12     tol-stack 0.7
center-mark-overhang 4    drawing link stroke-width 1   drawing link font-size 12
```

The schematic chrome ([SPEC 16](#16-schematic)) — sheet-space:

```
schematic track gap 60    pin-pitch 20    pin-stub 20    junction 4 (radius)    tag-point 8 (a flag's nose reservation; the nose draws at 45°)
label-seat max(25, 2.5 × clearance) — a seat is a routing corridor, so it is
  derived from the scope's own clearance, floored at one pin pitch
net-label-run 40 (2 × pin-pitch — the floor on a plain net label's run of
  trace; a longer name grows it, `width:` raises the floor — SPEC 16.4)
net-label-offset 4 (the clear space that name keeps off the trace, and off
  the run's two ends)
pin-number offset 9 (across the lead)    readout offset 40 (beside a turned part's axis)
readout gap 8 (part edge → its ref / value)    readout stack 4 (between the two)
schematic clearance 10 (the scope's config — pin-pitch stays ≥ min pitch)
schematic link stroke-width 1.5    corner-radius 0 (the scope's link default)
```

### 10.6 `--static`

Class rules and inline `style=` work everywhere, but CSS *variables* don't — resvg
and librsvg fail `var()` in every position (browsers, even `<img>`-embedded, are
fine) — and neither honours `@font-face`. **`--static`**
keeps the rules but inlines every `var(--lini-name)` as its literal
**and outlines text to paths** ([SPEC 18](#18-svg-output)): no runtime theming, but
a self-contained SVG that renders identically anywhere, installed fonts or none.

### 10.7 Expressions & functions

A **parenthesized expression** `(…)` holds compile-time math — folded to a literal (a
number, or a point `(x, y)` for geometry) when the diagram compiles. Parentheses are the
**only place operators appear**: outside them `-` is a link or a number's sign, `<` / `>`
are markers, `//` a comment, so the parens are what let `*` mean "times". A value stays
paren-free until an operator does. **A call's own parens count**, so an operator
inside a call's arguments needs no inner group — what makes math usable inline
everywhere; a signed number is a sign, not an operator, so `-2` stays bare
(`translate: -35 20`) — to subtract, group it:

```
gap: 8;                     // a literal — bare
width: scale(3);            // a call — bare, no group
padding: (8 * 2);           // an operator → a group (= 16)
draw: move(-2, 5) up(8)     // calls and signed numbers — bare
draw: right(w / 2)          // an operator in a call's own parens — no group
```

Inside a group the language is small and total:

- **Operators** `+ - * / ^` (`^` power, right-associative), unary `-`, grouping `( )`,
  comparisons `< <= > >= == !=`, the ternary `cond ? a : b`.
- **Functions** — the math library `exp ln log sqrt abs sin cos tan min max clamp floor
  round pow`, and any you define (below); each returns a number or a point, called
  `name(args)`. (Colour / track builders like `rgb` / `repeat` make typed values, so
  they live in value position, never inside math.)
- **Constants** `pi`, `e`; **scientific notation** `1e6`, `1.32e-6`; the sample
  parameters `u` (geometry, below) and chart `x`; and your **bound names**, read bare
  (below). A bare name resolves: locals → the ambient (`u` / `x`) → `pi` / `e` → your
  bindings.
- **Locals** — `name = expr;` binds for the rest of the group; the **final expression is
  the value** (no keyword, no `return`). `=` binds, `==` compares. A top-level `,` makes
  the value a **point**. Values are numbers and points — no strings, no loops.

```
(r = 40; n = 6; 2 * pi * r / n)   // r, n are locals; the last line is the value
```

**Bindings** are written in the stylesheet with `=` — a name bound to a value, for reuse
in any expression. A **scalar** is `name = value`; a **function** adds a parameter list,
`name(params) = value`. The value is **bare** when it is a literal, a name, or a call,
and a **group** when it holds an operator, locals, or a point. `=` binds and reads
compile-time (baked), where `:` sets a live property — the two never meet:

```
{
  my_radius = 5;                          // a scalar — read bare as `my_radius`
  scale(n)  = (100 * 1.2^n);              // a function
  wave(a, f) = (u*300, a*sin(2*pi*f*u));  // a function returning a point
}
|sketch#part| { draw: move(-my_radius, 0) right(2 * my_radius) up(my_radius); }
```

Call a binding anywhere a value goes — bare like `rgb(…)` / `repeat(…)`, or inside a
group; a computed argument rides the call's own parens
(`|box| { padding: (scale(2) + 4); columns: repeat(3, 80 * 2) }`).

**Geometry.** `points:` (on `|line|` / `|poly|`) may be a **parametric expression in
`u`** — `u` sweeps `0 → 1`, sampled at `samples:` points into a vertex list, drawing
curves, waves, and spirals procedurally:

```
|line| { points: (u*300, 20*sin(2*pi*3*u)); samples: 60 }   // a sine wave
|line| { points: wave(20, 3); samples: 60 }                 // the same, named
```

Everything an expression touches **bakes** — a computed size, a sampled curve — so a
standalone SVG never depends on host CSS. The same sample-an-ambient seam feeds a
chart's `fn:` (with `x` bound to the domain — [SPEC 14](#14-charts)). Unknown names, wrong
arity, and out-of-range results are compile-time errors ([SPEC 21](#21-errors)).

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
| `tree` | rooted hierarchy | topics in generations ([SPEC 12](#12-flow-grid--tree)) | router (orthogonal / natural) | no — arranges in place |
| `sequence` | time axis | participants + messages + frames + notes ([SPEC 13](#13-sequence)) | time-rows → the `straight` strategy | yes |
| `chart` | data plane | series + axes + bands + marks ([SPEC 14](#14-charts)) | layout-time data→pixels | yes |
| `pie` | part-to-whole | slices ([SPEC 14](#14-charts)) | layout-time value→angle | yes |
| `drawing` | datum / geometry | geometry + annotations + mates ([SPEC 15](#15-drawing)) | layout-time dims / leaders | yes |
| `floorplan` | the drawing engine, architectural dialect | walls + openings + fixtures + annotations ([SPEC 15.11](#1511-floorplan--the-architectural-dialect)) | layout-time dims / leaders | yes |
| `schematic` | circuit sheet | anchors on tracks + satellites at pins ([SPEC 16](#16-schematic)) | orthogonal router, fixed ports | no — arranges in place |

**Defaults.** Every container — the root included — defaults to `layout: flow` with
`direction: row` and `gap: 36` — source order flows the way it reads, so
`cat -> dog -> bird` runs left to right (a **closed shape**'s or a `|topic|`'s children
are card content, not arranged nodes, so those stack instead — `direction: column`,
`gap: 12` — and an icon sits over its label; `|block|`, `|group|`, and the frameless
wrappers are containers and keep the flow pair); padding defaults per [SPEC 10.5](#105-layout-constants-baked),
the root's framing the whole rendered scene — links and labels included — out to the
SVG edge.

### Three seams every engine plugs into

The engines differ, but three contracts are shared — which is why a new layout is a
small, bounded addition ([Part III](#part-iii--reference) formalises each):

1. **The smart label extends.** The one label rule ([SPEC 3](#3-statements--the-label)) — each
   type places its `"X"` — is inherited by every layout (title, legend, axis title, header,
   guard; [SPEC 13](#13-sequence), [SPEC 14](#14-charts)). No layout invents a label syntax.

2. **The wiring strategy realises a scope's links.** `flow` / `grid` / `tree` — and
   `schematic`, whose wires land on fixed ports ([SPEC 16.5](#165-wires)) — hand their
   links to the router ([SPEC 9](#9-links), [ROUTING.md](ROUTING.md)); `sequence` fixes each message's
   geometry (column x, row y) and hands it to the `straight` strategy; a `drawing` — and its
   `floorplan` dialect ([SPEC 15.11](#1511-floorplan--the-architectural-dialect)) — lowers each link to a dimension, leader, or mate
   ([SPEC 15](#15-drawing)); `chart` / `pie` have no links. One scope, one strategy — set by the
   scope's `layout` (with `routing:` selecting `orthogonal`, `natural`, or `straight` for the routed
   ones), and it governs that scope's **own** links only — an ordinary `|row|` / `|grid|`
   nested inside a sequence or drawing still hands its internal links to the router
   (a schematic is the one scope whose *link laws* also reach links written in its
   nested ordinary containers — placement never cascades, [SPEC 16](#16-schematic)).
   A `sequence` message is thus the one place a link's *order* is its geometry, not a
   routing problem.

3. **A layout-owning engine lowers to primitives in the layout phase.** `flow` / `grid`
   arrange their children where they sit. `sequence` / `chart` / `pie` / `drawing`
   (`floorplan` included) instead **read their
   whole subtree** and emit an ordinary primitive tree — `|block|`s, `|line|`s, `|path|`s,
   text — at baked coordinates ([SPEC 19](#19-compile-pipeline)). So the cascade, palette, theming,
   gradients, `--static`, `fmt`, and determinism all apply to a chart, a sequence, or a drawing with
   **no engine-specific render code** — a chart *is* a diagram once lowered.

**The container is still a box.** An engine owns *where its children go*, but the
container node itself is an ordinary box: its own `fill`, `stroke`, `stroke-width`,
`radius`, `opacity`, `shadow`, `rotate`, and `href` paint in **every** layout — a chart,
a sequence, or a pie can carry a background, a frame, or a link like any `|box|`.

### Universal container properties

The container property set — which engine honours which is
[SPEC 17](#17-property-ledger--support)'s matrix, the authoritative table.

`gap` is honoured everywhere but **means what the engine needs**: inter-child spacing in
flow / grid, generation distance × sibling separation in a tree
([SPEC 12](#12-flow-grid--tree)), the plot-to-title/legend gutter in a chart / pie
(default 10), and the message pitch / participant spacing in a sequence (default 32); a
drawing places by datum and ignores it (its mates read a scoped `gap:` of
their own — [SPEC 15.5](#155-mates--seating)). `direction`, `align`, `justify`, and
`gap-fill` are the **flow / grid arranger's** knobs — a `sequence`, `chart` / `pie`, or
`drawing` container places its own children and ignores them, and a `tree` reads
`direction` and `gap` alone. `padding` is honoured wherever it has meaning
(the matrix, [SPEC 17](#17-property-ledger--support)).

**Nested boxes are unaffected.** These knobs govern a container *engine*'s placement of
its own children; an ordinary box **nested inside any layout** still lays out its own
content by the box model. So a participant box in a `sequence` — an ordinary box —
honours `padding`, `align`, `justify`, and `gap-fill` on its **own** content, even
though the sequence engine placed the participant on the time axis. (A `chart` / `pie`
consumes its children into marks, so this case does not arise there — [SPEC 14](#14-charts).)

**`gap-fill`** (default `none`) fills a flow's or grid's interior **gutters** — the gap
regions between children — with a colour, thickness = the `gap` (`gap: 1; gap-fill: --stroke`
paints hairline rules). Per-axis `gap` picks which rules show (`gap: 1 0` row rules, `0 1`
column). Gutters are **interior only** — the outer frame is the container's own `stroke`,
never doubled — and span-aware in a grid (skipping pinned and spanning cells). This is what
makes `|table|` plain `grid + gap: 1 + gap-fill: --stroke`, not a magic type ([SPEC 8](#8-templates)).

---

## 12. Flow, Grid & Tree

The **router-routed** layouts: they arrange boxes and text in place, then hand
their links to the router ([SPEC 9](#9-links)). `flow` is 1D flex, `grid` is 2D,
`tree` a rooted hierarchy.

### Flex — `align` / `justify`

`layout: flow` runs its children along one axis, set by `direction` (`row`
horizontal — the default, and `column` in card content — [SPEC 11](#11-the-layout-model)).
`justify` runs *along* the flow (main axis), `align` runs *across* it (cross axis).
Both default `center` — so the knob that flushes a box's text horizontally is
`justify` in a row box and `align` in a column one, a card included.

| Value | `justify` (main axis) | `align` (cross axis) |
|---|---|---|
| `start` / `center` / `end` | pack at the edge / centre / opposite | align each child to the edge / centre / opposite |
| `stretch` | fills children to span the main axis | each child's **box** fills the cross axis |
| `evenly` | equal gaps between and around children | (treated as `center`) |
| `origin` | (treated as `center`) | children line up **origin-to-origin** |

`stretch` fills the child's **box**, not its *content* (placed by the child's own
`align`/`justify`, also `center`). `evenly` needs multiple children.

**`align: origin` aligns what the boxes contain, not the boxes.** Every node has
an **origin** — the bbox centre of an ordinary node, a `|sketch|`'s pen origin, a
`pattern:`'s seed datum, a `|drawing|`'s datum ([SPEC 15.1](#151-the-container-the-datum--the-scale)) —
and `origin` puts every child's origin on one shared cross-axis line, which is how
a row of drawings shares one axis
([SPEC 15.8](#158-assemblies-views-sheets--titles)). **Where the line sits:**
given an explicit cross size the group fits into, it is the container's **centre
line** — a small part's axis rides the sheet's centreline; on an auto-sized (or
overfull) axis the group centres around the line instead, so a large ensemble
stays balanced. For ordinary children it *is* `center`; it differs exactly where
a box is asymmetric about its origin — a view whose dimensions stack on one
side. In a **grid**, both `align`
and `justify` accept it: the cell puts the child's origin on its track centre, so
one row of cells shares a horizontal axis and one column a vertical one — the
projection-sheet arrangement.

All of `align`/`justify`/`stretch`/`evenly` are **no-ops unless the container is
larger than its packed children** — an auto-sized container has no slack to
distribute (`origin` is the exception: it re-lines children even without slack).
Slack comes from an explicit `width`/`height`, or a grid's fixed tracks.

### Grid — `columns` / `rows` / `cell` / `span`

A grid is sized by its track lists:

| Property | Notes |
|---|---|
| `columns` | **Required.** A track list — `columns: 80, 140, 80` (3 fixed), `columns: repeat(3)` (3 auto), or a mix (`auto, 40, auto`). The list length is the column count. |
| `rows` | Optional. Same form. A floor, not a cap: extra children flow into implicit auto rows. Omitted → all rows implicit, count `⌈children / columns⌉`. |
| `cell` | A **box** child's placement `column row`, 1-indexed (`cell: 2 1`). |
| `span` | A **box** child's span `columns rows`, default `1 1` (`span: 2` = `2 1`). |

A **track** is a size (`80`), `auto` (sized to its widest/tallest child), or
`repeat(N)` / `repeat(N, size)` for many equal tracks. The count comes from the
list length. There is no `fr` unit. A fixed track is a **floor** like an
explicit `width` ([SPEC 5](#5-the-box-model)) — it grows to its widest child,
so a grid never clips.

**Auto-flow.** Children without `cell:` flow left-to-right, wrapping at the column
count; a `cell:` pins one explicitly and the rest flow around it. Bare-text cells are
pure auto-flow — `cell:` / `span:` apply to **box** children only (a text
node has no block to carry them). A grid is positional, so an empty `""` cell is
**kept** — it holds its track and keeps the cells after it aligned (in flow, an
empty `""` is dropped). `cell:` is read on a grid and on a schematic
([SPEC 16.1](#161-placement--anchors--satellites)), `span:` on a grid alone; where the
container's layout is statically known to be neither, they are an **error**
([SPEC 17](#17-property-ledger--support)'s strict rule, [SPEC 21](#21-errors): `'cell' places a
grid or schematic child — this box sits in a 'layout: flow'`).

**Per-column alignment.** On a grid, `align` (horizontal ↔) and `justify`
(vertical ↕) accept a **list parallel to `columns`** (one value per track) or a
scalar for all — so `align: start, center, end` aligns three columns in one
declaration. Mind the axes: a grid follows **column-flow, not CSS grid**, so `align`
is horizontal — the same knob that left-aligns text in a `direction: column` box.
`stretch` fills the track; `start`/`center`/`end` pack the cell's box at natural
size; the default centres.

A cell that **fills** its track (`stretch`) then honours its **own** `align`/
`justify` to place its content: an auto cell has no slack and sits centred, but a
filled one slides its text to the aligned edge — what lets a `|table|` align a
whole column ([SPEC 8](#8-templates)) with no notion of "table" in the core.
The same knob aligns a multi-line text's *lines* ([SPEC 6](#6-paint-stroke--text)).

### Tree — rooted structure

`layout: tree` arranges a rooted hierarchy. **Structure is `|topic|` nesting**
([SPEC 8](#8-templates)): a direct `|topic|`-derived child is a **branch**,
every other child the topic's own content; custom structural types derive from
it (`|person::topic|`). The scope holds **exactly one root topic** (none or two
errors; a forest is beyond 1.0 — [SPEC 24](#24-deferred)), and `|topic|` outside
a tree scope errors ([SPEC 21](#21-errors)).

| `direction:` | Growth | The look |
|---|---|---|
| `column` *(default)* | down from the root | org chart |
| `row` | rightward from the root | logic tree / outline |
| `bilateral` | both sides, horizontally | mindmap |

Placement is post-order: each subtree packs its children at the cross-axis
`gap` (sibling separation), the parent centred over its subtree's span one
main-axis `gap` (generation distance) away; subtrees never overlap.
**`bilateral`** splits the first level — the first ⌈n/2⌉ first-level topics
fill the **right** side top-to-bottom in declaration order, the rest the
**left** (each half the `row` layout, the left mirrored, the root centred
between them); a first-level **`side: left | right`** overrides its half.
`top` / `bottom` there, or any `side:` on a `row` / `column` topic, is an
error — there is no vertical bilateral; growing downward is `column`.

**Branch links are generated, and ordinary.** Desugar adds one unmarked **fan
per parent** — `ceo:bottom - ceo.cto:top & ceo.coo:top` — written in the scope
that contains the parent, with the direction's forced sides (`column`:
`bottom` → `top`; `row`: `right` → `left`; `bilateral` mirrors per half, the
root emitting both sides). `lini desugar` shows them; the scope's `routing`
draws them like any wire; and a link into one's own descendant cascades as if
written in that node ([SPEC 4](#4-selectors-cascade--specificity)), so
`#cto |-| { }` restyles exactly cto's arm. An **anonymous** topic gets a
deterministic minted id — `lini-topic-N`, 1-based among its scope's topics —
so its wires exist; an authored id is used as-is, and may not begin `lini-`
([SPEC 23](#23-reserved-words)). Authored cross-links stay legal, never alter
the tree, and keep the neutral link default. Every topic also wears a
generated **`.lini-level-N`** class (root 0), so one rule restyles a tier
(`.lini-level-2 { font-size: 12 }`).

The engine reads `direction` and `gap` alone (`gap: g s` — generation, then
sibling; a scalar sets both; a tree scope defaults `gap: 64 48`, room to route
at the default `clearance`). A plain tree is neutral — uniform topics, elbow
connectors from the default `routing: orthogonal`; the mindmap look is the
`|mindmap|` preset ([SPEC 8](#8-templates)).

---
## 13. Sequence

A **sequence** reads a diagram on a **time axis**: `layout: sequence` places named
**participants** across the top, drops a **lifeline** from each, and lays **messages** —
ordinary links — top-to-bottom **in source order**, so the order you write the wires *is*
the order they happen. It adds **no grammar**: participants are nodes, messages are links
([SPEC 9](#9-links)), frames and notes are nodes — only the engine, six type names, and two
properties (`place`, `activation`) are new, and it lowers to primitives like any layout-owning engine
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

**Nodes and links interleave in source order** ([SPEC 9](#internal-links-in-a-body)),
so a frame (a node) sits among the messages (links) around it.

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
deferred — [SPEC 24](#24-deferred).)

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
**`place:`** — a mode, then its lifeline id(s): `{ place: over api }` a box over one
lifeline, `{ place: over api db }` a box spanning those (and any between),
`{ place: left api }` / `{ place: right api }` a box beside one. **One mode per
note.** Its smart label is the text; a multi-line or styled note rides the `[ ]`
like any box. `place:` is valid only in a sequence. `par` and other fragments are
deferred ([SPEC 24](#24-deferred)).

### Defaults

The five sequence types are bundles over `|block|`, tuned to read with no styling; the
cascade overrides any of it, and they reuse the scene's role variables — no new ones.
(`|note|` is the **core** template, compacted here by its built-in scoped rule —
[SPEC 8](#8-templates).)

| Type | Defaults over `\|block\|` |
|---|---|
| `\|sequence\|` | `layout: sequence; gap: 32 32` (a root `{ layout: sequence }` gets the same `gap`) |
| `\|loop\| / \|opt\| / \|alt\|` | `fill: none; stroke: --group-stroke; stroke-style: dashed; stroke-width: 1; radius: 4; padding: 24; font-size: 12` |
| `\|else\|` | `fill: none; stroke: --group-stroke; stroke-style: dashed; stroke-width: 1; font-size: 12` |

The engine resolves in the layout phase — a message's x-ends are the lifelines' positions
(fixed once participants are placed) and its y is its row — placing participants, walking
messages/frames/notes in source order, and lowering headers → `|block|` + text, lifelines
and arrows → `|line|`, activations/frames/notes → `|block|` ([SPEC 19](#19-compile-pipeline)).
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
| `legend` | both | `top` · `right` · `bottom` · `none` ⌛ ([SPEC 24](#24-deferred)) — writing it is an **error** until the reader lands | auto (shown when ≥ 2 entries) — built |
| `tooltip` | both | `none` · `hover` · `auto` · `always` ([14.8](#148-tooltips)) | `auto` |
| `gap` | both | number — clear space between the plot and the title / legend outside it | `10` |

`categories` sets the **x (domain) axis's** tick labels — the one form today;
explicit per-axis tick text is deferred ([SPEC 24](#24-deferred)), and setting
both will be an error when it lands ([SPEC 21](#21-errors)).

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

**Singular vs. plural is the cardinality**: `|line|` / `|area|` are **one** shape;
`|bars|` / `|dots|` a **set** of marks, one per datum; a `|slice|` / `|bubble|` one
each, per node.

Inside a chart, `|line|` reads `data:` / `fn:` (data space); the standalone `|line|`
primitive ([SPEC 7](#7-nodes)) reads `points:` (pixels) — the chart layout branches on which.

**A line carries markers at every datum**, reusing the core `marker:` family generalised
from line *ends* to every vertex: `|line| { marker: circle }` shows a marker at each point.
A chart marker is **centred**, so only the symmetric kinds apply — **`dot`**, **`circle`**
(a larger, hover-sized point), and **`diamond`**; the directional `arrow` / `crow` are an
error on a series ([SPEC 21](#21-errors)). Every marker carries the datum's `<title>` — a
marked point is a hover target ([14.8](#148-tooltips)). `|dots|` is markers with no line,
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
the core comma law ([SPEC 2](#2-lexical-syntax)), so charts add **no value form**;
the **item width** is the discriminator:

| Source | Syntax | Meaning |
|---|---|---|
| categorical | `data: 9, 15, 24, 18, 30` | scalar items → one value per category |
| points | `data: 0 225, 60 225, 118 221` | `x y` items → points (numeric x; scatter) |
| formula | `fn: (min(8/(x/100-1)^2, 2000))` | an expression in `x`, sampled at `samples:` |

Scalar and pair items never mix in one `data:` — `data: 10 20` is **one point**
([SPEC 2](#2-lexical-syntax)); a legacy space list errors with the comma form.
A `|line|` / `|area|` needs ≥ 2 vertices; with categorical data the value count must match
the `categories:` count ([SPEC 21](#21-errors)).

A point's x may be a **date** — a quoted ISO-8601 literal: `data: "2026-01-01" 18,
"2026-02-01" 25`. `YYYY-MM-DD`, optionally `THH:MM[:SS]`, optionally `Z` / `±HH:MM`;
a bare date is date-only (midnight UTC), an offset keeps its instant, and rendering is
timezone-independent (all math in UTC). Date x-values make the x axis a **time axis**
([14.4](#144-axes-scales--domain)); dates and plain numbers never mix in one domain,
and an invalid date is an error ([SPEC 21](#21-errors)). Time-only literals don't exist —
a numeric axis covers them.

**`labels:`** is the **per-datum** text — a quoted-string list parallel to `data:`
(one entry per value or `x y` point), distinct from the series' one legend label
(its smart label). An entry rides with its datum: on the plot beside the point, or
on hover when there's no room — the placement is `tooltip:`'s job
([14.8](#148-tooltips)). The count must equal the data count; `labels:` needs
discrete `data:` (a sampled `fn:` has no authored points, so `labels:` with `fn:` is
an error). A per-node mark (`|bubble|`, `|slice|`, `|mark|`) takes no `labels:` —
its one smart label *is* its point label.

```
|line| "GLM-5.2" { data: 35 63, 42 72, 84 75; labels: "Non-Thinking", "High", "Max"; marker: circle }
```

**Formulas are the core expression engine** ([SPEC 10.7](#107-expressions--functions)):
operators, the math library, `name = expr;` locals, the ternary, and stylesheet functions.
Charts bind two ambient names — the same seam that injects `u` for parametric `points:`:
**`x`** the x-axis data value (a whole-domain `fn:` uses it) and **`u`** a band-local clock
`0 → 1` ([14.5](#145-bands--annotations)). A `fn:` is therefore **not folded at resolve**
(its `x` is unbound there) but held and **sampled at chart layout**, once the x-domain is
fixed — so a `fn:` value is always a `(…)` group (or a bare constant), never a bare
call: the resolver would fold a call with `x` unbound. Locals chain derivations in one group; a stylesheet function keeps twins DRY:

```
{ ramp(s) = min(100, 25 + 1.572*(x/s) + 0.0142*(x/s)^2); }
|chart| [
  |area| "Steel"    { fn: (ramp(1)) }
  |line| "Aluminum" { fn: (ramp(1/0.7)) }
]
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
| `range` | `a b` (each end a number, a quoted date, or `auto`) | the data window — and crop, and reverse (below) |
| `scale` | `linear` · `log` · `time` | `log` emits decade ticks labelled 1-2-5; its domain must be above 0. `time` reads date literals (below) |
| `step` / `ticks` | number / list · calendar (time) | tick spacing, or explicit ticks; omitted → nice ticks |
| `format` | family + args ([SPEC 17](#17-property-ledger--support)) | tick-value presentation; inherits from the chart |
| `unit` | `"%"` | a quoted suffix appended to tick labels (and tooltips) |
| `gridlines` | `none` · *colour* | this axis's gridlines: `none`, or a colour (a colour turns them on) |
| `stroke` / `color` / `font-size` | core | `stroke` tints the axis line + ticks, `color` the labels + title |

An **x (domain) axis** is categorical when `categories:` gives it labels (or by default,
indices `1…N`) and numeric when the data is points or a `fn:`. A **value axis** carries
series magnitudes; `axis: <id>` on a series binds it (default: the first value axis of the
series' orientation). Multiple value axes share a plot for dual-unit charts; only the
**primary** value axis and the x axis draw gridlines by default, so a normal grid appears
and a second value axis adds none (avoiding moiré). The default tint is the
`--lini-grid` role variable ([SPEC 10.1](#101-visual-variables-live-themeable)).

**`range: a b`** does three jobs at once: it sets the visible **window** (`a`…`b`),
**crops** data outside it to the plot area, and **reverses** the axis when `a > b`
(`range: 50 1` runs high→low — both scale and tick order flip). Either end may be `auto`
(`range: 0 auto`); the two ends must be distinct ([SPEC 21](#21-errors)). Ticks are "nice" by
default (1-2-5 × 10ⁿ); `step:` sets a spacing, `ticks:` an explicit list, `scale: log`
decade ticks (domain above 0). Tick labels come from `categories:` (an x axis) or the
formatted tick value + `unit:` (a value axis) — `format:` sets the value's presentation
([SPEC 17](#17-property-ledger--support));
`labels:` is the **series'** per-datum text ([14.3](#143-data--formulas)).

**`scale: time`** — a numeric domain in epoch seconds, set by date literals in `data:`
([14.3](#143-data--formulas)); `range:` and `ticks:` read the same literals. Ticks are
**calendar-aware**: auto picks the boundary unit from the span (years → months → weeks →
days → hours → minutes) and lands on calendar boundaries; **`step:`** takes a calendar
interval — a unit ident with an optional count (`step: month`, `step: 2 week`) — and a
plain number errors, pointing at the calendar form ([SPEC 21](#21-errors)). Tick text
follows the tick unit (years read `2026`, months `Jan 2026`, days `Mar 4`, finer
`04:30`); an explicit `format:` date preset wins ([SPEC 17](#17-property-ledger--support)).

### 14.5 Bands & annotations

Both are children placed in **data** coordinates; the model gives them for free.
`axis:` names the axis they measure against and is required on a `|mark|`.

A **`|band|`** partitions an axis and drives three things from one declaration: a
background **shade**, a **tick** (its smart label), and the **segment boundaries** every
series shares. `range: a b` is its data range on its bound `axis:` — the same
interval shape the axis itself reads ([14.4](#144-axes-scales--domain));
`fill: none` makes it a divider + label with no shading.

```
|band| "Inject" { range: 1.4 3.1; axis: time; fill: --rose }
```

**A series opts into segmentation** with a per-band `fn:` **list** — one `(…)` expression
(or a bare constant) per band, comma-separated (`fn: (u*10), 5, (ramp(2))`), evaluated
in local `u`; a **single** `fn:` samples the whole
domain in `x` and ignores bands. Consecutive segments connect end-to-start (the riser is
drawn), so a jump is explicit. A per-band list whose length ≠ the band count is an error
([SPEC 21](#21-errors)) — never a silent truncation.

A **`|mark|`** places a reference line, point, or label by *value* on a *named* axis, so it
survives a `direction` flip unchanged:

| Form | Draws |
|---|---|
| `\|mark\| "100 °C" { at: 100; axis: temp }` | a reference **line** at value 100, across the plot perpendicular to `temp` |
| `\|mark\| "60 °C — 19 min" { at: 19 60; axis: temp }` | a **point** (dot + label): `x = 19`, value `60` |
| `\|mark\| "safe" { at: 170 4; axis: temp; marker: none }` | a **label** only (no dot) |

`at: V` (one value) is a line, `at: X Y` (two) a point; `marker: none` suppresses a point's
dot, leaving the label — so there is no separate free-label node. Bands and marks render in
`column` and `row` directions; in `radial` they are a **compile error** until built
([SPEC 21](#21-errors), [SPEC 24](#24-deferred)) — never a silent drop.

### 14.6 Legend, title & colour

One smart-label rule, placed by where the label sits: on the `|chart|` / `|pie|` → the
**title** (a caption above the plot); on a series / `|slice|` → a **legend** entry with a
swatch **mirroring its paint** (fill and edge); on an `|axis|` → the **axis title**; on a
`|band|` → a **tick** tinted its `fill`; on a `|mark|` → the annotation's **label**. A
legend appears automatically at ≥ 2 entries (`legend:` is deferred —
[SPEC 24](#24-deferred), [14.1](#141-the-chart-plane)). **`gap:`**
sets the plot-to-title/legend clearance (default 10; `gap: 0` ≈ touching). The chart sets its
**chrome** — title and legend — in **semibold**, while its **data text** — axis ticks, per-datum labels,
annotation labels — stays **normal** weight, so the numbers read quietly beneath the captions.

**Colour.** Explicit `stroke:` / `fill:` wins. Otherwise series **walk the palette**
([SPEC 10.2](#102-the-colour-palette)) in declaration order, skipping `red` (reserved for
danger), repeating if exhausted — deterministic, and **interleaved** around the
hue wheel (adjacent series read as distinct, the common 2–4-series case getting
the strongest contrast):

```
--rose  --teal  --orange  --sky  --amber  --purple  --green  --blue  --lime  --gray
```

(A mindmap's branch walk is the different job — wheel order, red *and* grey
skipped — [SPEC 8](#8-templates).)

Each series takes its hue at the tier the role wants — **the outlined look**: a `|bars|` /
`|area|` / `|slice|` fills with the **`soft`** tier and gains a **`deep`** edge (`stroke:
none` removes it — a flat fill); a line takes the `deep` stroke, dots the `ink`. An
explicit `fill:` keeps its colour and still gains a deep edge of it. In `layout: pie` the
walk is **per slice** — the one place colour walks per datum rather than per series.

**Per-datum paint** rides the comma law on the repeated-mark series — `|bars|` /
`|dots|` only: `fill:` / `stroke:` / `opacity:` take a comma list, one item per datum,
where **`auto`** is the paint that datum would get anyway (the walk, the deep-edge rule) —
`fill: auto, auto, --red, auto` highlights one bar, and with no authored stroke each
datum's default deep edge deepens its **own** fill. The count must equal the count of
**explicit `data:`** (a sampled `fn:` has no authored data — [SPEC 21](#21-errors));
the legend swatch keeps the series' base paint. A list on
`|line|` / `|area|` is an error — one shape has one paint, no ambiguous interpolation;
`|slice|` / `|bubble|` / `|mark|` are already per-node.

### 14.7 Direction, radial & pie

`direction` orients the chart — the same property a `flow` uses to pick its axis, plus
`radial`: `column` (default, cartesian, bars grow up), `row` (cartesian, bars grow right),
`radial` (polar, bars grow outward). **The flip is never silently lossy** — nothing is
authored in screen coordinates (`categories:`, series `data:`, and annotations bound to
a *named* axis with `at:` / `span:` are all logical), so `direction` only changes how
that plane is projected, and what a direction cannot yet draw **errors** instead of
vanishing (a radial band / mark — [SPEC 21](#21-errors)). An explicit axis `side:` is a
screen edge and is honoured as written.

**Radial** (`direction: radial`) projects the cartesian model into polar coordinates: the
x (domain) axis bends into a ring (categories → evenly-spaced **spokes**, from the top,
clockwise) and the value axis becomes the **radius**. A radar `|line|` connects a series'
value on every spoke and **closes** to the first; an `|area|` fills that polygon; `|bars|`
fill their angular slot. A radial chart has **one value (radius) axis** — writing `side:`
on it is an error ([SPEC 21](#21-errors)) — and one x axis (the spokes). Concentric circular
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
total, and exploded slices are deferred ([SPEC 24](#24-deferred)).

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
`labels:` entry, or a per-node mark's smart label — while *hover* shows its **value**. So a
point can read `Max` on the plot and `GLM-5.2: 75%` on hover, never competing.

**The hover floor is always honest.** A labelled mark carries a native `<title>` — its
accessible name, readable in any renderer and surviving `--static`. Over it, a live CSS
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

`layout: chart` / `pie` resolve in the layout phase ([SPEC 19](#19-compile-pipeline)), since the
shared scale needs every child's data first: **collect** series and resolve `data:` /
sample `fn:`; fix each axis **domain** and scale (bars force zero); inset the **plot rect**
by measured label / legend gutters; **lower** every series, axis, band, annotation, and the
legend to primitives at baked pixels; **emit** in a **semantic draw order** — bands →
gridlines → areas → bars → lines → dots → annotations → axes → labels → inline labels →
tooltip — so a line sits above its bars without hand-ordering (the one place a chart
overrides source-order rendering; `layer:` still wins). The output is an ordinary primitive
subtree ([SPEC 19](#19-compile-pipeline)).

---

## 15. Drawing

A **drawing** reads a diagram as a dimensioned sheet: `layout: drawing` places every
geometry child on one shared **datum**, and its links are **annotations** — dimensions,
callouts, leaders — or **mates** that seat parts against each other. One bet carries the
design: because the engine *has* the geometry in numbers, a dimension's smart label is
its **measured value** — the numbers live once, in the geometry, and the annotations
point at them. Drawings are the one layout that substantially extends the grammar —
in the seven ways [SPEC 22](#22-grammar) enumerates; everything else is nodes,
declarations, and links, and it lowers to primitives like any layout-owning engine
([SPEC 11](#11-the-layout-model), seam 3). A drawing needs at least one
geometry child ([SPEC 21](#21-errors)); its children split by role:

| Child | Is | Drawn |
|---|---|---|
| a box (`\|sketch\|`, `\|rect\|`, `\|oval\|`, `\|hole\|`, …) | **geometry** — a part or a feature | its outline and fill, at the shared datum |
| a link with a measuring op — `(-)` `(o)` `(<)` — or a leader op — `<-` `*-` `>-` | an **annotation** | extension lines, arrows, text ([15.6](#156-dimensions), [15.7](#157-leaders-notes--line-conventions)) |
| a link with `\|\|` | a **mate** — or, with a sheet-content end, a **seat** | nothing — it positions a part or an annotation ([15.5](#155-mates--seating)) |
| any other two-ended link (`->`, `<->`, `-->`, `-*`, …) | a straight **annotation arrow** | one segment, markers per the op |
| `"…"`, `\|note\|`, `\|balloon\|`, `\|table\|`, `\|surface-finish\|`, `\|feature-control\|`, `\|datum\|`, … | sheet content | per its own type, sheet-space ([15.1](#151-the-container-the-datum--the-scale), [15.9](#159-drafting-symbols--annotation-composition)) |

Four properties of the model, each inherited from the core:

- **A drawing scope owns its links** — the wiring strategy ([SPEC 11](#11-the-layout-model)):
  the router never sees them; every one lowers at layout time to dimension or leader
  primitives, or (for `||`) to a position. `routing:` and `along:` have no role on
  them; `clearance:` reads as a dimension's stand-off minimum ([15.6](#156-dimensions)).
- **No auto-create.** Unlike a diagram (`cat -> dog` invents boxes), a drawing never
  invents an endpoint: an annotation must point at real geometry. An unknown endpoint
  is an error with suggestions ([SPEC 21](#21-errors)).
- **One placement model, whole scope.** Every geometry child — and a part's own `[ ]`
  features, recursively — places its **origin on the parent's datum**, not by flow. A
  child that owns a layout (a `\|table\|`, a nested `\|drawing\|`, a `\|row\|`…) lays
  out its interior as usual and places as one box.
- **What you measure is a node — or a point or edge it names.** Anything dimensioned,
  mated, or pointed at is a node with an id, or a `:segment` a `\|sketch\|` authors on
  its own profile ([15.2](#152-anchors), [15.3](#153-the-sketch-pen)). Anonymous geometry is
  drawable but not addressable.

The geometry machinery is ordinary Lini, usable in any layout; only the annotation
semantics need a drawing scope:

| Global — works everywhere | Drawing-scope only |
|---|---|
| `\|sketch\|` + `draw:` / `mirror:` / `revolve:` / `thread:` / `break:`; `pattern:`; `scale:`; `hatch()` fills; `stroke-style: center` / `phantom`; `\|note\|` / `\|balloon\|` / `\|hidden\|`; the `\|page\|` sheet | the measuring ops (`(-)` linear, `(o)` round, `(<)` angle), the leader ops, `\|\|`, `tol:`, dim `side:` / `project:`, auto-measure, `unit:`, datum placement, the drafting-symbol types (`\|surface-finish\|` / `\|feature-control\|` / `\|control\|` / `\|datum\|`, [15.9](#159-drafting-symbols--annotation-composition)), the chrome (centre marks, auto centerlines, dimension packing) |

Outside a drawing a `\|sketch\|` is just a shape; its authored `:segment`s are declared
but dormant (a routed link landing on one is deferred — [SPEC 24](#24-deferred)).
A **floorplan** is this same engine under an architectural vocabulary —
[15.11](#1511-floorplan--the-architectural-dialect), the one subsection that is a
dialect rather than a mechanism.

### 15.1 The container, the datum & the scale

`|drawing|` is `|block|` + `layout: drawing` — frameless, padding 0 (the geometry and
its annotations *are* the content). `{ layout: drawing }` on the root makes the whole
file one drawing, exactly as a root sequence works; the root's padding then frames the
sheet.

**Datum & ground.** The datum is the container's own origin. Every child's **origin**
lands on it — *not* its bbox centre: a symmetric primitive's origin is its centre, so
primitives stack concentric by default; a `|sketch|`'s origin is its **pen origin**
([15.3](#153-the-sketch-pen)), so two sketches drawn at different pen offsets keep
their drawn relationship. `translate: x y` offsets a child from the datum — the
universal nudge, unchanged. Children paint in **source order** (later on top), so
overlaps, punched holes, and cutaways compose with no boolean operations. The
**ground** is the first-declared geometry child: mates resolve by walking outward from
it ([15.5](#155-mates--seating)); to reground, reorder the declarations.

**Scale — three settings, one derived number.** Numbers in a drawing are **drawing
units**; three settings turn them into pixels and paper:

- **`scale:`** — the drafting **ratio**, per view. Default **1**; `scale: 2` reads
  2 : 1, `scale: 0.5` reads 1 : 2, and the composed section / detail / view titles
  read it directly ([15.8](#158-assemblies-views-sheets--titles)). Magnitude is
  `scale:`'s job — a 5 m beam on an A4 is `scale: 0.02` (1 : 50), never a
  resolution fudge.
- **`unit:`** — the **physical size of one drawing unit**: `mm` (default), `cm`,
  `m`, or `in`. Inherits nearest-wins (state it once, on the page); semantic only
  in drawing scopes — a `|sketch|` in a flow diagram stays pixel-space
  (`right(300)` is 300 px). Displaying a unit suffix on measured values is
  presentation — `format:`'s territory ([15.6](#156-dimensions)).
- **density** — pixels per millimetre: `density: N` on the **root** only, default
  **4**. Non-semantic — it sets screen/raster resolution and nothing else: print
  stays true-scale regardless ([SPEC 18](#18-svg-output)), and no measured value,
  mate, or title reads it.

The engine's pixels-per-unit is always **derived** — `ratio × unit-in-mm × density`
— never authored; desugar folds the three into that one number, so `lini desugar`
shows it ([SPEC 19](#19-compile-pipeline)). Draw a 300 mm bar as `right(300)` at
the defaults and it renders 1200 px wide while every dimension still reads `300` —
**measured values are always pre-scale**; an absurd rendered extent draws a hint
naming the likely `scale:` fix ([SPEC 21](#21-errors)).

`scale:` is an ordinary node property, nearest ancestor wins: on the drawing it is
the view's ratio (a 2 : 1 detail is a sibling drawing at `scale: 2`,
[15.8](#158-assemblies-views-sheets--titles)); on any node it overrides — `scale: 1` opts a
node out. One split makes it behave: a node's **position** (`translate:`) scales by its
*parent's* scale, its **own shape** (`draw:`, `points:`, `width` / `height`,
`pattern:` offsets) by its *own* — so a balloon in a 2:1 view stays beside its part at
true size. What never scales, at any setting: text (`font-size` is compile-measured,
per core), `stroke-width`, markers, hatch pitch, every dimension / leader constant
([SPEC 10.5](#105-layout-constants-baked)), and a **pinned** overlay's `translate:` —
a pin-relative nudge is chrome anatomy (a badge's offset, the title's gap), not a
position in the drawing. The `|note|` / `|balloon|` / `|table|` / `|surface-finish|` /
`|feature-control|` / `|datum|`
templates carry `scale: 1` ([SPEC 8](#8-templates)) — annotations are sheet chrome — and
a define inherits its base's side (`|steel::sketch|` scales, `|finish::note|` doesn't).

**Sizing & measurement.** A drawing's bbox is the union of its children's **paint**
bboxes *and* its annotations (dimensions stack outside the geometry and count), plus
`padding`; an explicit `width` / `height` is a floor, per core. Measurement, by
contrast, uses each node's **geometry bbox** — the drawn path, stroke excluded — so
line weight never leaks into a value or a mate. Geometry defaults to
`stroke-width: 2` and a drawing's links to `1`, their text to `font-size: 12` (the
caption size) — drawing-scope link defaults (like the scope's `clearance` /
`routing`), below every user rule, so a plain `|-| { stroke-width: … }` restyles
them — the drafting 2 : 1 line-weight contrast. Pen geometry, holes, shoulder
lines, and the dimension/leader linework paint the full drafting tone
(`--stroke-dark` — black on white); support lines the translucent
`--stroke-light` ([SPEC 10.1](#101-visual-variables-live-themeable)). `gap`, `align`, `justify`, and `direction` have no
role on a drawing container and are ignored.

### 15.2 Anchors

The endpoint form is the core one ([SPEC 9](#9-links)) with a wider point set, valid
only in a drawing scope:

```
anchor = id { "." id } [ "." index ] [ ":" point ]
index  = a 1-based pattern-copy number                      (15.4)
point  = center                                            (the default)
       | top | bottom | left | right                       (side midpoints)
       | top-left | top-right | bottom-left | bottom-right  (corners)
       | segment                                            (authored in draw:, 15.3)
```

- Points sit on the node's **geometry bbox** ([15.1](#151-the-container-the-datum--the-scale)):
  a side is that side's midpoint, a corner the bbox corner, `center` its centre.
  Corners glue **vertical word first**, matching `pin`'s vocabulary (`pin: top left` →
  `:top-left`); the reversed order errors with a did-you-mean. Corners and `:center`
  are drawing-scope only — elsewhere the core four sides stand, with one exception:
  a sheet's **projection link** ([15.8](#158-assemblies-views-sheets--titles)).
- A `|sketch|` **authors** its own **segments** with the point sigil in `draw:`
  ([15.3](#153-the-sketch-pen)) — declared in the pen, selected on an endpoint, the
  same declare / select symmetry as `#id`. Built-in names win (`:left` cannot be
  authored); an unknown segment errors with suggestions; `mirror:` copies of a
  segment are not addressable ([SPEC 24](#24-deferred)) — a `pattern:` copy is
  addressed by its index ([15.4](#154-features-holes--patterns)).
- For **measurement** every anchor reduces to a representative point — a point is
  itself, an edge or arc its midpoint, a bbox name its bbox point — and a named edge
  additionally carries its **direction**, which sets a dimension's axis and feeds the
  angular op ([15.6](#156-dimensions)).
- Dot-paths walk into children as everywhere (`pump.body:right`), resolve in the
  statement's scope, and never search ([SPEC 9](#9-links)). A patterned node's position
  is its **seed** copy (grid) or ring **centre** (radial), its other anchors read one
  copy's geometry about that datum, and a numeric segment picks a copy outright —
  `plate.bolt.2` ([15.4](#154-features-holes--patterns)).
- **The anchor aims; the outline lands.** A leader's tip is a ray from its text toward
  the anchor's representative point, stopped at the ray's *first crossing of the drawn
  path* — aiming at the bbox corner of a filleted plate touches the fillet arc itself.
  Dimension extension lines, by contrast, spring exactly from the anchor
  points — except an **edge** anchor's, which springs from the edge's **end
  nearest the dimension line** (the drafting convention: the witness line
  leaves the corner, never travels the face — so it also never crosses a
  wall opening, [15.11](#1511-floorplan--the-architectural-dialect)).
  Measurement is untouched — the representative point stands.

### 15.3 The sketch pen

`|sketch|` is a closed primitive ([SPEC 7](#7-nodes)): a pen that folds to a path. It
**requires `draw:`** (as `|poly|` requires `points:`), paints like any closed primitive
(defaults `--fill` / `--stroke` / `stroke-width: 2`), and derives its bbox from the
geometry.

`draw:` is a left-to-right list of **bare calls** — ordinary value-position calls, no
new value grammar beyond the `:segment` suffix; the value runs to its `;` and may span
lines. An argument is an expression — a number, a bound value, a call, or math inside the
call's own parens (`right(w / 2)`, `up(5 * r)`, [SPEC 10.7](#107-expressions--functions)).

| Call | Does |
|---|---|
| `move(x, y)` | set the start / begin a new subpath — **absolute**, in the sketch's own frame |
| `left(n)` / `right(n)` / `up(n)` / `down(n)` | an orthogonal run; the verb is visual (`up` goes up on screen) |
| `line(dx, dy)` | a relative straight segment |
| `angle(deg, n)` | a run of length `n` at a bearing — **0 = up, clockwise** (90 right, 180 down, 270 left) |
| `arc(dx, dy, r)` | the **minor** arc to a relative point; `r > 0` sweeps clockwise, `r < 0` counter-clockwise; `\|r\|` ≥ half the chord or it errors |
| `arc(r, deg)` | a **tangent** arc: continue the current heading, sweeping `deg` on radius `r > 0` — `deg > 0` turns clockwise; the heading updates by `deg` |
| `curve(dx1, dy1, dx2, dy2, dx, dy)` | a relative cubic bézier |
| `fillet(r)` / `chamfer(c)` | **corner modifiers** between two segments — a line or an **arc** on either side — trim both legs (`chamfer` cuts `c` back along each, **by arclength** on a curved leg; on a square corner, the 45° bevel) and join with a tangent arc / a straight bevel. They draw nothing alone and error anywhere but at a corner. |
| `circle(r)` | a circle subpath centred on the current point; the point and heading are unchanged |
| `point()` | record the pen's **current point** under its attached `:segment` — a station; draws nothing, changes nothing |
| `close()` | close the current subpath. **A closed path is cyclic**: a modifier may sit on either side of `close()` — `fillet(3) close()` rounds the corner where the last segment meets the seam, `close() fillet(3)` the one where the seam meets the first segment. |

**Coordinates.** The pen's frame keeps the core orientation — y grows **down**, like
`points:` and `translate:` everywhere in Lini — but the verbs and bearings are visual,
so a profile written with `up` / `right` / `arc` never types a signed y; only
`move()`, `line()`, and `curve()` expose raw coordinates. Heading state: each drawing
call leaves the pen heading along its own direction; `angle()` and the tangent `arc()`
read and update it.

**Subpaths & holes.** A second `move()` starts a new subpath; fill is **even-odd**, so
an inner subpath reads as a hole — an outline with a bore is one shape, composite
parts are overlapping nodes, and no boolean operations exist or are needed. An open
path (no `close()`, no `mirror:`) is legal; `fill` paints it as if closed (SVG
semantics).

#### `:segment` — the point sigil in the pen

Anything the pen draws can carry a **segment name**, written with the point sigil
([15.2](#152-anchors)) **glued to its call** — one rule, two readings:

| On | Names | Example |
|---|---|---|
| a drawing call | that call's drawn segment: an edge, an arc, a bevel, a circle, a `close()` seam | `right(50):neck`, `fillet(3):r1` |
| `point()` | the pen's **current point** | `right(38):thread point():m1 right(32)` — a station with no drawn edge |

The names are **yours**, not vocabulary. A `:segment` **always glues to a call** — a
floating `:name` is an error. `point()` draws nothing and changes nothing; beside a
`fillet` / `chamfer` (either order) it records the **theoretical sharp corner** — the
point drafting measures (the arc itself is named on the modifier). `move()` takes no
segment — name its landing with `point()` (`move(-90, 0) point():origin`). A
duplicate segment in one `draw:` is an error.

#### `mirror:` — draw half, get the whole

`mirror:` reflects everything the node holds — the path the **pen** drew **and its
features** — and unions the copy. The value is a **list**, applied left to right,
each item reflecting the union so far — two items give a 4-fold part:

| Item | Axis (through the node's origin) | Gives |
|---|---|---|
| `x-axis` | the horizontal axis (y = 0) | top ↔ bottom symmetry |
| `y-axis` | the vertical axis (x = 0) | left ↔ right symmetry |
| a number `45` | the line at that bearing (`angle()`'s convention) | angled symmetry |

What mirroring does is decided **per subpath**, and both intents fall out of one rule
each: an **open** subpath is **fused** — the copy joins end-to-end, the edge on the
axis the invisible seam (*draw the half, get the whole*); a **closed** subpath is
**duplicated** — a reflected second copy (*draw one ear, get both*). So leave a
half-profile open (a `close()` there would draw a visible spine down the axis — the
cue you meant the other form), and close a shape you want twice. A fused mirror also
generates its axis `|centerline|` — auto chrome,
[15.7](#157-leaders-notes--line-conventions); a duplicated subpath generates none.
`mirror:` runs before `pattern:` and before placement: it builds the node's geometry,
so anchors, dimensions, and mates all see the whole part.

A **feature** takes the same split, read on its **position**: one **on** the axis
reflects onto itself and is drawn once; one **off** it becomes a reflected second
copy — a carrier addressed and counted exactly like `pattern:`'s
([15.4](#154-features-holes--patterns)). A reflected copy is one whose
**coordinates** are reflected, never a node wearing a flip: its labels read forward,
its anchors stay handedness-free, and a silhouette the renderer draws from a box (a
`|slant|`'s lean, a `|cyl|`'s rim) rides upright with them. A node declines with
**`mirror: none`**, and its subtree with it: `none` means no reflection touches it,
its own axis and its ancestors' alike. The `auto` default reflects iff an ancestor
does. Only the pen folds a path, so on any other primitive `mirror:` reflects the
features and leaves the node's own shape; `|path|` and `|image|` read `none` outright
— a raw `d` and a raster have no reflection to take — and naming an axis on either
errors ([SPEC 21](#21-errors)).

#### `revolve:` — a turned part

`revolve: x-axis` (or `y-axis`) declares the profile a **solid of revolution** about
that axis through the pen origin. It folds exactly as a fused `mirror:` on the same
axis — draw the half, get the whole, plus the axis `|centerline|` — and adds the
**edge lines** a lathe part's side view draws: at every profile vertex where two
segments meet with a **tangent break**, off the axis, a generated `|shoulder|` line
(geometry weight — real visible edges, [SPEC 8](#8-templates)) runs perpendicular to
the axis to the vertex's reflected twin; a span the profile already draws whole is
skipped, and vertices sharing a station draw once, at the widest span. So a
`fillet()` joins tangent-continuously and generates **nothing**, a `chamfer()` keeps
two sharp vertices and generates its two lines, a step completes itself — drafting's
rule falls out of the geometry, with no per-call cases. Edge lines live in the
sketch's frame, so they ride `break:` like features. A sketch takes `revolve:` **or**
`mirror:`, never both; `revolve:` folds the **profile alone**, a turned part's
features being drilled, not turned. The unary `⌀` readings require a revolved profile
([15.6](#156-dimensions)).

#### `break:` — cut the boring middle

`break: a b;` removes the span between two stations from the **view** — the model
stays whole. `a < b` (error otherwise) are coordinates in the node's own frame on the
**break axis**: the node's **longer axis** by default, or named per group —
`break: -40 40 y-axis;` reads *the stations sit on the y-axis*. Several breaks are a
comma list, each group defaulting to the longer axis: `break: -90 -30, 30 90;`.

- The far piece slides toward the near one, leaving a sheet-space `break-gap`; the cut
  edges draw as generated `|breakline|` children — the standards' thin line with a
  sharp jog mid-span — styled or removed by the cascade like all chrome
  ([15.7](#157-leaders-notes--line-conventions)).
- **The break is a black hole for position.** Everything placed in the broken node's
  frame rides the compression — its features, their sub-features, a `pattern:`'s
  copies: a far-side hole slides with the far piece. (A descendant's own *shape*
  never clips — only the profile cuts.)
- **Dimensions stay true.** Anchors and extension lines land at *displayed* positions;
  measured values always read the *unbroken* model — the same law as `scale:`.

#### `thread:` — dress a threaded surface

`thread: seg pitch;` marks an authored segment as an ISO 6410 thread — comma groups
for several (`thread: left 1.5, right 1.5;`, a double-end stud). The segment name
reads **bare** — a value has no id to separate it from, the same way a chart band's
`axis: t` names its axis ([SPEC 14.5](#145-bands--annotations)) — and must name a
straight run parallel to the `revolve:` axis, on a revolved profile. The pitch is in
drawing units, and the numbers live once — the surface gives the major `⌀`,
`thread:` the pitch, and the chrome follows:

- the **thin line** — `--stroke-light` — offset **into the material**, running the
  segment and stopping at an adjoining `chamfer()`'s trim point. The subpath sets
  the sense: on an outer profile the run is the major and the line marks the
  **minor**, in by the ISO 60° depth, **0.6134 × pitch**; on an **inner** (even-odd
  hole) subpath the thread is internal — the run is the drilled minor and the line
  marks the **major**, out by **0.5413 × pitch** (the round view's numbers,
  [15.4](#154-features-holes--patterns));
- the **thread-end line** — geometry weight, across the full diameter — at an end
  where the surface **continues collinearly** past the run (a thread stopping
  mid-surface); where the profile turns instead — a chamfer, a face, a step — the
  geometry already ends the thread and no line is drawn;
- both doubled about the axis by the revolve.

A **bare leader** on a threaded segment composes its spec — `bar:m20 <-` reads
**`M20×1.5`** (major ⌀ × pitch, the metric form; an internal run composes from its
major the same way) — re-cut the bar and the callout follows. An authored text
**follows** the composed spec, per the one-ended label law
([15.6](#156-dimensions)): `bar:m20 <- "LH"` reads `M20×1.5 LH`.
On a round node — a threaded hole's top view, a stud's end view — `thread:` takes
the pitch alone ([15.4](#154-features-holes--patterns)).

### 15.4 Features, holes & patterns

**A part's features ride in its `[ ]`** — placed at the part's datum and **rigid**
with it: mate or translate the part and its holes travel along.

```
|rect#plate| { width: 120; height: 70 } [
  |hole#pin| { width: 10; translate: -35 20; pattern: grid(2, 1, 70, 0) }
]
plate:left (-) plate.pin { side: top }        // dot-path to the feature → 25
```

**`|hole|`** ([SPEC 8](#8-templates)) is round: `width:` — **required** — is its
**diameter**. It **punches** by paint order (`fill: --bg` over a filled or hatched
part reads as a through-hole, hatch-exempt with no special case) and draws its own
dash-dot **centre marks**, overhanging by a sheet-space constant — a hole without
marks is a plain `|oval|`. `pin (o)` reads its diameter ([15.6](#156-dimensions));
`pattern:` prefixes the count (`2× ⌀10`).

**`thread: pitch`** dresses a round feature's view with the ISO 6410 **¾ arc** — a
thin (`--stroke-light`) circle broken over its upper-right quadrant. The **type
carries the sense**: on a `|hole|` the thread is internal — the drawn circle stays
the drilled bore and the arc sits *outside* it at the major ⌀ (`width +
1.0825 × pitch`, the ISO internal thread height); on plain round geometry (`|oval|`
lineage) it is external — the outline is the major and the arc sits *inside* at the
minor (`width − 1.2269 × pitch`). Centre marks are unchanged and `pin (o)` still
reads the drawn width. Counterbores and countersinks stay deferred
([SPEC 24](#24-deferred)).

**`pattern:`** replicates a node about its own position — a node property, legal in
any layout, though its chrome belongs to drawings:

| Form | Copies |
|---|---|
| `pattern: grid(cols, rows, dx, dy)` | `cols × rows` copies at offsets `(i·dx, j·dy)`; the **seed is copy one** and keeps the node's position |
| `pattern: radial(count, radius)` | `count` copies **on** the circle, first at bearing 0, clockwise; the node's position is the **ring centre** and no copy is drawn there |

The two datums match drafting practice — you locate a grid by its first hole and a
bolt circle by its centre. The node's bbox becomes the **union** of the copies; each
copy repeats the full lowering (a patterned `|hole|` punches and centre-marks per
copy); a radial pattern generates its `|pitch-circle|`
([15.7](#157-leaders-notes--line-conventions)). Counts ≥ 1 (grid) / ≥ 2 (radial),
`radius > 0`; offsets are drawing units.

**Copies are addressable** by a numeric path segment — `plate.bolt.2`: 1-based,
grid copies **row-major from the seed**, radial copies **clockwise from bearing 0**, a
`mirror:`'s reflections after their originals, item by item.
The index extends the carrier's dot-path only — copies leak no ids (`bolt.2` alone
is an unknown endpoint); an index past the count errors with it
([SPEC 21](#21-errors)). A copy is the feature at its own position: every anchor —
bbox points and authored `:segment`s — reads that copy's geometry; a dimension on it
measures the **true model position** (displayed anchors still ride `break:`'s
compression — [15.3](#153-the-sketch-pen)); a leader lands on the displayed copy.
The bare carrier keeps its seed / ring-centre reading and its `N×` count prefix
([15.2](#152-anchors), [15.6](#156-dimensions)).

**Composition is the geometry model** — there is no CSG. A part is one `|sketch|`,
its surfaces and corners named where dimensions will land, or **composed** from
overlapping nodes in paint order: a bore in a section view is a `--bg`-filled
`|rect|` — it punches the hatch and its edges anchor a `(o)`. The escape hatches are
core (`|poly|`, `|path|`, `|image|`). A **parts library** is plain defines — no engine
support, just bundled geometry and paint:

```
{
  |steel::sketch| { fill: hatch(45, 6) }
  |brass::sketch| { fill: hatch(-45, 4) }
}
```

### 15.5 Mates & seating

`a:anchor || b:anchor` seats one node against another — `||`, the parallel bars of
GD&T: it moves a part — or, with a sheet-content end, an **annotation** (seating,
below) — and **draws nothing**, so it can never be confused with an annotation line.
Grammatically one more link op ([SPEC 22](#22-grammar)); chains and fans parse as
usual; a mate takes **no label** and no markers.

```
nozzle:left || barrel:right              // abut those faces, flush
cap || barrel                            // no anchors — concentric (origins coincide)
nozzle:left || barrel:right { gap: 4 }   // 4 units of daylight along the normal
piston:left || bore:left { gap: -6 }     // negative gap — inserted 6 deep
```

- **Resolution.** Mates resolve after datum placement, walking outward from the
  **ground** (the first-declared child, [15.1](#151-the-container-the-datum--the-scale)):
  each mate moves the side *not yet connected* to the ground, translating that whole
  scope-level child, rigid, features and all. `a || b` and `b || a` are the same
  mate — grounding, not operator order, decides who moves. A mate whose ends are both
  already grounded is over-constrained — an error naming the cycle; an unconnected
  island grounds its own first-declared node. Deterministic, source-ordered.
- **Directed vs point anchors.** Sides and named edges are **directed**: a mate
  between them aligns the faces flush along the shared normal (the other axis stays
  where the datum put it — `translate:` slides it), the two directions must be
  parallel (`a:left || b:top` errors), and a named edge seats a part against an
  **interior** face (`ring:right || housing:shoulder`). A named edge faces the
  **left of the pen's travel** — draw the profile with the material on the pen's
  right (axis → up → across → down, the natural half) and every face points out,
  interior shoulders included. `gap:` offsets along the normal and may be
  **negative** (overlap — the one place `gap` goes below zero). **Point** anchors
  (`center`, a freestanding name) make the points **coincide** — the bare `a || b`
  is the origin-to-origin case — and have no normal, so `gap:` there errors.
- **Rotate, then mate; translate after.** A part's `rotate:` turns its geometry first
  and the mate aligns the *rotated* anchor; the mated child's own `translate:` applies
  **after** — the universal post-placement nudge, here a lateral slide along the face.
  A `pin:` on a mated child is ignored with a warning.
- A mate between two features of **one** part errors — a part is rigid. Mates are
  valid only where children datum-place: inside a layout-owning child the flow already
  decided every position, the same over-constraint error. Dot-paths reach into parts
  (`pump.shaft:right || frame:left`), moving the scope-level child that contains the
  moving anchor.

**`||` with a sheet-content end is a seat.** The operator generalizes to annotation
**seating** — same syntax, split by what the ends are:

| Ends | Reads | Who moves |
|---|---|---|
| geometry `\|\|` geometry | a **mate** | the grounding walk above |
| annotation `\|\|` geometry | a **seat** | the annotation, always — either operand order |
| annotation `\|\|` annotation | error | seat annotations on geometry ([SPEC 21](#21-errors)) |

- **Seats run after mates**, outside the grounding graph: every part is already
  seated when annotations place, and a seat never grounds, moves geometry, or
  over-constrains anything. One seat per annotation — a second errors.
- **The target supplies the face**: the geometry anchor must be **directed** — a
  side or a named edge; a point target errors ([SPEC 21](#21-errors)).
- **A seat places; a mate aligns.** The annotation's **seat anchor** — its own
  endpoint anchor, or the type's default (the table) — lands **on** the target
  anchor's representative point, both axes: flush contact (the annotation had no
  position of its own worth keeping). `gap:` offsets along the target's outward
  normal, positive = daylight (the mate's signed law); `rotate:` turns the
  annotation **before** the seat — the rotated anchor aligns, so `rotate: -90`
  stands a symbol on a vertical face; `translate:` nudges **after**, the lateral
  slide along the face.
- **Bundles seat as one.** A wrapper (a `|column|` of finish symbol over frame) is
  sheet content like its children: it seats whole — interior laid out as usual —
  and reports **one painted extent** to the dimension packer
  ([15.6](#156-dimensions)), so rows stand off the bundle, never thread it.

| Annotation | Default seat anchor |
|---|---|
| `\|surface-finish\|` | the symbol's **tip** — the vee stands on the face ([15.9](#159-drafting-symbols--annotation-composition)) |
| everything else — `\|feature-control\|`, `\|datum\|`, `\|note\|`, `\|balloon\|`, a bundle | the **facing side** — the bbox side whose outward opposes the target's normal, read after `rotate:` |

### 15.6 Dimensions

A dimension is a **link**; the operator carries the kind and supplies the glyph you
can't type. The statement is the core link statement, with one relaxation: the
measuring and leader ops may stand **one-ended** ([SPEC 22](#22-grammar)).

| Write | Reads | Renders |
|---|---|---|
| `a:left (-) b:right` | a linear span | extension lines, arrows, `25` |
| `a:left (-) b (-) c` | a **chain** | each hop its own dim, one shared row |
| `pin (o)` | a round feature | the **⌀ line across the circle** — both arrows on the rims — `2× ⌀10` |
| `hole:top (o)` | a round feature, side-anchored | the **diametral line** through the circle |
| `bore:top (o)` | any node, side-anchored | the span to the opposite side, ⌀-read — `⌀16` |
| `body:neck (o)` | a **revolved**-profile segment | the station's span across the axis — `⌀28` |
| `body:r1 (o)` | a named arc | a leader — `R3` |
| `body:flank (<) body:base` | two line-like anchors | the angle arc — `40°` |
| `body:taper (<)` | a mirrored- / revolved-profile segment | the **included** angle vs its own twin |

Each glyph is a **picture of what it measures**: the dash `(-)` is a length, the
circle `(o)` a diameter, the wedge `(<)` an angle. **Arity disambiguates** — `(-)` is
always binary, `(o)` always unary / side-anchored, `(<)` either.

**`(-)` — the linear measure.** The dash pictures a length: `(-)` spans two anchors
and reads the distance between them, projected on its axis. It is **always binary** —
`a (-) b`, or a chain `a (-) b (-) c` sharing one row; a unary `a (-)` errors ("a
linear dimension measures two anchors", [SPEC 21](#21-errors)). Extension lines spring
from the anchors and the value rides the line (**Placement & stacking**, below).

**`(o)` — the round measure.** The circle pictures a diameter: `(o)` is **unary /
side-anchored** — `hole (o)`, `bore:top (o)`; a binary `a (o) b` errors ("`(o)`
measures one round feature", [SPEC 21](#21-errors)). The **feature picks the symbol**,
per the standards: a named **arc** (a `fillet`, an `arc()` product) reads its radius —
`R` — and **everything else** reads as a diameter, `⌀`, across whatever span its anchor
gives. Roundness is by construction (`|hole|` / `|oval|` lineage, a `circle()` product,
`|pitch-circle|`, a **revolved** profile), never guessed from coordinates. A bare `(o)`
needs an inferable axis — a round node (symmetric, any) or a revolved sketch (across
its axis, the full span); otherwise the error asks for an anchor. The `⌀` station and
full-span readings require `revolve:` — a merely mirrored profile's span is a width,
not a diameter, and errors asking for the revolve ([SPEC 21](#21-errors)). `R` on a full circle has no auto form
(the standards say ⌀) — type a leader (`pin <- "SR5"`), the universal fallback for
anything auto-measure can't read.

**The diametral line.** On a **round** node, a side anchor draws the dimension
*through* the circle, arrows out against the rims: `:top` / `:bottom` vertical,
`:left` / `:right` horizontal, a corner the 45° diagonal. The value sits on the line
when it fits inside; otherwise the line overruns the **anchored** rim and carries the
text there — `hole:top (o)` spills upward, packing along that ray
(**Placement & stacking**, below). Deterministic, no solver.

**`(<)` — the angle.** Binary, between two **line-like** anchors — a named edge, a
`|line|` / `|centerline|`, a bbox side: the angle between their directions, the arc
drawn at their (extended) intersection, the value riding the arc. Unary, on a named
edge of a mirrored or revolved sketch: the **included** angle of a taper against its
own reflection. Point anchors have no direction and error. `(>)` is **reserved** — an
error with a did-you-mean, kept for a future reading.

**Auto-measure — the smart label.** A dimension with no label renders its **measured
value**: the anchor distance projected on its axis, in drawing units, measured **after
mates resolve** and on the **unbroken** model. The number renders through
**`format:`** — the inherited presentation property
([SPEC 17](#17-property-ledger--support)): the `auto` default rounds to at most
2 decimals, trailing zeros trimmed — a bare number: drafting states units once, in
the title block, and a per-value suffix is `format:`'s job. `format:` shapes the
number **only**, never the measurement — the pieces compose around the formatted
number as **count → glyph → number → label words → `tol:`** (`2× ⌀10 H7`); a
`fraction D` stack rides the same raised / lowered machinery as `tol:` deviations.
The text composes from sources that each own one thing:

| Source | Owns | Example |
|---|---|---|
| the **op** | the glyph | `(o)` → `⌀` / `R` · `(<)` → `°` · `tol:` → `±` (linear `(-)` adds none — a plain length) |
| the **geometry** | the number | `10` |
| the **label** | the words | two-ended: **replaces** the number and its glyph (`a (-) b "180"` — the honest override for schematic or nominal figures); one-ended: **follows** the value (`pin (o) "H7"` → `2× ⌀10 H7`) |
| **`tol:`** | the tolerance, appended | `tol: 0.1` → `±0.1` · `tol: +0.2 -0.05` → stacked deviations, 0.7 × font, raised / lowered · `tol: H7` → a fit class |
| **`pattern:`** · **`mirror:`** | the count prefix — stacked replications multiply | `2× ` · a mirrored pair of holes `4× ` |

**Axis — inference & `project:`.** The anchors pick the axis. A **directed** anchor
sets it — a side name (`left` / `right` → horizontal, `top` / `bottom` → vertical) or
a named edge (a vertical shoulder → a horizontal dim across it); two directed anchors
must be parallel — a perpendicular pair has no shared normal and errors, pointing at
`(<)` ([SPEC 21](#21-errors)). Two **point** anchors read the true **aligned**
distance — the dim line parallel to the span, extension lines perpendicular to it.
`project: horizontal | vertical | aligned` overrides the point readings; against a
directed anchor it must agree — a conflict errors ([SPEC 21](#21-errors)).

**Placement & stacking.** A dimension sits **outside** the geometry, on a `side:` —
a horizontal dim defaults to `bottom`, a vertical one to `right`; anchors both on one
edge pull it there; `side:` must suit the axis. An **aligned** dim sits on the side
of its span facing **away from the geometry centre** — the bbox centre of the
scope's geometry union; its `side: left | right` overrides, read **along the span,
first anchor → second** (`left` is the walker's left). Dims sharing a side pack
into **rows**: each dim, in source order, takes the innermost row where its span —
text included — overlaps nothing already placed, so a chain shares one row and dims
over different stations share too. Row offsets derive from **painted bounds**: a
row stands `clearance` off everything already painted on its side — geometry, text,
callouts, frames, earlier rows — never at a fixed pitch. `clearance` is a
**minimum, not a coordinate** ([SPEC 17](#17-property-ledger--support)); a
per-dim value widens that dim's own stand-off independently, and the packer may
still go farther out to clear obstacles. A statement that leaves along a **ray**
instead of seating on a side — a leader's text, a spilled diametral value — packs the
same way along its exit. `translate` stays the exact nudge; a dimension takes no `gap:`
([SPEC 21](#21-errors)).
The anatomy is baked sheet constants ([SPEC 10.5](#105-layout-constants-baked)):
extension lines spring from the anchors (an edge anchor's from its
dimension-side end — [15.2](#152-anchors)) with a small gap and overshoot past the dim
line — painted the light support tone (`--stroke-light`,
[SPEC 10.1](#101-visual-variables-live-themeable)) unless the statement recolours, so
the geometry reads first; arrows are **drafting-slender** (≈ 3 : 1, filled), sized by
the dim's `stroke-width`; the value rides **above the line, ISO-aligned** — it rotates
with the line and reads from the bottom or from the right, overridable like any text
(the styled-label form + `rotate:`). A span too narrow for text + arrows flips its
arrows outside the extension lines; the value stays centred **inside** while it still
fits there, and only a span too tight even for the bare text slides it past the
nearer one. A packed row also clears every callout's text — leaders, angles, and
every **seated or carried annotation node** register as obstacles before dims
seat, each statement one painted box, a bundle's the union of its children
([15.5](#155-mates--seating), [15.9](#159-drafting-symbols--annotation-composition)). Dimensions
are links, styled per core ([SPEC 9](#9-links)) at the drawing scope's link defaults
([15.1](#151-the-container-the-datum--the-scale)).

```
{ layout: drawing }                            // ratio 1, unit mm, density 4 — the defaults

|sketch#body| {
  draw: move(-80, 0)
        up(14) right(50):neck fillet(3):r1 up(8) right(60):mid fillet(3) down(8) right(50) down(14);
  revolve: x-axis;                             // a turned part: half → whole, axis + edge lines
}

body:left (-) body:right { side: bottom }      // → 160
body:neck (o) { side: left; tol: h6 }          // → ⌀28 h6 — the surface, doubled about the axis
body:r1 (o)                                    // → R3 — the fillet knows its radius
```

### 15.7 Leaders, notes & line conventions

A **callout** is a one-ended link, written tip-first: the glyph hugs the feature, the
line runs toward the text — which is formally the link's **label**, so everything core
says about labels (the `[ ]` form, styling, one inline label) applies verbatim:

| Op | Tip on the feature | For |
|---|---|---|
| `<-` | arrow | an edge or outline |
| `*-` | dot | a leader landing **within** an outline — a face, a region |
| `>-` | **datum** triangle | a datum feature (`>-` is the crow op elsewhere — the scope reinterprets it, as a sequence reinterprets `->`) |

```
bolt <- "THRU"                              // arrow lands on the hole's rim
face *- "Ra 1.6"                            // a dot — a surface note
body:seat >- "A"                            // datum A on that face
bolt <- [ "R3 TYP" { translate: 30 -24 } ]  // a styled / nudged text — the core form
```

- A callout has **one** tip, so the singular `marker:` overrides it; the marker set
  gains **`datum`** ([SPEC 7](#7-nodes)). One arrowhead style per sheet (ISO 129):
  a word leader's `<-` tips with the **same drafting-slender arrow** as every
  dimension; `*-`'s dot and the datum triangle keep their own shapes. A one-ended callout with no text is an
  error; a one-ended `->` / `-*` errors the other way — a leader points *back* at its
  feature (a schematic scope reads that same statement shape as a label wire —
  [SPEC 16.5](#165-wires)). A label-terminated statement is single-hop — chain before the text
  ([SPEC 21](#21-errors)).
- **A fan shares one note.** `a & b <- "2× R5"` — `&` on a one-ended leader op keeps
  **one** text and one landing (the **first** endpoint steers the auto placement;
  `side:` overrides), each endpoint its own ray-cast leg, sharing what trunk the
  geometry permits; a leg that cannot land is an error, never a silent drop. `&` on
  a two-ended op stays the core fan of links ([SPEC 9](#9-links)); on a measuring op
  or mate it errors ([SPEC 21](#21-errors)).
- **A datum's letter is an identity.** `body:seat >- "A"` seats the letter in the
  standard **framed box**, riding the leader's text seat at the landing —
  sheet-space and obstacle-registered like any callout text
  ([15.6](#156-dimensions)). Letters collect per drawing scope — a duplicate errors
  ([SPEC 21](#21-errors)); referenced elsewhere the letter is written bare — a
  `|feature-control|`'s `datums:` validates against the set, and the `|datum|`
  node states the same identity in node form
  ([15.9](#159-drafting-symbols--annotation-composition)).
- **Text placement.** The text auto-places **outward**: a **directed** feature's
  leader leaves straight off its face — along the surface normal — while a point
  feature's runs along the ray from the drawing's datum through it; either way just
  past the geometry union (`note-offset`), horizontal — and the leader ends in a
  short horizontal **landing** (`note-landing`) before it, the drafting elbow.
  `side:` picks the direction instead (a side or a corner); a styled label's
  `translate` nudges from there; the text packs along its exit
  ([15.6](#156-dimensions)). The tip ray-casts onto the drawn outline
  ([15.2](#152-anchors)).
- **The leader makes the note.** A callout's text lowers to a bare leaf — drafting
  callouts are unboxed. A **boxed** note is the `|note|` template
  ([SPEC 8](#8-templates)) wired with an ordinary two-ended link; a **balloon** is
  `|balloon|` plus a leader (`b1 -* nozzle`); bare `"…"` stays plain sheet text
  ("SECTION A-A"). Any other **two-ended** op between two nodes draws a straight
  annotation line, markers per the op — a flow direction, an exploded-view path.

**Line & material conventions.** `hatch()` fills section cuts
([SPEC 10.3](#103-gradients)); `stroke-style: center` / `phantom` are the drafting
dash conventions and `dashed` the hidden-edge one, each on its own child — one node
has one stroke style ([SPEC 7](#7-nodes)). The **`|hidden|`** template
([SPEC 8](#8-templates)) is that child ready-made — a dashed, unfilled pen profile
for interior geometry (a socket, a bore): a feature in the part's `[ ]`, rigid under
mates, riding `break:`, its `:segment`s dimensionable like any sketch's. Besides the section `|plane|` ([15.8](#158-assemblies-views-sheets--titles)), two
chrome types carry the centerline pattern in the part frame ([SPEC 8](#8-templates)): `|centerline|` (a `|line|` — an axis, a symmetry
line, a spoke) and `|pitch-circle|` (an `|oval|`, `width:` its diameter — the bolt
circle; being round, `bc (o)` reads its PCD). A manual `|pitch-circle|` covers what
`pattern:` can't — unequally spaced holes still share one drawn circle.

**Crossing halos.** Annotation linework — dimension, extension, and leader lines —
**breaks** where it crosses geometry: a sheet-space knockout, `halo-margin` wide
each side ([SPEC 10.5](#105-layout-constants-baked)), mask-based so the break holds
over hatching and in every theme. Never over arrowheads, text, frames, or the
contact region (a tip, a landing) — the crossing alone. The generated `|halo|`
chrome rule restyles or removes them scope-wide (`|halo| { … }`), like all chrome.

**Auto chrome — one mechanism, twelve producers.** The lines drafting always draws are
**generated children**, so the cascade styles or removes them with no dedicated knobs
(`|sketch| |centerline| { stroke: none }`):

| Producer | Generates |
|---|---|
| a **fused** `mirror:` ([15.3](#153-the-sketch-pen)) | the axis `\|centerline\|`, overhanging the profile |
| a `revolve:` ([15.3](#153-the-sketch-pen)) | the axis `\|centerline\|` + the `\|shoulder\|` edge lines at every sharp diameter change |
| a `thread:` ([15.3](#153-the-sketch-pen), [15.4](#154-features-holes--patterns)) | the thin minor line + the thread-end line; on a round view, the ¾ thread arc |
| `pattern: radial` ([15.4](#154-features-holes--patterns)) | the `\|pitch-circle\|` through the copies |
| a `\|hole\|` | its centre-mark crosshair |
| a `\|plane\|` ([15.8](#158-assemblies-views-sheets--titles)) | its thick end strokes, the viewing-direction arrows, and the paired section letter |
| a `break:` ([15.3](#153-the-sketch-pen)) | the `\|breakline\|` pair — thin, sharply jogged mid-span |
| a `\|page\|` ([15.8](#158-assemblies-views-sheets--titles)) | the sheet chrome — the `\|frame\|`, the `\|zone\|` references, the `\|tick\|` dividers and centring marks |
| annotation linework crossing geometry | its `\|halo\|` knockouts — the understroke break, above |
| a sheet's **projection link** ([15.8](#158-assemblies-views-sheets--titles)) | its straight `\|projection\|` construction line |
| a `\|door\|` / `\|window\|` ([15.11](#1511-floorplan--the-architectural-dialect)) | the leaf + quarter swing arc / the sill lines |
| a `\|stairs\|` ([15.11](#1511-floorplan--the-architectural-dialect)) | its tread lines + the up arrow |

### 15.8 Assemblies, views, sheets & titles

There is no `|assembly|` type: **an assembly is a drawing whose children mate** — and
drawings **nest**. A child `|drawing|` is one rigid body from outside (the core
sealed-body law): its internal mates, dims, and features stay in its `[ ]`, its
geometry bbox is its parts' union, and it grounds, mates, and anchors like any part.
Build sub-assemblies in isolation, then seat them — the same vocabulary at every
level; reach in where both ends are visible (`motor.shaft:right || pump.rotor:left`).
A project that wants the word writes `|assembly::drawing| { }` — a define, not a
language feature. Item balloons are `|balloon|` + a leader; the parts list is a core
`|table|` beside the drawing; auto-numbering and auto-BOM are deferred
([SPEC 24](#24-deferred)).

A multi-view sheet is ordinary layout: drawings in a `|row|` / `|grid|`, each view its
own scope and `scale:` (a 2 : 1 detail still dims true,
[15.1](#151-the-container-the-datum--the-scale)). There is no `|view|` type and no
projection engine; views **share their axes with `align: origin`**
([SPEC 12](#12-flow-grid--tree)) — a drawing's origin is its datum, so a row of views
lines up datum-to-datum however their dimensions stack, and a grid with
`align: origin; justify: origin` is the first- / third-angle arrangement.
**A drawing's smart label is its title, placed *below*** — it lowers to a
`|footnote|` (the bottom-centred caption template), because drafting titles sit
under the view: `|drawing| "SECTION A-A"`; style every title with
`|drawing| |footnote| { … }`. An authored label always wins; a view sourced from a
marker with **`of:`** composes one instead (**Sections & details**, below).

**Projection construction links.** The thin lines tying a feature across views are
**authored** correspondences, never inferred (no projection engine): in the sheet's
scope — outside every drawing, where both views are visible — the **unmarked `-` op**
between two anchors that dot-path into **different** views draws one **straight** thin
line: `side.screw:head - end.od:top`. On such a link — and only there — the full
drawing anchor vocabulary ([15.2](#152-anchors)) is legal outside a drawing scope,
the **one** exception to sealed bodies. It lowers at layout, after `align: origin`
and every seat have placed the views — never routed, never a packing obstacle — as
generated **`|projection|`** chrome
([15.7](#157-leaders-notes--line-conventions)): `|projection| { … }` restyles or
removes projection lines scope-wide. Everything else stands: a marked op, a
dimension, or a mate across views errors ([SPEC 21](#21-errors)) — a construction
line relates views; it never measures or seats. View-letter arrows (`of:` an arrow
marker) are beyond 1.0 ([SPEC 24](#24-deferred)).

**Sections & details.** Lini is 2D: a **section's cut face is authored** — drawn with
the pen and filled with `hatch()`, as the bushing is ([15.4](#154-features-holes--patterns))
— but a **detail** needs no concession, being a 2D re-render, and the engine is
re-entrant. Either way the view is a plain **`|drawing| { of: <marker> }`** — one
property, one view type. `of:` names a **marker** on the source view by id (like a
chart's `axis:`); the marker's *kind* decides what the view captures:

- **The cutting plane** — `|plane#a| "A" { at: N }`, a chrome child of the view it
  cuts (a `|line|`), its smart label the section **letter**. `at: N` places it: the
  plane runs **perpendicular to** an axis at station `N`, the axis defaulting to the
  model's longer one or named — `at: 40 y-axis` (`break:`'s convention). It lowers to
  the ISO plane: a thin dash-dot line (`stroke-style: center`) across the geometry and
  its overhang, **thick end strokes** just past each end, a viewing-direction **arrow**
  (the slender dimension arrow) at each, and the letter beside them. `facing: left |
  right | up | down` turns the arrows — default `right` for a vertical plane, `down`
  for a horizontal one. The cascade styles or removes the whole marker.
- **The magnifier** — `|magnifier#c| "C" { width: … }`, ringing a region: a thin
  outlined circle (`|oval|`, `--stroke-light`), `width:` its diameter, positioned with
  `translate:` like any feature; its smart label the **letter**, set just outside the
  rim at 45°. An ordinary part-frame child, like a `|balloon|` — not generated chrome,
  and the **single source of truth for the region** it names.
- **The section view** — `|drawing#sec| { of: a }`, `a` a `|plane|`. The face is
  **authored** (the hatched cut you draw); `of:` composes the title from the plane's
  letter — **doubled**, `A-A` — plus the drafting **ratio**: the view's own `scale:`
  read directly (`1:1` at the default, `2:1` enlarged, `1:1.5` reduced, ≤ 2 dp —
  [15.1](#151-the-container-the-datum--the-scale)).
- **The detail view** — `|drawing#det| { of: c }`, `c` a `|magnifier|`. The view takes
  its **centre** and **diameter** from the marker and its **letter** titles it (`C
  (1:1)`, composed as above), so only the magnifying `scale:` is yours. The engine
  **re-lays the marker's host view** at the detail's scale — a plain 2D re-render, no
  projection — keeping the **geometry**, **dropping the source's annotations**, shifted
  to centre the region and **clipped to the circle**, with the circle drawn as its
  **boundary** — the marker's own thin chrome paint, since the rim is the marker's
  other half: both wear `.lini-magnifier`, so restyling the marker (`|magnifier| { … }`,
  or the instance's block) carries the rim with it and neither inlines a thing
  ([SPEC 18](#18-svg-output)). The detail's own `[ ]` annotations dimension the re-laid
  copies (by the ids the clones carry from the source); only the detail's own links may
  reach them. A detail re-renders a **base** view — `of:` can't name a marker inside
  another sourced view.

**The sheet.** `|page|` gives the multi-view story its walls: the trimmed ISO 5457
sheet as a **template container**, not a layout — inside its frame it is an ordinary
container (default `flow`; `layout:` / `columns:` / `direction:` free), hosting
drawings, tables, and notes as normal children in **sheet space** (a page is never a
drawing scope). **`sheet:`** names the trimmed size — `sheet: a3`,
`sheet: a4 landscape` — pure sugar for `width` / `height` **in millimetres** (the
orientation keyword swaps the pair; ISO defaults — A4 and A5 portrait, A3–A0
landscape; a bare `|page|` is `a4`), so an explicit `width:` / `height:` overrides
through the ordinary slot and a custom sheet still derives its zones. The
**ANSI/ASME Y14.1 letters** ride the same sugar in their own millimetres —
`sheet: b` (`a`…`e`; `a` portrait, `b`–`e` landscape) — nothing else differs. A
page's `direction` defaults by **orientation** — landscape → `row`, portrait →
`column` — so views flow with the paper; set it to override. A page
carries **no `scale:`** of its own: the root's `density:` sets pixels per millimetre
(default 4, screen-only — [15.1](#151-the-container-the-datum--the-scale)), a
drawing's `scale:` is its drafting ratio directly, and so a default drawing on any
page draws **1 : 1 true** (a 2 : 1 detail is `scale: 2`).

The ISO furniture is generated chrome ([15.7](#157-leaders-notes--line-conventions)):
the thick `|frame|` 10 mm in from every trimmed edge; the **zone grid** —
divisions of ≈ 50 mm, rounded to the nearest even count per edge (A4 4 × 6,
A3 8 × 6, A0 24 × 16) — numbered `1…` left-to-right along top and bottom and
lettered `A…` top-to-bottom along both sides, drawn as `|zone|` labels and
`|tick|` dividers in the **reference band**, the margin beside the frame; and the
four centring marks, each crossing the frame at an edge's midpoint (the middle
divider, which would coincide, is not drawn). The
content area is the frame inset by 5 mm (`padding:` adds to it). A
**`|title-block|`** child (ISO 7200 — a `|table|`, [SPEC 8](#8-templates)) is seated
by **type**, flush inside the frame's bottom-right corner. **String-valued field
properties** — `title`, `drawing-number`, `revision`, `date`, `sheet-number`,
`author`, `approved`, `department`, `reference`, `document-type`, `status` — desugar
(like `sheet:`) into the fixed ISO grid: each a caption in the muted footer tone
over its value, and **absent fields collapse** their cells, so the default block is
minimal (Title / DWG No. / Rev / Sheet). The block's **smart label is its `title`
field** — `|title-block| "Socket cap screw"` lowers to the same generated spanning
cell; a label **or any field property** selects the structured-field mode, and a
`|title-block|` with neither keeps the plain-table form — its cells fully authored.
In field mode, authored children remain **ordinary cells after the generated ones**,
in the same grid — `cell:` / `span:` honoured; an authored cell landing on a
generated field's slot errors, naming the field ([SPEC 21](#21-errors)). There is no
`logo:` — a logo is an `|image|` in a cell ([SPEC 7](#7-nodes)), or anywhere on the
page. A file whose drawn content is only pages **hugs them** — the paper is the
margin, so the root's `padding` defaults to 0 (your own `{ padding: … }` still
wins) and the sheet runs edge to edge of the SVG. That same predicate makes the
sheet **true-scale in print** ([SPEC 18](#18-svg-output)).

```
|page| { sheet: a4 } [
  |drawing#side| "DIN 912 — M8 × 40" [ … ]         // 1 : 1 on the sheet
  |drawing#detail| "DETAIL A" { scale: 2 } [ … ]    // a 2 : 1 view
  |title-block| { columns: 60 auto } [
    "Title" "Socket cap screw"
    "Scale" "1:1"
  ]
]
```

### 15.9 Drafting symbols & annotation composition

GD&T rides three node templates over one shared **drafting-glyph set** — the
characteristic symbols, the modifier circles (Ⓜ Ⓛ Ⓕ Ⓣ Ⓟ), the finish vees —
drawn as paths like icons but sized in **natural units**, never fit to a box: a
glyph's height follows the annotation `font-size`, its line weight the
statement's `stroke-width`, so every symbol reads at dimension-linework weight
beside every value, at every view scale. All three types are **sheet content**
(`scale: 1` — [15.1](#151-the-container-the-datum--the-scale)) and drawing-scope
only ([SPEC 21](#21-errors)). Place one with
`translate:`, or attach it three ways, all ordinary: **seat** it on a face with `||`
([15.5](#155-mates--seating)), wire it with a leader (`body:seat <- sf` — the
same node either way), or carry it in an annotation's `[ ]` (below).

**`|surface-finish|`** — the ISO 1302 surface-texture symbol. Its smart label is
the **textual indication** (`|surface-finish| "Ra 1.6"`), riding the symbol's
long leg; `symbol:` picks the variant:

| `symbol:` | Draws | Means |
|---|---|---|
| `basic` (default) | the bare vee | any process |
| `machined` | vee + bar | material removal required |
| `prohibited` | vee + circle | removal prohibited |

Seated, the vee's tip stands on the face (the type's seat anchor,
[15.5](#155-mates--seating)); `rotate:` turns it for a vertical face.

**`|feature-control|`** — the GD&T frame. The common single frame carries its
properties directly; a **composite / combined** frame holds `|control|`
children, one row each (mixing the two forms errors). The **smart label names
the characteristic** — the frame's in one-row form, each `|control|`'s
otherwise; longhand `characteristic:`; setting both errors. The set is
**ISO 1101's fourteen** (ASME Y14.5-2018 dropped `concentricity` / `symmetry`;
lini's drafting lineage is ISO and both validate):

| Group | Characteristics | `datums:` |
|---|---|---|
| form | `straightness` · `flatness` · `circularity` · `cylindricity` | forbidden |
| profile | `profile-line` · `profile-surface` | optional |
| orientation | `angularity` · `perpendicularity` · `parallelism` | required |
| location | `position` (optional) · `concentricity` · `symmetry` (required) | — |
| runout | `circular-runout` · `total-runout` | required |

The row properties, each owning one compartment slot:

- **`tol:`** — the tolerance **zone width**, required: a number > 0 (the
  deviation / fit forms are a dimension's — [15.6](#156-dimensions)).
- **`zone: diameter | spherical`** — the ⌀ / S⌀ zone prefix; legal only where
  the zone is axial — `position`, `straightness`, `perpendicularity`,
  `parallelism`, `angularity`, `concentricity`.
- **`material: maximum | least`** — Ⓜ / Ⓛ after the value; legal on the
  feature-of-size controls — `position`, the orientation three, `straightness`.
- **`datums: A, B maximum, C`** — primary → tertiary, at most three; each a
  **bare letter** declared in the scope (`>-` or `|datum|`,
  [15.7](#157-leaders-notes--line-conventions)) with an optional per-datum
  `maximum` / `least`; an unknown letter errors naming the declared set.
- **`modifiers:`** — ordered extras after the material modifier:
  `projected N` (Ⓟ plus the projection length), `free-state` (Ⓕ),
  `tangent-plane` (Ⓣ).

Any combination outside these rules — the table's forbidden / required cells,
an unknown characteristic — is an **error** with a correction
([SPEC 21](#21-errors)): a frame renders semantically valid or not at all,
never plausible-looking and wrong. Adjacent `|control|` rows sharing one
characteristic **merge its symbol compartment** — the composite frame; rows
with different characteristics stack as a combined frame, in source order.

**`|datum|`** — the framed datum letter as a **node**: its smart label the
letter, joining the scope's identity set exactly as `>-` does
([15.7](#157-leaders-notes--line-conventions) — a duplicate errors across both
forms), one frame anatomy shared with the leader's box. Standalone it seats or
wires like any annotation; carried in a dimension's `[ ]` it states the
feature-of-size **axis datum** — the measured feature's axis is the datum
feature.

**Annotation nodes on a dimension or leader.** A drawing link's `[ ]` may carry
these **nodes** beside its text labels: each stacks at the statement's **text
seat** — under the dim value or callout lines, in source order — rides the row
like the text does, and registers its painted bounds with the packer
([15.6](#156-dimensions)), so no row overlaps a carried frame. Strings keep
their label semantics (replace / follows, [15.6](#156-dimensions)); a node is
never a label. **Core routed links stay text-only** — the grammar is
scope-blind ([SPEC 22](#22-grammar)), and a node in a `[ ]` outside a drawing
scope errors at resolve ([SPEC 21](#21-errors)).

```
{ layout: drawing }

|sketch#body| { draw: …; revolve: x-axis }
body:seat >- "A"                                     // datum A, the leader form

|surface-finish#sf| "Ra 1.6" { symbol: machined }
sf || body:top                                       // the vee stands on the face

body:mid (o) { tol: h6 } [                           // a frame rides the dimension
  |feature-control| "circular-runout" { tol: 0.05; datums: A }
]
```

### 15.10 Lowering

`layout: drawing` resolves in the **layout** phase ([SPEC 19](#19-compile-pipeline)) —
geometry must exist before it can be measured:

1. **Geometry** per child, bottom-up: fold `draw:` to a path (corner modifiers applied
   cyclically through `close()`), collect its `:segment`s, apply `mirror:` /
   `revolve:` (+ the edge lines and `thread:` dressing), expand `pattern:`, build
   `break:`'s view map; nested drawings lower first, becoming rigid subtrees. Compute
   each node's geometry bbox (stroke excluded) and paint bbox (core).
2. **Place** children: origins on the datum, `translate:` applied.
3. **Mates**: walk from the ground; rotate first, seat, the child's own translate
   after; flag cycles and over-constraints.
4. **Measure** every annotation's anchors against the seated, unbroken geometry;
   compose the texts (glyph + number / label + `tol:` + count).
5. **Annotate**: assign dims to sides and pack the rows in source order; auto-place
   callout texts outward; ray-cast leader tips; land the elbow.
6. **Lower** to primitives at baked coordinates: sketch → `|path|`; hole → `|oval|` +
   centre marks; the auto chrome → generated children; dim → extension `|line|`s + a
   marker-tipped dimension `|line|` + text; an angle → its arc `|path|` + text;
   leader → `|line|` + marker + text; hatch → one deduplicated `<defs>` `<pattern>`.
7. **Scale** geometry per the effective per-node scale; chrome stays sheet-space. Emit
   geometry in source order and annotations **above** all of it (the drawing's one
   draw-order override, like a chart's semantic order; `layer:` still wins).

The output is an ordinary primitive subtree ([SPEC 11](#11-the-layout-model),
seam 3). The **parser is scope-blind**: the ops and forms parse everywhere and
*mean* drawing only in a drawing scope — elsewhere they error at resolve
([SPEC 21](#21-errors)).

The drawing property index — owners, value shapes, defaults — is the
[Property Ledger](#17-property-ledger--support); each property's law lives in its
subsection above.

### 15.11 Floorplan — the architectural dialect

`layout: floorplan` — and the `|floorplan|` template (`|drawing|`-based, so
`|drawing|`-scoped rules dress a floorplan too) — **is the drawing engine**
under another vocabulary: everything in this section applies unchanged (the
datum, `scale:` / `unit:`, anchors, the pen, `pattern:`, dimensions, leaders,
mates, sheets, hatch), and **every "drawing-scope only" rule reads a floorplan
scope as a drawing scope** — the ops, `tol:`, the drafting symbols, and the
[SPEC 21](#21-errors) gates hold unchanged. What the dialect adds is a
**vocabulary with its own gate**: the floorplan types are legal only in a
floorplan scope — a `|wall|` in a plain drawing errors like any scope type
([SPEC 21](#21-errors)) — so each drafting language keeps its own completion
surface, while every drawing-global mechanism (`|sketch|`, `|hole|`, `|note|`,
`|page|`, …) stays welcome here. No new role variables.

**True-size defaults.** A floorplan type's intrinsic sizes — a wall's
`thickness:`, an opening's `width:`, every fixture body — are **physical
millimetres**, converted to drawing units through the scope's `unit:` where
each is read ([15.1](#151-the-container-the-datum--the-scale)): a bed is
1500 × 2000 mm whether the file drafts in `m` or `mm`. An **authored** value is
drawing units like everything else — at `unit: m`, a 100 mm partition reads
`thickness: 0.1`.

**Walls.** A `|wall|` is a `|sketch|` whose `draw:` traces the wall's
**centreline** — named `:segment`s and all — and **`thickness:`** (inherits
nearest-wins, like `unit:`; default 200 mm) grows it into the wall outline:
each run offset **± thickness ∕ 2**, corners **mitred** (an acute spike bevels
at miter limit 4), an arc offset to its concentric pair (an arc radius under
thickness ∕ 2 errors), an **open** end butt-capped at its endpoint, a `close()`
seam mitred like any corner; `curve()` in a wall's `draw:` errors
([SPEC 21](#21-errors)). The outline **is** the wall's shape: it takes the
paint — solid `--stroke-dark`, the poché read (`{ fill: --bg; stroke:
--stroke-dark }` is the hollow double-line look, `fill: hatch(45)` a section
convention — both show their junctions, where the solid default merges by
paint order) — and it is the geometry bbox
([15.1](#151-the-container-the-datum--the-scale)). In
[15.10](#1510-lowering) step 1 the offset runs **after the `draw:` fold and
before the bboxes**, and an opening clips it there — resolving against its
already-folded parent, the one place a child reads down from its part.
Anchors: `:segment`s read the **centreline**, bbox points the **outline** —
and every named segment **derives its two face anchors**, `name-in` /
`name-out`: the segment's own offset edges. On a **closed** run `-in` is the
enclosed side; on an **open** one it is the left of the pen's travel (the
named-edge convention, [15.5](#155-mates--seating)). So a **clear room span**
— what a listing plan dimensions — is face to face,
`outer:north-in (-) bedwall:top`, measured like everything else; an authored
segment name ending `-in` / `-out` errors as colliding with its derived twin
([SPEC 21](#21-errors)). `|partition|` is the built-in 100 mm interior define
([SPEC 8](#8-templates)).

**Openings.** A `|door|` / `|window|` rides in its wall's `[ ]`, stationed
**on a straight named segment**: `on:` the segment, bare (the `thread:` shape,
[15.3](#153-the-sketch-pen)); `at:` the near jamb's distance from the
segment's start; `width:` the clear opening. The gap **clips the wall
outline** at the two jambs — a profile clip, not `break:`
([15.3](#153-the-sketch-pen)): the wall keeps its length, nothing compresses,
no `|breakline|` draws, and each jamb closes flat across the thickness. An
opening's own geometry is that jamb-to-jamb box — `width` × `thickness`,
seated on the segment — so an id'd opening anchors a dimension at its centre
(`outer:west (-) outer.entry (-) outer:east`, the location chain along a
wall — a dot-path, the sealed-body rule as everywhere, [SPEC 9](#9-links)); it
is placed by `on:` / `at:` alone, so `translate:` on an opening errors.
`hinge: start | end` picks the jamb by the segment's draw direction;
`swing: left | right` the side the leaf opens toward — `left` is the left of
the pen's travel, the named-edge convention ([15.5](#155-mates--seating)).
The chrome is generated children in the thin tone
([15.7](#157-leaders-notes--line-conventions)): a door's **leaf** — a line of
length `width` from the hinge jamb, drawn at 90° open — and its quarter
**swing arc**, radius `width`, sweeping leaf to closed; `symbol: double`
splits two half-width leaves + arcs mirrored about the gap's centre;
`symbol: sliding` draws two overlapping half-length panel lines offset to
either face, no arc (`hinge:` / `swing:` on it error). A window draws two
**sill lines** across the gap at the thickness's thirds — the double-glazing
read. An opening past its segment, on a curved one, or overlapping another
errors ([SPEC 21](#21-errors)).

**Fixtures.** Six symbol-bodied types — the discretes' pattern
([SPEC 16.3](#163-discretes)), their **smart label below the body like a
discrete's value** — except an `|appliance|`'s, which centres **in** its
body: the labelled-box convention (`"F"`, `"DW"`, `"W/D"`). Fixture and
opening labels stay **readable like dimension text** — ISO-aligned, from
the bottom or the right, never upside-down ([15.6](#156-dimensions)'s rule,
shared). (An opening's is
its schedule tag beside the gap; a `|floorplan|`'s is the drawing title it
inherits, [15.8](#158-assemblies-views-sheets--titles); a `|wall|`'s keeps
the sketch's centred read.) `width` / `height` are floors as everywhere
([SPEC 5](#5-the-box-model)) and the body **stretches** to the resolved box;
`symbol:` picks the variant:

| Type | `symbol:` | Body (mm) |
|---|---|---|
| `\|bed\|` | `queen` *(default)* · `king` · `double` · `single` | 1500 × 2000 · 1800 × 2000 · 1350 × 1900 · 900 × 2000 |
| `\|sofa\|` | `three` *(default)* · `two` · `one` (the armchair) · `corner` · `stool` (the bar stool — a plain round seat) | 2200 × 900 · 1600 × 900 · 900 × 900 · 2400 × 2400 L · ⌀350 |
| `\|dining\|` | `six` *(default)* · `four` · `round` | the **tabletop** — 1800 × 900 · 1200 × 800 · ⌀1000 — its chairs (450 × 450, drawn a small pull-back off the edge; `six` 3 + 3 on the long sides, `four` 2 + 2, `round` 4 at the quadrants) extending the bbox |
| `\|bath\|` | `tub` *(default)* · `shower` · `toilet` · `sink` · `double-sink` (one unit, two square basins — the kitchen run's) | 1700 × 750 · 900 × 900 · 700 × 400 · 500 × 400 · 800 × 450 |
| `\|appliance\|` | `stove` *(default)* · `fridge` · `washer` · `dishwasher` | 600 × 600 each |
| `\|stairs\|` | — (`steps: N` **required**, ≥ 2) | 900 wide × N × 250 run; treads across the flight, the **up arrow** from the first tread past the last |

A counter, island, desk, or coffee table is a plain `|rect|`; anything else is
a `|sketch|` define — the parts-library escape
([15.4](#154-features-holes--patterns)). A room name is plain sheet text
(`"KITCHEN"`, an authored area beside it); computed room areas, curved-segment
openings, and a north arrow are deferred ([SPEC 24](#24-deferred)).

---

## 16. Schematic

A **schematic** reads a diagram as a circuit sheet: `layout: schematic`
places **components** and lets the orthogonal router draw the **wires** —
unlike sequence and drawing, the engine never consumes its links
([SPEC 11](#11-the-layout-model)); it places, reinterprets a few link forms,
and dresses the result. Wires land on **pins** — fixed ports the router hits
exactly ([ROUTING.md](ROUTING.md), Fixed ports) — bend square, and meet at
**junction dots**. Everything else is the core: a named wire is a link label,
the region boxes are `|group|`s, the sheet is a `|page|`, the title block ISO
7200 ([SPEC 15.8](#158-assemblies-views-sheets--titles)). Its children split
by role:

| Child | Is | Drawn |
|---|---|---|
| a 3+-pin part (`\|component\|`, `\|opamp\|`, `\|J\|`, `\|Q\|`), or anything explicitly placed (`cell:`) | an **anchor** | on the scope's track grid |
| a `\|label\|`, or an *unplaced* 1–2-pin part | a **satellite** | seated at the pin its wire touches |
| a link (`a - b`) | a **wire** | routed orthogonally, square-cornered, junction-dotted |
| a one-ended link with text or a capsule (`U7.DIAG - "NSTDBY"`, `c24.p2 - \|gnd\|`) | a **label wire** | a lead to a seated `\|label\|` — a run of trace under a plain net name, a stub to a tag or a symbol ([16.4](#164-labels)) |

Vocabulary: a **component** is the part instance; a **symbol** is the drawing
it (or a label) wears — `symbol:` names one, exactly as on `|icon|`
([SPEC 7](#7-nodes)). Schematic types are legal only in a schematic scope
([SPEC 21](#21-errors)); the schematic **link laws** ([16.5](#165-wires))
reach links written in nested ordinary containers, but **placement never
cascades** — a nested `|row|` or `|grid|` places its own children, exactly as
in a drawing.

### 16.1 Placement — anchors & satellites

**Anchors ride tracks.** Anchors take the scope's **track grid**: one row by
default, in declaration order; `columns: N` wraps; `cell: c r` places
explicitly. Track indices are **ordinal** — tracks spring into existence up
to the largest referenced index and **empty tracks collapse entirely**, so
sparse indices (10, 20, 30…) are safe ordering room and never inject
invisible space. This is the engine's own track list; it does not alter the
grid layout's laws ([SPEC 12](#12-flow-grid--tree)). Track sizing reads each
anchor's **cluster** — the anchor plus its seated satellites — so satellites
consume space, never cells.

**Satellites seat at pins.** A satellite chain reads its wire:

- **one placed end** — the chain hangs off the wire's first leg (one seat out
  along the pin, and as much farther as it needs to stand clear of the part it
  hangs from — the seat measured on the **connection geometry** a wire arrives
  at, a flag's symbol rather than the name beside it, which need only not reach
  back over the part) and grows from there, link by link, in the direction of its
  **terminator's** connection geometry. Only a `|label|` carries that
  convention — a `|gnd|` is drawn with its connection point at its top, so the
  chain grows down; a power flag's sits at its bottom, so up; a text label, and
  any chain a *part* terminates, runs along the pin's outward normal;
- **two placed ends** — the chain's satellites distribute along the straight
  line between the two pins at even fractions;
- **no placed end** — the parts fall back to the flow with a warning.

A chain that leaves its pin **sideways** takes a **lane** of its own — its own
distance out along the pin before it turns onto the ray — so its lead is one
square turn; one that grows straight out along its pin takes no lane and
**stacks** outward instead. Either way chains order by **the pins they hang
on**, so no two leads overtake each other and cross; chains sharing one pin
keep statement order. A seated satellite
registers as a router obstacle like any node. **`cell:` promotes a satellite
to an anchor**; `translate:` nudges it from its seat (pin-relative — move
the component and the nudge travels along, [SPEC 5](#5-the-box-model)).

**Pose is rotation.** Every schematic part has authored connection geometry
— pins on parts, one connection point on a label symbol. A satellite
**auto-poses**: the seat pass picks the 90°-step pose that presents its
terminal back up the chain's own growth ray (deterministic tie-break: the
unrotated pose, then clockwise) — so a ground, which sets that ray from its
own drawing, is never turned, and a part in the middle of the chain stands to
meet it. An explicit **`rotate: 0 | 90 | 180 | 270`** forces the pose;
the seat direction derives from the rotated connection point. Rotation on a
connection-bearing part is read **at lowering** — pins re-side, the symbol
re-lays, and every text (net text, ref, value, pin names) stays upright —
never as a paint transform; any other angle is an error
([SPEC 21](#21-errors)).

### 16.2 Components & pins

```
|component#U7| "TMC2300-LA-T" [
  |pin#VS| { number: 18 };  |pin#STEP| { number: 4 }          // auto — the bilateral split
  |pin#nstdby| "VIO/NSTDBY" { side: right; number: 11 }
]
U7.VS - c24.p1 "VM"
```

`|component|` is the generic pin-bearing box — an IC, a module, a relay. Its
smart label is the **part name / value**; its `[ ]` holds `|pin|` children.
Pins without a `side:` split **bilaterally** — the first ⌈n/2⌉ on the left,
the rest on the right, declaration order top-to-bottom (the ⌈n/2⌉ split of
[SPEC 12](#12-flow-grid--tree)'s bilateral tree, mirrored — a component reads
left-to-right); `side: left | right | top | bottom`
overrides, and explicitly-sided pins are excluded from the split count. Pins
lower into generated *anonymous* side rails — scope-transparent
([SPEC 9](#endpoints--scope)), so `U7.VS` resolves with no rail in the path —
one `pin-pitch` apart **along** the rail they landed on.

A **pin**'s smart label is its **name**, displayed inside the body; with no
label the pin's **id is displayed** — schematic identity is drawn, the way a
`|hole|` draws its centre marks — and the label form covers names that can't
be ids (`"VIO/NSTDBY"`, `"1.8VOUT"`). `number:` draws the pin number outside,
beside the **stub** — the short lead the pin extends outward; the wire lands
on the stub tip, departing outward along the pin's side. Pin anatomy — stub,
name, number — folds into the **component's own** routing obstacle, and a
pin's `translate:` slides it along its side — a cross-axis component is an
error (a pin lives on its side). A single-pin component is legal
(a test point, a mounting pad); an unwired part needs no id at all. `|pin|`
the type and `pin:` the out-of-flow property ([SPEC 5](#5-the-box-model)) are
one word in two roles — never ambiguous: a type lives in bars, a property
before a `:`.

**The id is the reference designator.** A component or discrete displays its
id verbatim (`#U7` reads "U7"). An **anonymous** part **mints a display
ref** — prefix from its type (`|R|` → R1, R2…, IEEE 315), `prefix:`
overriding (`|ic::component| { prefix: "IC" }` mints IC1…), declaration
order, skipping authored names. A minted ref is **display-only, never an
endpoint** — wiring `R1.p1` to a minted ref is an unknown endpoint
([SPEC 21](#21-errors)): don't care → free numbering; wire it → name it.
Ref/value text places deterministically — above a component (the ref over the
value), across a discrete's symbol: above and below an upright one, **beside**
a turned one, whose own wire runs down the column those seats would take.
`translate:` on the styled-label form nudges either.

`|J|` is the **connector** — a `|component|` define, prefix J, whose pins
show numbers only; **`pins: N`** generates N numbered, nameless pins
(`|J#J3| "JST S4B-ZR" { pins: 4 }`). `|opamp|` is the amplifier triangle —
prefix U, pins `out`, `inp`, `inn`, its power pins present but hidden by
default.

### 16.3 Discretes

Two- and three-terminal parts drawn as symbols (IEC), each with **generated
pins** — so `c24.p1` works with zero authoring — and its ref family as its
type name:

| Type | Mints | Pins | `symbol:` variants |
|---|---|---|---|
| `\|R\|` | R1… | p1 p2 | — |
| `\|C\|` | C1… | p1 p2 | `polarized` |
| `\|L\|` | L1… | p1 p2 | — |
| `\|D\|` | D1… | a k | `zener` · `tvs` · `schottky` |
| `\|LED\|` | LED1… | a k | — |
| `\|Q\|` | Q1… | b c e / g d s | `npn` *(default)* · `pnp` · `nfet` · `pfet` · `nfet-circled` · `pfet-circled` (the ringed FET) |
| `\|Y\|` | Y1… | p1 p2 | — |
| `\|F\|` | F1… | p1 p2 | — |
| `\|FB\|` | FB1… | p1 p2 | — |
| `\|SW\|` | SW1… | p1 p2 | `toggle` *(default)* · `push` |
| `\|BT\|` | BT1… | plus minus | `cell` *(default)* · `battery` |
| `\|V\|` / `\|I\|` | V1… / I1… | plus minus | `dc` *(default)* · `ac` |

The smart label is the **value** (`|R#R18| "470m"`); `symbol:` picks the
variant — one knob for every family, and it sets the pin ids where they are
semantic (`d3.a`, `q1.b`, `q1.g` per variant, `bt1.plus`). Polarity in a
wire is a pin path (`vm - |D|.k - x` — cathode first). Orientation is
`rotate:` ([16.1](#161-placement--anchors--satellites)).

### 16.4 Labels

**Components have pins; a label is its own terminal.** `|label|` is the net
tag: its smart label is the **net text**, drawn in the tag outline;
**`shape:`** picks the outline — `plain` *(default, no outline at all)* ·
`left` · `right` · `both` (a **flag**, one or both ends drawn to a point) ·
`round` (a stadium) — the shapes are **visual, not semantic** (the
conventional readings — output, input, bidirectional — are the reader's, as a
sequence's `->` vs `-->` are); **`symbol:`** swaps in a drawing from the
**schematic symbol set** — `gnd` · `earth` · `chassis` · `power` · `nc` ·
`antenna` — text beside it like an icon's, never under it: the symbol's own
edge is the label's connection point, and the wire arrives there. Text alone is a net label, a symbol alone a
ground, symbol + text a power flag. `|gnd|` and `|nc|` ship as built-in
defines; a power net is a one-line define with intrinsic text
([SPEC 8](#8-templates)):

```
{ layout: schematic; |vm::label| { symbol: power } [ "VM" ] }
c24.p2 - |gnd|
U7.VS  - |vm|
```

A label has **no pins and no dot-path**; a wire lands on its connection
point — a fixed port like a pin's. `:side` is an error on **every** terminal,
pin or label — a terminal owns its connection geometry.

**A plain label is a run, not a stop.** The two shaped readings are *bodies*
the wire ends on — an outlined tag, a symbol — but a sheet writes a bare net
name **beside a stretch of trace**, and `shape: plain` with no `symbol:` draws
exactly that: the label's box is a **run** of wire, its connection point the
end **away** from the pin, so the router draws one wire the whole length of it
and the name ends up over a trace. The run is `net-label-run` long
([SPEC 10.5](#105-layout-constants-baked)) and grows for a longer name — the
ordinary `width` floor ([SPEC 5](#5-the-box-model)), so `|label| { width: N }`
raises it. Being a run and not a body, it is no obstacle: its frame is that
landing line alone, and its text obstructs nothing, exactly as a link label
does not ([SPEC 9](#9-links)).

**Net text stands off its wire, never on it.** The name sits a constant
`net-label-offset` clear of the centreline — a schematic wire is never cut
([16.5](#165-wires)). Which side:

| Run | Text |
|---|---|
| horizontal | **above** |
| vertical | **beside**, on the freer side — more clear space that way; ties break on the routing side rank (right → bottom → left → top) |

**`side: left \| right \| top \| bottom`** forces it — on the `|label|` for the
minted run, on the **wire statement** for the two-ended form (`u7.vs -
c24.p1 "VM" { side: bottom }`), one more owner of the `side` homonym
([SPEC 17](#17-property-ledger--support)). Text always stays **upright**: a run
poses like any part ([16.1](#161-placement--anchors--satellites)) and rotation
is read at lowering, never as a paint transform.

### 16.5 Wires

A schematic wire is an ordinary link, routed by the orthogonal router
([ROUTING.md](ROUTING.md)) with the scope's dress: ends land on **fixed
ports** (stub tips, label connection points), corners bend **square**
(`corner-radius: 0`, the scope's link default — [SPEC 17](#17-property-ledger--support)),
and a **junction dot** — generated `|junction|` chrome — marks every point
where **three or more wire ends meet** (a fan's trunk split, a shared pin).
A **plain net run**'s lead never counts — the run's box *is* the trace it
names ([16.4](#164-labels)), so that wire is the one being named, not a
second conductor leaving the point; every other terminal's lead counts, so a
rail forking to its power flag and its decoupling cap is dotted where it
forks. Crossings stay clean and dotless. The wire laws:

- **Pinless landing gates on arity**, never on a type list: a wire to a
  1-pin part lands on it; to a 2-pin part, on the next free pin in the
  type's pin order (both taken → an error naming one); to a 3+-pin part it
  is an error suggesting a pin. Dangling pins are legal — `|R| -> a` lands
  `p1`, `p2` stays open.
- **A chain passes through a 2-pin part**: the named (or next-free) pin is
  the entry, the *other* pin the exit — `vm - |R| - |LED| - |gnd|` is a
  series circuit in one line; `vm - |D|.k - x` enters at the cathode and
  exits at the anode. This law is the one **carve-out from the chain
  equivalence** ([SPEC 9](#9-links), [SPEC 19](#19-compile-pipeline)): two
  statements naming one pin are a junction there, not a pass, so a schematic
  chain lowers cut **only where the pass-through resolved** (both pins
  written down) and one whose landing the scope cannot see stays a chain —
  only the chain itself still says what it means.
- **Duplicates error** — a repeated endpoint pair means nothing on a sheet;
  and **same-pin landings merge** into one implicit fan at the shared port,
  drawn as one lead until the split, dotted there.
- **No implicit auto-create.** A bare unknown id never mints a box in a
  schematic scope — the error suggests the quoted form: `did you mean
  - "NSTDBY" (a net label)?`. Declaration-at-first-use is the typed capsule
  ([SPEC 9](#9-links)).
- **A label wire** is the one-ended form — the drawing-leader statement
  shape ([SPEC 15.7](#157-leaders-notes--line-conventions)) read by this
  scope: `U7.DIAG - "NSTDBY"` mints a `|label|` seated at the pin; the op's
  **end marker sets the label's `shape:`** (`-` plain, `->` right, `-<`
  left, `-<>` both, `-*` round) exactly as an operator's line sets
  `stroke-style` ([SPEC 9](#9-links)); an explicit `shape:` wins. A capsule
  terminator (`- |gnd|`) is the symbol form of the same statement.
- **Markers shape labels, nothing else**: an op's marker is legal only on a
  wire ending in a text-form label — a marked part-to-part wire, or a
  marker at a symbol-form label, errors. The op's **line** stays free
  (`--` is a dashed wire, plain `stroke-style`).
- A two-ended wire's **net name is its link label** — `U7.VS - c24.p1 "VM"`
  — placed by `along:` as everywhere, then given the net-label convention
  **whole** ([16.4](#164-labels)): stood clear of the trace *and* inked
  `--lini-label-ink`, since a sheet carries no wire text but net names. Both
  spellings of one name therefore read alike. A shaped tag on such a wire is a
  separate label-wire statement at one of its pins.
- **A sheet never opens a trace.** The label knockout — a label riding its
  wire, the wire masked open behind it — is the **diagram** convention
  ([SPEC 9](#9-links)); a schematic scope draws in the other one, standing the
  net name beside the line, so there is nothing to cut around. The law is the
  scope's, not the placement's: should a name ever fail to clear its wire, it
  overlaps a whole trace, which still reads, rather than punching a hole in
  one, which is a wrong drawing.

### 16.6 Look

The classic sheet is the default *inside the scope*, riding role variables
([SPEC 10](#10-colour-variables--expressions)) so a theme retunes it: wires
green, part bodies pale yellow with dark-red outlines, labels teal, pin
numbers muted, the scene beige — each a `light-dark()` pair. The scope's
generated link defaults ([SPEC 17](#17-property-ledger--support)): a thinner
wire, a tighter `clearance`, `corner-radius: 0`.

**Opting into the engine is one decision.** `layout: schematic` carries the
scope's own config — the track `gap` and that tighter `clearance` — wherever
it is written: the sheet's baked constants ([SPEC 10.5](#105-layout-constants-baked))
are tuned to it, so a scope routing at the diagram's default would stray the
leads it seats. `|schematic|` is the template (`|block|` + the layout, plus
the sheet wash); a root `{ layout: schematic }` works like every root engine;
and `|region::group| { layout: schematic }` is a captioned block that seats
its own parts, keeping its own paint. Groups, pages, and title blocks are the
core types, restyled by scoped rules.

### 16.7 Lowering

`layout: schematic` resolves in the layout phase
([SPEC 19](#19-compile-pipeline)): desugar has already lowered components
into rails and chrome, minted label wires and capsule declarations, and
emitted the scoped look rules; the engine then **seats** satellites
(pin-relative, auto-posed), computes **cluster** extents, sizes and fills
the **tracks**, absolutizes satellites, and hands every wire — with its
fixed ports — to the router. Junction dots are read off the routed geometry
and emitted as `|junction|` chrome. The scope's links stay ordinary routed
links and its children arrange in place — no subtree is consumed; only the
generated chrome (rails, readouts, tags, junctions) is new.

---
# Part III — Reference

Canonical, dense lookup. The narrative ([Parts I–II](#part-i--core)) teaches once; this
part is the authoritative tables — every property, the output, the pipeline, the grammar,
the errors — and never repeats the prose.

---

## 17. Property Ledger & Support

Every property is `name: value;` — dash-case, positional, space-separated values
([SPEC 3](#3-statements--the-label)). This section is the one place that answers **which
property works where.**

**A property applies everywhere by default; the exceptions are marked.** An exception is
always one of two kinds: **type-owned** — a property a primitive requires or reads
(`points` on `|line|`, `symbol` on `|icon|`, `skew` on `|slant|`) — or **layout-owned** — a
property an engine interprets (`cell` on a grid, `place` on a sequence note, `data` on a
chart).

**Validation is strict where the wearer is known, lenient where a class is
polymorphic** (messages in [SPEC 21](#21-errors)):

- An **unknown property name** is an **error**, everywhere — even in a class rule;
  no owner accepts it. The message suggests the nearest name.
- A known property **misused where its wearer is statically known** — an instance's
  own block, an element rule (`|box| { }`, `|-| { }`), an id rule, a descendant
  rule's tail, the root block — is an **error** with a contextual correction: `points` on a `|box|`,
  `cell:` off a grid, a box property on bare text, a layout's own surface used
  outside it ([SPEC 21](#21-errors)). A **layout-owned** property errors where
  its owning layout is statically known to be absent, and is inert otherwise.
- In a **`.class` rule** the CSS semantics hold: a property is **inert** on wearers
  that can't use it; it **warns** only when it is dead for *every* wearer, and a
  defined class no node wears warns too.
- A **malformed value** (wrong arity, out of range) is an **error**, wearer-independent.

**State marks** used below: **✓** built and honoured · **⌛** meaningful but not built, a
candidate ([SPEC 24](#24-deferred)) · **—** not applicable.

### The container × layout matrix

The high-signal grid: which **container / layout** property each engine honours. (Paint,
text, and box-model properties are universal to every node — the tables that follow.)

| Property | `flow` | `grid` | `tree` | `sequence` | `chart` | `pie` | `drawing` | `schematic` |
|---|---|---|---|---|---|---|---|---|
| `direction` | ✓ `row`/`column` | — | ✓ `+bilateral` | — | ✓ `+radial` | — | — | — |
| `gap` | ✓ spacing | ✓ spacing | ✓ generation × sibling | ✓ pitch / spacing | ✓ plot gutter | ✓ plot gutter | — (a mate reads its own — [SPEC 15.5](#155-mates--seating)) | ✓ track spacing |
| `gap-fill` | ✓ | ✓ | — | ✓ᵇ | — | — | — | — |
| `padding` | ✓ | ✓ | ✓ | ✓ᵇ | — | — | ✓ frames the sheet | ✓ |
| `align` / `justify` | ✓ | ✓ per-column | — | ✓ᵇ | — | — | — | — |
| `width` / `height` | ✓ (slack) | ✓ (slack) | ✓ a floor | ✓ (surplus distributed) | ✓ box size | ✓ box size | ✓ a floor | ✓ a floor |
| `columns` / `rows` / `cell` / `span` | — | ✓ | — | — | — | — | — | ✓ `columns` + ordinal `cell` ([SPEC 16.1](#161-placement--anchors--satellites)) |
| container paint (`fill` `stroke` `radius` `shadow` `opacity` `href`) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

**✓ᵇ** — honoured on the participant / frame **boxes' own content** (they are ordinary
boxes), but *not* by the sequence engine's placement of them on the time axis
([SPEC 11](#11-the-layout-model)). A `chart` / `pie` consumes its children into marks, so that
case does not arise — hence `—`.

A **`floorplan`** reads the `drawing` column — the same engine
([SPEC 15.11](#1511-floorplan--the-architectural-dialect)).

### Universal properties

Honoured on every drawn node, in every layout (a box; text takes the marked subset).

**Paint & stroke** ([SPEC 6](#6-paint-stroke--text), colour [SPEC 10](#10-colour-variables--expressions)):

| Property | Value | Default |
|---|---|---|
| `fill` | colour · `none` · gradient · `auto` | `--fill` (box) · `none` (block/line) · `--icon-fill` (icon) · `currentColor` (text) · `none` (root — the scene background, [SPEC 18](#18-svg-output)) |
| `color` | colour | inherits (`--text-color`) — text colour for the subtree |
| `opacity` | `0..1` | 1 |
| `stroke` | colour · `none` · gradient | `--stroke` (`--group-stroke` on group) |
| `stroke-width` | number | 2 (`\|group\|` and a sequence frame: 1) |
| `stroke-style` | `solid`·`dashed`·`dotted`·`wavy`·`center`·`phantom` | `solid` — `wavy` link-only by design; `center` / `phantom` on shapes and `\|line\|`s ([SPEC 15.7](#157-leaders-notes--line-conventions)) |
| `radius` | number | 0 (block/rect) · 8 (box/group) — rect + polyline join; on a hex / diamond / slant / poly an **error** ⌛ |
| `shadow` | `N` · `dx dy` · `dx dy blur` · `dx dy blur color` | off — tint `--shadow-color` |

**Text** — all **inherit** ([SPEC 6](#6-paint-stroke--text)); text-valid on a bare string:

| Property | Value | Default | Kind |
|---|---|---|---|
| `font-family` | ident · string · `--var` | `--font-family` | live |
| `font-size` | number | 15 — chrome derives from it: a link label 11∕15, a caption 12∕15 ([SPEC 6](#6-paint-stroke--text)) | baked |
| `font-weight` | `normal`·`medium`·`semibold`·`bold`·`400`·`500`·`600`·`700` | `medium` (500, `--font-weight`) | live — measured at the resolved weight ([SPEC 6](#6-paint-stroke--text)); another number is an **error**, arbitrary 100–900 ⌛ |
| `font-style` | `normal` · `italic` · `oblique` | `normal` | live |
| `text-transform` | `uppercase` · `lowercase` · `capitalize` · `none` | `none` | live |
| `text-decoration` | `underline` · `overline` · `line-through` · `none` | `none` | live |
| `text-shadow` | `dx dy blur colour` | — | live (numbers gain `px`) |
| `letter-spacing` | number | 0 | baked |
| `line-spacing` | number | 0 | baked |

**Box model & placement** ([SPEC 5](#5-the-box-model)):

| Property | Value | Default | Notes |
|---|---|---|---|
| `width` · `height` | number · `auto` | `auto` | border-box; a **floor**. `\|image\|` needs both. |
| `max-width` | number | — | caps an auto width; a `width` above it is invalid; text inside wraps to it ([SPEC 5](#5-the-box-model)). |
| `text-wrap` | `wrap` · `nowrap` | `wrap` | whether text breaks to honour `max-width`; inert without one ([SPEC 5](#5-the-box-model)). |
| `padding` | `N` · `v h` · `t r b l` | 0 (block) · 20 (box) | inner padding; places content. |
| `pin` | `none` · `center` · edge · corner | `none` | out-of-flow anchor; a **box** property (not text). |
| `translate` | `x y` | — | post-placement nudge; **any** node incl. text. |
| `rotate` | degrees | 0 | turn about bbox centre; **any** node incl. text. |
| `layer` | integer | 0 (flow) · 1 (pinned) | paint order; ties → source order. |
| `scale` | number > 0 | 1 | the drafting **ratio** (`2` = 2 : 1) — nearest-wins; position scales by the parent, shape by self ([SPEC 15.1](#151-the-container-the-datum--the-scale)). |
| `pattern` | `grid(…)` · `radial(…)` | — | replicate about the node's position ([SPEC 15.4](#154-features-holes--patterns)). |
| `mirror` | axis list · `none` · `auto` | `auto` | reflect the node's path and features about the axis through its origin; `auto` reflects iff an ancestor does ([SPEC 15.3](#153-the-sketch-pen)). |

**Media & accessibility** — any node (`href` also a link):

| Property | Value | Notes |
|---|---|---|
| `href` | quoted URL | wraps the node / link in `<a href>` — clickable. |
| `hint` | quoted string | emits a `<title>` child (tooltip + screen-reader name). |

### Type-owned properties

Read on the listed primitive; required where noted ([SPEC 7](#7-nodes)).

| Property | On | Value | Notes |
|---|---|---|---|
| `points` | `\|line\|` `\|poly\|` | `x y, …` · parametric `u` expr | vertex list; **required**. |
| `samples` | `\|line\|` `\|poly\|`, chart `fn:` | integer | sample count (geometry default 2 — a straight segment; chart default 24). |
| `path` | `\|path\|` | quoted SVG path | **required**; native top-left coords. |
| `src` | `\|image\|` | quoted URL / data URI / local path | **required**; a local file embeds ([SPEC 7](#7-nodes)). |
| `symbol` | `\|icon\|` · `\|surface-finish\|` · `\|label\|` · the discretes · `\|door\|` and the floorplan fixtures (not `\|stairs\|`) | ident | Phosphor name, **required** (or via the label) · the finish vee variant — `basic`·`machined`·`prohibited`, default `basic` ([SPEC 15.9](#159-drafting-symbols--annotation-composition)) · a schematic symbol / variant ([SPEC 16.3](#163-discretes), [SPEC 16.4](#164-labels)) · a floorplan variant ([SPEC 15.11](#1511-floorplan--the-architectural-dialect)). |
| `fit` | `\|icon\|` `\|image\|` | `auto` · `contain` · `cover` · `stretch` | maps content into the box (size unchanged); `auto` default, `\|sign\|` `contain`. |
| `skew` | `\|slant\|` | degrees `(-89,89)` | 15. |
| `stack` | closed primitives | `N` · `dx dy` | offset duplicate behind. |
| `marker` · `marker-start` · `marker-end` | `\|line\|`, links | see [SPEC 7](#7-nodes) | endpoint / vertex glyphs; from the operator on a link. |
| `draw` | `\|sketch\|` | pen calls + `:segment`s | **required** ([SPEC 15.3](#153-the-sketch-pen)). |
| `revolve` | `\|sketch\|` | `x-axis` / `y-axis` | solid of revolution — fused fold + `\|shoulder\|` lines ([SPEC 15.3](#153-the-sketch-pen)). |
| `thread` | `\|sketch\|` `\|hole\|` round geometry | `seg pitch, …` · `pitch` | ISO 6410 thread dressing ([SPEC 15.3](#153-the-sketch-pen), [SPEC 15.4](#154-features-holes--patterns)). |
| `sheet` | `\|page\|` | `a5…a0` / ANSI `a…e` `[portrait \| landscape]` | trimmed-size sugar → `width` / `height` in mm ([SPEC 15.8](#158-assemblies-views-sheets--titles)). |
| `break` | `\|sketch\|` | `a b [axis]` groups | cut the view between stations ([SPEC 15.3](#153-the-sketch-pen)). |

### Grid, tree, chart, pie, sequence, drawing, floorplan & schematic properties

Layout-owned — an error only where a hard gate exists ([SPEC 21](#21-errors)); otherwise inert
out of scope.

| Property | Owner | Value | Default | Ref |
|---|---|---|---|---|
| `layout` | any container | `flow`·`grid`·`tree`·`sequence`·`chart`·`pie`·`drawing`·`floorplan`·`schematic` | `flow` | [SPEC 11](#11-the-layout-model) |
| `direction` | flow, chart, tree | `row`·`column` · `radial` (chart) · `bilateral` (tree) | `row` (flow) · `column` (a closed shape's or a `\|topic\|`'s card content, chart, tree) | [SPEC 11](#11-the-layout-model) |
| `gap` · `gap-fill` · `align` · `justify` · `padding` | flow, grid | — | see matrix (`gap` 36 in a flow, 12 in card content) | [SPEC 11](#11-the-layout-model), [SPEC 12](#12-flow-grid--tree) |
| `columns` · `rows` | grid · schematic (`columns` — its own ordinal tracks, [SPEC 16.1](#161-placement--anchors--satellites)) | track list | — (`columns` required on a grid) | [SPEC 12](#12-flow-grid--tree) |
| `cell` · `span` | grid box child; `cell` also a schematic's — ordinal ([SPEC 16.1](#161-placement--anchors--satellites)) | `col row` / `cols rows` | `— / 1 1` | [SPEC 12](#12-flow-grid--tree) |
| `data` · `fn` | chart series | list / pairs / `(…)` expr | — | [SPEC 14.3](#143-data--formulas) |
| `labels` | chart series | quoted-string list | — | [SPEC 14.3](#143-data--formulas) |
| `curve` | `\|line\|` `\|area\|` | `linear`·`smooth`·`step` | `linear` | [SPEC 14.2](#142-series) |
| `baseline` | `\|area\|` | number | axis zero | [SPEC 14.2](#142-series) |
| `axis` | series, `\|mark\|`, `\|band\|` | an `\|axis\|` id | — | [SPEC 14.4](#144-axes-scales--domain) |
| `bars` · `categories` · `samples` | `\|chart\|` | see [SPEC 14.1](#141-the-chart-plane) | `grouped` · indices · 24 | [SPEC 14](#14-charts) |
| `hole` | `\|pie\|` | `0` ≤ n < `1` | 0 | [SPEC 14.7](#147-direction-radial--pie) |
| `legend` · `tooltip` | `\|chart\|` `\|pie\|`, series (`tooltip`) | see [SPEC 14](#14-charts) | auto · auto | [SPEC 14](#14-charts) |
| `value` | `\|slice\|` `\|bubble\|` | number ≥ 0 | — | [SPEC 14](#14-charts) |
| `at` | `\|mark\|` `\|bubble\|` · `\|plane\|` · an opening (`\|door\|` / `\|window\|` — its station on `on:`'s segment) | `V` / `X Y` · `N [x-axis \| y-axis]` · `N` | — | [SPEC 14.5](#145-bands--annotations), [SPEC 15.8](#158-assemblies-views-sheets--titles), [SPEC 15.11](#1511-floorplan--the-architectural-dialect) |
| `side` · `range` · `scale` · `step` · `ticks` · `unit` · `gridlines` | `\|axis\|` (`range` also a `\|band\|`'s extent — [SPEC 14.5](#145-bands--annotations)) | see [SPEC 14.4](#144-axes-scales--domain) | — | [SPEC 14.4](#144-axes-scales--domain) |
| `format` | chart / drawing scope · `\|axis\|` · series · a dimension — **inherits** | `auto` (a chart tick to 4 decimals, a dimension to 2, zeros trimmed) · `decimal N` · `significant N` · `scientific N` · `engineering N` · `percent N` · `fraction D` · date preset (`year`·`month`·`day`·`hour`·`minute`) | `auto` | presentation only, never measurement; composes before `unit:`, `tol:`, the `⌀`/`R`/`°` glyphs, and `N×` counts ([SPEC 14.4](#144-axes-scales--domain), [SPEC 15.6](#156-dimensions)) |
| `side` (homonym: also an `\|axis\|`'s, above, and a dimension's, below) | first-level `\|topic\|`, `bilateral` | `left` · `right` | the split rule | [SPEC 12](#12-flow-grid--tree) |
| `place` | sequence `\|note\|` | `over` · `left` · `right`, then id(s) | — | [SPEC 13](#13-sequence) |
| `activation` | `\|sequence\|` | `auto` · `none` | `auto` | [SPEC 13](#13-sequence) |
| `scale` (homonym: an `\|axis\|`'s is `linear`·`log`·`time`) | any node | number > 0 | 1 | [SPEC 15.1](#151-the-container-the-datum--the-scale) |
| `unit` (homonym: an `\|axis\|`'s is its quoted tick suffix) | drawing scopes · `\|axis\|` | `mm`·`cm`·`m`·`in` — inherits | `mm` | [SPEC 15.1](#151-the-container-the-datum--the-scale), [SPEC 14.4](#144-axes-scales--domain) |
| `density` | the root | number > 0 | 4 | px per mm, screen/raster only ([SPEC 15.1](#151-the-container-the-datum--the-scale)) |
| `tol` | a dimension · a control row | `t` / `+u -l` / fit ident · number > 0 (a frame's zone width) | — | [SPEC 15.6](#156-dimensions), [SPEC 15.9](#159-drafting-symbols--annotation-composition) |
| `characteristic` | a control row (`\|control\|`, or a one-row `\|feature-control\|`) | the ISO 1101 ident set | — (the smart label) | [SPEC 15.9](#159-drafting-symbols--annotation-composition) |
| `zone` | a control row | `diameter` · `spherical` | — | [SPEC 15.9](#159-drafting-symbols--annotation-composition) |
| `material` | a control row | `maximum` · `least` | — | [SPEC 15.9](#159-drafting-symbols--annotation-composition) |
| `datums` | a control row | letters, each + optional `maximum` / `least` — ≤ 3 | — | [SPEC 15.9](#159-drafting-symbols--annotation-composition) |
| `modifiers` | a control row | `projected N` · `free-state` · `tangent-plane` list | — | [SPEC 15.9](#159-drafting-symbols--annotation-composition) |
| `side` | a dimension / callout (also `\|axis\|`, above) | side · corner · `left` / `right` along an aligned span | by axis | [SPEC 15.6](#156-dimensions) |
| `project` | a `(-)` dimension | `horizontal` · `vertical` · `aligned` | inferred | [SPEC 15.6](#156-dimensions) |
| `gap` | a mate | signed number — separation along the normal (a dimension stands off by `clearance` — [SPEC 21](#21-errors)) | — | [SPEC 15.5](#155-mates--seating) |
| `facing` | `\|plane\|` | `left`·`right`·`up`·`down` | by plane | [SPEC 15.8](#158-assemblies-views-sheets--titles) |
| `of` | `\|drawing\|` | a `\|plane\|` / `\|magnifier\|` id | — | [SPEC 15.8](#158-assemblies-views-sheets--titles) |
| ISO 7200 fields | `\|title-block\|` | quoted string | — | [SPEC 15.8](#158-assemblies-views-sheets--titles) |
| `thickness` | a floorplan scope · `\|wall\|` — **inherits**, nearest wins | number > 0 | 200 mm | [SPEC 15.11](#1511-floorplan--the-architectural-dialect) |
| `on` | `\|door\|` `\|window\|` | a straight wall `:segment`, bare | — **required** | [SPEC 15.11](#1511-floorplan--the-architectural-dialect) |
| `hinge` | `\|door\|` | `start` · `end` | `start` | [SPEC 15.11](#1511-floorplan--the-architectural-dialect) |
| `swing` | `\|door\|` | `left` · `right` | `left` | [SPEC 15.11](#1511-floorplan--the-architectural-dialect) |
| `steps` | `\|stairs\|` | integer ≥ 2 | — **required** | [SPEC 15.11](#1511-floorplan--the-architectural-dialect) |
| `number` | `\|pin\|` | integer | — | [SPEC 16.2](#162-components--pins) |
| `prefix` | `\|component\|` lineage, the discretes | quoted string | the type name (`\|component\|`: `"U"`) | [SPEC 16.2](#162-components--pins) |
| `shape` | `\|label\|` | `plain`·`left`·`right`·`both`·`round` | `plain` — a label wire's marker sets it | [SPEC 16.4](#164-labels), [SPEC 16.5](#165-wires) |
| `pins` | `\|J\|` | integer ≥ 1 | — | [SPEC 16.2](#162-components--pins) |
| `side` (homonym) | `\|pin\|` | `left`·`right`·`top`·`bottom` | the bilateral split | [SPEC 16.2](#162-components--pins) |
| `side` (homonym) | a `\|label\|` · a schematic **wire** | `left`·`right`·`top`·`bottom` | above a horizontal run, the freer side of a vertical one | which side of its trace a net name sits ([SPEC 16.4](#164-labels)) |

### Link properties

A link is styled like a node ([SPEC 9](#9-links)) — its wire takes `stroke*`, its labels the
text props. Its own properties:

| Property | Value | Default | Notes |
|---|---|---|---|
| `clearance` | number | 16 (a drawing's dimensions: 4; a schematic scope: 10) | min gap from nodes and links; a dimension's packing stand-off ([SPEC 15.6](#156-dimensions)). **Scene config** — cascades. |
| `routing` | `orthogonal` · `natural` · `straight` | `orthogonal` | wiring strategy; scene config, cascades ([ROUTING.md](ROUTING.md)). |
| `along` | fraction list | auto | label positions along the route. |
| `marker` · `marker-start` · `marker-end` | marker | from the operator | endpoint glyphs ([SPEC 7](#7-nodes)). |
| `corner-radius` | number · `auto` | `auto` — the clearance-derived cap | a wire's corner rounding radius; a schematic scope's link default is 0 ([SPEC 16.5](#165-wires)). |

---

## 18. SVG Output

```svg
<svg xmlns="http://www.w3.org/2000/svg"
     viewBox="X Y W H" width="W" height="H" class="lini lini-scope-HHHHHHHH">
  <style>
    @layer lini.defaults {
      :root, .lini-scope-HHHHHHHH { color-scheme: light dark; /* --lini-*: light-dark(…, …) */ }
      .lini-scope-HHHHHHHH[data-theme="dark"],  [data-theme="dark"]  .lini-scope-HHHHHHHH { color-scheme: dark; }
      .lini-scope-HHHHHHHH[data-theme="light"], [data-theme="light"] .lini-scope-HHHHHHHH { color-scheme: light; }
    }
    .lini-scope-HHHHHHHH { font-family: var(--lini-font-family); font-size: 15px; font-weight: var(--lini-font-weight); color: var(--lini-text-color); }
    .lini-scope-HHHHHHHH .lini-canvas { fill: #eef; }           /* only when the scene sets a background */
    .lini-scope-HHHHHHHH .lini-box { fill: var(--lini-fill); stroke: var(--lini-stroke); stroke-width: 2; }
    .lini-scope-HHHHHHHH .lini-style-hot { stroke-width: 3; }   /* one rule per class def */
    .lini-scope-HHHHHHHH .lini-link { stroke: var(--lini-stroke); stroke-width: 2; fill: none; }
  </style>
  <defs><!-- filters, gradients, clipPaths --></defs>
  <rect class="lini-canvas" .../>   <!-- …and then this plate, over the viewBox -->
  <g class="lini-scene"> <!-- scene tree --> </g>
  <g class="lini-links"> <!-- links --> </g>
</svg>
```

**A figure paints no background it was not given.** The `lini-canvas` plate — rect
*and* rule — is emitted only when the scene asks for one: a root `fill:` (a schematic
root's `--lini-sheet` wash rides exactly this), or `--static`, whose output is a
standalone document for renderers with no CSS variables and so carries its own opaque
`--lini-bg` backdrop. Otherwise there is no rect and no rule, so a figure inlined in a
page shows the page through it with nothing to override. `fill: --bg` on the root is
how a live figure asks for the themed backdrop; `fill: none` is that default said out
loud.

`viewBox` auto-sizes to content + the scene's `padding` (20 px by default) on every
side. When a file's drawn content
is only `|page|`s ([SPEC 15.8](#158-assemblies-views-sheets--titles)), the root `width`
and `height` carry the sheet's trimmed size in real **millimetres** rather than pixels,
so a print is true-scale; the `viewBox` is unchanged, so on-screen layout and CSS sizing
are not.

**Names are content-addressed.** Two figures inlined in one HTML document share its
id and selector spaces, so every name Lini writes into either comes from the **thing
it names**: a `<defs>` id from its definition, an asset's prefix from its bytes, a
glyph from its outline, the root's `lini-scope-HHHHHHHH` class from its stylesheet's
text. Figures then collide only on equal things, where sharing is correct —
`url(#…)` resolves to an equal def, a duplicate rule is a no-op. That class heads
every selector *in place of* `.lini` (one class either way, so specificity and host
overrides are unchanged); `lini` stays on the root as the host hook.

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

**Fonts — three output modes** ([SPEC 6](#6-paint-stroke--text)). By default the SVG
carries **names only** — zero font bytes; the stack leads with the bundled family
names, so an installed or hosted copy engages. **`--embed-font`** inlines a base64
`@font-face` per family × weight actually used, under **Lini-scoped family names**
(never colliding with a user's installed versions) — browser-faithful and
**browser-only by design**: resvg and librsvg ignore `@font-face`. **`--static`**
outlines text to paths — glyphs deduplicated through `<defs>` / `<use>`, italic as
synthetic oblique — and bakes the variables ([SPEC 10.6](#106---static)): faithful
in every renderer. Layout never varies by mode — measurement always reads the
compiled-in metrics tables ([SPEC 5](#5-the-box-model)).

**Embedded assets.** A local `|image|` ([SPEC 7](#7-nodes)) emits its resolved form:
an SVG asset nests as a child `<svg>` mapped into the node box (`fit:` sets its
`preserveAspectRatio`) — with **every id prefixed `lini-aHHHHHHHH-`** (a tag of the
asset's own bytes) and every internal reference rewritten to match (`url(#…)`
in attributes and inline `style`, fragment `href` / `xlink:href`), since nesting
alone does not isolate ids; a raster asset emits
`<image href="data:…;base64,…"/>`. Authored URLs and data URIs emit unchanged.
Embedding is deterministic from the asset bytes.

**Box:**

```svg
<g class="lini-node lini-{type} lini-{base} lini-style-{class}"
   data-id="ID" transform="translate(X,Y)">
  <title>…</title>            <!-- when `hint:` is set -->
  <!-- geometry, then children -->
</g>
```

Auto-classes: `lini-node` (every box); `lini-{name}` (the type and every type it
inherits, down to `lini-block`); `lini-style-{name}` (per worn class). With rotation,
the transform becomes `translate(X,Y) rotate(N)`.

**Text** emits a bare `<text class="lini-text">…</text>` at its placed position — no
wrapping `<g>`; a worn class joins it (`class="lini-text lini-style-quiet"`). A table's
cells are `|block|`s wrapping their text, so each renders as a
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
sequence's lowered primitives ([SPEC 19](#19-compile-pipeline)) emit exactly like the boxes,
text, and lines above — a chart's tooltip card is a `<g class="lini-chart-tip">`, a
reserved styling hook. Every generated dress is **one rule, never a `style=` per
wearer**, each rule emitted only when its role is actually worn, and an authored
class rule of the same name **replaces** the generated one (which is what lets
`|halo| { … }` restyle chrome scope-wide rather than layer under it). The hook
families:

| Family | Classes |
|---|---|
| core | `lini-node` · `lini-{type}` · `lini-style-{class}` · `lini-text` · `lini-canvas` · `lini-gutter` |
| link | `lini-link` · `lini-link-label` · `lini-link-dashed` / `-dotted` · `lini-stray` · `lini-marker` + `lini-marker-{kind}` (`arrow`·`dot`·`circle`·`diamond`·`datum`·`dim`·`open`) · `lini-cut` / `lini-cut-bg` (label mask) |
| chart | `lini-chart-title` · `lini-chart-label` · `lini-chart-tip` · `lini-tip-N` / `lini-hit-N` |
| sequence | `lini-sequence-tab` · `lini-sequence-guard` · `lini-sequence-message` |
| tree | `lini-level-N` · `lini-hue-{name}` (the mindmap walk) |
| drawing | `lini-dim-line` (dimension / leader linework) · `lini-ext-line` (`--lini-stroke-light`) · `lini-dim-text` (`font-size: 12; font-weight: normal` — no annotation leaf inlines its size) · `lini-dim` (the restyled `(-)` tier's compound, on dimension-owned chrome only) · `lini-frame-cell` / `lini-frame-plate` (GD&T) · `lini-plane-end` / `-shaft` / `-arrow` · `lini-drafting-glyph` · `lini-datum-frame` · `lini-halo` |
| floorplan | `lini-door-leaf` (a door's leaf, a slider's panels) · `lini-door-swing` (the quarter arc) · `lini-window-sill` · `lini-stair-tread` (a flight's risers) · `lini-stair-arrow` (its up arrow) ([SPEC 15.11](#1511-floorplan--the-architectural-dialect)) |
| schematic | `lini-schematic-wire` (a nested sheet's dress) · `lini-sch-line` / `-solid` · `lini-sch-tag-line` · `lini-tag-outline` / `-round` / `-flag-left` / `-flag-right` / `-flag-both` · `lini-net-run` / `lini-net-run-turned` (a plain label's run of trace, [SPEC 16.4](#164-labels)) · `lini-pin-stub` · `lini-pin-number` · `lini-ref` · `lini-part-value` |
| highlight | `lini-tok-{kind}` — a source **listing**'s token spans, not a figure's: `lini highlight` writes them and `lini highlight --css` paints them ([SPEC 20](#20-cli)) |
| marker | `lini-align-*` / `lini-justify-*` (a table column's carried alignment, [SPEC 8](#8-templates)) · `lini-side-left` / `-right` (which half of a bilateral tree a first-level topic fills, [SPEC 12](#12-flow-grid--tree)) · `lini-pose-90` / `-180` / `-270` (a schematic part's turn, consumed at lowering, [SPEC 16.1](#161-placement--anchors--satellites)) · `lini-carried` (an annotation node riding a drawing statement's `[ ]`, [SPEC 15.9](#159-drafting-symbols--annotation-composition)) |

The last family is the odd one out: its classes carry **structure, not paint**.
They emit no CSS rule and there is nothing in them for host CSS to restyle —
the engine reads them back off the chain, and they are listed so nothing a
figure emits is undocumented. Every other family above is a paint hook.

Generated **ids** are prefixed too — **every one, without exception** — each
tagged with what it names: `lini-aHHHHHHHH-` for embedded assets,
`lini-shadow-HHHHHHHH` / `lini-clip-…` / `lini-gradient-…` / `lini-hatch-…` /
`lini-label-cut-…` / `lini-halo-…` in `<defs>`, and `--static`'s **glyph defs**
`lini-g{kind}{weight}-{gid}` (an outline is equal whenever those three are, so
two figures share the def rather than collide). A detail view (`|drawing| { of: <magnifier> }`, [SPEC 15.8](#158-assemblies-views-sheets--titles))
clips to its region with one interned `<clipPath>` in `<defs>` and a `clip-path=` on
its group.

---

## 19. Compile Pipeline

A reference pipeline; implementations may differ if the observable output matches.

**Parse.** Lex to tokens, then a single recursive-descent pass to the AST. The
bracket-and-bars vocabulary (`|…|` identity, `{ }` style, `[ ]` content) resolves every
statement with one token of lookahead — no type-set prescan ([SPEC 22](#22-grammar)).

**Desugar.** Lower all surface sugar to primitives + classes — the engine's true
input. The pass is idempotent; type-system errors (cycle, depth > 16, a define
shadowing a built-in) surface here. What becomes explicit:

- *Types & rules:* each template / define instance becomes its base primitive
  wearing a `.lini-*` class chain (derived → base → primitive, down to `block`
  for every rectangular type); a type's defaults and any `|type| { }` element
  rule fold into a generated `.lini-<type> { … }` class; a `|table| |box| { }`
  descendant rule rewrites to `.lini-table .lini-box { }`, and `|-|` (the link
  type) to `.lini-link` — the class every link wears; define bodies inline per
  instance.
- *Scene config:* the scene defaults (`layout`, `padding`, `gap`, `font-size`,
  `clearance`, `routing`, `density`) settle on the root; a drawing (or
  floorplan) scope's `scale:` (the ratio) × `unit:` × the root `density:` fold
  into its one internal px-per-unit, and a floorplan scope's `unit:` is stamped
  for its types' mm defaults to convert through where each is read
  ([SPEC 15.1](#151-the-container-the-datum--the-scale), [SPEC 15.11](#1511-floorplan--the-architectural-dialect)).
- *Statements:* the per-type smart label (text / caption / symbol / link label /
  chart title …); auto-`along:`; chain expansion (`a -> b -> c` →
  `a -> b; b -> c`, auto-created ids included — fan-out `&` stays a resolve /
  routing concept; a schematic chain is the carve-out, cut only where its
  pass-through resolved, [SPEC 16.5](#165-wires)); a tree's branch links,
  `.lini-level-N` classes, and a mindmap's palette-walk rules
  ([SPEC 12](#12-flow-grid--tree)); link auto-create (an undeclared endpoint
  `x` → `|box#x| "x"`); and **capsule hoisting** (an endpoint capsule → a
  declaration at the statement's position + a reference, anonymous ones under
  minted `lini-cap-N` ids — [SPEC 9](#capsule-endpoints)).
- *Schematic lowerings:* pin rails, ref readouts and minted display refs,
  label-wire minting (`U7.DIAG - "NSTDBY"` → a `|label|` + its wire), and the
  scope's look rules ([SPEC 16](#16-schematic)).

**Resolve** (top-to-bottom):

1. *Variables, functions & rules:* merge visual-var defaults ← `--theme` ←
   `--name: value`; build the function table; compile the stylesheet's class / id /
   element / descendant rules. Parenthesized expressions and function calls fold to literal
   numbers / points ([SPEC 10.7](#107-expressions--functions)).
2. *Scene tree:* each box is a primitive wearing `.lini-*` (type) and user classes;
   layer properties per the [cascade](#4-selectors-cascade--specificity); lift internal
   links; build the path index. A `|table|`/`|entity|`'s structure settles here, once
   its `columns:` has: the first row becomes the header band, the table's
   `align`/`justify` carry onto its cells, and an entity's `|header|`/`|footer|`
   take their full-width span ([SPEC 8](#8-templates)).
3. *Links:* resolve endpoints by scoped path walk with suggestion errors; merge link
   properties through the link's ladder ([SPEC 4](#4-selectors-cascade--specificity));
   cartesian-expand fan groups into one resolved link per pair; the
   operator's line sets `stroke-style` unless overridden.

**Layout** (bottom-up): leaf bbox from `width`/`height` or defaults (text → its glyphs;
box → content + `padding`; + half-`stroke-width` per side); arrange flow children per
`layout` / `direction` honouring `align`/`justify`/`stretch`/`evenly` when there is slack; pin
out-of-flow children to their parent anchor (the parent never grows for them); compute
gutters; apply `padding`; apply each node's `translate`; `rotate` last. A **layout-owning**
container — `sequence` ([SPEC 13](#13-sequence)), `chart` / `pie`
([SPEC 14.9](#149-lowering)), and `drawing` / `floorplan` ([SPEC 15.10](#1510-lowering),
[SPEC 15.11](#1511-floorplan--the-architectural-dialect)) — instead
reads its whole subtree here and lowers it to primitives, **consuming its own links**,
so the router never sees them.

**Route links.** Per [`ROUTING.md`](ROUTING.md) — orthogonal, clearance-respecting,
deterministic — over every link **except** those a `drawing` scope already drew;
a sequence's messages route `straight` with layout-fixed anchors.
Place markers (sized `max(5, stroke-width × 4) + 1`, tip on the endpoint) and link labels at
their `along:` fractions (auto-distributed when unset).

**Render.** Depth-first emit SVG per [SPEC 18](#18-svg-output): a box is a `<g>`, a string is a
`<text>`. A lowered chart / sequence subtree renders as ordinary primitives
([SPEC 11](#11-the-layout-model), seam 3).

---

## 20. CLI

```
lini [options] <input.lini>
lini fmt [--check] [--stdout] <input.lini>
lini desugar <input.lini>
lini highlight [--css] <input.lini>
lini serve [--port N] [--static] [--theme NAME|FILE|A/B] [PATH]
lini theme [NAME]
```

| Flag | Meaning |
|---|---|
| `-o FILE` | Output path (default stdout). |
| `--format svg\|html` | `svg` (default) or HTML wrapper. |
| `--check` | Parse + validate + resolve only — layout/render errors still surface on a full compile. (`fmt --check` is that subcommand's own flag — below.) |
| `--port N` | `lini serve` only — the preview port (default 7700). |
| `--json` | Emit diagnostics as a JSON document (stable codes, severity, spans, related spans, machine-applicable fixes — [SPEC 21](#21-errors)) instead of SVG; the tooling/LSP form. Exit 1 if any error-level diagnostic fired. |
| `--theme NAME\|FILE\|A/B` | A built-in theme (`dark`, `high-contrast`, …), a CSS file of `--lini-*` overrides, or a light/dark pair (`light/dark`). |
| `--no-warn` / `--strict` | Silence warnings / treat them as errors. |
| `--static` | Inline `var()`s as literals **and** outline text to paths — self-contained for any renderer ([SPEC 10.6](#106---static), [SPEC 18](#18-svg-output)). |
| `--embed-font` | Embed the used bundled family × weights as base64 `@font-face` — browser-only ([SPEC 18](#18-svg-output)). Both font flags need the default-on `font` build feature; name-only output never does. |
| `--watch` | Recompile on every input change (requires `-o`). |
| `-h`, `-V` | Help / version. |

`lini -` reads stdin (filename `<stdin>` in errors). **`lini serve`** runs a local live
preview (default port 7700): a `.lini` file live-reloads that one file; a directory (or
no path → the current directory) opens the **playground** — pick, edit, and render any
`.lini` file beneath it in the browser. A served compile reads **image assets**
([SPEC 7](#7-nodes)) under the same boundary that confines the file list: the served
root — a file target's root is its directory — and an asset path escaping it is a
compile error; a plain `lini` compile is unbounded (you compile your own file). **`lini theme`** lists the built-in themes;
**`lini theme NAME`** prints one as a `--lini-*` CSS file — a ready starting point for
your own (`light-dark()` colours, the font commented out).

**`lini fmt`** reformats to canonical style — 2-space indent, `key: value;`
declarations grouped on one line, a style-only node collapsed onto its head line when it
fits (`|box#api| { fill: red }`), a lone label trailing the head (`|box#api| "API"`),
children one per line in `[ ]`, table cells padded into aligned columns — a styled
cell (`"Apple" { color: --red-ink }`) keeps its block and its **row steps out of the
aligned grid**; unstyled rows stay aligned — a `draw:`
value broken before each `move()` and wrapped between calls at the column limit
(continuations indented, so a profile reads as its subpaths), comments and
blank lines preserved. `--check` exits 1 if it would change anything; `--stdout` writes
instead of rewriting.

**`lini desugar`** prints the file fully **lowered to primitives** — the Desugar pass
([SPEC 19](#19-compile-pipeline)) that is the engine's true input — so the lowered form
re-renders byte-identically. A chart's or sequence's *type* desugars here (a `|chart|`
is a `|block|` wearing `.lini-chart`); its geometric primitive subtree is a layout-phase
artefact ([SPEC 19](#19-compile-pipeline)), like a routed link's geometry. A
teaching/debugging view; prints to stdout, never rewrites, comments not preserved.
A `|table|`'s header band and its per-column alignment are likewise **not**
shown: both are decided from the *resolved* `columns:`, which a class, a
descendant or id rule, or a user template can set ([SPEC 8](#8-templates)), so
they are a cascade-phase artefact — the structure desugar does show (each cell
in its `|cell|`, an entity's label as its title `|header|`) is the part that
needs no column count.

**`lini highlight`** prints the file as `<span class="lini-tok-…">` HTML — the
one syntax highlighter, at a shell. It is **lexical**: it never parses, so a
file mid-edit still colours and the only failure left is I/O; and it is
**byte-preserving** — strip the tags, undo the four entity escapes, and the
source comes back exactly, which is what lets a host drop the output into a
`<pre>` and trust the listing. Newlines pass through as newlines (a caller that
cannot carry one rewrites them itself). The classes are the token kinds, under
the reserved prefix like every other name Lini writes into a host document
([SPEC 18](#18-svg-output), [SPEC 23](#23-reserved-words)): `lini-tok-` +
`comment` · `string` · `number` · `const` · `keyword` · `type` · `type-user` ·
`prop` · `prop-user` · `var` · `op` · `class` · `punct`. The words behind them
come from the same source the editor grammars do ([SPEC 22](#22-grammar)), so a
new type or property colours the moment it has a ledger row.

**`lini highlight --css`** prints the **token palette** those spans wear —
nine `--lini-tok-*` role variables as `light-dark()` pairs, then the rules that
paint the thirteen classes from them — so the markup and its colours come from
one place and a listing reads the same in a book, on a site, and in the
playground. The role defaults are layered (`@layer lini.defaults`), so a host
re-tints one by redeclaring its variable with no `!important`; the sheet sets no
`color-scheme`, leaving the light/dark choice to whatever the host has set on
the listing's ancestors.

The same scanner is `lini::highlight_html` to a crate and `highlight()` to the
browser build; a host that can link Rust should, and this subcommand is for the
one that cannot.

Exit codes: 0 success · 1 parse/resolution error or `--check` reformat needed · 2 I/O ·
3 invalid CLI.

---
## 21. Errors

Format: `filename:line:col: error: <message>` (LSP-compatible), compile-time, with a span.
`--strict` promotes warnings to errors; `--no-warn` silences them ([SPEC 20](#20-cli)).

Every diagnostic carries a **stable code** — a phase letter (`L`ex · `P`arse · `R`esolve ·
`V`alidate · la`Y`out · rou`T`e) then a 3-digit number, e.g. `V001`. Codes are stable once
assigned; the message may still improve. **The implementation's diagnostic
registry is the authority for code assignment** — this section's tables and
their ordering carry no codes and never will. The human form above stays code-free; `lini --json`
([SPEC 20](#20-cli)) emits the structured record — code, severity, span, related span, and a
machine-applicable replacement where one exists.

**What the `--json` document freezes.** `{ "file", "diagnostics": [ … ] }`, each
entry `code` · `family` · `severity` (`error` / `warning`) · `message` ·
`span`, then `related` and `suggestion` (`span` · `replacement` ·
`applicability`) where the diagnostic has them; every span carries
`start` · `end` (byte offsets) and 1-based `line` · `col` · `endLine` ·
`endCol`. A tool may rely on **that shape and those codes**; a `message`
may be reworded to read better, and a later release may add fields — never
rename or drop one. A clean file emits an empty `diagnostics` array, not an
error.

**Lexing**

| Condition | Message |
|---|---|
| Unclosed string | `unterminated string literal` |
| Bad number | `invalid number literal` |
| Bad escape | `invalid escape sequence '\X'` |
| Stray character | `unexpected character 'X'` |
| Operator outside a group | `math operators appear inside ( ) — e.g. padding: (8 * 2)` |

**Properties & validation** ([SPEC 17](#17-property-ledger--support)'s strict/lenient rule)

| Condition | Message |
|---|---|
| Unknown property name | `unknown property 'colr'; did you mean 'color'?` |
| Misused property, wearer known | `'points' has no meaning on '\|box\|' — it reads on '\|line\|' / '\|poly\|'` · `'cell' places a grid or schematic child — this box sits in a 'layout: flow'` |
| Property dead for every wearer | `'.hot { cell: … }' is inert on every wearer` (warning) |
| Class defined, never worn | `class '.hot' is never worn` (warning) |
| Malformed value | `'opacity' is a fraction 0..1` · `'translate' takes 'x y'` · `'padding' takes one value, not a comma list` · `'wavy' waves a link's wire — a shape's outline takes solid, dashed, dotted, center, or phantom` |
| Legacy space-separated list | `'data' takes comma-separated values — 'data: 9, 15, 24'` ([SPEC 2](#2-lexical-syntax)) |
| Deferred property | `'legend' is named but not built yet — see SPEC 24` — a named-but-unbuilt row ([SPEC 24](#24-deferred)) errors, so accepting it can never freeze the non-behaviour |
| Property on a link that has no link meaning | `'routing' is a scope's strategy — one scope, one strategy; set it on the container` (`clearance:` **is** a link's, [ROUTING.md](ROUTING.md)) |
| `radius` on a non-rect primitive | `'radius' rounds a rect or a polyline join — rounding a '\|hex\|' is deferred` ([SPEC 24](#24-deferred)) |
| Arbitrary numeric `font-weight` | `'font-weight' takes normal, medium, semibold, bold, or 400, 500, 600, 700` (100–900 is deferred) |
| Gradient in a text-colour slot | `'color' takes a flat colour — a gradient fills a shape, and gradient-on-text is deferred` |
| `%` outside a colour component | `'width' takes a number — a '%' is a colour component` ([SPEC 2](#2-lexical-syntax)) |

**Identity, cascade & statements**

| Condition | Message |
|---|---|
| Duplicate id | `duplicate id 'X' (previously at L:C)` |
| Unknown type / class | `unknown type 'X'` / `unknown class '.X'` |
| Inheritance cycle / depth | `cycle in 'X → … → X'` / `'X' exceeds max inheritance depth (16)` |
| Define shadows builtin | `'X' shadows a built-in type` |
| Empty bars | `'\| \|' needs a type or an '#id'` |
| Invalid id | `'#123' is not a valid id — an id starts with a letter or '_'` |
| Reserved id prefix | `an id may not begin 'lini-' — the prefix is reserved for generated names` |
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
| Auto-create near-miss | `'cta' auto-creates a new box; did you mean 'cat'?` (warning — edit distance ≤ 2 or case-fold vs names known in scope, [SPEC 3](#implicit-nodes)) |
| Chain mixes operators | `link chain mixes operators 'X' and 'Y'` |
| Chain < 2 nodes | `link requires at least two endpoints` |
| Bare `o` marker | `'-o' needs a max glyph — write '-o<' or '-o+'; the hollow ring is an ER component only` |
| Missing required property | `'\|line\|' requires 'points'` |
| `->` in the stylesheet | `'->' draws a link on the canvas — style every link with '\|-\| { stroke: … }' in a '{ }' block` |
| `\|-\|` / `\|link\|` as an instance | `a link is drawn by an operator — '\|-\|' only styles links (write 'a -> b')` / `links are drawn by operators, not the '\|link\|' type` |
| `\|node\|` as instance | `'node' is the umbrella concept — write '\|block\|' for the bare box` |
| Unknown routing strategy | `routing takes orthogonal, natural, or straight — 'curved' was replaced by 'natural'` |
| Unknown side | `':X' is not a side — use top, bottom, left, or right` |
| Link labels split | `keep a link's labels together — write 'a -> b [ "x" "y" ]'` (warning) |
| Capsule endpoint in a drawing | `a drawing never invents an endpoint — declare the node, then annotate it` |
| Pin path on an inline component | `'\|component#U9\|.p4' — an inline component has no authored pins` |

**Values, colour & expressions**

| Condition | Message |
|---|---|
| Invalid / out-of-range color | `invalid color 'XYZ'` / `rgb(300,0,0): component out of range` |
| Invalid `oklch()` | `oklch expects (L, C, H) or (L, C, H, A) — L and A in 0..1, C ≥ 0, H in degrees` |
| Gradient with < 2 stops | `gradient() needs at least two colour stops` |
| `linear-gradient` without an angle | `linear-gradient needs an angle first, then ≥ 2 colour stops — e.g. linear-gradient(135, --teal, --sky)` |
| Single-quoted string | `single quotes are not strings — use "…"` |
| Unquoted text value | `'title' takes a quoted string — write title: "…"` |
| Invalid `pin` value | `'pin' expects none, center, an edge (top/bottom/left/right), or a corner (e.g. 'top right')` |
| Negative container `gap` | `a container's 'gap' must be ≥ 0` — a **mate's** `gap:` may go negative ([SPEC 15.5](#155-mates--seating)) |
| `skew` out of range | `skew: N must be in (-89, 89)` |
| Unknown name in an expression | `unknown name 'foo' in an expression` |
| Function arity | `'sin' takes 1 argument(s), got 2` |
| Spaced call paren | `a call's '(' glues to its name — write 'rgb(…)'` |
| `hatch()` off `fill` | `'hatch' is a fill — 'stroke' takes a colour or gradient` |
| Unreadable image path | `cannot read image './logo.svg' — no such file` ([SPEC 7](#7-nodes)) |
| Asset escapes the served root | `'../secret.svg' resolves outside the served root` ([SPEC 20](#20-cli)) |

**Layout — grid**

| Condition | Message |
|---|---|
| Missing `columns` | `'layout: grid' requires 'columns'` |
| Empty / bad track | `'columns' needs at least one track` / `a track is a size, 'auto', or repeat(N[, size])` |
| Grid out of range | `cell: 5 _ exceeds columns=3` |

**Layout — wrap** ([SPEC 5](#5-the-box-model))

| Condition | Message |
|---|---|
| `nowrap` text can't fit | `text cannot fit 'max-width: 80' without wrapping — widen it or drop 'text-wrap: nowrap'` |
| Non-text child wider than the cap | `a child is wider than 'max-width: 80' — only text wraps` |
| `width` above `max-width` | `'width: 200' exceeds 'max-width: 120'` |

**Layout — tree** ([SPEC 12](#12-flow-grid--tree))

| Condition | Message |
|---|---|
| `\|topic\|` outside a tree | `'\|topic\|' builds a tree — it belongs in a 'layout: tree'` |
| No root topic | `a tree needs exactly one root '\|topic\|'` |
| A second root topic | `a tree has one root — '\|topic\|' 'X' is a second` |
| `side:` top/bottom in `bilateral` | `a bilateral tree grows left and right — 'side' takes left or right` |
| `side:` in `row` / `column` | `'side' picks a bilateral branch's half — this tree has one growth direction` |
| Unknown `direction:` | `unknown direction 'radial' — a tree grows column, row, or bilateral` (a ring-radial tree is deferred, [SPEC 24](#24-deferred)) |

**Layout — sequence**

| Condition | Message |
|---|---|
| Sequence node outside a sequence | `'\|loop\|' belongs in a 'layout: sequence'` (same for `\|opt\|` / `\|alt\|`; a `\|note\|` is core — [SPEC 8](#8-templates)) |
| `\|else\|` outside an `\|alt\|` | `'\|else\|' separates an '\|alt\|' — write it inside one` |
| `\|note\|` in a sequence, no placement | `a sequence '\|note\|' needs 'place:'` |
| Bad `place:` | `'place' is a mode then its lifelines — 'place: over api db', 'place: left api'` |
| Sequence property off a sequence | `'place' is valid only in a 'layout: sequence'` (same for `activation`) |

**Layout — chart & pie**

| Condition | Message |
|---|---|
| Series / axis / band / mark outside a chart | `'\|bars\|' is a chart series — it belongs in a 'layout: chart'` · `'\|axis\|' belongs in a 'layout: chart'` |
| `\|slice\|` outside a pie | `'\|slice\|' belongs in a 'layout: pie'` |
| Pie given an axis or series | `a pie's children are '\|slice\|' only` |
| A `\|slice\|` with children | `a '\|slice\|' is one wedge — multi-ring pie / sunburst is deferred` ([SPEC 24](#24-deferred)) |
| Empty chart / pie | `a chart needs at least one series` / `a pie needs at least one '\|slice\|'` |
| Series with both / neither `data:` `fn:` | `a series takes 'data' or 'fn', not both` / `a series needs 'data' or 'fn'` |
| `arrow` / `crow` marker on a series | `'marker: arrow' has no centred form on a chart — use dot, circle, or diamond` |
| `fn:` list ≠ band count | `'fn' has N formulas but the chart has M bands` |
| Data ≠ categories count | `series data has N values but the chart has M categories` |
| `labels:` count ≠ data count / on `fn:` | `'labels' has N entries but the series has M data points` / `'labels' needs explicit 'data'` |
| `categories:` + an axis text | `set 'categories' or an axis 'labels', not both` (⌛ — reachable once per-axis tick text lands, [SPEC 24](#24-deferred)) |
| `\|mark\|` without `axis:` / bad `at:` | `a '\|mark\|' needs 'axis:' to place it` / `'at' takes one value (a line) or two (a point)` |
| `\|bubble\|` missing `at:` / `value:` | `a '\|bubble\|' needs 'at:' (x y) and 'value:'` |
| Unknown `axis:` id | `axis 'X' not found` + `; did you mean 'Y'?` |
| `range:` bad / equal ends | `'range' takes two ends: 'a b', 'a auto', or 'auto b'` / `'range' needs distinct ends` |
| `scale: log` over a non-positive domain | `a 'scale: log' axis needs a domain above 0` |
| Paint list count ≠ data count | `'fill' lists N paints but the series has M data points` |
| Paint list on `\|line\|` / `\|area\|` | `a '\|line\|' is one shape with one paint — per-datum lists read on '\|bars\|' / '\|dots\|'` |
| Mixed date / numeric domain | `the x axis mixes dates and numbers — one domain, one kind` |
| Invalid date literal | `'2026-13-01' is not a date — ISO-8601: '2026-01-31', optionally 'T09:30' and 'Z'` |
| Numeric `step:` on a time axis | `a time axis steps by calendar — 'step: month', 'step: 2 week'` |
| Bad `format:` value | `'format' takes auto, decimal N, significant N, scientific N, engineering N, percent N, fraction D, or a date preset` |
| `side:` in `direction: radial` | `'side' has no meaning in a radial chart — it has one radius axis` |
| `\|band\|` / `\|mark\|` in `direction: radial` | `a radial chart draws no bands / marks yet — remove it or change 'direction'` ([SPEC 24](#24-deferred)) |
| `hole:` out of range | `'hole' is a fraction 0..1` |
| Negative slice value / pie total zero | `a '\|slice\|' value must be ≥ 0` / `a pie's slice values sum to zero` |

**Layout — drawing** ([SPEC 15](#15-drawing))

| Condition | Message |
|---|---|
| `\|sketch\|` without `draw:` | `'\|sketch\|' requires 'draw'` |
| `\|hole\|` / `\|pitch-circle\|` / `\|magnifier\|` without `width:` | `'\|hole\|' requires 'width' — its diameter` |
| Unknown pen call / arity | `unknown draw call 'X'` / `'arc' takes (dx, dy, r) or (r, deg)` |
| `fillet` / `chamfer` off a corner | `'fillet' modifies the corner between two segments` |
| Floating `:segment` | `a ':segment' glues to its call — name a station with point():v` |
| Bare `point()` | `'point()' names the pen's position — attach a ':segment'` |
| Arc radius too small | `arc radius N is smaller than half the chord` |
| Bad `mirror:` item | `'mirror' takes x-axis, y-axis, a bearing, or none` |
| Axis `mirror:` on `\|path\|` / `\|image\|` | `'\|path\|' has no reflection — draw it with the pen` |
| Bad `break:` group | `'break' takes two stations 'a b' — a < b — and an optional x-axis / y-axis` |
| `break:` off a sketch | `'break' cuts a '\|sketch\|' — draw the profile with the pen` |
| `break:` station off the profile | `'break' at N misses the profile` |
| Overlapping `break:` groups | `'break' spans overlap — merge them` |
| `break:` through a cubic | `a 'break' can't cut a 'curve()' — move the stations` ([SPEC 24](#24-deferred)) |
| Drawing statement outside a drawing | `'(-)' draws a dimension — it belongs in a 'layout: drawing' (or its 'floorplan' dialect)` (same for `(o)`, `(<)`, `\|\|`, corner anchors, `tol:`, …) |
| Unknown endpoint | `dimension endpoint 'X' not found at <scope>` + suggestions — **never auto-created** |
| Corner order | `':right-top' is not an anchor — did you mean ':top-right'?` |
| `(>)` | `'(>)' is reserved — the angle op is '(<)'` |
| One-ended `(-)` / `\|\|` | `a linear dimension measures two anchors` / `a mate seats two parts` |
| Two-ended `(o)` | `'(o)' measures one round feature — write 'a:top (o)' for a span` |
| Empty one-ended leader | `a leader needs its text — 'bolt <- "THRU"'` |
| One-ended `->` / `-*` | `a leader points back at its feature — write 'a <- "…"'` |
| Bare `(o)` with no axis | `'(o)' can't pick an axis on 'X' — anchor a side ('X:top (o)') or a segment` |
| `(<)` on a point anchor | `an angle reads two edges — a named segment, a '\|line\|', or a side` |
| Unary `(<)` on an unmirrored name | `'(<)' on ':taper' needs 'mirror:' — no twin to measure against` |
| Station `⌀` on a mirror-only profile | `a station '⌀' reads a revolved profile — 'revolve: x-axis'` |
| `revolve:` + `mirror:` together | `a sketch takes 'revolve:' or 'mirror:', not both` |
| Bad `revolve:` value | `'revolve' takes x-axis or y-axis` |
| Bad `thread:` group | `'thread' takes a segment and its pitch — 'thread: m8 1.5'` |
| `thread:` without `revolve:` | `'thread' dresses a revolved profile — add 'revolve: x-axis'` |
| `thread:` segment off-axis / not straight | `'thread' runs along the axis — 'm8' must be a straight run parallel to it` |
| Unknown `thread:` segment | `no segment 'm8' in this 'draw:'` + suggestions |
| `thread:` on a non-round node | `'thread' dresses a '\|sketch\|' segment or a round feature` |
| Bad `sheet:` | `'sheet' takes a size — a5…a0 (ISO) or a…e (ANSI) — and an optional portrait / landscape` + did-you-mean |
| `of:` finds no marker | `'of' finds no marker 'X'` |
| `of:` names a non-marker | `'of' names 'X', not a '\|plane\|' or '\|magnifier\|'` |
| Detail of a sourced view | `a detail magnifies a base view — 'of' can't name a marker inside another sourced view` |
| `at:` off the model | `a 'plane' at N sits off the model` |
| Bad `facing:` | `'facing' turns the arrows — left, right, up, or down` |
| Marked projection op | `a projection line is unmarked — write 'side.screw:head - end.od:top'` |
| Projection ends in one view | `a projection link ties two views — both ends read 'side'` |
| Projection end off a view | `a projection link ties drawing anchors — 'notes' is not in a drawing view` |
| Cross-view dimension / mate | `a dimension reads one view — a cross-view correspondence is a construction link ('a - b')` |
| Authored cell on a generated field | `cell 2 1 is taken by the generated 'Rev' field — place it after the fields` |
| `:segment` shadows a built-in point | `':left' is a built-in anchor — pick another name` |
| Unknown `:segment` | `no segment ':step' on 'body'` + suggestions |
| Duplicate `:segment` in one `draw:` | `':step' is already named in this 'draw:'` |
| Label on a mate | `a mate takes no label` |
| `gap:` on a point mate | `a point mate coincides — 'gap' needs directed anchors (sides or named edges)` |
| Non-parallel mate directions | `mated anchors must face along one axis — 'a:left \|\| b:top' has no shared normal` |
| Over-constrained mate | `mate over-constrains 'X' — already positioned via 'A \|\| B'` |
| Mate within one part | `'a' and 'b' are features of one part — a part is rigid` |
| Perpendicular directed pair | `'a:left (-) b:top' — perpendicular faces have no shared normal; the angle between edges is '(<)'` |
| `project:` vs a directed anchor | `'project: vertical' conflicts with 'a:left' — the directed anchor reads horizontal` |
| Unknown copy index | `no copy 'bolt.5' — the replication places 4` |
| Duplicate datum letter | `datum 'A' is already placed (previously at L:C)` |
| Drafting type outside a drawing | `'\|feature-control\|' annotates a drawing — it belongs in a 'layout: drawing' (or its 'floorplan' dialect)` (same for `\|surface-finish\|`, `\|control\|`, `\|datum\|`) |
| Unknown characteristic | `unknown characteristic 'flatnes'; did you mean 'flatness'?` |
| Characteristic set twice | `a control's characteristic is its label or 'characteristic:', not both` |
| Unknown finish variant | `'symbol' picks the vee — basic, machined, or prohibited` |
| Control row without `tol:` | `a control row needs 'tol' — its zone width` |
| Mixed frame forms | `a frame is one row or '\|control\|' rows — not both` |
| `\|control\|` outside a frame | `'\|control\|' is a '\|feature-control\|' row` |
| `datums:` on a form control | `'flatness' is a form control — it takes no datum` |
| Missing required datum | `'circular-runout' measures against a datum — name one in 'datums:'` |
| Unknown datum reference | `no datum 'D' in this drawing — declared: A, B` |
| Too many datums | `'datums' orders primary, secondary, tertiary — three at most` |
| `zone:` off an axial control | `'zone: diameter' has no meaning on 'flatness' — its zone is a width, not an axis` |
| `material:` off a feature-of-size control | `'material' modifies a feature-of-size control — position, orientation, or straightness` |
| Unknown modifier | `'modifiers' takes projected N, free-state, or tangent-plane` |
| Point-target seat | `a seat needs a face — anchor a side or a named edge ('sf \|\| plate:top')` |
| Seat with no geometry end | `a seat stands an annotation on geometry — 'sf \|\| n1' seats nothing` |
| Annotation seated twice | `'sf' is already seated (previously at L:C)` |
| Annotation node on a routed link | `a routed link's '[ ]' holds text labels — annotation nodes ride a drawing's dimensions and leaders` |
| `&` fan on a measuring op / mate | `'&' fans one-ended leaders — chain dimensions instead ('a (-) b (-) c')` |
| `gap:` on a dimension | `a dimension stands off by 'clearance' — 'gap' is a mate's separation` |
| `side:` off-axis | `a horizontal dimension stacks on top or bottom` / `a vertical dimension stacks on left or right` |
| Parallel `(<)` edges | `the angle's edges are parallel — they never meet` |
| Bad `tol:` | `'tol' takes a number, '+upper -lower', or a fit ident` |
| Bad `pattern:` | `'pattern' takes grid(cols, rows, dx, dy) or radial(count, radius)` (name **and** arity) · `'radial' needs count ≥ 2 and radius > 0` |
| `scale:` ≤ 0 | `'scale' must be > 0` |
| `scale:` on a `\|page\|` | `a '\|page\|' carries no 'scale:' — 'density:' sets its pixels per millimetre (root), a drawing's 'scale:' its drafting ratio` |
| `density:` ≤ 0 | `'density' must be > 0` |
| Absurd rendered extent | `the drawing renders 48000 px wide — 'scale:' is a ratio; a 5 m beam at 1:50 is 'scale: 0.02'` (hint) |
| Bad `unit:` / `density:` off the root | `'unit' is mm, cm, m, or in` / `'density' is scene config — set it in the root block` |
| Chain past a label | `a text callout ends its statement — chain before it` |
| Mate in a flow scope | `a '\|row\|' places its own children — mates seat a drawing's` |
| Empty drawing | `a drawing needs at least one geometry child` |

**Layout — floorplan** ([SPEC 15.11](#1511-floorplan--the-architectural-dialect))

| Condition | Message |
|---|---|
| Floorplan type outside the scope | `'\|wall\|' belongs in a 'layout: floorplan'` (every floorplan type) |
| `on:` an unknown / curved segment | `'sout' is not a segment of this wall; did you mean 'south'?` · `an opening sits on a straight run — ':bay' is an arc` |
| An opening off its segment | `'d2' at 1.8 + width 0.9 overruns 'side' (length 2.3)` |
| Overlapping openings | `'entry' and 'w1' overlap on 'south'` |
| An opening outside a wall's `[ ]` | `a '\|door\|' rides in its wall's '[ ]'` |
| `translate:` on an opening | `an opening sits at 'on:' / 'at:' — move the station, or nudge the wall` |
| `curve()` in a wall's `draw:` | `a wall bends with 'arc()' — 'curve()' has no offset` |
| `hinge:` / `swing:` on a sliding door | `a sliding door has no leaf to hang — remove 'hinge:' / 'swing:'` |
| A wall segment authored `*-in` / `*-out` | `':north-in' collides with the derived face anchor — rename the segment` |
| An arc wall tighter than its thickness | `arc radius 40 is under thickness/2 — the inner face vanishes` |
| Missing `on:` / `steps:` | required-property errors, as `points` on a `\|line\|` |

**Layout — schematic** ([SPEC 16](#16-schematic))

| Condition | Message |
|---|---|
| Schematic type outside the scope | `'\|R\|' belongs in a 'layout: schematic'` (every schematic type) |
| `:side` on a terminal | `a terminal owns its connection — a pin or label takes no ':side'` |
| Non-90° rotation on a connection-bearing part | `a schematic part rotates in 90° steps — 0, 90, 180, or 270` |
| Pinless wire to a 3+-pin part | `'U7' has 21 pins — name one ('U7.VS')` |
| Both pins of a 2-pin part taken | `both pins of 'R5' are wired — name one ('R5.p1')` |
| Minted ref as an endpoint | `link endpoint 'R1' not found — a minted ref is display-only; give the part an id to wire it` |
| Marker on a part-to-part wire | `a schematic wire is plain — markers shape a text label's tag; write 'a - b'` |
| Marker at a symbol-form label | `'\|gnd\|' draws its symbol — there is no tag to shape` |
| Bare unknown id in the scope | `'NSTDBY' is unknown — a schematic never invents a box; did you mean '- "NSTDBY"' (a net label)?` |
| Duplicate wire | `'a - b' is already wired — a repeated wire means nothing on a sheet` |
| Dot-path into a label | `a label is its own terminal — it has no pins` |
| Unknown schematic `symbol:` | `unknown symbol 'gnb'; did you mean 'gnd'?` |
| Bad `shape:` / `pins:` / `number:` | `'shape' takes plain, left, right, both, or round — not 'X'` · `'pins' takes a count ≥ 1` · `'number' takes an integer` |
| Satellite chain with no placed end | `'C7' has no placed end — its chain falls back to the flow` (warning) |

**Routing** — a stray's reasons ([ROUTING.md](ROUTING.md), Impossible layouts / Fixed ports); each is a warning naming the link, `--strict` an error

| Condition | Message |
|---|---|
| Closed graph | `no legal route: every side entry or channel is closed at this layout` |
| Blocked fixed port | `fixed port blocked: a body covers the port's landing` |
| Crowded fixed ports | `fixed ports closer than the minimum pitch on one side` |
| Conflicted fan | `fan ends carry two different fixed ports` |
| Pinned self-loop | `self-loop with both ends forced onto one side` |


---

## 22. Grammar

```
file        = [ stylesheet ] { drawn }              # setup block, then drawn statements in source order
stylesheet  = "{" { setup_item } "}"                # the root's setup block; omit when empty
setup_item  = decl | vardecl | binding | rule | define | comment | newline
drawn       = node | text | link | comment | newline   # instances and links interleave; a sequence reads order as time (SPEC 13)

decl        = ident ":" values ";"                  # ';' optional before '}'
vardecl     = css_var ":" values ";"                # --name : value ;
binding     = ident [ "(" [ ident { "," ident } ] ")" ] "=" value ";"  # my_r = 5 ; scale(n) = (…) ;
rule        = selector style                        # |box| { } , |table| |box| { } , .hot { } , #hero { }
define      = "|" ident "::" ident "|" body         # name :: base, optional children

node        = ident_bars [ string ] [ classes ] [ style ] [ children ]
text        = string [ classes ] [ style ]          # bare content; a styleable leaf, never a box
ident_bars  = "|" ( type [ "#" ident ] | "#" ident ) "|"   # |type| , |type#id| , |#id|
type        = ident
classes     = "." ident { "." ident }               # a worn class chain — .hot, .hot.loud

style       = "{" { decl } "}"                       # declarations only
children    = "[" { node | text | link } "]"         # nodes, text, links — in source order
body        = [ style ] [ children ]                 # define / container body

link        = endpoints op [ endpoints ] { op endpoints }
              [ string ] [ classes ] [ style ] [ label_block ]   # the node tail, on a link head
op          = link_op | draw_op
draw_op     = "||" | "(-)" | "(o)" | "(<)"          # mate, linear, round, angle (SPEC 15)
selector    = sel_unit { sel_unit }                 # whitespace-separated = descendant
sel_unit    = ident_bars | "|-|" | "(-)" | "." ident | "#" ident  # a type(+id), the link type, the dimension type, a class, or an id
endpoints   = endpoint { "&" endpoint }
endpoint    = ( ident | ident_bars ) { "." ident } [ "." index ] [ ":" point ]   # a capsule declares (SPEC 9)
index       = digit+                                 # a 1-based pattern copy — drawing
                                                     #   scope only (SPEC 15.4)
point       = "top" | "bottom" | "left" | "right"    # + corners, center, authored segments
                                                     #   in a drawing scope (SPEC 15.2)
pen_item    = call [ ":" ident ]                     # a draw: item — a pen call, optionally
                                                     #   naming its product (point(): a station)

label_block = "[" { text | node } "]"                # canonical labels — styleable text leaves;
                                                     #   a node among them is a drawing
                                                     #   annotation (SPEC 15.9)

values      = value_group { "," value_group }        # comma only between list items
value_group = value { value }                        # space-separated scalars
value       = number | percent | string | hex | ident | css_var | call | group
call        = ident "(" [ expr { "," expr } ] ")"    # a call; each argument is an expr
group       = "(" expr ")"                           # a math group — a number or point (SPEC 10.7)
css_var     = "--" ident { "-" ident }
expr        = { ident "=" expr ";" } value_expr [ "," value_expr ]  # locals, then a value or a point
value_expr  = operators, math library, a ternary, calls, groups — the grammar of SPEC 10.7

link_op     = [ start_marker ] line [ end_marker ]
line        = "-" | "--" | "---" | "~"
start_marker = "<" | ">" | "*" | "<>" | card_start
end_marker  = "<" | ">" | "*" | "<>" | card_end   # ER cardinality, either side (SPEC 9)
card_end    = [ "o" | "+" ] ( "+" | "<" )         # [min][max] — min (o/+) hugs the line, max (+/<) outer
card_start  = ( "+" | ">" ) [ "o" | "+" ]         # the mirror — max (+/>) outer, min (o/+) hugs the line

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
one token of lookahead enough — the first token of every statement tells its kind (a
leading capsule resolves node-vs-link on the single token after its closing bar,
[SPEC 1](#1-mental-model)):
in the stylesheet, `|…|` → a rule or (with an inner `::`) a define, `.name` → a class rule,
`#name` → an id rule, `--name :` → a variable, `ident :` → a root declaration, `ident =`
or `ident (…) =` → a binding; after it, a
drawn statement is a `node` (`|…|`), `text` (`"…"`), or — when a bare `ident` is followed by
a link-op, `&`, or a `.` path — a `link`. A **declaration** ends with `;` (its value may
span lines); a **statement** ends at a newline or `;`.

**The comma law rides `values`.** `value_group { "," value_group }` is the whole
mechanism: a comma between repeated list items, a space between one item's
components, pipelines (`draw:`, `mirror:`) one space-separated group
([SPEC 2](#2-lexical-syntax)). The parser preserves the shape; each **list
reader** enforces it with a targeted correction — a legacy space list errors as
`` `data` takes comma-separated values — `data: 9, 15, 24` ``.

**Adjacency tells a `.class` from a path; a `:` tells a side.** A space before the `.`
makes it a worn class (`a .hot`), no space an endpoint path (`a.b`); the first class is
spaced from the identity, the rest of the chain glues (`.hot.loud`); a `:` after an
endpoint forces a side (`a:left`), distinct from the declaration `:` by position.

**Every layout reuses this grammar; drawing extends it, schematic adds one
relaxation.** Charts and sequences add
**no** lexer or parser grammar — they are nodes, declarations, and children, distinguished
by type name and by the scope's `layout` ([SPEC 13](#13-sequence), [SPEC 14](#14-charts)).
The `drawing` layout ([SPEC 15](#15-drawing)) adds exactly seven things:

1. the four **`draw_op`** tokens — glued, like every link op; `||` is resolved in
   the parser from two **adjacent** pipes at **operator position only**, so bars
   stay paired and selectors are untouched;
2. the **one-ended relaxation** — the right-hand endpoints may be omitted for
   `<-`, `*-`, `>-`, `(<)`, and **must** be for the unary-only `(o)`; the binary
   `(-)` and `||` require both ends. (Meaning only in a schematic scope, the
   wire ops `-` `->` `-<` `-<>` `-*` may stand one-ended before a string or
   capsule — the label wire of [SPEC 16.5](#165-wires).) One token of lookahead
   decides: after the op, an ident or a `|` opens an endpoint (bars: a capsule);
   a string, `.`, `{`, `[`, or end-of-statement is the tail;
3. the widened endpoint **`point`** set in drawing scope;
4. the numeric **copy `index`** in an endpoint path — `plate.bolt.2`; the lexer
   glues `.` + digits in endpoint position only, so `1.5` in value position
   stays a number;
5. the `(-)` **dimension-family `sel_unit`** at a stylesheet statement head — a
   leading `(` there is unambiguous, calls and groups appearing only in value
   position;
6. the annotation **node** among a `label_block`'s labels — parsed everywhere,
   *meaning* only on a drawing's dimensions and leaders: a core routed link's
   `[ ]` stays text-only and a node there errors at resolve,
   [SPEC 15.9](#159-drafting-symbols--annotation-composition);
7. the **`pen_item`** form inside a `draw:` value.

A call's `(` **glues to its name**; a free-standing `(…)` is a math group and a
free-standing `(-)`, `(o)`, or `(<)` an op ([SPEC 2](#2-lexical-syntax)). The pen
calls, `grid` / `radial`, and `hatch` are **call names**, contextual before `(`
like `rgb` / `repeat` ([SPEC 23](#23-reserved-words)).

---

## 23. Reserved Words

Because a type only ever appears in bars (`|box|`) and an id always wears a `#`, **type
names are free as ids and ids are free as type names** — `|block#oval|` is fine, and
`block -> oval` is two ordinary nodes. A small set of words stays reserved:

- **`node`, `link`,** and the structural class names **`text`, `marker`, `canvas`,
  `scene`, `cut`:** not instantiable types — `node` is the umbrella concept (write
  `|block|` for the bare box), links are drawn by operators and styled by `|-|` (`|link|`
  is an error), and a **define** may not take one of these (its generated `.lini-<name>`
  would collide with a built-in SVG class — `|-|` lowers to the reserved `.lini-link`).

The **`lini-` prefix** is reserved for generated names: desugar generates the type
classes (`.lini-block`, `.lini-box`, `.lini-<define>`) and mints ids
(`#lini-topic-N` — [SPEC 12](#12-flow-grid--tree)), so a user class or an authored id
may not begin `lini-`. User classes are emitted `.lini-style-<name>`. The
highlighter's token classes (`.lini-tok-<kind>` — [SPEC 20](#20-cli)) take the
same prefix for the same reason: they land in a *host's* document, where an
unprefixed name would collide.

The side names **`top`, `bottom`, `left`, `right`** are **not** reserved — they are
keywords only after an endpoint's `:` (`a:left`), so a node may be named `|box#left|`.
Single quotes (`'`) are reserved and are not strings.

Value keywords are **contextual**, not reserved as ids — `flow`, `grid`, `tree`,
`sequence`, `chart`, `pie`, `row`, `column`, `radial`, `bilateral`, `start`, `center`,
`end`, `stretch`, `evenly`, `none`, `auto`, `orthogonal`, `natural`, `straight` mean
their keyword only after the property that
expects them. **Every built-in type** — the primitives ([SPEC 7](#7-nodes)),
the templates ([SPEC 8](#8-templates)), and each layout's own types
([SPEC 13](#13-sequence)–[SPEC 16](#16-schematic)) — is protected from a
define shadowing it, free as an id.
Function names `rgb`, `rgba`, `hsl`, `repeat` are reserved only before `(` — as are
`hatch`, `grid` / `radial` (in `pattern:`), and the pen calls (`move`, `left`, `right`,
`up`, `down`, `line`, `angle`, `arc`, `curve`, `fillet`, `chamfer`, `circle`, `close`)
inside a `draw:` value.

In **link-operator position** the marker glyphs `+` (one) and `o` (zero) are contextual —
they compose the ER cardinality marker ([SPEC 9](#9-links)) and mean nothing elsewhere;
`o` is valid only next to a max glyph (`-o<`, `+o-`, …), so it never collides with an id or
the round measuring op `(o)` (delimited by parens). A leading `+` not followed by a digit
starts a cardinality op, mirroring `-`. The digit `0` is **not** part of any operator — a
round endpoint is `marker-end: circle` (a larger *filled* dot,
[SPEC 7](#7-nodes); the hollow ring exists only inside the ER glyphs),
never `-o`.

Inside a `(…)` expression ([SPEC 10.7](#107-expressions--functions)), `pi`, `e`, and the
sample parameters `u` / `x` are keywords, and the math-function names (`sin`, `exp`,
`min`, …) are reserved before `(` — all contextual to the expression, free as ids
elsewhere. The backtick `` ` `` is unused and reserved.

---

## 24. Deferred

Named in the language, not built yet; the syntax is stable.

Every item below whose syntax is **reachable today is an error** — never silently
accepted, never silently dropped ([SPEC 21](#21-errors)). That is what keeps each
one a free option: a refusal can be relaxed in any later release, a quiet
acceptance could not. The repo's `tests/deferred.rs` pins one test per reachable
slot, in this section's order, so the two can be diffed item by item.

**Core**

- **flow / grid callouts** — the one-ended leader (`a <- "THRU"`) is a drawing's
  ([SPEC 15.7](#157-leaders-notes--line-conventions)); the same shape in a flow or
  a schematic is an error.
- **balloon-capsule leaders** — an inline capsule endpoint
  (`p -> \|note\| "x"`) in a drawing, which never invents an endpoint
  ([SPEC 15](#15-drawing)).
- **fractional / `fr` grid tracks** — a track is a size, `auto`, or
  `repeat(N[, size])`; equal tracks are `repeat(N)` ([SPEC 12](#12-flow-grid--tree)).
- **gradient fills on text** — gradients fill nodes today ([SPEC 10.3](#103-gradients)).
- `radius` on non-rect primitives (hex / diamond / slant / poly).
- arbitrary numeric `font-weight` (100–900 beyond the built 400–700 set) and
  **kerning-aware measurement** — the metrics ship without shaping (≈ 1 % on a
  proportional line, [SPEC 5](#5-the-box-model)).
- **bidirectional text (RTL)** — measurement and outlining walk a string in the
  order it was written: no Unicode bidi reordering, no shaping, and no `dir`
  knob to ask for either. A live figure hands its `<text>` to the renderer,
  which may reorder an Arabic or Hebrew run visually — against a box measured
  without having done so; `--static` outlines the glyphs in written order, so
  the run reads left-to-right and unjoined. There is nothing to refuse here
  (the syntax reserves no surface for it), and nothing built: a right-to-left
  scene is not supported today, and full bidi + shaping can land whole in any
  later release.
- a solid (`fill`-weight) icon variant (the built-in set is Phosphor duotone,
  behind a default-on `icons` cargo feature).
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

- `legend:` placement / suppression (`top` · `right` · `bottom` · `none`) — the auto
  legend (≥ 2 entries) is built.
- **bands / marks in `radial` charts** — a compile error today ([SPEC 21](#21-errors));
  `column` and `row` are built.
- explicit per-axis tick text — `categories:` covers the x axis today (the series'
  per-datum text is `labels:`, [SPEC 14.3](#143-data--formulas)).
- **gauge** (a partial arc for one value); **stacked areas** (`bars: stacked` extended to
  `|area|`); polar-area **circular gridlines** and a configurable radial **start angle /
  direction** (the polygon web and top-clockwise are the defaults).
- per-slice **explode**, **on-slice value / percent labels**, and a **centred total** in a
  donut hole; **per-segment styling** (a style list mirroring a segmented `fn:`).
- **multi-ring pie / sunburst**.

**Drawings** ([SPEC 15](#15-drawing))

- **per-kind dimension selectors** — `(o) { }` / `(<) { }`; the family selector `(-) { }`
  reaches every dimension today ([SPEC 4](#4-selectors-cascade--specificity), [SPEC 15.6](#156-dimensions)),
  and a leader-specific selector under `|-|` is deferred too (YAGNI).
- **`explode:`** — scale every directed mate's separation along its normal for exploded
  views; unmated overlaid children stay put (overlay composes one part, mates relate
  parts — only relationships explode). Balloons follow their parts.
- **authored-segment twins** — a `mirror:` copy of a `:segment` is unaddressable
  (the name reads the drawn original; the unary mirrored readings cover the
  turned-profile cases; a `pattern:` copy is addressed by index —
  [SPEC 15.4](#154-features-holes--patterns)).
- **routed links to authored anchors** — the fixed-port routing contract is
  built ([ROUTING.md](ROUTING.md), Fixed ports — schematic pins ride it);
  the flow / grid *surface syntax* `a -> b:port` onto a sketch's authored
  `:segment`s remains deferred.
- **repeated-segment counting** — one `:segment` on several corners auto-prefixing `4× R3`,
  as `pattern:` does for features; today, type it.
- **hole variants** — counterbore and countersink (threads are built — `thread:`,
  [SPEC 15.3](#153-the-sketch-pen), [SPEC 15.4](#154-features-holes--patterns)).
- **deeper sourced-view nesting** — a detail of a marker inside another detail /
  section is gated ([SPEC 21](#21-errors)); projection construction links between
  views are built ([SPEC 15.8](#158-assemblies-views-sheets--titles)).
- **angled break lines** and a scope-level `break:` on the `\|drawing\|` itself; a
  `break:` station **through a `curve()`** (lines and arcs clip exactly today — move the
  stations off the cubic) and `break:` on non-sketch geometry (draw the profile with
  the pen).
- the ASME **text-in-a-broken-line** diametral form and a horizontal-text knob
  (ISO aligned is the built-in; crossing halos are built —
  [SPEC 15.7](#157-leaders-notes--line-conventions)).
- an ambient **`w` / `h`** bound to a node's own size (circular against auto-sizing
  today — a named constant covers the workflow, [SPEC 10.7](#107-expressions--functions)).
- **balloon auto-numbering and auto-BOM** from the scene's parts.
- **`\|mark\|` / `\|note\|` in charts** — data-coordinate placement (`at:`).

**Floorplans** ([SPEC 15.11](#1511-floorplan--the-architectural-dialect))

- **computed room areas** — a room polygon read off the wall topology, its area the
  smart label; today a room name and area are authored sheet text.
- **openings on curved segments** — straight runs only today (an arced wall itself
  is fine).
- a **north arrow / scale bar** type (a `\|sketch\|` define covers it today).
- more built-in fixtures (wardrobes, counter runs with an inset sink, door
  casings / thresholds) — the `\|sketch\|`-define parts library is the escape.

**Schematics** ([SPEC 16](#16-schematic))

- **wire-seating** — placing a series chain's parts along the routed wire;
  today capsule chains hoist as adjacent flow siblings and satellites seat
  at pins.
- an **ANSI symbol standard** knob (scope-level, swapping the whole family;
  IEC is the built-in).
- logic gates; transformer (`T`), relay (`K`), motor (`M`), speaker (`LS`),
  potentiometer (`RV`); crossing **hop-over** arcs; **buses**; pin
  electrical marks; hierarchical sheets; netlist semantics; a mid-wire tag
  riding a link's `[ ]` at an `along:` fraction.

**Beyond 1.0** — directions deliberately outside the release contract, listed so
they reserve no premature syntax: automatic graph / DAG layout (multi-parent,
cycles); a true ring-radial tree and forest (multi-root) trees
([SPEC 12](#12-flow-grid--tree)); **view-letter arrows** on sheets (`of:` an arrow
marker composing "VIEW A (2:1)" — an arrow defines no capture, so it is title sugar
over a view's smart label; construction links are built,
[SPEC 15.8](#158-assemblies-views-sheets--titles)); imports / modules / namespaces
for shared themes and part libraries; animation; native PNG / WebP export.
(The **`blueprint` theme** — white linework on cyanotype blue, any diagram —
shipped as a `--theme` builtin, [SPEC 20](#20-cli); a floorplan's default
stays black-on-white.)

---

## 25. Examples

One worked example per family; the full per-feature gallery is the repo's
`samples/` directory (one tested `.lini` file per feature — tables and entities,
gradients, icons, every chart kind, the drawing set — tie bar with `break:`,
bushing section, mated pump assembly, patterns, details, sheets — and a floorplan
studio).

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

**A tree and a mindmap** ([SPEC 12](#12-flow-grid--tree)):

```
|column| "Org" { layout: tree; } [        // plain tree: neutral, orthogonal
  |topic#ceo| "CEO" [
    |topic#cto| "CTO" [ |topic| "Backend"; |topic| "Frontend" ]
    |topic#coo| "COO" [ |topic| "Ops" ]
  ]
]

|mindmap#plan| "Launch" [                 // preset: bilateral, natural curves,
  |topic#product| "Product" [             // palette walk, depth ramp, 160 wrap
    |topic| "MVP"; |topic| "Docs"
  ]
  |topic#sales| "Sales" { side: left; } [ // overrides the ⌈n/2⌉ split
    |topic| "Leads"
  ]
]
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
|note| "rate-limited" { place: over api db }
```

**Charts — bars, a formula with a band, and a pie:**

```
|chart| "Cycle time (s)" { categories: "15 cm³", "30 cm³", "50 cm³" } [
  |bars| "1.8 kW" { data: 9, 15, 24; fill: --sky }
  |bars| "2.3 kW" { data: 7, 13, 20; fill: --amber }
]

|chart| "Injection profile" [
  |axis#bar| "Pressure (bar)" { side: left; range: 0 1100 }
  |axis#x|   "Speed (mm/s)"   { side: bottom; range: 0 133 }
  |area| "Pressure" { axis: bar; fn: (x <= 93 ? 1000 : 1000 - 319*((x-93)/40)); fill: --teal }
  |band| { range: 93 133; axis: x; fill: --red }
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
|chart| "Profiles" { direction: radial; categories: "Speed", "Range", "Armor", "Cost", "Stealth" } [
  |axis| { range: 0 5 }
  |line| "Scout"   { data: 5, 4, 2, 3, 5 }
  |area| "Cruiser" { data: 3, 3, 5, 4, 2; fill: --teal }
]

|chart| "Effort vs. score" [
  |axis| "tokens (k)" { side: bottom }
  |axis| "score %"    { side: left }
  |line| "GLM-5.2" { data: 35 63, 42 72, 84 75; labels: "Base", "High", "Max"; marker: circle; tooltip: always }
]
```

**A drawing — a sheeted screw, two views sharing an axis** ([SPEC 15](#15-drawing)):

```
|page| { sheet: a5 landscape; gap: 50; align: origin; } [    // landscape → direction: row
  // the ISO sheet: frame, zones, marks — views share their axes datum-to-datum

  |drawing#side| "DIN 912 — M8 × 40" { scale: 1.5; } [
    |sketch#screw| {
      draw: move(0, 0) up(6.5) chamfer(0.8) right(8):head down(2.5):k right(12)
            point():v right(28):m8 chamfer(1) down(4);
      revolve: x-axis;                           // a turned part
      thread: m8 1.25;                           // the threaded run
    } [
      |hidden#socket| {                          // the hex socket, dashed
        draw: move(0, 3) right(4) line(3, -3);
        mirror: x-axis;
      }
    ]
    screw:head (o) { side: left; }               // → ⌀13
    screw:left (-) screw:k { side: bottom; }     // → 8 — K, the head
    screw:k (-) screw:right { side: bottom; }    // → 40 — L, under the head
    screw:v (-) screw:right { side: top; }       // → 28 — the thread length
    screw:m8 <- { side: top; }                   // → M8×1.25 — composed by the thread
  ]

  |drawing#end| { scale: 1.5; } [
    |oval#od| { width: 13; height: 13; }
    |oval| { width: 11.4; height: 11.4; }        // the head, end-on
    |hex#socket| { width: 7; height: 6; }
    socket:left (-) socket:right                 // the socket, visible here
  ]

  |title-block| {
    title: "Socket cap screw"; drawing-number: "DIN 912 — M8 × 40";
    revision: "A"; sheet-number: "1/1"; date: "2026-07-08"; author: "AM";
  }
]
```

**A floorplan — a studio flat** ([SPEC 15.11](#1511-floorplan--the-architectural-dialect)):

```
{ layout: floorplan; unit: m; scale: 0.02 }        // 1 : 50 — thickness defaults 200 mm

|wall#outer| {
  draw: move(0, 0) right(7.2):north down(4.8):east left(7.2):south close():west;
} [
  |door#entry| { on: south; at: 3.0; swing: right }      // width: the 900 mm default
  |window#w1|  { on: north; at: 0.9; width: 1.8 }
  |window#w2|  { on: north; at: 4.5; width: 1.8 }
]
|partition#bathwall| {
  draw: move(4.9, 0) down(2.2) right(2.3):side;          // the bathroom corner
} [
  |door| { on: side; at: 0.6; hinge: end }
]

|bed|  { translate: 1.2 1.2; rotate: 90 }
|sofa| { symbol: corner; translate: 2.0 3.3 }
|bath| { symbol: toilet; translate: 5.5 0.5 }
|bath| { symbol: shower; translate: 6.6 1.6 }
|appliance| { symbol: fridge; translate: 4.2 0.5 }
"STUDIO 27 m²" { translate: 4.0 3.4 }

outer:west (-) outer:east { side: top }                       // → 7.2 — centreline to centreline
outer:west (-) outer.entry (-) outer:east { side: bottom }    // the door's location chain
```

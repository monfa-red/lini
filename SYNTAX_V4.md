# Lini Syntax v4 — CSS-shaped (draft)

A draft of the next syntax, not the final spec. The bet: **make Lini read like
CSS.** `key: value;` declarations in `{ }` blocks, dash-case property names,
space-separated values, real selectors. Keep only the handful of concepts CSS
has no word for. Clean break — pre-release, zero users, no aliases for old names.

This supersedes the earlier `SYNTAX_V2`/`V3` drafts.

---

## 1. Mental model

**The file *is* the root container's body.** There is no `|scene|` and no
wrapping block — bare declarations at the top configure the scene; rules and
nodes follow, in any order:

```lini
layout: grid;                  // scene config — bare declarations on the root
columns: 3;  rows: 2;  gap: 30;

rect { fill: #eee; radius: 6; } // a rule (stylesheet) — draws nothing
.hot { stroke-width: 2; }

server |rect| "Server"          // an instance (canvas) — drawn
client |rect| "Client"
server -> client "requests"     // a wire
```

Every line is one of two kinds — **scan the left edge:**

| Starts with | Kind |
|---|---|
| bars `\|type\|`, an **id**, or a `"label"` | an **instance** (on the canvas) |
| a bare **type name**, `name:base`, `.class`, or `--var` | a **stylesheet rule** (draws nothing) |
| `key: value;` | a **declaration** (configures the thing it's in) |
| `… -> …` | a **wire** |

So: **bars = canvas, bare names = stylesheet.** That one split removes every
"is this drawn or just styled?" ambiguity.

---

## 2. Statements

### The three type forms

| Form | Kind | Means |
|---|---|---|
| `\|rect\|` | instance | draw a rect (bars = canvas) |
| `rect { … }` | rule | style every rect (a CSS element selector) |
| `treat:rect { … }` | define | a new type `treat`, base `rect`, with its defaults |

A bare type name can only be a rule because **type names are reserved** — `rect`
can never be a node id. `treat:rect` reads "treat **is a** rect" (the inheritance
colon of C#/Kotlin/Swift/TS); its `{` block tells it apart from a `key:value;`
attribute. Defines chain (`panel:treat`) and may carry intrinsic children.

### Node declaration

```
[id] [|type|] ["label" …] [.class …] [ { block } ]
```

Everything is optional; the type defaults to `rect`. The block holds
declarations, child nodes, and internal wires, in any order.

```lini
db |cyl| "Postgres" .primary {
  fill: #eef;
  badge |rect| "v16" { mount: on; side: top; align: end; }
}
```

- **Omit the type** → `|rect|`. **Omit the id** → anonymous (can't be wired to).
- **Labels are positional strings.** A `|group|`'s 1st label is a **top
  caption**, its 2nd a **bottom footer** (both reserved `mount:in` bands); the
  rest are centred text. Every other shape just centres all its labels. There is
  no `|title|` — a caption is a `|text|` with `mount:in`.
- An empty label `""` suppresses it.

### Implicit nodes

A wire endpoint naming an id declared nowhere auto-creates an empty `|rect|` —
so `cat -> dog -> bird` is a complete three-box diagram.

---

## 3. Declarations (property reference)

Values are **space-separated and positional**, like CSS (`padding: 5 2 5 5`,
`shadow: 2 2 4 #0003`, `at: 100 50`). Commas separate only list items (`points`);
functions use parens (`rgb()`, `hsl()`, `repeat()`). Names are **dash-case**; a
bare group word is sugar for its obvious sub-property.

### Paint

| Property | Notes |
|---|---|
| `fill` | shape body colour (and the canvas, on the root). On `\|text\|` it's the glyph colour |
| `color` | inherited foreground for text + icon glyphs (cascades, like CSS) |
| `opacity` | 0..1 |
| `radius` | uniform corner radius |
| `rotate` | degrees |
| `skew` | slant degrees (`\|slant\|`) |
| `shadow` | `N` \| `dx dy` \| `dx dy blur` \| `dx dy blur color` |
| `stack` | offset duplicate of the shape, behind it; `N` → `N -N`, or `dx dy`. Shapes only |

### Stroke

| Property | Notes |
|---|---|
| `stroke` | outline/line/wire **colour** (sugar for `stroke-color`) |
| `stroke-width` | thickness |
| `stroke-style` | `solid \| dashed \| dotted \| wavy \| double` |

### Geometry & placement

| Property | Notes |
|---|---|
| `width`, `height` | bbox dims. **Set one → the other auto-sizes** to content/default. `\|image\|` needs both |
| `at` | `x y` — centre at absolute parent-local coords; removes from flow. (On a wire label, `at: 0.5` = fraction along the route — see §6.) |
| `side` | `top \| right \| bottom \| left` — which edge `mount` anchors to |
| `mount` | `none` (default, in flow) \| `in` \| `out` \| `on`. `in`/`out` reserve a band, `on` overlays. (was: `place`) |
| `align` | on a mounted child: position along its anchored edge |
| `offset` | `x y` — visual nudge after placement |
| `layer` | render order (ties break on source order) |
| `points` | `x y, x y, …` (`\|poly\|`, `\|line\|`) |
| `path` | raw SVG path string (`\|path\|`, native top-left coords) |

### Spacing

| Property | Forms |
|---|---|
| `padding` | `N` \| `v h` \| `t r b l`; longhands `padding-top`/`-right`/`-bottom`/`-left` |
| `margin` | same forms + longhands. Per-child outer spacing, **signed** (negative tightens) |
| `gap` | `N` (both) \| `row col` (CSS order); space between children |

### Layout

| Property | Notes |
|---|---|
| `layout` | `row \| column \| grid` |
| `align` | **cross axis** — `start \| center \| end \| stretch \| evenly` |
| `justify` | **main axis** — `start \| center \| end \| stretch \| evenly` |
| `divider` | `none` (default) \| `all` \| `rows` \| `columns` — separators between flow children, painted by `stroke*`. `all` adapts (1-D → the one axis); span-aware in grids; skips mounted children |

`align`/`justify` follow flexbox: `justify` runs *along* the flow, `align`
*across* it; default `center` for both. `stretch` fills the child's **box**
along that axis — it doesn't touch the child's content, which is placed by the
child's *own* `align`/`justify` (also `center`), so a stretched table cell
centres its text for free. `evenly` spreads children with equal gaps. All of
these are **no-ops unless the container is larger than its packed children**
(auto-sized containers have no slack), and `evenly` needs multiple tracks —
`justify` always, `align` only across a grid's rows, never a 1-D cross axis.

### Grid

| Property | Notes |
|---|---|
| `columns`, `rows` | a **track list** — sizes or `auto` (`columns: 80 140 80`, `rows: auto 40`); count = length. `repeat(N)` / `repeat(N, size)` for many equal tracks |
| `cell` | child placement `column row`, 1-indexed |
| `span` | child span `columns rows` (default `1 1`; `span: 2` = `2 1`) |

`columns`/`rows`/`cell`/`span` are only valid on a grid (`layout: grid` or
`|table|`); using them elsewhere is an error. Grid lines come from `divider`
(see Layout), not from the grid props.

### Text

| Property | Notes |
|---|---|
| `font-family`, `font-size` | |
| `font-weight` | `normal \| bold` |
| `font-style` | `normal \| italic \| oblique` |
| `text-align` | multi-line justification `start \| center \| end` (≠ `align`) |
| `line-height` | baseline-to-baseline multiple (default 1.2); a single line's box stays snug to the glyphs |
| `letter-spacing` | feeds width measurement |

`font-*`, `text-align`, `line-height`, `letter-spacing`, and `color` cascade to
descendant `|text|` — nearest ancestor wins, like CSS.

### Markers (on `|line|` and wires)

| Property | Notes |
|---|---|
| `marker`, `marker-start`, `marker-end` | `none \| arrow \| dot \| diamond \| crow` |

### Routing

| Property | Notes |
|---|---|
| `clearance` | wire keep-out from nodes & wires; set on `wire { }` or the root, **inherits** to every wire |

### Media & a11y

| Property | Notes |
|---|---|
| `src` | image source (`\|image\|`) |
| `link` | wrap this node or wire in `<a href>` |
| `icon-variant` | `outlined \| filled \| rounded \| sharp` (the glyph name is the `\|icon\|` label: `\|icon\| "home"`) |
| `title` | SVG `<title>` / tooltip |
| `aria-label` | emitted verbatim |

### Variables

`--name: value;` declares a themeable variable; `--name` in a value references
one. Built-in theme variables track these property names: `--lini-fill`,
`--lini-stroke`, `--lini-accent`, `--lini-font-size`, … Visual variables stay
live `var()`; layout values bake.

### Dropped

`size` (→ `width`+`height`), `origin` (was a no-op), per-corner radius, all
coordinate/box **tuples** (→ space lists), wire `at:start|mid|end` sugar (→ numeric
`at`), `row-gap`/`column-gap` (→ `gap`), `col-span`/`row-span` (→ `span`), the
positional href (→ `link`), `|title|` / `|cell|` (see §5–6), and the
`between`/`around` distributions (`evenly` stays).

---

## 4. Selectors & the cascade

A **rule** is `selector { declarations }`. Selectors are CSS-shaped:

```lini
rect { … }                  // every rect (element selector)
.hot { … }                  // every .hot (class selector)
table rect { … }            // every rect inside a table (descendant)
.sidebar rect { … }         // every rect inside a .sidebar
```

(In a selector the type is **bare** — `table rect`, never `table |rect|` —
because bars are only for instances.) A **define** is a rule whose selector
introduces a new type from a base: `treat:rect { … }`.

**Specificity** — most specific wins; ties break by source order (the CSS
cascade):

1. type rule (`rect { }`) and a type's own define defaults
2. descendant rule (`table rect { }`)
3. class (`.hot { }`)
4. the instance's own block (`client |rect| { fill: white }`) — wins

---

## 5. Layout, tables

`row`/`column` are 1-D flex; `grid` is 2-D. A **`|table|` is just sugar** — a
grid that draws dividers. Two things ship with the type:

```lini
table:group { layout: grid; divider: all; stroke: --stroke; }                   // the type
table rect  { stroke-width: 0; padding: 8; align: stretch; justify: stretch; }  // fill cells
```

So a table's outer frame is its group border and its inner lines are
`divider: all`, both painted by its `stroke*`; cells are borderless, so no edge
is ever doubled.

Cells are ordinary children that auto-flow into the tracks left-to-right,
wrapping at the column count; `cell:` pins one explicitly and the rest flow
around it. There is no `|cell|` type — a cell is the default `|rect|`. The
shipped `stretch` rule makes each cell **fill** its track; its text centres for
free because the cell's own `align`/`justify` default to `center`. A plain
`layout: grid` leaves its children at their natural size, centred in each cell.

```lini
basket |table| {
  columns: 80 140 80;
  rows: auto 40;

  "Fruit" { font-weight: bold; }  "Qty" { font-weight: bold; }  "Notes"
  "Apple"                         "12"                          "fresh"
  "Mango"                         "3"                           "ripe"
}
```

`fmt` aligns the cells into visual columns, so the flat form reads like a table
without a row construct. Style cells in bulk with `.my-table rect { … }`.

---

## 6. Wires

Operators and endpoints are unchanged from the current language.

```lini
a -> b                 // arrow         a -- b   dashed     a ~> b   wavy
a <-> b                // both ends      a -.- b  dotted     a -* b   dot
a -> b & c             // fan            a & b -> c          fan-in
a -> b -> c            // chain
a.right -> b.left      // forced sides
```

The op is `[start-marker][line][end-marker]`: line `-`/`--`/`-.-`/`~`, markers
`<` `>` `*` `<>` (the same glyph differs by end). Explicit `marker*` /
`stroke-style` attrs override the op.

**Wire labels** are `|text|` children — inline sugar `a -> b "label"` still
works. They ride the wire — no `mount`; `at` + `offset` place them:

| Property | Notes |
|---|---|
| `at` | `0..1` along the route; unset = auto-distribute across the hops |
| `offset` | `x y` in the route's tangent frame — lifts the label off the line |

```lini
cat.right -> kitchen.bowl.left {
  |text| "watches" { at: 0.5; font-size: 10; }
}
```

Routing (orthogonal, clearance-respecting, deterministic) is unchanged — see
`WIRING.md`.

---

## 7. Deliberately non-CSS

CSS has no word for these, so they stay Lini-specific:

- **`|type|`** — the pipe sigil for instances and `name:base` defines.
- **`fill` / `stroke`** — the SVG/design-tool pair (not `background`/`border`),
  because wires and open lines have a stroke but no border, and `fill`+`stroke`
  stay consistent across every shape.
- **`layout: row|column|grid`** — one word instead of `display` + `flex-direction`.
- **`divider`** — separators between flow children; `|table|` is `grid` + `divider: all`.
- **`mount` / `side`** — edge anchoring (captions, badges).
- **Wire operators**, **`marker*`**, **`clearance`** — routing.
- **`stack`** — the offset-duplicate effect.

Everything else aims to *be* the CSS property — same name, same value shape.

---

## 8. Grammar sketch

```
file        = { stmt }
stmt        = decl | rule | node | wire | comment
decl        = ident ":" values ";"
rule        = selector block
selector    = type | "." ident | type ":" type | selector ws selector   // descendant
node        = [ ident ] [ "|" type "|" ] { string } { "." ident } [ block ]
wire        = endpoints op endpoints { op endpoints } [ string ] [ block ]
endpoint    = ident { "." ident } [ "." side ]
block       = "{" { decl | node | wire | comment } "}"
values      = value { value }            // space-separated; "," only inside lists
```

`type` is a reserved or user-defined type name; the pipe form is an instance,
the bare form a selector. A `decl` ends at `;`/newline; a `rule`/`node` is told
apart by its `{`.

---

## 9. Deferred (named here, not built yet)

- `stroke-style: double` / `wavy` rendering on shapes.
- `radius` on non-rect shapes (hex/diamond/slant/poly).
- numeric `font-weight` (`100…900`).
- `|icon|` Material Symbols glyph embedding (currently a placeholder).
- embedded font metrics (text sizing is approximate until then).

---

## 10. Naming audit (the non-obvious calls)

- **`fill`, not `background`** — pairs consistently with `stroke` (Figma/SVG),
  works for wires/lines that have no "border," and is still valid CSS. *(If you'd
  rather optimize for web-CSS familiarity, `background` is the alternative.)*
- **`stroke`, not `border`** — open shapes, lines, and wires have a stroke, not a
  border.
- **`layer`, not `z-index`** — a plain render-order integer; Lini has no
  stacking contexts to justify `z-index`'s baggage.
- **`align` / `justify`, not `align-items` / `justify-content`** — same flexbox
  model, shorter.
- **`columns`/`rows` are always a track list** (`auto` for auto-sized,
  `repeat(N)` for many) — a bare number would read like a width; the list form
  has no such ambiguity.
- **`mount` (was `place`)** — renamed so it doesn't blur with `at`; reads with
  the values ("mounted *in* / *on* / *out* the `side` edge").
- **`at` does double duty** — node coords (`at: x y`) and a wire label's route
  fraction (`at: 0.5`); a node never has a route and a label never has coords,
  so it's unambiguous per element (and matches the current language).
- **`divider`** — one separator control for every layout, painted by `stroke*`;
  it's what lets `|table|` be plain `grid + divider: all` instead of a magic type.
- **`line-height`** kept (CSS-familiar) even though the single-line box is snug —
  the multiple still describes multi-line spacing.
- **`clearance` separate from `margin`** — routing keep-out (inherits) vs. layout
  spacing (per-node); different kinds.
- **`stack`** kept for the offset-duplicate; sits near `layer` but reads as "a
  stack of cards," a distinct idea from render order.

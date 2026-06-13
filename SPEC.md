# Lini — Language Specification (v3)

A small, human-readable language for plain-text diagrams. Flex/grid layout,
composable primitives, CSS-driven theming — compiles to clean SVG.

This document is complete: an implementer can build a conforming engine from it
alone. Wire **routing** has its own contract — see [`WIRING.md`](WIRING.md).

---

## Table of Contents

**Language** — 1 [Mental Model](#1-mental-model) · 2 [Lexical Syntax](#2-lexical-syntax) ·
3 [Sigils](#3-sigils) · 4 [Defs Block](#4-defs-block) · 5 [Node Declarations](#5-node-declarations) ·
6 [Layout](#6-layout) · 7 [Positioning & Anchors](#7-positioning--anchors) ·
8 [Primitives](#8-primitives) · 9 [Templates](#9-templates) · 10 [Wires](#10-wires)

**Reference** — 11 [Attributes](#11-attributes) · 12 [Variables & Defaults](#12-variables--defaults) ·
13 [Specificity](#13-specificity) · 14 [SVG Output](#14-svg-output) · 15 [CLI](#15-cli) ·
16 [Errors](#16-errors) · 17 [Grammar](#17-grammar-ebnf) · 18 [Implementer Algorithm](#18-implementer-algorithm) ·
19 [Reserved Words](#19-reserved-words) · 20 [Non-Goals](#20-non-goals) · 21 [Examples](#21-examples)

---

## Quickstart

```
cat -> dog -> bird
```

That's a complete diagram: three boxes, two arrows. Lini fills in the rest.

| Sigil | Means |
|---|---|
| `\|name\|` | A type — built-in or user-defined (`\|rect\|`, `\|group\|`). |
| `key:value` | An attribute. **No spaces around `:`** (`radius:5`). |
| `.name` | Apply a style (space before required: `cat .loud`). |
| `id.side` | A wire endpoint's side (no space: `cat.right`). |
| `--name` | A themeable CSS variable (`fill:--accent`). |

Three defaults make small diagrams trivial:

- Omit the type → `|rect|`.
- Omit the label → the node's id (`""` to suppress it).
- Name an undeclared id in a wire → it's auto-created as a `|rect|`.

A file is **one optional `{ defs }` block, then the scene**:

```
{
  |scene| layout:row gap:30          // root container
  |wire|  stroke:#444 clearance:8    // defaults for every wire
  .loud   stroke:red                 // a reusable style
  |treat:rect| radius:5              // a new shape type
}

cat |treat| "Cat"
dog |treat| "Dog"
cat -> dog "chases"
```

---

## 1. Mental Model

A Lini file is at most one anonymous **defs block** `{ … }` followed by the
**scene** — node and wire statements at the root, in any order. The defs block,
if present, must come first.

The defs block holds reusable declarations (scene config, wire defaults, type
defaults, styles, shape definitions, variable overrides). The scene is what gets
drawn. See [Defs Block](#4-defs-block) and [Node Declarations](#5-node-declarations).

**Defs resolve together; the scene reads top-down.** Within the defs block,
styles, shape defs, and type-defaults may reference one another regardless of
order (like CSS rules) — cycles are still an error. Scene statements are read in
source order, which is also render order. Wires are the exception to needing a
declaration at all: naming an id that is declared nowhere in the file
auto-creates it; a declaration anywhere, before or after the wire, simply wins.

**Two kinds of defaults.**

- *Visual* defaults — colors, fonts, shadow tint — are exposed as CSS variables
  (`--lini-fg`, `--lini-accent`, …) so a host page can re-theme them at runtime.
- *Layout* defaults — gaps, paddings, sizes, thicknesses — are language
  constants, baked into the SVG. They are settable per-node, per-wire, via
  `|scene|` / `|wire|`, or via styles, but never left as runtime `var()`s — so a
  standalone SVG always looks right.

See [Variables & Defaults](#12-variables--defaults).

---

## 2. Lexical Syntax

| Property | Value |
|---|---|
| Extension | `.lini` |
| Encoding | UTF-8 (BOM ignored) |
| Line endings | LF or CRLF (normalized on read) |
| Comments | `// …` to end of line. No block comments. |
| Statement end | newline or `;` |
| Identifier | `[a-zA-Z_][a-zA-Z0-9_-]*` — case-sensitive, ASCII |

Whitespace is insignificant except as a token separator and where a sigil rule
says otherwise:

| Form | Whitespace rule |
|---|---|
| `\|…\|` | Opening and closing `\|` paired; whitespace allowed inside, not at an ident boundary. |
| `key:value` | **No whitespace either side of `:`.** `radius: 5` is an error. |
| `.name` (style) | **Space required before** when it follows an ident or `\|`. `cat .loud` ✓; `cat.loud` parses as an endpoint dot-path. |
| `id.side` | **No space before.** Only in wire endpoints. |
| `--name` | As an attr value, or at a defs line start to override a variable. |

**Strings** — double-quoted UTF-8. Escapes: `\"`, `\\`, `\n`, `\t`. Single quotes
are not strings.

**Numbers** — integer or decimal, optional sign, no units (px for lengths,
degrees for angles, 0–1 for opacities). `10`, `-5`, `0.25`, `+3`.

**Tuples & lists** — `(10, 20)`, `(2, 2, 4, gray)` (2–5 components); `[(0,0),
(10,10)]`. The component count is fixed by the receiving attr.

**Colors** — `#fff`, `#ffaa00`, `#ffaa00cc` (alpha), CSS names (`red`,
`cornflowerblue`), `rgb(…)`, `rgba(…)`, `hsl(…)`, a `--name` variable reference,
or `none`. Out-of-range channels are an error.

---

## 3. Sigils

Each row is a parsing rule. The `:` and `.` sigils each do two jobs,
disambiguated by whitespace and position; the pipe form takes its meaning from
context.

| Form | Where | Means |
|---|---|---|
| `\|rect\|` | node decl | Reference a built-in or user type. |
| `\|rect\|` | defs (no `:base`) | Set defaults for every `\|rect\|`. |
| `\|scene\|` | defs (≤ 1) | Configure the root scene container. |
| `\|wire\|` | defs (≤ 1) | Defaults for every wire. |
| `\|treat:rect\|` | defs | Define shape `treat`, base `rect`. |
| `key:value` | attr lists | Attribute binding. |
| `name:base` | inside `\|…\|` in defs | Inheritance binding. |
| `.alert` | after type (space before) | Apply style `alert`. |
| `cat.right` | wire endpoint (no space) | Side `right` of node `cat`. |
| `--accent` | attr value, or defs line start | Reference / override `--lini-accent`. |

---

## 4. Defs Block

One optional `{ … }` at the top of the file. It holds these line kinds, in any
order, each identified by its leading sigil:

```
{
  |scene| layout:(3,2) gap:40 padding:20 background:--bg   // scene config (≤ 1)
  |wire|  stroke:#444 thickness:1 clearance:8              // wire defaults (≤ 1)
  |rect|  radius:4                                         // type defaults
  --accent:#0a84ff                                         // variable override
  .loud   stroke:red thickness:2                           // style def
  |treat:rect| radius:5                                    // shape def
  |den:group|  layout:column gap:8 padding:12 {            // shape def with body
    |text| "Title" at:top weight:bold
    body  |text| "Content"
  }
}
```

| First token | Line kind |
|---|---|
| `\|scene\|` | Scene config (singleton). |
| `\|wire\|` | Wire defaults (singleton). |
| `\|name:base\|` | New shape def — base is any primitive, template, or earlier user shape. |
| `\|name\|` (existing type) | Type defaults — applies to every instance of that type. |
| `.name` | Style def. |
| `--name` | Variable override. |

**The three pipe roles.**

- **`|scene|`** — the root container: `layout`, `gap`, `padding`, `background`,
  `h`, `v`, `col-widths`, `row-heights`. Defaults to `layout:row gap:20
  padding:20` if omitted.
- **`|wire|`** — defaults for every wire: `stroke`, `thickness`, `line`,
  `clearance`, `color`, `marker*`, `opacity`. Wire-relevant attrs only.
- **`|name|`** (a primitive, template, or user shape) — defaults for every
  instance of that type, sitting at the lowest specificity layer (see
  [Specificity](#13-specificity)). One entry per name. Composes with
  inheritance: `|rect| fill:lightyellow` tints every `|rect|`, `|card|`, and
  `|treat:rect|` instance.

**`|name:base| attrs… { body }`** defines a new type. At least one of attrs or
body must be present. Max inheritance depth 16; cycles are an error. A body may
contain id'd children **and** internal wires referencing those ids (see
[Wires](#10-wires)); internal ids are scoped to the body.

**`.name attrs…`** is a reusable attribute bundle. It may reference other styles
by `.other` (applied left-to-right). Cycles are an error.

---

## 5. Node Declarations

```
id [|type|] ["label"] ["href"] [.style…] [attrs…] [{ body }]
```

Everything but `id` is optional. **Order is strict:** id → type → label → href →
styles/attrs (these may interleave) → `{ body }`.

| Form | Effect |
|---|---|
| `cat` | `\|rect\|`, label "cat". |
| `cat \|treat\|` | Shape `treat`, label "cat". |
| `cat "Friendly cat"` | `\|rect\|`, label "Friendly cat". |
| `cat \|treat\| ""` | Shape `treat`, **no** label. |
| `cat \|treat\| "Cat" "https://example.com"` | Label + link (whole shape wrapped in `<a>`). |
| `cat \|treat\| .bold .loud cell:1 padding:5` | Shape + styles + attrs. |
| `garden \|group\| { … }` | Container with a body. |

**Inside a body**, primitives may be anonymous, declared starting with `|type|`:

```
garden |group| {
  |text| "Title" at:top weight:bold
  body |text| "Content"          // id'd, so a wire can reach it
}
```

Without an id a primitive can't be wired to.

**Implicit nodes.** A root wire's single-segment endpoint naming an id that
exists nowhere in the file auto-creates an empty `|rect|` at the scene root
with the id as its label. Declaring the id anywhere — before or after the wire
— prevents auto-creation. If the id exists only deeper in the tree (say inside
a shape instance), nothing is created: the wire must use the full path, and the
error suggests it. Body wires never auto-create.

**Label sugar.** `id |type| "label"` expands to a `|text|` child:

```
cat |treat| "Cat"     ≡     cat |treat| { |text| "Cat" }
```

If both sugar and explicit `|text|` children are present, the sugar's text comes
first. Multi-line labels use `\n`; the text bbox sizes to the widest line, line
spacing is `size × 1.2`.

**href.** A second positional string after the label wraps the whole shape in
`<a href>` (every child becomes clickable). On a bare `|text|`, only that text is
wrapped.

---

## 6. Layout

A container picks a mode via `layout`:

| Value | Behavior |
|---|---|
| `layout:row` | 1D horizontal flex. |
| `layout:column` | 1D vertical flex. |
| `layout:(COLS, ROWS)` | 2D grid. |

Grid children place with `cell:(c, r)` and span with `span:(c, r)` — both in
**(col, row) = (x, y)** order, 1-indexed.

### Container attrs

| Attr | Applies to | Notes |
|---|---|---|
| `layout` | all | `row`, `column`, or `(c, r)`. |
| `gap` | all | Space between children. Scalar = both axes; `(y, x)` = vertical / horizontal. Negative allowed. |
| `padding` | all | Inner padding. `N`, `(y, x)`, or `(t, r, b, l)`. |
| `col-widths`, `row-heights` | grid | Fixed track sizes. Scalar = all equal; list = per track. |
| `h`, `v` | all | Alignment / distribution. |
| `background` | scene only | Canvas background color. |

With `col-widths` / `row-heights` set, cells take exactly those sizes (an
explicit child `size:` still wins). Omitted → tracks auto-size to their widest /
tallest child.

**Multi-value `padding` / `radius`:** `N` = all four sides; `(y, x)` = vertical,
horizontal; `(t, r, b, l)` = clockwise from top. For `radius`, the 2-value form is
`(top-corners, bottom-corners)`.

### `h` / `v` values

| Value | Stacking axis | Cross axis |
|---|---|---|
| `start` / `center` / `end` | Pack at edge / center / opposite | Align child to edge / center / opposite |
| `stretch` | (no effect) | Children fill the cross axis |
| `between` / `around` / `evenly` | Distribute | (treated as `start`) |

For `layout:row` the stacking axis is horizontal; for `column`, vertical; for
grids, both axes stack and `h`/`v` align cell content. **Defaults:** grid cells
`h:center v:center`; flex `start` on the stacking axis, `stretch` on the cross
axis.

### Child positioning

| Attr | Effect |
|---|---|
| `at:(x, y)` | Bbox center at (x, y). Removes from flow. |
| `at:anchor` | Named anchor — see [Positioning](#7-positioning--anchors). |
| `offset:(x, y)` | Fine-tune from an anchor. |
| `cell:(c, r)` | Grid placement, 1-indexed. |
| `span:(c, r)` | Grid span. Default `(1, 1)`. |
| `z:N` | Render-order override; source order is the tiebreak. |

`at:` always beats `cell:`. An absolutely-positioned child still expands the
parent's bbox. Out-of-range cells are an error.

---

## 7. Positioning & Anchors

A shape's **bounding box** is the smallest axis-aligned rectangle containing it,
stroke included.

1. **Center origin.** Every bbox is centered at the parent's origin by default;
   `at:(x,y)` puts the center at (x,y).
2. **`origin:top-left`** opts into top-left positioning — per instance, or
   globally via `|scene| origin:top-left`.
3. **Source order = render order;** later draws on top. `z:N` overrides; ties
   break by source order.
4. **Strokes count** toward the bbox — `size:(100,50) thickness:4` → 104×54.
5. **`|path|`** is the only center-origin exception — `d:` uses native top-left
   coordinates.
6. **Rotation** applies last as an SVG transform; the rotated bounding rectangle
   propagates upward.

### Anchors

Relative to the parent's bbox.

- **Inside:** `center`, `top`, `bottom`, `left`, `right`, and the four corners
  (`top-left`, …).
- **Outside** (child's facing edge tangent to the parent's): `out-top`,
  `out-bottom`, `out-left`, `out-right`, plus the four corner variants. Computed
  against the parent's bbox **excluding** out-\* children, so they can't recurse.
- **Wire-route** (only on a `|text|` child of a wire): `start`, `mid`, `end`, or a
  fraction `0..1` along the route.

`offset:(x,y)` shifts from any anchor.

### Auto-sizing

Closed shapes auto-size to their text children + 16 px padding per side when
`size:` is omitted (text width from embedded font metrics). With neither `size:`
nor text:

| Shape | Default size |
|---|---|
| `\|rect\|`, `\|group\|`, `\|slant\|` | `(100, 40)` |
| `\|oval\|` | `(60, 40)` |
| `\|hex\|`, `\|cyl\|`, `\|diamond\|`, `\|cloud\|` | `(60, 60)` |
| `\|icon\|` | `24` |
| `\|poly\|`, `\|image\|`, `\|line\|` | Error if required attrs missing |

---

## 8. Primitives

13 primitives. All accept position and visual attrs; closed shapes also accept
`double:`, `rotation:`, `shadow:`.

**Dimensions** use `size:` — `size:N` is square/circle, `size:(w, h)` is a
rectangle/ellipse. `size:` is always **bbox dimensions**: `|oval| size:(60,40)` is
an ellipse in a 60×40 box; `|oval| size:40` is a circle.

| Primitive | Required | Notes |
|---|---|---|
| `\|rect\|` | `size` (auto) | Rounded via `radius:`. |
| `\|oval\|` | `size` (auto) | Bbox ellipse; `size:N` = circle. |
| `\|hex\|` | `size` (auto) | Regular hex, flat top/bottom. |
| `\|slant\|` | `size` (auto) | Parallelogram; top edge shifted `tan(skew) × h`. `skew` in degrees, (-89, 89). |
| `\|cyl\|` | `size` (auto) | Cylinder; body height `h`, end ellipses ±h/8. |
| `\|diamond\|` | `size` (auto) | Rhombus inscribed in the bbox. |
| `\|cloud\|` | `size` (auto) | Cloud path scaled to fit. |
| `\|poly\|` | `points:[(x,y),…]` | ≥3 points, local (center-origin) coords. Closed. |
| `\|path\|` | `d:"…"` | Raw SVG path. **Native top-left coords.** |
| `\|text\|` | label string | See [label sugar](#5-node-declarations) and [text attrs](#11-attributes). |
| `\|line\|` | `points:[(x,y),…]` | 2+ points. Markers via `marker*:`. |
| `\|icon\|` | `name` | Material Symbols. `variant:outlined\|filled\|rounded\|sharp`, `size:N`. Only referenced icons are bundled. |
| `\|image\|` | `href size` | `<image href="…">`. External URLs only. |

### Visual modifiers (closed shapes)

| Attr | Forms | Effect |
|---|---|---|
| `line` | `solid` / `dashed` / `dotted` | Stroke pattern. Default `solid`. |
| `double` | `N` / `(x, y)` | Draw twice with an offset, second copy behind. Scalar = `(N, -N)`. |
| `rotation` | `N` degrees | Rotate around the bbox center. |
| `shadow` | `N` / `(dx, dy)` / `(dx, dy, blur)` / `(dx, dy, blur, color)` | Drop shadow via SVG `<filter>`. |

### Markers (on `|line|` and wires)

| Attr | Effect |
|---|---|
| `marker:X` | Both ends. |
| `marker-start:X` | Start end (wire source). |
| `marker-end:X` | End end (wire target). |

Values: `none`, `arrow`, `dot`, `diamond`, `crow`. Markers scale with thickness,
floor 5 px; color follows the stroke. `|line|` is bare by default — write `|line|
marker-end:arrow` for a one-shot arrow. For wires the operator picks markers (see
[Wires](#10-wires)). Source order wins: `marker:arrow marker-end:dot` → start
arrow, end dot.

---

## 9. Templates

7 templates — each an attribute bundle over a primitive base, named because the
pattern is common.

| Template | Base | Defaults | For |
|---|---|---|---|
| `\|group\|` | `\|rect\|` | `line:dashed stroke:--muted fill:none padding:15`; text `at:top weight:bold` | Frame + label. |
| `\|badge\|` | `\|rect\|` | `at:top-right radius:999 padding:(2,8) shadow:2 fill:--accent z:10`; small on-accent text | Corner pill. |
| `\|button\|` | `\|rect\|` | `radius:4 padding:(8,16) shadow:2 fill:--accent`; on-accent text | Click target (with link). |
| `\|card\|` | `\|rect\|` | `radius:8 padding:16 shadow:2 stroke:none fill:--fill` | Content surface. |
| `\|note\|` | `\|rect\|` | `radius:2 padding:12 shadow:2 stroke:none fill:--note-bg` | Sticky note. |
| `\|table\|` | `\|group\|` | `gap:0 stroke:none`; use with `layout:(c,r)`, `col-widths:`, `row-heights:` | Container for `\|cell\|`s. |
| `\|cell\|` | `\|rect\|` | `padding:8 stroke:--stroke thickness:1 fill:none` | Bordered cell. |

Extend any of them: `|mybox:card| stroke:--accent`. Common shapes need no
template:

| For | Write |
|---|---|
| Circle | `\|oval\| size:N` |
| Database | `\|cyl\|` |
| Arrow | `\|line\| marker-end:arrow points:[(0,0),(50,0)]` |
| Dimension line | `\|line\| marker:arrow points:[…]` |

---

## 10. Wires

Wires connect scene-node ids.

### Operators

A wire op is `[start_marker?][line][end_marker?]`, no spaces:

| Part | Tokens |
|---|---|
| Line | `-` solid · `--` dashed · `-.-` dotted · `~` wavy |
| Start markers | `<` arrow · `>` crow · `*` dot · `<>` diamond |
| End markers | `>` arrow · `<` crow · `*` dot · `<>` diamond |

The same glyph differs by position (`<` is arrow at the start, crow at the end).

| Op | Markers | Line |
|---|---|---|
| `->` | none / arrow | solid |
| `<-` / `<->` | arrow / none, arrow / arrow | solid |
| `-*` / `*-` / `*-*` | dot combinations | solid |
| `-<>` / `<>-<>` | diamond | solid |
| `-<` / `>-<` | crow | solid |
| `*->` / `<-*` | mixed | solid |
| `-->` `--*` `--<` | (same markers) | dashed |
| `-.->` `-.-*` | (same markers) | dotted |
| `~>` `~*` `~<>` | (same markers) | wavy |
| `-` `--` `-.-` `~` | none | (each style) |

If the operator carries no markers, there are none on both ends. Explicit
`marker:` / `marker-start:` / `marker-end:` attrs override the operator (source
order wins). The operator's line part sets the wire's pattern (`--` ⇒
`line:dashed`, `-.-` ⇒ `line:dotted`); an explicit `line:` attr overrides it.

### Syntax

```
endpoints op endpoints [op endpoints …] ["label"] [.style…] [attrs…] [{ children }]
```

`endpoints` is one or more endpoints joined by `&`:

```
a -> b               // 1 wire
a -> b -> c          // chain: 2 wires
a -> b & c           // fan-out: a→b, a→c
a & b -> c           // fan-in
a & b -> c & d       // cartesian: 4 wires
a -> b -> c & d      // chain + fan: a→b, b→c, b→d
```

Mixing operators in one chain is a parse error. Children may only be `|text|`
(labels).

### Endpoints

```
endpoint = ident { "." ident } [ "." side ]
side     = left | right | top | bottom
```

Every wire resolves in a **scope** — the scene root for top-level wires, the
container's body for wires written inside one, the shape body for internal
wires. An endpoint is a path walked from that scope: the first segment names a
node directly in the scope, each further segment a child of the previous, and a
final segment matching a side name forces that side. **There is no search**: a
name that isn't reachable this way is an error even if it exists elsewhere, and
the error suggests full paths of same-named nodes —
`wire endpoint 'bowl' not found at scene root; did you mean 'kitchen.counter.bowl'?`

| Endpoint (in a root wire) | Resolves to |
|---|---|
| `cat` | root node `cat` |
| `kitchen.counter.bowl` | exactly that path |
| `kitchen.counter.bowl.left` | the same node, left side forced |

Bodies are **sealed**: a body wire connects nodes of its own subtree only.
Cross-container wires are written at the lowest level where both ends are
visible — usually the root. Without a side, the router picks edges by
geometry; with a side, that edge is forced.

### Labels & wire-text children

`a -> b "label"` expands to `a -> b { |text| "label" at:mid }`. For chains and
fans, the label sits at the midpoint of the whole route. Children take wire-route
anchors only (`start` / `mid` / `end` / `0..1`); `offset:(x,y)` shifts in the
route's tangent frame (`x` along the wire, `y` to its left). A label rides its
wire — it is an obstacle to nothing, and may slide along the wire to keep clear
of nodes and other labels; the wire never moves for it:

```
a -> b {
  |text| "label" at:mid text-size:10
  |text| "↓"     at:0.75
}
```

### Internal wires in shape defs

A shape body may wire its own children; ids are local and materialize per
instance — the same sealed-body rule as any container. From outside, the
dot-path navigates in:

```
{
  |room:group| layout:column gap:10 {
    inlet  |rect| "Inlet"
    outlet |rect| "Outlet"
    inlet -> outlet "flows"
  }
}
garden |room| "Garden"
kitchen |room| "Kitchen"
garden.outlet -> kitchen.inlet "carries"
```

### Routing

Wires route **orthogonally** — horizontal and vertical runs through the free space
between nodes, corners rounded. The router picks entry/exit sides globally unless an
explicit `.side` forces one. A single **`clearance`** (default 16), set once on
`|wire|` or `|scene|`, is the minimum gap every wire keeps from nodes and from other
wires.

The full routing contract — clearance, spacing, crossings, priority, fan-out,
self-loops — lives in [`WIRING.md`](WIRING.md), the source of truth for routing.
Marker tips sit on their endpoint, touching the shape edge.

---

## 11. Attributes

Every attr is `name:value` — no bare attrs.

### Visual (style)

Inline use (outside the defs block) emits a lint warning — see [Errors](#16-errors).

| Attr | Type | Default |
|---|---|---|
| `fill` | color | `--fill` (closed shapes); `currentColor` for `\|text\|`; `--stroke` for icons |
| `stroke` | color | `--stroke` |
| `color` | color | inherits — sets text color for descendant `\|text\|`; on `\|text\|`, an alias for `fill` |
| `thickness` | number | 1 |
| `line` | `solid` / `dashed` / `dotted` | `solid` |
| `opacity` | 0..1 | 1 |
| `radius` | scalar / (y,x) / (t,r,b,l) | 0 |
| `double` | `N` / `(x,y)` | off |
| `rotation` | degrees | 0 |
| `shadow` | `N` / `(dx,dy)` / `(dx,dy,blur)` / `(dx,dy,blur,color)` | off |
| `marker`, `marker-start`, `marker-end` | marker name | per-type |

`color` cascades through the SVG tree via native `currentColor`: set it on a
container to recolor every `|text|` descendant that doesn't override. Use `color`
for *labels*, `fill` for *bodies*.

### Geometry

| Attr | Type | Notes |
|---|---|---|
| `at` | `(x,y)` or anchor | Bbox center / anchor. |
| `offset` | `(x,y)` | From an anchor. |
| `size` | `N` or `(w,h)` | Bbox dimensions. |
| `points` | `[(x,y),…]` | Vertex list. |
| `d` | string | Raw SVG path (`\|path\|` only). |
| `skew` | number | Slant degrees (`\|slant\|` only). |
| `origin` | `center` / `top-left` | Bbox origin. |
| `z` | integer | Render order. |

### Container & grid

`layout`, `gap`, `padding`, `col-widths`, `row-heights`, `h`, `v`, `background`,
`cell`, `span` — see [Layout](#6-layout).

### Text

| Attr | Default | Notes |
|---|---|---|
| `at` | `center` | Anchor or `(x,y)`. `|text|` only. |
| `align` | `center` | `left` / `center` / `right` — multi-line alignment. |
| `text-size` | 13 | Font size, px. |
| `font` | `--font` | Font family: ident, string, or `--var` reference. |
| `weight` | `normal` | `normal` / `bold`. |
| `fill` | inherited (`currentColor`) | Text color; set `color` on an ancestor. |

`font`, `text-size`, `weight`, `align`, and `color` may be set on **any** node
and cascade to every descendant `|text|` — nearest ancestor wins, exactly like
CSS inheritance. `size` always means bbox dimensions; on a `|text|` it is an
error (`'size' is not a text attr; use 'text-size'`). Text metrics use the
embedded default font; setting `font` restyles without re-measuring (the same
honesty as runtime `--lini-font` theming).

### Accessibility & interaction

`title` emits a `<title>` child (tooltip + screen reader); `aria-label` is emitted
on the `<g>`. Links use the positional second string after the label (see [Node
Declarations](#5-node-declarations)).

---

## 12. Variables & Defaults

CSS variables are for **visual theming only** — colors, fonts, shadow tint.
Everything that affects layout is a baked language constant (settable per-node,
per-wire, via `|scene|` / `|wire|`, or via styles), so standalone SVG never
depends on the host CSS.

### 12.1 Visual variables (themeable)

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
--lini-font          sans-serif
--lini-text-color    var(--lini-fg)
--lini-shadow        rgba(0, 0, 0, 0.2)
```

These emit as live `var(--lini-*)` references, and the compiler ships an `@layer
lini.defaults` block alongside the SVG — so unlayered host CSS wins automatically,
no `!important`.

### 12.2 `--name` references

Any `--name` value is a Lini visual-var reference: the compiler prepends
`--lini-` and emits `var(--lini-name)`. Layout values can't be referenced this
way (they aren't themeable). Declare your own themeable var in the defs:

```
{ --brand:#ff6600 }
cat |rect| fill:--brand
```

Alias a host var from CSS: `.lini { --lini-accent: var(--my-brand-blue); }`.

### 12.3 Layout constants

Baked compile-time defaults — override per-node, per-wire, on `|scene|` /
`|wire|`, or via styles:

```
text-size 13   text-pad 16   thickness 1   radius 0
gap 20         padding 0     clearance 16  icon-size 24
canvas-pad 20
```

Per-shape default sizes are in [Positioning → Auto-sizing](#7-positioning--anchors).

### 12.4 `--bake-vars`

`<style>` class rules and inline `style=` work everywhere, browsers and
non-browser renderers alike — but CSS *variables* don't: resvg and librsvg fail
`var()` in every position (fallback syntax included). Browsers need nothing,
even for `<img>`-embedded SVGs — the defaults ship inside the file.
`--bake-vars` is the switch for everything else: it keeps the rules and inlines
every `var(--lini-name)` as its resolved literal — no runtime theming, but a
self-contained SVG that renders identically anywhere.

---

## 13. Specificity

Attrs on a node merge like CSS — **the more specific source wins**, ties broken
by **later wins**:

1. **Type cascade** — walked from the base primitive up to the node's declared
   type. Each step layers in that type's `|name|` defaults (from the defs
   block) and then its own definition, so a more-derived type overrides what it
   builds on. `|mybox:rect| fill:pink` paints pink even when a `|rect|
   fill:yellow` default is in scope: `mybox` is the more specific type, and an
   attr it sets itself wins over the inherited default.
2. **Style classes** — applied after the type cascade, in defs-block
   **definition order**; the order they are listed on the node is irrelevant,
   exactly as with CSS classes.
3. **Inline attrs** — on the line itself; the most specific, they beat
   everything above.

For a wire: `|wire|` defaults → style classes (definition order) → inline attrs.

Complex values (`at:(x,y)`, `padding:(t,r,b,l)`) replace wholesale — the merge
is per-key, not deep. `at:` always beats `cell:`.

---

## 14. SVG Output

```svg
<svg xmlns="http://www.w3.org/2000/svg"
     viewBox="X Y W H" width="W" height="H" class="lini">
  <style>
    @layer lini.defaults { :root, .lini { /* --lini-* variables */ } }
    .lini { font-family: var(--lini-font); font-size: 13px; color: var(--lini-text-color); }
    .lini .lini-shape-rect { fill: var(--lini-fill); stroke: var(--lini-stroke); stroke-width: 1; }
    .lini .lini-style-loud { stroke: red; stroke-width: 2; }   /* one rule per style def */
    .lini .lini-wire { stroke: var(--lini-stroke); stroke-width: 1; fill: none; }
    .lini .lini-marker { fill: var(--lini-stroke); }
  </style>
  <defs><!-- filters, clipPaths, icon symbols --></defs>
  <g class="lini-scene"> <!-- scene tree --> </g>
  <g class="lini-wires"> <!-- wires --> </g>
</svg>
```

`viewBox` auto-sizes to content + a 20 px canvas pad.

**Paint compiles to CSS; geometry bakes.** Shape and wire paint defaults — and
every `.style` def — are stated once as class rules; only the shape classes
and styles actually used are emitted. A node whose resolved paint differs from
what those rules provide carries the difference as an inline `style="…"`
(inline style beats class rules, mirroring [Specificity](#13-specificity)).
Geometry — sizes, positions, radii, points, paths, transforms — is always
baked into attributes. Inherited text properties (`font-family`, `font-size`,
`font-weight`, `color`) are stated on `.lini` and cascade natively; a node's
`font:` / `text-size:` / `weight:` / `color:` emits on its `<g>` and inherits
to its subtree. The variable block stays inside `@layer lini.defaults` so
unlayered host CSS wins; structural rules are unlayered so non-browser
renderers (which skip `@layer`) still parse them.

**Classes are a styling surface.** Host CSS may restyle any `lini-`-prefixed
class — the prefix makes collisions a non-issue. Variables remain the designed
theming surface. One caveat: layout is computed at compile time, so runtime
restyling (say a fatter `stroke-width`) restyles without re-layout.

**Node:**

```svg
<g class="lini-node lini-shape-{type} lini-shape-{parent} lini-style-{s}"
   data-id="ID" transform="translate(X,Y)">  <!-- geometry, then children --></g>
```

Auto-classes: `lini-node` (every node); `lini-shape-{name}` (the type and every
type it inherits); `lini-style-{name}` (per applied style). With rotation, the
transform becomes `translate(X,Y) rotate(N)`.

**Wire:**

```svg
<g class="lini-wire lini-style-{s}" data-from="A" data-to="B">
  <path d="…" fill="none" stroke="…"/>
  <polygon class="lini-marker lini-marker-arrow" …/>
</g>
```

Standalone output always embeds the full `<style>` block; host pages override
via the `@layer`ed variables or by restyling the classes.

---

## 15. CLI

```
lini [options] <input.lini>
lini fmt [--check] [--stdout] <input.lini>
lini serve [--port N] [--bake-vars] <input.lini>
```

| Flag | Meaning |
|---|---|
| `-o FILE` | Output path (default stdout). |
| `--format svg\|html` | `svg` (default) or HTML wrapper. |
| `--standalone` | Accepted for compatibility; the default output already embeds the CSS. |
| `--check` | Parse + resolve only — layout/render errors still surface on a full compile. |
| `--theme FILE` | CSS file of `--lini-*` overrides. |
| `--no-warn` / `--strict` | Silence warnings / treat them as errors. |
| `--bake-vars` | Inline `var()`s as literals (for non-browser renderers). |
| `--watch` | Recompile on every input change (requires `-o`). |
| `-h`, `-V` | Help / version. |

`lini -` reads stdin (filename `<stdin>` in errors). **`lini serve`** runs a
local live-reloading preview (default port 7700).

**`lini fmt`** reformats to canonical style — 2-space indent, column-aligned
id/type/label/attrs within a block, comments and blank lines preserved. `--check`
exits 1 if it would change anything; `--stdout` writes instead of rewriting.

Exit codes: 0 success · 1 parse/resolution error or `--check` reformat needed · 2
I/O · 3 invalid CLI.

---

## 16. Errors

Format: `filename:line:col: error: <message>` (LSP-compatible).

| Condition | Message |
|---|---|
| Duplicate id | `duplicate id 'X' (previously at L:C)` |
| Unknown endpoint | `wire endpoint 'X' not found at <scope>` + `; did you mean 'A', 'B'?` when same-named nodes exist |
| Chain mixes operators | `wire chain mixes operators 'X' and 'Y'` |
| Chain < 2 nodes | `wire requires at least two endpoints` |
| Unknown type / style | `unknown type '\|X\|'` / `unknown style '.X'` |
| Inheritance cycle / depth | `cycle in 'X' → … → 'X'` / `'X' exceeds max inheritance depth (16)` |
| Forward reference | `'X' used before its definition (L:C)` |
| Defs block not first | `defs block must be the first statement` |
| Missing required attr | `'\|line\|' requires 'points'` |
| Unknown attr | `unknown attr 'foo' on '\|rect\|'` (warning) |
| Renamed attr | `unknown attr 'stroke-style'; use 'line'` (warning) |
| `size` on text | `'size' is not a text attr; use 'text-size'` |
| Wire body non-text | `wire body may only contain \|text\| primitives` |
| Wire text node anchor | `wire labels accept only start/mid/end/0..1` |
| Invalid / out-of-range color | `invalid color 'XYZ'` / `rgb(300,0,0): component out of range` |
| Reserved identifier | `'start' is reserved (ids are case-sensitive — 'Start' is free)` |
| Grid out of range | `cell:(5, _) exceeds grid cols=3` |
| `skew` out of range | `skew:N must be in (-89, 89)` |
| Unknown icon | `unknown icon name 'XYZ'` |
| Track length mismatch | `col-widths has N values but grid cols=M` |
| Whitespace around `:` | `binding ':' must have no whitespace on either side` |
| Duplicate `\|scene\|` / `\|wire\|` | `'\|scene\|' may appear at most once in the defs block` |
| Non-wire attr on `\|wire\|` | `attr 'layout' is not valid on '\|wire\|'` |
| Type-defaults unknown type | `unknown type '\|frog\|' in defs` |
| Duplicate type-defaults | `duplicate type-defaults entry '\|rect\|'` |
| Visual attr inline (lint) | `visual attr 'fill' inline; consider a .style` (warning) |

**Visual-attr lint category** (the paint set): `fill`, `stroke`, `color`,
`thickness`, `line`, `opacity`, `radius`, `double`, `rotation`, `shadow`,
`weight`, `align`, `variant`, `font`, `text-size`.

**Always inline-OK (structural):** type / class / id / label / href / `title` /
`aria-label`; placement (`at`, `offset`, `cell`, `span`, `z`); container (`layout`,
`gap`, `padding`, `col-widths`, `row-heights`); geometry (`size`, `points`, `d`,
`skew`); wire `marker*` / `clearance`; and `size` / `name` on `|icon|`.

---

## 17. Grammar (EBNF)

```
file           = [ defs_block ] { stmt | comment | newline } EOF
defs_block     = "{" { defs_line | comment | newline } "}"

defs_line      = scene_config | wire_config | type_defaults | shape_def | style_def | var_override
scene_config   = "|scene|" { attr } end          # ≤ 1
wire_config    = "|wire|"  { attr } end           # ≤ 1
type_defaults  = "|" ident "|" { attr } end       # ident ≠ scene/wire; a known type
shape_def      = "|" ident ":" ident "|" { style_ref | attr } [ "{" body "}" ] end
style_def      = "." ident { style_ref | attr } end
var_override   = "--" ident ":" value end

stmt           = node_decl | wire_decl
node_decl      = ident [ type_use ] [ string [ string ] ] { style_ref | attr } [ "{" body "}" ] end
primitive_decl = type_use [ string [ string ] ] { style_ref | attr } [ "{" body "}" ] end
type_use       = "|" ident "|"
body           = { node_decl | primitive_decl | wire_decl | comment | newline }

wire_decl      = endpoint_group wire_op endpoint_group { wire_op endpoint_group }
                 [ string ] { style_ref | attr } [ "{" { text_decl } "}" ] end
endpoint_group = endpoint { "&" endpoint }
endpoint       = ident { "." ident } [ "." side ]
side           = "left" | "right" | "top" | "bottom"
text_decl      = "|text|" string { style_ref | attr } end

wire_op        = [ start_marker ] line [ end_marker ]
line           = "-" | "--" | "-.-" | "~"
start_marker   = "<" | ">" | "*" | "<>"
end_marker     = ">" | "<" | "*" | "<>"

attr           = ident ":" value                  # no whitespace around ":"
style_ref      = "." ident                         # whitespace before required
value          = number | string | color | tuple | list | ident | lini_var
tuple          = "(" value { "," value } ")"
list           = "[" [ value { "," value } ] "]"
color          = "#" hexdigit{3|6|8} | css_name | "rgb(" … ")" | "rgba(" … ")" | "hsl(" … ")" | "none"
lini_var      = "--" ident { "-" ident }
number         = [ "+" | "-" ] ( digit+ [ "." digit+ ] | "." digit+ )
string         = '"' { char | escape } '"'
escape         = "\\" ( '"' | "\\" | "n" | "t" )
ident          = ( letter | "_" ) { letter | digit | "_" | "-" }
comment        = "//" { not-newline } newline
end            = newline | ";"
```

LL(1) — single-token lookahead suffices; a hand-written recursive-descent parser
fits in ~600 LOC.

---

## 18. Implementer Algorithm

A reference pipeline; implementations may differ if the observable output matches.

**Parse.** Lex to tokens, then recursive-descent to an AST.

**Resolve** (top-to-bottom):

1. *Defs:* merge visual-var defaults ← `--theme` ← `--name:value`; apply `|scene|`
   (else `layout:row gap:20 padding:20`); capture `|wire|` defaults; register
   styles, shape defs (detect cycles / depth > 16), and type-defaults (validate
   the name; reject duplicates).
2. *Scene tree:* resolve each node's type and styles; layer attrs per
   [Specificity](#13-specificity); expand shape instances, scoping internal
   ids; build the path index, then auto-create root rects for single-segment
   root-wire endpoints absent from it.
3. *Wires:* resolve endpoints by scoped path walk (root wires from the scene
   root, body wires from their container) with suggestion errors; merge wire
   attrs; cartesian-expand fan groups into one resolved wire per pair.

**Layout** (bottom-up): leaf bbox from `size:` or defaults (+ half-`thickness`
stroke per side); arrange children per `layout`; `at:` children skip flow but
expand the bbox; apply `padding`, then place via `at:`/`offset:`; `rotation` last.

**Route wires.** Per the contract in [`WIRING.md`](WIRING.md) — orthogonal,
clearance-respecting, deterministic. Then place markers (sized `max(5,
thickness × 4)`, tip on the endpoint) and wire-text at their anchors.

**Render.** Depth-first emit SVG per [SVG Output](#14-svg-output).

---

## 19. Reserved Words

User identifiers cannot be any of the words below. The list is deliberately
maximal for v3 — relaxing a reservation later is backward compatible, the
reverse is not. Ids are case-sensitive, so a capitalized variant is always
free (`Start`, `Card`, …).

- **Layout:** `row`, `column`, `grid`.
- **Alignment:** `start`, `center`, `end`, `stretch`, `between`, `around`, `evenly`.
- **Node anchors / endpoint sides:** `top`, `bottom`, `left`, `right`, the 4 corners, the 8 `out-*`.
- **Wire-route anchor:** `mid` (`start`/`end` overlap alignment values; resolved by context).
- **Origin:** `top-left`.
- **Primitives:** `rect`, `oval`, `line`, `path`, `poly`, `text`, `hex`, `slant`, `cyl`, `diamond`, `cloud`, `icon`, `image`.
- **Templates:** `group`, `badge`, `button`, `card`, `note`, `table`, `cell`.
- **Special:** `scene`, `wire` (defs block only).
- **Constants:** `true`, `false`, `none`, `auto`.
- **Functions:** `var`, `rgb`, `rgba`, `hsl`.

---

## 20. Non-Goals

Out of scope for v3; the syntax stays forward-compatible.

- **Auto-layout** — you position nodes (flex/grid/anchors); Lini does not place
  or route them for you (no force-directed or hierarchical placement).
- **Multi-file imports.**
- **Animation**, and interactivity beyond `href`.
- **Manual wire waypoints.**
- **Cross-instance addressing** into a shape definition's internals — internal
  wires and dot-path reads work, but an external wire cannot reach into and
  restructure another instance's private nodes.

---

## 21. Examples

```
{
  |scene| layout:(3,2) gap:40 padding:20 background:--bg
  |wire|  stroke:#666 thickness:1 clearance:6
  |rect|  radius:4                                   // every rect rounds

  --accent:#0a84ff

  .thin   stroke:#444 thickness:1
  .bold   weight:bold
  .loud   stroke:red thickness:2

  |treat:rect| radius:5
  |nest:slant| fill:gray
  |alert:oval| stroke:red size:36                    // a circle

  |room:group| layout:column gap:8 {
    inlet  |rect| "Inlet"
    outlet |rect| "Outlet"
    inlet -> outlet "flows"
  }
}

cat |oval| "Cat — patient hunter" cell:(1,1)

kitchen |group| "Kitchen" cell:(2,1) layout:column gap:20 {
  counter |group| "Counter" layout:row gap:10 {
    bowl |treat| "Bowl of oats"
    water |nest| "Water"
  }
}

garden |group| "Garden" cell:(3,1) layout:column gap:20 {
  den |group| "Den" layout:row gap:15 {
    rabbit |alert| "Rabbit" { |badge| "FAST" }
    carrot |rect|  "Carrot patch" double:4 size:(80,40) fill:white
  }
}

closet |room| "Closet" cell:(1,2)
fridge |room| "Fridge" cell:(2,2)

// wires — full paths from the wire's scope (here: the root)
cat.right -> kitchen.counter.bowl.left -> kitchen.counter.water
kitchen.counter.water -> garden.den.rabbit -> garden.den.carrot .loud
cat <-> kitchen "watches"
closet.outlet -> fridge.inlet "restocks"
```

### Table + dimension line

```
basket |table| layout:(3,3) col-widths:[80,140,80] row-heights:28 {
  |cell| "Fruit" weight:bold; |cell| "Qty" weight:bold; |cell| "Notes" weight:bold
  |cell| "Apple"; |cell| "12"; |cell| "fresh"
  |cell| "Mango"; |cell| "3";  |cell| "ripe"
}

dim1 |line| points:[(0,200),(300,200)] marker:arrow color:#666 {
  |text| "300 mm" at:center text-size:11
}
```

### Mermaid-fast

```
cat -> dog -> bird     // 3 implicit rects, 2 wires
fox & owl -> mouse     // fan-in
frog ~> pond           // wavy arrow
fish --> bowl          // dashed arrow
```

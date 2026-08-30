---
name: lini
description: Use when asked to create, edit, review, or debug a Lini diagram (a .lini file or its SVG) — architecture/box-and-arrow diagrams, flowcharts, mindmaps, org charts, ER schemas, tables, sequence diagrams, bar/line/area/pie/radar/scatter charts, engineering drawings, floor plans, or circuit schematics written in the Lini language.
---

# Lini — writing beautiful diagrams

Lini compiles plain text to clean, themeable SVG: composable nodes, a CSS-like
cascade, compile-time layout. One core drives every diagram family. This file is
self-sufficient for real work; `SPEC.md` (the full language), `ROUTING.md` (wire
geometry), and `samples/` (the showroom) go deeper.

## The loop

Write → compile → **look at the render** → refine. Never ship a diagram you
haven't seen.

```sh
lini d.lini -o d.svg                 # compile; errors are file:line:col with fixes
lini --strict d.lini -o d.svg        # warnings become errors — use before finishing
lini fmt d.lini                      # canonical formatting, rewrites in place
lini --static d.lini -o s.svg && resvg s.svg d.png   # rasterize, then READ d.png
```

`--static` inlines CSS variables and outlines text — required before `resvg`
(it can't resolve `var()`). In this repo the binary is `target/release/lini`
(`cargo build --release` if missing). `lini desugar d.lini` prints the lowered
form when sugar confuses; `lini serve` opens a live playground.

## A complete file

A file is **one optional `{ }` stylesheet, then drawn statements** in source
order. The stylesheet configures and styles; it draws nothing, and must come
first.

```
{                                          // the stylesheet — setup only
  layout: grid;  columns: repeat(2);  gap: 30;   // scene config (root declarations)
  --brand: #ff6600;                        // a themeable colour variable
  |box| { radius: 6; }                     // a rule: style every box
  |-| { stroke: --gray-deep; }             // a rule: style every link
  .hot { stroke: --red-deep; }             // a class definition
  |svc::box| { fill: --teal-wash; stroke: --teal-ink; }  // a define: new type over a base
}

|svc#api| "API"                            // instances — the canvas
|cyl#db| "Postgres" { fill: --rose-wash; }
api -> db "queries" .hot                   // a link with a label and a worn class
users -> api                               // undeclared id → auto-creates |box#users| "users"
```

Every drawn statement is a node (`|…|`), a text leaf (`"…"`), or a link (bare
name + operator). The full node anatomy — **only the bars are required, order
fixed** — is `|type#id| "label" .class1.class2 { key: value; } [ children ]`;
a link takes the same tail on a different head: `a -> b "label" .cls { } [ ]`.

## Syntax laws that bite

- **Declarations end with `;`** and live only inside `{ }`. A value runs to its
  `;`, so it may span lines. (`;` optional right before `}`.)
- **A statement ends at a newline or `;`** — two nodes on one line need the `;`:
  `|topic| "A"; |topic| "B"`. Bare strings are self-delimiting (`"a" "b"` is
  two text leaves, no `;` needed). A `{ }` or `[ ]` may span lines freely; the
  statement ends after its final bracket.
- **Text is always double-quoted**; a bare word is an identifier (keyword,
  colour name, id). Single quotes are not strings. String-valued properties
  (`title`, `hint`, `href`, `src`) need quotes even for one word.
- **The comma law**: commas separate repeated list items, spaces separate the
  components of one item. `data: 9, 15, 24` (three values) · `data: 10 20, 30 40`
  (two x-y points) · `padding: 5 2 5 5` (one four-part value) · `translate: 10 -4`.
- **Math needs parens** — operators appear only inside `(…)`: `padding: (8 * 2);`
  `width: (w / 2)`. Calls are bare (`width: scale(3)`), signed numbers are bare
  (`translate: -35 20`). Bindings in the stylesheet: `w = 120;` `scale(n) = (100 * 1.2^n);`
  — read bare anywhere a value goes.
- **A class is worn, never glued into bars**: `|box| .hot` — not `|box.hot|`.
  The label comes before classes: `|box#a| "A" .hot`. First class spaced off the
  head, further ones glued: `.hot.loud`.
- **`--name` variables are visual only** (colours, `font-family`). Sizes, gaps,
  padding, `font-size` bake at compile time — use literals or `name = 5;` bindings.
- **No coordinate property.** Layout places nodes; to place absolutely use
  `pin: center; translate: x y` (parent-local coordinates, y grows down).
- **No `text-align`** — a text's lines align by its container's *horizontal*
  packing knob: `justify` in a row, `align` in a column or grid — so `align` in
  a card, which stacks. Split intents: wrap text in its own
  `|block| { justify: start; }`.
- **`id.child` paths glue** (no spaces): `kitchen.bowl`. `a:left` forces a link
  side. Paths resolve exactly in scope — never searched, never auto-created.
- Comments are `// …` only. Identifiers are `[a-zA-Z_][a-zA-Z0-9_-]*`,
  case-sensitive. Ids may not start with `lini-`.

## Cascade

Five tiers, most specific wins, ties → later wins:
type defaults/rules `|box| { }` → descendant rules `|table| |box| { }` → class
rules `.hot { }` → id rule `#hero { }` → the instance's own `{ }` block.
Links walk the same ladder via `|-|` (`#g |-| { }` styles links written in `#g`).
Values replace wholesale (no per-component merge). Text properties (`font-*`,
`color`, `letter-spacing`) plus `clearance`/`routing` also **inherit** down the
tree, nearest ancestor wins — set `font-size` once on the root and everything
scales, captions and link labels included.

## Box model & placement

- **Center origin**; source order = paint order (later on top; `layer: N` overrides).
- **Auto-size**: box = content + `padding` each side (default 20 on framed
  boxes, 0 on `|block|`). Explicit `width`/`height` are **floors** — content
  never clips. Empty auto box = 2×padding (40×40).
- **`max-width: N`** wraps text to fit (`text-wrap: nowrap` forbids); the
  wrapped size is the measured size.
- **`pin`** lifts a child out of flow onto a parent anchor: `center`, edges
  (`top` …), corners (`top left` …). A pinned child is an overlay — paints above,
  never grows the parent.
- **`translate: x y`** nudges any node after placement (layout-neutral);
  **`rotate: N`** turns about the bbox centre. Both work on text too.
- Flow containers: `direction: row | column` (default row — source order flows
  the way it reads; a closed shape's and a `|topic|`'s children are **card
  content** and stack instead, so an icon sits over its label), `gap` (default
  36; 8 in card content), `align` (cross axis) / `justify` (main axis):
  `start | center | end | stretch | evenly | origin` — no-ops without slack
  (explicit size or fixed tracks).
- Grid: `columns` **required** — `columns: 80, auto, repeat(3)`; `rows`
  optional; children auto-flow, or `cell: col row` / `span: cols rows` on any
  non-text child (bare text can't carry them — wrap it in `|block|`). Empty
  `""` holds a grid cell. Per-column alignment:
  `align: start, center, end` (one entry per track).
- `gap-fill: colour` paints the gutters (`gap: 1; gap-fill: --stroke` =
  hairline rules — how `|table|` works).

## Node catalogue

Primitives: `|block|` (frameless base rect), `|oval|` (equal sides = circle),
`|hex|`, `|slant|`, `|cyl|`, `|diamond|`, `|poly|` (`points:`), `|line|`
(`points:`, `marker*:`), `|path|` (raw SVG `path:`), `|image|` (`src:` +
`width`/`height`; local files embed), `|icon|` (a Phosphor icon by name),
`|sketch|` (`draw:` pen — see Drawing). Text is not a node type — a bare `"…"`
is a text leaf; wrap it in `|block|` when it needs an id, border, padding, or pin.

Templates (all overridable; extend with `|name::base| { … }`):

| Type | What it is |
|---|---|
| `\|box\|` | the default: rounded framed card (radius 8, padding 20) |
| `\|rect\|` | sharp-cornered box |
| `\|group\|` | dashed light frame for a captioned region |
| `\|caption\|` | small muted title pinned above the top-left corner (a group/table **label becomes one**) |
| `\|footnote\|` | muted caption at the bottom centre |
| `\|badge\|` | small accent pill pinned over the top-right corner |
| `\|row\|` / `\|column\|` / `\|grid\|` | frameless layout wrappers |
| `\|icon\|` / `\|sign\|` | Phosphor icon (label names the symbol: `\|icon\| "heart"`); `\|sign\|` is the 64px standalone preset. **Icons paint with `fill` (body) + `stroke` (line)** — `color:` does nothing on the glyph. An icon's `[ ]` text rides *on* the symbol as a badge and grows the square: `\|icon\| "bell" [ "3" ]` |
| `\|table\|` | ruled grid; first row auto-becomes the header band; cells via bare strings |
| `\|entity\|` | ER card: label = centred title, rows = `"field" "type"` (3 columns for a key gutter) |
| `\|note\|` | folded-corner callout card (works in every layout) |
| `\|topic\|` / `\|mindmap\|` | tree structure node / the full mindmap preset |
| `\|chart\|` / `\|pie\|` / `\|sequence\|` / `\|drawing\|` / `\|floorplan\|` / `\|schematic\|` | layout presets (below) |

Shape extras: `stack: N` (offset duplicate behind — "a pile"), `shadow: dx dy blur`,
`stroke-style: solid|dashed|dotted` (+ drafting `center`/`phantom` on shapes,
`wavy` on links only). `href: "url"` makes anything clickable; `hint: "…"` adds
a tooltip/accessible `<title>`.

Common shapes need no template: circle = `|oval| { width: 40; }`, database =
`|cyl|`, standalone arrow = `|line| { points: 0 0, 50 0; marker-end: arrow; }`.

## Links

```
a -> b                          // 1 link (labels boxes into existence)
a -> b -> c                     // chain: 2 links, every hop marked
a -> b & c                      // fan-out — shares one trunk at a's side
a & b -> c                      // fan-in
|group#g| [ |box#child| "C" ]
a:right -> g.child:left "label" // forced sides, path endpoint, label
client -> |cyl#db|              // capsule endpoint: declare + link in one statement
```

Operators are `[marker][line][marker]`, glued. Lines: `-` solid, `--` dashed,
`---` dotted, `~` wavy. End markers: `>` arrow, `<` crow, `*` dot, `<>` diamond
(mirrored at the start: `<-`, `<->`, `*-*`…). ER cardinality (crow's-foot),
end-side forms: `-+` one · `-<` many · `-o+` zero-or-one · `-+<` one-or-many ·
`-o<` zero-or-many · `-++` exactly one (mirror at the start: `>o-o<`).

- Style links like nodes: `|-| { stroke: #888; stroke-width: 1.5; }` for all,
  a worn class per link (`a -> b .loud`), the link's own `{ }` to override.
  `stroke*` is the wire; `color`/`font-*` the labels. Explicit
  `marker:`/`marker-start:`/`marker-end:` override the operator.
- Labels: one inline (`a -> b "hi"`), several or styled ones in `[ ]`:
  `a -> b { along: 0.3, 0.7; } [ "near a" "near b" ]`. Labels slide to dodge;
  they never move the wire.
- **Scene config, not link paint**: `clearance: N` (min gap wire↔node, default
  16) and `routing: orthogonal | natural | straight` sit on a container's `{ }`
  and cascade. `orthogonal` (default) = right-angle runs, rounded corners;
  `natural` = smooth direct curves (the mindmap look — free crossings);
  `straight` = one trimmed segment. Per-link `routing:` is an error.
- A self-loop `a -> a` exits right, hooks over the top. Bodies are sealed: a
  link inside `[ ]` connects that body's own children; cross-container links go
  at the lowest scope seeing both ends, via dot-paths. **Where a link is
  written is its routing world**: a link between two children of one group
  routes *inside* that group only if written in its `[ ]` — written at root it
  detours around the group's outside. Keep intra-group wires in the group.
- An unroutable link draws as a dashed slanted **stray** and is reported —
  fix by widening `gap`, shrinking `clearance`, or re-siding; nodes never move.

## Colour & theming

Every colour is a `light-dark()` pair — dark mode is automatic; **never
hardcode hex where a variable fits**. Role variables (as values, write the
short form `--bg`, `--fill`, `--stroke`, `--accent`, `--muted`, `--danger`,
`--warn`…): `fill: --bg` on the root paints the backdrop (default is none —
transparent).

**The palette** is the beauty engine. Eleven hues — `red rose orange amber lime
green teal sky blue purple gray` — each in five job-named tiers that survive the
dark flip:

| Tier | Job |
|---|---|
| `--teal-wash` | palest — card/section backgrounds |
| `--teal-soft` | gentle pastel fill (chart fills) |
| `--teal` | the everyday pastel |
| `--teal-deep` | strong — borders, strokes, wires |
| `--teal-ink` | deepest — text and emphasis |

The core recipe for a coloured node: **wash fill + ink (or deep) stroke + ink
text** — `{ fill: --teal-wash; stroke: --teal-ink; color: --teal-ink; }`.
`red` is reserved for danger; `rose` is the decorative pink.

Gradients (on `fill`/`stroke`/`gap-fill`): `gradient(--rose, --sky)` auto-135°,
`linear-gradient(135, --rose, --sky)`, `radial-gradient(…)`. `hatch(45)` /
`hatch(45, 6, --gray-deep)` is the section-line fill texture. Declare your own
variables in the stylesheet: `--brand: #ff6600;` then `fill: --brand`.

Fonts: bundled **Google Sans** (default) and **Google Sans Code** (mono, via
`font-family: "Google Sans Code"`). `font-weight: normal | medium | semibold |
bold` (or 400–700). Body default 15/medium; captions and link labels derive
from the inherited size, so one root `font-size:` scales everything.

Themes re-skin at render time — no file edits: `--theme` takes a builtin
(`light` · `dark` · `high-contrast` · `blueprint`, white linework on cyanotype
blue — the diazo print, for any diagram), a CSS file of `--lini-*` overrides,
or a `light/dark` pair. `lini serve --theme` paints every served compile;
`lini theme NAME` prints a builtin as CSS to start your own.

## Layout engines

`layout:` on any container (the root included): `flow` (default) · `grid` ·
`tree` · `sequence` · `chart` · `pie` · `drawing` · `floorplan` · `schematic`.
Everything core — cascade, paint, palette, links syntax — works identically
inside each.

### Tables & entities

```
|table#basket| { columns: 80, 140, 80; align: start, center, end; } [
  "Fruit" "Quantity" "Notes"       // first row → header band
  "Apple" "12"       "fresh"
  "Mango" "3" { color: --red-ink }  "ripe"   // a styled cell
]

|entity#users| "Users" { columns: auto, auto, auto; } [
  "PK" "id"    "int"
  ""   "email" "varchar"
]
users -o< orders        // crow's-foot relationship, lands on the card edge
```

Style cells with `|table| |cell| { … }`, the header with `|table| |header| { … }`.
A cell needing an id (to wire a field) becomes a box child: `|block#uid| "user_id"`.

### Tree & mindmap

Structure is `|topic|` nesting inside a `layout: tree` scope — exactly one root
topic; non-topic children are a topic's own content (an icon, a badge).
`direction: column` (org chart, default) · `row` (outline) · `bilateral`
(mindmap split; per-branch `side: left|right` overrides). Branch wires are
generated; style a whole arm with `#branchid |-| { }`; size a tier with
`.lini-level-2 { font-size: 12; }`.

`|mindmap| "Root"` is the preset worth reaching for: bilateral + `routing:
natural` + an automatic hue per first-level branch (wash fill, deep stroke and
wires, ink text) + depth-ramped sizes + `max-width: 160` wrap. Authored
cross-links stay neutral: `a.x:right --- b.y:right "relates" { along: 0.8; }`.

```
|mindmap| "Product Vision" [
  |topic#ship| "Shipping" [ |topic| "Weekly train"; |topic| "Feature flags" ]
  |topic#team| "Team" [ |topic| "Small and senior" ]
]
```

### Sequence

`layout: sequence` reads links as time: participants across the top (declared,
or auto-created on first use), messages top-to-bottom in source order. `->`
call (opens an activation bar) · `-->` return (closes it) · `~>` async ·
`a -> a` self-message. A participant's own paint colours its lifeline and bars.

```
{ layout: sequence; }
|box#user| "Customer" { fill: --rose-wash; stroke: --rose-deep; } [
  |icon| "user" { fill: none; stroke: --rose-deep; }
]
|box#shop| "Storefront" { fill: --sky-wash; stroke: --sky-deep; }
|cyl#db| "Orders" { fill: --orange-wash; stroke: --orange-deep; }

user -> shop "place order"
|loop| "each item" [
  shop -> db "reserve stock"
  db --> shop "in stock"
]
|alt| "paid" [
  shop --> user "receipt"
  |else| "declined"
  shop --> user "error"
]
|note| "rate-limited" { place: over shop db; }
```

Frames: `|loop|` / `|opt|` / `|alt|` (+ `|else|` separators) hold their
messages in `[ ]` but open no scope — messages always wire the sequence's
participants. `gap: row col` spaces rows/columns. `place:` modes: `over a`
(one lifeline), `over a b` (span), `left a` / `right a` (beside). A **named
actor** is a box wrapping an icon — `|box#user| "Customer" [ |icon| "user" ]` —
since a bare `|icon#user| "user"` spends its label on the symbol name and
renders an unnamed glyph.

### Charts & pie

A chart fixes a shared scale from all children, then draws. Default size
360×220 (`width`/`height` set the whole box); radial/pie default 280 square.

```
|chart| "Cycle time (s)" { categories: "15", "30", "50"; } [
  |bars| "1.8 kW" { data: 9, 15, 24; }
  |bars| "2.3 kW" { data: 7, 13, 20; }
]
```

- Series: `|bars|`, `|line|`, `|area|`, `|dots|` (+ per-node `|bubble|
  { at: x y; value: N; }`; `|slice| { value: N; }` in a pie). Label = legend
  entry (auto-shown at ≥ 2). Data: categorical `data: 9, 15, 24` (must match
  `categories:` count) · points `data: 0 225, 60 221` · dates
  `data: "2026-01-01" 18, …` · formula `fn: (min(100, x * 2))` sampled over the
  domain (`samples:` count).
- **Colour is automatic and good**: series walk the palette (interleaved hues,
  red skipped) in the outlined look — soft fill + deep edge. Only override for
  meaning: `fill: --sky-soft`, `stroke: --teal`. Per-datum highlight on bars/
  dots: `fill: auto, auto, --red, auto`.
- `curve: smooth` (monotone, never overshoots) · `step`; `marker: dot|circle|
  diamond` puts a mark at every datum; `labels: "a", "b", …` per-datum text
  (needs explicit `data:`); `tooltip: none|hover|auto|always`.
- Axes only when you have something to say: `|axis#t| "Speed (mm/s)"
  { side: bottom; range: 0 133; unit: "%"; scale: log|time; }`. Bind a series
  with `axis: t`. `range: 50 1` reverses; `gridlines: none` or a colour.
- Annotations in data space: `|band| "Hold" { range: 1.5 4; axis: t;
  fill: --amber; }` shades a region; a `|mark|` is a reference line (`at: V`),
  a labelled point (`at: x y`), or label-only (`marker: none`) — style its rule
  like a wire: `|mark| "SLO 250 ms" { at: 250; axis: ms; stroke: --amber-deep;
  stroke-style: dashed; color: --amber-ink; }`.
- `direction: row` flips bars horizontal; `direction: radial` makes radar
  (lines close into polygons); `bars: grouped (default) | stacked | overlay`
  combines bar series. `|pie| { hole: 0.5 }` is a donut.

### Drawing (engineering)

`layout: drawing` places every child's origin on a shared **datum** (no flow);
links become dimensions/leaders; **measured values are computed from the
geometry** — never type a number a dimension can read. No auto-create.

```
{ layout: drawing; }
|rect#plate| { width: 120; height: 70; } [
  |hole#pin| { width: 10; translate: -35 20; pattern: grid(2, 1, 70, 0); }
]
plate:left (-) plate:right { side: bottom; }   // → 120
plate:left (-) plate.pin { side: top; }        // → 25
plate.pin (o)                                  // → 2× ⌀10
plate.pin.2 <- "THRU"                          // leader to the 2nd pattern copy
```

- Ops: `(-)` linear (binary, chains share a row: `a (-) b (-) c`) · `(o)`
  round (unary: ⌀ for round features, R for named arcs) · `(<)` angle ·
  leaders `<- "text"` (arrow) / `*- "text"` (dot on a face) / `>- "A"` (datum
  triangle) · `a:left || b:right { gap: 4 }` mates part faces (moves geometry,
  draws nothing; negative gap = inserted).
- Custom profiles: `|sketch| { draw: move(-80, 0) up(14) right(50):neck
  fillet(3):r1 …; }` — pen calls left-to-right; `:name` glued to a call names
  that segment for dimensioning (`body:neck (o)` → ⌀). `mirror: x-axis` draws
  half a profile and fuses the whole; `revolve: x-axis` makes a turned part
  (centerline + shoulder lines auto); `|hole|` punches and centre-marks itself.
- Dims: `side:` picks the stacking edge, `tol: 0.1` / `tol: +0.2 -0.05` /
  `tol: h6` appends tolerance, labels follow (`pin (o) "H7"`) or replace
  (two-ended) the value. `scale: 2` on a drawing = 2:1 view that still
  measures true. Sheets: `|page| { sheet: a4 }` + `|title-block| { title: "…";
  drawing-number: "…"; }`; multi-view rows share axes with `align: origin`.
- Deep machinery, a line each: `thread: neck 1.25` dresses an ISO thread on a
  revolved profile — a bare leader on that segment composes `M8×1.25`;
  `break: -40 40` cuts a long part's boring middle (the view compresses, dims
  still read the unbroken model). A section / detail is a marker plus a view:
  `|plane#a| "A" { at: 40 }` or `|magnifier#c| "C" { width: 12 }` on the
  source, a sibling `|drawing| { of: a }` as the view — its title (`A-A (1:1)`,
  `C (3:1)`) composes itself. GD&T: `|surface-finish| "Ra 1.6"`,
  `|feature-control| "position" { tol: 0.05; datums: A }`, `|datum|` — seat on
  a face with `||` or carry in a dimension's `[ ]`; datum letters come from
  `>-` leaders (`body:seat >- "A"`).

### Floorplan (architectural)

`layout: floorplan` is the drawing engine in an architect's vocabulary —
same datum, `scale:`/`unit:`, anchors and dimensions. Build it in four passes:
**walls → openings → fixtures → dimensions**. Sizes you type are drawing units;
every *built-in* size is true physical mm converted through `unit:`.

```
{ layout: floorplan; unit: m; scale: 0.02 }     // 1:50 — 80 px per metre

|wall#outer| {                                  // draw: is the CENTRELINE
  draw: move(0, 0) right(7.2):north down(4.8):east
        left(7.2):south close():west;
} [                                             // openings ride the wall's [ ]
  |door#entry| { on: south; at: 3; swing: right }        // width: 900 mm default
  |window|     { on: north; at: 0.9; width: 1.8 }
  |door|       { on: west; at: 1.2; width: 2.4; symbol: sliding }
]
|partition#bathwall| { draw: move(4.9, 0) down(2.2) right(2.3):side } [
  |door| { on: side; at: 0.6; hinge: end }
]

|rect#counter| { width: 2.1; height: 0.6; translate: 6.05 0.4;
                 fill: --bg; stroke: --stroke-dark; stroke-width: 1 }
|bed|  { translate: 1.2 1.2; rotate: 90 }
|sofa| { symbol: corner; translate: 2 3.3 }
|appliance| "F" { symbol: fridge; translate: 0.5 4.3 }
"KITCHEN" { translate: 6 1.4 }                  // room names are plain sheet text

outer:north-in (-) outer:south-in { side: left }      // → 4.6 — the clear interior
outer:north-in (-) bathwall:side-in { side: right }   // → 2.05 — the kitchen's depth
outer:west (-) outer.entry (-) outer:east { side: bottom }  // locate the door
```

- **Walls.** `|wall|` is a `|sketch|` whose `draw:` traces the centreline;
  `thickness:` (200 mm default, inherits nearest-wins) offsets it into the
  mitred, solid-filled **poché** outline that takes the paint. `|partition|`
  is the 100 mm interior define. `fill: --bg; stroke: --stroke-dark` is the
  hollow double-line look, `fill: hatch(45)` the section convention. Walls bend
  with `arc()`; `curve()` errors. Draw meeting walls as **separate nodes** —
  paint order merges them seamlessly.
- **Openings.** A `|door|` / `|window|` must sit in its wall's `[ ]`, stationed
  `on:` a **straight named segment**, `at:` the near jamb's distance from that
  segment's start (mind the draw direction — a `left(...)` run measures from its
  east end). They clip the wall and generate their chrome: `hinge: start|end` ×
  `swing: left|right` (left of the pen's travel), `symbol: single | double |
  sliding` (a slider takes no `hinge:`/`swing:`). `translate:` on one is an error.
- **Fixtures.** `|bed|` (queen·king·double·single) · `|sofa|`
  (three·two·one·corner·stool — `one` is the armchair, `stool` the ⌀350 bar
  seat) · `|dining|` (six·four·round — sized by its **tabletop**, ⌀1000 for
  `round`; the pull-back chairs extend the bbox) · `|bath|`
  (tub·shower·toilet·sink·double-sink — the last is one unit, two basins) ·
  `|appliance|` (stove·fridge·washer·dishwasher) ·
  `|stairs|` (`steps: N` required). `width`/`height` are floors that **stretch**
  the body. Each fills `--bg`, so furniture masks the floor under it. Two label
  seats: a fixture's smart label hangs **below** the body — leave air there —
  but an `|appliance|`'s centres **inside** it (write `"F"` / `"DW"` / `"W/D"`);
  either way it turns upright, so a tag on a right-to-left wall still reads.
- **Everything else** is plain geometry: counters, islands, desks and coffee
  tables are `|rect|`s; a balcony deck, a north arrow or a scale bar is a
  `|sketch|`; room names and areas are sheet text placed with `translate:`.
  A casework `|rect|` takes the core `radius:` for a softened counter — mind
  that it is **sheet-space pixels**, not drawing units, so it does not scale
  with the plan (at 1:50 and the default density, `radius: 4` is 50 mm).
- **Dimensions** anchor on the wall's own named runs, which answer three ways:
  every named run derives its two **face anchors** — `-in` (the enclosed side
  on a closed run, the left of the pen's travel on an open one) and `-out` —
  and the bare `:segment` is the **centreline**, where a structural drawing
  measures. **Dimension inside faces by default**: a room reads its **clear**
  span (`outer:north-in (-) bathwall:side-in`) and the overall the shell's
  clear interior — what a listing plan publishes. A name of your own ending
  `-in`/`-out` on a wall errors. A named **edge**'s extension line springs
  from the end nearest the dimension line, so it leaves a corner and runs away
  from the plan, never down a wall face. Mind the axis — an edge dimensions
  **across** itself, so a horizontal span names the two vertical runs; and an
  id'd opening anchors at its centre, so a chain locates a door along its wall
  (`outer:west (-) outer.entry (-) outer:east`).
- **The print look is a theme, never authoring**: render with
  `--theme blueprint` (Colour & theming) for white-on-cyanotype; a plan's
  default stays black-on-white.

### Schematic

`layout: schematic` seats parts and lets the router draw square, junction-dotted
wires onto pins. 3+-pin parts are anchors on tracks (`columns:`/`cell:` order
them); 1–2-pin parts and labels are satellites seated at the pin their wire
touches. No auto-create — unknown bare ids error.

```
{ layout: schematic; |vcc::label| { symbol: power } [ "5V" ] }
|component#u1| "AMS1117-3.3" [
  |pin#vin| { side: left; number: 3; }
  |pin#gnd| { side: bottom; number: 1; }
  |pin#vout| { side: right; number: 2; }
]
|J#j1| "3V3 OUT" { pins: 4; rotate: 180; }
|C#c1| "22u"

u1.vin - |vcc|              // the power-flag capsule, defined above
u1.vout - c1 - |gnd|        // a chain PASSES THROUGH a 2-pin part: series circuit
u1.vout - j1.p3 "3V3"       // net name = the wire's label, set beside the trace
j1.p1 -> "NSTDBY"           // one-ended label wire; the marker shapes the tag
```

Discretes with generated pins: `|R| |C| |L| |D| |LED| |Q| |Y| |F| |FB| |SW|
|BT| |V| |I|` (pins `p1 p2`, or `a k`, `b c e` / `g d s` by `symbol:` variant —
`zener`, `npn`, `nfet`, `polarized`…). The id is the reference designator
(`#R5` reads R5); anonymous parts auto-number (display only — give an id to
wire it). `rotate:` is 90°-step; text stays upright. The classic look (green
wires, yellow bodies, beige sheet) is automatic.

## Making it beautiful

The defaults are designed — a plain file already reads well. Beauty is mostly
restraint plus a few deliberate moves:

1. **Colour by meaning, from the palette.** One hue per subsystem / branch /
   state, in the wash-fill + ink-stroke recipe. Two or three hues, not seven.
   Define the pairing once (`|svc::box| { fill: --teal-wash; stroke: --teal-ink;
   color: --teal-ink; }`) and instantiate — never repeat paint per node.
2. **Name regions with `|group|` + its caption label** (`|group#edge| "Edge"
   { gap: 20; } [ … ]`). Groups organize; boxes state.
3. **Refined over heavy**: keep strokes thin (1.5–2), body text
   `normal`/`medium`; save bold and strong colour for titles, one hero node, a
   `|badge|`. One gradient per scene at most (`fill: gradient(--sky, --purple);
   stroke: none; color: white` on the hero).
4. **Icons carry recognition**: `|icon| "bell"` inline, `|sign#cdn| "cloud"`
   as a standalone actor — painted with the same pair, via `fill`/`stroke`
   (`fill: --rose-wash; stroke: --rose-deep;` — `color:` never reaches a glyph).
5. **Let the engines work.** Don't hand-place what flow/grid/tree can lay out;
   reach for `pin`+`translate` only for free-form canvases (ER graphs) and
   overlays. Force `:side` sparingly — reorder declarations first. Charts:
   accept the palette walk unless colour has meaning.
6. **Meaning in line style**: solid = sync/primary, `-->` dashed = return/
   cache/secondary, `~>` wavy = async/event. Encode it as classes
   (`.async { stroke: --amber-deep; }`) so the legend lives in one place.
7. **Air**: root `padding: 24–30`; group `gap` 20–28; don't shrink the default
   36 scene gap without reason. `max-width` on prose-y labels (~160–200).
8. **Routing to match the mood**: `orthogonal` for architecture, `natural` for
   organic/mindmap scenes, `straight` for quick sketches.
9. Add `hint:` on dense nodes, `href:` where a diagram lands in a doc. Set
   root `fill: --bg` only when a backdrop plate is wanted.
10. **Look at the PNG** after every meaningful change; check label collisions,
    stray links (slanted dashed = unroutable), crowding. Fix strays by widening
    `gap` / reordering, never by fighting the router.

## When the compiler complains

Errors carry did-you-mean suggestions — read them, they're usually exact. The
frequent self-inflicted ones:

| Symptom | Fix |
|---|---|
| `a declaration ends with ';'` | add the `;` — every `key: value;` inside `{ }` |
| `unknown property 'colr'` | typo, or you invented a property — check the tables above |
| `math operators appear inside ( )` | wrap: `padding: (8 * 2);` |
| `'data' takes comma-separated values` | `data: 9, 15, 24` — spaces make x-y points |
| `a class follows the bars` | `\|box\| .hot`, never `\|box.hot\|` |
| `the stylesheet '{ }' must come first` | move it above all instances |
| `'layout: grid' requires 'columns'` | add `columns: repeat(N);` |
| `text content takes no '[ ]'` / `'pin' needs a box` | wrap the string in `\|block\|` |
| `'routing' is a scope's strategy` | set `routing:` on the container, not the link |
| link endpoint not found | paths never auto-create; declare it or fix the path (the error lists candidates) |
| `impossible (a -> b): no legal route: …` | a **stray** — drawn as a slanted dashed line, a warning unless `--strict`. Widen `gap`, drop a forced `:side`, move the link into the group whose children it joins, or lower `clearance` |

Warnings matter too (`--strict` for final output): near-miss ids (`cta` vs
`cat`), split label blocks, never-worn classes.

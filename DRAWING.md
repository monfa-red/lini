# Drawings — Specification

An extension of [`SPEC.md`](SPEC.md): the source of truth for **engineering drawings** —
dimensioned parts, sections, and assemblies for books, docs, and generated figures.
Written in the same register as [`CHARTS.md`](CHARTS.md) and to the same standard. A
drawing is **a layout** — `layout: drawing` — so everything the core language defines
(the cascade, paint roles, the `"string"` rule, the expression engine,
lower-to-primitives, theming, baking, determinism) applies unchanged and is referenced,
not restated. Once drawings are proven this folds into `SPEC.md`; until then it is the
law for drawings.

**Status: design spec, not yet implemented.** Agreed in brainstorm (supersedes
`ASSEMBLY_LAYOUT.md`); ready to be built against and refined where building teaches
better.

**One bet carries the whole design.** A drawing is a container whose children are
**geometry** sharing one datum, and whose links are **annotations** — dimensions,
callouts, leaders, and mates. Because the engine *has* the geometry in numbers, a
dimension's smart label is its **measured value**: the numbers live once, in the
geometry, and the annotations point at them. Everything lowers to ordinary Lini
primitives, so render, theming, dark/light, `--bake-vars`, `fmt`, and determinism come
for free.

---

## Table of Contents

1 [Mental model](#1-mental-model) · 2 [The drawing container](#2-the-drawing-container) ·
3 [Anchors](#3-anchors) · 4 [The sketch pen](#4-the-sketch-pen) ·
5 [Features & composition](#5-features--composition) · 6 [Patterns](#6-patterns) ·
7 [Mates](#7-mates--) · 8 [Dimensions](#8-dimensions) · 9 [Leaders & notes](#9-leaders--notes) ·
10 [Line & material conventions](#10-line--material-conventions) ·
11 [Assemblies & nesting](#11-assemblies--nesting) · 12 [Views & titles](#12-views--titles) ·
13 [Lowering & render](#13-lowering--render) · 14 [Properties](#14-properties) ·
15 [Grammar](#15-grammar) · 16 [Errors](#16-errors) · 17 [Examples](#17-examples) ·
18 [Deferred](#18-deferred)

---

## 1. Mental model

A drawing is a container ([SPEC §5](SPEC.md)) whose **layout** is `drawing`; the
`|drawing|` template is the preset over `|block|`, as `|chart|` is `layout: chart`. Its
children split by role:

| Child | Is | Drawn |
|---|---|---|
| a box (`\|sketch\|`, `\|rect\|`, `\|oval\|`, `\|hole\|`, …) | **geometry** — a part or a feature | its outline and fill, at the shared datum |
| a link with a drawing op — dims `<->` `(-)` `<)`, callouts `<-` `*-` `>-` `(-`, plain arrows `->` | an **annotation** — dimension, callout, leader | extension lines, arrows, text |
| a link with `@` | a **mate** — a positioning relationship | nothing — it moves a part |
| `"…"` bare text, `\|note\|`, `\|table\|`, … | ordinary content | per its own type |

Three properties of the model, each inherited from the core language:

- **A drawing scope owns its links** — the wiring strategy, exactly as a sequence lowers
  messages to time rows ([SPEC §10](SPEC.md)). The orthogonal router never runs here;
  every link lowers at layout time to dimension or leader primitives (or, for `@`, to a
  position). `routing:` and `clearance:` have no role in a drawing.
- **The smart label carries the measurement.** A dimension with no label renders its
  measured value; a label overrides it ([§8](#8-dimensions)). The op supplies the glyph
  (⌀, R, °, ±), the text supplies the words, `tol:` supplies the tolerance, the geometry
  supplies the number.
- **What you dimension is a node — or a named point or edge on one.** Anything you
  measure, mate, or point at is a node with an id — a `|hole|` child, a `--bg`-filled
  gap in a section view ([§5](#5-features--composition)) — or a point or edge a
  `|sketch|` **names on its own profile** with the `:name` sigil ([§3](#3-anchors),
  [§4](#4-the-sketch-pen)). Anonymous geometry (an unnamed fillet, a subpath) is
  drawable but not addressable.

**No auto-create.** Unlike a diagram (`cat -> dog` invents boxes) or a sequence (an
undeclared endpoint invents a participant), a drawing **never auto-creates an
endpoint**: an annotation must point at real geometry, and an invented box has none. An
unknown endpoint is an error with suggestions ([§16](#16-errors)).

**One placement model, whole scope.** Inside a drawing, every box child — and a
geometry child's own `[ ]` children, recursively — places at its parent's **datum**
([§2](#2-the-drawing-container)), not by flow. A child that **owns a layout** — a
`|table|`, a `|chart|`, a nested `|drawing|`, a `|row|` / `|column|` / `|grid|`, or any
node whose bundle or instance declares `layout:` or `direction:` — lays out its own
interior as usual and is placed as one box. This mirrors the sequence's one-scope rule:
the drawing decides placement; interiors keep their own engines.

### What is core, what is scoped

The geometry machinery is **ordinary Lini**, usable in any diagram; only the
annotation semantics need a drawing scope:

| Global — works everywhere | Drawing-scope only |
|---|---|
| `\|sketch\|` + `draw:` / `mirror:` — a primitive beside `\|poly\|` / `\|path\|` (`points:` draws by vertices, `path:` by SVG data, `draw:` by pen) | the dimension ops `<->` `(-)` `<)`, the callout ops `<-` `*-` `>-` `(-`, and `@`, with `tol:` and dim `side:` / `gap:` |
| `:name` **declarations** in a `draw:` | `:name` (and corner) **endpoints** — consumed by dims, leaders, and mates |
| `pattern:` | auto-measure, `scale:`, `unit:`, datum placement, the ground |
| `hatch()` fills; `stroke-style: center` | `\|hole\|` / `\|centerline\|` / `\|pitch-circle\|`, auto chrome, dimension packing |

Outside a drawing a `|sketch|` is just a shape — its names are declared but dormant: an
endpoint `:name` in a routed scope errors today, because teaching the orthogonal router
to land on an arbitrary point is a [ROUTING.md](ROUTING.md) contract extension (ports
and Law 2 are side-based) — deferred ([§18](#18-deferred)).

---

## 2. The drawing container

`|drawing|` is `|block|` + `layout: drawing` — frameless, padding 0 (the geometry and
its annotations *are* the content). `{ layout: drawing }` on the root makes the whole
file one drawing, exactly as a root sequence works; the root's padding then frames the
sheet.

### Datum & ground

The drawing's **datum** is its own origin (its center, like every Lini parent origin).
Every child's **origin lands on the datum** — *not* its bbox center:

- A symmetric primitive's origin is its center (`|rect|`, `|oval|`, `|cyl|`, …), so
  primitives stack **concentric** by default.
- A `|sketch|`'s origin is its **pen origin** — the coordinate frame its `move()`
  coordinates are written in ([§4](#4-the-sketch-pen)). Two sketches drawn at different
  pen offsets keep their drawn relationship; the bbox is derived and never re-centers
  the geometry.

`translate: x y` offsets a child's origin from the datum — the universal nudge,
unchanged ([SPEC §6](SPEC.md)). Children paint in **source order** (later on top), so
overlaps, punched holes, and cutaways compose with no boolean operations. Annotations
always paint **above** all geometry ([§13](#13-lowering--render)).

The **ground** is the first-declared geometry child: mates resolve by walking outward
from it, moving the *other* side of each mate ([§7](#7-mates--)). There is no ground
marker — to reground, reorder the declarations (source order is Lini's universal
tiebreak).

### Scale & units

Numbers inside a drawing are **drawing units** — draw in millimetres and the dimensions
read in millimetres. `scale:` (a number > 0, default 1) multiplies **geometry** into
pixels at lowering; dimensions always report **pre-scale drawing units**, so rescaling a
view never touches a value.

Scale splits the world in two, the drafting way — the model scales, the sheet does not:

| Scales with `scale:` (model) | Stays sheet-space px (chrome) |
|---|---|
| `draw:` paths, `points:`, `width` / `height` of geometry | `stroke-width` (shapes and links), markers |
| `translate:` of geometry children, mate `gap:` | `font-size`, all text |
| `pattern:` offsets and radii | hatch pitch, center-mark overhang |
| | dimension offsets, stacking pitch, leader gaps |

`unit: "mm"` (the chart-axis property, same meaning) appends a suffix to
**auto-measured** values only — labels are verbatim. Default: none (drafting states
units once, in the title block).

### Sizing

A drawing's bbox is the union of its children's **paint** bboxes *and* its annotations
(dimensions stack outside the geometry and count), plus `padding`. An explicit
`width` / `height` is a floor, per core. Inside the drawing, measurement uses the
**geometry bbox** instead — the drawn path, stroke excluded — so line weight never leaks
into a dimension or a mate; the paint bbox (stroke included, per core) is only for
layout and the canvas.

`gap`, `align`, `justify`, `direction`, and `divider` have no role on a drawing
container (it owns placement) and are ignored.

---

## 3. Anchors

The endpoint form is the core one ([SPEC §9](SPEC.md)) with a wider point set, valid
only in a drawing scope:

```
anchor = id { "." id } [ ":" point ]
point  = center                                          (the default)
       | top | bottom | left | right                     (side extremes)
       | top-left | top-right | bottom-left | bottom-right   (corners)
       | name                                            (authored in draw:, §4)
```

- Points are taken on the node's **geometry bbox** (stroke excluded, [§2](#2-the-drawing-container)).
  A side is that side's midpoint; a corner is the bbox corner; `center` the bbox center.
- Corner names glue **vertical word first**, matching `pin`'s vocabulary
  (`pin: top left` → `:top-left`); the reversed order errors with a did-you-mean
  ([§16](#16-errors)). Corners (and `:center`) are drawing-scope only — elsewhere the
  core four sides stand.
- A `|sketch|` **authors** its own names with the `:name` sigil in `draw:`
  ([§4](#4-the-sketch-pen)) — attached to a call it names the call's drawn product (an
  edge, a fillet arc, a circle: `right(50):flank`, `fillet(3):r1`), freestanding it
  names the pen's current point (`right(38):thread :tl up(8)`). Declared in the pen,
  selected on an endpoint — the same declare/select symmetry as `#id` (bars vs rule
  head): **`:` is the point sigil.** The built-in names win (`:left` cannot be
  authored, [§16](#16-errors)); an unknown name errors with suggestions. Authored
  names are **not** duplicated by `mirror:` / `pattern:` — a name addresses the drawn
  original ([§18](#18-deferred)).
- For **measurement** every anchor reduces to a representative point — a point is
  itself, an edge or arc its midpoint, a bbox name its bbox point — and a named edge
  additionally carries its **direction**, which drives a dimension's axis and the
  angular op ([§8](#8-dimensions)).
- Dot-paths walk into children as everywhere (`pump.barrel:right`), resolve in the
  statement's scope, and never search ([SPEC §9](SPEC.md)).
- A **radial-patterned** node's position is its ring **center**; a grid-patterned node's
  is its **seed** copy ([§6](#6-patterns)) — each is the point drafting locates.

**The anchor aims; the outline lands.** A leader or callout arrow is a ray from its text
toward its anchor's representative point (an edge aims at its midpoint, a point at
itself), and the tip sits at the ray's *first intersection with the drawn path* — so
aiming at the bbox corner of a filleted plate touches the fillet arc itself, and a
leader onto any profile lands on the profile, never floating at a bbox point.
Dimension extension lines, by contrast, spring exactly from the anchor points.

---

## 4. The sketch pen

`|sketch|` is a new closed primitive ([SPEC §7](SPEC.md)): a pen that folds to a path.
It **requires `draw:`** (as `|poly|` requires `points:`), paints like any closed
primitive (`fill`, `stroke`, `stroke-width`, default `--fill` / `--stroke` / 1.5), and
its bbox is computed from the geometry.

`draw:` is a left-to-right list of **bare calls** — the same value-position rule as
`repeat()` / `rgb()`, so it adds no value grammar; the value runs to its `;` and may
span lines. An argument may be a number, a function call, or a backtick expression
(`right(`w/2`)`) — operators stay fenced, per core.

| Call | Does |
|---|---|
| `move(x, y)` | set the start / begin a new subpath — **absolute**, in the sketch's own frame |
| `left(n)` / `right(n)` / `up(n)` / `down(n)` | an orthogonal run; the verb is visual (`up` goes up on screen) |
| `line(dx, dy)` | a relative straight segment |
| `angle(deg, n)` | a run of length `n` at a bearing — **0 = up, clockwise** (90 right, 180 down, 270 left) |
| `arc(dx, dy, r)` | the **minor** arc to a relative point; `r > 0` sweeps clockwise, `r < 0` counter-clockwise; `\|r\|` ≥ half the chord or it errors |
| `arc(r, deg)` | a **tangent** arc: continue the current heading, sweeping `deg` on radius `r > 0` — `deg > 0` turns clockwise; the heading updates by `deg` |
| `curve(dx1, dy1, dx2, dy2, dx, dy)` | a relative cubic bézier (the advanced 10 %) |
| `fillet(r)` / `chamfer(c)` | **corner modifiers** between two segments — trim both legs and drop in a tangent arc / bevel; they draw nothing alone, and are an error anywhere but between two segments |
| `circle(r)` | a circle subpath centred on the current point; the point and heading are unchanged |
| `close()` | close the current subpath |

**Coordinates.** The pen's frame keeps the core orientation — y grows **down**, like
`points:` and `translate:` everywhere in Lini — but the verbs and bearings are visual,
so a profile written with `up`/`right`/`arc` never types a signed y. Only `move()`,
`line()`, and `curve()` expose raw coordinates; there, negative y is up. Heading state:
each drawing call leaves the pen heading along its own direction; `angle()` and the
tangent `arc()` read and update it.

**Subpaths & holes.** A second `move()` starts a new subpath; fill is **even-odd**, so
an inner subpath reads as a hole. An outline with a bore is one shape; composite parts
are overlapping nodes — no boolean operations exist or are needed. An open path (no
`close()`, no `mirror:`) is legal; `fill` paints it as if closed (SVG semantics).

### Naming — `:name`, the point sigil in the pen

Any drawn thing the pen makes can carry a **name**, written with the point sigil
([§3](#3-anchors)) in two positions of one rule:

| Position | Names | Example |
|---|---|---|
| **attached** — glued to a call | that call's drawn product: an edge, a fillet arc, a bevel, a circle, a `close()` seam | `right(50):flank`, `fillet(3):r1` |
| **freestanding** — between calls | the pen's **current point** | `right(38):thread :tl right(32):land` — a station with no drawn edge |

A freestanding `:name` draws nothing and changes nothing; at a `fillet` / `chamfer`
corner it records the **theoretical sharp corner** — the point drafting measures (the
arc itself is named on the modifier: `fillet(3):r1`). At a corner, `fillet` / `chamfer`
and a freestanding `:name` may sit in either order. `move()` takes no product name;
name its landing with a freestanding `:name` after it (`move(-90, 0) :origin`).

### `mirror:` — draw half, get the whole

`mirror:` is a `|sketch|` property: it reflects the entire drawn path and unions the
copy. The value is a **list**, applied left to right, each item reflecting the union so
far — two items give a 4-fold part:

| Item | Axis (through the pen origin) | Gives |
|---|---|---|
| `x-axis` | the horizontal axis (y = 0) | top ↔ bottom symmetry |
| `y-axis` | the vertical axis (x = 0) | left ↔ right symmetry |
| a number `45` | the line at that bearing (`angle()`'s convention: 0 = up, clockwise) | angled symmetry |

```
mirror: x-axis;            // half → whole
mirror: x-axis y-axis;     // quarter → whole
mirror: 45 135;            // double angled seam
```

The names are deliberately unambiguous: `x-axis` means *about the x-axis* and cannot be
misread as "flip x". What mirroring does is decided **per subpath**, and both intents
fall out of one rule each:

| Subpath | Mirrored result |
|---|---|
| **open** | **fused** — the copy joins end-to-end (original A→B, then the reflection walked back B′→A′, straight seam segments where an endpoint sits off the axis); the edge lying on the axis is the invisible seam. *Draw the half, get the whole.* |
| **closed** | **duplicated** — a reflected second copy of the whole subpath. *Draw one ear, get both.* |

So leave a half-profile open (a `close()` there means the other thing — and would draw
a visible spine down the axis, the cue you meant the open form), and close a feature
you want twice. A fused mirror also generates its axis `|centerline|` — auto chrome,
[§10](#10-line--material-conventions); a duplicated subpath generates none.

`mirror:` runs before `pattern:` and before placement ([§13](#13-lowering--render)): it
is part of building the node's geometry, so anchors, dimensions, and mates all see the
whole part.

---

## 5. Features & composition

**A part's features ride in its `[ ]`.** A child of a geometry node places at that
node's datum ([§1](#1-mental-model)) and is **rigid with it** — mate or translate the
part and its holes travel along. Sibling features work for a single fixed part, but
anything that will ever move should own its features:

```
|rect#plate| { width: 120; height: 70; } [
  |hole#pin| { width: 10; translate: -35 20; }
]
plate:left <-> plate.pin { side: top }        // dot-path to the feature
```

### `|hole|`

A built-in drawing feature type (as `|note|` is to sequences), over `|oval|`:

- **`width` is required** — the diameter (equal height implied; it is round).
- It **punches** by paint order: `fill: --bg`, `stroke: --stroke` — over a filled or
  hatched part it reads as a through-hole, and in a section the background shows
  through, hatch-exempt with no special case.
- It draws its own **center marks** — a dash-dot crosshair overhanging the circle by a
  fixed sheet-space amount. No knob: a hole without marks is a plain `|oval|`.
- The unary `(-)` / `(-` callouts read its diameter / radius directly
  ([§8](#8-dimensions)), and `pattern:` prefixes the count ([§6](#6-patterns)).

Counterbores, countersinks, and threads are defines or deferred ([§18](#18-deferred)).

### Composition is the geometry model

There is no CSG. A part is one `|sketch|` — its stations, sections, and corners named
with `:name` where dimensions will land ([§4](#4-the-sketch-pen)) — or **composed**
from overlapping nodes in paint order. One geometry mechanism; composition is for
what is genuinely a separate piece:

- A multi-section turned part is **one profile**: name each surface run
  (`right(32):land`), each shoulder (`up(1.5):sh1`), each treated corner
  (`fillet(3):r1`) — the dimensions read them directly ([§8](#8-dimensions),
  [§17](#17-examples), barrel). A stack of loose `|rect|`s in a `|row| { gap: 0 }`
  works too (it is all core), but the profile is the canonical form.
- A bore in a **section** view is not round on the page — model the gap as a
  `--bg`-filled `|rect|`: it punches the hatch and its edges anchor a `(-)` dimension
  ([§17](#17-examples), bushing). The same paint-order trick cuts any shape a profile
  would contort for.
- The escape hatches are the core ones: `|poly|`, `|path|` (raw SVG), `|image|` /
  `|path| { src: … }` for imported detail.

### Parts library

Plain **defines** — no engine support, just bundled geometry and paint: `|washer|`,
`|bearing|`, `|screw|`, `|balloon|`, `|finish|`, a book's own `|sprue|` or `|gear|`. A define's `draw:` is its default; an instance overrides it wholesale
(per-property replacement, core [SPEC §13](SPEC.md)) or parametrises via stylesheet
functions:

```
{
  |steel::sketch|   { fill: hatch(45, 6); }
  |balloon::oval|   { width: 16; fill: --fill; stroke: --stroke; font-size: 11; }
}
```

---

## 6. Patterns

`pattern:` replicates a node about its own position — a **node property**, legal on any
node in any layout, though its chrome belongs to drawings:

| Form | Copies |
|---|---|
| `pattern: grid(cols, rows, dx, dy)` | `cols × rows` copies at offsets `(i·dx, j·dy)`; the **seed is copy one** and the node's position stays the seed's |
| `pattern: radial(count, radius)` | `count` copies **on** a circle of `radius` centred on the node's position, first at bearing 0 (up), clockwise; the node's position is the **ring centre** and no copy is drawn there |

The two datums are deliberate and match drafting practice: you locate a grid by its
first hole and a bolt circle by its **centre**. Dimensions and mates to a patterned id
use that position ([§3](#3-anchors)); per-copy addressing is deferred
([§18](#18-deferred)).

- The node's bbox becomes the **union** of the copies (layout, canvas, and the drawing's
  sizing all see it).
- A radial pattern draws its **pitch circle** as a generated `|pitch-circle|`
  ([§10](#10-line--material-conventions)) — the bolt-circle chrome drafting expects; a
  grid pattern adds none.
- Each copy repeats the node's full lowering — a patterned `|hole|` punches and
  center-marks per copy.
- A unary callout on a patterned feature prefixes the count: `pin (-)` → `2× ⌀10`
  ([§8](#8-dimensions)).
- Constraints: counts ≥ 1 (grid) / ≥ 2 (radial), `radius > 0`; offsets are drawing units
  and scale as geometry.

---

## 7. Mates — `@`

`a:anchor @ b:anchor` is a **positioning relationship** between two geometry nodes: it
moves a part and **draws nothing**, so it can never be confused with an annotation.
Grammatically it is one more link operator — endpoints, chains, and fans all parse as
usual ([§15](#15-grammar)); a mate takes **no label** and no markers.

```
nozzle:left @ barrel:right              // abut those faces
cap @ barrel                            // no sides — concentric (origins coincide)
nozzle:left @ barrel:right { gap: 4 }   // 4 units of daylight along the mate normal
piston:left @ bore:left  { gap: -6 }    // negative gap = inserted 6 deep
```

- **Resolution.** Mates resolve after datum placement, walking outward from the
  **ground** (the first-declared child, [§2](#2-the-drawing-container)): each mate moves
  the side *not yet connected* to the ground, translating that whole scope-level child
  (rigid, features and all). `a @ b` and `b @ a` are the same mate — grounding, not
  operator order, decides who moves.
- A mate whose ends are **both** already connected to the ground is over-constrained —
  an error naming the cycle. A mate between two nodes in an unconnected **island**
  positions the later-declared relative to the earlier; the island's first-declared node
  holds its datum placement (a local ground). Deterministic, source-ordered.
- **Directed vs point anchors.** A mate between **directed** anchors — sides, or a
  sketch's named edges ([§3](#3-anchors)) — aligns them flush; the two directions must
  be parallel (`a:left @ b:top` errors), and a named edge lets a part seat against an
  **interior** face (`ring:right @ housing:shoulder`). `gap:` offsets along the shared
  normal and may be **negative** (overlap — the one place `gap` goes below zero; the
  container property stays ≥ 0). A mate between **point** anchors (`center`, a
  freestanding name) makes the points **coincide** — the bare `a @ b` is the
  origin-to-origin case (`a:pin @ b:socket` aligns two exact positions) — and has no
  normal, so `gap:` there is an error.
- **Rotate, then mate.** A part's `rotate:` applies to its geometry first; the mate
  aligns the *rotated* anchor, so a part seats at its angle and the turn pivots on the
  mate point. The mated child's own `translate:` applies **after** the mate — the
  universal post-placement nudge, here a lateral slide along the face.
- A `pin:` on a mated child is ignored with a warning (the mate owns its position).
- A mate between two features of **one** part is an error — a part is rigid.
- Mates are valid only where children **datum-place** — a drawing's scope. Inside a
  layout-owning child (a `|row|`, a `|table|`) the flow has already decided every
  position, so a mate there is the same over-constraint error. Dot-paths reach into
  parts (`pump.barrel:right @ frame:left`), moving the scope-level child that contains
  the moving anchor.

---

## 8. Dimensions

A dimension is a **link**; the operator carries the kind and supplies the glyph you
can't type. The statement is the core link statement — endpoints, op, then the node
tail — with one relaxation: some ops may omit the right-hand endpoints (the exact set:
[§15](#15-grammar)).

| Write | Is | Renders |
|---|---|---|
| `a:left <-> b:right` | linear dimension | extension lines, arrows, `25` |
| `a:left <-> b <-> c:right` | a **chain** of dimensions | each hop its own dim, sharing one row |
| `body:top (-) body:bottom` | **diameter, binary** — a linear measure, ⌀-formatted | `⌀44` |
| `pin (-)` / `body:land (-)` | **diameter, unary** — a round feature, or a named edge / point of a mirrored profile | `2× ⌀10` / `⌀42` |
| `boss (-` / `body:r1 (-` | **radius, unary** — a round feature, or a named fillet arc | `R12` / `R3` |
| `body:flank <) body:base` | **angular** — between two line-like anchors | arc + `40°` |
| `body:taper <)` | angular, unary — a named edge of a mirrored profile vs its twin | arc + `30°` (included) |

### Auto-measure — the smart label

A dimension with **no label** renders its **measured value**: the distance between its
anchors ([§3](#3-anchors)) in drawing units, projected on the dimension's axis — or a
round feature's diameter/radius for the unary forms. Values round to at most 2 decimals,
trailing zeros trimmed; `unit:` appends its suffix ([§2](#2-the-drawing-container)).
Measurement happens **after mates resolve** — a dimension across an assembly reads the
seated geometry.

The text composes from four sources, each owning one thing:

| Source | Owns | Example |
|---|---|---|
| the **op** | the glyph | `(-)` → `⌀`, `(-` → `R`, `tol:` → `±` |
| the **geometry** | the number | `10` |
| the **label** | the words | see below — replaces or follows by form |
| **`tol:`** | the tolerance, appended last | `±0.1` |
| **`pattern:`** | the count prefix (unary callouts) | `2× ` |

**The label's seat follows the statement's shape.** A **two-ended** dimension's label
*is* its text — it **replaces the number** (the glyph stays on a binary `(-)`):
`a <-> b "180"` reads exactly `180`, the honest override for schematic, exaggerated, or
nominal-≠-drawn figures — auto-measure is the default, not a vow. A **one-ended**
callout's label is commentary that **follows the measured callout**:
`pin (-) "H7"` → `2× ⌀10 H7`, `bolt (-) "THRU"` → `⌀10 THRU`; to replace a callout
wholesale, use a plain leader (`pin <- "⌀10 H7"`).

So `pin (-) { tol: H7 }` renders `2× ⌀10 H7` (via `tol:`, the semantic path — the label
form above is its freeform twin); `plate:left <-> plate:right "120.0" { tol: 0.2 }`
renders `120.0 ±0.2`.

**`tol:`** takes three forms: `tol: 0.1` → `±0.1`; `tol: +0.2 -0.05` → stacked upper /
lower deviations, typeset small (0.7 × font) and raised/lowered; `tol: H7` → a fit
class, appended as text. (Signed numbers are core lexing; a bare ident is a core value —
zero grammar.)

### Axis & anchors

- A **directed anchor sets the axis**. A side name gives its direction
  (`left`/`right` → measure horizontally, `top`/`bottom` → vertically), and so does a
  named edge (a vertical shoulder → a horizontal dim across it; a horizontal surface →
  a vertical one). One directed anchor is enough (`plate:left <-> pin` is horizontal);
  two must agree — perpendicular directions in one `<->` error, pointing at `<)`.
- **Point ↔ point** (centers, freestanding names) measures the dominant delta (the
  larger |Δ| component; tie → horizontal). True point-to-point *aligned* dims are
  deferred ([§18](#18-deferred)).
- The binary `(-)` measures exactly like `<->` and prepends ⌀ — for a turned part's
  silhouette (`body:top (-) body:bottom`).
- **`<)`** reads two **line-like** anchors — a named edge, a `|line|` /
  `|centerline|` node, or a bbox side — and measures the angle between their
  directions; the arc is drawn at their (extended) intersection, the value riding the
  arc. Point anchors have no direction and error.
- **The unary forms and the mirror.** `(-)` unary reads a **round** feature's diameter
  (`|oval|` lineage — `|hole|`, `|balloon|`, a circle define) — or, on a named edge or
  point of a **mirrored** sketch, the **symmetric span**: twice the anchor's offset
  from the mirror axis, the ⌀ at that surface or station (`body:land (-)` → `⌀42`).
  `<)` unary reads a named edge of a mirrored sketch against its own reflection — the
  **included angle** of a taper or cone (`body:taper <)`). `(-` unary reads a round
  feature — or a named `fillet` arc, whose radius the pen knows (`body:r1 (-` → `R3`);
  it needs no mirror and has no binary form. Unary `(-)` / `<)` on an unmirrored name
  error — there is no axis to double about; unary anything on an unnamed, non-round
  node errors naming the binary form.

### Placement & stacking

A dimension sits **outside** the geometry, on a `side:` (the chart-axis property, reused):

- Default: a horizontal dim goes `bottom`, a vertical one `right`; anchors that both sit
  on one edge pull the dim to that edge (`a:top <-> b:top` stacks on top). `side:` must
  suit the axis — a horizontal dim stacks on `top`/`bottom` only ([§16](#16-errors)).
- Dims sharing a side pack into **rows**, `dim-pitch` apart, the first row `dim-offset`
  from the geometry's extent. Each dim, in **source order**, takes the innermost row
  where its span — text included — overlaps nothing already placed: a chain shares one
  row, and dims over different stations share rows instead of each opening one — the
  packing a real sheet uses. `gap:` pins a dim's own offset; `translate` nudges one
  freely, per core.
- Anatomy, all baked sheet-space constants ([§13](#13-lowering--render)): extension
  lines spring from the anchors with a small gap at the feature and overshoot past the
  dim line; arrows are **drafting-slender** — the drawing lowers its own links, so its
  arrowhead is the long, narrow, filled form (≈3:1), sized by the dim's `stroke-width`,
  while `->` elsewhere keeps the core marker; the value sits centred **above** the line.
  A span too narrow for text + arrows flips its arrows outside the extension lines and
  slides the text past the nearer one, away from the geometry — deterministic, no solver.
- Dimensions are links, styled like any link ([SPEC §9](SPEC.md)): `stroke` /
  `stroke-width` / `stroke-style` via the `|-|` selector or the dim's own `{ }`. A
  drawing scope defaults its links to `stroke-width: 1` (`|-| { stroke-width: 1 }`), so
  annotation lines read one weight under the 1.5 object lines. Dimension text uses the
  link-label defaults (`font-size: 11`, normal weight); `font-size:` / `color:` on the
  dim restyle it, per core.
- `along:` has no role in a drawing (as in a sequence) and is ignored.

---

## 9. Leaders & notes

A **callout** is a one-ended link: feature first, a tip-first op, then the text — and
**source order mirrors the drawn leader**: the tip glyph hugs the feature name, the
line runs toward the text. The text is formally the link's **label**, so everything
core says about labels (placement in the `[ ]`, styling, one-inline-label) applies
verbatim:

| Op | Tip on the feature | For |
|---|---|---|
| `<-` | arrow | an edge or outline |
| `*-` | dot | a leader landing **within** an outline — a face, a region |
| `>-` | **datum** triangle | a datum feature (in a diagram `>-` is the crow op — the scope reinterprets it, as a sequence reinterprets `->`) |
| `(-` | the R glyph — the **left half of `(-)`**, as a radius is half a diameter | a radius; auto-measured ([§8](#8-dimensions)) |

```
bolt <- "THRU"                              // leader: arrow lands on the hole
face *- "Ra 1.6"                            // dot — a face / surface note
body:seat >- "A"                            // datum triangle on the surface
pin (-)                                     // auto-measured diameter callout (§8)
boss (- "MAX"                               // → R12 MAX — a callout label follows
                                            // the measured value (§8)
bolt <- [ "R3 TYP" { translate: 30 -24 } ]  // a styled / nudged text — the core
                                            // styled-label form, unchanged
```

- **Markers land on feature ends, never on text.** A callout has **one** tip, so the
  singular `marker:` overrides it (`marker-start`/`-end` have no role); the set gains
  **`datum`** — the filled triangle `>-` lowers to, one op ↔ marker pair like every
  core op. `<->` and `@` cannot be one-ended; a one-ended callout with no label is an
  empty leader — an error; and a one-ended `->` / `-*` errors the other way around:
  a leader points *back* at its feature — write `a <- "…"`.
- **Text placement.** The text auto-places **outward**: along the ray from the drawing's
  datum through the feature, just past the geometry union (`note-offset`), horizontal;
  a feature *at* the datum defaults to the top-right diagonal. `side:` on the callout
  picks the direction instead (a side or a corner — a direction, not a stack); a styled
  label's `translate` nudges from there. The arrow tip ray-casts onto the drawn outline
  ([§3](#3-anchors)).
- **The leader makes the note.** A callout's text lowers to a bare text leaf — drafting
  callouts are unboxed. A **boxed** note is the `|note|` type (reused from sequences —
  its `over:`/`left:`/`right:` are sequence-only; in a drawing it places like any child)
  wired with an ordinary two-ended link. A **balloon** is the `|balloon|` define plus a
  link. Bare `"…"` text in a drawing stays a plain text leaf ("SECTION A-A",
  "NOT TO SCALE") — it is the *leader* that makes something a callout.
- A plain `a -> b` between two geometry nodes is a straight annotation arrow (a flow
  direction, an exploded-view path) — drawn by the drawing, not the router.
- Chains and fans keep their core expansion for two-ended links; a **label-terminated**
  statement is single-hop (`a <- b <- "x"` errors; fan leaders `a & b <- "2× R5"` are
  deferred, [§18](#18-deferred)).

---

## 10. Line & material conventions

**`hatch()`** is a paint function beside `gradient()` ([SPEC §12.3](SPEC.md)), valid on
`fill` only:

| Form | Result |
|---|---|
| `hatch(45)` | section lines at 45°, default pitch 6 |
| `hatch(45, 6)` | explicit pitch (sheet-space px — hatch never scales, [§2](#2-the-drawing-container)) |
| `hatch(45, 6, --gray-deep)` | explicit line colour (default `--stroke`) |
| `hatch(45 -45, 6)` | a space-group of angles — cross-hatch |

Angles use the pen's bearing convention. Each distinct hatch emits one `<pattern>` in
`<defs>`, deduplicated and shared like gradients and filters ([SPEC §14](SPEC.md));
colours are ordinary paints, so hatching themes, flips dark/light, and bakes. Hatch
line width is fixed (0.75) — a texture, not a stroke.

**`stroke-style: center`** is a new stroke-style value — the dash-dot **centerline**
pattern — on shapes and `|line|`s (a link's `stroke-style` keeps its core set — no
`center`). Two built-in types carry it, one per geometry:

| Type | Base | Requires | Draws |
|---|---|---|---|
| `\|centerline\|` | `\|line\|` | `points:` | a dash-dot line — an axis, a symmetry line, a spoke |
| `\|pitch-circle\|` | `\|oval\|` | `width:` — the **diameter**, the `\|hole\|` convention | the dash-dot circle through a pattern's centers (the bolt circle); being round, `bc (-)` dimensions its PCD (`⌀60`) |

Both default `stroke-width: 1`, `fill: none`. A manual `|pitch-circle|` covers what
`pattern:` can't — unequally spaced holes still share one drawn circle.

**Auto chrome — one mechanism, three producers.** The centerlines drafting always
draws are **generated children** of these two types, so the cascade styles or removes
them with no dedicated knobs (`|sketch| |centerline| { stroke: none }`):

| Producer | Generates |
|---|---|
| a **fused** `mirror:` ([§4](#4-the-sketch-pen)) | a `\|centerline\|` along the mirror axis, overhanging the profile (a duplicated — closed — subpath generates none) |
| `pattern: radial` ([§6](#6-patterns)) | a `\|pitch-circle\|` through the copies |
| a `\|hole\|` ([§5](#5-features--composition)) | its center-mark crosshair |

**Hidden edges** are the core `stroke-style: dashed` on a separate `|line|`/`|sketch|`
child (one node has one stroke style). `phantom` (dash-dot-dot) is deferred.

---

## 11. Assemblies & nesting

There is no `|assembly|` type: **an assembly is a drawing whose children mate** — and
drawings **nest**. A child `|drawing|` is one rigid body from outside: its internal
mates, dims, and features are sealed in its own `[ ]` (the core sealed-body law), its
geometry bbox is its parts' union, and it grounds, mates, and anchors like any part:

```
|drawing#gearpump| [ …parts, mates, dims… ]
|drawing#motor|    [ … ]

gearpump:left @ motor:right                     // mate two sub-assemblies
motor.shaft:right @ gearpump.rotor:left         // or reach in — written at the level
                                                // where both ends are visible (core law)
```

Build sub-assemblies in isolation, then seat them — the same statement vocabulary at
every level. A project that wants the word writes `|assembly::drawing| { }`, a one-line
define, not a language feature.

**Balloons & BOM.** Item balloons are the `|balloon|` define + a leader
(`b1 -* nozzle`); the parts list is a core `|table|`, placed as a sibling of the drawing
in an ordinary flow. Auto-numbering and auto-BOM are deferred ([§18](#18-deferred)).

**Exploded views** are deferred but the model carries them: `explode: N` on a drawing
would scale every **directed** mate's separation along its normal — unmated overlaid
children (composed geometry) stay put, which is the correct split: overlay composes one
part; mates relate parts; only relationships explode. Balloons follow their parts;
dimensions would honestly re-measure, so exploded views carry balloons, not dims.

---

## 12. Views & titles

A multi-view sheet is ordinary layout: drawings in a `|row|` / `|grid|`, each view its
own scope with its own `scale:` (a 2:1 detail view is just `scale:` doubled — its dims
still read true, [§2](#2-the-drawing-container)). There is no `|view|` type, no
projection engine; alignment between views is the author's (or a future layout's,
[§18](#18-deferred)) business.

**A drawing's smart label is its title, placed *below*** — it lowers to a `|footnote|`
(the existing bottom-centred caption template), because drafting titles sit under the
view: `|drawing| "SECTION A-A"`. Style every title at once with
`|drawing| |footnote| { … }`, per core.

---

## 13. Lowering & render

`layout: drawing` resolves in the **layout** phase ([SPEC §18](SPEC.md)) — geometry must
exist before it can be measured:

1. **Geometry** per child, bottom-up: fold `draw:` calls to a path, collect its
   `:name`d points and edges, apply `mirror:`, expand `pattern:` (nested drawings
   lower first, becoming rigid subtrees). Compute each node's geometry bbox (path,
   stroke excluded) and paint bbox (core).
2. **Place** children: origins on the datum, `translate:` applied.
3. **Mates**: walk from the ground, seat each mated child (rotate first, translate
   after); flag cycles and over-constraints.
4. **Measure**: resolve every annotation's anchors against the seated geometry; compute
   values; compose texts (op glyph + number/label + `tol:` + pattern count + `unit:`).
5. **Annotate**: assign dims to sides and pack the side rows in source order
   ([§8](#8-dimensions)); auto-place callout texts; ray-cast leader tips onto outlines.
6. **Lower** to primitives at baked coordinates: sketch → `|path|`; hole → `|oval|` +
   center marks; the auto chrome → generated `|centerline|` / `|pitch-circle|`
   children ([§10](#10-line--material-conventions)); dim → extension `|line|`s + a
   marker-tipped dimension `|line|` + text; an angle → its arc `|path|` + text;
   leader → `|line|` + marker (`datum` for `>-`) + text; hatch → a `<defs>`
   `<pattern>`; balloon / note / table — already primitives. A link's wire paint
   (`stroke` / `stroke-width` / `stroke-style`) is already the `|line|`'s, as in a sequence.
7. **Scale**: multiply geometry by `scale:`; chrome stays sheet-space
   ([§2](#2-the-drawing-container)). Emit with geometry in source order and annotations
   **above** all of it (the drawing's one draw-order override, like a chart's semantic
   order; `layer:` still wins).

The output is an ordinary primitive subtree — theming, the palette, gradients,
`--bake-vars`, `fmt`, and byte-for-byte determinism apply with no drawing-specific code.
The **parser is scope-blind** (as with charts): the ops and forms parse everywhere and
*mean* drawing only in a drawing scope — elsewhere they error at resolve
([§16](#16-errors)). `lini desugar` shows the type desugaring (a `|drawing|` is a
`|block|` wearing `.lini-drawing`; a `|sketch|` carries its `draw:`); the measured,
lowered geometry is a layout-time artefact, exactly like a chart's bars or a routed
link.

**Baked constants** (sheet-space, the [SPEC §12.5](SPEC.md) table's new rows):

```
dim-offset 18      dim-pitch 16            dim-ext-gap 3     dim-ext-overshoot 3
dim-arrow 9 × 3    note-offset 14          center-mark-overhang 4
hatch pitch 6      hatch line-width 0.75   drawing link stroke-width 1   tol-stack 0.7
```

---

## 14. Properties

New properties, and core ones reused with their core meaning. All paint, text, and
marker properties are the core ones.

| Property | On | Value | Notes |
|---|---|---|---|
| `scale` | `\|drawing\|` | number > 0 | drawing units → px; geometry only ([§2](#2-the-drawing-container)). (An `\|axis\|`'s `scale:` is the chart homonym — both name the data→pixel mapping.) |
| `unit` | `\|drawing\|` | quoted string | suffix on auto-measured values only. |
| `draw` | `\|sketch\|` | pen items — calls and `:name`s | **required**; [§4](#4-the-sketch-pen). |
| `mirror` | `\|sketch\|` | list of `x-axis` / `y-axis` / bearing | reflect + union, left to right. |
| `pattern` | any node | `grid(c, r, dx, dy)` / `radial(n, r)` | replicate about its position ([§6](#6-patterns)). |
| `width` | `\|hole\|` `\|pitch-circle\|` | number | **required** — the diameter. |
| `tol` | a dimension | `t` / `+u -l` / fit ident | tolerance text ([§8](#8-dimensions)). |
| `side` | a dimension / callout | side (dims) / side or corner (callouts) | which side it stacks on / which way the text sits. |
| `gap` | a dimension / a mate | number | dim: its offset from the geometry. Mate: separation along the normal — **may be negative** (overlap). |
| `translate` / `rotate` | core | core | placement nudge / rotate-then-mate. |
| `explode` | `\|drawing\|` | number | **deferred** ([§18](#18-deferred)). |

`fill` accepts `hatch(…)` ([§10](#10-line--material-conventions)); `stroke-style` gains
`center`; `marker` gains `datum` ([§9](#9-leaders--notes)). `routing`, `clearance`,
`along`, and the container `gap`/`align`/`justify` have no role in a drawing scope.

---

## 15. Grammar

Drawings add **four op tokens, one relaxation, and one value form** to
[SPEC §17](SPEC.md); everything else is nodes, declarations, and the existing value
forms.

```
link      = endpoints draw_op [ endpoints ] { draw_op endpoints } tail
draw_op   = link_op | "@" | "(-)" | "(-" | "<)"
tail      = [ string ] [ classes ] [ style ] [ label_block ]     # the core node tail

pen_item  = call [ ":" ident ]         # a pen call, optionally naming its product
          | ":" ident                  # freestanding — names the current pen point
```

- The right-hand `endpoints` may be **omitted** only for `<-`, `*-`, `>-`, `(-`,
  `(-)`, and `<)` — the one-ended callout / mirrored reading, whose label is its text
  ([§9](#9-leaders--notes)). One token of lookahead decides: after the op, an ident is
  an endpoint; a string, `.`, `{`, `[`, or end-of-statement is the tail. `<->` and `@`
  require both ends.
- The new ops lex as glued tokens like every link op (no internal space). Chains and
  fans parse per core; mixing ops in a chain stays an error; a label-terminated
  statement is single-hop.
- `@` takes no label and no `[ ]`; `(-)` and `<)` carry both binary and unary
  readings; `(-` and the callout ops are one-ended-only toward text (a two-ended
  `a <- b` stays the core arrow); `>-` reads as the datum leader in a drawing and the
  crow op elsewhere.
- `pen_item` is the one **value-grammar** addition — a `:` suffix after a call's `)`,
  or a freestanding `:ident`, both only inside a `draw:` value ([§4](#4-the-sketch-pen)).
- Endpoint `:side` gains the corner and `center` names — and a sketch's **authored
  names** ([§3](#3-anchors)) — in drawing scope; same token shape, wider name set
  (built-in names win).
- The pen calls, `grid` / `radial`, and `hatch` are **call names**, contextual before
  `(` like `rgb` / `repeat` ([SPEC §19](SPEC.md)).
- New built-in **type names**: `drawing`, `sketch`, `hole`, `centerline`,
  `pitch-circle` (`note` is reused) — protected from shadowing, free as ids, per core.

---

## 16. Errors

Format and discipline per [SPEC §16](SPEC.md): compile-time, with a span.

| Condition | Message |
|---|---|
| `\|sketch\|` without `draw:` | `'\|sketch\|' requires 'draw'` |
| `\|hole\|` / `\|pitch-circle\|` without `width:` | `'\|hole\|' requires 'width' — its diameter` |
| Unknown pen call / arity | `unknown draw call 'X'` / `'arc' takes (dx, dy, r) or (r, deg)` |
| `fillet` / `chamfer` misplaced | `'fillet' modifies the corner between two segments` |
| Arc radius too small | `arc radius N is smaller than half the chord` |
| Bad `mirror:` item | `'mirror' takes x-axis, y-axis, or a bearing` |
| Drawing statement outside a drawing | `'(-)' draws a dimension — it belongs in a 'layout: drawing'` (same for `(-`, `>-`-as-datum, `@`, corner anchors, `tol:`, …) |
| Unknown endpoint | `dimension endpoint 'X' not found at <scope>` + suggestions — **never auto-created** |
| Corner order | `':right-top' is not an anchor — did you mean ':top-right'?` |
| One-ended `<->` / `@` | `a linear dimension measures two anchors` / `a mate seats two parts` |
| Empty one-ended leader | `a leader needs its text — 'bolt <- "THRU"'` |
| One-ended `->` / `-*` | `a leader points back at its feature — write 'a <- "…"'` |
| Unary `(-)` / `(-` on a non-round node | `'(-)' reads a round feature's diameter — measure a span with 'a (-) b'` |
| Unary `(-)` / `<)` on an unmirrored name | `'(-)' on ':land' needs 'mirror:' — no axis to double about` |
| `<)` on a point anchor | `an angle reads two edges — a named segment, a '\|line\|', or a side` |
| `:name` shadows a built-in point | `':left' is a built-in anchor — pick another name` |
| Unknown authored name | `no point ':step' on 'body'` + suggestions |
| Duplicate `:name` in one `draw:` | `':step' is already named at L:C` |
| Binary `(-` | `a radius reads one round feature` |
| Label on a mate | `a mate takes no label` |
| `gap:` on a point mate | `a point mate coincides — 'gap' needs directed anchors (sides or named edges)` |
| Non-parallel mate directions | `mated anchors must face along one axis — 'a:left @ b:top' has no shared normal` |
| Over-constrained mate | `mate over-constrains 'X' — already positioned via 'A @ B'` |
| Mate within one part | `'a' and 'b' are features of one part — a part is rigid` |
| Mixed dim axes | `'a:left <-> b:top' mixes axes — anchor one axis` |
| `side:` off-axis | `a horizontal dimension stacks on top or bottom` |
| Bad `tol:` | `'tol' takes a number, '+upper -lower', or a fit ident` |
| Bad `pattern:` | `'radial' needs count ≥ 2 and radius > 0` |
| `hatch()` off `fill` | `'hatch' is a fill — a stroke takes a colour` |
| `scale:` ≤ 0 | `'scale' must be > 0` |
| Chain past a label | `a text callout ends its statement — chain before it` |
| Mate in a flow scope | `a '\|row\|' places its own children — mates seat a drawing's` |
| Empty drawing | `a drawing needs at least one geometry child` |

---

## 17. Examples

**A dimensioned plate** — primitives only, holes punched, every value measured:

```
{ layout: drawing; }

|rect#plate| { width: 120; height: 70; }
|hole#pin|   { width: 10; translate: -35 20; pattern: grid(2, 1, 70, 0); }

plate:left <-> plate:right  { side: bottom }   // → 120
plate:top  <-> plate:bottom { side: right }    // → 70
plate:left <-> pin { side: top }               // → 25   (edge to first hole)
pin (-) { tol: H7 }                            // → 2× ⌀10 H7
```

**A turned shaft** — one mirrored profile, named edges and corners:

```
{ layout: drawing; }

|sketch#body| {
  draw: move(-80, 0)
        up(14) right(50):small fillet(3):r1 up(8):sh right(60):mid fillet(3) down(8) right(50) down(14);
  mirror: x-axis;
}
|centerline| { points: -88 0, 88 0; }

body:left <-> body:right { side: bottom }      // → 160
body:left <-> body:sh    { side: bottom }      // → 50 — to the shoulder edge
body:small (-) { side: left }                  // → ⌀28 — the surface, doubled about the axis
body:mid   (-) { side: left }                  // → ⌀44
body:r1 (-                                     // → R3 — the fillet knows its radius
```

**A bushing in section** — hatch, and a bore modelled as the node it is:

```
{
  layout: drawing;  scale: 1.6;
  |steel::sketch| { fill: hatch(45, 6); }
}

|steel#body| {
  draw: move(-30, -8) right(60) up(10) left(60) close();   // the upper wall
  mirror: x-axis;                                          // → both walls
}
|rect#bore| { width: 60; height: 16; fill: --bg; stroke: none; }
|centerline| { points: -34 0, 34 0; }          // duplicated walls generate no auto axis

body:top (-) body:bottom { side: right }       // → ⌀36
bore:top (-) bore:bottom { side: right }       // → ⌀16  (stacks inside the ⌀36 dim)
body:left <-> body:right { side: bottom }      // → 60
```

**A multi-section barrel** — one named profile; stations chain, ⌀s stack (after
`ramjet/drawings/barrel.pdf`, condensed):

```
{ layout: drawing; }

|sketch#body| {
  draw: move(-242.5, 0) up(21) chamfer(1.5)
        right(38):thread :tl right(32):land up(1.5):sh1
        right(14):port down(4):sh2 right(7):groove up(4):sh3
        right(10):collar up(2.5) right(384):tube chamfer(1.5) down(25);
  mirror: x-axis;
} [
  |hole#m8| { width: 8; translate: -45 0; pattern: grid(3, 1, 115, 0); }
]
|centerline| { points: -250 0, 250 0; }

body:left <-> body:right { side: bottom }              // → 485
body:left <-> body:tl { side: bottom }                 // → 38 — ':tl' is a freestanding
                                                       //   point: no shoulder is drawn there
body:sh1 <-> body:sh2 <-> body:sh3 { side: bottom }    // → 14 7 — shoulder edges, one row
body:thread <- "M42×1.5"                               // thread spec — leader to the surface
body:land >- "A"                                       // datum A on the Ø42 land
body:land   (-) { tol: h6; side: left }                // → ⌀42 h6
body:collar (-) { tol: f7; side: left }                // → ⌀45 f7
body:tube   (-) { side: left }                         // → ⌀50
body.m8 <- "3× M8×1.25 ↧10"                            // typed thread callout
```

The bottom dims pack into two rows; the ⌀ callouts stack at the left in source order —
every value read from the one profile. What the print adds beyond this — the section
view A-A, the hidden bore lines, chamfer leaders (`body:top-left <- "C1.5"`) — are a
second `|drawing|`, a dashed overlay, and corner-aimed leaders, all per the sections
above; GD&T frames and finish flags are deferred ([§18](#18-deferred)).

**An assembly** — mates, balloons, a BOM beside it:

```
{
  gap: 24;
  |steel::sketch|  { fill: hatch(45, 6); }
  |balloon::oval|  { width: 16; fill: --fill; stroke: --stroke; font-size: 11; }
}

|drawing#pump| "HAND PUMP — SECTION" [
  |steel#barrel| {
    draw: move(-90, 0) up(23) right(60) up(6) right(60) down(6) right(60) down(23);
    mirror: x-axis;
  }
  |steel#nozzle| {
    draw: move(0, 0) up(12) right(40) down(4) right(20) down(8);
    mirror: x-axis;
  }
  nozzle:left @ barrel:right { gap: -10 }      // pressed 10 into the barrel

  barrel:left <-> nozzle:right { side: bottom }    // → overall, as seated

  |balloon#b1| "1" { translate: -60 -50 }
  |balloon#b2| "2" { translate: 100 -40 }
  b1 -* barrel
  b2 -* nozzle
]

|table#bom| { columns: 24 auto 30; } [
  "#" "Part"   "Qty"
  "1" "Barrel" "1"
  "2" "Nozzle" "1"
]
```

---

## 18. Deferred

Named, not yet built; the syntax above is stable without them.

- **Aligned (point-to-point) dimensions** — today a dim is horizontal or vertical.
- **Per-copy pattern anchors** (`bolt.2`) and pitch dims between copies — today the
  callout text carries them (a `|pitch-circle|`'s own ⌀ already dimensions:
  `bc (-)` → `⌀60`, [§10](#10-line--material-conventions)).
- **Fan leaders** — `a & b <- "2× R5"`, one note with two leaders.
- **`explode:`** — scale directed-mate separations for exploded views
  ([§11](#11-assemblies--nesting)).
- **Authored-name twins** — a `mirror:` / `pattern:` copy of a `:name`d point or edge
  is unaddressable (the name reads the drawn original; the unary mirrored readings
  cover the turned-profile cases, [§8](#8-dimensions)).
- **Routed links to authored anchors** — `a -> b:port` in a flow / grid diagram, the
  orthogonal router landing on a named point or edge; needs a
  [ROUTING.md](ROUTING.md) contract extension (ports, stubs, and Law 2 are side-based
  today).
- **Repeated-name counting** — the same `:name` on several corners auto-prefixing a
  callout's count (`4× R3`), as `pattern:` does for features; today, type it.
- **GD&T** — datums and feature-control frames (position, flatness, runout,
  concentricity, …). The designed direction, no new grammar: an `|fcf|` note type over
  `|table|` (compartments are cells, the border and dividers the box), its symbols a
  small **built-in glyph set named by ident** (`symbol: position`), drawn as paths
  like icons; a `|datum|` note type **boxing** the letter (the `>-` op and its
  `datum` triangle marker exist today, [§9](#9-leaders--notes), with a plain-text
  letter); **surface-finish** (`√`, Ra flags) the same way via `|finish|`. Today:
  `body:seat >- "A"`, `body:face *- "Ra 1.6"`.
- **Hole variants** — counterbore, countersink, thread conventions (`\|thread\|`).
- **View machinery** — projection lines between views, detail circles ("VIEW A"),
  cutting-plane arrows (A–A), broken/interrupted views; today, composed by hand.
- **`stroke-style: phantom`** (dash-dot-dot).
- **Balloon auto-numbering and auto-BOM** from the scene's parts.
- **Dim-line breaks / halos** where annotations cross geometry — today text sits above
  its line and stacking keeps dims clear.

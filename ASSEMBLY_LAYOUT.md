# Engineering drawings for Lini — design sketch

**Status: brainstorm.** These are promising ideas, not decisions. Everything here
should be criticised and thought through before any of it is built.

## Goal

Explanatory engineering drawings — parts, sections, and assemblies, dimensioned
and labelled — for books, docs, and AI-generated figures. Technically correct and
readable as plain text; **not** shop-accurate or mm-exact. The target is the 90 %
of *simpler* drawings (a dimensioned plate, a stepped shaft, a labelled bearing, a
small cutaway), done well.

## Principles

- Reuse Lini's model: `|type#id|` identity, `{ }` style, `[ ]` content, `a -> b`
  links, the CSS-like cascade.
- Reuse existing **properties**; let the type/layout give them meaning — no short
  props like `x:` / `y:` / `d:`. `width` is a round feature's diameter, or a step's
  length, depending on the type.
- New **sigils/operators** are acceptable; new **grammar mechanisms** are not.
  Everything rides the existing node / link / value productions.
- Everything **lowers to Lini's existing primitives** (as charts and sequences do),
  so render, theming, `--bake-vars`, `fmt`, and determinism come for free.

---

## 1. Geometry — `|sketch| { draw: … }`

A pen that folds to a path. `draw:` is a left-to-right list of **bare calls** — the
same value-position rule as `repeat()` / `rgb()`. An argument that needs arithmetic
is backtick-fenced (`left(`w / 2`)`); operators never appear bare. Coordinates are
center-origin like every primitive, and the bbox is computed from the geometry.

| command | does |
|---|---|
| `move(x, y)` | set the start / begin a new subpath |
| `left/right/up/down(n)` | an orthogonal run of length `n` |
| `line(dx, dy)` | a relative straight segment |
| `angle(deg, n)` | a run of length `n` at a bearing — **0 = up, clockwise** (right 90, down 180, left 270) |
| `arc(dx, dy, r)` | arc to a relative point, radius `r` |
| `arc(r, deg)` | tangent arc — continue the heading, sweep `deg` on radius `r` |
| `curve(…)` | bezier (advanced 10 %) |
| `fillet(r)` / `chamfer(c)` | **corner modifiers** between two segments — trim both legs and drop in a tangent arc / bevel; they add nothing on their own |
| `circle(r)` | a circle subpath |
| `close()` | close the current subpath |

A second `move()` starts another subpath; an inner subpath reads as a **hole**
(even-odd). So an outline with a bore is one shape; composite parts are just
overlapping shapes — no boolean operations are needed.

### mirror

`mirror:` is a `|sketch|` **property** (not a pen step): it reflects the whole drawn
path and unions the copy — draw half, mirror to whole. The axis is the coordinate
axis **through the draw origin** (your `move` start), so the idiom is literally
"draw from the centerline":

| `mirror:` | line | gives |
|---|---|---|
| `x` | the x-axis (y = 0) | top ↔ bottom symmetry |
| `y` | the y-axis (x = 0) | left ↔ right symmetry |
| `x y` | both | draw a quarter → full 4-fold part |
| `45` | a line at that bearing through origin | angled symmetry |

No `close()` when mirroring — the edge lying on the axis is the seam. An offset
(`mirror: x 10`, about y = 10) covers the rare off-origin case.

---

## 2. Placement — the `overlay` layout + `@`

**`overlay`** — children share one datum (concentric by default), positioned by
`translate`; the container sizes to the **union** of its children (it grows) and
paints in **source order**, so overlaps and cutaways just work. Templates:
`|drawing|` (an annotated single part) and `|assembly|` (a stack of parts).

**`@` — mate.** `a:anchor @ b:anchor` is a **relationship** between two parts, not a
one-way move; it positions and **draws nothing** (so it never conflicts with drawn
links, which move nothing). It coexists with `pin` (pin → the parent, `@` → a peer).
Concentric by default; a `:side` abuts faces — `nozzle:left @ barrel:right`.

**Grounding.** One node per assembly is **fixed** — the **ground** — and every mate
resolves by moving the *other* side, walking outward from it. The ground defaults to
the **first-declared part**, so you usually mark nothing; tag another to override.
Because the asymmetry comes from what is grounded (not operator order), `a @ b` and
`b @ a` mean the same mate; order only breaks ties grounding does not settle. A cycle
of mates is over-constrained and flagged.

**Offsets & angle.** The anchor's `(dx, dy)` adds a gap or nudges a non-rect edge on
either end (`n:left(4,0) @ b:right`); a part that seats at an angle carries the
existing `rotate:` (rotate, then mate — the turn pivots on the mate point).

**Anchor notation**, reused by links, `@`, callouts, and dimensions:

```
id[.child…][:side][(dx, dy)]     // :side defaults to center; (dx,dy) a gap/nudge
```

---

## 3. Dimensions & callouts — links

In an overlay scope, links are **annotations**: no router runs; each link lowers
straight to a dimension or a leader (a wiring strategy, exactly as a sequence
lowers links to time-rows). The **operator carries the kind** and supplies the
glyph you can't type on a keyboard; you type only the value. Nothing auto-measures
— which suits schematic, not-to-scale drawings.

| write | is | renders |
|---|---|---|
| `a <-> b "80"` | linear dimension | arrows + 80 |
| `a (-) "10"` | diameter | ⌀10 |
| `a -) "5"` | radius | R5 |
| `a <) b "90"` | angular (arc) | arc + 90° |
| `a -> n` / `a -* tip` | leader / callout | note + arrow / dot |

Anchors pick the flavour where useful: `plate:left <-> plate:right` is a width,
`hole:center <-> hole:right` a radius.

---

## 4. Patterns

`pattern:` — a **node property**, works on any node in any layout, replicating it
about its own position:

```
pattern: grid(cols, rows, dx, dy)
pattern: radial(count, radius)
```

---

## 5. Materials & chrome

- `hatch(45, 6, --gray)` — a **fill**, like `gradient()`: section hatching at an
  angle and spacing. Cross-hatch: `hatch(45 -45, 6, --gray)`.
- `stroke-style: center` — a dash-dot pattern for centerlines; plus a `|centerline|`
  define.

---

## 6. Parts library

Plain **defines over `|sketch|`** — no engine support, just bundled geometry:
`|hole| |tube| |washer| |bearing| |screw| |balloon| |finish| |centerline|`.
Parametrised by reused props (`width` = diameter, …). A book or a project defines
its own the same way (`sprue`, `runner`, `gear`).

---

## Worked example

```
{ |steel::sketch| { fill: hatch(45, 6, --gray-deep); stroke: --gray-ink } }

|assembly#pump| [
  |steel#barrel| {
    draw: move(-90, 0)
          up(23) right(60) up(6) right(60) down(6) right(60) down(23)  // top half
    mirror: x                                     // → full symmetric body
  }
  |cyl#nozzle| {}
  nozzle:left @ barrel:right                      // abut faces; the assembly grows

  |hole#bolt| { width: 6; pattern: radial(6, 30) }   // Ø6, six on an R30 circle

  barrel:left <-> barrel:right "180"              // linear dimension
  bolt (-) "6 (6×)"                               // diameter callout → ⌀6 (6×)
]
```

---

## Open questions — criticise these first

- **Unary `(-)` / `-)`.** Convenient, but the one spelling that bends the `a op b`
  link grammar (no right endpoint). Keep it, or always spell a diameter
  `a:left <-> a:right`?
- **Two-diameter parts** (tube / washer / bearing, OD *and* ID). A bore as a named
  sub-feature (`t.bore { width: … }`, separately dimensionable), or just draw them
  as a `|sketch|`?
- **`mirror: x` means *about the x-axis* (top/bottom).** Some read "mirror x" as
  "flip left/right" — confirm the convention.
- **Grounding marker.** How to override the default (first-part) ground — a
  `.ground` class, a `ground` prop, or something else?
- **One layout or two.** Is `|assembly|` distinct from `|drawing|`, or is one
  overlay layout with different defaults enough?
- **Deferred.** `<)` angular (needs two edges + a vertex) and any auto-measure —
  worth keeping out of a first cut?
- **Import.** `|image|` / `|path| { src: "detail.svg#id" }` as a vector escape
  hatch — in or out?

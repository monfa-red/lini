# Drawing — cutaways & constructive shapes (brainstorm)

Brainstorm notes, nothing built. Goal: explanatory engineering cutaways — name the parts
of a part (e.g. an injection-moulding barrel). Reuses Lini's sigils; almost no new properties.

**The pieces**

- **Anchor** — `id:side(dx,dy)`: a point on a node. `:side` defaults to `center`; `(dx,dy)` is an
  optional px nudge. One notation, reused three ways: link endpoints (`box -> gate:left(-5,0)`),
  attach, and callouts.

- **`@` attach** — `guest @ host` ("guest at host"): move the guest's anchor onto the host's;
  draws nothing. Positions only; one placement per node; orthogonal to `layout:` (not a new
  layout mode); coexists with `pin` (pin → parent, `@` → a named peer).
  `noz:left @ barrel:right`, `shaft @ barrel.bore`, `h2:bottom @ barrel:top(0,0)`.

- **Callouts** — just a node wired with `->` to an anchor point; the operator picks the leader
  end (`-*` dot, `->` arrow, `-` plain). `b -* noz:tip`.

- **`combine`** — on a child: `none | add | remove` (default `none`). Fuses the child into the
  parent as one recomputed outline: `remove` carves (the region empties — fill it separately),
  `add` unions a bump. Useful beyond cutaways, for quick composite shapes. (Needs real polygon
  booleans; curves flatten to polygons — fine for explanatory drawings.)

- **`pattern()` fill** — like `gradient()`: `pattern(--gray)` easy, `pattern(45, 6, --gray)` full,
  `pattern(45 -45, 6, --gray)` cross-hatch. Material presets (steel, plastic) are user defines.

- **Shapes** — parametric shapes are plain functions feeding `points:` (a hexagon / gear already
  render in stock Lini). Ship a few **convenience shapes** (ngon, gear, star, arc, arrow,
  spline-through-points) *and* keep the generator functions available — use the ready ones or
  write your own (a book defines its own `sprue()`, `runner()`).

- **Dimension & centerline** — pinned child overlays belonging to their part (same family as
  `caption`/`badge`): centerline pins center, auto-sizes + overshoots; a linear dimension pins to
  a side and auto-rotates. Translatable to nudge off the part.

- **Import** — `|path| { src: "detail.svg#outline" }` pulls vector geometry from an SVG at compile
  time, inlined as real (hatchable/`combine`-able) path — the PDF→SVG→annotate escape hatch.

**A cutaway reads like:**

```
{
  |steel::box| { fill: pattern(45, 6, --gray-deep); stroke: --gray-ink }   // material presets (user defines)
  |melt::box|  { fill: --teal-wash; stroke: --teal-ink }
  |band::box|  { fill: pattern(90, 4, --amber-deep); stroke: --amber-deep; width: 70; height: 14 }
}

|steel#barrel| { width: 380; height: 92; radius: 8 } [
  |block#bore| { combine: remove; width: 360; height: 44 }   // carve the melt channel
  |centerline|                                               // pinned overlay — auto-sized + overshoot
  |dimension| "L/D ≈ 20" { pin: bottom; translate: 0 40 }
]

|melt#melt| { width: 356; height: 40 }     // the polymer — a separate fill for the void
|cyl#nozzle| {}
|band#h1| {}   |band#h2| {}                // heater bands

melt        @ barrel.bore                  // fill the carved channel
nozzle:left @ barrel:right                 // attach — abut the right face
h1:bottom   @ barrel:top(-90, 0)
h2:bottom   @ barrel:top(90, 0)

|balloon#b| "1"
b @ nozzle:tip(50, -40)                    // park the balloon in the margin…
b -* nozzle:tip                            // …dot-leader back to the tip
```

**Still open:** anchor px vs %; `combine` naming; how a convenience shape takes its parameter;
procedural repetition for multi-part shapes (a screw = shaft + N flights).

# Schematic placement — the lattice

A rebuild of `layout: schematic`'s placement. The engine keeps its inputs
(desugar's lowered parts, the scope's links) and its output contract (a placed
tree the orthogonal router draws wires onto), and replaces everything in
between: three passes computing continuous coordinates from local rules become
six passes assigning **integer cells** on one grid.

Status: design, approved 2026-08-31. Branch `schematic-grid`.

---

## 1. Why

Every coordinate in the current engine is *derived from ink*. A lane ladder's
pitch is "the greediest step any neighbouring pair asks"; a seat is measured off
connection geometry and the reach of whatever stack it crosses; a track sizes to
its anchors' cluster bboxes. So a part's position is a function of its
neighbour's **value text width** — and no two decisions share a coordinate
system.

That is why the sheet cannot state "these three caps are in a row": there is no
row. It is also why thirty commits of placement rules did not converge — each
was a local repair to a global property. `seat.rs` is 1473 lines and its module
doc reads as case analysis.

Three symptoms survived all of it, and all three are the same shape — a rule
that cannot be stated without reaching into another pass:

- a lone span member reads off-centre against the router's incoming jog, and
  centring it on that jog is circular (the jog's corridor is bounded by the
  member's own keep-out);
- a `|label|` symbol's name takes the reading side even when that points back at
  the part it hangs off;
- 16.4's "freer side" for a net-run name is measured at the run's naive
  position, not the line it lands on.

Against a real sheet (`fadec.pdf`, `even.pdf`, and the TPS61023 boost block used
as the reference for this design) the difference is not subtlety. A drafted
sheet is **on a grid**: columns on one pitch, part bodies sharing rows, every
ground on one line. Ours is on none.

## 2. The model

One integer lattice per schematic scope. Two pitches, both already implied by
the sheet:

| | Is | Value |
|---|---|---|
| **pitch** (fine) | the wire and pin lattice | baked, `PIN_PITCH` = 20 |
| **gap** (coarse) | the part lattice — column and row pitch | authored, default 100, rounded **up** to a multiple of pitch |

Column `c` lies at `c · col_gap`, row `r` at `r · row_gap`; cells are signed.
`gap: 120 80` states row and column pitch separately, as `gap` does everywhere.
`gap` stops meaning "the space between two tracks" in this scope and means the
column pitch: two adjacent anchors with nothing between them stand one coarse
column apart.

`pitch` is **not** authored. It is the pin rail's own spacing, the stub length
and the router's minimum track separation at the scope's clearance; a sheet
whose pins are off their own grid is not a sheet.

**The invariant**, judgeable on the output alone, the way ROUTING.md's four laws
are:

> Every part's centre lies on a fine lattice point. Every satellite's lane and
> slot lie on coarse lattice lines. The scope's centring shift is a whole number
> of pitches, so the lattice is absolute.

The one exemption is a satellite no wire holds: it has nothing to seat against,
falls back to the flow with a warning as it does today, and is judged by no
lattice law.

### 2.1 Ink never places

A satellite's cell comes from the lattice, never from its symbol's size or its
ref/value width. A long value overhangs its neighbour's column; `gap` is the
lever, and the author owns it.

This is the single deletion that makes the rest possible: it removes the ladder
pitch, the ink-measured column step, the stack reach, the cluster bbox, and the
"greediest neighbouring pair" rhythm — every mechanism in which one part's text
moved another part.

The one place a part's own ink is read is the **field origin** (§2.3): how far
out on each side its own field begins. That is per anchor and per side, so every
lane on a side starts level and nothing wobbles between columns.

### 2.2 A chain is a walk

A satellite chain — the connected run of satellites one wire holds, exactly as
`chain::chains` computes today — walks the lattice:

- **ray** — the growth direction. The terminator's own drawing decides (a
  `|gnd|`'s connection point is at its top, so its chain grows down; a power
  flag's at its bottom, so up); with no convention the chain runs straight out
  along the pin's normal. A ray anti-parallel to the pin's normal yields to the
  normal, and the terminator poses inverted, as a sheet flips a ground above a
  part.
- **lane** — the cross coordinate. A chain that *turns* off its pin takes a
  coarse line: the innermost free one on that side, stepping outward. A chain
  that grows straight out along its pin takes no lane — it keeps the pin's own
  fine line.
- **slots** — member *k* is centred on the *k*-th coarse line along the ray from
  the field origin. Centred, not lead-anchored: a cap and a resistor hanging off
  one bus share a body row, and their leads differ by their own lengths.

The **straight corridor** of a pin belongs to its first claimant in statement
order; a later chain on that pin turns onto the canonical ray (down off a side
pin, rightward off a top or bottom one). A **tap** — a single symbol-label leaf
hanging off a mid-chain member — takes no slot: it hangs off its attachment
member along its own drawn convention, stepping aside when that points back into
the trunk. A **branch** of more than one member grows as its own sub-chain from
its attachment junction.

### 2.3 Collision is set intersection

A chain's cells form a set. A lane is free when its set does not meet the
scope's occupancy; when it does, the lane steps out one coarse line and retries.
That one rule replaces stacks, corridors, wired-row keep-outs and the pitch
rhythm, and two consequences fall out of it rather than being authored:

- an up-chain and a down-chain off one pin **share a lane** — their cell sets are
  disjoint, so the second one's first candidate is already free;
- a chain never lands on a part a lead must cross, because the crossed part is in
  the set.

**Lane order** is the pins' own, read along the ray: the pin *deeper* along the
ray keeps the inner lane and the shallower one steps out, so a lead crossing an
inner column crosses it above where that column is live. That order **is** the
allocation order — a side's chains take their lanes deepest-first, ties on
statement order, each the innermost cell set the occupancy leaves free. A side
carrying both rays cannot read it two ways and falls back to the canonical
direction, the deepest pin innermost either way.

**Field origin** — the first slot line, and the innermost lane line, on a side:
the first coarse line clear of the anchor's own drawn ink on that side, its
ref/value readouts included.

### 2.4 Rails

Scope-wide, after the anchors place: every downward chain's ground symbol sinks
to one **ground row** — the deepest slot any chain in the scope reached, plus
one — and every upward flag rises to one **flag row**. Rails are vertical only;
a horizontal chain keeps its own end, which is what both reference sheets draw.

This is the largest single visual win available: the reference boost block puts
six grounds on one line, and `fadec`'s buck block seven.

### 2.5 Packing

Anchors ride the ordinal track grid unchanged — one row by default in
declaration order, `columns: N` wraps, `cell: c r` places, ordinals collapse.
What changes is the sizing: a track's width is a whole number of coarse columns,
taken from its anchors' field widths, and the region between two tracks holds,
in order, the earlier anchor's right field, any span's members, and the later
anchor's left field.

**Alignment.** Two anchors in one track row default to centre-to-centre on a
shared row line. Where a wire — or a span, whose members all ride one line —
joins a **facing** pin pair (my right pins against a later column's left pins;
columns mirror it), that pair aligns instead and the wire draws dead straight.
The shift is always a whole number of pitches, so alignment never breaks the
lattice. Anchors take alignment in track order, each through the first
statement-order wire reaching a placed neighbour.

A **span** (a chain held at two anchors) rides the landing leg — the straight
run into the second-named end, on that pin's own line — its members on
consecutive coarse cells, the last-named nearest that end. A **bridge** (both
ends on one anchor) grows off the first-named pin as an ordinary one-end chain;
the far wire is the router's, merged at a junction dot.

### 2.6 Readouts

A part's ref/value pair is placed by rule, never by search, and is never an
obstacle:

| Part | Text | Aligned |
|---|---|---|
| on a lane, in an anchor's **left** field | to its left | right |
| on a lane, in an anchor's **right** field | to its right | left |
| riding a row (pins left/right) | above and below | centre |

Outward from the anchor, in one sentence. `translate:` on the styled-label form
is the escape.

### 2.7 The router

One clause in ROUTING.md's Model step 5: an **interior run** prefers its
channel's anchor *rounded to the world's track quantum* where the scope sets
one, clamped back into its corridor. A schematic scope sets the quantum to its
fine pitch; every other scope sets none and is unchanged. No law changes — a
preferred track is a preference, and the four laws still judge the drawn wire.

Everything else in the routing contract stands: fixed ports, worlds, channels,
search, geometry, the law checker.

## 3. Shape

```
src/layout/schematic/
  mod.rs        unchanged entry (node / root / arrange)
  place.rs      SLIMMED — the orchestrator: roles, tracks, the six passes
  lattice.rs    NEW — pitch, gap, cell↔px, the snap, the invariant
  field.rs      NEW — chains to cells: rays, lanes, slots, occupancy
  pack.rs       NEW — tracks in coarse columns, field widths, facing alignment
  rail.rs       NEW — the ground and flag rows
  readout.rs    NEW — the ref/value side rule
  seat.rs       DELETED (1473) + seat_tests.rs (1036)
  net.rs · junction.rs · ports.rs · terminal.rs · tag.rs · hints.rs   kept
src/desugar/schematic/**                                             kept whole
```

`hints.rs` keeps both diagnostics (a chain with no placed end falls back to the
flow; a third placed end is dropped) and re-reads the same chains the field pass
read, so the two cannot disagree.

## 4. Constants

| Constant | Now | Then |
|---|---|---|
| `SCH_GAP` | 60 (track gap) | 100 (coarse pitch), tuned by eye in P7 |
| `PIN_PITCH` | 20 | 20 — now also the fine lattice and the router's quantum |
| `LABEL_SEAT` | 25 | deleted — the lattice states the distance |
| `READOUT_OFFSET` | 40 | the readout's clear gap from the part's cross edge |
| `NET_LABEL_RUN`, `NET_LABEL_OFFSET`, `PIN_STUB`, `JUNCTION_RADIUS`, `TAG_POINT` | | unchanged |

A component's pin rail is snapped so its pins land on fine lines (an even pin
count currently seats them on half-pitches).

## 5. Non-goals

- **No second router.** Wires stay the orthogonal router's, under its contract.
- **No scoring or search over candidate layouts.** A rule you can read beats a
  cost function you cannot; determinism is a language property here.
- **No inferred pose for 3-pin parts.** They are anchors already; `rotate:` and
  a per-family default pose are the levers, and one word beats a heuristic.
- **No text-aware placement.** §2.1 is the point of the rebuild.

## 6. Testing

- Unit tests per pass — lattice arithmetic, lane allocation against a seeded
  occupancy, rail sinking, readout sides.
- An **invariant checker** over every schematic sample, asserting §2's
  invariant: the analogue of the routing law checker, judging output alone.
- `insta` snapshots for the samples.
- A rendered PNG read at every phase gate — `resvg --zoom 3`, cropped per block.

## 7. Phases

Each lands on its own, with tests, and leaves the tree green.

| | Phase | Leaves |
|---|---|---|
| **P0** | SPEC 16 rewritten around the lattice; ROUTING.md's one clause; 10.5 constants | docs true, code unchanged and failing nothing |
| **P1** | `lattice.rs` + the invariant checker + constants | checker present, marked expected-fail |
| **P2** | `field.rs`; `seat.rs` deleted | chains on cells, packing still crude |
| **P3** | `pack.rs` — tracks in coarse columns, facing alignment | anchors and fields on one lattice |
| **P4** | `rail.rs` — ground and flag rows | the reference sheet's ground line |
| **P5** | `readout.rs` — ref/value sides | text outward, never placing |
| **P6** | the router's track quantum | bare runs on the fine grid |
| **P7** | samples rebuilt, constants tuned, visual audit | the showroom |
| **P8** | audit — SPEC↔code, xtask regen, `fmt`/`clippy`/`test` | mergeable |

P2–P5 are the risky ones and are where the design will meet cases it did not
predict; the phase boundaries are drawn so each can be re-cut without unwinding
its neighbours.

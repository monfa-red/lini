# The Routing Contract

Lini routes links over a **finished, immutable layout**. Routing never moves a
node, never grows a gap, and no later pass repairs what it draws: every
decision is made once, in one place, and stands. This document is the source
of truth for routing — two halves: the **laws**, judgeable on the rendered
output, and the **model**, the one way routes are produced, so the same
diagram can never route two different ways.

`SPEC.md` §9 owns link *syntax*; this document owns link *geometry*.

---

## Strategies

`routing:` selects a strategy per scope and cascades like `clearance`:

| Strategy | Status | Shape |
|---|---|---|
| `orthogonal` | the default — specified below | horizontal/vertical runs, corners rounded at render time |
| `natural` | specified below — lands with alpha.1 | obstacle-aware smooth curves: a legal corridor, then a curve inside it |
| `straight` | built — sequence messages | one segment between caller-supplied anchors |

Every strategy consumes the same input (the placed scene, the expanded link
requests) and produces the same output (drawn wires, a report, strays), sharing
one spine — request expansion, markers, labels, stray drawing. Only geometry
construction differs, so a new diagram family adds a strategy module, never a
refactor. **Validation is per strategy**: the law checker judges orthogonal
wires against the four laws; a `natural` wire is judged by its own arm
(§The natural strategy); `straight` wires are skipped — lawfully oblique,
avoiding nothing.

**`straight`** is the trivial strategy: each link is one segment between two
anchors its caller supplies (plus the rectangular self-hook), trimmed to the
endpoint bodies. It avoids nothing; markers and labels ride it like any wire.
A `layout: sequence` scope routes its messages through `straight` — sequence
layout owns *where* (column x, row y), the strategy owns the wire.

## The natural strategy

`natural` draws **smooth curves through free space** — the mindmap's branch
look, available to any routed scope. It is corridor-first: steps 1–4 of the
orthogonal model run unchanged (keep-outs, worlds, channels, requests,
search with admission — one corridor choice, priced by the Law-3 cost), and
only steps 5–6 differ: instead of placed runs and rounded polylines, the
route lowers to a G1-continuous cubic spline fitted **inside the chosen
corridor** — never a rounded illegal straight line. Constraints:

- **Contact holds exactly**: each end meets its side perpendicular
  (tangent-normal), inside the port window, straight for at least its marker.
  On a laid-out tree this yields the classic horizontal-tangent S-curve.
- **Clearance holds sampled**: the curve stays ≥ `clearance` from every
  keep-out along its length; where a corridor cannot bend that gently the
  curve tightens toward the corridor's polyline rather than leave it.
- **Crossings need not be square-on** — two curves may cross obliquely, still
  point contact, counted in the report like any crossing.
- **Duplicates** draw as offset parallels at pitch; a **fan** shares one
  trunk curve to the split; a **self-loop** is a smooth hook; forced sides,
  labels (arc-length `along:`, sliding), bundles, and **strays** ride the
  shared spine unchanged.
- **Determinism** is Law 4 verbatim; there are no tension / curvature knobs.

The natural checker judges contact (perpendicular arrival, window, marker
stub), sampled clearance, and duplicate separation; the run/track and
square-crossing laws are orthogonal-only.

The rest of this document is the `orthogonal` contract.

---

## Vocabulary

- **Node** — anything a link avoids: a box, an oval, a text label. A node is
  its axis-aligned bbox with four **sides**. A **group** not containing a
  link's endpoint is solid; one containing an endpoint is transparent to that
  link, its *other* children solid.
- **clearance** — one number, default **16** (`clearance:` cascades; the
  diagram routes at the maximum any link carries): the minimum gap between a
  link and every node body. Node clearance **never shrinks**.
- **pitch** — the gap between a link and its neighbouring links. Starts at
  `clearance`; where a channel or side cannot hold its wires at that spacing
  it compresses, uniformly per group, **never below `clearance / 2`**. Pitch
  is the one relief valve — layout is never the relief.
- **Keep-out** — a node's bbox inflated by `clearance`. Only a link's own
  perpendicular end segment enters its own endpoint's keep-out.
- **Channel** — a maximal free rectangle between keep-outs, from the sweep
  decomposition: V-channels carry vertical travel, H-channels horizontal;
  each axis's channels partition the free space. A **cell** is an H∩V
  overlap; cells are the graph's vertices. A channel wall is a keep-out
  edge, a **shared boundary** with a same-axis channel, or the **canvas
  edge**; no wall charges a margin — a run may hug whatever bounds its
  corridor, and separation across a shared boundary is placement's job like
  any other (near runs on the two sides settle in one cluster). The sweep
  may slice one free corridor into several same-axis channels; capacity,
  anchors, and usable width always read the reassembled **corridor** — the
  walls that actually bound a run's span — so a shared boundary interior to
  a void costs nothing.
- **Run** — one straight piece of a route, lying in one channel of its axis.
  A run's **track** is its ordinate across the channel. A route is an
  alternating chain of runs.
- **Port** — the point where a link meets a side: the ordinate of its end
  run. Ports are not chosen ahead of routing; they fall out of placement.
- **Bundle** — the links sharing one unordered endpoint pair and the same
  forced sides: one route, adjacent tracks, parallel rails the whole way.
- **Crossing** — an intersection of two links: exactly perpendicular, both
  locally straight, point contact.
- **Stray** — the drawn report for an unroutable link (§Impossible layouts).

---

## The Four Laws

Checkable on the output with no knowledge of the router:

1. **Clearance.** A link keeps ≥ `clearance` from every node body, and
   ≥ pitch from every other link. Sub-`clearance` pitch is excused only by a
   channel or side that cannot hold its wires at full clearance, is uniform
   within that group, and never falls below `clearance / 2`. Exactly three
   surrenders: a link's own end segments (each entering only its own
   endpoint's keep-out, perpendicular), crossings (square-on, point contact),
   and a fan's shared trunk (drawn as one line until the split).

2. **Contact.** Every link end lands **on a side**, **perpendicular**,
   ≥ `clearance` from that side's corners — never on a corner, never inside a
   body. Ports sharing a side sit ≥ pitch apart, in the same order as their
   wires (no braiding at the mouth), each as close to where its wire runs
   straightest as its neighbours allow — a lone aligned pair connects dead
   straight; a crowded side ladders around the contested spot.

3. **Economy.** Each link takes the cheapest legal route, where
   **cost = length + 2·clearance per turn + 4·clearance per crossing**, given
   every earlier link's committed route — earlier links never move. Routing
   order is declaration order; the constants are part of this contract. A
   crossing is worth a `4·clearance` detour, no more: long orbits never beat
   short crossings, and turns cost real length, so straight beats dogleg
   beats staircase.

4. **Determinism.** The same input renders byte-identically, every time.
   Every tie breaks on the fixed side rank (right → bottom → left → top),
   then declaration order.

When the layout leaves a link no legal route, the link is **reported and
drawn as a stray** — never as a lawful-looking wire; `--strict` turns the
report into an error. There is no other escape: nodes never move, gaps never
grow, pitch never drops below half clearance, bundles never split.

---

## The Model

Six steps. Each decides once; none revisits an earlier step's answer.

1. **Keep-outs & worlds.** Layout finishes first and is immutable. Every node
   inflates by `clearance`. A link routes in the innermost container holding
   both endpoints — its **world** — with that world's other children solid
   and the endpoints' ancestor groups transparent; if the inner world has no
   route, the link retries one world up, to the root (a tight interior never
   walls in a link its ancestors would let out).

2. **Channels.** Per world, the free space — bounds plus canvas margin, minus
   keep-outs — decomposes by sweep into H- and V-channels, cells, and
   adjacencies. The graph depends only on node placement; links never reshape
   it. A channel span's **capacity** is the runs it can hold at minimum
   pitch: `floor(usable width / (clearance/2)) + 1`.

3. **Requests.** Resolve expands every link statement into edges, grouped
   into bundles (multiplicity *k*), ordered by declaration then expansion
   order — the order routing consumes them in.

4. **Search.** Per bundle, in order: enter the graph by a perpendicular
   **punch** from each permitted side (a forced side prunes to one; the punch
   crosses transparent ancestor walls, never a solid keep-out); run weighted
   Dijkstra over cells with the Law-3 cost. Length is the L1 estimate through
   the entered cells; turns count axis changes, plus the end jog when the two
   ports' windows cannot meet on one track; crossings count the committed
   perpendicular runs whose spans the candidate sweeps. A channel span whose
   committed load leaves fewer than *k* tracks at minimum pitch is **closed**
   — so is a side without *k* free port slots at minimum pitch — and the
   search detours around it; capacity is never exceeded, only priced. The
   winning route commits its runs (channel, span, *k*) and its two sides. No
   route in any world: every member of the bundle is a stray.

5. **Placement.** Per channel, independently: runs whose spans come within
   pitch of one another form a **cluster**; a cluster's pitch is
   `min(clearance, usable / (n−1))`, floored at `clearance / 2` (step 4
   guaranteed it fits). Runs order within the cluster so wires leave in the
   order they arrive — nested, never braided; bundle members keep declaration
   order; remaining ties break by declaration. Each run states a **preferred
   track**, and the cluster takes the order-preserving ordinates that
   minimize total squared deviation from those preferences at ≥ pitch inside
   the channel's usable width (unique; pool-adjacent-violators):
   - an **interior run** prefers its channel's **anchor** — the midline when
     both walls are keep-out edges (a bend between two nodes lands halfway
     between them), the keep-out wall when the other wall is the canvas edge
     (wires hug the diagram, not the margin);
   - an **end run** prefers the straightest lawful line: its ports' shared
     window when one run serves both ends (the two side centres' midpoint,
     clamped into the window), its own side's centre otherwise. A **port
     window** is the side minus a `clearance` corner margin at each end;
     an end run never leaves its window.
   Ports *are* the end-run ordinates — one mechanism places tracks and ports,
   so a port can never disagree with the wire it serves.

6. **Geometry.** Routes lower to polylines: corners are run intersections,
   collinear points merge, each end segment stays straight for at least its
   marker. Corners round at render time with radius
   `min(clearance, half the shorter adjacent leg)`, two refinements intact:
   corners nested on one diagonal round **concentrically** — the innermost
   takes the base radius and each outward radius grows by exactly its track
   offset, so wires turning together hold their gap through the arc — and
   every radius caps at the nearest crossing on its legs, so a crossing never
   lands mid-arc. Rounding never brings a link nearer than the law allows to
   anything. Labels ride the drawn route at their `along:` fractions
   (auto-distributed when unset), may slide along it to dodge nodes and other
   labels, and never move the link.

The report counts every drawn crossing with its link pair, and names every
stray with its source span and reason — a blocked endpoint (no side reaches
free space), a closed graph (no path at minimum pitch), or a full side.

---

## Consequences

The laws above make these shapes canonical — worth knowing because tests pin
them:

| Scene | Drawn |
|---|---|
| Facing sides, centres aligned | one straight wire, zero turns |
| Facing sides, offset, windows overlap | still straight — the wire rides the shared window |
| Facing sides, offset past overlap | one dogleg, the perpendicular run on the gap **midline** |
| The same, ×k (a bundle) | k parallel rails at pitch, the ladder centred on the midline route |
| Wire along the canvas edge | hugs the nodes' keep-outs, not the margin |
| Two buses landing on one side | two nested ladders, no braid, straighter bus nearer its target |
| Crossing vs. orbit | crosses — a crossing costs `4·clearance` of detour, never the diagram's circumference |

---

## Special nodes

- **Fan** (`a -> b & c`): siblings share one port and one end segment on the
  shared end; the shared side and port are the first sibling's. Along the
  common prefix — the **trunk** — siblings are one drawn line; past the split
  each is a full link under every law.
- **Chain** (`a -> b -> c`): separate links, nothing shared.
- **Duplicates** (`a -> b` twice): one bundle — one route, adjacent rails the
  whole way. A bundle routes whole or not at all.
- **Self-loop** (`a -> a`): out one side, around the keep-out corner, back in
  an adjacent side. Defaults right → top; forced sides win; both ends forced
  onto one side is an error.
- **Bidirectional** (`a <-> b`): one link, a marker at each end.
- **Containment** (one endpoint **geometrically** inside the other): the link
  runs *inside* the parent — from the inner node's side to the parent's
  **inner** side, the parent's other children solid. The trigger is geometry,
  not path ancestry: everywhere but a tree, nesting implies enclosure, but a
  tree's branch child is a path-descendant placed *beside* its parent, so it is
  an ordinary side-by-side wire, not a containment link.

---

## Impossible layouts

The laws are absolute. When geometry allows no legal route, the engine draws
every link it can and **names the ones it couldn't**, each with its source
span and reason; `--strict` makes that an error. The engine never draws a
link through a node, oblique into a side, or squeezed below half-clearance
pitch to paper over a crowded diagram — and it never moves the layout to
help. The user's levers are the honest ones: widen `gap`, shrink
`clearance`, reorder or re-side the links.

An impossible link renders as a **stray**: a single straight segment between
its two bodies, centre to centre, trimmed to their boundaries, at whatever
angle the geometry gives. Lawful wires are orthogonal, so a slanted dashed
line (themed `--lini-stray`, a warning glyph at its midpoint) cannot be
mistaken for one. A stray obeys no law, takes no port, and blocks nothing.

---

## Implementation shape

```
src/routing/
  mod.rs        strategy dispatch, shared Routing result (links, report, strays)
  report.rs     violations, crossings, stray construction
  straight.rs   the straight strategy (sequence messages)
  ortho/        the six-step model — scene index (scene, rect), channel graph
                (graph), requests/bundles (request), admission (admit, cost,
                entry, ledger), search, placement (place, ladder, order,
                pairwise), geometry, labels
  natural/      the natural strategy — corridor choice over the shared search
                (corridor), curve fit (curve)
  validate.rs   the independent law checker (+ validate/excuse.rs) — a test
                oracle over orthogonal and natural wires, never a repair
```

One Dijkstra per bundle over a graph of tens of cells, one linear placement
sweep per channel: routing a busy diagram is microseconds, not seconds. The
validator re-judges every sample's orthogonal wires against the four laws in
CI; complex fixtures pin turn counts, crossing counts, and byte-identical
reruns — no image reading in tests.

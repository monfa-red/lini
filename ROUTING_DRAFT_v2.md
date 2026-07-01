# Routing v2

Draft replacement for `ROUTING.md`.

`SPEC.md` owns link syntax. This document owns link geometry.

Lini has named routing strategies. This draft defines the shared contract and
the `orthogonal` strategy. `straight` and `curved` are named extension points;
their complete rules are deferred.

---

## Design Goals

- Layout is immutable. Routing never moves nodes, grows gaps, changes padding,
  or rewrites the scene.
- One mechanism owns one problem. The orthogonal solver owns ports, paths, lanes,
  crossings, and failure. No later nudge, slide, audit, or layout-feedback pass
  repairs its result.
- The solver optimizes the final polyline it will draw. Ports and lanes are
  part of search, not a later realization step.
- Bends dominate length. A straight route beats a dogleg; a dogleg beats a
  staircase; a staircase beats wrapping around the diagram.
- Retry is conflict-driven. The router retries because it found a concrete
  conflict and added a concrete constraint, not because it is cycling through
  guesses.
- Failure is honest. If the finite search is exhausted, the unresolved link is
  reported and drawn as a stray. If an implementation budget is exhausted before
  the search is exhausted, the report says that instead of claiming impossibility.

---

## Strategies

`routing` selects a strategy for links in a scope.

| Strategy | Status | Shape |
|---|---|---|
| `orthogonal` | built first | horizontal and vertical runs, rounded only at render time |
| `straight` | deferred | one trimmed segment between endpoint sides |
| `curved` | deferred | a spline between endpoint sides |

Sequence diagrams are not a strategy. A `layout: sequence` scope lowers its
messages to primitive time-row arrows during layout; the link router never sees
those messages.

All strategies share request expansion, styles, markers, labels, diagnostics,
and strays. Only geometry construction changes.

---

## Vocabulary

- **Body**: a link endpoint or obstacle, represented by its axis-aligned bbox.
  Text labels are bodies for avoidance.
- **Node clearance**: minimum distance from a routed link to any non-endpoint
  body. Default `clearance: 16`.
- **Wire pitch**: minimum distance between unrelated links. The first orthogonal
  attempt uses `clearance`; the density attempt uses `clearance / 2`. Node
  clearance never shrinks.
- **Keep-out**: a body inflated by node clearance.
- **Port**: where a link meets an endpoint side. The adjacent segment is
  perpendicular to that side.
- **Track**: a horizontal or vertical legal travel segment in free space.
- **Lane**: one wire position on a track. Adjacent lanes are separated by the
  current wire pitch.
- **Resource**: a port slot, track interval, lane interval, or crossing point
  consumed by a route.
- **Conflict**: an illegal resource relationship between routes: insufficient
  pitch, over-capacity lane use, incompatible port use, forbidden crossing, or
  node-clearance breach.
- **Bundle**: duplicate links with the same unordered endpoint pair and the same
  forced sides. A bundle routes as one ribbon of adjacent lanes.
- **Fan**: links from one statement sharing one endpoint. The shared end uses one
  port and one trunk until the branches split.
- **Stray**: the visible report for an unroutable link: a dashed straight segment
  between the two bodies, trimmed to their boundaries.

---

## Laws

These are judged on the rendered output.

1. **Contact.** Every routed end lands on an endpoint side, never a corner. The
   first segment leaves perpendicular to that side. A forced side is never
   changed.

2. **Clearance.** Routed links keep node clearance from all non-endpoint bodies.
   Routed links keep the current wire pitch from unrelated routed links, except
   at reported perpendicular crossings. Fan trunks are one shared line, so they
   do not separate until the split.

3. **Economy.** Within one solver stage, no lower-priority cost can beat a
   higher-priority cost. The order is:
   `(strays, crossings, bends, length, port deviation, declaration order)`.
   Bends and length are measured on the final polyline.

4. **Completeness.** A normal impossible report is allowed only when the finite
   graph search for the applicable stage is exhausted. A defensive budget stop is
   reported as `routing search budget exhausted`, not as impossible.

5. **Determinism.** The same input produces byte-identical output. Every tie
   breaks by declaration order, then stable geometry order.

6. **Honest failure.** The router never draws a lawful-looking illegal link.
   Unresolved links are reported and drawn as strays. `--strict` escalates the
   report.

---

## Orthogonal Stages

Orthogonal routing runs these stages in order:

1. Crossing-free at `wire_pitch = clearance`.
2. Crossing-free at `wire_pitch = clearance / 2`, only if stage 1 leaves strays.
3. Crossing-relaxed at `wire_pitch = clearance / 2`, only if stage 2 leaves
   strays and all crossing-free states are exhausted.

Stage 2 is the only density relief. It changes link-link pitch and port pitch
only. It does not change node clearance or layout.

Stage 3 allows only perpendicular crossings. Parallel overlaps and sub-pitch
hugs remain illegal. Every kept crossing is reported.

Stage order outranks in-stage cost. A crossing-free half-pitch solution beats a
full-pitch solution with crossings. A budget stop exits with a budget report; it
does not prove the current stage exhausted and does not silently advance.

---

## Orthogonal Model

### 1. Requests

Resolve expands chains, fans, cartesian endpoints, and duplicates into ordered
edge requests. Sequence-scope messages are excluded before this point.

Requests are grouped into bundles by unordered endpoint pair plus forced sides.
Bundles stay in declaration order; members stay in declaration order inside the
ribbon. A bundle may split only if routing the whole ribbon is impossible in the
current stage; split pieces remain adjacent in declaration order.

### 2. Worlds And Obstacles

Each request routes in the innermost scope that contains both endpoints.

A group that is not an endpoint ancestor is solid. A group that contains an
endpoint is transparent to that request; its other children remain solid. Labels
inside an endpoint's own body are ignored for that endpoint's stub, but labels
elsewhere remain obstacles.

Keep-outs are built once per stage from node clearance. The graph is built from
keep-outs, endpoint side intervals, and world bounds.

### 3. Track Graph

Orthogonal routing happens on a finite visibility graph. Exhausting that graph
is the definition of impossible for Lini's orthogonal model.

For each world, collect x and y coordinates from:

- keep-out walls;
- endpoint side lines;
- endpoint centers projected onto legal opposite or adjacent sides;
- free-gap midlines between adjacent keep-outs;
- canvas margin tracks;
- wire-pitch offsets needed to fit bundle ribbons beside those coordinates.

Horizontal and vertical tracks are maximal open segments on those coordinates.
Intersections are vertices. Edges carry actual length, axis, free interval, lane
capacity, and owning corridor.

This is a Hanan-style visibility graph plus gap midlines and lane offsets. It is
finite, deterministic, and intentionally includes the routes users expect:
direct shots, midpoint doglegs, bus ribbons, and rectangular obstacle detours.

### 4. Ports

Ports are selected by the solver.

For each endpoint side, the solver sees a finite ordered set of port candidates:

- side center;
- projections from facing endpoint centers;
- intersections with reachable track coordinates;
- pitch offsets around those positions, within the corner inset;
- side endpoints inset by node clearance, when a side is otherwise too short.

Forced sides prune to that side. Unforced ends may use any side. Side preference
is only a tie-breaker; it can never beat fewer bends.

Ports sharing a side keep route order and current wire pitch where the side has
room. If a side cannot hold that pitch, the density stage may retry at half
pitch. There is no side compaction below half pitch.

### 5. Direct Bus Rule

Simple adjacent geometry has a canonical route.

For endpoints on facing sides with one free corridor between their keep-outs:

- aligned ports draw straight;
- unaligned ports draw one dogleg whose perpendicular run sits on the corridor
  midline;
- duplicate links draw adjacent lanes centered on that same route;
- fans share the trunk, then split in declaration order.

The track graph must contain these routes. Since bends beat length, the general
search cannot prefer a staircase or wraparound when this route is legal.

### 6. Low-Level Search

The low-level search routes one bundle under a constraint set.

Use Dijkstra or A* with an admissible Manhattan heuristic. The search is exact
for the finite graph. The bundle cost is:

`(bends, length, port deviation, side preference, declaration order)`.

A bundle consumes adjacent lanes on every used track interval. A route is
illegal if:

- it enters a non-endpoint keep-out;
- the track lacks enough adjacent lanes;
- the constraint set forbids a port, track interval, lane interval, or crossing;
- a forced side is not used.

When this search returns `None`, every legal path for that bundle under those
constraints has been considered.

### 7. Conflict-Based Solve

The diagram solver is best-first conflict-based search over complete route sets.

1. Seed every bundle with its low-level cheapest route, ignoring other bundles
   but respecting bodies, forced sides, and per-bundle width.
2. Detect conflicts by sweeping resources: ports, lanes, track intervals,
   body-clearance boxes, and crossing points.
3. Pick the first conflict by severity, then declaration order, then geometry.
4. Branch only on bundles involved in that conflict. Each branch adds one
   minimal constraint to one bundle.
5. Re-route only the constrained bundle.
6. Push the new state into a priority queue ordered by:
   `(strays, crossings, bends, length, port deviation, declaration order)`.
7. The first state with no illegal conflicts is the chosen result for that
   stage.

This is the only retry loop. It is not blind: every retry has a conflict and a
new constraint. Searches are cached by `(stage, bundle, constraints)`.

Dominance pruning is required. A state is discarded when an already-seen state
has the same routed bundles, no higher cost, and constraints that are a subset
of the new state's constraints. Independent conflict components may be solved
separately and joined by declaration order.

If the queue is exhausted, no route set exists in this finite model for the
stage. The caller advances to the next stage or reports the remaining links.

An implementation may stop early for a defensive budget. A budget stop must be
reported as `routing search budget exhausted`; it is not an impossible proof.

### 8. Crossing Relaxation

Crossings are forbidden in stages 1 and 2.

Stage 3 allows a crossing only as a resource at the intersection of one
horizontal and one vertical lane. The crossing must be strictly inside both
segments. The two links are locally straight at the crossing. Rounding is capped
so no arc contains the crossing.

Stage 3 cost still starts with crossing count. A route with fewer crossings wins
before bends or length. Every kept crossing is listed in the report with its
link pair.

### 9. Geometry

The chosen graph paths lower directly to final polylines:

- ports are the chosen port candidates;
- corners are intersections of adjacent chosen lanes;
- duplicate bundles are adjacent lanes in member order;
- fan trunks are one drawn line until the split;
- collinear points are simplified.

Rounding is render-only. It may shorten corner arcs, but it may not move ports,
change topology, create a crossing, or reduce clearance below the stage's node
clearance and wire pitch.

Labels ride the final route. Labels may slide along the route to avoid bodies
and other labels. Labels never move the route.

### 10. Failure Reports

A request can be unresolved because:

- `blocked endpoint`: no legal port candidate reaches the graph;
- `no path in routing graph`: the low-level graph has no path under constraints;
- `no conflict-free assignment`: the conflict search exhausted all states;
- `routing search budget exhausted`: implementation budget stopped the search.

Only unresolved requests become strays. Draw every solved route.

Reports name the endpoint pair, source span, stage, pitch, and reason.
Kept crossings are separate info reports naming the crossing point and link pair.

---

## Practical Guardrails

These rules keep the solver implementable.

- Solve per world first. Links in different worlds share no routing resources.
- Solve independent resource components separately after the seed pass.
- Route bundles wider than one lane as a single ribbon. Do not route duplicate
  members independently unless the bundle has already proven impossible whole.
- Prefer constraints over penalties for legality. Penalties may order legal
  choices; they must not make an illegal route drawable.
- Keep all resource coordinates integer or exact decimal values derived from
  existing coordinates and pitch. Avoid accumulated float drift in keys.
- Validate the rendered output independently. The validator is a bug detector,
  not a repair pass.

---

## Algorithm Notes

The chosen model borrows known pieces rather than inventing a bespoke rescue
machine:

| Approach | Use in Lini |
|---|---|
| Maze routing / A* | Good low-level path search for one bundle. Not enough alone because prioritized routing is order-dependent. |
| Hanan / visibility graph | Good finite graph for rectilinear paths; extended here with gap midlines and lane offsets for aesthetics. |
| Conflict-based search | Good high-level retry loop: conflicts create constraints, and each retry reroutes one affected bundle. |
| libavoid-style object avoiding | Good reference for orthogonal/polyline connectors and pins; Lini still needs its own deterministic Rust-native contract. |
| yFiles-style orthogonal routing | Good reference for fixed nodes, port candidates, bus/group routing, maze routing, and selective distance reduction. |
| Graphviz `splines=ortho` | Useful comparison, but not enough as a contract because its own docs note limitations around ports and edge labels. |
| ILP/SAT/MIP | Can prove optimal routes, but too heavy for normal compile/render use. Could be a future oracle for tests. |

---

## Deferred Strategies

### Straight

`straight` will draw one trimmed segment between endpoint sides. It will use the
same endpoint resolution, markers, labels, diagnostics, and strays as
`orthogonal`. Its obstacle and crossing rules are deferred.

### Curved

`curved` will draw a spline between endpoint sides. It will use the same endpoint
resolution, markers, labels, diagnostics, and strays as `orthogonal`. Its
obstacle, crossing, and bundling rules are deferred.

---

## Implementation Shape

The routing code should be split by strategy:

```text
routing/
  mod.rs              request expansion, shared reports, strategy dispatch
  orthogonal/         v2 solver and geometry
  straight.rs         deferred
  curved.rs           deferred
  labels.rs           shared link-label placement
  validate.rs         rendered-output law checks
```

The orthogonal module owns all orthogonal path decisions. There should be no
separate crossing audit, separation audit, port slide pass, or gap-growth
feedback loop.

Tests should pin:

- `pcb_fail`: `pwr -> mcu` is a direct bus, and `flash -> mcu` never wraps around
  the MCU while a left-side route exists.
- `pcb`: routes complete without layout growth and stay under a fixed time
  budget.
- sweep clearances: legal geometry at full pitch or half pitch, no silent loss.
- failure fixtures: blocked endpoints produce strays and diagnostics.
- determinism: repeated renders are byte-identical.

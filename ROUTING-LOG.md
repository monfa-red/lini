# Routing v2 — implementation log

> The v2 rewrite's plan and its **Execution log** — all seven stages landed
> (2026-07). Kept as the record of what was built and *why*: the log's dated
> entries hold every design decision the plan didn't anticipate, and the
> open-bug diagnoses future sessions start from — most importantly the
> **bug batch** entry on links_hard's four strays (estimate-overshot
> admission; needs connection-feasible pricing + coupled cross-axis
> placement bounds). Re-orient with `ROUTING.md` (the contract, the source
> of truth) first, then the Execution log at the bottom, newest first. If a
> rendered result looks stale: `cargo clean -p lini`.

**Goal:** Replace the v1 guess-then-repair router with the ROUTING.md v2
contract: a sequential weighted channel router with constructive placement,
a strategy seam (`orthogonal` / `straight`), and sequence messages migrated
onto `straight`.

**Architecture:** Six decide-once steps (keep-outs & worlds → channels →
requests → weighted search → placement → geometry). No audit, no repair, no
gap growth, no port pre-pinning: ports *are* end-run ordinates. Cost =
`length + 2·clearance·turns + 4·clearance·crossings`, links routed in
declaration order.

**Tech stack:** Rust, `insta` snapshots, `resvg` for visual verification
during development only (never in CI tests).

---

## Global constraints

Every stage inherits these; they restate AGENTS.md + the spec where routing
work is most likely to violate them.

- **No `unsafe`.** No panics on user input — impossible layouts are strays.
- **One mechanism per problem; no parallel implementations.** The v1 router is
  *deleted*, never kept alongside. If a stage needs behavior v1 had, port the
  code, don't re-derive a sibling.
- **Code reads as designed-for-v2-day-one.** No compatibility shims, no
  `_v2` suffixes, no dead flags. Comments only for non-obvious *why*.
- **Modularity:** one concept per file, split past ~500 LOC.
- **Determinism:** `f64::total_cmp` for every float comparison that orders
  anything; iterate `BTreeMap`/sorted vecs, never `HashMap` order; no
  wall-clock, no randomness.
- **Cost constants** live in exactly one place (`ortho/cost.rs`):
  `TURN_COST = 2.0 * clearance`, `CROSS_COST = 4.0 * clearance`, minimum pitch
  `= clearance / 2`. Put a comment at the definition: a per-diagram override
  (e.g. `link-crossing-cost:`) is a plausible future property — keep the
  constants threaded, not inlined.
- **Tests:** `insta` for output-shaped code; contract tests assert on route
  *geometry* (turn counts, ordinates, crossings) — fast, no image reading in
  CI. Rendered-PNG eyeballing (`--bake-vars` + `resvg`) is for development and
  the final visual pass only.
- **Git:** one purposeful change per commit; descriptive messages; **never**
  "Co-Authored-By"; `cargo fmt && cargo clippy && cargo test` before every
  commit that ends a stage; pushing is the user's call.

### Fixed cross-stage vocabulary

These names are the contract between stages (internals are yours):

```rust
// src/routing/mod.rs
pub struct Routing {
    pub links: Vec<RoutedLink>,          // crate::layout::ir::RoutedLink (unchanged contract w/ renderer)
    pub report: Vec<Violation>,
    pub strays: Vec<Stray>,
}
pub fn route(program: &Program, nodes: &[PlacedNode]) -> Result<Routing, Error>;

// src/routing/report.rs
pub enum Rule { Clearance, Separation, Contact, Crossing, Impossible }
pub enum Severity { Warning, Info }
pub struct Violation { pub rule: Rule, pub severity: Severity, pub links: Vec<String>, pub detail: String, pub span: Span }

// src/routing/ortho/ — the six-step pipeline
pub(crate) struct Chain {                // one link's route, search output → placement input → geometry input
    pub link: usize,                     // request index (declaration order)
    pub world: usize,
    pub runs: Vec<Run>,                  // alternating axes, ends first/last
    pub ends: [EndInfo; 2],              // path, side, rect, fan group
}
pub(crate) struct Run {
    pub axis: Axis,
    pub channel: usize,                  // index into that world's ChannelGraph
    pub span: (f64, f64),                // travel extent (provisional until geometry)
    pub ordinate: Option<f64>,           // None until placement
}

// src/routing/ortho/ledger.rs — the one committed-state ledger
impl Ledger {
    pub fn tracks_left(&self, world: usize, axis: Axis, chan: usize, span: (f64, f64), graph: &ChannelGraph) -> usize;
    pub fn crossings(&self, world: usize, band: Rect, axis: Axis) -> u32;
    pub fn side_free(&self, path: &str, side: Side, rect: Rect) -> usize;
    pub fn commit(&mut self, chain: &Chain, k: usize);
}

// src/routing/ortho/ladder.rs — the placement primitive
/// Order-preserving ordinates minimizing Σ(x_i − pref_i)², subject to
/// x_{i+1} − x_i ≥ pitch, lo_i ≤ x_i ≤ hi_i. Unique; caller guarantees feasibility.
pub(crate) fn ladder(prefs: &[f64], bounds: &[(f64, f64)], pitch: f64) -> Vec<f64>;
```

### Module map (end state)

```
src/routing/
  mod.rs        strategy dispatch, Routing, route()
  report.rs     Rule / Severity / Violation, crossing + stray reports
  straight.rs   the straight strategy (sequence messages; routing: straight)
  validate.rs   independent v2-law checker — test oracle, never a repair
  ortho/
    mod.rs      the six-step driver
    cost.rs     TURN_COST, CROSS_COST, MIN_PITCH (+ future-override comment)
    rect.rs     Rect                        (moved from layout/links)
    scene.rs    SceneIndex                  (moved)
    graph.rs    ChannelGraph decomposition  (moved, tests intact)
    request.rs  EdgeReq / Bundle / Fans     (moved from bundle.rs, split() deleted)
    ledger.rs   committed runs + side ports: closure, crossing counts
    search.rs   entries/punch + weighted Dijkstra
    place.rs    clusters, nesting order, preferences, ports
    ladder.rs   bounded min-separation isotonic regression (PAV)
    geometry.rs chains → polylines, jogs, self-loops, strays, min end segments
    labels.rs   label placement along routes (moved)
```

`src/layout/links/` ceases to exist. The renderer contract (`RoutedLink`
polylines, fillets at render time, concentric nesting, crossing-capped radii
in `src/render/links.rs`) is **unchanged**.

---

## Stage 1 — Demolition & scaffold

Everything v1-specific dies; the keepers move to their v2 home; the diagram
still compiles and renders (every link as an honest stray). After this stage
the tree contains no code that fights the v2 contract.

**Files:**
- Create `src/routing/{mod,report,straight,validate}.rs` (straight/validate as
  minimal stubs), `src/routing/ortho/{mod,cost,rect,scene,graph,request,labels,geometry}.rs`
- Move (git mv + adjust paths, keep unit tests): `layout/links/rect.rs`,
  `scene.rs`, `graph.rs`, `labels.rs`, `bundle.rs → ortho/request.rs`
  (delete `split()` and its callers), `geometry.rs` (keep `polyline`,
  `simplify`, `stray_segment`, `self_loop_chain` skeleton; delete
  chain-construction tied to v1 runs)
- Delete: `layout/links/{audit,capacity,feedback,runs,order,path,validate,mod}.rs`
- Modify `src/layout/mod.rs`: delete the gap-growth loop (`layout_mode`
  rerun block, `GapGrowth`, `grow`, `growable`, the `growth` param threaded
  through `attempt`/`layout_inst`/`lay_out_container_children`) and
  `Routing.starved` plumbing; `attempt` calls `routing::route`
- Modify `src/render/mod.rs` / `src/main.rs` / `src/lib.rs`: imports of
  `layout::links::{Violation, Rule, …}` → `routing::…`; keep
  `layout::cross` re-export alive by moving `cross()` (the transversal
  primitive shared with the fillet pass) into `routing/report.rs`
- Tests: delete `tests/linking.rs` + `tests/linking_sweep.rs` assertions that
  pin v1 behavior (compaction, gap growth, splitting, port slides). Mark
  link-bearing snapshot tests and the sample-law sweep
  `#[ignore = "routing-v2: re-enabled in stage 6/7"]`.

**Steps:**
- [x] Move the keepers; fix imports; delete the v1 router files
- [x] Excise gap growth from `src/layout/mod.rs` (see Execution log if the
      threading has drifted from this description)
- [x] `routing::route()` stub: expand requests via `request.rs`, return every
      link as a stray + `Rule::Impossible` violation (`detail: "routing v2 in
      construction"`)
- [x] Prune/ignore tests as above; `cargo fmt && cargo clippy && cargo test`
      green
- [x] Render `samples/pcb_fail.lini` — nodes + 8 strays, nothing panics
- [x] Commit (`refactor: raze v1 router, scaffold src/routing`)

**Done when:** no `layout/links` directory, no gap-growth code, green build,
strays render.

---

## Stage 2 — Search: ledger, entries, weighted Dijkstra

The Law-3 engine. Pure code against `ChannelGraph` + `Ledger`; no pipeline
integration yet (the stub keeps returning strays).

**Files:**
- Create `src/routing/ortho/ledger.rs`, `src/routing/ortho/search.rs`,
  fill `src/routing/ortho/cost.rs`
- Port from v1 `path.rs` (from git history): `entries()`/`punch()` largely
  as-is — an `Entry` gains its side's **port window**
  `(f64, f64)` (side span minus `clearance` corner margins); Dijkstra is
  rewritten for scalar weighted cost

**Interfaces (consumed by stage 4):**
```rust
pub(crate) struct Entry { pub side: Side, pub window: (f64, f64), pub tip: (f64, f64), pub axis: Axis, pub cell: usize }
pub(crate) fn entries(graph: &ChannelGraph, body: Rect, stub: f64, forced: Option<Side>, blockers: &[Rect], inward: bool) -> Vec<Entry>;
pub(crate) struct Route { pub cells: Vec<usize>, pub start: usize, pub goal: usize, pub cost: f64 }
pub(crate) fn cheapest(graph: &ChannelGraph, world: usize, starts: &[Entry], goals: &[Entry], ledger: &Ledger, k: usize, clearance: f64) -> Option<Route>;
```

**Guidance:**
- Cost is one `f64`: estimated length + `TURN_COST`·turns + `CROSS_COST`·crossings.
  Length estimate: L1 through cell entry midpoints (v1's estimator is fine);
  it must rank straight < dogleg < staircase — placement, not search, decides
  exact ordinates. Ties: goal-side rank, then start-side rank
  (right → bottom → left → top), then smaller cell id — total order, no NaN.
- Turns = axis changes along the cell path, **plus 2** for the single-channel
  case whose two entry windows don't overlap (the jog — ROUTING.md model
  step 4). The jog's perpendicular run must also pass `tracks_left` on the
  cell's crossing channel, and is costed as a crossing band like any run.
- Closure is counting, not simulation: `tracks_left` = capacity of the span's
  usable width at `MIN_PITCH` minus the **max point-load** of committed runs
  overlapping the span (sweep over span endpoints). An edge is closed when
  `tracks_left < k`. Sides close the same way: `side_free` =
  `floor(window / MIN_PITCH) + 1 − landed`.
- `crossings(band)` = committed perpendicular runs whose `(channel width ×
  span)` band intersects the candidate's band, counting bundle multiplicity.
- Fan groups: the shared end consumes one port slot and its side is fixed by
  the first-routed sibling (carry v1's `fan_pick` idea into stage 4's driver;
  the ledger only needs `side_free` to treat a fan group as one landing).

**Tests (unit, in-module):**
- [x] Port v1 `path.rs` entry/punch tests (facing sides, walled-off side,
      forced side, transparent-wall punch, blocked sibling)
- [x] `cheapest` picks facing sides for neighbours; picks an L for diagonal
      placement; detours around a closed channel
- [x] Weighted trade: a scene where the crossing-free route is > `CROSS_COST`
      longer → route crosses; shrink the detour below `CROSS_COST` → route
      detours. (Build committed state via `Ledger::commit` by hand.)
- [x] Jog: same-channel misaligned windows costs 2 turns and respects the
      crossing channel's capacity
- [x] `tracks_left`: overlapping committed spans reduce it by max point-load,
      disjoint spans don't; capacity floor at `MIN_PITCH` matches
      `floor(usable/(c/2)) + 1`
- [x] Determinism: 100 identical runs, identical `Route`
- [x] `cargo fmt && cargo clippy && cargo test`; commit
      (`feat: weighted search + capacity ledger for routing v2`)

---

## Stage 3 — Placement: ladder, clusters, preferences, ports

The Law-2 engine: per-channel constructive placement. Still pure; integrated
in stage 4.

**Files:**
- Create `src/routing/ortho/ladder.rs`, `src/routing/ortho/place.rs`

**Interfaces (consumed by stage 4):**
```rust
pub(crate) fn ladder(prefs: &[f64], bounds: &[(f64, f64)], pitch: f64) -> Vec<f64>;
/// Assign every Run::ordinate in every chain. Channels processed in fixed
/// order (world, axis, index); within a channel: cluster → order → ladder.
pub(crate) fn place(worlds: &[World], chains: &mut [Option<Chain>], reqs: &[EdgeReq], clearance: f64);
```

**Guidance:**
- `ladder` is bounded isotonic regression with minimum separation: substitute
  `y_i = x_i − i·pitch`, run pool-adjacent-violators on `y` with per-item
  bounds (pool blocks average their prefs; clamp each block into the
  intersection of its members' transformed bounds; re-pool on violation).
  Verify against brute force on small n — this function is the mathematical
  heart of Law 2, make its tests merciless.
- Clusters: runs in one channel chain-cluster when spans come within `pitch`;
  cluster pitch = `min(clearance, usable/(n−1))` floored at `MIN_PITCH`
  (search guaranteed fit; `debug_assert` it).
- Preferences and bounds per ROUTING.md model step 5, verbatim:
  interior run → channel anchor (midline between two keep-out walls; hug the
  keep-out wall when the other wall is the canvas edge); end run serving both
  ports → `clamp((centre_a+centre_b)/2, shared window)` with the shared
  window as bounds; end run serving one port → own side centre, own window
  as bounds.
- Order within a cluster: bundle members keep declaration order; otherwise by
  preference (`total_cmp`); tie → one-hop key (the neighbouring run's
  preference/ordinate, sign-flipped by turn direction so wires leave in the
  order they arrive — nested, never braided); final tie → declaration order.
  This comparator is the successor of v1 `order.rs`'s recursive walk. Start
  one-hop; if a contract test in stage 4/6 exposes a braid, deepen it and log
  the case in the Execution log rather than adding a repair pass.
- Ports are not placed separately: the end-run ordinate **is** the port.
  Fan siblings' shared end: one run/ordinate shared by the group.

**Tests:**
- [x] `ladder` vs brute force (n ≤ 5, random-ish fixed seeds hand-inlined):
      optimality, order, pitch, bounds — plus degenerate cases (one item,
      all-equal prefs, tight walls forcing exact pitch)
- [x] A 4-run bundle cluster centres its ladder on the midline preference
- [x] Two buses landing on one side ladder without braid (flash/pwr shape:
      prefs 300×4 + [285,295,305,315], pitch 10 → flash block yields
      [245,255,265,275], pwr exact — the worked example from the design)
- [x] Cluster chaining: spans touching within pitch merge; disjoint spans
      place independently (both may sit on the midline)
- [x] `cargo fmt && cargo clippy && cargo test`; commit
      (`feat: constructive placement — ladder, clusters, ports`)

---

## Stage 4 — Assembly: the six-step driver, geometry, first light

Wire search + placement into `ortho/mod.rs`, lower chains to polylines,
delete the stray-stub, and land the contract tests that prove the two pcb
bugs dead.

**Files:**
- Fill `src/routing/ortho/mod.rs` (driver), finish `ortho/geometry.rs`
- Modify `src/routing/mod.rs`: `route()` dispatches scopes to `ortho`
- Create `tests/routing.rs` (the v2 contract tests)
- Modify `src/lib.rs`: add the test hook
  ```rust
  /// Test-facing: routed polylines by endpoint pair, in declaration order.
  pub fn routes_str(src: &str) -> Result<Vec<((String, String), Vec<(f64, f64)>)>, Error>
  ```
  (thin wrapper: parse → resolve → layout → collect `RoutedLink.seg_from/seg_to/path`).

**Driver (ROUTING.md model, six steps):** build SceneIndex → per-world
`ChannelGraph`s (world ladder as v1: innermost first) → requests/bundles/fans
in declaration order → per bundle: entries per permitted side, `cheapest`
across the world ladder, commit to `Ledger` (no route in any world → all
members stray with reason) → `place()` over all chains → geometry: runs +
ordinates + jogs → polyline, collinear merge, end segments ≥ marker run-up
(`EdgeReq.stub_*`), self-loops around the keep-out corner (port v1
`self_loop_chain`), strays, labels (`labels::place`), report (exact crossing
count over final polylines using `report::cross` — one O(L²·S²) pass, tiny).

**Contract tests — `tests/routing.rs`** (helpers: `turns(path)` = direction
changes; `is_straight`; assert with geometry, never images):
- [x] **Consequences table, one test per row** (small inline `.lini` sources):
      facing aligned → 2 points; offset-within-windows → still 2 points;
      offset-past-windows → 4 points with mid-run on the gap midline
      (assert ordinate == midline ± ε); bundle ×4 → 4 parallel rails at pitch
      centred on midline; canvas-edge wire hugs keep-outs (ordinate ==
      keep-out wall); crossing beats orbit (assert total length < direct
      distance + margin, crossings reported == expected count)
- [x] **pcb_fail pins:** all 4 `pwr → mcu` have `turns == 0`; all 4
      `flash → mcu` have `turns == 2` and every point stays left of `mcu`'s
      right edge (no orbit); crossings reported == 0
- [x] Forced sides honored or stray (`a:left -> b` with `a`'s left walled)
- [x] Fan trunk: `a -> b & c` siblings share their first point
- [x] Self-loop right → top; both-ends-one-side errors
- [x] Containment link lands on the parent's inner side
- [x] Determinism: `routes_str` twice, assert equal
- [x] Render `pcb.lini`, `pcb_fail.lini`, `links_hard.lini` to PNG
      (`--bake-vars` + `resvg`) and **look at them** — development
      verification, not CI
- [x] `cargo fmt && cargo clippy && cargo test`; commit
      (`feat: routing v2 orthogonal pipeline — search, placement, geometry`)

**Done when:** the two pcb bugs are impossible per tests, samples render sane
wires, and `time ./target/release/lini samples/pcb.lini -o /dev/null` is
< 50 ms.

---

## Stage 5 — Straight strategy & sequence migration

**Files:**
- Fill `src/routing/straight.rs`: a link is one segment between two anchors,
  trimmed to the endpoint bodies (reuse `stray_segment`'s trim math — one
  mechanism), plus the rectangular self-hook; markers/labels ride as on any
  wire
- Modify `src/layout/sequence/messages.rs`: messages emit `RoutedLink`s
  (polyline + markers + label texts) through `straight` instead of lowering
  to `prim::line`; the renderer's existing link path (fillets, markers,
  label masks) draws them
- Modify `src/resolve/links.rs`: `routing: straight` becomes a legal value
  (any scope): straight trimmed segments, no avoidance, no report;
  `curved` stays deferred with the same error
- Update sequence snapshots (`cargo insta review`) after a visual PNG pass on
  `samples/sequence.lini`

**Steps:**
- [x] `straight.rs` + unit tests (trim math, self-hook shape, marker anchors)
- [x] Sequence migration; visual check that arrows/hooks/labels look
      unchanged (hook rounding now via the shared fillet pass — log any
      radius drift in the Execution log)
- [x] `routing: straight` end-to-end test in `tests/routing.rs` (an oblique
      pair renders one trimmed segment)
- [x] Snapshots re-accepted for sequence; `cargo fmt && cargo clippy &&
      cargo test`; commit (`feat: straight strategy; sequence messages ride
      the routing spine`)

---

## Stage 6 — Validator & the hard tests

The independent oracle plus the adversarial suite. This is where "works"
becomes "works at the edge".

**Files:**
- Fill `src/routing/validate.rs` for the v2 laws; re-enable the sample sweep
  (`tests/linking.rs` → rename `tests/laws.rs`)
- Extend `tests/routing.rs` with the stress suite

**Validator checks (each drawn polyline, no router knowledge):** ends
perpendicular on a side, ≥ clearance from corners; no point of any link
inside a foreign keep-out (own end segments excepted); pairwise separation
≥ `MIN_PITCH − ε` everywhere, with < clearance gaps flagged unless the local
corridor demonstrably can't hold its wires at full clearance; crossings
square-on point contacts, each present in the report and vice versa
(`reconcile` both directions, as v1 did); fan trunks shared-then-split.
Port v1 `validate.rs` geometry helpers from git history where they fit —
delete the compaction/slide excuses (those laws are gone).

**Steps:**
- [x] Validator + unit tests (hand-built violations are caught: oblique
      landing, corner graze, sub-pitch hug, unreported crossing)
- [x] `tests/laws.rs`: every sample routes with zero Warning-severity
      violations
- [x] **Tightness sweep** (the "right before impossible" test): a facing pair
      with a k-bundle, `gap` swept from roomy down to below `(k−1)·c/2`;
      assert lawful at every step, pitch compresses only when full clearance
      can't fit, and the stray appears exactly at the capacity boundary —
      no silent squeeze, no ugly detour
- [x] **Side-capacity sweep:** land n wires on one short side, n swept past
      `floor(window/MIN_PITCH)+1` → excess routes to other sides, then
      strays; port order matches wire order at every n (no braid)
- [x] **Crossing-vs-orbit torture:** the pcb_fail shape with the corridor
      progressively walled until crossing is forced; assert crossings appear
      one at a time (never a wrap), each reported
- [x] Determinism: full `routes_str` over every sample ×3, byte-equal
- [x] Perf tripwire: route `pcb.lini` 10× in a test under a generous bound
      (e.g. < 2 s debug total) — catches any audit-style regression
- [x] `cargo fmt && cargo clippy && cargo test`; commit
      (`test: v2 law oracle + adversarial routing suite`)

---

## Stage 7 — Polish, snapshots, docs

- [x] Visual pass: render **every** sample `--bake-vars` → PNG, read each one
      (AGENTS.md: don't make the user spot-check). Specifically confirm:
      concentric fillets on turning buses (also at compressed pitch — the
      radii grow by the *actual* track offset), crossing never mid-arc,
      labels legible, `pcb.lini` pretty end to end
- [x] Re-accept remaining snapshots; remove every stage-1 `#[ignore]`
- [x] Cosmetics-last pass over `src/routing/` (naming, dead imports, doc
      comments; every file ≤ ~500 LOC)
- [x] Docs: purge stale references — `PLAN.md` routing/architecture sections
      (point at `ROUTING.md`), `graph.rs`'s "PLAN.md §Architecture" header,
      SPEC.md §9's "only mode built today" line (now orthogonal + straight),
      README if it names routing modes
- [x] Full gate: `cargo fmt --all -- --check && cargo clippy && cargo test`
- [x] Final commit; leave pushing/merging to the user
      (`superpowers:finishing-a-development-branch`)

---

## Execution log

Executing sessions: append dated notes here — decisions the plan didn't
anticipate, gotchas, deferred items, comparator cases that needed deepening,
anything the next session must know. Keep entries terse.

- **2026-07-17, the stadium sweep (owner: water → roof rounded garden in
  three sharp turns).** The corner-pair detour was polygonal — out, along
  the face, back in, with Catmull tangents wobbling at the vias. The
  spline gained per-knot **forced tangents** (`spans` takes
  `(Pt, Option<Pt>)` knots; ends force their normals as before) and the
  detour became one **stadium sweep**: the two vias carry tangents along
  the face, so entry is one S, the face a straight glide, exit one S. A
  single-apex variant was tried first and failed geometry: one interior
  knot cannot round a wide body crossed off-centre (the tail beelines to
  the port and cuts the exit corner — needed apex depths past any
  budget). Also fixed on the way: the grazed one-sided body (all four
  corners on one chord side) panicked the corner filter — the detour
  side is now simply the chord side with the smaller worst deviation
  (the empty side for a graze: bow away, don't orbit), vias from the
  two corners nearest it. Known softness: duplicate rails can split
  when one member's sweep clears and its twin's doesn't (different
  ports → different verdicts) — both halves stay smooth; revisit only
  if it reads badly in practice.

- **2026-07-17, the hooked-landing gate (owner: n2 → e1 still elbowed).**
  The handle floor treated the symptom; the disease was the dodge
  reshaping a wire's approach — a via can turn the final span nearly
  parallel to the landing side, and a perpendicular port then demands a
  hook no handle length can soften. New acceptance term in the dodge
  policy: a detour must also **land clean** (`curve::hooky` — an end
  span advancing < `HOOK_RATIO` (0.1) of its length along its forced
  tangent is a wrench); a hooked dodge falls back to the smooth direct
  fit + report, same as a second body. First cut tied the gate to the
  floor threshold (0.25) and wrongly rejected the mindmap's C arc
  (ratio 0.24) — a sweep is 0.2–0.4, a wrench ~0.02; 0.1 separates
  them. links_hard's n2 → e1 pair now draws two parallel diagonal S
  curves straight into e1.top; mindmap byte-identical.

- **2026-07-17 (later still), the handle floor.** Diagonal connections
  and dodge entries elbowed: the forward-travel clamp collapses a handle
  whenever a span's chord runs nearly perpendicular to its tangent.
  Handles now floor at half the pull — crossing is lawful, sharpness is
  not — links_hard-natural's n2 → e1 landings and the hub detours sweep;
  the mindmap moved by a hair on its steepest arms (re-blessed, still
  warning-free). Also that pass: a dodge via seats at a full clearance
  (2 × margin) so a deliberate pass-by reads as passing, not grazing —
  the confirms-arc-hugging-CI-fmt report.

- **2026-07-17 (later), natural v2 smoothness pass (owner feedback).**
  The first cut still read busy: 16 px stubs made every wire "run
  straight then turn", multi-body via weaves slalomed, and far-projection
  ports wandered off the side centres. Three inversions, smoothness now
  outranking avoidance: (1) **ports sit at the side centre** — landings
  spread around it at pitch, ordered by far ends (order only, position no
  longer pulled); (2) **the stub is the marker's run-up alone** (`NUB` =
  2 px when unmarked) — the curve begins at the port; ortho keeps its
  clearance-length stubs, the seam is natural's driver, not `EdgeReq`;
  (3) **one gentle detour or none** — dodge targets only the direct
  fit's first offender and stands only if it clears the wire entirely;
  a second body or a spent budget falls back to the smooth direct fit +
  reports (the envelope prune and via merge died with the weaves;
  `DODGE_ROUNDS` back to 6). mindmap.lini's cross-link would now draw
  straight through its column — the sample forces `:right` on both ends
  and gets the clean outside arc (zero warnings, laws green). Mindmap
  snapshot re-blessed; light + dark + links_medium-natural eyeballed.

- **2026-07-17, natural v2 — curve-first (PLAN-NATURAL).** The alpha.1
  corridor-first natural is deleted whole: the ROUTING.md natural section
  is now its own five-law contract (contact, smoothness, directness,
  respect, determinism) and `natural::route` a sibling driver — sides by
  facing, ports laddered per node side before any curve, then per wire an
  independent direct spline with bounded via dodges
  (`natural/{port,curve,dodge}.rs`; `corridor.rs` and the `EdgeReq::
  corridor()` seam gone; the crossing report moved to the
  `routing::route` spine). Numbers: mindmap.lini 2.9 s debug → 29 ms
  (the profile had shown 99 % of the corridor build's time in
  `cheapest`/`tracks_left`); every non-mindmap snapshot byte-identical.
  Decisions the plan didn't anticipate: (a) a wide body needs its corner
  *pair* on repeat offence — a single escalating via lets the arrival
  tangent sag into the far corner; (b) near-coincident vias from two
  bodies merge and same-side dips prune to the envelope (both fix real
  kinks/wiggles found on natural-forced links_medium/links_hard); (c)
  `spans()` dedupes knots *before* reading tangents — the old
  filter-after left a G1 kink at a dropped duplicate; (d) the checker's
  directness law binds only undodged fits (a lawful arch over a tall
  wall swings past its own takeoff); (e) `DODGE_ROUNDS` landed at 12 —
  6 starved the mindmap cross-link's weave past three cards; (f) a
  same-side forced self-loop draws (natural has no stray), and natural
  duplicates need no bundle machinery — the port ladder alone makes the
  rails. Known softness, accepted: duplicates that dodge differently can
  pinch a hair under the floor mid-flight (checker flags it; only on
  forced-natural contended scenes, not samples).

- **2026-07-11, the 0.21/0.22 round's routing-adjacent changes (PLAN-ALPHA
  M6).** Three seams moved; the router core is untouched. (1) The root
  `layout: drawing` / `layout: sequence` arms now call `routing::route(…)`
  — nested ordinary scopes' wires used to vanish (the arms collected only
  engine-owned links). (2) That exposed a label-pairing drift: the label
  pass walked all program links while the request pass filtered
  engine-owned ones, so statement numbering disagreed and a routed wire
  could wear a sequence message's label. The ownership filter is one
  shared predicate now — `ortho::request::is_routed` — used by both
  passes; anything touching request/label statement grouping must go
  through it. (3) Chain expansion moved to desugar (`a -> b -> c` →
  `a -> b; b -> c`, per-hop ops in the AST): the router now only ever
  sees 2-endpoint wire requests, but the hops share the statement's span,
  so the span-keyed stmt/expansion grouping still treats them as one
  statement — label distribution and per-statement crossing accounting
  are unchanged by construction. Multi-endpoint requests still exist for
  drawing measure chains only, which never reach the router.

- **2026-07-04, the checker reads the wall-hugger's own channel (user bug
  batch 6).** The author's links_hard tuning (gap 34, two blues) tripped
  the clearance-9 sweep: four Separation flags, each "with room for full
  clearance" — the excuse model judged a genuinely pinched five-wire
  pocket spreadable. The checker's `channel_at` resolved a wire's channel
  by point containment; a wire hugging a wall (normal since walls charge
  nothing) sits exactly on two channels' shared coordinate, and the
  EPS-inclusive test picked the neighbour — whose shorter travel then
  clamped the corridor query past the very stretch that pins the wire
  (east → west's tail beside hub read as free to slide to x = 1, through
  hub's keep-out). `channel_of` now prefers the candidate whose travel
  covers the wire's whole extent, falling back to the old pick. Checker
  only — engine output untouched, all samples byte-identical. The
  batch-3 open item (fan siblings 11 apart "with room for 16") was this
  same tie: re-tested, resolved.
- **2026-07-04, the chain never crosses a zero gap (user bug batch 5).**
  links_medium's `cat → bowl & water` ports sat pinned at the top of
  their windows instead of the side centres. The fan's end runs owe
  nothing to the packed bowl↔dog band (spans 78 apart), but one cluster
  legitimately spans both (cat → roof's long H bridges them), and the
  ladder's **total order** still forces `x_i ≤ x_j` across a zero-sep
  boundary — so the band, relieved to exactly its window, crushed the
  fan's port to the window edge through an order constraint no pair owed.
  `chain_ok` now also requires every adjacent gap positive: a chain with
  a zero gap over-constrains (order across the boundary) exactly as an
  under-sized bridge under-constrains, and either sends the cluster to
  the pairwise solver, which imposes only the true contending
  constraints. When the chain holds, the two feasible sets coincide and
  the ladder stays the exact, cheaper solve. Ports of uncontended end
  runs land on their side centres (contract test
  `uncontended_fan_ports_take_their_side_centres`); links, links_medium,
  links_hard drift (ports centring), visually verified; benchmark flat
  (2.40 s vs 2.26 s, still well under the pre-batch-4 2.77 s).
- **2026-07-04, cross-boundary separation owned by placement (user bug
  batch 4).** The soft-boundary margin is retired; ROUTING.md §Vocabulary
  updated: no wall charges anything — separation across a shared boundary
  is placement's job like any other. Four pieces, one principle (Law 1 is
  a distance, and one mechanism owns it):
  - **Walls charge nothing** (`Corridor::usable` = walls; `Channel.soft`,
    `soften`, the corridor's faced-margins all deleted). Corridor capacity
    gains the lane the two half-margins used to eat; the canvas-edge hug
    is now exact (the hug test pins keep-out + clearance, no surrendered
    half). The margin guarded near-tip pairs in mutually-unabsorbed
    abutting channels — clusters now couple those directly: corridors
    whose walls meet (exact sweep-edge equality) share a cluster when
    their spans are near, and the ladder holds the pitch.
  - **Interior preferences anchor the corridor a run can inhabit**
    (`Corridor::clipped` to the corner clamp): a span kissing a keep-out
    corner lets the walk absorb a void the clamp forbids, flipping the
    anchor rule (outer-hug vs midline) between twin rails and ordering
    them into unplaceable chains.
  - **Separations are the distance model** (`place::owed`): runs
    alongside owe the full pitch across; runs past each other owe
    `√(pitch² − gap²)` — tips a clearance apart along travel may share an
    ordinate (two collinear segments a clearance apart are lawful), so
    stage 6's recorded conservatism is spent. Coupling stays inclusive at
    exactly a clearance (round two never forgets a pair); only the owed
    amount tells the truth. `chain_ok` becomes sum-of-gaps ≥ owed;
    `pairwise` and the admission probe's floor ride the same function.
  - **Results:** w2 → s1 draws at full pitch (contract test
    `a_duplicate_pair_keeps_full_pitch_beside_a_keepout`); links_hard's
    alpha → delta now routes through the middle instead of orbiting the
    west perimeter; 8 samples drift, each visually verified (hero, links,
    links_hard, links_medium, links_simple, pcb, text, themes — mostly
    tighter hugs and nicer nests); the link-heavy benchmark runs 18%
    *faster* (2.26 s vs 2.77 s for 50×5 compiles — fewer denials, less
    relief); laws sweep green at every clearance.
- **2026-07-04, concentric fillets across asymmetric pitches (user bug
  batch 3).** links_hard ships at gap 32 (everything routes — the showcase
  renders whole; the four gap-30 strays and their pin are history), and
  the fillet nest generalised.
  - **Fillet nesting** (`render/links.rs::fillet_targets`): the old
    cluster key demanded corners on one *exact* diagonal (equal x and y
    offsets within 1e-6). Two relief groups can compress the two axes
    differently — links_hard's blue bundle rides a V corridor at pitch 8
    and a port ladder at 9.94 — so co-turning corners sat on a skewed
    diagonal and every arc drew the base radius, pinching the gaps
    through the turns. Corners now chain per pair: same quadrant, offset
    outward on **both** axes at lane scale, radius growing by the mean
    offset — for equal offsets exactly the old concentric family
    (byte-identical), for skewed ones the choice whose arc gap never
    drops below the tighter leg pitch (nested circles: gap ≥ (r₂−r₁) −
    |ΔC| = mean − half the skew = the smaller offset). Interleaved
    independent nests stay independent (the backward scan skips
    one-sided offsets). pcb's rf pair and links_medium's parallel
    detours pick up nests they always deserved; snapshots re-accepted
    after a visual pass.
  - **Open, diagnosed — sub-clearance pitch without visible scarcity**
    (user: w2 → s1 ×2 middle legs at 7.5 where the void looks wide).
    Two stacked corridor-model effects: (a) the pair's corridor reads
    *diverge* — one span ends exactly at gamma's keep-out corner and the
    walk absorbs the whole western void (walls to −223.5), the other
    crosses the corner and stops at −73.5 — so their prefs and bounds
    disagree and relief squeezes the mixed boxes to 7.5; (b) the
    surviving wall is charged the soft-boundary c/2 margin over the whole
    span although gamma's keep-out backs it for all but a sliver —
    usable 9 instead of 15, when 12 fits with one rail hugging the
    keep-out. (b) is contract-specified (ROUTING.md §Vocabulary: each
    side of a shared boundary keeps half a clearance off it) and guards
    exactly the near-tip pairs in mutually-unabsorbed channels that
    clustering never couples; most such neighbours *do* cluster (either
    side's corridor absorbing the other suffices), so the margin
    double-guards them at the cost of pitch. Fixing it honestly means
    either per-stretch wall character (hard where keep-out-backed) plus
    cluster-coupling for abutting near-tip pairs — one mechanism owning
    cross-boundary separation — or accepting the margin as the law.
    A contract decision, not a patch; parked for the user.
- **2026-07-04, placement-aware admission (user bug batch 2).** The
  stage-6 "honest fix is a placement-aware admission probe" landed;
  every known-limit pin healed and dropped. Every sample byte-identical
  at native attributes.
  - **The probe** (`ortho/admit.rs`): before a route commits, the driver
    runs the real `place()` over a copy of every committed chain plus the
    candidate's k rails, refreshes spans from the final ordinates, and
    judges every contending pair against the half-clearance floor — no
    separate model, so nothing to drift (a first cut re-judging the
    *chain* feasibility on provisional spans false-denied the side-spill
    sweep: committed corner estimates manufacture phantom contention that
    only the two-round simulation resolves). A failing route becomes a
    `Deny` on the same learned-closure loop the ledger rides. Shared
    machinery factored from place.rs: `collect`, `clusters_of`,
    `arrange`, `bound`, `overrun`; the probe chain now carries its fan
    groups so a sibling merges with its committed twin instead of
    contending.
  - **All three pinned cells heal**: links_hard @8 — `beta → gamma`
    denied from the 6 px sliver (point-load 2 fits, but the full-height
    run chained between two span-disjoint neighbours needs both gaps),
    reroutes east, zero breaches; links_medium @13 — one honest stray;
    pcb @12 — the rf bundle's own corners spread its H legs into a 1 px
    refreshed-span pocket (the stage-6 coupled-axis case: admission saw
    7 px on provisional spans), now caught by the simulation and strayed
    honestly. `known_limit` is gone from tests/laws.rs.
  - **Fan sibling braid, diagnosed and deferred** (user-reported:
    `hub → n1 & e1` crosses itself right after the split, links_hard @8).
    A clean twin route ties: a committed rail sitting exactly on a travel
    endpoint is a corner in the rail's own channel — both estimate the
    same anchor — and `crossings_covering`'s half-open bound charges it,
    so crossing and clean routes price equal and the tie-break drew the
    braid. A strict bound fixes it and breaks the mirror case
    (links_simple's fan then draws two *real* crossings the inclusive
    charge steered around): whether a corner-at-anchor crossing is real
    depends on which flank the nesting order gives the corner's
    perpendicular leg — deciding it truly needs the order walk over both
    chains at pricing time, and the ledger holds spans, not topology.
    The braid is a lawful, counted Info crossing; the user accepted it
    (2026-07-04) and the repro pin was dropped — the mechanism stays
    documented at `Ledger::crossings_covering`.
- **2026-07-03, run-order totality + lawful preferences (user bug batch,
  post stage 7).** Two placement bugs fixed at source, one new known-limit
  pin, one open diagnosis. Every sample byte-identical at native attributes.
  - **`cmp_runs` was not a total order** (links_hard @ clearance 6 panicked
    Rust's sort — latent; the stroke-width 1.6→2 bbox shift exposed it).
    Each pairwise judgment is sound, but the three criteria (walk±,
    convention, same-chain est→index) don't compose: the geometric walks
    ordered the hub fan trunk *between* the two end runs of `east → west` —
    a chain revisiting the middle corridor — while the same-chain
    convention said the opposite; a cycle no pairwise tie-break can fix
    (both conflicting edges were genuinely geometric vs conventional).
    Fix in `order.rs`: the cluster's order is built whole — `ranks()` sorts
    preference classes, then linearly extends the pairwise judgments:
    geometric (anti-braid) edges bind, conventions rank what geometry
    leaves free, declaration settles ties; a braid-forced knot (geometric
    edges themselves cyclic) competes as one pool. Where judgments are
    consistent — every cluster the old comparator sorted without panicking
    — the extension is provably their unique order, so nothing lawful
    moved. Unit test pins the links_hard triple.
  - **A duplicate bundle's trunk rails collapsed onto one ordinate**
    (user repro: `nn2 → ee1` ×3 S-curving across a 3×3 grid, gap 35 —
    drawn 0 apart, floor 6). Round-two spans reach through voids far wider
    than the pockets their corners pin them to, so `chain_prefs`' corridor
    anchors landed *outside* the runs' own bounds; the preference-first
    sort then interleaved the N2 trunk (bounds ≤ 22) with the E1 pocket
    (bounds ≥ 50.5) — an order no solver realises — the zero-gap bridges
    broke the chain model and pairwise's final clamp piled all three trunk
    rails onto x = 22. Fix in `place.rs::settle`: a preference is the
    nearest lawful ordinate to its aesthetic target — clamped into
    law range ∩ corner clamp — which makes the sort's stated premise
    ("prefs sit inside their boxes") true by construction. The clamp is
    the identity wherever the invariant already held: all samples
    byte-identical. Test: `a_bundle_of_s_curves_keeps_pitch_on_both_legs`.
  - **links_hard @ 8 joins the known-limit pins** (links_medium @13,
    pcb @12) — same admission blind spot, previously masked by the @6
    panic: admission counts point-load (the chan-22 sliver's 6 px usable
    lawfully holds `hub→n1` and `alpha→delta`, span-disjoint), but
    `beta→gamma` runs the full height between them and the nesting chain
    needs both gaps at once — 8 px in a 6 px band; pairwise leaves
    2.67/3.33 (< floor 4). The honest fix is still the placement-aware
    admission probe.
  - **Open, diagnosed:** the user scene at clearance 16 draws its fan
    siblings 11 apart where the checker proves room for 16. The engine's
    relief is working as designed *on its corridor read* (11 px usable —
    the pair spans the whole band); the checker's rebuilt per-wire ranges
    disagree. Engine corridor walk vs checker excuse model need
    reconciling; a session of its own.
- **2026-07-03, stage 7.** Done (snapshots, cosmetics, docs; the bug batch
  above landed first so snapshots were accepted once).
  - Visual pass: all 31 samples rendered `--bake-vars` → PNG and read.
    pcb end-to-end clean, flash/pwr/usb buses fully concentric (radii
    10/20/30/40 measured in the SVG arcs), links_medium/links_simple nest
    without braids, links_hard stands at its four pinned strays.
  - Snapshots re-accepted; both stage-1 `#[ignore]`s removed (conformance,
    hello). Link samples stay excluded from snapshots by design — routing
    is gated semantically (tests/laws.rs, tests/routing.rs). links.lini
    gained its first accepted snapshot.
  - Splits along concept seams, code ≤ ~500 lines per file:
    `ortho/pairwise.rs` (the general pairwise settle) out of place.rs,
    `ortho/entry.rs` (punches, port windows, clipping) out of search.rs,
    `validate/excuse.rs` (Law 1's contention-component excuse) out of
    validate.rs. Shared predicates (`contend`, `near`) stayed with the
    cluster model in place.rs.
  - Docs: SPEC §9/§10/§16/§20 now say `straight` is built (`curved` alone
    deferred); the resolver's deferred-routing message pointed at §19,
    fixed to §20; sequence/mod.rs's PLAN.md pointer and README/conformance
    references to the old `tests/linking.rs` updated.
- **2026-07-03, bug batch (user-reported, pre stage 7).** Two fixed, one
  diagnosed to its architectural root and deferred whole:
  - **Duplicate parallels braided** (links_simple `over`/`chat`,
    links_medium `.happy` ×2). Exact parallels tie at every estimate; every
    cluster fell to the declaration tie, each flipped by its own walk
    parity `m` — the clusters disagreed about who is inner. `cmp_runs` now
    falls to one oriented convention (`order::convention`): the
    earlier-declared wire keeps the left of its own travel through the
    queried channel — the offset-curve rule, self-consistent across every
    channel a pair shares. Tests: `duplicate_detours_nest_without_crossing`,
    `links_simple_reports_zero_crossings`.
  - **Fillet radii squashed** (pcb_fail outer track drew r 27.9, nest wants
    40). Both `fillet_targets`' ceiling and `rounding::round` capped every
    corner at *half* of each adjacent leg. The real constraint is joint:
    the two arcs sharing a leg together fill its length; a terminal leg
    belongs to its one corner (marker pull-back already shortened it).
    `round()` scales an over-full leg's pair in proportion to their desires
    — concentric desired sums are constant per shared leg, so one factor
    scales the whole bus and pitch stays uniform under squeeze.
  - **links_hard's 4 strays are NOT capacity truths** — stage 6's "no
    k-track exit at gap 30 / clearance 12" is hereby corrected. Lawful
    routes exist (hand-verified against the channel table): hub→n1 up the
    west flank with a corner shuffle through the H sliver below north;
    n2→e1 ×3 down through north's bottom wall into the hub pocket and
    into e1.left, V legs ending above hub's inflated top. They stray
    because **admission prices estimate-overshot spans**: a run's span
    reaches its neighbours' *anchor estimates* (mid-void), which poke past
    fragment boundaries the traversed cells never cross — the corridor
    walk then loses its absorbing neighbours and reads an inverted sliver.
    Three fixes tried, each surfacing coupling one layer deeper: (a)
    in-search pass-capacity (prev-centre→next-centre spans on straight
    extensions) — sound, but the probe still denies on estimates; (b)
    containment-based deny — sound, but estimate-driven denials contain no
    pass, so the loop loses progress and *more* links stray; (c) admission
    over the traversed extent (`Run.ext`) — all four draw, but **placement
    cannot realize them lawfully**: the lawful geometry needs an H run
    hugging the top of its pocket because its perpendicular *neighbour's*
    range ends there — a coupled cross-axis constraint placement doesn't
    model (corner clamps only bound by travel extents), so round-2 span
    refresh stretched V legs into west's latitudes (11.5 < 12) and pinned
    three wires onto one boundary ordinate (0 apart). The honest fix is a
    stage of its own: connection-feasible admission (price the traversed
    extent + the nearest lawful connection ordinate, not the anchor) plus
    coupled-axis placement bounds (a run's law range clipped by its
    perpendicular neighbours' placed/feasible ordinates). Until then the
    four strays stand: honest, conservative, and pinned in laws.rs with the
    real reason.
- **2026-07-03, stage 6.** Done (validate.rs filled, tests/laws.rs, three
  adversarial sweeps in tests/routing.rs). The oracle earned its keep on day
  one: it found six real engine bugs, each fixed at source — stage 6 turned
  into half validator, half the bug hunt the plan hoped it would be.
  - **`RoutedLink` gained `strategy: Strategy`** — the checker judges
    orthogonal wires only (a `straight` wire is lawfully oblique; sequence
    messages ride the same stream). `testing::drawn_edges`/`declared_edges`
    count per strategy: completeness is an orthogonal property (a
    `routing: straight` pair whose trim is empty lawfully draws nothing,
    and a root sequence's messages never lived on a `PlacedNode`).
  - **The checker's separation excuse** is one mechanism: floor (c/2)
    absolute; a sub-clearance gap excused only if the pair's **contention
    component** (parallel wires transitively owing pitch, drawn order kept)
    cannot spread to full clearance under per-wire lawful ranges — port
    window ∩ corridor usable, the graph rebuilt from `child_rects` of the
    pair's common world (walked up like the world ladder). Judged by
    longest-path reach, so a chain pinched at any cross-section excuses the
    group it compresses with. Simpler prongs (side-only, corridor-only,
    single cross-section) all mis-judge multi-window chains — tried and
    discarded.
  - **Engine bugs the oracle caught, and their fixes:**
    (a) *Phantom relief* (links.lini): provisional spans meeting at a shared
    estimate charged pitch never owed; the relief then compressed two nodes'
    windows as one scarce side. Fix: `place()` settles twice — round two
    re-derives spans from round-one ordinates (the search's probe-refine
    shape) and settles real contention.
    (b) *Cross-world blindness* (links.lini @6): an inner world's port and
    an outer punch landing on one physical side never coupled. Fix: items
    group by axis alone; clusters union across worlds on shared
    `(side, rect)` landings.
    (c) *Chain-model coupling* (links_hard): the stage-3 "known
    approximation" bit both ways — a zero-sep bridge dissolves a contending
    pair's pitch while order+envelope bind travel-disjoint groups. Fix:
    such clusters settle on true pairwise constraints (feasibility relief
    on the gap DAG + Dykstra projections; exact ladder kept for chains).
    (d) *Corner escape* (links_hard @9): a run priced over an estimated span
    drew through hub when a later round moved its far corner. Fix:
    `corner_clamp` — every ordinate stays inside its perpendicular
    neighbours' channel travel extents, so a corner never leaves either
    run's channel. (A max-extent bounds variant was tried first and
    reverted: pricing a ±200-long leg over the hull of its neighbours'
    ranges manufactures scarcity.)
    (e) *Ledger fragment blindness* (links_hard @8): `max_load` only counted
    commits in absorbed channels; a partially-covering fragment's rails
    park in the same void invisibly, so two overlapping corridors each
    admitted a full complement. Fix: foreign commits count wherever their
    estimated ordinate lies inside the corridor walls.
    (f) *Overflow blindness* (links_hard @9, the caption): `child_rects` and
    `gather` collapsed subtrees to the node's own rect; a group's caption
    overflowing its bbox was invisible to every world graph and wires drew
    through the drawn text. Fix: `SceneNode.overflow` — a collapsed
    keep-out is the rect plus each overflowing descendant rect (a hull was
    tried and reverted: it walls off free space beside a narrow caption
    and strayed hero.lini).
  - **Boundary condition worth remembering:** round one separates
    contenders by exactly the pitch they owe, so refreshed spans sit at
    exactly one clearance — `near` must be inclusive there (strict `<` let
    round two forget the contention and collapse wires onto a shared
    corner). The checker's component edges match, so both models charge
    tips flanking at exactly the pitch; wires that could lawfully share an
    ordinate across an exact-clearance tip gap ladder apart instead — a
    recorded conservatism.
  - **Relief is feasibility-driven now**, both paths: compress only what a
    greedy/longest-path pass proves cannot fit (the envelope test squeezed
    stretches whose staggered boxes actually held full clearance). A chain
    the floors cannot save falls through to `pairwise`, whose final clamp
    keeps windows and walls absolute — pitch carries the visible debt.
  - **Known limits, pinned in tests/laws.rs** (`known_limit`): admission is
    per corridor and per side, so a group jointly pinched by several nodes'
    port windows can over-admit by a hair and compress below the floor
    instead of straying — links_medium @13 and pcb @12 sit exactly there.
    The honest fix is a placement-aware admission probe; future work.
    Native attributes: all samples clean; links_hard's 4 strays pinned.
  - **Deviations from the plan's test sketch:** the tightness sweep sweeps
    the shared window (node height, sides forced) — the `gap` knob doesn't
    bound a facing bundle, and unforced the router lawfully spills the whole
    bundle over the top (a better outcome the test notes). The torture test
    pins that *every* rail costs exactly one crossing: a dodge needs two
    turns — already the crossing's whole price — so Law 3 never hops; count
    increments one per rail, no wrap, each reported. Perf tripwire budget is
    10 s / 10 debug compiles (~4 s measured; debug ≈ 25× the ~17 ms release
    route — catches an audit-style blowup, not machine variance).
  - `entries()` clips port windows by blockers in the punch stretch
    (labels inside transparent ancestors reduce the lawful window; a
    mid-window blocker keeps the wider shore). The punch itself still casts
    from the side centre — a blocker dead ahead kills the side even when
    the clipped window has room; logged for stage 7+.
  - `routing: straight` trim-empty silence stands (strategy has no report
    by contract; the checker skips straight wires).
- **2026-07-02, corridor view** (between stages 5 and 6; fixes the pcb_fail
  aesthetics the user flagged — fragment-midline bends, sub-clearance pitch
  with room to spare — both symptoms of the fragmentation weakness the
  stage-4 log recorded). The sweep slices one free corridor into several
  same-axis channels wherever any far-away node edge cuts the strip list;
  every consumer now reads the reassembled **corridor** instead
  (`ChannelGraph::corridor(axis, chan, span)`: walk shared boundaries into
  each same-axis channel free over the whole span, clamped to the origin
  channel's travel extent so an end segment's keep-out tail never blocks the
  walk). Consequences and decisions:
  - **One decision surface, three consumers**: `Corridor::{anchor, usable}`
    replace `Channel::{anchor, usable}` (deleted); `Ledger::tracks_left`
    takes corridor capacity minus the committed load of *every* absorbed
    channel (fragments share the void's tracks — over-stuffing one closes
    all, see the rebuilt `closed_channel_forces_the_detour`); commit
    ordinates, placement preferences, and geometry's seed estimates (seeds
    now carry their travel extent) all use the corridor anchor.
  - **Placement clusters across fragments** (union-find on span proximity +
    corridor connectivity, per world/axis) — without this, corridor-wide
    bounds would let blind per-fragment ladders collide. The cluster-pitch
    shortcut `min(clearance, usable/(n−1))` died: seps start at clearance
    and the **relief valve is the one compression mechanism**. Seps are owed
    only by *contending* neighbours (spans overlapping or within a
    clearance); transitively-chained far-apart items owe 0 — links_medium's
    20-item merged cluster is serializable only under that rule. Known
    approximation, logged for stage 6: an overlapping pair bridged *only* by
    span-disjoint middles can under-separate (the chain model can't express
    pairwise constraints); the validator should watch for it.
  - **Whole-span admission with learned closures**: the search prices edge
    by edge, but a merged run needs one ordinate lawful over its entire
    travel — the corridor intersection, which junction-fed edges overstate
    (the hub row-gaps admitted six wires through a 6 px band; stage 5 drew
    them in mutually-blind fragment clusters, quietly unlawful). After
    `cheapest`, the driver probes the chain run by run; a failing run's
    `(channel, span)` becomes a `Deny` and the same world searches again
    around it (bounded, no-progress-guarded) — so links with lawful
    alternatives find them and only genuine oversubscription strays.
  - Interior bounds fall back to the corridor **walls** when a sliver's soft
    margins cross — the search admitted the run; it draws there.
  - **Results**: pcb_fail — drops at clearance pitch 10 (was 6.13) centred
    by the void midline (was the fragment's), flash exits and the 8-port
    mcu:left ladder at pitch 10 (were 8.87/8.09); pins added to
    tests/routing.rs. pcb — `mcu → rf` ×2 now routes (was 2 honest strays).
    links_hard — 4 strays (was 6): `w2 → s1` ×2 now route; the rest are
    genuine capacity truths at gap 30/clearance 12. Sequence output
    byte-identical. pcb.lini ~17 ms release (corridor walks; budget 50 ms).
- **2026-07-02, stage 5.** Done (straight.rs, dispatch in `routing::route`,
  sequence messages on the spine). Decisions and drift:
  - **Dispatch shape:** `ResolvedLink`/`EdgeReq` carry a typed
    `routing: Strategy` (resolve still strips the attr, so nothing leaks
    into `style=`); `bundles()` admits only orthogonal requests;
    `ortho::route(index, reqs)` skips foreign requests and returns its
    drawn links' request indices; `routing::route` merges the strategies'
    output in declaration order and runs the **shared label pass** — labels
    moved out of the ortho driver into the spine, where ROUTING.md puts
    them.
  - **Sequence seam:** the layout owns *where*, so messages are lowered in
    the sequence's local frame through `straight::wire`/`hook` and stored on
    the container's new `PlacedNode::links`; `routing::owned_links` lifts
    them into scene coordinates (the root-sequence branch collects them
    directly). The renderer's one link path draws them — no `prim::line`
    arrows, no `.lini-sequence-message` labels (the text carries
    `font-size: 13` itself; the rule stays for any future use).
  - **Visual drift** (sequence.lini before/after, 0.2 % of pixels, all on
    the wires): arrow tips gain the standard `MARKER_OVERLAP` half-pixel
    nudge, dash phase starts from the marker-shortened end, and the
    self-hook's return corner radius moved 6.5 → 6.75 (the fillet pass
    caps at half the *drawn* leg, the old line path at half the shortened
    leg). No layout movement; labels, frames, bars identical. Messages now
    render in the link layer — above frame tabs and notes rather than
    below; nothing in the samples overlaps to show it.
  - Sequence "snapshots" are the conformance suite, ignored until stage 7 —
    the before/after PNG diff replaces the re-accept here. Orthogonal
    samples are byte-identical across the refactor.
  - A `routing: straight` pair whose trim leaves nothing (containment,
    coincident bodies) draws nothing, silently — the strategy has no
    report by contract. Revisit in stage 6 if the validator wants a say.
- **2026-07-02, stage 4.** Done (driver in ortho/mod.rs, chain construction +
  polylines in geometry.rs, labels.rs ported, `ortho/order.rs` added,
  tests/routing.rs with 17 contract tests; `routes_str` lives in
  `lini::testing` beside the other hooks, not top-level). pcb.lini routes in
  3 ms release. Decisions and fixes the stage forced — stage 5/6 sessions
  read these first:
  - **Self-loops ride the ordinary search** — no ported `self_loop_chain`.
    `self_loop_sides` (v1's resolution: default right → top, forced side's
    partner stays adjacent, one side = Impossible) turns the loop into a
    forced-sides bundle; wall runs hug the keep-out through ordinary channel
    anchors, ties wrap over the top through ordinary state-id ties. One
    mechanism; the contract test pins the drawn corner shape.
  - **The one-hop nesting key died; `ortho/order.rs` replaces it** — the v1
    outward-walk comparator rebuilt over v2 chains, reading placement
    estimates (ports/anchors) instead of assigned ordinates. The stage-3 log's
    predicted braid materialized immediately (pcb_fail: flash landing *below*
    pwr, 16 crossings). Cluster order is preference → walk → declaration:
    prefs sit inside their boxes so disjoint windows order themselves; the
    walk arbitrates equal preferences (nested, never braided); same-chain
    pieces order by estimate (no self-nesting).
  - **Search holes found by first light, fixed at source, all with the same
    shape — a run the Dijkstra edges never priced:**
    (a) the single-run straight never checked its channel's committed load
    nor that the *shared window holds k* rails at min pitch — `fits` added,
    and `jog_span`'s formula already generalizes to the too-tight-overlap
    case (it yields the shared window); geometry mirrors the same rule;
    (b) a reversal's U-connector was costed but never capacity-checked —
    `u_open` closes it, estimating the connector span from the doubled leg's
    port when it is an end run (links.lini: three side-hooks overcommitted
    one 8.8 px corridor without this).
  - **`usable()` charges soft walls the span faces, not walls it merely comes
    within a clearance of** — proximity charging sealed lawful single-track
    corner passes shut (links_hard's hub pocket). Plus `tracks_left` takes a
    −1e-6 epsilon: an exact-zero usable width is one lawful track, and float
    noise in wall coordinates flipped it closed.
  - **`settle` grew the general relief valve**: any stretch of cluster items
    pinned between two hard boxes compresses *uniformly* to fit (floored at
    min pitch), subsuming per-side and per-window compression. Same-wire
    seps stay 0. pcb_fail's 8-rail cluster (flash's channel floor vs pwr's
    window cap: 54 px for 7 gaps) needs exactly this.
  - The canvas-edge hug is keep-out edge **plus clearance/2** wherever the
    crossing span faces free space past the wall's ends — the soft-boundary
    surrender, pinned in the hug test. Bottom outranks top on side-rank
    ties, so the symmetric over/under detour goes under.
  - The consequences dogleg row holds for **facing sides**; unforced, the
    one-turn L via a third side is cheaper and correct (turns cost length) —
    both pinned as tests.
  - **Strays that remain by design** (v1 drew these by splitting bundles or
    ignoring capacity; v2 must not): pcb `mcu → rf` ×2 (a k=2 bundle through
    a 10 px soft-walled neck holding one track), links_hard `n2 → e1` ×3
    and `w2 → s1` ×2 (no k-track exit exists). The user's lever is `gap`.
  - **Stage-6 work items:** links_hard `hub → north.n1` (k=1) strays because
    the sweep fragments one wide junction void into parallel same-axis
    channels that each charge phantom-neighbour soft margins — capacity
    needs to see the void, not the fragment (or junction-aware merging).
    Fan siblings routing in *different worlds* would not share a port
    (`merge_fans` is per-channel) — theoretical, but the law says one port.
    Min end-segment length is enforced only by the punch stub (as v1);
    a marker run-up exceeding clearance isn't specially held.
- **2026-07-02, stage 1.** Done; deviations from the stage-1 file list:
  - `graph.rs` and `labels.rs` were *not* pre-moved (they'd sit dead until
    their consumers exist). Retrieve from history when their stage lands:
    `git show c70b13f:src/layout/links/graph.rs` (unit tests included, port
    them too) in stage 2, same for `labels.rs` (+ `order.rs`/`path.rs`/
    `runs.rs` as reference) in stages 2–4. v1 `geometry.rs`'s
    `polyline`/`simplify`/`self_loop_chain` likewise — they typed against v1's
    `Chain`; v2's `Chain` differs, so stage 4 rebuilds them from that
    reference rather than inheriting a mismatched shape.
  - Transitional `#[allow(dead_code)]`s are all tagged with a `// Scaffold:`
    comment naming the stage that consumes them — `grep -rn "Scaffold:" src/`
    is the cleanup list (request.rs, scene.rs, rect.rs module-level;
    `Side::ALL` in ast.rs; `ResolvedText.along/attrs` + `Along::Fraction` in
    resolve/ir.rs). Remove each allow when its consumer lands.
  - Ignored tests all carry the literal prefix `routing-v2:` in their ignore
    reason — `grep -rn "routing-v2:" tests/ src/` is the re-enable list
    (conformance glob, hello snapshot, 3 link tests in tests/rendering.rs,
    2 unit tests in src/render/rules.rs). `tests/linking.rs` and
    `tests/linking_sweep.rs` are deleted outright — stage 6 rebuilds the law
    sweep as `tests/laws.rs`.
  - `layout_raw` / `route_sample_raw` died with gap growth (raw is the only
    path now); `lini::testing` keeps `node_rect`, `route_sample`,
    `declared_edges`, `laws`.
  - `layout::ir` became `pub(crate)` so `routing` can name IR types directly;
    `cross()` now lives in `routing::report` (renderer calls
    `crate::routing::cross`).
- **2026-07-02, stage-4 handoff notes** (written at the stage-3/4 boundary —
  tacit knowledge the driver session needs):
  - Where this log contradicts the plan's earlier sections (the vocabulary
    block especially), **the log wins**: it records what stages 1–3 actually
    built. Current true signatures: `entries(graph, body, stub, clearance,
    forced, blockers, inward)`; `cheapest(graph, world, starts, goals,
    ledger, k, clearance) -> Option<Route>`; `Ledger::{commit_run,
    commit_port, tracks_left, side_free, crossings_covering,
    crossings_overlapping}`; `ladder(prefs, bounds, seps)`;
    `place(worlds, chains, clearance)`; types in `ortho/mod.rs`.
  - The driver replaces the stray-stub body of `routing::route()` — keep the
    stub's stray+Impossible construction for links that end up undrawn.
  - Filter each end's entries to sides with `ledger.side_free(path, side,
    rect) ≥ k` before search (a fan group needs 1 slot, not k). Fan side
    fixing: the first-routed sibling's side binds the group — port the
    `fan_pick` idea from v1's driver, reference at
    `git show c70b13f:src/layout/links/mod.rs` (also `world_ladder`,
    `route_self_loop`, and commit ordering). Port patterns, never v1
    mechanisms the spec killed.
  - Route → Chain: merge consecutive same-channel cells into one run (v1
    `geometry::chain` is the reference, `git show c70b13f:src/layout/links/
    geometry.rs`); a repeated cell in `Route::cells` is a U-turn — expand to
    run–jog–run; a single-run route whose windows don't overlap likewise
    expands to end–jog–end (the search already verified the jog's channel
    and priced its two turns; the jog run's channel is the cell's
    perpendicular one). End-run spans start at `EndInfo::side_coord()`.
  - `Ledger::commit_run` per run with provisional spans right after each
    bundle routes; `commit_port` per landing side (fan groups once).
  - After `place()`: corners are adjacent runs' ordinates; polyline = port →
    corners → port; collinear points merge (port v1 `simplify`); first/last
    segments stay ≥ `EdgeReq::stub_*` (marker run-up). `labels.rs` returns
    from history (`git show c70b13f:src/layout/links/labels.rs`) — it reads
    `EdgeReq` + polylines and should port nearly clean.
  - Remove each `// Scaffold:` allow whose code the driver consumes (grep
    `Scaffold:`); re-enable the `routing-v2:`-tagged tests drawn links
    satisfy again (grep `routing-v2:` — the two `src/render/rules.rs` unit
    tests and the three `tests/rendering.rs` link tests belong to stage 4/5;
    conformance + hello snapshots stay ignored until stage 7).
  - Visual dev loop: `lini <sample> --bake-vars -o x.svg && resvg x.svg
    x.png --zoom 2` — without `--bake-vars`, resvg renders var() as black.
- **2026-07-02, stage 3.** Done (ladder.rs, place.rs, pipeline types in
  ortho/mod.rs; 67 unit tests). Notes for stages 4/6:
  - **`ladder` takes per-pair separations**, not one pitch: two pieces of one
    wire owe each other nothing unless their spans overlap (a U's doubled
    legs keep pitch; a Z's jog may collapse to zero and the legs weld).
    `settle()` derives the seps; the same-wire test is member-chain overlap.
  - The first ladder cut (clip the unbounded isotonic fit by bound
    envelopes) was provably suboptimal — a bound activating inside a pooled
    block must re-balance its neighbours. The landed algorithm is bounded
    PAV with clipped block minimizers; the brute-force tests pin the
    difference. Treat `ladder` as sealed math.
  - `place(worlds, chains, clearance)` — the plan's `reqs` param went unused
    (declaration order rides `Chain::link`; bundles need no bookkeeping:
    rails come out adjacent because equal prefs + equal keys fall to
    declaration ties).
  - Nesting comparator is one-hop: sort by (pref, key-to-prev, key-to-next,
    link, run); a key is (arm_n, arm_r, arm_n·arm_r·neighbour_pref) — the
    sign product makes ascending key = ascending ordinate for corners
    turning the same way. If stage 4/6 exposes a braid, deepen the walk
    here, never add a repair pass.
  - Pipeline types live in `ortho/mod.rs`: `Chain{link, world, runs, ends}`,
    `Run{axis, chan, span, ord}`, `EndInfo{side, rect, window, fan}` (+
    `side_coord()`/`centre()`), `World{path, graph}`. Ports are end-run
    ordinates; fan siblings merge to one ladder item (windows intersected).
- **2026-07-02, stage 2.** Done (graph restored, cost.rs, ledger.rs,
  search.rs; 53 unit tests). Design decisions the plan didn't anticipate —
  stage 3/4 sessions read these first:
  - **Dijkstra states are (cell, travel direction) — 4 per cell, not 2.**
    Axis-only turn counting priced a doubling-back U at zero turns (a v1
    blind spot that made backtracking artificially cheap once crossings cost
    money). A reversal now costs its real two corners plus the U-connector's
    crossing estimate. **Stage 4's chain builder must expand a reversal
    (repeated cell in `Route::cells`) into run–jog–run**, same shape as the
    misaligned-window jog; a bundle's U needs 2k concurrent tracks but the
    search checks k per leg — placement's pitch compression absorbs it;
    revisit if a stage-6 sweep catches a violation.
  - **Crossing charges are certainty-based and optimistic.** Committed runs
    are stored as `(span, k, ord)` with `ord = Channel::anchor()` — the same
    anchor placement's interior-run preference uses (midline between
    keep-out walls, hug when one wall is the canvas edge; `graph.rs`).
    `crossings_covering` (rail's span covers the candidate's whole ordinate
    window → unavoidable) charges run travel, edges, jogs; half-open travel
    intervals stop double-charging at piece joints. `crossings_overlapping`
    charges only pinned stubs and U-connectors. Dodgeable rails are *never*
    charged — optimism is deliberate: over-charging is what bred v1's
    orbits; the exact count lands post-placement in the report.
  - Interface drift from the plan's vocabulary block: `entries()` gained
    `clearance` (port windows) and entries carry `dir`; the ledger is
    chain-agnostic (`commit_run`/`commit_port` instead of `commit(&Chain)`)
    — the stage-4 driver iterates a chain's runs itself; `crossings(band)`
    became the covering/overlapping pair.
  - v1's `Channel::capacity/width/capacity_for` died with their tests;
    capacity exists only in the ledger, at min pitch over `usable()`. Soft
    walls shrink pinches more than raw width suggests (a 28px pinch holds 6
    tracks, not 8) — remember when hand-deriving capacities in tests.

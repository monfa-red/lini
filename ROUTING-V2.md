# Routing v2 — Implementation Plan

> **For the executing session:** work happens on branch `routing-v2`. Re-orient
> by reading `ROUTING.md` (the contract — the single source of truth), this
> plan's **Global constraints**, your stage below, and the **Execution log** at
> the bottom. Check off steps as you complete them. When you hit a gotcha, make
> a judgement call the plan didn't anticipate, or leave something for a later
> stage — **append a dated note to the Execution log**. If a rendered result
> looks stale, remember `cargo clean -p lini` (the binary can run stale code).

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
- [ ] **Consequences table, one test per row** (small inline `.lini` sources):
      facing aligned → 2 points; offset-within-windows → still 2 points;
      offset-past-windows → 4 points with mid-run on the gap midline
      (assert ordinate == midline ± ε); bundle ×4 → 4 parallel rails at pitch
      centred on midline; canvas-edge wire hugs keep-outs (ordinate ==
      keep-out wall); crossing beats orbit (assert total length < direct
      distance + margin, crossings reported == expected count)
- [ ] **pcb_fail pins:** all 4 `pwr → mcu` have `turns == 0`; all 4
      `flash → mcu` have `turns == 2` and every point stays left of `mcu`'s
      right edge (no orbit); crossings reported == 0
- [ ] Forced sides honored or stray (`a:left -> b` with `a`'s left walled)
- [ ] Fan trunk: `a -> b & c` siblings share their first point
- [ ] Self-loop right → top; both-ends-one-side errors
- [ ] Containment link lands on the parent's inner side
- [ ] Determinism: `routes_str` twice, assert equal
- [ ] Render `pcb.lini`, `pcb_fail.lini`, `links_hard.lini` to PNG
      (`--bake-vars` + `resvg`) and **look at them** — development
      verification, not CI
- [ ] `cargo fmt && cargo clippy && cargo test`; commit
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
- [ ] `straight.rs` + unit tests (trim math, self-hook shape, marker anchors)
- [ ] Sequence migration; visual check that arrows/hooks/labels look
      unchanged (hook rounding now via the shared fillet pass — log any
      radius drift in the Execution log)
- [ ] `routing: straight` end-to-end test in `tests/routing.rs` (an oblique
      pair renders one trimmed segment)
- [ ] Snapshots re-accepted for sequence; `cargo fmt && cargo clippy &&
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
- [ ] Validator + unit tests (hand-built violations are caught: oblique
      landing, corner graze, sub-pitch hug, unreported crossing)
- [ ] `tests/laws.rs`: every sample routes with zero Warning-severity
      violations
- [ ] **Tightness sweep** (the "right before impossible" test): a facing pair
      with a k-bundle, `gap` swept from roomy down to below `(k−1)·c/2`;
      assert lawful at every step, pitch compresses only when full clearance
      can't fit, and the stray appears exactly at the capacity boundary —
      no silent squeeze, no ugly detour
- [ ] **Side-capacity sweep:** land n wires on one short side, n swept past
      `floor(window/MIN_PITCH)+1` → excess routes to other sides, then
      strays; port order matches wire order at every n (no braid)
- [ ] **Crossing-vs-orbit torture:** the pcb_fail shape with the corridor
      progressively walled until crossing is forced; assert crossings appear
      one at a time (never a wrap), each reported
- [ ] Determinism: full `routes_str` over every sample ×3, byte-equal
- [ ] Perf tripwire: route `pcb.lini` 10× in a test under a generous bound
      (e.g. < 2 s debug total) — catches any audit-style regression
- [ ] `cargo fmt && cargo clippy && cargo test`; commit
      (`test: v2 law oracle + adversarial routing suite`)

---

## Stage 7 — Polish, snapshots, docs

- [ ] Visual pass: render **every** sample `--bake-vars` → PNG, read each one
      (AGENTS.md: don't make the user spot-check). Specifically confirm:
      concentric fillets on turning buses (also at compressed pitch — the
      radii grow by the *actual* track offset), crossing never mid-arc,
      labels legible, `pcb.lini` pretty end to end
- [ ] Re-accept remaining snapshots; remove every stage-1 `#[ignore]`
- [ ] Cosmetics-last pass over `src/routing/` (naming, dead imports, doc
      comments; every file ≤ ~500 LOC)
- [ ] Docs: purge stale references — `PLAN.md` routing/architecture sections
      (point at `ROUTING.md`), `graph.rs`'s "PLAN.md §Architecture" header,
      SPEC.md §9's "only mode built today" line (now orthogonal + straight),
      README if it names routing modes
- [ ] Full gate: `cargo fmt --all -- --check && cargo clippy && cargo test`
- [ ] Final commit; leave pushing/merging to the user
      (`superpowers:finishing-a-development-branch`)

---

## Execution log

Executing sessions: append dated notes here — decisions the plan didn't
anticipate, gotchas, deferred items, comparator cases that needed deepening,
anything the next session must know. Keep entries terse.

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

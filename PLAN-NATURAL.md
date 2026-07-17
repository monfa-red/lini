# PLAN-NATURAL — natural v2: curve-first, no search

Replaces the alpha.1 corridor-first natural per the ROUTING.md contract
(commit `bf1e6b7`, "The natural strategy"). Planned with Abbas 2026-07-17
after the post-release audit: the corridor skeleton showed through as
rounded elbows at ports, the tightener converged to kinked near-polylines
on contended scenes, and every natural link paid the full
channel/search/admission machinery — mindmap.lini spent 2.9 s (debug) in
`search::cheapest`/`tracks_left`, none of it in curves. **ROUTING.md is
the contract; this file is the build order and the log.**

**Goal:** `routing: natural` draws each wire as an independent direct
spline — sides by facing, ports laddered per side before any curve
exists, one G1 cubic chain with bounded via dodges around offending
bodies. No channels, no search, no ledger, no strays. Mindmap routes in
spline-fit time.

**Architecture:** `natural::route(index, reqs)` becomes a sibling
strategy driver beside `ortho::route` (today natural rides ortho steps
1–5). Three decide-once steps: sides → ports → fit-and-dodge. The shared
spine (requests, fans, markers, labels, crossing report, checker
dispatch) is untouched except where it must split per strategy.

**Tech stack:** Rust; `insta` + geometry contract tests (no image reads
in CI); `resvg` PNG eyeballing during development.

## Global constraints

AGENTS.md verbatim where routing work is most likely to violate it: no
`unsafe`; one mechanism per problem, no parallel implementations (the
corridor tightener is *deleted*, never kept beside the dodger); reuse
`ladder()`/`Keepouts`/`self_loop_sides` rather than re-deriving; files ≤
~500 LOC, one concept per file; determinism — `total_cmp` for every
ordering float, no HashMap iteration, no wall clock; `cargo fmt && cargo
clippy && cargo test` before every commit; never "Co-Authored-By";
pushing is Abbas's call.

Constants (ledger/consts.rs, beside `NATURAL_PULL: 0.5`):
`DODGE_ROUNDS: usize = 6`; margin = `clearance / 2` (derive via
`ortho::cost::min_pitch` — the same number, one definition).

## Fixed vocabulary (the contract between stages)

```rust
// src/routing/natural/mod.rs — the driver, mirrors straight::route's seam
pub(crate) fn route(index: &SceneIndex, reqs: &[EdgeReq],
                    routing: &mut Routing, req_of: &mut Vec<usize>);

// src/routing/natural/port.rs — steps 1–2, pure
pub(crate) struct Landing { pub side: Side, pub port: Pt, pub normal: Pt }
/// Sides then ports for every natural request end, decided before any
/// curve exists. `landings[i]` = (end A, end B) of request i (None for
/// non-natural requests).
pub(crate) fn landings(index: &SceneIndex, reqs: &[EdgeReq], fans: &Fans,
                       c: f64) -> Vec<Option<[Landing; 2]>>;

// src/routing/natural/curve.rs — the direct fit (kept: bezier, span,
// spans, sample_span, sample; deleted: fit, stubs — polyline-fed)
pub(crate) fn direct(pa: Pt, na: Pt, sa: f64, pb: Pt, nb: Pt, sb: f64,
                     vias: &[Pt]) -> (Vec<Pt>, Vec<[Pt; 4]>);

// src/routing/natural/dodge.rs — step 3's obstacle half.
// Keepouts moves here from corridor.rs unchanged in shape, judged at
// `margin`, and stays shared with the checker (one construction, one
// metric). corridor.rs dies whole.
pub(crate) fn dodge(base: (Vec<Pt>, Vec<[Pt; 4]>), keep: &Keepouts,
                    fit: impl Fn(&[Pt]) -> (Vec<Pt>, Vec<[Pt; 4]>))
                    -> ((Vec<Pt>, Vec<[Pt; 4]>), Vec<(Rect, f64)>);
```

## Decisions ledger (settled in design review — do not relitigate)

1. **Curve-first, approach A.** Direct spline + local via dodges; the
   tangent-visibility graph (B) was rejected — a second search fighting
   orthogonal's job. Guarantee-needing scenes route `orthogonal`.
2. **Natural never strays.** A wire that cannot dodge draws anyway and
   the wire-body pair is reported (`Rule::Clearance`, Warning);
   `--strict` promotes it. Wire-wire: no law — crossings free at any
   angle, counted as today; only duplicates (port-ladder pitch) and fan
   trunks (shared landing) bind wires.
3. **Ports before curves.** Per (node, side): preference = far end's
   centre projected onto the side (a fan landing: the mean of its
   members' projections), clamped into the port window (side minus
   `clearance` corner margins — the existing entry.rs shape, centre
   point when the side is too short); spread by `ladder()` at
   pitch `min(clearance, usable/(n−1))`, order by preference then
   declaration. Wires then fit independently — no committed order.
4. **Sides by facing.** Forced side wins (trees/mindmaps stamp theirs;
   self-loops via the shared `self_loop_sides`, right → top). Otherwise
   the permitted side with the greatest `dot(outward normal, chord)`;
   a containment end scores with the inward normal and lands on the
   parent's inner side. Ties: side rank.
5. **Via rule.** Sample the fitted curve against solids inflated by
   margin (Keepouts::offence, end-span excuses as today). The first
   offending body inserts one via: the corner of the margin-inflated
   body with the smallest |perpendicular distance to the chord|, tie by
   (x, y); vias keep chord-projection order as knots. Refit, re-judge,
   ≤ `DODGE_ROUNDS` rounds; leftovers draw + report (decision 2).
6. **Bundles need no machinery in natural.** Duplicate members are
   separate landings; the port ladder spreads them at pitch on both
   sides, so their curves are translates — "parallel rails riding one
   shape" for free. `request::bundles`/ledger stay orthogonal-only.
7. **The crossing report moves to the spine.** The pairwise count in
   `ortho::route` relocates to `routing::route` over the merged drawn
   links (strategy ≠ Straight), reading names/spans off `RoutedLink` —
   natural×orthogonal pairs keep counting when the strategies split.
8. **Checker arm follows the laws**: contact (shared) + stub law;
   G1 knot smoothness (adjacent tangents parallel within ε);
   directness — where both drawn stub directions advance along the
   chord, path progress is monotone (small slack for spline
   overshoot); respect at margin, a reported pair excused; duplicate
   separation as today. The sampled-clearance-at-`c` law dies with the
   corridor.

## Stages

### Stage 1 — the plan + contract touch-up

- [ ] Commit this file.
- [ ] ROUTING.md micro-edits found while planning: port spread
  "compressing toward margin only when the window is short" →
  "compressing evenly when the window cannot hold them" (short windows
  have no floor — natural never strays); note the crossing report is
  spine-owned. Commit with Stage 4's doc pass if trivial.

### Stage 2 — engine swap (one commit; the tree must stay green)

- [ ] `natural/port.rs`: `Landing`, `landings()` — side scoring,
  windows (port from the entry.rs window shape, no graph, no blocker
  clipping), per-side ladder spread; self-loop sides via
  `self_loop_sides` (make it `pub(crate)`, move to `natural/port.rs`?
  no — it stays in ortho/mod.rs, natural imports it; one definition).
  Unit tests: facing pair lands on facing sides at aligned centres →
  dead straight; far-end pull lands ports off-centre within windows;
  two duplicates ladder at pitch on both sides; fan landing at the
  members' mean; forced side wins; short side collapses to centre;
  containment lands inside; determinism (rerun equality).
- [ ] `natural/curve.rs`: add `direct()` (tips = port + normal·stub,
  knots = tips + vias, `spans` with forced end tangents, `sample`);
  delete `fit`/`stubs` and their polyline tests; port the aligned-pair
  and S-curve tests onto `direct` (the classic S must come out
  byte-identical in shape: horizontal tangents, symmetric midpoint).
- [ ] `natural/dodge.rs`: Keepouts moved from corridor.rs (offence
  unchanged); `dodge()` — the via loop of decision 5, returning the
  final geometry plus the unresolved `(body, dist)` offences.
  corridor.rs deleted. Unit tests: clean fit passes through untouched;
  a straddling body forces one via and clears at margin; an
  undodgeable wall reports and still draws; determinism.
- [ ] `natural/mod.rs`: the driver — filter Natural requests, c = their
  max clearance, margin = min_pitch(c); fans via `fan_groups` gaining a
  strategy predicate (ortho passes Orthogonal, natural Natural);
  `landings()`; per request: knots → `direct` → `dodge` → RoutedLink
  (path/curve/markers/attrs as today), offences → Rule::Clearance
  Warnings; self-loop hook (tips + one out-corner via, no dodging).
- [ ] The split: `EdgeReq::corridor()` dies; `ortho::route` filters
  `Strategy::Orthogonal` (its Natural lowering arm and the
  `natural::lower` seam die); `bundles()` keeps its
  orthogonal-only filter explicitly; `routing::route` calls
  `natural::route` beside `straight::route`; the crossing count moves
  to `routing::route` per decision 7.
- [ ] `validate/natural.rs`: rework per decision 8; `validate.rs::check`
  threads `report` into the natural arm for the respect excuse.
- [ ] tests/routing.rs natural block: obstacle test now expects a
  margin-clearing dodge (or a report — assert one of the two, and that
  the wire drew); bundle test asserts port pitch on both sides; fan
  trunk, self-hook, oblique crossing, rerun determinism, anon-world
  trees — updated expectations, same coverage. Perf tripwire: 10 debug
  compiles of samples/mindmap.lini < 10 s.
- [ ] `cargo fmt && cargo clippy && cargo test`; commit
  (`routing: natural v2 — direct splines, via dodges, no search`).

### Stage 3 — visual + snapshots + samples

- [ ] Render tree.lini (unchanged — orthogonal), mindmap.lini, and
  natural-forced links_medium/links_hard scratch copies to PNG, light +
  dark, and **read them**: mindmap S-curves smooth at every port (no
  rounded elbows), fans centred, links_medium crossing-happy but
  smooth, no kinks anywhere.
- [ ] Re-bless the mindmap snapshot (ports move off the old placement);
  regenerate README's mindmap asset if the hero drifted visibly.
- [ ] Timing sanity: mindmap.lini debug ≪ 0.5 s, release ≪ 20 ms;
  laws sweep green.
- [ ] Commit (`mindmap rides natural v2; snapshots re-blessed`).

### Stage 4 — docs & close-out

- [ ] ROUTING-LOG.md gains the v2 entry (what died, why, the numbers);
  ROUTING.md wording synced with what was actually built (constants'
  names, the spine-owned crossing note); Stage 1's micro-edits if
  deferred.
- [ ] Full gate: `cargo fmt --all -- --check && cargo clippy &&
  cargo test`; commit. Pushing stays with Abbas.

## Execution log

Executing sessions: append dated notes — decisions the plan didn't
anticipate, gotchas, the next session's starting points. Terse.

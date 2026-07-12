# TREE-alpha1 — tree, mindmap & natural routing

The first post-freeze round (`1.0.0-alpha.1`), planned with Abbas on
2026-07-11 against the shipped `1.0.0-alpha.0`. Sources: ROADMAP 3.2/3.3
(the settled decisions), PLAN-V1's alpha.1 contract, and the design
brainstorm logged in the ledger below. **Stage 0 amends SPEC and
ROUTING.md; the amended spec is the contract**; this file holds the settled
decisions, the build order, and the execution log. The quality bar is
ROADMAP §5, verbatim. Everything is **additive** — the syntax is frozen; no
existing sample or snapshot changes except where a stage names it.

Scope: `layout: tree` (`direction: row | column | bilateral`), `|topic|`,
desugar-generated branch links, `|mindmap|` with the palette walk and
depth classes; `routing: natural`. Plus two rider items: the README
accuracy pass (the crates.io face still says v0.20) and the zero-size rect
emission fix (palette.lini emits a degenerate `<rect width="0"/>`).

Stages are sized for one session at ~60 % of a context window; sub-agents
per AGENTS.md (always with an explicit `model`). At each stage's end:
fmt/test/clippy clean, a **Log** line here.

---

## Decisions ledger (settled in design review — do not relitigate)

1. **`direction: bilateral`, not `radial`.** The mindmap arrangement is
   what every tool actually draws: root centred, first-level topics fanned
   **right and left**, subtrees growing horizontally outward — not a true
   ring. `radial` already means truly-circular on a chart (`direction:
   radial` bends the plane into a radar), and one word must not mean two
   geometries — so the tree value is `bilateral`, and `radial` stays
   unclaimed for a possible true ring-radial tree post-1.0. ROADMAP 3.2's
   "radial = ring distance × tangential minimum" wording is superseded;
   Stage 0 syncs it. Left/right only — a vertical bilateral is what
   `column` is for; no syntax reserved.
2. **The split rule**: the first ⌈n/2⌉ first-level topics fill the right
   side top-to-bottom in declaration order, the rest the left — dead
   predictable ("source order = sibling order" is already the tree law).
   A per-topic **`side: left | right`** overrides (lini's
   customization style); `top`/`bottom` in a bilateral tree errors, any
   `side:` on a topic in `row`/`column` errors (one growth direction).
3. **Boxed all the way down.** Every topic renders as a box at every
   depth — no text-on-branch mode (the wash tint needs a box to fill).
4. **`.lini-level-N` generated classes.** Every topic wears its depth
   class (root = 0), the one hook both the `|mindmap|` ramp and user
   tier-restyling ride — no depth selectors invented.
5. **The palette walk tints branch links only.** An authored cross-link
   between branches keeps the neutral link default (tinting would pick one
   of two subtrees' hues arbitrarily); the author styles it explicitly.
   Walk order: the `HUES` table order, **red and grey skipped** (red reads
   as danger, grey as neutral chrome), wrapping past nine.
6. **Topic wrap**: `|mindmap|` presets `max-width: 160` on topics (final
   number confirmed by eye in the visual pass); plain `layout: tree` sets
   no cap.
7. **Mindmap branches are horizontal-tangent S-curves** — the de-facto
   standard (XMind, MindMeister, markmap, FigJam): leave the parent
   sideways, arrive at the child sideways, a cubic absorbs the vertical
   offset. Constant width — branches are ordinary `|-|` wires; no tapered
   organic strokes, no tension/curvature knobs.
8. **`natural` = corridor first, curve second.** Reuse the orthogonal
   search end-to-end for corridor choice (channel graph, worlds, admission,
   Law-3 cost), then fit a smooth curve inside the corridor instead of
   placing polylines — never a rounded illegal straight line. Tangent-normal
   endpoints, keep-out clearance, honest strays; the laws hold where
   geometrically applicable. Curve character outside the mindmap is settled
   in the Stage 3 prototype against rendered PNGs, not by discussion.
   `curved` is **removed, not aliased** — `routing: curved` errors with a
   did-you-mean toward `natural`.
9. **Branch links are desugar-generated** (AUDIT D2): ordinary unmarked
   `|-|` links resolving in the parent topic's scope — `#syntax |-| { }`
   styles one arm, `lini desugar` shows them, the router routes them.
   Authored cross-links are legal and never alter the tree.
10. **One root topic.** A direct `|topic|`-derived child is a branch;
    every other child is the topic's own content. `|topic|` outside a tree
    errors; two roots (a forest) errors — relaxing later is non-breaking.
11. **Plain trees stay neutral and orthogonal.** `layout: tree` without
    the preset reads as an org chart: uniform boxes, monochrome, elbow
    connectors (the scope's default `routing: orthogonal`).
12. **This round only.** alpha.2+ explode into their own docs at entry
    (the PLAN-V1 pattern); nothing here reserves syntax for them.
13. **Classes on text** (rider, added 2026-07-12): a string may wear a
    class chain — `"Starter" .card-title` — the node tail on a string
    head. No new mechanism: the worn-class cascade tier applies to the
    text leaf, text-valid properties land, anything else is **inert**
    per the class-polymorphism law (M2's lenient clause). Additive
    grammar, so the freeze permits it. Kills the `|block|`-wrapper-only-
    for-styling pattern, which also restores a parent's `max-width` wrap
    over those strings (a box child stays opaque to the cap — that hard
    error is the honest contract and does not change).

---

## Stages

### Stage 0 — SPEC & ROUTING.md amendments (the contract)

Write all round law before any code, per the Stage-0 pattern. The SPEC
alone must suffice to implement Stages 1–5.

- [x] New SPEC Part II section **Tree** (between 12 Flow & Grid and 13
  Sequence, renumbering or as 12b — pick what keeps anchors stable): the
  engine (rooted structure from `|topic|` nesting, source order = sibling
  order, parent-over-subtree centring), `direction: row | column |
  bilateral` with the split rule and `side:` overrides (decision 2), `gap`
  as generation distance × sibling separation (transposed by direction),
  the single-root and topic-outside-tree errors, branch-link generation and
  scope (decision 9), `.lini-level-N` classes, wiring-strategy row (tree =
  router-routed, like flow/grid).
- [x] SPEC 8 template rows: `|topic|` (base `|block|`; the structural
  type; custom structural types derive from it — `|person::topic|`) and
  `|mindmap|` (the preset: visible root topic, `layout: tree; direction:
  bilateral; routing: natural`, the palette walk, the depth ramp,
  `max-width: 160` on topics).
- [x] SPEC 9/11: `natural` joins the strategy row (`orthogonal` default /
  `natural` / `straight`); the layout-model table gains the tree row;
  SPEC 16 ledger rows (`layout: tree`, `direction: bilateral`, `side` on
  topics, `routing: natural`); SPEC 20 error rows (topic outside tree,
  forest, `side:` misuse per direction, `routing: curved` removal message);
  SPEC 23: the `curved` deferral comes out (replaced by `natural`, built).
- [x] ROUTING.md: replace the `curved` row with `natural`'s contract
  (decision 8 — corridor via the shared search, curve fit, which laws bind
  a curved wire and how: tangent-normal contact, clearance, separated
  duplicates, honest strays; crossings need not be square-on), refresh the
  implementation-shape map (`routing/natural/{mod,corridor,curve}.rs`),
  tighten the "validation is the orthogonal contract's alone" claim to
  name what the natural checker judges instead.
- [x] ROADMAP sync: 3.2/3.3 wording updated to `bilateral` (+ a line
  noting the rename and why); the ladder row for alpha.1 confirmed.
- [x] **README accuracy pass** (its own commit; the crates.io face):
  Status section rewritten for 1.0.0-alpha.0 (not "v0.20"; drawings are
  through sections/sheets/details, not "work in progress"), the sequence
  pointer fixed to §13, the entities section's `layout: auto` promise
  dropped (ROADMAP 6 rejects the name — say "an automatic graph layout is
  on the roadmap" without naming syntax), performance/architecture claims
  spot-checked. The mindmap feature bullet waits for Stage 5.

Acceptance: SPEC/ROUTING.md alone sufficient to implement Stages 1–5;
anchors and cross-references intact (the S1 anchor-script check); every
example in the new text uses shipped syntax; `cargo test` untouched.
**Log:** 2026-07-11 — **done**, 4 commits (SPEC, ROUTING, ROADMAP/PLAN-V1,
README), all acceptance met (anchor script: zero broken anchors; tests
untouched — the one conformance failure during the stage was Abbas's own
in-flight `cards.lini` restyle, staged in his editor, left alone). Decisions
executed in place: the Tree section lives **inside SPEC 12** — retitled
"Flow, Grid & Tree" — so 13/14/15 keep their numbers (code comments and
error messages cite them); all 13 `#12-flow--grid` anchors retargeted.
`|topic|` defaults set at `padding: 8 14` (a compact card; Stage 3's visual
pass may retune — flagged, not silent). SPEC 20's routing row now states the
**new** `curved`-replacement message while `resolve/links.rs` still emits the
old deferral text — reconciled in Stage 3 when `Strategy::Natural` lands (the
S2 precedent: SPEC leads code inside a round). SPEC 23's beyond-1.0 tail
gains ring-radial + forest trees so the bilateral rename reserves nothing.

### Stage 1 — the tree engine: `|topic|`, row/column, branch links

The structural half — no bilateral, no curves, no preset yet. An org
chart must come out whole.

- [x] Ledger rows: `layout` accepts `tree`; `topic`/`mindmap` template
  names; `side` gains its topic owner (it already exists for endpoints);
  validation wiring so M2's owner-aware pass covers the new rows.
- [x] `|topic|` template bundle (over `|block|` — framed like a card:
  reuse `|box|`'s paint tier as the topic default so a bare topic reads);
  resolve-side structure checks: `|topic|` (or a topic-derived type)
  outside a `layout: tree` scope errors; a tree scope with zero or ≥ 2
  root topics errors (SPEC 20 wording).
- [x] Desugar branch links (D2, beside `classes.rs`'s generated-rule
  precedent): for each topic, one unmarked `|-|` link parent → child per
  topic-derived child, generated **in the parent topic's scope** (sealed-
  body law holds; `lini desugar` shows them; re-desugar is a fixed point —
  the scoped-note-rules pattern). Depth computed in the same walk wears
  `.lini-level-N` on every topic.
- [x] `layout/tree.rs` — the engine: `is_tree(attrs)` beside the other
  predicates in `layout_inst`'s dispatch; **flow/grid stay untouched**.
  Tree is router-routed (arranges in place, hands links to the router —
  the flow/grid row of the SPEC 11 table, not a lowering engine).
  Placement: post-order — each subtree packs its children (sibling
  separation = cross-axis `gap`), parent centred over (row: left of /
  column: above) its subtree span at generation distance (main-axis
  `gap`). Reuse `flex`'s measurement/positioning helpers where they fit
  (D3's "flex is a pure reusable positioner"); introduce the engine trait
  only if the dispatch genuinely wants it — the existing predicate
  dispatch is the house shape, and a trait that only tree implements is
  a parallel mechanism.
- [x] Orthogonal routing over trees just works (branch links are ordinary
  requests; forced sides by direction: column = parent `bottom` → child
  `top`, row = parent `right` → child `left` — stamped on the generated
  links at desugar so the router needs no tree knowledge).
- [x] Sample: new `samples/tree.lini` — an org chart (column, orthogonal)
  and a row tree in one scene (the cluster policy: one file for the plain-
  tree cluster). Snapshots (conformance + desugar oracle showing branch
  links); laws sweep green over the routed tree.

Acceptance: org chart renders correctly light + dark (PNG eyeballed);
`lini desugar samples/tree.lini` shows every branch link in its scope;
the structure errors fire with SPEC 20's wording; zero diffs outside the
new sample.
**Log:** 2026-07-12 — **done**, 6 commits over two passes plus an owner
SPEC-errata commit; fmt/test/clippy clean throughout; org chart + row
tree + anonymous tree eyeballed light + dark; every non-tree snapshot
byte-identical. The first pass (`20fcf0d`/`e9ded02`, Opus) FLATTENED
topic nesting at desugar to dodge the router's containment case — owner
verification caught the price (anonymous topics silently lost their
wires; scoped ids collided across arms; `#id |-|` arm styling matched
nothing) and Abbas rejected it. The second pass (**Option B**,
`ae03ef0`/`d01dd54`/`e5ee3a2`/`bb975fd`) keeps nesting through the whole
front half and fixes the failure modes at their sources:
- Router: the containment special case gates on **geometry, not path
  prefix** — new `SceneIndex::geo_contains` (prefix AND placed-rect
  enclosure) feeds `world_ladder`, the inward port flips, and the
  validator's containment skip; a mere path-descendant climbs the world
  ladder like any wire.
- Desugar: anonymous topics mint deterministic per-scope
  `lini-topic-N` ids (authored `lini-*` ids now error — the reservation
  extended to ids); one unmarked **fan per parent** in the scope
  containing it, dotted endpoints, forced sides per direction;
  byte-idempotent including root trees (`Child::span()` unified; the
  root fan's span seats past the instances).
- Resolve: the **containment-cascade law** — a link into a node's own
  descendant cascades as if written in that node — so `#cto |-| { }`
  styles exactly cto's arm (pinned non-vacuously in tests/rendering.rs
  after the first arm test proved vacuous); inheritance (clearance /
  routing) deliberately keeps the written scope's channel.
- Engine: nested-input placement (generations / sibling packing /
  centring), each card sized from non-topic content only, placed nested
  with overhang (a topic's keep-out is its own card); root
  `{ layout: tree }` scenes lay out (new `layout_root` arm).
Deviations adopted into SPEC as errata (`8d7e0b3`): fan-per-parent,
minted ids + id-prefix reservation, tree default `gap: 64 48`, the
SPEC 4 cascade-law sentence. Sample note: the org chart's cross-link
moved inside `coo`'s body (`ops --- qa`) — the tree-scope spelling had
relied on flat ids; SPEC 9's no-search law stands.

### Stage 2 — bilateral

- [x] Bilateral placement in `layout/tree.rs`: the split rule (first
  ⌈n/2⌉ right, rest left, declaration order both sides), `side:` override
  read per first-level topic; each half is the row-tree layout mirrored;
  root centred between the halves' spans; `gap` semantics unchanged
  (generation = horizontal, sibling = vertical).
- [x] Branch-link sides for bilateral: right-half links parent `right` →
  child `left`, mirrored on the left half (root emits from both sides).
- [x] `side:` validation per decision 2 (top/bottom in bilateral errors;
  any `side:` on a topic in row/column errors), SPEC 20 rows.
- [x] Extend `samples/tree.lini` with a small bilateral tree (still
  orthogonal routing — proves bilateral is independent of `natural`).

Acceptance: bilateral sample balanced and readable light + dark; `side:`
override demonstrably moves a branch; error rows fire; laws sweep green.
**Log:** 2026-07-12 — **done**, 1 commit (`ef6afe0`, Opus agent; owner
re-verified: probes, gates, PNGs light + dark). The split rule +
mirrored halves land in `desugar/tree.rs` (`build_bilateral` — first
⌈n/2⌉ right, rest left; inline `side:` read and **consumed**; each half
re-expressed as a generated `.lini-side-{left,right}` class) and
`layout/tree.rs` (`place_bilateral` reuses `assign` with a sign-flipped
band array — a mirror, not a reimplementation; root centred between the
halves). The root emits one fan per non-empty half with mirrored forced
sides. Both SPEC 20 `side:` errors fire verbatim; a deeper bilateral
`side:` reuses the one-growth-direction message (judgment call, in
SPEC's spirit — first-level `left|right` is the only legal form).
`direction` validation is per-engine (no central table): `bilateral`
off a tree errors in each engine's reader exactly as `radial` off a
chart does. tree.lini gains the 5-branch bilateral scene with a
`side: left` override; only its snapshot changed. **Noted edge**: a
rule-borne `side:` (`#a { side: right }`) is inert — the split reads
the authored inline decl only, the M3 scale-fold precedent; SPEC's
examples are inline.

### Stage 3 — `natural`: the strategy + the curve prototype

The round's real risk is curve aesthetics — this stage exists to iterate
them against rendered PNGs *before* the general engine work, on the
geometry where no avoidance is needed (a laid-out tree guarantees
parent/child free sight-lines).

- [x] `Strategy::Natural` variant — the D4 exhaustive matches flag every
  touch site; resolve accepts `routing: natural`, the `curved` message
  becomes the removal error with did-you-mean (decision 8); the
  `request.rs` bundle filter widens per the PLAN-V1 seam (natural
  requests bundle like orthogonal ones).
- [x] `routing/natural/{mod,curve}.rs` — the direct case first: when the
  straight corridor between the endpoint sides is free (the tree/mindmap
  case), emit the horizontal-tangent cubic S-curve (decision 7): ends
  perpendicular to their sides, marker run-up straight for at least the
  marker, labels riding the curve at `along:` fractions (arc-length
  parameterised), duplicates as offset parallels at pitch.
- [x] Render: the link path emitter takes cubic segments (today it takes
  polylines + fillets — the seam is the one `d`-builder); markers/label
  masks/strays unchanged.
- [x] **The aesthetic prototype loop**: a throwaway mindmap scene (hand-
  tinted, no preset yet) rendered to PNG light + dark; iterate curvature
  (control-point pull), sibling fan spread at the parent port, and the
  160 wrap cap **with Abbas** until the hero look is agreed. The agreed
  constants land in `ledger/consts.rs`; the scene becomes the skeleton of
  Stage 5's hero sample.

Acceptance: a tree with `routing: natural` draws clean S-curves (PNG
eyeballed light + dark, multiple fan-outs); orthogonal output everywhere
byte-identical; the curve constants named in one place.
**Log:** 2026-07-12 — **done**, 1 commit (`114fbd7`, Fable agent; owner
re-verified: suite, clippy, PNGs, prototype loop). Architecture decided
up front: natural is NOT a separate router — a `Strategy::Natural`
request rides `ortho::route` steps 1–5 unchanged (worlds, channels,
bundles, fans, search, admission, placement — the corridor choice is the
shared search by construction) and diverges only at geometry lowering,
where `routing/natural/{mod,curve}.rs` fits stub-anchored G1 cubics
(Catmull-Rom-blended tangents; the single-jog chain collapses to the one
classic horizontal-tangent S). `RoutedLink` gains `curve: Vec<Cubic>`
while `path` holds a dense sampling (24/cubic, exact stub points), so the
whole spine — markers, label arc-walk, masks, crossing count, validator —
reads true drawn geometry unchanged; the render seam is one `cubic_d`
builder (exact `C`s between marker-shortened stub `L`s). Labels'
`arc_len`/`at_arc` went Euclidean (identical on axis-aligned wires — zero
snapshot diffs; true arc-length on curves). The strategy-filter cleanup
folded three parallel copies into one `EdgeReq::corridor()` predicate.
`routing: curved` now errors with SPEC 20's exact replacement message.
Constants: `NATURAL_PULL: 0.5` in `ledger/consts.rs`. **Deviation**: the
aesthetic loop ran solo (autonomous session) — a 6-branch hand-tinted
bilateral prototype rendered light + dark; a 0.35/0.5/0.7 pull sweep is
near-indistinguishable (the forward-travel clamp dominates long spans;
short doglegs barely differ), so 0.5 stands; the 160 wrap cap reads
balanced at 2–3 lines; the fan crow's-foot (shared trunk port) is the
right mindmap join. Constants await Abbas's veto in the Stage 5 visual
pass; the prototype's shape (root + 6 tinted branches with `.hue` classes
+ per-arm `#id |-| { stroke: --hue-deep }` rules) is the Stage 5 hero
skeleton. **Noted for Stage 5**: an arm rule tints a branch's subtree
wires but not the root→branch wire (it lives in the root topic's scope) —
the palette walk's generated rules must tint that arm explicitly.

### Stage 4 — `natural` general: corridors, obstacles, laws

- [x] `routing/natural/corridor.rs`: reuse the orthogonal search end-to-
  end (`build_worlds`, channels, admission, Law-3 cost) to pick the
  corridor; then fit the curve inside it — spline through the corridor's
  cell sequence honouring keep-out clearance (sampled), tangent-normal at
  both ends, never tighter than the corridor allows. A link the search
  cannot route is the same honest stray.
- [x] The shared spine holds: forced sides, markers, labels (slide along
  the drawn curve), bundles (parallel offset curves), fans (shared trunk
  until the split), self-links (a smooth hook), reports, determinism
  (byte-identical reruns pinned).
- [x] The natural law checker (`routing/validate` gains a natural arm,
  per the ROUTING.md Stage-0 wording): endpoint contact + perpendicularity,
  sampled clearance from keep-outs, duplicate separation; the orthogonal-
  only laws (square-on crossings) explicitly skipped.
- [x] Flow/grid scenes accept `routing: natural` (tests: a dogleg-forcing
  obstacle scene, a bundle, a fan, a self-link); tree/mindmap unaffected.

Acceptance: natural obstacle scenes lawful under the new checker;
deterministic reruns byte-identical; orthogonal and straight outputs
untouched; a stray still draws honest.
**Log:** 2026-07-12 — **done**, 1 commit (Fable agent). Corridor
tightening (`natural/corridor.rs`): a clean fit passes through untouched
(the Stage-3 trees byte-identical); on a sampled clearance breach the
spline re-anchors through the polyline's corners, then only the offending
spans' handles halve toward their chords, a final round snapping them —
a zero-handle span *is* its polyline piece, legal by construction. The
checker gains its natural arm (`validate/natural.rs`): the shared landing
judgment plus the straight marker stub, sampled clearance through the
router's own `Keepouts` offence predicate (one mechanism, one metric —
`box_dist` now lives in `ortho/rect.rs`), duplicate pitch-floor
separation with the fan-trunk excuse; run/track and square-crossing laws
explicitly skipped. Crossing counts widen to generic segment intersection
for pairs involving a natural wire (`cross_oblique`; orthogonal pairs keep
`cross`, reports byte-identical), reconciled by the checker the same way;
testkit `declared_edges`/`drawn_edges` count natural. Six routing tests —
obstacle dodge (plus a clearance mini-sweep), bundle rails, exact fan
trunk, smooth self-hook, oblique crossing counted+reconciled, byte-
identical reruns — plus corridor/checker unit tests; obstacle, bundle,
fan, and self-link PNGs eyeballed.

### Stage 5 — `|mindmap|`, the hero & release

- [ ] `|mindmap|` bundle (the chart-preset precedent: the layout preset is
  the whole bundle): `layout: tree; direction: bilateral; routing:
  natural`, visible root topic; the depth ramp as generated `.lini-level-N`
  rules (root large/semibold, level-1 medium, level-2+ small — by eye in
  the visual pass); `max-width: 160` on topics.
- [ ] The palette walk at desugar (decision 5): next `HUES` entry per
  first-level branch in declaration order (red and grey skipped, wrap past
  nine), lowered as generated rules tinting the subtree — `wash` fill,
  `deep` stroke **and branch wires**, `ink` text; the root stays neutral;
  explicit author paint wins (the generated rules sit below the authored
  tiers); cross-links stay neutral. Deterministic; `lini desugar` shows
  the rules; dark mode free via the tiers.
- [ ] Sample: new `samples/mindmap.lini` — the README-worthy hero (rich
  topics with icons/badges, one authored cross-link, wrapped labels),
  grown from the Stage 3 prototype.
- [ ] **Zero-size rect fix** (the rider): an empty box emits no degenerate
  `<rect width="0" height="0"/>` (palette.lini line 28 today) — guard at
  the rect emitter; re-bless the one snapshot.
- [ ] README: the mindmap bullet + hero image (`assets/`), tree/mindmap in
  the tour; docs cross-check (SPEC 24 example if the family warrants one).
- [ ] Release sweep (the M7 shape): fmt/test/clippy; `lini fmt --check`
  over samples; desugar + laws oracles; every **new/changed** sample
  rendered light + dark via `--static` and eyeballed; version
  `1.0.0-alpha.1`, publish + tag (push deferred to Abbas); PLAN-V1's
  alpha.1 section and ROADMAP's ladder row marked done; retro line here.

Acceptance: the mindmap hero reads well light + dark (palette walk
verified); `cat -> dog` diagrams unchanged; all suites green; crates.io
shows `1.0.0-alpha.1`.
**Log:**

### Stage 6 — rider: classes on text (decision 13)

Independent of the tree work; may land any time before the Stage 5
release sweep (whose version bump then carries it).

- [x] SPEC amendment first, tight: SPEC 3 (Text content — a string takes
  a class chain like any node tail; text-valid properties apply, others
  inert per the class law), SPEC 4 (worn classes reach text leaves,
  tier 3), SPEC 21 grammar (`text_stmt = string [ classes ] [ block ]`),
  SPEC 16 note if any, SPEC 20 unchanged (no new errors — inertness is
  the law).
- [x] Parser: the string statement head accepts the worn-class chain
  (spaced off the string, glued within — the node-tail rule); fmt prints
  it canonically; desugar carries classes on text leaves (user classes
  emit as `.lini-style-*` on the `<text>` beside `.lini-text`).
- [x] Resolve: text leaves walk the class tier of the cascade (today:
  inline block + inheritance only); the text-valid filter is the same
  one the inline block uses — worn-class non-text props are inert, not
  errors.
- [x] Tests: parser/fmt round-trip, cascade (class vs inline precedence,
  inert non-text prop), render snapshot (class hook on `<text>`),
  validation (class-dead-on-every-wearer warning still correct when the
  only wearer is text and the prop is text-valid).
- [x] `cards.lini` cleanup: the `|block|` wrappers drop — titles/briefs
  become bare classed strings under the cards' `max-width` (re-bless +
  eyeball light/dark; a long title now wraps instead of erroring).

Acceptance: `"Starter" .card-title` styles the text; every existing
sample byte-identical except cards; `lini fmt` round-trips the new form;
a worn class's box-only property is silently inert on text, exactly as
on any non-wearing node.
**Log:** 2026-07-12 — **done**, 3 commits in an isolated worktree
(`e69371d`/`4d08a31`/`2a36f48`, Opus agent) merged as `8d987e2`; SPEC
amendment (`e929675`) written first, inline by the owner. `TextNode`
gained `classes`; content-position strings (statements, `[ ]` children,
link `[ ]` labels — one shared `parse_text_node`) take the tail;
head-label classes stay owner-bound structurally (`parse_tail` fills
the owner's chain — no grammar ambiguity). Resolve inserts tier 3 via
one shared `apply_text_classes` (factored `user_class_decls` out of
`node_layers` — node and text read one source); classes merge before
layout, so class-borne `font-size` measures. Render: `lini-style-*`
beside `lini-text`; validation counts text wearers. fmt: a classed cell
breaks its aligned row via the shared `is_plain_text`, the styled-cell
rule. cards.lini drops its wrappers — bare classed strings under the
cards' `max-width`; a long title now wraps instead of erroring; only
cards' snapshot changed (byte-identity everywhere else). **Deviation
adopted**: link `[ ]` labels wired too (`ResolvedText.applied_styles`
threaded to `render_link_text`) — SPEC 3 lists them as content
position, and parsed-but-unwired would drop a class silently. Merge
note: one conflict in `resolve/links.rs` (Stage 1's `resolve_ladder`
closure × the label-class insertion) — resolved by grafting
`apply_text_classes` into the closure's label loop; 886 tests green
post-merge; classed text inside a bilateral tree smoke-verified.

---

## Execution log

Executing sessions: append dated notes here — decisions the plan didn't
anticipate, gotchas, deferred items, anything the next session must know.
Keep entries terse.

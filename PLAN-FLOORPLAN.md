# PLAN — Floorplan (the architectural dialect)

Architectural floor plans as a native lini family: `layout: floorplan` riding
the **drawing engine** (SPEC 15) unchanged, plus a scope-gated vocabulary —
`|wall|` (centreline + `thickness:` → offset poché outline), `|door|` /
`|window|` openings stationed on wall segments, six symbol-bodied fixtures,
and true-size physical-mm defaults converted through `unit:`.
**SPEC 15.11 (Floorplan — the architectural dialect) is the law** — landed in
Phase 0 below, audited; where this plan and SPEC disagree, SPEC wins. The
design conversation is not preserved anywhere else: settled decisions live in
this file, in "Settled design decisions" below.

Branch: `blueprint` — pushed to `origin/blueprint` by the **session lead
after each phase** (user-approved); phase agents never push. Merging to
`main` (and pushing main) is the user's, kept for last.

---

## How to work this plan (read this every session)

1. **Re-orient**: read `SPEC.md` **fully** — at minimum Part I, SPEC 11, all
   of SPEC 15 (the drawing engine you are riding — 15.11 is your section but
   it states only deltas, so the rest of 15 is your contract too), SPEC 17,
   21, 24 — and this plan **fully**, including every phase's execution log
   and carry-over notes. Then `git log --oneline -15` and `cargo test`
   (must be green before you start).
2. **One phase per session.** Do not start a phase you can't finish.
3. **Log as you go**: every phase has `### Execution log` and `### Carry-over
   notes`. Log decisions made, constants chosen, surprises, anything the next
   phase (or a re-run of this one) needs. **Never rely on conversation
   memory — if it matters, it goes in this file.**
4. **Ask the user** about: contract changes not covered here, visual-taste
   calls you can't settle from reference floor plans, anything that would
   break a SPEC law. Small obvious calls are yours — make them and log them.
5. **House rules bind** (AGENTS.md): no `unsafe`; one mechanism per problem;
   no parallel implementations — promote visibility and share, never copy;
   split modules past ~500 LOC; reused style rides a CSS rule + class, never
   inline `style=`; comments only for non-obvious *why*; modern, clean,
   modular, human-readable Rust.
6. **Before every commit**: `cargo fmt`, `cargo test`, `cargo clippy`. Tick
   the phase's checkboxes as you complete them. Never push.
7. **Visual verification is mandatory** where output changes: render the
   sample SVG to PNG with `resvg` and *look at it* (light and dark where
   paint is involved). A floor plan that "compiles" but reads wrong — a swing
   arc on the wrong side, a wall seam showing — is a failed phase.

### Reference sheet (user-supplied, `plans/refs-floorplan/` — untracked)

Five symbol charts the user collected; Phases 3–4 **read these images**
(Read tool renders them) before drawing chrome or fixtures, and settle
visual-taste calls against them:

- `floor-plan-symbols-1.jpg` — single door (jambs + leaf + arc) and window
  (thin rect + centre line) exactly as SPEC lays them out.
- `furniture-symbols-for-floor-plans.jpg` — bed (folded-corner + pillow
  read), sofa/armchair/dining-chair anatomy, "dining table" as a plain rect
  vs "table with chairs" as the drawn set — confirming `|dining|` = table
  **with** chairs and bare tables = `|rect|`; wardrobe (rail line) stays
  deferred.
- `appliance-and-furniture-floor-plan-symbols.webp` — bathtub, shower
  (X + drain circle), toilet (tank + oval bowl), sink (rect + oval basin),
  stove (4 burners), fridge/washer/dryer as labelled boxes. Note the
  industry also writes appliances as a plain box + abbreviation (REF, DW,
  W) — if a pictorial variant reads poorly at 1:50, flag the inside-the-box
  text convention to the user as a possible SPEC amendment before inventing
  anything.
- `stair-symbols-for-floor-plans.jpg` / `stair-floor-plan-symbols.webp` —
  straight stairs = tread rects + direction arrow (our v1); winder / L / U /
  spiral forms are deferred-list material, do not build them.
- `20sw-b1.webp` and `158Front-1BD-D-10.pdf` — **real condo plans** (the
  webp is exactly our solid-poché look). What they settle: stove = square +
  4 circles, nothing more; fridge / dishwasher / washer read as near-plain
  boxes with their letter **inside** (`F`, `DW`, `W/D` — SPEC 15.11 states
  the appliance label centres in the body); kitchen counters and islands are
  plain thin-outline rects; bed = rect + pillow rects + fold line; sofa /
  tables are bare outlines; balcony sliders read as our `sliding` door.

**The taste rule (user-set): MINIMAL.** Every fixture symbol is the fewest
strokes that still reads at 1:50 — "4 circles in a square is good enough"
for a stove. When the symbol charts above show more detail than the two real
condo plans, follow the condo plans. No upholstery lines, no faucet handles,
no burner grates.

### Repo state notes

- A pre-existing dangling edit to `samples/links_hard.lini` (gap/clearance
  tweak, no snapshot) is **stashed** on this branch
  (`git stash list` → "pre-blueprint dangling edit"). Leave it stashed; it is
  the user's, not this plan's.
- The SPEC block **#57** excuse row in `tests/spec_blocks.rs` was removed
  after Phase 1 — the §25 example compiles; the block is guarded like any
  other from here on.

### Cross-phase invariants (verify each against source at session start — they drift)

- **Desugar fixed point**: `tests/oracle.rs` proves lowered output renders
  identically; `tests/desugar.rs` holds the byte-identical source fixed
  point. Every generated node/link needs (a) idempotent detection so
  re-desugar doesn't duplicate it, (b) a span seated past the last instance
  (see the pattern in `desugar/mod.rs`).
- **`layout:` is a bare ident, not an enum.** Dispatch is per-engine
  predicates. The drawing family: `resolve/ir.rs is_drawing`,
  `layout/drawing/mod.rs is_drawing_scope`, plus the near-name family
  (`is_drawing_body`, `is_drawing_node`, `scope_is_drawing`,
  `container_layout` in `validate.rs`, `read_layout_mode` in
  `layout/arrange.rs`). Grep both families; a missed site fails *silently*.
  **Phase 1's central law: `layout: floorplan` satisfies every one of these
  through ONE shared predicate change** — never a second copied check.
- **Error codes**: one row in `src/error/codes.rs::catalog!`, never a
  literal; numbers are stable once assigned (snapshot-pinned).
- **Regen artifacts**: any change to templates, node kinds, or `PROPERTIES`
  requires `cargo xtask gen-schema` + `cargo xtask gen-grammars`
  (byte-identical guards in `tests/schema.rs` / `tests/grammar.rs`); every
  `PROPERTIES` row needs a matching `src/ledger/examples.rs` entry (the
  schema test compiles each).
- **`--lini-*` vars are tree-shaken** by literal `var(--lini-` scans in
  `render/used_vars.rs`. 15.11 adds **no new role variables** — if you think
  you need one, re-read 15.11's "no new role variables" and ask the user.
- **`tests/deferred.rs` pins one test per reachable deferred slot**, in SPEC
  24's order — Phase 5 adds the Floorplans block's slots.
- **Oversized files**: when a phase grows a file past ~500 LOC, split it as
  part of that phase and log the split. Known-big files this plan touches:
  `ledger/defaults.rs` (981), `desugar/types.rs`, `validate.rs`,
  `ledger/properties/mod.rs`.

### The feature in one paragraph (orientation, not law — law is SPEC 15.11)

A floorplan is the drawing engine wearing an architect's vocabulary. The
scope is a drawing scope in every mechanical sense — datum placement,
`scale:` / `unit:`, anchors, the pen, `pattern:`, dimensions, leaders, mates,
sheets, hatch all just work — and adds: `|wall|`, a `|sketch|` whose `draw:`
traces the wall **centreline** and whose inheritable `thickness:` (default
200 mm) is offset at lowering into a mitred, solid-filled (poché) outline —
`|partition|` is the 100 mm interior define; `|door|` / `|window|` ride a
wall's `[ ]` stationed on a straight named `:segment` (`on:` + `at:` +
`width:`), cut a gap in the outline, and draw generated chrome (leaf + quarter
swing arc; sill lines) — `hinge: start|end`, `swing: left|right`, door
`symbol: single|double|sliding`; six fixtures (`|bed| |sofa| |dining| |bath|
|appliance| |stairs|`) are symbol-bodied types with true-size physical-mm
defaults converted through `unit:`, placed with `translate:` / `rotate:`.
Everything is gated: floorplan types outside a floorplan scope error, and the
look is the drawing's own black-on-white — no new role variables.

### Settled design decisions (do not re-litigate)

- **One engine, a dialect flag.** There is no floorplan layout engine. The
  drawing-scope predicate family recognises `floorplan` as a drawing scope
  through **one shared helper** (e.g. a `drawing_dialect(attrs) ->
  Option<Dialect>` with `Dialect::{Drawing, Floorplan}` — pick the idiom that
  fits `resolve/ir.rs`, but there must be exactly one place that knows the
  two names). `is_floorplan` exists only for vocabulary gating and chrome.
- **True-size conversion is one function.** physical-mm → drawing-units is
  `mm / unit_in_mm(scope_unit)`; it lives in one place (beside the existing
  `unit:` handling in `desugar/scale.rs` or `ledger`) and every consumer —
  `thickness:` default, opening `width:` defaults, fixture bodies — calls it.
  Authored values are drawing units and are never converted.
- **The wall's pen path is the centreline; the outline is derived.** SPEC
  15.11 now states the full geometry law (post-audit): offset ± thickness/2,
  miters capped at limit 4, concentric arc offsets (r < t/2 errors),
  butt caps on open ends, `curve()` errors, the outline takes paint and is
  the geometry bbox, and the offset runs in 15.10 step 1 after the `draw:`
  fold and before the bboxes. Implement exactly that.
- **Openings clip the outline at the jambs** — a profile clip, NOT `break:`
  (the wall keeps its length, no `|breakline|`, jambs capped flat across the
  thickness). An opening resolves against its already-folded parent wall —
  the one child that reads down from its part. `translate:` on an opening is
  an error (SPEC 21's floorplan table).
- **Chrome is generated children**, per SPEC 15.7's auto-chrome table (now
  twelve producers): door leaf (a line of length `width` from the hinge jamb
  at 90° open) + quarter swing arc (radius `width`, leaf → closed); `double`
  = two half-width leaves + arcs mirrored about the gap centre; `sliding` =
  two overlapping half-length panel lines offset to either face, no arc
  (`hinge:`/`swing:` error there); window = two sill lines at the
  thickness's thirds; stairs = treads + up arrow. Classes are SPEC 18's
  floorplan hook family (`lini-door-leaf` · `lini-door-swing` ·
  `lini-window-sill` · `lini-stair-tread` · `lini-stair-arrow`) — one rule
  each, never inline `style=`.
- **Swing handedness is SPEC 15.5's pen-travel law** ("left of the pen's
  travel" — the named-edge convention), not a new walker. `hinge: start|end`
  reads the segment's draw direction.
- **Fixture paint**: `stroke: --stroke-dark; stroke-width: 1; fill: --bg`
  (the §8 template row) — furniture masks what it overlaps, thin outline.
  `width`/`height` are floors; the body **stretches** to the resolved box.
  Smart labels: a fixture's beside the body like a discrete's value; an
  opening's is its schedule tag beside the gap; `|floorplan|` (based on
  `|drawing|` — its class chain is `.lini-floorplan .lini-drawing
  .lini-block`, so `|drawing|`-scoped rules reach it) inherits the title →
  `|footnote|` law; a wall's keeps the sketch's centred read.
- **Fixture symbols are authored path data on a true-size mm grid**, one
  module (`layout/floorplan/fixtures.rs` or split per family), scaled by the
  one conversion function and stretched by `width` / `height` overrides.
  Follow the schematic discretes' authoring pattern (`desugar/schematic/`,
  `layout/drawing/symbols.rs`) — but note fixtures are *geometry* (they
  scale with the view's `scale:`), not sheet-space annotation.
- **No routing involvement.** A floorplan scope's links are the drawing's
  (dims, leaders, mates); the router never sees them. Nothing in
  `src/routing` changes in this plan.
- **The theme is out of scope.** The `blueprint` colour theme is SPEC 24
  deferred; no phase here touches `theme.rs`. Floorplans read black-on-white
  through existing tones (`--stroke-dark` / `--stroke-light` / `--bg`).

---

## Phase 0 — SPEC 15.11: the law ✅ (landed before this plan)

- [x] SPEC.md: intro, §8 template rows, §11 layout row, §15 preamble pointer,
      §15.11, §17 (matrix note, `layout` values, `at:`/`symbol:` owners, five
      property rows, retitled heading), §21 floorplan error block, §24
      Floorplans deferred block, §25 worked example.
- [x] `tests/spec_blocks.rs`: ledger row for block #57 (removed in Phase 5).
- [x] Independent audit of the SPEC delta (agent); findings applied.

### Execution log
- 2026-08-28: SPEC delta written on branch `blueprint`; independent audit
  (19 findings, 5 blocking) applied in full: mm defaults unit-marked and the
  authored-value law stated; the `break:` claim replaced with the profile
  clip; smart labels stated for every new type; the wall offset (± t/2,
  miter limit 4, butt caps, concentric arcs), door/window/stairs chrome
  geometry, and the 15.10 step-1 ordering spelled out; `|floorplan|` re-based
  onto `|drawing|`; §11 seams, §19, §15.7 (twelve producers), §18 hook
  family, §21 (widened drawing-gate wording + five new floorplan rows), §24
  (theme bullet moved to Beyond 1.0) all integrated; §25 example corrected
  (west/east centreline dim → 7.2, sofa clear of wall and swing, 27 m²).
  Green baseline (`cargo test`) with the spec_blocks #57 ledger row in place.

### Carry-over notes
- SPEC 15.11 is post-audit the **implementable** law — build to its letter;
  this plan's "Settled design decisions" only restate it plus code-side calls.
- README.md and SKILL.md still enumerate the layout family without
  `floorplan` — Phase 5 sweeps both (enumerations + a short SKILL authoring
  section beside its schematic one).

---

## Phase 1 — Scope & vocabulary plumbing (no drawing behaviour changes)

**Goal**: `layout: floorplan` is a drawing scope everywhere; every 15.11 type
and property exists, cascades, formats, and gates — but lowers to nothing new
yet (a `|wall|` may temporarily lower as its bare centreline sketch). At the
end of this phase the §25 example *parses and resolves* (layout may be wrong;
rendering is Phases 2–4).

- [x] The dialect predicate: one shared change in the drawing-scope family
      (see invariants). Grep `is_drawing` / near-names; verify every caller
      via the greps, then prove with tests: a floorplan root hosts a
      `|sketch|` + dimension exactly as a drawing does.
- [x] Templates: `|floorplan|` (`|block|` + layout), `|wall|` (over
      `|sketch|`), `|partition|` (over `|wall|`, thickness 100), `|door|`,
      `|window|`, `|bed|`, `|sofa|`, `|dining|`, `|bath|`, `|appliance|`,
      `|stairs|` in `ledger/defaults.rs` with SPEC 15.11's defaults
      (true-size defaults stored as physical mm; conversion applied where
      read). Type classes in desugar (`.lini-wall` etc.).
- [x] Properties: `thickness` (inheritable — follow `unit:`'s inheritance
      path), `on`, `hinge`, `swing`, `steps` in `ledger/properties/`;
      extended owners for `at:` and `symbol:`; one `ledger/examples.rs` entry
      per new row; validation of value shapes (SPEC 17).
- [x] Gating: floorplan types outside the scope error (`'|wall|' belongs in a
      'layout: floorplan'` — every type, one mechanism, follow the schematic
      gate in `layout/gates.rs` / `resolve`); openings outside a wall's `[ ]`
      error; required `on:` / `steps:` error like missing `points`. All codes
      through `error/codes.rs::catalog!`.
- [x] `cargo xtask gen-schema` + `gen-grammars`; guards green.
- [x] `lini fmt` handles the new properties (should be free — verify with a
      floorplan snippet through `fmt` tests).
- [x] Tests: gate errors both directions, template defaults present, desugar
      fixed point over a floorplan snippet, schema/grammar guards.

### Execution log

2026-08-28, one session. Baseline 1410 passed / **1 pre-existing failure**
(below); after: 1418 passed / the same 1 failure. `cargo fmt`, `cargo clippy
--all-targets` clean.

**The dialect predicate — where it landed.** `src/resolve/ir.rs` now holds the
**only** place the two layout names sit together:

```rust
pub fn is_drawing_layout(name: &str) -> bool { matches!(name, "drawing" | "floorplan") }
pub fn is_floorplan_layout(name: &str) -> bool { name == "floorplan" }
pub fn layout_reads(scope: &str, owner: &str) -> bool     // the dialect reads its parent engine's surface
pub fn is_drawing(attrs)  -> uses is_drawing_layout        // unchanged callers
pub fn is_floorplan(attrs)-> the vocabulary gate's twin
```

Every site in the predicate family was reconciled through it (greps
`is_drawing`, `is_drawing_scope`, `is_drawing_body`, `is_drawing_node`,
`scope_is_drawing`, `container_layout`, `read_layout_mode`, plus a literal
`"drawing"` sweep):

| Site | How it now answers |
|---|---|
| `resolve/ir.rs is_drawing` | `is_drawing_layout` — so `layout/drawing/mod.rs is_drawing_scope`, `resolve/program/link_scope.rs scope_is_drawing`, the layout dispatch (`layout/mod.rs` root + `layout_inst`), `frames.rs`, `collect_datum_nodes` and `enclosing_view` all followed for free |
| `resolve/ir.rs LinkOwner::consumes_links` | `sequence` **or** `is_drawing_layout` — a floorplan consumes its own links |
| `resolve/scene.rs root_facts` | a drawing-family root now wears its **template chain** (`lini-floorplan` + `lini-drawing`), derived by walking `template_base` — so `\|drawing\|`-scoped rules (the `\|drawing\| \|note\|` pair, chrome dress) reach a root floorplan exactly as they reach the node form |
| `desugar/nest.rs is_drawing_body` | `is_drawing_layout` on the style; the type-chain arm needed nothing — `\|floorplan\|`'s chain **contains** `drawing` |
| `desugar/nest.rs STATEMENT_ENGINES` | `"floorplan"` added (it reads its own body's statements) — feeds `seals_schematic_scope` and `link_scope::seals_schematic` |
| `desugar/mod.rs` root `Nest.drawing` | `is_drawing_layout` (this also drives the `scale::fold` root flag) |
| `lint.rs` `is_drawing_node` / `root_opaque` | `types::derives_from(t, "drawing")` (new, in `desugar/types.rs`) + `is_drawing_layout` |
| `validate.rs` `Owner::Type` satisfaction (node **and** root block) | new `scope_reads_type` → `container_layout(t)` + `layout_reads` — this is what lets `{ layout: floorplan; unit: m }` (the §25 form) validate |
| `layout/arrange.rs read_layout_mode` | unreachable for a floorplan (the engine intercepts first), but its two enumerations now name `floorplan` |
| `desugar/classes.rs scoped_note_rules` | **no change needed** — it emits `.lini-drawing .lini-note`, which the dialect wears |

**Decisions made (log them, they bind later phases):**

1. **True-size defaults are never class rules.** SPEC 8's table states
   `\|partition\| thickness: 100`, `\|door\| width: 900`, `hinge: start`,
   `swing: left` — but a template bundle lowers to a `.lini-*` **class rule**,
   and a rule-borne value cannot be told from an authored one. That breaks two
   laws at once: the mm value would read as drawing units (a 100 m partition at
   `unit: m`), and the sliding-door gate ("`hinge:`/`swing:` on a sliding door
   errors") would fire on every sliding door. So the bundles carry **paint and
   engine name only**; `|partition|`, `|window|` and `|door|` carry **no
   bundle at all**, and their defaults are the reader's (`DefaultRef::Engine`
   in the ledger). Phase 2/3/4 supply them at the read site through the one
   conversion function. `.lini-wall { fill: --stroke-dark; stroke: none }` and
   the fixtures' `stroke: --stroke-dark; stroke-width: 1; fill: --bg` are real
   bundles — they are paint, not size.
2. **The one conversion function** is `desugar::scale::mm_to_units(mm,
   unit_mm)`, beside `read_unit` (the only place a scope's millimetres-per-unit
   is known). It carries `#[cfg_attr(not(test), allow(dead_code))]` until
   Phase 2 wires the first reader (repo precedent: `font/mod.rs`), and a unit
   test pins 200 mm → 200 / 20 / 0.2 at `mm` / `cm` / `m`.
3. **One gate, one walk.** `src/layout/floorplan/mod.rs` is the family home
   (`FpKind` + `fp_kind` + `check`), driven from `layout/gates.rs` — which now
   carries a `floorplan` flag **and the host's type chain**, since "an opening
   rides in its wall's `[ ]`" is a parent question. Five laws, one pass: type
   out of scope, opening host, required `on:`, `translate:` on an opening,
   `hinge:`/`swing:` on `symbol: sliding`; plus `|stairs|` requiring `steps:`.
   Value *shapes* (`hinge` start|end, `swing` left|right, `steps` ≥ 2 integer)
   ride `validate.rs::check_value` beside `pins`/`number` — the existing home
   for wearer-independent malformed values.
4. **New codes** (stable): `Y010 floorplan-type`, `Y011 opening-host`,
   `Y012 opening-placed`, `Y013 sliding-door-leaf`. Missing `on:` / `steps:`
   reuse `Y001 missing-required-property`, as SPEC 21 asks.
5. **Type lists** live beside `DISCRETES` in `desugar/types.rs`: `OPENINGS`
   (door, window) and `FIXTURES` (the six). `Role("opening")` is a new
   validation role (`at:`, `on:`); `symbol:` lists its five variant-bearing
   fixtures explicitly rather than a role, because SPEC 17 excludes
   `|stairs|` from it.
6. **SPEC 21's widened drawing-gate wording applied**: every
   `belongs in a 'layout: drawing'` message now reads `… (or its 'floorplan'
   dialect)` (11 sites, incl. `resolve/links/gates.rs`, `links/mod.rs`,
   `layout/mod.rs`, `drawing/frames.rs`, `drawing/symbols.rs`), and the
   unknown-layout enumeration names `floorplan`.

**No file needed splitting** (the touched ones grew by tens of lines);
`layout/floorplan/mod.rs` is 145 LOC, `tests.rs` 120.

**Surprises / findings:**

- **The tree was already red when this phase started, and still is** —
  `tests/hooks.rs::every_documented_class_is_worn` fails on the five classes
  Phase 0 documented in SPEC 18 (`lini-door-leaf`, `lini-door-swing`,
  `lini-window-sill`, `lini-stair-tread`, `lini-stair-arrow`). Nothing emits
  them until Phases 3–4. It is **not fixable inside Phase 1** and must not be
  papered over: the test's `UNSAMPLED` ledger demands a scene that actually
  renders the class (a twin test proves it), so an entry there would fail too.
  **Phase 3 closes the first three, Phase 4 the last two** — check this test
  the moment door/window/stairs chrome lands.
- **The §25 example does not resolve as written** — `outer:west (-) entry (-)
  outer:east` names the door **bare** from the root scope, but `entry` is
  declared inside `outer`'s `[ ]`, and SPEC 9's sealed bodies make that
  `outer.entry` (exactly as SPEC 15.4 writes `plate:left (-) plate.pin`).
  With that one edit the whole §25 block parses, resolves **and renders**
  today. The same shorthand appears in SPEC 15.11's Openings paragraph. This
  is a SPEC-internal inconsistency, not an implementation gap — **left for the
  user to rule on** (recommended: change both to `outer.entry`); Phase 5 can
  not remove the `spec_blocks.rs` #57 row until it is settled.
- A `|door|`'s `at:` had to join the `at:` homonym row; the misuse message for
  `symbol:` is now long (ten homes) — accepted rather than blurring the owner
  list.
- **Rulings (session lead, 2026-08-28), closing both surprises:** (1) the
  bare-`entry` shorthand was a SPEC error — §25 and 15.11 now write
  `outer.entry` (sealed bodies hold, no carve-out). (2) The five chrome
  classes are **pulled from SPEC 18 until their emitters land** — Phase 3
  restores the floorplan hook-family row with `lini-door-leaf` /
  `lini-door-swing` / `lini-window-sill`, Phase 4 extends it with
  `lini-stair-tread` / `lini-stair-arrow`, each landing together with sample
  coverage so `tests/hooks.rs` stays green; the repo's existing guard IS the
  deferral mechanism, no second ledger. With the path fix the §25 block
  compiles today, so the `spec_blocks.rs` #57 excuse row is removed now, not
  in Phase 5. Tree returns to green with this commit.

### Carry-over notes

**For Phase 2 (walls):**
- Call `desugar::scale::mm_to_units` for the 200 mm / 100 mm thickness default
  and drop its `allow(dead_code)`. The type→default split is
  `fp_kind(...) == Wall` plus a `type_chain` test for `partition`; nothing
  stores those numbers yet — **Phase 2 states them**, beside the reader.
- `thickness:` is ledger-`Inherit::Engine` (nearest-wins **inside** the
  engine, like `scale:`), owners `Type("floorplan")` + `Type("wall")`. The
  inheritance walk is Phase 2's: `unit:`'s own path is the `ScaleCtx` carried
  down `desugar::scale::walk`, which is the model to follow (and the place
  that already knows `unit_mm`).
- A `|wall|` lowers today as a plain `|sketch#w| .lini-wall.lini-sketch` whose
  **centreline** path takes the poché fill — so the §25 render is a solid
  black rectangle. That is the expected Phase-1 state; the offset outline
  replaces it (SPEC 15.10 step 1, after the `draw:` fold, before the bboxes).
- `layout/floorplan/mod.rs` is where the wall module lands (`wall.rs` beside
  `mod.rs`); `FpKind` is already the dispatch to branch on.

**For Phase 3 (openings):**
- The gate already guarantees, before any geometry runs: an opening's parent
  **is** a wall, `on:` is present, `translate:` is absent, and a sliding door
  carries no pose. Phase 3 adds the segment/station laws (unknown segment,
  curved segment, overrun, overlap) — put them in the same `check` pass if
  they are resolved-tree facts, or in the wall lowering if they need folded
  geometry (they do: the segment table comes from `SketchGeo`). Prefer the
  lowering, and keep one law per site.
- The opening's default `width:` (900 / 1200 mm) and its default pose
  (`hinge: start`, `swing: left`) are **the reader's**, deliberately — read
  `attrs`, fall back to the constant through `mm_to_units`.
- An opening currently has no geometry at all, so a dimension chain through
  one reads `0` — the jamb-to-jamb box (`width` × `thickness`) is what fixes
  the §25 location chain.

**For Phase 4 (fixtures):**
- `symbol:` is accepted on `|bed| |sofa| |dining| |bath| |appliance|` only;
  a `symbol:` on `|stairs|` is a *validation* misuse error already. The
  variant tables belong beside `fp_kind`, following
  `desugar/schematic/family.rs`'s `variants()` shape (default = first row).

**For Phase 5:**
- `tests/hooks.rs::every_documented_class_is_worn` must be green by then.
- README.md / SKILL.md still enumerate the layouts without `floorplan`; the
  in-code enumerations (`layout/arrange.rs`, the error messages) are done.

---

## Phase 2 — Walls: thickness, offset outline, poché

**Goal**: a `|wall|` renders as SPEC 15.11's wall — centreline authored,
outline drawn.

- [x] The one mm→units conversion function; `thickness:` resolved through
      inheritance (scope → wall), default 200 mm; `|partition|` 100 mm.
- [x] The offset walk (`layout/floorplan/wall.rs`, new module): centreline
      polyline/arc path → closed outline at ±t/2, mitred at joins, flat caps
      on open ends, `close()` seams mitred like any corner; arcs offset to
      concentric arcs (radius ± t/2); reject `curve()` in a wall (error).
      Property-test the geometry where practical (e.g. outline area ≈
      centreline length × t for gentle paths; offset segments parallel at
      distance t/2).
- [x] Fill/paint: solid `--stroke-dark`, `stroke: none` (the template row);
      verify `fill: --bg` (hollow) and `fill: hatch(45)` read correctly.
- [x] Geometry bbox = outline bbox; anchors: `:segment`s stay centreline
      (dimensions read centreline stations — the architectural convention),
      bbox sides/corners read the outline. Dims, leaders, mates against walls
      under test.
- [x] Overlap composition: two solid walls crossing/meeting read seamless
      (visual check); a wall respects source-order painting.
- [x] Snapshot tests + **visual PNG check** (resvg): an L-corner, a T-meet of
      two walls, a closed rectangle loop, an arc wall segment, open-ended run.

### Execution log

2026-08-28, one session. Baseline 1419 passed / 0 failed → after 1432 / 0;
`cargo fmt`, `cargo clippy --all-targets` clean.

**Thickness inheritance — where it landed.** The `desugar::scale` walk (the
carry-over's `ScaleCtx` model) gained `thickness: Option<f64>`, read at the
root and at every drawing-scope entry beside `unit:`. Every wall-family node
in a drawing scope gets a generated internal **`wall-thickness:`** attr (the
`px-per-unit` pattern: `pub(crate)` const in `scale.rs`, whitelisted in
`validate.rs::INTERNAL`, retain-then-push so desugar stays a byte fixed
point) carrying its **resolved fallback** in drawing units. Precedence, and
why: a wall's own authored `thickness:` suppresses the stamp (the cascade
already carries it); **a `|partition|`'s 100 mm beats the scope's inherited
value** — SPEC 8 calls it "a define, nothing more", and a define's bundle
value sits *at the node*, above inheritance, so the read-site emulation
preserves that (`{ thickness: 0.15 }` on the scope re-sizes plain walls,
never partitions); then the nearest scope value; then 200 mm. The mm
defaults convert through `mm_to_units` (its `allow(dead_code)` dropped); the
read site (`layout/floorplan/wall.rs`) does `attrs.number("thickness")` —
inline *or rule-borne*, so a `|wall| { thickness: … }` class rule wins over
the stamp exactly as the cascade ranks it — else the stamp. `thickness`
value shape (`number > 0`) rides `validate.rs::check_value` beside `steps`.

**The offset walk** (`layout/floorplan/wall.rs`, 444 LOC): hooks in
`layout/mod.rs`'s Sketch branch right after `pen::fold` (= SPEC 15.10 step 1,
after the fold, before the bboxes), gated on `fp_kind == Wall`; it rewrites
`Folded.subs/d/geometry` to the outline and leaves `segments` (centreline
stations), so dims/`:segment`s read the centreline while the bbox, paint,
and `SketchGeo.outline` (leader ray-casts) read the outline — no other
module changed. Algorithm, per centreline subpath (**one contiguous run**,
the structure Phase 3 cuts into):
1. **Raw parallels**: a line shifts along its side normal (`left(d) = (d.1,
   −d.0)`, y-down); an arc offsets concentric — `r + h` when `sweep == left`
   (left of travel is outside a clockwise arc), else `r − h`, endpoints moved
   radially, flags kept.
2. **Joins** at each vertex (and across the seam for a closed run — the wrap
   join re-runs the first element and seats its trimmed copy back at the
   head): coincident ends snap (tangent-continuous — fillets, tangent arcs);
   an **outside** corner mitres by tangent-line intersection (a line extends
   in place; an arc gets a straight tangent connector — SVG's own join
   geometry), bevelling at **limit 4** via `cos θ > 1 − 2∕4² = 7/8` (θ the
   wedge between the runs); an **inside** corner trims both elements to
   their carriers' true crossing — exact line×line, line×circle,
   circle×circle, candidate nearest the corner that lies on both spans, arc
   `large` recomputed from the swept angle — falling back to a straight
   connect when elements are too short to reach it.
3. **Assembly**: closed run → two closed loops, the right side reversed
   (opposite windings, so even-odd and nonzero agree on the band); open run
   → one loop: left chain, flat butt cap at the endpoint (no extension),
   right chain reversed, cap home.

**Errors** (codes Y014 `wall-curve`, Y015 `arc-under-thickness`; catalog
snapshot re-blessed): `curve()` per SPEC 21 verbatim; an arc errors when
`r < t∕2 − ε` with the radius printed in drawing units (`r_px / own`) —
`r == t∕2` stays **legal**: the inner face degenerates to the centre point,
which is well-defined (a half-disc wall), and SPEC says "under".

**Decisions / findings:**

1. **Even-odd stands.** A wall is a sketch and keeps the pen's even-odd law:
   two *crossing subpaths in one wall node* would unfill their overlap. The
   windings are built so nonzero would agree for sane walls, but the law is
   the pen's — draw meeting walls as **separate nodes** (as §25 does) and
   paint order merges them seamlessly. Verified visually: T-meet and full
   crossing of two solid walls read seamless, light and dark.
2. **Sketch paint bbox now uses `half_stroke()`** (`layout/mod.rs`): it used
   raw `stroke-width / 2`, which inflates a `stroke: none` sketch's bbox by
   1 px and would have pushed every wall's bbox anchors off the outline.
   `AttrMap::half_stroke` is the one owner of "how far paint reaches"
   (0 for `stroke: none`), so the fix is the shared reader, not a wall
   special case. No sample churn (no committed sample has an unpainted
   sketch stroke).
3. `tests/desugar.rs`'s "no baked thickness" guard matched the *substring*
   `thickness:` and tripped on the generated `wall-thickness:`; the match is
   now space-anchored (still catches any rule/node literal) and the test
   *positively* pins the stamp — `wall-thickness: 200;` at unit mm,
   `0.2` at `unit: m`.
4. `PathSeg::reverse` promoted `pub(in crate::layout)` (was private);
   `desugar::scale` promoted `pub(crate)` — the stamp const and
   `mm_to_units` are its exports.
5. **Property tests are deterministic sweeps** (no fuzz dep, `crate::math`
   only — the libm determinism test rejects `f64::sin/cos`): a straight run
   at bearings 0..360×7° is an exact L×t rectangle (area *and* perimeter);
   mitred zig-zags and arc bands hold area = centreline length × t exactly
   (the outer miter kite equals the inner trim; (r+h)²−(r−h)² does the same
   for arcs) — measured by an independent sampled-shoelace oracle over the
   placed `SketchGeo.outline`.

**Visual pass** (resvg → PNG, read by eye, light + dark): L-corner (square
outer miter, crisp inner notch, flat caps); T-meet + crossing of two solid
walls (seamless); closed rectangle (two concentric loops, courtyard empty);
arc wall (concentric band, radial butt caps); ~12° hairpin (bevel, no
spike); hollow (`fill: --bg; stroke: --stroke-dark` double-line) and
`fill: hatch(45)` bands with junctions showing, both masking correctly; the
§25 block renders real walls — solid poché rectangle, partition tee-ing in
seamlessly, top dim reading 7.2 (centreline), matching
`plans/refs-floorplan/20sw-b1.webp`'s look. One false alarm worth logging:
an early gallery scene "showed huge filled regions" — those were correct
200 mm-default walls at `density: 1` (200 px wide); authored test walls
should state a thickness or a sane unit/scale.

### Carry-over notes

**For Phase 3 (openings):**
- **The run structure to cut into**: `wall::offset_run` offsets one
  centreline subpath via `side(segs, closed, h, left)` → two joined side
  chains, then assembles loops. A gap at stations `a..b` on a straight
  segment splits **both side chains** at those parameters and assembles
  multiple loops, each jamb capped by a straight `Line` across the thickness
  — from `p + h·normal(dir, true)` to `p + h·normal(dir, false)` at station
  point `p` — exactly the open-run butt cap's construction (`push_line`,
  `normal` are right there). Suggested shape: pass per-segment gap intervals
  into `offset_run` and cut in `side` before the joins run (a gap never
  coincides with a corner: overrun validation guarantees it stays inside
  the segment).
- **The segment table** lives on `Folded.segments` / `SketchGeo.segments` —
  **centreline** coordinates, already ×`own` (px). `on:` resolves there
  (`Segment::Edge(a, b)` carries direction; reject `Arc`/`Circle` — the
  curved-segment error; `Point` is not a segment). `at:`/`width:` are
  drawing units ×`own`; their 900/1200 mm *defaults* need the same
  desugar-time stamping `wall-thickness:` got — **extend
  `scale::stamp_wall_thickness`'s pattern** (the walk is the only place
  `unit_mm` is known; layout cannot convert mm), one internal attr per
  true-size fallback, whitelisted in `validate::INTERNAL`.
- **Where the cut runs**: inside `wall::offset` (called from
  `layout/mod.rs`'s Sketch branch with the wall's `ResolvedInst` — the
  openings are `inst.children`, resolved and gate-checked by then: host,
  `on:` present, no `translate:`, sliding-pose law all already hold).
  Station laws that need folded geometry (unknown segment + suggestion,
  arc segment, overrun, overlap) belong there too — one law per site.
- An opening's own geometry (the `width × thickness` jamb-to-jamb box) is
  what makes `outer:west (-) outer.entry (-) outer:east` read the true
  location chain — today it reads 0 and 7.2 (an opening still has no box;
  expected). Mind `at:`'s frame: it measures from the segment's **draw
  start**, and §25's `south` is drawn right-to-left.
- Restore the SPEC 18 floorplan hook-family row **with** the leaf/swing/sill
  emitters and rendered sample coverage in the same commit —
  `tests/hooks.rs` is the guard (Phase 1 ruling).
- The wall's placed node carries `attrs["path"]` (outline `d`),
  `sketch.outline` (outline subs — leader targets), `sketch.segments`
  (centreline). Door/window chrome should follow the `place_features` /
  `chrome::fill` pattern for generated children.

**For Phase 4 (fixtures):** nothing new beyond Phase 1's notes; the
mm-fallback stamping pattern above applies to fixture bodies too if their
sizes are read at layout.

---

## Phase 3 — Openings: doors & windows

**Goal**: SPEC 15.11's openings, fully — gaps, chrome, poses, errors, dims.

- [x] Station model: `on:` resolves a straight segment of the host wall
      (unknown → suggestion error; arc → error); `at:` + `width:` (mm default
      via conversion; 900 door / 1200 window) validated against segment
      length; overlapping openings on one segment error.
- [x] Gap cutting in the outline builder (Phase 2's module): both wall faces
      opened, jambs capped flat.
- [x] Door chrome (generated children, `--stroke-light` weight 1): leaf line
      from the hinge jamb + quarter swing arc, `hinge: start|end` ×
      `swing: left|right` (walker's rule along the segment's draw direction);
      `symbol: single` (default) / `double` (two leaves, two arcs) /
      `sliding` (offset panel lines, no arc).
- [x] Window chrome: two sill lines at the thickness's thirds (SPEC 15.11);
      compare against `plans/refs-floorplan/floor-plan-symbols-1.jpg` (the
      reference draws rect + centre line — if that reads better at 1:50,
      propose the SPEC tweak in the log rather than silently diverging).
- [x] Openings as geometry: an id'd opening anchors dimensions at its centre
      (`outer:west (-) outer.entry (-) outer:east` renders the location
      chain).
- [x] Restore the SPEC 18 floorplan hook-family row with `lini-door-leaf` ·
      `lini-door-swing` · `lini-window-sill` — emitted as one rule each and
      worn in a rendered snapshot so `tests/hooks.rs` passes (Phase 1 ruling).
- [x] Snapshot + **visual PNG check**: all four hinge/swing poses, each door
      symbol, a window, a door at a segment end, two openings on one wall —
      light and dark.

### Execution log

2026-08-28, one session. Baseline 1432 passed / 0 failed → after **1442 / 0**;
`cargo fmt --check`, `cargo clippy --all-targets` clean. §15.7's producer row
for `|door|` / `|window|` was already there (Phase 0) — verified, not
duplicated. No SPEC contradiction found; the only SPEC edit is the §18
hook-family row this phase owed.

**The central geometry decision: the gap cuts the *run*, not the two side
chains.** Phase 2's carry-over proposed splitting `left`/`right` at the
stations and capping each jamb with the butt cap's `push_line`/`normal`
construction. Cutting the **centreline run** first and offsetting each piece
as its own *open* run gets the identical outline with **no new geometry code
at all**: a jamb cap *is* the open-run butt cap, and every interior corner
keeps its mitre because it stays inside a piece. `wall::cut` is the whole
mechanism (≈50 lines): walk the subpath, split the gapped `Line`s at their
station parameters, emit a piece per interval; a **closed** run's tail piece
then absorbs the head piece — a gapped ring is one band, not two loops (the
seam stops being a corner). Ungapped runs pass through untouched, so every
Phase-2 snapshot is byte-identical.

**Where each piece lives.**

| Concern | Home |
|---|---|
| stations, laws, opening geometry, chrome fill | `layout/floorplan/opening.rs` (new, 325 LOC) — **one pass** over the wall's `[ ]`, so the station is computed once |
| the cut + the offset assembly | `layout/floorplan/wall/mod.rs` (282) |
| corner joins / carriers / trims | `layout/floorplan/wall/join.rs` (250) — `wall.rs` passed 500 LOC, split along "how a side chain runs" vs "how two offset elements meet"; `push_line` stays in `mod.rs` (`pub(super)`) since both the cap and the bevel are the same connector |
| chrome *count* (never geometry) | `desugar/drawing.rs::opening_chrome` — the existing `chrome:` marker mechanism, one more producer |
| the 900 / 1200 mm fallbacks | `desugar/scale.rs::stamp_opening_width` → internal `opening-width:`, the `wall-thickness:` pattern verbatim (whitelisted in `validate::INTERNAL`) |

**The opening's frame** (the reason there is no second walker): the placed
`|door|` / `|window|` carries `rotation = the segment's bearing`, so its
children draw in a frame where `+x` is the pen's travel and `+y` is the
**right** of it. `hinge: start` is then always `−x`, `swing: left` always
`−y`, at every bearing and in both draw directions — §25's right-to-left
`south` needs no special case, and its `swing: right` opens north, into the
flat (verified visually). The arc's SVG sweep flag is computed from the turn's
own cross product, never a 4-row pose table.

**Decisions / findings:**

1. **`place_features` skips openings**, beside its chrome skip: an opening is
   placed by `on:` / `at:` alone [SPEC 15.11], seated by its wall as it folded,
   exactly as chrome takes its geometry from the shape that generated it. One
   clause in the one datum-placement site — no second placement path.
2. **The leaf pivots on the swing-side *face*, not the centreline.** SPEC
   states "a line of length `width` from the hinge jamb … its quarter swing
   arc … sweeping leaf to closed" without naming the face; both reference
   plans (`20sw-b1.webp`, `158Front-1BD-D-10.pdf`) and the symbol chart draw
   the leaf rising from the jamb **at the wall face the door opens toward**,
   with the arc landing flat on that same face. That is what is built.
3. **Sill lines: SPEC's thirds stand — no divergence.** Both variants were
   rendered at 1:50 (`sillA` = ±t/6, `sillB` = one centre line, the symbol
   chart's read). At 1:50 the paired lines read unmistakably as glazing and
   match the condo plan's window exactly; the single centre line reads thin
   and is easy to mistake for a break. The "rect" half of the chart's symbol
   is already supplied by the clip itself — the gap's white against the poché
   *is* the frame. **No SPEC edit proposed.**
4. **A slider's panels** are half the clear width each, seated **on** the two
   faces (`y = ∓t/2`), meeting at the gap centre — SPEC's "two overlapping
   half-length panel lines offset to either face": at plan scale the pair
   reads as one set passing the other, which is exactly the reference
   balcony slider. `hinge:`/`swing:` are gate errors there (Phase 1), so the
   panels need no pose. `double` ignores `hinge:` (both jambs hang a leaf)
   and still reads `swing:`.
5. **Three chrome types, three rules**: `|door-leaf|` (a slider's panels wear
   it too), `|door-swing|`, `|window-sill|` — `|line|`-based templates in
   `desugar/types.rs` with **one** shared bundle row in `ledger/defaults.rs`
   (`stroke: --stroke-light; stroke-width: 1; fill: none`), so each is one CSS
   rule and a class on every wearer, never an inline style. The swing arc
   flips its kind to `|path|` where it fills — the round-thread play.
6. **New codes** (stable): `Y016 opening-segment` (both halves of SPEC 21's
   `on:` row — unknown, with the suggestion, and not-straight, which also
   covers a `point():name` station: "… — ':corner' is a point"), `Y017
   opening-overrun`, `Y018 opening-overlap`. An anonymous opening names its
   written type in those messages (`'|window|' at 9.5 + width 1200 overruns
   'run' (length 10)`).
7. An opening's box paints nothing by default — `|door|`'s chain bottoms out
   at `.lini-block { fill: none; stroke: none }` — but it *is* a real shape, so
   `|door| { fill: --bg }` masks, and the box is what dimensions anchor on.
8. §25's location chain now reads **3.75 then 3.45** (sum 7.2): `south` is
   drawn east→west, so `at: 3.0` seats the near jamb 3 m from the **east** end
   and the gap centre 3.45 m from it — 3.75 m from the west. The plan's
   "3.45 / 3.75-ish" is exactly this, in that order.

**Visual pass** (`resvg` → PNG, read by eye, light **and** dark): the full §25
studio flat (doors, windows, both dimension rows — reads like a real-estate
plan, and the entry door swings into the flat); all four hinge × swing poses
on one east-running wall; `single` / `double` / `sliding` / window side by
side (the double's two arcs meet at the gap centre exactly as
`20sw-b1.webp`'s); a vertical wall with a door at `at: 0.2` and one running to
the segment's very end; solid / hollow / `hatch(45)` walls all clipping with
flat jambs; a 60°-bearing wall (leaf and sills turn with it); the two sill
variants stacked for the taste call.

### Carry-over notes

**For Phase 4 (fixtures):**
- The `opening-width:` stamp is the pattern for any fixture size read at
  layout: one internal attr per true-size fallback, pushed in
  `scale::walk`, whitelisted in `validate::INTERNAL`, read as
  `attrs.number("width").or_else(stamp)` so a cascaded value always wins.
- `|stairs|`'s treads + up arrow are generated children like the door chrome:
  add the count to `desugar::drawing::opening_chrome`'s neighbourhood (a
  `stairs_chrome`, same `chrome:` tuple shape `[Ident(kind), Number(index)]`),
  register `stair-tread` / `stair-arrow` in `desugar/types.rs`, give them the
  **same** one bundle row in `ledger/defaults.rs`, and fill their geometry
  where the flight is sized. Then extend SPEC 18's floorplan row (already
  restored, three classes) with the two, and add the ledger entries in
  `tests/hooks.rs` beside `FLOORPLAN_OPENINGS`.
- Fixtures **do** datum-place (`translate:`), so they need no `place_features`
  exemption — that skip is openings-only.

**For Phase 5 (showroom):**
- `tests/hooks.rs::UNSAMPLED` now carries `lini-door-leaf` /
  `lini-door-swing` / `lini-window-sill` against a small inline scene
  (`FLOORPLAN_OPENINGS`). Once `samples/floorplan.lini` wears them, those three
  rows can go — the sweep will cover them.
- `tests/deferred.rs` wants the curved-segment-opening slot: the message is
  `an opening sits on a straight run — ':bay' is an arc` (code `Y016`).
- The desugar fixed-point corpus in `tests/desugar.rs` already carries a
  four-symbol opening scene; extend it rather than adding a case if a new
  generator lands.
- A wall whose `[ ]` openings consume every segment renders only its chrome
  (no panic, empty outline) — worth one line in the sample if it ever looks
  wrong.

---

## Phase 4 — Fixtures: the furniture library

**Goal**: the six fixture types render true-size with their `symbol:`
variants; `|stairs|` generates.

- [x] **Read the reference sheet first** (`plans/refs-floorplan/`, all five
      images — the Read tool renders them); author every symbol against it.
- [x] Symbol path library on the mm grid (SPEC 15.11's table is the size
      law): bed double/single (mattress + pillow read), sofa three/two/corner
      (seat + back + arms), dining six/four/round (table **with chairs**),
      bath tub/shower/toilet/sink, appliance stove (4 burners) / fridge
      (double-door tick) / washer / dishwasher (door line + circle reads).
      Author for the modern-condo read; keep line work `stroke-width: 1`.
- [x] Sizing: intrinsic mm through the conversion function; `width` /
      `height` floors stretch the symbol (like `|image| fit: stretch` — pick
      and log the exact scaling rule per family; a toilet should not distort
      absurdly — if a family must keep aspect, log it and error or letterbox
      consistently).
- [x] `|stairs|`: `steps: N` (required, ≥ 2) generates treads at 250 mm pitch
      × 900 mm width + the direction arrow; `width`/`height` override.
- [x] Unknown `symbol:` errors with a suggestion (shared machinery).
- [x] Extend SPEC 18's floorplan hook-family row with `lini-stair-tread` ·
      `lini-stair-arrow`, worn in a rendered snapshot (`tests/hooks.rs`
      green — Phase 1 ruling).
- [x] Snapshot + **visual PNG check**: one sheet laying out every fixture ×
      every variant at `unit: m`, verified by eye at 1:50.

### Execution log

2026-08-29, one session. Baseline 1442 passed / 0 failed → after **1451 / 0**;
`cargo fmt --check`, `cargo clippy --all-targets` clean. The only SPEC edit is
the §18 hook-family row this phase owed (two classes added). No SPEC
contradiction found — 15.11's table is implementable as written.

**A fixture is one path on its own node.** The body — outline *and* detail —
is a single `d` on the fixture node, whose kind flips to `|path|`; the type's
own class rule (`stroke: --stroke-dark; stroke-width: 1; fill: --bg`) paints
it. So a symbol needs **no generated children and no class of its own**, SPEC
18 gains only the two `|stairs|` hooks it was promised, and the masking read
is the node's own `fill: --bg` under the nonzero rule. `|stairs|` is the one
exception the SPEC states: its risers and up arrow are real chrome children.

| Concern | Home |
|---|---|
| sizing, stretch, variant table, label seat, flight chrome | `layout/floorplan/fixtures/mod.rs` (229) |
| every family's strokes on the mm grid | `layout/floorplan/fixtures/draw.rs` (198) |
| the shape alphabet + the one path emitter | `layout/floorplan/fixtures/shape.rs` (132) |
| the flight's chrome **count** | `desugar/drawing.rs::stairs_chrome`, beside `opening_chrome` |
| the scope's `unit:`, for the reader | `desugar/scale.rs::stamp_unit_mm` → internal `unit-mm:` |
| the unknown-`symbol:` wording | `suggest::unknown_symbol` — the discretes now call it too |

**The stamp is the *unit*, not a size — and why that is not a second
mechanism.** `wall-thickness:` and `opening-width:` carry a **resolved
fallback**: the walk can compute them because nothing downstream changes the
answer. A fixture's body cannot be resolved that way — the `symbol:` that
picks its millimetres is the cascade's (a rule-borne `|bath| { symbol: toilet
}` must work), and `width:` / `height:` then stretch it. So the same walk —
still the only place `unit:` is known — stamps `unit-mm:` (millimetres per
drawing unit) on every fixture, and the read site calls the **one**
conversion function, `scale::mm_to_units`. Whitelisted in `validate::INTERNAL`
beside the other two.

**Where it hooks.** `layout_inst` gained three lines, no new pass:
`fixtures::plan` runs where `part` is known and its box **is** the bbox
(ahead of the `Sketch` / `part_bbox` arms); `fixtures::finish` runs beside
`page::finish` / `schematic::fill_tag` — the existing fill-once-sized step —
and `fixtures::paint` sets the kind and `path` beside the sketch's `d`. So
`rotate:`, `translate:` (datum placement, no exemption), `mirror:` and
`pattern:` all still fold around it exactly as they do for any part.

**The stretch rule, one for every family.** Intrinsic extent in millimetres →
pixels through `mm_to_units(1, unit_mm) × own`; the resolved box is
`max(intrinsic, width × own) × max(intrinsic, height × own)` (`width:` /
`height:` are floors, [SPEC 5]); the body draws at one factor **per axis**,
`px = per_mm × resolved ∕ intrinsic`. **Nothing keeps aspect** — a stretched
circle is an ellipse, a stretched rounded corner an elliptical one. That is
what "the body stretches to the resolved box" means, and a per-family
exception would be exactly the special-casing AGENTS.md forbids. Unstretched
is the overwhelming case: only an authored `width:`/`height:` above the true
size moves either factor.

**Winding is the emitter's, never the author's.** One path + nonzero fill
means two subpaths winding against each other *cancel* — a hole exactly where
the furniture is supposed to mask. Rather than trust each family to author its
detail runs in the right direction, `shape::wound` normalises every
polyline/polygon to the alphabet's one sense (rects, rounds and ovals are
authored in it). The corner sofa's seat line is the case that found this: read
naturally it winds against the L and punched the seat out. Pinned by
`a_bodys_detail_lines_wind_with_its_outline` and proved visually over a solid
poché wall.

**Per-family authoring decisions** (taste rule: MINIMAL, real condo plans over
the charts wherever they differ):

- **bed** — mattress + pillow(s) at the head + one turned-down sheet line
  400 mm below them. `double` splits two pillows about an 80 mm gap, `single`
  takes one. Three strokes; the pillows are what make it read.
- **sofa** — outline + **one** inner run tracing arm → back → arm (200 mm
  arms). The charts' cushion divisions are clutter at 1:50 and were dropped.
  `corner` folds the same two strokes round a 900 mm-deep L in the 2400 square,
  seat facing the open quadrant.
- **dining** — tabletop + chairs as plain 450 squares seated **flush** against
  the long edges, which is what makes the bbox exactly SPEC's (1800 × 1800 for
  `six`, 1200 × 1700 for `four`, 2100 square for `round`). A gap would have
  contradicted the stated extent; flush is also how the chart draws it.
- **bath** — `tub` rim + rounded basin + drain at the tap end (the drain is
  the one stroke that says which way it lies); `shower` square + X + drain;
  `toilet` tank + bowl **lapping 120 mm into it** (drawn tangent first, and
  the pair read as two detached pieces — fixed after looking); `sink` rect +
  oval basin, no tap.
- **appliance** — `stove` is the square + four circles the real plans draw and
  nothing else. `fridge` / `washer` / `dishwasher` are near-plain boxes whose
  **letter is the smart label**, never baked into the path (SPEC's
  labelled-box convention, and `20sw-b1.webp`'s own F / DW / W/D): a door line
  across the front, a drum panel, and the plain box respectively — one stroke
  apiece, enough to tell them apart unlabelled without competing with the text
  that centres in them. **No pictorial variant needed flagging** — the
  label-in-box read is strong at 1:50.
- **stairs** — 900 wide × `steps` × 250 run; the outline is the body, the
  `steps − 1` interior risers and the up arrow are chrome. The arrow climbs
  from the **first tread's centre** to the far edge, head 110 mm.

**Decisions / findings:**

1. **The label's two seats.** Five families read their smart label beside the
   body — centred under it, clear by `consts::READOUT_GAP`, the discrete
   value-readout's own constant (reused, not re-invented). An `|appliance|`'s
   stays where a `|block|`'s text already lands: the centre. Text children
   stack if there is more than one.
2. **No new error code.** SPEC 21's floorplan block states no unknown-variant
   row, and the discretes' equivalent has always been codeless (phase-generic).
   Rather than pin a number SPEC does not know about, the message was
   **shared**: `suggest::unknown_symbol` now words both, so a fixture reads
   `unknown symbol 'sectional' on '|sofa|' — its variants are three, two,
   corner`. The variant list beats a fuzzy guess for a set this small.
3. **The variant is read at layout, not desugar** — the whole reason the stamp
   carries the unit. That is what makes a rule-borne `symbol:` work, and it is
   tested both ways.
4. **Known limitation, inherited from the door**: `stairs_chrome` counts from
   the **authored** `steps:`, exactly as `door_symbol` reads the authored
   `symbol:`. A `steps:` reaching a flight only through a class rule sizes the
   body correctly (layout reads the cascade) but generates no risers and no
   arrow. Closing it means either matching the count at layout (a second
   mechanism for the same job) or teaching desugar the cascade; neither is
   worth it for the form nobody writes. If it ever bites, fix **both**
   producers in `desugar/drawing.rs` at once.
5. `ledger/defaults.rs`'s chrome row now covers five types in one arm
   (`door-leaf` · `door-swing` · `window-sill` · `stair-tread` ·
   `stair-arrow`) — one rule each, a class on every wearer, no inline style
   (pinned in `tests/desugar.rs`).

**Visual pass** (`resvg --static` → PNG, read by eye, light **and** dark):
the full variant sheet — every fixture × every variant at `unit: m`, 1:50 —
looked at and iterated three times (the toilet's detached bowl, the fridge's
door line sitting on the edge, the bed's turndown crowding its pillows); a
labelled sheet proving the beside/inside seats; furniture crossing a solid
poché wall, proving the mask and catching the corner sofa's winding; and the
**§25 studio flat, rendering complete for the first time** — walls, openings,
both dimension rows (7.2 / 3.75 / 3.45), a rotated bed, the corner sofa, the
toilet and shower in the bathroom corner, the fridge. Side by side with
`plans/refs-floorplan/20sw-b1.webp` it is the same drawing: solid poché,
thin-outline furniture masking the floor, doors with their swing arcs.

### Carry-over notes

**For Phase 5 (showroom):**
- `tests/hooks.rs::UNSAMPLED` now carries five floorplan rows —
  `lini-door-leaf` / `-swing` / `lini-window-sill` against `FLOORPLAN_OPENINGS`
  and `lini-stair-tread` / `-arrow` against `FLOORPLAN_STAIRS`.
  `samples/floorplan_parts.lini` wears all five by construction (a door, a
  window, a flight), so the whole block of rows drops with that sample.
- **Every hook is emitted now**, so `every_documented_class_is_worn` is green
  on its own terms — Phase 1's red is fully closed.
- The catalog sheet is already drafted, in effect: this phase's variant sheet
  (every fixture × every variant, labelled, on a `translate:` grid at
  `unit: m; scale: 0.02`) is what was looked at, and it is the shape
  `floorplan_parts.lini` wants — add the door symbols × poses and a window
  beside it. Author sizes in **drawing units** (`width: 2` is 2 m at
  `unit: m`); the true-size defaults need nothing stated.
- Two seats worth remembering when composing: a fixture's label hangs
  **below** its body (it is real ink and collides), and an `|appliance|`'s
  sits inside — so a catalog row wants vertical air, and the kitchen boxes
  want their `F` / `DW` / `W/D`.
- `tests/deferred.rs` still wants the curved-segment-opening slot (Phase 3's
  note) — nothing new is owed by this phase; the Floorplans deferred block
  names no fixture syntax that is reachable.
- The desugar fixed-point corpus in `tests/desugar.rs` already carries a
  `|stairs| { steps: 12 }` and a `|bed|`, so the chrome and the `unit-mm:`
  stamp are both pinned idempotent.
- SKILL.md's floorplan section should state the two label seats explicitly
  (beside for furniture, inside for an appliance) — it is the one rule an
  author is surprised by.

---

## Phase 5 — Showroom, canon & hardening

**Goal**: the feature is a shipped family — sample, SPEC example compiling,
deferred slots pinned, everything green.

- [x] Read `PLAN-GALLERY.md`'s "pretty bar" first — samples are showroom
      pieces (user rule: pretty, feature-dense, never crowded; showcase more,
      like the schematic samples do).
- [x] `samples/floorplan.lini`: the studio flat (walls + partition + openings
      + fixtures + room text + dimension chains), §25's example grown into a
      real showpiece — a full one-bedroom condo reading like
      `plans/refs-floorplan/20sw-b1.webp`; render, **look at it** (light +
      dark), iterate until it reads like a real-estate plan.
- [x] `samples/floorplan_parts.lini`: the catalog sheet — **every fixture ×
      every variant**, every door symbol × poses, a window, a stairs run,
      labelled, on a tidy grid (the `schematic_parts.lini` precedent); this
      is also what lets the hooks `UNSAMPLED` rows drop.
- [x] Remove the `tests/spec_blocks.rs` #57 ledger row — the §25 block must
      compile as written. (Done early, after Phase 1's path-fix ruling.)
- [x] `tests/deferred.rs`: one pinned test per reachable SPEC 24 Floorplans
      slot (computed areas — n/a if no syntax is reachable; curved-segment
      opening → the error; others n/a — mirror the file's conventions).
- [x] Conformance snapshot for the sample; `lini fmt` canon over it
      (`fmt --check` clean); desugar fixed point over it.
- [x] Docs sweep: README.md and SKILL.md layout enumerations gain
      `floorplan`; SKILL.md gains a short floorplan authoring section beside
      its schematic one (walls → openings → fixtures → dims, one example).
- [x] `cargo fmt` / `cargo test` / `cargo clippy` clean; sweep this plan's
      checkboxes; write the final execution log.
- [x] Re-read SPEC 15.11 top to bottom against the built behaviour — fix
      either (SPEC wins on intent; if the implementation taught us better,
      propose the SPEC edit to the user in the log).

### Execution log

2026-08-29, one session. Baseline 1451 passed / 0 failed → after **1452 / 0**
(the one new test is `tests/deferred.rs::openings_on_a_curved_segment`);
`cargo fmt --all --check`, `cargo clippy --all-targets` clean. **No engine code
changed** — nothing in the two samples exposed a bug.

**`samples/floorplan.lini` — the showpiece.** A 9.6 × 6.8 m one-bedroom condo at
`unit: m; scale: 0.02` (1 : 50) with `density: 5`, so a metre is 100 px and the
200 mm poché reads 20 px — the `20sw-b1.webp` weight. The whole plan rides one
`|floorplan#unit|` node, so its label lowers to the drawing title → footnote.
What it wears: the outer `|wall|` loop (`close():west`), two `|partition|`s
(bedroom L, bathroom L), an entry door + a `symbol: sliding` balcony door +
three windows on the shell, an interior door per partition (one `swing: right`,
one `hinge: end` on the defaults), seven `|slab::rect|` casework pieces
(counters, island, coffee table, nightstand, wardrobe, vanity, balcony deck),
four `|appliance|`s (the F / DW / W/D labels centring **in** the box), sink /
toilet / tub, sofa / dining / bed, six `|room::block|` sheet texts with authored
areas, a `|sketch|` north arrow (SPEC 24's "a `|sketch|` define covers it
today", built), and four dimension rows.

**Three sample-level findings worth remembering.**

1. **A `|block|` in a drawing scope does not stack its children** — every child
   datum-places, so a two-line room label overprinted itself. `|room::block| {
   layout: flow }` restores the column. That is the idiom for any multi-line
   sheet text in a drawing or floorplan scope.
2. **Dimension extension lines start at the anchor's *midpoint*, so a
   `:segment` anchor draws a line straight down the wall — and it shows through
   every opening it crosses** (a window read as *three* lines: two sills plus
   the extension line). §25's `outer:west (-) outer:east` form hits this on any
   plan with openings. The fix used here is architectural anyway: name the
   corners with `point():nw` … `point():sw` and dimension **corner to corner**,
   so every extension line runs *away* from the plan. See the carry-over — this
   may deserve an engine answer.
3. **Dimensions stack outside the geometry on their side**, so a `side: left`
   row landed ~2 px off the balcony deck's edge. Moving both vertical rows to
   `side: right` (the location chain inside, the overall outside — the drafting
   order) and the north arrow to the empty left column fixed the composition.

**`samples/floorplan_parts.lini` — the catalog.** Seven columns on a 2.7 m
pitch, six labelled rows, `density: 4`, root `font-size: 10` so the fixtures'
own smart labels and the `|cap|` captions share one size. Rows: **walls**
(solid 200 · `|partition|` 100 · `thickness: 0.4` · `fill: --bg` hollow ·
`hatch(45)` section · mitred corner · `arc()` concentric), **doors** (all four
`hinge` × `swing` poses · `double` · `sliding` · one in a `|partition|`),
**windows · stairs** (the 1200 mm default · `width: 2` · in a `|partition|` · a
door **and** a window sharing one run · `steps: 4` · `steps: 8`), **beds ·
sofas**, **dining · bath**, **appliances**. A fixture's variant name is its own
smart label (which is also the demo of the beside-the-body seat); the four
appliances carry their letter **inside** and a `|cap|` below, which is the
two-seats contrast in one row. `tests/hooks.rs`'s five floorplan `UNSAMPLED`
rows and their two scene constants are **gone** — the sample wears every hook by
construction, and `every_documented_class_is_worn` passes from `samples/` alone.

**`tests/deferred.rs`.** SPEC 24's Floorplans block has four items and exactly
**one** is reachable: `openings_on_a_curved_segment` pins `Y016`
(`an opening sits on a straight run — ':bay' is an arc`) and asserts both
built forms beside it — the arced wall itself compiles, and a straight run of
the same wall takes an opening. The other three (computed room areas, a north
arrow / scale-bar type, more built-in fixtures) reserve **no spelling** — SPEC
names no type or property for any of them — so they went into the module
header's "Unreachable — no test" list in SPEC 24's order, the file's own
convention for exactly this.

**Canon.** `lini fmt` needed no fix: its only changes over both samples were
ordinary canon (trailing `.0` trimmed, over-long declaration blocks wrapped,
double spaces closed) — nothing floorplan-shaped is mangled. Both files are
`fmt --check` clean as committed. `tests/conformance.rs`'s glob picked both up
(two new `.snap` files), and the `tests/fmt.rs` / `tests/desugar.rs` /
`tests/oracle.rs` sample sweeps cover them for free — desugar's byte fixed point
and the lowered-render oracle both hold.

**`crates/lini-wasm/pkg` was stale** (built 2026-08-28 11:15, before Phase 1)
and `tests/wasm.rs` failed with `unknown type 'floorplan'` on both new samples.
`cargo xtask wasm` rebuilt it; parity is green. The pkg is gitignored build
output, so nothing is committed — but **any session adding a sample must rebuild
it** or the parity test fails misleadingly.

**Docs.** README gained `floor plans` in the two family enumerations plus a full
**Floor plans** section between Engineering drawings and Schematics (compiled
snippet, `--strict` clean). SKILL.md gained `floorplan` in the `layout:` list
and the preset table, and a **Floorplan (architectural)** section beside the
schematic one: the walls → openings → fixtures → dimensions order, a compiled
example, and five bullets covering the poché, the station model, the fixture
table, the plain-`|rect|` escape, and the corner-station dimension idiom. Both
doc snippets were compiled `--strict` before landing.

**Visual verification** (`--static` → `resvg`, read by eye): both samples at
full size and at `--zoom 0.3` thumbnail, light **and** dark (eight PNGs), plus
2× / 2.5× crops of the entry door, the kitchen, the bedroom, the bathroom, a
north-wall window and the wall row. What the looking changed: the room labels
(the `layout: flow` fix), the dimension anchors and sides (findings 2 and 3),
the north arrow's size and its move to the left column, dropping a trial `"D1"`
schedule tag off the entry door (it seats *in* the gap — see the SPEC verdict),
adding `stroke: --stroke-dark` to the catalog's `hatch(45)` wall so the section
reads with edges, tightening the catalog's row pitch, and sliding the toilet
south to group with the tub. Dark mode inverts to white poché on ink and reads
like a negative blueprint; both thumbnails stay legible.

**SPEC 15.11 re-read — verdict.** Clause by clause against the built behaviour,
the section is **implemented as written** with two exceptions, neither fixed
here (SPEC is not edited by this phase):

1. **An opening's smart label seats *in* the gap, not beside it.** 15.11 says
   "An opening's is its schedule tag **beside** the gap"; the implementation
   gives an opening no label seat of its own, so `|door#d1| "D1"` falls through
   to `|block|`'s centred text and lands in the middle of the jamb-to-jamb box
   — on the wall line, and turned with the door's frame. Verified on a
   horizontal wall: `<text x="0" y="0">D1</text>` inside the door's own group.
   **Recommendation (Phase 6):** implement the beside-seat in
   `layout/floorplan/opening.rs` by reusing `fixtures::finish`'s mechanism —
   push the text child clear of the wall by `thickness/2 + consts::READOUT_GAP`
   on the swing side, one shared constant, no second mechanism. That is a code
   fix to match SPEC, not a SPEC change.
2. **"converted to drawing units at desugar" is inexact for fixture bodies.**
   `thickness:` and an opening's `width:` really do resolve at desugar (the
   `wall-thickness:` / `opening-width:` stamps), but a fixture body *cannot* —
   its `symbol:` is cascade-borne, so desugar stamps `unit-mm:` and
   `scale::mm_to_units` converts at layout (Phase 4's decision 3). The
   observable law is untouched and holds: a `|bed|` renders 1500 × 2000 px at
   `scale: 1; density: 1` under **both** `unit: m` and `unit: mm`, verified.
   **Recommendation:** if the user wants the sentence exact, drop the phase
   name — "converted to drawing units through the scope's `unit:`". Purely
   editorial; no behaviour is at stake.

Everything else checked out, including the clauses no earlier phase had to
exercise: `|page|` + `|title-block|` + `|note|` + `|hole|` all compile inside a
floorplan scope (the "every drawing-global mechanism stays welcome" law); a
`|wall|`'s smart label keeps the sketch's centred read; `:segment` dimensions
read the **centreline** (the showpiece's 9.6, not the outline's 9.8); the door
leaf is a line of length `width` from the hinge jamb and the arc has radius
`width`, sweeping leaf to closed; `swing: left` opens north on an east-running
wall (left of the pen's travel).

### Carry-over notes

**For Phase 6 (code audit):**
- **The opening label seat is a real SPEC-vs-code gap** (verdict 1 above) —
  the smallest confirmed defect on the branch. Fix it by *sharing* the fixture
  label seat, not by writing a second one; `fixtures::finish` and the opening's
  seat want one helper between them (AGENTS' no-parallel-implementations rule
  applies directly).
- `desugar/scale.rs` now carries **three** internal stamps —
  `wall-thickness:`, `opening-width:`, `unit-mm:` — pushed by the same walk and
  whitelisted in `validate::INTERNAL`. Two carry a resolved fallback, one
  carries an input. Worth a look as one mechanism with one naming scheme; the
  asymmetry is deliberate (Phase 4's decision 3) but undocumented at the
  stamps themselves.
- Phase 4's known limitation is still open and still logged: `door_symbol` /
  `stairs_chrome` count chrome from the **authored** `symbol:` / `steps:`, so a
  rule-borne value sizes the body right and generates nothing. Neither sample
  writes that form. If it is ever fixed, fix **both** producers together.
- Nothing in `src/` changed this phase, so the branch diff for the audit is
  exactly Phases 1–4 plus this phase's samples, tests and docs.

**For Phase 7 (visual polish):**
- **The extension-line question is the one systemic cosmetic issue.** An
  extension line starts at its anchor's midpoint, so a `:segment`-anchored
  dimension draws a line through the whole part; the halo breaks it wherever it
  crosses geometry, but a wall **opening is a hole**, so it shows through the
  gap and reads as a spurious sill or mullion. Both samples dodge it with
  `point()` corner stations, and SKILL.md now teaches that idiom — but SPEC
  §25's own `outer:west (-) outer:east` still hits it. The principled fix is to
  start an *edge/segment* anchor's extension line at the segment **end nearest
  the dimension's side** rather than its midpoint, which is also the drafting
  convention. That would move every drawing snapshot, so it needs the user's
  call — do not slip it in.
- Cosmetic itches deliberately left in the samples: the bathroom's toilet reads
  a little thin at 1 : 50 (tank 180 mm against a bowl that laps into it); the
  vanity slab under a `symbol: sink` shows three nested outlines; the catalog's
  right column is empty on three of six rows (the `schematic_parts.lini`
  precedent does the same, so it was left); the parts sheet's per-item smart
  labels sit at each body's own bottom, so a row's captions are not on one
  baseline.
- The `|floorplan|` title lowers to a `|footnote|` — small and muted at the
  sheet's bottom centre. On a plan sheet a title usually wants more presence;
  if that is worth changing it is a `|drawing|`-wide question, not a floorplan
  one.
- Render recipe used here, for repeatability:
  `lini --static samples/floorplan.lini -o x.svg && resvg x.svg x.png`
  (add `--theme dark`; `resvg --zoom 0.3` for the thumbnail,
  `--zoom 2.5` + a PIL crop for detail).

---

## Phase 6 — Branch code audit & quality pass

**Goal**: the whole `blueprint` branch diff — and any pre-existing code it
leans on — reads clean, professional, and organized: no parallel
implementations, no dead scaffolding, easy to find things, human-readable
(the user's explicit bar). Two stages, review then fix.

- [x] **Review stage** (adversarial, whole branch): `git diff main...HEAD`
      end to end, plus the blast radius (every pre-existing module the
      phases touched: `desugar/scale.rs`, `resolve/ir.rs`, `validate.rs`,
      `layout/mod.rs`, `ledger/*`). Hunt: correctness bugs (geometry edge
      cases, station arithmetic, inheritance precedence), duplicated logic
      that should share one mechanism (the house rule — including drift
      between floorplan and drawing/schematic twins), naming and module
      organization, over-long files, dead code, missed `AttrMap` /
      shared-helper reuse, snapshot gaps. Verify each finding before
      reporting it (read the code paths, run the case).
- [x] **Fix stage**: apply confirmed findings; improvements to pre-existing
      code are in scope where they serve one-mechanism/organization (no
      drive-by rewrites of unrelated subsystems). One commit per coherent
      cleanup theme, or one purposeful commit overall.
- [x] Full gate after: `cargo fmt` / `cargo test` / `cargo clippy` clean;
      every sample still byte-identical unless a fix intentionally changed
      output (then re-snapshot + re-look).

### Execution log

2026-08-29, one session. Baseline **1452 passed / 0 failed** → after **1458 /
0**; `cargo fmt --all`, `cargo clippy --all-targets` clean at every commit.
Review ran as a whole-branch read plus two adversarial sweeps (a geometry
edge-case hunt that actually ran ~120 cases, and a parallel-implementation
hunt); every finding below was re-verified against the real code path here
before it was acted on. Six commits, one per theme.

**Only one sample moved**: `floorplan_parts.lini`'s last DOORS cell wears a
`"D1"` schedule tag (the rendered proof of the label-seat fix). Every other
snapshot is byte-identical — the geometry fixes touch only cases neither
sample exercises. Both samples re-rendered and looked at (catalog at 2×, the
condo dark at thumbnail).

#### Findings ledger — fixed

| # | Sev | Finding | Verdict / where |
|---|---|---|---|
| 1 | **high** | **Panic.** `right(0):zero` names a run of no length; an opening on it hit `unit(…).expect("a named edge has length")` and aborted the compiler. | Confirmed. Fixed in `opening::straight_run`: a zero-length `Segment::Edge` **is** a point, so it takes the law and the message that already exist (`':zero' is a point`, `Y016`) — no new code, no new wording. |
| 2 | **high** | **`fillet()` / `chamfer()` silently voided every opening on the adjacent named run** — no gap, no chrome, no box, no diagnostic, `--strict` clean. The pen records `Segment::Edge` at the *theoretical* corner (so dimensions measure there) while the fold trims the drawn run back; `opening::locate` demanded matching endpoints and skipped the station. | Confirmed. Fixed: `locate` matches the **carrier** (same straight line, same travel, starting inside the named span) and returns how much the fillet trimmed; the station shifts into the drawn segment's own frame so `at:` still measures from the corner [SPEC 15.11], and `wall::cut` clamps a gap to the piece it lands on. |
| 3 | **high** | **A centreline jog ≤ thickness ∕ 2 punched a white notch out of the poché** (a stray diagonal inside a hollow wall, a white wedge in a hatched one). Threshold swept exactly: at h = 100 mm, jog 99 broken / 101 clean. `join`'s inside corner could trim neither element, and the straight-connector fallback doubles the face back over itself — even-odd then cancels the overlap. | Confirmed. Fixed in `wall/join.rs::consumed`: the offset has eaten the short element **whole** (the straight-run twin of SPEC 21's arc-under-thickness), so it is dropped and its neighbours step. `join` now reports whether `next` landed in the chain, and the seam join drops a consumed head the same way; a pair with no carrier crossing still connects straight. |
| 4 | med | **The assigned fix**: an opening's smart label seated *in* the gap, on the wall line, turned with the door — SPEC 15.11 says "its schedule tag **beside** the gap". | Confirmed. Fixed by **sharing** `fixtures::finish`'s seat, never copying it: `layout/floorplan/label.rs` is the one seat, with the fixture and the opening as its two callers. An opening clears the wall face by `thickness/2 + READOUT_GAP` on the face the leaf never sweeps. |
| 5 | med | **A rule-borne door `symbol: double` drew *half* a double door**, and a rule-borne `sliding` drew a hinged leaf **plus an arc** (SPEC: a slider has no arc). The count came from desugar's authored read, the shape from layout's cascade read — two readings of one decision. | Confirmed. Fixed at the mechanism: `opening::chrome` derives `double` from the **leaves it was given**, so the count has one source. A rule-borne symbol now draws the door desugar generated, consistently. |
| 6 | med | **An unknown door `symbol:` compiled and drew `single`.** Every other variant-bearing type — the fixtures, the schematic discretes — refuses one through `suggest::unknown_symbol`; `\|door\|` was the only consumer not calling it. | Confirmed. Fixed in `opening::symbol_law`, through that same shared message. |
| 7 | med | **`fn single(&Decl)` in three copies** — the branch turned one into three (`scale.rs`, `drawing.rs`, beside `validate.rs`'s) — plus `scale::find` re-implementing `ast::ident_of`'s last-wins lookup and `door_symbol` re-implementing `Decl::ident`. | Confirmed. Fixed: `Decl::single()` beside `Decl::ident()`, and `ident_of` splits into `decl_of` + `.ident()`. Every copy in `src/` is gone (`grep 'groups.as_slice()'` finds only the definition), including two pre-existing narrowed ones. |
| 8 | med | **The `chrome:` `(kind, index)` marker destructured verbatim in both floorplan fillers**, and the `break:` producer built the same marker inline beside desugar's own `indexed()`. | Confirmed. Fixed: `drawing::chrome::indexed` is the one reader (documented in the marker table beside `is_chrome`), `desugar::drawing::indexed` the one writer, used by the break producer too. |
| 9 | med | **Three internal stamps, two doing one job** (the plan's own audit target). `wall-thickness:` and `opening-width:` carried resolved sizes, `unit-mm:` an input — but an opening's 900/1200 mm depends on nothing but its type, exactly the case `unit-mm:` exists for. Both stamps' raw-mm last rungs also dropped millimetres into a drawing-unit slot, defended as unreachable (it is reachable — through a nested layout-owning container that seals the drawing scope). | Confirmed. Fixed: `opening-width:` folded into `unit-mm:`, leaving **two** stamps with the split written at the stamps ("can the walk resolve this?"). `floorplan::true_size(attrs, mm)` is the one reader for all three consumers, so the last rung converts instead of being accidentally right. |
| 10 | med | **Three walkers over `TEMPLATES`** — the branch added `derives_from` and `root_facts`' class walk beside `schema::template_chain`. | Confirmed. Fixed: `types::template_chain(name)` is the one walk; all three read it. |
| 11 | low | **A latent silent miss in the validation matrix**: the root-block check compared `Owner::Layout` by name while `Owner::Type` went through `layout_reads`. Safe only because no ledger row carries `Layout("drawing")` — the day one does, a `layout: floorplan` root would silently reject what a drawing root accepts, with nothing failing. | Confirmed. Both arms now ask the one predicate. |
| 12 | low | Stacked label lines had **no gap**, while the schematic readout stacks by `READOUT_STACK` and calls itself the one seat table. | Confirmed. `label::seat` stacks by the same constant. |
| 13 | low | Dead / over-wide surface: `is_floorplan_layout` `pub` with no caller outside its file; `layout_reads` and `is_floorplan` `pub` with one each; `validate::INTERNAL` re-spelling the stamp consts as literals; `floorplan::is_floorplan` a wrapper where its schematic sibling re-exports. | Confirmed. All narrowed / re-exported / pointed at the consts. |
| 14 | low | Readability: `match part { true =>, false => }`; the openings' "carries no bundle" comment reading as the fixtures' own (nothing separated them); the synthetic `layout: drawing` link-dress probe unexplained for a dialect. | Confirmed. All three reworded / re-idiomed. |

#### Findings ledger — confirmed, deliberately not fixed

| # | Sev | Finding | Why not, and what it needs |
|---|---|---|---|
| A | med | **`mirror:` on a `\|wall\|` renders a degenerate band.** The fuse leaves a doubled-back centreline, so the offset traces its band twice in one subpath and even-odd cancels it (`M 7 -1 … L 10 1 L 7 1 Z`, self-overlapping). | Needs a design call: what does mirroring a *centreline* mean for a wall — a symmetric wall, or the fused sketch it is today? Not a code bug to patch blind. **User ruling.** |
| B | med | **A negative or zero opening `width:`** makes degenerate geometry — overlapping jamb pieces and an SVG arc with negative radii. | `width:` / `height:` are unvalidated **language-wide** (`\|box\| { width: -50 }` is equally accepted, no error, `--strict` clean). A floorplan-only positivity rule would be the special case, not the fix. **SPEC 17 question for the user.** |
| C | med | **`scale:` does not inherit into a nested drawing scope.** `\|floorplan\| { scale: 0.02 } [ \|floorplan\| [ … ] ]` folds the inner scope at ratio 1 (`px-per-unit: 4`, not `0.2`), so the inner wall renders 20× oversize. SPEC 15.1: "nearest ancestor wins". Reproduces in a plain `\|drawing\|` inside a `\|drawing\|`. | **Pre-existing drawing-engine behaviour**, not the dialect's — `ScaleCtx` carries `unit_mm` and `thickness` down but not the ratio. Flagged because `scale.rs` was in the blast radius; fixing it moves drawing snapshots and wants its own pass. |
| D | med | **The chrome mechanism's boundary**, the plan's other audit target: chrome is counted at desugar from **authored** decls while its geometry reads the cascade at layout. A rule-borne `steps:` sizes a flight correctly and generates **no treads and no arrow** — a silent blank rectangle. | **Not a floorplan flaw**: verified pre-existing and systemic — a rule-borne `pattern: radial()` places its copies and loses its `\|pitch-circle\|` in exactly the same way. So the limit belongs to the mechanism, and it is now **documented once** in `drawing/chrome.rs` (with the rule a filler must follow — never re-derive a count from an attribute, which is what closed finding 5), not at each of the twelve producers. Closing it means teaching desugar the cascade or letting layout mint nodes; both are design calls beyond an audit. |
| E | low | An exact hairpin (`right(10) left(10)`) traces the band twice in one subpath — even-odd renders nothing at all. | The degenerate root of A. Same ruling. |
| F | low | A `\|wall\|` inside a nested layout-owning container inside a floorplan scope passes the vocabulary gate but the scale fold does not reach it, so it draws in raw units. | Consistent with the sealed-scope law (a nested `\|row\|` in a plain drawing arranges the same way), and finding 9's shared reader now converts correctly there (`unit:` defaults to mm). Worth a gate one day; not a regression. |

#### Rejected (checked, not defects)

- *"The stairs arrow lands **on** the far edge, not past the last tread."* The
  `steps − 1` risers stop one pitch short of the edge, so the arrow **does**
  run past the last tread. SPEC satisfied.
- *"A slider's panels abut rather than overlap."* They sit on **opposite
  faces** and read as one set passing the other — Phase 3's ruling against
  both reference plans stands.
- *"`FIXTURES` is restated in the ledger tables."* Declarative `match` / `&[…]`
  data that cannot take a runtime slice, and `DISCRETES` has the identical
  shape pre-existing. House precedent, not an oversight.
- *"`\|dining\| { symbol: round; width: 3 }` stretches ⌀1200 into an ellipse."*
  That is Phase 4's stated stretch law for every family; a per-family
  exception would be the special-casing AGENTS.md forbids.

#### Housekeeping verified

`crates/lini-wasm/pkg` was rebuilt (`cargo xtask wasm`) after the sample
change — the parity test fails misleadingly otherwise. `cargo xtask gen-schema`
/ `gen-grammars` produce **no diff** (no `PROPERTIES` or template row moved).
No file the branch touches is near the ~500 LOC line: `opening.rs` 380,
`scale.rs` 353, `wall/mod.rs` 294, `wall/join.rs` 283, `fixtures/mod.rs` 218,
`label.rs` 30. `desugar/scale.rs` doubled over the branch but stays one
concept — *what the walk that knows `unit:` stamps* — and splitting the
stamps away from that walk would read worse, so it was left whole and
documented instead.

### Carry-over notes

**For Phase 7 (visual polish):**
- **Nothing the fixes changed needs re-judging** — every sample snapshot is
  byte-identical except `floorplan_parts.lini`'s new `"D1"` tag in the last
  DOORS cell (looked at, light; the sheet's composition is unmoved).
- **The showpiece can now carry a door schedule.** Phase 5 dropped a trial
  `"D1"` off the entry door because it seated in the gap; that is fixed, so
  `D1` / `W1` tags are available if the plan wants them. They seat on the face
  away from the swing and rotate with the wall — worth a look before adopting
  on a north/south wall.
- The Phase-5 cosmetic itches all stand, unchanged: the toilet reads thin at
  1 : 50; the vanity slab under a `symbol: sink` shows three nested outlines;
  the parts sheet's per-item smart labels sit at each body's own bottom, so a
  row's captions are not on one baseline; the catalog's right column is empty
  on two of six rows (the DOORS row is now full).
- The **extension-line-origin** convention was left alone as instructed — the
  samples keep the `point()` corner idiom.
- `|dining| { symbol: round }` under a `width:` override draws an ellipse
  (finding "rejected", above): if that ever reads badly it is a SPEC question
  about the stretch law, not a family patch.

**For whoever rules on the deferred findings (A–D above):** each is written up
with its exact repro in the ledger; A/B/E want the user, C wants its own pass
over the drawing engine's `ScaleCtx`, D wants a decision about where a chrome
count may be read.

---

## Phase 7 — Visual polish loop (the last pass)

**Goal**: the rendered results are *pretty* — iterate on the two floorplan
samples (and any render the earlier phases flagged) by looking, not by
guessing. Cosmetics last, per AGENTS.md.

- [ ] Render every floorplan sample + the §25 SPEC block via the CLI
      (`--static` for resvg), PNG at 2× and at thumbnail size, light AND
      dark; **read every PNG**. (`lini serve` is the user-facing playground;
      the render-and-look loop is the agent equivalent.)
- [ ] Judge against `plans/refs-floorplan/20sw-b1.webp` and the pretty bar:
      line-weight contrast (poché vs thin chrome), swing arcs, fixture
      proportions at 1:50, dimension row spacing, label collisions, dark-mode
      legibility. Fix in the sample source first; fix engine constants only
      where the flaw is systemic (log it; SPEC constants need the user).
- [ ] Iterate render → look → adjust until nothing jars; final side-by-side
      screenshot set in the execution log (paths), tests green.

### Execution log
### Carry-over notes

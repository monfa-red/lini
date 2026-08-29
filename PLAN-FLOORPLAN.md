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

Branch: `blueprint`. Do not push; the user merges.

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
- `tests/spec_blocks.rs` carries a ledger row for SPEC block **#57** (the
  §25 floorplan example) — Phase 5 removes it and the block must compile.

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

- [ ] The one mm→units conversion function; `thickness:` resolved through
      inheritance (scope → wall), default 200 mm; `|partition|` 100 mm.
- [ ] The offset walk (`layout/floorplan/wall.rs`, new module): centreline
      polyline/arc path → closed outline at ±t/2, mitred at joins, flat caps
      on open ends, `close()` seams mitred like any corner; arcs offset to
      concentric arcs (radius ± t/2); reject `curve()` in a wall (error).
      Property-test the geometry where practical (e.g. outline area ≈
      centreline length × t for gentle paths; offset segments parallel at
      distance t/2).
- [ ] Fill/paint: solid `--stroke-dark`, `stroke: none` (the template row);
      verify `fill: --bg` (hollow) and `fill: hatch(45)` read correctly.
- [ ] Geometry bbox = outline bbox; anchors: `:segment`s stay centreline
      (dimensions read centreline stations — the architectural convention),
      bbox sides/corners read the outline. Dims, leaders, mates against walls
      under test.
- [ ] Overlap composition: two solid walls crossing/meeting read seamless
      (visual check); a wall respects source-order painting.
- [ ] Snapshot tests + **visual PNG check** (resvg): an L-corner, a T-meet of
      two walls, a closed rectangle loop, an arc wall segment, open-ended run.

### Execution log
### Carry-over notes

---

## Phase 3 — Openings: doors & windows

**Goal**: SPEC 15.11's openings, fully — gaps, chrome, poses, errors, dims.

- [ ] Station model: `on:` resolves a straight segment of the host wall
      (unknown → suggestion error; arc → error); `at:` + `width:` (mm default
      via conversion; 900 door / 1200 window) validated against segment
      length; overlapping openings on one segment error.
- [ ] Gap cutting in the outline builder (Phase 2's module): both wall faces
      opened, jambs capped flat.
- [ ] Door chrome (generated children, `--stroke-light` weight 1): leaf line
      from the hinge jamb + quarter swing arc, `hinge: start|end` ×
      `swing: left|right` (walker's rule along the segment's draw direction);
      `symbol: single` (default) / `double` (two leaves, two arcs) /
      `sliding` (offset panel lines, no arc).
- [ ] Window chrome: two sill lines at the thickness's thirds (SPEC 15.11);
      compare against `plans/refs-floorplan/floor-plan-symbols-1.jpg` (the
      reference draws rect + centre line — if that reads better at 1:50,
      propose the SPEC tweak in the log rather than silently diverging).
- [ ] Openings as geometry: an id'd opening anchors dimensions at its centre
      (`outer:west (-) entry (-) outer:east` renders the location chain).
- [ ] Snapshot + **visual PNG check**: all four hinge/swing poses, each door
      symbol, a window, a door at a segment end, two openings on one wall —
      light and dark.

### Execution log
### Carry-over notes

---

## Phase 4 — Fixtures: the furniture library

**Goal**: the six fixture types render true-size with their `symbol:`
variants; `|stairs|` generates.

- [ ] **Read the reference sheet first** (`plans/refs-floorplan/`, all five
      images — the Read tool renders them); author every symbol against it.
- [ ] Symbol path library on the mm grid (SPEC 15.11's table is the size
      law): bed double/single (mattress + pillow read), sofa three/two/corner
      (seat + back + arms), dining six/four/round (table **with chairs**),
      bath tub/shower/toilet/sink, appliance stove (4 burners) / fridge
      (double-door tick) / washer / dishwasher (door line + circle reads).
      Author for the modern-condo read; keep line work `stroke-width: 1`.
- [ ] Sizing: intrinsic mm through the conversion function; `width` /
      `height` floors stretch the symbol (like `|image| fit: stretch` — pick
      and log the exact scaling rule per family; a toilet should not distort
      absurdly — if a family must keep aspect, log it and error or letterbox
      consistently).
- [ ] `|stairs|`: `steps: N` (required, ≥ 2) generates treads at 250 mm pitch
      × 900 mm width + the direction arrow; `width`/`height` override.
- [ ] Unknown `symbol:` errors with a suggestion (shared machinery).
- [ ] Snapshot + **visual PNG check**: one sheet laying out every fixture ×
      every variant at `unit: m`, verified by eye at 1:50.

### Execution log
### Carry-over notes

---

## Phase 5 — Showroom, canon & hardening

**Goal**: the feature is a shipped family — sample, SPEC example compiling,
deferred slots pinned, everything green.

- [ ] `samples/floorplan.lini`: the studio flat (walls + partition + openings
      + fixtures + room text + dimension chains), matching §25's example in
      spirit; render, **look at it** (light + dark), iterate until it reads
      like a real-estate plan.
- [ ] Remove the `tests/spec_blocks.rs` #57 ledger row — the §25 block must
      compile as written.
- [ ] `tests/deferred.rs`: one pinned test per reachable SPEC 24 Floorplans
      slot (computed areas — n/a if no syntax is reachable; curved-segment
      opening → the error; others n/a — mirror the file's conventions).
- [ ] Conformance snapshot for the sample; `lini fmt` canon over it
      (`fmt --check` clean); desugar fixed point over it.
- [ ] Docs sweep: README.md and SKILL.md layout enumerations gain
      `floorplan`; SKILL.md gains a short floorplan authoring section beside
      its schematic one (walls → openings → fixtures → dims, one example).
- [ ] `cargo fmt` / `cargo test` / `cargo clippy` clean; sweep this plan's
      checkboxes; write the final execution log.
- [ ] Re-read SPEC 15.11 top to bottom against the built behaviour — fix
      either (SPEC wins on intent; if the implementation taught us better,
      propose the SPEC edit to the user in the log).

### Execution log
### Carry-over notes

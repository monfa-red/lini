# GDT-alpha4 — drafting symbols & annotation composition

The alpha.4 round, planned 2026-07-18 against the shipped combined
alpha.2 + alpha.3 (`CHART-DRAW-alpha23.md`). Sources: ROADMAP 3.5 (second
half), PLAN-V1's alpha.4 contract, and the design settlements in the
ledger below. **One plan, one tag**: the ladder renumbered when alpha.3
shipped inside the `1.0.0-alpha.2` release — this round releases as
**`1.0.0-alpha.3`** when Stage 4 closes. **Stage 0 is the SPEC pass; the
amended SPEC is the contract**; this file holds the settled decisions,
the build order, and the execution log. The quality bar is ROADMAP §5,
verbatim. Everything is **additive** — no existing sample or snapshot
changes except where a stage names it.

Scope (ROADMAP 3.5, second half): the shared **drafting-glyph registry**
(the icon machinery generalized, with a natural-units sizing path);
`|surface-finish|`; `|feature-control|` + `|control|` rows (the canonical
characteristic set, `tol`/`zone`/`material`/`datums`/`modifiers`,
composite frames, `datums:` validated against the scope's collected
letters); `||` generalized to **annotation seating**; **annotation nodes
inside a drawing link's `[ ]`** (the round's one deep change — the label
path is text-typed end-to-end today) plus the `|datum|` node the
annotation-node seam enables. `format:` on dimensions already shipped in
alpha.3's Stage 8 — it is not this round's work.

Stages are sized for one session at ~60 % of a context window; sub-agents
per AGENTS.md (always with an explicit `model`). At each stage's end:
fmt/test/clippy clean, a **Log** line here.

---

## Decisions ledger (settled in design review — do not relitigate)

1. **One plan, one tag.** The ladder's alpha.4 row stands; versions
   renumbered — this round tags **`1.0.0-alpha.3`** when Stage 4 closes;
   pushing stays with Abbas.
2. **The glyph registry sizes in natural units.** The icon machinery
   (`lookup` / role groups / suggest) generalizes to a drafting-glyph set
   drawn as paths — but **never fit-to-box**: a glyph's height follows
   the annotation `font-size`, its line weight the statement's
   `stroke-width`, so symbols read at dimension-linework weight beside
   every value at every view scale. The set serves the templates
   internally — drafting glyphs are not `|icon|` symbols.
3. **`|surface-finish|`** — ISO 1302: smart label = the textual
   indication (`"Ra 1.6"`), riding the symbol's long leg; `symbol: basic
   | machined | prohibited` (default `basic`; bar = removal required,
   circle = removal prohibited). Sheet content (`scale: 1`),
   drawing-scope only. The leader form (`body:seat <- sf`) wires the
   **same placed node** — one lowering, two attachments.
4. **The characteristic set is ISO 1101's fourteen**: `straightness`,
   `flatness`, `circularity`, `cylindricity`, `profile-line`,
   `profile-surface`, `angularity`, `perpendicularity`, `parallelism`,
   `position`, `concentricity`, `symmetry`, `circular-runout`,
   `total-runout`. ASME Y14.5-2018 dropped concentricity and symmetry;
   lini's drafting lineage is ISO (129, 5457, 7200, 6410, 1302) and both
   validate. No obsolete symbols beyond the set.
5. **`|feature-control|` with `|control|` rows.** One row = the frame's
   own properties (the common case); several = `|control|` children,
   one row each; mixing the two forms errors. The **smart label names
   the characteristic** (the frame's in one-row form, each row's
   otherwise; longhand `characteristic:`). Row properties: `tol:` the
   zone width (required, number > 0); `zone: diameter | spherical` (⌀ /
   S⌀, axial-zone characteristics only); `material: maximum | least`
   (Ⓜ / Ⓛ, feature-of-size controls only); `datums: A, B maximum, C`
   (primary → tertiary, ≤ 3, per-datum modifiers); `modifiers:
   projected N | free-state | tangent-plane` (ordered, Ⓟ Ⓕ Ⓣ). An
   invalid combination — the validity table's forbidden/required cells,
   an unknown characteristic — is an **error**, never a
   plausible-looking wrong frame.
6. **`datums:` validates against the scope's letters** — the identity
   set alpha.3's Stage 6 collects at resolve (`>-`, now also `|datum|`);
   an unknown reference errors naming the declared set.
7. **Composite frames merge the shared symbol.** Adjacent `|control|`
   rows with one characteristic merge its symbol compartment (the
   composite frame); differing rows stack as a combined frame; rows
   render in source order.
8. **`||` with a sheet-content end is a seat.** Geometry↔geometry keeps
   mate/ground semantics untouched; annotation↔geometry **always moves
   the annotation**, runs **after mates**, outside the grounding graph
   (never grounds, never over-constrains; operand order irrelevant); an
   annotation seats once; annotation↔annotation errors. The old
   mate-on-sheet-content error dissolves into the seat semantics. The
   **target must supply a directed side/edge** — a point target errors.
9. **A seat places, a mate aligns.** The annotation's **seat anchor**
   lands on the target anchor's representative point — flush contact,
   both axes (the annotation had no position worth keeping). `gap:`
   offsets along the face normal (mate's signed law); `rotate:` turns
   the annotation **before** the seat (the rotated anchor aligns);
   `translate:` nudges **after** — the lateral slide. Default seat
   anchors are **type-defined**: `|surface-finish|` seats its **vee
   tip** on the face; everything else (frames, `|datum|`, notes,
   balloons, bundles) seats its **facing side** — the bbox side whose
   outward opposes the target's normal, read after `rotate:`. An
   explicit anchor on the annotation endpoint overrides.
10. **A bundle seats as one and reports one extent.** A wrapper
    (`|column|` of finish + frame) is sheet content: it seats whole and
    registers **one painted bbox** — the union of its lowered children —
    with the dimension row packer before dims pack, through the same
    painted-bounds channel the datum frame already uses (the
    `datum-frame` special case generalizes to one annotation-obstacle
    class). Seated annotations are obstacles, never geometry extent.
11. **Annotation nodes ride a drawing link's `[ ]`.** A dimension or
    leader may carry `|feature-control|` / `|surface-finish|` /
    `|datum|` nodes beside its text labels: each stacks at the
    statement's **text seat** (under the value / callout lines, source
    order) and registers its painted bounds as a packing obstacle.
    Strings keep their label semantics (replace / follows). **Core
    routed links stay text-only** — the parser stays scope-blind
    (`label_block` widens to `{ text | node }`), resolve errors outside
    a drawing scope. This is the round's deep change: the label path is
    `TextNode` → `ResolvedText` end-to-end today.
12. **`|datum|` is the framed letter as a node** — smart label the
    letter, joining the scope's identity set exactly as `>-` does
    (duplicates error across both forms); in a dimension's `[ ]` it
    states the feature-of-size axis datum. One frame anatomy — the
    `>-` datum box and the `|datum|` node share their lowering.
13. **This round only.** alpha.5 explodes into its own doc at entry;
    nothing here reserves syntax for it.

---

## Stages

### Stage 0 — SPEC amendment: drafting symbols & annotation composition

Write all law before any code. The SPEC alone must suffice to implement
Stages 1–4.

- [x] SPEC 15.5 → **Mates & seating**: the `||` semantics table
  (mate / seat / error), the seat law (decisions 8–9 — after mates,
  outside grounding, directed target, flush + `gap:`, rotate-before /
  translate-after, the seat-anchor defaults table, one seat per
  annotation, bundles whole); the anchor rename swept everywhere.
- [x] New **SPEC 15.9 — Drafting symbols & annotation composition**
  (Lowering renumbers to 15.10): the natural-units glyph law
  (decision 2); `|surface-finish|` (decision 3 — variants table,
  indication label, seat + leader forms); `|feature-control|` /
  `|control|` (decisions 4–7 — the characteristic table with the
  ISO/ASME note, row properties, the validity table, composite
  merging); `|datum|` (decision 12); annotation nodes in `[ ]`
  (decision 11 — text-seat stacking, obstacles, core links text-only).
- [x] Surrounding drawing law squared: the §15 child-role and
  drawing-scope tables gain the annotation types and the seat reading;
  15.1's sheet-chrome (`scale: 1`) list; 15.6's packer sentence (seated
  and carried annotations register painted bounds before dims pack);
  15.7's datum bullet (validated now; the `|datum|` node form).
- [x] SPEC 8: four template rows (`|surface-finish|`,
  `|feature-control|`, `|control|`, `|datum|`). SPEC 16: `symbol`
  gains its finish owner (homonym noted), `tol` widens to a control
  row's zone width, new `characteristic` / `zone` / `material` /
  `datums` / `modifiers` rows.
- [x] SPEC 21: `label_block = "[" { text | node } "]"` + the
  drawing-extends prose; SPEC 9's "a link's `[ ]` holds only labels"
  sentence squared with a 15.9 pointer.
- [x] SPEC 20 rows: unknown characteristic (did-you-mean),
  characteristic set twice, missing `tol:`, `datums:` on a form
  control, missing required datum, unknown datum reference (naming the
  declared set), > 3 datums, `zone:` / `material:` misuse, unknown
  modifier, mixed one-row + `|control|` forms, `|control|` outside a
  frame, annotation type outside a drawing, point-target seat, seat
  with no geometry end (replacing the mate-on-sheet-content row),
  annotation seated twice, node label outside a drawing scope.
- [x] SPEC 23 prunes: the GD&T deferred bullet dies.
- [x] Sync: ROADMAP's ladder row points here (and drops the
  already-shipped `format:` item); PLAN-V1's alpha.4 contract gains its
  round-entered note; code comments citing SPEC 15.9 follow the
  Lowering renumber.

Acceptance: SPEC alone sufficient for Stages 1–4; anchors intact;
examples use this round's settled syntax; `cargo test` untouched.
**Log:** 2026-07-18 — **done**, one commit (SPEC + ROADMAP/PLAN-V1 + this
doc). Settled in the pass: the new section lands as **15.9** with only
Lowering renumbering to 15.10 — assemblies keep 15.8, so the ~90 code
comments citing it stay true (three 15.9 citations swept); seat contact
is **both axes** (a seat places; a mate aligns one axis); a seat's
`gap:` reads along the target's outward normal, positive = daylight;
the finish vee's indication rides the long leg; the old
mate-on-sheet-content error row dissolved into the seat semantics.
Anchor sweep: 519 refs, zero broken; 995 tests untouched.

### Stage 1 — the drafting-glyph registry & `|surface-finish|`

- [x] The glyph registry (beside `src/icon/` — the lookup/suggest shape
  reused): path data for the fourteen characteristics, the modifier
  circles (Ⓜ Ⓛ Ⓕ Ⓣ Ⓟ), and the three finish vees; **natural-units
  emission** — height from the annotation `font-size`, stroke from the
  statement's `stroke-width` (decision 2), never fit-to-box.
- [x] `|surface-finish|` (decision 3): the template + drawing-scope
  gate; smart label = the indication (longhand none — the label is the
  text); `symbol:` variant validation; lowering to vee + indication at
  the text seat; ordinary placement (`translate:`) and the two-ended
  leader form both render the one node.
- [x] Tests: registry units (every glyph resolves, natural-units
  scaling), snapshots (three variants, a leader-wired finish), the
  outside-drawing and unknown-variant errors; PNG light + dark.

Acceptance: glyph line weight matches dimension linework at 1:1 and at
a scaled view; the three variants read correctly; errors fire.
**Log:** 2026-07-18 — **done**, one commit; 1005 tests, clippy clean.
The registry is `src/glyph/` (a static sorted table reusing
`icon::Role`, fragments on a 100-unit grid, vee anatomy exported as
constants); emission generalizes the icon machinery at render — a
`drafting-glyph`-classed `Icon` node reuses `emit_role_group` with a
**height-derived** scale (`bbox.h / GRID`) and a counter-scaled stroke,
so the linework weighs exactly the statement's `stroke-width`. The
lowering (`layout/drawing/symbols.rs`) puts the vee **tip at the node's
local origin** (Stage 3's seat anchor for free); the shell is a
path-less `Path` node — identity and bbox with no drawn box. Placed
symbols register as packer obstacles through a `Rows::obstruct` painted
box (the datum-frame channel; Stage 3 folds both into the one
annotation-obstacle class). Snapshots: the new `drawing_gdt.lini`
conformance snap (three vees + a leader-wired finish), one validation
snap re-blessed (`symbol`'s misuse message now names both owners —
the SPEC 16 homonym). PNGs read light + dark at zoom 2 + 4: vee
proportions per ISO 1302 (bar and crotch circle correct), indication
riding the long leg, glyph weight matching dimension linework at 1:1
and at a `scale: 2` view.

### Stage 2 — `|feature-control|`, `|control|` & `|datum|`

- [ ] The frame model (decisions 5, 7): one-row sugar vs `|control|`
  rows (mixing errors), characteristic from label or `characteristic:`
  (twice errors), compartment layout at font-derived sizes — symbol,
  zone-prefixed tolerance + material + modifiers, datum compartments
  with per-datum modifiers; composite merging of a shared symbol
  compartment.
- [ ] Validation (decisions 4–6): the validity table enforced with
  did-you-means; `datums:` checked against the resolve-collected letter
  set (surfaced from resolve to layout), unknown references naming the
  declared letters; every new SPEC 20 row wired.
- [ ] `|datum|` (decision 12): the framed-letter node sharing the `>-`
  box's lowering; its letter joins the identity collection (duplicates
  error across both forms).
- [ ] Tests: a snapshot matrix (single row, composite, modifiers,
  datum'd), one test per error row; PNG light + dark.

Acceptance: frames render semantically valid or error — never
plausible-wrong; unknown datum references name the scope's letters.
**Log:**

### Stage 3 — `||` annotation seating

- [ ] Resolve/layout split (decision 8): a `||` statement with a
  sheet-content end classifies as a **seat** — collected apart from
  mates, run after them, outside the grounding graph; operand order
  irrelevant; both-annotation and double-seat errors; the
  mate-on-sheet-content error replaced by the seat semantics.
- [ ] The seat itself (decision 9): directed-target check; type-defined
  default seat anchors (vee tip / facing side) with explicit-anchor
  override; flush contact both axes; `gap:` along the normal;
  rotate-before / translate-after (the mate law reused, not copied).
- [ ] The packer channel (decision 10): seated annotations — a bundle
  as **one** union bbox — register painted bounds before dims pack; the
  `datum-frame` class generalizes to the one annotation-obstacle class.
- [ ] Tests: seats on all four sides + a named edge, a rotated seat, a
  `gap:` seat, a bundle seat with a dim packing past it (the no-overlap
  oracle), symmetric operand order, every error; PNG light + dark.

Acceptance: a dim row never overlaps a seated bundle; `a || sf` ≡
`sf || a`; geometry mates byte-identical to alpha.3.
**Log:**

### Stage 4 — annotation nodes in link `[ ]`; the sample; alpha.4 closes

- [ ] The deep widening (decision 11): `label_block` accepts nodes
  (parser stays scope-blind); the link content path carries annotation
  nodes beside `ResolvedText` end-to-end (AST → desugar → resolve →
  layout); a node label outside a drawing scope errors; `fmt` preserves
  node labels in `[ ]`.
- [ ] Drawing lowering: carried nodes stack at the statement's text
  seat in source order, ride the row, and register as packing
  obstacles; a `|datum|` in a dimension's `[ ]` states the axis datum
  (decision 12).
- [ ] `samples/drawing_gdt.lini` — the genuinely new cluster: the fully
  toleranced part — `>-` and `|datum|` datums, finish symbols seated
  and leader-wired, a seated frame, a frame carried on a dimension, a
  composite frame; snapshots.
- [ ] The round-closing visual pass (ROADMAP §5): every drawing sample
  at 1:1 + a detail scale, light + dark, screen + print size; ladder
  row confirmed; bump `1.0.0-alpha.3`, tag (push deferred to Abbas).

Acceptance: carried nodes register as obstacles (the no-overlap oracle
green across all drawing samples); a core link carrying a node errors;
the sample reads in both themes; the tag is cut.
**Log:**

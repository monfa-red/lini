# Drawing-subsystem audit

Read-only audit of `src/layout/drawing/**`, `src/glyph/`, `src/render/knockout.rs`,
and the image/halo emission in `src/render/primitives.rs`, after alpha rounds 2–5
(`plans/CHART-DRAW-alpha23.md`, `GDT-alpha4.md`, `SHEET-alpha5.md`). Audited against
`AGENTS.md`: *one mechanism per problem / no parallel implementations*, *one concept
per file / split past ~500 LOC*, *robust fixes over patches*, *reused style rides a
rule*. Every finding is cited to `file:line` in the tree as it stands.

Nothing was changed. LOC of the source files (tests excluded where a `#[cfg(test)]`
module is present): `dims.rs` 616 (all source), `leaders.rs` 558 (all source),
`frames.rs` 567 source + 378 tests, `anchors.rs` 496, `threads.rs` 434 source,
`corner.rs` 431, `symbols.rs` 417 source.

---

## Ranked findings

### 1. Aligned dims bypass the row packer and re-derive its band — parallel impl + patch
**Law:** one mechanism per problem / no parallel implementations. **Effort:** L. **Risk:** M.

`dims::linear` splits seating in two: a horizontal/vertical dim gets `Seat::Row(side)`
and packs through `Rows::seat`, but an *aligned* dim gets `Seat::Line(c)` with the
cross coordinate computed by `aligned_line_c` (`dims.rs:409-416`). That function
open-codes the packer's band:

- `na.max(nb) + clearance + paint.fs + 2.0` (text side) reduplicates `Band::neg = fs + 2.0`
  (`annotate/rows.rs:35`),
- `na.min(nb) - clearance - ARROW_HALF * paint.sw` (arrow side) reduplicates
  `Band::pos = arrow` (`rows.rs:39-42`).

Two consequences beyond the copy: an aligned dim **never registers with `Rows`**, so it
cannot clear a callout/symbol obstacle or an earlier row, and later rows cannot clear it.
The Stage-5 log already flags this as a deviation ("aligned dims don't row-pack",
`plans/CHART-DRAW-alpha23.md:366-367`). SPEC 15.6 defines one seating law
(*"a row stands `clearance` off everything already painted on its side"*); an aligned
dim is that same law along the frame's own cross axis.

**Fix:** generalize `Rows`/`Band` to seat along an arbitrary `Frame` (the packer is
today hard-keyed to `Side` in `line_at`/`band_box`/`past`), and route aligned dims
through it so `Band`, obstacle registration, and inter-row clearance are computed once.
Collapses `aligned_away`/`aligned_line_c` into the packer. The L is real: the packer's
world-coordinate math assumes axis-aligned sides.

---

### 2. Stroke-excluded "geometry box" is implemented twice as a fn, plus 7 inline sites
**Law:** no parallel implementations / missing shared helper. **Effort:** M. **Risk:** low.

The stroke-excluded drawn box (`node.bbox.inflate(-stroke_width/2)`) is a named function
in two files that never see each other:
- `outline::geometry_box` — `outline.rs:131-134`,
- `Anchor::geometry_box` — `anchors.rs:256-260` (identical, but on `feature()` for patterns).

The same `let half = …("stroke-width")…/2.0; …inflate(half|-half)` expression is
open-coded seven more times: `edges.rs:168`, `chrome.rs:95`, `chrome.rs:135`,
`breaks/mod.rs:162`, `engine/mod.rs:66`, plus the two test sites. A fix to the
stroke-exclusion rule (e.g. a node type whose stroke isn't centred) has to be found in
nine places.

**Fix:** one helper — `fn stroke_excluded_box(n: &PlacedNode) -> Bbox` (or a
`PlacedNode::geometry_box` method) — and have `Anchor::geometry_box` call it on
`feature()`. The layout-time inflate-for-paint sites (`chrome`, `edges`, `engine`) share
the `half` derivation too.

---

### 3. No `Bbox::center()` — the centre expression is open-coded 13× in the drawing tree
**Law:** missing shared helper. **Effort:** S. **Risk:** low.

`Bbox` (`src/layout/ir.rs:217`) has `centered`, `w`, `h`, `union`, `from_points`,
`shifted`, `extent_of` — but no `center`. So
`((g.min_x + g.max_x)/2.0, (g.min_y + g.max_y)/2.0)` is written by hand at
`anchors.rs:266`, `anchors.rs:471`, `outline.rs:206`, `round.rs:133`, `round.rs:191`,
`round.rs:367`, `dims.rs:137-140`, `engine/mod.rs:55-56`, `symbols.rs:198,213`
(the x-centre half), and two test sites — 13 in all.

**Fix:** add `Bbox::center(&self) -> (f64, f64)` and replace the sites. Pure mechanical,
byte-identical.

---

### 4. ISO "text riding a line" placement re-derived in `round::diametral`
**Law:** no parallel implementations. **Effort:** M. **Risk:** low-M.

The canonical placement — turn text to `iso_text_angle(dir)`, lift `fs/2 + 2` toward the
"above" side — lives in `dims::Frame` + `dims::value_texts` (`dims.rs:502-520`, lift at
508, angle via `Frame`). `round::diametral` re-computes the whole thing by hand:
`iso_text_angle` + `to_radians().sin_cos()` + `up = (ts,-tc)` + `lift = fs/2+2`
(`round.rs:298-347`). `angle::arc_between` places its value along a bisector with its own
`r + fs/2 + 6` offset (`angle.rs:134-138`) — related but genuinely a different anatomy.

**Fix:** hoist the "text seat above a line at direction `dir`" into one helper (a small
constructor on `Frame`, e.g. `Frame::text_seat(centre, dir, fs) -> (P, angle)`), and have
`diametral` call it. Watch for snapshot churn — the two paths currently agree only by
hand.

---

### 5. `dims.rs` (616) and `leaders.rs` (558) exceed the ~500-LOC split line
**Law:** one concept per file, split past ~500 LOC. **Effort:** M. **Risk:** low.

- **`dims.rs`** carries three concepts: the measure `Frame` (u/n/pt/cross — pure geometry,
  `dims.rs:190-240`), the row-vs-line `Seat`/`Plan`/`stack_side`/`corner_pull` packing
  glue, and the aligned-dim seat (`aligned_away`/`aligned_line_c`). Seam: pull `Frame`
  into `geometry.rs` (it is plane geometry, not dim policy) and the aligned-seat pair into
  wherever finding #1 lands.
- **`leaders.rs`** mixes the leader skeleton (`leader_line`, `clear_along`, `carried_push`),
  the callout/measured/arrows dispatch, and the datum-triangle + framed-datum-box drawing
  (`leg`, `datum_box`, `fan_tip`). Seam: `leaders/skeleton.rs` (line + push math) vs
  `leaders/mod.rs` (the dispatchers).
- `frames.rs` is 567 source but genuinely one concept (the ISO-1101 validity table is
  indivisible); leave it. The 945 total is test-dominated.

---

### 6. The "is this a drawn-geometry child" predicate is scattered across six variants
**Law:** one mechanism per problem (drift risk). **Effort:** M. **Risk:** M (silent drift).

Deciding which children count as real geometry (vs sheet content, pinned overlays, chrome,
annotation ink) is answered independently in:
- `engine::lay_out` — `!sheet_node && !is_pinned && !is_chrome` (`engine/mod.rs:111-117`),
- `annotate::geometry_extent` — `!sheet_node && !is_pinned` (`annotate/mod.rs:198-201`),
- `annotate::drawn_geometry` (test) — adds a dim/ext/marker exclusion (`mod.rs:209-217`),
- `halo::apply` geometry set — `!sheet_node && !is_pinned && drawn` (`halo.rs:37-43`),
- `halo::drawn` — `!is_chrome && !Text && !centerline/pitch-circle` (`halo.rs:93-100`),
- `section::is_relaid_geometry` — `!sheet_node && !plane && !magnifier` (`section/mod.rs:267-273`).

Each cares about a slightly different slice, so they're not literally one predicate — but
the `!sheet_node && !is_pinned` base repeats verbatim and the chrome/annotation exclusions
drift apart (halo excludes `centerline`/`pitch-circle` by type-chain; engine excludes all
`is_chrome`). A new chrome type or a new pinned kind must be remembered in all of them.

**Fix:** a small `GeometryClass`/predicate module — `is_geometry(n)` for the shared base,
with the callers layering only their genuine extra (halo's line-type exclusion, section's
marker exclusion). Not a single boolean, but a single source for the base.

---

### 7. "Project a box's four corners onto a unit axis → (lo, hi)" written three times
**Law:** missing shared helper. **Effort:** S. **Risk:** low.

Same corner-projection loop in `chrome::fill` (`chrome.rs:76-88`), `section::plane::project`
(`plane.rs:139-152`), and `breaks/clip.rs:36`. `plane::project` is already the clean named
form; the other two open-code it.

**Fix:** promote `project(bbox, dir) -> (f64, f64)` to `geometry.rs` and call it from all
three (chrome then applies its `CENTER_MARK_OVERHANG`, plane its `PLANE_OVERHANG`).

---

### 8. `Side`→name and `Side`→vector conversions are scattered
**Law:** missing shared helper. **Effort:** S. **Risk:** low.

`Side` (`src/ast.rs:8`) has `parse` and `index` but no `name`/`outward`. So:
`round::side_name` (`round.rs:370-377`) duplicates the `Side`→str arm inside
`anchors::spell` (`anchors.rs:456-461`); `anchors::side_out` (`anchors.rs:481-488`) is the
4-way `Side`→unit-normal, while `annotate::side_unit` (`annotate/mod.rs:229-242`) is the
8-way name→unit (corners included) — the 4 cardinals overlap.

**Fix:** add `Side::name(self) -> &'static str` and `Side::outward(self) -> (f64,f64)` on
the enum; delete `round::side_name`, fold `anchors::spell`/`side_out` onto them. Leave
`side_unit` (it also handles diagonals from a raw string).

---

### 9. Dead / vestigial code and one stale comment
**Law:** dead code / stale comment. **Effort:** S. **Risk:** none.

- **Stale comment:** `annotate/mod.rs:25` — *"true aligned dims are deferred"* on the
  `Axis` enum. Aligned dims shipped in Stage 5 (`plans/CHART-DRAW-alpha23.md:84,283`); the
  comment now contradicts `dims.rs:1-8`.
- **Dead parameter:** `round::spill_dir(attrs, a: &Anchor)` opens with `let _ = a;`
  (`round.rs:359-360`) — `a` is unused; drop it from the signature and the two call sites
  (`round.rs:118,153`).
- **Reinvented helper:** `geometry::geometry_bbox` (`geometry.rs:360-379`) hand-rolls the
  min/max fold over `path_bbox::extent_points` that `Bbox::from_points` already does
  (`ir.rs:289`). Replace the body with `Bbox::from_points(&pts)`.
- **Reinvented helper:** `plane::seg_bbox` (`plane.rs:189-196`) and the inline box at
  `chrome.rs:96-101` both reimplement `Bbox::from_points(&[a, b])` (used cleanly at
  `edges.rs:176`). Fold onto `from_points`.

---

## Things checked and found clean (so the audit is on record)

- **Carried-stack measure vs its band.** The stack is lowered once by
  `CarriedStack::lower` (`symbols.rs:161`); both consumers — the dim path's `carried_band`
  (`dims.rs:289`) and the leader/round path's `carried_push` (`leaders.rs:102`) — read the
  *same* `box_below`, and the same lowered nodes reseat via `CarriedStack::seat`. One
  measure, no drift. Good.
- **Text seat.** `symbols::seat_of` (`symbols.rs:227`) is the single owner, shared by
  leaders, dims, and round. `texts_beside` (leaders) / `value_texts` (dims) /
  `DimText::nodes` (compose) place text for genuinely different anatomies — not a copy.
- **Obstacle registration.** Two entry points (`Rows::obstruct` for placed symbols,
  `Rows::obstruct_texts` for lowered statements) but one owner (`Rows`) and one class
  predicate (`annotation_obstacle`). Not split.
- **Anchor math.** `anchors.rs` is the sole owner; `mates`, `dims`, `round`, `angle`,
  `leaders`, `section` all resolve through `anchors::resolve` and read the `Anchor`
  methods. No duplicated frame/rotation math in `section`/`engine`.
- **The knockout mask** is the single mechanism for both link-label cuts and drawing halos
  (`render/knockout.rs`, consumed by `render/links.rs` and `render/primitives.rs:halo_mask`).
  Matches the "one mechanism" law exactly.

---

## Reorganization sketch (only where genuinely better)

The measure/seat concepts are today smeared across `dims.rs`, `round.rs`, and
`annotate/rows.rs`; the plane-geometry helpers are split between `geometry.rs` and inline
sites. A cleaner layout, addressing findings #1/#3/#4/#5/#7 together:

- **`geometry.rs` → split** into `pen.rs`-adjacent path geometry (`PathSeg`, `Subpath`,
  `mirror`, `to_d`, `scale`, `geometry_bbox`) and a `plane.rs` of pure 2-D helpers
  (`dist`, `unit`, `iso_text_angle`, `arc_center`, `rotate_about`, `project(bbox,dir)`,
  a new `Bbox::center`). `Frame` (now in `dims.rs`) moves here — it is plane geometry.
- **A `measure/` submodule** owning `Frame`, `Seat`, `Band`, `Plan`, and one packer that
  seats along an arbitrary `Frame` (finding #1). `dims.rs`, `round.rs`, and the aligned
  path all become thin callers; `annotate/rows.rs` folds in as `measure/pack.rs`.
- **`leaders/`** split into `skeleton.rs` (line + carried-push math) and the dispatchers,
  with the datum-triangle/box drawing beside `symbols` (it already shares
  `framed_letter_size`/`datum_frame_box`).
- **A `classify.rs`** (or a couple of `PlacedNode` predicates) owning the geometry/sheet/
  chrome/pinned base test (finding #6), layered by the halo/section callers.

Everything else (`corner`, `edges`, `threads`, `breaks`, `mates`, `frames`, `section`,
`compose`, `chrome`, `glyph`, `knockout`) is well-homed and single-concept — leave it.
The reorg is worth doing only if #1 is taken on; absent that, land the mechanical wins
(#2, #3, #7, #8, #9) which are pure subtraction and carry no snapshot risk.

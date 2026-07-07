# DRAWING-0.18 — sections, details & sheet finish (design)

The third drawing round, planned with Abbas on 2026-07-07 against the shipped
0.17 (sheets, revolve, thread) and his ramjet references
(`screw_tip_spike.pdf`, `cooling_ring.pdf`, `screw.pdf` — the injection screw
is the bar). **Stage 0 amends SPEC 15; the amended spec is the contract**;
this file holds the settled decisions, the build order, and the execution
log. The quality bar is DRAWING-0.16.md's, verbatim. Fuller background per
item lives in `TODO.md`'s gap analysis; graduated items are removed there.

Scope: the cutting-plane / section-title machinery, detail markers and the
**auto detail view**, the `.lini-dim-text` de-inlining, physical-size
emission, `|title-block|` fields, and `fillet` / `chamfer` against arcs.

The principle throughout (settled in the 0.17 round): lini is 2D — a
**section's cut face is authored** (pen + `hatch()`, as the bushing already
is), but everything *about* a section that is bookkeeping — the cutting
plane, the letters, the title, the ratio — composes from facts the engine
already has. A **detail view needs no such concession**: it is a 2D
re-render, and the engine is re-entrant.

---

## Decisions ledger (settled in design review — do not relitigate)

1. **`|cutting-plane|`** — a chrome-style child of the source view (`::line`),
   its smart label the letter (`|cutting-plane| "A"`). `at: N [x-axis|y-axis]`
   places it: the line runs **perpendicular to** the named axis at station
   `N`, the axis defaulting to the model's longer one (`break:`'s convention).
   Lowering (the `chrome:`-marker plumbing end to end, geometry filled from
   the parent's sized box like `|centerline|`): the ISO line — thin dash-dot
   (`stroke-style: center`) across the geometry + overhang, **thick end
   strokes** just past it, viewing-direction arrows (the slender dim arrow),
   the letter beside each end. `facing: left|right|up|down` turns the arrows;
   default `right` for a vertical plane, `down` for a horizontal one. The
   multi-part lowering clones typed pieces from the seed — `edges::fill`'s
   play.
2. **Section titles compose.** A view declaring `section: a` (no label of its
   own) synthesizes its title: the uppercased letter doubled — `A-A` — plus
   the ratio `(own scale ÷ enclosing page scale)`, formatted `1:1` / `1:1.5` /
   `2:1` via `compose::fmt` (≤ 2 dp). Desugar lowers a placeholder
   `|footnote|` carrying the attr; the drawing engine fills the text where it
   pins the title (the scale is only known there). A `detail: c` declaration
   titles `C (1:1)` the same way. An authored label always wins.
3. **`|detail-circle|`** — the region marker on the source view: `::oval`
   (`stroke: --stroke-light; stroke-width: 1; fill: none`), `width:` the
   region ⌀, positioned with `translate:` like any feature; its smart label is
   the letter, placed just outside the rim at 45° (`note-offset` out). Not
   chrome — an ordinary part-frame child, like `|balloon|`.
4. **`|detail|` — the auto view** (`::drawing`): `of: <path to a
   |detail-circle|>` + its own `scale:`. **The marker is the single source of
   truth for the region** — the view derives centre and ⌀ from it, and its
   letter titles the view (`C (1:1)` per decision 2). The engine re-lays the
   marker's *host drawing*'s **geometry children** under the detail's scale
   (`layout_inst` is re-entrant; a different `Ctx.scale` is the whole trick),
   shifts by `−centre × scale`, **skips the source's annotations** (the
   engine's link/geometry partition already separates them), and clips to the
   circle. The detail's own `[ ]` annotations anchor into the re-laid clones —
   `annotate::Ctx.kids` takes whatever slice it is given, so the existing
   anchor walk works unchanged. Clip: a new `clip: circle r` on the placed
   node, emitted by render as one interned `<clipPath>` in `<defs>` (the
   `paints.rs` interner pattern) + `clip-path=` on the group. Clones are
   placed copies, never re-registered in the scene — only the detail's own
   links may address them.
5. **`.lini-dim-text`** — annotation text stops inlining its style. Dim,
   leader, and callout text leaves get `type_chain: ["dim-text"]` and stop
   carrying `font-size` / `font-weight` in `own_style`; render emits one
   `.lini .lini-dim-text { font-size: 12px; font-weight: normal }` rule
   beside `.lini-dim-line` (present only when drawings are), and the repeated
   `style="font-weight: normal; font-size: 12px"` inlines disappear from
   every sheet. A statement's own text styling still inlines (that is what
   `own_style` is for). While in there: sweep any other repeated inline the
   drawing lowering emits — the paint-review rule (2026-07-05) finished the
   linework; this finishes the text.
6. **Physical-size emission.** A file whose drawn content is only `|page|`s
   (the hug-the-canvas predicate, shared) emits `width="210mm"
   height="148mm"` beside the viewBox — true-scale prints; on-screen CSS
   sizing is unaffected. Carried as `LaidOut.physical: Option<(f64, f64)>`
   (mm), written by the root `finish`, emitted by render. Closes SPEC 23's
   physical-size deferral.
7. **`|title-block|` fields** — property-driven ISO 7200 sugar at desugar
   (like `sheet:`): string-valued field properties on the template —
   `dept`, `reference`, `author`, `approved`, `doc-type`, `status`, `title`,
   `dwg`, `rev`, `date`, `sheet` — build the fixed nested grid from Abbas's
   sheets (the Fusion/PicoFinity block in `tie_bar.pdf` is the reference
   look: field captions in the muted footer tone at ~7 px over values at
   11). **Absent fields collapse** — their rows/cells don't render, so the
   default block is minimal (Title / DWG No. / Rev / Sheet suffice); a
   `|title-block|` with **no** field properties keeps today's plain-table
   behaviour (fields as cells, fully custom). The logo
   cell waits for local-image embedding (TODO) — out of scope here.
8. **`fillet` / `chamfer` against an arc** — `corner.rs` grows the line↔arc
   and arc↔arc cases: the tangent circle of radius `r` against a line and a
   circle solves closed-form (offset-line ∩ offset-circle / offset-circle ∩
   offset-circle — quadratics, no iteration); `chamfer(c)` cuts back along an
   arc by **arclength**. The pending-slot and cyclic-through-`close()`
   plumbing is untouched — only `apply_mod`'s geometry grows. Removes the
   SPEC 23 deferral and the "joins two straight segments today" error.
9. **Zero grammar**, again: types, properties, keywords, chrome, and one
   render feature (`clipPath`) — no tokens, no ops, no value forms.

The target sample: **`drawing_screw.lini`** — the injection screw
(`RJ-SCR-20-001`), the hardest sheet Abbas has: the sectioned A-A barrel as a
revolved root profile + two half-pitch-offset `pattern: grid` rows of flight
sections under `hatch()`, detail circles C / D / E with auto detail views,
zone-band dims (FEED / TRANSITION / METERING), flight-root fillets against
arcs, an internal-thread E detail, on an A3 page with a filled title block.
Whatever it can't express is the next round's gap list.

## Stages

Each stage ends green per DRAWING-0.16.md's bar (fmt, test, clippy, snapshots
reviewed, new samples PNG-rendered via resvg and looked at); one purposeful
commit per stage; append to the execution log.

### Stage 0 — SPEC 15 amendments (the contract)

- 15.7/15.8: `|cutting-plane|` (anatomy, `at:` / `facing:`, the letter), the
  composed `section:` / `detail:` titles, `|detail-circle|`, `|detail|` with
  `of:` (the marker as the region's source of truth, source annotations
  skipped, own annotations against the clones, the clip).
- SPEC 8 rows: `|cutting-plane|`, `|detail-circle|`, `|detail|`; the
  `|title-block|` row gains the field properties.
- 15.3: the corner modifiers lose "straight runs only"; SPEC 23 loses that
  row and physical-size; SPEC 17 gains `<clipPath>` and the `.lini-dim-text`
  rule and the physical `width`/`height` attrs; SPEC 16 rows; SPEC 20 rows
  (`of:` misses / names a non-marker, `at:` off the profile, bad `facing:`,
  detail-in-detail if gated).
- Abbas reviews the amended spec before any code.

### Stage 1 — the section bookkeeping

- `|cutting-plane|` chrome (template, seed, multi-part fill) + `facing:`.
- `section:` / `detail:` title composition through the engine's title seat.
- `|detail-circle|` (template + rim label placement).
- Sample: `drawing_ring.lini` — the cooling ring: left view with the plane
  A–A and marker-free hidden ports, the **authored** hatched section view
  titled `A-A (1:1)` by composition. PNG-inspect against `cooling_ring.pdf`.

### Stage 2 — the auto detail view

- `clip:` on `PlacedNode` + the interned `<clipPath>` emission.
- `|detail|` engine arm: resolve `of:`, re-lay the host's geometry at the
  detail scale, shift, clip, seat the composed title; the detail's own
  annotations lower against the clones.
- Sample: a detail on `drawing_shaft.lini`'s groove (`|detail-circle#c| "C"`
  + a `|detail|` view dimensioning the groove radii at 4 : 1). Watch the
  interactions logged in TODO: a broken source (positions are per-inst —
  expected fine), id collisions (clones unregistered — by design).

### Stage 3 — sheet finish, small round

- `.lini-dim-text` (decision 5) — snapshots sweep, no geometry moves.
- Physical-size emission (decision 6).

### Stage 4 — `|title-block|` fields

- The field-property grid at desugar; captions/values styling; empty-field
  cells; the plain-table form untouched. `drawing_sheet.lini` upgrades to
  fields; SPEC 24 syncs.

### Stage 5 — corners against arcs & the screw

- `corner.rs` line↔arc / arc↔arc fillet + chamfer (decision 8), with the
  seam/cyclic tests extended to curved legs.
- **`drawing_screw.lini`** — the stress sample (see above). Cross-check the
  measured values by hand; whatever composes badly is logged, not patched.

---

## Execution log

Append-only, per DRAWING-0.16.md's rule.

- **2026-07-07 — plan written** (same session as the 0.17 finish rounds:
  ISO 129 arrowheads, `align: origin` + the centreline rule, the
  scope-transparency fix). Decisions 1–9 settled with Abbas; the marker-as-
  region-source design for details and the property-driven title block were
  settled here (the TODO's earlier by-path field idea dropped — lini has no
  set-text-by-path, and properties are the `sheet:` precedent).

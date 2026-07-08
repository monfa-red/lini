# DRAWING-0.19 — sections, details & sheet finish (design)

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

- **2026-07-08 — stage 3 landed** (all gates green — 769 tests, clippy silent,
  fmt clean; 11 drawing snapshots swept — the diff is **only** font inlines →
  class + the mm attrs, zero coordinate churn, so no geometry moved; the ring
  re-inspected as pixel-identical). Sheet finish:
  - **`.lini-dim-text`** (decision 5). Dimension / round / leader / callout text
    (and the cutting-plane letters) build through a new `prim::dim_text` — a
    `.lini-dim-text` leaf that inlines nothing at the default (12 px, normal);
    only a size that differs (a `tol:` deviation at 0.7×, a restyled link) keeps
    an inline `font-size`. Render states the rule once (`rules.rs`, gated on the
    class being present, beside `.lini-dim-line`). Composed titles went further
    — `prim::text_plain`, a bare leaf that **inherits** the footnote's font — so
    the repeated `style="font-weight: normal; font-size: 12px"` is gone from
    every sheet.
  - **Physical-size emission** (decision 6). `LaidOut.physical: Option<(f64,
    f64)>`, set by the root `finish` when the scene is **pages-only** (the
    shared `pages_only` predicate): the viewBox extent over the page's
    px-per-mm `scale:`. Render emits it as the SVG root's `width="210mm"
    height="148mm"` (the viewBox stays px, so on-screen sizing is unchanged) —
    true-scale prints. Closes SPEC 23's physical-size deferral.
- **2026-07-08 — stage 2 landed** (all gates green — 768 tests, clippy silent,
  fmt clean; `drawing_detail.lini` PNG-rendered via resvg `--bake-vars` and
  inspected: the shaft's groove re-rendered at 4:1, clipped, ⌀18-dimensioned,
  titled `C (4:1)`). The auto detail view:
  - **`clip:` on the placed node → an interned `<clipPath>`.** A `Number(r)`
    attr, rewritten to a `url(#lini-clip-N)` reference by `paints::lower` (the
    same intern-and-rewrite pass as the gradients / hatches; `LaidOut.clips`
    holds the radii), one `<clipPath clipPathUnits="userSpaceOnUse"><circle
    r=…>` per distinct radius in `<defs>`, and render puts `clip-path=` on the
    group. Zero struct churn — `clip:` rides `attrs` like `points:` / `chrome:`.
  - **`|detail|` engine arm** (`section::layout_detail`): resolve `of:` to the
    marker **by id** (like a chart's `axis:` — a dotted path isn't a value
    form, SPEC 21; `find_marker` returns the marker + its host), take the
    centre / diameter / letter, **re-lay the host's geometry children** through
    the re-entrant `layout_inst` at the detail scale, shift by `−centre ×
    scale`, wrap them in a clipped group, and lower the detail's own
    annotations against those **clones** (extent = the region circle, so dims
    hug it). The title composes `C (1:1)` from the marker's letter, seated in a
    desugar-seeded `detail-title` footnote.
  - **Resolve defers the detail's endpoints.** The clones exist only at layout,
    so a new `LinkScope.detail` flag keeps a detail scope's annotation
    endpoints as qualified paths (skipping the scene-index lookup); the anchor
    walk lands them on the clones. The **lint's** auto-create-shadow pass
    likewise gained the drawing-scope gate it lacked (it re-derived
    auto-create without it), so a detail's `shaft:groove` no longer warns.
  - **Decisions / deltas from the plan:** `of:` names the marker **by id**, not
    a path (the design said "path"; a dotted path doesn't parse as a value —
    SPEC 15.8 / 16 / 20 synced). A `|detail-circle|`'s letter is read from its
    text child (an `|oval|`-based type carries the label there, not `label:`).
    Detail-in-detail is gated (SPEC 20). `part_bbox` gained `detail-circle` so
    a lone `width:` reads as a circle.
  - Sample `drawing_detail.lini`; conformance snapshot accepted.
- **2026-07-08 — stage 1 landed** (all gates green — 766 tests, clippy silent,
  fmt clean; `drawing_ring.lini` PNG-rendered via resvg `--bake-vars` and
  inspected). The section bookkeeping:
  - **`|cutting-plane|` is authored chrome.** Desugar tags it `chrome:
    cutting-plane` in a drawing scope (so `layout_inst` intercepts it as a
    placeholder, and `chrome::placeholder` now keeps the label — the section
    letter). A new `layout/drawing/section.rs` fills it from the seated
    view's **geometry extent** (computed in `lay_out` after mates, chrome
    excluded from the extent): the thin dash-dot chain line across the model +
    overhang, thick end strokes, a slender viewing arrow (`dims::arrow`) at
    each end, and the letter beside each. `at: N [axis]` stations it on the
    longer axis by default; `facing:` turns the arrows (default `right` /
    `down`), and `at:`-off-model / bad-`facing:` error per SPEC 20.
  - **Composed titles.** `section:` / `detail:` on a labelless drawing seeds a
    placeholder `|footnote|` (`section-title: <kind> <letter>`) at desugar; the
    engine composes `A-A (1:1)` in `layout_node` where `own` and the enclosing
    `ctx.scale` are both known (`compose::section_title` — the letter doubled
    for a section, the `own ÷ page` ratio normalised to `2:1` / `1:1.5`).
  - **`|detail-circle|`** is an ordinary round feature (added to `part_bbox`'s
    width-is-diameter set, so a lone `width:` is a circle, not an ellipse); its
    smart label moves out to the rim at 45° (`NOTE_OFFSET` past the rim).
  - **Chrome tone.** The whole cutting-plane marker (line, thick ends, arrow
    shafts, arrowheads) rides the cutting-plane's own `stroke` (default
    `--stroke-light`), varying only weight — chain line at 1, thick ends at 2 —
    so `|cutting-plane| { stroke: … }` restyles it whole; the letters read the
    default text tone.
  - Sample `drawing_ring.lini`: the cooling ring, front view (A–A plane,
    marker-free ports, a `C` detail circle) + the authored hatched section
    titled `A-A (1:1)` by composition. Conformance snapshot accepted.
- **2026-07-08 — stage 0 landed** (SPEC only; no code, so no test gate — the
  contract for Abbas's review). SPEC amended per the ledger:
  - **15.8** grew a **Sections & details** block: `|cutting-plane|` (a `::line`
    chrome child, its label the section letter; `at: N [axis]` on the
    longer-axis-default station, `break:`'s convention; `facing:` turning the
    ISO viewing arrows; the ISO plane — thin dash-dot across geometry +
    overhang, thick ends, arrows, letters); the composed `section:` → `A-A
    (ratio)` and `detail:` → `C (1:1)` titles (ratio = own scale ÷ enclosing
    page's, both default 4; the seat is the existing title `|footnote|`, filled
    where the ratio is known); `|detail-circle|` as the region's single source
    of truth; `|detail|` re-laying its host view at `of:`'s marker — geometry
    kept, source annotations dropped, shifted and clipped to the circle.
  - **15.7** producer table grew to **eight** (the cutting-plane's ends /
    arrows / letter). **15.3** corner modifiers gained the **arc** leg
    (`chamfer` cuts back by arclength on a curve) — decision 8.
  - **SPEC 8** gained three template rows (`|cutting-plane|`, `|detail-circle|`,
    `|detail|`) and the `|title-block|` field note. **15.10 / SPEC 16** property
    rows: `section` / `detail` (on `|drawing|`), `at` / `facing` (on
    `|cutting-plane|`), `of` (on `|detail|`), the ISO 7200 fields (on
    `|title-block|`). **SPEC 17** gained the `.lini-dim-text` rule, the
    `<clipPath>`, and the physical-mm `width` / `height`. **SPEC 20** gained six
    rows (`of:` missing / unknown / non-marker, detail-of-a-detail, `at:` off
    the model, bad `facing:`). **SPEC 22** the three type names. **SPEC 23**
    lost physical-size and fillet-vs-arc, and narrowed "view machinery" to just
    projection lines.
  - **The round is renamed 0.18 → 0.19**: v0.18.0 already shipped the post-0.17
    sheet polish (ANSI sheets, the equal reference band, the ISO print tones),
    so this "sections & details" round is 0.19 (the file name was already
    0.19; the header lagged).
  - Decisions surfaced while writing the contract: **a `|detail|` of a
    `|detail|` is gated** (the re-lay stays one level — SPEC 20 + SPEC 23);
    **`sheet:` on a title-block is the sheet-number field**, distinct from a
    page's size sugar. SPEC 24 stays untouched until the samples exist (per
    stage), as in the 0.17 round.
- **2026-07-08 — spec review: tooltip `title:` → `hint:`** (Abbas's call; all
  gates green — 613 lib tests + integration suites, clippy silent, fmt clean;
  `hint:`/`title:` behaviour confirmed end-to-end). The ISO 7200 field list
  wants `title:` for the drawing's title, which collided with the universal
  tooltip / accessible-name property. Rather than overload it in the
  title-block scope, the **tooltip property is renamed `hint:`** and `title:`
  is the title-block field cleanly. Landed as its own commit: the internal
  attr key `title` → `hint` (one key feeds the `<title>` element — `set_hint`,
  the render reader, the chart `<title>` floor in `tooltip.rs`), `hint` added
  to `is_string_valued`, SPEC 2/16 + the box-only-property line updated, tests
  swept. The **`<title>` element is unchanged**, so zero snapshot churn; a bare
  `title:` on a plain box is now inert (freed for the field).
  - **Awaiting Abbas's spec review before Stage 1** (the plan's Stage 0 gate).
- **2026-07-07 — plan written** (same session as the 0.17 finish rounds:
  ISO 129 arrowheads, `align: origin` + the centreline rule, the
  scope-transparency fix). Decisions 1–9 settled with Abbas; the marker-as-
  region-source design for details and the property-driven title block were
  settled here (the TODO's earlier by-path field idea dropped — lini has no
  set-text-by-path, and properties are the `sheet:` precedent).

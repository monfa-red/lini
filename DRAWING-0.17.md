# DRAWING-0.17 — sheets, turned parts & threads (design)

The second drawing round, designed with Abbas on 2026-07-06 (this file is the
record — the session brainstormed against SPEC 15 as built and PLAN.md's
execution log). **Stage 0 amends SPEC 15; the amended spec is the contract**,
exactly as SPEC 15 was for PLAN.md; this file holds the settled decisions, the
build order, and the execution log. The quality bar is PLAN.md's, verbatim.

Scope: the `|page|` sheet container, `|title-block|`, `|hidden|` geometry,
`revolve:` with the edge-line law, and `thread:` on sketches and holes.

---

## Decisions ledger (settled in design review — do not relitigate)

1. **`|page|` is a template container, not a layout.** Sheet-space, never a
   drawing scope; its interior is ordinary layout (default `flow`;
   `layout:` / `columns:` / `direction:` free — `direction:` is the inner
   flow's axis and is *not* the sheet orientation). It generates the ISO 5457
   furniture as **generated chrome children** (the existing auto-chrome
   mechanism, new producers): the thick frame (20 mm filing margin left,
   10 mm the other three sides), the zone grid (50 mm divisions **derived
   from the final width/height** — numbers 1..n across top and bottom,
   letters A.. down both sides), and the centring marks. The cascade styles
   or removes all of it (`|page| |zone| { stroke: none }`).
2. **`sheet:` is pure sugar** on `|page|`: `sheet: a3` / `sheet: a4 landscape`
   desugars to `width` / `height` **in mm** (the orientation keyword swaps the
   pair; ISO defaults — A4 portrait, A3–A0 landscape; a bare `|page|`
   defaults to `sheet: a4`). Explicit `width:` / `height:` override through
   the ordinary slot, and zones always derive from the final numbers, so a
   custom sheet stays zoned.
3. **Page `scale:` is px per mm**, default **4** — a `|drawing|`'s own
   default, so a default drawing on a default page draws **1 : 1 true**, and
   the drafting ratio is drawing scale ÷ page scale (a 2 : 1 detail is
   `scale: 8`). This is the natural on-ramp for the deferred physical-size
   emission (real-mm SVG for true-scale prints).
4. **`|title-block|`** — a define over `|table|` (sharp corners, thin rules,
   small font; ISO 7200 look, anatomy constants at build time). The page
   seats it **by type**, flush inside the frame's bottom-right corner (a
   `pin:` would anchor the sheet edge, so the page engine seats it, as a
   sequence seats notes). Fields are ordinary cells; smart fields (sheet
   name, view scales, the page label as the title) are deferred — no syntax
   change when they land. No `title-block:` property: a chart's `axis: t`
   indirection disambiguates among several axes; a page has one seat, so the
   type is the reference.
5. **`|hidden|`** — `::sketch` with
   `{ stroke-style: dashed; stroke-width: 1; fill: none }`: the hidden-edge
   convention on its own child, per SPEC 15.7's one-node-one-stroke-style
   law. It is a feature in a part's `[ ]` — rigid under mates, rides
   `break:`'s black hole, its `:segment`s dimensionable via dot-path. No
   per-call styling in the pen (`right(50, dashed)` rejected: one path folds
   to one `<path>` with one stroke, and a dashed run inside a closed profile
   would tear the fill boundary and the mirror seam).
6. **`revolve: x-axis | y-axis`** declares a **solid of revolution**. It
   folds and fuses exactly like a fused `mirror:` (axis `|centerline|`
   included) and additionally generates the **edge lines** — the projection
   law of a turned part: at every **tangent-discontinuous** vertex off the
   axis, a line to its mirrored twin, perpendicular to the axis; a span
   coincident with a drawn profile segment is skipped. Fillets are tangent →
   no line; a chamfer keeps two sharp vertices → its two edge circles; a
   step completes itself; a groove draws its lips. Lowered as **`|shoulder|`**
   chrome children (*amended 2026-07-06 in stage 1*: `|edge|` collided with
   `hero.lini`'s `|edge::box|` define — "edge" is the graph word and no
   built-in may claim it) — **geometry weight** (they are real visible
   edges), the first thick chrome — cascade-removable like all generated
   children. Edge lines live in the sketch frame, so they ride `break:` like
   features.
7. **`revolve:` and `mirror:` are exclusive** on one sketch (error; relax
   later if a real part needs both). `mirror:` stays exactly as built, for
   flat symmetric parts, and never draws edge lines.
8. **The ⌀ readings move to `revolve:`.** The station `(o)` (a segment's
   span across the axis, ⌀-formatted) and the bare `(o)` full-span reading
   require a revolved profile — on a merely mirrored one they error ("a
   station '⌀' reads a revolved profile — 'revolve: x-axis'"): a mirrored
   plate's span is a width, not a diameter. The unary `(<)` included angle
   stays valid on both (a taper on a flat wedge is a real included angle).
9. **`thread:` is a sketch property**, beside `mirror:` / `break:` — the
   taxonomy: facts **at a point** in the pen stream are calls
   (`fillet` / `chamfer`); facts **along the path** are properties. Value:
   `thread: seg pitch` with comma groups (`thread: left 1.5, right 1.5;` —
   a double-end stud); the segment name reads **bare** (the chart `axis: t`
   precedent — an endpoint's `:` separates id from point; a value has
   nothing to separate). Requires `revolve:`; the segment must be a straight
   run parallel to the axis (error otherwise). It draws, per ISO 6410
   external convention, as generated chrome doubled by the revolve: the
   **minor line** — thin (`--stroke-light`, width 1) — offset into the
   material by **0.6134 × pitch** (the ISO 60° thread depth), running the
   segment and stopping at an adjoining chamfer's trim point; and the
   **thread-end line** — geometry weight, across the full diameter at the
   segment's inner end.
10. **`thread: pitch` on `|hole|`** (no segment — the hole is the feature):
    the top view's thin **¾ arc at the major diameter** (gap top-right,
    deterministic) outside the solid drilled bore; centre marks unchanged.
    The **type carries the sense**: `|hole|` reads internal; the same
    property on plain round geometry (`|oval|` lineage) reads external —
    solid major outline, thin ¾ **minor** arc — the stud-end view.
11. **The threaded smart leader.** A bare leader (`<-`, no label) on a
    threaded segment auto-composes **`M{⌀}×{pitch}`** — the numbers live
    once (⌀ from the geometry, pitch from `thread:`); re-cut the bar and the
    callout follows. Metric form only; a label overrides, as everywhere
    (imperial threads are a label).
12. **Zero grammar.** No new tokens, ops, or value forms in the whole round —
    templates, properties, keywords, and generated chrome throughout
    (`sheet:` / `revolve:` / `thread:` values are existing ident + number
    shapes). SPEC 15's four-ops-and-one-value-form budget is untouched.

The target sample (drove the whole design): a DIN 912 socket cap screw —
side view `revolve: x-axis` + `thread: m8 1.25` + a `|hidden|` hex socket,
end view `|oval|` + `|hex|`, later hosted on an A4 `|page|` with a
`|title-block|`.

## Stages

Each stage ends green per PLAN.md's bar (fmt, test, clippy, snapshots
reviewed, new samples PNG-rendered via resvg and looked at); one purposeful
commit per stage; append to the execution log.

### Stage 0 — SPEC 15 amendments (the contract)

- A new sheet section (15.x): `|page|`, `sheet:`, `|title-block|`, the page
  chrome table; SPEC 15.8's multi-view prose points at it.
- 15.3 gains `revolve:` (+ the edge-line law and its `|edge|` chrome) and
  `thread:`; 15.4 gains the `|hole|` / round-geometry `thread:` readings;
  15.6 re-gates the ⌀ readings on `revolve:`; 15.7's producer table grows
  (revolve → centerline + edges; thread → minor / end lines / ¾ arc; page →
  frame / zones / marks).
- SPEC 8 template rows: `|page|`, `|title-block|`, `|hidden|`, `|edge|`.
- SPEC 16 ledger rows; SPEC 20 errors (thread without revolve, thread on a
  non-parallel or non-straight segment, unknown segment in `thread:`,
  station ⌀ on mirror-only, revolve + mirror together, `sheet:` keyword
  typos with did-you-mean); SPEC 24 gains the DIN 912.
- Abbas reviews the amended spec before any code.

### Stage 1 — `revolve:`, the edge law & `|hidden|`

- `revolve:` shares the mirror fold/fuse path (one mechanism; a flag on the
  fold, not a parallel pipeline); edge-line generation from the folded
  profile's tangent-discontinuous vertices; coincident-span skip; `|edge|`
  chrome + cascade; the ⌀ re-gate.
- `|hidden|` template registration (a bundle row — near-zero code).
- Samples: the tie bar migrates (`mirror:` → `revolve:`); a stepped shaft
  with a groove and a fillet-vs-chamfer contrast (fillet draws no edge line —
  the law's showpiece). PNG-inspect.

### Stage 2 — `thread:` & the smart leader

- Sketch `thread:`: resolve keeps the groups structured; layout validates
  (revolve present, segment straight and axis-parallel), computes minor
  offset and end line, chamfer trim stop; chrome children.
- `|hole|` / round-geometry `thread:`: the ¾ arcs, sense by type.
- The bare-leader auto-compose (`M⌀×P`) in `compose.rs`.
- Samples: the DIN 912 (side + end view); the tie bar gains
  `thread: m8 1.5` and drops its typed leader label. Hand-check every value.

### Stage 3 — `|page|`, `sheet:` & `|title-block|`

- `sheet:` desugar; the page chrome producers (frame, zones from the 50 mm
  rule, centring marks); title-block seating by type; the A4-portrait /
  A3-landscape defaults.
- Samples: an A4 sheet hosting the DIN 912 views with a filled title block;
  an A3 landscape multi-view. PNG-inspect both themes.
- SPEC 16 marks flipped in one sweep; README layouts list checked.

---

## Execution log

Append-only, per PLAN.md's rule.

- **2026-07-06 — stage 1 landed** (same session; all suites green — 735
  tests — clippy silent, fmt clean; shaft / tie bar / pump / barrel
  PNG-rendered via resvg at `--bake-vars` and inspected). What shipped, and
  what stages 2–3 must know:
  - **`|edge|` → `|shoulder|`.** The planned chrome type name collided
    immediately: `samples/hero.lini` defines `|edge::box|`, and "edge" is
    *the* graph word in a diagram language — a built-in may not claim it.
    Renamed to `|shoulder|` (the machining word; the SPEC prose already
    called them shoulder lines). Ledger 6 stands otherwise; SPEC 8 / 15.3 /
    15.7 / DRAWING-0.17 updated.
  - **The edge law lives in `layout/drawing/edges.rs`**: sharp vertices from
    the folded, scaled, break-clipped subpaths (open run ends and stitched
    break cuts are not joints; arc tangents from `arc_center`, cubic
    tangents from the control polygon); same-station dedup at the widest
    span; the coverage skip merges the profile's own perpendicular straight
    segments. Desugar seeds **one** `|shoulder|` chrome child
    (`chrome: edges`); `edges::fill` clones it per span — the pattern-carrier
    play, so the cascade's resolved style rides every line and
    `|sketch| |shoulder| { stroke: none }` removes them (test pins it).
  - **`revolve:`** parses in the pen (`x-axis` / `y-axis` only; exclusive
    with `mirror:`; folds via the same `geometry::mirror` — one mechanism);
    `Folded` / `SketchGeo` carry `revolved` + the spans. The ⌀ station and
    bare full-span readings now error on mirror-only profiles ("a station
    '⌀' reads a revolved profile — 'revolve: x-axis'"); the unary `(<)`
    stays valid on both.
  - Samples: tie bar (segment renamed `:thread` → `:m20`, per SPEC 24),
    pump, and barrel migrated to `revolve:` — every shoulder, chamfer edge
    circle, and groove lip now draws its full line, matching Abbas's
    reference sheets; `drawing_shaft.lini` is the law's showpiece (root
    fillet R3 draws nothing, the sharp step completes itself, a `|hidden|`
    centre bore rides the `[ ]`, its redundant centerline removed by the
    cascade in the stylesheet). Tie bar matches SPEC 24 fully only after
    stage 2 adds `thread:`.
- **2026-07-06 — stage 3 landed** (same session; all suites green — 750
  tests — clippy silent, fmt clean; the DIN 912 sheet PNG-rendered and
  inspected: frame at the 20/10 margins, 6 × 4 zone grid with dividers and
  references, centring marks, the screw at true 1 : 1, the title block sharp
  and flush inside the frame corner). What shipped, and the decisions:
  - **`sheet:`** desugars in place (`desugar/page.rs`) to `width` / `height`
    in mm — a5…a0, ISO orientation defaults (A4/A5 portrait, A3–A0
    landscape), a real did-you-mean via a small edit distance. The `|page|`
    bundle carries the A4 default plus `scale: 4` and **`stroke-width: 0`**
    (a trimmed sheet has no stroke; the `|frame|` child draws the border —
    without this the sheet box ran 2 px proud).
  - **The furniture** is desugar-generated pinned chrome (`pin: center`
    keeps it out of the page's flow — core machinery, no new placement
    mode): one `|frame|`, `|tick|` dividers + four centring marks (thin,
    not ISO's frame weight — the house light aesthetic; SPEC reworded),
    `|zone|` labels on all four edges. `layout/page.rs::finish` gives them
    geometry from the sized sheet — the divisions derive from the children
    desugar counted, so the two never disagree. The content area folds into
    the padding for the arrange pass only (`padded_attrs`), so `padding:`
    adds, per the spec; page sizing itself stays the plain floor.
  - **`|title-block|`** (a `|table|`: font 11, stroke 1, sharp) gets
    `pin: bottom right` injected at desugar when it has no `pin:` of its
    own; `finish` pulls it in by the margins — flush inside the frame line.
  - **A parser fix at the source**: a *spaced* `:name` after a pen call
    (`right(12) :v`) silently **attached** as the call's segment —
    `at_glued_point_name` never checked the colon's own gluing. Every
    existing sample dodged it (their freestanding names all followed
    already-named calls); the screw's `:v` exposed it as a phantom
    "mixes axes" error. One-line fix (`glued_at(0)`); spaced = freestanding,
    exactly as SPEC 15.2's table always said.
  - **The anonymous-container scoping edge bit again**: an id-less `|page|`
    broke its descendant drawings' link scoping ("'(o)' belongs in a
    'layout: drawing'") — the known lifted-prefix limitation. The sample
    and SPEC 24 example carry `|page#sheet|`; the real fix stays with the
    deferred core-scoping session (PLAN.md's open thread).
  - Samples: `drawing_sheet.lini` — SPEC 24's sheeted DIN 912,
    byte-for-byte. README's drawings section now names revolve, thread,
    and the sheet.
- **2026-07-06 — stage 2 landed** (same session; all suites green — 740
  tests — clippy silent, fmt clean; tie bar + barrel PNG-rendered and
  inspected against Abbas's references: minor lines from the chamfer trim to
  the thread-end line, ¾ arcs on all three tapped holes riding the pattern
  copies, composed M-specs). What shipped, and what stage 3 must know:
  - **`layout/drawing/threads.rs`**: parse the `thread:` groups (bare
    segment + pitch; comma groups; SPEC 20 messages verbatim incl. the
    did-you-mean); the minor lines at `level − 0.61343 × pitch` span the
    segment's **drawn extent** — the profile's collinear segments clipped to
    the authored run, so a chamfer's trim ends them with no chamfer-specific
    code; the **thread-end line** draws where a collinear segment continues
    past an authored end, and joins `Folded.edges` — it *is* an edge line,
    so it rides the `|shoulder|` machinery (dedup, coverage, chrome) for
    free. Minor spans ride a new `|threadline|` template
    (`--stroke-light`, w1 — extension-line tone) through the generalized
    `edges::fill(children, marker, spans)` seed-clone.
  - **The ¾ arc** is a `chrome::fill` arm: desugar seeds
    `chrome: thread-arc <sense> <pitch>` on a `|hole|` (internal — major =
    `width + 1.0825 × P`) or plain `|oval|` (external — minor =
    `width − 1.2269 × P`); fill flips the child's kind to `Path` (the old
    S-break play) with the gap over the upper-right quadrant.
    `chrome::fill` now takes the part's own scale.
  - **The smart leader**: the empty-text gate for `<-` moved from resolve to
    layout (only the arrow form — `*-` / `>-` still gate at resolve;
    `tests/resolution.rs` updated), and `leaders::callout` composes
    `M{⌀}×{pitch}` from `SketchGeo.threads` + the segment's level about the
    revolve axis. A bare `<-` on anything unthreaded errors with the same
    message, now at layout.
  - Samples: the tie bar now matches SPEC 24 **byte-for-byte**; the barrel's
    `:thread` segment became `:m42` (a segment named like the property read
    badly), gained `thread: m42 1.5` + the bare leader, and its M8 holes
    `thread: 1.25` — the ¾ arcs replicate per pattern copy with no new code.
  - **Accepted edge** (noted for later): a `break:` cutting *through* a
    threaded run maps only the minor line's endpoints — the line would span
    the gap. No sample does this; fix if a real sheet ever breaks inside a
    thread.
- **2026-07-06 — stage 0 landed** (same session as the design). SPEC 15 amended
  per the ledger: SPEC 8 gained eight template rows (`|hidden|`, `|edge|`,
  `|page|`, `|title-block|`, `|frame|`, `|zone|`, `|tick|`); 15.3 gained the
  `revolve:` and `thread:` subsections (the edge-line law includes the
  coincident-span skip and same-station dedup; the thread-end line draws only
  where the surface continues **collinearly** — where the profile turns, the
  geometry already ends the thread); 15.4 the hole/round `thread:` senses
  (internal major = `width + 1.0825 × pitch`, external minor =
  `width − 1.2269 × pitch`, ¾ arc open over the upper-right quadrant); 15.6
  re-gated the `⌀` station / full-span readings on `revolve:` (the unary `(<)`
  stays valid on both); 15.7's producer table grew to seven; 15.8 retitled
  "Assemblies, views, sheets & titles" (+ the `|page|` prose — content area =
  frame inset by 5 mm, `padding:` adds); 15.10 / SPEC 16 property rows;
  eleven SPEC 20 rows; SPEC 24's tie bar migrated to
  `revolve:` + `thread: m20 1.5` + a bare composed leader (segment renamed
  `:thread` → `:m20` — a segment named like the property read badly), the
  pump's barrel/nozzle to `revolve:`, and a sheeted DIN 912 example landed.
  Decision made here: the page's ISO furniture is **typed chrome children**
  (`|frame|` / `|zone|` / `|tick|`), not classed lowering — the ledger's
  cascade-styling promise requires types. Samples catch up per stage (tie bar:
  revolve in stage 1, thread in stage 2 — SPEC 24 byte-for-byte only after
  stage 2).

- **2026-07-06 — design settled** (this file). The whole round emerged from
  Abbas's three asks: a standard sheet, threads on turned parts, and hidden
  geometry — plus his observation that every geometry change on a lathe part
  draws a perpendicular line, which became `revolve:` and the edge-line law.
  Names chosen in review: `|title-block|` (over `|stamp|` and a chart-style
  `title-block:` property), `|hidden|` (over `|hidden-line|`), `sheet:`
  (over `size:` — too generic — and overloading `direction:`). `thread:`
  moved from a pen-call proposal to a property on Abbas's point that
  fillet/chamfer act at a point while a thread runs along the path.

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
   step completes itself; a groove draws its lips. Lowered as **`|edge|`**
   chrome children — **geometry weight** (they are real visible edges), the
   first thick chrome — cascade-removable like all generated children. Edge
   lines live in the sketch frame, so they ride `break:` like features.
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

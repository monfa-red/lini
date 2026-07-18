# CHART-DRAW-alpha23 — charts remainder & drawing measurement

The combined alpha.2 + alpha.3 round, planned with Abbas 2026-07-18 against
the shipped `1.0.0-alpha.1` (+ natural v2). Sources: ROADMAP 3.4 + 3.5
(first half), PLAN-V1's alpha.2/alpha.3 contracts, and the design brainstorm
settled in the ledger below. **One plan, two tags**: the chart half ships as
`1.0.0-alpha.2`, the drawing half as `1.0.0-alpha.3` — combined so the
`format:` engine lands once, beside both its consumers. **Each half opens
with its own Stage 0 SPEC pass; the amended SPEC is the contract**; this
file holds the settled decisions, the build order, and the execution log.
The quality bar is ROADMAP §5, verbatim. Everything is **additive** — the
syntax is frozen; no existing sample or snapshot changes except where a
stage names it.

Scope, chart half (ROADMAP 3.4): the `format:` machinery (all families,
applied to value-axis ticks + tooltips), per-datum paint lists on
`|bars|`/`|dots|`, `scale: time` axes with calendar-aware ticks and
calendar `step:`. Per-datum `labels:` and tooltips already landed in 0.21 —
alpha.2's contract is the remainder. Scope, drawing half (ROADMAP 3.5,
first half): dimension `clearance` (cascade, replaces dim `gap:`),
painted-bounds row packing, linear-dimension inference + `project:` +
aligned dims, boxed datum letters + datum identities, crossing halos,
internal threads in sections, addressable pattern copies, fan leaders,
`format:` on dimensions, and the one-ended-leader **follows** conformance
bug (`bar:m10 <- "LH"` must read `M10×1.5 LH`).

Stages are sized for one session at ~60 % of a context window; sub-agents
per AGENTS.md (always with an explicit `model`). At each stage's end:
fmt/test/clippy clean, a **Log** line here.

---

## Decisions ledger (settled in design review — do not relitigate)

1. **One plan, two tags.** The ladder's alpha.2 and alpha.3 rows stand;
   this doc serves both rounds. Tag `1.0.0-alpha.2` when Stage 3 closes,
   `1.0.0-alpha.3` when Stage 8 closes; pushing stays with Abbas.
2. **One format engine, beside the ledger.** `src/ledger/format.rs`: a
   `Format` value — `auto | decimal N | significant N | scientific N |
   engineering N | percent N | fraction D` + date presets — parsed once
   at resolve, registered as an **ordinary inherited ledger property**
   (chart/drawing scope sets the default; axis, series, tooltip text, and
   later a dimension rule or block override). One
   `render(value, &Format) -> String` (+ a date arm) consumed by chart
   ticks (today's `fmt` in `layout/chart/scale.rs`), tooltips, and — in
   Stage 8 — `drawing/compose.rs`. `format:` is presentation only, never
   measurement; it composes **before** `unit:` suffixes, tolerance, the
   `⌀`/`R`/`°` glyphs, and `pattern:` counts.
3. **Time is epoch seconds, `f64`, UTC.** Quoted ISO-8601 literals in
   `data:` points and `range:` (and `ticks:` — the same literal reader);
   in-house civil-date math (days-from-civil; no chrono — the crate stays
   near-zero-dep). A bare date is midnight UTC; an offset keeps its
   instant; the renderer is timezone-independent (all math in UTC). Mixed
   date/numeric domains error; an invalid date errors with its span.
   Time-only literals are **not** in 1.0 — a numeric axis covers them.
4. **`Scale::Time` owns calendar ticks.** Auto picks the boundary unit
   from the domain span (years → months → weeks → days → hours →
   minutes); calendar `step:` overrides — `step: month`, `step: 2 week`
   (unit idents: `year month week day hour minute`, optional count).
   Numeric `step:` on a time axis errors, pointing at the calendar form.
   The auto `format:` preset follows the tick unit (years tick as `2026`,
   months as `Jan 2026`, …); an explicit date preset wins.
5. **`fraction D` renders the drafting stack** — superscript numerator,
   slash, subscript denominator, riding the same raised/lowered text
   machinery as `tol:` deviations. Value = nearest multiple of `1/D`,
   whole part leading (`1 ³⁄₈`-style); `D` a positive integer.
6. **Per-datum paint is the comma law on `|bars|`/`|dots|` only.**
   `fill:`/`stroke:`/`opacity:` accept comma lists; `auto` = the
   palette-derived paint that datum would get anyway (walk + deep-edge
   rules apply per datum, so `stroke: auto` still deepens each datum's
   own fill). List count ≠ data count errors with both counts. A list on
   `|line|`/`|area|` errors (no ambiguous interpolation);
   `|slice|`/`|bubble|`/`|mark|` are already per-node. The legend swatch
   keeps the series' base (auto) paint.
7. **Dim `clearance` replaces dim `gap:`** — `gap` stays on mates (signed
   separation). Cascade: drawing default → `(-)`/`(o)`/`(<)` family rule →
   descendant/class → the dimension's block. A **minimum, not a
   coordinate**: the packer may go farther out to clear rows, text, and
   frames; a per-dimension value is honored independently; `translate`
   stays the exact nudge. The fixed `DIM_OFFSET + k·DIM_PITCH` generator
   dies: row offsets derive from **painted annotation bounds +
   clearance** (the `Rows` packer already carries `obstacles`/`blocked()`).
8. **Linear inference + `project:`.** Two point anchors → the true
   **aligned** distance (aligned dims un-deferred); point + directed
   side/edge → along the directed normal; two parallel directed → their
   shared normal; two non-parallel directed → error suggesting `(<)`.
   `project: horizontal | vertical | aligned` overrides. An aligned dim
   defaults to the side of its span facing **away from the geometry
   centre** (Stage 0b defines "centre" precisely — the sketch union's
   bbox centre is the lean).
9. **Pattern copies are addressable**: `plate.bolt.2` — 1-based, grid
   copies row-major from the seed, radial copies clockwise from bearing 0;
   carrier-path-only (no leaked ids — `bolt.2` alone stays an unknown-id
   error). Dimensions measure **true model positions** on the unbroken
   model; segments read through `break:` compression; leaders land on the
   displayed copy. The bare carrier keeps its seed/centre meaning and its
   `N×` prefix. Grammar: the lexer's `.`+digit glue rule + `parse_endpoint`
   — the round's one front-end change.
10. **Datum letters are identities.** `body:seat >- "A"` keeps its syntax;
    the letter lowers into the standard framed datum box at the landing
    (sheet-space, obstacle-aware, riding the leader's text seat). Letters
    are collected per drawing scope — duplicates error; alpha.4's
    feature-control `datums:` will validate against this set. Bare letters
    are references, per "bare when referenced".
11. **Fan leaders**: `a & b <- "2× R5"` — resolve allows `&` on one-ended
    leader ops; **one** text and landing (the first endpoint steers,
    `side:` overrides), independent ray-cast legs sharing what trunk
    geometry permits; an unroutable leg is reported, never silently
    dropped.
12. **Crossing halos are mask-based** (an understroke breaks over hatching
    and in dark mode): generated sheet-space knockouts under
    dimension/extension/leader linework where it crosses **geometry**,
    generalizing the existing `label_mask`. Never over arrowheads, text,
    frames, or the contact region (tip/landing). Halo margin is a baked
    sheet constant (Stage 0b settles the number by eye); one cascade hook
    restyles or removes them.
13. **Internal threads**: `thread:` on an inner even-odd subpath flips the
    material side — major/minor and offset reverse; callouts compose the
    internal spec from the same numbers. No new property.
14. **The follows bug is conformance, not design**: SPEC 15.6 already says
    a one-ended label **follows** the composed value; today an authored
    label suppresses the thread auto-compose. Fixed where compose owns it.
15. **This round only.** alpha.4+ explode into their own docs at entry;
    nothing here reserves syntax for them (feature-control validation of
    `datums:` explicitly waits).

---

## Stages — chart half (→ `1.0.0-alpha.2`)

### Stage 0a — SPEC amendment: charts & `format:`

Write all chart-half law before any code. The SPEC alone must suffice to
implement Stages 1–3.

- [ ] SPEC 14.4: `scale` gains `time`; date literals in `range:`/`ticks:`;
  calendar `step:` (unit idents, optional count, numeric-step error); tick
  presentation via `format:` (auto follows the tick unit); the
  mixed-domain and invalid-date error rows referenced.
- [ ] SPEC 14.3: quoted ISO-8601 x-values in `data:` points (`data:
  "2026-01-01" 18, …`) — the item-width discriminator unchanged; bare
  date vs offset semantics (decision 3).
- [ ] SPEC 14.6: per-datum paint lists (decision 6) — the comma law on
  repeated-mark series, `auto`, the count law, the legend-swatch rule.
- [ ] A `format:` entry in SPEC 16's ledger section: families, arguments,
  inheritance, owners (chart scope, axis, series; dimensions noted as
  Stage-8/15.6 territory), presentation-only law, compose order
  (decision 2).
- [ ] SPEC 20 rows: paint-list count mismatch (both counts in the
  message), list paint on `|line|`/`|area|`, mixed date/numeric domain,
  invalid date literal, numeric `step:` on a time axis, invalid `format:`
  arguments.
- [ ] SPEC 23 prunes: time axes and `format:` come out of deferred; 15.6's
  "a per-value suffix arrives with `format:`" pointer updated to cite the
  ledger entry (suffixes themselves are `unit:`'s job, unchanged).
- [ ] ROADMAP/PLAN-V1 sync: ladder rows point here; alpha.2/alpha.3
  contract sections gain their round-entered note.

Acceptance: SPEC alone sufficient for Stages 1–3; anchors intact; every
example uses shipped syntax; `cargo test` untouched.
**Log:**

### Stage 1 — the `format:` engine, on ticks & tooltips

- [ ] `src/ledger/format.rs`: the `Format` value (decision 2 families),
  the value-shape parser (comma law: `format:` is one item — family +
  args space-separated), `render(f64, &Format) -> String` with exact,
  deterministic formatting (no locale, `.` decimal point, minus sign
  `-`); `fraction D` via the raised/lowered stack (decision 5) — the
  composed runs surface so consumers can stack them.
- [ ] Ledger registration: `format` as an inherited property; owner
  validation rows (chart scope, `|axis|`, series). Drawing owners arrive
  in Stage 8 — until then a `format:` on a statically-known drawing owner
  **errors** per the owner rule (not yet an owner — the same law as any
  misplaced property; no special case).
- [ ] Chart consumption: value-axis tick text routes through `render`
  (today's `fmt` in `scale.rs` becomes `format::render(v, &Format::Auto)`
  — byte-identical default output, snapshot-proven); tooltip value text
  the same; `unit:` still appends after.
- [ ] Unit tests per family (boundaries: zero, negatives, rounding ties,
  engineering exponent snapping, percent scaling, fraction rounding);
  snapshot: a chart with `format: percent 1` on its value axis.

Acceptance: default output byte-identical everywhere (`Auto` = today's
trim rule); every family unit-tested; `format:` inherits chart → axis.
**Log:**

### Stage 2 — per-datum paint

- [ ] `layout/chart/model/paint.rs` + `series.rs`: `fill:`/`stroke:`/
  `opacity:` list reading on `|bars|`/`|dots|` (comma law; single value =
  today's path, untouched); the `auto` sentinel resolves per datum
  through the existing walk + `fill_outline` deep-edge rules; count
  mismatch and line/area list errors (SPEC 20 wording).
- [ ] Legend swatch keeps the base paint (decision 6); per-datum `<title>`
  / tooltip text unchanged.
- [ ] Sample: `charts.lini` (or `chart_advanced.lini` — whichever cluster
  fits) gains one paint-list-highlighted bar series; snapshots; PNG
  eyeballed light + dark (the highlight must read in both).

Acceptance: unlisted series byte-identical; the highlighted-bar sample
reads in light + dark; both error rows fire with counts in the message.
**Log:**

### Stage 3 — time axes; alpha.2 closes

- [ ] Date parsing: ISO-8601 (`YYYY-MM-DD`, optional `THH:MM[:SS]`,
  optional `Z`/`±HH:MM`) → epoch seconds `f64`; in-house civil-date math
  (days-from-civil + inverse), unit-tested against known dates (epoch,
  leap years, century rules).
- [ ] `Scale::Time` in `layout/chart/scale.rs`: domain fixing from
  date-literal data/range, calendar tick generation (auto unit from span;
  calendar `step:` override), tick labels via `format::render_time`
  (auto preset follows the tick unit; explicit preset wins).
- [ ] Series/axis plumbing: quoted x-values in `data:` points feed the
  time domain; mixed date/numeric errors; `range:` and `ticks:` accept
  the same literals; tooltips show the formatted instant.
- [ ] Samples: a time-series line in `charts.lini`/`chart_advanced.lini`
  (cluster policy — no new file); snapshot matrix pinning tick text
  across zoom-y domains (minutes → hours → days → months → years).
- [ ] Visual pass: PNG light + dark; alpha.2 ladder row confirmed; bump
  `1.0.0-alpha.2`, tag (push deferred to Abbas).

Acceptance: tick text pinned across the domain matrix; calendar `step:`
honored; date errors carry spans; determinism (rerun equality) holds.
**Log:**

---

## Stages — drawing half (→ `1.0.0-alpha.3`)

### Stage 0b — SPEC amendment: drawing measurement

Write all drawing-half law before any code. The SPEC alone must suffice
to implement Stages 4–8.

- [ ] SPEC 15.6 rewrite: `clearance` cascade replacing dim `gap:`
  (decision 7 — minimum-not-coordinate, bounds-derived packing);
  linear-dimension inference + `project:` (decision 8), aligned dims
  un-deferred, the away-from-centre default with "centre" defined; the
  concise side override wording relative to endpoint order; `format:` on
  dimensions (compose order vs `unit:`/`tol:`/glyphs — the machinery
  cites Stage 1's ledger entry).
- [ ] SPEC 15.7: boxed datum frames + datum identities (decision 10), fan
  leaders (decision 11), crossing halos (decision 12 — margin constant
  named in SPEC 10.5's baked table).
- [ ] SPEC 15.4: pattern copy addressing (decision 9); SPEC 15.3:
  internal-thread sense (decision 13); SPEC 21: the endpoint grammar
  gains the numeric segment; SPEC 15.6's follows rule confirmed against
  the thread-spec compose (decision 14).
- [ ] SPEC 20 rows: perpendicular-directed pair (suggest `(<)`), unknown
  copy index (with the copy count), duplicate datum letter, fan-leader
  misuse (`&` on two-ended ops unchanged), `project:` vs directed-anchor
  conflict, numeric `gap:` on a dimension (points at `clearance`).
- [ ] SPEC 23 prunes: aligned dims, fan leaders out of deferred.

Acceptance: SPEC alone sufficient for Stages 4–8; anchors intact;
examples use shipped syntax; `cargo test` untouched.
**Log:**

### Stage 4 — dimension `clearance` & painted-bounds packing

- [ ] Ledger: `clearance` gains its dimension owner (the property exists —
  routing's); dim `gap:` removed with the SPEC 20 pointer error.
- [ ] `drawing` dim packing: `Rows` offsets derive from painted annotation
  bounds + clearance (decision 7); `DIM_OFFSET`/`DIM_PITCH` die;
  per-dimension `clearance` honored independently; `translate` untouched.
- [ ] Existing drawing samples re-blessed **once**, deliberately (offsets
  move); PNG diff eyeballed — rows must sit tighter/looser only where
  bounds say so, never overlap.

Acceptance: no dim row overlaps any painted annotation across all drawing
samples (oracle-style check if cheap — PLAN-V1's ask); cascade proven
(drawing default, family rule, per-dim override each pinned).
**Log:**

### Stage 5 — inference, `project:` & pattern copy ids

- [ ] Front-end: lexer `.`+digit glue + `parse_endpoint` numeric segments
  (decision 9); resolve maps copy indices to placed copies (grid
  row-major, radial clockwise), unknown index errors with the count.
- [ ] `drawing/dims.rs` + `compose`: inference rules + `project:`
  (decision 8); aligned dims (rotated extension/dim lines, ISO-aligned
  text via the existing rotate machinery); away-from-centre side default.
- [ ] Dims measure true model positions on the unbroken model; `break:`
  interplay pinned (a dim across a break reads the true span; the
  breakline stays).
- [ ] Samples: `drawing_screw`/`drawing_sheet`/`drawing_annotations` gain
  an aligned dim, a `project:` override, and a `plate.bolt.2`-style
  copy-addressed dim (extend, no new files); snapshots + PNG pass.

Acceptance: all four inference rules pinned (incl. the `(<)` suggestion);
copy dims measure model truth; aligned text reads bottom/right per ISO.
**Log:**

### Stage 6 — datum identities & fan leaders

- [ ] Datum letters: a resolve-scene pass beside `id_seen` collects `>-`
  letters per drawing scope (duplicate errors); the letter lowers to the
  framed box at the leader landing, obstacle-registered for Stage 4's
  packer.
- [ ] Fan leaders: resolve allows `&` on one-ended leader ops; one
  text/landing steered by the first endpoint (`side:` overrides);
  independent ray-cast legs; unroutable legs reported.
- [ ] Samples: a datum + fan leader in the drawing keepers; snapshots +
  PNG pass.

Acceptance: datum boxes framed per the standard look, never overlapping
(packer obstacle proven); fan legs land on both features; duplicate
letter and misuse errors fire.
**Log:**

### Stage 7 — halos, internal threads & the follows bug

- [ ] Halos: mask-based knockouts generalizing `label_mask` (decision 12)
  — applied to dim/extension/leader linework over geometry; the
  exclusion set (arrowheads, text, frames, contact) proven by
  construction, not clipping fixes; one cascade hook; margin constant in
  the baked table.
- [ ] Internal threads: the even-odd inner-subpath sense flip
  (decision 13) in `threads.rs`; callouts compose the internal spec.
- [ ] The follows bug (decision 14): a one-ended leader's authored label
  follows the composed thread spec; conformance test from SPEC 15.6's
  own example.
- [ ] Samples: hatching-crossed dims in `drawing_section` show the halo;
  an internal thread in the section sample; snapshots + PNG light +
  dark (the mask must hold in dark mode — the halo's whole reason).

Acceptance: halos break linework over hatching in both themes; no halo
touches an arrowhead/text/frame; internal callouts read the internal
spec; `bar:m10 <- "LH"` renders `M10×1.5 LH`.
**Log:**

### Stage 8 — `format:` on dimensions; alpha.3 closes

- [ ] Ledger: `format`'s drawing owners (drawing scope, dim family rules,
  the dimension's block); `compose::compose` routes the measured number
  through `format::render` before `tol:`/glyphs/`pattern:` (decision 2's
  compose order); the auto default stays today's ≤ 2-decimals trim —
  byte-identical unformatted output.
- [ ] Sample: one `format: decimal 2` (or `fraction 8` — the showroom
  pick) dim in the keepers; snapshots.
- [ ] Full drawing visual pass (ROADMAP §5): every drawing sample at 1:1
  + a detail scale, light + dark, screen + print size.
- [ ] Ladder row confirmed; bump `1.0.0-alpha.3`, tag (push deferred).

Acceptance: unformatted dims byte-identical; formatted dim composes in
the documented order; the full visual pass logged.
**Log:**

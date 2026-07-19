# Lini 1.0 Roadmap

The master document for taking Lini from pre-release to 1.0. It records the
**settled decisions** — what each feature is and why — and the **version ladder**
that sequences them. It contains no implementation stages: those live in the plan
files it points to.

| File | Role |
|---|---|
| `SPEC.md` | The law. Amended per round (each round's Stage 0), tightened once before 0.21. |
| `AUDIT.md` | Codebase findings + refactor decisions (2026-07-10). Feeds PLAN-ALPHA; deleted when its stages land. |
| `PLAN-ALPHA.md` | Detailed, session-sized stages: refactor → SPEC tightening → the 0.21 breaking round. Ends with the syntax frozen at `1.0.0-alpha`. |
| `PLAN-V1.md` | The alpha/beta/rc rounds — contract + acceptance per round; each round explodes into its own detailed plan doc at entry (the DRAWING-0.1x pattern). |

Lini is unreleased: no backward compatibility is owed. Every breaking change lands
**once, coherently, in 0.21** — SPEC, implementation, formatter, samples, snapshots,
diagnostics together. Removed spellings are not kept as aliases.

---

## 1. Principles (frozen)

- **One core language.** Flow, grid, tree, sequence, charts, and drawings are layout
  engines over one node/link/cascade model. A new family adds types, properties,
  roles, and lowering — never a second document language.
- **One mechanism per concept.** A visual body is a node; a relationship is a link;
  seating without a drawn relationship is `||`; repetition uses the one list
  grammar; paint is `fill`/`stroke`/`color`/`opacity` everywhere. Layout places,
  routing shapes connections after placement.
- **Reuse properties by meaning, not by count.** A property is shared only when its
  plain-language definition survives every owner.
- **Deterministic and inspectable.** Same input → byte-identical output. Sugar stays
  visible through `lini desugar`; canonical syntax through `lini fmt`; every
  inferred decision is deterministic or has an explicit override.
- **No silent author mistakes.** Lini is a compiler: an unknown property, impossible
  value, misused owner, or dropped construct never disappears silently.

## 2. What 1.0 promises (the stability contract)

- **The language is stable.** Syntax, property names, value shapes, defaults, and
  documented behavior do not break until a 2.0. Additive growth (new types,
  properties, layouts) arrives in minors.
- **Determinism is per-version.** Within one version, the same input produces
  byte-identical SVG, always. Across versions, output bytes may improve (better
  routing, nicer chrome) in minors — pinned snapshots are a per-version tool, not a
  cross-version promise.
- **The theming surface is API.** `--lini-*` variable names, the `.lini-*` class
  scheme, and the SVG top-level structure (SPEC 17) are stable; hosts may build on
  them.
- **Diagnostics codes are stable** once assigned (beta); messages may improve.

---

## 3. Settled decisions

### 3.1 Language hardening (the 0.21 breaking round)

**Property validation — strict where the owner is known, lenient where a class is
polymorphic.**
- Unknown property name → **error**, everywhere (even in a class) — no owner accepts
  it. With a did-you-mean suggestion.
- Known property misused where the owner is statically known — an instance's own
  block, an element rule (`|box| { }`, `|-| { }`), an id rule, a descendant rule's
  tail — → **error** with a contextual correction.
- Known property in a `.class` rule → CSS semantics: inert on wearers that can't use
  it; a **warning** when it's dead for every wearer; unused classes warn.
- Malformed values (wrong arity, out of range) → **error**, owner-independent.
- Validation, defaults, and value shapes read one ledger (`src/ledger/`, AUDIT D1);
  the schema and generated docs read the same ledger, so they cannot drift.

**The comma law.** CSS's rule, stated once:
- a **comma** separates repeated list items — `data: 2, 3, 4`,
  `columns: 80, 140, auto`, `fill: auto, --red, auto`, `categories: "Q1", "Q2"`,
  `along: 0.2, 0.5, 0.8`, `align: start, center, end`, `fn: (u*10), 5, ramp(2)`,
  `thread: left 1.5, right 1.5`, `break: -90 -30, 30 90`;
- a **space** separates the components of one item, tuple, interval, or shorthand —
  `padding: 4 8`, `translate: 10 -4`, `range: 0 100`, `cell: 2 1`,
  `data: 10 20, 30 40`;
- a **pipeline of calls that fold into one value stays space-separated** (CSS
  `transform`/`filter` precedent) — `draw: move(0,0) up(8) fillet(2)`, `mirror:`
  (its items each reflect the union so far — a fold).
Enforcement lives in the list readers (targeted errors like "`data` takes
comma-separated values"), not the parser, which already preserves the comma/space
shape (AUDIT D7). Samples migrate by hand, once.

**Implicit-node warning — similarity, not scope-mixing.** An endpoint that
auto-creates warns only when its id is a near-miss (edit distance ≤ 2, or case-fold
equal) of a name already known in that scope — declared *or* previously
auto-created. Catches `cta -> bird` even in an all-implicit file; stays silent for
legitimate mixed use (a sequence with one styled participant). Multi-segment paths
remain never-created errors; the existing shadow warning stays.

**Text wrapping.** `max-width` + `text-wrap: wrap | nowrap` (default `wrap`; inert
without a finite `max-width`). Wrapping prefers whitespace, falls back to grapheme
breaks so the no-clip/no-spill law holds; `nowrap` + can't-fit is a compile error;
a non-text child wider than `max-width` is a layout error; `width > max-width` is
invalid. Wrapped measurement feeds auto-size, tracks, gutters, spacing, labels, and
routing obstacles.

**Line alignment rides `align` — there is no `text-align`.** The table rule
generalized: a text leaf's lines align per its **nearest container box's horizontal
packing knob** — `align` in column/grid contexts, `justify` in a row — mapped
`start/center/end` (stretch/evenly/origin read as center). Applies even without
slack; each box is a container, so the box holding the text decides. Default center
everywhere (today's output unchanged). Split intents use a wrapper
`|block| { align: … }`.

**Drawing scale becomes human.** Three quantities, two authored (AUDIT-verified
desugar fold):
- `scale:` — the **drafting ratio**, per view. Default 1 (`scale: 2` = 2:1,
  `scale: 0.5` = 1:2). Composes section/detail/view titles.
- `unit:` — the **physical size of one drawing unit**: `mm` (default), `cm`, `m`,
  `in`. Inherits nearest-wins (set once on the page). Semantic only in drawing
  scopes — a `|sketch|` in a flow diagram stays pixel-space (`right(300)` = 300px).
- **density** — px per mm, default 4, root-level and non-semantic (raster/screen
  resolution only; print is true-scale regardless). Never per-node; ppu is always
  derived (`unit-mm × density`), never authored.
`|page|` loses its `scale:`. Magnitude is `scale:`'s job — a 5 m beam is
`scale: 0.02` (1:50), never a density fudge; an absurd rendered extent gets a hint
diagnostic. Desugar folds ratio × unit × density into the engine's existing
internal px-per-unit, so the core stays dumb and `lini desugar` shows the number.

**Sequence note placement.** One `place:` property replaces `over`/`left`/`right`:
`place: over api db`, `place: left api`. One mode per note; old properties removed.

**Renames** (no aliases): series `tags:` → `labels:` (reconcile the existing
`labels` name first — AUDIT R1); title-block `dwg` → `drawing-number`, `rev` →
`revision`, `sheet` → `sheet-number`, `dept` → `department`, `doc-type` →
`document-type`; the title-block **smart label becomes the title field** (lowers to
the same generated spanning cell; a label or any field property selects
structured-field mode; neither → the plain-table form).

**`format:`** — presentation only, never measurement: `auto`, `decimal N`,
`significant N`, `scientific N`, `engineering N`, `percent N`, `fraction D`, and
date/time presets for time axes. Inherits from the chart/drawing; axis, series,
dimension rule, or one dimension overrides. Composes before `unit:`, tolerance,
`⌀`/`R`/`°` glyphs, and pattern counts. (Additive, so it lands with its first
consumer in alpha.2, dims following in alpha.3 — the *decision* is settled here.)

**Hardening fixes in the same round** (AUDIT R6 + local bug list): the
root-drawing router gap (links in nested flow/grid children of a root
`{ layout: drawing }` are silently dropped today — one-line fix; same latent gap
in the root-sequence arm); scoped note rules move to desugar so the teaching view
stops lying; dead `--standalone` flag removed; row-direction chart bands/marks
built (see 3.4); and four local fixes:
- **Chain ops mark every hop.** `a -> b -> c` desugars to `a -> b; b -> c` — each
  hop carries the operator's full markers (today the first hop draws unmarked; an
  author who wants a bare first hop writes `a - b -> c`). Same for
  `a <- b <-> c`. Chain expansion moves to **desugar** (it's pure sugar,
  inspectable); fan-out stays resolve/routing (shared-trunk geometry is a
  different thing, not sugar).
- **`|page|` direction follows orientation**: landscape defaults
  `direction: row`, portrait `direction: column`.
- **`stroke-style: wavy` is link-only, by design.** The closed-primitive/drawing
  deferral is dropped from SPEC 23 entirely — it will never be built.
- **`fmt` preserves a table cell's style block** (`"Apple" { color: --red-ink }`
  is legal and renders, but fmt silently deletes the block today — data loss). A
  row containing a styled cell leaves the aligned-columns grid; unstyled rows
  stay aligned.

### 3.2 Tree layout and mindmaps

*(Amended in the alpha.1 design review, 2026-07-11 — `radial` renamed
`bilateral`: a chart's `radial` is truly circular, and the mindmap arrangement
every tool actually draws is bilateral; `radial` stays unclaimed for a possible
true ring tree post-1.0. Full decisions ledger: `PLAN-TREE-alpha1.md`.)*

- **`layout: tree`** with `direction: row | column | bilateral`. Rooted
  parent/child structure; source order = sibling order; no multi-parent/cyclic
  layout (that's the deferred DAG engine). `gap`: generation distance × sibling
  separation (transposed by direction). **Bilateral** = root centred, the first
  ⌈n/2⌉ first-level topics right top-to-bottom, the rest left; a first-level
  `side: left | right` overrides.
- **`|topic|`** (over `|block|`) separates structure from content: a direct
  `|topic|`-derived child is a branch; every other child is the topic's own visual
  content (icons, badges, tables, charts). Custom structural types derive from it
  (`|person::topic|`). `|topic|` outside a tree errors; a tree needs **exactly one
  root topic** (forest = error now; relaxing later is non-breaking). Topics are
  boxed at every depth and wear generated `.lini-level-N` classes — the one hook
  for the mindmap ramp and user tier-restyling alike.
- **Branch links are generated at desugar** (AUDIT D2) as ordinary unmarked `|-|`
  links **resolving in the parent topic's scope** — so `#syntax |-| { stroke: … }`
  styles one subtree's branches, `lini desugar` shows them, and the router routes
  them like any wire. Authored cross-links remain legal, never alter the tree,
  and keep the neutral link default (the walk never tints them).
- **`|mindmap|`** preset: a visible root topic, `layout: tree; direction:
  bilateral; routing: natural` — plus the **palette walk**: each first-level
  branch takes the next hue (declaration order, red and grey skipped) and tints
  its subtree at the outlined tiers — `wash` fill, `deep` stroke and branch
  links, `ink` text; the root stays neutral; explicit paint wins. Deterministic,
  themeable, dark-mode-free. A depth ramp (root largest, deeper smaller) and
  `max-width: 160` topic wrap ride the preset. Plain `layout: tree` stays
  neutral (org charts read monochrome); its default routing is the global
  `orthogonal` (elbow connectors).

### 3.3 Routing

- 1.0 strategies: **`orthogonal`** (default, ROUTING.md), **`natural`** (new),
  **`straight`**. `curved` is replaced by `natural`, not aliased.
- **`natural`** = obstacle-aware smooth curves: choose a legal topological corridor
  through free space (reusing the channel graph), then fit a curve inside it —
  never a rounded illegal straight line. Shares the spine (requests, forced sides,
  markers, labels, bundles, fans, self-links, strays, reports, determinism) and
  honours the observable laws where geometrically applicable (tangent-normal
  endpoints, keep-out clearance, clean crossings, separated duplicates, sliding
  labels, honest strays). No `tension`/`bend`/`curvature` knobs in 1.0.
- Layout and routing stay independent: flow/grid/tree accept all three strategies;
  sequence and drawing keep their layout-owned wiring; charts/pies have no routed
  links; nested ordinary scopes keep their own strategy.
- Implementation seam per AUDIT: `src/routing/natural/{mod,corridor,curve}.rs` +
  five named touches to shared files (extract `build_worlds`, widen the bundle
  filter, exhaustive `match`es on `Strategy`, its own validator, ROUTING.md table).

### 3.4 Charts

- **Per-datum paint** through the comma law on repeated-mark series (`|bars|`,
  `|dots|`): `fill: auto, auto, --red, auto` (`auto` = the palette-derived paint);
  also `stroke`, `opacity`. Count must match the data; `|line|`/`|area|` reject
  list paint (no ambiguous interpolation); `|slice|`/`|bubble|` are already
  per-node.
- **Per-datum `labels:`** (the renamed `tags:`) — count must match explicit
  `data:`; invalid with `fn:`; tooltip behavior decides inline vs hover.
- **Time axes**: `scale: time` on an axis; quoted ISO-8601 values in `data:`
  (`data: "2026-01-01" 18, …`) and in `range:`; ordered numeric domain,
  calendar-aware ticks; bare dates are date-only, offsets keep their instant,
  renderer timezone-independent; mixed date/numeric domains error. `format:`
  controls tick presentation. No external data loading.
- **The direction flip is never silently lossy** (relaxed from "never breaks"):
  bands/marks work in `row` (one-file fix per AUDIT); in `radial` they are a
  **compile error** until implemented. Built in 0.21 with validation, since the
  silent drop is a hardening bug.

### 3.5 Engineering drawings

- **Dimension `clearance` replaces dimension `gap:`** (`gap` stays on mates —
  signed separation). Cascade: drawing default → `(-)` family rule →
  descendant/class → the dimension's block. A minimum, not a coordinate — the
  packer may go farther out to clear rows/text/frames; a per-dimension value is
  honored independently; `translate` stays the exact nudge. Fixed
  `dim-offset`/`dim-pitch` become derived from painted annotation bounds +
  clearance (the `Rows` packer already carries obstacle infrastructure).
- **Linear-dimension inference + `project:`.** Two points → true aligned distance;
  point + directed side/edge → along the directed normal; two parallel directed →
  their shared normal; two non-parallel directed → error suggesting `(<)`.
  `project: horizontal | vertical | aligned` overrides; aligned dims default to
  the side of their span facing away from the geometry centre.
- **Boxed datum letters**: `body:seat >- "A"` keeps its syntax; the letter lowers
  into the standard framed datum box at the landing (sheet-space, obstacle-aware).
  **Datum letters are identities**: collected per drawing scope, so
  feature-control `datums:` references validate (unknown letter = error with
  suggestions); bare letters are references, per "bare when referenced".
  Datum-on-dimension (feature-of-size axis datums) rides the annotation-node seam.
- **`|surface-finish|`** — smart label = the textual indication, `symbol:` ∈
  `basic | machined | prohibited` from the shared drafting-symbol registry (the
  icon machinery generalized). **`||` generalizes to annotation seating**:
  geometry↔geometry keeps mate/ground semantics; annotation↔geometry always moves
  the annotation, after mates resolve, outside the grounding graph; the target
  must supply a directed side/edge; type-defined default seat anchor with explicit
  override; `rotate` before seating, `translate` after. The leader form
  (`body:seat <- sf:bottom`) renders the same node. Bundles (`|column|` of finish
  + frame) seat as one.
- **`|feature-control|`** with semantic `|control|` rows — smart label names the
  characteristic (position, flatness, runout, …; the SPEC round settles the
  canonical current-standard set); `tol`, `zone`, `material`
  (`maximum`/`least` = Ⓜ/Ⓛ), `datums: A maximum, B, C` (primary/secondary/
  tertiary, per-datum modifiers), ordered `modifiers`. Unknown combinations error —
  never a plausible-looking invalid frame. Multiple rows = composite frame.
- **Annotation nodes inside drawing link `[ ]`** — a dimension or leader may carry
  node annotations (frames, finish symbols) that stack at the text seat and count
  as packing obstacles; authored strings keep replace/follow semantics. Core
  routed links stay text-only for 1.0. (The one deep change — the label path is
  text-typed end-to-end today; AUDIT seam table.)
- **Internal threads in authored sections**: `thread:` on an inner even-odd
  subpath flips the material side (major/minor and offset reverse); callouts
  compose the internal spec from the same numbers. No new property.
- **Addressable pattern copies**: `plate.bolt.2` — 1-based, grid row-major from
  the seed, radial clockwise from bearing 0; carrier-path-only (no leaked ids);
  dimensions measure true model positions; the bare carrier keeps its seed/centre
  meaning and its `N×` prefix.
- **Fan leaders**: `a & b <- "2× R5"` — one text/landing (first endpoint steers,
  `side:` overrides), independent ray-cast legs sharing what trunk geometry
  permits; unroutable legs are reported.
- **Crossing halos**: generated sheet-space knockouts under dimension/extension/
  leader linework where it crosses geometry — **mask-based** (AUDIT D5: an
  understroke breaks over hatching and in dark mode), generalizing the existing
  link-label mask; never covers arrowheads, text, frames, or the contact region;
  one cascade hook to restyle/remove.
- **Projection assistance** (authored views only — no 3D, no inference):
  - **Construction lines are authored links**: declare a correspondence between
    two stations and lini draws the thin line — `front:head-top -> side:head-top`
    lowered as `|projection|` chrome (styleable/removable). The one SPEC change:
    cross-view anchor references become legal **only** for straight construction
    links, never dims or mates (sealed bodies stand otherwise).
  - **View arrows** (optional garnish): a letter+arrow marker on the source view;
    `|drawing| { of: <arrow> }` composes "VIEW A (2:1)" exactly like sections and
    details. Can slip to a later minor without touching construction lines.

### 3.6 Images and title blocks

- **Local images**: `src: "./logo.svg"` resolves from the source `.lini` file;
  SVG embeds as a nested `<svg>` **with ids rewritten** (AUDIT D6 — nesting alone
  doesn't isolate ids); raster embeds as base64 data URIs; HTTP(S) and authored
  data URIs unchanged; missing/unreadable paths are source-spanned errors;
  embedding is deterministic from bytes, no network at compile time. The dir
  server's traversal boundary generalizes past `.lini`; file-mode gains the
  boundary it lacks.
- **Title block**: structured-field mode per 3.1's renames + smart-label-as-title;
  authored children remain ordinary cells after the generated ones (`cell`/`span`
  honored, overlap errors). No `logo:` property — a logo is an `|image|` in a
  cell, or anywhere on the page. BOM stays an ordinary `|table|`; no auto-BOM in
  1.0.

### 3.7 Fonts & text fidelity (the 0.21 round)

Today text is measured as monospace (0.6em/char) but the SVG only *names* a
system stack — a proportional override measures wrong, and resvg renders with
whatever font it finds. v1 fixes both with two bundled families and real metrics.

- **Two bundled families** (both SIL OFL 1.1): **Google Sans Code** (mono) and
  **Google Sans** (proportional) — four **static** roman weights each
  (Regular/Medium/SemiBold/Bold = 400/500/600/700). No variable fonts: the
  3-axis Google Sans VF is 4.6MB and needs axis-pinning; the statics are exact
  instances and the pure-Rust subsetter works on them directly.
- **Metrics are always compiled in** — never feature-gated (layout must not
  depend on build flags). `xtask extract-fonts` generates per-glyph advance
  tables + ascent/descent/cap-height per family × weight from the raw statics
  (**committed** at `assets/fonts/raw/` so regeneration is reproducible even if
  Google re-versions the download; cargo-package-excluded so the published
  crate stays lean), plus **subset TTFs** (Latin +
  Latin-1 + Latin-Ext-A + punctuation + the drafting symbols lini composes),
  committed and gated behind a default-on `font` cargo feature. **Budget:
  shipped font payload ≤ 600KB total** — trim charset first, weights second.
- **Measurement**: width = Σ per-glyph advances at the compile-resolved weight;
  no kerning/shaping in v1 (~1% error, documented). **Metrics follow the kind,
  not the name**: a user `font-family` override changes only the emitted name;
  the table is picked by kind (known-mono list + a "mono" substring heuristic →
  mono, else proportional). Unknown glyphs fall back to a fixed advance (wide
  for CJK ranges); out-of-charset text falls through to system fonts per-glyph
  in browsers. The mono table is exactly 0.6em/char, so existing diagrams
  measure identically. Vertical centering upgrades from font-size guesswork to
  cap-height optical centering (one deliberate output change, re-blessed once).
- **Weights**: `font-weight: normal | medium | semibold | bold | 400 | 500 |
  600 | 700` (normal→400, bold→700). Arbitrary 100–900 stays deferred.
  Measurement uses the compile-resolved weight; a runtime CSS override keeps
  the compiled layout box — the same caveat as a family override. Whether
  chrome (titles/headers) retunes from bold to semibold is decided in the
  stage's visual pass (mono advances are weight-invariant, so it's
  layout-neutral).
- **The default stays monospace.** Proportional is one declaration away
  (`font-family: "Google Sans"`), and the metrics flip costs existing diagrams
  nothing.
- **Three output modes** (the pivotal constraint, verified on resvg 0.47:
  resvg/librsvg **ignore `@font-face`**):
  - **default** — names only, zero bytes; the stack leads with the bundled
    family names so installed/hosted copies engage.
  - **`--embed-font`** — base64 `@font-face` of the family × weights actually
    used (~100KB each). Browser-faithful, self-contained; documented as
    browser-only. Embedded faces use Lini-scoped family names so they never
    collide with a user's installed versions.
  - **`--static`** — renames `--bake-vars`, no alias kept: bakes the vars *and*
    outlines text to paths (glyphs deduped via `<defs>`/`<use>`; italic as
    synthetic oblique). Faithful **everywhere** — including our own
    resvg-rendered visual reviews, which should use it from this stage on.
- License obligations: both OFL texts in `LICENSES/` (`google-sans`,
  `google-sans-code`), copyright lines kept in the subsets.

### 3.8 AI and tooling readiness (beta)

- **Machine-readable schema** generated from the ledger (types, templates, roles,
  inheritance, properties, value shapes, defaults, owners, list-vs-tuple arity,
  layout/routing compatibility, required/exclusive sets, deferred flags, one
  example each). The ledger is also what validation reads — no drift by
  construction.
- **Structured diagnostics**: stable codes, severity, span, related span,
  suggestions, safe machine-applicable replacements; JSON output mode
  (serde-free, AUDIT D9). Human LSP-style output stays the default.
- **Compact generated reference** for tools/AI from the same ledger.
- **Editor grammars**: VS Code + Zed syntax highlighting, keyword lists generated
  from the ledger.
- **`fmt`** adopts every 1.0 decision; all samples formatter-idempotent; error
  messages show canonical syntax.

---

## 4. The version ladder

Every breaking change lands **before** the alpha flip; everything after is
additive. Each version = one focused round: SPEC amendment first, then a bounded
plan, then code + samples + snapshots + visual review together.

| Version | Round | Contents |
|---|---|---|
| **0.21** (spills into 0.22 if needed) | Refactor + hardening + migration (`PLAN-ALPHA.md`) | AUDIT work packages (ledger, shared helpers, constants, render chokepoint, splits); SPEC tightening pass; sample consolidation (~50 → ~25, the showroom bar); then the breaking round: comma law, validation, similarity warning, wrap + line-align, scale/unit/density, `place:`, renames, **fonts** (metrics, subsets, `--static`/`--embed-font` — 3.7), root-drawing/sequence router fix, row bands/marks, radial band error. Intermediate releases: 0.20.1 after the refactor phase, 0.21.0 with the breaking core, 0.22.0 at the freeze. Ends: **syntax frozen**, tag `1.0.0-alpha`. |
| **1.0.0-alpha.1** ✓ (2026-07-12) | Tree & natural | `layout: tree`, `|topic|`, `|mindmap|`, desugar branch links, palette walk; `routing: natural`. |
| **1.0.0-alpha.2** ✓ (2026-07-18, released with the drawing half below as one `1.0.0-alpha.2`) | Charts (`CHART-DRAW-alpha23.md`, combined round) | per-datum paint + labels, time axes, the `format:` machinery + axes/tooltips. |
| **1.0.0-alpha.3** ✓ (2026-07-18, shipped inside `1.0.0-alpha.2` — versions renumber: the alpha.4 round releases as `1.0.0-alpha.3`) | Drawing measurement (`CHART-DRAW-alpha23.md`, combined round) | dimension clearance + bounds-derived packing, inference + `project:`, boxed datums + datum identities, halos, internal threads, pattern copy ids, fan leaders. |
| **1.0.0-alpha.4** (releases as `1.0.0-alpha.3` — the renumber above) | Drafting symbols (`GDT-alpha4.md`) | glyph registry, `|surface-finish|`, `|feature-control|` + `|control|`, `|datum|`, `||` annotation seating, annotation nodes in link `[ ]` (`format:` on dimensions shipped with alpha.3). |
| **1.0.0-alpha.5** ✓ (2026-07-19, released as `v1.0.0-alpha.4`) (releases as `1.0.0-alpha.4` — the renumber above) | Sheet & views (`SHEET-alpha5.md`) | local images, title-block finish, projection construction links (view arrows → section 6). |
| **1.0.0-beta.x** | Tooling & docs | schema, structured diagnostics, generated reference, editor grammars, README/docs refresh. Feature-complete. |
| **1.0.0-rc.x** | Stabilize | bug fixes only; every sample re-verified visually; the stability contract (section 2) goes into SPEC. |
| **1.0.0** | Release | — |

## 5. Quality bar (every round)

- SPEC amendment complete before implementation; one plan doc per round in the
  repo root; one canonical sample per feature; `insta` snapshots for every output
  shape and diagnostic family; formatter/parser/resolve/layout/desugar/render/
  determinism coverage as applicable; routed output validated against the routing
  laws; SVG rendered to PNG with `resvg` and visually inspected (light + dark
  where paint is involved); drawing features checked at multiple view scales and
  on a physical page; no new silent behavior; no duplicate lowering paths;
  `cargo fmt` + `cargo test` + `cargo clippy` clean before release commits.

## 6. Deferred beyond 1.0

Valid directions, deliberately outside the release contract — they must not
distort the features above or reserve premature syntax:

- **Automatic graph/DAG layout** (multi-parent, cycles; layered/Sugiyama). Accepts
  orthogonal/natural/straight when it lands. No `layout: auto` catch-all.
- **Sequence extensions**: `par`/`ref` fragments, create/destroy lifelines,
  found/lost messages, grouping, explicit activations, delays, numbering.
- **Chart families & data**: external CSV/JSON, gauges, stacked areas,
  multi-ring pie/sunburst, per-segment line/area paint, exploded slices, richer
  polar controls.
- **Drawing variants**: slots, blind holes, counterbores/countersinks,
  repeated-segment counting, baseline/ordinate dimension systems, auto-BOM,
  exploded mates, deeper sourced-view nesting, view-letter arrows
  (`of: <arrow>` — title sugar; construction links shipped in alpha.5),
  full orthographic/3D projection.
- **Imports/modules/namespaces** for shared themes and part libraries.
- **Core/rendering**: animation; native PNG/WebP export (design notes: rasterize
  the `--static` SVG via resvg→tiny-skia, PNG encode built-in, WebP lossless via
  `image-webp`, behind a `raster` feature; needs a `--scale`/`--width` knob; a
  raster bakes one theme — auto dark/light is lost); oklch output flag;
  arbitrary numeric font weights beyond the 400–700 set and kerning-aware
  measurement (the 3.7 metrics ship without shaping); gradient text; non-rect
  radius; richer accessibility metadata; `w`/`h` ambient in pen expressions.

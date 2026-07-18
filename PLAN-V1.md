# PLAN-V1 — from `1.0.0-alpha` to `1.0.0`

The feature rounds after the syntax freeze (`PLAN-ALPHA.md`). Decisions are in
`ROADMAP.md` section 3; implementation seams in `AUDIT.md`'s readiness table.
Everything here is **additive** — no breaking changes past the alpha tag.

This file holds each round's **contract**: scope, SPEC-amendment checklist, open
questions to settle in that round's SPEC pass, seams, samples, acceptance. At
round entry, write the round's own detailed stage doc (`PLAN-TREE-alpha1.md`,
`CHART-alpha2.md`, … — the DRAWING-0.1x pattern): Stage 0 is always the SPEC
amendment, then bounded coding stages sized like PLAN-ALPHA's, each with a Log
line. A round is complete only when contract, code, samples, snapshots, visual
review, docs, and `ROADMAP.md`'s ladder row all agree.

---

## alpha.1 — tree, mindmap & natural routing

**Round entered 2026-07-11, shipped 2026-07-12 as `1.0.0-alpha.1`** — design
settled and staged in `PLAN-TREE-alpha1.md` (the decisions ledger supersedes the
open questions below; `radial` became `bilateral` in the design review; the
stage Logs there are the round's record).

**Scope** (ROADMAP 3.2, 3.3): `layout: tree` (row/column/bilateral), `|topic|`,
single-root rule, desugar-generated branch links resolving in the parent topic's
scope, `|mindmap|` preset with the first-level palette walk; `routing: natural`.

**SPEC amendment**: a new Part II section (tree — engine, roles, gap semantics,
errors) + SPEC 8 templates (`|topic|`, `|mindmap|`) + SPEC 9/11 (branch links,
wiring strategy row) + ROUTING.md (replace the `curved` row with `natural`'s
contract; refresh the implementation-shape map; tighten the "validated" claim).

**Open questions for the SPEC pass**: exact branch-link attachment geometry
(which sides at each direction); radial first-topic bearing and sector tie-break
wording; whether `|mindmap|`'s palette walk tints authored cross-links (lean no —
branch links only); topic wrap defaults (`max-width` preset on `|topic|`?).

**Seams** (AUDIT): D2 branch links in desugar; D3 the `LayoutEngine` trait lands
with this round; flex is a pure reusable positioner; new placement code =
parent-over-subtree centring + radial sectors; `natural` =
`routing/natural/{mod,corridor,curve}.rs` + `build_worlds` reuse + the widened
bundle filter (`request.rs:174`) + its own law checker; D4's exhaustive matches
flag every touch site.

**Samples** (cluster policy — extend before adding): one `tree.lini` holding an
org chart (column, orthogonal) and a row tree in one scene, plus one
README-worthy mindmap hero (rich topics, a cross-link, natural). **Prototype the
mindmap early** — natural-curve aesthetics are the round's real risk; iterate
the curve fit against the rendered PNG before polishing the engine.

**Acceptance**: tree samples deterministic under the laws oracle where routed;
mindmap reads well in light + dark (palette walk verified); `lini desugar` shows
branch links; `|topic|` outside tree and forest-roots error per SPEC.

---

## alpha.2 — charts: labels, per-datum paint, time, format

**Round entered 2026-07-18, combined with alpha.3 in `CHART-DRAW-alpha23.md`**
(one plan, two tags — its decisions ledger supersedes the open questions
below; `labels:` and tooltips had already landed in 0.21).

**Scope** (ROADMAP 3.4 + the `format:` machinery): per-datum `fill`/`stroke`/
`opacity` lists on `|bars|`/`|dots|` with `auto`; per-datum `labels:`;
`scale: time` axes (ISO-8601 quoted literals in `data:`/`range:`,
calendar-aware ticks); `format:` (all families) applied to axes + tooltips.

**SPEC amendment**: SPEC 14.2/14.3/14.4 + a new `format:` entry in the ledger
section; SPEC 20 error rows (list-count mismatch, list paint on line/area,
mixed date/numeric domain, invalid date).

**Open questions**: date literal edge cases (timezone offsets, time-only?);
`step:` for time axes (calendar intervals — `step: month`?) or auto-only in 1.0;
`fraction D` rendering style (unicode vs slash); whether `format:` rides the
ledger as an ordinary inherited property (lean yes).

**Seams** (AUDIT): repeated-mark series already lower one mark per datum;
`read_data` is the single data reader post-M1; `format:` machinery is
greenfield — put it beside the ledger so dims (alpha.3) consume the same engine.

**Samples** (cluster policy): extend `charts.lini` / `chart_advanced.lini` —
a time-series line, a paint-list-highlighted bar, `labels:` on the scatter.
No new files.

**Acceptance**: tick text pinned by snapshots across zoom-y domains (minutes →
years); paint lists verified against palette `auto` in light + dark; count
mismatches error with counts in the message.

---

## alpha.3 — drawing measurement

**Round entered 2026-07-18, combined with alpha.2 in `CHART-DRAW-alpha23.md`**
(one plan, two tags; the drawing half's SPEC pass is that plan's Stage 0b).

**Scope** (ROADMAP 3.5, first half): dimension `clearance` (cascade, replaces
dim `gap:`), painted-bounds row packing, linear-dimension inference +
`project:`, boxed datum letters + datum identities, crossing halos, internal
threads in sections, addressable pattern copies, fan leaders, `format:` on
dimensions. Plus one conformance bug from the old TODO: a bare leader's
authored label must **follow** the composed thread spec (`bar:m10 <- "LH"`
reads `M10×1.5 LH` — SPEC 15.6's one-ended follows rule; today a label
suppresses the auto-compose).

**SPEC amendment**: SPEC 15.6 rewrite (clearance, inference, `project:`,
aligned dims un-deferred), 15.7 (datum frames, fan leaders, halos), 15.4
(pattern copy ids), 15.3 (internal-thread sense), SPEC 20 rows, SPEC 23 prunes.

**Open questions**: aligned-dim side default wording (away from geometry
centre — define "centre" precisely); the concise left/right override relative
to endpoint order; halo margin width; whether `.2` copy segments read through
`break:` compression (they should — dims measure the unbroken model; leaders
land on the displayed copy).

**Seams** (AUDIT): `Rows` already carries `obstacles` + `blocked()` — replace
the fixed `DIM_OFFSET + k·DIM_PITCH` generator; D5 mask-based halos generalize
`label_mask`; numeric path segments need the lexer `.`+digit glue rule **and**
`parse_endpoint` (front-end M, the round's grammar change); datum letters =
a resolve-scene pass beside `id_seen`; fan leaders = allow `&` on one-ended
leader ops in resolve + one text/multi-tip in `leaders::callout`.

**Samples** (cluster policy): extend the drawing keepers — `drawing_screw` /
`drawing_sheet` / `drawing_annotations` exercise inference, aligned dims,
copies (`plate.bolt.2`), fan leaders, halos. No new files.

**Acceptance**: every drawing sample re-rendered and inspected at 1:1 and a
detail scale, light + dark, on-screen and at print size; the dim packer never
overlaps painted annotations (add an oracle-style check if cheap).

---

## alpha.4 — drafting symbols & annotation composition

**Scope** (ROADMAP 3.5, second half): the shared drafting-symbol path registry;
`|surface-finish|`; `|feature-control|` + `|control|` rows (full common
characteristic set, validated `datums:` against declared letters); `||`
generalized to annotation seating; annotation nodes inside drawing link `[ ]`.

**SPEC amendment**: SPEC 15.7/15.8 additions + a new 15.x for feature control;
the `||` semantics table (geometry vs annotation); SPEC 21 note (link `[ ]`
accepts nodes in drawing scope; core links stay text-only); SPEC 20 rows
(invalid frame combinations, point-target seating, unknown datum).

**Open questions**: the canonical current-standard characteristic names
(ISO 1101 vs ASME Y14.5 naming — settle the ident set, exclude obsolete
symbols); default seat anchors per annotation type; how a seated bundle
reports its extent to the dim packer.

**Seams** (AUDIT): icon `lookup`/`Role`/`emit_role_group` generalize (add a
natural-units sizing path — drafting glyphs don't fit-to-box); **the deep one**:
the link-label path is `TextNode`→`ResolvedText` end-to-end — widening it to
carry child nodes is the round's L item; annotation seating runs after mates,
outside the grounding graph.

**Samples** (cluster policy): one new `drawing_gdt.lini` — a genuinely new
cluster — holding the fully-toleranced part: frames, finish symbols (seated and
leader forms), datums.

**Acceptance**: frames render semantically valid or error — never plausible-
wrong; glyph line weights match dimension linework at every view scale;
annotation nodes register as packing obstacles (no overlaps).

---

## alpha.5 — images, title blocks & projection assistance

**Scope** (ROADMAP 3.5 end + 3.6): local image embedding (nested `<svg>` +
id-rewrite, raster data URIs, serve boundaries); title-block polish per the M3
renames; cross-view construction links (`|projection|` chrome; the legalized
cross-scope anchor form); view arrows via `of:` if cheap, else explicitly moved
to post-1.0 in ROADMAP 6.

**SPEC amendment**: SPEC 7 (`|image|` sources), 15.8 (projection links, view
arrows), 17 (embedded-asset output), 19 (serve boundary note); SPEC 20 rows
(missing asset, escape-root, cross-view misuse).

**Open questions**: the exact cross-view anchor spelling (a page-level link
naming `view.anchor` on each side — confirm it stays outside drawing scopes and
lowers to straight `|projection|` lines only); id-rewrite scheme for embedded
SVGs; a `--no-embed` escape hatch (the old TODO proposed one — keep?).

**Seams** (AUDIT): `emit_image` (`primitives.rs:465`) switches on the resolved
form; asset bytes read at resolve for determinism; `dir_mode::resolve_in_root`
generalized in M5 — file-mode needs its new boundary here; D6 id-rewrite.

**Samples**: a title block with an embedded logo; a two-view sheet with
projection lines (the DIN-912 screw gains them).

**Acceptance**: embedded output renders in resvg + a browser byte-identically
across runs; a traversal attempt errors; projection lines style/remove via the
cascade.

---

## beta — tooling, schema & docs (feature-complete)

**Scope** (ROADMAP 3.8): the generated machine-readable schema (from the
ledger); structured JSON diagnostics with stable codes (D9 — serde-free);
the compact generated reference; VS Code + Zed grammars (keyword lists
generated from the ledger); README/docs refresh; `lini fmt` final canon pass.

**Open questions**: schema format (lean JSON, one file, versioned); diagnostic
code taxonomy (prefix per family: `L` lex, `P` parse, `R` resolve, `V`
validate, `Y` layout, `T` route…); whether the reference ships in-repo or
generated at release.

**Acceptance**: schema + reference regenerate byte-identically from the ledger
in CI (drift = test failure); every diagnostic carries a code; grammars
highlight every sample correctly (spot-check in both editors).

**Carried over from PLAN-ALPHA** (the M7 retro): the render `{:?}` Debug
dedup keys → derived-`PartialEq` structural keys (needs `PartialEq` on
`ResolvedValue`, cascading through `Expr` — R1 follow-up, fits the D9
serde-free structuring work); deeper gate-driven validation reading the
ledger's `gate` column (R2/M2 follow-up — rides schema generation, which
walks the same rows).

---

## rc → 1.0.0

- Bug fixes only; anything feature-shaped goes to ROADMAP 6.
- The stability contract (ROADMAP section 2) lands **in SPEC** as a normative
  section.
- Full visual review: every sample, light + dark, screen + print scale.
- Cut `1.0.0` when an rc survives with zero code changes needed.

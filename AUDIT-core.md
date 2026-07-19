# AUDIT-core — cross-cutting audit (resolve / desugar / render / ledger / validate / syntax / serve / lib)

Read-only audit after five alpha rounds. Scope excludes `src/layout/drawing/**`+`src/glyph`
and `src/layout/chart/**`+format/date/assets (sibling auditors own those in depth). This
covers everything else and the seams between subsystems. Each finding is verified against
code (file:line quoted). Ranked by value.

House laws checked against: one mechanism per problem; no parallel implementations; ~500 LOC
ceiling; robust over patches; reused style rides a rule (`AGENTS.md`, restated in
`plans/DRAWING-0.16.md:14`).

---

## Finding 1 — Projection default paint is stated twice (parallel implementation)  · M · low risk

**Locations**
- `src/ledger/defaults.rs:342-346` — `template_bundle("projection")` = `[stroke: stroke-light, stroke-width: 1, fill: none]` (the tuning home).
- `src/render/stylesheet/families.rs:283-290` — `build_projection_rule` hand-restates the identical three props (reordered `fill/stroke/stroke-width`).

**Violated law:** *no parallel implementations* / *reused style rides a rule*. The comment at
`families.rs:266` openly admits "The props mirror `template_bundle("projection")`." Two copies
of one default: when the support-tone or width is retuned in the bundle, the render copy
silently rots — exactly the drift the law names. (`resolve/links.rs:500-505` already reads the
bundle for the *authored* path, so only the render default duplicates it.)

**Fix:** `build_projection_rule` should resolve `template_bundle("projection")` through
`paint_props` (as `build_template_rules` does at `families.rs:247-257`), not restate literals.
One line of props, one home.

---

## Finding 2 — The "emit a generated class default unless authored" pattern is hand-rolled per family with divergent guards  · M · low risk

**Locations** — three rounds each added one emit path, each with different guard mechanics:
- `src/render/stylesheet/families.rs:267-291` `build_projection_rule` — guards by scanning for an authored `lini-projection` class rule.
- `src/render/stylesheet/families.rs:444-455` `build_halo_rules` — *no* explicit guard; correctness relies on being emitted **before** `build_template_rules` so an authored `|halo|` rule wins by cascade order.
- `src/render/stylesheet/families.rs:411-437` + `cut_bg_rule` (429) — the shared white ground is de-duplicated by a `has_labels` boolean threaded between the label-cut path and the halo path.
- Same shape, hardcoded, in `build_shape_rules` (`families.rs:120-154`: `dim-line`/`ext-line`/`dim-text`) and `build_marker_rules` (`482-489`: `marker-dim`/`marker-datum`).

**Violated law:** *one mechanism per problem*. Six present-gated default emitters, three
distinct guard strategies (scan-for-authored vs rely-on-order vs dedup-flag). Emission order is
load-bearing and undocumented outside prose.

**Fix:** one helper `emit_generated_default(rules, class, present, props, authored)` that emits
only when present-and-not-authored, so ordering stops being the mechanism. At minimum unify the
guard (present ∧ ¬authored) across projection/halo. Pairs naturally with Finding 1 (props come
from the bundle).

---

## Finding 3 — Ledger `format` row's single `Inherit` cell can't express its dual cascade; two self-flagged dead/aspirational rows  · M · **beta-relevant**

**Locations**
- `src/ledger/properties.rs:540-553` — `format` owns `chart/pie/axis/series` (read engine-side, like `tooltip`) **and** `drawing/dimension` (rides the scope-link channel), but `inherit` is a single `ScopeLink` cell. `scope_link_props()` (`properties.rs:802`) therefore treats `format` as scope-link **globally**; only the drawing leg actually uses that channel (`resolve/program/link_scope.rs:156-165`).
- `src/validate.rs:544-548` — `node_accepts` returns `true` for *any* `inherit != No`, so `format:` validates as accepted on **every** node (inert, and the misuse message never fires, on a flow `|box|`).
- Same table, self-flagged: `text-shadow` "honoured but missing from SPEC 16" (`properties.rs:180`); `legend` "SPEC 16 marks honoured; no reader exists yet" (`properties.rs:466`) — an aspirational row with no behavior behind it.

**Violated law:** ledger-is-the-source-of-truth drift. The module header (`properties.rs:5-6`)
names **schema generation** as the next consumer; it will read `inherit=ScopeLink` for `format`
and mis-describe the chart behavior, and will publish a `legend` property that does nothing.

**Fix (before schema gen reads the table):** make `inherit` owner-scoped (or split `format`
into its two legs) so the generated schema is truthful; resolve `legend`/`text-shadow` against
SPEC 16 (add the reader or drop/annotate the row). This is the one seam beta will actually read.

---

## Finding 4 — Files past the ~500 LOC ceiling; `tests/rendering.rs` is the standout  · S · low risk

**Locations** (in-scope, over the ceiling):
`tests/rendering.rs` 1589 · `tests/routing.rs` 984 · `src/ledger/properties.rs` 987 ·
`src/lexer.rs` 865 · `src/desugar/tree.rs` 823 · `src/resolve/links.rs` 805 ·
`src/validate.rs` 747 · `src/desugar/mod.rs` 725 · `src/resolve/scene.rs` 723.

**Violated law:** *modular — split past ~500 LOC*.

**Named split seams:**
- `tests/rendering.rs` — 95 flat `#[test]`s, no `mod` structure (3× the ceiling). Split by theme: text/font, shape emission, paint/diff, links. Highest-value, lowest-risk split.
- `src/resolve/links.rs` — `try_projection` + `projection_attrs` (`links.rs:335-516`, ~180 LOC) is a cohesive cross-view slice that could move to `resolve/links/projection.rs`.
- `src/ledger/properties.rs` — mostly the data table (fine as data); its `#[cfg(test)]` block (`properties.rs:809-987`, ~180 LOC) could move to a sibling `properties/tests.rs`.

Low urgency; none is a correctness risk.

---

## Finding 5 — Duplicated id-path tree walk across the resolve/layout seam  · S · low risk

**Locations**
- `src/resolve/program/mod.rs:210-219` `inst_at_path` (takes `&[&str]`).
- `src/layout/mod.rs:262-271` `node_at` (takes a dot-path string, splits it).

Both are the identical "walk by `scene::find_in_scope` through anonymous containers, track
`found`, descend `children`" loop over `ResolvedInst` (both call `find_in_scope`, confirmed at
`program/mod.rs:214` and `layout/mod.rs:266`). *No parallel implementations.*

**Fix:** one shared helper on `find_in_scope` (segments in, `Option<&ResolvedInst>` out); the
two callers differ only in how they produce the segment list.

---

## Verified clean (non-findings — reassurance)

- **Link classification is centralized.** `resolve/links.rs::resolve_link` runs
  `try_projection` → operator `kind` match → `validate_statement` as sequential *stages* of one
  function (`links.rs:79-102`, `548`), not parallel copies. No second classifier.
- **Text paint has one chokepoint.** `render/mod.rs::text_paint_attr` (`mod.rs:366`) serves both
  node text leaves and link labels (`links.rs:521`); the comment records that link labels *used*
  to have a hand-rolled path that lost `text-shadow` — already unified. Good instance of the law
  applied.
- **`fmt_tick` is a thin documented delegation** to `ledger::format::auto`
  (`layout/chart/scale.rs:234-236`), not a parallel number formatter.
- **Knockout is one mechanism.** Label cuts and crossing halos both fold into the single
  `render/knockout.rs` luminance mask (`links.rs::label_mask` → `knockout::open/cut_rect/close`;
  `halo` cuts fold into the same mask). No second break mechanism.
- **`Options` surface is coherent** (`lib.rs:51-78`): `static_mode/embed_font/format/theme_css/
  base_dir/asset_root`, all documented; `base_dir` (image-src resolution) vs `asset_root` (serve
  traversal boundary) are distinct, well-named roles.
- **`resolve` (aka `resolve_with_theme`, `resolve/mod.rs:24`)** is a test-only convenience alias
  over `resolve_with_env`; not dead code, not a duplicate path.

---

## Verdict

**Beta-ready as-is, with one caveat.** The codebase is disciplined: the "one mechanism / no
parallel implementations" laws are visibly followed across resolve, render, and validate, and
the parallel-implementation instances that remain are few and small (Findings 1, 2, 5). The one
seam that is *not* yet beta-clean is the ledger itself — and the ledger is precisely what beta's
schema generation is documented to read next (`properties.rs:5`). Before schema gen turns the
ledger into a published contract, do these three:

1. **Finding 3** — reconcile the `format`/`legend`/`text-shadow` rows so the generated schema is
   truthful (the `format` dual-cascade `Inherit` cell, the reader-less `legend` row).
2. **Finding 1** — de-duplicate the projection default so the ledger/bundles stay the single
   tuning home (schema will derive defaults from bundles — a rotted render copy would leak).
3. **Finding 2** — unify the generated-default emit path so "which class defs exist and why" is
   one mechanism validation and schema can reason about, not three prose-coupled ones.

Findings 4–5 are healthy-hygiene cleanups that can ride any later pass.

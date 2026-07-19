# Audit — charts + format/date + assets

Read-only audit of `src/layout/chart/**`, `src/ledger/format.rs`,
`src/ledger/date.rs`, `src/resolve/assets.rs`, and the `format:` consumption
in `src/layout/drawing/compose.rs`, after the alpha.2/alpha.3 rounds
(`plans/CHART-DRAW-alpha23.md` Stages 0a–3, 8). Every finding is quoted against
the code. Ranked by value. Nothing here has been changed.

Overall: the round work is high quality — the `format:`/`date` engines are one
parse + one renderer shared by every consumer (`scale::label`, `format::render`,
`date::render`), draw-time paint routes through `Series::fill_at/outline_at/
opacity_at` with no duplication, and `palette::hue/deepen` genuinely live in one
place. The findings below are the residue: trivial-helper copies, two twin
attribute-reader pairs, one re-derived edge rule, one over-ceiling file, and a
handful of vestigial/patch-shaped bits.

---

## F1 — `live` / `muted` role-tint helpers copied across 5 files (High)

**Location.** `model/paint.rs:61-71` already defines and exports the pair
(`pub(crate) fn live`, `pub(super) fn muted`) — and `pie.rs:7` correctly imports
`live` from `model`. Yet four renderer files outside `model/` redefine
byte-identical private copies:
- `axis.rs:13-22` — both `live` and `muted`
- `radial.rs:15-20` — `live` (and inlines `live("muted")` at :61)
- `tooltip.rs:154-159` — `live`
- `bubble.rs:65-70` — `muted`

That is 4 `live` + 3 `muted` bodies for two three-line helpers.

**Violated law.** "No parallel implementations" / "Missing shared helpers"
(AGENTS.md). The root cause is reach: `muted` is `pub(super)` *inside* `model/`,
so the sibling renderers physically cannot call it and each grew its own.

**Fix.** Give the pair one chart-wide home reachable by both `model/` and the
renderers — either promote them to `chart/palette.rs` (already the "one place
for the colour walk") or a tiny `chart/tint.rs`, exported `pub(super)` to the
whole `chart` module. Delete the 5 copies; point `pie.rs` and the four
renderers at the one home.

**Effort.** S. **Risk.** Low — identical bodies, pure move.

---

## F2 — twin `range` / `ticks` attribute readers (High)

**Location.** `axis.rs`:
- `read_range` (`610-630`) vs `read_time_range` (`374-401`)
- `read_ticks` (`491-511`) vs `read_time_ticks` (`404-423`)

Each pair shares its whole envelope — the `range` pair both do
`Tuple` → `len != 2` guard → map two ends, with the **identical** error string
`"'range' takes two ends: 'a b', 'a auto', or 'auto b'"` typed twice; the
`ticks` pair both do the `List`-or-`from_ref(one)` unwrap → map → `collect`.
They differ only in the per-value reader: `read_end` (number/`auto`) vs the
inline date reader (`date_secs`/`auto`), and `as_number` vs `date_secs`.

**Violated law.** "No parallel implementations" — "two places do the same job …
call one shared function; never copy logic." A fix to the range envelope (say a
better arity message) must be made twice today and one copy will rot.

**Fix.** One generic each, taking the per-value reader as a closure:
`read_range(inst, |v| -> Result<End>)` and
`read_number_list(inst, "ticks"/"range", |v| -> Result<f64>)`. The numeric and
date call sites pass their end/value reader; the envelope and messages live once.

**Effort.** M. **Risk.** Low–Med (snapshot-verify the error messages).

---

## F3 — the edge rule re-derived per datum instead of shared (High)

**Location.** `model/paint.rs`. The single-value edge (`fill_outline`,
`40-50`) encodes: `stroke: none` → `None`, `stroke: auto`/unset → `deepen(fill)`,
explicit colour → `(colour, width)`. The per-datum path (`paint_lists`,
`181-204`) re-encodes the very same table item-by-item
(`none` → `None`, `auto` → `deepen(fill_at(i))`, other → `(other, width)`), and
the "unset still gains a deep edge" case is a separate block (`166-180`). The
`palette::deepen` doc-comment claims "the outlined look derives its edge in one
place" — `paint_lists` is a second place.

Compounding it, the unset-detector is a patch-shaped double-`matches!` over the
same key:
```rust
// paint.rs:162-165
let stroke_default = matches!(inst.attrs.get("stroke"), None | Some(ResolvedValue::Ident(_)))
    && !matches!(inst.attrs.get("stroke"), Some(ResolvedValue::Ident(s)) if s == "none");
```

**Violated law.** "No parallel implementations" + "One mechanism per problem."

**Fix.** Extract one `edge_from(stroke: Option<&ResolvedValue>, fill: &ResolvedValue,
width: f64) -> Option<(ResolvedValue, f64)>` that owns the none/auto/explicit/
unset table; `fill_outline` calls it with `attrs.get("stroke")`, `paint_lists`
calls it per item with the datum's fill. Replace `stroke_default` with a clean
3-arm `match` (`None => true; Some(Ident(s)) => s != "none"; _ => false`).

**Effort.** M. **Risk.** Med — touches paint resolution; render a bars/pie
sample to PNG and diff snapshots.

---

## F4 — `model/axes.rs` is 653 LOC, three concepts in one file (Med)

**Location.** `model/axes.rs` (653 LOC, well past the ~500 ceiling). It bundles
(a) axis binding/lookup (`bind_axis`, `lookup_axis`, `no_axis`), (b) scale
construction (`build_x_axis`, `build_value_axes`, `value_scale`,
`numeric_scale`, `time_scale`, `log_scale`), and (c) ~10 attribute readers
(`read_range`, `read_time_range`, `read_ticks`, `read_time_ticks`,
`read_cal_step`, `read_side`, `read_grid`, `read_unit`, `read_scale_kind`,
`numeric_fmt`).

**Violated law.** "Modular: one concept per file. Split a module past ~500 LOC."

**Fix.** Lift the attribute readers into `model/axes/read.rs` — which also
lands the F2 dedup naturally. Leaves `axes.rs` as binding + scale construction.

**Effort.** M. **Risk.** Low (mechanical move).

---

## F5 — the domain-from-`range` block triplicated (Med)

**Location.** `axis.rs`: `value_scale` (`262-280`), `numeric_scale` (`305-312`),
`time_scale` (`340-347`). The `Some((a,b))` arm —
`lo = end(a,dmin); hi = end(b,dmax); (lo.min(hi), lo.max(hi), lo > hi)` — is
byte-identical in all three, and each is preceded by the same
`data_min/data_max` fold with an empty-data fallback.

**Violated law.** "Missing shared helpers" / "No parallel implementations."

**Fix.** A `resolve_domain(xs: &[f64], range: Option<(End, End)>, empty: (f64,f64))
-> (f64, f64, bool)` returning `(min, max, rev)`. `value_scale` keeps only its
distinct bars-include-zero `None` branch; the other two collapse to the call.

**Effort.** M. **Risk.** Low.

---

## F6 — the "date preset reads a time axis" gate copied into 3 consumers (Med-low)

**Location.** The same literal message and inline gate appear at
`axis.rs:552-554` (`numeric_fmt`), `pie.rs:163-164`, and
`compose.rs:64-65` — each doing `read_or(...)` then `if matches!(f, Date(_)) { err
"a date preset reads a time axis" }`. Separately, the paint-list-shape message
`"a '|…|' is one shape with one paint — per-datum lists read on '|bars|' / '|dots|'"`
is typed verbatim in both `validate.rs:407-408` and `paint.rs:121-122`.

On the two-home question the prompt raises: the **validate vs paint** *split* is
principled and documented — `paint.rs:111-112` states the model is "the semantic
authority, so a library compile can't slip past" the linter. Likewise the
**chart's asymmetric gate** (`numeric_fmt` downgrades a *cascaded* date via
`format::numeric`, errors only on an *authored* one) vs the **dims' symmetric
gate** (`compose.rs` errors on any `Date`) is principled: a value axis genuinely
cascades `chart_fmt` (which may be a date preset for the time x), so it must
distinguish inherited from authored; a dimension defaults to `Auto` and never
inherits a format, so there is no cascaded-date case to downgrade. Neither is
drift. The smell is only the duplicated *strings*.

**Fix.** A `format::reject_date(f: Format, span: Span) -> Result<(), Error>`
(or a `pub const` message) shared by the three date gates; a shared `const` for
the paint-list message referenced from both validate and paint.

**Effort.** S. **Risk.** Low.

---

## F7 — `fmt_tick` is a vestigial one-caller wrapper (Low)

**Location.** `scale.rs:234-236` — `pub fn fmt_tick(n) { format::auto(n) }`, a
pre-`format:`-engine relic. Its only non-test caller is `pie.rs:103` (the percent
share); `pie.rs` already imports the engine and calls `format::render` on the
next line.

**Fix.** Call `format::auto` directly in `pie.rs`, delete `fmt_tick` and its test
(`scale.rs:419-424`).

**Effort.** S. **Risk.** Low.

---

## F8 — `auto`'s `-0` pin is inconsistent with the other families (Low)

**Location.** `format.rs`: every family routes through `no_neg_zero` (`218-224`)
so `-0.00` reads `0.00`, but `auto` (`132-143`) deliberately does **not**, and the
test at `:283` pins `render(-0.00001, Format::Auto) == "-0"` for byte-identity
with the historic tick formatter. In practice snapped ticks
(`ticks_by_step`/`nice_ticks` round to clean multiples) never emit a genuine
`-0`, so this is a latent wart, not a live bug — but it is one family behaving
unlike the rest.

**Fix (optional).** Route `auto` through `no_neg_zero` too — "one mechanism per
problem" — and refresh the pinned snapshot/test. Or, if the byte-pin is kept
deliberately, that is a defensible "trust the historic output" call; leave it.
Flagging the inconsistency, not asserting it must change.

**Effort.** S. **Risk.** Low (a snapshot/test churn).

---

## F9 — `marks::dot_title` re-does `scale::label`'s time dispatch (Low, note only)

**Location.** `marks.rs:223-227` hand-rolls a `(Scale::Time, Format)` match for
the hover x-value that mirrors `scale.rs:220-223`. The
`(Time, Date(p)) => date::render` arm is identical; the rest legitimately differs
(hover wants `date::render_full`, the full instant per SPEC 14.8, vs the tick's
granularity `render`). The genuinely-shared slice is a single arm, so this is a
weak dedup target — noting it only. If F2/F5 spawn a `chart` display helper, an
`x_display(scale, fmt, v, full: bool)` could absorb both.

---

## F10 — `resolve/assets.rs` at 618 LOC (borderline, note only)

`assets.rs` is 618 LOC but ~503 excluding tests (`503-618`), one coherent concept
(embed classification + the SPEC 17 id-rewrite). It is untouched by these rounds
(SPEC 7/17/19, not format/date) and reads clean. If it grows, the scan/rewrite
machinery (`152-465`) is a natural `assets/svg.rs`. No action now.

---

## Reorganization sketch (only where it genuinely helps)

A light touch, not a rewrite:

- **`chart/tint.rs`** (or fold into `palette.rs`): the one home for `live` /
  `muted` (F1). Removes 5 copies.
- **`model/axes/read.rs`**: the attribute readers moved out of `axes.rs` (F4),
  with the range/ticks readers collapsed to one generic each (F2) and a
  `resolve_domain` helper (F5). `axes.rs` drops back under the ceiling and holds
  only binding + scale construction.
- **`format.rs`**: add `reject_date` + share the paint-list message const (F6);
  optionally unify `auto` under `no_neg_zero` (F8).

Everything else (the `format:`/`date:` engines, `scale::label`, per-datum
draw-time accessors, `palette`, `assets`) is already single-home and should be
left alone.

---

### Top 5 (inline)

1. **F1** — `live`/`muted` copied in 5 files (`axis`/`radial`/`tooltip`/`bubble`
   vs `paint`'s exported pair); `muted` is `pub(super)` so renderers can't reach
   it → give the pair one chart-wide home. S / low.
2. **F2** — `read_range`/`read_time_range` and `read_ticks`/`read_time_ticks`
   (`axis.rs`) are twin readers, identical envelopes + messages → one generic each
   taking the value reader. M / low-med.
3. **F3** — the none/auto/deepen edge rule is re-derived per datum in
   `paint_lists:181-204` alongside `fill_outline:40-50` (+ a double-`matches!`
   `stroke_default`) → extract `edge_from`, clean the match. M / med.
4. **F5** — the `Some((a,b))` domain-from-range block is byte-identical in
   `value_scale`/`numeric_scale`/`time_scale` → `resolve_domain` helper. M / low.
5. **F4** — `model/axes.rs` is 653 LOC over three concepts → split the attribute
   readers into `axes/read.rs`. M / low.

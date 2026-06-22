# PLAN — Colour palette + gradients

Implements [SPEC §11.2 (palette)](SPEC.md) and [§11.3 (gradients)]. The SPEC is the
contract; this file is the build order. Pre-v1, so **no backward compatibility** —
refactor or rewrite freely, leave no patchwork. Modern Rust, no `unsafe`, one
concept per file, `insta` snapshots, one `samples/` file per feature, and **verify
colours by rendering to PNG with `resvg` and looking** — prettiness is the bar.

## The bet

The easy path is the pretty path: a curated 11-hue × 4-tier palette (OKLCH-derived,
`light-dark()`, themeable) plus angle-less gradients that can't look wrong. Taste
lives in the system.

## What already exists (lean on it)

- **No grammar change.** `gradient(--a, --b)`, `linear-gradient(135, --a, --b)`, and
  `--teal` already lex/parse: a gradient arrives as `ResolvedValue::Call { name, args }`
  and `--teal` as `ResolvedValue::LiveVar`. Confirmed against `src/syntax`, `src/lexer.rs`,
  `src/resolve/value.rs`. The whole feature is **render-layer + data**.
- **`src/render/filters.rs::FilterTable`** is the exact pattern for gradients:
  collect → dedup by formatted key → `emit_defs` into `<defs>` → reference by id.
  Mirror it.
- **`src/resolve/defaults.rs::built_in_defaults()`** builds the `--lini-*` `VarTable`
  (roles, each a `light-dark()` `Call`). The palette is added here.
- **`src/render/values.rs::format_value()`** turns a `ResolvedValue` into CSS:
  `LiveVar → var(--lini-name)` (or baked literal), `light-dark` baked to its light
  arm, `RawCss` passes through verbatim. The url-rewrite (Phase 2) exploits `RawCss`.
- **`src/render/style_block.rs::emit()`** emits the `@layer lini.defaults` block —
  **today it dumps every var**. Phase 1 makes it tree-shake.
- **`src/theme.rs`** renders palettes back to CSS for `lini theme` and composes
  built-ins; it starts from `built_in_defaults()`, so the palette flows through for
  free (audit its snapshots).
- Paint props that may carry a colour/gradient: `PAINT_PROPS` in `src/render/rules.rs`
  (`fill`, `stroke`, …). Paint reaches SVG two ways — the structural class rules
  (`rules.rs`) and the per-node inline diff (`render/mod.rs::node_style_attr`) — so
  any gradient rewrite and any var-usage scan **must cover both**.

---

## Phase 1 — Palette foundation + tree-shaking  ✅ DONE

The colour science, the prettiness tuning, and the tree-shake refactor. Everything
else builds on the vars this phase defines.

Shipped: `src/palette/{oklch.rs, mod.rs}` (OKLCH→sRGB + the seed/tier tables, tuned
and PNG-verified light **and** dark), appended in `src/resolve/defaults.rs`;
`src/render/used_vars.rs` tree-shakes the `@layer` block via `style_block.rs`;
`samples/palette.lini` shows all 11 × 4 + cards. `cargo fmt`/`clippy`/`test` clean.
Re-tune by editing `HUES` / `TIERS` in `src/palette/mod.rs`.

### 1a. OKLCH → sRGB  (`src/palette/oklch.rs`, new)
- Pure-`f64` OKLab→linear-sRGB→gamma→`#rrggbb`, with gamut clamping (reduce chroma
  toward grey until in-gamut, or clip — pick the one that stays prettiest).
- `pub fn oklch_to_hex(l: f64, c: f64, h_deg: f64) -> String`.
- Tests: a handful of known OKLCH↔sRGB reference triples within ±1 / 255.

### 1b. Seeds + ramp  (`src/palette/mod.rs`, `src/palette/seeds.rs`, new)
- `HUES`: the 11 seeds — `red rose orange amber lime green teal sky blue purple gray`
  — each a base hue angle (+ per-hue chroma scale; grey ≈ 0 chroma).
- Per-tier **L/C targets for each mode** (`wash soft base ink` × light/dark). Job-stable
  across the flip: in light mode L descends wash→ink; in dark mode the *ink* arm is
  light (high-contrast detail) and *wash* is a deep muted surface (SPEC §11.2 table).
- `pub fn palette_vars() -> Vec<(String, ResolvedValue)>` → for every hue: `--{hue}`,
  `--{hue}-wash`, `--{hue}-soft`, `--{hue}-ink`, each a `light-dark(#light, #dark)`
  `Call`. Keep targets in one tunable table — re-tuning the whole palette = editing it.
- **Aliases** as `LiveVar` pointers: `yellow→amber`, `pink→rose`, `indigo→purple`,
  `cyan→teal` (so `--lini-yellow: var(--lini-amber)`; tree-shake follows the pointer).

### 1c. Register  (`src/resolve/defaults.rs`)
- `built_in_defaults()` appends `palette::palette_vars()` after the roles. Roles stay
  independent in v1 (role→hue aliasing is Phase 3, decided after we eyeball the hues).

### 1d. Tree-shake the var block  (`src/render/used_vars.rs` new; `style_block.rs`)
- **Invariant:** emit a `--lini-*` var iff the document actually references it
  (directly or transitively); never drop a referenced var, never keep an unused one.
- Collect referenced names from everything that reaches output, computed *before*
  `style_block::emit`:
  - the built `RuleSet` prop strings (scan `var(--lini-NAME)`) — this pins the core
    roles the structural rules always state (`fill`, `stroke`, `bg`, `text-color`, …);
  - every `ResolvedValue` in node attrs, wire attrs, `SheetInputs.class_rules`,
    `wire_defaults`, `root_text`, the canvas fill, and (Phase 2) gradient stops —
    gather `LiveVar` names recursively;
  - then **transitive closure** over the `VarTable` (a kept var whose value references
    another, e.g. `text-color → fg`, or an alias → its target, pulls that in too).
- `style_block::emit` takes the set and emits only those (still sorted, still
  `color-scheme` when any kept colour is `light-dark`).
- Tests: `x |box|` keeps only core roles (no `teal`/`rose`); `x |box| { fill: --teal }`
  keeps `teal` (+ core), not `rose`; an alias use keeps its target; `--bake-vars`
  emits no `@layer` block at all (unchanged).

### 1e. Samples, snapshots, eyeball
- `samples/palette.lini`: a grid of the 11 hues × 4 tiers; a few “pretty card” nodes.
- `insta` snapshot of the emitted var block + a small compiled diagram.
- Render light **and** dark (`data-theme`) to PNG with `resvg`, look, and **tune the
  L/C targets until it's genuinely pretty** — this is the real acceptance test.
- Audit `src/theme.rs` snapshots: `lini theme default` now includes the palette.

**Phase 1 done when:** palette renders pretty in both modes (PNG-verified), the var
block is tree-shaken, `cargo test`/`clippy`/`fmt` clean.

---

## Phase 2 — Gradients  ✅ DONE

`gradient()` / `linear-gradient()` / `radial-gradient()` on `fill` and `stroke`.

Shipped: `src/render/gradients.rs` (parse + structural dedup + `lower` rewrites
use-sites to `url(#…)` + `emit_defs`); `GradientDef`/`GradientKind` data on `LaidOut`
(`src/layout/ir.rs`); `lower_gradients` runs post-layout from `lib.rs`; defs join the
`<defs>` beside the filters; stops ride `style="stop-color: var(…)"` live (browser
flip) and a `stop-color` literal when baked (resvg). Tree-shake walks `laid.gradients`
stops. `samples/gradient.lini` PNG-verified light **and** dark (2-stop, 3-stop, angle,
radial, on-stroke, oklch stops). All gates green.

**Also shipped (your ask): `oklch()` colour input** — SPEC §2, folded to a hex at
resolve time (`src/resolve/value.rs::resolve_oklch`), so it renders in every target
and works as a gradient stop. Errors cleanly on bad arity / out-of-range.

### 2a. Model + table  (`src/render/gradients.rs`, new — twin of `filters.rs`)
- Parse a gradient `Call` → `Gradient { kind: Linear { angle_deg } | Radial, stops: Vec<ResolvedValue> }`.
  Names: `gradient` (linear, 135°), `linear-gradient` (first arg = angle), `radial-gradient`.
  `gradient(a, b, c, …)` = N evenly-spaced stops. Require ≥2 stops.
- `GradientTable::collect(...)` over nodes + wires + `class_rules` + `wire_defaults`
  + canvas fill; dedup by a formatted key (kind + angle + stop list, honouring
  `--bake-vars` like `filters.rs::key`); assign `lini-gradient-{n}`.
- `emit_defs`: `<linearGradient gradientTransform="rotate({deg} .5 .5)">` (default
  `objectBoundingBox` units fit any shape) / `<radialGradient>`, with `<stop
  offset="i/(N-1)" stop-color="{format_value(stop)}"/>`. Stops stay vars (flip) or
  bake — same honesty as the shadow tint.

### 2b. Use-site rewrite  (after layout, before emit — `render/mod.rs`)
- One pass rewrites every gradient `Call` in every output-bound `AttrMap` (nodes,
  wires, `class_rules`, `wire_defaults`, canvas) to `ResolvedValue::RawCss("url(#lini-gradient-n)")`,
  building the table as it goes. `format_value` then passes `url(#…)` through
  unchanged everywhere — inline diff **and** class rules — with zero new branches.
- `render()` emits gradient defs into the existing `<defs>` alongside the filters
  (handle the “defs present” / empty branch together).

### 2c. Tree-shake interaction
- The Phase 1 collector must walk gradient **stops** so their palette vars survive.
  (Rewrite to `url()` happens after collection, or the collector reads the table’s
  stop lists — keep the ordering correct.)

### 2d. Samples, snapshots, eyeball
- `samples/gradient.lini`: two-stop, three-stop, explicit angle, radial; on fill and
  stroke. `insta` snapshot. PNG-verify angles, multi-stop, dark flip, and `--bake-vars`.

**Phase 2 done when:** gradients render (PNG-verified) on fill+stroke, dedup to shared
defs, flip and bake correctly, tests/clippy/fmt clean.

---

## Phase 3 — Polish & integration  (mostly DONE)

- ✅ **Diagnostics**: `<2` stops and a missing `linear-gradient` angle now error with a
  span at resolve (`resolve_gradient`/`validate_gradient` in `src/resolve/value.rs`),
  added to SPEC §15. (oklch errors landed with the oklch work.)
- ✅ **README**: a “Colour” section leads with the palette + gradients; test count
  refreshed to 407.
- ✅ **`fmt`** round-trips: `gradient.lini` passes the idempotence + semantic suites.
- ✅ **Conformance**: `palette.lini` + `gradient.lini` are snapshot-covered.
- ⏸ **Role→hue aliasing** — **needs a call, not done.** The roles are deliberately
  punchier than the pastel palette (accent = vivid blue, danger = crimson, warn =
  bright orange); the base tier is softer and the ink tier darker, so a clean 1:1
  alias would *regress* them, and it changes existing output. Recommend keeping roles
  independent (the gray ramp already gives neutral resolution). Revisit only if you
  want the consistency more than the punch.
- ⏸ **`lini theme` grouping** (optional polish): the palette prints alphabetically
  today — readable, but section comments (roles / hues) would read nicer. Low value.

---

## Decisions (locked)

| Decision | Choice |
|---|---|
| Palette model | combo — semantic roles **+** 11 named hues |
| Hues | red rose orange amber lime green teal sky blue purple gray |
| Tiers | `wash` / `soft` / `(bare)` / `ink` — job-named, flip-stable, 4 per hue |
| Derivation | OKLCH seeds → baked `light-dark()` literals, one tunable target table |
| Aliases | yellow→amber, pink→rose, indigo→purple, cyan→teal |
| Gradient stops | explicit `--name`s (themeable) or raw colours; **no** bare-hue magic |
| Gradient angle | auto-135° default; `linear-gradient(deg, …)` for control |
| Presets (`gradient(sunset)`) | **dropped** — invented names are the memory tax |
| Bare `gradient` keyword | **dropped** for v1 — gradients take explicit colours |
| Roles → hues | deferred to Phase 3 (avoid regressing the tuned look) |
| Output size | tree-shake the var block — non-negotiable given palette size |

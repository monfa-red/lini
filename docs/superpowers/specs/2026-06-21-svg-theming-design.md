# SVG Theming — Dark/Light Adaptivity & Built-in Themes

**Status:** design approved (brainstorm), pending spec review
**Date:** 2026-06-21

## 1. Goal

Two related capabilities, built on one foundation:

1. **Live, self-adapting SVG.** A single exported SVG carries both a light and a
   dark palette. It switches **automatically** with the OS (`prefers-color-scheme`)
   and can be **forced** by a `data-theme="dark"` / `"light"` attribute on the SVG
   or any ancestor — so once embedded in a host web app it adjusts on its own.
2. **Theme selection for export.** A `--theme` flag picks a palette: a built-in
   name, a custom CSS file, or an adaptive pair. Ships built-in themes (light,
   dark, high-contrast, blueprint, terminal, pastel) and bakes them to literals
   for non-web renderers with `--bake-vars`.

## 2. The single lever (why this is small)

Every default colour in lini already resolves to a `var(--lini-*)` reference, and
the literal values appear in exactly **one** place in the output — the
`@layer lini.defaults { :root, .lini { … } }` block emitted by
`render/style_block.rs::emit`. `resolve/defaults.rs::built_in_defaults()` is the
source of those values, i.e. **the current light theme is just that implicit
palette.** Because lini separates *visual* (live `var()`) from *layout* (baked),
switching colours never triggers re-layout.

**Therefore: a theme is a named set of `--lini-*` values.** Both features fall out
of that one concept. Feature 2 chooses which palette fills the defaults block;
feature 1 emits two palettes with CSS-native switching between them.

## 3. Core mechanism — `light-dark()` + `color-scheme`

Each themeable colour is written **once** as `light-dark(LIGHT, DARK)`. The value
the UA picks is driven by the element's `color-scheme`, which follows
`prefers-color-scheme` when set to `light dark`. No `@media` block, no JS, no
duplicated values.

### 3.1 Emitted `<style>` — live (adaptive) mode

```css
<style>
  @layer lini.defaults {
    :root, .lini {
      color-scheme: light dark;                 /* opt into both; drives light-dark() */
      --lini-fg:     light-dark(black, #e8e8ea);
      --lini-fill:   light-dark(white, #26262b);
      --lini-stroke: light-dark(#444,  #9aa0a6);
      /* …every colour var, exactly once… */
      --lini-canvas: transparent;               /* single value = same in both modes */
      --lini-font-family: ui-monospace, …;      /* non-colour vars: single value */
    }
    .lini[data-theme="dark"],  [data-theme="dark"]  .lini { color-scheme: dark; }
    .lini[data-theme="light"], [data-theme="light"] .lini { color-scheme: light; }
  }
  /* structural rules (unchanged) … */
</style>
```

- **Auto:** `color-scheme: light dark` makes `light-dark()` follow the OS.
- **Toggle:** the two attribute rules just flip `color-scheme`; the attribute
  selector's higher specificity beats the auto state, so an explicit
  `data-theme` overrides the OS. Both rules are one line — **no colour value is
  repeated.**
- **Host wins:** everything stays inside `@layer lini.defaults`, so unlayered
  host CSS still overrides with no `!important`.

### 3.2 Per-var dedup rule

A var is emitted as `light-dark(base, dark)` **only when its dark override differs
from its base**; otherwise it emits as a single value. So fonts, and any colour a
theme leaves unchanged in dark, stay single. `--lini-text-color` stays
`var(--lini-fg)` (a single reference; `fg` itself is `light-dark(...)`).

### 3.3 Browser floor (decided: no fallback)

`light-dark()` requires browsers from ~2024+. On older browsers the declaration
is dropped and default colours fall back to SVG initials (geometry intact,
colours wrong). We accept this for the cleanest, repetition-free output; any
non-modern or non-web target uses `--bake-vars`, whose output contains no
`light-dark()` at all (§6). The ~2024 floor is documented.

### 3.4 Baked mode (`--bake-vars`)

No `var()`, no `light-dark()`, no `@layer`, no `color-scheme`, no toggle rules —
a single frozen palette. Every `light-dark(L, D)` resolves to the **L or D arm**
chosen by mode (default light; `--theme dark` bakes dark). Renderer-safe for
resvg / librsvg / raster / email, exactly as today.

## 4. Theme file format (= the user's boilerplate)

A theme is plain CSS with `--lini-*` declarations; each colour is
`light-dark(LIGHT, DARK)` (a single value means identical in both modes). This is
the same shape the engine emits, so it doubles as copy-paste boilerplate.

```css
/* lini theme — copy & edit. Colours/fonts only; sizes are baked. */
:root, .lini {
  color-scheme: light dark;
  --lini-fg:     light-dark(black,   #e8e8ea);
  --lini-fill:   light-dark(white,   #26262b);
  --lini-stroke: light-dark(#444,    #9aa0a6);
  --lini-accent: light-dark(#0a84ff, #4aa3ff);
  --lini-canvas: light-dark(#fbfbfd, #1b1b1f);   /* opaque — a self-contained theme;
                                                    the default leaves this transparent (§7) */
  --lini-font-family: ui-monospace, "SF Mono", …, monospace;
}
```

`lini theme <name>` prints a built-in's CSS to stdout (real, in-sync boilerplate);
`lini theme` lists the built-ins.

## 5. CLI surface (`--theme` polymorphic)

| Invocation | Output |
|---|---|
| *(none)* | the default theme: light base + dark → **adaptive** |
| `--theme dark` | force the `dark` built-in as a single palette |
| `--theme blueprint` | a built-in single palette |
| `--theme ./brand.css` | a user file — adaptive if it has dark values, else single |
| `--theme light/dark` | compose two built-ins into an adaptive pair |
| `--theme auto` | alias for the default `light/dark` pair |
| `… --bake-vars` | freeze one palette to literals (the non-web export case) |
| `lini theme [name]` | list built-ins / print one as CSS boilerplate |

**Resolution order** for a `--theme VALUE`:
1. contains `/` → split into a `base/dark` **pair** (each side a built-in name or
   a file); compose into `light-dark()` per var.
2. matches a built-in name → embedded theme.
3. otherwise → a **file path** (today's behaviour). Unreadable & not a name → error.

**Baking a pair/adaptive theme** freezes the light arm by default; to bake dark,
use the single `--theme dark`.

## 6. Built-in themes

Authored as embedded theme files (`themes/*.css`, compiled in via `include_str!`)
in the §4 format, parsed by the same upgraded parser as user `--theme` files —
one mechanism. Starting palettes (to refine by visual rendering):

| Theme | Character | Canvas | Notes |
|---|---|---|---|
| `default` | the light+dark adaptive pair (no `--theme`) | transparent both | blends into host |
| `light` | the base alone | transparent | forces light, single |
| `dark` | dark alone | `#1b1b1f` | forces dark, opaque, bake-friendly |
| `high-contrast` | max colour contrast, light+dark | white / black | contrast-only (see limits) |
| `blueprint` | cyan/white ink on deep blue | `#0d2b57` | single aesthetic |
| `terminal` | phosphor green on near-black | `#0a0e0a` | single aesthetic |
| `pastel` | tasteful soft pinks/purples on warm white | `#fdf7fb` | single aesthetic |

Dark override starting values for the `default`/`dark` palette:
`fg #e8e8ea`, `fill #26262b`, `stroke #9aa0a6`, `accent #4aa3ff`, `on-accent white`,
`muted #9aa0a6`, `danger #ff6b6b`, `warn #ffb454`, `airwire #ff6b6b`,
`note-bg #4a4733`, `group-stroke rgba(255,255,255,.4)`,
`group-fill rgba(255,255,255,.05)`, `caption-color rgba(255,255,255,.55)`,
`footer-color rgba(255,255,255,.55)`, `shadow-color rgba(0,0,0,.5)`,
`bg #1b1b1f` (the glyph knock-out colour).

## 7. Scene background — `--lini-canvas`

New themeable var for the scene's backing plate, distinct from `--lini-bg` (which
stays the airwire-glyph knock-out colour, untouched).

- Default `transparent` (both modes) — preserves today's embeddable, no-plate scene.
- The backing `<rect class="lini-canvas">` is **always emitted in live mode**,
  filled with the root's explicit `fill:` if set, else `var(--lini-canvas)`. So a
  theme can paint a scene that wasn't authored with a `fill:`.
- In **baked** mode the rect is emitted only when the resolved colour is not
  `none`/transparent (keeps minimal static exports clean).
- SPEC §10: the root `fill` default becomes `--canvas` (mirrors how a box's
  `fill` defaults to `--fill`); `fill: none` still forces transparent.

## 8. Internals — components & data flow

### 8.1 Value representation
A themeable colour default becomes a `ResolvedValue::Call { name: "light-dark",
args: [light, dark] }`. `render/values.rs::format_value` already emits Calls
generically → `light-dark(…, …)` live. Add **one** bake case: when
`opts.bake_vars`, a `light-dark` Call resolves to `args[mode]` (the chosen arm).
The bake-time var follower (`format_value`'s `LiveVar` branch) already chases
`--name` → value; it now lands on a `light-dark` Call and picks the arm.

### 8.2 VarTable / defaults
**One mechanism:** the default palette lives in `themes/default.css` (the §4
format, light+dark via `light-dark()`), embedded with `include_str!` and parsed by
§8.3 — so `built_in_defaults()` becomes "parse the default theme," and our own
default is literally the CSS a user could feed (§1 goal). Parsing yields, per var,
a `light-dark(base, dark)` Call (when a dark override is present) or a single
value; `--lini-canvas` is added (default `transparent`). **Fidelity risk:** the
parser must reproduce today's literal forms closely enough that baked light output
stays sensible (`rgba(…)` spacing, the font stack, the `text-color → var(--lini-fg)`
self-reference). If that proves fragile in implementation, the light base may stay
typed in `built_in_defaults()` with only the dark overrides parsed — same observable
output, noted here so the plan can fall back without a redesign.

### 8.3 Theme parsing (`theme.rs`)
`extract_lini_vars` grows a small **value grammar** so a theme value can be
`light-dark(…)`, `var(--lini-*)`, `rgb()/rgba()/hsl()/hsla()`, hex, ident,
number, or a font stack — reusing lini's existing value lexer/parser where
possible. `var(--lini-X)` maps to `LiveVar { name: X }`. It remains a flat
declaration scanner (not a full CSS parser); declarations outside `--lini-*` are
ignored as today.

### 8.4 Theme resolution (`main.rs` / `lib.rs`)
`--theme` resolves (per §5) to a `Theme { base, dark }` (two `(name, raw_value)`
lists). `Options.theme_css: Option<String>` is replaced by
`Options.theme: Option<Theme>`; `lib.rs::resolve_pipeline` passes it to resolve in
place of today's `extract_lini_vars(css)` call. `apply_theme` in
`resolve/program.rs` sets each var to `light-dark(base, dark)` when the two
differ, else the single value. A built-in name loads the embedded string; a pair
composes two; `auto` = the default pair. Single-palette (`--theme dark`, or any
theme with no dark values) sets single values and emits no toggle rules.

### 8.5 Emission (`render/style_block.rs`)
`emit` gains the adaptive structure of §3.1: `color-scheme: light dark` on the
base rule, the two `data-theme` toggle rules, all inside `@layer lini.defaults`.
Toggle rules are emitted only when at least one var is `light-dark` (i.e. the
theme actually has a dark variant). Baked mode emits none of this (§3.4).

### 8.6 Canvas rect (`render/mod.rs`, `layout/mod.rs`)
`canvas_fill` becomes "root `fill:` if set, else `var(--lini-canvas)`". The rect
emits per §7.

## 9. Honest limits (document in SPEC §11)

- **Only var-backed colours theme.** A literal `fill: #eef` on a node bakes and
  won't switch — theming affects defaults and `--name` vars, not hard-coded
  colours.
- **Themes are colour-only by construction.** `stroke-width`, `radius`, `padding`
  are baked layout, so even high-contrast changes only colour contrast, never
  line weight.
- **`<img>`-embedded SVG follows the OS only.** Auto (`prefers-color-scheme`)
  works in `<img>`/background embeds; the `data-theme` toggle needs host-DOM
  access, i.e. inline SVG.
- **`--bake-vars` can't be adaptive.** It freezes one palette (target renderers
  support neither `var()` nor `light-dark()`).

## 10. Default-output change (decided: adaptive by default)

`lini in.lini` with no flags now emits light+dark. Consequences, accepted:
- Every diagram's `<style>` gains the `light-dark()` base + toggle rules.
- All `tests/snapshots/conformance__*` regenerate (output stays deterministic).
- SPEC §11/§13 and the README "Theming" section are rewritten.
- `--theme light` reproduces a single light palette for anyone wanting the old
  minimal output.

## 11. Testing

- **insta snapshots** of the emitted `<style>` for: default adaptive, `--theme
  light` (single), `--theme dark` (single), `--bake-vars` (light arm + dark arm),
  and one of each built-in.
- **Unit**: per-var dedup (single value when no dark override; `light-dark` when
  differs); `light-dark` arm-pick under `bake_vars`; theme-value parsing of
  `light-dark`/`var`/`rgba`/font-stack; `--theme` resolution order (pair/name/file).
- **Conformance**: regenerate `tests/snapshots`, review the diff is purely the new
  `<style>` structure.
- **Sample**: `samples/themes.lini` exercising a `--name` colour + canvas, plus a
  scene that renders meaningfully in both modes.
- **Visual (resvg → PNG)**: render `--theme dark --bake-vars` and a baked
  `pastel`/`blueprint`, read the PNGs to confirm legible contrast; confirm a
  default adaptive SVG renders light in a browser and flips with `data-theme`.

## 12. Out of scope / deferred

- Per-element theme overrides beyond the var palette.
- `@media (prefers-contrast)` auto high-contrast (built-in theme is opt-in only).
- Animated theme transitions.
- Migrating layout constants into themes (they bake; non-goal by §9).
```


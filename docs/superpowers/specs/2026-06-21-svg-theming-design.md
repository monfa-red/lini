# SVG Theming — Dark/Light Adaptivity & Built-in Themes

**Status:** design approved (brainstorm), implementation in progress
**Date:** 2026-06-21

## 1. Goal

Two capabilities on one foundation:

1. **Live, self-adapting SVG.** One exported SVG carries both a light and a dark
   palette and switches **automatically** with the OS and on a `data-theme`
   override — so once embedded it adjusts itself.
2. **Theme selection for export.** `--theme` picks a palette (built-in name,
   custom CSS file, or an adaptive pair); ships built-in themes; bakes to literals
   for non-web renderers with `--bake-vars`.

## 2. The single lever

Every default colour already resolves to a `var(--lini-*)` reference, and the
literal values appear in exactly one place — the `@layer lini.defaults` block in
`render/style_block.rs::emit`. `resolve/defaults.rs::built_in_defaults()` is their
source; the current light theme is just that implicit palette. Visual (live
`var()`) is already split from layout (baked), so switching colours never
re-layouts. **A theme is a named set of `--lini-*` values.** Both features fall out
of that.

## 3. Core mechanism — `light-dark()` + `color-scheme`

Each themeable colour is written **once** as `light-dark(LIGHT, DARK)`. The arm the
UA paints is driven by the element's `color-scheme`; `color-scheme: light dark`
makes it follow `prefers-color-scheme` automatically — **no `@media`, no JS, no
duplicated values.** A `data-theme` attribute overrides by flipping `color-scheme`.

### 3.1 Emitted `<style>` — live (adaptive) mode

```css
<style>
  @layer lini.defaults {
    :root, .lini {
      color-scheme: light dark;                  /* drives light-dark(); follows the OS */
      --lini-bg:     light-dark(white, #1b1b1f);
      --lini-fg:     light-dark(black, #e8e8ea);
      --lini-fill:   light-dark(white, #26262b);
      --lini-stroke: light-dark(#444,  #9aa0a6);
      /* …every colour var, exactly once… */
      --lini-font-family: ui-monospace, …;       /* non-colour vars: single value */
    }
    .lini[data-theme="dark"],  [data-theme="dark"]  .lini { color-scheme: dark; }
    .lini[data-theme="light"], [data-theme="light"] .lini { color-scheme: light; }
  }
  .lini .lini-canvas { fill: var(--lini-bg); }   /* the scene background (§7) */
  /* …structural rules… */
</style>
```

- **Auto:** `color-scheme: light dark` → `light-dark()` follows the OS.
- **Toggle:** the two attribute rules just flip `color-scheme`; the attribute
  selector's higher specificity beats the auto state, so `data-theme` wins over the
  OS. One line each — **no colour value repeated.**
- **Host wins:** all inside `@layer lini.defaults`, so unlayered host CSS overrides
  with no `!important`.

### 3.2 Per-var dedup

A var emits as `light-dark(base, dark)` only when its dark value differs from base;
otherwise a single value. Fonts and any colour unchanged in dark stay single.
`--lini-text-color` stays `var(--lini-fg)` (a single reference; `fg` is the pair).

### 3.3 Browser floor (decided: no fallback)

`light-dark()` needs browsers ~2024+. On older browsers the declaration is dropped
and default colours fall back to SVG initials (geometry intact, colours wrong). We
accept this for the cleanest repetition-free output; non-modern / non-web targets
use `--bake-vars`, whose output has no `light-dark()` at all.

### 3.4 Baked mode (`--bake-vars`)

No `var()`, `light-dark()`, `@layer`, `color-scheme`, or toggle rules — one frozen
palette. Each `light-dark(L, D)` resolves to the **light arm (`L`)**; to bake a dark
file use the single-palette `--theme dark` (whose values aren't `light-dark`).
Renderer-safe for resvg / librsvg / raster / email.

## 4. Theme file format (= the user's boilerplate)

Plain CSS with `--lini-*` declarations; each colour is `light-dark(LIGHT, DARK)` (a
single value = identical in both modes). Same shape the engine emits, so it doubles
as copy-paste boilerplate. `lini theme default` prints this; `lini theme` lists the
built-ins.

```css
/* lini theme — copy & edit. Colours; font optional. Sizes are baked, not themeable. */
:root, .lini {
  --lini-bg:     light-dark(white,   #1b1b1f);   /* scene background */
  --lini-fg:     light-dark(black,   #e8e8ea);
  --lini-fill:   light-dark(white,   #26262b);
  --lini-stroke: light-dark(#444,    #9aa0a6);
  --lini-accent: light-dark(#0a84ff, #4aa3ff);
  /* --lini-font-family: "Inter", system-ui, sans-serif;  // optional; monospace keeps text sizing exact */
}
```

## 5. CLI surface (`--theme` polymorphic)

| Invocation | Output |
|---|---|
| *(none)* | default theme: light base + dark → **adaptive** |
| `--theme dark` | force the `dark` built-in as a single palette |
| `--theme blueprint` | a built-in single palette |
| `--theme ./brand.css` | a user file — adaptive if it has dark values, else single |
| `--theme light/dark` | compose two built-ins into an adaptive pair (`auto` = this) |
| `… --bake-vars` | freeze one palette to literals (the non-web export case) |
| `lini theme [name]` | list built-ins / print one as CSS boilerplate |

**Resolution order** of `--theme VALUE`: (1) contains `/` → a `base/dark` **pair**
(each side a name or file) composed into `light-dark()` per var; (2) a built-in
**name**; (3) a **file path** (today's behaviour). Not a name and unreadable → error.

## 6. Built-in themes

Starting palettes (refined by visual rendering). The `default`/`light`/`dark` share
one adaptive palette; the rest are single aesthetics.

| Theme | Character | `--lini-bg` | Notes |
|---|---|---|---|
| `default` | the light+dark adaptive pair (no `--theme`) | `light-dark(white,#1b1b1f)` | the boilerplate template |
| `light` | the base arm alone | white | forces light, single |
| `dark` | the dark arm alone | `#1b1b1f` | forces dark, single, bake-friendly |
| `high-contrast` | max colour contrast, light+dark | white / black | contrast-only (see limits) |
| `blueprint` | cyan/white ink on deep blue | `#0d2b57` | single aesthetic |
| `terminal` | phosphor green on near-black | `#0a0e0a` | single aesthetic |
| `pastel` | tasteful soft pinks/purples on warm white | `#fdf7fb` | single aesthetic |

Dark-arm starting values for the `default` palette: `bg #1b1b1f`, `fg #e8e8ea`,
`fill #26262b`, `stroke #9aa0a6`, `accent #4aa3ff`, `accent-text white`,
`muted #9aa0a6`, `danger #ff6b6b`, `warn #ffb454`, `airwire #ff6b6b`,
`note-bg #4a4733`, `group-stroke rgba(255,255,255,.4)`,
`group-fill rgba(255,255,255,.05)`, `caption-color rgba(255,255,255,.55)`,
`footer-color rgba(255,255,255,.55)`, `shadow-color rgba(0,0,0,.5)`.

## 7. Scene background — repurpose `--lini-bg`

`--lini-bg` becomes the scene's background colour. (It previously only knocked out
the airwire warning glyph — an unimportant role; that glyph now fills with
`var(--lini-fill)`, which reads against any background.)

- The backing `<rect class="lini-canvas">` is **always emitted** and painted by a
  CSS class rule `.lini .lini-canvas { fill: var(--lini-bg) }` — a CSS property, so
  `var()` / `light-dark()` resolve live and bake to a literal (resvg/email-safe).
  Not `background-color` on the svg root: resvg ignores that, dropping the canvas in
  png/email. A root `fill:` (author override) emits inline on the rect, beating the
  rule.
- Default `--lini-bg: light-dark(white, #1b1b1f)` → light/dark canvas, self-contained
  and legible regardless of host. A host re-themes by overriding `--lini-bg` (it
  lives in `@layer`, so host CSS wins) or `.lini-canvas`.
- `--bake-vars`: the rect bakes the chosen arm; skipped only if the resolved colour
  is `none`. `fill: none` still forces transparent.
- The playground drops its own `background: var(--paper)` on the render pane so the
  SVG's own `--lini-bg` shows.

## 8. Internals — components & data flow

### 8.1 Value representation
A themeable colour default is `ResolvedValue::Call { name: "light-dark", args: [L,
D] }`. `render/values.rs::format_value` already emits Calls → `light-dark(…, …)`
live. One added bake case: under `opts.bake_vars`, a `light-dark` Call formats
`args[0]` (the light arm). The bake-time `LiveVar` follower already chases `--name`
→ value and now lands on the Call; it likewise takes the light arm. No mode field.

### 8.2 VarTable / defaults (typed, generated boilerplate)
`built_in_defaults()` stays the **typed** source of truth (bake-safe, no startup
parse, font default intact): each themeable colour becomes a `light-dark(L, D)`
Call; `bg` becomes the canvas colour; `on-accent` is renamed `accent-text`. The
other built-in themes are typed override maps (name → `Vec<(var, ResolvedValue)>`).
`lini theme <name>` *generates* the canonical CSS (§4) from this data — light-dark()
pairs, with `font-family` emitted as a **commented** hint for `default` — so the
boilerplate is real and always in sync (one format, generated not hand-kept).

### 8.3 Theme value parsing (`theme.rs`, for user files)
A user `--theme FILE` value can be `light-dark(a, b)`, `var(--lini-X)`,
`rgb()/rgba()/hsl()/hsla()`, hex, ident, number, or a font stack. The value parser
gains these forms: `light-dark(a,b)` → the Call; `var(--lini-X)` → `LiveVar{X}`;
function calls → `Call`. It stays a flat `--lini-*` declaration scanner, not a full
CSS parser.

### 8.4 Theme resolution (`theme.rs` / `lib.rs` / `main.rs`)
`--theme` resolves (per §5) to a `Vec<(String, ResolvedValue)>` override list:
a built-in name → its typed map; a file → parsed (§8.3); a pair `a/b` → for each var
present in either, `light-dark(a_val, b_val)`. `Options.theme_css: Option<String>`
is replaced by `Options.theme: Vec<(String, ResolvedValue)>`. `resolve::apply_theme`
applies the list over `built_in_defaults()` (overlay; a missing var keeps the base).

### 8.5 Emission (`render/style_block.rs`)
`emit` gains §3.1: `color-scheme: light dark` on the base rule and the two
`data-theme` toggle rules, all inside `@layer lini.defaults` — emitted only when the
palette is **adaptive** (some var is a `light-dark` Call). Baked mode emits none of
this (§3.4).

### 8.6 Canvas rect (`render/rules.rs`, `render/mod.rs`, `layout/mod.rs`)
Add a `.lini .lini-canvas { fill: var(--lini-bg) }` structural rule. The rect is
always emitted in live mode (no `fill` attribute; the rule paints it); a root
`fill:` emits inline on the rect. In baked mode the rect carries the literal and is
skipped only when the colour resolves to `none`.

### 8.7 Airwire glyph (`render/wires.rs`)
`render_airwire`'s knock-out fill changes from `--lini-bg` to `--lini-fill`.

## 9. Limits (document in SPEC §11)

- **Colours and `font-family` theme; layout never does.** Visual vars are live;
  sizes/weights of layout (`stroke-width`, `radius`, `padding`, `gap`, `font-size`)
  bake. So even high-contrast changes only colour, not line weight. A themed
  proportional `font-family` makes text-width estimates approximate (monospace keeps
  them exact) — hence it's an opt-in, commented in the default boilerplate.
- **Only var-backed colours theme.** A literal `fill: #eef` on a node bakes and
  won't switch; theming affects defaults and `--name` vars.
- **`<img>`-embedded SVG follows the OS only.** Auto (`color-scheme`) works in
  `<img>`/background embeds; the `data-theme` toggle needs host-DOM access (inline SVG).
- **`--bake-vars` can't be adaptive.** It freezes one palette (target renderers
  support neither `var()` nor `light-dark()`).

## 10. Default-output change (decided: adaptive by default)

`lini in.lini` with no flags emits light+dark. Accepted consequences: every
diagram's `<style>` gains the `light-dark()` base + toggle rules and a canvas rect;
all `tests/snapshots/conformance__*` regenerate (output stays deterministic); SPEC
§10/§11/§13/§14 and the README "Theming" section are rewritten; `--theme light`
reproduces a single light palette.

## 11. Testing

- **insta snapshots** of the emitted `<style>` for: default adaptive, `--theme
  light` (single), `--theme dark` (single), `--bake-vars` (light arm), and one of
  each built-in.
- **Unit:** per-var dedup; `light-dark` light-arm under `bake_vars`; theme-value
  parsing of `light-dark`/`var`/`rgba`; `--theme` resolution order (pair/name/file).
- **Conformance:** regenerate `tests/snapshots`; review the diff is purely the new
  `<style>` + canvas rect.
- **Sample:** `samples/themes.lini` — a `--name` colour, a node that reads in both
  modes.
- **Visual (resvg → PNG):** render `--theme dark --bake-vars` and a baked
  `pastel`/`blueprint`; confirm legible contrast; confirm a default adaptive SVG
  renders light in a browser and flips with `data-theme`.

## 12. Out of scope

Per-element theme overrides beyond the palette; `@media (prefers-contrast)`;
animated transitions; theming layout constants (they bake — §9).
```


# TODO

## Diagnostics / error reporting

Many misuses are **silently ignored** instead of reported — the value just does
nothing. Build one context-aware pass, keyed by node kind (box / text / wire /
wire-label) → its valid properties; anything else warns (errors under
`--strict`), LSP-formatted (`file:line:col`).

- **Property on the wrong node** — `translate`/`pin`/`padding`/`width` on a wire
  do nothing; error and suggest a `|plain|` label.
- **Unknown property**, with a "did you mean" hint (`paddding:` → `padding`).
- **Out-of-range / wrong-shape value** — `translate: 0 -10 0`, `pin: middle` —
  point at the offending token.
- Fold in the existing grid-prop checks (`cell` / `span` / `columns`).

## Colour palette + gradients (idea — explore later)

Make lini *pretty by default*: a curated colour system where the easy path is
the pretty path and making things ugly takes more syntax. One mechanism, layered
on the existing `light-dark()` theme vars — so everything themes and flips
dark/light for free, and bakes to literals for resvg/email.

**Palette** — built-in `--lini-*` colours, each a `light-dark()` pair, native
`--name` (themeable, host-overridable):

- *Roles*: `--accent` + `--accent-2` (brand pair), plus today's
  `--fg` / `--bg` / `--fill` / `--stroke` / `--muted` / `--danger` / `--warn`.
- *Hues*: ~14 hand-tuned hues (`--red --orange --amber --yellow --green --teal
  --cyan --blue --indigo --violet --purple --pink --rose --gray`) — one tuned
  base each, with an optional `-100…-900` ramp reachable when wanted.
- Emit only the vars actually used (tree-shake the `@layer` block) so a big
  palette never bloats a small diagram.

**Gradients** — a `gradient()` paint value compiling to a deduped `<defs>`
`<linearGradient>` (a `GradientTable`, twin of the shadow `FilterTable`); the
node's paint becomes `url(#…)`. Stops are `var(--lini-*)`, so gradients theme and
flip dark/light. `objectBoundingBox` units fill any shape; works on **fill and
stroke**.

- `gradient` → the brand pair · `gradient(pink, purple)` → an auto-angled blend
  (pretty for any two hues) · `gradient(sunset)` → a curated preset ·
  `linear-gradient(135, …)` → full control (a custom angle is the "more syntax"
  gate; `gradient()` itself is angle-less and always lands on a flattering 135°).
- ~10 curated presets built from hue vars (sunset, ocean, candy, mint, aurora,
  ember, grape, dusk, sky, slate) for the fancy multi-stop look.
- Simple linear is plenty; radial comes ~free. Mesh isn't native SVG, so
  "multi-colour" = multi-stop. Gradient-on-text is a later step.

The taste lives in the system, not the user: curated hues + presets + an
auto-angle so *any* two colours look good.

## Animation (idea — later, very light)

Small, native CSS/SVG effects so the browser does the work and it degrades to a
static frame when baked: moving dashes/dots on wires (cf. d2), a gentle wobble, a
colour/shadow pulse, maybe animating a gradient. Live-only; currently a SPEC §19
non-goal to revisit.

## OKLCH colour output (idea)

The palette is generated from OKLCH but emitted as hex for renderer compatibility
(resvg / librsvg / email don't parse `oklch()`). Add an opt-in — a
`--color-space oklch` flag or similar — that emits the `--lini-*` palette and
gradient stops as `oklch(L C H)` for users who target modern browsers only. Hex
stays the default; oklch is the wide-gamut, perceptual path for those who can use
it. `oklch()` *input* already works (`fill: oklch(0.7, 0.14, 200)`).

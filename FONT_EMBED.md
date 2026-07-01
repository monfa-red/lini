# Font embedding — design (not yet implemented)

**Status:** design agreed in principle; four open choices remain (below). No code written yet.
Fonts vendored at `assets/fonts/Google_Sans_Code/`. Continue from here.

## Goal

Make standalone Lini SVGs render **faithfully and identically everywhere**, with the option
to stay lean. Today text is measured at compile time as monospace (0.6em advance) but the
SVG only *names* a system-monospace stack, so an arbitrary viewer renders with whatever it
has (or Times New Roman under resvg/librsvg), breaking the compile-time alignment.

## Font: Google Sans Code (SIL OFL 1.1)

- Vendored in `assets/fonts/Google_Sans_Code/` (roman + italic variable fonts, 24 statics, `OFL.txt`).
  We only actually need the **roman VF** (`GoogleSansCode-VariableFont_MONO,wght.ttf`, 133 KB);
  the statics/italic are kept "in case." Trim later if desired.
- **Monospace advance is exactly 0.6em** (upem 2000, advance 1200 → 0.6000). Matches Lini's
  `AVG_CHAR_WIDTH_RATIO` in `src/layout/text.rs` **exactly** → zero layout/measurement change.
- VF axes: `wght 300..400..800`, `MONO 0..1..1` (**default MONO=1 = monospace**). Use MONO=1
  always; the *Proportional* instances would break Lini's fixed-advance layout — never use them.
- OFL: embedding + subsetting allowed; **no "Reserved Font Name" clause** in the header, so no
  rename obligation (we still give `@font-face` a Lini-scoped family name to avoid colliding with
  a user's installed copy). Obligation: ship `OFL.txt` (→ `LICENSES/`, like Phosphor's MIT) and
  keep the copyright line.

## Pivotal constraint: resvg/librsvg ignore `@font-face`

Verified on resvg 0.47: `Warning: The @font-face rule is not supported. Skipped.` So a font
embedded in the SVG is honoured by **browsers** but **ignored by raster-CLI tools** (resvg,
librsvg — including our own snapshot→PNG pipeline). Embedding alone therefore does **not** give
faithful rasterization. This is why the design splits web (font) from static (outline).

## The three output modes

| Mode | Colors | Text | Font bytes in SVG | Faithful in | Selectable |
|---|---|---|---|---|---|
| **Default (web)** | `var()` (themeable) | real `<text>`, `font-family: "Google Sans Code", ‹mono fallbacks›` | none | browsers — perfect if GSC hosted/installed, graceful monospace fallback otherwise | ✅ (SEO) |
| **`--embed-font` (web, opt-in)** | `var()` | real `<text>` + **whole roman VF** via `@font-face` (base64) | ~178 KB | browsers, self-contained | ✅ |
| **`--static`** *(renamed `--bake-vars`)* | literal | **text outlined to `<path>`** (VF sampled per run's weight) | none (baked into geometry) | **everywhere** (resvg, librsvg, editors, browsers) | ❌ |

Consequences:
- **No font subsetter anywhere, pure Rust.** Web default = a family name (zero bytes). Web embed =
  the *whole* VF base64'd (honestly "not optimized" — the optimized web path is "host our font on
  your page"; fallback stack means lazy users still get a readable result). Static = outline via
  `ttf-parser` / `skrifa` glyph extraction (already pure Rust; dedupe unique glyphs with
  `<defs>`+`<use>`).
- **Arbitrary weights (400/500/600/700…) come free.** Outline samples the VF at any weight;
  browser embed/host resolves the axis. No fixed weight set to pick.
- **Our own resvg pipeline gets better:** `--static` outlines → pixel-identical PNGs with no font
  needed (today `--bake-vars` leaves text and resvg falls back).

## Open decisions (decide, then implement)

1. **Rename `--bake-vars` → `--static`** (recommended; reads as "static SVG") or `--flatten`.
   It now does *both* var-baking **and** text-outlining. Keep `--bake-vars` as a hidden deprecated
   alias (same treatment as the no-op `--standalone`).
2. **`font` cargo feature** (default-on, mirrors `icons`) gating the bundled VF *bytes* used by
   `--embed-font` and `--static` outlining. Default web mode (name only) needs no bytes, so it works
   under `--no-default-features` too.
3. **Which font files to keep committed** — lean (roman VF + italic VF + `OFL.txt`) vs the whole
   2.2 MB folder. Currently the whole folder is committed per owner's "keep them in case."
4. **v1 scope:** all three modes; **defer italic** (separate VF, rare in diagrams) and per-diagram
   subsetting (unneeded by this design). Confirm.

## Implementation pointers

- `src/layout/text.rs` — `AVG_CHAR_WIDTH_RATIO = 0.6` (leave as-is; GSC mono = 0.6em).
- `src/render/rules.rs` — where the `--lini-font-family` / font-family stack is emitted; lead the
  stack with `"Google Sans Code"`. This is a name only (no bytes) → safe even without the feature.
- `src/main.rs` — the `--bake-vars` flag (rename + expand); add `--embed-font`; the `Options` struct.
- Icons are the exact precedent: `Cargo.toml [features] default=["icons"]`, `#[cfg(feature="icons")]`,
  `include_bytes!`/`include_str!` of an `assets/` payload, and an `xtask` regen command
  (`extract-icons`). Mirror for `font`.
- **Verify first in the plan:** confirm `ttf-parser`/`skrifa` VF outline extraction at a given
  `wght`, and confirm the outline PNGs match under resvg (they should — no font needed). Also
  reconfirm resvg still ignores `@font-face` (so `--embed-font` is documented browser-only).

## Already shipped alongside this (separate work, on `main`)

Node `stroke-width` 1.5 → **1.6** (icon stays 2; group + sequence frames stay 1); filled markers
(arrow/dot/circle/diamond) **+1px** via `head_size` in `src/render/markers.rs` (crow/ER unchanged).
`tests/linking.rs`: `links_hard` crossings re-pinned 5→7 (lawful); `a_walled_in_link_is_reported_impossible`
is `#[ignore]`d — at 1.6 the gap-growth lever routes `core→n2` instead of reporting impossible;
routing wiring to be revisited.

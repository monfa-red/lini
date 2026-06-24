# Phosphor Icons — Implementation Plan

> **For agentic workers:** implement task-by-task, top to bottom. Steps use checkbox (`- [ ]`) syntax — tick them as you go so a fresh session can resume mid-plan. Each phase ends with a commit (a "stage"). After each phase: `cargo fmt && cargo test && cargo clippy` must be green before committing.

**Goal:** Make `|icon|` real — embed the Phosphor *duotone* icon set as inline SVG paths, painted like any Lini shape (`fill` body, `stroke` line, counter-scaled `stroke-width`), tree-shaken to only the icons a diagram uses.

**Architecture:** One vendored, sorted text file (`assets/phosphor-duotone.txt`) holds *path data only* (`name<TAB><role><d>…`). It's `include_str!`'d behind a default-on `icons` cargo feature and queried by name via a lazily-built `OnceLock<Vec<&'static str>>` + binary search (zero deps, zero allocation, fast compile). `|icon|` resolves a `symbol:` property to those paths and the renderer scales them into the node's box with a baked counter-scaled stroke. A committed `examples/extract-icons.rs` regenerates the data file from a Phosphor checkout.

**Tech Stack:** Rust 2024 (rustc ≥ 1.88), std only (no new deps), `insta` snapshots, `resvg` for the PNG visual check.

## Global Constraints

- **No `unsafe`.** Ever.
- **No new dependencies** — `clap` is the only runtime dep; the icon path is std-only (`OnceLock`, `include_str!`, slices).
- **SPEC.md is the law.** This plan implements SPEC §6/§7/§10/§11/§19 as already edited; if code and SPEC disagree, the SPEC wins (or update both deliberately).
- **One concept per file**; split a module past ~500 LOC.
- **Reproducible output** — compilation is byte-identical; the vendored data file is pinned to a specific Phosphor tag.
- **No "Co-Authored-By" lines** in commits. Descriptive messages, one purposeful change each.
- **Before every commit:** `cargo fmt` (CI fails on any diff), `cargo test`, `cargo clippy`.
- **Verify SVG visually** — render to PNG with `resvg` and read it; don't ask the user to spot-check.
- Commit to `main` is fine (user-authorised). **Do not push** — pushing to `main` is deferred to the user.

## Locked Decisions (full context for a cold start)

| Decision | Choice |
|---|---|
| Icon set | **Phosphor**, MIT — **duotone weight only** (it's a superset: the line layer == regular geometry, so single-tone is free) |
| Naming syntax | `symbol:` **property** — `\|icon\| { symbol: heart }`. *Not* a type suffix (would collide with the `::` derive operator) and *not* the label (kept free for optional text). Composes with `::`-derive + cascade for free. |
| Paint model | Paints like a box: `fill` = body, `stroke` = line, `stroke-width` = weight. Each stored path has a role — `Line`, `Fill`, or `Both`. |
| Defaults | `fill: --icon-fill` (a soft, *visible* grey ⇒ duotone reads out of the box), `stroke: --stroke` (matches borders/wires), `stroke-width: 2`, size `32`. Single-tone = `fill: none`. |
| stroke-width | **Counter-scaled & baked**: drawn at `stroke-width × 256 / scaled-size` so on-screen weight is constant at any size and matches other strokes. |
| Sizing | `icon-size` 32, square; `width`/`height` adjust; non-square fits **uniformly** (scale `min(w,h)/256`), centred — no distortion. |
| Text inside | Optional label rides as centred text over the glyph. |
| `icon-variant` | **Dropped** (it was Material-Symbols-only). |
| Storage | `include_str!` a sorted text file; lazy `OnceLock<Vec<&'static str>>` + `binary_search`. Zero-copy `&'static str` slices. |
| Packaging | Default-on cargo feature `icons`. `--no-default-features` ⇒ lean binary; `\|icon\|` then errors with a build hint. |
| Sourcing | Vendor a pinned snapshot (record the exact tag). `examples/extract-icons.rs` regenerates the file from `raw/duotone/*.svg`. |

## Data file format (`assets/phosphor-duotone.txt`)

- UTF-8, LF line endings, **sorted by icon name**, one icon per line.
- Optional leading `#` comment lines (provenance); the loader skips them.
- Line: `name<TAB>FIELD<TAB>FIELD…` where each `FIELD` = one role char + the path `d` (no separator; `d` always starts with `M`).
  - Role char: `L` = line (stroke only), `F` = fill only, `B` = both (fill + stroke, same geometry).
- Example:
  ```
  heart	BM128,224S24,168,24,102A54,54,0,0,1,78,48c22.59,12.31,50,32,8.06-19.69,27.41-32,50-32a54,54,0,0,1,54,54C232,168,128,224,128,224Z
  user	BM128,32a64,64,0,1,0,64,64A64,64,0,0,0,128,32Z	LM32,216c19.37-33.47,54.55-56,96-56s76.63,22.53,96,56
  ```

## File Structure

| File | Responsibility |
|---|---|
| `Cargo.toml` | add `[features] default = ["icons"]`, `icons = []` |
| `assets/phosphor-duotone.txt` | **new** — vendored path data (generated) |
| `LICENSES/phosphor` | **new** — Phosphor MIT text + pinned tag |
| `examples/extract-icons.rs` | **new** — std-only regenerator (offline tool) |
| `src/icon/mod.rs` | **new** — embedded table: `Role`, `lookup`, `names`, `ENABLED` |
| `src/lib.rs` | add `mod icon;` |
| `src/resolve/defaults.rs` | add `--icon-fill` visual var |
| `src/desugar/bundles.rs:74` | icon bundle: `fill: --icon-fill`, `stroke: --stroke`, `stroke-width: 2`, size `32` |
| `src/resolve/scene.rs:192-202` | read `symbol` (not label); keep label as text; validate name |
| `src/render/primitives.rs:346-379` | rewrite `emit_icon` — real paths, counter-scaled stroke, optional text |
| `samples/icons.lini` | **new** — feature sample |
| `tests/*`, `README.md`, `TODO.md` | snapshots + docs cleanup |

---

## Phase 1 — Vendor the data + the regenerator

**Goal:** `assets/phosphor-duotone.txt` exists (pinned, sorted, path-only), the `icons` feature is wired, the regenerator is committed, attribution is in place.

**Files:** Create `assets/phosphor-duotone.txt`, `examples/extract-icons.rs`, `LICENSES/phosphor`; modify `Cargo.toml`.

- [ ] **Step 1: Add the feature.** In `Cargo.toml`, after `[dependencies]`:

```toml
[features]
default = ["icons"]
icons = []
```

- [ ] **Step 2: Find the latest stable `phosphor-icons/core` tag** (e.g. via `gh release list -R phosphor-icons/core` or the repo tags). Record it; it goes in the data-file header and `LICENSES/phosphor`.

- [ ] **Step 3: Fetch that tag's tarball once** (single download, not per-icon), e.g.:

```bash
mkdir -p /tmp/phosphor && curl -sL \
  https://github.com/phosphor-icons/core/archive/refs/tags/<TAG>.tar.gz \
  | tar -xz -C /tmp/phosphor --strip-components=1
# duotone SVGs now under /tmp/phosphor/raw/duotone/*.svg
```

- [ ] **Step 4: Write `examples/extract-icons.rs`** (std-only). It reads a directory of `*-duotone.svg`, emits the sorted data file to stdout. Element handling:
  - Skip the bounding `<rect width="256" height="256" fill="none"/>`.
  - Convert each drawable element to a path `d`:
    - `circle cx cy r` → `M{cx-r},{cy}a{r},{r} 0 1,0 {2r},0a{r},{r} 0 1,0 {-2r},0Z`
    - `rect x y w h` → `M{x},{y}h{w}v{h}h{-w}Z`
    - `line x1 y1 x2 y2` → `M{x1},{y1}L{x2},{y2}`
    - `polygon pts` → `M{p0}L{p1}…Z`; `polyline` → same without `Z`
    - `path d` → `d` verbatim
  - Classify by attributes: has `stroke=` (and `fill="none"`) ⇒ a **line** path; has `opacity="0.2"` / no stroke ⇒ a **fill** path.
  - Dedup geometry: a `d` present in *both* the line set and the fill set ⇒ role `B`; line-only ⇒ `L`; fill-only ⇒ `F`. Preserve the line-layer order.
  - Icon name = filename minus `-duotone.svg`.
  - Print a `# phosphor-icons/core <TAG> — duotone — <count> icons — regenerate: cargo run --example extract-icons -- <dir>` header, then sorted lines.

  Skeleton (fill in the element parsers; the SVGs are flat and regular, so simple `str` scanning suffices — no XML crate):

```rust
//! Regenerate assets/phosphor-duotone.txt from a Phosphor duotone SVG folder.
//! Usage: cargo run --example extract-icons -- <core>/raw/duotone > assets/phosphor-duotone.txt
use std::{collections::BTreeMap, fs, path::PathBuf};

#[derive(Clone, Copy, PartialEq)]
enum Role { Line, Fill, Both }

fn main() {
    let dir = std::env::args().nth(1).expect("path to raw/duotone");
    let mut icons: BTreeMap<String, Vec<(Role, String)>> = BTreeMap::new();
    for entry in fs::read_dir(&dir).expect("read dir") {
        let path: PathBuf = entry.unwrap().path();
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else { continue };
        let Some(name) = stem.strip_suffix("-duotone") else { continue };
        let svg = fs::read_to_string(&path).unwrap();
        icons.insert(name.to_string(), paths_of(&svg)); // paths_of: parse + convert + classify + dedup
    }
    print!("# phosphor-icons/core <TAG> — duotone — {} icons\n", icons.len());
    for (name, paths) in &icons {
        print!("{name}");
        for (role, d) in paths {
            let flag = match role { Role::Line => 'L', Role::Fill => 'F', Role::Both => 'B' };
            print!("\t{flag}{d}");
        }
        println!();
    }
}

// fn paths_of(svg: &str) -> Vec<(Role, String)> { /* element scan + convert + classify + dedup */ }
```

- [ ] **Step 5: Generate the file:** `cargo run --example extract-icons -- /tmp/phosphor/raw/duotone > assets/phosphor-duotone.txt`. Sanity-check: `wc -l` ≈ icon count + 1; `grep '^heart\t' assets/phosphor-duotone.txt` shows the heart line; no `<`/`>`/`svg` anywhere (`! grep -q '<' assets/phosphor-duotone.txt`).

- [ ] **Step 6: Add `LICENSES/phosphor`** — the Phosphor MIT licence text + a line naming the pinned tag and the source repo.

- [ ] **Step 7: Commit.**

```bash
git add Cargo.toml assets/phosphor-duotone.txt examples/extract-icons.rs LICENSES/phosphor
git commit -m "feat(icons): vendor Phosphor duotone path data + regenerator

Add a default-on `icons` cargo feature and a single sorted, path-only data
file extracted from Phosphor's duotone weight (pinned tag <TAG>), plus a
std-only `examples/extract-icons.rs` to regenerate it. No SVG wrapper stored."
```

---

## Phase 2 — The icon table module

**Goal:** `icon::lookup(name)` returns the icon's paths as `(Role, &'static str)` with O(log n) lookup and zero allocation; `icon::names()` lists them; `icon::ENABLED` reflects the feature. Call sites stay cfg-free.

**Files:** Create `src/icon/mod.rs`; modify `src/lib.rs`.

**Interfaces — Produces:**
- `pub enum Role { Line, Fill, Both }`
- `pub const ENABLED: bool`
- `pub fn lookup(name: &str) -> Option<impl Iterator<Item = (Role, &'static str)>>`
- `pub fn names() -> impl Iterator<Item = &'static str>`

- [ ] **Step 1: Write `src/icon/mod.rs`.**

```rust
//! Embedded Phosphor (duotone) icon set — path data only, queried by name.
//!
//! Source: phosphor-icons/core <TAG>, duotone weight, MIT (see LICENSES/phosphor).
//! The data lives in `assets/phosphor-duotone.txt` (sorted `name\t<role><d>…`);
//! regenerate it with `cargo run --example extract-icons -- <core>/raw/duotone`.
//!
//! Lookup builds the line index once (`OnceLock`) and binary-searches it. Every
//! returned `d` borrows the embedded bytes — no allocation, no full parse.

/// Which duotone layer a stored path belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    /// Stroke only — the line layer.
    Line,
    /// Fill only — a background region with no matching stroke.
    Fill,
    /// One geometry that is both the fill region and the line.
    Both,
}

/// Whether the icon set was compiled in (the `icons` feature).
pub const ENABLED: bool = cfg!(feature = "icons");

#[cfg(feature = "icons")]
const DATA: &str = include_str!("../../assets/phosphor-duotone.txt");
#[cfg(not(feature = "icons"))]
const DATA: &str = "";

/// Data lines (`name\t<role><d>…`), sorted by name; comments/blanks stripped.
fn lines() -> &'static [&'static str] {
    use std::sync::OnceLock;
    static LINES: OnceLock<Vec<&'static str>> = OnceLock::new();
    LINES.get_or_init(|| {
        DATA.lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect()
    })
}

fn name_of(line: &str) -> &str {
    line.split_once('\t').map_or(line, |(n, _)| n)
}

fn parse_field(field: &str) -> Option<(Role, &str)> {
    let (flag, d) = field.split_at_checked(1)?;
    let role = match flag {
        "L" => Role::Line,
        "F" => Role::Fill,
        "B" => Role::Both,
        _ => return None,
    };
    Some((role, d))
}

/// The paths for `name`, or `None` if there is no such icon (or icons are off).
pub fn lookup(name: &str) -> Option<impl Iterator<Item = (Role, &'static str)>> {
    let lines = lines();
    let i = lines.binary_search_by(|l| name_of(l).cmp(name)).ok()?;
    let (_, rest) = lines[i].split_once('\t')?;
    Some(rest.split('\t').filter_map(parse_field))
}

/// Every known icon name, sorted — used for "did you mean …?" suggestions.
pub fn names() -> impl Iterator<Item = &'static str> {
    lines().iter().copied().map(name_of)
}

#[cfg(all(test, feature = "icons"))]
mod tests {
    use super::*;

    #[test]
    fn known_icon_resolves_to_at_least_one_path() {
        let paths: Vec<_> = lookup("heart").expect("heart exists").collect();
        assert!(!paths.is_empty());
        assert!(paths.iter().all(|(_, d)| d.starts_with('M')));
    }

    #[test]
    fn unknown_icon_is_none() {
        assert!(lookup("definitely-not-an-icon").is_none());
    }

    #[test]
    fn names_are_sorted_and_nonempty() {
        let all: Vec<_> = names().collect();
        assert!(all.len() > 1000, "expected the full set, got {}", all.len());
        assert!(all.windows(2).all(|w| w[0] < w[1]), "names must be sorted & unique");
    }
}
```

- [ ] **Step 2: Register the module** in `src/lib.rs` (add `mod icon;` with the other `mod` lines; make it `pub mod icon;` only if integration tests need it — otherwise keep private and let `render`/`resolve` use `crate::icon`).

- [ ] **Step 3: Run the tests.** `cargo test icon::` → PASS. Then `cargo test --no-default-features icon::` → the cfg'd tests compile out; build still succeeds.

- [ ] **Step 4: Commit.**

```bash
git add src/icon/mod.rs src/lib.rs
git commit -m "feat(icons): embedded icon table with lazy binary-search lookup

src/icon: include_str! the data, build the line index once via OnceLock, and
binary-search by name. Returns zero-copy (Role, &str) paths. ENABLED tracks the
feature so call sites stay cfg-free."
```

---

## Phase 3 — `symbol` property, defaults, validation

**Goal:** `|icon| { symbol: heart }` resolves; the label is free for text; missing/unknown symbols error well; defaults give a grey-bodied duotone at size 32.

**Files:** modify `src/desugar/bundles.rs`, `src/resolve/defaults.rs`, `src/resolve/scene.rs` (and wherever the icon label was consumed).

**Interfaces — Consumes:** `crate::icon::{lookup, names, ENABLED}` from Phase 2.

- [ ] **Step 1: Add the `--icon-fill` var.** In `src/resolve/defaults.rs`, after the `group-fill` block, add a *visible* soft grey (tunable in Phase 4's visual check):

```rust
    set_visual(
        &mut t,
        "icon-fill",
        light_dark(rgba(0.0, 0.0, 0.0, 0.16), rgba(255.0, 255.0, 255.0, 0.18)),
    );
```

- [ ] **Step 2: Update the icon bundle.** In `src/desugar/bundles.rs`, replace the `Icon` arm (currently `Icon => vec![var("fill", "stroke"), n("width", 24.0), n("height", 24.0)]`):

```rust
        Icon => vec![
            var("fill", "icon-fill"),
            var("stroke", "stroke"),
            n("stroke-width", 2.0),
            n("width", 32.0),
            n("height", 32.0),
        ],
```

Update the neighbouring `slant_carries_skew_icon_carries_size` test's expected width/height to `32.0`.

- [ ] **Step 3: Resolve `symbol`, free the label.** In `src/resolve/scene.rs` (the `is_icon` branch, ~192-202): stop using `first_text`/id as the glyph name. Instead read the `symbol` attribute; **do not** consume the children, so a bare `"3"` stays a normal centred text child. Validate:

```rust
    if shape == ShapeKind::Icon {
        let symbol = attrs.get("symbol").and_then(|v| v.as_ident_or_str()); // helper: Ident|String → &str
        match symbol {
            None => return Err(Error::at(node.span, "'|icon|' requires a 'symbol' (e.g. { symbol: heart })")),
            Some(name) if crate::icon::lookup(name).is_none() => {
                let msg = if !crate::icon::ENABLED {
                    "icons are not built in — rebuild with the `icons` feature".to_string()
                } else {
                    match nearest(name, crate::icon::names()) {
                        Some(s) => format!("unknown icon '{name}' — did you mean '{s}'?"),
                        None => format!("unknown icon '{name}'"),
                    }
                };
                return Err(Error::at(node.span, msg));
            }
            Some(_) => {}
        }
    }
```

  Add a small `nearest(target, candidates)` helper (Levenshtein/edit-distance ≤ 2, std-only) — or reuse one if the codebase already has a "did you mean" helper (check `src/lint.rs` / error paths first; the TODO mentions a planned hint table — don't duplicate it). Keep the icon's children flowing through `resolve_child` like any other node (drop the `if !is_icon` guard that skipped them), so the optional label renders as text.

- [ ] **Step 4: Tests** (in `tests/resolution.rs` or inline): `|icon| { symbol: heart }` resolves with `attrs["symbol"] == "heart"`; `|icon|` with no symbol errors; `|icon| { symbol: nope }` errors with a suggestion; `|icon| { symbol: heart } "3"` keeps a text child `"3"`.

- [ ] **Step 5:** `cargo test` → PASS. `cargo clippy` clean.

- [ ] **Step 6: Commit.**

```bash
git add src/desugar/bundles.rs src/resolve/defaults.rs src/resolve/scene.rs tests/
git commit -m "feat(icons): symbol property, --icon-fill default, name validation

|icon| takes its name from `symbol:` (label freed for optional text); defaults
to a soft-grey duotone (--icon-fill) at size 32; missing/unknown symbols error
with a build hint or a nearest-name suggestion."
```

---

## Phase 4 — Render the real icon

**Goal:** replace the placeholder square with scaled Phosphor paths — duotone via `fill`+`stroke`, counter-scaled baked `stroke-width`, round caps, optional centred text. Inlining gives tree-shaking for free.

**Files:** modify `src/render/primitives.rs` (`emit_icon`, ~346-379); touch `src/render/rules.rs` only if the `lini-icon` rule fights the element-level paint.

- [ ] **Step 1: Rewrite `emit_icon`.** Look up the symbol; scale the 256-grid into the box; paint per role. Every path states explicit `fill`/`stroke` so it's independent of inheritance (matching the existing element-level approach).

```rust
fn emit_icon(out: &mut String, n: &PlacedNode, indent: &str, vars: &VarTable, opts: &Options) {
    let Some(ResolvedValue::String(name)) | Some(ResolvedValue::Ident(name)) = n.attrs.get("symbol")
    else {
        return; // resolve already errored on a missing symbol
    };
    let Some(paths) = crate::icon::lookup(name) else { return };

    let (w, h) = (n.bbox.w(), n.bbox.h());
    let size = w.min(h);                     // uniform fit, square-centred
    let s = size / 256.0;
    let fill = attr_or_var(&n.attrs, "fill", "icon-fill", vars, opts);
    let stroke = attr_or_var(&n.attrs, "stroke", "stroke", vars, opts);
    let sw = n.attrs.number("stroke-width").unwrap_or(2.0) / s; // counter-scale, baked

    writeln!(out, r#"{indent}<g transform="scale({}) translate(-128 -128)">"#, num(s)).unwrap();
    for (role, d) in paths {
        let (f, st) = match role {
            crate::icon::Role::Line => ("none".to_string(), stroke.clone()),
            crate::icon::Role::Fill => (fill.clone(), "none".to_string()),
            crate::icon::Role::Both => (fill.clone(), stroke.clone()),
        };
        writeln!(
            out,
            r#"{indent}  <path d="{}" fill="{}" stroke="{}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"/>"#,
            d, f, st, num(sw),
        )
        .unwrap();
    }
    writeln!(out, "{indent}</g>").unwrap();

    // Optional label (SPEC §7): centred text over the glyph, like any text node.
    if let Some(label) = n.label.as_deref().filter(|s| !s.is_empty()) {
        let color = attr_or_var(&n.attrs, "color", "text-color", vars, opts);
        writeln!(
            out,
            r#"{indent}<text x="0" y="0" text-anchor="middle" dominant-baseline="central" font-size="{}" fill="{}">{}</text>"#,
            num(size * 0.4), color, escape_xml(label),
        )
        .unwrap();
    }
}
```

  Note: with `fill: none` set by the user, `Both`/`Fill` paths render `fill="none"` ⇒ a clean single-tone line icon. If the icon's `<g>` also carries a `style=` paint diff, the per-path attrs still win (they're explicit) — but verify in Step 3; if it doubles up confusingly, exclude `ShapeKind::Icon` from `node_style_attr`'s paint loop (icon paint is element-level).

  Confirm `n.label` still carries the optional text after Phase 3 (the label now means "extra text", not the glyph name). If labels for icons aren't populated, read the first text child instead.

- [ ] **Step 2: Add a rendering snapshot** in `tests/rendering.rs`: assert the SVG for `x |icon| { symbol: heart }` contains a `<path d="M…"`, `stroke-linecap="round"`, the counter-scaled `stroke-width` (for 32px: `2 × 256 / 32 = 16`), and **no** `<rect … fill="none" …/>` placeholder. Add a single-tone case (`fill: none` ⇒ every path `fill="none"`) and a two-tone case (`fill: --teal-wash; stroke: --teal-ink`).

- [ ] **Step 3: Run + review.** `cargo test`; `cargo insta review` to accept new snapshots after eyeballing them.

- [ ] **Step 4: PNG visual check** (AGENTS.md — do this yourself):

```bash
cargo run -- samples/icons.lini -o /tmp/icons.svg --bake-vars   # sample created in Phase 5; or a scratch file now
resvg /tmp/icons.svg /tmp/icons.png && open /tmp/icons.png       # read it
```

  Confirm: line weight matches the box borders/wires; the grey duotone body reads clearly; hued icons (`--teal-wash`/`--teal-ink`) look like cards; `user` shows a faint head with a line shoulders (no filled shoulders). Tune `--icon-fill` (Phase 3 Step 1) if the body is too faint/heavy; re-snapshot.

- [ ] **Step 5: Commit.**

```bash
git add src/render/primitives.rs src/render/rules.rs tests/ src/snapshots/
git commit -m "feat(icons): render Phosphor duotone paths with counter-scaled stroke

emit_icon scales the 256-grid into the node box, paints each path by role
(fill body / stroke line / both), bakes a counter-scaled stroke-width so weight
holds at any size, and rounds caps/joins. Optional label rides as centred text.
Inlining only the used icon = automatic tree-shaking."
```

---

## Phase 5 — Sample, snapshots, docs cleanup

**Goal:** a feature sample under `samples/`, conformance snapshot, and the stale Material-Symbols references removed from the other docs.

**Files:** create `samples/icons.lini`; update `tests/conformance.rs` snapshots, `README.md`, `TODO.md`.

- [ ] **Step 1: Write `samples/icons.lini`** — show the spread: a default duotone, a single-tone (`fill: none`), a hued two-tone, a `::`-derived reusable icon (`|warn::icon| { symbol: warning-circle; stroke: --amber-ink }`), an icon with a text label, and a couple wired together so routing-to-icons is exercised.

- [ ] **Step 2: Conformance snapshot.** `cargo test` regenerates the `insta` snapshot for the new sample; `cargo insta review` after eyeballing the SVG.

- [ ] **Step 3: PNG visual check** of `samples/icons.lini` (as Phase 4 Step 4) — final look.

- [ ] **Step 4: README.** Fix the shapes paragraph (currently "icon (Material Symbols)"): describe it as a Phosphor duotone icon, `symbol:`-named, paints like a shape. Add an icon line to the shapes example if natural.

- [ ] **Step 5: TODO.md.** The "Icons — duotone, palette-themed" entry is now built — remove it or move it to a short "done" note; drop the obsolete Material-Symbols mention.

- [ ] **Step 6:** `cargo fmt && cargo test && cargo clippy` — all green.

- [ ] **Step 7: Commit.**

```bash
git add samples/icons.lini tests/ src/snapshots/ README.md TODO.md
git commit -m "docs(icons): sample, conformance snapshot, README/TODO cleanup"
```

---

## How to resume

Open this file, find the first unchecked `- [ ]`, and continue. Phases are independent and each ends green + committed, so a mid-plan restart only needs the current phase re-read. `git log --oneline` shows which stages already landed.

## Self-review (done while writing — recorded for the executor)

- **Spec coverage:** symbol naming (§7/§10) → Phase 3; duotone fill/stroke + counter-scaled stroke-width (§7) → Phases 3–4; `icon-size` 32 (§6/§11.5) → Phase 3 bundle; `--icon-fill` (§10/§11.1) → Phase 3; tree-shaking by inlining (§7/§13) → Phase 4; `icons` feature + path-only data file (§7/§19) → Phases 1–2; dropped `icon-variant` (§10) → already gone in SPEC, no code path remains (verify in Phase 3). ✓
- **No placeholders:** the only deferred detail is the latest Phosphor tag (resolved live in Phase 1 Step 2) and the `paths_of`/`nearest` bodies (algorithms fully specified, mechanical to write). ✓
- **Type consistency:** `Role {Line, Fill, Both}`, `lookup → Option<impl Iterator<Item=(Role, &'static str)>>`, `names`, `ENABLED` used identically in Phases 2–4. ✓

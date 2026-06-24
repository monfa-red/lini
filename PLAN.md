# Phosphor Icons — Implementation Plan

> **For agentic workers:** implement top to bottom; tick the `- [ ]` boxes as you go so a fresh session can resume mid-plan. Each phase ends with a commit (a "stage"). After each phase: `cargo fmt --all && cargo test && cargo clippy` must be green before committing.

**Goal:** Make `|icon|` real — embed the Phosphor *duotone* icon set as inline SVG geometry, painted like any Lini shape (`fill` body, `stroke` line, counter-scaled `stroke-width`), tree-shaken to only the icons a diagram uses.

**Architecture:** One vendored, sorted text file (`assets/phosphor-duotone.txt`) holds *geometry only* — each icon is a tab-separated list of cleaned SVG fragments (`<path…/>`, `<circle…/>`, … with their transforms), each prefixed by a one-char paint role. It is `include_str!`'d behind a default-on `icons` cargo feature and queried by name via a lazily-built `OnceLock<Vec<&'static str>>` + binary search (zero deps, zero allocation, fast compile). The renderer scales the 256-grid into the node's box and wraps the role-grouped fragments in paint `<g>`s. A `cargo xtask extract-icons` command regenerates the data file from a Phosphor checkout.

**Tech Stack:** Rust 2024 (rustc ≥ 1.88), std only (no new deps), `insta` snapshots, `resvg` for the PNG visual check.

## Global Constraints

- **No `unsafe`.** No new dependencies (clap stays the only runtime dep; icons are std-only).
- **SPEC.md is the law** — implement §6/§7/§10/§11/§19 as edited; if code and SPEC disagree, the SPEC wins.
- **One concept per file**; split past ~500 LOC. Standard idioms; don't fight `rustfmt`/`clippy`.
- **Reproducible output** — byte-identical; the data file is pinned to **phosphor-icons/core v2.1.1 (2b75f3ad)**.
- **No "Co-Authored-By" lines.** Descriptive messages, one purposeful change each. **Backticks in `-m` trigger zsh command substitution — commit via `git commit -F <file>`.**
- **Before each commit:** `cargo fmt --all`, `cargo test`, `cargo clippy`.
- **Verify SVG visually** — render to PNG with `resvg` and read it.
- Commit to `main` (user-authorised). **Do not push** — that's the user's call.

## Locked Decisions

| Decision | Choice |
|---|---|
| Icon set | **Phosphor** v2.1.1, MIT — **duotone weight only** (superset: line layer == regular geometry) |
| Naming | `symbol:` **property** — `\|icon\| { symbol: heart }`. Composes with `::`-derive + cascade; label freed for optional text. |
| Geometry | **Preserved verbatim** (paths/circles/ellipses/rects/lines/polys + their `transform`s) — *not* baked to a single `d` (5 files have `<path transform>`; baking is fragile). |
| Paint roles | `Fill` (faint body → `fill`), `Line` (outline → `stroke`), `Solid` (foreground dot → ink `stroke` as fill), `Both` (one shape, body + outline). Counts reconcile exactly across the 9083 elements. |
| Defaults | `fill: --icon-fill` (soft, *visible* grey ⇒ duotone reads by default), `stroke: --stroke`, `stroke-width: 2`, size `32`. Single-tone = `fill: none` (drops body; `Solid`/`Line` stay ink). |
| stroke-width | **Counter-scaled & baked**: `stroke-width × 256 / size`, on the line paint group, so weight is constant at any size. |
| Sizing | `icon-size` 32, square; `width`/`height` adjust, fit **uniformly** (`min(w,h)/256`), centred. |
| Text inside | Optional label rides as centred text over the glyph. |
| Storage | `include_str!` a sorted text file; lazy `OnceLock<Vec<&'static str>>` + `binary_search`; zero-copy `&'static str` fragments. |
| Packaging | Default-on cargo feature `icons`; `--no-default-features` ⇒ lean binary, `\|icon\|` errors with a build hint. |
| Tooling | `cargo xtask extract-icons <dir>` (an `xtask` workspace crate) regenerates the data file; future home for `embed-font`. |

## Data file format (`assets/phosphor-duotone.txt`)

- UTF-8, LF, **sorted by name**, one icon per line; leading `#` comment lines (provenance) are skipped by the loader.
- Line: `name<TAB>FIELD<TAB>FIELD…`; each `FIELD` = a one-char **role flag** (`F`/`L`/`S`/`B`) immediately followed by a cleaned, self-closing SVG fragment (geometry attrs + any `transform`; paint attrs stripped). Fragments contain spaces but never tabs, so TAB-splitting is safe.
- Example (verified):
  ```
  heart	B<path d="M128,224S24,168,…Z"/>
  user	B<circle cx="128" cy="96" r="64"/>	L<path d="M32,216c19.37-33.47,…"/>
  atom	B<ellipse cx="128" cy="128" rx="44.13" ry="116.33" transform="translate(-53.02 128) rotate(-45)"/>	L<ellipse …/>	S<circle cx="128" cy="128" r="12"/>
  ```

## File Structure

| File | Responsibility | State |
|---|---|---|
| `Cargo.toml` | `[features] default=["icons"]`, `icons=[]`; `[workspace] members=["xtask"]` | ✅ done |
| `.cargo/config.toml` | `xtask` alias | ✅ done |
| `xtask/{Cargo.toml,src/main.rs}` | std-only regenerator (`extract-icons`) | ✅ done |
| `assets/phosphor-duotone.txt` | vendored geometry (427K, 1512 icons) | ✅ done |
| `LICENSES/phosphor` | Phosphor MIT + pinned tag | ✅ done |
| `src/icon/mod.rs` | embedded table: `Role`, `lookup`, `names`, `ENABLED` | Phase 2 |
| `src/lib.rs` | `mod icon;` | Phase 2 |
| `src/resolve/defaults.rs` | `--icon-fill` var | Phase 3 |
| `src/desugar/bundles.rs` | icon bundle: fill `--icon-fill`, stroke `--stroke`, stroke-width 2, size 32 | Phase 3 |
| `src/resolve/scene.rs` | read `symbol` (not label); keep label as text; validate | Phase 3 |
| `src/render/primitives.rs` | rewrite `emit_icon` — role-grouped scaled fragments + optional text | Phase 4 |
| `samples/icons.lini`, tests, `README.md`, `TODO.md` | sample, snapshots, docs | Phase 5 |

---

> **Status: all phases complete.** Phase 1 landed as its own commit; Phases 2–4
> (implementation) and Phase 5 (sample + docs) follow as the next stages.

## Phase 1 — Vendor data + regenerator ✅ DONE

- [x] `[features]` (default `icons`) + `[workspace]` (`xtask`, default-members `["."]`) in `Cargo.toml`.
- [x] `.cargo/config.toml` alias `xtask = "run --package xtask --"`.
- [x] `xtask/` crate: `extract-icons <dir>` reads `*-duotone.svg`, drops the 256 bounding rect + `<g opacity="0.2">` wrappers (inner = fill role), strips paint attrs, classifies role, dedups fill∩line→Both, writes sorted `assets/phosphor-duotone.txt`.
- [x] Generated the file from `/Users/abbas/workspace/core/raw/duotone` → 1512 icons, 427K, verified (heart=B, user=B+L, atom has S nucleus, squares-four=4×B, transforms preserved, no SVG cruft).
- [x] `LICENSES/phosphor` (MIT + v2.1.1/2b75f3ad).
- [x] `cargo fmt --all`, build (default + `--no-default-features`), `cargo clippy -p xtask` all green.
- [ ] Commit (message via file): "feat(icons): vendor Phosphor duotone geometry + xtask regenerator".

---

## Phase 2 — The icon table module ✅ DONE

**Goal:** `icon::lookup(name)` → the icon's `(Role, &'static str fragment)`s, O(log n), zero-alloc; `icon::names()`; `icon::ENABLED`. Call sites stay cfg-free.

**Files:** Create `src/icon/mod.rs`; modify `src/lib.rs`.

**Produces:**
- `pub enum Role { Fill, Line, Solid, Both }`
- `pub const ENABLED: bool`
- `pub fn lookup(name: &str) -> Option<impl Iterator<Item = (Role, &'static str)>>`
- `pub fn names() -> impl Iterator<Item = &'static str>`

- [ ] **Step 1: Write `src/icon/mod.rs`.**

```rust
//! Embedded Phosphor (duotone) icon set — geometry only, queried by name.
//!
//! Source: phosphor-icons/core v2.1.1, duotone, MIT (see LICENSES/phosphor).
//! Data lives in `assets/phosphor-duotone.txt` (sorted `name\t<role><fragment>…`);
//! regenerate via `cargo xtask extract-icons <core>/raw/duotone`.
//!
//! Lookup builds the line index once (`OnceLock`) and binary-searches it; every
//! returned fragment borrows the embedded bytes — no allocation, no full parse.

/// How a stored geometry fragment is painted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    /// Faint background body — painted with the icon's `fill`.
    Fill,
    /// Outline — stroked with the icon's `stroke`, no fill.
    Line,
    /// Solid foreground detail (a dot) — filled with the ink `stroke`.
    Solid,
    /// One shape that is both the body fill and the outline.
    Both,
}

/// Whether the icon set was compiled in (the `icons` feature).
pub const ENABLED: bool = cfg!(feature = "icons");

#[cfg(feature = "icons")]
const DATA: &str = include_str!("../../assets/phosphor-duotone.txt");
#[cfg(not(feature = "icons"))]
const DATA: &str = "";

/// Data lines (`name\t<role><fragment>…`), sorted by name; comments/blanks dropped.
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
    let (flag, frag) = field.split_at_checked(1)?;
    let role = match flag {
        "F" => Role::Fill,
        "L" => Role::Line,
        "S" => Role::Solid,
        "B" => Role::Both,
        _ => return None,
    };
    Some((role, frag))
}

/// The geometry fragments for `name`, or `None` if there is no such icon (or
/// icons are not compiled in).
pub fn lookup(name: &str) -> Option<impl Iterator<Item = (Role, &'static str)>> {
    let lines = lines();
    let i = lines.binary_search_by(|l| name_of(l).cmp(name)).ok()?;
    let (_, rest) = lines[i].split_once('\t')?;
    Some(rest.split('\t').filter_map(parse_field))
}

/// Every known icon name, sorted — for "did you mean …?" suggestions.
pub fn names() -> impl Iterator<Item = &'static str> {
    lines().iter().copied().map(name_of)
}

#[cfg(all(test, feature = "icons"))]
mod tests {
    use super::*;

    #[test]
    fn known_icons_resolve() {
        // heart: one Both path.
        let heart: Vec<_> = lookup("heart").unwrap().collect();
        assert_eq!(heart.len(), 1);
        assert_eq!(heart[0].0, Role::Both);
        assert!(heart[0].1.starts_with("<path"));
        // atom: has a Solid nucleus.
        assert!(lookup("atom").unwrap().any(|(r, _)| r == Role::Solid));
        // user: a Both head + a Line shoulders.
        let user: Vec<_> = lookup("user").unwrap().map(|(r, _)| r).collect();
        assert!(user.contains(&Role::Both) && user.contains(&Role::Line));
    }

    #[test]
    fn unknown_icon_is_none() {
        assert!(lookup("definitely-not-an-icon").is_none());
    }

    #[test]
    fn names_sorted_and_complete() {
        let all: Vec<_> = names().collect();
        assert_eq!(all.len(), 1512);
        assert!(all.windows(2).all(|w| w[0] < w[1]));
    }
}
```

- [ ] **Step 2:** add `mod icon;` to `src/lib.rs` (with the other `mod` declarations; keep private — `render`/`resolve` use `crate::icon`).
- [ ] **Step 3:** `cargo test icon::` → PASS; `cargo test --no-default-features` still compiles (cfg'd tests drop out).
- [ ] **Step 4: Commit** (`-F` file): "feat(icons): embedded icon table with lazy binary-search lookup".

---

## Phase 3 — `symbol` property, defaults, validation ✅ DONE

**Goal:** `|icon| { symbol: heart }` resolves; label freed for text; missing/unknown symbols error well; defaults give a grey-bodied duotone at size 32.

**Files:** `src/resolve/defaults.rs`, `src/desugar/bundles.rs`, `src/resolve/scene.rs`, tests.

**Consumes:** `crate::icon::{lookup, names, ENABLED}`.

- [ ] **Step 1:** add `--icon-fill` in `defaults.rs` after the `group-fill` block — a *visible* soft grey (tune in Phase 4):

```rust
    set_visual(
        &mut t,
        "icon-fill",
        light_dark(rgba(0.0, 0.0, 0.0, 0.16), rgba(255.0, 255.0, 255.0, 0.18)),
    );
```

- [ ] **Step 2:** replace the `Icon` arm in `desugar/bundles.rs` (was `vec![var("fill","stroke"), n("width",24.0), n("height",24.0)]`):

```rust
        Icon => vec![
            var("fill", "icon-fill"),
            var("stroke", "stroke"),
            n("stroke-width", 2.0),
            n("width", 32.0),
            n("height", 32.0),
        ],
```

Update the `slant_carries_skew_icon_carries_size` test (expect width/height `32.0`).

- [ ] **Step 3:** in `resolve/scene.rs`, the icon branch (~192-218): drop the label-as-glyph logic and the `if !is_icon` child-skip — read the `symbol` attr instead, let children flow (so a bare `"3"` stays a centred text node). Validate:

```rust
    if shape == ShapeKind::Icon {
        match attrs.get("symbol").and_then(value_ident_or_str) {
            None => return Err(Error::at(node.span, "'|icon|' needs a 'symbol' (e.g. { symbol: heart })")),
            Some(sym) if crate::icon::lookup(sym).is_none() => {
                return Err(Error::at(node.span, unknown_icon_msg(sym)));
            }
            Some(_) => {}
        }
    }
```

with helpers (std-only; check `src/lint.rs` first for an existing edit-distance/"did you mean" util to reuse rather than duplicate the planned hint table):

```rust
fn value_ident_or_str(v: &ResolvedValue) -> Option<&str> {
    match v {
        ResolvedValue::Ident(s) | ResolvedValue::String(s) => Some(s),
        _ => None,
    }
}

fn unknown_icon_msg(sym: &str) -> String {
    if !crate::icon::ENABLED {
        return "icons are not built in — rebuild with the `icons` feature".into();
    }
    match crate::icon::names().min_by_key(|c| edit_distance(c, sym)).filter(|c| edit_distance(c, sym) <= 2) {
        Some(near) => format!("unknown icon '{sym}' — did you mean '{near}'?"),
        None => format!("unknown icon '{sym}'"),
    }
}
```

- [ ] **Step 4: Tests** (`tests/resolution.rs`): `{ symbol: heart }` → `attrs["symbol"] == "heart"`; no symbol → error; `{ symbol: nope }` → error w/ suggestion; `{ symbol: heart } "3"` keeps a `"3"` text child.
- [ ] **Step 5:** `cargo test` + `cargo clippy` clean.
- [ ] **Step 6: Commit** (`-F`): "feat(icons): symbol property, --icon-fill default, name validation".

---

## Phase 4 — Render the real icon ✅ DONE

**Goal:** replace the placeholder square with scaled Phosphor geometry — role-grouped paint, counter-scaled baked `stroke-width`, round caps, optional centred text. Inlining = tree-shaking.

**Files:** `src/render/primitives.rs` (`emit_icon`); check `src/render/rules.rs` / `node_style_attr` so the node `<g>` paint doesn't fight the element-level groups (exclude `ShapeKind::Icon` from the paint diff if needed — icon paint is element-level).

- [ ] **Step 1: rewrite `emit_icon`.** Scale the 256-grid into the box; paint role-grouped fragments (verbatim — they are markup, not text, so do **not** `escape_xml` them):

```rust
fn emit_icon(out: &mut String, n: &PlacedNode, indent: &str, vars: &VarTable, opts: &Options) {
    let Some(name) = n.attrs.get("symbol").and_then(|v| match v {
        ResolvedValue::Ident(s) | ResolvedValue::String(s) => Some(s.as_str()),
        _ => None,
    }) else {
        return; // resolve already rejected a missing/unknown symbol
    };
    let Some(frags) = crate::icon::lookup(name) else { return };
    let frags: Vec<(crate::icon::Role, &str)> = frags.collect();

    let size = n.bbox.w().min(n.bbox.h());
    let s = size / 256.0;
    let body = attr_or_var(&n.attrs, "fill", "icon-fill", vars, opts);
    let ink = attr_or_var(&n.attrs, "stroke", "stroke", vars, opts);
    let sw = n.attrs.number("stroke-width").unwrap_or(2.0) / s;

    use crate::icon::Role::*;
    writeln!(out, r#"{indent}<g transform="scale({}) translate(-128 -128)">"#, num(s)).unwrap();
    // Body (faint) behind, then outline, then solid ink on top.
    if body != "none" {
        emit_role_group(out, indent, &frags, |r| matches!(r, Fill | Both),
            &format!(r#"fill="{body}" stroke="none""#));
    }
    emit_role_group(out, indent, &frags, |r| matches!(r, Line | Both),
        &format!(r#"fill="none" stroke="{ink}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round""#, num(sw)));
    emit_role_group(out, indent, &frags, |r| matches!(r, Solid),
        &format!(r#"fill="{ink}" stroke="none""#));
    writeln!(out, "{indent}</g>").unwrap();

    // Optional label over the glyph (SPEC §7).
    if let Some(label) = n.label.as_deref().filter(|s| !s.is_empty()) {
        let color = attr_or_var(&n.attrs, "color", "text-color", vars, opts);
        writeln!(out,
            r#"{indent}<text x="0" y="0" text-anchor="middle" dominant-baseline="central" font-size="{}" fill="{}">{}</text>"#,
            num(size * 0.4), color, escape_xml(label)).unwrap();
    }
}

/// Emit the fragments matching `want` inside one paint `<g>` (skips if none).
fn emit_role_group(out: &mut String, indent: &str, frags: &[(crate::icon::Role, &str)],
                   want: impl Fn(crate::icon::Role) -> bool, paint: &str) {
    if !frags.iter().any(|&(r, _)| want(r)) { return; }
    writeln!(out, "{indent}  <g {paint}>").unwrap();
    for &(_, frag) in frags.iter().filter(|&&(r, _)| want(r)) {
        writeln!(out, "{indent}    {frag}").unwrap();
    }
    writeln!(out, "{indent}  </g>").unwrap();
}
```

- [ ] **Step 2: rendering snapshot** (`tests/rendering.rs`): `x |icon| { symbol: heart }` → contains `<g transform="scale(`, a `<path d="M`, `stroke-width="16"` (32px → 2×256/32), round caps, no placeholder `<rect … fill="none"`. Single-tone (`fill: none`) → no body group. Two-tone (`fill: --teal-wash; stroke: --teal-ink`) → both colours present. `atom` → a solid group emitted.
- [ ] **Step 3:** `cargo test`; `cargo insta review` (eyeball, then accept).
- [ ] **Step 4: PNG visual check** (do it yourself): render a scratch/`samples/icons.lini`, `resvg … out.png`, read it. Confirm line weight matches box borders, the grey body reads as duotone, `user` shows a faint head + line shoulders, `atom` shows a solid nucleus. Tune `--icon-fill` if needed; re-snapshot.
- [ ] **Step 5: Commit** (`-F`): "feat(icons): render Phosphor duotone geometry with counter-scaled stroke".

---

## Phase 5 — Sample, snapshots, docs ✅ DONE

**Files:** `samples/icons.lini`; `tests/conformance.rs` snapshots; `README.md`; `TODO.md`.

- [ ] **Step 1:** `samples/icons.lini` — default duotone, single-tone (`fill: none`), hued two-tone, a `::`-derived reusable icon (`|warn::icon| { symbol: warning-circle; stroke: --amber-ink }`), an icon with a text label, and two icons wired together.
- [ ] **Step 2:** `cargo test` regenerates the conformance snapshot; `cargo insta review` after eyeballing.
- [ ] **Step 3:** final PNG visual check of the sample.
- [ ] **Step 4:** README — fix the shapes paragraph ("icon (Material Symbols)") to the Phosphor `symbol:` model.
- [ ] **Step 5:** TODO.md — the "Icons — duotone" entry is built; remove/condense it.
- [ ] **Step 6:** `cargo fmt --all && cargo test && cargo clippy` green.
- [ ] **Step 7: Commit** (`-F`): "docs(icons): sample, conformance snapshot, README/TODO cleanup".

---

## How to resume
Open this file, find the first unchecked `- [ ]`, continue. Phases end green + committed; `git log --oneline` shows landed stages.

## Self-review
- **Spec coverage:** symbol (§7/§10)→P3; duotone fill/stroke + counter-scaled width (§7)→P3–4; size 32 (§6/§11.5)→P3; `--icon-fill` (§10/§11.1)→P3; tree-shake by inlining (§7/§13)→P4; `icons` feature + geometry-only data (§7/§19)→P1–2; `icon-variant` dropped (§10)→gone, verify no code path in P3. ✓
- **Type consistency:** `Role {Fill,Line,Solid,Both}`, `lookup → Option<impl Iterator<Item=(Role,&'static str)>>`, `names`, `ENABLED` used identically P2–4. ✓

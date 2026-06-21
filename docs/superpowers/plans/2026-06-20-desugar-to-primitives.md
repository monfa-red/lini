# Desugar to Primitives Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make desugaring a real pipeline stage (`parse → desugar → resolve`) that lowers every template, define, element rule, and type-selector into a uniform `.lini-*` class namespace, leaving the core to see only primitive shapes.

**Architecture:** A new `src/desugar/` module rewrites the AST so each typed instance becomes a `|primitive|` wearing a `.lini-*` class chain, with every built-in default materialized into generated `.lini-*` class defs, the global block, or the `-> { }` wire defaults. `resolve` loses its type system entirely and becomes a class-only cascade. `render` renames `lini-shape-*` → `lini-*` and reads each node's classes directly. The "dumb core" carries no baked geometry/layout fallback.

**Tech Stack:** Rust 2024, `insta` snapshot tests, `resvg` for visual PNG checks.

## Global Constraints

- **No `unsafe`.** Find another path or surface the question.
- **One concept per file**; split a module past ~500 LOC.
- **Never include "Co-Authored-By" lines** in commits.
- **Run `cargo fmt` before any push**; CI runs `cargo fmt --all -- --check`, `cargo test`, `cargo clippy`.
- **`insta` snapshot tests** for output-shaped code; **one sample per feature** in `samples/`.
- **Verify SVG visually** — render to PNG with `resvg` and read it.
- **Defer pushing to `main` to the user.** Local commits are fine.
- **Locked decision — dumb core:** the post-desugar engine carries no baked geometry/layout default; every such constant is materialized by desugar; the `unwrap_or(const)` fallbacks are removed. Identity/no-op defaults (`letter-spacing: 0`, `translate: 0 0`, `rotate: 0`, `opacity: 1`, `pin: none`, `divider: none`, `stack`/`shadow` off) stay implicit.
- **Locked decision — `.lini-*` scheme:** generated type classes are `.lini-box`/`.lini-group`/`.lini-<define>` in source, mapping verbatim to the same SVG class (drop the `shape` infix). User classes keep `lini-style-`. `.lini-*` is a reserved class prefix.
- **Invariants (oracle):** `compile(src)` ≡ `compile(desugar(src))`; `desugar(desugar(x))` ≡ `desugar(x)`.
- **Design source of truth:** `docs/superpowers/specs/2026-06-20-desugar-to-primitives-design.md`.

---

## File Structure

**New (`src/desugar/`):**
- `mod.rs` — `pub fn desugar(&File) -> Result<File, Error>`; orchestration; the entry `lib.rs` calls.
- `bundles.rs` — every built-in default as AST `Decl`s (per-primitive constants, template deltas, root/scene defaults, wire defaults).
- `types.rs` — TEMPLATES table + define/template chain walk + cycle/depth/shadow validation (moved from `resolve/types.rs`).
- `classes.rs` — `.lini-*` class-def generation, the worn-class chain, the reserved-prefix split helper.
- `labels.rs` — id-as-label · trailing label · auto-`along:` (ported from the old `src/desugar.rs`).
- `scene.rs` — root/scene config decls; auto-create root boxes.

**Modified:** `src/lib.rs`, `src/resolve/{mod,program,scene,cascade,ir,defaults}.rs`, `src/render/{values,rules,mod}.rs`, `src/layout/{primitives,mod}.rs` + the wire layout fallbacks, `SPEC.md`, `WIRING.md`, the snapshot tests.

**Deleted:** `src/desugar.rs` (→ dir), `src/resolve/types.rs`.

---

## Task 1: Rename SVG classes `lini-shape-*` → `lini-*` and reserve structural names

Pure render-naming change; the old typed pipeline still runs. Isolates the global SVG rename from the architectural work so later tasks cause no further snapshot churn.

**Files:**
- Modify: `src/render/values.rs` (`class_list`, ~line 159-173)
- Modify: `src/render/rules.rs` (every `lini-shape-` literal)
- Modify: `src/render/mod.rs` test assertions referencing `lini-shape-`
- Modify: `src/resolve/types.rs` (`is_builtin_type`, ~line 193) and the `lini-shape-*` doc comment in `src/resolve/ir.rs:21`
- Modify: `SPEC.md`, `tests/cli.rs`, `tests/rendering.rs` (any `lini-shape-` text)
- Test: existing `tests/conformance.rs`, `tests/rendering.rs`, `src/render/rules.rs` unit tests (re-blessed/updated)

**Interfaces:**
- Produces: SVG node classes `lini-node lini-<type…> lini-<primitive> lini-style-<class>`; CSS rules `.lini .lini-<type> { … }`. `is_builtin_type` now also rejects `node`, `text`, `marker`, `canvas`, `scene`, `cut`.

- [ ] **Step 1: Update the reserved-name test (failing test)**

In `src/resolve/types.rs`, under `mod tests`, add:

```rust
#[test]
fn define_shadowing_a_structural_name_errors() {
    for name in ["node", "text", "marker", "canvas", "scene", "cut"] {
        let src = format!("{{ |{name}::box| {{ }} }}\n");
        assert!(
            build_err(&src).contains("shadows a built-in"),
            "'{name}' must be reserved"
        );
    }
}
```

- [ ] **Step 2: Run it, expect failure**

Run: `cargo test -p lini --lib resolve::types::tests::define_shadowing_a_structural_name_errors`
Expected: FAIL (`node` is currently a legal define name).

- [ ] **Step 3: Reserve the structural names**

In `src/resolve/types.rs`, change `is_builtin_type`:

```rust
fn is_builtin_type(name: &str) -> bool {
    ShapeKind::parse(name).is_some()
        || is_template(name)
        || matches!(name, "wire" | "node" | "text" | "marker" | "canvas" | "scene" | "cut")
}
```

- [ ] **Step 4: Drop the `shape` infix in `class_list`**

In `src/render/values.rs`, `class_list`:

```rust
pub fn class_list(
    primitive_kind: &str,
    type_chain: &[String],
    applied_styles: &[String],
) -> Vec<String> {
    let mut classes = vec!["lini-node".to_string()];
    for name in type_chain {
        classes.push(format!("lini-{}", name));
    }
    classes.push(format!("lini-{}", primitive_kind));
    for name in applied_styles {
        classes.push(format!("lini-style-{}", name));
    }
    classes
}
```

- [ ] **Step 5: Drop the `shape` infix in `rules.rs`**

In `src/render/rules.rs`, replace every `format!("lini-shape-{}", …)` and the literals `"lini-shape-line"`, `"lini-shape-icon"`, `"lini-shape-text"` → drop `shape-`. Net: `lini-shape-{x}` → `lini-{x}`. The `lini-text`, `lini-marker*`, `lini-wire*`, `lini-cut*`, `lini-style-*` names are unchanged. Update the `present.contains(...)` keys — they already key on `kind.as_str()` / template name, so only the emitted class strings change.

- [ ] **Step 6: Update unit-test assertions in `render/rules.rs` and `render/mod.rs`**

Replace `lini-shape-box`→`lini-box`, `lini-shape-oval`→`lini-oval`, `lini-shape-group`→`lini-group`, `lini-shape-treat`→`lini-treat`, etc. in the `#[test]` assertions in both files.

- [ ] **Step 7: Update doc/text references**

Run: `grep -rln 'lini-shape-' src SPEC.md tests` — for each non-snapshot hit, replace `lini-shape-`→`lini-` (the `ir.rs:21` doc comment, `SPEC.md` §13 examples, `tests/cli.rs`, `tests/rendering.rs`). Leave the design doc and `.snap` files (snapshots re-bless next).

- [ ] **Step 8: Re-bless snapshots and run the suite**

Run: `cargo test -p lini 2>/dev/null; INSTA_UPDATE=always cargo test` then review:
Run: `git diff --stat tests/snapshots/` — every changed `.snap` should differ only by `lini-shape-X` → `lini-X`. Spot-check two diffs with `git diff tests/snapshots/conformance__*@templates_all.lini.snap`.

- [ ] **Step 9: Run the full suite clean**

Run: `cargo test -p lini && cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "render: drop the lini-shape- infix; reserve structural type names"
```

---

## Task 2: `desugar/bundles.rs` — built-in defaults as AST decls

The single source of every materialized default, expressed as parser-shaped `Decl`s (so they reference `--vars` and resolve like hand-written source). Pure data + lookups; nothing else consumes it yet.

**Files:**
- Create: `src/desugar/mod.rs` (module declarations only, this task)
- Create: `src/desugar/bundles.rs`
- Modify: `src/lib.rs` (`mod desugar;` already exists as a file — see Step 1)
- Test: in-file `#[cfg(test)] mod tests` in `bundles.rs`

**Interfaces:**
- Produces:
  - `pub fn primitive_bundle(kind: ShapeKind) -> Vec<Decl>` — a primitive's full default set.
  - `pub fn template_bundle(name: &str) -> Vec<Decl>` — a template's delta (empty for a non-template).
  - `pub fn root_defaults() -> Vec<Decl>` — scene/root config (`layout: column; padding: 0; gap: 20; font-size: 15; line-height: 1.2; text-align: center; canvas-pad: 20`).
  - `pub fn wire_defaults() -> Vec<Decl>` — `stroke-width: 2; clearance: 16; font-size: 11`.

- [ ] **Step 1: Convert `src/desugar.rs` into a directory module**

```bash
mkdir -p src/desugar
git mv src/desugar.rs src/desugar/labels.rs
```

Create `src/desugar/mod.rs`:

```rust
//! Desugar: lower all surface sugar (types, templates, defines, element/descendant
//! rules, labels, scene defaults) to primitive shapes + `.lini-*` classes, so the
//! core only ever sees primitives (design: docs/superpowers/specs/2026-06-20-desugar-to-primitives-design.md).

mod bundles;

pub use labels::desugar; // temporary re-export; replaced by mod::desugar in Task 7

#[path = "labels.rs"]
mod labels;
```

(`labels.rs` keeps its current `pub fn desugar(&File) -> File` for now; `lib.rs` still references `desugar::desugar` unchanged. Confirm `src/lib.rs:2` `mod desugar;` now resolves to the directory.)

- [ ] **Step 2: Write the failing bundle tests**

In `src/desugar/bundles.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::ShapeKind;

    fn has(decls: &[Decl], name: &str) -> bool {
        decls.iter().any(|d| d.name == name)
    }
    fn num(decls: &[Decl], name: &str) -> Option<f64> {
        decls.iter().find(|d| d.name == name).and_then(|d| match d.groups.first()?.first()? {
            Value::Number(n) => Some(*n),
            _ => None,
        })
    }

    #[test]
    fn box_bundle_carries_its_geometry_and_paint() {
        let b = primitive_bundle(ShapeKind::Box);
        assert_eq!(num(&b, "radius"), Some(6.0));
        assert_eq!(num(&b, "padding"), Some(20.0));
        assert_eq!(num(&b, "gap"), Some(20.0));
        assert_eq!(num(&b, "stroke-width"), Some(2.0));
        assert!(has(&b, "fill") && has(&b, "stroke"));
    }

    #[test]
    fn slant_carries_skew_icon_carries_size() {
        assert_eq!(num(&primitive_bundle(ShapeKind::Slant), "skew"), Some(15.0));
        let icon = primitive_bundle(ShapeKind::Icon);
        assert_eq!(num(&icon, "width"), Some(24.0));
        assert_eq!(num(&icon, "height"), Some(24.0));
    }

    #[test]
    fn group_template_is_a_dashed_frame() {
        let g = template_bundle("group");
        assert!(g.iter().any(|d| d.name == "stroke-style"));
        assert_eq!(num(&g, "stroke-width"), Some(1.0));
        assert!(template_bundle("oval").is_empty());
    }

    #[test]
    fn root_and_wire_defaults_are_present() {
        assert_eq!(num(&root_defaults(), "canvas-pad"), Some(20.0));
        assert_eq!(num(&root_defaults(), "font-size"), Some(15.0));
        assert_eq!(num(&wire_defaults(), "clearance"), Some(16.0));
        assert_eq!(num(&wire_defaults(), "font-size"), Some(11.0));
    }
}
```

- [ ] **Step 3: Run, expect failure**

Run: `cargo test -p lini --lib desugar::bundles`
Expected: FAIL (`primitive_bundle` not defined).

- [ ] **Step 4: Implement `bundles.rs`**

```rust
//! Every built-in default, expressed as parser-shaped `Decl`s. This is the one
//! place Lini's look is tuned; desugar lowers these into `.lini-*` class defs,
//! the global block, and the `-> { }` wire defaults. Visual `--lini-*` colours
//! stay live `--var` references (render emits their defaults as `@layer` CSS).

use crate::resolve::ShapeKind;
use crate::span::Span;
use crate::syntax::ast::{Decl, Value};

fn decl(name: &str, values: Vec<Value>) -> Decl {
    Decl { name: name.into(), groups: vec![values], span: Span::empty() }
}
fn n(name: &str, v: f64) -> Decl { decl(name, vec![Value::Number(v)]) }
fn id(name: &str, v: &str) -> Decl { decl(name, vec![Value::Ident(v.into())]) }
fn var(name: &str, v: &str) -> Decl { decl(name, vec![Value::Var(v.into())]) }
fn pair(name: &str, a: f64, b: f64) -> Decl {
    decl(name, vec![Value::Number(a), Value::Number(b)])
}

/// A primitive's complete default set (paint + geometry).
pub fn primitive_bundle(kind: ShapeKind) -> Vec<Decl> {
    use ShapeKind::*;
    // Closed, content-sized shapes share paint + box-model defaults.
    let sized = || vec![
        var("fill", "fill"),
        var("stroke", "stroke"),
        n("stroke-width", 2.0),
        n("padding", 20.0),
        n("gap", 20.0),
    ];
    match kind {
        Box => { let mut b = sized(); b.push(n("radius", 6.0)); b }
        Oval | Hex | Cyl | Diamond | Cloud => sized(),
        Slant => { let mut b = sized(); b.push(n("skew", 15.0)); b }
        // Geometry-sized closed shapes: paint only, no box model.
        Poly | Path => vec![var("fill", "fill"), var("stroke", "stroke"), n("stroke-width", 2.0)],
        Line => vec![id("fill", "none"), var("stroke", "stroke"), n("stroke-width", 2.0)],
        Icon => vec![var("fill", "stroke"), n("width", 24.0), n("height", 24.0)],
        // Text is structural (render's `lini-text` rule); Image requires src/dims.
        Text | Image => Vec::new(),
    }
}

/// A built-in template's delta over its base (SPEC §8). Empty for a non-template.
pub fn template_bundle(name: &str) -> Vec<Decl> {
    match name {
        "plain" => vec![id("stroke", "none"), id("fill", "none"), n("padding", 0.0)],
        "rect" => vec![n("radius", 0.0)],
        "group" => vec![
            var("stroke", "group-stroke"),
            id("stroke-style", "dashed"),
            n("stroke-width", 1.0),
            var("fill", "group-fill"),
            n("radius", 6.0),
        ],
        "caption" => vec![
            decl("pin", vec![Value::Ident("top".into()), Value::Ident("left".into())]),
            pair("translate", 0.0, -18.0),
            var("color", "caption-color"),
            n("font-size", 12.0),
            var("font-weight", "caption-font-weight"),
        ],
        "footer" => vec![
            id("pin", "bottom"),
            pair("translate", 0.0, 17.0),
            n("font-size", 11.0),
            var("color", "footer-color"),
        ],
        "badge" => vec![
            decl("pin", vec![Value::Ident("top".into()), Value::Ident("right".into())]),
            pair("translate", 6.0, -6.0),
            n("radius", 8.0),
            pair("padding", 2.0, 6.0),
            decl("shadow", vec![Value::Number(2.0), Value::Number(3.0), Value::Number(3.0)]),
            id("stroke", "none"),
            var("fill", "accent"),
            var("color", "on-accent"),
            n("font-size", 11.0),
            id("font-weight", "normal"),
        ],
        "note" => vec![
            n("radius", 2.0),
            n("shadow", 2.0),
            id("stroke", "none"),
            var("fill", "note-bg"),
        ],
        "row" => vec![id("layout", "row")],
        "column" => vec![id("layout", "column")],
        "table" => vec![
            id("layout", "grid"),
            id("divider", "all"),
            n("gap", 0.0),
            pair("padding", 4.0, 8.0),
            id("fill", "none"),
            var("stroke", "stroke"),
            id("stroke-style", "solid"),
            n("font-size", 14.0),
            id("font-weight", "normal"),
        ],
        _ => Vec::new(),
    }
}

/// Scene/root config defaults — prepended to the global block (user decls override).
pub fn root_defaults() -> Vec<Decl> {
    vec![
        id("layout", "column"),
        n("padding", 0.0),
        n("gap", 20.0),
        n("font-size", 15.0),
        n("line-height", 1.2),
        id("text-align", "center"),
        n("canvas-pad", 20.0),
    ]
}

/// Wire defaults — prepended to the `-> { }` rule (user decls override).
pub fn wire_defaults() -> Vec<Decl> {
    vec![n("stroke-width", 2.0), n("clearance", 16.0), n("font-size", 11.0)]
}
```

Add `mod bundles;` is already in `mod.rs` Step 1. Ensure `Span::empty()` exists (it is used across the codebase — see `src/span.rs`).

- [ ] **Step 5: Run the tests, expect pass**

Run: `cargo test -p lini --lib desugar::bundles`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "desugar: bundles — every built-in default as AST decls"
```

---

## Task 3: `desugar/types.rs` — template table + chain walk

Resolve a type name to its base→derived chain (primitive excluded), validating cycles, depth, and built-in shadowing — at the AST level, before resolve.

**Files:**
- Create: `src/desugar/types.rs`
- Modify: `src/desugar/mod.rs` (`mod types;`)
- Test: in-file tests

**Interfaces:**
- Consumes: `crate::syntax::ast::{File, Define, StyleItem}`, `bundles::template_bundle`.
- Produces:
  - `pub struct TypeInfo { pub kind: ShapeKind, pub chain: Vec<String> }` — `chain` is type names base→derived (templates + defines), primitive in `kind`.
  - `pub struct Types<'a>` with `pub fn build(file: &'a File) -> Result<Self, Error>` and `pub fn resolve(&self, name: &str, span: Span) -> Result<TypeInfo, Error>` and `pub fn is_known(&self, name: &str) -> bool`.
  - `pub fn is_template(name: &str) -> bool`, `pub fn template_base(name: &str) -> Option<&'static str>`, `pub const TEMPLATES: &[(&str, &str)]` (moved verbatim from `resolve/types.rs`).
  - `pub fn each_define(file: &File) -> impl Iterator<Item = &Define>` helper (or inline).

- [ ] **Step 1: Write the failing tests**

In `src/desugar/types.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn types_of(src: &str) -> (crate::syntax::ast::File, ()) {
        let toks = crate::lexer::lex(src).expect("lex");
        (crate::syntax::parser::parse(&toks).expect("parse"), ())
    }
    fn chain(src: &str, name: &str) -> Vec<String> {
        let (file, _) = types_of(src);
        let t = Types::build(&file).expect("build");
        t.resolve(name, Span::empty()).expect("resolve").chain
    }
    fn build_err(src: &str) -> String {
        let (file, _) = types_of(src);
        match Types::build(&file).and_then(|t| t.resolve("box", Span::empty())) {
            Err(e) => e.message,
            Ok(_) => Types::build(&file).err().map(|e| e.message).unwrap_or_default(),
        }
    }

    #[test]
    fn primitive_has_empty_chain() {
        assert!(chain("", "box").is_empty());
    }
    #[test]
    fn table_chain_is_group_then_table() {
        assert_eq!(chain("", "table"), vec!["group", "table"]);
    }
    #[test]
    fn user_define_appends_after_its_base_chain() {
        assert_eq!(chain("{ |panel::group| { } }\n", "panel"), vec!["group", "panel"]);
    }
    #[test]
    fn cycle_and_depth_and_shadow_error() {
        assert!(build_err("{ |a::b| { }\n|b::a| { } }\n").contains("cycle"));
        assert!(build_err("{ |rect::oval| { } }\n").contains("shadows a built-in"));
    }
}
```

- [ ] **Step 2: Run, expect failure**

Run: `cargo test -p lini --lib desugar::types`
Expected: FAIL (`Types` undefined).

- [ ] **Step 3: Implement `types.rs`**

Port `TEMPLATES`, `is_template`, `template_base`, the `MAX_INHERITANCE_DEPTH` constant, `is_builtin_type` (including the structural-name reservation from Task 1), and the walk from `resolve/types.rs` — but return **chains of names**, not resolved values. Key differences from the old module:

```rust
//! Template table + define/template chain resolution at the AST level. Returns
//! base→derived name chains (primitive excluded); desugar turns each name into a
//! `.lini-<name>` class. Cycles, depth > 16, and shadowing a built-in are errors.

use crate::error::Error;
use crate::resolve::ShapeKind;
use crate::span::Span;
use crate::syntax::ast::{Define, File, StyleItem};
use std::collections::HashMap;

const MAX_INHERITANCE_DEPTH: usize = 16;

pub const TEMPLATES: &[(&str, &str)] = &[
    ("plain", "box"), ("rect", "box"), ("group", "box"), ("caption", "plain"),
    ("footer", "caption"), ("badge", "box"), ("note", "box"), ("row", "plain"),
    ("column", "plain"), ("table", "group"),
];

pub fn is_template(name: &str) -> bool { TEMPLATES.iter().any(|(n, _)| *n == name) }
pub fn template_base(name: &str) -> Option<&'static str> {
    TEMPLATES.iter().find(|(n, _)| *n == name).map(|(_, b)| *b)
}
fn is_builtin_type(name: &str) -> bool {
    ShapeKind::parse(name).is_some()
        || is_template(name)
        || matches!(name, "wire" | "node" | "text" | "marker" | "canvas" | "scene" | "cut")
}

pub struct TypeInfo { pub kind: ShapeKind, pub chain: Vec<String> }

pub struct Types<'a> { user: HashMap<String, (&'a Define, String)> } // name -> (define, base)

impl<'a> Types<'a> {
    pub fn build(file: &'a File) -> Result<Self, Error> {
        let mut user = HashMap::new();
        for d in file.stylesheet.iter().filter_map(as_define) {
            if is_builtin_type(&d.name) {
                return Err(Error::at(d.span, format!("'{}' shadows a built-in type", d.name)));
            }
            if user.insert(d.name.clone(), (d, d.base.clone())).is_some() {
                return Err(Error::at(d.span, format!("duplicate type '{}'", d.name)));
            }
        }
        let types = Self { user };
        for d in file.stylesheet.iter().filter_map(as_define) {
            types.walk(&d.name, d.span, &mut Vec::new(), 0)?; // validate every define up front
        }
        Ok(types)
    }

    pub fn is_known(&self, name: &str) -> bool {
        ShapeKind::parse(name).is_some() || is_template(name) || self.user.contains_key(name)
    }

    pub fn resolve(&self, name: &str, span: Span) -> Result<TypeInfo, Error> {
        self.walk(name, span, &mut Vec::new(), 0)
    }

    fn walk(&self, name: &str, span: Span, visiting: &mut Vec<String>, depth: usize)
        -> Result<TypeInfo, Error>
    {
        if depth > MAX_INHERITANCE_DEPTH {
            return Err(Error::at(span, format!("'{}' exceeds max inheritance depth (16)", name)));
        }
        if visiting.iter().any(|n| n == name) {
            return Err(Error::at(span, format!("cycle in '{} -> {}'", visiting.join(" -> "), name)));
        }
        if let Some(kind) = ShapeKind::parse(name) {
            return Ok(TypeInfo { kind, chain: Vec::new() });
        }
        let base = template_base(name)
            .map(str::to_string)
            .or_else(|| self.user.get(name).map(|(_, b)| b.clone()))
            .ok_or_else(|| Error::at(span, format!("unknown type '{}'", name)))?;
        visiting.push(name.to_string());
        let mut info = self.walk(&base, span, visiting, depth + 1)?;
        visiting.pop();
        info.chain.push(name.to_string()); // base→derived
        Ok(info)
    }
}

fn as_define(it: &StyleItem) -> Option<&Define> {
    match it { StyleItem::Define(d) => Some(d), _ => None }
}
```

Add `mod types;` to `mod.rs`.

- [ ] **Step 4: Run tests, expect pass**

Run: `cargo test -p lini --lib desugar::types`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "desugar: types — AST-level template/define chain walk"
```

---

## Task 4: `desugar/classes.rs` — `.lini-*` generation + reserved split

Turn a type chain into worn `.lini-*` classes, generate their stylesheet class defs (merging element-rule decls), and split a worn-class list into the `.lini-*` (type) and user (style) halves.

**Files:**
- Create: `src/desugar/classes.rs`
- Modify: `src/desugar/mod.rs` (`mod classes;`)
- Test: in-file tests

**Interfaces:**
- Consumes: `types::{TypeInfo, TEMPLATES}`, `bundles::{primitive_bundle, template_bundle}`, `ast::{Decl, Selector, SelPart}`.
- Produces:
  - `pub fn lini_class(name: &str) -> String` → `format!("lini-{name}")`.
  - `pub fn is_lini_class(name: &str) -> bool` → `name.starts_with("lini-") && !name.starts_with("lini-style-")` (the reserved-prefix test; user classes are stored bare so this is really `name.starts_with("lini-")`).
  - `pub fn worn_classes(info: &TypeInfo) -> Vec<String>` → `["lini-<primitive>", "lini-<chain…>"]` (primitive first, then base→derived).
  - `pub fn class_defs(present: &BTreeSet<String>, element_rules: &HashMap<String, Vec<Decl>>) -> Vec<Rule>` → one `.lini-<name> { bundle + element-rule decls }` per present type name, ordered primitives → templates → defines.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::ShapeKind;
    use crate::desugar::types::TypeInfo;

    #[test]
    fn worn_chain_is_primitive_then_base_to_derived() {
        let info = TypeInfo { kind: ShapeKind::Box, chain: vec!["group".into(), "table".into()] };
        assert_eq!(worn_classes(&info), vec!["lini-box", "lini-group", "lini-table"]);
    }
    #[test]
    fn reserved_prefix_test() {
        assert!(is_lini_class("lini-box"));
        assert!(!is_lini_class("hot"));
    }
    #[test]
    fn class_def_merges_bundle_and_element_rule() {
        use std::collections::{BTreeSet, HashMap};
        let mut present = BTreeSet::new();
        present.insert("box".to_string());
        let mut el = HashMap::new();
        el.insert("box".to_string(), vec![crate::desugar::bundles_test_decl("radius", 4.0)]);
        let defs = class_defs(&present, &el);
        let boxdef = defs.iter().find(|r| sel_name(&r.selector) == "lini-box").unwrap();
        // element-rule radius:4 overrides bundle radius:6 (appended last)
        let last_radius = boxdef.decls.iter().rev().find(|d| d.name == "radius").unwrap();
        assert!(matches!(last_radius.groups[0][0], crate::syntax::ast::Value::Number(n) if n == 4.0));
    }
}
```

(Provide a tiny `pub(crate) fn bundles_test_decl` in `bundles.rs` returning `Decl{name,groups:vec![vec![Value::Number]],span}` for the test, or inline-construct the `Decl`. `sel_name` is a local helper extracting the class from a single-class `Selector`.)

- [ ] **Step 2: Run, expect failure**

Run: `cargo test -p lini --lib desugar::classes`
Expected: FAIL.

- [ ] **Step 3: Implement `classes.rs`**

```rust
//! `.lini-*` class generation and the reserved-prefix split. A type chain becomes
//! worn classes (primitive first, base→derived after); each present type name gets
//! one `.lini-<name> { bundle + element-rule decls }` stylesheet rule.

use super::bundles::{primitive_bundle, template_bundle};
use super::types::{TypeInfo, is_template};
use crate::resolve::ShapeKind;
use crate::span::Span;
use crate::syntax::ast::{Decl, Rule, SelPart, Selector};
use std::collections::{BTreeSet, HashMap};

pub fn lini_class(name: &str) -> String { format!("lini-{name}") }
pub fn is_lini_class(name: &str) -> bool { name.starts_with("lini-") }

/// Worn classes for a typed instance: the primitive, then each template/define
/// base→derived. Render maps these verbatim; resolve applies them at the type tier.
pub fn worn_classes(info: &TypeInfo) -> Vec<String> {
    let mut out = vec![lini_class(info.kind.as_str())];
    out.extend(info.chain.iter().map(|n| lini_class(n)));
    out
}

/// One class def per present type name, ordered primitives → templates → defines,
/// each = its bundle then its element-rule decls (so element rules override the
/// bundle in the cascade fold). `present` holds bare type names (e.g. "box","group").
pub fn class_defs(
    present: &BTreeSet<String>,
    element_rules: &HashMap<String, Vec<Decl>>,
    define_order: &[String],
) -> Vec<Rule> {
    let mut rules = Vec::new();
    let mut emit = |name: &str, base: Vec<Decl>| {
        if !present.contains(name) { return; }
        let mut decls = base;
        if let Some(extra) = element_rules.get(name) { decls.extend(extra.iter().cloned()); }
        rules.push(class_rule(name, decls));
    };
    for kind in ShapeKind::ALL { emit(kind.as_str(), primitive_bundle(*kind)); }
    for (name, _) in super::types::TEMPLATES { emit(name, template_bundle(name)); }
    for name in define_order { emit(name, Vec::new()); } // define decls flow via element_rules
    rules
}

fn class_rule(name: &str, decls: Vec<Decl>) -> Rule {
    Rule {
        selector: Selector { parts: vec![SelPart::Class(lini_class(name))] },
        decls,
        span: Span::empty(),
    }
}
```

Add `ShapeKind::ALL: [ShapeKind; 13]` to `resolve/ir.rs` (the 13 variants in `as_str` order) if it does not exist. Add `mod classes;` to `mod.rs`.

- [ ] **Step 4: Run tests, expect pass**

Run: `cargo test -p lini --lib desugar::classes`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "desugar: classes — .lini-* generation and the reserved-prefix split"
```

---

## Task 5: `desugar/scene.rs` — root config + auto-create

Build the global-block default decls and the auto-created root boxes (moved from `resolve/program.rs::auto_created`).

**Files:**
- Create: `src/desugar/scene.rs`
- Modify: `src/desugar/mod.rs` (`mod scene;`)
- Test: in-file tests

**Interfaces:**
- Consumes: `ast::{File, Node, Wire}`, `bundles::{root_defaults, wire_defaults}`.
- Produces:
  - `pub fn auto_created_ids(file: &File) -> Vec<(String, Span)>` — single-segment root-wire endpoints absent from every declared id (templates/defines already inlined by the caller's prior passes — see Task 7 ordering note).
  - `pub fn declared_ids(file: &File) -> std::collections::HashSet<String>` — every final id segment present anywhere in `instances` (for the auto-create gate).

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    fn parse(src: &str) -> crate::syntax::ast::File {
        crate::syntax::parser::parse(&crate::lexer::lex(src).unwrap()).unwrap()
    }
    #[test]
    fn undeclared_root_wire_ids_are_auto_created() {
        let f = parse("cat -> dog\n");
        let ids: Vec<String> = auto_created_ids(&f).into_iter().map(|(s, _)| s).collect();
        assert_eq!(ids, vec!["cat", "dog"]);
    }
    #[test]
    fn a_declared_id_is_not_auto_created() {
        let f = parse("cat |box|\ncat -> dog\n");
        let ids: Vec<String> = auto_created_ids(&f).into_iter().map(|(s, _)| s).collect();
        assert_eq!(ids, vec!["dog"]);
    }
}
```

- [ ] **Step 2: Run, expect failure**

Run: `cargo test -p lini --lib desugar::scene`
Expected: FAIL.

- [ ] **Step 3: Implement `scene.rs`**

Port `auto_created`/`auto_box`'s id-gathering logic from `resolve/program.rs` (the `has_final_segment` gate becomes a `HashSet` of declared final segments walked over `file.instances`). Return ids + spans; the caller (mod.rs) builds the actual `Node`s so they flow through the same lowering. Provide `declared_ids` by recursively walking `Child::Box` ids in `file.instances`.

- [ ] **Step 4: Run tests, expect pass**

Run: `cargo test -p lini --lib desugar::scene`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "desugar: scene — root defaults inputs and auto-create id gathering"
```

---

## Task 6: `desugar/labels.rs` — keep label/along lowering, drop the old type logic

The ported `labels.rs` still references `resolve::type_chain_contains`. Make it self-contained (icon/group classification via `desugar::types`) and keep its public surface internal — `mod.rs` will own the public `desugar`.

**Files:**
- Modify: `src/desugar/labels.rs`
- Modify: `src/desugar/mod.rs`
- Test: existing logic exercised via Task 7's `tests/desugar.rs`

**Interfaces:**
- Produces:
  - `pub(super) fn label_child_for(node: &Node, is_icon: bool, is_container: bool) -> Option<Child>` — the id-as-label text child (or None).
  - `pub(super) fn auto_along(w: &Wire) -> Wire` — the existing `desugar_wire`, renamed.
- Removes the old `pub fn desugar(&File)` (mod.rs replaces it in Task 7) and the `type_chain_contains` dependency.

- [ ] **Step 1: Refactor `labels.rs` to helpers**

Extract the id-as-label decision (current `desugar_node` body) into `label_child_for(node, is_icon, is_container)`, and rename `desugar_wire` → `auto_along`. Delete the old top-level `desugar`/`desugar_child`/`desugar_node` (mod.rs owns traversal now). Remove `use crate::resolve::type_chain_contains;`.

- [ ] **Step 2: Provide icon/container classification in `types.rs`**

Add to `desugar/types.rs`:

```rust
impl<'a> Types<'a> {
    /// Whether `name` resolves through `target` (e.g. "icon" / "group") — for the
    /// label rules (an icon consumes its text; a group holds children).
    pub fn resolves_through(&self, name: &str, target: &str) -> bool {
        match self.resolve(name, Span::empty()) {
            Ok(info) => info.kind.as_str() == target || info.chain.iter().any(|n| n == target),
            Err(_) => false,
        }
    }
}
```

- [ ] **Step 3: Build the crate**

Run: `cargo build -p lini`
Expected: compiles (labels.rs no longer has a `desugar` fn — `mod.rs`'s temporary `pub use labels::desugar` from Task 2 must be removed; add a placeholder `pub fn desugar(f: &File) -> Result<File, Error> { Ok(f.clone()) }` in `mod.rs` so `lib.rs` still links — Task 7 fills it in).

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "desugar: labels — extract id-as-label and auto-along helpers"
```

---

## Task 7: `desugar/mod.rs` — full lowering; repoint `lini desugar`

Assemble the complete AST→AST lowering and route `lini desugar` through it. The compile pipeline still uses the old resolve (Task 8 flips it), so this task is validated purely by desugar-output tests + idempotency.

**Files:**
- Modify: `src/desugar/mod.rs`
- Modify: `src/lib.rs` (`desugar_source` uses the new `desugar`)
- Modify/Replace: `tests/desugar.rs`
- Test: `tests/desugar.rs`, in-file `mod.rs` tests

**Interfaces:**
- Produces: `pub fn desugar(file: &File) -> Result<File, Error>` — the lowered file: every instance is a primitive wearing `.lini-*` classes with an explicit `[ "label" ]`; define bodies inlined; descendant/element rules rewritten to `.lini-*`; the global block carries root defaults + the `-> { }` wire defaults + generated `.lini-*` class defs; auto-created boxes appended; wire labels carry explicit `along:`.

- [ ] **Step 1: Write the failing desugar tests**

Replace `tests/desugar.rs` with lowered-form expectations:

```rust
//! `lini desugar` lowers ALL sugar to primitives + `.lini-*` classes.

use lini::desugar_source;

#[test]
fn a_plain_box_wears_its_lini_class_and_explicit_label() {
    let out = desugar_source("cat |box|\n").unwrap();
    assert!(out.contains("cat |box| .lini-box [ \"cat\" ]"), "{out}");
    assert!(out.contains(".lini-box {"), "the box bundle is a generated class: {out}");
}

#[test]
fn a_group_lowers_to_box_plus_chain_and_a_generated_class() {
    let out = desugar_source("g |group| [\n  a |box|\n]\n").unwrap();
    assert!(out.contains("|box| .lini-box.lini-group"), "{out}");
    assert!(out.contains(".lini-group {") && out.contains("stroke-style: dashed;"), "{out}");
}

#[test]
fn element_rule_merges_into_the_generated_class() {
    let out = desugar_source("{ |box| { radius: 4; } }\nx |box|\n").unwrap();
    // .lini-box ends with the element-rule radius:4 (the fold's last radius wins).
    let lini_box = out.split(".lini-box {").nth(1).unwrap();
    assert!(lini_box.contains("radius: 4;"), "{out}");
}

#[test]
fn descendant_rule_rewrites_types_to_lini_classes() {
    let out = desugar_source("{ |table box| { fill: gray; } }\nt |table| [ \"a\" ]\n").unwrap();
    assert!(out.contains("|.lini-table .lini-box|"), "{out}");
}

#[test]
fn define_body_inlines_per_instance() {
    let src = "{ |room::group| [\n  inlet |box|\n] }\nr |room|\n";
    let out = desugar_source(src).unwrap();
    assert!(out.contains(".lini-room {"), "{out}");
    assert!(out.contains("inlet |box| .lini-box [ \"inlet\" ]"), "{out}");
    assert!(!out.contains("::"), "no defines remain: {out}");
}

#[test]
fn scene_and_wire_defaults_land_in_the_global_block() {
    let out = desugar_source("a -> b \"w\"\n").unwrap();
    assert!(out.contains("canvas-pad: 20;"), "scene defaults: {out}");
    assert!(out.contains("-> {") && out.contains("clearance: 16;"), "wire defaults: {out}");
    assert!(out.contains("a |box| .lini-box [ \"a\" ]"), "auto-create: {out}");
    assert!(out.contains("along: 0.5;"), "auto-along: {out}");
}

#[test]
fn desugar_is_idempotent() {
    let src = "g |group| [\n  |caption| \"T\"\n  a |box|\n]\nx -> y \"w\"\n";
    let once = desugar_source(src).unwrap();
    assert_eq!(desugar_source(&once).unwrap(), once, "idempotent");
}
```

- [ ] **Step 2: Run, expect failure**

Run: `cargo test -p lini --test desugar`
Expected: FAIL (placeholder `desugar` returns input unchanged).

- [ ] **Step 3: Implement the orchestration in `mod.rs`**

Replace the placeholder with the full pass. Structure:

```rust
pub fn desugar(file: &File) -> Result<File, Error> {
    let types = types::Types::build(file)?;
    let mut present: BTreeSet<String> = BTreeSet::new();   // bare type names used
    let mut element_rules: HashMap<String, Vec<Decl>> = HashMap::new();
    let mut user_items: Vec<StyleItem> = Vec::new();        // rewritten rules/classes/vars/root decls
    let mut define_order: Vec<String> = Vec::new();

    // 1. Walk the stylesheet: rewrite rules, collect element-rule decls + define decls.
    for item in &file.stylesheet {
        match item {
            StyleItem::Define(d) => {
                define_order.push(d.name.clone());
                // a define's own decls become its class's element-rule layer
                element_rules.entry(d.name.clone()).or_default().extend(d.style.iter().cloned());
                present.insert(d.name.clone());
                mark_chain(&types, &d.name, &mut present);
            }
            StyleItem::Rule(r) => match r.selector.parts.as_slice() {
                [SelPart::Type(t)] if t == "wire" => user_items.push(item.clone()), // wire defaults: merged later
                [SelPart::Type(t)] => {
                    element_rules.entry(t.clone()).or_default().extend(r.decls.iter().cloned());
                    present.insert(t.clone());
                    mark_chain(&types, t, &mut present);
                }
                _ => user_items.push(StyleItem::Rule(rewrite_selector(r))), // descendant/class rules
            },
            other => user_items.push(other.clone()), // Var, RootDecl
        }
    }

    // 2. Lower instances (collect present primitives/types as we go).
    let mut instances = Vec::new();
    for child in &file.instances {
        instances.push(lower_child(child, &types, &mut present)?);
    }

    // 3. Auto-create root boxes (after lowering: define bodies already inlined).
    let declared = scene::declared_ids_of(&instances);
    for (id, span) in scene::auto_created_ids_from(&file.wires, &declared) {
        present.insert("box".into());
        instances.push(lower_node(&scene::auto_box(&id, span), &types, &mut present)?);
    }

    // 4. Build the new stylesheet: root defaults + user root decls/vars + wire defaults
    //    + descendant/class rules + generated `.lini-*` class defs (ordered).
    let stylesheet = assemble_stylesheet(user_items, &present, &element_rules, &define_order);

    Ok(File {
        stylesheet,
        stylesheet_span: Span::empty(),
        instances,
        wires: file.wires.iter().map(labels::auto_along).collect(),
    })
}
```

Helper sketches (implement each fully):

- `mark_chain(types, name, present)` — resolve the type and insert every chain name + the primitive `kind.as_str()` into `present`.
- `rewrite_selector(rule)` — map each `SelPart::Type(t)` whose `t` is not a primitive-or-known-type-kept-as-class… actually map **every** `SelPart::Type(t)` → `SelPart::Class(lini_class(t))` (descendant rules now match via classes); leave `SelPart::Class` untouched.
- `lower_child` / `lower_node` — for a box: resolve its type via `types`, set `ty = Some(kind.as_str())`, prepend `classes` with `worn_classes(info)` (preserving any user `.classes` after), inline the define body children/wires ahead of its own, recurse into children, and append the id-as-label child via `labels::label_child_for(node, is_icon, is_container)` when there is no content. Text children pass through. Mark `present`.
- `assemble_stylesheet(...)` — order: (a) one `StyleItem::RootDecl` per `bundles::root_defaults()` **then** the user's root decls (user wins via resolve's collapse); (b) user `Var` items; (c) a `wire` `Rule` = `bundles::wire_defaults()` then any user `-> {}` decls; (d) user descendant/class `Rule`s; (e) `classes::class_defs(&present, &element_rules, &define_order)` as `StyleItem::Rule`s. Generated `.lini-*` defs come last in the block but `fmt`'s `print_file` emits them in vector order — that is fine for the cascade because resolve assigns tiers by class identity, not source order (Task 8).

Add the needed `use` lines and `mod scene; mod classes; mod bundles; mod types; mod labels;`.

- [ ] **Step 4: Repoint `lini desugar`**

In `src/lib.rs`, change `desugar_source`:

```rust
pub fn desugar_source(src: &str) -> Result<String, Error> {
    let tokens = lexer::lex(src)?;
    let file = syntax::parser::parse(&tokens)?;
    Ok(fmt::print_file(&desugar::desugar(&file)?))
}
```

- [ ] **Step 5: Run the desugar tests, expect pass**

Run: `cargo test -p lini --test desugar && cargo test -p lini --lib desugar`
Expected: PASS. Fix lowering until green. Confirm idempotency test passes (the lowered form must re-lower to itself — a primitive already wearing `.lini-box` must not gain a second).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "desugar: full lowering to primitives + .lini-* classes; repoint lini desugar"
```

---

## Task 8: The flip — class-based resolve; compile through desugar

Wire desugar into the compile pipeline and rewrite `resolve` to a class-only cascade. This is the keystone: it is one coherent deliverable (a partial flip leaves a broken pipeline). The oracle test (`compile(src) == compile(desugar(src))`) and the snapshot suite are the gates.

**Files:**
- Modify: `src/lib.rs` (`resolve_pipeline`)
- Modify: `src/resolve/cascade.rs`, `src/resolve/scene.rs`, `src/resolve/program.rs`, `src/resolve/ir.rs`, `src/resolve/mod.rs`, `src/resolve/defaults.rs`
- Delete: `src/resolve/types.rs`
- Modify: `src/render/values.rs` (`class_list`), `src/render/rules.rs` (source from worn classes)
- Modify: resolve unit tests across the above files
- Create: `tests/oracle.rs`
- Test: `tests/oracle.rs`, `tests/conformance.rs`, `tests/resolution.rs`, full suite

**Interfaces:**
- Consumes: `desugar::desugar` (Task 7).
- Produces: `ResolvedInst` carries `pub classes: Vec<String>` (worn, in order) instead of `type_chain` + `applied_styles`; `SheetInputs { class_rules: Vec<(String, AttrMap)>, wire_defaults: AttrMap }`.

- [ ] **Step 1: Add the oracle test (failing only if desugar is not yet wired)**

Create `tests/oracle.rs`:

```rust
//! Desugar transparency: compiling the lowered form must byte-match compiling the
//! source, and desugar must be idempotent through the full pipeline.

use lini::{Options, OutputFormat};

fn svg(src: &str) -> String {
    let opts = Options { bake_vars: true, format: OutputFormat::Svg, ..Default::default() };
    lini::compile_str_with(src, &opts).expect("compile")
}

#[test]
fn compile_is_transparent_to_desugar_for_every_sample() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("samples");
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("lini") { continue; }
        let src = std::fs::read_to_string(&path).unwrap();
        let lowered = lini::desugar_source(&src).expect("desugar");
        assert_eq!(svg(&src), svg(&lowered), "{}: compile(src) != compile(desugar(src))", path.display());
    }
}
```

- [ ] **Step 2: Wire desugar into the pipeline**

In `src/lib.rs`, `resolve_pipeline`:

```rust
fn resolve_pipeline(src: &str, opts: &Options) -> Result<resolve::Program, Error> {
    let tokens = lexer::lex(src)?;
    let file = syntax::parser::parse(&tokens)?;
    let lowered = desugar::desugar(&file)?;
    let theme = match &opts.theme_css {
        Some(css) => theme::extract_lini_vars(css),
        None => Vec::new(),
    };
    resolve::resolve_with_theme(&lowered, &theme)
}
```

- [ ] **Step 3: Run the suite to see the damage**

Run: `cargo test -p lini 2>&1 | tail -40`
Expected: many resolve/conformance failures (resolve still expects types; descendant rules now use `.lini-*`; double classes). This is the to-do list for the rest of the task.

- [ ] **Step 4: Trim `resolve/defaults.rs` to visual vars**

Delete the `set_layout_n` block (lines 65-75) and the `set_layout_n` fn. `built_in_defaults` now returns only visual `--lini-*` vars. (Layout constants live in `desugar/bundles.rs`.)

- [ ] **Step 5: Simplify `ResolvedInst` and `SheetInputs` (`resolve/ir.rs`)**

- Replace `type_chain: Vec<String>` + `applied_styles: Vec<String>` on `ResolvedInst` with `classes: Vec<String>` (all worn, in source order). Keep `shape: ShapeKind`.
- `SheetInputs` becomes `{ class_rules: Vec<(String, AttrMap)>, wire_defaults: AttrMap }`. Remove `element_rules`, `defines`, `templates`. `class_rules` holds **every** class def (the `.lini-*` and user ones), keyed by the bare class name (e.g. `lini-box`, `hot`).
- `ResolvedWire.applied_styles` stays (wire classes).

- [ ] **Step 6: Class-only cascade (`resolve/cascade.rs`)**

- `NodeFacts` drops `types`; keep `classes: Vec<String>`.
- `part_matches`: a `SelPart::Type` should no longer occur post-desugar (selectors are class-only), but keep a defensive arm matching nothing. `SelPart::Class` matches `facts.classes`.
- Add `pub fn class_decls(&self, class: &str) -> Vec<(String, ResolvedValue)>` returning a single-class rule's decls (for the tier-1 `.lini-*` lookup), merged across rules in source order.
- `node_layers` unchanged in spirit (descendant tier 2 + class tier 3) but tier 3 must **exclude** `.lini-*` classes (they are tier 1). Pass the node's user classes (non-`lini-`) for tier 3.

- [ ] **Step 7: Class-based node resolution (`resolve/scene.rs`)**

Rewrite `resolve_node`:
- `shape = ShapeKind::parse(node.ty.as_deref().unwrap_or("box"))` — post-desugar `ty` is always a primitive; error `unknown shape '{t}'` otherwise (this is the "bypass desugar" guard).
- Split `node.classes` into `lini: Vec<&str>` (tier 1, in order) and `user: Vec<&str>` (tier 3) via `name.starts_with("lini-")`.
- Build `ordered` = tier-1 (`sheet.class_decls("lini-X")` for each lini class, in order) ++ `sheet.node_layers(ancestors, &facts_user)` (tiers 2+3) ++ the instance block.
- Remove the type-cascade (`rt.defaults`), the define-body iteration (`rt.body_*`), and the id-as-label synthesis (desugar owns all three). Keep markers, skew validation, text inheritance, `drop_blank_text`, internal-wire lifting.
- `facts.classes = node.classes.clone()` (all worn, so descendant `.lini-*` selectors match).
- `ResolvedInst { classes: node.classes.clone(), shape, … }`.
- `INHERITED_TEXT` seeding unchanged.

- [ ] **Step 8: Slim the orchestrator (`resolve/program.rs`)**

- Delete `builtin_rules`, the `Types::build` call and `referenced_types` validation, `auto_created`/`auto_box` (now in desugar).
- `root_attrs`: drop the hardcoded `layout`/`padding` seed — just collapse the block's `RootDecl`s (desugar injected the defaults).
- `build_sheet_inputs`: collect **all** class rules (single-`SelPart::Class` rules, both `.lini-*` and user) into `class_rules`; the `wire` rule → `wire_defaults`. Drop templates/defines/element_rules collection.
- `Stylesheet::build` still consumes every `Rule`.

- [ ] **Step 9: Delete `resolve/types.rs` and update `resolve/mod.rs`**

```bash
git rm src/resolve/types.rs
```
Remove `mod types;` and the `type_chain_contains` fn from `resolve/mod.rs` (the latter moved to desugar in Task 6/7).

- [ ] **Step 10: Render reads worn classes (`render/values.rs`, `render/rules.rs`, `render/mod.rs`)**

- `class_list(shape, classes)` → `["lini-node"]` ++ `classes.map(|c| if c.starts_with("lini-") { c.clone() } else { format!("lini-style-{c}") })`. Update its one call site in `render/mod.rs:130` (drop `type_chain`/`applied_styles`, pass `&n.classes`).
- `render/rules.rs::build`: `present`/`used_styles` now derive from each node's `classes` (a `lini-` class → a shape rule key; a non-`lini-` class → a style key). Replace the `templates`/`defines`/`element_rules` loops with: for each present `lini-X` class, emit `.lini-X { paint subset of laid.sheet.class_rules["lini-X"] }`; for each used style, emit `.lini-style-Y`. Keep the closed-shape `stroke-dasharray: none` mask, the `.lini-text`/`.lini-marker`/`.lini-wire*`/`.lini-cut*` structural rules, and the type-before-style emission order. The `.lini` root rule's `font-size` reads from `laid` root attrs (the global block) — thread it via `LaidOut` (a `root_font_size: f64`) or read `laid.scene.attrs` if available at render.
- `collect(...)` keys on `node.classes` (lini → present, non-lini → used_styles) + `node.shape`.

- [ ] **Step 11: Rewrite resolve unit tests**

In `resolve/program.rs`, `resolve/scene.rs`, `resolve/cascade.rs` test modules: feed already-lowered source or wrap with `desugar`. Replace `type_chain`/`applied_styles`/`template_attrs` assertions with `classes` assertions (e.g. `caption_is_a_small_text_plain_title` checks `cap.classes.contains(&"lini-caption".into())`). For tests that build `Stylesheet`/cascade directly, construct `NodeFacts { classes }` only.

- [ ] **Step 12: Iterate to green; re-bless corrected snapshots**

Run: `cargo test -p lini 2>&1 | tail -40` — fix until only conformance snapshot diffs remain.
Run: `cargo test -p lini --test oracle` — must PASS (proves transparency).
Run: `git stash && cargo test -p lini --test conformance 2>/dev/null; git stash pop` … instead: `INSTA_UPDATE=always cargo test -p lini --test conformance` then `git diff tests/snapshots/` and confirm each diff is **only** a corrected fallback (e.g. a multiline-text `font-size`/spacing moving to the spec's `15`). No structural class changes should appear (Task 1 already did the rename). Bless.

- [ ] **Step 13: Full clean run**

Run: `cargo test -p lini && cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: PASS.

- [ ] **Step 14: Commit**

```bash
git add -A
git commit -m "resolve: class-only cascade; compile through desugar; drop the type system"
```

---

## Task 9: Dumb core — remove the engine fallbacks

The materialized defaults now always arrive in `attrs`, so the scattered `unwrap_or(const)` fallbacks are dead. Delete them and prove the core is dumb.

**Files:**
- Modify: `src/layout/primitives.rs` (`padding` 138, `gap` 147), `src/layout/mod.rs:181` (`canvas-pad`), `src/layout/mod.rs:199` + `src/layout/wires/labels.rs:72` + `src/layout/wires/bundle.rs:85` (font-size/stroke-width), `src/render/primitives.rs` (`radius` 129, `stroke-width` 100, `skew` 183), `src/render/mod.rs:194/257`, `src/render/rules.rs:451`, `src/render/wires.rs:50/443/473`
- Test: `tests/oracle.rs` (must still pass), new dumb-core test

**Interfaces:** none changed — values now read straight from `attrs`.

- [ ] **Step 1: Write the dumb-core proof test**

In `tests/oracle.rs`:

```rust
/// The core carries no defaults: a primitive with no `.lini-*` class (i.e. source
/// that bypassed desugar) has no radius/padding — proving every default lives in
/// the lowered form, not the engine.
#[test]
fn a_bare_core_box_has_no_baked_radius() {
    use lini::testing::node_rect;
    // Bare box, no .lini-box class, fixed size: with the radius fallback gone, the
    // engine sizes it exactly to width/height (no default padding growth).
    let lowered = "x |box| { width: 40; height: 40 }\n";
    let opts = lini::Options { bake_vars: true, ..Default::default() };
    let laid = /* compile to LaidOut via testing hook */ unimplemented!();
    // Assert the box is exactly 40x40 (no +padding), unlike a desugared box.
    let _ = (lowered, opts, laid, node_rect);
}
```

(If a direct `LaidOut` testing hook for arbitrary source is not exposed, instead assert via SVG: a bare-core `|box|` renders `rx="0"` while a desugared one renders `rx="6"`. Use whichever the public test API supports; the point is to pin "no class → no default".)

- [ ] **Step 2: Run, expect failure**

Run: `cargo test -p lini --test oracle a_bare_core_box`
Expected: FAIL (the `radius`/`padding` fallbacks still fire).

- [ ] **Step 3: Delete the fallbacks**

For each site, replace `attrs.number("X").unwrap_or(CONST)` with a read that returns the attr or the **identity** (not the old default):
- `radius` (render/primitives.rs:129): `n.attrs.number("radius").unwrap_or(0.0)` (absent ⇒ sharp).
- `padding` (layout/primitives.rs:128-138): absent ⇒ `PaddingBox::ZERO`.
- `gap` (layout/primitives.rs:147): absent ⇒ `0.0`.
- `stroke-width` (all sites): absent ⇒ `0.0`.
- `skew` (render/primitives.rs:183): absent ⇒ `0.0`.
- `font-size` (layout/mod.rs:199, render/mod.rs:194, wires labels.rs:72, wires.rs:443/473): absent ⇒ `0.0` for measurement (these only fire on text, which always inherits `font-size` from the global block now).
- `canvas-pad` (layout/mod.rs:181): absent ⇒ `0.0` (the root block always carries it).

Keep `unwrap_or(identity)` reads for genuinely-optional props (`opacity` 1.0, etc.) — those are not defaults, they are "property unset."

- [ ] **Step 4: Run the dumb-core test and the oracle**

Run: `cargo test -p lini --test oracle`
Expected: PASS (bare core has no radius; transparency holds because desugar supplies every default).

- [ ] **Step 5: Full suite — confirm no snapshot movement**

Run: `cargo test -p lini`
Expected: PASS with **zero** new snapshot diffs (the fallbacks were already dead after Task 8; this is pure removal). If any snapshot moves, a default was reaching a node only via the fallback — add it to the relevant `bundles.rs` set and re-run.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "core: remove baked default fallbacks — every default now flows through desugar"
```

---

## Task 10: Docs, sample, and visual verification

Bring SPEC/WIRING in line, add a teaching sample, and verify a rendered diagram visually.

**Files:**
- Modify: `SPEC.md` (§8, §11.3, §12, §13, §14, §18), `WIRING.md` (confirm no change needed)
- Create: `samples/desugar.lini` (a small diagram exercising a template, a define, and a wire label)
- Create: `tests/snapshots` entry via the existing conformance glob (auto-picks up the new sample)
- Test: `cargo test`, `resvg` visual check

- [ ] **Step 1: Update SPEC.md**

- §8: templates are `.lini-*` bundles lowered by desugar, not magic types.
- §11.3: defaults now materialize through desugar; list each home (table from the design doc §4).
- §12: the cascade carriers are classes (`.lini-*` tier 1, user classes tier 3).
- §13: SVG class names are `lini-*` / `lini-style-*` (no `shape` infix); show the new `<g class="lini-node lini-box lini-group">`.
- §14: `lini desugar` lowers everything and the lowered form re-renders identically; note the pipeline is `parse → desugar → resolve`.
- §18: add `node, text, marker, canvas, scene, cut` to reserved type names and note `.lini-*` is a reserved class prefix.

- [ ] **Step 2: Add the sample**

Create `samples/desugar.lini`:

```
{
  layout: row; gap: 30;
  |chip::box| { radius: 12; fill: #eef; }
}

api |group| [
  |caption| "API"
  health |chip| "OK"
  |badge| "v2"
]
db |cyl|
```

- [ ] **Step 3: Snapshot the sample**

Run: `INSTA_UPDATE=always cargo test -p lini --test conformance`
Run: `git status --short tests/snapshots/` — one new `…@desugar.lini.snap`. Read it; confirm `lini-chip`, `lini-group`, `lini-caption`, `lini-badge` classes appear and look right.

- [ ] **Step 4: Visual check (AGENT.md requirement)**

```bash
cargo run -q -- samples/desugar.lini --bake-vars -o /tmp/desugar.svg
resvg /tmp/desugar.svg /tmp/desugar.png
```
Then read `/tmp/desugar.png` and confirm: the group's dashed frame, the caption above it, the rounded `chip`, the corner badge, and the cylinder all render. Also eyeball `lini desugar samples/desugar.lini` output for readability.

- [ ] **Step 5: Cross-check desugar transparency on the sample**

Run: `cargo run -q -- samples/desugar.lini --bake-vars > /tmp/a.svg; cargo run -q -- desugar samples/desugar.lini | cargo run -q -- - --bake-vars > /tmp/b.svg; diff /tmp/a.svg /tmp/b.svg && echo IDENTICAL`
Expected: `IDENTICAL`.

- [ ] **Step 6: Final clean run + commit**

Run: `cargo test -p lini && cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check`

```bash
git add -A
git commit -m "docs: align SPEC with the desugar stage; add desugar sample + visual check"
```

---

## Self-Review

**Spec coverage:**
- Pipeline `parse → desugar → resolve` → Task 8 Step 2. ✓
- `.lini-*` reserved tier-1 model → Task 1 (render names), Task 4 (gen/split), Task 8 Steps 6-7 (tier-1 cascade). ✓
- Desugar transforms (templates, defines, element rules, descendant rules, labels, along, scene config, auto-create) → Tasks 2-7. ✓
- Dumb core (remove fallbacks) → Task 9; identity defaults stay implicit → Task 9 Step 3. ✓
- Module layout → Tasks 2-7. ✓
- Resolve changes (delete types.rs, trim defaults.rs, class cascade, SheetInputs) → Task 8. ✓
- Render changes (rename, worn-class mapping, masks preserved) → Tasks 1, 8 Step 10. ✓
- Reserved words → Task 1 Step 3, Task 10 Step 1. ✓
- Doc updates → Task 10. ✓
- Testing (oracle, idempotency, dumb-core proof, snapshots, visual) → Tasks 7, 8, 9, 10. ✓

**Placeholder scan:** Task 9 Step 1's test body has an `unimplemented!()` sketch with an explicit fallback instruction (assert `rx="6"` vs `rx="0"` via SVG) — the executor picks the form the public test API supports; this is a known API-availability branch, not a vague TODO. No other placeholders.

**Type consistency:** `ResolvedInst.classes: Vec<String>` (Task 8 Step 5) is consumed by `class_list(shape, &n.classes)` and `collect` (Task 8 Step 10). `Types::resolve → TypeInfo { kind, chain }` (Task 3) feeds `worn_classes` and `mark_chain` (Tasks 4, 7). `SheetInputs { class_rules, wire_defaults }` (Task 8 Step 5) consumed by `render/rules.rs` (Task 8 Step 10). `desugar(&File) -> Result<File, Error>` (Task 7) consumed by `resolve_pipeline` (Task 8 Step 2). Consistent.

**Ordering note:** Task 8 is the one large, indivisible task (a partial flip is a broken build). Tasks 1-7 deliberately front-load all isolated, independently-green work so the flip is as small as possible.

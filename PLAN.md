# PLAN — node / block / link refactor

The contract is [`SPEC.md`](SPEC.md) + [`LINKING.md`](LINKING.md) (committed in
`551c5bf`). This file is the build order for the **code** to match them. Pre-v1,
so **no backward compatibility** — rename and rewrite freely, leave no patchwork.

> **For agentic workers:** steps use `- [ ]` checkboxes. Each step is one
> green, committable milestone — finish a step's checklist, run its **Gate**,
> commit, then move on.

## The bet

One sweeping rename, then three focused reshapes. Behavior is preserved except
where the SPEC deliberately changed it; the test suite (snapshots + the wiring
sweep) is the safety net at every step.

## Ground rules (from AGENTS.md)

- **No `unsafe`.** One concept per file; split past ~500 LOC.
- `insta` snapshot tests own the output. After an intended output change, review
  with `cargo insta review` (never blind-accept) and confirm each diff is only
  the change you meant.
- **Verify SVG visually** for any step that changes pixels: render a sample to
  PNG with `resvg` into the scratchpad and *look* — don't ship a visual on faith.
- **Gate before every commit:** `cargo fmt --all`, then `cargo clippy --all-targets`
  and `cargo test` both clean (CI runs `cargo fmt --all -- --check`).
- Defer pushing `main`/the branch to the user. We are on branch
  `node-link-refactor`.

## Map (the refactor surface)

`git grep -il wire` spans ~60 files; `git grep -il airwire` 12; `git grep -il plain`
~20. The ShapeKind model lives in `src/resolve/ir.rs`; templates + bundles in
`src/desugar/{types,bundles}.rs`. These greps are the checklist for Steps 1–2.

**Steps 1–2 leave `-> {}` working** (renamed to the link layer) — it is removed in
Step 3. That intermediate divergence from the SPEC is expected and temporary.

---

## Step 1 — Rename `wire` → `link` (mechanical, behavior-preserving)

One atomic rename across the whole tree: identifiers, modules, files, SVG class
names, error strings, the themable var. **No semantics change** — only names.
Output shifts only in class/var spelling (`lini-wire`→`lini-link`,
`lini-wires`→`lini-links`, `--lini-airwire`→`--lini-stray`), so every snapshot
re-baselines to an otherwise-identical SVG.

**Identifier mapping** (apply everywhere; `LineStyle` stays — it is neutral):

| from | to |
|---|---|
| `WireOp` / `WireMarker` | `LinkOp` / `LinkMarker` |
| `TokKind::WireOp`, `lex_wire_op`, `is_wire_line_start` | `…Link…` |
| `Wire`, `parse_wire`, `*_wire_op`, `wire_op_str` | `Link`, `parse_link`, `*_link_op`, `link_op_str` |
| `File.wires`, `Node.wires`, `Define.wires`, `Program.wires` | `…links` |
| `ResolvedWire`, `child_has_wire`, `wire_rule` | `ResolvedLink`, `child_has_link`, `link_rule` |
| reserved selector element `"wire"` | `"link"` |
| `airwire` (all forms), `--lini-airwire`, `.lini-airwire` | `stray`, `--lini-stray`, `.lini-stray` |
| SVG `lini-wire`, `lini-wires` | `lini-link`, `lini-links` |
| error text `wire endpoint/chain/…`, `\|wire\|` | `link …`, `\|link\|` |

**File / dir renames** (use `git mv` to keep history):

- [ ] `git mv src/layout/wires src/layout/links` (+ update `mod` paths in `src/layout/mod.rs`)
- [ ] `git mv src/render/wires.rs src/render/links.rs`
- [ ] `git mv src/resolve/wires.rs src/resolve/links.rs`
- [ ] `git mv tests/wiring.rs tests/linking.rs` · `git mv tests/wiring_sweep.rs tests/linking_sweep.rs`
- [ ] `git mv samples/wires.lini samples/links.lini` (and `wires_simple/medium/hard` → `links_*`); leave `pcb*.lini` as-is (a routing stress fixture, not user-facing).

**Edits** (the grep list is the checklist — work module by module):

- [ ] Shared vocab + front end: `src/ast.rs`, `src/lexer.rs`, `src/syntax/ast.rs`, `src/syntax/parser.rs`.
- [ ] Desugar: `src/desugar/{mod,bundles,scene,labels,types}.rs` (rename `wire_defaults`/`wire_rule`/the `"wire"` selector; `is_builtin_type` swaps `"wire"`→`"link"`).
- [ ] Resolve: `src/resolve/{links,program,scene,ir,mod,merge}.rs`.
- [ ] Layout: `src/layout/{mod,ir}.rs` + every file under `src/layout/links/`.
- [ ] Render: `src/render/{mod,links,rules,markers,used_vars,values,wavy,rounding,gradients}.rs` (paint defaults, the structural `lini-link` rule, `lini-links` layer, stray var/class).
- [ ] App + defaults: `src/lib.rs`, `src/main.rs`, `src/resolve/defaults.rs` (the `--lini-stray` palette var), `src/theme.rs`, `src/fmt.rs` + `src/fmt/*`, `src/serve/playground.html`.
- [ ] Tests + samples: `tests/*.rs`, the renamed sample files.

**Gate:**

- [ ] `cargo build` clean.
- [ ] `git grep -in '\bwire' src tests | grep -vi 'wired\? (prose)'` returns nothing — no `wire` identifier survives (allow only deliberate English, of which there should be none after this step).
- [ ] `cargo insta review` — every diff is purely `wire→link` / `airwire→stray` class/var renames over identical geometry.
- [ ] `cargo fmt --all` · `cargo clippy --all-targets` · `cargo test` green.
- [ ] PNG-verify one sample (`samples/links.lini`) — pixels identical to pre-rename.

**Commit:** `refactor: rename wire → link across the codebase`

---

## Step 2 — `block` primitive + template rebase

Make the bare rectangle the base primitive and demote `box` to a template, per
SPEC §7/§8. This changes which `.lini-*` classes carry paint, but a default box
must render **pixel-identical**.

- [ ] `src/resolve/ir.rs`: rename `ShapeKind::Box` → `ShapeKind::Block`; `ShapeKind::parse` maps `"block"` → `Block` and **no longer** maps `"box"`. Update every `ShapeKind::Box` match arm in `src/layout/{primitives,ir,mod}.rs`, `src/layout/links/{scene,validate}.rs`, `src/render/mod.rs`, `src/desugar/{classes,mod}.rs` to `Block`.
- [ ] `src/desugar/types.rs` — `TEMPLATES` becomes (note `box` joins as a template, `plain` is removed):

```rust
pub const TEMPLATES: &[(&str, &str)] = &[
    ("box", "block"),
    ("rect", "box"),
    ("group", "block"),
    ("caption", "block"),
    ("footer", "caption"),
    ("badge", "block"),
    ("note", "block"),
    ("row", "block"),
    ("column", "block"),
    ("table", "group"),
];
```

- [ ] `src/desugar/bundles.rs`:
  - `primitive_bundle(Block)` is bare — `fill: none; stroke: none; padding: 0; gap: 20` (a frameless div; `radius` defaults to 0 with no decl). The old `sized()` paint moves to the `box` template.
  - `template_bundle("box")` = `fill: --fill; stroke: --stroke; stroke-width: 2; radius: 6; padding: 20` (the paint lifted off the old `Box` primitive).
  - Re-base `group` / `note` / `badge` / `caption` onto `block`: add the `padding: 20` that `group`/`note` used to inherit from `box`; drop now-redundant `stroke: none` from `badge`/`note` (block has none).
  - Delete the `"plain"` arm.
- [ ] `src/desugar/mod.rs` — confirm the omitted-type default stays `"box"` (now a template) in `lower_node`; `is_container` still keys on `"group"`.
- [ ] `samples/*.lini`: replace every `|plain|` with `|block|` (`grep -l plain samples`). Default boxes are untouched.

**Gate:**

- [ ] `cargo build` · `cargo clippy` · `cargo fmt --all` clean.
- [ ] `cargo insta review`: a default `|box|` is now `class="lini-node lini-box lini-block"` with paint on `.lini-box`; geometry/paint values unchanged. `|block|` (ex-`|plain|`) renders frameless as before.
- [ ] **PNG-verify** `samples/templates.lini` + `samples/shapes.lini` light **and** dark — box framed, group dashed, note/badge/caption unchanged.
- [ ] `cargo test` green.

**Commit:** `feat: rebase the shape model on a bare |block| primitive`

---

## Step 3 — Link styling cascade + `routing` + `href` (drop `-> {}`)

Replace the `-> {}` rule with cascading `link` / `link-width` / `link-style`, add
the `routing` property, and move the hyperlink prop to `href` (freeing `link`).

- [ ] **`href`:** in `src/render/mod.rs` and `src/render/links.rs`, read the attr key `"href"` (was `"link"`) for the `<a href>` wrap; add `href` to the property allowlist wherever `link` (hyperlink) was accepted.
- [ ] **Drop `-> {}`:** `src/syntax/parser.rs` — remove the link-op branch from `classify_setup` and the `-> {}` arm from `parse_rule` (a `->` in the stylesheet is now an error). `src/desugar/mod.rs` + `bundles.rs` — delete the `link_rule` injection and the reserved `"link"` selector; rename `link_defaults()` to the cascade defaults below.
- [ ] **Link paint family:** add `link` / `link-width` / `link-style` as resolved properties. In `src/resolve/links.rs`, resolve each link by walking its scope's ancestor chain for `link*` / `clearance` / `routing` (extend the mechanism `clearance` already uses), then class rules, then the link's own block. `src/render/links.rs` paints the path stroke from `link` / `link-width` and the dash from `link-style` (operator-set, overridable).
- [ ] **Defaults:** in `src/desugar/bundles.rs`, fold the link defaults (`link-width: 2`, `clearance: 16`, `link-font-size: 11`) into `root_defaults()` so they cascade from the root; `link` colour defaults to `--stroke` in render.
- [ ] **`routing`:** accept `routing: orthogonal` (the built mode); `straight` / `curved` error as deferred (SPEC §19) — add the message to `src/resolve/*` validation and SPEC §15 if missing.
- [ ] **Samples:** migrate `-> { stroke: …; clearance: … }` → root `link: …; clearance: …`; link `.class { stroke: red }` → `{ link: red }`.

**Gate:**

- [ ] `cargo build` · `cargo clippy` · `cargo fmt --all` clean.
- [ ] A `-> {}` block now errors with a clear message (add/observe a parse test).
- [ ] `cargo insta review`: link colours/widths now resolve from `link*`; SVG geometry unchanged.
- [ ] **PNG-verify** `samples/links.lini` + `samples/flow.lini` — links look identical to Step 2.
- [ ] `cargo test` + the wiring sweep (`tests/linking_sweep.rs`) green.

**Commit:** `feat: cascade link/link-width/link-style + routing, drop -> {} and href the hyperlink`

---

## Step 4 — Stylable text + link `[ ]` + tooling sweep

The two additive syntax changes, then bring the tooling and docs in line.

- [ ] **Stylable text:** `src/syntax/ast.rs` — add `style: Vec<Decl>` to `TextNode`. `src/syntax/parser.rs` — after a string (canvas, `[ ]`, or trailing label) consume an optional `{ }` into it. `src/resolve/*` — merge a text node's own style (text-valid props only); a box-only prop on text errors (`'pin' needs a box…`). `src/layout/text.rs` — honour `translate` / `rotate` on a text node. `src/render/mod.rs` — emit a text node's style as `style="…"` and `translate`/`rotate` as `transform` on the `<text>`.
- [ ] **Link `[ ]`:** `src/syntax/ast.rs` — `Link` carries labels as `[ ]` children. `src/syntax/parser.rs::parse_link` — after the head + optional `{ }`, accept `[ children ]` **or** trailing labels (sugar). `src/desugar/labels.rs` — lower trailing labels into the `[ ]` form. Delete the "a wire is not a container" error. Labels already flow to layout/render — source them from the `[ ]`.
- [ ] **fmt:** `src/fmt.rs` + `src/fmt/*` — format `"x" { … }`, a link's `[ … ]` labels, and the `link` / `link-width` properties; keep idempotence (the fmt test suite).
- [ ] **VSCode grammar:** `editors/vscode/syntaxes/lini.tmLanguage.json` — link ops, `block`, `link`/`link-width`/`link-style`/`routing`/`href`; remove `plain`/`wire`.
- [ ] **Docs/serve:** `README.md` (rewrite wire→link, box→block, examples), `src/serve/{single,playground}.html` (any wire/plain copy).
- [ ] Add one sample exercising the new surface (e.g. `samples/text.lini`: styled standalone text + a link with a styled `[ ]` label).

**Gate:**

- [ ] `cargo build` · `cargo clippy` · `cargo fmt --all` clean.
- [ ] New parse/desugar tests: `"x" { color: red }` → styled `<text>`; `a -> b "x"` desugars to `a -> b [ "x" ]`; a box-only prop on text errors.
- [ ] `cargo insta review` (new sample + fmt round-trips).
- [ ] **PNG-verify** the new sample — styled text colour/rotate/translate land; link label styles apply.
- [ ] `lini fmt --check` is idempotent on every sample; `cargo test` fully green.

**Commit:** `feat: stylable text + link [ ] labels; update fmt, grammar, docs`

---

## Risks & notes

- **Snapshot churn is the point, not noise.** Review every diff; the danger is a
  *geometry* change sneaking in under a rename. Steps 1 and 3 must show
  geometry-identical SVGs.
- **`ShapeKind::Text`** already exists and stays — it is the leaf text kind; Step 4
  adds its style block, it does not become a `block`.
- **Property allowlist / lint.** If `src/lint.rs` enumerates valid props per node
  kind, extend it for `link*`, `routing`, `href`, and text's style set as those
  land (Steps 3–4); this also seeds the TODO diagnostics pass.
- **`href` collision** is the reason it moves off `link` — do the `href` rename
  before introducing the `link` paint prop within Step 3.

## Decisions (locked — from the brainstorm)

| Decision | Choice |
|---|---|
| Connector noun | `link` (+ `link-width` / `link-style`); `node` = umbrella concept, not writable |
| Base primitive | `|block|` (bare, div-like); `|box|` the default template over it; `|plain|` removed |
| Routing | `routing: orthogonal` default, cascades like `clearance`; `straight`/`curved` deferred |
| Link defaults | cascade from scope (`-> {}` removed) |
| Link labels | a `[ ]` of styleable text leaves; trailing label is sugar |
| Text | always a `<text>` leaf, stylable in place; never wrapped |
| Hyperlink prop | `href` (was `link`) |
| Impossible link | `stray link` (`--lini-stray`) |
| Order | rename → block rebase → link styling/routing/href → text + `[ ]` + tooling |

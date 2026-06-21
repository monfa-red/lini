# Desugar to primitives — design

Date: 2026-06-20
Status: brainstorm approved; pending spec review → implementation plan

## Motivation

Today the type system is woven through `resolve`: `resolve/types.rs` walks templates
and `|name::base|` defines, `template_attrs` injects each type's bundle as a tier‑1
"type cascade," `resolve/scene.rs` synthesizes the id‑as‑label text, and
`resolve/program.rs` seeds the root container's config. Defaults live in two
inconsistent places — some as injected attrs (template bundles), some as scattered
`unwrap_or(…)` fallbacks read deep in `layout`/`render` (`radius` 6, `padding` 20,
`gap` 20, …).

This makes the engine harder to reason about and gives the user no honest view of
"what does `|group|` actually mean?" The current `lini desugar` only expands two
surface sugars (id‑as‑label, auto‑`along:`); it does not show that a `|badge|` is a
box with a pin, a shadow, and a fill.

**Goal.** Make desugaring a real pipeline stage that lowers *all* sugar to
primitives, so the rest of the engine only ever sees primitive shapes and explicit
classes. After desugar there are **no types in the cascade** — only primitives (for
kind/geometry) and classes. Every template, define, element rule, and type used in a
descendant selector collapses into one uniform `.lini-*` class namespace.

This decouples the whole type/default system from the core, shrinks `resolve` to
*variables + class cascade + scene tree + wires*, and gives the user a faithful,
re‑renderable view of the lowered diagram.

## Decisions locked (from brainstorming)

1. **Dumb core.** The post‑desugar engine carries **no** baked geometry/layout
   default. Every such constant is materialized by desugar (into a `.lini-*` class,
   the global block, or the `-> { }` wire defaults). The scattered `unwrap_or(const)`
   fallbacks in `layout`/`render` are **removed**. A primitive fed to the core with
   no `.lini-*` class genuinely has no defaults.

2. **`.lini-*` class scheme.** Generated type classes are `.lini-box`, `.lini-group`,
   `.lini-<define>` in the desugared source, mapping **verbatim** to the same SVG
   class (the redundant `shape` infix is dropped: `lini-shape-box` → `lini-box`).
   User classes keep their `lini-style-` infix (`.hot` → `.lini-style-hot`), which
   also prevents a user class named `box` from colliding with the `box` shape.
   `.lini-*` is a reserved class prefix the user may not define.

## 1. Pipeline

```
lex → parse → DESUGAR → resolve → layout → render
```

`desugar: File → Result<File, Error>` is a total AST→AST lowering. `lini desugar`
prints its output; the compile path runs `resolve` on it. The same function feeds
both, so the teaching view and the compiled artifact can never drift.

**Invariants (the test oracle):**

- `compile(src)` ≡ `compile(desugar(src))` — desugar is transparent to the SVG.
- `desugar(desugar(x))` ≡ `desugar(x)` — idempotent; the lowered form is a fixed
  point (every primitive already wears its `.lini-*` class, every label is explicit).

Errors that depend on the type system move to desugar (inheritance cycle, depth > 16,
a define shadowing a built‑in). "unknown shape" stays a `resolve` error: desugar
expands only *known* templates/defines and passes anything else through, so a stray
`|ghost|` (or a hand‑written `|group|` that bypassed desugar) surfaces as
`unknown shape 'ghost'` in the core — exactly the decoupling we want.

## 2. The `.lini-*` class model

After desugar a node is `id |primitive| .lini-<chain> [.user-classes] { own block } [ body ]`.
The `|primitive|` carries the kind (geometry/render); the `.lini-*` chain carries the
type cascade as classes.

**Cascade tiers (unchanged specificity, new carriers):**

| Tier | Was | Now |
|---|---|---|
| 1 — type | type chain (template/define bundles + element rules), base→derived | worn `.lini-*` classes, in chain order, resolved from their stylesheet defs |
| 2 — descendant | `\|table box\|` | `\|.lini-table .lini-box\|` (type parts rewritten to `.lini-*`) |
| 3 — class | `.hot` | worn user classes (the non‑`lini-` ones) |
| 4 — instance | `x \|box\| { … }` | unchanged |

The reserved‑prefix rule is the whole trick: a worn class beginning `lini-` is the
**type tier (1)**; any other worn class is the **class tier (3)**. Descendant rules
stay tier 2 regardless of whether their parts are `.lini-*` or user classes. This
reproduces SPEC §12 exactly while collapsing four mechanisms (templates, defines,
element rules, type‑selectors) into one (classes).

**Render mapping** (the `lini-` prefixing stays a render concern, so hand‑written
lowered source is safe):

- structural markers (`lini-node`, `lini-text`, `lini-wire`, `lini-marker`,
  `lini-scene`, `lini-wires`, `lini-canvas`, `lini-cut*`) — render‑emitted scaffolding,
  **not** source classes (as today).
- a worn `.lini-*` class — emitted **verbatim**.
- a worn user class — emitted as `.lini-style-<name>`.

So `g |box| .lini-box.lini-group .hot` →
`<g class="lini-node lini-box lini-group lini-style-hot" data-id="g">`.

## 3. Desugar transforms (the contract)

| Sugar (input) | Lowered (output) |
|---|---|
| `\|group\|` instance | `\|box\| .lini-box.lini-group` + the instance's own `{ }` and body |
| a type's default bundle | a generated `.lini-<type> { … }` at the top of the global block |
| `\|box\| { radius: 4 }` element rule | merged into the generated `.lini-box { … }` |
| `\|table box\| { … }` descendant rule | `\|.lini-table .lini-box\| { … }` |
| `\|treat::box\| { d } [ body ]` define | `.lini-treat { d }` + `body` inlined into each instance's `[ ]` (ids scoped as today by `resolve`'s lifting) |
| id‑as‑label / trailing label | explicit `[ "…" ]` text child (as today) |
| auto‑distributed wire labels | explicit `along:` list (as today) |
| root‑wire endpoint declared nowhere | an auto‑created `\|box\| .lini-box [ "id" ]` at the scene root |
| scene/root defaults + user `--name` + user rules/classes | carried into the global `{ }` block |

**Class chain order.** A node wears its primitive class plus every template/define in
its chain. Stored/emitted in the order that keeps the SVG byte‑identical to today's
`type_chain`+primitive order; the tier‑1 fold applies base→derived so a derived type
overrides its base. (Concrete order is an implementation detail pinned by the
byte‑diff check; not user‑facing.)

**What stays out of desugar (render‑side, no layout impact):** the visual `--lini-*`
palette and the `@layer lini.defaults { }` block. They are a live runtime‑theming
layer; baking them would defeat `var()` theming. The lowered `.lini-*` class defs
*reference* them (`.lini-box { fill: --fill }` → `var(--lini-fill)`), exactly as a
user writes `fill: --accent` without declaring it.

## 4. Dumb core — every removed default's new home

Each scattered fallback is deleted and the value materialized so `attrs` always
carries it.

| Default (old fallback) | Value | New home |
|---|---|---|
| `radius` | 6 | `.lini-box { radius: 6 }` · `.lini-rect { radius: 0 }` |
| `padding` | 20 | every container `.lini-<kind>`; root → global block `padding: 0`; `.lini-plain`/`row`/`column` → `padding: 0` |
| `gap` | 20 | every container `.lini-<kind>`; root → global block `gap: 20` |
| `stroke-width` | 2 | every closed `.lini-<kind>` + `.lini-line`; `.lini-group { stroke-width: 1 }`; wires → `-> { stroke-width: 2 }` |
| `skew` | 15 | `.lini-slant { skew: 15 }` |
| `icon-size` | 24 | `.lini-icon { width: 24; height: 24 }` |
| `font-size` body | 15 | global block `font-size: 15` (inherits) |
| `font-size` caption | 12 | `.lini-caption { font-size: 12 }` (already in the template bundle) |
| `font-size` wire label | 11 | `-> { font-size: 11 }` |
| `line-height` | 1.2 | global block `line-height: 1.2` (inherits) |
| `text-align` | center | global block `text-align: center` (inherits) |
| `clearance` | 16 | `-> { clearance: 16 }` |
| `canvas-pad` | 20 | global block (root config) |

**Identity defaults stay implicit.** A property whose default *is its absence*
(`letter-spacing: 0`, `translate: 0 0`, `rotate: 0`, `opacity: 1`, `stack`/`shadow`
off, `pin: none`, `divider: none`) is **not** materialized — absent already means the
no‑op. "Dumb core" removes defaults that *do something visible*, not the engine's
treatment of an unset property. (Consumers read these as `attrs.get(...).unwrap_or(identity)`,
which is reading "is the property set," not "what's the hidden default.")

Visual `--lini-*` defaults are unaffected (see §3).

## 5. Module layout

```
src/desugar/
  mod.rs        — pub fn desugar(&File) -> Result<File, Error>; orchestration; the entry lib.rs calls
  types.rs      — TEMPLATES table + define walk/validation (moved from resolve/types.rs)
  bundles.rs    — every built-in default as AST Decls: per-primitive constants + template deltas
                  (replaces resolve/types.rs::template_attrs + the layout half of resolve/defaults.rs)
  classes.rs    — .lini-* class-def generation, the worn-class chain, the reserved-prefix split helper
  labels.rs     — id-as-label · trailing label · auto-along (today's src/desugar.rs)
  scene.rs      — root/scene config → global-block decls; auto-create
```

`src/desugar.rs` (file) becomes `src/desugar/` (dir). `resolve::type_chain_contains`
moves into `desugar` (it is desugar‑only today).

## 6. Resolve changes

- **Delete `resolve/types.rs`** (the type cascade is gone) and the `Types`/`ResolvedType`
  plumbing in `resolve/program.rs`/`scene.rs`.
- **`resolve/defaults.rs`** keeps **only** the visual `--lini-*` palette. The layout
  constants move to `desugar/bundles.rs`. `VarTable` therefore holds visual vars only.
- **`resolve/scene.rs`**: node resolution loses type resolution, template bundles,
  define‑body materialization, and id‑as‑label synthesis (all now upstream). It
  becomes: kind from `|primitive|`, split worn classes into tier‑1 `.lini-*` (applied
  in order from their defs) and tier‑3 user classes, overlay descendant rules (tier 2)
  and the instance block (tier 4). `ResolvedInst` drops `type_chain`; it carries the
  flat worn‑class list (order preserved) + the primitive `shape`.
- **`resolve/cascade.rs`**: `NodeFacts.types` is removed (everything matches via
  classes now). `Stylesheet` keeps the generated `.lini-*` and user class defs;
  `element_decls`/`referenced_types` are removed. `node_layers` gains the tier‑1
  `.lini-*` lookup (or that lookup lives beside it); selector matching is class‑only.
- **`resolve/program.rs`**: `builtin_rules` and the `SheetInputs.templates/defines/
  element_rules` fields go away. `SheetInputs` becomes `{ class_rules, wire_defaults }`.
  `root_attrs` drops its hardcoded `layout: column; padding: 0` (desugar injects those
  into the global block); it just collapses the block's root decls. Auto‑create moves
  to desugar.

## 7. Render changes

- **`values::class_list`**: `["lini-node"]` + each worn class mapped (`.lini-*`
  verbatim, else `lini-style-*`). Drop the `type_chain`/primitive auto‑append.
- **`render/rules.rs`**: `lini-shape-*` → `lini-*` throughout. Per‑kind paint rules,
  template/define paint rules, and element‑rule merges all collapse into "emit the
  paint subset of each present `.lini-*` class def." Type (`.lini-*`) rules are emitted
  before user (`.lini-style-*`) rules so the CSS cascade matches the tier order. The
  `.lini` root rule's `font-size` reads from the resolved root attrs (the global
  block), not the var table. The non‑type structural rules are **unchanged** — render
  still synthesizes the closed‑shape `stroke-dasharray: none` mask, the `lini-marker*`,
  `lini-wire*`, `lini-wire-label`, and `lini-cut*` rules exactly as today; only the
  shape/type rules change name (drop `shape`) and source (the class def vs the old
  template/element tables).
- **Remove every `unwrap_or(const)`** for a materialized default (`radius`, `padding`,
  `gap`, `stroke-width`, `skew`, `icon-size`, `font-size`, …) in `render`/`layout`;
  read straight from `attrs`. Keep `unwrap_or(identity)` only for the implicit
  identity defaults of §4.

## 8. Reserved words (SPEC §18 update)

Add the structural class names to the reserved‑type list so a define can't shadow a
render marker: **`node`, `text`, `marker`, `canvas`, `scene`, `cut`** (joining the
existing **`wire`**). `.lini-*` becomes a reserved **class** prefix (a user `.lini-foo`
definition is an error). The four sides stay reserved as ids.

## 9. Documentation updates

- **SPEC.md**: §8 (templates as `.lini-*` bundles, not magic types), §11.3 (defaults
  now materialize through desugar; list their homes), §12 (cascade carriers), §13
  (SVG class names `lini-*`/`lini-style-*`, no `shape` infix), §14 (`lini desugar` now
  lowers everything, and the lowered form re‑renders identically), §18 (reserved
  words above), the EBNF note that the lowered form is the engine's true input.
- **WIRING.md**: untouched (routing is unaffected; wire defaults still arrive via
  `-> { }`).

## 10. Testing

- **Oracle test** (new): for every `samples/*.lini`, assert `compile(src)` byte‑equals
  `compile(desugar(src))`, and `desugar(desugar(src))` equals `desugar(src)`.
- **Snapshot suite** (`insta`): expected diffs are exactly (a) the global `lini-shape-*`
  → `lini-*` rename, and (b) the handful of *corrected* fallback inconsistencies
  (e.g. multiline text spacing moving from the stray `14` to the spec's `15`). Each is
  reviewed and re‑blessed; nothing else moves.
- **Unit tests**: `resolve` tests that poked `type_chain`/`template_attrs` move to
  `desugar` and assert on the lowered AST (the class chain, the generated class defs,
  the inlined define body). New `desugar` tests cover each transform in §3 and each
  default home in §4.
- **Visual check** (AGENT.md): render one new `samples/` desugar example to PNG with
  `resvg` and read it, confirming the lowered form draws identically.

## 11. Risks · non‑goals · deferred

- **Risk — a missed default.** Dumb core means an un‑materialized constant renders
  wrong (e.g. radius 0). The oracle test over every sample is the safety net; the diff
  must reduce to the known rename + corrections.
- **Risk — verbosity.** Every node now wears `.lini-<kind>` and every box shows an
  explicit `[ "id" ]`. This is the intended teaching view; per‑node lines stay short,
  only the global block grows (one class def per present type).
- **Non‑goal — moving wire operator lowering.** `op → markers/stroke-style` stays in
  `resolve` (it is syntax, not a default).
- **Deferred — generated `// comments`.** The class names are self‑documenting; the
  AST has no comment node, so annotated output (`// group brings a dashed frame`) is a
  later polish, not v1.

# PLAN — implement the new lini syntax

Tracks the code refactor that makes the toolchain speak the syntax in `SPEC.md`
(commit `5abd57d`). **Delete this file when all three steps are done.**

## Principles (read first, every session)

- **`SPEC.md` is law.** Build to it. `WIRING.md` is unchanged — routing geometry
  is syntax-agnostic; only the per-label `translate` clause was dropped.
- **Refactor as if this were day one.** No old names, no dead branches, no
  compatibility shims, no "this used to be X" comments. Delete what the redesign
  obsoletes. The diff should read like the language was always this way.
- **Streamline where the redesign frees you** (each step lists its wins). The new
  syntax *removes* machinery — take the deletions.
- **One concept per file; split past ~500 LOC** (AGENT.md). Prefer small, clear
  modules and obvious names over clever code.
- **Every step ends green and is committed.** Done = `cargo test` +
  `cargo fmt --all -- --check` + `cargo clippy` all clean, and the relevant `lini`
  subcommands correct on new-syntax input. A step is a session boundary and a
  checkpoint.
- **Verify SVG visually** — render a couple of samples with `resvg` to PNG and
  *look* (AGENT.md). Snapshots alone don't prove the picture is right.

## The shape of the change (why it's smaller than it looks)

The AST already separated *style* from *content*: `Block { decls, children, wires }`
and `File { stylesheet, instances, wires }`. The new syntax is mostly a **re-spelling**
— `{ }` → decls, `[ ]` → children+wires — so `layout` / `render` and the *logic* of
`resolve` are untouched. The work concentrates in **lexer → AST → parser → fmt**,
with mechanical field-access updates in `resolve` / `desugar`. The old `ident { }`
rule-vs-node ambiguity (and its `types` set + "define before use" parse rule) simply
**disappears**: rules live in the stylesheet `{ }`, the canvas holds only instances.

## Target AST (the Step-1 contract — `src/syntax/ast.rs`)

Replace `Block` and `WireBlock` entirely. A node, a define, and a container body all
share the same `style` / `children` / `wires` triple, sourced from a `{ }` then a `[ ]`.

```rust
pub struct File {
    pub stylesheet: Vec<SetupItem>,   // the leading { } block (empty Vec if absent)
    pub canvas: Vec<Child>,           // instances — boxes and text, in source order
    pub wires: Vec<Wire>,             // root wires
}

pub enum SetupItem {                  // what lives in the stylesheet { }
    RootDecl(Decl),                   // key: value  — configures the root
    Var(Decl),                        // --name: value
    Rule(Rule),                       // |selector| { } , .class { }
    Define(Define),                   // |name::base| { } [ ]
    WireDefaults(Vec<Decl>),          // -> { }   (was Rule{selector: wire}; a clean variant now)
}

pub struct Node {                     // a box
    pub id: Option<String>,
    pub ty: Option<String>,           // None = default |box|
    pub classes: Vec<String>,         // worn classes from |type.class.class| / |.class|
    pub style: Vec<Decl>,             // the { } block
    pub children: Vec<Child>,         // the [ ] block: children…
    pub wires: Vec<Wire>,             // …then internal wires
    pub span: Span,
}

pub enum Child { Box(Node), Text(TextNode) }   // unchanged

pub struct Wire {
    pub chain: Vec<EndpointGroup>,
    pub op: WireOp,
    pub classes: Vec<String>,         // trailing .class (one floating class)
    pub style: Vec<Decl>,             // the { } : along + style
    pub labels: Vec<TextNode>,        // trailing strings — NO box labels, no [ ]
    pub span: Span,
}

pub struct Define {
    pub name: String, pub base: String,
    pub style: Vec<Decl>,             // defaults
    pub children: Vec<Child>,         // intrinsic children
    pub wires: Vec<Wire>,             // internal wires
    pub span: Span,
}

pub struct Rule { pub selector: Selector, pub decls: Vec<Decl>, pub span: Span }
// Selector / SelPart / Decl / Value / EndpointGroup / Endpoint / TextNode: unchanged.
```

Note `Wire` loses `WireBlock`; labels are `Vec<TextNode>` (a wire never holds a box).
`Define` loses `body: Block`. `Node` loses `block: Option<Block>`.

---

## Step 1 — The compiler speaks the new language  ✅ DONE

> Done: lexer (`[`/`]` no longer suppress newlines), the target AST (`Block`/`WireBlock`
> deleted; `Node`/`Define`/`Wire` carry `style`/`children`/`wires`/`labels`), parser rewrite
> (zone-driven `classify`, `types` set + define-ordering gone, freed type-name ids), resolve
> field updates (incl. only-sides reserved, per-label wire styling removed), desugar, samples,
> and ALL tests. `cargo test` + `fmt --check` + `clippy` green; conformance SVG byte-identical
> (proves the change is purely surface); `full_example` visually verified.

**Goal:** new syntax parses into the target AST and `lini compile` emits correct SVG.
`resolve`/`layout`/`render` logic unchanged — only field access. End the session green.

**Front end**
- `src/lexer.rs` — add `LBracket` / `RBracket` tokens (verify they're absent first).
  Everything else (`Pipe`, `DColon`, wire ops, strings) already lexes — confirm.
- `src/syntax/ast.rs` — the target AST above. Delete `Block`, `WireBlock`.
- `src/syntax/parser.rs` — rewrite around the bracket vocabulary:
  - `parse_file`: optional leading `{ }` → `stylesheet`; then canvas `Child`s; then
    wires. Error if a `{ }` stylesheet appears after an instance.
  - Stylesheet item dispatch inside the `{ }`: `--x:`→Var, `ident:`→RootDecl,
    `.name`→class Rule, `-> {`→WireDefaults, `|…|`→Rule, or Define if the bars hold
    `::` (peek inside the bars for `DColon`).
  - `parse_node`: `[ident] [typeref] [ { style } ] [ labels | [ children ] ]`. Trailing
    labels desugar to text `children` at parse time. `{ }` (style) coexists with a
    trailing label or `[ ]`; content is the trailing label XOR the `[ ]`.
  - `parse_typeref` (`|…|`): a type, optional glued `.class`es, or `.class`es alone
    (default box). Reject `::` here on the canvas (defines are stylesheet-only).
  - `parse_style` (`{ }`): decls only. `parse_children` (`[ ]`): children, then wires.
  - `parse_wire`: chain, then trailing `.class`es, then optional `{ style }`, then
    trailing string labels. Reject a `[ ]` after a wire (clear error).
  - **Delete:** the `types: HashSet`, the rule-vs-node type-set logic in `classify`,
    the "define before use" parse coupling, the trailing-label-XOR-block check (now
    label-XOR-`[ ]`). `classify` becomes zone-driven and much smaller.

**Consumers (mechanical)**
- `src/resolve/scene.rs` (`resolve_node` ~94–248): `node.block.decls`→`node.style`,
  `…children`→`node.children`, `…wires`→`node.wires`. Same for `Define`.
- `src/resolve/program.rs` (~168/184/246): iterate `Vec<SetupItem>`; `WireDefaults`
  feeds the wire cascade wherever the old `wire` selector did (`cascade.rs:42`).
- `src/resolve/types.rs` + wherever ids are validated: **remove the
  reserved-type-name-as-id rule** — types only appear in bars, so a bare `box` is a
  valid id. Keep only sides (`top/bottom/left/right`) and `|wire|` reserved.
- `src/desugar.rs` (`desugar_node` ~30–83): insert id-as-label into `node.children`;
  `desugar_wire` puts auto-`along:` into `wire.style`.

**fmt (just enough to compile)** — `src/fmt.rs` reads the AST, so update it to compile
and not crash. **Canonical output and fmt tests are Step 2** — mark `tests/fmt.rs` and
`src/fmt/tests.rs` `#[ignore]` with a `// re-enabled in PLAN Step 2` note.

**Tests & samples**
- Rewrite `samples/*.lini` to valid new syntax (parse + compile). Formatting polish is
  Step 2; here they just need to be correct.
- Rewrite `src/syntax/parser.rs` unit tests and `tests/parsing.rs` for the new grammar.
- Update `tests/resolution.rs` / `tests/conformance.rs` inputs; **regenerate** the
  conformance (compile-output) snapshots (`cargo insta accept` after eyeballing a diff).
- Add error-path tests for the new rules (§15): wire `[ ]`, decl outside `{ }`, bare
  type name, stylesheet-after-canvas, label-and-`[ ]`, glued-compound rule selector.

**Streamline wins:** delete `Block`/`WireBlock`; gut `classify`; drop `types` set +
define-ordering parse rule; drop reserved-type-id machinery.

**Done when:** `cargo build` + `cargo clippy` + `cargo fmt --all -- --check` clean;
all tests green except the `#[ignore]`d fmt tests; `lini compile` matches expected SVG
for every sample (regenerated snapshots); spot-render 2 samples to PNG and look.
**Commit.**

---

## Step 2 — `fmt` & `desugar` to canonical, + visual conformance  ✅ DONE (folded into Step 1)

> `fmt` emits the new `{ } [ ]` shape (inline style blocks, trailing labels, table-cell + sibling
> alignment, `|name::base|`, bar selectors) and is idempotent; `desugar` expands to explicit
> `[ "…" ]` / `along:`; every `samples/*.lini` is canonical (`fmt --check` a no-op) and resolves
> to identical SVG; representative samples visually checked. The only deferred polish is inline
> `[ … ]` for a single box child (currently always multi-line) — cosmetic, revisit if wanted.

**Goal:** `lini fmt` emits the canonical new style and is idempotent; `lini desugar`
is correct; samples are in canonical form; the pictures are verified.

- `src/fmt.rs` (+ `fmt/align.rs`, `fmt/trivia.rs`) — reprint the new shapes
  (`emit_file`, `emit_node`, `emit_define`, `emit_selector`, the wire emitter). Canonical
  style per SPEC §14: 2-space indent; decls grouped on one line in `{ }`; a style-only
  node collapsed onto its head (`api |box| { fill: red }`); a lone label trailing the
  head; children one per line in `[ ]`; table cells aligned (the `align.rs` column logic,
  re-keyed to `[ ]` cells); comments/blank lines preserved (`trivia.rs`, mind the extra
  indent level from the stylesheet `{ }`). Re-enable + rewrite the fmt tests.
- `src/desugar.rs` — confirm id-as-label / trailing-label → `[ "…" ]` and auto-`along:`
  expansions; rewrite `tests/desugar.rs`.
- Run `lini fmt` over every `samples/*.lini`; commit the canonicalized form; assert
  `lini fmt --check` is a no-op on all of them.
- **Visual sweep:** render the representative samples (`full_example`, `table`,
  `wires_*`, `captions`, `templates_all`) with `resvg` to PNG and read them — confirm
  the redesign didn't shift any geometry.

**Streamline win:** wire labels are always bare text now — simplify any `|plain|`-label
path in `src/layout/wires/labels.rs` / the wire renderer if one exists.

**Done when:** full `cargo test` green; `lini fmt --check` idempotent on all samples;
visual spot-checks correct. **Commit.**

---

## Step 3 — Ecosystem: docs, editor highlighter, final review  ⬜ TODO (next session)

**Goal:** everything a user sees matches the new syntax; one final adversarial pass.

- `README.md` — rewrite every example in the new syntax.
- `editors/` — update the VSCode `.lini` TextMate grammar (this highlighter is what
  started the redesign): color `|…|` as a type incl. `|name::base|`, `.class` glued in
  bars vs trailing a wire, `{ }` vs `[ ]`, `--vars`, wire ops, strings, comments.
  Verify on `samples/full_example.lini`.
- `src/serve/playground.html` and any `serve/` demo text — update embedded examples.
- Grep the tree for stragglers in old syntax (docs, comments, fixtures).
- Final review: run a `/code-review`-style pass (or the review workflow) over the whole
  diff for correctness and leftover patchwork; fix findings.
- Full `cargo test` + `cargo fmt` + `cargo clippy`. **Delete `PLAN.md`. Commit.**

---

## Reference — where things live (from the impact survey)

| Concern | Location |
|---|---|
| Canonical AST | `src/syntax/ast.rs:66-115` (Node/Block/Child/Wire/WireBlock/Define) |
| Statement dispatch | `src/syntax/parser.rs:170-205` (`classify`) |
| File / block parse | `parser.rs:209-242` (`parse_file`), `:525-562` (`parse_block`), `:404-421` (`parse_rule_block`) |
| Node / define / wire parse | `parser.rs:468-512` (`parse_node`), `:423-438` (`parse_define`), `:566-693` (`parse_wire`/`parse_wire_block`) |
| Type sigil / `::` | lexer `Pipe`, `DColon`; `parse_type_use` `parser.rs:514-522` |
| resolve consumes block | `src/resolve/scene.rs:136/170/182`; stylesheet `program.rs:168/184/246`; wire defaults `cascade.rs:42` |
| reserved-type ids | `parser.rs` `BUILTIN_TYPES` + resolve id checks (remove) |
| fmt | `src/fmt.rs:74-126/162-192/265-437`, `fmt/align.rs`, `fmt/trivia.rs:18-81` |
| desugar | `src/desugar.rs:30-83` (`desugar_node`), `:87` (`desugar_wire`) |
| wire labels (geometry) | `src/layout/wires/labels.rs` — routing unchanged; only the box-label path simplifies |

Routing (`src/layout/wires/**`, ~6k LOC) and `render` are **not** touched beyond AST
field renames — the geometry contract is unchanged.

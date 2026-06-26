# Lini Syntax Refactor (SPEC v0.10) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the engine in line with the rewritten `SPEC.md` — identity in the bars (`|type#id|`), smart labels, side via `:`, floating class, juxtaposed selectors, and `->`/`-->`/`--->`/`~>` link lines.

**Architecture:** The change is **front-loaded onto the parse pipeline**. Lex → parse → desugar → resolve produce a syntax-agnostic IR; **layout, routing, and render consume that IR and need no changes** (a forced side is still a `Side` enum, a class is still a `.lini-style-*` class — only how they're *written* changed). So the blast radius is: `lexer` → `syntax/ast` → `syntax/parser` → `desugar` → `resolve` (cascade tiers + selector match) → `fmt` → `lint`, then `samples`, `tests`, `editors`, `docs`.

**Tech Stack:** Rust, `insta` snapshot tests, `resvg` for visual verification. `SPEC.md` and `LINKING.md` are the normative contract — every task cites the section it implements and is correct iff it matches the spec.

## Global Constraints

- **No `unsafe`.** (AGENTS.md)
- **One mechanism per problem; no parallel implementations.** Node text-label and link label lower through **one** shared smart-label function; node and link share **one** tail-parser (`"label"? .class…? { style }? [ … ]?`). (AGENTS.md)
- **One concept per file; split past ~500 LOC.** (AGENTS.md)
- Before any push: `cargo fmt`, then `cargo test` and `cargo clippy` clean. (AGENTS.md / CI)
- `insta` snapshots for output-shaped code; one sample per feature in `samples/`; **verify SVG visually** (render to PNG with `resvg` and read it). (AGENTS.md)
- This is a **breaking** pre-release change — do not preserve back-compat or add migration shims. Old forms become errors with helpful messages (SPEC §15).
- The branch `spec-v0.10` already exists with `SPEC.md` + `LINKING.md` refactored. Work on it.

**The nine syntax changes (the law — SPEC is authoritative):**

1. Identity in bars: `|type#id|`, `|box|`, `|#id|` (≥ one of type/id). No `id |type|`. A bare leading name on the canvas is invalid (link endpoint only). (§1, §3)
2. `#id` declares/selects an id (`|box#cat|`, `#cat { }`), referenced **bare** in links (`cat -> b`). `#hex` is a colour only in a value. (§2, §4)
3. Side via `:` on an endpoint: `a:left -> b:top`. Path into children stays `.` (`kitchen.bowl`). `top/bottom/left/right` are no longer reserved. (§9, §18)
4. Smart label: one `"label"` right after the head, before class/style; lowered per type — box/shapes → centred text, group/table → caption, icon/sign → symbol, link → route label. **No default label** — a bare node is empty. A link to a name not present in its scope (root **or any body**) is a desugar that adds labelled stub boxes (`a -> b` → `|box#a| "a"` + `|box#b| "b"`). `""` is just an empty string. Coexists with `[ ]` (prepended). Takes no style; styled labels go in `[ ]`. One inline label; 2+ in `[ ]`. (§3, §9)
5. Class floats after head (and label), spaced — identical node/link, never in bars. (§3, §4, §9)
6. Selectors are juxtaposed units, space = descendant: `|box|`, `.hot`, `#hero`, `|table| |box|`, `.sidebar |box|`. No `|table box|` one-bars form. (§4)
7. Link lines: `->` `-->` `--->` `~>`. No `..`/`..>`. (§9)
8. Tail identical node/link: `head "label"? .class…? { style }? [ content ]?`. (§9)
9. Strings are trimmed of leading/trailing whitespace (`" ABC "` → "ABC"). (§2)

---

## Task 0: Baseline & guardrail

**Files:** none (read-only).

- [ ] **Step 1: Confirm branch and capture the current test state**

```bash
cd /Users/abbas/workspace/lini
git branch --show-current        # expect: spec-v0.10
git status                       # expect: SPEC.md, LINKING.md modified (the refactor)
cargo test 2>&1 | tail -30       # baseline — record pass/fail counts
```

Expected: green today. Many snapshot/conformance tests **will** go red as syntax changes land; that is expected — they are re-accepted after visual review in Task 8/9. Note the current pass count so regressions outside the intended surface are visible.

- [ ] **Step 2: Commit the spec refactor (already on disk) as the plan's anchor**

```bash
git add SPEC.md LINKING.md SYNTAX-0.10.md
git commit -m "docs: SPEC v0.10 — identity in bars, smart label, :side, juxtaposed selectors, -/--/--- links"
```

---

## Task 1: Lexer — `#` hash token, `:`/`::`, and `-`/`--`/`---` link lines

**Files:**
- Modify: `src/lexer.rs`
- Test: `src/lexer.rs` (inline `#[cfg(test)]`) or `tests/parsing.rs` lexer cases

**Interfaces:**
- Produces: a `TokKind::Hash(String)` token = `#` + the run of `[A-Za-z0-9_-]` after it (raw, undecided). The parser interprets it: a **colour** in a value position (validate as 3/4/6/8 hex digits) or an **id** in bars / at a rule head (validate as an ident). This replaces the old `TokKind::Hex(String)`.
- Produces: `LineStyle::Dotted` now lexes from `---` (three `-`), `Dashed` from `--`, `Solid` from `-`, `Wavy` from `~`. `..` is no longer a line.

**Why a single `Hash(String)`:** `#abc` is a valid colour *and* a valid id; only context decides. A context-free lexer cannot. Emitting one raw `Hash(String)` and letting the (context-aware) parser validate is the one-mechanism fix; it also makes `#hero { }` id-selectors lex outside bars (the old `lex_hex` would reject `#hero` as bad hex).

- [ ] **Step 1: Write failing lexer tests**

Add tests asserting:
- `#fff`, `#ffaa00cc`, `#cat`, `#load-balancer` all lex to a single `Hash("fff")` / `Hash("ffaa00cc")` / `Hash("cat")` / `Hash("load-balancer")`.
- `->` → `LinkOp{solid, no markers}`; `-->` → dashed; `--->` → dotted; `~>` → wavy.
- `a:left` lexes as `Ident("a") Colon Ident("left")` (no new side token; `:` is `Colon`).
- `--brand` still lexes as `RawCssVar("brand")` (var beats dashed line when followed by ident-start).
- `..` lexes as `Dot Dot` (no longer a link op).
- `" ABC "` lexes to `String("ABC")` (value trimmed); `"a b"` keeps the inner space.

```bash
cargo test --lib lexer 2>&1 | tail -20   # or the module path you place them in
```
Expected: FAIL (Hash doesn't exist; `---` not dotted; `..` still a link op).

- [ ] **Step 2: Replace `Hex` with `Hash(String)`**

In `TokKind` (top of `src/lexer.rs`): remove `Hex(String)`, add `Hash(String)`. Replace `lex_hex` with `lex_hash`: consume `#`, then a run of `is_ident_continue` chars (`[A-Za-z0-9_-]`), store the raw run as `Hash(run)`. Do **not** validate length/hex here — the parser does, by context.

- [ ] **Step 3: Update the link-line lexer for `-`/`--`/`---`**

In `consume_line`: match longest-first — `---` → `Dotted`, `--` → `Dashed` (with the existing `--name` guard: if the chars after `--` are ident-start, it is a var, not a line — extend the guard so `---` also yields a line only when not a var; `---name` is `---` line then `name`), `-` → `Solid`, `~` → `Wavy`. Remove the `..` → `Dotted` branch.

In the `b'.'` arm of `run()`: remove the `peek(1) == Some(b'.')` → `lex_link_op` branch (so `..` becomes two `Dot`s). Keep `.` + digit → number, else `Dot`.

In `is_link_line_start`: drop `b'.'`, keep `b'-' | b'~'` (so `*` is a dot-marker only before `-`/`~`).

- [ ] **Step 3b: Trim string values**

In `lex_string`, after the value is built (escapes resolved), store it **trimmed** — `TokKind::String(value.trim().to_string())` — the span still covers the quotes for errors. This trims every string (labels and text leaves): `" ABC "` → `String("ABC")`; inner spaces are kept (SPEC §2).

- [ ] **Step 4: Run the lexer tests**

```bash
cargo test --lib lexer 2>&1 | tail -20
```
Expected: PASS. (Parser/AST code referencing `TokKind::Hex` will now fail to compile — fixed in Tasks 2–3; that is expected.)

- [ ] **Step 5: Commit**

```bash
git add src/lexer.rs
git commit -m "lex: #→Hash(String) token; ---/--/-/~ link lines; drop .. dotted; trim strings"
```

---

## Task 2: AST — identity in bars, smart label, selector units, link tail

**Files:**
- Modify: `src/syntax/ast.rs`
- (No standalone test; the parser tests in Task 3 exercise these shapes.)

**Interfaces (Produces — later tasks depend on these exact names):**
- `Node { id: Option<String>, ty: Option<String>, classes: Vec<String>, label: Option<TextNode>, style, style_span, children: Vec<Child>, links: Vec<Link>, span }` — `id`/`ty` now come from the bars; `label` is the smart-label head string (lowered per type at desugar), `None` when absent.
- `Selector { units: Vec<SelUnit> }` where `enum SelUnit { Type { name: String, id: Option<String> }, Class(String), Id(String) }`. (Replaces `parts: Vec<SelPart>`.)
- `Link { chain, op, classes, label: Option<TextNode>, labels: Vec<TextNode>, style, style_span, span }` — `label` is the head string (unstyled), `labels` are the `[ ]` leaves; desugar concatenates `label` ++ `labels` for `along:`.
- `Endpoint { path: Vec<String>, side: Option<Side>, span }` — unchanged shape; `side` now parsed from `:side`.

- [ ] **Step 1: Edit `Node`** — add `label: Option<TextNode>`. Keep `id`, `ty`, `classes`, `children`, `links`. Update the doc comment: id/type live in the bars; `label` is the smart label.

- [ ] **Step 2: Edit `Selector`/`SelPart`** — replace `Selector { parts }` and `enum SelPart { Type, Class }` with `Selector { units }` and `enum SelUnit { Type { name, id }, Class, Id }` (above).

- [ ] **Step 3: Edit `Link`** — add `label: Option<TextNode>` (head, unstyled) alongside `labels: Vec<TextNode>` (`[ ]`).

- [ ] **Step 4: Compile check (will fail in parser/desugar/resolve — expected)**

```bash
cargo build 2>&1 | grep -E 'error\[|error:' | head -30
```
Expected: errors only in `syntax/parser.rs`, `desugar/*`, `resolve/*`, `fmt*`, `lint.rs` (the consumers, fixed next). No errors elsewhere confirms the IR boundary held.

- [ ] **Step 5: Commit**

```bash
git add src/syntax/ast.rs
git commit -m "ast: id in bars + smart-label field; selector units (type#id/class/id); link head label"
```

---

## Task 3: Parser — identity, head label, juxtaposed selectors, `:side`, statement kinds

**Files:**
- Modify: `src/syntax/parser.rs`
- Test: `src/syntax/parser.rs` `#[cfg(test)]` (rewrite the existing cases to the new syntax + add new ones)

**Interfaces:**
- Consumes: Task 1 tokens (`Hash`, `Colon`, link ops), Task 2 AST.
- Produces: `parse(tokens) -> Result<File>` accepting the new grammar (SPEC §16).

- [ ] **Step 1: Rewrite the parser tests to the new syntax (failing)**

Port every existing test and add the new shapes. Key assertions:
- `|box#server|\n` → instance, `ty=Some("box")`, `id=Some("server")`, `label=None`.
- `|#cat|` → `ty=None`, `id=Some("cat")`.
- `|box| "Load balancer"` → `ty=Some("box")`, `id=None`, `label=Some("Load balancer")`.
- `|box#cat| "Cat" .hot.loud { fill: red } [ |badge| "x" ]` → label, two classes, style, one child.
- `|box#cat| .hot "Cat"` → **error** "head label takes no … / one inline label position" *(label must precede class)* — i.e. a string after a class is rejected (label slot is before classes).
- `|box#cat| "a" "b"` → **error** "one inline label — put two or more in a '[ ]'".
- `|box.hot|` → **error** "a class follows the bars".
- `| |` / `||` → **error** "needs a type or an '#id'".
- Selectors: `|box| { }`, `.hot { }`, `#hero { }`, `|table| |box| { }`, `.sidebar |box| { }`, `|table#main| |box| { }` parse to the right `SelUnit` sequence.
- Links: `a:left -> b:top "watches" .loud { along: 0.5 }` → endpoints with sides, head `label`, class, style. `a -> b -> c` chain; `a & b -> c` fan; `a ---> b` dotted; `a -> b [ "x" "y" ]` two `[ ]` labels.
- Canvas classification: bare `cat\n` → **error** "a node leads with bars …"; `cat -> dog` → link; `"hi"` → text node.
- Stylesheet classification: `#hero { }` → id rule; `|treat::box| { }` → define; `box { }` (bare) → **error** "a type only appears in bars".

```bash
cargo test --lib syntax::parser 2>&1 | tail -25
```
Expected: FAIL (parser not updated).

- [ ] **Step 2: Identity parser** — replace `parse_type` with `parse_identity() -> (Option<String> ty, Option<String> id)`: expect `|`; then a `Hash(s)` (→ id, validate `s` is an ident; `ty=None`) **or** an `Ident` (→ type; then optional glued `Hash(s)` → id; reject a glued `Dot` = class-in-bars with the existing error; reject `DColon` inside an instance = "a define belongs in the stylesheet"); reject empty `| |`; expect closing `|`. Reject `|link|`/`|node|` as before.

- [ ] **Step 3: Node tail (shared with links)** — write `parse_tail()` returning `(label: Option<TextNode>, classes, style, style_span)` for the **head string + classes + style** portion, enforcing: at most one head string (2nd string → "one inline label"); a `{` immediately after the head string → the *node's* block (not the label's), so a head label never carries style; a class appearing *before* the label slot is the head order (label, then classes) — a string after a class → error. Then content (`[ ]`) is parsed by the caller. `parse_node` = `parse_identity` + `parse_tail` + `opt_children`. `parse_link` = endpoints/ops + `parse_tail` + `parse_label_block`. **One tail parser, both callers** (Global Constraint).

- [ ] **Step 4: Selector parser** — `parse_selector()` reads space-separated units until `{`: a `|…|` unit via `parse_identity` (→ `SelUnit::Type{name,id}`, or `SelUnit::Id` when type is None), a `Dot`+ident → `SelUnit::Class`, a `Hash(s)` → `SelUnit::Id`. Reject a glued type+class inside one `|…|` (existing error). `parse_rule` now dispatches on the leading token (`|`, `.`, `#`).

- [ ] **Step 5: Endpoint `:side`** — in `parse_endpoint`, after the dotted path, if `Colon` then expect a side ident (`Side::parse`); unknown → "':X' is not a side". Sides are no longer peeled from the dotted path (a final `.left` is now a *child* named left).

- [ ] **Step 6: Statement classification** — `classify_body`: `Pipe` → Node; `String` → text; `Ident` → Link only if followed by a link-op / `&` / glued `Dot` (path), else the bare-node error. `classify_setup`: `Pipe` → rule/define; `Dot` → class rule; `Hash` → id rule; `RawCssVar` → var; `Ident`+`Colon` → decl, else the bare-type error.

- [ ] **Step 7: Value parser handles `Hash` as a colour** — in `parse_value`, a `Hash(s)` → `Value::Hex(s)` after validating `s` is 3/4/6/8 hex digits (else "invalid hex color '#…'"). (Keeps `fill: #f80` working.)

- [ ] **Step 8: Run parser tests**

```bash
cargo test --lib syntax::parser 2>&1 | tail -25
```
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add src/syntax/parser.rs src/syntax/ast.rs
git commit -m "parse: |type#id| identity, smart-label head, juxtaposed selectors, :side, #id values"
```

---

## Task 4: Desugar — smart label per type, juxtaposed-selector lowering, auto-create stub label

**Files:**
- Modify: `src/desugar/labels.rs`, `src/desugar/classes.rs`, `src/desugar/mod.rs` (and `scene.rs`/`types.rs` as needed)
- Test: `tests/desugar.rs` (+ the `samples/desugar.lini` round-trip)

**Interfaces:**
- Consumes: the Task 2/3 AST.
- Produces: the lowered primitive tree where the smart label has become concrete content and selectors are `.lini-*` class chains. One shared `lower_label(node_or_link, kind)` used by both node and link paths.

- [ ] **Step 1: Failing desugar tests** — assert the lowering, one per type:
  - `|box#cat|` (no label) → **no** text child (empty box).
  - `|box#cat| ""` → no text child (`""` is an empty string, same as no label).
  - `a -> b` (a, b undeclared) → auto-creates `|box#a| "a"` + `|box#b| "b"` (the desugar adds the labels).
  - **Body scope:** `|group#g| [ x -> y ]` (x, y undeclared in the body) → `x`/`y` created as the group's own children "x"/"y" — auto-create runs in **any** scope, not just root (SPEC §3).
  - **Shadow warning:** a bare id auto-created while a same-named node exists deeper → still drawn, plus a warning (SPEC §3/§15).
  - `|box#lb| "Load balancer"` → text child "Load balancer".
  - `|group#k| "Kitchen" [ child ]` → a `|caption|` child "Kitchen" **prepended**, then `child`.
  - `|icon| "heart"` → `symbol: heart` set; no text child. `|icon| "heart" { symbol: x }` → **error** "symbol is its label or 'symbol:', not both". `|icon| "x" [ "3" ]` → symbol x + text child "3".
  - `a -> b "watches"` → label list `["watches"]`; `a -> b "w" [ "x" ]` → `["w","x"]` **plus a "link labels split" warning** (SPEC §15); auto-`along:` unchanged.
  - Every type shows only the label it's given; the `a -> b` desugar adds the labels to auto-created boxes.

```bash
cargo test --test desugar 2>&1 | tail -25
```
Expected: FAIL.

- [ ] **Step 2: Rewrite `labels.rs`** — replace `label_child_for` with `lower_label`: given the node's resolved base/type kind, place `label`:
  - box-like (block/box/rect/oval/hex/slant/cyl/diamond/poly/path/line/note and their derivations) → a centred `TextNode` child from the label, **prepended** to `children`. `label == None` → **nothing**; `""` → nothing. (`lower_label` places only an explicit label; an auto-created link stub's "x" label is added by the implicit-node desugar — SPEC §17.)
  - group/table-like → inject a `|caption|` child carrying the label text, prepended.
  - icon/sign → set `symbol` from the label (error if `{ symbol }` already set); never a text child; id is not a symbol.
  - link → push label text onto the label list (head label first), feeding existing auto-`along:`.
  Keep `auto_along` as-is. This is the single shared lowering (Global Constraint: node text-leaf and link label call one function).

- [ ] **Step 3: Selector lowering** — where a descendant rule was rewritten (`classes.rs`/`scene.rs`), map each `SelUnit` to its `.lini-*` form: `Type{name}` → `.lini-<name>` (the generated type class), `Type{name,id}` → `.lini-<name>` + an id match on `data-id`, `Class(c)` → `.lini-style-<c>`, `Id(i)` → an id match. Descendant = ancestor-chain match over the unit sequence (juxtaposed). `|table| |box|` → match `.lini-box` whose ancestor chain has `.lini-table` (per §17).

- [ ] **Step 4: Run desugar tests + the desugar round-trip**

```bash
cargo test --test desugar 2>&1 | tail -25
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/desugar/
git commit -m "desugar: smart label per type (text/caption/symbol/link-label), juxtaposed-selector lowering"
```

---

## Task 5: Resolve — id-rule cascade tier + juxtaposed selector matching

**Files:**
- Modify: `src/resolve/cascade.rs`, `src/resolve/scene.rs` (selector match), `src/resolve/links.rs` (endpoint side)
- Test: `tests/resolution.rs`

**Interfaces:**
- Consumes: lowered tree from Task 4.
- Produces: resolved IR with the new specificity order: type cascade < descendant < class < **id** < instance block (SPEC §12).

- [ ] **Step 1: Failing resolution tests** — `#hero { fill: gold }` colours only node `hero`; an instance block beats it; `|table| |box| { }` hits boxes inside a table but not elsewhere; `a:left -> b` forces a's left side (endpoint resolves with `Side::Left`).

```bash
cargo test --test resolution 2>&1 | tail -25
```
Expected: FAIL.

- [ ] **Step 2: Add the id tier** — in `cascade.rs`, insert id-rule application between class rules and the instance block. In `scene.rs`, implement juxtaposed descendant matching over `SelUnit`s against the ancestor chain (type/class/id units). Confirm `links.rs` endpoint resolution consumes the already-parsed `Side` unchanged (no logic change expected — assert with a test).

- [ ] **Step 3: Run resolution tests**

```bash
cargo test --test resolution 2>&1 | tail -25
```
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/resolve/
git commit -m "resolve: id-rule specificity tier + juxtaposed descendant matching"
```

---

## Task 6: Lint & error messages

**Files:**
- Modify: `src/lint.rs`, `src/error.rs` (message strings only)
- Test: `tests/` wherever error-message assertions live (search `did you mean`, `follows the bars`)

**Interfaces:** strings must match SPEC §15 exactly (they are part of the contract).

- [ ] **Step 1: Failing tests** for the new/changed messages: empty bars, bare node on canvas, **bare type in the stylesheet**, class in bars, symbol-twice, two head labels, **label after a class**, styled head label, **invalid id (digit-leading)**, unknown side, glued selector unit, and the **"link labels split" / "auto-create shadows a node" warnings**. Remove the dead "Reserved identifier (side)" message and the old "Label and `[ ]` both" message (both now allowed/changed).

```bash
cargo test 2>&1 | grep -iE 'error message|lint' | tail -20
```

- [ ] **Step 2: Update messages** to the §15 table verbatim. Drop the side-reserved error (sides are free ids now).

- [ ] **Step 3: Run**

```bash
cargo test --test parsing --test conformance 2>&1 | tail -25
```
Expected: error-assertion tests PASS (snapshot/render tests still red — Task 8/9).

- [ ] **Step 4: Commit**

```bash
git add src/lint.rs src/error.rs
git commit -m "errors: messages for v0.10 syntax (empty bars, bare node, :side, symbol-twice)"
```

---

## Task 7: Formatter

**Files:**
- Modify: `src/fmt.rs`, `src/fmt/align.rs`, `src/fmt/trivia.rs`
- Test: `src/fmt/tests.rs`, `tests/fmt.rs`

**Interfaces:** canonical output per SPEC §14 `fmt` description: `|type#id|`, head label first (`|box#api| "API"`), juxtaposed selectors, `->`/`-->`/`--->`/`~>`, `:side`, table-cell alignment preserved.

- [ ] **Step 1: Failing fmt tests** — feed canonical and messy inputs, assert idempotent canonical output: `|box#api|"API"{fill:red}` → `|box#api| "API" { fill: red }`; `a->b"x".loud` → `a -> b "x" .loud`; selector `|table||box|{}` → `|table| |box| { }`; `a:left->b:top` spacing.

```bash
cargo test fmt 2>&1 | tail -25
```
Expected: FAIL.

- [ ] **Step 2: Update the printer** — node head: `|type#id|`, then a lone label, then classes, then a one-line `{ }` when it fits, then `[ ]` children one per line. Link head: endpoints with `:side`, ops, then the same tail. Selector printer joins units with single spaces. Keep table-cell column padding (`align.rs`) — it now pads after `|table#id| "Caption"`.

- [ ] **Step 3: Run + idempotence check**

```bash
cargo test fmt 2>&1 | tail -25
for f in samples/*.lini; do cargo run -q -- fmt --stdout "$f" | cargo run -q -- fmt --stdout - >/dev/null || echo "NONIDEMPOTENT: $f"; done
```
(Run the second line after Task 8 rewrites the samples.) Expected: tests PASS; no non-idempotent files.

- [ ] **Step 4: Commit**

```bash
git add src/fmt.rs src/fmt/
git commit -m "fmt: canonical v0.10 — |type#id|, label-first, juxtaposed selectors, ---/:side"
```

---

## Task 8: Rewrite every sample to v0.10

**Files:**
- Modify: all of `samples/*.lini` (hello, flow, links, links_simple/medium/hard, hero, layout, shapes, icons, text, styles, themes, palette, gradient, templates, desugar, mermaid_fast, pcb, pcb_fail)
- Test: visual (resvg) — see Step 3.

**Interfaces:** every sample parses and renders under the new engine; each still demonstrates its feature.

- [ ] **Step 1: Convert each file** by the mechanical rules: `id |type|` → `|type#id|`; `id |type| "L"` → `|type#id| "L"`; `|type| "L"` (anon) stays; sides `a.left` → `a:left`; dotted `..>` → `--->`; descendants `|table box|` → `|table| |box|`; `|caption| "X"` as a group's first child → fold to the group's label where it reads better (keep explicit where a sample is *showing* `|caption|`, e.g. shapes/templates); icon `{ symbol: x } "label-text"` → `|icon| "x" [ "label-text" ]` or keep `{ symbol }` + `[ ]`. Use `lini fmt` to normalize after hand-editing.

- [ ] **Step 2: Each sample compiles**

```bash
for f in samples/*.lini; do cargo run -q -- "$f" -o /dev/null 2>&1 | sed "s|^|$f: |"; done
```
Expected: no errors (except `pcb_fail.lini`, which is a deliberate stray-link/error sample — confirm it fails the *intended* way).

- [ ] **Step 3: Visual verification (required, AGENTS.md)** — render representative samples to PNG and read them:

```bash
mkdir -p /tmp/lini-verify/png   # a scratchpad for the rendered PNGs
for f in hello flow links_medium hero layout shapes icons text; do
  cargo run -q -- "samples/$f.lini" -o "/tmp/.../$f.svg" && resvg "/tmp/.../$f.svg" "/private/tmp/.../scratchpad/png/$f.png"
done
```
Open each PNG and confirm: labels present and placed (centred text / captions / icon symbols), links solid/dashed/dotted/wavy correct, sides forced where written. Fix any sample that regressed.

- [ ] **Step 4: Commit**

```bash
git add samples/
git commit -m "samples: rewrite all to v0.10 syntax; visually verified"
```

---

## Task 9: Tests & snapshots — regenerate and verify

**Files:**
- Modify: `tests/*.rs` (hello, cli, oracle, linking, linking_sweep, rendering, conformance), `tests/snapshots/*`
- Test: the whole suite.

- [ ] **Step 1: Update hand-written test inputs** in each `tests/*.rs` to v0.10 (search for `|` usages and `->`). `tests/conformance.rs` snapshots one baked SVG per sample — its inputs are the rewritten samples from Task 8.

- [ ] **Step 2: Review and accept snapshots** — never blind-accept:

```bash
cargo test 2>&1 | tail -20            # see what changed
cargo insta review                    # inspect each diff; accept only intended changes
```
For conformance SVGs, spot-render the new baked SVG (Task 8 Step 3 method) before accepting, so a wrong snapshot isn't frozen.

- [ ] **Step 3: Full green + lints**

```bash
cargo fmt --all
cargo test 2>&1 | tail -10
cargo clippy --all-targets 2>&1 | tail -10
```
Expected: all pass, clippy clean, fmt no diff.

- [ ] **Step 4: Commit**

```bash
git add tests/
git commit -m "tests: update inputs + regenerate snapshots for v0.10 (reviewed)"
```

---

## Task 10: Editors, README, playground

**Files:**
- Modify: `editors/vscode/syntaxes/lini.tmLanguage.json`, `editors/vscode/language-configuration.json`, `README.md`, `src/serve/playground.html` + `src/serve/single.html` (any embedded `.lini` examples)

- [ ] **Step 1: tmLanguage grammar** — update patterns: identity `\|[a-z]+(#[\w-]+)?\|` and `\|#[\w-]+\|`; `#id` selector/id; `:side` (`:(top|bottom|left|right)\b` after an endpoint); link ops `-{1,3}>|<-{1,3}|~>|<->|...`; smart-label string after a head; class `\.[\w-]+`. Remove `..`/`..>` and the old `id |type|` highlighting. Confirm in VS Code (or by reading the JSON against the new token set).

- [ ] **Step 2: README** — rewrite every `.lini` snippet to v0.10 (it leads with examples). Align the icons/links/shapes sections with the rewritten samples. Re-generate any embedded asset references if their source `.lini` changed shape.

- [ ] **Step 3: Playground HTML** — update any seeded example `.lini` text to v0.10 so the live preview opens valid.

- [ ] **Step 4: Commit**

```bash
git add editors/ README.md src/serve/
git commit -m "editors+docs: v0.10 syntax in tmLanguage, README, playground"
```

---

## Task 11: Final sweep

- [ ] **Step 1: Grep for stragglers** across the whole repo (not just SPEC):

```bash
grep -rnE '\.\.>|[a-z0-9_]+ \|[a-z]+\| ' --include='*.rs' --include='*.lini' --include='*.md' --include='*.json' . | grep -v target | head
grep -rnE '\b[a-z]+\.(left|right|top|bottom)\b' --include='*.lini' --include='*.rs' . | grep -v target | head
```
Investigate every hit; convert or justify.

- [ ] **Step 2: Whole-suite green, then hand to the user for the `main` merge** (AGENTS.md: defer pushing to `main`).

```bash
cargo fmt --all && cargo test && cargo clippy --all-targets
```

- [ ] **Step 3: Summarize** the diff (`git log --oneline main..spec-v0.10`, `git diff --stat main..spec-v0.10`) for review.

---

## Self-Review (run before execution)

- **Spec coverage:** §1–§4 → Tasks 2–5; §7 icons / smart label → Task 4; §9 links (`:side`, ops, head label) → Tasks 1/3/4; §15 errors → Task 6; §14 fmt → Task 7; §16 grammar → Tasks 1–3; §13 SVG output → unchanged (verified by Task 9 snapshots). §5/§6 layout & positioning, §10–§13, §17–§20 are behavior the IR already produces — covered by snapshot/visual verification (Tasks 8–9), not new code.
- **IR boundary:** layout/route/render untouched — Task 2 Step 4 proves it (compile errors confined to front-end). If errors appear in `layout/`/`render/`, a syntax detail leaked into the IR; stop and reconcile with the spec before continuing.
- **One mechanism:** the shared tail-parser (Task 3 Step 3) and shared `lower_label` (Task 4 Step 2) are explicit, per AGENTS.md "no parallel implementations."
- **Type consistency:** `SelUnit` (Task 2) is consumed by parser (Task 3), desugar (Task 4), resolve (Task 5) under the same name. `Node.label` / `Link.label` produced in Task 2, parsed in Task 3, lowered in Task 4.
- **No silent truncation:** the visual-verification step (Task 8 Step 3) is mandatory, not optional — a passing snapshot proves *stability*, not *correctness*; only a rendered PNG proves the diagram is right.

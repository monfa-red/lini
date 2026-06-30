# PLAN — implement `layout: sequence`

Implementation plan for the sequence-diagram feature specified in **[SPEC.md §10](SPEC.md)**.
This file is scaffolding — delete it when the feature lands. It is written so any single
step can be executed in a fresh session: each step lists its files, changes, tests, and a
"done when" gate, and every step leaves `cargo build && cargo test` green.

> **Re-orient (every session):** read `SPEC.md` §10 (Sequences) and §17 (Grammar) and §18
> (Implementer Algorithm); skim `CHARTS.md` §1–§3, §15 (the sibling layout-owning engine this
> mirrors); read this whole PLAN; run `cargo test` to confirm the baseline is green; check
> the **Progress** box below to see what's done. Branch: `sequence`.
> **Stale-binary trap:** the dev binary can run stale code — `cargo clean -p lini` before any
> render-verify (see `memory/cargo-fresh-staleness.md`).

## Progress

- [x] **Step 1 — Grammar relaxation (parser + fmt)** ✓ (parser both sites relaxed; `fmt` source-order merge; 427 lib tests + clippy + fmt clean)
- [x] **Step 2 — Sequence types + dispatch + participants/lifelines** ✓ (6 types + defaults; `prim` promoted to shared `layout::prim`; root + node forms render headers + lifelines; tests + clippy + fmt clean; visually verified)
- [ ] Step 3 — Messages + the link-partition / wiring-strategy seam
- [ ] Step 4 — Activations
- [ ] Step 5 — Frames (`loop`/`opt`/`alt`/`else`) + notes
- [ ] Step 6 — Samples, snapshots, render-verify, polish, acceptance

---

## 1. What we're building

A sequence is a **layout-owning container** — exactly like `chart`/`pie`: it is intercepted
in `layout_inst` *before* the generic flow/grid path, reads its whole subtree, and **lowers
to primitive `PlacedNode`s** (lifelines/arrows/frames/notes → `|line|`/`|block|`/text). The
renderer, cascade, palette, theming, and `--bake-vars` are reused unchanged.

Two things make it different from charts, and they are the whole job:

1. **Its links are content.** A message is an ordinary link (`a -> b "x"`), but in a sequence
   scope its *order is time* and it is drawn by the sequence layout, **not** the orthogonal
   router. So the router must be told to skip these links, and the layout must claim them.
2. **Nodes and links interleave.** A frame is a node whose `[ ]` holds messages (links), so the
   parser's "children before links" rule is relaxed to *source order preserved* (SPEC §17).

Everything else (participants, frames, notes, activations) is nodes + the smart label, already
in the language.

## 2. Architecture & the pipeline

```
lex → parse → desugar → resolve → layout → route → render
        │        │          │         │        │
   (Step 1)  (Step 2)   (Step 3)  (Steps    (Step 3
   relax     new types  tag link   2-5)     skips seq
   ordering  + defaults  scope     sequence  links)
                                    engine
```

- **parse** (`src/syntax/parser.rs`): relax the two ordering checks; keep `(Vec<Child>,
  Vec<Link>)` separate — each already in source order; the sequence engine recovers the
  *interleave* by sorting on `Span` (Step 1, no AST change).
- **desugar** (`src/desugar/`): register `sequence`/`note`/`loop`/`opt`/`alt`/`else` as
  bundles over `|block|` with the SPEC §10 Defaults table (Step 2).
- **resolve** (`src/resolve/`): tag each `ResolvedLink` with its **scope** (the container it
  was authored in) so layout/route can ask "is this a sequence message?" (Step 3). Make a
  sequence frame's `[ ]` **transparent** to endpoint resolution (Step 5).
- **layout** (`src/layout/sequence/`, new): the engine — `is_sequence` interception in
  `layout_inst`, mirroring `is_chart` (Steps 2–5).
- **route** (`src/layout/links/`): partition links — sequence-scope messages are excluded
  from `route_links`; the seam is documented for future `straight`/`curved` (Step 3).
- **render**: **unchanged** — sequence lowers to primitives. Each visual step verifies this.

### The link-partition / wiring-strategy seam (the "linking refactor")

The user-facing model (SPEC §10): each scope has a **wiring strategy** that realises its
links. Implementation is a *dispatch*, not a second router:

| Strategy | Realised by | Status |
|---|---|---|
| `orthogonal` | `links::route_links` (the LINKING.md contract) | built |
| `sequence` | the sequence **layout** (lowers messages to `|line|` arrows) | this feature |
| `straight` / `curved` | future graph/mindmap layouts | **comment-only scaffold** |

Concretely (Step 3): after resolve, `program.links` is partitioned by each link's scope:
links whose scope container has `layout: sequence` go to the sequence engine; the rest go to
`route_links`. Add a `links::strategy(scope_layout) -> Strategy` helper and a doc-comment block
naming the future `Straight`/`Curved` arms (no code — the user asked to scaffold, not build,
graph routing). Keep linking isolated from rendering, as today.

## 3. Locked design decisions (from SPEC §10 — do not re-litigate)

- `layout: sequence` engine; root `{ layout: sequence }` or `|sequence|` template.
- Participants = any box, top row, declaration order; lifelines share a common foot;
  undeclared endpoints auto-create participants (reuse implicit-node rule).
- Messages = links read as time: `->` call · `-->` return · `~>` async · `a->a` self.
  Paint maps `link*→stroke*`, operator end-marker → `marker-end`. `along:`/forced sides/
  `routing` ignored on a message.
- Grammar relaxed: nodes/links interleave in source order (general; non-sequence unaffected).
- Activations: implicit, sequence-global LIFO stack; `->` opens on target, `-->` from target
  closes its top; self/async open none; orphan return draws only its arrow; `activation: none`
  off.
- Frames = nodes whose `[ ]` holds messages (`loop`/`opt`/`alt`); `alt` compartments split by
  `|else| "guard"`; **a frame opens no scope** (messages resolve to the sequence's participants).
- Notes = `|note| "x" { over: a }` / `{ over: a b }` / `{ left: a }` / `{ right: a }`.
- No new sigils, no new primitives (actor = `|icon|`).

### Micro-decisions deferred to implementation (lock via snapshot tests)

These are render constants the SPEC leaves open (SPEC §18: "implementations may differ if the
observable output matches"). Pick sensible values, then freeze them with `insta`:

- Row pitch (default `gap` row), header→first-row gap, lifeline foot margin.
- Activation bar width and nesting offset.
- Frame inset past the lifelines it spans, frame title-tab height, whether `|else|` consumes a row.
- Self-message hook size and rows consumed.
- Note box size / offset from its lifeline; `--group-fill`-tinted note vs. a sticky look.

---

## Step 1 — Grammar relaxation (parser + fmt)

**Goal:** nodes and links may interleave in any body and at the root, in source order; the
formatter round-trips it. Standalone, useful on its own; unblocks frames.

**Files:** `src/syntax/parser.rs`, `src/fmt/` (and `src/fmt/tests.rs`), `src/syntax/ast.rs`
(inspect only — expect no change).

**Changes:**
1. `parser.rs:~230` (root canvas/links) and `parser.rs:~669` (`parse_children` body): **remove**
   the `"instances must come before links"` / `"a child must come before the body's links"`
   errors. Keep appending nodes→children and links→links as encountered (intra-list source
   order is preserved; cross-list interleave is recovered later via `Span`).
2. Update/delete the two parser tests asserting those errors (`parser.rs:~1256`, `~1283`) —
   replace with tests that an interleaved body now parses and preserves per-list order.
3. `fmt`: ensure a body with interleaved nodes/links **round-trips** (source order kept; a
   formatter pass must not reorder a link before a following node or vice-versa). Add a fmt
   snapshot/round-trip test with `[ a -> b ; |box#c| ; c -> d ]`-shaped input.

**Verify:** `cargo build && cargo test`; `cargo run -- fmt <(printf '|g| [\n a -> b\n |box#c|\n]\n') --stdout` keeps order. Confirm a normal diagram (children-then-links) is unchanged.

**Done when:** interleaved bodies parse + fmt round-trips; all existing tests green (the two
removed-error tests are replaced, not just deleted).

> **Step 1 outcome / decisions (done):**
> - **AST kept the `(Vec<Child>, Vec<Link>)` split** (not a unified ordered body). Rationale:
>   resolve's two-phase build genuinely wants the split, it maximises reuse (AST/resolve/desugar
>   untouched), and source order is faithfully recoverable from `Span`. The interleave is
>   reconstructed only where it matters (fmt now; the sequence engine in Step 3). Spans are the
>   canonical position source, so this is a core read, not patchwork. *If Step 3/5 finds span
>   ordering fragile with synthesised nodes, escalate to a unified `body: Vec<Stmt>`.*
> - **Parser:** removed both ordering guards (`parser.rs` root loop + `parse_children`); nodes
>   and links append to their lists in source order. Doc comments updated.
> - **fmt:** one shared `emit_ordered(children, links, depth)` (replaces `emit_children`), used by
>   the file level *and* every `[ ]` body — merges by span so the trivia cursor stays monotonic.
>   A `phased(instances, links)` helper preserves the conventional canvas↔links blank line for
>   normal files and drops it only when interleaved (a sequence). **File-level interleave is
>   handled** — nothing deferred here.
> - **Tests:** the two error-tests became relaxation tests (`instances_and_links_interleave_at_root`,
>   `body_children_and_links_interleave`); two fmt round-trip tests added.

---

## Step 2 — Sequence types + dispatch + participants/lifelines

**Goal:** `layout: sequence` is recognised end-to-end and renders the **skeleton** —
participants across the top with lifelines (no messages yet). A participant-only sequence
produces a valid SVG.

**Files:** `src/desugar/types.rs` (`TEMPLATES`), `src/desugar/bundles.rs` (`template_bundle`),
`src/layout/mod.rs` (`layout_inst`, `read_layout_mode`), **new** `src/layout/sequence/mod.rs`
(+ `model.rs`, `prim.rs`), `src/resolve/` (only if a sequence-only-child check lands here).

**Changes:**
1. **Desugar types** (`types.rs` `TEMPLATES`): add `("sequence","block")`, `("note","block")`,
   `("loop","block")`, `("opt","block")`, `("alt","block")`, `("else","block")`. They become
   protected built-ins automatically (`is_builtin_type`).
2. **Defaults** (`bundles.rs` `template_bundle`): add the SPEC §10 Defaults arms — `sequence`→
   `layout: sequence`; `note`→`fill: --group-fill; stroke: --stroke; padding: 6 8; font-size: 12`;
   `loop`/`opt`/`alt`→`fill: none; stroke: --group-stroke; stroke-style: dashed; stroke-width: 1;
   radius: 4`; `else`→ the same minus radius.
3. **Dispatch** (`layout/mod.rs`): add `sequence::is_sequence(&inst.attrs)` and intercept in
   `layout_inst` *before* child recursion, mirroring the `is_chart` block; teach
   `read_layout_mode` to accept `"sequence"` (route to the engine, never the flow/grid path).
4. **Engine skeleton** (`sequence/`): `model.rs` partitions children by type into participants
   / frames / notes (reject a non-participant/non-frame/non-note box with a scoped error, the
   chart-model way; defer frames/notes handling to Steps 5). `mod.rs::layout_sequence` places
   participants left→right by declaration order, draws each lifeline (`|line|` in scene
   `stroke`) to a common foot, and returns the container `PlacedNode` (copy `chart_box`).
   `prim.rs`: thin `text`/`line`/`block`/`rect` builders (copy `chart/prim.rs`).

**Verify:** `cargo build && cargo test`; `cargo clean -p lini` then render
`|box#a| "A"` + `|box#b| "B"` under `{ layout: sequence }` to PNG with `resvg` and **read the
PNG**: two headers, two lifelines, common foot. Add a first `insta` snapshot.

**Done when:** a participant-only sequence renders correctly; sequence types desugar/resolve;
non-participant children error clearly; tests green.

> **Step 2 outcome / decisions (done):**
> - **Refactor for reuse:** promoted `chart/prim.rs` → **`layout::prim`** (the generic
>   `PlacedNode` builder library) so charts *and* sequences share it (9 chart imports rewired,
>   `chart_box` now calls the new `prim::container` — no duplicated container shell). `prim`
>   (builders) sits beside `primitives` (sizing); both documented to avoid the name clash.
> - **Dispatch:** node-form `|sequence|` intercepted in `layout_inst` (like `is_chart`);
>   **root-form `{ layout: sequence }`** intercepted in `attempt()` (it owns the whole scene,
>   bypassing the generic arrange + router). Shared core `lay_out(attrs, participants)`.
> - **Participant detection** by type: every box that isn't `loop`/`opt`/`alt`/`else`/`note`.
> - **GOTCHA / deferred:** non-participant children (frames/notes) and messages are **filtered
>   out (not yet drawn)** in Step 2 — they arrive in Steps 3/5. A root sequence with frames
>   silently drops them *until Step 5*; that's the only interim gap. Lifeline foot length is a
>   placeholder (`gap_row*3`) until Step 3 sets it to the last message row.

---

## Step 3 — Messages + the link-partition / wiring-strategy seam

**Goal:** call/return/async/self **messages** render as time-row arrows; the orthogonal router
no longer sees them.

**Files:** `src/resolve/ir.rs` (`ResolvedLink`), `src/resolve/program.rs` (set scope),
`src/layout/links/mod.rs` (`route_links`), `src/layout/links/bundle.rs` (`requests`),
`src/layout/mod.rs` (thread links into the engine), `src/layout/sequence/messages.rs` (new).

**Changes:**
1. **Tag link scope** (`ir.rs` + `program.rs`): add `pub scope: String` to `ResolvedLink`
   (the dot-path of the container the link was authored in; `""` = root) and set it where
   links are resolved (`program.rs:~81/85`, `resolve_link`). Derive a scope's layout by
   looking up that node's attrs in the scene tree.
2. **Partition** (`links/`): add `pub(crate) fn strategy(layout: Option<&str>) -> Strategy`
   returning `Orthogonal | Sequence` (+ a doc-comment block scaffolding future
   `Straight`/`Curved`). In `bundle::requests` (or at the top of `route_links`), **skip** every
   link whose scope resolves to a `Sequence` strategy. The router's law tests (`tests/linking.rs`)
   must still pass for non-sequence scenes.
3. **Claim + lay out messages** (`sequence/`): thread the scope's messages into
   `layout_sequence` (filter `program.links` by `scope == self_path`; pass `&program` or the
   filtered slice through `layout_inst`, as `funcs` is passed today). In `messages.rs`: build
   the **time order** by sorting the union of {frame/note children, messages} on `Span`;
   participants keep declaration order. For each message emit a horizontal `|line|` arrow at
   its row between the two lifelines (self-message → a lifeline hook), apply the paint map
   (`link→stroke`, `link-width→stroke-width`, `link-style→stroke-style`, operator end-marker →
   `marker-end`), and place its label centred above the arrow (reuse the link-label text leaf;
   **no parallel label code**).
4. **Sizing** (`model.rs`): participant spacing = `max(gap-col, widest message label between
   adjacent lifelines + margin)`, text measured via `layout::approx_width`.

**Verify:** `cargo build && cargo test` (incl. `tests/linking.rs` still green); render the
§10 login example (sans frame) to PNG and read it: arrows on time rows, dashed returns, wavy
async, a self-hook, labels above arrows, none routed orthogonally. Snapshots.

**Done when:** messages render as time arrows; router skips them; `tests/linking.rs` unaffected;
the strategy seam + future-arm comments are in place.

---

## Step 4 — Activations

**Goal:** implicit activation bars.

**Files:** `src/layout/sequence/activations.rs` (new), `model.rs` (read `activation:`).

**Changes:**
1. Read `activation` (`auto` default / `none`) on the sequence.
2. Compute bars by the SPEC §10 rule: a **sequence-global** per-participant LIFO stack — `->`
   pushes a bar opening at the target/row; `-->` *from* that target pops its top (closes at the
   row); self/async push nothing; an orphan `-->` pops nothing. Unclosed bars run to the foot.
   Nested bars offset outward. Emit each as a thin `|block|` (`fill: --fill; stroke: --stroke`)
   on the lifeline; messages attach to the bar edge.
3. Determinism: drive purely off message row order (already span-sorted).

**Verify:** `cargo build && cargo test`; render a nested call/return example and read the PNG —
stacked bars, correct open/close rows, `activation: none` removes them. Snapshots.

**Done when:** bars are correct + deterministic; the toggle works.

---

## Step 5 — Frames (`loop`/`opt`/`alt`/`else`) + notes

**Goal:** frames and notes render; frame bodies are scope-transparent.

**Files:** `src/resolve/` (frame-body transparency), `src/layout/sequence/frames.rs`,
`notes.rs` (new), `model.rs`.

**Changes:**
1. **Scope transparency (resolve):** a message inside a sequence frame's `[ ]` must resolve its
   endpoints against the **enclosing sequence's participants**, not the frame body, and
   **auto-create none** locally (SPEC §10 "One scope"; overrides §3/§9 sealed-body inside a
   sequence). Implement by resolving a sequence frame's internal links in the sequence scope
   (walk frames transparently when computing a link's scope/participant set). Add tests that
   `|alt| [ db --> api ]` wires the outer `db`/`api`, with **no** phantom frame-local boxes.
2. **Frames (layout):** the engine already time-sorts frames with messages (Step 3). For each
   frame, compute the **row span** of its contained messages and the **lifeline span** of the
   participants they touch; draw a dashed `|block|` rectangle (inset past those lifelines) with
   a top-left **title tab** (smart label). `|alt|`: split by `|else|` children into compartments,
   a dashed divider + guard label per `|else|`; the first guard is the `|alt|` label. Frames
   **nest** (recurse). Decide + snapshot whether a frame tab / `|else|` occupies a row.
3. **Notes (layout):** `|note|` at its time row; resolve `over` (one id, or `a b` span across
   those lifelines + any between), `left`, `right`; draw a `|block|` + text; multi-line/styled
   note rides `[ ]`.

**Verify:** `cargo build && cargo test`; render the §10 `|alt|`/`|else|` example and a note
example and read the PNGs — frame rectangle spans the right lifelines/rows, `else` divider +
guards, note placement. Snapshots. Re-render the full §21 login flow.

**Done when:** frames (incl. nesting + `alt`/`else`) and notes render; scope-transparency tests
pass; the §21 example renders as drawn in SPEC §10.

---

## Step 6 — Samples, snapshots, render-verify, polish, acceptance

**Goal:** ship-quality — one sample, full snapshot coverage, visual verification, clean lint.

**Files:** `samples/sequence.lini` (the §21 login flow), `tests/conformance.rs` (auto-snapshots
samples), `editors/` (syntax highlight for the 6 new types — nice-to-have), `README.md` (a short
Sequences section, mirroring Charts).

**Changes / checks:**
1. Add `samples/sequence.lini` (the SPEC §21 example) — one sample per feature (AGENTS.md).
2. `cargo insta review` — accept the new snapshots; confirm determinism (run twice,
   byte-identical).
3. **Render-verify** (`cargo clean -p lini` first): render the sample to PNG with `resvg` and
   read it — participants, messages (all 4 ops), activations, the `alt`/`else` frame, the note,
   light **and** dark (`--theme dark`), and `--bake-vars`. Fix any visual issues.
4. README: a Sequences subsection; mention `layout: sequence` in the feature list.
5. **Gates:** `cargo fmt --all -- --check`, `cargo clippy --all-targets`, `cargo test` all
   clean. No `unsafe`. One mechanism per problem (labels reuse the link-label path; bars/frames/
   notes/lifelines reuse `|block|`/`|line|`).

**Done when:** the acceptance checklist below is fully ticked.

---

## Acceptance checklist (final)

- [ ] §10 examples and the §21 login flow render correctly (read the PNGs), light + dark + baked.
- [ ] Messages: call/return/async/self all correct; router never touches them; `tests/linking.rs` green.
- [ ] Activations deterministic; `activation: none` works.
- [ ] Frames (`loop`/`opt`/`alt` + `|else|`, nested) and notes (`over`/`over a b`/`left`/`right`) correct.
- [ ] Frame bodies are scope-transparent — no phantom participants (test).
- [ ] Undeclared endpoints auto-create participants; declared order honoured.
- [ ] `fmt` round-trips interleaved bodies and sequence diagrams.
- [ ] Determinism: two runs byte-identical; one `samples/sequence.lini`; snapshots committed.
- [ ] `cargo fmt --check`, `cargo clippy`, `cargo test` clean; no `unsafe`.
- [ ] Linking seam documented with future `straight`/`curved` arms scaffolded (comment-only).

## Risks & mitigations

- **Router law tests break** when messages are excluded → partition *before* `bundle::requests`
  builds edges; assert `tests/linking.rs` green at Step 3.
- **Threading links into `layout_inst`** is new (charts don't) → pass the filtered scope slice
  alongside `funcs`; keep the engine's input explicit and small.
- **Scope detection** for a link → prefer the explicit `ResolvedLink.scope` field over deriving
  from endpoints; set it once at resolve.
- **Grammar relaxation regressions** → the only behavioural change is *removing* an error; add
  tests that normal diagrams are byte-identical before/after.
- **Frame scope transparency** is the subtle one (two auditors flagged it) → land it with a
  dedicated "no phantom participant" test before drawing frames.
- **Micro-geometry drift** → freeze every constant with `insta`; the SPEC intentionally leaves them open.

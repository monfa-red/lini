# PLAN — Schematics (beta 2)

PCB schematics as a native lini family: fixed-port routing (global), identity
capsules in link endpoints (global), and the `layout: schematic` scope with
components, pins, discretes, net labels, and the anchor + satellite placement
model. **SPEC 16 (Schematic) is the law** — written in Phase 0, below;
this plan sequences the work. The design ledger (13 brainstorm rounds) is
`SCHEMATICS_BRAINSTORM.md` — history, not law; where it and SPEC disagree,
SPEC wins. Section numbers in this plan are **post-renumber** (Phase 0 moves
Part III: Ledger 17 · SVG 18 · Pipeline 19 · CLI 20 · Errors 21 · Grammar 22
· Reserved 23 · Deferred 24 · Examples 25).

---

## How to work this plan (read this every session)

1. **Re-orient**: read `SPEC.md` **fully** (yes, all of it — schematic touches
   core), `ROUTING.md` fully, and this plan **fully** including every phase's
   execution log and carry-over notes. Then `git log --oneline -15` and
   `cargo test` (must be green before you start).
2. **One or two phases per session.** Do not start a phase you can't finish.
3. **Log as you go**: every phase has `### Execution log` and
   `### Carry-over notes`. Log decisions made, constants chosen, surprises,
   anything the next phase (or a re-run of this one) needs. **Never rely on
   conversation memory — if it matters, it goes in this file.**
4. **Ask the user** about: contract changes not covered here, visual-taste
   calls you can't settle with the reference sheet, anything that would break
   a SPEC law. Small obvious calls are yours — make them and log them.
5. **House rules bind** (AGENTS.md): no `unsafe`; one mechanism per problem;
   no parallel implementations — promote visibility and share, never copy;
   split modules past ~500 LOC; reused style rides a CSS rule + class, never
   inline `style=`; comments only for non-obvious *why*.
6. **Before every commit**: `cargo fmt`, `cargo test`, `cargo clippy`. Tick
   the phase's checkboxes as you complete them. Do not push to main without
   the user's go-ahead.
7. **Visual verification is mandatory** where output changes: render the
   sample SVG to PNG with `resvg` and *look at it* (light and dark where
   paint is involved).

### Cross-phase invariants (verified against source — violating these breaks CI)

- **Desugar fixed point**: `tests/oracle.rs:65-71` proves lowered output
  *renders* identically (SVG compare); the byte-identical *source* fixed
  point lives in `tests/desugar.rs:111-116` over one hand-written string.
  Phase 2 extends the source fixed point to a samples sweep **before** the
  first minting work. Every generated node/link needs (a) idempotent
  detection so re-desugar doesn't duplicate it, (b) a span seated *past the
  last instance* (`desugar/mod.rs:120-137` pattern).
- **The routed-link filter**: one definition
  (`routing/ortho/request.rs:89 is_routed`), one caller
  (`routing/ortho/labels.rs:41`), and one **divergent reimplementation** —
  `lib.rs:347 declared_edges_with` filters on strategy and omits the
  `LinkKind::Wire` / `!projection` tests. Any scope-predicate change must
  reconcile all three (schematic wires stay routed, so likely only the
  reconciliation, not a new clause — verify and log).
- **`--lini-*` vars are tree-shaken** by literal `var(--lini-` text scans
  (`render/used_vars.rs:91-106`) — introduce new roles through the same
  formatting path or their `@layer` defaults silently vanish.
- **`layout:` is a bare ident, not an enum.** Dispatch is per-engine
  predicates; exact-name occurrences of `is_drawing`/`is_sequence`/`is_tree`
  total **29** across resolve/desugar/layout/lint/validate, plus the
  near-name family (`is_drawing_scope`, `is_sequence_scope`,
  `scope_is_drawing`, `is_drawing_body`, `is_drawing_node`, `is_tree_scope`,
  `container_layout`). A missed site fails *silently* unless control reaches
  `layout/arrange.rs:217 read_layout_mode` (`"unknown layout … expected flow
  or grid"` — that message and its snapshot must also be updated). Grep both
  families as the checklist.
- **Error codes**: one row in `src/error/codes.rs::catalog!`, never a
  literal; numbers are stable once assigned (snapshot-pinned).
- **Regen artifacts**: any change to `TEMPLATES`, `NodeKind::ALL`, or
  `PROPERTIES` requires `cargo xtask gen-schema` + `cargo xtask gen-grammars`
  (byte-identical guards in `tests/schema.rs` / `tests/grammar.rs`); every
  `PROPERTIES` row needs a matching `src/ledger/examples.rs` entry
  (`schema/mod.rs:507` asserts the counts match, `tests/schema.rs:35`
  compiles each).
- **Oversized files** (at/over the ~500 split rule) this plan touches:
  `desugar/mod.rs` 727 · `desugar/tree.rs` 823 · `resolve/links/mod.rs` 624
  · `validate.rs` 785 · `ledger/defaults.rs` 657 · `ledger/properties/mod.rs`
  849 · `layout/mod.rs` 608 · `routing/ortho/place.rs` 844 · `lexer.rs` 869
  · `fmt.rs` 927 · `grammar/mod.rs` 826 · `render/links.rs` 671 ·
  `render/stylesheet/families.rs` 653 · `syntax/parser/tests.rs` 662. When a
  phase grows one, split it as part of that phase (log the split).

### The feature in one paragraph (orientation, not law — law is SPEC 16)

A schematic is a thin scope: **placement is grid-like** (anchors — 3+-pin
parts or anything explicitly placed — auto-flow one row, `cell:` overrides
with ordinal collapsing indices), **satellites** (labels and unplaced 1–2-pin
parts) seat at the pin their wire touches (chains grow by the terminator's
authored connection geometry; auto-pose in 90° steps; `rotate:` forces,
`translate:` nudges), and **the orthogonal router keeps the wires** — with
fixed ports at pin stubs, square corners, junction dots at ≥3-way meets,
duplicate wires an error, and same-pin landings merged as implicit fans.
`|component|`+`|pin|` (desugared side rails, scope-transparent), uppercase
discretes (`|R| |C| |L| |D| |LED| |Q| |Y| |F| |FB| |SW| |BT| |V| |I|`,
`|opamp|`, `|J|`) with generated pins and `symbol:` variants, `|label|` with
the schematic symbol set (`gnd earth chassis power nc antenna`) and `shape:`
tags. Identity capsules (`a -> |cyl#db|`, `c24.p2 - |gnd|`) are a **core**
feature (desugar-hoisted declarations), as is typed auto-create. Refs (U7,
R1…) are the displayed id; anonymous discretes mint display-only refs. The
scope kills implicit auto-create; `U7.DIAG - "NSTDBY"` mints a label.

### Settled design decisions the phases depend on (do not re-litigate)

- **Pin routing identity**: a pin endpoint (`U7.VS`) resolves to the pin
  node, but the router's endpoint **body rect is the component's obstacle
  rect** — the body with pin stubs and pin chrome folded in; pin nodes are
  **never** separate router obstacles. The **fixed port** is the stub-tip
  ordinate on the pin's side of that rect; the forced side is the pin's
  side. Same model for a label: its body is its own tag/symbol bbox, its
  fixed port its connection point.
- **Grid ordinal-collapse never touches core grid semantics** (SPEC 12's
  "an empty `""` cell holds its track" law stands). The schematic engine
  builds its **own track list** from the max referenced ordinal (empty
  tracks collapse entirely) and reuses grid's *placement helpers* only.
- **Square corners ride a new link property `corner-radius`** (default
  `auto` = today's clearance-derived cap, `render/links.rs:29 radius_cap`);
  the schematic scope's link defaults set it `0`. **Never** reuse
  `clearance` — it is simultaneously the router's Law-1 spacing
  (`request.rs:326 link_clearance`, `validate.rs:47-58`), and zeroing it
  would disable clearance for the scope.
- **Fixed-port infeasibility is loud, never a clamp**: two fixed ports
  closer than min pitch, a fan across two *different* fixed ports
  (`place.rs:346 merge_fans` window intersection), or a fixed port blocked
  by a keep-out (`entry.rs:76` drops inverted windows → no entry) each
  produce a structured error or an honest stray with a named reason —
  today's release behaviour on these paths is *silent misplacement*, which
  Phase 1 must close. Schematic constants keep pin pitch ≥ min pitch at the
  scope's default clearance (tuning note for Phase 6).
- **Property naming**: `number:` (pin), `prefix:` (parts), `shape:` (label
  tag — kept despite the docs-table column also titled "Shape"; log any
  reader confusion), `pins: N` (connector count — one character from the
  universal `pin:`; the value shapes disagree so validation catches mixups
  with a did-you-mean; watch it in review). `|pin|` + `pin:` is lini's
  first **type/property homonym** — never grammatically ambiguous (bars vs
  before-`:`); state it in SPEC 16.
- **Built-in label defines**: `gnd` **and `nc`** ship (the brainstorm's own
  examples use `- |nc|`); power nets stay user defines.

---

## Phase 0 — SPEC 16: Schematic (the law) + renumber

**Goal**: SPEC.md reads as if schematics existed from day one. Executed in
the planning session; recorded here so every later phase can trust it.

**Tasks**
- [ ] New Part II section **16. Schematic** — full family: the scope, roles
      (anchor/satellite), `|component|`/`|pin|` (bilateral split, `side:`,
      order, pin `translate:` slide, `number:`, name label override, id
      readouts, minted display refs never endpoints), discretes table
      (types, mints, pins incl. polar/variant sets, `symbol:` variants,
      `prefix:`, IEC-only), `|opamp|` (mints U; pins `out`/`inp`/`inn`;
      power pins hidden by default), `|V|`/`|I|` (`symbol: dc | ac`), `|J|`
      (+`pins: N`), `|label|` (text/symbol/shape, connection points,
      `shape:` from the wire op's marker — visual not semantic), label
      wires (one-ended forms), wiring laws (arity incl. dangling pins
      legal, pass-through chains, duplicates error, same-pin merge, marker
      gate, no auto-create, `:side` banned on terminals,
      `|component#X|.p4`-style unknown-pin resolve error), placement
      (anchors/one-row/`columns:`/ordinal `cell:`, satellite seating,
      cluster bboxes, two-placed-end distribution, auto-pose + 90°
      `rotate:`, upright text), junctions, `corner-radius`, ref/value
      smart-label placement (component: above; discrete: beside;
      deterministic; translate overrides), scoped defaults + role
      variables (wire, part fill/outline, label, pin-number muted, scene).
- [ ] Core amendments: SPEC 3/9 (capsule endpoints — declaration in
      endpoint position, no tail, either end, fans/chains, drawing-scope
      ban), SPEC 21-era grammar section (endpoint rule gains the capsule
      alternative while **keeping** `[ "." index ]`; one-ended op list
      gains the schematic arm; first-token law relaxed by one token for a
      statement-head capsule), SPEC 15.7 (the "a leader points back"
      one-ended law gains its schematic counterpart), SPEC 8 (template
      table rows), Ledger (new properties incl. `corner-radius`), Reserved
      Words (new protected type names), Errors (new families), Deferred
      (wire-seating, ANSI knob, gates/T/K/M/LS/RV, hop-overs, buses,
      mid-wire tags…; **remove or rewrite** the routed-anchors row —
      Phase 1 builds the contract but not the flow/grid `a -> b:port`
      surface, so the row is *narrowed*, not deleted).
- [ ] **Renumber**: Part III 16-24 → 17-25; every `[SPEC N]` cross-ref +
      anchor fragment in SPEC.md; `AGENTS.md` ("[SPEC 17]'s class-diff" →
      18); grep ROADMAP.md, README.md, and `src/`+`tests/` comments for
      `SPEC 1[6-9]|SPEC 2[0-4]` and fix.
- [ ] Self-check: ToC matches, no dangling anchors (grep `](#`), the four
      SPEC-16-adjacent laws read against rounds 1-13 of the brainstorm.

### Execution log

### Carry-over notes

---

## Phase 1 — Fixed-port routing (the ROUTING.md contract extension)

**Goal**: the orthogonal router accepts per-end **fixed ports** — a
caller-supplied ordinate on a forced side — landing wires exactly there,
lawfully and loudly. Global core capability; no schematic types exist yet —
test through `lib.rs::testing` hooks with synthetic requests and
`:side`-forced fixtures.

**Read first**: ROUTING.md (all), `src/routing/ortho/{mod,request,entry,
place,ladder,ledger,admit,search}.rs`, `src/routing/validate.rs`
(especially `landing()` at ~:114).

**Design (settled; details yours)**:
- `EdgeReq` (`request.rs:19-45`) gains an optional fixed-port ordinate per
  end beside `side_a/side_b`; the end's body rect stays the *obstacle* rect
  per the pin-routing-identity decision above. Forced sides already prune
  entries, split bundles, and fan across statements — build on that.
- `entry::entries`: a fixed port collapses the window to a point —
  `entry.rs:44-50` already degenerates short sides to `(centre,centre)`
  (pinned by `a_short_side_offers_its_centre_point_window`), so a point
  window is a supported downstream shape. **But** `clip_window`
  (`entry.rs:88`) can invert a point window under a blocker and `:76` then
  drops the entry → the link must become an **honest stray with a named
  reason** ("fixed port blocked"), per ROUTING.md's stray contract.
- `place::chain_prefs` (`place.rs:285`): a fixed end is
  `(port, Some((port, port)))`. `ladder.rs` accepts equal bounds for a lone
  item, but: `place.rs:294-302` intersects both-end windows behind a
  `debug_assert!`; `ladder.rs:66-74` pools with a debug-only infeasibility
  assert and **release clamps to `hi`** (`ladder.rs:44-46`), i.e. silently
  off the fixed port; `place.rs:426 overrun` → `pairwise.rs:115-117` clamps
  the same way. **Close all of these**: infeasible fixed-port systems are
  structured errors/strays, never clamps.
- Capacity: the ledger records only per-`(path, side)` *counts*
  (`ledger.rs:33`, `commit_port :64`, `side_free :197`) — no ordinates. The
  fixed-port constraint is enforced through the windows carried into
  `place`/`ladder`; the counter needs at most a capacity adjustment, and
  the admission filter is `ortho/mod.rs:216-224` (`side_free >= need`).
  `admit.rs` is a placement simulation and may need nothing — verify.
- **Fans at fixed ports**: same-point landings merge into one shared fixed
  port (the implicit-fan law, SPEC 16); a fan whose ends carry two
  *different* fixed ports is a structured error (`merge_fans`,
  `place.rs:346-349`, currently intersects with no emptiness check).
- **Law 2 amendment** (ROUTING.md): a fixed-port end lands *at its port*,
  perpendicular; the corner-margin rule is waived for fixed ports. The
  waiver lives in the validator's `landing()` (`validate.rs:114-147`, which
  already relaxes to `c.min(len/2)` for short sides) — **not** in
  `excuse.rs` (that mechanism is Law-1-only, called solely from
  `separation` at `validate.rs:309`). There is no corner-rounding law in
  the validator (verified: zero hits) — don't invent one. The validator
  judges output alone, so the fixed-port ordinate must be carried on the
  drawn link for it to judge; decide the carrier and log it.

**Files**: `ROUTING.md`; `src/routing/ortho/{request,entry,place,ledger,
mod}.rs`; `src/routing/validate.rs`; `src/lib.rs` testing hooks if a new
probe is needed. `place.rs` (844) — split it.

**Tasks**
- [ ] ROUTING.md: "Fixed ports" section — vocabulary, Law 2 amendment,
      fan-at-fixed-port law, infeasibility-is-loud contract, determinism
      unchanged. Tight, lawful prose in the document's voice.
- [ ] `EdgeReq` fixed-port field + plumbing (nothing sets it yet except
      tests).
- [ ] `entries` point-window path incl. blocked-port stray; `chain_prefs`
      fixed prefs; both-end/pooling infeasibility errors (no release
      clamps); `merge_fans` conflict error; capacity adjustment.
- [ ] Validator: `landing()` fixed-port waiver + exact-port check; carrier
      for the ordinate.
- [ ] Tests in `tests/routing.rs`: fixed port lands exactly (± ε); two
      fixed ports on one side don't braid; fixed + free mix ladders around
      the fixed one; same-point fan merges; conflicting fan errors; fixed
      ports closer than min pitch error; blocked fixed port → named stray;
      determinism (byte-identical rerun). `tests/laws.rs` sweep green
      including the low-clearance end (6.0) with fixed ports in play.
- [ ] `cargo fmt && cargo test && cargo clippy` green.

### Execution log

### Carry-over notes

---

## Phase 2 — Identity capsules in endpoints + typed auto-create (core)

**Goal**: `a -> |cyl#db|` everywhere — an endpoint may be a bare id or an
identity capsule (`|type|` / `|type#id|`); a capsule declares (desugar
hoists it to a declaration at the statement's position + the link references
it); capsules compose with endpoint anatomy but never take a statement
tail; legal at either end, mid-chain, and in fans (fan = one instance).
Drawing scopes reject capsules (no-invention law).

**Read first**: SPEC 3/9/22 (as amended by Phase 0),
`src/syntax/parser/{mod,classify,links}.rs`, `src/desugar/{mod,scene,
tree}.rs` (minting + auto-create patterns), `src/resolve/links/mod.rs`,
`src/fmt.rs`.

**Design (settled; details yours)**:
- **Grammar**: `endpoint = (ident | ident_bars) { "." ident } [ "." index ]
  [ ":" point ]` — the pattern-copy index **stays**. After an op, `|` opens
  a capsule. The three parser gates that currently admit `Ident` only:
  `links.rs:17` (second group), `:35` (one-ended detection), `:77`
  (`expect_ident` in `parse_endpoint`) — each must admit the capsule form.
  At statement head, `classify.rs:52` is
  `Some(Pipe) | Some(String) => Kind::Node` — **split** the arm (don't
  extend it): parse past the self-delimiting capsule; op/`&` next ⇒ link,
  anything else ⇒ node statement. `||`-vs-capsule was never ambiguous
  (`pipes_glued_at`, `parser/mod.rs:183-188`, requires two *adjacent*
  pipes); the real new cases to test are `|a| || |b|` (capsule-headed
  mate — drawing-scope error at resolve, but must parse), `a - |gnd| - b`
  (capsule mid-chain), and spacing edge cases around `-|gnd|`.
- **Capsule takes no tail**: a label/class/`{ }`/`[ ]` after a capsule at
  operator position belongs to the link; a statement-head capsule followed
  by a tail is a plain node statement.
- **Desugar hoist**: mint internal ids for anonymous capsules (`lini-cap-N`
  — the `tree.rs:542 mint_ids` pattern: 1-based, idempotent, reserved
  prefix), emit the declaration at the statement's position among instances
  (span-seated per the fixed-point discipline), rewrite the endpoint to the
  id. An id'd capsule hoists as-written; duplicate id = the existing
  duplicate-id error. New module `src/desugar/capsule.rs`.
- **Typed auto-create replaces nothing** — implicit bare-id auto-create
  (`scene::auto_box`) stays; capsules are the typed form.
- **Gates**: drawing scope → error (extend the no-auto-create family in the
  phase that knows the scope — desugar knows `root_drawing`
  (`desugar/mod.rs:138`); log where you land and why). Sequence scope →
  allowed (a typed participant).
- **fmt** round-trips capsules canonically (`a -> |cyl#db|`).
- Resolve-time: a capsule + dot-path into a child that doesn't exist
  (`|component#U9|.p4`) errors as unknown endpoint — the inline component
  has no authored pins; add the message.

**Files**: `src/syntax/parser/{classify,links}.rs`, `src/syntax/ast.rs`,
`src/desugar/{mod.rs,capsule.rs(new)}`, `src/resolve/links/mod.rs`,
`src/fmt.rs`, `src/error/codes.rs`, `src/grammar/mod.rs` + regen.

**Tasks**
- [ ] Extend `tests/desugar.rs` source-idempotence to a samples sweep
      (pre-work — see the fixed-point invariant).
- [ ] Parser: capsules both positions; the three `Ident`-gate sites; the
      `classify.rs:52` split; `|a| || |b|`, `a - |gnd| - b`, spacing cases.
- [ ] Desugar hoisting (`desugar/capsule.rs`), minted `lini-cap-N`,
      fixed-point sweeps green.
- [ ] Resolve: endpoint rewrite before path resolution; drawing-scope gate;
      sequence allowed; `.p4`-on-inline error.
- [ ] fmt round-trip + `tests/fmt.rs` cases; `lini desugar` output of a
      capsule statement re-desugars byte-identically.
- [ ] Grammar/schema regen; `tests/{parsing,desugar,resolution}.rs`:
      `a -> |cyl#db| "watches" { stroke: red }` (tail is the link's),
      `|cyl| -> a`, `a -> |cyl| -> c`, `a & b -> |cyl|` (one instance),
      capsule + `.path`/`:side`/`.index` composition, drawing-scope error.
- [ ] `cargo fmt && cargo test && cargo clippy` green.

### Execution log

### Carry-over notes

---

## Phase 3 — Schematic types, symbols & refs (no engine yet)

**Goal**: every schematic type exists, lowers, and renders *outside* any
schematic engine (scope gates arrive in Phase 5 — **log that deferral
here so Phase 5 doesn't forget**): `|component|` + `|pin|` with generated
side rails; the discrete family with generated pins and `symbol:` variants;
`|label|` with the schematic symbol set and `shape:`; built-in defines
(`|gnd|`, `|nc|`, `|J|`); ref readouts + display-only minted refs; new
properties in the ledger.

**Read first**: SPEC 16 + SPEC 8/18 (as amended); `src/desugar/{types,
titleblock,page}.rs`, `src/glyph/mod.rs`, `src/layout/drawing/symbols.rs`,
`src/ledger/*`, `src/render/stylesheet/families.rs`.

**The six-item registration checklist (per new type — from the survey)**:
`desugar/types.rs::TEMPLATES` row → `ledger/defaults.rs::template_bundle`
arm → `ledger/properties/mod.rs::PROPERTIES` rows (new props only) →
`ledger/examples.rs` entries → `validate.rs` arms (`container_layout` :566,
`role_accepts` :618, `misuse_message` :637) → regen (`gen-schema`,
`gen-grammars`).

**Design (settled; details yours)**:
- **Types**: `component`, `pin`, `label`, `schematic`, discretes
  `R C L D LED Q Y F FB SW BT V I`, `opamp` (mints U; pins
  `out`/`inp`/`inn`; power pins exist but hidden by default — decide the
  reveal knob and log it), `J` (define over component; `pins: N` generates
  N numbered nameless pins), label defines `gnd` + `nc`. Reserved-word
  protection is automatic via `TEMPLATES`.
- **Component/pin desugar** (`desugar/schematic.rs`, new): bilateral split
  (first ⌈n/2⌉ left, rest right, declaration order; explicit-`side:` pins
  excluded from the count, autos split over the remainder) into generated
  *anonymous* rails — verify scope-transparency (`U7.VS` resolves, no rail
  in any path). Pin anatomy (stub, name inside, `number:` outside) lowers
  as pin-owned chrome **folded into the component's obstacle** (the settled
  pin-routing-identity decision). A pin's `translate:` slides it along its
  side.
- **Symbols**: schematic bodies + label symbols join the `glyphs!` registry
  (`src/glyph/mod.rs:45` — note the name). Three registry invariants: the
  count assertion (`:135`, currently 22), the table stays **sorted**
  (binary-searched `:110`), fragments `starts_with("<path")` (`:141`).
  `names()`/`suggest()` are dead-code-allowed until a consumer exists —
  wiring the schematic unknown-symbol error is that consumer. Extend
  `drafting_type` into a scope-tagged lookup rather than a second list (one
  mechanism; six call sites listed in the survey). **Sizing law is new and
  explicit**: discrete bodies size from baked schematic pitch constants —
  *not* font-coupled like the file-local `FINISH_HEIGHT_EM`
  (`drawing/symbols.rs:70`). Constant placement rule: values that become
  cascade `Decl`s go in `ledger/defaults.rs` (beside `MINDMAP_*`); baked
  chrome geometry goes in `ledger/consts.rs` (`// schematic` section).
- **Generated pins**: `p1/p2`; polar semantic ids (`a`/`k`, `b c e` /
  `g d s` by `symbol:` variant, `plus`/`minus`); the variant sets body and
  pin set together. IEC bodies only. Each symbol records its **connection
  point(s)** in the registry so Phase 4 can pose satellites (gnd's at its
  top, power's at its bottom, tag's at its flat end).
- **Refs**: id displayed verbatim as generated chrome text
  (desugar-visible); placement per SPEC 16 (component: above; discrete:
  beside; deterministic; translate overrides). Anonymous discretes mint
  **display refs** (prefix = type name; `prefix:` overrides; authored ids
  win; skips taken names) as a display attribute at desugar — **never as
  the node id**; test that wiring a minted ref errors as unknown endpoint.
- **`|label|`**: smart label = net text in the tag outline; `symbol:` from
  the schematic set; `shape:` (`plain`·`left`·`right`·`both`·`round`).
- **Ledger**: `number:`, `prefix:`, `shape:`, `pins:`, `corner-radius`
  (Phase 5 consumes it; the row can land here) — owners, shapes, gates per
  the ledger's style.

**Files**: `src/desugar/{types.rs,schematic.rs(new)}`,
`src/ledger/{defaults,properties/mod,consts,examples}.rs`,
`src/glyph/mod.rs` (+ maybe `glyph/schematic.rs`),
`src/layout/schematic/symbols.rs` (new, sibling of drawing's),
`src/render/stylesheet/families.rs`, `src/validate.rs`,
`src/error/codes.rs`, regen.

**Tasks**
- [ ] Six-item checklist per type; regen guards green.
- [ ] Component/pin desugar with bilateral split; rails scope-transparency
      test; pin `translate:` slide.
- [ ] Registry entries (bodies + label symbols + connection points); three
      invariants; unknown-symbol error consumes `suggest()`.
- [ ] Discrete generated pins incl. polar variants; `|J| { pins: 4 }`;
      opamp pin set + hidden power pins.
- [ ] Ref readout chrome + placement + minted display refs (never
      endpoints — test).
- [ ] `|label|` text/symbol/shape lowering.
- [ ] fmt for the new type names; define-shadowing / reserved-word tests
      (SPEC 23's protection rule).
- [ ] Unit snapshots per type (desugar + render); visual PNG contact sheet
      of every symbol, light + dark.
- [ ] `cargo fmt && cargo test && cargo clippy` green.

### Execution log

### Carry-over notes

---

## Phase 4 — The schematic engine: placement

**Goal**: `layout: schematic` exists — anchors on the grid-like scope,
satellite seating, auto-pose, 90°-step rotation at lowering, cluster
bboxes. Wires between placed pins route via Phase 1's fixed ports; the
scope's *link semantics* arrive in Phase 5.

**Read first**: SPEC 16; survey §3; `src/layout/{mod,arrange,grid,tree}.rs`,
`src/layout/drawing/{engine,annotate}/` (`annotate/rows.rs` closely),
`src/routing/ortho/scene.rs` (`obstacle_rects` ~:238).

**Design (settled; details yours)**:
- **Engine shape: follow `tree`, not sequence/drawing** —
  `layout/tree.rs:30 is_tree`, `:82 layout_node`, `:94 layout_root`:
  arrange in place, never consume links; the root intercept runs *before*
  the generic child loop (like tree's at `layout/mod.rs:60`), not after
  (sequence's at `:80`). New `layout::schematic::{is_schematic,
  is_schematic_scope, layout_node, layout_root}`; dispatch in
  `layout_inst`; root defaults via `root_layout_defaults(Some("schematic"))`
  (called from `desugar/mod.rs:~184`).
- **Scope semantics split** (the audit's finding — respect it):
  **placement does not cascade** — a nested `|row|`/`|grid|` inside a
  schematic places its own children, the drawing precedent
  (`layout/mod.rs:44-48`); **link reinterpretation** (Phase 5's laws)
  *does* reach nested ordinary scopes — carried like `Inherit::ScopeLink`
  properties (`ledger/properties/mod.rs:762`, consumed at
  `resolve/program/link_scope.rs:127-165`) or a scope-chain predicate;
  decide the carrier in Phase 5, but build `is_schematic_scope` here to
  answer "nearest schematic ancestor" either way. Test the boundary.
- **Anchors**: 3+-pin parts and anything with `cell:`/`translate:` —
  default one row, declaration order; `columns:` wraps; `cell:` with
  ordinal collapsing indices via the engine's **own track list** (the
  settled decision — grid's placement helpers reused, grid's semantics
  untouched). `validate.rs::container_layout` gains a
  `"schematic" => "schematic"`-style arm so `cell:` is legal in the scope.
- **Satellites**: labels + unplaced 1–2-pin parts. Chains with one placed
  end grow outward from the pin (direction = the terminator's authored
  connection geometry, auto-posed in 90° steps, deterministic tie-break —
  document it); two placed ends → distribute along the pin-to-pin straight
  line at even fractions; none → flow fallback + warning. Multiple chains
  on one pin stack in statement order. **Stacking machinery**: extract the
  shared core out of `drawing/annotate/rows.rs` (`Rows`/`SeatLine` are
  `pub(in crate::layout::drawing)` and drawing-shaped — `SeatLine::away`
  errors with dimension wording) into `layout/` and have both engines call
  it; decide whether that's `Rows` whole or a smaller primitive, and log
  it. Never copy.
- **Cluster bbox**: an anchor's placed satellites join its extent for track
  sizing (placement math only — satellites remain scope-level siblings).
- **Rotation**: schematic parts read `rotate:` in 90° steps at lowering
  (pins re-side, symbol re-lays, label/ref text stays upright); non-90° on
  a connection-bearing part errors. The router sees post-rotation geometry.
- **Router integration**: satellites and anchors register through
  `SceneIndex` as normal nodes (`scene.rs obstacle_rects`) — pin/label
  chrome folded into their owners per the settled identity decision; pins
  and label connection points feed fixed-port requests; `:side` on any
  terminal errors.
- **Pass order** (log the final version): seat satellites (pin-relative) →
  cluster extents → tracks → place anchors → absolutize satellites →
  route (fixed ports) — labels/junctions in Phase 5.

**Files**: `src/layout/schematic/` (new: `mod.rs`, `place.rs`, `seat.rs`,
`rotate.rs`, `tests.rs`), `src/layout/{mod,arrange}.rs`, `src/layout/`
(the extracted stacking primitive), `src/layout/drawing/annotate/rows.rs`,
`src/routing/ortho/request.rs` (fixed ports from pins), `src/validate.rs`,
`src/ledger/*`.

**Tasks**
- [ ] Engine dispatch + root defaults + `is_schematic_scope`; sweep the 29
      predicate call sites + near-name family; `read_layout_mode` message +
      snapshot updated.
- [ ] Anchor tracks: one-row default, `columns:`, ordinal `cell:` (+
      errors), determinism tests; `container_layout` arm.
- [ ] Stacking-core extraction (both engines green afterwards — drawing's
      1206-line annotate tests are the regression net).
- [ ] Satellite seat pass (one-end, two-end, no-end chains); stacking
      order; cluster extents.
- [ ] Auto-pose + `rotate:` 90° lowering (pins re-side; upright text);
      non-90° error.
- [ ] Pins/labels → fixed ports; `:side`-on-terminal errors.
- [ ] Nested `|row|` boundary test (places its own children).
- [ ] Layout tests in the `tree.rs`/`drawing/engine/tests.rs` style; a
      minimal end-to-end sample compiles and routes; laws sweep green;
      visual PNG check.
- [ ] `cargo fmt && cargo test && cargo clippy` green.

### Execution log

### Carry-over notes

---

## Phase 5 — Schematic wiring semantics, chrome & look

**Goal**: the scope's link laws and the classic look: label wires, pinless
arity, pass-through chains, marker gate, no implicit auto-create, duplicate
error, junction dots, square corners, scoped defaults + role variables +
theme, out-of-scope type gates.

**Read first**: SPEC 16 + SPEC 9/22 (as amended);
`src/resolve/links/mod.rs` (`:426-431` one-ended gate, `:447-452` the
leader-direction error), `src/render/links.rs`, `src/render/markers.rs`,
`src/resolve/defaults.rs`, `src/theme.rs`, `src/desugar/schematic.rs`.

**Design (settled; details yours)**:
- **Label wires**: one-ended `-`/`->`/`-<`/`-<>`/`-*` with trailing text in
  schematic scope → desugar mints `|label#lini-label-N| "text"` + the wire.
  **Desugar must intercept before the resolve gates** — today
  `resolve/links/mod.rs:426` errors one-ended wires outside drawing
  (`CHAIN_TOO_SHORT`) and `:447` errors these very ops inside drawing with
  the opposite-direction message; desugar already owns auto-create
  (`desugar/mod.rs:140`), so the interception precedent exists. Phase 0
  amended the SPEC laws; make the code match. The op's marker sets the
  label's `shape:` (the op-sets-stroke-style precedent); explicit `shape:`
  wins. fmt round-trips the new one-ended forms.
- **Marker gate**: markers legal only on wires terminating in a *text-form*
  label; marked part-to-part wires and markers at symbol-form labels error.
  Line part (`--`, `---`, `~`) stays free.
- **No implicit auto-create in scope**: bare unknown id errors with the
  quote suggestion (`did you mean - "NSTDBY" (a net label)?`).
- **Arity** (resolve): 1 pin lands; 2 pins next-free in the type's pin
  order (both taken → error naming one); 3+ errors with a pin suggestion;
  **dangling pins legal** (`|R| -> a` lands p1, p2 open). **Pass-through
  chains**: a 2-pin part mid-chain takes entry (named or next-free) and
  exits the *other* pin.
- **Duplicates**: same endpoint pair twice in scope → error. **Same-pin
  landings merge** into the implicit fan at the shared fixed port
  (Phase 1's law).
- **Junction dots**: generated chrome (`|junction|`-classed `PlacedNode`,
  the `prim::dim_marker` pattern, `layout/prim.rs:102`) at every ≥3-way
  meet read off the routed geometry (trunk splits + shared fixed ports;
  label stubs excluded); styled by one CSS rule; removable
  (`|junction| { … }` rule) — and verify removal survives the tree-shaker
  (`used_vars.rs:91-106` literal-scan constraint).
- **Square corners**: the `corner-radius` link property (settled decision)
  — schematic link defaults set 0; `radius_cap` reads it with the
  clearance-derived value as the `auto` fallback. No validator change (no
  rounding law exists).
- **Scoped defaults & look**: schematic link defaults (clearance — start
  ~8, tune in Phase 6; `stroke-width` ~1.5; `corner-radius` 0), role
  variables (wire green, part fill pale-yellow, part outline dark-red,
  label teal, **pin-number muted**, scene beige) — new `--lini-*` roles
  through the standard formatting path (tree-shake!), light/dark pairs,
  scoped rules generated at desugar (the `mindmap_rules` pattern,
  `desugar/tree.rs:~685`). A built-in `theme` may ship the KiCad-esque
  alternative.
- **Out-of-scope gates** (deferred from Phase 3): schematic types outside a
  schematic scope error (`"'|R|' belongs in a 'layout: schematic'"` — the
  `layout/mod.rs:420` family).
- **Link-reinterpretation carrier** (from Phase 4's split): decide
  ScopeLink-style attr vs scope-chain predicate; nested-scope label wire +
  junction test.

**Files**: `src/desugar/schematic.rs`, `src/resolve/links/mod.rs` (624 —
split it), `src/render/{links,markers}.rs`, `src/layout/schematic/`,
`src/resolve/defaults.rs`, `src/theme.rs`, `src/error/codes.rs`,
`src/fmt.rs`.

**Tasks**
- [ ] Label-wire desugar (pre-resolve interception) + shape-from-marker +
      marker gate + fmt + errors.
- [ ] No-auto-create + quote suggestion.
- [ ] Arity + dangling + pass-through + duplicates + same-pin merge
      (resolve tests).
- [ ] Junction chrome (+ tree-shake-safe removal test) + `corner-radius`
      + laws sweep green.
- [ ] Roles/defaults/scoped rules/theme; light + dark PNG review.
- [ ] Out-of-scope type gates; nested-scope reinterpretation carrier +
      boundary tests; `--strict` clean on the working sample.
- [ ] `cargo fmt && cargo test && cargo clippy` green.

### Execution log

### Carry-over notes

---

## Phase 6 — Samples, canon & hardening

**Goal**: the showroom, the tooling surface, and the release row.

**Tasks**
- [ ] **The hero sample**: reproduce the reference sheet (TMC2300 stepper
      driver page — `|page| { sheet: a5 landscape }`, title block, two
      captioned groups, U7 + J3 + passives + labels) as
      `samples/schematic_hero.lini`. `samples/pcb.lini` stays as-is (it's a
      *routing* showcase of plain boxes — note that in a file comment); one
      more compact sample for the discrete/label/symbol families
      (`samples/schematic_parts.lini`) only if the hero can't carry them —
      one sample per cluster, extend before adding.
- [ ] Visual pass: constants tuning (pin pitch ≥ min pitch at scope
      clearance, stub length, tag anatomy, symbol poses, clearance)
      against the reference PDF; PNG review light + dark; log every
      constant chosen.
- [ ] `fmt` canon for all new forms; all samples formatter-idempotent.
- [ ] Full regen; conformance + oracle + laws + rendering suites green;
      `--strict` clean on both schematic samples.
- [ ] SPEC cross-check: read SPEC 16 top to bottom against built behavior;
      fix drift in whichever is wrong (ask the user if the fix is
      SPEC-side).
- [ ] ROADMAP.md: the beta-2 row; README touch (the family deserves a
      line).
- [ ] `cargo fmt && cargo test && cargo clippy`; final PNG review; ready
      for release tagging by the user.

### Execution log

### Carry-over notes

---

## Deferred (recorded so nobody re-litigates)

Wire-seating of series parts along the route (flow placement accepted for
beta 2 — capsule chains hoist adjacently); the flow/grid `a -> b:port`
*surface syntax* onto sketch segments (Phase 1 builds the routing contract
that makes it buildable — the SPEC Deferred row is narrowed, not removed);
ANSI symbol standard (scope-level knob, IEC only now); logic gates,
transformer (T), relay (K), motor (M), speaker (LS), pot (RV); crossing
hop-over arcs; buses; pin electrical marks; hierarchical sheets; netlist
semantics; mid-wire label tags riding a link's `[ ]` at an `along:`
fraction.

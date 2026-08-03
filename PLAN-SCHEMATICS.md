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
- [x] New Part II section **16. Schematic** — full family: the scope, roles
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
- [x] Core amendments: SPEC 3/9 (capsule endpoints — declaration in
      endpoint position, no tail, either end, fans/chains, drawing-scope
      ban), SPEC 22 grammar section (endpoint rule gains the capsule
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
- [x] **Renumber**: Part III 16-24 → 17-25; every `[SPEC N]` cross-ref +
      anchor fragment in SPEC.md; `AGENTS.md` ("[SPEC 17]'s class-diff" →
      18); grep ROADMAP.md, README.md, and `src/`+`tests/` comments for
      `SPEC 1[6-9]|SPEC 2[0-4]` and fix.
- [x] Self-check: ToC matches, no dangling anchors (grep `](#`), the four
      SPEC-16-adjacent laws read against rounds 1-13 of the brainstorm.

### Execution log

- 2026-07-30 — Phase 0 executed in the planning session (commit c94cd2a):
  SPEC 16 landed with the core amendments (§1 §3 §8 §9 §10.1 §10.5 §11 §17
  §19 §21 §22 §23 §24); Part III renumbered 17–25 across SPEC, AGENTS,
  ROADMAP, BETA-tooling, and src/tests comments; grammar + schema artifacts
  regenerated; ROADMAP gained the beta-2 row. cargo fmt/test/clippy green.
- 2026-07-30 — Final Opus audit (5 blocking, 11 minor) verified and applied:
  anchor-role table corrected (anchors = 3+-pin or explicitly placed —
  discretes are satellites when unplaced); `|pin|` lost its `side: left`
  template default (it would have killed the bilateral split — a template
  default is a cascade Decl, so "without a side:" would never hold); ledger
  rows extended (`layout` value list gains `schematic`; `columns`/`cell`
  owners; `symbol` owners gain `|label|` + discretes); `|component|` carries
  `prefix: "U"`; "anonymous discrete" → "anonymous part"; §15.7 gained the
  schematic pointer; the `|pin|`/`pin:` homonym stated; §16.7 reworded
  (arranges in place, no subtree consumed); §11 seam 2 parenthetical
  (link laws reach nested scopes, placement never cascades); §21 gained the
  no-placed-end warning row; pin `translate:` cross-axis is an error.
  Renumber-script defects repaired: compound `[SPEC A/B]` refs were
  double-bumped (now `17/21`, `20/21`, `18/20`, `21/23`);
  `editors/zed/tree-sitter-lini/grammar.js` (→ SPEC 22) and
  `xtask/src/fonts.rs` (→ SPEC 18) were missed; the pre-existing wrong ref
  in `grammar/mod.rs` ("sides free as ids") now points at SPEC 23.

### Carry-over notes

- Baked schematic constants chosen (SPEC 10.5 — Phase 6 tunes them
  visually): pin-pitch 20, pin-stub 12, label-seat 10, junction radius 3,
  scope clearance 8 (pin-pitch must stay ≥ min pitch), wire stroke-width
  1.5, corner-radius 0. Role vars (SPEC 10.1, placeholder values — visual
  pass tunes): `--lini-wire`, `--lini-component-fill`,
  `--lini-component-stroke`, `--lini-label-ink`, `--lini-pin-number`,
  `--lini-sheet`.
- Phase 1 must title its ROUTING.md section exactly **"Fixed ports"** —
  SPEC 16.5 and SPEC 24 already cite it by that name.
- A pin's `translate:` reads its along-side component only; a cross-axis
  component errors (SPEC 16.2) — build in Phase 3/4.
- SPEC 16's role table now defines anchors as *3+-pin or explicitly
  placed* — an unplaced 2-pin authored `|component|` (a jumper) is a
  satellite; keep Phase 4's classifier on pin arity + placement, never on
  the type.

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
- [x] ROUTING.md: "Fixed ports" section — vocabulary, Law 2 amendment,
      fan-at-fixed-port law, infeasibility-is-loud contract, determinism
      unchanged. Tight, lawful prose in the document's voice.
- [x] `EdgeReq` fixed-port field + plumbing (nothing sets it yet except
      tests).
- [x] `entries` point-window path incl. blocked-port stray; `chain_prefs`
      fixed prefs; both-end/pooling infeasibility errors (no release
      clamps); `merge_fans` conflict error; capacity adjustment.
- [x] Validator: `landing()` fixed-port waiver + exact-port check; carrier
      for the ordinate.
- [x] Tests in `tests/routing.rs`: fixed port lands exactly (± ε); two
      fixed ports on one side don't braid; fixed + free mix ladders around
      the fixed one; same-point fan merges; conflicting fan errors; fixed
      ports closer than min pitch error; blocked fixed port → named stray;
      determinism (byte-identical rerun). `tests/laws.rs` sweep green
      including the low-clearance end (6.0) with fixed ports in play.
- [x] `cargo fmt && cargo test && cargo clippy` green.

### Execution log

- 2026-07-31 — Phase 1 executed (branch `worktree-fixed-port-routing`).
  ROUTING.md gained **"Fixed ports"** (between Special nodes and Impossible
  layouts) + a Law-2 pointer and a stray-reason row. Decisions:
  - **Plumbing**: `ResolvedEndpoint.port: Option<f64>` (port ⇒ forced side,
    debug-asserted at request build) → `EdgeReq.port_a/port_b` (+`port(end)`
    accessor). Test surface: `testing::route_sample_with_ports(src,
    clearance, &[(from, to, at_path, side_str, ordinate)])` injects onto
    resolved endpoints — Phase 4 replaces the injection, not the plumbing.
  - **Entries**: the fixed ordinate moves the punch to the pinned point and
    collapses the window to `(f, f)`; an ordinate off the side yields no
    entry. `entry.rs:76`'s inverted-window drop now ends in a named stray:
    the route loop tracks pre-filter emptiness per fixed end and, when every
    world fails, strays "fixed port blocked: a body covers the port's
    landing" instead of the generic NO_ROUTE.
  - **Fans**: `request::fan_groups` gained a second bucket pass keyed
    `(path, side, port.to_bits())` — same-point landings merge into one
    implicit fan **across statements and across ends** (`a - p` B-end and
    `p - b` A-end are one landing; duplicates ride one line). The
    member-move is one helper (`absorb`), shared with the containment-arms
    pass. Equality is bit-exact by design (ports come from one
    connection-geometry computation). A fan whose members disagree strays
    whole ("fan ends carry two different fixed ports") via a pre-pass in
    `ortho::route` — chose **stray over compile error**: routing never
    errors on geometry (the stray contract), and the surface syntax can't
    author the conflict once pins exist.
  - **Too-close**: a per-bundle pre-check in `ortho::route` against a local
    `landed_ports` record — the later of two fixed ports under min pitch on
    one `(path, side)` strays "fixed ports closer than the minimum pitch on
    one side"; equal ordinates are the fan, never a collision. The **ledger
    needed no capacity change** (verified: pins are their own endpoint
    paths, and shared-port landings fan and count once); `admit` needed
    nothing — point windows ride `Item.window` into the probe's real
    placement.
  - **Clamps closed**: `ladder()` → `Option<Vec<f64>>` (None on a crossed or
    pooled-crossed box — was a debug_assert + release clamp to `hi`);
    `settle` falls through to the pairwise solver, whose bounds-win behavior
    keeps windows absolute. `natural`'s port ladder `expect`s feasibility
    (its pitch derives from the window). `merge_fans` debug-asserts window
    intersection (conflicts stray upstream). `chain_prefs`'s shared-window
    assert stands — the search's `fits` jogs unequal fixed pairs, so the
    single-run branch never sees them.
  - **Validator carrier**: `RoutedLink.port_from/port_to:
    Option<(Side, f64)>`, filled by ortho from the chain ends, `None` in
    every other strategy. `landing()` gained the fixed parameter: exact
    side + ordinate check (EPS 1e-6), corner-margin rule waived. `excuse.rs`
    reads the carried port as the end run's lawful range `(f, f)`, so pinned
    pairs between min pitch and clearance are excused by the *existing*
    scarcity walk — no second excuse mechanism.
  - **Split** (the plan's `place.rs` order): the contention model — `Item`,
    `clusters_of`, `merge_fans`, `contend`/`owed`/`near` — moved to
    `ortho/cluster.rs` (201 lines); `place.rs` 848 → 659 (≈380 sans tests)
    keeps prefs, bounds, relief, and the settle dispatch; `admit`/`pairwise`
    import the model from `cluster`.
  - Tests: entry point-window + blocked (unit), fan-merge unit, ladder
    infeasibility unit, validator waiver/exact/excuse units, and 9
    integration tests in `tests/routing.rs` (exact landing, two ports one
    side, fixed+free ladder ≥ floor, same-point fan with shared `fan_to`,
    conflicted fan strays ×2 named, too-close strays the later wire, blocked
    port named, determinism ×25, laws sweep [6, 8, 9, 10, 12, 13, 16] with
    every edge drawn-or-reported). Full suite 1117 green; fmt + clippy
    clean.

### Carry-over notes

- **The fixed-port ordinate is an absolute scene coordinate** on the forced
  side, and merging is **bit-exact**: Phase 4 must compute each pin's
  stub-tip ordinate **once** and reuse that value for every wire touching
  the pin (never recompute per wire — float drift would split the fan).
- **Phase 4's seam is the request builder**: `Program` is immutable at
  layout, so the pin pass cannot write `ResolvedEndpoint.port`. Derive the
  endpoint's `(obstacle rect = component body, forced side, stub-tip
  ordinate)` in **one** place feeding `request::requests()` (it already
  reads the `SceneIndex`), and keep `ResolvedEndpoint.port` as the carrier
  the testing hook uses. Don't build a second path — extend `requests()`.
- Two fixed ports **between min pitch and clearance** apart draw lawfully
  (validator excuses via the point windows); only sub-min-pitch strays. The
  schematic constants (pin pitch 20 ≥ min pitch at clearance 8) keep real
  sheets clear of both edges — Phase 6 tunes.
- A self-loop on one pin needs no new code: port ⇒ forced side puts both
  ends on one side → the existing ONE_SIDE_LOOP stray.
- `Severity` for all three new failure shapes is the standard
  `Rule::Impossible` warning — `--strict` escalates as usual; no new error
  codes were needed (routing reports, resolve errors — Phase 5's scope
  errors will use `src/error/codes.rs`).
- `validate.rs` is now 868 lines (was 803) — over the split rule; Phase 5
  touches the validator's neighbourhood (junction chrome, corner-radius):
  split it there if it grows again.

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
- [x] Extend `tests/desugar.rs` source-idempotence to a samples sweep
      (pre-work — see the fixed-point invariant).
- [x] Parser: capsules both positions; the three `Ident`-gate sites; the
      `classify.rs:52` split; `|a| || |b|`, `a - |gnd| - b`, spacing cases.
- [x] Desugar hoisting (`desugar/capsule.rs`), minted `lini-cap-N`,
      fixed-point sweeps green.
- [x] Resolve: endpoint rewrite before path resolution; drawing-scope gate;
      sequence allowed; `.p4`-on-inline error.
- [x] fmt round-trip + `tests/fmt.rs` cases; `lini desugar` output of a
      capsule statement re-desugars byte-identically.
- [x] Grammar/schema regen; `tests/{parsing,desugar,resolution}.rs`:
      `a -> |cyl#db| "watches" { stroke: red }` (tail is the link's),
      `|cyl| -> a`, `a -> |cyl| -> c`, `a & b -> |cyl|` (one instance),
      capsule + `.path`/`:side`/`.index` composition, drawing-scope error.
- [x] `cargo fmt && cargo test && cargo clippy` green.

### Execution log

- 2026-07-31 — Phase 2 executed (branch `worktree-capsule-endpoints`).
  Decisions:
  - **Pre-work sweep caught a real break**: `entity_hero.lini` was not a
    fixed point — the generated `.lini-align-start` rule was folded back as
    an "extra" class def *and* regenerated by the `ALIGN_CLASSES` path,
    doubling on re-desugar. Fixed: the stylesheet walk drops incoming
    align-class rules (`classes::is_align_class`); they regenerate from the
    worn set. The sweep (`every_sample_is_a_byte_identical_desugar_fixed_point`)
    now guards all 30+ samples.
  - **AST carrier**: `Endpoint.capsule: Option<Capsule>` (`ty`/`id`/`span`;
    `path` holds only the segments *after* the bars) plus
    `Endpoint.from_capsule: Option<String>` — the hoist's residue (the
    written type name), resolve's error-message hint. The hint is never
    printed, so it does not survive a print/re-parse round trip — a
    re-resolved `lini desugar` output gives the generic walk error
    (accepted: messages only).
  - **Parser**: `parse_endpoint` reuses `parse_identity(BarsCtx::Instance)`
    (so `a -> |-|` and `a -> |x::box|` keep their existing wordings); the
    two chain gates admit `Pipe`; classify's Pipe arm splits via
    `capsule_width_at` + `capsule_heads_link` (walks the glued dotted run,
    then op / `&` / mate / `:point`+op ⇒ link). `|box| .hot` (spaced) and
    the pre-existing lenient `|box|.hot` (glued) both stay node statements —
    a dotted run reads as a link only when an op follows it.
  - **Hoist is a per-scope step inside the main walk**, not a pre-pass
    (`desugar/capsule.rs::hoist`): root between the tree build and
    auto-create; bodies in `lower_node` over the combined define-body +
    own raw links (auto-create still reads only the node's own slice —
    define-link auto-create semantics unchanged). Declarations lower
    through the one node path; **minted ids are stamped post-lowering**
    (the topic pattern), so the reserved-prefix check still rejects an
    authored `|box#lini-x|` capsule. Mint = `lini-cap-N`, 1-based in
    statement order, **skipping taken names** — a lowered scope gaining a
    new capsule can never collide. Declaration span = the capsule's span,
    so fmt interleaves it at the statement's position (the auto-create
    precedent); byte-idempotent.
  - **Drawing gate lives in the hoist** (it has the per-scope `in_drawing`
    the chrome/auto-create classification already computes; a sealing
    `|row|` inside a drawing allows capsules, matching its routed links):
    "a drawing never invents an endpoint — declare the node, then annotate
    it", new code **R016 capsule-in-drawing** (catalog + pinned snapshot).
    Sequence scopes hoist normally — a typed participant.
  - **Pre-hoist consumers reconciled**: `auto_created_ids` skips capsule
    endpoints and counts their ids as declared (one shared function, so the
    lint's pre-hoist view matches the lowering); the pinned-mates lint
    skips capsule endpoints; tree's `same_link` treats any capsule endpoint
    as unequal.
  - **Resolve**: `endpoint_error` words the inline-path failure from
    `from_capsule` — `'|cyl#u9|.p4' — an inline cyl has no authored pins`
    (SPEC 21's shape with the written type interpolated; when Phase 3 lands
    `|component|` it reads exactly as the SPEC row). An inline define's
    intrinsic children resolve normally (`x -> |room#r2|.inlet` works).
    Duplicate capsule ids fall into the ordinary duplicate-id error.
  - **Editor grammars unchanged**: the textmate patterns and the
    tree-sitter token soup highlight bars position-independently; no
    TEMPLATES/PROPERTIES rows changed, so the schema/grammar byte-guards
    pass as-is.
  - Tests: 16 parser units, 10 desugar integration (samples sweep, hoist
    forms, mint-skip, define-body ×2 materialization, drawing error), 9
    resolution (nested + root drawing gates, sequence participant,
    duplicate ×2, inline path, sided capsule, reserved id), fmt canon
    round-trip + glued-op normalization, parsing sweep. Suite 1153 green;
    fmt + clippy clean; PNG visual check of a capsule scene (cylinder +
    minted mid-chain box, wires routed) done.

### Carry-over notes

- **Phase 5's label-wire desugar** (`U7.DIAG - "NSTDBY"`) wants the same
  seam capsule hoisting took: the per-scope raw-links step in
  `desugar/mod.rs` (root + `lower_node`), which sees statements pre-split
  with the drawing/scope flags at hand — add the minting as a sibling
  transform beside `capsule::hoist`, not a new walk.
- `from_capsule` is an in-memory hint only; after `lini desugar` →
  re-resolve, the inline-path error degrades to the generic unknown
  endpoint. Accepted for beta 2 — revisit only if tooling needs it stable.
- A capsule inside an **id-less** sequence frame hoists into the frame's
  children and stays visible through container transparency; an id'd frame
  would scope it away — untested edge, worth a gate if frames ever take ids.
- `|box|.hot` (glued first class) still parses as a worn class — a
  pre-existing leniency the classify change deliberately preserved. If the
  canon ever tightens, `capsule_heads_link` is the decision point.
- Phase 3's `|gnd|`/`|nc|`/discrete capsules: everything here works the
  moment the types exist (`a & b -> |gnd|` currently errors only as
  "unknown type 'gnd'").

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
- **Style rides classes, never inline `style=`** (user directive, 2026-07-31
  — AGENTS.md's class-diff rule, restated for this phase's chrome): every
  generated schematic feature with a shared look (pin names/numbers, ref
  readouts, tag outlines, symbol bodies, stubs) wears a generated class
  whose paint states **once** as a CSS rule; the emitted SVG carries
  `style=` only for an element's authored *diff* against its rules. Lower
  chrome as typed/classed children riding rules (the `|caption|` pattern),
  never as per-node inline decls that render as `style=`. Desugar lowers
  **as much as can be lowered** — anything expressible as generated
  nodes/classes/rules lands in desugar, not later phases.

**Files**: `src/desugar/{types.rs,schematic.rs(new)}`,
`src/ledger/{defaults,properties/mod,consts,examples}.rs`,
`src/glyph/mod.rs` (+ maybe `glyph/schematic.rs`),
`src/layout/schematic/symbols.rs` (new, sibling of drawing's),
`src/render/stylesheet/families.rs`, `src/validate.rs`,
`src/error/codes.rs`, regen.

**Tasks**
- [x] Six-item checklist per type; regen guards green.
- [x] Component/pin desugar with bilateral split; rails scope-transparency
      test; pin `translate:` slide (deferred to Phase 4 — see log).
- [x] Registry entries (bodies + label symbols + connection points); three
      invariants; unknown-symbol error consumes `suggest()`.
- [x] Discrete generated pins incl. polar variants; `|J| { pins: 4 }`;
      opamp pin set + hidden power pins (hidden = not generated; see log).
- [x] Ref readout chrome + placement + minted display refs (never
      endpoints — test).
- [x] `|label|` text/symbol/shape lowering.
- [x] fmt for the new type names; define-shadowing / reserved-word tests
      (SPEC 23's protection rule).
- [x] Unit tests per type (desugar + resolve); visual PNG contact sheet of
      every symbol, light + dark.
- [x] `cargo fmt && cargo test && cargo clippy` green.

### Execution log

- 2026-07-31 — Phase 3 executed (branch `worktree-schematic-types`, rebased
  onto Phase 2). **User directive recorded this session**: desugar lowers as
  much as possible, and every shared look rides a generated class + one CSS
  rule — no inline `style=` in the SVG beyond an element's authored diff
  (AGENTS.md's class-diff law, restated in this phase's Design). Decisions:
  - **All 23 types registered** through the six-item checklist:
    TEMPLATES rows (`schematic`·`component`·`pin`·`label`·`junction`·`J`·
    `opamp`·`gnd`·`nc` + 13 discretes — `DISCRETES` const exported beside
    TEMPLATES, the one list validation/lowering key off), bundles
    (component pale-yellow/dark-red per SPEC 8; `pin` height = PIN_PITCH 20
    so gap-0 rows stack on exact pitch centres; `junction` diameter from
    JUNCTION_RADIUS), PROPERTIES rows (`number`/`prefix`/`shape`/`pins` +
    `corner-radius` on Link for Phase 5; `symbol` gains Type("label") +
    Role("discrete"); `side` gains Type("pin")), examples, validation role
    `discrete`, schema + grammar regen. `|schematic|` stays a plain block
    until Phase 4's engine (no `layout: schematic` value yet).
  - **Role vars** wired in `resolve/defaults.rs` (`wire`, `component-fill`,
    `component-stroke`, `label-ink`, `pin-number`, `sheet` — SPEC 10.1
    values, tree-shaken like every var). Constants in `ledger/consts.rs`
    (`// schematic` section): PIN_PITCH 20, PIN_STUB 12, JUNCTION_RADIUS 3,
    SCH_STROKE_WIDTH 1.5, PIN_NUMBER_FONT 9, REF_FONT 11.
  - **Symbols live in the `glyphs!` registry**, extended: `Glyph` gains
    `height` and `ports` (connection points, **in pin order** — the settled
    connection-point decision). 30 `sch-*` glyphs (13 discrete families ×
    variants + opamp + 6 label symbols), authored at **real sheet size**
    (pitch constants — the explicit not-font-coupled law), one `Line`
    fragment each; drafting glyphs keep GRID sizing and empty ports. New
    invariant test: every `sch-` port inside its box, exactly one Line frag.
  - **The `Lower` context** (`desugar/mod.rs`): `lower_node` &co. now
    thread one `Lower { types, bodies, rules }` — `chain_ident/number/str`
    give desugar its slice of the cascade (own style → element rules →
    template bundles, derived-first), so `symbol:`/`prefix:`/`pins:` read
    through defines (`|vm::label| { symbol: power }` works). Descendant /
    class-rule values are invisible to desugar — accepted stage boundary.
  - **`desugar/schematic.rs`** (new, ~470 LOC): `sch_kind` dispatch
    (Component / Opamp / Discrete / Label); bilateral split into anonymous
    `|row|`/`|column|` rails (top/(left·right)/bottom, autos ⌈n/2⌉ left,
    explicit `side:` excluded from the count); per-pin chrome (stub `|line|`
    pinned outward past the body padding, `number:` readout, id-as-name
    text); `|J| { pins: N }` generation; symbol bodies = the glyph's one
    fragment as a `|path|` + zero-size **port nodes** (`p1`, `a`/`k`,
    `b c e`/`g d s` per variant, `plus`/`minus`, opamp `out inp inn`) pinned
    at the glyph's ports; label symbol drawing + `shape:` outline classes
    (`round` = stadium; `left`/`right`/`both` draw the plain outline until
    Phase 5's marker-driven tag path); per-scope display-ref minting
    (id verbatim, else prefix+N skipping taken, `prefix:` through the
    chain, discrete type name as fallback prefix).
  - **The class-diff law implemented via `lowered_chrome`**: every
    generated chrome child lowers through the one node path then seats its
    chrome class **first** (most-derived), so the class rule wins the
    type-tier fold and the emitted SVG carries **zero** `style=` (verified
    by test `a_schematic_scene_emits_no_inline_style` and by hand on the
    contact sheet). Chrome classes: `lini-sch-line`, `lini-sch-tag-line`,
    `lini-pin-stub`, `lini-pin-number`, `lini-ref`, `lini-part-value`,
    `lini-tag-outline`, `lini-tag-round` — looks in `SchChrome` +
    `sch_chrome_decls` (ledger/defaults.rs, the one tuning home), emitted
    when worn, regenerated per pass (`is_align_class` generalized to
    `is_generated_class`, which drops incoming copies — same fix shape as
    Phase 2's align-class bug).
  - **Anonymous parts generate no port nodes** — found by the contact
    sheet: an anonymous part is scope-transparent, so two anonymous |R|s'
    generated `p1` ids collide in the parent scope; they are also unwirable
    (no dot-path). Only id'd parts get terminals (`symbol_body`'s `wired`).
  - **Opamp power pins**: "present but hidden" = **not generated** in beta
    2 — no reveal knob; the deferred ANSI-knob row is the natural home if
    one emerges. Logged as the decision the plan asked for.
  - **Pin `translate:` slide + cross-axis error deferred to Phase 4** —
    the slide is placement semantics (re-siding under rotation), dead code
    until the engine reads pin geometry; Phase 4 must build both.
  - Contact sheet (30 symbols, light + dark PNG) reviewed: family reads at
    even weight, refs/values/net-symbols correct in both themes. Constants
    untouched — Phase 6 tunes against the reference PDF.
  - Tests: +10 desugar (split/rails, J pins, discrete variants + suggest,
    labels, power-flag define, ref minting, fixed points, anonymous-part
    ports), +5 resolution (rail scope-transparency incl. semantic pin ids,
    minted-ref-never-endpoint, shadow protection, no-inline-style), glyph
    invariants. Suite 1167 green; fmt + clippy clean.

### Carry-over notes

- **Phase 5 must gate schematic types out of non-schematic scopes** — today
  `|R|` renders anywhere (deliberately: no engine exists). The
  `layout/mod.rs:420` family arm is the deferral this phase's goal
  statement promised.
- **Phase 4 reads pin geometry from what desugar built**: a component pin's
  fixed port = its rail row's stub tip (PIN_STUB past the body edge on the
  pin's side); a symbol part's = its port node's `translate` (the glyph
  port, bit-exact — one computation, reused per wire, per Phase 1's
  carry-over). The zero-size port nodes are scope-level **children** of the
  part; the settled obstacle-identity decision (pins fold into the part's
  rect) is Phase 4's to implement in `SceneIndex`.
- **Pin `translate:` slide + cross-axis error and `rotate:` re-siding are
  unbuilt** — Phase 4 owns both (log said so above).
- `|label|` `shape: left|right|both` draw the plain outline; Phase 5's
  label-wire marker mapping should land the pointed tag geometry (a
  `|path|` outline like the symbols, or a clip — decide there).
- The value readout anchors (`component`: above under the ref at −16/−30;
  discrete: below at +12) are provisional chrome geometry — Phase 4's
  seat/cluster pass may re-anchor them (SPEC says component-above,
  discrete-beside; "beside" needs cluster knowledge desugar lacks).
- `|junction|` is registered + bundled but never generated — Phase 5 reads
  routed geometry for ≥3-way meets.
- `corner-radius` row exists, no consumer — Phase 5 wires `radius_cap`.
- `desugar/mod.rs` is now ~780 LOC (over the split rule) — Phase 4/5
  touching it should split the smart-label lowering out (log the split).

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
- **Anchors**: 3+-pin parts and anything with `cell:` (**corrected in
  execution** — SPEC 16.1's body sentence is normative: `cell:` promotes,
  `translate:` only nudges from the seat; the role table's
  "`cell:`/`translate:`" is its loose summary) —
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
- [x] Engine dispatch + root defaults + `is_schematic_scope`; sweep the 29
      predicate call sites + near-name family; `read_layout_mode` message +
      snapshot updated.
- [x] Anchor tracks: one-row default, `columns:`, ordinal `cell:` (+
      errors), determinism tests; `container_layout` arm.
- [x] Stacking-core extraction (both engines green afterwards — drawing's
      1206-line annotate tests are the regression net).
- [x] Satellite seat pass (one-end, two-end, no-end chains); stacking
      order; cluster extents.
- [x] Auto-pose + `rotate:` 90° lowering (pins re-side; upright text);
      non-90° error.
- [x] Pins/labels → fixed ports; `:side`-on-terminal errors.
- [x] Nested `|row|` boundary test (places its own children).
- [x] Layout tests in the `tree.rs`/`drawing/engine/tests.rs` style; a
      minimal end-to-end sample compiles and routes; laws sweep green;
      visual PNG check.
- [x] `cargo fmt && cargo test && cargo clippy` green.

### Execution log

- 2026-07-31/08-03 — Phase 4 executed (branch `worktree-schematic-engine`,
  ff25fd7 → b16c78a + close-out, 19 commits) as seven reviewed tasks, each
  with an adversarial task review and scoped re-review, then a whole-branch
  final review + one fix wave. Decisions and surprises:
  - **Engine shape as planned**: `layout/schematic/{mod,place,seat,hints,
    terminal,ports}.rs` + tests split three ways; tree-style root intercept
    before the generic loop; dispatch in `layout_inst`; 29-site predicate
    sweep executed with per-site rationale (task-1 report). `read_layout_mode`
    message now names the six engines (fix wave M1).
  - **Stacking core**: the smaller primitive, not `Rows` whole —
    `layout/stack.rs` (`Stack`/`SeatLine`/`Band`) + `layout/geom.rs`
    (`P`/`Frame`/`project`), drawing keeps extent/paint/away wording;
    `path_bbox.rs` → `path_data/` with an exact quarter-turn shared by
    desugar and layout.
  - **Tracks**: engine-own ordinal list (sort+dedup collapse, binary-search
    index); grid's `cumulative`/`read_cell` reused via visibility only —
    grid semantics untouched. `cumulative_gaps` added by the fix wave so
    spanning chains size the space between their anchors' tracks (C2).
  - **Role law adjudicated**: `cell:` promotes a satellite to an anchor;
    `translate:` only nudges (SPEC 16.1 body sentence normative — reviewer-
    adjudicated; the design bullet above is corrected). One `family::role`
    with three readers; one `chain::placed_ends` filter for pose chooser,
    seat pass, and hints (fix wave C1 closed the third divergent reader).
  - **Rotation at desugar**: `desugar/pose.rs` (`Pose`, exact quarter-turns,
    swap-and-negate — byte-stable); pins re-side via the reading-vector flip
    law; texts stay upright structurally; landed `side:` rewritten onto the
    pin; pin `translate:` slide reads the same cascade slice the pose reads
    (R017 schematic-pose, R018 pin-slide). **Auto-pose decides at desugar**
    (Program is immutable at layout — no second applier can exist):
    `autopose::choose` writes `rotate:` before lowering through the one
    take/apply path; `Pose::ALL` is the tie-break; byte-equality with an
    authored `rotate:` is test-pinned.
  - **Seat pass order as planned**: classify → seat pin-relative (grow by
    the terminator's connection geometry through `Stack`) → cluster extents
    → tracks (+ spanning demands) → place anchors → nudge anchors → 
    absolutize satellites → satellites' own nudge → flow fallback +
    Y007-named warnings.
  - **Fixed ports**: ports computed once per part in the scene walk
    (`scene/parts.rs` bridge; obstacle folding = descendant paths alias the
    part's rect; stubs/chrome extend the frame) — bit-exact by construction,
    selected not recomputed per wire; `request::fixed` consumes them,
    Phase 1's injection hook untouched. Terminal law (`:side` errors) keys
    on *scope* + address, not type (fix wave I2). Self-loop on one pin =
    the existing ONE_SIDE_LOOP stray, test-pinned.
  - **Two SPEC-side corrections landed**: `label-seat` 10 → **20**
    (SPEC 10.5 edited — a seat is a routing corridor; free width is
    `gap − 2 × clearance`, so 10 at clearance 8 could never route; 20 is
    the one unjustified constant moved). `excuse.rs` fan-sibling contention
    exemption removed (monotonically permissive; trunk still guarded by
    `separation`'s fan_pair; both directions pinned — a still-breaching
    synthetic case and the actually-flipped EXPECTED-EXCUSED case with its
    measured cost).
  - **Pre-existing Phase 1 defect fixed en route** (found by the final
    review "beyond the plan"): `cluster.rs` `merge_fans` debug_assert
    ("fan windows must intersect") was over-asserted for free-window fans —
    replaced by a total merge (valid intervals, deterministic first-window
    order); no surface repro existed (1500+ scene sweep) but the mutation
    test pins it.
  - `samples/schematic.lini` (root sheet: LDO with a `rotate: 180` header,
    auto-posed power flag/cap/grounds, capsule ground, net-name wire
    labels) — fmt-idempotent, `--strict` clean, in the desugar fixed-point
    sweep and the laws-at-every-clearance sweep; visual PNG verified light
    + dark by two independent agents. `label_body`/`symbol_body` glyph
    seating unified (`seat_glyph`, glyph ahead of authored text — the
    span-order transparency bug).
  - Suite grew 1117 → 1254; insta run log (`.pending-snap`) untracked and
    gitignored.

### Carry-over notes

- **Phase 5 must re-order hoist-then-pose**: `autopose::choose` runs before
  `capsule::hoist`, so capsule-hoisted satellites (`u1.a - |gnd|` — SPEC 16's
  own idiom) and define-body links/children are never auto-posed; the
  sample's capsule ground is right only because a bottom pin suits the
  default pose. The autopose module doc names the gaps precisely.
- **Cross-scope wires into a nested sheet don't see pins**: the terminal/
  fixed-port gate keys on the *wire's* scope (fix wave I2); a root wire into
  a nested schematic lands on the part's box (pre-fix it strayed). Phase 5's
  link-law carrier (`is_schematic_scope` is built and tested for it) should
  settle cross-scope semantics deliberately.
- **A nested `|schematic|` node's interior has no routing margin** — wires
  that must leave the parts' bbox stray; root sheets are fine. Phase 5/6:
  give the scope's container a padding/clearance answer.
- **Seat-model residuals for Phase 5/6**: same-pin chains stack collinearly
  (a decoupling cap sits behind a power flag); spanning chains still don't
  pack against same-pin stacks; a third placed end on one chain warns (Y007)
  and drops; vertical spans between stacked components can still stray at
  defaults (wants the router's own diagnosis); `Seats::cluster` measures raw
  bbox while grow/step measure `drawn()` — unify the extent notion.
- **Phase 5 gate reminders**: schematic types outside the scope still render
  (Phase 3's deliberate behaviour — the `layout/mod.rs` family arm is
  Phase 5's); `- u1` on a component gets NO fixed port (not "first pin") —
  arity work builds on that; `pin:`+`cell:` silently drops the cell (Y005 in
  hand); junction chrome reads routed geometry; `corner-radius` row exists
  unconsumed (`radius_cap`).
- **Perf note for Phase 6**: the router is superlinear on sheet size
  (10/20/40 parts → 0.24/1.98/7.4 s release); the hero sample will feel it —
  budget a profiling pass. Debug-build sweeps of big sheets are slow.
- Oversized files still over the ~500 rule: `desugar/mod.rs` ~866,
  `validate.rs` 798, `layout/mod.rs` 629, `ledger/defaults.rs` 820,
  `ledger/properties/mod.rs` 900 — split when a phase next grows them.

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

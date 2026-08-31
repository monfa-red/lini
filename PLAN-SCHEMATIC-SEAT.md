# PLAN — Schematic seat pass, spans, and router perf

Goal: `samples/schematic_hero.lini` reads like a real sheet (reference:
~/Desktop/fadec.pdf — KiCad A4 sheets). The router is sound (0 warnings,
median detour 1.00 on the hero); the damage is in the **seat pass** and two
contained routing gaps. Diagnosis re-done from scratch; the earlier agent's
list was right in outline but wrong or shallow in the marked places.

Baseline: 862a885, 1512 tests green. Render check:
`./target/release/lini --static samples/schematic_hero.lini -o /tmp/sh.svg && resvg /tmp/sh.svg /tmp/sh.png --zoom 1.2`
Repros in /tmp/lini-schematic-diag/ (disposable; each is ~12 lines).

## Root causes (verified by instrumentation, LINI_DEBUG_SEAT prints in seat.rs — strip before commit)

1. **Chain growth is not monotone** (bug_seat). Chain member order is
   CORRECT ([R2, D2, gnd]); `Stack::seat` is first-fit-innermost, so a
   narrow later member (the gnd) tucks into a hole opened before an earlier
   member (R2 was pushed deep by a neighbouring chain's band overlapping its
   readout text). Fix: within one chain, each member's seat base = previous
   member's outer paint edge (monotone growth), in `Seats::grow`.

2. **Share/ladder feedback loop** = THE sprawl (satellites 400–800 px out:
   C7/SW1 at along=775.9 in the MCU region, buck column at 417.8). The
   share rule (same-pin chains take one lane) fights the ladder (each chain
   in a (anchor,ray,side) group steps past the previous one's reach); the
   fixpoint loop runs held.len() rounds, each adding ~45 px. The share
   intent (one lead, split once) is only right for chains leaving one pin on
   **different rays** (flag up + cap down). Same-pin **same-ray** chains must
   ladder side by side (real sheets: two adjacent drops off NRST). Fix:
   share only across differing rays; ladder unchanged. (The earlier agent
   called this "not in scope / design question ~1100px lane growth" — wrong,
   it is a bug.)

3. **Two pins of one part can't be wired** (bug_samepin, `U5.NRE - U5.DE`):
   paths differ so it is not a self-loop; `solve`'s end_entries adds the
   PARTNER's rect as a blocker, but the partner rect IS the same body →
   punch blocked instantly → "fixed port blocked". Fix in
   src/routing/ortho/mod.rs: skip the partner blocker when
   `rep.a_rect == rep.b_rect`. ROUTING.md Fixed-ports clause: two distinct
   fixed ports on one side of one body are a lawful pair (U-route), distinct
   from a self-loop (same path). Does NOT touch cost/laws.

4. **Growth ray anti-parallel to the pin normal** (bug_pintop): a gnd
   terminator says "down" off a `side: top` pin; lead computes 0 (normal ∥
   ray) → treated as straight growth → satellite stacks THROUGH/below the
   body, wire strays. Fix: one shared rule (autopose + seat read it from one
   helper): terminator ray yields to the pin's outward normal when
   anti-parallel; members then pose against the flipped ray (an inverted
   gnd above a top pin is sheet-idiomatic).

5. **2-port glyph facing** — `sch-l` ports sit on box corners → `facing()`
   ties → None → L1 never posed, chains through it garble. Fix: a 2-port
   glyph's terminal faces along the port-to-port axis (dominant axis, away
   from the other port); 3-port keeps nearest-edge. Kills the route_tests
   comment that documents the limitation.

6. **Branching chains linearize by BFS** → the buck's 5V flag lands
   mid-stack upside-down. Fix (taps): a **single-member symbol-label leaf
   that is not the chain's terminator** becomes a side TAP: seated off its
   attachment member's terminal along its own natural ray (power up, gnd
   down), unposed; if that ray is anti-parallel to the trunk's ray, it goes
   to the outward (+u) side instead, posed to face the junction (sideways
   rail flags are idiomatic — see fadec). Multi-member side branches stay in
   the BFS stack (now monotone) — documented limit.

7. **Two-placed-end chains** (bug_twoend/bridge — the sheet's worst wires):
   - Same-anchor, same-side pair (`U2.EN - R5 - U2.VIN`): currently grows
     off the first pin along its normal with the member posed facing back →
     4-turn loop around the member. Fix: bridge treatment — lane out along
     the shared side's normal (ladder-assigned), members posed ALONG the
     side axis, inbound terminal toward the first-named pin (pin order on a
     side is static), distributed between the two pins' ordinates.
   - Two anchors (`J1.p1 - F1 - Q1.s`): members unposed + raw chord through
     bodies. Fix: pose members along the span's dominant axis, direction
     A→B — derived STATICALLY from the anchors' ordinal slots (share the
     slot-assignment algorithm between desugar and place.rs — one function,
     neutral inputs; autopose may not re-implement it). Distribute along the
     chord CLIPPED to clear both anchors' drawn extents + seat gap (empty
     clip → full chord fallback). Update seat::Demand/place::charge to the
     clipped need.

8. **`|J|` bilateral split is wrong** vs. real usage: connectors are one
   column (fadec: every header/terminal block single-sided). Fix: `pins: N`
   generated pins default `side: left`; author rotates 180 when the
   connector sits on the sheet's left edge. SPEC 16.2 note. Sample: add
   `rotate: 180` to J1 (+ comment).

9. **Component readouts overprint top-rail pin numbers** (bug_pintop):
   `readout_at` pins ref/value at the body top ignoring the top rail band
   (stub + number). Fix: when the (posed) component has top-side pins,
   raise the readout by the rail band. Same check for bottom? — ref/value
   only go top on components, so top band only.

10. **Ortho search perf** (LAST, re-measure first): full-drain Dijkstra, no
    goal bias. Hero ortho 0.35s vs natural 0.03s. Fix: A* on the L1 lower
    bound (min over goal tips; consistent) + terminate once every
    (goal cell × dir) state is settled or heap empty. Store g in `best`,
    f only in the heap. Cost function and tie-breaks unchanged;
    tests/laws.rs + tests/routing.rs must stay green UNTOUCHED. Equal-cost
    path selection may shift (snapshots may move; laws must hold). If it
    can't be made faster without touching cost — stop, report.

## Non-issues (checked)
- Router choices: sound (0 warnings, detour 1.00) — do not restructure.
- "3 chains on a pin cliff", "declaration order decides routability":
  refuted, don't chase.
- Syntax: the language reads right; no grammar changes needed beyond #8.

## Commit plan (one bug per commit; after each: re-render hero + LOOK,
cargo test, insta accept for moved conformance snapshots, fmt, clippy)

- [x] A. routing: same-rect fixed-port pair routes (fix #3 + ROUTING.md + tests) [32c7fb3]
- [x] B. [0dfacba] seat: monotone chain growth (#1) + share-vs-ladder de-feedback (#2) + seat tests
- [x] B2. [04ea703, with C/D/E] **facing-pin row/column alignment across anchors** (NEW — found after B):
      cluster centering offsets neighbouring anchors' pin rows by sub-pitch
      amounts, so every part-to-part wire jogs; two adjacent jogs then collide
      (< min-pitch) and the deny loop over-blocks (denying a failed end run's
      whole span also kills every shorter version anchored at the same fixed
      port) → "fixed port blocked" strays (bus.U5.B→J3.p2). Real sheets align
      facing pin rows; then those wires are straight and the conflict class
      vanishes. Rule: anchors in one row, in slot order — each aligns to the
      first (statement-order) wire pairing one of its LEFT pins with an
      already-placed same-row anchor's RIGHT pin (columns mirror with
      top/bottom); default stays cluster-centred; track sizes to the union of
      shifted clusters. In place.rs arrange().
      NOTE: hero carries 2 transient strays between B and E (bus one dies at
      B2, entry one — F1 span overlapping the chassis seat, the documented
      "two-end chain is never packed" limit — dies at E).
- [x] C. [04ea703] shared growth-ray rule (#4) + 2-port glyph axis facing (#5)
- [x] D. [04ea703] taps (#6)
- [x] E. [04ea703] spans: bridge + clipped, posed spans (#7) — the largest; may split
- [ ] F. readouts clear the top rail (#9) — opus-xhigh agent task
- [ ] G. |J| single column + sample touch-up (#8) — opus-xhigh agent task
- [x] H. [0138370] perf — profile showed ADMIT 77% (not search): world-scoped admission sims (0.74→0.06s) + A* bound with sound eval-priced early exit (0.19→0.004s); 0.95→0.09s, snapshots byte-identical
- [ ] SPEC 16.1/16.2 + ROUTING.md amendments land WITH their commits.

Delegation: F, G go to opus-xhigh agents (isolated, snapshot churn — run
them between my commits, not in parallel); the rest inline in the main
(Fable) session per the user's instruction.

## Status (2026-08-30, resumed)
A/B/B2/C/D/E/H committed; hero routes whole (0 warnings) in 0.09s and
reads like the reference sheets. Remaining: F (readouts vs top rail,
agent), G (|J| single column + hero J1 rotate, agent), final visual pass.

## Old pause notes (resolved)
Committed: A [32c7fb3], B [0dfacba]. IN THE WORKING TREE, green except ONE
test: B2 (facing-pin alignment, place.rs), C (growth_ray/shared-pin/2-port
facing/bridge_ray in chain.rs+autopose+family), D (taps + Chain.parents),
E (bridge + landing-leg spans + cluster-aware Demand + window insets),
plus: ladder clears both-sided ink (readouts), canonical column order
(breaks the 2×2 up/down share cycle), share pairs only ray-firsts per pin.
SPEC 16.1 amendments written (alignment + new seat rules). Hero renders
ZERO warnings and reads like a real sheet (user-confirmed direction:
one column per chain; up+down pair shares a column; two same-way don't).

RESUME HERE:
1. tests/rendering/links.rs `an_authored_wire_paint_beats_the_scopes_dress`
   fails — my sch_sheet fixture change moved the `{tail}` from wire u2.a to
   u2.b (needed for the corner-radius test since the aligned a-wire is now
   straight); the paint test asserts which wire wears the authored stroke —
   update its expectation to wire b (line ~397-419).
2. cargo insta accept for schematic sample snapshots; full suite green.
3. Strip LINI_DEBUG_ROUTE/LINI_DEBUG_ENTRY eprintlns from
   src/routing/ortho/mod.rs and entry.rs (uncommitted debug — NOT part of
   any commit; also fixes the clippy find-is_some warning there).
4. cargo fmt; clippy -D warnings; commit the working tree as ONE commit
   (the seat-pass rework arc: B2+C+D+E — intermediate splits each fail the
   zero-stray sample gate, so one coherent commit, honest message).
5. Seat tests to add: bridge (U2.EN-R5-U2.VIN poses along the side), span
   landing-leg + clip (J1.p1-F1-Q1.s shape), tap (buck 5V flag sideways),
   two-column divider (2×2 up/down monitor shape).
6. Then F (readouts vs top rail — opus agent), G (|J| single column + J1
   rotate:180 in the hero sample — opus agent), H (perf: A* + early exit,
   re-measure first; debug prints in mod.rs show solve traces).
7. Sample touch-up pass + final SPEC/ROUTING read-through; render every
   schematic sample + LOOK.

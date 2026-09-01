# Schematic lattice — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** replace `layout: schematic`'s placement with one integer lattice per scope, so alignment is arithmetic rather than an accumulating pile of rules.

**Architecture:** six decide-once passes — model, field, pack, rails, absolutize, readouts — over a scope-wide grid of two pitches (fine = pin pitch = wires; coarse = `gap` = parts). Placement assigns cells; one pass multiplies them out. The orthogonal router keeps every wire, gaining only a per-world track quantum.

**Tech Stack:** Rust 2024 (`rust-toolchain.toml`), `insta` snapshots, `resvg` for visual checks, `cargo xtask` for generated artifacts.

**Spec:** `SCHEMATIC-GRID.md` (repo root) — read it whole before Task 1. `SPEC.md` §16 and `ROUTING.md` are the two contracts this changes; `AGENTS.md` is the house rules.

**Branch:** `schematic-grid`.

---

## Global Constraints

Copied from `AGENTS.md` and the design; every task's requirements include these.

- **No `unsafe`.** Find another path or surface the question.
- **One mechanism per problem.** Extend whatever owns a failure mode; never add a second that re-fights it.
- **No parallel implementations.** Two places doing one job call one shared function.
- **Modular: one concept per file.** Split a module past ~500 LOC.
- **Clean, human-readable, modern Rust.** Idiomatic over clever; small focused functions with names that say what they mean; `let … else`, iterator chains and pattern matching over index arithmetic where they read better. Don't fight `rustfmt` / `clippy`.
- **Comments explain the non-obvious *why*, never the what.** Nothing beyond the task — no extra features, validation, or commentary.
- **Reused style rides a rule, never inline** — a generated class emits one CSS rule; `style=` is only the per-element diff.
- **Trust a correct model.** Never special-case a principled formula's output to nudge one case to taste.
- **`insta` snapshot tests for output-shaped code**; verify SVG visually by rasterising with `resvg` and reading the PNG — never ask the user to spot-check.
- **Git:** descriptive messages, one purposeful change per commit, **never** a `Co-Authored-By` line. Defer pushing to `main` to the user.
- **Before any push:** `cargo fmt --all -- --check`, `cargo clippy --all-targets`, `cargo test` — all clean.
- **Generated artifacts drift-test:** after any ledger or property-table change run `cargo run --release -p xtask -- gen-schema`, `gen-grammars`, and `wasm`, or three suites fail.
- **`tests/spec_blocks.rs` ledgers SPEC's fenced code blocks by index**, each pinned by its first line. Adding or removing a fenced block in `SPEC.md` shifts every later index — update `SPEC_LEDGER` in the same commit. SPEC §16's blocks are currently indices 45 and 46.

### Vocabulary (used by every task)

| Term | Is |
|---|---|
| **pitch** | the fine lattice: `consts::PIN_PITCH` = 20. Wires, pins, stub tips. |
| **gap** | the coarse lattice: the scope's `gap`, per axis, rounded **up** to a multiple of pitch. Part centres. |
| **anchor** | a child riding the track grid — a 3+-pin part, or anything with `cell:`. |
| **satellite** | a child seated at a pin — a `\|label\|`, or an unplaced 1–2-pin part. |
| **chain** | the connected run of satellites one wire holds (`chain::chains`). |
| **ray** | the direction a chain grows. |
| **lane** | a chain's cross coordinate: a coarse line out from the anchor's ink, or the pin's own fine line for a chain growing straight out. |
| **slot** | a member's position along the ray: the *k*-th coarse line from the field origin. |
| **field** | one anchor's cells — its chains' lanes and slots. |
| **rail row** | the scope-wide row every downward ground sinks to (and the flag row above). |

---

## File Structure

```
src/layout/schematic/
  mod.rs         unchanged entry — node / root / arrange
  place.rs       SLIMMED to the orchestrator: roles, tracks, the pass order
  lattice.rs     NEW  the two pitches; cell ↔ px; the snap                 (T3)
  field.rs       NEW  chains to cells: rays, lanes, slots, occupancy       (T4,T5,T6)
  pack.rs        NEW  tracks in coarse cells, field widths, facing align   (T7)
  rail.rs        NEW  the ground and flag rows                             (T8)
  readout.rs     NEW  the ref/value side rule                              (T9)
  seat.rs        DELETED at T5 (1473 lines)
  seat_tests.rs  DELETED at T5 (1036 lines), replaced by field_tests.rs
  field_tests.rs NEW  the seat suite, rewritten against the lattice        (T5)
  place_tests.rs REWRITTEN at T7 for coarse-cell tracks
  tests.rs       helpers kept; `ink`/`cell`/`body` re-homed off seat.rs    (T5)
  net.rs · junction.rs · ports.rs · terminal.rs · tag.rs · hints.rs   kept
src/routing/ortho/
  scene/mod.rs   SceneNode gains `quantum`; SceneIndex gains a root setter (T10)
  mod.rs         World gains `quantum`                                     (T10)
  world.rs       build_worlds copies the quantum in                        (T10)
  place.rs       chain_prefs rounds an interior run's preference           (T10)
src/ledger/
  consts.rs      LABEL_SEAT deleted; the schematic block re-documented     (T3)
  defaults.rs    SCH_GAP retuned                                           (T3,T11)
SPEC.md          §16 rewritten (T1); §10.5 constants (T3)
ROUTING.md       one clause in Model step 5 (T2)
samples/         schematic*.lini rebuilt (T11)
```

`drawn()` — the engine's one "ink of a placed part" helper — currently lives in `seat.rs` and is used by `tests.rs` and `place.rs`. **T5 moves it to `field.rs`** unchanged; every caller follows.

---

### Task 1: SPEC §16 rewritten around the lattice

Documentation only. No code changes, no behaviour change. The spec is the source of truth, so it lands before the engine that implements it.

**Files:**
- Modify: `SPEC.md:3383-3780` (§16 whole; §16.1 is the rewrite, 16.2–16.7 are amendments)
- Modify: `tests/spec_blocks.rs` — only if the fenced-block count in §16 changes

**Interfaces:**
- Consumes: `SCHEMATIC-GRID.md` §2 (the model), §4 (constants).
- Produces: the prose every later task cites as `[SPEC 16.1]` in its doc comments. Task 3–9 doc comments must quote this text, not the old text.

- [ ] **Step 1: Read the two sources end to end**

Read `SCHEMATIC-GRID.md` whole, then `SPEC.md:3383-3780` whole. Note the voice: dense, present tense, no execution log, `[SPEC N]` cross-references, tables where the content is genuinely tabular. Read three neighbouring sections (`§15.11`, `§14.9`, `§13`) to calibrate length and register — §16.1 is currently the densest execution-log site left in the document and an audit called it "a sequence of patch notes in prose form". That is what this step removes.

- [ ] **Step 2: Rewrite §16.1 as "Placement — the lattice"**

Replace the whole of §16.1 (`SPEC.md:3411-3537`) with prose covering, in this order, and nothing else:

1. **The lattice.** Two pitches: the fine `pitch` (the pin pitch, baked — wires, pins and stub tips land on it) and the coarse `gap` (the part pitch, authored, per axis, rounded **up** to a multiple of pitch). `gap` in a schematic scope is the column and row pitch, not the space between tracks: two adjacent anchors with nothing between them stand one coarse column apart.
2. **Ink never places.** A satellite's cell comes from the lattice, never from its symbol's size or its ref/value width. A long value overhangs its neighbour's column; `gap` is the lever. The one reading of a part's own ink is its field origin.
3. **Anchors ride tracks.** Keep the existing ordinal-track paragraph — one row by default, `columns: N` wraps, `cell: c r` places, ordinals collapse — amended so tracks size in whole coarse cells from their anchors' field widths.
4. **A chain is a walk.** Ray (terminator convention, anti-parallel yields to the pin's normal, the straight corridor belongs to its first claimant), lane (a coarse line out from the anchor's ink, or the pin's own fine line for a chain growing straight out), slots (member *k* centred on the *k*-th coarse line, so a cap and a resistor off one bus share a body row). Taps and branches, one sentence each.
5. **Collision.** A member's cell is a `gap`-sized rectangle at its lattice point; a lane is free when none of a chain's cells meets a committed one, else it steps out. Leads reserve nothing — the lane order keeps them clear. Two consequences stated as consequences: an up-chain and a down-chain off one pin share a lane; a chain never lands where a lead must cross.
6. **Lane order.** Deepest pin along the ray keeps the inner lane; that is also the allocation order, ties on statement order; a side carrying both rays falls back to the canonical direction.
7. **Field origin.** The first coarse line clear of the anchor's own drawn ink on that side, readouts included; lanes and slots are absolute coarse lines of the scope, so two anchors' fields share rows.
8. **Rails.** Every downward chain's ground sinks to the scope's one ground row, every upward flag rises to the flag row; vertical only.
9. **Alignment.** Centre-to-centre by default; a facing pin pair joined by a wire or a span aligns instead, the shift a whole number of pitches, struck before the tracks size.
10. **Spans and bridges.** A span rides the landing leg on consecutive coarse cells, last-named nearest the second end; a bridge grows off the first-named pin as an ordinary one-end chain.
11. **Pose is rotation.** Keep the existing final paragraph verbatim — auto-pose and `rotate:` are unchanged.

Delete outright: the lane-ladder pitch rhythm, the "greediest neighbouring pair" rule, stack reach, the wired-row corridor rule, `ROOM_LIMIT`, the seat-gap prose, and every sentence describing how one pass anticipates another. Target: no more than half the current length.

- [ ] **Step 3: Amend §16.2, §16.4, §16.6, §16.7**

- **§16.2** — the readout paragraph ("Ref/value text places deterministically…") is replaced by the §2.6 rule: outward from the anchor for a part on a lane (left field → text left, right-aligned; right field → right, left-aligned), above and below and centred for a part riding a row. Add one sentence: a component's pin rail is seated so its pins land on fine lattice lines.
- **§16.4** — the net-run "freer side" table keeps its two rows, but "the freer side — more clear space that way" is replaced by a lattice reading: the side away from the anchor whose field the run sits in, ties on the routing side rank. Nothing else in §16.4 changes.
- **§16.6** — the "Opting into the engine is one decision" paragraph names `gap` as the coarse pitch rather than the track gap.
- **§16.7** — the lowering paragraph lists the new pass order: field, pack, rails, absolutize, readouts, then the router.

- [ ] **Step 4: Check the fenced-block ledger**

Run: `grep -c '^```' SPEC.md`
Expected: an even number, and the count unchanged from `git show HEAD:SPEC.md | grep -c '^```'`. If §16 gained or lost a fence, update every `SPEC_LEDGER` row index after it in `tests/spec_blocks.rs`.

- [ ] **Step 5: Run the docs suites**

Run: `cargo test --test spec_blocks && cargo test --test deferred`
Expected: PASS. `spec_blocks` compiles every fenced block in `SPEC.md`; `deferred` checks §24 against the ledger.

- [ ] **Step 6: Commit**

```bash
git add SPEC.md tests/spec_blocks.rs
git commit -m "SPEC 16.1 states a lattice, not an execution log

Placement was described as the sequence of repairs that produced it —
ladder pitches read off neighbouring ink, stack reach, wired-row
corridors, each rule reaching into another pass. The model underneath is
one integer lattice per scope with two pitches, and stated that way the
section is half as long and says more."
```

---

### Task 2: ROUTING.md's track quantum

Documentation only, and the only change this rebuild makes to the routing contract.

**Files:**
- Modify: `ROUTING.md` — §Vocabulary (one entry), §The Model step 5 (one clause)

**Interfaces:**
- Produces: the contract sentence Task 10 implements. Task 10's code comment cites it.

- [ ] **Step 1: Add the vocabulary entry**

In `ROUTING.md` §Vocabulary, after the **Run** entry, add:

```markdown
- **Track quantum** — a world's own grid step, when its scope states one (a
  schematic's fine pitch, [SPEC 16.1](SPEC.md#161-placement--the-lattice)).
  It rounds an interior run's *preference*, never its lawful range: no law
  reads it, and a world without one is unchanged.
```

- [ ] **Step 2: Amend Model step 5's interior-run bullet**

The bullet currently reads "an **interior run** prefers its channel's **anchor** — the midline when both walls are keep-out edges …, the keep-out wall when the other wall is the canvas edge …". Append one sentence:

> Where the run's world states a **track quantum**, that preference rounds to the nearest multiple of it and clamps back into the corridor — so a bare run between two gridded parts lands on their grid rather than a millimetre off it.

- [ ] **Step 3: Verify the doc still compiles**

Run: `cargo test --test spec_blocks`
Expected: PASS (the guard sweeps `ROUTING.md` too; neither edit adds a fence).

- [ ] **Step 4: Commit**

```bash
git add ROUTING.md
git commit -m "A world may state a track quantum, and interior runs round to it

A schematic places its parts on a grid; a bare run bending between two of
them lands on a channel midline that is nobody's line. The quantum is a
preference-time rounding, so the four laws are untouched and a world
without one routes exactly as before."
```

---

### Task 3: `lattice.rs` — the two pitches

A new module and its unit tests. Nothing calls it yet, so the tree stays green and the behaviour unchanged.

**Files:**
- Create: `src/layout/schematic/lattice.rs`
- Modify: `src/layout/schematic/mod.rs` (add `mod lattice;`)
- Modify: `src/ledger/consts.rs` (retire `LABEL_SEAT`, re-document the schematic block)
- Modify: `src/ledger/defaults.rs:31` (`SCH_GAP`)
- Modify: `SPEC.md` §10.5's schematic constants block

**Interfaces:**
- Consumes: `crate::ledger::consts::PIN_PITCH`, `crate::layout::primitives::gap`, `crate::desugar::pose::Side`.
- Produces, for Tasks 4–9:

```rust
/// Which lattice axis a coordinate lies on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Ax { X, Y }

impl Ax {
    /// The axis a side's outward normal runs along.
    pub(super) fn of(side: Side) -> Ax;
    /// `+1` when the side's normal points the increasing way, `-1` otherwise.
    pub(super) fn outward(side: Side) -> f64;
}

/// A schematic scope's grid [SPEC 16.1]: the fine pitch every wire and pin
/// lands on, and the coarse pitch every part centre does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Lattice {
    pub pitch: f64,
    pub row: f64,
    pub col: f64,
}

impl Lattice {
    /// Read the scope's lattice off its attrs.
    pub(super) fn of(attrs: &AttrMap, span: Span) -> Result<Lattice, Error>;
    /// The coarse step along `ax`.
    pub(super) fn step(self, ax: Ax) -> f64;
    /// The coordinate of coarse line `i` on `ax`.
    pub(super) fn line(self, ax: Ax, i: i32) -> f64;
    /// The first coarse line **strictly** beyond `v` along `ax`, going the
    /// way `outward` points (`+1` / `-1`).
    pub(super) fn beyond(self, ax: Ax, v: f64, outward: f64) -> i32;
    /// `v` rounded to the nearest fine line.
    pub(super) fn snap(self, v: f64) -> f64;
}
```

- [ ] **Step 1: Write the failing tests**

Create `src/layout/schematic/lattice.rs` with the module doc and a test module at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::consts::PIN_PITCH;
    use crate::ledger::defaults::SCH_GAP;
    use crate::testutil::program;

    fn lat(style: &str) -> Lattice {
        let p = program(&format!("|schematic#s|{style} [ |gnd#g| ]\n"));
        let scope = &p.scene.nodes[0];
        Lattice::of(&scope.attrs, scope.span).expect("a lattice")
    }

    #[test]
    fn the_scope_gap_is_the_coarse_pitch() {
        let l = lat("");
        assert_eq!((l.row, l.col), (SCH_GAP, SCH_GAP), "the scope default");
        assert_eq!(l.pitch, PIN_PITCH, "the fine pitch is the pin pitch");
    }

    #[test]
    fn a_coarse_pitch_rounds_up_to_a_whole_number_of_fine_ones() {
        // [SPEC 16.1] the coarse grid is built of fine units, so a part
        // centre is always on a wire line too.
        let l = lat(" { gap: 90 }");
        assert_eq!(l.row, 100.0, "90 rounds up to 5 pitches");
        assert_eq!(l.col, 100.0);
        assert_eq!(lat(" { gap: 100 }").row, 100.0, "an exact multiple stands");
    }

    #[test]
    fn gap_states_the_two_axes_separately() {
        let l = lat(" { gap: 120 80 }");
        assert_eq!((l.row, l.col), (120.0, 80.0), "row then column, as gap reads");
    }

    #[test]
    fn a_coarse_pitch_never_falls_below_one_fine_one() {
        assert_eq!(lat(" { gap: 0 }").row, PIN_PITCH);
    }

    #[test]
    fn lines_and_snapping_are_plain_arithmetic() {
        let l = Lattice { pitch: 20.0, row: 100.0, col: 100.0 };
        assert_eq!(l.line(Ax::X, 3), 300.0);
        assert_eq!(l.line(Ax::Y, -2), -200.0);
        assert_eq!(l.snap(53.0), 60.0);
        assert_eq!(l.snap(-53.0), -60.0);
    }

    #[test]
    fn beyond_is_strict_so_a_field_never_starts_on_the_ink() {
        let l = Lattice { pitch: 20.0, row: 100.0, col: 100.0 };
        assert_eq!(l.beyond(Ax::X, 120.0, 1.0), 2, "the next line out");
        assert_eq!(l.beyond(Ax::X, 200.0, 1.0), 3, "strictly beyond, never on");
        assert_eq!(l.beyond(Ax::X, -120.0, -1.0), -2);
        assert_eq!(l.beyond(Ax::X, -200.0, -1.0), -3);
    }
}
```

- [ ] **Step 2: Run them to watch them fail**

Run: `cargo test --lib schematic::lattice`
Expected: FAIL to compile — `Lattice` does not exist.

- [ ] **Step 3: Implement the module**

Write `Ax`, `Lattice` and the six methods above. `Lattice::of` reads `super::super::primitives::gap(attrs, span)?` — which returns `(gap_y, gap_x)` — and rounds each up: `(v / pitch).ceil().max(1.0) * pitch`. `beyond` is `(v / step).floor() as i32 + 1` outward-positive and `(v / step).ceil() as i32 - 1` outward-negative; both are strict because `floor`/`ceil` of an exact multiple returns it. Keep the module under 150 lines including tests; the doc comment states the two pitches and cites `[SPEC 16.1]`, and explains only the *why* of rounding up (a part centre must also be a wire line).

- [ ] **Step 4: Run them to watch them pass**

Run: `cargo test --lib schematic::lattice`
Expected: PASS, 6 tests.

- [ ] **Step 5: Retune the constants**

- `src/ledger/defaults.rs:31` — `SCH_GAP: f64 = 60.0` becomes `100.0`, its doc comment restated as the coarse lattice pitch (five fine pitches) rather than the track gap.
- `src/ledger/consts.rs` — delete `LABEL_SEAT` and its doc comment; the lattice states the distance now. Re-word the `PIN_PITCH` doc to say it is also the fine lattice and the router's track quantum.
- `SPEC.md` §10.5's schematic constants fence — drop the `net-label-seat` row if present, and restate the gap default. **This fence is `SPEC_LEDGER` index 21 (`Kind::NotLini`, "the schematic chrome constants")** — editing its content is fine, but do not change how many fences exist.

- [ ] **Step 6: Run the whole suite**

Run: `cargo test`
Expected: `LABEL_SEAT` no longer resolves in `seat.rs` and `seat_tests.rs`. Fix by inlining `LABEL_SEAT`'s value as a local `const` in `seat.rs` with a comment saying it dies at Task 5 — `seat.rs` is deleted two tasks from now and must not grow a real dependency. Then: PASS. Note the `SCH_GAP` change will move golden values in `place_tests.rs` / `seat_tests.rs`; update the numbers only, never the assertions' meaning.

- [ ] **Step 7: Commit**

```bash
git add src/layout/schematic/lattice.rs src/layout/schematic/mod.rs src/ledger/consts.rs src/ledger/defaults.rs SPEC.md
git commit -m "The schematic scope gets a lattice of two pitches

The fine one is the pin pitch every wire and pin already landed on; the
coarse one is 'gap', rounded up to a whole number of fine pitches so a
part centre is always a wire line too. Nothing reads it yet — the seat
pass still places — but the arithmetic every later pass shares is in one
place, with its own tests."
```

---

### Task 4: `field.rs` — chains to cells

The assignment itself: rays, lane allocation, slots. Built and unit-tested, but not yet wired into placement, so behaviour is unchanged and the tree stays green.

**Files:**
- Create: `src/layout/schematic/field.rs`
- Modify: `src/layout/schematic/mod.rs` (add `mod field;`)

**Interfaces:**
- Consumes: `lattice::{Ax, Lattice}` (T3); `crate::desugar::schematic::chain::{Chain, End, chains, growth_ray, holder, limbs, placed_ends, shared_pin, tap_ray, taps}`; `super::terminal::{Terminal, terminal}`; `super::place::role`; `crate::desugar::schematic::Role`.
- Produces, for Tasks 5–9:

```rust
/// Where a satellite sits [SPEC 16.1], in cells, before the tracks size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Seat {
    /// The anchor whose field holds it.
    pub anchor: usize,
    /// The direction its chain grows.
    pub ray: Side,
    /// The side of the anchor its lead leaves by — the pin's own normal.
    pub side: Side,
    /// Coarse lanes out from the anchor's ink on `side`, 1-based; `None` for
    /// a chain that grew straight out and keeps its pin's fine line.
    pub lane: Option<i32>,
    /// The pin's cross coordinate in the anchor's own frame — the line a
    /// laneless seat keeps.
    pub pin_line: f64,
    /// Coarse slots along `ray` from the field origin, 1-based.
    pub slot: i32,
}

/// Every satellite's cell in one scope.
pub(super) struct Field { /* private */ }

impl Field {
    pub(super) fn build(
        children: &[PlacedNode],
        roles: &[Role],
        links: &[&ResolvedLink],
        scope: &str,
        lat: Lattice,
    ) -> Field;

    /// The seat of child `i`, if a chain held it.
    pub(super) fn seat(&self, i: usize) -> Option<Seat>;
    /// How many coarse lanes `anchor`'s field takes on `side`.
    pub(super) fn lanes(&self, anchor: usize, side: Side) -> i32;
    /// How many coarse slots deep `anchor`'s field runs along `ray`.
    pub(super) fn depth(&self, anchor: usize, ray: Side) -> i32;
    /// Satellites no wire held — the flow fallback [SPEC 16.1].
    pub(super) fn floating(&self) -> &[usize];
}

/// A placed part's **drawn** extent in its own frame — moved here from
/// `seat.rs` at Task 5, unchanged.
pub(super) fn drawn(node: &PlacedNode) -> Bbox;

/// The scope's wire edges — moved here from `seat.rs` at Task 5, unchanged.
pub(super) fn edges(children: &[PlacedNode], links: &[&ResolvedLink], scope: &str) -> Vec<[End; 2]>;
```

- [ ] **Step 1: Write the failing test for lane allocation**

The allocator is the one genuinely pure piece; unit-test it directly. In `field.rs`'s test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// A chain's cells at lane `k`, on a left side growing down: one column,
    /// `n` cells deep, in a 100-pitch lattice.
    fn column(k: i32, n: i32) -> Vec<Bbox> {
        (1..=n)
            .map(|slot| Bbox::centered(100.0, 100.0)
                .shifted(-100.0 * k as f64, 100.0 * slot as f64))
            .collect()
    }

    #[test]
    fn the_innermost_free_lane_wins() {
        assert_eq!(allocate(&[], |k| column(k, 2)), 1, "an empty field takes lane 1");
    }

    #[test]
    fn an_occupied_lane_steps_out_one_and_retries() {
        let taken = column(1, 2);
        assert_eq!(allocate(&taken, |k| column(k, 2)), 2);
    }

    #[test]
    fn opposite_rays_off_one_pin_share_a_lane() {
        // [SPEC 16.1] the down-chain's cells and the up-chain's are disjoint,
        // so the second one's first candidate is already free — the column
        // sharing is a consequence of the occupancy test, not a rule.
        let down = column(1, 2);
        let up = |k: i32| (1..=2)
            .map(|slot| Bbox::centered(100.0, 100.0)
                .shifted(-100.0 * k as f64, -100.0 * slot as f64))
            .collect::<Vec<_>>();
        assert_eq!(allocate(&down, up), 1, "one lane, two rays");
    }

    #[test]
    fn a_deeper_chain_only_needs_the_lanes_its_own_cells_meet() {
        // Lane 1 is held two cells deep; a four-cell chain still cannot use
        // it, but a chain starting past it can.
        let taken = column(1, 2);
        let deep = |k: i32| (3..=4)
            .map(|slot| Bbox::centered(100.0, 100.0)
                .shifted(-100.0 * k as f64, 100.0 * slot as f64))
            .collect::<Vec<_>>();
        assert_eq!(allocate(&taken, deep), 1, "below what lane 1 holds");
    }
}
```

- [ ] **Step 2: Run it to watch it fail**

Run: `cargo test --lib schematic::field`
Expected: FAIL to compile — `allocate` does not exist.

- [ ] **Step 3: Implement `allocate`**

```rust
/// The innermost lane whose cells meet nothing already committed [SPEC 16.1].
/// The lead reserves nothing: the lane order (deepest pin innermost) already
/// keeps a lead clear of the columns it crosses.
fn allocate(taken: &[Bbox], cells: impl Fn(i32) -> Vec<Bbox>) -> i32 {
    (1..)
        .find(|&k| cells(k).iter().all(|c| !taken.iter().any(|t| overlaps(*t, c))))
        .expect("an unbounded lattice always has a free lane")
}
```

with a small `overlaps(Bbox, Bbox) -> bool` (strict: touching edges do not overlap, so adjacent cells are legal neighbours).

- [ ] **Step 4: Run it to watch it pass**

Run: `cargo test --lib schematic::field`
Expected: PASS, 4 tests.

- [ ] **Step 5: Implement `Field::build`**

The pass, in this order — a fresh reader should be able to follow it top to bottom:

1. `let sat: Vec<bool> = roles.iter().map(|r| *r == Role::Satellite).collect();`
2. `let edges = edges(children, links, scope);` and `let chains = chains(&sat, &edges);`
3. Per chain: `placed_ends(&chain, roles)`. None → push every member to `floating`. Two or more on different anchors → a span (Task 6; for now, `floating` with a `TODO(T6)` marker is **not** acceptable — instead leave `seat()` returning `None` for its members and record them in a `spans` vec the struct exposes as empty until Task 6). `holder(&ends)` → a one-end chain or a bridge, both grown the same way.
4. For a held chain: read the pin's `Terminal` (`terminal(&children[end.child], end.terminal.as_deref())`), the terminator's facing, and call `growth_ray(...)` exactly as `seat.rs` does today — the ray rule is unchanged and must stay one function shared with the pose chooser.
5. Group the anchor's chains by `(anchor, side)` where `side` is the pin's own facing. Sort each group by **depth along the ray, descending** (the pin's coordinate along the ray), ties on statement order — this is the allocation order.
6. Per chain in that order: if `ray == side` it is laneless (`lane: None`, `pin_line` = the pin's cross coordinate); otherwise `allocate` against the anchor's committed cells, then commit its cells.
7. Members take slots `1..=n` along the ray in chain order. `taps(&chain, …)` take no slot — a tap hangs off its attachment member's cell along `tap_ray(...)`. `limbs(&chain)` splits trunk from branch; a multi-member branch grows as its own sub-chain from its attachment, allocated in the same field.

Cells for the occupancy test are built with the `Lattice`: a cell is `Bbox::centered(lat.col, lat.row)` at the lane/slot lattice point, in the anchor's own frame with the field origin at the first coarse line beyond the anchor's `drawn()` ink on that side (`lat.beyond`).

Keep `field.rs` under 500 lines. If the tap/branch handling pushes it over, split the walk into `field/walk.rs` and keep allocation in `field/mod.rs` — one concept per file.

- [ ] **Step 6: Run the whole suite**

Run: `cargo test`
Expected: PASS — nothing calls `Field::build` yet, so behaviour is unchanged. `cargo clippy --all-targets` must be clean; a `#[allow(dead_code)]` on `Field` for this one commit is acceptable and must be removed at Task 5.

- [ ] **Step 7: Commit**

```bash
git add src/layout/schematic/field.rs src/layout/schematic/mod.rs
git commit -m "A chain becomes cells: ray, lane, slot

The field pass reads the same chains and the same growth ray the seat
pass reads, and answers in lattice cells instead of measured pixels. A
lane is the innermost one whose cells meet nothing committed, which is
the whole of collision — an up-chain and a down-chain off one pin share
a lane because their cells are disjoint, not because a rule says so.
Nothing calls it yet."
```

---

### Task 5: wire the field into placement, delete `seat.rs`

The behaviour change. Output will be gridded but not yet packed, railed or re-readout — that is Tasks 7–9. Expect the samples to look different and, in places, worse than before this task; the visual gate is Task 11.

**Files:**
- Modify: `src/layout/schematic/place.rs` — `arrange` uses `Field`, drops `Seats`
- Modify: `src/layout/schematic/field.rs` — gains `absolutize`, `extent`, `drawn`, `edges`
- Modify: `src/layout/schematic/hints.rs` — reads `field::edges` instead of `seat::edges`
- Modify: `src/layout/schematic/tests.rs` — `ink()` calls `field::drawn`
- Delete: `src/layout/schematic/seat.rs`, `src/layout/schematic/seat_tests.rs`
- Create: `src/layout/schematic/field_tests.rs`

**Interfaces:**
- Consumes: everything Task 4 produced.
- Produces:

```rust
impl Field {
    /// Move every seated satellite onto its absolute lattice point, now that
    /// its anchor is placed. Lanes and slots become the scope's own coarse
    /// lines, read off the anchor's placed ink [SPEC 16.1].
    pub(super) fn absolutize(&self, children: &mut [PlacedNode], lat: Lattice);
    /// An anchor's cluster: its own ink plus every cell its field holds, in
    /// the anchor's frame — what a track sizes against.
    pub(super) fn cluster(&self, children: &[PlacedNode], anchor: usize, lat: Lattice) -> Bbox;
}
```

- [ ] **Step 1: Write the failing tests**

Create `src/layout/schematic/field_tests.rs`, replacing `seat_tests.rs`. Port every test from `seat_tests.rs` whose *meaning* survives — the ray rules, auto-pose, the `rotate:` override, taps, branches, the flow fallback and its warnings — and rewrite the ones that asserted ladder pitches or ink-measured seats. Add these five, which are the new contract:

```rust
use super::tests::{anchor, at, body, close, laid, scope, sided, seat_warnings};
use crate::ledger::consts::PIN_PITCH;
use crate::ledger::defaults::SCH_GAP;

/// Every part centre lands on a fine lattice point — the invariant [SPEC 16.1].
fn on_fine_grid(v: f64) -> bool {
    let r = (v / PIN_PITCH).round() * PIN_PITCH;
    (v - r).abs() < 1e-6
}

#[test]
fn three_chains_off_one_pin_take_three_columns_one_coarse_pitch_apart() {
    // [SPEC 16.1] the pitch is the lattice's, never the parts' ink: three
    // values of wildly different widths still stand on one rhythm.
    let src = scope("", &(sided("u1")
        + "  |C#c1| \"1n\"\n  |C#c2| \"100000pF\"\n  |C#c3| \"1u\"\n"
        + "  |gnd#g1|\n  |gnd#g2|\n  |gnd#g3|\n"
        + "  u1.a - c1 - g1\n  u1.a - c2 - g2\n  u1.a - c3 - g3\n"));
    let nodes = laid(&src);
    let [x1, x2, x3] = ["c1", "c2", "c3"].map(|id| at(&nodes, id).0);
    assert!(close((x1 - x2).abs(), SCH_GAP), "one coarse pitch: {x1} {x2}");
    assert!(close((x2 - x3).abs(), SCH_GAP), "and the same one: {x2} {x3}");
}

#[test]
fn members_of_different_chains_share_a_slot_row() {
    // The reference sheet's row: a cap and a resistor hanging off one bus
    // have their bodies on one line, whatever their own lengths.
    let src = scope("", &(sided("u1")
        + "  |C#c1| \"1u\"\n  |R#r1| \"10k\"\n  |gnd#g1|\n  |gnd#g2|\n"
        + "  u1.a - c1 - g1\n  u1.a - r1 - g2\n"));
    let nodes = laid(&src);
    assert!(close(at(&nodes, "c1").1, at(&nodes, "r1").1), "one slot row");
}

#[test]
fn a_second_member_stands_one_coarse_pitch_deeper() {
    let src = scope("", &(sided("u1")
        + "  |R#r1| \"1k\"\n  |LED#d1| \"red\"\n  |gnd#g1|\n  u1.a - r1 - d1 - g1\n"));
    let nodes = laid(&src);
    let (r, d) = (at(&nodes, "r1").1, at(&nodes, "d1").1);
    assert!(close(d - r, SCH_GAP), "slot 1 then slot 2: {r} {d}");
}

#[test]
fn an_up_chain_and_a_down_chain_off_one_pin_share_a_column() {
    // [SPEC 16.1] their cells are disjoint, so the second one's innermost
    // candidate is free — no rule, a consequence.
    let src = scope("", &(sided("u1")
        + "  { |v3::label| { symbol: power } [ \"3V3\" ] }\n"
        + "  |C#c1| \"1u\"\n  |gnd#g1|\n  |R#r1| \"10k\"\n  |v3#f1|\n"
        + "  u1.a - c1 - g1\n  u1.a - r1 - f1\n"));
    let nodes = laid(&src);
    assert!(close(at(&nodes, "c1").0, at(&nodes, "r1").0), "one lane, two rays");
    assert!(at(&nodes, "r1").1 < at(&nodes, "u1").1, "the flag chain climbs");
    assert!(at(&nodes, "c1").1 > at(&nodes, "u1").1, "the ground chain drops");
}

#[test]
fn every_seated_part_lands_on_the_fine_lattice() {
    let src = scope("", &(sided("u1")
        + "  |R#r1| \"1k\"\n  |C#c1| \"1u\"\n  |gnd#g1|\n  |gnd#g2|\n"
        + "  u1.a - r1 - g1\n  u1.c - c1 - g2\n"));
    let nodes = laid(&src);
    for id in ["r1", "c1", "g1", "g2"] {
        let (x, y) = at(&nodes, id);
        assert!(on_fine_grid(x) && on_fine_grid(y), "'{id}' off the grid: {x} {y}");
    }
}
```

- [ ] **Step 2: Run them to watch them fail**

Run: `cargo test --lib schematic::field_tests`
Expected: FAIL — the seat pass still places, so columns stand on ink-derived pitches.

- [ ] **Step 3: Rewire `place.rs`**

In `place::arrange`, replace `Seats::build(...)` with `Field::build(children, &roles, links, scope, lat)` where `lat = Lattice::of(attrs, span)?`, and replace `seats.cluster(...)` / `seats.absolutize(...)` / `seats.floating()` with the `Field` equivalents. Leave the track sizing, the `align` pass and the span `charge` machinery **exactly as they are** for now — Task 7 replaces them. Move `drawn` and `edges` from `seat.rs` into `field.rs` unchanged and update `hints.rs` and `tests.rs` to the new path.

- [ ] **Step 4: Delete the seat pass**

```bash
git rm src/layout/schematic/seat.rs src/layout/schematic/seat_tests.rs
```

and drop `mod seat;` / `mod seat_tests;` from `mod.rs`, adding `mod field_tests;`. Delete the `LABEL_SEAT` local const Task 3 left behind. Nothing in the tree may still reference `Seats`, `Growing`, `SubStack`, `Rung` or `Demand` — grep for each.

- [ ] **Step 5: Run the tests**

Run: `cargo test --lib schematic`
Expected: the five new tests PASS. Other schematic tests will fail on golden numbers — `place_tests.rs`'s track gaps and `route_tests.rs`'s turn counts. Fix only the numbers whose *meaning* is unchanged; where a test asserted behaviour this rebuild deletes, delete the test and say so in the commit message.

- [ ] **Step 6: Look at it**

```bash
cargo build --release
./target/release/lini --static samples/schematic.lini -o /tmp/s.svg && resvg --zoom 3 /tmp/s.svg /tmp/s.png
```

Read `/tmp/s.png`. Expect: columns on one pitch, slot rows shared, grounds still ragged (Task 8) and tracks still ink-sized (Task 7). Record what is wrong in the commit message — it is the input to the next tasks.

- [ ] **Step 7: Commit**

```bash
git add -A src/layout/schematic
git commit -m "Placement seats satellites on the lattice, and the seat pass goes

2509 lines of ladders, stacks, corridors and rhythm come out; the field
pass puts every satellite on a lane and a slot. Columns stand one coarse
pitch apart whatever their values' widths, and members of different
chains share a slot row. Tracks are still ink-sized and grounds still
land wherever their own chain ended — the next two tasks."
```

---

### Task 6: spans and bridges

A chain held at two anchors. Split from Task 5 because it is the one case that lives *between* fields rather than inside one, and a reviewer can reject it alone.

**Files:**
- Modify: `src/layout/schematic/field.rs`
- Modify: `src/layout/schematic/field_tests.rs`

**Interfaces:**
- Produces:

```rust
/// A chain held at two anchors [SPEC 16.1]: its members ride the landing leg
/// on consecutive coarse cells, the last-named nearest the second end.
#[derive(Clone, Debug)]
pub(super) struct Spanning {
    pub members: Vec<usize>,
    pub ends: [(usize, Terminal); 2],
}

impl Field {
    pub(super) fn spans(&self) -> &[Spanning];
    /// The coarse cells a span asks of the region between its two anchors.
    pub(super) fn span_cells(&self, s: &Spanning) -> i32;
}
```

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_span_rides_the_landing_leg_on_coarse_cells() {
    // [SPEC 16.1] the fuse between a connector and a switch stands on the
    // line joining the two pins, one coarse cell per member.
    let src = scope("", &(sided_with("u1", "") + &sided_with("u2", "")
        + "  |F#f1| \"2A\"\n  u1.b - f1 - u2.a\n"));
    let nodes = laid(&src);
    let (fx, fy) = at(&nodes, "f1");
    let pin = at(&nodes, "b").1;
    assert!(close(fy, pin), "on the landing leg: {fy} vs {pin}");
    assert!(at(&nodes, "u1").0 < fx && fx < at(&nodes, "u2").0, "between them");
}

#[test]
fn two_span_members_stand_one_coarse_pitch_apart() {
    let src = scope("", &(sided_with("u1", "") + &sided_with("u2", "")
        + "  |F#f1| \"2A\"\n  |R#r1| \"1k\"\n  u1.b - f1 - r1 - u2.a\n"));
    let nodes = laid(&src);
    assert!(close((at(&nodes, "r1").0 - at(&nodes, "f1").0).abs(), SCH_GAP));
}

#[test]
fn a_bridge_grows_off_its_first_named_pin_like_any_chain() {
    // [SPEC 16.1] both ends on one anchor is a fan, not a span: the pull-up
    // stands in the first pin's own corridor and the router merges the rest.
    let src = scope("", &(sided_with("u1", "") + "  |R#r1| \"100k\"\n  u1.a - r1 - u1.b\n"));
    let nodes = laid(&src);
    let (rx, _) = at(&nodes, "r1");
    assert!(rx < at(&nodes, "u1").0, "off the left pin it was named at first");
}
```

- [ ] **Step 2: Run to watch them fail**

Run: `cargo test --lib schematic::field_tests`
Expected: FAIL — spans currently seat nothing.

- [ ] **Step 3: Implement**

In `Field::build`, a chain whose `placed_ends` gives two ends on **different** anchors records a `Spanning`; `holder` already sends both-ends-on-one-anchor down the ordinary one-end path, so the bridge case needs no new code — verify that by test, not by adding a branch. In `absolutize`, a span's members take consecutive coarse cells along the axis of the landing leg, cross coordinate = the second end's terminal line, last-named nearest that end. `span_cells` is `members.len() as i32`, which Task 7's packer charges to the region between the two tracks.

- [ ] **Step 4: Run to watch them pass**

Run: `cargo test --lib schematic` — Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/layout/schematic
git commit -m "A span rides its landing leg on coarse cells

A chain held at two anchors takes the cells between them, one per member,
last-named nearest the pin it lands on. A bridge — both ends on one
anchor — needs no case of its own: 'holder' already calls it a fan, and
it grows off its first-named pin like any one-end chain."
```

---

### Task 7: `pack.rs` — tracks in coarse cells

Replaces `place.rs`'s cluster sizing, `align` and `charge` with integer packing.

**Files:**
- Create: `src/layout/schematic/pack.rs`
- Modify: `src/layout/schematic/place.rs` (delete `align`, `charge`; call `pack`)
- Modify: `src/layout/schematic/place_tests.rs`

**Interfaces:**
- Consumes: `Field::{lanes, depth, spans, span_cells}`, `Lattice`.
- Produces:

```rust
/// Where every anchor lands [SPEC 16.1]: the ordinal track grid, sized in
/// whole coarse cells from its anchors' fields.
pub(super) struct Packing {
    /// Per anchor, its origin in scope coordinates.
    pub origins: Vec<(f64, f64)>,
    /// The packed content box.
    pub body: Bbox,
}

pub(super) fn pack(
    children: &[PlacedNode],
    anchored: &[usize],
    slots: &[Slot],
    field: &Field,
    lat: Lattice,
) -> Packing;
```

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_track_sizes_in_whole_coarse_cells() {
    let src = scope("", &(sided("u1") + &sided("u2")
        + "  |C#c1| \"1u\"\n  |gnd#g1|\n  u1.b - c1 - g1\n"));
    let nodes = laid(&src);
    let d = at(&nodes, "u2").0 - at(&nodes, "u1").0;
    assert!(close(d % SCH_GAP, 0.0), "anchors stand a whole number of cells apart: {d}");
}

#[test]
fn a_facing_pin_pair_aligns_and_the_wire_runs_straight() {
    // [SPEC 16.1] the right pins of one anchor against the left pins of the
    // next: the pair shares a row exactly, so the wire draws dead straight.
    let src = scope("", &(sided_with("u1", "") + &sided_with("u2", "") + "  u1.b - u2.a\n"));
    let nodes = laid(&src);
    assert!(close(at(&nodes, "b").1, at(&nodes, "a").1), "one row");
}

#[test]
fn the_alignment_shift_is_a_whole_number_of_fine_pitches() {
    let src = scope("", &(sided_with("u1", "") + &sided_with("u2", "") + "  u1.b - u2.a\n"));
    let nodes = laid(&src);
    let d = at(&nodes, "u2").1 - at(&nodes, "u1").1;
    assert!(close((d / PIN_PITCH).round() * PIN_PITCH, d), "off the fine grid: {d}");
}

#[test]
fn unaligned_anchors_stand_centre_to_centre() {
    let src = scope("", &(sided("u1") + &sided("u2")));
    let nodes = laid(&src);
    assert!(close(at(&nodes, "u1").1, at(&nodes, "u2").1), "one row line");
}
```

Then rewrite `place_tests.rs`'s track tests: `anchors_take_one_row_in_declaration_order`, `columns_wraps_the_flow` and `sparse_cell_ordinals_collapse_to_adjacent_tracks` keep their meaning; their `SCH_GAP` distance assertions become "a whole number of coarse cells apart".

- [ ] **Step 2: Run to watch them fail** — `cargo test --lib schematic` — Expected: FAIL on the cell-multiple assertions.

- [ ] **Step 3: Implement `pack.rs`**

Order, and it matters:

1. **Alignment offsets first.** Per axis, in track order, each anchor takes the first statement-order wire (or span) reaching a placed neighbour whose pins face each other, and offsets so the two landings share a line; the offset is `lat.snap`ped to a whole number of pitches. Everything else keeps centre-to-centre. This is the existing `align` logic, snapped — port it, do not rewrite it.
2. **Track sizes in cells.** Each track's extent = `ceil(anchor ink / gap)` cells + its field's lanes on each side + the cells its alignment offset consumed. A track takes the max over its anchors.
3. **Between tracks:** the earlier anchor's right-field lanes, then `span_cells` for every span crossing the pair, then the later anchor's left-field lanes.
4. **Origins** are the cumulative cell offsets multiplied out, plus each anchor's alignment offset.

Delete `place.rs`'s `align`, `charge` and the `col_lo`/`row_lo` cluster arithmetic. `grid::read_cell` / the ordinal collapse stay — the track grid's semantics are unchanged.

- [ ] **Step 4: Run to watch them pass** — `cargo test --lib schematic` — Expected: PASS.

- [ ] **Step 5: Look at it**

Render `samples/schematic.lini` and `samples/schematic_blocks.lini` at `--zoom 3` and read both PNGs. Expect even track spacing and straight part-to-part wires.

- [ ] **Step 6: Commit**

```bash
git add -A src/layout/schematic
git commit -m "Tracks size in whole coarse cells, and facing pins align on the grid

Cluster bboxes and the spanning-chain charge come out; a track is now the
cells its anchors' fields take, and the region between two tracks is the
right field, the spans, the left field. A facing pin pair still aligns —
the shift is a whole number of fine pitches, struck before the tracks
size, so an aligned anchor never overruns its allotment."
```

---

### Task 8: `rail.rs` — the ground and flag rows

**Files:**
- Create: `src/layout/schematic/rail.rs`
- Modify: `src/layout/schematic/place.rs` (call it after `absolutize`)
- Modify: `src/layout/schematic/field_tests.rs`

**Interfaces:**
- Produces:

```rust
/// Sink every downward chain's ground to one row and raise every upward
/// flag to one [SPEC 16.1] — the scope's rails, struck once the satellites
/// are absolute.
pub(super) fn rails(children: &mut [PlacedNode], field: &Field, lat: Lattice);
```

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn every_ground_in_a_scope_sinks_to_one_row() {
    // The reference sheet's line: a one-member chain's ground and a
    // three-member chain's stand on the same row [SPEC 16.1].
    let src = scope("", &(sided("u1")
        + "  |C#c1| \"1u\"\n  |R#r1| \"1k\"\n  |LED#d1| \"red\"\n"
        + "  |gnd#g1|\n  |gnd#g2|\n"
        + "  u1.a - c1 - g1\n  u1.a - r1 - d1 - g2\n"));
    let nodes = laid(&src);
    assert!(close(at(&nodes, "g1").1, at(&nodes, "g2").1), "one ground row");
    assert!(at(&nodes, "g1").1 > at(&nodes, "d1").1, "below the deepest member");
}

#[test]
fn every_power_flag_rises_to_one_row() {
    let src = scope("", &(sided("u1")
        + "  { |v3::label| { symbol: power } [ \"3V3\" ] }\n"
        + "  |R#r1| \"1k\"\n  |R#r2| \"2k\"\n  |L#l1| \"1u\"\n  |v3#f1|\n  |v3#f2|\n"
        + "  u1.a - r1 - f1\n  u1.a - r2 - l1 - f2\n"));
    let nodes = laid(&src);
    assert!(close(at(&nodes, "f1").1, at(&nodes, "f2").1), "one flag row");
}

#[test]
fn a_horizontal_chain_keeps_its_own_end() {
    // [SPEC 16.1] rails are vertical only — a chain running out along a pin's
    // row ends where it ends, as both reference sheets draw it.
    let src = scope("", &(sided("u1")
        + "  |R#r1| \"1k\"\n  |gnd#g1| { rotate: 90 }\n  |C#c1| \"1u\"\n  |gnd#g2|\n"
        + "  u1.a - r1 - g1\n  u1.c - c1 - g2\n"));
    let nodes = laid(&src);
    assert!(!close(at(&nodes, "g1").1, at(&nodes, "g2").1), "no rail across the axes");
}
```

- [ ] **Step 2: Run to watch them fail** — Expected: FAIL, grounds land at their own chain's depth.

- [ ] **Step 3: Implement**

After `Field::absolutize`, walk every seat whose chain terminator is a rail symbol (`gnd` · `earth` · `chassis` for the ground row; `power` for the flag row — read the `symbol:` attr through `terminal::ident`) and whose `ray` is `Side::Top` or `Side::Bottom`. The ground row is the deepest such terminator's slot line plus one coarse row; the flag row the shallowest minus one. Move each terminator's `cy` to that line; nothing else moves, and the lead between the last member and the rail is the router's.

- [ ] **Step 4: Run to watch them pass** — `cargo test --lib schematic` — Expected: PASS.

- [ ] **Step 5: Look at it** — render `samples/schematic_hero.lini` at `--zoom 3`, crop the `24 V entry` and `5 V buck` blocks, read them. This is the task with the largest visual payoff; the grounds should form one line per block.

- [ ] **Step 6: Commit**

```bash
git add -A src/layout/schematic
git commit -m "A scope's grounds sink to one row, its flags rise to one

Every drafted sheet does this and it is the single biggest thing ours
was missing: a one-cap column and a three-part divider end on the same
line. Vertical only — a chain running out along a pin's row ends where
it ends, which is what both reference sheets draw."
```

---

### Task 9: `readout.rs` — the ref/value side rule

**Files:**
- Create: `src/layout/schematic/readout.rs`
- Modify: `src/layout/schematic/place.rs` (call it after `rails`)
- Modify: `src/layout/schematic/field_tests.rs`
- Modify: `src/layout/schematic/net.rs` — the run's text side reads the field, not "the freer side"

**Interfaces:**
- Produces:

```rust
/// Re-seat a seated part's ref/value pair on the side away from its anchor
/// [SPEC 16.2]. Desugar placed the pair in the part's own default seat; the
/// field is the first pass that knows which side of the anchor it stands on.
pub(super) fn readouts(children: &mut [PlacedNode], field: &Field);
```

- [ ] **Step 1: Write the failing tests**

```rust
use super::tests::chrome;

#[test]
fn a_left_field_part_wears_its_readouts_to_its_left() {
    let src = scope("", &(sided("u1") + "  |C#c1| \"100n\"\n  |gnd#g1|\n  u1.a - c1 - g1\n"));
    let nodes = laid(&src);
    let text = chrome(&nodes, "c1", "part-value");
    assert!(text.max_x <= at(&nodes, "c1").0, "outward, on the left");
}

#[test]
fn a_right_field_part_wears_them_to_its_right() {
    let src = scope("", &(sided("u1") + "  |C#c1| \"100n\"\n  |gnd#g1|\n  u1.b - c1 - g1\n"));
    let nodes = laid(&src);
    let text = chrome(&nodes, "c1", "part-value");
    assert!(text.min_x >= at(&nodes, "c1").0, "outward, on the right");
}

#[test]
fn a_part_riding_a_row_wears_them_above_and_below() {
    let src = scope("", &(sided("u1")
        + "  |R#r1| \"1k\"\n  |label#n1| \"NET\"\n  u1.a - r1 - n1\n"));
    let nodes = laid(&src);
    let (rx, ry) = at(&nodes, "r1");
    let (r, v) = (chrome(&nodes, "r1", "ref"), chrome(&nodes, "r1", "part-value"));
    assert!(r.max_y <= ry && v.min_y >= ry, "ref above, value below");
    assert!(close(r.center().0, rx) && close(v.center().0, rx), "centred");
}

#[test]
fn a_readout_never_moves_a_part() {
    // [SPEC 16.1] ink never places: a long value overhangs its neighbour's
    // column rather than parting the columns.
    let short = laid(&scope("", &(sided("u1")
        + "  |C#c1| \"1n\"\n  |C#c2| \"1n\"\n  |gnd#g1|\n  |gnd#g2|\n"
        + "  u1.a - c1 - g1\n  u1.a - c2 - g2\n")));
    let long = laid(&scope("", &(sided("u1")
        + "  |C#c1| \"1n\"\n  |C#c2| \"4700000pF x7r 25V\"\n  |gnd#g1|\n  |gnd#g2|\n"
        + "  u1.a - c1 - g1\n  u1.a - c2 - g2\n")));
    assert!(
        close(at(&short, "c1").0 - at(&short, "c2").0, at(&long, "c1").0 - at(&long, "c2").0),
        "the columns stand where the lattice put them, whatever the value reads"
    );
}
```

- [ ] **Step 2: Run to watch them fail** — Expected: FAIL, readouts sit where desugar's default seat put them.

- [ ] **Step 3: Implement**

`readouts` walks every seated satellite, reads its `Seat`, and rewrites the `pin:` / offset attrs on its `lini-ref` and `lini-part-value` children:

- a seat whose `ray` is vertical (a lane part) → both readouts on the side away from the anchor, `text-anchor` `end` for a left field and `start` for a right one, stacked about the part's middle at `READOUT_OFFSET`;
- a seat whose `ray` is horizontal (riding a row) → ref above, value below, centred.

Desugar's `readout_at` keeps producing the default seat; this pass only moves it. Do **not** duplicate `readout_at`'s offset arithmetic — factor the shared piece into one function both call, per the no-parallel-implementations rule.

In `net.rs`, replace `forced_side`'s "freer side" measurement with the field reading: away from the anchor whose field the run sits in, ties on the routing side rank. This retires the third of the three symptoms named in the design.

- [ ] **Step 4: Run to watch them pass** — `cargo test --lib schematic` — Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A src/layout/schematic
git commit -m "Readouts step outward from the anchor, and never place anything

A part in the left field wears its ref and value to its left, right
aligned; one in the right field to its right; one riding a row wears them
above and below. The net run's name takes the same reading, which retires
the last of the three symptoms that would not sit still: it was measured
at the run's naive position rather than the line it lands on."
```

---

### Task 10: the router's track quantum

**Files:**
- Modify: `src/routing/ortho/scene/mod.rs` — `SceneNode` gains `quantum`, `SceneIndex` a root setter and a `quantum(WorldKey)` reader
- Modify: `src/routing/ortho/mod.rs:40` — `World` gains `quantum: Option<f64>`
- Modify: `src/routing/ortho/world.rs:31` — `build_worlds` copies it in
- Modify: `src/routing/ortho/place.rs:274-330` — `chain_prefs`'s interior-run arm rounds
- Modify: `src/routing/mod.rs:42` — `route` supplies the root scene's quantum
- Modify: `tests/routing.rs`

**Interfaces:**
- Consumes: `crate::resolve::is_schematic`, `crate::ledger::consts::PIN_PITCH`.
- Produces:

```rust
impl SceneIndex {
    /// The root scene's own track quantum — the root world has no scene node
    /// to read it off, so the caller supplies it.
    pub(crate) fn with_root_quantum(self, q: Option<f64>) -> Self;
    /// A world's track quantum (ROUTING.md §Vocabulary), if its scope states one.
    pub(crate) fn quantum(&self, key: WorldKey) -> Option<f64>;
}
```

- [ ] **Step 1: Write the failing test**

In `tests/routing.rs`:

```rust
#[test]
fn a_schematic_worlds_interior_runs_land_on_its_track_quantum() {
    // ROUTING.md §Vocabulary: a schematic scope states the fine pitch, so a
    // bare run bending between two gridded parts lands on their grid.
    let src = "|schematic#s| [\n\
               \x20 |component#u1| [ |pin#a| { side: left } ]\n\
               \x20 |component#u2| { cell: 2 1 } [ |pin#b| { side: bottom } ]\n\
               \x20 u1.a - u2.b\n]\n";
    let routed = route_of(src);
    for w in &routed.links {
        for p in &w.path {
            assert!(
                (p.0 / 20.0).fract().abs() < 1e-6 || (p.1 / 20.0).fract().abs() < 1e-6,
                "a corner off the quantum: {p:?}"
            );
        }
    }
}
```

Use the file's existing routing-result helper rather than inventing one — read the top of `tests/routing.rs` and follow it.

- [ ] **Step 2: Run to watch it fail** — `cargo test --test routing` — Expected: FAIL on at least one corner.

- [ ] **Step 3: Implement**

`SceneIndex::build` records `is_schematic(&n.attrs).then_some(PIN_PITCH)` per node; `with_root_quantum` sets the root's; `quantum(key)` reads either. `build_worlds` copies it onto `World`. In `chain_prefs`'s interior-run arm, after `clipped`, round: `let t = q.map_or(raw, |q| (raw / q).round() * q).clamp(clipped.walls.0, clipped.walls.1)`. The canonical-U arm is untouched — it names a side line, not a corridor anchor. `routing::route` passes `is_schematic(&program.scene.attrs).then_some(PIN_PITCH)`.

- [ ] **Step 4: Run to watch it pass** — `cargo test --test routing && cargo test --test laws` — Expected: PASS. The law checker must stay clean: the quantum moves a preference inside its lawful range, so no law can break; if one does, the clamp is wrong.

- [ ] **Step 5: Commit**

```bash
git add -A src/routing tests/routing.rs
git commit -m "A world may state a track quantum, and a schematic states its pitch

An interior run's preferred ordinate rounds to the world's quantum and
clamps back into its corridor, so a bare run between two gridded parts
bends on their grid instead of on a channel midline. Preference only —
the four laws never read it, and a world without one routes byte for byte
as before."
```

---

### Task 11: samples, constants, and the visual gate

**Files:**
- Modify: `samples/schematic_hero.lini`, `samples/schematic.lini`, `samples/schematic_blocks.lini`, `samples/schematic_parts.lini`
- Modify: `src/ledger/defaults.rs` (`SCH_GAP`, if the eye says so)
- Create: `src/layout/schematic/field_tests.rs`'s invariant sweep
- Modify: `tests/snapshots/*` (accepted `insta` output)

- [ ] **Step 1: Write the invariant sweep**

```rust
/// Every part a schematic scope placed lands on the fine lattice
/// [SPEC 16.1] — the analogue of the routing law checker, judged on output.
#[test]
fn every_sample_lands_on_the_lattice() {
    for path in ["samples/schematic.lini", "samples/schematic_hero.lini",
                 "samples/schematic_blocks.lini", "samples/schematic_parts.lini"] {
        let src = std::fs::read_to_string(path).expect("a sample");
        let laid = crate::testutil::laid(&src);
        for (id, x, y) in schematic_parts(&laid.nodes) {
            assert!(on_fine_grid(x) && on_fine_grid(y), "{path}: '{id}' at {x} {y}");
        }
    }
}
```

`schematic_parts` walks the placed tree collecting every descendant of a schematic scope whose `type_chain` names a schematic type, with its accumulated centre. Write it in `tests.rs` beside the other helpers.

- [ ] **Step 2: Run it** — `cargo test --lib schematic::field_tests::every_sample_lands_on_the_lattice` — Expected: PASS, or a named part to fix.

- [ ] **Step 3: Render and read every sample**

```bash
cargo build --release
for s in schematic schematic_hero schematic_blocks schematic_parts; do
  ./target/release/lini --static samples/$s.lini -o /tmp/$s.svg
  resvg --zoom 3 /tmp/$s.svg /tmp/$s.png
done
```

Read all four PNGs, cropping per block with `sips -c H W --cropOffset Y X`. Compare against the reference: `pdftoppm -r 110 -f 2 -l 2 -png ~/Desktop/fadec.pdf /tmp/ref`. Judge: are columns evenly pitched, do slot rows line up, is there one ground line per block, do the readouts read outward, are there stray long runs or large empty gaps?

- [ ] **Step 4: Tune `SCH_GAP` by eye**

The default starts at 100. If values collide at 100, raise it; if blocks read sparse, lower it (always a multiple of `PIN_PITCH`). Change one number, re-render, re-read. Record the chosen value's reasoning in the constant's doc comment.

- [ ] **Step 5: Rewrite the samples**

`schematic_hero.lini` was written against the old engine — its per-block `gap: 100`, the `translate:` on `U1.FB` and the comments explaining ladders and lanes are all workarounds for machinery that no longer exists. Delete the workarounds; keep the circuit. Each sample keeps its role: one per feature cluster, extended rather than multiplied. Update every SPEC-citing comment to the rewritten §16.1.

- [ ] **Step 6: Accept the snapshots**

Run: `cargo insta test --review` (or `cargo test` then `cargo insta accept` once the PNGs have been read and judged good — never accept a snapshot you have not looked at).

- [ ] **Step 7: Commit**

```bash
git add -A samples src tests
git commit -m "The showroom, on the grid

Every sample re-rendered and read against a real sheet: columns on one
pitch, slot rows shared, one ground line per block, readouts outward. The
hero loses the workarounds it carried for the old engine — a per-block
gap override, a pin nudge, and a paragraph of comments about lanes — and
keeps its circuit. SCH_GAP settles where the eye put it."
```

---

### Task 12: the audit round

**Files:** everything the rebuild touched.

- [ ] **Step 1: SPEC ↔ code audit**

Re-read `SPEC.md` §16 against `lattice.rs`, `field.rs`, `pack.rs`, `rail.rs`, `readout.rs`, `place.rs`. Every sentence must be implemented and every behaviour must be stated; where they disagree, decide which is wrong and fix that one. Re-read `ROUTING.md`'s new clause against `place.rs`'s `chain_prefs`.

- [ ] **Step 2: Diagnostics**

`hints.rs`'s two warnings must still fire: a chain with no placed end, and a third placed end dropped. Check `tests/diagnostics.rs` covers both against the new engine, and that no error message names a deleted mechanism ("seat gap", "ladder", "lane pitch").

- [ ] **Step 3: Regenerate the derived artifacts**

```bash
cargo run --release -p xtask -- gen-schema
cargo run --release -p xtask -- gen-grammars
cargo run --release -p xtask -- wasm
```

- [ ] **Step 4: The full gate**

```bash
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --all-targets
cargo test
```

Expected: all clean. Every module under ~500 lines; check with `wc -l src/layout/schematic/*.rs`.

- [ ] **Step 5: Delete the plan documents**

Per `AGENTS.md`, a completed round's doc is deleted — git history is the archive.

```bash
git rm SCHEMATIC-GRID.md SCHEMATIC-GRID-PLAN.md
```

- [ ] **Step 6: Commit and hand back**

```bash
git add -A
git commit -m "The lattice round closes: SPEC and code agree, and the plan goes

A pass over every sentence of SPEC 16 against the six passes that
implement it, the generated artifacts regenerated, and the round's
planning documents deleted — git history is the archive."
```

Report to the user: what landed, what the samples look like, and what the round chose to leave alone. **Do not push to `main`** — that is the user's call.

---

## Self-review

**Spec coverage.** §2 lattice → T3. §2.1 ink never places → T4/T5, asserted by T9's `a_readout_never_moves_a_part`. §2.2 ray/lane/slot → T4, T5. §2.3 collision and lane order → T4. §2.4 rails → T8. §2.5 packing and alignment → T7; spans and bridges → T6. §2.6 readouts → T9. §2.7 router → T2, T10. §3 shape → the File Structure table. §4 constants → T3, retuned at T11. §6 testing → tests in every task plus T11's sweep. §7 phases → T1–T12.

**Known gap, accepted:** the design's "component pin rail snapped so its pins land on fine lines" (§4) is stated in SPEC at T1 Step 3 but has no task of its own — it belongs to desugar's `assemble_component`, and whether it is needed depends on whether an even pin count actually lands on half-pitches. **T11 Step 2's invariant sweep is what discovers it**; if a component's pins are off the grid, fix it there and say so in the commit.

**Type consistency.** `Lattice`, `Ax`, `Seat`, `Field`, `Spanning`, `Packing` are each defined once, in the task that creates them, and later tasks use those exact names. `drawn` and `edges` move from `seat.rs` to `field.rs` at T5 and every caller is named. `Slot` (the ordinal track cell) stays `place.rs`'s and is distinct from a `Seat`'s `slot` field — do not merge them.

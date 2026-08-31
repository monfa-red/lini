# `layout: stack` — a general-purpose datum layout

Extract the placement core that `layout: drawing` already owns into a basic
layout of its own, so co-registered geometry stops requiring an engineering
drawing. Two follow-on fixes ride along: a generated-chrome removal bug and a
`fmt` comment bug, both found while rebuilding `samples/logo.lini` from CAD.

**Nothing here is implemented.** Another agent is fixing schematic bugs; this
file is the only thing this round has added to the repo.

---

## 1. Why

`|sketch|` is effectively unusable outside `layout: drawing`, because flow throws
away the one thing a pen frame is for:

```
{ padding: 0 }
|sketch#a| { draw: move(0, 0)  right(40) down(20) close() }
|sketch#b| { draw: move(10, 5) right(20) down(10) close() }
                    → translate(-48,-10)  translate(18,-10)    ← flowed apart
```

A single sketch is fine; **co-registered** geometry is what's missing. So today
any artwork built from several sketches — `samples/logo.lini` is the live
example — must declare `layout: drawing` and then fight the engineering
apparatus that comes with it (auto chrome, annotation link semantics, the
"needs at least one geometry child" rule).

The engine is already shaped for the split. `drawing::engine::lay_out` is ~50
lines, and the placement core is *lay each child out under a ctx and don't move
it*; everything else in that function is the geometry-child check, the mate
walk, and annotation lowering.

## 2. What `layout: stack` is

> Every child's **origin** lands on the container's datum — not its bbox centre.
> A symmetric primitive's origin is its centre, so primitives stack concentric;
> a `|sketch|`'s origin is its **pen origin**, so several sketches keep their
> drawn relationship. `translate:` is the only offset. No flow, no annotations,
> no chrome.

That paragraph is SPEC 15.1's datum law, minus the drafting. `drawing` becomes
`stack` + annotations + chrome + mates; `floorplan` follows for free, since it
*is* the drawing engine.

**Name.** `stack`, after Flutter's `Stack` and SwiftUI's `ZStack`. Not
`stacked` (Flutter's spelling of the same idea, and it reads as a state rather
than a thing); not `datum` (CAD jargon — a logo author won't parse it);
`canvas` is unavailable (reserved structural class, SPEC 23).

**Position.** After `grid`, before `tree`, everywhere the layouts are ordered.
It is a *basic arranger*, not a domain engine — that placement is what makes
the SPEC change cheap (§4).

### Decided semantics

| Question | Decision | Why |
|---|---|---|
| Links | hand to the **router**, like flow/grid | absolute placement *plus* ordinary arrows is the "fine-tuned diagram" use case |
| Lowers its subtree? | **no** — arranges in place | nothing to emit; only `drawing` lowers, for its annotations |
| `gap` / `direction` / `gap-fill` | ignored | no flow to tune; matches `drawing`'s row in SPEC 11 |
| `align` / `justify` | ignored, **with a warning** | they mean nothing once every child is on the datum — the CSS `position: absolute` reading. Silence would let an author think they had done something |
| `padding` | honoured | it frames the placed extent |
| Minimum children | **none** | `drawing`'s "needs at least one geometry child" is an annotation-target rule, not a placement rule |
| Chrome | **none generated** | already true outside drawing scopes — verified, §6 |
| `\|stack\|` node | yes — `\|block\|` + `layout: stack`, mirroring `\|drawing\|` | SPEC 8 symmetry |

### Units — pixel by default, millimetres on request

Today `unit:` errors outside a drawing scope:

```
{ padding: 0; density: 10; unit: mm }
  → error: 'unit' has no meaning on the root block — it reads on '|drawing|' / '|axis|'
```

The derived number is unchanged — `px-per-unit = ratio × unit-in-mm × density`.
Add **`px`** as a `unit:` value meaning *one drawing unit is one pixel*, which
makes the non-physical case nameable instead of a fudge:

| Scope | Default | px per unit |
|---|---|---|
| `layout: stack` | `unit: px` | **1** — draw in pixels, get 1 : 1 |
| `layout: drawing` / `floorplan` | `unit: mm`, `density: 4` | 4 |

Either is overridable from either side, so `samples/logo.lini` opts back into
the model's true millimetres with `layout: stack; unit: mm; density: 10`.

`density:` is *pixels per millimetre*, so under `unit: px` there are no
millimetres for it to convert. **`density: 1` is accepted** — it is the identity
and agrees with what `px` already means; any other value **warns** and is
ignored, so a `density: 4` copied from a drawing cannot quietly do nothing.

## 3. Property rename: `stack:` → `multiple:`

`stack:` the property must vacate the word. It is **not** a count — it draws
exactly one duplicate, and the value is that copy's offset:

```
|box#a| "X" { stack: 4 }   →   two <rect>s: the node, and <g transform="translate(4,-4)">
```

The copy carries the node's own fill and stroke, so the effect is
**multiplicity** ("there are several of these" — UML's multiobject, D2's
`style.multiple`), not depth. Lini already owns depth with `shadow: dx dy blur`.
Hence `multiple:`, which names the meaning rather than the mechanic.

Value shape is unchanged: `multiple: N` (scalar `N` ⇒ `N -N`) or `multiple: dx dy`.

**Sites** (5):

| File | What |
|---|---|
| `src/ledger/properties/mod.rs:402` | the property row |
| `src/ledger/examples.rs:98` | `("stack", "\|box\| \"deck\" { stack: 4; }")` |
| `src/render/primitives.rs:94` | `stack_offset()` reads `attrs.get("stack")` |
| `schema/lini.schema.json:2157` | the schema entry |
| `samples/styles.lini:49`, `samples/hero.lini:58` | the two authored uses |

`src/layout/drawing/engine/tests.rs:565` and the `drawing_gdt` snapshot use
`stack` as an **id**, not the property — leave them.

## 4. SPEC changes

### The renumbering hazard does not apply

SPEC 12's subsections are **unnumbered** — every reference in the repo is a bare
`[SPEC 12](#12-flow-grid--tree)`, never `[SPEC 12.2]`. So `stack` lands as a new
`### Stack` subsection between `### Grid` and `### Tree`, and:

> **No section number changes anywhere. No cross-repo renumber pass.**

This is the whole reason to keep `stack` in SPEC 12 rather than giving it a
top-level section. A new `## 13. Stack` would push 13→14 … 24→25 and force a
descending rename of every `[SPEC N]` in SPEC, `src/`, `samples/`, `tests/`,
`SKILL.md`, and `ROUTING.md`. Not worth it for a subsection-sized idea.

*If a later round does need a new numbered section*: renumber **from the highest
number downward** so no two sections ever share a number mid-pass, and treat
`grep -rn "SPEC [0-9]" .` as the checklist.

### Section-by-section

| SPEC | Change |
|---|---|
| **11** — The Layout Model | new table row `stack` **between `grid` and `tree`**: *"datum / geometry · children's origins on one datum · orthogonal router · no — arranges in place"*. Extend the "Universal container properties" prose that currently reads "a `sequence`, `chart`/`pie`, or `drawing` container places its own children and ignores them" to include `stack`. |
| **12** — retitle **Flow, Grid, Stack & Tree** | new `### Stack — one datum, no flow` subsection after `### Grid`. States the datum law, `translate:` as the only offset, `unit: px` default, that links route normally, and that it is the placement core `drawing` builds on. Update the TOC line and the anchor `#12-flow-grid-stack--tree` — **and every `[SPEC 12](#12-flow-grid--tree)` link in the repo**, which is an anchor edit, not a renumber. |
| **8** — Templates | `\|stack\|` row: `\|block\|` + `layout: stack`. |
| **15** — Drawing | reframe the opening: a drawing **is** `stack` + measuring ops + leaders + mates + chrome. 15.1's datum/scale prose moves to SPEC 12 and 15.1 cites it. Add `px` to 15.1's `unit:` list and state the per-layout default. The global/drawing-scope-only table gains a note that the placement model itself is now global. |
| **15.11** — Floorplan | one line: it inherits the same core through `drawing`. |
| **17** — Property Ledger | `stack` column in the container × layout matrix; `unit:`/`density:`/`scale:` rows extend to it; rename the `stack` property row to `multiple`. |
| **21** — Errors | the `'unit' has no meaning on the root block` message must learn about `stack`. Decide the `density:`-under-`px` diagnostic (§8). |
| **22** — Grammar | `layout:` value list. |
| **23** — Reserved Words | add `stack` to the contextual value keywords list (`flow`, `grid`, `tree`, `sequence`, …). Note the property/value pair is grammatically fine either way — the rename is for readers, not the parser. |
| **7** — Nodes | `stack` → `multiple` in the closed-primitive modifier list (SPEC 673, 707). |

## 5. Code changes

| File | Change |
|---|---|
| `src/resolve/ir.rs:253-266` | the seam. `is_drawing_layout` currently `matches!(name, "drawing" \| "floorplan")`. Add `is_stack_layout`, and make drawing/floorplan *imply* it. |
| `src/layout/stack.rs` **(new)** | the extracted core: place each child's origin on the datum, recentre the extent, place pinned overlays. Lifted from `drawing/engine/mod.rs` `lay_out` / `layout_node` / `flow_extent`. Keep under ~200 LOC. |
| `src/layout/drawing/engine/mod.rs` | re-base on it: call the stack core, then add the geometry-child check, `place_features`, `mates::seat`, section chrome, `annotate::lower`. |
| `src/layout/mod.rs:200-213` | `Ctx.drawing` stays the chrome/annotation gate; a stack scope leaves it `false`, which is already what suppresses chrome (§6). Add whatever the scale trio needs to reach a stack scope. |
| `src/desugar/types.rs:110`, `src/desugar/nest.rs` | register `stack`; `\|stack\|` template. |
| `src/ledger/defaults.rs`, `src/ledger/properties/mod.rs` | layout row, matrix entries, the `multiple` rename. |
| `schema/lini.schema.json` | `layout` enum + the `multiple` rename. |
| `src/error/…`, `src/suggest.rs` | did-you-mean for `stack`; the `unit:` message. |

## 6. Two independent bugs found alongside

Both are real today and neither is fixed by `layout: stack`. Worth their own
commits.

### 6a. Chrome styled away still occupies the bbox

SPEC 15.7 says chrome is "styled **or removed** by the cascade
(`|sketch| |centerline| { stroke: none }`)". Removal is not implemented — only
hiding:

```
{ layout: drawing; density: 1; padding: 0 }
|sketch#half| { draw: move(0,0) right(40) down(20); mirror: y-axis; … }

visible        → viewBox -40 -3.5 80 27
stroke: none   → viewBox -40 -3   80 26      ← still measured
                 <line x1="0" y1="23" x2="0" y2="-3"/>   invisible, still there
no drawing     → viewBox -40  0   80 20      ← chrome correctly never generated
```

That invisible `<line>` is the mystery space at `padding: 0`, and the reason
`samples/logo.lini` currently carries `padding: 25 25 22 25`.

**Fix, one rule at the chrome layer:** generated chrome that resolves to no
paint is **dropped before measurement** — not emitted, not measured. Covers
`|centerline|`, `|pitch-circle|`, `|shoulder|`, `|breakline|`, and the
floorplan's indexed chrome with one mechanism. Note the test is "nothing would
be painted" (stroke *and* fill), not "stroke is none", so a floorplan door leaf
with a fill survives.

**Decided:** this is a bug fix, not a new knob. `stroke: none` in CSS zeroes the
stroke and the line ceases to exist; chrome should read the same way, in a
drawing as much as anywhere — an author may legitimately not want that line on
an engineering sheet either. No `chrome: none` property.

The last line of the block above is the good news: chrome is already correctly
gated to drawing scopes, exactly as SPEC 15's global/scope-only table claims. So
`layout: stack` inherits the right behaviour with no new gating code.

### 6b. `lini fmt` moves a trailing comment below its statement

```
|box#a| "A"    // the hero      →      |box#a| "A"
                                       // the hero     ← now annotates the NEXT node
```

It inverts what the comment refers to, so it is a correctness bug, not a style
preference. It is also why a parametric file cannot keep an aligned constants
table — the `samples/logo.lini` rebuild had to move every annotation into prose
blocks above each group.

## 7. Validation

- `insta` snapshots for the new layout: several sketches co-registered; a
  `translate:`d child; a stack scope with routed links; `unit: px` 1 : 1 vs
  `unit: mm; density: 10`.
- The regression that matters: **`samples/logo.lini` on `layout: stack` must
  render byte-identical to its `layout: drawing` output minus the padding
  workaround** — 290 × 190 at an even `padding: 25`, once 6a removes the
  phantom overhang. The CAD rebuild is otherwise carried over verbatim; its
  path data is already verified against `Lini.step`'s eight B-rep solids to
  4 dp, so any drift in the emitted `d` attributes is a real regression.
- Every existing drawing/floorplan snapshot must be unchanged by the re-base.
  If any moves, the extraction changed behaviour and is wrong.
- `cargo fmt`, `cargo clippy`, `cargo test`, and `lini --strict` on every sample.

## 8. Decisions taken

1. **`density:` under `unit: px`** — `density: 1` accepted (the identity); any
   other value warns and is ignored. §2.
2. **`samples/logo.lini` moves to `layout: stack`** in this round, carrying the
   CAD rebuild verbatim and reverting to an even `padding: 25` once 6a lands.
   §7, §9.
3. **`align` / `justify` warn** in a stack scope — they mean nothing when every
   child is already on the datum. §2.
4. **Chrome removal is a bug fix**, not a `chrome: none` knob. §6a.

Still open, and not mine to answer: **should `schematic` re-base on the core
too?** The plan says no — it seats anchors on tracks, a different placement
model — but the agent currently in `src/layout/schematic/` would know better.
Worth one question to them before step 4.

## 9. Sequencing

The `multiple:` rename and 6a/6b are independent of the layout work and each
other. Suggested order, smallest blast radius first:

1. `stack:` → `multiple:` (5 sites, no behaviour change)
2. 6b — `fmt` trailing comments
3. 6a — chrome removal; `samples/logo.lini` back to an even `padding: 25`
4. `layout: stack` + the drawing re-base + SPEC 11/12/15/17 (the big one)
5. `samples/logo.lini` onto `layout: stack`
6. **Audits** — §10, both agent-run
7. `SKILL.md` pass — §11, its own session

Steps 1–3 are safe to land beside schematic work; step 4 touches
`src/layout/` broadly and should wait for a clear tree.

## 10. Audits — agent-run

Two audits, after the edits land. Both are cleanly isolated, so both are agent
work: **`model: opus`**, xhigh-tier tasks, at most **two concurrent** (5 parallel
has OOM'd this repo before). Never Sonnet — it summarises here, it does not
judge Lini.

Every brief must carry these, spelled out rather than left to `AGENTS.md`:

- **Read the whole artefact.** SPEC.md is ~5,000 lines and the audit is worthless
  on excerpts. Say "read SPEC.md end to end before reporting" explicitly.
- **The parallel-implementation check.** Name it: *does any change introduce a
  second place doing a job an existing one already does?* For this round the
  specific trap is the drawing engine **copying** the stack core instead of
  calling it — divergent copies drift, and a fix would land in one path while
  the other rots.
- **Report, don't fix.** Findings with file:line and a one-line failure
  scenario; no edits.

### Audit A — SPEC.md, whole document

Not scoped to this round's edits. SPEC.md has taken several rounds of edits
without an audit, and it is the single source of truth, so the sweep is the
point. Brief covers:

- **Trim the execution log.** Past rounds have treated SPEC.md as a changelog —
  prose that records *what a fix repaired* or *why a round changed something*
  rather than stating what the language **is**. Find it and cut it. SPEC.md is
  the source of truth, not the history; git is the archive. This is a first-class
  goal of the audit, not a tidy-up: report each passage with file:line and the
  compacted replacement, so the trim is reviewable rather than a bulk rewrite.
- **Internal consistency** — a rule stated in one section contradicted in
  another; the property ledger (SPEC 17) disagreeing with the section that owns
  the property; the SPEC 11 layout table disagreeing with SPEC 12–16.
- **Spec vs code** — where the prose is ambiguous or looks stale, check the
  implementation and report the divergence. The chrome-removal wording in 15.7
  is a known live example (§6a): SPEC promised behaviour the code never had.
- **Numbering and links** — every `[SPEC N]` and `[SPEC N.M]` resolves, every
  anchor matches its heading, the TOC matches the body. The `#12-flow-grid--tree`
  → `#12-flow-grid-stack--tree` change makes this pass mandatory this round.
- **Reserved words (SPEC 23) and the grammar (SPEC 22)** actually list what the
  language accepts.

### Audit B — the code change, light

Scoped to the diff. Brief covers: the extraction is a genuine *move* (drawing
calls the core, no copied logic); no behaviour change to existing
drawing/floorplan snapshots; the `multiple:` rename caught every site including
the schema; the new warnings fire where §2 says and nowhere else; `no unsafe`;
modules under ~500 LOC.

A third, smaller agent job if wanted: the repo-wide
`#12-flow-grid--tree` → `#12-flow-grid-stack--tree` anchor sweep — mechanical,
verifiable by grep, and easy to get 90% right and 10% wrong by hand.

## 11. SKILL.md gaps this round exposed

Collected for the SKILL session, not this one:

1. `mirror:` is one clause and omits the rule everything turns on — **open
   subpath → fused, closed subpath → duplicated** — plus `y-axis`, the list
   form, `mirror: none`, and that a fuse generates a centerline.
2. The pen's `arc` is undocumented: two forms (`arc(dx,dy,r)` two-point *minor*
   arc vs `arc(r,deg)` *tangent* arc), the sign conventions (`> 0` clockwise for
   both), the bearing convention (up = 0, clockwise), and that a tangent arc
   needs a prior heading — so it cannot follow `move()`.
3. The y-down / verbs-visual split: `move`/`line`/`curve` take raw y-down
   coordinates while `up`/`down` are visual. A guaranteed sign bug on first
   contact; no example in SKILL uses a negative coordinate.
4. `density:` does not appear at all.
5. "Math needs parens" misleads — the real rule is *a call's own parens count*,
   so `move(-tail - quarter, -notch_y)` needs no group. No mention of the math
   library (`sqrt`, `sin`, `clamp`, …).
6. `fmt` relocates trailing comments (6b) — worth a line once fixed.
7. `pattern:`: no statement that **the seed is copy one** (`grid(1, 3, …)` gives
   three, not four), that it is legal in any layout, or the `radial()` form.
8. Nothing says a drawing with no annotations is just an exact vector canvas —
   which `layout: stack` will make the correct answer instead.
9. `opacity:` is in no property list.
10. `stack: N` is glossed "a pile", implying many. It is one copy, and the value
    is its offset (§3).

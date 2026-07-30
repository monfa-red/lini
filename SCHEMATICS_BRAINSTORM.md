# PCB Schematics — brainstorm notes (beta 2 candidate)

Session notes, 2026-07-29/30 (thirteen rounds). **Nothing here is decided** — this is
the state of the brainstorm. Reference schematic: a KiCad A5 page (TMC2300
stepper driver — `even-Z.pdf`, U7 + passives + connector + net labels +
gnd/power symbols + grouped regions + title block).

## What already exists — reuse, don't build

| Schematic need | Existing lini machinery |
|---|---|
| Sheet, zone borders (1–4 / A–C), centring marks | `|page|` — KiCad's sheet *is* ISO 5457 |
| Title block (Title / Rev / Id / Date / company) | `|title-block|` (ISO 7200) |
| The captioned blue region boxes ("Stepper Driver") | `|group|` + caption; restyle via a scoped rule |
| Wires | the orthogonal router, unchanged |
| Net name on a wire | the link label — `U7.VS - c24.p1 "VM"` works today |
| One-ended op + trailing text | drawing leaders (`bolt <- "THRU"`) — the net-label statement shape |
| Colours (dark-red outline, green wire, beige page…) | role variables + theme, the drawing precedent (`--stroke-dark`/`--stroke-light`) |
| Symbols drawn at text-relative size | the 15.9 drafting-symbol registry (natural units, not box-fit) |
| Custom power flags, part libraries | defines (`|vm::label| { … } [ "VM" ]` — intrinsic children materialize per instance) |
| Minted ids for anonymous nodes | `lini-topic-N` — the precedent for minted refs |

## Vocabulary

The industry pair is **component** (the part) / **symbol** (its drawing); KiCad
collapses both into "symbol". Lini keeps the full pair: `|component|` is the
instance, `symbol:` picks a drawing — the same meaning it already has on
`|icon|`. **"Symbol" is the one word** (round 5 — "glyph" dropped): `symbol:`
answers *which drawing to wear*, read from a per-type family — Phosphor on
`|icon|`, the drafting set in drawings, the **schematic symbol set** on
`|label|` and the discretes. An icon is a node whose body *is* a pictogram; a
symbol is the name of the drawing it wears.

## Decisions leaned into

### 1. Architecture: a thin scope — `layout: schematic`

Chosen over pure sugar and over a full lowering engine. Placement stays
flow/grid-like (parts are placed manually — grid, translate); **the router keeps
the wires** (unlike sequence/drawing, which consume their links). The scope
gates reinterpretations and owns the small extras, as drawing reinterprets `>-`
and auto-places callouts:

- legalizes label wires (below) and the marker → label-shape mapping;
- auto-seats a label just off its pin (drawing's `note-offset` precedent);
- draws junction dots at fan (`&`) trunk splits — generated chrome;
- scoped rules give the schematic look.

Why not pure sugar (the `|table|` model): labels would have no owner — every
gnd/net label hand-placed, no one-ended wires, no junction home. Why not a
lowering engine: fights "the orthogonal router should do the wiring", and manual
placement is what schematics-for-showing want.

### 2. `|component|` + `|pin|`

```
|component#U7| "TMC2300-LA-T" [
  |pin#VS| { number: 18 };  |pin#STEP| { number: 4 }         // default side: left
  |pin#nstdby| "VIO/NSTDBY" { side: right; number: 11 }
]
U7.VS - c24.p1 "VM"
```

- **Schematic identity is displayed** (round-2 correction). A declared node's id
  is never its label (SPEC 3 — only auto-create labels from the id), so "label
  defaults to id" is out. Instead the type's **lowering draws the id as chrome**
  — the ref designator on a component/discrete, the pin name on a pin — the way
  a `|hole|` draws its centre marks. Desugar-visible, label untouched. The
  label keeps one job: the **value / part name**; on a pin it overrides the
  displayed name for strings that can't be ids (`"VIO/NSTDBY"`, `"1.8VOUT"`).
- **Minted refs** for auto-numbering: an anonymous discrete displays `R1`, `R2`…
  — prefix from the type, declaration order, authored ids win, minting skips
  taken names (the `lini-topic-N` precedent). Safety law: **minted refs are
  display-only, never endpoints** — inserting a part would renumber and
  silently re-wire otherwise. Don't care → free numbering; wire it → name it.
- **No authored rails.** Desugar generates per-side rails as *anonymous*
  containers — already scope-transparent, so `U7.VS` resolves with no rail in
  the path.
- **Pin sides, auto with override**: no `side:` → the bilateral split (first
  ⌈n/2⌉ left, rest right, declaration order — the mindmap rule). `side:`
  overrides.
- **Pin routing law**: a pin's connection point is its stub tip; departure is
  fixed outward along its side. `:side` on a pin endpoint is an **error** — the
  pin desugars to a forced-side endpoint the router already honours.
- **Pin anatomy** (name inside, `number:` outside, stub outward) is the pin
  type's lowering; stub length etc. are baked schematic constants.
- Single-pin components are legal (test point, J3's MP pad); an unwired part
  needs no id at all.
- `|pin|` + `pin:` is lini's first **type/property homonym** (one word, two
  roles) — never ambiguous (bars vs before-`:`); `scale`/`side`/`unit` are
  property/property precedents.

### 3. Discretes: symbol-bodied types (round 4 — settled shape)

Symbol-bodied nodes (natural-units machinery), generated pins (wires must land on
terminals). Smart label = the value; ref readout from the id / minted; orient
with `rotate:`. **Uppercase short types only** (full words dropped — the type
*is* the ref family; precedent for short types: `|cyl|`, `|hex|`).

**Minting rule**: mint prefix = the type name; `prefix:` (a string property,
read only at minting) overrides — `|ic::component| { prefix: "IC" }` mints
IC1, IC2… An authored id wins outright. Prefixes follow IEEE 315 / ASME
Y14.44; `|component|` (any generic pin-bearing box — IC, module, relay) mints
U. `|J|` ships as a built-in define over `|component|` — prefix J, pins
nameless (numbers only): the connector.

**Variants ride `symbol:`** — the property that answers "which drawing" on
`|icon|` and `|label|` — one mechanism for every variant family; the variant
also sets the generated pin ids where they're semantic:

| Type | Mints | Pins | `symbol:` variants |
|---|---|---|---|
| `\|R\|` | R1… | p1 p2 | (variable/pot — defer?) |
| `\|C\|` | C1… | p1 p2 | `polarized` |
| `\|L\|` | L1… | p1 p2 | — |
| `\|D\|` | D1… | a k | `zener` · `tvs` · `schottky` |
| `\|LED\|` | LED1… | a k | — (`prefix: "D"` for purists) |
| `\|Q\|` | Q1… | b c e / g d s | `npn` (default) · `pnp` · `nfet` · `pfet` |
| `\|Y\|` | Y1… | p1 p2 | — |
| `\|F\|` | F1… | p1 p2 | — |
| `\|FB\|` | FB1… | p1 p2 | — |
| `\|SW\|` | SW1… | p1 p2 | `toggle` (default) · `push` |
| `\|BT\|` | BT1… | plus minus | `cell` · `battery` |

Earth / chassis / antenna are **not** discretes — they're `|label|` symbols
(`symbol: earth | chassis | antenna`), the terminator family with gnd/power/nc.

**Standard: IEC only** — lini's drafting lineage is already ISO (SPEC 15.9).
Never per-type variants (`|R_ANSI|`): a sheet never mixes standards, so ANSI,
if ever, is one scope-level knob swapping the whole symbol family. Deferred.

### 4. Net labels, power, gnd: `|label|` + two statement forms

`|label|` (the name is free) is icon-shaped: smart label = the net text in the
tag outline; `symbol:` swaps in a drawing from the schematic symbol set —
settled 90% list: `gnd`, `earth`, `chassis`, `power`, `nc`, `antenna`. Text
alone = net label; symbol alone = gnd; symbol + text = power flag. Power nets are defines with intrinsic
text: `|vm::label| { symbol: power } [ "VM" ]`.

**Form 1 — text labels need no new grammar**: the drawing-leader statement
shape, reinterpreted by the scope:

```
U7.DIAG - "NSTDBY"      // one-ended wire + text → a tag, seated at the pin
U7.STEP -> "STEP"       // marker picks the tag shape (output)
```

Marker → shape (round 5 — settled, and **visual, not semantic**): `-` plain,
`->` the right-pointed tag, `-<` the left-pointed, `-<>` both, `-*` the round
one (KiCad's directive look). SPEC/docs *suggest* the conventional readings
(output / input / bidirectional); the compiler attaches no meaning — the
sequence stance (`->` vs `-->` draw differently, the reader supplies call vs
return).

**Form 2 — inline instantiation, proposed as a *core-wide* feature** (round-2
resolution of the `- |gnd|` consistency worry — generalize rather than
special-case; round-3 tightening — **bars only, no tail**): anywhere in lini,
an endpoint position after an op may hold an identity capsule:

```
c24.p2 - |gnd|                   // fresh anonymous gnd at the open end
U7.VS  - |vm|                    // fresh VM power flag (define carries "VM")
U7.VCP - |nc|                    // no-connect ✕ — another terminator symbol
cat -> |cyl#db|                  // any diagram: typed declare-and-link
cat -> |cyl#db| "watches" { … }  // tail = the LINK's, as with a bare endpoint
```

Desugar **hoists** the inline node to a declaration in the enclosing scope +
the link — pure sugar, *typed* auto-create. No new binding rule is needed;
two existing laws do all the work:

- **A statement's tail belongs to its head.** `a -> b "x" .hot { }` already
  labels/classes/styles the link, never `b` — an endpoint owns nothing. So the
  capsule takes no label, class, style, or children; everything after it is the
  link's. `a -> |cyl| "DB"` is therefore unambiguous: "DB" is the link label.
  The node's label/style live where they always do — a normal declaration, a
  define's intrinsic children (`|vm::label| { symbol: power } [ "VM" ]`), or
  an id + id rule (`a -> |cyl#db|` … `#db { fill: red }`).
- **Capsules are legal wherever endpoints are** (round 5 — head ban lifted):
  chains, fans, either end — `|cyl| -> a`, `a -> |cyl| -> c`, even
  `|cyl| - |gnd|` (two fresh nodes wired). One honest law adjustment: "the
  first token tells the statement's kind" relaxes to — a leading capsule opens
  a node statement *or* a capsule-headed link, decided by the **single token
  after the capsule** (an op → link; anything else → node declaration). Still
  bounded, no prescan; the capsule is self-delimiting. The round-3 mess stays
  dead for a different reason: `|box| "L" { } -> …` errors because a capsule
  endpoint takes no tail.

Gates and edges: **drawing scopes reject it** ("a drawing never invents an
endpoint" — the existing law); sequence allows it (a typed participant).
Anonymous mid-chain capsules get a minted internal handle. A fan into a
capsule is **one** instance (one declaration); per-wire grounds are separate
statements. Declaring an id twice is the ordinary duplicate-id error. Cost
accepted: label + wire in one line for a one-off node doesn't exist — two
statements, lini's normal price.

Rejected on the way here: bars + optional inline label (ambiguous — whose
"DB"?); the full inline node tail (two `{ }` owners in one statement, mess).

The explicit node form (`|label#step2| "STEP"` + a normal wire) remains for
stretched, styled, or shared labels; both sugars lower to it, desugar-visible.

Rejected alternatives, for the record: bare `- GND` (a shared id would collide
five gnd triangles into one node — "bare = referenced id" is load-bearing);
`{ flag: gnd }` per use (verbose, un-lini); a new sigil like `(gnd)` (parens
already mean math + measuring ops, and bars already mean "an instance of a
type" — a second mechanism for the same concept); auto-create type-swap in
scope (same shared-id collision).

### 5. Look & theme

New role variables (dark/light pairs, tree-shaken): component fill (pale
yellow), component outline (dark red), wire (green), label (teal), pin number
(muted), page background (beige). Schematic types carry defaults referencing
them — classic look by default inside the scope, retunable by theme.
`|schematic|` template = `|block|` + `layout: schematic`.

## Explicitly out of scope (for *showing*, not netlisting)

Hierarchical sheets/labels, ERC, netlist export, buses (thick wires — defer),
pin electrical types (input/output pin marks), auto part placement. Wire
connectivity is honest lini: connection is a shared endpoint or a fan `&`
(junction dot); crossings are just crossings.

## SPEC & release integration

- SPEC gets a new Part II section (a peer of Sequence/Charts/Drawing), written
  as if from day one — full refactor: templates, ledger (16), grammar (21 — the
  inline-instantiation form, if blessed, lands in the *core* link grammar),
  reserved words (22), errors (20), the symbol registry shared with 15.9.
- ROADMAP needs a **beta 2 row** (currently beta.1 → rc). Syntax isn't frozen
  until v1, so schematic gets first-class naming.

## Settled in round 4

- **Inline instantiation blessed, as a global law**: a routed-link endpoint is
  a bare `id`, a `|type|`, or a `|type#id|`; a capsule always instantiates
  inline and never takes a tail (no label/class/style/children — the tail is
  the link's). Drawing scopes keep rejecting it (no-invention law). A fan into
  a capsule = one instance; its trunk merge is a junction dot.
- **Discretes: uppercase short types only**; the table above, `symbol:`
  variants, type-name minting with `prefix:` override, IEC-only symbols.
- **Connectors resolved**: `|J|` = built-in define over `|component|`.
- Ids are displayed verbatim and case-sensitive — `|component#U7|` shows "U7"
  and is wired as `U7.VS` (not `u7.VS`).

## Settled in round 5 — open list emptied

- **Minted refs accepted** (display-only, never endpoints).
- **Marker → shape settled and visual-not-semantic** (see section 4).
- **Nested scopes**: schematic-ness **cascades** like `routing:`/`clearance:` —
  label wires and junction dots reach links written in nested flow/grid
  containers.
- **Junction**: the fan trunk-split dot lowers as a generated **`|junction|`
  chrome child** (the `|halo|` pattern — one rule restyles/removes sheet-wide),
  not a baked marker. Nothing is ever authored; `a & b -> c` is the authoring.
- **Terminology**: "glyph" dropped — **symbol** everywhere (see Vocabulary).
  `|label|` symbol set: `gnd`, `earth`, `chassis`, `power`, `nc`, `antenna`.
- **Capsules legal wherever endpoints are** — head ban lifted (see section 4);
  mid-chain anonymous capsules get minted internal handles.
- **Polar pin ids are semantic** (`d3.a`/`d3.k`, `q1.b c e` or `g d s` per
  variant, `bt1.plus`/`bt1.minus`); symmetric parts stay `p1`/`p2`.
- `|BT|` confirmed — IEEE 315 / KiCad both use BT; BATT is colloquial
  (`prefix:` covers taste).

## Settled in round 6 — wiring edge cases

- **Pinless wiring, 2-pin parts only**: a wire to a 2-pin part without a pin
  path takes the **next free pin in the type's pin order** (p1→p2, a→k,
  plus→minus) — deterministic, source order. Chaining *through* a 2-pin part
  reads **in series**: `vm - |R| - |LED| - |gnd|` is a series circuit in one
  line. A pinless wire to a 3+-pin part (`|Q|`, `|component|`) errors with a
  suggestion — no honest guess among many pins. Dangling pins are legal
  (`|R| -> a` lands p1, p2 stays open).
- **Marker gate**: in a schematic scope an op's *marker* part is legal only on
  a label wire terminating in a **text-form** `|label|` (markers shape the
  tag). A marked wire between parts, or a marker aimed at a symbol-form label
  (`|gnd|` has no tag), errors with a correction. The op's *line* part stays
  free everywhere — a dashed run (`--`) is just `stroke-style` (the PDF's
  COMM⌁RTX wire).
- **Shaped tag on a pin-to-pin wire = two statements**, no new mechanism:
  `U1.p1 -* "My Label"` (tag seated at the pin) + `U1.p1 - U2.p2` (the wire) —
  structurally KiCad's model (a label attaches at a point of the net). A label
  wire's stub may collapse to ~zero when the tag seats adjacent; **label wires
  never produce junction dots** — only real wire tees (`&` fans) do. Plain
  text labels keep the core form (`U1.p1 - U2.p2 "My Label"`). Mid-wire tags,
  if ever needed, are reserved as a `|label|` node riding the link's `[ ]` at
  an `along:` fraction (the drawing annotation-node seam) — deferred.
- Label defines stay lowercase (`|gnd|`, `|vm|`) — uppercase is for ref
  families.

## Settled in round 7 — capsule endpoint anatomy & the arity rule

- **A capsule composes with endpoint anatomy.** Two different tails: the
  *statement* tail (label/class/style — the link's; a capsule never takes it)
  vs the *endpoint* anatomy (`.path`, `:side` — the endpoint's). A capsule
  stands where the leading id segment would, the rest composes:
  `endpoint = (id | capsule) { "." ident } [ ":" side ]`.
  So `|cyl|:left -> a` (core, forced side) and `vm - |D|.k - x` (schematic,
  cathode-first — **the polarity answer**) are both legal. `|component#U9|.p4`
  parses but errors at resolve — an inline component has no authored pins (a
  capsule can't carry `[ ]`); discretes' pins are *generated*, so they exist
  the moment the instance does. In a schematic, `:side` never selects a pin —
  sides aren't terminals.
- **Pinless wiring gates on pin arity, never on a type list** — any
  pin-bearing part, authored or generated: **1 pin** → lands on it; **2 pins**
  → next free in the type's pin order (chain-through = series; both taken →
  error "name one"); **3+ pins** → error with a suggestion. `|Q| -> a` errors
  because Q has three pins, not because it's Q; a 2-pin authored `|component|`
  (a jumper) chains in series like an `|R|`.
- **Components have pins; a label is its own terminal.** A wire lands on the
  label's attachment point — no `|gnd|.p1`, no dot-path into a label, ever. An
  id'd label may take several wires (a star into one tag). Whether discretes
  desugar over `|component|` or a shared pin-bearing base with a symbol body
  is a SPEC-pass lowering detail; the pins are real generated `|pin|` children
  either way.

## Round 8 — adversarial review (ROUTING.md read end to end)

### Load-bearing findings — must be solved in the plan/SPEC

1. **Fixed-port routing is THE structural work item.** ROUTING.md's port model
   is "ports fall out of placement" (the ladder spreads landings along a
   side); a pin is the opposite — a **fixed** port at an exact ordinate.
   SPEC 23 already defers exactly this ("routed links to authored anchors —
   needs a ROUTING.md contract extension; ports and Law 2 are side-based").
   Schematics force building it: fixed ports, Law 2 amended (land *at the
   port*, perpendicular; the corner-margin rule waived for pins — end pins sit
   near corners), placement's no-braid logic coping with preassigned ports.
2. **Rotation**: `rotate:` is an SVG transform applied *after* routing
   (SPEC 5) — a rotated part would swing away from its wires. Fix: schematic
   parts read `rotate:` in **90° steps at lowering time** (pins re-side,
   geometry re-lays, then routing sees the truth); arbitrary angles error on
   pin-bearing parts.
3. **Series placement**: `vm - |R| - |LED| - |gnd|` wires correctly but the
   thin scope places hoisted parts by *flow*, not "on the wire". Saving
   grace: hoisted declarations are **consecutive flow siblings** — usually
   adjacent. Accepted for beta 2 (documented); wire-seating is a possible
   later refinement (note: a chain of all-capsules has no anchor to seat
   against, so it can never be the only mechanism).
4. **Same-pin tees**: two separate statements landing on one pin have no
   contract answer (ports are unique today). Fix: same-pin landings collapse
   into an **implicit fan at the port** (shared segment); junction law
   generalizes to "**a dot wherever ≥3 wire ends meet at a point**" (trunk
   splits and pin tees alike), label stubs excluded.
5. **Implicit auto-create dies in the scope** (drawing precedent):
   `U7.DIAG - NSTDBY` (forgotten quotes) must not mint a box — error with
   "did you mean `- \"NSTDBY\"` (a net label)?".
6. **Chain-through + explicit pin**: core expansion would share `.k` between
   hops (a tee at the cathode). Scoped fix: chaining through a 2-pin part is
   a **pass-through** — named (or next-free) pin = entry, the other = exit.
   `vm - |D|.k - x` is the reversed diode, as intended.

### Determinism details to pin in the SPEC (solvable, not vague-able)

- Label auto-seat when the pin also carries wires — deterministic dodge
  offset (perpendicular first, then along the side; translate overrides).
- Mixing explicit `side:` pins with the bilateral auto-split — autos split
  over the remainder.
- Ref/value smart-label placement per type (component: above; discrete:
  beside; deterministic, translate overrides).
- **`shape:`** property on `|label|` (plain · left · right · both · round);
  the wire op's marker is sugar setting it — exact precedent: the op's line
  part sets `stroke-style`. A *declared* label can therefore carry any shape.
- Pin chrome (stub, `number:`, name) folds into the **component's own
  obstacle** — never free-floating text obstacles (they'd choke 20 px pin
  pitch at default clearance).
- Schematic scope link defaults, like drawing's: smaller `clearance`
  (drawing uses 4), `stroke-width` ~1.5, wire colour role.

### Missing popular parts — user to decide

- **`|opamp|`** (mints U; pins `out` / `inp` / `inn`, power pins hidden by
  default) — huge in book schematics. **`|V|` / `|I|`** sources (SPICE
  prefixes, 2-pin, `symbol: dc | ac`) — circuit-theory teaching diagrams.
- Listed-and-deferred: pot (RV), transformer (T), relay (K), motor (M),
  speaker (LS), logic gates, crossing hop-over arcs.
- QoL: **`pins: N`** on `|J|` — generates N numbered pins (`J3` in one line).

## Round 9 — placement (the blocker) & routing dress

### Agreed

- **Duplicates error in scope**: `a - b` twice is meaningless in a schematic.
  `a - b; c - b` merges into the implicit fan (round 8) — one trunk, one dot.
- **Junction/fan needs no new strategy** — the same orthogonal router wearing
  scoped dress: (1) **corner radius → 0** (square elbows; the render-time
  rounding radius becomes a scoped constant), (2) **junction dots** as
  generated chrome at every computed ≥3-meet read off the routed geometry,
  (3) the duplicates error. Laws, search, channels untouched.
- **`|opamp|` and `|V|` / `|I|` are in** (op-amp: mints U, pins
  `out`/`inp`/`inn`, power pins hidden by default; sources: SPICE prefixes,
  2-pin, `symbol: dc | ac`).

### PROPOSED — the anchor + satellite placement model (pending verdict)

The reference sheet's own structure: every passive sits *beside the pin it
serves* (C24/C25 off VS, C26 by NSTDBY, R18/R19 by BRA/BRB, each gnd below
its cap). Real schematics = a few placed **anchors** + **satellites clinging
to pins**. Deterministic version, reusing drawing's annotation-placement
pattern:

- **Anchors** — multi-pin parts (3+ pins), or anything explicitly placed —
  go on the scope's **grid**: `layout: schematic` is grid-like, `cell:` is
  the "imaginary grid" (coarse, no translate), auto-flow when you don't care.
- **Satellites** — labels and *unplaced 1–2-pin parts* — **seat at the pin
  their wire touches**: outward along the pin's direction at a baked
  wire-length constant; a chain seats outward link by link; several satellite
  groups on one pin stack in statement order; the satellite orients so its
  entry pin faces its anchor. Seated satellites register as router obstacles
  (drawing's annotation-obstacle precedent). `cell:` / `translate:` opts out.
- A chain with **no placed end** falls back to flow with a warning (lean).
- This restores the capsule's usefulness: `- |C| "100n" - |gnd|` seats where
  its wire is — the original intent.

```
{ layout: schematic }
|component#U7| "TMC2300-LA-T" [ …pins… ]        // the anchor
U7.VS  - |C#C24| "22u" - |gnd|                   // C24 seats beside VS, gnd below
U7.VS  - |vm|                                    // power flag above the pin
U7.BRA - |R#R18| "470m" - U7.BRB                 // R18 between the pins
U7.DIAG - "NSTDBY"
```

Honest cost: satellite seating is one real placement pass the scope owns — a
step past "thin", but the drawing-annotation pattern reapplied, not
auto-placement; anchors never move.

## Round 10 — the positioning ladder (seating direction & overrides)

- **Seating direction is terminator-driven, never random.** A label symbol
  carries a natural direction — `gnd`/`earth`/`chassis` seat **below**,
  `power` **above**, `nc` at the pin, text labels **outward along the pin
  normal**. A satellite chain reads its terminator: `U7.VS - |C| - |gnd|`
  grows down (cap below the wire, gnd below the cap — the reference sheet's
  own convention, baked); `- |vm|` grows up; part-terminated chains run along
  the pin normal.
- **Pin positioning = the KiCad symbol-editor workflow, existing vocabulary**:
  `side:` picks the side, declaration order is the order along it,
  `translate:` slides a pin along its side for odd cases.
- **The override ladder** — all existing semantics:

  | Layer | Anchors (3+ pins / placed) | Satellites (labels, unplaced 1–2-pin) |
  |---|---|---|
  | auto | grid auto-flow | chain-seats at its pin, direction from terminator |
  | coarse | `cell:` — the imaginary grid | `cell:` **converts to an anchor** |
  | fine | `translate:` nudge from the cell | `translate:` nudge **from the seat** |

- **Translate stays the core law** (SPEC 5): a post-placement nudge from
  wherever the node was placed, never a coordinate. A satellite's translate
  is therefore **pin-relative and robust** — move the chip, the nudge travels
  along. Rejected: translate-only placement from the scene centre (breaks the
  nudge law; centre-relative coordinates are the fragile kind). Auto-flowed
  anchors whose origin must not drift get a pinned `cell:` — same answer as
  every lini grid today.
- **"Satellite" is a placement role, not a type** — any label or unplaced
  1–2-pin part. The parts stay "discretes"; the drawings they wear stay
  "symbols".

## Round 11 — the cluster move: the auto-grid dissolves

- **Seated satellites join their anchor's bbox** (drawing precedent: a
  drawing's bbox is the union of children *and annotations*). Seat satellites
  first, pin-relative; the anchor's cell auto-sizes to the whole cluster
  (chip + caps + gnds + labels); grid tracks size to cells as grids already
  do. Nobody adds a column *for* a satellite — satellites consume **space**,
  not **cells**.
- **Anchors: default one row** — side by side in declaration order (the
  common habit); `columns:` optional to wrap; `cell:` for explicit placement.
- **Cell indices are ordinal, not distance**: `cell: 9 5` creates tracks up
  to 9×5 but **empty tracks collapse entirely** (9 ≈ max+1). Distance-meaning
  would inject invisible whitespace — no silent anything; spacing is the job
  of `gap`, clusters, and clearance. Sparse indices (10, 20, 30…) are safe
  ordering room.
- **Between two placed anchors** (`u1.p1 - |R| - u2.p1`): the chain has no
  outward direction, so its satellites **distribute along the straight line
  between the two pins** (midpoint / even fractions — `along:`
  auto-distribution applied to placement), oriented to the line's dominant
  axis, before routing. Chains: two placed ends → distribute; one → grow by
  terminator; none → flow fallback + warning.
- **Seating override rides `side:`** (the dimension precedent — which side an
  annotation stacks on): label types carry defaults (`gnd`/`earth`/`chassis`
  → `bottom`, `power` → `top`, text → `auto` = outward along the pin normal);
  any instance overrides; the terminator's `side:` steers its whole chain.
  The styling levers: pin position (start), terminator `side:` (end),
  everything else falls in the middle; `translate:` last.

## Round 12 — satellite positioning closure & the terminal-side symmetry

- **Arbitrary satellite positions need nothing new**: `cell:` promotes a
  satellite to an anchor (its own cell, anywhere — including an empty
  region); `translate:` nudges from the seat. Empty columns can't help
  satellites (they consume space, not cells) and don't need to.
- **`:side` never applies to terminals — pins or labels.** Round 2 already
  errors `:side` on a pin endpoint (a terminal owns its connection geometry);
  round 7 made a label its own terminal; so `|gnd|:top` — legal-looking via
  capsule anatomy — is an **error**, not a second spelling of seating
  (`:top` ≈ `side: bottom` inverted would be an alias, and it reads wrong).
  The one seating knob is **`side:` on the label**.
- **What `side: bottom` means is the dimension precedent, verbatim**: "I sit
  below the thing I annotate" (a dim below the geometry; a label below its
  pin) — never the node's own side. The SPEC states it by pointing at dims.

## Round 13 — rotation replaces label `side:` (supersedes rounds 10 & 12 on this point)

- **Conventions bake into authored symbol geometry, not a rules table**: gnd
  is *drawn* with its connection point at the top → unrotated it hangs below;
  power's is at the bottom → it stands above. Trust a correct model.
- **Satellites auto-pose to face their anchor**: the seat pass picks the
  90°-step pose whose connection geometry faces the pin/wire (deterministic
  tie-break) — text labels flip to read outward on either side, a cap in a
  downward chain stands vertical.
- **Explicit `rotate: 0|90|180|270` forces the pose** (the KiCad "R" key);
  seat direction *derives* from the rotated connection point. Four-position
  gnd is legal. Non-90° rotation on a connection-bearing part errors
  (round 8's law).
- **Lowering nuance**: rotating a label re-lays its anatomy — the tag turns,
  the **text stays upright** (never transform-mirrored); ref/value text on
  rotated discretes likewise.
- Label `side:` is dead — orientation was already owned by `rotate:`; a
  second mechanism for one family is what lini kills. `side:` survives only
  where it lived (pins, dimensions, axes); `:side` stays banned on all
  terminals. Every satellite has exactly two knobs, both core: `rotate:`
  (pose) and `translate:` (nudge).

## What's left

The brainstorm is converged. Next steps, per the original plan:

1. **SPEC refactor** — the new Part II schematic section written as if from day
   one; the capsule law lands in the *core* link grammar (SPEC 3/9/21 — the
   first-token wording in 21 needs the round-5 relaxation); ledger (16),
   reserved words (22), errors (20), the symbol registry shared with 15.9.
2. **ROADMAP beta-2 row**.
3. **The round's plan doc**, then implementation.

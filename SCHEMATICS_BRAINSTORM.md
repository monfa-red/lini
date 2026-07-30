# PCB Schematics — brainstorm notes (beta 2 candidate)

Session notes, 2026-07-29/30 (five rounds). **Nothing here is decided** — this is
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

## What's left

The brainstorm is converged. Next steps, per the original plan:

1. **SPEC refactor** — the new Part II schematic section written as if from day
   one; the capsule law lands in the *core* link grammar (SPEC 3/9/21 — the
   first-token wording in 21 needs the round-5 relaxation); ledger (16),
   reserved words (22), errors (20), the symbol registry shared with 15.9.
2. **ROADMAP beta-2 row**.
3. **The round's plan doc**, then implementation.

# PCB Schematics — brainstorm notes (beta 2 candidate)

Session notes, 2026-07-29/30 (two rounds). **Nothing here is decided** — this is
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
| Symbol glyphs drawn at text-relative size | the 15.9 drafting-glyph registry (natural units, not box-fit) |
| Custom power flags, part libraries | defines (`|vm::label| { … } [ "VM" ]` — intrinsic children materialize per instance) |
| Minted ids for anonymous nodes | `lini-topic-N` — the precedent for minted refs |

## Vocabulary

The industry pair is **component** (the part) / **symbol** (its drawing); KiCad
collapses both into "symbol". Lini keeps the full pair: `|component|` is the
instance, `symbol:` picks a drawing — the same meaning it already has on
`|icon|`. "Symbol" always answers *which glyph*.

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

### 3. Discretes: glyph types (round 4 — settled shape)

Glyph nodes (natural-units machinery), generated pins (wires must land on
terminals). Smart label = the value; ref readout from the id / minted; orient
with `rotate:`. **Uppercase short types only** (full words dropped — the type
*is* the ref family; precedent for short types: `|cyl|`, `|hex|`).

**Minting rule**: mint prefix = the type name; `prefix:` (a string property,
read only at minting) overrides — `|ic::component| { prefix: "IC" }` mints
IC1, IC2… An authored id wins outright. Prefixes follow IEEE 315 / ASME
Y14.44; `|component|` (any generic pin-bearing box — IC, module, relay) mints
U. `|J|` ships as a built-in define over `|component|` — prefix J, pins
nameless (numbers only): the connector.

**Variants ride `symbol:`** — the property that answers "which glyph" on
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

Earth / chassis / antenna are **not** discretes — they're `|label|` glyphs
(`symbol: earth | chassis | antenna`), the terminator family with gnd/power/nc.

**Standard: IEC only** — lini's drafting lineage is already ISO (SPEC 15.9).
Never per-type variants (`|R_ANSI|`): a sheet never mixes standards, so ANSI,
if ever, is one scope-level knob swapping the whole glyph family. Deferred.

### 4. Net labels, power, gnd: `|label|` + two statement forms

`|label|` (the name is free) is icon-shaped: smart label = the net text in the
tag outline; `symbol:` swaps in a glyph from the schematic glyph set (`gnd`,
`earth`, `chassis`, `power`, `antenna`, `nc`, …). Text alone = net label; glyph
alone = gnd; glyph + text = power flag. Power nets are defines with intrinsic
text: `|vm::label| { symbol: power } [ "VM" ]`.

**Form 1 — text labels need no new grammar**: the drawing-leader statement
shape, reinterpreted by the scope:

```
U7.DIAG - "NSTDBY"      // one-ended wire + text → a tag, seated at the pin
U7.STEP -> "STEP"       // marker picks the tag shape (output)
```

Marker → shape: `-` plain, `->` output, `-<` input, `-<>` bidirectional, `-*`
the round one (exact table is SPEC-pass detail; sequence-reinterprets-`->`
precedent). Also covers no-connect if desired, or see form 2.

**Form 2 — inline instantiation, proposed as a *core-wide* feature** (round-2
resolution of the `- |gnd|` consistency worry — generalize rather than
special-case; round-3 tightening — **bars only, no tail**): anywhere in lini,
an endpoint position after an op may hold an identity capsule:

```
c24.p2 - |gnd|                   // fresh anonymous gnd at the open end
U7.VS  - |vm|                    // fresh VM power flag (define carries "VM")
U7.VCP - |nc|                    // no-connect ✕ — another terminator glyph
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
- **The first token tells the statement's kind**, so bars at statement *head*
  stay a node declaration — inline identity is legal only after an op, never
  as the source end (`|box| "L" { } -> …` cannot exist). LL(1) holds: after an
  op, `|` opens a capsule, an ident is an endpoint.

Gates and edges: **drawing scopes reject it** ("a drawing never invents an
endpoint" — the existing law); sequence allows it (a typed participant).
Chaining through a capsule works when it has an id (declared, then referenced
by the next hop); anonymous mid-chain — lean allow via minted internal handle,
could gate terminal-only. A fan into a capsule is **one** instance (one
declaration); per-wire grounds are separate statements. Declaring an id twice
is the ordinary duplicate-id error. Cost accepted: label + wire in one line
for a one-off node doesn't exist — two statements, lini's normal price.

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
pin electrical types (input/output pin glyphs), auto part placement. Wire
connectivity is honest lini: connection is a shared endpoint or a fan `&`
(junction dot); crossings are just crossings.

## SPEC & release integration

- SPEC gets a new Part II section (a peer of Sequence/Charts/Drawing), written
  as if from day one — full refactor: templates, ledger (16), grammar (21 — the
  inline-instantiation form, if blessed, lands in the *core* link grammar),
  reserved words (22), errors (20), the glyph registry shared with 15.9.
- ROADMAP needs a **beta 2 row** (currently beta.1 → rc). Syntax isn't frozen
  until v1, so schematic gets first-class naming.

## Settled in round 4

- **Inline instantiation blessed, as a global law**: a routed-link endpoint is
  a bare `id`, a `|type|`, or a `|type#id|`; a capsule always instantiates
  inline and never takes a tail (no label/class/style/children — the tail is
  the link's). Drawing scopes keep rejecting it (no-invention law). A fan into
  a capsule = one instance; its trunk merge is a junction dot.
- **Discretes: uppercase short types only**; the table above, `symbol:`
  variants, type-name minting with `prefix:` override, IEC-only glyphs.
- **Connectors resolved**: `|J|` = built-in define over `|component|`.
- Ids are displayed verbatim and case-sensitive — `|component#U7|` shows "U7"
  and is wired as `U7.VS` (not `u7.VS`).

## Open questions (next session)

1. **Minted refs**: accept display-only minting (never endpoints)?
2. Marker → shape table; how `-<` reads (crow elsewhere).
3. Nested-scope semantics: wires written inside a `|grid|` used for placement
   belong to that grid's scope — do label wires / junction dots reach them?
   Likely cascade, like `routing:`.
4. Junction form: generated `|junction|` chrome child, styleable by rule (the
   `|halo|` pattern — lean) vs a marker on the wire.
5. Glyph set for `|label|` (gnd, earth, chassis, power, antenna, nc, …).
6. Anonymous mid-chain capsules (`a -> |cyl| -> c`) — allow via minted internal
   handle, or terminal-only.
7. Polar pin-id detail: semantic ids (`a`/`k`, `b c e`, `plus minus`) vs
   uniform `p1`/`p2` + aliases.

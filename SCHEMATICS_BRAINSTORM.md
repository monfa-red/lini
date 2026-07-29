# PCB Schematics — brainstorm notes (beta 2 candidate)

Session notes, 2026-07-29. **Nothing here is decided** — this is the state of the
brainstorm, to continue on another machine. Reference schematic: a KiCad A5 page
(TMC2300 stepper driver — `even-Z.pdf`, U7 + passives + connector + net labels +
gnd/power symbols + grouped regions + title block).

## What already exists — reuse, don't build

| Schematic need | Existing lini machinery |
|---|---|
| Sheet, zone borders (1–4 / A–C), centring marks | `|page|` — KiCad's sheet *is* ISO 5457 |
| Title block (Title / Rev / Id / Date / company) | `|title-block|` (ISO 7200) |
| The captioned blue region boxes ("Stepper Driver") | `|group|` + caption; restyle via a scoped rule |
| Wires | the orthogonal router, unchanged |
| Net name on a wire | the link label — `u7.VS - c24.p1 "VM"` works today |
| Colours (dark-red outline, green wire, beige page…) | role variables + theme, the drawing precedent (`--stroke-dark`/`--stroke-light`) |
| Symbol glyphs drawn at text-relative size | the 15.9 drafting-glyph registry (natural units, not box-fit) |
| Custom power flags, part libraries | defines (`|vm::label| { … } [ "VM" ]` — intrinsic children materialize per instance) |

## Decisions leaned into during the session

### 1. Architecture: a thin scope — `layout: schematic`

Chosen over pure sugar and over a full lowering engine. Placement stays
flow/grid-like (parts are placed manually — grid, translate); **the router keeps
the wires** (unlike sequence/drawing, which consume their links). The scope exists
to gate reinterpretations and own the small extras, exactly as drawing reinterprets
`>-` and auto-places callouts:

- legalizes label-terminated wires (below) and marker→label-shape mapping;
- auto-seats a label just off its pin (drawing's `note-offset` precedent);
- draws junction dots at fan (`&`) trunk splits — generated chrome, styleable/removable
  like `|halo|`;
- scoped rules give the schematic look (group caption style, wire colour, …).

Why not pure sugar (the `|table|` model): labels would have no owner — every net
label/gnd hand-placed, no one-ended wires, no junction home. Kills text-to-diagram.
Why not a lowering engine: fights "the orthogonal router should do the wiring",
and manual placement is what schematics-for-showing want anyway.

Open detail: how schematic-ness reaches links written inside a *nested* `|grid|`
of parts — cascade like `routing:`/`clearance:` (scene config), or nearest
layout-owning ancestor. Needs a SPEC answer.

### 2. The symbol type: `|component|` + `|pin|`

`|symbol|` stays icon vocabulary; `|part|` too generic (drawings use the word);
`|component|` is the universal EE term and only appears a few times per sheet
(discretes get their own types).

```
|component#U7| "TMC2300-LA-T" [
  |pin#VS| { number: 18 };  |pin#STEP| { number: 4 }         // default side: left
  |pin#nstdby| "VIO/NSTDBY" { side: right; number: 11 }
]
u7.VS - c24.p1 "VM"
```

- **No authored rails.** Desugar generates per-side rails as *anonymous*
  containers — anonymous containers are already scope-transparent, so `u7.VS`
  resolves with no rail segment in the path. The mechanism exists today.
- **Pin sides, auto with override**: no `side:` → the bilateral split (first
  ⌈n/2⌉ pins left, rest right, declaration order — the mindmap rule reused).
  `side: left|right|top|bottom` overrides. Deterministic, no routing feedback.
- **Pin smart label = the pin name**, defaulting to the id when omitted
  (`|pin#VS|` reads "VS" — the implicit-node labelling rule reused); the label
  form covers names that can't be ids (`"VIO/NSTDBY"`, `"1.8VOUT"`). `number:`
  optional, drawn outside beside the stub.
- **Pin anatomy** (name inside the body, number outside, stub line outward, wire
  lands on the stub tip) is the pin type's lowering; stub length etc. are baked
  schematic constants, like drawing's dim constants.
- **Reference designator**: the id is displayed as the ref (`#U7` shows "U7"),
  mirroring name-from-id. Open: an override/suppress knob (`ref:`? `""`?).
- `|pin|` is lini's first type/property homonym with `pin:` — grammatically safe
  (types live in bars, properties before `:`), same spirit as `scale`/`side`/`unit`.
- Open: connector-style pins that show numbers only (J3) — per-pin `""` labels
  are verbose ×N; maybe a `|connector|` derivative whose pins default nameless.

### 3. Discretes: glyph types

`|resistor|`, `|capacitor|`, `|inductor|`, `|diode|`, `|led|`, `|crystal|` (set
open) — two-terminal glyph nodes drawn in natural units via the drafting-glyph
machinery, each with **generated pins `p1`/`p2`** (`c24.p1` works). Smart label =
the value (`|resistor#R18| "470m"`); ref from the id as above; orient with
`rotate:`. Open: IEC vs ANSI resistor body (pick one default; variant knob or
theme-level).

### 4. Net labels, power, gnd: `|label|` + inline wire termination

`|label|` (the name is free) is icon-shaped: smart label = the net text drawn in
the tag outline; **`symbol:`** — the property icons already wear — swaps in a
glyph from a small schematic glyph set (`gnd`, `earth`, `chassis`, `power`,
`antenna`, `nc`, …). Text alone = net label; glyph alone = gnd; glyph + text =
power flag. Power nets are defines with intrinsic text:

```
{ |vm::label| { symbol: power } [ "VM" ] }
```

**The key proposal — a schematic wire may terminate in an inline node:**

```
c24.p2 - |gnd|          // fresh anonymous gnd, seated at the open end
u7.VS  - |vm|           // fresh VM power flag
u7.DIAG - "NSTDBY"      // sugar for  - |label| "NSTDBY"
u7.VCP - |nc|           // no-connect ✕ — just another terminator glyph
```

Why not bare `- GND` (the first instinct): "a bare name is always a referenced
id" is load-bearing, and a shared id would silently collide five gnd triangles
into one node with five wires. `- |gnd|` costs the same keystrokes, keeps bars =
identity, and each mention mints a fresh instance. Parser stays LL(1): after the
op, `|` opens the inline node, an ident is an endpoint. Precedent for nodes
riding a link: drawing's annotation nodes in a dimension's `[ ]`.

- Desugars to a generated seated `|label|` — visible in `lini desugar`; the
  explicit node form (`|label#step2| "STEP"` + a normal wire) remains for
  stretched, styled, or multi-wire labels.
- **Marker → tag shape** (user's idea, liked): the op's end marker picks the
  net-label shape — `-` plain, `->` output, `-<` input, `-<>` bidirectional,
  `-*` the round one — the scope reinterpreting markers exactly as sequence
  reinterprets `->`. Exact glyph table is SPEC-pass detail.
- Restriction to state in SPEC: inline termination is single-hop, terminal-end
  only (no chains through it; fan `&` behaviour needs a ruling — probably one
  shared label, like fan leaders share one text, or just an error).

Rejected alternatives, for the record: `{ flag: gnd }` per use (un-lini,
verbose); link-property-only labels (no node to stretch or share); auto-create
type-swap in scope (`x - GND` creating a label named GND — shared-id collision
above).

### 5. Look & theme

New role variables (dark/light pairs, tree-shaken like the palette): component
fill (pale yellow), component outline (dark red), wire (green), label (teal),
pin number (muted), page background (beige). Schematic types carry defaults
referencing them, so the classic look is default *inside the scope* and a theme
retunes it. `|schematic|` template = `|block|` + `layout: schematic` (+ maybe the
bg fill).

## Explicitly out of scope (this is for *showing*, not netlisting)

Hierarchical sheets/labels, ERC, netlist export, buses (thick wires — defer),
pin electrical types (input/output arrows on pins), auto part placement,
wire-drag semantics. Wire connectivity is honest lini: connection is a shared
endpoint or a fan `&` (junction dot); crossings are just crossings.

## SPEC & release integration

- SPEC gets a new Part II section (a peer of Sequence/Charts/Drawing), written
  as if from day one — full refactor, not a patch: templates into SPEC 8 or the
  new section, properties into the ledger (16), the inline-termination grammar
  into 21 (drawing-style scoped extension), new type names into 22, errors into
  20, the glyph registry shared with 15.9.
- ROADMAP needs a **beta 2 row** (currently beta.1 → rc): this feature lands
  after beta.1 tooling, before rc. Syntax isn't frozen until v1, so schematic
  gets first-class naming.

## Open questions (next session)

1. Bless or reject **inline wire termination** — the one grammar extension.
   Fallback: labels always explicit nodes (ids + two statements per gnd).
2. Marker→shape table; how `-<` reads (crow elsewhere).
3. Nested-scope semantics (cascade vs nearest-owner).
4. Connector pins (number-only), ref override, IEC/ANSI variants.
5. Junction dot form: generated `|junction|` chrome type vs a marker.
6. Discrete set for beta 2 (R, C, polarized C?, L, D, LED, crystal, …).
7. Glyph set for `|label|` (gnd, earth, chassis, power, antenna, nc, …).

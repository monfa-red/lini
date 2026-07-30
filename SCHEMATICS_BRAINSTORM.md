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
| Net name on a wire | the link label — `u7.VS - c24.p1 "VM"` works today |
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
u7.VS - c24.p1 "VM"
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
  containers — already scope-transparent, so `u7.VS` resolves with no rail in
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

### 3. Discretes: glyph types

Two-terminal glyph nodes (natural-units machinery), generated pins `p1`/`p2`
(`c24.p1` works; wires must land on terminals). Smart label = the value; ref
readout from the id / minted; orient with `rotate:`.

Naming — two candidates, pick one (no aliases):

- **Uppercase short set (lean)**: `|R|` `|C|` `|L|` `|D|` `|LED|` `|Y|`
  (crystal) `|J|` (connector) — the type *is* the ref family exactly as sheets
  write it, so minted refs need no name→prefix table (`|R| "10k"` → R1).
  Precedent for short types: `|cyl|`, `|hex|`. `|component|` stays a word
  (prefix U).
- **Full words**: `|resistor|`, `|capacitor|`, … — more lini-wordy; needs a
  name→prefix table for minting.

Open: IEC vs ANSI resistor body (default + variant knob or theme-level);
polarized capacitor.

### 4. Net labels, power, gnd: `|label|` + two statement forms

`|label|` (the name is free) is icon-shaped: smart label = the net text in the
tag outline; `symbol:` swaps in a glyph from the schematic glyph set (`gnd`,
`earth`, `chassis`, `power`, `antenna`, `nc`, …). Text alone = net label; glyph
alone = gnd; glyph + text = power flag. Power nets are defines with intrinsic
text: `|vm::label| { symbol: power } [ "VM" ]`.

**Form 1 — text labels need no new grammar**: the drawing-leader statement
shape, reinterpreted by the scope:

```
u7.DIAG - "NSTDBY"      // one-ended wire + text → a tag, seated at the pin
u7.STEP -> "STEP"       // marker picks the tag shape (output)
```

Marker → shape: `-` plain, `->` output, `-<` input, `-<>` bidirectional, `-*`
the round one (exact table is SPEC-pass detail; sequence-reinterprets-`->`
precedent). Also covers no-connect if desired, or see form 2.

**Form 2 — inline instantiation, proposed as a *core-wide* feature** (round-2
resolution of the `- |gnd|` consistency worry — generalize rather than
special-case): anywhere in lini, an endpoint position may hold bars:

```
c24.p2 - |gnd|          // fresh anonymous gnd at the open end
u7.VS  - |vm|           // fresh VM power flag
u7.VCP - |nc|           // no-connect ✕ — just another terminator glyph
cat -> |cyl| "DB"       // …and in any diagram: typed declare-and-link
```

Desugar **hoists** the inline node to a declaration in the enclosing scope +
the link — pure sugar, strictly more consistent than today's box-only implicit
auto-create (it's *typed* auto-create). LL(1): after an op, `|` opens a node,
an ident is an endpoint. **Binding law**: the inline form takes bars + an
optional label only; any classes / `{ }` / `[ ]` after it belong to the link —
a styled node uses the declared form.

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

## Open questions (next session)

1. **Bless inline instantiation as a core feature?** (`a -> |cyl| "DB"`
   everywhere; the schematic glyph terminators are just its best customer.)
   Fallback: labels always explicit nodes.
2. **Discrete naming**: uppercase short set (`|R|`, `|C|`…) vs full words.
3. **Minted refs**: accept display-only minting (never endpoints)?
4. Marker → shape table; how `-<` reads (crow elsewhere).
5. Nested-scope semantics: wires written inside a `|grid|` used for placement
   belong to that grid's scope — do label wires / junction dots reach them?
   Likely cascade, like `routing:`.
6. Connector pins show numbers only (J3) — suppress the name readout via a
   `|connector|`/`|J|` whose pins default nameless?
7. Junction form: generated `|junction|` chrome child, styleable by rule (the
   `|halo|` pattern — lean) vs a marker on the wire.
8. Glyph set for `|label|` (gnd, earth, chassis, power, antenna, nc, …).

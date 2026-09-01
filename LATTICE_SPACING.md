# Round: the lattice pays its clearances in fine pitches

The lattice rebuild landed the alignment it promised. What it did not land is
density: the hero's blocks are twice as tall as the circuit in them, and a
power flag stands 390 units of bare wire above the connector pin it flags.

This round is not a change to the model. The two pitches, the cells, the chains
and the tracks all stay. What changes is **what a clearance is measured against
and what it is rounded to**.

## The diagnosis

Measured off `samples/schematic_hero.lini`, `mcu` block, scope frame:

| | today | why | wanted |
|---|---|---|---|
| U3 left pins | −100 … −40 | VDD…PA5 | — |
| first slot row (C6 R7 SW1 C7 R8) | **140** | a whole coarse cell clear of U3's *entire ink* (body + VSS stub + pin numbers ≈ 78) | ~80 |
| flag row (3V3 over VDD **and** over J2) | **−370** | a whole coarse cell clear of U3's ink *top* (ref + value text ≈ −230) | −160 / −100 |
| ground row | 346 | a whole coarse row past the deepest member's centre | ~280 |
| block height | **716** | | **~440** |

Three defects, one theme — *the placement pays every separation in whole coarse
cells, against ink that is not in the way*:

1. **Wrong ink on the ray axis.** `field::origins` computes one line index per
   anchor per side and `Field::line_of` uses it as *both* the lane line (across
   the ray) and the slot line (along it). A chain that turned into a lane is
   clear of the body by construction — that is what the lane *is* — yet its
   slot origin still clears the body a second time, on the axis where nothing
   stood in the way.

2. **Coarse-quantised separations.** A field origin and a rail row round *up to
   a coarse line*, so a four-unit shortfall costs a hundred. Positions want
   coarse quantisation — that is what makes parts share columns. Separations do
   not.

3. **The flag row over-aligns.** `rail::rails` runs one row per ray and maxes it
   over every chain on that ray, so R16's pull-up drags D8's 24 V flag three
   hundred units up. A block has one *ground* net and one line for it is right.
   Power flags are different nets — 24 V, 3V3, 5 V — and no reference sheet
   aligns them.

## The model, after

Three sentences of SPEC 16.1 change.

**A lane and a slot stop being one count read on two axes.** They were never
the same measurement:

- A **lane** is the cross coordinate. It carries a part's whole cell, so lane 1
  is the first **coarse** line whose cell clears the anchor's ink on that side.
  Unchanged, in rule and in number.
- A **slot** is the coordinate along the ray. It is the first **fine** line
  whose cell clears whatever *that chain's own lead actually passes*: a chain
  that turned into a lane passes the deepest pin the chains on that ray leave
  from, and nothing else; a chain that grew straight out passes the anchor's
  ink, the ray pointing through the body. Two classes, one rule — what the lead
  passes — so this is not a second mechanism beside the first.

**Slot origins are the track line's, not the anchor's.** All the anchors on the
track line *perpendicular to the ray* — the same track row for an up or down
ray, the same track column for a left or right one — share one slot origin per
ray per class, the deepest requirement among them. That is what makes two
anchors' fields share their rows, and it replaces the "absolute coarse lines
counted from an anchor that stands on one itself" sentence, which only held
while no anchor took a facing-pin offset.

**There is one rail row and it is the downward ray's.** It stands on the first
fine line clear of every down-chain member's ink, and never shallower than a
rail terminator already reached. A chain terminating upward stands on its own
slot like any other member — which, because the slot origin is now the row's,
lands two single-member up-chains in one row on one line anyway. The flag row
was doing by a rule what the shared origin does by construction.

## The plan

Each task is a commit. Verify visually at 3, 4 and 6 — render, read the PNG,
and measure `data-id` transforms against the table above.

1. **`Slot` assignment moves ahead of the field pass.** `place::arrange` calls
   `slots(...)` before `Field::build` and passes the ordinals in. No behaviour
   change; `slots()` already reads nothing but `children`, `anchored` and
   `columns`. Pure move, tests unchanged.

2. **`Field` splits `origins` into `lanes` and slot origins.** `lanes:
   Vec<[i32; 4]>` keeps today's rule and today's numbers, and serves
   `Field::cross` alone. Slot origins arrive as `Vec<[[i32; 2]; 4]>` — per
   anchor, per ray, per class — in **fine** line indices, still holding today's
   value (the coarse origin, expressed in fine lines) so this commit is a
   refactor with no visual diff. `Seat` gains `turned: bool`, set from
   `Held::turns()` and inherited through `Field::stepped`. `Field::line_of` /
   `ordinal` / `coord` split into a lane pair and a slot pair; the slot pair
   steps by `coarse / pitch` fine lines.

3. **`Field::cells` / `free` answer distances, not cell counts.** They are the
   packer's whole view of a field and they cannot stay in coarse cells once a
   slot origin is a fine line. Both become `f64` distances from the anchor's own
   origin; `pack::extent` drops its `(n + 0.5) * step` for `d + step / 2.0`, and
   `pack::axis`'s `holds` ceils a distance into cells. Behaviour identical —
   the old integers were these distances divided by the step.

4. **The slot origin takes its own measure, shared per track line.**
   `Field::walk` builds its `Held` list, then strikes one origin per (track
   line ⟂ ray, ray, class) before growing anything: `Straight` clears the
   anchor's ink on the ray side, `Laned` clears the deepest `Held::depth` among
   that group's laned chains, both by half a coarse cell, both onto the first
   fine line beyond. **First visual gate** — the mcu block's first slot row
   should be ~80 and its 3V3 flags ~−160.

5. **The rail row is the ground row.** `rail::rails` loses its `Side::Top` half
   and fine-quantises what is left: the first fine line clear of every
   non-terminator down-rider's drawn ink, never shallower than a rail rider's
   own slot.

6. **SPEC 16.1** takes the three sentences above. **Second visual gate** — all
   four samples rendered and read, `--strict` silent on each.

Deferred to its own round, after this one renders: **discrete pin-to-pin
64 → 80.** Ports land at ±32 today and 32 is not a multiple of the fine pitch,
so any wire that turns off a discrete pin turns off-track. 80 is four fine
pitches, 80 + 20 = 100 = `gap`, and two stacked parts' facing pins then stand
exactly one fine pitch apart — the whole model integral in fine units. Twelve
glyphs in `src/glyph/mod.rs`; body stays 32, the lead stubs grow 16 → 24.

## Known residue

An anchor that took a facing-pin offset (`pack::align`) stands off its track
line by that offset, and its satellites ride it, so they miss the shared slot
row by the same amount. The offsets are struck in `pack`, which runs after the
field, so removing the residue means splitting the span scan out of
`Field::build` and aligning first. Today's code has the same residue and no
shared origin at all; this round strictly reduces it. Left alone unless the
hero shows it.

## Rejected

`gap: 100 → 80`, bought by shrinking the standard discrete 64 → 56. Our slot
pitch is already tighter against the body than the reference sheets' (1.56 vs
~2.0), it points the opposite way from the 64 → 80 above, and the pitch is not
what makes the sheet empty — the roundings are.

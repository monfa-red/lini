# `mirror:` reflects the part, not just the profile

Today `mirror:` folds `draw:` subpaths and nothing else, so a half-profile
with a hole in it ships an asymmetric part — silently, `--strict` clean. The
fix is to make `mirror:` mean what the word says: it reflects **everything the
node holds**, and a child that shouldn't reflect says so.

The workaround it replaces is the tell: `translate: -96 0` beside
`pattern: grid(2, 1, 192, 0)` keeps a number in sync by hand — edit the
translate and the part goes silently asymmetric again, one step later.

## The law

- `mirror:` reflects the node's drawn path **and its features**, about the
  axis through the node's own origin. It stops being `|sketch|`-only.
- A feature reflects by the same split the subpaths use, read on **position**
  instead of openness: one **on** the axis reflects onto itself and is drawn
  once; one **off** it becomes a reflected second copy.
- Those copies are `pattern:`'s carrier, reused verbatim — addressed
  `w.p.1` / `w.p.2`, counted into the `N×` prefix. Each mirror item doubles
  the set, reflections following their originals.
- **`mirror: none`** on a child declines, and its subtree with it. `none`
  means *no reflection touches this node* — its own axis and its ancestors'
  alike, so there is no fourth value to invent.
- The default is **`auto`**: reflect iff an ancestor mirrors.
- `|path|` and `|image|` read `none` — a raw `d` has no parse/emit round-trip
  (`src/path_data/mod.rs:20` extracts extent points only) and a raster has no
  reflection at all. An explicit `mirror:` on either errors.
- `revolve:` keeps folding the **profile alone**. It is defined as "exactly a
  fused `mirror:`", so this has to be said or it inherits the new law; a
  turned part's features are drilled, not turned.

## Stage 0 — SPEC.md

The source of truth moves first, and the wording is the deliverable. Seven
edits, all in place, nothing restated twice:

| Where | Edit |
|---|---|
| 15.3 `mirror:` opening (`SPEC.md:2436`) | "reflects the entire drawn path" → "reflects everything the node holds — its drawn path and its features"; the axis table's "through the pen origin" → "through the node's origin" |
| 15.3, after the per-subpath paragraph (`:2446`) | +3 sentences: the position split, `none` / `auto`, the `\|path\|` / `\|image\|` reading. Copy order and addressing **link** to 15.4 rather than restating it |
| 15.3 `revolve:` (`:2460`) | append "— the profile alone; features are not reflected" to "folds exactly as a fused `mirror:`" |
| 15.4 addressability (`:2568`) | the copy-order list gains "mirror copies after their originals, one item at a time" — the ordering law lives here, with the rest of the addressing law |
| 15.6 composition table (`:2742`) | `**\`pattern:\`**` → `**\`pattern:\`** · **\`mirror:\`**` in the count-prefix row |
| Ledger (`:3598`) | owner `\|sketch\|` → any node; values gain `none` · `auto`, `auto` the default |
| Errors (`:4104`, after the bad-item row) | `\| \`mirror:\` on \`\|path\|\` / \`\|image\|\` \| '\|path\|' has no reflection — draw it with the pen \|` |

## Stage 1 — one carrier, no behaviour change

Six sites read `attrs.get("pattern")` to mean *this node is a replication
carrier: it draws nothing itself, its copies are the geometry* —
`drawing/mod.rs:304`, `outline.rs:44`, `halo.rs:198`, `anchors.rs:128`, `:151`,
`:244` — and a seventh (`anchors.rs:430`) re-reads the call for the count.
Mirror copies need every one of those behaviours, so the predicate becomes one
shared notion of replication and the count rides the carrier instead of being
re-derived. This is the load-bearing refactor and the riskiest part; it ships
on its own, snapshot-verified byte-identical.

## Stage 2 — the reflection

**A reflected copy is a copy whose coordinates are reflected — not a node
wearing a flip.** `PlacedNode` gains nothing, the renderer changes nothing,
and no label can come out backwards, because a reflected `|hole|` is just a
`|hole|` somewhere else. The arithmetic is already in the repo:

- position: `reflect_point(p, u)` (`geometry.rs:159`)
- rotation: `2θ − rot` — exact for a shape symmetric about its own axes
- a child's own drawn geometry: `PathSeg::reflect(u)` (`geometry.rs:55`),
  which covers Line, Arc (sweep flips) and Cubic
- `points:`: reflect each pair
- text leaves: position reflects, glyphs stay upright

Recursive into children. The hook is a slot that already exists: children are
laid out and the pen has folded by `layout/mod.rs:496`, and the node's own
`pattern::expand` runs at `:628` — the reflection goes between them, which is
exactly where SPEC puts it ("before `pattern:` and before placement").

On-axis test: `|dot(local_position, n)| < eps` → one copy, no carrier.

## Stage 3 — samples & tests

`samples/drawing_section.lini:36` **is** the reported bug, in the showroom: a
`mirror: y-axis` ring wall whose cross-drilled `|hole#drain|` sits at
`translate: -28.5 0` and today appears in one wall only. It reflects under the
new law and its snapshot is the proof; add a declining feature there rather
than a new sample file. (Its `wall:left (-) wall.drain` reading is unaffected —
only the `(o)` readings take a count prefix, `round.rs:37`.)

Tests: the reported repro, the on-axis single, the `none` opt-out and its
subtree, the `2×` prefix on `wall.drain (o)`, `wall.drain.2` addressing, the
`|path|` error, and `revolve:` leaving features alone.

## Risk

Stage 1 is where this can go wrong — seven call sites whose current meaning is
implicit in an attr name. Stage 2 is arithmetic that already exists and is
already exercised on subpaths. Stage 0 is the one that has to be right first,
because everything else is a reading of it.

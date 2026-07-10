# Lini 1.0 Roadmap

This document records the product and language decisions to take Lini from its
current pre-release state to 1.0. It is the bridge between the complete language
contract in `SPEC.md` and the focused implementation plans that will follow.
It deliberately does not assign code modules, stages, or commits, and it is not
yet normative grammar. Each section is detailed enough to drive a dedicated
SPEC amendment and an implementation plan without reopening the product intent.

The central decision is to preserve Lini's core language. A node remains
`|type#id| "label" .class { style } [ children ]`; `{ }` remains style, `[ ]`
content, and `|…|` identity. Text remains a leaf, links remain endpoint statements,
the cascade remains shared, and every layout continues to lower to the same
primitive scene. The 1.0 work hardens that model, removes a few contextual
collisions while breaking changes are still cheap, and fills the largest gaps in
automatic layout, charts, and production drafting.

---

## 1. Principles that are frozen

### One core language

Flow, grid, tree, sequence, charts, and drawings remain layout engines over one
node/link/cascade model. A new family adds types, properties, roles, and lowering;
it does not introduce a second document language.

### One mechanism per concept

- A visual body is a node.
- A relationship is a link.
- Direct attachment without a drawn relationship uses `||`.
- Repeated items use the common list grammar.
- Paint uses `fill`, `stroke`, `color`, and `opacity`, whether the target is a
  box, link, chart mark, or drafting annotation.
- Layout chooses positions; routing chooses the shape of connections after
  placement. A layout must not silently select a routing style merely because it
  is convenient to implement.

### Reuse properties by meaning, not by count

A property is shared only when its plain-language definition survives every
owner. `fill`, `stroke`, `clearance`, `side`, `unit`, `at`, `fit`, `pattern`, and
`format` are useful shared concepts. A small number of precise properties is a
goal, but an overloaded property with unrelated meanings is worse than two clear
ones.

### Deterministic and inspectable

The same input must continue to produce byte-identical output. Sugar must remain
visible through `lini desugar`; canonical syntax must remain visible through
`lini fmt`; every inferred decision must either be deterministic or expose an
explicit override.

### No silent author mistakes

Lini is a compiler, not a browser. An unknown property, impossible value, or known
property on an owner that cannot use it must never disappear silently.

---

## 2. Language hardening before 1.0

### 2.1 Property validation

Unknown property names become compile errors with a nearest-name suggestion.
Known properties used on the wrong kind of object also become errors with a
contextual correction. Examples include `cell:` outside a grid child, `pin:` on a
link, `points:` on a box, or `tol:` on a non-dimension.

The validation model must know the resolved owner, not only the parsed statement:

- box, text, link, and link-label core owners;
- primitive and template lineage;
- layout-owned roles such as chart series, tree topics, sequence frames, and
  drawing annotations;
- generated types and user defines;
- properties accepted through inheritance versus properties read only on the
  immediate owner.

CSS variables (`--name`) and compile-time bindings remain open namespaces. A
normal dash-case property is closed and validated. Diagnostics use the existing
LSP-compatible source spans and include the valid owners or value shape when that
is useful.

### 2.2 Implicit-node warning

The one-line quickstart remains first-class:

```lini
cat -> dog -> bird
```

A file or scope containing only implicit endpoint nodes emits no warning. Once a
scope contains an explicit node declaration, an undeclared single-id endpoint in
that scope is still created but emits a warning: the mixture is more likely to be
a misspelling than intentional shorthand. Compilation succeeds; this behavior is
not tied to `--strict`.

The warning names the created node and the scope, suggests similarly named
explicit ids, and explains that declaring the node suppresses it. Multi-segment
paths remain never-created errors. Existing shadow warnings for a same-named node
elsewhere in the tree remain.

### 2.3 One list and tuple law

The value grammar is normalized around one visible rule:

- a comma separates repeated items;
- spaces separate the components of one item, tuple, interval, or shorthand.

```lini
data: 20, 35, 10;
data: 10 20, 30 40;                 // two x/y points
fill: auto, --red, auto;
categories: "Q1", "Q2", "Q3";
columns: 80, 140, auto;
labels: "Low", "High", "Max";

translate: 10 -4;                   // one point
range: 0 100;                       // one interval
padding: 4 8;                       // one shorthand
cell: 2 1;                          // one column/row tuple
```

This applies to all genuine lists: scalar and point data, categories, labels,
ticks, explicit link-label positions, track lists, per-column alignment, mirror
axes, segmented formulas, paint lists, thread groups, break groups, and any new
repeated value. Functions retain commas between arguments because their arguments
are repeated items; a point argument remains `x y` where a property expects a
point.

Fixed-arity values remain space-separated: padding, translate, range, cell, span,
shadow anatomy, stack offsets, and similar compounds. `fmt` is the canonical
migration tool. Old ambiguous scalar lists should produce a targeted error rather
than being guessed from context.

### 2.4 Text wrapping and maximum width

Add `max-width` and `text-wrap` to the shared box/text vocabulary:

```lini
|box| "A long generated label that should remain compact" {
  max-width: 180;
  text-wrap: wrap;
}
```

`text-wrap` accepts `wrap` and `nowrap`; `wrap` is the default. Without a finite
`max-width`, wrapping has no effect beyond authored `\n` breaks. On a box,
`max-width` is a border-box limit and the available line width is reduced by its
horizontal padding and stroke. On a bare text leaf it limits the text directly.

Wrapping prefers whitespace boundaries. A single token wider than the available
line may break at a Unicode grapheme boundary so Lini's no-clipping/no-spill law
continues to hold. `text-wrap: nowrap` with content that cannot fit a finite
`max-width` is a compile error rather than silent overflow. A non-text child whose
minimum width exceeds its container's `max-width` likewise produces a useful
layout error. An explicit `width` remains a floor; `width > max-width` is invalid.

Wrapped line measurement participates in auto sizing, grid tracks, tree subtree
measurement, chart gutters, sequence spacing, labels, and routing obstacles.

### 2.5 Human drawing scale

Authored drawing scale becomes a human view ratio, not renderer density:

```lini
|page| { sheet: a4 landscape } [
  |drawing#main| "SHAFT" { scale: 1 } [ … ]
  |drawing#detail| { of: c; scale: 4 } [ … ]
]
```

`scale: 1` means 1:1 on the sheet, `scale: 2` means 2:1, and `scale: 0.5`
means 1:2. The page's SVG pixels-per-millimetre density becomes an internal
rendering concern and is no longer something an author multiplies into a view
scale. Root drawings outside a page use the same internal display density while
retaining `scale: 1` author semantics.

Geometry and drawing-unit positions follow the view ratio. Text, strokes,
markers, hatches, dimension anatomy, leaders, title gaps, tables, balloons, and
other sheet chrome remain sheet-space. A sourced section/detail title composes
its printed ratio directly from this value. Export or raster resolution belongs
to output tooling, not the drawing language.

### 2.6 Sequence note placement

Replace the mutually exclusive `over`, `left`, and `right` properties with one
placement property:

```lini
|note| "cached" { place: over api }
|note| "shared state" { place: over api db }
|note| "external" { place: left api }
|note| "result" { place: right db }
```

The mode and its participant ids form one placement tuple; this is not a list of
independent placements. There remains one `place:` property and exactly one
placement mode per note. The old properties are removed rather than retained as
aliases.

### 2.7 Canonical chart labels

Rename chart-series `tags:` to `labels:`. The head label remains the singular
series/legend label; `labels:` is the parallel list of per-datum labels. The same
property name is reserved for future explicit axis tick labels because both mean
"the labels parallel to this owner's items."

### 2.8 Shared number and date formatting

Add a `format:` property whose only role is presentation of an already-resolved
value. It never changes domains, measurement, geometry, tolerances, or data.
It applies to chart axes/tooltips and drawing dimensions, inheriting from the
containing chart or drawing and allowing an axis, series, dimension rule, or
individual dimension to override it.

The initial format family should cover the production cases without becoming a
general string-template language:

- `auto` — current compact deterministic formatting;
- `decimal N` — exactly or at most `N` decimal places, settled in SPEC;
- `significant N` — `N` significant digits;
- `scientific N` and `engineering N`;
- `percent N` — display fraction-to-percent with `N` decimals;
- `fraction D` — nearest fraction with maximum denominator `D`;
- date/time presets for time axes, with an explicit quoted pattern only if the
  preset set proves insufficient.

`unit:` remains a separate suffix/measurement-unit concept. Formatting composes
before `unit`, tolerance, diameter/radius glyphs, and pattern counts.

---

## 3. Tree layout and mindmaps

### 3.1 The `tree` engine

Add `layout: tree` with `direction: row | column | radial`. A tree is a rooted
parent/child structure: each branch topic has exactly one structural parent and
source order determines sibling order. Arbitrary multi-parent and cyclic graph
layout is not part of this engine.

`column` places generations top-to-bottom, `row` left-to-right, and `radial`
places the root at the centre with generations expanding outward. Row/column can
be implemented through recursive subtree measurement and the existing flow
placement primitives, but tree is a real layout owner: it understands structural
children, centres a parent over its child-subtree span, creates branch links, and
handles variable node sizes. Radial uses the same subtree weights to allocate
deterministic angular sectors.

`gap` retains its shared spacing meaning. For column trees its row component is
the generation distance and column component the minimum sibling-subtree gap;
row direction transposes those axes. In radial trees the components become radial
generation distance and minimum tangential separation. The engine may expand an
angular sector to satisfy the minimum; it never overlaps topics to preserve an
exact angle.

Explicit width/height remain floors on the tree owner. Auto size is the union of
topics, branch links, labels, and padding. Source order, then the standard side
rank, resolves symmetric ties.

### 3.2 `|topic|` separates structure from visual content

Add `|topic|` as a tree-layout role over `|block|`. Its smart label is its visible
primary text. A direct child derived from `|topic|` is a structural branch; every
other direct child is ordinary visual content inside the topic.

```lini
|topic#syntax| "Syntax" [
  |icon| "code"                 // visual content
  |badge| "core"                // visual content
  |topic| "Nodes"               // branch
  |topic| "Links"               // branch
]
```

Custom structural nodes derive from the role:

```lini
{
  |person::topic| { fill: --blue-wash; stroke: --blue-ink }
  |folder::topic| { fill: --amber-wash; stroke: --amber-ink }
}
```

This permits rich topics without confusing an embedded badge, icon, table, or
chart for another branch. A `|topic|` outside `layout: tree` is an error, matching
the chart/sequence role model. A tree owner with no structural topic child is an
empty-tree error; the root stylesheet may hold a forest only if the SPEC round
explicitly chooses to support it, otherwise it requires one root topic.

### 3.3 Generated branch links

The tree engine generates one ordinary unmarked link from each parent topic to
each direct topic child. Generated links wear `|-|`, inherit routing and
clearance, and participate in the normal link cascade, markers, labels (if future
branch labels are added), rendering, and diagnostics. `|mindmap|` defaults these
links to unmarked natural strokes; users may style or add markers through the
ordinary link rule.

Explicit authored links remain allowed for cross-connections and do not replace
the structural branch link. Endpoints and scope use the existing id/path model.
The tree layout places only from structural nesting; cross-links do not change
the tree or create multiple parents.

### 3.4 `|mindmap|`

Add `|mindmap|` as the common preset: a visible root topic with `layout: tree`,
`direction: radial`, and `routing: natural`.

```lini
|mindmap#lini| "Lini" [
  |topic#syntax| "Syntax" [
    |topic| "Nodes"
    |topic| "Links"
  ]
  |topic#layouts| "Layouts" [
    |topic| "Charts"
    |topic| "Sequences"
    |topic| "Drawings"
  ]
]
```

Like every template, its paint, padding, type rules, direction, routing, and gap
remain overridable. There is no second mindmap grammar.

---

## 4. Routing strategies

### 4.1 The public strategy set

The 1.0 routing values are:

| Strategy | Contract |
|---|---|
| `orthogonal` | Obstacle-aware horizontal/vertical polylines under `ROUTING.md`; the global default. |
| `natural` | Obstacle-aware smooth curves with deterministic side choice, clearance, and crossings; the mindmap default. |
| `straight` | One body-trimmed direct segment; it deliberately avoids nothing. |

The deferred name `curved` is replaced by `natural`; it is not retained as an
alias. `natural` describes the intended organic result rather than merely the
presence of a Bézier command.

### 4.2 Layout and routing stay independent

Flow, grid, tree, and any future placed-node layout accept all three strategies
when links are routed after placement. A future automatic DAG layout must not
arbitrarily reject orthogonal routing if the ordinary router can consume its
placed scene.

Sequence and drawing remain layout-owned wiring systems: sequence messages are
time rows, and drawing links become mates/annotations. They do not expose the
normal routing choice for those consumed links. Charts and pies have no routed
links. Nested ordinary flow/grid/tree scopes inside those layouts retain their own
routing strategy where their links are not consumed by the outer engine.

### 4.3 Natural-routing behavior

Natural routing shares the existing routing spine: request expansion, endpoint
resolution, forced sides, marker placement, label placement, bundles, fans,
self-links, reports, strays, and deterministic ordering. It must satisfy the same
observable clearance and contact principles where geometrically applicable:

- endpoints land on body sides, with a tangent normal to the side;
- curves stay outside inflated node keep-outs;
- unrelated curves keep the configured clearance except at explicit crossings;
- crossings are locally clean and deterministic;
- duplicate links remain visibly separate;
- fan siblings may share a trunk before a deterministic split;
- labels ride and may slide along the final curve without moving it;
- an impossible route is reported and visibly rendered as a stray, never passed
  off as a legal curve.

The natural strategy chooses a legal topological path through free space, then
fits a smooth curve inside that corridor. It must not simply round an illegal
straight segment through obstacles. Endpoint tangent length, smoothing, and
curvature are engine decisions in 1.0; no `tension`, `bend`, or `curvature`
property is exposed until real diagrams demonstrate a need.

Straight routing remains the honest escape hatch for layouts that already leave
a clear direct segment or for authors who accept intersections.

---

## 5. Chart work for 1.0

### 5.1 Per-datum paint through the common list grammar

A scalar paint continues to apply to the complete series. A comma-list paint on
a repeated-mark series applies item-for-item:

```lini
|bars| "Revenue" {
  data: 20, 35, 18, 42;
  fill: auto, auto, --red, auto;
  stroke: auto, auto, --red-ink, auto;
  opacity: 1, 1, 0.8, 1;
}
```

`auto` means the series' normal palette-derived paint for that item. A paint list
must contain exactly one item per authored datum; mismatches are errors. Initial
support covers `fill`, `stroke`, and `opacity` on `|bars|` and `|dots|` because
each datum lowers to a distinct mark.

`|slice|` and `|bubble|` already expose one node per datum and need no list form.
A `|line|` and `|area|` are continuous shapes, so a list paint is rejected there
for 1.0; per-marker or per-segment line paint remains deferred rather than being
given ambiguous interpolation rules.

### 5.2 Per-datum labels

```lini
|line| "Model" {
  data: 35 63, 42 72, 84 75;
  labels: "Base", "High", "Max";
  marker: circle;
}
```

The count must match explicit data. Labels on a sampled `fn:` remain invalid
because there are no authored data items to pair with. Existing tooltip behavior
continues to decide inline versus hover presentation.

### 5.3 Time axes

Extend an axis scale with a time mapping for real time-series data. The expected
surface is an axis declaration such as `scale: time`; the exact accepted date
literal representation is settled in the SPEC amendment, with ISO-8601 quoted
values as the preferred direction:

```lini
|chart| "Orders" [
  |axis#time| "Date" { side: bottom; scale: time }
  |axis#count| "Orders" { side: left }
  |line| "orders" {
    axis: count;
    data: "2026-01-01" 18, "2026-02-01" 24, "2026-03-01" 31;
  }
]
```

Time axes parse dates/timestamps to an ordered numeric domain, crop through
`range`, and generate calendar-aware ticks rather than treating seconds as a
generic linear number. Bare dates are date-only values; timestamps with offsets
retain their absolute instant. The renderer remains timezone-independent unless
the source explicitly carries a timezone. Invalid or mixed date/numeric domains
are errors.

`format:` controls tick presentation. Auto formatting selects an appropriate
year/month/day/time precision from the visible domain and tick interval. This
feature does not introduce external data loading.

### 5.4 Chart scope kept deliberately small

External CSV/JSON sources, gauges, and new chart families are not 1.0 requirements.
The release work focuses on making the existing chart family complete and
consistent: labels, time domains, formatting, and mark-level paint.

---

## 6. Engineering drawing work for 1.0

### 6.1 Dimension clearance replaces dimension `gap`

Remove `gap:` from dimensions. `gap` remains on mates, where it is an actual
signed separation between parts. Dimensions use the existing `clearance` concept:

```lini
{
  layout: drawing;
  clearance: 18;
  (-) { clearance: 24 }
}

a:left (-) b:right { clearance: 32 }
```

The cascade is drawing default, dimension-family `(-)` rule, any descendant/class
rule, and the individual dimension block. The drawing template desugars the
drafting default into the same property slot.

For a dimension, clearance is a minimum empty distance from geometry and other
annotations, not an exact coordinate. The packer may move it farther outward to
clear rows, text, leaders, or feature-control frames. A per-dimension value is
honored independently; unlike routed-link global capacity decisions, it does not
force every dimension to the maximum value. `translate` is the exact final nudge.

Fixed `dim-offset`/`dim-pitch` implementation constants should be replaced or
derived from painted annotation bounds plus clearance so different font sizes,
tolerances, and stacked annotations remain correct without special cases.

### 6.2 Smart linear-dimension inference

`(-)` infers its measurement axis from anchor kinds:

1. Two point anchors measure the true aligned distance between them.
2. A point and a directed side/edge measure along the directed anchor's normal.
3. Two parallel directed sides/edges measure along their shared normal.
4. Two non-parallel directed anchors error and suggest `(<)` when an angular
   measurement is likely.

Thus points `(0, 0)` and `(30, 40)` read `50` by default. A plate side to a hole
centre remains horizontal or vertical because the side supplies the projection.
Existing dimensions between opposing bbox sides retain their current readings.

Add an explicit override:

```lini
a (-) b { project: horizontal }
a (-) b { project: vertical }
a (-) b { project: aligned }
```

`project:` accepts `horizontal`, `vertical`, and `aligned`; omission means infer.
It changes what distance is measured and the dimension-line direction, not where
the row is placed. Aligned dimensions default to the side of their span facing
away from the drawing's geometry centre. The SPEC round must define a concise
left/right override relative to endpoint order, while `translate` remains valid
for exceptional placement.

### 6.3 Boxed datum feature labels

Keep the existing datum leader syntax:

```lini
body:seat >- "A"
```

The datum triangle remains on the feature. Its letter lowers into the standard
small rectangular datum frame at the landing rather than bare text. Font, stroke,
fill, and line weight inherit from the link/drawing cascade; the generated box is
sheet-space and participates in annotation obstacles and extents. Styled text
and an explicit label continue to work through the ordinary link content model.

### 6.4 Surface-finish annotations

Add `|surface-finish|` as a sheet-space annotation node. Its smart label is its
textual indication and `symbol:` selects the graphical form:

```lini
|surface-finish#sf| "Ra 0.8" { symbol: machined }
```

The initial symbol set is `basic`, `machined`, and `prohibited`, drawn from one
internal drafting-symbol path registry. The registry is shared with feature-control
symbols so line weight, scale, theming, and SVG lowering have one implementation.
The node may later grow the full ISO surface-texture parameter model without
changing its attachment syntax.

Direct placement reuses `||`:

```lini
sf || body:seat
```

Generalize `||` from "mate two geometry parts" to "seat two anchors without
drawing." The cases are deterministic:

- geometry to geometry retains today's mate/ground behavior;
- sheet annotation (or an annotation bundle) to geometry always moves the
  annotation, never the part;
- geometry mates resolve first, then annotation seating sees final part positions;
- annotation seating does not enter the geometry grounding graph;
- the target must supply a directed side/edge when the annotation needs surface
  orientation; a point-only target errors;
- the annotation has a type-defined default seat anchor, with an explicit anchor
  such as `sf:bottom` available when needed;
- authored `rotate` establishes or overrides orientation before seating, and
  `translate` applies afterward.

The leader form is an ordinary two-ended drawing annotation:

```lini
body:seat <- sf:bottom
```

The arrow lands on the feature and the surface-finish node sits at the other end.
Both direct and leader forms render the same `|surface-finish|` node; there is no
parallel string-only surface-finish implementation.

Surface finish and geometrical feature control remain distinct annotations, but
ordinary layout may bundle them visually:

```lini
|column#spec| [
  |surface-finish| "Ra 0.8" { symbol: machined }
  |feature-control| [ … ]
]

spec || body:seat
```

`|row|`, `|column|`, and other sheet annotation bundles carry `scale: 1` and move
as one when seated or connected.

### 6.5 Full feature-control frames

Use the readable type `|feature-control|`; no built-in acronym alias is added.
Projects that prefer it can define `|fcf::feature-control| { }` themselves.

A feature-control frame is a table-like stack of semantic `|control|` rows:

```lini
|feature-control| [
  |control| "position" {
    tol: 0.10;
    zone: diameter;
    material: maximum;
    datums: A maximum, B, C;
  }
  |control| "position" {
    tol: 0.05;
    zone: diameter;
    datums: A, B;
  }
]
```

The `|control|` smart label selects the characteristic symbol. The initial
registry must cover the common form, orientation, location, profile, and run-out
families, including straightness, flatness, circularity, cylindricity, line/surface
profile, parallelism, perpendicularity, angularity, position, concentricity or
the chosen current-standard equivalent, symmetry where supported, circular
run-out, and total run-out. The standards review performed during the SPEC round
decides the canonical current names and excludes obsolete symbols rather than
silently approximating them.

Each row lowers to framed semantic compartments:

- characteristic glyph from the label;
- tolerance-zone shape from `zone`;
- numeric/string tolerance through the existing `tol` and `format` machinery;
- feature material condition from `material`;
- one compartment per comma-separated `datums` item, with an optional modifier
  in that item's space-separated tuple;
- ordered extra `modifiers` for projected zones, common zones, tangent planes,
  free state, all-around/all-over, and other supported indications.

Unknown combinations error; the engine must not render a plausible-looking but
semantically invalid frame. One `|control|` is one row. Multiple rows in one
`|feature-control|` produce a composite/stacked frame with shared boundaries.
Multiple sibling `|feature-control|` nodes remain separate frames and stack by
ordinary layout.

#### Annotation nodes inside drawing links

Drawing annotation content expands from text-only to text plus annotation nodes.
Core routed links outside a drawing remain text-label-only for 1.0.

```lini
hole (o) [
  |feature-control| [
    |control| "position" {
      tol: 0.10;
      zone: diameter;
      datums: A, B, C;
    }
  ]
]
```

Because there is no authored string, the dimension keeps its auto-measured
`⌀…` text and stacks the frame beneath it. An authored string retains the current
replace/follow semantics, then node annotations follow it. Annotation nodes are
laid out in source order as a compact column at the dimension text seat and count
as obstacles for row packing.

The same model works at a leader landing:

```lini
body:seat <- [
  |feature-control| [
    |control| "flatness" { tol: 0.02 }
  ]
]
```

Multiple node children stack without a new combination property. Styling,
translation, rotation, hints, and classes stay ordinary node behavior. This
annotation-content seam is the foundation for future welding, inspection, and
other structured callouts without adding a new statement grammar for each.

### 6.6 Internal threads in authored sections

Extend existing `thread:` dressing to correctly recognize segments on inner
even-odd subpaths. The pen already knows subpath geometry and nesting; an inner
surface reverses the material side, so the major/minor convention and thin-line
offset reverse relative to an external thread.

The existing syntax remains:

```lini
|sketch#section| {
  draw: …;
  revolve: x-axis;
  thread: bore-thread 1.5;
}
```

Validation still requires an appropriate straight segment parallel to the
revolve axis. Thread callouts compose the correct internal/external specification
from the same geometry and pitch. No second `internal-thread` property or type is
introduced.

### 6.7 Addressable pattern copies

Materialize deterministic 1-based copy ids under a patterned node:

```lini
plate.bolt.1 (-) plate.bolt.2
plate.bolt.2 <- "DEBURR"
```

This narrowly extends an endpoint path segment to accept a positive integer after
`.` when the preceding node is a pattern carrier; ordinary authored ids retain
their existing identifier grammar.

Grid copies number row-major from the seed copy; `.1` is the seed. Radial copies
number from bearing 0 clockwise. The numbering follows authored pattern order and
does not change with rendering or routing. A pattern carrier endpoint without a
copy suffix retains its current seed/ring-centre meaning.

Copies are addressable only through their carrier path, preventing synthetic ids
from leaking into the parent scope. Dimensions measure true model positions,
including across breaks and scale; leaders land on the selected rendered copy.
Pattern count prefixes remain automatic when the carrier rather than one copy is
annotated.

### 6.8 Fan leaders

Allow the existing endpoint fan syntax on one-ended drafting leaders:

```lini
a & b <- "2× R5"
```

One note/text stack and landing are placed using the first endpoint's natural
outward direction unless `side` overrides it. A leader leg ray-casts independently
to every endpoint; compatible legs share the landing and as much trunk as the
geometry permits. The label is authored once and is not duplicated. Classes,
stroke, marker override, and annotation node content apply to the whole fan.

Fan order is source order, endpoints must belong to the drawing scope, and an
unroutable leg is reported rather than silently omitted.

### 6.9 Annotation crossing halos

Dimension, extension, and leader linework must remain readable when it crosses
geometry. Add generated sheet-space halo/knockout anatomy beneath annotation
strokes at crossings rather than moving dimensions or editing source geometry.

The halo is emitted once through a generated styling hook/type, uses the local
sheet background role by default, and is wider than the visible line by a fixed
sheet-space margin. It does not cover arrowheads, text, feature-control frames,
or the target contact region. Crossings between annotations preserve their
source/layer order rather than erasing each other indiscriminately. The cascade
must provide one way to restyle or remove generated halos without a family of
per-dimension knobs.

The implementation must inspect hatching, filled regions, and dark mode during
the SPEC/visual round; a mask-based knockout is preferred where a background
understroke would produce the wrong local surface.

### 6.10 Projection and auxiliary-view assistance

Production drafting needs projection support, but Lini remains a 2D authored
drawing system. It must not claim to derive a complete orthographic view from one
2D profile without a 3D model.

The 1.0 goal is therefore assistance between authored views:

- align related view datums through the existing `align: origin` mechanism;
- declare correspondences between named features/anchors in sibling drawings;
- generate thin, removable projection/construction lines between corresponding
  stations;
- allow an authored auxiliary view to declare the source edge/plane whose normal
  establishes its orientation;
- retain independent authored geometry and dimensions in every view;
- keep projection lines as generated chrome so the cascade can style/remove them;
- preserve true measurement, view scale, and sheet-space line weight.

The exact declaration syntax is intentionally left for its dedicated SPEC
brainstorm because it must compose with `of:`, cross-view scopes, sections,
details, and future repeated-copy anchors. The implementation plan must begin
from concrete orthographic and inclined-feature samples and must reject any
syntax that implies geometry can be inferred when it cannot. Automatic 3D
projection is not a 1.0 promise.

---

## 7. Images and title blocks

### 7.1 Self-contained local images

Extend `|image|` beyond external URLs so diagrams and title blocks can embed local
assets while remaining one self-contained SVG:

```lini
|image| {
  src: "./company-logo.svg";
  width: 32;
  height: 12;
}
```

Relative paths resolve from the source `.lini` file, not the process working
directory. SVG remains vector when embedded; raster formats are encoded as data
URIs. Existing HTTP(S) sources and authored data URIs continue to work. Missing,
unsupported, or unreadable paths produce source-spanned errors. The directory
server applies its existing traversal boundary to image loads and rejects assets
that escape its served root; normal file compilation follows ordinary filesystem
permissions relative to the source file.

Embedding is deterministic from asset bytes. It must not make network requests
during compilation unless the source explicitly remains an external URL. Both
live and `--bake-vars` output remain self-contained for local assets.

### 7.2 Title-block smart label and field names

The smart label becomes the document-title field:

```lini
|title-block| "Socket cap screw" {
  drawing-number: "DIN 912 — M8 × 40";
  revision: "A";
  sheet-number: "1/1";
  date: "2026-07-08";
  author: "AM";
}
```

It lowers directly to the same generated spanning field cell that `title:` builds
today: a `|cell|` spanning the field grid, containing `|field| "Title"` and the
label as its value text. It does not lower to a direct table string or to an
intermediate hidden `title:` property.

Rename abbreviated/colliding field properties to clear dash-case names:

- `dwg` → `drawing-number`;
- `rev` → `revision`;
- title-block `sheet` → `sheet-number`;
- retain already-clear names such as `date`, `author`, `approved`, `department`,
  `reference`, `document-type`, and `status`, expanding remaining abbreviations
  consistently during the SPEC amendment.

A label or any field property selects structured-field mode. A title block with
no label and no field property retains the fully custom table form.

### 7.3 Authored children remain table cells

`|title-block|` remains a real `|table|`. In structured-field mode, generated
field cells are created first and authored children follow as ordinary cells.
They may use `cell` and `span`; auto-flow avoids explicit placements, and overlap
is an error.

```lini
|title-block| "Pump housing" {
  drawing-number: "PH-104";
  revision: "A";
} [
  |cell| "Material: 6061-T6" { span: 2 1 }
  |cell| "Finish: Anodized"
]
```

There is no `logo:` property, generated logo, or reserved logo cell. A custom
corporate layout uses the plain-table form and may place an `|image|` or any
Lini-drawn node in a cell:

```lini
|title-block| { columns: 36, 80, 28 } [
  |image| {
    src: "./logo.svg";
    width: 30;
    height: 12;
    span: 1 2;
  }
  |cell| "Pump housing" { span: 2 1 }
  |cell| "DWG PH-104"
  |cell| "REV A"
]
```

A logo may instead sit anywhere else on the page. Title-block composition is
general table behavior, not branding-specific language.

A bill of materials remains an ordinary `|table|`. Automatic BOM generation is
not required for 1.0.

---

## 8. AI and tooling readiness

The grammar is already compact and regular enough for generated use. The 1.0 work
must make the compiler's knowledge available to tools instead of relying on an AI
to absorb the entire narrative SPEC.

### Machine-readable schema

Expose one generated schema describing:

- built-in primitives, templates, and layout roles;
- inheritance chains and valid child roles;
- every property, value shape, default, inheritance behavior, and valid owners;
- list-versus-tuple arity;
- layout/routing compatibility;
- required and mutually exclusive properties;
- deferred versus supported values;
- short canonical examples.

The schema must be generated from or share the same authoritative ledger used by
validation so documentation and compiler behavior cannot drift.

### Structured diagnostics

Add a structured output mode for checking/compilation with stable diagnostic
codes, severity, source span, message, related span, suggestions, and machine-
applicable replacement where safe. Human LSP-style output remains the default.

### Compact generated reference

Generate a concise AI/tool reference from the same ledger: grammar, mental model,
property tables, and one sample per feature, without implementation prose. It
supplements rather than replaces `SPEC.md`.

### Canonical output

`fmt` adopts every 1.0 syntax decision, especially comma lists and structured
annotation formatting. Every sample remains formatter-idempotent, and error
messages show canonical corrected syntax.

---

## 9. 1.0 quality bar

Every feature entering 1.0 must satisfy the repository's existing gates and these
release-level requirements:

- complete SPEC amendment before implementation;
- one focused implementation plan in the repository root;
- one canonical sample per feature or coherent feature cluster;
- `insta` snapshots for every output shape and diagnostic family;
- formatter, parser, resolve, layout, desugar, render, and determinism coverage as
  applicable;
- routed output validated against the routing laws;
- SVG rendered to PNG with `resvg` and visually inspected, including light/dark
  where paint is involved;
- drawing features inspected at multiple view scales and on a physical page;
- no new silent property/value behavior;
- no duplicate lowering paths for the same label, symbol, annotation, or generated
  chrome;
- `cargo fmt`, `cargo test`, and `cargo clippy` clean before release commits.

Breaking migrations are acceptable before 1.0 and should be performed once,
coherently: update SPEC, implementation, formatter, all samples, README, snapshots,
and diagnostics together. Removed spellings are not retained as permanent aliases.

---

## 10. Deferred until after 1.0

These ideas remain valid directions but are deliberately outside the 1.0 release
contract. They should not distort the features above or reserve premature syntax.

### Automatic general graph/DAG layout

A DAG is a directed acyclic graph in which nodes may have multiple parents; a
general graph may also contain cycles. This is different from the parent/child
tree model. A future placed-node graph engine may use layered/Sugiyama-style
placement and should accept orthogonal, natural, or straight routing wherever
possible. No `layout: auto` catch-all is planned.

### Sequence extensions

Parallel (`par`) and referenced (`ref`) fragments, create/destroy lifelines,
found/lost messages, participant grouping, explicit activation spans, delays,
dividers, message numbering, and the remaining UML interaction fragments stay
deferred. Current calls, returns, async messages, activations, `loop`, `opt`,
`alt`, `else`, and notes define the 1.0 sequence scope.

### Additional chart families and data sources

External CSV/JSON sources, gauges, stacked areas, multi-ring pie/sunburst,
per-segment line/area paint, exploded slices, and richer polar controls remain
post-1.0 candidates. Inline data/formulas, existing chart families, time axes,
formatting, labels, and repeated-mark paint are the 1.0 target.

### Drawing feature variants

Slots, blind holes, counterbores, countersinks, repeated-segment counting,
baseline/ordinate dimension systems, automatic BOMs, exploded mates, deeper
sourced-view nesting, and full automatic orthographic/3D projection remain
post-1.0. Projection assistance between authored views is in scope; inferred 3D
geometry is not.

### Imports, modules, and type namespaces

Shared files and namespaces may eventually support corporate themes, component
libraries, and engineering part libraries while preventing imported type-name
collisions. The single-file language and local defines are sufficient for 1.0;
no speculative import grammar is reserved here.

### Remaining core/rendering ideas

Animation, native PNG/WebP export, optional wide-gamut OKLCH output, embedded font
metrics, numeric font weights, gradient text, arbitrary non-rect radius, richer
accessibility metadata, and other items already named in `SPEC.md`/`TODO.md`
remain independently deferred unless promoted by a later reviewed plan.

---

## 11. From roadmap to work

This roadmap should be consumed in focused rounds rather than one monolithic
release branch. Each round begins by converting its section into normative SPEC
language and explicit examples, resolving any "SPEC round" choices named above,
then writing a bounded coding plan. Likely coherent rounds are:

1. language hardening and migration;
2. tree topics, mindmaps, and natural routing;
3. chart lists, labels, formatting, and time axes;
4. drawing measurement/clearance and annotation composition;
5. drafting symbols, feature control, and attachment seating;
6. images, title blocks, projection assistance, and release tooling.

The order is guidance, not an implementation plan. A round is complete only when
its contract, code, samples, snapshots, visual review, documentation, and migration
all agree.

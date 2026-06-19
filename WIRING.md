# Wiring — The Routing Contract

Lini routes wires **orthogonally**: horizontal and vertical runs through the free
space between nodes, corners rounded, like traces on a well-routed board. This
document is the source of truth for routing. It has two halves: the **laws**,
judgeable on the rendered output alone, and the **model**, the one way routes are
produced — so the same diagram can never route two different ways.

`SPEC.md` §10 owns wire *syntax*; this document owns wire *geometry*.

---

## Vocabulary

- **Node** — anything a wire avoids: a box, an oval, a cylinder, **a text label**.
  A node is its axis-aligned bounding box, with four **sides** (top, right,
  bottom, left). A **group** that does not contain a wire's endpoint is solid —
  the wire goes around it, never between its children. A group containing an
  endpoint is transparent to that wire; its *other* children stay solid.
- **clearance** — one number, default **16**, set once for the diagram
  (`-> { clearance: }`): the minimum gap between a wire and every node body, and
  between a wire and every other wire.
- **Keep-out** — a node's bbox inflated by `clearance` on all sides. Wire bodies
  stay outside every keep-out; only a wire's own **stub** enters its own
  endpoint's keep-out.
- **Corridor** — a maximal free rectangle between keep-outs (and the canvas
  margin). All wire travel happens in corridors.
- **Lane** — one of a corridor's discrete running positions: parallel to the
  corridor's axis, exactly `clearance` apart, the occupied set centred on the
  corridor's width. A corridor of free width `W` holds `floor(W / clearance) + 1`
  lanes. Where two same-axis corridors abut across free space, each pulls its
  lanes half a `clearance` off the shared boundary, so runs on opposite sides
  keep full clearance without seeing each other.
- **Port** — the point where a wire meets a side. **Stub** — the straight,
  perpendicular run from the port across its own keep-out into the first
  corridor; never shorter than the wire's marker.
- **Crossing** — an intersection of two wires: exactly perpendicular, both wires
  locally straight, point contact.

---

## The Four Laws

Checkable on the output with no knowledge of the router:

1. **Clearance.** A wire keeps ≥ `clearance` from every node body and from every
   other wire. Exactly three surrenders: its own stubs (each crossing only its
   own endpoint's keep-out, perpendicular), crossings (square-on, point
   contact), and the **row band** of a compacted side (Law 2's compaction
   clause): the strip bounded by such a row's outermost ports, running
   perpendicular to the side, where the row's wires converge from lane
   spacing to the row's pitch. Inside the band the row's wires surrender
   clearance *to one another*; outside it — and against every wire not
   landing on the row — full clearance holds.

2. **Contact.** Every wire end lands **on a side**, **perpendicular**, and
   ≥ `clearance` from that side's corners — never *on* a corner, never inside a
   body, its own endpoints included. Ports sharing a side are evenly spaced,
   ≥ `clearance` apart, their median at the side's centre — or slid as one
   group along the side, spacing and order intact, when the centred rows
   would put a port within `clearance` of another wire (verifiable on the
   output: an off-centre group is excused only by such a neighbour). A side
   holding a **single** port leaves it free along the side (corner margins
   still hold): the engine aligns it with its wire so a straight shot
   arrives straight, centred when nothing demands otherwise. A side
   asked for more than its capacity **compacts**: all of its ports —
   incumbent and new alike — re-space **evenly** at the widest pitch the
   side allows, `usable length / (ports − 1)`, corner margins intact at full
   `clearance`, like the pins of an IC. On the output a sub-clearance pitch
   is excused only by genuine overflow — more ports than the side's
   capacity — and must be uniform; in the limit of a side too short for
   distinct points, ports coincide.

3. **Economy.** No crossing exists that a different lane order or a longer route
   could remove — a wire always detours rather than crosses. Among crossing-free
   legal routes a wire takes the **shortest**; among equally short, the **fewest
   turns**. Surviving crossings are square-on and **counted in the engine's
   report** — a crossing the report doesn't name is a bug.

4. **Determinism.** The same input renders byte-identically, every time. Every
   tie in the model breaks on declaration order.

When the layout leaves a wire no legal route — an endpoint walled in, a forced
side with no reachable port — the wire is **reported and drawn as an airwire**
(§Impossible layouts), never as a lawful-looking wire; `--strict` turns the
report into an error. Node positions are never moved for a wire; the user's
`gap` is the density dial, and when wires are impossible for lack of corridor
lanes the affected containers' gaps grow by exactly the deficit — the one
sanctioned layout feedback, last resort before the airwire.

---

## The Model

Seven steps. Each is deterministic; together they make the laws true by
construction rather than by checking.

1. **Keep-outs.** Layout finishes first and is immutable. Every node body
   inflates by `clearance` into a keep-out. Per wire: its endpoints' ancestor
   groups are transparent, everything else solid.

2. **Track graph.** The free space — canvas plus margin, minus keep-outs —
   decomposes into corridors. Corridor junctions are vertices; corridor runs are
   edges; every corridor knows its lane count. The graph depends only on node
   placement — wires never reshape it.

3. **Requests.** Resolve expands every wire statement (chains, fans, `&`-groups)
   into edges. Edges with the same unordered endpoint pair and same forced sides
   form one **bundle** of multiplicity *k*. Requests are ordered by declaration,
   then expansion order within a statement — the order every later tie breaks on.

4. **Paths.** Each bundle takes the cheapest track-graph path between its
   endpoints, entering the graph through up to four side-stubs per endpoint (a
   forced side prunes to one). Cost is lexicographic **(length, turns)**, ties by
   declaration order. A corridor without *k* free lanes over the needed span is
   **closed** to a bundle of multiplicity *k* — it detours; lanes are never
   squeezed. A side without a free port (capacity
   `floor((side length − 2·clearance) / clearance) + 1`, minimum 1) closes the
   same way, so wires spread to the next-cheapest side instead of cramming.
   A bundle no route can hold whole splits — *k* into ⌈k/2⌉ and the rest,
   down to singles — before it is ever reported impossible (§Duplicates).

5. **Lanes.** Within a corridor, a bundle takes adjacent lanes; wires take the
   relative order that matches the order of their ends outside the corridor —
   nested, never braided. When two wires' ends demand opposite orders at the
   corridor's two ends, the pair is **inverted**: they cross once, square-on —
   at the swap, or wherever the drawn geometry already crosses them (a swap
   whose halves land on one side of its partner buys no crossing and is not
   drawn). A run that ends at a port keeps the port's ordinate for its final
   approach; through-runs sit centred in the corridor at `clearance` pitch.
   Runs that never overlap along the corridor have no boundary order to match;
   their lane order — and whether two of them share a lane — is chosen so
   their staircases nest apart rather than interleave within clearance.

6. **Crossing audit.** Every crossing — an inversion, or one wire piercing a
   corridor another runs along — is audited: crossing-involved wires re-route
   with the crossing count against drawn wires leading the cost, and each
   round keeps the single reroute that most lowers the diagram's actual total
   without raising its law-1 breaches; at a plateau, paired moves run (one
   wire steps aside so another can clear), kept only if the pair strictly
   improves. Every applied move strictly drops the total, so the audit
   terminates. Whatever remains is forced, square-on, and reported with its
   wire pair. Law 1 is audited the same way: ports settle only after routing,
   so two pinned approaches can land within `clearance` of each other — the
   engine then reroutes one of the pair (or a wire sharing a port side with
   one), slides a port group off the conflicted row, or, when nothing legal
   exists, undraws and reports the later wire. Every repair is judged on the
   drawn ground truth and must strictly improve `(conflicts, crossings)`.

7. **Geometry.** Port order along a side equals lane order, so wires never braid
   at the mouth. Stubs run straight and perpendicular, markers ride them.
   Corners round with radius `min(clearance, half the shorter adjacent run)`,
   with two refinements: corners nested on one diagonal round
   **concentrically** — each radius grows outward by exactly the corner
   offset, so wires turning together hold their gap through the arc instead
   of flaring — and every radius caps at the nearest crossing on its legs,
   so a crossing never lands mid-arc (an arc may land tangent exactly on
   one, keeping the perpendicular point contact). Rounding never brings a
   wire nearer than `clearance` to anything. Wire
   labels ride their wire at the fractions of its drawn route given by `along:`
   (auto-distributed when unset), nudged in the tangent frame by `translate:`;
   they are obstacles to nothing and may slide along the wire to dodge nodes and
   other labels — the wire itself never moves for a label.

---

## Ports — which side

- A **forced side wins**: `a.r -> b` leaves `a` on the right. If no legal route
  exists from that side, the wire is impossible and reported — a forced side is
  never bent to fit.
- Otherwise **the path chooses the side**: step 4 enters the graph through all
  four stubs, so a wire lands on whichever side gives the cheapest legal route —
  facing sides for neighbours, an L for diagonals, a far side only when every
  nearer one is full or walled.
- **Cramming is impossible by construction**: a full side is a closed door, and
  the path search walks to the next one. Only a wire that would otherwise be
  impossible — every side full, every lever spent — unlocks **port
  compaction** (Law 2's compaction clause): the landing side re-pitches
  every port on it evenly below `clearance`; the row's wires converge
  inside the band its outermost ports bound and fan out to full clearance
  beyond it, like escape routing away from a fine-pitch chip. Compaction
  changes no geometry, so it runs before gap growth; it never runs during
  normal routing. The same lever may overflow the **canvas margin** — the
  outer bound is the router's own construct, open outward, with overflow
  lanes pitched away from the scene; running out of margin is never a
  reason to fail.
- **A port group may slide.** When another wire's geometry sits within
  `clearance` of a side's centred port rows — a punch through a transparent
  wall, a facing node's approach — the whole group slides along the side,
  exactly as far as needed, keeping its spacing, order, and corner margins.
  The wire bends to its port mid-corridor; the port never bends the law.
- **A lone port meets its wire.** A side holding one port does not pin it to
  the centre: a straight shot between two sides rides **one ordinate** end
  to end — when the centres miss, the movable end (its side lone, no fan
  trunk, no accepted slide, corner margins kept, the line inside its
  corridors) re-pins to its partner's ordinate, goal end first — so centre
  misalignment never buys a pair of stub-side turns. An end that cannot
  move keeps the centred jog, and every other law — clearance included —
  still holds on the straightened line.

---

## Special shapes

- **Fan** (`a -> b & c`, one shared end): siblings share one port and one stub on
  the shared end; the shared port is the one the first sibling's path picks.
  Along their common path prefix — the **trunk** — siblings are exempt from
  separation (there is nothing to separate: it is drawn as one line); past the
  split each is a full wire under every law.
- **Chain** (`a -> b -> c`): separate wires, nothing shared.
- **Duplicates** (`a -> b` twice): one bundle — adjacent lanes, parallel rails
  the whole way. Adjacent rails are the preferred form, not a vow: when no
  route holds the whole bundle, it splits — half by half, singles last — and
  each piece routes as a bundle in its own right. Splitting beats vanishing.
- **Self-loop** (`a -> a`): out one side, around the keep-out corner on the
  nearest lane, back in an adjacent side. Defaults right → top; forced sides win,
  but both ends forced onto one side is an error.
- **Bidirectional** (`a <-> b`): one wire, a marker at each end.
- **Containment** (one endpoint inside the other): the wire runs *inside* the
  parent — from the inner node's side to the parent's **inner** side, with the
  parent's other children solid.

---

## Impossible layouts

The laws are absolute. When geometry allows no legal route — an endpoint whose
every side is walled shut, a forced side that cannot reach the target, an
interleaving no detour resolves — the engine draws every wire it can and **names
the ones it couldn't**, each with its source span. `--strict` makes that an
error. The engine never draws a wire through a node, against a node, oblique
into a side, or braided over another wire to paper over a crowded diagram.

An impossible verdict is never first-come: before reporting, the engine
retries the starved wire with margins relaxed to the corridor walls, splits
its bundle (§Duplicates), and inserts it along its best unconstrained route
while the audit machinery asks the incumbents to move — the rearrangement
kept only when strictly more of the diagram draws, judged on the drawn
ground truth. Past those levers, a wire starved of **port slots** compacts
its landing side (Law 2's compaction clause), and a wire starved of
**corridor lanes** triggers gap growth: the deficient containers' gaps widen
by exactly the missing lanes' worth and layout + routing rerun, bounded.
Only what survives all of that is reported.

What is reported is also **seen**: an impossible wire renders as an
**airwire** — a single straight segment between its two bodies, centre to
centre and trimmed to their boundaries, at whatever angle the geometry
gives. Lawful wires are orthogonal, so a slanted line cannot be mistaken
for one; it is dashed in the themable `--lini-airwire` style with a
warning glyph at its midpoint. An airwire is the report made visible, not a
wire: it obeys no law, takes no port slot, and blocks nothing.

The report also **counts the crossings** it was forced to keep, naming each wire
pair — so "no crossing unless impossible" is not a promise but an audited fact.

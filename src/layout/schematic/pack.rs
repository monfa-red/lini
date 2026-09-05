//! **Packing** [SPEC 16.1] — the anchor tracks, sized on the fine grid.
//!
//! Every anchor's origin lands on a fine lattice line, and what parts two of
//! them is what stands between: the earlier one's field on the side they face,
//! the members of any span riding between them, the later one's field, and
//! the corridor the two keep — a fine track per wire crossing it. A part's
//! own ink is read here and nowhere else in the packing, and only ever to say
//! how far two of them must stand apart or how far the sheet reaches — never
//! where a part lands, so a long value still overhangs the column beside it
//! rather than parting the tracks.
//!
//! **Facing pins align** first: where a wire — or a **span**, whose members
//! all ride one line — joins an earlier column's right pin to a later column's
//! left pin, the later anchor offsets so the two landings share a line and the
//! bus draws dead straight. The offset is a whole number of fine pitches
//! ([`Lattice::snap`]), so the lattice survives it, and the cells it consumes
//! are charged to the track like any other content — struck before the sizing,
//! an aligned anchor can never overrun the allotment its neighbour was placed
//! against.

use super::super::ir::{Bbox, PlacedNode};
use super::field::{Field, drawn, edges};
use super::lattice::{Ax, EPS, Lattice};
use super::place::{Slot, collapse};
use super::terminal::{Terminal, terminal};
use crate::desugar::pose::Side;
use crate::desugar::schematic::chain::End;
use crate::resolve::ResolvedLink;

/// Where every anchor lands [SPEC 16.1]: the ordinal track grid, sized on the
/// fine grid from its anchors' fields.
pub(super) struct Packing {
    /// Per anchor, its origin in scope coordinates, parallel to `anchored`.
    pub origins: Vec<(f64, f64)>,
    /// The packed content box — every anchor's own ink and the cells its field
    /// holds, and nothing a `translate:` could move, so a nudge never grows
    /// the sheet [SPEC 5].
    pub body: Bbox,
}

pub(super) fn pack(
    children: &[PlacedNode],
    anchored: &[usize],
    slots: &[Slot],
    links: &[&ResolvedLink],
    scope: &str,
    field: &Field,
    lat: Lattice,
) -> Packing {
    let offsets = align(children, links, scope, anchored, slots, field, lat);
    let tracks = Tracks {
        children,
        anchored,
        slots,
        field,
        offsets: &offsets,
        lat,
    };
    let hops = edges(children, links, scope);
    let x = tracks.axis(Ax::X, &hops);
    let y = tracks.axis(Ax::Y, &hops);
    let origins: Vec<(f64, f64)> = x.into_iter().zip(y).collect();
    let body = origins
        .iter()
        .zip(anchored)
        .map(|(&(x, y), &i)| extent(field, &children[i], i).shifted(x, y))
        .reduce(Bbox::union)
        .unwrap_or_else(Bbox::empty);
    Packing { origins, body }
}

/// The packing's one reading of the scope, shared by its two axes.
struct Tracks<'a> {
    children: &'a [PlacedNode],
    anchored: &'a [usize],
    slots: &'a [Slot],
    field: &'a Field,
    /// Each anchor's alignment offset, parallel to `anchored` ([`align`]).
    offsets: &'a [(f64, f64)],
    lat: Lattice,
}

/// One anchor's whole extent [SPEC 16.1]: its own drawn ink, and the **cells**
/// its field holds — never its satellites' ink, so a long value overhangs the
/// sheet's edge rather than growing it.
fn extent(field: &Field, node: &PlacedNode, anchor: usize) -> Bbox {
    let ink = drawn(node);
    let out = |side: Side| field.extent(anchor, side).max(0.0);
    ink.union(Bbox::from_points(&[
        (-out(Side::Left), -out(Side::Top)),
        (out(Side::Right), out(Side::Bottom)),
    ]))
}

impl Tracks<'_> {
    /// One axis of the packing [SPEC 16.1]: each track takes the first fine
    /// line every anchor before it leaves free, and every anchor stands on its
    /// track's line plus its own alignment offset. `hops` are the scope's
    /// wires, one per hop ([`edges`]).
    fn axis(&self, ax: Ax, hops: &[[End; 2]]) -> Vec<f64> {
        let (children, anchored, slots, field) =
            (self.children, self.anchored, self.slots, self.field);
        let (step, pitch) = (self.lat.step(ax), self.lat.pitch);
        let ordinals = collapse(slots.iter().map(|s| s.on(ax)));
        let track = |k: usize| {
            ordinals
                .binary_search(&slots[k].on(ax))
                .expect("a collapsed ordinal")
        };
        let shift = |k: usize| match ax {
            Ax::X => self.offsets[k].0,
            Ax::Y => self.offsets[k].1,
        };
        let (back, ahead) = match ax {
            Ax::X => (Side::Left, Side::Right),
            Ax::Y => (Side::Top, Side::Bottom),
        };
        let ink = |k: usize, side| edge(&children[anchored[k]], side);
        let crossings = crossings(hops, anchored, field, &track, ordinals.len());
        // How far an anchor reaches out on a side: to its field's outermost
        // cell edge, or to its own drawn ink where no chain went that way — a
        // field stands clear of the ink already, so the further is the edge.
        let reach = |k: usize, side| field.extent(anchored[k], side).max(ink(k, side));
        // What one anchor owes another ahead of it: its reach that way, the
        // span riding between them on its coarse cells, the reach back at it,
        // and the corridor between the two — a fine pitch of air per wire
        // crossing it, one at the least — and never less than one coarse cell
        // centre to centre. Their offsets are already struck, so the cells
        // those consume are part of the distance rather than a shift applied
        // over the top.
        let apart = |k: usize, j: usize| -> f64 {
            let wires = (track(k) + 1..=track(j))
                .map(|b| crossings[b])
                .sum::<usize>()
                .max(1);
            let span = spanning(field, anchored, k, j);
            // A span's members ride coarse cells counted from the first free
            // coarse line past the landing anchor's field ([`Field::lay`]),
            // the first-named one furthest out; the region ends half a cell
            // past it.
            let held_back = if span > 0 {
                let free = (field.free(anchored[j], back) / step - EPS).ceil();
                (free + f64::from(span) - 0.5) * step
            } else {
                reach(j, back)
            };
            let d = (reach(k, ahead) + pitch * wires as f64 + held_back).max(step);
            // Onto the fine grid, taking the lattice's slack before the
            // ceiling as every other count does.
            (d / pitch - EPS).ceil() * pitch + shift(k) - shift(j)
        };
        let mut line = vec![0.0f64; ordinals.len()];
        for t in 1..ordinals.len() {
            let mut at = line[t - 1];
            for j in (0..anchored.len()).filter(|&j| track(j) == t) {
                for k in (0..anchored.len()).filter(|&k| track(k) < t) {
                    at = at.max(line[track(k)] + apart(k, j));
                }
            }
            line[t] = at;
        }
        (0..anchored.len())
            .map(|k| line[track(k)] + shift(k))
            .collect()
    }
}

/// The wires that **cross** each track boundary on one axis [SPEC 16.1] —
/// `crossings[b]` counts those with one end before track `b` and the other
/// at or past it: every hop between two anchors, and every span between two.
/// A wire may jog in the corridor it crosses, so the corridor holds a fine
/// track for each; a wire between two anchors of one track crosses nothing.
fn crossings(
    hops: &[[End; 2]],
    anchored: &[usize],
    field: &Field,
    track: &dyn Fn(usize) -> usize,
    tracks: usize,
) -> Vec<usize> {
    let mut out = vec![0usize; tracks + 1];
    let anchor_of = |child: usize| anchored.iter().position(|&i| i == child);
    let mut cross = |a: usize, b: usize| {
        let (lo, hi) = (track(a).min(track(b)), track(a).max(track(b)));
        for slot in out.iter_mut().take(hi + 1).skip(lo + 1) {
            *slot += 1;
        }
    };
    for [a, b] in hops {
        if let (Some(ka), Some(kb)) = (anchor_of(a.child), anchor_of(b.child)) {
            cross(ka, kb);
        }
    }
    for s in field.spans() {
        if let (Some(ka), Some(kb)) = (anchor_of(s.ends[0].0), anchor_of(s.ends[1].0)) {
            cross(ka, kb);
        }
    }
    out
}

/// How far one part's **drawn** ink reaches out on `side` of its own origin —
/// its leads and readouts included, since they are ink on the sheet and the
/// router keeps its clearance off them too.
fn edge(node: &PlacedNode, side: Side) -> f64 {
    let ink = drawn(node);
    match side {
        Side::Left => -ink.min_x,
        Side::Right => ink.max_x,
        Side::Top => -ink.min_y,
        Side::Bottom => ink.max_y,
    }
}

/// The coarse cells the spans held between two anchors ask of the region
/// between their tracks [SPEC 16.1] — one per member. Two spans on one pair of
/// anchors ride two landing legs, so the region holds the longer of them
/// rather than both.
fn spanning(field: &Field, anchored: &[usize], k: usize, j: usize) -> i32 {
    field
        .spans()
        .iter()
        .filter(|s| {
            let ends = [s.ends[0].0, s.ends[1].0];
            ends.contains(&anchored[k]) && ends.contains(&anchored[j])
        })
        .map(|s| field.span_cells(s))
        .max()
        .unwrap_or(0)
}

/// Each anchor's offset from its track's own line [SPEC 16.1]. Anchors stand
/// centre to centre by default — no offset at all — except where a wire pairs
/// an anchor's pin with a **facing** pin of an already-placed anchor in the
/// same track row: its right pins against a later column's left pins. The
/// later anchor then offsets so the two landings share one line and the wire
/// draws dead straight; columns mirror it with bottom pins against top.
///
/// A **span** names such a pair exactly as a bare wire does — its members all
/// ride one line, so the pin it leaves and the pin it lands on are as much a
/// facing pair as two pins wired directly. Its hops are not: each is
/// anchor-to-satellite, and read hop by hop the pair is never seen at all
/// ([`Field::spans`] is the one place it is written down).
///
/// The offset is snapped to a whole number of **fine** pitches, so an aligned
/// anchor is still on the lattice — which is also why a part's pins stand a
/// whole pitch from its own centre [SPEC 16.2]: the snap is then exact and the
/// straight wire really is straight. A symbol whose pins straddle its centre
/// line instead [SPEC 16.3] keeps whatever it is off by, and the wire jogs.
///
/// Deterministic throughout: anchors take alignment in track order (rows, then
/// columns within one), each aligning through the first statement-order wire
/// that reaches a placed neighbour; everything else keeps the line.
fn align(
    children: &[PlacedNode],
    links: &[&ResolvedLink],
    scope: &str,
    anchored: &[usize],
    slots: &[Slot],
    field: &Field,
    lat: Lattice,
) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = vec![(0.0, 0.0); anchored.len()];
    let hops = edges(children, links, scope);
    let anchor_of = |child: usize| anchored.iter().position(|&i| i == child);
    // A wire's far end as a **placed** pair: the anchor it lands on and the
    // terminal it lands at — read straight off the end where the wire reaches
    // another anchor, and through the span the end starts where it reaches a
    // satellite instead.
    let facing_end = |k: usize, theirs: &End| -> Option<(usize, Terminal)> {
        if let Some(j) = anchor_of(theirs.child) {
            return Some((
                j,
                terminal(&children[anchored[j]], theirs.terminal.as_deref()),
            ));
        }
        let span = field
            .spans()
            .iter()
            .find(|s| s.members.contains(&theirs.child))?;
        let (child, far) = *span.ends.iter().find(|(c, _)| *c != anchored[k])?;
        Some((anchor_of(child)?, far))
    };
    // One pass per axis: `set` marks anchors whose offset on that axis is
    // final (the first anchor of every track seeds it), and each later anchor
    // takes the first wire to a set neighbour whose pins face each other.
    let pass = |vertical: bool, out: &mut Vec<(f64, f64)>| {
        let mut order: Vec<usize> = (0..anchored.len()).collect();
        order.sort_by_key(|&k| {
            if vertical {
                (slots[k].row, slots[k].col)
            } else {
                (slots[k].col, slots[k].row)
            }
        });
        let mut set = vec![false; anchored.len()];
        for &k in &order {
            let hit = hops.iter().find_map(|[a, b]| {
                let (mine, theirs) = if anchor_of(a.child) == Some(k) {
                    (a, b)
                } else if anchor_of(b.child) == Some(k) {
                    (b, a)
                } else {
                    return None;
                };
                let (j, their_t) = facing_end(k, theirs)?;
                let same_track = if vertical {
                    slots[j].row == slots[k].row && slots[j].col != slots[k].col
                } else {
                    slots[j].col == slots[k].col && slots[j].row != slots[k].row
                };
                if !set[j] || !same_track {
                    return None;
                }
                let mine_t = terminal(&children[anchored[k]], mine.terminal.as_deref());
                // Only pins pointing **at** each other align — the pair a
                // straight wire can actually join.
                let facing = if vertical {
                    if slots[k].col > slots[j].col {
                        (Some(Side::Left), Some(Side::Right))
                    } else {
                        (Some(Side::Right), Some(Side::Left))
                    }
                } else if slots[k].row > slots[j].row {
                    (Some(Side::Top), Some(Side::Bottom))
                } else {
                    (Some(Side::Bottom), Some(Side::Top))
                };
                if (mine_t.facing, their_t.facing) != facing {
                    return None;
                }
                Some(lat.snap(if vertical {
                    out[j].1 + their_t.at.1 - mine_t.at.1
                } else {
                    out[j].0 + their_t.at.0 - mine_t.at.0
                }))
            });
            if let Some(o) = hit {
                if vertical {
                    out[k].1 = o;
                } else {
                    out[k].0 = o;
                }
            }
            set[k] = true;
        }
    };
    pass(true, &mut out);
    pass(false, &mut out);
    out
}

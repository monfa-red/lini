//! The anchor **track grid** [SPEC 16.1] — the engine's own track list.
//!
//! Anchors ride tracks: one row by default in declaration order, `columns: N`
//! wraps the flow, and `cell: c r` places explicitly. The indices are
//! **ordinal** — a track springs into existence at every referenced ordinal and
//! empty ones collapse entirely, so sparse indices (10, 20, 30…) are pure
//! ordering room that never injects invisible space. That collapse is why this
//! is a track list of its own and not `layout: grid`: the grid's laws (a
//! declared `columns` track list, the empty-`""`-cell law [SPEC 12]) are
//! untouched — only its *placement helpers* are shared
//! ([`grid::read_cell`], [`grid::cumulative`]).

use super::super::flex::Axis;
use super::super::ir::{Bbox, PlacedNode};
use super::super::{anchors, flex, grid, primitives};
use super::seat::{Seats, seat_gap};
use crate::desugar::schematic::{Role, role as schematic_role, sch_kind, terminal_ids};
use crate::error::{Code, Error};
use crate::resolve::{AttrMap, ResolvedLink, ResolvedValue};
use crate::span::Span;

/// Classify one direct child [SPEC 16.1] — through the one role table, which
/// the pose chooser reads too ([`crate::desugar::schematic::role`]), so a part
/// cannot be posed as a satellite and then seated as an anchor.
///
/// A lowered node's family is its `type_chain` [SPEC 16.7]: lowering turns the
/// authored type into a primitive plus `lini-*` classes, and resolve reads
/// those back as the chain — the same names desugar dispatched on.
pub(super) fn role(node: &PlacedNode) -> Role {
    let kind = sch_kind(&node.type_chain);
    schematic_role(
        anchors::is_pinned(&node.attrs),
        node.attrs.get("cell").is_some(),
        kind,
        // How many wirable terminals the part carries [SPEC 16.2/16.3] — its
        // own, never its descendants': a container holding a two-pin part is
        // not itself a jumper. The one pin walk, which the router's ports and
        // resolve's arity read too, so no two stages can count a part's pins
        // differently.
        terminal_ids(node).len(),
    )
}

/// A place on the ordinal grid, 1-indexed as authored.
#[derive(Clone, Copy)]
struct Slot {
    col: usize,
    row: usize,
}

/// `columns: N` — how many columns the schematic's **flow** takes before it
/// wraps [SPEC 16.1]. Not the grid's `columns`, which is a track *list* sizing
/// its tracks: a schematic's tracks size themselves from their anchors'
/// clusters, so the only thing left to state is the wrap count. Explicit
/// `cell:` ordinals are free to reach past it — they place, they don't flow.
fn read_columns(attrs: &AttrMap, span: Span) -> Result<Option<usize>, Error> {
    let Some(v) = attrs.get("columns") else {
        return Ok(None);
    };
    // A one-value list is the normalized scalar [SPEC 2].
    let count = match v {
        ResolvedValue::List(items) => match items.as_slice() {
            [one] => one.as_number(),
            _ => None,
        },
        other => other.as_number(),
    };
    match count {
        Some(n) if n >= 1.0 && n.fract() == 0.0 => Ok(Some(n as usize)),
        _ => Err(Error::at(
            span,
            "'columns' in a schematic is the wrap count — a positive integer (its tracks size to their anchors)",
        )
        .code(Code::SCHEMATIC_TRACKS)),
    }
}

/// Assign every anchor its ordinal slot: an explicit `cell: c r` places, and
/// the rest flow in declaration order through the slots the explicit ones left
/// free, wrapping at `columns` (one unbounded row without it) [SPEC 16.1].
fn slots(
    children: &[PlacedNode],
    riders: &[usize],
    columns: Option<usize>,
) -> Result<Vec<Slot>, Error> {
    let mut out: Vec<Option<Slot>> = Vec::with_capacity(riders.len());
    let mut taken: Vec<(usize, (usize, usize))> = Vec::new();
    for &i in riders {
        let placed = grid::read_cell(&children[i].attrs, children[i].span)
            .map_err(|e| e.code(Code::SCHEMATIC_TRACKS))?;
        if let Some(cell) = placed {
            // Two anchors on one ordinal would stack, and a wire between two
            // parts sharing a point cannot route [SPEC 16.1/21]. Name both.
            if let Some(&(who, _)) = taken.iter().find(|(_, c)| *c == cell) {
                return Err(Error::at(
                    children[i].span,
                    format!(
                        "cell {} {} already holds '{}' — give '{}' its own ordinal",
                        cell.0,
                        cell.1,
                        part_name(&children[who]),
                        part_name(&children[i]),
                    ),
                )
                .code(Code::SCHEMATIC_TRACKS));
            }
            taken.push((i, cell));
        }
        out.push(placed.map(|(col, row)| Slot { col, row }));
    }
    let taken: Vec<(usize, usize)> = taken.into_iter().map(|(_, c)| c).collect();
    let (mut col, mut row) = (1usize, 1usize);
    for slot in out.iter_mut() {
        if slot.is_some() {
            continue;
        }
        while taken.contains(&(col, row)) {
            (col, row) = advance(col, row, columns);
        }
        *slot = Some(Slot { col, row });
        (col, row) = advance(col, row, columns);
    }
    Ok(out
        .into_iter()
        .map(|s| s.expect("every slot filled"))
        .collect())
}

/// How an error names a part: its reference designator [SPEC 16.2] — the id an
/// author reads it by — else its written type.
fn part_name(node: &PlacedNode) -> String {
    node.id
        .clone()
        .or_else(|| {
            crate::desugar::schematic::schematic_type(&node.type_chain).map(|t| format!("|{t}|"))
        })
        .unwrap_or_else(|| "a part".to_string())
}

/// The flow cursor's next slot: one column on, wrapping to the next row at the
/// `columns` count (an unset count is one unbounded row).
fn advance(col: usize, row: usize, columns: Option<usize>) -> (usize, usize) {
    match columns {
        Some(n) if col >= n => (1, row + 1),
        _ => (col + 1, row),
    }
}

/// The **ordinal collapse** [SPEC 16.1]: the sorted distinct ordinals used on
/// one axis, so `10, 20, 30` become tracks 0, 1, 2 and every skipped ordinal
/// vanishes instead of reserving space.
fn collapse(used: impl Iterator<Item = usize>) -> Vec<usize> {
    let mut v: Vec<usize> = used.collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Place a schematic scope's already-laid-out children [SPEC 16.1] and return
/// the content bbox. **The pass order**, which is what makes a cluster
/// possible at all:
///
/// 1. classify (the role table), then **seat** every satellite pin-relative —
///    off its anchor's own origin, so no anchor need be placed yet;
/// 2. size the tracks from each anchor's **cluster** (itself plus its seats);
/// 3. place the anchors on the collapsed ordinal grid;
/// 4. **absolutize** the seats — a pin-relative one rides its anchor, a
///    two-ended chain reads the two now-placed pins;
/// 5. flow the satellites no wire held (the caller warns), seat the `pin:`
///    overlays on the finished box, and take every `translate:` nudge last.
pub(super) fn arrange(
    children: &mut [PlacedNode],
    attrs: &AttrMap,
    span: Span,
    links: &[&ResolvedLink],
    scope: &str,
) -> Result<Bbox, Error> {
    if children.is_empty() {
        return Ok(Bbox::empty());
    }
    let (gap_y, gap_x) = primitives::gap(attrs, span)?;
    let roles: Vec<Role> = children.iter().map(role).collect();
    let anchored: Vec<usize> = (0..children.len())
        .filter(|&i| roles[i] == Role::Anchor)
        .collect();
    let seats = Seats::build(children, &roles, links, scope, seat_gap(attrs));

    let slots = slots(children, &anchored, read_columns(attrs, span)?)?;
    let cols = collapse(slots.iter().map(|s| s.col));
    let rows = collapse(slots.iter().map(|s| s.row));
    let index = |list: &[usize], ord: usize| list.binary_search(&ord).expect("a collapsed ordinal");

    // Auto tracks only: each sizes to the widest / tallest cluster it holds —
    // an anchor's satellites consume space without consuming cells.
    let clusters: Vec<Bbox> = anchored
        .iter()
        .map(|&i| seats.cluster(children, i))
        .collect();
    let mut widths = vec![0.0f64; cols.len()];
    let mut heights = vec![0.0f64; rows.len()];
    for (extent, slot) in clusters.iter().zip(&slots) {
        let (c, r) = (index(&cols, slot.col), index(&rows, slot.row));
        widths[c] = widths[c].max(extent.w());
        heights[r] = heights[r].max(extent.h());
    }
    // A chain held at both ends seats **between** its anchors [SPEC 16.1], in
    // no cluster and no track: so it sizes the space between the two tracks it
    // spans. Whatever the even fractions still need is charged to the gaps
    // lying between them — the chain's own extent, never a constant bump (a
    // bump only moves the threshold at which the seats collide).
    let (mut extra_x, mut extra_y) = (vec![0.0; cols.len()], vec![0.0; rows.len()]);
    for demand in seats.demands(children) {
        let anchor = |end: usize| {
            let (child, at) = demand.ends[end];
            let k = anchored.iter().position(|&i| i == child)?;
            let (ex, ey) = clusters[k].center();
            Some((slots[k], (at.0 - ex, at.1 - ey)))
        };
        let (Some((sa, oa)), Some((sb, ob))) = (anchor(0), anchor(1)) else {
            continue;
        };
        let (ca, cb) = (index(&cols, sa.col), index(&cols, sb.col));
        let (ra, rb) = (index(&rows, sa.row), index(&rows, sb.row));
        charge(
            &mut extra_x,
            &widths,
            gap_x,
            (ca, oa.0),
            (cb, ob.0),
            demand.need.0,
        );
        charge(
            &mut extra_y,
            &heights,
            gap_y,
            (ra, oa.1),
            (rb, ob.1),
            demand.need.1,
        );
    }
    let col_off = grid::cumulative_gaps(&widths, |i| gap_x + extra_x[i]);
    let row_off = grid::cumulative_gaps(&heights, |i| gap_y + extra_y[i]);
    let total_w = (col_off[cols.len()] - gap_x).max(0.0);
    let total_h = (row_off[rows.len()] - gap_y).max(0.0);

    // The anchor lands where its **cluster** centres, so a part with a ground
    // hanging under it sits high in its cell rather than straddling the row.
    for ((&i, slot), extent) in anchored.iter().zip(&slots).zip(&clusters) {
        let (c, r) = (index(&cols, slot.col), index(&rows, slot.row));
        let cell_cx = col_off[c] + widths[c] / 2.0 - total_w / 2.0;
        let cell_cy = row_off[r] + heights[r] / 2.0 - total_h / 2.0;
        let (ex, ey) = extent.center();
        children[i].cx = cell_cx - ex;
        children[i].cy = cell_cy - ey;
    }
    // An anchor's `translate:` lands **before** its seats absolutize, so the
    // satellites ride along — move the component and the nudge travels with it
    // [SPEC 16.1]. It cannot grow the scope: the tracks already sized from the
    // un-nudged clusters, and a nudge never reshapes a box [SPEC 5].
    for &i in &anchored {
        nudge(&mut children[i])?;
    }
    let mut body = Bbox::centered(total_w, total_h);
    seats.absolutize(children);
    // Every anchor's ink is already in its cluster, and so in a track; a
    // spanning chain rides neither, so its parts join the box here.
    if let Some(span) = seats.spanning_extent(children) {
        body = body.union(span);
    }

    // A satellite no wire holds has nothing to seat against [SPEC 16.1]: it
    // falls back to the flow — one trailing row under the grid, declaration
    // order, at the scope's gap — and the caller reports it.
    let adrift = seats.floating();
    if !adrift.is_empty() {
        let mut row: Vec<PlacedNode> = adrift.iter().map(|&i| children[i].clone()).collect();
        let strip = flex::lay_out_flex(Axis::Row, &mut row, attrs, span, (None, None))?;
        let dy = if anchored.is_empty() {
            0.0
        } else {
            body.max_y + gap_y - strip.min_y
        };
        for (&i, placed) in adrift.iter().zip(row) {
            children[i] = placed;
            children[i].cy += dy;
        }
        body = body.union(strip.shifted(0.0, dy));
    }

    // `pin:` in a schematic scope is the drawing precedent [SPEC 5/15.8]: an
    // out-of-flow overlay flush on the scope's finished content box — sheet
    // chrome (a note, a legend), never a part on the grid. It never grows the
    // scope, so it seats against the body the tracks produced.
    anchors::place_pinned(children, body)?;

    // A satellite's own `translate:` nudges it off its seat, last — after the
    // flow fallback, which rewrites the node a floating one landed in. The
    // anchors took theirs above and the pinned overlays theirs in
    // `place_pinned`, so every child is nudged exactly once.
    for i in 0..children.len() {
        if roles[i] == Role::Satellite {
            nudge(&mut children[i])?;
        }
    }
    Ok(body)
}

/// Charge one axis's gaps with what a spanning chain still lacks [SPEC 16.1]:
/// the pin-to-pin distance the tracks currently offer is the two half tracks,
/// everything between them, their gaps (charges included, so a second chain
/// asks only for what the first left short) and the two landings' own offsets
/// from their cell centres. The shortfall spreads over the gaps in between —
/// [`grid::charge`], the same spreader a grid's spanning cell runs through.
/// Anchors sharing a track ask nothing of it — there is no gap between a track
/// and itself; the chain then sizes the other axis alone.
fn charge(extra: &mut [f64], sizes: &[f64], gap: f64, a: (usize, f64), b: (usize, f64), need: f64) {
    let ((lo, lo_off), (hi, hi_off)) = if a.0 <= b.0 { (a, b) } else { (b, a) };
    if lo == hi {
        return;
    }
    let between: f64 = sizes[lo + 1..hi].iter().sum::<f64>()
        + extra[lo..hi].iter().sum::<f64>()
        + (hi - lo) as f64 * gap;
    let have = sizes[lo] / 2.0 + between + sizes[hi] / 2.0 + hi_off - lo_off;
    grid::charge(&mut extra[lo..hi], have, need);
}

/// `translate:` — a post-placement shift, reshaping nothing [SPEC 5]. Always
/// sheet-space: a schematic's interior never rides a view scale
/// [SPEC 15.1/16.6].
fn nudge(node: &mut PlacedNode) -> Result<(), Error> {
    if let Some((dx, dy)) = anchors::translate(&node.attrs, node.span)? {
        node.cx += dx;
        node.cy += dy;
    }
    Ok(())
}

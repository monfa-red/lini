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
use super::super::{anchors, flex, grid};
use super::field::Field;
use super::lattice::{Ax, Lattice};
use super::pack::pack;
use super::readout;
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
pub(super) struct Slot {
    pub col: usize,
    pub row: usize,
}

impl Slot {
    /// Its ordinal on one lattice axis — the column on `X`, the row on `Y`.
    pub(super) fn on(self, ax: Ax) -> usize {
        match ax {
            Ax::X => self.col,
            Ax::Y => self.row,
        }
    }
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
pub(super) fn slots(
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
pub(super) fn collapse(used: impl Iterator<Item = usize>) -> Vec<usize> {
    let mut v: Vec<usize> = used.collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Place a schematic scope's already-laid-out children [SPEC 16.1] and return
/// the content bbox. **The pass order**, which is what lets a field be struck
/// before anything is placed:
///
/// 1. classify (the role table) and strike the **ordinal slots**, then run the
///    **field** pass — every satellite takes a ray, a lane and a slot in its
///    anchor's own frame, so no anchor need be placed yet, and a slot origin
///    can be shared by the track line the anchor rides;
/// 2. **pack** the tracks in whole coarse cells and land every anchor on the
///    lattice ([`pack`]);
/// 3. **absolutize** the field — a seated satellite rides its anchor, a span
///    reads the two now-placed landings;
/// 4. turn the **readouts** outward ([`readout`]);
/// 5. flow the satellites no wire held (the caller warns), then centre the
///    sheet on the scope's origin a whole number of fine pitches at a time, so
///    the lattice the passes agreed on stays absolute;
/// 6. seat the `pin:` overlays on the finished box, and take every
///    `translate:` nudge last.
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
    let lat = Lattice::of(attrs, span)?;
    let roles: Vec<Role> = children.iter().map(role).collect();
    let anchored: Vec<usize> = (0..children.len())
        .filter(|&i| roles[i] == Role::Anchor)
        .collect();
    // The slots are struck **before** the field, so a chain's slot origin can
    // be the track line's rather than its own anchor's [SPEC 16.1]. Nothing
    // here reads a field: an ordinal is the author's `cell:` or the flow's.
    let slots = slots(children, &anchored, read_columns(attrs, span)?)?;
    let mut tracks: Vec<Option<Slot>> = vec![None; children.len()];
    for (&i, &s) in anchored.iter().zip(&slots) {
        tracks[i] = Some(s);
    }
    let field = Field::build(children, &roles, links, scope, &tracks, lat);

    let packed = pack(children, &anchored, &slots, links, scope, &field, lat);
    for (&i, &(x, y)) in anchored.iter().zip(&packed.origins) {
        (children[i].cx, children[i].cy) = (x, y);
    }
    // An anchor's `translate:` lands **before** its seats absolutize, so the
    // satellites ride along — move the component and the nudge travels with it
    // [SPEC 16.1]. It cannot grow the scope: the packing already measured the
    // sheet off the un-nudged cells, and a nudge never reshapes a box [SPEC 5].
    for &i in &anchored {
        anchors::nudge(&mut children[i], anchors::SHEET_SPACE)?;
    }
    field.absolutize(children);
    // The readouts turn outward, which moves text and no part, so it changes
    // nothing the box was measured from [SPEC 16.2].
    let mut body = packed.body;
    readout::readouts(children, &field);

    // A satellite no wire holds has nothing to seat against [SPEC 16.1]: it
    // falls back to the flow — one trailing row under the grid, declaration
    // order, at the scope's gap — and the caller reports it.
    let adrift = field.floating();
    if !adrift.is_empty() {
        let mut row: Vec<PlacedNode> = adrift.iter().map(|&i| children[i].clone()).collect();
        let strip = flex::lay_out_flex(Axis::Row, &mut row, attrs, span, (None, None))?;
        let dy = if anchored.is_empty() {
            0.0
        } else {
            body.max_y + lat.row - strip.min_y
        };
        for (&i, placed) in adrift.iter().zip(row) {
            children[i] = placed;
            children[i].cy += dy;
        }
        body = body.union(strip.shifted(0.0, dy));
    }

    // The sheet centres on the scope's origin — the tracks start at it, but a
    // flowed-out satellite or a wide field hangs the body off to one side — and
    // the shift is a whole number of **fine** pitches, so the lattice every
    // pass agreed on survives the move [SPEC 16.1]. The half pitch that leaves
    // off centre goes to the **box**, which the caller draws a rect of and an
    // overlay seats flush on: it holds the sheet evenly either way.
    let (sx, sy) = body.center();
    let (sx, sy) = (lat.snap(sx), lat.snap(sy));
    for c in children.iter_mut() {
        c.cx -= sx;
        c.cy -= sy;
    }
    body = body.shifted(-sx, -sy);
    let body = Bbox::centered(
        2.0 * body.min_x.abs().max(body.max_x),
        2.0 * body.min_y.abs().max(body.max_y),
    );

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
            anchors::nudge(&mut children[i], anchors::SHEET_SPACE)?;
        }
    }

    Ok(body)
}

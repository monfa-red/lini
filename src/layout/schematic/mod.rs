//! `layout: schematic` [SPEC 16] — the circuit sheet.
//!
//! Desugar has already lowered the parts ([`crate::desugar::schematic`]):
//! components into pin rails and chrome, discretes and labels into symbol
//! bodies, refs and value readouts into text children. This engine **places**
//! them and hands every wire to the orthogonal router — it never consumes a
//! link and never rewrites a subtree, so it follows the `tree` engine's shape
//! (arrange in place, intercepted before the generic child loop), not the
//! sequence's or the drawing's.
//!
//! Placement does **not** cascade [SPEC 16]: a nested `|row|` / `|grid|` inside
//! a schematic places its own children, exactly as in a drawing. The scope's
//! **laws** do reach them, and reaching is carried, never read back off a path
//! — [`crate::layout::gates`] is the one place that says so on this side of
//! resolve, for this family and every other.

use super::ir::{Bbox, PlacedNode};
use super::{Ctx, child_path, layout_inst, prim, primitives};
use crate::error::Error;
use crate::resolve::{AttrMap, Program, ResolvedInst};
use crate::span::Span;

mod field;
mod hints;
mod junction;
mod lattice;
mod net;
mod pack;
mod place;
mod ports;
mod rail;
mod readout;
mod tag;
mod terminal;

pub(super) use hints::seat_hints;
/// The generated connection dots [SPEC 16.5], read off the routed geometry.
pub(crate) use junction::junctions;
/// The track quantum a schematic world hands the router [SPEC 16.1].
pub(crate) use lattice::quantum;
/// The pass that puts every scope's origin on that quantum, before a wire is
/// asked for [SPEC 16.1].
pub(super) use lattice::snap_scopes;
/// The **net-label convention** [SPEC 16.4] — which side of its wire a net
/// name takes and how far off it sits. Shared with the router's label pass,
/// which places the two-ended spelling (`u7.vs - c24.p1 "VM"`) while
/// [`net::seat_text`] places the minted run.
pub(crate) use net::{
    clear_run, forced_side, is_run as is_net_run, offset as net_offset, text_normal,
};
/// The router's view of a placed part [SPEC 16.5] — the scene index folds a
/// part's anatomy into this one obstacle and reads its fixed ports off it.
pub(crate) use ports::{PartPorts, part_ports};
/// A shaped net tag's outline, drawn once its label is sized [SPEC 16.4].
pub(in crate::layout) use tag::fill as fill_tag;

/// Is this node a schematic scope [SPEC 16]? Detected by its `layout:` attr —
/// the same key the tree / sequence / drawing dispatch reads, so it is
/// intercepted before the generic container path. It lives beside its drawing
/// twin in [`crate::resolve`], because the link pass carries the same reading
/// down the scope chain one stage earlier [SPEC 16.5].
pub(super) use crate::resolve::is_schematic;

/// A `|schematic|` **node** [SPEC 16]: place its children and return the
/// container carrying them.
pub(super) fn layout_node(
    inst: &ResolvedInst,
    path: &str,
    program: &Program,
) -> Result<PlacedNode, Error> {
    let (children, body) = arrange(
        &inst.attrs,
        &inst.children,
        path,
        program,
        inst.span,
        Some(inst.span),
    )?;
    // Border-box, `width`/`height` a floor over the placed sheet [SPEC 5/17] —
    // the one sizing mechanism, the same a `|drawing|` node runs through. A
    // schematic's interior is sheet-space, so it sizes at scale 1 [SPEC 16.6].
    let bbox = primitives::closed_bbox(inst, body, 1.0)?;
    Ok(prim::container(inst, bbox, children))
}

/// A root `{ layout: schematic }` scene [SPEC 16]: the scene itself is the
/// schematic scope. Its wires are ordinary routed links — the caller routes
/// them after, like any scene.
pub(super) fn layout_root(program: &Program) -> Result<(Vec<PlacedNode>, Bbox), Error> {
    let (children, body) = arrange(
        &program.scene.attrs,
        &program.scene.nodes,
        "",
        program,
        Span::empty(),
        None,
    )?;
    let pad = primitives::padding(&program.scene.attrs, Span::empty())?;
    Ok((
        children,
        body.expand(pad.top, pad.right, pad.bottom, pad.left),
    ))
}

/// The placement shared by the node and root entries: lay every child out on
/// its own, then seat them on the scope's anchor track grid [SPEC 16.1] —
/// [`place::arrange`] owns the roles, the tracks and the seats. Returns the
/// placed children and the **content** bbox; each caller sizes its own box
/// around it.
fn arrange(
    attrs: &AttrMap,
    inst_children: &[ResolvedInst],
    path: &str,
    program: &Program,
    span: Span,
    owner: Option<Span>,
) -> Result<(Vec<PlacedNode>, Bbox), Error> {
    // A schematic's interior is sheet-space [SPEC 15.1/16.6] — its chrome is
    // baked in sheet px and never inherits an enclosing drawing's view scale.
    let mut children: Vec<PlacedNode> = Vec::with_capacity(inst_children.len());
    for c in inst_children {
        children.push(layout_inst(c, &child_path(path, c), program, Ctx::sheet())?);
    }
    // The scope's own wires — what the seat pass reads a satellite's chain
    // off [SPEC 16.1]; the engine only *reads* them, the router still draws
    // every one [SPEC 16.7].
    let links: Vec<&crate::resolve::ResolvedLink> =
        crate::layout::scope_links(program, path, owner);
    // Placement centres the sheet on the scope's origin itself, a whole number
    // of fine pitches at a time [SPEC 16.1] — so the box the caller sizes (and
    // the rect it draws) is the sheet's own, and every part is still on the
    // lattice. A `width` floor grows around it [SPEC 5].
    let body = place::arrange(&mut children, attrs, span, &links, path)?;
    Ok((children, body))
}

#[cfg(test)]
mod field_tests;
#[cfg(test)]
mod place_tests;
#[cfg(test)]
mod route_tests;
#[cfg(test)]
mod tests;

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
//! a schematic places its own children, exactly as in a drawing — only the
//! scope's link laws reach nested ordinary scopes, which is what
//! [`is_schematic_scope`] answers.

use super::ir::{Bbox, PlacedNode};
use super::{Ctx, child_path, layout_inst, prim, primitives};
use crate::error::Error;
use crate::resolve::{AttrMap, Program, ResolvedInst, ResolvedValue};
use crate::span::Span;

mod hints;
mod place;
mod ports;
mod seat;
mod terminal;

pub(super) use hints::seat_hints;
/// The router's view of a placed part [SPEC 16.5] — the scene index folds a
/// part's anatomy into this one obstacle and reads its fixed ports off it.
pub(crate) use ports::{PartPorts, part_ports};

/// Is this node a schematic scope [SPEC 16]? Detected by its `layout:` attr —
/// the same key the tree / sequence / drawing dispatch reads, so it is
/// intercepted before the generic container path.
pub(super) fn is_schematic(attrs: &AttrMap) -> bool {
    matches!(attrs.get("layout"), Some(ResolvedValue::Ident(s)) if s == "schematic")
}

/// Whether the scope at `scope` has a schematic **ancestor** — itself, any
/// enclosing container, or the scene root [SPEC 16]. Unlike the sequence's and
/// the drawing's immediate-scope predicates, this one reaches: the schematic
/// link laws apply to a wire written in a nested ordinary container (a `|row|`
/// of parts), even though that container places its own children.
/// The router asks it per link ([`crate::routing::ortho::request`]): a part's
/// pin is a fixed port, and refuses a `:side`, only inside the scope — the
/// family renders anywhere [SPEC 16.7], but the sheet's laws are the scope's.
/// (The rest of the link laws land in Phase 5.)
pub(crate) fn is_schematic_scope(program: &Program, scope: &str) -> bool {
    std::iter::once("")
        .chain(scope.match_indices('.').map(|(i, _)| &scope[..i]))
        .chain(std::iter::once(scope))
        .any(|p| super::scope_attrs(program, p).is_some_and(is_schematic))
}

/// A `|schematic|` **node** [SPEC 16]: place its children and return the
/// container carrying them.
pub(super) fn layout_node(
    inst: &ResolvedInst,
    path: &str,
    program: &Program,
) -> Result<PlacedNode, Error> {
    let (children, bbox) = arrange(&inst.attrs, &inst.children, path, program, inst.span)?;
    Ok(prim::container(inst, bbox, children))
}

/// A root `{ layout: schematic }` scene [SPEC 16]: the scene itself is the
/// schematic scope. Its wires are ordinary routed links — the caller routes
/// them after, like any scene.
pub(super) fn layout_root(program: &Program) -> Result<(Vec<PlacedNode>, Bbox), Error> {
    arrange(
        &program.scene.attrs,
        &program.scene.nodes,
        "",
        program,
        Span::empty(),
    )
}

/// The placement shared by the node and root entries: lay every child out on
/// its own, then seat them on the scope's anchor track grid [SPEC 16.1] —
/// [`place::arrange`] owns the roles, the tracks and the seats. Returns the
/// placed children and the padded bbox.
fn arrange(
    attrs: &AttrMap,
    inst_children: &[ResolvedInst],
    path: &str,
    program: &Program,
    span: Span,
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
        program.links.iter().filter(|w| w.scope == path).collect();
    let body = place::arrange(&mut children, attrs, span, &links, path)?;
    let pad = primitives::padding(attrs, span)?;
    Ok((
        children,
        body.expand(pad.top, pad.right, pad.bottom, pad.left),
    ))
}

#[cfg(test)]
mod place_tests;
#[cfg(test)]
mod route_tests;
#[cfg(test)]
mod seat_tests;
#[cfg(test)]
mod tests;

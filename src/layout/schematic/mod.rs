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
//! — [`check_types`] is the one place that says so on this side of resolve.

use super::ir::{Bbox, PlacedNode};
use super::{Ctx, child_path, layout_inst, prim, primitives};
use crate::error::Error;
use crate::resolve::{AttrMap, Program, ResolvedInst};
use crate::span::Span;

mod hints;
mod junction;
mod place;
mod ports;
mod seat;
mod tag;
mod terminal;

pub(super) use hints::seat_hints;
/// The generated connection dots [SPEC 16.5], read off the routed geometry.
pub(crate) use junction::junctions;
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

/// **The out-of-scope type gate** [SPEC 16/21]: a schematic type belongs in a
/// `layout: schematic`, and this is the one place that says so — swept once
/// over the resolved tree before anything places, beside the sequence's own
/// pre-layout check.
///
/// The scope is **carried down the walk**, not read back off a dot-path: an
/// anonymous container contributes no path segment [SPEC 9], so an anonymous
/// `|schematic|` — or an anonymous part inside one — is invisible to a path
/// predicate and plain to this. Desugar carries the same law the same way
/// ([`crate::desugar::Nest`]), which is what makes the two stages agree.
///
/// Placement still does not cascade — a nested `|row|` runs its own engine —
/// but the *laws* reach it, so `|R|` inside a row inside a sheet is legal.
///
/// **This walk is not sealed, and the statement laws are** — deliberately.
/// A nested `|sequence|` or `|drawing|` stops the *reading of statements*
/// (`desugar::seals_schematic_scope`, `link_scope::statement_owner`),
/// because that engine already owns its body's links: a leader stays a leader,
/// and a pinless landing there is not the sheet's to resolve. **Existence** is
/// a different question: a part is drawn by the family wherever it sits, and
/// what SPEC 21 forbids is a schematic type *outside the scope* — a `|R|`
/// participating in a sequence drawn on a sheet is still on the sheet. Sealing
/// this walk too would make it an error, which is a language change no law
/// asks for.
///
/// A part inside a sealed engine is still a **landing**, for the same reason
/// the sheet's own laws are endpoint-decided: being an addressed part is the
/// proof of scope, so a wire written outside the sheet lands on its pins like
/// any wire (`a_wire_from_outside_lands_on_a_sealed_engines_pin`). What the
/// seal stops is the *reading* of the statement, never the address.
///
/// Everything downstream trusts this gate: past it a schematic part exists
/// only inside a schematic scope, so the router's fixed ports and `:side` ban
/// key on the **part** and never re-ask the scope
/// ([`crate::routing::ortho::request`]).
pub(super) fn check_types(program: &Program) -> Result<(), Error> {
    walk_types(&program.scene.nodes, is_schematic(&program.scene.attrs))
}

fn walk_types(nodes: &[ResolvedInst], schematic: bool) -> Result<(), Error> {
    for n in nodes {
        if !schematic && let Some(ty) = crate::desugar::schematic::schematic_type(&n.type_chain) {
            return Err(
                Error::at(n.span, format!("'|{ty}|' belongs in a 'layout: schematic'"))
                    .code(crate::error::Code::SCHEMATIC_TYPE),
            );
        }
        walk_types(&n.children, schematic || is_schematic(&n.attrs))?;
    }
    Ok(())
}

/// A `|schematic|` **node** [SPEC 16]: place its children and return the
/// container carrying them.
pub(super) fn layout_node(
    inst: &ResolvedInst,
    path: &str,
    program: &Program,
) -> Result<PlacedNode, Error> {
    let (children, body) = arrange(&inst.attrs, &inst.children, path, program, inst.span)?;
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
    // Centre the placed sheet on the scope's origin — the tracks already sit
    // there, but a spanning chain or a flowed-out satellite can hang the body
    // off to one side. The box the caller sizes (and the rect it draws) is then
    // the sheet's own, and a `width` floor grows around it [SPEC 5].
    let (sx, sy) = body.center();
    if (sx, sy) != (0.0, 0.0) {
        for c in children.iter_mut() {
            c.cx -= sx;
            c.cy -= sy;
        }
    }
    Ok((children, body.shifted(-sx, -sy)))
}

#[cfg(test)]
mod place_tests;
#[cfg(test)]
mod route_tests;
#[cfg(test)]
mod seat_tests;
#[cfg(test)]
mod tests;

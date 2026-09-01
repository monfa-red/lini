//! What the field pass **reads** off a placed chain [SPEC 16.1]: the growth
//! ray, the terminator's own drawn convention, and which members are taps.
//!
//! Every answer here is [`crate::desugar::schematic::chain`]'s rule asked over
//! the **placed** tree. The pose chooser asks the very same rules over the
//! authored tree one stage earlier ([`crate::desugar::autopose`]), so a part is
//! never posed for one ray and seated along another.

use super::super::net;
use super::super::place::role;
use super::super::terminal::{Terminal, ident, terminal};
use crate::desugar::pose::Side;
use crate::desugar::schematic::chain::{Chain, End, growth_ray, shared_pin, taps};
use crate::desugar::schematic::{Role, SchKind, sch_kind};
use crate::layout::ir::PlacedNode;

/// Where a one-held chain grows **from** and **toward** [SPEC 16.1]: the pin
/// that holds it, and the ray it runs along — the one shared rule
/// ([`growth_ray`]): the terminator's own convention, yielding to the pin's
/// normal when anti-parallel, and off the straight corridor of a shared pin.
///
/// One home for the ray, because every reader of a chain needs it before
/// anything is seated: the lane order sorts by it, and the walk lays the chain
/// along it.
pub(super) fn growth(
    children: &[PlacedNode],
    chain: &Chain,
    held: &End,
    edges: &[[End; 2]],
) -> (Side, Terminal) {
    let pin = terminal(&children[held.child], held.terminal.as_deref());
    let last = *chain.members.last().expect("a chain has a member");
    let ray = growth_ray(
        tag_facing(
            &children[last],
            chain.inbound.last().and_then(|t| t.as_deref()),
        ),
        pin.facing,
        shared_pin(edges, held, |c| {
            sch_kind(&children[c].type_chain).is_some() && role(&children[c]) == Role::Satellite
        }),
        chain.members.iter().all(|&m| net::is_run(&children[m])),
    );
    (ray, pin)
}

/// The direction a chain's **terminator** sets [SPEC 16.1] — the way its own
/// drawing points. Only a `|label|` carries that convention: a ground is drawn
/// with its point at the top, a power flag with its at the bottom, and the
/// sheet reads the symbol rather than a table of names. A part's pins are just
/// pins, so a part-terminated chain has nothing to say here and its caller
/// falls back to the pin's own normal.
pub(super) fn tag_facing(node: &PlacedNode, inbound: Option<&str>) -> Option<Side> {
    (sch_kind(&node.type_chain) == Some(SchKind::Label))
        .then(|| terminal(node, inbound).facing)
        .flatten()
}

/// Which of a chain's members are **taps** [SPEC 16.1], by this pass's own
/// reading of "symbol label" — one classifier, so the walk and everything that
/// forecasts it cannot disagree.
pub(super) fn tap_flags(children: &[PlacedNode], chain: &Chain) -> Vec<bool> {
    taps(chain, |m| {
        sch_kind(&children[m].type_chain) == Some(SchKind::Label)
            && ident(&children[m].attrs, "symbol").is_some()
    })
}

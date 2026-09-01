//! The seat pass's diagnostics [SPEC 16.1/21] — the two ways a chain's placed
//! ends fail to say where it goes:
//!
//! - **none**: nothing to grow from, so it falls back to the flow;
//! - **more than two**: the distribution runs between two, so every further
//!   end is dropped — unless one part holds them all ([`holder`]), in which case
//!   the chain grows off that part and leaves nothing behind to name.
//!
//! Either way the sheet says so rather than moving a part in silence.
//!
//! A post-layout read of the placed scene, like [`super::super::extent_hints`]
//! — the repo's one channel for a layout warning. It re-runs the *same*
//! [`chains`] the seat pass ran, over the same roles, so the two cannot
//! disagree about which satellites are adrift.

use super::super::ir::PlacedNode;
use super::place::role;
use crate::desugar::schematic::Role;
use crate::desugar::schematic::chain::{End, chains, holder, placed_ends};
use crate::error::Diagnostic;
use crate::layout::ir::LaidOut;
use crate::resolve::Program;

/// One warning per satellite the wires never held.
pub(in crate::layout) fn seat_hints(laid: &LaidOut, program: &Program) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if super::is_schematic(&program.scene.attrs) {
        report(&laid.nodes, "", None, program, &mut out);
    }
    walk(&laid.nodes, "", program, &mut out);
    out
}

fn walk(nodes: &[PlacedNode], prefix: &str, program: &Program, out: &mut Vec<Diagnostic>) {
    for n in nodes {
        // Anonymous containers are scope-transparent [SPEC 9] — a link's
        // scope path skips them, so the walk must too.
        let path = match &n.id {
            Some(id) if prefix.is_empty() => id.clone(),
            Some(id) => format!("{prefix}.{id}"),
            None => prefix.to_string(),
        };
        if super::is_schematic(&n.attrs) {
            report(&n.children, &path, Some(n.span), program, out);
        }
        walk(&n.children, &path, program, out);
    }
}

fn report(
    children: &[PlacedNode],
    scope: &str,
    owner: Option<crate::span::Span>,
    program: &Program,
    out: &mut Vec<Diagnostic>,
) {
    let roles: Vec<Role> = children.iter().map(role).collect();
    let satellite: Vec<bool> = roles.iter().map(|r| *r == Role::Satellite).collect();
    if !satellite.contains(&true) {
        return;
    }
    let links: Vec<&crate::resolve::ResolvedLink> =
        crate::layout::scope_links(program, scope, owner);
    for chain in chains(&satellite, &super::field::edges(children, &links, scope)) {
        // Held is exactly what the seat pass calls held — the same placed-end
        // filter, so a chain cannot flow, or lose an end, silently.
        let held = placed_ends(&chain, &roles);
        let name = |i: usize| {
            let part = &children[i];
            part.id.clone().unwrap_or_else(|| ref_of(part))
        };
        let warn = |out: &mut Vec<Diagnostic>, i: usize, message: String| {
            out.push(
                Diagnostic::warn(children[i].span, message)
                    .code(crate::error::Code::SCHEMATIC_SEAT),
            );
        };
        if held.is_empty() {
            for &member in &chain.members {
                let what = name(member);
                warn(
                    out,
                    member,
                    format!("'{what}' has no placed end — its chain falls back to the flow"),
                );
            }
            continue;
        }
        // A chain distributes between **two** pins [SPEC 16.1] — the first two
        // it is held at, in statement order. A third holds nothing, so the
        // sheet names it rather than drawing a wire to a part seated as if it
        // were not there. (A junction dot marks where such ends *meet*
        // [SPEC 16.5]; it does not seat them — distribution stays two-ended.)
        // …but only a chain that really does distribute can drop one. A chain
        // one anchor holds grows off it instead ([`holder`]) and every end of
        // it reaches that same part, so nothing is left behind to name.
        if holder(&held).is_some() {
            continue;
        }
        for end in held.iter().skip(2) {
            let (pin, chain_name) = (address(children, end), name(chain.members[0]));
            warn(
                out,
                end.child,
                format!(
                    "'{pin}' also holds '{chain_name}' — a chain distributes between two placed ends, so this one is dropped"
                ),
            );
        }
    }
}

/// An end's address as the author wrote it — the part and, when the wire named
/// one, the terminal on it.
fn address(children: &[PlacedNode], end: &End) -> String {
    let part = &children[end.child];
    let head = part.id.clone().unwrap_or_else(|| ref_of(part));
    match &end.terminal {
        Some(t) => format!("{head}.{t}"),
        None => head,
    }
}

/// What to call an **anonymous** part: its minted display ref [SPEC 16.2] —
/// the name the reader sees on the sheet — else its family.
fn ref_of(part: &PlacedNode) -> String {
    part.children
        .iter()
        .find(|c| c.type_chain.iter().any(|t| t == "ref"))
        .and_then(|c| c.children.first())
        .and_then(|t| t.label.clone())
        .or_else(|| part.type_chain.first().map(|t| format!("|{t}|")))
        .unwrap_or_else(|| "a part".into())
}

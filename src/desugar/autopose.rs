//! **Auto-pose** [SPEC 16.1]: a schematic scope's satellites are turned to
//! face the pin they hang off, *before* any of its children lower.
//!
//! **Why here and nowhere else.** A pose is structural — pins re-side, the
//! symbol's `d` and its ports re-lay ([`super::pose`]) — so it can only be
//! applied where that structure is built, which is lowering. `Program` is
//! immutable at layout, and re-laying a placed subtree there would be a
//! second applier. So this pass **decides** and writes the decision as the
//! `rotate:` a user would have written ([`pose::set_rotate`]); lowering
//! applies it through the one path an authored `rotate:` takes. An
//! explicit `rotate:` — the part's own or its define's — is a *forced* pose
//! and is left alone.
//!
//! **The rule.** A satellite chain's placed end is an anchor pin pointing
//! outward along its own side; the satellite sits out there, so the terminal
//! it wires back through must face the **opposite** way. Walk [`Pose::ALL`] —
//! `0 → 90 → 180 → 270`, the unrotated pose then clockwise — and take the
//! first pose that lands the terminal on that side. That order *is* the
//! deterministic tie-break; it only ever bites when a terminal has no facing
//! at all (a symbol-less `|label|`, a port at the box centre), and then no
//! candidate matches and the part stays unrotated.
//!
//! **What this pass cannot see.** It runs on the scope's *authored* children
//! and links, before the rest of lowering builds the scope out, so three
//! constructs are invisible to it and their parts stay unposed:
//!
//! - **capsule-hoisted declarations** (`u1.gnd - |gnd|`) — [`super::capsule`]
//!   hoists the inline part into a child *after* this runs, so the chooser
//!   sees neither the child nor the rewritten endpoint;
//! - **links written inside a `define` body**, which reach the scope only when
//!   the instance expands;
//! - **children a `define` body contributes**, for the same reason.
//!
//! None of this is a seat bug — the layout-side seat pass reads the *resolved*
//! tree and grows those chains correctly; they simply grow unturned. Closing it
//! is a pass reordering (hoist and expand, *then* pose), which Phase 5 owns.

use super::Lower;
use super::pose::{self, Pose, Side};
use super::schematic::{
    self, Role, SchKind,
    chain::{End, placed_ends},
    pins_of,
};
use crate::ast::ChainOp;
use crate::syntax::ast::{Child, Decl, Link, Node};
use std::borrow::Cow;

/// One direct child of the scope, surveyed once: everything the decision
/// reads off the **authored** tree.
struct Part<'a> {
    node: Option<&'a Node>,
    chain: Vec<String>,
    kind: Option<SchKind>,
    symbol: Option<String>,
    /// The part's own authored pose — an anchor's turn re-sides its pins, and
    /// a satellite's *forces* the choice.
    pose: Pose,
    forced: bool,
    role: Role,
}

impl<'a> Part<'a> {
    fn of(cx: &Lower, child: &'a Child) -> Part<'a> {
        let Child::Box(node) = child else {
            // A bare text leaf is no part and seats nowhere.
            return Part {
                node: None,
                chain: Vec::new(),
                kind: None,
                symbol: None,
                pose: Pose::NONE,
                forced: false,
                role: Role::Anchor,
            };
        };
        let chain = cx.authored_chain(node);
        let kind = schematic::sch_kind(&chain);
        let symbol = cx.chain_ident(&chain, &node.style, "symbol");
        let has = |name: &str| cx.chain_decl(&chain, &node.style, name).is_some();
        let pose = cx
            .chain_number(&chain, &node.style, "rotate")
            .and_then(|deg| Pose::from_degrees(deg, node.span).ok())
            .unwrap_or(Pose::NONE);
        let pins = match kind {
            Some(SchKind::Component) => pins_of(cx, node, &chain).len(),
            _ => schematic::part_pin_ids(&chain, symbol.as_deref()).len(),
        };
        Part {
            node: Some(node),
            // `cell:` is the only promoter [SPEC 16.1] — the same argument the
            // engine's `place::role` passes. Counting `translate:` here too
            // would make a nudged satellite an anchor to this pass and a
            // satellite to the seat pass: never posed, and then seated by a
            // growth direction read off an unposed terminal.
            role: schematic::role(has("pin"), has("cell"), kind, pins),
            kind,
            symbol,
            pose,
            forced: has("rotate"),
            chain,
        }
    }

    /// Which way this part's `terminal` points, in the part's **own**
    /// (unposed) frame: a `|component|` pin's side off the bilateral split, a
    /// symbol part's or a `|label|`'s glyph port off the registry.
    fn terminal_side(&self, cx: &Lower, terminal: Option<&str>) -> Option<Side> {
        let node = self.node?;
        match self.kind? {
            SchKind::Component => {
                let want = terminal?;
                let pins = pins_of(cx, node, &self.chain);
                let authored: Vec<Option<Side>> = pins
                    .iter()
                    .map(|p| schematic::authored_side(cx, &cx.authored_chain(p), &p.style))
                    .collect();
                schematic::pin_sides(&authored, Pose::NONE)
                    .into_iter()
                    .zip(&pins)
                    .find(|(_, p)| p.id.as_deref() == Some(want))
                    .map(|((_, side, _), _)| side)
            }
            _ => schematic::terminal_facing(&self.chain, self.symbol.as_deref(), terminal),
        }
    }
}

/// Whether a lowering scope is a schematic — the desugar-side twin of
/// `layout::schematic::is_schematic`. One read of desugar's cascade slice
/// answers every form: `|schematic|` (whose template bundle sets the attr), an
/// explicit `layout: schematic` on any container, and a define that carries
/// one (`{ |sheet::group| { layout: schematic } }`).
pub(super) fn scope_is_schematic(cx: &Lower, chain: &[String], style: &[Decl]) -> bool {
    cx.chain_ident(chain, style, "layout").as_deref() == Some("schematic")
}

/// Pose the satellites of a schematic scope's authored `children` against its
/// `links`. `Cow::Borrowed` whenever nothing is decided, so a non-schematic
/// scope — and a schematic one whose satellites are all forced or unwired —
/// costs nothing.
pub(super) fn choose<'a>(
    cx: &Lower,
    children: &'a [Child],
    links: &[Link],
    schematic: bool,
) -> Cow<'a, [Child]> {
    if !schematic || links.is_empty() {
        // Nothing to face: a scope with no wire seats no satellite.
        return Cow::Borrowed(children);
    }
    let parts: Vec<Part> = children.iter().map(|c| Part::of(cx, c)).collect();
    let roles: Vec<Role> = parts.iter().map(|p| p.role).collect();
    let satellite: Vec<bool> = roles.iter().map(|r| *r == Role::Satellite).collect();
    if !satellite.contains(&true) {
        return Cow::Borrowed(children);
    }
    let index = |path: &[String]| {
        let head = path.first()?;
        parts
            .iter()
            .position(|p| p.node.and_then(|n| n.id.as_deref()) == Some(head.as_str()))
    };

    let mut decided: Vec<(usize, Pose)> = Vec::new();
    for chain in schematic::chains(&satellite, &edges(links, index)) {
        // Only a chain held at a pin has something to face [SPEC 16.1]: with
        // no placed end it falls back to the flow, with two it distributes —
        // neither turns a part. **Placed** ends, through the one filter the
        // seat pass judges by, so the two passes cannot disagree about which
        // chain is held (a `pin:` overlay end holds nothing, either side).
        let ends = placed_ends(&chain, &roles);
        let [anchor] = ends.as_slice() else {
            continue;
        };
        let held = &parts[anchor.child];
        let Some(base) = held.terminal_side(cx, anchor.terminal.as_deref()) else {
            continue;
        };
        // The pin points out along the side it landed on; the satellite sits
        // out there, so it must present its terminal facing back.
        let want = held.pose.side(base).opposite();
        for (&member, inbound) in chain.members.iter().zip(&chain.inbound) {
            let part = &parts[member];
            if part.forced {
                continue;
            }
            let Some(base) = part.terminal_side(cx, inbound.as_deref()) else {
                continue;
            };
            if let Some(pose) = Pose::ALL.into_iter().find(|p| p.side(base) == want) {
                decided.push((member, pose));
            }
        }
    }
    if decided.is_empty() {
        return Cow::Borrowed(children);
    }
    let mut out = children.to_vec();
    for (i, pose) in decided {
        if let Child::Box(node) = &mut out[i] {
            pose::set_rotate(node, pose);
        }
    }
    Cow::Owned(out)
}

/// The scope's wires as chain edges, one per hop, in statement order — every
/// endpoint pair whose two ends both name a direct child of this scope. A
/// measure or a mate joins nothing here; nor does a wire reaching outside the
/// scope (there is no seat to grow from).
fn edges(links: &[Link], index: impl Fn(&[String]) -> Option<usize>) -> Vec<[End; 2]> {
    let mut out = Vec::new();
    for link in links {
        for (hop, ops) in link.chain.windows(2).zip(&link.ops) {
            if !matches!(ops, ChainOp::Wire(_)) {
                continue;
            }
            for a in &hop[0].endpoints {
                for b in &hop[1].endpoints {
                    if let (Some(ai), Some(bi)) = (index(&a.path), index(&b.path)) {
                        out.push([end(ai, &a.path), end(bi, &b.path)]);
                    }
                }
            }
        }
    }
    out
}

fn end(child: usize, path: &[String]) -> End {
    End {
        child,
        // Everything past the direct child is the terminal it names: a pin
        // id, a glyph port id, or nothing at all (`- gnd1`).
        terminal: (path.len() > 1).then(|| path[1..].join(".")),
    }
}

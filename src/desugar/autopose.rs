//! **Auto-pose** [SPEC 16.1]: a schematic scope's satellites are turned to
//! face back up the chain that holds them, *before* any of its children lower.
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
//! **The rule.** A satellite chain runs along a **ray**, and every part on it
//! must present the terminal it wires back through facing *up* that ray. The
//! ray is the one shared rule's ([`schematic::chain::growth_ray`] — what the
//! trunk states: an authored `rotate:`'s turned pin, else the terminator's
//! own drawing; yielding to the pin's normal and to a shared pin's corridor);
//! a **tap** answers to its own convention ([`schematic::chain::tap_ray`]),
//! and a spanning chain's members face back against the sheet's reading
//! direction. Then walk [`Pose::ALL`] — `0 → 90 → 180 → 270`, the unrotated
//! pose then clockwise — and take the first pose that lands the terminal
//! facing as wanted. That order *is* the deterministic tie-break; it only
//! ever bites when a terminal has no facing at all (a symbol-less `|label|`,
//! a port at the box centre), and then no candidate matches and the part
//! stays unrotated.
//!
//! The seat pass reads the same two answers off the lowered tree
//! ([`crate::layout::schematic`]), so a satellite is never posed one way and
//! seated another.
//!
//! **What this pass sees.** It runs over the scope's **gathered** statements
//! ([`super::gather`]) and nothing lowers before it, so every way a part can
//! reach the scope is in hand: its declared children, the children and links a
//! `define` body contributes, the declarations capsule hoisting lifts out
//! (`u1.gnd - |gnd|`), and the tags a label wire mints — each already a child,
//! each wire already rewritten to the id the chooser matches against.
//!
//! What it still declines to turn is a matter of the rule, not of order: a
//! part with an authored `rotate:`, a chain with no placed end, and a
//! terminal with no facing at all (a symbol-less `|label|`).

use super::Lower;
use super::pose::{self, Pose, Side};
use super::schematic::{
    self, Role, SchKind,
    chain::{End, holder, placed_ends},
    pins_of,
};
use crate::ast::ChainOp;
use crate::syntax::ast::{Child, Link, Node};
use std::borrow::Cow;

/// One direct child of the scope, surveyed once: everything the decision
/// reads off the **authored** tree.
struct Part<'a> {
    node: Option<&'a Node>,
    chain: Vec<String>,
    kind: Option<SchKind>,
    symbol: Option<String>,
    /// A `|label|`'s `shape:` — what tells a **net run** from an outlined tag
    /// [SPEC 16.4].
    shape: Option<String>,
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
                shape: None,
                pose: Pose::NONE,
                forced: false,
                role: Role::Anchor,
            };
        };
        let chain = cx.authored_chain(node);
        let kind = schematic::sch_kind(&chain);
        let symbol = cx.chain_ident(&chain, &node.style, "symbol");
        let shape = cx.chain_ident(&chain, &node.style, "shape");
        let has = |name: &str| cx.chain_decl(&chain, &node.style, name).is_some();
        let pose = cx
            .chain_number(&chain, &node.style, "rotate")
            .and_then(|deg| Pose::from_degrees(deg, node.span).ok())
            .unwrap_or(Pose::NONE);
        // The one authored pin reader — the same list the landings resolve
        // against, so a part's arity is one answer at this stage too.
        let pins = schematic::authored_terminal_ids(cx, node, &chain).len();
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
            shape,
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
            _ => schematic::terminal_facing(
                &self.chain,
                self.symbol.as_deref(),
                self.shape.as_deref(),
                terminal,
            ),
        }
    }
}

/// Pose the satellites of a schematic scope's authored `children` against its
/// `links`. `Cow::Borrowed` whenever nothing is decided, so a non-schematic
/// scope — and a schematic one whose satellites are all forced or unwired —
/// costs nothing.
///
/// `schematic` is the **immediate** container's reading
/// ([`super::is_schematic_body`]), never the carrier: a pose is placement, and
/// placement does not cascade [SPEC 16] — a nested `|row|` inside a sheet runs
/// its own engine, so a part it holds has no pin to face and must not turn.
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
    let wire_edges = edges(links, index);
    for chain in schematic::chains(&satellite, &wire_edges) {
        // Only a chain held at a pin has something to face [SPEC 16.1]: with
        // no placed end it falls back to the flow, with two it distributes —
        // neither turns a part. **Placed** ends, through the one filter the
        // seat pass judges by, so the two passes cannot disagree about which
        // chain is held (a `pin:` overlay end holds nothing, either side).
        let ends = placed_ends(&chain, &roles);
        let Some(anchor) = holder(&ends) else {
            // Two anchors — a **spanning** chain [SPEC 16.1]: its members ride
            // the wire's landing leg, marching the sheet's reading direction
            // along the second end's facing axis — rightward into a side pin,
            // downward into a top or bottom one — so every member presents
            // its inbound terminal back against that march. The one static
            // rule that reads the same for a facing pair and for ends whose
            // pins point away (the loop around a connector's own body).
            if let [_, b, ..] = ends.as_slice()
                && let Some(side) = parts[b.child].terminal_side(cx, b.terminal.as_deref())
            {
                let want = if parts[b.child].pose.side(side).is_vertical() {
                    Side::Left
                } else {
                    Side::Top
                };
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
            continue;
        };
        let held = &parts[anchor.child];
        let Some(base) = held.terminal_side(cx, anchor.terminal.as_deref()) else {
            continue;
        };
        // Which way the chain grows [SPEC 16.1] — the one shared rule
        // ([`schematic::chain::growth_ray`]): what the trunk **states**, read
        // from the pin out, yielding to the pin's outward normal when
        // anti-parallel, and off a shared pin's straight corridor. Every
        // member then presents its terminal back up that ray — the same ray
        // the seat pass reads off the lowered tree
        // ([`crate::layout::schematic`]), so the two agree.
        //
        // What a member states here: an authored `rotate:` states its turned
        // pin, a symbol label its own drawing (turned, where forced); every
        // other pose is this pass's to decide, so it states nothing.
        let states = |i: usize| -> Option<Side> {
            let part = &parts[chain.members[i]];
            let side = part.terminal_side(cx, chain.inbound[i].as_deref())?;
            if part.forced {
                return Some(part.pose.side(side));
            }
            (part.kind == Some(SchKind::Label) && part.symbol.is_some()).then_some(side)
        };
        let stated = schematic::chain::stated_facing(schematic::chain::trunk(&chain), states);
        let ray = schematic::chain::growth_ray(
            stated,
            Some(held.pose.side(base)),
            schematic::chain::shared_pin(&wire_edges, anchor, |c| parts[c].role == Role::Satellite),
            chain.members.iter().all(|&m| {
                let p = &parts[m];
                schematic::is_net_run(&p.chain, p.symbol.as_deref(), p.shape.as_deref())
            }),
        );
        // A **tap** — a symbol-label leaf that is not the terminator — hangs
        // along its own drawn convention rather than the trunk's ray
        // ([`schematic::chain::taps`]), so its want is its own
        // ([`schematic::chain::tap_ray`]): a flag above its junction stands
        // unturned, and one whose convention points back into the trunk
        // steps aside and turns to face the junction.
        let tap = schematic::chain::taps(&chain, |m| {
            let p = &parts[m];
            p.kind == Some(SchKind::Label) && p.symbol.is_some()
        });
        let limbs = schematic::chain::limbs(&chain);
        let normal = held.pose.side(base);
        for (i, (&member, inbound)) in chain.members.iter().zip(&chain.inbound).enumerate() {
            let part = &parts[member];
            if part.forced {
                continue;
            }
            let Some(side) = part.terminal_side(cx, inbound.as_deref()) else {
                continue;
            };
            let want = if tap[i] {
                let tr = schematic::chain::tap_ray(Some(side), ray, Some(normal));
                if tr != side.opposite() {
                    // Stepped aside [SPEC 16.1]: the tap keeps its upright
                    // pose — a flag is never laid sideways — and its lead
                    // bends the corner instead (the seat pass raises it).
                    continue;
                }
                tr.opposite()
            } else if let Some(r) = limbs[i] {
                // A **branch** member faces back up its branch's own ray —
                // what its limb states, the trunk's ray when nothing does
                // ([`schematic::chain::limbs`]) — never the trunk's, which
                // may run the other way entirely.
                schematic::chain::stated_facing(schematic::chain::branch(&chain, r), states)
                    .map_or(ray, Side::opposite)
                    .opposite()
            } else {
                ray.opposite()
            };
            if let Some(pose) = Pose::ALL.into_iter().find(|p| p.side(side) == want) {
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

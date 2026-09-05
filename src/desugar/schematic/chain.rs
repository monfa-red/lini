//! Satellite **chains** [SPEC 16.1] — the graph a schematic scope's wires
//! draw over its direct children, reduced to what placement needs: which
//! satellites hang together, which anchor pins hold them, and in what order
//! they grow outward from the placed end.
//!
//! Neutral on purpose. The pose chooser runs it over the **authored** tree at
//! desugar (to learn which way each satellite must face) and the engine over
//! the **placed** tree at layout (to seat them); both hand it the same thing —
//! child indices, terminal names, and whether each child is a satellite — so
//! the two passes cannot disagree about what a chain is.

use super::super::pose::Side;
use super::Role;
use std::collections::VecDeque;

/// The ray a one-held chain grows along [SPEC 16.1] — **the** rule, shared by
/// the pose chooser (authored tree) and the seat pass (placed tree), so a
/// part is never posed for one ray and seated along another:
///
/// - a member that **states** a facing decides ([`stated_facing`]): the ray
///   is the opposite of it, since every member presents its inbound
///   terminal back up the ray — a `|gnd|`'s point is at its top, so its
///   chain grows down; a power flag's at its bottom, so up; a part turned by
///   an authored `rotate:` the way its turned pin points — *unless* honouring
///   it would grow the chain back **through** the part it hangs from (a gnd
///   off a `side: top` pin): an anti-parallel ray yields to the pin's outward
///   normal, and the terminator poses upside-down instead, as a sheet flips a
///   ground above a part;
/// - with nothing stated the chain runs straight out along the pin's normal —
///   *unless* the pin is **shared**: the straight corridor of a shared pin is
///   the trunk line of every sibling wire, so a seat there stands on all of
///   them. The chain yields it exactly as chains yield each other's lanes,
///   turning onto the sheet's roomy axis — down off a side pin, rightward off
///   a top or bottom one. A **bodiless** chain — every member a net run —
///   never yields: a run is a stretch of the corridor's own wire with a name
///   over it, so it rides the trunk it would otherwise dodge, and the sibling
///   wires merge along it.
pub(crate) fn growth_ray(
    stated: Option<Side>,
    pin_facing: Option<Side>,
    shared_pin: bool,
    bodiless: bool,
) -> Side {
    // A terminal with no facing at all still hangs below — the one
    // direction a sheet always has room in [SPEC 16.1].
    let normal = pin_facing.unwrap_or(Side::Bottom);
    if let Some(f) = stated
        && f.opposite() != normal.opposite()
    {
        return f.opposite();
    }
    if !shared_pin || bodiless {
        return normal;
    }
    match normal {
        Side::Left | Side::Right => Side::Bottom,
        Side::Top | Side::Bottom => Side::Right,
    }
}

/// The facing a run of members **states** [SPEC 16.1]: the first of
/// `members`, walked outward from where the run grows, for which `states`
/// answers — the way that member's inbound terminal points. A chain's trunk
/// asks it from the pin, a branch from its junction ([`limbs`]).
///
/// What a member states is the caller's reading: the pose chooser hears an
/// authored `rotate:` (the turned pin) and a symbol label's own drawing, and
/// nothing else, since every other pose is the one it is about to decide;
/// the seat pass, over the lowered tree, hears every posed terminal — the
/// chooser turned the undecided ones to face the ray it found, so the first
/// answer is that ray again.
pub(crate) fn stated_facing(
    members: impl IntoIterator<Item = usize>,
    states: impl Fn(usize) -> Option<Side>,
) -> Option<Side> {
    members.into_iter().find_map(states)
}

/// The member indices on a chain's **trunk** [SPEC 16.1], from the held end
/// out to the terminator — the run [`stated_facing`] walks for the growth
/// ray.
pub(crate) fn trunk(chain: &Chain) -> Vec<usize> {
    let limbs = limbs(chain);
    (0..chain.members.len())
        .filter(|&i| limbs[i].is_none())
        .collect()
}

/// The member indices of the **branch** rooted at `root` [SPEC 16.1], from
/// the junction out — the run its own ray is stated over.
pub(crate) fn branch(chain: &Chain, root: usize) -> Vec<usize> {
    let limbs = limbs(chain);
    (0..chain.members.len())
        .filter(|&i| limbs[i] == Some(root))
        .collect()
}

/// Whether `end`'s pin carries **through traffic** — a wire to another
/// placed part, which runs down the pin's straight corridor. That is the
/// sharing [`growth_ray`] yields to; a sibling satellite chain turns off
/// into a lane of its own and claims no corridor. Counted over the same
/// `edges` both callers walk, with each caller's own satellite reading.
pub(crate) fn shared_pin(edges: &[[End; 2]], end: &End, satellite: impl Fn(usize) -> bool) -> bool {
    edges
        .iter()
        .any(|[a, b]| (a == end && !satellite(b.child)) || (b == end && !satellite(a.child)))
}

/// One end of a wire: the direct child it lands on, and the terminal within
/// it the endpoint named (`None` — a bare `- gnd1` — is the part itself).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct End {
    pub child: usize,
    pub terminal: Option<String>,
}

/// A run of satellites wired together, with the placed ends holding it.
#[derive(Debug, PartialEq)]
pub(crate) struct Chain {
    /// The satellites, **outward from the first placed end** — breadth-first
    /// in statement order, so the growth order is the reading order.
    pub members: Vec<usize>,
    /// The terminal each member faces back through, toward the placed end
    /// (`members[i]`'s inbound terminal is `inbound[i]`).
    pub inbound: Vec<Option<String>>,
    /// The member each was discovered **from** — its attachment up the walk
    /// (`None` for the first). What lets a branching chain tell its trunk
    /// from a tap hanging off it ([`taps`]).
    pub parents: Vec<Option<usize>>,
    /// The chain's non-satellite ends, in statement order. **Not** the placed
    /// ends: a `pin:` overlay is no satellite and lands here too, so every
    /// reader asks [`placed_ends`] rather than counting these.
    pub anchors: Vec<End>,
}

/// Which members are **taps** [SPEC 16.1]: a single symbol-label leaf hanging
/// off a mid-chain member — the rail flag or ground a sheet stands beside a
/// junction — everything that is *not* the trunk's own terminator (the last
/// member). A tap seats off its attachment member along its own drawn
/// convention rather than taking a slot in the trunk's stack, where a BFS
/// linearization stood the buck's 5 V flag upside-down between the inductor
/// and the feedback divider. A tap is the one-member case of a **branch**
/// ([`limbs`]); the multi-member ones grow their own sub-chains.
///
/// One classifier for the pose chooser and the seat pass; each supplies its
/// own reading of "this member is a symbol label".
pub(crate) fn taps(chain: &Chain, symbol_label: impl Fn(usize) -> bool) -> Vec<bool> {
    let n = chain.members.len();
    let mut leaf = vec![true; n];
    for p in chain.parents.iter().flatten() {
        leaf[*p] = false;
    }
    let limbs = limbs(chain);
    (0..n)
        .map(|i| limbs[i] == Some(i) && leaf[i] && symbol_label(chain.members[i]))
        .collect()
}

/// A chain's **limb decomposition** [SPEC 16.1]: per member, `None` on the
/// **trunk** — the walk from the held end to the chain's terminator (the
/// BFS-last member, whose convention sets the growth ray) — or `Some(root)`,
/// naming the off-trunk subtree it belongs to by the **branch**'s root
/// member index. A branch hangs off a trunk member at a junction and grows
/// its own way from there: a one-member symbol branch is a tap ([`taps`]),
/// anything larger a sub-chain along its own ray. Shared by the pose
/// chooser and the seat pass, so a branch is never posed for one ray and
/// seated along another.
pub(crate) fn limbs(chain: &Chain) -> Vec<Option<usize>> {
    let n = chain.members.len();
    let mut on_trunk = vec![false; n];
    let mut cur = n.checked_sub(1);
    while let Some(i) = cur {
        on_trunk[i] = true;
        cur = chain.parents[i];
    }
    (0..n)
        .map(|i| {
            if on_trunk[i] {
                return None;
            }
            let mut r = i;
            while let Some(p) = chain.parents[r] {
                if on_trunk[p] {
                    break;
                }
                r = p;
            }
            Some(r)
        })
        .collect()
}

/// The ray a **tap** hangs along [SPEC 16.1]: its own drawn convention — a
/// power flag stands above its junction, a ground below — unless that points
/// back into the trunk it hangs off, in which case it steps aside: out along
/// the pin's own normal where the trunk turned off it, else onto the fixed
/// side rank's first free direction. Shared by the pose chooser and the seat
/// pass like [`growth_ray`].
pub(crate) fn tap_ray(natural: Option<Side>, trunk: Side, pin_facing: Option<Side>) -> Side {
    let natural = match natural {
        Some(f) => f.opposite(),
        None => return trunk,
    };
    if natural != trunk.opposite() {
        return natural;
    }
    beside(trunk, pin_facing)
}

/// The sideways direction **beside** a trunk [SPEC 16.1] — where a
/// conflicted tap steps and a trunk-axis branch lays its lane: out along
/// the pin's own normal where the trunk turned off it, else onto the fixed
/// side rank's first free direction.
pub(crate) fn beside(trunk: Side, pin_facing: Option<Side>) -> Side {
    match pin_facing {
        Some(p) if p != trunk && p != trunk.opposite() => p,
        _ if matches!(trunk, Side::Top | Side::Bottom) => Side::Right,
        _ => Side::Bottom,
    }
}

/// A chain's **placed ends** [SPEC 16.1]: the ends that actually ride a track.
/// A `pin:` overlay is sheet chrome seated on the *finished* scope box, so it
/// has no position while the satellites seat and holds nothing.
///
/// **The one filter.** Both passes that decide off a chain's ends ask this and
/// nothing else — the pose chooser over the authored tree
/// ([`crate::desugar::autopose`]) and the seat pass over the placed one
/// ([`crate::layout::schematic`], warning included) — so a sheet can never pose
/// a part the seat pass then flows, nor flow one it silently posed.
pub(crate) fn placed_ends(chain: &Chain, roles: &[Role]) -> Vec<End> {
    chain
        .anchors
        .iter()
        .filter(|e| roles[e.child] == Role::Anchor)
        .cloned()
        .collect()
}

/// The one end a chain **grows from** [SPEC 16.1], if any: its lone placed
/// end — or, when every placed end is a terminal of the *same* part, that
/// part's first.
///
/// **Two ends on one anchor are a fan, not a span.** `u1.a & u1.b - r1` (and a
/// chain that loops back to its own component) names a line running down that
/// anchor's own side, so distributing along it seats the satellite *inside* the
/// anchor — and no track pair can ever widen it, because both pins ride one
/// track. It hangs off the anchor like any one-end chain instead, and the
/// router fans the remaining wires onto the shared landing [SPEC 16.5].
///
/// **The one rule**, beside [`placed_ends`] and asked by both passes that
/// decide off a chain's ends — the pose chooser and the seat pass — because a
/// part the seat pass grows is a part the chooser must turn to face.
pub(crate) fn holder(ends: &[End]) -> Option<&End> {
    match ends {
        [one] => Some(one),
        [first, rest @ ..] if !rest.is_empty() && rest.iter().all(|e| e.child == first.child) => {
            Some(first)
        }
        _ => None,
    }
}

/// Every satellite chain in a scope. `satellite[i]` classifies child `i`
/// (see [`super::role`]); `edges` are the scope's wires, one per hop, in
/// statement order. Deterministic throughout: components are discovered in
/// child declaration order and walked in edge order.
pub(crate) fn chains(satellite: &[bool], edges: &[[End; 2]]) -> Vec<Chain> {
    let count = satellite.len();
    // Incident (edge, my end) per child, in statement order.
    let mut incident: Vec<Vec<(usize, usize)>> = vec![Vec::new(); count];
    for (e, ends) in edges.iter().enumerate() {
        if ends[0].child == ends[1].child {
            continue; // a self-loop joins nothing
        }
        for (k, end) in ends.iter().enumerate() {
            incident[end.child].push((e, k));
        }
    }

    let mut seen = vec![false; count];
    let mut out = Vec::new();
    for start in 0..count {
        if !satellite[start] || seen[start] {
            continue;
        }
        // The connected component of satellites, so the placed ends can be
        // gathered before the outward walk knows where to start.
        let mut members = vec![start];
        seen[start] = true;
        let mut i = 0;
        while i < members.len() {
            let c = members[i];
            i += 1;
            for &(e, k) in &incident[c] {
                let other = edges[e][1 - k].child;
                if satellite[other] && !seen[other] {
                    seen[other] = true;
                    members.push(other);
                }
            }
        }
        let anchors: Vec<End> = edges
            .iter()
            .filter_map(|ends| {
                let placed = |k: usize| {
                    (members.contains(&ends[1 - k].child) && !satellite[ends[k].child])
                        .then(|| ends[k].clone())
                };
                placed(0).or_else(|| placed(1))
            })
            .collect();

        // The outward walk: from the satellite the first placed end holds
        // (or, with none, from the first-declared member), breadth-first.
        let seed = anchors
            .first()
            .and_then(|a| {
                edges.iter().find_map(|ends| {
                    let sat = |k: usize| {
                        (ends[k] == *a && members.contains(&ends[1 - k].child))
                            .then(|| ends[1 - k].clone())
                    };
                    sat(0).or_else(|| sat(1))
                })
            })
            .unwrap_or(End {
                child: start,
                terminal: None,
            });
        let mut walked = vec![false; count];
        let mut order: Vec<(End, Option<usize>)> = Vec::new();
        let mut queue: VecDeque<(End, Option<usize>)> = VecDeque::from([(seed, None)]);
        while let Some((end, from)) = queue.pop_front() {
            if std::mem::replace(&mut walked[end.child], true) {
                continue;
            }
            let me = order.len();
            for &(e, k) in &incident[end.child] {
                let next = &edges[e][1 - k];
                if satellite[next.child] && !walked[next.child] {
                    queue.push_back((next.clone(), Some(me)));
                }
            }
            order.push((end, from));
        }
        out.push(Chain {
            members: order.iter().map(|(e, _)| e.child).collect(),
            inbound: order.iter().map(|(e, _)| e.terminal.clone()).collect(),
            parents: order.into_iter().map(|(_, from)| from).collect(),
            anchors,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn end(child: usize, terminal: Option<&str>) -> End {
        End {
            child,
            terminal: terminal.map(str::to_string),
        }
    }

    #[test]
    fn a_chain_walks_outward_from_its_placed_end() {
        // 0 = an anchor; 1, 2 satellites: `u1.a - r1.p1; r1.p2 - g1`.
        let edges = [
            [end(0, Some("a")), end(1, Some("p1"))],
            [end(1, Some("p2")), end(2, None)],
        ];
        let got = chains(&[false, true, true], &edges);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].members, vec![1, 2], "outward, not declaration order");
        assert_eq!(
            got[0].inbound,
            vec![Some("p1".into()), None],
            "each member's terminal facing back"
        );
        assert_eq!(got[0].anchors, vec![end(0, Some("a"))]);
    }

    #[test]
    fn the_walk_starts_at_the_placed_end_whatever_the_declaration_order() {
        // The same chain written from the far end: `g1 - r1.p2; r1.p1 - u1.a`.
        let edges = [
            [end(2, None), end(1, Some("p2"))],
            [end(1, Some("p1")), end(0, Some("a"))],
        ];
        let got = chains(&[false, true, true], &edges);
        assert_eq!(got[0].members, vec![1, 2]);
        assert_eq!(got[0].inbound, vec![Some("p1".into()), None]);
    }

    #[test]
    fn two_placed_ends_and_none_are_both_reported() {
        let spanning = chains(
            &[false, true, false],
            &[
                [end(0, Some("a")), end(1, Some("p1"))],
                [end(1, Some("p2")), end(2, Some("b"))],
            ],
        );
        assert_eq!(spanning[0].anchors.len(), 2);
        let floating = chains(&[true, true], &[[end(0, Some("p2")), end(1, None)]]);
        assert!(floating[0].anchors.is_empty());
        assert_eq!(floating[0].members, vec![0, 1]);
    }

    #[test]
    fn each_chain_on_a_pin_is_its_own() {
        // Two chains off one pin stay two chains — they stack, never merge.
        let got = chains(
            &[false, true, true],
            &[
                [end(0, Some("a")), end(1, None)],
                [end(0, Some("a")), end(2, None)],
            ],
        );
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].members, vec![1]);
        assert_eq!(got[1].members, vec![2]);
    }
}

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

use super::Role;
use std::collections::VecDeque;

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
    /// The chain's non-satellite ends, in statement order. **Not** the placed
    /// ends: a `pin:` overlay is no satellite and lands here too, so every
    /// reader asks [`placed_ends`] rather than counting these.
    pub anchors: Vec<End>,
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
        let mut order: Vec<End> = Vec::new();
        let mut queue: VecDeque<End> = VecDeque::from([seed]);
        while let Some(end) = queue.pop_front() {
            if std::mem::replace(&mut walked[end.child], true) {
                continue;
            }
            for &(e, k) in &incident[end.child] {
                let next = &edges[e][1 - k];
                if satellite[next.child] && !walked[next.child] {
                    queue.push_back(next.clone());
                }
            }
            order.push(end);
        }
        out.push(Chain {
            members: order.iter().map(|e| e.child).collect(),
            inbound: order.into_iter().map(|e| e.terminal).collect(),
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

//! The natural strategy's sides and ports (ROUTING.md The natural strategy,
//! model steps 1–2), decided for every request **before any curve exists**,
//! so each wire then fits independently. A forced side wins (trees and
//! mindmaps stamp theirs; self-loops resolve through the shared
//! [`self_loop_sides`]); otherwise an end takes the side that most faces the
//! other end — a fan, the side facing its members' mean. Per node side,
//! every landing prefers its far end's centre projected onto the side and
//! the side's landings spread at ≥ pitch by the same bounded [`ladder`]
//! placement uses, inside the port window (the side minus `clearance`
//! corner margins, its centre point when too short).

use super::curve::Pt;
use crate::ast::Side;
use crate::resolve::Strategy;
use crate::routing::ortho::ladder::ladder;
use crate::routing::ortho::rect::Rect;
use crate::routing::ortho::request::{EdgeReq, End, Fans};
use crate::routing::ortho::scene::SceneIndex;
use crate::routing::ortho::self_loop_sides;

/// One resolved end: the exact port point on the side line and the leave
/// direction (the side's outward normal — inward for a containment end,
/// which lands on its parent's inner face).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Landing {
    pub port: Pt,
    pub normal: Pt,
}

/// The tie rank every routing tie breaks on (ROUTING.md Law 4).
const RANK: [Side; 4] = [Side::Right, Side::Bottom, Side::Left, Side::Top];

fn normal(side: Side) -> Pt {
    match side {
        Side::Right => (1.0, 0.0),
        Side::Bottom => (0.0, 1.0),
        Side::Left => (-1.0, 0.0),
        Side::Top => (0.0, -1.0),
    }
}

fn centre(r: Rect) -> Pt {
    ((r.x0 + r.x1) / 2.0, (r.y0 + r.y1) / 2.0)
}

/// The lawful port window on a side — the side span minus `clearance`
/// corner margins, collapsing to the centre point when the side is too
/// short (the entry.rs window shape, graph-free).
fn window(rect: Rect, side: Side, c: f64) -> (f64, f64) {
    let (lo, hi) = match side {
        Side::Left | Side::Right => (rect.y0, rect.y1),
        Side::Top | Side::Bottom => (rect.x0, rect.x1),
    };
    if hi - lo < 2.0 * c {
        let mid = (lo + hi) / 2.0;
        (mid, mid)
    } else {
        (lo + c, hi - c)
    }
}

/// A port point from a side and its ordinate across the side.
fn port_at(rect: Rect, side: Side, ord: f64) -> Pt {
    match side {
        Side::Right => (rect.x1, ord),
        Side::Left => (rect.x0, ord),
        Side::Top => (ord, rect.y0),
        Side::Bottom => (ord, rect.y1),
    }
}

/// The side most facing `toward` from `rect`'s centre — greatest dot of the
/// outward normal against the chord; ties break on the side rank. An inward
/// (containment) end scores the same way: the side nearest the inner node is
/// the one whose inner face sees it.
fn facing_side(rect: Rect, toward: Pt) -> Side {
    let c = centre(rect);
    let chord = (toward.0 - c.0, toward.1 - c.1);
    let mut best = RANK[0];
    let mut score = f64::NEG_INFINITY;
    for side in RANK {
        let n = normal(side);
        let s = n.0 * chord.0 + n.1 * chord.1;
        if s.total_cmp(&score) == std::cmp::Ordering::Greater {
            (best, score) = (side, s);
        }
    }
    best
}

/// A far centre projected onto a side's ordinate axis.
fn pref_of(side: Side, far: Pt) -> f64 {
    match side {
        Side::Left | Side::Right => far.1,
        Side::Top | Side::Bottom => far.0,
    }
}

/// One landing slot on a node side — a fan group (one shared port) or a
/// single request end.
struct Slot {
    path: String,
    rect: Rect,
    /// Far end centres, one per member; preferences read their mean.
    fars: Vec<Pt>,
    forced: Option<Side>,
    inward: bool,
    decl: usize,
    members: Vec<(usize, End)>,
    side: Side,
}

impl Slot {
    fn mean_far(&self) -> Pt {
        let n = self.fars.len() as f64;
        let (sx, sy) = self
            .fars
            .iter()
            .fold((0.0, 0.0), |(x, y), f| (x + f.0, y + f.1));
        (sx / n, sy / n)
    }
}

/// Sides then ports for every natural request end. `out[i]` is `None` for
/// non-natural requests.
pub(crate) fn landings(
    index: &SceneIndex,
    reqs: &[EdgeReq],
    fans: &Fans,
    c: f64,
) -> Vec<Option<[Landing; 2]>> {
    let mut slots: Vec<Slot> = Vec::new();
    let mut slot_of: Vec<[usize; 2]> = vec![[usize::MAX; 2]; reqs.len()];
    let mut fan_slot: Vec<Option<usize>> = vec![None; fans.groups.len()];

    for (i, req) in reqs.iter().enumerate() {
        if req.routing != Strategy::Natural {
            continue;
        }
        let self_loop = req.a_path == req.b_path;
        // Both ends forced onto one side stay as written: natural draws the
        // same-side loop rather than straying (it has no strays at all).
        let loop_sides = self_loop.then(|| {
            self_loop_sides(req.side_a, req.side_b).unwrap_or_else(|| {
                (
                    req.side_a.expect("equal forced sides"),
                    req.side_b.expect("equal forced sides"),
                )
            })
        });
        for (end, path, rect, far_rect, forced) in [
            (
                End::A,
                &req.a_path,
                req.a_rect,
                req.b_rect,
                loop_sides.map_or(req.side_a, |(s, _)| Some(s)),
            ),
            (
                End::B,
                &req.b_path,
                req.b_rect,
                req.a_rect,
                loop_sides.map_or(req.side_b, |(_, s)| Some(s)),
            ),
        ] {
            let far = centre(far_rect);
            let fan = if self_loop {
                None
            } else {
                fans.group_at(i, end)
            };
            if let Some(s) = fan.and_then(|g| fan_slot[g]) {
                slots[s].fars.push(far);
                slots[s].members.push((i, end));
                slot_of[i][end as usize] = s;
                continue;
            }
            let other = if end == End::A {
                &req.b_path
            } else {
                &req.a_path
            };
            slots.push(Slot {
                path: path.clone(),
                rect,
                fars: vec![far],
                forced,
                inward: !self_loop && index.geo_contains(path, other),
                decl: i,
                members: vec![(i, end)],
                side: Side::Right,
            });
            if let Some(g) = fan {
                fan_slot[g] = Some(slots.len() - 1);
            }
            slot_of[i][end as usize] = slots.len() - 1;
        }
    }

    // Sides, once every fan slot knows all its members (its facing reads
    // the members' mean).
    for slot in &mut slots {
        slot.side = slot
            .forced
            .unwrap_or_else(|| facing_side(slot.rect, slot.mean_far()));
    }

    // Spread each node side's slots at pitch inside the window, ordered by
    // preference (where the far ends lie) then declaration — no braiding at
    // the mouth.
    let mut ords: Vec<f64> = vec![0.0; slots.len()];
    let mut keys: Vec<(String, u8)> = slots
        .iter()
        .map(|s| (s.path.clone(), s.side.index()))
        .collect();
    keys.sort();
    keys.dedup();
    for key in keys {
        let mut group: Vec<usize> = (0..slots.len())
            .filter(|&s| (slots[s].path.clone(), slots[s].side.index()) == key)
            .collect();
        group.sort_by(|&a, &b| {
            let (sa, sb) = (&slots[a], &slots[b]);
            pref_of(sa.side, sa.mean_far())
                .total_cmp(&pref_of(sb.side, sb.mean_far()))
                .then(sa.decl.cmp(&sb.decl))
        });
        let first = &slots[group[0]];
        let win = window(first.rect, first.side, c);
        let n = group.len();
        let pitch = if n > 1 {
            c.min((win.1 - win.0) / (n - 1) as f64)
        } else {
            c
        };
        let prefs: Vec<f64> = group
            .iter()
            .map(|&s| pref_of(slots[s].side, slots[s].mean_far()))
            .collect();
        let bounds: Vec<(f64, f64)> = vec![win; n];
        let seps = vec![pitch; n.saturating_sub(1)];
        for (&s, ord) in group.iter().zip(ladder(&prefs, &bounds, &seps)) {
            ords[s] = ord;
        }
    }

    reqs.iter()
        .enumerate()
        .map(|(i, req)| {
            (req.routing == Strategy::Natural).then(|| {
                [End::A, End::B].map(|end| {
                    let slot = &slots[slot_of[i][end as usize]];
                    let n = normal(slot.side);
                    Landing {
                        port: port_at(slot.rect, slot.side, ords[slot_of[i][end as usize]]),
                        normal: if slot.inward { (-n.0, -n.1) } else { n },
                    }
                })
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::{AttrMap, Markers};
    use crate::span::Span;

    fn req(a: &str, ra: Rect, b: &str, rb: Rect) -> EdgeReq {
        EdgeReq {
            a_path: a.to_owned(),
            b_path: b.to_owned(),
            a_rect: ra,
            b_rect: rb,
            side_a: None,
            side_b: None,
            routing: Strategy::Natural,
            clearance: 12.0,
            stub_a: 12.0,
            stub_b: 12.0,
            markers: Markers::default(),
            attrs: AttrMap::default(),
            applied_styles: Vec::new(),
            span: Span::empty(),
            data_from: a.to_owned(),
            data_to: b.to_owned(),
            stmt: 0,
            seg: 0,
            expansion: 0,
        }
    }

    fn index_of(nodes: &[(&str, Rect)]) -> SceneIndex {
        use crate::layout::ir::{Bbox, PlacedNode};
        use crate::resolve::NodeKind;
        let placed: Vec<PlacedNode> = nodes
            .iter()
            .map(|(id, r)| PlacedNode {
                id: Some((*id).to_owned()),
                kind: NodeKind::Block,
                type_chain: Vec::new(),
                applied_styles: Vec::new(),
                label: None,
                attrs: AttrMap::default(),
                own_style: AttrMap::default(),
                markers: Markers::default(),
                cx: (r.x0 + r.x1) / 2.0,
                cy: (r.y0 + r.y1) / 2.0,
                bbox: Bbox::centered(r.x1 - r.x0, r.y1 - r.y0),
                rotation: 0.0,
                children: Vec::new(),
                gutters: Vec::new(),
                links: Vec::new(),
                sketch: None,
                origin: (0.0, 0.0),
                span: Span::empty(),
            })
            .collect();
        SceneIndex::build(&placed)
    }

    fn fans_of(reqs: &[EdgeReq]) -> Fans {
        crate::routing::ortho::request::fan_groups(reqs, Strategy::Natural)
    }

    #[test]
    fn an_aligned_facing_pair_lands_dead_centre_on_facing_sides() {
        let (ra, rb) = (
            Rect::new(0.0, 0.0, 40.0, 40.0),
            Rect::new(100.0, 0.0, 140.0, 40.0),
        );
        let reqs = vec![req("a", ra, "b", rb)];
        let idx = index_of(&[("a", ra), ("b", rb)]);
        let [la, lb] = landings(&idx, &reqs, &fans_of(&reqs), 12.0)[0].expect("natural");
        assert_eq!(la.port, (40.0, 20.0));
        assert_eq!(lb.port, (100.0, 20.0));
        assert_eq!(la.normal, (1.0, 0.0), "lands on the right side");
        assert_eq!(lb.normal, (-1.0, 0.0), "lands on the left side");
    }

    #[test]
    fn duplicates_ladder_at_pitch_on_both_sides_without_braiding() {
        let (ra, rb) = (
            Rect::new(0.0, 0.0, 40.0, 100.0),
            Rect::new(120.0, 0.0, 160.0, 100.0),
        );
        let mut r2 = req("a", ra, "b", rb);
        r2.stmt = 1;
        let reqs = vec![req("a", ra, "b", rb), r2];
        let idx = index_of(&[("a", ra), ("b", rb)]);
        let l = landings(&idx, &reqs, &fans_of(&reqs), 12.0);
        let [a1, b1] = l[0].expect("natural");
        let [a2, b2] = l[1].expect("natural");
        // Rails at pitch = clearance, centred on the shared preference, the
        // same order on both sides — translates, never a braid.
        assert!((a2.port.1 - a1.port.1 - 12.0).abs() < 1e-9);
        assert!((b2.port.1 - b1.port.1 - 12.0).abs() < 1e-9);
        assert!(((a1.port.1 + a2.port.1) / 2.0 - 50.0).abs() < 1e-9);
    }

    #[test]
    fn a_fan_shares_one_port_at_the_members_mean() {
        let ra = Rect::new(0.0, 40.0, 40.0, 80.0);
        let rb = Rect::new(120.0, 0.0, 160.0, 40.0);
        let rc = Rect::new(120.0, 80.0, 160.0, 120.0);
        // a -> b & c: one statement, two expansions.
        let mut r2 = req("a", ra, "c", rc);
        r2.expansion = 1;
        let reqs = vec![req("a", ra, "b", rb), r2];
        let idx = index_of(&[("a", ra), ("b", rb), ("c", rc)]);
        let fans = fans_of(&reqs);
        assert_eq!(fans.groups.len(), 1, "the shared end fans");
        let l = landings(&idx, &reqs, &fans, 12.0);
        let [a1, _] = l[0].expect("natural");
        let [a2, _] = l[1].expect("natural");
        assert_eq!(a1.port, a2.port, "one shared trunk port");
        // The far centres (y 20 and 100) mean to the side centre here.
        assert_eq!(a1.port, (40.0, 60.0));
    }

    #[test]
    fn a_forced_side_wins_over_facing() {
        let (ra, rb) = (
            Rect::new(0.0, 0.0, 40.0, 40.0),
            Rect::new(100.0, 0.0, 140.0, 40.0),
        );
        let mut r = req("a", ra, "b", rb);
        r.side_a = Some(Side::Top);
        let reqs = vec![r];
        let idx = index_of(&[("a", ra), ("b", rb)]);
        let [la, _] = landings(&idx, &reqs, &fans_of(&reqs), 12.0)[0].expect("natural");
        assert_eq!(la.normal, (0.0, -1.0), "the forced top side wins");
        assert_eq!(la.port.1, 0.0, "the port sits on the top side line");
    }

    #[test]
    fn a_short_side_collapses_its_window_to_the_centre() {
        let (ra, rb) = (
            Rect::new(0.0, 0.0, 40.0, 20.0),
            Rect::new(100.0, 100.0, 140.0, 120.0),
        );
        let reqs = vec![req("a", ra, "b", rb)];
        let idx = index_of(&[("a", ra), ("b", rb)]);
        let [la, _] = landings(&idx, &reqs, &fans_of(&reqs), 12.0)[0].expect("natural");
        // The far end pulls down-right; a 20-high side has no window slack,
        // so the right side's port stays at its centre.
        assert_eq!(la.port, (40.0, 10.0));
    }

    #[test]
    fn a_self_loop_defaults_right_then_top() {
        let ra = Rect::new(0.0, 0.0, 60.0, 60.0);
        let reqs = vec![req("a", ra, "a", ra)];
        let idx = index_of(&[("a", ra)]);
        let [la, lb] = landings(&idx, &reqs, &fans_of(&reqs), 12.0)[0].expect("natural");
        assert_eq!(la.normal, (1.0, 0.0), "out the right side");
        assert_eq!(lb.normal, (0.0, -1.0), "back in the top side");
    }

    #[test]
    fn landings_are_deterministic() {
        let (ra, rb) = (
            Rect::new(0.0, 0.0, 40.0, 100.0),
            Rect::new(120.0, 30.0, 160.0, 130.0),
        );
        let mut r2 = req("b", rb, "a", ra);
        r2.stmt = 1;
        let reqs = vec![req("a", ra, "b", rb), r2];
        let idx = index_of(&[("a", ra), ("b", rb)]);
        let once = format!("{:?}", landings(&idx, &reqs, &fans_of(&reqs), 12.0));
        let twice = format!("{:?}", landings(&idx, &reqs, &fans_of(&reqs), 12.0));
        assert_eq!(once, twice);
    }
}

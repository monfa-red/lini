//! Layout feedback for gap growth (PLAN Phase 8, WIRING §Impossible
//! layouts): why did a wire stay impossible, and which corridors are short?
//!
//! After every lever has run, each reported-impossible bundle is probed —
//! one more route search with sharing unlocked, observed rather than acted
//! on. The probe classifies the failure: an end no stub can reach is
//! **walled** (an airwire — no width grows a sealed face open), and a
//! search that hit channels short of lanes names a **corridor deficit** —
//! the one case the layout may repair, by growing the named containers'
//! gaps by exactly the missing lanes' worth. Port starvation never
//! terminates a non-fan end (sharing reopens its sides), so it is no class
//! of its own. The probe may even find a route: ground truth already
//! disproved it — every lever judged and rejected such candidates — so the
//! shortfalls along the refused corridors stay the honest deficit.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

use super::Router;
use super::audit;
use super::bundle::End;
use super::graph::Axis;
use super::path;
use super::runs::Chain;

/// Observations from one probed [`Router::route_bundle`] call: per end,
/// whether any rung of the world ladder offered entries; per channel the
/// search consulted, the worst lane shortfall.
#[derive(Default)]
pub struct Probe {
    offered: [Cell<bool>; 2],
    short: RefCell<BTreeMap<(usize, Axis), usize>>,
}

impl Probe {
    /// Record one end's entry set at one rung of the world ladder.
    pub fn entries(&self, end: End, offered: bool) {
        let e = match end {
            End::A => 0,
            End::B => 1,
        };
        self.offered[e].set(self.offered[e].get() | offered);
    }

    /// Record a channel the search wanted but found short of lanes.
    pub fn lanes_short(&self, world: usize, axis: Axis, lanes: usize) {
        let mut short = self.short.borrow_mut();
        let worst = short.entry((world, axis)).or_insert(0);
        *worst = (*worst).max(lanes);
    }

    /// The failure is a corridor deficit iff both ends could enter the graph
    /// somewhere — a walled end is unreachable at any width — and at least
    /// one consulted channel closed for lack of lanes alone.
    fn deficits(self) -> Option<BTreeMap<(usize, Axis), usize>> {
        let entered = self.offered.iter().all(Cell::get);
        let short = self.short.into_inner();
        (entered && !short.is_empty()).then_some(short)
    }
}

/// Classify every impossible bundle against the final routing state and
/// aggregate the corridor deficits per container: dot-path (`""` = scene) →
/// `(Δgap_y, Δgap_x)` in px, the worst H- and V-channel shortfalls times
/// `clearance`. An H-channel widens with the gap between rows, a V-channel
/// with the gap between columns. An endpoint side **sealed** by a sibling
/// keep-out nearer than `2·clearance` is the same deficit in disguise —
/// zero lanes where the route needed at least one, no closure event ever
/// fired — and names the gap's owner directly.
pub fn starved(
    router: &Router,
    raw: &[Option<Chain>],
    impossible: &[usize],
) -> BTreeMap<String, (f64, f64)> {
    let c = router.clearance;
    let mut out: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    let mut grow = |path: String, vertical_gap: bool, px: f64| {
        let entry = out.entry(path).or_insert((0.0, 0.0));
        if vertical_gap {
            entry.0 = entry.0.max(px);
        } else {
            entry.1 = entry.1.max(px);
        }
    };
    let mut probed: Vec<usize> = impossible
        .iter()
        .map(|&m| audit::bundle_of(router, m))
        .collect();
    probed.sort_unstable();
    probed.dedup();
    for bi in probed {
        let members = router.bundles[bi].members.clone();
        let rep = &router.reqs[members[0]];
        let occ = audit::occupancy_without(raw, &members, c);
        let ports = audit::ports_without(raw, &members, c);
        let probe = Probe::default();
        let _ = router.route_bundle(
            bi,
            &occ,
            &ports,
            path::FREE,
            &[],
            [None, None],
            false,
            true,
            Some(&probe),
        );
        for (path, rect) in [(&rep.a_path, rep.a_rect), (&rep.b_path, rep.b_rect)] {
            for (parent, vertical_gap, px) in sealed_sides(router, path, rect, c) {
                grow(parent, vertical_gap, px);
            }
        }
        let Some(deficits) = probe.deficits() else {
            continue;
        };
        for ((w, axis), lanes) in deficits {
            let need = lanes as f64 * c;
            let path = router.worlds[w].path.clone();
            match axis {
                Axis::H => grow(path, true, need),
                Axis::V => grow(path, false, need),
            }
        }
    }
    out
}

/// The sealed sides of one impossible endpoint: each face a sibling
/// keep-out covers from nearer than `2·clearance` — too close for even one
/// lane — reported as `(parent container, gap axis, px missing)`. Walls
/// (padding) don't count: gap growth owns gaps, nothing else.
fn sealed_sides(
    router: &Router,
    path: &str,
    rect: super::rect::Rect,
    c: f64,
) -> Vec<(String, bool, f64)> {
    let parent = super::parent_path(path);
    let mut out = Vec::new();
    for sibling in router.index.child_rects(&parent) {
        if sibling.x0 == rect.x0 && sibling.y0 == rect.y0 && sibling.x1 == rect.x1 {
            continue;
        }
        let across = sibling.x1.min(rect.x1) > sibling.x0.max(rect.x0);
        let down = sibling.y1.min(rect.y1) > sibling.y0.max(rect.y0);
        let gap = if across && sibling.y1 <= rect.y0 {
            Some((true, rect.y0 - sibling.y1))
        } else if across && sibling.y0 >= rect.y1 {
            Some((true, sibling.y0 - rect.y1))
        } else if down && sibling.x1 <= rect.x0 {
            Some((false, rect.x0 - sibling.x1))
        } else if down && sibling.x0 >= rect.x1 {
            Some((false, sibling.x0 - rect.x1))
        } else {
            None
        };
        if let Some((vertical_gap, g)) = gap
            && g < 2.0 * c
        {
            out.push((parent.clone(), vertical_gap, 2.0 * c - g));
        }
    }
    out
}

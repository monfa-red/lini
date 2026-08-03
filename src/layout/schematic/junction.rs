//! Junction dots [SPEC 16.5] — the generated `|junction|` chrome marking every
//! point where three or more wire ends meet, read off the routed geometry once
//! the router has drawn it.
//!
//! **The meets come from one source: the router's own fan groups.** A fan is
//! the merge machinery's record that two wire ends share one landing — an `&`
//! group's shared port, and the implicit fan that same-pin landings merge into
//! (ROUTING.md Fixed ports, [SPEC 16.5]) — and the drawn link carries its group
//! ids out (`RoutedLink::fan_from` / `fan_to`). Nothing here re-derives
//! shared-ness by scanning the sheet for coincident geometry: a point is a meet
//! only where the router said those ends are one landing, and the only geometry
//! read is that of the members it named. Those ids are per-**driver**, not per
//! scene, and only the orthogonal driver's are read — see [`junctions`].
//!
//! **Where the dot goes.** A fan draws as *one lead until the split*
//! [SPEC 16.5]: every member leaves the shared landing along the same normal,
//! so their leads lie on top of one another and peel off one at a time. The
//! conductors meeting at a peel-off point are the one arriving lead, one per
//! distinct direction leaving there, and the continuation if any member is
//! still on the lead — three or more of those and the point is dotted. At the
//! landing itself only the terminal's own stub and the single lead meet, which
//! is two: the pin is never the dot, the split is ([SPEC 16.5] says so in as
//! many words).
//!
//! **A label's stub never counts** [SPEC 16.5]. A net tag hangs off the wire
//! rather than conducting away from it, so a member whose far end lands on a
//! `|label|` is dropped by *type* before any counting — not by hoping its leg
//! draws too short to matter.
//!
//! The dot is chrome: it paints entirely through its one `.lini-junction` rule
//! and authors no `style=` diff of its own [SPEC 18], so `|junction| { fill:
//! none; stroke: none }` removes it.

use std::collections::{BTreeMap, BTreeSet};

use super::super::ir::{PlacedNode, RoutedLink};
use super::super::prim;
use crate::desugar::schematic::{SchKind, sch_kind};
use crate::ledger::consts::JUNCTION_RADIUS;
use crate::resolve::Strategy;

/// Coordinate slack. The router's fan members share a landing bit-exactly, but
/// their peel-off coordinates come from placement's own arithmetic, so two legs
/// turning on one ordinate agree to floating-point noise, not to the bit.
const EPS: f64 = 1e-6;

/// The junction dots a drawn scene calls for, in scene coordinates — empty for
/// every scene that placed no schematic part.
pub(crate) fn junctions(nodes: &[PlacedNode], links: &[RoutedLink]) -> Vec<PlacedNode> {
    let parts = parts(nodes);
    if parts.is_empty() {
        return Vec::new();
    }
    // A fan group id identifies a landing **within its driver**: every strategy
    // calls `request::fan_groups` for itself and numbers its kept groups from
    // zero, so an orthogonal fan and a natural one both answer to 0. Hence the
    // key — an id alone is not a landing.
    let mut groups: BTreeMap<(Strategy, u32), Vec<(usize, bool)>> = BTreeMap::new();
    for (i, w) in links.iter().enumerate() {
        // …and only the orthogonal driver's fans are read at all. A schematic
        // wire is orthogonal by law [SPEC 16.5], and the arithmetic below reads
        // straight legs off a polyline: a `natural` wire's `path` is a *sampling*
        // of a drawn curve, so its "peel-off point" would be wherever that
        // sampling first bends away — which is no meet, and can land on the pin
        // itself, the one place SPEC 16.5 says a dot never goes. A scope that
        // overrides `routing:` gets no dots rather than wrong ones.
        if w.strategy != Strategy::Orthogonal || w.path.len() < 2 {
            continue;
        }
        if let Some(g) = w.fan_from {
            groups.entry((w.strategy, g)).or_default().push((i, true));
        }
        if let Some(g) = w.fan_to {
            groups.entry((w.strategy, g)).or_default().push((i, false));
        }
    }
    let mut at: Vec<(f64, f64)> = Vec::new();
    for members in groups.values() {
        for p in meets(links, members, &parts) {
            if !at.iter().any(|q| same(*q, p)) {
                at.push(p);
            }
        }
    }
    at.into_iter()
        .map(|(x, y)| prim::junction_dot(x, y, JUNCTION_RADIUS))
        .collect()
}

/// The placed schematic parts by dot-path [SPEC 16.2] — a part is a leaf, so
/// the walk stops there and every address inside it (`u7.vs`, `c24.p1`) is that
/// part's. Anonymous containers contribute no path segment [SPEC 9], exactly as
/// the router's scene index reads them.
fn parts(nodes: &[PlacedNode]) -> BTreeMap<String, SchKind> {
    fn walk(nodes: &[PlacedNode], prefix: &str, out: &mut BTreeMap<String, SchKind>) {
        for n in nodes {
            let path = match &n.id {
                Some(id) if prefix.is_empty() => id.clone(),
                Some(id) => format!("{prefix}.{id}"),
                None => prefix.to_owned(),
            };
            match (sch_kind(&n.type_chain), n.id.is_some()) {
                (Some(kind), true) => {
                    out.insert(path, kind);
                }
                _ => walk(&n.children, &path, out),
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(nodes, "", &mut out);
    out
}

/// The part an endpoint address names — itself, or the nearest ancestor path
/// (a pin's `u1.c` is the part `u1`).
fn part_of<'a>(parts: &'a BTreeMap<String, SchKind>, addr: &str) -> Option<&'a SchKind> {
    let mut at = addr;
    loop {
        if let Some(kind) = parts.get(at) {
            return Some(kind);
        }
        at = &at[..at.rfind('.')?];
    }
}

/// One counted member of a fan: how far along the shared lead it stays, and how
/// it leaves — a turn's direction, or `None` where the wire ends there.
struct Leg {
    along: f64,
    away: Option<(i8, i8)>,
}

/// The dotted points of one fan group. Empty unless the group's shared landing
/// is a schematic part's — the scope's own law, asked of the geometry the
/// router landed on rather than of the wire's written scope, because a pin is a
/// pin whoever wires it [SPEC 16.4].
fn meets(
    links: &[RoutedLink],
    members: &[(usize, bool)],
    parts: &BTreeMap<String, SchKind>,
) -> Vec<(f64, f64)> {
    let mut origin: Option<(f64, f64)> = None;
    let mut dir: Option<(i8, i8)> = None;
    let mut legs: Vec<Leg> = Vec::new();
    for &(i, from_start) in members {
        let w = &links[i];
        let path: Vec<(f64, f64)> = if from_start {
            w.path.clone()
        } else {
            w.path.iter().rev().copied().collect()
        };
        let (addr, far) = if from_start {
            (&w.seg_from, &w.seg_to)
        } else {
            (&w.seg_to, &w.seg_from)
        };
        if part_of(parts, addr).is_none() {
            return Vec::new();
        }
        match origin {
            None => origin = Some(path[0]),
            Some(p) if same(p, path[0]) => {}
            // A group whose members do not actually share a point is no lead to
            // split (a fan on an unpinned landing, whose ends the ladder placed
            // apart) — it says nothing about meets, so it draws nothing.
            Some(_) => return Vec::new(),
        }
        let d = step(path[0], path[1]);
        match dir {
            None => dir = Some(d),
            Some(t) if t == d => {}
            Some(_) => return Vec::new(),
        }
        // A label's stub never counts [SPEC 16.5] — by type, at the far end.
        if part_of(parts, far) == Some(&SchKind::Label) {
            continue;
        }
        legs.push(leg(&path, path[0], d));
    }
    let (Some(origin), Some(dir)) = (origin, dir) else {
        return Vec::new();
    };
    if legs.len() < 2 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (k, leg) in legs.iter().enumerate() {
        // One point, judged once: the first leg peeling off at this distance
        // speaks for every leg that shares it.
        if legs[..k].iter().any(|l| (l.along - leg.along).abs() <= EPS) {
            continue;
        }
        let away: BTreeSet<Option<(i8, i8)>> = legs
            .iter()
            .filter(|l| (l.along - leg.along).abs() <= EPS)
            .map(|l| l.away)
            .collect();
        let on = legs.iter().any(|l| l.along > leg.along + EPS);
        if 1 + away.len() + usize::from(on) >= 3 {
            out.push((
                origin.0 + f64::from(dir.0) * leg.along,
                origin.1 + f64::from(dir.1) * leg.along,
            ));
        }
    }
    out
}

/// How far a member stays on the shared lead, and how it leaves it: the walk
/// runs while the path is still on the ray `origin + t·dir`, so a leg that
/// bends away at its first corner reports that corner and a straight leg
/// reports its own far landing (`away: None` — it ends there).
fn leg(path: &[(f64, f64)], origin: (f64, f64), dir: (i8, i8)) -> Leg {
    let along =
        |p: (f64, f64)| f64::from(dir.0) * (p.0 - origin.0) + f64::from(dir.1) * (p.1 - origin.1);
    let off = |p: (f64, f64)| {
        f64::from(dir.1.abs()) * (p.0 - origin.0) + f64::from(dir.0.abs()) * (p.1 - origin.1)
    };
    let mut last = 0;
    for (k, &p) in path.iter().enumerate().skip(1) {
        if off(p).abs() > EPS {
            break;
        }
        last = k;
    }
    Leg {
        along: along(path[last]),
        away: (last + 1 < path.len()).then(|| step(path[last], path[last + 1])),
    }
}

/// The unit direction from `a` to `b` on an orthogonal path.
fn step(a: (f64, f64), b: (f64, f64)) -> (i8, i8) {
    let unit = |d: f64| if d.abs() <= EPS { 0 } else { d.signum() as i8 };
    if (b.0 - a.0).abs() >= (b.1 - a.1).abs() {
        (unit(b.0 - a.0), 0)
    } else {
        (0, unit(b.1 - a.1))
    }
}

fn same(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() <= EPS && (a.1 - b.1).abs() <= EPS
}

#[cfg(test)]
#[path = "junction_tests.rs"]
mod tests;

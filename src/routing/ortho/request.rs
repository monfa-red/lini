//! Link statements → per-edge route requests, bundles, and fan groups.
//!
//! Resolve has already expanded fans, chains, and `&`-groups into
//! `ResolvedLink`s with endpoint lists; each consecutive pair becomes one
//! request here, ordered by declaration then expansion — the order every later
//! tie breaks on. Edges with the same unordered `(path, forced side)` pair form
//! one **bundle** of multiplicity *k* (adjacent rails); edges of one statement
//! sharing a segment endpoint form a **fan group** (one shared port and stub).

use super::rect::Rect;
use super::scene::SceneIndex;
use crate::ast::Side;
use crate::error::Error;
use crate::ledger::consts::DEFAULT_CLEARANCE;
use crate::render::markers::marker_size;
use crate::resolve::{AttrMap, MarkerKind, Markers, Program, Strategy};
use crate::span::Span;

pub struct EdgeReq {
    pub a_path: String,
    pub b_path: String,
    pub a_rect: Rect,
    pub b_rect: Rect,
    pub side_a: Option<Side>,
    pub side_b: Option<Side>,
    /// The wiring strategy drawing this edge [SPEC 9]: `routing:` cascades
    /// per scope and per link, so one expansion serves every strategy.
    pub routing: Strategy,
    pub clearance: f64,
    /// Stub lengths: ≥ clearance, and ≥ the end's marker so it has a run-up.
    pub stub_a: f64,
    pub stub_b: f64,
    pub markers: Markers,
    pub attrs: AttrMap,
    /// `.style` names on the link — carried through to the rendered group's
    /// `lini-style-*` classes (paint never read here; routing ignores it).
    pub applied_styles: Vec<String>,
    pub span: Span,
    pub data_from: String,
    pub data_to: String,
    /// Statement / chain-segment / cartesian-expansion position — the
    /// declaration-order key every later stage ties on.
    pub stmt: usize,
    pub seg: usize,
    pub expansion: usize,
}

/// One end of a request, as later stages address it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    A,
    B,
}

impl End {
    /// The far end.
    pub fn other(self) -> End {
        match self {
            End::A => End::B,
            End::B => End::A,
        }
    }
}

impl EdgeReq {
    pub fn path(&self, end: End) -> &str {
        match end {
            End::A => &self.a_path,
            End::B => &self.b_path,
        }
    }

    pub fn side(&self, end: End) -> Option<Side> {
        match end {
            End::A => self.side_a,
            End::B => self.side_b,
        }
    }
}

/// Whether the router owns this link [SPEC 9/13/15]: a sequence scope's
/// messages are drawn by the sequence layout, which owns *where* (column x,
/// row y) and lowers each wire through the `straight` strategy itself — they
/// are never requests; the router likewise only ever routes wires, and a
/// drawing scope owns *all* its links — measures, mates, and its annotation
/// arrows alike. The label pass must walk `program.links` with **this same
/// filter** or its statement numbering drifts off the requests' and labels
/// land on the wrong wire.
pub fn is_routed(program: &Program, w: &crate::resolve::ResolvedLink) -> bool {
    w.kind == crate::resolve::LinkKind::Wire
        && !w.projection
        && !crate::layout::sequence::is_sequence_scope(program, &w.scope)
        && !crate::layout::drawing::is_drawing_scope(program, &w.scope)
}

pub fn requests(program: &Program, index: &SceneIndex) -> Result<Vec<EdgeReq>, Error> {
    let mut out = Vec::new();
    let mut stmt_ids: Vec<Span> = Vec::new();
    for w in &program.links {
        if !is_routed(program, w) {
            continue;
        }
        let stmt = match stmt_ids.iter().position(|s| *s == w.span) {
            Some(i) => i,
            None => {
                stmt_ids.push(w.span);
                stmt_ids.len() - 1
            }
        };
        let expansion = out
            .iter()
            .rev()
            .find(|r: &&EdgeReq| r.stmt == stmt)
            .map_or(0, |r| r.expansion + 1);
        let clearance = link_clearance(&w.attrs);
        let thickness = w.attrs.number("stroke-width").unwrap_or(0.0);
        let eps = &w.endpoints;
        let segs = eps.len() - 1;
        for i in 0..segs {
            let (a, b) = (&eps[i], &eps[i + 1]);
            let rect_of = |e: &crate::resolve::ResolvedEndpoint| {
                index.rect(&e.path).ok_or_else(|| {
                    Error::at(e.span, format!("link endpoint '{}' not placed", e.path))
                })
            };
            let start = if i == 0 {
                w.markers.start
            } else {
                MarkerKind::None
            };
            let end = if i == segs - 1 {
                w.markers.end
            } else {
                MarkerKind::None
            };
            let stub = |m: MarkerKind| {
                let run_up = if m == MarkerKind::None {
                    0.0
                } else {
                    marker_size(thickness)
                };
                clearance.max(run_up)
            };
            out.push(EdgeReq {
                a_path: a.path.clone(),
                b_path: b.path.clone(),
                a_rect: rect_of(a)?,
                b_rect: rect_of(b)?,
                side_a: a.side,
                side_b: b.side,
                routing: w.routing,
                clearance,
                stub_a: stub(start),
                stub_b: stub(end),
                markers: Markers { start, end },
                attrs: w.attrs.clone(),
                applied_styles: w.applied_styles.clone(),
                span: w.span,
                data_from: eps[0].path.clone(),
                data_to: eps[segs].path.clone(),
                stmt,
                seg: i,
                expansion,
            });
        }
    }
    Ok(out)
}

/// One bundle: requests with the same unordered `(path, side)` endpoint pair.
/// They route once and ride as `members.len()` adjacent rails.
pub struct Bundle {
    pub members: Vec<usize>,
}

type PairKey = ((String, u8), (String, u8));

fn pair_key(r: &EdgeReq) -> PairKey {
    let side_id = |s: Option<Side>| s.map_or(4u8, Side::index);
    let a = (r.a_path.clone(), side_id(r.side_a));
    let b = (r.b_path.clone(), side_id(r.side_b));
    if a <= b { (a, b) } else { (b, a) }
}

/// Bundles in declaration order of their first member; members in declaration
/// order within. Self-loops never bundle, and only orthogonal requests enter
/// — a `routing: straight` wire is one trimmed segment, and a natural
/// duplicate needs no route sharing (its port ladder alone makes the rails).
pub fn bundles(reqs: &[EdgeReq]) -> Vec<Bundle> {
    let mut out: Vec<(PairKey, Bundle)> = Vec::new();
    for (i, r) in reqs.iter().enumerate() {
        if r.routing != Strategy::Orthogonal {
            continue;
        }
        if r.a_path == r.b_path {
            out.push((pair_key(r), Bundle { members: vec![i] }));
            continue;
        }
        let key = pair_key(r);
        match out
            .iter_mut()
            .find(|(k, b)| *k == key && reqs[b.members[0]].a_path != reqs[b.members[0]].b_path)
        {
            Some((_, b)) => b.members.push(i),
            None => out.push((key, Bundle { members: vec![i] })),
        }
    }
    out.into_iter().map(|(_, b)| b).collect()
}

/// Fan groups: requests of one statement (and one strategy — each driver
/// groups its own) sharing a segment endpoint share that end's port and
/// stub. `groups[g]` lists members in expansion order; the per-request entry
/// holds `(group, end)` for each fanned end.
pub struct Fans {
    pub groups: Vec<Vec<usize>>,
    pub of: Vec<Vec<(usize, End)>>,
}

impl Fans {
    pub fn group_at(&self, req: usize, end: End) -> Option<usize> {
        self.of[req]
            .iter()
            .find(|(_, e)| *e == end)
            .map(|(g, _)| *g)
    }
}

pub fn fan_groups(reqs: &[EdgeReq], strategy: Strategy) -> Fans {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut of: Vec<Vec<(usize, End)>> = vec![Vec::new(); reqs.len()];
    let mut keys: Vec<(usize, usize, End, String, Option<Side>)> = Vec::new();
    for (i, r) in reqs.iter().enumerate() {
        if r.routing != strategy {
            continue;
        }
        for end in [End::A, End::B] {
            let key = (r.stmt, r.seg, end, r.path(end).to_owned(), r.side(end));
            match keys.iter().position(|k| *k == key) {
                Some(g) => {
                    groups[g].push(i);
                    of[i].push((g, end));
                }
                None => {
                    keys.push(key);
                    groups.push(vec![i]);
                    of[i].push((groups.len() - 1, end));
                }
            }
        }
    }
    // Cross-statement port sharing [SPEC 12/8]: a node's wires **into its own
    // descendants**, forced onto one port (same path, same explicit side) with
    // pairwise-distinct far ends, are one fan across statements — the
    // crow's-foot a mindmap's per-branch root arms form (each arm its own
    // statement so it wears its own hue class). The containment gate keeps
    // ordinary same-port wires as they are — a link into one's own descendant
    // is that node's internal affair, the same judgment the cascade and the
    // world ladder already apply — and a repeated far end keeps the
    // parallel-rails contract (duplicates separate at pitch, the pcb look).
    let mut buckets: Vec<((End, String, Side), Vec<usize>)> = Vec::new();
    for (g, key) in keys.iter().enumerate() {
        let (_, _, end, ref path, side) = *key;
        let Some(side) = side else { continue };
        let bkey = (end, path.clone(), side);
        match buckets.iter_mut().find(|(k, _)| *k == bkey) {
            Some((_, gs)) => gs.push(g),
            None => buckets.push((bkey, vec![g])),
        }
    }
    for ((end, near, _), gs) in buckets {
        if gs.len() < 2 {
            continue;
        }
        let far = end.other();
        let prefix = format!("{near}.");
        let mut far_paths: Vec<&str> = Vec::new();
        let mut fans = true;
        for &g in &gs {
            for &i in &groups[g] {
                let p = reqs[i].path(far);
                fans &= p.starts_with(&prefix) && !far_paths.contains(&p);
                far_paths.push(p);
            }
        }
        if !fans {
            continue;
        }
        let (host, rest) = gs.split_first().expect("non-empty bucket");
        for &g in rest {
            for i in std::mem::take(&mut groups[g]) {
                groups[*host].push(i);
                let slot = of[i]
                    .iter_mut()
                    .find(|(og, oe)| *og == g && *oe == end)
                    .expect("end registered");
                slot.0 = *host;
            }
        }
    }
    // Only shared ends are fans; singleton "groups" dissolve.
    let mut remap: Vec<Option<usize>> = Vec::with_capacity(groups.len());
    let mut kept: Vec<Vec<usize>> = Vec::new();
    for g in &groups {
        if g.len() > 1 {
            kept.push(g.clone());
            remap.push(Some(kept.len() - 1));
        } else {
            remap.push(None);
        }
    }
    for ends in &mut of {
        ends.retain_mut(|(g, _)| match remap[*g] {
            Some(n) => {
                *g = n;
                true
            }
            None => false,
        });
    }
    Fans { groups: kept, of }
}

/// The one clearance number (ROUTING Vocabulary): the link's merged attrs,
/// already carrying the cascaded link default.
pub fn link_clearance(attrs: &AttrMap) -> f64 {
    attrs.number("clearance").unwrap_or(DEFAULT_CLEARANCE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(stmt: usize, seg: usize, expansion: usize, a: &str, b: &str) -> EdgeReq {
        EdgeReq {
            a_path: a.to_owned(),
            b_path: b.to_owned(),
            a_rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            b_rect: Rect::new(40.0, 0.0, 50.0, 10.0),
            side_a: None,
            side_b: None,
            routing: Strategy::Orthogonal,
            clearance: 8.0,
            stub_a: 8.0,
            stub_b: 8.0,
            markers: Markers::default(),
            attrs: AttrMap::default(),
            applied_styles: Vec::new(),
            span: Span::empty(),
            data_from: a.to_owned(),
            data_to: b.to_owned(),
            stmt,
            seg,
            expansion,
        }
    }

    #[test]
    fn same_unordered_pair_forms_one_bundle_across_statements() {
        let reqs = vec![
            req(0, 0, 0, "a", "b"),
            req(1, 0, 0, "c", "d"),
            req(2, 0, 0, "b", "a"),
            req(3, 0, 0, "a", "b"),
        ];
        let bs = bundles(&reqs);
        assert_eq!(bs.len(), 2);
        assert_eq!(bs[0].members, vec![0, 2, 3]);
        assert_eq!(bs[1].members, vec![1]);
    }

    #[test]
    fn forced_sides_split_bundles() {
        let mut r1 = req(0, 0, 0, "a", "b");
        r1.side_a = Some(Side::Left);
        let r2 = req(1, 0, 0, "a", "b");
        let bs = bundles(&[r1, r2]);
        assert_eq!(bs.len(), 2);
    }

    #[test]
    fn self_loops_never_bundle() {
        let reqs = vec![req(0, 0, 0, "a", "a"), req(1, 0, 0, "a", "a")];
        let bs = bundles(&reqs);
        assert_eq!(bs.len(), 2);
    }

    #[test]
    fn fan_groups_share_a_statement_segment_endpoint() {
        // a -> b & c (one statement, two expansions) + an unrelated a -> d.
        let reqs = vec![
            req(0, 0, 0, "a", "b"),
            req(0, 0, 1, "a", "c"),
            req(1, 0, 0, "a", "d"),
        ];
        let fans = fan_groups(&reqs, Strategy::Orthogonal);
        assert_eq!(fans.groups, vec![vec![0, 1]]);
        assert_eq!(fans.group_at(0, End::A), Some(0));
        assert_eq!(fans.group_at(1, End::A), Some(0));
        assert_eq!(fans.group_at(0, End::B), None);
        assert_eq!(fans.group_at(2, End::A), None);
    }

    #[test]
    fn forced_containment_arms_fan_across_statements() {
        // r:right → r.a / r.b / r.c, three statements (a mindmap's tinted root
        // arms): one crow's-foot. A sibling wire forced to the same port
        // (x → nowhere inside r) and duplicates (two arms to r.a) stay out.
        let arm = |stmt: usize, to: &str| {
            let mut r = req(stmt, 0, 0, "r", to);
            r.side_a = Some(Side::Right);
            r
        };
        let reqs = vec![arm(0, "r.a"), arm(1, "r.b"), arm(2, "r.c")];
        let fans = fan_groups(&reqs, Strategy::Orthogonal);
        assert_eq!(fans.groups, vec![vec![0, 1, 2]]);
        for i in 0..3 {
            assert_eq!(fans.group_at(i, End::A), Some(0));
        }
        // A non-descendant far end blocks the whole family (conservative).
        let mut sibling = req(3, 0, 0, "r", "x");
        sibling.side_a = Some(Side::Right);
        let reqs = vec![arm(0, "r.a"), arm(1, "r.b"), sibling];
        assert!(fan_groups(&reqs, Strategy::Orthogonal).groups.is_empty());
        // A repeated far end keeps the parallel-rails contract.
        let reqs = vec![arm(0, "r.a"), arm(1, "r.a")];
        assert!(fan_groups(&reqs, Strategy::Orthogonal).groups.is_empty());
        // An unforced end never fans across statements.
        let reqs = vec![req(0, 0, 0, "r", "r.a"), req(1, 0, 0, "r", "r.b")];
        assert!(fan_groups(&reqs, Strategy::Orthogonal).groups.is_empty());
    }

    #[test]
    fn fan_in_groups_the_shared_target() {
        // fox & owl -> mouse.
        let reqs = vec![req(0, 0, 0, "fox", "mouse"), req(0, 0, 1, "owl", "mouse")];
        let fans = fan_groups(&reqs, Strategy::Orthogonal);
        assert_eq!(fans.groups, vec![vec![0, 1]]);
        assert_eq!(fans.group_at(0, End::B), Some(0));
        assert_eq!(fans.group_at(1, End::B), Some(0));
    }

    #[test]
    fn chain_segments_do_not_fan() {
        // a -> b -> c: two segments of one statement share `b` but not a seg index.
        let reqs = vec![req(0, 0, 0, "a", "b"), req(0, 1, 0, "b", "c")];
        let fans = fan_groups(&reqs, Strategy::Orthogonal);
        assert!(fans.groups.is_empty());
    }
}

//! Link routing — the strategy seam and shared result (ROUTING.md).
//!
//! Every strategy consumes the placed scene and the expanded link requests
//! and produces the same outputs — polylines, a report, strays — sharing one
//! spine: request expansion, markers, labels, stray drawing, render-time
//! rounding. Only geometry construction differs; validation ([`validate`]) is
//! per strategy. `orthogonal` (the default) is the six-step model in
//! [`ortho`]; `natural` rides that model's corridor choice and lowers to
//! cubic splines in [`natural`]; `straight` carries sequence messages.

pub(crate) mod natural;
pub(crate) mod ortho;
mod report;
pub(crate) mod straight;
mod validate;

pub use report::{Rule, Severity, Violation};
/// The transversal-crossing primitive, shared with the renderer's fillet
/// pass (a crossing must never land mid-arc).
pub(crate) use report::{cross, cross_oblique};

use crate::error::Error;
use crate::layout::ir::{PlacedNode, RoutedLink, Stray};
use crate::resolve::Program;

/// The routing result: the drawn links and the engine's report — the drawn
/// crossings (counted output) and the links it could not legally draw, each
/// of those rendered as a stray (the report made visible).
#[derive(Default)]
pub struct Routing {
    pub links: Vec<RoutedLink>,
    pub report: Vec<Violation>,
    pub strays: Vec<Stray>,
}

/// Route every link of the scene over the finished, immutable layout: expand
/// the requests once, hand each strategy its own, then run the shared spine —
/// declaration order, the label pass, and the wires containers drew
/// themselves (a sequence's messages, already lowered through `straight`).
pub fn route(program: &Program, nodes: &[PlacedNode]) -> Result<Routing, Error> {
    let index = ortho::scene::SceneIndex::build(nodes);
    let reqs = ortho::request::requests(program, &index)?;
    let (mut routing, mut req_of) = ortho::route(&index, &reqs);
    straight::route(&reqs, &mut routing, &mut req_of);
    let mut drawn: Vec<(usize, RoutedLink)> =
        req_of.drain(..).zip(routing.links.drain(..)).collect();
    drawn.sort_by_key(|&(i, _)| i);
    (req_of, routing.links) = drawn.into_iter().unzip();
    ortho::labels::place(&mut routing.links, &req_of, &reqs, program, &index);
    routing.links.extend(owned_links(nodes));
    Ok(routing)
}

/// The links containers drew themselves — a sequence's messages, stored on
/// their `PlacedNode` in local coordinates — lifted into scene coordinates.
pub(crate) fn owned_links(nodes: &[PlacedNode]) -> Vec<RoutedLink> {
    fn walk(n: &PlacedNode, ox: f64, oy: f64, out: &mut Vec<RoutedLink>) {
        let (cx, cy) = (ox + n.cx, oy + n.cy);
        for l in &n.links {
            let mut l = l.clone();
            for p in &mut l.path {
                *p = (p.0 + cx, p.1 + cy);
            }
            for t in &mut l.texts {
                t.position = (t.position.0 + cx, t.position.1 + cy);
            }
            out.push(l);
        }
        for c in &n.children {
            walk(c, cx, cy, out);
        }
    }
    let mut out = Vec::new();
    for n in nodes {
        walk(n, 0.0, 0.0, &mut out);
    }
    out
}

/// The independent four-law check over a drawn scene (see [`validate`]).
pub fn validate_routing(
    nodes: &[PlacedNode],
    links: &[RoutedLink],
    report: &[Violation],
) -> Vec<Violation> {
    validate::check(nodes, links, report)
}

/// Test-only hook: a node's absolute rect by full dot-path.
pub fn node_rect(nodes: &[PlacedNode], path: &str) -> Option<(f64, f64, f64, f64)> {
    let idx = ortho::scene::SceneIndex::build(nodes);
    idx.rect(path).map(|r| (r.x0, r.y0, r.x1, r.y1))
}

//! Link routing — the strategy seam and shared result (ROUTING.md).
//!
//! Every strategy consumes the placed scene and the expanded link requests
//! and produces the same outputs — polylines, a report, strays — sharing one
//! spine: request expansion, markers, labels, stray drawing, render-time
//! rounding, validation. Only geometry construction differs. `orthogonal`
//! (the default) is the six-step model in [`ortho`]; `straight` carries
//! sequence messages; `curved` is deferred.

pub(crate) mod ortho;
mod report;
mod validate;

/// The transversal-crossing primitive, shared with the renderer's fillet
/// pass (a crossing must never land mid-arc).
pub(crate) use report::cross;
pub use report::{Rule, Severity, Violation};

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

/// Route every link of the scene over the finished, immutable layout.
pub fn route(program: &Program, nodes: &[PlacedNode]) -> Result<Routing, Error> {
    let index = ortho::scene::SceneIndex::build(nodes);
    let reqs = ortho::request::requests(program, &index)?;
    Ok(ortho::route(program, &index, &reqs))
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

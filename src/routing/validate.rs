//! The independent law checker (ROUTING.md §The Four Laws) — a test oracle
//! that re-judges the drawn output with no router knowledge, never a repair.
//!
//! The v2 checks land with the adversarial suite (ROUTING-V2.md stage 6);
//! until then a drawn scene passes vacuously and the engine's own report
//! (impossible links, counted crossings) is the only verdict.

use super::report::Violation;
use crate::layout::ir::{PlacedNode, RoutedLink};

pub fn check(
    _nodes: &[PlacedNode],
    _links: &[RoutedLink],
    _report: &[Violation],
) -> Vec<Violation> {
    Vec::new()
}

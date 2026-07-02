//! The `orthogonal` strategy — ROUTING.md's six-step model: keep-outs &
//! worlds → channels → requests → weighted search → placement → geometry.
//! Each step decides once; none revisits an earlier step's answer.
//!
//! Landing stage by stage (ROUTING-V2.md): requests, the scene model, the
//! channel graph, and the weighted search are in; placement and geometry
//! follow.

pub(crate) mod cost;
pub(crate) mod geometry;
pub(crate) mod graph;
pub(crate) mod ledger;
pub(crate) mod rect;
pub(crate) mod request;
pub(crate) mod scene;
pub(crate) mod search;

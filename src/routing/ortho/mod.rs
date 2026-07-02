//! The `orthogonal` strategy — ROUTING.md's six-step model: keep-outs &
//! worlds → channels → requests → weighted search → placement → geometry.
//! Each step decides once; none revisits an earlier step's answer.
//!
//! Landing stage by stage (ROUTING-V2.md): requests, the scene model, the
//! channel graph, the weighted search, and placement are in; the driver and
//! geometry follow.

pub(crate) mod cost;
pub(crate) mod geometry;
pub(crate) mod graph;
pub(crate) mod ladder;
pub(crate) mod ledger;
pub(crate) mod place;
pub(crate) mod rect;
pub(crate) mod request;
pub(crate) mod scene;
pub(crate) mod search;

use crate::ast::Side;
use graph::{Axis, ChannelGraph};
use rect::Rect;

/// One routing world: a container's interior (`""` = the scene root) and its
/// channel decomposition.
// Scaffold: constructed by the pipeline driver (ROUTING-V2.md stage 4).
#[allow(dead_code)]
pub(crate) struct World {
    pub path: String,
    pub graph: ChannelGraph,
}

/// One end of a chain: the side it lands on, the endpoint's body, the lawful
/// port window on that side, and the fan group whose siblings share the port.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EndInfo {
    pub side: Side,
    pub rect: Rect,
    pub window: (f64, f64),
    pub fan: Option<usize>,
}

/// One straight piece of a route, in one channel of its axis. The span is
/// provisional until geometry fixes corners; the ordinate is placement's.
#[derive(Clone, Debug)]
pub(crate) struct Run {
    pub axis: Axis,
    pub chan: usize,
    pub span: (f64, f64),
    pub ord: Option<f64>,
}

/// One link's route: alternating runs, `runs[0]` serving `ends[0]`'s port
/// and the last run `ends[1]`'s — a single run serves both (a straight).
#[derive(Clone, Debug)]
pub(crate) struct Chain {
    /// Request index — the declaration-order key.
    pub link: usize,
    pub world: usize,
    pub runs: Vec<Run>,
    pub ends: [EndInfo; 2],
}

impl EndInfo {
    /// The side line's coordinate along the end run's travel axis — where
    /// the wire leaves the body.
    pub fn side_coord(&self) -> f64 {
        match self.side {
            Side::Right => self.rect.x1,
            Side::Left => self.rect.x0,
            Side::Top => self.rect.y0,
            Side::Bottom => self.rect.y1,
        }
    }

    /// The window's centre — the side centre whenever margins fit.
    pub fn centre(&self) -> f64 {
        (self.window.0 + self.window.1) / 2.0
    }
}

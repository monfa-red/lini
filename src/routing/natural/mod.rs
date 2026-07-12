//! The `natural` strategy's geometry lowering (ROUTING.md The natural
//! strategy): a chain the shared corridor search routed and placed, lowered
//! to straight marker stubs plus a G1 cubic spline instead of a rounded
//! polyline. Obstacle-aware corridor tightening (clearance sampling inside
//! the chosen corridor) lands with Stage 4 (`corridor.rs`); here the curve
//! follows the chain's polyline directly — the tree/mindmap free-sight case.

pub(crate) mod curve;

use super::ortho::{Chain, geometry};
use crate::layout::ir::Cubic;

/// Lower a placed chain to drawn natural geometry: the dense sampled path
/// (port and stub points exact) and the exact cubic segments between the
/// stubs — so every shared consumer (marker anchors, the label
/// arc-walk, masks, crossing counts) reads true drawn geometry with no
/// strategy knowledge.
pub(crate) fn lower(chain: &Chain, stub_a: f64, stub_b: f64) -> (Vec<(f64, f64)>, Vec<Cubic>) {
    curve::fit(&geometry::polyline(chain), stub_a, stub_b)
}

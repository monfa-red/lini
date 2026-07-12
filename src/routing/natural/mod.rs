//! The `natural` strategy's geometry lowering (ROUTING.md The natural
//! strategy): a chain the shared corridor search routed and placed, lowered
//! to straight marker stubs plus a G1 cubic spline instead of a rounded
//! polyline ([`curve`]), then held to the corridor — sampled against the
//! same keep-outs the search avoided and tightened toward the chain's
//! polyline wherever the free-flowing fit would break clearance
//! ([`corridor`]). On a free corridor (the tree/mindmap case) the fit passes
//! through untouched.

pub(crate) mod corridor;
pub(crate) mod curve;

use super::ortho::{Chain, geometry, request::EdgeReq, scene::SceneIndex};
use crate::layout::ir::Cubic;

/// Lower a placed chain to drawn natural geometry: the dense sampled path
/// (port and stub points exact) and the exact cubic segments between the
/// stubs — so every shared consumer (marker anchors, the label
/// arc-walk, masks, crossing counts) reads true drawn geometry with no
/// strategy knowledge. `c` is the diagram's routing clearance — the same
/// number the corridor search spent on keep-outs.
pub(crate) fn lower(
    index: &SceneIndex,
    req: &EdgeReq,
    chain: &Chain,
    c: f64,
) -> (Vec<(f64, f64)>, Vec<Cubic>) {
    let poly = geometry::polyline(chain);
    let fitted = curve::fit(&poly, req.stub_a, req.stub_b);
    let keep = corridor::Keepouts::build(
        index,
        [(&req.a_path, req.a_rect), (&req.b_path, req.b_rect)],
        c,
    );
    corridor::tighten(&poly, req.stub_a, req.stub_b, &keep, fitted)
}

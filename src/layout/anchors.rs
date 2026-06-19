//! Out-of-flow positioning for children inside a parent's bbox (SPEC §6).
//!
//! `pin` lifts a child out of the flow and centers its bbox on a named point of
//! the parent — `center`, an edge midpoint, or a corner. `translate` then nudges
//! any node (flow or pinned) after placement, reshaping nothing. There are no
//! compound anchor names and no numeric coordinate property: a corner falls out
//! of the two-word value, and exact coords are `pin: center` + `translate: x y`.

use super::ir::Bbox;
use super::values::as_pair;
use crate::error::Error;
use crate::resolve::{AttrMap, ResolvedValue};
use crate::span::Span;

/// A parent anchor a pinned child centers on, as signed fractions of the parent
/// bbox measured from its center: `center` = (0, 0), `top` = (0, -0.5),
/// `top right` = (0.5, -0.5), and so on.
#[derive(Clone, Copy)]
pub struct Pin {
    pub fx: f64,
    pub fy: f64,
}

/// A child's layout role, by how it is positioned.
#[derive(Clone, Copy, PartialEq)]
pub enum Role {
    /// No `pin` (or `pin: none`) — laid out by the container's `layout`.
    Flow,
    /// `pin:` set — an out-of-flow overlay; the parent does not grow for it.
    Pinned,
}

impl Pin {
    /// The child's local origin (`cx`, `cy`) that lands its **matching** anchor
    /// point on the parent's — so the child sits flush, corner on corner and
    /// edge on edge, never straddling. `pin: center` is centre-to-centre;
    /// `pin: top left` puts the child's top-left corner on the parent's.
    pub fn target(self, parent: Bbox, child: Bbox) -> (f64, f64) {
        let px = (parent.min_x + parent.max_x) / 2.0 + self.fx * parent.w();
        let py = (parent.min_y + parent.max_y) / 2.0 + self.fy * parent.h();
        let cbx = (child.min_x + child.max_x) / 2.0;
        let cby = (child.min_y + child.max_y) / 2.0;
        // Offset the child's own matching point (same fractions of its bbox)
        // back to the parent point — flush, not centred on it.
        (
            px - cbx - self.fx * child.w(),
            py - cby - self.fy * child.h(),
        )
    }
}

/// Read a child's `pin` (SPEC §6). `None` for an absent `pin` or `pin: none` (a
/// flow child); `Some(Pin)` for a named anchor; an error otherwise.
pub fn read_pin(attrs: &AttrMap, span: Span) -> Result<Option<Pin>, Error> {
    let Some(v) = attrs.get("pin") else {
        return Ok(None);
    };
    let bad = || {
        Error::at(
            span,
            "'pin' expects none, center, an edge (top/bottom/left/right), or a corner (e.g. 'top right')",
        )
    };
    match v {
        ResolvedValue::Ident(s) => Ok(match s.as_str() {
            "none" => None,
            "center" => Some(Pin { fx: 0.0, fy: 0.0 }),
            "top" => Some(Pin { fx: 0.0, fy: -0.5 }),
            "bottom" => Some(Pin { fx: 0.0, fy: 0.5 }),
            "left" => Some(Pin { fx: -0.5, fy: 0.0 }),
            "right" => Some(Pin { fx: 0.5, fy: 0.0 }),
            _ => return Err(bad()),
        }),
        // A corner: a vertical edge then a horizontal one (`top right`).
        ResolvedValue::Tuple(parts) if parts.len() == 2 => {
            let fy = match ident(&parts[0]) {
                Some("top") => -0.5,
                Some("bottom") => 0.5,
                _ => return Err(bad()),
            };
            let fx = match ident(&parts[1]) {
                Some("left") => -0.5,
                Some("right") => 0.5,
                _ => return Err(bad()),
            };
            Ok(Some(Pin { fx, fy }))
        }
        _ => Err(bad()),
    }
}

/// Whether a child is pinned — `pin` present and not `none`. A cheap check for
/// paint order; [`read_pin`] is the validating reader.
pub fn is_pinned(attrs: &AttrMap) -> bool {
    match attrs.get("pin") {
        Some(ResolvedValue::Ident(s)) => s != "none",
        Some(_) => true,
        None => false,
    }
}

/// Classify a child from its positioning attrs.
pub fn child_role(attrs: &AttrMap, span: Span) -> Result<Role, Error> {
    Ok(match read_pin(attrs, span)? {
        None => Role::Flow,
        Some(_) => Role::Pinned,
    })
}

/// `translate: x y` — a post-placement shift of the node, or `None` if unset.
pub fn translate(attrs: &AttrMap, span: Span) -> Result<Option<(f64, f64)>, Error> {
    match attrs.get("translate") {
        Some(v) => Ok(Some(as_pair(v, span)?)),
        None => Ok(None),
    }
}

fn ident(v: &ResolvedValue) -> Option<&str> {
    match v {
        ResolvedValue::Ident(s) => Some(s.as_str()),
        _ => None,
    }
}

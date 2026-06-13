//! Anchor + absolute positioning for children inside a parent's bbox.

use super::ir::Bbox;
use super::values::as_pair;
use crate::error::Error;
use crate::resolve::ResolvedValue;
use crate::span::Span;

#[derive(Clone, Copy)]
pub enum AbsolutePos {
    Coord(f64, f64),
    Anchor(NamedAnchor),
}

#[derive(Clone, Copy)]
pub enum NamedAnchor {
    Center,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    OutTop,
    OutBottom,
    OutLeft,
    OutRight,
    OutTopLeft,
    OutTopRight,
    OutBottomLeft,
    OutBottomRight,
}

impl NamedAnchor {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "center" => Self::Center,
            "top" => Self::Top,
            "bottom" => Self::Bottom,
            "left" => Self::Left,
            "right" => Self::Right,
            "top-left" => Self::TopLeft,
            "top-right" => Self::TopRight,
            "bottom-left" => Self::BottomLeft,
            "bottom-right" => Self::BottomRight,
            "out-top" => Self::OutTop,
            "out-bottom" => Self::OutBottom,
            "out-left" => Self::OutLeft,
            "out-right" => Self::OutRight,
            "out-top-left" => Self::OutTopLeft,
            "out-top-right" => Self::OutTopRight,
            "out-bottom-left" => Self::OutBottomLeft,
            "out-bottom-right" => Self::OutBottomRight,
            _ => return None,
        })
    }
}

/// Parse `at=` value. `(x, y)` → Coord; named ident → Anchor.
pub fn parse_at(value: &ResolvedValue, span: Span) -> Result<AbsolutePos, Error> {
    match value {
        ResolvedValue::Tuple(_) => {
            let (x, y) = as_pair(value, span)?;
            Ok(AbsolutePos::Coord(x, y))
        }
        ResolvedValue::Ident(name) => NamedAnchor::parse(name)
            .map(AbsolutePos::Anchor)
            .ok_or_else(|| Error::at(span, format!("unknown anchor '{}'", name))),
        _ => Err(Error::at(span, "'at=' expects (x,y) or an anchor name")),
    }
}

pub fn parse_offset(value: &ResolvedValue, span: Span) -> Result<(f64, f64), Error> {
    as_pair(value, span)
}

/// Resolve a named anchor against a parent bbox into a target center
/// position in the parent's local frame. The child's bbox is used to align
/// the child's bbox EDGE/CORNER to the parent's anchor for `out-*` anchors
/// and for keeping the child's own bbox tangent.
pub fn resolve_anchor(anchor: NamedAnchor, parent_bbox: Bbox, child_bbox: Bbox) -> (f64, f64) {
    let parent_cx = (parent_bbox.min_x + parent_bbox.max_x) / 2.0;
    let parent_cy = (parent_bbox.min_y + parent_bbox.max_y) / 2.0;
    let child_w = child_bbox.w();
    let child_h = child_bbox.h();

    match anchor {
        NamedAnchor::Center => (parent_cx, parent_cy),
        NamedAnchor::Top => (parent_cx, parent_bbox.min_y),
        NamedAnchor::Bottom => (parent_cx, parent_bbox.max_y),
        NamedAnchor::Left => (parent_bbox.min_x, parent_cy),
        NamedAnchor::Right => (parent_bbox.max_x, parent_cy),
        NamedAnchor::TopLeft => (parent_bbox.min_x, parent_bbox.min_y),
        NamedAnchor::TopRight => (parent_bbox.max_x, parent_bbox.min_y),
        NamedAnchor::BottomLeft => (parent_bbox.min_x, parent_bbox.max_y),
        NamedAnchor::BottomRight => (parent_bbox.max_x, parent_bbox.max_y),
        NamedAnchor::OutTop => (parent_cx, parent_bbox.min_y - child_h / 2.0),
        NamedAnchor::OutBottom => (parent_cx, parent_bbox.max_y + child_h / 2.0),
        NamedAnchor::OutLeft => (parent_bbox.min_x - child_w / 2.0, parent_cy),
        NamedAnchor::OutRight => (parent_bbox.max_x + child_w / 2.0, parent_cy),
        NamedAnchor::OutTopLeft => (
            parent_bbox.min_x - child_w / 2.0,
            parent_bbox.min_y - child_h / 2.0,
        ),
        NamedAnchor::OutTopRight => (
            parent_bbox.max_x + child_w / 2.0,
            parent_bbox.min_y - child_h / 2.0,
        ),
        NamedAnchor::OutBottomLeft => (
            parent_bbox.min_x - child_w / 2.0,
            parent_bbox.max_y + child_h / 2.0,
        ),
        NamedAnchor::OutBottomRight => (
            parent_bbox.max_x + child_w / 2.0,
            parent_bbox.max_y + child_h / 2.0,
        ),
    }
}

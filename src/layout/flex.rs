//! Row / column flex layout.
//!
//! Stacks children along the main axis with `gap` between them; centers them
//! on the cross axis by default. `h:` / `v:` alignment supports `start`,
//! `center`, `end`; the `between` / `around` / `evenly` distributions and
//! `stretch` (SPEC §6) are not implemented yet.

use super::ir::{Bbox, PlacedNode};
use super::primitives;
use crate::error::Error;
use crate::resolve::{AttrMap, ResolvedValue, VarTable};
use crate::span::Span;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Row,
    Column,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
}

/// Lay out already-bboxed children in row or column order. Positions the
/// children's `cx`/`cy` and returns the union bbox (in the parent's local
/// frame, i.e. centered on the container's origin).
pub fn lay_out_flex(
    axis: Axis,
    children: &mut [PlacedNode],
    attrs: &AttrMap,
    vars: &VarTable,
    span: Span,
) -> Result<Bbox, Error> {
    if children.is_empty() {
        return Ok(Bbox::empty());
    }

    let (gap_y, gap_x) = primitives::gap(attrs, vars, span)?;
    let h_align = parse_align(attrs, "h").unwrap_or(Align::Center);
    let v_align = parse_align(attrs, "v").unwrap_or(Align::Center);

    // Determine the cross-axis extent (max of children's cross sizes).
    let cross_size = match axis {
        Axis::Row => children.iter().map(|c| c.bbox.h()).fold(0.0_f64, f64::max),
        Axis::Column => children.iter().map(|c| c.bbox.w()).fold(0.0_f64, f64::max),
    };

    // Total main-axis extent = sum of sizes + (n-1)*gap.
    let total_main: f64 = children
        .iter()
        .map(|c| match axis {
            Axis::Row => c.bbox.w(),
            Axis::Column => c.bbox.h(),
        })
        .sum::<f64>()
        + gap_main(axis, gap_x, gap_y) * (children.len() as f64 - 1.0);

    // Place children starting at the left/top of the line, centered as a whole
    // on the container's local origin.
    let mut cursor = -total_main / 2.0;
    for child in children.iter_mut() {
        let (main_size, cross_size_child) = match axis {
            Axis::Row => (child.bbox.w(), child.bbox.h()),
            Axis::Column => (child.bbox.h(), child.bbox.w()),
        };

        let main_origin = cursor + main_size / 2.0 - child_offset(child.bbox, axis);
        let cross_origin = cross_align(
            cross_size,
            cross_size_child,
            cross_axis_align(axis, h_align, v_align),
        ) - child_offset(child.bbox, cross_of(axis));

        match axis {
            Axis::Row => {
                child.cx = main_origin;
                child.cy = cross_origin;
            }
            Axis::Column => {
                child.cx = cross_origin;
                child.cy = main_origin;
            }
        }

        cursor += main_size + gap_main(axis, gap_x, gap_y);
    }

    // Union of children bboxes in container frame.
    let mut union = children[0].bbox.shifted(children[0].cx, children[0].cy);
    for c in children.iter().skip(1) {
        union = union.union(c.bbox.shifted(c.cx, c.cy));
    }
    Ok(union)
}

fn child_offset(bbox: Bbox, axis: Axis) -> f64 {
    // For asymmetric bboxes (e.g., lines starting at local 0,0), the
    // bbox center isn't at the local origin. `cursor + main_size/2` puts the
    // bbox CENTER at the right spot; subtract the local-origin offset to get
    // the placement's `cx`/`cy` (which is the local origin).
    match axis {
        Axis::Row => (bbox.min_x + bbox.max_x) / 2.0,
        Axis::Column => (bbox.min_y + bbox.max_y) / 2.0,
    }
}

fn gap_main(axis: Axis, gap_x: f64, gap_y: f64) -> f64 {
    match axis {
        Axis::Row => gap_x,
        Axis::Column => gap_y,
    }
}

fn cross_of(axis: Axis) -> Axis {
    match axis {
        Axis::Row => Axis::Column,
        Axis::Column => Axis::Row,
    }
}

fn cross_axis_align(main_axis: Axis, h_align: Align, v_align: Align) -> Align {
    match main_axis {
        Axis::Row => v_align,
        Axis::Column => h_align,
    }
}

fn cross_align(track_size: f64, item_size: f64, align: Align) -> f64 {
    match align {
        Align::Start => -track_size / 2.0 + item_size / 2.0,
        Align::Center => 0.0,
        Align::End => track_size / 2.0 - item_size / 2.0,
    }
}

fn parse_align(attrs: &AttrMap, name: &str) -> Option<Align> {
    match attrs.get(name)? {
        ResolvedValue::Ident(s) => match s.as_str() {
            "start" => Some(Align::Start),
            "center" => Some(Align::Center),
            "end" => Some(Align::End),
            _ => None,
        },
        _ => None,
    }
}

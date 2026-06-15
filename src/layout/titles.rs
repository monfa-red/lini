//! Title bands.
//!
//! A `|title|` child reserves a strip on its container's **top** or **bottom**
//! edge: the flow content shifts to clear it, the box grows, and the title is
//! placed in the strip (slid by `align`). So a container's caption never
//! collides with its content — at any size, and regardless of the content's
//! own layout direction. `gap:` (default `title-gap`) is the breathing room
//! between the title and the content; `gap:0` makes the band exactly the
//! title's height. Left/right sides fall back to top — horizontal text only
//! reads on a top or bottom band.

use super::anchors::{Align, Side};
use super::ir::{Bbox, GridRule, PlacedNode};
use super::values::layout_var;
use crate::resolve::{AttrMap, ResolvedValue, VarTable};

/// Reserve bands for the title children and lay them out around the
/// already-placed flow content. Mutates flow children (shifted to clear the
/// bands), the grid rules (shifted with them), and the titles (placed in their
/// bands). Returns the container body bbox (content + bands), centred.
pub fn reserve_bands(
    children: &mut [PlacedNode],
    flow_indices: &[usize],
    reserve_indices: &[usize],
    flow_bbox: Bbox,
    grid_rules: &mut [GridRule],
    vars: &VarTable,
) -> Bbox {
    let mut top: Vec<usize> = Vec::new();
    let mut bottom: Vec<usize> = Vec::new();
    for &i in reserve_indices {
        match side(&children[i].attrs) {
            Side::Bottom => bottom.push(i),
            _ => top.push(i),
        }
    }

    // Each title contributes its own height plus its gap to the band.
    let band = |idxs: &[usize]| -> f64 {
        idxs.iter()
            .map(|&i| children[i].bbox.h() + gap(&children[i].attrs, vars))
            .sum()
    };
    let top_band = band(&top);
    let bottom_band = band(&bottom);

    let content_h = flow_bbox.h();
    let title_w = reserve_indices
        .iter()
        .map(|&i| children[i].bbox.w())
        .fold(0.0_f64, f64::max);
    let total_w = flow_bbox.w().max(title_w);
    let total_h = top_band + content_h + bottom_band;

    // Shift the flow content (and a table's rules) down so the whole stack —
    // top band, content, bottom band — sits centred on the origin.
    let content_dy = -total_h / 2.0 + top_band + content_h / 2.0;
    for &i in flow_indices {
        children[i].cy += content_dy;
    }
    for seg in grid_rules.iter_mut() {
        seg.1 += content_dy;
        seg.3 += content_dy;
    }

    // Place top titles from the top edge down, bottom titles from below content.
    let mut cursor = -total_h / 2.0;
    for &i in &top {
        let (h, g) = (children[i].bbox.h(), gap(&children[i].attrs, vars));
        place(&mut children[i], cursor + h / 2.0, total_w);
        cursor += h + g;
    }
    let mut cursor = total_h / 2.0;
    for &i in &bottom {
        let (h, g) = (children[i].bbox.h(), gap(&children[i].attrs, vars));
        place(&mut children[i], cursor - h / 2.0, total_w);
        cursor -= h + g;
    }

    Bbox::centered(total_w, total_h)
}

/// A title's bbox is centred, so its target band centre is its `cx`/`cy`
/// directly; `align` slides it along the band.
fn place(node: &mut PlacedNode, band_cy: f64, total_w: f64) {
    let w = node.bbox.w();
    node.cx = match align(&node.attrs) {
        Align::Start => -total_w / 2.0 + w / 2.0,
        Align::Center => 0.0,
        Align::End => total_w / 2.0 - w / 2.0,
    };
    node.cy = band_cy;
}

fn side(attrs: &AttrMap) -> Side {
    match attrs.get("side").and_then(ident).and_then(Side::parse) {
        Some(Side::Bottom) => Side::Bottom,
        _ => Side::Top,
    }
}

/// Titles default to `start` (a label hugs the leading edge), unlike the
/// `center` default for generic positioning.
fn align(attrs: &AttrMap) -> Align {
    attrs
        .get("align")
        .and_then(ident)
        .and_then(Align::parse)
        .unwrap_or(Align::Start)
}

fn gap(attrs: &AttrMap, vars: &VarTable) -> f64 {
    attrs
        .number("gap")
        .or_else(|| layout_var(vars, "title-gap"))
        .unwrap_or(6.0)
}

fn ident(v: &ResolvedValue) -> Option<&str> {
    match v {
        ResolvedValue::Ident(s) => Some(s.as_str()),
        _ => None,
    }
}

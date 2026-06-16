//! Caption bands.
//!
//! A `place:in` child (e.g. a group's caption — a `|text|` with `place:in`)
//! reserves a strip on its container's **top** or **bottom** edge: the flow
//! content shifts to clear it, the box grows, and the caption is placed in the
//! strip (slid by `align`). So a container's caption never collides with its
//! content — at any size, and regardless of the content's own layout direction.
//! The separation from the content is the container's own `gap` — a caption is
//! spaced like any sibling, so it lines up evenly with the rows below it.
//! Tighten (or loosen) one with `margin:` (negative eats the spacing); the band
//! already accounts for it, since margin inflates the footprint before it lands
//! here. Left/right sides fall back to top — horizontal text only reads on a
//! top or bottom band.
//!
//! `place` decides which side of the border the band lands on. `reserve_bands`
//! handles **`place:in`** — inside the frame, before padding is added, so the
//! border wraps the band. `place_out_bands` handles **`place:out`** — outside
//! the drawn frame, after padding is known: the band sits a `gap` beyond the
//! border and the layout footprint grows to reserve it, so the border still
//! hugs the content while siblings clear the caption (SPEC §7).

use super::anchors::{Align, Side, is_out_band};
use super::ir::{Bbox, GridRule, PlacedNode};
use super::primitives;
use crate::error::Error;
use crate::resolve::{AttrMap, ResolvedValue};

/// Reserve bands for the title children and lay them out around the
/// already-placed flow content. Mutates flow children (shifted to clear the
/// bands), the grid rules (shifted with them), and the titles (placed in their
/// bands). `gap` is the container's vertical gap — the space between a title
/// and the content, the same gap that separates flow siblings. Returns the
/// container body bbox (content + bands), centred.
pub fn reserve_bands(
    children: &mut [PlacedNode],
    flow_indices: &[usize],
    reserve_indices: &[usize],
    flow_bbox: Bbox,
    grid_rules: &mut [GridRule],
    gap: f64,
) -> Bbox {
    let mut top: Vec<usize> = Vec::new();
    let mut bottom: Vec<usize> = Vec::new();
    for &i in reserve_indices {
        match side(&children[i].attrs) {
            Side::Bottom => bottom.push(i),
            _ => top.push(i),
        }
    }

    // Each title contributes its (margin-inflated) footprint height plus one
    // container gap — separating stacked titles, and the last from the content.
    let band = |idxs: &[usize]| -> f64 { idxs.iter().map(|&i| children[i].bbox.h() + gap).sum() };
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
        let h = children[i].bbox.h();
        place(&mut children[i], cursor + h / 2.0, total_w);
        cursor += h + gap;
    }
    let mut cursor = total_h / 2.0;
    for &i in &bottom {
        let h = children[i].bbox.h();
        place(&mut children[i], cursor - h / 2.0, total_w);
        cursor -= h + gap;
    }

    Bbox::centered(total_w, total_h)
}

/// Place every `place:out` child outside the drawn `frame` and return the
/// layout footprint (`frame` ∪ the out-bands) plus whether any were placed.
///
/// Each out child sits flush beyond its edge — a container `gap` past the
/// border, the same spacing a flow sibling gets — and stacks outward when
/// several share an edge. The frame stays put (centred on the origin, where
/// the shape draws); the footprint grows asymmetrically outward, exactly as a
/// child's `margin:` grows its footprint, so siblings clear the whole caption
/// while the border keeps hugging the content. A child's own `margin:` insets
/// it within its band, just as it does for `place:in` (so a negative facing
/// margin pulls the caption toward the border). Top/bottom only; left/right
/// fall back to top, matching the inside bands.
pub fn place_out_bands(
    children: &mut [PlacedNode],
    frame: Bbox,
    gap: f64,
) -> Result<(Bbox, bool), Error> {
    let mut top: Vec<usize> = Vec::new();
    let mut bottom: Vec<usize> = Vec::new();
    for (i, c) in children.iter().enumerate() {
        if !is_out_band(&c.attrs) {
            continue;
        }
        match side(&c.attrs) {
            Side::Bottom => bottom.push(i),
            _ => top.push(i),
        }
    }
    if top.is_empty() && bottom.is_empty() {
        return Ok((frame, false));
    }

    let mut footprint = frame;
    // Top bands stack upward from the frame's top edge; bottom bands downward
    // from its bottom edge. The cursor tracks the outer edge reached so far.
    let mut cursor = frame.min_y;
    for &i in &top {
        cursor = place_out(
            &mut children[i],
            cursor,
            -1.0,
            gap,
            frame.w(),
            &mut footprint,
        )?;
    }
    let mut cursor = frame.max_y;
    for &i in &bottom {
        cursor = place_out(
            &mut children[i],
            cursor,
            1.0,
            gap,
            frame.w(),
            &mut footprint,
        )?;
    }
    Ok((footprint, true))
}

/// Place one out-band child flush beyond `edge` in direction `dir` (-1 up, +1
/// down), a `gap` clear of it, slid along the band by `align` within `band_w`.
/// Unions the placed (margin-inflated) footprint into `footprint` and returns
/// the new outer edge. The child's `margin:` inflates its footprint for the
/// band, then the drawn box deflates back margin-inset within it.
fn place_out(
    node: &mut PlacedNode,
    edge: f64,
    dir: f64,
    gap: f64,
    band_w: f64,
    footprint: &mut Bbox,
) -> Result<f64, Error> {
    let (t, r, b, l) = primitives::margin(&node.attrs, node.span)?;
    let drawn = node.bbox;
    node.bbox = drawn.expand(t, r, b, l);
    let h = node.bbox.h();
    place(node, edge + dir * (gap + h / 2.0), band_w);
    *footprint = footprint.union(node.bbox.shifted(node.cx, node.cy));
    node.bbox = drawn;
    Ok(edge + dir * (gap + h))
}

/// Land the title's footprint centre at `band_cy`, slid along the band by
/// `align`. The footprint may be off-centre (asymmetric `margin:`), so subtract
/// its centre — exactly as flex does — to get the `cx`/`cy` of the local origin.
fn place(node: &mut PlacedNode, band_cy: f64, total_w: f64) {
    let w = node.bbox.w();
    let center_x = (node.bbox.min_x + node.bbox.max_x) / 2.0;
    let center_y = (node.bbox.min_y + node.bbox.max_y) / 2.0;
    let target_x = match align(&node.attrs) {
        Align::Start => -total_w / 2.0 + w / 2.0,
        Align::Center => 0.0,
        Align::End => total_w / 2.0 - w / 2.0,
    };
    node.cx = target_x - center_x;
    node.cy = band_cy - center_y;
}

fn side(attrs: &AttrMap) -> Side {
    match attrs.get("side").and_then(ident).and_then(Side::parse) {
        Some(Side::Bottom) => Side::Bottom,
        _ => Side::Top,
    }
}

/// Bands slide by `align`, defaulting to `center` — the one alignment default
/// across the language (a left-hugging caption is `align:start`).
fn align(attrs: &AttrMap) -> Align {
    attrs
        .get("align")
        .and_then(ident)
        .and_then(Align::parse)
        .unwrap_or(Align::Center)
}

fn ident(v: &ResolvedValue) -> Option<&str> {
    match v {
        ResolvedValue::Ident(s) => Some(s.as_str()),
        _ => None,
    }
}

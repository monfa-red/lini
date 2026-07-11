//! Arrange a container's children per its `layout=` — the flow and grid modes,
//! and the interior gutters `gap-fill` paints between them [SPEC 11/12].

use super::*;

/// Interior gutter rects between adjacent flow children — at each gap's midpoint,
/// `gap` thick along the main axis and spanning the flow's cross extent ([SPEC 12],
/// the 1-D `gap-fill` case). Filled with the container's `gap-fill`.
fn one_d_gutters(
    children: &[PlacedNode],
    flow: &[usize],
    axis: Axis,
    flow_bbox: Bbox,
    gap: f64,
) -> Vec<Gutter> {
    let row = axis == Axis::Row;
    let main = |i: usize| if row { children[i].cx } else { children[i].cy };
    let half = |i: usize| {
        if row {
            children[i].bbox.w() / 2.0
        } else {
            children[i].bbox.h() / 2.0
        }
    };
    let (cx, cy) = (
        (flow_bbox.min_x + flow_bbox.max_x) / 2.0,
        (flow_bbox.min_y + flow_bbox.max_y) / 2.0,
    );
    let mut order: Vec<usize> = flow.to_vec();
    order.sort_by(|&a, &b| main(a).total_cmp(&main(b)));
    let mut out = Vec::new();
    for pair in order.windows(2) {
        let mid = (main(pair[0]) + half(pair[0]) + main(pair[1]) - half(pair[1])) / 2.0;
        if row {
            out.push((mid, cy, gap, flow_bbox.h()));
        } else {
            out.push((cx, mid, flow_bbox.w(), gap));
        }
    }
    out
}

/// Position children within their container per its `layout=` attr.
/// Returns the bounding bbox of all placed children, in container-local
/// coords. `scale` is the container's own effective `scale:` — a child's
/// `translate:` (and the container's declared content area) are drawing
/// units under it [SPEC 15.1].
pub(super) fn lay_out_container_children(
    children: &mut [PlacedNode],
    container_attrs: &crate::resolve::AttrMap,
    span: Span,
    scale: f64,
) -> Result<(Bbox, Vec<Gutter>), Error> {
    if children.is_empty() {
        return Ok((Bbox::empty(), Vec::new()));
    }

    // Split children by role [SPEC 5]: a `pin`ned child is an out-of-flow
    // overlay (the parent does not grow for it); everything else flows.
    let mut flow_indices: Vec<usize> = Vec::new();
    let mut pinned_indices: Vec<usize> = Vec::new();
    for (i, c) in children.iter().enumerate() {
        match anchors::child_role(&c.attrs, c.span)? {
            anchors::Role::Flow => flow_indices.push(i),
            anchors::Role::Pinned => pinned_indices.push(i),
        }
    }

    // Lay out the flow children per the container's `layout=` attr.
    let mode = read_layout_mode(container_attrs, span)?;
    // A flow's axis comes from `direction`; a grid has none.
    let flow_axis = match mode {
        LayoutMode::Flow => Some(read_flow_direction(container_attrs, span)?),
        LayoutMode::Grid => None,
    };
    // Slack for align/justify/stretch comes only from an explicit container
    // size: the content area is the declared dimension minus padding [SPEC 12].
    let pad = primitives::padding(container_attrs, span)?;
    let avail = (
        container_attrs
            .number("width")
            .map(|w| (w * scale - pad.left - pad.right).max(0.0)),
        container_attrs
            .number("height")
            .map(|h| (h * scale - pad.top - pad.bottom).max(0.0)),
    );

    let mut gutters: Vec<Gutter> = Vec::new();
    let flow_bbox = if !flow_indices.is_empty() {
        let mut flow_children: Vec<PlacedNode> =
            flow_indices.iter().map(|i| children[*i].clone()).collect();
        let bbox = match mode {
            LayoutMode::Flow => {
                let axis = flow_axis.expect("a flow has an axis");
                // The horizontal packing knob reaches a text leaf's lines
                // [SPEC 6]: `justify` on a row's main axis, `align` on a
                // column's cross axis.
                let knob = match axis {
                    Axis::Row => flex::ident(container_attrs.get("justify")),
                    Axis::Column => flex::ident(container_attrs.get("align")),
                };
                let la = line_align_of(knob);
                for c in flow_children.iter_mut() {
                    stamp_line_align(c, la);
                }
                flex::lay_out_flex(axis, &mut flow_children, container_attrs, span, avail)?
            }
            LayoutMode::Grid => {
                let (bbox, rects) = grid::lay_out_grid(&mut flow_children, container_attrs, span)?;
                gutters = rects;
                bbox
            }
        };
        for (slot, placed) in flow_indices.iter().zip(flow_children) {
            children[*slot] = placed;
        }
        bbox
    } else {
        Bbox::empty()
    };

    // Asymmetric padding offsets the flow within the box [SPEC 5]: the content
    // area is the box inset by `padding`, so the flow centre sits at
    // ((left−right)/2, (top−bottom)/2) from the box centre.
    let (off_x, off_y) = ((pad.left - pad.right) / 2.0, (pad.top - pad.bottom) / 2.0);
    if (off_x, off_y) != (0.0, 0.0) {
        for &i in &flow_indices {
            children[i].cx += off_x;
            children[i].cy += off_y;
        }
    }

    // 1-D gutters between flow children (a grid produced its own above), filled by
    // the container's `gap-fill` [SPEC 11] when set and the main-axis gap is
    // positive. They track the offset flow; the body bbox below stays centred,
    // since `closed_bbox` and pins anchor to it.
    if let Some(axis) = flow_axis
        && grid::has_gap_fill(container_attrs)
        && flow_indices.len() > 1
    {
        let (gap_y, gap_x) = primitives::gap(container_attrs, span)?;
        let main_gap = match axis {
            Axis::Row => gap_x,
            Axis::Column => gap_y,
        };
        if main_gap > 0.0 {
            gutters = one_d_gutters(
                children,
                &flow_indices,
                axis,
                flow_bbox.shifted(off_x, off_y),
                main_gap,
            );
        }
    }

    // The body the parent sizes to is the flow content alone — pinned children
    // are overlays that never grow it [SPEC 5].
    let body_bbox = flow_bbox;

    // Resolution box for pins: the parent's drawn shape — its padding included —
    // the same box `closed_bbox` sizes, and like it **centred** on the origin. An
    // explicit size gives it directly; otherwise it is the flow content plus
    // padding. Centring matters under asymmetric padding: an off-centre box would
    // drag a pinned caption/badge off the corner it anchors to.
    let anchor_parent_bbox = container_anchor_bbox(container_attrs, scale).unwrap_or_else(|| {
        Bbox::centered(
            body_bbox.w() + pad.left + pad.right,
            body_bbox.h() + pad.top + pad.bottom,
        )
    });

    // Pin out-of-flow children flush onto their parent anchor [SPEC 5]. The
    // parent does not grow for them — an all-pinned container with no explicit
    // size collapses — and the canvas still includes them (see `finish`), so an
    // overlay is never clipped.
    for &i in &pinned_indices {
        let pin = anchors::read_pin(&children[i].attrs, children[i].span)?
            .expect("pinned child carries pin:");
        let (cx, cy) = pin.target(anchor_parent_bbox, children[i].bbox);
        children[i].cx = cx;
        children[i].cy = cy;
    }

    // `translate:` nudges every node after placement [SPEC 5] — applied last,
    // once the body bbox is fixed, so it shifts the child (and its subtree, via
    // `cx`/`cy`) without reflowing siblings or growing the parent. A flow
    // child's translate is a position — drawing units under the parent's
    // `scale:` — while a pinned overlay's is chrome anatomy (a badge's nudge,
    // the title's gap) and stays sheet-space [SPEC 15.1].
    for (i, c) in children.iter_mut().enumerate() {
        if let Some((dx, dy)) = anchors::translate(&c.attrs, c.span)? {
            let s = if pinned_indices.contains(&i) {
                1.0
            } else {
                scale
            };
            c.cx += dx * s;
            c.cy += dy * s;
        }
    }

    Ok((body_bbox, gutters))
}

/// Container layout engine, parsed from the `layout=` attr. Chart/pie are a
/// separate engine intercepted in `layout_inst` *before* this runs, so this only
/// ever sees the box-arranger's two modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LayoutMode {
    /// 1-D flex; its axis comes from `direction` (`read_flow_direction`).
    Flow,
    /// 2D grid; sized by its `columns` / `rows` track lists (read in `grid`).
    Grid,
}

fn read_layout_mode(attrs: &crate::resolve::AttrMap, span: Span) -> Result<LayoutMode, Error> {
    match attrs.get("layout") {
        None => Ok(LayoutMode::Flow),
        Some(ResolvedValue::Ident(s)) => match s.as_str() {
            "flow" => Ok(LayoutMode::Flow),
            "grid" => Ok(LayoutMode::Grid),
            // Removed: orientation moved to `direction` [SPEC 11].
            dir @ ("row" | "column") => Err(Error::at(
                span,
                format!(
                    "'layout: {dir}' is not a layout — flow is the default; set 'direction: {dir}'"
                ),
            )),
            other => Err(Error::at(
                span,
                format!("unknown layout '{other}' — expected flow or grid"),
            )),
        },
        Some(_) => Err(Error::at(span, "'layout' expects flow or grid")),
    }
}

/// A flow's main axis from `direction` [SPEC 11], default `column`. `radial`
/// belongs to a chart, which owns its subtree and never reaches here.
fn read_flow_direction(attrs: &crate::resolve::AttrMap, span: Span) -> Result<Axis, Error> {
    match attrs.get("direction") {
        None => Ok(Axis::Column),
        Some(ResolvedValue::Ident(s)) => match s.as_str() {
            "column" => Ok(Axis::Column),
            "row" => Ok(Axis::Row),
            "radial" => Err(Error::at(
                span,
                "'direction: radial' is only valid in a chart — a flow is row or column",
            )),
            other => Err(Error::at(
                span,
                format!("unknown direction '{other}' — expected row or column"),
            )),
        },
        Some(_) => Err(Error::at(span, "'direction' expects row or column")),
    }
}

/// If the container declared explicit `width` *and* `height`, the children's
/// anchors resolve against those edges (no stroke pad — anchors live on the
/// drawn shape); otherwise they fall back to the body extent. The declared
/// dims are drawing units × the container's own `scale:` [SPEC 15.1].
fn container_anchor_bbox(attrs: &crate::resolve::AttrMap, scale: f64) -> Option<Bbox> {
    let w = attrs.number("width")?;
    let h = attrs.number("height")?;
    Some(Bbox::centered(w * scale, h * scale))
}

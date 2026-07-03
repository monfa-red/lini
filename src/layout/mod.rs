mod anchors;
mod chart;
mod flex;
mod grid;
pub(crate) mod ir;
pub(crate) mod path_bbox; // glyph-extent computation also serves `render::icon_fit`
mod prim; // PlacedNode *builders* for lowered primitives (charts, sequences)
mod primitives; // primitive *sizing* (leaf/closed bbox) — distinct from `prim`
pub(crate) mod sequence;
mod text;
mod values;

pub(crate) use anchors::is_pinned;
pub use ir::*;
pub(crate) use text::{approx_height, approx_width};
pub(crate) use values::as_pair;

use crate::error::Error;
use crate::resolve::{NodeKind, Program, ResolvedInst, ResolvedValue};
use crate::routing;
use crate::span::Span;

use flex::Axis;

/// Lay out the scene, then route its links over the finished, immutable
/// layout (ROUTING.md) — layout never moves for a link; whatever cannot be
/// drawn lawfully is reported and rendered as a stray.
pub fn layout(program: &Program) -> Result<LaidOut, Error> {
    sequence::validate(program)?;

    // Lay out top-level scene children.
    let mut top_nodes = Vec::with_capacity(program.scene.nodes.len());
    for inst in &program.scene.nodes {
        top_nodes.push(layout_inst(inst, &child_path("", inst), program)?);
    }

    // A root sequence (`{ layout: sequence }`, SPEC §10) owns the whole scene: it
    // arranges the participants and lowers its messages through the `straight`
    // strategy itself, bypassing the generic arrange and the orthogonal router.
    if sequence::is_sequence(&program.scene.attrs) {
        let (bbox, mut links) = sequence::layout_root(&mut top_nodes, program)?;
        links.extend(routing::owned_links(&top_nodes));
        let routed = routing::Routing {
            links,
            ..Default::default()
        };
        return finish(program, top_nodes, bbox, routed);
    }

    // Apply scene-level layout to top-level children (scene itself is a
    // container; its attrs drive how its children are positioned). The scene
    // is never a table, so its grid rules — if any — are discarded.
    let (bbox, _) =
        lay_out_container_children(&mut top_nodes, &program.scene.attrs, Span::empty())?;

    // Route links once the nodes are placed.
    let routed = routing::route(program, &top_nodes)?;
    finish(program, top_nodes, bbox, routed)
}

/// The scene instance at a dot-path (`""` → `None`: the root is not an instance).
/// Walks by id, like an endpoint path. Used by the sequence engine.
pub(super) fn node_at<'a>(program: &'a Program, path: &str) -> Option<&'a ResolvedInst> {
    let mut nodes = &program.scene.nodes;
    let mut found = None;
    for seg in path.split('.') {
        let inst = nodes.iter().find(|n| n.id.as_deref() == Some(seg))?;
        found = Some(inst);
        nodes = &inst.children;
    }
    found
}

/// A child's dot-path under `parent`. Anonymous children get a `#` segment —
/// never addressable, so never a link endpoint's ancestor.
fn child_path(parent: &str, inst: &ResolvedInst) -> String {
    let id = inst.id.as_deref().unwrap_or("#");
    if parent.is_empty() {
        id.to_owned()
    } else {
        format!("{parent}.{id}")
    }
}

/// Union every node's drawn extent into `bbox`, in world coords — so the
/// canvas includes absolute overlays that don't grow their parent's bbox.
fn accumulate_extent(n: &PlacedNode, ox: f64, oy: f64, bbox: &mut Bbox) {
    let (wx, wy) = (ox + n.cx, oy + n.cy);
    *bbox = bbox.union(n.bbox.shifted(wx, wy));
    for c in &n.children {
        accumulate_extent(c, wx, wy, bbox);
    }
}

fn finish(
    program: &Program,
    nodes: Vec<PlacedNode>,
    scene_bbox: Bbox,
    routing: routing::Routing,
) -> Result<LaidOut, Error> {
    // Viewbox = the whole drawn extent (scene bbox + link paths, labels, strays,
    // overlays) framed by the scene's `padding` on every side — the margin between
    // the diagram and the SVG edge.
    let pad = primitives::padding(&program.scene.attrs, Span::empty())?;
    // Absolute overlays don't grow their parent's bbox, so the scene bbox can
    // miss one that overflows; the canvas must still include every drawn node,
    // so take the true visual extent of the whole tree.
    let mut bbox = scene_bbox;
    for n in &nodes {
        accumulate_extent(n, 0.0, 0.0, &mut bbox);
    }
    let link_points = routing.links.iter().flat_map(|w| &w.path);
    let air_points = routing.strays.iter().flat_map(|a| [&a.from, &a.to]);
    for &(x, y) in link_points.chain(air_points) {
        bbox.min_x = bbox.min_x.min(x);
        bbox.min_y = bbox.min_y.min(y);
        bbox.max_x = bbox.max_x.max(x);
        bbox.max_y = bbox.max_y.max(y);
    }
    for t in routing.links.iter().flat_map(|w| &w.texts) {
        let size = t.attrs.number("font-size").unwrap_or(0.0);
        let ls = t.attrs.number("letter-spacing").unwrap_or(0.0);
        let lsp = t.attrs.number("line-spacing").unwrap_or(0.0);
        let (hw, hh) = (
            text::approx_width(&t.content, size, ls) / 2.0,
            text::approx_height(&t.content, size, lsp) / 2.0,
        );
        bbox.min_x = bbox.min_x.min(t.position.0 - hw);
        bbox.min_y = bbox.min_y.min(t.position.1 - hh);
        bbox.max_x = bbox.max_x.max(t.position.0 + hw);
        bbox.max_y = bbox.max_y.max(t.position.1 + hh);
    }
    let vb = ViewBox {
        x: bbox.min_x - pad.left,
        y: bbox.min_y - pad.top,
        w: bbox.w() + pad.left + pad.right,
        h: bbox.h() + pad.top + pad.bottom,
    };

    // A root `fill:` overrides the canvas colour inline (SPEC §13); the default
    // comes from the `.lini-canvas` rule (`--lini-bg`). `none` → transparent.
    let canvas_fill = program.scene.attrs.get("fill").cloned();

    Ok(LaidOut {
        viewbox: vb,
        nodes,
        links: routing.links,
        link_report: routing.report,
        strays: routing.strays,
        vars: program.vars.clone(),
        sheet: program.sheet.clone(),
        canvas_fill,
        gradients: Vec::new(),
    })
}

/// Validate a laid-out scene's links against the routing contract (ROUTING.md):
/// the engine's own report (drawn crossings, impossible links), then the
/// independent four-law check. Used by `lini::validate_str`.
pub fn validate_routing(laid: &LaidOut) -> Vec<routing::Violation> {
    let mut out = laid.link_report.clone();
    out.extend(routing::validate_routing(
        &laid.nodes,
        &laid.links,
        &laid.link_report,
    ));
    out
}

/// Recursively lay out a single instance into a PlacedNode.
///
/// Bottom-up: lay out children first, then size this node around them. For
/// leaf primitives (no children), the shape's dimensions drive the bbox.
/// `path` is the inst's dot-path — how a sequence scope finds its messages.
fn layout_inst(inst: &ResolvedInst, path: &str, program: &Program) -> Result<PlacedNode, Error> {
    let funcs = &program.funcs;
    // A chart ([CHARTS.md]) owns its whole subtree: it reads its children's data,
    // fixes a shared scale, samples any `fn:`, and emits primitive PlacedNodes itself.
    // Intercept it here — before the child recursion (which would run `leaf_bbox` on a
    // series with no `points:`) and before the flow/grid path (`read_layout_mode`
    // only handles flow and grid).
    if chart::is_chart(&inst.attrs) {
        return chart::layout_chart(inst, funcs);
    }
    if chart::is_pie(&inst.attrs) {
        return chart::layout_pie(inst);
    }
    // A `|sequence|` node ([SPEC §10]) owns its subtree the same way — it reads its
    // participants (and, later, messages / frames / notes) and lowers to primitives.
    if sequence::is_sequence(&inst.attrs) {
        return sequence::layout_node(inst, path, program);
    }

    // Recurse into children first.
    let mut children: Vec<PlacedNode> = Vec::with_capacity(inst.children.len());
    for c in &inst.children {
        children.push(layout_inst(c, &child_path(path, c), program)?);
    }

    // Determine this node's bbox + arrange children inside.
    let mut gutters: Vec<Gutter> = Vec::new();
    let bbox = if children.is_empty() {
        // Leaf primitive.
        primitives::leaf_bbox(inst)?
    } else {
        // Container or closed primitive with content.
        let (content_bbox, rects) =
            lay_out_container_children(&mut children, &inst.attrs, inst.span)?;

        // Interior gutters (grid or 1-D) the container fills with `gap-color`.
        // A table is just a group with `gap-color: --stroke` — no special-casing;
        // its border is the group rect, its inner rules these gutter rects.
        gutters = rects;

        // An icon sizes to a square that grows with its label child (SPEC §7);
        // every other closed primitive sizes border-box — explicit width/height,
        // else content + padding per axis (SPEC §6).
        let b = if inst.kind == NodeKind::Icon {
            primitives::icon_square_bbox(inst, content_bbox)?
        } else {
            primitives::closed_bbox(inst, content_bbox)?
        };
        let text_only = children.iter().all(|c| c.kind == NodeKind::Text);

        // Some closed shapes carry decoration at the top — a cloud's lobes, a
        // cylinder's rim — so the optical body-center sits below the bbox center
        // and a centered label reads too high. Drop a text-only label into the
        // body by a per-primitive fraction of the height (the outlines are
        // scale-invariant, so a fraction holds at any size).
        const CYL_LABEL_DROP: f64 = 0.03;
        let label_drop = match inst.kind {
            NodeKind::Cyl => CYL_LABEL_DROP,
            _ => 0.0,
        };
        if label_drop > 0.0 && text_only {
            let dy = b.h() * label_drop;
            for c in &mut children {
                c.cy += dy;
            }
        }

        b
    };

    let rotation = inst.attrs.number("rotate").unwrap_or(0.0);

    Ok(PlacedNode {
        id: inst.id.clone(),
        kind: inst.kind,
        type_chain: inst.type_chain.clone(),
        applied_styles: inst.applied_styles.clone(),
        label: inst.label.clone(),
        attrs: inst.attrs.clone(),
        own_style: inst.own_style.clone(),
        markers: inst.markers.clone(),
        cx: 0.0,
        cy: 0.0,
        bbox,
        rotation,
        children,
        gutters,
        links: Vec::new(),
        span: inst.span,
    })
}

/// Interior gutter rects between adjacent flow children — at each gap's midpoint,
/// `gap` thick along the main axis and spanning the flow's cross extent (SPEC §5,
/// the 1-D `gap-color` case). Filled with the container's `gap-color`.
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
/// coords.
fn lay_out_container_children(
    children: &mut [PlacedNode],
    container_attrs: &crate::resolve::AttrMap,
    span: Span,
) -> Result<(Bbox, Vec<Gutter>), Error> {
    if children.is_empty() {
        return Ok((Bbox::empty(), Vec::new()));
    }

    // Split children by role (SPEC §6): a `pin`ned child is an out-of-flow
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
    // size: the content area is the declared dimension minus padding (SPEC §5).
    let pad = primitives::padding(container_attrs, span)?;
    let avail = (
        container_attrs
            .number("width")
            .map(|w| (w - pad.left - pad.right).max(0.0)),
        container_attrs
            .number("height")
            .map(|h| (h - pad.top - pad.bottom).max(0.0)),
    );

    let mut gutters: Vec<Gutter> = Vec::new();
    let flow_bbox = if !flow_indices.is_empty() {
        let mut flow_children: Vec<PlacedNode> =
            flow_indices.iter().map(|i| children[*i].clone()).collect();
        let bbox = match mode {
            LayoutMode::Flow => flex::lay_out_flex(
                flow_axis.expect("a flow has an axis"),
                &mut flow_children,
                container_attrs,
                span,
                avail,
            )?,
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

    // Asymmetric padding offsets the flow within the box (SPEC §6): the content
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
    // the container's `gap-color` (SPEC §5) when set and the main-axis gap is
    // positive. They track the offset flow; the body bbox below stays centred,
    // since `closed_bbox` and pins anchor to it.
    if let Some(axis) = flow_axis
        && grid::has_gap_color(container_attrs)
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
    // are overlays that never grow it (SPEC §6).
    let body_bbox = flow_bbox;

    // Resolution box for pins: the parent's drawn shape — its padding included —
    // the same box `closed_bbox` sizes, and like it **centred** on the origin. An
    // explicit size gives it directly; otherwise it is the flow content plus
    // padding. Centring matters under asymmetric padding: an off-centre box would
    // drag a pinned caption/badge off the corner it anchors to.
    let anchor_parent_bbox = container_anchor_bbox(container_attrs).unwrap_or_else(|| {
        Bbox::centered(
            body_bbox.w() + pad.left + pad.right,
            body_bbox.h() + pad.top + pad.bottom,
        )
    });

    // Pin out-of-flow children flush onto their parent anchor (SPEC §6). The
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

    // `translate:` nudges every node after placement (SPEC §6) — applied last,
    // once the body bbox is fixed, so it shifts the child (and its subtree, via
    // `cx`/`cy`) without reflowing siblings or growing the parent.
    for c in children.iter_mut() {
        if let Some((dx, dy)) = anchors::translate(&c.attrs, c.span)? {
            c.cx += dx;
            c.cy += dy;
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
            // Removed: orientation moved to `direction` (SPEC §5).
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

/// A flow's main axis from `direction` (SPEC §5), default `column`. `radial`
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
/// drawn shape); otherwise they fall back to the body extent.
fn container_anchor_bbox(attrs: &crate::resolve::AttrMap) -> Option<Bbox> {
    let w = attrs.number("width")?;
    let h = attrs.number("height")?;
    Some(Bbox::centered(w, h))
}

// ───────────────────────────── Tests ─────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn lay_out(src: &str) -> LaidOut {
        let tokens = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(&tokens).expect("parse");
        let lowered = crate::desugar::desugar(&file).expect("desugar");
        let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
        layout(&program).expect("layout")
    }

    // ── Sizing (SPEC §6) ──

    #[test]
    fn empty_closed_primitive_is_two_paddings() {
        // padding 20 each side → 40 drawn; + stroke 1.6 → 41.6 bbox.
        let n = &lay_out("|box|\n").nodes[0];
        assert!((n.bbox.w() - 41.6).abs() < 0.01, "w={}", n.bbox.w());
        assert!((n.bbox.h() - 41.6).abs() < 0.01, "h={}", n.bbox.h());
    }

    #[test]
    fn explicit_dims_are_border_box() {
        let n = &lay_out("|box| { width: 100; height: 50; }\n").nodes[0];
        assert!((n.bbox.w() - 101.6).abs() < 0.01, "w={}", n.bbox.w());
        assert!((n.bbox.h() - 51.6).abs() < 0.01, "h={}", n.bbox.h());
    }

    #[test]
    fn stroke_width_counts_toward_the_bbox() {
        // SPEC §6: width 100 height 50 stroke-width 4 → 104×54.
        let n = &lay_out("|box| { width: 100; height: 50; stroke-width: 4; }\n").nodes[0];
        assert!((n.bbox.w() - 104.0).abs() < 0.01, "w={}", n.bbox.w());
        assert!((n.bbox.h() - 54.0).abs() < 0.01, "h={}", n.bbox.h());
    }

    #[test]
    fn label_auto_sizes_to_content_plus_padding() {
        // text ~18 + 2×20 padding + 2 stroke → ~60.
        let n = &lay_out("|box| \"hi\"\n").nodes[0];
        assert!(n.bbox.w() > 55.0 && n.bbox.w() < 65.0, "w={}", n.bbox.w());
    }

    #[test]
    fn dims_are_independent_per_axis() {
        let n = &lay_out("|box| \"hi\" { width: 200 }\n").nodes[0];
        assert!((n.bbox.w() - 201.6).abs() < 0.01, "w={}", n.bbox.w());
        // height auto = one text line (15) + 40 padding + 1.6 stroke = 56.6.
        assert!((n.bbox.h() - 56.6).abs() < 0.01, "h={}", n.bbox.h());
    }

    #[test]
    fn explicit_size_is_a_floor_not_a_clip() {
        // Content wider than the declared width grows the box instead of spilling.
        let grown = &lay_out("|box| \"a long label\" { width: 40 }\n").nodes[0];
        assert!(
            grown.bbox.w() > 60.0,
            "floor grows to content: w={}",
            grown.bbox.w()
        );
        // A width the content fits within is honoured exactly (border-box + stroke).
        let kept = &lay_out("|box| \"hi\" { width: 300 }\n").nodes[0];
        assert!((kept.bbox.w() - 301.6).abs() < 0.01, "w={}", kept.bbox.w());
    }

    #[test]
    fn asymmetric_padding_offsets_the_content() {
        // padding t r b l = 0 0 0 20 → 20 on the left, 0 on the right, so the
        // content shifts right by (20 − 0)/2 = 10.
        let off = &lay_out("|box| \"x\" { padding: 0 0 0 20 }\n").nodes[0];
        assert!(
            (off.children[0].cx - 10.0).abs() < 0.01,
            "cx={}",
            off.children[0].cx
        );
        // Symmetric padding keeps it centred.
        let mid = &lay_out("|box| \"x\" { padding: 8 }\n").nodes[0];
        assert!(
            mid.children[0].cx.abs() < 0.01,
            "centred: cx={}",
            mid.children[0].cx
        );
    }

    #[test]
    fn oval_uses_width_height() {
        let n = &lay_out("|oval| { width: 100; height: 50; }\n").nodes[0];
        assert!((n.bbox.w() - 101.6).abs() < 0.01, "w={}", n.bbox.w());
        assert!((n.bbox.h() - 51.6).abs() < 0.01, "h={}", n.bbox.h());
    }

    #[test]
    fn text_sizes_to_its_glyphs_without_padding() {
        let n = &lay_out("\"hi\"\n").nodes[0];
        assert!((n.bbox.w() - 18.0).abs() < 0.5, "w={}", n.bbox.w()); // 2 × 15 × 0.6
        assert!((n.bbox.h() - 15.0).abs() < 0.5, "h={}", n.bbox.h());
    }

    // ── Basic flow (full align/justify/stretch/evenly land in the flex chunk) ──

    #[test]
    fn row_layout_stacks_horizontally() {
        let l = lay_out(
            "{ direction: row; gap: 10; }\n\
             |box| { width: 100; height: 40; }\n\
             |box| { width: 60; height: 40; }\n",
        );
        assert_eq!(l.nodes.len(), 2);
        // half (50.8) + gap (10) + half (30.8) = 91.6.
        let dx = l.nodes[1].cx - l.nodes[0].cx;
        assert!((dx - 91.6).abs() < 0.5, "dx={}", dx);
        assert!((l.nodes[0].cy - l.nodes[1].cy).abs() < 0.01);
    }

    #[test]
    fn column_layout_stacks_vertically() {
        let l = lay_out(
            "{ direction: column; gap: 20; }\n\
             |box| { width: 100; height: 40; }\n\
             |box| { width: 100; height: 60; }\n",
        );
        // half (20.8) + gap (20) + half (30.8) = 71.6.
        let dy = l.nodes[1].cy - l.nodes[0].cy;
        assert!((dy - 71.6).abs() < 0.5, "dy={}", dy);
        assert!((l.nodes[0].cx - l.nodes[1].cx).abs() < 0.01);
    }

    fn lay_out_err(src: &str) -> Error {
        let tokens = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(&tokens).expect("parse");
        let lowered = crate::desugar::desugar(&file).expect("desugar");
        let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
        match layout(&program) {
            Ok(_) => panic!("expected a layout error"),
            Err(e) => e,
        }
    }

    #[test]
    fn layout_row_and_column_are_removed() {
        for dir in ["row", "column"] {
            let err = lay_out_err(&format!("{{ layout: {dir}; }}\n|box|\n|box|\n"));
            assert!(
                err.message.contains(&format!("direction: {dir}")),
                "msg={}",
                err.message
            );
        }
    }

    #[test]
    fn direction_radial_is_rejected_in_a_flow() {
        let err = lay_out_err("{ direction: radial; }\n|box|\n|box|\n");
        assert!(err.message.contains("chart"), "msg={}", err.message);
    }

    #[test]
    fn viewbox_wraps_content_with_scene_padding() {
        // bbox 101.6×41.6, + the scene's 20 padding each side → 141.6×81.6.
        let l = lay_out("|box| { width: 100; height: 40; }\n");
        assert!((l.viewbox.w - 141.6).abs() < 0.01, "w={}", l.viewbox.w);
        assert!((l.viewbox.h - 81.6).abs() < 0.01, "h={}", l.viewbox.h);
    }

    // ── Captions: ordinary flow children (SPEC §8) ──

    #[test]
    fn caption_overlay_does_not_grow_the_group() {
        // A caption pins to the top edge (an overlay), so it reserves no flow
        // row — the group sizes to its content alone, with or without it.
        let h = |src: &str| lay_out(src).nodes[0].bbox.h();
        let plain = h("|group#g| [\n  |box#a| { width: 80; height: 30; }\n]\n");
        let capped =
            h("|group#g| [\n  |caption| \"Cap\"\n  |box#a| { width: 80; height: 30; }\n]\n");
        assert!(
            (capped - plain).abs() < 0.01,
            "caption is an overlay, no extra height: plain={plain} capped={capped}"
        );
    }

    #[test]
    fn caption_sits_above_the_content() {
        let l =
            lay_out("|group#g| [\n  |caption| \"Cap\"\n  |box#a| { width: 80; height: 30; }\n]\n");
        let g = &l.nodes[0];
        let cap = g
            .children
            .iter()
            .find(|c| c.type_chain.iter().any(|t| t == "caption"))
            .expect("caption child");
        let a = g
            .children
            .iter()
            .find(|c| c.id.as_deref() == Some("a"))
            .expect("box child");
        assert!(cap.cy < a.cy, "cap.cy={} a.cy={}", cap.cy, a.cy);
    }

    // ── Flex distribution with slack (SPEC §5) ──

    #[test]
    fn justify_orders_children_start_center_end() {
        let first_cx = |j: &str| {
            let src = format!(
                "|row#g| {{ width: 300; justify: {j} }} [\n  |box#a| {{ width: 40; height: 20; }}\n  |box#b| {{ width: 40; height: 20; }}\n]\n"
            );
            lay_out(&src).nodes[0].children[0].cx
        };
        let (start, center, end) = (first_cx("start"), first_cx("center"), first_cx("end"));
        assert!(
            start < center && center < end,
            "start={start} center={center} end={end}"
        );
    }

    #[test]
    fn justify_evenly_spaces_children_equally() {
        let l = lay_out(
            "|row#g| { width: 300; justify: evenly } [\n  |box#a| { width: 20; height: 20; }\n  |box#b| { width: 20; height: 20; }\n  |box#c| { width: 20; height: 20; }\n]\n",
        );
        let cx: Vec<f64> = l.nodes[0].children.iter().map(|c| c.cx).collect();
        assert!(
            ((cx[1] - cx[0]) - (cx[2] - cx[1])).abs() < 0.01,
            "centers {cx:?}"
        );
    }

    #[test]
    fn align_stretch_fills_the_cross_axis() {
        // An unsized child grows to the row's content height (row pads 0).
        let l = lay_out("|row#g| { height: 80; align: stretch } [\n  |box#a| { width: 40; }\n]\n");
        let a = &l.nodes[0].children[0];
        assert!((a.bbox.h() - 80.0).abs() < 1.0, "a.h={}", a.bbox.h());
    }

    #[test]
    fn no_slack_means_no_distribution() {
        // An auto-width row ignores justify — children stay packed at the gap.
        let span = |j: &str| {
            let src = format!(
                "|row#g| {{ justify: {j} }} [\n  |box#a| {{ width: 40; height: 20; }}\n  |box#b| {{ width: 40; height: 20; }}\n]\n"
            );
            let l = lay_out(&src);
            l.nodes[0].children[1].cx - l.nodes[0].children[0].cx
        };
        assert!(
            (span("start") - span("end")).abs() < 0.01,
            "auto row: justify is a no-op"
        );
    }

    // ── Grid (SPEC §5) ──

    #[test]
    fn grid_fixed_columns_place_children_in_order() {
        let l = lay_out(
            "{ layout: grid; columns: 80 80 80; gap: 0; }\n\
             |box#a| { width: 40; height: 40; }\n\
             |box#b| { width: 40; height: 40; }\n\
             |box#c| { width: 40; height: 40; }\n",
        );
        let cx: Vec<f64> = l.nodes.iter().map(|n| n.cx).collect();
        assert!((cx[1] - cx[0] - 80.0).abs() < 0.5, "dx={}", cx[1] - cx[0]);
        assert!((cx[2] - cx[1] - 80.0).abs() < 0.5);
        assert!((l.nodes[0].cy - l.nodes[1].cy).abs() < 0.01);
    }

    #[test]
    fn grid_repeat_makes_auto_columns_and_wraps() {
        let l = lay_out(
            "{ layout: grid; columns: repeat(2); }\n\
             |box#a| { width: 30; height: 30; }\n\
             |box#b| { width: 30; height: 30; }\n\
             |box#c| { width: 30; height: 30; }\n",
        );
        // 2 columns, 3 children → c wraps to the second row.
        assert!(l.nodes[2].cy > l.nodes[0].cy, "c below a");
    }

    #[test]
    fn grid_cell_pins_placement() {
        let l = lay_out(
            "{ layout: grid; columns: repeat(3); }\n\
             |box#a| { cell: 3 1; }\n\
             |box#b|\n",
        );
        // a pins to column 3; b auto-flows to the first free cell (column 1).
        assert!(
            l.nodes[0].cx > l.nodes[1].cx,
            "a (col 3) right of b (col 1)"
        );
    }

    #[test]
    fn grid_cell_fills_its_track_under_stretch() {
        let l = lay_out(
            "{ layout: grid; columns: 120 120; gap: 0; }\n\
             |box#a| { justify: stretch; align: stretch; }\n\
             |box#b|\n",
        );
        assert!(
            (l.nodes[0].bbox.w() - 120.0).abs() < 1.0,
            "a.w={}",
            l.nodes[0].bbox.w()
        );
    }

    #[test]
    fn grid_rows_track_list_is_a_floor_implicit_rows_overflow() {
        // SPEC §5/§20: a declared `rows` track list sizes the first rows; extra
        // children flow into implicit auto rows (CSS grid) rather than erroring.
        // Here 2 cols × 1 declared row track, 4 children → a second, implicit row.
        let l = lay_out(
            "{ layout: grid; columns: 40 40; rows: auto; }\n\
             |box#a| { width: 30; height: 30; }\n\
             |box#b| { width: 30; height: 30; }\n\
             |box#c| { width: 30; height: 30; }\n\
             |box#d| { width: 30; height: 30; }\n",
        );
        assert!(l.nodes[2].cy > l.nodes[0].cy, "c (row 2) below a (row 1)");
        assert!(
            (l.nodes[2].cy - l.nodes[3].cy).abs() < 0.01,
            "c, d share row 2"
        );
    }

    #[test]
    fn grid_without_columns_is_an_error() {
        let tokens = crate::lexer::lex("{ layout: grid; }\n|box#a|\n|box#b|\n").expect("lex");
        let file = crate::syntax::parser::parse(&tokens).expect("parse");
        let lowered = crate::desugar::desugar(&file).expect("desugar");
        let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
        assert!(layout(&program).is_err());
    }

    // ── Gutters (SPEC §5) ──

    #[test]
    fn table_fills_interior_gutters_no_frame() {
        let l = lay_out("|table#t| { columns: 40 40 } [\n  \"a\" \"b\" \"c\" \"d\"\n]\n");
        // The table's `gap-color: --stroke` fills the interior gutters.
        assert!(!l.nodes[0].gutters.is_empty(), "table has interior gutters");
        // A plain group has no `gap-color`, so no gutters.
        assert!(
            lay_out("|group#g| [ |box#x| ]\n").nodes[0]
                .gutters
                .is_empty()
        );
    }

    #[test]
    fn grid_gutters_stay_within_the_content_box() {
        // Interior gutter rects must not overshoot the frame: every rect sits fully
        // inside the grid's own content box.
        let l = lay_out(
            "|table#t| { columns: 40 40; gap: 20 } [\n  \"a\"\n  \"b\"\n  \"c\"\n  \"d\"\n]\n",
        );
        let t = &l.nodes[0];
        let (hw, hh) = (t.bbox.w() / 2.0 + 0.01, t.bbox.h() / 2.0 + 0.01);
        for (cx, cy, w, h) in &t.gutters {
            assert!(cx.abs() + w / 2.0 <= hw, "gutter x {cx}±{} > {hw}", w / 2.0);
            assert!(cy.abs() + h / 2.0 <= hh, "gutter y {cy}±{} > {hh}", h / 2.0);
        }
    }

    #[test]
    fn one_d_gutter_falls_between_flow_children() {
        let l = lay_out(
            "|row#g| { gap-color: --stroke } [\n  |box#a| { width: 30; height: 30; }\n  |box#b| { width: 30; height: 30; }\n  |box#c| { width: 30; height: 30; }\n]\n",
        );
        assert_eq!(
            l.nodes[0].gutters.len(),
            2,
            "two gutters between three children"
        );
    }

    #[test]
    fn gap_color_per_axis_selects_gutters() {
        // `gap: row col` (SPEC §5): `4 0` paints row rules (horizontal gutters), `0 4`
        // column rules (vertical). A 2×2 grid has one interior boundary each way.
        let rows_only = lay_out(
            "|grid#g| { columns: 40 40; gap: 4 0; gap-color: --stroke } [\n  \"a\" \"b\"\n  \"c\" \"d\"\n]\n",
        );
        let (_, _, w, h) = rows_only.nodes[0].gutters[0];
        assert_eq!(rows_only.nodes[0].gutters.len(), 1, "row gap → one gutter");
        assert!(w > h, "horizontal gutter is wide: w={w} h={h}");

        let cols_only = lay_out(
            "|grid#g| { columns: 40 40; gap: 0 4; gap-color: --stroke } [\n  \"a\" \"b\"\n  \"c\" \"d\"\n]\n",
        );
        let (_, _, w2, h2) = cols_only.nodes[0].gutters[0];
        assert_eq!(cols_only.nodes[0].gutters.len(), 1, "col gap → one gutter");
        assert!(h2 > w2, "vertical gutter is tall: w={w2} h={h2}");
    }

    #[test]
    fn a_filled_grid_cell_aligns_its_text_by_its_own_align() {
        // A grid cell filled by the container's `align: stretch` then honours its
        // own `align` (↔) to place its text (SPEC §5) — the generic rule tables use.
        let text_cx = |a: &str| {
            let src = format!(
                "|grid#g| {{ columns: 200; align: stretch }} [\n  |block#c| \"x\" {{ align: {a} }}\n]\n"
            );
            let l = lay_out(&src);
            let text = &l.nodes[0].children[0].children[0];
            assert_eq!(text.kind, NodeKind::Text);
            text.cx
        };
        // The cell fills the 200-wide track; `start` hugs the text left of centre,
        // `end` right, `center` stays centred.
        assert!(text_cx("start") < -50.0, "start: {}", text_cx("start"));
        assert!(text_cx("end") > 50.0, "end: {}", text_cx("end"));
        assert!(
            text_cx("center").abs() < 5.0,
            "center: {}",
            text_cx("center")
        );
    }
}

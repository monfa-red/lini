mod anchors;
mod chart;
pub(crate) mod drawing;
mod flex;
mod grid;
pub(crate) mod ir;
mod note;
pub(crate) mod path_bbox; // glyph-extent computation also serves `render::icon_fit`
mod pattern;
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

    // A root drawing (`{ layout: drawing }`, [SPEC 15]) owns the whole scene:
    // its children datum-place, mates seat them, and its links never route —
    // intercepted before the generic per-child layout, which would flow-arrange
    // features and reject the chrome.
    if drawing::is_drawing(&program.scene.attrs) {
        let (top_nodes, bbox) = drawing::layout_root(program)?;
        let links = routing::owned_links(&top_nodes);
        let routed = routing::Routing {
            links,
            ..Default::default()
        };
        return finish(program, top_nodes, bbox, routed);
    }

    let ctx = Ctx {
        scale: effective_scale(&program.scene.attrs, 1.0, Span::empty())?,
        drawing: false,
    };

    // Lay out top-level scene children.
    let mut top_nodes = Vec::with_capacity(program.scene.nodes.len());
    for inst in &program.scene.nodes {
        top_nodes.push(layout_inst(inst, &child_path("", inst), program, ctx)?);
    }

    // A root sequence (`{ layout: sequence }`, [SPEC 13]) owns the whole scene: it
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
    let (bbox, _) = lay_out_container_children(
        &mut top_nodes,
        &program.scene.attrs,
        Span::empty(),
        ctx.scale,
    )?;

    // Route links once the nodes are placed.
    let routed = routing::route(program, &top_nodes)?;
    finish(program, top_nodes, bbox, routed)
}

/// The layout context a node inherits [SPEC 15]: the parent's effective
/// `scale:` (px per drawing unit — nearest ancestor wins) and whether the node
/// sits in a drawing scope, where a shape's `[ ]` children datum-place as its
/// features. Layout-owning engines (chart / pie / sequence) reset it — their
/// interiors are sheet-space.
#[derive(Clone, Copy)]
pub(crate) struct Ctx {
    pub scale: f64,
    pub drawing: bool,
}

impl Ctx {
    pub(crate) fn sheet() -> Self {
        Ctx {
            scale: 1.0,
            drawing: false,
        }
    }
}

/// A node's effective `scale:` — its own when set (must be > 0, [SPEC 20]),
/// else the inherited one.
pub(crate) fn effective_scale(
    attrs: &crate::resolve::AttrMap,
    inherited: f64,
    span: Span,
) -> Result<f64, Error> {
    match attrs.get("scale") {
        None => Ok(inherited),
        Some(v) => match v.as_number() {
            Some(s) if s > 0.0 => Ok(s),
            _ => Err(Error::at(span, "'scale' must be > 0")),
        },
    }
}

/// The attrs of the container at `scope` (`""` = the scene root) — shared by
/// the sequence's and the drawing's scope detectors.
pub(crate) fn scope_attrs<'a>(
    program: &'a Program,
    scope: &str,
) -> Option<&'a crate::resolve::AttrMap> {
    if scope.is_empty() {
        Some(&program.scene.attrs)
    } else {
        node_at(program, scope).map(|i| &i.attrs)
    }
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
/// `rot` is the accumulated ancestor rotation: each node renders as
/// `translate(cx, cy) rotate(deg)`, so a turned node's true extent is its
/// bbox corners swung about its origin — without this, a `rotate:`d part
/// (a mated bar stood on end, [SPEC 15.5]) clips at the canvas edge.
fn accumulate_extent(n: &PlacedNode, ox: f64, oy: f64, rot: f64, bbox: &mut Bbox) {
    let turn = |x: f64, y: f64, deg: f64| -> (f64, f64) {
        if deg == 0.0 {
            return (x, y);
        }
        let (s, c) = deg.to_radians().sin_cos();
        (x * c - y * s, x * s + y * c)
    };
    let (dx, dy) = turn(n.cx, n.cy, rot);
    let (wx, wy) = (ox + dx, oy + dy);
    let total = rot + n.rotation;
    let b = &n.bbox;
    for (x, y) in [
        (b.min_x, b.min_y),
        (b.max_x, b.min_y),
        (b.min_x, b.max_y),
        (b.max_x, b.max_y),
    ] {
        let (px, py) = turn(x, y, total);
        *bbox = bbox.union(Bbox {
            min_x: wx + px,
            min_y: wy + py,
            max_x: wx + px,
            max_y: wy + py,
        });
    }
    for c in &n.children {
        accumulate_extent(c, wx, wy, total, bbox);
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
        accumulate_extent(n, 0.0, 0.0, 0.0, &mut bbox);
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

    // A root `fill:` overrides the canvas colour inline [SPEC 17]; the default
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
        hatches: Vec::new(),
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
fn layout_inst(
    inst: &ResolvedInst,
    path: &str,
    program: &Program,
    ctx: Ctx,
) -> Result<PlacedNode, Error> {
    let funcs = &program.funcs;
    // `break:` clips a folded profile — only a `|sketch|` has one [SPEC 15.3].
    if inst.attrs.get("break").is_some() && inst.kind != NodeKind::Sketch {
        return Err(Error::at(
            inst.span,
            "'break' cuts a '|sketch|' — draw the profile with the pen",
        ));
    }
    // A layout-owning engine (chart / pie / sequence / drawing) owns its whole
    // subtree and emits primitive PlacedNodes itself — intercepted before the
    // child recursion (which would run `leaf_bbox` on a series with no
    // `points:`) and before the flow/grid path. `pattern:` still applies to
    // the finished box — it is a node property, any node [SPEC 15.4].
    let engine = if chart::is_chart(&inst.attrs) {
        Some(chart::layout_chart(inst, funcs)?)
    } else if chart::is_pie(&inst.attrs) {
        Some(chart::layout_pie(inst)?)
    } else if sequence::is_sequence(&inst.attrs) {
        Some(sequence::layout_node(inst, path, program)?)
    } else if drawing::is_drawing(&inst.attrs) {
        Some(drawing::layout_node(inst, path, program, ctx)?)
    } else {
        None
    };
    if let Some(mut placed) = engine {
        if placed.attrs.get("pattern").is_some() {
            let own = effective_scale(&inst.attrs, ctx.scale, inst.span)?;
            pattern::expand(&mut placed, own)?;
        }
        return Ok(placed);
    }
    // Generated drawing chrome ([SPEC 15.7]) has no geometry of its own — the
    // parent's shape decides it once that shape is sized (below).
    if ctx.drawing && drawing::chrome::is_chrome(&inst.attrs) {
        return Ok(drawing::chrome::placeholder(inst));
    }

    let own = effective_scale(&inst.attrs, ctx.scale, inst.span)?;
    // In a drawing scope a shape's `[ ]` children are its **features** — they
    // datum-place at the part's origin, rigid with it [SPEC 15.4]; a child that
    // owns a layout — or is sheet content (a note, the title) — arranges its
    // interior as usual and places as one box.
    let part =
        ctx.drawing && !owns_layout(&inst.attrs) && !drawing::is_sheet(inst.kind, &inst.type_chain);
    let child_ctx = Ctx {
        scale: own,
        drawing: part,
    };

    // Recurse into children first.
    let mut children: Vec<PlacedNode> = Vec::with_capacity(inst.children.len());
    for c in &inst.children {
        children.push(layout_inst(c, &child_path(path, c), program, child_ctx)?);
    }

    // Determine this node's bbox + arrange children inside.
    let mut gutters: Vec<Gutter> = Vec::new();
    let mut sketch_d: Option<String> = None;
    let mut sketch_geo = None;
    let bbox = if inst.kind == NodeKind::Sketch {
        // The pen folds here [SPEC 15.3]: geometry decides the bbox — never
        // content + padding. Outside a drawing any children still arrange
        // normally over it; in one they are features, datum-placed below.
        if !children.is_empty() && !part {
            let _ = lay_out_container_children(&mut children, &inst.attrs, inst.span, own)?;
        }
        let folded = drawing::pen::fold(inst, own)?;
        let half = inst.attrs.number("stroke-width").unwrap_or(0.0) / 2.0;
        sketch_d = Some(folded.d);
        drawing::breaks::fill_chrome(&mut children, &folded.cuts);
        sketch_geo = Some(std::sync::Arc::new(drawing::SketchGeo {
            segments: folded.segments,
            mirrors: folded.mirror_axes,
            outline: folded.subs,
            view: folded.view,
        }));
        folded.geometry.inflate(half)
    } else if part {
        // A part sizes to its own shape — its features never grow it, they
        // overhang [SPEC 15.4] (`|hole|` / `|pitch-circle|` are circles, ⌀ width).
        drawing::part_bbox(inst, own)?
    } else if children.is_empty() {
        // Leaf primitive.
        primitives::leaf_bbox(inst, own)?
    } else {
        // Container or closed primitive with content.
        let (content_bbox, rects) =
            lay_out_container_children(&mut children, &inst.attrs, inst.span, own)?;

        // Interior gutters (grid or 1-D) the container fills with `gap-fill`.
        // A table is just a group with `gap-fill: --stroke` — no special-casing;
        // its border is the group rect, its inner rules these gutter rects.
        gutters = rects;

        // An icon sizes to a square that grows with its label child [SPEC 7];
        // every other closed primitive sizes border-box — explicit width/height,
        // else content + padding per axis [SPEC 5].
        let b = if inst.kind == NodeKind::Icon {
            primitives::icon_square_bbox(inst, content_bbox, own)?
        } else {
            primitives::closed_bbox(inst, content_bbox, own)?
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

    // A part's features datum-place (origin on the part's datum, `translate:`
    // in drawing units × the part's scale, [SPEC 15.4]); its generated chrome
    // takes its geometry from the sized shape.
    if part {
        let half = inst.attrs.number("stroke-width").unwrap_or(0.0) / 2.0;
        drawing::place_features(&mut children, own, sketch_geo.as_ref().map(|g| &g.view))?;
        drawing::chrome::fill(&mut children, bbox.inflate(-half));
    }

    let rotation = inst.attrs.number("rotate").unwrap_or(0.0);

    let mut placed = PlacedNode {
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
        sketch: sketch_geo,
        span: inst.span,
    };
    if let Some(d) = sketch_d {
        placed.attrs.insert("path", ResolvedValue::String(d));
    }
    // The drawn `points:` scale with the shape [SPEC 15.1] — the render reads
    // them off the placed node, so they carry the same factor `leaf_bbox`
    // sized with.
    if own != 1.0 {
        values::scale_points_attr(&mut placed.attrs, own);
    }
    // The core `|note|` silhouette [SPEC 8] — folded once, whatever the layout;
    // the sequence (and later the drawing) engine only places the card. Before
    // any pattern expansion, so the copies are folded cards.
    if placed.kind == NodeKind::Block && placed.type_chain.iter().any(|t| t == "note") {
        note::fold(&mut placed);
    }
    // `pattern:` replicates the node about its own position [SPEC 15.4] — any
    // layout; the offsets are shape, so they carry the node's own scale.
    if placed.attrs.get("pattern").is_some() {
        pattern::expand(&mut placed, own)?;
    }
    Ok(placed)
}

/// Whether a node arranges its own interior — an explicit `layout:` (grid /
/// chart / sequence / drawing) or a flow `direction:` (`|row|` / `|column|`).
/// In a drawing scope such a child seals: it lays out as usual and places as
/// one box [SPEC 15.1]; everything else datum-places its children as features.
fn owns_layout(attrs: &crate::resolve::AttrMap) -> bool {
    attrs.get("layout").is_some() || attrs.get("direction").is_some()
}

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
fn lay_out_container_children(
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

    // ── Sizing [SPEC 5] ──

    #[test]
    fn empty_closed_primitive_is_two_paddings() {
        // padding 20 each side → 40 drawn; + stroke 2 → 42 bbox.
        let n = &lay_out("|box|\n").nodes[0];
        assert!((n.bbox.w() - 42.0).abs() < 0.01, "w={}", n.bbox.w());
        assert!((n.bbox.h() - 42.0).abs() < 0.01, "h={}", n.bbox.h());
    }

    #[test]
    fn explicit_dims_are_border_box() {
        let n = &lay_out("|box| { width: 100; height: 50; }\n").nodes[0];
        assert!((n.bbox.w() - 102.0).abs() < 0.01, "w={}", n.bbox.w());
        assert!((n.bbox.h() - 52.0).abs() < 0.01, "h={}", n.bbox.h());
    }

    #[test]
    fn stroke_width_counts_toward_the_bbox() {
        // [SPEC 5]: width 100 height 50 stroke-width 4 → 104×54.
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
        assert!((n.bbox.w() - 202.0).abs() < 0.01, "w={}", n.bbox.w());
        // height auto = one text line (15) + 40 padding + 2 stroke = 57.
        assert!((n.bbox.h() - 57.0).abs() < 0.01, "h={}", n.bbox.h());
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
        assert!((kept.bbox.w() - 302.0).abs() < 0.01, "w={}", kept.bbox.w());
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
        assert!((n.bbox.w() - 102.0).abs() < 0.01, "w={}", n.bbox.w());
        assert!((n.bbox.h() - 52.0).abs() < 0.01, "h={}", n.bbox.h());
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
        // half (51) + gap (10) + half (31) = 92.
        let dx = l.nodes[1].cx - l.nodes[0].cx;
        assert!((dx - 92.0).abs() < 0.5, "dx={}", dx);
        assert!((l.nodes[0].cy - l.nodes[1].cy).abs() < 0.01);
    }

    #[test]
    fn column_layout_stacks_vertically() {
        let l = lay_out(
            "{ direction: column; gap: 20; }\n\
             |box| { width: 100; height: 40; }\n\
             |box| { width: 100; height: 60; }\n",
        );
        // half (21) + gap (20) + half (31) = 72.
        let dy = l.nodes[1].cy - l.nodes[0].cy;
        assert!((dy - 72.0).abs() < 0.5, "dy={}", dy);
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
        // bbox 102×42, + the scene's 20 padding each side → 142×82.
        let l = lay_out("|box| { width: 100; height: 40; }\n");
        assert!((l.viewbox.w - 142.0).abs() < 0.01, "w={}", l.viewbox.w);
        assert!((l.viewbox.h - 82.0).abs() < 0.01, "h={}", l.viewbox.h);
    }

    // ── Captions: ordinary flow children [SPEC 8] ──

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

    // ── Flex distribution with slack [SPEC 12] ──

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

    // ── Grid [SPEC 12] ──

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
        // [SPEC 12/18]: a declared `rows` track list sizes the first rows; extra
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

    // ── Gutters [SPEC 11] ──

    #[test]
    fn table_fills_interior_gutters_no_frame() {
        let l = lay_out("|table#t| { columns: 40 40 } [\n  \"a\" \"b\" \"c\" \"d\"\n]\n");
        // The table's `gap-fill: --stroke` fills the interior gutters.
        assert!(!l.nodes[0].gutters.is_empty(), "table has interior gutters");
        // A plain group has no `gap-fill`, so no gutters.
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
            "|row#g| { gap-fill: --stroke } [\n  |box#a| { width: 30; height: 30; }\n  |box#b| { width: 30; height: 30; }\n  |box#c| { width: 30; height: 30; }\n]\n",
        );
        assert_eq!(
            l.nodes[0].gutters.len(),
            2,
            "two gutters between three children"
        );
    }

    #[test]
    fn gap_fill_per_axis_selects_gutters() {
        // `gap: row col` [SPEC 11]: `4 0` paints row rules (horizontal gutters), `0 4`
        // column rules (vertical). A 2×2 grid has one interior boundary each way.
        let rows_only = lay_out(
            "|grid#g| { columns: 40 40; gap: 4 0; gap-fill: --stroke } [\n  \"a\" \"b\"\n  \"c\" \"d\"\n]\n",
        );
        let (_, _, w, h) = rows_only.nodes[0].gutters[0];
        assert_eq!(rows_only.nodes[0].gutters.len(), 1, "row gap → one gutter");
        assert!(w > h, "horizontal gutter is wide: w={w} h={h}");

        let cols_only = lay_out(
            "|grid#g| { columns: 40 40; gap: 0 4; gap-fill: --stroke } [\n  \"a\" \"b\"\n  \"c\" \"d\"\n]\n",
        );
        let (_, _, w2, h2) = cols_only.nodes[0].gutters[0];
        assert_eq!(cols_only.nodes[0].gutters.len(), 1, "col gap → one gutter");
        assert!(h2 > w2, "vertical gutter is tall: w={w2} h={h2}");
    }

    // ── `scale:` — a global node transform [SPEC 15.1] ──

    #[test]
    fn scale_multiplies_the_shape_never_text_or_stroke() {
        let plain = &lay_out("|box#a| \"hi\" { width: 100; height: 40 }\n").nodes[0];
        let scaled = &lay_out("|box#a| \"hi\" { width: 100; height: 40; scale: 2 }\n").nodes[0];
        assert!(
            (scaled.bbox.w() - 202.0).abs() < 0.01,
            "w={}",
            scaled.bbox.w()
        );
        assert!(
            (scaled.bbox.h() - 82.0).abs() < 0.01,
            "h={}",
            scaled.bbox.h()
        );
        // The text child keeps its size — text never scales.
        assert!((scaled.children[0].bbox.w() - plain.children[0].bbox.w()).abs() < 0.01);
    }

    #[test]
    fn scale_inherits_nearest_ancestor_wins() {
        // The root's scale reaches the child; the note's own `scale: 1` opts out.
        let l = lay_out(
            "{ scale: 2 }\n|rect#a| { width: 50; height: 20 }\n|note#n| { width: 50; height: 20 }\n",
        );
        let a = &l.nodes[0];
        assert!(
            (a.bbox.w() - 102.0).abs() < 0.01,
            "inherited: w={}",
            a.bbox.w()
        );
        let n = &l.nodes[1];
        assert!(
            n.bbox.w() < 60.0,
            "the note is sheet chrome: w={}",
            n.bbox.w()
        );
    }

    #[test]
    fn translate_scales_by_the_parent() {
        // A column flow: the x offset between the boxes is the translate alone,
        // in drawing units × the parent's scale [SPEC 15.1].
        let nudge = |src: &str| {
            let l = lay_out(src);
            l.nodes[1].cx - l.nodes[0].cx
        };
        let plain = nudge(
            "|rect#a| { width: 10; height: 10 }\n|rect#b| { width: 10; height: 10; translate: 5 0 }\n",
        );
        let scaled = nudge(
            "{ scale: 3 }\n|rect#a| { width: 10; height: 10 }\n|rect#b| { width: 10; height: 10; translate: 5 0 }\n",
        );
        assert!((plain - 5.0).abs() < 0.01, "plain={plain}");
        assert!((scaled - 15.0).abs() < 0.01, "scaled={scaled}");
    }

    #[test]
    fn a_scaled_sketch_in_a_flow_doubles_its_geometry() {
        let one = &lay_out("|sketch#s| { draw: move(0, 0) right(40) down(20) left(40) close() }\n")
            .nodes[0];
        let two = &lay_out(
            "|sketch#s| { draw: move(0, 0) right(40) down(20) left(40) close(); scale: 2 }\n",
        )
        .nodes[0];
        assert!((two.bbox.w() - one.bbox.w() - 40.0).abs() < 0.01);
        // The folded d carries the scaled coordinates for render.
        assert!(
            matches!(two.attrs.get("path"), Some(ResolvedValue::String(d)) if d.contains("80")),
            "scaled path"
        );
    }

    #[test]
    fn scale_must_be_positive() {
        let err = lay_out_err("|box#a| { scale: 0 }\n");
        assert_eq!(err.message, "'scale' must be > 0");
    }

    // ── `pattern:` — replicate in any layout [SPEC 15.4] ──

    #[test]
    fn a_patterned_box_in_a_flow_unions_its_copies() {
        let l = lay_out("|rect#a| { width: 20; height: 20; pattern: grid(3, 1, 30, 0) }\n");
        let a = &l.nodes[0];
        // Seed at 0, copies at 30 and 60 → 20 + 60 + stroke.
        assert!((a.bbox.w() - 82.0).abs() < 0.01, "w={}", a.bbox.w());
        assert_eq!(a.children.len(), 3, "three copies");
        assert!(a.id.as_deref() == Some("a"), "the carrier keeps the id");
    }

    #[test]
    fn a_filled_grid_cell_aligns_its_text_by_its_own_align() {
        // A grid cell filled by the container's `align: stretch` then honours its
        // own `align` (↔) to place its text [SPEC 12] — the generic rule tables use.
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

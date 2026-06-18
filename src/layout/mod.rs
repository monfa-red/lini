mod anchors;
mod flex;
mod grid;
mod ir;
mod primitives;
mod text;
mod titles;
mod values;
mod wires;

pub use ir::*;
pub(crate) use wires::cross;
pub use wires::{Rule, Severity, Violation, node_rect};

use crate::error::Error;
use crate::resolve::{Program, ResolvedInst, ResolvedValue, ShapeKind, VarTable};
use crate::span::Span;

use flex::Axis;

/// Extra gap per container (dot-path → `(Δy, Δx)` px), accumulated by gap
/// growth. Growth only ever adds to the gap the user set — `gap` stays
/// their density dial.
type GapGrowth = std::collections::BTreeMap<String, (f64, f64)>;

pub fn layout(program: &Program) -> Result<LaidOut, Error> {
    layout_mode(program, true)
}

/// The testing hook (PLAN Phase 8): growth disabled, so the clearance sweep
/// measures the raw router rather than the escape hatch.
pub fn layout_raw(program: &Program) -> Result<LaidOut, Error> {
    layout_mode(program, false)
}

/// Lay out and route; when wires are impossible for lack of corridor lanes
/// (WIRING §Impossible layouts), grow the named containers' gaps by exactly
/// the deficit and rerun — at most 2 rounds, keeping the best result (most
/// drawn, then fewest crossings). Airwires cover whatever still fails.
fn layout_mode(program: &Program, growth_on: bool) -> Result<LaidOut, Error> {
    let mut growth = GapGrowth::new();
    let mut best = attempt(program, &growth)?;
    if growth_on && !best.routing.starved.is_empty() {
        let mut starved = best.routing.starved.clone();
        for _ in 0..2 {
            if !grow(&mut growth, &starved, program) {
                break;
            }
            let next = attempt(program, &growth)?;
            starved = next.routing.starved.clone();
            if better(&next, &best) {
                best = next;
            }
            if starved.is_empty() {
                break;
            }
        }
    }
    Ok(finish(program, best))
}

/// One placed-and-routed scene under a given growth map.
struct Attempt {
    nodes: Vec<PlacedNode>,
    bbox: Bbox,
    routing: wires::Routing,
}

fn attempt(program: &Program, growth: &GapGrowth) -> Result<Attempt, Error> {
    // Lay out top-level scene children.
    let mut top_nodes = Vec::with_capacity(program.scene.nodes.len());
    for inst in &program.scene.nodes {
        top_nodes.push(layout_inst(
            inst,
            &program.vars,
            growth,
            &child_path("", inst),
        )?);
    }

    // Apply scene-level layout to top-level children (scene itself is a
    // container; its attrs drive how its children are positioned). The scene
    // is never a table, so its grid rules — if any — are discarded.
    let (bbox, _) = lay_out_container_children(
        &mut top_nodes,
        &program.scene.attrs,
        &program.vars,
        Span::empty(),
        gap_bump(growth, ""),
    )?;

    // The scene has no border or padding, so a scene-level `place:out` band has
    // no frame to sit outside of — it coincides with `place:in`, reserved just
    // beyond the content extent. Place it before routing so wires see it.
    let (scene_gap_y, _) = primitives::gap(&program.scene.attrs, &program.vars, Span::empty())?;
    let (bbox, _) =
        titles::place_out_bands(&mut top_nodes, bbox, scene_gap_y + gap_bump(growth, "").0)?;

    // Route wires once the nodes are placed.
    let routing = wires::route_wires(program, &top_nodes)?;
    Ok(Attempt {
        nodes: top_nodes,
        bbox,
        routing,
    })
}

/// Strictly better routing outcome: more wires drawn, then fewer crossings.
fn better(a: &Attempt, b: &Attempt) -> bool {
    let key = |t: &Attempt| {
        let crossings = t
            .routing
            .report
            .iter()
            .filter(|v| v.rule == Rule::Crossing)
            .count();
        (t.routing.wires.len(), std::cmp::Reverse(crossings))
    };
    key(a) > key(b)
}

/// Fold one routing's corridor deficits into the growth map. A container
/// pinned by an explicit `size` cannot honestly widen and is skipped.
/// Returns whether anything grew — `false` ends the growth loop.
fn grow(growth: &mut GapGrowth, starved: &GapGrowth, program: &Program) -> bool {
    let mut grew = false;
    for (path, &(dy, dx)) in starved {
        if !growable(program, path) {
            continue;
        }
        let (gy, gx) = growth.entry(path.clone()).or_insert((0.0, 0.0));
        *gy += dy;
        *gx += dx;
        grew |= dy > 0.0 || dx > 0.0;
    }
    grew
}

fn growable(program: &Program, path: &str) -> bool {
    if path.is_empty() {
        return true;
    }
    let mut nodes = &program.scene.nodes;
    let mut found: Option<&ResolvedInst> = None;
    for seg in path.split('.') {
        match nodes.iter().find(|n| n.id.as_deref() == Some(seg)) {
            Some(inst) => {
                nodes = &inst.children;
                found = Some(inst);
            }
            None => return false,
        }
    }
    found.is_some_and(|inst| {
        inst.attrs.get("width").is_none() && inst.attrs.get("height").is_none()
    })
}

fn gap_bump(growth: &GapGrowth, path: &str) -> (f64, f64) {
    growth.get(path).copied().unwrap_or((0.0, 0.0))
}

/// A child's dot-path under `parent`. Anonymous children get a `#` segment —
/// never a wire endpoint's ancestor, so never a growth target.
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

fn finish(program: &Program, attempt: Attempt) -> LaidOut {
    // Viewbox = scene bbox + wire paths, labels, airwires + canvas-pad on
    // every side.
    let pad = values::layout_var(&program.vars, "canvas-pad").unwrap_or(20.0);
    // Absolute overlays don't grow their parent's bbox, so the scene bbox can
    // miss one that overflows; the canvas must still include every drawn node,
    // so take the true visual extent of the whole tree.
    let mut bbox = attempt.bbox;
    for n in &attempt.nodes {
        accumulate_extent(n, 0.0, 0.0, &mut bbox);
    }
    let routing = attempt.routing;
    let wire_points = routing.wires.iter().flat_map(|w| &w.path);
    let air_points = routing.airwires.iter().flat_map(|a| [&a.from, &a.to]);
    for &(x, y) in wire_points.chain(air_points) {
        bbox.min_x = bbox.min_x.min(x);
        bbox.min_y = bbox.min_y.min(y);
        bbox.max_x = bbox.max_x.max(x);
        bbox.max_y = bbox.max_y.max(y);
    }
    for t in routing.wires.iter().flat_map(|w| &w.texts) {
        let size = t.attrs.number("text-size").unwrap_or(11.0);
        let (hw, hh) = (
            text::approx_width(&t.content, size) / 2.0,
            text::approx_height(&t.content, size) / 2.0,
        );
        bbox.min_x = bbox.min_x.min(t.position.0 - hw);
        bbox.min_y = bbox.min_y.min(t.position.1 - hh);
        bbox.max_x = bbox.max_x.max(t.position.0 + hw);
        bbox.max_y = bbox.max_y.max(t.position.1 + hh);
    }
    let vb = ViewBox {
        x: bbox.min_x - pad,
        y: bbox.min_y - pad,
        w: bbox.w() + 2.0 * pad,
        h: bbox.h() + 2.0 * pad,
    };

    // A root `fill:` is the canvas colour (SPEC §13); `none` stays transparent.
    let canvas_fill = program
        .scene
        .attrs
        .get("fill")
        .filter(|v| !matches!(v, ResolvedValue::Ident(s) if s == "none"))
        .cloned();

    LaidOut {
        viewbox: vb,
        nodes: attempt.nodes,
        wires: routing.wires,
        wire_report: routing.report,
        airwires: routing.airwires,
        vars: program.vars.clone(),
        sheet: program.sheet.clone(),
        canvas_fill,
    }
}

/// Validate a laid-out scene's wires against the routing contract (WIRING.md):
/// the router's own report (kept crossings, impossible wires), then the
/// independent four-law check. Used by `lini::validate_str`.
pub fn validate_routing(laid: &LaidOut) -> Vec<Violation> {
    let mut out = laid.wire_report.clone();
    out.extend(wires::validate_routing(
        &laid.nodes,
        &laid.wires,
        &laid.wire_report,
        &laid.vars,
    ));
    out
}

/// Recursively lay out a single instance into a PlacedNode.
///
/// Bottom-up: lay out children first, then size this node around them. For
/// leaf primitives (no children), the shape's dimensions drive the bbox.
/// `path` is the inst's dot-path — the key gap growth bumps it under.
fn layout_inst(
    inst: &ResolvedInst,
    vars: &VarTable,
    growth: &GapGrowth,
    path: &str,
) -> Result<PlacedNode, Error> {
    // Recurse into children first.
    let mut children: Vec<PlacedNode> = Vec::with_capacity(inst.children.len());
    for c in &inst.children {
        children.push(layout_inst(c, vars, growth, &child_path(path, c))?);
    }

    // Determine this node's bbox + arrange children inside.
    let mut dividers: Vec<GridRule> = Vec::new();
    // The drawn frame, set only when a `place:out` band grows the footprint
    // past it; otherwise the footprint is the drawn box (`frame` stays None).
    let mut frame: Option<Bbox> = None;
    let bbox = if children.is_empty() {
        // Leaf primitive.
        primitives::leaf_bbox(inst, vars)?
    } else {
        // Container or closed shape with content.
        let (content_bbox, rules) = lay_out_container_children(
            &mut children,
            &inst.attrs,
            vars,
            inst.span,
            gap_bump(growth, path),
        )?;

        // Interior dividers (grid or 1-D) the container draws, per `divider:`.
        // A table is just a group with `divider: all` — no special-casing; its
        // border is the group rect, its inner lines these dividers.
        dividers = rules;

        // The closed shape sizes border-box: explicit width/height, else
        // content + padding per axis (SPEC §6).
        let b = primitives::closed_bbox(inst, content_bbox, vars)?;
        let text_only = children.iter().all(|c| c.shape == ShapeKind::Text);

        // Some closed shapes carry decoration at the top — a cloud's lobes, a
        // cylinder's rim — so the optical body-center sits below the bbox center
        // and a centered label reads too high. Drop a text-only label into the
        // body by a shape-specific fraction of the height (the outlines are
        // scale-invariant, so a fraction holds at any size).
        const CLOUD_LABEL_DROP: f64 = 0.1;
        const CYL_LABEL_DROP: f64 = 0.03;
        let label_drop = match inst.shape {
            ShapeKind::Cloud => CLOUD_LABEL_DROP,
            ShapeKind::Cyl => CYL_LABEL_DROP,
            _ => 0.0,
        };
        if label_drop > 0.0 && text_only {
            let dy = b.h() * label_drop;
            for c in &mut children {
                c.cy += dy;
            }
        }

        // `place:out` bands sit a container `gap` outside the drawn frame `b`.
        // The footprint grows to reserve them — so siblings clear the caption —
        // while the shape keeps drawing at `b`.
        let (gap_y, _) = primitives::gap(&inst.attrs, vars, inst.span)?;
        let (footprint, has_out) =
            titles::place_out_bands(&mut children, b, gap_y + gap_bump(growth, path).0)?;
        if has_out {
            frame = Some(b);
        }
        footprint
    };

    let rotation = inst.attrs.number("rotate").unwrap_or(0.0);

    Ok(PlacedNode {
        id: inst.id.clone(),
        shape: inst.shape,
        type_chain: inst.type_chain.clone(),
        applied_styles: inst.applied_styles.clone(),
        label: inst.label.clone(),
        attrs: inst.attrs.clone(),
        markers: inst.markers.clone(),
        cx: 0.0,
        cy: 0.0,
        bbox,
        frame,
        rotation,
        children,
        dividers,
        span: inst.span,
    })
}

/// Interior separators between adjacent flow children — perpendicular to the
/// flow at each gap's midpoint, spanning the flow's cross extent (SPEC §5,
/// 1-D `divider`).
fn one_d_dividers(
    children: &[PlacedNode],
    flow: &[usize],
    mode: LayoutMode,
    flow_bbox: Bbox,
) -> Vec<GridRule> {
    let row = matches!(mode, LayoutMode::Row);
    let main = |i: usize| if row { children[i].cx } else { children[i].cy };
    let half = |i: usize| {
        if row {
            children[i].bbox.w() / 2.0
        } else {
            children[i].bbox.h() / 2.0
        }
    };
    let mut order: Vec<usize> = flow.to_vec();
    order.sort_by(|&a, &b| main(a).total_cmp(&main(b)));
    let mut segs = Vec::new();
    for pair in order.windows(2) {
        let mid = (main(pair[0]) + half(pair[0]) + main(pair[1]) - half(pair[1])) / 2.0;
        if row {
            segs.push((mid, flow_bbox.min_y, mid, flow_bbox.max_y));
        } else {
            segs.push((flow_bbox.min_x, mid, flow_bbox.max_x, mid));
        }
    }
    segs
}

/// Position children within their container per its `layout=` attr.
/// Returns the bounding bbox of all placed children, in container-local
/// coords. A non-zero `grow` is gap growth's `(Δy, Δx)` for this container,
/// added to whatever gap the user set.
fn lay_out_container_children(
    children: &mut [PlacedNode],
    container_attrs: &crate::resolve::AttrMap,
    vars: &VarTable,
    span: Span,
    grow: (f64, f64),
) -> Result<(Bbox, Vec<GridRule>), Error> {
    if children.is_empty() {
        return Ok((Bbox::empty(), Vec::new()));
    }
    let grown;
    let container_attrs = if grow == (0.0, 0.0) {
        container_attrs
    } else {
        let (gy, gx) = primitives::gap(container_attrs, vars, span)?;
        let mut attrs = container_attrs.clone();
        attrs.insert(
            "gap",
            ResolvedValue::Tuple(vec![
                ResolvedValue::Number(gy + grow.0),
                ResolvedValue::Number(gx + grow.1),
            ]),
        );
        grown = attrs;
        &grown
    };

    // `margin:` is signed outer spacing on a child. Inflate each child's bbox
    // into its layout footprint up front; every layout routine below then
    // spaces and sizes against the footprint (so margin grows/shrinks the
    // parent and the gaps), and we deflate back to the drawn box at the end.
    // Negative margins simply make the footprint smaller than the box — they
    // tighten, and may overlap, which is the intent.
    let margins: Vec<(f64, f64, f64, f64)> = children
        .iter()
        .map(|c| primitives::margin(&c.attrs, c.span))
        .collect::<Result<_, _>>()?;
    for (c, &(t, r, b, l)) in children.iter_mut().zip(&margins) {
        c.bbox = c.bbox.expand(t, r, b, l);
    }

    // Sort children into three roles (SPEC §7). `side:` with `place:in`/`out`
    // reserves an edge band (the parent grows); `at:(x,y)` or `place:on` is an
    // absolute overlay (the parent does not grow); everything else flows.
    let mut flow_indices: Vec<usize> = Vec::new();
    let mut abs_indices: Vec<usize> = Vec::new();
    let mut reserve_indices: Vec<usize> = Vec::new();
    for (i, c) in children.iter().enumerate() {
        match anchors::child_role(&c.attrs, c.span)? {
            anchors::Role::Flow => flow_indices.push(i),
            anchors::Role::Reserve => reserve_indices.push(i),
            anchors::Role::Absolute => abs_indices.push(i),
        }
    }

    // Lay out the flow children per the container's `layout=` attr.
    let mode = read_layout_mode(container_attrs, span)?;
    // Slack for align/justify/stretch comes only from an explicit container
    // size: the content area is the declared dimension minus padding (SPEC §5).
    let pad = primitives::padding(container_attrs, vars, span)?;
    let avail = (
        container_attrs
            .number("width")
            .map(|w| (w - pad.left - pad.right).max(0.0)),
        container_attrs
            .number("height")
            .map(|h| (h - pad.top - pad.bottom).max(0.0)),
    );

    let mut grid_rules: Vec<GridRule> = Vec::new();
    let flow_bbox = if !flow_indices.is_empty() {
        let mut flow_children: Vec<PlacedNode> =
            flow_indices.iter().map(|i| children[*i].clone()).collect();
        let bbox = match mode {
            LayoutMode::Row => flex::lay_out_flex(
                Axis::Row,
                &mut flow_children,
                container_attrs,
                vars,
                span,
                avail,
            )?,
            LayoutMode::Column => flex::lay_out_flex(
                Axis::Column,
                &mut flow_children,
                container_attrs,
                vars,
                span,
                avail,
            )?,
            LayoutMode::Grid => {
                let (bbox, rules) =
                    grid::lay_out_grid(&mut flow_children, container_attrs, vars, span)?;
                grid_rules = rules;
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

    // 1-D dividers between flow children (a grid produced its own above),
    // painted by the container's own stroke (SPEC §5).
    if matches!(mode, LayoutMode::Row | LayoutMode::Column)
        && grid::read_divider(container_attrs) != grid::Divider::None
        && flow_indices.len() > 1
    {
        grid_rules = one_d_dividers(children, &flow_indices, mode, flow_bbox);
    }

    // Reserve children carve a band on the top/bottom edge. `place:in` bands
    // sit inside the frame, so the box grows around them here — before padding,
    // so the border wraps them. `place:out` bands sit outside the drawn frame
    // and are placed by the caller (`titles::place_out_bands`) once padding has
    // fixed the border. The result here — content + inside bands — is the body
    // the anchors and the parent bbox resolve against.
    let in_indices: Vec<usize> = reserve_indices
        .iter()
        .copied()
        .filter(|&i| !anchors::is_out_band(&children[i].attrs))
        .collect();
    let body_bbox = if in_indices.is_empty() {
        flow_bbox
    } else {
        // A title is separated from the content by the container's vertical gap
        // — the same gap that spaces flow siblings (SPEC §7).
        let (gap_y, _) = primitives::gap(container_attrs, vars, span)?;
        titles::reserve_bands(
            children,
            &flow_indices,
            &in_indices,
            flow_bbox,
            &mut grid_rules,
            gap_y,
        )
    };

    // Resolution bbox for edge anchors. If the container has explicit
    // dimensions (e.g. `size:(200, 120)`), anchors snap to those edges;
    // otherwise we fall back to the body extent.
    let anchor_parent_bbox = container_anchor_bbox(container_attrs).unwrap_or(body_bbox);

    // Absolutely positioned children.
    for i in &abs_indices {
        let pos = anchors::read_pos(&children[*i].attrs, children[*i].span)?
            .expect("abs child carries at: or side:");
        let offset = match children[*i].attrs.get("offset") {
            Some(v) => anchors::parse_offset(v, children[*i].span)?,
            None => (0.0, 0.0),
        };
        let (target_cx, target_cy) = anchors::resolve(pos, anchor_parent_bbox, children[*i].bbox);
        // `at:(x,y)` puts the bbox CENTER at (x,y) per SPEC §7 rule 1.
        let cb = children[*i].bbox;
        let local_off_x = (cb.min_x + cb.max_x) / 2.0;
        let local_off_y = (cb.min_y + cb.max_y) / 2.0;
        children[*i].cx = target_cx + offset.0 - local_off_x;
        children[*i].cy = target_cy + offset.1 - local_off_y;
    }

    // Absolute overlays (`at:`, `place:on`) are positioned above, but they do
    // NOT grow the parent (SPEC §7): the parent sizes to its flow + reserved
    // bands only. An absolutes-only container with no explicit `size:` collapses
    // — that's the deal. The canvas viewBox still includes them (see `finish`),
    // so an overlay is never clipped.

    // Deflate footprints back to drawn boxes. Each routine placed the footprint
    // centre at its target, so the box — sitting margin-inset within the
    // footprint — is already in the right spot; only its bbox shrinks back.
    for (c, &(t, r, b, l)) in children.iter_mut().zip(&margins) {
        c.bbox = c.bbox.expand(-t, -r, -b, -l);
    }

    Ok((body_bbox, grid_rules))
}

/// Container layout mode, parsed from the `layout=` attr.
#[derive(Clone, Copy, Debug)]
enum LayoutMode {
    Row,
    Column,
    /// 2D grid; sized by its `columns` / `rows` track lists (read in `grid`).
    Grid,
}

fn read_layout_mode(attrs: &crate::resolve::AttrMap, span: Span) -> Result<LayoutMode, Error> {
    match attrs.get("layout") {
        None => Ok(LayoutMode::Column),
        Some(ResolvedValue::Ident(s)) => match s.as_str() {
            "row" => Ok(LayoutMode::Row),
            "column" => Ok(LayoutMode::Column),
            "grid" => Ok(LayoutMode::Grid),
            other => Err(Error::at(
                span,
                format!("unknown layout '{}' — expected row, column, or grid", other),
            )),
        },
        Some(_) => Err(Error::at(span, "'layout' expects row, column, or grid")),
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
        let program = crate::resolve::resolve_with_theme(&file, &[]).expect("resolve");
        layout(&program).expect("layout")
    }

    // ── Sizing (SPEC §6) ──

    #[test]
    fn empty_closed_shape_is_two_paddings() {
        // padding 16 each side → 32 drawn; + stroke 1 → 33 bbox.
        let n = &lay_out("|rect|\n").nodes[0];
        assert!((n.bbox.w() - 33.0).abs() < 0.01, "w={}", n.bbox.w());
        assert!((n.bbox.h() - 33.0).abs() < 0.01, "h={}", n.bbox.h());
    }

    #[test]
    fn explicit_dims_are_border_box() {
        let n = &lay_out("|rect| { width: 100; height: 50; }\n").nodes[0];
        assert!((n.bbox.w() - 101.0).abs() < 0.01, "w={}", n.bbox.w());
        assert!((n.bbox.h() - 51.0).abs() < 0.01, "h={}", n.bbox.h());
    }

    #[test]
    fn stroke_width_counts_toward_the_bbox() {
        // SPEC §6: width 100 height 50 stroke-width 4 → 104×54.
        let n = &lay_out("|rect| { width: 100; height: 50; stroke-width: 4; }\n").nodes[0];
        assert!((n.bbox.w() - 104.0).abs() < 0.01, "w={}", n.bbox.w());
        assert!((n.bbox.h() - 54.0).abs() < 0.01, "h={}", n.bbox.h());
    }

    #[test]
    fn label_auto_sizes_to_content_plus_padding() {
        // text ~15.4 + 2×16 padding + stroke → ~48.
        let n = &lay_out("|rect| \"hi\"\n").nodes[0];
        assert!(n.bbox.w() > 40.0 && n.bbox.w() < 60.0, "w={}", n.bbox.w());
    }

    #[test]
    fn dims_are_independent_per_axis() {
        let n = &lay_out("|rect| \"hi\" { width: 200; }\n").nodes[0];
        assert!((n.bbox.w() - 201.0).abs() < 0.01, "w={}", n.bbox.w());
        // height auto = one text line (14) + 32 padding + 1 stroke = 47.
        assert!((n.bbox.h() - 47.0).abs() < 0.01, "h={}", n.bbox.h());
    }

    #[test]
    fn oval_uses_width_height() {
        let n = &lay_out("|oval| { width: 100; height: 50; }\n").nodes[0];
        assert!((n.bbox.w() - 101.0).abs() < 0.01, "w={}", n.bbox.w());
        assert!((n.bbox.h() - 51.0).abs() < 0.01, "h={}", n.bbox.h());
    }

    #[test]
    fn text_sizes_to_its_glyphs_without_padding() {
        let n = &lay_out("|text| \"hi\"\n").nodes[0];
        assert!((n.bbox.w() - 15.4).abs() < 0.5, "w={}", n.bbox.w()); // 2 × 14 × 0.55
        assert!((n.bbox.h() - 14.0).abs() < 0.5, "h={}", n.bbox.h());
    }

    // ── Basic flow (full align/justify/stretch/evenly land in the flex chunk) ──

    #[test]
    fn row_layout_stacks_horizontally() {
        let l = lay_out(
            "layout: row; gap: 10;\n\
             |rect| { width: 100; height: 40; }\n\
             |rect| { width: 60; height: 40; }\n",
        );
        assert_eq!(l.nodes.len(), 2);
        // half (50.5) + gap (10) + half (30.5) = 91.
        let dx = l.nodes[1].cx - l.nodes[0].cx;
        assert!((dx - 91.0).abs() < 0.5, "dx={}", dx);
        assert!((l.nodes[0].cy - l.nodes[1].cy).abs() < 0.01);
    }

    #[test]
    fn column_layout_stacks_vertically() {
        let l = lay_out(
            "layout: column; gap: 20;\n\
             |rect| { width: 100; height: 40; }\n\
             |rect| { width: 100; height: 60; }\n",
        );
        // half (20.5) + gap (20) + half (30.5) = 71.
        let dy = l.nodes[1].cy - l.nodes[0].cy;
        assert!((dy - 71.0).abs() < 0.5, "dy={}", dy);
        assert!((l.nodes[0].cx - l.nodes[1].cx).abs() < 0.01);
    }

    #[test]
    fn viewbox_wraps_content_with_canvas_pad() {
        // bbox 101×41, + 20 canvas-pad each side → 141×81.
        let l = lay_out("|rect| { width: 100; height: 40; }\n");
        assert!((l.viewbox.w - 141.0).abs() < 0.01, "w={}", l.viewbox.w);
        assert!((l.viewbox.h - 81.0).abs() < 0.01, "h={}", l.viewbox.h);
    }

    // ── Caption bands (mount: in, SPEC §6/§8) ──

    #[test]
    fn group_caption_reserves_a_band_above_the_content() {
        let h = |src: &str| lay_out(src).nodes[0].bbox.h();
        let plain = h("g |group| {\n  a |rect| { width: 80; height: 30; }\n}\n");
        let capped = h("g |group| \"Cap\" {\n  a |rect| { width: 80; height: 30; }\n}\n");
        assert!(
            capped > plain + 10.0,
            "caption adds a band: plain={plain} capped={capped}"
        );
    }

    #[test]
    fn caption_sits_above_the_content() {
        let l = lay_out("g |group| \"Cap\" {\n  a |rect| { width: 80; height: 30; }\n}\n");
        let g = &l.nodes[0];
        let cap = g
            .children
            .iter()
            .find(|c| c.shape == ShapeKind::Text)
            .expect("caption child");
        let rect = g
            .children
            .iter()
            .find(|c| c.shape == ShapeKind::Rect)
            .expect("rect child");
        assert!(cap.cy < rect.cy, "cap.cy={} rect.cy={}", cap.cy, rect.cy);
    }

    // ── Flex distribution with slack (SPEC §5) ──

    #[test]
    fn justify_orders_children_start_center_end() {
        let first_cx = |j: &str| {
            let src = format!(
                "g |row| {{ width: 300; justify: {j};\n  a |rect| {{ width: 40; height: 20; }}\n  b |rect| {{ width: 40; height: 20; }}\n}}\n"
            );
            lay_out(&src).nodes[0].children[0].cx
        };
        let (start, center, end) = (first_cx("start"), first_cx("center"), first_cx("end"));
        assert!(start < center && center < end, "start={start} center={center} end={end}");
    }

    #[test]
    fn justify_evenly_spaces_children_equally() {
        let l = lay_out(
            "g |row| { width: 300; justify: evenly;\n  a |rect| { width: 20; height: 20; }\n  b |rect| { width: 20; height: 20; }\n  c |rect| { width: 20; height: 20; }\n}\n",
        );
        let cx: Vec<f64> = l.nodes[0].children.iter().map(|c| c.cx).collect();
        assert!(((cx[1] - cx[0]) - (cx[2] - cx[1])).abs() < 0.01, "centers {cx:?}");
    }

    #[test]
    fn align_stretch_fills_the_cross_axis() {
        // An unsized child grows to the row's content height (row pads 0).
        let l = lay_out("g |row| { height: 80; align: stretch;\n  a |rect| { width: 40; }\n}\n");
        let a = &l.nodes[0].children[0];
        assert!((a.bbox.h() - 80.0).abs() < 1.0, "a.h={}", a.bbox.h());
    }

    #[test]
    fn no_slack_means_no_distribution() {
        // An auto-width row ignores justify — children stay packed at the gap.
        let span = |j: &str| {
            let src = format!(
                "g |row| {{ justify: {j};\n  a |rect| {{ width: 40; height: 20; }}\n  b |rect| {{ width: 40; height: 20; }}\n}}\n"
            );
            let l = lay_out(&src);
            l.nodes[0].children[1].cx - l.nodes[0].children[0].cx
        };
        assert!((span("start") - span("end")).abs() < 0.01, "auto row: justify is a no-op");
    }

    // ── Grid (SPEC §5) ──

    #[test]
    fn grid_fixed_columns_place_children_in_order() {
        let l = lay_out(
            "layout: grid; columns: 80 80 80; gap: 0;\n\
             a |rect| { width: 40; height: 40; }\n\
             b |rect| { width: 40; height: 40; }\n\
             c |rect| { width: 40; height: 40; }\n",
        );
        let cx: Vec<f64> = l.nodes.iter().map(|n| n.cx).collect();
        assert!((cx[1] - cx[0] - 80.0).abs() < 0.5, "dx={}", cx[1] - cx[0]);
        assert!((cx[2] - cx[1] - 80.0).abs() < 0.5);
        assert!((l.nodes[0].cy - l.nodes[1].cy).abs() < 0.01);
    }

    #[test]
    fn grid_repeat_makes_auto_columns_and_wraps() {
        let l = lay_out(
            "layout: grid; columns: repeat(2);\n\
             a |rect| { width: 30; height: 30; }\n\
             b |rect| { width: 30; height: 30; }\n\
             c |rect| { width: 30; height: 30; }\n",
        );
        // 2 columns, 3 children → c wraps to the second row.
        assert!(l.nodes[2].cy > l.nodes[0].cy, "c below a");
    }

    #[test]
    fn grid_cell_pins_placement() {
        let l = lay_out(
            "layout: grid; columns: repeat(3);\n\
             a |rect| \"a\" { cell: 3 1; }\n\
             b |rect| \"b\"\n",
        );
        // a pins to column 3; b auto-flows to the first free cell (column 1).
        assert!(l.nodes[0].cx > l.nodes[1].cx, "a (col 3) right of b (col 1)");
    }

    #[test]
    fn grid_cell_fills_its_track_under_stretch() {
        let l = lay_out(
            "layout: grid; columns: 120 120; gap: 0;\n\
             a |rect| \"x\" { justify: stretch; align: stretch; }\n\
             b |rect| \"y\"\n",
        );
        assert!((l.nodes[0].bbox.w() - 120.0).abs() < 1.0, "a.w={}", l.nodes[0].bbox.w());
    }

    #[test]
    fn grid_without_columns_is_an_error() {
        let tokens = crate::lexer::lex("layout: grid;\na |rect|\nb |rect|\n").expect("lex");
        let file = crate::syntax::parser::parse(&tokens).expect("parse");
        let program = crate::resolve::resolve_with_theme(&file, &[]).expect("resolve");
        assert!(layout(&program).is_err());
    }

    // ── Dividers (SPEC §5) ──

    #[test]
    fn table_draws_interior_dividers_no_frame() {
        let l = lay_out(
            "t |table| { columns: 40 40;\n  a |rect| \"a\"\n  b |rect| \"b\"\n  c |rect| \"c\"\n  d |rect| \"d\"\n}\n",
        );
        // 2×2 grid with the table's divider: all → interior separators.
        assert!(!l.nodes[0].dividers.is_empty(), "table has interior dividers");
        // A plain group draws none.
        assert!(lay_out("g |group| { x |rect| \"x\" }\n").nodes[0].dividers.is_empty());
    }

    #[test]
    fn one_d_divider_falls_between_flow_children() {
        let l = lay_out(
            "g |row| { divider: all;\n  a |rect| { width: 30; height: 30; }\n  b |rect| { width: 30; height: 30; }\n  c |rect| { width: 30; height: 30; }\n}\n",
        );
        assert_eq!(l.nodes[0].dividers.len(), 2, "two separators between three children");
    }
}

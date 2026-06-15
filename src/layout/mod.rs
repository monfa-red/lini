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
    found.is_some_and(|inst| inst.attrs.get("size").is_none())
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

    LaidOut {
        viewbox: vb,
        nodes: attempt.nodes,
        wires: routing.wires,
        wire_report: routing.report,
        airwires: routing.airwires,
        vars: program.vars.clone(),
        sheet: program.sheet.clone(),
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
    let mut grid_rules: Vec<GridRule> = Vec::new();
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

        // Only a table draws its grid lines; other grids place silently.
        if is_table(&inst.type_chain) {
            grid_rules = rules;
        }

        // A closed shape whose content is text only auto-sizes with text-pad;
        // anything else sizes to content + padding.
        let text_only = inst.attrs.get("layout").is_none()
            && children.iter().all(|c| c.shape == ShapeKind::Text);

        let b = if let Some(explicit) = explicit_size(inst, vars)? {
            explicit
        } else {
            primitives::auto_sized_bbox(inst, content_bbox, vars, text_only)?
        };

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
        b
    };

    let rotation = inst
        .attrs
        .get("rotation")
        .and_then(|v| match v {
            ResolvedValue::Number(n) => Some(*n),
            _ => None,
        })
        .unwrap_or(0.0);

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
        rotation,
        children,
        grid_rules,
        span: inst.span,
    })
}

/// Whether a resolved type chain is (or extends) the `table` template — the
/// only node that draws grid rules.
fn is_table(type_chain: &[String]) -> bool {
    type_chain.iter().any(|t| t == "table")
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

    let mut grid_rules: Vec<GridRule> = Vec::new();
    let flow_bbox = if !flow_indices.is_empty() {
        let mut flow_children: Vec<PlacedNode> =
            flow_indices.iter().map(|i| children[*i].clone()).collect();
        let bbox = match mode {
            LayoutMode::Row => {
                flex::lay_out_flex(Axis::Row, &mut flow_children, container_attrs, vars, span)?
            }
            LayoutMode::Column => flex::lay_out_flex(
                Axis::Column,
                &mut flow_children,
                container_attrs,
                vars,
                span,
            )?,
            LayoutMode::Grid(cols, rows) => {
                let (bbox, rules) =
                    grid::lay_out_grid(&mut flow_children, cols, rows, container_attrs, vars, span)?;
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

    // Reserve children carve a band on the top/bottom edge: content shifts to
    // clear them and the box grows. The result — content + bands — is the body
    // the anchors and the parent bbox resolve against.
    let body_bbox = if reserve_indices.is_empty() {
        flow_bbox
    } else {
        titles::reserve_bands(
            children,
            &flow_indices,
            &reserve_indices,
            flow_bbox,
            &mut grid_rules,
            vars,
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
    Ok((body_bbox, grid_rules))
}

/// Container layout mode, parsed from the `layout=` attr.
#[derive(Clone, Copy, Debug)]
enum LayoutMode {
    Row,
    Column,
    /// `layout=(cols, rows)` — 2D grid with the given dimensions.
    Grid(usize, usize),
}

fn read_layout_mode(attrs: &crate::resolve::AttrMap, span: Span) -> Result<LayoutMode, Error> {
    match attrs.get("layout") {
        None => Ok(LayoutMode::Column),
        Some(ResolvedValue::Ident(s)) => match s.as_str() {
            "row" => Ok(LayoutMode::Row),
            "column" => Ok(LayoutMode::Column),
            other => Err(Error::at(
                span,
                format!(
                    "unknown layout '{}' — expected 'row', 'column', or (cols, rows)",
                    other
                ),
            )),
        },
        Some(ResolvedValue::Tuple(items)) if items.len() == 2 => {
            let cols = positive_int(&items[0], span, "layout.cols")?;
            let rows = positive_int(&items[1], span, "layout.rows")?;
            Ok(LayoutMode::Grid(cols, rows))
        }
        Some(_) => Err(Error::at(
            span,
            "layout= expects 'row', 'column', or a (cols, rows) tuple",
        )),
    }
}

fn positive_int(v: &ResolvedValue, span: Span, what: &str) -> Result<usize, Error> {
    let n = v
        .as_number()
        .ok_or_else(|| Error::at(span, format!("{} must be a positive integer", what)))?;
    if n < 1.0 || n.fract() != 0.0 {
        return Err(Error::at(
            span,
            format!("{} must be a positive integer, got {}", what, n),
        ));
    }
    Ok(n as usize)
}

/// If a closed shape sets `size=` explicitly, use its geometric bbox
/// (with stroke padding); otherwise fall through to content-driven sizing.
fn explicit_size(inst: &ResolvedInst, vars: &VarTable) -> Result<Option<Bbox>, Error> {
    let accepts_size = matches!(
        inst.shape,
        ShapeKind::Rect
            | ShapeKind::Slant
            | ShapeKind::Hex
            | ShapeKind::Cyl
            | ShapeKind::Diamond
            | ShapeKind::Cloud
            | ShapeKind::Oval
    );
    if !accepts_size || inst.attrs.get("size").is_none() {
        return Ok(None);
    }
    Ok(Some(primitives::leaf_bbox(inst, vars)?))
}

/// If the container declared explicit `size:`, return a bbox the children's
/// anchors should resolve against (no stroke pad — anchors live on the drawn
/// shape's edges).
fn container_anchor_bbox(attrs: &crate::resolve::AttrMap) -> Option<Bbox> {
    let (w, h) = read_size_loose(attrs)?;
    Some(Bbox::centered(w, h))
}

fn read_size_loose(attrs: &crate::resolve::AttrMap) -> Option<(f64, f64)> {
    let v = attrs.get("size")?;
    match v {
        ResolvedValue::Tuple(items) if items.len() == 2 => {
            Some((items[0].as_number()?, items[1].as_number()?))
        }
        _ => v.as_number().map(|n| (n, n)),
    }
}

// ───────────────────────────── Tests ─────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn lay_out(src: &str) -> LaidOut {
        let tokens = crate::lexer::lex(src).expect("lex");
        let file = crate::parser::parse(&tokens).expect("parse");
        let program = crate::resolve::resolve(file).expect("resolve");
        layout(&program).expect("layout")
    }

    #[test]
    fn rect_with_explicit_size_keeps_those_dims() {
        let l = lay_out("|rect| size:(200, 80)\n");
        let n = &l.nodes[0];
        assert!((n.bbox.w() - 201.0).abs() < 0.01, "bbox.w={}", n.bbox.w());
        assert!((n.bbox.h() - 81.0).abs() < 0.01, "bbox.h={}", n.bbox.h());
    }

    #[test]
    fn rect_with_label_auto_sizes_to_text_plus_pad() {
        let l = lay_out("|rect| \"hi\"\n");
        let n = &l.nodes[0];
        assert!(
            n.bbox.w() > 30.0 && n.bbox.w() < 60.0,
            "got w={}",
            n.bbox.w()
        );
    }

    #[test]
    fn rect_with_size_and_text_overrides_auto_size() {
        let l = lay_out("|rect| \"hello\" size:(200, 40)\n");
        let n = &l.nodes[0];
        assert!((n.bbox.w() - 201.0).abs() < 0.01, "bbox.w={}", n.bbox.w());
        assert!((n.bbox.h() - 41.0).abs() < 0.01, "bbox.h={}", n.bbox.h());
    }

    #[test]
    fn rect_with_scalar_size() {
        let l = lay_out("|rect| \"sq\" size:100\n");
        let n = &l.nodes[0];
        assert!((n.bbox.w() - 101.0).abs() < 0.01, "bbox.w={}", n.bbox.w());
        assert!((n.bbox.h() - 101.0).abs() < 0.01, "bbox.h={}", n.bbox.h());
    }

    #[test]
    fn oval_uses_size() {
        let l = lay_out("|oval| size:(100, 50)\n");
        let n = &l.nodes[0];
        assert!((n.bbox.w() - 101.0).abs() < 0.01);
        assert!((n.bbox.h() - 51.0).abs() < 0.01);
    }

    #[test]
    fn row_layout_stacks_horizontally() {
        let l = lay_out(
            "{ |scene| layout:row gap:10 }\n\
             |rect| size:(100, 40)\n\
             |rect| size:(60, 40)\n",
        );
        assert_eq!(l.nodes.len(), 2);
        let dx = l.nodes[1].cx - l.nodes[0].cx;
        assert!((dx - 90.0).abs() < 2.0, "dx={}", dx);
        assert!((l.nodes[0].cy - l.nodes[1].cy).abs() < 0.01);
    }

    #[test]
    fn column_layout_stacks_vertically() {
        let l = lay_out(
            "{ |scene| layout:column gap:20 }\n\
             |rect| size:(100, 40)\n\
             |rect| size:(100, 60)\n",
        );
        let dy = l.nodes[1].cy - l.nodes[0].cy;
        assert!((dy - 70.0).abs() < 2.0, "dy={}", dy);
        assert!((l.nodes[0].cx - l.nodes[1].cx).abs() < 0.01);
    }

    #[test]
    fn grid_cells_default_to_center_alignment() {
        let l = lay_out(
            "{ |scene| layout:(2, 1) col-widths:[200, 200] row-heights:100 gap:0 }\n\
             cat |rect| size:(40, 40) cell:(1, 1)\n\
             dog |rect| size:(40, 40) cell:(2, 1)\n",
        );
        let dx = l.nodes[1].cx - l.nodes[0].cx;
        assert!((dx - 200.0).abs() < 0.01, "dx={}", dx);
        assert!((l.nodes[0].cy - l.nodes[1].cy).abs() < 0.01);
    }

    #[test]
    fn grid_places_by_cell() {
        let l = lay_out(
            "{ |scene| layout:(3, 2) gap:20 }\n\
             |rect| size:(80, 40) cell:(1, 1)\n\
             |rect| size:(80, 40) cell:(3, 1)\n\
             |rect| size:(80, 40) cell:(2, 2)\n",
        );
        assert_eq!(l.nodes.len(), 3);
        assert!(l.nodes[0].cx < l.nodes[1].cx);
        assert!(l.nodes[2].cy > l.nodes[0].cy);
    }

    #[test]
    fn at_coord_places_absolutely() {
        let l = lay_out("|rect| size:(40, 40) at:(100, 50)\n");
        let n = &l.nodes[0];
        assert!((n.cx - 100.0).abs() < 0.01, "cx={}", n.cx);
        assert!((n.cy - 50.0).abs() < 0.01, "cy={}", n.cy);
    }

    #[test]
    fn viewbox_wraps_content_with_canvas_pad() {
        let l = lay_out("|rect| size:(100, 40)\n");
        assert!((l.viewbox.w - 141.0).abs() < 0.01, "w={}", l.viewbox.w);
        assert!((l.viewbox.h - 81.0).abs() < 0.01, "h={}", l.viewbox.h);
    }

    #[test]
    fn defaults_override_layout_var_changes_layout_math() {
        let l = lay_out(
            "{ |scene| layout:row\n  --gap:60 }\n\
             |rect| size:(40, 40)\n\
             |rect| size:(40, 40)\n",
        );
        let dx = l.nodes[1].cx - l.nodes[0].cx;
        assert!((dx - 100.0).abs() < 2.0, "dx={}", dx);
    }

    #[test]
    fn full_spec_example_lays_out_without_error() {
        let src = std::fs::read_to_string("samples/full_example.lini").unwrap();
        let tokens = crate::lexer::lex(&src).expect("lex");
        let file = crate::parser::parse(&tokens).expect("parse");
        let program = crate::resolve::resolve(file).expect("resolve");
        let l = layout(&program).expect("layout");
        // Smoke check: the showcase lays out into a non-trivial multi-node scene.
        assert!(l.viewbox.w > 100.0);
        assert!(l.viewbox.h > 100.0);
        assert!(l.nodes.len() >= 4);
    }
}

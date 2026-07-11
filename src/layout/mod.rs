mod anchors;
mod arrange;
mod chart;
pub(crate) mod drawing;
mod flex;
mod frame;
mod grid;
pub(crate) mod ir;
mod note;
mod page;
pub(crate) mod path_bbox; // glyph-extent computation also serves `render::icon_fit`
mod pattern;
mod prim; // PlacedNode *builders* for lowered primitives (charts, sequences)
mod primitives; // primitive *sizing* (leaf/closed bbox) — distinct from `prim`
pub(crate) mod sequence;
mod text;
mod values;
mod wrap;

pub(crate) use anchors::is_pinned;
pub use ir::*;
pub(crate) use text::{approx_height, approx_width};
pub(crate) use values::as_pair;

use crate::error::Error;
use crate::resolve::{NodeKind, Program, ResolvedInst, ResolvedValue};
use crate::routing;
use crate::span::Span;

use flex::Axis;

use arrange::lay_out_container_children;
use frame::{accumulate_extent, finish};

/// Lay out the scene, then route its links over the finished, immutable
/// layout (ROUTING.md) — layout never moves for a link; whatever cannot be
/// drawn lawfully is reported and rendered as a stray.
pub fn layout(program: &Program) -> Result<LaidOut, Error> {
    sequence::validate(program)?;

    // A root drawing (`{ layout: drawing }`, [SPEC 15]) owns the whole scene:
    // its children datum-place, mates seat them, and its drawing-scope links
    // never route — intercepted before the generic per-child layout, which
    // would flow-arrange features and reject the chrome. A nested *ordinary*
    // scope (a `|row|` of blocks on the sheet) still routes its own wires
    // [SPEC 11/15]: the router's request pass skips drawing/sequence scopes,
    // so the full route sees exactly those.
    if crate::resolve::is_drawing(&program.scene.attrs) {
        let (top_nodes, bbox) = drawing::layout_root(program)?;
        let routed = routing::route(program, &top_nodes)?;
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
        let (bbox, links) = sequence::layout_root(&mut top_nodes, program)?;
        // Nested ordinary scopes route their own wires [SPEC 11/13]; the
        // request pass skips the sequence's own messages, which the engine
        // lowered above — extend the routed set with them.
        let mut routed = routing::route(program, &top_nodes)?;
        routed.links.extend(links);
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

/// Where a text leaf's lines align [SPEC 6]: the nearest container box's
/// **horizontal packing knob** — `align` in a column / grid context, `justify`
/// in a row — mapped `start` / `center` / `end`; everything else (`stretch`,
/// `evenly`, `origin`, unset) reads `center`. The one resolver behind flex,
/// grid tracks, and the table-cell slide.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum LineAlign {
    Start,
    Center,
    End,
}

pub(crate) fn line_align_of(knob: Option<&str>) -> LineAlign {
    match knob {
        Some("start") => LineAlign::Start,
        Some("end") => LineAlign::End,
        _ => LineAlign::Center,
    }
}

/// Carry a resolved line alignment onto a placed **text** leaf, for the
/// renderer's per-line anchoring. Centre is the default — nothing to carry.
pub(crate) fn stamp_line_align(child: &mut PlacedNode, align: LineAlign) {
    let word = match align {
        LineAlign::Center => return,
        LineAlign::Start => "start",
        LineAlign::End => "end",
    };
    if child.kind == NodeKind::Text {
        child.attrs.insert(
            "line-align",
            crate::resolve::ResolvedValue::Ident(word.into()),
        );
    }
}

/// A node's effective multiplier: the desugar-folded `px-per-unit:` when
/// present (a drawing scope / page — ratio × unit × density, [SPEC 15.1/18]),
/// else its own `scale:` (sheet chrome pins 1), else the inherited one.
pub(crate) fn effective_scale(
    attrs: &crate::resolve::AttrMap,
    inherited: f64,
    span: Span,
) -> Result<f64, Error> {
    let own = match attrs.get("px-per-unit") {
        Some(v) => Some(v),
        None => attrs.get("scale"),
    };
    match own {
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
/// Walks by id, like an endpoint path — descending through **anonymous**
/// containers, which are scope-transparent [SPEC 9]. Used by the scope
/// detectors and the sequence engine.
pub(super) fn node_at<'a>(program: &'a Program, path: &str) -> Option<&'a ResolvedInst> {
    let mut nodes = &program.scene.nodes;
    let mut found = None;
    for seg in path.split('.') {
        let inst = crate::resolve::scene::find_in_scope(nodes, seg, &mut Vec::new())?;
        found = Some(inst);
        nodes = &inst.children;
    }
    found
}

/// A child's dot-path under `parent`. **Anonymous children are
/// scope-transparent** [SPEC 9]: they contribute no segment — their children
/// address as the parent's — matching resolve's link prefixes and the routing
/// index, so an engine's `w.scope == path` filter agrees with resolve.
fn child_path(parent: &str, inst: &ResolvedInst) -> String {
    let Some(id) = inst.id.as_deref() else {
        return parent.to_owned();
    };
    if parent.is_empty() {
        id.to_owned()
    } else {
        format!("{parent}.{id}")
    }
}

/// Validate a laid-out scene's links against the routing contract (ROUTING.md):
/// the engine's own report (drawn crossings, impossible links), then the
/// independent four-law check. Used by `lini::validate_str`.
/// The absurd-extent hint [SPEC 15.1/20]: a drawing view rendering wider or
/// taller than the threshold almost certainly authored a magnitude into
/// `scale:` (a ratio) — say so, with the likely fix. Pages are bounded by
/// their sheet and never hint.
pub fn extent_hints(laid: &LaidOut, program: &Program) -> Vec<crate::error::Diagnostic> {
    fn walk(nodes: &[PlacedNode], out: &mut Vec<crate::error::Diagnostic>) {
        for n in nodes {
            let is_drawing =
                n.attrs.get("px-per-unit").is_some() && !n.type_chain.iter().any(|t| t == "page");
            if is_drawing {
                let (w, h) = (n.bbox.w(), n.bbox.h());
                if w.max(h) > crate::ledger::consts::ABSURD_EXTENT_PX {
                    let (extent, axis) = if w >= h { (w, "wide") } else { (h, "tall") };
                    out.push(crate::error::Diagnostic::warn(
                        n.span,
                        format!(
                            "the drawing renders {} px {axis} — 'scale:' is a ratio; a 5 m beam at 1:50 is 'scale: 0.02'",
                            extent.round()
                        ),
                    ));
                }
            }
            walk(&n.children, out);
        }
    }
    let mut out = Vec::new();
    // A `{ layout: drawing }` root is a view too — judge the whole canvas.
    if program.scene.attrs.get("px-per-unit").is_some() {
        let (w, h) = (laid.viewbox.w, laid.viewbox.h);
        if w.max(h) > crate::ledger::consts::ABSURD_EXTENT_PX {
            let (extent, axis) = if w >= h { (w, "wide") } else { (h, "tall") };
            out.push(crate::error::Diagnostic::warn(
                Span::empty(),
                format!(
                    "the drawing renders {} px {axis} — 'scale:' is a ratio; a 5 m beam at 1:50 is 'scale: 0.02'",
                    extent.round()
                ),
            ));
        }
    }
    walk(&laid.nodes, &mut out);
    out
}

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
    // `thread:` dresses a sketch segment (side view) or a round feature's
    // circle (the ¾ arc) [SPEC 15.3/15.4]; the pitch-only round form takes
    // one positive number.
    if let Some(v) = inst.attrs.get("thread") {
        match inst.kind {
            NodeKind::Sketch => {}
            NodeKind::Oval => {
                // `thread:` is list-shaped; the round pitch-only form is one
                // bare number [SPEC 15.4].
                let pitch = match v {
                    ResolvedValue::List(items) => match items.as_slice() {
                        [one] => one.as_number(),
                        _ => None,
                    },
                    one => one.as_number(),
                };
                if !pitch.is_some_and(|p| p > 0.0) {
                    return Err(Error::at(
                        inst.span,
                        "'thread' takes a segment and its pitch — 'thread: m8 1.5'",
                    ));
                }
            }
            _ => {
                return Err(Error::at(
                    inst.span,
                    "'thread' dresses a '|sketch|' segment or a round feature",
                ));
            }
        }
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
    } else if crate::resolve::is_drawing(&inst.attrs) {
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

    // `max-width` [SPEC 5]: wrap text children to the cap (re-measuring them)
    // and reject what cannot honour it, before anything is arranged — the
    // wrapped size is what tracks, gutters, and routing see.
    wrap::apply_max_width(inst, &mut children, own, inst.span)?;

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
        drawing::edges::fill(&mut children, "edges", &folded.edges);
        drawing::edges::fill(&mut children, "thread", &folded.threads);
        sketch_geo = Some(std::sync::Arc::new(drawing::SketchGeo {
            segments: folded.segments,
            mirrors: folded.mirror_axes,
            revolved: folded.revolved,
            threads: folded.thread_specs,
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
        // Container or closed primitive with content. A `|page|` arranges its
        // flow inside the frame's content area — its inset folds into the
        // padding for this pass alone [SPEC 15.8].
        let page_attrs;
        let arrange_attrs = if page::is_page(&inst.type_chain) {
            page_attrs = page::padded_attrs(&inst.attrs, own, inst.span)?;
            &page_attrs
        } else {
            &inst.attrs
        };
        let (content_bbox, rects) =
            lay_out_container_children(&mut children, arrange_attrs, inst.span, own)?;

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
        drawing::chrome::fill(&mut children, bbox.inflate(-half), own);
    }
    // A page's furniture takes its geometry from the sized sheet, and any
    // title block seats flush inside the frame corner [SPEC 15.8].
    if page::is_page(&inst.type_chain) {
        page::finish(&mut children, bbox, own);
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
        origin: (0.0, 0.0),
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

// ───────────────────────────── Tests ─────────────────────────────

#[cfg(test)]
mod tests;

mod anchors;
mod arrange;
pub(crate) mod chart;
pub(crate) mod datum;
pub(crate) mod drawing;
mod flex;
pub(crate) mod floorplan;
mod frame;
mod gates;
pub(crate) mod geom;
mod grid;
pub(crate) mod ir;
mod mirror;
mod note;
mod page;
mod pattern;
mod prim; // PlacedNode *builders* for lowered primitives (charts, sequences)
mod primitives; // primitive *sizing* (leaf/closed bbox) — distinct from `prim`
pub(crate) mod schematic;
pub(crate) mod sequence;
pub(crate) mod stack;
pub(crate) mod text;
pub(crate) mod tree;
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
    // A layout's own types belong in its scope [SPEC 21] — the schematic
    // family, the chart family, and the drafting symbol's law one family over:
    // one sweep, the gate every later law leans on ([`gates::check_types`]).
    gates::check_types(program)?;

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

    // A root `{ layout: stack }` scene ([SPEC 12]) puts every child's origin on
    // the scene datum — the drawing's placement law without the drafting. Its
    // links route like any diagram's, so the routing pass runs as usual;
    // intercepted here because the generic per-child layout would flow them.
    if crate::resolve::is_stack(&program.scene.attrs) {
        let (top_nodes, bbox) = datum::layout_root(program)?;
        let routed = routing::route(program, &top_nodes)?;
        return finish(program, top_nodes, bbox, routed);
    }

    // A root `{ layout: tree }` scene ([SPEC 12]) is the tree container: it
    // arranges its topics as generations (each topic's card sized from its own
    // content), then the router routes the branch links like any wires —
    // intercepted before the generic per-child layout, which would fold a
    // topic's branches into its own box.
    if tree::is_tree(&program.scene.attrs) {
        let (top_nodes, bbox) = tree::layout_root(program)?;
        let routed = routing::route(program, &top_nodes)?;
        return finish(program, top_nodes, bbox, routed);
    }

    // A root `{ layout: schematic }` scene ([SPEC 16]) is the schematic scope:
    // it places its parts on the track grid, then the router draws its wires
    // like any links — intercepted before the generic per-child layout, which
    // would flow-arrange the parts instead.
    if schematic::is_schematic(&program.scene.attrs) {
        let (top_nodes, bbox) = schematic::layout_root(program)?;
        let routed = routing::route(program, &top_nodes)?;
        return finish(program, top_nodes, bbox, routed);
    }

    let ctx = effective_scale(
        &program.scene.attrs,
        NodeKind::Block,
        &[],
        Ctx::sheet(),
        Span::empty(),
    )?;

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
    // Lower the sheet's projection construction links [SPEC 15.8]: after the
    // views have placed (and `align: origin` lined them up), each ties two
    // resolved anchors with one straight `|projection|` chrome line, in sheet
    // space — never routed, never a packing obstacle.
    lower_projections(program, &mut top_nodes)?;
    finish(program, top_nodes, bbox, routed)
}

/// Append one straight `|projection|` chrome line per sheet-scope projection
/// link [SPEC 15.8], between its two anchors in scene coordinates. The views
/// are already placed, so this only reads their geometry; the lines sit within
/// the views' extent, so they never grow the canvas.
fn lower_projections(program: &Program, nodes: &mut Vec<PlacedNode>) -> Result<(), Error> {
    let mut lines = Vec::new();
    for w in &program.links {
        if !w.projection {
            continue;
        }
        let a = drawing::project_anchor(nodes, &w.endpoints[0])?;
        let b = drawing::project_anchor(nodes, &w.endpoints[1])?;
        lines.push(projection_line(w, a, b));
    }
    nodes.append(&mut lines);
    Ok(())
}

/// The generated `|projection|` line node: a two-point `|line|` wearing the
/// projection type, its paint the cascade the link resolved [SPEC 8/15.8], its
/// points the two anchors in scene space (so `cx`/`cy` stay zero).
fn projection_line(w: &crate::resolve::ResolvedLink, a: (f64, f64), b: (f64, f64)) -> PlacedNode {
    use crate::resolve::ResolvedValue;
    let mut attrs = w.attrs.clone();
    let point = |p: (f64, f64)| {
        ResolvedValue::Tuple(vec![ResolvedValue::Number(p.0), ResolvedValue::Number(p.1)])
    };
    attrs.insert("points", ResolvedValue::List(vec![point(a), point(b)]));
    // Generated chrome, like every SPEC 15.7 producer's — what lets a
    // pages-only sheet stay pages-only (its physical mm size, its hugging
    // padding) with projection lines drawn across it.
    attrs.insert("chrome", ResolvedValue::Ident("projection".into()));
    let half = attrs.number("stroke-width").unwrap_or(0.0) / 2.0;
    let bbox = Bbox {
        min_x: a.0.min(b.0),
        min_y: a.1.min(b.1),
        max_x: a.0.max(b.0),
        max_y: a.1.max(b.1),
    }
    .inflate(half);
    PlacedNode {
        id: None,
        kind: NodeKind::Line,
        type_chain: vec!["projection".to_string()],
        applied_styles: w.applied_styles.clone(),
        label: None,
        attrs,
        own_style: crate::resolve::AttrMap::new(),
        markers: crate::resolve::Markers::default(),
        cx: 0.0,
        cy: 0.0,
        bbox,
        rotation: 0.0,
        children: Vec::new(),
        gutters: Vec::new(),
        links: Vec::new(),
        sketch: None,
        origin: (0.0, 0.0),
        span: w.span,
    }
}

/// The layout context a node inherits [SPEC 15]: the parent's effective
/// `scale:` (px per drawing unit — nearest ancestor wins) and whether the node
/// sits in a drawing scope, where a shape's `[ ]` children datum-place as its
/// features. Layout-owning engines (chart / pie / sequence) reset it — their
/// interiors are sheet-space.
#[derive(Clone, Copy)]
pub(crate) struct Ctx {
    /// The multiplier itself — pixels per drawing unit, `base × ratio`, set
    /// only by [`effective_scale`] so the three can never drift.
    pub scale: f64,
    /// The enclosing scope's pixels per drawing unit **at ratio 1** — the
    /// desugar-folded `unit:` × `density:` [SPEC 15.1]; 1 off a drawing.
    pub base: f64,
    /// The drafting ratio in force, nearest ancestor wins [SPEC 15.1].
    pub ratio: f64,
    /// **Placement**: this scope's children — and a shape's `[ ]` features —
    /// put their origin on the scope's datum instead of flowing [SPEC 12].
    /// True in a `stack`, and in the `drawing` family that builds on it.
    pub datum: bool,
    /// **Drafting**: the annotation machinery is live — generated chrome, the
    /// drafting symbols, `unit:` [SPEC 15]. A drawing is a stack that also
    /// drafts, so this always implies `datum`; a plain `stack` places without
    /// it, which is what lets artwork use the pen without a sheet's apparatus.
    pub drawing: bool,
}

impl Ctx {
    pub(crate) fn sheet() -> Self {
        Ctx {
            scale: 1.0,
            base: 1.0,
            ratio: 1.0,
            datum: false,
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

/// A node's own **base** and **ratio** [SPEC 15.1], descended from its
/// parent's — the pair whose product is its effective multiplier, pixels per
/// drawing unit ([`Ctx::scale`]).
///
/// The base is the desugar-folded `px-per-unit:` where the node opens a scope
/// (a drawing / floorplan, or a `|page|`'s paper millimetres — `unit:` × the
/// root `density:`), **1** where the node is sheet content (an annotation
/// draws in sheet space [SPEC 15.10], whatever ratio the view around it
/// drafts at), else the enclosing scope's. The ratio is the node's own
/// `scale:` — wherever the cascade found it — else the nearest ancestor's,
/// which is what makes `scale:` the ordinary node property [SPEC 15.1] calls
/// it: an element rule, an id rule and an ancestor's block reach a drawing
/// exactly as its own block does. `drawing` is the caller's own seal test and
/// rides through untouched.
pub(crate) fn effective_scale(
    attrs: &crate::resolve::AttrMap,
    kind: NodeKind,
    type_chain: &[String],
    ctx: Ctx,
    span: Span,
) -> Result<Ctx, Error> {
    let stamp = attrs.number("px-per-unit");
    let sheet = stamp.is_none() && drawing::is_sheet(kind, type_chain);
    let base = stamp.unwrap_or(if sheet { 1.0 } else { ctx.base });
    let inherited = if sheet { 1.0 } else { ctx.ratio };
    let ratio = match attrs.get("scale") {
        None => inherited,
        Some(v) => match v.as_number() {
            Some(r) if r > 0.0 => r,
            _ => return Err(Error::at(span, "'scale' must be > 0")),
        },
    };
    Ok(Ctx {
        scale: base * ratio,
        base,
        ratio,
        ..ctx
    })
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

/// The links a container scope **owns** [SPEC 9] — the statements written in
/// that very container, which its `layout:` then realises ([SPEC 11] seam 2).
/// `owner` is the container's own span (`None` at the scene root), the identity
/// resolve stamped on each link: a dot-path cannot serve, because an anonymous
/// container shares its parent's — scope-transparency is about names, not
/// geometry. The path still tells define-inlined twins of one container apart.
pub(crate) fn scope_links<'a>(
    program: &'a Program,
    path: &str,
    owner: Option<crate::span::Span>,
) -> Vec<&'a crate::resolve::ResolvedLink> {
    program
        .links
        .iter()
        .filter(|w| w.scope == path && w.written_in.span == owner)
        .collect()
}

/// The scene instance at a dot-path (`""` → `None`: the root is not an instance).
/// Walks by id, like an endpoint path — descending through **anonymous**
/// containers, which are scope-transparent [SPEC 9]. Used by the scope
/// detectors and the sequence engine.
pub(super) fn node_at<'a>(program: &'a Program, path: &str) -> Option<&'a ResolvedInst> {
    crate::resolve::scene::walk_scope(&program.scene.nodes, path.split('.'))
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
pub fn layout_hints(laid: &LaidOut, program: &Program) -> Vec<crate::error::Diagnostic> {
    let mut out = extent_hints(laid, program);
    out.extend(schematic::seat_hints(laid, program));
    out
}

fn extent_hints(laid: &LaidOut, program: &Program) -> Vec<crate::error::Diagnostic> {
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
    // `mirror:` reflects what a node holds [SPEC 15.3] — but a raw `d` has no
    // parse/emit round-trip here and a raster no reflection at all, so both
    // read `none`. Naming an axis on one is an error, never a silent no-op;
    // spelling out the reading they already take is not.
    if let Some(v) = inst.attrs.get("mirror")
        && let Some(ty) = match inst.kind {
            NodeKind::Path => Some("path"),
            NodeKind::Image => Some("image"),
            _ => None,
        }
        && matches!(
            drawing::pen::read_mirror(v, inst.span)?,
            drawing::pen::Mirror::Axes(_)
        )
    {
        return Err(Error::at(
            inst.span,
            format!("'|{ty}|' has no reflection — draw it with the pen"),
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
    // A layout-owning engine (chart / pie / sequence / tree / schematic /
    // drawing) owns its whole
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
    } else if tree::is_tree(&inst.attrs) {
        Some(tree::layout_node(inst, path, program)?)
    } else if schematic::is_schematic(&inst.attrs) {
        Some(schematic::layout_node(inst, path, program)?)
    } else if crate::resolve::is_drawing(&inst.attrs) {
        Some(drawing::layout_node(inst, path, program, ctx)?)
    // …and a plain `stack` after it: a drawing answers `is_stack` too, so the
    // narrower engine claims its own scopes first [SPEC 12].
    } else if crate::resolve::is_stack(&inst.attrs) {
        Some(datum::layout_node(inst, path, program, ctx)?)
    } else {
        None
    };
    if let Some(mut placed) = engine {
        mirror::expand(&mut placed)?;
        if placed.attrs.get("pattern").is_some() {
            let own =
                effective_scale(&inst.attrs, inst.kind, &inst.type_chain, ctx, inst.span)?.scale;
            pattern::expand(&mut placed, own)?;
        }
        return Ok(placed);
    }
    // Generated drawing chrome ([SPEC 15.7]) has no geometry of its own — the
    // parent's shape decides it once that shape is sized (below).
    if ctx.drawing && drawing::chrome::is_chrome(&inst.attrs) {
        return Ok(drawing::chrome::placeholder(inst));
    }
    // A drafting symbol lowers off the glyph registry [SPEC 15.9] —
    // drawing-scope only ([SPEC 21]).
    if let Some(ty) = drawing::symbols::drafting_type(&inst.type_chain) {
        if !ctx.drawing {
            return Err(Error::at(
                inst.span,
                format!(
                    "'|{ty}|' annotates a drawing — it belongs in a 'layout: drawing' (or its 'floorplan' dialect)"
                ),
            ));
        }
        return drawing::symbols::layout_node(inst, ty, path, program);
    }

    let scaled = effective_scale(&inst.attrs, inst.kind, &inst.type_chain, ctx, inst.span)?;
    let own = scaled.scale;
    // In a drawing scope a shape's `[ ]` children are its **features** — they
    // datum-place at the part's origin, rigid with it [SPEC 15.4]; a child that
    // owns a layout — or is sheet content (a note, the title) — arranges its
    // interior as usual and places as one box.
    // A **part** is a drawing's recursive law [SPEC 15.4]: a shape's `[ ]` are
    // its features, placed at its own datum, and its children never grow it.
    // That is drafting, not placement — a plain `stack` puts *its own*
    // children on its datum [SPEC 12] and leaves them ordinary boxes, so an
    // `\|box\|` nested in one still lays out its content by the box model, as
    // it does inside every other layout [SPEC 11].
    let part = ctx.drawing
        && !owns_layout(inst.kind, &inst.type_chain, &inst.attrs)
        && !drawing::is_sheet(inst.kind, &inst.type_chain);
    let child_ctx = Ctx {
        // A **bundle** — a layout-owning wrapper of sheet content — stays in
        // the drawing scope [SPEC 15.5]: its drafting children lower here,
        // and the seat moves it whole.
        datum: part || (ctx.datum && drawing::is_bundle(inst)),
        drawing: ctx.drawing && (part || drawing::is_bundle(inst)),
        ..scaled
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
    // A fixture's body is authored geometry on the true-size mm grid
    // [SPEC 15.11] — it sizes the node, so it is drawn before the bbox picks.
    let fixture = if part {
        floorplan::fixtures::plan(inst, own)?
    } else {
        None
    };
    let bbox = if let Some(f) = &fixture {
        f.bbox
    } else if inst.kind == NodeKind::Sketch {
        // The pen folds here [SPEC 15.3]: geometry decides the bbox — never
        // content + padding. Outside a drawing any children still arrange
        // normally over it; in one they are features, datum-placed below.
        if !children.is_empty() && !part {
            let _ = lay_out_container_children(&mut children, &inst.attrs, inst.span, own)?;
        }
        let mut folded = drawing::pen::fold(inst, own)?;
        // A wall's outline replaces its centreline for paint and the geometry
        // bbox [SPEC 15.11] — offset after the fold, before the bboxes
        // ([SPEC 15.10] step 1); its `:segment`s stay centreline stations.
        if floorplan::fp_kind(&inst.type_chain) == Some(floorplan::FpKind::Wall) {
            floorplan::wall::offset(&mut folded, inst, &mut children, own)?;
        }
        let half = inst.attrs.half_stroke();
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
    // …and a fixture's own chrome and smart label seat off the sized body
    // [SPEC 15.11], the same fill-once-sized step.
    if let Some(f) = &fixture {
        floorplan::fixtures::finish(&mut children, f);
    }
    // A page's furniture takes its geometry from the sized sheet, and any
    // title block seats flush inside the frame corner [SPEC 15.8].
    if page::is_page(&inst.type_chain) {
        page::finish(&mut children, bbox, own);
    }
    // A shaped net tag draws its pointed outline over its finished box
    // [SPEC 16.4] — the same fill-once-sized step as the two above.
    if crate::desugar::schematic::sch_kind(&inst.type_chain)
        == Some(crate::desugar::schematic::SchKind::Label)
    {
        schematic::fill_tag(&mut children, bbox);
    }
    // The fillers above have given their chrome geometry, and this node is
    // about to be measured with it — so take it back from the pieces the
    // cascade painted away [SPEC 15.7]. The same sweep runs once more over the
    // finished tree (`frame::finish`), for the producers that generate after
    // their parent is laid out: a radial `pattern:`'s pitch circle, a
    // `revolve:`'s shoulders. Two call sites, one rule — the alternative is
    // the test repeated inside twelve producers, where one will forget it.
    drawing::chrome::drop_unpainted(&mut children);

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
    if let Some(f) = fixture {
        floorplan::fixtures::paint(&mut placed, f);
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
    // `mirror:` reflects the node's **features** [SPEC 15.3] — the pen folded
    // its drawn path already; here its children take the same split, read on
    // their position. Before `pattern:`, as the lowering order states.
    mirror::expand(&mut placed)?;
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
///
/// A **card**'s `direction` is not such a request: a closed shape and a
/// `|topic|` stack their content by type ([SPEC 11], [`ledger::defaults::is_card`]),
/// so a `|hole|`, a `|rect|`, a `|balloon|` stays geometry — its `[ ]` are the
/// part's features, as they were before the class had a `direction` at all.
/// This is the post-cascade twin of `desugar::nest::seals_drawing_scope`, which
/// reads the *authored* style and so never saw a bundled default.
fn owns_layout(
    kind: crate::resolve::NodeKind,
    type_chain: &[String],
    attrs: &crate::resolve::AttrMap,
) -> bool {
    attrs.get("layout").is_some()
        || (attrs.get("direction").is_some() && !crate::ledger::defaults::is_card(kind, type_chain))
}

// ───────────────────────────── Tests ─────────────────────────────

#[cfg(test)]
mod tests;

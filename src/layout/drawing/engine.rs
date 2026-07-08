//! `layout: drawing` [SPEC 15] — the engine. One placement model, whole scope:
//! every child's origin lands on the container's **datum** (`translate:` the
//! only offset), mates seat parts against each other, annotations lower
//! against the seated geometry, and the sheet sizes to the union of its
//! children's paint — annotations included. The scope owns its links; the
//! router never runs here.

use super::super::ir::{Bbox, PlacedNode};
use super::super::{Ctx, anchors, child_path, effective_scale, layout_inst, prim, primitives};
use super::{annotate, mates, place_features};
use crate::error::Error;
use crate::resolve::{LinkKind, Program, ResolvedInst, ResolvedLink, ResolvedValue};
use crate::span::Span;

/// A `|drawing|` **node**: lay out and seat its children, then size border-box
/// around their extent (padding inside, explicit dims a floor — the core law)
/// and pin its sheet chrome (the title footnote) to the finished box.
pub(in crate::layout) fn layout_node(
    inst: &ResolvedInst,
    path: &str,
    program: &Program,
    ctx: Ctx,
) -> Result<PlacedNode, Error> {
    let own = effective_scale(&inst.attrs, ctx.scale, inst.span)?;
    let mut children = lay_out(
        &inst.children,
        path,
        program,
        own,
        unit_of(&inst.attrs),
        inst.span,
        inst.id.is_some(),
    )?;

    // Centre the drawn extent on the node's origin, so the container places in
    // a flow like any box (and a styled drawing's own rect backs its content).
    let extent = flow_extent(&children);
    let (sx, sy) = (
        (extent.min_x + extent.max_x) / 2.0,
        (extent.min_y + extent.max_y) / 2.0,
    );
    for c in children
        .iter_mut()
        .filter(|c| !anchors::is_pinned(&c.attrs))
    {
        c.cx -= sx;
        c.cy -= sy;
    }
    let bbox = primitives::closed_bbox(inst, extent, own)?;
    let half = inst.attrs.number("stroke-width").unwrap_or(0.0) / 2.0;
    place_pinned(&mut children, bbox.inflate(-half))?;
    let mut placed = prim::container(inst, bbox, children);
    // The recentre moved the datum off the node's local zero — record where
    // it landed, so `align/justify: origin` can line views up datum-to-datum
    // [SPEC 12/15.8].
    placed.origin = (-sx, -sy);
    Ok(placed)
}

/// A **root** drawing (`{ layout: drawing; scale: 1 }`): the file is the sheet. Children
/// stay in scene coordinates — the root's padding frames them in `finish`.
pub(in crate::layout) fn layout_root(program: &Program) -> Result<(Vec<PlacedNode>, Bbox), Error> {
    let own = effective_scale(&program.scene.attrs, 1.0, Span::empty())?;
    let mut children = lay_out(
        &program.scene.nodes,
        "",
        program,
        own,
        unit_of(&program.scene.attrs),
        Span::empty(),
        true,
    )?;
    let extent = flow_extent(&children);
    place_pinned(&mut children, extent)?;
    Ok((children, extent))
}

/// The shared body: lay each child out (features, chrome, and patterns fold
/// inside `layout_inst` under the drawing context), place origins on the
/// datum, seat the mates, then lower every other link — dimensions, leaders,
/// annotation arrows — against the seated geometry [SPEC 15.9]. The
/// annotations append after the children, so they paint above the geometry
/// (`layer:` still wins) and size into the drawing's bbox.
fn lay_out(
    insts: &[ResolvedInst],
    path: &str,
    program: &Program,
    own: f64,
    unit: Option<&str>,
    span: Span,
    owns_links: bool,
) -> Result<Vec<PlacedNode>, Error> {
    let ctx = Ctx {
        scale: own,
        drawing: true,
    };
    let mut kids = Vec::with_capacity(insts.len());
    for c in insts {
        kids.push(layout_inst(c, &child_path(path, c), program, ctx)?);
    }

    let geometry: Vec<usize> = kids
        .iter()
        .enumerate()
        .filter(|(_, k)| !super::is_sheet(k.kind, &k.type_chain) && !anchors::is_pinned(&k.attrs))
        .map(|(i, _)| i)
        .collect();
    if geometry.is_empty() {
        return Err(Error::at(
            span,
            "a drawing needs at least one geometry child",
        ));
    }

    // The scope's links, in source order: mates seat parts first, and the
    // annotations measure the seated result [SPEC 15.9]. An **anonymous**
    // drawing node is scope-transparent [SPEC 9]: its path is its parent's and
    // its links resolved there — consuming by path would steal the parent's.
    let mut links: Vec<&ResolvedLink> = if owns_links {
        program.links.iter().filter(|w| w.scope == path).collect()
    } else {
        Vec::new()
    };
    links.sort_by_key(|w| w.span.start);
    let (mates, annotations): (Vec<&ResolvedLink>, Vec<&ResolvedLink>) =
        links.iter().partition(|w| w.kind == LinkKind::Mate);

    place_features(&mut kids, own, None)?;
    mates::seat(&mut kids, geometry[0], &mates, path, own)?;
    let mut lowered = annotate::lower(&kids, &annotations, path, own, unit)?;
    kids.append(&mut lowered);
    Ok(kids)
}

/// The drawing's `unit:` — a suffix on auto-measured linear values
/// [SPEC 15.1].
fn unit_of(attrs: &crate::resolve::AttrMap) -> Option<&str> {
    match attrs.get("unit") {
        Some(ResolvedValue::String(u)) => Some(u),
        _ => None,
    }
}

/// The drawn extent of the in-flow children (pinned overlays never grow their
/// parent — the core law; the canvas still includes them via `finish`).
fn flow_extent(kids: &[PlacedNode]) -> Bbox {
    let mut ext = Bbox {
        min_x: f64::INFINITY,
        min_y: f64::INFINITY,
        max_x: f64::NEG_INFINITY,
        max_y: f64::NEG_INFINITY,
    };
    for k in kids.iter().filter(|k| !anchors::is_pinned(&k.attrs)) {
        super::super::accumulate_extent(k, 0.0, 0.0, 0.0, &mut ext);
    }
    ext
}

/// Pin sheet chrome onto the finished box — the title `|footnote|` under the
/// view [SPEC 15.8] — flush per the core pin law, `translate:` after. A pinned
/// overlay's nudge is chrome **anatomy** (the title's 17 px gap), sheet-space
/// like every drafting constant — never the view scale [SPEC 15.1].
fn place_pinned(kids: &mut [PlacedNode], anchor_box: Bbox) -> Result<(), Error> {
    for k in kids.iter_mut().filter(|k| anchors::is_pinned(&k.attrs)) {
        if let Some(pin) = anchors::read_pin(&k.attrs, k.span)? {
            let (cx, cy) = pin.target(anchor_box, k.bbox);
            k.cx = cx;
            k.cy = cy;
        }
        if let Some((dx, dy)) = anchors::translate(&k.attrs, k.span)? {
            k.cx += dx;
            k.cy += dy;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{by_id, laid, layout_err};
    use crate::layout::PlacedNode;
    use crate::resolve::NodeKind;

    // ── Datum placement [SPEC 15.1] ──

    #[test]
    fn primitives_stack_concentric_on_the_datum() {
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|oval#disc| { width: 60; height: 60 }\n|hole#bore| { width: 12 }\n",
        );
        let (disc, bore) = (by_id(&l.nodes, "disc"), by_id(&l.nodes, "bore"));
        assert_eq!((disc.cx, disc.cy), (bore.cx, bore.cy), "origins coincide");
    }

    #[test]
    fn translate_offsets_in_drawing_units_times_scale() {
        let l = laid(
            "{ layout: drawing; scale: 2 }\n|rect#a| { width: 20; height: 10 }\n|rect#b| { width: 20; height: 10; translate: 30 5 }\n",
        );
        let (a, b) = (by_id(&l.nodes, "a"), by_id(&l.nodes, "b"));
        assert!((b.cx - a.cx - 60.0).abs() < 1e-9, "dx={}", b.cx - a.cx);
        assert!((b.cy - a.cy - 10.0).abs() < 1e-9, "dy={}", b.cy - a.cy);
    }

    #[test]
    fn features_ride_in_the_part_rigid() {
        // The hole datum-places at the plate's origin + translate; mating the
        // plate moves the whole subtree (the feature's cx is parent-relative).
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#base| { width: 40; height: 40 }\n|rect#plate| { width: 120; height: 70 } [\n  |hole#pin| { width: 10; translate: -35 20 }\n]\nplate:left || base:right\n",
        );
        let plate = by_id(&l.nodes, "plate");
        let pin = by_id(&plate.children, "pin");
        assert_eq!((pin.cx, pin.cy), (-35.0, 20.0), "feature at the part datum");
        // plate:left flush on base:right → plate centre at 20 + 60 = 80.
        assert!((plate.cx - 80.0).abs() < 1e-6, "plate.cx={}", plate.cx);
    }

    #[test]
    fn a_drawing_needs_geometry() {
        assert_eq!(
            layout_err("{ layout: drawing; scale: 1 }\n\"SECTION A-A\"\n"),
            "a drawing needs at least one geometry child"
        );
    }

    #[test]
    fn a_hole_requires_width() {
        assert!(
            layout_err("{ layout: drawing; scale: 1 }\n|hole#h|\n")
                .contains("'|hole|' requires 'width' — its diameter")
        );
    }

    // ── Annotations [SPEC 15.6] ──

    #[test]
    fn a_linear_dim_measures_the_seated_pre_scale_span() {
        // Two 20-wide rects, b mated flush to a's right at scale 2: the dim
        // reads the anchors after mates, in drawing units — 40, not 80 px.
        let l = laid(
            "{ layout: drawing; scale: 2 }\n|rect#a| { width: 20; height: 20 }\n|rect#b| { width: 20; height: 20 }\nb:left || a:right\na:left (-) b:right { side: bottom }\n",
        );
        let texts: Vec<&PlacedNode> = l
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Text)
            .collect();
        assert!(
            texts.iter().any(|t| t.label.as_deref() == Some("40")),
            "measured 40: {:?}",
            texts.iter().map(|t| &t.label).collect::<Vec<_>>()
        );
        // The dim stacks below the geometry: its text sits under both rects.
        let dim_text = texts
            .iter()
            .find(|t| t.label.as_deref() == Some("40"))
            .unwrap();
        let a = by_id(&l.nodes, "a");
        assert!(
            dim_text.cy > a.cy + a.bbox.max_y,
            "stacked on bottom: text.cy={} a.bottom={}",
            dim_text.cy,
            a.cy + a.bbox.max_y
        );
    }

    // ── The title [SPEC 15.8] ──

    #[test]
    fn a_drawing_title_lowers_to_a_footnote_below_the_view() {
        let l = laid("|drawing#v| \"SECTION A-A\" [\n  |rect#a| { width: 40; height: 20 }\n]\n");
        let v = by_id(&l.nodes, "v");
        let title = v
            .children
            .iter()
            .find(|c| c.type_chain.iter().any(|t| t == "footnote"))
            .expect("the title footnote");
        let a = by_id(&v.children, "a");
        assert!(title.cy > a.cy + a.bbox.max_y, "title sits under the view");
    }

    #[test]
    fn the_title_gap_is_sheet_space_at_any_view_scale() {
        // A pinned overlay's translate is chrome anatomy [SPEC 15.1]: the
        // title's 17 px drafting gap must not grow with the view scale.
        let gap = |scale: u32| {
            let l = laid(&format!(
                "|drawing#v| \"T\" {{ scale: {scale} }} [\n  |rect#a| {{ width: 30; height: 20 }}\n]\n"
            ));
            let v = by_id(&l.nodes, "v");
            let title = v
                .children
                .iter()
                .find(|c| c.type_chain.iter().any(|t| t == "footnote"))
                .expect("title");
            let a = by_id(&v.children, "a");
            (title.cy + title.bbox.min_y) - (a.cy + a.bbox.max_y)
        };
        assert!(
            (gap(1) - gap(4)).abs() < 0.01,
            "gap(1)={} gap(4)={}",
            gap(1),
            gap(4)
        );
    }

    // ── Chrome [SPEC 15.7] ──

    #[test]
    fn a_fused_mirror_generates_its_axis_centerline() {
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|sketch#body| { draw: move(-20, 0) up(8) right(40) down(8); mirror: x-axis }\n",
        );
        let body = by_id(&l.nodes, "body");
        let cl = body
            .children
            .iter()
            .find(|c| c.type_chain.iter().any(|t| t == "centerline"))
            .expect("the auto centerline");
        // Along x, overhanging the 40-wide profile by 4 each side.
        assert!((cl.bbox.w() - 48.0).abs() < 1.5, "w={}", cl.bbox.w());
        assert!(cl.bbox.h() < 2.0, "an axis line: h={}", cl.bbox.h());
    }

    #[test]
    fn a_closed_profile_mirror_generates_no_centerline() {
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|sketch#ears| { draw: move(0, -10) circle(3); mirror: x-axis }\n",
        );
        let ears = by_id(&l.nodes, "ears");
        assert!(
            ears.children
                .iter()
                .all(|c| c.type_chain.iter().all(|t| t != "centerline")),
            "a duplicated mirror draws no axis"
        );
    }

    #[test]
    fn the_cascade_styles_generated_chrome() {
        // The chrome is a real child, so a descendant rule reaches it [SPEC 15.7].
        let src = "{ layout: drawing;\n  |sketch| |centerline| { stroke: none }\n}\n|sketch#s| { draw: move(-20, 0) up(8) right(40) down(8); mirror: x-axis }\n";
        let toks = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(src, &toks).expect("parse");
        let lowered = crate::desugar::desugar(&file).expect("desugar");
        let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
        let laid = crate::layout::layout(&program).expect("layout");
        let s = by_id(&laid.nodes, "s");
        let cl = s
            .children
            .iter()
            .find(|c| c.type_chain.iter().any(|t| t == "centerline"))
            .expect("centerline");
        assert!(
            matches!(cl.attrs.get("stroke"), Some(crate::resolve::ResolvedValue::Ident(v)) if v == "none"),
            "the rule baked onto the chrome"
        );
    }

    #[test]
    fn a_hole_draws_its_centre_marks() {
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#p| { width: 40; height: 40 } [ |hole#h| { width: 10 } ]\n",
        );
        let h = by_id(&l.nodes, "h");
        let marks: Vec<_> = h
            .children
            .iter()
            .filter(|c| c.type_chain.iter().any(|t| t == "centerline"))
            .collect();
        assert_eq!(marks.len(), 2, "the crosshair");
        // ⌀10 + 4 overhang each side = 18 long.
        assert!(marks.iter().any(|m| (m.bbox.w() - 18.0).abs() < 1.5));
        assert!(marks.iter().any(|m| (m.bbox.h() - 18.0).abs() < 1.5));
    }

    // ── Patterns [SPEC 15.4] ──

    #[test]
    fn a_grid_pattern_replicates_marks_per_copy_seed_in_place() {
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#plate| { width: 120; height: 70 } [\n  |hole#pin| { width: 10; translate: -35 -20; pattern: grid(2, 2, 70, 40) }\n]\n",
        );
        let pin = by_id(&l.nodes, "pin");
        assert_eq!(
            (pin.cx, pin.cy),
            (-35.0, -20.0),
            "the carrier keeps the seed position"
        );
        let copies: Vec<_> = pin
            .children
            .iter()
            .filter(|c| c.kind == NodeKind::Oval)
            .collect();
        assert_eq!(copies.len(), 4, "2 × 2 copies");
        assert_eq!(copies[0].cx, 0.0, "the seed is copy one");
        assert!(copies.iter().any(|c| c.cx == 70.0 && c.cy == 40.0));
        // Each copy carries the full lowering — its own centre marks.
        assert!(copies.iter().all(|c| {
            c.children
                .iter()
                .filter(|m| m.type_chain.iter().any(|t| t == "centerline"))
                .count()
                == 2
        }));
    }

    #[test]
    fn a_radial_pattern_rings_its_pitch_circle() {
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|oval#flange| { width: 80; height: 80 }\n|hole#bolt| { width: 8; pattern: radial(6, 28) }\n",
        );
        let bolt = by_id(&l.nodes, "bolt");
        let ring = bolt
            .children
            .iter()
            .find(|c| c.type_chain.iter().any(|t| t == "pitch-circle"))
            .expect("the pitch circle");
        assert!(
            (ring.bbox.w() - 57.0).abs() < 0.1,
            "⌀56 + stroke: {}",
            ring.bbox.w()
        );
        let copies = bolt.children.len() - 1;
        assert_eq!(copies, 6, "six bolts on the circle, none at the centre");
        // First copy at bearing 0 — straight up.
        let first = &bolt.children[1];
        assert!(
            (first.cx, first.cy) == (0.0, -28.0),
            "({}, {})",
            first.cx,
            first.cy
        );
    }

    #[test]
    fn radial_pattern_validates_its_arguments() {
        assert!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|hole#h| { width: 8; pattern: radial(1, 20) }\n"
            )
            .contains("'radial' needs count ≥ 2 and radius > 0")
        );
    }

    // ── Mates [SPEC 15.5] ──

    #[test]
    fn directed_mate_abuts_flush_and_gap_separates() {
        let flush = laid(
            "{ layout: drawing; scale: 1 }\n|rect#a| { width: 40; height: 20 }\n|rect#b| { width: 20; height: 20 }\nb:left || a:right\n",
        );
        let b = by_id(&flush.nodes, "b");
        let a = by_id(&flush.nodes, "a");
        assert!(
            (b.cx - a.cx - 30.0).abs() < 1e-6,
            "20 + 10 flush: {}",
            b.cx - a.cx
        );

        let gapped = laid(
            "{ layout: drawing; scale: 1 }\n|rect#a| { width: 40; height: 20 }\n|rect#b| { width: 20; height: 20 }\nb:left || a:right { gap: -6 }\n",
        );
        let b = by_id(&gapped.nodes, "b");
        let a = by_id(&gapped.nodes, "a");
        assert!(
            (b.cx - a.cx - 24.0).abs() < 1e-6,
            "negative gap inserts: {}",
            b.cx - a.cx
        );
    }

    #[test]
    fn grounding_decides_who_moves_not_operator_order() {
        // `a` is first-declared — the ground — so `a:right || b:left` and
        // `b:left || a:right` both move `b` [SPEC 15.5].
        for src in [
            "{ layout: drawing; scale: 1 }\n|rect#a| { width: 40; height: 20 }\n|rect#b| { width: 20; height: 20 }\na:right || b:left\n",
            "{ layout: drawing; scale: 1 }\n|rect#a| { width: 40; height: 20 }\n|rect#b| { width: 20; height: 20 }\nb:left || a:right\n",
        ] {
            let l = laid(src);
            let (a, b) = (by_id(&l.nodes, "a"), by_id(&l.nodes, "b"));
            assert!(
                (b.cx - a.cx - 30.0).abs() < 1e-6,
                "b moved: {}",
                b.cx - a.cx
            );
        }
    }

    #[test]
    fn a_point_mate_coincides_origins() {
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|oval#barrel| { width: 60; height: 60; translate: 10 5 }\n|oval#cap| { width: 30; height: 30 }\ncap || barrel\n",
        );
        let (barrel, cap) = (by_id(&l.nodes, "barrel"), by_id(&l.nodes, "cap"));
        assert_eq!((cap.cx, cap.cy), (barrel.cx, barrel.cy), "concentric");
    }

    #[test]
    fn a_named_edge_seats_a_part_against_an_interior_face() {
        // `:step` is a vertical face inside the profile; the ring seats on it.
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|sketch#housing| { draw: move(0, 0) up(20) right(30) down(10):step right(30) down(10) close() }\n|rect#ring| { width: 10; height: 8 }\nring:left || housing:step\n",
        );
        let ring = by_id(&l.nodes, "ring");
        let housing = by_id(&l.nodes, "housing");
        // The step face is at x = 30 in the housing frame.
        let face = housing.cx + 30.0;
        assert!(
            (ring.cx - 5.0 - face).abs() < 1e-6,
            "ring's left face on the step: ring.cx={} face={}",
            ring.cx,
            face
        );
    }

    #[test]
    fn islands_ground_their_own_first_node() {
        // Two unconnected pairs: each grounds its first-declared part.
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#a| { width: 20; height: 20 }\n|rect#b| { width: 20; height: 20 }\n|rect#c| { width: 20; height: 20; translate: 0 60 }\n|rect#d| { width: 20; height: 20 }\na:right || b:left\nc:right || d:left\n",
        );
        let (c, d) = (by_id(&l.nodes, "c"), by_id(&l.nodes, "d"));
        assert!(
            (d.cx - c.cx - 20.0).abs() < 1e-6,
            "d seated on c: {}",
            d.cx - c.cx
        );
        // A directed mate constrains the normal axis only — laterally the
        // mover keeps its datum position [SPEC 15.5].
        assert!(d.cy.abs() < 1e-6, "lateral untouched: {}", d.cy);
    }

    #[test]
    fn rotate_then_mate_seats_the_rotated_anchor() {
        // A 90°-turned bar: its `:right` face now points down, parallel to the
        // base's `:top` — the mate stacks them.
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#base| { width: 60; height: 20 }\n|rect#bar| { width: 40; height: 10; rotate: 90 }\nbar:right || base:top\n",
        );
        let (base, bar) = (by_id(&l.nodes, "base"), by_id(&l.nodes, "bar"));
        // base:top at y = −10; the bar's rotated right face must land there:
        // bar centre 20 above it.
        assert!(
            (bar.cy - (base.cy - 10.0 - 20.0)).abs() < 1e-6,
            "bar.cy={} base.cy={}",
            bar.cy,
            base.cy
        );
    }

    // ── Mate errors [SPEC 20] ──

    #[test]
    fn mate_errors_speak_spec() {
        let over = layout_err(
            "{ layout: drawing; scale: 1 }\n|rect#a| { width: 20; height: 20 }\n|rect#b| { width: 20; height: 20 }\na:right || b:left\na:left || b:right\n",
        );
        assert_eq!(
            over,
            "mate over-constrains 'b' — already positioned via 'a:right || b:left'"
        );

        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|rect#a| { width: 20; height: 20 }\n|rect#b| { width: 20; height: 20 }\na:left || b:top\n"
            ),
            "mated anchors must face along one axis — 'a:left || b:top' has no shared normal"
        );

        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|oval#a| { width: 20; height: 20 }\n|oval#b| { width: 20; height: 20 }\na || b { gap: 4 }\n"
            ),
            "a point mate coincides — 'gap' needs directed anchors (sides or named edges)"
        );

        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|rect#plate| { width: 60; height: 40 } [\n  |hole#x| { width: 6; translate: -20 0 }\n  |hole#y| { width: 6; translate: 20 0 }\n]\nplate.x || plate.y\n"
            ),
            "'plate.x' and 'plate.y' are features of one part — a part is rigid"
        );
    }

    #[test]
    fn a_mate_rejects_sheet_content() {
        // A mate seats two geometry nodes [SPEC 15.5]; a note is sheet content.
        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|rect#a| { width: 20; height: 20 }\n|note#n| \"x\"\na:right || n:left\n"
            ),
            "a mate seats geometry — '|note|' is sheet content"
        );
    }

    #[test]
    fn unknown_segment_suggests_names() {
        let msg = layout_err(
            "{ layout: drawing; scale: 1 }\n|sketch#body| { draw: move(0, 0) up(10) right(20):neck down(10) close() }\n|rect#cap| { width: 8; height: 8 }\ncap:left || body:nek\n",
        );
        assert!(msg.contains("no segment ':nek' on 'body'"), "{msg}");
        assert!(msg.contains("':neck'"), "suggests the near name: {msg}");
    }

    // ── Assemblies [SPEC 15.8] ──

    #[test]
    fn a_nested_drawing_is_one_rigid_body() {
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#frame| { width: 30; height: 60 }\n|drawing#pump| [\n  |rect#casing| { width: 40; height: 40 }\n  |hole#inlet| { width: 8 }\n]\npump:left || frame:right { gap: 5 }\n",
        );
        let (frame, pump) = (by_id(&l.nodes, "frame"), by_id(&l.nodes, "pump"));
        let pump_left = pump.cx + pump.bbox.min_x + 1.0; // geometry faces (stroke excluded)
        let frame_right = frame.cx + 15.0;
        assert!(
            (pump_left - frame_right - 5.0).abs() < 1e-6,
            "seated with daylight: left={pump_left} right={frame_right}"
        );
        assert!(
            by_id(&pump.children, "inlet").children.len() >= 2,
            "the sub-view kept its chrome"
        );
    }

    // ── Anonymous containers are scope-transparent [SPEC 9] ──

    #[test]
    fn an_anonymous_wrapper_keeps_its_drawings_scoped() {
        // The id-less |page| bug: a drawing inside an anonymous container must
        // still own its links — the scope walk descends through the wrapper.
        let l = laid(
            "|group| [\n  |drawing#d| { scale: 1 } [\n    |rect#bar| { width: 60; height: 20 }\n    bar:left (-) bar:right { side: bottom }\n  ]\n]\n",
        );
        super::super::testutil::text_at(&l.nodes, "60");
    }

    #[test]
    fn a_dim_reaches_through_an_anonymous_wrapper() {
        // A feature inside an id-less sealed wrapper is addressable — the
        // anchor walk descends the wrapper's placed hops.
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|row| [ |rect#x| { width: 40; height: 10 } ]\nx:left (-) x:right { side: bottom }\n",
        );
        super::super::testutil::text_at(&l.nodes, "40");
    }

    #[test]
    fn an_anonymous_sequence_never_steals_its_parents_links() {
        // Its path equals its parent's under transparency; the engine must not
        // consume the parent's wires by that path.
        let l = laid("|box#a| \"A\"\n|box#b| \"B\"\n|sequence| [ |box#p| \"P\" ]\na -> b\n");
        assert_eq!(l.links.len(), 1, "the root wire still routes");
    }

    #[test]
    fn wires_still_route_into_anonymous_groups() {
        let l = laid("|group| [\n  |box#a| \"A\"\n  |box#b| \"B\"\n]\na -> b\n");
        assert_eq!(l.links.len(), 1);
    }
}

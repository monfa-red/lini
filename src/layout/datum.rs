//! `layout: stack` [SPEC 12] — the **datum layout**, and the placement core the
//! drawing family is built from.
//!
//! One rule: every child's **origin** lands on the container's datum, and
//! `translate:` is the only offset. Not its bbox centre — a symmetric
//! primitive's origin *is* its centre, so primitives stack concentric, while a
//! `|sketch|`'s origin is its **pen origin**, so several sketches keep the
//! relationship they were drawn in. That last property is the whole reason the
//! layout exists: flow arranges sketches side by side and throws away the one
//! thing a pen frame is for.
//!
//! A `stack` places and stops. `layout: drawing` is this engine plus the
//! drafting apparatus — mates, annotations, generated chrome, `unit:` — so the
//! two share [`lay_out`] and [`contain`] rather than each keeping a copy;
//! [`crate::layout::Ctx`]'s `datum` / `drawing` pair is the split, and only the
//! drawing side turns the second bit on.
//!
//! The module is named for the concept rather than the keyword: `layout::stack`
//! is already the outward band packer [SPEC 15.6], a different kind of stacking.

use super::ir::{Bbox, PlacedNode};
use super::{Ctx, anchors, child_path, drawing, effective_scale, layout_inst, prim, primitives};
use crate::error::Error;
use crate::resolve::{NodeKind, Program, ResolvedInst};
use crate::span::Span;

/// The scope's own context: children datum-place, and nothing drafts.
/// A drawing builds its own, with `drawing` set.
fn scope_ctx(scaled: Ctx) -> Ctx {
    Ctx {
        datum: true,
        drawing: false,
        ..scaled
    }
}

/// Lay a scope's children out and seat each on the datum — the shared body.
/// `place_features` is the seating step itself: it reads each child's
/// `translate:` in drawing units and writes its position, which is exactly
/// what "origin on the datum, `translate:` the only offset" means. A drawing
/// runs the same two steps and then adds its own.
pub(in crate::layout) fn lay_out(
    insts: &[ResolvedInst],
    path: &str,
    program: &Program,
    ctx: Ctx,
) -> Result<Vec<PlacedNode>, Error> {
    let mut kids = Vec::with_capacity(insts.len());
    for c in insts {
        kids.push(layout_inst(c, &child_path(path, c), program, ctx)?);
    }
    drawing::place_features(&mut kids, ctx.scale, None)?;
    Ok(kids)
}

/// The drawn extent of the in-flow children — pinned overlays never grow their
/// parent (the core law); the canvas still includes them via `finish`.
pub(in crate::layout) fn extent(kids: &[PlacedNode]) -> Bbox {
    Bbox::extent_of(kids, |k| !anchors::is_pinned(&k.attrs))
}

/// Centre the placed extent on the node's origin and size the container around
/// it, so a stack places in an enclosing flow like any other box. The datum's
/// landing is recorded on the node, which is what lets `align: origin` line two
/// scopes up datum-to-datum [SPEC 12].
pub(in crate::layout) fn contain(
    inst: &ResolvedInst,
    mut children: Vec<PlacedNode>,
    own: f64,
) -> Result<PlacedNode, Error> {
    let ext = extent(&children);
    let (sx, sy) = ext.center();
    for c in children
        .iter_mut()
        .filter(|c| !anchors::is_pinned(&c.attrs))
    {
        c.cx -= sx;
        c.cy -= sy;
    }
    let bbox = primitives::closed_bbox(inst, ext, own)?;
    let half = drawing::half_stroke(&inst.attrs);
    anchors::place_pinned(&mut children, bbox.inflate(-half))?;
    let mut placed = prim::container(inst, bbox, children);
    placed.origin = (-sx, -sy);
    Ok(placed)
}

/// A `|stack|` **node**: place its children on its datum, then size to them.
pub(in crate::layout) fn layout_node(
    inst: &ResolvedInst,
    path: &str,
    program: &Program,
    ctx: Ctx,
) -> Result<PlacedNode, Error> {
    let scaled = effective_scale(&inst.attrs, inst.kind, &inst.type_chain, ctx, inst.span)?;
    let children = lay_out(&inst.children, path, program, scope_ctx(scaled))?;
    contain(inst, children, scaled.scale)
}

/// A **root** stack (`{ layout: stack }`): the file is the canvas. Children stay
/// in scene coordinates — the root's padding frames them in `finish`.
pub(in crate::layout) fn layout_root(program: &Program) -> Result<(Vec<PlacedNode>, Bbox), Error> {
    let scaled = effective_scale(
        &program.scene.attrs,
        NodeKind::Block,
        &[],
        Ctx::sheet(),
        Span::empty(),
    )?;
    let mut children = lay_out(&program.scene.nodes, "", program, scope_ctx(scaled))?;
    let ext = extent(&children);
    anchors::place_pinned(&mut children, ext)?;
    Ok((children, ext))
}

#[cfg(test)]
mod tests {
    use super::super::drawing::testutil::{by_id, laid};

    /// The reason the layout exists [SPEC 12]: two sketches drawn in one frame
    /// keep it. Flow would arrange them side by side and throw the frame away.
    #[test]
    fn sketches_share_one_frame() {
        let l = laid(
            "{ layout: stack; padding: 0 }\n\
             |sketch#a| { draw: move(0, 0) right(40) down(20) close(); stroke: none }\n\
             |sketch#b| { draw: move(10, 5) right(20) down(10) close(); stroke: none }\n",
        );
        // Neither is displaced: each pen's own coordinates are the position.
        for id in ["a", "b"] {
            let n = by_id(&l.nodes, id);
            assert_eq!((n.cx, n.cy), (0.0, 0.0), "{id} was moved");
        }
        // The canvas is their union, not a row of two.
        assert_eq!((l.viewbox.w, l.viewbox.h), (40.0, 20.0));
    }

    /// A stack draws in pixels unless it says otherwise [SPEC 12]; `unit: mm`
    /// with a root `density:` opts into physical millimetres, exactly as a
    /// drawing does.
    #[test]
    fn pixels_by_default_millimetres_on_request() {
        let px = laid(
            "{ layout: stack; padding: 0 }\n\
             |sketch#a| { draw: move(0, 0) right(40) down(20) close(); stroke: none }\n",
        );
        assert_eq!((px.viewbox.w, px.viewbox.h), (40.0, 20.0), "1 : 1");

        let mm = laid(
            "{ layout: stack; padding: 0; unit: mm; density: 10 }\n\
             |sketch#a| { draw: move(0, 0) right(4) down(2) close(); stroke: none }\n",
        );
        assert_eq!((mm.viewbox.w, mm.viewbox.h), (40.0, 20.0), "10 px per mm");
    }

    /// A stack is not a drawing: it places, and generates none of the chrome
    /// drafting always draws [SPEC 12/15.7]. A fused `mirror:` still folds —
    /// the pen is core — but no axis line comes with it.
    #[test]
    fn a_fused_mirror_draws_no_centerline() {
        let l = laid(
            "{ layout: stack; padding: 0 }\n\
             |sketch#body| { draw: move(-20, 0) up(8) right(40) down(8); mirror: x-axis }\n",
        );
        let body = by_id(&l.nodes, "body");
        assert!(
            body.children
                .iter()
                .all(|c| c.type_chain.iter().all(|t| t != "centerline")),
            "a stack drafts nothing"
        );
        // …and the fold itself still happened: 40 wide, plus the 2-wide stroke.
        assert_eq!(l.viewbox.w, 42.0);
    }

    /// The container is still a box [SPEC 11]: a `|stack|` sizes to its
    /// children and places in an enclosing flow like any other node.
    #[test]
    fn a_stack_node_places_in_a_flow() {
        let l = laid(
            "|box#before| \"before\"\n\
             |stack#art| [\n\
               |sketch| { draw: move(0, 0) right(40) down(20) close() }\n\
             ]\n\
             before -> art\n",
        );
        let art = by_id(&l.nodes, "art");
        assert_eq!(
            (art.bbox.w(), art.bbox.h()),
            (42.0, 22.0),
            "sized to content"
        );
        assert!(art.cx > by_id(&l.nodes, "before").cx, "flowed after it");
        assert_eq!(l.links.len(), 1, "its links route like any diagram's");
    }
}

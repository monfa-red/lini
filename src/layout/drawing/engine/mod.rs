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
use crate::resolve::{LinkKind, Program, ResolvedInst, ResolvedLink};
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
    // `of:` sources the view from a marker [SPEC 15.8]. A `|magnifier|` re-lays
    // the geometry it rings — a detail (a 2D re-render). A `|plane|` only names
    // the cut: the section face is authored here, and the marker composes the
    // `A-A` title. No `of:` — an ordinary drawing.
    let mut children = match super::section::resolve_of(inst, program)? {
        Some(super::section::OfView::Detail {
            marker,
            host,
            letter,
        }) => super::section::layout_detail(inst, path, program, own, marker, host, &letter)?,
        of => {
            let mut c = lay_out(
                &inst.children,
                path,
                program,
                own,
                inst.span,
                inst.id.is_some(),
            )?;
            if let Some(super::section::OfView::Section { letter }) = of {
                super::section::fill_of_title(&mut c, "section", &letter, ratio_of(&inst.attrs));
            }
            c
        }
    };

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

/// A **root** drawing (`{ layout: drawing; density: 1 }`): the file is the sheet. Children
/// stay in scene coordinates — the root's padding frames them in `finish`.
pub(in crate::layout) fn layout_root(program: &Program) -> Result<(Vec<PlacedNode>, Bbox), Error> {
    let own = effective_scale(&program.scene.attrs, 1.0, Span::empty())?;
    let mut children = lay_out(&program.scene.nodes, "", program, own, Span::empty(), true)?;
    let extent = flow_extent(&children);
    place_pinned(&mut children, extent)?;
    Ok((children, extent))
}

/// The shared body: lay each child out (features, chrome, and patterns fold
/// inside `layout_inst` under the drawing context), place origins on the
/// datum, seat the mates, then lower every other link — dimensions, leaders,
/// annotation arrows — against the seated geometry [SPEC 15.10]. The
/// annotations append after the children, so they paint above the geometry
/// (`layer:` still wins) and size into the drawing's bbox.
fn lay_out(
    insts: &[ResolvedInst],
    path: &str,
    program: &Program,
    own: f64,
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
        .filter(|(_, k)| {
            !super::is_sheet(k.kind, &k.type_chain)
                && !anchors::is_pinned(&k.attrs)
                && !super::chrome::is_chrome(&k.attrs)
        })
        .map(|(i, _)| i)
        .collect();
    if geometry.is_empty() {
        return Err(Error::at(
            span,
            "a drawing needs at least one geometry child",
        ));
    }

    // The scope's links, in source order: mates seat parts first, and the
    // annotations measure the seated result [SPEC 15.10]. An **anonymous**
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
    // The section chrome fills from the seated geometry's extent [SPEC 15.8]:
    // the plane's ISO anatomy and the detail markers' rim letters.
    let geo_extent = geometry.iter().fold(Bbox::empty(), |b, &i| {
        b.union(kids[i].bbox.shifted(kids[i].cx, kids[i].cy))
    });
    super::section::fill_planes(&mut kids, geo_extent, own)?;
    super::section::place_detail_labels(&mut kids);
    let mut lowered = annotate::lower(&kids, &annotations, path, own, None)?;
    kids.append(&mut lowered);
    Ok(kids)
}

/// The view's drafting **ratio** — the authored `scale:` (default 1), read
/// for the composed section / detail titles [SPEC 15.8]. The engine's
/// multiplier is the folded `px-per-unit:` [SPEC 15.1], not this.
fn ratio_of(attrs: &crate::resolve::AttrMap) -> f64 {
    attrs.number("scale").unwrap_or(1.0)
}

/// The drawn extent of the in-flow children (pinned overlays never grow their
/// parent — the core law; the canvas still includes them via `finish`).
fn flow_extent(kids: &[PlacedNode]) -> Bbox {
    Bbox::extent_of(kids, |k| !anchors::is_pinned(&k.attrs))
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
mod tests;

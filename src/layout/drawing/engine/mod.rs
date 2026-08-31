//! `layout: drawing` [SPEC 15] — the engine. One placement model, whole scope:
//! every child's origin lands on the container's **datum** (`translate:` the
//! only offset), mates seat parts against each other, annotations lower
//! against the seated geometry, and the sheet sizes to the union of its
//! children's paint — annotations included. The scope owns its links; the
//! router never runs here.

use super::super::ir::{Bbox, PlacedNode};
use super::super::{Ctx, anchors, datum, effective_scale};
use super::{annotate, mates};
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
    let scaled = effective_scale(&inst.attrs, inst.kind, &inst.type_chain, ctx, inst.span)?;
    let own = scaled.scale;
    // `of:` sources the view from a marker [SPEC 15.8]. A `|magnifier|` re-lays
    // the geometry it rings — a detail (a 2D re-render). A `|plane|` only names
    // the cut: the section face is authored here, and the marker composes the
    // `A-A` title. No `of:` — an ordinary drawing.
    let children = match super::section::resolve_of(inst, program)? {
        Some(super::section::OfView::Detail {
            marker,
            host,
            letter,
        }) => super::section::layout_detail(inst, path, program, scaled, marker, host, &letter)?,
        of => {
            let mut c = lay_out(
                &inst.children,
                path,
                program,
                scaled,
                inst.span,
                Some(inst.span),
            )?;
            if let Some(super::section::OfView::Section { letter }) = of {
                super::section::fill_of_title(&mut c, "section", &letter, scaled.ratio);
            }
            c
        }
    };

    // Sizing is the datum layout's [SPEC 12] — a drawing is a stack that also
    // drafts, so the recentre, the bbox and the pinned pass are shared, not
    // repeated here.
    datum::contain(inst, children, own)
}

/// A **root** drawing (`{ layout: drawing; density: 1 }`): the file is the sheet. Children
/// stay in scene coordinates — the root's padding frames them in `finish`.
pub(in crate::layout) fn layout_root(program: &Program) -> Result<(Vec<PlacedNode>, Bbox), Error> {
    let scaled = effective_scale(
        &program.scene.attrs,
        crate::resolve::NodeKind::Block,
        &[],
        Ctx::sheet(),
        Span::empty(),
    )?;
    let mut children = lay_out(
        &program.scene.nodes,
        "",
        program,
        scaled,
        Span::empty(),
        None,
    )?;
    let extent = datum::extent(&children);
    anchors::place_pinned(&mut children, extent)?;
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
    scaled: Ctx,
    span: Span,
    owner: Option<Span>,
) -> Result<Vec<PlacedNode>, Error> {
    let own = scaled.scale;
    // Place first, exactly as a plain `stack` does [SPEC 12]; everything below
    // is the drafting a drawing adds on top.
    let mut kids = datum::lay_out(
        insts,
        path,
        program,
        Ctx {
            datum: true,
            drawing: true,
            ..scaled
        },
    )?;

    let geometry: Vec<usize> = kids
        .iter()
        .enumerate()
        .filter(|(_, k)| super::is_geometry(k) && !super::chrome::is_chrome(&k.attrs))
        .map(|(i, _)| i)
        .collect();
    if geometry.is_empty() {
        return Err(Error::at(
            span,
            "a drawing needs at least one geometry child",
        ));
    }

    // The scope's links, in source order: mates seat parts first, and the
    // annotations measure the seated result [SPEC 15.10]. The owner is the
    // drawing's own identity, not its path — an anonymous one shares its
    // parent's path, and taking links by path would steal the parent's
    // [SPEC 9].
    let mut links: Vec<&ResolvedLink> = super::super::scope_links(program, path, owner);
    links.sort_by_key(|w| w.span.start);
    let (mates, annotations): (Vec<&ResolvedLink>, Vec<&ResolvedLink>) =
        links.iter().partition(|w| w.kind == LinkKind::Mate);

    // The `||` statements: the mate walk, then the seats — the returned
    // seated annotations register as packer obstacles [SPEC 15.5/15.6].
    let seated = mates::seat(&mut kids, geometry[0], &mates, path, own)?;
    // The section chrome fills from the seated geometry's extent [SPEC 15.8]:
    // the plane's ISO anatomy and the detail markers' rim letters.
    let geo_extent = geometry.iter().fold(Bbox::empty(), |b, &i| {
        b.union(kids[i].bbox.shifted(kids[i].cx, kids[i].cy))
    });
    super::section::fill_planes(&mut kids, geo_extent, own)?;
    super::section::place_detail_labels(&mut kids);
    let mut lowered = annotate::lower(&kids, &annotations, path, own, None, &seated, program)?;
    kids.append(&mut lowered);
    Ok(kids)
}

#[cfg(test)]
mod tests;

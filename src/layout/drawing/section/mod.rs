//! Sections & details [SPEC 15.8] — the plane's ISO anatomy and the
//! detail marker's rim label, composed from a seated view once its geometry
//! extent is known.
//!
//! A `|plane|` is authored chrome: desugar marks it, layout intercepts
//! it as a placeholder, and here it fills from the view's box + `at:` /
//! `facing:` into the ISO plane — a thin dash-dot line across the geometry and
//! its overhang, thick end strokes, a viewing-direction arrow at each end, and
//! the section letter beside them. A `|magnifier|` is an ordinary round
//! feature; only its smart label moves out to the rim at 45°.

use super::super::ir::{Bbox, PlacedNode};
use super::super::{Ctx, child_path, layout_inst, prim};
use super::annotate::Paint;
use super::compose::{fmt, section_title};
use super::dims;
use super::geometry::P;
use crate::error::Error;
use crate::ledger::consts::{
    NOTE_OFFSET, PLANE_ARROW_SHAFT, PLANE_LETTER_GAP, PLANE_LETTER_SIZE, PLANE_OVERHANG,
    PLANE_THICK_END, PLANE_THICK_WIDTH,
};
use crate::resolve::NodeKind;
use crate::resolve::{Program, ResolvedInst, ResolvedLink, ResolvedValue};

mod plane;

#[cfg(test)]
mod tests;

pub(in crate::layout) use plane::fill_planes;

/// Move each `|magnifier|`'s smart label out to the rim at 45° [SPEC 15.8]:
/// the circle rings a region at the view scale, but the letter is sheet-space,
/// set `NOTE_OFFSET` beyond the rim on the upper-right diagonal.
pub(in crate::layout) fn place_detail_labels(kids: &mut [PlacedNode]) {
    for k in kids.iter_mut() {
        if !k.type_chain.iter().any(|t| t == "magnifier") {
            continue;
        }
        // The rim (`r` already carries the view scale) plus the sheet-space
        // note offset, on the upper-right diagonal.
        let off = (k.bbox.w() / 2.0 + NOTE_OFFSET) * std::f64::consts::FRAC_1_SQRT_2;
        for c in k.children.iter_mut().filter(|c| c.kind == NodeKind::Text) {
            c.cx = off;
            c.cy = -off;
        }
    }
}

/// Set a placeholder title `|footnote|`'s text and size it [SPEC 15.8]. The
/// text child inherits the footnote's font and colour (`--footer-color`), so
/// `|drawing| |footnote| { … }` styles a composed title like any authored one.
pub(super) fn fill_footnote(foot: &mut PlacedNode, title: &str) {
    let fs = foot.attrs.number("font-size").unwrap_or(12.0);
    let text = prim::text_plain(title, 0.0, 0.0, fs, crate::font::Font::of(&foot.attrs).kind);
    foot.bbox = text.bbox;
    foot.children = vec![text];
}

/// A view sourced from a marker via `of:` [SPEC 15.8].
pub(in crate::layout) enum OfView<'a> {
    /// `of:` a `|plane|` — a **section**: the cut face is authored, and the
    /// marker only composes the doubled `A-A` title.
    Section { letter: String },
    /// `of:` a `|magnifier|` — a **detail**: re-render the region it rings.
    Detail {
        marker: &'a ResolvedInst,
        host: &'a ResolvedInst,
        letter: String,
    },
}

/// Resolve a drawing's `of:` to its marker [SPEC 15.8] — found by id, like a
/// chart's `axis:`. `None` when absent; a `Section` for a `|plane|`, a `Detail`
/// for a `|magnifier|`. A detail can't magnify a marker inside another sourced
/// view (a nested detail / section).
pub(in crate::layout) fn resolve_of<'a>(
    inst: &ResolvedInst,
    program: &'a Program,
) -> Result<Option<OfView<'a>>, Error> {
    let Some(ResolvedValue::Ident(id)) = inst.attrs.get("of") else {
        return Ok(None);
    };
    let (marker, host) = find_marker(&program.scene.nodes, id, None)
        .ok_or_else(|| Error::at(inst.span, format!("'of' finds no marker '{id}'")))?;
    let letter = marker_letter(marker);
    if marker.type_chain.iter().any(|t| t == "magnifier") {
        if host.attrs.get("of").is_some() {
            return Err(Error::at(
                inst.span,
                "a detail magnifies a base view — 'of' can't name a marker inside another sourced view",
            ));
        }
        Ok(Some(OfView::Detail {
            marker,
            host,
            letter,
        }))
    } else if marker.type_chain.iter().any(|t| t == "plane") {
        Ok(Some(OfView::Section { letter }))
    } else {
        Err(Error::at(
            inst.span,
            format!("'of' names '{id}', not a '|plane|' or '|magnifier|'"),
        ))
    }
}

/// The auto detail view [SPEC 15.8]: re-lay the `|magnifier|`'s **host
/// geometry** at the detail scale, shift the region centre to the datum, clip
/// to the circle (drawing its boundary), lower the detail's own annotations
/// against the clones, and title it `C (ratio)` from the marker's letter.
/// Returns the detail's children; the engine sizes and places the container.
pub(in crate::layout) fn layout_detail(
    inst: &ResolvedInst,
    path: &str,
    program: &Program,
    own: f64,
    marker: &ResolvedInst,
    host: &ResolvedInst,
    letter: &str,
) -> Result<Vec<PlacedNode>, Error> {
    let center =
        super::super::anchors::translate(&marker.attrs, marker.span)?.unwrap_or((0.0, 0.0));
    let diameter = marker
        .attrs
        .number("width")
        .ok_or_else(|| Error::at(marker.span, "'|magnifier|' requires 'width' — its diameter"))?;
    let r = diameter / 2.0 * own;

    // Re-lay the host's geometry children at the detail scale — layout_inst is
    // re-entrant, a different scale the whole trick — then shift the region
    // centre onto the datum.
    let ctx = Ctx {
        scale: own,
        drawing: true,
    };
    let mut clones = Vec::new();
    for c in host.children.iter().filter(|c| is_relaid_geometry(c)) {
        clones.push(layout_inst(c, &child_path(path, c), program, ctx)?);
    }
    super::place_features(&mut clones, own, None)?;
    for c in &mut clones {
        c.cx -= center.0 * own;
        c.cy -= center.1 * own;
    }

    // The detail's own annotations, against the clones — dims stack outside the
    // region **circle**, not the clipped-away part.
    let circle = Bbox::centered(2.0 * r, 2.0 * r);
    let links: Vec<&ResolvedLink> = if inst.id.is_some() {
        program.links.iter().filter(|w| w.scope == path).collect()
    } else {
        Vec::new()
    };
    let mut annotations =
        super::annotate::lower(&clones, &links, path, own, Some(circle), &[], program)?;

    // Clip the clones to the region circle (one interned <clipPath> at render),
    // then draw the boundary circle over it [SPEC 15.8].
    let mut clip_group = prim::group(clones, Vec::new(), circle);
    clip_group.attrs.insert("clip", ResolvedValue::Number(r));

    // The detail's own children (the placeholder title footnote), composed from
    // the marker's letter.
    let mut own_kids = Vec::new();
    for c in &inst.children {
        own_kids.push(layout_inst(c, &child_path(path, c), program, ctx)?);
    }
    fill_of_title(
        &mut own_kids,
        "detail",
        letter,
        inst.attrs.number("scale").unwrap_or(1.0),
    );

    let mut kids = vec![clip_group, boundary_circle(inst, r)];
    kids.append(&mut annotations);
    kids.append(&mut own_kids);
    Ok(kids)
}

/// Fill the `of-title` footnote a marker-sourced view seeded [SPEC 15.8]: a
/// `|plane|` (kind `section`) reads `A-A`, a `|magnifier|` (kind `detail`) `C`,
/// both with the drafting ratio.
pub(in crate::layout) fn fill_of_title(
    kids: &mut [PlacedNode],
    kind: &str,
    letter: &str,
    ratio: f64,
) {
    let title = section_title(kind, letter, ratio);
    for k in kids
        .iter_mut()
        .filter(|k| k.attrs.get("of-title").is_some())
    {
        fill_footnote(k, &title);
    }
}

/// The detail's boundary circle [SPEC 15.8]: an unfilled ring at the clip
/// radius, drawn over the clipped geometry. Default the geometry weight
/// (`--stroke-dark`, width 2 — a `|drawing|` is frameless, so its default
/// `stroke: none` is *not* the boundary's); an explicit `stroke:` / `stroke-width:`
/// on the view restyles it (`{ of: c; stroke: red }` → a red ring).
fn boundary_circle(inst: &ResolvedInst, r: f64) -> PlacedNode {
    // The rim wears `.lini-magnifier` — it is the marker's other half, and one
    // rule keeps the pair's paint in lockstep [SPEC 15.8]: the thin light
    // stroke of a view boundary (ISO draws it thin — it is chrome, not part
    // geometry), never the part stroke. An authored `stroke`/`stroke-width`
    // on the detail still wins as an inline diff.
    let light = ResolvedValue::LiveVar {
        name: "stroke-light".into(),
        raw: false,
    };
    let stroke = match inst.attrs.get("stroke") {
        Some(ResolvedValue::Ident(s)) if s == "none" => light,
        Some(v) => v.clone(),
        None => light,
    };
    let width = inst
        .attrs
        .number("stroke-width")
        .filter(|w| *w > 0.0)
        .unwrap_or(1.0);
    let mut c = prim::oval(
        0.0,
        0.0,
        2.0 * r,
        2.0 * r,
        ResolvedValue::Ident("none".into()),
    );
    c.attrs.insert("stroke", stroke);
    c.attrs.insert("stroke-width", ResolvedValue::Number(width));
    c.attrs.insert("fill", ResolvedValue::Ident("none".into()));
    c.type_chain.push("magnifier".to_string());
    c
}

/// Find a marker by id and its **host** (the enclosing node) [SPEC 15.8] — a
/// `|plane|` or a `|magnifier|`, matched like a chart's `axis:`.
fn find_marker<'a>(
    nodes: &'a [ResolvedInst],
    id: &str,
    parent: Option<&'a ResolvedInst>,
) -> Option<(&'a ResolvedInst, &'a ResolvedInst)> {
    for n in nodes {
        if n.id.as_deref() == Some(id)
            && n.type_chain
                .iter()
                .any(|t| t == "magnifier" || t == "plane")
        {
            return parent.map(|p| (n, p));
        }
        if let Some(hit) = find_marker(&n.children, id, Some(n)) {
            return Some(hit);
        }
    }
    None
}

/// A host geometry child the detail re-renders [SPEC 15.8]: a real part — not
/// sheet content, and not the authored markers (cutting planes, detail circles)
/// that annotate the source. The source's annotations are links, not children,
/// so they are already excluded.
fn is_relaid_geometry(inst: &ResolvedInst) -> bool {
    !super::sheet_node(inst)
        && !inst
            .type_chain
            .iter()
            .any(|t| t == "plane" || t == "magnifier")
}

/// A marker's letter [SPEC 15.8] — its smart label: a `|plane|` (a `|line|`)
/// keeps it in `label`, an `|magnifier|` (an `|oval|`) as a text child.
fn marker_letter(marker: &ResolvedInst) -> String {
    marker
        .label
        .clone()
        .or_else(|| {
            marker
                .children
                .iter()
                .find(|c| c.kind == NodeKind::Text)
                .and_then(|c| c.label.clone())
        })
        .unwrap_or_default()
}

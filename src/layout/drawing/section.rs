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
use super::annotate::{NOTE_OFFSET, Paint};
use super::compose::{fmt, section_title};
use super::dims;
use super::geometry::P;
use crate::error::Error;
use crate::resolve::NodeKind;
use crate::resolve::{Program, ResolvedInst, ResolvedLink, ResolvedValue};

// The plane anatomy — baked sheet constants [SPEC 10.5], never scaled.
/// The chain line runs past the geometry by this on each end.
const OVERHANG: f64 = 6.0;
/// The thick end stroke's length and (geometry) weight.
const THICK_END: f64 = 10.0;
const THICK_WIDTH: f64 = 2.0;
/// The viewing arrow's shaft, from the line end out along the sight line.
const ARROW_SHAFT: f64 = 13.0;
/// The section letter, just past each arrow.
const LETTER_GAP: f64 = 7.0;
const LETTER_SIZE: f64 = 12.0;

/// Fill every authored `|plane|` among a view's children from its
/// geometry box [SPEC 15.8]: the placeholder becomes the thin chain line and
/// grows the thick ends, viewing arrows, and letters as its children.
pub(in crate::layout) fn fill_planes(
    kids: &mut [PlacedNode],
    geo: Bbox,
    scale: f64,
) -> Result<(), Error> {
    for k in kids.iter_mut() {
        if matches!(k.attrs.get("chrome"), Some(ResolvedValue::Ident(m)) if m == "plane") {
            fill_one(k, geo, scale)?;
        }
    }
    Ok(())
}

/// The station and axis a `plane`'s `at:` places it on [SPEC 15.8]:
/// `at: N` on the model's longer axis (`break:`'s convention), or `at: N axis`.
fn read_at(cp: &PlacedNode, geo: Bbox) -> Result<(f64, P), Error> {
    let longer = if geo.w() >= geo.h() {
        (1.0, 0.0)
    } else {
        (0.0, 1.0)
    };
    let axis = |a: &str| match a {
        "x-axis" => Ok((1.0, 0.0)),
        "y-axis" => Ok((0.0, 1.0)),
        _ => Err(Error::at(
            cp.span,
            "'at' takes a station and an optional x-axis / y-axis",
        )),
    };
    match cp.attrs.get("at") {
        Some(ResolvedValue::Number(n)) => Ok((*n, longer)),
        Some(ResolvedValue::Tuple(t)) => {
            let (Some(n), Some(ResolvedValue::Ident(a))) =
                (t.first().and_then(ResolvedValue::as_number), t.get(1))
            else {
                return Err(Error::at(
                    cp.span,
                    "'at' takes a station and an optional x-axis / y-axis",
                ));
            };
            Ok((n, axis(a)?))
        }
        _ => Ok((0.0, longer)),
    }
}

/// The viewing (sight) direction the arrows point [SPEC 15.8] — `facing:` or
/// the plane's default (`right` for a vertical plane, `down` for a horizontal).
fn read_facing(cp: &PlacedNode, axis: P) -> Result<P, Error> {
    match cp.attrs.get("facing") {
        Some(ResolvedValue::Ident(f)) => match f.as_str() {
            "right" => Ok((1.0, 0.0)),
            "left" => Ok((-1.0, 0.0)),
            "down" => Ok((0.0, 1.0)),
            "up" => Ok((0.0, -1.0)),
            _ => Err(Error::at(
                cp.span,
                "'facing' turns the arrows — left, right, up, or down",
            )),
        },
        _ if axis.0 != 0.0 => Ok((1.0, 0.0)), // a vertical plane → look right
        _ => Ok((0.0, 1.0)),                  // a horizontal plane → look down
    }
}

fn fill_one(cp: &mut PlacedNode, geo: Bbox, scale: f64) -> Result<(), Error> {
    let (n, axis) = read_at(cp, geo)?;
    let facing = read_facing(cp, axis)?;
    let line_dir = (-axis.1, axis.0); // the plane runs perpendicular to its axis

    // The station along the axis; it must sit within the model's extent.
    let s = n * scale;
    let (amin, amax) = project(geo, axis);
    if s < amin - 1e-6 || s > amax + 1e-6 {
        return Err(Error::at(
            cp.span,
            format!("a 'plane' at {} sits off the model", fmt(n)),
        ));
    }

    // The chain line spans the geometry across the axis, plus the overhang.
    let (mut lo, mut hi) = project(geo, line_dir);
    lo -= OVERHANG;
    hi += OVERHANG;
    let at = |t: f64| (axis.0 * s + line_dir.0 * t, axis.1 * s + line_dir.1 * t);
    let (a, b) = (at(lo), at(hi));
    set_points(cp, a, b);

    let paint = Paint::of(&cp.attrs);
    let mut pieces = Vec::new();
    // Thick end strokes, into the line from each end.
    pieces.push(thick_end(
        cp,
        a,
        (line_dir.0 * THICK_END, line_dir.1 * THICK_END),
    ));
    pieces.push(thick_end(
        cp,
        b,
        (-line_dir.0 * THICK_END, -line_dir.1 * THICK_END),
    ));
    // A viewing arrow (and the letter) at each end, along the sight line.
    for &end in &[a, b] {
        let tip = (
            end.0 + facing.0 * ARROW_SHAFT,
            end.1 + facing.1 * ARROW_SHAFT,
        );
        pieces.push(shaft(cp, end, tip));
        pieces.push(dims::arrow(tip, facing, &paint));
        if let Some(letter) = cp.label.clone() {
            let lp = (tip.0 + facing.0 * LETTER_GAP, tip.1 + facing.1 * LETTER_GAP);
            pieces.push(prim::dim_text(&letter, lp.0, lp.1, LETTER_SIZE));
        }
    }

    cp.label = None; // the letter is drawn; the line node itself carries no text
    cp.bbox = seg_bbox(a, b).union(Bbox::extent_of(&pieces, |_| true));
    cp.children = pieces;
    Ok(())
}

/// The `[min, max]` projection of a box's corners onto a unit direction.
fn project(geo: Bbox, dir: P) -> (f64, f64) {
    let corners = [
        (geo.min_x, geo.min_y),
        (geo.max_x, geo.min_y),
        (geo.min_x, geo.max_y),
        (geo.max_x, geo.max_y),
    ];
    corners
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), c| {
            let t = c.0 * dir.0 + c.1 * dir.1;
            (lo.min(t), hi.max(t))
        })
}

fn set_points(n: &mut PlacedNode, a: P, b: P) {
    n.attrs
        .insert("points", ResolvedValue::List(vec![point(a), point(b)]));
    n.bbox = seg_bbox(a, b);
}

/// A thick, solid end stroke — a clone of the plane line (its cascade style),
/// re-weighted and undashed.
fn thick_end(cp: &PlacedNode, from: P, d: P) -> PlacedNode {
    let mut e = cp.clone();
    e.children.clear();
    e.attrs.remove("chrome");
    e.attrs
        .insert("stroke-width", ResolvedValue::Number(THICK_WIDTH));
    e.attrs
        .insert("stroke-style", ResolvedValue::Ident("solid".into()));
    set_points(&mut e, from, (from.0 + d.0, from.1 + d.1));
    e
}

/// A thin, solid arrow shaft — the plane line's tone at its own weight.
fn shaft(cp: &PlacedNode, a: P, b: P) -> PlacedNode {
    let mut s = cp.clone();
    s.children.clear();
    s.attrs.remove("chrome");
    s.attrs
        .insert("stroke-style", ResolvedValue::Ident("solid".into()));
    set_points(&mut s, a, b);
    s
}

fn point(p: P) -> ResolvedValue {
    ResolvedValue::Tuple(vec![ResolvedValue::Number(p.0), ResolvedValue::Number(p.1)])
}

fn seg_bbox(a: P, b: P) -> Bbox {
    Bbox {
        min_x: a.0.min(b.0),
        min_y: a.1.min(b.1),
        max_x: a.0.max(b.0),
        max_y: a.1.max(b.1),
    }
}

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
    let text = prim::text_plain(title, 0.0, 0.0, fs);
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
#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn layout_detail(
    inst: &ResolvedInst,
    path: &str,
    program: &Program,
    own: f64,
    page: f64,
    marker: &ResolvedInst,
    host: &ResolvedInst,
    letter: &str,
) -> Result<Vec<PlacedNode>, Error> {
    let center = translate_of(marker);
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
    let unit = match inst.attrs.get("unit") {
        Some(ResolvedValue::String(u)) => Some(u.as_str()),
        _ => None,
    };
    let mut annotations = super::annotate::lower(&clones, &links, path, own, unit, Some(circle))?;

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
    fill_of_title(&mut own_kids, "detail", letter, own, page);

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
    own: f64,
    page: f64,
) {
    let title = section_title(kind, letter, own, page);
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
    let dark = ResolvedValue::LiveVar {
        name: "stroke-dark".into(),
        raw: false,
    };
    let stroke = match inst.attrs.get("stroke") {
        Some(ResolvedValue::Ident(s)) if s == "none" => dark,
        Some(v) => v.clone(),
        None => dark,
    };
    let width = inst
        .attrs
        .number("stroke-width")
        .filter(|w| *w > 0.0)
        .unwrap_or(2.0);
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
    !super::is_sheet(inst.kind, &inst.type_chain)
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

/// A node's `translate:` in drawing units — `(0, 0)` when absent.
fn translate_of(inst: &ResolvedInst) -> (f64, f64) {
    match inst.attrs.get("translate") {
        Some(ResolvedValue::Tuple(t)) => (
            t.first().and_then(ResolvedValue::as_number).unwrap_or(0.0),
            t.get(1).and_then(ResolvedValue::as_number).unwrap_or(0.0),
        ),
        _ => (0.0, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{by_id, laid, layout_err, texts};

    #[test]
    fn a_plane_spans_the_view_and_names_its_ends() {
        // A 120-wide plate; the plane A–A at the centre (longer axis x → a
        // vertical line), two letters, arrows facing right by default.
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#plate| { width: 120; height: 40 }\n|plane| \"A\" { at: 0 }\n",
        );
        let cp = by_id(&l.nodes, "plate"); // the plane is a sibling; find its texts
        let _ = cp;
        let letters: Vec<_> = texts(&l.nodes)
            .into_iter()
            .filter(|(t, ..)| t == "A")
            .collect();
        assert_eq!(letters.len(), 2, "a letter beside each end: {letters:?}");
    }

    #[test]
    fn at_off_the_model_errors() {
        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|rect#plate| { width: 40; height: 40 }\n|plane| \"A\" { at: 90 }\n",
            ),
            "a 'plane' at 90 sits off the model"
        );
    }

    #[test]
    fn bad_facing_errors() {
        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|rect#plate| { width: 40; height: 40 }\n|plane| \"A\" { at: 0; facing: sideways }\n",
            ),
            "'facing' turns the arrows — left, right, up, or down"
        );
    }

    #[test]
    fn a_detail_view_re_lays_the_region_titles_and_clips_and_dims_the_clone() {
        // A plate with a marker `c`; the detail magnifies it 2:1 (scale 8 over
        // the page's 4) and dimensions the **clone** (40, pre-scale, deferred
        // past resolve) — the source has no such dimension.
        let l = laid(
            "|page#p| { sheet: a5 landscape } [\n  |drawing#m| { scale: 4 } [\n    |rect#plate| { width: 40; height: 20 }\n    |magnifier#c| \"C\" { width: 30 }\n  ]\n  |drawing#d| { of: c; scale: 8 } [\n    plate:left (-) plate:right { side: bottom }\n  ]\n]\n",
        );
        let all = texts(&l.nodes);
        assert!(
            all.iter().any(|(t, ..)| t == "C (2:1)"),
            "composed detail title: {all:?}"
        );
        assert!(
            all.iter().any(|(t, ..)| t == "40"),
            "the clone's dimension: {all:?}"
        );
        let d = by_id(&l.nodes, "d");
        assert!(
            d.children.iter().any(|c| c.attrs.get("clip").is_some()),
            "the detail clips its geometry to the region circle"
        );
    }

    #[test]
    fn of_a_missing_marker_errors() {
        assert!(
            layout_err(
                "|page#p| { sheet: a5 } [\n  |drawing#m| { scale: 4 } [ |rect#r| { width: 10; height: 10 } ]\n  |drawing#d| { of: nope }\n]\n",
            )
            .contains("'of' finds no marker 'nope'")
        );
    }

    #[test]
    fn a_detail_circle_sets_its_letter_at_the_rim() {
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#plate| { width: 60; height: 60 }\n|magnifier#c| \"C\" { width: 20; translate: 15 0 }\n",
        );
        let c = by_id(&l.nodes, "c");
        let letter = c
            .children
            .iter()
            .find(|t| t.label.as_deref() == Some("C"))
            .expect("the rim letter");
        // Up-and-right of the centre (positive x, negative y).
        assert!(
            letter.cx > 0.0 && letter.cy < 0.0,
            "at the 45° rim: {},{}",
            letter.cx,
            letter.cy
        );
    }
}

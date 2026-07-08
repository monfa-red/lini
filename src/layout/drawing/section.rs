//! Sections & details [SPEC 15.8] — the cutting-plane's ISO anatomy and the
//! detail marker's rim label, composed from a seated view once its geometry
//! extent is known.
//!
//! A `|cutting-plane|` is authored chrome: desugar marks it, layout intercepts
//! it as a placeholder, and here it fills from the view's box + `at:` /
//! `facing:` into the ISO plane — a thin dash-dot line across the geometry and
//! its overhang, thick end strokes, a viewing-direction arrow at each end, and
//! the section letter beside them. A `|detail-circle|` is an ordinary round
//! feature; only its smart label moves out to the rim at 45°.

use super::super::ir::{Bbox, PlacedNode};
use super::super::prim;
use super::annotate::{NOTE_OFFSET, Paint};
use super::compose::fmt;
use super::dims;
use super::geometry::P;
use crate::error::Error;
use crate::resolve::{NodeKind, ResolvedValue};

// The cutting-plane anatomy — baked sheet constants [SPEC 10.5], never scaled.
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

/// Fill every authored `|cutting-plane|` among a view's children from its
/// geometry box [SPEC 15.8]: the placeholder becomes the thin chain line and
/// grows the thick ends, viewing arrows, and letters as its children.
pub(in crate::layout) fn fill_cutting_planes(
    kids: &mut [PlacedNode],
    geo: Bbox,
    scale: f64,
) -> Result<(), Error> {
    for k in kids.iter_mut() {
        if matches!(k.attrs.get("chrome"), Some(ResolvedValue::Ident(m)) if m == "cutting-plane") {
            fill_one(k, geo, scale)?;
        }
    }
    Ok(())
}

/// The station and axis a `cutting-plane`'s `at:` places it on [SPEC 15.8]:
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
            format!("a 'cutting-plane' at {} sits off the model", fmt(n)),
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
            pieces.push(prim::text(&letter, lp.0, lp.1, LETTER_SIZE, None, false));
        }
    }

    cp.label = None; // the letter is drawn; the line node itself carries no text
    let mut bbox = seg_bbox(a, b);
    for p in &pieces {
        bbox = bbox.union(p.bbox.shifted(p.cx, p.cy));
    }
    cp.bbox = bbox;
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

/// Move each `|detail-circle|`'s smart label out to the rim at 45° [SPEC 15.8]:
/// the circle rings a region at the view scale, but the letter is sheet-space,
/// set `NOTE_OFFSET` beyond the rim on the upper-right diagonal.
pub(in crate::layout) fn place_detail_labels(kids: &mut [PlacedNode]) {
    for k in kids.iter_mut() {
        if !k.type_chain.iter().any(|t| t == "detail-circle") {
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

#[cfg(test)]
mod tests {
    use super::super::testutil::{by_id, laid, layout_err, texts};

    #[test]
    fn a_cutting_plane_spans_the_view_and_names_its_ends() {
        // A 120-wide plate; the plane A–A at the centre (longer axis x → a
        // vertical line), two letters, arrows facing right by default.
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#plate| { width: 120; height: 40 }\n|cutting-plane| \"A\" { at: 0 }\n",
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
                "{ layout: drawing; scale: 1 }\n|rect#plate| { width: 40; height: 40 }\n|cutting-plane| \"A\" { at: 90 }\n",
            ),
            "a 'cutting-plane' at 90 sits off the model"
        );
    }

    #[test]
    fn bad_facing_errors() {
        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|rect#plate| { width: 40; height: 40 }\n|cutting-plane| \"A\" { at: 0; facing: sideways }\n",
            ),
            "'facing' turns the arrows — left, right, up, or down"
        );
    }

    #[test]
    fn a_detail_circle_sets_its_letter_at_the_rim() {
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#plate| { width: 60; height: 60 }\n|detail-circle#c| \"C\" { width: 20; translate: 15 0 }\n",
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

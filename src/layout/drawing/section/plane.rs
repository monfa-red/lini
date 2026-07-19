//! The cutting-plane geometry [SPEC 15.8]: fill each authored `|plane|` from the view's box + `at:` / `facing:` into the ISO chain line, thick ends, viewing arrows, and letters.

use super::*;
use crate::layout::drawing::geometry::project;

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
    lo -= PLANE_OVERHANG;
    hi += PLANE_OVERHANG;
    let at = |t: f64| (axis.0 * s + line_dir.0 * t, axis.1 * s + line_dir.1 * t);
    let (a, b) = (at(lo), at(hi));
    set_points(cp, a, b);

    let paint = Paint::of(&cp.attrs);
    let mut pieces = Vec::new();
    // Thick end strokes, into the line from each end.
    pieces.push(thick_end(
        cp,
        a,
        (line_dir.0 * PLANE_THICK_END, line_dir.1 * PLANE_THICK_END),
    ));
    pieces.push(thick_end(
        cp,
        b,
        (-line_dir.0 * PLANE_THICK_END, -line_dir.1 * PLANE_THICK_END),
    ));
    // A viewing arrow (and the letter) at each end, along the sight line.
    for &end in &[a, b] {
        let tip = (
            end.0 + facing.0 * PLANE_ARROW_SHAFT,
            end.1 + facing.1 * PLANE_ARROW_SHAFT,
        );
        pieces.push(shaft(cp, end, tip));
        pieces.push(dims::arrow(tip, facing, &paint));
        if let Some(letter) = cp.label.clone() {
            let lp = (
                tip.0 + facing.0 * PLANE_LETTER_GAP,
                tip.1 + facing.1 * PLANE_LETTER_GAP,
            );
            pieces.push(prim::dim_text(
                &letter,
                lp.0,
                lp.1,
                PLANE_LETTER_SIZE,
                paint.font.kind,
            ));
        }
    }

    cp.label = None; // the letter is drawn; the line node itself carries no text
    cp.bbox = Bbox::from_points(&[a, b]).union(Bbox::extent_of(&pieces, |_| true));
    cp.children = pieces;
    Ok(())
}

fn set_points(n: &mut PlacedNode, a: P, b: P) {
    n.attrs
        .insert("points", ResolvedValue::List(vec![point(a), point(b)]));
    n.bbox = Bbox::from_points(&[a, b]);
}

/// A thick, solid end stroke — a clone of the plane line (its cascade style),
/// re-weighted and undashed.
fn thick_end(cp: &PlacedNode, from: P, d: P) -> PlacedNode {
    let mut e = cp.clone();
    e.children.clear();
    e.attrs.remove("chrome");
    e.attrs
        .insert("stroke-width", ResolvedValue::Number(PLANE_THICK_WIDTH));
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

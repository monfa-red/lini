//! `(<)` — the angle [SPEC 15.6]. Binary, between two **line-like** anchors
//! (a named edge, a `|line|` / `|centerline|`, a bbox side): the angle
//! between their directions, the arc drawn at their (extended) intersection,
//! the value riding the arc. Unary, on a named edge of a mirrored sketch:
//! the **included** angle against its own reflection.

use super::super::ir::{Bbox, PlacedNode};
use super::Segment;
use super::anchors::{self, Anchor, Spot, rotated};
use super::annotate::{Ctx, Paint};
use super::compose::{self, Glyph};
use super::geometry::{P, dist, n as fmt_n, reflect_point, unit};
use crate::error::Error;
use crate::resolve::ResolvedLink;

pub(super) fn lower(ctx: &Ctx, w: &ResolvedLink) -> Result<Vec<PlacedNode>, Error> {
    let paint = Paint::of(&w.attrs);
    let mut out = Vec::new();
    if w.endpoints.len() == 1 {
        out.extend(unary(ctx, w, &paint)?);
        return Ok(out);
    }
    for hop in 0..w.endpoints.len() - 1 {
        let a = anchors::resolve(ctx.kids, ctx.scope, &w.endpoints[hop], "dimension")?;
        let b = anchors::resolve(ctx.kids, ctx.scope, &w.endpoints[hop + 1], "dimension")?;
        let edges = ((a.point(), line_dir(&a, w)?), (b.point(), line_dir(&b, w)?));
        out.extend(arc_between(w, &paint, edges.0, edges.1)?);
    }
    Ok(out)
}

/// The included angle of a taper: the edge against its own mirror twin
/// [SPEC 15.6].
fn unary(ctx: &Ctx, w: &ResolvedLink, paint: &Paint) -> Result<Vec<PlacedNode>, Error> {
    let ep = &w.endpoints[0];
    let a = anchors::resolve(ctx.kids, ctx.scope, ep, "dimension")?;
    let Spot::Segment(Segment::Edge(pa, pb)) = a.spot else {
        return Err(two_edges(w));
    };
    let Some(axis) = a.mirrors().first() else {
        let name = ep.point.as_deref().unwrap_or("?");
        return Err(Error::at(
            w.span,
            format!("'(<)' on ':{name}' needs 'mirror:' — no twin to measure against"),
        ));
    };
    let u = axis.dir();
    let mid = ((pa.0 + pb.0) / 2.0, (pa.1 + pb.1) / 2.0);
    let dir = edge_dir(pa, pb);
    let twin_mid = reflect_point(mid, u);
    let twin_dir = reflect_point(dir, u);
    arc_between(
        w,
        paint,
        (a.to_world(mid), rotated(dir, a.rot)),
        (a.to_world(twin_mid), rotated(twin_dir, a.rot)),
    )
}

/// The drawn wedge: legs through `p1` / `p2` along `d1` / `d2` meet at the
/// (extended) intersection; the arc spans the wedge holding both anchors,
/// the value rides its bisector.
fn arc_between(
    w: &ResolvedLink,
    paint: &Paint,
    (p1, d1): (P, P),
    (p2, d2): (P, P),
) -> Result<Vec<PlacedNode>, Error> {
    let denom = d1.0 * d2.1 - d1.1 * d2.0;
    if denom.abs() < 1e-9 {
        return Err(Error::at(
            w.span,
            "the angle's edges are parallel — they never meet",
        ));
    }
    let t = ((p2.0 - p1.0) * d2.1 - (p2.1 - p1.1) * d2.0) / denom;
    let i = (p1.0 + d1.0 * t, p1.1 + d1.1 * t);
    let leg = |p: P, d: P| {
        let v = (p.0 - i.0, p.1 - i.1);
        let len = dist(v, (0.0, 0.0));
        if len > 1e-6 {
            ((v.0 / len, v.1 / len), len)
        } else {
            (d, 0.0)
        }
    };
    let ((u1, l1), (u2, l2)) = (leg(p1, d1), leg(p2, d2));
    let theta = (u1.0 * u2.0 + u1.1 * u2.1)
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees();
    let r = l1.min(l2).clamp(14.0, 40.0);

    let start = (i.0 + u1.0 * r, i.1 + u1.1 * r);
    let end = (i.0 + u2.0 * r, i.1 + u2.1 * r);
    let sweep = u8::from(u1.0 * u2.1 - u1.1 * u2.0 > 0.0);
    let d = format!(
        "M {} {} A {} {} 0 0 {} {} {}",
        fmt_n(start.0),
        fmt_n(start.1),
        fmt_n(r),
        fmt_n(r),
        sweep,
        fmt_n(end.0),
        fmt_n(end.1)
    );
    // The bisector — where the value rides [SPEC 15.6].
    let ub = {
        let v = (u1.0 + u2.0, u1.1 + u2.1);
        let len = dist(v, (0.0, 0.0));
        if len > 1e-6 {
            (v.0 / len, v.1 / len)
        } else {
            (-u1.1, u1.0)
        }
    };
    let apex = (i.0 + ub.0 * r, i.1 + ub.1 * r);
    let bbox = Bbox {
        min_x: start.0.min(end.0).min(apex.0),
        min_y: start.1.min(end.1).min(apex.1),
        max_x: start.0.max(end.0).max(apex.0),
        max_y: start.1.max(end.1).max(apex.1),
    };
    let mut out = vec![paint.stroked_path(d, bbox)];
    let text = compose::compose(
        Glyph::Deg,
        theta,
        None,
        None,
        w.texts.first().map(|t| t.text.as_str()),
        &w.attrs,
        w.span,
    )?;
    let text_c = (
        i.0 + ub.0 * (r + paint.fs / 2.0 + 6.0),
        i.1 + ub.1 * (r + paint.fs / 2.0 + 6.0),
    );
    out.extend(text.nodes(text_c, 0.0, paint.fs, paint.font));
    Ok(out)
}

/// A line-like anchor's direction, or the SPEC 20 error.
fn line_dir(a: &Anchor, w: &ResolvedLink) -> Result<P, Error> {
    a.direction().ok_or_else(|| two_edges(w))
}

fn two_edges(w: &ResolvedLink) -> Error {
    Error::at(
        w.span,
        "an angle reads two edges — a named segment, a '|line|', or a side",
    )
}

fn edge_dir(a: P, b: P) -> P {
    unit((b.0 - a.0, b.1 - a.1))
}

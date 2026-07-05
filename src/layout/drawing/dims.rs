//! Dimension lowering [SPEC 15.6] — `<->` linear spans and chains, and the
//! `(-)` round readings: a named arc → an `R` leader; a bare round node → a
//! `⌀` leader; a round node + side / corner → the **diametral line**; any
//! node + side → the span to the opposite side, ⌀-read and stacked; a
//! mirrored `:segment` → the station span across the axis. Measured values are
//! anchor distances in drawing units — pre-scale, on the unbroken model —
//! and the anatomy (extension lines, slender arrows, ISO-aligned text) is
//! baked sheet constants.

use super::super::ir::PlacedNode;
use super::anchors::{self, Anchor, Spot, rotated};
use super::annotate::{
    ARROW_HALF, ARROW_LEN, Axis, Ctx, EXT_GAP, EXT_OVERSHOOT, Paint, Rows, side_attr, side_unit,
};
use super::compose::{self, DimText, Glyph};
use super::geometry::{P, reflect_point};
use super::{Segment, leaders};
use crate::ast::Side;
use crate::error::Error;
use crate::resolve::{ResolvedLink, ResolvedText};
use crate::span::Span;

/// `a <-> b` (and chains — each hop its own dim, sharing rows naturally)
/// [SPEC 15.6]. A hop's label **replaces** its number; labels map to hops in
/// order.
pub(super) fn linear(
    ctx: &Ctx,
    w: &ResolvedLink,
    rows: &mut Rows,
) -> Result<Vec<PlacedNode>, Error> {
    let paint = Paint::of(&w.attrs);
    let mut out = Vec::new();
    for hop in 0..w.endpoints.len() - 1 {
        let (ea, eb) = (&w.endpoints[hop], &w.endpoints[hop + 1]);
        let a = anchors::resolve(ctx.kids, ctx.scope, ea, "dimension")?;
        let b = anchors::resolve(ctx.kids, ctx.scope, eb, "dimension")?;
        let (pa, pb) = (a.point(), b.point());

        // The axis [SPEC 15.6]: a directed anchor sets it; two must agree.
        let (da, db) = (a.outward().map(dominant), b.outward().map(dominant));
        let axis = match (da, db) {
            (Some(x), Some(y)) if x != y => {
                return Err(Error::at(
                    w.span,
                    format!(
                        "'{} <-> {}' mixes axes — anchor one axis",
                        anchors::spell(ea, ctx.scope),
                        anchors::spell(eb, ctx.scope)
                    ),
                ));
            }
            (Some(x), _) | (_, Some(x)) => x,
            // Point ↔ point: the dominant delta, tie → horizontal.
            (None, None) => {
                if (pb.1 - pa.1).abs() > (pb.0 - pa.0).abs() {
                    Axis::Vertical
                } else {
                    Axis::Horizontal
                }
            }
        };

        let value = span_on(pa, pb, axis) / ctx.scale;
        let label = w.texts.get(hop);
        let text = compose::compose(
            Glyph::None,
            value,
            None,
            label.map(|t| t.text.as_str()),
            None,
            &w.attrs,
            ctx.unit,
            w.span,
        )?;
        let side = stack_side(&w.attrs, axis, corner_pull(&a, &b, axis), w.span)?;
        out.extend(stacked(
            Stacked {
                axis,
                a: pa,
                b: pb,
                text,
                side,
                gap: w.attrs.number("gap"),
                label,
            },
            rows,
            &paint,
        ));
    }
    Ok(out)
}

/// `(-)` — the round measure, unary; the feature picks the reading
/// [SPEC 15.6].
pub(super) fn round(
    ctx: &Ctx,
    w: &ResolvedLink,
    rows: &mut Rows,
) -> Result<Vec<PlacedNode>, Error> {
    let paint = Paint::of(&w.attrs);
    let ep = &w.endpoints[0];
    let a = anchors::resolve(ctx.kids, ctx.scope, ep, "dimension")?;
    let follows = w.texts.first();
    let count = a.pattern_count();
    let no_axis = || {
        let who = super::rel_path(&ep.path, ctx.scope);
        Error::at(
            w.span,
            format!(
                "'(-)' can't pick an axis on '{who}' — anchor a side ('{who}:top (-)') or a segment"
            ),
        )
    };
    let compose = |glyph: Glyph, value: f64| {
        compose::compose(
            glyph,
            value,
            count,
            None,
            follows.map(|t| t.text.as_str()),
            &w.attrs,
            ctx.unit,
            w.span,
        )
    };

    match &a.spot {
        // A named arc knows its radius — an `R` leader onto the arc itself.
        Spot::Segment(Segment::Arc { mid, r }) => {
            let text = compose(Glyph::R, r / ctx.scale)?;
            let aim = a.to_world(*mid);
            Ok(leaders::measured(
                ctx,
                &a,
                aim,
                Some(aim),
                &w.attrs,
                text,
                &paint,
                w.span,
            ))
        }
        // A `circle()` segment — round by construction, a `⌀` leader onto its rim.
        Spot::Segment(Segment::Circle { center, r }) => {
            let text = compose(Glyph::Dia, 2.0 * r / ctx.scale)?;
            let c = a.to_world(*center);
            Ok(leaders::measured_circle(
                ctx, &a, c, *r, &w.attrs, text, &paint, w.span,
            ))
        }
        // A mirrored `:segment` — the station's span across the axis, stacked.
        Spot::Segment(Segment::Edge(..) | Segment::Point(..)) => {
            let m = a.local_point();
            let axis = a
                .mirrors()
                .iter()
                .find(|ax| {
                    let twin = reflect_point(m, ax.dir());
                    super::geometry::dist(m, twin) > 1e-6
                })
                .copied()
                .ok_or_else(no_axis)?;
            let (pa, pb) = (a.point(), a.to_world(reflect_point(m, axis.dir())));
            station(ctx, w, rows, &paint, pa, pb, count, follows)
        }
        // A side anchor: the diametral line through a round node, or the span
        // to the opposite side — ⌀-read, stacked — on anything else.
        Spot::Side(side) => {
            if let Some(d) = a.round_diameter() {
                let dir = spill_dir(&w.attrs, &a).unwrap_or_else(|| {
                    rotated(side_unit(side_name(*side)).expect("a side"), a.rot)
                });
                let text = compose(Glyph::Dia, d / ctx.scale)?;
                return Ok(diametral(centre_of(&a), d / 2.0, dir, text, &paint));
            }
            let g = a.geometry_box();
            let (cx, cy) = ((g.min_x + g.max_x) / 2.0, (g.min_y + g.max_y) / 2.0);
            let (la, lb) = match side {
                Side::Top | Side::Bottom => ((cx, g.min_y), (cx, g.max_y)),
                Side::Left | Side::Right => ((g.min_x, cy), (g.max_x, cy)),
            };
            station(
                ctx,
                w,
                rows,
                &paint,
                a.to_world(la),
                a.to_world(lb),
                count,
                follows,
            )
        }
        Spot::Corner(diag) => {
            let Some(d) = a.round_diameter() else {
                return Err(no_axis());
            };
            let dir = spill_dir(&w.attrs, &a).unwrap_or_else(|| rotated(*diag, a.rot));
            let text = compose(Glyph::Dia, d / ctx.scale)?;
            Ok(diametral(centre_of(&a), d / 2.0, dir, text, &paint))
        }
        // Bare: a round node reads its ⌀ onto the rim; a mirrored sketch its
        // full span across the axis; anything else can't pick an axis.
        Spot::Origin | Spot::Center => {
            if let Some(d) = a.round_diameter() {
                let c = centre_of(&a);
                let text = compose(Glyph::Dia, d / ctx.scale)?;
                return Ok(leaders::measured_circle(
                    ctx,
                    &a,
                    c,
                    d / 2.0,
                    &w.attrs,
                    text,
                    &paint,
                    w.span,
                ));
            }
            let axis = a.mirrors().first().copied().ok_or_else(no_axis)?;
            let g = a.geometry_box();
            let (cx, cy) = ((g.min_x + g.max_x) / 2.0, (g.min_y + g.max_y) / 2.0);
            let perp = {
                let d = axis.dir();
                (-d.1, d.0)
            };
            let (la, lb) = if perp.1.abs() >= perp.0.abs() {
                ((cx, g.min_y), (cx, g.max_y))
            } else {
                ((g.min_x, cy), (g.max_x, cy))
            };
            station(
                ctx,
                w,
                rows,
                &paint,
                a.to_world(la),
                a.to_world(lb),
                count,
                follows,
            )
        }
    }
}

/// A ⌀-read span between two world points, stacked like a linear dim — the
/// station and opposite-side readings share it.
#[allow(clippy::too_many_arguments)]
fn station(
    ctx: &Ctx,
    w: &ResolvedLink,
    rows: &mut Rows,
    paint: &Paint,
    pa: P,
    pb: P,
    count: Option<usize>,
    follows: Option<&ResolvedText>,
) -> Result<Vec<PlacedNode>, Error> {
    let axis = if (pb.1 - pa.1).abs() > (pb.0 - pa.0).abs() {
        Axis::Vertical
    } else {
        Axis::Horizontal
    };
    let text = compose::compose(
        Glyph::Dia,
        span_on(pa, pb, axis) / ctx.scale,
        count,
        None,
        follows.map(|t| t.text.as_str()),
        &w.attrs,
        ctx.unit,
        w.span,
    )?;
    let side = stack_side(&w.attrs, axis, None, w.span)?;
    Ok(stacked(
        Stacked {
            axis,
            a: pa,
            b: pb,
            text,
            side,
            gap: w.attrs.number("gap"),
            label: follows,
        },
        rows,
        paint,
    ))
}

/// One stacked dimension: extension lines springing from the anchors, the
/// dim line on its packed row, slender arrows, ISO-aligned text above the
/// line — flipped outside when the span is too narrow [SPEC 15.6].
struct Stacked<'a> {
    axis: Axis,
    a: P,
    b: P,
    text: DimText,
    side: Side,
    gap: Option<f64>,
    /// The authored label, if any — its `translate:` / `rotate:` override the
    /// auto text placement (the styled-label form).
    label: Option<&'a ResolvedText>,
}

fn stacked(s: Stacked, rows: &mut Rows, paint: &Paint) -> Vec<PlacedNode> {
    let (fs, sw) = (paint.fs, paint.sw);
    let u = |p: P| match s.axis {
        Axis::Horizontal => p.0,
        Axis::Vertical => p.1,
    };
    let cross = |p: P| match s.axis {
        Axis::Horizontal => p.1,
        Axis::Vertical => p.0,
    };
    let pt = |u: f64, c: f64| match s.axis {
        Axis::Horizontal => (u, c),
        Axis::Vertical => (c, u),
    };
    let (ua, ub) = (u(s.a), u(s.b));
    let (u_lo, u_hi) = (ua.min(ub), ua.max(ub));
    let arrow_len = ARROW_LEN * sw;
    let tw = s.text.width(fs);
    let stub = 2.0;
    let fits = u_hi - u_lo >= 2.0 * arrow_len + tw + 6.0;
    // A narrow span flips its arrows outside the extension lines and slides
    // the text past the nearer one — rightward / upward, where it reads.
    let (interval, text_u) = if fits {
        ((u_lo, u_hi), (u_lo + u_hi) / 2.0)
    } else {
        let reach = arrow_len + stub;
        match s.axis {
            Axis::Horizontal => (
                (u_lo - reach, u_hi + reach + 4.0 + tw),
                u_hi + reach + 4.0 + tw / 2.0,
            ),
            Axis::Vertical => (
                (u_lo - reach - 4.0 - tw, u_hi + reach),
                u_lo - reach - 4.0 - tw / 2.0,
            ),
        }
    };
    let line_c = rows.seat(s.side, interval, s.gap);

    let mut out = Vec::new();
    // Extension lines spring from the anchor points exactly [SPEC 15.2],
    // with a small gap, and overshoot past the dim line.
    for p in [s.a, s.b] {
        let toward = (line_c - cross(p)).signum();
        let c0 = cross(p) + EXT_GAP * toward;
        let c1 = line_c + EXT_OVERSHOOT * toward;
        if (c1 - c0) * toward > 0.0 {
            out.push(paint.line(vec![pt(u(p), c0), pt(u(p), c1)]));
        }
    }
    // The dim line (running past the span when the arrows flip outside).
    let (l0, l1) = if fits {
        (u_lo, u_hi)
    } else {
        (u_lo - arrow_len - stub, u_hi + arrow_len + stub)
    };
    out.push(paint.line(vec![pt(l0, line_c), pt(l1, line_c)]));
    // Slender arrows: tips on the extension lines; bodies inside the span,
    // or outside pointing in when flipped.
    let along = match s.axis {
        Axis::Horizontal => (1.0, 0.0),
        Axis::Vertical => (0.0, 1.0),
    };
    let flip = if fits { -1.0 } else { 1.0 };
    out.push(arrow(pt(u_lo, line_c), scale_p(along, flip), paint));
    out.push(arrow(pt(u_hi, line_c), scale_p(along, -flip), paint));
    // ISO-aligned text above the line: horizontal dims read from the bottom,
    // vertical ones from the right (turned −90°) [SPEC 15.6].
    let lift = fs / 2.0 + 2.0;
    let (mut centre, mut rot) = match s.axis {
        Axis::Horizontal => ((text_u, line_c - lift), 0.0),
        Axis::Vertical => ((line_c - lift, text_u), -90.0),
    };
    if let Some(t) = s.label {
        if let Some(r) = t.attrs.number("rotate") {
            rot = r;
        }
        if let Ok(Some((dx, dy))) = translate_of(t) {
            centre = (centre.0 + dx, centre.1 + dy);
        }
    }
    out.extend(s.text.nodes(centre, rot, fs));
    out
}

/// The diametral line [SPEC 15.6]: through a round node's centre along the
/// anchored direction, arrows out against the rims; the value rides the line
/// when it fits inside, else the line overruns the **anchored** rim and
/// carries the text there. Deterministic, no solver.
fn diametral(c: P, r: f64, dir: P, text: DimText, paint: &Paint) -> Vec<PlacedNode> {
    let (fs, sw) = (paint.fs, paint.sw);
    let rim_a = (c.0 + dir.0 * r, c.1 + dir.1 * r);
    let rim_b = (c.0 - dir.0 * r, c.1 - dir.1 * r);
    let arrow_len = ARROW_LEN * sw;
    let tw = text.width(fs);
    let fits = 2.0 * r >= 2.0 * arrow_len + tw + 8.0;
    // ISO alignment: turn with the line, reading from the bottom / right —
    // a vertical line turns its text −90, like a stacked vertical dim.
    let mut theta = dir.1.atan2(dir.0).to_degrees();
    if theta < -90.0 {
        theta += 180.0;
    } else if theta >= 90.0 {
        theta -= 180.0;
    }
    let (ts, tc) = theta.to_radians().sin_cos();
    let up = (ts, -tc);
    let lift = fs / 2.0 + 2.0;

    let mut out = Vec::new();
    let (end, text_c) = if fits {
        (rim_a, c)
    } else {
        let over = 4.0 + tw + 4.0;
        (
            (rim_a.0 + dir.0 * over, rim_a.1 + dir.1 * over),
            (
                rim_a.0 + dir.0 * (4.0 + tw / 2.0),
                rim_a.1 + dir.1 * (4.0 + tw / 2.0),
            ),
        )
    };
    out.push(paint.line(vec![rim_b, end]));
    out.push(arrow(rim_a, dir, paint));
    out.push(arrow(rim_b, scale_p(dir, -1.0), paint));
    out.extend(text.nodes((text_c.0 + up.0 * lift, text_c.1 + up.1 * lift), theta, fs));
    out
}

/// The drafting-slender arrowhead [SPEC 15.6]: ≈ 3 : 1, filled with the dim's
/// stroke and sized by its stroke-width; `dir` is where the tip points.
pub(super) fn arrow(tip: P, dir: P, paint: &Paint) -> PlacedNode {
    let (l, w) = (ARROW_LEN * paint.sw, ARROW_HALF * paint.sw);
    let base = (tip.0 - dir.0 * l, tip.1 - dir.1 * l);
    let perp = (-dir.1, dir.0);
    super::super::prim::poly(
        vec![
            tip,
            (base.0 + perp.0 * w, base.1 + perp.1 * w),
            (base.0 - perp.0 * w, base.1 - perp.1 * w),
        ],
        paint.stroke.clone(),
        1.0,
    )
}

/// The stacking side [SPEC 15.6]: explicit `side:` (validated against the
/// axis), a corner pull, or the axis default — bottom / right.
fn stack_side(
    attrs: &crate::resolve::AttrMap,
    axis: Axis,
    pull: Option<Side>,
    span: Span,
) -> Result<Side, Error> {
    let valid = |s: Side| match axis {
        Axis::Horizontal => matches!(s, Side::Top | Side::Bottom),
        Axis::Vertical => matches!(s, Side::Left | Side::Right),
    };
    let off_axis = || {
        Error::at(
            span,
            match axis {
                Axis::Horizontal => "a horizontal dimension stacks on top or bottom",
                Axis::Vertical => "a vertical dimension stacks on left or right",
            },
        )
    };
    if let Some(name) = side_attr(attrs) {
        let side = Side::parse(name).ok_or_else(off_axis)?;
        if !valid(side) {
            return Err(off_axis());
        }
        return Ok(side);
    }
    if let Some(side) = pull.filter(|s| valid(*s)) {
        return Ok(side);
    }
    Ok(match axis {
        Axis::Horizontal => Side::Bottom,
        Axis::Vertical => Side::Right,
    })
}

/// Corner anchors both on one edge pull the dim there [SPEC 15.6]:
/// `a:top-left <-> b:top-right` stacks on top.
fn corner_pull(a: &Anchor, b: &Anchor, axis: Axis) -> Option<Side> {
    let edge = |anchor: &Anchor| -> Option<Side> {
        let Spot::Corner((dx, dy)) = anchor.spot else {
            return None;
        };
        Some(match axis {
            Axis::Horizontal => {
                if dy < 0.0 {
                    Side::Top
                } else {
                    Side::Bottom
                }
            }
            Axis::Vertical => {
                if dx < 0.0 {
                    Side::Left
                } else {
                    Side::Right
                }
            }
        })
    };
    match (edge(a), edge(b)) {
        (Some(x), Some(y)) if x == y => Some(x),
        _ => None,
    }
}

/// An explicit `side:` (side **or corner**) as the diametral spill direction.
fn spill_dir(attrs: &crate::resolve::AttrMap, a: &Anchor) -> Option<P> {
    let _ = a;
    side_attr(attrs).and_then(side_unit)
}

/// The round feature's centre, world.
fn centre_of(a: &Anchor) -> P {
    let g = a.geometry_box();
    a.to_world(((g.min_x + g.max_x) / 2.0, (g.min_y + g.max_y) / 2.0))
}

fn side_name(side: Side) -> &'static str {
    match side {
        Side::Top => "top",
        Side::Bottom => "bottom",
        Side::Left => "left",
        Side::Right => "right",
    }
}

/// A directed anchor's axis: the dominant component of its outward normal —
/// left / right → horizontal, top / bottom → vertical, a vertical shoulder →
/// a horizontal dim across it [SPEC 15.6].
fn dominant(outward: P) -> Axis {
    if outward.0.abs() >= outward.1.abs() {
        Axis::Horizontal
    } else {
        Axis::Vertical
    }
}

/// The span projected on the dim's axis, px.
fn span_on(a: P, b: P, axis: Axis) -> f64 {
    match axis {
        Axis::Horizontal => (b.0 - a.0).abs(),
        Axis::Vertical => (b.1 - a.1).abs(),
    }
}

fn scale_p(p: P, k: f64) -> P {
    (p.0 * k, p.1 * k)
}

/// A styled label's `translate:` [SPEC 3].
fn translate_of(t: &ResolvedText) -> Result<Option<(f64, f64)>, Error> {
    match t.attrs.get("translate") {
        None => Ok(None),
        Some(v) => Ok(Some(super::super::as_pair(v, Span::empty())?)),
    }
}

//! Linear dimensions [SPEC 15.6] — `<->` spans and chains — and the shared
//! **stacked-dim anatomy** every span reading lowers through: extension
//! lines springing from the anchors, the dim line on its packed row,
//! drafting-slender arrows (flipped outside a narrow span), ISO-aligned
//! text. Measured values are anchor distances in drawing units — pre-scale,
//! on the unbroken model. The `(-)` readings live in `round`.

use super::super::ir::PlacedNode;
use super::anchors::{self, Anchor, Spot};
use super::annotate::{
    ARROW_HALF, ARROW_LEN, Axis, Ctx, EXT_GAP, EXT_OVERSHOOT, Paint, Rows, side_attr,
};
use super::compose::{self, DimText, Glyph};
use super::geometry::P;
use crate::ast::Side;
use crate::error::Error;
use crate::resolve::{ResolvedLink, ResolvedText};
use crate::span::Span;

/// `a <-> b` (and chains — each hop its own dim) [SPEC 15.6]. A hop's label
/// **replaces** its number; labels map to hops in order. A chain **shares
/// one row**: its hops seat as one unit (their union interval), so a flipped
/// narrow hop's outside arrow abutting its neighbour tip-to-tip at the
/// shared extension line — drafting-normal — never splits the row.
pub(super) fn linear(
    ctx: &Ctx,
    w: &ResolvedLink,
    rows: &mut Rows,
) -> Result<Vec<PlacedNode>, Error> {
    let paint = Paint::of(&w.attrs);
    let mut hops = Vec::new();
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

        // Extension lines land at the displayed anchors; the value reads the
        // unbroken model — a `break:` never changes a dimension [SPEC 15.3].
        let value = span_on(a.model_point(), b.model_point(), axis) / ctx.scale;
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
        hops.push(Stacked {
            axis,
            a: pa,
            b: pb,
            text,
            side,
            gap: w.attrs.number("gap"),
            label,
        });
    }

    let mut out = Vec::new();
    let one_row = hops.len() > 1
        && hops
            .iter()
            .all(|h| h.axis == hops[0].axis && h.side == hops[0].side);
    if one_row {
        let plans: Vec<Plan> = hops.iter().map(|h| plan(h, &paint)).collect();
        let union = plans
            .iter()
            .map(|p| p.interval)
            .reduce(|u, iv| (u.0.min(iv.0), u.1.max(iv.1)))
            .expect("hops non-empty");
        let line_c = rows.seat(hops[0].side, union, hops[0].gap);
        for (h, p) in hops.into_iter().zip(plans) {
            out.extend(at_row(h, &p, line_c, &paint));
        }
    } else {
        for h in hops {
            out.extend(stacked(h, rows, &paint));
        }
    }
    Ok(out)
}

/// One stacked dimension: extension lines springing from the anchors, the
/// dim line on its packed row, slender arrows, ISO-aligned text above the
/// line — flipped outside when the span is too narrow [SPEC 15.6].
pub(super) struct Stacked<'a> {
    pub axis: Axis,
    pub a: P,
    pub b: P,
    pub text: DimText,
    pub side: Side,
    pub gap: Option<f64>,
    /// The authored label, if any — its `translate:` / `rotate:` override the
    /// auto text placement (the styled-label form).
    pub label: Option<&'a ResolvedText>,
}

pub(super) fn stacked(s: Stacked, rows: &mut Rows, paint: &Paint) -> Vec<PlacedNode> {
    let p = plan(&s, paint);
    let line_c = rows.seat(s.side, p.interval, s.gap);
    at_row(s, &p, line_c, paint)
}

/// A dim's row footprint before seating: the packed interval (text included),
/// where the text sits along the line, and whether it fits inside the span.
struct Plan {
    interval: (f64, f64),
    text_u: f64,
    fits: bool,
}

fn plan(s: &Stacked, paint: &Paint) -> Plan {
    let u = |p: P| match s.axis {
        Axis::Horizontal => p.0,
        Axis::Vertical => p.1,
    };
    let (ua, ub) = (u(s.a), u(s.b));
    let (u_lo, u_hi) = (ua.min(ub), ua.max(ub));
    let arrow_len = ARROW_LEN * paint.sw;
    let tw = s.text.width(paint.fs);
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
    Plan {
        interval,
        text_u,
        fits,
    }
}

/// Lower one dim's anatomy onto its seated row.
fn at_row(s: Stacked, p: &Plan, line_c: f64, paint: &Paint) -> Vec<PlacedNode> {
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
    let stub = 2.0;
    let (fits, text_u) = (p.fits, p.text_u);

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
pub(super) fn stack_side(
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
pub(super) fn span_on(a: P, b: P, axis: Axis) -> f64 {
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

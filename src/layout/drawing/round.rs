//! The `(o)` round readings [SPEC 15.6] — unary; the feature picks the
//! reading: a named arc → an `R` leader onto the arc; a `circle()` segment
//! or a bare round node → the `⌀` line across the circle; a round node + side /
//! corner → the **diametral line**; any node + side → the span to the
//! opposite side, ⌀-read; a mirrored `:segment` → the station span across
//! the axis. Span readings lower through `dims::stacked`, leaders through
//! `leaders::measured*`.

use super::super::ir::PlacedNode;
use super::anchors::{self, Anchor, Spot, rotated};
use super::annotate::{Axis, Ctx, Paint, Rows, side_attr, side_unit};
use super::compose::{self, DimText, Glyph};
use super::dims::{Stacked, arrow, dim_clearance, span_on, stack_side, stacked};
use super::geometry::{P, iso_text_angle, reflect_point};
use super::{Segment, leaders};
use crate::ast::Side;
use crate::error::Error;
use crate::ledger::consts::ARROW_LEN;
use crate::resolve::{ResolvedLink, ResolvedText};

/// `(o)` — the round measure, unary; the feature picks the reading
/// [SPEC 15.6].
pub(super) fn lower(
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
                "'(o)' can't pick an axis on '{who}' — anchor a side ('{who}:top (o)') or a segment"
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
        // A `circle()` segment — round by construction, the `⌀` line across it.
        Spot::Segment(Segment::Circle { center, r }) => {
            let text = compose(Glyph::Dia, 2.0 * r / ctx.scale)?;
            let c = a.to_world(*center);
            Ok(leaders::measured_circle(
                ctx, &a, c, *r, &w.attrs, text, &paint, w.span,
            ))
        }
        // A revolved `:segment` — the station's span across the axis, stacked.
        // The reflection happens on the **model** (the axis mirrors the
        // unbroken profile); display maps each end through any break. A merely
        // mirrored profile's span is a width, not a diameter [SPEC 15.6].
        Spot::Segment(Segment::Edge(..) | Segment::Point(..)) => {
            let m = a.unmap_local(a.local_point());
            let axis = a
                .mirrors()
                .iter()
                .find(|ax| {
                    let twin = reflect_point(m, ax.dir());
                    super::geometry::dist(m, twin) > 1e-6
                })
                .copied()
                .ok_or_else(no_axis)?;
            if !a.revolved() {
                return Err(Error::at(
                    w.span,
                    "a station '\u{2300}' reads a revolved profile — 'revolve: x-axis'",
                ));
            }
            let twin = reflect_point(m, axis.dir());
            station(
                ctx,
                w,
                rows,
                &paint,
                span2(&a, a.map_local(m), a.map_local(twin)),
                count,
                follows,
            )
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
            station(ctx, w, rows, &paint, span2(&a, la, lb), count, follows)
        }
        Spot::Corner(diag) => {
            let Some(d) = a.round_diameter() else {
                return Err(no_axis());
            };
            let dir = spill_dir(&w.attrs, &a).unwrap_or_else(|| rotated(*diag, a.rot));
            let text = compose(Glyph::Dia, d / ctx.scale)?;
            Ok(diametral(centre_of(&a), d / 2.0, dir, text, &paint))
        }
        // Bare: a round node reads its ⌀ onto the rim; a revolved sketch its
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
            if !a.revolved() {
                return Err(Error::at(
                    w.span,
                    "a station '\u{2300}' reads a revolved profile — 'revolve: x-axis'",
                ));
            }
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
            station(ctx, w, rows, &paint, span2(&a, la, lb), count, follows)
        }
    }
}

/// A span read off the displayed geometry box: drawn there, valued on the
/// unbroken model [SPEC 15.3].
fn span2(a: &Anchor, la: P, lb: P) -> Span2 {
    Span2 {
        disp: (a.to_world(la), a.to_world(lb)),
        model: (a.model_world(la), a.model_world(lb)),
    }
}

/// A station span's two lives: where it draws, what it measures.
struct Span2 {
    disp: (P, P),
    model: (P, P),
}

/// A ⌀-read span between two world points, stacked like a linear dim — the
/// station and opposite-side readings share it. Drawn at the displayed
/// points; the value reads the model pair [SPEC 15.3].
fn station(
    ctx: &Ctx,
    w: &ResolvedLink,
    rows: &mut Rows,
    paint: &Paint,
    s: Span2,
    count: Option<usize>,
    follows: Option<&ResolvedText>,
) -> Result<Vec<PlacedNode>, Error> {
    let (ma, mb) = s.model;
    let axis = if (mb.1 - ma.1).abs() > (mb.0 - ma.0).abs() {
        Axis::Vertical
    } else {
        Axis::Horizontal
    };
    let text = compose::compose(
        Glyph::Dia,
        span_on(ma, mb, axis) / ctx.scale,
        count,
        None,
        follows.map(|t| t.text.as_str()),
        &w.attrs,
        w.span,
    )?;
    let side = stack_side(&w.attrs, axis, None, w.span)?;
    Ok(stacked(
        Stacked {
            axis,
            a: s.disp.0,
            b: s.disp.1,
            text,
            side,
            clearance: dim_clearance(&w.attrs),
            label: follows,
        },
        rows,
        paint,
    ))
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
    let tw = text.width(fs, paint.font);
    let fits = 2.0 * r >= 2.0 * arrow_len + tw + 8.0;
    // ISO alignment: turn with the line, reading from the bottom / right —
    // a vertical line turns its text −90, like a stacked vertical dim.
    let theta = iso_text_angle(dir);
    let (ts, tc) = theta.to_radians().sin_cos();
    let up = (ts, -tc);
    let lift = fs / 2.0 + 2.0;

    let mut out = Vec::new();
    let (end, text_c) = if fits {
        // Stopped short of both tips, like every dim line.
        let trim = 2.0 * sw;
        ((rim_a.0 - dir.0 * trim, rim_a.1 - dir.1 * trim), c)
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
    let trim = 2.0 * sw;
    out.push(paint.dim(vec![(rim_b.0 + dir.0 * trim, rim_b.1 + dir.1 * trim), end]));
    out.push(arrow(rim_a, dir, paint));
    out.push(arrow(rim_b, (-dir.0, -dir.1), paint));
    out.extend(text.nodes(
        (text_c.0 + up.0 * lift, text_c.1 + up.1 * lift),
        theta,
        fs,
        paint.font,
    ));
    out
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

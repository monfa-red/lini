//! Linear dimensions [SPEC 15.6] — `(-)` spans and chains — and the shared
//! **stacked-dim anatomy** every span reading lowers through: extension
//! lines springing from the anchors, the dim line on its packed row (or, for
//! an **aligned** dim, standing off its own span), drafting-slender arrows
//! (flipped outside a narrow span), ISO-aligned text. Measured values are
//! anchor distances in drawing units — pre-scale, on the unbroken model.
//! The axis is inferred from the anchors (directed normal / true aligned
//! span) with `project:` the override. The `(o)` readings live in `round`.

use super::super::ir::{Bbox, PlacedNode};
use super::anchors::{self, Anchor, Spot};
use super::annotate::{Axis, Ctx, Paint, Rows, SeatLine, side_attr};
use super::compose::{self, DimText, Glyph};
use super::geometry::{P, dist, iso_text_angle, unit};
use super::symbols::CarriedStack;
use crate::ast::Side;
use crate::error::Error;
use crate::ledger::consts::{ARROW_HALF, ARROW_LEN, EXT_GAP, EXT_OVERSHOOT};
use crate::resolve::{AttrMap, ResolvedLink, ResolvedText, ResolvedValue};
use crate::span::Span;

/// Below this, a unit axis component reads as zero — the horizontal /
/// vertical classification and the parallel test [SPEC 15.6].
const AXIS_EPS: f64 = 1e-6;

/// `a (-) b` (and chains — each hop its own dim) [SPEC 15.6]. A hop's label
/// **replaces** its number; labels map to hops in order. A chain **shares
/// one row**: its hops seat as one unit (their union interval), so a flipped
/// narrow hop's outside arrow abutting its neighbour tip-to-tip at the
/// shared extension line — drafting-normal — never splits the row.
pub(super) fn linear(
    ctx: &Ctx,
    w: &ResolvedLink,
    rows: &mut Rows,
    stack: &CarriedStack,
) -> Result<Vec<PlacedNode>, Error> {
    let paint = Paint::of(&w.attrs);
    let mut hops = Vec::new();
    for hop in 0..w.endpoints.len() - 1 {
        let (ea, eb) = (&w.endpoints[hop], &w.endpoints[hop + 1]);
        let a = anchors::resolve(ctx.kids, ctx.scope, ea, "dimension")?;
        let b = anchors::resolve(ctx.kids, ctx.scope, eb, "dimension")?;
        let (pa, pb) = (a.point(), b.point());
        let spell = |ep| anchors::spell(ep, ctx.scope);

        // The axis [SPEC 15.6]: a directed anchor sets it — its outward
        // normal; two directed must be parallel; two points read the true
        // **aligned** span; `project:` overrides the point readings and must
        // agree with a directed anchor.
        let directed = match (a.outward(), b.outward()) {
            (Some(x), Some(y)) => {
                if (x.0 * y.1 - x.1 * y.0).abs() > AXIS_EPS {
                    return Err(Error::at(
                        w.span,
                        format!(
                            "'{} (-) {}' — perpendicular faces have no shared normal; the angle between edges is '(<)'",
                            spell(ea),
                            spell(eb)
                        ),
                    ));
                }
                Some((x, ea))
            }
            (Some(x), None) => Some((x, ea)),
            (None, Some(y)) => Some((y, eb)),
            (None, None) => None,
        };
        let span_dir = {
            let d = (pb.0 - pa.0, pb.1 - pa.1);
            (d.0.hypot(d.1) > AXIS_EPS).then(|| unit(d))
        };
        let project = project_attr(&w.attrs, w.span)?;
        // The measure direction `u` — the dim line runs along it — and
        // whether the value is the true aligned distance (two points, no
        // projection to flatten it).
        let (u, true_aligned) = match (directed, project) {
            (Some((n, ep)), p) => {
                if let Some(p) = p {
                    let pu = p.dir(span_dir).unwrap_or(n);
                    if (n.0 * pu.1 - n.1 * pu.0).abs() > AXIS_EPS {
                        return Err(Error::at(
                            w.span,
                            format!(
                                "'project: {}' conflicts with '{}' — the directed anchor reads {}",
                                p.name(),
                                spell(ep),
                                reading(n)
                            ),
                        ));
                    }
                }
                (n, false)
            }
            (None, Some(p)) => match p.dir(span_dir) {
                Some(d) => (d, p == Project::Aligned),
                None => ((1.0, 0.0), false),
            },
            (None, None) => match span_dir {
                Some(d) => (d, true),
                None => ((1.0, 0.0), false),
            },
        };

        // Extension lines land at the displayed anchors; the value reads the
        // unbroken model — a `break:` never changes a dimension [SPEC 15.3].
        let (am, bm) = (a.model_point(), b.model_point());
        let value = if true_aligned {
            dist(am, bm)
        } else {
            ((bm.0 - am.0) * u.0 + (bm.1 - am.1) * u.1).abs()
        } / ctx.scale;
        let label = w.texts.get(hop);
        let text = compose::compose(
            Glyph::None,
            value,
            None,
            label.map(|t| t.text.as_str()),
            None,
            &w.attrs,
            w.span,
        )?;
        let clearance = dim_clearance(&w.attrs);
        // Seat: horizontal / vertical dims pack into side rows; an aligned
        // dim packs along its own span's normal, on the side facing away
        // from the geometry centre — the extent's bbox centre [SPEC 15.6].
        let (frame, seat) = match axis_of(u) {
            Some(axis) => {
                let side = stack_side(&w.attrs, axis, corner_pull(&a, &b, axis), w.span)?;
                (Frame::axis(axis), Seat::Row(side))
            }
            None => {
                let frame = Frame::of(u);
                let away_pos = aligned_away(
                    &w.attrs,
                    span_dir.unwrap_or(u),
                    ((pa.0 + pb.0) / 2.0, (pa.1 + pb.1) / 2.0),
                    (
                        (ctx.extent.min_x + ctx.extent.max_x) / 2.0,
                        (ctx.extent.min_y + ctx.extent.max_y) / 2.0,
                    ),
                    frame.n,
                    w.span,
                )?;
                (frame, Seat::Aligned(away_pos))
            }
        };
        hops.push(Stacked {
            frame,
            a: pa,
            b: pb,
            text,
            seat,
            clearance,
            label,
        });
    }

    let mut out = Vec::new();
    let row_key = |h: &Stacked| match h.seat {
        Seat::Row(side) => Some((side, h.frame)),
        Seat::Aligned(_) => None,
    };
    let one_row = hops.len() > 1
        && row_key(&hops[0]).is_some()
        && hops.iter().all(|h| row_key(h) == row_key(&hops[0]));
    if one_row {
        let plans: Vec<Plan> = hops.iter().map(|h| plan(h, &paint)).collect();
        let union = plans
            .iter()
            .map(|p| p.interval)
            .reduce(|u, iv| (u.0.min(iv.0), u.1.max(iv.1)))
            .expect("hops non-empty");
        let Seat::Row(side) = hops[0].seat else {
            unreachable!("one_row is row-seated");
        };
        let carried = carried_band(hops.iter().zip(&plans), &paint, stack);
        let line_c = rows.seat(
            rows.side_line(side),
            union,
            hops[0].clearance,
            &paint,
            carried,
        );
        for (h, p) in hops.into_iter().zip(plans) {
            out.extend(at_row(h, &p, line_c, &paint));
        }
    } else {
        for h in hops {
            out.extend(stacked(h, rows, &paint, stack));
        }
    }
    Ok(out)
}

/// The dim's measure frame [SPEC 15.6]: `u` runs along the dim line (the
/// measure direction), `n` across it — oriented so **−n is the ISO reading's
/// "above the line"** (text lifts toward −n, whatever the angle).
#[derive(Clone, Copy, PartialEq)]
pub(super) struct Frame {
    pub u: P,
    pub n: P,
}

impl Frame {
    /// The two row axes — exact unit vectors, so the packed paths stay
    /// byte-identical to the axis-matched arithmetic.
    pub fn axis(axis: Axis) -> Frame {
        match axis {
            Axis::Horizontal => Frame {
                u: (1.0, 0.0),
                n: (0.0, 1.0),
            },
            Axis::Vertical => Frame {
                u: (0.0, 1.0),
                n: (1.0, 0.0),
            },
        }
    }

    /// An aligned frame along `dir` — folded to the ISO reading direction, so
    /// the text and its "above" side are order-independent.
    pub fn of(dir: P) -> Frame {
        let theta = iso_text_angle(dir);
        let (s, c) = theta.to_radians().sin_cos();
        Frame {
            u: (c, s),
            n: (-s, c),
        }
    }

    /// The coordinate along the dim line.
    pub(super) fn u(&self, p: P) -> f64 {
        p.0 * self.u.0 + p.1 * self.u.1
    }

    /// The coordinate across the dim line.
    pub(super) fn cross(&self, p: P) -> f64 {
        p.0 * self.n.0 + p.1 * self.n.1
    }

    /// Frame coordinates back to the drawing plane.
    pub(super) fn pt(&self, u: f64, c: f64) -> P {
        (u * self.u.0 + c * self.n.0, u * self.u.1 + c * self.n.1)
    }
}

/// Where a dim's line seats [SPEC 15.6]: a packed side row (horizontal /
/// vertical), or — aligned — packed along its own span's normal, on the
/// away side (`true` = the frame's +n side). Both route through the one
/// row packer.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Seat {
    Row(Side),
    Aligned(bool),
}

/// One stacked dimension: extension lines springing from the anchors, the
/// dim line on its seat, slender arrows, ISO-aligned text above the line —
/// flipped outside when the span is too narrow [SPEC 15.6].
pub(super) struct Stacked<'a> {
    pub frame: Frame,
    pub a: P,
    pub b: P,
    pub text: DimText,
    pub seat: Seat,
    /// The dim's resolved stand-off minimum [SPEC 15.6] — the cascade's value
    /// (drawing default → `(-)` rule → class → the dim's block).
    pub clearance: f64,
    /// The authored label, if any — its `translate:` / `rotate:` override the
    /// auto text placement (the styled-label form).
    pub label: Option<&'a ResolvedText>,
}

pub(super) fn stacked(
    s: Stacked,
    rows: &mut Rows,
    paint: &Paint,
    stack: &CarriedStack,
) -> Vec<PlacedNode> {
    let p = plan(&s, paint);
    let carried = carried_band(std::iter::once((&s, &p)), paint, stack);
    let at = match s.seat {
        Seat::Row(side) => rows.side_line(side),
        Seat::Aligned(away_pos) => SeatLine::span(s.frame, away_pos, (s.a, s.b)),
    };
    let line_c = rows.seat(at, p.interval, s.clearance, paint, carried);
    at_row(s, &p, line_c, paint)
}

/// A carrying statement's painted box relative to its row line [SPEC 15.9]:
/// the value texts probed at a **zero line** (the seat's offset from the line
/// is constant), the stack's one measured box hung below them — what
/// `Rows::seat` folds into the band, and exactly where `CarriedStack::seat`
/// will put the stack once the row is real.
fn carried_band<'a>(
    hops: impl Iterator<Item = (&'a Stacked<'a>, &'a Plan)>,
    paint: &Paint,
    stack: &CarriedStack,
) -> Option<Bbox> {
    if stack.is_empty() {
        return None;
    }
    let probe: Vec<PlacedNode> = hops
        .flat_map(|(s, p)| value_texts(s, p, 0.0, paint))
        .collect();
    let seat = super::symbols::seat_of(&probe);
    Some(stack.box_below(seat)?.union(seat))
}

/// A dimension's resolved `clearance` [SPEC 15.6]: the ordinary cascade — the
/// drawing scope's base default rides the link attrs, so the fallback here
/// only restates it.
pub(super) fn dim_clearance(attrs: &AttrMap) -> f64 {
    attrs
        .number("clearance")
        .unwrap_or(crate::ledger::consts::DIM_CLEARANCE)
}

/// The `project:` override [SPEC 15.6].
#[derive(Clone, Copy, PartialEq)]
enum Project {
    Horizontal,
    Vertical,
    Aligned,
}

impl Project {
    /// The projected measure direction; `aligned` needs a span to align to.
    fn dir(self, span_dir: Option<P>) -> Option<P> {
        match self {
            Project::Horizontal => Some((1.0, 0.0)),
            Project::Vertical => Some((0.0, 1.0)),
            Project::Aligned => span_dir,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Project::Horizontal => "horizontal",
            Project::Vertical => "vertical",
            Project::Aligned => "aligned",
        }
    }
}

fn project_attr(attrs: &AttrMap, span: Span) -> Result<Option<Project>, Error> {
    match attrs.get("project") {
        None => Ok(None),
        Some(ResolvedValue::Ident(s)) if s == "horizontal" => Ok(Some(Project::Horizontal)),
        Some(ResolvedValue::Ident(s)) if s == "vertical" => Ok(Some(Project::Vertical)),
        Some(ResolvedValue::Ident(s)) if s == "aligned" => Ok(Some(Project::Aligned)),
        Some(_) => Err(Error::at(
            span,
            "'project' takes horizontal, vertical, or aligned",
        )),
    }
}

/// A directed anchor's reading, for the `project:` conflict message.
fn reading(n: P) -> &'static str {
    match axis_of(n) {
        Some(Axis::Horizontal) => "horizontal",
        Some(Axis::Vertical) => "vertical",
        None => "along its face's normal",
    }
}

/// A unit measure direction's row axis — `None` is the aligned case.
fn axis_of(u: P) -> Option<Axis> {
    if u.1.abs() < AXIS_EPS {
        Some(Axis::Horizontal)
    } else if u.0.abs() < AXIS_EPS {
        Some(Axis::Vertical)
    } else {
        None
    }
}

/// An aligned dim's stand-off side [SPEC 15.6]: `side: left | right` read
/// along the span, first anchor → second (the walker's left); the default
/// faces **away from the geometry centre**. Returns whether the dim line
/// sits on the frame's +n side.
fn aligned_away(
    attrs: &AttrMap,
    span_dir: P,
    mid: P,
    centre: P,
    n: P,
    span: Span,
) -> Result<bool, Error> {
    if let Some(name) = side_attr(attrs) {
        // Walker's left with y down: facing along `d`, left is (d.1, -d.0).
        let dir = match name {
            "left" => (span_dir.1, -span_dir.0),
            "right" => (-span_dir.1, span_dir.0),
            _ => {
                return Err(Error::at(
                    span,
                    "an aligned dimension sits left or right of its span — read along it, first anchor to second",
                ));
            }
        };
        return Ok(dir.0 * n.0 + dir.1 * n.1 > 0.0);
    }
    let v = (mid.0 - centre.0, mid.1 - centre.1);
    // A tie (a right triangle's hypotenuse runs exactly through its bbox
    // centre) falls to the ISO-above side (−n) — outside the common taper.
    Ok(v.0 * n.0 + v.1 * n.1 > 1e-9)
}

/// A dim's row footprint before seating: the packed interval (text included),
/// where the text sits along the line, and whether it fits inside the span.
struct Plan {
    interval: (f64, f64),
    text_u: f64,
    fits: bool,
}

fn plan(s: &Stacked, paint: &Paint) -> Plan {
    let (ua, ub) = (s.frame.u(s.a), s.frame.u(s.b));
    let (u_lo, u_hi) = (ua.min(ub), ua.max(ub));
    let arrow_len = ARROW_LEN * paint.sw;
    let tw = s.text.width(paint.fs, paint.font);
    let stub = 2.0;
    let span = u_hi - u_lo;
    let fits = span >= 2.0 * arrow_len + tw + 6.0;
    // A narrow span flips its arrows outside the extension lines; the value
    // stays **inside** while it still reads there (drafting's middle form —
    // chained narrow hops keep their numbers apart), and only a span too
    // tight even for the bare text slides it past the nearer line.
    let reach = arrow_len + stub;
    let (interval, text_u) = if fits {
        ((u_lo, u_hi), (u_lo + u_hi) / 2.0)
    } else if span >= tw + 4.0 {
        ((u_lo - reach, u_hi + reach), (u_lo + u_hi) / 2.0)
    } else if s.frame.u.0.abs() >= s.frame.u.1.abs() {
        (
            (u_lo - reach, u_hi + reach + 4.0 + tw),
            u_hi + reach + 4.0 + tw / 2.0,
        )
    } else {
        (
            (u_lo - reach - 4.0 - tw, u_hi + reach),
            u_lo - reach - 4.0 - tw / 2.0,
        )
    };
    Plan {
        interval,
        text_u,
        fits,
    }
}

/// Lower one dim's anatomy onto its seated line.
fn at_row(s: Stacked, p: &Plan, line_c: f64, paint: &Paint) -> Vec<PlacedNode> {
    let sw = paint.sw;
    let f = s.frame;
    let (ua, ub) = (f.u(s.a), f.u(s.b));
    let (u_lo, u_hi) = (ua.min(ub), ua.max(ub));
    let arrow_len = ARROW_LEN * sw;
    let stub = 2.0;
    let fits = p.fits;

    let mut out = Vec::new();
    // Extension lines spring from the anchor points exactly [SPEC 15.2],
    // with a small gap, and overshoot past the dim line.
    for p in [s.a, s.b] {
        let toward = (line_c - f.cross(p)).signum();
        let c0 = f.cross(p) + EXT_GAP * toward;
        let c1 = line_c + EXT_OVERSHOOT * toward;
        if (c1 - c0) * toward > 0.0 {
            out.push(paint.ext(vec![f.pt(f.u(p), c0), f.pt(f.u(p), c1)]));
        }
    }
    // The dim line — stopped short of the arrow tips (a butt-capped stroke
    // ending exactly at the tip blunts it, the same fix links carry); it runs
    // past the span when the arrows flip outside.
    let trim = 2.0 * sw;
    let (l0, l1) = if fits {
        (u_lo + trim, u_hi - trim)
    } else {
        (u_lo - arrow_len - stub, u_hi + arrow_len + stub)
    };
    out.push(paint.dim(vec![f.pt(l0, line_c), f.pt(l1, line_c)]));
    // Slender arrows: tips on the extension lines; bodies inside the span,
    // or outside pointing in when flipped.
    let along = f.u;
    let flip = if fits { -1.0 } else { 1.0 };
    out.push(arrow(f.pt(u_lo, line_c), scale_p(along, flip), paint));
    out.push(arrow(f.pt(u_hi, line_c), scale_p(along, -flip), paint));
    out.extend(value_texts(&s, p, line_c, paint));
    out
}

/// The dim's ISO-aligned value texts above the line [SPEC 15.6]: turned with
/// the line, reading from the bottom / right (the frame's −n is that
/// "above"), the authored label's `translate:` / `rotate:` overriding. The
/// one placement the row's carried-band probe (at a zero line) and the
/// lowered row share.
fn value_texts(s: &Stacked, p: &Plan, line_c: f64, paint: &Paint) -> Vec<PlacedNode> {
    let lift = paint.fs / 2.0 + 2.0;
    let mut centre = s.frame.pt(p.text_u, line_c - lift);
    let mut rot = iso_text_angle(s.frame.u);
    if let Some(t) = s.label {
        if let Some(r) = t.attrs.number("rotate") {
            rot = r;
        }
        if let Ok(Some((dx, dy))) = super::super::anchors::translate(&t.attrs, Span::empty()) {
            centre = (centre.0 + dx, centre.1 + dy);
        }
    }
    s.text.nodes(centre, rot, paint.fs, paint.font)
}

/// The drafting-slender arrowhead [SPEC 15.6]: ≈ 3 : 1, filled with the dim's
/// stroke and sized by its stroke-width; `dir` is where the tip points.
pub(super) fn arrow(tip: P, dir: P, paint: &Paint) -> PlacedNode {
    let (l, w) = (ARROW_LEN * paint.sw, ARROW_HALF * paint.sw);
    let base = (tip.0 - dir.0 * l, tip.1 - dir.1 * l);
    let perp = (-dir.1, dir.0);
    super::super::prim::dim_marker(
        "dim",
        vec![
            tip,
            (base.0 + perp.0 * w, base.1 + perp.1 * w),
            (base.0 - perp.0 * w, base.1 - perp.1 * w),
        ],
        paint.stroke.clone(),
    )
}

/// The stacking side [SPEC 15.6]: explicit `side:` (validated against the
/// axis), a corner pull, or the axis default — bottom / right.
pub(super) fn stack_side(
    attrs: &AttrMap,
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
/// `a:top-left (-) b:top-right` stacks on top.
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

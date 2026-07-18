//! Leaders & annotation arrows [SPEC 15.7]. A **callout** is a one-ended
//! link written tip-first: the tip ray-casts onto the drawn outline, the
//! text auto-places outward past the geometry (or where `side:` points) with
//! a horizontal **landing** elbow before it. The `(o)` readings that leader
//! (`R`, bare `⌀`) share the same line, tipped with the slender dim arrow.
//! Any other two-ended op draws a **straight annotation line**, markers per
//! the op, ends trimmed to the outlines they spring from.

use super::super::ir::PlacedNode;
use super::super::{approx_height, approx_width, prim};
use super::anchors::{self, Anchor, Spot, rotated};
use super::annotate::{Ctx, Paint, side_attr, side_unit};
use super::compose::DimText;
use super::geometry::{P, dist, unit};
use super::{dims, outline};
use crate::error::Error;
use crate::ledger::consts::{ARROW_LEN, NOTE_LANDING, NOTE_OFFSET};
use crate::resolve::{MarkerKind, ResolvedLink, ResolvedText, ResolvedValue};
use crate::span::Span;

/// A leader's drawn skeleton: tip → elbow → landing, plus where its text
/// starts (just past the landing, on the `sx` side).
struct LeaderLine {
    points: Vec<P>,
    text_at: P,
    sx: f64,
}

/// Build the leader skeleton toward `aim` (world). The text direction is
/// `side:`'s; else a **directed** feature's surface normal (the leader
/// leaves a face straight off it, then the elbow — the drafting default); a
/// point feature's is the ray from the drawing's **datum** through it. The
/// text clears the geometry union by `NOTE_OFFSET` [SPEC 15.7]. The tip:
/// `exact` lands as given (an arc's own point); `circle` intersects
/// analytically; otherwise the ray casts onto the node's drawn outline.
fn leader_line(
    ctx: &Ctx,
    anchor: &Anchor,
    aim: P,
    dir_override: Option<P>,
    exact: Option<P>,
    circle: Option<(P, f64)>,
) -> LeaderLine {
    let u = dir_override
        .or_else(|| {
            // The normal's axis comes from the surface; its sign points away
            // from the datum — an edge authored material-on-the-left reports
            // `outward` into the part.
            let n = anchor.outward()?;
            Some(if n.0 * aim.0 + n.1 * aim.1 < 0.0 {
                (-n.0, -n.1)
            } else {
                n
            })
        })
        .unwrap_or_else(|| {
            let len = dist(aim, (0.0, 0.0));
            if len > 1e-6 {
                (aim.0 / len, aim.1 / len)
            } else {
                // A feature on the datum has no outward ray — drafting's
                // default leader runs up-right.
                let d = std::f64::consts::FRAC_1_SQRT_2;
                (d, -d)
            }
        });
    let t_exit = outline::exit_box(aim, u, ctx.extent);
    let elbow = (
        aim.0 + u.0 * (t_exit + NOTE_OFFSET),
        aim.1 + u.1 * (t_exit + NOTE_OFFSET),
    );
    let sx = if u.0 < 0.0 { -1.0 } else { 1.0 };
    let landing = (elbow.0 + sx * NOTE_LANDING, elbow.1);
    let tip = exact
        .or_else(|| circle_tip(circle, elbow))
        .unwrap_or_else(|| {
            let d = unit((aim.0 - elbow.0, aim.1 - elbow.1));
            let o = anchor.to_local(elbow);
            match outline::raycast(anchor.node, o, rotated(d, -anchor.rot)) {
                Some(t) => (elbow.0 + d.0 * t, elbow.1 + d.1 * t),
                None => aim,
            }
        });
    LeaderLine {
        points: vec![tip, elbow, landing],
        text_at: (landing.0 + sx * 2.0, landing.1),
        sx,
    }
}

/// The nearest rim point of an analytic circle toward the elbow.
fn circle_tip(circle: Option<(P, f64)>, from: P) -> Option<P> {
    let (c, r) = circle?;
    let u = unit((from.0 - c.0, from.1 - c.1));
    Some((c.0 + u.0 * r, c.1 + u.1 * r))
}

/// A measured `(o)` leader (an `R` onto its arc, a `⌀` onto a rim): the
/// leader line tipped with the slender dim arrow, the composed text past the
/// landing [SPEC 15.6].
#[allow(clippy::too_many_arguments)]
pub(super) fn measured(
    ctx: &Ctx,
    a: &Anchor,
    aim: P,
    exact: Option<P>,
    attrs: &crate::resolve::AttrMap,
    text: DimText,
    paint: &Paint,
    _span: Span,
) -> Vec<PlacedNode> {
    lower_measured(ctx, a, aim, exact, None, attrs, text, paint)
}

/// A measured `⌀` leader onto an analytic circle's rim.
#[allow(clippy::too_many_arguments)]
pub(super) fn measured_circle(
    ctx: &Ctx,
    a: &Anchor,
    c: P,
    r: f64,
    attrs: &crate::resolve::AttrMap,
    text: DimText,
    paint: &Paint,
    _span: Span,
) -> Vec<PlacedNode> {
    lower_measured(ctx, a, c, None, Some((c, r)), attrs, text, paint)
}

#[allow(clippy::too_many_arguments)]
fn lower_measured(
    ctx: &Ctx,
    a: &Anchor,
    aim: P,
    exact: Option<P>,
    circle: Option<(P, f64)>,
    attrs: &crate::resolve::AttrMap,
    text: DimText,
    paint: &Paint,
) -> Vec<PlacedNode> {
    let dir = side_attr(attrs).and_then(side_unit);
    let mut line = leader_line(ctx, a, aim, dir, exact, circle);
    let tip = line.points[0];
    let elbow = line.points[1];
    let to_tip = unit((tip.0 - elbow.0, tip.1 - elbow.1));
    let mut out = vec![dims::arrow(tip, to_tip, paint)];
    // A circle's ⌀ line runs along the diameter [SPEC 15.6]: through the
    // centre to the far rim, overshooting it, both arrowheads pressing the
    // rims inward from outside — never mistakable for a word leader.
    if let Some((c, _)) = circle {
        let far = (2.0 * c.0 - tip.0, 2.0 * c.1 - tip.1);
        let over = ARROW_LEN * paint.sw + 2.0;
        line.points[0] = (far.0 + to_tip.0 * over, far.1 + to_tip.1 * over);
        out.push(dims::arrow(far, (-to_tip.0, -to_tip.1), paint));
    } else {
        // The line stops short of the arrow tip, like every dim line.
        let trim = 2.0 * paint.sw;
        line.points[0] = (tip.0 - to_tip.0 * trim, tip.1 - to_tip.1 * trim);
    }
    out.insert(0, paint.dim(line.points.clone()));
    let tw = text.width(paint.fs, paint.font);
    let centre = (line.text_at.0 + line.sx * tw / 2.0, line.text_at.1);
    out.extend(text.nodes(centre, 0.0, paint.fs, paint.font));
    out
}

/// A one-ended callout [SPEC 15.7]: `<-` arrow · `*-` dot · `>-` the datum
/// triangle (the scope reinterprets the crow op); the text is the link's
/// label, lowered to bare leaves — or, for `>-`, seated in the standard
/// framed datum box. A `&` fan keeps one text and landing (the first
/// endpoint steers, `side:` overrides) with an independent leg from the
/// shared elbow to each further feature.
pub(super) fn callout(ctx: &Ctx, w: &ResolvedLink) -> Result<Vec<PlacedNode>, Error> {
    let paint = Paint::of(&w.attrs);
    let a = anchors::resolve(ctx.kids, ctx.scope, &w.endpoints[0], "leader")?;
    // A callout on a threaded segment composes its spec — `M⌀×pitch`, the
    // numbers living once [SPEC 15.7]. A bare `<-` reads the spec alone; an
    // authored label **follows** it, per the one-ended label law [SPEC 15.6]
    // (`bar:m20 <- "LH"` reads `M20×1.5 LH`). A `>-` letter is a datum
    // identity, never a label — it composes nothing. Anything unthreaded
    // still needs its word.
    let spec = (w.markers.start != MarkerKind::Crow)
        .then(|| thread_spec(&a, &w.endpoints[0]))
        .flatten();
    let composed = match (&spec, w.texts.is_empty()) {
        (_, false) => None,
        (Some(text), true) => Some(ResolvedText {
            text: text.clone(),
            along: crate::resolve::Along::Auto,
            attrs: crate::resolve::AttrMap::default(),
            applied_styles: Vec::new(),
        }),
        (None, true) => {
            return Err(Error::at(
                w.span,
                "a leader needs its text — 'bolt <- \"THRU\"'",
            ));
        }
    };
    // The follows merge: the spec leads, the authored words trail on the
    // first line; further lines stack unchanged.
    let followed: Vec<ResolvedText>;
    let texts: &[ResolvedText] = match (&composed, &spec) {
        (Some(t), _) => std::slice::from_ref(t),
        (None, Some(spec)) => {
            followed = w
                .texts
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let mut t = t.clone();
                    if i == 0 {
                        t.text = format!("{spec} {}", t.text);
                    }
                    t
                })
                .collect();
            &followed
        }
        (None, None) => &w.texts,
    };
    let dir = side_attr(&w.attrs).and_then(side_unit);
    let line = leader_line(ctx, &a, a.point(), dir, None, None);
    let elbow = line.points[1];
    let landing = line.points[2];

    let mut out = leg(&a, w.markers.start, line.points.clone(), &paint);
    // Fan legs [SPEC 15.7]: each further endpoint casts its own ray from the
    // shared elbow — the trunk (elbow → landing → text) is shared, nothing
    // else; an unroutable leg is an error, never a silent drop.
    for ep in &w.endpoints[1..] {
        let b = anchors::resolve(ctx.kids, ctx.scope, ep, "leader")?;
        let tip = fan_tip(ctx, &b, elbow, w.markers.start, ep)?;
        out.extend(leg(&b, w.markers.start, vec![tip, elbow], &paint));
    }
    if w.markers.start == MarkerKind::Crow {
        out.extend(datum_box(texts, landing, line.sx, &paint));
    } else {
        out.extend(texts_beside(texts, line.text_at, line.sx, paint.fs));
    }
    Ok(out)
}

/// One leader leg's linework and tip, `points[0]` the tip and `points[1]`
/// the vertex it leaves toward — shared by the steering leg (tip → elbow →
/// landing) and a fan's extra legs (tip → elbow) [SPEC 15.7].
///
/// `>-` is the crow op elsewhere — on a drawing's datum leader it lowers to
/// the filled GD&T triangle. On a directed feature the triangle **seats on
/// the surface**: base flush with the drawn edge, apex out along the surface
/// normal — the leg meets the apex at whatever angle it arrives, never
/// tilting the symbol.
fn leg(a: &Anchor, marker: MarkerKind, mut points: Vec<P>, paint: &Paint) -> Vec<PlacedNode> {
    let mut out = Vec::new();
    if marker == MarkerKind::Crow
        && let Some(n) = a.outward()
    {
        let tip = points[0];
        // The surface sets the triangle's axis; the leg sets its sign — the
        // apex meets the leg, which approaches from outside the material (an
        // edge authored the other way round flips `outward`).
        let to_elbow = (points[1].0 - tip.0, points[1].1 - tip.1);
        let n = if n.0 * to_elbow.0 + n.1 * to_elbow.1 < 0.0 {
            (-n.0, -n.1)
        } else {
            n
        };
        let size = crate::render::markers::datum_size(paint.sw);
        let half = size * 0.5;
        let t = (-n.1, n.0);
        let apex = (tip.0 + n.0 * size, tip.1 + n.1 * size);
        points[0] = apex;
        out.push(paint.dim(points));
        out.push(prim::dim_marker(
            "datum",
            vec![
                (tip.0 + t.0 * half, tip.1 + t.1 * half),
                (tip.0 - t.0 * half, tip.1 - t.1 * half),
                apex,
            ],
            paint.stroke.clone(),
        ));
    } else if marker == MarkerKind::Arrow {
        // ISO 129: one arrowhead style per sheet — a word leader tips with
        // the same slender arrow as every dimension [SPEC 15.7].
        let tip = points[0];
        let to_tip = unit((tip.0 - points[1].0, tip.1 - points[1].1));
        let trim = 2.0 * paint.sw;
        points[0] = (tip.0 - to_tip.0 * trim, tip.1 - to_tip.1 * trim);
        out.push(paint.dim(points));
        out.push(dims::arrow(tip, to_tip, paint));
    } else {
        let mut node = paint.dim(points);
        node.markers.start = match marker {
            // A point-anchored datum has no surface normal — the core marker
            // orients along the leg, today's fallback.
            MarkerKind::Crow => MarkerKind::Datum,
            m => m,
        };
        out.push(node);
    }
    out
}

/// A fan leg's landing point [SPEC 15.7]: an independent ray from the shared
/// elbow toward the endpoint's feature. A `*-` dot lands **within** the face
/// — the anchor point itself; the outline tips cast onto the drawn outline.
/// A leg that cannot land is an **error**, never a silent drop: a degenerate
/// ray, a default anchor whose outline the ray misses, or an explicitly
/// anchored point occluded by its own outline (the near face struck first).
fn fan_tip(
    ctx: &Ctx,
    b: &Anchor,
    elbow: P,
    marker: MarkerKind,
    ep: &crate::resolve::ResolvedEndpoint,
) -> Result<P, Error> {
    let name = anchors::spell(ep, ctx.scope);
    let aim = b.point();
    let full = dist(elbow, aim);
    if full < 1e-6 {
        return Err(Error::at(
            ep.span,
            format!("a fan leg onto '{name}' cannot land — it coincides with the fan's landing"),
        ));
    }
    if marker == MarkerKind::Dot {
        return Ok(aim);
    }
    let d = ((aim.0 - elbow.0) / full, (aim.1 - elbow.1) / full);
    let o = b.to_local(elbow);
    let explicit = !matches!(b.spot, Spot::Origin);
    match outline::raycast(b.node, o, rotated(d, -b.rot)) {
        // A default anchor lands on the first outline the ray strikes; an
        // explicit one must be reachable — a nearer strike means the anchored
        // face turns away from the shared landing.
        Some(t) if !explicit || t >= full - 1.0 => Ok((elbow.0 + d.0 * t, elbow.1 + d.1 * t)),
        Some(_) => Err(Error::at(
            ep.span,
            format!(
                "a fan leg cannot reach '{name}' — its own outline blocks the way; anchor the near face or steer the fan with 'side:'"
            ),
        )),
        None if explicit => Ok(aim),
        None => Err(Error::at(
            ep.span,
            format!(
                "a fan leg onto '{name}' finds no outline to land on — anchor a point or a side"
            ),
        )),
    }
}

/// The standard framed datum box [SPEC 15.7]: the letter seated in a square
/// frame riding the leader's text seat at the landing, linework in the
/// dimension stroke. The frame is classed `datum-frame` so the row packer
/// registers the box itself as painted bounds (`Rows::obstruct_texts`) — a
/// dim row stands off the frame, not just the letter inside it.
fn datum_box(texts: &[ResolvedText], landing: P, sx: f64, paint: &Paint) -> Vec<PlacedNode> {
    let letter = &texts[0];
    let size = letter.attrs.number("font-size").unwrap_or(paint.fs);
    let font = crate::font::Font::of(&letter.attrs);
    let tw = approx_width(&letter.text, font, size, 0.0);
    let h = size + 6.0;
    let w = h.max(tw + 6.0);
    // The letter centres in the frame, whose near edge meets the landing;
    // its own `translate` nudge carries the frame along.
    let mut out = texts_beside(
        std::slice::from_ref(letter),
        (landing.0 + sx * (w - tw) / 2.0, landing.1),
        sx,
        paint.fs,
    );
    let c = (out[0].cx, out[0].cy);
    let (x0, x1) = (c.0 - w / 2.0, c.0 + w / 2.0);
    let (y0, y1) = (c.1 - h / 2.0, c.1 + h / 2.0);
    let mut frame = paint.dim(vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)]);
    frame.type_chain.push("datum-frame".into());
    out.push(frame);
    // Any further authored lines stack below the box, the usual leaf rules.
    if texts.len() > 1 {
        out.extend(texts_beside(
            &texts[1..],
            (landing.0, y1 + 3.0 + paint.fs / 2.0),
            sx,
            paint.fs,
        ));
    }
    out
}

/// The `M⌀×pitch` spec of a threaded segment [SPEC 15.7]: the dressing
/// composed the numbers once where it drew the thin lines — external reads
/// the run's own `⌀`, internal the major out from the drilled minor
/// [SPEC 15.3] — re-cut the bar and the callout follows.
fn thread_spec(a: &Anchor, ep: &crate::resolve::ResolvedEndpoint) -> Option<String> {
    let name = ep.point.as_ref()?;
    let geo = a.node.sketch.as_ref()?;
    let spec = geo.threads.iter().find(|t| &t.name == name)?;
    Some(format!(
        "M{}×{}",
        super::compose::fmt(spec.major),
        super::compose::fmt(spec.pitch)
    ))
}

/// Any other two-ended op — a straight annotation line between two nodes,
/// markers per the op [SPEC 15.7]. A default-anchored end springs from the
/// node's outline (a balloon's rim), except a dot's, which lands **within**
/// it (`-*` — a face, a region); explicit anchors are honoured exactly.
pub(super) fn arrows(ctx: &Ctx, w: &ResolvedLink) -> Result<Vec<PlacedNode>, Error> {
    let paint = Paint::of(&w.attrs);
    let mut out = Vec::new();
    for hop in 0..w.endpoints.len() - 1 {
        let a = anchors::resolve(ctx.kids, ctx.scope, &w.endpoints[hop], "leader")?;
        let b = anchors::resolve(ctx.kids, ctx.scope, &w.endpoints[hop + 1], "leader")?;
        let (mut pa, mut pb) = (a.point(), b.point());
        let full = dist(pa, pb);
        pa = trim(&a, pa, pb, w.markers.start, full);
        pb = trim(&b, pb, pa, w.markers.end, full);
        let mut node = paint.dim(vec![pa, pb]);
        node.markers = w.markers.clone();
        if let Some(style) = match w.line {
            crate::ast::LineStyle::Solid => None,
            crate::ast::LineStyle::Dashed => Some("dashed"),
            crate::ast::LineStyle::Dotted => Some("dotted"),
            crate::ast::LineStyle::Wavy => Some("wavy"),
        } {
            node.attrs
                .insert("stroke-style", ResolvedValue::Ident(style.into()));
        }
        out.push(node);
        if hop == 0 && !w.texts.is_empty() {
            let mid = ((pa.0 + pb.0) / 2.0, (pa.1 + pb.1) / 2.0);
            let at = (mid.0, mid.1 - (paint.fs / 2.0 + 3.0));
            out.extend(texts_beside(&w.texts, at, 0.0, paint.fs));
        }
    }
    Ok(out)
}

/// Pull a default-anchored end out to its node's outline, toward the other
/// end — a line springs from the rim, not the centre.
fn trim(anchor: &Anchor, p: P, other: P, marker: MarkerKind, full: f64) -> P {
    if !matches!(anchor.spot, Spot::Origin) || marker == MarkerKind::Dot {
        return p;
    }
    let d = unit((other.0 - p.0, other.1 - p.1));
    let o = anchor.to_local(p);
    match outline::raycast(anchor.node, o, rotated(d, -anchor.rot)) {
        Some(t) if t < full => (p.0 + d.0 * t, p.1 + d.1 * t),
        _ => p,
    }
}

/// Callout texts, lowered to bare leaves beside the landing [SPEC 15.7]:
/// stacked lines, each styleable (`translate` nudges, `rotate` turns, text
/// props ride along). `sx` anchors the run leftward or rightward of the
/// point (`0` centres it — an annotation arrow's label).
fn texts_beside(texts: &[ResolvedText], at: P, sx: f64, fs: f64) -> Vec<PlacedNode> {
    let mut out = Vec::new();
    let mut y = at.1;
    for t in texts {
        let size = t.attrs.number("font-size").unwrap_or(fs);
        let font = crate::font::Font::of(&t.attrs);
        let w = approx_width(&t.text, font, size, 0.0);
        let mut n = prim::dim_text(&t.text, at.0 + sx * w / 2.0, y, size, font.kind);
        for (k, v) in &t.attrs.map {
            match k.as_str() {
                "translate" => {
                    if let Ok((dx, dy)) = super::super::as_pair(v, Span::empty()) {
                        n.cx += dx;
                        n.cy += dy;
                    }
                }
                "rotate" => {
                    if let Some(r) = v.as_number() {
                        n.rotation = r;
                        n.attrs.insert("rotate", v.clone());
                    }
                }
                _ => {
                    n.attrs.insert(k.as_str(), v.clone());
                    n.own_style.insert(k.as_str(), v.clone());
                }
            }
        }
        y += approx_height(&t.text, size, 0.0) + 3.0;
        out.push(n);
    }
    out
}

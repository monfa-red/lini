//! Marker geometry — filled heads (arrow / dot / circle / diamond) and the open ER
//! cardinality family (crow's-foot / one / zero-or-one / one-or-many / zero-or-many).
//! Shared between inline `|line|` primitives and link rendering.

use super::values::num;
use crate::resolve::{MarkerKind, Markers};
use std::fmt::Write;

/// How a marker is painted: the resolved colour, whether it must be inlined
/// (a direct `stroke:` that no class rule can target), and the line thickness
/// that sizes it.
#[derive(Clone, Copy)]
pub struct MarkerPaint<'a> {
    pub color: &'a str,
    pub inline: bool,
    pub thickness: f64,
}

/// The marker tip sits on the line endpoint, with the direction unit-vector
/// pointing into the shape. The line itself stops short (see
/// `shorten_for_markers`) so the marker body still covers its end. Filled
/// markers (arrow / diamond / dot) get `stroke: none` from the `.lini-marker`
/// rule: their size is the `points`/`r` geometry alone, never the link's
/// `stroke-width` inherited off the `<g>` — which used to balloon the head and
/// shove its tip into the shape as the link thickened. Links then nudge the tip
/// a fixed [`MARKER_OVERLAP`] into the shape so the head reads as connected at
/// any thickness.
pub fn marker_anchor(
    from: (f64, f64),
    to: (f64, f64),
    at_start: bool,
) -> Option<((f64, f64), (f64, f64))> {
    let (anchor, neighbor) = if at_start { (from, to) } else { (to, from) };
    let dx = anchor.0 - neighbor.0;
    let dy = anchor.1 - neighbor.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-9 {
        return Some((anchor, (1.0, 0.0)));
    }
    // Tip on the endpoint so the arrow touches the shape edge.
    Some((anchor, (dx / len, dy / len)))
}

/// Marker scales gently with line thickness, with a small floor so thin lines
/// still get a visible head — 5 gives a 1 px line a clear arrow, and a 4× slope
/// keeps thicker links' heads in proportion without chunking.
pub fn marker_size(thickness: f64) -> f64 {
    5.0_f64.max(thickness * 4.0)
}

/// Dot radius as a fraction of the marker `size` — a touch fuller so the circle
/// reads level with the arrow and diamond rather than undersized.
const DOT_RADIUS: f64 = 0.375;

/// `circle` radius as a fraction of the marker `size` — a deliberately larger `dot`
/// ([SPEC §7]), big enough to hover or read on a chart point. Same drawing as the dot,
/// only fuller.
const CIRCLE_RADIUS: f64 = 0.5;

/// ER crow's-foot family geometry, in `marker_size` units (SPEC §7). The fan splays
/// onto the entity edge (at the tip) and converges back along the line; a bar / ring
/// sits behind it for the finer cardinalities.
const CROW_DEPTH: f64 = 1.0; // foot convergence, back from the entity edge
const CROW_SPREAD: f64 = 0.55; // foot half-spread, perpendicular to the line
const BAR_HALF: f64 = 0.5; // the "one" bar's half-width
const RING_R: f64 = 0.4; // the optionality ring's radius

/// How far a link's marker tip is pushed past the endpoint into the shape, so the
/// head overlaps the border by a hair and reads as connected — constant at every
/// `stroke-width` (the line-end shortening absorbs the same shift). `|line|`
/// markers don't use it: a bare line has no shape to meet.
pub const MARKER_OVERLAP: f64 = 0.5;

/// How far back from the endpoint the drawn line stops for an end marker, scaled
/// off `stroke-width` so a thicker line pulls back proportionally. The same `2 ×
/// stroke-width` stub for every marker — always shorter than the head's body, so
/// the line's end tucks under it with no gap (a dot stopped at its own back edge
/// instead left a hairline gap where the circle curved away).
pub fn line_inset(kind: MarkerKind, thickness: f64) -> f64 {
    match kind {
        MarkerKind::None => 0.0,
        // An open ER marker is stroked, not filled, so the line must stop *behind* it
        // (at its furthest element back from the entity) rather than tuck under it.
        k if k.is_open() => marker_size(thickness) * open_back_extent(k),
        _ => thickness * 2.0,
    }
}

/// How far back from the entity edge an open ER marker reaches, in `marker_size` units
/// — where the drawn line ends so it never crosses the marker's strokes.
fn open_back_extent(kind: MarkerKind) -> f64 {
    match kind {
        MarkerKind::One => 0.7,
        MarkerKind::Crow => CROW_DEPTH,
        MarkerKind::OneOrMany => 1.5,
        MarkerKind::ZeroOrOne => 1.5 + RING_R,
        MarkerKind::ZeroOrMany => 1.7 + RING_R,
        _ => CROW_DEPTH,
    }
}

/// Pull a polyline's marker-bearing ends back so the drawn line stops where the
/// marker body begins, not at its tip — otherwise the stroke poked through the
/// arrowhead. `overlap` is how far the tip is nudged past the endpoint
/// ([`MARKER_OVERLAP`] for a link meeting a shape, `0` for a bare `|line|` whose
/// tip sits on the endpoint). Markers still draw at the original endpoints.
pub fn shorten_for_markers(
    path: &[(f64, f64)],
    markers: &Markers,
    thickness: f64,
    overlap: f64,
) -> Vec<(f64, f64)> {
    let inset = |kind| (line_inset(kind, thickness) - overlap).max(0.0);
    let mut p = path.to_vec();
    if p.len() < 2 {
        return p;
    }
    if markers.end != MarkerKind::None {
        let n = p.len();
        if let Some(q) = pulled_back(p[n - 2], p[n - 1], inset(markers.end)) {
            p[n - 1] = q;
        }
    }
    if markers.start != MarkerKind::None
        && let Some(q) = pulled_back(p[1], p[0], inset(markers.start))
    {
        p[0] = q;
    }
    p
}

/// Move `endpoint` toward `inner` by `amount`; `None` if the segment is too short
/// to absorb the shift.
fn pulled_back(inner: (f64, f64), endpoint: (f64, f64), amount: f64) -> Option<(f64, f64)> {
    let (dx, dy) = (endpoint.0 - inner.0, endpoint.1 - inner.1);
    let len = (dx * dx + dy * dy).sqrt();
    if len <= amount + 0.5 {
        return None;
    }
    Some((
        endpoint.0 - dx / len * amount,
        endpoint.1 - dy / len * amount,
    ))
}

/// A round marker's centre: pulled back from the tip (on the shape edge) by its
/// radius along the link, so the circle sits fully on the link side — tangent to
/// the edge, never poking into the shape. Shared by `dot` and `circle`.
fn round_center(tip: (f64, f64), direction: (f64, f64), r: f64) -> (f64, f64) {
    (tip.0 - direction.0 * r, tip.1 - direction.1 * r)
}

/// Emit a filled round marker (`dot` / `circle`): one mechanism, the radius fraction
/// the only difference. Pulled back so its leading edge sits tangent to the endpoint.
fn emit_round(
    out: &mut String,
    indent: &str,
    suffix: &str,
    tip: (f64, f64),
    direction: (f64, f64),
    r: f64,
    fill: &str,
) {
    let (cx, cy) = round_center(tip, direction, r);
    writeln!(
        out,
        r#"{}<circle class="lini-marker lini-marker-{}" cx="{}" cy="{}" r="{}"{}/>"#,
        indent,
        suffix,
        num(cx),
        num(cy),
        num(r),
        fill,
    )
    .unwrap();
}

/// Paint a marker. Filled heads (arrow / diamond / dot) take their `fill` and
/// `stroke: none` from CSS — the base `.lini-marker` rule, or a
/// `.lini-style-* .lini-marker` descendant rule when the link/line carries a
/// recolouring class. They inline `fill` (via `style=`, to beat those rules)
/// only for a *direct* inline `stroke:`, which no class rule can target
/// (`inline`). The open ER markers are stroked, not filled: they paint entirely via the
/// `.lini-marker-open` rule (`stroke: inherit` off the enclosing `<g>`), so they
/// need no inline paint.
pub fn emit_marker(
    out: &mut String,
    indent: &str,
    kind: MarkerKind,
    tip: (f64, f64),
    direction: (f64, f64),
    paint: &MarkerPaint,
) {
    let MarkerPaint {
        color,
        inline,
        thickness,
    } = *paint;
    let size = marker_size(thickness);
    let ux = direction.0;
    let uy = direction.1;
    let px = -uy;
    let py = ux;
    let fill = if inline {
        format!(r#" style="fill: {color}""#)
    } else {
        String::new()
    };
    match kind {
        MarkerKind::Arrow => {
            let bx = tip.0 - ux * size;
            let by = tip.1 - uy * size;
            let lx = bx + px * size * 0.5;
            let ly = by + py * size * 0.5;
            let rx = bx - px * size * 0.5;
            let ry = by - py * size * 0.5;
            writeln!(
                out,
                r#"{}<polygon class="lini-marker lini-marker-arrow" points="{},{} {},{} {},{}"{}/>"#,
                indent,
                num(tip.0), num(tip.1),
                num(lx), num(ly),
                num(rx), num(ry),
                fill,
            ).unwrap();
        }
        MarkerKind::Dot => {
            emit_round(out, indent, "dot", tip, direction, size * DOT_RADIUS, &fill);
        }
        MarkerKind::Circle => {
            emit_round(
                out,
                indent,
                "circle",
                tip,
                direction,
                size * CIRCLE_RADIUS,
                &fill,
            );
        }
        MarkerKind::Diamond => {
            let bx = tip.0 - ux * size;
            let by = tip.1 - uy * size;
            let mx = (tip.0 + bx) / 2.0;
            let my = (tip.1 + by) / 2.0;
            let lx = mx + px * size * 0.425;
            let ly = my + py * size * 0.425;
            let rx = mx - px * size * 0.425;
            let ry = my - py * size * 0.425;
            writeln!(
                out,
                r#"{}<polygon class="lini-marker lini-marker-diamond" points="{},{} {},{} {},{} {},{}"{}/>"#,
                indent,
                num(tip.0), num(tip.1),
                num(lx), num(ly),
                num(bx), num(by),
                num(rx), num(ry),
                fill,
            ).unwrap();
        }
        // The ER cardinality family ([SPEC §7]) — all open strokes (`stroke: inherit`
        // via `.lini-marker-open`). The crow's foot splays onto the entity edge and
        // converges back along the line; a bar means "one", a ring "optional (zero)".
        MarkerKind::Crow => {
            open_path(out, indent, &crow_d(tip, (ux, uy), (px, py), size));
        }
        MarkerKind::One => {
            open_path(out, indent, &bar_d(tip, (ux, uy), (px, py), size, 0.7));
        }
        MarkerKind::ZeroOrOne => {
            open_path(out, indent, &bar_d(tip, (ux, uy), (px, py), size, 0.6));
            open_ring(out, indent, tip, (ux, uy), size, 1.5);
        }
        MarkerKind::OneOrMany => {
            open_path(out, indent, &crow_d(tip, (ux, uy), (px, py), size));
            open_path(out, indent, &bar_d(tip, (ux, uy), (px, py), size, 1.5));
        }
        MarkerKind::ZeroOrMany => {
            open_path(out, indent, &crow_d(tip, (ux, uy), (px, py), size));
            open_ring(out, indent, tip, (ux, uy), size, 1.7);
        }
        MarkerKind::None => {}
    }
}

/// An open-stroke marker element — a crow's-foot fan or a bar — sharing the
/// `.lini-marker-open` class so the stroked-marker rule paints it (`stroke: inherit`).
fn open_path(out: &mut String, indent: &str, d: &str) {
    writeln!(
        out,
        r#"{}<path class="lini-marker lini-marker-open" d="{}"/>"#,
        indent, d
    )
    .unwrap();
}

/// The optionality ring (`zero-or-*`), centred `offset · size` back along the line.
fn open_ring(
    out: &mut String,
    indent: &str,
    tip: (f64, f64),
    dir: (f64, f64),
    size: f64,
    offset: f64,
) {
    let (cx, cy) = (tip.0 - dir.0 * size * offset, tip.1 - dir.1 * size * offset);
    writeln!(
        out,
        r#"{}<circle class="lini-marker lini-marker-open" cx="{}" cy="{}" r="{}"/>"#,
        indent,
        num(cx),
        num(cy),
        num(size * RING_R),
    )
    .unwrap();
}

/// The crow's-foot path: three prongs fanning from a convergence point (back along the
/// line) onto the entity edge at the tip — the standard ER "many".
fn crow_d(tip: (f64, f64), dir: (f64, f64), perp: (f64, f64), size: f64) -> String {
    let (depth, spread) = (size * CROW_DEPTH, size * CROW_SPREAD);
    let (cx, cy) = (tip.0 - dir.0 * depth, tip.1 - dir.1 * depth);
    let (lx, ly) = (tip.0 + perp.0 * spread, tip.1 + perp.1 * spread);
    let (rx, ry) = (tip.0 - perp.0 * spread, tip.1 - perp.1 * spread);
    format!(
        "M {} {} L {} {} L {} {} M {} {} L {} {}",
        num(lx),
        num(ly),
        num(cx),
        num(cy),
        num(rx),
        num(ry),
        num(tip.0),
        num(tip.1),
        num(cx),
        num(cy),
    )
}

/// A cardinality bar — a tick perpendicular to the line, `offset · size` back from the
/// entity edge.
fn bar_d(tip: (f64, f64), dir: (f64, f64), perp: (f64, f64), size: f64, offset: f64) -> String {
    let half = size * BAR_HALF;
    let (bx, by) = (tip.0 - dir.0 * size * offset, tip.1 - dir.1 * size * offset);
    format!(
        "M {} {} L {} {}",
        num(bx + perp.0 * half),
        num(by + perp.1 * half),
        num(bx - perp.0 * half),
        num(by - perp.1 * half),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_sits_tangent_to_the_edge_on_the_link_side() {
        // tip on the shape edge, direction pointing into the shape (+x here). The dot
        // centre is pulled back by its radius, so its leading edge lands exactly on
        // the tip (no overshoot) and the whole circle is on the link side.
        let size = marker_size(1.0);
        let r = size * DOT_RADIUS;
        let (cx, cy) = round_center((100.0, 50.0), (1.0, 0.0), r);
        assert!((cx - (100.0 - r)).abs() < 1e-9, "centre pulled back by r");
        assert!((cy - 50.0).abs() < 1e-9);
        assert!(
            (cx + r - 100.0).abs() < 1e-9,
            "leading edge tangent to the shape edge, not past it"
        );
    }

    #[test]
    fn line_stops_a_uniform_stub_short_of_every_marker() {
        // Every head pulls the line back the same `2 × stroke-width` so its end
        // tucks under the head — the dot included, so no hairline gap forms.
        assert_eq!(line_inset(MarkerKind::Dot, 1.0), 2.0);
        assert_eq!(line_inset(MarkerKind::Circle, 1.0), 2.0);
        assert_eq!(line_inset(MarkerKind::Arrow, 1.0), 2.0);
        assert_eq!(line_inset(MarkerKind::None, 1.0), 0.0);
    }

    #[test]
    fn a_circle_marker_is_a_fuller_dot() {
        // `circle` reuses the dot path at a larger radius fraction — a deliberately
        // bigger, hover-sized point ([SPEC §7]).
        let size = marker_size(1.0);
        assert!(
            size * CIRCLE_RADIUS > size * DOT_RADIUS,
            "circle radius exceeds the dot's"
        );
    }

    #[test]
    fn open_er_markers_are_stroked_via_the_shared_class() {
        let paint = MarkerPaint {
            color: "red",
            inline: true,
            thickness: 2.0,
        };
        // Every ER marker rides `.lini-marker-open` and never inlines a `fill` — even
        // when `inline` is set (which a filled head would honour).
        for kind in [
            MarkerKind::Crow,
            MarkerKind::One,
            MarkerKind::ZeroOrOne,
            MarkerKind::OneOrMany,
            MarkerKind::ZeroOrMany,
        ] {
            let mut s = String::new();
            emit_marker(&mut s, "", kind, (100.0, 50.0), (1.0, 0.0), &paint);
            assert!(
                s.contains("lini-marker-open"),
                "{kind:?} uses the open class"
            );
            assert!(
                !s.contains("fill"),
                "{kind:?} is stroked, never inline-filled"
            );
        }
        // A compound cardinality draws two elements (crow/bar + bar/ring).
        let mut s = String::new();
        emit_marker(
            &mut s,
            "",
            MarkerKind::ZeroOrMany,
            (100.0, 50.0),
            (1.0, 0.0),
            &paint,
        );
        assert_eq!(s.matches("lini-marker-open").count(), 2);
    }

    #[test]
    fn open_marker_line_inset_clears_the_whole_marker() {
        // The line stops behind an open marker (it is stroked, so it can't tuck under);
        // a marker reaching further back (the ring) pulls the line back further.
        assert!(line_inset(MarkerKind::Crow, 2.0) > line_inset(MarkerKind::Arrow, 2.0));
        assert!(line_inset(MarkerKind::ZeroOrMany, 2.0) >= line_inset(MarkerKind::Crow, 2.0));
    }
}

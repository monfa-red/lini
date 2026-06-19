//! Marker geometry (arrow / dot / diamond / crow). Shared between inline
//! `|line|` primitives and wire rendering.

use super::values::num;
use crate::layout::PlacedNode;
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

/// Emit markers for inline `|line|` primitives. Resolve has already settled
/// `n.markers` per source-order rules — we just paint what's there.
pub fn emit_inline_markers(
    out: &mut String,
    indent: &str,
    n: &PlacedNode,
    from: (f64, f64),
    to: (f64, f64),
    paint: &MarkerPaint,
) {
    if n.markers.start != MarkerKind::None
        && let Some((tip, dir)) = marker_anchor(from, to, true)
    {
        emit_marker(out, indent, n.markers.start, tip, dir, paint);
    }
    if n.markers.end != MarkerKind::None
        && let Some((tip, dir)) = marker_anchor(from, to, false)
    {
        emit_marker(out, indent, n.markers.end, tip, dir, paint);
    }
}

/// The marker tip sits on the line endpoint, with the direction unit-vector
/// pointing into the shape. The line itself stops short (see
/// `shorten_for_markers`) so the marker body still covers its end. Filled
/// markers (arrow / diamond / dot) get `stroke: none` from the `.lini-marker`
/// rule: their size is the `points`/`r` geometry alone, never the wire's
/// `stroke-width` inherited off the `<g>` — which used to balloon the head and
/// shove its tip into the shape as the wire thickened. Wires then nudge the tip
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
/// keeps thicker wires' heads in proportion without chunking.
pub fn marker_size(thickness: f64) -> f64 {
    5.0_f64.max(thickness * 4.0)
}

/// How far a wire's marker tip is pushed past the endpoint into the shape, so the
/// head overlaps the border by a hair and reads as connected — constant at every
/// `stroke-width` (the line-end shortening absorbs the same shift). `|line|`
/// markers don't use it: a bare line has no shape to meet.
pub const MARKER_OVERLAP: f64 = 0.5;

/// How far back from the endpoint the drawn line stops for an end marker so the
/// marker body covers the line's end with neither a gap nor an overshoot, scaled
/// off `stroke-width` so a thicker line pulls back proportionally. A dot sits
/// tangent to the endpoint, so the line stops at its back edge (`2·radius`); a
/// pointed marker (arrow / crow / diamond) covers a `2 × stroke-width` stub,
/// always shorter than its `size` body so the line's end stays hidden under it.
pub fn line_inset(kind: MarkerKind, thickness: f64) -> f64 {
    match kind {
        MarkerKind::None => 0.0,
        MarkerKind::Dot => marker_size(thickness) * 2.0 / 3.0,
        _ => thickness * 2.0,
    }
}

/// Pull a polyline's marker-bearing ends back so the drawn line stops where the
/// marker body begins, not at its tip — otherwise the stroke poked through the
/// arrowhead. `overlap` is how far the tip is nudged past the endpoint
/// ([`MARKER_OVERLAP`] for a wire meeting a shape, `0` for a bare `|line|` whose
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
    Some((endpoint.0 - dx / len * amount, endpoint.1 - dy / len * amount))
}

/// A dot marker's centre: pulled back from the tip (on the shape edge) by its
/// radius along the wire, so the circle sits fully on the wire side — tangent to
/// the edge, never poking into the shape.
pub fn dot_center(tip: (f64, f64), direction: (f64, f64), size: f64) -> (f64, f64) {
    let r = size / 3.0;
    (tip.0 - direction.0 * r, tip.1 - direction.1 * r)
}

/// Paint a marker. Filled heads (arrow / diamond / dot) take their `fill` and
/// `stroke: none` from CSS — the base `.lini-marker` rule, or a
/// `.lini-style-* .lini-marker` descendant rule when the wire/line carries a
/// recolouring class. They inline `fill` (via `style=`, to beat those rules)
/// only for a *direct* inline `stroke:`, which no class rule can target
/// (`inline`). The crow is stroked, not filled, so it always states its
/// resolved `color` inline, beating the rule's `stroke: none`.
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
            let (cx, cy) = dot_center(tip, direction, size);
            writeln!(
                out,
                r#"{}<circle class="lini-marker lini-marker-dot" cx="{}" cy="{}" r="{}"{}/>"#,
                indent,
                num(cx),
                num(cy),
                num(size / 3.0),
                fill,
            )
            .unwrap();
        }
        MarkerKind::Diamond => {
            let bx = tip.0 - ux * size;
            let by = tip.1 - uy * size;
            let mx = (tip.0 + bx) / 2.0;
            let my = (tip.1 + by) / 2.0;
            let lx = mx + px * size * 0.4;
            let ly = my + py * size * 0.4;
            let rx = mx - px * size * 0.4;
            let ry = my - py * size * 0.4;
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
        MarkerKind::Crow => {
            let bx = tip.0 - ux * size;
            let by = tip.1 - uy * size;
            let lx = bx + px * size * 0.5;
            let ly = by + py * size * 0.5;
            let rx = bx - px * size * 0.5;
            let ry = by - py * size * 0.5;
            // The two outer prongs are one path through the tip, so a miter join
            // gives a real point (three butt-capped segments used to blunt it);
            // round caps tidy the three splayed feet. The centre prong's tip end
            // tucks inside the join.
            writeln!(
                out,
                r#"{}<path class="lini-marker lini-marker-crow" d="M {} {} L {} {} L {} {} M {} {} L {} {}" style="fill: none; stroke: {}; stroke-width: {}; stroke-linecap: round; stroke-dasharray: none"/>"#,
                indent,
                num(lx), num(ly), num(tip.0), num(tip.1), num(rx), num(ry),
                num(bx), num(by), num(tip.0), num(tip.1),
                color, num(thickness),
            ).unwrap();
        }
        MarkerKind::None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_sits_tangent_to_the_edge_on_the_wire_side() {
        // tip on the shape edge, direction pointing into the shape (+x here). The dot
        // centre is pulled back by its radius, so its leading edge lands exactly on
        // the tip (no overshoot) and the whole circle is on the wire side.
        let size = marker_size(1.0);
        let r = size / 3.0;
        let (cx, cy) = dot_center((100.0, 50.0), (1.0, 0.0), size);
        assert!((cx - (100.0 - r)).abs() < 1e-9, "centre pulled back by r");
        assert!((cy - 50.0).abs() < 1e-9);
        assert!(
            (cx + r - 100.0).abs() < 1e-9,
            "leading edge tangent to the shape edge, not past it"
        );
    }

    #[test]
    fn line_stops_at_the_dot_back_edge_but_a_stub_for_pointed_markers() {
        let size = marker_size(1.0);
        // The dot spans [tip-2r, tip]; the line must stop at its back edge = 2r.
        assert!((line_inset(MarkerKind::Dot, 1.0) - 2.0 * size / 3.0).abs() < 1e-9);
        assert_eq!(line_inset(MarkerKind::Arrow, 1.0), 2.0);
        assert_eq!(line_inset(MarkerKind::None, 1.0), 0.0);
    }
}

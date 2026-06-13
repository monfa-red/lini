//! Marker geometry (arrow / dot / diamond / crow). Shared between inline
//! `|line|` primitives and wire rendering.

use super::values::num;
use crate::layout::PlacedNode;
use crate::resolve::MarkerKind;
use std::fmt::Write;

/// Emit markers for inline `|line|` primitives. Resolve has already settled
/// `n.markers` per source-order rules — we just paint what's there.
pub fn emit_inline_markers(
    out: &mut String,
    indent: &str,
    n: &PlacedNode,
    from: (f64, f64),
    to: (f64, f64),
    stroke: &str,
    thickness: f64,
) {
    if n.markers.start != MarkerKind::None
        && let Some((tip, dir)) = marker_anchor(from, to, true)
    {
        emit_marker(out, indent, n.markers.start, tip, dir, stroke, thickness);
    }
    if n.markers.end != MarkerKind::None
        && let Some((tip, dir)) = marker_anchor(from, to, false)
    {
        emit_marker(out, indent, n.markers.end, tip, dir, stroke, thickness);
    }
}

/// The marker tip sits exactly on the line endpoint (the shape edge), with the
/// direction unit-vector pointing into the shape. The line itself stops short
/// (see `shorten_for_markers`) so the marker body still covers its end.
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

/// The fixed stub a pointed marker (arrow / crow / diamond) covers: its body runs
/// `size` (≥ 5) back from the tip, so stopping the line 4 px short leaves no gap.
pub const STUB_INSET: f64 = 4.0;

/// How far back from the endpoint the drawn line should stop for an end marker so
/// the marker body covers the line's end with neither a gap nor an overshoot. A dot
/// is small and sits tangent to the shape edge, so the line stops at its back edge
/// (`2·radius`); pointed markers cover the fixed [`STUB_INSET`] stub.
pub fn line_inset(kind: MarkerKind, thickness: f64) -> f64 {
    match kind {
        MarkerKind::None => 0.0,
        MarkerKind::Dot => marker_size(thickness) * 2.0 / 3.0,
        _ => STUB_INSET,
    }
}

/// A dot marker's centre: pulled back from the tip (on the shape edge) by its
/// radius along the wire, so the circle sits fully on the wire side — tangent to
/// the edge, never poking into the shape.
pub fn dot_center(tip: (f64, f64), direction: (f64, f64), size: f64) -> (f64, f64) {
    let r = size / 3.0;
    (tip.0 - direction.0 * r, tip.1 - direction.1 * r)
}

pub fn emit_marker(
    out: &mut String,
    indent: &str,
    kind: MarkerKind,
    tip: (f64, f64),
    direction: (f64, f64),
    stroke: &str,
    thickness: f64,
) {
    let size = marker_size(thickness);
    let ux = direction.0;
    let uy = direction.1;
    let px = -uy;
    let py = ux;
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
                r#"{}<polygon class="lini-marker lini-marker-arrow" points="{},{} {},{} {},{}" fill="{}"/>"#,
                indent,
                num(tip.0), num(tip.1),
                num(lx), num(ly),
                num(rx), num(ry),
                stroke,
            ).unwrap();
        }
        MarkerKind::Dot => {
            let (cx, cy) = dot_center(tip, direction, size);
            writeln!(
                out,
                r#"{}<circle class="lini-marker lini-marker-dot" cx="{}" cy="{}" r="{}" fill="{}"/>"#,
                indent,
                num(cx),
                num(cy),
                num(size / 3.0),
                stroke,
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
                r#"{}<polygon class="lini-marker lini-marker-diamond" points="{},{} {},{} {},{} {},{}" fill="{}"/>"#,
                indent,
                num(tip.0), num(tip.1),
                num(lx), num(ly),
                num(bx), num(by),
                num(rx), num(ry),
                stroke,
            ).unwrap();
        }
        MarkerKind::Crow => {
            let bx = tip.0 - ux * size;
            let by = tip.1 - uy * size;
            let lx = bx + px * size * 0.5;
            let ly = by + py * size * 0.5;
            let rx = bx - px * size * 0.5;
            let ry = by - py * size * 0.5;
            writeln!(
                out,
                r#"{}<path class="lini-marker lini-marker-crow" d="M {} {} L {} {} M {} {} L {} {} M {} {} L {} {}" stroke="{}" stroke-width="{}" stroke-dasharray="none" fill="none"/>"#,
                indent,
                num(tip.0), num(tip.1), num(bx), num(by),
                num(tip.0), num(tip.1), num(lx), num(ly),
                num(tip.0), num(tip.1), num(rx), num(ry),
                stroke, num(thickness),
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
        assert_eq!(line_inset(MarkerKind::Arrow, 1.0), STUB_INSET);
        assert_eq!(line_inset(MarkerKind::None, 1.0), 0.0);
    }
}

//! Wire emission — the wire path, optional markers, optional labels — and
//! airwires, the impossible-wire report made visible.

use super::markers::{
    MARKER_OVERLAP, MarkerPaint, emit_marker, marker_anchor, shorten_for_markers,
};
use super::rounding::{Seg, round};
use super::rules::{PAINT_PROPS, RuleSet, effective_stroke};
use super::values::{attr_or_var, dasharray_value, escape_xml, format_value, num};
use super::wavy;
use crate::Options;
use crate::layout::{Airwire, RoutedText, RoutedWire, approx_height, approx_width};
use crate::resolve::{AttrMap, MarkerKind, ResolvedValue, VarTable};
use std::fmt::Write;

/// Whether a wire's `~` operator (or an explicit `stroke-style: wavy`) asks for
/// a wavy line, drawn as an undulating centreline rather than a dash pattern.
fn is_wavy(attrs: &AttrMap) -> bool {
    matches!(attrs.get("stroke-style"), Some(ResolvedValue::Ident(s)) if s == "wavy")
}

/// Breathing room the label cut keeps around the glyph run, in `font-size`
/// units per side. `H` pads the approximate text width; `V` makes the hole
/// taller than the tight single-line box ([`approx_height`] is ~1 em, which
/// clips descenders) so g/y/p stay inside the cut.
const LABEL_CUT_PAD_H: f64 = 0.3;
const LABEL_CUT_PAD_V: f64 = 0.15;

/// The wire's corner-radius cap (WIRING §Model step 7): the wire's resolved
/// `clearance` (carrying the `-> { }` default desugar injects), else 0.
pub fn radius_cap(w: &RoutedWire) -> f64 {
    w.attrs.number("clearance").unwrap_or(0.0)
}

#[allow(clippy::too_many_arguments)]
pub fn render_wire(
    out: &mut String,
    idx: usize,
    w: &RoutedWire,
    targets: &[f64],
    label_size: f64,
    vars: &VarTable,
    ruleset: &RuleSet,
    opts: &Options,
) {
    if w.path.len() < 2 {
        return;
    }
    let thickness = w.attrs.number("stroke-width").unwrap_or(0.0);

    // Paint rides the group, exactly like a node: the `.lini-wire` rule states
    // the `|wire|` defaults, each applied `.style` rides a `lini-style-*` class,
    // and only genuine differences (the operator's dash, an inline attr) land in
    // `style=`.
    let mut wire_classes = vec!["lini-wire".to_string()];
    // A dashed/dotted line rides its `lini-wire-{style}` class (the dash pattern
    // is stated once in the sheet); only an off-default `stroke-width` then makes
    // the pattern differ enough to inline.
    if let Some(ResolvedValue::Ident(s)) = w.attrs.get("stroke-style")
        && (s == "dashed" || s == "dotted")
    {
        wire_classes.push(format!("lini-wire-{s}"));
    }
    wire_classes.extend(w.applied_styles.iter().map(|s| format!("lini-style-{}", s)));
    let mut decls: Vec<(&str, String)> = Vec::new();
    for (lini, css) in PAINT_PROPS {
        let Some(v) = w.attrs.get(lini) else {
            continue;
        };
        let formatted = format_value(v, vars, opts);
        if ruleset.provided(&wire_classes, css) != Some(formatted.as_str()) {
            decls.push((css, formatted));
        }
    }
    if w.attrs.get("stroke-style").is_some() {
        let dash = dasharray_value(&w.attrs, thickness);
        let value = if dash.is_empty() {
            "none".to_string()
        } else {
            dash
        };
        if ruleset.provided(&wire_classes, "stroke-dasharray") != Some(value.as_str()) {
            decls.push(("stroke-dasharray", value));
        }
    }
    let style_attr = if decls.is_empty() {
        String::new()
    } else {
        let body: Vec<String> = decls.iter().map(|(p, v)| format!("{}: {}", p, v)).collect();
        format!(r#" style="{}""#, body.join("; "))
    };

    // `link:` makes the wire clickable, mirroring a node's `<a href>` wrap.
    let link = match w.attrs.get("link") {
        Some(crate::resolve::ResolvedValue::String(s)) => Some(s.clone()),
        _ => None,
    };
    if let Some(href) = &link {
        writeln!(out, r#"    <a href="{}">"#, escape_xml(href)).unwrap();
    }

    writeln!(
        out,
        r#"    <g class="{}"{} data-from="{}" data-to="{}">"#,
        wire_classes.join(" "),
        style_attr,
        escape_xml(&w.data_from),
        escape_xml(&w.data_to),
    )
    .unwrap();

    let wavy = is_wavy(&w.attrs);

    // A label cuts the wire out beneath it (a mask hole, not a painted halo) so
    // it reads cleanly over the wire on any background. A wavy line swings
    // `AMPLITUDE` past the routed bbox, so the cut region grows to match.
    let reach = if wavy { wavy::AMPLITUDE } else { 0.0 };
    let mask = label_mask(idx, &w.path, &w.texts, thickness, reach);
    let mask_attr = match &mask {
        Some((id, svg)) => {
            writeln!(out, "      {svg}").unwrap();
            format!(r#" mask="url(#{id})""#)
        }
        None => String::new(),
    };

    // Stop the drawn line where the marker body will sit so the stroke never
    // pokes past it (and never leaves a gap before a dot).
    let drawn = shorten_for_markers(&w.path, &w.markers, thickness, MARKER_OVERLAP);
    let d = wavy
        .then(|| wavy::wavy_d(&drawn, targets))
        .flatten()
        .unwrap_or_else(|| rounded_d(&drawn, targets));
    writeln!(out, r#"      <path d="{d}"{mask_attr}/>"#).unwrap();

    // The marker colour: filled heads inherit it from CSS (the `.lini-marker`
    // base or a `.lini-style-* .lini-marker` descendant rule), so they inline it
    // only for a direct inline `stroke:`. The crow inlines the cascade-resolved
    // colour regardless (it is stroked, no fill rule reaches it).
    let marker_color = effective_stroke(&w.attrs, &wire_classes, ruleset, vars, opts);
    let paint = MarkerPaint {
        color: &marker_color,
        inline: ruleset.marker_fill(&wire_classes) != Some(marker_color.as_str()),
        thickness,
    };
    if w.markers.start != MarkerKind::None
        && let Some((tip, dir)) = marker_anchor(w.path[1], w.path[0], false)
    {
        emit_marker(
            out,
            "      ",
            w.markers.start,
            overlap_tip(tip, dir),
            dir,
            &paint,
        );
    }
    if w.markers.end != MarkerKind::None {
        let n = w.path.len();
        if let Some((tip, dir)) = marker_anchor(w.path[n - 2], w.path[n - 1], false) {
            emit_marker(
                out,
                "      ",
                w.markers.end,
                overlap_tip(tip, dir),
                dir,
                &paint,
            );
        }
    }

    for t in &w.texts {
        render_wire_text(out, t, label_size, vars, opts);
    }

    out.push_str("    </g>\n");
    if link.is_some() {
        out.push_str("    </a>\n");
    }
}

/// An airwire (WIRING §Impossible layouts): a dashed straight segment in the
/// `--lini-airwire` style with a warning glyph at its midpoint. Lawful wires
/// are orthogonal, so the slant is structurally unmistakable; the dashing and
/// glyph cover the aligned-bodies case.
pub fn render_airwire(out: &mut String, a: &Airwire, vars: &VarTable, opts: &Options) {
    let none = AttrMap::default();
    let stroke = attr_or_var(&none, "stroke", "airwire", vars, opts);
    let bg = attr_or_var(&none, "fill", "bg", vars, opts);
    writeln!(
        out,
        r#"    <g class="lini-airwire" data-from="{}" data-to="{}">"#,
        escape_xml(&a.data_from),
        escape_xml(&a.data_to),
    )
    .unwrap();
    writeln!(
        out,
        r#"      <path d="M {} {} L {} {}" fill="none" stroke="{stroke}" stroke-width="1.5" stroke-dasharray="6,4"/>"#,
        num(a.from.0),
        num(a.from.1),
        num(a.to.0),
        num(a.to.1),
    )
    .unwrap();
    let (mx, my) = ((a.from.0 + a.to.0) / 2.0, (a.from.1 + a.to.1) / 2.0);
    writeln!(
        out,
        r#"      <path d="M {} {} L {} {} L {} {} Z" fill="{bg}" stroke="{stroke}" stroke-width="1.5" stroke-linejoin="round"/>"#,
        num(mx),
        num(my - 6.5),
        num(mx + 7.0),
        num(my + 5.5),
        num(mx - 7.0),
        num(my + 5.5),
    )
    .unwrap();
    writeln!(
        out,
        r#"      <path d="M {mx} {} L {mx} {}" stroke="{stroke}" stroke-width="1.6" stroke-linecap="round"/>"#,
        num(my - 2.5),
        num(my + 1.0),
        mx = num(mx),
    )
    .unwrap();
    writeln!(
        out,
        r#"      <circle cx="{}" cy="{}" r="0.9" fill="{stroke}"/>"#,
        num(mx),
        num(my + 3.6),
    )
    .unwrap();
    out.push_str("    </g>\n");
}

/// The path `d` with every interior corner rounded into a quarter arc —
/// radius from the fillet pass ([`fillet_targets`]), still capped by half
/// of each adjacent *drawn* segment so arcs never eat marker run-ups
/// (WIRING §Model step 7). The end segments stay straight.
fn rounded_d(pts: &[(f64, f64)], targets: &[f64]) -> String {
    let rounded = round(pts, targets);
    let mut d = format!("M {} {}", num(rounded.start.0), num(rounded.start.1));
    for seg in &rounded.segs {
        match seg {
            Seg::Line { to } => write!(d, " L {} {}", num(to.0), num(to.1)).unwrap(),
            Seg::Arc {
                to, radius, sweep, ..
            } => write!(
                d,
                " A {r} {r} 0 0 {sweep} {} {}",
                num(to.0),
                num(to.1),
                r = num(*radius),
            )
            .unwrap(),
        }
    }
    d
}

/// One interior corner of one polyline, keyed for nesting: the turn's
/// **quadrant** (the diagonal direction its arc centre lies in, from the
/// leg directions), the **diagonal line** it sits on (the coordinate
/// orthogonal to the quadrant diagonal), and its **projection** along the
/// diagonal (innermost — nearest the shared centre side — first).
struct Corner {
    wire: usize,
    /// Interior vertex index − 1: position in the wire's target vector.
    slot: usize,
    quad: (i8, i8),
    diag: f64,
    proj: f64,
    /// Structural ceiling: min(half legs, nearest crossing on the legs).
    /// Nested radii may exceed the clearance cap, never this.
    ceil: f64,
    /// The wire's clearance cap — the base radius for lone and innermost
    /// corners.
    cap: f64,
}

/// Per-wire, per-interior-corner fillet radius targets (WIRING §Model
/// step 7): corners nested on one diagonal — same turn quadrant, vertices
/// offset equally in x and y — round **concentrically**: the innermost
/// keeps the base cap and each corner outward grows by exactly its offset,
/// so the gap through the turn holds constant instead of flaring. Every
/// radius also caps at the nearest crossing on its own legs, so a crossing
/// never lands mid-arc (an arc may land tangent exactly on one — the
/// perpendicular point contact is preserved). A capped radius only ever
/// *flares* a nested pair apart (the centres part toward the outside), so
/// rounding never brings two wires nearer than their polylines' pitch.
pub fn fillet_targets(polys: &[&[(f64, f64)]], caps: &[f64]) -> Vec<Vec<f64>> {
    const EPS: f64 = 1e-6;
    let mut out: Vec<Vec<f64>> = polys
        .iter()
        .map(|p| vec![0.0; p.len().saturating_sub(2)])
        .collect();
    let mut corners: Vec<Corner> = Vec::new();
    for (wi, poly) in polys.iter().enumerate() {
        for k in 1..poly.len().saturating_sub(1) {
            let (a, v, b) = (poly[k - 1], poly[k], poly[k + 1]);
            let (ix, iy) = (v.0 - a.0, v.1 - a.1);
            let (ox, oy) = (b.0 - v.0, b.1 - v.1);
            if ix * oy - iy * ox == 0.0 {
                continue; // collinear: no arc
            }
            let unit = |x: f64, y: f64| {
                let l = x.abs() + y.abs();
                (x / l, y / l)
            };
            let (ux, uy) = unit(ix, iy);
            let (wx, wy) = unit(ox, oy);
            let quad = ((wx - ux).signum() as i8, (wy - uy).signum() as i8);
            let in_len = ix.abs() + iy.abs();
            let out_len = ox.abs() + oy.abs();
            let mut ceil = (in_len / 2.0).min(out_len / 2.0);
            for (wj, other) in polys.iter().enumerate() {
                if wj == wi {
                    continue;
                }
                for s in other.windows(2) {
                    for leg in [[a, v], [v, b]] {
                        if let Some(at) = crate::layout::cross(&leg, s) {
                            let t = (at.0 - v.0).abs() + (at.1 - v.1).abs();
                            ceil = ceil.min(t);
                        }
                    }
                }
            }
            corners.push(Corner {
                wire: wi,
                slot: k - 1,
                quad,
                diag: if quad.0 as f64 * quad.1 as f64 > 0.0 {
                    v.0 - v.1
                } else {
                    v.0 + v.1
                },
                proj: v.0 * quad.0 as f64 + v.1 * quad.1 as f64,
                ceil,
                cap: caps[wi],
            });
        }
    }
    // Cluster first — same quadrant, diagonal coordinates chained within
    // EPS (one geometric diagonal carries float dust from differing
    // coordinate sums) — then walk each cluster innermost-out by
    // projection. Sorting by the raw diagonal value alone once interleaved
    // a nest's walk order and drove radii negative.
    corners.sort_by(|a, b| {
        a.quad
            .cmp(&b.quad)
            .then(a.diag.total_cmp(&b.diag))
            .then(a.wire.cmp(&b.wire))
            .then(a.slot.cmp(&b.slot))
    });
    let mut i = 0;
    while i < corners.len() {
        let mut j = i + 1;
        while j < corners.len()
            && corners[j].quad == corners[i].quad
            && (corners[j].diag - corners[j - 1].diag).abs() <= EPS
        {
            j += 1;
        }
        let mut cluster: Vec<&Corner> = corners[i..j].iter().collect();
        cluster.sort_by(|a, b| {
            b.proj
                .total_cmp(&a.proj)
                .then(a.wire.cmp(&b.wire))
                .then(a.slot.cmp(&b.slot))
        });
        let mut prev: Option<(&Corner, f64)> = None;
        for c in cluster {
            // Offset to the previous (inner) corner along the diagonal. A
            // far-apart pair on one diagonal is coincidence, not nesting:
            // only lane-scale offsets chain (a skipped lane still nests).
            let r = match prev {
                Some((p, pr)) if (p.proj - c.proj) / 2.0 <= 2.0 * c.cap.max(p.cap) + EPS => {
                    (pr + (p.proj - c.proj) / 2.0).min(c.ceil)
                }
                _ => c.cap.min(c.ceil),
            };
            out[c.wire][c.slot] = r;
            prev = Some((c, r));
        }
        i = j;
    }
    out
}

/// A wire marker's tip, nudged [`MARKER_OVERLAP`] past the endpoint into the
/// shape so the head reads as connected (`dir` points into the shape).
fn overlap_tip(tip: (f64, f64), dir: (f64, f64)) -> (f64, f64) {
    (
        tip.0 + dir.0 * MARKER_OVERLAP,
        tip.1 + dir.1 * MARKER_OVERLAP,
    )
}

/// A luminance mask that cuts the wire path out under each of its labels — the
/// background-independent replacement for a painted halo. White shows the path
/// (over its stroked bounds); a black box per label punches a hole. An explicit
/// `userSpaceOnUse` region is required, else a straight wire's near-flat bbox
/// would shrink the default region to nothing and hide the whole wire. `None`
/// when the wire carries no labels.
fn label_mask(
    idx: usize,
    path: &[(f64, f64)],
    texts: &[RoutedText],
    thickness: f64,
    reach: f64,
) -> Option<(String, String)> {
    if texts.is_empty() {
        return None;
    }
    let id = format!("lini-label-cut-{idx}");
    let pad = thickness / 2.0 + 1.0 + reach;
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(x, y) in path {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    let (rx, ry) = (x0 - pad, y0 - pad);
    let (rw, rh) = (x1 - x0 + 2.0 * pad, y1 - y0 + 2.0 * pad);
    // The mask rects carry their fill/stroke via CSS (`.lini-cut-bg` /
    // `.lini-cut`), not inline — so the wire's own `stroke` can't bleed into the
    // luminance mask, and the SVG stays free of per-label paint (SPEC §13).
    let mut m = format!(
        r#"<mask id="{id}" maskUnits="userSpaceOnUse" x="{}" y="{}" width="{}" height="{}"><rect class="lini-cut-bg" x="{}" y="{}" width="{}" height="{}"/>"#,
        num(rx),
        num(ry),
        num(rw),
        num(rh),
        num(rx),
        num(ry),
        num(rw),
        num(rh),
    );
    for t in texts {
        let size = t.attrs.number("font-size").unwrap_or(0.0);
        let cw = approx_width(&t.content, size) + size * LABEL_CUT_PAD_H * 2.0;
        let ch = approx_height(&t.content, size) + size * LABEL_CUT_PAD_V * 2.0;
        let (cx, cy) = t.position;
        write!(
            m,
            r#"<rect class="lini-cut" x="{}" y="{}" width="{}" height="{}"/>"#,
            num(cx - cw / 2.0),
            num(cy - ch / 2.0),
            num(cw),
            num(ch),
        )
        .unwrap();
    }
    m.push_str("</mask>");
    Some((id, m))
}

/// A wire label. The constant paint (`fill: currentColor`, `stroke: none` so the
/// glyphs don't inherit the wire `<g>`'s stroke, the anchor pair, the baked wire
/// font size) rides `.lini-wire-label`; only a label that overrides one of those
/// inlines the difference via `style=` (which beats the class rule).
fn render_wire_text(out: &mut String, t: &RoutedText, wfs: f64, vars: &VarTable, opts: &Options) {
    let (x, y) = t.position;
    let mut style: Vec<String> = Vec::new();

    // `wfs` is the wire-label default (the `.lini-wire-label` rule's size); inline
    // the label's own size only when it differs.
    let size = t.attrs.number("font-size").unwrap_or(wfs);
    if (size - wfs).abs() > 1e-9 {
        style.push(format!("font-size: {}px", num(size)));
    }
    if let Some(v) = t.attrs.get("fill").or_else(|| t.attrs.get("color")) {
        style.push(format!("fill: {}", format_value(v, vars, opts)));
    }
    if t.attrs.get("font-family").is_some() {
        let font = attr_or_var(&t.attrs, "font-family", "font-family", vars, opts);
        if font
            != attr_or_var(
                &AttrMap::default(),
                "font-family",
                "font-family",
                vars,
                opts,
            )
        {
            style.push(format!("font-family: {font}"));
        }
    }
    if let Some(v) = t.attrs.get("font-weight") {
        style.push(format!("font-weight: {}", format_value(v, vars, opts)));
    }

    let style_attr = if style.is_empty() {
        String::new()
    } else {
        format!(r#" style="{}""#, style.join("; "))
    };
    writeln!(
        out,
        r#"      <text class="lini-wire-label" x="{}" y="{}"{}>{}</text>"#,
        num(x),
        num(y),
        style_attr,
        escape_xml(&t.content),
    )
    .unwrap();
}

#[cfg(test)]
mod tests {
    use super::fillet_targets;

    /// An L-corner travelling +x then +y, vertex at `v`, legs `len` long.
    fn ell(v: (f64, f64), len: f64) -> Vec<(f64, f64)> {
        vec![(v.0 - len, v.1), v, (v.0, v.1 + len)]
    }

    #[test]
    fn nested_corners_round_concentrically() {
        // Three wires turning together at lane pitch 8: vertices step
        // outward along the (+1,−1) diagonal (centre quadrant (−1,+1)).
        let (a, b, c) = (
            ell((0.0, 0.0), 100.0),
            ell((8.0, -8.0), 100.0),
            ell((16.0, -16.0), 100.0),
        );
        let t = fillet_targets(&[&a, &b, &c], &[8.0; 3]);
        assert_eq!((t[0][0], t[1][0], t[2][0]), (8.0, 16.0, 24.0));
    }

    #[test]
    fn opposite_travel_still_nests() {
        // The outer wire traverses the same corner the other way
        // (−y then −x): same arc quadrant, same shared centre.
        let a = ell((0.0, 0.0), 100.0);
        let b = vec![(8.0, 92.0), (8.0, -8.0), (-92.0, -8.0)];
        let t = fillet_targets(&[&a, &b], &[8.0; 2]);
        assert_eq!((t[0][0], t[1][0]), (8.0, 16.0));
    }

    #[test]
    fn a_far_corner_on_the_same_diagonal_is_not_nested() {
        let (a, b) = (ell((0.0, 0.0), 100.0), ell((80.0, -80.0), 100.0));
        let t = fillet_targets(&[&a, &b], &[8.0; 2]);
        assert_eq!((t[0][0], t[1][0]), (8.0, 8.0));
    }

    #[test]
    fn a_crossing_on_a_leg_caps_the_radius() {
        // A vertical wire crosses the corner's incoming leg 5 before the
        // vertex: the arc must land tangent at the crossing, never past it.
        let a = ell((0.0, 0.0), 100.0);
        let b = vec![(-5.0, -50.0), (-5.0, 50.0)];
        let t = fillet_targets(&[&a, &b], &[8.0; 2]);
        assert_eq!(t[0][0], 5.0);
    }

    #[test]
    fn float_dust_on_the_diagonal_never_reorders_a_nest() {
        // The wires_hard hub fan: three corners whose diagonal coordinates
        // differ only in the last float bits, declared outermost-first. The
        // nest must still walk innermost-out (8, 16, 24) — sorting by the
        // raw diagonal value once interleaved the walk and drove radii
        // negative.
        let corner =
            |v: (f64, f64), down: f64| vec![(-5.775000000000006, v.1), v, (v.0, v.1 + down)];
        let outer = corner((-78.22500000000001, -64.6), 52.1);
        let inner = corner((-62.22500000000001, -48.6), 81.7);
        let middle = corner((-70.22500000000001, -56.6), 156.0);
        let t = fillet_targets(&[&outer, &inner, &middle], &[8.0; 3]);
        for (got, want) in [(t[1][0], 8.0), (t[2][0], 16.0), (t[0][0], 24.0)] {
            assert!((got - want).abs() < 1e-9, "{got} != {want}");
        }
    }

    #[test]
    fn short_legs_cap_a_nested_radius_without_unnesting_the_rest() {
        // The middle wire's outgoing leg holds only r = 10: it caps there,
        // and the outer corner keeps stepping from the capped value.
        let (a, b, c) = (
            ell((0.0, 0.0), 100.0),
            vec![(-92.0, -8.0), (8.0, -8.0), (8.0, 12.0)],
            ell((16.0, -16.0), 100.0),
        );
        let t = fillet_targets(&[&a, &b, &c], &[8.0; 3]);
        assert_eq!((t[0][0], t[1][0], t[2][0]), (8.0, 10.0, 18.0));
    }
}

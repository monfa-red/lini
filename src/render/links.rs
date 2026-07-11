//! Link emission — the link path, optional markers, optional labels — and
//! strays, the impossible-link report made visible.

use super::markers::{
    MARKER_OVERLAP, MarkerPaint, emit_marker, marker_anchor, shorten_for_markers,
};
use super::rules::{RuleSet, effective_stroke};
use super::values::{attr_or_var, escape_xml, format_value, num};
use super::wavy;
use crate::Options;
use crate::layout::{RoutedLink, RoutedText, Stray, approx_height, approx_width};
use crate::ledger::consts::DEFAULT_CLEARANCE;
use crate::resolve::{AttrMap, MarkerKind, ResolvedValue, VarTable};
use std::fmt::Write;

/// Whether a link's `~` operator (or an explicit `stroke-style: wavy`) asks for
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

/// The link's corner-radius cap (ROUTING Model step 7): the link's resolved
/// `clearance` (its cascaded default).
pub fn radius_cap(w: &RoutedLink) -> f64 {
    w.attrs.number("clearance").unwrap_or(DEFAULT_CLEARANCE)
}

#[allow(clippy::too_many_arguments)] // one link's full emission context
pub fn render_link(
    out: &mut String,
    idx: usize,
    w: &RoutedLink,
    targets: &[f64],
    cuts: &[(f64, f64, f64, f64)],
    vars: &VarTable,
    ruleset: &RuleSet,
    opts: &Options,
    sink: &super::fonts::FontSink,
) {
    if w.path.len() < 2 {
        return;
    }
    let thickness = w.attrs.number("stroke-width").unwrap_or(0.0);

    // Paint rides the group, exactly like a node: the `.lini-link` rule states
    // the `|link|` defaults, each applied `.style` rides a `lini-style-*` class,
    // and only genuine differences (the operator's dash, an inline attr) land in
    // `style=`.
    let mut link_classes = vec!["lini-link".to_string()];
    // A dashed/dotted line rides its `lini-link-{style}` class (the dash pattern
    // is stated once in the sheet); only an off-default `stroke-width` then makes
    // the pattern differ enough to inline.
    if let Some(ResolvedValue::Ident(s)) = w.attrs.get("stroke-style")
        && (s == "dashed" || s == "dotted")
    {
        link_classes.push(format!("lini-link-{s}"));
    }
    link_classes.extend(w.applied_styles.iter().map(|s| format!("lini-style-{}", s)));
    // A link's `<g>` paint is the same class-diff a node's is [SPEC 17] — one
    // shared computation; a link never aliases `color` and formats with plain
    // `format_value`.
    let mut decls = ruleset.inline_paint_diff(
        &link_classes,
        &w.attrs,
        |lini| w.attrs.get(lini),
        |_lini, v| format_value(v, vars, opts),
    );
    // The `<g>` carries only **wire** paint; its labels own their text (font / colour),
    // stated once by the label's own class — so text props never inline on the link
    // (once per label × diagram), and no stray `font-size` rides the `<g>` [SPEC 9].
    decls.retain(|(k, _)| {
        matches!(
            *k,
            "stroke" | "stroke-width" | "stroke-dasharray" | "opacity"
        )
    });
    let style_attr = super::style_attr_from(&decls);

    // `href:` makes the link clickable, mirroring a node's `<a href>` wrap.
    let href = match w.attrs.get("href") {
        Some(crate::resolve::ResolvedValue::String(s)) => Some(s.clone()),
        _ => None,
    };
    if let Some(url) = &href {
        writeln!(out, r#"    <a href="{}">"#, escape_xml(url)).unwrap();
    }

    writeln!(
        out,
        r#"    <g class="{}"{} data-from="{}" data-to="{}">"#,
        link_classes.join(" "),
        style_attr,
        escape_xml(&w.data_from),
        escape_xml(&w.data_to),
    )
    .unwrap();

    let wavy = is_wavy(&w.attrs);

    // A label cuts the link out beneath it (a mask hole, not a painted halo) so
    // it reads cleanly over the link on any background. A wavy line swings
    // `AMPLITUDE` past the routed bbox, so the cut region grows to match.
    let reach = if wavy {
        crate::ledger::consts::WAVY_AMPLITUDE
    } else {
        0.0
    };
    let mask = label_mask(idx, &w.path, cuts, thickness, reach);
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
    let marker_color = effective_stroke(&w.attrs, &link_classes, ruleset, vars, opts);
    let paint = MarkerPaint {
        color: &marker_color,
        inline: ruleset.marker_fill(&link_classes) != Some(marker_color.as_str()),
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
        render_link_text(out, t, ruleset, vars, opts, sink);
    }

    out.push_str("    </g>\n");
    if href.is_some() {
        out.push_str("    </a>\n");
    }
}

/// A stray (ROUTING Impossible layouts): a dashed straight segment in the
/// `--lini-stray` style with a warning glyph at its midpoint. Lawful links
/// are orthogonal, so the slant is structurally unmistakable; the dashing and
/// glyph cover the aligned-bodies case.
pub fn render_stray(out: &mut String, a: &Stray, vars: &VarTable, opts: &Options) {
    let none = AttrMap::default();
    let stroke = attr_or_var(&none, "stroke", "stray", vars, opts);
    // The warning glyph knocks out against the box fill so it reads on any
    // background (was `--lini-bg`, now the scene background — [SPEC 10.1]).
    let glyph_fill = attr_or_var(&none, "fill", "fill", vars, opts);
    writeln!(
        out,
        r#"    <g class="lini-stray" data-from="{}" data-to="{}">"#,
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
        r#"      <path d="M {} {} L {} {} L {} {} Z" fill="{glyph_fill}" stroke="{stroke}" stroke-width="1.5" stroke-linejoin="round"/>"#,
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
/// radius from the fillet pass ([`fillet_targets`]), kept feasible on the
/// *drawn* (marker-shortened) legs by the shared formatter, so an arc never
/// eats a neighbouring arc or a marker pull-back (ROUTING Model step 7).
fn rounded_d(pts: &[(f64, f64)], targets: &[f64]) -> String {
    super::rounding::path_d(pts, targets)
}

/// One interior corner of one polyline, keyed for nesting: the turn's
/// **quadrant** (the diagonal direction its arc centre lies in, from the
/// leg directions), its **vertex**, and its **projection** along the
/// quadrant diagonal (innermost — nearest the shared centre side — first).
struct Corner {
    link: usize,
    /// Interior vertex index − 1: position in the link's target vector.
    slot: usize,
    quad: (i8, i8),
    v: (f64, f64),
    proj: f64,
    /// Structural ceiling: min(own legs, nearest crossing on the legs).
    /// Nested radii may exceed the clearance cap, never this. Two corners
    /// sharing a leg settle their joint fit at draw time
    /// ([`super::rounding::round`]) — the ceiling is per corner.
    ceil: f64,
    /// The link's clearance cap — the base radius for lone and innermost
    /// corners.
    cap: f64,
}

/// Per-link, per-interior-corner fillet radius targets (ROUTING Model
/// step 7): corners nested on one diagonal — same turn quadrant, each
/// vertex offset outward from an inner corner on **both** axes — round
/// **concentrically**: the innermost keeps the base cap and each corner
/// outward grows by the mean of its two axis offsets. Equal offsets (one
/// true diagonal) share an exact centre and the gap holds constant through
/// the turn; unequal offsets — two relief groups can compress the two axes
/// differently — have no common centre, and the mean is the choice whose
/// arc gap never drops below the tighter leg pitch (nested circles: gap ≥
/// (r₂−r₁) − |ΔC| = mean − half the skew = the smaller offset). Every
/// radius also caps at the nearest crossing on its own legs, so a crossing
/// never lands mid-arc (an arc may land tangent exactly on one — the
/// perpendicular point contact is preserved). A capped radius only ever
/// *flares* a nested pair apart (the centres part toward the outside), so
/// rounding never brings two links nearer than their polylines' pitch.
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
            let mut ceil = in_len.min(out_len);
            for (wj, other) in polys.iter().enumerate() {
                if wj == wi {
                    continue;
                }
                for s in other.windows(2) {
                    for leg in [[a, v], [v, b]] {
                        if let Some(at) = crate::routing::cross(&leg, s) {
                            let t = (at.0 - v.0).abs() + (at.1 - v.1).abs();
                            ceil = ceil.min(t);
                        }
                    }
                }
            }
            corners.push(Corner {
                link: wi,
                slot: k - 1,
                quad,
                v,
                proj: v.0 * quad.0 as f64 + v.1 * quad.1 as f64,
                ceil,
                cap: caps[wi],
            });
        }
    }
    // Walk each quadrant innermost-out by projection; every corner chains
    // to the nearest inner corner it nests on — offset outward on both
    // axes, at lane scale (a far-apart pair is coincidence, not nesting; a
    // skipped lane still nests). Two independent nests interleaved along
    // the projection stay independent: the backward scan skips corners
    // whose offset is one-sided.
    corners.sort_by(|a, b| {
        a.quad
            .cmp(&b.quad)
            .then(b.proj.total_cmp(&a.proj))
            .then(a.link.cmp(&b.link))
            .then(a.slot.cmp(&b.slot))
    });
    // Outward per-axis offsets from inner `p` to outer `c` — positive when
    // `c` sits past `p` away from the arc-centre side on that axis.
    let off = |p: &Corner, c: &Corner| {
        (
            (c.v.0 - p.v.0) * -f64::from(p.quad.0),
            (c.v.1 - p.v.1) * -f64::from(p.quad.1),
        )
    };
    let mut i = 0;
    while i < corners.len() {
        let mut j = i + 1;
        while j < corners.len() && corners[j].quad == corners[i].quad {
            j += 1;
        }
        let mut radii: Vec<f64> = Vec::with_capacity(j - i);
        for k in i..j {
            let c = &corners[k];
            let parent = (i..k).rev().find(|&p| {
                let (u, w) = off(&corners[p], c);
                u > EPS && w > EPS && (u + w) / 2.0 <= 2.0 * c.cap.max(corners[p].cap) + EPS
            });
            let r = match parent {
                Some(p) => {
                    let (u, w) = off(&corners[p], c);
                    (radii[p - i] + (u + w) / 2.0).min(c.ceil)
                }
                None => c.cap.min(c.ceil),
            };
            radii.push(r);
            out[c.link][c.slot] = r;
        }
        i = j;
    }
    out
}

/// A link marker's tip, nudged [`MARKER_OVERLAP`] past the endpoint into the
/// shape so the head reads as connected (`dir` points into the shape).
fn overlap_tip(tip: (f64, f64), dir: (f64, f64)) -> (f64, f64) {
    (
        tip.0 + dir.0 * MARKER_OVERLAP,
        tip.1 + dir.1 * MARKER_OVERLAP,
    )
}

/// One label's cut box — the rect the mask punches out beneath it, and the
/// rect other wires test against. One computation, so a label cuts every
/// wire identically.
pub(super) fn cut_rect(t: &RoutedText) -> (f64, f64, f64, f64) {
    let size = t.attrs.number("font-size").unwrap_or(0.0);
    let ls = t.attrs.number("letter-spacing").unwrap_or(0.0);
    let lsp = t.attrs.number("line-spacing").unwrap_or(0.0);
    let font = crate::font::Font::of(&t.attrs);
    let cw = approx_width(&t.content, font, size, ls) + size * LABEL_CUT_PAD_H * 2.0;
    let ch = approx_height(&t.content, size, lsp) + size * LABEL_CUT_PAD_V * 2.0;
    let (cx, cy) = t.position;
    (cx - cw / 2.0, cy - ch / 2.0, cw, ch)
}

/// A luminance mask that cuts the link path out under **every** label box the
/// path reaches — its own and any other statement's (a fan sibling's arc can
/// sweep beneath a label seated on its twin) — the background-independent
/// replacement for a painted halo. White shows the path (over its stroked
/// bounds); a black box per label punches a hole; a box the path never enters
/// is a mask no-op, so the padded-bbox filter only trims noise. An explicit
/// `userSpaceOnUse` region is required, else a straight link's near-flat bbox
/// would shrink the default region to nothing and hide the whole link. `None`
/// when no label box reaches the path.
fn label_mask(
    idx: usize,
    path: &[(f64, f64)],
    cuts: &[(f64, f64, f64, f64)],
    thickness: f64,
    reach: f64,
) -> Option<(String, String)> {
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
    let hits: Vec<&(f64, f64, f64, f64)> = cuts
        .iter()
        .filter(|(cx, cy, cw, ch)| {
            cx + cw >= rx && *cx <= rx + rw && cy + ch >= ry && *cy <= ry + rh
        })
        .collect();
    if hits.is_empty() {
        return None;
    }
    let id = format!("lini-label-cut-{idx}");
    // The mask rects carry their fill/stroke via CSS (`.lini-cut-bg` /
    // `.lini-cut`), not inline — so the link's own `stroke` can't bleed into the
    // luminance mask, and the SVG stays free of per-label paint [SPEC 17].
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
    for (cx, cy, cw, ch) in hits {
        write!(
            m,
            r#"<rect class="lini-cut" x="{}" y="{}" width="{}" height="{}"/>"#,
            num(*cx),
            num(*cy),
            num(*cw),
            num(*ch),
        )
        .unwrap();
    }
    m.push_str("</mask>");
    Some((id, m))
}

/// A link label. The constant paint (`fill: currentColor`, `stroke: none` so the
/// glyphs don't inherit the link `<g>`'s stroke, the anchor pair, the baked font
/// size) rides the label's own class (`t.class` — `.lini-link-label` on a diagram
/// wire, `.lini-sequence-message` above a sequence arrow); only a label that
/// overrides one of those inlines the difference via `style=`.
fn render_link_text(
    out: &mut String,
    t: &RoutedText,
    ruleset: &RuleSet,
    vars: &VarTable,
    opts: &Options,
    sink: &super::fonts::FontSink,
) {
    // A label's paint is the same class-diff a node's text leaf is [SPEC 17],
    // against its own role rule (`.lini-link-label` / `.lini-sequence-message`) —
    // so `text-shadow` and every other font/paint prop ride through, and the
    // per-role default size states once in the sheet, not on each label.
    let classes = vec![t.class.to_string()];
    let style_attr = super::text_paint_attr(&t.attrs, &classes, ruleset, vars, opts);
    // The label rides its `along:` point already shifted by `translate` (folded in
    // at routing — `links::labels`), so the shared emitter handles only `rotate`,
    // multi-line, and `letter-spacing` — one code path with a node's text leaf.
    super::text::emit(
        out,
        "      ",
        &classes,
        &t.content,
        t.position,
        &t.attrs,
        &style_attr,
        ruleset,
        opts,
        sink,
    );
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
        // Three links turning together at lane pitch 8: vertices step
        // outward along the (+1,−1) diagonal (centre quadrant (−1,+1)).
        let (a, b, c) = (
            ell((0.0, 0.0), 100.0),
            ell((8.0, -8.0), 100.0),
            ell((16.0, -16.0), 100.0),
        );
        let t = fillet_targets(&[&a, &b, &c], &[8.0; 3]);
        assert_eq!((t[0][0], t[1][0], t[2][0]), (8.0, 16.0, 24.0));
    }

    /// The S-bend bus (a pcb-style flash → mcu): four wires at pitch 10 turn
    /// two nested corners each. Concentric radii grow to 4× clearance on the
    /// outermost track — the legs *jointly* fit every pair (40+10 on a
    /// 56-long shared leg), so no half-leg squash may flatten the nest.
    #[test]
    fn a_bus_s_bend_keeps_full_concentric_radii() {
        let wires: Vec<Vec<(f64, f64)>> = (0..4)
            .map(|k| {
                let (port, dive, land) = (
                    10.0 * k as f64,
                    94.0 - 10.0 * k as f64,
                    56.0 + 10.0 * k as f64,
                );
                vec![(0.0, port), (dive, port), (dive, land), (110.0, land)]
            })
            .collect();
        let polys: Vec<&[(f64, f64)]> = wires.iter().map(|w| &w[..]).collect();
        let t = fillet_targets(&polys, &[10.0; 4]);
        assert_eq!(
            t,
            vec![
                vec![40.0, 10.0],
                vec![30.0, 20.0],
                vec![20.0, 30.0],
                vec![10.0, 40.0]
            ]
        );
    }

    #[test]
    fn opposite_travel_still_nests() {
        // The outer link traverses the same corner the other way
        // (−y then −x): same arc quadrant, same shared centre.
        let a = ell((0.0, 0.0), 100.0);
        let b = vec![(8.0, 92.0), (8.0, -8.0), (-92.0, -8.0)];
        let t = fillet_targets(&[&a, &b], &[8.0; 2]);
        assert_eq!((t[0][0], t[1][0]), (8.0, 16.0));
    }

    /// Two relief groups can compress the two axes differently (links_hard:
    /// a V corridor at pitch 8, the port ladder at 10): the corners then sit
    /// on a skewed diagonal, but they still turn together and must still
    /// nest — the radius grows by the mean offset, which keeps the arc gap
    /// no smaller than the tighter leg pitch (nested circles: gap ≥
    /// (r₂−r₁) − |ΔC| = mean − half the skew = the smaller offset).
    #[test]
    fn asymmetric_lane_pitches_nest_by_the_mean_offset() {
        let (a, b, c) = (
            ell((0.0, 0.0), 100.0),
            ell((8.0, -10.0), 100.0),
            ell((16.0, -20.0), 100.0),
        );
        let t = fillet_targets(&[&a, &b, &c], &[8.0; 3]);
        assert_eq!((t[0][0], t[1][0], t[2][0]), (8.0, 17.0, 26.0));
    }

    #[test]
    fn a_far_corner_on_the_same_diagonal_is_not_nested() {
        let (a, b) = (ell((0.0, 0.0), 100.0), ell((80.0, -80.0), 100.0));
        let t = fillet_targets(&[&a, &b], &[8.0; 2]);
        assert_eq!((t[0][0], t[1][0]), (8.0, 8.0));
    }

    #[test]
    fn a_crossing_on_a_leg_caps_the_radius() {
        // A vertical link crosses the corner's incoming leg 5 before the
        // vertex: the arc must land tangent at the crossing, never past it.
        let a = ell((0.0, 0.0), 100.0);
        let b = vec![(-5.0, -50.0), (-5.0, 50.0)];
        let t = fillet_targets(&[&a, &b], &[8.0; 2]);
        assert_eq!(t[0][0], 5.0);
    }

    #[test]
    fn float_dust_on_the_diagonal_never_reorders_a_nest() {
        // The links_hard hub fan: three corners whose diagonal coordinates
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
        // The middle link's outgoing leg holds only r = 12: it caps there,
        // and the outer corner keeps stepping from the capped value.
        let (a, b, c) = (
            ell((0.0, 0.0), 100.0),
            vec![(-92.0, -8.0), (8.0, -8.0), (8.0, 4.0)],
            ell((16.0, -16.0), 100.0),
        );
        let t = fillet_targets(&[&a, &b, &c], &[8.0; 3]);
        assert_eq!((t[0][0], t[1][0], t[2][0]), (8.0, 12.0, 20.0));
    }
}

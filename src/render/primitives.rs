//! Per-primitive SVG geometry. One emitter per `NodeKind`; most produce a
//! single SVG element, `cyl` and `cloud` build a small composition.
//!
//! Geometry only: paint (fill, stroke, widths, dash) lives on the node's
//! `<g>` — provided by the stylesheet's class rules or the node's `style=`
//! diff — and reaches these elements through SVG inheritance. The icon
//! placeholder is the one exception: a composite whose two elements need
//! different paint, stated per element (element-level presentation attrs
//! only fight inheritance, which they win — no class rule matches them).

use super::filters::FilterTable;
use super::rules::{RuleSet, effective_stroke};
use super::values::{attr_or_var, attr_points, class_list, escape_xml, num};
use crate::Options;
use crate::layout::PlacedNode;
use crate::resolve::{MarkerKind, NodeKind, ResolvedValue, VarTable};
use std::fmt::Write;

pub fn render_geometry(
    out: &mut String,
    n: &PlacedNode,
    depth: usize,
    vars: &VarTable,
    ruleset: &RuleSet,
    filters: &FilterTable,
    opts: &Options,
) {
    // A drop shadow wraps the geometry only — never the label — so text on a
    // shadowed card stays crisp.
    let shadow = filters.id_for(n, vars, opts);
    let body_depth = match &shadow {
        Some(id) => {
            writeln!(out, r#"{}<g filter="url(#{})">"#, "  ".repeat(depth), id).unwrap();
            depth + 1
        }
        None => depth,
    };

    // `stack:` draws an offset copy behind the shape (SPEC §7); both copies
    // sit inside the shadow group, so the stacked silhouette casts one shadow.
    if let Some((dx, dy)) = stack_offset(n) {
        let indent = "  ".repeat(body_depth);
        writeln!(
            out,
            r#"{}<g transform="translate({},{})">"#,
            indent,
            num(dx),
            num(dy)
        )
        .unwrap();
        emit_shape(out, n, body_depth + 1, vars, ruleset, opts);
        writeln!(out, "{}</g>", indent).unwrap();
    }

    emit_shape(out, n, body_depth, vars, ruleset, opts);

    if shadow.is_some() {
        writeln!(out, "{}</g>", "  ".repeat(depth)).unwrap();
    }

    // Interior gutters, each a `<rect>` filled with the container's resolved
    // `gap-color` (SPEC §5) — drawn over the shape, interior only, so the outer
    // frame stays the container's own border and no edge is doubled. The rect
    // states its own `stroke="none"`, so the container's border never bleeds onto
    // it, and a filled rect (unlike a `<line>`) carries a gradient `gap-color`.
    // Layout emits gutters only when `gap-color` is set, so it is always a real paint.
    if !n.gutters.is_empty() {
        let indent = "  ".repeat(depth);
        let fill = n
            .attrs
            .get("gap-color")
            .map(|v| super::values::format_value(v, vars, opts))
            .unwrap_or_else(|| "none".to_string());
        for (cx, cy, w, h) in &n.gutters {
            writeln!(
                out,
                r#"{}<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="none"/>"#,
                indent,
                num(cx - w / 2.0),
                num(cy - h / 2.0),
                num(*w),
                num(*h),
                fill,
            )
            .unwrap();
        }
    }
}

/// The `(dx, dy)` of a `stack:` offset copy (SPEC §7): scalar `N` ⇒ `(N, -N)`.
fn stack_offset(n: &PlacedNode) -> Option<(f64, f64)> {
    match n.attrs.get("stack")? {
        ResolvedValue::Number(v) => Some((*v, -*v)),
        ResolvedValue::Tuple(items) if items.len() == 2 => {
            Some((items[0].as_number()?, items[1].as_number()?))
        }
        _ => None,
    }
}

fn emit_shape(
    out: &mut String,
    n: &PlacedNode,
    depth: usize,
    vars: &VarTable,
    ruleset: &RuleSet,
    opts: &Options,
) {
    let indent = "  ".repeat(depth);
    // Default matches the `stroke-width` layout var (SPEC §11.3) so the drawn
    // shape stays inside the bbox the layout reserved.
    let thickness = n.attrs.number("stroke-width").unwrap_or(0.0);

    match n.kind {
        NodeKind::Block => emit_rect(out, n, &indent, thickness),
        NodeKind::Slant => emit_slant(out, n, &indent, thickness),
        NodeKind::Hex => emit_hex(out, n, &indent, thickness),
        NodeKind::Diamond => emit_diamond(out, n, &indent, thickness),
        NodeKind::Cyl => emit_cyl(out, n, &indent, thickness),
        NodeKind::Oval => emit_oval(out, n, &indent, thickness),
        // Text is emitted by `render::render_text` as a bare `<text>` (SPEC §13),
        // never as wrapped geometry — so it never reaches this dispatch.
        NodeKind::Text => {}
        NodeKind::Line => emit_line(out, n, &indent, vars, ruleset, opts, thickness),
        NodeKind::Poly => emit_poly(out, n, &indent),
        NodeKind::Path => emit_path(out, n, &indent),
        NodeKind::Icon => emit_icon(out, n, &indent, vars, opts),
        NodeKind::Image => emit_image(out, n, &indent),
    }
}

fn dim_excluding_stroke(n: &PlacedNode, thickness: f64) -> (f64, f64) {
    let w = (n.bbox.w() - thickness).max(0.0);
    let h = (n.bbox.h() - thickness).max(0.0);
    (w, h)
}

fn emit_rect(out: &mut String, n: &PlacedNode, indent: &str, thickness: f64) {
    let (w, h) = dim_excluding_stroke(n, thickness);
    let radius = n.attrs.number("radius").unwrap_or(0.0);
    writeln!(
        out,
        r#"{}<rect x="{}" y="{}" width="{}" height="{}" rx="{}" ry="{}"/>"#,
        indent,
        num(-w / 2.0),
        num(-h / 2.0),
        num(w),
        num(h),
        num(radius),
        num(radius),
    )
    .unwrap();
}

fn emit_oval(out: &mut String, n: &PlacedNode, indent: &str, thickness: f64) {
    let (w, h) = dim_excluding_stroke(n, thickness);
    writeln!(
        out,
        r#"{}<ellipse cx="0" cy="0" rx="{}" ry="{}"/>"#,
        indent,
        num(w / 2.0),
        num(h / 2.0),
    )
    .unwrap();
}

fn emit_hex(out: &mut String, n: &PlacedNode, indent: &str, thickness: f64) {
    let (w, h) = dim_excluding_stroke(n, thickness);
    // Flat-top hex per SPEC §8. Two horizontal edges, four slanted edges.
    let pts = [
        (-w / 2.0, 0.0),
        (-w / 4.0, -h / 2.0),
        (w / 4.0, -h / 2.0),
        (w / 2.0, 0.0),
        (w / 4.0, h / 2.0),
        (-w / 4.0, h / 2.0),
    ];
    emit_polygon(out, indent, &pts);
}

fn emit_diamond(out: &mut String, n: &PlacedNode, indent: &str, thickness: f64) {
    let (w, h) = dim_excluding_stroke(n, thickness);
    let pts = [
        (0.0, -h / 2.0),
        (w / 2.0, 0.0),
        (0.0, h / 2.0),
        (-w / 2.0, 0.0),
    ];
    emit_polygon(out, indent, &pts);
}

fn emit_slant(out: &mut String, n: &PlacedNode, indent: &str, thickness: f64) {
    let (w, h) = dim_excluding_stroke(n, thickness);
    let skew_deg = n.attrs.number("skew").unwrap_or(0.0);
    let shift = (skew_deg.to_radians()).tan() * h / 2.0;
    let pts = [
        (-w / 2.0 + shift, -h / 2.0),
        (w / 2.0 + shift, -h / 2.0),
        (w / 2.0 - shift, h / 2.0),
        (-w / 2.0 - shift, h / 2.0),
    ];
    emit_polygon(out, indent, &pts);
}

fn emit_cyl(out: &mut String, n: &PlacedNode, indent: &str, thickness: f64) {
    // Cylinder = ellipse top + body rect + ellipse bottom. The bbox carries
    // total height; the ellipse rx == w/2, ry ≈ h/10.
    let (w, h) = dim_excluding_stroke(n, thickness);
    let rx = w / 2.0;
    let ry = (h / 10.0).max(2.0);
    let top_cy = -h / 2.0 + ry;
    let bottom_cy = h / 2.0 - ry;
    // Body as a path that draws sides + bottom arc (fill the cylinder).
    writeln!(
        out,
        r#"{}<path d="M {} {} L {} {} A {} {} 0 0 0 {} {} L {} {} A {} {} 0 0 0 {} {} Z"/>"#,
        indent,
        num(-rx),
        num(top_cy),
        num(-rx),
        num(bottom_cy),
        num(rx),
        num(ry),
        num(rx),
        num(bottom_cy),
        num(rx),
        num(top_cy),
        num(rx),
        num(ry),
        num(-rx),
        num(top_cy),
    )
    .unwrap();
    // Top ellipse rim (visible curve on top).
    writeln!(
        out,
        r#"{}<ellipse cx="0" cy="{}" rx="{}" ry="{}"/>"#,
        indent,
        num(top_cy),
        num(rx),
        num(ry),
    )
    .unwrap();
}

fn emit_line(
    out: &mut String,
    n: &PlacedNode,
    indent: &str,
    vars: &VarTable,
    ruleset: &RuleSet,
    opts: &Options,
    thickness: f64,
) {
    let points = attr_points(&n.attrs, "points").unwrap_or_default();
    if points.len() < 2 {
        return;
    }

    // Stop the drawn line short of its markers so the stroke doesn't poke through
    // the arrowhead; the markers still ride the true endpoints (below).
    let drawn = super::markers::shorten_for_markers(&points, &n.markers, thickness, 0.0);

    // A `stroke-style: wavy` line rides an undulating centreline (reusing the link
    // wave) rather than a dash pattern — an async sequence message (SPEC §10), or an
    // explicit wavy `|line|`. `wavy_d` returns `None` below one wavelength, falling
    // back to the straight forms.
    let wavy = matches!(n.attrs.get("stroke-style"), Some(ResolvedValue::Ident(s)) if s == "wavy");
    // `radius` rounds the interior corners of a multi-point line into quarter arcs,
    // reusing the link fillet formatter ([`super::rounding`]) — a sequence self-message
    // hook bends exactly like a routed wire (SPEC §10), and any `|line|` may round.
    let radius = n.attrs.number("radius").unwrap_or(0.0);
    if let Some(d) = wavy.then(|| super::wavy::wavy_d(&drawn, &[])).flatten() {
        writeln!(out, r#"{indent}<path d="{d}" fill="none"/>"#).unwrap();
    } else if drawn.len() == 2 {
        let (from, to) = (drawn[0], drawn[1]);
        writeln!(
            out,
            r#"{}<line x1="{}" y1="{}" x2="{}" y2="{}"/>"#,
            indent,
            num(from.0),
            num(from.1),
            num(to.0),
            num(to.1),
        )
        .unwrap();
    } else if radius > 0.5 {
        let targets = vec![radius; drawn.len() - 2];
        let d = super::rounding::path_d(&drawn, &targets);
        writeln!(out, r#"{indent}<path d="{d}" fill="none"/>"#).unwrap();
    } else {
        let pts: Vec<String> = drawn
            .iter()
            .map(|(x, y)| format!("{},{}", num(*x), num(*y)))
            .collect();
        writeln!(out, r#"{}<polyline points="{}"/>"#, indent, pts.join(" ")).unwrap();
    }

    // Markers at the first and last points. Their fill follows the line's
    // stroke from CSS (the `.lini-marker` base or a `.lini-style-* .lini-marker`
    // descendant rule), so only a direct inline `stroke:` inlines it; the crow
    // states the cascade-resolved colour regardless.
    let classes = class_list(n.kind.as_str(), &n.type_chain, &n.applied_styles);
    let color = effective_stroke(&n.attrs, &classes, ruleset, vars, opts);
    let paint = super::markers::MarkerPaint {
        color: &color,
        inline: ruleset.marker_fill(&classes) != Some(color.as_str()),
        thickness,
    };
    // A marker orients to its **own end segment**, not the line's overall direction:
    // a multi-point hook (a sequence self-message) turns back on itself, so an
    // end-to-end vector would point the head along the lifeline instead of into it.
    // The router resolves markers the same way (per-segment, [`super::links`]).
    let np = points.len();
    if n.markers.start != MarkerKind::None
        && let Some((tip, dir)) = super::markers::marker_anchor(points[1], points[0], false)
    {
        super::markers::emit_marker(out, indent, n.markers.start, tip, dir, &paint);
    }
    if n.markers.end != MarkerKind::None
        && let Some((tip, dir)) =
            super::markers::marker_anchor(points[np - 2], points[np - 1], false)
    {
        super::markers::emit_marker(out, indent, n.markers.end, tip, dir, &paint);
    }
}

fn emit_poly(out: &mut String, n: &PlacedNode, indent: &str) {
    let points = attr_points(&n.attrs, "points").unwrap_or_default();
    emit_polygon(out, indent, &points);
}

fn emit_path(out: &mut String, n: &PlacedNode, indent: &str) {
    let d = match n.attrs.get("path") {
        Some(crate::resolve::ResolvedValue::String(s)) => s.clone(),
        _ => return,
    };
    writeln!(out, r#"{}<path d="{}"/>"#, indent, escape_xml(&d)).unwrap();
}

fn emit_icon(out: &mut String, n: &PlacedNode, indent: &str, vars: &VarTable, opts: &Options) {
    // The `symbol` (validated at resolve) names a Phosphor glyph: a list of
    // geometry fragments, each tagged with a paint role. They are authored on a
    // 256-unit grid and scaled into the node box; the stroke is counter-scaled so
    // its weight stays constant at any size and matches other strokes (SPEC §7).
    let Some(ResolvedValue::Ident(name) | ResolvedValue::String(name)) = n.attrs.get("symbol")
    else {
        return;
    };
    let Some(frags) = crate::icon::lookup(name) else {
        return;
    };
    let frags: Vec<(crate::icon::Role, &str)> = frags.collect();

    // `fit` picks the content rectangle to map into the box (SPEC §10): the whole
    // 256-grid for `auto` (Phosphor's authored margin), else the glyph's own
    // extent. `contain`/`auto` fit inside (min scale), `cover` covers (max),
    // `stretch` fills both axes independently.
    use super::icon_fit::Fit;
    let (bw, bh) = (n.bbox.w(), n.bbox.h());
    let (cx, cy, cw, ch) = match Fit::of(&n.attrs) {
        Fit::Auto => (128.0, 128.0, 256.0, 256.0),
        _ => super::icon_fit::glyph_box(&frags).unwrap_or((128.0, 128.0, 256.0, 256.0)),
    };
    let (sx, sy) = match Fit::of(&n.attrs) {
        Fit::Cover => {
            let s = (bw / cw).max(bh / ch);
            (s, s)
        }
        Fit::Stretch => (bw / cw, bh / ch),
        _ => {
            let s = (bw / cw).min(bh / ch);
            (s, s)
        }
    };
    let body = attr_or_var(&n.attrs, "fill", "icon-fill", vars, opts);
    let ink = attr_or_var(&n.attrs, "stroke", "stroke", vars, opts);
    // Counter-scale the stroke by the geometric-mean scale so a 2px line stays 2px
    // at any size or fit (uniform fits have `sx == sy`, so it is just the scale).
    let stroke_width = n.attrs.number("stroke-width").unwrap_or(2.0) / (sx * sy).sqrt();

    use crate::icon::Role;
    let scale = if (sx - sy).abs() < 1e-9 {
        num(sx)
    } else {
        format!("{} {}", num(sx), num(sy))
    };
    writeln!(
        out,
        r#"{indent}<g transform="scale({scale}) translate({} {})">"#,
        num(-cx),
        num(-cy),
    )
    .unwrap();
    // Faint body behind, then the outline, then any solid ink on top. A
    // `fill: none` drops the body, leaving a clean single-tone line icon.
    if body != "none" {
        emit_role_group(
            out,
            indent,
            &frags,
            |r| matches!(r, Role::Fill | Role::Both),
            &format!(r#"fill="{body}" stroke="none""#),
        );
    }
    emit_role_group(
        out,
        indent,
        &frags,
        |r| matches!(r, Role::Line | Role::Both),
        &format!(
            r#"fill="none" stroke="{ink}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round""#,
            num(stroke_width)
        ),
    );
    emit_role_group(
        out,
        indent,
        &frags,
        |r| r == Role::Solid,
        &format!(r#"fill="{ink}" stroke="none""#),
    );
    writeln!(out, "{indent}</g>").unwrap();
    // The optional label is an ordinary centred-text child (SPEC §7), drawn by the
    // shared text emitter via the normal child recursion — so it inherits the
    // colour / `font-size`, never scales with the glyph, and honours `translate` /
    // `rotate` / styling exactly like any node's text.
}

/// Emit the fragments whose role matches `want`, wrapped in one inheriting paint
/// `<g>`; nothing when none match (so an all-`fill: none` icon emits no body).
fn emit_role_group(
    out: &mut String,
    indent: &str,
    frags: &[(crate::icon::Role, &str)],
    want: impl Fn(crate::icon::Role) -> bool,
    paint: &str,
) {
    if !frags.iter().any(|&(r, _)| want(r)) {
        return;
    }
    writeln!(out, "{indent}  <g {paint}>").unwrap();
    for &(_, frag) in frags.iter().filter(|&&(r, _)| want(r)) {
        writeln!(out, "{indent}    {frag}").unwrap();
    }
    writeln!(out, "{indent}  </g>").unwrap();
}

fn emit_image(out: &mut String, n: &PlacedNode, indent: &str) {
    let href = match n.attrs.get("src") {
        Some(crate::resolve::ResolvedValue::String(s)) => s.clone(),
        _ => return,
    };
    // Image dimensions come from its bbox (driven by `width`/`height`). `fit` maps
    // to `preserveAspectRatio` (SPEC §10); `auto`/`contain` is the SVG default
    // (`xMidYMid meet`), so only `cover`/`stretch` need stating.
    let w = n.bbox.w();
    let h = n.bbox.h();
    let par = match super::icon_fit::Fit::of(&n.attrs) {
        super::icon_fit::Fit::Cover => r#" preserveAspectRatio="xMidYMid slice""#,
        super::icon_fit::Fit::Stretch => r#" preserveAspectRatio="none""#,
        _ => "",
    };
    writeln!(
        out,
        r#"{}<image href="{}" x="{}" y="{}" width="{}" height="{}"{}/>"#,
        indent,
        escape_xml(&href),
        num(-w / 2.0),
        num(-h / 2.0),
        num(w),
        num(h),
        par,
    )
    .unwrap();
}

fn emit_polygon(out: &mut String, indent: &str, points: &[(f64, f64)]) {
    let pts_str: Vec<String> = points
        .iter()
        .map(|(x, y)| format!("{},{}", num(*x), num(*y)))
        .collect();
    writeln!(
        out,
        r#"{}<polygon points="{}"/>"#,
        indent,
        pts_str.join(" ")
    )
    .unwrap();
}

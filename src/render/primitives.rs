//! Per-primitive SVG geometry. One emitter per `ShapeKind`; most produce a
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
use crate::resolve::{ResolvedValue, ShapeKind, VarTable};
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

    // Interior dividers as one path, painted by the container's stroke through
    // inheritance — drawn over the shape so border and inner lines share one
    // colour, never doubled (the container owns its rules; cells stay
    // borderless).
    if !n.dividers.is_empty() {
        let indent = "  ".repeat(depth);
        let d: Vec<String> = n
            .dividers
            .iter()
            .map(|(x1, y1, x2, y2)| {
                format!("M {} {} L {} {}", num(*x1), num(*y1), num(*x2), num(*y2))
            })
            .collect();
        writeln!(out, r#"{}<path d="{}" fill="none"/>"#, indent, d.join(" ")).unwrap();
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

    match n.shape {
        ShapeKind::Block => emit_rect(out, n, &indent, thickness),
        ShapeKind::Slant => emit_slant(out, n, &indent, thickness),
        ShapeKind::Hex => emit_hex(out, n, &indent, thickness),
        ShapeKind::Diamond => emit_diamond(out, n, &indent, thickness),
        ShapeKind::Cyl => emit_cyl(out, n, &indent, thickness),
        ShapeKind::Cloud => emit_cloud(out, n, &indent, thickness),
        ShapeKind::Oval => emit_oval(out, n, &indent, thickness),
        // Text is emitted by `render::render_text` as a bare `<text>` (SPEC §13),
        // never as wrapped geometry — so it never reaches this dispatch.
        ShapeKind::Text => {}
        ShapeKind::Line => emit_line(out, n, &indent, vars, ruleset, opts, thickness),
        ShapeKind::Poly => emit_poly(out, n, &indent),
        ShapeKind::Path => emit_path(out, n, &indent),
        ShapeKind::Icon => emit_icon(out, n, &indent, vars, opts),
        ShapeKind::Image => emit_image(out, n, &indent),
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

fn emit_cloud(out: &mut String, n: &PlacedNode, indent: &str, thickness: f64) {
    // Minimal cloud outline (after Inkscape "cloudSimple"): a small left lobe, a
    // larger right lobe, and a flat bottom with rounded corners. Reference
    // coordinates (their own bbox 42.77..158.47 × 114.33..180.02) are normalized
    // to the node bbox by point — never a non-uniform transform — so the stroke
    // stays an even width at any size.
    let (w, h) = dim_excluding_stroke(n, thickness);
    let p = |x: f64, y: f64| {
        let nx = (x - 42.768) / 115.706;
        let ny = (y - 114.328) / 65.689;
        fmt_pt((nx * w - w / 2.0, ny * h - h / 2.0))
    };
    // 11 cubic segments as (c1x, c1y, c2x, c2y, endx, endy). The flat bottom is a
    // straight line inserted between the 7th and 8th segment.
    const SEGS: [[f64; 6]; 11] = [
        [107.942, 114.332, 98.406, 120.038, 93.496, 129.180],
        [90.025, 126.557, 85.796, 125.134, 81.446, 125.125],
        [70.373, 125.125, 61.397, 134.101, 61.397, 145.174],
        [61.401, 145.385, 61.408, 145.595, 61.419, 145.805],
        [60.918, 145.758, 60.416, 145.733, 59.913, 145.730],
        [50.444, 145.730, 42.768, 153.405, 42.769, 162.874],
        [42.769, 172.342, 50.444, 180.017, 59.913, 180.017],
        [150.798, 180.017, 158.473, 172.342, 158.474, 162.874],
        [158.470, 155.286, 153.478, 148.603, 146.203, 146.446],
        [146.397, 145.146, 146.500, 143.834, 146.511, 142.519],
        [146.511, 126.949, 133.889, 114.328, 118.320, 114.328],
    ];
    let mut d = format!("M {}", p(118.320, 114.328));
    for (i, s) in SEGS.iter().enumerate() {
        if i == 7 {
            d.push_str(&format!(" L {}", p(141.330, 180.017)));
        }
        d.push_str(&format!(
            " C {} {} {}",
            p(s[0], s[1]),
            p(s[2], s[3]),
            p(s[4], s[5])
        ));
    }
    d.push_str(" Z");
    writeln!(out, r#"{}<path d="{}"/>"#, indent, d).unwrap();
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

    // 2 points → SVG <line>; 3+ → SVG <polyline>.
    if drawn.len() == 2 {
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
    let classes = class_list(n.shape.as_str(), &n.type_chain, &n.applied_styles);
    let color = effective_stroke(&n.attrs, &classes, ruleset, vars, opts);
    let paint = super::markers::MarkerPaint {
        color: &color,
        inline: ruleset.marker_fill(&classes) != Some(color.as_str()),
        thickness,
    };
    let from = points[0];
    let to = points[points.len() - 1];
    super::markers::emit_inline_markers(out, indent, n, from, to, &paint);
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
    // Material Symbols embedding lands in a follow-up; until then a
    // placeholder square keeps layout visible and the icon's name
    // discoverable through the SVG. The glyph name is the node's label
    // (`|icon| "home"`, SPEC §7); size comes from `width`/`height`.
    let size = n
        .attrs
        .number("width")
        .or_else(|| n.attrs.number("height"))
        .unwrap_or(0.0);
    let name = n.label.as_deref().unwrap_or("?");
    let stroke = attr_or_var(&n.attrs, "stroke", "stroke", vars, opts);
    let fill = attr_or_var(&n.attrs, "fill", "stroke", vars, opts);
    writeln!(
        out,
        r#"{}<rect x="{}" y="{}" width="{}" height="{}" fill="none" stroke="{}" stroke-width="1"/>"#,
        indent,
        num(-size / 2.0),
        num(-size / 2.0),
        num(size),
        num(size),
        stroke,
    )
    .unwrap();
    writeln!(
        out,
        r#"{}<text x="0" y="0" text-anchor="middle" dominant-baseline="central" font-size="{}" fill="{}">{}</text>"#,
        indent,
        num(size * 0.4),
        fill,
        escape_xml(name),
    )
    .unwrap();
}

fn emit_image(out: &mut String, n: &PlacedNode, indent: &str) {
    let href = match n.attrs.get("src") {
        Some(crate::resolve::ResolvedValue::String(s)) => s.clone(),
        _ => return,
    };
    // Image dimensions come from its bbox (driven by `width`/`height`).
    let w = n.bbox.w();
    let h = n.bbox.h();
    writeln!(
        out,
        r#"{}<image href="{}" x="{}" y="{}" width="{}" height="{}"/>"#,
        indent,
        escape_xml(&href),
        num(-w / 2.0),
        num(-h / 2.0),
        num(w),
        num(h),
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

fn fmt_pt((x, y): (f64, f64)) -> String {
    format!("{} {}", num(x), num(y))
}

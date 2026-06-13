//! Per-primitive SVG geometry. One emitter per `ShapeKind`; most produce a
//! single SVG element, `cyl` and `cloud` build a small composition.
//!
//! Geometry only: paint (fill, stroke, widths, dash) lives on the node's
//! `<g>` — provided by the stylesheet's class rules or the node's `style=`
//! diff — and reaches these elements through SVG inheritance. The icon
//! placeholder is the one exception: a composite whose two elements need
//! different paint, stated per element (element-level presentation attrs
//! only fight inheritance, which they win — no class rule matches them).

use super::values::{attr_or_var, attr_points, attr_str, escape_xml, num};
use crate::Options;
use crate::layout::PlacedNode;
use crate::resolve::{ShapeKind, VarTable};
use std::fmt::Write;

pub fn render_geometry(
    out: &mut String,
    n: &PlacedNode,
    depth: usize,
    vars: &VarTable,
    opts: &Options,
) {
    let indent = "  ".repeat(depth);
    let thickness = n.attrs.number("thickness").unwrap_or(1.0);

    match n.shape {
        ShapeKind::Rect => emit_rect(out, n, &indent, thickness),
        ShapeKind::Slant => emit_slant(out, n, &indent, thickness),
        ShapeKind::Hex => emit_hex(out, n, &indent, thickness),
        ShapeKind::Diamond => emit_diamond(out, n, &indent, thickness),
        ShapeKind::Cyl => emit_cyl(out, n, &indent, thickness),
        ShapeKind::Cloud => emit_cloud(out, n, &indent, thickness),
        ShapeKind::Oval => emit_oval(out, n, &indent, thickness),
        ShapeKind::Text => emit_text(out, n, &indent),
        ShapeKind::Line => emit_line(out, n, &indent, vars, opts, thickness),
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
    let skew_deg = n.attrs.number("skew").unwrap_or(15.0);
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
    // Stylized cloud. Reference path is sized for 100 × 60; scale to bbox.
    let (w, h) = dim_excluding_stroke(n, thickness);
    let sx = w / 100.0;
    let sy = h / 60.0;
    let pt = |x: f64, y: f64| (x * sx - w / 2.0, y * sy - h / 2.0);
    let d = format!(
        "M {a} Q {b} {c} Q {d} {e} Q {f} {g} Q {h} {i} Q {j} {k} Q {l} {m} Z",
        a = fmt_pt(pt(25.0, 60.0)),
        b = fmt_pt(pt(5.0, 60.0)),
        c = fmt_pt(pt(5.0, 40.0)),
        d = fmt_pt(pt(5.0, 20.0)),
        e = fmt_pt(pt(25.0, 25.0)),
        f = fmt_pt(pt(30.0, 5.0)),
        g = fmt_pt(pt(55.0, 15.0)),
        h = fmt_pt(pt(75.0, 5.0)),
        i = fmt_pt(pt(75.0, 25.0)),
        j = fmt_pt(pt(95.0, 30.0)),
        k = fmt_pt(pt(95.0, 50.0)),
        l = fmt_pt(pt(95.0, 70.0)),
        m = fmt_pt(pt(75.0, 60.0)),
    );
    writeln!(out, r#"{}<path d="{}"/>"#, indent, d).unwrap();
}

fn emit_text(out: &mut String, n: &PlacedNode, indent: &str) {
    // Paint, font, and the anchor pair all ride the wrapping <g> (class rules
    // + style= diff) and reach the element by inheritance.
    let label = n.label.as_deref().unwrap_or("");
    let lines: Vec<&str> = label.split('\n').collect();
    if lines.len() <= 1 {
        writeln!(
            out,
            r#"{}<text x="0" y="0">{}</text>"#,
            indent,
            escape_xml(label)
        )
        .unwrap();
        return;
    }
    // Multi-line (SPEC §5): one tspan per line, spacing `text-size × 1.2`, the
    // block centred on the origin so `dominant-baseline:central` still holds.
    let size = n.attrs.number("text-size").unwrap_or(13.0);
    let spacing = size * 1.2;
    let top = -spacing * (lines.len() as f64 - 1.0) / 2.0;
    write!(out, r#"{}<text x="0" y="0">"#, indent).unwrap();
    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            write!(
                out,
                r#"<tspan x="0" y="{}">{}</tspan>"#,
                num(top),
                escape_xml(line)
            )
            .unwrap();
        } else {
            write!(
                out,
                r#"<tspan x="0" dy="{}">{}</tspan>"#,
                num(spacing),
                escape_xml(line)
            )
            .unwrap();
        }
    }
    writeln!(out, "</text>").unwrap();
}

fn emit_line(
    out: &mut String,
    n: &PlacedNode,
    indent: &str,
    vars: &VarTable,
    opts: &Options,
    thickness: f64,
) {
    let points = attr_points(&n.attrs, "points").unwrap_or_default();
    if points.len() < 2 {
        return;
    }

    // 2 points → SVG <line>; 3+ → SVG <polyline>.
    if points.len() == 2 {
        let (from, to) = (points[0], points[1]);
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
        let pts: Vec<String> = points
            .iter()
            .map(|(x, y)| format!("{},{}", num(*x), num(*y)))
            .collect();
        writeln!(out, r#"{}<polyline points="{}"/>"#, indent, pts.join(" ")).unwrap();
    }

    // Markers go at the first and last points, filled per the line's stroke
    // (explicit per element — markers must not pick up the dash pattern or
    // fill:none the line's rules provide).
    let stroke = attr_or_var(&n.attrs, "stroke", "stroke", vars, opts);
    let from = points[0];
    let to = points[points.len() - 1];
    super::markers::emit_inline_markers(out, indent, n, from, to, &stroke, thickness);
}

fn emit_poly(out: &mut String, n: &PlacedNode, indent: &str) {
    let points = attr_points(&n.attrs, "points").unwrap_or_default();
    emit_polygon(out, indent, &points);
}

fn emit_path(out: &mut String, n: &PlacedNode, indent: &str) {
    let d = match n.attrs.get("d") {
        Some(crate::resolve::ResolvedValue::String(s)) => s.clone(),
        _ => return,
    };
    writeln!(out, r#"{}<path d="{}"/>"#, indent, escape_xml(&d)).unwrap();
}

fn emit_icon(out: &mut String, n: &PlacedNode, indent: &str, vars: &VarTable, opts: &Options) {
    // Material Symbols embedding lands in a follow-up; until then a
    // placeholder square keeps layout visible and the icon's name
    // discoverable through the SVG.
    let size = n.attrs.number("size").unwrap_or(24.0);
    let name = attr_str(&n.attrs, "name", "?", vars, opts);
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
        escape_xml(&name),
    )
    .unwrap();
}

fn emit_image(out: &mut String, n: &PlacedNode, indent: &str) {
    let href = match n.attrs.get("href") {
        Some(crate::resolve::ResolvedValue::String(s)) => s.clone(),
        _ => return,
    };
    // Image dimensions come from its bbox (driven by `size=`).
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

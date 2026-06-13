pub(crate) mod markers; // `marker_size` is read by the router to reserve stub room
mod primitives;
mod rules;
mod style_block;
mod values;
mod wires;

use crate::Options;
use crate::layout::{LaidOut, PlacedNode};
use crate::resolve::{ShapeKind, VarTable};
use rules::RuleSet;
use values::{escape_xml, format_value, num};

pub fn render(laid_out: &LaidOut, opts: &Options) -> String {
    let mut out = String::with_capacity(2048);
    let vb = &laid_out.viewbox;

    use std::fmt::Write;
    writeln!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" width="{}" height="{}" class="lini">"#,
        num(vb.x),
        num(vb.y),
        num(vb.w),
        num(vb.h),
        num(vb.w),
        num(vb.h),
    )
    .unwrap();

    // The stylesheet's structural rules state every paint default once; the
    // root `.lini` rule seeds the inherited text properties (font, size,
    // color) for the whole tree. Per-node differences ride `style=`.
    let ruleset = rules::build(laid_out, opts);
    style_block::emit(&mut out, &laid_out.vars, &ruleset, opts);
    out.push_str("  <defs/>\n");

    out.push_str("  <g class=\"lini-scene\">\n");
    for node in &laid_out.nodes {
        render_node(&mut out, node, 2, &laid_out.vars, &ruleset, opts);
    }
    out.push_str("  </g>\n");

    if laid_out.wires.is_empty() && laid_out.airwires.is_empty() {
        out.push_str("  <g class=\"lini-wires\"/>\n");
    } else {
        out.push_str("  <g class=\"lini-wires\">\n");
        let polys: Vec<&[(f64, f64)]> = laid_out.wires.iter().map(|w| &w.path[..]).collect();
        let caps: Vec<f64> = laid_out
            .wires
            .iter()
            .map(|w| wires::radius_cap(w, &laid_out.vars))
            .collect();
        let targets = wires::fillet_targets(&polys, &caps);
        for (wire, targets) in laid_out.wires.iter().zip(&targets) {
            wires::render_wire(&mut out, wire, targets, &laid_out.vars, &ruleset, opts);
        }
        for air in &laid_out.airwires {
            wires::render_airwire(&mut out, air, &laid_out.vars, opts);
        }
        out.push_str("  </g>\n");
    }

    out.push_str("</svg>\n");
    out
}

fn render_node(
    out: &mut String,
    n: &PlacedNode,
    depth: usize,
    vars: &VarTable,
    ruleset: &RuleSet,
    opts: &Options,
) {
    use std::fmt::Write;
    let indent = "  ".repeat(depth);
    let class_list = values::class_list(n.shape.as_str(), &n.type_chain, &n.applied_styles);
    let classes = class_list.join(" ");
    let transform = if n.rotation != 0.0 {
        format!(
            r#" transform="translate({},{}) rotate({})""#,
            num(n.cx),
            num(n.cy),
            num(n.rotation)
        )
    } else {
        format!(r#" transform="translate({},{})""#, num(n.cx), num(n.cy))
    };
    let id_attr = match &n.id {
        Some(id) => format!(r#" data-id="{}""#, escape_xml(id)),
        None => String::new(),
    };
    let style_attr = node_style_attr(n, &class_list, ruleset, vars, opts);
    writeln!(
        out,
        r#"{}<g class="{}"{}{}{}>"#,
        indent, classes, id_attr, style_attr, transform
    )
    .unwrap();

    primitives::render_geometry(out, n, depth + 1, vars, opts);
    for child in &n.children {
        render_node(out, child, depth + 1, vars, ruleset, opts);
    }

    writeln!(out, "{}</g>", indent).unwrap();
}

/// The node's paint, as the difference against what the stylesheet already
/// provides for its classes — inline style beats class rules, mirroring the
/// specificity ladder (SPEC §13/§14). Geometry never appears here.
fn node_style_attr(
    n: &PlacedNode,
    classes: &[String],
    ruleset: &RuleSet,
    vars: &VarTable,
    opts: &Options,
) -> String {
    let mut decls: Vec<(&str, String)> = Vec::new();
    for (lini, css) in rules::PAINT_PROPS {
        let value = match (*lini, n.shape) {
            // On |text|, `color` is an alias for `fill` (CSS-style); the
            // shape rule's `currentColor` keeps SVG inheritance working when
            // neither is set.
            ("fill", ShapeKind::Text) => n.attrs.get("fill").or_else(|| n.attrs.get("color")),
            ("color", ShapeKind::Text) => None,
            _ => n.attrs.get(lini),
        };
        let Some(v) = value else { continue };
        let formatted = match *lini {
            "text-size" => format!("{}px", format_value(v, vars, opts)),
            _ => format_value(v, vars, opts),
        };
        if ruleset.provided(classes, css) != Some(formatted.as_str()) {
            decls.push((css, formatted));
        }
    }
    if n.attrs.get("line").is_some() {
        let thickness = n.attrs.number("thickness").unwrap_or(1.0);
        let dash = values::dasharray_value(&n.attrs, thickness);
        let value = if dash.is_empty() {
            "none".to_string()
        } else {
            dash
        };
        if ruleset.provided(classes, "stroke-dasharray") != Some(value.as_str()) {
            decls.push(("stroke-dasharray", value));
        }
    }
    style_attr_from(&decls)
}

/// ` style="…"` from prop declarations, or empty.
fn style_attr_from(decls: &[(&str, String)]) -> String {
    if decls.is_empty() {
        return String::new();
    }
    let body: Vec<String> = decls.iter().map(|(p, v)| format!("{}: {}", p, v)).collect();
    format!(r#" style="{}""#, body.join("; "))
}

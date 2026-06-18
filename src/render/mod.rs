mod filters;
pub(crate) mod markers; // `marker_size` is read by the router to reserve stub room
mod primitives;
mod rules;
mod style_block;
mod values;
mod wires;

use crate::Options;
use crate::layout::{LaidOut, PlacedNode};
use crate::resolve::{ShapeKind, VarTable};
use filters::FilterTable;
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

    let filters = FilterTable::collect(&laid_out.nodes, &laid_out.vars, opts);
    if filters.is_empty() {
        out.push_str("  <defs/>\n");
    } else {
        out.push_str("  <defs>\n");
        filters.emit_defs(&mut out, &laid_out.vars, opts);
        out.push_str("  </defs>\n");
    }

    // A root `fill:` paints a backing rect over the whole viewBox (SPEC §13),
    // behind everything else.
    if let Some(fill) = &laid_out.canvas_fill {
        writeln!(
            out,
            r#"  <rect class="lini-canvas" x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
            num(vb.x),
            num(vb.y),
            num(vb.w),
            num(vb.h),
            format_value(fill, &laid_out.vars, opts),
        )
        .unwrap();
    }

    out.push_str("  <g class=\"lini-scene\">\n");
    for node in in_layer_order(&laid_out.nodes) {
        render_node(&mut out, node, 2, &laid_out.vars, &ruleset, &filters, opts);
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
    filters: &FilterTable,
    opts: &Options,
) {
    use std::fmt::Write;
    // `link:` wraps the whole node in an `<a href>` so the shape (and its
    // children) is clickable (SPEC §5). The group sits one level deeper inside.
    let link = match n.attrs.get("link") {
        Some(crate::resolve::ResolvedValue::String(s)) => Some(s.clone()),
        _ => None,
    };
    let depth = if let Some(href) = &link {
        writeln!(
            out,
            r#"{}<a href="{}">"#,
            "  ".repeat(depth),
            escape_xml(href)
        )
        .unwrap();
        depth + 1
    } else {
        depth
    };
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

    // `title:` emits a `<title>` as the group's first child — an SVG tooltip
    // and the accessible name (SPEC §13).
    if let Some(crate::resolve::ResolvedValue::String(title)) = n.attrs.get("title") {
        writeln!(out, "{}  <title>{}</title>", indent, escape_xml(title)).unwrap();
    }

    primitives::render_geometry(out, n, depth + 1, vars, filters, opts);
    for child in in_layer_order(&n.children) {
        render_node(out, child, depth + 1, vars, ruleset, filters, opts);
    }

    writeln!(out, "{}</g>", indent).unwrap();
    if link.is_some() {
        writeln!(out, "{}</a>", "  ".repeat(depth - 1)).unwrap();
    }
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
            "font-size" => format!("{}px", format_value(v, vars, opts)),
            _ => format_value(v, vars, opts),
        };
        if ruleset.provided(classes, css) != Some(formatted.as_str()) {
            decls.push((css, formatted));
        }
    }
    if n.attrs.get("stroke-style").is_some() {
        let width = n.attrs.number("stroke-width").unwrap_or(1.0);
        let dash = values::dasharray_value(&n.attrs, width);
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

/// Siblings in paint order: ascending `layer:` (default 0), ties by source
/// order (SPEC §6). A stable sort keeps the source order within each layer.
fn in_layer_order(nodes: &[PlacedNode]) -> Vec<&PlacedNode> {
    let mut order: Vec<&PlacedNode> = nodes.iter().collect();
    order.sort_by(|a, b| {
        let layer = |n: &PlacedNode| n.attrs.number("layer").unwrap_or(0.0);
        layer(a).total_cmp(&layer(b))
    });
    order
}

/// ` style="…"` from prop declarations, or empty.
fn style_attr_from(decls: &[(&str, String)]) -> String {
    if decls.is_empty() {
        return String::new();
    }
    let body: Vec<String> = decls.iter().map(|(p, v)| format!("{}: {}", p, v)).collect();
    format!(r#" style="{}""#, body.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svg_for(src: &str) -> String {
        let tokens = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(&tokens).expect("parse");
        let program = crate::resolve::resolve_with_theme(&file, &[]).expect("resolve");
        let laid = crate::layout::layout(&program).expect("layout");
        render(&laid, &Options::default())
    }

    #[test]
    fn root_fill_paints_a_backing_rect_over_the_viewbox() {
        let svg = svg_for("fill: #eef;\nx |box|\n");
        assert!(
            svg.contains(r#"class="lini-canvas""#) && svg.contains(r##"fill="#eef""##),
            "{svg}"
        );
    }

    #[test]
    fn no_backing_rect_without_a_root_fill() {
        let svg = svg_for("x |box|\n");
        assert!(!svg.contains("lini-canvas"), "{svg}");
    }

    #[test]
    fn layer_lifts_a_node_above_later_source_order() {
        // `a` is written first; its higher `layer` paints it last (on top),
        // so its <g> is emitted after `b`'s (SPEC §6).
        let svg = svg_for("a |box| { layer: 5; }\nb |box|\n");
        let ai = svg.find(r#"data-id="a""#).expect("a");
        let bi = svg.find(r#"data-id="b""#).expect("b");
        assert!(ai > bi, "a (layer 5) should paint after b: {svg}");
    }

    #[test]
    fn equal_layer_keeps_source_order() {
        let svg = svg_for("a |box|\nb |box|\n");
        assert!(
            svg.find(r#"data-id="a""#).unwrap() < svg.find(r#"data-id="b""#).unwrap(),
            "{svg}"
        );
    }

    #[test]
    fn title_emits_a_title_child_on_the_node_g() {
        let svg = svg_for("x |box| { title: \"a tooltip\"; }\n");
        assert!(svg.contains("<title>a tooltip</title>"), "{svg}");
    }

    #[test]
    fn no_title_element_without_a_title_prop() {
        let svg = svg_for("x |box|\n");
        assert!(!svg.contains("<title>"), "{svg}");
    }
}

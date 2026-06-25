mod filters;
mod gradients;
mod icon_fit;
mod links;
pub(crate) mod markers; // `marker_size` is read by the router to reserve stub room
mod primitives;
mod rounding;
mod rules;
mod style_block;
mod text;
mod used_vars;
pub(crate) mod values;
mod wavy;

use crate::Options;
use crate::layout::{LaidOut, PlacedNode};
use crate::resolve::{AttrMap, NodeKind, VarTable};
use filters::FilterTable;
pub(crate) use gradients::lower as lower_gradients;
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
    let used = used_vars::referenced(laid_out, &ruleset);
    style_block::emit(&mut out, &laid_out.vars, &ruleset, &used, opts);

    let filters = FilterTable::collect(&laid_out.nodes, &laid_out.vars, opts);
    if filters.is_empty() && laid_out.gradients.is_empty() {
        out.push_str("  <defs/>\n");
    } else {
        out.push_str("  <defs>\n");
        filters.emit_defs(&mut out, &laid_out.vars, opts);
        gradients::emit_defs(laid_out, &mut out, opts);
        out.push_str("  </defs>\n");
    }

    // The backing rect paints the scene background over the whole viewBox (SPEC
    // §13); its fill comes from the `.lini-canvas` rule (`--lini-bg`), unless the
    // root set an explicit `fill:`, which overrides it inline.
    let canvas_style = match &laid_out.canvas_fill {
        Some(fill) => format!(
            r#" style="fill: {}""#,
            format_value(fill, &laid_out.vars, opts)
        ),
        None => String::new(),
    };
    writeln!(
        out,
        r#"  <rect class="lini-canvas" x="{}" y="{}" width="{}" height="{}"{}/>"#,
        num(vb.x),
        num(vb.y),
        num(vb.w),
        num(vb.h),
        canvas_style,
    )
    .unwrap();

    out.push_str("  <g class=\"lini-scene\">\n");
    for node in in_layer_order(&laid_out.nodes) {
        render_node(&mut out, node, 2, &laid_out.vars, &ruleset, &filters, opts);
    }
    out.push_str("  </g>\n");

    if laid_out.links.is_empty() && laid_out.strays.is_empty() {
        out.push_str("  <g class=\"lini-links\"/>\n");
    } else {
        out.push_str("  <g class=\"lini-links\">\n");
        let polys: Vec<&[(f64, f64)]> = laid_out.links.iter().map(|w| &w.path[..]).collect();
        let caps: Vec<f64> = laid_out.links.iter().map(links::radius_cap).collect();
        let targets = links::fillet_targets(&polys, &caps);
        // The link-label default font size (the `.lini-link-label` rule's value) —
        // a label inlines its own size only when it differs from this.
        let label_size = laid_out
            .sheet
            .link_defaults
            .number("font-size")
            .unwrap_or(11.0);
        for (idx, (link, targets)) in laid_out.links.iter().zip(&targets).enumerate() {
            links::render_link(
                &mut out,
                idx,
                link,
                targets,
                label_size,
                &laid_out.vars,
                &ruleset,
                opts,
            );
        }
        for air in &laid_out.strays {
            links::render_stray(&mut out, air, &laid_out.vars, opts);
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
    // Text renders as a bare `<text class="lini-text">` at its placed position —
    // no wrapping `<g>` (SPEC §13). Font and colour inherit from the enclosing box.
    if n.kind == NodeKind::Text {
        render_text(out, n, depth, vars, opts);
        return;
    }
    // `href:` wraps the whole node in an `<a href>` so the shape (and its
    // children) is clickable (SPEC §10). The group sits one level deeper inside.
    let href = match n.attrs.get("href") {
        Some(crate::resolve::ResolvedValue::String(s)) => Some(s.clone()),
        _ => None,
    };
    let depth = if let Some(url) = &href {
        writeln!(
            out,
            r#"{}<a href="{}">"#,
            "  ".repeat(depth),
            escape_xml(url)
        )
        .unwrap();
        depth + 1
    } else {
        depth
    };
    let indent = "  ".repeat(depth);
    let class_list = values::class_list(n.kind.as_str(), &n.type_chain, &n.applied_styles);
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

    primitives::render_geometry(out, n, depth + 1, vars, ruleset, filters, opts);
    for child in in_layer_order(&n.children) {
        render_node(out, child, depth + 1, vars, ruleset, filters, opts);
    }

    writeln!(out, "{}</g>", indent).unwrap();
    if href.is_some() {
        writeln!(out, "{}</a>", "  ".repeat(depth - 1)).unwrap();
    }
}

/// A text node: a bare `<text class="lini-text">` at its placed centre (SPEC
/// §13), via the shared text emitter ([`text::emit`]) that also draws link
/// labels. `text-anchor: middle` + `dominant-baseline: central` (on `.lini-text`)
/// centre it on (cx, cy); font and colour inherit from the enclosing box's `<g>`.
/// Its own `{ }` paint/font rides `style=`; `translate` is folded into (cx, cy).
fn render_text(out: &mut String, n: &PlacedNode, depth: usize, vars: &VarTable, opts: &Options) {
    let indent = "  ".repeat(depth);
    let label = n.label.as_deref().unwrap_or("");
    let style = text_style_attr(&n.own_style, vars, opts);
    text::emit(
        out,
        &indent,
        "lini-text",
        label,
        (n.cx, n.cy),
        &n.attrs,
        &style,
    );
}

/// The `style=` for a text node's own `{ }` (SPEC §3): paint and font props ride
/// CSS. `letter-spacing` / `line-spacing` / `translate` / `rotate` / `layer` are
/// geometry or transforms, handled elsewhere, so they never appear here.
fn text_style_attr(own: &AttrMap, vars: &VarTable, opts: &Options) -> String {
    let mut decls: Vec<String> = Vec::new();
    if let Some(v) = own.get("fill").or_else(|| own.get("color")) {
        decls.push(format!("fill: {}", format_value(v, vars, opts)));
    }
    for prop in [
        "font-family",
        "font-weight",
        "font-style",
        "text-transform",
        "text-decoration",
        "text-shadow",
        "opacity",
    ] {
        if let Some(v) = own.get(prop) {
            decls.push(format!("{}: {}", prop, format_value(v, vars, opts)));
        }
    }
    if let Some(v) = own.get("font-size") {
        decls.push(format!("font-size: {}px", format_value(v, vars, opts)));
    }
    if decls.is_empty() {
        String::new()
    } else {
        format!(r#" style="{}""#, decls.join("; "))
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
        let value = match (*lini, n.kind) {
            // On |text|, `color` is an alias for `fill` (CSS-style); the
            // shape rule's `currentColor` keeps SVG inheritance working when
            // neither is set.
            ("fill", NodeKind::Text) => n.attrs.get("fill").or_else(|| n.attrs.get("color")),
            ("color", NodeKind::Text) => None,
            _ => n.attrs.get(lini),
        };
        let Some(v) = value else { continue };
        let formatted = values::css_value(lini, v, vars, opts);
        if ruleset.provided(classes, css) != Some(formatted.as_str()) {
            decls.push((css, formatted));
        }
    }
    if n.attrs.get("stroke-style").is_some() {
        let width = n.attrs.number("stroke-width").unwrap_or(0.0);
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

/// Siblings in paint order: ascending effective layer, ties by source order
/// (SPEC §6). The effective layer is the explicit `layer:`, else 1 for a pinned
/// child (overlays paint above the flow) and 0 for a flow child. A stable sort
/// keeps source order within each layer.
fn in_layer_order(nodes: &[PlacedNode]) -> Vec<&PlacedNode> {
    let mut order: Vec<&PlacedNode> = nodes.iter().collect();
    order.sort_by(|a, b| eff_layer(a).total_cmp(&eff_layer(b)));
    order
}

fn eff_layer(n: &PlacedNode) -> f64 {
    n.attrs
        .number("layer")
        .unwrap_or(if crate::layout::is_pinned(&n.attrs) {
            1.0
        } else {
            0.0
        })
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
        let lowered = crate::desugar::desugar(&file).expect("desugar");
        let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
        let mut laid = crate::layout::layout(&program).expect("layout");
        gradients::lower(&mut laid);
        render(&laid, &Options::default())
    }

    #[test]
    fn root_fill_overrides_the_canvas_inline() {
        let svg = svg_for("{ fill: #eef; }\nx |box|\n");
        assert!(
            svg.contains(r#"class="lini-canvas""#) && svg.contains(r##"style="fill: #eef""##),
            "{svg}"
        );
    }

    #[test]
    fn canvas_rect_defaults_to_the_bg_var() {
        // The backing rect is always present, painted by the `.lini-canvas` rule.
        let svg = svg_for("x |box|\n");
        assert!(svg.contains(r#"<rect class="lini-canvas""#), "{svg}");
        assert!(
            svg.contains(".lini .lini-canvas { fill: var(--lini-bg); }"),
            "{svg}"
        );
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

    #[test]
    fn shadow_flood_color_is_a_literal_in_live_mode() {
        // `flood-color` is a filter presentation attribute: var()/light-dark()
        // there fall back to opaque black, so the tint must be a resolved literal
        // even with live vars (the canvas-fill bug, on the shadow).
        let svg = svg_for("x |box| { shadow: 4 }\n");
        let flood = svg
            .split("flood-color=\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .expect("a shadow flood-color");
        assert!(
            !flood.starts_with("var(") && !flood.contains("light-dark"),
            "shadow tint must be a literal, got {flood:?}"
        );
    }
}

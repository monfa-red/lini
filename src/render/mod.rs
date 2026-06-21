mod filters;
pub(crate) mod markers; // `marker_size` is read by the router to reserve stub room
mod primitives;
mod rounding;
mod rules;
mod style_block;
pub(crate) mod values;
mod wavy;
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

    if laid_out.wires.is_empty() && laid_out.airwires.is_empty() {
        out.push_str("  <g class=\"lini-wires\"/>\n");
    } else {
        out.push_str("  <g class=\"lini-wires\">\n");
        let polys: Vec<&[(f64, f64)]> = laid_out.wires.iter().map(|w| &w.path[..]).collect();
        let caps: Vec<f64> = laid_out.wires.iter().map(wires::radius_cap).collect();
        let targets = wires::fillet_targets(&polys, &caps);
        // The wire-label default font size (the `.lini-wire-label` rule's value) —
        // a label inlines its own size only when it differs from this.
        let label_size = laid_out
            .sheet
            .wire_defaults
            .number("font-size")
            .unwrap_or(11.0);
        for (idx, (wire, targets)) in laid_out.wires.iter().zip(&targets).enumerate() {
            wires::render_wire(
                &mut out,
                idx,
                wire,
                targets,
                label_size,
                &laid_out.vars,
                &ruleset,
                opts,
            );
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
    // Text renders as a bare `<text class="lini-text">` at its placed position —
    // no wrapping `<g>` (SPEC §13). Font and colour inherit from the enclosing box.
    if n.shape == ShapeKind::Text {
        render_text(out, n, depth);
        return;
    }
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

    primitives::render_geometry(out, n, depth + 1, vars, ruleset, filters, opts);
    for child in in_layer_order(&n.children) {
        render_node(out, child, depth + 1, vars, ruleset, filters, opts);
    }

    writeln!(out, "{}</g>", indent).unwrap();
    if link.is_some() {
        writeln!(out, "{}</a>", "  ".repeat(depth - 1)).unwrap();
    }
}

/// A text node: a bare `<text class="lini-text">` at its placed centre (SPEC
/// §13). `text-anchor: middle` + `dominant-baseline: central` (on `.lini-text`)
/// centre it on (x, y); font and colour inherit from the enclosing box's `<g>`.
fn render_text(out: &mut String, n: &PlacedNode, depth: usize) {
    use std::fmt::Write;
    let indent = "  ".repeat(depth);
    let label = n.label.as_deref().unwrap_or("");
    let (x, y) = (num(n.cx), num(n.cy));
    let lines: Vec<&str> = label.split('\n').collect();
    // `letter-spacing` bakes into a per-glyph `dx` list — geometry, never CSS
    // (SPEC §10); `text-anchor: middle` still centres the spaced run.
    let ls = n.attrs.number("letter-spacing").unwrap_or(0.0);
    if lines.len() <= 1 {
        writeln!(
            out,
            r#"{}<text class="lini-text" x="{}" y="{}"{}>{}</text>"#,
            indent,
            x,
            y,
            dx_attr(label, ls),
            escape_xml(label)
        )
        .unwrap();
        return;
    }
    // Multi-line (SPEC §6): one tspan per line, leading `font-size × 1.2` plus
    // `line-spacing`, the block centred on (cx, cy) so `dominant-baseline:
    // central` still holds.
    let size = n.attrs.number("font-size").unwrap_or(0.0);
    let spacing = size * 1.2 + n.attrs.number("line-spacing").unwrap_or(0.0);
    let top = n.cy - spacing * (lines.len() as f64 - 1.0) / 2.0;
    write!(
        out,
        r#"{}<text class="lini-text" x="{}" y="{}">"#,
        indent, x, y
    )
    .unwrap();
    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            write!(
                out,
                r#"<tspan x="{}" y="{}"{}>{}</tspan>"#,
                x,
                num(top),
                dx_attr(line, ls),
                escape_xml(line)
            )
            .unwrap();
        } else {
            write!(
                out,
                r#"<tspan x="{}" dy="{}"{}>{}</tspan>"#,
                x,
                num(spacing),
                dx_attr(line, ls),
                escape_xml(line)
            )
            .unwrap();
        }
    }
    writeln!(out, "</text>").unwrap();
}

/// The ` dx="0 s s …"` glyph-advance list that bakes `letter-spacing` into the
/// positions: 0 before the first glyph, `s` before each later one. Empty when
/// there is no spacing or fewer than two glyphs (nothing to space).
fn dx_attr(line: &str, letter_spacing: f64) -> String {
    let count = line.chars().count();
    if letter_spacing == 0.0 || count < 2 {
        return String::new();
    }
    let mut s = String::from(r#" dx="0"#);
    for _ in 1..count {
        s.push(' ');
        s.push_str(&num(letter_spacing));
    }
    s.push('"');
    s
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
        let laid = crate::layout::layout(&program).expect("layout");
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
}

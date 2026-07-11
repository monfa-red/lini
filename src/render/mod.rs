mod filters;
mod fonts;
mod icon_fit;
mod intern;
mod links;
pub(crate) mod markers; // `marker_size` is read by the router to reserve stub room
mod paints;
mod primitives;
mod rounding;
mod rules;
mod style_block;
mod stylesheet;
mod text;
mod used_vars;
pub(crate) mod values;
mod wavy;

use crate::Options;
use crate::layout::{LaidOut, PlacedNode};
use crate::resolve::{AttrMap, NodeKind, VarTable};
use filters::FilterTable;
pub(crate) use paints::lower as lower_paints;
use rules::RuleSet;
use values::{escape_xml, format_value, num};

pub fn render(laid_out: &LaidOut, opts: &Options) -> String {
    let vb = &laid_out.viewbox;

    use std::fmt::Write;

    // The stylesheet's structural rules state every paint default once; the
    // root `.lini` rule seeds the inherited text properties (font, size,
    // color) for the whole tree. Per-node differences ride `style=`.
    let ruleset = stylesheet::build(laid_out, opts);
    let filters = FilterTable::collect(&laid_out.nodes, &laid_out.vars, opts);

    // The scene + link body renders first, into its own buffer: the one text
    // emitter registers every face (and, under `--static`, glyph) it uses in
    // the sink, and the `<style>` / `<defs>` blocks below then carry exactly
    // that set [SPEC 17].
    let sink = fonts::FontSink::new(laid_out);
    let mut body = String::with_capacity(2048);
    body.push_str("  <g class=\"lini-scene\">\n");
    for node in in_layer_order(&laid_out.nodes) {
        render_node(
            &mut body,
            node,
            2,
            &laid_out.vars,
            &ruleset,
            &filters,
            opts,
            &sink,
        );
    }
    body.push_str("  </g>\n");

    if laid_out.links.is_empty() && laid_out.strays.is_empty() {
        body.push_str("  <g class=\"lini-links\"/>\n");
    } else {
        body.push_str("  <g class=\"lini-links\">\n");
        let polys: Vec<&[(f64, f64)]> = laid_out.links.iter().map(|w| &w.path[..]).collect();
        let caps: Vec<f64> = laid_out.links.iter().map(links::radius_cap).collect();
        let targets = links::fillet_targets(&polys, &caps);
        for (idx, (link, targets)) in laid_out.links.iter().zip(&targets).enumerate() {
            links::render_link(
                &mut body,
                idx,
                link,
                targets,
                &laid_out.vars,
                &ruleset,
                opts,
                &sink,
            );
        }
        for air in &laid_out.strays {
            links::render_stray(&mut body, air, &laid_out.vars, opts);
        }
        body.push_str("  </g>\n");
    }

    let mut out = String::with_capacity(2048 + body.len());
    // A pages-only scene prints true-scale [SPEC 15.8]: real millimetres for
    // `width` / `height` (the `viewBox` stays px, so on-screen sizing is
    // unchanged). Every other scene sizes in pixels.
    let size = match laid_out.physical {
        Some((w, h)) => format!(r#"width="{}mm" height="{}mm""#, num(w), num(h)),
        None => format!(r#"width="{}" height="{}""#, num(vb.w), num(vb.h)),
    };
    writeln!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" {size} class="lini">"#,
        num(vb.x),
        num(vb.y),
        num(vb.w),
        num(vb.h),
    )
    .unwrap();

    let used = used_vars::referenced(laid_out, &ruleset);
    // One `:hover ~` reveal rule per tooltip card — live charts only.
    let tooltip_rules = if opts.static_mode {
        0
    } else {
        tooltip_count(&laid_out.nodes)
    };
    style_block::emit(
        &mut out,
        &laid_out.vars,
        &ruleset,
        &used,
        opts,
        tooltip_rules,
        if opts.embed_font { Some(&sink) } else { None },
    );

    let glyphs = opts.static_mode && fonts::has_glyphs(&sink);
    if filters.is_empty()
        && laid_out.gradients.is_empty()
        && laid_out.hatches.is_empty()
        && laid_out.clips.is_empty()
        && !glyphs
    {
        out.push_str("  <defs/>\n");
    } else {
        out.push_str("  <defs>\n");
        filters.emit_defs(&mut out, &laid_out.vars, opts);
        paints::emit_defs(laid_out, &mut out, opts);
        if glyphs {
            fonts::emit_glyph_defs(&mut out, &sink);
        }
        out.push_str("  </defs>\n");
    }

    // The backing rect paints the scene background over the whole viewBox
    // [SPEC 13]; its fill comes from the `.lini-canvas` rule (`--lini-bg`), unless the
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

    out.push_str(&body);
    out.push_str("</svg>\n");
    out
}

/// The number of rich tooltip cards (max `tip-N` index + 1) — the renderer emits one
/// `.lini-hit-N:hover ~ .lini-tip-N` reveal rule per card [SPEC 14.8].
fn tooltip_count(nodes: &[PlacedNode]) -> usize {
    fn scan(nodes: &[PlacedNode], max: &mut Option<usize>) {
        for n in nodes {
            for t in &n.type_chain {
                if let Some(i) = t.strip_prefix("tip-").and_then(|s| s.parse::<usize>().ok()) {
                    *max = Some(max.map_or(i, |m| m.max(i)));
                }
            }
            scan(&n.children, max);
        }
    }
    let mut max = None;
    scan(nodes, &mut max);
    max.map_or(0, |m| m + 1)
}

#[allow(clippy::too_many_arguments)] // the recursive walk threads every emission context
fn render_node(
    out: &mut String,
    n: &PlacedNode,
    depth: usize,
    vars: &VarTable,
    ruleset: &RuleSet,
    filters: &FilterTable,
    opts: &Options,
    sink: &fonts::FontSink,
) {
    use std::fmt::Write;
    // The rich chart tooltip card is live-only [SPEC 14.8]: a baked SVG keeps the
    // `<title>` floor and drops the `:hover` card, so skip it (and its subtree) here.
    if opts.static_mode && n.type_chain.iter().any(|t| t == "chart-tip") {
        return;
    }
    // Text renders as a bare `<text class="lini-text">` at its placed position —
    // no wrapping `<g>` [SPEC 17]. Font and colour inherit from the enclosing box.
    if n.kind == NodeKind::Text {
        render_text(out, n, depth, ruleset, vars, opts, sink);
        return;
    }
    // `href:` wraps the whole node in an `<a href>` so the shape (and its
    // children) is clickable [SPEC 13]. The group sits one level deeper inside.
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
    // The tooltip `hit-N` class is a live-only hover hook [SPEC 14.8]: strip it when
    // baking so a data mark renders exactly as it would with tooltips off.
    let stripped;
    let chain: &[String] = if opts.static_mode && n.type_chain.iter().any(|t| t.starts_with("hit-"))
    {
        stripped = n
            .type_chain
            .iter()
            .filter(|t| !t.starts_with("hit-"))
            .cloned()
            .collect::<Vec<_>>();
        &stripped
    } else {
        &n.type_chain
    };
    let class_list = values::class_list(n.kind.as_str(), chain, &n.applied_styles);
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
    // A `|detail|` clips its geometry to the region circle [SPEC 15.8] — the
    // `url(#…)` reference `paints::lower` interned.
    let clip_attr = match n.attrs.get("clip") {
        Some(crate::resolve::ResolvedValue::RawCss(u)) => format!(r#" clip-path="{u}""#),
        _ => String::new(),
    };
    let style_attr = node_style_attr(n, &class_list, ruleset, vars, opts);
    writeln!(
        out,
        r#"{}<g class="{}"{}{}{}{}>"#,
        indent, classes, id_attr, clip_attr, style_attr, transform
    )
    .unwrap();

    // `hint:` emits a `<title>` as the group's first child — an SVG tooltip
    // and the accessible name [SPEC 17].
    if let Some(crate::resolve::ResolvedValue::String(hint)) = n.attrs.get("hint") {
        writeln!(out, "{}  <title>{}</title>", indent, escape_xml(hint)).unwrap();
    }

    primitives::render_geometry(out, n, depth + 1, vars, ruleset, filters, opts);
    for child in in_layer_order(&n.children) {
        render_node(out, child, depth + 1, vars, ruleset, filters, opts, sink);
    }

    writeln!(out, "{}</g>", indent).unwrap();
    if href.is_some() {
        writeln!(out, "{}</a>", "  ".repeat(depth - 1)).unwrap();
    }
}

/// A text node: a bare `<text class="lini-text">` at its placed centre
/// [SPEC 13], via the shared text emitter ([`text::emit`]) that also draws link
/// labels. `text-anchor: middle` (on `.lini-text`) + the baked cap-height `dy`
/// centre it on (cx, cy); font and colour inherit from the enclosing box's `<g>`.
/// Its own `{ }` paint/font rides `style=`; `translate` is folded into (cx, cy).
fn render_text(
    out: &mut String,
    n: &PlacedNode,
    depth: usize,
    ruleset: &RuleSet,
    vars: &VarTable,
    opts: &Options,
    sink: &fonts::FontSink,
) {
    let indent = "  ".repeat(depth);
    let label = n.label.as_deref().unwrap_or("");
    // A text node's `type_chain` carries any extra class (e.g. a chart's `.lini-chart-label`
    // inline labels, [SPEC 14.8]); plain text has none, so this stays `lini-text`.
    let mut classes = vec!["lini-text".to_string()];
    classes.extend(n.type_chain.iter().map(|t| format!("lini-{t}")));
    let style = text_paint_attr(&n.own_style, &classes, ruleset, vars, opts);
    text::emit(
        out,
        &indent,
        &classes,
        label,
        (n.cx, n.cy),
        &n.attrs,
        &style,
        ruleset,
        opts,
        sink,
    );
}

/// The inline `style=` for a text leaf — a node's `<text>` or a link label — as
/// the class-diff of its paint [SPEC 17], the same chokepoint a node `<g>` uses
/// ([`node_style_attr`]). This is the **one** place text paint inlines, so no
/// font/paint prop is ever hand-dropped (link labels used to silently lose
/// `text-shadow` on their own hand-rolled path). `color` aliases `fill`
/// (CSS-style); `css_value` adds the `px` / length units. Geometry
/// (`letter-spacing`, `translate`, `rotate`, `layer`) is handled elsewhere and
/// never rides here — it is not in `PAINT_PROPS`.
pub(super) fn text_paint_attr(
    own: &AttrMap,
    classes: &[String],
    ruleset: &RuleSet,
    vars: &VarTable,
    opts: &Options,
) -> String {
    let decls = ruleset.inline_paint_diff(
        classes,
        own,
        |lini| match lini {
            "fill" => own.get("fill").or_else(|| own.get("color")),
            "color" => None,
            _ => own.get(lini),
        },
        |lini, v| values::css_value(lini, v, vars, opts),
    );
    style_attr_from(&decls)
}

/// The node's paint, as the difference against what the stylesheet already
/// provides for its classes — inline style beats class rules, mirroring the
/// specificity ladder [SPEC 4/16]. Geometry never appears here.
fn node_style_attr(
    n: &PlacedNode,
    classes: &[String],
    ruleset: &RuleSet,
    vars: &VarTable,
    opts: &Options,
) -> String {
    let decls = ruleset.inline_paint_diff(
        classes,
        &n.attrs,
        // On |text|, `color` is an alias for `fill` (CSS-style); the shape rule's
        // `currentColor` keeps SVG inheritance working when neither is set.
        |lini| match (lini, n.kind) {
            ("fill", NodeKind::Text) => n.attrs.get("fill").or_else(|| n.attrs.get("color")),
            ("color", NodeKind::Text) => None,
            _ => n.attrs.get(lini),
        },
        |lini, v| values::css_value(lini, v, vars, opts),
    );
    style_attr_from(&decls)
}

/// Siblings in paint order: ascending effective layer, ties by source order
/// [SPEC 5]. The effective layer is the explicit `layer:`, else 1 for a pinned
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
pub(super) fn style_attr_from(decls: &[(&str, String)]) -> String {
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
        let file = crate::syntax::parser::parse(src, &tokens).expect("parse");
        let lowered = crate::desugar::desugar(&file).expect("desugar");
        let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
        let mut laid = crate::layout::layout(&program).expect("layout");
        paints::lower(&mut laid);
        render(&laid, &Options::default())
    }

    #[test]
    fn root_fill_overrides_the_canvas_inline() {
        let svg = svg_for("{ fill: #eef; }\n|box#x|\n");
        assert!(
            svg.contains(r#"class="lini-canvas""#) && svg.contains(r##"style="fill: #eef""##),
            "{svg}"
        );
    }

    #[test]
    fn canvas_rect_defaults_to_the_bg_var() {
        // The backing rect is always present, painted by the `.lini-canvas` rule.
        let svg = svg_for("|box#x|\n");
        assert!(svg.contains(r#"<rect class="lini-canvas""#), "{svg}");
        assert!(
            svg.contains(".lini .lini-canvas { fill: var(--lini-bg); }"),
            "{svg}"
        );
    }

    #[test]
    fn layer_lifts_a_node_above_later_source_order() {
        // `a` is written first; its higher `layer` paints it last (on top),
        // so its <g> is emitted after `b`'s [SPEC 5].
        let svg = svg_for("|box#a| { layer: 5; }\n|box#b|\n");
        let ai = svg.find(r#"data-id="a""#).expect("a");
        let bi = svg.find(r#"data-id="b""#).expect("b");
        assert!(ai > bi, "a (layer 5) should paint after b: {svg}");
    }

    #[test]
    fn equal_layer_keeps_source_order() {
        let svg = svg_for("|box#a|\n|box#b|\n");
        assert!(
            svg.find(r#"data-id="a""#).unwrap() < svg.find(r#"data-id="b""#).unwrap(),
            "{svg}"
        );
    }

    #[test]
    fn hint_emits_a_title_child_on_the_node_g() {
        let svg = svg_for("|box#x| { hint: \"a tooltip\"; }\n");
        assert!(svg.contains("<title>a tooltip</title>"), "{svg}");
    }

    #[test]
    fn no_title_element_without_a_title_prop() {
        let svg = svg_for("|box#x|\n");
        assert!(!svg.contains("<title>"), "{svg}");
    }

    #[test]
    fn shadow_flood_color_is_a_literal_in_live_mode() {
        // `flood-color` is a filter presentation attribute: var()/light-dark()
        // there fall back to opaque black, so the tint must be a resolved literal
        // even with live vars (the canvas-fill bug, on the shadow).
        let svg = svg_for("|box#x| { shadow: 4 }\n");
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

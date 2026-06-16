//! The output stylesheet's structural rules (SPEC §14): paint defaults stated
//! once as class rules — paint rides CSS, geometry bakes. Rules are unlayered
//! so non-browser renderers (which skip `@layer`) parse them, and scoped under
//! `.lini` so an SVG inlined into a host document restyles nothing else.
//!
//! Markers carry no rule: their fill follows each wire's stroke per element,
//! and a `.lini-marker` rule would override those presentation attrs.

use super::values::format_value;
use crate::Options;
use crate::layout::{LaidOut, PlacedNode};
use crate::resolve::{AttrMap, ResolvedValue, ShapeKind, VarTable};
use std::collections::BTreeSet;

/// lini attr → CSS property, for every paint attr that maps one-to-one.
/// `line` is the exception: it compiles to `stroke-dasharray`, whose pattern
/// scales with `thickness`, so the pair is translated together.
pub const PAINT_PROPS: &[(&str, &str)] = &[
    ("fill", "fill"),
    ("stroke", "stroke"),
    ("thickness", "stroke-width"),
    ("opacity", "opacity"),
    ("color", "color"),
    ("font", "font-family"),
    ("text-size", "font-size"),
    ("weight", "font-weight"),
];

pub struct Rule {
    /// The single class the selector keys on (`lini` = the root rule).
    pub class: String,
    /// CSS property → formatted value, emission order.
    pub props: Vec<(String, String)>,
}

pub struct RuleSet {
    pub rules: Vec<Rule>,
}

impl RuleSet {
    /// Append the rules to the `<style>` body.
    pub fn emit(&self, out: &mut String) {
        for rule in &self.rules {
            if rule.props.is_empty() {
                continue;
            }
            if rule.class == "lini" {
                out.push_str("    .lini {");
            } else {
                out.push_str("    .lini .");
                out.push_str(&rule.class);
                out.push_str(" {");
            }
            for (prop, value) in &rule.props {
                out.push(' ');
                out.push_str(prop);
                out.push_str(": ");
                out.push_str(value);
                out.push(';');
            }
            out.push_str(" }\n");
        }
    }

    /// The value the sheet provides for an element carrying `classes` —
    /// later rules win on the tie, exactly the CSS cascade for equal
    /// single-class specificity. The root `.lini` rule is deliberately
    /// excluded: its props (`font-*`, `color`) are *inherited*, so a nested
    /// element's effective value comes from its nearest ancestor, not the
    /// root — diffing against the root would drop a reset-to-default that an
    /// overriding ancestor then overrides (the node must state its own value
    /// to win, exactly as `font-weight` already does by never being on root).
    pub fn provided(&self, classes: &[String], prop: &str) -> Option<&str> {
        let mut hit = None;
        for rule in &self.rules {
            if !classes.contains(&rule.class) {
                continue;
            }
            if let Some((_, v)) = rule.props.iter().find(|(p, _)| p == prop) {
                hit = Some(v.as_str());
            }
        }
        hit
    }
}

/// Build the document's structural rules: root inherited-text rule, per-shape
/// paint defaults (only shapes present), built-in template looks, user shape
/// defs, `.style` defs (definition order), and the wire/marker defaults.
pub fn build(laid: &LaidOut, opts: &Options) -> RuleSet {
    let vars = &laid.vars;
    let live = |name: &str| {
        format_value(
            &ResolvedValue::LiveVar {
                name: name.to_string(),
                raw: false,
                baked: None,
            },
            vars,
            opts,
        )
    };

    let mut present: BTreeSet<&str> = BTreeSet::new();
    let mut used_styles: BTreeSet<&str> = BTreeSet::new();
    for node in &laid.nodes {
        collect(node, &mut present, &mut used_styles);
    }
    // Wires carry styles too (same class surface as nodes), so a style used
    // only by a wire still emits its rule.
    for wire in &laid.wires {
        for style in &wire.applied_styles {
            used_styles.insert(style.as_str());
        }
    }

    let mut rules: Vec<Rule> = Vec::new();

    // Root rule: the inherited text properties, stated once. `text-size` is a
    // layout constant, so it always formats to a literal.
    let text_size = layout_number(vars, "text-size", 13.0);
    rules.push(Rule {
        class: "lini".into(),
        props: vec![
            ("font-family".into(), live("font")),
            (
                "font-size".into(),
                format!("{}px", super::values::num(text_size)),
            ),
            ("color".into(), live("text-color")),
        ],
    });

    // Primitive paint defaults, fixed order — these state what the renderer's
    // per-element fallbacks used to inline. Every closed rule masks
    // `stroke-dasharray` so a container's `line:` can never bleed into
    // children through a gap in the cascade.
    const CLOSED: &[ShapeKind] = &[
        ShapeKind::Rect,
        ShapeKind::Oval,
        ShapeKind::Hex,
        ShapeKind::Slant,
        ShapeKind::Cyl,
        ShapeKind::Diamond,
        ShapeKind::Cloud,
        ShapeKind::Poly,
        ShapeKind::Path,
    ];
    for kind in CLOSED {
        if present.contains(kind.as_str()) {
            rules.push(Rule {
                class: format!("lini-shape-{}", kind.as_str()),
                props: vec![
                    ("fill".into(), live("fill")),
                    ("stroke".into(), live("stroke")),
                    ("stroke-width".into(), "1".into()),
                    ("stroke-dasharray".into(), "none".into()),
                ],
            });
        }
    }
    if present.contains("line") {
        rules.push(Rule {
            class: "lini-shape-line".into(),
            props: vec![
                ("fill".into(), "none".into()),
                ("stroke".into(), live("stroke")),
                ("stroke-width".into(), "1".into()),
                ("stroke-dasharray".into(), "none".into()),
            ],
        });
    }
    if present.contains("text") {
        // The anchor pair is honored via CSS by resvg (verified) — inherited
        // properties, so they flow from the text's <g> into the element.
        // `stroke: none` masks a container's stroke from bleeding into the
        // glyphs (text never strokes; the wire-label halo is element-level).
        // Group captions are `|text|`, so they share this rule via lini-shape-text.
        rules.push(Rule {
            class: "lini-shape-text".into(),
            props: vec![
                ("fill".into(), "currentColor".into()),
                ("stroke".into(), "none".into()),
                ("text-anchor".into(), "middle".into()),
                ("dominant-baseline".into(), "central".into()),
            ],
        });
    }
    if present.contains("icon") {
        rules.push(Rule {
            class: "lini-shape-icon".into(),
            props: vec![("fill".into(), live("stroke"))],
        });
    }

    // Built-in template looks (group's container wash), then user shape defs
    // in definition order — paint subset only, geometry stays baked.
    for (name, attrs) in &laid.sheet.templates {
        if present.contains(name.as_str()) {
            rules.push(Rule {
                class: format!("lini-shape-{}", name),
                props: paint_props(attrs, vars, opts),
            });
        }
    }
    for (name, attrs) in &laid.sheet.shape_defs {
        if present.contains(name.as_str()) {
            rules.push(Rule {
                class: format!("lini-shape-{}", name),
                props: paint_props(attrs, vars, opts),
            });
        }
    }

    // `|name|` type defaults merge into the matching rule (creating it for
    // paint-less templates like `|card|` that gain paint only via defaults).
    for (name, attrs) in &laid.sheet.type_defaults {
        if !present.contains(name.as_str()) {
            continue;
        }
        let class = format!("lini-shape-{}", name);
        let props = paint_props(attrs, vars, opts);
        if let Some(rule) = rules.iter_mut().find(|r| r.class == class) {
            for (prop, value) in props {
                if let Some(slot) = rule.props.iter_mut().find(|(p, _)| *p == prop) {
                    slot.1 = value;
                } else {
                    rule.props.push((prop, value));
                }
            }
        } else {
            rules.push(Rule { class, props });
        }
    }

    // Wires: the `|wire|` defaults stated once. Emitted *before* the style
    // rules — it is the wire's default layer (like a shape rule for a node), so
    // a wire's `.style` class overrides it in the cascade (SPEC §13).
    if !laid.wires.is_empty() || !laid.airwires.is_empty() {
        let defaults = &laid.sheet.wire_defaults;
        let mut props = vec![("fill".into(), "none".into())];
        let mut wire_paint = paint_props(defaults, vars, opts);
        if !wire_paint.iter().any(|(p, _)| p == "stroke") {
            wire_paint.push(("stroke".into(), live("stroke")));
        }
        if !wire_paint.iter().any(|(p, _)| p == "stroke-width") {
            wire_paint.push(("stroke-width".into(), "1".into()));
        }
        if !wire_paint.iter().any(|(p, _)| p == "stroke-dasharray") {
            wire_paint.push(("stroke-dasharray".into(), "none".into()));
        }
        props.extend(wire_paint);
        rules.push(Rule {
            class: "lini-wire".into(),
            props,
        });
    }

    // `.style` defs in definition order — the defs block shipped as CSS. After
    // the shape/wire default rules, so a class overrides a default.
    for (name, attrs) in &laid.sheet.styles {
        if used_styles.contains(name.as_str()) {
            rules.push(Rule {
                class: format!("lini-style-{}", name),
                props: paint_props(attrs, vars, opts),
            });
        }
    }

    rules.retain(|r| !r.props.is_empty());
    RuleSet { rules }
}

/// The paint subset of an attr map, translated to CSS props. `line` compiles
/// jointly with `thickness` into `stroke-dasharray`.
pub fn paint_props(attrs: &AttrMap, vars: &VarTable, opts: &Options) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (lini, css) in PAINT_PROPS {
        if let Some(v) = attrs.get(lini) {
            let formatted = match *lini {
                "text-size" => format!("{}px", format_value(v, vars, opts)),
                _ => format_value(v, vars, opts),
            };
            out.push((css.to_string(), formatted));
        }
    }
    if attrs.get("line").is_some() {
        let thickness = attrs.number("thickness").unwrap_or(1.0);
        let dash = super::values::dasharray_value(attrs, thickness);
        out.push((
            "stroke-dasharray".to_string(),
            if dash.is_empty() {
                "none".to_string()
            } else {
                dash
            },
        ));
    }
    out
}

fn layout_number(vars: &VarTable, name: &str, fallback: f64) -> f64 {
    vars.get(name)
        .and_then(|e| e.value.as_number())
        .unwrap_or(fallback)
}

fn collect<'a>(
    node: &'a PlacedNode,
    present: &mut BTreeSet<&'a str>,
    used_styles: &mut BTreeSet<&'a str>,
) {
    present.insert(node.shape.as_str());
    for name in &node.type_chain {
        present.insert(name.as_str());
    }
    for name in &node.applied_styles {
        used_styles.insert(name.as_str());
    }
    for child in &node.children {
        collect(child, present, used_styles);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules_for(src: &str) -> RuleSet {
        let tokens = crate::lexer::lex(src).expect("lex");
        let file = crate::parser::parse(&tokens).expect("parse");
        let program = crate::resolve::resolve(file).expect("resolve");
        let laid = crate::layout::layout(&program).expect("layout");
        build(&laid, &Options::default())
    }

    fn emit_str(set: &RuleSet) -> String {
        let mut s = String::new();
        set.emit(&mut s);
        s
    }

    #[test]
    fn root_rule_carries_inherited_text_props() {
        let css = emit_str(&rules_for("x |rect| \"hi\"\n"));
        assert!(
            css.contains(".lini { font-family: var(--lini-font); font-size: 14px; color: var(--lini-text-color); }"),
            "{}",
            css
        );
    }

    #[test]
    fn shape_rules_only_for_present_types() {
        let css = emit_str(&rules_for("x |rect| \"hi\"\n"));
        assert!(css.contains(".lini .lini-shape-rect {"), "{}", css);
        assert!(!css.contains("lini-shape-oval"), "{}", css);
    }

    #[test]
    fn shape_rules_complete_over_inheritable_paint() {
        let set = rules_for("x |rect| \"hi\"\ny |oval|\nz |line| points:[(0,0),(10,0)]\n");
        for rule in &set.rules {
            if rule.class == "lini-shape-text" {
                // Text masks stroke — a container's stroke must never bleed
                // into glyph outlines.
                assert!(
                    rule.props.iter().any(|(p, v)| p == "stroke" && v == "none"),
                    "text rule lacks the stroke mask"
                );
                continue;
            }
            if rule.class.starts_with("lini-shape-") && rule.class != "lini-shape-icon" {
                assert!(
                    rule.props.iter().any(|(p, _)| p == "stroke-dasharray"),
                    "rule {} lacks the dasharray mask",
                    rule.class
                );
            }
        }
    }

    #[test]
    fn style_defs_emit_in_defs_order_used_only() {
        let css = emit_str(&rules_for(
            "{ .a stroke:red\n  .b stroke:blue\n  .unused stroke:green }\nx |rect| .b .a\n",
        ));
        let a = css.find(".lini .lini-style-a").expect("a rule");
        let b = css.find(".lini .lini-style-b").expect("b rule");
        assert!(a < b, "definition order: {}", css);
        assert!(!css.contains("lini-style-unused"), "{}", css);
    }

    #[test]
    fn wire_rule_states_defaults() {
        let css = emit_str(&rules_for("a -> b\n"));
        assert!(
            css.contains(
                ".lini .lini-wire { fill: none; stroke: var(--lini-stroke); stroke-width: 1; stroke-dasharray: none; }"
            ),
            "{}",
            css
        );
        assert!(!css.contains("lini-marker"), "{}", css);
    }

    #[test]
    fn type_defaults_merge_into_shape_rule() {
        let css = emit_str(&rules_for("{ |rect| fill:lightyellow }\nx |rect|\n"));
        assert!(
            css.contains(".lini .lini-shape-rect { fill: lightyellow;"),
            "{}",
            css
        );
    }

    #[test]
    fn group_template_rule_follows_rect_rule() {
        let css = emit_str(&rules_for("g |group| { x |rect| }\n"));
        let rect = css.find(".lini .lini-shape-rect").expect("rect rule");
        let group = css.find(".lini .lini-shape-group").expect("group rule");
        assert!(rect < group, "{}", css);
        assert!(
            css.contains("lini-shape-group { fill: var(--lini-group-fill); stroke: var(--lini-group-stroke); }"),
            "{}",
            css
        );
    }

    #[test]
    fn user_shape_rule_carries_its_paint() {
        let css = emit_str(&rules_for(
            "{ |treat:rect| fill:pink radius:5 }\nx |treat|\n",
        ));
        assert!(
            css.contains(".lini .lini-shape-treat { fill: pink; }"),
            "geometry (radius) must not ride CSS: {}",
            css
        );
    }
}

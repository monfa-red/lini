//! The output stylesheet's structural rules (SPEC §14): paint defaults stated
//! once as class rules — paint rides CSS, geometry bakes. Rules are unlayered
//! so non-browser renderers (which skip `@layer`) parse them, and scoped under
//! `.lini` so an SVG inlined into a host document restyles nothing else.
//!
//! Constants that used to inline on every element ride a class instead: wire
//! labels (`.lini-wire-label`, mirroring `.lini-text`) and markers
//! (`.lini-marker` — `fill` the wire stroke, `stroke: none`). A marker whose
//! wire is recoloured states its own `fill`; the stroked crow re-asserts its
//! paint via `style=` so the rule's `stroke: none` doesn't erase it.

use super::values::format_value;
use crate::Options;
use crate::layout::{LaidOut, PlacedNode};
use crate::resolve::{AttrMap, MarkerKind, ResolvedValue, ShapeKind, VarTable};
use std::collections::BTreeSet;

/// lini attr → CSS property. lini property names already match CSS, so this is
/// near-identity; `stroke-style` is the exception, compiling to
/// `stroke-dasharray` (a pattern that scales with `stroke-width`), so the pair
/// is translated together, not here.
pub const PAINT_PROPS: &[(&str, &str)] = &[
    ("fill", "fill"),
    ("stroke", "stroke"),
    ("stroke-width", "stroke-width"),
    ("opacity", "opacity"),
    ("color", "color"),
    ("font-family", "font-family"),
    ("font-size", "font-size"),
    ("font-weight", "font-weight"),
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

    /// The `fill` the sheet paints a `.lini-marker` with, for a marker nested in
    /// an element carrying `classes`: the base `.lini-marker` rule, overridden by
    /// the last `.lini-style-* .lini-marker` descendant rule whose style the
    /// element carries. A filled marker inlines its own `fill` only when its
    /// required colour differs from this — so a class-driven colour rides the
    /// descendant rule, and only a direct inline `stroke:` (which no rule can
    /// target) lands in `style=`.
    pub fn marker_fill(&self, classes: &[String]) -> Option<&str> {
        let mut hit = None;
        for rule in &self.rules {
            let matches = rule.class == "lini-marker"
                || rule
                    .class
                    .strip_suffix(" .lini-marker")
                    .is_some_and(|prefix| classes.iter().any(|c| c == prefix));
            if matches && let Some((_, v)) = rule.props.iter().find(|(p, _)| p == "fill") {
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
    let mut has_markers = false;
    for node in &laid.nodes {
        collect(node, &mut present, &mut used_styles, &mut has_markers);
    }
    // Wires carry styles too (same class surface as nodes), so a style used
    // only by a wire still emits its rule.
    for wire in &laid.wires {
        for style in &wire.applied_styles {
            used_styles.insert(style.as_str());
        }
        has_markers |=
            wire.markers.start != MarkerKind::None || wire.markers.end != MarkerKind::None;
    }
    let has_labels = laid.wires.iter().any(|w| !w.texts.is_empty());

    let mut rules: Vec<Rule> = Vec::new();

    // Root rule: the inherited text properties, stated once. `font-size` is a
    // layout constant, so it always formats to a literal.
    let font_size = layout_number(vars, "font-size", 14.0);
    rules.push(Rule {
        class: "lini".into(),
        props: vec![
            ("font-family".into(), live("font-family")),
            (
                "font-size".into(),
                format!("{}px", super::values::num(font_size)),
            ),
            ("font-weight".into(), live("font-weight")),
            ("color".into(), live("text-color")),
        ],
    });

    // Primitive paint defaults, fixed order — these state what the renderer's
    // per-element fallbacks used to inline. Every closed rule masks
    // `stroke-dasharray` so a container's `line:` can never bleed into
    // children through a gap in the cascade.
    const CLOSED: &[ShapeKind] = &[
        ShapeKind::Box,
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
                    ("stroke-width".into(), "2".into()),
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
                ("stroke-width".into(), "2".into()),
                ("stroke-dasharray".into(), "none".into()),
            ],
        });
    }
    if present.contains("text") {
        // A bare `<text class="lini-text">` (SPEC §13). `fill: currentColor` ties
        // the glyph colour to the inherited `color`; `stroke: none` keeps a
        // container's stroke off the glyphs; the anchor pair centres it on (x, y).
        rules.push(Rule {
            class: "lini-text".into(),
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

    // Built-in template looks (group's container wash), then user define
    // defaults in source order — paint subset only, geometry stays baked.
    for (name, attrs) in &laid.sheet.templates {
        if present.contains(name.as_str()) {
            rules.push(Rule {
                class: format!("lini-shape-{}", name),
                props: paint_props(attrs, vars, opts),
            });
        }
    }
    for (name, attrs) in &laid.sheet.defines {
        if present.contains(name.as_str()) {
            rules.push(Rule {
                class: format!("lini-shape-{}", name),
                props: paint_props(attrs, vars, opts),
            });
        }
    }

    // Element rules (`box { }`) merge into the matching shape rule (creating it
    // for paint-less templates that gain paint only via the rule).
    for (name, attrs) in &laid.sheet.element_rules {
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
            wire_paint.push(("stroke-width".into(), "2".into()));
        }
        if !wire_paint.iter().any(|(p, _)| p == "stroke-dasharray") {
            wire_paint.push(("stroke-dasharray".into(), "none".into()));
        }
        props.extend(wire_paint);
        rules.push(Rule {
            class: "lini-wire".into(),
            props,
        });

        // Line styles (`--` dashed / `..` dotted from the operator, or an
        // explicit `stroke-style:`) ride a `lini-wire-{style}` class so the dash
        // pattern is stated once, not inlined on every wire — exactly as a
        // shape's stroke rides its class. The pattern bakes the wire default
        // `stroke-width`; a wire that overrides the width inlines its own
        // pattern via the cascade diff in `render_wire`.
        let wire_width = laid
            .sheet
            .wire_defaults
            .number("stroke-width")
            .unwrap_or(2.0);
        let mut wire_styles: BTreeSet<&str> = BTreeSet::new();
        for w in &laid.wires {
            if let Some(ResolvedValue::Ident(s)) = w.attrs.get("stroke-style")
                && (s == "dashed" || s == "dotted")
            {
                wire_styles.insert(s.as_str());
            }
        }
        for style in wire_styles {
            rules.push(Rule {
                class: format!("lini-wire-{style}"),
                props: vec![(
                    "stroke-dasharray".into(),
                    super::values::dash_pattern(style, wire_width),
                )],
            });
        }
    }

    // Wire labels: the constant `<text>` paint stated once (mirrors `.lini-text`),
    // plus the baked wire font size. A label that overrides any of these inlines
    // the difference via `style=`.
    if has_labels {
        let wfs = layout_number(vars, "wire-font-size", 11.0);
        rules.push(Rule {
            class: "lini-wire-label".into(),
            props: vec![
                ("fill".into(), "currentColor".into()),
                ("stroke".into(), "none".into()),
                ("text-anchor".into(), "middle".into()),
                ("dominant-baseline".into(), "central".into()),
                ("font-size".into(), format!("{}px", super::values::num(wfs))),
                ("font-weight".into(), live("wire-font-weight")),
            ],
        });
        // The label cut's mask rects state their fill/stroke as CSS, not inline —
        // so the wire's own `stroke` can't bleed into the luminance mask, and the
        // SVG stays free of per-label paint attrs (SPEC §13). White shows the
        // wire, a black box per label punches the hole.
        rules.push(Rule {
            class: "lini-cut-bg".into(),
            props: vec![
                ("fill".into(), "white".into()),
                ("stroke".into(), "none".into()),
            ],
        });
        rules.push(Rule {
            class: "lini-cut".into(),
            props: vec![
                ("fill".into(), "black".into()),
                ("stroke".into(), "none".into()),
            ],
        });
    }

    // Markers: fill follows the wire stroke (the common default stated once),
    // `stroke: none` for the filled heads. The crow re-asserts paint inline.
    if has_markers {
        rules.push(Rule {
            class: "lini-marker".into(),
            props: vec![
                ("fill".into(), live("stroke")),
                ("stroke".into(), "none".into()),
            ],
        });
    }

    // Class rules in source order — the stylesheet's `.name { }` rules shipped
    // as CSS. After the shape/wire default rules, so a class overrides a default.
    for (name, attrs) in &laid.sheet.class_rules {
        if !used_styles.contains(name.as_str()) {
            continue;
        }
        rules.push(Rule {
            class: format!("lini-style-{}", name),
            props: paint_props(attrs, vars, opts),
        });
        // A filled marker inside an element carrying this class fills with the
        // class's stroke — one descendant rule (3 classes, so it beats the base
        // `.lini-marker`), no per-marker inline. The crow is stroked, not
        // filled, so it resolves its colour inline instead (`emit_marker`).
        if has_markers && let Some(v) = attrs.get("stroke") {
            rules.push(Rule {
                class: format!("lini-style-{} .lini-marker", name),
                props: vec![("fill".into(), format_value(v, vars, opts))],
            });
        }
    }

    rules.retain(|r| !r.props.is_empty());
    RuleSet { rules }
}

/// The stroke colour an element actually paints with — its inline `stroke`,
/// else what its classes get from the sheet (`.lini-style-*`, `.lini-wire`),
/// else the `--lini-stroke` default. A crow marker fills no descendant rule
/// (it is stroked, not filled), so it resolves its colour through this.
pub fn effective_stroke(
    attrs: &AttrMap,
    classes: &[String],
    set: &RuleSet,
    vars: &VarTable,
    opts: &Options,
) -> String {
    if let Some(v) = attrs.get("stroke") {
        return format_value(v, vars, opts);
    }
    if let Some(v) = set.provided(classes, "stroke") {
        return v.to_string();
    }
    super::values::attr_or_var(&AttrMap::default(), "stroke", "stroke", vars, opts)
}

/// The paint subset of an attr map, translated to CSS props. `stroke-style`
/// compiles jointly with `stroke-width` into `stroke-dasharray`.
pub fn paint_props(attrs: &AttrMap, vars: &VarTable, opts: &Options) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (lini, css) in PAINT_PROPS {
        if let Some(v) = attrs.get(lini) {
            let formatted = match *lini {
                "font-size" => format!("{}px", format_value(v, vars, opts)),
                _ => format_value(v, vars, opts),
            };
            out.push((css.to_string(), formatted));
        }
    }
    if attrs.get("stroke-style").is_some() {
        let width = attrs.number("stroke-width").unwrap_or(2.0);
        let dash = super::values::dasharray_value(attrs, width);
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
    has_markers: &mut bool,
) {
    present.insert(node.shape.as_str());
    for name in &node.type_chain {
        present.insert(name.as_str());
    }
    for name in &node.applied_styles {
        used_styles.insert(name.as_str());
    }
    *has_markers |= node.markers.start != MarkerKind::None || node.markers.end != MarkerKind::None;
    for child in &node.children {
        collect(child, present, used_styles, has_markers);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules_for(src: &str) -> RuleSet {
        let tokens = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(&tokens).expect("parse");
        let program = crate::resolve::resolve_with_theme(&file, &[]).expect("resolve");
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
        let css = emit_str(&rules_for("x |box|\n"));
        assert!(
            css.contains(".lini { font-family: var(--lini-font-family); font-size: 15px; font-weight: var(--lini-font-weight); color: var(--lini-text-color); }"),
            "{}",
            css
        );
    }

    #[test]
    fn shape_rules_only_for_present_types() {
        let css = emit_str(&rules_for("x |box|\n"));
        assert!(css.contains(".lini .lini-shape-box {"), "{}", css);
        assert!(!css.contains("lini-shape-oval"), "{}", css);
    }

    #[test]
    fn shape_rules_complete_over_inheritable_paint() {
        let set = rules_for("x |box|\ny |oval|\nz |line| { points: 0 0, 10 0; }\n");
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
            ".a { stroke: red; }\n.b { stroke: blue; }\n.unused { stroke: green; }\nx |box| .b .a\n",
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
                ".lini .lini-wire { fill: none; stroke: var(--lini-stroke); stroke-width: 2; stroke-dasharray: none; }"
            ),
            "{}",
            css
        );
    }

    #[test]
    fn marker_rule_states_fill_and_stroke_none() {
        // `a -> b` carries an arrow, so the shared marker rule emits once.
        let css = emit_str(&rules_for("a -> b\n"));
        assert!(
            css.contains(".lini .lini-marker { fill: var(--lini-stroke); stroke: none; }"),
            "{}",
            css
        );
        // No markers, no rule.
        let plain = emit_str(&rules_for("a - b\n"));
        assert!(!plain.contains("lini-marker"), "{}", plain);
    }

    #[test]
    fn wire_label_rule_states_constants() {
        let css = emit_str(&rules_for("a -> b \"x\"\n"));
        assert!(
            css.contains(
                ".lini .lini-wire-label { fill: currentColor; stroke: none; text-anchor: middle; dominant-baseline: central; font-size: 11px; font-weight: var(--lini-wire-font-weight); }"
            ),
            "{}",
            css
        );
        // No labels, no rule.
        let plain = emit_str(&rules_for("a -> b\n"));
        assert!(!plain.contains("lini-wire-label"), "{}", plain);
    }

    #[test]
    fn type_defaults_merge_into_shape_rule() {
        let css = emit_str(&rules_for("box { fill: lightyellow; }\nx |box|\n"));
        assert!(
            css.contains(".lini .lini-shape-box { fill: lightyellow;"),
            "{}",
            css
        );
    }

    #[test]
    fn group_template_rule_follows_rect_rule() {
        let css = emit_str(&rules_for("g |group| { x |box| }\n"));
        let rect = css.find(".lini .lini-shape-box").expect("rect rule");
        let group = css.find(".lini .lini-shape-group").expect("group rule");
        assert!(rect < group, "{}", css);
        assert!(
            css.contains("lini-shape-group { fill: var(--lini-group-fill); stroke: var(--lini-group-stroke); stroke-width: 1; stroke-dasharray:"),
            "{}",
            css
        );
    }

    #[test]
    fn user_shape_rule_carries_its_paint() {
        let css = emit_str(&rules_for(
            "treat::box { fill: pink; radius: 5; }\nx |treat|\n",
        ));
        assert!(
            css.contains(".lini .lini-shape-treat { fill: pink; }"),
            "geometry (radius) must not ride CSS: {}",
            css
        );
    }
}

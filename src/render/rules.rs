//! The output stylesheet's structural rules [SPEC 17]: paint defaults stated
//! once as class rules — paint rides CSS, geometry bakes. Rules are unlayered
//! so non-browser renderers (which skip `@layer`) parse them, and scoped under
//! `.lini` so an SVG inlined into a host document restyles nothing else.
//!
//! Constants that used to inline on every element ride a class instead: link
//! labels (`.lini-link-label`, mirroring `.lini-text`) and markers
//! (`.lini-marker` — `fill` the link stroke, `stroke: none`). A marker whose
//! link is recoloured states its own `fill`; the stroked open ER markers (crow's-foot,
//! cardinality bars / rings) flip to `fill: none; stroke: inherit` via
//! `.lini-marker-open`, pulling the line's paint off the enclosing `<g>`.

use super::values::format_value;
use crate::Options;
use crate::layout::ir::{LINK_LABEL_CLASS, SEQUENCE_MESSAGE_CLASS};
use crate::layout::{LaidOut, PlacedNode};
use crate::resolve::{AttrMap, MarkerKind, NodeKind, ResolvedValue, VarTable};
use std::collections::{BTreeSet, HashMap};

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
    ("font-style", "font-style"),
    ("text-transform", "text-transform"),
    ("text-decoration", "text-decoration"),
    ("text-shadow", "text-shadow"),
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

    /// The inline paint `style=` declarations for one element — a node `<g>` or a
    /// link `<g>` — as the **difference** from what its classes already provide:
    /// each `PAINT_PROPS` entry, then the joint `stroke-style → stroke-dasharray`
    /// pair, kept only when it differs from the class rule (so inline beats the
    /// rule, [SPEC 17]). The one place that diff lives, shared by both renderers:
    /// `value_of` resolves a prop to its value (a node aliases text `color`→`fill`),
    /// `fmt` formats it (a node's `css_value` adds `px` to `font-size`; a link's
    /// `format_value` does not).
    pub fn inline_paint_diff<'a>(
        &self,
        classes: &[String],
        attrs: &AttrMap,
        value_of: impl Fn(&str) -> Option<&'a ResolvedValue>,
        fmt: impl Fn(&str, &ResolvedValue) -> String,
    ) -> Vec<(&'static str, String)> {
        let mut decls = Vec::new();
        for (lini, css) in PAINT_PROPS {
            let Some(v) = value_of(lini) else { continue };
            let formatted = fmt(lini, v);
            if self.provided(classes, css) != Some(formatted.as_str()) {
                decls.push((*css, formatted));
            }
        }
        if attrs.get("stroke-style").is_some() {
            let width = attrs.number("stroke-width").unwrap_or(0.0);
            let dash = super::values::dasharray_value(attrs, width);
            let value = if dash.is_empty() {
                "none".to_string()
            } else {
                dash
            };
            if self.provided(classes, "stroke-dasharray") != Some(value.as_str()) {
                decls.push(("stroke-dasharray", value));
            }
        }
        decls
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
/// defs, `.style` defs (definition order), and the link/marker defaults.
pub fn build(laid: &LaidOut, opts: &Options) -> RuleSet {
    let vars = &laid.vars;
    let live = |name: &str| {
        format_value(
            &ResolvedValue::LiveVar {
                name: name.to_string(),
                raw: false,
            },
            vars,
            opts,
        )
    };

    let mut present: BTreeSet<&str> = BTreeSet::new();
    let mut used_styles: BTreeSet<&str> = BTreeSet::new();
    let mut has_markers = false;
    let mut has_open = false;
    for node in &laid.nodes {
        collect(
            node,
            &mut present,
            &mut used_styles,
            &mut has_markers,
            &mut has_open,
        );
    }
    // Links carry styles too (same class surface as nodes), so a style used
    // only by a link still emits its rule.
    for link in &laid.links {
        for style in &link.applied_styles {
            used_styles.insert(style.as_str());
        }
        has_markers |=
            link.markers.start != MarkerKind::None || link.markers.end != MarkerKind::None;
        has_open |= link.markers.start.is_open() || link.markers.end.is_open();
    }
    let has_labels = laid.links.iter().any(|w| !w.texts.is_empty());
    let label_class = |c: &str| {
        laid.links
            .iter()
            .flat_map(|w| &w.texts)
            .any(|t| t.class == c)
    };
    let has_link_labels = label_class(LINK_LABEL_CLASS);
    let has_seq_labels = label_class(SEQUENCE_MESSAGE_CLASS);

    let mut rules: Vec<Rule> = Vec::new();

    // Root rule: the inherited-text baseline, stated once. `font-family` /
    // `font-weight` / `color` default to their themeable var, but a global override
    // (in `root_text`) wins; `font-size` is the baked literal.
    let font_size = laid.sheet.root_font_size;
    let rt = &laid.sheet.root_text;
    let global = |attr: &str, var: &str| match rt.get(attr) {
        Some(v) => super::values::css_value(attr, v, vars, opts),
        None => live(var),
    };
    let mut root_props = vec![
        ("font-family".into(), global("font-family", "font-family")),
        (
            "font-size".into(),
            format!("{}px", super::values::num(font_size)),
        ),
        ("font-weight".into(), global("font-weight", "font-weight")),
        ("color".into(), global("color", "text-color")),
    ];
    // The rest ride `.lini` only when globally set — live CSS with no default.
    for attr in [
        "font-style",
        "text-transform",
        "text-decoration",
        "text-shadow",
    ] {
        if let Some(v) = rt.get(attr) {
            root_props.push((
                attr.to_string(),
                super::values::css_value(attr, v, vars, opts),
            ));
        }
    }
    rules.push(Rule {
        class: "lini".into(),
        props: root_props,
    });

    // The scene background plate: `.lini-canvas` fills with `--lini-bg`, stated as
    // a CSS rule (not a presentation attr, where `var()` is invalid) so it switches
    // live and bakes to a literal for resvg/email [SPEC 17].
    rules.push(Rule {
        class: "lini-canvas".into(),
        props: vec![("fill".into(), live("bg"))],
    });

    // Per-node paint, sourced from the generated `.lini-*` class defs — desugar
    // folded the bundles + element rules into them. Geometry stays baked; only
    // the paint subset rides CSS. Closed primitives and `line` mask
    // `stroke-dasharray` so a container's `line:` can't bleed into children.
    let class_map: HashMap<&str, &AttrMap> = laid
        .sheet
        .class_rules
        .iter()
        .map(|(n, a)| (n.as_str(), a))
        .collect();
    let shape_paint = |class: &str| -> Vec<(String, String)> {
        class_map
            .get(class)
            .map(|a| paint_props(a, vars, opts))
            .unwrap_or_default()
    };

    const CLOSED: &[NodeKind] = &[
        NodeKind::Block,
        NodeKind::Oval,
        NodeKind::Hex,
        NodeKind::Slant,
        NodeKind::Cyl,
        NodeKind::Diamond,
        NodeKind::Poly,
        NodeKind::Path,
    ];
    for kind in CLOSED {
        if present.contains(kind.as_str()) {
            let class = format!("lini-{}", kind.as_str());
            let mut props = shape_paint(&class);
            ensure_dash_none(&mut props);
            rules.push(Rule { class, props });
        }
    }
    if present.contains("line") {
        let mut props = shape_paint("lini-line");
        ensure_dash_none(&mut props);
        rules.push(Rule {
            class: "lini-line".into(),
            props,
        });
    }
    // The drawing dimension anatomy [SPEC 15.6] states its constant paint
    // once: dimension / leader linework at the drafting thin weight, and the
    // extension lines a step lighter (`--lini-stroke-light`) so the geometry
    // reads first. After the shape rules, so they win the same-specificity tie.
    if present.contains("dim-line") {
        rules.push(Rule {
            class: "lini-dim-line".into(),
            props: vec![
                ("fill".into(), "none".into()),
                ("stroke".into(), live("stroke")),
                ("stroke-width".into(), "1".into()),
            ],
        });
    }
    if present.contains("ext-line") {
        rules.push(Rule {
            class: "lini-ext-line".into(),
            props: vec![
                ("fill".into(), "none".into()),
                ("stroke".into(), live("stroke-light")),
                ("stroke-width".into(), "1".into()),
            ],
        });
    }
    if present.contains("text") {
        // A bare `<text class="lini-text">` [SPEC 17]. `fill: currentColor` ties
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
        let mut props = shape_paint("lini-icon");
        // Mask `stroke-dasharray` so a dashed container's stroke can't bleed onto
        // the icon's lines (its strokes are element-level, but dash inherits).
        ensure_dash_none(&mut props);
        rules.push(Rule {
            class: "lini-icon".into(),
            props,
        });
    }
    // Sequence tab / guard text [SPEC 13] takes its size / weight from these rules,
    // never an inline `style=`, so a diagram's many labels don't each repeat the font
    // props. (The message-label rule rides `has_seq_labels`, with the wire-label rules.)
    if present.contains("sequence-tab") {
        rules.push(Rule {
            class: "lini-sequence-tab".into(),
            props: vec![
                ("font-size".into(), "12px".into()),
                ("font-weight".into(), "bold".into()),
            ],
        });
    }
    if present.contains("sequence-guard") {
        rules.push(Rule {
            class: "lini-sequence-guard".into(),
            props: vec![
                // A touch smaller than the bold tab keyword, so the guard reads as its
                // subordinate condition.
                ("font-size".into(), "11px".into()),
                ("font-weight".into(), "normal".into()),
            ],
        });
    }

    // Template + define looks (group's wash, a define's paint), in class-def order
    // (templates then defines). Element rules are already folded into these.
    for (name, attrs) in &laid.sheet.class_rules {
        if let Some(tn) = name.strip_prefix("lini-")
            && NodeKind::parse(tn).is_none()
            && present.contains(tn)
        {
            rules.push(Rule {
                class: name.clone(),
                props: paint_props(attrs, vars, opts),
            });
        }
    }

    // Links: the `|link|` defaults stated once. Emitted *before* the style
    // rules — it is the link's default layer (like a primitive rule for a node), so
    // a link's `.style` class overrides it in the cascade [SPEC 4].
    if !laid.links.is_empty() || !laid.strays.is_empty() {
        // The link path's paint, in a fixed order (fill, stroke, width, dash) so a
        // root `link:` that overrides only some props still emits a stable rule. Font
        // props from the defaults style labels, not the `<path>`, so they're dropped.
        let defaults = &laid.sheet.link_defaults;
        let dp = paint_props(defaults, vars, opts);
        let from_defaults = |p: &str| dp.iter().find(|(k, _)| k == p).map(|(_, v)| v.clone());
        let mut props = vec![
            ("fill".into(), "none".into()),
            (
                "stroke".into(),
                from_defaults("stroke").unwrap_or_else(|| live("stroke")),
            ),
            (
                "stroke-width".into(),
                from_defaults("stroke-width").unwrap_or_else(|| "2".into()),
            ),
            (
                "stroke-dasharray".into(),
                from_defaults("stroke-dasharray").unwrap_or_else(|| "none".into()),
            ),
        ];
        for (k, v) in &dp {
            if !k.starts_with("font")
                && !matches!(k.as_str(), "stroke" | "stroke-width" | "stroke-dasharray")
            {
                props.push((k.clone(), v.clone()));
            }
        }
        rules.push(Rule {
            class: "lini-link".into(),
            props,
        });

        // Line styles (`--` dashed / `..` dotted from the operator, or an
        // explicit `stroke-style:`) ride a `lini-link-{style}` class so the dash
        // pattern is stated once, not inlined on every link — exactly as a
        // shape's stroke rides its class. The pattern bakes the link default
        // `stroke-width`; a link that overrides the width inlines its own
        // pattern via the cascade diff in `render_link`.
        let link_width = laid
            .sheet
            .link_defaults
            .number("stroke-width")
            .unwrap_or(0.0);
        let mut link_styles: BTreeSet<&str> = BTreeSet::new();
        for w in &laid.links {
            if let Some(ResolvedValue::Ident(s)) = w.attrs.get("stroke-style")
                && (s == "dashed" || s == "dotted")
            {
                link_styles.insert(s.as_str());
            }
        }
        for style in link_styles {
            rules.push(Rule {
                class: format!("lini-link-{style}"),
                props: vec![(
                    "stroke-dasharray".into(),
                    super::values::dash_pattern(style, link_width),
                )],
            });
        }
    }

    // Link labels: the constant `<text>` paint stated once per role, plus the baked
    // font size — a diagram label rides on the wire (`.lini-link-label`, the baked
    // link size), a sequence message rides above the arrow like a heading
    // (`.lini-sequence-message`, the larger `messages::LABEL_SIZE`). Two rules so
    // both coexist in one file; a label that overrides one inlines the difference.
    if has_link_labels {
        let wfs = laid.sheet.link_defaults.number("font-size").unwrap_or(11.0);
        rules.push(Rule {
            class: "lini-link-label".into(),
            props: vec![
                ("fill".into(), "currentColor".into()),
                ("stroke".into(), "none".into()),
                ("text-anchor".into(), "middle".into()),
                ("dominant-baseline".into(), "central".into()),
                ("font-size".into(), format!("{}px", super::values::num(wfs))),
                ("font-weight".into(), live("link-font-weight")),
            ],
        });
    }
    if has_seq_labels {
        rules.push(Rule {
            class: "lini-sequence-message".into(),
            props: vec![
                ("fill".into(), "currentColor".into()),
                ("stroke".into(), "none".into()),
                ("text-anchor".into(), "middle".into()),
                ("dominant-baseline".into(), "central".into()),
                // `messages::LABEL_SIZE` — larger than the wire label so messages read
                // on the time axis; kept in sync with that constant.
                ("font-size".into(), "13px".into()),
                ("font-weight".into(), "normal".into()),
            ],
        });
    }
    if has_labels {
        // The label cut's mask rects state their fill/stroke as CSS, not inline —
        // so the link's own `stroke` can't bleed into the luminance mask, and the
        // SVG stays free of per-label paint attrs [SPEC 17]. White shows the
        // link, a black box per label punches the hole.
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

    // Markers: fill follows the link stroke (the common default stated once),
    // `stroke: none` for the filled heads. The crow flips this below. A
    // drawing's dimension arrows are marker-classed nodes ([SPEC 15.6]), so
    // their presence pulls the rule in too.
    if has_markers || present.contains("marker") {
        rules.push(Rule {
            class: "lini-marker".into(),
            props: vec![
                ("fill".into(), live("stroke")),
                ("stroke".into(), "none".into()),
            ],
        });
    }

    // Inline chart labels [SPEC 14.8] take no pointer events, so hovering a labelled
    // point still reveals the point's card through the label sitting over it.
    if present.contains("chart-label") {
        rules.push(Rule {
            class: "lini-chart-label".into(),
            props: vec![("pointer-events".into(), "none".into())],
        });
    }

    // Class rules in source order — the stylesheet's `.name { }` rules shipped
    // as CSS. After the shape/link default rules, so a class overrides a default.
    for (name, attrs) in &laid.sheet.class_rules {
        if name.starts_with("lini-") || !used_styles.contains(name.as_str()) {
            continue;
        }
        // One paint vocabulary: a `.style` class states `stroke` whether it dresses a
        // node's outline or a link's wire, so its `.lini-style-*` rule paints both
        // with no per-link inline [SPEC 17].
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

    // The open ER markers (crow's-foot, bars, rings) are stroked, not filled — the
    // opposite of the other heads. A compound selector (specificity ties the
    // `.lini-style-* .lini-marker` fill descendants and wins by coming last) flips the
    // base `.lini-marker` paint; `stroke: inherit` pulls the line's colour and width off
    // the enclosing `<g>`, so an open marker's paint never needs inlining.
    if has_open {
        rules.push(Rule {
            class: "lini-marker.lini-marker-open".into(),
            props: vec![
                ("fill".into(), "none".into()),
                ("stroke".into(), "inherit".into()),
                ("stroke-linecap".into(), "round".into()),
                ("stroke-dasharray".into(), "none".into()),
            ],
        });
    }

    rules.retain(|r| !r.props.is_empty());
    RuleSet { rules }
}

/// The stroke colour an element actually paints with — its inline `stroke`,
/// else what its classes get from the sheet (`.lini-style-*`, `.lini-link`),
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
            out.push((
                css.to_string(),
                super::values::css_value(lini, v, vars, opts),
            ));
        }
    }
    if attrs.get("stroke-style").is_some() {
        let width = attrs.number("stroke-width").unwrap_or(0.0);
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

/// Ensure a closed-shape rule masks `stroke-dasharray` (so a container's `line:`
/// can't bleed into children through a gap in the cascade).
fn ensure_dash_none(props: &mut Vec<(String, String)>) {
    if !props.iter().any(|(p, _)| p == "stroke-dasharray") {
        props.push(("stroke-dasharray".into(), "none".into()));
    }
}

fn collect<'a>(
    node: &'a PlacedNode,
    present: &mut BTreeSet<&'a str>,
    used_styles: &mut BTreeSet<&'a str>,
    has_markers: &mut bool,
    has_open: &mut bool,
) {
    present.insert(node.kind.as_str());
    // An icon's optional label renders as a `lini-text`, so register the text
    // rule even though the label is not a separate Text node.
    if node.kind == NodeKind::Icon && node.label.is_some() {
        present.insert("text");
    }
    for name in &node.type_chain {
        present.insert(name.as_str());
    }
    for name in &node.applied_styles {
        used_styles.insert(name.as_str());
    }
    *has_markers |= node.markers.start != MarkerKind::None || node.markers.end != MarkerKind::None;
    *has_open |= node.markers.start.is_open() || node.markers.end.is_open();
    for child in &node.children {
        collect(child, present, used_styles, has_markers, has_open);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules_for(src: &str) -> RuleSet {
        let tokens = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(&tokens).expect("parse");
        let lowered = crate::desugar::desugar(&file).expect("desugar");
        let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
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
        let css = emit_str(&rules_for("|box#x|\n"));
        assert!(
            css.contains(".lini { font-family: var(--lini-font-family); font-size: 15px; font-weight: var(--lini-font-weight); color: var(--lini-text-color); }"),
            "{}",
            css
        );
    }

    #[test]
    fn shape_rules_only_for_present_types() {
        let css = emit_str(&rules_for("|box#x|\n"));
        assert!(css.contains(".lini .lini-box {"), "{}", css);
        assert!(!css.contains("lini-oval"), "{}", css);
    }

    #[test]
    fn shape_rules_complete_over_inheritable_paint() {
        let set = rules_for("|box#x|\n|oval#y|\n|line#z| { points: 0 0, 10 0; }\n");
        for rule in &set.rules {
            let Some(suffix) = rule.class.strip_prefix("lini-") else {
                continue;
            };
            if suffix == "text" {
                // Text masks stroke — a container's stroke must never bleed
                // into glyph outlines.
                assert!(
                    rule.props.iter().any(|(p, v)| p == "stroke" && v == "none"),
                    "text rule lacks the stroke mask"
                );
            } else if NodeKind::parse(suffix).is_some() {
                // Every primitive node rule masks `stroke-dasharray` so a
                // container's dashed `line:`/stroke can't bleed in. A template
                // (e.g. `box`) inherits the mask from its base primitive (`block`).
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
            "{ .a { stroke: red; }\n.b { stroke: blue; }\n.unused { stroke: green; } }\n|box#x| .b.a\n",
        ));
        let a = css.find(".lini .lini-style-a").expect("a rule");
        let b = css.find(".lini .lini-style-b").expect("b rule");
        assert!(a < b, "definition order: {}", css);
        assert!(!css.contains("lini-style-unused"), "{}", css);
    }

    #[test]
    fn link_rule_states_defaults() {
        let css = emit_str(&rules_for("a -> b\n"));
        assert!(
            css.contains(
                ".lini .lini-link { fill: none; stroke: var(--lini-stroke); stroke-width: 2; stroke-dasharray: none; }"
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
    fn link_label_rule_states_constants() {
        let css = emit_str(&rules_for("a -> b \"x\"\n"));
        assert!(
            css.contains(
                ".lini .lini-link-label { fill: currentColor; stroke: none; text-anchor: middle; dominant-baseline: central; font-size: 11px; font-weight: var(--lini-link-font-weight); }"
            ),
            "{}",
            css
        );
        // No labels, no rule.
        let plain = emit_str(&rules_for("a -> b\n"));
        assert!(!plain.contains("lini-link-label"), "{}", plain);
    }

    #[test]
    fn type_defaults_merge_into_shape_rule() {
        let css = emit_str(&rules_for("{ |box| { fill: lightyellow; } }\n|box#x|\n"));
        assert!(
            css.contains(".lini .lini-box { fill: lightyellow;"),
            "{}",
            css
        );
    }

    #[test]
    fn group_template_rule_follows_rect_rule() {
        let css = emit_str(&rules_for("|group#g| [ |box#x| ]\n"));
        let rect = css.find(".lini .lini-box").expect("rect rule");
        let group = css.find(".lini .lini-group").expect("group rule");
        assert!(rect < group, "{}", css);
        assert!(
            css.contains("lini-group { fill: var(--lini-group-fill); stroke: var(--lini-group-stroke); stroke-width: 1; stroke-dasharray:"),
            "{}",
            css
        );
    }

    #[test]
    fn user_shape_rule_carries_its_paint() {
        let css = emit_str(&rules_for(
            "{ |treat::box| { fill: pink; radius: 5; } }\n|treat#x|\n",
        ));
        assert!(
            css.contains(".lini .lini-treat { fill: pink; }"),
            "geometry (radius) must not ride CSS: {}",
            css
        );
    }
}

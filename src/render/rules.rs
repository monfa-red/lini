//! The stylesheet's data model and the queries the renderers run against it.
//! A [`Rule`] is one class selector plus its ordered CSS props; a [`RuleSet`]
//! is a document's rules. `emit` writes the `<style>` body; `provided` /
//! `marker_fill` answer the cascade so inline paint can be diffed against the
//! class rules (`inline_paint_diff`), and `effective_stroke` resolves a
//! stroke through the sheet. The builder that populates a `RuleSet` from a
//! laid-out document lives in `super::stylesheet`.

use super::values::format_value;
use crate::Options;
use crate::resolve::{AttrMap, ResolvedValue, VarTable};

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
        if let Some(value) = dash_value(attrs)
            && self.provided(classes, "stroke-dasharray") != Some(value.as_str())
        {
            decls.push(("stroke-dasharray", value));
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

/// The `stroke-dasharray` value for `attrs` — `stroke-style` compiled jointly
/// with `stroke-width` [SPEC 6] — or `None` when no `stroke-style` is set.
/// `"none"` when the style resolves to a solid line (no dashes).
pub(super) fn dash_value(attrs: &AttrMap) -> Option<String> {
    attrs.get("stroke-style")?;
    let width = attrs.number("stroke-width").unwrap_or(0.0);
    let dash = super::values::dasharray_value(attrs, width);
    Some(if dash.is_empty() {
        "none".to_string()
    } else {
        dash
    })
}

/// Ensure a closed-shape rule masks `stroke-dasharray` (so a container's `line:`
/// can't bleed into children through a gap in the cascade).
pub(super) fn ensure_dash_none(props: &mut Vec<(String, String)>) {
    if !props.iter().any(|(p, _)| p == "stroke-dasharray") {
        props.push(("stroke-dasharray".into(), "none".into()));
    }
}

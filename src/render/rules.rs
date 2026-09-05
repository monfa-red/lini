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
    ("text-decoration", "text-decoration"),
    ("text-shadow", "text-shadow"),
];

/// The paint a link's wire `<g>` may carry [SPEC 9/17]: a wire strokes, never
/// fills, and its labels own their text paint — shared by the inline diff's
/// retain and the `.lini-links` companion rules, so the two can never drift.
pub const LINK_WIRE_PAINT: &[&str] = &["stroke", "stroke-width", "stroke-dasharray", "opacity"];

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
    /// Append the rules to the `<style>` body, every selector headed by the
    /// figure's own `scope` class rather than the shared `.lini` — one class
    /// either way, so specificity is unchanged and host CSS overrides exactly
    /// as before [SPEC 18].
    pub fn emit(&self, out: &mut String, scope: &str) {
        for rule in &self.rules {
            if rule.props.is_empty() {
                continue;
            }
            out.push_str("    .");
            out.push_str(scope);
            if rule.class != "lini" {
                out.push_str(" .");
                out.push_str(&rule.class);
            }
            out.push_str(" {");
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

    /// The value the sheet provides for an element carrying `classes` under
    /// DOM ancestors carrying `ancestors` — the **most specific** matching
    /// rule wins, later rules win the tie within a specificity, exactly the
    /// CSS cascade the emitted `<style>` computes. Specificity is the
    /// selector's class count ([`selector_rank`]), so a compound
    /// (`"a.b"`) and a descendant (`"outer .inner"`) both beat a lone class
    /// and tie with each other. The root `.lini` rule is deliberately
    /// excluded: its props (`font-*`, `color`) are *inherited*, so a nested
    /// element's effective value comes from its nearest ancestor, not the
    /// root — diffing against the root would drop a reset-to-default that an
    /// overriding ancestor then overrides (the node must state its own value
    /// to win, exactly as `font-weight` already does by never being on root).
    pub fn provided(&self, classes: &[String], ancestors: &[String], prop: &str) -> Option<&str> {
        let mut hit: Option<(usize, &str)> = None;
        for rule in &self.rules {
            let Some(rank) = selector_rank(&rule.class, classes, ancestors) else {
                continue;
            };
            if let Some((_, v)) = rule.props.iter().find(|(p, _)| p == prop)
                && hit.is_none_or(|(best, _)| rank >= best)
            {
                hit = Some((rank, v.as_str()));
            }
        }
        hit.map(|(_, v)| v)
    }

    /// The inline paint `style=` declarations for one element — a node `<g>` or a
    /// link `<g>` — as the **difference** from what its classes already provide:
    /// each `PAINT_PROPS` entry, then the joint `stroke-style → stroke-dasharray`
    /// pair, kept only when it differs from the class rule (so inline beats the
    /// rule, [SPEC 18]). The one place that diff lives, shared by both renderers:
    /// `value_of` resolves a prop to its value (a node aliases text `color`→`fill`),
    /// `fmt` formats it (a node's `css_value` adds `px` to `font-size`; a link's
    /// `format_value` does not).
    ///
    /// **Paint contract.** Every *class-styled* element — a node `<g>`, a link
    /// `<g>`, a text leaf (via `node_style_attr` / `text_paint_attr`) — states its
    /// fill / stroke / font paint *only* through this diff. No renderer hand-writes
    /// a paint declaration on such an element outside it (that whack-a-mole is what
    /// this ended). The inline paints that remain in `render/` are all elements
    /// that carry no class rule and so cannot diff against one: icon role groups,
    /// drawing-chrome geometry, `<defs>` gradient / hatch bodies, a gutter rect's
    /// varying `fill`, and the marker / stray diagnostics.
    pub fn inline_paint_diff<'a>(
        &self,
        classes: &[String],
        ancestors: &[String],
        attrs: &AttrMap,
        value_of: impl Fn(&str) -> Option<&'a ResolvedValue>,
        fmt: impl Fn(&str, &ResolvedValue) -> String,
    ) -> Vec<(&'static str, String)> {
        let mut decls = Vec::new();
        for (lini, css) in PAINT_PROPS {
            let Some(v) = value_of(lini) else { continue };
            let formatted = fmt(lini, v);
            if self.provided(classes, ancestors, css) != Some(formatted.as_str()) {
                decls.push((*css, formatted));
            }
        }
        if let Some(value) = dash_value(attrs)
            && self.provided(classes, ancestors, "stroke-dasharray") != Some(value.as_str())
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
    ///
    /// A marker *is* an element carrying `.lini-marker` inside its line's `<g>`,
    /// so this is [`Self::provided`] asked that question — never a second
    /// cascade walk beside it.
    pub fn marker_fill(&self, classes: &[String]) -> Option<&str> {
        self.provided(&[MARKER_CLASS.to_string()], classes, "fill")
    }
}

/// The class every filled marker head wears — what `emit_marker` writes and
/// what the sheet's marker rules key on.
pub const MARKER_CLASS: &str = "lini-marker";

/// How specifically a rule's selector matches an element — its **class count**,
/// or `None` when it does not match at all. `class` is one of the three shapes a
/// [`Rule`] holds: a lone class (`a`), a compound on one element (`a.b`, every
/// unit worn by the element), or a descendant (`outer .inner`, the inner part
/// compound-capable and the outer worn by an ancestor). CSS scores all three by
/// class count, so counting the units is the cascade, not an approximation of it.
fn selector_rank(class: &str, classes: &[String], ancestors: &[String]) -> Option<usize> {
    let worn = |c: &str| classes.iter().any(|x| x == c);
    let (outer, inner) = match class.split_once(" .") {
        Some((outer, inner)) => (Some(outer), inner),
        None => (None, class),
    };
    if let Some(outer) = outer
        && !ancestors.iter().any(|c| c == outer)
    {
        return None;
    }
    inner
        .split('.')
        .all(worn)
        .then(|| inner.split('.').count() + usize::from(outer.is_some()))
}

/// The stroke colour an element actually paints with — its inline `stroke`,
/// else what its classes get from the sheet (`.lini-style-*`, `.lini-link`),
/// else the `--lini-stroke` default. A crow marker fills no descendant rule
/// (it is stroked, not filled), so it resolves its colour through this.
pub fn effective_stroke(
    attrs: &AttrMap,
    classes: &[String],
    ancestors: &[String],
    set: &RuleSet,
    vars: &VarTable,
    opts: &Options,
) -> String {
    if let Some(v) = attrs.get("stroke") {
        return format_value(v, vars, opts);
    }
    if let Some(v) = set.provided(classes, ancestors, "stroke") {
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

//! The output stylesheet's structural rules [SPEC 18]: paint defaults stated
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
//!
//! `build` collects which shapes / styles / markers a document actually uses,
//! then appends each rule family in cascade order (`families`); the model the
//! rules live in — `Rule` / `RuleSet` and its queries — is `super::rules`.

mod families;

use super::rules::{PAINT_PROPS, Rule, RuleSet, dash_value};
use super::values::css_value;
use crate::Options;
use crate::layout::ir::{LINK_LABEL_CLASS, SEQUENCE_MESSAGE_CLASS};
use crate::layout::{LaidOut, PlacedNode};
use crate::resolve::{AttrMap, MarkerKind, NodeKind, VarTable};
use std::collections::BTreeSet;

/// Build the document's structural rules: root inherited-text rule, per-shape
/// paint defaults (only shapes present), built-in template looks, user shape
/// defs, `.style` defs (definition order), and the link/marker defaults.
pub fn build(laid: &LaidOut, opts: &Options) -> RuleSet {
    let vars = &laid.vars;

    let mut seen = Presence::default();
    // The junction dots ride with the wires rather than in the scene tree
    // [SPEC 16.5], and they wear the classes whose rules paint them — so the
    // presence walk reads both, or `.lini-junction` would never emit.
    for node in laid.nodes.iter().chain(&laid.junctions) {
        collect(node, &mut seen);
    }
    // Links carry styles too (same class surface as nodes), so a style used
    // only by a link still emits its rule.
    for link in &laid.links {
        for style in &link.applied_styles {
            seen.used_styles.insert(style.as_str());
        }
        seen.has_markers |=
            link.markers.start != MarkerKind::None || link.markers.end != MarkerKind::None;
        seen.has_open |= link.markers.start.is_open() || link.markers.end.is_open();
    }
    let Presence {
        present,
        dim_roles,
        marker_hosts,
        used_styles,
        has_markers,
        has_open,
        has_gutters,
    } = seen;
    // The mask rules' wearer test [SPEC 18]: a label only cuts a wire whose
    // path it actually reaches, so "any link has texts" would emit `.lini-cut`
    // / `.lini-cut-bg` for a document where no mask is ever produced. The
    // renderer's own hit test answers it.
    let has_cuts = super::links::any_label_cut(laid);
    let label_class = |c: &str| {
        laid.links
            .iter()
            .flat_map(|w| &w.texts)
            .any(|t| t.class == c)
    };
    let has_link_labels = label_class(LINK_LABEL_CLASS);
    let has_seq_labels = label_class(SEQUENCE_MESSAGE_CLASS);

    let mut rules: Vec<Rule> = Vec::new();

    families::build_frame_rules(&mut rules, laid, vars, opts);
    families::build_shape_rules(&mut rules, laid, &present, vars, opts);
    families::build_symbol_chrome_rules(&mut rules, laid, &present, vars, opts);
    families::build_sequence_text_rules(&mut rules, &present);
    families::build_gutter_rule(&mut rules, has_gutters);
    families::build_halo_rules(&mut rules, laid, present.contains("halo"), has_cuts);
    families::build_template_rules(&mut rules, laid, &present, vars, opts);
    families::build_projection_rule(&mut rules, laid, present.contains("projection"), vars, opts);
    families::build_link_rules(&mut rules, laid, vars, opts);
    families::build_link_label_rules(
        &mut rules,
        laid,
        has_link_labels,
        has_seq_labels,
        has_cuts,
        vars,
        opts,
    );
    families::build_marker_rules(
        &mut rules,
        laid,
        &present,
        &marker_hosts,
        has_markers,
        vars,
        opts,
    );
    families::build_dim_tier_rules(&mut rules, laid, &dim_roles, vars, opts);
    families::build_plane_chrome_rules(&mut rules, laid, &present, vars, opts);
    families::build_style_class_rules(&mut rules, laid, &used_styles, has_markers, vars, opts);
    families::build_descendant_rules(&mut rules, laid, &present, vars, opts);
    families::build_open_marker_rule(&mut rules, has_open);

    rules.retain(|r| !r.props.is_empty());
    RuleSet { rules }
}

/// The paint subset of an attr map, translated to CSS props. `stroke-style`
/// compiles jointly with `stroke-width` into `stroke-dasharray`.
pub fn paint_props(attrs: &AttrMap, vars: &VarTable, opts: &Options) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (lini, css) in PAINT_PROPS {
        if let Some(v) = attrs.get(lini) {
            out.push((css.to_string(), css_value(lini, v, vars, opts)));
        }
    }
    if let Some(dash) = dash_value(attrs) {
        out.push(("stroke-dasharray".to_string(), dash));
    }
    out
}

/// What the sheet's wearer tests read off the placed tree — one walk, one
/// struct, so a new test is a field rather than another out-parameter.
#[derive(Default)]
struct Presence<'a> {
    /// Every `.lini-*` type / chrome class worn anywhere (sans the prefix).
    present: BTreeSet<&'a str>,
    /// The chrome roles worn **beside** the `(-)` tier class — what the tier's
    /// compound rules key on [SPEC 18].
    dim_roles: BTreeSet<&'a str>,
    /// The classes of nodes that actually carry a marker — what a
    /// `"{host} .lini-marker"` companion rule keys on, so a head's fill rides
    /// the sheet only where a head exists.
    marker_hosts: BTreeSet<&'a str>,
    used_styles: BTreeSet<&'a str>,
    has_markers: bool,
    has_open: bool,
    has_gutters: bool,
}

fn collect<'a>(node: &'a PlacedNode, seen: &mut Presence<'a>) {
    seen.has_gutters |= !node.gutters.is_empty();
    seen.present.insert(node.kind.as_str());
    // A chrome node in the `(-)` tier registers its role there too, so the
    // tier's compound rules emit for exactly the roles a dimension lowers.
    let tier = crate::layout::drawing::annotate::DIM_TIER;
    if node.type_chain.iter().any(|t| t == tier) {
        seen.dim_roles
            .extend(node.type_chain.iter().map(String::as_str));
    }
    // A line with baked crossing-halo cuts registers the `halo` chrome type,
    // so its base cut paint and any `|halo|` user rule emit [SPEC 15.7].
    if node.attrs.get("halo").is_some() {
        seen.present.insert("halo");
    }
    // An icon's optional label renders as a `lini-text`, so register the text
    // rule even though the label is not a separate Text node.
    if node.kind == NodeKind::Icon && node.label.is_some() {
        seen.present.insert("text");
    }
    for name in &node.type_chain {
        seen.present.insert(name.as_str());
    }
    for name in &node.applied_styles {
        seen.used_styles.insert(name.as_str());
    }
    let marked = node.markers.start != MarkerKind::None || node.markers.end != MarkerKind::None;
    if marked {
        seen.marker_hosts
            .extend(node.type_chain.iter().map(String::as_str));
    }
    seen.has_markers |= marked;
    seen.has_open |= node.markers.start.is_open() || node.markers.end.is_open();
    for child in &node.children {
        collect(child, seen);
    }
}

#[cfg(test)]
mod tests;

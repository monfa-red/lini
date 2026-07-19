//! The per-family rule sub-builders `build` appends in cascade order: the root
//! inherited-text rule and canvas plate, per-shape paint defaults, the
//! structural text / sequence / gutter rules, template + define looks, the link
//! defaults, marker rules, and the link-label rules. Each pushes onto the shared
//! `rules` Vec; kept in one place so their emission order stays byte-identical.

use super::super::rules::{Rule, ensure_dash_none};
use super::super::values::{css_value, dash_pattern, format_value, num};
use super::paint_props;
use crate::Options;
use crate::layout::LaidOut;
use crate::resolve::{AttrMap, NodeKind, ResolvedValue, VarTable};
use std::collections::{BTreeSet, HashMap};

/// Emit a generated class's default paint [SPEC 17] — the look for a class the
/// engine synthesises (the projection line, the crossing halo and its white
/// ground, the dimension chrome, the drafting heads) with no desugar-visible
/// instance to fold a bundle into. One guard for all six: emit only when the
/// class is `present` (something wears it) **and** no authored `|class| { }` rule
/// already forced it into `class_rules` (where `build_template_rules` then dresses
/// it). This replaces three prose-coupled guards — scan-for-authored,
/// rely-on-emission-order, and the `has_labels` dedup flag — so cascade order
/// stops being the mechanism that lets an authored rule win.
fn emit_generated_default(
    rules: &mut Vec<Rule>,
    laid: &LaidOut,
    class: &str,
    present: bool,
    props: Vec<(String, String)>,
) {
    let authored = laid.sheet.class_rules.iter().any(|(n, _)| n == class);
    if present && !authored {
        rules.push(Rule {
            class: class.into(),
            props,
        });
    }
}

/// The root inherited-text baseline (`.lini`) and the scene background plate
/// (`.lini-canvas`).
pub(super) fn build_frame_rules(
    rules: &mut Vec<Rule>,
    laid: &LaidOut,
    vars: &VarTable,
    opts: &Options,
) {
    // Root rule: the inherited-text baseline, stated once. `font-family` /
    // `font-weight` / `color` default to their themeable var, but a global override
    // (in `root_text`) wins; `font-size` is the baked literal.
    let font_size = laid.sheet.root_font_size;
    let rt = &laid.sheet.root_text;
    let global = |attr: &str, var: &str| match rt.get(attr) {
        Some(v) => css_value(attr, v, vars, opts),
        None => live(var, vars, opts),
    };
    let mut root_props = vec![
        ("font-family".into(), global("font-family", "font-family")),
        ("font-size".into(), format!("{}px", num(font_size))),
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
            root_props.push((attr.to_string(), css_value(attr, v, vars, opts)));
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
        props: vec![("fill".into(), live("bg", vars, opts))],
    });
}

/// Per-node paint defaults (only for shapes present), plus the structural
/// drawing-anatomy, text, and sequence-text rules.
pub(super) fn build_shape_rules(
    rules: &mut Vec<Rule>,
    laid: &LaidOut,
    present: &BTreeSet<&str>,
    vars: &VarTable,
    opts: &Options,
) {
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
        NodeKind::Sketch,
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
    emit_generated_default(
        rules,
        laid,
        "lini-dim-line",
        present.contains("dim-line"),
        vec![
            ("fill".into(), "none".into()),
            ("stroke".into(), live("stroke-dark", vars, opts)),
            ("stroke-width".into(), "1".into()),
        ],
    );
    emit_generated_default(
        rules,
        laid,
        "lini-ext-line",
        present.contains("ext-line"),
        vec![
            ("fill".into(), "none".into()),
            ("stroke".into(), live("stroke-light", vars, opts)),
            ("stroke-width".into(), "1".into()),
        ],
    );
    // Annotation text reads at the caption size [SPEC 15.6/17]: stated once
    // here, so no dimension / leader / callout leaf inlines it (only a `tol:`
    // deviation or a restyled link overrides).
    emit_generated_default(
        rules,
        laid,
        "lini-dim-text",
        present.contains("dim-text"),
        vec![
            (
                "font-size".into(),
                format!("{}px", num(crate::ledger::consts::DRAWING_LINK_FONT_SIZE)),
            ),
            ("font-weight".into(), "normal".into()),
        ],
    );
    if present.contains("text") {
        // A bare `<text class="lini-text">` [SPEC 17]. `fill: currentColor` ties
        // the glyph colour to the inherited `color`; `stroke: none` keeps a
        // container's stroke off the glyphs; `text-anchor: middle` centres it
        // on x. Vertical centring is a baked `dy` on the element — cap-height
        // optical centring [SPEC 5], not `dominant-baseline` (renderers
        // disagree on it; a baked offset is faithful everywhere).
        rules.push(Rule {
            class: "lini-text".into(),
            props: vec![
                ("fill".into(), "currentColor".into()),
                ("stroke".into(), "none".into()),
                ("text-anchor".into(), "middle".into()),
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
}

/// Sequence tab / guard text [SPEC 13] — size / weight stated once, never inline.
pub(super) fn build_sequence_text_rules(rules: &mut Vec<Rule>, present: &BTreeSet<&str>) {
    // Sequence tab / guard text [SPEC 13] takes its size / weight from these rules,
    // never an inline `style=`, so a diagram's many labels don't each repeat the font
    // props. (The message-label rule rides `has_seq_labels`, with the wire-label rules.)
    if present.contains("chart-title") {
        // A chart / pie title [SPEC 14.6]: stated once, so no title inlines
        // its font; semibold — the chrome weight, a step under bold.
        rules.push(Rule {
            class: "lini-chart-title".into(),
            props: vec![
                (
                    "font-size".into(),
                    format!("{}px", num(crate::layout::chart::metrics::TITLE_SIZE)),
                ),
                ("font-weight".into(), "600".into()),
            ],
        });
    }
    if present.contains("sequence-tab") {
        rules.push(Rule {
            class: "lini-sequence-tab".into(),
            props: vec![
                ("font-size".into(), "12px".into()),
                ("font-weight".into(), "600".into()),
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
}

/// A `gap-fill` gutter rect's `stroke: none`, stated once rather than per rect.
pub(super) fn build_gutter_rule(rules: &mut Vec<Rule>, has_gutters: bool) {
    // A `gap-fill` gutter rect states its fill inline (it varies per container) but
    // never a stroke — the container's border can't bleed onto it [SPEC 11]. Stated
    // once here, not `stroke="none"` on every gutter rect.
    if has_gutters {
        rules.push(Rule {
            class: "lini-gutter".into(),
            props: vec![("stroke".into(), "none".into())],
        });
    }
}

/// Template + define looks (a group's wash, a define's paint), in class-def order.
pub(super) fn build_template_rules(
    rules: &mut Vec<Rule>,
    laid: &LaidOut,
    present: &BTreeSet<&str>,
    vars: &VarTable,
    opts: &Options,
) {
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
}

/// The generated projection line's default paint [SPEC 8/15.8] — the thin
/// support-tone `|projection|` line a sheet's cross-view link lowers to. It has
/// no desugar-visible instance (it is generated at layout), so its class def is
/// emitted here when a line baked, exactly as the halo's cut rule is — **unless**
/// an authored `|projection| { }` rule already forced it present at desugar, in
/// which case that (template default merged with the override) rides
/// `build_template_rules`. Its paint has one home — `template_bundle("projection")`,
/// resolved by `projection_default_attrs` — never restated here.
pub(super) fn build_projection_rule(
    rules: &mut Vec<Rule>,
    laid: &LaidOut,
    present: bool,
    vars: &VarTable,
    opts: &Options,
) {
    let props = paint_props(
        &crate::ledger::defaults::projection_default_attrs(),
        vars,
        opts,
    );
    emit_generated_default(rules, laid, "lini-projection", present, props);
}

/// The `|link|` defaults (`.lini-link`) and the per-operator dash styles.
pub(super) fn build_link_rules(
    rules: &mut Vec<Rule>,
    laid: &LaidOut,
    vars: &VarTable,
    opts: &Options,
) {
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
                from_defaults("stroke").unwrap_or_else(|| live("stroke", vars, opts)),
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
                props: vec![("stroke-dasharray".into(), dash_pattern(style, link_width))],
            });
        }
    }
}

/// Link-label paint stated once per role (`.lini-link-label`,
/// `.lini-sequence-message`) plus the label-cut mask rects.
pub(super) fn build_link_label_rules(
    rules: &mut Vec<Rule>,
    laid: &LaidOut,
    has_link_labels: bool,
    has_seq_labels: bool,
    has_labels: bool,
    vars: &VarTable,
    opts: &Options,
) {
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
                ("font-size".into(), format!("{}px", num(wfs))),
                ("font-weight".into(), live("link-font-weight", vars, opts)),
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
                // Larger than the wire label so messages read on the time axis —
                // the size layout measured with.
                (
                    "font-size".into(),
                    format!("{}px", num(crate::layout::sequence::messages::LABEL_SIZE)),
                ),
                ("font-weight".into(), "normal".into()),
            ],
        });
    }
    // The label cut's mask rects state their fill/stroke as CSS, not inline — so
    // the link's own `stroke` can't bleed into the luminance mask, and the SVG
    // stays free of per-label paint attrs [SPEC 17]. White shows the link, a black
    // box per label punches the hole. When labels exist the white ground seats
    // here beside `.lini-cut` (the halo path defers it via its `present` flag).
    emit_generated_default(rules, laid, "lini-cut-bg", has_labels, cut_bg_props());
    emit_generated_default(
        rules,
        laid,
        "lini-cut",
        has_labels,
        vec![
            ("fill".into(), "black".into()),
            ("stroke".into(), "none".into()),
        ],
    );
}

/// The knockout mask's white ground paint [SPEC 17] — one home for the `.lini-cut-bg`
/// props, shared by the label-cut and crossing-halo consumers.
fn cut_bg_props() -> Vec<(String, String)> {
    vec![
        ("fill".into(), "white".into()),
        ("stroke".into(), "none".into()),
    ]
}

/// The crossing-halo cut paint [SPEC 15.7]: the black `.lini-halo` mask stroke.
/// An authored `|halo| { stroke: none }` removes every crossing break scope-wide
/// (like all chrome) by suppressing this default — the `emit_generated_default`
/// ¬authored guard, no longer cascade order. Without link labels the white ground
/// rides here too; with them it keeps its seat beside `.lini-cut` (deferred via
/// the `present` flag).
pub(super) fn build_halo_rules(
    rules: &mut Vec<Rule>,
    laid: &LaidOut,
    has_halos: bool,
    has_labels: bool,
) {
    emit_generated_default(
        rules,
        laid,
        "lini-cut-bg",
        has_halos && !has_labels,
        cut_bg_props(),
    );
    emit_generated_default(
        rules,
        laid,
        "lini-halo",
        has_halos,
        vec![("stroke".into(), "black".into())],
    );
}

/// The base marker paint (`.lini-marker`), the drafting-head variants, and the
/// chart-label pointer-events rule.
pub(super) fn build_marker_rules(
    rules: &mut Vec<Rule>,
    laid: &LaidOut,
    present: &BTreeSet<&str>,
    has_markers: bool,
    vars: &VarTable,
    opts: &Options,
) {
    // Markers: fill follows the link stroke (the common default stated once),
    // `stroke: none` for the filled heads. The crow flips this below. A
    // drawing's dimension arrows are marker-classed nodes ([SPEC 15.6]), so
    // their presence pulls the rule in too.
    if has_markers || present.contains("marker") {
        rules.push(Rule {
            class: "lini-marker".into(),
            props: vec![
                ("fill".into(), live("stroke", vars, opts)),
                ("stroke".into(), "none".into()),
            ],
        });
    }
    // The drafting heads read the geometry tone [SPEC 10.1]: the slender dim
    // arrow and the datum triangle at full black/white, after `.lini-marker`
    // so the variant wins the same-specificity tie.
    for variant in ["marker-dim", "marker-datum"] {
        emit_generated_default(
            rules,
            laid,
            &format!("lini-{variant}"),
            present.contains(variant),
            vec![("fill".into(), live("stroke-dark", vars, opts))],
        );
    }

    // Inline chart labels [SPEC 14.8] take no pointer events, so hovering a labelled
    // point still reveals the point's card through the label sitting over it.
    if present.contains("chart-label") {
        rules.push(Rule {
            class: "lini-chart-label".into(),
            props: vec![("pointer-events".into(), "none".into())],
        });
    }
}

/// The `.style` class rules in source order, plus the filled-marker descendant
/// rule each stroke-bearing style contributes.
pub(super) fn build_style_class_rules(
    rules: &mut Vec<Rule>,
    laid: &LaidOut,
    used_styles: &BTreeSet<&str>,
    has_markers: bool,
    vars: &VarTable,
    opts: &Options,
) {
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
}

/// The open ER markers (`.lini-marker.lini-marker-open`) — stroked, not filled.
pub(super) fn build_open_marker_rule(rules: &mut Vec<Rule>, has_open: bool) {
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
}

/// A live-var reference formatted for CSS — `var(--lini-name)` live, its baked
/// literal for resvg / email.
fn live(name: &str, vars: &VarTable, opts: &Options) -> String {
    format_value(
        &ResolvedValue::LiveVar {
            name: name.to_string(),
            raw: false,
        },
        vars,
        opts,
    )
}

/// Two-class descendant rules (the generated mindmap garnish and scoped
/// engine rules among them), stated as real CSS so a reused look never
/// inlines on every wearer [SPEC 17]. Node `<g>`s nest as the scene tree, so
/// `.outer .inner` matches them natively; wires render flat under the
/// `.lini-links` layer, so a rule whose inner class a link wears also emits a
/// `.lini-links .inner` companion (resolve already enforced the outer scoping
/// — generated inner classes exist only inside their scope). The class-diff
/// credits both through the same composite match ([`super::super::rules::RuleSet::provided`]).
pub(super) fn build_descendant_rules(
    rules: &mut Vec<Rule>,
    laid: &LaidOut,
    present: &BTreeSet<&str>,
    vars: &VarTable,
    opts: &Options,
) {
    // `present` holds bare generated-class names (sans `lini-`); a user class
    // rides `lini-style-*` on its wearers and is checked against that spelling.
    let node_wears = |class: &str| match class.strip_prefix("lini-") {
        Some(bare) => present.contains(bare),
        None => false,
    };
    let link_wears = |inner: &str| {
        laid.links.iter().any(|w| {
            w.applied_styles
                .iter()
                .any(|s| s == inner || format!("lini-style-{s}") == inner)
        })
    };
    for (outer, inner, attrs) in &laid.sheet.descendant_rules {
        let props = paint_props(attrs, vars, opts);
        if props.is_empty() {
            continue;
        }
        if node_wears(outer) && node_wears(inner) {
            rules.push(Rule {
                class: format!("{outer} .{inner}"),
                props: props.clone(),
            });
        }
        if link_wears(inner) {
            // A wire strokes, never fills: the companion keeps the wire-paint
            // whitelist, or the card tint's wash would flood the path.
            let wire_props: Vec<(String, String)> = props
                .into_iter()
                .filter(|(p, _)| super::super::rules::LINK_WIRE_PAINT.contains(&p.as_str()))
                .collect();
            if !wire_props.is_empty() {
                rules.push(Rule {
                    class: format!("lini-links .{inner}"),
                    props: wire_props,
                });
            }
        }
    }
}

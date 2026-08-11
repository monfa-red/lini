use super::*;

fn rules_for(src: &str) -> RuleSet {
    let tokens = crate::lexer::lex(src).expect("lex");
    let file = crate::syntax::parser::parse(src, &tokens).expect("parse");
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
            ".lini .lini-link-label { fill: currentColor; stroke: none; text-anchor: middle; font-size: 11px; font-weight: var(--lini-link-font-weight); }"
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

/// A `|group| { layout: schematic }` nested in a plain document: the root's
/// `.lini-link` rule cannot state the scope's wire dress, so it rides the
/// generated class the wires wear [SPEC 16.5/18].
const NESTED_SHEET: &str = "{ |bay::group| { layout: schematic } }\n|bay#b| [\n  |R#R1| \"1k\"\n  |R#R2| \"2k\"\n  R1.p2 - R2.p1\n]\n";

#[test]
fn a_nested_schematic_scope_states_its_wire_dress_once() {
    let css = emit_str(&rules_for(NESTED_SHEET));
    assert!(
        css.contains(
            ".lini .lini-links .lini-schematic-wire { stroke: var(--lini-wire); stroke-width: 1.5; }"
        ),
        "{css}"
    );
}

#[test]
fn a_root_schematic_scope_needs_no_wire_class() {
    // `.lini-link` already carries the dress there, so no second rule and no
    // class with a dead rule behind it.
    let css = emit_str(&rules_for(
        "{ layout: schematic }\n|R#R1| \"1k\"\n|R#R2| \"2k\"\nR1.p2 - R2.p1\n",
    ));
    assert!(!css.contains("lini-schematic-wire"), "{css}");
    assert!(
        css.contains(".lini .lini-link { fill: none; stroke: var(--lini-wire); stroke-width: 1.5;"),
        "{css}"
    );
}

#[test]
fn a_restyled_link_states_its_marker_fill_once() {
    // A head fills with the wire it caps: the document's `|-|` colour rides
    // one `.lini-link .lini-marker` rule, never a `fill` per head [SPEC 18].
    let css = emit_str(&rules_for("{ |-| { stroke: red } }\na -> b\nb -> c\n"));
    assert!(
        css.contains(".lini .lini-link .lini-marker { fill: red; }"),
        "{css}"
    );
    // Undressed links leave the base rule alone — no redundant companion.
    let plain = emit_str(&rules_for("a -> b\n"));
    assert!(!plain.contains(".lini-link .lini-marker"), "{plain}");
}

#[test]
fn drawing_chrome_rules_carry_the_documents_annotation_tone() {
    // A recoloured `|-|` states the dimension anatomy's colour in the sheet,
    // so no chrome node inlines it [SPEC 15.6/18].
    let css = emit_str(&rules_for(
        "{ |-| { stroke: red } }\n|drawing#d| [\n  |rect#p| { width: 40; height: 20 }\n  p:left (-) p:right\n]\n",
    ));
    for rule in [
        ".lini .lini-dim-line { fill: none; stroke: red; stroke-width: 1; }",
        ".lini .lini-ext-line { fill: none; stroke: red; stroke-width: 1; }",
        ".lini .lini-marker-dim { fill: red; }",
    ] {
        assert!(css.contains(rule), "missing {rule}: {css}");
    }
}

#[test]
fn the_cut_rules_wait_for_an_actual_cut() {
    // A wire label that reaches its wire punches a mask, so the mask rects'
    // rules emit…
    let cut = emit_str(&rules_for("a -> b \"x\"\n"));
    assert!(
        cut.contains(".lini .lini-cut-bg {") && cut.contains(".lini .lini-cut {"),
        "{cut}"
    );
    // …but a sequence's messages ride *above* their arrows and cut nothing,
    // so neither rule may emit with nobody to wear it [SPEC 18].
    let seq = emit_str(&rules_for(
        "{ layout: sequence }\n|box#a| \"A\"\n|box#b| \"B\"\na -> b \"hi\"\n",
    ));
    assert!(!seq.contains("lini-cut"), "{seq}");
}

/// A drawing with a plain dimension, a leader, a GD&T frame, and a datum — the
/// chrome-rule fixtures below all read it, so each states only what it tests.
const CHROME: &str = concat!(
    "|drawing#d| [\n",
    "  |rect#p| { width: 40; height: 20 }\n",
    "  p:left (-) p:right\n",
    "  p:top <- \"note\"\n",
    "  |feature-control#f| \"flatness\" { tol: 0.1; translate: 0 40 }\n",
    "  |datum#dm| \"A\" { translate: 0 60 }\n",
    "]\n",
);

#[test]
fn the_dimension_tier_states_its_chrome_paint_as_compound_rules() {
    // `(-)` dresses dimensions alone, and the chrome roles are shared with the
    // leaders — so the tier's paint rides one compound rule per role, keyed on
    // the class only a dimension's chrome wears [SPEC 4/15.6/18].
    let css = emit_str(&rules_for(&format!(
        "{{ (-) {{ stroke: blue }} }}\n{CHROME}"
    )));
    for rule in [
        ".lini .lini-dim-line.lini-dim { stroke: blue; }",
        ".lini .lini-ext-line.lini-dim { stroke: blue; }",
        ".lini .lini-marker-dim.lini-dim { fill: blue; }",
    ] {
        assert!(css.contains(rule), "missing {rule}: {css}");
    }
    // The leaders keep the document tone — the base rules are untouched.
    assert!(
        css.contains(
            ".lini .lini-dim-line { fill: none; stroke: var(--lini-stroke-dark); stroke-width: 1; }"
        ),
        "{css}"
    );
}

#[test]
fn an_undressed_dimension_tier_emits_no_rule() {
    // No `(-)` restyle, no tier: the class would be dead chrome [SPEC 18].
    let css = emit_str(&rules_for(CHROME));
    assert!(
        !css.contains(".lini-dim {") && !css.contains(".lini-dim "),
        "{css}"
    );
    assert!(!css.contains(".lini-dim-line.lini-dim"), "{css}");
    // …and a `(-)` that repaints nothing is no restyle either.
    let sized = emit_str(&rules_for(&format!(
        "{{ (-) {{ clearance: 20 }} }}\n{CHROME}"
    )));
    assert!(!sized.contains(".lini-dim-line.lini-dim"), "{sized}");
}

#[test]
fn drafting_symbol_chrome_states_its_dress_once() {
    // A frame's compartments, the datum plate, and the glyph linework are
    // stamped identically on every wearer, so each dress rides one rule.
    let css = emit_str(&rules_for(CHROME));
    for rule in [
        ".lini .lini-frame-cell { fill: var(--lini-bg); stroke: var(--lini-stroke-dark); stroke-width: 1; }",
        ".lini .lini-frame-plate { fill: var(--lini-bg); stroke: none; stroke-width: 0; }",
        ".lini .lini-drafting-glyph { fill: none; stroke: var(--lini-stroke-dark); stroke-width: 1; }",
    ] {
        assert!(css.contains(rule), "missing {rule}: {css}");
    }
    // A drawing with no drafting symbol grows none of them.
    let bare = emit_str(&rules_for(
        "|drawing#d| [\n  |rect#p| { width: 40; height: 20 }\n  p:left (-) p:right\n]\n",
    ));
    assert!(
        !bare.contains("lini-frame-cell") && !bare.contains("lini-drafting-glyph"),
        "{bare}"
    );
}

#[test]
fn a_leaders_own_head_fills_through_the_chrome_companion() {
    // A `*-` dot is a core marker inside its leader's `<g>`, so it takes the
    // linework tone through one companion rule, never a `fill` per head.
    let css = emit_str(&rules_for(
        "{ |-| { stroke: red } }\n|drawing#d| [\n  |rect#p| { width: 40; height: 20 }\n  p:top *- \"note\"\n]\n",
    ));
    assert!(
        css.contains(".lini .lini-dim-line .lini-marker { fill: red; }"),
        "{css}"
    );
    // The annotation tone is `--stroke-dark`, never the link stroke the base
    // `.lini-marker` rule states, so the companion earns its seat undressed too.
    let plain = emit_str(&rules_for(
        "|drawing#d| [\n  |rect#p| { width: 40; height: 20 }\n  p:top *- \"note\"\n]\n",
    ));
    assert!(
        plain.contains(".lini .lini-dim-line .lini-marker { fill: var(--lini-stroke-dark); }"),
        "{plain}"
    );
    // A drawing whose leaders carry no head of their own grows no companion.
    let headless = emit_str(&rules_for(
        "|drawing#d| [\n  |rect#p| { width: 40; height: 20 }\n  p:left (-) p:right\n]\n",
    ));
    assert!(
        !headless.contains(".lini-dim-line .lini-marker"),
        "{headless}"
    );
}

#[test]
fn the_cutting_planes_chrome_states_its_roles_once() {
    // A `|plane|` fills into ends, shafts, and heads — each role identical on
    // every plane, so each departure from the `.lini-plane` dress is a rule.
    let css = emit_str(&rules_for(
        "|drawing#d| [\n  |rect#p| { width: 40; height: 20 }\n  |plane#a| \"A\" { at: 0 }\n]\n",
    ));
    for rule in [
        ".lini .lini-plane-end { stroke-width: 2; stroke-dasharray: none; }",
        ".lini .lini-plane-shaft { stroke-dasharray: none; }",
        ".lini .lini-plane-arrow { fill: var(--lini-stroke-light); }",
    ] {
        assert!(css.contains(rule), "missing {rule}: {css}");
    }
}

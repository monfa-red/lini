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

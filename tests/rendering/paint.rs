use super::*;

#[test]
fn live_mode_emits_var_refs_for_visual_attrs() {
    let svg = render_live("|box| \"hi\"\n");
    assert!(svg.contains("var(--lini-fill)"), "{}", svg);
    assert!(svg.contains("var(--lini-stroke)"), "{}", svg);
    assert!(
        svg.contains("@layer lini.defaults"),
        "default style block should be present in live mode"
    );
}

#[test]
fn gap_fill_accepts_a_gradient() {
    // SPEC §11.3: `gap-fill` is a paint like `stroke`, so it takes a gradient — the
    // gutter rect fills with a `url(#…)` reference and the def is emitted.
    let svg = render_baked(
        "|row#r| { gap: 10; gap-fill: gradient(red, blue) } [\n  |box#a| \"x\" { width: 40; height: 40 }\n  |box#b| \"y\" { width: 40; height: 40 }\n]\n",
    );
    assert!(
        svg.contains("<linearGradient"),
        "gradient def emitted: {svg}"
    );
    assert!(
        svg.contains(r#"class="lini-gutter""#) && svg.contains(r#"fill="url(#lini-gradient-1)""#),
        "the gutter rect fills with the gradient: {svg}"
    );
}

#[test]
fn colors_support_rgba_hsl_hsla_and_alpha_hex() {
    // SPEC §2: rgb/rgba/hsl/hsla (percentages) and 3/4/6/8-digit hex all round-trip.
    for c in [
        "rgba(1, 2, 3, 0.5)",
        "hsl(200, 50%, 50%)",
        "hsla(0, 70%, 50%, 0.5)",
        "#0a8f",
    ] {
        let svg = render_baked(&format!("|box#b| \"x\" {{ fill: {c} }}\n"));
        assert!(svg.contains(&format!("fill: {c}")), "{c}: {svg}");
    }
}

#[test]
fn define_paint_rides_its_shape_rule() {
    // SPEC §13: a define's own paint states on its `lini-{name}` rule;
    // geometry (radius) stays baked, never on the rule.
    let svg = render_baked("{\n  |s::box| { stroke: blue; radius: 5; }\n}\n|s#n| \"n\"\n");
    let rule = svg
        .lines()
        .find(|l| l.contains(".lini-s {"))
        .expect("shape-s rule present");
    assert!(
        rule.contains("stroke: blue"),
        "define paint on its rule: {}",
        rule
    );
    assert!(
        !rule.contains("radius"),
        "geometry must not ride CSS: {}",
        rule
    );
}

#[test]
fn bake_mode_resolves_var_refs_to_literals() {
    let svg = render_baked("|box| \"hi\"\n");
    assert!(svg.contains("fill: white; stroke: #444;"), "{}", svg);
    assert!(svg.contains("fill: currentColor"), "{}", svg);
    assert!(svg.contains("color: black"), "{}", svg);
    assert!(svg.contains(".lini .lini-box {"), "{}", svg);
    assert!(
        !svg.contains("@layer lini.defaults"),
        "bake mode should omit the var defaults block"
    );
    assert!(!svg.contains("var("), "no var() survives baking: {}", svg);
}

#[test]
fn inline_override_baked_into_style_attr() {
    // An inline paint override differs from the rules and rides style=.
    let svg = render_baked("{\n  --accent: #ff00aa;\n}\n|box#cat| \"Cat\" { fill: --accent }\n");
    assert!(svg.contains(r#"style="fill: #ff00aa""#), "{}", svg);
}

#[test]
fn auto_classes_include_primitive_and_styles() {
    let svg = render_live(
        "{\n  .bold { font-weight: bold; }\n  .thin { stroke: #444; }\n}\n|box#cat| \"Cat\" .bold.thin\n",
    );
    assert!(svg.contains("lini-box"), "{}", svg);
    assert!(svg.contains("lini-style-bold"), "{}", svg);
    assert!(svg.contains("lini-style-thin"), "{}", svg);
}

#[test]
fn auto_classes_include_user_shape_chain() {
    let svg = render_live("{\n  |treat::box| { radius: 5; }\n}\n|treat#cat| \"Cat\"\n");
    assert!(svg.contains("lini-treat"), "{}", svg);
    assert!(svg.contains("lini-box"), "{}", svg);
    assert!(svg.contains(r#"data-id="cat""#), "{}", svg);
}

#[test]
fn stroke_style_renders_dasharray() {
    let svg = render_live("|box| \"d\" { width: 80; height: 40; stroke-style: dashed }\n");
    assert!(svg.contains("stroke-dasharray"), "{}", svg);
}

#[test]
fn css_cascade_emits_rules_and_diffs() {
    // Self-contained: element rules, class rules, inline diffs, link defaults, the
    // operator-dash class, and cascading text props. (The pretty user-facing
    // cascade demo lives in samples/styles.lini.)
    let src = r#"{
  |-| { stroke: #666; stroke-width: 1; }
  |box| { fill: lightyellow; }
  .loud { stroke: red; stroke-width: 2; }
  .calm { stroke: teal; }
  .wire { stroke: teal; }
}

|box#flat| "Plain"
|box#loud| "Loud" .loud
|box#mix| "Mix" .calm { fill: lavender; }
|group#crew| { font-size: 10; font-family: serif; } [
  |caption| "Crew"
  |box#tiny| "Tiny"
]

flat -> loud
loud --> mix .wire
"#;
    let svg = lini::compile_str(src).expect("compile");
    assert!(
        svg.contains(".lini .lini-style-loud { stroke: red; stroke-width: 2; }"),
        "{}",
        svg
    );
    assert!(
        svg.contains(".lini .lini-box { fill: lightyellow;"),
        "element rule merged into the shape rule: {}",
        svg
    );
    assert!(
        svg.contains(".lini .lini-link { fill: none; stroke: #666;"),
        "{}",
        svg
    );
    // A styled node carries no inline paint — the class provides it.
    assert!(
        svg.contains(r#"lini-style-loud" data-id="loud" transform"#),
        "loud node must carry no style attr: {}",
        svg
    );
    // Only genuine differences ride style=.
    assert!(svg.contains(r#"style="fill: lavender""#), "{}", svg);
    // Cascading text props sit on the group and inherit natively.
    assert!(
        svg.contains(r#"style="font-family: serif; font-size: 10px""#),
        "{}",
        svg
    );
    // A link carries its style classes like a node; the `--` operator's dash
    // rides a `lini-link-dashed` class (the pattern stated once in the sheet),
    // never an inline diff repeated on every dashed link.
    assert!(
        svg.contains(".lini .lini-link-dashed { stroke-dasharray: 4,3; }"),
        "the operator dash must be stated once as a class rule: {}",
        svg
    );
    // A link's `.wire` class paints its wire with `stroke`, one vocabulary with a
    // node's outline (SPEC §9/§13); its colour states once as a
    // `.lini-style-wire { stroke: … }` rule — never inline on the link.
    assert!(
        svg.contains(".lini .lini-style-wire { stroke: teal; }"),
        "a link's stroke class states once as a rule: {}",
        svg
    );
    let link_g = svg
        .lines()
        .find(|l| l.contains(r#"data-from="loud""#))
        .expect("loud→mix link present");
    assert!(
        link_g.contains(r#"class="lini-link lini-link-dashed lini-style-wire""#),
        "link must carry its dash + style classes: {}",
        link_g
    );
    assert!(
        !link_g.contains("stroke-dasharray"),
        "the dash rides the class, not inline: {}",
        link_g
    );
    assert!(
        !link_g.contains("stroke: teal"),
        ".wire colour must ride the class rule, not inline: {}",
        link_g
    );
}

#[test]
fn a_halo_rule_restyles_the_crossing_knockouts() {
    // A crossed extension line bakes halo cuts; the render folds them into a
    // knockout mask whose `.lini-halo` stroke defaults black [SPEC 15.7].
    let src = "{ layout: drawing; density: 1 }\n|rect#plate| { width: 100; height: 40 }\n|hole#h| { width: 8 }\nplate:left (-) h { side: top }\n";
    let svg = render_baked(src);
    assert!(
        svg.contains(r##"mask="url(#lini-halo-"##),
        "the line wears its mask: {svg}"
    );
    assert!(svg.contains(".lini-halo { stroke: black; }"), "{svg}");
    // The `|halo|` chrome hook: an authored rule folds into `class_rules` and
    // emits via the template rules, and the generated default is *suppressed*
    // (the `emit_generated_default` ¬authored guard — like `|projection|`), so
    // `stroke: none` removes every crossing break scope-wide with no dead default
    // left behind and ordering never the mechanism.
    let themed = render_baked(&format!(
        "{{ layout: drawing; density: 1;\n  |halo| {{ stroke: none }}\n}}\n{}",
        &src[32..]
    ));
    assert!(
        themed.contains(".lini-halo { stroke: none; }"),
        "the user rule dresses the halo: {themed}"
    );
    assert!(
        !themed.contains(".lini-halo { stroke: black; }"),
        "the default is suppressed, not overridden by order: {themed}"
    );
}

#[test]
fn projection_line_chrome_rides_one_rule_and_removes_via_the_cascade() {
    let sheet = |rule: &str| {
        format!(
            "{rule}|page| {{ align: origin; gap: 40 }} [\n  \
            |drawing#a| {{ scale: 2 }} [ |oval#c| {{ width: 13; height: 13 }} ]\n  \
            |drawing#b| {{ scale: 2 }} [ |oval#d| {{ width: 13; height: 13 }} ]\n  \
            a.c:top - b.d:top\n]\n"
        )
    };
    // The generated line wears the type class and its paint rides one rule —
    // never inlined on the element [SPEC 8/15.8/17].
    let svg = lini::compile_str(&sheet("")).expect("compile");
    assert!(
        svg.contains(
            ".lini-projection { fill: none; stroke: var(--lini-stroke-light); stroke-width: 1; }"
        ),
        "the projection default rides one rule: {svg}"
    );
    assert!(
        svg.contains(
            r#"<g class="lini-node lini-projection lini-line" transform="translate(0,0)">"#
        ),
        "the projection line carries the class and no inline paint: {svg}"
    );
    // `|projection| { stroke: none }` removes it scope-wide via the cascade.
    let removed =
        lini::compile_str(&sheet("{ |projection| { stroke: none } }\n")).expect("compile");
    assert!(
        removed.contains(".lini-projection { fill: none; stroke: none; stroke-width: 1; }"),
        "the cascade removes the projection line: {removed}"
    );
}

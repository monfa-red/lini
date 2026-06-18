use lini::Options;

fn render_live(src: &str) -> String {
    lini::compile_str(src).expect("compile")
}

fn render_baked(src: &str) -> String {
    lini::compile_str_with(
        src,
        &Options {
            bake_vars: true,
            ..Default::default()
        },
    )
    .expect("compile")
}

fn render_themed(src: &str, theme_css: &str) -> String {
    lini::compile_str_with(
        src,
        &Options {
            theme_css: Some(theme_css.to_string()),
            ..Default::default()
        },
    )
    .expect("compile")
}

#[test]
fn theme_font_stack_emits_verbatim() {
    // SPEC §11: a `--theme` font value is valid CSS already. A family stack must
    // round-trip into the @layer block as-is — not get wrapped into one bogus
    // quoted family (`"Inter, system-ui, sans-serif"`).
    let svg = render_themed(
        "|box| { \"hi\" }\n",
        ".lini { --lini-font-family: Inter, system-ui, sans-serif; }",
    );
    assert!(
        svg.contains("--lini-font-family: Inter, system-ui, sans-serif;"),
        "font stack should emit verbatim: {}",
        svg
    );
}

#[test]
fn theme_quoted_font_family_is_not_double_wrapped() {
    // A family with spaces arrives already quoted; re-quoting yields the
    // malformed `""Helvetica Neue", sans-serif"`.
    let svg = render_themed(
        "|box| { \"hi\" }\n",
        ".lini { --lini-font-family: \"Helvetica Neue\", sans-serif; }",
    );
    assert!(
        svg.contains("--lini-font-family: \"Helvetica Neue\", sans-serif;"),
        "quoted family must not be double-wrapped: {}",
        svg
    );
    assert!(!svg.contains("\"\"Helvetica"), "double-wrapped: {}", svg);
}

#[test]
fn theme_font_inherit_stays_a_keyword() {
    // SPEC §11: `--lini-font-family: inherit` lets an embedded diagram pick up
    // the host page's font. It must stay the bare CSS keyword, never quoted.
    let svg = render_themed("|box| { \"hi\" }\n", ".lini { --lini-font-family: inherit; }");
    assert!(svg.contains("--lini-font-family: inherit;"), "{}", svg);
    assert!(!svg.contains("\"inherit\""), "inherit must be a keyword: {}", svg);
}

#[test]
fn live_mode_emits_var_refs_for_visual_attrs() {
    let svg = render_live("|box| { \"hi\" }\n");
    assert!(svg.contains("var(--lini-fill)"), "{}", svg);
    assert!(svg.contains("var(--lini-stroke)"), "{}", svg);
    assert!(
        svg.contains("@layer lini.defaults"),
        "default style block should be present in live mode"
    );
}

#[test]
fn multiline_label_emits_one_tspan_per_line() {
    // SPEC §6: `\n` splits a label across lines (spacing size × 1.2). Layout
    // already sizes the bbox for N lines; render lays them out as tspans.
    let svg = render_baked("n |box| { \"one\\ntwo\" }\n");
    assert_eq!(svg.matches("<tspan").count(), 2, "expected two tspans: {}", svg);
    assert!(
        svg.contains(">one</tspan>") && svg.contains(">two</tspan>"),
        "{}",
        svg
    );
}

#[test]
fn single_line_label_stays_a_bare_text() {
    let svg = render_baked("n |box| { \"solo\" }\n");
    assert!(!svg.contains("<tspan"), "single line must not wrap in a tspan: {}", svg);
}

#[test]
fn line_missing_points_error_uses_pipe_sigil() {
    let err = lini::compile_str("x |line|\n").expect_err("line needs points");
    assert!(
        err.to_string().contains("'|line|' requires 'points'"),
        "got: {}",
        err
    );
}

#[test]
fn define_paint_rides_its_shape_rule() {
    // SPEC §13: a define's own paint states on its `lini-shape-{name}` rule;
    // geometry (radius) stays baked, never on the rule.
    let svg = render_baked("s::box { stroke: blue; radius: 5; }\nn |s| { \"n\" }\n");
    let rule = svg
        .lines()
        .find(|l| l.contains(".lini-shape-s {"))
        .expect("shape-s rule present");
    assert!(rule.contains("stroke: blue"), "define paint on its rule: {}", rule);
    assert!(!rule.contains("radius"), "geometry must not ride CSS: {}", rule);
}

#[test]
fn inherited_text_prop_reset_to_default_is_emitted() {
    // A descendant that resets an inherited text prop, under an overriding
    // ancestor, must still emit it on its own <g> — else the dropped
    // declaration leaves it inheriting the ancestor's value.
    let svg =
        render_baked("crew |group| { font-size: 20; reset |plain| { font-size: 13; \"x\" } }\n");
    let g_line = svg
        .lines()
        .find(|l| l.contains("data-id=\"reset\""))
        .expect("reset node present");
    assert!(
        g_line.contains("font-size: 13px"),
        "reset must emit its own font-size, not inherit 20px: {}",
        g_line
    );
}

#[test]
fn bake_mode_resolves_var_refs_to_literals() {
    let svg = render_baked("|box| { \"hi\" }\n");
    assert!(svg.contains("fill: white; stroke: #444;"), "{}", svg);
    assert!(svg.contains("fill: currentColor"), "{}", svg);
    assert!(svg.contains("color: black"), "{}", svg);
    assert!(svg.contains(".lini .lini-shape-box {"), "{}", svg);
    assert!(
        !svg.contains("@layer lini.defaults"),
        "bake mode should omit the var defaults block"
    );
    assert!(!svg.contains("var("), "no var() survives baking: {}", svg);
}

#[test]
fn inline_override_baked_into_style_attr() {
    // An inline paint override differs from the rules and rides style=.
    let svg = render_baked("--accent: #ff00aa;\ncat |box| { fill: --accent; \"Cat\" }\n");
    assert!(svg.contains(r#"style="fill: #ff00aa""#), "{}", svg);
}

#[test]
fn auto_classes_include_primitive_and_styles() {
    let svg = render_live(
        ".bold { font-weight: bold; }\n.thin { stroke: #444; }\ncat |box| .bold .thin { \"Cat\" }\n",
    );
    assert!(svg.contains("lini-shape-box"), "{}", svg);
    assert!(svg.contains("lini-style-bold"), "{}", svg);
    assert!(svg.contains("lini-style-thin"), "{}", svg);
}

#[test]
fn auto_classes_include_user_shape_chain() {
    let svg = render_live("treat::box { radius: 5; }\ncat |treat| { \"Cat\" }\n");
    assert!(svg.contains("lini-shape-treat"), "{}", svg);
    assert!(svg.contains("lini-shape-box"), "{}", svg);
    assert!(svg.contains(r#"data-id="cat""#), "{}", svg);
}

#[test]
fn hex_emits_polygon() {
    assert!(render_live("|hex| { width: 60; height: 60; }\n").contains("<polygon"));
}

#[test]
fn diamond_emits_polygon() {
    assert!(render_live("|diamond| { width: 60; height: 60; }\n").contains("<polygon"));
}

#[test]
fn slant_emits_polygon_with_skew() {
    assert!(render_live("|slant| { width: 80; height: 40; skew: 20; }\n").contains("<polygon"));
}

#[test]
fn oval_emits_ellipse() {
    assert!(render_live("|oval| { width: 80; height: 40; }\n").contains("<ellipse"));
}

#[test]
fn cyl_emits_ellipse_and_path() {
    let svg = render_live("|cyl| { width: 60; height: 80; }\n");
    assert!(svg.contains("<ellipse"), "{}", svg);
    assert!(svg.contains("<path"), "{}", svg);
}

#[test]
fn cloud_emits_path() {
    assert!(render_live("|cloud| { width: 100; height: 60; }\n").contains("<path"));
}

#[test]
fn poly_emits_polygon_with_user_points() {
    assert!(render_live("|poly| { points: 0 0, 20 0, 10 20; }\n").contains("<polygon"));
}

#[test]
fn full_spec_example_renders_in_both_modes() {
    let src = std::fs::read_to_string("samples/full_example.lini").expect("read");
    let live = lini::compile_str(&src).expect("live compile");
    let baked = lini::compile_str_with(
        &src,
        &Options {
            bake_vars: true,
            ..Default::default()
        },
    )
    .expect("baked compile");
    assert!(live.contains("var(--lini-"));
    assert!(!baked.contains("@layer lini.defaults"));
    assert!(live.starts_with("<svg"));
    assert!(baked.starts_with("<svg"));
    assert!(live.contains("lini-wire"));
    assert!(baked.contains("lini-wire"));
}

#[test]
fn stroke_style_renders_dasharray() {
    let svg = render_live("|box| { width: 80; height: 40; stroke-style: dashed; \"d\" }\n");
    assert!(svg.contains("stroke-dasharray"), "{}", svg);
}

#[test]
fn font_size_on_container_reaches_descendant_text() {
    let svg = render_live("g |group| { font-size: 10; \"hi\" }\n");
    assert!(svg.contains("font-size: 10px"), "{}", svg);
}

#[test]
fn css_cascade_sample_emits_rules_and_diffs() {
    let src = std::fs::read_to_string("samples/css_cascade.lini").expect("read");
    let svg = lini::compile_str(&src).expect("compile");
    assert!(
        svg.contains(".lini .lini-style-loud { stroke: red; stroke-width: 2; }"),
        "{}",
        svg
    );
    assert!(
        svg.contains(".lini .lini-shape-box { fill: lightyellow;"),
        "element rule merged into the shape rule: {}",
        svg
    );
    assert!(svg.contains(".lini .lini-wire { fill: none; stroke: #666;"), "{}", svg);
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
    // A wire carries its style classes like a node; only the `--` operator's
    // dash (not from a style) is left as an inline diff.
    let wire_g = svg
        .lines()
        .find(|l| l.contains(r#"data-from="loud""#))
        .expect("loud→mix wire present");
    assert!(
        wire_g.contains(r#"class="lini-wire lini-style-calm""#),
        "wire must carry its style class: {}",
        wire_g
    );
    assert!(
        wire_g.contains(r#"style="stroke-dasharray: 4,4""#),
        "only the operator dash rides inline: {}",
        wire_g
    );
    assert!(
        !wire_g.contains("stroke: teal"),
        ".calm stroke must ride the class rule, not inline: {}",
        wire_g
    );
}

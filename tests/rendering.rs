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

#[test]
fn live_mode_emits_var_refs_for_visual_attrs() {
    let svg = render_live("|rect| \"hi\"\n");
    assert!(svg.contains("var(--lini-fill)"), "{}", svg);
    assert!(svg.contains("var(--lini-stroke)"), "{}", svg);
    assert!(
        svg.contains("@layer lini.defaults"),
        "default style block should be present in live mode"
    );
}

#[test]
fn multiline_label_emits_one_tspan_per_line() {
    // SPEC §5: `\n` splits a label across lines (spacing size × 1.2). Layout
    // already sizes the bbox for N lines; render must lay them out as tspans.
    let svg = render_baked("n |rect| \"one\\ntwo\"\n");
    assert_eq!(
        svg.matches("<tspan").count(),
        2,
        "expected two tspans: {}",
        svg
    );
    assert!(
        svg.contains(">one</tspan>") && svg.contains(">two</tspan>"),
        "{}",
        svg
    );
}

#[test]
fn single_line_label_stays_a_bare_text() {
    // No churn for the common case: one line emits no tspan.
    let svg = render_baked("n |rect| \"solo\"\n");
    assert!(
        !svg.contains("<tspan"),
        "single line must not wrap in a tspan: {}",
        svg
    );
}

#[test]
fn line_missing_points_error_uses_pipe_sigil() {
    // SPEC §16: the type is named with the pipe sigil (|line|), not :line.
    let err = lini::compile_str("x |line|\n").expect_err("line needs points");
    assert!(
        err.to_string().contains("'|line|' requires 'points'"),
        "got: {}",
        err
    );
}

#[test]
fn shape_def_styles_merge_in_defs_order() {
    // SPEC §13: defs-block order decides between styles, listing order is
    // irrelevant — and that must hold inside a shape def too, exactly as it
    // does on a node. `.a` (red) precedes `.b` (blue), so `.b` wins.
    let svg = render_baked("{ .a stroke:red\n.b stroke:blue\n|s:rect| .b .a }\nn |s| \"n\"\n");
    let rule = svg
        .lines()
        .find(|l| l.contains(".lini-shape-s {"))
        .expect("shape-s rule present");
    assert!(
        rule.contains("stroke: blue"),
        "defs order must decide: .b (blue) should win over .a (red): {}",
        rule
    );
}

#[test]
fn inherited_text_prop_reset_to_default_is_emitted() {
    // A descendant that resets an inherited text prop to the root default,
    // under an overriding ancestor, must still emit it on its own <g> — else
    // the dropped declaration leaves it inheriting the ancestor's value.
    let svg = render_baked("crew |group| text-size:20 { reset |text| \"x\" text-size:13 }\n");
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
    let svg = render_baked("|rect| \"hi\"\n");
    // Defaults live in the structural rules, baked to literals.
    assert!(svg.contains("fill: white; stroke: #444;"), "{}", svg);
    // Text fill is `currentColor` (SVG-native cascade); the root rule sets
    // `color` to the baked `--text-color` (= --fg = black).
    assert!(svg.contains("fill: currentColor"), "{}", svg);
    assert!(svg.contains("color: black"), "{}", svg);
    assert!(svg.contains(".lini .lini-shape-rect {"), "{}", svg);
    assert!(
        !svg.contains("@layer lini.defaults"),
        "bake mode should omit the var defaults block"
    );
    assert!(!svg.contains("var("), "no var() survives baking: {}", svg);
}

#[test]
fn defaults_override_baked_into_output() {
    // An inline paint override differs from the rules and rides style=.
    let svg = render_baked("{ --accent:#ff00aa }\ncat |rect| \"Cat\" fill:--accent\n");
    assert!(svg.contains(r#"style="fill: #ff00aa""#), "{}", svg);
}

#[test]
fn auto_classes_include_primitive_and_styles() {
    let svg = render_live(
        "{ .bold weight:bold\n  .thin stroke:#444 }\n\
         cat |rect| \"Cat\" .bold .thin\n",
    );
    assert!(svg.contains("lini-shape-rect"), "{}", svg);
    assert!(svg.contains("lini-style-bold"), "{}", svg);
    assert!(svg.contains("lini-style-thin"), "{}", svg);
}

#[test]
fn auto_classes_include_user_shape_chain() {
    let svg = render_live(
        "{ |treat:rect| radius:5 }\n\
         cat |treat| \"Cat\"\n",
    );
    assert!(svg.contains("lini-shape-treat"), "{}", svg);
    assert!(svg.contains("lini-shape-rect"), "{}", svg);
    assert!(svg.contains(r#"data-id="cat""#), "{}", svg);
}

#[test]
fn hex_emits_polygon() {
    let svg = render_live("|hex| size:(60, 60)\n");
    assert!(svg.contains("<polygon"), "{}", svg);
}

#[test]
fn diamond_emits_polygon() {
    let svg = render_live("|diamond| size:(60, 60)\n");
    assert!(svg.contains("<polygon"), "{}", svg);
}

#[test]
fn slant_emits_polygon_with_skew() {
    let svg = render_live("|slant| size:(80, 40) skew:20\n");
    assert!(svg.contains("<polygon"), "{}", svg);
}

#[test]
fn oval_emits_ellipse() {
    let svg = render_live("|oval| size:(80, 40)\n");
    assert!(svg.contains("<ellipse"), "{}", svg);
}

#[test]
fn cyl_emits_ellipse_and_path() {
    let svg = render_live("|cyl| size:(60, 80)\n");
    assert!(svg.contains("<ellipse"), "{}", svg);
    assert!(svg.contains("<path"), "{}", svg);
}

#[test]
fn cloud_emits_path() {
    let svg = render_live("|cloud| size:(100, 60)\n");
    assert!(svg.contains("<path"), "{}", svg);
}

#[test]
fn poly_emits_polygon_with_user_points() {
    let svg = render_live("|poly| points:[(0,0),(20,0),(10,20)]\n");
    assert!(svg.contains("<polygon"), "{}", svg);
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
fn line_attr_renders_dasharray() {
    let svg = render_live("|rect| \"d\" size:(80,40) line:dashed\n");
    assert!(svg.contains("stroke-dasharray"), "{}", svg);
}

#[test]
fn text_size_on_container_reaches_descendant_text() {
    let svg = render_live("g |group| text-size:10 { |text| \"hi\" }\n");
    assert!(svg.contains("font-size: 10px"), "{}", svg);
}

#[test]
fn css_cascade_sample_emits_rules_and_diffs() {
    let src = std::fs::read_to_string("samples/css_cascade.lini").expect("read");
    let svg = lini::compile_str(&src).expect("compile");
    // Defs block ships as a stylesheet.
    assert!(
        svg.contains(".lini .lini-style-loud { stroke: red; stroke-width: 2; }"),
        "{}",
        svg
    );
    assert!(
        svg.contains(".lini .lini-shape-rect { fill: lightyellow;"),
        "type default merged into the shape rule: {}",
        svg
    );
    assert!(
        svg.contains(".lini .lini-wire { fill: none; stroke: #666;"),
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
    // Cascading text attrs sit on the group and inherit natively.
    assert!(
        svg.contains(r#"style="font-family: serif; font-size: 10px""#),
        "{}",
        svg
    );
    // A wire carries its defs styles as classes, exactly like a node: `.calm`
    // rides `lini-style-calm`, and only the `--` operator's dash (not from a
    // style) is left as an inline diff.
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

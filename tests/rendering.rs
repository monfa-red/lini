use lini::Options;

fn render_live(src: &str) -> String {
    lini::compile_str(src).expect("compile")
}

fn render_baked(src: &str) -> String {
    lini::compile_str_with(
        src,
        &Options {
            static_mode: true,
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
        "|box| \"hi\"\n",
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
        "|box| \"hi\"\n",
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
    let svg = render_themed("|box| \"hi\"\n", ".lini { --lini-font-family: inherit; }");
    assert!(svg.contains("--lini-font-family: inherit;"), "{}", svg);
    assert!(
        !svg.contains("\"inherit\""),
        "inherit must be a keyword: {}",
        svg
    );
}

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
fn multiline_label_emits_one_tspan_per_line() {
    // SPEC §6: `\n` splits a label across lines (spacing size × 1.2). Layout
    // already sizes the bbox for N lines; render lays them out as tspans.
    let svg = render_live("|box#n| \"one\\ntwo\"\n");
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
    let svg = render_baked("|box#n| \"solo\"\n");
    assert!(
        !svg.contains("<tspan"),
        "single line must not wrap in a tspan: {}",
        svg
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
fn letter_spacing_bakes_a_dx_list_never_css() {
    // SPEC §10: letter-spacing compiles into a per-glyph `dx` list (geometry),
    // never a CSS property. "abc" → two 5px gaps.
    let svg = render_live("|box| \"abc\" { letter-spacing: 5 }\n");
    assert!(svg.contains(r#"dx="0 5 5""#), "{}", svg);
    assert!(
        !svg.contains("letter-spacing"),
        "no CSS letter-spacing: {}",
        svg
    );
}

#[test]
fn line_spacing_widens_the_tspan_leading_never_css() {
    // SPEC §10: line-spacing adds to the leading between `\n` lines (font-size 15
    // → 18, +10 = 28), via the tspan `dy` — never a CSS property.
    let svg = render_live("|box| \"one\\ntwo\" { line-spacing: 10 }\n");
    assert!(svg.contains(r#"dy="28""#), "{}", svg);
    assert!(
        !svg.contains("line-spacing"),
        "no CSS line-spacing: {}",
        svg
    );
}

#[test]
fn font_style_emits_as_live_css_with_no_default() {
    // SPEC §10: font-style is a live CSS property — it emits where set (on the
    // box `<g>`, inherited to its text) and has no global default.
    let svg = render_baked("|group#g| \"hi\" { font-style: italic }\n");
    assert!(
        svg.contains("font-style: italic"),
        "emits where set: {}",
        svg
    );
    // No baked default — a plain box never carries it.
    assert!(
        !render_baked("|box| \"x\"\n").contains("font-style"),
        "no global font-style default"
    );
}

fn lini_root_rule(svg: &str) -> String {
    svg.lines()
        .find(|l| l.trim_start().starts_with(".lini {"))
        .expect(".lini root rule")
        .to_string()
}

#[test]
fn global_font_style_states_on_the_lini_rule() {
    // SPEC §10: a global font-style applies scene-wide via the `.lini` rule,
    // exactly like a global font-size.
    let rule = lini_root_rule(&render_baked("{ font-style: italic }\n|box| \"hi\"\n"));
    assert!(rule.contains("font-style: italic"), "{}", rule);
}

#[test]
fn text_transform_is_live_css_on_an_element_and_globally() {
    // On an element it rides the `<g>`; in the global block it states on `.lini`.
    let el = render_baked("|box| \"hi\" { text-transform: uppercase }\n");
    assert!(el.contains("text-transform: uppercase"), "{}", el);
    let rule = lini_root_rule(&render_baked(
        "{ text-transform: capitalize }\n|box| \"hi\"\n",
    ));
    assert!(rule.contains("text-transform: capitalize"), "{}", rule);
    // No default — absent until set.
    assert!(!render_baked("|box| \"x\"\n").contains("text-transform"));
}

#[test]
fn text_decoration_is_live_css_on_an_element_and_globally() {
    // Same live-CSS treatment as text-transform: element rides the `<g>`, global
    // states on `.lini`, no default.
    let el = render_baked("|box| \"hi\" { text-decoration: underline }\n");
    assert!(el.contains("text-decoration: underline"), "{}", el);
    let rule = lini_root_rule(&render_baked(
        "{ text-decoration: line-through }\n|box| \"hi\"\n",
    ));
    assert!(rule.contains("text-decoration: line-through"), "{}", rule);
    assert!(!render_baked("|box| \"x\"\n").contains("text-decoration"));
}

#[test]
fn text_shadow_compiles_unitless_lengths_to_px() {
    // SPEC §10: text-shadow is live CSS; lini's unitless offsets/blur gain `px`,
    // colours pass through. Works on an element and globally.
    let el = render_baked("|box| \"hi\" { text-shadow: 1 1 2 gray }\n");
    assert!(el.contains("text-shadow: 1px 1px 2px gray"), "{}", el);
    let rule = lini_root_rule(&render_baked("{ text-shadow: 2 2 black }\n|box| \"hi\"\n"));
    assert!(rule.contains("text-shadow: 2px 2px black"), "{}", rule);
}

#[test]
fn global_font_family_weight_color_override_their_var() {
    // SPEC §10: a global font-family / font-weight / color states on `.lini`,
    // overriding its themeable var; unset, the live var stays.
    let set = lini_root_rule(&render_baked(
        "{ font-weight: normal; color: navy; font-family: serif }\n|box| \"hi\"\n",
    ));
    assert!(
        set.contains("font-weight: normal")
            && set.contains("color: navy")
            && set.contains("font-family: serif"),
        "{}",
        set
    );
    let dflt = lini_root_rule(&render_live("|box| \"hi\"\n"));
    assert!(
        dflt.contains("color: var(--lini-text-color)")
            && dflt.contains("font-weight: var(--lini-font-weight)"),
        "{}",
        dflt
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
fn line_missing_points_error_uses_pipe_sigil() {
    let err = lini::compile_str("|line#x|\n").expect_err("line needs points");
    assert!(
        err.to_string().contains("'|line|' requires 'points'"),
        "got: {}",
        err
    );
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
fn inherited_text_prop_reset_to_default_is_emitted() {
    // A descendant that resets an inherited text prop, under an overriding
    // ancestor, must still emit it on its own <g> — else the dropped
    // declaration leaves it inheriting the ancestor's value.
    let svg =
        render_baked("|group#crew| { font-size: 20 } [ |block#reset| \"x\" { font-size: 13 } ]\n");
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
fn poly_emits_polygon_with_user_points() {
    assert!(render_live("|poly| { points: 0 0, 20 0, 10 20; }\n").contains("<polygon"));
}

#[test]
fn hero_renders_in_both_modes() {
    let src = std::fs::read_to_string("samples/hero.lini").expect("read");
    let live = lini::compile_str(&src).expect("live compile");
    let baked = lini::compile_str_with(
        &src,
        &Options {
            static_mode: true,
            ..Default::default()
        },
    )
    .expect("baked compile");
    assert!(live.contains("var(--lini-"));
    assert!(!baked.contains("@layer lini.defaults"));
    assert!(live.starts_with("<svg"));
    assert!(baked.starts_with("<svg"));
    assert!(live.contains("lini-link"));
    assert!(baked.contains("lini-link"));
}

#[test]
fn stroke_style_renders_dasharray() {
    let svg = render_live("|box| \"d\" { width: 80; height: 40; stroke-style: dashed }\n");
    assert!(svg.contains("stroke-dasharray"), "{}", svg);
}

#[test]
fn font_size_on_container_reaches_descendant_text() {
    let svg = render_live("|group#g| \"hi\" { font-size: 10 }\n");
    assert!(svg.contains("font-size: 10px"), "{}", svg);
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

// ───────────────────────────── icons (SPEC §7) ─────────────────────────────

#[cfg(feature = "icons")]
#[test]
fn icon_renders_phosphor_paths_counter_scaled() {
    // A default 32px icon scales the 256 grid by 0.125 and counter-scales the
    // stroke (2 × 256 / 32 = 16) so its weight matches other 2px strokes.
    let svg = render_live("|icon#x| { symbol: heart }\n");
    assert!(
        svg.contains(r#"transform="scale(0.125) translate(-128 -128)""#),
        "{svg}"
    );
    assert!(svg.contains(r#"stroke-width="16""#), "{svg}");
    assert!(svg.contains(r#"stroke-linecap="round""#), "{svg}");
    assert!(svg.contains(r#"<path d="M"#), "{svg}");
}

// ── Classes on text [SPEC 3/4/17] ──

#[test]
fn worn_class_joins_lini_text_on_the_text_element() {
    // The worn class emits as `lini-style-*` beside `lini-text`, and its live
    // declarations ride the stylesheet rule exactly as a node class does.
    let svg = render_live("{ .quiet { color: --teal-deep; } }\n\"hi\" .quiet\n");
    assert!(
        svg.contains(r#"<text class="lini-text lini-style-quiet""#),
        "class hook on <text>: {svg}"
    );
    assert!(
        svg.contains(".lini .lini-style-quiet { color: var(--lini-teal-deep); }"),
        "class rule carries its live decls: {svg}"
    );
}

#[test]
fn a_class_font_size_grows_the_text_leaf() {
    // A baked property (`font-size`) on a worn class must reach measurement, so
    // the leaf's box — and the scene's height — grow.
    let plain = render_live("\"Grows\"\n");
    let big = render_live("{ .big { font-size: 40; } }\n\"Grows\" .big\n");
    let h = |svg: &str| {
        svg.split("height=\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .and_then(|s| s.parse::<f64>().ok())
            .expect("height")
    };
    assert!(
        h(&big) > h(&plain) + 10.0,
        "font-size class should grow the leaf: {} vs {}",
        h(&big),
        h(&plain)
    );
}

#[test]
fn a_worn_class_beats_the_inherited_context_on_text() {
    // Tier 3 sits above inheritance: the box paints its text red, but the leaf's
    // worn class repaints it — the class rule rides the `<text>`, winning in CSS.
    let svg = render_live(
        "{ .blue { color: --teal-deep; } }\n|box#b| { color: --red; } [ \"child\" .blue ]\n",
    );
    assert!(
        svg.contains(r#"<text class="lini-text lini-style-blue""#),
        "the class rides the text over the inherited color: {svg}"
    );
}

#[test]
fn own_block_beats_a_worn_class_on_text() {
    // The leaf's own `{ }` is tier 5, above the class (tier 3): its `font-size`
    // inlines and wins.
    let svg = render_live("{ .big { font-size: 40; } }\n\"x\" .big { font-size: 10 }\n");
    assert!(
        svg.contains(r#"style="font-size: 10px""#),
        "own block overrides the class: {svg}"
    );
}

#[test]
fn a_box_property_in_a_class_is_inert_on_text_never_an_error() {
    // The class-polymorphism law: a non-text-valid class declaration is inert on
    // a text wearer — it compiles, and never rides the text's inline style.
    let svg = render_live("{ .card { padding: 40; color: --red; } }\n\"x\" .card\n");
    assert!(
        svg.contains(r#"<text class="lini-text lini-style-card""#),
        "class still hooks: {svg}"
    );
    assert!(
        !svg.contains("padding"),
        "padding is inert on text, never emitted: {svg}"
    );
}

#[cfg(feature = "icons")]
#[test]
fn icon_two_tone_paints_body_fill_and_line_stroke() {
    // fill = body, stroke = line — exactly like a box.
    let svg = render_live("|icon#x| { symbol: heart; fill: --teal-wash; stroke: --teal-ink }\n");
    assert!(
        svg.contains(r#"fill="var(--lini-teal-wash)" stroke="none""#),
        "{svg}"
    );
    assert!(svg.contains(r#"stroke="var(--lini-teal-ink)""#), "{svg}");
}

#[cfg(feature = "icons")]
#[test]
fn icon_single_tone_drops_the_body_group() {
    // `fill: none` leaves a clean line icon — the line group only, no body fill.
    let svg = render_live("|icon#x| { symbol: heart; fill: none }\n");
    assert!(
        svg.contains(r#"fill="none" stroke="var(--lini-stroke)""#),
        "{svg}"
    );
    assert!(!svg.contains(r#"stroke="none">"#), "no body group: {svg}");
}

#[cfg(feature = "icons")]
#[test]
fn icon_solid_role_keeps_a_foreground_dot() {
    // atom's nucleus is a solid fill (ink), kept distinct from the faint body.
    let svg = render_live("|icon#x| { symbol: atom }\n");
    assert!(svg.contains(r#"<circle cx="128" cy="128" r="12""#), "{svg}");
}

#[cfg(feature = "icons")]
#[test]
fn icon_label_is_lini_text_never_stroked() {
    // The label rides as a `lini-text` (which masks stroke), so the icon's stroke
    // never leaks onto it.
    let svg = render_live("|icon#x| { symbol: bell } [ \"3\" ]\n");
    assert!(
        svg.contains(r#"<text class="lini-text" x="0" y="0""#),
        "{svg}"
    );
    assert!(
        svg.contains(".lini-text { fill: currentColor; stroke: none;"),
        "{svg}"
    );
}

#[cfg(feature = "icons")]
#[test]
fn sign_is_a_larger_icon() {
    // |sign| is the icon primitive at 64px; with `fit: auto` that's the plain
    // 64/256 = 0.25 framing.
    let svg = render_live("|sign#x| { symbol: cloud; fit: auto }\n");
    assert!(
        svg.contains(r#"class="lini-node lini-sign lini-icon""#),
        "{svg}"
    );
    assert!(
        svg.contains(r#"transform="scale(0.25) translate(-128 -128)""#),
        "{svg}"
    );
}

#[cfg(feature = "icons")]
#[test]
fn sign_defaults_to_fit_contain() {
    // Unlike a bare |icon| (fit: auto), a stand-alone |sign| fills its box — the
    // shield scales to its own bounds (0.3478), not the 0.25 grid framing.
    let svg = render_live("|sign#x| { symbol: shield }\n");
    assert!(svg.contains(r#"scale(0.3478)"#), "{svg}");
    assert!(!svg.contains("scale(0.25)"), "{svg}");
}

#[cfg(feature = "icons")]
#[test]
fn icon_label_inherits_font_size_no_inline() {
    // The label is a plain lini-text with no inline font-size — it inherits 15px
    // from the .lini rule, like any text, at any icon size (never scaled).
    let svg = render_live("|icon#x| { symbol: bell; width: 96; height: 96 } [ \"3\" ]\n");
    assert!(
        svg.contains(r#"<text class="lini-text" x="0" y="0" dy="0.358em">3</text>"#),
        "{svg}"
    );
    assert!(svg.contains("font-size: 15px"), "{svg}");
}

#[cfg(feature = "icons")]
#[test]
fn a_long_label_grows_the_icon_uniformly() {
    // The icon is a square that grows with its label, so the symbol scales up too
    // (not just the box). A short label keeps the 32px default (scale 0.125); a
    // long one grows past it.
    let short = render_live("|icon#x| { symbol: cloud } [ \"hi\" ]\n");
    let long = render_live("|icon#x| { symbol: cloud } [ \"Storage Service\" ]\n");
    assert!(
        short.contains(r#"transform="scale(0.125) translate(-128 -128)""#),
        "a short label keeps 32px: {short}"
    );
    assert!(
        !long.contains(r#"scale(0.125)"#),
        "a long label grows the symbol past 32px: {long}"
    );
}

#[cfg(feature = "icons")]
#[test]
fn icon_masks_an_inherited_dash() {
    // A dashed container's stroke-dasharray must not bleed onto the icon's lines.
    let svg = render_live("|icon#x| { symbol: heart }\n");
    let rule = svg
        .lines()
        .find(|l| l.contains(".lini-icon {"))
        .expect(".lini-icon rule");
    assert!(rule.contains("stroke-dasharray: none"), "{rule}");
}

#[cfg(feature = "icons")]
#[test]
fn icon_fit_auto_keeps_the_grid_framing() {
    // `fit: auto` maps the whole 256 grid to the box — Phosphor's authored margin —
    // so a 64px sign is the plain scale(0.25) translate(-128 -128).
    let svg = render_live("|sign#x| { symbol: shield; fit: auto }\n");
    assert!(
        svg.contains(r#"transform="scale(0.25) translate(-128 -128)""#),
        "{svg}"
    );
}

#[cfg(feature = "icons")]
#[test]
fn icon_fit_contain_fills_and_recentres_on_the_glyph() {
    // `contain` measures the shield's own bounds (≈176×184, centred at y=140) and
    // scales it uniformly to fit the 64px box (64/184 = 0.3478) — larger than
    // auto's 0.25, and centred on the glyph, not the grid.
    let svg = render_live("|sign#x| { symbol: shield; fit: contain }\n");
    assert!(
        svg.contains(r#"transform="scale(0.3478) translate(-128 -140)""#),
        "{svg}"
    );
    assert!(!svg.contains("scale(0.25)"), "contain enlarges: {svg}");
}

#[cfg(feature = "icons")]
#[test]
fn icon_fit_cover_uses_the_larger_scale() {
    // `cover` scales until the box is covered — the max ratio (64/176 = 0.3636) —
    // so it exceeds `contain` and the glyph overflows on the long axis.
    let svg = render_live("|sign#x| { symbol: shield; fit: cover }\n");
    assert!(
        svg.contains(r#"transform="scale(0.3636) translate(-128 -140)""#),
        "{svg}"
    );
}

#[cfg(feature = "icons")]
#[test]
fn icon_fit_stretch_is_non_uniform() {
    // `stretch` fits each axis independently → a two-value scale.
    let svg = render_live("|sign#x| { symbol: shield; fit: stretch }\n");
    assert!(
        svg.contains(r#"transform="scale(0.3636 0.3478) translate(-128 -140)""#),
        "{svg}"
    );
}

#[cfg(feature = "icons")]
#[test]
fn icon_fit_holds_the_stroke_weight() {
    // The counter-scaled stroke follows the fit scale, so the on-screen weight is
    // constant: a |sign|'s 2 bakes auto 2 / 0.25 = 8, contain 2 / 0.3478 = 5.75 —
    // both draw 2px.
    let auto = render_live("|sign#x| { symbol: shield; fit: auto }\n");
    let contain = render_live("|sign#x| { symbol: shield; fit: contain }\n");
    assert!(auto.contains(r#"stroke-width="8""#), "{auto}");
    assert!(contain.contains(r#"stroke-width="5.75""#), "{contain}");
}

#[cfg(feature = "icons")]
#[test]
fn bad_fit_value_errors() {
    let err = lini::compile_str("|icon#x| { symbol: heart; fit: squish }\n")
        .expect_err("invalid fit value");
    assert!(err.message.contains("fit"), "{}", err.message);
}

#[test]
fn image_fit_cover_sets_slice() {
    let svg = render_live("|image#x| { src: \"a.png\"; width: 80; height: 40; fit: cover }\n");
    assert!(
        svg.contains(r#"preserveAspectRatio="xMidYMid slice""#),
        "{svg}"
    );
}

#[test]
fn image_fit_stretch_sets_none() {
    let svg = render_live("|image#x| { src: \"a.png\"; width: 80; height: 40; fit: stretch }\n");
    assert!(svg.contains(r#"preserveAspectRatio="none""#), "{svg}");
}

#[test]
fn image_fit_auto_omits_preserve_aspect_ratio() {
    // auto / contain is the SVG default (xMidYMid meet) — nothing extra emitted.
    let svg = render_live("|image#x| { src: \"a.png\"; width: 80; height: 40 }\n");
    assert!(svg.contains("<image "), "{svg}");
    assert!(!svg.contains("preserveAspectRatio"), "{svg}");
}

// ── Text leaves: a node's text and a link's label share one renderer ──

#[test]
fn link_label_translate_is_applied_once() {
    // Regression: `translate` used to be folded in at routing *and* re-applied at
    // render, doubling the nudge on a link label vs a node's text (SPEC §6/§9).
    // The shared text emitter applies it once. Both ends sit at y=0, so a clean
    // -10 nudge must land the label at exactly y="-10".
    let svg = render_live(
        "{ direction: row; gap: 120 }\n|box#a|\n|box#b|\na -> b [ \"L\" { translate: 0 -10 } ]\n",
    );
    let tag = svg
        .lines()
        .find(|l| l.contains(r#"<text class="lini-link-label""#))
        .expect("a link label");
    assert!(tag.contains(r#"y="-10""#), "translate once → y=-10: {tag}");
    assert!(!tag.contains(r#"y="-20""#), "not doubled: {tag}");
}

#[test]
fn link_label_supports_multiline_and_letter_spacing() {
    // A link label is an ordinary styleable text leaf (SPEC §3/§9), so the same
    // multi-line `\n` tspans and baked `letter-spacing` dx a node's text gets must
    // reach it too — the two render through one path.
    let svg = render_live("|box#a|\n|box#b|\na -> b [ \"AB\\nCD\" { letter-spacing: 5 } ]\n");
    let label = svg
        .split(r#"<text class="lini-link-label""#)
        .nth(1)
        .and_then(|s| s.split("</text>").next())
        .expect("a link label");
    assert!(label.contains("<tspan"), "multi-line tspans: {label}");
    assert!(
        label.contains(r#"dx="0 5""#),
        "baked letter-spacing: {label}"
    );
}

#[test]
fn a_sketch_outside_a_drawing_scope_stays_pixel_space() {
    // SPEC 15.1: `unit:` / density are semantic only in drawing scopes — a
    // sketch in a flow diagram is pixels: `right(300)` spans exactly 300.
    let l = lini::testing::route_sample(
        "|sketch#s| { draw: move(0, 0) right(300) down(10) left(300) close(); stroke-width: 0 }\n",
        16.0,
    );
    let (x0, _, x1, _) = lini::testing::node_rect(&l, "s").expect("sketch rect");
    assert!(
        (x1 - x0 - 300.0).abs() < 1e-6,
        "pixel-space width: {}",
        x1 - x0
    );
}

#[test]
fn an_absurd_drawing_extent_draws_the_ratio_hint() {
    // A 5 m beam authored at ratio 1 [SPEC 20] — the hint names the fix.
    let (_, diags) = lini::compile_str_checked(
        "{ layout: drawing; unit: m }\n|rect#beam| { width: 5; height: 0.4 }\n",
        &Options::default(),
    )
    .expect("compile");
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("'scale:' is a ratio")),
        "{diags:?}"
    );
    // At the honest ratio the hint is silent.
    let (_, diags) = lini::compile_str_checked(
        "{ layout: drawing; unit: m; scale: 0.02 }\n|rect#beam| { width: 5; height: 0.4 }\n",
        &Options::default(),
    )
    .expect("compile");
    assert!(diags.is_empty(), "{diags:?}");
}

// ── max-width / text-wrap + line alignment [SPEC 5/6] ──

#[test]
fn max_width_wraps_text_and_caps_the_auto_width() {
    let l = lini::testing::route_sample(
        "|box#card| \"A rather long label that should wrap\" { max-width: 160 }\n",
        16.0,
    );
    let (x0, _, x1, _) = lini::testing::node_rect(&l, "card").expect("card");
    assert!(
        x1 - x0 <= 160.0 + 1e-6,
        "the wrapped size is the measured size: {}",
        x1 - x0
    );
}

#[test]
fn wrapped_boxes_feed_grid_tracks() {
    // An auto track reads the wrapped width, not the unwrapped line [SPEC 5].
    let l = lini::testing::route_sample(
        "{ layout: grid; columns: auto, 40; }\n|box#a| \"a rather long wrapped label\" { max-width: 120 }\n|box#b| \"x\"\n",
        16.0,
    );
    let (x0, _, x1, _) = lini::testing::node_rect(&l, "a").expect("a");
    assert!(x1 - x0 <= 120.0 + 1e-6, "track fed the cap: {}", x1 - x0);
}

#[test]
fn a_wrapped_box_is_a_routing_obstacle_at_its_wrapped_size() {
    // The wire routes with the wrapped bbox as its obstacle — bbox-driven,
    // no separate plumbing [SPEC 5]; the route exists and stays lawful.
    let routes = lini::testing::routes_str(
        "|box#a| \"go\"\n|box#mid| \"a rather long label that wraps down\" { max-width: 120 }\n|box#b| \"stop\"\na -> b\n",
    )
    .expect("routes");
    assert_eq!(routes.len(), 1, "the wire drew");
}

#[test]
fn line_alignment_rides_the_holding_boxes_knob() {
    // `align: start` on the box holding the text left-flushes its lines
    // [SPEC 6]: the first (wider) line's centre sits right of the second's.
    let svg =
        render_live("|block#t| { max-width: 120; align: start } [ \"wider line\\nshort\" ]\n");
    let xs: Vec<f64> = svg
        .match_indices("<tspan x=\"")
        .map(|(i, _)| {
            let rest = &svg[i + 10..];
            rest[..rest.find('"').unwrap()].parse().unwrap()
        })
        .collect();
    assert_eq!(xs.len(), 2, "{svg}");
    assert!(
        xs[0] > xs[1],
        "start-aligned: the wider line's centre sits right: {xs:?}"
    );
    // Default stays centred — both lines share one x (today's output).
    let svg = render_live("|block#t| [ \"wider line\\nshort\" ]\n");
    let xs: Vec<&str> = svg
        .match_indices("<tspan x=\"")
        .map(|(i, _)| {
            let rest = &svg[i + 10..];
            &rest[..rest.find('"').unwrap()]
        })
        .collect();
    assert!(xs.windows(2).all(|w| w[0] == w[1]), "{svg}");
}

/// The drawn link lines of an SVG, split into (dashed, solid) `data-to` targets.
fn link_targets(svg: &str) -> (Vec<&str>, Vec<&str>) {
    let (mut dashed, mut solid) = (Vec::new(), Vec::new());
    for l in svg.lines() {
        let Some(at) = l.find("data-to=\"") else {
            continue;
        };
        let to = &l[at + 9..at + 9 + l[at + 9..].find('"').unwrap()];
        if l.contains("lini-link-dashed") {
            dashed.push(to);
        } else if l.contains("lini-link") {
            solid.push(to);
        }
    }
    (dashed, solid)
}

#[test]
fn a_scoped_link_rule_dashes_exactly_one_arm() {
    // A containment-shaped link (endpoints X and X.path) cascades as if written
    // in X [SPEC 9/12], so `#cto |-|` reaches cto's OWN spokes — the fan
    // `cto:bottom - cto.be & cto.fe` is textually written in ceo's body, but its
    // outer endpoint is cto — and no other arm. ceo's and coo's spokes stay
    // solid.
    let src = "{\n  layout: tree;\n  #cto |-| { stroke-style: dashed; }\n}\n\
        |topic#ceo| \"CEO\" [\n\
          |topic#cto| \"CTO\" [\n\
            |topic#be| \"BE\"\n\
            |topic#fe| \"FE\"\n\
          ]\n\
          |topic#coo| \"COO\" [\n\
            |topic#ops| \"Ops\"\n\
          ]\n\
        ]\n";
    let svg = render_live(src);
    let (dashed, solid) = link_targets(&svg);
    assert_eq!(
        dashed,
        ["ceo.cto.be", "ceo.cto.fe"],
        "exactly cto's two spokes dash"
    );
    assert_eq!(
        solid,
        ["ceo.cto", "ceo.coo", "ceo.coo.ops"],
        "ceo's and coo's spokes stay solid"
    );
}

#[test]
fn the_arm_rule_reaches_the_whole_subtree() {
    // With grandchildren under be, `#cto |-|` dashes the whole arm: cto's own
    // spokes AND be's fan (every chain passes through cto) [SPEC 9/12].
    let src = "{\n  layout: tree;\n  #cto |-| { stroke-style: dashed; }\n}\n\
        |topic#ceo| \"CEO\" [\n\
          |topic#cto| \"CTO\" [\n\
            |topic#be| \"BE\" [ |topic#api| \"API\" ]\n\
            |topic#fe| \"FE\"\n\
          ]\n\
          |topic#coo| \"COO\"\n\
        ]\n";
    let svg = render_live(src);
    let (dashed, solid) = link_targets(&svg);
    assert_eq!(
        dashed,
        ["ceo.cto.be", "ceo.cto.fe", "ceo.cto.be.api"],
        "the whole cto arm dashes"
    );
    assert_eq!(solid, ["ceo.cto", "ceo.coo"], "other spokes stay solid");
}

#[test]
fn natural_routing_renders_cubics_deterministically() {
    // A row tree with `routing: natural` [SPEC 9]: every branch wire draws as
    // straight stubs plus exact cubic segments — `C` commands in the link
    // path `d` — and reruns are byte-identical (ROUTING.md Law 4).
    let src = "{ layout: tree; direction: row; routing: natural }\n\
        |topic#root| \"Root\" [\n\
          |topic#a| \"Alpha\"\n\
          |topic#b| \"Beta\"\n\
          |topic#c| \"Gamma\"\n\
        ]\n";
    let svg = render_live(src);
    let wires: Vec<&str> = svg
        .lines()
        .skip_while(|l| !l.contains("lini-links"))
        .filter(|l| l.trim_start().starts_with("<path d=\""))
        .collect();
    assert_eq!(wires.len(), 3, "three branch wires");
    for w in &wires {
        assert!(w.contains(" C "), "a natural wire draws cubics: {w}");
        assert!(!w.contains(" A "), "no render-time fillet arcs: {w}");
    }
    assert_eq!(svg, render_live(src), "byte-identical rerun");
}

/// The Stage-5 mindmap scene the palette-walk render tests share: three named
/// branches (one with a subtopic), an anonymous branch, and a cross-link.
const MINDMAP: &str = "|mindmap#m| \"Plan\" [\n\
      |topic#a| \"Alpha\" [ |topic#a1| \"Deep\" ]\n\
      |topic#b| \"Beta\"\n\
      |topic#c| \"Gamma\"\n\
      |topic| \"Delta\"\n\
      a.a1 --- c\n\
    ]\n";

#[test]
fn the_palette_walk_tints_cards_and_wires_and_leaves_root_and_cross_links_neutral() {
    let svg = render_live(MINDMAP);
    // The root topic is neutral: level-0, no hue class, no hue paint.
    let root = svg
        .lines()
        .find(|l| l.contains("data-id=\"m\""))
        .expect("root node");
    assert!(root.contains("lini-level-0"), "level hook: {root}");
    assert!(!root.contains("lini-hue-"), "root neutral: {root}");
    // Branch cards tint at the tiers (wash fill, deep stroke, ink text) and
    // wear their level hook.
    let a = svg
        .lines()
        .find(|l| l.contains("data-id=\"a\""))
        .expect("branch a");
    for want in ["lini-level-1", "lini-hue-rose"] {
        assert!(a.contains(want), "{want}: {a}");
    }
    // The tint rides the emitted CSS rule, never inline on each wearer
    // [SPEC 17] — the card's `<g>` carries the classes and no hue paint.
    assert!(!a.contains("style="), "card free of inline paint: {a}");
    assert!(
        svg.contains(
            ".lini .lini-mindmap .lini-hue-rose { fill: var(--lini-rose-wash); \
             stroke: var(--lini-rose-deep); color: var(--lini-rose-ink); }"
        ),
        "the hue rule is real CSS: {svg}"
    );
    // Every branch wire tints — the root arm (written in the scene scope) and
    // the subtree wire alike, one generated rule each [SPEC 8].
    for (to, hue) in [
        ("data-to=\"m.a\"", "rose"),
        ("data-to=\"m.b\"", "orange"),
        ("data-to=\"m.c\"", "amber"),
        ("data-to=\"m.lini-topic-4\"", "lime"),
        ("data-to=\"m.a.a1\"", "rose"),
    ] {
        let wire = svg
            .lines()
            .find(|l| l.contains("lini-link") && l.contains(to))
            .unwrap_or_else(|| panic!("wire {to}"));
        assert!(wire.contains(&format!("lini-hue-{hue}")), "{to}: {wire}");
        assert!(
            !wire.contains("stroke:"),
            "the wire's tint rides the .lini-links companion rule: {wire}"
        );
        assert!(
            svg.contains(&format!(".lini .lini-links .lini-hue-{hue}")),
            "companion rule for {hue}: {svg}"
        );
    }
    // The authored cross-link keeps the neutral link default.
    let cross = svg
        .lines()
        .find(|l| l.contains("data-from=\"m.a.a1\"") && l.contains("data-to=\"m.c\""))
        .expect("cross-link");
    assert!(
        !cross.contains("lini-hue-") && !cross.contains("stroke: var(--lini-"),
        "cross-link neutral: {cross}"
    );
}

#[test]
fn authored_paint_beats_the_palette_walk() {
    // Explicit author paint wins: the generated tints are descendant rules, so
    // an inline block (and any user id/class rule) sits above them [SPEC 4/8].
    let src = "{ #b { stroke: --purple-deep; } }\n\
        |mindmap#m| \"Plan\" [\n\
          |topic#a| \"Alpha\" { fill: --amber-wash; }\n\
          |topic#b| \"Beta\"\n\
        ]\n";
    let svg = render_live(src);
    let a = svg
        .lines()
        .find(|l| l.contains("data-id=\"a\""))
        .expect("branch a");
    assert!(
        a.contains("fill: var(--lini-amber-wash)"),
        "inline fill wins over the rose wash: {a}"
    );
    // The untouched channels keep the walk *through the CSS rule* — the diff
    // inlines only the authored difference, never the rule's own values.
    assert!(
        !a.contains("stroke:"),
        "the walk's stroke rides the hue rule, not the wearer: {a}"
    );
    let b = svg
        .lines()
        .find(|l| l.contains("data-id=\"b\""))
        .expect("branch b");
    assert!(
        b.contains("stroke: var(--lini-purple-deep)"),
        "an id rule beats the generated descendant tint: {b}"
    );
}

#[test]
fn a_mindmap_compiles_transparent_to_its_desugar() {
    // The oracle law holds off-samples too: compiling the lowered mindmap —
    // seated scope, tinted per-branch arms, garnish rules — byte-matches
    // compiling the source (fan grouping included).
    let lowered = lini::desugar_source(MINDMAP).expect("desugar");
    assert_eq!(
        render_baked(MINDMAP),
        render_baked(&lowered),
        "compile(src) != compile(desugar(src))"
    );
}

#[test]
fn mindmap_root_arms_share_one_trunk_port_per_side() {
    // Per-branch root arms are separate statements so each wears its own hue,
    // yet they form one crow's-foot per side: a node's forced-port wires into
    // its own descendants fan across statements (the containment gate).
    let svg = render_live(MINDMAP);
    let mut starts: Vec<(String, String)> = Vec::new();
    for l in svg.lines() {
        if !l.contains("data-from=\"m\"") {
            continue;
        }
        let path = svg
            .lines()
            .skip_while(|x| *x != l)
            .find(|x| x.trim_start().starts_with("<path d=\""))
            .expect("wire path");
        let d = path.trim_start().strip_prefix("<path d=\"M ").unwrap();
        let xy: Vec<&str> = d.split(' ').take(2).collect();
        let to = &l[l.find("data-to=\"").unwrap() + 9..];
        starts.push((xy.join(" "), to[..to.find('"').unwrap()].to_string()));
    }
    assert_eq!(starts.len(), 4, "four root arms: {starts:?}");
    let mut ports: Vec<&str> = starts.iter().map(|(p, _)| p.as_str()).collect();
    ports.sort_unstable();
    ports.dedup();
    assert_eq!(
        ports.len(),
        2,
        "one shared port per side, not one per arm: {starts:?}"
    );
}

// ── `format:` on ticks & tooltips [SPEC 14.4/16, CHART-DRAW Stage 1] ──

#[test]
fn format_percent_formats_value_axis_ticks() {
    let svg = render_live(
        "|chart| [\n|axis| { side: left; range: 0 1; format: percent 0 }\n|bars| { data: 0.25, 0.5, 1 }\n]\n",
    );
    assert!(svg.contains(">100%<"), "formatted tick missing:\n{svg}");
    assert!(svg.contains(">20%<"), "formatted tick missing:\n{svg}");
}

#[test]
fn format_inherits_from_the_chart_and_reaches_titles() {
    // Chart-level `format:` defaults the axis ticks and the bar `<title>` values.
    let svg = render_live("|chart| { format: decimal 1 } [ |bars| \"S\" { data: 2, 4 } ]\n");
    assert!(svg.contains(">4.0<"), "formatted tick missing:\n{svg}");
    assert!(svg.contains("S: 4.0"), "formatted title missing:\n{svg}");
}

#[test]
fn format_date_preset_errors_off_a_time_axis() {
    let err = lini::compile_str(
        "|chart| [\n|axis| { side: left; format: month }\n|bars| { data: 1, 2 }\n]\n",
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("a date preset reads a time axis"),
        "got: {err}"
    );
}

#[test]
fn format_bad_value_errors_with_the_usage() {
    let err =
        lini::compile_str("|chart| { format: decimals } [ |bars| { data: 1 } ]\n").unwrap_err();
    assert!(
        err.to_string().contains("'format' takes auto"),
        "got: {err}"
    );
}

// ── Per-datum paint [SPEC 14.6, CHART-DRAW Stage 2] ──

#[test]
fn per_datum_fill_list_highlights_one_bar() {
    let svg =
        render_live("|chart| [\n|bars| { data: 9, 15, 24; fill: auto, auto, --red-soft }\n]\n");
    // Two bars keep the palette walk's first hue, the third takes the listed paint.
    assert_eq!(svg.matches("var(--lini-rose-soft)").count(), 2, "{svg}");
    assert!(
        svg.contains("var(--lini-red-deep)"),
        "the listed fill deepens its own edge:\n{svg}"
    );
    assert!(svg.contains("--lini-red-soft"), "{svg}");
}

#[test]
fn per_datum_stroke_auto_deepens_each_datums_own_fill() {
    let svg = render_live(
        "|chart| [\n|bars| { data: 9, 15; fill: auto, --red-soft; stroke: auto, auto }\n]\n",
    );
    // Each datum's edge is the deep tier of its *own* fill.
    assert!(svg.contains("--lini-rose-deep"), "{svg}");
    assert!(svg.contains("--lini-red-deep"), "{svg}");
}

#[test]
fn per_datum_list_count_mismatch_errors_with_both_counts() {
    let err = lini::compile_str("|chart| [\n|bars| { data: 9, 15, 24; fill: auto, --red }\n]\n")
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("'fill' lists 2 paints but the series has 3 data points"),
        "got: {err}"
    );
}

#[test]
fn per_datum_list_on_a_line_errors() {
    let err =
        lini::compile_str("|chart| [\n|line| { data: 9, 15; stroke: red, blue }\n]\n").unwrap_err();
    assert!(
        err.to_string().contains("one shape with one paint"),
        "got: {err}"
    );
}

#[test]
fn per_datum_list_with_fn_errors() {
    let err =
        lini::compile_str("|chart| [\n|bars| { fn: (x); fill: auto, --red }\n]\n").unwrap_err();
    assert!(
        err.to_string().contains("needs explicit 'data'"),
        "got: {err}"
    );
}

#[test]
fn per_datum_fill_list_reaches_dots() {
    let svg = render_live("|chart| [\n|dots| { data: 1 2, 3 4; fill: --blue-ink, auto }\n]\n");
    assert!(svg.contains("--lini-blue-ink"), "{svg}");
}

// ── Time axes [SPEC 14.3/14.4, CHART-DRAW Stage 3] ──

/// The x-axis tick labels of a compiled chart, in document order: muted tick
/// text nodes, minus the value-axis ticks (small numbers — the test data keeps
/// its values < 1900 so year ticks stay).
fn x_tick_texts(svg: &str) -> Vec<String> {
    svg.match_indices("var(--lini-muted); font-size: 11px; font-weight: normal\">")
        .map(|(i, m)| {
            let rest = &svg[i + m.len()..];
            rest[..rest.find('<').unwrap_or(0)].to_string()
        })
        .filter(|t| t.parse::<f64>().map(|n| n >= 1900.0).unwrap_or(true))
        .collect()
}

#[test]
fn time_ticks_across_zoomy_domains() {
    // One chart per span — minutes → hours → days → months → years; the tick
    // text (unit choice + calendar boundaries + rendering) is pinned whole.
    let spans = [
        ("minutes", "2026-03-04T09:07", "2026-03-04T09:26"),
        ("hours", "2026-03-04T03:12", "2026-03-04T21:40"),
        ("days", "2026-03-04", "2026-03-09T12:00"),
        ("weeks", "2026-03-04", "2026-04-20"),
        ("months", "2026-01-15", "2026-11-02"),
        ("years", "2019-06-01", "2026-02-01"),
        ("decades", "1985-01-01", "2026-01-01"),
    ];
    let mut out = String::new();
    for (name, a, b) in spans {
        let src = format!("|chart| [\n|line| {{ data: \"{a}\" 1, \"{b}\" 2 }}\n]\n");
        let svg = render_live(&src);
        out.push_str(&format!("{name}: {}\n", x_tick_texts(&svg).join(" | ")));
    }
    insta::assert_snapshot!(out);
}

#[test]
fn calendar_step_overrides_the_auto_unit() {
    let svg = render_live(
        "|chart| [\n|axis#t| { side: bottom; step: 2 month }\n|line| { data: \"2026-01-10\" 1, \"2026-07-20\" 2 }\n]\n",
    );
    let ticks = x_tick_texts(&svg);
    assert!(
        ticks.iter().any(|t| t == "Feb 2026") && ticks.iter().any(|t| t == "Apr 2026"),
        "{ticks:?}"
    );
    assert!(!ticks.iter().any(|t| t == "Mar 2026"), "{ticks:?}");
}

#[test]
fn time_axis_error_rows() {
    // Numeric step on a time axis.
    let err = lini::compile_str(
        "|chart| [\n|axis| { side: bottom; step: 5 }\n|line| { data: \"2026-01-01\" 1, \"2026-06-01\" 2 }\n]\n",
    )
    .unwrap_err();
    assert!(err.to_string().contains("steps by calendar"), "got: {err}");
    // Mixed date/numeric series.
    let err = lini::compile_str(
        "|chart| [\n|line| { data: \"2026-01-01\" 1, \"2026-06-01\" 2 }\n|dots| { data: 3 4, 5 6 }\n]\n",
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("mixes dates and numbers"),
        "got: {err}"
    );
    // An invalid date carries the literal.
    let err =
        lini::compile_str("|chart| [\n|line| { data: \"2026-13-01\" 1, \"2026-06-01\" 2 }\n]\n")
            .unwrap_err();
    assert!(
        err.to_string().contains("'2026-13-01' is not a date"),
        "got: {err}"
    );
    // scale: time belongs to the x axis.
    let err = lini::compile_str(
        "|chart| [\n|axis| { side: left; scale: time }\n|bars| { data: 1, 2 }\n]\n",
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("a value axis is numeric"),
        "got: {err}"
    );
}

#[test]
fn time_axis_date_preset_and_range_and_hover() {
    let svg = render_live(
        "|chart| [\n|axis#t| { side: bottom; format: year; range: \"2024-01-01\" \"2027-01-01\" }\n|line| \"S\" { data: \"2024-06-01\" 1, \"2026-06-01\" 2; marker: circle }\n]\n",
    );
    let ticks = x_tick_texts(&svg);
    assert!(ticks.iter().any(|t| t == "2025"), "{ticks:?}");
    // The authored date preset wins everywhere — ticks and hover alike.
    assert!(svg.contains("S: 2026, 2"), "{svg}");
    // Without a preset, hover shows the full instant.
    let svg = render_live(
        "|chart| [\n|line| \"S\" { data: \"2026-01-01\" 1, \"2026-06-01T09:30\" 2; marker: circle }\n]\n",
    );
    assert!(svg.contains("S: Jun 1 2026, 09:30, 2"), "{svg}");
}

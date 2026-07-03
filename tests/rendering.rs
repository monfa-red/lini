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
    let svg = render_baked("|box#n| \"one\\ntwo\"\n");
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
fn gap_color_accepts_a_gradient() {
    // SPEC §11.3: `gap-color` is a paint like `stroke`, so it takes a gradient — the
    // gutter rect fills with a `url(#…)` reference and the def is emitted.
    let svg = render_baked(
        "|row#r| { gap: 10; gap-color: gradient(red, blue) } [\n  |box#a| \"x\" { width: 40; height: 40 }\n  |box#b| \"y\" { width: 40; height: 40 }\n]\n",
    );
    assert!(
        svg.contains("<linearGradient"),
        "gradient def emitted: {svg}"
    );
    assert!(
        svg.contains(r#"fill="url(#lini-gradient-1)" stroke="none""#),
        "the gutter rect fills with the gradient: {svg}"
    );
}

#[test]
fn letter_spacing_bakes_a_dx_list_never_css() {
    // SPEC §10: letter-spacing compiles into a per-glyph `dx` list (geometry),
    // never a CSS property. "abc" → two 5px gaps.
    let svg = render_baked("|box| \"abc\" { letter-spacing: 5 }\n");
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
    let svg = render_baked("|box| \"one\\ntwo\" { line-spacing: 10 }\n");
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
            bake_vars: true,
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
  link-color: #666; link-width: 1;
  |box| { fill: lightyellow; }
  .loud { stroke: red; stroke-width: 2; }
  .calm { stroke: teal; }
  .wire { link-color: teal; }
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
    // A link's `.wire` class paints with the `link-*` family; its colour states
    // once as a `.lini-style-wire { stroke: … }` rule (mapped from `link-color:`), like
    // a node's stroke class — never inline on the link (SPEC §9/§13).
    assert!(
        svg.contains(".lini .lini-style-wire { stroke: teal; }"),
        "a link class's `link-color:` maps to a stroke rule: {}",
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
        svg.contains(r#"<text class="lini-text" x="0" y="0">3</text>"#),
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
    // constant: a |sign|'s 1.6 bakes auto 1.6 / 0.25 = 6.4, contain 1.6 / 0.3478 = 4.6 —
    // both draw 1.6px.
    let auto = render_live("|sign#x| { symbol: shield; fit: auto }\n");
    let contain = render_live("|sign#x| { symbol: shield; fit: contain }\n");
    assert!(auto.contains(r#"stroke-width="6.4""#), "{auto}");
    assert!(contain.contains(r#"stroke-width="4.6""#), "{contain}");
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
    let svg = render_baked(
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
    let svg = render_baked("|box#a|\n|box#b|\na -> b [ \"AB\\nCD\" { letter-spacing: 5 } ]\n");
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

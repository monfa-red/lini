use super::*;

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
fn font_size_on_container_reaches_descendant_text() {
    let svg = render_live("|group#g| \"hi\" { font-size: 10 }\n");
    assert!(svg.contains("font-size: 10px"), "{}", svg);
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

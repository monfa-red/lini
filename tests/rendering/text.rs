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

/// The live-CSS text family [SPEC 10]: each of these properties emits
/// verbatim where it is set — on an element it rides that box's `<g>` and
/// inherits to its text — and none of them has a baked default.
///
/// `text-shadow` is the one that rewrites: lini's unitless offsets and blur
/// gain `px`, colours pass through.
#[test]
fn every_live_css_text_property_emits_where_set() {
    for (decl, emitted) in [
        ("font-style: italic", "font-style: italic"),
        ("text-decoration: underline", "text-decoration: underline"),
        ("text-shadow: 1 1 2 gray", "text-shadow: 1px 1px 2px gray"),
    ] {
        let el = render_baked(&format!("|group#g| \"hi\" {{ {decl} }}\n"));
        assert!(el.contains(emitted), "{decl} emits where set: {el}");
        let property = decl.split(':').next().unwrap();
        assert!(
            !render_baked("|box| \"x\"\n").contains(property),
            "no baked default for {property}"
        );
    }
}

/// …and set globally, each states scene-wide on the `.lini` rule, exactly
/// like a global font-size.
#[test]
fn every_live_css_text_property_states_globally_on_the_lini_rule() {
    for (decl, emitted) in [
        ("font-style: italic", "font-style: italic"),
        (
            "text-decoration: line-through",
            "text-decoration: line-through",
        ),
        ("text-shadow: 2 2 black", "text-shadow: 2px 2px black"),
    ] {
        let rule = lini_root_rule(&render_baked(&format!("{{ {decl} }}\n|box| \"hi\"\n")));
        assert!(rule.contains(emitted), "global {decl}: {rule}");
    }
}

/// `text-transform` is baked, not live [SPEC 6]: the content is rewritten
/// before measurement, so the box fits the glyphs it draws and no CSS is
/// emitted — on a node's text, a link label, and scene-wide from the root.
#[test]
fn text_transform_bakes_into_the_measured_content() {
    let narrow = render_baked("|box| \"iiiiiiiiiiii\" { padding: 0 }\n");
    let upper = render_baked("|box| \"iiiiiiiiiiii\" { padding: 0; text-transform: uppercase }\n");
    assert!(upper.contains(">IIIIIIIIIIII<"), "{upper}");
    assert!(
        !upper.contains("text-transform"),
        "no live CSS for a baked prop: {upper}"
    );
    let width = |svg: &str| {
        let at = svg.find("<rect").expect("the box");
        let w = svg[at..].split("width=\"").nth(1).unwrap();
        w.split('"').next().unwrap().parse::<f64>().unwrap()
    };
    assert!(
        width(&upper) > width(&narrow) * 1.5,
        "the box grows to the capitals"
    );

    let scene = render_baked(
        "{ text-transform: uppercase }\n|box#a| \"hi\"\n|box#b| \"yo\"\na -> b \"via\"\n",
    );
    for run in [">HI<", ">YO<", ">VIA<"] {
        assert!(
            scene.contains(run),
            "{run} inherits the root transform: {scene}"
        );
    }
    let lower = render_baked("|box| \"Ab Cd\" { text-transform: capitalize }\n");
    assert!(lower.contains(">Ab Cd<"), "{lower}");
}

/// A `--static` export outlines only what the bundled face can draw
/// [SPEC 18]: a run holding any other character stays a live `<text>` —
/// never a row of `.notdef` boxes — and the export warns, naming the
/// characters, under a stable output code; a plain export is silent.
#[test]
fn static_leaves_uncovered_runs_as_text_and_warns() {
    let src = "|box#a| \"Grüße\"\n|box#b| \"你好\"\na -> b \"→\"\n";
    let baked = Options {
        static_mode: true,
        ..Default::default()
    };
    let (svg, diags) = lini::compile_str_checked(src, &baked).expect("compile");
    assert!(svg.contains(">你好<") && svg.contains(">→<"), "{svg}");
    assert!(!svg.contains(">Grüße<"), "a covered run outlines: {svg}");
    let codes: Vec<String> = diags.iter().map(|d| d.code.to_string()).collect();
    assert_eq!(codes, ["O001", "O001"], "{diags:?}");
    assert!(
        diags[0].message.contains("'你', '好'"),
        "{}",
        diags[0].message
    );
    assert_eq!(diags[0].span.start, src.find("\"你好\"").unwrap());

    let (_, live) = lini::compile_str_checked(src, &Options::default()).expect("compile");
    assert!(
        live.is_empty(),
        "a live export draws them by name: {live:?}"
    );
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
    let h = |svg: &str| scrape(svg, "height=\"")[0].parse::<f64>().expect("height");
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

/// `text-wrap:` [SPEC 5/6] is the escape hatch on `max-width:`. The default
/// `wrap` breaks the label into lines and holds the cap; `nowrap` refuses
/// rather than silently overflowing, naming both the cap and the way out.
#[test]
fn text_wrap_nowrap_refuses_the_cap_that_wrap_holds() {
    const LONG: &str = "|box#card| \"A rather long label that should wrap\"";
    let wrapped = lini::testing::route_sample(&format!("{LONG} {{ max-width: 160 }}\n"), 16.0);
    let (x0, _, x1, _) = lini::testing::node_rect(&wrapped, "card").expect("card");
    assert!(x1 - x0 <= 160.0 + 1e-6, "wrap holds the cap: {}", x1 - x0);
    assert!(
        render_live(&format!("{LONG} {{ max-width: 160 }}\n")).contains("<tspan"),
        "…by breaking the label into lines"
    );

    // `nowrap` on the same label and cap: an error, not an overflow.
    let err = lini::compile_str(&format!("{LONG} {{ max-width: 160; text-wrap: nowrap }}\n"))
        .expect_err("nowrap cannot fit");
    assert!(
        err.message
            .contains("text cannot fit 'max-width: 160' without wrapping")
            && err.message.contains("drop 'text-wrap: nowrap'"),
        "{}",
        err.message
    );

    // `nowrap` under a cap the text already fits is silent — it only refuses
    // the break it would otherwise have to make.
    lini::compile_str("|box#card| \"tiny\" { max-width: 400; text-wrap: nowrap }\n")
        .expect("nowrap is inert when the text fits");
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
    // The *horizontal* packing knob of the box holding the text left-flushes its
    // lines [SPEC 6] — `justify` in a row (the default direction), `align` in a
    // column — so the first (wider) line's centre sits right of the second's.
    for holder in [
        "|block#t| { max-width: 120; justify: start }",
        "|block#t| { max-width: 120; direction: column; align: start }",
    ] {
        let svg = render_live(&format!("{holder} [ \"wider line\\nshort\" ]\n"));
        let xs: Vec<f64> = scrape(&svg, "<tspan x=\"")
            .iter()
            .map(|x| x.parse().unwrap())
            .collect();
        assert_eq!(xs.len(), 2, "{svg}");
        assert!(
            xs[0] > xs[1],
            "{holder}: the wider line's centre sits right: {xs:?}"
        );
    }
    // Default stays centred — both lines share one x (today's output).
    let svg = render_live("|block#t| [ \"wider line\\nshort\" ]\n");
    let xs = scrape(&svg, "<tspan x=\"");
    assert!(xs.windows(2).all(|w| w[0] == w[1]), "{svg}");
}

#[test]
fn chrome_text_scales_with_the_inherited_body_size() {
    // [SPEC 6]: captions read 12/15 and link labels 11/15 of the inherited
    // font-size — one knob scales the scene; explicit sizes stay absolute.
    let s = lini::compile_str(
        "{ font-size: 30; }\n|group#g| \"Cap\" [\n  |box#a| \"x\"\n  |box#b| \"y\"\n  a -> b \"wire\"\n]\n",
    )
    .expect("compile");
    assert!(s.contains("font-size: 24px"), "caption 30 x 12/15: {s}");
    assert!(s.contains("font-size: 22px"), "link label 30 x 11/15: {s}");
    let s = lini::compile_str(
        "{ font-size: 30;\n  |caption| { font-size: 13 }\n  |-| { font-size: 9 }\n}\n|group#g| \"Cap\" [\n  |box#a| \"x\"\n  |box#b| \"y\"\n  a -> b \"wire\"\n]\n",
    )
    .expect("compile");
    assert!(
        s.contains("font-size: 13px"),
        "explicit caption absolute: {s}"
    );
    assert!(s.contains("font-size: 9px"), "explicit label absolute: {s}");
    // The default scene is byte-exact: 15 / 12 / 11, no ratio dust.
    let s = lini::compile_str(
        "|group#g| \"Cap\" [\n  |box#a| \"x\"\n  |box#b| \"y\"\n  a -> b \"wire\"\n]\n",
    )
    .expect("compile");
    assert!(s.contains("font-size: 12px"), "caption exactly 12: {s}");
    assert!(s.contains("font-size: 11px"), "label exactly 11: {s}");
}

#[test]
fn class_sized_chrome_stacks_its_lines_live_like_baked() {
    // [SPEC 5]: chrome built by `prim::text_classed` (a chart title) states its
    // size only in a class rule, so the live `<text>` must step its baselines
    // by the cascade-resolved size — the same step the outlined twin bakes,
    // never zero (which collapsed every line onto one baseline).
    let src = "|pie| \"Share\\nof revenue\" [ |slice| \"a\" { value: 1 } ]\n";

    let live = render_live(src);
    let text = live
        .rsplit_once("lini-chart-title")
        .and_then(|(_, r)| r.split_once("</text>"))
        .expect("live title")
        .0;
    let dy: f64 = scrape(text, " dy=\"")
        .first()
        .expect("second line dy")
        .parse()
        .expect("number");

    let baked = render_baked(src);
    let group = baked
        .rsplit_once("lini-chart-title")
        .and_then(|(_, r)| r.split_once("</g>"))
        .expect("baked title")
        .0;
    let mut ys: Vec<String> = scrape_to(group, "transform=\"translate(", ')')
        .iter()
        .filter_map(|t| t.split_whitespace().nth(1).map(str::to_string))
        .collect();
    ys.dedup();
    let step: f64 = ys[1].parse::<f64>().unwrap() - ys[0].parse::<f64>().unwrap();

    assert!(dy > 0.0, "live lines collapsed onto one baseline: {live}");
    assert!(
        (dy - step).abs() < 1e-9,
        "live dy {dy} must match the baked line step {step}"
    );
}

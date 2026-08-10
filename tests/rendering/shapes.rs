use super::*;

#[test]
fn line_missing_points_error_uses_pipe_sigil() {
    let err = lini::compile_str("|line#x|\n").expect_err("line needs points");
    assert!(
        err.to_string().contains("'|line|' requires 'points'"),
        "got: {}",
        err
    );
}

/// Each primitive's SVG element [SPEC 7]: the shape a type lowers to, one
/// row per type.
#[test]
fn every_primitive_emits_its_element() {
    for (src, wants) in [
        ("|hex| { width: 60; height: 60; }\n", &["<polygon"][..]),
        ("|diamond| { width: 60; height: 60; }\n", &["<polygon"]),
        (
            "|slant| { width: 80; height: 40; skew: 20; }\n",
            &["<polygon"],
        ),
        ("|oval| { width: 80; height: 40; }\n", &["<ellipse"]),
        ("|cyl| { width: 60; height: 80; }\n", &["<ellipse", "<path"]),
        ("|poly| { points: 0 0, 20 0, 10 20; }\n", &["<polygon"]),
    ] {
        let svg = render_live(src);
        for want in wants {
            assert!(svg.contains(want), "{src:?} must emit {want:?}: {svg}");
        }
    }
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

/// The whole `fit:` family on one glyph [SPEC 7]. A 64px `|sign|` holding
/// Phosphor's shield (own bounds ≈176×184, centred at y=140 in the 256 grid):
///
/// - `auto` maps the whole grid — the authored margin — at scale(0.25);
/// - `contain` measures the glyph and fits it uniformly, 64/184 = 0.3478,
///   recentred on the glyph rather than the grid;
/// - `cover` takes the larger ratio, 64/176 = 0.3636, so it overflows the
///   long axis;
/// - `stretch` fits each axis independently → a two-value scale.
///
/// The counter-scaled stroke follows the fit, so the on-screen weight is
/// constant: `|sign|`'s 2 bakes to 2/0.25 = 8 under auto and 2/0.3478 = 5.75
/// under contain — both draw 2px.
#[cfg(feature = "icons")]
#[test]
fn icon_fit_scales_the_glyph_and_holds_the_stroke_weight() {
    for (fit, transform, stroke) in [
        ("auto", "scale(0.25) translate(-128 -128)", Some("8")),
        (
            "contain",
            "scale(0.3478) translate(-128 -140)",
            Some("5.75"),
        ),
        ("cover", "scale(0.3636) translate(-128 -140)", None),
        ("stretch", "scale(0.3636 0.3478) translate(-128 -140)", None),
    ] {
        let svg = render_live(&format!("|sign#x| {{ symbol: shield; fit: {fit} }}\n"));
        assert!(
            svg.contains(&format!(r#"transform="{transform}""#)),
            "fit: {fit}: {svg}"
        );
        if let Some(w) = stroke {
            assert!(
                svg.contains(&format!(r#"stroke-width="{w}""#)),
                "fit: {fit} counter-scales the stroke: {svg}"
            );
        }
    }
    // `contain` genuinely enlarges past auto's plain grid mapping.
    assert!(
        !render_live("|sign#x| { symbol: shield; fit: contain }\n").contains("scale(0.25)"),
        "contain enlarges"
    );
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
    let svg = render_live(
        "|image#x| { src: \"https://example.com/a.png\"; width: 80; height: 40; fit: cover }\n",
    );
    assert!(
        svg.contains(r#"preserveAspectRatio="xMidYMid slice""#),
        "{svg}"
    );
}

#[test]
fn image_fit_stretch_sets_none() {
    let svg = render_live(
        "|image#x| { src: \"https://example.com/a.png\"; width: 80; height: 40; fit: stretch }\n",
    );
    assert!(svg.contains(r#"preserveAspectRatio="none""#), "{svg}");
}

#[test]
fn image_fit_auto_omits_preserve_aspect_ratio() {
    // auto / contain is the SVG default (xMidYMid meet) — nothing extra emitted.
    let svg =
        render_live("|image#x| { src: \"https://example.com/a.png\"; width: 80; height: 40 }\n");
    assert!(svg.contains("<image "), "{svg}");
    assert!(!svg.contains("preserveAspectRatio"), "{svg}");
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
    // A 5 m beam authored at ratio 1 [SPEC 21] — the hint names the fix.
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

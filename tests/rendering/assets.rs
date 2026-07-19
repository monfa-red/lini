use super::*;

// ─────────────────────── Local image assets [SPEC 7/17/19] ───────────────────────

#[test]
fn a_local_svg_asset_embeds_id_rewritten() {
    let svg = render_assets(
        "|image| { src: \"assets/logo.svg\"; width: 12; height: 12 }\n",
        None,
    )
    .expect("compile");
    // A nested `<svg>` mapped into the node box — placement ours, the asset's
    // viewBox kept, its redundant xmlns dropped.
    assert!(
        svg.contains(r#"<svg x="-6" y="-6" width="12" height="12" viewBox="0 0 48 48">"#),
        "{svg}"
    );
    // Every id prefixed `lini-a1-`, every internal reference following.
    assert!(svg.contains(r#"id="lini-a1-g""#), "{svg}");
    assert!(svg.contains("url(#lini-a1-g)"), "{svg}");
    assert!(svg.contains(r##"href="#lini-a1-dot""##), "{svg}");
    assert!(
        !svg.contains(r#"id="g""#) && !svg.contains("url(#g)"),
        "{svg}"
    );
}

#[test]
fn two_embedded_assets_never_collide() {
    let svg = render_assets(
        "|image| { src: \"assets/logo.svg\"; width: 12; height: 12 }\n|image| { src: \"assets/logo.svg\"; width: 12; height: 12 }\n",
        None,
    )
    .expect("compile");
    // Document-order numbering: the same asset twice gets distinct prefixes.
    assert!(svg.contains(r#"id="lini-a1-g""#), "{svg}");
    assert!(svg.contains(r#"id="lini-a2-g""#), "{svg}");
    assert!(svg.contains("url(#lini-a2-g)"), "{svg}");
}

#[test]
fn a_local_raster_embeds_as_a_data_uri() {
    let svg = render_assets(
        "|image| { src: \"assets/mark.png\"; width: 8; height: 8 }\n",
        None,
    )
    .expect("compile");
    assert!(
        svg.contains(r#"<image href="data:image/png;base64,iVBOR"#),
        "{svg}"
    );
    assert!(!svg.contains("mark.png"), "no path leaks: {svg}");
}

#[test]
fn urls_and_authored_data_uris_pass_through_unchanged() {
    let svg = render_assets(
        "|image| { src: \"https://example.com/x.png\"; width: 8; height: 8 }\n|image| { src: \"data:image/gif;base64,R0lGOD\"; width: 8; height: 8 }\n",
        None,
    )
    .expect("compile");
    assert!(svg.contains(r#"href="https://example.com/x.png""#), "{svg}");
    assert!(
        svg.contains(r#"href="data:image/gif;base64,R0lGOD""#),
        "{svg}"
    );
}

#[test]
fn a_missing_asset_errors_at_the_src_span() {
    let err = render_assets(
        "|image| { src: \"assets/nope.svg\"; width: 8; height: 8 }\n",
        None,
    )
    .expect_err("missing asset");
    assert!(
        err.to_string()
            .contains("cannot read image 'assets/nope.svg' — no such file"),
        "{err}"
    );
}

#[test]
fn an_asset_escaping_the_served_root_errors() {
    // base samples/, root samples/assets — the logo one level up escapes.
    let err = render_assets(
        "|image| { src: \"../Cargo.toml\"; width: 8; height: 8 }\n",
        Some("samples"),
    )
    .expect_err("escape");
    assert!(
        err.to_string()
            .contains("'../Cargo.toml' resolves outside the served root"),
        "{err}"
    );
    // The same file inside the boundary is fine.
    render_assets(
        "|image| { src: \"assets/logo.svg\"; width: 8; height: 8 }\n",
        Some("samples"),
    )
    .expect("in-root asset compiles");
}

#[test]
fn embedded_output_is_byte_identical_across_runs() {
    let src = std::fs::read_to_string("samples/drawing_sheet.lini").expect("read");
    let first = render_assets(&src, None).expect("compile");
    for _ in 0..2 {
        assert_eq!(render_assets(&src, None).expect("recompile"), first);
    }
}

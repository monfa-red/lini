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
    // Every id prefixed from the asset's own bytes, every internal reference
    // following [SPEC 18].
    let prefix = svg
        .split_once(r#"id="lini-a"#)
        .and_then(|(_, rest)| rest.split_once('-'))
        .map(|(tag, _)| format!("lini-a{tag}-"))
        .expect("an asset id prefix");
    assert!(svg.contains(&format!(r#"id="{prefix}g""#)), "{svg}");
    assert!(svg.contains(&format!("url(#{prefix}g)")), "{svg}");
    assert!(svg.contains(&format!(r##"href="#{prefix}dot""##)), "{svg}");
    assert!(
        !svg.contains(r#"id="g""#) && !svg.contains("url(#g)"),
        "{svg}"
    );
}

#[test]
fn the_same_asset_twice_shares_one_prefix() {
    // The prefix is the asset's own bytes [SPEC 18], not its document order, so
    // two figures inlined in one page can never cross-wire a `url(#…)`: a
    // shared prefix means byte-identical embeddings, where sharing resolves to
    // an equal definition either way. (`name::prefixes_differ_by_asset` pins
    // the other half — distinct bytes, distinct prefix.)
    let svg = render_assets(
        "|image| { src: \"assets/logo.svg\"; width: 12; height: 12 }\n|image| { src: \"assets/logo.svg\"; width: 12; height: 12 }\n",
        None,
    )
    .expect("compile");
    let prefixes: Vec<&str> = scrape(&svg, r#"id="lini-a"#);
    assert!(prefixes.len() >= 2, "both copies embed: {svg}");
    let tag = |p: &str| p.split_once('-').map(|(t, _)| t.to_string());
    assert_eq!(tag(prefixes[0]), tag(prefixes.last().unwrap()), "{svg}");
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
fn no_fs_refuses_every_local_path_before_any_read() {
    // `--no-fs` [SPEC 7/20]: a host compiling source it did not write. The
    // refusal comes before the path is opened, so a file that exists and one
    // that does not read the same — the error says nothing about the disk.
    let compile = |src: &str, assets: &[(&str, &[u8])]| {
        lini::compile_str_with(
            src,
            &Options {
                base_dir: Some("samples".into()),
                no_fs: true,
                assets: assets
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_vec()))
                    .collect(),
                ..Default::default()
            },
        )
    };
    for path in ["assets/logo.svg", "assets/nope.svg", "/etc/passwd"] {
        let err = compile(
            &format!("|image| {{ src: \"{path}\"; width: 8; height: 8 }}\n"),
            &[],
        )
        .expect_err("a local path under --no-fs");
        assert_eq!(
            err.to_string(),
            format!(
                "error: cannot read image '{path}' — local files are off (--no-fs); use an http(s) URL or a data: URI"
            ),
        );
    }
    // URLs and data URIs are not reads; they pass as always.
    compile(
        "|image| { src: \"https://example.com/x.png\"; width: 8; height: 8 }\n|image| { src: \"data:image/gif;base64,R0lGOD\"; width: 8; height: 8 }\n",
        &[],
    )
    .expect("URLs compile");
    // Bytes the host hands over are not reads either — they still embed.
    let logo = std::fs::read("samples/assets/logo.svg").expect("the sample logo");
    let svg = compile(
        "|image| { src: \"logo.svg\"; width: 8; height: 8 }\n",
        &[("logo.svg", &logo)],
    )
    .expect("a supplied asset compiles");
    assert!(svg.contains(r#"viewBox="0 0 48 48""#), "{svg}");
}

//! Conformance suite — every `samples/*.lini` file is compiled with
//! `--static` and its SVG output snapshotted via `insta`. Changes that
//! shift any sample's output surface as a snapshot diff, surfacing
//! regressions across all SPEC features at once.
//!
//! Bake mode is the default snapshot because it produces hermetic output:
//! no `var(...)` indirection, every literal frozen. Live mode is pinned by
//! `snapshot_live_svg_for_hello` below and by the targeted assertions in
//! `tests/rendering/`.
//!
//! The routing-oracle scenes are *not* here: they live in
//! `tests/fixtures/routing/`, outside this glob, because routing is gated
//! semantically by `tests/laws.rs` and `tests/routing.rs` (laws, crossing
//! counts, determinism) and never by snapshots — a snapshot would pin one
//! router's coordinates and churn on every phase.

use lini::testing::{read_sample, sample_opts};
use lini::{Options, OutputFormat};

/// Collapse the outline payload of `<defs>` glyph paths.
///
/// A glyph outline is a pure function of the committed font subset — no
/// diagram change can move it — yet at ~500 KB it was a third of all snapshot
/// bytes and diffed on every re-bless. The sweep still pins *which* glyph is
/// cut, that it is deduped, and every `<use>` that places it; only the curve
/// data is elided. The curves themselves are pinned byte-for-byte once, by
/// `glyph_outlines_are_pinned_once`.
const GLYPH_OUTLINES: (&str, &str) = (r#"(<path id="lini-g[^"]*" d=")[^"]*""#, "${1}…\"");

fn baked_opts() -> Options {
    Options {
        static_mode: true,
        format: OutputFormat::Svg,
        ..sample_opts()
    }
}

#[test]
fn snapshot_baked_svg_for_every_sample() {
    // The snapshots carry `--static` **outlined** text [SPEC 18]; without the
    // `font` feature outlining is inert (text stays `<text>`), so there is
    // nothing meaningful to compare — same policy as the icons skip below.
    if !cfg!(feature = "font") {
        return;
    }
    let samples_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("samples");
    let opts = baked_opts();

    insta::with_settings!({filters => vec![GLYPH_OUTLINES]}, {
        insta::glob!(&samples_dir, "*.lini", |path| {
            let src = std::fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
            // Icons need the `icons` feature; skip icon-using samples when it's off
            // (their non-icon siblings render identically with or without it).
            if !cfg!(feature = "icons") && src.contains("|icon|") {
                return;
            }
            let svg = lini::compile_str_with(&src, &opts)
                .unwrap_or_else(|e| panic!("{}: compile failed: {}", path.display(), e));
            insta::assert_snapshot!(svg);
        });
    });
}

/// The one place the outlined curves themselves are pinned: `hello.lini`'s
/// five glyphs, whole. Everything the sweep above elides is here, so a font
/// subset that silently re-cut its outlines still fails the suite.
#[test]
fn glyph_outlines_are_pinned_once() {
    if !cfg!(feature = "font") {
        return;
    }
    let src = read_sample(&sample_opts().base_dir.unwrap().join("hello.lini"));
    let svg = lini::compile_str_with(&src, &baked_opts()).expect("compile hello.lini");
    let start = svg.find("<defs>").expect("glyph defs");
    let end = svg.find("</defs>").expect("glyph defs close") + "</defs>".len();
    insta::assert_snapshot!(&svg[start..end]);
}

/// Live mode's counterpart to the sweep: the same sheet with `var(...)`
/// indirection intact and text still a `<text>` element.
#[test]
fn snapshot_live_svg_for_hello() {
    let src = read_sample(&sample_opts().base_dir.unwrap().join("hello.lini"));
    let svg = lini::compile_str(&src).expect("compile hello.lini");
    insta::assert_snapshot!(svg);
}

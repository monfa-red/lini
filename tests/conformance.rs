//! Conformance suite — every `samples/*.lini` file is compiled with
//! `--static` and its SVG output snapshotted via `insta`. Changes that
//! shift any sample's output surface as a snapshot diff, surfacing
//! regressions across all SPEC features at once.
//!
//! Bake mode is the default snapshot because it produces hermetic output:
//! no `var(...)` indirection, every literal frozen. Live-mode snapshots
//! are covered by the dedicated tests in `tests/rendering.rs`.

use lini::{Options, OutputFormat};

/// Link-bearing samples are excluded: routing is gated semantically by
/// `tests/laws.rs` and `tests/routing.rs` (laws, crossing counts, determinism), never by snapshots —
/// a snapshot would pin one router's coordinates and churn on every phase.
const LINK_SAMPLES: &[&str] = &["links_simple.lini", "links_medium.lini", "links_hard.lini"];

#[test]
fn snapshot_baked_svg_for_every_sample() {
    // The snapshots carry `--static` **outlined** text [SPEC 17]; without the
    // `font` feature outlining is inert (text stays `<text>`), so there is
    // nothing meaningful to compare — same policy as the icons skip below.
    if !cfg!(feature = "font") {
        return;
    }
    let samples_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("samples");
    let opts = Options {
        static_mode: true,
        format: OutputFormat::Svg,
        // Samples resolve their image assets against their own dir [SPEC 7].
        base_dir: Some(samples_dir.clone()),
        ..Default::default()
    };

    insta::glob!(samples_dir, "*.lini", |path| {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if LINK_SAMPLES.contains(&name) {
            return;
        }
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
}

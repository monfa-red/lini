//! Desugar transparency: compiling the lowered form must byte-match compiling the
//! source over every sample. Since `compile` already desugars, this proves desugar
//! is a fixed point through the whole pipeline (parse → desugar → resolve → render).

use lini::{Options, OutputFormat};

fn svg(src: &str) -> String {
    let opts = Options {
        static_mode: true,
        format: OutputFormat::Svg,
        // Samples resolve their image assets against their own dir [SPEC 7].
        base_dir: Some(std::path::PathBuf::from("samples")),
        ..Default::default()
    };
    lini::compile_str_with(src, &opts).expect("compile")
}

/// The no-spill law [SPEC 15.8]: on every `|page|` sample, no view or its
/// annotations may cross the sheet's inner frame — the packer counts each
/// view's full extent (annotations included) against the content area. Guards
/// the tapped-bush frame rider from returning.
#[test]
fn no_view_or_annotation_crosses_the_sheet_frame() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("samples");
    let opts = Options {
        base_dir: Some(std::path::PathBuf::from("samples")),
        ..Default::default()
    };
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("lini") {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        if !src.contains("|page|") {
            continue;
        }
        if !cfg!(feature = "icons") && src.contains("|icon|") {
            continue;
        }
        let laid = lini::testing::layout_sample(&src, &opts);
        let spills = lini::testing::frame_overflow(&laid);
        assert!(
            spills.is_empty(),
            "{}: content crosses the sheet frame:\n  {}",
            path.display(),
            spills.join("\n  ")
        );
    }
}

#[test]
fn compile_is_transparent_to_desugar_for_every_sample() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("samples");
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("lini") {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        // Icons need the `icons` feature; skip icon-using samples when it's off.
        if !cfg!(feature = "icons") && src.contains("|icon|") {
            continue;
        }
        let lowered = lini::desugar_source(&src).expect("desugar");
        assert_eq!(
            svg(&src),
            svg(&lowered),
            "{}: compile(src) != compile(desugar(src))",
            path.display()
        );
    }
}

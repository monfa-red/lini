//! Desugar transparency: compiling the lowered form must byte-match compiling the
//! source over every sample. Since `compile` already desugars, this proves desugar
//! is a fixed point through the whole pipeline (parse → desugar → resolve → render).

use lini::OutputFormat;
use lini::testing::{read_sample, sample_opts, samples};

fn svg(src: &str) -> String {
    let opts = lini::Options {
        static_mode: true,
        format: OutputFormat::Svg,
        ..sample_opts()
    };
    lini::compile_str_with(src, &opts).expect("compile")
}

/// The no-spill law [SPEC 15.8]: on every `|page|` sample, no view or its
/// annotations may cross the sheet's inner frame — the packer counts each
/// view's full extent (annotations included) against the content area. Guards
/// the tapped-bush frame rider from returning.
#[test]
fn no_view_or_annotation_crosses_the_sheet_frame() {
    for path in samples() {
        let src = read_sample(&path);
        if !src.contains("|page|") {
            continue;
        }
        let laid = lini::testing::layout_sample(&src, &sample_opts());
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
    for path in samples() {
        let src = read_sample(&path);
        let lowered = lini::desugar_source(&src).expect("desugar");
        assert_eq!(
            svg(&src),
            svg(&lowered),
            "{}: compile(src) != compile(desugar(src))",
            path.display()
        );
    }
}

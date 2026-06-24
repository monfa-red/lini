//! Desugar transparency: compiling the lowered form must byte-match compiling the
//! source over every sample. Since `compile` already desugars, this proves desugar
//! is a fixed point through the whole pipeline (parse → desugar → resolve → render).

use lini::{Options, OutputFormat};

fn svg(src: &str) -> String {
    let opts = Options {
        bake_vars: true,
        format: OutputFormat::Svg,
        ..Default::default()
    };
    lini::compile_str_with(src, &opts).expect("compile")
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

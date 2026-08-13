//! Lini's compiler, exposed to JavaScript.
//!
//! A binding layer and nothing else: every function here forwards to the
//! library's own entry point ([`lini::compile_str_with`], [`lini::desugar_source`],
//! [`lini::diagnostics_json`]), so a browser runs the *same* engine as the
//! binary — byte for byte, guarded by `tests/wasm.rs`. No compiler logic lives
//! in this crate, and none may: a second lowering path is exactly the drift the
//! byte-equality test exists to catch.

use lini::{Options, OutputFormat};
use wasm_bindgen::prelude::*;

/// Compile Lini source to SVG.
///
/// Throws on any error-level diagnostic, with the compiler's own LSP-shaped
/// message (`play.lini:3:5: error: …`) as the thrown value.
#[wasm_bindgen]
pub fn compile(src: &str) -> Result<String, JsError> {
    lini::compile_str(src).map_err(js_err)
}

/// Compile to a full HTML page rather than a bare SVG — the `--format html`
/// output, for a preview pane that wants a self-contained document.
#[wasm_bindgen]
pub fn compile_html(src: &str) -> Result<String, JsError> {
    let opts = Options {
        format: OutputFormat::Html,
        ..Options::default()
    };
    lini::compile_str_with(src, &opts).map_err(js_err)
}

/// Compile with `var()` references inlined and text outlined to paths — the
/// `--static` output. Self-contained for download, or for a canvas rasteriser.
#[wasm_bindgen]
pub fn compile_static(src: &str) -> Result<String, JsError> {
    let opts = Options {
        static_mode: true,
        ..Options::default()
    };
    lini::compile_str_with(src, &opts).map_err(js_err)
}

/// Every diagnostic as the JSON document `--json` emits — stable codes, spans,
/// severities, and machine-applicable fixes. Never throws: a file that cannot
/// compile still reports why, which is what an editor's gutter wants.
#[wasm_bindgen]
pub fn diagnostics(src: &str) -> String {
    lini::diagnostics_json(src, &Options::default(), "play.lini").0
}

/// The source with every bit of sugar lowered to primitives — what `lini
/// desugar` prints. The teaching view.
#[wasm_bindgen]
pub fn desugar(src: &str) -> Result<String, JsError> {
    lini::desugar_source(src).map_err(js_err)
}

/// Canonical formatting — what `lini fmt` writes.
#[wasm_bindgen]
pub fn format(src: &str) -> Result<String, JsError> {
    lini::format_source(src).map_err(js_err)
}

/// The compiler's version, so a page can show which engine it is running.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn js_err(e: lini::Error) -> JsError {
    JsError::new(&e.to_string())
}

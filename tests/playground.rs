//! The playground's tokenizer highlights every sample **byte-identically** to
//! `lini::highlight_html`.
//!
//! One grammar has one scanner — `src/grammar/highlight.rs` — and every host
//! that can call Rust calls it: `mdbook-lini` links the crate, a static site
//! shells out to `lini highlight`, a page with wasm calls the `highlight`
//! export. `src/serve/playground.html` is the one host that can do none of
//! those: it ships no wasm, and its overlay must re-colour on every keystroke,
//! so it carries a hand-written copy in JavaScript.
//!
//! A copy is only honest if it is *proven* equal, which is what this is — the
//! same bargain `tests/wasm.rs` strikes with the browser compiler and
//! `tests/grammar.rs` with the editor grammars: the derived thing may not
//! drift from its source. The JS is lifted verbatim from the region marked
//! `<lini:tokenizer>`; nothing is re-typed, so what runs here is what the page
//! runs.
//!
//! Running it needs `node`. Absent it, the test **skips with a note** — except
//! under `LINI_JS_REQUIRED=1`, where it fails instead. CI sets that, so the
//! skip can never quietly become the permanent state.

use std::path::PathBuf;
use std::process::Command;

const BEGIN: &str = "// <lini:tokenizer>";
const END: &str = "// </lini:tokenizer>";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Skip unless CI demanded the run, in which case fail with the same reason.
fn unavailable(reason: &str) {
    if std::env::var("LINI_JS_REQUIRED").is_ok_and(|v| v == "1") {
        panic!("LINI_JS_REQUIRED=1 but {reason}");
    }
    eprintln!("skipping playground parity: {reason}");
}

fn have_node() -> bool {
    Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// The marked region of the page, as an ES module exporting `tokenize`.
fn tokenizer_module(page: &str) -> String {
    let start = page
        .find(BEGIN)
        .unwrap_or_else(|| panic!("playground.html has no {BEGIN} marker"));
    let end = page
        .find(END)
        .unwrap_or_else(|| panic!("playground.html has no {END} marker"));
    assert!(start < end, "the tokenizer markers are inverted");
    format!("{}\nexport {{ tokenize }};\n", &page[start..end])
}

#[test]
fn the_playground_tokenizer_matches_the_rust_scanner() {
    let root = repo_root();
    if !have_node() {
        return unavailable("node is not installed");
    }

    let page = std::fs::read_to_string(root.join("src/serve/playground.html"))
        .expect("read the playground");
    let out = root.join("target/playground-parity");
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("create the comparison directory");
    let module = out.join("tokenizer.mjs");
    std::fs::write(&module, tokenizer_module(&page)).expect("write the lifted tokenizer");

    let samples = lini::testing::samples();
    assert!(!samples.is_empty(), "no samples to compare");

    let mut cmd = Command::new("node");
    cmd.arg(root.join("tests/fixtures/playground_driver.mjs"))
        .arg(&module)
        .arg(&out);
    for s in &samples {
        cmd.arg(s);
    }
    let run = cmd.output().expect("run the playground driver");
    assert!(
        run.status.success(),
        "the playground driver failed:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );

    let mut drift = Vec::new();
    for (i, path) in samples.iter().enumerate() {
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let name = format!("{i}-{stem}");
        let rust = lini::highlight_html(&lini::testing::read_sample(path));
        let js = std::fs::read_to_string(out.join(format!("{name}.html")))
            .unwrap_or_else(|_| format!("<the driver wrote no output for {name}>"));
        if rust != js {
            drift.push(describe(&name, &rust, &js));
        }
    }

    assert!(
        drift.is_empty(),
        "the playground tokenizer has drifted from lini::highlight_html on {} sample(s):\n\n{}",
        drift.len(),
        drift.join("\n\n")
    );
}

/// Name the first byte that differs, with a window of context each side — a
/// whole-listing diff is unreadable and the first divergence is what matters.
fn describe(name: &str, rust: &str, js: &str) -> String {
    let at = rust
        .bytes()
        .zip(js.bytes())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| rust.len().min(js.len()));
    let from = at.saturating_sub(60);
    format!(
        "{name}: first difference at byte {at} (rust {} B, js {} B)\n  rust: …{}…\n  js:   …{}…",
        rust.len(),
        js.len(),
        window(rust, from, at + 60),
        window(js, from, at + 60),
    )
}

fn window(s: &str, from: usize, to: usize) -> String {
    let to = to.min(s.len());
    let from = from.min(to);
    s.get(from..to)
        .unwrap_or("<not a char boundary>")
        .to_string()
}

/// The page must keep the markers the lift depends on, whether or not node is
/// installed — otherwise a rename would turn the guard above into a silent
/// skip on every machine at once.
#[test]
fn the_page_still_marks_its_tokenizer() {
    let page = std::fs::read_to_string(repo_root().join("src/serve/playground.html"))
        .expect("read the playground");
    let module = tokenizer_module(&page);
    assert!(
        module.contains("function tokenize(src)") && module.contains("const esc ="),
        "the marked region no longer holds the whole tokenizer"
    );
    assert!(
        !module.contains("document.") && !module.contains("editor."),
        "the marked region must not touch the DOM — it runs in node"
    );
}

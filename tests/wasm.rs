//! The browser artifact compiles — and highlights — every sample
//! **byte-identically** to the binary.
//!
//! `crates/lini-wasm` is a binding layer with no compiler logic of its own, and
//! this is what keeps it that way: the moment someone reimplements a lowering
//! step "just for the web", a sample's bytes move and this test says so. It is
//! the same guarantee `tests/schema.rs` and `tests/grammar.rs` give the
//! generated artifacts — the generated thing may not drift from its source.
//!
//! Scope is the **default** compile (live `var()`s, text as `<text>`). The wasm
//! build drops the `font` feature, which changes `--static` output alone
//! (outlining needs the subsets) and never measurement — the metrics tables
//! compile in unconditionally [SPEC 6].
//!
//! Running it needs two things the plain `cargo test` has no business
//! requiring: `node`, and a built `crates/lini-wasm/pkg/`. Absent either, the
//! test **skips with a note** — except under `LINI_WASM_REQUIRED=1`, where it
//! fails instead. CI sets that, so the skip can never quietly become the
//! permanent state.

use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Skip unless CI demanded the run, in which case fail with the same reason.
fn unavailable(reason: &str) {
    if std::env::var("LINI_WASM_REQUIRED").is_ok_and(|v| v == "1") {
        panic!("LINI_WASM_REQUIRED=1 but {reason}");
    }
    eprintln!("skipping wasm parity: {reason}");
    eprintln!("  build it with `cargo xtask wasm`");
}

fn have(tool: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

#[test]
fn wasm_matches_the_binary_on_every_sample() {
    let root = repo_root();
    let pkg = root.join("crates/lini-wasm/pkg");

    if !pkg.join("lini_wasm_bg.wasm").is_file() {
        return unavailable("crates/lini-wasm/pkg is not built");
    }
    if !have("node") {
        return unavailable("node is not installed");
    }

    // Samples that need an `|image| src:` resolve it against `samples/`; the
    // browser build has no filesystem, so those cannot round-trip here.
    let samples: Vec<PathBuf> = lini::testing::samples()
        .into_iter()
        .filter(|p| !lini::testing::read_sample(p).contains("|image|"))
        .collect();
    assert!(!samples.is_empty(), "no samples to compare");

    let out = root.join("target/wasm-parity");
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("create the comparison directory");

    let mut cmd = Command::new("node");
    cmd.arg(root.join("crates/lini-wasm/tests/driver.mjs"))
        .arg(&pkg)
        .arg(&out);
    for s in &samples {
        cmd.arg(s);
    }
    let run = cmd.output().expect("run the wasm driver");
    assert!(
        run.status.success(),
        "the wasm driver failed:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );

    let mut drift = Vec::new();
    for (i, path) in samples.iter().enumerate() {
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        // Outputs are keyed by list position, mirroring the driver: the
        // corpus carries a samples/ sheet and a routing fixture that share
        // the basename links_hard.lini, and a flat name once let the
        // fixture's compile silently overwrite the sample's — the "drift"
        // it then reported was two different sources, not two engines.
        let name = format!("{i}-{stem}");
        let source = lini::testing::read_sample(path);
        let native = lini::compile_str(&source)
            .unwrap_or_else(|e| panic!("{stem} does not compile natively: {e}"));
        let browser = std::fs::read_to_string(out.join(format!("{name}.svg")))
            .unwrap_or_else(|_| format!("<the driver wrote no output for {name}>"));
        if native != browser {
            drift.push(describe(&name, &native, &browser));
        }
        // The binding layer carries a tokenizer as well as a compiler, and it
        // is bound the same way — one call through to the library, guarded the
        // same way [SPEC 22].
        let native = lini::highlight_html(&source);
        let browser = std::fs::read_to_string(out.join(format!("{name}.html")))
            .unwrap_or_else(|_| format!("<the driver highlighted nothing for {name}>"));
        if native != browser {
            drift.push(describe(&format!("{name} (highlight)"), &native, &browser));
        }
    }

    assert!(
        drift.is_empty(),
        "the browser build has drifted from the binary on {} sample(s):\n\n{}",
        drift.len(),
        drift.join("\n\n")
    );
}

/// Name the first byte that differs, with a window of context each side — a
/// whole-SVG diff is unreadable and the first divergence is what matters.
fn describe(name: &str, native: &str, browser: &str) -> String {
    let at = native
        .bytes()
        .zip(browser.bytes())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| native.len().min(browser.len()));
    let from = at.saturating_sub(60);
    format!(
        "{name}: first difference at byte {at} (native {} B, wasm {} B)\n  native: …{}…\n  wasm:   …{}…",
        native.len(),
        browser.len(),
        window(native, from, at + 60),
        window(browser, from, at + 60),
    )
}

fn window(s: &str, from: usize, to: usize) -> String {
    let to = to.min(s.len());
    let from = from.min(to);
    s.get(from..to)
        .unwrap_or("<not a char boundary>")
        .to_string()
}

/// The parity test is worthless if it silently compares nothing, so pin the
/// corpus it walks: every sample the showroom ships, minus the image-bearing
/// ones the browser cannot read.
#[test]
fn the_parity_corpus_is_not_empty() {
    let covered = lini::testing::samples()
        .into_iter()
        .filter(|p| !lini::testing::read_sample(p).contains("|image|"))
        .count();
    assert!(
        covered >= 25,
        "only {covered} samples feed the wasm parity check — has the corpus moved?"
    );
}

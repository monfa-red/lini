//! Stage 4 drift guard [BETA-tooling, ROADMAP 3.8]. The editor grammars take
//! their keyword lists from the property ledger and are committed under
//! `editors/`; here we regenerate them in memory and assert byte-equality, so a
//! stale checkout fails CI and never ships — the same guarantee the schema has.

use std::path::{Path, PathBuf};

fn editors_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("editors")
}

#[test]
fn vscode_grammar_matches_committed_byte_for_byte() {
    let path = editors_dir().join("vscode/syntaxes/lini.tmLanguage.json");
    let committed = std::fs::read_to_string(&path)
        .expect("editors/vscode/syntaxes/lini.tmLanguage.json — run `cargo xtask gen-grammars`");
    assert_eq!(
        committed,
        lini::vscode_grammar(),
        "VS Code grammar drift — regenerate with `cargo xtask gen-grammars` and commit"
    );
}

#[test]
fn zed_highlights_match_committed_byte_for_byte() {
    let path = editors_dir().join("zed/languages/lini/highlights.scm");
    let committed = std::fs::read_to_string(&path)
        .expect("editors/zed/languages/lini/highlights.scm — run `cargo xtask gen-grammars`");
    assert_eq!(
        committed,
        lini::zed_highlights(),
        "Zed highlights drift — regenerate with `cargo xtask gen-grammars` and commit"
    );
}

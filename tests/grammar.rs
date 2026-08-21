//! **One word source, three homes** [PLAN-PRE-V1 chunk 5]. The grammar is
//! written down three times — the real lexer/parser, the editor grammars under
//! `editors/`, and the playground tokenizer in `src/serve/playground.html` — so
//! the two generated homes take every keyword list from `src/grammar/vocab.rs`,
//! which derives each set from the table that already owns it. Here we
//! regenerate all three in memory and assert byte-equality with the committed
//! files, so a stale checkout fails CI and never ships — the same guarantee the
//! schema has.
//!
//! Two guards sit beside the drift check: SPEC 23's own list of contextual
//! value keywords is the fixture for the value-keyword set (the doc is the
//! source, as in `tests/spec_blocks.rs` and `tests/hooks.rs`), and a spot-check
//! that a word which reaches one home reaches all three.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel))
        .unwrap_or_else(|e| panic!("{rel}: {e} — run `cargo xtask gen-grammars`"))
}

#[test]
fn vscode_grammar_matches_committed_byte_for_byte() {
    assert_eq!(
        read("editors/vscode/syntaxes/lini.tmLanguage.json"),
        lini::vscode_grammar(),
        "VS Code grammar drift — regenerate with `cargo xtask gen-grammars` and commit"
    );
}

#[test]
fn zed_highlights_match_committed_byte_for_byte() {
    assert_eq!(
        read("editors/zed/languages/lini/highlights.scm"),
        lini::zed_highlights(),
        "Zed highlights drift — regenerate with `cargo xtask gen-grammars` and commit"
    );
}

/// The playground's tokenizer is hand-written (it must preserve every
/// character); only its marked word-list region is generated, so the guard is a
/// re-splice that must change nothing.
#[test]
fn playground_word_lists_match_committed_byte_for_byte() {
    let src = read("src/serve/playground.html");
    assert_eq!(
        lini::splice_playground(&src),
        src,
        "playground tokenizer drift — regenerate with `cargo xtask gen-grammars` and commit"
    );
}

/// SPEC 23 names the contextual value keywords one by one; every one of them
/// must be in the generated value-keyword alternation, or the playground and
/// both editors quietly stop colouring a word the language calls a keyword.
#[test]
fn spec_23_value_keywords_all_highlight() {
    let spec = read("SPEC.md");
    let start = spec
        .find("Value keywords are **contextual**")
        .expect("SPEC 23 states the contextual value keywords");
    let end = start
        + spec[start..]
            .find("**Every built-in type**")
            .expect("the contextual-keyword sentence runs up to the built-in-type one");
    let vscode = lini::vscode_grammar();
    let zed = lini::zed_highlights();
    let playground = read("src/serve/playground.html");
    for word in backticked(&spec[start..end]) {
        for (home, text) in [
            ("VS Code", &vscode),
            ("Zed", &zed),
            ("playground", &playground),
        ] {
            assert!(
                in_alternation(text, &word),
                "SPEC 23 keyword '{word}' is missing from the {home} grammar"
            );
        }
    }
}

/// A word that reaches one home reaches all three — the drift this chunk
/// closes, pinned on one representative per set: a template type, two ledger
/// properties, a builder call, a marker glyph, a layout name.
#[test]
fn every_home_carries_the_same_vocabulary() {
    let vscode = lini::vscode_grammar();
    let zed = lini::zed_highlights();
    let playground = read("src/serve/playground.html");
    for word in ["FB", "revolve", "thread", "oklch", "diamond", "schematic"] {
        for (home, text) in [
            ("VS Code", &vscode),
            ("Zed", &zed),
            ("playground", &playground),
        ] {
            assert!(
                in_alternation(text, word),
                "'{word}' is missing from the {home} grammar"
            );
        }
    }
}

/// Whether `word` appears as a whole alternative of some regex alternation in
/// `text` — bounded by the `|` that separates alternatives or by the group's
/// own parens, so `dot` never matches inside `dotted`.
fn in_alternation(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        matches!(before, Some('|' | '(' | ':')) && matches!(after, Some('|' | ')'))
    })
}

/// Every backticked token in `text`, in order.
fn backticked(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else { break };
        out.push(after[..close].to_string());
        rest = &after[close + 1..];
    }
    out
}

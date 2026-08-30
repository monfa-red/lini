//! **One word source, four homes** [PLAN-PRE-V1 chunk 5]. The grammar is
//! written down three times — the real lexer/parser, the editor grammars under
//! `editors/`, and the playground tokenizer in `src/serve/playground.html` — so
//! the two generated homes take every keyword list from `src/grammar/vocab.rs`,
//! which derives each set from the table that already owns it. Here we
//! regenerate all three in memory and assert byte-equality with the committed
//! files, so a stale checkout fails CI and never ships — the same guarantee the
//! schema has. The fourth home, `lini::highlight_html`, generates no file, so
//! the two cross-home guards below ask it the same questions **by behaviour**:
//! a word the editors colour must come back marked from the scanner too. (Its
//! exhaustive per-set sweep — every ledger property, every built-in type, every
//! value keyword — lives beside it in `src/grammar/highlight.rs`, and the
//! playground's hand-written twin is held byte-identical to it by
//! `tests/playground.rs`.)
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
        // The fourth home generates no file, so it is asked the same question
        // by behaviour: put the word where a value goes and it must come back
        // marked as a keyword.
        assert!(
            marks(&format!("|box| {{ p: {word}; }}"), &word, "keyword"),
            "SPEC 23 keyword '{word}' is not a keyword to lini::highlight_html"
        );
    }
}

/// A word that reaches one home reaches all four — the drift this chunk
/// closes, pinned on one representative per set: a template type, two ledger
/// properties, a builder call, a marker glyph, a layout name. The three
/// generated homes are asked of their text; `lini::highlight_html` is asked of
/// its output, each word in the position that gives it its meaning.
#[test]
fn every_home_carries_the_same_vocabulary() {
    let vscode = lini::vscode_grammar();
    let zed = lini::zed_highlights();
    let playground = read("src/serve/playground.html");
    for (word, probe, class) in [
        ("FB", "|FB|", "type"),
        ("revolve", "|sketch| { revolve: x-axis; }", "prop"),
        ("thread", "|sketch| { thread: m8 1.5; }", "prop"),
        ("oklch", "|box| { fill: oklch(0.7, 0.1, 200); }", "type"),
        ("diamond", "|-| { marker: diamond; }", "keyword"),
        ("schematic", "{ layout: schematic; }", "keyword"),
    ] {
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
        assert!(
            marks(probe, word, class),
            "lini::highlight_html does not mark '{word}' as tok-{class} in {probe:?}"
        );
    }
}

/// Whether highlighting `src` marks `word` with `tok-<class>`.
fn marks(src: &str, word: &str, class: &str) -> bool {
    lini::highlight_html(src).contains(&format!("<span class=\"tok-{class}\">{word}</span>"))
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

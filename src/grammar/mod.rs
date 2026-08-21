//! Grammar generation — one word source, three homes [SPEC 22 / 23].
//!
//! The surface grammar is written down in three places that can drift: the real
//! lexer/parser, the editor grammars under `editors/`, and the playground's
//! tokenizer in `src/serve/playground.html`. Here the two generated homes take
//! every keyword list from [`vocab`] — which in turn derives each set from the
//! table that already owns it (the primitive/template type tables, the property
//! ledger, `MarkerKind::NAMES`, `Side::name`). So a new type, property, builder,
//! or glyph highlights everywhere the moment it has a row, or the drift test in
//! `tests/grammar.rs` fails.
//!
//! - [`vscode_grammar`] — `editors/vscode/syntaxes/lini.tmLanguage.json`.
//! - [`zed_highlights`] — `editors/zed/languages/lini/highlights.scm`.
//! - [`splice_playground`] — the marked word-list region of `playground.html`.
//!
//! `cargo xtask gen-grammars` writes all three; the drift tests regenerate them
//! in memory and assert byte-equality with the committed files, exactly as the
//! schema does.

mod playground;
mod vocab;
mod vscode;
mod zed;

pub use playground::splice_playground;
pub use vscode::vscode_grammar;
pub use zed::zed_highlights;

/// The command that writes every generated home — named in each file's header.
const GENERATOR: &str = "cargo xtask gen-grammars";

#[cfg(test)]
mod tests {
    use super::vocab::{properties, types};
    use crate::desugar::types::TEMPLATES;
    use crate::ledger::properties::PROPERTIES;

    /// The generated built-in type list is exactly the writable primitives plus
    /// every template — a template gains highlighting the moment it has a row.
    #[test]
    fn types_cover_primitives_and_every_template() {
        let ts = types();
        for (name, _) in TEMPLATES {
            assert!(
                ts.contains(name),
                "template '{name}' missing from grammar types"
            );
        }
        assert!(ts.contains(&"box") && ts.contains(&"cyl") && ts.contains(&"sketch"));
        assert!(!ts.contains(&"text"));
    }

    /// Every ledger property is in the generated property alternation.
    #[test]
    fn properties_cover_every_ledger_row() {
        let ps = properties();
        for p in PROPERTIES {
            assert!(
                ps.contains(&p.name),
                "property '{}' missing from grammar",
                p.name
            );
        }
    }
}

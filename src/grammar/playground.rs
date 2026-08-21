//! The playground tokenizer's word lists — `src/serve/playground.html`.
//!
//! The third home of the grammar. Its tokenizer is hand-written JavaScript
//! (it must preserve every character so the highlight layer lines up with the
//! textarea), so only the **lists** are generated: one marked region holding
//! the sticky regexes, spliced in by `cargo xtask gen-grammars` from the same
//! [`super::vocab`] sets the two editor grammars read.

use super::GENERATOR;
use super::vocab::{COLOR_NAMES, builder_calls, properties, side_names, types, value_keywords};

/// The line that opens the generated region (matched by prefix, so the trailing
/// generator note may be reworded without breaking the splice).
const BEGIN: &str = "        // <lini:generated>";
/// The line that closes it.
const END: &str = "        // </lini:generated>";

/// One `const NAME = /…/y;` line, laid out the way Prettier lays the file's
/// other consts out: inline while the declaration fits the 80-column print
/// width, else the literal on its own continuation line (a regex literal is
/// unbreakable).
fn js_const(name: &str, body: &str) -> String {
    let inline = format!("        const {name} = /{body}/y;");
    if inline.chars().count() <= 80 {
        format!("{inline}\n")
    } else {
        format!("        const {name} =\n          /{body}/y;\n")
    }
}

/// A sticky regex over a word-bounded alternation of literal idents.
fn js_alt(name: &str, words: &[&str], tail: &str) -> String {
    js_const(name, &format!("(?:{}){tail}", words.join("|")))
}

/// The generated region's body: every word list the tokenizer matches on.
fn word_lists() -> String {
    // The decl head, `name:` — declining a colon glued to a side word, which is
    // a forced endpoint side (`body:left`), exactly as the editors' `prop_head`
    // does. Both the ledger names and the catch-all wear it.
    let head = format!(
        "(?=[ \\t]*:(?!:)(?!(?:{})(?![\\w-])))",
        side_names().join("|")
    );
    let mut out = format!("{BEGIN} word lists — regenerate with `{GENERATOR}`.\n");
    out.push_str(&js_alt("TYPE_BUILTIN", &types(), "(?![\\w-])"));
    out.push_str(&js_alt("PROP_KNOWN", &properties(), &head));
    out.push_str(&js_const("PROP_NAME", &format!("[A-Za-z_][\\w-]*{head}")));
    out.push_str(&js_alt("BUILDER", &builder_calls(), "(?=\\s*\\()"));
    out.push_str(&js_alt("KEYWORD", &value_keywords(), "(?![\\w-])"));
    out.push_str(&js_alt("COLOR", COLOR_NAMES, "(?![\\w-])"));
    out.push_str(&js_alt("SIDE", &side_names(), "(?![\\w-])"));
    out.push_str(END);
    out.push('\n');
    out
}

/// Replace the marked region in `src` with a freshly generated one. Idempotent:
/// splicing an already-current file returns it unchanged, which is exactly what
/// `tests/grammar.rs` asserts.
pub fn splice_playground(src: &str) -> String {
    let begin = src
        .find(BEGIN)
        .expect("playground.html must carry the `// <lini:generated>` marker");
    let end = src
        .find(END)
        .expect("playground.html must carry the `// </lini:generated>` marker");
    let end = end + src[end..].find('\n').map_or(src.len() - end, |n| n + 1);
    format!("{}{}{}", &src[..begin], word_lists(), &src[end..])
}

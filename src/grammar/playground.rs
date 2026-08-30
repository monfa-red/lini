//! The playground's generated regions — `src/serve/playground.html`.
//!
//! Two of them, both spliced by `cargo xtask gen-grammars` and both guarded
//! byte-identical by `tests/grammar.rs`:
//!
//! - the tokenizer's **word lists**, from the same [`super::vocab`] sets the
//!   two editor grammars read. Only the lists are generated — the tokenizer
//!   itself is hand-written JavaScript, because it must preserve every
//!   character so the highlight layer lines up with the textarea;
//! - the **token palette**, from [`super::highlight_css`]. The page used to
//!   carry its own copy of the nine role variables and the thirteen rules,
//!   which is the same palette a book and a site ship; now there is one, and
//!   the page wears it like every other host.

use super::GENERATOR;
use super::highlight_css;
use super::vocab::{COLOR_NAMES, builder_calls, properties, side_names, types, value_keywords};

/// The line that opens the word-list region (matched by prefix, so the trailing
/// generator note may be reworded without breaking the splice).
const BEGIN: &str = "        // <lini:generated>";
/// The line that closes it.
const END: &str = "        // </lini:generated>";

/// The same pair around the palette, inside the page's `<style>`.
const CSS_BEGIN: &str = "      /* <lini:palette>";
/// The line that closes it.
const CSS_END: &str = "      /* </lini:palette> */";

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

/// The palette region's body: [`highlight_css`], re-indented to sit inside the
/// page's `<style>` where Prettier expects it. The sheet itself is untouched —
/// what the page wears is what `lini highlight --css` prints.
fn palette() -> String {
    let mut out = format!("{CSS_BEGIN} the token palette — regenerate with `{GENERATOR}`. */\n");
    for line in highlight_css().lines() {
        if line.is_empty() {
            out.push('\n');
        } else {
            out.push_str(&format!("      {line}\n"));
        }
    }
    out.push_str(CSS_END);
    out.push('\n');
    out
}

/// Replace both marked regions in `src` with freshly generated ones.
/// Idempotent: splicing an already-current file returns it unchanged, which is
/// exactly what `tests/grammar.rs` asserts.
pub fn splice_playground(src: &str) -> String {
    let spliced = splice(src, BEGIN, END, &word_lists());
    splice(&spliced, CSS_BEGIN, CSS_END, &palette())
}

/// Replace one `begin`…`end` region (both marker lines included) with `body`.
fn splice(src: &str, begin_mark: &str, end_mark: &str, body: &str) -> String {
    let begin = src
        .find(begin_mark)
        .unwrap_or_else(|| panic!("playground.html must carry the `{begin_mark}` marker"));
    let end = src
        .find(end_mark)
        .unwrap_or_else(|| panic!("playground.html must carry the `{end_mark}` marker"));
    let end = end + src[end..].find('\n').map_or(src.len() - end, |n| n + 1);
    format!("{}{}{}", &src[..begin], body, &src[end..])
}

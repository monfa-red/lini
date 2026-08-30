//! The syntax highlighter — the grammar's fourth home [SPEC 22 / 23], and the
//! only one that is a scanner rather than a word list.
//!
//! [`highlight_html`] turns Lini source into `<span class="lini-tok-…">` markup.
//! It
//! reads the same [`super::vocab`] sets the two editor grammars and the
//! playground tokenizer do, so a new type, property, template, or glyph colours
//! here the moment it has a row.
//!
//! **One scanner, three doors** — each a thin wrapper, never a second copy: the
//! crate function for a host that links Lini (`mdbook-lini`), `lini highlight`
//! for a build step that cannot ([SPEC 20](../../SPEC.md)), and `highlight()`
//! in `crates/lini-wasm` for a page. `src/serve/playground.html` is the one
//! host none of the three reaches — no wasm, and an overlay that re-colours on
//! every keystroke — so it carries a hand-written copy, held byte-identical to
//! this file by `tests/playground.rs`.
//!
//! It preserves **every character** of the input: strip the tags and undo the
//! entity escapes and you have the source back, byte for byte. That invariant
//! is what lets a caller drop the output into a `<pre>` and trust the listing
//! to be the author's own text. It is lexical — it never parses — so a file
//! mid-keystroke still colours, and it cannot fail.

use super::vocab::{COLOR_NAMES, builder_calls, properties, side_names, types, value_keywords};

/// The class every highlighted span wears, before its token kind. It carries
/// the reserved prefix like every other name Lini writes into a host document
/// ([SPEC 18](../../SPEC.md), [SPEC 23](../../SPEC.md)), so a page's own
/// `.tok-string` can never repaint a Lini listing.
const PREFIX: &str = "lini-tok-";

/// The token classes. [`highlight_css`] paints them; the two are held together
/// by `tests::every_token_class_has_a_rule`.
///
/// `Plain` emits no span at all — most of a file is punctuation-free identifier
/// text that needs no colour, and a span per word would triple the output.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tok {
    Plain,
    Comment,
    String,
    Number,
    Const,
    Keyword,
    Type,
    TypeUser,
    Prop,
    PropUser,
    Var,
    Op,
    Class,
    Punct,
}

impl Tok {
    /// The class suffix, or `None` for text that goes out bare.
    fn class(self) -> Option<&'static str> {
        Some(match self {
            Tok::Plain => return None,
            Tok::Comment => "comment",
            Tok::String => "string",
            Tok::Number => "number",
            Tok::Const => "const",
            Tok::Keyword => "keyword",
            Tok::Type => "type",
            Tok::TypeUser => "type-user",
            Tok::Prop => "prop",
            Tok::PropUser => "prop-user",
            Tok::Var => "var",
            Tok::Op => "op",
            Tok::Class => "class",
            Tok::Punct => "punct",
        })
    }
}

/// Highlight `src` as HTML: `<span class="lini-tok-…">` runs over escaped text.
///
/// Newlines pass through as newlines — a caller that cannot carry a literal
/// newline (an HTML block inside Markdown, say) rewrites them itself.
pub fn highlight_html(src: &str) -> String {
    Scanner::new(src).run()
}

// ─────────────────────────────── the palette ───────────────────────────────

/// The nine role variables, each a `light-dark()` pair — the **one** token
/// palette. Nine roles for thirteen classes because some kinds share a colour
/// and differ only in face: a number, a constant and a keyword are all values.
///
/// `(role, light, dark)`.
const PALETTE: &[(&str, &str, &str)] = &[
    ("comment", "#8a8f98", "#6272a4"),
    ("string", "#0a7d2c", "#f1fa8c"),
    ("value", "#8250df", "#bd93f9"),
    ("type", "#1078a8", "#8be9fd"),
    ("prop", "#c2185b", "#ff79c6"),
    ("var", "#b3530b", "#ffb86c"),
    ("op", "#c2185b", "#ff79c6"),
    ("class", "#1a7f37", "#50fa7b"),
    ("punct", "#6e7781", "#7b7f9e"),
];

/// One rule each, in the order they are written: `(classes, role, face)`. The
/// `-user` pair is the same hue dimmed — a type with no ledger row and a
/// typo'd `padidng:` read as not-in-the-ledger at a glance.
const RULES: &[(&[&str], &str, &str)] = &[
    (&["comment"], "comment", "  font-style: italic;\n"),
    (&["string"], "string", ""),
    (&["number", "const", "keyword"], "value", ""),
    (&["type"], "type", "  font-style: italic;\n"),
    (&["prop"], "prop", ""),
    (
        &["type-user"],
        "type",
        "  font-style: italic;\n  opacity: 0.65;\n",
    ),
    (&["prop-user"], "prop", "  opacity: 0.65;\n"),
    (&["var"], "var", ""),
    (&["op"], "op", ""),
    (&["class"], "class", "  font-style: italic;\n"),
    (&["punct"], "punct", ""),
];

/// The stylesheet [`highlight_html`]'s output wears — what `lini highlight
/// --css` prints, what the playground splices, and what a book or a site ships
/// beside its own CSS. One palette, so a listing reads the same everywhere.
///
/// The role variables sit in `@layer lini.defaults` and the rules unlayered,
/// exactly as a compiled figure's do ([SPEC 18](../../SPEC.md)): a host
/// re-tints a role by redeclaring the variable, with no `!important`.
///
/// It sets no `color-scheme` — that is the host's, and `light-dark()` reads
/// whatever the host has set on the listing's ancestors, which is how one sheet
/// serves a book's five themes and an editor's toggle alike.
pub fn highlight_css() -> String {
    let mut out = String::new();
    out.push_str(
        "/* Lini syntax highlighting — the token palette [SPEC 18].\n   \
         Printed by `lini highlight --css`; re-tint a role by redeclaring its variable. */\n",
    );
    out.push_str("@layer lini.defaults {\n  :root {\n");
    for (role, light, dark) in PALETTE {
        out.push_str(&format!(
            "    --{PREFIX}{role}: light-dark({light}, {dark});\n"
        ));
    }
    out.push_str("  }\n}\n");
    for (classes, role, face) in RULES {
        let selector: Vec<String> = classes.iter().map(|c| format!(".{PREFIX}{c}")).collect();
        out.push_str(&format!("\n{} {{\n", selector.join(",\n")));
        out.push_str(&format!("  color: var(--{PREFIX}{role});\n"));
        out.push_str(face);
        out.push_str("}\n");
    }
    out
}

/// A word character as the surface grammar counts one: an ident may carry
/// digits, `_` and `-` after its first byte ([SPEC 22](../../SPEC.md)).
fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

/// The scanner. Two modes: **structural** — identities, selectors and links —
/// and **value**, entered by a declaration's `:` and left by the `;`, `}` or
/// `]` that ends it. The same word may colour differently in each, which is the
/// whole reason the mode is tracked rather than inferred per token.
struct Scanner<'a> {
    src: &'a str,
    b: &'a [u8],
    i: usize,
    out: String,
    in_value: bool,
    types: Vec<&'static str>,
    props: Vec<&'static str>,
    builders: Vec<&'static str>,
    keywords: Vec<&'static str>,
    sides: Vec<&'static str>,
}

impl<'a> Scanner<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            b: src.as_bytes(),
            i: 0,
            out: String::with_capacity(src.len() * 2),
            in_value: false,
            types: types(),
            props: properties(),
            builders: builder_calls(),
            keywords: value_keywords(),
            sides: side_names(),
        }
    }

    fn run(mut self) -> String {
        while self.i < self.b.len() {
            self.step();
        }
        self.out
    }

    // ───────────────────────────── output ─────────────────────────────

    /// Emit `len` bytes from the cursor as `tok`, and advance past them.
    fn emit(&mut self, tok: Tok, len: usize) {
        let text = &self.src[self.i..self.i + len];
        match tok.class() {
            Some(class) => {
                self.out.push_str("<span class=\"");
                self.out.push_str(PREFIX);
                self.out.push_str(class);
                self.out.push_str("\">");
                escape_into(&mut self.out, text);
                self.out.push_str("</span>");
            }
            None => escape_into(&mut self.out, text),
        }
        self.i += len;
    }

    /// Emit one whole character unclassified — the fallback that guarantees
    /// no byte of the source is ever dropped.
    fn emit_one(&mut self) {
        let len = self.src[self.i..].chars().next().map_or(1, char::len_utf8);
        self.emit(Tok::Plain, len);
    }

    // ──────────────────────────── scanners ────────────────────────────
    // Each returns a byte length, 0 for no match.

    fn at(&self, k: usize) -> u8 {
        self.b.get(self.i + k).copied().unwrap_or(0)
    }

    fn byte(&self, j: usize) -> u8 {
        self.b.get(j).copied().unwrap_or(0)
    }

    /// `[A-Za-z_][\w-]*` starting at `j`.
    fn ident_len(&self, j: usize) -> usize {
        if !is_ident_start(self.byte(j)) {
            return 0;
        }
        let mut k = j + 1;
        while k < self.b.len() && is_word(self.b[k]) {
            k += 1;
        }
        k - j
    }

    fn ws_len(&self) -> usize {
        let mut k = self.i;
        while matches!(self.byte(k), b' ' | b'\t' | b'\r') {
            k += 1;
        }
        k - self.i
    }

    fn comment_len(&self) -> usize {
        if self.at(0) != b'/' || self.at(1) != b'/' {
            return 0;
        }
        let mut k = self.i + 2;
        while k < self.b.len() && self.b[k] != b'\n' {
            k += 1;
        }
        k - self.i
    }

    /// A string literal, tolerating the unterminated one an editor sees
    /// mid-keystroke: it then runs to the newline or to end of input.
    fn string_len(&self) -> usize {
        if self.at(0) != b'"' {
            return 0;
        }
        let mut k = self.i + 1;
        while k < self.b.len() {
            match self.b[k] {
                b'\\' if k + 1 < self.b.len() && self.b[k + 1] != b'\n' => k += 2,
                b'"' => return k + 1 - self.i,
                b'\n' => break,
                _ => k += 1,
            }
        }
        k - self.i
    }

    /// `--name` — a palette or user variable, in either mode.
    fn css_var_len(&self) -> usize {
        if self.at(0) != b'-' || self.at(1) != b'-' {
            return 0;
        }
        let id = self.ident_len(self.i + 2);
        if id == 0 { 0 } else { 2 + id }
    }

    /// `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` — and nothing between.
    fn hex_len(&self) -> usize {
        if self.at(0) != b'#' {
            return 0;
        }
        let mut k = self.i + 1;
        while k < self.b.len() && self.b[k].is_ascii_hexdigit() {
            k += 1;
        }
        let run = k - self.i - 1;
        if matches!(run, 3 | 4 | 6 | 8) {
            1 + run
        } else {
            0
        }
    }

    fn digits_len(&self, j: usize) -> usize {
        let mut k = j;
        while self.byte(k).is_ascii_digit() {
            k += 1;
        }
        k - j
    }

    fn number_len(&self) -> usize {
        let mut k = self.i;
        if matches!(self.byte(k), b'+' | b'-') {
            k += 1;
        }
        let whole = self.digits_len(k);
        if whole > 0 {
            let frac = if self.byte(k + whole) == b'.' {
                self.digits_len(k + whole + 1)
            } else {
                0
            };
            let end = if frac > 0 {
                k + whole + 1 + frac
            } else {
                k + whole
            };
            return end - self.i;
        }
        if self.byte(k) == b'.' {
            let frac = self.digits_len(k + 1);
            if frac > 0 {
                return k + 1 + frac - self.i;
            }
        }
        0
    }

    /// A link operator: an optional marker, a line, an optional marker.
    fn link_op_len(&self) -> usize {
        let marker = |j: usize| -> usize {
            if self.byte(j) == b'<' && self.byte(j + 1) == b'>' {
                2
            } else if matches!(self.byte(j), b'<' | b'>' | b'*') {
                1
            } else {
                0
            }
        };
        let mut k = self.i + marker(self.i);
        let line = if self.byte(k) == b'-' && self.byte(k + 1) == b'-' && self.byte(k + 2) == b'-' {
            3
        } else if self.byte(k) == b'-' && self.byte(k + 1) == b'-' {
            2
        } else if matches!(self.byte(k), b'~' | b'-') {
            1
        } else {
            return 0;
        };
        k += line;
        k += marker(k);
        k - self.i
    }

    /// A worn class chain — `.hot`, `.hot.loud`.
    fn class_len(&self) -> usize {
        let mut k = self.i;
        loop {
            if self.byte(k) != b'.' {
                break;
            }
            let id = self.ident_len(k + 1);
            if id == 0 {
                break;
            }
            k += 1 + id;
        }
        k - self.i
    }

    /// Whether the previous byte is word-ish, which makes a following `.` an
    /// endpoint path (`a.port`) rather than a worn class (`a .hot`).
    fn prev_is_word(&self) -> bool {
        self.i > 0 && is_word(self.b[self.i - 1])
    }

    /// Whether the ident of `len` bytes at the cursor heads a declaration:
    /// `name:`, declining the `::` of a define and the glued side of a forced
    /// endpoint (`plate:left`), exactly as [`super::vocab::prop_head`] does.
    fn heads_a_decl(&self, len: usize) -> bool {
        let mut k = self.i + len;
        while matches!(self.byte(k), b' ' | b'\t') {
            k += 1;
        }
        if self.byte(k) != b':' || self.byte(k + 1) == b':' {
            return false;
        }
        let id = self.ident_len(k + 1);
        id == 0 || !self.sides.contains(&&self.src[k + 1..k + 1 + id])
    }

    /// Whether the ident of `len` bytes at the cursor is a builder call —
    /// a known name with a `(` after it.
    fn heads_a_call(&self, len: usize) -> bool {
        if !self.builders.contains(&&self.src[self.i..self.i + len]) {
            return false;
        }
        let mut k = self.i + len;
        while self.byte(k).is_ascii_whitespace() {
            k += 1;
        }
        self.byte(k) == b'('
    }

    fn word(&self, len: usize) -> &'a str {
        &self.src[self.i..self.i + len]
    }

    // ───────────────────────────── the walk ─────────────────────────────

    fn step(&mut self) {
        // A newline is structure: it closes an unterminated value.
        if self.at(0) == b'\n' {
            self.out.push('\n');
            self.i += 1;
            self.in_value = false;
            return;
        }
        let ws = self.ws_len();
        if ws > 0 {
            return self.emit(Tok::Plain, ws);
        }
        let comment = self.comment_len();
        if comment > 0 {
            return self.emit(Tok::Comment, comment);
        }
        let string = self.string_len();
        if string > 0 {
            return self.emit(Tok::String, string);
        }
        if self.in_value {
            self.step_value()
        } else {
            self.step_structural()
        }
    }

    fn step_value(&mut self) {
        let var = self.css_var_len();
        if var > 0 {
            return self.emit(Tok::Var, var);
        }
        let id = self.ident_len(self.i);
        if id > 0 && self.heads_a_call(id) {
            return self.emit(Tok::Type, id);
        }
        let hex = self.hex_len();
        if hex > 0 {
            return self.emit(Tok::Const, hex);
        }
        let number = self.number_len();
        if number > 0 {
            return self.emit(Tok::Number, number);
        }
        if id > 0 {
            let word = self.word(id);
            if self.keywords.contains(&word) {
                return self.emit(Tok::Keyword, id);
            }
            if COLOR_NAMES.contains(&word) {
                return self.emit(Tok::Const, id);
            }
            return self.emit(Tok::Plain, id);
        }
        match self.at(0) {
            b';' | b'}' | b']' => {
                self.in_value = false;
                self.emit(Tok::Punct, 1)
            }
            b'(' | b')' | b',' => self.emit(Tok::Punct, 1),
            _ => self.emit_one(),
        }
    }

    fn step_structural(&mut self) {
        if self.at(0) == b'|' {
            return self.step_capsule();
        }
        let var = self.css_var_len();
        if var > 0 {
            return self.emit(Tok::Var, var);
        }
        let id = self.ident_len(self.i);
        if id > 0 && self.heads_a_decl(id) {
            let tok = if self.props.contains(&self.word(id)) {
                Tok::Prop
            } else {
                Tok::PropUser
            };
            return self.emit(tok, id);
        }
        let op = self.link_op_len();
        if op > 0 {
            return self.emit(Tok::Op, op);
        }
        if !self.prev_is_word() {
            let class = self.class_len();
            if class > 0 {
                return self.emit(Tok::Class, class);
            }
            if id > 0 && self.sides.contains(&self.word(id)) {
                return self.emit(Tok::Const, id);
            }
        }
        let hex = self.hex_len();
        if hex > 0 {
            return self.emit(Tok::Const, hex);
        }
        let number = self.number_len();
        if number > 0 {
            return self.emit(Tok::Number, number);
        }
        match self.at(0) {
            b':' => {
                self.in_value = true;
                self.emit(Tok::Punct, 1)
            }
            b'&' => self.emit(Tok::Op, 1),
            b'{' | b'}' | b'[' | b']' | b';' | b',' | b'(' | b')' => self.emit(Tok::Punct, 1),
            _ if id > 0 => self.emit(Tok::Plain, id),
            _ => self.emit_one(),
        }
    }

    /// The identity between bars: `|type|`, `|type#id|`, `|#id|`, `|a::b|`.
    fn step_capsule(&mut self) {
        self.emit(Tok::Punct, 1);
        while self.i < self.b.len() && self.at(0) != b'|' && self.at(0) != b'\n' {
            let ws = self.ws_len();
            if ws > 0 {
                self.emit(Tok::Plain, ws);
                continue;
            }
            if self.at(0) == b':' && self.at(1) == b':' {
                self.emit(Tok::Op, 2);
                continue;
            }
            if self.at(0) == b'#' {
                self.emit(Tok::Punct, 1);
                let id = self.ident_len(self.i);
                if id > 0 {
                    self.emit(Tok::Type, id);
                }
                continue;
            }
            let class = self.class_len();
            if class > 0 {
                self.emit(Tok::Class, class);
                continue;
            }
            let id = self.ident_len(self.i);
            if id > 0 {
                let tok = if self.types.contains(&self.word(id)) {
                    Tok::Type
                } else {
                    Tok::TypeUser
                };
                self.emit(tok, id);
                continue;
            }
            self.emit_one();
        }
        if self.at(0) == b'|' {
            self.emit(Tok::Punct, 1);
        }
    }
}

/// Escape the four characters the playground's `esc` escapes, so both homes
/// emit byte-identical markup for the same source.
fn escape_into(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every span this module emits, undone — the inverse of the writer.
    fn plain_text(html: &str) -> String {
        let mut out = String::new();
        let mut rest = html;
        while let Some(open) = rest.find('<') {
            out.push_str(&rest[..open]);
            let close = rest[open..].find('>').expect("well-formed tag") + open;
            rest = &rest[close + 1..];
        }
        out.push_str(rest);
        out.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&amp;", "&")
    }

    /// Assert `src` highlights to exactly `want`.
    fn check(src: &str, want: &str) {
        assert_eq!(highlight_html(src), want, "source: {src:?}");
    }

    #[test]
    fn a_links_operator_is_the_only_marked_token() {
        check("a -> b", "a <span class=\"lini-tok-op\">-&gt;</span> b");
    }

    #[test]
    fn a_capsule_marks_its_bars_and_its_builtin_type() {
        check(
            "|box|",
            "<span class=\"lini-tok-punct\">|</span>\
             <span class=\"lini-tok-type\">box</span>\
             <span class=\"lini-tok-punct\">|</span>",
        );
    }

    #[test]
    fn a_type_with_no_ledger_row_reads_as_user_defined() {
        let html = highlight_html("|widget|");
        assert!(
            html.contains("<span class=\"lini-tok-type-user\">widget</span>"),
            "{html}"
        );
    }

    #[test]
    fn an_id_after_the_hash_reads_as_a_type() {
        let html = highlight_html("|box#hero|");
        assert!(
            html.contains("<span class=\"lini-tok-punct\">#</span>"),
            "{html}"
        );
        assert!(
            html.contains("<span class=\"lini-tok-type\">hero</span>"),
            "{html}"
        );
    }

    #[test]
    fn a_ledger_property_outranks_one_that_is_only_well_formed() {
        let known = highlight_html("{ fill: red; }");
        assert!(
            known.contains("<span class=\"lini-tok-prop\">fill</span>"),
            "{known}"
        );
        let typo = highlight_html("{ padidng: red; }");
        assert!(
            typo.contains("<span class=\"lini-tok-prop-user\">padidng</span>"),
            "{typo}"
        );
    }

    #[test]
    fn a_palette_variable_is_marked_in_value_position() {
        let html = highlight_html("{ fill: --teal-wash; }");
        assert!(
            html.contains("<span class=\"lini-tok-var\">--teal-wash</span>"),
            "{html}"
        );
    }

    #[test]
    fn a_declared_variable_is_marked_in_structural_position() {
        let html = highlight_html("--brand: #ff6600;");
        assert!(
            html.contains("<span class=\"lini-tok-var\">--brand</span>"),
            "{html}"
        );
        assert!(
            html.contains("<span class=\"lini-tok-const\">#ff6600</span>"),
            "{html}"
        );
    }

    #[test]
    fn a_comment_runs_to_the_end_of_its_line() {
        check(
            "// note\na",
            "<span class=\"lini-tok-comment\">// note</span>\na",
        );
    }

    /// Quotes are escaped as the playground escapes them, so the two homes
    /// produce byte-identical markup for the same source.
    #[test]
    fn a_string_is_one_token_including_its_quotes() {
        let html = highlight_html("|box| \"Hi there\"");
        assert!(
            html.contains("<span class=\"lini-tok-string\">&quot;Hi there&quot;</span>"),
            "{html}"
        );
    }

    #[test]
    fn a_layout_name_is_a_keyword_in_value_position() {
        let html = highlight_html("{ layout: sequence; }");
        assert!(
            html.contains("<span class=\"lini-tok-keyword\">sequence</span>"),
            "{html}"
        );
    }

    #[test]
    fn a_worn_class_is_marked_where_a_path_is_not() {
        let worn = highlight_html("a -> b .hot");
        assert!(
            worn.contains("<span class=\"lini-tok-class\">.hot</span>"),
            "{worn}"
        );
        let path = highlight_html("a.port -> b");
        assert!(
            !path.contains("lini-tok-class"),
            "an endpoint path is not a worn class: {path}"
        );
    }

    #[test]
    fn html_metacharacters_are_escaped() {
        let html = highlight_html("\"a<b & c>d\"");
        assert!(html.contains("a&lt;b &amp; c&gt;d"), "{html}");
        assert!(!html.contains("a<b"), "{html}");
    }

    #[test]
    fn a_blank_line_survives_as_a_blank_line() {
        let html = highlight_html("|box| \"a\"\n\n|box| \"b\"");
        assert!(html.contains("\n\n"), "{html:?}");
    }

    /// The invariant the whole module rests on: highlighting is decoration
    /// only. Strip the tags, undo the escapes, and the source comes back.
    #[test]
    fn every_character_of_the_source_survives() {
        let src = "{ layout: sequence; --brand: #ff6600; }\n\n\
                   |box#a| \"A <&> B\" .hot { fill: --teal-wash; }\n\
                   // a comment\n\
                   a -> b \"then\"\n";
        assert_eq!(plain_text(&highlight_html(src)), src);
    }

    /// The same invariant over the showroom — every construct the language has
    /// a sample for, including the ones no unit test above thought to name.
    #[test]
    fn every_sample_survives_verbatim() {
        for path in crate::testing::samples() {
            let src = crate::testing::read_sample(&path);
            assert_eq!(
                plain_text(&highlight_html(&src)),
                src,
                "highlighting altered {}",
                path.display()
            );
        }
    }

    /// A capsule spanning a newline never swallows the rest of the file: the
    /// identity scan stops at the line end, as the lexer's does.
    #[test]
    fn an_unclosed_capsule_stops_at_the_line_end() {
        let html = highlight_html("|box\na -> b");
        assert!(
            html.contains("<span class=\"lini-tok-op\">-&gt;</span>"),
            "{html}"
        );
    }

    /// Multi-byte text inside a label is carried through untouched — the
    /// scanner walks bytes, so this is the guard that it never splits one.
    #[test]
    fn multibyte_label_text_is_carried_through() {
        let src = "|box| \"café — 日本\"";
        assert_eq!(plain_text(&highlight_html(src)), src);
    }

    /// This home reads the same tables the three generated ones read, so the
    /// drift guard is direct rather than a byte-comparison: every ledger
    /// property, placed in a declaration, colours as a *known* property — never
    /// the dimmed `prop-user` an unknown key gets. A new row lights up here the
    /// moment it exists, or this fails.
    #[test]
    fn every_ledger_property_highlights_as_known() {
        for name in properties() {
            let html = highlight_html(&format!("{{ {name}: 1; }}"));
            assert!(
                html.contains(&format!("<span class=\"lini-tok-prop\">{name}</span>")),
                "property '{name}' does not colour as a ledger property: {html}"
            );
        }
    }

    /// The same over the type tables — every writable primitive and every
    /// template colours as a built-in between bars, never as user-defined.
    #[test]
    fn every_builtin_type_highlights_as_known() {
        for name in types() {
            let html = highlight_html(&format!("|{name}|"));
            assert!(
                html.contains(&format!("<span class=\"lini-tok-type\">{name}</span>")),
                "type '{name}' does not colour as a built-in: {html}"
            );
        }
    }

    /// The markup and the palette are one surface, so neither may name a class
    /// the other does not: every `Tok` that emits a span has exactly one rule,
    /// and every rule paints a class the scanner can actually emit.
    #[test]
    fn every_token_class_has_a_rule() {
        let kinds = [
            Tok::Plain,
            Tok::Comment,
            Tok::String,
            Tok::Number,
            Tok::Const,
            Tok::Keyword,
            Tok::Type,
            Tok::TypeUser,
            Tok::Prop,
            Tok::PropUser,
            Tok::Var,
            Tok::Op,
            Tok::Class,
            Tok::Punct,
        ];
        let painted: Vec<&str> = RULES
            .iter()
            .flat_map(|(cs, _, _)| cs.iter().copied())
            .collect();
        for kind in kinds {
            let Some(class) = kind.class() else { continue };
            assert_eq!(
                painted.iter().filter(|c| **c == class).count(),
                1,
                "{kind:?} emits .{PREFIX}{class}, which the stylesheet paints \
                 {} time(s)",
                painted.iter().filter(|c| **c == class).count()
            );
        }
        for class in &painted {
            assert!(
                kinds.iter().any(|k| k.class() == Some(class)),
                "the stylesheet paints .{PREFIX}{class}, which no token emits"
            );
        }
        for (_, role, _) in RULES {
            assert!(
                PALETTE.iter().any(|(r, _, _)| r == role),
                "a rule paints from --{PREFIX}{role}, which the palette does not declare"
            );
        }
    }

    /// The sheet a host ships must actually name what the markup wears — the
    /// reserved prefix included, since that is the whole point of the rename.
    #[test]
    fn the_stylesheet_paints_what_the_markup_wears() {
        let css = highlight_css();
        let html = highlight_html("|box#a| \"Hi\" { fill: red } // note\n");
        for class in ["type", "string", "prop", "comment", "punct"] {
            assert!(
                html.contains(&format!("<span class=\"{PREFIX}{class}\">")),
                "the markup does not wear .{PREFIX}{class}"
            );
            assert!(
                css.contains(&format!(".{PREFIX}{class}")),
                "the stylesheet does not paint .{PREFIX}{class}"
            );
        }
        assert!(css.contains("@layer lini.defaults"), "{css}");
        assert!(
            !css.contains("color-scheme"),
            "the palette leaves color-scheme to its host: {css}"
        );
    }

    /// And over the remaining three value-position sets, so the sweep covers
    /// every table `vocab` hands the editor grammars rather than two of them:
    /// the contextual keywords, the builder calls, and the colour names.
    #[test]
    fn every_value_word_highlights_in_its_own_class() {
        for name in value_keywords() {
            let html = highlight_html(&format!("{{ p: {name}; }}"));
            assert!(
                html.contains(&format!("<span class=\"lini-tok-keyword\">{name}</span>")),
                "value keyword '{name}' does not colour as a keyword: {html}"
            );
        }
        for name in builder_calls() {
            let html = highlight_html(&format!("{{ p: {name}(1); }}"));
            assert!(
                html.contains(&format!("<span class=\"lini-tok-type\">{name}</span>")),
                "builder call '{name}' does not colour as a call: {html}"
            );
        }
        for name in COLOR_NAMES {
            // A colour name the keyword set also claims (`none`) is a keyword
            // there and a constant here; either mark is a colour on screen, so
            // the guard is that it is marked at all.
            let html = highlight_html(&format!("{{ fill: {name}; }}"));
            assert!(
                html.contains(&format!("<span class=\"lini-tok-const\">{name}</span>"))
                    || html.contains(&format!("<span class=\"lini-tok-keyword\">{name}</span>")),
                "colour name '{name}' is not marked at all: {html}"
            );
        }
    }
}

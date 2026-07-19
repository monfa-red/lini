//! Editor-grammar generation [Stage 4, ROADMAP 3.8]. Both editor grammars — the
//! VS Code TextMate bundle and the Zed tree-sitter highlight queries — take
//! their keyword lists from the **same ledger** the resolver and validator read:
//! the primitive/template type tables, `PROPERTIES`, `BUILDER_CALLS`, and the
//! layout/role names off the owner column. So a new type or property highlights
//! the moment it has a row (or the drift test in `tests/grammar.rs` fails). One
//! generator feeds both editors — no per-grammar hand-list to drift.
//!
//! - [`vscode_grammar`] — `editors/vscode/syntaxes/lini.tmLanguage.json`.
//! - [`zed_highlights`] — `editors/zed/languages/lini/highlights.scm`.
//!
//! `cargo xtask gen-grammars` writes both; the drift test regenerates them in
//! memory and asserts byte-equality with the committed files, exactly as the
//! schema does.

use crate::desugar::types::TEMPLATES;
use crate::json::{self, J};
use crate::ledger::properties::{BUILDER_CALLS, Owner, PROPERTIES};
use crate::resolve::NodeKind;

const GENERATOR: &str = "cargo xtask gen-grammars";

// ─────────────────────────── ledger → keyword sets ───────────────────────────

/// The primitives that are written between the identity bars (`|box|`, `|sketch|`).
/// `text` — a bare `"…"` leaf, never `|text|` — is excluded; it is not a bar type.
fn primitive_types() -> Vec<&'static str> {
    NodeKind::ALL
        .iter()
        .map(|k| k.as_str())
        .filter(|k| *k != "text")
        .collect()
}

/// Every built-in type name: the writable primitives plus the templates, sorted
/// and deduped — the alternation the `|type|` bars highlight.
pub fn types() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = primitive_types();
    names.extend(TEMPLATES.iter().map(|(name, _)| *name));
    names.sort_unstable();
    names.dedup();
    names
}

/// Every ledger property name, sorted and deduped — the `key:` names that get
/// the strong property scope (an unknown `key:` still highlights, but weakly).
pub fn properties() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = PROPERTIES.iter().map(|p| p.name).collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// The value **builders** [SPEC 10.3] — calls that stay a typed value; sorted.
pub fn builder_calls() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = BUILDER_CALLS.to_vec();
    names.sort_unstable();
    names
}

/// The layout-engine names off the owner column — the `layout:` values, sorted.
pub fn layouts() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = PROPERTIES
        .iter()
        .flat_map(|p| p.owners.iter())
        .filter_map(|o| match o {
            Owner::Layout(l) => Some(*l),
            _ => None,
        })
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// The enum value idents highlighted as constants in value position. Not ledger
/// rows (they live in the readers' `parse` arms), but stated **once** here so
/// both grammars share one home — the sides [`crate::ast::Side`], link/ER marker
/// glyphs [`crate::resolve::ir::MarkerKind`], stroke styles, alignment, flow
/// direction, and the booleans/keywords.
const VALUE_KEYWORDS: &[&str] = &[
    // flow / grid / tree direction and placement
    "flow",
    "row",
    "column",
    "radial",
    "bilateral",
    "start",
    "center",
    "end",
    "stretch",
    "evenly",
    "between",
    "around",
    "rows",
    "columns",
    "all",
    // booleans and the empty/auto sentinels
    "true",
    "false",
    "none",
    "auto",
    // stroke styles
    "solid",
    "dashed",
    "dotted",
    "wavy",
    // marker glyphs and endpoint markers
    "arrow",
    "dot",
    "circle",
    "crow",
    "many",
    "datum",
    "one",
    "exactly-one",
    "zero-or-one",
    "one-or-many",
    "zero-or-many",
    // corner / outline styles
    "outlined",
    "filled",
    "rounded",
    "sharp",
    // scales and tooltip modes
    "log",
    "linear",
    "hover",
    "always",
    // sides (Side::name) — free as ids elsewhere, keywords in value position
    "top",
    "bottom",
    "left",
    "right",
    "over",
];

/// The CSS colour names understood alongside the OKLCH `--hue` palette vars.
const COLOR_NAMES: &[&str] = &[
    "white",
    "black",
    "red",
    "green",
    "blue",
    "gray",
    "grey",
    "crimson",
    "orange",
    "yellow",
    "gold",
    "silver",
    "navy",
    "teal",
    "purple",
    "pink",
    "brown",
    "cyan",
    "magenta",
    "lime",
    "maroon",
    "olive",
    "cornflowerblue",
    "currentColor",
    "transparent",
];

/// The forced-side names [`crate::ast::Side`] — one home for the `#side` rule and
/// the property rules' glued-side guard, so an endpoint `a:left` is a side, not a
/// property named `a` with value `left`.
const SIDE_NAMES: &[&str] = &["top", "bottom", "left", "right"];

/// A word-bounded alternation of literal idents: `(?<![\w-])(a|b|c)(?![\w-])`.
fn word_alt(words: &[&str]) -> String {
    format!("(?<![\\w-])({})(?![\\w-])", words.join("|"))
}

/// A property decl head, `name:` — but declining a colon glued to a side word,
/// which is a forced endpoint side (`plate:left`), handled by `#side`.
fn prop_head(name_alt: &str) -> String {
    format!(
        "{name_alt}\\s*(:)(?!:)(?!({sides})(?![\\w-]))",
        sides = SIDE_NAMES.join("|"),
    )
}

// ─────────────────────────── VS Code (TextMate) ───────────────────────────

/// The `editors/vscode/syntaxes/lini.tmLanguage.json` grammar as pretty JSON.
/// Structure is stable; the type / property / builder / value alternations are
/// generated from the ledger, so they cannot drift from the language.
pub fn vscode_grammar() -> String {
    let type_names = types();

    let root_patterns = J::Arr(
        [
            "#comment",
            "#string",
            "#var-declaration",
            "#binding",
            "#known-property",
            "#property-declaration",
            "#type-bars",
            "#css-var",
            "#link-op",
            "#class",
            "#side",
            "#hex-color",
            "#id-selector",
            "#number",
            "#punctuation",
        ]
        .iter()
        .map(|p| J::Obj(vec![("include", J::s(*p))]))
        .collect(),
    );

    let repo = J::Obj(vec![
        (
            "comment",
            J::Obj(vec![
                ("match", J::s("(//).*$")),
                ("name", J::s("comment.line.double-slash.lini")),
                (
                    "captures",
                    J::Obj(vec![(
                        "1",
                        J::Obj(vec![("name", J::s("punctuation.definition.comment.lini"))]),
                    )]),
                ),
            ]),
        ),
        (
            "string",
            J::Obj(vec![
                ("name", J::s("string.quoted.double.lini")),
                ("begin", J::s("\"")),
                (
                    "beginCaptures",
                    J::Obj(vec![(
                        "0",
                        J::Obj(vec![(
                            "name",
                            J::s("punctuation.definition.string.begin.lini"),
                        )]),
                    )]),
                ),
                ("end", J::s("\"")),
                (
                    "endCaptures",
                    J::Obj(vec![(
                        "0",
                        J::Obj(vec![(
                            "name",
                            J::s("punctuation.definition.string.end.lini"),
                        )]),
                    )]),
                ),
                (
                    "patterns",
                    J::Arr(vec![J::Obj(vec![
                        ("match", J::s("\\\\[\"\\\\nt]")),
                        ("name", J::s("constant.character.escape.lini")),
                    ])]),
                ),
            ]),
        ),
        (
            "var-declaration",
            J::Obj(vec![
                ("begin", J::s("(--[A-Za-z_][\\w-]*)\\s*(:)(?!:)")),
                (
                    "beginCaptures",
                    J::Obj(vec![
                        ("1", J::Obj(vec![("name", J::s("variable.other.lini"))])),
                        (
                            "2",
                            J::Obj(vec![("name", J::s("punctuation.separator.key-value.lini"))]),
                        ),
                    ]),
                ),
                ("end", J::s("(?=[;}])|$")),
                (
                    "patterns",
                    J::Arr(vec![J::Obj(vec![("include", J::s("#value-body"))])]),
                ),
            ]),
        ),
        (
            "known-property",
            J::Obj(vec![
                (
                    "comment",
                    J::s(
                        "A ledger property name at a decl head — the strong scope. Generated \
                         from PROPERTIES so a new row highlights on sight.",
                    ),
                ),
                ("begin", J::s(prop_head(&word_alt(&properties())))),
                (
                    "beginCaptures",
                    J::Obj(vec![
                        (
                            "1",
                            J::Obj(vec![("name", J::s("support.type.property-name.lini"))]),
                        ),
                        (
                            "2",
                            J::Obj(vec![("name", J::s("punctuation.separator.key-value.lini"))]),
                        ),
                    ]),
                ),
                ("end", J::s("(?=[;}])|$")),
                (
                    "patterns",
                    J::Arr(vec![J::Obj(vec![("include", J::s("#value-body"))])]),
                ),
            ]),
        ),
        (
            "property-declaration",
            J::Obj(vec![
                (
                    "comment",
                    J::s("Any other `key:` — an unknown / user property, weakly scoped."),
                ),
                ("begin", J::s(prop_head("([A-Za-z_][\\w-]*)"))),
                (
                    "beginCaptures",
                    J::Obj(vec![
                        (
                            "1",
                            J::Obj(vec![("name", J::s("entity.other.attribute-name.lini"))]),
                        ),
                        (
                            "2",
                            J::Obj(vec![("name", J::s("punctuation.separator.key-value.lini"))]),
                        ),
                    ]),
                ),
                ("end", J::s("(?=[;}])|$")),
                (
                    "patterns",
                    J::Arr(vec![J::Obj(vec![("include", J::s("#value-body"))])]),
                ),
            ]),
        ),
        (
            "binding",
            J::Obj(vec![
                (
                    "comment",
                    J::s(
                        "An = binding (SPEC 10.7): name = value, or name(params) = value. The \
                         right-hand side reads through #value-body.",
                    ),
                ),
                (
                    "begin",
                    J::s("([A-Za-z_][\\w-]*)\\s*(\\([A-Za-z_,\\s-]*\\))?\\s*(=)(?!=)"),
                ),
                (
                    "beginCaptures",
                    J::Obj(vec![
                        (
                            "1",
                            J::Obj(vec![("name", J::s("entity.name.function.lini"))]),
                        ),
                        ("2", J::Obj(vec![("name", J::s("variable.parameter.lini"))])),
                        (
                            "3",
                            J::Obj(vec![("name", J::s("keyword.operator.assignment.lini"))]),
                        ),
                    ]),
                ),
                ("end", J::s("(?=[;}])|$")),
                (
                    "patterns",
                    J::Arr(vec![J::Obj(vec![("include", J::s("#value-body"))])]),
                ),
            ]),
        ),
        (
            "value-body",
            J::Obj(vec![(
                "patterns",
                J::Arr(vec![
                    J::Obj(vec![("include", J::s("#comment"))]),
                    J::Obj(vec![("include", J::s("#string"))]),
                    J::Obj(vec![("include", J::s("#css-var"))]),
                    J::Obj(vec![("include", J::s("#hex-color"))]),
                    J::Obj(vec![("include", J::s("#builder-call"))]),
                    J::Obj(vec![("include", J::s("#function-call"))]),
                    J::Obj(vec![("include", J::s("#number"))]),
                    J::Obj(vec![("include", J::s("#color-name"))]),
                    J::Obj(vec![("include", J::s("#value-keyword"))]),
                    J::Obj(vec![
                        (
                            "comment",
                            J::s("Math operators inside a (…) group or a call's args (SPEC 10.7)."),
                        ),
                        ("match", J::s("\\*|/|\\^|<=|>=|==|!=|<|>|\\?")),
                        ("name", J::s("keyword.operator.arithmetic.lini")),
                    ]),
                    J::Obj(vec![
                        ("match", J::s("[(),]")),
                        ("name", J::s("punctuation.separator.value.lini")),
                    ]),
                ]),
            )]),
        ),
        (
            "type-bars",
            J::Obj(vec![
                (
                    "comment",
                    J::s(
                        "Bars hold identity (SPEC 3): a type (|box|), a type with an id \
                         (|box#cat|), an id alone (|#cat|), or a name::base define. The built-in \
                         list is generated from the primitive + template tables.",
                    ),
                ),
                ("begin", J::s("\\|")),
                (
                    "beginCaptures",
                    J::Obj(vec![(
                        "0",
                        J::Obj(vec![(
                            "name",
                            J::s("punctuation.definition.type.begin.lini entity.name.tag.lini"),
                        )]),
                    )]),
                ),
                ("end", J::s("\\|")),
                (
                    "endCaptures",
                    J::Obj(vec![(
                        "0",
                        J::Obj(vec![(
                            "name",
                            J::s("punctuation.definition.type.end.lini entity.name.tag.lini"),
                        )]),
                    )]),
                ),
                (
                    "patterns",
                    J::Arr(vec![
                        J::Obj(vec![
                            ("comment", J::s("name::base define")),
                            (
                                "match",
                                J::s("([A-Za-z_][\\w-]*)\\s*(::)\\s*([A-Za-z_][\\w-]*)"),
                            ),
                            (
                                "captures",
                                J::Obj(vec![
                                    ("1", J::Obj(vec![("name", J::s("entity.name.type.lini"))])),
                                    (
                                        "2",
                                        J::Obj(vec![(
                                            "name",
                                            J::s("keyword.operator.define.lini"),
                                        )]),
                                    ),
                                    (
                                        "3",
                                        J::Obj(vec![(
                                            "name",
                                            J::s("entity.other.inherited-class.lini"),
                                        )]),
                                    ),
                                ]),
                            ),
                        ]),
                        J::Obj(vec![
                            (
                                "comment",
                                J::s("an #id pinned in the bars (|box#cat|, |#cat|)"),
                            ),
                            ("match", J::s("(#)([A-Za-z_][\\w-]*)")),
                            (
                                "captures",
                                J::Obj(vec![
                                    (
                                        "1",
                                        J::Obj(vec![(
                                            "name",
                                            J::s("punctuation.definition.entity.lini"),
                                        )]),
                                    ),
                                    ("2", J::Obj(vec![("name", J::s("entity.name.tag.id.lini"))])),
                                ]),
                            ),
                        ]),
                        J::Obj(vec![
                            (
                                "comment",
                                J::s("a built-in primitive or template (generated)"),
                            ),
                            ("match", J::s(word_alt(&type_names))),
                            ("name", J::s("entity.name.tag.lini")),
                        ]),
                        J::Obj(vec![
                            ("comment", J::s("a user-defined type")),
                            ("match", J::s("[A-Za-z_][\\w-]*")),
                            ("name", J::s("entity.name.tag.instance.lini")),
                        ]),
                    ]),
                ),
            ]),
        ),
        (
            "css-var",
            J::Obj(vec![
                ("match", J::s("--[A-Za-z_][\\w-]*")),
                ("name", J::s("variable.other.lini")),
            ]),
        ),
        (
            "link-op",
            J::Obj(vec![
                (
                    "comment",
                    J::s(
                        "[start_marker?] line [end_marker?]; line is - / -- / --- / ~ (longest first).",
                    ),
                ),
                ("match", J::s("(?:<>|[<>*])?(?:---|--|~|-)(?:<>|[<>*])?")),
                ("name", J::s("keyword.operator.link.lini")),
            ]),
        ),
        (
            "class",
            J::Obj(vec![
                (
                    "comment",
                    J::s(
                        "A class outside the bars: a definition (.hot { }) or worn by a node / \
                         link after its type/endpoints, chained .hot.loud. A '.' after a word \
                         char is an endpoint path (a.b), not a class.",
                    ),
                ),
                ("match", J::s("(?<![\\w-])(?:\\.[A-Za-z_][\\w-]*)+")),
                ("name", J::s("entity.other.attribute-name.class.lini")),
            ]),
        ),
        (
            "side",
            J::Obj(vec![
                (
                    "comment",
                    J::s(
                        "A forced side on a link endpoint (a:left); sides are free as ids elsewhere (SPEC 18).",
                    ),
                ),
                (
                    "match",
                    J::s(format!("(:)({})(?![\\w-])", SIDE_NAMES.join("|"))),
                ),
                (
                    "captures",
                    J::Obj(vec![
                        (
                            "1",
                            J::Obj(vec![("name", J::s("punctuation.separator.side.lini"))]),
                        ),
                        (
                            "2",
                            J::Obj(vec![("name", J::s("support.constant.side.lini"))]),
                        ),
                    ]),
                ),
            ]),
        ),
        (
            "id-selector",
            J::Obj(vec![
                (
                    "comment",
                    J::s(
                        "An #id at a rule head (#hero { }); a #hex run is a colour (handled first).",
                    ),
                ),
                ("match", J::s("#[A-Za-z_][\\w-]*")),
                ("name", J::s("entity.name.tag.id.lini")),
            ]),
        ),
        (
            "hex-color",
            J::Obj(vec![
                (
                    "match",
                    J::s(
                        "#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{4}|[0-9a-fA-F]{3})(?![0-9a-fA-F])",
                    ),
                ),
                ("name", J::s("constant.other.color.lini")),
            ]),
        ),
        (
            "number",
            J::Obj(vec![
                (
                    "match",
                    J::s("(?<![\\w-])[-+]?(?:\\d+\\.\\d+|\\d+|\\.\\d+)"),
                ),
                ("name", J::s("constant.numeric.lini")),
            ]),
        ),
        (
            "builder-call",
            J::Obj(vec![
                (
                    "comment",
                    J::s(
                        "A value builder — colour / track / hatch (SPEC 10.3). Generated from BUILDER_CALLS.",
                    ),
                ),
                (
                    "match",
                    J::s(format!("{}(?=\\s*\\()", word_alt(&builder_calls()))),
                ),
                ("name", J::s("support.function.builtin.lini")),
            ]),
        ),
        (
            "function-call",
            J::Obj(vec![
                (
                    "comment",
                    J::s("Any other call in value position — a math or pen call (SPEC 10.7)."),
                ),
                ("match", J::s("(?<![\\w-])[A-Za-z_][\\w-]*(?=\\s*\\()")),
                ("name", J::s("support.function.lini")),
            ]),
        ),
        (
            "value-keyword",
            J::Obj(vec![
                (
                    "comment",
                    J::s("Enum value idents + the layout names (generated tail)."),
                ),
                ("match", J::s(word_alt(&value_keyword_words()))),
                ("name", J::s("support.constant.lini")),
            ]),
        ),
        (
            "color-name",
            J::Obj(vec![
                ("match", J::s(word_alt(COLOR_NAMES))),
                ("name", J::s("support.constant.color.lini")),
            ]),
        ),
        (
            "punctuation",
            J::Obj(vec![(
                "patterns",
                J::Arr(vec![
                    J::Obj(vec![
                        ("match", J::s("[{}]")),
                        ("name", J::s("punctuation.section.block.lini")),
                    ]),
                    J::Obj(vec![
                        ("match", J::s("[\\[\\]]")),
                        ("name", J::s("punctuation.section.children.lini")),
                    ]),
                    J::Obj(vec![
                        ("match", J::s(";")),
                        ("name", J::s("punctuation.terminator.lini")),
                    ]),
                    J::Obj(vec![
                        ("match", J::s(",")),
                        ("name", J::s("punctuation.separator.lini")),
                    ]),
                    J::Obj(vec![
                        ("match", J::s("&")),
                        ("name", J::s("keyword.operator.fanout.lini")),
                    ]),
                ]),
            )]),
        ),
    ]);

    let root = J::Obj(vec![
        (
            "$schema",
            J::s("https://raw.githubusercontent.com/martinring/tmlanguage/master/tmlanguage.json"),
        ),
        ("name", J::s("Lini")),
        (
            "comment",
            J::s(format!(
                "Generated by `{GENERATOR}` from the property ledger — do not edit."
            )),
        ),
        ("scopeName", J::s("source.lini")),
        ("patterns", root_patterns),
        ("repository", repo),
    ]);

    json::to_string(&root)
}

/// The value-keyword alternation words: the enum vocab plus the ledger layout
/// names (deduped, layouts appended so `layout: chart` highlights).
fn value_keyword_words() -> Vec<&'static str> {
    let mut words: Vec<&'static str> = VALUE_KEYWORDS.to_vec();
    for l in layouts() {
        if !words.contains(&l) {
            words.push(l);
        }
    }
    words
}

// ─────────────────────────────── Zed queries ───────────────────────────────

/// The `editors/zed/languages/lini/highlights.scm` tree-sitter highlight query.
/// The structural captures come from the `tree-sitter-lini` grammar; the ledger
/// keyword sets ride `#match?` predicates, generated here so they cannot drift.
pub fn zed_highlights() -> String {
    let mut out = String::new();
    out.push_str(&format!(
        ";; Generated by `{GENERATOR}` from the property ledger — do not edit.\n\
         ;; Keyword predicates are ledger sets (types, properties, builders,\n\
         ;; layouts); a new row appears here on regeneration or the drift test fails.\n\n",
    ));

    out.push_str(";; Comments, strings, numbers.\n");
    out.push_str("(comment) @comment\n");
    out.push_str("(string) @string\n");
    out.push_str("(escape) @string.escape\n");
    out.push_str("(number) @number\n");
    out.push_str("(hex_color) @constant\n\n");

    out.push_str(";; Identity bars: |type#id|.\n");
    out.push_str("(type_bars \"|\" @punctuation.bracket)\n");
    out.push_str("(id) @tag\n");
    out.push_str("(define_op) @operator\n\n");

    out.push_str(";; Built-in types (primitives + templates) vs user types.\n");
    out.push_str(&format!(
        "((type) @type.builtin\n  (#match? @type.builtin \"{}\"))\n",
        anchored_alt(&types()),
    ));
    out.push_str("(type) @type\n\n");

    out.push_str(";; Property names: ledger rows strongly, others weakly.\n");
    out.push_str(&format!(
        "((property) @property\n  (#match? @property \"{}\"))\n",
        anchored_alt(&properties()),
    ));
    out.push_str("(property) @variable.other.member\n\n");

    out.push_str(";; CSS variables and the theme --vars.\n");
    out.push_str("(css_var) @variable\n\n");

    out.push_str(";; Value builders vs plain calls.\n");
    out.push_str(&format!(
        "((call_name) @function.builtin\n  (#match? @function.builtin \"{}\"))\n",
        anchored_alt(&builder_calls()),
    ));
    out.push_str("(call_name) @function\n\n");

    out.push_str(";; Enum value idents + layout names.\n");
    out.push_str(&format!(
        "((value_ident) @constant.builtin\n  (#match? @constant.builtin \"{}\"))\n\n",
        anchored_alt(&value_keyword_words()),
    ));

    out.push_str(";; Colour names.\n");
    out.push_str(&format!(
        "((value_ident) @constant.builtin\n  (#match? @constant.builtin \"{}\"))\n\n",
        anchored_alt(COLOR_NAMES),
    ));

    out.push_str(";; Classes, sides, operators, punctuation.\n");
    out.push_str("(class) @attribute\n");
    out.push_str("(side) @constant.builtin\n");
    out.push_str("(link_op) @operator\n");
    out.push_str("(assign_op) @operator\n");
    out.push_str("(fanout) @operator\n");
    out.push_str("[\"{\" \"}\" \"[\" \"]\" \"(\" \")\"] @punctuation.bracket\n");
    out.push_str("[\";\" \",\"] @punctuation.delimiter\n");

    out
}

/// An anchored alternation for a tree-sitter `#match?` predicate — the whole
/// node text must be one of the words: `^(a|b|c)$`.
fn anchored_alt(words: &[&str]) -> String {
    format!("^({})$", words.join("|"))
}

#[cfg(test)]
mod tests {
    use super::*;

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

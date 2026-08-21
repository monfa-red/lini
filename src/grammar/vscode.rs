//! The VS Code TextMate bundle — `editors/vscode/syntaxes/lini.tmLanguage.json`.
//! Structure is hand-authored; every type / property / builder / value / side
//! alternation comes from [`super::vocab`], so it cannot drift from the language.

use super::GENERATOR;
use super::vocab::{
    COLOR_NAMES, builder_calls, prop_head, properties, side_names, types, value_keywords, word_alt,
};
use crate::json::{self, J};

/// The grammar as pretty JSON.
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
                        "A forced side on a link endpoint (a:left); sides are free as ids elsewhere (SPEC 23).",
                    ),
                ),
                (
                    "match",
                    J::s(format!("(:)({})(?![\\w-])", side_names().join("|"))),
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
                    J::s("Enum value idents + the marker glyphs, sides, and layout names."),
                ),
                ("match", J::s(word_alt(&value_keywords()))),
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
                "Generated by `{GENERATOR}` from the compiler's own tables — do not edit."
            )),
        ),
        ("scopeName", J::s("source.lini")),
        ("patterns", root_patterns),
        ("repository", repo),
    ]);

    json::to_string(&root)
}

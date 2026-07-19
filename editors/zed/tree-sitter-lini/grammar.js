/**
 * Tree-sitter grammar for the Lini diagram language.
 *
 * Highlighting-oriented: it tokenises the surface and gives a loose structure —
 * enough to drive `../languages/lini/highlights.scm`, not a full semantic parse
 * (Lini's own parser in the `lini` crate is the authority on meaning). Node
 * names here are the ones the highlight query captures.
 */
module.exports = grammar({
  name: "lini",

  extras: ($) => [/[ \t\r\n]/, $.comment],

  conflicts: ($) => [[$.property, $.value_ident], [$.assignment, $.value_ident]],

  rules: {
    source_file: ($) => repeat($._item),

    _item: ($) =>
      choice(
        $.type_bars,
        $.property,
        $.assignment,
        $.call,
        $.css_var,
        $.class,
        $.side,
        $.string,
        $.number,
        $.hex_color,
        $.link_op,
        $.fanout,
        $.value_ident,
        $._punct,
      ),

    comment: (_) => token(seq("//", /[^\n]*/)),

    string: ($) => seq('"', repeat(choice($.escape, /[^"\\]+/)), optional('"')),
    escape: (_) => token(seq("\\", /["\\nt]/)),

    number: (_) => token(/[-+]?(\d+\.\d+|\d+|\.\d+)/),

    hex_color: (_) =>
      token(/#([0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{4}|[0-9a-fA-F]{3})/),

    css_var: (_) => token(/--[A-Za-z_][\w-]*/),

    _ident: (_) => /[A-Za-z_][\w-]*/,

    id: (_) => token(seq("#", /[A-Za-z_][\w-]*/)),

    define_op: (_) => "::",

    // |type#id|, |type::base|, |#id|, or a bare |type|.
    type_bars: ($) =>
      seq(
        "|",
        optional(seq(alias($._ident, $.type), optional(seq($.define_op, $._ident)))),
        optional($.id),
        "|",
      ),

    // A `key:` property head — an ident immediately followed by `:`.
    property: ($) => seq(field("name", alias($._ident, $.property)), token.immediate(":")),

    assign_op: (_) => "=",
    params: (_) => token(seq("(", /[^)]*/, ")")),
    // A `name =` or `name(params) =` binding (SPEC 10.7).
    assignment: ($) =>
      seq(field("name", alias($._ident, $.value_ident)), optional($.params), $.assign_op),

    // A value builder / math call: `name(`.
    call: ($) => seq(alias($._ident, $.call_name), token.immediate("(")),

    class: (_) => token(/(\.[A-Za-z_][\w-]*)+/),

    // A forced endpoint side, `a:left` — glued to the `:`.
    side: (_) => token(seq(":", choice("top", "bottom", "left", "right"))),

    link_op: (_) => token(/(<>|[<>*])?(---|--|~|-)(<>|[<>*])?/),

    fanout: (_) => "&",

    value_ident: ($) => alias($._ident, $.value_ident),

    _punct: (_) => choice("{", "}", "[", "]", "(", ")", ";", ","),
  },
});

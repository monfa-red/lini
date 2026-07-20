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

  rules: {
    source_file: ($) => repeat($._item),

    _item: ($) =>
      choice(
        $.type_bars,
        $.property,
        $.assignment,
        $.call,
        $.group,
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

    // The close quote is required; an unterminated string is an ERROR node,
    // which is right for highlighting (strings never span lines [SPEC 2]).
    string: ($) => seq('"', repeat(choice($.escape, /[^"\\\n]+/)), '"'),
    escape: (_) => token(seq("\\", /["\\nt]/)),

    number: (_) => token(/[-+]?(\d+\.\d+|\d+|\.\d+)/),

    hex_color: (_) =>
      token(/#([0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{4}|[0-9a-fA-F]{3})/),

    css_var: (_) => token(/--[A-Za-z_][\w-]*/),

    _ident: (_) => /[A-Za-z_][\w-]*/,

    id: (_) => token(seq("#", /[A-Za-z_][\w-]*/)),

    define_op: (_) => "::",

    // |type#id|, |type::base|, |#id|, a bare |type|, or the link selector |-|
    // (SPEC 21's `|-|` sel_unit — a lone `-` between the bars).
    type_bars: ($) =>
      seq(
        "|",
        optional(
          choice(
            seq(alias($._ident, $.type), optional(seq($.define_op, $._ident))),
            "-",
          ),
        ),
        optional($.id),
        "|",
      ),

    // A `key:` property head — an ident immediately followed by `:`.
    property: ($) => seq(field("name", alias($._ident, $.property)), token.immediate(":")),

    assign_op: (_) => "=",
    // A binding's parameter list: `scale(n) = …`. Shares the parenthesised
    // expression body with `call`, so the two need a declared conflict.
    params: ($) => seq(token.immediate("("), repeat($._expr_item), ")"),
    // A `name =` or `name(params) =` binding (SPEC 10.7).
    assignment: ($) =>
      seq(field("name", alias($._ident, $.value_ident)), optional($.params), $.assign_op),

    // A value builder / math call: `name(args…)`. The `(` glues to the name
    // (SPEC 2); a free-standing `(…)` is a `group`.
    call: ($) =>
      seq(alias($._ident, $.call_name), token.immediate("("), repeat($._expr_item), ")"),

    // A free-standing math group `(…)` — folds to a number or point (SPEC 10.7),
    // and also carries the measuring ops `(-)`, `(o)`, `(<)` (SPEC 15.6): their
    // inner glyph reads as an operator / ident, the parens as brackets.
    group: ($) => seq("(", repeat($._expr_item), ")"),

    // Inside `(…)` — a call's args or a math group. Operators live here only,
    // so they never clash with links / bindings at statement position.
    _expr_item: ($) =>
      choice(
        $.number,
        $.string,
        $.hex_color,
        $.css_var,
        $.call,
        $.group,
        $.side,
        $.value_ident,
        $.math_op,
        ",",
      ),

    // Arithmetic / comparison / ternary punctuation inside an expression.
    math_op: (_) => token(/[-+*/%^<>=!?:~]+/),

    class: (_) => token(/(\.[A-Za-z_][\w-]*)+/),

    // A glued `:name` suffix — a forced endpoint side (`a:left`), a corner or
    // named anchor (`a:top-left`, `a:head`), or a pen segment name after a call
    // (`right(40):m20`, `point():a`). A generic ident is more honest for
    // highlighting than the four cardinal words (SPEC 15.2 / task note).
    side: (_) => token(seq(":", /[A-Za-z_][\w-]*/)),

    // A link / ER / draw operator, glued, no internal space (SPEC 21): optional
    // markers (`< > * <>` and ER cardinality `o +`) around a line (`- -- --- ~`)
    // or the mate op `||`.
    link_op: (_) => token(/[<>*o+]*(---|--|-|~|\|\|)[<>*o+]*/),

    fanout: (_) => "&",

    value_ident: ($) => alias($._ident, $.value_ident),

    _punct: (_) => choice("{", "}", "[", "]", ";", ","),
  },
});

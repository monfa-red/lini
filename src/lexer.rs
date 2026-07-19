use crate::ast::{DrawOp, LineStyle, LinkMarker, LinkOp};
use crate::error::{Code, Error};
use crate::span::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum TokKind {
    Ident(String),
    String(String),
    Number(f64),
    Percent(f64), // a number with a '%' suffix (color components, [SPEC 2])
    /// `#` + the raw run of ident chars after it, undecided: the parser reads it
    /// as a colour in a value (`#f80`, validated as hex) or an id in bars / at a
    /// rule head (`#cat`, validated as an ident). A context-free lexer can't tell
    /// the two apart, so it emits one raw token [SPEC 2].
    Hash(String),
    RawCssVar(String), // CSS var name without leading '--'

    Pipe,   // |
    Colon,  // : (attr binding / ternary)
    DColon, // :: (define operator)
    Dot,    // . (style ref or endpoint side)
    Amp,    // &
    Semi,   // ;
    Comma,  // ,
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,

    // Math operators [SPEC 10.7] — lexed only inside a value's parens (an
    // expression context, `paren_depth > 0`), except `=` / `==` / `!=`, which also
    // bind names and compare. They exist so the parser can spot an expression and
    // slice its raw source for [`crate::expr`], which re-lexes and folds it.
    Assign,   // =
    Plus,     // +
    Minus,    // -
    Star,     // *
    Slash,    // /
    Caret,    // ^
    Lt,       // <
    Le,       // <=
    Gt,       // >
    Ge,       // >=
    EqEq,     // ==
    Ne,       // !=
    Question, // ?

    LinkOp(LinkOp),
    /// A drawing measuring op [SPEC 15.6] — `(-)` / `(o)` / `(<)`, lexed as one
    /// glued token only where the `(` is free-standing (the call-glue rule, [SPEC 2]).
    DrawOp(DrawOp),

    Newline,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokKind,
    pub span: Span,
}

pub fn lex(src: &str) -> Result<Vec<Token>, Error> {
    Lexer::new(src, false).into_tokens()
}

/// Lex an isolated `(…)` expression body [SPEC 10.7] — the single tokenizer for
/// [`crate::expr`], reached after the main parser has sliced a group or an
/// operator-bearing argument. Expression mode is the "semantic split" the whole
/// source can't carry in one pass: `-`/`+` are always operators (never a signed
/// number, so `r-1` is subtraction), `-` is not an ident char, scientific
/// notation is a number, and newlines are whitespace. Outside a math group the
/// main pass keeps the opposite rules (`-45` a signed number, so `hatch(45 -45)`
/// is two angles), so the two coexist only by lexing the expression region on its
/// own here.
pub fn lex_expr(src: &str) -> Result<Vec<Token>, Error> {
    Lexer::new(src, true).into_tokens()
}

struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    i: usize,
    paren_depth: usize,
    /// Lexing an isolated expression body (`lex_expr`) rather than a whole source.
    expr_mode: bool,
    tokens: Vec<Token>,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str, expr_mode: bool) -> Self {
        Lexer {
            src,
            bytes: src.as_bytes(),
            i: 0,
            paren_depth: 0,
            expr_mode,
            tokens: Vec::new(),
        }
    }

    fn into_tokens(mut self) -> Result<Vec<Token>, Error> {
        self.run()?;
        Ok(self.tokens)
    }

    /// A math-operator context [SPEC 10.7]: inside a value's parens, or lexing a
    /// bare expression body. Where `-`/`+`/`*`/`/`/`^`/comparisons are operators.
    fn in_math(&self) -> bool {
        self.paren_depth > 0 || self.expr_mode
    }

    fn run(&mut self) -> Result<(), Error> {
        while self.i < self.bytes.len() {
            let c = self.bytes[self.i];

            match c {
                b' ' | b'\t' | b'\r' => self.i += 1,
                b'\n' => self.handle_newline(),
                b'/' if self.peek(1) == Some(b'/') => self.skip_line_comment(),
                b'{' => self.push_punct(TokKind::LBrace, 1),
                b'}' => self.push_punct(TokKind::RBrace, 1),
                b'(' => {
                    // The call-glue rule [SPEC 2]: a '(' glued to an ident char
                    // opens a call; free-standing, an exact `(-)` / `(o)` / `(<)` is
                    // a measuring op [SPEC 15.6] and `(>)` is reserved. The
                    // three-char match keeps `pin (-90)`, `foo(o)`, and every call
                    // intact.
                    let glued_call = self.i > 0 && is_ident_continue(self.bytes[self.i - 1]);
                    let rest = &self.bytes[self.i..];
                    if self.expr_mode {
                        // In a bare expression `(` only ever opens a sub-group.
                        self.paren_depth += 1;
                        self.push_punct(TokKind::LParen, 1);
                    } else if !glued_call && rest.starts_with(b"(-)") {
                        self.push_punct(TokKind::DrawOp(DrawOp::Linear), 3);
                    } else if !glued_call && rest.starts_with(b"(o)") {
                        self.push_punct(TokKind::DrawOp(DrawOp::Round), 3);
                    } else if !glued_call && rest.starts_with(b"(<)") {
                        self.push_punct(TokKind::DrawOp(DrawOp::Angle), 3);
                    } else if !glued_call && rest.starts_with(b"(>)") {
                        return Err(Error::at(
                            Span::new(self.i, self.i + 3),
                            "'(>)' is reserved — the angle op is '(<)'",
                        ));
                    } else {
                        self.paren_depth += 1;
                        self.push_punct(TokKind::LParen, 1);
                    }
                }
                b')' => {
                    self.paren_depth = self.paren_depth.saturating_sub(1);
                    self.push_punct(TokKind::RParen, 1);
                }
                // Child lists keep newlines significant (children are newline /
                // `;` separated, like a `{ }` block) — so brackets, unlike parens,
                // do not suppress them.
                b'[' => self.push_punct(TokKind::LBracket, 1),
                b']' => self.push_punct(TokKind::RBracket, 1),
                b'|' => self.push_punct(TokKind::Pipe, 1),
                b':' if self.peek(1) == Some(b':') => self.push_punct(TokKind::DColon, 2),
                b':' => self.push_punct(TokKind::Colon, 1),
                b';' => self.push_punct(TokKind::Semi, 1),
                b',' => self.push_punct(TokKind::Comma, 1),
                b'&' => self.push_punct(TokKind::Amp, 1),
                // `=` binds a name (`name = value`) and, in a group, a local; `==`
                // and `!=` compare inside an expression [SPEC 10.7].
                b'=' if self.peek(1) == Some(b'=') => self.push_punct(TokKind::EqEq, 2),
                b'=' => self.push_punct(TokKind::Assign, 1),
                b'!' if self.peek(1) == Some(b'=') => self.push_punct(TokKind::Ne, 2),
                b'"' => self.lex_string()?,
                // Single quotes are reserved, not strings [SPEC 2/21].
                b'\'' => {
                    return Err(Error::at(
                        Span::new(self.i, self.i + 1),
                        "single quotes are not strings — use \"…\"",
                    ));
                }
                b'#' => self.lex_hash()?,
                b'.' => {
                    if self.glued_copy_index() {
                        // Endpoint position [SPEC 15.4/21]: a `.` glued to an
                        // ident and followed by digits is a path dot + a copy
                        // index (`plate.bolt.2`), so `1.5` in value position
                        // stays a number (its dot glues to a digit, not an
                        // ident) and `opacity: .5` stays a fraction.
                        self.push_punct(TokKind::Dot, 1);
                        self.lex_copy_index();
                    } else if self.peek(1).is_some_and(|c| c.is_ascii_digit()) {
                        self.lex_number()?;
                    } else {
                        // `.` is a path / class / side separator; `..` is two of
                        // them, no longer a link line.
                        self.push_punct(TokKind::Dot, 1);
                    }
                }
                // CSS var override `--name…` (defs line start or attr value). An
                // expression has no vars, so there `--` is two operators.
                b'-' if !self.expr_mode
                    && self.peek(1) == Some(b'-')
                    && self.peek(2).is_some_and(is_ident_start) =>
                {
                    self.lex_raw_css_var()?;
                }
                // Signed number: `-5`, `-.5`, `+5`. In an expression a leading `-`/`+`
                // is a unary operator instead (so `r-1` is subtraction), handled below.
                b'-' | b'+'
                    if !self.expr_mode
                        && (self.peek(1).is_some_and(|c| c.is_ascii_digit())
                            || (self.peek(1) == Some(b'.')
                                && self.peek(2).is_some_and(|c| c.is_ascii_digit()))) =>
                {
                    self.lex_number()?;
                }
                // Inside a value's parens (or an expression) these are math operators
                // [SPEC 10.7]; outside, `< > + -` stay link / marker syntax. Signed
                // numbers were caught above, so a `-` / `+` here is always the operator.
                b'-' if self.in_math() => self.push_punct(TokKind::Minus, 1),
                b'+' if self.in_math() => self.push_punct(TokKind::Plus, 1),
                b'*' if self.in_math() => self.push_punct(TokKind::Star, 1),
                b'/' if self.in_math() => self.push_punct(TokKind::Slash, 1),
                b'^' if self.in_math() => self.push_punct(TokKind::Caret, 1),
                b'?' if self.in_math() => self.push_punct(TokKind::Question, 1),
                b'<' if self.in_math() && self.peek(1) == Some(b'=') => {
                    self.push_punct(TokKind::Le, 2)
                }
                b'<' if self.in_math() => self.push_punct(TokKind::Lt, 1),
                b'>' if self.in_math() && self.peek(1) == Some(b'=') => {
                    self.push_punct(TokKind::Ge, 2)
                }
                b'>' if self.in_math() => self.push_punct(TokKind::Gt, 1),
                // Link-op starts: any of these characters can begin a link op.
                b'-' | b'~' | b'<' | b'>' | b'+' => self.lex_link_op()?,
                // `*` is a link-op start marker only when followed by a line char.
                b'*' if self.peek(1).is_some_and(is_link_line_start) => self.lex_link_op()?,
                // A bare operator outside parens: math must sit in a group [SPEC 10.7].
                b'*' | b'/' | b'^' => {
                    return Err(Error::at(
                        Span::new(self.i, self.i + 1),
                        "math operators appear inside ( ) — e.g. padding: (8 * 2)",
                    ));
                }
                d if d.is_ascii_digit() => self.lex_number()?,
                c if is_ident_start(c) => self.lex_ident(),
                _ => {
                    return Err(Error::at(
                        Span::new(self.i, self.i + 1),
                        format!("unexpected character {:?}", c as char),
                    )
                    .code(Code::UNEXPECTED_CHAR));
                }
            }
        }
        Ok(())
    }

    fn peek(&self, n: usize) -> Option<u8> {
        self.bytes.get(self.i + n).copied()
    }

    fn push_punct(&mut self, kind: TokKind, len: usize) {
        let span = Span::new(self.i, self.i + len);
        self.tokens.push(Token { kind, span });
        self.i += len;
    }

    fn handle_newline(&mut self) {
        let start = self.i;
        self.i += 1;
        while self.i < self.bytes.len() {
            let c = self.bytes[self.i];
            if c == b' ' || c == b'\t' || c == b'\r' || c == b'\n' {
                self.i += 1;
            } else {
                break;
            }
        }
        if self.paren_depth == 0 && !self.expr_mode {
            self.tokens.push(Token {
                kind: TokKind::Newline,
                span: Span::new(start, start + 1),
            });
        }
    }

    fn skip_line_comment(&mut self) {
        while self.i < self.bytes.len() && self.bytes[self.i] != b'\n' {
            self.i += 1;
        }
    }

    fn lex_string(&mut self) -> Result<(), Error> {
        let start = self.i;
        self.i += 1; // opening quote
        let mut value = String::new();

        while self.i < self.bytes.len() {
            let b = self.bytes[self.i];
            if b == b'"' {
                self.i += 1;
                // Leading / trailing whitespace is trimmed from every string
                // value (inner spacing kept) so source spacing never leaks into
                // the render — `" ABC "` is "ABC" [SPEC 2]. The span still covers
                // the quotes for errors.
                self.tokens.push(Token {
                    kind: TokKind::String(value.trim().to_string()),
                    span: Span::new(start, self.i),
                });
                return Ok(());
            }
            if b == b'\\' {
                let esc_start = self.i;
                self.i += 1;
                let next = self.bytes.get(self.i).copied().ok_or_else(|| {
                    Error::at(Span::new(esc_start, self.i), "unterminated escape sequence")
                        .code(Code::BAD_ESCAPE)
                })?;
                match next {
                    b'"' => value.push('"'),
                    b'\\' => value.push('\\'),
                    b'n' => value.push('\n'),
                    b't' => value.push('\t'),
                    other => {
                        return Err(Error::at(
                            Span::new(esc_start, self.i + 1),
                            format!("invalid escape sequence '\\{}'", other as char),
                        )
                        .code(Code::BAD_ESCAPE));
                    }
                }
                self.i += 1;
                continue;
            }
            let ch = self.src[self.i..].chars().next().expect("non-empty utf-8");
            value.push(ch);
            self.i += ch.len_utf8();
        }

        Err(
            Error::at(Span::new(start, self.i), "unterminated string literal")
                .code(Code::UNTERMINATED_STRING),
        )
    }

    fn lex_hash(&mut self) -> Result<(), Error> {
        let start = self.i;
        self.i += 1; // '#'
        let run_start = self.i;
        while self.i < self.bytes.len() && is_ident_continue(self.bytes[self.i]) {
            self.i += 1;
        }
        // Raw, undecided: the parser validates the run as hex digits (a colour in
        // a value) or an ident (an id in bars / at a rule head) by context.
        let run = self.src[run_start..self.i].to_string();
        self.tokens.push(Token {
            kind: TokKind::Hash(run),
            span: Span::new(start, self.i),
        });
        Ok(())
    }

    fn lex_raw_css_var(&mut self) -> Result<(), Error> {
        let start = self.i;
        self.i += 2; // skip '--'
        let name_start = self.i;
        while self.i < self.bytes.len() && is_ident_continue(self.bytes[self.i]) {
            self.i += 1;
        }
        let name = self.src[name_start..self.i].to_string();
        self.tokens.push(Token {
            kind: TokKind::RawCssVar(name),
            span: Span::new(start, self.i),
        });
        Ok(())
    }

    fn lex_number(&mut self) -> Result<(), Error> {
        let start = self.i;

        if matches!(self.bytes[self.i], b'+' | b'-') {
            self.i += 1;
        }

        let mut saw_digit = false;
        while self.i < self.bytes.len() && self.bytes[self.i].is_ascii_digit() {
            self.i += 1;
            saw_digit = true;
        }
        if self.i < self.bytes.len()
            && self.bytes[self.i] == b'.'
            && self.peek(1).is_some_and(|c| c.is_ascii_digit())
        {
            self.i += 1; // '.'
            while self.i < self.bytes.len() && self.bytes[self.i].is_ascii_digit() {
                self.i += 1;
                saw_digit = true;
            }
        }
        // Scientific notation is expression-only [SPEC 10.7]: `1e6`, `1.5e-2`. Back
        // off if no exponent digits follow (so a stray `e` stays an ident).
        if self.expr_mode && self.i < self.bytes.len() && matches!(self.bytes[self.i], b'e' | b'E')
        {
            let save = self.i;
            self.i += 1;
            if matches!(self.peek(0), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if self.peek(0).is_some_and(|c| c.is_ascii_digit()) {
                while self.i < self.bytes.len() && self.bytes[self.i].is_ascii_digit() {
                    self.i += 1;
                }
            } else {
                self.i = save;
            }
        }

        if !saw_digit {
            return Err(
                Error::at(Span::new(start, self.i), "invalid number literal")
                    .code(Code::BAD_NUMBER),
            );
        }

        let text = &self.src[start..self.i];
        let value: f64 = text.parse().map_err(|_| {
            Error::at(
                Span::new(start, self.i),
                format!("invalid number literal '{}'", text),
            )
            .code(Code::BAD_NUMBER)
        })?;
        // A trailing `%` makes it a percentage (color components, [SPEC 2]).
        let kind = if self.i < self.bytes.len() && self.bytes[self.i] == b'%' {
            self.i += 1;
            TokKind::Percent(value)
        } else {
            TokKind::Number(value)
        };
        self.tokens.push(Token {
            kind,
            span: Span::new(start, self.i),
        });
        Ok(())
    }

    /// Whether the `.` at the cursor opens a pattern-copy index [SPEC 15.4]:
    /// glued to a just-lexed ident (an endpoint path) and followed by a digit,
    /// outside any math context. Everywhere else `.`+digit stays a number.
    fn glued_copy_index(&self) -> bool {
        !self.in_math()
            && self.peek(1).is_some_and(|c| c.is_ascii_digit())
            && self
                .tokens
                .last()
                .is_some_and(|t| matches!(t.kind, TokKind::Ident(_)) && t.span.end == self.i)
    }

    /// The copy index's digit run — a bare integer (never a decimal: a second
    /// glued `.` is another path dot, and the index is always last).
    fn lex_copy_index(&mut self) {
        let start = self.i;
        while self.i < self.bytes.len() && self.bytes[self.i].is_ascii_digit() {
            self.i += 1;
        }
        let value: f64 = self.src[start..self.i].parse().expect("a digit run");
        self.tokens.push(Token {
            kind: TokKind::Number(value),
            span: Span::new(start, self.i),
        });
    }

    fn lex_ident(&mut self) {
        let start = self.i;
        // In an expression `-` is subtraction, not part of a name, so `r-1` splits.
        while self.i < self.bytes.len()
            && is_ident_continue(self.bytes[self.i])
            && !(self.expr_mode && self.bytes[self.i] == b'-')
        {
            self.i += 1;
        }
        let name = self.src[start..self.i].to_string();
        self.tokens.push(Token {
            kind: TokKind::Ident(name),
            span: Span::new(start, self.i),
        });
    }

    // ─────────────────────── Link ops ───────────────────────
    //
    // A link op is `[start_marker?][line][end_marker?]`, all glued together
    // with no whitespace. Start markers (`<`,`>`,`*`,`<>`) translate to
    // different `LinkMarker` kinds than end markers — `<` at start is Arrow,
    // `<` at end is Crow, and vice versa. Position discriminates.
    //
    // The line component is required; if no line is consumed, lexing fails.

    fn lex_link_op(&mut self) -> Result<(), Error> {
        let start_i = self.i;
        let mut p = self.i;

        let start = self.consume_marker(&mut p, MarkerSide::Start)?;
        let line = match self.consume_line(&mut p) {
            Some(l) => l,
            None => {
                return Err(Error::at(
                    Span::new(start_i, p.max(start_i + 1)),
                    format!(
                        "expected link-op line after '{}'",
                        &self.src[start_i..p.max(start_i + 1)]
                    ),
                ));
            }
        };
        let end = self.consume_marker(&mut p, MarkerSide::End)?;

        // Reject the no-op case: a lone `--` followed by ident-start should have
        // been caught earlier as a RawCssVar. But a stray `--name` reached here
        // is an error (unreachable in practice).
        let op = LinkOp { line, start, end };
        let span = Span::new(start_i, p);
        self.tokens.push(Token {
            kind: TokKind::LinkOp(op),
            span,
        });
        self.i = p;
        Ok(())
    }

    fn consume_marker(&self, p: &mut usize, side: MarkerSide) -> Result<LinkMarker, Error> {
        // Try `<>` first (longest match).
        if self.bytes.get(*p) == Some(&b'<') && self.bytes.get(*p + 1) == Some(&b'>') {
            *p += 2;
            return Ok(LinkMarker::Diamond);
        }
        let c = match self.bytes.get(*p) {
            Some(&c) => c,
            None => return Ok(LinkMarker::None),
        };
        let next = self.bytes.get(*p + 1).copied();
        // The ER cardinality marker [SPEC 9] composes `[min][max]` on **either**
        // side: the optionality ring `o` (min = zero) or a bar `+` (min = one) hugs
        // the line, the max glyph — a bar `+` (one) or the crow (`<` at the end, `>`
        // at the start, = many) — sits outermost. A lone `+` is "one"; a bare `o`
        // (no max) is an error with a hint. The two sides mirror.
        match side {
            MarkerSide::End if c == b'+' || c == b'o' => {
                let hit = match (c, next) {
                    (b'+', Some(b'+')) => Some((LinkMarker::ExactlyOne, 2)),
                    (b'+', Some(b'<')) => Some((LinkMarker::OneOrMany, 2)),
                    (b'+', _) => Some((LinkMarker::One, 1)),
                    (b'o', Some(b'+')) => Some((LinkMarker::ZeroOrOne, 2)),
                    (b'o', Some(b'<')) => Some((LinkMarker::ZeroOrMany, 2)),
                    _ => None, // a bare `o`
                };
                return self.finish_cardinality(p, hit);
            }
            MarkerSide::Start if c == b'+' || c == b'>' => {
                let hit = match (c, next) {
                    (b'+', Some(b'+')) => Some((LinkMarker::ExactlyOne, 2)),
                    (b'+', Some(b'o')) => Some((LinkMarker::ZeroOrOne, 2)),
                    (b'+', _) => Some((LinkMarker::One, 1)),
                    (b'>', Some(b'+')) => Some((LinkMarker::OneOrMany, 2)),
                    (b'>', Some(b'o')) => Some((LinkMarker::ZeroOrMany, 2)),
                    (b'>', _) => Some((LinkMarker::Crow, 1)), // a bare crow-start
                    _ => None,                                // guard admits only `+` / `>`
                };
                return self.finish_cardinality(p, hit);
            }
            _ => {}
        }
        let marker = match (c, side) {
            (b'<', MarkerSide::Start) => LinkMarker::Arrow,
            (b'<', MarkerSide::End) => LinkMarker::Crow,
            (b'>', MarkerSide::End) => LinkMarker::Arrow,
            (b'*', _) => LinkMarker::Dot,
            _ => return Ok(LinkMarker::None),
        };
        *p += 1;
        Ok(marker)
    }

    /// Land a composed cardinality marker (advance `p`), or the bare-`o` error.
    fn finish_cardinality(
        &self,
        p: &mut usize,
        hit: Option<(LinkMarker, usize)>,
    ) -> Result<LinkMarker, Error> {
        match hit {
            Some((m, n)) => {
                *p += n;
                Ok(m)
            }
            None => Err(Error::at(
                Span::new(p.saturating_sub(1), *p + 1),
                "'-o' needs a max glyph — write '-o<', '-o+', or 'marker-end: circle'",
            )),
        }
    }

    fn consume_line(&self, p: &mut usize) -> Option<LineStyle> {
        let rest = self.bytes.get(*p..)?;
        // The line grows more broken as it lengthens: `-` solid, `--` dashed,
        // `---` dotted, `~` wavy. Longest match first; `..` is no longer a line.
        if rest.starts_with(b"---") {
            *p += 3;
            return Some(LineStyle::Dotted);
        }
        if rest.starts_with(b"--") {
            // Disambiguate from `--name` (CSS var) — should have been caught
            // before reaching here, but guard anyway: if `--` is followed by
            // an ident-start, do not consume it as a line.
            if rest.get(2).is_some_and(|&c| is_ident_start(c)) {
                return None;
            }
            *p += 2;
            return Some(LineStyle::Dashed);
        }
        match rest.first()? {
            b'-' => {
                *p += 1;
                Some(LineStyle::Solid)
            }
            b'~' => {
                *p += 1;
                Some(LineStyle::Wavy)
            }
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
enum MarkerSide {
    Start,
    End,
}

fn is_link_line_start(c: u8) -> bool {
    matches!(c, b'-' | b'~')
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_ident_continue(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'-'
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{LineStyle, LinkMarker};

    fn kinds(src: &str) -> Vec<TokKind> {
        lex(src)
            .expect("lex ok")
            .into_iter()
            .map(|t| t.kind)
            .filter(|k| !matches!(k, TokKind::Newline))
            .collect()
    }

    fn line_of(src: &str) -> LineStyle {
        match &kinds(src)[..] {
            [TokKind::LinkOp(op)] => op.line,
            other => panic!("expected one link op, got {other:?}"),
        }
    }

    #[test]
    fn hash_lexes_raw_run_for_colours_and_ids() {
        assert_eq!(kinds("#fff"), vec![TokKind::Hash("fff".into())]);
        assert_eq!(kinds("#ffaa00cc"), vec![TokKind::Hash("ffaa00cc".into())]);
        assert_eq!(kinds("#cat"), vec![TokKind::Hash("cat".into())]);
        assert_eq!(
            kinds("#load-balancer"),
            vec![TokKind::Hash("load-balancer".into())]
        );
    }

    #[test]
    fn link_lines_grow_more_broken() {
        assert_eq!(line_of("->"), LineStyle::Solid);
        assert_eq!(line_of("-->"), LineStyle::Dashed);
        assert_eq!(line_of("--->"), LineStyle::Dotted);
        assert_eq!(line_of("~>"), LineStyle::Wavy);
    }

    #[test]
    fn arrow_marker_sits_at_the_end() {
        let TokKind::LinkOp(op) = &kinds("->")[0] else {
            panic!("expected a link op");
        };
        assert_eq!(op.start, LinkMarker::None);
        assert_eq!(op.end, LinkMarker::Arrow);
    }

    #[test]
    fn endpoint_side_is_ident_colon_ident() {
        assert_eq!(
            kinds("a:left"),
            vec![
                TokKind::Ident("a".into()),
                TokKind::Colon,
                TokKind::Ident("left".into()),
            ]
        );
    }

    #[test]
    fn css_var_beats_the_dashed_line() {
        assert_eq!(kinds("--brand"), vec![TokKind::RawCssVar("brand".into())]);
    }

    #[test]
    fn double_dot_is_two_dots_not_a_line() {
        assert_eq!(kinds(".."), vec![TokKind::Dot, TokKind::Dot]);
    }

    #[test]
    fn string_values_are_trimmed() {
        assert_eq!(kinds("\" ABC \""), vec![TokKind::String("ABC".into())]);
        assert_eq!(kinds("\"a b\""), vec![TokKind::String("a b".into())]);
    }

    #[test]
    fn measuring_ops_lex_free_standing_only() {
        use crate::ast::DrawOp;
        // Free-standing exact matches are the ops [SPEC 15.6]…
        assert_eq!(kinds("(-)"), vec![TokKind::DrawOp(DrawOp::Linear)]);
        assert_eq!(kinds("(o)"), vec![TokKind::DrawOp(DrawOp::Round)]);
        assert_eq!(kinds("(<)"), vec![TokKind::DrawOp(DrawOp::Angle)]);
        // …`(o)` glued to an ident is still a call, so `foo(o)` is untouched…
        assert_eq!(
            kinds("foo(o)"),
            vec![
                TokKind::Ident("foo".into()),
                TokKind::LParen,
                TokKind::Ident("o".into()),
                TokKind::RParen,
            ]
        );
        // …a '(' glued to an ident opens a call — `move(-90, 0)` is untouched
        // (the call-glue rule, [SPEC 2])…
        assert_eq!(
            kinds("move(-90, 0)"),
            vec![
                TokKind::Ident("move".into()),
                TokKind::LParen,
                TokKind::Number(-90.0),
                TokKind::Comma,
                TokKind::Number(0.0),
                TokKind::RParen,
            ]
        );
        // …and a free-standing non-exact `(` stays a plain paren.
        assert_eq!(kinds("(-90)")[0], TokKind::LParen);
    }

    #[test]
    fn operators_lex_inside_parens_and_links_outside() {
        // Outside parens, `-` / `<` / `>` stay link / marker syntax [SPEC 10.7].
        assert!(matches!(kinds("a -> b")[1], TokKind::LinkOp(_)));
        // Inside a value's parens, they are math operators.
        assert_eq!(
            kinds("(8 * 2)"),
            vec![
                TokKind::LParen,
                TokKind::Number(8.0),
                TokKind::Star,
                TokKind::Number(2.0),
                TokKind::RParen,
            ]
        );
        // A spaced `-` inside parens is subtraction; a glued `-2` stays a number.
        assert_eq!(
            kinds("(a - b, -2)"),
            vec![
                TokKind::LParen,
                TokKind::Ident("a".into()),
                TokKind::Minus,
                TokKind::Ident("b".into()),
                TokKind::Comma,
                TokKind::Number(-2.0),
                TokKind::RParen,
            ]
        );
        // `=` binds a name at any depth; `<=` / `==` compare inside parens.
        assert_eq!(kinds("x = 5")[1], TokKind::Assign);
        assert_eq!(kinds("(a <= b)")[2], TokKind::Le);
        // A bare operator outside parens asks for a group.
        let err = lex("padding: 8 ^ 2").expect_err("bare ^ errors");
        assert!(err.message.contains("inside ( )"), "{err:?}");
    }

    #[test]
    fn a_glued_dot_digit_run_is_a_copy_index_only_after_an_ident() {
        // Endpoint position [SPEC 15.4/21]: `plate.bolt.2` is a path with a
        // numeric copy segment…
        assert_eq!(
            kinds("plate.bolt.2"),
            vec![
                TokKind::Ident("plate".into()),
                TokKind::Dot,
                TokKind::Ident("bolt".into()),
                TokKind::Dot,
                TokKind::Number(2.0),
            ]
        );
        // …with a `:point` still free to follow…
        assert_eq!(
            kinds("bolt.2:top"),
            vec![
                TokKind::Ident("bolt".into()),
                TokKind::Dot,
                TokKind::Number(2.0),
                TokKind::Colon,
                TokKind::Ident("top".into()),
            ]
        );
        // …while `.`+digit anywhere else keeps its number reading.
        assert_eq!(kinds("1.5"), vec![TokKind::Number(1.5)]);
        assert_eq!(kinds(".5"), vec![TokKind::Number(0.5)]);
        assert_eq!(
            kinds("a .5"),
            vec![TokKind::Ident("a".into()), TokKind::Number(0.5)]
        );
    }

    #[test]
    fn reversed_angle_op_is_reserved() {
        let err = lex("a (>) b").expect_err("(>) is reserved");
        assert!(err.message.contains("the angle op is '(<)'"), "{err:?}");
    }

    #[test]
    fn cardinality_markers_compose_min_max_on_both_sides() {
        let op_of = |src: &str| match &kinds(src)[..] {
            [TokKind::Ident(_), TokKind::LinkOp(op), TokKind::Ident(_)] => *op,
            other => panic!("expected `a OP b`, got {other:?}"),
        };
        // End side [min][max]: the ring/bar hugs the line, the max glyph is outer.
        assert_eq!(op_of("a -+ b").end, LinkMarker::One);
        assert_eq!(op_of("a -< b").end, LinkMarker::Crow);
        assert_eq!(op_of("a -++ b").end, LinkMarker::ExactlyOne);
        assert_eq!(op_of("a -o+ b").end, LinkMarker::ZeroOrOne);
        assert_eq!(op_of("a -+< b").end, LinkMarker::OneOrMany);
        assert_eq!(op_of("a -o< b").end, LinkMarker::ZeroOrMany);
        // Start side mirrors — `>` is the crow, the ring/bar still hugs the line.
        assert_eq!(op_of("a +- b").start, LinkMarker::One);
        assert_eq!(op_of("a >- b").start, LinkMarker::Crow);
        assert_eq!(op_of("a ++- b").start, LinkMarker::ExactlyOne);
        assert_eq!(op_of("a +o- b").start, LinkMarker::ZeroOrOne);
        assert_eq!(op_of("a >+- b").start, LinkMarker::OneOrMany);
        assert_eq!(op_of("a >o- b").start, LinkMarker::ZeroOrMany);
        // Both sides at once: one-to-many, and the round-trip via the op spelling.
        let both = op_of("a +-< b");
        assert_eq!((both.start, both.end), (LinkMarker::One, LinkMarker::Crow));
        assert_eq!(op_of("a +-+ b").start, LinkMarker::One);
        assert_eq!(op_of("a +-+ b").end, LinkMarker::One);
        // A signed number is still a number, never a start marker.
        assert_eq!(kinds("+5"), vec![TokKind::Number(5.0)]);
        // A bare `o` with no max glyph is an error with a did-you-mean [SPEC 9].
        let err = lex("a -o b").expect_err("bare -o is an error");
        assert!(err.message.contains("'-o' needs a max glyph"), "{err:?}");
    }
}

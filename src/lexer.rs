use crate::ast::{LineStyle, LinkMarker, LinkOp};
use crate::error::Error;
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
    /// A backtick `` `…` `` expression body, captured raw (multi-line); the
    /// expression sub-language ([`crate::expr`]) parses it, so the main lexer never
    /// sees its operators [SPEC 10.7].
    Expr(String),

    Pipe,   // |
    Colon,  // : (attr binding)
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

    LinkOp(LinkOp),

    Newline,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokKind,
    pub span: Span,
}

pub fn lex(src: &str) -> Result<Vec<Token>, Error> {
    let mut lexer = Lexer {
        src,
        bytes: src.as_bytes(),
        i: 0,
        paren_depth: 0,
        tokens: Vec::new(),
    };
    lexer.run()?;
    Ok(lexer.tokens)
}

struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    i: usize,
    paren_depth: usize,
    tokens: Vec<Token>,
}

impl<'a> Lexer<'a> {
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
                    self.paren_depth += 1;
                    self.push_punct(TokKind::LParen, 1);
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
                b'"' => self.lex_string()?,
                // Single quotes are reserved, not strings [SPEC 2/21].
                b'\'' => {
                    return Err(Error::at(
                        Span::new(self.i, self.i + 1),
                        "single quotes are not strings — use \"…\"",
                    ));
                }
                b'`' => self.lex_expr()?,
                b'#' => self.lex_hash()?,
                b'.' => {
                    if self.peek(1).is_some_and(|c| c.is_ascii_digit()) {
                        self.lex_number()?;
                    } else {
                        // `.` is a path / class / side separator; `..` is two of
                        // them, no longer a link line.
                        self.push_punct(TokKind::Dot, 1);
                    }
                }
                // CSS var override `--name…` (defs line start or attr value).
                b'-' if self.peek(1) == Some(b'-') && self.peek(2).is_some_and(is_ident_start) => {
                    self.lex_raw_css_var()?;
                }
                // Signed number: `-5`, `-.5`, `+5`.
                b'-' if self.peek(1).is_some_and(|c| c.is_ascii_digit())
                    || (self.peek(1) == Some(b'.')
                        && self.peek(2).is_some_and(|c| c.is_ascii_digit())) =>
                {
                    self.lex_number()?;
                }
                b'+' => self.lex_number()?,
                // Link-op starts: any of these characters can begin a link op.
                b'-' | b'~' | b'<' | b'>' => self.lex_link_op()?,
                // `*` is a link-op start marker only when followed by a line char.
                b'*' if self.peek(1).is_some_and(is_link_line_start) => self.lex_link_op()?,
                d if d.is_ascii_digit() => self.lex_number()?,
                c if is_ident_start(c) => self.lex_ident(),
                _ => {
                    return Err(Error::at(
                        Span::new(self.i, self.i + 1),
                        format!("unexpected character {:?}", c as char),
                    ));
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
        if self.paren_depth == 0 {
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
                        ));
                    }
                }
                self.i += 1;
                continue;
            }
            let ch = self.src[self.i..].chars().next().expect("non-empty utf-8");
            value.push(ch);
            self.i += ch.len_utf8();
        }

        Err(Error::at(
            Span::new(start, self.i),
            "unterminated string literal",
        ))
    }

    /// A backtick `` `…` `` region, captured raw (multi-line) — the expression
    /// engine parses the body, so the main lexer never sees operators [SPEC 10.7].
    fn lex_expr(&mut self) -> Result<(), Error> {
        let start = self.i;
        self.i += 1; // opening backtick
        let body_start = self.i;
        while self.i < self.bytes.len() && self.bytes[self.i] != b'`' {
            self.i += 1;
        }
        if self.i >= self.bytes.len() {
            return Err(Error::at(
                Span::new(start, self.i),
                "unterminated `…` expression",
            ));
        }
        let body = self.src[body_start..self.i].to_string();
        self.i += 1; // closing backtick
        self.tokens.push(Token {
            kind: TokKind::Expr(body),
            span: Span::new(start, self.i),
        });
        Ok(())
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

        if !saw_digit {
            return Err(Error::at(
                Span::new(start, self.i),
                "invalid number literal",
            ));
        }

        let text = &self.src[start..self.i];
        let value: f64 = text.parse().map_err(|_| {
            Error::at(
                Span::new(start, self.i),
                format!("invalid number literal '{}'", text),
            )
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

    fn lex_ident(&mut self) {
        let start = self.i;
        while self.i < self.bytes.len() && is_ident_continue(self.bytes[self.i]) {
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

        let start = self.consume_marker(&mut p, MarkerSide::Start);
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
        let end = self.consume_marker(&mut p, MarkerSide::End);

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

    fn consume_marker(&self, p: &mut usize, side: MarkerSide) -> LinkMarker {
        // Try `<>` first (longest match).
        if self.bytes.get(*p) == Some(&b'<') && self.bytes.get(*p + 1) == Some(&b'>') {
            *p += 2;
            return LinkMarker::Diamond;
        }
        let c = match self.bytes.get(*p) {
            Some(&c) => c,
            None => return LinkMarker::None,
        };
        let marker = match (c, side) {
            (b'<', MarkerSide::Start) => LinkMarker::Arrow,
            (b'<', MarkerSide::End) => LinkMarker::Crow,
            (b'>', MarkerSide::Start) => LinkMarker::Crow,
            (b'>', MarkerSide::End) => LinkMarker::Arrow,
            (b'*', _) => LinkMarker::Dot,
            _ => return LinkMarker::None,
        };
        *p += 1;
        marker
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
}

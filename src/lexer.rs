use crate::ast::{LineStyle, WireMarker, WireOp};
use crate::error::Error;
use crate::span::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum TokKind {
    Ident(String),
    String(String),
    Number(f64),
    Hex(String),       // hex digits without leading '#'
    RawCssVar(String), // CSS var name without leading '--'

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

    WireOp(WireOp),

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
                // Single quotes are reserved, not strings (SPEC §2/§18).
                b'\'' => {
                    return Err(Error::at(
                        Span::new(self.i, self.i + 1),
                        "single quotes are not strings — use \"…\"",
                    ));
                }
                b'#' => self.lex_hex()?,
                b'.' => {
                    if self.peek(1).is_some_and(|c| c.is_ascii_digit()) {
                        self.lex_number()?;
                    } else if self.peek(1) == Some(b'.') {
                        // `..` is the dotted wire line; a single `.` is a path /
                        // class / side separator.
                        self.lex_wire_op()?;
                    } else {
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
                // Wire-op starts: any of these characters can begin a wire op.
                b'-' | b'~' | b'<' | b'>' => self.lex_wire_op()?,
                // `*` is a wire-op start marker only when followed by a line char.
                b'*' if self.peek(1).is_some_and(is_wire_line_start) => self.lex_wire_op()?,
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
                self.tokens.push(Token {
                    kind: TokKind::String(value),
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

    fn lex_hex(&mut self) -> Result<(), Error> {
        let start = self.i;
        self.i += 1; // '#'
        let digits_start = self.i;
        while self.i < self.bytes.len() && self.bytes[self.i].is_ascii_hexdigit() {
            self.i += 1;
        }
        let len = self.i - digits_start;
        if !matches!(len, 3 | 6 | 8) {
            return Err(Error::at(
                Span::new(start, self.i),
                format!("invalid hex color '{}'", &self.src[start..self.i]),
            ));
        }
        let digits = self.src[digits_start..self.i].to_string();
        self.tokens.push(Token {
            kind: TokKind::Hex(digits),
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
        self.tokens.push(Token {
            kind: TokKind::Number(value),
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

    // ─────────────────────── Wire ops ───────────────────────
    //
    // A wire op is `[start_marker?][line][end_marker?]`, all glued together
    // with no whitespace. Start markers (`<`,`>`,`*`,`<>`) translate to
    // different `WireMarker` kinds than end markers — `<` at start is Arrow,
    // `<` at end is Crow, and vice versa. Position discriminates.
    //
    // The line component is required; if no line is consumed, lexing fails.

    fn lex_wire_op(&mut self) -> Result<(), Error> {
        let start_i = self.i;
        let mut p = self.i;

        let start = self.consume_marker(&mut p, MarkerSide::Start);
        let line = match self.consume_line(&mut p) {
            Some(l) => l,
            None => {
                return Err(Error::at(
                    Span::new(start_i, p.max(start_i + 1)),
                    format!(
                        "expected wire-op line after '{}'",
                        &self.src[start_i..p.max(start_i + 1)]
                    ),
                ));
            }
        };
        let end = self.consume_marker(&mut p, MarkerSide::End);

        // Reject the no-op case: a lone `--` followed by ident-start should have
        // been caught earlier as a RawCssVar. But a stray `--name` reached here
        // is an error (unreachable in practice).
        let op = WireOp { line, start, end };
        let span = Span::new(start_i, p);
        self.tokens.push(Token {
            kind: TokKind::WireOp(op),
            span,
        });
        self.i = p;
        Ok(())
    }

    fn consume_marker(&self, p: &mut usize, side: MarkerSide) -> WireMarker {
        // Try `<>` first (longest match).
        if self.bytes.get(*p) == Some(&b'<') && self.bytes.get(*p + 1) == Some(&b'>') {
            *p += 2;
            return WireMarker::Diamond;
        }
        let c = match self.bytes.get(*p) {
            Some(&c) => c,
            None => return WireMarker::None,
        };
        let marker = match (c, side) {
            (b'<', MarkerSide::Start) => WireMarker::Arrow,
            (b'<', MarkerSide::End) => WireMarker::Crow,
            (b'>', MarkerSide::Start) => WireMarker::Crow,
            (b'>', MarkerSide::End) => WireMarker::Arrow,
            (b'*', _) => WireMarker::Dot,
            _ => return WireMarker::None,
        };
        *p += 1;
        marker
    }

    fn consume_line(&self, p: &mut usize) -> Option<LineStyle> {
        let rest = self.bytes.get(*p..)?;
        // Longest match: `..`, `--`, `-`, `~`.
        if rest.starts_with(b"..") {
            *p += 2;
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

fn is_wire_line_start(c: u8) -> bool {
    matches!(c, b'-' | b'~' | b'.')
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_ident_continue(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'-'
}

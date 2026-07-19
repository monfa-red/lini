//! The parser — single-pass recursive descent over the grammar in [SPEC 21].
//!
//! The bracket-and-bars vocabulary makes one token of lookahead enough, with no
//! type-set prescan: `{` opens style, `[` opens children, `|…|` is a type. The
//! file is three phases — an optional leading `{ }` stylesheet, then the canvas
//! instances, then the links — and a body nests the same idea (a `{ }`, then a
//! `[ ]` of children and internal links).

use super::ast::*;
use crate::ast::{ChainOp, DrawOp, LineStyle, LinkMarker, LinkOp};
use crate::error::{Code, Error};
use crate::lexer::{TokKind, Token};
use crate::span::Span;

mod classify;
mod decl;
mod links;
mod nodes;
mod selector;
mod values;

#[cfg(test)]
mod tests;

/// Parse a token stream into a [`File`]. `src` backs the raw slices a `(…)` group or
/// operator-bearing argument keeps for [`crate::expr`] [SPEC 10.7].
pub fn parse(src: &str, tokens: &[Token]) -> Result<File, Error> {
    Parser::new(src, tokens).parse_file()
}

/// The shared head tail [SPEC 3/9]: a head label, worn classes, and the head's
/// own style block — what `parse_tail` reads after a node's bars or a link's
/// endpoints. The `[ ]` content is parsed by the caller.
struct Tail {
    label: Option<TextNode>,
    classes: Vec<String>,
    style: Vec<Decl>,
    style_span: Option<Span>,
}

/// What a statement at the cursor is.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Node,
    Link,
    Decl,
    Var,
    Rule,
    Define,
    Func,
    Unknown,
}

struct Parser<'a> {
    src: &'a str,
    toks: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str, toks: &'a [Token]) -> Self {
        Self { src, toks, pos: 0 }
    }

    // ───────────────────────── Cursor ─────────────────────────

    fn kind(&self) -> Option<&TokKind> {
        self.toks.get(self.pos).map(|t| &t.kind)
    }

    fn kind_at(&self, n: usize) -> Option<&TokKind> {
        self.toks.get(self.pos + n).map(|t| &t.kind)
    }

    /// The cursor sits on the link selector `|-|` [SPEC 4, 9]: bars wrapping a
    /// bare solid dash — a line, in the identity capsule. The `-` lexes as a
    /// marker-less solid link op, so this is `| · - · |`.
    fn at_link_bars(&self) -> bool {
        matches!(self.kind(), Some(TokKind::Pipe))
            && matches!(
                self.kind_at(1),
                Some(TokKind::LinkOp(LinkOp {
                    line: LineStyle::Solid,
                    start: LinkMarker::None,
                    end: LinkMarker::None,
                }))
            )
            && matches!(self.kind_at(2), Some(TokKind::Pipe))
    }

    fn span(&self) -> Span {
        self.toks
            .get(self.pos)
            .map(|t| t.span)
            .unwrap_or_else(|| self.last_span())
    }

    fn last_span(&self) -> Span {
        self.toks
            .get(self.pos.saturating_sub(1))
            .map(|t| t.span)
            .unwrap_or_default()
    }

    /// The span of a construct that opened at `start` and runs to the last
    /// consumed token — the common "whole statement / node / block" span.
    fn span_from(&self, start: Span) -> Span {
        Span::new(start.start, self.last_span().end)
    }

    /// Whether the token at `pos + n` is glued (no whitespace) to the one before
    /// it — how `a.b` (endpoint path) is told from `a .b` (a class).
    fn glued_at(&self, n: usize) -> bool {
        let i = self.pos + n;
        match (
            i.checked_sub(1).and_then(|j| self.toks.get(j)),
            self.toks.get(i),
        ) {
            (Some(a), Some(b)) => a.span.end == b.span.start,
            _ => false,
        }
    }

    fn eat(&mut self, k: &TokKind) -> bool {
        if self.kind() == Some(k) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, k: &TokKind, what: &str) -> Result<(), Error> {
        if self.eat(k) {
            Ok(())
        } else {
            Err(self
                .err(format!("expected {}", what))
                .code(Code::EXPECTED_TOKEN))
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.kind(), Some(TokKind::Newline)) {
            self.pos += 1;
        }
    }

    fn err(&self, msg: impl Into<String>) -> Error {
        Error::at(self.span(), msg.into())
    }

    fn expect_ident(&mut self) -> Result<(String, Span), Error> {
        let name = match self.kind() {
            Some(TokKind::Ident(s)) => s.clone(),
            _ => return Err(self.err("expected identifier").code(Code::EXPECTED_TOKEN)),
        };
        let span = self.span();
        self.pos += 1;
        Ok((name, span))
    }

    /// Consume a statement terminator (newline / `;`), or accept a closing
    /// bracket / a following string / EOF. A string is self-delimiting, so
    /// `"a" "b" "c"` is three text nodes [SPEC 3].
    fn terminator(&mut self) -> Result<(), Error> {
        if matches!(self.kind(), Some(TokKind::Newline) | Some(TokKind::Semi)) {
            self.pos += 1;
            self.skip_newlines();
            Ok(())
        } else if matches!(
            self.kind(),
            Some(TokKind::RBrace) | Some(TokKind::RBracket) | Some(TokKind::String(_)) | None
        ) {
            Ok(())
        } else {
            Err(self
                .err("expected a newline, ';', or a closing bracket")
                .code(Code::EXPECTED_TOKEN))
        }
    }

    /// `||` at `pos + n` — two **adjacent** pipes, the mate op [SPEC 15.5, 21].
    fn pipes_glued_at(&self, n: usize) -> bool {
        matches!(self.kind_at(n), Some(TokKind::Pipe))
            && matches!(self.kind_at(n + 1), Some(TokKind::Pipe))
            && self.glued_at(n + 1)
    }

    // ───────────────────────── File ─────────────────────────

    fn parse_file(&mut self) -> Result<File, Error> {
        let mut file = File {
            stylesheet: Vec::new(),
            stylesheet_span: Span::default(),
            instances: Vec::new(),
            links: Vec::new(),
        };
        self.skip_newlines();
        if matches!(self.kind(), Some(TokKind::LBrace)) {
            let start = self.span();
            file.stylesheet = self.parse_stylesheet()?;
            file.stylesheet_span = self.span_from(start);
            self.skip_newlines();
        }
        // Instances and links interleave, in source order [SPEC 3]: a
        // `layout: sequence` reads that order as time. They stay in separate
        // lists — each already in source order — and the formatter / sequence
        // engine recover the interleave from spans.
        while self.kind().is_some() {
            match self.classify_body() {
                Kind::Node => file.instances.push(self.parse_child()?),
                Kind::Link => file.links.push(self.parse_link()?),
                Kind::Decl => {
                    return Err(self
                        .err("a declaration belongs in a '{ }' block")
                        .code(Code::DECL_OUTSIDE_BLOCK));
                }
                Kind::Var => {
                    return Err(self.err("variables are declared in the stylesheet '{ }'"));
                }
                _ if matches!(self.kind(), Some(TokKind::LBrace)) => {
                    return Err(self
                        .err("the stylesheet '{ }' must come first, before any instance")
                        .code(Code::STYLESHEET_ORDER));
                }
                _ => {
                    return Err(self.err(
                        "a node leads with bars — write '|box#X|' (a bare name is a link endpoint)",
                    ));
                }
            }
            self.terminator()?;
        }
        Ok(file)
    }

    fn parse_stylesheet(&mut self) -> Result<Vec<StyleItem>, Error> {
        self.expect(&TokKind::LBrace, "'{'")?;
        self.skip_newlines();
        let mut items = Vec::new();
        while !matches!(self.kind(), Some(TokKind::RBrace) | None) {
            let item = match self.classify_setup()? {
                Kind::Var => StyleItem::Var(self.parse_var()?),
                Kind::Decl => StyleItem::RootDecl(self.parse_decl()?),
                Kind::Rule => StyleItem::Rule(self.parse_rule()?),
                Kind::Define => StyleItem::Define(self.parse_define()?),
                Kind::Func => StyleItem::Binding(self.parse_binding()?),
                _ => unreachable!(),
            };
            items.push(item);
            self.terminator()?;
        }
        self.expect(&TokKind::RBrace, "'}'")?;
        Ok(items)
    }
}

/// Which bars are being parsed — picks the glued-class error wording, and gates
/// the `|-|`-as-instance rejection [SPEC 9].
#[derive(Clone, Copy, PartialEq)]
enum BarsCtx {
    Instance,
    Selector,
}

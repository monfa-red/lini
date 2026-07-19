//! Declarations, `--var`s, `=` bindings, and `|name::base|` defines.

use super::*;
use crate::error::Code;

impl<'a> Parser<'a> {
    // ───────────────────────── Declarations ─────────────────────────

    /// `key: v…, v…` — the name token is an `Ident`. A `draw:` value additionally
    /// admits the pen items (`call:segment` / freestanding `:segment`) [SPEC 15.3, 21] —
    /// the property-scoped flag keeps the runaway-declaration diagnostics sharp
    /// everywhere else.
    pub(super) fn parse_decl(&mut self) -> Result<Decl, Error> {
        let (name, start) = self.expect_ident()?;
        if !self.eat(&TokKind::Colon) {
            return Err(self
                .err(format!("expected ':' after '{}'", name))
                .code(Code::EXPECTED_TOKEN));
        }
        let pen = name == "draw";
        let (groups, end) = self.parse_values_in(pen)?;
        Ok(Decl {
            name,
            groups,
            span: Span::new(start.start, end.end),
        })
    }

    /// `--name: v…` — the name token is a `RawCssVar` (name stored without `--`).
    pub(super) fn parse_var(&mut self) -> Result<Decl, Error> {
        let start = self.span();
        let name = match self.kind() {
            Some(TokKind::RawCssVar(s)) => s.clone(),
            _ => return Err(self.err("expected '--name'").code(Code::EXPECTED_TOKEN)),
        };
        self.pos += 1;
        if !self.eat(&TokKind::Colon) {
            return Err(self
                .err(format!("expected ':' after '--{}'", name))
                .code(Code::EXPECTED_TOKEN));
        }
        let (groups, end) = self.parse_values()?;
        Ok(Decl {
            name,
            groups,
            span: Span::new(start.start, end.end),
        })
    }

    /// `|name::base| { style } [ children ]`.
    pub(super) fn parse_define(&mut self) -> Result<Define, Error> {
        let start = self.span();
        self.expect(&TokKind::Pipe, "'|'")?;
        let (name, _) = self.expect_ident()?;
        self.expect(&TokKind::DColon, "'::'")?;
        let (base, _) = self.expect_ident()?;
        self.expect(&TokKind::Pipe, "'|'")?;
        let (style, style_span) = self.opt_style()?;
        let (children, links) = self.opt_children()?;
        Ok(Define {
            name,
            base,
            style,
            style_span,
            children,
            links,
            span: self.span_from(start),
        })
    }

    /// An `=` binding [SPEC 10.7]: `name = value` (a scalar) or `name(params) = value`
    /// (a function). The right-hand value is captured as raw text for [`crate::expr`].
    /// The trailing `;` is consumed by the caller's terminator, like a declaration.
    pub(super) fn parse_binding(&mut self) -> Result<FuncDef, Error> {
        let (name, start) = self.expect_ident()?;
        let mut params = Vec::new();
        if self.eat(&TokKind::LParen) {
            if !matches!(self.kind(), Some(TokKind::RParen)) {
                params.push(self.expect_ident()?.0);
                while self.eat(&TokKind::Comma) {
                    params.push(self.expect_ident()?.0);
                }
            }
            self.expect(&TokKind::RParen, "')'")?;
        }
        self.expect(&TokKind::Assign, "'=' — a binding is 'name = value'")?;
        let body = self.take_binding_body()?;
        Ok(FuncDef {
            name,
            params,
            body,
            span: self.span_from(start),
        })
    }

    /// The right-hand side of a binding: a `(…)` group's inner text (so locals and a
    /// point read as a body), or a bare literal / name / call up to the terminator.
    pub(super) fn take_binding_body(&mut self) -> Result<String, Error> {
        if matches!(self.kind(), Some(TokKind::LParen)) {
            return self.take_group();
        }
        let start_byte = self.span().start;
        let mut end_byte = start_byte;
        while !matches!(
            self.kind(),
            Some(TokKind::Semi) | Some(TokKind::Newline) | Some(TokKind::RBrace) | None
        ) {
            end_byte = self.span().end;
            self.pos += 1;
        }
        let body = self.src[start_byte..end_byte].trim();
        if body.is_empty() {
            return Err(self.err("a binding needs a value — 'name = value'"));
        }
        Ok(body.to_string())
    }
}

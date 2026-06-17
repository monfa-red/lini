use crate::ast::*;
use crate::error::Error;
use crate::lexer::{TokKind, Token};
use crate::span::Span;

pub fn parse(tokens: &[Token]) -> Result<File, Error> {
    let mut p = Parser {
        toks: tokens,
        pos: 0,
    };
    p.skip_newlines();

    // Optional defs block: must be the very first non-trivia token.
    let defs = if matches!(p.peek_kind(), Some(TokKind::LBrace)) {
        Some(p.parse_defs_block()?)
    } else {
        None
    };
    p.skip_newlines();

    let mut stmts = Vec::new();
    while p.peek().is_some() {
        stmts.push(p.parse_stmt()?);
        p.skip_newlines();
    }
    Ok(File { defs, stmts })
}

/// Parse a single Lini value from a complete token stream. Used by the theme
/// loader to interpret `--lini-NAME: VALUE;` declarations as Lini values.
pub fn parse_value_only(tokens: &[Token]) -> Result<Value, Error> {
    let mut p = Parser {
        toks: tokens,
        pos: 0,
    };
    p.skip_newlines();
    let v = p.parse_value()?;
    p.skip_newlines();
    if p.peek().is_some() {
        return Err(Error::at(p.next_span(), "trailing tokens after value"));
    }
    Ok(v)
}

struct Parser<'a> {
    toks: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    // ───────────────────────── Cursor helpers ─────────────────────────

    fn peek(&self) -> Option<&Token> {
        self.toks.get(self.pos)
    }

    fn peek_kind(&self) -> Option<&TokKind> {
        self.peek().map(|t| &t.kind)
    }

    fn next_span(&self) -> Span {
        self.peek()
            .map(|t| t.span)
            .unwrap_or_else(|| self.last_span())
    }

    fn last_span(&self) -> Span {
        self.toks
            .get(self.pos.saturating_sub(1))
            .map(|t| t.span)
            .unwrap_or_default()
    }

    fn prev_end(&self) -> Option<usize> {
        if self.pos == 0 {
            None
        } else {
            self.toks.get(self.pos - 1).map(|t| t.span.end)
        }
    }

    /// True iff the current token is glued to the previous one (no whitespace).
    fn current_glued_to_prev(&self) -> bool {
        match (self.prev_end(), self.peek()) {
            (Some(prev_end), Some(tok)) => prev_end == tok.span.start,
            _ => false,
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek_kind(), Some(TokKind::Newline)) {
            self.pos += 1;
        }
    }

    fn consume_terminator(&mut self) -> Result<(), Error> {
        match self.peek_kind() {
            Some(TokKind::Newline) | Some(TokKind::Semi) => {
                self.pos += 1;
                self.skip_newlines();
                Ok(())
            }
            Some(TokKind::RBrace) | None => Ok(()),
            Some(other) => Err(Error::at(
                self.next_span(),
                format!("expected newline, ';' or '}}', found {}", tok_desc(other)),
            )),
        }
    }

    fn expect_kind(&mut self, pred: impl Fn(&TokKind) -> bool, what: &str) -> Result<Span, Error> {
        match self.peek() {
            Some(t) if pred(&t.kind) => {
                let span = t.span;
                self.pos += 1;
                Ok(span)
            }
            Some(t) => Err(Error::at(
                t.span,
                format!("expected {}, found {}", what, tok_desc(&t.kind)),
            )),
            None => Err(Error::at(
                self.last_span(),
                format!("expected {}, found end of file", what),
            )),
        }
    }

    fn expect_lbrace(&mut self) -> Result<Span, Error> {
        self.expect_kind(|k| matches!(k, TokKind::LBrace), "'{'")
    }
    fn expect_rbrace(&mut self) -> Result<Span, Error> {
        self.expect_kind(|k| matches!(k, TokKind::RBrace), "'}'")
    }
    fn expect_lparen(&mut self) -> Result<Span, Error> {
        self.expect_kind(|k| matches!(k, TokKind::LParen), "'('")
    }
    fn expect_rparen(&mut self) -> Result<Span, Error> {
        self.expect_kind(|k| matches!(k, TokKind::RParen), "')'")
    }
    fn expect_lbracket(&mut self) -> Result<Span, Error> {
        self.expect_kind(|k| matches!(k, TokKind::LBracket), "'['")
    }
    fn expect_rbracket(&mut self) -> Result<Span, Error> {
        self.expect_kind(|k| matches!(k, TokKind::RBracket), "']'")
    }
    fn expect_pipe(&mut self) -> Result<Span, Error> {
        self.expect_kind(|k| matches!(k, TokKind::Pipe), "'|'")
    }
    fn expect_dot(&mut self) -> Result<Span, Error> {
        self.expect_kind(|k| matches!(k, TokKind::Dot), "'.'")
    }

    fn expect_ident(&mut self) -> Result<(String, Span), Error> {
        match self.peek() {
            Some(Token {
                kind: TokKind::Ident(name),
                span,
            }) => {
                let out = (name.clone(), *span);
                self.pos += 1;
                Ok(out)
            }
            Some(t) => Err(Error::at(
                t.span,
                format!("expected identifier, found {}", tok_desc(&t.kind)),
            )),
            None => Err(Error::at(
                self.last_span(),
                "expected identifier, found end of file",
            )),
        }
    }

    fn expect_string(&mut self) -> Result<String, Error> {
        match self.peek() {
            Some(Token {
                kind: TokKind::String(s),
                ..
            }) => {
                let out = s.clone();
                self.pos += 1;
                Ok(out)
            }
            Some(t) => Err(Error::at(
                t.span,
                format!("expected string, found {}", tok_desc(&t.kind)),
            )),
            None => Err(Error::at(
                self.last_span(),
                "expected string, found end of file",
            )),
        }
    }

    fn eat_string(&mut self) -> Option<String> {
        if let Some(Token {
            kind: TokKind::String(s),
            ..
        }) = self.peek()
        {
            let out = s.clone();
            self.pos += 1;
            Some(out)
        } else {
            None
        }
    }

    // ───────────────────────── Values ─────────────────────────

    fn parse_value(&mut self) -> Result<Value, Error> {
        let tok = match self.peek() {
            Some(t) => t,
            None => {
                return Err(Error::at(
                    self.last_span(),
                    "expected value, found end of file",
                ));
            }
        };
        match &tok.kind {
            TokKind::Number(n) => {
                let v = *n;
                self.pos += 1;
                Ok(Value::Number(v))
            }
            TokKind::String(s) => {
                let v = s.clone();
                self.pos += 1;
                Ok(Value::String(v))
            }
            TokKind::Hex(h) => {
                let v = h.clone();
                self.pos += 1;
                Ok(Value::Hex(v))
            }
            TokKind::Ident(_) => {
                let (name, name_span) = self.expect_ident()?;
                if matches!(self.peek_kind(), Some(TokKind::LParen)) {
                    Ok(Value::Call(self.parse_call(name, name_span)?))
                } else {
                    Ok(Value::Ident(name))
                }
            }
            TokKind::LParen => self.parse_tuple_value(),
            TokKind::LBracket => self.parse_list_value(),
            TokKind::RawCssVar(name) => {
                let v = name.clone();
                self.pos += 1;
                Ok(Value::RawCssVar(v))
            }
            other => Err(Error::at(
                tok.span,
                format!("expected value, found {}", tok_desc(other)),
            )),
        }
    }

    fn parse_call(&mut self, name: String, name_span: Span) -> Result<FnCall, Error> {
        self.expect_lparen()?;
        let mut args = Vec::new();
        if !matches!(self.peek_kind(), Some(TokKind::RParen)) {
            args.push(self.parse_value()?);
            while matches!(self.peek_kind(), Some(TokKind::Comma)) {
                self.pos += 1;
                args.push(self.parse_value()?);
            }
        }
        let end = self.expect_rparen()?;
        Ok(FnCall {
            name,
            args,
            span: Span::new(name_span.start, end.end),
        })
    }

    fn parse_tuple_value(&mut self) -> Result<Value, Error> {
        self.expect_lparen()?;
        let mut items = Vec::new();
        if !matches!(self.peek_kind(), Some(TokKind::RParen)) {
            items.push(self.parse_value()?);
            while matches!(self.peek_kind(), Some(TokKind::Comma)) {
                self.pos += 1;
                items.push(self.parse_value()?);
            }
        }
        self.expect_rparen()?;
        Ok(Value::Tuple(items))
    }

    fn parse_list_value(&mut self) -> Result<Value, Error> {
        self.expect_lbracket()?;
        let mut items = Vec::new();
        if !matches!(self.peek_kind(), Some(TokKind::RBracket)) {
            items.push(self.parse_value()?);
            while matches!(self.peek_kind(), Some(TokKind::Comma)) {
                self.pos += 1;
                items.push(self.parse_value()?);
            }
        }
        self.expect_rbracket()?;
        Ok(Value::List(items))
    }

    // ───────────────────────── Attr items ─────────────────────────

    fn parse_attr_items(&mut self) -> Result<Vec<AttrItem>, Error> {
        let mut items = Vec::new();
        loop {
            match self.peek_kind() {
                Some(TokKind::Dot) => {
                    // SPEC section 2: style refs require whitespace before `.`. A no-WS
                    // dot would mean an endpoint side, which is wire-only — stop
                    // and let the caller decide.
                    if self.current_glued_to_prev() {
                        break;
                    }
                    items.push(AttrItem::Style(self.parse_style_ref()?));
                }
                Some(TokKind::Ident(_)) => items.push(AttrItem::Attr(self.parse_attr()?)),
                _ => break,
            }
        }
        Ok(items)
    }

    fn parse_style_ref(&mut self) -> Result<StyleRef, Error> {
        let start = self.expect_dot()?;
        let (name, end) = self.expect_ident()?;
        Ok(StyleRef {
            name,
            span: Span::new(start.start, end.end),
        })
    }

    fn parse_attr(&mut self) -> Result<Attr, Error> {
        let (name, name_span) = self.expect_ident()?;
        // SPEC section 2: `name:value` — binding `:` has no whitespace on either side.
        let next_is_colon = matches!(self.peek_kind(), Some(TokKind::Colon));
        if !next_is_colon {
            return Err(Error::at(
                name_span,
                format!("attr '{}' requires a value: write '{}:VALUE'", name, name),
            ));
        }
        if !self.current_glued_to_prev() {
            return Err(Error::at(
                self.next_span(),
                "binding ':' must have no whitespace before it",
            ));
        }
        let colon_span = self.next_span();
        self.pos += 1;
        // Check no whitespace after the colon (value must be glued).
        if !self.current_glued_to_prev() {
            return Err(Error::at(
                colon_span,
                "binding ':' must have no whitespace after it",
            ));
        }
        let value = self.parse_value()?;
        let end = self.last_span();
        Ok(Attr {
            name,
            value,
            span: Span::new(name_span.start, end.end),
        })
    }

    // ───────────────────────── Type refs (`|name|`) ─────────────────────────

    fn parse_type_use(&mut self) -> Result<TypeRef, Error> {
        let start = self.expect_pipe()?;
        let (name, _) = self.expect_ident()?;
        // Disallow `name:base` inside a type-use ref (that's defs syntax).
        if matches!(self.peek_kind(), Some(TokKind::Colon)) {
            return Err(Error::at(
                self.next_span(),
                format!(
                    "type-use ref '|{}|' cannot carry ':base' (only valid in the defs block)",
                    name
                ),
            ));
        }
        let end = self.expect_pipe()?;
        Ok(TypeRef {
            name,
            span: Span::new(start.start, end.end),
        })
    }

    // ───────────────────────── Defs block ─────────────────────────

    fn parse_defs_block(&mut self) -> Result<DefsBlock, Error> {
        let start = self.expect_lbrace()?;
        let mut entries = Vec::new();
        self.skip_newlines();
        while !matches!(self.peek_kind(), Some(TokKind::RBrace) | None) {
            entries.push(self.parse_defs_entry()?);
            self.consume_terminator()?;
        }
        let end = self.expect_rbrace()?;
        Ok(DefsBlock {
            entries,
            span: Span::new(start.start, end.end),
        })
    }

    fn parse_defs_entry(&mut self) -> Result<DefsEntry, Error> {
        let start = self.next_span();
        match self.peek_kind() {
            // `--name:value`
            Some(TokKind::RawCssVar(_)) => {
                let name = match self.peek_kind() {
                    Some(TokKind::RawCssVar(n)) => n.clone(),
                    _ => unreachable!(),
                };
                self.pos += 1;
                if !matches!(self.peek_kind(), Some(TokKind::Colon)) {
                    return Err(Error::at(
                        self.next_span(),
                        format!("expected ':' after --{}", name),
                    ));
                }
                if !self.current_glued_to_prev() {
                    return Err(Error::at(
                        self.next_span(),
                        "binding ':' must have no whitespace before it",
                    ));
                }
                let colon_span = self.next_span();
                self.pos += 1;
                if !self.current_glued_to_prev() {
                    return Err(Error::at(
                        colon_span,
                        "binding ':' must have no whitespace after it",
                    ));
                }
                let value = self.parse_value()?;
                let end = self.last_span();
                Ok(DefsEntry::VarOverride(VarOverride {
                    name,
                    value,
                    span: Span::new(start.start, end.end),
                }))
            }
            // `.style attrs...`
            Some(TokKind::Dot) => {
                let _ = self.expect_dot()?;
                let (name, _) = self.expect_ident()?;
                let items = self.parse_attr_items()?;
                let end = self.last_span();
                Ok(DefsEntry::StyleDef(StyleDef {
                    name,
                    items,
                    span: Span::new(start.start, end.end),
                }))
            }
            // `|scene| ...`, `|name:base| ...`
            Some(TokKind::Pipe) => self.parse_pipe_defs_entry(start),
            other => Err(Error::at(
                self.next_span(),
                format!(
                    "expected defs entry (|scene|, |name:base|, .style, or --name:value), found {}",
                    other.map_or("end of file".to_string(), tok_desc)
                ),
            )),
        }
    }

    fn parse_pipe_defs_entry(&mut self, start: Span) -> Result<DefsEntry, Error> {
        // Peek inside the pipes to decide between scene config / shape def.
        // Expected: `|` Ident (`:` Ident)? `|` …
        // After parsing the first ident, if next is Pipe → scene config (or
        // shorthand). If next is Colon → shape def.
        self.expect_pipe()?;
        let (first, _first_span) = self.expect_ident()?;
        match self.peek_kind() {
            Some(TokKind::Pipe) => {
                // `|first|` — three roles, dispatched by name:
                //   `|scene|`  — root container config
                //   `|wire|`   — global wire defaults
                //   `|name|`   — type-defaults for any other recognised type
                // (Validation that `name` is a known type happens in resolve;
                // the parser accepts any identifier here.)
                let close = self.expect_pipe()?;
                let items = self.parse_attr_items()?;
                let end = if items.is_empty() {
                    close
                } else {
                    self.last_span()
                };
                let span = Span::new(start.start, end.end);
                Ok(match first.as_str() {
                    "scene" => DefsEntry::SceneConfig(SceneConfig { items, span }),
                    "wire" => DefsEntry::WireConfig(WireConfig { items, span }),
                    _ => DefsEntry::TypeDefaults(TypeDefaults {
                        name: first,
                        items,
                        span,
                    }),
                })
            }
            Some(TokKind::Colon) => {
                if !self.current_glued_to_prev() {
                    return Err(Error::at(
                        self.next_span(),
                        "binding ':' must have no whitespace before it",
                    ));
                }
                let colon_span = self.next_span();
                self.pos += 1;
                if !self.current_glued_to_prev() {
                    return Err(Error::at(
                        colon_span,
                        "binding ':' must have no whitespace after it",
                    ));
                }
                let (base_name, base_span) = self.expect_ident()?;
                let end_pipe = self.expect_pipe()?;
                let items = self.parse_attr_items()?;
                let body = self.parse_optional_body()?;
                let end = self.last_span();
                Ok(DefsEntry::ShapeDef(ShapeDef {
                    name: first,
                    base: TypeRef {
                        name: base_name,
                        span: base_span,
                    },
                    items,
                    body,
                    span: Span::new(start.start, end.end.max(end_pipe.end)),
                }))
            }
            other => Err(Error::at(
                self.next_span(),
                format!(
                    "expected '|' or ':' after '|{}', found {}",
                    first,
                    other.map_or("end of file".to_string(), tok_desc)
                ),
            )),
        }
    }

    // ───────────────────────── Statements (scene root + bodies) ─────────────────────────

    fn parse_stmt(&mut self) -> Result<Stmt, Error> {
        // Anonymous primitive: `|type| …`
        if matches!(self.peek_kind(), Some(TokKind::Pipe)) {
            let inst = self.parse_anonymous_inst()?;
            return Ok(Stmt::Node(inst));
        }
        // Otherwise must start with an ident.
        if !matches!(self.peek_kind(), Some(TokKind::Ident(_))) {
            return Err(Error::at(
                self.next_span(),
                format!(
                    "expected statement, found {}",
                    self.peek_kind().map_or("end of file".to_string(), tok_desc)
                ),
            ));
        }

        // We need lookahead to decide between a node decl and a wire decl.
        // The deciding token: WireOp/Amp/glued-Dot → wire; everything else → node.
        let save = self.pos;
        let (id, id_span) = self.expect_ident()?;

        let next = self.peek_kind();
        let is_glued_dot = matches!(next, Some(TokKind::Dot)) && self.current_glued_to_prev();

        if matches!(next, Some(TokKind::WireOp(_)) | Some(TokKind::Amp)) || is_glued_dot {
            // Wire: rewind and let parse_wire_decl re-parse the first endpoint.
            self.pos = save;
            let wire = self.parse_wire_decl()?;
            return Ok(Stmt::Wire(wire));
        }

        // Node decl continuing from this id.
        let inst = self.parse_node_inst_after_id(Some(id), id_span)?;
        Ok(Stmt::Node(inst))
    }

    fn parse_body(&mut self) -> Result<Vec<BodyItem>, Error> {
        self.expect_lbrace()?;
        let mut items = Vec::new();
        self.skip_newlines();
        while !matches!(self.peek_kind(), Some(TokKind::RBrace) | None) {
            items.push(self.parse_body_item()?);
            self.consume_terminator()?;
        }
        self.expect_rbrace()?;
        Ok(items)
    }

    fn parse_body_item(&mut self) -> Result<BodyItem, Error> {
        let stmt = self.parse_stmt()?;
        Ok(match stmt {
            Stmt::Node(n) => BodyItem::Inst(n),
            Stmt::Wire(w) => BodyItem::Wire(w),
        })
    }

    fn parse_optional_body(&mut self) -> Result<Option<Vec<BodyItem>>, Error> {
        if matches!(self.peek_kind(), Some(TokKind::LBrace)) {
            Ok(Some(self.parse_body()?))
        } else {
            Ok(None)
        }
    }

    // ─────────── Node decls ───────────

    /// Parse an anonymous primitive: `|type| label* (attr|style)* [{body}]`.
    fn parse_anonymous_inst(&mut self) -> Result<ShapeInst, Error> {
        let start = self.next_span();
        let ty = self.parse_type_use()?;
        let labels = self.parse_labels();
        let items = self.parse_attr_items()?;
        let body = self.parse_optional_body()?;
        let end = self.last_span();
        Ok(ShapeInst {
            id: None,
            ty,
            labels,
            items,
            body,
            span: Span::new(start.start, end.end),
        })
    }

    /// Parse a node decl after the id has been consumed. The id may be `None`
    /// only when called from `parse_anonymous_inst` — here always Some.
    fn parse_node_inst_after_id(
        &mut self,
        id: Option<String>,
        id_span: Span,
    ) -> Result<ShapeInst, Error> {
        // Optional `|type|` next.
        let ty = if matches!(self.peek_kind(), Some(TokKind::Pipe)) {
            self.parse_type_use()?
        } else {
            // SPEC section 1 default: omitted type → |rect|.
            TypeRef {
                name: "rect".to_string(),
                span: id_span,
            }
        };
        let labels = self.parse_labels();
        let items = self.parse_attr_items()?;
        let body = self.parse_optional_body()?;
        let end = self.last_span();
        Ok(ShapeInst {
            id,
            ty,
            labels,
            items,
            body,
            span: Span::new(id_span.start, end.end),
        })
    }

    /// Collect the leading positional label strings (zero or more). A `link:`
    /// attribute — not a positional string — makes a node clickable (SPEC §5).
    fn parse_labels(&mut self) -> Vec<String> {
        let mut labels = Vec::new();
        while let Some(s) = self.eat_string() {
            labels.push(s);
        }
        labels
    }

    // ─────────── Wire decls ───────────

    fn parse_wire_decl(&mut self) -> Result<WireDecl, Error> {
        let start = self.next_span();
        let first = self.parse_endpoint_group()?;
        let op = self.expect_wire_op()?;
        let mut chain = vec![first];
        chain.push(self.parse_endpoint_group()?);

        while let Some(TokKind::WireOp(next_op)) = self.peek_kind() {
            let next_op = *next_op;
            if next_op == op {
                self.pos += 1;
                chain.push(self.parse_endpoint_group()?);
            } else {
                return Err(Error::at(
                    self.next_span(),
                    format!(
                        "wire chain mixes operators '{}' and '{}'",
                        wire_op_str(op),
                        wire_op_str(next_op)
                    ),
                ));
            }
        }

        let labels = self.parse_labels();
        let items = self.parse_attr_items()?;
        let body = if matches!(self.peek_kind(), Some(TokKind::LBrace)) {
            Some(self.parse_wire_text_body()?)
        } else {
            None
        };
        let end = self.last_span();
        Ok(WireDecl {
            chain,
            op,
            labels,
            items,
            body,
            span: Span::new(start.start, end.end),
        })
    }

    fn parse_endpoint_group(&mut self) -> Result<EndpointGroup, Error> {
        let mut endpoints = vec![self.parse_endpoint()?];
        while matches!(self.peek_kind(), Some(TokKind::Amp)) {
            self.pos += 1;
            endpoints.push(self.parse_endpoint()?);
        }
        Ok(EndpointGroup { endpoints })
    }

    fn parse_endpoint(&mut self) -> Result<WireEndpoint, Error> {
        let (first, first_span) = self.expect_ident()?;
        let mut path = vec![first];
        let mut end_span = first_span;

        // Consume any number of `.ident` segments — but only if glued (no WS).
        while matches!(self.peek_kind(), Some(TokKind::Dot)) && self.current_glued_to_prev() {
            self.pos += 1; // dot
            // Next must be a glued ident.
            if !matches!(self.peek_kind(), Some(TokKind::Ident(_))) {
                return Err(Error::at(
                    self.next_span(),
                    "expected identifier after '.' in endpoint path",
                ));
            }
            if !self.current_glued_to_prev() {
                return Err(Error::at(
                    self.next_span(),
                    "endpoint '.' must have no whitespace after it",
                ));
            }
            let (seg, seg_span) = self.expect_ident()?;
            path.push(seg);
            end_span = seg_span;
        }

        // Per SPEC section 10: if the LAST segment matches a side name, peel it off.
        let side = if path.len() > 1 {
            if let Some(s) = Side::parse(path.last().unwrap()) {
                path.pop();
                Some(s)
            } else {
                None
            }
        } else {
            None
        };

        Ok(WireEndpoint {
            path,
            side,
            span: Span::new(first_span.start, end_span.end),
        })
    }

    fn expect_wire_op(&mut self) -> Result<WireOp, Error> {
        match self.peek_kind() {
            Some(TokKind::WireOp(op)) => {
                let op = *op;
                self.pos += 1;
                Ok(op)
            }
            _ => Err(Error::at(
                self.next_span(),
                format!(
                    "expected wire operator, found {}",
                    self.peek_kind().map_or("end of file".to_string(), tok_desc)
                ),
            )),
        }
    }

    fn parse_wire_text_body(&mut self) -> Result<Vec<TextDecl>, Error> {
        self.expect_lbrace()?;
        self.skip_newlines();
        let mut texts = Vec::new();
        while !matches!(self.peek_kind(), Some(TokKind::RBrace) | None) {
            let start = self.next_span();
            // Must be `|text| "string" attrs...`.
            self.expect_pipe()?;
            let (kw, kw_span) = self.expect_ident()?;
            if kw != "text" {
                return Err(Error::at(
                    kw_span,
                    "wire body may only contain |text| primitives",
                ));
            }
            self.expect_pipe()?;
            let text = self.expect_string()?;
            let items = self.parse_attr_items()?;
            let end = self.last_span();
            texts.push(TextDecl {
                text,
                items,
                span: Span::new(start.start, end.end),
            });
            self.consume_terminator()?;
        }
        self.expect_rbrace()?;
        Ok(texts)
    }
}

fn wire_op_str(op: WireOp) -> String {
    format!(
        "{}{}{}",
        op.start.start_str(),
        op.line.as_str(),
        op.end.end_str()
    )
}

fn tok_desc(k: &TokKind) -> String {
    match k {
        TokKind::Ident(s) => format!("identifier '{}'", s),
        TokKind::String(_) => "string".to_string(),
        TokKind::Number(_) => "number".to_string(),
        TokKind::Hex(_) => "hex color".to_string(),
        TokKind::RawCssVar(s) => format!("'--{}'", s),
        TokKind::Pipe => "'|'".to_string(),
        TokKind::Colon => "':'".to_string(),
        TokKind::DColon => "'::'".to_string(),
        TokKind::Dot => "'.'".to_string(),
        TokKind::Amp => "'&'".to_string(),
        TokKind::Semi => "';'".to_string(),
        TokKind::Comma => "','".to_string(),
        TokKind::LBrace => "'{'".to_string(),
        TokKind::RBrace => "'}'".to_string(),
        TokKind::LParen => "'('".to_string(),
        TokKind::RParen => "')'".to_string(),
        TokKind::LBracket => "'['".to_string(),
        TokKind::RBracket => "']'".to_string(),
        TokKind::WireOp(op) => format!("'{}'", wire_op_str(*op)),
        TokKind::Newline => "newline".to_string(),
    }
}

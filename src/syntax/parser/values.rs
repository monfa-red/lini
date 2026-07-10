//! Values, calls, groups, and expression capture [SPEC 10.7].

use super::*;

impl<'a> Parser<'a> {
    /// Comma-separated value groups; each group is a space-separated sequence. A
    /// declaration's value runs to `;` (or a closing `}` / `]`), so it may span
    /// lines — a newline inside a value is whitespace, not a terminator [SPEC 2/3].
    pub(super) fn parse_values(&mut self) -> Result<(Vec<Vec<Value>>, Span), Error> {
        self.parse_values_in(false)
    }

    pub(super) fn parse_values_in(&mut self, pen: bool) -> Result<(Vec<Vec<Value>>, Span), Error> {
        let start = self.span();
        let mut groups: Vec<Vec<Value>> = Vec::new();
        let mut current: Vec<Value> = Vec::new();
        loop {
            match self.kind() {
                Some(TokKind::Semi) | Some(TokKind::RBrace) | Some(TokKind::RBracket) | None => {
                    break;
                }
                Some(TokKind::Newline) => self.pos += 1,
                Some(TokKind::Comma) => {
                    self.pos += 1;
                    groups.push(std::mem::take(&mut current));
                }
                _ => current.push(self.parse_value_in(pen)?),
            }
        }
        groups.push(current);
        if groups.iter().all(|g| g.is_empty()) {
            return Err(self.err("declaration needs a value"));
        }
        let end = self.last_span();
        Ok((groups, Span::new(start.start, end.end)))
    }

    pub(super) fn parse_value(&mut self) -> Result<Value, Error> {
        self.parse_value_in(false)
    }

    pub(super) fn parse_value_in(&mut self, pen: bool) -> Result<Value, Error> {
        // An ident glued to `(` begins a call (`rgb(…)`, `scale(3)`); a spaced
        // `foo (…)` is the ident then a group (the call-glue rule, [SPEC 2]).
        if matches!(self.kind(), Some(TokKind::Ident(_))) {
            let (name, _) = self.expect_ident()?;
            return if matches!(self.kind(), Some(TokKind::LParen)) && self.glued_at(0) {
                let call = self.parse_call(name)?;
                // In a pen, a glued `:segment` after the `)` names the call's drawn
                // segment (`right(50):seat`, `fillet(3):r1`) [SPEC 15.3] — glued to
                // the `)` itself: a **spaced** `:name` is the floating-segment
                // error below (a station is `point():v`).
                if pen && self.glued_at(0) && self.at_glued_point_name() {
                    self.pos += 1; // ':'
                    let (point, _) = self.expect_ident()?;
                    if let Value::Call(c) = call {
                        return Ok(Value::NamedCall(c, point));
                    }
                    unreachable!("parse_call returns Value::Call");
                }
                Ok(call)
            } else {
                Ok(Value::Ident(name))
            };
        }
        // A free-standing `(…)` is a math group [SPEC 10.7] — its inner text folds to
        // a number or a point.
        if matches!(self.kind(), Some(TokKind::LParen)) {
            return Ok(Value::Expr(self.take_group()?));
        }
        // A `:segment` always glues to its call [SPEC 15.3] — a floating one
        // is one space away from silently renaming the wrong thing.
        if pen && self.at_glued_point_name() {
            return Err(self.err("a ':segment' glues to its call — name a station with point():v"));
        }
        let v = match self.kind() {
            Some(TokKind::Number(n)) => Value::Number(*n),
            Some(TokKind::Percent(n)) => Value::Percent(*n),
            Some(TokKind::String(s)) => Value::String(s.clone()),
            // `#…` in a value is a colour — validate the run as 3/4/6/8 hex digits.
            Some(TokKind::Hash(h)) => {
                let h = h.clone();
                if !is_hex_color(&h) {
                    return Err(self.err(format!("invalid hex color '#{h}'")));
                }
                Value::Hex(h)
            }
            Some(TokKind::RawCssVar(s)) => Value::Var(s.clone()),
            // A `:` in value position is the start of the next declaration — the
            // previous one ran on because it lacks a terminating `;` [SPEC 3/19].
            Some(TokKind::Colon) => return Err(self.err("a declaration ends with ';'")),
            // A bare link op in a value: math belongs in a group [SPEC 10.7].
            Some(TokKind::LinkOp(_)) => {
                return Err(self.err("to compute here, wrap the math in a group — e.g. (a - b)"));
            }
            _ => return Err(self.err("expected a value")),
        };
        self.pos += 1;
        Ok(v)
    }

    /// Consume a balanced `( … )` at the cursor and return its **inner** source, the
    /// outer parens stripped [SPEC 10.7]. The lexer already balanced the parens, so
    /// this counts token depth; the raw text goes to [`crate::expr`].
    pub(super) fn take_group(&mut self) -> Result<String, Error> {
        let open = self.span();
        self.pos += 1; // '('
        let inner_start = open.end;
        let mut depth = 1usize;
        loop {
            match self.kind() {
                Some(TokKind::LParen) => depth += 1,
                Some(TokKind::RParen) => {
                    depth -= 1;
                    if depth == 0 {
                        let inner = self.src[inner_start..self.span().start].to_string();
                        self.pos += 1; // ')'
                        return Ok(inner);
                    }
                }
                None => return Err(Error::at(open, "unterminated '(' group")),
                _ => {}
            }
            self.pos += 1;
        }
    }

    /// The cursor sits on a `:` immediately followed by a **glued** ident — the
    /// pen's point sigil (`:segment`), never a declaration's `:` (whose value is
    /// spaced off it in canonical style and is not an ident-only suffix).
    pub(super) fn at_glued_point_name(&self) -> bool {
        matches!(self.kind(), Some(TokKind::Colon))
            && matches!(self.kind_at(1), Some(TokKind::Ident(_)))
            && self.glued_at(1)
    }

    pub(super) fn parse_call(&mut self, name: String) -> Result<Value, Error> {
        self.pos += 1; // '('
        let mut args = Vec::new();
        if !matches!(self.kind(), Some(TokKind::RParen)) {
            args.push(self.parse_call_arg()?);
            while self.eat(&TokKind::Comma) {
                args.push(self.parse_call_arg()?);
            }
        }
        if !self.eat(&TokKind::RParen) {
            return Err(self.err("expected ')'"));
        }
        Ok(Value::Call(Call { name, args }))
    }

    /// One call-argument slot [SPEC 10.7]: an arg carrying a top-level operator is an
    /// expression, captured raw (the call's own parens are its group); otherwise a
    /// single value, or a space-separated group (`hatch(45 -45, 6)`'s angles, [SPEC 10.3]).
    pub(super) fn parse_call_arg(&mut self) -> Result<Value, Error> {
        if let Some(raw) = self.take_arg_expr() {
            return Ok(Value::Expr(raw));
        }
        let first = self.parse_value()?;
        if matches!(
            self.kind(),
            Some(TokKind::Comma) | Some(TokKind::RParen) | None
        ) {
            return Ok(first);
        }
        let mut group = vec![first];
        while !matches!(
            self.kind(),
            Some(TokKind::Comma) | Some(TokKind::RParen) | None
        ) {
            group.push(self.parse_value()?);
        }
        Ok(Value::Tuple(group))
    }

    /// If the argument at the cursor carries a top-level math operator, advance to its
    /// boundary (the next top-level `,` or the call's `)`) and return its raw source;
    /// otherwise leave the cursor untouched and return `None` [SPEC 10.7].
    pub(super) fn take_arg_expr(&mut self) -> Option<String> {
        let start_byte = self.span().start;
        let mut depth = 0usize;
        let mut has_op = false;
        let mut i = self.pos;
        while let Some(tok) = self.toks.get(i) {
            match &tok.kind {
                TokKind::LParen => depth += 1,
                TokKind::RParen if depth == 0 => break,
                TokKind::RParen => depth -= 1,
                TokKind::Comma if depth == 0 => break,
                k if depth == 0 && is_math_op(k) => has_op = true,
                _ => {}
            }
            i += 1;
        }
        if !has_op {
            return None;
        }
        let end_byte = self.toks[i - 1].span.end;
        self.pos = i;
        Some(self.src[start_byte..end_byte].to_string())
    }
}

/// A valid hex colour run (no `#`): 3, 4, 6, or 8 hex digits.
fn is_hex_color(s: &str) -> bool {
    matches!(s.len(), 3 | 4 | 6 | 8) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// A math-operator token — the tell that a value or call argument is an expression
/// [SPEC 10.7], so the parser keeps its raw source for [`crate::expr`].
fn is_math_op(k: &TokKind) -> bool {
    matches!(
        k,
        TokKind::Plus
            | TokKind::Minus
            | TokKind::Star
            | TokKind::Slash
            | TokKind::Caret
            | TokKind::Lt
            | TokKind::Le
            | TokKind::Gt
            | TokKind::Ge
            | TokKind::EqEq
            | TokKind::Ne
            | TokKind::Question
            | TokKind::Assign
    )
}

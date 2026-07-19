//! Rule selectors and the shared `|type#id|` identity parse.

use super::*;
use crate::error::Code;

impl<'a> Parser<'a> {
    // ───────────────────────── Rules & defines ─────────────────────────

    /// `selector { decls }` [SPEC 4] — `|box| { }`, `.hot { }`, `#hero { }`,
    /// `|table| |box| { }`.
    pub(super) fn parse_rule(&mut self) -> Result<Rule, Error> {
        let start = self.span();
        let selector = self.parse_selector()?;
        let (decls, _) = self.parse_style()?;
        Ok(Rule {
            selector,
            decls,
            span: self.span_from(start),
        })
    }

    /// A run of space-separated selector units up to the `{` [SPEC 4]: a type
    /// `|box|` / `|table#main|`, a class `.hot`, or an id `#hero`. The space is
    /// the descendant combinator; each unit keeps its sigil.
    pub(super) fn parse_selector(&mut self) -> Result<Selector, Error> {
        let mut units = Vec::new();
        loop {
            match self.kind() {
                // `|-|` — the link type [SPEC 9]: every link, styled like a node.
                Some(TokKind::Pipe) if self.at_link_bars() => {
                    self.pos += 3;
                    units.push(SelUnit::Link);
                }
                // `(-)` — the dimension type [SPEC 4, 15.6]: the `|-|` subtype, one
                // token, matching every dimension. In selector position (not after
                // endpoints) it never means the linear operator.
                Some(TokKind::DrawOp(DrawOp::Linear)) => {
                    self.pos += 1;
                    units.push(SelUnit::Dimension);
                }
                Some(TokKind::Pipe) => {
                    let (ty, id) = self.parse_identity(BarsCtx::Selector)?;
                    units.push(match ty {
                        Some(name) => SelUnit::Type { name, id },
                        None => SelUnit::Id(id.expect("identity yields a type or an id")),
                    });
                }
                Some(TokKind::Dot) => {
                    self.pos += 1;
                    units.push(SelUnit::Class(self.expect_ident()?.0));
                }
                Some(TokKind::Hash(_)) => units.push(SelUnit::Id(self.parse_hash_id()?)),
                _ => break,
            }
        }
        if units.is_empty() {
            return Err(self.err("a rule needs a selector"));
        }
        Ok(Selector { units })
    }

    /// The bars — `|type|`, `|type#id|`, or `|#id|` [SPEC 3]: the optional type
    /// and id, at least one present. Shared by an instance and a selector unit;
    /// `ctx` only picks the glued-class error wording.
    pub(super) fn parse_identity(
        &mut self,
        ctx: BarsCtx,
    ) -> Result<(Option<String>, Option<String>), Error> {
        // `|-|` is the link selector, not an identity: reachable here only as an
        // instance (a selector peels it first), so it draws nothing [SPEC 9].
        if ctx == BarsCtx::Instance && self.at_link_bars() {
            return Err(self
                .err("a link is drawn by an operator — '|-|' only styles links (write 'a -> b')"));
        }
        self.expect(&TokKind::Pipe, "'|'")?;
        let (ty, id) = match self.kind() {
            // `|#id|` — an id alone, the default box type.
            Some(TokKind::Hash(_)) => (None, Some(self.parse_hash_id()?)),
            Some(TokKind::Ident(_)) => {
                let name = self.expect_ident()?.0;
                if matches!(self.kind(), Some(TokKind::DColon)) {
                    return Err(self.err("a define belongs in the stylesheet"));
                }
                // A glued `#id` after the type (`|box#cat|`).
                let id = if matches!(self.kind(), Some(TokKind::Hash(_))) && self.glued_at(0) {
                    Some(self.parse_hash_id()?)
                } else {
                    None
                };
                // A glued `.class` is worn styling, not identity — rejected.
                if matches!(self.kind(), Some(TokKind::Dot)) && self.glued_at(0) {
                    return Err(self.glued_class_err(ctx));
                }
                (Some(name), id)
            }
            Some(TokKind::Dot) => return Err(self.glued_class_err(ctx)),
            _ => {
                return Err(self
                    .err("'| |' needs a type or an '#id'")
                    .code(Code::EMPTY_BARS));
            }
        };
        if let Some(t) = &ty {
            if t == "link" {
                return Err(self.err("links are drawn by operators, not the '|link|' type"));
            }
            if t == "node" {
                return Err(
                    self.err("'node' is the umbrella concept — write '|block|' for the bare box")
                );
            }
        }
        self.expect(&TokKind::Pipe, "'|'")?;
        Ok((ty, id))
    }

    /// Consume a `#name` token as an id, validating the run is a real ident
    /// (`#cat`, not `#123`).
    pub(super) fn parse_hash_id(&mut self) -> Result<String, Error> {
        let (run, span) = match self.kind() {
            Some(TokKind::Hash(s)) => (s.clone(), self.span()),
            _ => return Err(self.err("expected an '#id'").code(Code::EXPECTED_TOKEN)),
        };
        if !is_ident(&run) {
            return Err(Error::at(
                span,
                format!("'#{run}' is not a valid id — an id starts with a letter or '_'"),
            )
            .code(Code::INVALID_ID));
        }
        self.pos += 1;
        Ok(run)
    }

    pub(super) fn glued_class_err(&self, ctx: BarsCtx) -> Error {
        self.err(match ctx {
            BarsCtx::Instance => "a class follows the bars — write '|box| .hot', not '|box.hot|'",
            BarsCtx::Selector => {
                "a selector unit can't glue a type and a class — space them (descendant) or style '.hot'"
            }
        })
    }
}

/// A valid id / ident: starts with a letter or `_`, then ident chars.
fn is_ident(s: &str) -> bool {
    let mut bytes = s.bytes();
    matches!(bytes.next(), Some(c) if c.is_ascii_alphabetic() || c == b'_')
        && bytes.all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}

//! Links: endpoint chains, fan groups, and chain operators [SPEC 9].

use super::*;

impl<'a> Parser<'a> {
    // ───────────────────────── Links ─────────────────────────

    pub(super) fn parse_link(&mut self) -> Result<Link, Error> {
        let start = self.span();
        let mut chain = vec![self.parse_endpoint_group()?];
        let op = self.expect_chain_op()?;
        // A statement may be one-ended — a leader or a unary measure toward its
        // text [SPEC 15.6/21]: after the op, an ident is an endpoint; anything
        // else is the tail. Which ops (and scopes) allow it is resolve's call.
        if matches!(self.kind(), Some(TokKind::Ident(_))) {
            chain.push(self.parse_endpoint_group()?);
            while let Some((next, width)) = self.peek_chain_op() {
                if next != op {
                    return Err(self.err(format!(
                        "link chain mixes operators '{}' and '{}'",
                        op.spelling(),
                        next.spelling()
                    )));
                }
                self.pos += width;
                if !matches!(self.kind(), Some(TokKind::Ident(_))) {
                    return Err(self.err("a text callout ends its statement — chain before it"));
                }
                chain.push(self.parse_endpoint_group()?);
            }
        }
        // The same tail a node uses: a head label, worn classes, the link's own
        // style. The head label and the `[ ]` labels coexist — desugar
        // concatenates them for `along:` [SPEC 9].
        let Tail {
            label,
            classes,
            style,
            style_span,
        } = self.parse_tail()?;
        let labels = if matches!(self.kind(), Some(TokKind::LBracket)) {
            self.parse_label_block()?
        } else {
            Vec::new()
        };
        Ok(Link {
            chain,
            op,
            classes,
            style,
            style_span,
            label,
            labels,
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_endpoint_group(&mut self) -> Result<EndpointGroup, Error> {
        let mut endpoints = vec![self.parse_endpoint()?];
        while self.eat(&TokKind::Amp) {
            endpoints.push(self.parse_endpoint()?);
        }
        Ok(EndpointGroup { endpoints })
    }

    pub(super) fn parse_endpoint(&mut self) -> Result<Endpoint, Error> {
        let (first, first_span) = self.expect_ident()?;
        let mut path = vec![first];
        let mut end = first_span;
        while matches!(self.kind(), Some(TokKind::Dot)) && self.glued_at(0) {
            self.pos += 1; // '.'
            if !self.glued_at(0) {
                return Err(self.err("endpoint '.' must have no whitespace after it"));
            }
            let (seg, seg_span) = self.expect_ident()?;
            path.push(seg);
            end = seg_span;
        }
        // A trailing `:point` names an anchor [SPEC 9, 15.2] — a side everywhere,
        // the wider set (corners, `center`, authored names) in a drawing scope;
        // resolve validates it there. The path no longer peels a final `.left` —
        // that is now a child named `left`.
        let point = if self.eat(&TokKind::Colon) {
            let (name, name_span) = self.expect_ident()?;
            end = name_span;
            Some(PointRef {
                name,
                span: name_span,
            })
        } else {
            None
        };
        Ok(Endpoint {
            path,
            point,
            span: Span::new(first_span.start, end.end),
        })
    }

    /// The chain op at the cursor (and its token width — `||` spans two), as an
    /// owned copy so a loop over it doesn't hold a borrow of `self`.
    pub(super) fn peek_chain_op(&self) -> Option<(ChainOp, usize)> {
        match self.kind() {
            Some(TokKind::LinkOp(op)) => Some((ChainOp::Wire(*op), 1)),
            Some(TokKind::DrawOp(d)) => Some((ChainOp::Measure(*d), 1)),
            Some(TokKind::Pipe) if self.pipes_glued_at(0) => Some((ChainOp::Mate, 2)),
            _ => None,
        }
    }

    pub(super) fn expect_chain_op(&mut self) -> Result<ChainOp, Error> {
        match self.peek_chain_op() {
            Some((op, width)) => {
                self.pos += width;
                Ok(op)
            }
            None => Err(self.err("expected a link operator")),
        }
    }
}

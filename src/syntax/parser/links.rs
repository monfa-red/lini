//! Links: endpoint chains, fan groups, and chain operators [SPEC 9].

use super::*;
use crate::error::Code;

impl<'a> Parser<'a> {
    // ───────────────────────── Links ─────────────────────────

    pub(super) fn parse_link(&mut self) -> Result<Link, Error> {
        let start = self.span();
        let mut chain = vec![self.parse_endpoint_group()?];
        let op = self.expect_chain_op()?;
        let mut ops = vec![op];
        // A statement may be one-ended — a leader or a unary measure toward its
        // text [SPEC 15.6/21]: after the op, an ident or a `|` (bars: a capsule,
        // [SPEC 9/22]) opens an endpoint; anything else is the tail. Which ops
        // (and scopes) allow it is resolve's call.
        if matches!(self.kind(), Some(TokKind::Ident(_)) | Some(TokKind::Pipe)) {
            chain.push(self.parse_endpoint_group()?);
            while let Some((next, width)) = self.peek_chain_op() {
                // Wire hops may each carry their own op — `a - b -> c` is the
                // bare-first-hop spelling [SPEC 9]; mixing operator *kinds*
                // (or measure/mate ops) in one chain stays a parse error.
                let compatible = match (op, next) {
                    (ChainOp::Wire(_), ChainOp::Wire(_)) => true,
                    _ => next == op,
                };
                if !compatible {
                    return Err(self.err(format!(
                        "link chain mixes operators '{}' and '{}'",
                        op.spelling(),
                        next.spelling()
                    )));
                }
                self.pos += width;
                if !matches!(self.kind(), Some(TokKind::Ident(_)) | Some(TokKind::Pipe)) {
                    return Err(self.err("a text callout ends its statement — chain before it"));
                }
                ops.push(next);
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
            ops,
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
        // An endpoint opens with a bare id or an identity capsule [SPEC 9]:
        // `|type#id|` bars, exactly as a declaration writes them — desugar
        // hoists the declaration and rewrites the reference. The rest of the
        // anatomy (`.path`, `.index`, `:point`) composes after either head.
        let start = self.span();
        let (capsule, mut path, mut end) = if matches!(self.kind(), Some(TokKind::Pipe)) {
            let (ty, id) = self.parse_identity(BarsCtx::Instance)?;
            let span = self.span_from(start);
            (Some(Capsule { ty, id, span }), Vec::new(), span)
        } else {
            let (first, first_span) = self.expect_ident()?;
            (None, vec![first], first_span)
        };
        let mut copy = None;
        while matches!(self.kind(), Some(TokKind::Dot)) && self.glued_at(0) {
            self.pos += 1; // '.'
            if !self.glued_at(0) {
                return Err(self.err("endpoint '.' must have no whitespace after it"));
            }
            // A numeric segment is the 1-based pattern-copy index
            // [SPEC 15.4/21] — it ends the path (only `:point` may follow).
            if let Some(TokKind::Number(n)) = self.kind() {
                let (n, span) = (*n, self.span());
                copy = Some(n as usize);
                end = span;
                self.pos += 1;
                if matches!(self.kind(), Some(TokKind::Dot)) && self.glued_at(0) {
                    return Err(self.err("the copy index ends an endpoint path"));
                }
                break;
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
            capsule,
            from_capsule: None,
            path,
            copy,
            point,
            span: Span::new(start.start, end.end),
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
            None => Err(self
                .err("expected a link operator")
                .code(Code::EXPECTED_TOKEN)),
        }
    }
}

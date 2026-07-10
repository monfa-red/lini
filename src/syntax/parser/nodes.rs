//! Nodes and text nodes: identity tail, style blocks, and child bodies.

use super::*;

impl<'a> Parser<'a> {
    // ───────────────────────── Nodes ─────────────────────────

    /// A drawn child [SPEC 3]: a bare string is a text node; anything else is a
    /// box.
    pub(super) fn parse_child(&mut self) -> Result<Child, Error> {
        if matches!(self.kind(), Some(TokKind::String(_))) {
            Ok(Child::Text(self.parse_text_node()?))
        } else {
            Ok(Child::Box(self.parse_node()?))
        }
    }

    /// A text node `"…"` with an optional `{ … }` style block [SPEC 3] — a `{`
    /// glued-or-spaced right after the string is its own text style; otherwise it
    /// is bare. (Strings are self-delimiting, so a following `"` is the next node.)
    pub(super) fn parse_text_node(&mut self) -> Result<TextNode, Error> {
        let text = match self.kind() {
            Some(TokKind::String(s)) => s.clone(),
            _ => return Err(self.err("expected a string")),
        };
        let start = self.span();
        self.pos += 1;
        let (style, style_span) = self.opt_style()?;
        Ok(TextNode {
            text,
            style,
            style_span,
            span: self.span_from(start),
        })
    }

    /// A drawn box [SPEC 3]: identity in the bars, then the shared tail (head
    /// label, classes, style), then the `[ ]` children. The smart label rides
    /// `Node.label` and is lowered per type at desugar.
    pub(super) fn parse_node(&mut self) -> Result<Node, Error> {
        let start = self.span();
        let (ty, id) = self.parse_identity(BarsCtx::Instance)?;
        let Tail {
            label,
            classes,
            style,
            style_span,
        } = self.parse_tail()?;
        let (children, links) = self.opt_children()?;
        Ok(Node {
            id,
            ty,
            label,
            classes,
            style,
            style_span,
            children,
            links,
            span: self.span_from(start),
        })
    }

    /// The class slot — `.name` worn by a node after its type or by a link after
    /// its endpoints [SPEC 3/9]. A `.` glued to an id or endpoint is a path and
    /// never reaches here; what does is the worn-class chain, written `.hot.loud`.
    pub(super) fn parse_classes(&mut self) -> Result<Vec<String>, Error> {
        let mut classes = Vec::new();
        while self.eat(&TokKind::Dot) {
            classes.push(self.expect_ident()?.0);
        }
        Ok(classes)
    }

    /// Consume an optional `{ }` style block; absent → no decls, no span.
    pub(super) fn opt_style(&mut self) -> Result<(Vec<Decl>, Option<Span>), Error> {
        if matches!(self.kind(), Some(TokKind::LBrace)) {
            let (decls, span) = self.parse_style()?;
            Ok((decls, Some(span)))
        } else {
            Ok((Vec::new(), None))
        }
    }

    /// `{ decls }` — declarations only. The span covers `{ … }`, for the formatter.
    pub(super) fn parse_style(&mut self) -> Result<(Vec<Decl>, Span), Error> {
        let start = self.span();
        self.expect(&TokKind::LBrace, "'{'")?;
        self.skip_newlines();
        let mut decls = Vec::new();
        while !matches!(self.kind(), Some(TokKind::RBrace) | None) {
            if matches!(self.kind(), Some(TokKind::Ident(_)))
                && matches!(self.kind_at(1), Some(TokKind::Colon))
            {
                decls.push(self.parse_decl()?);
            } else {
                return Err(self.err("a '{ }' style block holds only declarations"));
            }
            self.terminator()?;
        }
        self.expect(&TokKind::RBrace, "'}'")?;
        Ok((decls, self.span_from(start)))
    }

    /// Consume an optional `[ children ]` block; absent → empty.
    pub(super) fn opt_children(&mut self) -> Result<(Vec<Child>, Vec<Link>), Error> {
        if matches!(self.kind(), Some(TokKind::LBracket)) {
            self.parse_children()
        } else {
            Ok((Vec::new(), Vec::new()))
        }
    }

    /// `[ children and internal links, in source order ]` [SPEC 3]. They stay in
    /// two lists, each in source order; the interleave is recovered from spans
    /// where it matters (the formatter, the `layout: sequence` time axis).
    pub(super) fn parse_children(&mut self) -> Result<(Vec<Child>, Vec<Link>), Error> {
        self.expect(&TokKind::LBracket, "'['")?;
        self.skip_newlines();
        let mut children = Vec::new();
        let mut links = Vec::new();
        while !matches!(self.kind(), Some(TokKind::RBracket) | None) {
            match self.classify_body() {
                Kind::Node => children.push(self.parse_child()?),
                Kind::Link => links.push(self.parse_link()?),
                Kind::Decl => return Err(self.err("declarations go in '{ }', not '[ ]'")),
                Kind::Var => {
                    return Err(self.err("variables are declared in the stylesheet '{ }'"));
                }
                _ => {
                    return Err(self.err(
                        "a node leads with bars — write '|box#X|' (a bare name is a link endpoint)",
                    ));
                }
            }
            self.terminator()?;
        }
        self.expect(&TokKind::RBracket, "']'")?;
        Ok((children, links))
    }

    /// The shared head tail after the bars (a node) or the endpoints (a link),
    /// [SPEC 3/9]: an optional one-string **head label**, then worn **classes**,
    /// then the head's own **style** block — in that order. The `[ ]` content
    /// (children for a node, labels for a link) is parsed by the caller. The head
    /// label carries no style of its own — a `{ }` after it is the node's / link's
    /// block — so a styled label rides the `[ ]` (`[ "…" { … } ]`).
    pub(super) fn parse_tail(&mut self) -> Result<Tail, Error> {
        let label = if let Some(TokKind::String(s)) = self.kind() {
            let text = s.clone();
            let span = self.span();
            self.pos += 1;
            if matches!(self.kind(), Some(TokKind::String(_))) {
                return Err(self.err("one inline label — put two or more in a '[ ]'"));
            }
            Some(TextNode {
                text,
                style: Vec::new(),
                style_span: None,
                span,
            })
        } else {
            None
        };
        let classes = self.parse_classes()?;
        let (style, style_span) = self.opt_style()?;
        // The label / class slots precede the style block (label → classes →
        // style); a string or `.` after it is out of order.
        if matches!(self.kind(), Some(TokKind::String(_))) {
            return Err(self.err("a label comes before classes — write '|box| \"X\" .hot'"));
        }
        if matches!(self.kind(), Some(TokKind::Dot)) {
            return Err(self.err("a class comes before the style block — write '|box| .hot { … }'"));
        }
        Ok(Tail {
            label,
            classes,
            style,
            style_span,
        })
    }

    /// A link's `[ "label"… ]` content block [SPEC 9] — labels are styleable
    /// text leaves, newline-separated; the canonical form of the trailing sugar.
    pub(super) fn parse_label_block(&mut self) -> Result<Vec<TextNode>, Error> {
        self.expect(&TokKind::LBracket, "'['")?;
        self.skip_newlines();
        let mut labels = Vec::new();
        while matches!(self.kind(), Some(TokKind::String(_))) {
            labels.push(self.parse_text_node()?);
            self.skip_newlines();
        }
        if !matches!(self.kind(), Some(TokKind::RBracket)) {
            return Err(self.err("a link's '[ ]' holds only labels (text)"));
        }
        self.pos += 1; // ']'
        Ok(labels)
    }
}

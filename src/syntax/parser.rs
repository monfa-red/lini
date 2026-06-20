//! The parser — single-pass recursive descent over the grammar in SPEC §16.
//!
//! The bracket-and-bars vocabulary makes one token of lookahead enough, with no
//! type-set prescan: `{` opens style, `[` opens children, `|…|` is a type. The
//! file is three phases — an optional leading `{ }` stylesheet, then the canvas
//! instances, then the wires — and a body nests the same idea (a `{ }`, then a
//! `[ ]` of children and internal wires).

use super::ast::*;
use crate::ast::{LineStyle, Side, WireMarker, WireOp};
use crate::error::Error;
use crate::lexer::{TokKind, Token};
use crate::span::Span;

/// Parse a token stream into a [`File`].
pub fn parse(tokens: &[Token]) -> Result<File, Error> {
    Parser::new(tokens).parse_file()
}

/// What a statement at the cursor is.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Node,
    Wire,
    Decl,
    Var,
    Rule,
    Define,
    Unknown,
}

struct Parser<'a> {
    toks: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(toks: &'a [Token]) -> Self {
        Self { toks, pos: 0 }
    }

    // ───────────────────────── Cursor ─────────────────────────

    fn kind(&self) -> Option<&TokKind> {
        self.toks.get(self.pos).map(|t| &t.kind)
    }

    fn kind_at(&self, n: usize) -> Option<&TokKind> {
        self.toks.get(self.pos + n).map(|t| &t.kind)
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
            Err(self.err(format!("expected {}", what)))
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
            _ => return Err(self.err("expected identifier")),
        };
        let span = self.span();
        self.pos += 1;
        Ok((name, span))
    }

    /// Consume a statement terminator (newline / `;`), or accept a closing
    /// bracket / a following string / EOF. A string is self-delimiting, so
    /// `"a" "b" "c"` is three text nodes (SPEC §3).
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
            Err(self.err("expected a newline, ';', or a closing bracket"))
        }
    }

    // ───────────────────────── Classification ─────────────────────────

    /// A stylesheet item: a declaration, a `--var`, a rule (incl. `.class` and
    /// `-> {}`), or a define (`|name::base|`). Assumes newlines skipped.
    fn classify_setup(&self) -> Result<Kind, Error> {
        match self.kind() {
            Some(TokKind::RawCssVar(_)) => Ok(Kind::Var),
            Some(TokKind::Dot) => Ok(Kind::Rule), // .class { … }
            Some(TokKind::WireOp(_)) => Ok(Kind::Rule), // -> { … } wire defaults
            Some(TokKind::Pipe) => Ok(
                // `|name::base|` is a define; any other `|…|` is a rule selector.
                if matches!(self.kind_at(1), Some(TokKind::Ident(_)))
                    && matches!(self.kind_at(2), Some(TokKind::DColon))
                {
                    Kind::Define
                } else {
                    Kind::Rule
                },
            ),
            Some(TokKind::Ident(_)) => {
                match self.kind_at(1) {
                    Some(TokKind::Colon) => Ok(Kind::Decl),
                    _ => Err(self
                        .err("a type only appears in bars — write '|box| { }' to style every box")),
                }
            }
            _ => Err(self.err("the stylesheet holds declarations, rules, and defines")),
        }
    }

    /// A canvas / body statement: a node, a wire, or — flagged for a context
    /// error — a stray declaration or `--var`. Assumes newlines skipped.
    fn classify_body(&self) -> Kind {
        match self.kind() {
            Some(TokKind::Pipe) | Some(TokKind::String(_)) => Kind::Node,
            Some(TokKind::RawCssVar(_)) => Kind::Var,
            Some(TokKind::Ident(_)) => match self.kind_at(1) {
                Some(TokKind::Colon) => Kind::Decl,
                Some(TokKind::WireOp(_)) | Some(TokKind::Amp) => Kind::Wire,
                Some(TokKind::Dot) if self.glued_at(1) => Kind::Wire, // a.b endpoint path
                _ => Kind::Node,
            },
            _ => Kind::Unknown,
        }
    }

    // ───────────────────────── File ─────────────────────────

    fn parse_file(&mut self) -> Result<File, Error> {
        let mut file = File {
            stylesheet: Vec::new(),
            stylesheet_span: Span::default(),
            instances: Vec::new(),
            wires: Vec::new(),
        };
        self.skip_newlines();
        if matches!(self.kind(), Some(TokKind::LBrace)) {
            let start = self.span();
            file.stylesheet = self.parse_stylesheet()?;
            file.stylesheet_span = Span::new(start.start, self.last_span().end);
            self.skip_newlines();
        }
        let mut in_wires = false;
        while self.kind().is_some() {
            match self.classify_body() {
                Kind::Node => {
                    if in_wires {
                        return Err(self.err("instances must come before wires"));
                    }
                    file.instances.push(self.parse_child()?);
                }
                Kind::Wire => {
                    in_wires = true;
                    file.wires.push(self.parse_wire()?);
                }
                Kind::Decl => return Err(self.err("a declaration belongs in a '{ }' block")),
                Kind::Var => {
                    return Err(self.err("variables are declared in the stylesheet '{ }'"));
                }
                _ if matches!(self.kind(), Some(TokKind::LBrace)) => {
                    return Err(
                        self.err("the stylesheet '{ }' must come first, before any instance")
                    );
                }
                _ => return Err(self.err("a node needs an id or a type")),
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
                _ => unreachable!(),
            };
            items.push(item);
            self.terminator()?;
        }
        self.expect(&TokKind::RBrace, "'}'")?;
        Ok(items)
    }

    // ───────────────────────── Declarations ─────────────────────────

    /// `key: v…, v…` — the name token is an `Ident`.
    fn parse_decl(&mut self) -> Result<Decl, Error> {
        let (name, start) = self.expect_ident()?;
        if !self.eat(&TokKind::Colon) {
            return Err(self.err(format!("expected ':' after '{}'", name)));
        }
        let (groups, end) = self.parse_values()?;
        Ok(Decl {
            name,
            groups,
            span: Span::new(start.start, end.end),
        })
    }

    /// `--name: v…` — the name token is a `RawCssVar` (name stored without `--`).
    fn parse_var(&mut self) -> Result<Decl, Error> {
        let start = self.span();
        let name = match self.kind() {
            Some(TokKind::RawCssVar(s)) => s.clone(),
            _ => return Err(self.err("expected '--name'")),
        };
        self.pos += 1;
        if !self.eat(&TokKind::Colon) {
            return Err(self.err(format!("expected ':' after '--{}'", name)));
        }
        let (groups, end) = self.parse_values()?;
        Ok(Decl {
            name,
            groups,
            span: Span::new(start.start, end.end),
        })
    }

    /// Comma-separated value groups; each group is a space-separated sequence.
    fn parse_values(&mut self) -> Result<(Vec<Vec<Value>>, Span), Error> {
        let start = self.span();
        let mut groups: Vec<Vec<Value>> = Vec::new();
        let mut current: Vec<Value> = Vec::new();
        loop {
            if matches!(
                self.kind(),
                Some(TokKind::Newline)
                    | Some(TokKind::Semi)
                    | Some(TokKind::RBrace)
                    | Some(TokKind::RBracket)
                    | None
            ) {
                break;
            } else if matches!(self.kind(), Some(TokKind::Comma)) {
                self.pos += 1;
                groups.push(std::mem::take(&mut current));
            } else {
                current.push(self.parse_value()?);
            }
        }
        groups.push(current);
        if groups.iter().all(|g| g.is_empty()) {
            return Err(self.err("declaration needs a value"));
        }
        let end = self.last_span();
        Ok((groups, Span::new(start.start, end.end)))
    }

    fn parse_value(&mut self) -> Result<Value, Error> {
        // An ident may begin a call (`rgb(…)`, `repeat(…)`); handle separately.
        if matches!(self.kind(), Some(TokKind::Ident(_))) {
            let (name, _) = self.expect_ident()?;
            return if matches!(self.kind(), Some(TokKind::LParen)) {
                self.parse_call(name)
            } else {
                Ok(Value::Ident(name))
            };
        }
        let v = match self.kind() {
            Some(TokKind::Number(n)) => Value::Number(*n),
            Some(TokKind::String(s)) => Value::String(s.clone()),
            Some(TokKind::Hex(h)) => Value::Hex(h.clone()),
            Some(TokKind::RawCssVar(s)) => Value::Var(s.clone()),
            _ => return Err(self.err("expected a value")),
        };
        self.pos += 1;
        Ok(v)
    }

    fn parse_call(&mut self, name: String) -> Result<Value, Error> {
        self.pos += 1; // '('
        let mut args = Vec::new();
        if !matches!(self.kind(), Some(TokKind::RParen)) {
            args.push(self.parse_value()?);
            while self.eat(&TokKind::Comma) {
                args.push(self.parse_value()?);
            }
        }
        if !self.eat(&TokKind::RParen) {
            return Err(self.err("expected ')'"));
        }
        Ok(Value::Call(Call { name, args }))
    }

    // ───────────────────────── Rules & defines ─────────────────────────

    /// `|selector| { decls }`, `.class { decls }`, or `-> { decls }`.
    fn parse_rule(&mut self) -> Result<Rule, Error> {
        let start = self.span();

        // `-> { … }` — the routing layer's element selector (the wire glyph). It
        // carries the reserved `wire` selector internally, so the cascade and
        // renderer treat it like the old `wire { }`.
        if let Some(TokKind::WireOp(op)) = self.kind() {
            let op = *op;
            if op.line != LineStyle::Solid
                || op.start != WireMarker::None
                || op.end != WireMarker::Arrow
            {
                return Err(self.err("wire defaults are set with the '-> { … }' rule"));
            }
            self.pos += 1;
            let decls = self.parse_style()?;
            return Ok(Rule {
                selector: Selector {
                    parts: vec![SelPart::Type("wire".into())],
                },
                decls,
                span: Span::new(start.start, self.last_span().end),
            });
        }

        // `.class { … }` — a bare class definition.
        if self.eat(&TokKind::Dot) {
            let (name, _) = self.expect_ident()?;
            let decls = self.parse_style()?;
            return Ok(Rule {
                selector: Selector {
                    parts: vec![SelPart::Class(name)],
                },
                decls,
                span: Span::new(start.start, self.last_span().end),
            });
        }

        // `|selector| { … }` — a bar-wrapped element / descendant / class selector.
        self.expect(&TokKind::Pipe, "'|'")?;
        let mut parts = Vec::new();
        loop {
            match self.kind() {
                Some(TokKind::Ident(_)) => {
                    let (name, _) = self.expect_ident()?;
                    // A glued `type.class` compound is an instance form, not a
                    // selector (SPEC §4).
                    if matches!(self.kind(), Some(TokKind::Dot)) && self.glued_at(0) {
                        return Err(self.err(
                            "a selector part can't glue a type and a class — space them (descendant) or style '.hot'",
                        ));
                    }
                    parts.push(SelPart::Type(name));
                }
                Some(TokKind::Dot) => {
                    self.pos += 1;
                    parts.push(SelPart::Class(self.expect_ident()?.0));
                }
                Some(TokKind::Pipe) => break,
                _ => return Err(self.err("expected a selector part or '|'")),
            }
        }
        self.expect(&TokKind::Pipe, "'|'")?;
        if parts.is_empty() {
            return Err(self.err("a rule needs a selector"));
        }
        let decls = self.parse_style()?;
        Ok(Rule {
            selector: Selector { parts },
            decls,
            span: Span::new(start.start, self.last_span().end),
        })
    }

    /// `|name::base| { style } [ children ]`.
    fn parse_define(&mut self) -> Result<Define, Error> {
        let start = self.span();
        self.expect(&TokKind::Pipe, "'|'")?;
        let (name, _) = self.expect_ident()?;
        self.expect(&TokKind::DColon, "'::'")?;
        let (base, _) = self.expect_ident()?;
        self.expect(&TokKind::Pipe, "'|'")?;
        let style = self.opt_style()?;
        let (children, wires) = self.opt_children()?;
        Ok(Define {
            name,
            base,
            style,
            children,
            wires,
            span: Span::new(start.start, self.last_span().end),
        })
    }

    // ───────────────────────── Nodes ─────────────────────────

    /// A drawn child (SPEC §3): a bare string is a text node; anything else is a
    /// box.
    fn parse_child(&mut self) -> Result<Child, Error> {
        if let Some(TokKind::String(s)) = self.kind() {
            let text = s.clone();
            let span = self.span();
            self.pos += 1;
            Ok(Child::Text(TextNode { text, span }))
        } else {
            Ok(Child::Box(self.parse_node()?))
        }
    }

    fn parse_node(&mut self) -> Result<Node, Error> {
        let start = self.span();
        let id = if matches!(self.kind(), Some(TokKind::Ident(_))) {
            Some(self.expect_ident()?.0)
        } else {
            None
        };
        let (ty, classes) = if matches!(self.kind(), Some(TokKind::Pipe)) {
            self.parse_typeref()?
        } else {
            (None, Vec::new())
        };
        let style = self.opt_style()?;

        // Content is the `[ ]` children block, or the trailing-label sugar — never
        // both. A stray `.class` here is the floating-class mistake (SPEC §4).
        let (children, wires) = if matches!(self.kind(), Some(TokKind::LBracket)) {
            self.opt_children()?
        } else {
            let labels = self.trailing_labels();
            if matches!(self.kind(), Some(TokKind::LBracket)) {
                return Err(
                    self.err("a node's content is the trailing label or the '[ ]', not both")
                );
            }
            (labels.into_iter().map(Child::Text).collect(), Vec::new())
        };
        if matches!(self.kind(), Some(TokKind::Dot)) {
            return Err(self.err("a node wears its class in the bars — write '|box.hot|'"));
        }
        if id.is_none() && ty.is_none() {
            return Err(self.err("a node needs an id or a type"));
        }
        Ok(Node {
            id,
            ty,
            classes,
            style,
            children,
            wires,
            span: Span::new(start.start, self.last_span().end),
        })
    }

    /// `|type|`, `|type.class.class|`, or `|.class…|` (default `box`). Inside the
    /// bars is the node's type and worn classes (SPEC §1/§4).
    fn parse_typeref(&mut self) -> Result<(Option<String>, Vec<String>), Error> {
        self.expect(&TokKind::Pipe, "'|'")?;
        let mut ty = None;
        if matches!(self.kind(), Some(TokKind::Ident(_))) {
            let (name, _) = self.expect_ident()?;
            if matches!(self.kind(), Some(TokKind::DColon)) {
                return Err(self.err("a define belongs in the stylesheet"));
            }
            if name == "wire" {
                return Err(self.err("wires are drawn by operators, not the '|wire|' type"));
            }
            ty = Some(name);
        }
        let mut classes = Vec::new();
        while self.eat(&TokKind::Dot) {
            classes.push(self.expect_ident()?.0);
        }
        self.expect(&TokKind::Pipe, "'|'")?;
        if ty.is_none() && classes.is_empty() {
            return Err(self.err("empty '||' — name a type or a class"));
        }
        Ok((ty, classes))
    }

    /// Consume an optional `{ }` style block; an absent one is an empty decl list.
    fn opt_style(&mut self) -> Result<Vec<Decl>, Error> {
        if matches!(self.kind(), Some(TokKind::LBrace)) {
            self.parse_style()
        } else {
            Ok(Vec::new())
        }
    }

    /// `{ decls }` — declarations only.
    fn parse_style(&mut self) -> Result<Vec<Decl>, Error> {
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
        Ok(decls)
    }

    /// Consume an optional `[ children ]` block; absent → empty.
    fn opt_children(&mut self) -> Result<(Vec<Child>, Vec<Wire>), Error> {
        if matches!(self.kind(), Some(TokKind::LBracket)) {
            self.parse_children()
        } else {
            Ok((Vec::new(), Vec::new()))
        }
    }

    /// `[ children, then internal wires ]` (SPEC §3).
    fn parse_children(&mut self) -> Result<(Vec<Child>, Vec<Wire>), Error> {
        self.expect(&TokKind::LBracket, "'['")?;
        self.skip_newlines();
        let mut children = Vec::new();
        let mut wires = Vec::new();
        while !matches!(self.kind(), Some(TokKind::RBracket) | None) {
            match self.classify_body() {
                Kind::Node => {
                    if !wires.is_empty() {
                        return Err(self.err("a child must come before the body's wires"));
                    }
                    children.push(self.parse_child()?);
                }
                Kind::Wire => wires.push(self.parse_wire()?),
                Kind::Decl => return Err(self.err("declarations go in '{ }', not '[ ]'")),
                _ => return Err(self.err("a child needs an id, a type, or text")),
            }
            self.terminator()?;
        }
        self.expect(&TokKind::RBracket, "']'")?;
        Ok((children, wires))
    }

    /// Consume the trailing label string(s) after a box or wire head (SPEC §3/§9).
    /// The loop ends at the line's end, so the labels run to it.
    fn trailing_labels(&mut self) -> Vec<TextNode> {
        let mut labels = Vec::new();
        while let Some(TokKind::String(s)) = self.kind() {
            let text = s.clone();
            let span = self.span();
            self.pos += 1;
            labels.push(TextNode { text, span });
        }
        labels
    }

    // ───────────────────────── Wires ─────────────────────────

    fn parse_wire(&mut self) -> Result<Wire, Error> {
        let start = self.span();
        let mut chain = vec![self.parse_endpoint_group()?];
        let op = self.expect_wire_op()?;
        chain.push(self.parse_endpoint_group()?);
        while let Some(next) = self.peek_wire_op() {
            if next != op {
                return Err(self.err(format!(
                    "wire chain mixes operators '{}' and '{}'",
                    wire_op_str(op),
                    wire_op_str(next)
                )));
            }
            self.pos += 1;
            chain.push(self.parse_endpoint_group()?);
        }
        // Trailing class(es): a wire has no bars, so its class stands alone.
        let mut classes = Vec::new();
        while self.eat(&TokKind::Dot) {
            classes.push(self.expect_ident()?.0);
        }
        let style = self.opt_style()?;
        if matches!(self.kind(), Some(TokKind::LBracket)) {
            return Err(
                self.err("a wire is not a container — it carries trailing labels, not a '[ ]'")
            );
        }
        let labels = self.trailing_labels();
        Ok(Wire {
            chain,
            op,
            classes,
            style,
            labels,
            span: Span::new(start.start, self.last_span().end),
        })
    }

    fn parse_endpoint_group(&mut self) -> Result<EndpointGroup, Error> {
        let mut endpoints = vec![self.parse_endpoint()?];
        while self.eat(&TokKind::Amp) {
            endpoints.push(self.parse_endpoint()?);
        }
        Ok(EndpointGroup { endpoints })
    }

    fn parse_endpoint(&mut self) -> Result<Endpoint, Error> {
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
        let side = if path.len() > 1 {
            match Side::parse(path.last().unwrap()) {
                Some(s) => {
                    path.pop();
                    Some(s)
                }
                None => None,
            }
        } else {
            None
        };
        Ok(Endpoint {
            path,
            side,
            span: Span::new(first_span.start, end.end),
        })
    }

    /// The wire op at the cursor as an owned copy, so a `while let` over it
    /// doesn't hold a borrow of `self` across the loop body.
    fn peek_wire_op(&self) -> Option<WireOp> {
        match self.kind() {
            Some(TokKind::WireOp(op)) => Some(*op),
            _ => None,
        }
    }

    fn expect_wire_op(&mut self) -> Result<WireOp, Error> {
        match self.peek_wire_op() {
            Some(op) => {
                self.pos += 1;
                Ok(op)
            }
            None => Err(self.err("expected a wire operator")),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(src: &str) -> File {
        let tokens = crate::lexer::lex(src).expect("lex");
        parse(&tokens).expect("parse")
    }

    fn parse_err(src: &str) -> String {
        let tokens = crate::lexer::lex(src).expect("lex");
        match parse(&tokens) {
            Ok(_) => panic!("expected a parse error for: {src}"),
            Err(e) => e.message,
        }
    }

    /// The i-th top-level instance as a box, panicking if it is bare text.
    fn instance(f: &File, i: usize) -> &Node {
        match &f.instances[i] {
            Child::Box(n) => n,
            Child::Text(_) => panic!("instance {i} is text, not a box"),
        }
    }

    #[test]
    fn quickstart_three_box_chain() {
        let f = parse_ok("cat -> dog -> bird\n");
        assert!(f.stylesheet.is_empty() && f.instances.is_empty());
        assert_eq!(f.wires.len(), 1);
        assert_eq!(f.wires[0].chain.len(), 3);
    }

    #[test]
    fn three_phases() {
        let f = parse_ok(
            "{\n  layout: grid;\n  |box| { radius: 6; }\n  .hot { stroke-width: 2; }\n}\n\
             server |box|\nclient |box|\nserver -> client \"requests\"\n",
        );
        assert_eq!(f.stylesheet.len(), 3); // root decl, element rule, class rule
        assert_eq!(f.instances.len(), 2);
        assert_eq!(f.wires.len(), 1);
    }

    #[test]
    fn stylesheet_is_optional() {
        let f = parse_ok("server |box|\nserver -> server\n");
        assert!(f.stylesheet.is_empty());
        assert_eq!(f.instances.len(), 1);
    }

    #[test]
    fn element_rule_and_define_in_stylesheet() {
        let f =
            parse_ok("{\n  |box| { radius: 4; }\n  |treat::box| { radius: 5; }\n}\nx |treat|\n");
        assert!(matches!(f.stylesheet[0], StyleItem::Rule(_)));
        match &f.stylesheet[1] {
            StyleItem::Define(d) => {
                assert_eq!(d.name, "treat");
                assert_eq!(d.base, "box");
            }
            _ => panic!("expected a define"),
        }
        assert_eq!(instance(&f, 0).ty.as_deref(), Some("treat"));
    }

    #[test]
    fn descendant_selector_in_bars() {
        let f = parse_ok(
            "{\n  |table box| { stroke-width: 0; }\n  |.sidebar box| { fill: gray; }\n}\n",
        );
        match &f.stylesheet[0] {
            StyleItem::Rule(r) => assert_eq!(r.selector.parts.len(), 2),
            _ => panic!(),
        }
        match &f.stylesheet[1] {
            StyleItem::Rule(r) => assert!(matches!(r.selector.parts[0], SelPart::Class(_))),
            _ => panic!(),
        }
    }

    #[test]
    fn define_with_intrinsic_children() {
        let f = parse_ok(
            "{\n  |room::group| {\n    gap: 4;\n  } [\n    inlet |box|\n    outlet |box|\n    inlet -> outlet\n  ]\n}\n",
        );
        match &f.stylesheet[0] {
            StyleItem::Define(d) => {
                assert_eq!(d.children.len(), 2);
                assert_eq!(d.wires.len(), 1);
                assert_eq!(d.style.len(), 1);
            }
            _ => panic!("expected a define"),
        }
    }

    #[test]
    fn node_with_id_type_classes_style_and_children() {
        let f = parse_ok(
            "db |cyl.primary| { fill: #eef } [\n  \"Postgres\"\n  tag |badge| { pin: top right } \"v16\"\n]\n",
        );
        let n = instance(&f, 0);
        assert_eq!(n.id.as_deref(), Some("db"));
        assert_eq!(n.ty.as_deref(), Some("cyl"));
        assert_eq!(n.classes, vec!["primary"]);
        assert_eq!(n.style.len(), 1);
        assert_eq!(n.children.len(), 2); // text "Postgres", then the tag box
        assert!(matches!(&n.children[0], Child::Text(t) if t.text == "Postgres"));
        assert!(matches!(&n.children[1], Child::Box(b) if b.id.as_deref() == Some("tag")));
    }

    #[test]
    fn class_in_bars_with_default_box() {
        let f = parse_ok("x |.hot|\n");
        let n = instance(&f, 0);
        assert_eq!(n.ty, None);
        assert_eq!(n.classes, vec!["hot"]);
    }

    #[test]
    fn style_block_coexists_with_a_trailing_label() {
        let f = parse_ok("cat |box| { fill: red } \"Cat\"\n");
        let n = instance(&f, 0);
        assert_eq!(n.style.len(), 1);
        assert!(matches!(&n.children[0], Child::Text(t) if t.text == "Cat"));
    }

    #[test]
    fn trailing_label_sugar() {
        let f = parse_ok("cat |box| \"Cat\"\nx |box| \"a\" \"b\"\n");
        assert!(matches!(&instance(&f, 0).children[0], Child::Text(t) if t.text == "Cat"));
        assert_eq!(instance(&f, 1).children.len(), 2);
    }

    #[test]
    fn wire_with_class_style_and_labels() {
        let f = parse_ok("a -> b .loud { along: 0.3 0.7; stroke: red } \"near a\" \"near b\"\n");
        let w = &f.wires[0];
        assert_eq!(w.classes, vec!["loud"]);
        assert_eq!(w.style.len(), 2);
        assert_eq!(w.labels.len(), 2);
        assert_eq!(w.labels[0].text, "near a");
    }

    #[test]
    fn wire_trails_its_label() {
        let f = parse_ok("a -> b \"x\" \"y\"\n");
        let w = &f.wires[0];
        assert_eq!(w.labels.len(), 2);
        assert_eq!(w.labels[0].text, "x");
    }

    #[test]
    fn forced_side_endpoint() {
        let f = parse_ok("cat.right -> kitchen.bowl.left\n");
        let w = &f.wires[0];
        assert_eq!(w.chain[0].endpoints[0].path, vec!["cat"]);
        assert_eq!(w.chain[0].endpoints[0].side, Some(Side::Right));
        assert_eq!(w.chain[1].endpoints[0].path, vec!["kitchen", "bowl"]);
        assert_eq!(w.chain[1].endpoints[0].side, Some(Side::Left));
    }

    #[test]
    fn fan_and_class_on_wire() {
        let f = parse_ok("a & b -> c & d .loud\n");
        let w = &f.wires[0];
        assert_eq!(w.chain[0].endpoints.len(), 2);
        assert_eq!(w.chain[1].endpoints.len(), 2);
        assert_eq!(w.classes, vec!["loud"]);
    }

    #[test]
    fn wire_defaults_rule() {
        let f = parse_ok("{\n  -> { stroke: #666; }\n}\na -> b\n");
        match &f.stylesheet[0] {
            StyleItem::Rule(r) => {
                assert!(matches!(r.selector.parts[0], SelPart::Type(ref t) if t == "wire"))
            }
            _ => panic!(),
        }
    }

    #[test]
    fn value_groups_space_and_comma() {
        let f = parse_ok("|line| { points: 0 0, 10 10, 20 0; translate: 100 50 }\n");
        let b = instance(&f, 0);
        let points = b.style.iter().find(|d| d.name == "points").unwrap();
        assert_eq!(points.groups.len(), 3);
        assert_eq!(points.groups[0].len(), 2);
    }

    #[test]
    fn call_and_var_values() {
        let f = parse_ok(
            "{\n  columns: repeat(3);\n  --brand: #ff6600;\n}\ncard |box| { fill: --brand; columns: 80 repeat(2, 40) }\n",
        );
        match &f.stylesheet[0] {
            StyleItem::RootDecl(d) => {
                assert!(matches!(&d.groups[0][0], Value::Call(c) if c.name == "repeat"))
            }
            _ => panic!(),
        }
        match &f.stylesheet[1] {
            StyleItem::Var(d) => assert_eq!(d.name, "brand"),
            _ => panic!(),
        }
    }

    #[test]
    fn bare_and_consecutive_strings_are_text_nodes() {
        let f = parse_ok("\"a\" \"b\" \"c\"\n");
        assert_eq!(f.instances.len(), 3);
        assert!(f.instances.iter().all(|c| matches!(c, Child::Text(_))));
    }

    // ── Errors ──

    #[test]
    fn stylesheet_after_instance_errors() {
        assert!(parse_err("x |box|\n{\n  |box| { radius: 4; }\n}\n").contains("must come first"));
    }

    #[test]
    fn instance_after_wire_errors() {
        assert!(parse_err("a -> b\nc |box|\n").contains("must come before wires"));
    }

    #[test]
    fn wire_as_instance_errors() {
        assert!(parse_err("x |wire|\n").contains("drawn by operators"));
    }

    #[test]
    fn mixed_operators_error() {
        assert!(parse_err("a -> b -- c\n").contains("mixes operators"));
    }

    #[test]
    fn wire_with_a_bracket_errors() {
        assert!(parse_err("a -> b [ \"x\" ]\n").contains("not a container"));
    }

    #[test]
    fn floating_node_class_errors() {
        assert!(parse_err("cat |box| .hot\n").contains("in the bars"));
    }

    #[test]
    fn bare_type_rule_errors() {
        assert!(parse_err("{\n  box { radius: 4; }\n}\n").contains("only appears in bars"));
    }

    #[test]
    fn compound_rule_selector_errors() {
        assert!(parse_err("{\n  |box.hot| { fill: red; }\n}\n").contains("can't glue"));
    }

    #[test]
    fn decl_in_children_errors() {
        assert!(parse_err("g |group| [\n  gap: 4\n]\n").contains("go in '{ }'"));
    }

    #[test]
    fn child_after_wire_errors() {
        assert!(
            parse_err("g |group| [\n  a |box|\n  a -> a\n  b |box|\n]\n")
                .contains("before the body's wires")
        );
    }

    #[test]
    fn label_and_children_both_errors() {
        assert!(parse_err("cat |box| \"x\" [ \"y\" ]\n").contains("not both"));
    }

    #[test]
    fn empty_declaration_errors() {
        assert!(parse_err("a |box| { gap: }\n").contains("needs a value"));
    }
}

//! v4 parser (PLAN Phase 2) — single-pass recursive descent over the grammar in
//! SPEC §16. Built alongside the v3 parser; Phase 3 cuts `resolve` over and
//! removes the v3 front end.
//!
//! The three-phase order (stylesheet → instances → wires) plus "a type is
//! defined before it is used" make one token of lookahead enough: the only
//! ambiguous form, `ident { … }`, is a rule when `ident` is a known type and a
//! node otherwise — and the type set is always complete at that point because
//! defines come first.

use super::ast::*;
use crate::ast::{LineStyle, Side, WireMarker, WireOp};
use crate::error::Error;
use crate::lexer::{TokKind, Token};
use crate::span::Span;
use std::collections::HashSet;

/// Parse a v4 token stream into a [`File`].
pub fn parse(tokens: &[Token]) -> Result<File, Error> {
    Parser::new(tokens).parse_file()
}

/// Built-in type names — the reserved primitives and templates (SPEC §18). User
/// defines extend this set as they are parsed. `wire` is deliberately absent:
/// it is reserved but not a type, so `wire { }` reads as a (reserved-id) node,
/// not a rule — wire defaults are the `-> { }` rule.
const BUILTIN_TYPES: &[&str] = &[
    "box", "oval", "line", "path", "poly", "hex", "slant", "cyl", "diamond", "cloud", "icon",
    "image", "plain", "group", "caption", "badge", "note", "row", "column", "table",
];

/// What a statement at the cursor is.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Var,
    Decl,
    Rule,
    Define,
    Node,
    Wire,
}

/// File-level (and in-block) phase, in source order. In a block: `Stylesheet` =
/// declarations, `Instances` = child nodes, `Wires` = internal wires.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Phase {
    Stylesheet,
    Instances,
    Wires,
}

struct Parser<'a> {
    toks: &'a [Token],
    pos: usize,
    types: HashSet<String>,
}

impl<'a> Parser<'a> {
    fn new(toks: &'a [Token]) -> Self {
        Self {
            toks,
            pos: 0,
            types: BUILTIN_TYPES.iter().map(|s| s.to_string()).collect(),
        }
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

    fn skip_newlines(&mut self) {
        while matches!(self.kind(), Some(TokKind::Newline)) {
            self.pos += 1;
        }
    }

    fn err(&self, msg: impl Into<String>) -> Error {
        Error::at(self.span(), msg.into())
    }

    fn expect_pipe(&mut self) -> Result<(), Error> {
        if self.eat(&TokKind::Pipe) {
            Ok(())
        } else {
            Err(self.err("expected '|'"))
        }
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

    fn expect_string(&mut self) -> Result<String, Error> {
        let s = match self.kind() {
            Some(TokKind::String(s)) => s.clone(),
            _ => return Err(self.err("expected string")),
        };
        self.pos += 1;
        Ok(s)
    }

    /// Consume a statement terminator (newline / `;`), or accept `}` / EOF. A
    /// following `string` also ends the statement without a separator: a string
    /// is self-delimiting, so `"a" "b" "c"` is three text nodes (SPEC §3).
    fn terminator(&mut self) -> Result<(), Error> {
        if matches!(self.kind(), Some(TokKind::Newline) | Some(TokKind::Semi)) {
            self.pos += 1;
            self.skip_newlines();
            Ok(())
        } else if matches!(
            self.kind(),
            Some(TokKind::RBrace) | Some(TokKind::String(_)) | None
        ) {
            Ok(())
        } else {
            Err(self.err("expected newline, ';' or '}'"))
        }
    }

    // ───────────────────────── Classification ─────────────────────────

    /// Decide the statement kind at the cursor with at most two tokens of
    /// lookahead plus the type set. Assumes newlines already skipped.
    fn classify(&self) -> Result<Kind, Error> {
        match self.kind() {
            Some(TokKind::RawCssVar(_)) => Ok(Kind::Var),
            Some(TokKind::Dot) => Ok(Kind::Rule), // `.class …` selector
            // `-> { … }` — the wire-defaults rule (the wire glyph as selector).
            Some(TokKind::WireOp(_)) if matches!(self.kind_at(1), Some(TokKind::LBrace)) => {
                Ok(Kind::Rule)
            }
            Some(TokKind::Pipe) | Some(TokKind::String(_)) => Ok(Kind::Node),
            Some(TokKind::Ident(name)) => Ok(match self.kind_at(1) {
                Some(TokKind::DColon) => Kind::Define,
                Some(TokKind::Colon) => Kind::Decl,
                Some(TokKind::Pipe) | Some(TokKind::String(_)) => Kind::Node,
                Some(TokKind::WireOp(_)) | Some(TokKind::Amp) => Kind::Wire,
                Some(TokKind::Dot) if self.glued_at(1) => Kind::Wire, // a.b endpoint path
                Some(TokKind::Dot) | Some(TokKind::Ident(_)) => {
                    // `ident .class` or `ident ident` — a rule iff the lead is a
                    // type, else a node carrying a class.
                    if self.types.contains(name) {
                        Kind::Rule
                    } else {
                        Kind::Node
                    }
                }
                Some(TokKind::LBrace) => {
                    if self.types.contains(name) {
                        Kind::Rule
                    } else {
                        Kind::Node
                    }
                }
                _ => Kind::Node, // bare id (newline / ';' / '}' / EOF)
            }),
            _ => Err(self.err("expected a statement")),
        }
    }

    // ───────────────────────── File ─────────────────────────

    fn parse_file(&mut self) -> Result<File, Error> {
        let mut file = File {
            stylesheet: Vec::new(),
            instances: Vec::new(),
            wires: Vec::new(),
        };
        let mut phase = Phase::Stylesheet;
        self.skip_newlines();
        while self.kind().is_some() {
            match self.classify()? {
                k @ (Kind::Var | Kind::Decl | Kind::Rule | Kind::Define) => {
                    if phase != Phase::Stylesheet {
                        return Err(self.err(
                            "the stylesheet (declarations, rules, defines) must come before instances",
                        ));
                    }
                    file.stylesheet.push(self.parse_style_item(k)?);
                }
                Kind::Node => {
                    if phase > Phase::Instances {
                        return Err(self.err("instances must come before wires"));
                    }
                    phase = Phase::Instances;
                    file.instances.push(self.parse_child()?);
                }
                Kind::Wire => {
                    phase = Phase::Wires;
                    file.wires.push(self.parse_wire()?);
                }
            }
            self.terminator()?;
        }
        Ok(file)
    }

    fn parse_style_item(&mut self, k: Kind) -> Result<StyleItem, Error> {
        Ok(match k {
            Kind::Var => StyleItem::Var(self.parse_var()?),
            Kind::Decl => StyleItem::RootDecl(self.parse_decl()?),
            Kind::Rule => StyleItem::Rule(self.parse_rule()?),
            Kind::Define => StyleItem::Define(self.parse_define()?),
            _ => unreachable!(),
        })
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
                Some(TokKind::Newline) | Some(TokKind::Semi) | Some(TokKind::RBrace) | None
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
            let (name, start) = self.expect_ident()?;
            return if matches!(self.kind(), Some(TokKind::LParen)) {
                self.parse_call(name, start)
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

    fn parse_call(&mut self, name: String, start: Span) -> Result<Value, Error> {
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
        let end = self.last_span();
        Ok(Value::Call(Call {
            name,
            args,
            span: Span::new(start.start, end.end),
        }))
    }

    // ───────────────────────── Rules & defines ─────────────────────────

    fn parse_rule(&mut self) -> Result<Rule, Error> {
        let start = self.span();
        // `-> { … }` is the wire-defaults rule: the wire glyph stands in for the
        // selector. It carries the reserved `wire` element selector internally,
        // so the cascade and renderer treat it exactly like the old `wire { }`.
        if let Some(TokKind::WireOp(op)) = self.kind() {
            let op = *op;
            if op.line != LineStyle::Solid
                || op.start != WireMarker::None
                || op.end != WireMarker::Arrow
            {
                return Err(self.err("wire defaults are set with the '-> { … }' rule"));
            }
            self.pos += 1;
            let decls = self.parse_rule_block()?;
            let span = Span::new(start.start, self.last_span().end);
            return Ok(Rule {
                selector: Selector {
                    parts: vec![SelPart::Type("wire".into())],
                    span,
                },
                decls,
                span,
            });
        }
        let mut parts = Vec::new();
        loop {
            if matches!(self.kind(), Some(TokKind::Ident(_))) {
                parts.push(SelPart::Type(self.expect_ident()?.0));
            } else if matches!(self.kind(), Some(TokKind::Dot)) {
                self.pos += 1;
                parts.push(SelPart::Class(self.expect_ident()?.0));
            } else if matches!(self.kind(), Some(TokKind::LBrace)) {
                break;
            } else {
                return Err(self.err("expected a selector part or '{'"));
            }
        }
        if parts.is_empty() {
            return Err(self.err("a rule needs a selector"));
        }
        let decls = self.parse_rule_block()?;
        let end = self.last_span();
        let span = Span::new(start.start, end.end);
        Ok(Rule {
            selector: Selector { parts, span },
            decls,
            span,
        })
    }

    /// A rule body holds declarations only.
    fn parse_rule_block(&mut self) -> Result<Vec<Decl>, Error> {
        if !self.eat(&TokKind::LBrace) {
            return Err(self.err("expected '{'"));
        }
        self.skip_newlines();
        let mut decls = Vec::new();
        while !matches!(self.kind(), Some(TokKind::RBrace) | None) {
            if !matches!(self.classify()?, Kind::Decl) {
                return Err(self.err("a rule body holds only declarations"));
            }
            decls.push(self.parse_decl()?);
            self.terminator()?;
        }
        if !self.eat(&TokKind::RBrace) {
            return Err(self.err("expected '}'"));
        }
        Ok(decls)
    }

    fn parse_define(&mut self) -> Result<Define, Error> {
        let (name, start) = self.expect_ident()?;
        if !self.eat(&TokKind::DColon) {
            return Err(self.err("expected '::'"));
        }
        let (base, _) = self.expect_ident()?;
        self.types.insert(name.clone());
        let body = self.parse_block()?;
        let end = self.last_span();
        Ok(Define {
            name,
            base,
            body,
            span: Span::new(start.start, end.end),
        })
    }

    // ───────────────────────── Nodes ─────────────────────────

    /// A drawn child (SPEC §3): a bare string is a text node; anything else is a
    /// box. A box's label is a `Child::Text` *inside* its block, never positional.
    fn parse_child(&mut self) -> Result<Child, Error> {
        if matches!(self.kind(), Some(TokKind::String(_))) {
            let span = self.span();
            let text = self.expect_string()?;
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
        let ty = if matches!(self.kind(), Some(TokKind::Pipe)) {
            Some(self.parse_type_use()?)
        } else {
            None
        };
        let mut classes = Vec::new();
        while matches!(self.kind(), Some(TokKind::Dot)) {
            self.pos += 1;
            classes.push(self.expect_ident()?.0);
        }
        let block = if matches!(self.kind(), Some(TokKind::LBrace)) {
            Some(self.parse_block()?)
        } else {
            None
        };
        // A string after the head is a positional label — gone in the box/text
        // model (SPEC §3); the label belongs in the block.
        if matches!(self.kind(), Some(TokKind::String(_))) {
            return Err(
                self.err("a label is a child, not positional — put it in the block: { \"…\" }")
            );
        }
        if id.is_none() && ty.is_none() && block.is_none() {
            return Err(self.err("a node needs an id, type, or block"));
        }
        let end = self.last_span();
        Ok(Node {
            id,
            ty,
            classes,
            block,
            span: Span::new(start.start, end.end),
        })
    }

    fn parse_type_use(&mut self) -> Result<String, Error> {
        self.expect_pipe()?;
        let (name, _) = self.expect_ident()?;
        if name == "wire" {
            return Err(self.err("wires are drawn by operators, not the '|wire|' type"));
        }
        self.expect_pipe()?;
        Ok(name)
    }

    /// A node / define body: declarations, then child nodes, then internal wires.
    fn parse_block(&mut self) -> Result<Block, Error> {
        if !self.eat(&TokKind::LBrace) {
            return Err(self.err("expected '{'"));
        }
        self.skip_newlines();
        let mut block = Block::default();
        let mut phase = Phase::Stylesheet; // Stylesheet = decls, Instances = nodes, Wires = wires
        while !matches!(self.kind(), Some(TokKind::RBrace) | None) {
            match self.classify()? {
                Kind::Decl => {
                    if phase != Phase::Stylesheet {
                        return Err(self.err("declarations must come first in a block"));
                    }
                    block.decls.push(self.parse_decl()?);
                }
                Kind::Node => {
                    if phase > Phase::Instances {
                        return Err(self.err("child nodes must come before internal wires"));
                    }
                    phase = Phase::Instances;
                    block.children.push(self.parse_child()?);
                }
                Kind::Wire => {
                    phase = Phase::Wires;
                    block.wires.push(self.parse_wire()?);
                }
                Kind::Var => return Err(self.err("variables are declared at the top level")),
                Kind::Rule | Kind::Define => {
                    return Err(self.err("rules and defines are top-level only"));
                }
            }
            self.terminator()?;
        }
        if !self.eat(&TokKind::RBrace) {
            return Err(self.err("expected '}'"));
        }
        Ok(block)
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
        let mut classes = Vec::new();
        while matches!(self.kind(), Some(TokKind::Dot)) {
            self.pos += 1;
            classes.push(self.expect_ident()?.0);
        }
        if matches!(self.kind(), Some(TokKind::String(_))) {
            return Err(self.err("a wire label goes in the body: { \"…\" }"));
        }
        let block = if matches!(self.kind(), Some(TokKind::LBrace)) {
            Some(self.parse_wire_block()?)
        } else {
            None
        };
        let end = self.last_span();
        Ok(Wire {
            chain,
            op,
            classes,
            block,
            span: Span::new(start.start, end.end),
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
        let op = self.peek_wire_op();
        match op {
            Some(op) => {
                self.pos += 1;
                Ok(op)
            }
            None => Err(self.err("expected a wire operator")),
        }
    }

    /// A wire body (SPEC §9): declarations (including `along:`) and labels — bare
    /// text, or a `|plain|` box for a styled / offset label.
    fn parse_wire_block(&mut self) -> Result<WireBlock, Error> {
        if !self.eat(&TokKind::LBrace) {
            return Err(self.err("expected '{'"));
        }
        self.skip_newlines();
        let mut wb = WireBlock::default();
        while !matches!(self.kind(), Some(TokKind::RBrace) | None) {
            if matches!(self.kind(), Some(TokKind::String(_)) | Some(TokKind::Pipe)) {
                wb.labels.push(self.parse_child()?);
            } else if matches!(self.classify()?, Kind::Decl) {
                wb.decls.push(self.parse_decl()?);
            } else {
                return Err(self.err("a wire body holds only labels and 'along:'"));
            }
            self.terminator()?;
        }
        if !self.eat(&TokKind::RBrace) {
            return Err(self.err("expected '}'"));
        }
        Ok(wb)
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
            "layout: grid;\nbox { radius: 6; }\n.hot { stroke-width: 2; }\n\
             server |box|\nclient |box|\nserver -> client { \"requests\" }\n",
        );
        assert_eq!(f.stylesheet.len(), 3); // root decl, element rule, class rule
        assert_eq!(f.instances.len(), 2);
        assert_eq!(f.wires.len(), 1);
    }

    #[test]
    fn element_rule_vs_node_by_type_set() {
        let f = parse_ok("box { radius: 4; }\nserver { fill: red; }\n");
        assert!(matches!(f.stylesheet[0], StyleItem::Rule(_)));
        assert_eq!(f.instances.len(), 1);
        assert_eq!(instance(&f, 0).id.as_deref(), Some("server"));
    }

    #[test]
    fn define_then_use() {
        let f = parse_ok("treat::box { radius: 5; }\nx |treat|\n");
        match &f.stylesheet[0] {
            StyleItem::Define(d) => {
                assert_eq!(d.name, "treat");
                assert_eq!(d.base, "box");
            }
            _ => panic!("expected a define"),
        }
        assert_eq!(instance(&f, 0).ty.as_deref(), Some("treat"));
    }

    #[test]
    fn define_whitespace_around_dcolon() {
        let f = parse_ok("panel :: group { gap: 4; }\np |panel|\n");
        assert!(
            matches!(&f.stylesheet[0], StyleItem::Define(d) if d.name == "panel" && d.base == "group")
        );
    }

    #[test]
    fn descendant_selector() {
        let f = parse_ok("table box { stroke-width: 0; }\n.sidebar box { fill: gray; }\n");
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
    fn node_with_id_type_classes_block_and_text() {
        let f = parse_ok(
            "db |cyl| .primary {\n  fill: #eef;\n  \"Postgres\"\n  badge |box| { mount: on; \"v16\" }\n}\n",
        );
        let n = instance(&f, 0);
        assert_eq!(n.id.as_deref(), Some("db"));
        assert_eq!(n.ty.as_deref(), Some("cyl"));
        assert_eq!(n.classes, vec!["primary"]);
        let b = n.block.as_ref().unwrap();
        assert_eq!(b.decls.len(), 1);
        assert_eq!(b.children.len(), 2); // text "Postgres", then the badge box
        assert!(matches!(&b.children[0], Child::Text(t) if t.text == "Postgres"));
        assert!(matches!(&b.children[1], Child::Box(n) if n.id.as_deref() == Some("badge")));
    }

    #[test]
    fn a_positional_label_is_rejected() {
        assert!(parse_err("cat |box| \"Cat\"\n").contains("put it in the block"));
    }

    #[test]
    fn value_groups_space_and_comma() {
        let f = parse_ok("|line| { points: 0 0, 10 10, 20 0; at: 100 50; }\n");
        let b = instance(&f, 0).block.as_ref().unwrap();
        let points = b.decls.iter().find(|d| d.name == "points").unwrap();
        assert_eq!(points.groups.len(), 3);
        assert_eq!(points.groups[0].len(), 2);
        let at = b.decls.iter().find(|d| d.name == "at").unwrap();
        assert_eq!(at.groups.len(), 1);
        assert_eq!(at.groups[0].len(), 2);
    }

    #[test]
    fn call_and_var_values() {
        let f = parse_ok(
            "columns: repeat(3);\n--brand: #ff6600;\ncard |box| { fill: --brand; columns: 80 repeat(2, 40); }\n",
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
    fn wire_block_decls_and_labels() {
        let f = parse_ok(
            "a -> b {\n  along: 0.3 0.7;\n  \"watches\"\n  |plain| { color: red; \"x\" }\n}\n",
        );
        let wb = f.wires[0].block.as_ref().unwrap();
        assert_eq!(wb.decls.len(), 1); // along
        assert_eq!(wb.labels.len(), 2);
        assert!(matches!(&wb.labels[0], Child::Text(t) if t.text == "watches"));
        assert!(matches!(&wb.labels[1], Child::Box(_)));
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
    fn bare_string_is_a_text_node() {
        let f = parse_ok("\"Fruit\"\n");
        assert!(matches!(&f.instances[0], Child::Text(t) if t.text == "Fruit"));
    }

    #[test]
    fn consecutive_strings_are_separate_text_nodes() {
        let f = parse_ok("\"a\" \"b\" \"c\"\n");
        assert_eq!(f.instances.len(), 3);
        assert!(f.instances.iter().all(|c| matches!(c, Child::Text(_))));
    }

    // ── Errors ──

    #[test]
    fn ordering_rule_after_instance() {
        assert!(parse_err("x |box|\nbox { radius: 4; }\n").contains("must come before instances"));
    }

    #[test]
    fn ordering_instance_after_wire() {
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
    fn block_decls_before_children() {
        assert!(
            parse_err("g |group| {\n  a |box|\n  gap: 4;\n}\n")
                .contains("declarations must come first")
        );
    }

    #[test]
    fn empty_declaration_errors() {
        assert!(parse_err("gap:;\n").contains("needs a value"));
    }
}

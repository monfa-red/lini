//! The parser — single-pass recursive descent over the grammar in SPEC §16.
//!
//! The bracket-and-bars vocabulary makes one token of lookahead enough, with no
//! type-set prescan: `{` opens style, `[` opens children, `|…|` is a type. The
//! file is three phases — an optional leading `{ }` stylesheet, then the canvas
//! instances, then the links — and a body nests the same idea (a `{ }`, then a
//! `[ ]` of children and internal links).

use super::ast::*;
use crate::ast::{LinkOp, Side};
use crate::error::Error;
use crate::lexer::{TokKind, Token};
use crate::span::Span;

/// Parse a token stream into a [`File`].
pub fn parse(tokens: &[Token]) -> Result<File, Error> {
    Parser::new(tokens).parse_file()
}

/// The shared head tail (SPEC §3/§9): a head label, worn classes, and the head's
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

    /// A stylesheet item: a declaration, a `--var`, a rule (incl. `.class`), or a
    /// define (`|name::base|`). Assumes newlines skipped.
    fn classify_setup(&self) -> Result<Kind, Error> {
        match self.kind() {
            Some(TokKind::RawCssVar(_)) => Ok(Kind::Var),
            Some(TokKind::Dot) => Ok(Kind::Rule),  // .class { … }
            Some(TokKind::Hash(_)) => Ok(Kind::Rule), // #hero { … } — an id rule
            Some(TokKind::LinkOp(_)) => Err(self.err(
                "'->' draws a link on the canvas — set link defaults with 'link:' / 'link-width:' in a '{ }' block",
            )),
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

    /// A canvas / body statement: a node (`|…|`), text (`"…"`), a link (a bare id
    /// followed by a link-op / `&` / a `.path` / a `:side`), or — flagged for a
    /// context error — a stray declaration or `--var`. A bare leading name with no
    /// link follow is invalid (a node leads with bars). Assumes newlines skipped.
    fn classify_body(&self) -> Kind {
        match self.kind() {
            Some(TokKind::Pipe) | Some(TokKind::String(_)) => Kind::Node,
            Some(TokKind::RawCssVar(_)) => Kind::Var,
            Some(TokKind::Ident(_)) => match self.kind_at(1) {
                Some(TokKind::LinkOp(_)) | Some(TokKind::Amp) => Kind::Link,
                Some(TokKind::Dot) if self.glued_at(1) => Kind::Link, // a.b endpoint path
                // `a:left -> b` is a sided first endpoint — `:ident` then a link-op
                // / `&`. A misplaced `gap: 4` decl has a value there, not `side ->`,
                // so it stays a (context-error) declaration; an invalid side then
                // surfaces as the proper "not a side" error in `parse_endpoint`.
                Some(TokKind::Colon)
                    if matches!(self.kind_at(2), Some(TokKind::Ident(_)))
                        && matches!(
                            self.kind_at(3),
                            Some(TokKind::LinkOp(_)) | Some(TokKind::Amp)
                        ) =>
                {
                    Kind::Link
                }
                Some(TokKind::Colon) => Kind::Decl,
                _ => Kind::Unknown,
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
            links: Vec::new(),
        };
        self.skip_newlines();
        if matches!(self.kind(), Some(TokKind::LBrace)) {
            let start = self.span();
            file.stylesheet = self.parse_stylesheet()?;
            file.stylesheet_span = Span::new(start.start, self.last_span().end);
            self.skip_newlines();
        }
        let mut in_links = false;
        while self.kind().is_some() {
            match self.classify_body() {
                Kind::Node => {
                    if in_links {
                        return Err(self.err("instances must come before links"));
                    }
                    file.instances.push(self.parse_child()?);
                }
                Kind::Link => {
                    in_links = true;
                    file.links.push(self.parse_link()?);
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

    /// `selector { decls }` (SPEC §4) — `|box| { }`, `.hot { }`, `#hero { }`,
    /// `|table| |box| { }`.
    fn parse_rule(&mut self) -> Result<Rule, Error> {
        let start = self.span();
        let selector = self.parse_selector()?;
        let (decls, _) = self.parse_style()?;
        Ok(Rule {
            selector,
            decls,
            span: Span::new(start.start, self.last_span().end),
        })
    }

    /// A run of space-separated selector units up to the `{` (SPEC §4): a type
    /// `|box|` / `|table#main|`, a class `.hot`, or an id `#hero`. The space is
    /// the descendant combinator; each unit keeps its sigil.
    fn parse_selector(&mut self) -> Result<Selector, Error> {
        let mut units = Vec::new();
        loop {
            match self.kind() {
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

    /// The bars — `|type|`, `|type#id|`, or `|#id|` (SPEC §3): the optional type
    /// and id, at least one present. Shared by an instance and a selector unit;
    /// `ctx` only picks the glued-class error wording.
    fn parse_identity(&mut self, ctx: BarsCtx) -> Result<(Option<String>, Option<String>), Error> {
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
            _ => return Err(self.err("'| |' needs a type or an '#id'")),
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
    fn parse_hash_id(&mut self) -> Result<String, Error> {
        let (run, span) = match self.kind() {
            Some(TokKind::Hash(s)) => (s.clone(), self.span()),
            _ => return Err(self.err("expected an '#id'")),
        };
        if !is_ident(&run) {
            return Err(Error::at(
                span,
                format!("'#{run}' is not a valid id — an id starts with a letter or '_'"),
            ));
        }
        self.pos += 1;
        Ok(run)
    }

    fn glued_class_err(&self, ctx: BarsCtx) -> Error {
        self.err(match ctx {
            BarsCtx::Instance => "a class follows the bars — write '|box| .hot', not '|box.hot|'",
            BarsCtx::Selector => {
                "a selector unit can't glue a type and a class — space them (descendant) or style '.hot'"
            }
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
        let (style, style_span) = self.opt_style()?;
        let (children, links) = self.opt_children()?;
        Ok(Define {
            name,
            base,
            style,
            style_span,
            children,
            links,
            span: Span::new(start.start, self.last_span().end),
        })
    }

    // ───────────────────────── Nodes ─────────────────────────

    /// A drawn child (SPEC §3): a bare string is a text node; anything else is a
    /// box.
    fn parse_child(&mut self) -> Result<Child, Error> {
        if matches!(self.kind(), Some(TokKind::String(_))) {
            Ok(Child::Text(self.parse_text_node()?))
        } else {
            Ok(Child::Box(self.parse_node()?))
        }
    }

    /// A text node `"…"` with an optional `{ … }` style block (SPEC §3) — a `{`
    /// glued-or-spaced right after the string is its own text style; otherwise it
    /// is bare. (Strings are self-delimiting, so a following `"` is the next node.)
    fn parse_text_node(&mut self) -> Result<TextNode, Error> {
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
            span: Span::new(start.start, self.last_span().end),
        })
    }

    /// A drawn box (SPEC §3): identity in the bars, then the shared tail (head
    /// label, classes, style), then the `[ ]` children. The smart label rides
    /// `Node.label` and is lowered per type at desugar.
    fn parse_node(&mut self) -> Result<Node, Error> {
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
            span: Span::new(start.start, self.last_span().end),
        })
    }

    /// The class slot — `.name` worn by a node after its type or by a link after
    /// its endpoints (SPEC §3/§9). A `.` glued to an id or endpoint is a path and
    /// never reaches here; what does is the worn-class chain, written `.hot.loud`.
    fn parse_classes(&mut self) -> Result<Vec<String>, Error> {
        let mut classes = Vec::new();
        while self.eat(&TokKind::Dot) {
            classes.push(self.expect_ident()?.0);
        }
        Ok(classes)
    }

    /// Consume an optional `{ }` style block; absent → no decls, no span.
    fn opt_style(&mut self) -> Result<(Vec<Decl>, Option<Span>), Error> {
        if matches!(self.kind(), Some(TokKind::LBrace)) {
            let (decls, span) = self.parse_style()?;
            Ok((decls, Some(span)))
        } else {
            Ok((Vec::new(), None))
        }
    }

    /// `{ decls }` — declarations only. The span covers `{ … }`, for the formatter.
    fn parse_style(&mut self) -> Result<(Vec<Decl>, Span), Error> {
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
        Ok((decls, Span::new(start.start, self.last_span().end)))
    }

    /// Consume an optional `[ children ]` block; absent → empty.
    fn opt_children(&mut self) -> Result<(Vec<Child>, Vec<Link>), Error> {
        if matches!(self.kind(), Some(TokKind::LBracket)) {
            self.parse_children()
        } else {
            Ok((Vec::new(), Vec::new()))
        }
    }

    /// `[ children, then internal links ]` (SPEC §3).
    fn parse_children(&mut self) -> Result<(Vec<Child>, Vec<Link>), Error> {
        self.expect(&TokKind::LBracket, "'['")?;
        self.skip_newlines();
        let mut children = Vec::new();
        let mut links = Vec::new();
        while !matches!(self.kind(), Some(TokKind::RBracket) | None) {
            match self.classify_body() {
                Kind::Node => {
                    if !links.is_empty() {
                        return Err(self.err("a child must come before the body's links"));
                    }
                    children.push(self.parse_child()?);
                }
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
    /// SPEC §3/§9: an optional one-string **head label**, then worn **classes**,
    /// then the head's own **style** block — in that order. The `[ ]` content
    /// (children for a node, labels for a link) is parsed by the caller. The head
    /// label carries no style of its own — a `{ }` after it is the node's / link's
    /// block — so a styled label rides the `[ ]` (`[ "…" { … } ]`).
    fn parse_tail(&mut self) -> Result<Tail, Error> {
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

    /// A link's `[ "label"… ]` content block (SPEC §9) — labels are styleable
    /// text leaves, newline-separated; the canonical form of the trailing sugar.
    fn parse_label_block(&mut self) -> Result<Vec<TextNode>, Error> {
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

    // ───────────────────────── Links ─────────────────────────

    fn parse_link(&mut self) -> Result<Link, Error> {
        let start = self.span();
        let mut chain = vec![self.parse_endpoint_group()?];
        let op = self.expect_link_op()?;
        chain.push(self.parse_endpoint_group()?);
        while let Some(next) = self.peek_link_op() {
            if next != op {
                return Err(self.err(format!(
                    "link chain mixes operators '{}' and '{}'",
                    link_op_str(op),
                    link_op_str(next)
                )));
            }
            self.pos += 1;
            chain.push(self.parse_endpoint_group()?);
        }
        // The same tail a node uses: a head label, worn classes, the link's own
        // style. The head label and the `[ ]` labels coexist — desugar
        // concatenates them for `along:` (SPEC §9).
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
        // A trailing `:side` forces an edge (SPEC §9). The path no longer peels a
        // final `.left` — that is now a child named `left`.
        let side = if self.eat(&TokKind::Colon) {
            let (name, name_span) = self.expect_ident()?;
            match Side::parse(&name) {
                Some(s) => {
                    end = name_span;
                    Some(s)
                }
                None => {
                    return Err(Error::at(
                        name_span,
                        format!("':{name}' is not a side — use top, bottom, left, or right"),
                    ));
                }
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

    /// The link op at the cursor as an owned copy, so a `while let` over it
    /// doesn't hold a borrow of `self` across the loop body.
    fn peek_link_op(&self) -> Option<LinkOp> {
        match self.kind() {
            Some(TokKind::LinkOp(op)) => Some(*op),
            _ => None,
        }
    }

    fn expect_link_op(&mut self) -> Result<LinkOp, Error> {
        match self.peek_link_op() {
            Some(op) => {
                self.pos += 1;
                Ok(op)
            }
            None => Err(self.err("expected a link operator")),
        }
    }
}

fn link_op_str(op: LinkOp) -> String {
    format!(
        "{}{}{}",
        op.start.start_str(),
        op.line.as_str(),
        op.end.end_str()
    )
}

/// Which bars are being parsed — picks the glued-class error wording only.
#[derive(Clone, Copy)]
enum BarsCtx {
    Instance,
    Selector,
}

/// A valid id / ident: starts with a letter or `_`, then ident chars.
fn is_ident(s: &str) -> bool {
    let mut bytes = s.bytes();
    matches!(bytes.next(), Some(c) if c.is_ascii_alphabetic() || c == b'_')
        && bytes.all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}

/// A valid hex colour run (no `#`): 3, 4, 6, or 8 hex digits.
fn is_hex_color(s: &str) -> bool {
    matches!(s.len(), 3 | 4 | 6 | 8) && s.bytes().all(|b| b.is_ascii_hexdigit())
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

    fn label(n: &Node) -> Option<&str> {
        n.label.as_ref().map(|t| t.text.as_str())
    }

    // ── Identity in the bars ──

    #[test]
    fn identity_type_and_id_in_bars() {
        let f = parse_ok("|box#server|\n");
        let n = instance(&f, 0);
        assert_eq!(n.ty.as_deref(), Some("box"));
        assert_eq!(n.id.as_deref(), Some("server"));
        assert_eq!(label(n), None);
    }

    #[test]
    fn id_only_bars_default_box() {
        let f = parse_ok("|#cat|\n");
        let n = instance(&f, 0);
        assert_eq!(n.ty, None);
        assert_eq!(n.id.as_deref(), Some("cat"));
    }

    #[test]
    fn anonymous_labelled_box() {
        let f = parse_ok("|box| \"Load balancer\"\n");
        let n = instance(&f, 0);
        assert_eq!(n.ty.as_deref(), Some("box"));
        assert_eq!(n.id, None);
        assert_eq!(label(n), Some("Load balancer"));
    }

    #[test]
    fn full_node_head_label_classes_style_child() {
        let f = parse_ok("|box#cat| \"Cat\" .hot.loud { fill: red } [ |badge| \"x\" ]\n");
        let n = instance(&f, 0);
        assert_eq!(n.id.as_deref(), Some("cat"));
        assert_eq!(label(n), Some("Cat"));
        assert_eq!(n.classes, vec!["hot", "loud"]);
        assert_eq!(n.style.len(), 1);
        assert_eq!(n.children.len(), 1);
        assert!(matches!(&n.children[0], Child::Box(b) if b.ty.as_deref() == Some("badge")));
    }

    #[test]
    fn empty_string_label_is_kept() {
        let f = parse_ok("|box#cat| \"\"\n");
        assert_eq!(label(instance(&f, 0)), Some(""));
    }

    #[test]
    fn head_label_may_carry_the_nodes_own_style() {
        // `{ }` after the head label is the node's block, not the label's (SPEC §3).
        let f = parse_ok("|box#api| \"API\" { fill: red }\n");
        let n = instance(&f, 0);
        assert_eq!(label(n), Some("API"));
        assert_eq!(n.style.len(), 1);
    }

    #[test]
    fn label_and_bracket_content_coexist() {
        let f = parse_ok("|group#k| \"Kitchen\" [ |box#bowl| \"Bowl\" ]\n");
        let n = instance(&f, 0);
        assert_eq!(label(n), Some("Kitchen"));
        assert_eq!(n.children.len(), 1);
    }

    // ── Tail-order errors ──

    #[test]
    fn two_head_labels_error() {
        assert!(parse_err("|box#cat| \"a\" \"b\"\n").contains("one inline label"));
    }

    #[test]
    fn label_after_a_class_errors() {
        assert!(parse_err("|box#cat| .hot \"Cat\"\n").contains("comes before classes"));
    }

    #[test]
    fn class_in_the_bars_errors() {
        for src in ["|box.hot|\n", "|.hot|\n"] {
            assert!(parse_err(src).contains("follows the bars"), "{src}");
        }
        parse_ok("|box| .hot\n"); // the class follows the bars
    }

    #[test]
    fn empty_bars_error() {
        for src in ["| |\n", "||\n"] {
            assert!(parse_err(src).contains("needs a type or an '#id'"), "{src}");
        }
    }

    #[test]
    fn invalid_id_errors() {
        assert!(parse_err("|box#123|\n").contains("not a valid id"));
    }

    // ── Selectors ──

    #[test]
    fn selector_units() {
        let f = parse_ok(
            "{\n  |box| { radius: 4; }\n  .hot { stroke-width: 2; }\n  #hero { fill: gold; }\n  |table| |box| { stroke-width: 0; }\n  .sidebar |box| { fill: gray; }\n  |table#main| |box| { fill: white; }\n}\n",
        );
        let rule = |i: usize| match &f.stylesheet[i] {
            StyleItem::Rule(r) => &r.selector.units,
            _ => panic!("rule {i}"),
        };
        assert!(matches!(rule(0).as_slice(), [SelUnit::Type { name, id: None }] if name == "box"));
        assert!(matches!(rule(1).as_slice(), [SelUnit::Class(c)] if c == "hot"));
        assert!(matches!(rule(2).as_slice(), [SelUnit::Id(i)] if i == "hero"));
        assert_eq!(rule(3).len(), 2);
        assert!(matches!(rule(4)[0], SelUnit::Class(_)));
        assert!(
            matches!(&rule(5)[0], SelUnit::Type { name, id: Some(id) } if name == "main" || id == "main")
        );
    }

    #[test]
    fn compound_selector_unit_errors() {
        assert!(parse_err("{\n  |box.hot| { fill: red; }\n}\n").contains("can't glue"));
    }

    #[test]
    fn bare_type_rule_errors() {
        assert!(parse_err("{\n  box { radius: 4; }\n}\n").contains("only appears in bars"));
    }

    #[test]
    fn define_in_stylesheet() {
        let f = parse_ok("{\n  |treat::box| { radius: 5; }\n}\n|treat#x|\n");
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
    fn define_with_intrinsic_children() {
        let f = parse_ok(
            "{\n  |room::group| {\n    gap: 4;\n  } [\n    |box#inlet|\n    |box#outlet|\n    inlet -> outlet\n  ]\n}\n",
        );
        match &f.stylesheet[0] {
            StyleItem::Define(d) => {
                assert_eq!(d.children.len(), 2);
                assert_eq!(d.links.len(), 1);
                assert_eq!(d.style.len(), 1);
            }
            _ => panic!("expected a define"),
        }
    }

    // ── Links ──

    #[test]
    fn quickstart_three_box_chain() {
        let f = parse_ok("cat -> dog -> bird\n");
        assert!(f.stylesheet.is_empty() && f.instances.is_empty());
        assert_eq!(f.links.len(), 1);
        assert_eq!(f.links[0].chain.len(), 3);
    }

    #[test]
    fn link_with_sides_label_class_style() {
        let f = parse_ok("a:left -> b:top \"watches\" .loud { along: 0.5 }\n");
        let w = &f.links[0];
        assert_eq!(w.chain[0].endpoints[0].side, Some(Side::Left));
        assert_eq!(w.chain[1].endpoints[0].side, Some(Side::Top));
        assert_eq!(w.label.as_ref().map(|t| t.text.as_str()), Some("watches"));
        assert_eq!(w.classes, vec!["loud"]);
        assert_eq!(w.style.len(), 1);
    }

    #[test]
    fn link_line_styles() {
        assert_eq!(
            parse_ok("a -> b\n").links[0].op.line,
            crate::ast::LineStyle::Solid
        );
        assert_eq!(
            parse_ok("a --> b\n").links[0].op.line,
            crate::ast::LineStyle::Dashed
        );
        assert_eq!(
            parse_ok("a ---> b\n").links[0].op.line,
            crate::ast::LineStyle::Dotted
        );
        assert_eq!(
            parse_ok("a ~> b\n").links[0].op.line,
            crate::ast::LineStyle::Wavy
        );
    }

    #[test]
    fn fan_and_class_on_link() {
        let f = parse_ok("a & b -> c & d .loud\n");
        let w = &f.links[0];
        assert_eq!(w.chain[0].endpoints.len(), 2);
        assert_eq!(w.chain[1].endpoints.len(), 2);
        assert_eq!(w.classes, vec!["loud"]);
    }

    #[test]
    fn link_head_label_and_bracket_labels_coexist() {
        let f = parse_ok("a -> b \"x\" [ \"y\" ]\n");
        let w = &f.links[0];
        assert_eq!(w.label.as_ref().map(|t| t.text.as_str()), Some("x"));
        assert_eq!(w.labels.len(), 1);
    }

    #[test]
    fn link_two_bracket_labels() {
        let f = parse_ok("a -> b [ \"x\" \"y\" ]\n");
        assert_eq!(f.links[0].labels.len(), 2);
        assert_eq!(f.links[0].labels[0].text, "x");
    }

    #[test]
    fn two_head_labels_on_a_link_error() {
        assert!(parse_err("a -> b \"x\" \"y\"\n").contains("one inline label"));
    }

    #[test]
    fn dotpath_endpoint_and_forced_side() {
        let f = parse_ok("cat:right -> kitchen.counter.bowl:left\n");
        let w = &f.links[0];
        assert_eq!(w.chain[0].endpoints[0].path, vec!["cat"]);
        assert_eq!(w.chain[0].endpoints[0].side, Some(Side::Right));
        assert_eq!(
            w.chain[1].endpoints[0].path,
            vec!["kitchen", "counter", "bowl"]
        );
        assert_eq!(w.chain[1].endpoints[0].side, Some(Side::Left));
    }

    #[test]
    fn unknown_side_errors() {
        assert!(parse_err("a:middle -> b\n").contains("is not a side"));
    }

    #[test]
    fn mixed_operators_error() {
        assert!(parse_err("a -> b -- c\n").contains("mixes operators"));
    }

    // ── Statement classification ──

    #[test]
    fn bare_name_on_canvas_errors() {
        assert!(parse_err("cat\n").contains("leads with bars"));
    }

    #[test]
    fn bare_string_is_a_text_node() {
        let f = parse_ok("\"a\" \"b\" \"c\"\n");
        assert_eq!(f.instances.len(), 3);
        assert!(f.instances.iter().all(|c| matches!(c, Child::Text(_))));
    }

    #[test]
    fn text_node_takes_a_style_block() {
        let f = parse_ok("\"hi\" { color: red; translate: 0 -6 }\n\"plain\"\n");
        match &f.instances[0] {
            Child::Text(t) => assert_eq!(t.style.len(), 2),
            _ => panic!("styled text"),
        }
        match &f.instances[1] {
            Child::Text(t) => assert!(t.style.is_empty()),
            _ => panic!("bare text"),
        }
    }

    #[test]
    fn three_phases() {
        let f = parse_ok(
            "{\n  layout: grid;\n  |box| { radius: 6; }\n  .hot { stroke-width: 2; }\n}\n\
             |box#server|\n|box#client|\nserver -> client \"requests\"\n",
        );
        assert_eq!(f.stylesheet.len(), 3);
        assert_eq!(f.instances.len(), 2);
        assert_eq!(f.links.len(), 1);
    }

    #[test]
    fn stylesheet_is_optional() {
        let f = parse_ok("|box#server|\nserver -> server\n");
        assert!(f.stylesheet.is_empty());
        assert_eq!(f.instances.len(), 1);
    }

    // ── Values ──

    #[test]
    fn hex_value_validates() {
        let f = parse_ok("|box#x| { fill: #f80; stroke: #ffaa00cc }\n");
        let n = instance(&f, 0);
        assert!(matches!(&n.style[0].groups[0][0], Value::Hex(h) if h == "f80"));
        assert!(parse_err("|box#x| { fill: #zz }\n").contains("invalid hex color"));
    }

    #[test]
    fn call_and_var_values() {
        let f = parse_ok(
            "{\n  columns: repeat(3);\n  --brand: #ff6600;\n}\n|box#card| { fill: --brand; columns: 80 repeat(2, 40) }\n",
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
    fn value_groups_space_and_comma() {
        let f = parse_ok("|line#x| { points: 0 0, 10 10, 20 0; translate: 100 50 }\n");
        let points = instance(&f, 0)
            .style
            .iter()
            .find(|d| d.name == "points")
            .unwrap();
        assert_eq!(points.groups.len(), 3);
        assert_eq!(points.groups[0].len(), 2);
    }

    // ── Phase / context errors ──

    #[test]
    fn stylesheet_after_instance_errors() {
        assert!(parse_err("|box#x|\n{\n  |box| { radius: 4; }\n}\n").contains("must come first"));
    }

    #[test]
    fn instance_after_link_errors() {
        assert!(parse_err("a -> b\n|box#c|\n").contains("must come before links"));
    }

    #[test]
    fn link_as_instance_errors() {
        assert!(parse_err("|link|\n").contains("drawn by operators"));
    }

    #[test]
    fn node_type_as_instance_errors() {
        assert!(parse_err("|node|\n").contains("umbrella"));
    }

    #[test]
    fn link_defaults_block_is_rejected() {
        assert!(parse_err("{\n  -> { stroke: #666; }\n}\na -> b\n").contains("draws a link"));
    }

    #[test]
    fn decl_in_children_errors() {
        assert!(parse_err("|group#g| [\n  gap: 4\n]\n").contains("go in '{ }'"));
    }

    #[test]
    fn child_after_link_errors() {
        assert!(
            parse_err("|group#g| [\n  |box#a|\n  a -> a\n  |box#b|\n]\n")
                .contains("before the body's links")
        );
    }

    #[test]
    fn empty_declaration_errors() {
        assert!(parse_err("|box#a| { gap: }\n").contains("needs a value"));
    }
}

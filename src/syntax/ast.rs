//! The syntax tree: produced by [`super::parser`], consumed by `resolve`.
//!
//! A file is three ordered parts (SPEC §1/§3): the **stylesheet** — one leading
//! `{ }` block of root declarations, `--var` declarations, rules, and
//! `|name::base|` defines — then the **instances** (the canvas), then the
//! **wires**. Two brackets carry structure: `{ }` is style (declarations), `[ ]`
//! is content (a container's children, then its internal wires).

use crate::ast::{Side, WireOp};
use crate::span::Span;

#[derive(Debug, Clone)]
pub struct File {
    pub stylesheet: Vec<StyleItem>,
    /// The `{ … }` that wraps the stylesheet, for the formatter's trivia; empty
    /// when there is no stylesheet.
    pub stylesheet_span: Span,
    pub instances: Vec<Child>,
    pub wires: Vec<Wire>,
}

/// An entry in the stylesheet `{ }` block. Order among these is free; they all
/// precede the instances.
#[derive(Debug, Clone)]
pub enum StyleItem {
    /// `key: value;` — configures the root container.
    RootDecl(Decl),
    /// `--name: value;` — a themeable variable declaration. `Decl::name` holds
    /// the name without the `--` prefix.
    Var(Decl),
    /// `|selector| { decls }` / `.class { decls }` — element / class / descendant
    /// rule. The `-> { }` wire defaults are a `Rule` whose selector is the
    /// reserved `wire` element ([`super::parser`]).
    Rule(Rule),
    /// `|name::base| { style } [ children ]` — a new type from a base.
    Define(Define),
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub selector: Selector,
    pub decls: Vec<Decl>,
    pub span: Span,
}

/// One or more parts; more than one is a descendant combinator, matched against
/// a node's ancestor chain (ancestor → … → target), like CSS.
#[derive(Debug, Clone)]
pub struct Selector {
    pub parts: Vec<SelPart>,
}

#[derive(Debug, Clone)]
pub enum SelPart {
    /// A bare type name (`box`, a user type, …).
    Type(String),
    /// A `.class`.
    Class(String),
}

/// `|name::base| { style } [ children ]` — a new type from a base. `style` is the
/// type's defaults; `children` / `wires` are intrinsic, materialized per instance.
#[derive(Debug, Clone)]
pub struct Define {
    pub name: String,
    pub base: String,
    pub style: Vec<Decl>,
    /// The `{ … }` style block's span, for the formatter's trivia; `None` when
    /// the define has no style block.
    pub style_span: Option<Span>,
    pub children: Vec<Child>,
    pub wires: Vec<Wire>,
    pub span: Span,
}

/// A box — a drawn node (SPEC §3). Leads with an id or a `|type|`. Its `style` is
/// the `{ }` block; its `children` and internal `wires` are the `[ ]` block. Its
/// text is a `Child::Text` among the children, or its id (id-as-label) when there
/// is none.
#[derive(Debug, Clone)]
pub struct Node {
    pub id: Option<String>,
    /// `|type|`; `None` means the default `box`, filled at resolve.
    pub ty: Option<String>,
    pub classes: Vec<String>,
    pub style: Vec<Decl>,
    /// The `{ … }` style block's span, for the formatter's trivia; `None` when
    /// the node has no style block.
    pub style_span: Option<Span>,
    pub children: Vec<Child>,
    pub wires: Vec<Wire>,
    pub span: Span,
}

/// A body child, in source order: a box or a bare text node (SPEC §3).
#[derive(Debug, Clone)]
pub enum Child {
    Box(Node),
    Text(TextNode),
}

/// Bare text content `"…"` (SPEC §3) — a label, a cell, a wire label. No id,
/// type, classes, style, or children; never a wrapped node.
#[derive(Debug, Clone)]
pub struct TextNode {
    pub text: String,
    pub span: Span,
}

/// A wire (SPEC §9) — a relationship, not a container. `style` is its `{ }`
/// (`along:` and paint); `labels` are the trailing strings. A wire has no `[ ]`.
#[derive(Debug, Clone)]
pub struct Wire {
    pub chain: Vec<EndpointGroup>,
    pub op: WireOp,
    pub classes: Vec<String>,
    pub style: Vec<Decl>,
    /// The `{ … }` style block's span, for the formatter's trivia; `None` when
    /// the wire has no style block.
    pub style_span: Option<Span>,
    pub labels: Vec<TextNode>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EndpointGroup {
    pub endpoints: Vec<Endpoint>,
}

#[derive(Debug, Clone)]
pub struct Endpoint {
    pub path: Vec<String>,
    pub side: Option<Side>,
    pub span: Span,
}

/// `key: v…, v…;` — a declaration. `groups` is the comma-separated value list;
/// each group is a space-separated value sequence. One group is the common case;
/// `points` is the multi-group case.
#[derive(Debug, Clone)]
pub struct Decl {
    pub name: String,
    pub groups: Vec<Vec<Value>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Value {
    Number(f64),
    /// A number with a `%` suffix — a percentage (color components, SPEC §2).
    Percent(f64),
    String(String),
    Hex(String),
    Ident(String),
    /// `--name` reference (name stored without the `--`).
    Var(String),
    /// `rgb(…)`, `hsl(…)`, `repeat(…)`.
    Call(Call),
}

#[derive(Debug, Clone)]
pub struct Call {
    pub name: String,
    pub args: Vec<Value>,
}

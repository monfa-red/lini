//! v4 syntax tree (PLAN Phase 2). Produced by the v4 [`super::parser`]; consumed
//! by `resolve` once it is cut over in Phase 3. Until then the v3 front end
//! (`crate::ast` + `crate::parser`) still drives the pipeline.
//!
//! A file is three ordered parts (SPEC §1/§3): the **stylesheet** (root
//! declarations, `--var` declarations, rules, and `name::base` defines), then
//! the **instances** (nodes), then the **wires**.

use crate::ast::{Side, WireOp};
use crate::span::Span;

#[derive(Debug, Clone)]
pub struct File {
    pub stylesheet: Vec<StyleItem>,
    pub instances: Vec<Node>,
    pub wires: Vec<Wire>,
}

/// A top-level stylesheet entry. Order among these is free; they all precede the
/// instances.
#[derive(Debug, Clone)]
pub enum StyleItem {
    /// `key: value;` at the file top — configures the root container.
    RootDecl(Decl),
    /// `--name: value;` — a themeable variable declaration. `Decl::name` holds
    /// the name without the `--` prefix.
    Var(Decl),
    /// `selector { decls }` — element / class / descendant rule.
    Rule(Rule),
    /// `name::base { body }` — a new type from a base, with its defaults.
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
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum SelPart {
    /// A bare type name (`box`, a user type, …).
    Type(String),
    /// A `.class`.
    Class(String),
}

#[derive(Debug, Clone)]
pub struct Define {
    pub name: String,
    pub base: String,
    pub body: Block,
    pub span: Span,
}

/// An instance — a drawn node. At least one of id / type / label / block is
/// present (a statement that is only a newline is not a node).
#[derive(Debug, Clone)]
pub struct Node {
    pub id: Option<String>,
    /// `|type|`; `None` means the default `box`, filled at resolve.
    pub ty: Option<String>,
    pub labels: Vec<String>,
    pub classes: Vec<String>,
    pub block: Option<Block>,
    pub span: Span,
}

/// A node or define body: declarations, then child nodes, then internal wires
/// (SPEC §3 — the fixed in-block order).
#[derive(Debug, Clone, Default)]
pub struct Block {
    pub decls: Vec<Decl>,
    pub nodes: Vec<Node>,
    pub wires: Vec<Wire>,
}

#[derive(Debug, Clone)]
pub struct Wire {
    pub chain: Vec<EndpointGroup>,
    pub op: WireOp,
    pub labels: Vec<String>,
    pub classes: Vec<String>,
    pub block: Option<WireBlock>,
    pub span: Span,
}

/// A wire body: declarations and `|text|` children, in any order (SPEC §16).
#[derive(Debug, Clone, Default)]
pub struct WireBlock {
    pub decls: Vec<Decl>,
    pub texts: Vec<TextChild>,
}

#[derive(Debug, Clone)]
pub struct TextChild {
    pub text: String,
    pub classes: Vec<String>,
    pub decls: Vec<Decl>,
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
    pub span: Span,
}

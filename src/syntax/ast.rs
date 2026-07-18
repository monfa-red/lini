//! The syntax tree: produced by [`super::parser`], consumed by `resolve`.
//!
//! A file is three ordered parts [SPEC 1/3]: the **stylesheet** — one leading
//! `{ }` block of root declarations, `--var` declarations, rules, and
//! `|name::base|` defines — then the **instances** (the canvas), then the
//! **links**. Two brackets carry structure: `{ }` is style (declarations), `[ ]`
//! is content (a container's children, then its internal links).

use crate::ast::ChainOp;
use crate::span::Span;

#[derive(Debug, Clone)]
pub struct File {
    pub stylesheet: Vec<StyleItem>,
    /// The `{ … }` that wraps the stylesheet, for the formatter's trivia; empty
    /// when there is no stylesheet.
    pub stylesheet_span: Span,
    pub instances: Vec<Child>,
    pub links: Vec<Link>,
}

/// An entry in the stylesheet `{ }` block. Order among these is free; they all
/// precede the instances.
#[derive(Debug, Clone)]
pub enum StyleItem {
    /// `key: value;` — configures the root container.
    RootDecl(Decl),
    /// `--name: value;` — a themeable variable declaration. `Decl::segment` holds
    /// the name without the `--` prefix.
    Var(Decl),
    /// `|selector| { decls }` / `.class { decls }` — element / class / descendant
    /// rule. (Link defaults are cascading `link*` / `clearance` / `routing`
    /// properties now, not a rule — [SPEC 9].)
    Rule(Rule),
    /// `|name::base| { style } [ children ]` — a new type from a base.
    Define(Define),
    /// `name = value;` / `name(params) = value;` — an `=` binding [SPEC 10.7]: a
    /// compile-time value or function, read in any expression.
    Binding(FuncDef),
}

/// An `=` binding [SPEC 10.7] — a scalar (`params` empty, read bare like a constant)
/// or a function. `body` is the raw right-hand text (a group's inner content, or the
/// bare literal / name / call), parsed by [`crate::expr`] at resolve.
#[derive(Debug, Clone)]
pub struct FuncDef {
    pub name: String,
    pub params: Vec<String>,
    pub body: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub selector: Selector,
    pub decls: Vec<Decl>,
    pub span: Span,
}

/// One or more units; more than one is a descendant combinator (the space),
/// matched against a node's ancestor chain (ancestor → … → target), like CSS.
#[derive(Debug, Clone)]
pub struct Selector {
    pub units: Vec<SelUnit>,
}

/// A juxtaposed selector unit [SPEC 4]: a type (with an optional `#id`), a
/// `.class`, or an `#id`. Each keeps its sigil, so a selector reads as a run of
/// marked units and a bare word is never one.
#[derive(Debug, Clone)]
pub enum SelUnit {
    /// `|box|` or `|table#main|` — a type, optionally pinned to one id.
    Type { name: String, id: Option<String> },
    /// `.class`.
    Class(String),
    /// `#hero` — an id selector.
    Id(String),
    /// `|-|` — the link type [SPEC 4, 9]. Selector-only: it matches every link
    /// (a link is drawn by an operator, never instantiated), so a link is styled
    /// with the ordinary node/text vocabulary — `stroke` the wire, `color`/`font-*`
    /// the labels. Desugar lowers it to `.lini-link`, the class every link wears.
    Link,
    /// `(-)` — the dimension type [SPEC 4, 15.6]: the `|-|` subtype, selector-only,
    /// matching every drawing dimension (the whole family — `(-)`/`(o)`/`(<)`). A
    /// `(-) { }` rule beats a `|-| { }` rule for dimensions (a more-specific type).
    /// Desugar lowers it to `.lini-dimension`, worn by every dimension at resolve.
    Dimension,
}

/// `|name::base| { style } [ children ]` — a new type from a base. `style` is the
/// type's defaults; `children` / `links` are intrinsic, materialized per instance.
#[derive(Debug, Clone)]
pub struct Define {
    pub name: String,
    pub base: String,
    pub style: Vec<Decl>,
    /// The `{ … }` style block's span, for the formatter's trivia; `None` when
    /// the define has no style block.
    pub style_span: Option<Span>,
    pub children: Vec<Child>,
    pub links: Vec<Link>,
    pub span: Span,
}

/// A box — a drawn node [SPEC 3]. Leads with an id or a `|type|`. Its `style` is
/// the `{ }` block; its `children` and internal `links` are the `[ ]` block. Its
/// text is a `Child::Text` among the children, or its id (id-as-label) when there
/// is none.
#[derive(Debug, Clone)]
pub struct Node {
    /// From the bars (`|type#id|`).
    pub id: Option<String>,
    /// `|type|` from the bars; `None` means the default `box`, filled at resolve.
    pub ty: Option<String>,
    /// The smart-label head string (`|box| "X"`), lowered per type at desugar
    /// (text / caption / symbol); `None` when absent — a bare node is empty.
    pub label: Option<TextNode>,
    pub classes: Vec<String>,
    pub style: Vec<Decl>,
    /// The `{ … }` style block's span, for the formatter's trivia; `None` when
    /// the node has no style block.
    pub style_span: Option<Span>,
    pub children: Vec<Child>,
    pub links: Vec<Link>,
    pub span: Span,
}

/// A body child, in source order: a box or a bare text node [SPEC 3].
#[derive(Debug, Clone)]
pub enum Child {
    Box(Node),
    Text(TextNode),
}

impl Child {
    pub fn span(&self) -> Span {
        match self {
            Child::Box(n) => n.span,
            Child::Text(t) => t.span,
        }
    }
}

/// Text content `"…"` [SPEC 3] — a label, a cell, a link label. A leaf: no id,
/// type, or children, but in **content position** (a free-standing statement, a
/// `[ ]` child, a link `[ ]` label) it takes the node tail — it **may wear
/// classes and carry a style block** of text-only properties (`"Starter"
/// .card-title`, `"x" { color: red; translate: 0 -6 }`). A head label carries
/// neither (its classes belong to the owning node / link), so both are empty
/// there.
#[derive(Debug, Clone)]
pub struct TextNode {
    pub text: String,
    /// `"x" .hot` — worn classes [SPEC 3/4], tier 3 on the text leaf: text-valid
    /// declarations land, the rest are inert (the class-polymorphism law). Empty
    /// on a head label and a bare string.
    pub classes: Vec<String>,
    /// `"x" { … }` — the text node's own style (text-valid props only); empty
    /// when bare.
    pub style: Vec<Decl>,
    /// The `{ … }` style block's span, for the formatter's trivia; `None` when
    /// the text has no style block.
    pub style_span: Option<Span>,
    pub span: Span,
}

/// A link [SPEC 9]. `style` is its `{ }` (`along:`, `link*`); `labels` are its
/// text content — trailing strings, or a `[ ]` of styleable text leaves. A
/// one-ended statement (a leader / unary measure, [SPEC 15.6/21]) has a single
/// chain group; whether that is legal for the op is resolve's call, per scope.
#[derive(Debug, Clone)]
pub struct Link {
    pub chain: Vec<EndpointGroup>,
    /// One op per hop (`ops[i]` joins `chain[i]` → `chain[i+1]`); a one-ended
    /// statement keeps its single op. Wire hops may differ (`a - b -> c`, the
    /// bare-first-hop spelling [SPEC 9]); measures and mates chain one op.
    pub ops: Vec<ChainOp>,
    pub classes: Vec<String>,
    pub style: Vec<Decl>,
    /// The `{ … }` style block's span, for the formatter's trivia; `None` when
    /// the link has no style block.
    pub style_span: Option<Span>,
    /// The smart-label head string (`a -> b "watches"`), unstyled; desugar
    /// concatenates it ahead of `labels` for `along:`. `None` when absent.
    pub label: Option<TextNode>,
    /// The `[ ]` label leaves (styleable).
    pub labels: Vec<TextNode>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EndpointGroup {
    pub endpoints: Vec<Endpoint>,
}

impl Link {
    /// The statement's (first) operator — every op when the chain is a
    /// measure/mate (the parser enforces it), the first hop's otherwise.
    pub fn op(&self) -> ChainOp {
        self.ops[0]
    }
}

#[derive(Debug, Clone)]
pub struct Endpoint {
    pub path: Vec<String>,
    /// A trailing 1-based pattern-copy index [SPEC 15.4/21] —
    /// `plate.bolt.2` picks the second placed copy; drawing scope only.
    pub copy: Option<usize>,
    /// The `:point` after the path [SPEC 9, 15.2] — raw at parse: a side, a
    /// corner, `center`, or an authored name; resolve validates it per scope.
    pub point: Option<PointRef>,
    pub span: Span,
}

/// An endpoint's raw `:point` — the name and its own span, for anchor errors.
#[derive(Debug, Clone)]
pub struct PointRef {
    pub name: String,
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
    /// A number with a `%` suffix — a percentage (color components, [SPEC 2]).
    Percent(f64),
    String(String),
    Hex(String),
    Ident(String),
    /// `--name` reference (name stored without the `--`).
    Var(String),
    /// `rgb(…)`, `hsl(…)`, `repeat(…)`.
    Call(Call),
    /// A `(…)` math group's inner text (its outer parens stripped), or a call
    /// argument that carries an operator [SPEC 10.7] — parsed by [`crate::expr`] and
    /// folded to a number or a point at resolve.
    Expr(String),
    /// `right(50):segment` — a pen call naming its drawn segment; parsed only
    /// inside a `draw:` value [SPEC 15.3, 21]. A `:segment` always glues to a
    /// call — a station is `point():name` [SPEC 15.3].
    NamedCall(Call, String),
    /// A space-separated run inside **one call-argument slot** —
    /// `hatch(45 -45, 6)`'s angle group [SPEC 10.3]. Never nests.
    Tuple(Vec<Value>),
}

#[derive(Debug, Clone)]
pub struct Call {
    pub name: String,
    pub args: Vec<Value>,
}

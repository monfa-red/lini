use crate::span::Span;

/// A Lini file: optional defs block, then a stream of root statements
/// (node decls and wires) in any order.
#[derive(Debug)]
pub struct File {
    pub defs: Option<DefsBlock>,
    pub stmts: Vec<Stmt>,
}

/// Top-level scene statement.
#[derive(Debug)]
pub enum Stmt {
    Node(ShapeInst),
    Wire(WireDecl),
}

// ─────────────────────────── Defs block ───────────────────────────

#[derive(Debug)]
pub struct DefsBlock {
    pub entries: Vec<DefsEntry>,
    pub span: Span,
}

#[derive(Debug)]
pub enum DefsEntry {
    SceneConfig(SceneConfig),
    WireConfig(WireConfig),
    TypeDefaults(TypeDefaults),
    VarOverride(VarOverride),
    StyleDef(StyleDef),
    ShapeDef(ShapeDef),
}

impl DefsEntry {
    pub fn span(&self) -> Span {
        match self {
            DefsEntry::SceneConfig(s) => s.span,
            DefsEntry::WireConfig(w) => w.span,
            DefsEntry::TypeDefaults(t) => t.span,
            DefsEntry::VarOverride(v) => v.span,
            DefsEntry::StyleDef(s) => s.span,
            DefsEntry::ShapeDef(s) => s.span,
        }
    }
}

/// `|scene|` line — root scene container config.
#[derive(Debug)]
pub struct SceneConfig {
    pub items: Vec<AttrItem>,
    pub span: Span,
}

/// `|wire|` line — global wire defaults (lowest specificity, layered under
/// styles and per-wire attrs).
#[derive(Debug)]
pub struct WireConfig {
    pub items: Vec<AttrItem>,
    pub span: Span,
}

/// `|name|` line in the defs block — defaults applied to every instance of
/// the named type (primitive, template, or user shape). Sits as the lowest
/// specificity layer under inheritance, styles, and inline attrs.
#[derive(Debug)]
pub struct TypeDefaults {
    pub name: String,
    pub items: Vec<AttrItem>,
    pub span: Span,
}

/// `--name:value` line — CSS variable override.
#[derive(Debug)]
pub struct VarOverride {
    pub name: String,
    pub value: Value,
    pub span: Span,
}

#[derive(Debug)]
pub struct StyleDef {
    pub name: String,
    pub items: Vec<AttrItem>,
    pub span: Span,
}

#[derive(Debug)]
pub struct ShapeDef {
    pub name: String,
    pub base: TypeRef,
    pub items: Vec<AttrItem>,
    pub body: Option<Vec<BodyItem>>,
    pub span: Span,
}

// ─────────────────────────── Scene tree ───────────────────────────

/// Node or primitive instance. `id` is `Some` for declared scene nodes,
/// `None` for anonymous primitives declared with `|type|` directly.
#[derive(Debug, Clone)]
pub struct ShapeInst {
    pub id: Option<String>,
    pub ty: TypeRef,
    pub label: Option<String>,
    pub href: Option<String>,
    pub items: Vec<AttrItem>,
    pub body: Option<Vec<BodyItem>>,
    pub span: Span,
}

/// Items legal inside a shape/group body: child nodes/primitives, and
/// internal wires (per SPEC section 10 internal wires in shape definitions).
#[derive(Debug, Clone)]
pub enum BodyItem {
    Inst(ShapeInst),
    Wire(WireDecl),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeRef {
    pub name: String,
    pub span: Span,
}

// ─────────────────────────── Attrs / styles ───────────────────────────

#[derive(Debug, Clone)]
pub enum AttrItem {
    Attr(Attr),
    Style(StyleRef),
}

#[derive(Debug, Clone)]
pub struct Attr {
    pub name: String,
    pub value: Value,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StyleRef {
    pub name: String,
    pub span: Span,
}

// ─────────────────────────── Wires ───────────────────────────

/// A wire declaration: a sequence of endpoint groups joined by a single
/// operator (mixing operators within one chain is a parse error).
///
///   a -> b               (chain: [{a}, {b}])
///   a -> b -> c          (chain: [{a}, {b}, {c}])
///   a -> b & c           (chain: [{a}, {b,c}])  — fan-out
///   a & b -> c & d       (chain: [{a,b}, {c,d}]) — cartesian
#[derive(Debug, Clone)]
pub struct WireDecl {
    pub chain: Vec<EndpointGroup>,
    pub op: WireOp,
    pub label: Option<String>,
    pub items: Vec<AttrItem>,
    pub body: Option<Vec<TextDecl>>,
    pub span: Span,
}

/// A group of endpoints joined by `&` — the cartesian factor in a fan.
#[derive(Debug, Clone)]
pub struct EndpointGroup {
    pub endpoints: Vec<WireEndpoint>,
}

#[derive(Debug, Clone)]
pub struct WireEndpoint {
    /// Dot-path from scene root: `cat` is `["cat"]`, `garden.frog` is
    /// `["garden", "frog"]`.
    pub path: Vec<String>,
    pub side: Option<Side>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

impl Side {
    /// In `index` order — the canonical side enumeration.
    pub const ALL: [Side; 4] = [Side::Top, Side::Right, Side::Bottom, Side::Left];

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "top" => Self::Top,
            "bottom" => Self::Bottom,
            "left" => Self::Left,
            "right" => Self::Right,
            _ => return None,
        })
    }

    /// Dense id (clockwise from top) — the routing stages' map key.
    pub fn index(self) -> u8 {
        match self {
            Side::Top => 0,
            Side::Right => 1,
            Side::Bottom => 2,
            Side::Left => 3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TextDecl {
    pub text: String,
    pub items: Vec<AttrItem>,
    pub span: Span,
}

// ─────────────────────────── Wire ops ───────────────────────────

/// A composed wire operator: `[start_marker?][line][end_marker?]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireOp {
    pub line: LineStyle,
    pub start: WireMarker,
    pub end: WireMarker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStyle {
    Solid,  // -
    Dashed, // --
    Dotted, // -.-
    Wavy,   // ~
}

impl LineStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Solid => "-",
            Self::Dashed => "--",
            Self::Dotted => "-.-",
            Self::Wavy => "~",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WireMarker {
    #[default]
    None,
    Arrow,   // < at start, > at end
    Crow,    // > at start, < at end
    Dot,     // * on either side
    Diamond, // <> on either side
}

impl WireMarker {
    /// Glyph for this marker when rendered at the start side of a wire op.
    pub fn start_str(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Arrow => "<",
            Self::Crow => ">",
            Self::Dot => "*",
            Self::Diamond => "<>",
        }
    }
    /// Glyph for this marker when rendered at the end side of a wire op.
    pub fn end_str(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Arrow => ">",
            Self::Crow => "<",
            Self::Dot => "*",
            Self::Diamond => "<>",
        }
    }
}

// ─────────────────────────── Values ───────────────────────────

#[derive(Debug, Clone)]
pub enum Value {
    Number(f64),
    String(String),
    Hex(String),
    Ident(String),
    Tuple(Vec<Value>),
    List(Vec<Value>),
    Call(FnCall),
    RawCssVar(String), // `--name` reference to a Lini CSS var
}

#[derive(Debug, Clone)]
pub struct FnCall {
    pub name: String,
    pub args: Vec<Value>,
    pub span: Span,
}

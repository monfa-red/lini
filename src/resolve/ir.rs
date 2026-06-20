use crate::ast::Side;
use crate::span::Span;
use std::collections::{BTreeMap, HashMap};

/// Fully resolved program — output of phase 2.
pub struct Program {
    pub vars: VarTable,
    pub scene: ResolvedScene,
    pub wires: Vec<ResolvedWire>,
    pub sheet: SheetInputs,
}

/// The stylesheet layers the renderer restates as CSS class rules — paint rides
/// CSS, geometry bakes (SPEC §13). Node attrs arrive fully merged; these are the
/// per-layer inputs the rules builder emits alongside them. Descendant rules
/// (`|table box| { }`) carry no entry: their paint bakes inline via the cascade.
#[derive(Default, Clone)]
pub struct SheetInputs {
    /// `.name { }` class rules, in source order — emitted as `lini-style-*`.
    pub class_rules: Vec<(String, AttrMap)>,
    /// `name { }` element rules, in source order — merged into `lini-shape-*`.
    pub element_rules: Vec<(String, AttrMap)>,
    /// `|name::base| { }` define defaults (own attrs only), in source order.
    pub defines: Vec<(String, AttrMap)>,
    /// Built-in template attrs (e.g. `group`'s container look).
    pub templates: Vec<(String, AttrMap)>,
    /// `wire { }` defaults.
    pub wire_defaults: AttrMap,
}

/// Scene block: container attrs + body of instances.
pub struct ResolvedScene {
    pub attrs: AttrMap,
    pub nodes: Vec<ResolvedInst>,
}

/// A resolved node or primitive instance. `id` is `Some` iff the source used a
/// named scene node (`cat |treat| …`); anonymous primitives have `id == None`.
pub struct ResolvedInst {
    pub id: Option<String>,
    pub shape: ShapeKind,
    /// User-shape and template names walked from the inst's declared type back
    /// to its primitive (e.g. for `cat |treat|` where `treat:box`, this is
    /// `["treat"]` — the primitive `box` is in `shape`).
    pub type_chain: Vec<String>,
    /// Style class names applied to this inst, in source (left-to-right) order.
    pub applied_styles: Vec<String>,
    /// For `Text` shape: the text content. For other shapes: always `None` —
    /// label sugar on non-text shapes produces a `Text` child instead.
    pub label: Option<String>,
    pub attrs: AttrMap,
    pub markers: Markers,
    pub children: Vec<ResolvedInst>,
    pub span: Span,
}

/// One of the built-in primitives. All user shapes resolve to one of these.
/// (There is no title primitive — a caption is just a small-text `|plain|` flow
/// child, first in a column for a title or last for a footer, SPEC §8.)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Box,
    Oval,
    Hex,
    Slant,
    Cyl,
    Diamond,
    Cloud,
    Poly,
    Path,
    Text,
    Line,
    Icon,
    Image,
}

impl ShapeKind {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "box" => Self::Box,
            "oval" => Self::Oval,
            "hex" => Self::Hex,
            "slant" => Self::Slant,
            "cyl" => Self::Cyl,
            "diamond" => Self::Diamond,
            "cloud" => Self::Cloud,
            "poly" => Self::Poly,
            "path" => Self::Path,
            "line" => Self::Line,
            "icon" => Self::Icon,
            "image" => Self::Image,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Box => "box",
            Self::Oval => "oval",
            Self::Hex => "hex",
            Self::Slant => "slant",
            Self::Cyl => "cyl",
            Self::Diamond => "diamond",
            Self::Cloud => "cloud",
            Self::Poly => "poly",
            Self::Path => "path",
            Self::Text => "text",
            Self::Line => "line",
            Self::Icon => "icon",
            Self::Image => "image",
        }
    }
}

/// Final attribute values after section 13 specificity merging. Marker attrs are
/// extracted into `Markers` and not stored here.
#[derive(Default, Clone, Debug)]
pub struct AttrMap {
    pub map: BTreeMap<String, ResolvedValue>,
}

impl AttrMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, name: impl Into<String>, value: ResolvedValue) {
        self.map.insert(name.into(), value);
    }

    pub fn get(&self, name: &str) -> Option<&ResolvedValue> {
        self.map.get(name)
    }

    /// The attr's numeric value, if it has one (see [`ResolvedValue::as_number`]).
    pub fn number(&self, name: &str) -> Option<f64> {
        self.get(name).and_then(ResolvedValue::as_number)
    }
}

#[derive(Clone, Debug)]
pub enum ResolvedValue {
    Number(f64),
    String(String),
    Hex(String),
    Ident(String),
    /// Raw CSS text from a `--theme` value that isn't a typed Lini value (e.g. a
    /// font stack `Inter, system-ui, sans-serif`). Emitted verbatim — unlike
    /// `String`, it is never quote-wrapped.
    RawCss(String),
    Tuple(Vec<ResolvedValue>),
    List(Vec<ResolvedValue>),
    Call(ResolvedCall),
    LiveVar {
        name: String,
        raw: bool,
        baked: Option<Box<ResolvedValue>>,
    },
}

impl ResolvedValue {
    /// The numeric value, following a layout var's baked indirection.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            ResolvedValue::Number(n) => Some(*n),
            ResolvedValue::LiveVar { baked: Some(b), .. } => b.as_number(),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedCall {
    pub name: String,
    pub args: Vec<ResolvedValue>,
}

/// CSS variable defaults table. Entries are keyed by name without the
/// `--lini-` prefix.
#[derive(Clone, Debug, Default)]
pub struct VarTable {
    pub entries: HashMap<String, VarEntry>,
}

#[derive(Clone, Debug)]
pub struct VarEntry {
    pub kind: VarKind,
    pub value: ResolvedValue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarKind {
    Layout,
    Visual,
}

impl VarTable {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&VarEntry> {
        self.entries.get(name)
    }

    pub fn set(&mut self, name: impl Into<String>, kind: VarKind, value: ResolvedValue) {
        self.entries.insert(name.into(), VarEntry { kind, value });
    }
}

#[derive(Clone, Debug, Default)]
pub struct Markers {
    pub start: MarkerKind,
    pub end: MarkerKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MarkerKind {
    #[default]
    None,
    Arrow,
    Dot,
    Diamond,
    Crow,
}

impl MarkerKind {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "none" => Self::None,
            "arrow" => Self::Arrow,
            "dot" => Self::Dot,
            "diamond" => Self::Diamond,
            "crow" => Self::Crow,
            _ => return None,
        })
    }

    pub fn from_marker(m: crate::ast::WireMarker) -> Self {
        match m {
            crate::ast::WireMarker::None => Self::None,
            crate::ast::WireMarker::Arrow => Self::Arrow,
            crate::ast::WireMarker::Crow => Self::Crow,
            crate::ast::WireMarker::Dot => Self::Dot,
            crate::ast::WireMarker::Diamond => Self::Diamond,
        }
    }
}

pub struct ResolvedWire {
    pub endpoints: Vec<ResolvedEndpoint>,
    pub attrs: AttrMap,
    /// Names of the `.style`s applied to this wire, in source order — emitted as
    /// `lini-style-{name}` classes, exactly like a node's (SPEC §14).
    pub applied_styles: Vec<String>,
    pub markers: Markers,
    /// Wire labels (label sugar + body `|text|`s), placed onto the drawn
    /// route by the router's label pass (WIRING §Model step 7).
    pub texts: Vec<ResolvedText>,
    pub span: Span,
}

pub struct ResolvedEndpoint {
    /// Fully-qualified dot-path from scene root (e.g. `garden.outlet`).
    pub path: String,
    pub side: Option<Side>,
    pub span: Span,
}

#[derive(Clone)]
pub struct ResolvedText {
    pub text: String,
    pub along: Along,
    pub attrs: AttrMap,
}

/// Where a label rides its wire (SPEC §9): `Auto` distributes it along the
/// route; `Fraction` pins it at an explicit `along:` fraction (0..1).
#[derive(Clone, Debug)]
pub enum Along {
    Auto,
    Fraction(f64),
}

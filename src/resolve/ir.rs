use crate::ast::Side;
use crate::expr::{Expr, FuncTable};
use crate::span::Span;
use std::collections::{BTreeMap, HashMap};

/// Fully resolved program — output of phase 2.
pub struct Program {
    pub vars: VarTable,
    pub scene: ResolvedScene,
    pub links: Vec<ResolvedLink>,
    pub sheet: SheetInputs,
    /// Stylesheet functions (SPEC §11.7), carried to the layout phase so a chart can
    /// sample a deferred `fn:` once its x-domain is fixed ([CHARTS.md] §4). Built at
    /// resolve; only the chart layout reads it.
    pub funcs: FuncTable,
}

/// The render inputs the rules builder restates as CSS class rules — paint rides
/// CSS, geometry bakes (SPEC §13). After desugar every type/template/define lives
/// as a single-class rule, so this is just those rules' resolved attrs (the
/// generated `.lini-*` type classes and the user `.style` classes, in stylesheet
/// order), the link defaults, and the root inherited-text baseline. Descendant
/// rules (`|.lini-table .lini-box| { }`) carry no entry: their paint bakes inline.
#[derive(Default, Clone)]
pub struct SheetInputs {
    /// Single-class rules in source order: `lini-<type>` (generated type classes,
    /// emitted verbatim) and user classes (emitted `lini-style-<name>`).
    pub class_rules: Vec<(String, AttrMap)>,
    /// The link layer's defaults (the `.lini-link` rule).
    pub link_defaults: AttrMap,
    /// The root container's `font-size` — the inherited-text baseline for `.lini`
    /// (a baked layout constant carried in the global block, not a CSS var).
    pub root_font_size: f64,
    /// Inherited-text props the global block set, for the `.lini` rule (SPEC §10):
    /// `font-family` / `font-weight` / `color` override their themeable var, the
    /// rest (`font-style`, `text-transform`, `text-decoration`, `text-shadow`) are
    /// live CSS with no default. Present only when authored.
    pub root_text: AttrMap,
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
    pub kind: NodeKind,
    /// User-defined type and template names walked from the inst's declared type back
    /// to its primitive (e.g. for `cat |treat|` where `treat:box`, this is
    /// `["treat"]` — the primitive `box` is in `kind`).
    pub type_chain: Vec<String>,
    /// Style class names applied to this inst, in source (left-to-right) order.
    pub applied_styles: Vec<String>,
    /// For `Text`: the text content. For other kinds: always `None` —
    /// label sugar on non-text shapes produces a `Text` child instead.
    pub label: Option<String>,
    pub attrs: AttrMap,
    /// A `Text` node's own `{ }` style (SPEC §3) — the text-valid props it set,
    /// emitted as a `style=` / `transform` on the `<text>`. Empty for boxes (their
    /// per-node diff is computed at render) and for unstyled text. `attrs` stays
    /// the effective text context (inherited ∪ own) for layout measurement.
    pub own_style: AttrMap,
    pub markers: Markers,
    pub children: Vec<ResolvedInst>,
    pub span: Span,
}

/// One of the built-in primitives. All user shapes resolve to one of these.
/// (There is no title primitive — a caption is just a small-text `|block|` flow
/// child, first in a column for a title or last for a footer, SPEC §8.)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Block,
    Oval,
    Hex,
    Slant,
    Cyl,
    Diamond,
    Poly,
    Path,
    Text,
    Line,
    Icon,
    Image,
}

impl NodeKind {
    /// Every primitive, in `as_str` order — the canonical enumeration desugar
    /// walks to emit a `.lini-<kind>` class def per present primitive.
    pub const ALL: [NodeKind; 12] = [
        Self::Block,
        Self::Oval,
        Self::Hex,
        Self::Slant,
        Self::Cyl,
        Self::Diamond,
        Self::Poly,
        Self::Path,
        Self::Text,
        Self::Line,
        Self::Icon,
        Self::Image,
    ];

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "block" => Self::Block,
            "oval" => Self::Oval,
            "hex" => Self::Hex,
            "slant" => Self::Slant,
            "cyl" => Self::Cyl,
            "diamond" => Self::Diamond,
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
            Self::Block => "block",
            Self::Oval => "oval",
            Self::Hex => "hex",
            Self::Slant => "slant",
            Self::Cyl => "cyl",
            Self::Diamond => "diamond",
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

    /// Remove an attr, if present.
    pub fn remove(&mut self, name: &str) {
        self.map.remove(name);
    }

    /// The attr's numeric value, if it has one (see [`ResolvedValue::as_number`]).
    pub fn number(&self, name: &str) -> Option<f64> {
        self.get(name).and_then(ResolvedValue::as_number)
    }
}

#[derive(Clone, Debug)]
pub enum ResolvedValue {
    Number(f64),
    /// A percentage — `50%`, only valid inside a colour (SPEC §2).
    Percent(f64),
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
    },
    /// A `fn:` series formula, held **unevaluated** (its `x` / `u` are unbound at
    /// resolve, [CHARTS.md] §4): one `Expr` for a whole-domain `fn:`, or several for a
    /// per-band list. Sampled to baked numbers at chart layout and never rendered, so
    /// the two exhaustive `ResolvedValue` matches (`render::values::format_value`,
    /// `layout::values::describe`) treat it as unreachable / opaque.
    Deferred(Vec<Expr>),
}

impl ResolvedValue {
    /// The numeric value, if this is a plain number. A `--name` reference is a
    /// visual var (SPEC §11.2), never a layout number, so it has none.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            ResolvedValue::Number(n) => Some(*n),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedCall {
    pub name: String,
    pub args: Vec<ResolvedValue>,
}

/// The visual `--lini-*` variable table (SPEC §11.2 — vars are visual-only).
/// Entries are keyed by name without the `--lini-` prefix.
#[derive(Clone, Debug, Default)]
pub struct VarTable {
    pub entries: HashMap<String, ResolvedValue>,
}

impl VarTable {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&ResolvedValue> {
        self.entries.get(name)
    }

    pub fn set(&mut self, name: impl Into<String>, value: ResolvedValue) {
        self.entries.insert(name.into(), value);
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
    /// A larger `dot` — a filled point sized for hovering / reading ([SPEC §7]). On a
    /// chart line it marks a data point ([CHARTS §3]).
    Circle,
    Diamond,
    Crow,
}

impl MarkerKind {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "none" => Self::None,
            "arrow" => Self::Arrow,
            "dot" => Self::Dot,
            "circle" => Self::Circle,
            "diamond" => Self::Diamond,
            "crow" => Self::Crow,
            _ => return None,
        })
    }

    pub fn from_marker(m: crate::ast::LinkMarker) -> Self {
        match m {
            crate::ast::LinkMarker::None => Self::None,
            crate::ast::LinkMarker::Arrow => Self::Arrow,
            crate::ast::LinkMarker::Crow => Self::Crow,
            crate::ast::LinkMarker::Dot => Self::Dot,
            crate::ast::LinkMarker::Diamond => Self::Diamond,
        }
    }
}

pub struct ResolvedLink {
    pub endpoints: Vec<ResolvedEndpoint>,
    /// The dot-path of the container this link was written in (`""` = the scene
    /// root) — the link's **scope** (SPEC §9). Its scope's `layout` picks the wiring
    /// strategy: a `sequence` scope draws its links as time-row arrows and skips the
    /// orthogonal router (SPEC §10).
    pub scope: String,
    pub attrs: AttrMap,
    /// Names of the `.style`s applied to this link, in source order — emitted as
    /// `lini-style-{name}` classes, exactly like a node's (SPEC §14).
    pub applied_styles: Vec<String>,
    pub markers: Markers,
    /// Link labels (label sugar + body `|text|`s), placed onto the drawn
    /// route by the router's label pass (LINKING §Model step 7).
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

/// Where a label rides its link (SPEC §9): `Auto` distributes it along the
/// route; `Fraction` pins it at an explicit `along:` fraction (0..1).
#[derive(Clone, Debug)]
pub enum Along {
    Auto,
    Fraction(f64),
}

use crate::ast::{LineStyle, Side};
use crate::expr::{Expr, FuncTable};
use crate::span::Span;
use std::collections::{BTreeMap, HashMap};

/// Fully resolved program — output of phase 2.
pub struct Program {
    pub vars: VarTable,
    pub scene: ResolvedScene,
    pub links: Vec<ResolvedLink>,
    pub sheet: SheetInputs,
    /// Stylesheet functions [SPEC 10.7], carried to the layout phase so a chart can
    /// sample a deferred `fn:` once its x-domain is fixed [SPEC 14.3]. Built at
    /// resolve; only the chart layout reads it.
    pub funcs: FuncTable,
    /// Each drawing scope's declared datum letters [SPEC 15.7/15.9], in
    /// declaration order, keyed by the scope's dot-path (`""` = root) — the
    /// identity set `>-` statements and `|datum|` nodes join at resolve. A
    /// `|feature-control|`'s `datums:` validates against it at layout.
    pub datums: HashMap<String, Vec<String>>,
}

/// The render inputs the rules builder restates as CSS class rules — paint rides
/// CSS, geometry bakes [SPEC 17]. After desugar every type/template/define lives
/// as a single-class rule, so this is just those rules' resolved attrs (the
/// generated `.lini-*` type classes and the user `.style` classes, in stylesheet
/// order), the link defaults, and the root inherited-text baseline. Descendant
/// rules (`|.lini-table .lini-box| { }`) carry no entry: their paint bakes inline.
#[derive(Default, Clone)]
pub struct SheetInputs {
    /// Single-class rules in source order: `lini-<type>` (generated type classes,
    /// emitted verbatim) and user classes (emitted `lini-style-<name>`).
    pub class_rules: Vec<(String, AttrMap)>,
    /// Two-class descendant rules `(outer, inner, attrs)` in source order — the
    /// generated mindmap garnish and scoped engine rules among them. Resolve
    /// bakes them into node attrs for layout; render also states their paint as
    /// real CSS and diffs matching elements against it, so a reused look rides
    /// one rule instead of inlining on every wearer [SPEC 17].
    pub descendant_rules: Vec<(String, String, AttrMap)>,
    /// The link layer's defaults (the `.lini-link` rule).
    pub link_defaults: AttrMap,
    /// The root container's `font-size` — the inherited-text baseline for `.lini`
    /// (a baked layout constant carried in the global block, not a CSS var).
    pub root_font_size: f64,
    /// Inherited-text props the global block set, for the `.lini` rule [SPEC 6]:
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
#[derive(Clone)]
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
    /// A `Text` node's own `{ }` style [SPEC 3] — the text-valid props it set,
    /// emitted as a `style=` / `transform` on the `<text>`. Empty for boxes (their
    /// per-node diff is computed at render) and for unstyled text. `attrs` stays
    /// the effective text context (inherited ∪ own) for layout measurement.
    pub own_style: AttrMap,
    /// The effective measurement font [SPEC 5] — kind × weight off the
    /// inherited text context ∪ this node's own props, stamped here because
    /// only resolve sees the full inheritance chain. A text leaf measures its
    /// label with it; an engine scope (chart/sequence/drawing) measures its
    /// generated chrome with the kind, each run's own weight.
    pub font: crate::font::Font,
    pub markers: Markers,
    pub children: Vec<ResolvedInst>,
    pub span: Span,
}

/// One of the built-in primitives. All user shapes resolve to one of these.
/// (There is no title primitive — a caption is just a small-text `|block|` flow
/// child, first in a column for a title or last for a footer, [SPEC 8].)
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
    /// The sketch pen [SPEC 15.3] — a closed primitive folding `draw:` to a
    /// path. Geometry lands per DRAWING-0.16.md stage 2; until then layout reports it.
    Sketch,
}

impl NodeKind {
    /// Every primitive, in `as_str` order — the canonical enumeration desugar
    /// walks to emit a `.lini-<kind>` class def per present primitive.
    pub const ALL: [NodeKind; 13] = [
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
        Self::Sketch,
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
            "sketch" => Self::Sketch,
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
            Self::Sketch => "sketch",
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

/// Whether resolved `attrs` set `layout: drawing` [SPEC 15] — the one check for
/// "this is a drawing scope," shared by the resolve link pass and the layout
/// dispatch.
pub fn is_drawing(attrs: &AttrMap) -> bool {
    matches!(attrs.get("layout"), Some(ResolvedValue::Ident(l)) if l == "drawing")
}

#[derive(Clone, Debug)]
pub enum ResolvedValue {
    Number(f64),
    /// A percentage — `50%`, only valid inside a colour [SPEC 2].
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
    /// resolve, [SPEC 14.3]): one `Expr` for a whole-domain `fn:`, or several for a
    /// per-band list. Sampled to baked numbers at chart layout and never rendered, so
    /// the two exhaustive `ResolvedValue` matches (`render::values::format_value`,
    /// `layout::values::describe`) treat it as unreachable / opaque.
    Deferred(Vec<Expr>),
    /// One `draw:` pen item [SPEC 15.3], held structured for the sketch fold at
    /// layout — a call (args resolved to numbers) that may name its drawn
    /// segment. Like `Deferred`, it is consumed at layout and never rendered.
    PenCall {
        call: ResolvedCall,
        segment: Option<String>,
    },
}

impl ResolvedValue {
    /// The numeric value, if this is a plain number. A `--name` reference is a
    /// visual var [SPEC 10.2], never a layout number, so it has none.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            ResolvedValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// A `light-dark(…)` colour — both a light and a dark arm [SPEC 10]: the
    /// signal that the document is adaptive (needs `color-scheme` + the
    /// `data-theme` toggles).
    pub fn is_light_dark(&self) -> bool {
        matches!(self, ResolvedValue::Call(c) if c.name == "light-dark")
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedCall {
    pub name: String,
    pub args: Vec<ResolvedValue>,
}

/// The visual `--lini-*` variable table ([SPEC 10.2] — vars are visual-only).
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
    /// A larger `dot` — a filled point sized for hovering / reading ([SPEC 7]). On a
    /// chart line it marks a data point ([SPEC 14.2]).
    Circle,
    Diamond,
    /// The filled drafting **datum** triangle ([SPEC 7]) — base on the feature
    /// face, apex toward the leader; what a drawing's `>-` lowers to
    /// ([SPEC 15.7]).
    Datum,
    /// The ER "many" crow's-foot ([SPEC 7]); the four below pair it / a bar with an
    /// optionality ring for the full cardinality set.
    Crow,
    /// ER "one" — a single perpendicular bar.
    One,
    /// ER "exactly one" — a double bar (one and only one).
    ExactlyOne,
    /// ER "zero or one" — an optionality ring + a bar.
    ZeroOrOne,
    /// ER "one or many" — a bar + the crow's foot.
    OneOrMany,
    /// ER "zero or many" — an optionality ring + the crow's foot.
    ZeroOrMany,
}

impl MarkerKind {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "none" => Self::None,
            "arrow" => Self::Arrow,
            "dot" => Self::Dot,
            "circle" => Self::Circle,
            "diamond" => Self::Diamond,
            "datum" => Self::Datum,
            // The ER cardinality family ([SPEC 7]); `many` is an alias of `crow`.
            "crow" | "many" => Self::Crow,
            "one" => Self::One,
            "exactly-one" => Self::ExactlyOne,
            "zero-or-one" => Self::ZeroOrOne,
            "one-or-many" => Self::OneOrMany,
            "zero-or-many" => Self::ZeroOrMany,
            _ => return None,
        })
    }

    /// An open-stroked marker (the ER family) paints via `stroke: inherit` off the
    /// enclosing `<g>`, never a `fill` — unlike the filled heads (arrow / dot / diamond).
    pub fn is_open(self) -> bool {
        matches!(
            self,
            Self::Crow
                | Self::One
                | Self::ExactlyOne
                | Self::ZeroOrOne
                | Self::OneOrMany
                | Self::ZeroOrMany
        )
    }

    pub fn from_marker(m: crate::ast::LinkMarker) -> Self {
        use crate::ast::LinkMarker as L;
        match m {
            L::None => Self::None,
            L::Arrow => Self::Arrow,
            L::Crow => Self::Crow,
            L::Dot => Self::Dot,
            L::Diamond => Self::Diamond,
            L::One => Self::One,
            L::ExactlyOne => Self::ExactlyOne,
            L::ZeroOrOne => Self::ZeroOrOne,
            L::OneOrMany => Self::OneOrMany,
            L::ZeroOrMany => Self::ZeroOrMany,
        }
    }
}

/// A link's wiring strategy ([SPEC 9], ROUTING.md Strategies): `routing:`
/// cascades from the scope like `clearance`. `curved` was removed, replaced
/// by `natural` — smooth curves over the shared corridor search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    Orthogonal,
    Natural,
    Straight,
}

/// What a resolved link statement *is* [SPEC 9, 15]: a wire (routed, or drawn by
/// a sequence / as a drawing's straight annotation or leader), a drawing measure,
/// or a mate. Always `Wire` outside a drawing scope — resolve gates the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    Wire,
    Measure(MeasureOp),
    Mate,
}

/// A drawing measure's reading [SPEC 15.6], one per measuring op: `Linear` from
/// the binary `(-)`, `Round` from the unary `(o)` (⌀ / R by the feature), `Angle`
/// from `(<)`. Classified at resolve from the *operator* — an explicit `marker:`
/// restyles a wire but never re-types a statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasureOp {
    Linear,
    Round,
    Angle,
}

pub struct ResolvedLink {
    pub endpoints: Vec<ResolvedEndpoint>,
    /// Wire, measure, or mate — a drawing scope's layout consumes the non-wire
    /// kinds [SPEC 15]; the router only ever sees wires.
    pub kind: LinkKind,
    /// The dot-path of the container this link was written in (`""` = the scene
    /// root) — the link's **scope** [SPEC 9]. Its scope's `layout` picks the wiring
    /// strategy: a `sequence` scope draws its links as time-row arrows and skips the
    /// orthogonal router [SPEC 13].
    pub scope: String,
    /// The operator's line part (`->` solid · `-->` dashed · `~>` wavy) — the
    /// message's *kind* in a sequence (call / return / async), read here rather than
    /// from `stroke-style`, which a `link-style:` override can change [SPEC 13].
    pub line: LineStyle,
    /// The resolved `routing:` (cascaded, then the link's own block) — which
    /// strategy draws this link's wire.
    pub routing: Strategy,
    pub attrs: AttrMap,
    /// Names of the `.style`s applied to this link, in source order — emitted as
    /// `lini-style-{name}` classes, exactly like a node's [SPEC 17].
    pub applied_styles: Vec<String>,
    pub markers: Markers,
    /// Link labels (label sugar + body `|text|`s), placed onto the drawn
    /// route by the router's label pass (ROUTING Model step 7).
    pub texts: Vec<ResolvedText>,
    /// Annotation nodes carried in a drawing link's `[ ]` [SPEC 15.9] —
    /// resolved like scene children, lowered at the statement's text seat.
    /// Always empty outside a drawing scope (a node there errors at resolve).
    pub carried: Vec<ResolvedInst>,
    /// The statement was written one-ended (one endpoint group, the text as
    /// its far end) — a drawing's callout / unary-measure shape [SPEC 15.6/
    /// 15.7]. A one-ended `&` fan keeps **all** its endpoints on this one
    /// link (one text, one landing, a leg each), so the endpoint count alone
    /// can no longer tell a callout from a two-ended annotation arrow.
    pub one_ended: bool,
    /// A sheet-scope **projection construction link** [SPEC 15.8]: the one
    /// legalized cross-view anchor form (unmarked `-`, each end dot-pathing into
    /// a different view). The router never sees it — layout lowers it to one
    /// straight `|projection|` chrome line between the two resolved anchors,
    /// after `align: origin` has placed the views.
    pub projection: bool,
    pub span: Span,
}

pub struct ResolvedEndpoint {
    /// Fully-qualified dot-path from scene root (e.g. `garden.outlet`).
    pub path: String,
    /// A 1-based pattern-copy index [SPEC 15.4] — `plate.bolt.2`; the anchor
    /// walk steps into the placed copy. Drawing scope only.
    pub copy: Option<usize>,
    pub side: Option<Side>,
    /// A drawing-scope anchor beyond the four sides [SPEC 15.2] — a corner,
    /// `center`, or a sketch-authored segment; `None` everywhere else (the router
    /// vocabulary is `side`).
    pub point: Option<String>,
    pub span: Span,
}

#[derive(Clone)]
pub struct ResolvedText {
    pub text: String,
    pub along: Along,
    pub attrs: AttrMap,
    /// Worn user classes on a link `[ ]` label [SPEC 3] — emitted as
    /// `lini-style-*` on the `<text>`, exactly as a node's text leaf. Empty for
    /// an unclassed label and generated chrome.
    pub applied_styles: Vec<String>,
}

/// Where a label rides its link [SPEC 9]: `Auto` distributes it along the
/// route; `Fraction` pins it at an explicit `along:` fraction (0..1).
#[derive(Clone, Debug)]
pub enum Along {
    Auto,
    Fraction(f64),
}

#[cfg(test)]
mod tests {
    use super::MarkerKind;

    #[test]
    fn parses_the_er_cardinality_family() {
        assert_eq!(MarkerKind::parse("one"), Some(MarkerKind::One));
        assert_eq!(
            MarkerKind::parse("exactly-one"),
            Some(MarkerKind::ExactlyOne)
        );
        assert_eq!(
            MarkerKind::parse("zero-or-one"),
            Some(MarkerKind::ZeroOrOne)
        );
        assert_eq!(
            MarkerKind::parse("one-or-many"),
            Some(MarkerKind::OneOrMany)
        );
        assert_eq!(
            MarkerKind::parse("zero-or-many"),
            Some(MarkerKind::ZeroOrMany)
        );
        // `many` is an alias of `crow`.
        assert_eq!(MarkerKind::parse("many"), Some(MarkerKind::Crow));
        assert_eq!(MarkerKind::parse("crow"), Some(MarkerKind::Crow));
        assert_eq!(MarkerKind::parse("nope"), None);
        // The ER family is open-stroked; the filled heads are not.
        assert!(MarkerKind::One.is_open() && MarkerKind::Crow.is_open());
        assert!(MarkerKind::ExactlyOne.is_open());
        assert!(!MarkerKind::Arrow.is_open() && !MarkerKind::Dot.is_open());
    }
}

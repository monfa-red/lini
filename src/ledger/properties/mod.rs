//! One row per property [SPEC 16]: who owns it, its value shape, where its
//! default lives, how it inherits, and how out-of-scope use is gated. The
//! resolve classifiers read this table; the 0.21 validation pass, schema
//! generation, and generated SPEC tables come next. The `shape` column states
//! the 0.21 comma-law target ([`Shape::List`] reads across comma-groups) —
//! Stage M1 flips the readers to it.

use std::collections::HashMap;
use std::sync::OnceLock;

/// Where a property is honoured [SPEC 16]. A name may have several owners;
/// homonyms (`sheet` on `|page|` vs the ISO 7200 field) list each. The M2
/// validation pass ([`crate::validate`]) is the consumer.
#[derive(Debug)]
pub enum Owner {
    /// Every drawn node, in every layout.
    Universal,
    /// The root's scene config (the stylesheet block).
    Root,
    /// A link's own property.
    Link,
    /// Read on this type (a primitive or template).
    Type(&'static str),
    /// Interpreted by this layout engine
    /// (`flow` | `grid` | `sequence` | `chart` | `pie` | `drawing`).
    Layout(&'static str),
    /// A layout role that is not one type: `series` (line/bars/area/dots/
    /// bubble), `dimension` (the `(-)` link subtype), `mate`, `closed`
    /// (the closed primitives), `title-block` (the ISO 7200 field grid).
    Role(&'static str),
}

/// A value's shape [SPEC 2]: how its comma-groups read, and what each scalar is.
#[derive(Debug)]
pub enum Shape {
    /// One comma-group — a scalar or a space-separated tuple (`translate: 10 -4`).
    One(Kind),
    /// A comma-separated list of groups (`points: 0 0, 10 10`).
    List(Kind),
    /// A `draw:` pen run — structured calls + `:segment` points [SPEC 15.3].
    Pen,
    /// One `grid(…)` / `radial(…)` replication call [SPEC 15.4].
    Pattern,
}

/// The scalar kind inside a [`Shape`].
#[derive(Debug, PartialEq)]
pub enum Kind {
    Number,
    /// A keyword identifier.
    Ident,
    /// Quoted text — free text, a URL, an SVG path [SPEC 2]; a bare word errors.
    Str,
    /// A flat colour (or `--var`) — no gradient.
    Colour,
    /// A paint: colour · `none` · gradient [SPEC 10.3].
    Paint,
    /// A marker glyph name — rides [`crate::resolve::ir::Markers`], not the attr map.
    Marker,
    /// A grid track: `auto` · number · `repeat(…)` [SPEC 12].
    Track,
    /// Positional / mixed forms validated by the property's own reader
    /// (`at: V | X Y`, `tol: t | +u -l | fit`, `sheet: a4 landscape`).
    Any,
}

/// Where a property's default lives — a reference, not the value; defaults
/// stay in their tuning homes.
#[derive(Debug)]
pub enum DefaultRef {
    /// Defaultless — unset until authored (`points`, `symbol`, `data`, `of`, …).
    None,
    /// Stated in [`super::defaults`] (a primitive / template / root / link bundle).
    Bundles,
    /// A code constant or fallback (gathered into [`super::consts`] in Stage R3).
    Engine,
}

/// How a property flows down the tree.
#[derive(Debug, PartialEq)]
pub enum Inherit {
    No,
    /// The resolve text channel — nearest ancestor wins [SPEC 6].
    Text,
    /// Scene config a link takes from its scope [SPEC 9].
    ScopeLink,
    /// Nearest-wins inside a layout engine (`scale` [SPEC 15.1]), not resolve.
    Engine,
}

/// Out-of-scope treatment [SPEC 16]: inert by default; a handful hard-error
/// ([SPEC 20] — the sequence placement props, the drawing measurements).
#[derive(Debug)]
pub enum Gate {
    Lenient,
    Hard,
}

/// One property row. `text` marks the subset valid on a bare text leaf's own
/// `{ }` [SPEC 3]; `baked` the baked-spacing text props that compile into
/// glyph / line positions and are never live CSS [SPEC 6]; `deferred` a row
/// honoured only in part — its full reader is deferred ([SPEC 23]), so the
/// generated schema surfaces it as a deferred flag.
// `default` and `gate` are data for schema generation and later validation
// depth; today the tests pin them.
#[allow(dead_code)]
#[derive(Debug)]
pub struct Property {
    pub name: &'static str,
    pub owners: &'static [Owner],
    pub shape: Shape,
    pub default: DefaultRef,
    pub inherit: Inherit,
    pub text: bool,
    pub baked: bool,
    pub gate: Gate,
    pub deferred: bool,
}

const fn row(
    name: &'static str,
    owners: &'static [Owner],
    shape: Shape,
    default: DefaultRef,
    inherit: Inherit,
) -> Property {
    Property {
        name,
        owners,
        shape,
        default,
        inherit,
        text: false,
        baked: false,
        gate: Gate::Lenient,
        deferred: false,
    }
}

impl Property {
    /// Mark valid on a bare text leaf [SPEC 3].
    const fn text(mut self) -> Self {
        self.text = true;
        self
    }
    /// Mark a baked-spacing text prop [SPEC 6].
    const fn baked(mut self) -> Self {
        self.baked = true;
        self
    }
    /// Mark a hard out-of-scope gate [SPEC 16/20].
    const fn hard(mut self) -> Self {
        self.gate = Gate::Hard;
        self
    }
    /// Mark a partly-honoured row whose full reader is deferred [SPEC 23].
    const fn deferred(mut self) -> Self {
        self.deferred = true;
        self
    }

    /// Whether the row carries a **node** owner (a type/role a node can be, or
    /// `Universal`) beyond any link/scene-config role. `format` does
    /// (chart/axis/series/drawing); pure scene config (`clearance`/`routing`)
    /// does not. Validation reads the owners for the former, so a scope-link
    /// property is not blanket-accepted on every node by its inherit channel
    /// [SPEC 16].
    pub fn has_node_owner(&self) -> bool {
        self.owners.iter().any(|o| {
            matches!(o, Owner::Universal | Owner::Type(_))
                || matches!(o, Owner::Role(r) if *r != "dimension" && *r != "mate")
        })
    }
}

use DefaultRef::{Bundles, Engine};
use Inherit::{No, ScopeLink, Text};
use Owner::{Layout, Link, Role, Root, Type, Universal};
use Shape::{List, One};

const UNIVERSAL: &[Owner] = &[Universal];

/// Every property, grouped as in SPEC 16. Row order fixes the derived
/// iterators ([`inherited_text`], [`scope_link_props`]).
pub static PROPERTIES: &[Property] = &[
    // ── Paint & stroke [SPEC 6] ──
    row("fill", UNIVERSAL, One(Kind::Paint), Bundles, No).text(),
    row("opacity", UNIVERSAL, One(Kind::Number), Engine, No).text(),
    row("stroke", UNIVERSAL, One(Kind::Paint), Bundles, No),
    row("stroke-width", UNIVERSAL, One(Kind::Number), Bundles, No),
    row("stroke-style", UNIVERSAL, One(Kind::Ident), Bundles, No),
    row("radius", UNIVERSAL, One(Kind::Number), Bundles, No),
    row("shadow", UNIVERSAL, One(Kind::Any), DefaultRef::None, No),
    row(
        "gap-fill",
        &[Layout("flow"), Layout("grid"), Layout("sequence")],
        One(Kind::Paint),
        Bundles,
        No,
    ),
    // ── Text [SPEC 6] — the `Inherit::Text` rows, in the channel's order.
    //    (`text-shadow` rides the Universal Text table in SPEC 16.) ──
    row("font-family", UNIVERSAL, One(Kind::Any), Engine, Text).text(),
    row("font-size", UNIVERSAL, One(Kind::Number), Bundles, Text)
        .text()
        .baked(),
    // `normal|medium|semibold|bold` or `400|500|600|700` [SPEC 6] — ident or number.
    row("font-weight", UNIVERSAL, One(Kind::Any), Bundles, Text).text(),
    row("font-style", UNIVERSAL, One(Kind::Ident), Engine, Text).text(),
    row("text-transform", UNIVERSAL, One(Kind::Ident), Engine, Text).text(),
    row("text-decoration", UNIVERSAL, One(Kind::Ident), Engine, Text).text(),
    row(
        "text-shadow",
        UNIVERSAL,
        One(Kind::Any),
        DefaultRef::None,
        Text,
    )
    .text(),
    row("letter-spacing", UNIVERSAL, One(Kind::Number), Engine, Text)
        .text()
        .baked(),
    row("line-spacing", UNIVERSAL, One(Kind::Number), Engine, Text)
        .text()
        .baked(),
    row("color", UNIVERSAL, One(Kind::Colour), Engine, Text).text(),
    // ── Box model & placement [SPEC 5] ──
    row("width", UNIVERSAL, One(Kind::Any), Engine, No),
    row("height", UNIVERSAL, One(Kind::Any), Engine, No),
    row("padding", UNIVERSAL, One(Kind::Number), Bundles, No),
    // `max-width` caps an auto width; `text-wrap` says whether text inside
    // breaks to honour it — inert without a cap [SPEC 5].
    row(
        "max-width",
        UNIVERSAL,
        One(Kind::Number),
        DefaultRef::None,
        No,
    ),
    row(
        "text-wrap",
        UNIVERSAL,
        One(Kind::Ident),
        DefaultRef::None,
        No,
    ),
    row("pin", UNIVERSAL, One(Kind::Ident), Engine, No),
    row(
        "translate",
        UNIVERSAL,
        One(Kind::Number),
        DefaultRef::None,
        No,
    )
    .text(),
    row("rotate", UNIVERSAL, One(Kind::Number), Engine, No).text(),
    row("layer", UNIVERSAL, One(Kind::Number), Engine, No).text(),
    // Homonym: any node's px-per-unit ratio (number, nearest-wins [SPEC 15.1])
    // and an `|axis|`'s `linear` / `log` [SPEC 14.4].
    row(
        "scale",
        &[Universal, Type("axis")],
        One(Kind::Any),
        Bundles,
        Inherit::Engine,
    ),
    row("pattern", UNIVERSAL, Shape::Pattern, DefaultRef::None, No),
    // ── Media & accessibility ──
    row(
        "href",
        &[Universal, Link],
        One(Kind::Str),
        DefaultRef::None,
        No,
    ),
    row("hint", UNIVERSAL, One(Kind::Str), DefaultRef::None, No),
    // ── Type-owned [SPEC 7] ──
    row(
        "points",
        &[Type("line"), Type("poly")],
        List(Kind::Number),
        DefaultRef::None,
        No,
    ),
    row(
        "samples",
        &[Type("line"), Type("poly"), Type("chart")],
        One(Kind::Number),
        Engine,
        No,
    ),
    row(
        "path",
        &[Type("path")],
        One(Kind::Str),
        DefaultRef::None,
        No,
    ),
    row(
        "src",
        &[Type("image")],
        One(Kind::Str),
        DefaultRef::None,
        No,
    ),
    // `symbol` is a homonym [SPEC 16]: the icon's Phosphor name, and the
    // finish vee variant on `|surface-finish|` ([SPEC 15.9]).
    row(
        "symbol",
        &[Type("icon"), Type("surface-finish")],
        One(Kind::Ident),
        DefaultRef::None,
        No,
    ),
    row(
        "fit",
        &[Type("icon"), Type("image")],
        One(Kind::Ident),
        Bundles,
        No,
    ),
    row("skew", &[Type("slant")], One(Kind::Number), Bundles, No),
    row(
        "stack",
        &[Role("closed")],
        One(Kind::Number),
        DefaultRef::None,
        No,
    ),
    // Markers read on `|line|`s and links [SPEC 7], and on a chart's series
    // and `|mark|` points (the centred forms, [SPEC 14.2/14.5]).
    row(
        "marker",
        &[Type("line"), Type("mark"), Role("series"), Link],
        One(Kind::Marker),
        Engine,
        No,
    ),
    row(
        "marker-start",
        &[Type("line"), Type("mark"), Role("series"), Link],
        One(Kind::Marker),
        Engine,
        No,
    ),
    row(
        "marker-end",
        &[Type("line"), Type("mark"), Role("series"), Link],
        One(Kind::Marker),
        Engine,
        No,
    ),
    row("draw", &[Type("sketch")], Shape::Pen, DefaultRef::None, No),
    row(
        "mirror",
        &[Type("sketch")],
        One(Kind::Any),
        DefaultRef::None,
        No,
    ),
    row(
        "revolve",
        &[Type("sketch")],
        One(Kind::Ident),
        DefaultRef::None,
        No,
    ),
    row(
        "thread",
        &[Type("sketch"), Type("hole")],
        List(Kind::Any),
        DefaultRef::None,
        No,
    ),
    // `|page|` size sugar (`a4 landscape` [SPEC 15.8]); the former ISO 7200
    // field homonym is `sheet-number` now.
    row("sheet", &[Type("page")], One(Kind::Any), Engine, No),
    row(
        "break",
        &[Type("sketch")],
        List(Kind::Any),
        DefaultRef::None,
        No,
    ),
    // ── Layout & grid [SPEC 11/12] ──
    row("layout", UNIVERSAL, One(Kind::Ident), Bundles, No),
    row(
        "direction",
        &[Layout("flow"), Layout("chart"), Layout("tree")],
        One(Kind::Ident),
        Engine,
        No,
    ),
    // A dimension takes no `gap:` — it stands off by `clearance` [SPEC 15.6/20].
    row(
        "gap",
        &[
            Layout("flow"),
            Layout("grid"),
            Layout("sequence"),
            Layout("chart"),
            Layout("pie"),
            Layout("tree"),
            Role("mate"),
        ],
        One(Kind::Number),
        Bundles,
        No,
    ),
    row(
        "align",
        &[Layout("flow"), Layout("grid")],
        List(Kind::Ident),
        Bundles,
        No,
    ),
    row(
        "justify",
        &[Layout("flow"), Layout("grid")],
        List(Kind::Ident),
        Bundles,
        No,
    ),
    row("columns", &[Layout("grid")], List(Kind::Track), Bundles, No),
    row(
        "rows",
        &[Layout("grid")],
        List(Kind::Track),
        DefaultRef::None,
        No,
    ),
    // `cell`/`span` hard-gate off a grid [SPEC 12/16, decision 3] — off-grid is
    // an error, not silently inert (the `span`-on-a-`|band|` exception aside).
    row(
        "cell",
        &[Layout("grid")],
        One(Kind::Number),
        DefaultRef::None,
        No,
    )
    .hard(),
    row(
        "span",
        &[Layout("grid"), Type("band")],
        One(Kind::Number),
        DefaultRef::None,
        No,
    )
    .hard(),
    // ── Charts [SPEC 14] ──
    row(
        "data",
        &[Role("series")],
        List(Kind::Number),
        DefaultRef::None,
        No,
    ),
    row(
        "fn",
        &[Role("series")],
        List(Kind::Any),
        DefaultRef::None,
        No,
    ),
    // The series' per-datum text [SPEC 14.3] (0.20's `tags:`); the deferred
    // per-axis tick text keeps no property name (S2 — alpha.2 names it).
    row(
        "labels",
        &[Role("series")],
        List(Kind::Str),
        DefaultRef::None,
        No,
    ),
    row(
        "curve",
        &[Type("line"), Type("area")],
        One(Kind::Ident),
        Engine,
        No,
    ),
    row("baseline", &[Type("area")], One(Kind::Number), Engine, No),
    row(
        "axis",
        &[Role("series"), Type("mark"), Type("band")],
        One(Kind::Ident),
        DefaultRef::None,
        No,
    ),
    row("bars", &[Type("chart")], One(Kind::Ident), Engine, No),
    row("categories", &[Type("chart")], List(Kind::Str), Engine, No),
    row("hole", &[Type("pie")], One(Kind::Number), Engine, No),
    // The auto-legend (≥ 2 entries) is built [SPEC 14.6]; the `legend:`
    // placement / suppression reader is deferred [SPEC 23], so the row is
    // marked `deferred` — the schema states it truthfully. Building the reader
    // is a later minor's work.
    row(
        "legend",
        &[Type("chart"), Type("pie")],
        One(Kind::Any),
        Engine,
        No,
    )
    .deferred(),
    row(
        "tooltip",
        &[Type("chart"), Type("pie"), Role("series")],
        One(Kind::Any),
        Engine,
        No,
    ),
    row(
        "value",
        &[Type("slice"), Type("bubble")],
        One(Kind::Number),
        DefaultRef::None,
        No,
    ),
    row(
        "at",
        &[Type("mark"), Type("bubble"), Type("plane")],
        One(Kind::Any),
        DefaultRef::None,
        No,
    ),
    // Homonym: an `|axis|`'s side [SPEC 14.4], a dimension's / callout's seat
    // [SPEC 15.6], and a first-level `|topic|`'s bilateral half [SPEC 12].
    row(
        "side",
        &[Type("axis"), Type("topic"), Role("dimension")],
        One(Kind::Ident),
        Engine,
        No,
    ),
    row(
        "range",
        &[Type("axis")],
        One(Kind::Number),
        DefaultRef::None,
        No,
    ),
    row(
        "step",
        &[Type("axis")],
        One(Kind::Number),
        DefaultRef::None,
        No,
    ),
    row(
        "ticks",
        &[Type("axis")],
        List(Kind::Number),
        DefaultRef::None,
        No,
    ),
    // Homonym: a drawing scope's unit is the ident enum `mm cm m in`
    // (inherits nearest-wins, folded by desugar [SPEC 15.1]); an `|axis|`'s is
    // its quoted tick suffix [SPEC 14.4] — each reader validates its own.
    row(
        "unit",
        &[Type("drawing"), Type("axis")],
        One(Kind::Any),
        DefaultRef::None,
        No,
    ),
    row("gridlines", &[Type("axis")], One(Kind::Any), Engine, No),
    // Value presentation [SPEC 16] — parsed by `ledger::format`. A dual-channel
    // row: two genuinely different cascades over one property.
    //   • chart leg (chart / pie / axis / series): **engine-read** — the chart
    //     threads its own `format:` down as the axes' and series' fallback,
    //     exactly as `tooltip:` is read per-node. No resolve channel.
    //   • drawing leg (drawing scope / dimension): rides the **scope-link**
    //     channel — drawing scope → `(-)` rule → class → the dimension's block,
    //     exactly as `clearance` does [SPEC 15.6].
    // The row's single resolve channel is therefore `ScopeLink` (the only leg
    // that uses one). Validation reads the **owners** — not the inherit channel
    // — for a scope-link property that has node owners, so `format:` is accepted
    // on its owners and errors on a plain box; schema generation reads both legs
    // off `owners × ScopeLink` by construction (no per-owner-inherit split).
    row(
        "format",
        &[
            Type("chart"),
            Type("pie"),
            Type("axis"),
            Role("series"),
            Type("drawing"),
            Role("dimension"),
        ],
        One(Kind::Any),
        Engine,
        ScopeLink,
    ),
    // ── Sequence [SPEC 13] — the placement props hard-gate off a sequence
    //    [SPEC 16/20]. ──
    row(
        "place",
        &[Type("note")],
        One(Kind::Any),
        DefaultRef::None,
        No,
    )
    .hard(),
    row(
        "activation",
        &[Layout("sequence")],
        One(Kind::Ident),
        Engine,
        No,
    )
    .hard(),
    // ── Drawing [SPEC 15] ──
    // `tol` is a homonym [SPEC 16]: a dimension's tolerance forms, and a
    // control row's zone width (a number > 0) on `|feature-control|` /
    // `|control|` ([SPEC 15.9]).
    row(
        "tol",
        &[Role("dimension"), Type("feature-control"), Type("control")],
        One(Kind::Any),
        DefaultRef::None,
        No,
    )
    .hard(),
    // The GD&T control-row properties [SPEC 15.9] — each owns one frame
    // compartment slot; the validity table is enforced at the frame lowering.
    row(
        "characteristic",
        &[Type("feature-control"), Type("control")],
        One(Kind::Ident),
        DefaultRef::None,
        No,
    ),
    row(
        "zone",
        &[Type("feature-control"), Type("control")],
        One(Kind::Ident),
        DefaultRef::None,
        No,
    ),
    row(
        "material",
        &[Type("feature-control"), Type("control")],
        One(Kind::Ident),
        DefaultRef::None,
        No,
    ),
    row(
        "datums",
        &[Type("feature-control"), Type("control")],
        List(Kind::Any),
        DefaultRef::None,
        No,
    ),
    row(
        "modifiers",
        &[Type("feature-control"), Type("control")],
        List(Kind::Any),
        DefaultRef::None,
        No,
    ),
    // A `(-)` dimension's axis override [SPEC 15.6] — `horizontal` /
    // `vertical` / `aligned`; must agree with a directed anchor.
    row(
        "project",
        &[Role("dimension")],
        One(Kind::Ident),
        DefaultRef::None,
        No,
    )
    .hard(),
    row("facing", &[Type("plane")], One(Kind::Ident), Engine, No),
    row(
        "of",
        &[Type("drawing")],
        One(Kind::Ident),
        DefaultRef::None,
        No,
    ),
    // The ISO 7200 title-block fields [SPEC 15.8] (`sheet` rides its homonym
    // row above).
    row(
        "title",
        &[Role("title-block")],
        One(Kind::Str),
        DefaultRef::None,
        No,
    ),
    row(
        "drawing-number",
        &[Role("title-block")],
        One(Kind::Str),
        DefaultRef::None,
        No,
    ),
    row(
        "revision",
        &[Role("title-block")],
        One(Kind::Str),
        DefaultRef::None,
        No,
    ),
    row(
        "sheet-number",
        &[Role("title-block")],
        One(Kind::Str),
        DefaultRef::None,
        No,
    ),
    row(
        "date",
        &[Role("title-block")],
        One(Kind::Str),
        DefaultRef::None,
        No,
    ),
    row(
        "author",
        &[Role("title-block")],
        One(Kind::Str),
        DefaultRef::None,
        No,
    ),
    row(
        "approved",
        &[Role("title-block")],
        One(Kind::Str),
        DefaultRef::None,
        No,
    ),
    row(
        "department",
        &[Role("title-block")],
        One(Kind::Str),
        DefaultRef::None,
        No,
    ),
    row(
        "reference",
        &[Role("title-block")],
        One(Kind::Str),
        DefaultRef::None,
        No,
    ),
    row(
        "document-type",
        &[Role("title-block")],
        One(Kind::Str),
        DefaultRef::None,
        No,
    ),
    row(
        "status",
        &[Role("title-block")],
        One(Kind::Str),
        DefaultRef::None,
        No,
    ),
    // The root's px-per-mm for sheets [SPEC 15.1] — scene config only.
    row("density", &[Root], One(Kind::Number), Engine, No),
    // ── Links [SPEC 9] — clearance before routing (the scope-config order).
    //    On a dimension, `clearance` is the packing stand-off minimum
    //    [SPEC 15.6]. ──
    row(
        "clearance",
        &[Link, Root, Role("dimension")],
        One(Kind::Number),
        Bundles,
        ScopeLink,
    ),
    row(
        "routing",
        &[Link, Root],
        One(Kind::Ident),
        Engine,
        ScopeLink,
    ),
    row("along", &[Link], List(Kind::Number), Engine, No),
];

/// The value **builders** [SPEC 10.3, 12]: calls that stay a typed value (a
/// colour, gradient, track repeat, or hatch) for render / layout; every other
/// call is compute and folds to a number. Schema generation and editor
/// grammars consume this list too.
pub const BUILDER_CALLS: &[&str] = &[
    "oklch",
    "gradient",
    "linear-gradient",
    "radial-gradient",
    "rgb",
    "rgba",
    "hsl",
    "hsla",
    "repeat",
    "hatch",
];

fn index() -> &'static HashMap<&'static str, &'static Property> {
    static INDEX: OnceLock<HashMap<&'static str, &'static Property>> = OnceLock::new();
    INDEX.get_or_init(|| PROPERTIES.iter().map(|p| (p.name, p)).collect())
}

/// The property's row, if the name is known.
pub fn get(name: &str) -> Option<&'static Property> {
    index().get(name).copied()
}

/// Whether a call name is a value builder (vs a compute call).
pub fn is_builder_call(name: &str) -> bool {
    BUILDER_CALLS.contains(&name)
}

/// Whether the property's value is literal **text** and must be quoted [SPEC 2].
pub fn is_string_valued(name: &str) -> bool {
    get(name).is_some_and(|p| matches!(p.shape, One(Kind::Str) | List(Kind::Str)))
}

/// Whether the name is a marker prop — extracted into `Markers`, not the attr map.
pub fn is_marker(name: &str) -> bool {
    get(name).is_some_and(|p| matches!(p.shape, One(Kind::Marker)))
}

/// Whether the property is valid on a bare text leaf's own `{ }` [SPEC 3/6].
pub fn is_text_valid(name: &str) -> bool {
    get(name).is_some_and(|p| p.text)
}

/// Whether the property is a baked-spacing text prop — layout, never live CSS
/// [SPEC 6].
pub fn is_baked_text(name: &str) -> bool {
    get(name).is_some_and(|p| p.baked)
}

/// The text properties that cascade to descendant text [SPEC 6], in channel order.
pub fn inherited_text() -> impl Iterator<Item = &'static str> {
    PROPERTIES
        .iter()
        .filter(|p| p.inherit == Inherit::Text)
        .map(|p| p.name)
}

/// The scene-config properties a link takes from its scope [SPEC 9].
pub fn scope_link_props() -> impl Iterator<Item = &'static str> {
    PROPERTIES
        .iter()
        .filter(|p| p.inherit == Inherit::ScopeLink)
        .map(|p| p.name)
}

#[cfg(test)]
mod tests;

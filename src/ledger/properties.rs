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
/// glyph / line positions and are never live CSS [SPEC 6].
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
    //    `text-shadow` is honoured but missing from SPEC 16 (cross-check, S1). ──
    row("font-family", UNIVERSAL, One(Kind::Any), Engine, Text).text(),
    row("font-size", UNIVERSAL, One(Kind::Number), Bundles, Text)
        .text()
        .baked(),
    row("font-weight", UNIVERSAL, One(Kind::Ident), Bundles, Text).text(),
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
    row(
        "symbol",
        &[Type("icon")],
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
        &[Layout("flow"), Layout("chart")],
        One(Kind::Ident),
        Engine,
        No,
    ),
    row(
        "gap",
        &[
            Layout("flow"),
            Layout("grid"),
            Layout("sequence"),
            Layout("chart"),
            Layout("pie"),
            Role("dimension"),
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
    row(
        "cell",
        &[Layout("grid")],
        One(Kind::Number),
        DefaultRef::None,
        No,
    ),
    row(
        "span",
        &[Layout("grid"), Type("band")],
        One(Kind::Number),
        DefaultRef::None,
        No,
    ),
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
    // SPEC 16 marks `legend:` honoured; no reader exists yet (cross-check, S1/S2).
    row(
        "legend",
        &[Type("chart"), Type("pie")],
        One(Kind::Any),
        Engine,
        No,
    ),
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
    // Homonym: an `|axis|`'s side [SPEC 14.4] and a dimension's / callout's
    // seat [SPEC 15.6].
    row(
        "side",
        &[Type("axis"), Role("dimension")],
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
    row(
        "tol",
        &[Role("dimension")],
        One(Kind::Any),
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
    // ── Links [SPEC 9] — clearance before routing (the scope-config order). ──
    row(
        "clearance",
        &[Link, Root],
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
mod tests {
    use super::*;
    use crate::resolve::NodeKind;

    #[test]
    fn every_name_is_unique() {
        let mut seen = std::collections::HashSet::new();
        for p in PROPERTIES {
            assert!(seen.insert(p.name), "duplicate ledger row: {}", p.name);
        }
    }

    /// Acceptance (Stage R2): every property name that appears in the
    /// bundles-moved defaults exists in the ledger.
    #[test]
    fn every_bundled_default_has_a_row() {
        use super::super::defaults::*;
        let mut decls = Vec::new();
        for kind in [
            NodeKind::Block,
            NodeKind::Oval,
            NodeKind::Hex,
            NodeKind::Cyl,
            NodeKind::Diamond,
            NodeKind::Slant,
            NodeKind::Poly,
            NodeKind::Path,
            NodeKind::Sketch,
            NodeKind::Line,
            NodeKind::Icon,
            NodeKind::Text,
            NodeKind::Image,
        ] {
            decls.extend(primitive_bundle(kind));
        }
        for (name, _) in crate::desugar::types::TEMPLATES {
            decls.extend(template_bundle(name));
        }
        decls.extend(root_defaults());
        decls.extend(link_defaults());
        for layout in [Some("sequence"), Some("drawing"), None] {
            decls.extend(root_layout_defaults(layout));
        }
        for d in decls {
            assert!(
                get(&d.name).is_some(),
                "bundled default '{}' has no ledger row",
                d.name
            );
        }
    }

    /// Acceptance (Stage R2): the five classifiers' pre-migration sets fall out
    /// of the ledger unchanged — membership and order alike.
    #[test]
    fn classifier_sets_match_the_legacy_lists() {
        // resolve/scene.rs INHERITED_TEXT (order included).
        assert_eq!(
            inherited_text().collect::<Vec<_>>(),
            [
                "font-family",
                "font-size",
                "font-weight",
                "font-style",
                "text-transform",
                "text-decoration",
                "text-shadow",
                "letter-spacing",
                "line-spacing",
                "color",
            ]
        );
        // resolve/scene.rs BAKED_TEXT.
        assert_eq!(
            PROPERTIES
                .iter()
                .filter(|p| p.baked)
                .map(|p| p.name)
                .collect::<Vec<_>>(),
            ["font-size", "letter-spacing", "line-spacing"]
        );
        // resolve/scene.rs is_text_prop.
        let legacy_text = [
            "color",
            "fill",
            "opacity",
            "font-family",
            "font-size",
            "font-weight",
            "font-style",
            "text-transform",
            "text-decoration",
            "text-shadow",
            "letter-spacing",
            "line-spacing",
            "translate",
            "rotate",
            "layer",
        ];
        for name in legacy_text {
            assert!(is_text_valid(name), "'{name}' lost text validity");
        }
        assert_eq!(
            PROPERTIES.iter().filter(|p| p.text).count(),
            legacy_text.len(),
            "a property gained text validity the legacy classifier did not have"
        );
        // resolve/merge.rs is_marker_attr.
        for name in ["marker", "marker-start", "marker-end"] {
            assert!(is_marker(name));
        }
        assert!(!is_marker("stroke"));
        // resolve/program.rs SCOPE_LINK_PROPS (order included).
        assert_eq!(
            scope_link_props().collect::<Vec<_>>(),
            ["clearance", "routing"]
        );
        // resolve/value.rs is_string_valued. The ledger adds the ISO 7200
        // fields (`drawing-number: x` now errors toward quoting instead of dying silently
        // — they are SPEC-16 string-valued; desugar consumes the quoted ones
        // before resolve ever sees them).
        for name in [
            "title",
            "hint",
            "href",
            "src",
            "path",
            "categories",
            "labels",
        ] {
            assert!(is_string_valued(name), "'{name}' lost string-valuedness");
        }
        assert!(!is_string_valued("symbol"));
        assert!(!is_string_valued("font-family"));
        // resolve/value.rs is_builder + the pen / pattern special-cases.
        for name in ["oklch", "rgb", "repeat", "hatch", "linear-gradient"] {
            assert!(is_builder_call(name));
        }
        assert!(!is_builder_call("min"));
        assert!(matches!(get("draw").map(|p| &p.shape), Some(Shape::Pen)));
        assert!(matches!(
            get("pattern").map(|p| &p.shape),
            Some(Shape::Pattern)
        ));
    }

    /// The defaultless names AUDIT R1 called out all have rows.
    #[test]
    fn defaultless_names_are_covered() {
        for name in [
            "points", "symbol", "data", "cell", "of", "at", "tol", "draw",
        ] {
            let p = get(name).unwrap_or_else(|| panic!("'{name}' has no ledger row"));
            assert!(
                matches!(p.default, DefaultRef::None),
                "'{name}' should be defaultless"
            );
        }
    }

    /// The 0.21 rename [Stage M3]: `labels` is the series' per-datum text
    /// (0.20's `tags:`, gone), and the deferred per-axis tick text keeps no
    /// property name (S2).
    #[test]
    fn labels_is_the_series_per_datum_text() {
        let labels = get("labels").unwrap();
        assert!(matches!(labels.shape, List(Kind::Str)));
        assert!(
            labels
                .owners
                .iter()
                .any(|o| matches!(o, Owner::Role("series")))
        );
        assert!(get("tags").is_none(), "'tags' was renamed to 'labels'");
        assert!(get("over").is_none(), "'over' was replaced by 'place'");
    }
}

//! Ledger table unit tests — the schema's acceptance checks, split from
//! the data table [`super`] they validate.

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
    // resolve/program.rs SCOPE_LINK_PROPS (order included; `format` joined
    // with its drawing owners [CHART-DRAW Stage 8]).
    assert_eq!(
        scope_link_props().collect::<Vec<_>>(),
        ["format", "clearance", "routing"]
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

/// Beta Stage 0 reconciliation: the `legend` row is marked deferred (its
/// placement/suppression reader is [SPEC 23], only the auto-legend is
/// built); nothing else is.
#[test]
fn legend_is_the_only_deferred_row() {
    assert!(get("legend").unwrap().deferred);
    assert_eq!(
        PROPERTIES
            .iter()
            .filter(|p| p.deferred)
            .map(|p| p.name)
            .collect::<Vec<_>>(),
        ["legend"]
    );
}

/// Beta Stage 0 reconciliation: `format`'s dual cascade reads honestly.
/// Its single resolve channel is `ScopeLink` (the drawing leg), it keeps
/// both legs' owners, and it is a scope-link property with node owners — so
/// validation reads its owners, not the blanket inherit channel. Pure
/// scene config (`clearance`/`routing`) has no node owner.
#[test]
fn format_is_a_dual_channel_row() {
    let format = get("format").unwrap();
    assert_eq!(format.inherit, Inherit::ScopeLink);
    // the chart leg and the drawing leg both present in the owners.
    for owner in ["chart", "pie", "axis", "drawing"] {
        assert!(
            format
                .owners
                .iter()
                .any(|o| matches!(o, Owner::Type(t) if *t == owner)),
            "'format' lost its '{owner}' owner"
        );
    }
    assert!(
        format
            .owners
            .iter()
            .any(|o| matches!(o, Owner::Role("series") | Owner::Role("dimension")))
    );
    // has_node_owner distinguishes format (node owners) from pure config.
    assert!(format.has_node_owner());
    assert!(!get("clearance").unwrap().has_node_owner());
    assert!(!get("routing").unwrap().has_node_owner());
}

//! The grammar's word sets — the **single source** every generated home reads.
//!
//! Each set is derived from whatever table already owns it: the primitive and
//! template type tables, `PROPERTIES` / `BUILDER_CALLS` off the ledger, the
//! layout names off the owner column, the marker glyphs off
//! [`MarkerKind::NAMES`], the sides off [`Side::name`]. Only two sets have no
//! owning table — the contextual value keywords and the CSS colour names — and
//! they are stated **once**, here, for all three homes.

use crate::ast::Side;
use crate::desugar::types::TEMPLATES;
use crate::ledger::properties::{BUILDER_CALLS, Owner, PROPERTIES};
use crate::resolve::{MarkerKind, NodeKind};

/// The primitives that are written between the identity bars (`|box|`, `|sketch|`).
/// `text` — a bare `"…"` leaf, never `|text|` — is excluded; it is not a bar type.
fn primitive_types() -> Vec<&'static str> {
    NodeKind::ALL
        .iter()
        .map(|k| k.as_str())
        .filter(|k| *k != "text")
        .collect()
}

/// Every built-in type name: the writable primitives plus the templates, sorted
/// and deduped — the alternation the `|type|` bars highlight.
pub fn types() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = primitive_types();
    names.extend(TEMPLATES.iter().map(|(name, _)| *name));
    names.sort_unstable();
    names.dedup();
    names
}

/// Every ledger property name, sorted and deduped — the `key:` names that get
/// the strong property scope (an unknown `key:` still highlights, but weakly).
pub fn properties() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = PROPERTIES.iter().map(|p| p.name).collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// The value **builders** [SPEC 10.3] — calls that stay a typed value; sorted.
pub fn builder_calls() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = BUILDER_CALLS.to_vec();
    names.sort_unstable();
    names
}

/// The layout-engine names off the owner column — the `layout:` values, sorted.
pub fn layouts() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = PROPERTIES
        .iter()
        .flat_map(|p| p.owners.iter())
        .filter_map(|o| match o {
            Owner::Layout(l) => Some(*l),
            _ => None,
        })
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// The marker-glyph spellings off [`MarkerKind::NAMES`] — `marker-start:` /
/// `marker-end:` values, deduped (`many` aliases `crow`).
pub fn marker_names() -> Vec<&'static str> {
    MarkerKind::NAMES.iter().map(|(name, _)| *name).collect()
}

/// The forced-side names off [`Side::name`] — one home for the `#side` rule and
/// the property rules' glued-side guard, so an endpoint `a:left` is a side, not
/// a property named `a` with value `left` [SPEC 23].
pub fn side_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = Side::RANK.iter().map(|s| s.name()).collect();
    names.sort_unstable();
    names
}

/// The enum value idents highlighted as constants in value position that no
/// table owns — they live in the readers' `parse` arms, so they are stated once
/// here: flow/grid/tree direction and placement, the booleans and sentinels,
/// stroke styles, corner and outline styles, scales, tooltip modes, routing
/// strategies, `fit` modes, the revolve axes, and the sides. The glyph names
/// ([`marker_names`]) and the layout names ([`layouts`]) are appended by
/// [`value_keywords`] from their own tables.
const CONTEXTUAL_KEYWORDS: &[&str] = &[
    // flow / grid / tree direction and placement
    "flow",
    "row",
    "column",
    "radial",
    "bilateral",
    "start",
    "center",
    "end",
    "stretch",
    "evenly",
    "between",
    "around",
    "rows",
    "columns",
    "all",
    // booleans and the empty/auto sentinels
    "true",
    "false",
    "none",
    "auto",
    // stroke styles — the four line styles plus the two drafting conventions
    "solid",
    "dashed",
    "dotted",
    "wavy",
    "phantom",
    // corner / outline styles
    "outlined",
    "filled",
    "rounded",
    "sharp",
    // scales and tooltip modes
    "log",
    "linear",
    "hover",
    "always",
    // routing strategies [ROUTING.md]
    "orthogonal",
    "natural",
    "straight",
    // `fit:` modes beyond auto / stretch [SPEC 7]
    "contain",
    "cover",
    // the `revolve:` axes [SPEC 15.3]
    "x-axis",
    "y-axis",
    // `over` joins the sides in value position [SPEC 13]
    "over",
];

/// The CSS colour names understood alongside the OKLCH `--hue` palette vars.
pub const COLOR_NAMES: &[&str] = &[
    "white",
    "black",
    "red",
    "green",
    "blue",
    "gray",
    "grey",
    "crimson",
    "orange",
    "yellow",
    "gold",
    "silver",
    "navy",
    "teal",
    "purple",
    "pink",
    "brown",
    "cyan",
    "magenta",
    "lime",
    "maroon",
    "olive",
    "cornflowerblue",
    "currentColor",
    "transparent",
];

/// Every ident highlighted as a constant in value position: the contextual
/// vocabulary, the marker glyphs, the sides, and the ledger layout names —
/// first-stated order kept, duplicates dropped.
pub fn value_keywords() -> Vec<&'static str> {
    let mut words: Vec<&'static str> = CONTEXTUAL_KEYWORDS.to_vec();
    for word in marker_names()
        .into_iter()
        .chain(side_names())
        .chain(layouts())
    {
        if !words.contains(&word) {
            words.push(word);
        }
    }
    words
}

// ─────────────────────────────── regex shapes ───────────────────────────────

/// A word-bounded alternation of literal idents: `(?<![\w-])(a|b|c)(?![\w-])`.
pub fn word_alt(words: &[&str]) -> String {
    format!("(?<![\\w-])({})(?![\\w-])", words.join("|"))
}

/// An anchored alternation for a tree-sitter `#match?` predicate — the whole
/// node text must be one of the words: `^(a|b|c)$`.
pub fn anchored_alt(words: &[&str]) -> String {
    format!("^({})$", words.join("|"))
}

/// A property decl head, `name:` — but declining a colon glued to a side word,
/// which is a forced endpoint side (`plate:left`), handled by `#side`.
pub fn prop_head(name_alt: &str) -> String {
    format!(
        "{name_alt}\\s*(:)(?!:)(?!({sides})(?![\\w-]))",
        sides = side_names().join("|"),
    )
}

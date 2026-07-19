//! The published, machine-readable contract [Decision 8, ROADMAP 3.8]. Two
//! artifacts, both **generated from the ledger** and nothing else — the same
//! `PROPERTIES` / type / template / default tables the resolver and validator
//! read — so a new property surfaces in the schema the moment it has a row (or
//! the drift test fails). No prose is parsed; no timestamp is written, so a
//! regeneration is byte-identical.
//!
//! - [`schema_json`] — `schema/lini.schema.json`, the full contract.
//! - [`reference_md`] — `schema/reference.md`, the compact human mirror.
//!
//! `cargo xtask gen-schema` writes both; `tests/schema.rs` regenerates them in
//! memory and asserts byte-equality with the committed files.

mod json;

use crate::desugar::types::{TEMPLATES, template_base};
use crate::ledger::defaults::{link_defaults, primitive_bundle, root_defaults, template_bundle};
use crate::ledger::examples::EXAMPLES;
use crate::ledger::properties::{
    BUILDER_CALLS, DefaultRef, Gate, Inherit, Kind, Owner, PROPERTIES, Property, Shape,
};
use crate::resolve::NodeKind;
use crate::syntax::ast::Decl;
use json::J;

/// The schema-format version — bumped only when the JSON's *shape* changes, not
/// when a property row does (that rides the crate version and the drift check).
const SCHEMA_VERSION: i64 = 1;
const GENERATOR: &str = "cargo xtask gen-schema";

// ─────────────────────────── ledger → tokens ───────────────────────────

fn kind_name(k: &Kind) -> &'static str {
    match k {
        Kind::Number => "number",
        Kind::Ident => "ident",
        Kind::Str => "string",
        Kind::Colour => "colour",
        Kind::Paint => "paint",
        Kind::Marker => "marker",
        Kind::Track => "track",
        Kind::Any => "any",
    }
}

fn shape_name(s: &Shape) -> &'static str {
    match s {
        Shape::One(_) => "one",
        Shape::List(_) => "list",
        Shape::Pen => "pen",
        Shape::Pattern => "pattern",
    }
}

fn shape_kind(s: &Shape) -> Option<&'static str> {
    match s {
        Shape::One(k) | Shape::List(k) => Some(kind_name(k)),
        Shape::Pen | Shape::Pattern => None,
    }
}

fn default_name(d: &DefaultRef) -> &'static str {
    match d {
        DefaultRef::None => "none",
        DefaultRef::Bundles => "bundles",
        DefaultRef::Engine => "engine",
    }
}

fn inherit_name(i: &Inherit) -> &'static str {
    match i {
        Inherit::No => "no",
        Inherit::Text => "text",
        Inherit::ScopeLink => "scope-link",
        Inherit::Engine => "engine",
    }
}

fn gate_name(g: &Gate) -> &'static str {
    match g {
        Gate::Lenient => "lenient",
        Gate::Hard => "hard",
    }
}

/// A `format:`-style row: one resolve channel (scope-link) *and* node owners, so
/// it cascades two ways — engine-read on its chart owners, scope-link on its
/// drawing owners [Decision 4]. Derived from the data, never the comment.
fn is_dual_channel(p: &Property) -> bool {
    matches!(p.inherit, Inherit::ScopeLink) && p.has_node_owner()
}

fn owner_json(o: &Owner) -> J {
    let (kind, name): (&str, Option<&str>) = match o {
        Owner::Universal => ("universal", None),
        Owner::Root => ("root", None),
        Owner::Link => ("link", None),
        Owner::Type(t) => ("type", Some(t)),
        Owner::Layout(l) => ("layout", Some(l)),
        Owner::Role(r) => ("role", Some(r)),
    };
    let mut obj = vec![("kind", J::s(kind))];
    if let Some(name) = name {
        obj.push(("name", J::s(name)));
    }
    J::Obj(obj)
}

/// The value side of every decl in a bundle, keyed by property name, in the
/// bundle's authored order — the concrete default, printed exactly as authored.
fn bundle_json(decls: &[Decl]) -> J {
    J::Obj(
        decls
            .iter()
            .map(|d| {
                let name: &'static str = leak(&d.name);
                (name, J::s(crate::fmt::print_decl_value(d)))
            })
            .collect(),
    )
}

/// Object keys are `&'static str`; bundle decl names are owned. The generator
/// runs once per process (the CLI regen, or one test), so a small controlled
/// leak keeps the JSON builder allocation-simple without unsafe.
fn leak(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
}

/// The built-in template's primitive kind and its base→derived chain (the
/// primitive excluded — it is the kind), walked off the [`TEMPLATES`] table.
fn template_chain(name: &str) -> (NodeKind, Vec<String>) {
    let mut chain = vec![name.to_string()];
    let mut cur = name;
    loop {
        let base = template_base(cur).expect("a built-in template has a base");
        if let Some(kind) = NodeKind::parse(base) {
            chain.reverse();
            return (kind, chain);
        }
        chain.push(base.to_string());
        cur = base;
    }
}

fn example_for(name: &str) -> Option<&'static str> {
    EXAMPLES.iter().find(|(n, _)| *n == name).map(|(_, e)| *e)
}

/// The layout engines and roles named across the owner column, in first-seen
/// order — the only layout/role facts the ledger can state.
fn layouts_and_roles() -> (Vec<&'static str>, Vec<&'static str>) {
    let mut layouts = Vec::new();
    let mut roles = Vec::new();
    for p in PROPERTIES {
        for o in p.owners {
            match o {
                Owner::Layout(l) if !layouts.contains(l) => layouts.push(*l),
                Owner::Role(r) if !roles.contains(r) => roles.push(*r),
                _ => {}
            }
        }
    }
    (layouts, roles)
}

// ─────────────────────────── the JSON schema ───────────────────────────

fn enums_json() -> J {
    let pairs = |items: &[(&'static str, &'static str)]| {
        J::Obj(items.iter().map(|(k, v)| (*k, J::s(*v))).collect())
    };
    J::Obj(vec![
        (
            "kinds",
            pairs(&[
                ("number", "a bare number"),
                ("ident", "a keyword identifier"),
                ("string", "quoted text — a bare word errors"),
                ("colour", "a flat colour or --var, no gradient"),
                ("paint", "colour, none, or a gradient"),
                (
                    "marker",
                    "a marker glyph name (rides Markers, not the attr map)",
                ),
                ("track", "a grid track — auto, a number, or repeat(...)"),
                (
                    "any",
                    "a positional or mixed form the property's own reader validates",
                ),
            ]),
        ),
        (
            "shapes",
            pairs(&[
                (
                    "one",
                    "one comma-group — a scalar or a space-separated tuple",
                ),
                ("list", "a comma-separated list of groups"),
                (
                    "pen",
                    "a draw: pen run — structured calls + :segment points",
                ),
                ("pattern", "one grid(...) / radial(...) replication call"),
            ]),
        ),
        (
            "inherit",
            pairs(&[
                ("no", "does not flow down the tree"),
                ("text", "the resolve text channel — nearest ancestor wins"),
                (
                    "scope-link",
                    "scene config a link takes from its scope; a dual-channel row \
                     (format) is also engine-read on its chart owners",
                ),
                ("engine", "nearest-wins inside a layout engine, not resolve"),
            ]),
        ),
        (
            "gate",
            pairs(&[
                ("lenient", "inert out of scope"),
                ("hard", "errors out of scope"),
            ]),
        ),
        (
            "ownerKinds",
            pairs(&[
                ("universal", "every drawn node, in every layout"),
                ("root", "the root's scene config (the stylesheet block)"),
                ("link", "a link's own property"),
                ("type", "read on this primitive or template"),
                ("layout", "interpreted by this layout engine"),
                ("role", "a layout role that is not one type"),
            ]),
        ),
    ])
}

fn primitives_json() -> J {
    J::Arr(
        NodeKind::ALL
            .iter()
            .map(|k| {
                J::Obj(vec![
                    ("name", J::s(k.as_str())),
                    ("defaults", bundle_json(&primitive_bundle(*k))),
                ])
            })
            .collect(),
    )
}

fn templates_json() -> J {
    J::Arr(
        TEMPLATES
            .iter()
            .map(|(name, base)| {
                let (kind, chain) = template_chain(name);
                J::Obj(vec![
                    ("name", J::s(*name)),
                    ("primitive", J::s(kind.as_str())),
                    ("base", J::s(*base)),
                    ("chain", J::Arr(chain.into_iter().map(J::Str).collect())),
                    ("defaults", bundle_json(&template_bundle(name))),
                ])
            })
            .collect(),
    )
}

fn property_json(p: &Property) -> J {
    let mut obj = vec![
        ("name", J::s(p.name)),
        ("owners", J::Arr(p.owners.iter().map(owner_json).collect())),
        ("shape", J::s(shape_name(&p.shape))),
    ];
    if let Some(kind) = shape_kind(&p.shape) {
        obj.push(("kind", J::s(kind)));
    }
    obj.push(("default", J::s(default_name(&p.default))));
    obj.push(("inherit", J::s(inherit_name(&p.inherit))));
    obj.push(("gate", J::s(gate_name(&p.gate))));
    obj.push(("text", J::Bool(p.text)));
    obj.push(("baked", J::Bool(p.baked)));
    obj.push(("deferred", J::Bool(p.deferred)));
    obj.push(("dualChannel", J::Bool(is_dual_channel(p))));
    if let Some(example) = example_for(p.name) {
        obj.push(("example", J::s(example)));
    }
    J::Obj(obj)
}

fn schema_tree() -> J {
    let (layouts, roles) = layouts_and_roles();
    J::Obj(vec![
        ("schemaVersion", J::Int(SCHEMA_VERSION)),
        ("crate", J::s("lini")),
        ("crateVersion", J::s(env!("CARGO_PKG_VERSION"))),
        ("generator", J::s(GENERATOR)),
        ("enums", enums_json()),
        (
            "layouts",
            J::Arr(layouts.iter().map(|l| J::s(*l)).collect()),
        ),
        ("roles", J::Arr(roles.iter().map(|r| J::s(*r)).collect())),
        (
            "builderCalls",
            J::Arr(BUILDER_CALLS.iter().map(|c| J::s(*c)).collect()),
        ),
        ("primitives", primitives_json()),
        ("templates", templates_json()),
        ("sceneDefaults", bundle_json(&root_defaults())),
        ("linkDefaults", bundle_json(&link_defaults())),
        (
            "properties",
            J::Arr(PROPERTIES.iter().map(property_json).collect()),
        ),
    ])
}

/// The full machine-readable schema as pretty JSON with a trailing newline.
pub fn schema_json() -> String {
    json::to_string(&schema_tree())
}

// ────────────────────────── the compact reference ──────────────────────────

/// Escape a value for a Markdown table cell — the only hazard is the `|` in a
/// type's `|name|` bars.
fn md_cell(s: &str) -> String {
    s.replace('|', "\\|")
}

fn owner_short(o: &Owner) -> String {
    match o {
        Owner::Universal => "universal".into(),
        Owner::Root => "root".into(),
        Owner::Link => "link".into(),
        Owner::Type(t) => format!("|{t}|"),
        Owner::Layout(l) => format!("{l} (layout)"),
        Owner::Role(r) => format!("{r} (role)"),
    }
}

fn flags_short(p: &Property) -> String {
    let mut flags = Vec::new();
    if p.text {
        flags.push("text");
    }
    if p.baked {
        flags.push("baked");
    }
    if matches!(p.gate, Gate::Hard) {
        flags.push("hard-gate");
    }
    if p.deferred {
        flags.push("deferred");
    }
    if is_dual_channel(p) {
        flags.push("dual-channel");
    }
    if flags.is_empty() {
        "—".into()
    } else {
        flags.join(" ")
    }
}

fn shape_short(s: &Shape) -> String {
    match shape_kind(s) {
        Some(kind) => format!("{}:{kind}", shape_name(s)),
        None => shape_name(s).into(),
    }
}

fn inherit_short(i: &Inherit) -> &'static str {
    match i {
        Inherit::No => "—",
        other => inherit_name(other),
    }
}

/// The compact human reference — the ledger's truth as dense Markdown.
pub fn reference_md() -> String {
    let mut out = String::new();
    out.push_str("# Lini property reference\n\n");
    out.push_str(&format!(
        "Generated from the property ledger by `{GENERATOR}` — **do not edit**. \
         Schema v{SCHEMA_VERSION}, lini {}. The machine-readable form, with one \
         compiled example per property, is `lini.schema.json`.\n\n",
        env!("CARGO_PKG_VERSION"),
    ));

    // Primitives.
    out.push_str("## Primitives\n\n");
    let prims: Vec<String> = NodeKind::ALL
        .iter()
        .map(|k| format!("`{}`", k.as_str()))
        .collect();
    out.push_str(&prims.join(" "));
    out.push_str("\n\n");

    // Templates.
    out.push_str("## Templates\n\n");
    out.push_str("| template | primitive | chain |\n|---|---|---|\n");
    for (name, _) in TEMPLATES {
        let (kind, chain) = template_chain(name);
        out.push_str(&format!(
            "| `{name}` | `{}` | {} |\n",
            kind.as_str(),
            chain
                .iter()
                .map(|c| format!("`{c}`"))
                .collect::<Vec<_>>()
                .join(" → "),
        ));
    }
    out.push('\n');

    // Roles & layouts.
    let (layouts, roles) = layouts_and_roles();
    out.push_str("## Layout engines\n\n");
    out.push_str(
        &layouts
            .iter()
            .map(|l| format!("`{l}`"))
            .collect::<Vec<_>>()
            .join(" "),
    );
    out.push_str("\n\n## Roles\n\n");
    out.push_str(
        &roles
            .iter()
            .map(|r| format!("`{r}`"))
            .collect::<Vec<_>>()
            .join(" "),
    );
    out.push_str("\n\n");

    // Value builders.
    out.push_str("## Value builders\n\n");
    out.push_str(
        &BUILDER_CALLS
            .iter()
            .map(|c| format!("`{c}`"))
            .collect::<Vec<_>>()
            .join(" "),
    );
    out.push_str("\n\n");

    // Properties.
    out.push_str("## Properties\n\n");
    out.push_str(
        "Shape is `form:kind` (see `enums` in the JSON). Flags: `text` valid on a \
         bare text leaf · `baked` compiled into positions, never live CSS · \
         `hard-gate` errors out of scope · `deferred` reader partly built · \
         `dual-channel` cascades two ways (`format`).\n\n",
    );
    out.push_str("| property | owners | shape | default | inherit | flags |\n");
    out.push_str("|---|---|---|---|---|---|\n");
    for p in PROPERTIES {
        let owners = p
            .owners
            .iter()
            .map(|o| md_cell(&owner_short(o)))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "| `{}` | {} | `{}` | `{}` | {} | {} |\n",
            p.name,
            owners,
            shape_short(&p.shape),
            default_name(&p.default),
            inherit_short(&p.inherit),
            md_cell(&flags_short(p)),
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every property row carries exactly one example, and no example names a
    /// property that no longer exists — the schema's "one example each" cannot
    /// silently gain a hole or an orphan (the compile test lives beside it in
    /// `tests/schema.rs`).
    #[test]
    fn examples_cover_every_property_and_nothing_else() {
        for p in PROPERTIES {
            assert!(
                example_for(p.name).is_some(),
                "property '{}' has no schema example",
                p.name
            );
        }
        for (name, _) in EXAMPLES {
            assert!(
                crate::ledger::properties::get(name).is_some(),
                "example names unknown property '{name}'"
            );
        }
        assert_eq!(EXAMPLES.len(), PROPERTIES.len(), "one example per property");
    }

    /// `format` is the dual-channel row (scope-link + node owners); the pure
    /// scene-config scope-link rows are not [Decision 4].
    #[test]
    fn only_format_is_dual_channel() {
        let dual: Vec<&str> = PROPERTIES
            .iter()
            .filter(|p| is_dual_channel(p))
            .map(|p| p.name)
            .collect();
        assert_eq!(dual, ["format"]);
    }
}

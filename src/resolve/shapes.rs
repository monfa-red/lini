use super::ir::{ResolvedAttr, ResolvedValue, ShapeKind, VarTable};
use super::styles::StyleTable;
use crate::ast::{BodyItem, ShapeDef, TypeDefaults};
use crate::error::Error;
use crate::span::Span;
use std::collections::HashMap;

const MAX_INHERITANCE_DEPTH: usize = 16;

/// Built-in templates per SPEC section 9. Each maps to a base primitive.
pub(super) const TEMPLATES: &[(&str, &str)] = &[
    ("group", "rect"),
    ("badge", "rect"),
    ("note", "rect"),
    ("row", "rect"),
    ("col", "rect"),
    ("table", "group"),
    ("cell", "rect"),
];

pub fn is_template(name: &str) -> bool {
    TEMPLATES.iter().any(|(n, _)| *n == name)
}

/// Result of resolving a `|type|` reference. `kind` is the underlying primitive;
/// `attrs` and `body_items` accumulate from the inheritance chain (built-in →
/// defs-block type-defaults → shape definitions, walked top-down).
#[derive(Clone)]
pub struct ResolvedShape {
    pub kind: ShapeKind,
    pub attrs: Vec<ResolvedAttr>,
    pub body_items: Vec<BodyItem>,
    /// Names walked, from the inst's declared type back toward the primitive.
    /// Excludes the primitive itself (which is in `kind`).
    pub type_chain: Vec<String>,
}

pub struct ShapesTable {
    user: HashMap<String, ResolvedShapeDef>,
    /// `|name|` defaults from the defs block, keyed by type name. Applied
    /// during inheritance walking as the lowest-specificity attr layer for
    /// each type in the chain.
    type_defaults: HashMap<String, Vec<ResolvedAttr>>,
}

struct ResolvedShapeDef {
    base: String,
    attrs: Vec<ResolvedAttr>,
    body_items: Vec<BodyItem>,
    span: Span,
}

impl ShapesTable {
    pub fn build(
        defs: &[&ShapeDef],
        type_defaults: &[&TypeDefaults],
        styles: &StyleTable,
        vars: &VarTable,
    ) -> Result<Self, Error> {
        let user = build_user_shapes(defs, styles, vars)?;
        let type_defaults = build_type_defaults(type_defaults, &user, styles, vars)?;
        let table = Self {
            user,
            type_defaults,
        };

        // Validate every user shape's inheritance chain walks cleanly.
        for def in defs {
            let mut visiting: Vec<String> = Vec::new();
            table.walk_chain(&def.name, def.span, &mut visiting, 0)?;
        }
        Ok(table)
    }

    /// Resolve a `|type|` reference into a primitive kind and inherited attrs.
    pub fn resolve(&self, name: &str, use_span: Span) -> Result<ResolvedShape, Error> {
        let mut visiting: Vec<String> = Vec::new();
        self.walk_chain(name, use_span, &mut visiting, 0)
    }

    /// Walk the inheritance chain from `name` down to its primitive base,
    /// layering in attrs and body items at each level. The order is:
    ///
    ///   primitive < defs-block-type-defaults < template attrs < user-shape attrs
    ///
    /// so per [SPEC section 13] the most-specific type's own attrs win against the
    /// less-specific layers below it.
    fn walk_chain(
        &self,
        name: &str,
        use_span: Span,
        visiting: &mut Vec<String>,
        depth: usize,
    ) -> Result<ResolvedShape, Error> {
        if depth > MAX_INHERITANCE_DEPTH {
            return Err(Error::at(
                use_span,
                format!("'{}' exceeds max inheritance depth (16)", name),
            ));
        }
        if visiting.iter().any(|n| n == name) {
            let chain = format!("{} -> {}", visiting.join(" -> "), name);
            return Err(Error::at(use_span, format!("cycle in '{}'", chain)));
        }

        // Primitive — leaf. Only defs-block type-defaults attach here; the
        // primitive itself has no built-in attrs (those are constants in the
        // layout/render layers).
        if let Some(kind) = ShapeKind::parse(name) {
            return Ok(ResolvedShape {
                kind,
                attrs: self.defaults_for(name),
                body_items: Vec::new(),
                type_chain: Vec::new(),
            });
        }

        // Template — built-in. Walk its base, layer the template's own
        // built-in attrs, then defs-block type-defaults.
        if let Some((_, base)) = TEMPLATES.iter().find(|(n, _)| *n == name) {
            visiting.push(name.to_string());
            let mut resolved = self.walk_chain(base, use_span, visiting, depth + 1)?;
            visiting.pop();
            resolved.attrs.extend(template_attrs(name));
            resolved.attrs.extend(self.defaults_for(name));
            resolved.type_chain.insert(0, name.to_string());
            return Ok(resolved);
        }

        // User shape — walk base, then layer defs-block type-defaults, then
        // the shape's own definition attrs.
        let def = self
            .user
            .get(name)
            .ok_or_else(|| Error::at(use_span, format!("unknown type '|{}|'", name)))?;

        visiting.push(name.to_string());
        let base = self.walk_chain(&def.base, def.span, visiting, depth + 1)?;
        visiting.pop();

        let mut attrs = base.attrs;
        attrs.extend(self.defaults_for(name));
        attrs.extend(def.attrs.iter().cloned());

        let mut body_items = base.body_items;
        body_items.extend(def.body_items.iter().cloned());

        let mut type_chain = base.type_chain;
        type_chain.insert(0, name.to_string());

        Ok(ResolvedShape {
            kind: base.kind,
            attrs,
            body_items,
            type_chain,
        })
    }

    fn defaults_for(&self, name: &str) -> Vec<ResolvedAttr> {
        self.type_defaults.get(name).cloned().unwrap_or_default()
    }

    /// `|name|` type-defaults attrs, for the output stylesheet's shape rules.
    pub fn type_default_attrs(&self, name: &str) -> Option<&[ResolvedAttr]> {
        self.type_defaults.get(name).map(Vec::as_slice)
    }

    /// A user shape def's own attrs (chain excluded), for its stylesheet rule.
    pub fn own_attrs(&self, name: &str) -> Option<&[ResolvedAttr]> {
        self.user.get(name).map(|d| d.attrs.as_slice())
    }
}

/// Built-in template attrs — the lowest-specificity layer of a template, so
/// defs-block defaults and instance attrs override freely (SPEC §9). Visual
/// values stay live `--lini-*` references so a host page can re-theme them;
/// `row`/`col` carry no paint at all — they only lay out. `table`/`cell` are
/// handled by the grid-rule machinery (see the renderer), not here.
pub(super) fn template_attrs(name: &str) -> Vec<ResolvedAttr> {
    let live = |var: &str| ResolvedValue::LiveVar {
        name: var.into(),
        raw: false,
        baked: None,
    };
    let attr = |name: &str, value: ResolvedValue| ResolvedAttr {
        name: name.into(),
        value,
        span: Span::empty(),
    };
    let num = ResolvedValue::Number;
    let ident = |s: &str| ResolvedValue::Ident(s.into());
    let pair = |a: f64, b: f64| ResolvedValue::Tuple(vec![num(a), num(b)]);

    match name {
        // Frame + label: a quiet container — themable grey stroke, faint wash,
        // rounded corners, padding that keeps children clear of the border.
        "group" => vec![
            attr("stroke", live("group-stroke")),
            attr("fill", live("group-fill")),
            attr("radius", num(6.0)),
            attr("padding", num(10.0)),
        ],
        // Corner pill: small on-accent text on an accent capsule, centred on
        // the host's top-right corner (`place:on` — an overlay that straddles
        // the corner and grows nothing). `color`/`text-size` cascade to the
        // label.
        "badge" => vec![
            attr("side", ident("top")),
            attr("align", ident("end")),
            attr("place", ident("on")),
            attr("radius", num(999.0)),
            attr("padding", pair(2.0, 8.0)),
            attr("shadow", num(2.0)),
            attr("fill", live("accent")),
            attr("color", live("on-accent")),
            attr("text-size", num(11.0)),
            attr("z", num(10.0)),
        ],
        // Sticky note: pale fill, soft shadow, no border.
        "note" => vec![
            attr("radius", num(2.0)),
            attr("padding", num(12.0)),
            attr("shadow", num(2.0)),
            attr("stroke", ident("none")),
            attr("fill", live("note-bg")),
        ],
        // Frameless layout wrappers — invisible, zero padding, direction baked
        // in so `|row| { … }` / `|col| { … }` need no `layout:`.
        "row" => vec![
            attr("layout", ident("row")),
            attr("fill", ident("none")),
            attr("stroke", ident("none")),
            attr("padding", num(0.0)),
        ],
        "col" => vec![
            attr("layout", ident("column")),
            attr("fill", ident("none")),
            attr("stroke", ident("none")),
            attr("padding", num(0.0)),
        ],
        // The table draws its own grid lines (frame + separators) in
        // `--lini-stroke`; no wash, no padding, cells abut at gap:0. The
        // rule geometry is built in layout and rendered by the table itself.
        "table" => vec![
            attr("gap", num(0.0)),
            attr("padding", num(0.0)),
            attr("radius", num(0.0)),
            attr("fill", ident("none")),
            attr("stroke", live("stroke")),
        ],
        // A cell is a borderless padded slot — the table owns every line, so
        // a cell never strokes its own edge (that is what used to double).
        "cell" => vec![
            attr("stroke", ident("none")),
            attr("fill", ident("none")),
            attr("padding", num(8.0)),
        ],
        _ => Vec::new(),
    }
}

// ───────────────────────── Build helpers ─────────────────────────

fn build_user_shapes(
    defs: &[&ShapeDef],
    styles: &StyleTable,
    vars: &VarTable,
) -> Result<HashMap<String, ResolvedShapeDef>, Error> {
    let mut user: HashMap<String, ResolvedShapeDef> = HashMap::new();
    for def in defs {
        if super::is_reserved(&def.name) {
            return Err(super::reserved_error(def.span, &def.name));
        }
        if ShapeKind::parse(&def.name).is_some() {
            return Err(Error::at(
                def.span,
                format!("'{}' shadows a built-in primitive", def.name),
            ));
        }
        if is_template(&def.name) {
            return Err(Error::at(
                def.span,
                format!("'{}' shadows a built-in template", def.name),
            ));
        }
        if user.contains_key(&def.name) {
            return Err(Error::at(
                def.span,
                format!("duplicate shape '{}'", def.name),
            ));
        }

        let attrs = super::resolve_attrs(&def.items, styles, vars)?;
        let body_items = def.body.clone().unwrap_or_default();

        user.insert(
            def.name.clone(),
            ResolvedShapeDef {
                base: def.base.name.clone(),
                attrs,
                body_items,
                span: def.span,
            },
        );
    }
    Ok(user)
}

/// Resolve and validate the `|name|` type-defaults entries from the defs
/// block. Each entry must reference a known type — primitive, template, or
/// user-defined shape. Duplicates are rejected.
fn build_type_defaults(
    entries: &[&TypeDefaults],
    user: &HashMap<String, ResolvedShapeDef>,
    styles: &StyleTable,
    vars: &VarTable,
) -> Result<HashMap<String, Vec<ResolvedAttr>>, Error> {
    let mut out: HashMap<String, Vec<ResolvedAttr>> = HashMap::new();
    for entry in entries {
        if !is_known_type(&entry.name, user) {
            return Err(Error::at(
                entry.span,
                format!(
                    "unknown type '|{}|' in defs (no such primitive, template, or shape)",
                    entry.name
                ),
            ));
        }
        if out.contains_key(&entry.name) {
            return Err(Error::at(
                entry.span,
                format!("duplicate type-defaults entry '|{}|'", entry.name),
            ));
        }
        let attrs = super::resolve_attrs(&entry.items, styles, vars)?;
        out.insert(entry.name.clone(), attrs);
    }
    Ok(out)
}

fn is_known_type(name: &str, user: &HashMap<String, ResolvedShapeDef>) -> bool {
    ShapeKind::parse(name).is_some() || is_template(name) || user.contains_key(name)
}

//! Type resolution: turn a `|type|` reference into its primitive kind, its type
//! chain, and the layered **type-cascade** defaults (SPEC §8, §12.1).
//!
//! A type is a primitive (`box`), a built-in template (`group`, `table`, …), or
//! a user `name::base` define. Resolving walks base→derived, and at each level
//! layers, least-specific first:
//!
//! 1. the level's intrinsic defaults — a template's built-in bundle, or a
//!    define's own `{ … }` declarations,
//! 2. that type's **element rule** (`box { … }`) from the stylesheet.
//!
//! So a derived type overrides what it builds on, and an explicit `treat { … }`
//! rule overrides the `treat::box { … }` define defaults. Cycles and chains
//! deeper than 16 are errors; a define may not shadow a built-in type.

use super::cascade::Stylesheet;
use super::ir::{ResolvedValue, ShapeKind, VarTable};
use super::value::resolve_groups;
use crate::error::Error;
use crate::span::Span;
use crate::syntax::ast::{Child, Define, Wire};
use std::collections::HashMap;

const MAX_INHERITANCE_DEPTH: usize = 16;

/// Built-in templates and their base type (SPEC §8). Each is a bundle over a
/// primitive (or, for `table`, over `group`).
pub(super) const TEMPLATES: &[(&str, &str)] = &[
    ("plain", "box"),
    ("rect", "box"),
    ("group", "box"),
    ("caption", "plain"),
    ("footer", "caption"),
    ("badge", "box"),
    ("note", "box"),
    ("row", "plain"),
    ("column", "plain"),
    ("table", "group"),
];

pub fn is_template(name: &str) -> bool {
    TEMPLATES.iter().any(|(n, _)| *n == name)
}

/// The base type a built-in template builds on (`table` → `group`, `group` →
/// `box`, …); `None` for a primitive or a non-template.
pub(crate) fn template_base(name: &str) -> Option<&'static str> {
    TEMPLATES.iter().find(|(n, _)| *n == name).map(|(_, b)| *b)
}

/// A user define, resolved: its base, its own default declarations, and its
/// intrinsic body (child nodes + internal wires, materialised per instance).
struct DefineEntry {
    base: String,
    decls: Vec<(String, ResolvedValue)>,
    body_children: Vec<Child>,
    body_wires: Vec<Wire>,
    span: Span,
}

/// The type table: user defines plus borrowed access to the stylesheet (for
/// element rules) and the variable table (for resolving define declarations).
pub struct Types<'a> {
    user: HashMap<String, DefineEntry>,
    sheet: &'a Stylesheet,
}

/// A resolved type: its primitive kind, the type names walked (declared →
/// derived, primitive excluded), the layered type-cascade defaults, and the
/// intrinsic body to materialise per instance.
#[derive(Debug)]
pub struct ResolvedType {
    pub kind: ShapeKind,
    pub type_chain: Vec<String>,
    pub defaults: Vec<(String, ResolvedValue)>,
    pub body_children: Vec<Child>,
    pub body_wires: Vec<Wire>,
}

impl<'a> Types<'a> {
    pub fn build(
        defines: &[&Define],
        sheet: &'a Stylesheet,
        vars: &VarTable,
    ) -> Result<Self, Error> {
        let mut user = HashMap::new();
        for d in defines {
            if is_builtin_type(&d.name) {
                return Err(Error::at(
                    d.span,
                    format!("'{}' shadows a built-in type", d.name),
                ));
            }
            if user.contains_key(&d.name) {
                return Err(Error::at(d.span, format!("duplicate type '{}'", d.name)));
            }
            let mut decls = Vec::with_capacity(d.body.decls.len());
            for decl in &d.body.decls {
                decls.push((
                    decl.name.clone(),
                    resolve_groups(&decl.groups, decl.span, vars)?,
                ));
            }
            user.insert(
                d.name.clone(),
                DefineEntry {
                    base: d.base.clone(),
                    decls,
                    body_children: d.body.children.clone(),
                    body_wires: d.body.wires.clone(),
                    span: d.span,
                },
            );
        }
        let types = Self { user, sheet };
        // Validate every define's chain up front, so a cycle or over-deep
        // inheritance is reported even for a type no instance uses.
        for d in defines {
            types.walk(&d.name, d.span, &mut Vec::new(), 0)?;
        }
        Ok(types)
    }

    pub fn is_known(&self, name: &str) -> bool {
        ShapeKind::parse(name).is_some() || is_template(name) || self.user.contains_key(name)
    }

    pub fn resolve(&self, name: &str, span: Span) -> Result<ResolvedType, Error> {
        self.walk(name, span, &mut Vec::new(), 0)
    }

    /// Walk a type to its primitive base, layering defaults base→derived.
    /// `visiting` carries the chain for cycle detection; `depth` bounds it.
    fn walk(
        &self,
        name: &str,
        span: Span,
        visiting: &mut Vec<String>,
        depth: usize,
    ) -> Result<ResolvedType, Error> {
        if depth > MAX_INHERITANCE_DEPTH {
            return Err(Error::at(
                span,
                format!("'{}' exceeds max inheritance depth (16)", name),
            ));
        }
        if visiting.iter().any(|n| n == name) {
            return Err(Error::at(
                span,
                format!("cycle in '{} -> {}'", visiting.join(" -> "), name),
            ));
        }

        if let Some(kind) = ShapeKind::parse(name) {
            return Ok(ResolvedType {
                kind,
                type_chain: Vec::new(),
                defaults: self.sheet.element_decls(name),
                body_children: Vec::new(),
                body_wires: Vec::new(),
            });
        }

        if let Some((_, base)) = TEMPLATES.iter().find(|(n, _)| *n == name) {
            visiting.push(name.to_string());
            let mut r = self.walk(base, span, visiting, depth + 1)?;
            visiting.pop();
            r.defaults.extend(template_attrs(name));
            r.defaults.extend(self.sheet.element_decls(name));
            r.type_chain.insert(0, name.to_string());
            return Ok(r);
        }

        if let Some(entry) = self.user.get(name) {
            visiting.push(name.to_string());
            let mut r = self.walk(&entry.base, entry.span, visiting, depth + 1)?;
            visiting.pop();
            r.defaults.extend(entry.decls.iter().cloned());
            r.defaults.extend(self.sheet.element_decls(name));
            r.type_chain.insert(0, name.to_string());
            r.body_children.extend(entry.body_children.iter().cloned());
            r.body_wires.extend(entry.body_wires.iter().cloned());
            return Ok(r);
        }

        Err(Error::at(span, format!("unknown type '{}'", name)))
    }
}

/// A define may not take the name of a primitive, a template, the `wire` rule
/// target, a side, or a reserved-for-future name (`rect`, `circle`) — SPEC §15, §18.
fn is_builtin_type(name: &str) -> bool {
    ShapeKind::parse(name).is_some()
        || is_template(name)
        || matches!(
            name,
            "wire" | "circle" | "top" | "bottom" | "left" | "right"
        )
}

/// A template's built-in attribute bundle (SPEC §8) — the lowest layer of the
/// type cascade. Visual colours stay live `--lini-*` references so a host page
/// can re-theme them.
pub(super) fn template_attrs(name: &str) -> Vec<(String, ResolvedValue)> {
    let live = |var: &str| ResolvedValue::LiveVar {
        name: var.into(),
        raw: false,
        baked: None,
    };
    let num = ResolvedValue::Number;
    let ident = |s: &str| ResolvedValue::Ident(s.into());
    let pair = |a: f64, b: f64| ResolvedValue::Tuple(vec![num(a), num(b)]);
    let attr = |n: &str, v: ResolvedValue| (n.to_string(), v);

    match name {
        "plain" => vec![
            attr("stroke", ident("none")),
            attr("fill", ident("none")),
            attr("padding", num(0.0)),
        ],
        "rect" => vec![attr("radius", num(0.0))],
        "group" => vec![
            attr("stroke", live("group-stroke")),
            attr("stroke-style", ident("dashed")),
            attr("stroke-width", num(1.0)),
            attr("fill", live("group-fill")),
            attr("radius", num(6.0)),
        ],
        // A title pinned just above the group's top-left corner: `pin: top left`
        // sits it flush in the corner, `translate: 0 -16` lifts it out over the
        // border (≈ 1.35 × the 12px caption font; mirrors `caption-font-size`).
        "caption" => vec![
            attr(
                "pin",
                ResolvedValue::Tuple(vec![ident("top"), ident("left")]),
            ),
            attr("translate", pair(0.0, -16.0)),
            attr("color", live("caption-color")),
            attr("font-size", num(12.0)),
            attr("font-weight", live("caption-font-weight")),
        ],
        // A footer is a `caption` flipped to the bottom: same look, opposite
        // anchor and lift.
        "footer" => vec![
            attr(
                "pin",
                ResolvedValue::Tuple(vec![ident("bottom"), ident("left")]),
            ),
            attr("translate", pair(0.0, 16.0)),
        ],
        "badge" => vec![
            attr(
                "pin",
                ResolvedValue::Tuple(vec![ident("top"), ident("right")]),
            ),
            attr("radius", num(999.0)),
            attr("padding", pair(2.0, 8.0)),
            attr("shadow", num(2.0)),
            attr("fill", live("accent")),
            attr("color", live("on-accent")),
            attr("font-size", num(11.0)),
        ],
        "note" => vec![
            attr("radius", num(2.0)),
            attr("shadow", num(2.0)),
            attr("stroke", ident("none")),
            attr("fill", live("note-bg")),
        ],
        // `row` / `column` inherit the frameless paint + zero padding from
        // `plain`; they add only their flow direction.
        "row" => vec![attr("layout", ident("row"))],
        "column" => vec![attr("layout", ident("column"))],
        "table" => vec![
            attr("layout", ident("grid")),
            attr("divider", ident("all")),
            attr("gap", num(0.0)),
            attr("padding", pair(4.0, 8.0)),
            attr("fill", ident("none")),
            attr("stroke", live("stroke")),
            // Solid ruling, not the dashed frame `group` brings.
            attr("stroke-style", ident("solid")),
        ],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::ast::{Rule, StyleItem};

    /// Build a `Types` from `src` and resolve `name`, threading the parsed
    /// file, stylesheet, and vars as locals so the borrows live long enough.
    fn resolve_result(src: &str, name: &str) -> Result<ResolvedType, Error> {
        let toks = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(&toks).expect("parse");
        let vars = VarTable::new();
        let rules: Vec<&Rule> = file
            .stylesheet
            .iter()
            .filter_map(|it| match it {
                StyleItem::Rule(r) => Some(r),
                _ => None,
            })
            .collect();
        let sheet = Stylesheet::build(&rules, &vars).expect("sheet");
        let defines: Vec<&Define> = file
            .stylesheet
            .iter()
            .filter_map(|it| match it {
                StyleItem::Define(d) => Some(d),
                _ => None,
            })
            .collect();
        let types = Types::build(&defines, &sheet, &vars)?;
        types.resolve(name, Span::empty())
    }

    fn resolve_ok(src: &str, name: &str) -> ResolvedType {
        resolve_result(src, name).expect("resolve")
    }

    fn build_err(src: &str) -> String {
        match resolve_result(src, "box") {
            Err(e) => e.message,
            Ok(_) => panic!("expected an error building {src:?}"),
        }
    }

    fn has_number(t: &ResolvedType, name: &str, want: f64) -> bool {
        t.defaults
            .iter()
            .any(|(n, v)| n == name && matches!(v, ResolvedValue::Number(x) if *x == want))
    }

    #[test]
    fn primitive_resolves_to_its_kind_with_no_chain() {
        let t = resolve_ok("", "box");
        assert_eq!(t.kind, ShapeKind::Box);
        assert!(t.type_chain.is_empty());
    }

    #[test]
    fn template_resolves_to_its_base_primitive_with_chain_and_bundle() {
        let t = resolve_ok("", "group");
        assert_eq!(t.kind, ShapeKind::Box);
        assert_eq!(t.type_chain, vec!["group"]);
        assert!(has_number(&t, "radius", 6.0)); // group ships radius:6
    }

    #[test]
    fn table_chain_is_table_then_group_over_rect() {
        let t = resolve_ok("", "table");
        assert_eq!(t.kind, ShapeKind::Box);
        assert_eq!(t.type_chain, vec!["table", "group"]);
    }

    #[test]
    fn caption_resolves_over_plain() {
        let t = resolve_ok("", "caption");
        assert_eq!(t.kind, ShapeKind::Box);
        assert_eq!(t.type_chain, vec!["caption", "plain"]);
    }

    #[test]
    fn user_define_resolves_to_base_and_carries_its_decls() {
        let t = resolve_ok("treat::box { radius: 5; }\n", "treat");
        assert_eq!(t.kind, ShapeKind::Box);
        assert_eq!(t.type_chain, vec!["treat"]);
        assert!(has_number(&t, "radius", 5.0));
    }

    #[test]
    fn element_rule_layers_into_the_type_cascade() {
        let t = resolve_ok("box { radius: 4; }\n", "box");
        assert!(has_number(&t, "radius", 4.0));
    }

    #[test]
    fn derived_define_overrides_base_by_order() {
        // panel sets radius:10; group's bundle has radius:6 — panel is later in
        // the chain, so its value is last and wins the fold.
        let t = resolve_ok("panel::group { radius: 10; }\n", "panel");
        assert_eq!(t.type_chain, vec!["panel", "group"]);
        let last_radius = t
            .defaults
            .iter()
            .rev()
            .find(|(n, _)| n == "radius")
            .unwrap();
        assert!(matches!(last_radius.1, ResolvedValue::Number(x) if x == 10.0));
    }

    #[test]
    fn unknown_type_errors() {
        let e = resolve_result("", "ghost").unwrap_err();
        assert!(
            e.message.contains("unknown type 'ghost'"),
            "got: {}",
            e.message
        );
    }

    #[test]
    fn define_shadowing_a_builtin_errors() {
        assert!(build_err("rect::oval { }\n").contains("shadows a built-in"));
    }

    #[test]
    fn inheritance_cycle_errors() {
        assert!(build_err("a::b { }\nb::a { }\n").contains("cycle"));
    }

    #[test]
    fn inheritance_depth_over_16_errors() {
        let mut src = String::from("t0::box { }\n");
        for i in 1..=17 {
            src.push_str(&format!("t{}::t{} {{ }}\n", i, i - 1));
        }
        assert!(build_err(&src).contains("max inheritance depth"));
    }
}

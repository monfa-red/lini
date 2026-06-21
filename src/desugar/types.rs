//! Template table + define/template chain resolution at the AST level. Returns
//! base→derived name chains (primitive excluded — it is the `kind`); desugar turns
//! each chain name into a `.lini-<name>` class. Cycles, depth > 16, and shadowing a
//! built-in are errors.

use crate::error::Error;
use crate::resolve::ShapeKind;
use crate::span::Span;
use crate::syntax::ast::{Define, File, StyleItem};
use std::collections::HashMap;

const MAX_INHERITANCE_DEPTH: usize = 16;

/// Built-in templates and their base type (SPEC §8). Each is a bundle over a
/// primitive (or, for `table`, over `group`).
pub const TEMPLATES: &[(&str, &str)] = &[
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

/// The base a built-in template builds on; `None` for a primitive or non-template.
pub fn template_base(name: &str) -> Option<&'static str> {
    TEMPLATES.iter().find(|(n, _)| *n == name).map(|(_, b)| *b)
}

/// A define may not take the name of a primitive, a template, the `wire` rule
/// target, or a structural SVG class (SPEC §18) — once the `shape` infix is gone,
/// a `|node::box|` define's `.lini-node` would collide with the universal marker.
fn is_builtin_type(name: &str) -> bool {
    ShapeKind::parse(name).is_some()
        || is_template(name)
        || matches!(
            name,
            "wire" | "node" | "text" | "marker" | "canvas" | "scene" | "cut"
        )
}

/// A resolved type: its primitive kind and the template/define names walked
/// base→derived (the primitive is excluded — it is `kind`).
pub struct TypeInfo {
    pub kind: ShapeKind,
    pub chain: Vec<String>,
}

/// The type table: user defines (name → base), validated for cycles, depth, and
/// shadowing on construction.
pub struct Types {
    defines: HashMap<String, String>,
}

impl Types {
    pub fn build(file: &File) -> Result<Self, Error> {
        let mut defines = HashMap::new();
        for d in file.stylesheet.iter().filter_map(as_define) {
            if is_builtin_type(&d.name) {
                return Err(Error::at(
                    d.span,
                    format!("'{}' shadows a built-in type", d.name),
                ));
            }
            if defines.insert(d.name.clone(), d.base.clone()).is_some() {
                return Err(Error::at(d.span, format!("duplicate type '{}'", d.name)));
            }
        }
        let types = Self { defines };
        // Validate every define's chain up front, so a cycle or over-deep
        // inheritance is reported even for a type no instance uses.
        for d in file.stylesheet.iter().filter_map(as_define) {
            types.walk(&d.name, d.span, &mut Vec::new(), 0)?;
        }
        Ok(types)
    }

    pub fn is_known(&self, name: &str) -> bool {
        ShapeKind::parse(name).is_some() || is_template(name) || self.defines.contains_key(name)
    }

    pub fn resolve(&self, name: &str, span: Span) -> Result<TypeInfo, Error> {
        self.walk(name, span, &mut Vec::new(), 0)
    }

    /// Walk a type to its primitive base, accumulating the chain base→derived.
    /// `visiting` carries the chain for cycle detection; `depth` bounds it.
    fn walk(
        &self,
        name: &str,
        span: Span,
        visiting: &mut Vec<String>,
        depth: usize,
    ) -> Result<TypeInfo, Error> {
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
            return Ok(TypeInfo {
                kind,
                chain: Vec::new(),
            });
        }
        let base = template_base(name)
            .map(str::to_string)
            .or_else(|| self.defines.get(name).cloned())
            .ok_or_else(|| Error::at(span, format!("unknown type '{}'", name)))?;
        visiting.push(name.to_string());
        let mut info = self.walk(&base, span, visiting, depth + 1)?;
        visiting.pop();
        info.chain.push(name.to_string());
        Ok(info)
    }
}

fn as_define(it: &StyleItem) -> Option<&Define> {
    match it {
        StyleItem::Define(d) => Some(d),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> File {
        crate::syntax::parser::parse(&crate::lexer::lex(src).expect("lex")).expect("parse")
    }
    fn chain(src: &str, name: &str) -> Vec<String> {
        let file = parse(src);
        let t = Types::build(&file).expect("build");
        t.resolve(name, Span::empty()).expect("resolve").chain
    }
    fn build_err(src: &str) -> String {
        let file = parse(src);
        Types::build(&file)
            .err()
            .map(|e| e.message)
            .unwrap_or_default()
    }

    #[test]
    fn primitive_has_empty_chain() {
        assert!(chain("", "box").is_empty());
    }

    #[test]
    fn table_chain_is_group_then_table() {
        assert_eq!(chain("", "table"), vec!["group", "table"]);
    }

    #[test]
    fn user_define_appends_after_its_base_chain() {
        assert_eq!(
            chain("{ |panel::group| { } }\n", "panel"),
            vec!["group", "panel"]
        );
    }

    #[test]
    fn cycle_and_shadow_error() {
        assert!(build_err("{ |a::b| { }\n|b::a| { } }\n").contains("cycle"));
        assert!(build_err("{ |rect::oval| { } }\n").contains("shadows a built-in"));
    }
}

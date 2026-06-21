//! Every built-in default, expressed as parser-shaped [`Decl`]s. This is the one
//! place Lini's look is tuned; desugar lowers these into `.lini-*` class defs, the
//! global block, and the `-> { }` wire defaults. Visual `--lini-*` colours stay
//! live `--var` references (render emits their defaults as `@layer` CSS).

use crate::resolve::ShapeKind;
use crate::span::Span;
use crate::syntax::ast::{Decl, Value};

fn decl(name: &str, values: Vec<Value>) -> Decl {
    Decl {
        name: name.into(),
        groups: vec![values],
        span: Span::empty(),
    }
}
fn n(name: &str, v: f64) -> Decl {
    decl(name, vec![Value::Number(v)])
}
fn id(name: &str, v: &str) -> Decl {
    decl(name, vec![Value::Ident(v.into())])
}
fn var(name: &str, v: &str) -> Decl {
    decl(name, vec![Value::Var(v.into())])
}
fn pair(name: &str, a: f64, b: f64) -> Decl {
    decl(name, vec![Value::Number(a), Value::Number(b)])
}

/// A primitive's complete default set (paint + geometry).
pub fn primitive_bundle(kind: ShapeKind) -> Vec<Decl> {
    use ShapeKind::*;
    // Closed, content-sized shapes share paint + box-model defaults.
    let sized = || {
        vec![
            var("fill", "fill"),
            var("stroke", "stroke"),
            n("stroke-width", 2.0),
            n("padding", 20.0),
            n("gap", 20.0),
        ]
    };
    match kind {
        Box => {
            let mut b = sized();
            b.push(n("radius", 6.0));
            b
        }
        Oval | Hex | Cyl | Diamond | Cloud => sized(),
        Slant => {
            let mut b = sized();
            b.push(n("skew", 15.0));
            b
        }
        // Geometry-sized closed shapes: paint only, no box model.
        Poly | Path => vec![
            var("fill", "fill"),
            var("stroke", "stroke"),
            n("stroke-width", 2.0),
        ],
        Line => vec![
            id("fill", "none"),
            var("stroke", "stroke"),
            n("stroke-width", 2.0),
        ],
        Icon => vec![var("fill", "stroke"), n("width", 24.0), n("height", 24.0)],
        // Text is structural (render's `lini-text` rule); Image requires src/dims.
        Text | Image => Vec::new(),
    }
}

/// A built-in template's delta over its base (SPEC §8). Empty for a non-template.
pub fn template_bundle(name: &str) -> Vec<Decl> {
    match name {
        "plain" => vec![id("stroke", "none"), id("fill", "none"), n("padding", 0.0)],
        "rect" => vec![n("radius", 0.0)],
        "group" => vec![
            var("stroke", "group-stroke"),
            id("stroke-style", "dashed"),
            n("stroke-width", 1.0),
            var("fill", "group-fill"),
            n("radius", 6.0),
        ],
        "caption" => vec![
            decl(
                "pin",
                vec![Value::Ident("top".into()), Value::Ident("left".into())],
            ),
            pair("translate", 0.0, -18.0),
            var("color", "caption-color"),
            n("font-size", 12.0),
            var("font-weight", "caption-font-weight"),
        ],
        "footer" => vec![
            id("pin", "bottom"),
            pair("translate", 0.0, 17.0),
            n("font-size", 11.0),
            var("color", "footer-color"),
        ],
        "badge" => vec![
            decl(
                "pin",
                vec![Value::Ident("top".into()), Value::Ident("right".into())],
            ),
            pair("translate", 6.0, -6.0),
            n("radius", 8.0),
            pair("padding", 2.0, 6.0),
            decl(
                "shadow",
                vec![Value::Number(2.0), Value::Number(3.0), Value::Number(3.0)],
            ),
            id("stroke", "none"),
            var("fill", "accent"),
            var("color", "on-accent"),
            n("font-size", 11.0),
            id("font-weight", "normal"),
        ],
        "note" => vec![
            n("radius", 2.0),
            n("shadow", 2.0),
            id("stroke", "none"),
            var("fill", "note-bg"),
        ],
        "row" => vec![id("layout", "row")],
        "column" => vec![id("layout", "column")],
        "table" => vec![
            id("layout", "grid"),
            id("divider", "all"),
            n("gap", 0.0),
            pair("padding", 4.0, 8.0),
            id("fill", "none"),
            var("stroke", "stroke"),
            id("stroke-style", "solid"),
            n("font-size", 14.0),
            id("font-weight", "normal"),
        ],
        _ => Vec::new(),
    }
}

/// Scene/root config defaults — prepended to the global block (user decls override).
pub fn root_defaults() -> Vec<Decl> {
    vec![
        id("layout", "column"),
        n("padding", 20.0),
        n("gap", 20.0),
        n("font-size", 15.0),
    ]
}

/// Wire defaults — prepended to the `-> { }` rule (user decls override).
pub fn wire_defaults() -> Vec<Decl> {
    vec![
        n("stroke-width", 2.0),
        n("clearance", 16.0),
        n("font-size", 11.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::ShapeKind;
    use crate::syntax::ast::Value;

    fn has(decls: &[Decl], name: &str) -> bool {
        decls.iter().any(|d| d.name == name)
    }
    fn num(decls: &[Decl], name: &str) -> Option<f64> {
        decls
            .iter()
            .find(|d| d.name == name)
            .and_then(|d| match d.groups.first()?.first()? {
                Value::Number(n) => Some(*n),
                _ => None,
            })
    }

    #[test]
    fn box_bundle_carries_its_geometry_and_paint() {
        let b = primitive_bundle(ShapeKind::Box);
        assert_eq!(num(&b, "radius"), Some(6.0));
        assert_eq!(num(&b, "padding"), Some(20.0));
        assert_eq!(num(&b, "gap"), Some(20.0));
        assert_eq!(num(&b, "stroke-width"), Some(2.0));
        assert!(has(&b, "fill") && has(&b, "stroke"));
    }

    #[test]
    fn slant_carries_skew_icon_carries_size() {
        assert_eq!(num(&primitive_bundle(ShapeKind::Slant), "skew"), Some(15.0));
        let icon = primitive_bundle(ShapeKind::Icon);
        assert_eq!(num(&icon, "width"), Some(24.0));
        assert_eq!(num(&icon, "height"), Some(24.0));
    }

    #[test]
    fn group_template_is_a_dashed_frame() {
        let g = template_bundle("group");
        assert!(g.iter().any(|d| d.name == "stroke-style"));
        assert_eq!(num(&g, "stroke-width"), Some(1.0));
        assert!(template_bundle("oval").is_empty());
    }

    #[test]
    fn root_and_wire_defaults_are_present() {
        assert_eq!(num(&root_defaults(), "padding"), Some(20.0));
        assert_eq!(num(&root_defaults(), "font-size"), Some(15.0));
        assert_eq!(num(&wire_defaults(), "clearance"), Some(16.0));
        assert_eq!(num(&wire_defaults(), "font-size"), Some(11.0));
    }
}

//! Every built-in default, expressed as parser-shaped [`Decl`]s. This is the one
//! place Lini's look is tuned; desugar lowers these into `.lini-*` class defs and
//! the global block, and `resolve` reads [`link_defaults`] as the baked link base.
//! Visual `--lini-*` colours stay live `--var` references (render emits their
//! defaults as `@layer` CSS).

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
        // The bare rectangle (SPEC §7): frameless, no padding — like a `div`.
        // It keeps the default `stroke-width` (invisible while `stroke: none`, so
        // bbox geometry is unchanged from the old `|plain|`, and a styled `|block|`
        // gets a sensible 2px border); the `|box|` template lifts paint/radius/
        // padding back on top.
        Block => vec![
            id("fill", "none"),
            id("stroke", "none"),
            n("stroke-width", 2.0),
            n("padding", 0.0),
            n("gap", 20.0),
        ],
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
        // The default node: a rounded, framed card over the bare `|block|` base.
        "box" => vec![
            var("fill", "fill"),
            var("stroke", "stroke"),
            n("stroke-width", 2.0),
            n("padding", 20.0),
            n("radius", 8.0),
        ],
        "rect" => vec![n("radius", 0.0)],
        "group" => vec![
            var("stroke", "group-stroke"),
            id("stroke-style", "dashed"),
            n("stroke-width", 1.0),
            var("fill", "group-fill"),
            n("radius", 8.0),
            n("padding", 20.0),
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
            var("fill", "accent"),
            var("color", "accent-text"),
            n("font-size", 11.0),
            id("font-weight", "normal"),
        ],
        "note" => vec![
            n("radius", 2.0),
            n("shadow", 2.0),
            var("fill", "note-bg"),
            n("padding", 20.0),
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

/// The baked link base (SPEC §11.5): a link's lowest-specificity layer, resolved
/// per link below the scope's `link*` / `clearance` / `routing` cascade, the
/// class rules, and the link's own block.
pub fn link_defaults() -> Vec<Decl> {
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
    fn block_is_bare_and_box_template_carries_the_paint() {
        // The bare primitive: frameless, no padding, just the container gap.
        let block = primitive_bundle(ShapeKind::Block);
        assert_eq!(num(&block, "padding"), Some(0.0));
        assert_eq!(num(&block, "gap"), Some(20.0));
        assert!(!has(&block, "radius"));
        // The |box| template lifts the framed-card paint back on top.
        let boxt = template_bundle("box");
        assert_eq!(num(&boxt, "radius"), Some(8.0));
        assert_eq!(num(&boxt, "padding"), Some(20.0));
        assert_eq!(num(&boxt, "stroke-width"), Some(2.0));
        assert!(has(&boxt, "fill") && has(&boxt, "stroke"));
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
    fn root_and_link_defaults_are_present() {
        assert_eq!(num(&root_defaults(), "padding"), Some(20.0));
        assert_eq!(num(&root_defaults(), "font-size"), Some(15.0));
        assert_eq!(num(&link_defaults(), "clearance"), Some(16.0));
        assert_eq!(num(&link_defaults(), "font-size"), Some(11.0));
    }
}

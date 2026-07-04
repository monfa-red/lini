//! Gradient paints [SPEC 10.3]: `gradient()` / `linear-gradient()` /
//! `radial-gradient()` on any paint — `fill`, `stroke`, `link-color`, `gap-color`
//! (the rewrite is property-agnostic: any gradient-valued attr). Twin of the drop-shadow filter
//! table — every distinct gradient is interned once, emitted as a
//! `<linearGradient>` / `<radialGradient>` in `<defs>`, and the paint use-site
//! rewritten to a `url(#…)` reference.
//!
//! [`lower`] runs once, post-layout, over the owned scene: it dedups structurally
//! (so it needs no `--bake-vars` context — only the def emission formats stops),
//! rewrites each gradient paint to `RawCss("url(#…)")` so the ordinary formatter
//! emits it verbatim everywhere (inline diff and class rules alike), and stores the
//! definitions on the scene. `objectBoundingBox` units fit one def to any shape.

use super::values::{format_value, num};
use crate::Options;
use crate::layout::{GradientDef, GradientKind, LaidOut, PlacedNode};
use crate::resolve::{AttrMap, ResolvedValue, VarTable};
use std::fmt::Write;

/// Recognise and parse a gradient paint. `None` if `v` is not a gradient (or has
/// fewer than two stops).
fn parse(v: &ResolvedValue) -> Option<GradientDef> {
    let ResolvedValue::Call(c) = v else {
        return None;
    };
    let (kind, stops) = match c.name.as_str() {
        "gradient" => (GradientKind::Linear(135.0), c.args.clone()),
        "linear-gradient" => {
            let angle = c.args.first()?.as_number()?;
            (GradientKind::Linear(angle), c.args[1..].to_vec())
        }
        "radial-gradient" => (GradientKind::Radial, c.args.clone()),
        _ => return None,
    };
    (stops.len() >= 2).then_some(GradientDef { kind, stops })
}

/// A structural dedup key (bake-independent): kind + angle + the stops. Two paint
/// sites with the same gradient share one definition.
fn key(g: &GradientDef) -> String {
    let kind = match g.kind {
        GradientKind::Linear(a) => format!("L{}", num(a)),
        GradientKind::Radial => "R".to_string(),
    };
    format!("{kind}|{:?}", g.stops)
}

/// Intern every distinct gradient, rewrite each paint use-site to `url(#…)`, and
/// store the definitions on the scene [SPEC 10.3]. Runs once after layout.
pub(crate) fn lower(laid: &mut LaidOut) {
    let mut keys: Vec<String> = Vec::new();
    for node in &mut laid.nodes {
        lower_node(node, &mut laid.gradients, &mut keys);
    }
    for link in &mut laid.links {
        lower_attrs(&mut link.attrs, &mut laid.gradients, &mut keys);
    }
    for (_, attrs) in &mut laid.sheet.class_rules {
        lower_attrs(attrs, &mut laid.gradients, &mut keys);
    }
    lower_attrs(
        &mut laid.sheet.link_defaults,
        &mut laid.gradients,
        &mut keys,
    );
    if let Some(fill) = &mut laid.canvas_fill {
        rewrite(fill, &mut laid.gradients, &mut keys);
    }
}

fn lower_node(node: &mut PlacedNode, defs: &mut Vec<GradientDef>, keys: &mut Vec<String>) {
    lower_attrs(&mut node.attrs, defs, keys);
    for child in &mut node.children {
        lower_node(child, defs, keys);
    }
}

fn lower_attrs(attrs: &mut AttrMap, defs: &mut Vec<GradientDef>, keys: &mut Vec<String>) {
    for value in attrs.map.values_mut() {
        rewrite(value, defs, keys);
    }
}

/// If `value` is a gradient, intern it and replace it with its `url(#…)` reference.
fn rewrite(value: &mut ResolvedValue, defs: &mut Vec<GradientDef>, keys: &mut Vec<String>) {
    let Some(g) = parse(value) else { return };
    let k = key(&g);
    let idx = keys.iter().position(|e| *e == k).unwrap_or_else(|| {
        keys.push(k);
        defs.push(g);
        keys.len() - 1
    });
    *value = ResolvedValue::RawCss(format!("url(#lini-gradient-{})", idx + 1));
}

/// Emit every collected gradient as a `<linearGradient>` / `<radialGradient>`, in
/// the order their `url(#…)` ids were assigned.
pub(crate) fn emit_defs(laid: &LaidOut, out: &mut String, opts: &Options) {
    for (i, g) in laid.gradients.iter().enumerate() {
        let id = format!("lini-gradient-{}", i + 1);
        let stops = stops_svg(g, &laid.vars, opts);
        match g.kind {
            // SVG's default vector points east; CSS angles are clockwise from north,
            // so a `rotate(angle − 90)` about the centre lands the requested heading.
            GradientKind::Linear(angle) => writeln!(
                out,
                r#"    <linearGradient id="{}" gradientTransform="rotate({} 0.5 0.5)">{}</linearGradient>"#,
                id,
                num(angle - 90.0),
                stops,
            ),
            GradientKind::Radial => {
                writeln!(out, r#"    <radialGradient id="{}">{}</radialGradient>"#, id, stops)
            }
        }
        .unwrap();
    }
}

/// The evenly-spaced `<stop>`s. A live stop rides `style="stop-color: var(…)"` so
/// the browser resolves the var and it flips; a baked stop is a `stop-color`
/// attribute literal that resvg/email render.
fn stops_svg(g: &GradientDef, vars: &VarTable, opts: &Options) -> String {
    let n = g.stops.len();
    let mut out = String::new();
    for (i, stop) in g.stops.iter().enumerate() {
        let offset = if n <= 1 {
            0.0
        } else {
            i as f64 / (n as f64 - 1.0) * 100.0
        };
        let color = format_value(stop, vars, opts);
        if opts.bake_vars {
            write!(
                out,
                r#"<stop offset="{}%" stop-color="{}"/>"#,
                num(offset),
                color
            )
        } else {
            write!(
                out,
                r#"<stop offset="{}%" style="stop-color: {}"/>"#,
                num(offset),
                color
            )
        }
        .unwrap();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::ResolvedCall;

    fn grad(name: &str, args: Vec<ResolvedValue>) -> ResolvedValue {
        ResolvedValue::Call(ResolvedCall {
            name: name.into(),
            args,
        })
    }

    fn hue(name: &str) -> ResolvedValue {
        ResolvedValue::LiveVar {
            name: name.into(),
            raw: false,
        }
    }

    #[test]
    fn gradient_defaults_to_135_linear() {
        let g = parse(&grad("gradient", vec![hue("teal"), hue("sky")])).unwrap();
        assert!(matches!(g.kind, GradientKind::Linear(a) if a == 135.0));
        assert_eq!(g.stops.len(), 2);
    }

    #[test]
    fn linear_gradient_takes_a_leading_angle() {
        let g = parse(&grad(
            "linear-gradient",
            vec![ResolvedValue::Number(60.0), hue("rose"), hue("amber")],
        ))
        .unwrap();
        assert!(matches!(g.kind, GradientKind::Linear(a) if a == 60.0));
        assert_eq!(g.stops.len(), 2);
    }

    #[test]
    fn radial_and_multi_stop() {
        let g = parse(&grad(
            "radial-gradient",
            vec![hue("rose"), hue("amber"), hue("sky")],
        ))
        .unwrap();
        assert!(matches!(g.kind, GradientKind::Radial));
        assert_eq!(g.stops.len(), 3);
    }

    #[test]
    fn fewer_than_two_stops_is_not_a_gradient() {
        assert!(parse(&grad("gradient", vec![hue("teal")])).is_none());
    }

    #[test]
    fn non_gradient_call_is_ignored() {
        assert!(parse(&grad("rgb", vec![ResolvedValue::Number(1.0)])).is_none());
    }

    #[test]
    fn identical_gradients_share_one_id() {
        let mut defs = Vec::new();
        let mut keys = Vec::new();
        let mut a = grad("gradient", vec![hue("teal"), hue("sky")]);
        let mut b = grad("gradient", vec![hue("teal"), hue("sky")]);
        rewrite(&mut a, &mut defs, &mut keys);
        rewrite(&mut b, &mut defs, &mut keys);
        assert_eq!(defs.len(), 1, "identical gradients must dedup");
        assert!(matches!(&a, ResolvedValue::RawCss(s) if s == "url(#lini-gradient-1)"));
        assert!(matches!(&b, ResolvedValue::RawCss(s) if s == "url(#lini-gradient-1)"));
    }
}

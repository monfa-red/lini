//! Defs-backed paints [SPEC 10.3]: **gradients** (`gradient()` /
//! `linear-gradient()` / `radial-gradient()`, on any paint) and **hatches**
//! (`hatch()`, the drafting section-line texture — resolve gates it to
//! `fill`). Twin of the drop-shadow filter table — every distinct paint is
//! interned once, emitted as a `<linearGradient>` / `<radialGradient>` /
//! `<pattern>` in `<defs>`, and the use-site rewritten to a `url(#…)`
//! reference.
//!
//! [`lower`] runs once, post-layout, over the owned scene: it dedups structurally
//! (so it needs no `--bake-vars` context — only the def emission formats stops),
//! rewrites each gradient paint to `RawCss("url(#…)")` so the ordinary formatter
//! emits it verbatim everywhere (inline diff and class rules alike), and stores the
//! definitions on the scene. `objectBoundingBox` units fit one def to any shape.

use super::values::{format_value, num};
use crate::Options;
use crate::layout::{GradientDef, GradientKind, HatchDef, LaidOut, PlacedNode};
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

/// The two interning tables, threaded through one walk.
#[derive(Default)]
struct Interner {
    gradient_keys: Vec<String>,
    gradients: Vec<GradientDef>,
    hatch_keys: Vec<String>,
    hatches: Vec<HatchDef>,
    /// Distinct clip radii [SPEC 15.8], in encounter order — a detail view's
    /// `clip:` circle is interned by radius, one `<clipPath>` per distinct one.
    clips: Vec<f64>,
}

/// Intern every distinct gradient and hatch, rewrite each paint use-site to
/// `url(#…)`, and store the definitions on the scene [SPEC 10.3]. Runs once
/// after layout.
pub(crate) fn lower(laid: &mut LaidOut) {
    let mut it = Interner::default();
    for node in &mut laid.nodes {
        lower_node(node, &mut it);
    }
    for link in &mut laid.links {
        lower_attrs(&mut link.attrs, &mut it);
    }
    for (_, attrs) in &mut laid.sheet.class_rules {
        lower_attrs(attrs, &mut it);
    }
    lower_attrs(&mut laid.sheet.link_defaults, &mut it);
    if let Some(fill) = &mut laid.canvas_fill {
        rewrite(fill, &mut it);
    }
    laid.gradients = it.gradients;
    laid.hatches = it.hatches;
    laid.clips = it.clips;
}

fn lower_node(node: &mut PlacedNode, it: &mut Interner) {
    lower_attrs(&mut node.attrs, it);
    lower_clip(&mut node.attrs, it);
    for child in &mut node.children {
        lower_node(child, it);
    }
}

/// A `|detail|`'s `clip:` circle [SPEC 15.8]: intern the radius and rewrite the
/// attr to its `url(#lini-clip-N)` reference, exactly as the paints do.
fn lower_clip(attrs: &mut AttrMap, it: &mut Interner) {
    let Some(ResolvedValue::Number(r)) = attrs.get("clip") else {
        return;
    };
    let r = *r;
    let idx = it
        .clips
        .iter()
        .position(|e| (e - r).abs() < 1e-9)
        .unwrap_or_else(|| {
            it.clips.push(r);
            it.clips.len() - 1
        });
    attrs.insert(
        "clip",
        ResolvedValue::RawCss(format!("url(#lini-clip-{})", idx + 1)),
    );
}

fn lower_attrs(attrs: &mut AttrMap, it: &mut Interner) {
    for value in attrs.map.values_mut() {
        rewrite(value, it);
    }
}

/// If `value` is a gradient or a hatch, intern it and replace it with its
/// `url(#…)` reference.
fn rewrite(value: &mut ResolvedValue, it: &mut Interner) {
    if let Some(g) = parse(value) {
        let k = key(&g);
        let idx = it
            .gradient_keys
            .iter()
            .position(|e| *e == k)
            .unwrap_or_else(|| {
                it.gradient_keys.push(k);
                it.gradients.push(g);
                it.gradient_keys.len() - 1
            });
        *value = ResolvedValue::RawCss(format!("url(#lini-gradient-{})", idx + 1));
        return;
    }
    if let Some(h) = parse_hatch(value) {
        let k = hatch_key(&h);
        let idx = it
            .hatch_keys
            .iter()
            .position(|e| *e == k)
            .unwrap_or_else(|| {
                it.hatch_keys.push(k);
                it.hatches.push(h);
                it.hatch_keys.len() - 1
            });
        *value = ResolvedValue::RawCss(format!("url(#lini-hatch-{})", idx + 1));
    }
}

/// Recognise and parse a hatch paint [SPEC 10.3]: `hatch(angle[s][, pitch[,
/// colour]])` — angles a number or a space-group, pitch sheet-space px
/// (default 6), colour an ordinary paint (default `--stroke`).
fn parse_hatch(v: &ResolvedValue) -> Option<HatchDef> {
    let ResolvedValue::Call(c) = v else {
        return None;
    };
    if c.name != "hatch" {
        return None;
    }
    let angles: Vec<f64> = match c.args.first()? {
        ResolvedValue::Number(a) => vec![*a],
        ResolvedValue::Tuple(xs) => xs.iter().filter_map(ResolvedValue::as_number).collect(),
        _ => return None,
    };
    if angles.is_empty() {
        return None;
    }
    let pitch = c
        .args
        .get(1)
        .and_then(ResolvedValue::as_number)
        .filter(|p| *p > 0.0)
        .unwrap_or(6.0);
    let color = c.args.get(2).cloned().unwrap_or(ResolvedValue::LiveVar {
        name: "stroke".into(),
        raw: false,
    });
    Some(HatchDef {
        angles,
        pitch,
        color,
    })
}

fn hatch_key(h: &HatchDef) -> String {
    format!("{:?}|{}|{:?}", h.angles, num(h.pitch), h.color)
}

/// Emit every collected paint def — gradients, then hatch `<pattern>`s — in
/// the order their `url(#…)` ids were assigned.
pub(crate) fn emit_defs(laid: &LaidOut, out: &mut String, opts: &Options) {
    emit_gradients(laid, out, opts);
    emit_hatches(laid, out, opts);
    emit_clips(laid, out);
}

/// One `<clipPath>` per distinct detail-view clip radius [SPEC 15.8]: a circle
/// at the group's own origin (the region centre the detail recentres to),
/// `userSpaceOnUse` so it clips the placed geometry directly.
fn emit_clips(laid: &LaidOut, out: &mut String) {
    for (i, r) in laid.clips.iter().enumerate() {
        let _ = writeln!(
            out,
            r#"    <clipPath id="lini-clip-{}" clipPathUnits="userSpaceOnUse"><circle r="{}"/></clipPath>"#,
            i + 1,
            num(*r)
        );
    }
}

/// The drafting hatch tile [SPEC 10.3]: a `pitch × pitch` square turned to
/// the first bearing (0 = up, clockwise — `rotate()` in SVG screen coords is
/// exactly that), holding one full-height line per family. A family at +90°
/// from the first is the full-width line — the standard cross-hatch tiles
/// exactly at any bearing; other offsets draw through the tile centre, best
/// effort. Line width is fixed at 0.75 — a texture, not a stroke.
fn emit_hatches(laid: &LaidOut, out: &mut String, opts: &Options) {
    for (i, h) in laid.hatches.iter().enumerate() {
        let p = h.pitch;
        let color = format_value(&h.color, &laid.vars, opts);
        let paint = if opts.bake_vars {
            format!(r#"stroke="{color}""#)
        } else {
            format!(r#"style="stroke: {color}""#)
        };
        let mut lines = String::new();
        let base = h.angles.first().copied().unwrap_or(45.0);
        for a in &h.angles {
            let d = (a - base).rem_euclid(180.0);
            if d < 1e-6 || (d - 180.0).abs() < 1e-6 {
                // The base family: a vertical line mid-tile (bearing 0 = up).
                write!(
                    lines,
                    r#"<line x1="{m}" y1="0" x2="{m}" y2="{p}" {paint} stroke-width="0.75"/>"#,
                    m = num(p / 2.0),
                    p = num(p),
                )
                .unwrap();
            } else if (d - 90.0).abs() < 1e-6 {
                // The perpendicular family — the cross-hatch, exact.
                write!(
                    lines,
                    r#"<line x1="0" y1="{m}" x2="{p}" y2="{m}" {paint} stroke-width="0.75"/>"#,
                    m = num(p / 2.0),
                    p = num(p),
                )
                .unwrap();
            } else {
                // An oblique extra family has no shared tile period — drawn
                // through the tile centre, best effort.
                let rad = d.to_radians();
                let (dx, dy) = (rad.sin() * p, -rad.cos() * p);
                write!(
                    lines,
                    r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" {paint} stroke-width="0.75"/>"#,
                    x1 = num(p / 2.0 - dx),
                    y1 = num(p / 2.0 - dy),
                    x2 = num(p / 2.0 + dx),
                    y2 = num(p / 2.0 + dy),
                )
                .unwrap();
            }
        }
        writeln!(
            out,
            r#"    <pattern id="lini-hatch-{}" patternUnits="userSpaceOnUse" width="{}" height="{}" patternTransform="rotate({})">{}</pattern>"#,
            i + 1,
            num(p),
            num(p),
            num(base),
            lines,
        )
        .unwrap();
    }
}

/// Emit every collected gradient as a `<linearGradient>` / `<radialGradient>`, in
/// the order their `url(#…)` ids were assigned.
fn emit_gradients(laid: &LaidOut, out: &mut String, opts: &Options) {
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
    fn hatch_parses_its_forms_and_dedups() {
        // Bare angle → pitch 6, `--stroke`.
        let h = parse_hatch(&grad("hatch", vec![ResolvedValue::Number(45.0)])).unwrap();
        assert_eq!((h.angles.as_slice(), h.pitch), ([45.0].as_slice(), 6.0));
        assert!(matches!(&h.color, ResolvedValue::LiveVar { name, .. } if name == "stroke"));

        // A space-group of angles + explicit pitch — the cross-hatch.
        let cross = parse_hatch(&grad(
            "hatch",
            vec![
                ResolvedValue::Tuple(vec![
                    ResolvedValue::Number(45.0),
                    ResolvedValue::Number(-45.0),
                ]),
                ResolvedValue::Number(4.0),
            ],
        ))
        .unwrap();
        assert_eq!(cross.angles, vec![45.0, -45.0]);
        assert_eq!(cross.pitch, 4.0);

        // Same hatch interns once; a different pitch is a new def.
        let mut it = Interner::default();
        let mut a = grad("hatch", vec![ResolvedValue::Number(45.0)]);
        let mut b = grad("hatch", vec![ResolvedValue::Number(45.0)]);
        let mut c = grad(
            "hatch",
            vec![ResolvedValue::Number(45.0), ResolvedValue::Number(4.0)],
        );
        rewrite(&mut a, &mut it);
        rewrite(&mut b, &mut it);
        rewrite(&mut c, &mut it);
        assert_eq!(it.hatches.len(), 2, "45/6 dedups; 45/4 is distinct");
        assert!(matches!(&a, ResolvedValue::RawCss(s) if s == "url(#lini-hatch-1)"));
        assert!(matches!(&b, ResolvedValue::RawCss(s) if s == "url(#lini-hatch-1)"));
        assert!(matches!(&c, ResolvedValue::RawCss(s) if s == "url(#lini-hatch-2)"));
    }

    #[test]
    fn identical_gradients_share_one_id() {
        let mut it = Interner::default();
        let mut a = grad("gradient", vec![hue("teal"), hue("sky")]);
        let mut b = grad("gradient", vec![hue("teal"), hue("sky")]);
        rewrite(&mut a, &mut it);
        rewrite(&mut b, &mut it);
        assert_eq!(it.gradients.len(), 1, "identical gradients must dedup");
        assert!(matches!(&a, ResolvedValue::RawCss(s) if s == "url(#lini-gradient-1)"));
        assert!(matches!(&b, ResolvedValue::RawCss(s) if s == "url(#lini-gradient-1)"));
    }
}

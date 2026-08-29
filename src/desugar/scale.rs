//! The scale fold [SPEC 15.1/18]: a drawing scope's `scale:` (the drafting
//! **ratio**, default 1) × `unit:` (mm per drawing unit — `mm`/`cm`/`m`/`in`,
//! nearest-wins, default mm) × the root `density:` (px per mm, default 4)
//! become one generated internal **`px-per-unit:`** — the engine's existing
//! multiplier — so the layout core stays dumb and `lini desugar` shows the
//! number. A `|page|` folds the density alone (mm paper, ratio locked at 1 —
//! a page carries no `scale:` of its own [SPEC 15.8]). The pass recomputes
//! from the same authored inputs every time, so desugar stays idempotent.
//!
//! The fold reads **authored decls** (and the worn `.lini-*` classes of an
//! already-lowered file); a rule-borne `scale:` stays what it reaches the
//! engine as — a raw multiplier.

use super::nest::{in_drawing_scope, is_drawing_body, is_page_body};
use super::schematic::lowered_chain;
use crate::error::Error;
use crate::span::Span;
use crate::syntax::ast::{Child, Decl, Value};

/// The generated internal attr name [SPEC 19] — whitelisted in validation.
pub(crate) const PX_PER_UNIT: &str = "px-per-unit";

/// The unit / density context carried down the lowered tree.
struct ScaleCtx {
    density: f64,
    unit_mm: f64,
    in_drawing: bool,
}

/// Fold the whole lowered scene. `user_root` is the root's own decl list —
/// a `layout: drawing` root is itself a drawing scope and gets the stamp.
pub(super) fn fold(
    instances: &mut [Child],
    user_root: &mut Vec<Decl>,
    root_drawing: bool,
) -> Result<(), Error> {
    let density = read_density(user_root)?;
    let unit_mm = read_unit(user_root)?.unwrap_or(1.0);
    let ctx = ScaleCtx {
        density,
        unit_mm,
        in_drawing: root_drawing,
    };
    if root_drawing {
        stamp(user_root, &ctx, Span::empty())?;
    }
    for c in instances.iter_mut() {
        walk(c, &ctx)?;
    }
    Ok(())
}

fn walk(child: &mut Child, ctx: &ScaleCtx) -> Result<(), Error> {
    let Child::Box(n) = child else { return Ok(()) };
    let mut ctx = ScaleCtx {
        density: ctx.density,
        unit_mm: ctx.unit_mm,
        in_drawing: ctx.in_drawing,
    };
    let chain = lowered_chain(n);
    let opens = is_drawing_body(&chain, &n.style);
    if is_page_body(&chain) {
        if let Some(d) = find(&n.style, "scale") {
            return Err(Error::at(
                d.span,
                "a '|page|' carries no 'scale:' — 'density:' sets its pixels per millimetre (root), a drawing's 'scale:' its drafting ratio",
            ));
        }
        if let Some(u) = read_unit(&n.style)? {
            ctx.unit_mm = u;
        }
        // Paper is millimetres: px-per-unit is the density alone.
        n.style.retain(|d| d.name != PX_PER_UNIT);
        n.style.push(number_decl(ctx.density, n.span));
    } else if opens {
        if let Some(u) = read_unit(&n.style)? {
            ctx.unit_mm = u;
        }
        stamp(&mut n.style, &ctx, n.span)?;
    } else if ctx.in_drawing && find(&n.style, "scale").is_some() {
        // A node-level ratio override inside a drawing scope [SPEC 15.1].
        stamp(&mut n.style, &ctx, n.span)?;
    }
    ctx.in_drawing = in_drawing_scope(opens, ctx.in_drawing, &chain, &n.style);
    for c in &mut n.children {
        walk(c, &ctx)?;
    }
    Ok(())
}

/// Replace any prior stamp and push `px-per-unit = ratio × unit-mm × density`.
/// A `(…)` ratio stays symbolic — resolve folds it — multiplied by the base.
fn stamp(style: &mut Vec<Decl>, ctx: &ScaleCtx, span: Span) -> Result<(), Error> {
    style.retain(|d| d.name != PX_PER_UNIT);
    let base = ctx.unit_mm * ctx.density;
    let decl = match find(style, "scale") {
        None => number_decl(base, span),
        Some(d) => match single(d) {
            Some(Value::Number(r)) if *r > 0.0 => number_decl(r * base, d.span),
            Some(Value::Expr(src)) => Decl {
                name: PX_PER_UNIT.into(),
                groups: vec![vec![Value::Expr(format!("({src}) * {base}"))]],
                span: d.span,
            },
            _ => return Err(Error::at(d.span, "'scale' must be > 0")),
        },
    };
    style.push(decl);
    Ok(())
}

/// **The** physical-millimetre → drawing-unit conversion [SPEC 15.11]: a
/// floorplan type's true-size default — a wall's 200 mm thickness, a door's
/// 900 mm clear width, every fixture body — is stated in physical millimetres
/// and read through the scope's own `unit:` (`unit_mm` millimetres to the
/// drawing unit, [`read_unit`]), so a bed is 1500 × 2000 mm whether the file
/// drafts in `m` or `mm`. An **authored** value is drawing units like
/// everything else and never passes through here.
///
/// It lives beside the `unit:` reader because that is where the scope's
/// millimetres-per-unit is known; every true-size consumer calls this one
/// function.
#[cfg_attr(not(test), allow(dead_code))] // the wall / opening / fixture readers land in phases 2–4
pub(crate) fn mm_to_units(mm: f64, unit_mm: f64) -> f64 {
    mm / unit_mm
}

/// The nearest authored `unit:` as millimetres per drawing unit [SPEC 15.1].
/// Only the fold's own scopes (root, pages, drawings) are read, so an
/// `|axis|`'s quoted tick suffix never meets this enum.
fn read_unit(style: &[Decl]) -> Result<Option<f64>, Error> {
    let Some(d) = find(style, "unit") else {
        return Ok(None);
    };
    let mm = match single(d) {
        Some(Value::Ident(u)) => match u.as_str() {
            "mm" => Some(1.0),
            "cm" => Some(10.0),
            "m" => Some(1000.0),
            "in" => Some(25.4),
            _ => None,
        },
        _ => None,
    };
    mm.map(Some)
        .ok_or_else(|| Error::at(d.span, "'unit' is mm, cm, m, or in"))
}

/// The root `density:` — px per mm, default 4, must be positive [SPEC 15.1].
fn read_density(user_root: &[Decl]) -> Result<f64, Error> {
    let Some(d) = find(user_root, "density") else {
        return Ok(4.0);
    };
    match single(d) {
        Some(Value::Number(n)) if *n > 0.0 => Ok(*n),
        _ => Err(Error::at(d.span, "'density' must be > 0")),
    }
}

fn find<'a>(style: &'a [Decl], name: &str) -> Option<&'a Decl> {
    style.iter().rev().find(|d| d.name == name)
}

fn single(d: &Decl) -> Option<&Value> {
    match d.groups.as_slice() {
        [group] => match group.as_slice() {
            [v] => Some(v),
            _ => None,
        },
        _ => None,
    }
}

fn number_decl(v: f64, span: Span) -> Decl {
    Decl {
        name: PX_PER_UNIT.into(),
        groups: vec![vec![Value::Number(v)]],
        span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The true-size law [SPEC 15.11]: a physical millimetre default reads the
    /// same size at every `unit:` — 200 mm is `200` drafting in mm, `0.2`
    /// drafting in m, `20` in cm — while an authored value stays drawing units
    /// and never meets this function.
    #[test]
    fn a_true_size_default_converts_through_the_scope_unit() {
        let mm_per_unit = |src: &str| {
            read_unit(&[Decl {
                name: "unit".into(),
                groups: vec![vec![Value::Ident(src.into())]],
                span: Span::empty(),
            }])
            .expect("a known unit")
            .expect("a value")
        };
        assert_eq!(mm_to_units(200.0, mm_per_unit("mm")), 200.0);
        assert_eq!(mm_to_units(200.0, mm_per_unit("cm")), 20.0);
        assert_eq!(mm_to_units(200.0, mm_per_unit("m")), 0.2);
        // The scope default: no `unit:` at all is millimetres [SPEC 15.1].
        assert_eq!(mm_to_units(900.0, 1.0), 900.0);
    }
}

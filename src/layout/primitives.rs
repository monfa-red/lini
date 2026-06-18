//! Per-shape bbox computation (SPEC §6–§7).
//!
//! A closed shape sizes **border-box**: `width`/`height` each default `auto` =
//! content + `padding` on that axis; an empty one is `2 × padding`; an explicit
//! dimension is the exact drawn size with padding inside it. Strokes count
//! toward the bbox (half each side). `|text|` sizes to its glyphs (no padding),
//! `|icon|` to `icon-size`, and the geometry primitives to their `points`/`src`.

use super::ir::Bbox;
use super::text;
use super::values::{as_pair, expand_box_value, layout_var};
use crate::error::Error;
use crate::resolve::{AttrMap, ResolvedInst, ResolvedValue, ShapeKind, VarTable};
use crate::span::Span;

#[derive(Default, Clone, Copy)]
pub struct PaddingBox {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl PaddingBox {
    pub fn uniform(n: f64) -> Self {
        Self {
            top: n,
            right: n,
            bottom: n,
            left: n,
        }
    }
}

/// Bbox for a leaf primitive (no flow children). A closed shape is empty here —
/// `2 × padding` (or its explicit dims); text / icon / geometry size to their
/// own content.
pub fn leaf_bbox(inst: &ResolvedInst, vars: &VarTable) -> Result<Bbox, Error> {
    match inst.shape {
        ShapeKind::Box
        | ShapeKind::Oval
        | ShapeKind::Hex
        | ShapeKind::Slant
        | ShapeKind::Cyl
        | ShapeKind::Diamond
        | ShapeKind::Cloud => closed_bbox(inst, Bbox::empty(), vars),
        ShapeKind::Text => {
            let size = font_size(inst, vars);
            let label = inst.label.as_deref().unwrap_or("");
            Ok(Bbox::centered(
                text::approx_width(label, size),
                text::approx_height(label, size),
            ))
        }
        ShapeKind::Icon => {
            let size = inst
                .attrs
                .number("width")
                .or_else(|| inst.attrs.number("height"))
                .or_else(|| layout_var(vars, "icon-size"))
                .unwrap_or(24.0);
            Ok(Bbox::centered(size, size))
        }
        ShapeKind::Line => Ok(bounding_box(&require_points(inst, "line", 2)?)
            .inflate(stroke_half(inst, vars))),
        ShapeKind::Poly => Ok(bounding_box(&require_points(inst, "poly", 3)?)
            .inflate(stroke_half(inst, vars))),
        ShapeKind::Image => {
            let (w, h) = image_dims(inst)?;
            Ok(Bbox::centered(w, h))
        }
        // Native top-left coords (SPEC §7); a real bbox needs SVG path parsing.
        ShapeKind::Path => Ok(Bbox::empty()),
    }
}

/// A closed shape's bbox: each axis is its explicit `width`/`height` (border-box
/// — padding inside it) or `content + padding` on that axis, then inflated by
/// half the stroke so the outline counts toward the bbox (SPEC §6).
pub fn closed_bbox(inst: &ResolvedInst, content: Bbox, vars: &VarTable) -> Result<Bbox, Error> {
    let pad = padding(&inst.attrs, vars, inst.span)?;
    let w = inst
        .attrs
        .number("width")
        .unwrap_or(content.w() + pad.left + pad.right);
    let h = inst
        .attrs
        .number("height")
        .unwrap_or(content.h() + pad.top + pad.bottom);
    Ok(Bbox::centered(w, h).inflate(stroke_half(inst, vars)))
}

pub fn padding(attrs: &AttrMap, vars: &VarTable, span: Span) -> Result<PaddingBox, Error> {
    if let Some(v) = attrs.get("padding") {
        let (t, r, b, l) = expand_box_value(v, span)?;
        Ok(PaddingBox {
            top: t,
            right: r,
            bottom: b,
            left: l,
        })
    } else {
        Ok(PaddingBox::uniform(layout_var(vars, "padding").unwrap_or(16.0)))
    }
}

/// A child's `margin:` as `(top, right, bottom, left)` — signed outer spacing,
/// `N` / `v h` / `t r b l`, negatives allowed (they tighten). Absent → zero.
pub fn margin(attrs: &AttrMap, span: Span) -> Result<(f64, f64, f64, f64), Error> {
    match attrs.get("margin") {
        Some(v) => expand_box_value(v, span),
        None => Ok((0.0, 0.0, 0.0, 0.0)),
    }
}

/// `gap` → `(between_rows, between_cols)`. Scalar = both equal; `row col` (CSS
/// order) per axis. Negative allowed.
pub fn gap(attrs: &AttrMap, vars: &VarTable, span: Span) -> Result<(f64, f64), Error> {
    if let Some(v) = attrs.get("gap") {
        let nums = super::values::as_number_tuple(v, span)?;
        Ok(match nums.len() {
            1 => (nums[0], nums[0]),
            2 => (nums[0], nums[1]),
            n => {
                return Err(Error::at(
                    span,
                    format!("'gap' expects 1 or 2 values, got {}", n),
                ));
            }
        })
    } else {
        let g = layout_var(vars, "gap").unwrap_or(20.0);
        Ok((g, g))
    }
}

// ───────────────────────── Internal helpers ─────────────────────────

fn font_size(inst: &ResolvedInst, vars: &VarTable) -> f64 {
    inst.attrs
        .number("font-size")
        .or_else(|| layout_var(vars, "font-size"))
        .unwrap_or(14.0)
}

fn stroke_half(inst: &ResolvedInst, vars: &VarTable) -> f64 {
    inst.attrs
        .number("stroke-width")
        .or_else(|| layout_var(vars, "stroke-width"))
        .unwrap_or(1.0)
        / 2.0
}

fn require_points(inst: &ResolvedInst, name: &str, min: usize) -> Result<Vec<(f64, f64)>, Error> {
    let points = attr_points(&inst.attrs, "points", inst.span)?
        .ok_or_else(|| Error::at(inst.span, format!("'|{}|' requires 'points'", name)))?;
    if points.len() < min {
        return Err(Error::at(
            inst.span,
            format!("'|{}|' requires at least {} points", name, min),
        ));
    }
    Ok(points)
}

fn image_dims(inst: &ResolvedInst) -> Result<(f64, f64), Error> {
    match (inst.attrs.number("width"), inst.attrs.number("height")) {
        (Some(w), Some(h)) => Ok((w, h)),
        _ => Err(Error::at(
            inst.span,
            "'|image|' requires 'width' and 'height'",
        )),
    }
}

fn bounding_box(points: &[(f64, f64)]) -> Bbox {
    let mut bb = Bbox {
        min_x: f64::INFINITY,
        min_y: f64::INFINITY,
        max_x: f64::NEG_INFINITY,
        max_y: f64::NEG_INFINITY,
    };
    for (x, y) in points {
        bb.min_x = bb.min_x.min(*x);
        bb.min_y = bb.min_y.min(*y);
        bb.max_x = bb.max_x.max(*x);
        bb.max_y = bb.max_y.max(*y);
    }
    bb
}

pub fn attr_points(
    attrs: &AttrMap,
    name: &str,
    span: Span,
) -> Result<Option<Vec<(f64, f64)>>, Error> {
    match attrs.get(name) {
        Some(ResolvedValue::List(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(as_pair(item, span)?);
            }
            Ok(Some(out))
        }
        // A single `x y` group resolves to a Tuple, not a List — one point.
        Some(ResolvedValue::Tuple(_)) => Ok(Some(vec![as_pair(attrs.get(name).unwrap(), span)?])),
        Some(_) => Err(Error::at(
            span,
            format!("'{}' expects a list of (x, y) points", name),
        )),
        None => Ok(None),
    }
}

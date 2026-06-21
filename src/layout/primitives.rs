//! Per-shape bbox computation (SPEC §6–§7).
//!
//! A closed shape sizes **border-box**: `width`/`height` each default `auto` =
//! content + `padding` on that axis; an empty one is `2 × padding`; an explicit
//! dimension is a **floor** — it grows to content + padding rather than clip or
//! spill. Strokes count toward the bbox (half each side). `|text|` sizes to its
//! glyphs (no padding), `|icon|` to `icon-size`, and the geometry primitives to
//! their `points`/`src`.

use super::ir::Bbox;
use super::text;
use super::values::{as_pair, expand_box_value};
use crate::error::Error;
use crate::resolve::{AttrMap, ResolvedInst, ResolvedValue, ShapeKind};
use crate::span::Span;

#[derive(Default, Clone, Copy)]
pub struct PaddingBox {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

/// Bbox for a leaf primitive (no flow children). A closed shape is empty here —
/// `2 × padding` (or its explicit dims); text / icon / geometry size to their
/// own content.
pub fn leaf_bbox(inst: &ResolvedInst) -> Result<Bbox, Error> {
    match inst.shape {
        ShapeKind::Box
        | ShapeKind::Oval
        | ShapeKind::Hex
        | ShapeKind::Slant
        | ShapeKind::Cyl
        | ShapeKind::Diamond
        | ShapeKind::Cloud => closed_bbox(inst, Bbox::empty()),
        ShapeKind::Text => {
            let size = font_size(inst);
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
                .unwrap_or(0.0);
            Ok(Bbox::centered(size, size))
        }
        ShapeKind::Line => {
            Ok(bounding_box(&require_points(inst, "line", 2)?).inflate(stroke_half(inst)))
        }
        ShapeKind::Poly => {
            Ok(bounding_box(&require_points(inst, "poly", 3)?).inflate(stroke_half(inst)))
        }
        ShapeKind::Image => {
            let (w, h) = image_dims(inst)?;
            Ok(Bbox::centered(w, h))
        }
        // Native top-left coords (SPEC §7): size to the parsed path extent.
        ShapeKind::Path => {
            let Some(ResolvedValue::String(d)) = inst.attrs.get("path") else {
                return Err(Error::at(inst.span, "'|path|' requires 'path'"));
            };
            let pts = super::path_bbox::extent_points(d);
            if pts.is_empty() {
                return Ok(Bbox::empty());
            }
            Ok(bounding_box(&pts).inflate(stroke_half(inst)))
        }
    }
}

/// A closed shape's bbox: each axis is `content + padding`, with an explicit
/// `width`/`height` as a **floor** — border-box (padding inside), and the box
/// grows past the declared size rather than clip or spill its content (SPEC §6).
/// Inflated by half the stroke so the outline counts toward the bbox.
pub fn closed_bbox(inst: &ResolvedInst, content: Bbox) -> Result<Bbox, Error> {
    // A table consumes its `padding` as a per-cell inset inside the grid (SPEC
    // §8), so its outer box adds none.
    let pad = if super::grid::is_inset_grid(&inst.attrs) {
        PaddingBox::default()
    } else {
        padding(&inst.attrs, inst.span)?
    };
    let w = floor_dim(
        inst.attrs.number("width"),
        content.w(),
        pad.left + pad.right,
    );
    let h = floor_dim(
        inst.attrs.number("height"),
        content.h(),
        pad.top + pad.bottom,
    );
    Ok(Bbox::centered(w, h).inflate(stroke_half(inst)))
}

/// One axis of a closed shape, **border-box**: `content + padding`, with an
/// explicit dimension as a **floor** over it — the box grows to fit content
/// rather than clip or spill. An **empty** box (no content on this axis) keeps
/// its declared size, since there is nothing to protect; an *auto* empty box is
/// `2 × padding` (SPEC §6).
fn floor_dim(declared: Option<f64>, content: f64, pad: f64) -> f64 {
    match declared {
        None => content + pad,
        Some(d) if content > 0.0 => d.max(content + pad),
        Some(d) => d,
    }
}

pub fn padding(attrs: &AttrMap, span: Span) -> Result<PaddingBox, Error> {
    if let Some(v) = attrs.get("padding") {
        let (t, r, b, l) = expand_box_value(v, span)?;
        Ok(PaddingBox {
            top: t,
            right: r,
            bottom: b,
            left: l,
        })
    } else {
        Ok(PaddingBox::default())
    }
}

/// `gap` → `(between_rows, between_cols)`. Scalar = both equal; `row col` (CSS
/// order) per axis. Non-negative.
pub fn gap(attrs: &AttrMap, span: Span) -> Result<(f64, f64), Error> {
    let Some(v) = attrs.get("gap") else {
        return Ok((0.0, 0.0));
    };
    let nums = super::values::as_number_tuple(v, span)?;
    let (gy, gx) = match nums.len() {
        1 => (nums[0], nums[0]),
        2 => (nums[0], nums[1]),
        n => {
            return Err(Error::at(
                span,
                format!("'gap' expects 1 or 2 values, got {}", n),
            ));
        }
    };
    // Gap is non-negative, like CSS — overlap is `pin`'s job, not a spacing
    // value's. (To allow negative gaps again, drop this check; the flex/grid
    // math already handles them.)
    if gy < 0.0 || gx < 0.0 {
        return Err(Error::at(span, "'gap' must be ≥ 0"));
    }
    Ok((gy, gx))
}

// ───────────────────────── Internal helpers ─────────────────────────

fn font_size(inst: &ResolvedInst) -> f64 {
    inst.attrs.number("font-size").unwrap_or(0.0)
}

fn stroke_half(inst: &ResolvedInst) -> f64 {
    inst.attrs.number("stroke-width").unwrap_or(0.0) / 2.0
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

//! Per-shape bbox computation. Closed shapes with text-only children auto-size
//! to the text plus padding (or text-pad if no padding is set). Container
//! shapes get their bbox from already-laid-out children plus padding.

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

/// Bbox for a leaf primitive (no children — purely shape-driven dimensions).
pub fn leaf_bbox(inst: &ResolvedInst, vars: &VarTable) -> Result<Bbox, Error> {
    let bbox = geom_bbox(inst, vars)?;
    Ok(bbox.inflate(stroke_half(inst, vars)))
}

/// Bbox for a closed shape that has been auto-sized to its content (text or
/// nested children) plus padding.
pub fn auto_sized_bbox(
    inst: &ResolvedInst,
    content_bbox: Bbox,
    vars: &VarTable,
    use_text_pad: bool,
) -> Result<Bbox, Error> {
    let pad = if use_text_pad && !has_padding_attr(&inst.attrs) {
        PaddingBox::uniform(layout_var(vars, "text-pad").unwrap_or(16.0))
    } else {
        padding(&inst.attrs, vars, inst.span)?
    };
    let w = content_bbox.w() + pad.left + pad.right;
    let h = content_bbox.h() + pad.top + pad.bottom;
    let bbox = Bbox::centered(w, h);
    Ok(bbox.inflate(stroke_half(inst, vars)))
}

fn has_padding_attr(attrs: &AttrMap) -> bool {
    attrs.get("padding").is_some()
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
        Ok(PaddingBox::uniform(
            layout_var(vars, "padding").unwrap_or(0.0),
        ))
    }
}

pub fn gap(attrs: &AttrMap, vars: &VarTable, span: Span) -> Result<(f64, f64), Error> {
    // gap → (y_between_rows, x_between_cols). Scalar collapses to both equal;
    // (y, x) takes the form directly.
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

// ───────────────────────── Internal bbox computation ─────────────────────────

fn geom_bbox(inst: &ResolvedInst, vars: &VarTable) -> Result<Bbox, Error> {
    let attrs = &inst.attrs;
    match inst.shape {
        ShapeKind::Rect
        | ShapeKind::Slant
        | ShapeKind::Cyl
        | ShapeKind::Diamond
        | ShapeKind::Cloud
        | ShapeKind::Hex
        | ShapeKind::Oval => {
            let (w, h) = closed_shape_dims(inst, vars)?;
            Ok(Bbox::centered(w, h))
        }
        ShapeKind::Text => {
            let size = attrs
                .number("text-size")
                .or_else(|| layout_var(vars, "text-size"))
                .unwrap_or(13.0);
            let label = inst.label.as_deref().unwrap_or("");
            let w = text::approx_width(label, size);
            let h = text::approx_height(label, size);
            Ok(Bbox::centered(w, h))
        }
        ShapeKind::Line => {
            let points = attr_points(attrs, "points", inst.span)?.ok_or_else(|| {
                Error::at(
                    inst.span,
                    format!("'|{}|' requires 'points'", inst.shape.as_str()),
                )
            })?;
            if points.len() < 2 {
                return Err(Error::at(
                    inst.span,
                    format!("'|{}|' requires at least 2 points", inst.shape.as_str()),
                ));
            }
            Ok(bounding_box(&points))
        }
        ShapeKind::Icon => {
            let size = attrs
                .number("size")
                .or_else(|| layout_var(vars, "icon-size"))
                .unwrap_or(24.0);
            Ok(Bbox::centered(size, size))
        }
        ShapeKind::Image => {
            let (w, h) = read_size(attrs, inst.span)?
                .ok_or_else(|| Error::at(inst.span, "'|image|' requires 'size'"))?;
            Ok(Bbox::centered(w, h))
        }
        ShapeKind::Poly => {
            let points = attr_points(attrs, "points", inst.span)?
                .ok_or_else(|| Error::at(inst.span, "'|poly|' requires 'points'"))?;
            if points.len() < 3 {
                return Err(Error::at(inst.span, "'|poly|' requires at least 3 points"));
            }
            Ok(bounding_box(&points))
        }
        ShapeKind::Path => {
            // Native top-left coords (SPEC §7 rule 5). Real bbox needs SVG path
            // parsing; v1 returns a zero bbox.
            Ok(Bbox::empty())
        }
    }
}

/// Read the `size:` attr in its two forms: `size:N` (scalar = square) or
/// `size:(w, h)` (tuple = rectangle). Returns `Ok(None)` if absent.
pub fn read_size(attrs: &AttrMap, span: Span) -> Result<Option<(f64, f64)>, Error> {
    let Some(v) = attrs.get("size") else {
        return Ok(None);
    };
    match v.as_number() {
        Some(n) => Ok(Some((n, n))),
        None => Ok(Some(as_pair(v, span)?)),
    }
}

fn closed_shape_dims(inst: &ResolvedInst, vars: &VarTable) -> Result<(f64, f64), Error> {
    if let Some(dims) = read_size(&inst.attrs, inst.span)? {
        return Ok(dims);
    }
    let (default_w, default_h) = match inst.shape {
        ShapeKind::Rect | ShapeKind::Slant => (
            layout_var(vars, "rect-w").unwrap_or(100.0),
            layout_var(vars, "rect-h").unwrap_or(40.0),
        ),
        ShapeKind::Oval => (
            layout_var(vars, "oval-w").unwrap_or(60.0),
            layout_var(vars, "oval-h").unwrap_or(40.0),
        ),
        ShapeKind::Hex | ShapeKind::Cyl | ShapeKind::Diamond | ShapeKind::Cloud => (60.0, 60.0),
        _ => (0.0, 0.0),
    };
    Ok((default_w, default_h))
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

fn stroke_half(inst: &ResolvedInst, vars: &VarTable) -> f64 {
    let t = inst
        .attrs
        .number("thickness")
        .or_else(|| layout_var(vars, "thickness"))
        .unwrap_or(1.0);
    t / 2.0
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
        Some(_) => Err(Error::at(
            span,
            format!("'{}' expects a list of (x,y) tuples", name),
        )),
        None => Ok(None),
    }
}

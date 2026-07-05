//! Per-primitive bbox computation ([SPEC 5–7]).
//!
//! A closed primitive sizes **border-box**: `width`/`height` each default `auto` =
//! content + `padding` on that axis; an empty one is `2 × padding`; an explicit
//! dimension is a **floor** — it grows to content + padding rather than clip or
//! spill. Strokes count toward the bbox (half each side). `|text|` sizes to its
//! glyphs (no padding), `|icon|` to `icon-size`, and the geometry primitives to
//! their `points`/`src`.

use super::ir::Bbox;
use super::text;
use super::values::{as_pair, expand_box_value};
use crate::error::Error;
use crate::resolve::{AttrMap, NodeKind, ResolvedInst, ResolvedValue};
use crate::span::Span;

#[derive(Default, Clone, Copy)]
pub struct PaddingBox {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

/// Bbox for a leaf primitive (no flow children). A closed primitive is empty here —
/// `2 × padding` (or its explicit dims); text / icon / geometry size to their
/// own content. `scale` is the node's own effective `scale:` [SPEC 15.1]: it
/// multiplies the **shape** — declared `width`/`height`, `points:`, the pen's
/// fold — never text, padding, or stroke.
pub fn leaf_bbox(inst: &ResolvedInst, scale: f64) -> Result<Bbox, Error> {
    match inst.kind {
        NodeKind::Block
        | NodeKind::Oval
        | NodeKind::Hex
        | NodeKind::Slant
        | NodeKind::Cyl
        | NodeKind::Diamond => closed_bbox(inst, Bbox::empty(), scale),
        NodeKind::Text => {
            let size = font_size(inst);
            let label = inst.label.as_deref().unwrap_or("");
            let ls = inst.attrs.number("letter-spacing").unwrap_or(0.0);
            let lsp = inst.attrs.number("line-spacing").unwrap_or(0.0);
            Ok(Bbox::centered(
                text::approx_width(label, size, ls),
                text::approx_height(label, size, lsp),
            ))
        }
        // A label-less icon: an empty content box (the labelled case sizes the
        // same square from its laid-out label child — see `icon_square_bbox`).
        NodeKind::Icon => icon_square_bbox(inst, Bbox::empty(), scale),
        NodeKind::Line => Ok(
            bounding_box(&scaled_points(require_points(inst, "line", 2)?, scale))
                .inflate(stroke_half(inst)),
        ),
        NodeKind::Poly => Ok(
            bounding_box(&scaled_points(require_points(inst, "poly", 3)?, scale))
                .inflate(stroke_half(inst)),
        ),
        NodeKind::Image => {
            let (w, h) = image_dims(inst)?;
            Ok(Bbox::centered(w * scale, h * scale))
        }
        // Native top-left coords [SPEC 7]: size to the parsed path extent. A raw
        // `path:` is not in the shape set `scale:` multiplies ([SPEC 15.10]).
        NodeKind::Path => {
            let Some(ResolvedValue::String(d)) = inst.attrs.get("path") else {
                return Err(Error::at(inst.span, "'|path|' requires 'path'"));
            };
            let pts = super::path_bbox::extent_points(d);
            if pts.is_empty() {
                return Ok(Bbox::empty());
            }
            Ok(bounding_box(&pts).inflate(stroke_half(inst)))
        }
        // The pen [SPEC 15.3]: geometry-sized, like |path| — the fold is the one
        // source of truth (layout_inst intercepts sketches to keep the folded
        // `d`; this arm serves any other caller the bbox alone).
        NodeKind::Sketch => {
            let folded = super::drawing::pen::fold(inst, scale)?;
            Ok(folded.geometry.inflate(stroke_half(inst)))
        }
    }
}

/// A closed primitive's bbox: each axis is `content + padding`, with an explicit
/// `width`/`height` as a **floor** — border-box (padding inside), and the box
/// grows past the declared size rather than clip or spill its content [SPEC 5].
/// Inflated by half the stroke so the outline counts toward the bbox. The
/// declared dims are drawing units × the node's own `scale:`; content (text,
/// laid-out children), padding, and stroke stay sheet-space [SPEC 15.1].
pub fn closed_bbox(inst: &ResolvedInst, content: Bbox, scale: f64) -> Result<Bbox, Error> {
    let pad = padding(&inst.attrs, inst.span)?;
    let w = floor_dim(
        inst.attrs.number("width").map(|v| v * scale),
        content.w(),
        pad.left + pad.right,
    );
    let h = floor_dim(
        inst.attrs.number("height").map(|v| v * scale),
        content.h(),
        pad.top + pad.bottom,
    );
    Ok(Bbox::centered(w, h).inflate(stroke_half(inst)))
}

fn scaled_points(mut points: Vec<(f64, f64)>, scale: f64) -> Vec<(f64, f64)> {
    if scale != 1.0 {
        for p in &mut points {
            *p = (p.0 * scale, p.1 * scale);
        }
    }
    points
}

/// An `|icon|`'s bbox: a **square** that grows uniformly with its label content
/// and `padding`, so the symbol scales up *with* the text and keeps its
/// proportion [SPEC 7]. The side is the larger of the declared size (the bundle's
/// `icon-size` 32) and the content + padding on either axis — no stroke inflate
/// (the symbol's stroke sits inside its 256 grid). `content` is the empty box for
/// a bare icon, or its laid-out label child's extent.
pub fn icon_square_bbox(inst: &ResolvedInst, content: Bbox, scale: f64) -> Result<Bbox, Error> {
    let pad = padding(&inst.attrs, inst.span)?;
    let declared = scale
        * inst
            .attrs
            .number("width")
            .unwrap_or(0.0)
            .max(inst.attrs.number("height").unwrap_or(0.0));
    let side = declared
        .max(content.w() + pad.left + pad.right)
        .max(content.h() + pad.top + pad.bottom);
    Ok(Bbox::centered(side, side))
}

/// One axis of a closed primitive, **border-box**: `content + padding`, with an
/// explicit dimension as a **floor** over it — the box grows to fit content
/// rather than clip or spill. An **empty** box (no content on this axis) keeps
/// its declared size, since there is nothing to protect; an *auto* empty box is
/// `2 × padding` [SPEC 5].
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
    // A container's gap is non-negative, like CSS — overlap is `pin`'s job, not
    // a spacing value's. (A **mate**'s `gap:` may go negative — that one is the
    // drawing engine's, read off the link, never through here [SPEC 15.5].)
    if gy < 0.0 || gx < 0.0 {
        return Err(Error::at(span, "a container's 'gap' must be ≥ 0"));
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

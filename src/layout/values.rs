//! Extract layout-time numeric values from `ResolvedValue`s. A `--name`
//! reference is a visual var [SPEC 10.2], never a layout number, so it errors
//! where a number is required.

use crate::error::Error;
use crate::resolve::ResolvedValue;
use crate::span::Span;

pub fn as_number(value: &ResolvedValue, span: Span) -> Result<f64, Error> {
    match value {
        ResolvedValue::Number(n) => Ok(*n),
        ResolvedValue::LiveVar { name, .. } => Err(Error::at(
            span,
            format!(
                "var(--lini-{}) is a visual variable; layout attrs require a number",
                name
            ),
        )),
        other => Err(Error::at(
            span,
            format!("expected a number, got {}", describe(other)),
        )),
    }
}

pub fn as_number_tuple(value: &ResolvedValue, span: Span) -> Result<Vec<f64>, Error> {
    match value {
        ResolvedValue::Number(n) => Ok(vec![*n]),
        ResolvedValue::Tuple(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(as_number(item, span)?);
            }
            Ok(out)
        }
        ResolvedValue::LiveVar { name, .. } => Err(Error::at(
            span,
            format!(
                "var(--lini-{}) is a visual variable; layout attrs require a number or tuple",
                name
            ),
        )),
        other => Err(Error::at(
            span,
            format!("expected a number or tuple, got {}", describe(other)),
        )),
    }
}

pub fn as_pair(value: &ResolvedValue, span: Span) -> Result<(f64, f64), Error> {
    let nums = as_number_tuple(value, span)?;
    if nums.len() != 2 {
        return Err(Error::at(
            span,
            format!("expected a 2-tuple, got {} value(s)", nums.len()),
        ));
    }
    Ok((nums[0], nums[1]))
}

/// Expand a padding/gap/radius value into (top, right, bottom, left).
/// - `N`        → all four sides
/// - `(y, x)`   → (y, x, y, x)
/// - `(t, r, b, l)` → as written
pub fn expand_box_value(value: &ResolvedValue, span: Span) -> Result<(f64, f64, f64, f64), Error> {
    let nums = as_number_tuple(value, span)?;
    match nums.len() {
        1 => Ok((nums[0], nums[0], nums[0], nums[0])),
        2 => Ok((nums[0], nums[1], nums[0], nums[1])),
        4 => Ok((nums[0], nums[1], nums[2], nums[3])),
        n => Err(Error::at(
            span,
            format!("expected 1, 2, or 4 values, got {}", n),
        )),
    }
}

/// Multiply a `points:` attr's coordinates by the node's own `scale:`
/// [SPEC 15.1] — the render draws them off the placed node, so they must carry
/// the same factor sizing applied. Non-numbers pass through (they error in
/// sizing first).
pub(super) fn scale_points_attr(attrs: &mut crate::resolve::AttrMap, scale: f64) {
    let Some(v) = attrs.get("points") else {
        return;
    };
    fn scaled(v: &ResolvedValue, s: f64) -> ResolvedValue {
        match v {
            ResolvedValue::Number(n) => ResolvedValue::Number(n * s),
            ResolvedValue::Tuple(items) => {
                ResolvedValue::Tuple(items.iter().map(|i| scaled(i, s)).collect())
            }
            ResolvedValue::List(items) => {
                ResolvedValue::List(items.iter().map(|i| scaled(i, s)).collect())
            }
            other => other.clone(),
        }
    }
    let v = scaled(v, scale);
    attrs.insert("points", v);
}

fn describe(v: &ResolvedValue) -> &'static str {
    match v {
        ResolvedValue::Number(_) => "number",
        ResolvedValue::Percent(_) => "percentage",
        ResolvedValue::String(_) => "string",
        ResolvedValue::Hex(_) => "hex color",
        ResolvedValue::Ident(_) => "identifier",
        ResolvedValue::RawCss(_) => "CSS value",
        ResolvedValue::Tuple(_) => "tuple",
        ResolvedValue::List(_) => "list",
        ResolvedValue::Call(_) => "function call",
        ResolvedValue::LiveVar { .. } => "var() reference",
        ResolvedValue::Deferred(_) => "deferred fn: expression",
        ResolvedValue::PenCall { .. } | ResolvedValue::PenSegment(_) => "draw: pen item",
    }
}

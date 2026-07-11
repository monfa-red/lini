//! `|band|` and `|mark|` annotations, bound to an axis and placed by value.

use super::*;

/// Parse a `|band|` [SPEC 14.5]: its bound axis (default the x/domain axis), its
/// `span`, label, and fill (a real fill shades; `none` / unset makes it a divider).
pub(super) fn read_band(
    inst: &ResolvedInst,
    x_id: Option<&str>,
    specs: &[AxisSpec],
) -> Result<Band, Error> {
    let axis = match axis_id(inst) {
        Some(id) => lookup_axis(id, x_id, specs, inst.span)?,
        None => AxisRef::X,
    };
    let fill = real_color(inst.attrs.get("fill"));
    let tick = fill.clone().unwrap_or_else(muted);
    Ok(Band {
        axis,
        span: read_span(inst)?,
        label: label_of(inst),
        fill,
        tick,
    })
}

/// Parse a `|mark|` [SPEC 14.5]: a required bound axis, its `at` placement, the
/// label, whether a point shows its dot, and the accent (`stroke` / `fill`, else muted).
pub(super) fn read_mark(
    inst: &ResolvedInst,
    x_id: Option<&str>,
    specs: &[AxisSpec],
    chart_tip: Tooltip,
) -> Result<Mark, Error> {
    let axis = match axis_id(inst) {
        Some(id) => lookup_axis(id, x_id, specs, inst.span)?,
        None => return Err(Error::at(inst.span, "a '|mark|' needs 'axis:' to place it")),
    };
    let color = real_color(inst.attrs.get("stroke"))
        .or_else(|| real_color(inst.attrs.get("fill")))
        .unwrap_or_else(muted);
    Ok(Mark {
        axis,
        at: read_at(inst)?,
        label: label_of(inst),
        marker: chart_marker(inst)?,
        color,
        stroke_style: inst.attrs.get("stroke-style").cloned(),
        tooltip: super::tooltip::read_or(&inst.attrs, chart_tip)?,
    })
}

/// A `|band|`'s `span: a b` — its data range on the bound axis [SPEC 14.5].
fn read_span(inst: &ResolvedInst) -> Result<(f64, f64), Error> {
    match inst.attrs.get("span") {
        Some(ResolvedValue::Tuple(items)) if items.len() == 2 => {
            Ok((number(&items[0], inst.span)?, number(&items[1], inst.span)?))
        }
        _ => Err(Error::at(inst.span, "a '|band|' needs 'span: a b'")),
    }
}

/// A `|mark|`'s `at:` — one value (a reference line) or two (a point) [SPEC 14.5].
pub(super) fn read_at(inst: &ResolvedInst) -> Result<MarkAt, Error> {
    match inst.attrs.get("at") {
        Some(ResolvedValue::Number(v)) => Ok(MarkAt::Line(*v)),
        Some(ResolvedValue::Tuple(items)) if items.len() == 2 => Ok(MarkAt::Point(
            number(&items[0], inst.span)?,
            number(&items[1], inst.span)?,
        )),
        _ => Err(Error::at(
            inst.span,
            "'at' takes one value (a line) or two (a point)",
        )),
    }
}

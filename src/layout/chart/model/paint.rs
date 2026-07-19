//! Colour / outline resolution [SPEC 14.6] and the small numeric readers.

use super::*;

/// A series' legend label, harvested from the smart label [SPEC 14.6]: a
/// geometry series (`|line|`) keeps it on the node; a block series (`|bars|` /
/// `|dots|`) lowered it to a centred text child.
pub(crate) fn label_of(inst: &ResolvedInst) -> Option<String> {
    inst.label.clone().filter(|t| !t.is_empty()).or_else(|| {
        inst.children
            .iter()
            .find(|c| c.kind == NodeKind::Text)
            .and_then(|c| c.label.as_deref())
            .filter(|t| !t.is_empty())
            .map(str::to_string)
    })
}

/// The explicit **fill** of a fill shape [SPEC 14.6] — `fill:` only, never the
/// stroke (a stroke is a separate outline, read by [`outline`]). The inherited primitive
/// defaults — `none`, or the bare `--fill` role var a `|block|` carries — are **not** a
/// choice, so they fall through to the palette walk.
pub(crate) fn fill_color(attrs: &AttrMap) -> Option<ResolvedValue> {
    real_color(attrs.get("fill"))
}

/// A fill shape's explicit `stroke:` outline [SPEC 14.6]: its colour paired with
/// `stroke-width` (default 1.5), or `None` for no outline. Used by `|area|` and `|bubble|`
/// (explicit-only); `|bars|` / `|slice|` use [`fill_outline`] for the default deep edge.
pub(super) fn outline(attrs: &AttrMap) -> Option<(ResolvedValue, f64)> {
    real_color(attrs.get("stroke")).map(|c| (c, attrs.number("stroke-width").unwrap_or(1.5)))
}

/// A fill *series'* outline — the outlined look [SPEC 14.6]. An explicit `stroke:`
/// colour wins; the class default `stroke: auto` (or a bare role var / unset) draws a
/// **deep** edge of the `fill`; `stroke: none` removes it. The `auto` sentinel on the
/// `.lini-bars` / `.lini-slice` class is what separates an unset stroke (→ a default edge)
/// from an explicit `none` (→ no edge). Shared by `|bars|` (here) and `|slice|`
/// ([`build_pie`]), so the default edge derives in one place.
pub(crate) fn fill_outline(attrs: &AttrMap, fill: &ResolvedValue) -> Option<(ResolvedValue, f64)> {
    edge_from(
        attrs.get("stroke"),
        fill,
        attrs.number("stroke-width").unwrap_or(1.5),
    )
}

/// The outlined-look edge [SPEC 14.6] as one table: `stroke: none` → no edge;
/// `stroke: auto` (or a bare role var / unset) → a **deep** edge of the `fill`; an
/// explicit colour → that colour at `width`. The single-value [`fill_outline`] passes
/// the node's stroke; [`paint_lists`] passes each datum's stroke and own fill — the
/// edge derives here in one place.
fn edge_from(
    stroke: Option<&ResolvedValue>,
    fill: &ResolvedValue,
    width: f64,
) -> Option<(ResolvedValue, f64)> {
    match stroke {
        Some(ResolvedValue::Ident(s)) if s == "none" => None,
        Some(ResolvedValue::Ident(s)) if s == "auto" => Some((palette::deepen(fill), width)),
        other => match real_color(other) {
            Some(c) => Some((c, width)),
            None => Some((palette::deepen(fill), width)),
        },
    }
}

pub(super) fn real_color(v: Option<&ResolvedValue>) -> Option<ResolvedValue> {
    match v {
        Some(ResolvedValue::Ident(s)) if s == "none" => None,
        Some(ResolvedValue::LiveVar { name, .. }) if name == "stroke" || name == "fill" => None,
        Some(other) => Some(other.clone()),
        None => None,
    }
}

pub(super) fn clone_grid(g: &Grid) -> Grid {
    match g {
        Grid::Default => Grid::Default,
        Grid::Off => Grid::Off,
        Grid::Color(c) => Grid::Color(c.clone()),
    }
}

pub(super) fn numbers(items: &[ResolvedValue], span: Span) -> Result<Vec<f64>, Error> {
    items.iter().map(|it| number(it, span)).collect()
}

pub(super) fn number(v: &ResolvedValue, span: Span) -> Result<f64, Error> {
    v.as_number()
        .ok_or_else(|| Error::at(span, "'data' values must be numbers"))
}

/// Per-datum paint lists [SPEC 14.6] on a repeated-mark series: comma-listed
/// `fill:` / `stroke:` / `opacity:`, one item per authored datum. `auto` is
/// the paint the datum gets anyway — the series' derived fill, the deep edge
/// of that datum's own fill, the bar mode's opacity. The one-shape series
/// (`|line|` / `|area|`) were rejected at validation; counts must match.
pub(super) fn paint_lists(
    inst: &ResolvedInst,
    kind: &SeriesKind,
    base: &ResolvedValue,
    data: &Data,
) -> Result<PerDatum, Error> {
    let mut out = PerDatum::default();
    let get = |k| match inst.attrs.get(k) {
        Some(ResolvedValue::List(items)) => Some(items.as_slice()),
        _ => None,
    };
    let (fill, stroke, opacity) = (get("fill"), get("stroke"), get("opacity"));
    if fill.is_none() && stroke.is_none() && opacity.is_none() {
        return Ok(out);
    }
    if !matches!(kind, SeriesKind::Bars | SeriesKind::Dots) {
        // The validator's static arm says the same at lint; this is the
        // semantic authority, so a library compile can't slip past it.
        let shown = if matches!(kind, SeriesKind::Area) {
            "area"
        } else {
            "line"
        };
        return Err(Error::at(inst.span, format::one_shape_paint(shown)));
    }
    let count = match data {
        Data::Categorical(v) => v.len(),
        Data::Points(p) => p.len(),
        Data::Formula(_) => {
            return Err(Error::at(
                inst.span,
                "a per-datum paint list needs explicit 'data' — a sampled 'fn' \
                 has no authored data points",
            ));
        }
    };
    let counted = |name: &str, items: &[ResolvedValue]| -> Result<(), Error> {
        if items.len() == count {
            Ok(())
        } else {
            Err(Error::at(
                inst.span,
                format!(
                    "'{name}' lists {} paints but the series has {count} data points",
                    items.len()
                ),
            ))
        }
    };
    if let Some(items) = fill {
        counted("fill", items)?;
        out.fills = Some(
            items
                .iter()
                .map(|it| match it {
                    ResolvedValue::Ident(s) if s == "auto" => base.clone(),
                    other => other.clone(),
                })
                .collect(),
        );
    }
    let stroke_default = match inst.attrs.get("stroke") {
        None => true,
        Some(ResolvedValue::Ident(s)) => s != "none",
        _ => false,
    };
    if stroke.is_none()
        && stroke_default
        && let Some(fills) = &out.fills
    {
        // No stroke authored: the default deep edge deepens each datum's own
        // fill — the single-value "explicit fill still gains a deep edge of
        // it" law, per datum.
        let width = inst.attrs.number("stroke-width").unwrap_or(1.5);
        out.outlines = Some(
            fills
                .iter()
                .map(|f| Some((palette::deepen(f), width)))
                .collect(),
        );
    }
    if let Some(items) = stroke {
        counted("stroke", items)?;
        let width = inst.attrs.number("stroke-width").unwrap_or(1.5);
        let fill_at = |i: usize| -> ResolvedValue {
            out.fills
                .as_ref()
                .and_then(|v| v.get(i))
                .cloned()
                .unwrap_or_else(|| base.clone())
        };
        out.outlines = Some(
            items
                .iter()
                .enumerate()
                .map(|(i, it)| edge_from(Some(it), &fill_at(i), width))
                .collect(),
        );
    }
    if let Some(items) = opacity {
        counted("opacity", items)?;
        out.opacities = Some(
            items
                .iter()
                .map(|it| match it {
                    ResolvedValue::Ident(s) if s == "auto" => Ok(None),
                    ResolvedValue::Number(n) if (0.0..=1.0).contains(n) => Ok(Some(*n)),
                    _ => Err(Error::at(inst.span, "'opacity' is a fraction 0..1")),
                })
                .collect::<Result<_, Error>>()?,
        );
    }
    Ok(out)
}

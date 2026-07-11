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
    let width = attrs.number("stroke-width").unwrap_or(1.5);
    match attrs.get("stroke") {
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

pub(crate) fn live(name: &str) -> ResolvedValue {
    ResolvedValue::LiveVar {
        name: name.to_string(),
        raw: false,
    }
}

/// The muted role tint — a band tick / mark accent's default when unpainted.
pub(super) fn muted() -> ResolvedValue {
    live("muted")
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

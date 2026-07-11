//! `max-width` — wrap to fit [SPEC 5]. A box's `max-width:` caps its auto
//! width; `text-wrap: wrap | nowrap` (default `wrap`) says whether the text
//! inside breaks into lines to honour it. Wrapping rewrites a text leaf's
//! label with the [`text::wrap`] line breaks and re-measures it, so the
//! wrapped size **is** the measured size — it feeds auto-sizing, grid tracks,
//! gutters, and routing obstacles with no further plumbing. Only text wraps:
//! a non-text in-flow child wider than the cap is an error, as is a `width:`
//! floor above it and `nowrap` text that cannot fit ([SPEC 20]).

use super::ir::{Bbox, PlacedNode};
use super::{anchors, primitives, text};
use crate::error::Error;
use crate::resolve::{NodeKind, ResolvedInst, ResolvedValue};
use crate::span::Span;

/// Apply a container's `max-width` to its laid-out children, before they are
/// arranged. `own` is the container's effective scale — the cap is authored
/// in the same units as `width:`.
pub(super) fn apply_max_width(
    inst: &ResolvedInst,
    children: &mut [PlacedNode],
    own: f64,
    span: Span,
) -> Result<(), Error> {
    let Some(max_w) = inst.attrs.number("max-width") else {
        return Ok(());
    };
    // A `width:` floor above the cap is a contradiction [SPEC 20].
    if let Some(w) = inst.attrs.number("width")
        && w > max_w
    {
        return Err(Error::at(
            span,
            format!("'width: {w}' exceeds 'max-width: {max_w}'"),
        ));
    }
    let pad = primitives::padding(&inst.attrs, span)?;
    let stroke = inst.attrs.number("stroke-width").unwrap_or(0.0);
    // The content-area cap: the bbox cap is border-box (padding and stroke
    // inside it, per the core law [SPEC 5]).
    let avail = (max_w * own) - pad.left - pad.right - stroke;
    let nowrap = matches!(
        inst.attrs.get("text-wrap"),
        Some(ResolvedValue::Ident(s)) if s == "nowrap"
    );

    for child in children.iter_mut() {
        // Pinned overlays never grow the parent — the cap has no claim on them.
        if anchors::is_pinned(&child.attrs) {
            continue;
        }
        if child.bbox.w() <= avail + 1e-9 {
            continue;
        }
        if child.kind != NodeKind::Text {
            return Err(Error::at(
                child.span,
                format!("a child is wider than 'max-width: {max_w}' — only text wraps"),
            ));
        }
        if nowrap {
            return Err(Error::at(
                child.span,
                format!(
                    "text cannot fit 'max-width: {max_w}' without wrapping — widen it or drop 'text-wrap: nowrap'"
                ),
            ));
        }
        let Some(label) = child.label.clone() else {
            continue;
        };
        let size = child.attrs.number("font-size").unwrap_or(15.0);
        let ls = child.attrs.number("letter-spacing").unwrap_or(0.0);
        let lsp = child.attrs.number("line-spacing").unwrap_or(0.0);
        let wrapped = text::wrap(&label, size, ls, avail).join("\n");
        child.bbox = Bbox::centered(
            text::approx_width(&wrapped, size, ls),
            text::approx_height(&wrapped, size, lsp),
        );
        child.label = Some(wrapped);
    }
    Ok(())
}

//! The drawing layout family [SPEC 15]. The **sketch pen** (`pen`, `geometry`)
//! folds `draw:` profiles in any layout; the **engine** (`engine`, `mates`,
//! `anchors`, `chrome`) is the `layout: drawing` scope itself — datum
//! placement, features, mates, the generated chrome. Annotations (dimensions,
//! leaders) land per PLAN.md stage 4.

pub(crate) mod anchors;
pub(crate) mod chrome;
mod engine;
pub(crate) mod geometry;
mod mates;
pub(crate) mod pen;

pub(super) use engine::{layout_node, layout_root};

use super::ir::Bbox;
use crate::error::Error;
use crate::resolve::{AttrMap, Program, ResolvedInst, ResolvedValue};

/// `layout: drawing` [SPEC 15] — the drawing engine's dispatch check, the
/// `is_sequence` twin.
pub(crate) fn is_drawing(attrs: &AttrMap) -> bool {
    matches!(attrs.get("layout"), Some(ResolvedValue::Ident(l)) if l == "drawing")
}

/// Whether the container at `scope` is a `layout: drawing` — its links are the
/// engine's (dimensions, leaders, mates), so the router and the declared-edge
/// count skip them, exactly as a sequence scope's messages are skipped.
pub(crate) fn is_drawing_scope(program: &Program, scope: &str) -> bool {
    super::scope_attrs(program, scope).is_some_and(is_drawing)
}

/// Sheet content [SPEC 15]: placed and styled per its own type, never a part —
/// text, notes, balloons, the title footnote (tables seal via `layout: grid`).
pub(super) fn is_sheet(kind: crate::resolve::NodeKind, type_chain: &[String]) -> bool {
    kind == crate::resolve::NodeKind::Text
        || type_chain.iter().any(|t| {
            matches!(
                t.as_str(),
                "note" | "balloon" | "table" | "footnote" | "caption"
            )
        })
}

/// A part's own bbox in a drawing scope [SPEC 15.4]: `|hole|` / `|pitch-circle|`
/// are round — `width:` (required) is the diameter — and every other shape
/// sizes as a leaf: a part's features never grow it, they overhang.
pub(super) fn part_bbox(inst: &ResolvedInst, own: f64) -> Result<Bbox, Error> {
    if let Some(ty) = inst
        .type_chain
        .iter()
        .find(|t| *t == "hole" || *t == "pitch-circle")
        && !chrome::is_chrome(&inst.attrs)
    {
        let Some(w) = inst.attrs.number("width") else {
            return Err(Error::at(
                inst.span,
                format!("'|{ty}|' requires 'width' — its diameter"),
            ));
        };
        let sw = inst.attrs.number("stroke-width").unwrap_or(0.0);
        return Ok(Bbox::centered(w * own, w * own).inflate(sw / 2.0));
    }
    super::primitives::leaf_bbox(inst, own)
}

/// Datum placement [SPEC 15.1/15.4]: every child's **origin** lands on the
/// parent's datum, offset only by `translate:` (drawing units × the parent's
/// scale). Chrome children stay at the datum (their geometry is filled by
/// [`chrome::fill`]); pinned sheet content is re-placed by the engine after
/// the extent is known.
pub(super) fn place_features(children: &mut [super::PlacedNode], scale: f64) -> Result<(), Error> {
    for c in children.iter_mut() {
        if chrome::is_chrome(&c.attrs) {
            continue;
        }
        let (dx, dy) = super::anchors::translate(&c.attrs, c.span)?.unwrap_or((0.0, 0.0));
        c.cx = dx * scale;
        c.cy = dy * scale;
    }
    Ok(())
}

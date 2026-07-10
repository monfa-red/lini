//! The core `|note|` silhouette [SPEC 8]: a body with its top-right corner
//! clipped plus the folded flap (the dog-ear) in the note's stroke colour — so
//! a note reads as an annotation in **every** layout. Applied once, by the
//! generic arranger; the sequence and drawing engines only *place* the card.

use super::ir::PlacedNode;
use super::prim;
use crate::ledger::consts::{NOTE_FOLD_FRAC, NOTE_FOLD_MAX};
use crate::resolve::{NodeKind, ResolvedValue};

/// Reshape a laid-out note box: the silhouette changes from a `Block` rect to a
/// `Path`; the box's resolved `fill` / `stroke` (carried on its `<g>`) and its
/// text child stay.
pub(crate) fn fold(note: &mut PlacedNode) {
    let (w, h) = (note.bbox.w(), note.bbox.h());
    let fold = (h * NOTE_FOLD_FRAC).min(NOTE_FOLD_MAX).min(w * 0.5);
    let (l, t, rg, b) = (-w / 2.0, -h / 2.0, w / 2.0, h / 2.0);
    let body = format!(
        "M {l} {t} L {} {t} L {rg} {} L {rg} {b} L {l} {b} Z",
        rg - fold,
        t + fold
    );
    note.kind = NodeKind::Path;
    note.attrs.insert("path", ResolvedValue::String(body));
    let stroke = note
        .attrs
        .get("stroke")
        .cloned()
        .unwrap_or_else(|| ResolvedValue::LiveVar {
            name: "stroke".to_string(),
            raw: false,
        });
    let flap = format!(
        "M {} {t} L {rg} {} L {} {} Z",
        rg - fold,
        t + fold,
        rg - fold,
        t + fold
    );
    note.children.insert(
        0,
        prim::path(
            flap,
            stroke,
            super::ir::Bbox {
                min_x: rg - fold,
                min_y: t,
                max_x: rg,
                max_y: t + fold,
            },
        ),
    );
}

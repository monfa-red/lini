//! Notes [SPEC 13]: a `|note|` callout placed at its time row (source order), bound to
//! lifelines by **`place:`** — a mode then its lifelines: `place: over api db`
//! (one lifeline, or a span of them), `place: left api`, `place: right api`.
//! The box itself is laid out by the generic engine (so its text, padding, and
//! styling are reused — no parallel layout); this module only reads the placement and
//! fixes the box's centre over the right lifelines at its row.

use crate::resolve::{AttrMap, ResolvedValue};
use std::collections::HashMap;

/// Clear space between a `left` / `right` note and the lifeline it sits beside.
const SIDE_GAP: f64 = 12.0;

/// Where a note binds to the lifelines [SPEC 13]. `over` may name several — the box
/// centres over their span.
pub(super) enum Placement {
    Over(Vec<String>),
    Left(String),
    Right(String),
}

/// The note's placement, read from its `place:` attr (the box carries it
/// through resolve): a mode ident then its lifeline id(s). `Ok(None)` when
/// absent — caught as the missing-placement error upstream; a malformed value
/// errors here [SPEC 20].
pub(super) fn placement(
    attrs: &AttrMap,
    span: crate::span::Span,
) -> Result<Option<Placement>, crate::error::Error> {
    let Some(v) = attrs.get("place") else {
        return Ok(None);
    };
    let bad = || {
        crate::error::Error::at(
            span,
            "'place' is a mode then its lifelines — 'place: over api db', 'place: left api'",
        )
    };
    let parts = idents(v);
    let (mode, ids) = parts.split_first().ok_or_else(bad)?;
    match (mode.as_str(), ids) {
        ("over", [_, ..]) => Ok(Some(Placement::Over(ids.to_vec()))),
        ("left", [id]) => Ok(Some(Placement::Left(id.clone()))),
        ("right", [id]) => Ok(Some(Placement::Right(id.clone()))),
        _ => Err(bad()),
    }
}

/// The note box's centre x for its placement: over the midpoint of the named lifelines, or
/// beside one. `None` if a named participant has no lifeline (an unknown id).
pub(super) fn centre_x(
    placement: &Placement,
    box_w: f64,
    lifeline_x: &HashMap<String, f64>,
) -> Option<f64> {
    match placement {
        Placement::Over(ids) => {
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for id in ids {
                let x = *lifeline_x.get(id)?;
                lo = lo.min(x);
                hi = hi.max(x);
            }
            (lo <= hi).then_some((lo + hi) / 2.0)
        }
        Placement::Left(id) => lifeline_x.get(id).map(|x| x - box_w / 2.0 - SIDE_GAP),
        Placement::Right(id) => lifeline_x.get(id).map(|x| x + box_w / 2.0 + SIDE_GAP),
    }
}

/// The idents in a value — a single `Ident`, or each of a space / comma group.
fn idents(v: &ResolvedValue) -> Vec<String> {
    match v {
        ResolvedValue::Ident(s) => vec![s.clone()],
        ResolvedValue::Tuple(xs) | ResolvedValue::List(xs) => {
            xs.iter().filter_map(one_ident).collect()
        }
        _ => Vec::new(),
    }
}

fn one_ident(v: &ResolvedValue) -> Option<String> {
    match v {
        ResolvedValue::Ident(s) => Some(s.clone()),
        _ => None,
    }
}

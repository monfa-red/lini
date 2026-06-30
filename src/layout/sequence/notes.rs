//! Notes (SPEC §10): a `|note|` callout placed at its time row (source order), bound to
//! lifelines by **placement** — `over` (one lifeline, or a span of them), `left`, or
//! `right`. The box itself is laid out by the generic engine (so its text, padding, and
//! styling are reused — no parallel layout); this module only reads the placement and
//! fixes the box's centre over the right lifelines at its row.

use crate::resolve::{AttrMap, ResolvedValue};
use std::collections::HashMap;

/// Clear space between a `left` / `right` note and the lifeline it sits beside.
const SIDE_GAP: f64 = 12.0;

/// Where a note binds to the lifelines (SPEC §10). `over` may name several — the box
/// centres over their span.
pub(super) enum Placement {
    Over(Vec<String>),
    Left(String),
    Right(String),
}

/// The note's placement, read from its `over` / `left` / `right` attr (the box carries
/// them through resolve). `None` if it names none — caught as an error upstream.
pub(super) fn placement(attrs: &AttrMap) -> Option<Placement> {
    if let Some(v) = attrs.get("over") {
        return Some(Placement::Over(idents(v)));
    }
    if let Some(v) = attrs.get("left") {
        return first_ident(v).map(Placement::Left);
    }
    if let Some(v) = attrs.get("right") {
        return first_ident(v).map(Placement::Right);
    }
    None
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

fn first_ident(v: &ResolvedValue) -> Option<String> {
    idents(v).into_iter().next()
}

fn one_ident(v: &ResolvedValue) -> Option<String> {
    match v {
        ResolvedValue::Ident(s) => Some(s.clone()),
        _ => None,
    }
}

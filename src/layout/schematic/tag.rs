//! The **shaped tag** [SPEC 16.4]: a net label's pointed outline.
//!
//! `shape: left | right | both` draws a flag — a rectangle with one or both
//! ends drawn to a point. The point's depth is a baked constant but its span is
//! the tag's own box, which nothing knows until the text is measured, so
//! desugar emits the outline as a `|path|` chrome placeholder carrying its
//! `chrome: tag <shape>` marker and this fills it once the label is sized —
//! the same two-step every generated line takes
//! ([`crate::layout::drawing::chrome`], [`crate::layout::page`]).
//!
//! The shapes are **visual, not semantic** [SPEC 16.4]: the conventional
//! readings (output, input, bidirectional) are the reader's.

use super::super::ir::{Bbox, PlacedNode};
use crate::ledger::consts::TAG_POINT;
use crate::resolve::{NodeKind, ResolvedValue};

/// Fill a label's shaped-tag outline from its finished box (stroke excluded).
pub(in crate::layout) fn fill(children: &mut [PlacedNode], box_: Bbox) {
    for c in children.iter_mut() {
        let Some(ResolvedValue::Tuple(items)) = c.attrs.get("chrome") else {
            continue;
        };
        let [ResolvedValue::Ident(kind), ResolvedValue::Ident(shape)] = items.as_slice() else {
            continue;
        };
        if kind != "tag" {
            continue;
        }
        let half = super::super::drawing::half_stroke(&c.attrs);
        let (w, h) = (box_.w() - 2.0 * half, box_.h() - 2.0 * half);
        c.attrs
            .insert("path", ResolvedValue::String(outline(shape, w, h)));
        c.kind = NodeKind::Path;
        c.bbox = Bbox::centered(w, h).inflate(half);
    }
}

/// The flag outline in the tag's own centred frame — the point cut back from
/// the box's edge, so the drawn tip lands **on** it and the label's padding
/// already holds the whole shape.
///
/// The cut is **half the tag's height**, which draws the nose at exactly 45°
/// whatever the text measures — a fixed depth grows sharper as the tag gets
/// shorter, which is what read as a spike. [`TAG_POINT`] is what the class
/// rule reserved for it, so a tag taller than that blunts its nose rather
/// than letting the point eat the text.
fn outline(shape: &str, w: f64, h: f64) -> String {
    let (x, y) = (w / 2.0, h / 2.0);
    let point = y.min(TAG_POINT);
    let n = crate::layout::drawing::geometry::n;
    let (left, right) = (shape != "right", shape != "left");
    let mut d = String::new();
    // Clockwise from the top-left, cutting whichever ends carry a point.
    let mut go = |cmd: char, px: f64, py: f64| {
        d.push_str(&format!("{cmd} {} {} ", n(px), n(py)));
    };
    go('M', if left { -x + point } else { -x }, -y);
    go('L', if right { x - point } else { x }, -y);
    if right {
        go('L', x, 0.0);
    }
    go('L', if right { x - point } else { x }, y);
    go('L', if left { -x + point } else { -x }, y);
    if left {
        go('L', -x, 0.0);
    }
    d.push('Z');
    d
}

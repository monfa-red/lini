//! The pen's corner modifiers [SPEC 15.3]: `fillet(r)` / `chamfer(c)` park
//! between two segments, trim both legs, and drop in the joint — a tangent arc
//! or a straight bevel. The pen (`super::pen`) owns *when* a modifier applies
//! (incl. cyclically through `close()`); this module owns the trim itself.

use super::Segment;
use super::geometry::{self, PathSeg, dist};
use crate::error::Error;
use crate::span::Span;

/// A pending corner modifier — parked between its two segments.
#[derive(Clone, Copy)]
pub(super) enum Mod {
    Fillet(f64),
    Chamfer(f64),
}

impl Mod {
    pub(super) fn word(self) -> &'static str {
        match self {
            Mod::Fillet(_) => "fillet",
            Mod::Chamfer(_) => "chamfer",
        }
    }
}

/// Trim the corner between two straight runs and drop in the modifier's joint:
/// a tangent arc (`fillet`) or a straight bevel (`chamfer`, cut `c` back along
/// each leg) [SPEC 15.3]. Returns (trimmed prev, joint, trimmed next, the
/// joint's `:segment`).
pub(super) fn apply_mod(
    m: Mod,
    prev: PathSeg,
    next: PathSeg,
    span: Span,
) -> Result<(PathSeg, PathSeg, PathSeg, Segment), Error> {
    let (PathSeg::Line { from: a, to: c1 }, PathSeg::Line { from: c2, to: b }) = (prev, next)
    else {
        return Err(Error::at(
            span,
            format!("'{}' joins two straight segments today", m.word()),
        ));
    };
    debug_assert!(dist(c1, c2) < 1e-9, "corner segments meet at one point");
    let c = c1;
    let (la, lb) = (dist(a, c), dist(c, b));
    let da = ((c.0 - a.0) / la, (c.1 - a.1) / la);
    let db = ((b.0 - c.0) / lb, (b.1 - c.1) / lb);
    let cross = da.0 * db.1 - da.1 * db.0;
    if cross.abs() < 1e-9 {
        return Err(Error::at(
            span,
            format!("'{}' needs a turn between its two runs", m.word()),
        ));
    }
    let interior = (-(da.0 * db.0 + da.1 * db.1)).clamp(-1.0, 1.0).acos();
    let t = match m {
        Mod::Fillet(r) => r / (interior / 2.0).tan(),
        Mod::Chamfer(cc) => cc,
    };
    let amount = match m {
        Mod::Fillet(r) => r,
        Mod::Chamfer(cc) => cc,
    };
    if amount <= 0.0 || t > la - 1e-9 || t > lb - 1e-9 {
        return Err(Error::at(
            span,
            format!(
                "{} {} does not fit its corner",
                m.word(),
                geometry::n(amount)
            ),
        ));
    }
    let ta = (c.0 - da.0 * t, c.1 - da.1 * t);
    let tb = (c.0 + db.0 * t, c.1 + db.1 * t);
    let (mid, segment) = match m {
        Mod::Fillet(r) => {
            let sweep = cross > 0.0;
            // Centre: perpendicular off the incoming leg at the tangent point.
            let centre = if sweep {
                (ta.0 - da.1 * r, ta.1 + da.0 * r)
            } else {
                (ta.0 + da.1 * r, ta.1 - da.0 * r)
            };
            let clen = dist(centre, c);
            let on_arc = (
                centre.0 + (c.0 - centre.0) / clen * r,
                centre.1 + (c.1 - centre.1) / clen * r,
            );
            (
                PathSeg::Arc {
                    from: ta,
                    to: tb,
                    r,
                    large: false,
                    sweep,
                },
                Segment::Arc { mid: on_arc, r },
            )
        }
        Mod::Chamfer(_) => (PathSeg::Line { from: ta, to: tb }, Segment::Edge(ta, tb)),
    };
    Ok((
        PathSeg::Line { from: a, to: ta },
        mid,
        PathSeg::Line { from: tb, to: b },
        segment,
    ))
}

//! The derived **face anchors** [SPEC 15.11]: every named segment of a wall's
//! centreline also answers as `name-in` / `name-out` — the segment's own
//! offset edges, the two faces the poché is drawn between. They are the
//! anchors a listing plan dimensions: a **clear room span** is face to face,
//! `outer:north-in (-) bathwall:head-out`, while the centreline `:segment`
//! keeps the structural read it always had.
//!
//! Nothing new is measured here. A face is [`wall::raw_offset`](super::wall)
//! — the very function the outline walk offsets each run with — applied to the
//! named edge, so a face anchor is the wall's own face by construction, at the
//! segment's **theoretical** corners exactly as the centreline segment is
//! recorded there. The whole segment's face is one anchor: an opening cuts the
//! drawn outline, never the face a dimension names, so a wall reads one span
//! whether or not a window sits in it.
//!
//! Which face is `-in`: on a **closed** run the enclosed side — the side the
//! run's own winding puts its interior on; on an **open** one the left of the
//! pen's travel, the named-edge convention [SPEC 15.5].

use super::super::drawing::Segment;
use super::super::drawing::geometry::{PathSeg, SEAM_EPS, Subpath, arc_center, dist};
use super::super::drawing::pen::Folded;
use super::opening;
use crate::error::{Code, Error};
use crate::layout::geom::unit;
use crate::math;
use crate::span::Span;

/// Add every named segment's two face anchors to the wall's segment table —
/// read against the **centreline**, so this runs before the outline replaces
/// it. The reserved-name law comes first: a derived name is the wall's, as a
/// built-in point name is the pen's [SPEC 15.2].
pub(super) fn derive(folded: &mut Folded, h: f64, span: Span) -> Result<(), Error> {
    reserved(folded, span)?;
    let mut faces: Vec<(String, Segment)> = Vec::new();
    for (name, seg) in &folded.segments {
        let Segment::Edge(a, b) = *seg else { continue };
        let len = dist(a, b);
        if len <= SEAM_EPS {
            continue;
        }
        let d = unit((b.0 - a.0, b.1 - a.1)).expect("a run with length has a direction");
        // Which run carries the named edge — the one place the pen's names and
        // the folded runs meet, shared with the openings' own station.
        let Some((sub, _, _)) = opening::locate(folded, a, d, len) else {
            continue;
        };
        let run = &folded.subs[sub];
        let in_is_left = !run.closed || signed_area(run) < 0.0;
        let edge = PathSeg::Line { from: a, to: b };
        for (suffix, left) in [("-in", in_is_left), ("-out", !in_is_left)] {
            // Every face is walked with the **material on its right** — so the
            // left offset keeps the pen's travel and the right one reverses,
            // exactly as the outline's own two side chains assemble. That is
            // what makes a face's outward point off the wall [SPEC 15.5], so a
            // mate seats against it and a dimension takes its axis across it.
            let face = super::wall::raw_offset(&edge, h, left);
            let face = if left { face } else { face.reverse() };
            faces.push((
                format!("{name}{suffix}"),
                Segment::Edge(face.from(), face.to()),
            ));
        }
    }
    folded.segments.extend(faces);
    Ok(())
}

/// A wall reserves the two derived names on every segment [SPEC 21] — the same
/// law the pen's built-in point names carry, so an author never writes a name
/// the wall is about to define underneath them.
fn reserved(folded: &Folded, span: Span) -> Result<(), Error> {
    for (name, _) in &folded.segments {
        if name.ends_with("-in") || name.ends_with("-out") {
            return Err(Error::at(
                span,
                format!("':{name}' collides with the derived face anchor — rename the segment"),
            )
            .code(Code::FACE_ANCHOR));
        }
    }
    Ok(())
}

/// A closed run's signed area, in the drawing's own screen frame (y grows
/// down): **positive is clockwise on screen**, which puts the enclosed side on
/// the right of the pen's travel. Each segment contributes its chord's
/// shoelace term; an arc adds the circular segment it bulges past that chord,
/// signed by the side it sweeps toward — so a run whose corners are all arcs
/// still answers.
fn signed_area(run: &Subpath) -> f64 {
    let mut sum = 0.0;
    for seg in &run.segs {
        let (p, q) = (seg.from(), seg.to());
        sum += p.0 * q.1 - q.0 * p.1;
        if let PathSeg::Arc {
            r, large, sweep, ..
        } = *seg
        {
            let c = arc_center(p, q, r, large, sweep);
            let (u, v) = ((p.0 - c.0, p.1 - c.1), (q.0 - c.0, q.1 - c.1));
            let swept = math::atan2((u.0 * v.1 - u.1 * v.0).abs(), u.0 * v.0 + u.1 * v.1);
            let swept = if large {
                std::f64::consts::TAU - swept
            } else {
                swept
            };
            let bulge = r * r * (swept - math::sin(swept));
            sum += if sweep { bulge } else { -bulge };
        }
    }
    sum / 2.0
}

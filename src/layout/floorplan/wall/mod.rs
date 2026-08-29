//! The wall offset [SPEC 15.11]: a `|wall|`'s `draw:` traces its
//! **centreline**; this module grows it into the closed poché **outline** at
//! ± thickness ∕ 2 — mitred corners (an acute spike bevels at miter limit 4),
//! concentric arc offsets, butt caps on open ends, a `close()` seam mitred
//! like any corner — and the outline replaces the drawn path for paint and
//! for the geometry bbox ([SPEC 15.10] step 1: after the `draw:` fold, before
//! the bboxes). The authored `:segment`s stay **centreline** stations, so
//! dimensions measure where architects do while bbox anchors and leader
//! ray-casts read the outline.
//!
//! The walk is **per contiguous centreline run** (one pen subpath): each run
//! [cuts](cut) at its openings' stations, and every piece offsets to two side
//! chains joined corner by corner ([`join`]) which the assembly closes into
//! loops. Cutting the **run** rather than the two chains is what makes a jamb
//! cap and an open end's butt cap one construction [SPEC 15.11].

mod join;

use super::super::drawing::geometry::{
    self, P, PathSeg, SEAM_EPS, Subpath, arc_center, dist, n, to_d,
};
use super::super::drawing::pen::Folded;
use super::opening;
use crate::error::{Code, Error};
use crate::layout::geom::unit;
use crate::resolve::ResolvedInst;
use crate::span::Span;
use join::join;

/// Grow a folded wall centreline into its outline, in place: the subpaths,
/// the `d`, and the geometry bbox become the outline's; segments, mirrors,
/// and the view map stay the centreline's [SPEC 15.11].
pub(in crate::layout) fn offset(
    folded: &mut Folded,
    inst: &ResolvedInst,
    children: &mut [crate::layout::PlacedNode],
    own: f64,
) -> Result<(), Error> {
    // Nearest-wins [SPEC 15.11]: a cascaded `thickness:` on the wall itself
    // (authored or rule-borne), else the desugar-stamped fallback — the
    // partition define / scope value / 200 mm default, already in drawing
    // units. The raw-mm constant only guards a fold desugar never saw.
    let units = inst
        .attrs
        .number("thickness")
        .or_else(|| inst.attrs.number(crate::desugar::scale::WALL_THICKNESS))
        .unwrap_or(crate::desugar::scale::WALL_MM);
    let h = units * own / 2.0;
    // The openings resolve against the **folded** centreline — the one child
    // that reads down from its part [SPEC 15.11] — and their stations are what
    // the offset cuts around.
    let stations = opening::plan(inst, folded, own)?;
    let mut outline = Vec::new();
    for (i, sub) in folded.subs.iter().enumerate() {
        outline.extend(offset_run(sub, &stations, i, h, own, inst.span)?);
    }
    folded.subs = outline;
    folded.d = to_d(&folded.subs);
    folded.geometry = geometry::geometry_bbox(&folded.d);
    opening::place(children, &stations, units * own);
    Ok(())
}

/// One contiguous centreline run → its outline loops [SPEC 15.11]: a closed
/// run gives the two concentric loops (even-odd fills the band between); an
/// open run gives one loop — the left face out, a butt cap, the right face
/// back, a butt cap home.
fn offset_run(
    sub: &Subpath,
    stations: &[opening::Station],
    index: usize,
    h: f64,
    own: f64,
    span: Span,
) -> Result<Vec<Subpath>, Error> {
    check_run(sub, h, own, span)?;
    let mut out = Vec::new();
    for (piece, closed) in cut(sub, stations, index) {
        let segs: Vec<PathSeg> = piece
            .into_iter()
            .filter(|s| dist(s.from(), s.to()) > SEAM_EPS)
            .collect();
        if segs.is_empty() {
            continue;
        }
        let left = side(&segs, closed, h, true);
        let right = side(&segs, closed, h, false);
        let right_back: Vec<PathSeg> = right.iter().rev().map(PathSeg::reverse).collect();
        if closed {
            out.push(Subpath {
                segs: left,
                closed: true,
            });
            out.push(Subpath {
                segs: right_back,
                closed: true,
            });
            continue;
        }
        // Open ends butt-cap flat at the endpoints — no extension [SPEC 15.11];
        // a jamb is exactly that cap, which is why the gap cuts the run rather
        // than the two side chains.
        let mut segs = left;
        push_line(&mut segs, right_back[0].from());
        segs.extend(right_back);
        let home = segs[0].from();
        push_line(&mut segs, home);
        out.push(Subpath { segs, closed: true });
    }
    Ok(out)
}

/// Cut a centreline run at its openings' stations [SPEC 15.11] — the piece
/// list the offset walks, each with its own openness. Ungapped, the run passes
/// through whole; every gap turns the run open, and a closed run's seam-side
/// pieces rejoin so the wrap is one piece, not two.
fn cut(sub: &Subpath, stations: &[opening::Station], index: usize) -> Vec<(Vec<PathSeg>, bool)> {
    if !stations.iter().any(|s| s.on_subpath(index)) {
        return vec![(sub.segs.clone(), sub.closed)];
    }
    let mut pieces: Vec<Vec<PathSeg>> = Vec::new();
    let mut cur: Vec<PathSeg> = Vec::new();
    for (i, seg) in sub.segs.iter().enumerate() {
        let here = opening::gaps(stations, index, i);
        if here.is_empty() {
            cur.push(*seg);
            continue;
        }
        let (from, to) = (seg.from(), seg.to());
        let len = dist(from, to);
        let d = unit((to.0 - from.0, to.1 - from.1)).expect("a station rides a straight run");
        let at = |t: f64| (from.0 + d.0 * t, from.1 + d.1 * t);
        let mut cursor = 0.0;
        for (a, b) in here {
            if a > cursor + SEAM_EPS {
                cur.push(PathSeg::Line {
                    from: at(cursor),
                    to: at(a),
                });
            }
            pieces.push(std::mem::take(&mut cur));
            cursor = b;
        }
        if len > cursor + SEAM_EPS {
            cur.push(PathSeg::Line {
                from: at(cursor),
                to: at(len),
            });
        }
    }
    pieces.push(cur);
    if sub.closed {
        // The seam is not a corner to a gapped run: the tail runs on into the
        // head, one piece.
        let head = pieces.remove(0);
        pieces
            .last_mut()
            .expect("a gap leaves two pieces")
            .extend(head);
    }
    pieces
        .into_iter()
        .filter(|p| !p.is_empty())
        .map(|p| (p, false))
        .collect()
}

/// The centreline laws [SPEC 21]: `curve()` has no exact offset and errors;
/// an arc tighter than thickness ∕ 2 has no inner face and errors (`r ==
/// t ∕ 2` stays legal — the inner arc degenerates to the centre point).
fn check_run(sub: &Subpath, h: f64, own: f64, span: Span) -> Result<(), Error> {
    for seg in &sub.segs {
        match *seg {
            PathSeg::Cubic { .. } => {
                return Err(
                    Error::at(span, "a wall bends with 'arc()' — 'curve()' has no offset")
                        .code(Code::WALL_CURVE),
                );
            }
            PathSeg::Arc { r, .. } if r < h - SEAM_EPS => {
                return Err(Error::at(
                    span,
                    format!(
                        "arc radius {} is under thickness/2 — the inner face vanishes",
                        n(r / own)
                    ),
                )
                .code(Code::WALL_ARC));
            }
            _ => {}
        }
    }
    Ok(())
}

/// One side chain: every segment offset to its parallel (a line shifted along
/// its normal, an arc to its concentric), joined at each interior vertex —
/// and, for a closed run, across the seam, so `close()` mitres like any
/// corner [SPEC 15.11].
fn side(segs: &[PathSeg], closed: bool, h: f64, left: bool) -> Vec<PathSeg> {
    let mut out: Vec<PathSeg> = Vec::new();
    for seg in segs {
        let el = raw_offset(seg, h, left);
        if out.is_empty() {
            out.push(el);
        } else {
            join(&mut out, el);
        }
    }
    if closed && out.len() >= 2 {
        // The wrap join: run the first element through the same join, then
        // seat its (possibly trimmed) copy back at the head — cyclically the
        // inserted connectors belong at the tail.
        let first = out[0];
        join(&mut out, first);
        let seamed = out.pop().expect("join pushed the wrapped element");
        out[0] = seamed;
    }
    out
}

/// A segment's parallel at distance `h` on one side of travel. An arc's left
/// side is outside its circle exactly when it sweeps clockwise, so the
/// concentric radius is `r + h` when `sweep == left`, `r − h` otherwise
/// [SPEC 15.11].
fn raw_offset(seg: &PathSeg, h: f64, left: bool) -> PathSeg {
    match *seg {
        PathSeg::Line { from, to } => {
            let d = unit((to.0 - from.0, to.1 - from.1)).expect("zero-length filtered");
            let nrm = normal(d, left);
            PathSeg::Line {
                from: (from.0 + h * nrm.0, from.1 + h * nrm.1),
                to: (to.0 + h * nrm.0, to.1 + h * nrm.1),
            }
        }
        PathSeg::Arc {
            from,
            to,
            r,
            large,
            sweep,
        } => {
            let c = arc_center(from, to, r, large, sweep);
            let r2 = if sweep == left {
                r + h
            } else {
                (r - h).max(0.0)
            };
            let radial = |p: P| {
                if r2 <= SEAM_EPS {
                    c
                } else {
                    (c.0 + (p.0 - c.0) * (r2 / r), c.1 + (p.1 - c.1) * (r2 / r))
                }
            };
            PathSeg::Arc {
                from: radial(from),
                to: radial(to),
                r: r2,
                large,
                sweep,
            }
        }
        PathSeg::Cubic { .. } => unreachable!("curve() rejected before offsetting"),
    }
}

/// The unit normal on one side of travel direction `d` (y grows down): left
/// of travel is `d` turned a quarter counter-screen — the named-edge
/// convention's side [SPEC 15.5].
fn normal(d: P, left: bool) -> P {
    if left { (d.1, -d.0) } else { (-d.1, d.0) }
}

/// Append a straight connector from the chain's end to `p` — a butt cap
/// here, a bevel or a fallback connect in [`join`].
pub(super) fn push_line(out: &mut Vec<PathSeg>, p: P) {
    let from = out.last().expect("connector needs a chain").to();
    if dist(from, p) > SEAM_EPS {
        out.push(PathSeg::Line { from, to: p });
    }
}

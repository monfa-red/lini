//! `break:` — cut the boring middle [SPEC 15.3]. View-only compression: the
//! folded subpaths are **clipped** at the cut stations, the far piece slides
//! toward the near one leaving the sheet-space break gap, and a piecewise
//! **view map** — monotone, total, invertible — carries the law: anchors and
//! extension lines land at *displayed* positions, measured values always read
//! the *unbroken* model ([`ViewMap::unmap`]). The map is a black hole for
//! every position in the broken frame — features, their sub-features, a
//! pattern's copies all ride it (`ride_view`). A clipped subpath stays
//! **open** at its cut — SVG's implied fill closure is the straight cut edge,
//! the profile stroke never draws there, and the generated `|breakline|`
//! chrome (the standards' thin line with the sharp mid-jog) draws over it.

use super::super::ir::{Bbox, PlacedNode};
use super::geometry::{self, P, PathSeg, Subpath, arc_center, dist, n};
use crate::error::Error;
use crate::ledger::consts::{BREAK_GAP, CENTER_MARK_OVERHANG};
use crate::resolve::{ResolvedInst, ResolvedValue};
use crate::span::Span;

use super::Segment;

mod clip;
mod viewmap;

#[cfg(test)]
mod tests;

use clip::clip_out;
pub use viewmap::ViewMap;
use viewmap::{build_map, displace, forward};

/// One cut edge the chrome draws over [SPEC 15.7]: the cut line's displayed
/// station and its crossing span on the other coordinate.
#[derive(Debug)]
pub struct CutEdge {
    /// Stations on y (a `y-axis` break) — the cut line runs horizontal.
    pub horizontal: bool,
    /// The cut line's displayed coordinate on the break axis.
    pub t: f64,
    /// The crossing span on the other coordinate (lo, hi).
    pub lo: f64,
    pub hi: f64,
}

/// One `break:` group, resolved: stations in model px on its axis.
struct Group {
    a: f64,
    b: f64,
    /// Stations on y — `y-axis`.
    vertical: bool,
}

/// Apply a sketch's `break:` to its folded, scaled subpaths: clip, compress,
/// and return the view map + the cut edges (two per group, authored order).
pub(super) fn apply(
    inst: &ResolvedInst,
    subs: &mut Vec<Subpath>,
    scale: f64,
    span: Span,
) -> Result<(ViewMap, Vec<CutEdge>), Error> {
    let Some(value) = inst.attrs.get("break") else {
        return Ok((ViewMap::default(), Vec::new()));
    };
    let model = geometry::geometry_bbox(&geometry::to_d(subs));
    let groups = parse(value, model, scale, span)?;

    let view = build_map(&groups, span)?;
    let mut cuts = Vec::with_capacity(groups.len() * 2);
    for g in &groups {
        let (kept, xa, xb) = clip_out(std::mem::take(subs), g.vertical, g.a, g.b, span)?;
        *subs = kept;
        for (station, crossings) in [(g.a, xa), (g.b, xb)] {
            let (Some(lo), Some(hi)) = (
                crossings.iter().copied().min_by(f64::total_cmp),
                crossings.iter().copied().max_by(f64::total_cmp),
            ) else {
                return Err(Error::at(
                    span,
                    format!("'break' at {} misses the profile", n(station / scale)),
                ));
            };
            let t = if g.vertical {
                forward(&view.y, station)
            } else {
                forward(&view.x, station)
            };
            // The crossing span rides the *other* coordinate — displaced only
            // when a second break group cuts that axis too.
            let across = if g.vertical { &view.x } else { &view.y };
            cuts.push(CutEdge {
                horizontal: g.vertical,
                t,
                lo: forward(across, lo),
                hi: forward(across, hi),
            });
        }
    }
    displace(subs, &view);
    Ok((view, cuts))
}

/// `break:` value groups → stations [SPEC 15.3]: two numbers, `a < b`, an
/// optional axis; every group defaults to the model's **longer** axis.
fn parse(value: &ResolvedValue, model: Bbox, scale: f64, span: Span) -> Result<Vec<Group>, Error> {
    let bad = || {
        Error::at(
            span,
            "'break' takes two stations 'a b' — a < b — and an optional x-axis / y-axis",
        )
    };
    let longer_is_y = model.h() > model.w();
    let one = |v: &ResolvedValue| -> Result<Group, Error> {
        let ResolvedValue::Tuple(items) = v else {
            return Err(bad());
        };
        let (nums, rest) = match items.as_slice() {
            [a, b] => ([a, b], None),
            [a, b, axis] => ([a, b], Some(axis)),
            _ => return Err(bad()),
        };
        let (Some(a), Some(b)) = (nums[0].as_number(), nums[1].as_number()) else {
            return Err(bad());
        };
        if a >= b {
            return Err(bad());
        }
        let vertical = match rest {
            None => longer_is_y,
            Some(ResolvedValue::Ident(s)) if s == "x-axis" => false,
            Some(ResolvedValue::Ident(s)) if s == "y-axis" => true,
            Some(_) => return Err(bad()),
        };
        Ok(Group {
            a: a * scale,
            b: b * scale,
            vertical,
        })
    };
    match value {
        ResolvedValue::List(items) => items.iter().map(one).collect(),
        v => Ok(vec![one(v)?]),
    }
}
/// Fill the generated `|breakline|` chrome among a sketch's children
/// [SPEC 15.7]: each cut edge is the standards' thin break line — straight
/// across the profile with the sharp jog mid-span. Sheet-space, node-local,
/// indexed `chrome: break N` in authored order.
pub(in crate::layout) fn fill_chrome(children: &mut [PlacedNode], cuts: &[CutEdge]) {
    for c in children.iter_mut() {
        let Some(ResolvedValue::Tuple(items)) = c.attrs.get("chrome") else {
            continue;
        };
        let [ResolvedValue::Ident(k), ResolvedValue::Number(idx)] = items.as_slice() else {
            continue;
        };
        if k != "break" {
            continue;
        }
        let Some(cut) = cuts.get(*idx as usize) else {
            continue;
        };
        let half = c.attrs.number("stroke-width").unwrap_or(0.0) / 2.0;
        let pt = |t: f64, s: f64| if cut.horizontal { (s, t) } else { (t, s) };
        let pts = jogged(cut, pt);
        let value = ResolvedValue::List(
            pts.iter()
                .map(|p| {
                    ResolvedValue::Tuple(vec![
                        ResolvedValue::Number(p.0),
                        ResolvedValue::Number(p.1),
                    ])
                })
                .collect(),
        );
        c.attrs.insert("points", value);
        c.bbox = Bbox::from_points(&pts).inflate(half);
    }
}

/// The thin long-break line: straight across the profile (+ overhang), with
/// the sharp jog mid-span [SPEC 15.7].
fn jogged(cut: &CutEdge, pt: impl Fn(f64, f64) -> P) -> Vec<P> {
    let m = (cut.lo + cut.hi) / 2.0;
    let h = cut.hi - cut.lo;
    // The jog stays clear of its twin across the 12 px gap — amplitude well
    // under half of it.
    let jog = (h * 0.28).min(9.0);
    let amp = (h * 0.2).min(4.5);
    vec![
        pt(cut.t, cut.lo - CENTER_MARK_OVERHANG),
        pt(cut.t, m - jog),
        pt(cut.t + amp, m - jog * 0.15),
        pt(cut.t - amp, m + jog * 0.15),
        pt(cut.t, m + jog),
        pt(cut.t, cut.hi + CENTER_MARK_OVERHANG),
    ]
}

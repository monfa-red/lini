//! `break:` — cut the boring middle [SPEC 15.3]. View-only compression: the
//! folded subpaths are **clipped** at the cut stations, the far piece slides
//! toward the near one leaving the sheet-space break gap, and a piecewise
//! **view map** — monotone, total, invertible — carries the law: anchors and
//! extension lines land at *displayed* positions, measured values always read
//! the *unbroken* model ([`ViewMap::unmap`]). A clipped subpath stays **open**
//! at its cut — SVG's implied fill closure is the straight cut edge, the
//! profile stroke never draws there, and the generated `|breakline|` chrome
//! (zigzag, or the round-stock S when the sketch mirrors across the break
//! axis) draws the drafting line over it.

use super::super::ir::{Bbox, PlacedNode};
use super::geometry::{self, MirrorAxis, P, PathSeg, Subpath, arc_center, dist, n};
use crate::error::Error;
use crate::resolve::{ResolvedInst, ResolvedValue};
use crate::span::Span;

/// The sheet-space daylight a break leaves between the pieces [SPEC 10.5].
const BREAK_GAP: f64 = 12.0;
/// A break line overhangs the profile like the centerline chrome [SPEC 10.5].
const OVERHANG: f64 = 4.0;

const EPS: f64 = 1e-6;

/// The piecewise view-offset map a `break:` builds, per axis: knots of
/// (model, displayed) with identity slope outside and linear interpolation
/// between — the removed span squashes into the gap, so the map stays
/// monotone and invertible.
#[derive(Debug, Default)]
pub struct ViewMap {
    x: Vec<(f64, f64)>,
    y: Vec<(f64, f64)>,
}

impl ViewMap {
    pub fn is_identity(&self) -> bool {
        self.x.is_empty() && self.y.is_empty()
    }

    /// Model → displayed.
    pub fn map(&self, p: P) -> P {
        (forward(&self.x, p.0), forward(&self.y, p.1))
    }

    /// Displayed → model — what measured values read [SPEC 15.3].
    pub fn unmap(&self, p: P) -> P {
        (backward(&self.x, p.0), backward(&self.y, p.1))
    }

    /// A pen segment's displayed position — radii never change, a break only
    /// translates the kept pieces.
    pub fn segment(&self, s: super::Segment) -> super::Segment {
        use super::Segment;
        match s {
            Segment::Point(p) => Segment::Point(self.map(p)),
            Segment::Edge(a, b) => Segment::Edge(self.map(a), self.map(b)),
            Segment::Arc { mid, r } => Segment::Arc {
                mid: self.map(mid),
                r,
            },
            Segment::Circle { center, r } => Segment::Circle {
                center: self.map(center),
                r,
            },
        }
    }
}

/// Piecewise-linear through the knots, identity slope outside.
fn forward(knots: &[(f64, f64)], t: f64) -> f64 {
    interp(knots, t, |k| (k.0, k.1))
}

fn backward(knots: &[(f64, f64)], t: f64) -> f64 {
    interp(knots, t, |k| (k.1, k.0))
}

fn interp(knots: &[(f64, f64)], t: f64, pick: impl Fn(&(f64, f64)) -> (f64, f64)) -> f64 {
    let Some(first) = knots.first() else {
        return t;
    };
    let (f_in, f_out) = pick(first);
    if t <= f_in {
        return t - f_in + f_out;
    }
    for w in knots.windows(2) {
        let (a_in, a_out) = pick(&w[0]);
        let (b_in, b_out) = pick(&w[1]);
        if t <= b_in {
            let f = (t - a_in) / (b_in - a_in);
            return a_out + f * (b_out - a_out);
        }
    }
    let (l_in, l_out) = pick(knots.last().expect("non-empty"));
    t - l_in + l_out
}

/// One cut edge the chrome draws over [SPEC 15.7]: the cut line's displayed
/// station, its crossing span on the other coordinate, and the shape — S for
/// round stock (the sketch mirrors across the break axis), zigzag otherwise.
#[derive(Debug)]
pub struct CutEdge {
    /// Stations on y (a `y-axis` break) — the cut line runs horizontal.
    pub horizontal: bool,
    /// The cut line's displayed coordinate on the break axis.
    pub t: f64,
    /// The crossing span on the other coordinate (lo, hi).
    pub lo: f64,
    pub hi: f64,
    pub s_break: bool,
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
    mirrors: &[MirrorAxis],
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
        // The break axis runs along the stations; round stock mirrors across it.
        let axis_dir: P = if g.vertical { (0.0, 1.0) } else { (1.0, 0.0) };
        let s_break = mirrors.iter().any(|m| {
            let d = m.dir();
            (d.0 * axis_dir.0 + d.1 * axis_dir.1).abs() > 1.0 - EPS
        });
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
                s_break,
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

/// The per-axis knots [SPEC 15.3]: each cut squashes its span into the gap
/// and slides everything past it; groups on one axis must not overlap.
fn build_map(groups: &[Group], span: Span) -> Result<ViewMap, Error> {
    let mut view = ViewMap::default();
    for vertical in [false, true] {
        let mut cuts: Vec<(f64, f64)> = groups
            .iter()
            .filter(|g| g.vertical == vertical)
            .map(|g| (g.a, g.b))
            .collect();
        cuts.sort_by(|p, q| p.0.total_cmp(&q.0));
        let knots = if vertical { &mut view.y } else { &mut view.x };
        let mut shift = 0.0;
        for w in cuts.windows(2) {
            if w[1].0 <= w[0].1 {
                return Err(Error::at(span, "'break' spans overlap — merge them"));
            }
        }
        for (a, b) in cuts {
            knots.push((a, a - shift));
            knots.push((b, a - shift + BREAK_GAP));
            shift += (b - a) - BREAK_GAP;
        }
    }
    Ok(view)
}

/// A clip's yield: the kept subpaths and the cut-point cross-coordinates at
/// each station — what sizes the breakline chrome.
type Clipped = (Vec<Subpath>, Vec<f64>, Vec<f64>);

/// Remove the open band `(a, b)` on one coordinate: split every segment at
/// the stations, drop the pieces inside, and stitch the kept pieces into
/// maximal runs — each an **open** subpath whose endpoints sit on the cut
/// lines (recorded as the crossing spans the chrome draws over).
fn clip_out(
    subs: Vec<Subpath>,
    vertical: bool,
    a: f64,
    b: f64,
    span: Span,
) -> Result<Clipped, Error> {
    let t = |p: P| if vertical { p.1 } else { p.0 };
    let cross = |p: P| if vertical { p.0 } else { p.1 };

    let mut out = Vec::with_capacity(subs.len());
    let (mut xa, mut xb) = (Vec::new(), Vec::new());
    for sub in subs {
        let mut pieces: Vec<(PathSeg, bool)> = Vec::with_capacity(sub.segs.len());
        let mut any_dropped = false;
        for seg in &sub.segs {
            // A cubic never splits — its hull must clear both station lines
            // (cutting through a `curve()` is deferred, [SPEC 23]).
            if let PathSeg::Cubic { from, c1, c2, to } = seg {
                let ts = [t(*from), t(*c1), t(*c2), t(*to)];
                let (lo, hi) = ts
                    .iter()
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |acc, v| {
                        (acc.0.min(*v), acc.1.max(*v))
                    });
                for station in [a, b] {
                    if lo < station - EPS && hi > station + EPS {
                        return Err(Error::at(
                            span,
                            "a 'break' can't cut a 'curve()' — move the stations",
                        ));
                    }
                }
            }
            for piece in split_seg(*seg, vertical, a, b) {
                let m = t(piece_mid(&piece));
                let keep = m <= a + EPS || m >= b - EPS;
                any_dropped |= !keep;
                pieces.push((piece, keep));
            }
        }
        if !any_dropped {
            out.push(sub);
            continue;
        }

        // Maximal kept runs — cyclic for a closed subpath, so a run may wrap
        // through the seam.
        let n = pieces.len();
        let start = pieces
            .iter()
            .position(|(_, keep)| !keep)
            .expect("something dropped");
        let order: Vec<usize> = if sub.closed {
            (1..=n).map(|i| (start + i) % n).collect()
        } else {
            (0..n).collect()
        };
        let mut run: Vec<PathSeg> = Vec::new();
        let mut flush = |run: &mut Vec<PathSeg>| {
            if run.is_empty() {
                return;
            }
            for p in [
                run.first().expect("non-empty").from(),
                run.last().expect("non-empty").to(),
            ] {
                if (t(p) - a).abs() < EPS {
                    xa.push(cross(p));
                } else if (t(p) - b).abs() < EPS {
                    xb.push(cross(p));
                }
            }
            out.push(Subpath {
                segs: std::mem::take(run),
                closed: false,
            });
        };
        for i in order {
            let (piece, keep) = &pieces[i];
            if *keep {
                // Defensive: kept pieces separated by drops flush above; a
                // geometric discontinuity (never expected) flushes too.
                if let Some(last) = run.last()
                    && dist(last.to(), piece.from()) > EPS
                {
                    flush(&mut run);
                }
                run.push(*piece);
            } else {
                flush(&mut run);
            }
        }
        flush(&mut run);
    }
    Ok((out, xa, xb))
}

/// A point on the piece's interior — what classifies it against the band.
fn piece_mid(seg: &PathSeg) -> P {
    match *seg {
        PathSeg::Line { from, to } => ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0),
        PathSeg::Arc {
            from,
            to,
            r,
            large,
            sweep,
        } => {
            let c = arc_center(from, to, r, large, sweep);
            let (a0, travel) = arc_travel(from, to, c, sweep);
            arc_point(c, r, a0 + travel / 2.0 * if sweep { 1.0 } else { -1.0 })
        }
        PathSeg::Cubic { from, c1, c2, to } => (
            (from.0 + 3.0 * c1.0 + 3.0 * c2.0 + to.0) / 8.0,
            (from.1 + 3.0 * c1.1 + 3.0 * c2.1 + to.1) / 8.0,
        ),
    }
}

/// Split one segment at every strict crossing of the two station lines.
fn split_seg(seg: PathSeg, vertical: bool, a: f64, b: f64) -> Vec<PathSeg> {
    let t = |p: P| if vertical { p.1 } else { p.0 };
    match seg {
        PathSeg::Line { from, to } => {
            let (t0, t1) = (t(from), t(to));
            let mut cuts: Vec<f64> = [a, b]
                .into_iter()
                .filter_map(|c| {
                    let s = (c - t0) / (t1 - t0);
                    (s.is_finite() && s > EPS && s < 1.0 - EPS).then_some(s)
                })
                .collect();
            cuts.sort_by(f64::total_cmp);
            let at = |s: f64| (from.0 + (to.0 - from.0) * s, from.1 + (to.1 - from.1) * s);
            let mut pts = vec![from];
            pts.extend(cuts.into_iter().map(at));
            pts.push(to);
            pts.windows(2)
                .map(|w| PathSeg::Line {
                    from: w[0],
                    to: w[1],
                })
                .collect()
        }
        PathSeg::Arc {
            from,
            to,
            r,
            large,
            sweep,
        } => {
            let c = arc_center(from, to, r, large, sweep);
            let (a0, travel) = arc_travel(from, to, c, sweep);
            let dir = if sweep { 1.0 } else { -1.0 };
            // Circle × station line, kept when strictly inside the swept span.
            let mut hits: Vec<f64> = Vec::new();
            for station in [a, b] {
                let d = station - if vertical { c.1 } else { c.0 };
                if d.abs() >= r - EPS {
                    continue;
                }
                let h = (r * r - d * d).sqrt();
                for other in [h, -h] {
                    let p = if vertical {
                        (c.0 + other, station)
                    } else {
                        (station, c.1 + other)
                    };
                    let aq = (p.1 - c.1).atan2(p.0 - c.0);
                    let along = ((aq - a0) * dir).rem_euclid(std::f64::consts::TAU);
                    if along > EPS && along < travel - EPS {
                        hits.push(along);
                    }
                }
            }
            hits.sort_by(f64::total_cmp);
            hits.dedup_by(|p, q| (*p - *q).abs() < EPS);
            let mut angles = vec![0.0];
            angles.extend(hits);
            angles.push(travel);
            angles
                .windows(2)
                .map(|w| {
                    let (p, q) = (
                        arc_point(c, r, a0 + w[0] * dir),
                        arc_point(c, r, a0 + w[1] * dir),
                    );
                    PathSeg::Arc {
                        // Split ends snap to the authored endpoints, so runs
                        // chain watertight.
                        from: if w[0] == 0.0 { from } else { p },
                        to: if w[1] == travel { to } else { q },
                        r,
                        large: w[1] - w[0] > std::f64::consts::PI,
                        sweep,
                    }
                })
                .collect()
        }
        // The advanced 10 % — kept whole when its hull clears the band; a
        // cut through a curve is deferred [SPEC 23].
        PathSeg::Cubic { .. } => vec![seg],
    }
}

/// The arc's start angle and swept magnitude (radians, positive) about `c`.
fn arc_travel(from: P, to: P, c: P, sweep: bool) -> (f64, f64) {
    let a0 = (from.1 - c.1).atan2(from.0 - c.0);
    let a1 = (to.1 - c.1).atan2(to.0 - c.0);
    let dir = if sweep { 1.0 } else { -1.0 };
    (a0, ((a1 - a0) * dir).rem_euclid(std::f64::consts::TAU))
}

fn arc_point(c: P, r: f64, angle: f64) -> P {
    (c.0 + r * angle.cos(), c.1 + r * angle.sin())
}

/// Slide every kept point through the map — each kept piece translates
/// rigidly (the squashed slopes live only inside the removed spans), so arcs
/// stay circular.
fn displace(subs: &mut [Subpath], view: &ViewMap) {
    for sub in subs.iter_mut() {
        for seg in &mut sub.segs {
            *seg = match *seg {
                PathSeg::Line { from, to } => PathSeg::Line {
                    from: view.map(from),
                    to: view.map(to),
                },
                PathSeg::Arc {
                    from,
                    to,
                    r,
                    large,
                    sweep,
                } => PathSeg::Arc {
                    from: view.map(from),
                    to: view.map(to),
                    r,
                    large,
                    sweep,
                },
                PathSeg::Cubic { from, c1, c2, to } => PathSeg::Cubic {
                    from: view.map(from),
                    c1: view.map(c1),
                    c2: view.map(c2),
                    to: view.map(to),
                },
            };
        }
    }
}

/// Fill the generated `|breakline|` chrome among a sketch's children
/// [SPEC 15.7]: the zigzag is a thin polyline with the lightning jog
/// mid-span; the round-stock S turns the node into a stroked path. Both
/// sheet-space, node-local, indexed `chrome: break N` in authored order.
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
        if cut.s_break {
            let (d, bbox) = s_break_d(cut, pt);
            c.kind = crate::resolve::NodeKind::Path;
            c.attrs.insert("path", ResolvedValue::String(d));
            c.bbox = bbox.inflate(half);
        } else {
            let pts = zigzag(cut, pt);
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
            c.bbox = bounds(&pts).inflate(half);
        }
    }
}

/// The thin long-break line: straight across the profile (+ overhang), with
/// the lightning jog mid-span [SPEC 15.7].
fn zigzag(cut: &CutEdge, pt: impl Fn(f64, f64) -> P) -> Vec<P> {
    let m = (cut.lo + cut.hi) / 2.0;
    let h = cut.hi - cut.lo;
    let jog = (h * 0.3).min(6.0);
    let amp = (h * 0.2).min(4.0);
    vec![
        pt(cut.t, cut.lo - OVERHANG),
        pt(cut.t, m - jog),
        pt(cut.t + amp, m - jog / 3.0),
        pt(cut.t - amp, m + jog / 3.0),
        pt(cut.t, m + jog),
        pt(cut.t, cut.hi + OVERHANG),
    ]
}

/// The round-stock S [SPEC 15.7]: two opposed bows meeting at the axis — the
/// freehand break drafting draws on solid round bar.
fn s_break_d(cut: &CutEdge, pt: impl Fn(f64, f64) -> P) -> (String, Bbox) {
    let (lo, hi) = (cut.lo, cut.hi);
    let m = (lo + hi) / 2.0;
    let h = hi - lo;
    let amp = (h * 0.15).clamp(2.0, 8.0);
    let p = |q: P| format!("{} {}", n(q.0), n(q.1));
    let d = format!(
        "M {} C {} {} {} C {} {} {}",
        p(pt(cut.t, lo)),
        p(pt(cut.t + 2.2 * amp, lo + h * 0.28)),
        p(pt(cut.t + 2.2 * amp, m - h * 0.12)),
        p(pt(cut.t, m)),
        p(pt(cut.t - 2.2 * amp, m + h * 0.12)),
        p(pt(cut.t - 2.2 * amp, hi - h * 0.28)),
        p(pt(cut.t, hi)),
    );
    let along = |t: f64, s: f64| pt(t, s);
    let (a, b) = (along(cut.t - 2.2 * amp, lo), along(cut.t + 2.2 * amp, hi));
    (
        d,
        Bbox {
            min_x: a.0.min(b.0),
            min_y: a.1.min(b.1),
            max_x: a.0.max(b.0),
            max_y: a.1.max(b.1),
        },
    )
}

fn bounds(pts: &[P]) -> Bbox {
    let mut b = Bbox {
        min_x: f64::INFINITY,
        min_y: f64::INFINITY,
        max_x: f64::NEG_INFINITY,
        max_y: f64::NEG_INFINITY,
    };
    for p in pts {
        b.min_x = b.min_x.min(p.0);
        b.min_y = b.min_y.min(p.1);
        b.max_x = b.max_x.max(p.0);
        b.max_y = b.max_y.max(p.1);
    }
    b
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{by_id, laid, layout_err, text_at};
    use super::{BREAK_GAP, ViewMap};
    use crate::resolve::NodeKind;

    #[test]
    fn the_view_map_squashes_the_span_and_round_trips() {
        let mut v = ViewMap::default();
        // One cut, 100..200 on x: 100 px folds into the 12 px gap.
        v.x = vec![(100.0, 100.0), (200.0, 100.0 + BREAK_GAP)];
        assert_eq!(v.map((50.0, 7.0)), (50.0, 7.0), "near is identity");
        assert_eq!(v.map((300.0, 0.0)).0, 300.0 - 100.0 + BREAK_GAP);
        assert_eq!(
            v.map((150.0, 0.0)).0,
            100.0 + BREAK_GAP / 2.0,
            "mid-span squashes"
        );
        for t in [-20.0, 100.0, 137.0, 200.0, 450.0] {
            let d = v.map((t, 0.0));
            assert!((v.unmap(d).0 - t).abs() < 1e-9, "round-trip at {t}");
        }
    }

    #[test]
    fn a_break_compresses_the_view_and_the_dim_stays_true() {
        // 300 long, break −80..60: 140 removed, the gap left — 172 displayed;
        // the dimension still reads the unbroken 300 [SPEC 15.3].
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|sketch#bar| { draw: move(-150, 0) up(10) right(300) down(10); mirror: x-axis; break: -80 60 }\nbar:left <-> bar:right { side: bottom }\n",
        );
        let bar = by_id(&l.nodes, "bar");
        assert!(
            (bar.bbox.w() - (172.0 + 2.0)).abs() < 1e-6,
            "compressed + stroke: {}",
            bar.bbox.w()
        );
        text_at(&l.nodes, "300");
    }

    #[test]
    fn break_defaults_to_the_longer_axis() {
        // A tall profile with unnamed stations cuts on y.
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|sketch#post| { draw: move(-10, -60) right(20) down(120) left(20) close(); break: -30 30 }\n",
        );
        let post = by_id(&l.nodes, "post");
        assert!(
            (post.bbox.h() - (72.0 + 2.0)).abs() < 1e-6,
            "120 − 60 + gap: {}",
            post.bbox.h()
        );
        assert!((post.bbox.w() - 22.0).abs() < 1e-6, "x untouched");
    }

    #[test]
    fn round_stock_gets_the_s_break_flat_gets_the_zigzag() {
        // Mirrored across the break axis → the S (a stroked path); an
        // unmirrored plate → the thin zigzag polyline [SPEC 15.7].
        let round = laid(
            "{ layout: drawing; scale: 1 }\n|sketch#bar| { draw: move(-150, 0) up(10) right(300) down(10); mirror: x-axis; break: -80 60 }\n",
        );
        let cuts: Vec<_> = by_id(&round.nodes, "bar")
            .children
            .iter()
            .filter(|c| c.type_chain.iter().any(|t| t == "breakline"))
            .collect();
        assert_eq!(cuts.len(), 2, "one pair per group");
        assert!(
            cuts.iter().all(|c| c.kind == NodeKind::Path),
            "the round-stock S is a path"
        );

        let flat = laid(
            "{ layout: drawing; scale: 1 }\n|sketch#plate| { draw: move(-100, -12) right(200) down(24) left(200) close(); break: -50 50 }\n",
        );
        let cuts: Vec<_> = by_id(&flat.nodes, "plate")
            .children
            .iter()
            .filter(|c| c.type_chain.iter().any(|t| t == "breakline"))
            .collect();
        assert_eq!(cuts.len(), 2);
        assert!(
            cuts.iter().all(|c| c.kind == NodeKind::Line),
            "the zigzag stays a line"
        );
        // The near edge stands at the displayed station, spanning the profile
        // + overhang.
        assert!(
            cuts.iter().any(|c| (c.bbox.min_x - -54.5).abs() < 1e-6),
            "near cut at −50 − jog amplitude − half stroke: {}",
            cuts[0].bbox.min_x
        );
        assert!(
            cuts.iter().all(|c| (c.bbox.h() - 33.0).abs() < 1e-6),
            "24 + 2 × overhang + stroke: {}",
            cuts[0].bbox.h()
        );
    }

    #[test]
    fn a_station_in_the_removed_span_still_measures_true() {
        // `:mid` sits at x = 0, inside the cut — displayed it squashes into
        // the gap, but the dimension reads the model's 150.
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|sketch#bar| { draw: move(-150, 0) up(10) right(150):half :mid right(150) down(10); mirror: x-axis; break: -80 60 }\nbar:left <-> bar:mid { side: bottom }\n",
        );
        text_at(&l.nodes, "150");
    }

    #[test]
    fn a_mirrored_station_span_reads_true_across_a_break() {
        // The ⌀ station reading reflects on the model, so a break never
        // narrows it [SPEC 15.6].
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|sketch#bar| { draw: move(-150, 0) up(10) right(40):thread right(260) down(10); mirror: x-axis; break: -80 60 }\nbar:thread (-) { side: left }\n",
        );
        text_at(&l.nodes, "⌀20");
    }

    #[test]
    fn break_errors_speak_spec() {
        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|rect#a| { width: 40; height: 20; break: -5 5 }\n"
            ),
            "'break' cuts a '|sketch|' — draw the profile with the pen"
        );
        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|sketch#s| { draw: move(0, 0) right(40) down(20) left(40) close(); break: 30 10 }\n"
            ),
            "'break' takes two stations 'a b' — a < b — and an optional x-axis / y-axis"
        );
        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|sketch#s| { draw: move(0, 0) right(40) down(20) left(40) close(); break: 90 100 }\n"
            ),
            "'break' at 90 misses the profile"
        );
        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|sketch#s| { draw: move(0, 0) right(40) down(20) left(40) close(); break: 5 20, 15 35 }\n"
            ),
            "'break' spans overlap — merge them"
        );
        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|sketch#s| { draw: move(0, 0) curve(20, -30, 40, 30, 60, 0) down(20) left(60) close(); break: 20 40 }\n"
            ),
            "a 'break' can't cut a 'curve()' — move the stations"
        );
    }

    #[test]
    fn a_cut_through_an_arc_splits_it_clean() {
        // A half-round profile cut through its arc: both cut edges cross the
        // curve, the kept ends stay on the circle.
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|sketch#dome| { draw: move(-50, 0) arc(100, 0, 50) close(); break: -20 20 }\n",
        );
        let dome = by_id(&l.nodes, "dome");
        // 100 − 40 + 12 = 72 displayed.
        assert!(
            (dome.bbox.w() - 74.0).abs() < 0.1,
            "arc clipped and compressed: {}",
            dome.bbox.w()
        );
    }
}

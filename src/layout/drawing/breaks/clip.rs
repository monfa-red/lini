//! The clip-region construction [SPEC 15.3]: remove the cut band from the folded subpaths, splitting each segment at the station lines and stitching the kept pieces into open runs.

use super::*;

const EPS: f64 = 1e-6;
/// A clip's yield: the kept subpaths and the cut-point cross-coordinates at
/// each station — what sizes the breakline chrome.
pub(super) type Clipped = (Vec<Subpath>, Vec<f64>, Vec<f64>);

/// Remove the open band `(a, b)` on one coordinate: split every segment at
/// the stations, drop the pieces inside, and stitch the kept pieces into
/// maximal runs — each an **open** subpath whose endpoints sit on the cut
/// lines (recorded as the crossing spans the chrome draws over).
pub(super) fn clip_out(
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

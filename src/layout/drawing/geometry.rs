//! The pen's geometry [SPEC 15.3]: subpaths of segments, mirror reflection, and
//! the fold to an SVG `d`. Coordinates are the sketch's own frame — y grows
//! down, the core orientation — and bearings are visual: 0 = up, clockwise.

use super::super::ir::Bbox;
use super::super::path_bbox;

pub type P = (f64, f64);

/// One drawn segment. `from`/`to` chain through a subpath; an arc is circular
/// (SVG `A r r 0 large sweep`), `sweep: true` = clockwise on screen.
#[derive(Debug, Clone, Copy)]
pub enum PathSeg {
    Line {
        from: P,
        to: P,
    },
    Arc {
        from: P,
        to: P,
        r: f64,
        large: bool,
        sweep: bool,
    },
    Cubic {
        from: P,
        c1: P,
        c2: P,
        to: P,
    },
}

impl PathSeg {
    pub fn from(&self) -> P {
        match *self {
            PathSeg::Line { from, .. }
            | PathSeg::Arc { from, .. }
            | PathSeg::Cubic { from, .. } => from,
        }
    }

    pub fn to(&self) -> P {
        match *self {
            PathSeg::Line { to, .. } | PathSeg::Arc { to, .. } | PathSeg::Cubic { to, .. } => to,
        }
    }

    /// The segment reflected across a line through the origin with unit
    /// direction `u`. An arc's sweep flips — reflection reverses handedness.
    fn reflect(&self, u: P) -> PathSeg {
        let m = |p: P| reflect_point(p, u);
        match *self {
            PathSeg::Line { from, to } => PathSeg::Line {
                from: m(from),
                to: m(to),
            },
            PathSeg::Arc {
                from,
                to,
                r,
                large,
                sweep,
            } => PathSeg::Arc {
                from: m(from),
                to: m(to),
                r,
                large,
                sweep: !sweep,
            },
            PathSeg::Cubic { from, c1, c2, to } => PathSeg::Cubic {
                from: m(from),
                c1: m(c1),
                c2: m(c2),
                to: m(to),
            },
        }
    }

    /// The segment walked the other way (endpoints swapped; an arc's sweep
    /// flips, a cubic swaps its controls).
    fn reverse(&self) -> PathSeg {
        match *self {
            PathSeg::Line { from, to } => PathSeg::Line { from: to, to: from },
            PathSeg::Arc {
                from,
                to,
                r,
                large,
                sweep,
            } => PathSeg::Arc {
                from: to,
                to: from,
                r,
                large,
                sweep: !sweep,
            },
            PathSeg::Cubic { from, c1, c2, to } => PathSeg::Cubic {
                from: to,
                c1: c2,
                c2: c1,
                to: from,
            },
        }
    }
}

/// One subpath: a chained run of segments; `closed` emits the `Z` (a fused
/// mirror or a `close()`).
#[derive(Debug, Clone)]
pub struct Subpath {
    pub segs: Vec<PathSeg>,
    pub closed: bool,
}

/// A mirror item [SPEC 15.3]: an axis through the pen origin, kept as its
/// **bearing** (0 = up, clockwise; `x-axis` = 90, `y-axis` = 0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MirrorAxis {
    pub bearing: f64,
}

impl MirrorAxis {
    /// The axis line's unit direction vector.
    pub fn dir(self) -> P {
        bearing_dir(self.bearing)
    }
}

/// A bearing's unit vector — 0 = up = (0, −1), 90 = right, clockwise [SPEC 15.3].
/// The cardinals are exact, not `sin`/`cos` approximations: their coordinates
/// feed measured values ([SPEC 15.6]), where 1e-16 noise would surface.
pub fn bearing_dir(deg: f64) -> P {
    let norm = deg.rem_euclid(360.0);
    if norm == 0.0 {
        (0.0, -1.0)
    } else if norm == 90.0 {
        (1.0, 0.0)
    } else if norm == 180.0 {
        (0.0, 1.0)
    } else if norm == 270.0 {
        (-1.0, 0.0)
    } else {
        let rad = deg.to_radians();
        (rad.sin(), -rad.cos())
    }
}

/// A vector's bearing, the inverse of [`bearing_dir`].
pub fn dir_bearing(v: P) -> f64 {
    let deg = v.0.atan2(-v.1).to_degrees();
    if deg < 0.0 { deg + 360.0 } else { deg }
}

/// Reflect `p` across the line through the origin with unit direction `u`.
pub fn reflect_point(p: P, u: P) -> P {
    let d = p.0 * u.0 + p.1 * u.1;
    (2.0 * d * u.0 - p.0, 2.0 * d * u.1 - p.1)
}

const SEAM_EPS: f64 = 1e-9;

/// Apply one mirror item to every subpath [SPEC 15.3]: a **closed** subpath is
/// duplicated (a reflected second copy); an **open** one is **fused** — the
/// reflection walked back end-to-start, straight seam segments where an
/// endpoint sits off the axis — and becomes closed. Returns whether any
/// subpath fused (the auto-centerline cue, stage 3).
pub fn mirror(subs: &mut Vec<Subpath>, axis: MirrorAxis) -> bool {
    let u = axis.dir();
    let mut fused = false;
    let mut out = Vec::with_capacity(subs.len() * 2);
    for sub in subs.drain(..) {
        if sub.segs.is_empty() {
            continue;
        }
        if sub.closed {
            let copy = Subpath {
                segs: sub.segs.iter().map(|s| s.reflect(u)).collect(),
                closed: true,
            };
            out.push(sub);
            out.push(copy);
            continue;
        }
        fused = true;
        let a = sub.segs.first().expect("non-empty").from();
        let b = sub.segs.last().expect("non-empty").to();
        let (a2, b2) = (reflect_point(a, u), reflect_point(b, u));
        let mut segs = sub.segs.clone();
        if dist(b, b2) > SEAM_EPS {
            segs.push(PathSeg::Line { from: b, to: b2 });
        }
        segs.extend(sub.segs.iter().rev().map(|s| s.reflect(u).reverse()));
        if dist(a2, a) > SEAM_EPS {
            segs.push(PathSeg::Line { from: a2, to: a });
        }
        out.push(Subpath { segs, closed: true });
    }
    *subs = out;
    fused
}

pub fn dist(a: P, b: P) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// `v` scaled to unit length. A near-zero vector divides by a 1e-9 floor
/// rather than blowing up — the callers (leader aim, edge direction) clamp or
/// raycast past the degenerate case.
pub fn unit(v: P) -> P {
    let len = v.0.hypot(v.1).max(1e-9);
    (v.0 / len, v.1 / len)
}

/// The ISO reading angle for text riding a line in direction `dir` [SPEC 15.6]:
/// the line's own angle folded into [-90, 90) so the text reads from the
/// bottom / right — a vertical line's text turns exactly −90.
pub fn iso_text_angle(dir: P) -> f64 {
    let mut theta = dir.1.atan2(dir.0).to_degrees();
    if theta < -90.0 {
        theta += 180.0;
    } else if theta >= 90.0 {
        theta -= 180.0;
    }
    theta
}

/// A circular arc segment's centre — the SVG centre parameterization for the
/// pen's `(r, large, sweep)` encoding. The centre sits off the chord midpoint
/// along its perpendicular, on the side the flags pick: for a minor arc the
/// sweep side, flipped when `large`.
pub fn arc_center(from: P, to: P, r: f64, large: bool, sweep: bool) -> P {
    let chord = dist(from, to).max(1e-12);
    let m = ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0);
    let dhat = ((to.0 - from.0) / chord, (to.1 - from.1) / chord);
    let perp = (-dhat.1, dhat.0);
    let h = (r * r - (chord / 2.0) * (chord / 2.0)).max(0.0).sqrt();
    let sign = if sweep != large { 1.0 } else { -1.0 };
    (m.0 + perp.0 * h * sign, m.1 + perp.1 * h * sign)
}

/// Rotate `p` about `centre` by `deg` — positive reads clockwise on screen
/// (y grows down), matching the pen's bearing convention.
pub fn rotate_about(p: P, centre: P, deg: f64) -> P {
    let (s, c) = deg.to_radians().sin_cos();
    let (x, y) = (p.0 - centre.0, p.1 - centre.1);
    (centre.0 + x * c - y * s, centre.1 + x * s + y * c)
}

/// The minor arc's midpoint — on the far side of the chord from the centre;
/// for a semicircle, a quarter-turn from the start in the sweep direction.
pub fn arc_mid(centre: P, chord_mid: P, r: f64, from: P, sweep: bool) -> P {
    let v = (chord_mid.0 - centre.0, chord_mid.1 - centre.1);
    let len = dist(v, (0.0, 0.0));
    if len > 1e-9 {
        (centre.0 + v.0 / len * r, centre.1 + v.1 / len * r)
    } else {
        rotate_about(from, centre, if sweep { 90.0 } else { -90.0 })
    }
}

/// Multiply every coordinate (and arc radius) by `s` — the node's own `scale:`
/// applied to the folded output, exact for lines and circular arcs [SPEC 15.1].
pub fn scale(subs: &mut [Subpath], s: f64) {
    let m = |p: P| (p.0 * s, p.1 * s);
    for sub in subs {
        for seg in &mut sub.segs {
            *seg = match *seg {
                PathSeg::Line { from, to } => PathSeg::Line {
                    from: m(from),
                    to: m(to),
                },
                PathSeg::Arc {
                    from,
                    to,
                    r,
                    large,
                    sweep,
                } => PathSeg::Arc {
                    from: m(from),
                    to: m(to),
                    r: r * s,
                    large,
                    sweep,
                },
                PathSeg::Cubic { from, c1, c2, to } => PathSeg::Cubic {
                    from: m(from),
                    c1: m(c1),
                    c2: m(c2),
                    to: m(to),
                },
            };
        }
    }
}

/// Fold subpaths to an SVG `d` — absolute commands, deterministic number
/// formatting, even-odd fill semantics left to the emitter.
pub fn to_d(subs: &[Subpath]) -> String {
    let mut d = String::new();
    for sub in subs {
        if sub.segs.is_empty() {
            continue;
        }
        if !d.is_empty() {
            d.push(' ');
        }
        let start = sub.segs[0].from();
        d.push_str(&format!("M {} {}", n(start.0), n(start.1)));
        // A closed subpath's trailing straight seam back to the start is what
        // `Z` draws — skip the redundant `L` (a filleted seam is an arc and
        // stays).
        let count = sub.segs.len();
        let skip_last = |i: usize, seg: &PathSeg| {
            sub.closed
                && i + 1 == count
                && matches!(seg, PathSeg::Line { .. })
                && dist(seg.to(), start) < SEAM_EPS
        };
        for (i, seg) in sub.segs.iter().enumerate() {
            if skip_last(i, seg) {
                continue;
            }
            match *seg {
                PathSeg::Line { to, .. } => d.push_str(&format!(" L {} {}", n(to.0), n(to.1))),
                PathSeg::Arc {
                    to,
                    r,
                    large,
                    sweep,
                    ..
                } => d.push_str(&format!(
                    " A {} {} 0 {} {} {} {}",
                    n(r),
                    n(r),
                    u8::from(large),
                    u8::from(sweep),
                    n(to.0),
                    n(to.1)
                )),
                PathSeg::Cubic { c1, c2, to, .. } => d.push_str(&format!(
                    " C {} {} {} {} {} {}",
                    n(c1.0),
                    n(c1.1),
                    n(c2.0),
                    n(c2.1),
                    n(to.0),
                    n(to.1)
                )),
            }
        }
        if sub.closed {
            d.push_str(" Z");
        }
    }
    d
}

/// The drawn geometry's bbox — the path extent, **stroke excluded** (the
/// measurement box, [SPEC 15.1]); layout inflates it for paint.
pub fn geometry_bbox(d: &str) -> Bbox {
    let pts = path_bbox::extent_points(d);
    let mut it = pts.iter();
    let Some(&(x, y)) = it.next() else {
        return Bbox::empty();
    };
    let mut b = Bbox {
        min_x: x,
        min_y: y,
        max_x: x,
        max_y: y,
    };
    for &(x, y) in it {
        b.min_x = b.min_x.min(x);
        b.min_y = b.min_y.min(y);
        b.max_x = b.max_x.max(x);
        b.max_y = b.max_y.max(y);
    }
    b
}

/// Deterministic coordinate formatting: at most 3 decimals, trailing zeros
/// trimmed — byte-stable output at drafting precision.
pub fn n(v: f64) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    // Avoid "-0".
    let r = if r == 0.0 { 0.0 } else { r };
    let mut s = format!("{r:.3}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}

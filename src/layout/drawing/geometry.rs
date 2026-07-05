//! The pen's geometry [SPEC 15.3]: subpaths of segments, mirror reflection, and
//! the fold to an SVG `d`. Coordinates are the sketch's own frame — y grows
//! down, the core orientation — and bearings are visual: 0 = up, clockwise.

use super::super::ir::Bbox;
use super::super::path_bbox;

pub type P = (f64, f64);

/// One drawn segment. `from`/`to` chain through a subpath; an arc is circular
/// (SVG `A r r 0 large sweep`), `sweep: true` = clockwise on screen.
#[derive(Debug, Clone, Copy)]
pub enum Seg {
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

impl Seg {
    pub fn from(&self) -> P {
        match *self {
            Seg::Line { from, .. } | Seg::Arc { from, .. } | Seg::Cubic { from, .. } => from,
        }
    }

    pub fn to(&self) -> P {
        match *self {
            Seg::Line { to, .. } | Seg::Arc { to, .. } | Seg::Cubic { to, .. } => to,
        }
    }

    /// The segment reflected across a line through the origin with unit
    /// direction `u`. An arc's sweep flips — reflection reverses handedness.
    fn reflect(&self, u: P) -> Seg {
        let m = |p: P| reflect_point(p, u);
        match *self {
            Seg::Line { from, to } => Seg::Line {
                from: m(from),
                to: m(to),
            },
            Seg::Arc {
                from,
                to,
                r,
                large,
                sweep,
            } => Seg::Arc {
                from: m(from),
                to: m(to),
                r,
                large,
                sweep: !sweep,
            },
            Seg::Cubic { from, c1, c2, to } => Seg::Cubic {
                from: m(from),
                c1: m(c1),
                c2: m(c2),
                to: m(to),
            },
        }
    }

    /// The segment walked the other way (endpoints swapped; an arc's sweep
    /// flips, a cubic swaps its controls).
    fn reverse(&self) -> Seg {
        match *self {
            Seg::Line { from, to } => Seg::Line { from: to, to: from },
            Seg::Arc {
                from,
                to,
                r,
                large,
                sweep,
            } => Seg::Arc {
                from: to,
                to: from,
                r,
                large,
                sweep: !sweep,
            },
            Seg::Cubic { from, c1, c2, to } => Seg::Cubic {
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
    pub segs: Vec<Seg>,
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
            segs.push(Seg::Line { from: b, to: b2 });
        }
        segs.extend(sub.segs.iter().rev().map(|s| s.reflect(u).reverse()));
        if dist(a2, a) > SEAM_EPS {
            segs.push(Seg::Line { from: a2, to: a });
        }
        out.push(Subpath { segs, closed: true });
    }
    *subs = out;
    fused
}

pub fn dist(a: P, b: P) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// Multiply every coordinate (and arc radius) by `s` — the node's own `scale:`
/// applied to the folded output, exact for lines and circular arcs [SPEC 15.1].
pub fn scale(subs: &mut [Subpath], s: f64) {
    let m = |p: P| (p.0 * s, p.1 * s);
    for sub in subs {
        for seg in &mut sub.segs {
            *seg = match *seg {
                Seg::Line { from, to } => Seg::Line {
                    from: m(from),
                    to: m(to),
                },
                Seg::Arc {
                    from,
                    to,
                    r,
                    large,
                    sweep,
                } => Seg::Arc {
                    from: m(from),
                    to: m(to),
                    r: r * s,
                    large,
                    sweep,
                },
                Seg::Cubic { from, c1, c2, to } => Seg::Cubic {
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
        let skip_last = |i: usize, seg: &Seg| {
            sub.closed
                && i + 1 == count
                && matches!(seg, Seg::Line { .. })
                && dist(seg.to(), start) < SEAM_EPS
        };
        for (i, seg) in sub.segs.iter().enumerate() {
            if skip_last(i, seg) {
                continue;
            }
            match *seg {
                Seg::Line { to, .. } => d.push_str(&format!(" L {} {}", n(to.0), n(to.1))),
                Seg::Arc {
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
                Seg::Cubic { c1, c2, to, .. } => d.push_str(&format!(
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

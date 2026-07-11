//! The break view map [SPEC 15.3]: the monotone, invertible per-axis coordinate map a `break:` builds, and the displacement that slides kept geometry through it.

use super::*;

/// The piecewise view-offset map a `break:` builds, per axis: knots of
/// (model, displayed) with identity slope outside and linear interpolation
/// between — the removed span squashes into the gap, so the map stays
/// monotone and invertible.
#[derive(Debug, Default)]
pub struct ViewMap {
    pub(super) x: Vec<(f64, f64)>,
    pub(super) y: Vec<(f64, f64)>,
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
pub(super) fn forward(knots: &[(f64, f64)], t: f64) -> f64 {
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
/// The per-axis knots [SPEC 15.3]: each cut squashes its span into the gap
/// and slides everything past it; groups on one axis must not overlap.
pub(super) fn build_map(groups: &[Group], span: Span) -> Result<ViewMap, Error> {
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
/// Slide every kept point through the map — each kept piece translates
/// rigidly (the squashed slopes live only inside the removed spans), so arcs
/// stay circular.
pub(super) fn displace(subs: &mut [Subpath], view: &ViewMap) {
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

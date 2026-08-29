//! The mm grid a fixture symbol is drawn on [SPEC 15.11] — the few primitive
//! shapes every family is built from, and the one place they become path data.
//!
//! A symbol is authored in **physical millimetres**, centred on the fixture's
//! own origin, and then carried into pixels by a single anisotropic factor per
//! axis ([`Sym::d`]) — the stretch that takes an intrinsic body to its resolved
//! box. Nothing here knows a scale, a unit, or a variant: it is the drawing
//! alphabet, so a family module reads as the shape it draws and nothing else.

use crate::layout::drawing::geometry::n;

/// One stroke of a symbol, in millimetres on the fixture's centred grid.
pub(super) enum Shape {
    /// A closed run of points — a rectangle, an L-shaped body — with every
    /// corner filleted at radius `r` (`0` draws them sharp).
    Poly(Vec<(f64, f64)>, f64),
    /// An open run — the seat lines, the folds, the arrows — with its
    /// **interior** corners filleted at radius `r`; the two ends stay put.
    Line(Vec<(f64, f64)>, f64),
    /// `cx cy rx ry` — a closed ellipse (a circle is `rx == ry`).
    Oval(f64, f64, f64, f64),
}

/// A finished symbol: its shapes and the **intrinsic extent** they occupy on
/// the mm grid. The extent is what the resolved box stretches from, so it is
/// stated by the family (a dining set's chairs push it past the tabletop),
/// never measured back off the strokes.
pub(super) struct Sym {
    pub(super) extent: (f64, f64),
    pub(super) shapes: Vec<Shape>,
}

impl Sym {
    pub(super) fn new(extent: (f64, f64), shapes: Vec<Shape>) -> Sym {
        Sym { extent, shapes }
    }

    /// The symbol as SVG path data, at `sx` / `sy` pixels per millimetre. The
    /// two factors differ exactly when the body stretches [SPEC 15.11]; a
    /// round detail follows its body, which is what stretching a shape means.
    pub(super) fn d(&self, sx: f64, sy: f64) -> String {
        let mut d = String::new();
        for s in &self.shapes {
            if !d.is_empty() {
                d.push(' ');
            }
            d.push_str(&draw(s, sx, sy));
        }
        d
    }
}

fn draw(s: &Shape, sx: f64, sy: f64) -> String {
    match s {
        Shape::Poly(pts, r) => run(&wound(pts), *r, true, sx, sy),
        Shape::Line(pts, r) => run(&wound(pts), *r, false, sx, sy),
        // Two half-arcs — the one ellipse form every renderer draws alike.
        Shape::Oval(cx, cy, rx, ry) => {
            let p = |x: f64, y: f64| format!("{} {}", n(x * sx), n(y * sy));
            let (a, b) = (n(rx * sx), n(ry * sy));
            format!(
                "M {} A {a} {b} 0 1 1 {} A {a} {b} 0 1 1 {} Z",
                p(cx - rx, *cy),
                p(cx + rx, *cy),
                p(cx - rx, *cy)
            )
        }
    }
}

/// A point run as path data: straight from corner to corner, each **filleted**
/// corner replaced by the arc of radius `r` tangent to both its edges. A closed
/// run starts where its first corner's fillet leaves it, so the rounded form of
/// a shape traces the same circuit its sharp form does.
fn run(pts: &[(f64, f64)], r: f64, closed: bool, sx: f64, sy: f64) -> String {
    let p = |q: (f64, f64)| format!("{} {}", n(q.0 * sx), n(q.1 * sy));
    let Some(last) = pts.len().checked_sub(1) else {
        return String::new();
    };
    let (rx, ry) = (n(r * sx), n(r * sy));
    // A run's two ends are corners of nothing, so only a closed run rounds
    // them; everything else turns where the author put it.
    let corner = |i: usize| {
        (r > 0.0 && (closed || (i > 0 && i < last)))
            .then(|| {
                fillet(
                    pts[(i + last) % pts.len()],
                    pts[i],
                    pts[(i + 1) % pts.len()],
                    r,
                )
            })
            .flatten()
    };
    let arc = |f: &Fillet| {
        format!(
            " L {} A {rx} {ry} 0 0 {} {}",
            p(f.entry),
            f.sweep,
            p(f.exit)
        )
    };
    let head = corner(0);
    let mut d = format!(
        "M {}",
        p(match (closed, &head) {
            (true, Some(f)) => f.exit,
            _ => pts[0],
        })
    );
    for (i, q) in pts.iter().enumerate().skip(1) {
        match corner(i) {
            Some(f) => d.push_str(&arc(&f)),
            None => {
                d.push_str(" L ");
                d.push_str(&p(*q));
            }
        }
    }
    if closed {
        if let Some(f) = &head {
            d.push_str(&arc(f));
        }
        d.push_str(" Z");
    }
    d
}

/// Where a corner's fillet leaves one edge and rejoins the next, and which way
/// its arc turns.
struct Fillet {
    entry: (f64, f64),
    exit: (f64, f64),
    sweep: u8,
}

/// The arc of radius `r` tangent to both edges meeting at `v`. The trim
/// `r / tan(θ ∕ 2)` is written from the two unit edge vectors' dot and cross,
/// so it needs no trigonometry and a right angle — every corner this alphabet
/// actually draws — trims exactly `r`. A vertex the run passes straight
/// through, or folds back on, has no fillet.
fn fillet(prev: (f64, f64), v: (f64, f64), next: (f64, f64), r: f64) -> Option<Fillet> {
    let unit = |a: (f64, f64)| {
        let (dx, dy) = (a.0 - v.0, a.1 - v.1);
        let len = (dx * dx + dy * dy).sqrt();
        (len > 0.0).then_some((dx / len, dy / len))
    };
    let (u, w) = (unit(prev)?, unit(next)?);
    let cross = u.0 * w.1 - u.1 * w.0;
    if cross.abs() < 1e-9 {
        return None;
    }
    let trim = r * (1.0 + u.0 * w.0 + u.1 * w.1) / cross.abs();
    Some(Fillet {
        entry: (v.0 + trim * u.0, v.1 + trim * u.1),
        exit: (v.0 + trim * w.0, v.1 + trim * w.1),
        sweep: u8::from(cross < 0.0),
    })
}

/// A point run turned to the alphabet's **one winding** — the sense [`rect`]
/// states its corners in.
///
/// A fixture is a single path, so its subpaths fill by the nonzero rule: two
/// that wind against each other cancel, and the body would show a hole exactly
/// where it is meant to mask the floor. An open detail line encloses area too
/// (the fill closes it), so this is what makes the alphabet safe to draw with
/// — the author states the run in whichever direction reads, and the winding
/// is never theirs to get right.
fn wound(pts: &[(f64, f64)]) -> std::borrow::Cow<'_, [(f64, f64)]> {
    let area: f64 = pts
        .iter()
        .zip(pts.iter().cycle().skip(1))
        .map(|(a, b)| a.0 * b.1 - b.0 * a.1)
        .sum();
    if area < 0.0 {
        let mut flipped = pts.to_vec();
        flipped.reverse();
        return std::borrow::Cow::Owned(flipped);
    }
    std::borrow::Cow::Borrowed(pts)
}

/// A rectangle stated by its corners, `r` rounding all four of them.
pub(super) fn rect(x0: f64, y0: f64, x1: f64, y1: f64, r: f64) -> Shape {
    Shape::Poly(vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)], r)
}

/// A rectangle stated by its centre and size — the way most furniture reads.
pub(super) fn box_at(cx: f64, cy: f64, w: f64, h: f64, r: f64) -> Shape {
    rect(cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0, r)
}

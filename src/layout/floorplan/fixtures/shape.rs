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
    /// `x0 y0 x1 y1` — a closed rectangle.
    Rect(f64, f64, f64, f64),
    /// `x0 y0 x1 y1 r` — a rectangle with rounded corners of radius `r`.
    Round(f64, f64, f64, f64, f64),
    /// `cx cy rx ry` — a closed ellipse (a circle is `rx == ry`).
    Oval(f64, f64, f64, f64),
    /// An open polyline — the seat lines, the folds, the arrows.
    Line(Vec<(f64, f64)>),
    /// A closed polygon — the L-shaped bodies.
    Poly(Vec<(f64, f64)>),
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
    let p = |x: f64, y: f64| format!("{} {}", n(x * sx), n(y * sy));
    match s {
        Shape::Rect(x0, y0, x1, y1) => format!(
            "M {} L {} L {} L {} Z",
            p(*x0, *y0),
            p(*x1, *y0),
            p(*x1, *y1),
            p(*x0, *y1)
        ),
        Shape::Round(x0, y0, x1, y1, r) => {
            let (rx, ry) = (n(r * sx), n(r * sy));
            let arc = |x: f64, y: f64| format!("A {rx} {ry} 0 0 1 {}", p(x, y));
            format!(
                "M {} L {} {} L {} {} L {} {} L {} {} Z",
                p(x0 + r, *y0),
                p(x1 - r, *y0),
                arc(*x1, y0 + r),
                p(*x1, y1 - r),
                arc(x1 - r, *y1),
                p(x0 + r, *y1),
                arc(*x0, y1 - r),
                p(*x0, y0 + r),
                arc(x0 + r, *y0),
            )
        }
        // Two half-arcs — the one ellipse form every renderer draws alike.
        Shape::Oval(cx, cy, rx, ry) => {
            let (a, b) = (n(rx * sx), n(ry * sy));
            format!(
                "M {} A {a} {b} 0 1 1 {} A {a} {b} 0 1 1 {} Z",
                p(cx - rx, *cy),
                p(cx + rx, *cy),
                p(cx - rx, *cy)
            )
        }
        Shape::Line(pts) | Shape::Poly(pts) => {
            let pts = wound(pts);
            let mut d = String::new();
            for (i, (x, y)) in pts.iter().enumerate() {
                d.push_str(if i == 0 { "M " } else { " L " });
                d.push_str(&p(*x, *y));
            }
            if matches!(s, Shape::Poly(_)) {
                d.push_str(" Z");
            }
            d
        }
    }
}

/// A point run turned to the alphabet's **one winding** — the sense the
/// rectangles, rounds and ovals above are all authored in.
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

/// A rectangle stated by its centre and size — the way most furniture reads.
pub(super) fn box_at(cx: f64, cy: f64, w: f64, h: f64) -> Shape {
    Shape::Rect(cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0)
}

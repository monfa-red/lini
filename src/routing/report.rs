//! The routing report: rule identities, violations, and the crossing
//! primitive every consumer shares (the engine's count, the independent
//! checker, and the renderer's fillet pass — a crossing must never land
//! mid-arc).

use crate::span::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// Law 1 — a link holds ≥ clearance from every node body.
    Clearance,
    /// Law 1 — a link holds ≥ pitch from every other link.
    Separation,
    /// Law 2 — every end lands perpendicular on a side, clear of corners.
    Contact,
    /// Law 3 — a drawn, square-on crossing: counted output, not a defect.
    Crossing,
    /// A link with no legal route at this layout: reported, drawn as a stray.
    Impossible,
}

impl Rule {
    pub fn id(self) -> &'static str {
        match self {
            Rule::Clearance => "clearance",
            Rule::Separation => "separation",
            Rule::Contact => "contact",
            Rule::Crossing => "crossing",
            Rule::Impossible => "impossible",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// Surfaced as a diagnostic; `--strict` escalates it to an error.
    Warning,
    /// Normal, counted output (crossings).
    Info,
}

#[derive(Clone, Debug)]
pub struct Violation {
    pub rule: Rule,
    /// `Info` for the engine's own counted output (drawn crossings);
    /// `Warning` for everything the diagnostic layer must surface —
    /// impossible links and any law breach the independent checker finds.
    pub severity: Severity,
    pub links: Vec<String>,
    pub detail: String,
    /// The declaration this violation points back to.
    pub span: Span,
}

/// The transversal intersection of two orthogonal segments: one horizontal,
/// one vertical, each strictly inside the other — touches and collinear
/// overlaps are contact, not crossings. Pure geometry; a miss here surfaces
/// as a separation breach in the checker.
pub(crate) fn cross(a: &[(f64, f64)], b: &[(f64, f64)]) -> Option<(f64, f64)> {
    let (h, v) = if a[0].1 == a[1].1 && b[0].0 == b[1].0 {
        (a, b)
    } else if a[0].0 == a[1].0 && b[0].1 == b[1].1 {
        (b, a)
    } else {
        return None;
    };
    let (x, y) = (v[0].0, h[0].1);
    let (hx0, hx1) = (h[0].0.min(h[1].0), h[0].0.max(h[1].0));
    let (vy0, vy1) = (v[0].1.min(v[1].1), v[0].1.max(v[1].1));
    (hx0 < x && x < hx1 && vy0 < y && y < vy1).then_some((x, y))
}

/// The transversal intersection of two segments of **any** orientation —
/// the natural strategy's oblique crossings (ROUTING.md The natural
/// strategy: crossings need not be square-on). The same reading as
/// [`cross`] freed of the axis-aligned assumption: strictly interior on
/// both segments; parallels, touches, and collinear overlaps are contact.
/// Orthogonal pairs keep [`cross`] (comparisons, not cross products) so
/// their counts stay byte-identical.
pub(crate) fn cross_oblique(a: &[(f64, f64)], b: &[(f64, f64)]) -> Option<(f64, f64)> {
    let r = (a[1].0 - a[0].0, a[1].1 - a[0].1);
    let s = (b[1].0 - b[0].0, b[1].1 - b[0].1);
    let den = r.0 * s.1 - r.1 * s.0;
    if den == 0.0 {
        return None;
    }
    let qp = (b[0].0 - a[0].0, b[0].1 - a[0].1);
    let t = (qp.0 * s.1 - qp.1 * s.0) / den;
    let u = (qp.0 * r.1 - qp.1 * r.0) / den;
    (t > 0.0 && t < 1.0 && u > 0.0 && u < 1.0).then(|| (a[0].0 + t * r.0, a[0].1 + t * r.1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transversal_pairs_cross_at_the_meet() {
        let h = [(0.0, 5.0), (10.0, 5.0)];
        let v = [(4.0, 0.0), (4.0, 10.0)];
        assert_eq!(cross(&h, &v), Some((4.0, 5.0)));
        assert_eq!(cross(&v, &h), Some((4.0, 5.0)));
    }

    #[test]
    fn oblique_pairs_cross_at_the_meet() {
        let a = [(0.0, 0.0), (10.0, 10.0)];
        let b = [(0.0, 10.0), (10.0, 0.0)];
        assert_eq!(cross_oblique(&a, &b), Some((5.0, 5.0)));
        // Agreement with `cross` on a transversal orthogonal pair.
        let h = [(0.0, 5.0), (10.0, 5.0)];
        let v = [(4.0, 0.0), (4.0, 10.0)];
        assert_eq!(cross_oblique(&h, &v), cross(&h, &v));
        // Touches, parallels, and collinear overlaps are contact.
        assert_eq!(cross_oblique(&a, &[(10.0, 10.0), (20.0, 0.0)]), None);
        assert_eq!(cross_oblique(&a, &[(1.0, 0.0), (11.0, 10.0)]), None);
        assert_eq!(cross_oblique(&a, &[(5.0, 5.0), (20.0, 20.0)]), None);
    }

    #[test]
    fn touches_and_parallels_are_not_crossings() {
        let h = [(0.0, 5.0), (10.0, 5.0)];
        // Endpoint touch: the vertical starts exactly on the horizontal.
        assert_eq!(cross(&h, &[(10.0, 0.0), (10.0, 10.0)]), None);
        assert_eq!(cross(&h, &[(4.0, 5.0), (4.0, 10.0)]), None);
        // Parallel and collinear overlaps are contact, never crossings.
        assert_eq!(cross(&h, &[(0.0, 7.0), (10.0, 7.0)]), None);
        assert_eq!(cross(&h, &[(5.0, 5.0), (20.0, 5.0)]), None);
    }
}

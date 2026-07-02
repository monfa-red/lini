//! Axis-aligned rectangle — the one geometric primitive the router shares.

// Scaffold: the keep-out and channel maths consume these again
// (ROUTING-V2.md stage 2); the allow leaves with them.
#![allow(dead_code)]

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Rect {
    pub fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Rect {
        Rect { x0, y0, x1, y1 }
    }

    pub fn w(&self) -> f64 {
        self.x1 - self.x0
    }

    pub fn h(&self) -> f64 {
        self.y1 - self.y0
    }

    /// Grow by `d` on every side (the keep-out construction).
    pub fn inflate(&self, d: f64) -> Rect {
        Rect::new(self.x0 - d, self.y0 - d, self.x1 + d, self.y1 + d)
    }

    /// The overlap with positive area, if any — touching edges don't count.
    pub fn intersect(&self, other: &Rect) -> Option<Rect> {
        let r = Rect::new(
            self.x0.max(other.x0),
            self.y0.max(other.y0),
            self.x1.min(other.x1),
            self.y1.min(other.y1),
        );
        (r.w() > 0.0 && r.h() > 0.0).then_some(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extent_is_signed_span() {
        let r = Rect::new(-10.0, 5.0, 30.0, 25.0);
        assert_eq!(r.w(), 40.0);
        assert_eq!(r.h(), 20.0);
    }

    #[test]
    fn inflate_grows_every_side() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0).inflate(8.0);
        assert_eq!(r, Rect::new(-8.0, -8.0, 18.0, 18.0));
    }

    #[test]
    fn intersect_returns_the_overlap() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, -5.0, 20.0, 5.0);
        assert_eq!(a.intersect(&b), Some(Rect::new(5.0, 0.0, 10.0, 5.0)));
    }

    #[test]
    fn intersect_is_none_for_disjoint_and_touching() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert_eq!(a.intersect(&Rect::new(20.0, 0.0, 30.0, 10.0)), None);
        assert_eq!(a.intersect(&Rect::new(10.0, 0.0, 30.0, 10.0)), None);
    }
}

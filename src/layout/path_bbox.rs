//! Bounding box of an SVG `<path>` `d` string. `|path|` draws in native
//! coordinates (SPEC §7), so to size it in flow we walk the command stream and
//! accumulate a tight extent — every on-curve point, the analytic extrema of
//! each bézier, and sampled points along each elliptical arc. Layout only;
//! rendering emits the raw `d` untouched.

type P = (f64, f64);

/// Points bounding the path. `bounding_box` over them is the path's box; the
/// list is empty when `d` carries no drawable command (the caller then falls
/// back to an empty box).
pub fn extent_points(d: &str) -> Vec<P> {
    let mut s = Scanner::new(d);
    let mut pts: Vec<P> = Vec::new();
    let (mut cx, mut cy) = (0.0, 0.0); // current point
    let (mut sx, mut sy) = (0.0, 0.0); // subpath start (for Z)
    let mut cubic_ctrl = (0.0, 0.0); // 2nd control of the last C/S (for S)
    let mut quad_ctrl = (0.0, 0.0); // control of the last Q/T (for T)
    let mut prev = 0u8;

    while let Some(cmd) = s.command() {
        let rel = cmd.is_ascii_lowercase();
        let up = cmd.to_ascii_uppercase();
        match up {
            b'M' => {
                if let Some((x, y)) = s.coord(rel, cx, cy) {
                    cx = x;
                    cy = y;
                    sx = x;
                    sy = y;
                    pts.push((cx, cy));
                }
                // extra pairs after a moveto are implicit linetos
                while let Some((x, y)) = s.coord(rel, cx, cy) {
                    cx = x;
                    cy = y;
                    pts.push((cx, cy));
                }
            }
            b'L' => {
                while let Some((x, y)) = s.coord(rel, cx, cy) {
                    cx = x;
                    cy = y;
                    pts.push((cx, cy));
                }
            }
            b'H' => {
                while let Some(n) = s.number() {
                    cx = if rel { cx + n } else { n };
                    pts.push((cx, cy));
                }
            }
            b'V' => {
                while let Some(n) = s.number() {
                    cy = if rel { cy + n } else { n };
                    pts.push((cx, cy));
                }
            }
            b'C' => {
                while let Some([c1, c2, end]) = s.coords3(rel, cx, cy) {
                    cubic((cx, cy), c1, c2, end, &mut pts);
                    cubic_ctrl = c2;
                    (cx, cy) = end;
                }
            }
            b'S' => {
                let mut after_cubic = matches!(prev, b'C' | b'S');
                while let Some([c2, end]) = s.coords2(rel, cx, cy) {
                    let c1 = if after_cubic {
                        (2.0 * cx - cubic_ctrl.0, 2.0 * cy - cubic_ctrl.1)
                    } else {
                        (cx, cy)
                    };
                    cubic((cx, cy), c1, c2, end, &mut pts);
                    cubic_ctrl = c2;
                    (cx, cy) = end;
                    after_cubic = true;
                }
            }
            b'Q' => {
                while let Some([ctrl, end]) = s.coords2(rel, cx, cy) {
                    quad((cx, cy), ctrl, end, &mut pts);
                    quad_ctrl = ctrl;
                    (cx, cy) = end;
                }
            }
            b'T' => {
                let mut after_quad = matches!(prev, b'Q' | b'T');
                while let Some((x, y)) = s.coord(rel, cx, cy) {
                    let ctrl = if after_quad {
                        (2.0 * cx - quad_ctrl.0, 2.0 * cy - quad_ctrl.1)
                    } else {
                        (cx, cy)
                    };
                    quad((cx, cy), ctrl, (x, y), &mut pts);
                    quad_ctrl = ctrl;
                    (cx, cy) = (x, y);
                    after_quad = true;
                }
            }
            b'A' => {
                while let Some(a) = s.arc(rel, cx, cy) {
                    arc((cx, cy), &a, &mut pts);
                    (cx, cy) = a.end;
                }
            }
            b'Z' => {
                cx = sx;
                cy = sy;
                pts.push((cx, cy));
            }
            _ => break, // unknown command — stop, keep what was parsed
        }
        prev = up;
    }
    pts
}

// ── Bézier extents (endpoints + axis-aligned extrema) ──────────────────────

fn cubic(p0: P, p1: P, p2: P, p3: P, out: &mut Vec<P>) {
    out.push(p3);
    for t in cubic_ts(p0.0, p1.0, p2.0, p3.0)
        .into_iter()
        .chain(cubic_ts(p0.1, p1.1, p2.1, p3.1))
    {
        let u = 1.0 - t;
        let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        out.push((
            w0 * p0.0 + w1 * p1.0 + w2 * p2.0 + w3 * p3.0,
            w0 * p0.1 + w1 * p1.1 + w2 * p2.1 + w3 * p3.1,
        ));
    }
}

/// Roots of the cubic's derivative on one axis, in (0, 1).
fn cubic_ts(a: f64, b: f64, c: f64, d: f64) -> Vec<f64> {
    let (d0, d1, d2) = (b - a, c - b, d - c);
    solve_quadratic(d0 - 2.0 * d1 + d2, 2.0 * (d1 - d0), d0)
}

fn quad(p0: P, p1: P, p2: P, out: &mut Vec<P>) {
    out.push(p2);
    for t in quad_ts(p0.0, p1.0, p2.0)
        .into_iter()
        .chain(quad_ts(p0.1, p1.1, p2.1))
    {
        let u = 1.0 - t;
        out.push((
            u * u * p0.0 + 2.0 * u * t * p1.0 + t * t * p2.0,
            u * u * p0.1 + 2.0 * u * t * p1.1 + t * t * p2.1,
        ));
    }
}

/// Root of the quadratic's derivative on one axis, in (0, 1).
fn quad_ts(a: f64, b: f64, c: f64) -> Vec<f64> {
    let den = a - 2.0 * b + c;
    if den.abs() < 1e-12 {
        return vec![];
    }
    let t = (a - b) / den;
    if t > 0.0 && t < 1.0 { vec![t] } else { vec![] }
}

fn solve_quadratic(a: f64, b: f64, c: f64) -> Vec<f64> {
    if a.abs() < 1e-12 {
        if b.abs() < 1e-12 {
            return vec![];
        }
        let t = -c / b;
        return if t > 0.0 && t < 1.0 { vec![t] } else { vec![] };
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return vec![];
    }
    let sq = disc.sqrt();
    [(-b + sq) / (2.0 * a), (-b - sq) / (2.0 * a)]
        .into_iter()
        .filter(|&t| t > 0.0 && t < 1.0)
        .collect()
}

// ── Elliptical arc (endpoint → centre form, then sampled) ──────────────────

struct ArcSeg {
    rx: f64,
    ry: f64,
    rot: f64,
    large: bool,
    sweep: bool,
    end: P,
}

fn arc(p0: P, a: &ArcSeg, out: &mut Vec<P>) {
    out.push(a.end);
    let (mut rx, mut ry) = (a.rx.abs(), a.ry.abs());
    if rx < 1e-9 || ry < 1e-9 || (p0.0 - a.end.0).abs() + (p0.1 - a.end.1).abs() < 1e-9 {
        return; // degenerate — treated as a straight line, endpoints suffice
    }
    let phi = a.rot.to_radians();
    let (cosp, sinp) = (phi.cos(), phi.sin());
    let (dx, dy) = ((p0.0 - a.end.0) / 2.0, (p0.1 - a.end.1) / 2.0);
    let x1p = cosp * dx + sinp * dy;
    let y1p = -sinp * dx + cosp * dy;
    let lambda = x1p * x1p / (rx * rx) + y1p * y1p / (ry * ry);
    if lambda > 1.0 {
        let s = lambda.sqrt();
        rx *= s;
        ry *= s;
    }
    let sign = if a.large != a.sweep { 1.0 } else { -1.0 };
    let num = (rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p).max(0.0);
    let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
    let co = sign * (num / den).sqrt();
    let cxp = co * rx * y1p / ry;
    let cyp = co * -ry * x1p / rx;
    let cx = cosp * cxp - sinp * cyp + (p0.0 + a.end.0) / 2.0;
    let cy = sinp * cxp + cosp * cyp + (p0.1 + a.end.1) / 2.0;
    let angle = |ux: f64, uy: f64, vx: f64, vy: f64| {
        let dot = ux * vx + uy * vy;
        let len = ((ux * ux + uy * uy) * (vx * vx + vy * vy)).sqrt();
        let mut t = (dot / len).clamp(-1.0, 1.0).acos();
        if ux * vy - uy * vx < 0.0 {
            t = -t;
        }
        t
    };
    let (ux, uy) = ((x1p - cxp) / rx, (y1p - cyp) / ry);
    let (vx, vy) = ((-x1p - cxp) / rx, (-y1p - cyp) / ry);
    let theta1 = angle(1.0, 0.0, ux, uy);
    let mut dtheta = angle(ux, uy, vx, vy);
    if !a.sweep && dtheta > 0.0 {
        dtheta -= std::f64::consts::TAU;
    } else if a.sweep && dtheta < 0.0 {
        dtheta += std::f64::consts::TAU;
    }
    let steps = 24;
    for i in 1..steps {
        let t = theta1 + dtheta * (i as f64) / (steps as f64);
        let (ct, st) = (t.cos(), t.sin());
        out.push((
            cx + rx * ct * cosp - ry * st * sinp,
            cy + rx * ct * sinp + ry * st * cosp,
        ));
    }
}

// ── Tokenizer ──────────────────────────────────────────────────────────────

struct Scanner<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Scanner<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            b: s.as_bytes(),
            i: 0,
        }
    }

    fn skip(&mut self) {
        while matches!(
            self.b.get(self.i),
            Some(b' ' | b'\t' | b'\n' | b'\r' | b',')
        ) {
            self.i += 1;
        }
    }

    fn command(&mut self) -> Option<u8> {
        self.skip();
        let c = *self.b.get(self.i)?;
        if c.is_ascii_alphabetic() {
            self.i += 1;
            Some(c)
        } else {
            None
        }
    }

    /// Parse one SVG number, honouring implicit separators (`1-2`, `.5.5`).
    fn number(&mut self) -> Option<f64> {
        self.skip();
        let start = self.i;
        let n = self.b.len();
        if matches!(self.b.get(self.i), Some(b'+' | b'-')) {
            self.i += 1;
        }
        let mut any = false;
        while self.i < n && self.b[self.i].is_ascii_digit() {
            self.i += 1;
            any = true;
        }
        if self.b.get(self.i) == Some(&b'.') {
            self.i += 1;
            while self.i < n && self.b[self.i].is_ascii_digit() {
                self.i += 1;
                any = true;
            }
        }
        if any && matches!(self.b.get(self.i), Some(b'e' | b'E')) {
            let save = self.i;
            self.i += 1;
            if matches!(self.b.get(self.i), Some(b'+' | b'-')) {
                self.i += 1;
            }
            let mut exp = false;
            while self.i < n && self.b[self.i].is_ascii_digit() {
                self.i += 1;
                exp = true;
            }
            if !exp {
                self.i = save;
            }
        }
        if !any {
            self.i = start;
            return None;
        }
        std::str::from_utf8(&self.b[start..self.i])
            .ok()?
            .parse()
            .ok()
    }

    fn coord(&mut self, rel: bool, cx: f64, cy: f64) -> Option<P> {
        let save = self.i;
        let x = self.number()?;
        let Some(y) = self.number() else {
            self.i = save;
            return None;
        };
        Some(if rel { (cx + x, cy + y) } else { (x, y) })
    }

    fn coords2(&mut self, rel: bool, cx: f64, cy: f64) -> Option<[P; 2]> {
        let save = self.i;
        let a = self.coord(rel, cx, cy)?;
        let Some(b) = self.coord(rel, cx, cy) else {
            self.i = save;
            return None;
        };
        Some([a, b])
    }

    fn coords3(&mut self, rel: bool, cx: f64, cy: f64) -> Option<[P; 3]> {
        let save = self.i;
        let a = self.coord(rel, cx, cy)?;
        let (Some(b), Some(c)) = (self.coord(rel, cx, cy), self.coord(rel, cx, cy)) else {
            self.i = save;
            return None;
        };
        Some([a, b, c])
    }

    /// An arc flag is a single `0`/`1`, which may abut the next number.
    fn flag(&mut self) -> Option<f64> {
        self.skip();
        match self.b.get(self.i) {
            Some(b'0') => {
                self.i += 1;
                Some(0.0)
            }
            Some(b'1') => {
                self.i += 1;
                Some(1.0)
            }
            _ => self.number(),
        }
    }

    fn arc(&mut self, rel: bool, cx: f64, cy: f64) -> Option<ArcSeg> {
        let save = self.i;
        let rx = self.number()?;
        let (ry, rot) = (self.number(), self.number());
        let (large, sweep) = (self.flag(), self.flag());
        let (x, y) = (self.number(), self.number());
        match (ry, rot, large, sweep, x, y) {
            (Some(ry), Some(rot), Some(large), Some(sweep), Some(x), Some(y)) => Some(ArcSeg {
                rx,
                ry,
                rot,
                large: large != 0.0,
                sweep: sweep != 0.0,
                end: if rel { (cx + x, cy + y) } else { (x, y) },
            }),
            _ => {
                self.i = save;
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::extent_points;

    fn box_of(d: &str) -> (f64, f64, f64, f64) {
        let p = extent_points(d);
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for (x, y) in p {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
        (x0, y0, x1, y1)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 0.05
    }

    #[test]
    fn lines_and_relative() {
        // M then relative l: a 100×40 box from (10,10).
        let (x0, y0, x1, y1) = box_of("M 10 10 l 100 0 l 0 40 l -100 0 Z");
        assert!(close(x0, 10.0) && close(y0, 10.0) && close(x1, 110.0) && close(y1, 50.0));
    }

    #[test]
    fn h_and_v() {
        let (x0, y0, x1, y1) = box_of("M 0 0 H 50 V 30");
        assert!(close(x0, 0.0) && close(y0, 0.0) && close(x1, 50.0) && close(y1, 30.0));
    }

    #[test]
    fn quad_peak_is_tighter_than_control() {
        // Control at y=-28 but the curve only reaches y=0 (the extremum), so
        // the box is [-32,32]×[0,28], not the control hull.
        let (x0, y0, x1, y1) = box_of("M -32 28 Q 0 -28 32 28");
        assert!(close(x0, -32.0) && close(x1, 32.0), "x {x0}..{x1}");
        assert!(close(y0, 0.0) && close(y1, 28.0), "y {y0}..{y1}");
    }

    #[test]
    fn cubic_extrema() {
        // Symmetric cubic: controls at ±26, curve peaks at ±19.5.
        let (_, y0, _, y1) = box_of("M -30 0 C -30 -26 30 -26 30 0 C 30 26 -30 26 -30 0 Z");
        assert!(close(y0, -19.5) && close(y1, 19.5), "y {y0}..{y1}");
    }

    #[test]
    fn semicircle_arc() {
        // sweep=1 is clockwise in SVG's y-down space, so this 0→100 semicircle
        // of radius 50 bulges *up* to y=-50 (verified against resvg).
        let (x0, y0, x1, y1) = box_of("M 0 0 A 50 50 0 0 1 100 0");
        assert!(close(x0, 0.0) && close(x1, 100.0), "x {x0}..{x1}");
        assert!(close(y0, -50.0) && close(y1, 0.0), "y {y0}..{y1}");
    }
}

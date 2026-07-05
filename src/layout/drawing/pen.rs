//! The sketch pen [SPEC 15.3]: fold a `|sketch|`'s structured `draw:` items
//! (kept as [`ResolvedValue::PenCall`] / [`PenPoint`] by resolve) into
//! [`Subpath`]s, apply corner modifiers and `mirror:`, collect the authored
//! `:name` products, and emit the SVG `d` + geometry bbox.
//!
//! Errors follow SPEC 20 verbatim where a message is specified there.

use super::super::ir::Bbox;
use super::geometry::{
    self, MirrorAxis, P, Seg, Subpath, bearing_dir, dir_bearing, dist, geometry_bbox, to_d,
};
use crate::error::Error;
use crate::resolve::{ResolvedCall, ResolvedInst, ResolvedValue};
use crate::span::Span;

/// What an authored `:name` addresses [SPEC 15.2] — collected here, consumed by
/// the drawing engine's anchors (PLAN.md stage 4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Product {
    /// A freestanding name — the pen's point there.
    Point(P),
    /// A straight run (or a chamfer bevel, or a `close()` seam) — carries its
    /// direction for dimension axes.
    Edge(P, P),
    /// An arc (drawn, tangent, or a fillet) — a point on it plus its radius,
    /// the `R` reading.
    Arc { mid: P, r: f64 },
    /// A `circle(r)` subpath — round by construction, the `⌀` reading.
    Circle { center: P, r: f64 },
}

impl Product {
    /// The product under the node's own `scale:` — a uniform coordinate map,
    /// so directions survive and radii multiply.
    fn scaled(self, s: f64) -> Self {
        let m = |p: P| (p.0 * s, p.1 * s);
        match self {
            Product::Point(p) => Product::Point(m(p)),
            Product::Edge(a, b) => Product::Edge(m(a), m(b)),
            Product::Arc { mid, r } => Product::Arc {
                mid: m(mid),
                r: r * s,
            },
            Product::Circle { center, r } => Product::Circle {
                center: m(center),
                r: r * s,
            },
        }
    }
}

/// A folded sketch: the path, its measurement bbox, and everything the drawing
/// engine reads later.
#[derive(Debug)]
pub struct Folded {
    pub d: String,
    /// The drawn extent, stroke excluded — the measurement box [SPEC 15.1].
    pub geometry: Bbox,
    /// Authored `:name`s in source order (duplicates rejected at fold) — carried
    /// on the placed node so mates (and, later, dimensions) can anchor on them.
    pub names: Vec<(String, Product)>,
    /// The applied `mirror:` axes — the unary mirrored readings read them.
    #[expect(
        dead_code,
        reason = "read by the drawing annotations — PLAN.md stage 4"
    )]
    pub mirror_axes: Vec<MirrorAxis>,
    /// Whether any open subpath fused. The auto-centerline chrome keys on the
    /// same fact *syntactically* at desugar (an open subpath + `mirror:` —
    /// [SPEC 15.7]); the tests assert the two judgements agree.
    #[allow(
        dead_code,
        reason = "asserted against desugar's openness check in tests"
    )]
    pub fused: bool,
}

/// Fold a `|sketch|`'s `draw:` (+ `mirror:`) into its geometry, at the node's
/// own effective `scale:` (px per drawing unit — applied to the folded output,
/// so every call keeps its authored semantics; [SPEC 15.1]). The one entry
/// point — layout calls it; the drawing engine reads the same result.
pub fn fold(inst: &ResolvedInst, scale: f64) -> Result<Folded, Error> {
    let span = inst.span;
    let Some(draw) = inst.attrs.get("draw") else {
        return Err(Error::at(span, "'|sketch|' requires 'draw'"));
    };
    let items: Vec<&ResolvedValue> = match draw {
        ResolvedValue::Tuple(items) => items.iter().collect(),
        one => vec![one],
    };

    let mut pen = Pen::new(span);
    for item in items {
        match item {
            ResolvedValue::PenCall { call, product } => pen.call(call, product.as_deref())?,
            ResolvedValue::PenPoint(name) => pen.point_name(name)?,
            _ => {
                return Err(Error::at(
                    span,
                    "'draw' holds pen calls and ':name' points — see SPEC 15.3",
                ));
            }
        }
    }
    let (mut subs, mut names) = pen.finish()?;

    let mut mirror_axes = Vec::new();
    let mut fused = false;
    if let Some(v) = inst.attrs.get("mirror") {
        for axis in parse_mirror(v, span)? {
            fused |= geometry::mirror(&mut subs, axis);
            mirror_axes.push(axis);
        }
    }
    if scale != 1.0 {
        geometry::scale(&mut subs, scale);
        for (_, p) in &mut names {
            *p = p.scaled(scale);
        }
    }

    let d = to_d(&subs);
    Ok(Folded {
        geometry: geometry_bbox(&d),
        d,
        names,
        mirror_axes,
        fused,
    })
}

/// The built-in anchor names an authored `:name` may not shadow [SPEC 15.2].
fn is_builtin_point(name: &str) -> bool {
    matches!(
        name,
        "top"
            | "bottom"
            | "left"
            | "right"
            | "center"
            | "top-left"
            | "top-right"
            | "bottom-left"
            | "bottom-right"
    )
}

/// `mirror:` items → axes: `x-axis` (bearing 90), `y-axis` (0), or a bearing.
fn parse_mirror(v: &ResolvedValue, span: Span) -> Result<Vec<MirrorAxis>, Error> {
    let one = |item: &ResolvedValue| -> Result<MirrorAxis, Error> {
        match item {
            ResolvedValue::Ident(s) if s == "x-axis" => Ok(MirrorAxis { bearing: 90.0 }),
            ResolvedValue::Ident(s) if s == "y-axis" => Ok(MirrorAxis { bearing: 0.0 }),
            ResolvedValue::Number(b) => Ok(MirrorAxis { bearing: *b }),
            _ => Err(Error::at(
                span,
                "'mirror' takes x-axis, y-axis, or a bearing",
            )),
        }
    };
    match v {
        ResolvedValue::Tuple(items) => items.iter().map(one).collect(),
        item => Ok(vec![one(item)?]),
    }
}

/// A pending corner modifier — parked between its two segments.
#[derive(Clone, Copy)]
enum Mod {
    Fillet(f64),
    Chamfer(f64),
}

impl Mod {
    fn word(self) -> &'static str {
        match self {
            Mod::Fillet(_) => "fillet",
            Mod::Chamfer(_) => "chamfer",
        }
    }
}

/// Authored names with their products, in source order.
type Names = Vec<(String, Product)>;

/// The fold state machine: position, heading, the open subpath, a parked
/// corner modifier, and the finished subpaths.
struct Pen {
    span: Span,
    subs: Vec<Subpath>,
    cur: Vec<Seg>,
    start: Option<P>,
    pos: P,
    /// Bearing after the last drawing call; `angle()` sets it absolutely, the
    /// tangent `arc(r, deg)` reads and turns it.
    heading: Option<f64>,
    pending: Option<(Mod, Option<String>)>,
    /// The subpath just closed by `close()` — a modifier right after it rounds
    /// the seam-to-first corner (the cyclic case, [SPEC 15.3]).
    just_closed: bool,
    names: Vec<(String, Product)>,
}

impl Pen {
    fn new(span: Span) -> Self {
        Pen {
            span,
            subs: Vec::new(),
            cur: Vec::new(),
            start: None,
            pos: (0.0, 0.0),
            heading: None,
            pending: None,
            just_closed: false,
            names: Vec::new(),
        }
    }

    fn err(&self, msg: impl Into<String>) -> Error {
        Error::at(self.span, msg.into())
    }

    fn name(&mut self, name: &str, product: Product) -> Result<(), Error> {
        if is_builtin_point(name) {
            return Err(self.err(format!(
                "':{name}' is a built-in anchor — pick another name"
            )));
        }
        if self.names.iter().any(|(n, _)| n == name) {
            return Err(self.err(format!("':{name}' is already named in this 'draw:'")));
        }
        self.names.push((name.to_string(), product));
        Ok(())
    }

    /// A freestanding `:name` — the pen's current point; at a modifier corner
    /// it records the theoretical sharp corner, in either order [SPEC 15.3].
    fn point_name(&mut self, name: &str) -> Result<(), Error> {
        if self.start.is_none() && !self.just_closed {
            return Err(self.err("the pen starts with move(x, y)"));
        }
        self.name(name, Product::Point(self.pos))
    }

    fn call(&mut self, call: &ResolvedCall, product: Option<&str>) -> Result<(), Error> {
        // Only the corner modifiers (and a name / new subpath) may follow a
        // close() — the pen returned to the seam [SPEC 15.3].
        if self.just_closed
            && !matches!(call.name.as_str(), "fillet" | "chamfer" | "move" | "circle")
        {
            return Err(self.err("after close(), start the next subpath with move()"));
        }
        match call.name.as_str() {
            "move" => {
                let [x, y] = self.nums::<2>(call, "'move' takes (x, y)")?;
                if product.is_some() {
                    return Err(self.err(
                        "'move' takes no product name — name its landing with a freestanding ':name'",
                    ));
                }
                self.flush()?;
                self.start = Some((x, y));
                self.pos = (x, y);
            }
            "left" | "right" | "up" | "down" => {
                let [len] = self.nums::<1>(call, "an orthogonal run takes a length")?;
                let bearing = match call.name.as_str() {
                    "up" => 0.0,
                    "right" => 90.0,
                    "down" => 180.0,
                    _ => 270.0,
                };
                self.run(bearing_scaled(bearing, len), Some(bearing), product)?;
            }
            "line" => {
                let [dx, dy] = self.nums::<2>(call, "'line' takes (dx, dy)")?;
                self.run((dx, dy), Some(dir_bearing((dx, dy))), product)?;
            }
            "angle" => {
                let [deg, len] = self.nums::<2>(call, "'angle' takes (deg, n)")?;
                self.run(bearing_scaled(deg, len), Some(deg), product)?;
            }
            "arc" => match call.args.len() {
                3 => {
                    let [dx, dy, r] =
                        self.nums::<3>(call, "'arc' takes (dx, dy, r) or (r, deg)")?;
                    self.arc_to((dx, dy), r, product)?;
                }
                2 => {
                    let [r, deg] = self.nums::<2>(call, "'arc' takes (dx, dy, r) or (r, deg)")?;
                    self.arc_turn(r, deg, product)?;
                }
                _ => return Err(self.err("'arc' takes (dx, dy, r) or (r, deg)")),
            },
            "curve" => {
                let [dx1, dy1, dx2, dy2, dx, dy] =
                    self.nums::<6>(call, "'curve' takes (dx1, dy1, dx2, dy2, dx, dy)")?;
                let from = self.started()?;
                let c1 = (from.0 + dx1, from.1 + dy1);
                let c2 = (from.0 + dx2, from.1 + dy2);
                let to = (from.0 + dx, from.1 + dy);
                self.push_seg(Seg::Cubic { from, c1, c2, to })?;
                let tangent = (to.0 - c2.0, to.1 - c2.1);
                if dist(tangent, (0.0, 0.0)) > 1e-9 {
                    self.heading = Some(dir_bearing(tangent));
                }
                self.pos = to;
                if let Some(nm) = product {
                    self.name(nm, Product::Edge(from, to))?;
                }
            }
            "fillet" | "chamfer" => {
                let msg = format!("'{}' modifies the corner between two segments", call.name);
                let [v] = self.nums::<1>(call, &msg)?;
                let m = if call.name == "fillet" {
                    Mod::Fillet(v)
                } else {
                    Mod::Chamfer(v)
                };
                if self.pending.is_some() {
                    return Err(self.err(msg));
                }
                if self.just_closed {
                    // The cyclic corner: between the closed subpath's last
                    // segment (the seam) and its first [SPEC 15.3].
                    let name = self.apply_cyclic(m, product)?;
                    if let Some((nm, p)) = name {
                        self.name(&nm, p)?;
                    }
                } else {
                    if self.cur.is_empty() {
                        return Err(self.err(msg));
                    }
                    self.pending = Some((m, product.map(str::to_string)));
                }
            }
            "circle" => {
                let [r] = self.nums::<1>(call, "'circle' takes a radius")?;
                if r <= 0.0 {
                    return Err(self.err("'circle' takes a radius > 0"));
                }
                if self.start.is_none() && !self.just_closed {
                    return Err(self.err("the pen starts with move(x, y)"));
                }
                let (cx, cy) = self.pos;
                let (w, e) = ((cx - r, cy), (cx + r, cy));
                self.subs.push(Subpath {
                    segs: vec![
                        Seg::Arc {
                            from: w,
                            to: e,
                            r,
                            large: false,
                            sweep: true,
                        },
                        Seg::Arc {
                            from: e,
                            to: w,
                            r,
                            large: false,
                            sweep: true,
                        },
                    ],
                    closed: true,
                });
                if let Some(nm) = product {
                    self.name(
                        nm,
                        Product::Circle {
                            center: self.pos,
                            r,
                        },
                    )?;
                }
            }
            "close" => {
                if self.cur.is_empty() {
                    return Err(self.err("close() needs a drawn subpath"));
                }
                let start = self.start.expect("cur non-empty implies a start");
                let seam = Seg::Line {
                    from: self.pos,
                    to: start,
                };
                if dist(self.pos, start) > 1e-9 {
                    self.push_seg(seam)?; // consumes a pending corner modifier
                    self.heading = Some(dir_bearing((start.0 - self.pos.0, start.1 - self.pos.1)));
                } else if let Some((m, _)) = self.pending {
                    return Err(self.err(format!(
                        "'{}' modifies the corner between two segments",
                        m.word()
                    )));
                }
                if let Some(nm) = product {
                    self.name(nm, Product::Edge(self.pos, start))?;
                }
                self.pos = start;
                let segs = std::mem::take(&mut self.cur);
                self.subs.push(Subpath { segs, closed: true });
                self.start = None;
                self.just_closed = true;
            }
            other => return Err(self.err(format!("unknown draw call '{other}'"))),
        }
        Ok(())
    }

    /// A straight run by `delta`; the heading follows the drawn direction.
    fn run(&mut self, delta: P, bearing: Option<f64>, product: Option<&str>) -> Result<(), Error> {
        let from = self.started()?;
        let to = (from.0 + delta.0, from.1 + delta.1);
        self.push_seg(Seg::Line { from, to })?;
        self.pos = to;
        if bearing.is_some() {
            self.heading = bearing;
        }
        if let Some(nm) = product {
            self.name(nm, Product::Edge(from, to))?;
        }
        Ok(())
    }

    /// `arc(dx, dy, r)` — the minor arc to a relative point; `r > 0` sweeps
    /// clockwise; `|r|` at least half the chord [SPEC 15.3].
    fn arc_to(&mut self, delta: P, r: f64, product: Option<&str>) -> Result<(), Error> {
        let from = self.started()?;
        let to = (from.0 + delta.0, from.1 + delta.1);
        let chord = dist(from, to);
        if chord < 1e-9 {
            return Err(self.err("'arc' needs a non-zero chord — a full turn is 'circle(r)'"));
        }
        let ra = r.abs();
        if ra < chord / 2.0 - 1e-9 {
            return Err(self.err(format!(
                "arc radius {} is smaller than half the chord",
                geometry::n(ra)
            )));
        }
        let sweep = r > 0.0;
        let ra = ra.max(chord / 2.0);
        // Centre: chord midpoint offset along the (sweep-side) perpendicular.
        let m = ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0);
        let dhat = ((to.0 - from.0) / chord, (to.1 - from.1) / chord);
        let perp = (-dhat.1, dhat.0);
        let h = (ra * ra - (chord / 2.0) * (chord / 2.0)).max(0.0).sqrt();
        let centre = if sweep {
            (m.0 + perp.0 * h, m.1 + perp.1 * h)
        } else {
            (m.0 - perp.0 * h, m.1 - perp.1 * h)
        };
        self.push_seg(Seg::Arc {
            from,
            to,
            r: ra,
            large: false,
            sweep,
        })?;
        self.pos = to;
        let rad = (to.0 - centre.0, to.1 - centre.1);
        let tangent = if sweep {
            (-rad.1, rad.0)
        } else {
            (rad.1, -rad.0)
        };
        self.heading = Some(dir_bearing(tangent));
        if let Some(nm) = product {
            let mid = arc_mid(centre, m, ra, from, sweep);
            self.name(nm, Product::Arc { mid, r: ra })?;
        }
        Ok(())
    }

    /// `arc(r, deg)` — a tangent arc: continue the heading, sweep `deg`
    /// (positive turns clockwise); the heading updates by `deg` [SPEC 15.3].
    fn arc_turn(&mut self, r: f64, deg: f64, product: Option<&str>) -> Result<(), Error> {
        let from = self.started()?;
        if r <= 0.0 {
            return Err(self.err("'arc(r, deg)' takes a radius > 0"));
        }
        if deg == 0.0 || deg.abs() >= 360.0 {
            return Err(self.err("'arc(r, deg)' sweeps within (-360, 360), not 0"));
        }
        let Some(heading) = self.heading else {
            return Err(self.err("'arc(r, deg)' continues a heading — draw a run first"));
        };
        let hv = bearing_dir(heading);
        let cw = deg > 0.0;
        // Turning clockwise pivots on a centre to the heading's right.
        let centre = if cw {
            (from.0 - hv.1 * r, from.1 + hv.0 * r)
        } else {
            (from.0 + hv.1 * r, from.1 - hv.0 * r)
        };
        let to = rotate_about(from, centre, deg);
        self.push_seg(Seg::Arc {
            from,
            to,
            r,
            large: deg.abs() > 180.0,
            sweep: cw,
        })?;
        self.pos = to;
        self.heading = Some((heading + deg).rem_euclid(360.0));
        if let Some(nm) = product {
            let mid = rotate_about(from, centre, deg / 2.0);
            self.name(nm, Product::Arc { mid, r })?;
        }
        Ok(())
    }

    /// Append a segment, applying any parked corner modifier between the
    /// previous segment and this one.
    fn push_seg(&mut self, seg: Seg) -> Result<(), Error> {
        self.just_closed = false;
        let seg = match self.pending.take() {
            None => seg,
            Some((m, name)) => {
                let prev = self.cur.pop().expect("pending implies a previous segment");
                let (prev, mid, next, product) = apply_mod(m, prev, seg, self.span)?;
                self.cur.push(prev);
                self.cur.push(mid);
                if let Some(nm) = name {
                    self.name(&nm, product)?;
                }
                next
            }
        };
        self.cur.push(seg);
        Ok(())
    }

    /// The cyclic corner after `close()` [SPEC 15.3]: modify between the closed
    /// subpath's last and first segments.
    fn apply_cyclic(
        &mut self,
        m: Mod,
        product: Option<&str>,
    ) -> Result<Option<(String, Product)>, Error> {
        let sub = self.subs.last_mut().expect("just_closed implies a subpath");
        if sub.segs.len() < 2 {
            return Err(self.err(format!(
                "'{}' modifies the corner between two segments",
                m.word()
            )));
        }
        let last = sub.segs.pop().expect("len checked");
        let first = sub.segs.remove(0);
        let (last, mid, first, prod) = apply_mod(m, last, first, self.span)?;
        sub.segs.insert(0, first);
        sub.segs.push(last);
        sub.segs.push(mid);
        Ok(product.map(|nm| (nm.to_string(), prod)))
    }

    fn started(&self) -> Result<P, Error> {
        if self.start.is_none() {
            return Err(self.err("the pen starts with move(x, y)"));
        }
        Ok(self.pos)
    }

    /// Flush the open subpath (a new `move()` or the end of the stream).
    fn flush(&mut self) -> Result<(), Error> {
        if let Some((m, _)) = self.pending {
            return Err(self.err(format!(
                "'{}' modifies the corner between two segments",
                m.word()
            )));
        }
        if !self.cur.is_empty() {
            let segs = std::mem::take(&mut self.cur);
            self.subs.push(Subpath {
                segs,
                closed: false,
            });
        }
        self.start = None;
        self.just_closed = false;
        Ok(())
    }

    fn finish(mut self) -> Result<(Vec<Subpath>, Names), Error> {
        self.flush()?;
        if self.subs.iter().all(|s| s.segs.is_empty()) {
            return Err(self.err("'draw' draws nothing — add a pen run"));
        }
        Ok((self.subs, self.names))
    }

    /// N numeric arguments, exactly.
    fn nums<const N: usize>(&self, call: &ResolvedCall, usage: &str) -> Result<[f64; N], Error> {
        if call.args.len() != N {
            return Err(self.err(usage.to_string()));
        }
        let mut out = [0.0; N];
        for (slot, arg) in out.iter_mut().zip(&call.args) {
            *slot = arg
                .as_number()
                .ok_or_else(|| self.err(format!("a pen argument is a number — {usage}")))?;
        }
        Ok(out)
    }
}

/// `bearing_dir` scaled to a run length.
fn bearing_scaled(bearing: f64, len: f64) -> P {
    let d = bearing_dir(bearing);
    (d.0 * len, d.1 * len)
}

/// Rotate `p` about `centre` by `deg` — positive reads clockwise on screen
/// (y grows down), matching the pen's bearing convention.
fn rotate_about(p: P, centre: P, deg: f64) -> P {
    let (s, c) = deg.to_radians().sin_cos();
    let (x, y) = (p.0 - centre.0, p.1 - centre.1);
    (centre.0 + x * c - y * s, centre.1 + x * s + y * c)
}

/// The minor arc's midpoint — on the far side of the chord from the centre;
/// for a semicircle, a quarter-turn from the start in the sweep direction.
fn arc_mid(centre: P, chord_mid: P, r: f64, from: P, sweep: bool) -> P {
    let v = (chord_mid.0 - centre.0, chord_mid.1 - centre.1);
    let len = dist(v, (0.0, 0.0));
    if len > 1e-9 {
        (centre.0 + v.0 / len * r, centre.1 + v.1 / len * r)
    } else {
        rotate_about(from, centre, if sweep { 90.0 } else { -90.0 })
    }
}

/// Trim the corner between two straight runs and drop in the modifier's joint:
/// a tangent arc (`fillet`) or a straight bevel (`chamfer`, cut `c` back along
/// each leg) [SPEC 15.3]. Returns (trimmed prev, joint, trimmed next, the
/// joint's `:name` product).
fn apply_mod(m: Mod, prev: Seg, next: Seg, span: Span) -> Result<(Seg, Seg, Seg, Product), Error> {
    let (Seg::Line { from: a, to: c1 }, Seg::Line { from: c2, to: b }) = (prev, next) else {
        return Err(Error::at(
            span,
            format!("'{}' joins two straight segments today", m.word()),
        ));
    };
    debug_assert!(dist(c1, c2) < 1e-9, "corner segments meet at one point");
    let c = c1;
    let (la, lb) = (dist(a, c), dist(c, b));
    let da = ((c.0 - a.0) / la, (c.1 - a.1) / la);
    let db = ((b.0 - c.0) / lb, (b.1 - c.1) / lb);
    let cross = da.0 * db.1 - da.1 * db.0;
    if cross.abs() < 1e-9 {
        return Err(Error::at(
            span,
            format!("'{}' needs a turn between its two runs", m.word()),
        ));
    }
    let interior = (-(da.0 * db.0 + da.1 * db.1)).clamp(-1.0, 1.0).acos();
    let t = match m {
        Mod::Fillet(r) => r / (interior / 2.0).tan(),
        Mod::Chamfer(cc) => cc,
    };
    let amount = match m {
        Mod::Fillet(r) => r,
        Mod::Chamfer(cc) => cc,
    };
    if amount <= 0.0 || t > la - 1e-9 || t > lb - 1e-9 {
        return Err(Error::at(
            span,
            format!(
                "{} {} does not fit its corner",
                m.word(),
                geometry::n(amount)
            ),
        ));
    }
    let ta = (c.0 - da.0 * t, c.1 - da.1 * t);
    let tb = (c.0 + db.0 * t, c.1 + db.1 * t);
    let (mid, product) = match m {
        Mod::Fillet(r) => {
            let sweep = cross > 0.0;
            // Centre: perpendicular off the incoming leg at the tangent point.
            let centre = if sweep {
                (ta.0 - da.1 * r, ta.1 + da.0 * r)
            } else {
                (ta.0 + da.1 * r, ta.1 - da.0 * r)
            };
            let clen = dist(centre, c);
            let on_arc = (
                centre.0 + (c.0 - centre.0) / clen * r,
                centre.1 + (c.1 - centre.1) / clen * r,
            );
            (
                Seg::Arc {
                    from: ta,
                    to: tb,
                    r,
                    large: false,
                    sweep,
                },
                Product::Arc { mid: on_arc, r },
            )
        }
        Mod::Chamfer(_) => (Seg::Line { from: ta, to: tb }, Product::Edge(ta, tb)),
    };
    Ok((
        Seg::Line { from: a, to: ta },
        mid,
        Seg::Line { from: tb, to: b },
        product,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fold a `draw:` (+ optional `mirror:`) straight from source, through the
    /// real parse/desugar/resolve pipeline — the pen sees exactly what layout will.
    fn program(src: &str) -> Result<crate::resolve::Program, crate::error::Error> {
        let toks = crate::lexer::lex(src)?;
        let file = crate::syntax::parser::parse(&toks)?;
        let lowered = crate::desugar::desugar(&file)?;
        crate::resolve::resolve_with_theme(&lowered, &[])
    }

    fn folded(style: &str) -> Folded {
        let src = format!("|sketch#s| {{ {style} }}\n");
        let program = program(&src).expect("pipeline");
        fold(&program.scene.nodes[0], 1.0).expect("fold")
    }

    fn fold_err(style: &str) -> String {
        let src = format!("|sketch#s| {{ {style} }}\n");
        match program(&src) {
            Err(e) => e.message,
            Ok(p) => {
                fold(&p.scene.nodes[0], 1.0)
                    .expect_err("expected a fold error")
                    .message
            }
        }
    }

    #[test]
    fn a_rectangle_profile_folds_and_closes() {
        let f = folded("draw: move(0, 0) right(40) down(20) left(40) close();");
        assert_eq!(f.d, "M 0 0 L 40 0 L 40 20 L 0 20 Z");
        assert_eq!(
            (
                f.geometry.min_x,
                f.geometry.min_y,
                f.geometry.max_x,
                f.geometry.max_y
            ),
            (0.0, 0.0, 40.0, 20.0)
        );
    }

    #[test]
    fn verbs_are_visual_and_y_grows_down() {
        // up(10) must decrease y; the frame is the core one [SPEC 15.3].
        let f = folded("draw: move(0, 0) up(10) right(5);");
        assert_eq!(f.d, "M 0 0 L 0 -10 L 5 -10");
    }

    #[test]
    fn names_collect_products() {
        let f = folded("draw: move(0, 0) right(40):flat :station down(10) circle(4):bore;");
        let get = |n: &str| {
            f.names
                .iter()
                .find(|(name, _)| name == n)
                .map(|(_, p)| *p)
                .expect("named")
        };
        assert_eq!(get("flat"), Product::Edge((0.0, 0.0), (40.0, 0.0)));
        assert_eq!(get("station"), Product::Point((40.0, 0.0)));
        assert_eq!(
            get("bore"),
            Product::Circle {
                center: (40.0, 10.0),
                r: 4.0
            }
        );
    }

    #[test]
    fn chamfer_trims_both_legs() {
        let f = folded("draw: move(0, 0) right(20) chamfer(5) down(20);");
        assert_eq!(f.d, "M 0 0 L 15 0 L 20 5 L 20 20");
    }

    #[test]
    fn fillet_drops_a_tangent_arc() {
        // A square corner: trim = r, quarter arc, clockwise turn (right→down).
        let f = folded("draw: move(0, 0) right(20) fillet(5) down(20);");
        assert_eq!(f.d, "M 0 0 L 15 0 A 5 5 0 0 1 20 5 L 20 20");
    }

    #[test]
    fn cyclic_fillet_rounds_through_close() {
        // fillet(4) close() rounds the last-to-seam corner; close() fillet(4)
        // would round seam-to-first the same way [SPEC 15.3].
        let f = folded("draw: move(0, 0) right(20) down(20) left(20) fillet(4) close();");
        assert!(f.d.contains("A 4 4"), "seam corner rounded: {}", f.d);
        let g = folded("draw: move(0, 0) right(20) down(20) left(20) close() fillet(4);");
        assert!(g.d.contains("A 4 4"), "first corner rounded: {}", g.d);
    }

    #[test]
    fn tangent_arc_turns_the_heading() {
        // Heading right, 90° clockwise on r=10: quarter turn to heading down.
        let f = folded("draw: move(0, 0) right(10) arc(10, 90) right(5);");
        // After the turn the pen heads down; the trailing right(5) drew from
        // the arc's end (20, 10).
        assert_eq!(f.d, "M 0 0 L 10 0 A 10 10 0 0 1 20 10 L 25 10");
    }

    #[test]
    fn relative_arc_picks_the_sweep_by_sign() {
        let cw = folded("draw: move(0, 0) arc(10, 0, 5);");
        assert_eq!(cw.d, "M 0 0 A 5 5 0 0 1 10 0");
        let ccw = folded("draw: move(0, 0) arc(10, 0, -5);");
        assert_eq!(ccw.d, "M 0 0 A 5 5 0 0 0 10 0");
    }

    #[test]
    fn open_subpath_fuses_under_mirror() {
        // A half profile off the axis on one end: fused whole, one closed
        // subpath, with a seam segment at the off-axis end.
        let f = folded("draw: move(-10, 0) up(5) right(20) down(5); mirror: x-axis;");
        assert!(f.d.ends_with("Z"), "fused = closed: {}", f.d);
        assert_eq!(f.d.matches('M').count(), 1, "one fused subpath: {}", f.d);
        // The reflected walk-back visits (10, 5) and (-10, 5).
        assert!(f.d.contains("L 10 5") && f.d.contains("L -10 5"), "{}", f.d);
        assert_eq!(
            (f.geometry.min_y, f.geometry.max_y),
            (-5.0, 5.0),
            "symmetric about the axis"
        );
    }

    #[test]
    fn closed_subpath_duplicates_under_mirror() {
        let f = folded("draw: move(0, -10) circle(3); mirror: x-axis;");
        assert_eq!(
            f.d.matches('M').count(),
            2,
            "seed + reflected copy: {}",
            f.d
        );
        assert_eq!((f.geometry.min_y, f.geometry.max_y), (-13.0, 13.0));
    }

    #[test]
    fn fold_errors_speak_spec() {
        assert!(fold_err("draw: right(10);").contains("starts with move"));
        assert!(fold_err("draw: move(0, 0) wiggle(3);").contains("unknown draw call 'wiggle'"));
        assert!(
            fold_err("draw: move(0, 0) arc(100, 0, 2);").contains("smaller than half the chord")
        );
        assert!(
            fold_err("draw: move(0, 0) fillet(3) right(5);")
                .contains("corner between two segments")
        );
        assert!(fold_err("draw: move(0, 0) right(5) fillet(9) down(5);").contains("does not fit"));
        assert!(fold_err("draw: move(0, 0) right(5):left;").contains("built-in anchor"));
        assert!(fold_err("draw: move(0, 0) right(5):a up(2):a;").contains("already named"));
        assert!(fold_err("draw: move(0, 0) arc(4, 90);").contains("continues a heading"));
        assert!(fold_err("draw: move(0, 0):spot right(4);").contains("no product name"));
        assert!(
            fold_err("draw: move(0, 0) right(4); mirror: sideways;")
                .contains("x-axis, y-axis, or a bearing")
        );
    }
}

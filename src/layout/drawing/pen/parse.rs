//! The sketch pen's fold state machine [SPEC 15.3]: the `Pen` that interprets each `draw:` call into subpaths, plus the `mirror:` / `revolve:` axis parsers.

use super::*;

/// `revolve:` → its axis [SPEC 15.3]: `x-axis` or `y-axis`, nothing else — a
/// lathe turns about a cardinal axis of its profile.
pub(super) fn parse_revolve(v: &ResolvedValue, span: Span) -> Result<MirrorAxis, Error> {
    match v {
        ResolvedValue::Ident(s) if s == "x-axis" => Ok(MirrorAxis { bearing: 90.0 }),
        ResolvedValue::Ident(s) if s == "y-axis" => Ok(MirrorAxis { bearing: 0.0 }),
        _ => Err(Error::at(span, "'revolve' takes x-axis or y-axis")),
    }
}

/// The built-in anchor names an authored `:segment` may not shadow [SPEC 15.2].
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
pub(super) fn parse_mirror(v: &ResolvedValue, span: Span) -> Result<Vec<MirrorAxis>, Error> {
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
        // `mirror:` is a pipeline — its items reflect the union so far, in one
        // space-separated run [SPEC 2/15.3].
        ResolvedValue::List(_) => Err(Error::at(
            span,
            "'mirror' is one space-separated run of reflections — 'mirror: x-axis 45'",
        )),
        ResolvedValue::Tuple(items) => items.iter().map(one).collect(),
        item => Ok(vec![one(item)?]),
    }
}

/// Authored segments by name, in source order.
pub(super) type Segments = Vec<(String, Segment)>;

/// The fold state machine: position, heading, the open subpath, a parked
/// corner modifier, and the finished subpaths.
pub(super) struct Pen {
    span: Span,
    subs: Vec<Subpath>,
    cur: Vec<PathSeg>,
    start: Option<P>,
    pos: P,
    /// Bearing after the last drawing call; `angle()` sets it absolutely, the
    /// tangent `arc(r, deg)` reads and turns it.
    heading: Option<f64>,
    pending: Option<(Mod, Option<String>)>,
    /// The subpath just closed by `close()` — a modifier right after it rounds
    /// the seam-to-first corner (the cyclic case, [SPEC 15.3]).
    just_closed: bool,
    segments: Vec<(String, Segment)>,
}

impl Pen {
    pub(super) fn new(span: Span) -> Self {
        Pen {
            span,
            subs: Vec::new(),
            cur: Vec::new(),
            start: None,
            pos: (0.0, 0.0),
            heading: None,
            pending: None,
            just_closed: false,
            segments: Vec::new(),
        }
    }

    fn err(&self, msg: impl Into<String>) -> Error {
        Error::at(self.span, msg.into())
    }

    fn segment(&mut self, name: &str, segment: Segment) -> Result<(), Error> {
        if is_builtin_point(name) {
            return Err(self.err(format!(
                "':{name}' is a built-in anchor — pick another name"
            )));
        }
        if self.segments.iter().any(|(n, _)| n == name) {
            return Err(self.err(format!("':{name}' is already named in this 'draw:'")));
        }
        self.segments.push((name.to_string(), segment));
        Ok(())
    }

    pub(super) fn call(&mut self, call: &ResolvedCall, segment: Option<&str>) -> Result<(), Error> {
        // Only the corner modifiers (and a name / new subpath) may follow a
        // close() — the pen returned to the seam [SPEC 15.3].
        if self.just_closed
            && !matches!(
                call.name.as_str(),
                "fillet" | "chamfer" | "move" | "circle" | "point"
            )
        {
            return Err(self.err("after close(), start the next subpath with move()"));
        }
        match call.name.as_str() {
            "move" => {
                let [x, y] = self.nums::<2>(call, "'move' takes (x, y)")?;
                if segment.is_some() {
                    return Err(
                        self.err("'move' takes no segment — name its landing with point():name")
                    );
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
                self.run(bearing_scaled(bearing, len), Some(bearing), segment)?;
            }
            "line" => {
                let [dx, dy] = self.nums::<2>(call, "'line' takes (dx, dy)")?;
                self.run((dx, dy), Some(dir_bearing((dx, dy))), segment)?;
            }
            "angle" => {
                let [deg, len] = self.nums::<2>(call, "'angle' takes (deg, n)")?;
                self.run(bearing_scaled(deg, len), Some(deg), segment)?;
            }
            "arc" => match call.args.len() {
                3 => {
                    let [dx, dy, r] =
                        self.nums::<3>(call, "'arc' takes (dx, dy, r) or (r, deg)")?;
                    self.arc_to((dx, dy), r, segment)?;
                }
                2 => {
                    let [r, deg] = self.nums::<2>(call, "'arc' takes (dx, dy, r) or (r, deg)")?;
                    self.arc_turn(r, deg, segment)?;
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
                self.push_seg(PathSeg::Cubic { from, c1, c2, to })?;
                let tangent = (to.0 - c2.0, to.1 - c2.1);
                if dist(tangent, (0.0, 0.0)) > 1e-9 {
                    self.heading = Some(dir_bearing(tangent));
                }
                self.pos = to;
                if let Some(nm) = segment {
                    self.segment(nm, Segment::Edge(from, to))?;
                }
            }
            "point" => {
                // A station [SPEC 15.3]: record the pen's current point under
                // the attached `:segment` — draws nothing, changes nothing;
                // beside a pending `fillet` / `chamfer` (either order) the
                // position is still the theoretical sharp corner, the point
                // drafting measures.
                self.nums::<0>(call, "'point()' takes no arguments")?;
                if self.start.is_none() && !self.just_closed {
                    return Err(self.err("the pen starts with move(x, y)"));
                }
                let Some(nm) = segment else {
                    return Err(
                        self.err("'point()' names the pen's position — attach a ':segment'")
                    );
                };
                self.segment(nm, Segment::Point(self.pos))?;
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
                    let name = self.apply_cyclic(m, segment)?;
                    if let Some((nm, p)) = name {
                        self.segment(&nm, p)?;
                    }
                } else {
                    if self.cur.is_empty() {
                        return Err(self.err(msg));
                    }
                    self.pending = Some((m, segment.map(str::to_string)));
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
                        PathSeg::Arc {
                            from: w,
                            to: e,
                            r,
                            large: false,
                            sweep: true,
                        },
                        PathSeg::Arc {
                            from: e,
                            to: w,
                            r,
                            large: false,
                            sweep: true,
                        },
                    ],
                    closed: true,
                });
                if let Some(nm) = segment {
                    self.segment(
                        nm,
                        Segment::Circle {
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
                let seam = PathSeg::Line {
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
                if let Some(nm) = segment {
                    self.segment(nm, Segment::Edge(self.pos, start))?;
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
    fn run(&mut self, delta: P, bearing: Option<f64>, segment: Option<&str>) -> Result<(), Error> {
        let from = self.started()?;
        let to = (from.0 + delta.0, from.1 + delta.1);
        self.push_seg(PathSeg::Line { from, to })?;
        self.pos = to;
        if bearing.is_some() {
            self.heading = bearing;
        }
        if let Some(nm) = segment {
            self.segment(nm, Segment::Edge(from, to))?;
        }
        Ok(())
    }

    /// `arc(dx, dy, r)` — the minor arc to a relative point; `r > 0` sweeps
    /// clockwise; `|r|` at least half the chord [SPEC 15.3].
    fn arc_to(&mut self, delta: P, r: f64, segment: Option<&str>) -> Result<(), Error> {
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
        let m = ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0);
        let centre = geometry::arc_center(from, to, ra, false, sweep);
        self.push_seg(PathSeg::Arc {
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
        if let Some(nm) = segment {
            let mid = arc_mid(centre, m, ra, from, sweep);
            self.segment(nm, Segment::Arc { mid, r: ra })?;
        }
        Ok(())
    }

    /// `arc(r, deg)` — a tangent arc: continue the heading, sweep `deg`
    /// (positive turns clockwise); the heading updates by `deg` [SPEC 15.3].
    fn arc_turn(&mut self, r: f64, deg: f64, segment: Option<&str>) -> Result<(), Error> {
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
        self.push_seg(PathSeg::Arc {
            from,
            to,
            r,
            large: deg.abs() > 180.0,
            sweep: cw,
        })?;
        self.pos = to;
        self.heading = Some((heading + deg).rem_euclid(360.0));
        if let Some(nm) = segment {
            let mid = rotate_about(from, centre, deg / 2.0);
            self.segment(nm, Segment::Arc { mid, r })?;
        }
        Ok(())
    }

    /// Append a segment, applying any parked corner modifier between the
    /// previous segment and this one.
    fn push_seg(&mut self, seg: PathSeg) -> Result<(), Error> {
        self.just_closed = false;
        let seg = match self.pending.take() {
            None => seg,
            Some((m, name)) => {
                let prev = self.cur.pop().expect("pending implies a previous segment");
                let (prev, mid, next, segment) = apply_mod(m, prev, seg, self.span)?;
                self.cur.push(prev);
                self.cur.push(mid);
                if let Some(nm) = name {
                    self.segment(&nm, segment)?;
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
        segment: Option<&str>,
    ) -> Result<Option<(String, Segment)>, Error> {
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
        Ok(segment.map(|nm| (nm.to_string(), prod)))
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

    pub(super) fn finish(mut self) -> Result<(Vec<Subpath>, Segments), Error> {
        self.flush()?;
        if self.subs.iter().all(|s| s.segs.is_empty()) {
            return Err(self.err("'draw' draws nothing — add a pen run"));
        }
        Ok((self.subs, self.segments))
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

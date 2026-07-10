//! `thread:` — dress a threaded surface [SPEC 15.3]. The numbers live once:
//! the surface gives the major `⌀`, the property the pitch, and the chrome
//! follows — the thin **minor line** offset into the material by the ISO 60°
//! thread depth (0.6134 × pitch), running the segment's drawn extent (a
//! `chamfer()`'s trim already ends it); the **thread-end line**, geometry
//! weight across the full diameter, at an end where the surface continues
//! collinearly past the run (where the profile turns, the geometry already
//! ends the thread). Both are doubled about the revolve axis.

use super::geometry::{MirrorAxis, P, PathSeg, Subpath};
use super::{Segment, breaks::ViewMap};
use crate::error::Error;
use crate::resolve::{ResolvedInst, ResolvedValue};
use crate::span::Span;

/// The ISO metric 60° thread depth per side, as a fraction of the pitch —
/// external `d3 = d − 1.2269 × P` [SPEC 15.3].
pub(super) const THREAD_DEPTH: f64 = 0.61343;
/// Positional agreement, px — matches the edge-line law's.
const EPS: f64 = 1e-3;

/// A dressed profile's thread chrome, in displayed (scaled, break-mapped)
/// coordinates, plus the specs the smart leader composes from.
pub(super) struct Dressing {
    /// The minor-line spans — the `|threadline|` chrome's geometry.
    pub minors: Vec<(P, P)>,
    /// The thread-end lines — real edges, joining the `|shoulder|` spans.
    pub ends: Vec<(P, P)>,
    /// `(segment name, pitch)` per group, pitch in drawing units — the
    /// bare leader reads these [SPEC 15.7].
    pub specs: Vec<(String, f64)>,
}

/// Parse and dress every `thread:` group against the folded profile.
pub(super) fn dress(
    inst: &ResolvedInst,
    segments: &[(String, Segment)],
    subs: &[Subpath],
    revolve: Option<MirrorAxis>,
    view: &ViewMap,
    scale: f64,
    span: Span,
) -> Result<Dressing, Error> {
    let mut out = Dressing {
        minors: Vec::new(),
        ends: Vec::new(),
        specs: Vec::new(),
    };
    let Some(v) = inst.attrs.get("thread") else {
        return Ok(out);
    };
    let Some(axis) = revolve else {
        return Err(Error::at(
            span,
            "'thread' dresses a revolved profile — add 'revolve: x-axis'",
        ));
    };
    for (name, pitch) in parse(v, span)? {
        let seg = find_segment(&name, segments, span)?;
        let (a, b) = match seg {
            Segment::Edge(a, b) if parallel(a, b, axis) => (a, b),
            _ => {
                return Err(Error::at(
                    span,
                    format!(
                        "'thread' runs along the axis — '{name}' must be a straight run parallel to it"
                    ),
                ));
            }
        };
        out.specs.push((name, pitch));

        let u = axis.dir();
        let perp = (-u.1, u.0);
        let station = |p: P| p.0 * u.0 + p.1 * u.1;
        let off = |p: P| p.0 * perp.0 + p.1 * perp.1;
        // The authored run, at its displayed stations (a break slides pieces).
        let (da, db) = (view.map(a), view.map(b));
        let (lo, hi) = order(station(da), station(db));
        let level = off(a);

        // The drawn extent: the profile's collinear segments clipped to the
        // run — a chamfer's trim already shortened them [SPEC 15.3].
        let mut drawn: Option<(f64, f64)> = None;
        let mut continues = [false, false]; // past lo, past hi
        for sub in subs {
            for seg in &sub.segs {
                let PathSeg::Line { from, to } = *seg else {
                    continue;
                };
                if (off(from) - level).abs() > EPS || (off(to) - level).abs() > EPS {
                    continue;
                }
                let (s0, s1) = order(station(from), station(to));
                if s0 < lo - EPS && s1 > lo - EPS {
                    continues[0] = true;
                }
                if s1 > hi + EPS && s0 < hi + EPS {
                    continues[1] = true;
                }
                let (c0, c1) = (s0.max(lo), s1.min(hi));
                if c1 - c0 > EPS {
                    drawn = Some(match drawn {
                        Some((d0, d1)) => (d0.min(c0), d1.max(c1)),
                        None => (c0, c1),
                    });
                }
            }
        }
        let (t0, t1) = drawn.unwrap_or((lo, hi));

        // The minor line, offset toward the axis by the thread depth, on
        // both sides of it — the revolve's doubling.
        let depth = THREAD_DEPTH * pitch * scale;
        let minor = level - level.signum() * depth;
        let line = |o: f64| -> (P, P) {
            (
                (u.0 * t0 + perp.0 * o, u.1 * t0 + perp.1 * o),
                (u.0 * t1 + perp.0 * o, u.1 * t1 + perp.1 * o),
            )
        };
        out.minors.push(line(minor));
        out.minors.push(line(-minor));

        // The thread-end line where the surface continues past the run.
        let radius = level.abs();
        for (i, s) in [(0, lo), (1, hi)] {
            if continues[i] {
                out.ends.push((
                    (u.0 * s - perp.0 * radius, u.1 * s - perp.1 * radius),
                    (u.0 * s + perp.0 * radius, u.1 * s + perp.1 * radius),
                ));
            }
        }
    }
    Ok(out)
}

/// `thread: seg pitch` groups — the segment name bare, the pitch > 0.
fn parse(v: &ResolvedValue, span: Span) -> Result<Vec<(String, f64)>, Error> {
    let bad = || {
        Error::at(
            span,
            "'thread' takes a segment and its pitch — 'thread: m8 1.5'",
        )
    };
    let one = |item: &ResolvedValue| -> Result<(String, f64), Error> {
        let ResolvedValue::Tuple(parts) = item else {
            return Err(bad());
        };
        let [ResolvedValue::Ident(name), pitch] = parts.as_slice() else {
            return Err(bad());
        };
        match pitch.as_number() {
            Some(p) if p > 0.0 => Ok((name.clone(), p)),
            _ => Err(bad()),
        }
    };
    match v {
        ResolvedValue::List(items) => items.iter().map(one).collect(),
        item => Ok(vec![one(item)?]),
    }
}

/// The named segment, with the anchors' did-you-mean shape on a miss.
fn find_segment(name: &str, segments: &[(String, Segment)], span: Span) -> Result<Segment, Error> {
    if let Some((_, seg)) = segments.iter().find(|(n, _)| n == name) {
        return Ok(*seg);
    }
    let mut msg = format!("no segment '{name}' in this 'draw:'");
    let near = crate::suggest::nearest(name, segments.iter().map(|(n, _)| n.as_str()), 2);
    msg.push_str(&crate::suggest::did_you_mean(&near));
    Err(Error::at(span, msg))
}

fn parallel(a: P, b: P, axis: MirrorAxis) -> bool {
    let d = (b.0 - a.0, b.1 - a.1);
    let len = d.0.hypot(d.1);
    if len < 1e-9 {
        return false;
    }
    let u = axis.dir();
    ((d.0 * u.0 + d.1 * u.1) / len).abs() > 1.0 - 1e-9
}

fn order(a: f64, b: f64) -> (f64, f64) {
    if a <= b { (a, b) } else { (b, a) }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{by_id, laid, layout_err, texts};
    use super::*;
    use crate::resolve::{NodeKind, ResolvedValue};

    /// The tie bar at scale 1: thread on the ⌀20 left run, chamfered ends.
    const BAR: &str = "|sketch#bar| { draw: move(-150, 0) up(10) chamfer(1.5) right(40):m20 point():a right(260) chamfer(1.5) down(10); revolve: x-axis; thread: m20 1.5 }\n";

    fn lines_of(nodes: &[crate::layout::PlacedNode], id: &str, ty: &str) -> Vec<(P, P)> {
        by_id(nodes, id)
            .children
            .iter()
            .filter(|c| c.type_chain.iter().any(|t| t == ty))
            .filter_map(|c| {
                let ResolvedValue::List(pts) = c.attrs.get("points")? else {
                    return None;
                };
                let p = |v: &ResolvedValue| {
                    let ResolvedValue::Tuple(xy) = v else {
                        panic!("a point");
                    };
                    (xy[0].as_number().unwrap(), xy[1].as_number().unwrap())
                };
                Some((p(&pts[0]), p(&pts[1])))
            })
            .collect()
    }

    #[test]
    fn a_thread_draws_its_minors_and_end_line() {
        let l = laid(&format!("{{ layout: drawing; scale: 1 }}\n{BAR}"));
        // Two minor lines at ±(10 − 0.61343·1.5) ≈ ±9.08, spanning the drawn
        // run: from the chamfer's trim (−148.5) to the authored end (−110).
        let minors = lines_of(&l.nodes, "bar", "threadline");
        assert_eq!(minors.len(), 2, "{minors:?}");
        let m = 10.0 - THREAD_DEPTH * 1.5;
        for (a, b) in &minors {
            assert!((a.0 - -148.5).abs() < 1e-6 && (b.0 - -110.0).abs() < 1e-6);
            assert!((a.1.abs() - m).abs() < 1e-6, "minor level: {}", a.1);
        }
        // The thread-end line at −110 — the surface continues collinearly —
        // rides the |shoulder| chrome at full diameter; the chamfered entry
        // end draws none.
        let edges = lines_of(&l.nodes, "bar", "shoulder");
        assert!(
            edges.iter().any(|(a, b)| (a.0 - -110.0).abs() < 1e-6
                && (a.1 - -10.0).abs() < 1e-6
                && (b.1 - 10.0).abs() < 1e-6),
            "thread-end line at the run's inner end: {edges:?}"
        );
        assert!(
            !edges.iter().any(|(a, _)| (a.0 - -150.0).abs() < 1e-6),
            "no end line at the chamfered entry: {edges:?}"
        );
    }

    #[test]
    fn a_bare_leader_composes_the_thread_spec() {
        let l = laid(&format!(
            "{{ layout: drawing; scale: 1 }}\n{BAR}bar:m20 <- {{ side: top }}\n"
        ));
        assert!(
            texts(&l.nodes).iter().any(|(t, ..)| t == "M20×1.5"),
            "composed spec: {:?}",
            texts(&l.nodes)
        );
    }

    #[test]
    fn a_bare_leader_off_a_thread_still_needs_its_text() {
        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|rect#a| { width: 10 }\na <- { side: top }\n"
            ),
            "a leader needs its text — 'bolt <- \"THRU\"'"
        );
    }

    #[test]
    fn a_hole_thread_draws_the_internal_arc_and_an_oval_the_external() {
        let l = laid(
            "{ layout: drawing; scale: 2 }\n|rect#plate| { width: 60; height: 40 } [\n  |hole#tap| { width: 6.75; thread: 1 }\n]\n",
        );
        let arc = by_id(&l.nodes, "tap")
            .children
            .iter()
            .find(|c| c.kind == NodeKind::Path)
            .expect("the ¾ arc");
        // Internal: major r = drawn r + 0.54125 × pitch × scale.
        let r = 6.75 + 0.54125 * 1.0 * 2.0;
        assert!(
            (arc.bbox.w() / 2.0 - (r + 0.5)).abs() < 1e-6,
            "major radius + half stroke: {}",
            arc.bbox.w() / 2.0
        );
        let ext = laid(
            "{ layout: drawing; scale: 2 }\n|oval#stud| { width: 8; fill: none; thread: 1.25 }\n",
        );
        let arc = by_id(&ext.nodes, "stud")
            .children
            .iter()
            .find(|c| c.kind == NodeKind::Path)
            .expect("the ¾ arc");
        let r = 8.0 - THREAD_DEPTH * 1.25 * 2.0;
        assert!(
            (arc.bbox.w() / 2.0 - (r + 0.5)).abs() < 1e-6,
            "minor radius + half stroke: {}",
            arc.bbox.w() / 2.0
        );
    }

    #[test]
    fn thread_errors_follow_spec_20() {
        let sketch = |style: &str| {
            format!(
                "{{ layout: drawing; scale: 1 }}\n|sketch#s| {{ draw: move(-20, 0) up(5) right(40):m8 down(5); {style} }}\n"
            )
        };
        assert_eq!(
            layout_err(&sketch("revolve: x-axis; thread: m8")),
            "'thread' takes a segment and its pitch — 'thread: m8 1.5'"
        );
        assert_eq!(
            layout_err(&sketch("mirror: x-axis; thread: m8 1.5")),
            "'thread' dresses a revolved profile — add 'revolve: x-axis'"
        );
        assert_eq!(
            layout_err(&sketch("revolve: x-axis; thread: m9 1.5")),
            "no segment 'm9' in this 'draw:'; did you mean 'm8'?"
        );
        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|sketch#s| { draw: move(-20, 0) up(5):wall right(40) down(5); revolve: x-axis; thread: wall 1.5 }\n"
            ),
            "'thread' runs along the axis — 'wall' must be a straight run parallel to it"
        );
        assert_eq!(
            layout_err("{ layout: drawing }\n|rect#a| { width: 10; thread: 1.5 }\n"),
            "'thread' dresses a '|sketch|' segment or a round feature"
        );
    }
}

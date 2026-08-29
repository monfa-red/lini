//! Openings [SPEC 15.11]: a `|door|` / `|window|` riding its wall's `[ ]`,
//! **stationed** on a straight named `:segment` — `on:` names it, `at:` the
//! near jamb's distance from the segment's *draw* start, `width:` the clear
//! opening (900 mm / 1200 mm true-size, stamped at desugar).
//!
//! Three things come out of one pass over the wall's `[ ]`, so the station is
//! computed exactly once:
//!
//! 1. **The gap** — the interval the wall's outline is clipped over. It is a
//!    profile clip, not a `break:`: the wall keeps its length, and each jamb
//!    closes flat across the thickness. [`wall`](super::wall) cuts the
//!    centreline run at the stations and offsets each piece as its own **open**
//!    run, so a jamb cap *is* the open-run butt cap — one construction, not two.
//! 2. **The opening's own geometry** — the jamb-to-jamb box (`width` ×
//!    `thickness`), seated on the segment and turned with it, so a dimension
//!    anchors at its centre and the location chain reads.
//! 3. **The chrome** — the generated leaf / swing arc / sill children
//!    ([SPEC 15.7]) filled in the opening's own frame: `+x` runs along the
//!    segment's travel, `+y` is the **right** of that travel (the named-edge
//!    convention's other side, [SPEC 15.5]), so `swing: left` opens toward
//!    `−y` at every bearing with no second walker.

use super::super::drawing::Segment;
use super::super::drawing::geometry::{P, dist, n};
use super::super::drawing::pen::Folded;
use super::super::ir::{Bbox, PlacedNode};
use super::{FpKind, fp_kind};
use crate::desugar::scale::{DOOR_MM, OPENING_WIDTH, WINDOW_MM};
use crate::error::{Code, Error};
use crate::layout::geom::unit;
use crate::math;
use crate::resolve::{NodeKind, ResolvedInst, ResolvedValue};
use crate::suggest;

/// How wide the gap must be before the two ends of a station are distinct.
const EPS: f64 = 1e-6;

/// One opening's resolved station on its wall.
pub(super) struct Station {
    /// The opening's index among the wall's children — placed and resolved
    /// share it (layout builds one per resolved child, in order).
    child: usize,
    /// Which folded centreline subpath and which of its segments the gap cuts.
    sub: usize,
    seg: usize,
    /// The jamb distances along that segment, px from its draw start.
    near: f64,
    far: f64,
    /// The gap centre in the wall's frame, and the segment's bearing — the
    /// opening's placed origin and turn.
    origin: P,
    bearing: f64,
    width: f64,
}

impl Station {
    pub(super) fn on_subpath(&self, sub: usize) -> bool {
        self.sub == sub
    }
}

/// Resolve every opening in a wall's `[ ]` against its folded centreline
/// [SPEC 15.11] — the station laws ([SPEC 21]: unknown or curved segment,
/// overrun, overlap) all read folded geometry, so they live here, one law per
/// clause.
pub(super) fn plan(inst: &ResolvedInst, folded: &Folded, own: f64) -> Result<Vec<Station>, Error> {
    let mut out: Vec<Station> = Vec::new();
    for (child, node) in inst.children.iter().enumerate() {
        if fp_kind(&node.type_chain) != Some(FpKind::Opening) {
            continue;
        }
        // The gate already proved `on:` is there; a malformed value is
        // validation's to report and stations nothing.
        let Some(ResolvedValue::Ident(name)) = node.attrs.get("on") else {
            continue;
        };
        let (p0, p1) = straight_run(folded, name, node.span)?;
        let Some((sub, seg)) = locate(folded, p0, p1) else {
            continue;
        };
        let len = dist(p0, p1);
        let near = node.attrs.number("at").unwrap_or(0.0) * own;
        let far = near + width_of(node) * own;
        if near < -EPS || far > len + EPS {
            return Err(Error::at(
                node.span,
                format!(
                    "'{}' at {} + width {} overruns '{name}' (length {})",
                    who(node),
                    n(near / own),
                    n((far - near) / own),
                    n(len / own)
                ),
            )
            .code(Code::OPENING_OVERRUN));
        }
        if let Some(prev) = out
            .iter()
            .find(|s| s.sub == sub && s.seg == seg && s.near < far - EPS && near < s.far - EPS)
        {
            return Err(Error::at(
                node.span,
                format!(
                    "'{}' and '{}' overlap on '{name}'",
                    who(&inst.children[prev.child]),
                    who(node)
                ),
            )
            .code(Code::OPENING_OVERLAP));
        }
        let d = unit((p1.0 - p0.0, p1.1 - p0.1)).expect("a named edge has length");
        let mid = (near + far) / 2.0;
        out.push(Station {
            child,
            sub,
            seg,
            near,
            far,
            origin: (p0.0 + d.0 * mid, p0.1 + d.1 * mid),
            bearing: math::atan2(d.1, d.0).to_degrees(),
            width: far - near,
        });
    }
    Ok(out)
}

/// The gap intervals cutting one folded segment, near-jamb first.
pub(super) fn gaps(stations: &[Station], sub: usize, seg: usize) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = stations
        .iter()
        .filter(|s| s.sub == sub && s.seg == seg)
        .map(|s| (s.near, s.far))
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

/// Seat every opening on its station [SPEC 15.11]: the jamb-to-jamb box is its
/// geometry, the segment's bearing its turn, and its chrome fills in that frame.
pub(super) fn place(children: &mut [PlacedNode], stations: &[Station], thickness: f64) {
    for st in stations {
        let node = &mut children[st.child];
        node.cx = st.origin.0;
        node.cy = st.origin.1;
        node.rotation = st.bearing;
        node.bbox = Bbox::centered(st.width, thickness);
        chrome(node, st.width, thickness);
        // The schedule tag stands **beside** the gap [SPEC 15.11], on the face
        // the leaf never sweeps — the fixture label's own seat, shared.
        let (_, swing) = pose(node);
        super::label::seat(&mut node.children, thickness / 2.0, -swing);
    }
}

/// The named segment an opening stations on, as its two endpoints — the two
/// halves of [SPEC 21]'s `on:` row: a name the wall never drew, and a run that
/// is not straight.
fn straight_run(folded: &Folded, name: &str, span: crate::span::Span) -> Result<(P, P), Error> {
    let Some((_, seg)) = folded.segments.iter().find(|(n, _)| n == name) else {
        let near = suggest::nearest(name, folded.segments.iter().map(|(n, _)| n.as_str()), 1);
        return Err(Error::at(
            span,
            format!(
                "'{name}' is not a segment of this wall{}",
                suggest::did_you_mean(&near)
            ),
        )
        .code(Code::OPENING_SEGMENT));
    };
    match *seg {
        Segment::Edge(a, b) => Ok((a, b)),
        Segment::Point(_) => Err(curved(name, "a point", span)),
        _ => Err(curved(name, "an arc", span)),
    }
}

fn curved(name: &str, what: &str, span: crate::span::Span) -> Error {
    Error::at(
        span,
        format!("an opening sits on a straight run — ':{name}' is {what}"),
    )
    .code(Code::OPENING_SEGMENT)
}

/// Which folded segment carries a named edge — the pen names the centreline,
/// the offset cuts the folded run, and this is the one place they meet.
fn locate(folded: &Folded, p0: P, p1: P) -> Option<(usize, usize)> {
    folded.subs.iter().enumerate().find_map(|(i, sub)| {
        sub.segs
            .iter()
            .position(|s| {
                matches!(s, crate::layout::drawing::geometry::PathSeg::Line { .. })
                    && dist(s.from(), p0) < EPS
                    && dist(s.to(), p1) < EPS
            })
            .map(|j| (i, j))
    })
}

/// An opening's clear width in drawing units [SPEC 15.11]: a cascaded `width:`
/// (authored or rule-borne) first, else the desugar-stamped true-size fallback.
/// The raw-mm constant only guards a tree desugar never walked.
fn width_of(node: &ResolvedInst) -> f64 {
    node.attrs
        .number("width")
        .or_else(|| node.attrs.number(OPENING_WIDTH))
        .unwrap_or(if node.type_chain.iter().any(|t| t == "window") {
            WINDOW_MM
        } else {
            DOOR_MM
        })
}

/// How an opening names itself in an error — its id, else its written type.
fn who(node: &ResolvedInst) -> String {
    match &node.id {
        Some(id) => id.clone(),
        None => format!(
            "|{}|",
            crate::desugar::classes::written_type(&node.type_chain).unwrap_or("door")
        ),
    }
}

// ── The chrome [SPEC 15.11] ──

/// A door's pose: which jamb hangs the leaf, and which side it opens toward.
/// `hinge: start` is the segment's draw start (`−x` here); `swing: left` is the
/// left of the pen's travel, which in this frame is `−y` [SPEC 15.5].
fn pose(node: &PlacedNode) -> (f64, f64) {
    let ident = |name: &str, word: &str| matches!(node.attrs.get(name), Some(ResolvedValue::Ident(s)) if s == word);
    let hinge = if ident("hinge", "end") { 1.0 } else { -1.0 };
    let swing = if ident("swing", "right") { 1.0 } else { -1.0 };
    (hinge, swing)
}

/// Fill the generated children in the opening's own frame: a door's leaf +
/// quarter swing arc (a `double` halves both about the gap centre, a `sliding`
/// trades the arc for a second panel), a window's two sill lines.
fn chrome(node: &mut PlacedNode, width: f64, thickness: f64) {
    let (hinge, swing) = pose(node);
    // Two leaves *are* the `double` symbol: the count is the one desugar
    // emitted, never re-read off `symbol:` here — the marker mechanism's law
    // ([`crate::layout::drawing::chrome`]), and what keeps a rule-borne
    // `symbol:` from drawing half a door.
    let double = node
        .children
        .iter()
        .filter(|c| marker(c).is_some_and(|(kind, _)| kind == "leaf"))
        .count()
        > 1;
    let (half, face) = (width / 2.0, swing * thickness / 2.0);
    // Where each leaf hangs and how far it reaches: one hinged leaf the full
    // clear width, or — `double` — a half-width leaf off each jamb.
    let leaf = |i: f64| {
        if double {
            let pivot = if i == 0.0 { -half } else { half };
            (pivot, half, 0.0)
        } else {
            (hinge * half, width, -hinge * half)
        }
    };
    for child in node.children.iter_mut() {
        let Some((kind, i)) = marker(child) else {
            continue;
        };
        match kind.as_str() {
            // The leaf stands 90° open on the swing-side face, hinged at its
            // jamb…
            "leaf" => {
                let (pivot, reach, _) = leaf(i);
                line(child, (pivot, face), (pivot, face + swing * reach));
            }
            // …and its quarter arc sweeps that leaf back to closed — radius
            // the leaf's own length, landing flat on the same face.
            "swing" => {
                let (pivot, reach, shut) = leaf(i);
                arc(child, (pivot, face + swing * reach), (shut, face), reach);
            }
            // A slider's two panels: half the gap each, offset to either face,
            // so the pair reads as one set passing the other.
            "panel" => {
                let side = if i == 0.0 { -1.0 } else { 1.0 };
                line(
                    child,
                    (side * half, side * thickness / 2.0),
                    (0.0, side * thickness / 2.0),
                );
            }
            // A window's double-glazing read: two sills across the gap, at the
            // thickness's thirds.
            "sill" => {
                let y = (i - 0.5) * thickness / 3.0;
                line(child, (-half, y), (half, y));
            }
            _ => {}
        }
    }
}

/// The indexed chrome marker on a child, if it wears one — the one reader, so
/// the leaf count and the fill agree by construction.
fn marker(child: &PlacedNode) -> Option<(String, f64)> {
    crate::layout::drawing::chrome::indexed(&child.attrs)
}

fn line(child: &mut PlacedNode, a: P, b: P) {
    let point =
        |p: P| ResolvedValue::Tuple(vec![ResolvedValue::Number(p.0), ResolvedValue::Number(p.1)]);
    child
        .attrs
        .insert("points", ResolvedValue::List(vec![point(a), point(b)]));
    child.bbox = Bbox::from_points(&[a, b]).inflate(child.attrs.half_stroke());
}

/// The quarter swing arc. A `|line|` cannot bend, so the kind flips to a
/// `|path|` — the round-thread play ([`crate::layout::drawing::chrome`]); the
/// sweep flag is read off the turn itself, never a pose table.
fn arc(child: &mut PlacedNode, from: P, to: P, r: f64) {
    // The hinge: the leaf tip's own x, the closed leaf's own y.
    let pivot = (from.0, to.1);
    let u = (from.0 - pivot.0, from.1 - pivot.1);
    let v = (to.0 - pivot.0, to.1 - pivot.1);
    let sweep = u.0 * v.1 - u.1 * v.0 > 0.0;
    child.attrs.insert(
        "path",
        ResolvedValue::String(format!(
            "M {} {} A {} {} 0 0 {} {} {}",
            n(from.0),
            n(from.1),
            n(r),
            n(r),
            u8::from(sweep),
            n(to.0),
            n(to.1)
        )),
    );
    child.kind = NodeKind::Path;
    child.bbox = Bbox::from_points(&[from, to, pivot]).inflate(child.attrs.half_stroke());
}

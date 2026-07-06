//! Drawing anchors [SPEC 15.2], against **seated** placed geometry: a side or
//! corner sits on the node's geometry bbox (stroke excluded), `center` is its
//! centre, no point is the node's **origin** (`cap || barrel` is
//! origin-to-origin), and an authored `:segment` reads what the pen
//! collected. Every anchor reduces to a representative point in the drawing's
//! frame; sides and named edges additionally carry an **outward** unit normal
//! (what a mate seats along, what sets a dimension's axis) and the line-like
//! anchors a direction (what the angle op reads). Rotation is honoured: a
//! part's `rotate:` turns its anchors with it. A `pattern:`ed node's anchors
//! read **one copy's geometry at the pattern's datum** — the point drafting
//! locates [SPEC 15.2/15.4].

use super::super::ir::{Bbox, PlacedNode};
use super::geometry::{MirrorAxis, P, dist};
use super::{Segment, chrome};
use crate::ast::Side;
use crate::error::Error;
use crate::resolve::{ResolvedEndpoint, ResolvedValue};

/// Where an endpoint's anchor sits on its node — resolved against the node's
/// vocabulary, not yet reduced to a point.
pub(super) enum Spot {
    /// No anchor — the node's origin [SPEC 15.1].
    Origin,
    /// A side midpoint — directed, outward along its axis.
    Side(Side),
    /// A bbox corner — carries its outward unit diagonal.
    Corner(P),
    Center,
    /// An authored pen segment, in the node's local frame (scaled).
    Segment(Segment),
}

/// A resolved anchor: the scope-level child it belongs to (what a mate
/// moves), the node the anchor sits on (a feature, after the dot-path walk),
/// that node's accumulated frame in the drawing, and the spot itself.
pub(super) struct Anchor<'a> {
    pub child: usize,
    pub node: &'a PlacedNode,
    /// The node's origin in the drawing frame (rotations accumulated) — the
    /// **displayed** position, a broken parent's compression included.
    pub origin: P,
    /// The origin with every ancestor's `break:` undone [SPEC 15.3] — where
    /// the feature sits on the unbroken model; equals `origin` without one.
    pub model_origin: P,
    /// The accumulated rotation, degrees — local geometry turns by it.
    pub rot: f64,
    pub spot: Spot,
}

/// Resolve an endpoint against the drawing's placed children. `scope` is the
/// drawing's dot-path (`""` at the root); the endpoint's path is scene-rooted.
/// `noun` names the statement kind in errors ("mate", "dimension", "leader").
pub(super) fn resolve<'a>(
    kids: &'a [PlacedNode],
    scope: &str,
    ep: &ResolvedEndpoint,
    noun: &str,
) -> Result<Anchor<'a>, Error> {
    let rel = super::rel_path(&ep.path, scope);
    let mut segs = rel.split('.');
    let first = segs.next().expect("an endpoint path is non-empty");
    let child = kids
        .iter()
        .position(|k| k.id.as_deref() == Some(first))
        .ok_or_else(|| Error::at(ep.span, format!("{noun} endpoint '{rel}' not placed")))?;

    // Walk into features, accumulating origin and rotation — each level renders
    // as translate(cx, cy) rotate(deg), so a parent's turn carries its subtree.
    // A broken parent placed its features through its view map [SPEC 15.3];
    // the model origin unmaps each hop, so values stay true.
    let mut node = &kids[child];
    let mut origin = (node.cx, node.cy);
    let mut model_origin = origin;
    let mut rot = node.rotation;
    for seg in segs {
        let next = node
            .children
            .iter()
            .find(|c| c.id.as_deref() == Some(seg))
            .ok_or_else(|| {
                // The path resolved against the source tree, so the only placed
                // divergence is a pattern's copies [SPEC 15.4/23].
                Error::at(
                    ep.span,
                    format!("'{rel}' sits inside a 'pattern:' — per-copy features are deferred (SPEC 23)"),
                )
            })?;
        let local = rotated((next.cx, next.cy), rot);
        origin = (origin.0 + local.0, origin.1 + local.1);
        let unbroken = match node.sketch.as_ref() {
            Some(geo) if !geo.view.is_identity() => geo.view.unmap((next.cx, next.cy)),
            _ => (next.cx, next.cy),
        };
        let m = rotated(unbroken, rot);
        model_origin = (model_origin.0 + m.0, model_origin.1 + m.1);
        rot += next.rotation;
        node = next;
    }

    let last = rel.rsplit('.').next().expect("non-empty");
    let spot = spot(node, ep, last)?;
    Ok(Anchor {
        child,
        node,
        origin,
        model_origin,
        rot,
        spot,
    })
}

/// The anchor's spot in the node's own frame: the endpoint's side, corner,
/// `center`, authored segment, or (bare) the origin.
fn spot(node: &PlacedNode, ep: &ResolvedEndpoint, node_name: &str) -> Result<Spot, Error> {
    if let Some(side) = ep.side {
        return Ok(Spot::Side(side));
    }
    let Some(point) = &ep.point else {
        return Ok(Spot::Origin);
    };
    let d = std::f64::consts::FRAC_1_SQRT_2;
    match point.as_str() {
        "center" => return Ok(Spot::Center),
        "top-left" => return Ok(Spot::Corner((-d, -d))),
        "top-right" => return Ok(Spot::Corner((d, -d))),
        "bottom-left" => return Ok(Spot::Corner((-d, d))),
        "bottom-right" => return Ok(Spot::Corner((d, d))),
        _ => {}
    }
    // An authored `:segment` [SPEC 15.3]: what the pen collected — model
    // coordinates, mapped here to the **displayed** position (a `break:`
    // slides the kept pieces; without one the map is identity).
    let segments = node
        .sketch
        .as_ref()
        .map(|s| s.segments.as_slice())
        .unwrap_or(&[]);
    let Some((_, segment)) = segments.iter().find(|(n, _)| n == point) else {
        let mut msg = format!("no segment ':{point}' on '{node_name}'");
        let mut near: Vec<&str> = segments.iter().map(|(n, _)| n.as_str()).collect();
        near.sort_by_key(|n| usize::abs_diff(n.len(), point.len()));
        let near: Vec<String> = near.iter().take(2).map(|n| format!("':{n}'")).collect();
        if !near.is_empty() {
            msg.push_str(&format!("; did you mean {}?", near.join(", ")));
        }
        return Err(Error::at(ep.span, msg));
    };
    let view = &node.sketch.as_ref().expect("segments imply a sketch").view;
    Ok(Spot::Segment(view.segment(*segment)))
}

impl Anchor<'_> {
    /// The node whose shape the anchor reads: the node itself — or, on a
    /// `pattern:` carrier, **one copy** (the seed's shape at the datum; a
    /// radial ring reads the same shape about its centre) [SPEC 15.2].
    pub fn feature(&self) -> &PlacedNode {
        if self.node.attrs.get("pattern").is_some()
            && let Some(copy) = self
                .node
                .children
                .iter()
                .find(|c| !chrome::is_chrome(&c.attrs))
        {
            return copy;
        }
        self.node
    }

    /// The node's geometry bbox, local — the drawn shape, stroke excluded
    /// [SPEC 15.1]; one copy's shape for a patterned node.
    pub fn geometry_box(&self) -> Bbox {
        let f = self.feature();
        let half = f.attrs.number("stroke-width").unwrap_or(0.0) / 2.0;
        f.bbox.inflate(-half)
    }

    /// The representative point, node-local [SPEC 15.2]: a point is itself, an
    /// edge or arc its midpoint, a bbox name its bbox point.
    pub fn local_point(&self) -> P {
        let g = self.geometry_box();
        let (cx, cy) = ((g.min_x + g.max_x) / 2.0, (g.min_y + g.max_y) / 2.0);
        match &self.spot {
            Spot::Origin => (0.0, 0.0),
            Spot::Center => (cx, cy),
            Spot::Side(side) => match side {
                Side::Top => (cx, g.min_y),
                Side::Bottom => (cx, g.max_y),
                Side::Left => (g.min_x, cy),
                Side::Right => (g.max_x, cy),
            },
            Spot::Corner((dx, dy)) => (
                if *dx < 0.0 { g.min_x } else { g.max_x },
                if *dy < 0.0 { g.min_y } else { g.max_y },
            ),
            Spot::Segment(p) => match *p {
                Segment::Point(p) => p,
                Segment::Arc { mid, .. } => mid,
                Segment::Circle { center, .. } => center,
                Segment::Edge(a, b) => ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0),
            },
        }
    }

    /// The representative point in the drawing frame — the **displayed**
    /// position (a `break:` compresses the view; [SPEC 15.3]).
    pub fn point(&self) -> P {
        self.to_world(self.local_point())
    }

    /// The representative point with any `break:` undone — the node's own and
    /// every ancestor's — what measured values read: *dimensions stay true*
    /// [SPEC 15.3/15.6].
    pub fn model_point(&self) -> P {
        self.model_world(self.local_point())
    }

    /// A displayed node-local point on the unbroken model, world frame.
    pub fn model_world(&self, local_disp: P) -> P {
        let r = rotated(self.unmap_local(local_disp), self.rot);
        (self.model_origin.0 + r.0, self.model_origin.1 + r.1)
    }

    /// The feature's break view map, if it has one.
    fn view(&self) -> Option<&super::breaks::ViewMap> {
        let v = &self.feature().sketch.as_ref()?.view;
        (!v.is_identity()).then_some(v)
    }

    /// Node-local model → displayed under the feature's break map.
    pub fn map_local(&self, p: P) -> P {
        self.view().map_or(p, |v| v.map(p))
    }

    /// Node-local displayed → model — the unbroken position.
    pub fn unmap_local(&self, p: P) -> P {
        self.view().map_or(p, |v| v.unmap(p))
    }

    /// The **outward** unit normal of a directed anchor (a side, a named
    /// edge), in the drawing frame — what a mate seats along and what sets a
    /// dimension's axis. `None` for the point anchors.
    pub fn outward(&self) -> Option<P> {
        let local = match &self.spot {
            Spot::Side(side) => match side {
                Side::Top => (0.0, -1.0),
                Side::Bottom => (0.0, 1.0),
                Side::Left => (-1.0, 0.0),
                Side::Right => (1.0, 0.0),
            },
            Spot::Segment(Segment::Edge(a, b)) => {
                let len = dist(*a, *b).max(1e-9);
                let t = ((b.0 - a.0) / len, (b.1 - a.1) / len);
                // Outward = the **left of the pen's travel** [SPEC 15.5]: a
                // profile drawn the natural way (material on the pen's right —
                // axis, up, across, down) faces every edge outward, interior
                // shoulders included — where an away-from-centre guess flips.
                (t.1, -t.0)
            }
            _ => return None,
        };
        Some(rotated(local, self.rot))
    }

    /// A line-like anchor's unit direction in the drawing frame — what the
    /// angle op measures [SPEC 15.6]: a named edge, a `|line|`'s run, a bbox
    /// side (the edge along it). `None` for the point anchors.
    pub fn direction(&self) -> Option<P> {
        if let Spot::Segment(Segment::Edge(a, b)) = &self.spot {
            let len = dist(*a, *b).max(1e-9);
            return Some(rotated(((b.0 - a.0) / len, (b.1 - a.1) / len), self.rot));
        }
        if let Spot::Side(side) = &self.spot {
            let along = match side {
                Side::Top | Side::Bottom => (1.0, 0.0),
                Side::Left | Side::Right => (0.0, 1.0),
            };
            return Some(rotated(along, self.rot));
        }
        // A whole `|line|` / `|centerline|` is line-like [SPEC 15.6]: its run
        // from first to last drawn point.
        if matches!(self.spot, Spot::Origin | Spot::Center)
            && self.feature().kind == crate::resolve::NodeKind::Line
            && let Ok(Some(pts)) = super::super::primitives::attr_points(
                &self.feature().attrs,
                "points",
                self.node.span,
            )
            && pts.len() >= 2
        {
            let (a, b) = (pts[0], pts[pts.len() - 1]);
            let len = dist(a, b).max(1e-9);
            return Some(rotated(((b.0 - a.0) / len, (b.1 - a.1) / len), self.rot));
        }
        None
    }

    /// A round-by-construction anchor's **diameter**, px [SPEC 15.6]: a named
    /// `circle()` segment, or an `|oval|`-lineage node drawn as a circle —
    /// never guessed from coordinates.
    pub fn round_diameter(&self) -> Option<f64> {
        if let Spot::Segment(p) = &self.spot {
            return match p {
                Segment::Circle { r, .. } => Some(2.0 * r),
                _ => None,
            };
        }
        let f = self.feature();
        if f.kind != crate::resolve::NodeKind::Oval {
            return None;
        }
        let g = self.geometry_box();
        ((g.w() - g.h()).abs() < 1e-6).then(|| g.w())
    }

    /// The sketch's `mirror:` axes — the unary mirrored readings [SPEC 15.6].
    pub fn mirrors(&self) -> &[MirrorAxis] {
        self.node
            .sketch
            .as_ref()
            .map(|s| s.mirrors.as_slice())
            .unwrap_or(&[])
    }

    /// The anchored node's `pattern:` copy count — the dimension text's `N×`
    /// prefix [SPEC 15.4].
    pub fn pattern_count(&self) -> Option<usize> {
        let ResolvedValue::Call(call) = self.node.attrs.get("pattern")? else {
            return None;
        };
        let num = |i: usize| call.args.get(i).and_then(ResolvedValue::as_number);
        match call.name.as_str() {
            "grid" => Some((num(0)? as usize) * (num(1)? as usize)),
            "radial" => Some(num(0)? as usize),
            _ => None,
        }
    }

    /// Node-local → drawing frame.
    pub fn to_world(&self, p: P) -> P {
        let r = rotated(p, self.rot);
        (self.origin.0 + r.0, self.origin.1 + r.1)
    }

    /// Drawing frame → node-local.
    pub fn to_local(&self, p: P) -> P {
        rotated((p.0 - self.origin.0, p.1 - self.origin.1), -self.rot)
    }
}

/// An endpoint as the author wrote it — scope-relative path plus its anchor;
/// how mates and dimensions spell an endpoint in an error.
pub(super) fn spell(ep: &ResolvedEndpoint, scope: &str) -> String {
    let mut s = super::rel_path(&ep.path, scope).to_string();
    if let Some(side) = ep.side {
        s.push(':');
        s.push_str(match side {
            Side::Top => "top",
            Side::Bottom => "bottom",
            Side::Left => "left",
            Side::Right => "right",
        });
    } else if let Some(p) = &ep.point {
        s.push(':');
        s.push_str(p);
    }
    s
}

pub(super) fn rotated(p: P, deg: f64) -> P {
    if deg == 0.0 {
        return p;
    }
    let (s, c) = deg.to_radians().sin_cos();
    (p.0 * c - p.1 * s, p.0 * s + p.1 * c)
}

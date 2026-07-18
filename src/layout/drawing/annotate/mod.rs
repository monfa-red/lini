//! Annotations [SPEC 15.6/15.7] — the drawing scope's links, lowered to
//! primitives at baked coordinates after mates seat the geometry: linear
//! dimensions and chains, the `(o)` readings, `(<)` angles, leaders, and
//! straight annotation arrows. This module is the orchestrator: it fixes the
//! geometry extent the dims stack outside of, owns the **row packer**, and
//! dispatches each link to its lowering (`dims`, `angle`, `leaders`).

use super::super::ir::{Bbox, PlacedNode};
use super::geometry::P;
use super::{angle, dims, leaders, round};
use crate::ast::Side;
use crate::error::Error;
use crate::ledger::consts::{
    ARROW_HALF, DRAWING_LINK_FONT_SIZE, DRAWING_LINK_STROKE_WIDTH, EXT_OVERSHOOT,
};
use crate::resolve::{AttrMap, LinkKind, MeasureOp, NodeKind, ResolvedLink, ResolvedValue};

mod rows;

#[cfg(test)]
mod tests;

pub(super) use rows::Rows;

/// A dimension's measure axis [SPEC 15.6] — true aligned dims are deferred.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Axis {
    Horizontal,
    Vertical,
}

/// What every lowering reads: the seated children, the scope, the geometry
/// extent (what dims stack outside of, what leader texts clear), and the view
/// scale (measured values divide by it — always pre-scale [SPEC 15.1]).
pub(super) struct Ctx<'a> {
    pub kids: &'a [PlacedNode],
    pub scope: &'a str,
    pub extent: Bbox,
    pub scale: f64,
}

/// A link's resolved paint, read once per statement: the wire stroke (the
/// `|-|` cascade), the support-line tone (`--stroke-light` unless the
/// statement recolours, [SPEC 10.1]), its width (1 in a drawing — the scope
/// default), and the annotation font (the caption 12, the same scope default).
pub(super) struct Paint {
    pub stroke: ResolvedValue,
    pub light: ResolvedValue,
    pub sw: f64,
    pub fs: f64,
    /// The measurement font for the annotation text [SPEC 5] — the statement's
    /// resolved kind × weight (the scope default is mono regular).
    pub font: crate::font::Font,
}

impl Paint {
    pub fn of(attrs: &AttrMap) -> Paint {
        let set = attrs.get("stroke").cloned();
        let live = |name: &str| ResolvedValue::LiveVar {
            name: name.into(),
            raw: false,
        };
        Paint {
            stroke: set.clone().unwrap_or_else(|| live("stroke-dark")),
            light: set.unwrap_or_else(|| live("stroke-light")),
            sw: attrs
                .number("stroke-width")
                .unwrap_or(DRAWING_LINK_STROKE_WIDTH),
            fs: attrs.number("font-size").unwrap_or(DRAWING_LINK_FONT_SIZE),
            font: crate::font::Font::of(attrs),
        }
    }

    /// A dimension / leader polyline in this link's stroke — classed
    /// `lini-dim-line`, so the default paint rides the sheet [SPEC 17].
    pub fn dim(&self, points: Vec<P>) -> PlacedNode {
        let mut n = super::super::prim::line(points, self.stroke.clone(), self.sw);
        n.type_chain = vec!["dim-line".into()];
        n
    }

    /// An extension line — the thin spring that raises a dimension off the
    /// shape — in the light support tone, classed `lini-ext-line`.
    pub fn ext(&self, points: Vec<P>) -> PlacedNode {
        let mut n = super::super::prim::line(points, self.light.clone(), self.sw);
        n.type_chain = vec!["ext-line".into()];
        n
    }

    /// A stroked open path (an angle's arc) in this link's stroke —
    /// `prim::path` is fill-only, built for chart bodies.
    pub fn stroked_path(&self, d: String, bbox: Bbox) -> PlacedNode {
        let mut n = super::super::prim::path(d, ResolvedValue::Ident("none".into()), bbox);
        n.type_chain = vec!["dim-line".into()];
        n.attrs.insert("stroke", self.stroke.clone());
        n.attrs
            .insert("stroke-width", ResolvedValue::Number(self.sw));
        n
    }
}

/// Lower every non-mate link of a drawing scope. Leaders, callouts, and
/// angles go first — their placement is feature-anchored — and their **texts
/// register as obstacles** with the row packer, so a dimension never seats
/// its row on top of a callout ([SPEC 15.6]). Dims then pack in source
/// order; the output keeps source order regardless. The returned nodes
/// append after the geometry children, so annotations paint above it
/// (`layer:` still wins) and the drawing's bbox includes them [SPEC 15.9].
pub(in crate::layout) fn lower(
    kids: &[PlacedNode],
    links: &[&ResolvedLink],
    scope: &str,
    scale: f64,
    extent: Option<Bbox>,
) -> Result<Vec<PlacedNode>, Error> {
    let ctx = Ctx {
        kids,
        scope,
        // A `|detail|` stacks its dims outside the region **circle**, not the
        // full re-laid part it clips away [SPEC 15.8]; every other scope reads
        // its drawn geometry.
        extent: extent.unwrap_or_else(|| geometry_extent(kids)),
        scale,
    };
    let mut rows = Rows::new(ctx.extent);
    let mut outs: Vec<Vec<PlacedNode>> = vec![Vec::new(); links.len()];
    // A dimension takes no `gap:` — it stands off by `clearance` [SPEC 15.6];
    // `gap` is a mate's signed separation [SPEC 15.5/20].
    for w in links {
        if matches!(w.kind, LinkKind::Measure(_)) && w.attrs.get("gap").is_some() {
            return Err(Error::at(
                w.span,
                "a dimension stands off by 'clearance' — 'gap' is a mate's separation",
            ));
        }
    }
    for (i, w) in links.iter().enumerate() {
        let nodes = match w.kind {
            LinkKind::Measure(MeasureOp::Angle) => angle::lower(&ctx, w)?,
            LinkKind::Wire if w.endpoints.len() == 1 => leaders::callout(&ctx, w)?,
            LinkKind::Wire => leaders::arrows(&ctx, w)?,
            _ => continue,
        };
        rows.obstruct_texts(&nodes);
        outs[i] = nodes;
    }
    for (i, w) in links.iter().enumerate() {
        let nodes = match w.kind {
            LinkKind::Measure(MeasureOp::Linear) => dims::linear(&ctx, w, &mut rows)?,
            LinkKind::Measure(MeasureOp::Round) => round::lower(&ctx, w, &mut rows)?,
            _ => continue,
        };
        rows.obstruct_texts(&nodes);
        outs[i] = nodes;
    }
    Ok(outs.into_iter().flatten().collect())
}

/// The extent dimensions stack outside of and leader texts clear: the drawn
/// geometry (chrome included — dims spring past centre marks), sheet content
/// and pinned overlays excluded.
fn geometry_extent(kids: &[PlacedNode]) -> Bbox {
    Bbox::extent_of(kids, |k| {
        !super::is_sheet(k.kind, &k.type_chain) && !super::super::anchors::is_pinned(&k.attrs)
    })
}
/// A side / corner name as its outward unit vector — a leader's `side:`
/// direction, a diametral dim's line [SPEC 15.6/15.7].
pub(super) fn side_unit(name: &str) -> Option<P> {
    let d = std::f64::consts::FRAC_1_SQRT_2;
    Some(match name {
        "top" => (0.0, -1.0),
        "bottom" => (0.0, 1.0),
        "left" => (-1.0, 0.0),
        "right" => (1.0, 0.0),
        "top-left" => (-d, -d),
        "top-right" => (d, -d),
        "bottom-left" => (-d, d),
        "bottom-right" => (d, d),
        _ => return None,
    })
}

/// The `side:` value's raw name, if any.
pub(super) fn side_attr(attrs: &AttrMap) -> Option<&str> {
    match attrs.get("side") {
        Some(ResolvedValue::Ident(s)) => Some(s),
        _ => None,
    }
}

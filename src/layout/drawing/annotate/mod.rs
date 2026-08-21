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

pub(super) use rows::{Rows, SeatLine, away, corner_pull, stack_side};

/// A dimension's row axis [SPEC 15.6] — an aligned dim's frame carries its
/// own axes instead ([`crate::layout::drawing::dims::Frame`]).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Axis {
    Horizontal,
    Vertical,
}

/// What every lowering reads: the seated children, the scope, the geometry
/// extent (what dims stack outside of, what leader texts clear), the view
/// scale (measured values divide by it — always pre-scale [SPEC 15.1]), and
/// the program (carried frames validate `datums:` against its letter sets
/// [SPEC 15.9]).
pub(super) struct Ctx<'a> {
    pub kids: &'a [PlacedNode],
    pub scope: &'a str,
    pub extent: Bbox,
    pub scale: f64,
    pub program: &'a crate::resolve::Program,
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
    /// The statement is a **dimension**, so every chrome node it lowers joins
    /// the `(-)` tier ([`DIM_TIER`]).
    tier: bool,
}

impl Paint {
    pub fn of(attrs: &AttrMap) -> Paint {
        Paint::tiered(attrs, false)
    }

    /// A statement's paint, tiered by its kind — a **dimension** whose tier
    /// repaints the chrome ([`dim_tier_repaints`]) lowers chrome wearing
    /// [`DIM_TIER`], so that paint rides one rule instead of inlining on every
    /// chrome node [SPEC 18]. Two readings, both stated once: the kind is
    /// [`LinkKind::is_dimension`] (what the cascade wears `.lini-dimension`
    /// by), and the repaint test is the renderer's own, so the class is worn
    /// exactly when its rules emit — never a dead class.
    pub fn of_link(ctx: &Ctx, w: &ResolvedLink) -> Paint {
        let tier = w.kind.is_dimension() && dim_tier_repaints(&ctx.program.sheet);
        Paint::tiered(&w.attrs, tier)
    }

    fn tiered(attrs: &AttrMap, tier: bool) -> Paint {
        let set = attrs.get("stroke").cloned();
        Paint {
            stroke: set
                .clone()
                .unwrap_or_else(|| ResolvedValue::live("stroke-dark")),
            light: set.unwrap_or_else(|| ResolvedValue::live("stroke-light")),
            sw: attrs
                .number("stroke-width")
                .unwrap_or(DRAWING_LINK_STROKE_WIDTH),
            fs: attrs.number("font-size").unwrap_or(DRAWING_LINK_FONT_SIZE),
            font: crate::font::Font::of(attrs),
            tier,
        }
    }

    /// The chrome roles this statement's linework wears: the role class, plus
    /// the `(-)` tier when it is a dimension. One place, so a role can never
    /// join the tier on one lowering and not another.
    pub fn roles(&self, role: &str) -> Vec<String> {
        let mut chain = vec![role.to_string()];
        if self.tier {
            chain.push(DIM_TIER.to_string());
        }
        chain
    }

    /// A dimension / leader polyline in this link's stroke — classed
    /// `lini-dim-line`, so the default paint rides the sheet [SPEC 18].
    pub fn dim(&self, points: Vec<P>) -> PlacedNode {
        let mut n = super::super::prim::line(points, self.stroke.clone(), self.sw);
        n.type_chain = self.roles("dim-line");
        n
    }

    /// An extension line — the thin spring that raises a dimension off the
    /// shape — in the light support tone, classed `lini-ext-line`.
    pub fn ext(&self, points: Vec<P>) -> PlacedNode {
        let mut n = super::super::prim::line(points, self.light.clone(), self.sw);
        n.type_chain = self.roles("ext-line");
        n
    }

    /// A filled marker head this statement lowers [SPEC 15.6/15.7] — the
    /// slender dimension arrow, the seated datum triangle — in the statement's
    /// stroke, joining the `(-)` tier when the statement is a dimension.
    pub fn head(&self, variant: &str, points: Vec<P>) -> PlacedNode {
        let mut n = super::super::prim::dim_marker(variant, points, self.stroke.clone());
        if self.tier {
            n.type_chain.push(DIM_TIER.to_string());
        }
        n
    }

    /// A stroked open path (an angle's arc) in this link's stroke —
    /// `prim::path` is fill-only, built for chart bodies.
    pub fn stroked_path(&self, d: String, bbox: Bbox) -> PlacedNode {
        let mut n = super::super::prim::path(d, ResolvedValue::Ident("none".into()), bbox);
        n.type_chain = self.roles("dim-line");
        n.attrs.insert("stroke", self.stroke.clone());
        n.attrs
            .insert("stroke-width", ResolvedValue::Number(self.sw));
        n
    }
}

/// The generated class every chrome node a **dimension** lowers wears
/// [SPEC 4/15.6/18] — the `(-)` tier, alongside the node's chrome role
/// (`lini-dim-line`, `lini-ext-line`, `lini-marker-dim`). A leader's chrome
/// wears the same roles but not this, so a document that restyles `(-)` alone
/// states the tier's paint as compound rules (`.lini-dim-line.lini-dim`) and
/// no chrome node inlines it. Only dimensions mint it, so an unstyled document
/// emits no rule for it and the class is never worn without one.
pub(crate) const DIM_TIER: &str = "dim";

/// The paint a document's drawing chrome defaults to — `(linework tone,
/// extension-line tone, stroke width)` — for a statement dressed by nothing
/// but the document's `|-|` defaults. The renderer states these in the
/// `.lini-dim-line` / `.lini-ext-line` / drafting-head rules, so a sheet that
/// recolours **or re-weights** its annotations says it once in CSS rather than
/// on every chrome node [SPEC 18]. Read through [`Paint::of`], the one place a
/// statement's chrome paint is decided, so the rule and its wearers can never
/// disagree.
pub(crate) fn default_paint(link_defaults: &AttrMap) -> (ResolvedValue, ResolvedValue, f64) {
    let paint = Paint::of(link_defaults);
    (paint.stroke, paint.light, paint.sw)
}

/// Whether the `(-)` tier repaints a drawing's chrome [SPEC 4/15.6/18] — the
/// dimension layer's paint against the document's. The **one** test the tier
/// class and its rules both key on: layout mints [`DIM_TIER`] on a dimension's
/// chrome only when this holds, and the renderer emits the tier's compound
/// rules only for the roles that then wear it — so an unstyled document grows
/// neither a class nor a rule.
pub(crate) fn dim_tier_repaints(sheet: &crate::resolve::SheetInputs) -> bool {
    default_paint(&sheet.dim_defaults) != default_paint(&sheet.chrome_defaults)
}

/// Lower every non-mate link of a drawing scope. Leaders, callouts, and
/// angles go first — their placement is feature-anchored — and their **texts
/// register as obstacles** with the row packer, so a dimension never seats
/// its row on top of a callout ([SPEC 15.6]). Dims then pack in source
/// order; the output keeps source order regardless. The returned nodes
/// append after the geometry children, so annotations paint above it
/// (`layer:` still wins) and the drawing's bbox includes them [SPEC 15.10].
#[allow(clippy::too_many_arguments)]
pub(in crate::layout) fn lower(
    kids: &[PlacedNode],
    links: &[&ResolvedLink],
    scope: &str,
    scale: f64,
    extent: Option<Bbox>,
    seated: &[usize],
    program: &crate::resolve::Program,
) -> Result<Vec<PlacedNode>, Error> {
    let ctx = Ctx {
        kids,
        scope,
        // A `|detail|` stacks its dims outside the region **circle**, not the
        // full re-laid part it clips away [SPEC 15.8]; every other scope reads
        // its drawn geometry.
        extent: extent.unwrap_or_else(|| geometry_extent(kids)),
        scale,
        program,
    };
    let mut rows = Rows::new(ctx.extent);
    // Annotation obstacles [SPEC 15.5/15.6/15.9]: placed drafting symbols and
    // **seated** annotations — a bundle one union box — register their
    // painted bounds before anything packs, so a dim row stands off them.
    for (i, k) in kids.iter().enumerate() {
        if annotation_obstacle(k) || seated.contains(&i) {
            rows.obstruct(k.bbox.shifted(k.cx, k.cy));
        }
    }
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
        if !matches!(w.kind, LinkKind::Measure(MeasureOp::Angle) | LinkKind::Wire) {
            continue;
        }
        // The carried stack lowers **first** [SPEC 15.9]: its one measured
        // box is part of the statement's own clearing, and the same lowered
        // nodes seat at the text seat once the ink is placed.
        let stack = super::symbols::CarriedStack::lower(&ctx, w)?;
        let mut nodes = match w.kind {
            LinkKind::Measure(MeasureOp::Angle) => angle::lower(&ctx, w)?,
            // A one-ended statement is a callout — a `&` fan keeps every
            // endpoint on the one link [SPEC 15.7], so the shape, not the
            // endpoint count, decides.
            LinkKind::Wire if w.one_ended => leaders::callout(&ctx, w, &rows, &stack)?,
            _ => leaders::arrows(&ctx, w)?,
        };
        nodes.extend(stack.seat(&nodes));
        rows.obstruct_texts(&nodes);
        outs[i] = nodes;
    }
    for (i, w) in links.iter().enumerate() {
        if !matches!(
            w.kind,
            LinkKind::Measure(MeasureOp::Linear | MeasureOp::Round)
        ) {
            continue;
        }
        let stack = super::symbols::CarriedStack::lower(&ctx, w)?;
        let mut nodes = match w.kind {
            LinkKind::Measure(MeasureOp::Linear) => dims::linear(&ctx, w, &mut rows, &stack)?,
            _ => round::lower(&ctx, w, &mut rows, &stack)?,
        };
        nodes.extend(stack.seat(&nodes));
        rows.obstruct_texts(&nodes);
        outs[i] = nodes;
    }
    let mut out: Vec<PlacedNode> = outs.into_iter().flatten().collect();
    // Crossing halos [SPEC 15.7]: break the lowered linework where it crosses
    // the drawn geometry — after every statement is placed, one pass.
    super::halo::apply(&ctx, &mut out);
    Ok(out)
}

/// The extent dimensions stack outside of and leader texts clear: the drawn
/// geometry (chrome included — dims spring past centre marks), sheet content
/// and pinned overlays excluded.
fn geometry_extent(kids: &[PlacedNode]) -> Bbox {
    Bbox::extent_of(kids, super::is_geometry)
}

/// The drawn-geometry extent of an already-annotated drawing's children —
/// [`geometry_extent`] re-read after lowering: the same sheet / pinned
/// exclusions, minus the annotation ink the lowering appended (dim and
/// extension lines, arrow markers). The overlap oracle's ground truth
/// ([`crate::testing::carried_over_geometry`]).
pub(crate) fn drawn_geometry(kids: &[PlacedNode]) -> Bbox {
    Bbox::extent_of(kids, |k| {
        super::is_geometry(k)
            && !k
                .type_chain
                .iter()
                .any(|t| t == "dim-line" || t == "ext-line" || t == "marker")
    })
}

/// The **annotation-obstacle class** [SPEC 15.6/15.9]: a node whose painted
/// box dimension rows must clear — a drafting symbol, or framed annotation
/// linework (the `>-` leader's datum box).
pub(super) fn annotation_obstacle(n: &PlacedNode) -> bool {
    super::symbols::drafting_type(&n.type_chain).is_some()
        || n.type_chain.iter().any(|t| t == "datum-frame")
}
/// A side / corner name as its outward unit vector — a leader's `side:`
/// direction, a diametral dim's line [SPEC 15.6/15.7].
pub(super) fn side_unit(name: &str) -> Option<P> {
    if let Some(side) = Side::parse(name) {
        return Some(side.outward());
    }
    let d = std::f64::consts::FRAC_1_SQRT_2;
    Some(match name {
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

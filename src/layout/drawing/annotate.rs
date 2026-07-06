//! Annotations [SPEC 15.6/15.7] — the drawing scope's links, lowered to
//! primitives at baked coordinates after mates seat the geometry: linear
//! dimensions and chains, the `(-)` readings, `(<)` angles, leaders, and
//! straight annotation arrows. This module is the orchestrator: it fixes the
//! geometry extent the dims stack outside of, owns the **row packer**, and
//! dispatches each link to its lowering (`dims`, `angle`, `leaders`).

use super::super::ir::{Bbox, PlacedNode};
use super::geometry::P;
use super::{angle, dims, leaders, round};
use crate::ast::Side;
use crate::error::Error;
use crate::resolve::{AttrMap, LinkKind, MeasureOp, ResolvedLink, ResolvedValue};

// The dimension / leader anatomy — baked sheet constants [SPEC 10.5], never
// scaled by the view.
pub(super) const DIM_OFFSET: f64 = 18.0;
pub(super) const DIM_PITCH: f64 = 16.0;
pub(super) const EXT_GAP: f64 = 3.0;
pub(super) const EXT_OVERSHOOT: f64 = 3.0;
/// The drafting-slender arrow, ≈ 3 : 1 [SPEC 15.6] — length × half-width, at
/// stroke-width 1; both scale with the dim's `stroke-width`.
pub(super) const ARROW_LEN: f64 = 9.0;
pub(super) const ARROW_HALF: f64 = 1.5;
pub(super) const NOTE_OFFSET: f64 = 14.0;
pub(super) const NOTE_LANDING: f64 = 8.0;

/// A dimension's measure axis [SPEC 15.6] — true aligned dims are deferred.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Axis {
    Horizontal,
    Vertical,
}

/// What every lowering reads: the seated children, the scope, the geometry
/// extent (what dims stack outside of, what leader texts clear), the view
/// scale (measured values divide by it — always pre-scale [SPEC 15.1]), and
/// the drawing's `unit:`.
pub(super) struct Ctx<'a> {
    pub kids: &'a [PlacedNode],
    pub scope: &'a str,
    pub extent: Bbox,
    pub scale: f64,
    pub unit: Option<&'a str>,
}

/// A link's resolved paint, read once per statement: the wire stroke (the
/// `|-|` cascade), the support-line tone (`--stroke-light` unless the
/// statement recolours, [SPEC 10.1]), its width (1 in a drawing — the scope
/// default), and the label font (the link-label 11).
pub(super) struct Paint {
    pub stroke: ResolvedValue,
    pub light: ResolvedValue,
    pub sw: f64,
    pub fs: f64,
}

impl Paint {
    pub fn of(attrs: &AttrMap) -> Paint {
        let set = attrs.get("stroke").cloned();
        let live = |name: &str| ResolvedValue::LiveVar {
            name: name.into(),
            raw: false,
        };
        Paint {
            stroke: set.clone().unwrap_or_else(|| live("stroke")),
            light: set.unwrap_or_else(|| live("stroke-light")),
            sw: attrs.number("stroke-width").unwrap_or(1.0),
            fs: attrs.number("font-size").unwrap_or(11.0),
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

/// Lower every non-mate link of a drawing scope, in source order. The
/// returned nodes append after the geometry children, so annotations paint
/// above it (`layer:` still wins) and the drawing's bbox includes them
/// [SPEC 15.9].
pub(in crate::layout) fn lower(
    kids: &[PlacedNode],
    links: &[&ResolvedLink],
    scope: &str,
    scale: f64,
    unit: Option<&str>,
) -> Result<Vec<PlacedNode>, Error> {
    let ctx = Ctx {
        kids,
        scope,
        extent: geometry_extent(kids),
        scale,
        unit,
    };
    let mut rows = Rows::new(ctx.extent);
    let mut out = Vec::new();
    for w in links {
        match w.kind {
            LinkKind::Mate => {}
            LinkKind::Measure(MeasureOp::Linear) => {
                out.extend(dims::linear(&ctx, w, &mut rows)?);
            }
            LinkKind::Measure(MeasureOp::Round) => {
                out.extend(round::lower(&ctx, w, &mut rows)?);
            }
            LinkKind::Measure(MeasureOp::Angle) => out.extend(angle::lower(&ctx, w)?),
            LinkKind::Wire if w.endpoints.len() == 1 => {
                out.extend(leaders::callout(&ctx, w)?);
            }
            LinkKind::Wire => out.extend(leaders::arrows(&ctx, w)?),
        }
    }
    Ok(out)
}

/// The extent dimensions stack outside of and leader texts clear: the drawn
/// geometry (chrome included — dims spring past centre marks), sheet content
/// and pinned overlays excluded.
fn geometry_extent(kids: &[PlacedNode]) -> Bbox {
    let mut ext = Bbox {
        min_x: f64::INFINITY,
        min_y: f64::INFINITY,
        max_x: f64::NEG_INFINITY,
        max_y: f64::NEG_INFINITY,
    };
    for k in kids
        .iter()
        .filter(|k| !super::is_sheet(k.kind, &k.type_chain))
        .filter(|k| !super::super::anchors::is_pinned(&k.attrs))
    {
        super::super::accumulate_extent(k, 0.0, 0.0, 0.0, &mut ext);
    }
    if ext.min_x.is_finite() {
        ext
    } else {
        Bbox::empty()
    }
}

/// The row packer [SPEC 15.6]: dims sharing a side pack into rows `DIM_PITCH`
/// apart, the first `DIM_OFFSET` from the geometry's extent; each dim, in
/// source order, takes the innermost row where its span — text included —
/// overlaps nothing already placed. `gap:` pins a dim's own offset instead
/// (still recorded, so packed rows avoid it).
pub(super) struct Rows {
    extent: Bbox,
    placed: Vec<(Side, f64, (f64, f64))>,
}

impl Rows {
    fn new(extent: Bbox) -> Rows {
        Rows {
            extent,
            placed: Vec::new(),
        }
    }

    /// Seat a dim occupying `interval` on `side`; returns the dimension
    /// line's world coordinate along the stack axis.
    pub fn seat(&mut self, side: Side, interval: (f64, f64), pinned: Option<f64>) -> f64 {
        let off = pinned.unwrap_or_else(|| {
            (0..)
                .map(|k| DIM_OFFSET + k as f64 * DIM_PITCH)
                .find(|cand| {
                    !self.placed.iter().any(|(s, o, iv)| {
                        *s == side
                            && (o - cand).abs() < DIM_PITCH - 1e-6
                            && iv.0 < interval.1 - 1e-6
                            && iv.1 > interval.0 + 1e-6
                    })
                })
                .expect("an offset frees up")
        });
        self.placed.push((side, off, interval));
        match side {
            Side::Bottom => self.extent.max_y + off,
            Side::Top => self.extent.min_y - off,
            Side::Right => self.extent.max_x + off,
            Side::Left => self.extent.min_x - off,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::super::testutil::{by_id, laid, layout_err, text_at, texts};
    use super::{DIM_OFFSET, DIM_PITCH};
    use crate::resolve::{MarkerKind, NodeKind, ResolvedValue};

    // ── Linear dims & chains [SPEC 15.6] ──

    #[test]
    fn a_chain_shares_one_row_and_the_next_dim_packs_outside() {
        // Plate −75..75, holes at −50 and 10, at 2 px/unit: hops 25 · 60 · 65
        // all fit their spans and share one row; the overall 150 overlaps
        // them and takes the next.
        let l = laid(
            "{ layout: drawing; scale: 2 }\n|rect#plate| { width: 150; height: 40 }\n|hole#a| { width: 8; translate: -50 0 }\n|hole#b| { width: 8; translate: 10 0 }\nplate:left <-> a <-> b <-> plate:right { side: bottom }\nplate:left <-> plate:right { side: bottom }\n",
        );
        let (_, y25, _) = text_at(&l.nodes, "25");
        let (_, y60, _) = text_at(&l.nodes, "60");
        let (_, y65, _) = text_at(&l.nodes, "65");
        let (_, y150, _) = text_at(&l.nodes, "150");
        assert!(
            (y25 - y60).abs() < 1e-6 && (y60 - y65).abs() < 1e-6,
            "hops share the row: {y25} / {y60} / {y65}"
        );
        assert!(
            (y150 - y60 - DIM_PITCH).abs() < 0.01,
            "the 150 packs one pitch out: {y150} vs {y60}"
        );
        // First row sits DIM_OFFSET past the plate's paint extent (41 + 18),
        // text lifted 7.5 above the line.
        assert!((y60 - (41.0 + DIM_OFFSET - 7.5)).abs() < 0.6, "y60={y60}");
    }

    #[test]
    fn iso_text_turns_with_a_vertical_dim() {
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#a| { width: 60; height: 40 }\na:top <-> a:bottom { side: right }\n",
        );
        let (x, _, rot) = text_at(&l.nodes, "40");
        assert_eq!(rot, -90.0, "reads from the right");
        let a = by_id(&l.nodes, "a");
        assert!(x > a.cx + 30.0, "stacked right of the geometry: x={x}");
    }

    #[test]
    fn a_narrow_span_slides_its_text_past_the_extension_line() {
        // An 18-wide span can't hold text + arrows: the text slides right,
        // past the higher-u extension line.
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#a| { width: 18; height: 10 }\na:left <-> a:right { side: bottom }\n",
        );
        let (x, _, _) = text_at(&l.nodes, "18");
        assert!(x > 9.0, "text outside the span: x={x}");
    }

    #[test]
    fn corner_anchors_on_one_edge_pull_the_dim_there() {
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#a| { width: 40; height: 20 }\n|rect#b| { width: 40; height: 20; translate: 70 0 }\na:top-left <-> b:top-right\n",
        );
        let (_, y, _) = text_at(&l.nodes, "110");
        let a = by_id(&l.nodes, "a");
        assert!(y < a.cy - 10.0, "pulled to the top: y={y}");
    }

    #[test]
    fn a_two_ended_label_replaces_the_number() {
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#a| { width: 40; height: 20 }\na:left <-> a:right \"180\"\n",
        );
        text_at(&l.nodes, "180");
        assert!(
            !texts(&l.nodes).iter().any(|(t, ..)| t == "40"),
            "the measured 40 is replaced"
        );
    }

    #[test]
    fn unit_suffixes_linear_values_only() {
        let l = laid(
            "{ layout: drawing; unit: \"mm\" }\n|rect#a| { width: 40; height: 20 }\n|hole#h| { width: 12 }\na:left <-> a:right { side: bottom }\nh (-)\n",
        );
        text_at(&l.nodes, "40 mm");
        text_at(&l.nodes, "⌀12");
    }

    #[test]
    fn dim_errors_speak_spec() {
        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|rect#a| { width: 40; height: 20 }\n|rect#b| { width: 40; height: 20 }\na:left <-> b:top\n"
            ),
            "'a:left <-> b:top' mixes axes — anchor one axis"
        );
        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|rect#a| { width: 40; height: 20 }\na:left <-> a:right { side: left }\n"
            ),
            "a horizontal dimension stacks on top or bottom"
        );
        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|rect#a| { width: 40; height: 20 }\na:top <-> a:bottom { side: top }\n"
            ),
            "a vertical dimension stacks on left or right"
        );
        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|rect#a| { width: 40; height: 20 }\na:left <-> a:right { tol: \"x\" }\n"
            ),
            "'tol' takes a number, '+upper -lower', or a fit ident"
        );
    }

    // ── Tolerances [SPEC 15.6] ──

    #[test]
    fn tol_composes_its_three_forms() {
        let sym = laid(
            "{ layout: drawing; scale: 1 }\n|rect#a| { width: 40; height: 20 }\na:left <-> a:right { tol: 0.1 }\n",
        );
        text_at(&sym.nodes, "40±0.1");

        let fit = laid(
            "{ layout: drawing; scale: 1 }\n|rect#a| { width: 40; height: 20 }\na:left <-> a:right { tol: H7 }\n",
        );
        text_at(&fit.nodes, "40 H7");

        // Stacked deviations: raised / lowered beside the value, 0.7 × font.
        let dev = laid(
            "{ layout: drawing; scale: 1 }\n|rect#a| { width: 40; height: 20 }\na:left <-> a:right { tol: +0.2 -0.05 }\n",
        );
        let (_, yu, _) = text_at(&dev.nodes, "+0.2");
        let (_, yl, _) = text_at(&dev.nodes, "-0.05");
        let (_, ym, _) = text_at(&dev.nodes, "40");
        assert!(
            yu < ym && ym < yl,
            "raised {yu} / value {ym} / lowered {yl}"
        );
    }

    // ── The `(-)` readings [SPEC 15.6] ──

    #[test]
    fn a_named_arc_reads_its_radius() {
        let l = laid(
            "{ layout: drawing; scale: 2 }\n|sketch#s| { draw: move(0, 0) right(30) fillet(3):r1 up(20) left(30) down(20) close() }\ns:r1 (-)\n",
        );
        text_at(&l.nodes, "R3");
    }

    #[test]
    fn a_circle_segment_reads_its_diameter() {
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|sketch#s| { draw: move(0, 0) right(40) up(20) left(40) close() move(20, -10) circle(5):c }\ns:c (-)\n",
        );
        text_at(&l.nodes, "⌀10");
    }

    #[test]
    fn a_bare_round_node_leaders_its_diameter_with_the_copy_count() {
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#plate| { width: 120; height: 60 } [\n  |hole#pin| { width: 10; translate: -35 0; pattern: grid(2, 1, 70, 0) }\n]\nplate.pin (-) \"H7\"\n",
        );
        text_at(&l.nodes, "2× ⌀10 H7");
    }

    #[test]
    fn a_side_anchor_on_a_round_node_draws_the_diametral_line() {
        // The value doesn't fit inside ⌀16 — the line overruns the anchored
        // rim and the text spills upward, turned with the vertical line.
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#plate| { width: 80; height: 40 }\n|hole#eye| { width: 16 }\neye:top (-)\n",
        );
        let (_, y, rot) = text_at(&l.nodes, "⌀16");
        assert_eq!(rot, -90.0, "turned with the line");
        assert!(y < -8.0, "spills past the top rim: y={y}");
    }

    #[test]
    fn a_side_anchor_on_any_node_spans_to_the_opposite_side() {
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#bore| { width: 60; height: 16 }\nbore:top (-) { side: right }\n",
        );
        let (x, _, _) = text_at(&l.nodes, "⌀16");
        assert!(x > 30.0, "stacked on the right: x={x}");
    }

    #[test]
    fn a_mirrored_name_spans_its_station_across_the_axis() {
        let l = laid(
            "{ layout: drawing; scale: 2 }\n|sketch#bar| { draw: move(-150, 0) up(10) right(40):thread right(260) down(10); mirror: x-axis }\nbar:thread (-) { side: left; tol: h6 }\n",
        );
        text_at(&l.nodes, "⌀20 h6");
    }

    #[test]
    fn a_bare_round_measure_needs_an_axis() {
        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|rect#block| { width: 40; height: 20 }\nblock (-)\n"
            ),
            "'(-)' can't pick an axis on 'block' — anchor a side ('block:top (-)') or a segment"
        );
    }

    // ── `(<)` — the angle [SPEC 15.6] ──

    #[test]
    fn an_angle_reads_two_edges_and_rides_its_arc() {
        // rise 120 over run 160 → atan = 36.87°.
        let l = laid(
            "{ layout: drawing; scale: 2 }\n|sketch#g| { draw: move(-40, 30) right(80):base up(60) line(-80, 60):flank close() }\ng:flank (<) g:base\n",
        );
        text_at(&l.nodes, "36.87°");
    }

    #[test]
    fn a_unary_angle_measures_the_included_taper() {
        // A 10-in-40 taper mirrored about x: included angle = 2 · atan(10/40)
        // = 28.07°.
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|sketch#cone| { draw: move(0, 0) line(40, -10):taper; mirror: x-axis }\ncone:taper (<)\n",
        );
        text_at(&l.nodes, "28.07°");
    }

    #[test]
    fn angle_errors_speak_spec() {
        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|oval#a| { width: 20; height: 20 }\n|oval#b| { width: 20; height: 20 }\na (<) b\n"
            ),
            "an angle reads two edges — a named segment, a '|line|', or a side"
        );
        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|sketch#s| { draw: move(0, 0) line(40, -10):taper up(10) close() }\ns:taper (<)\n"
            ),
            "'(<)' on ':taper' needs 'mirror:' — no twin to measure against"
        );
        assert_eq!(
            layout_err(
                "{ layout: drawing; scale: 1 }\n|rect#a| { width: 40; height: 20 }\n|rect#b| { width: 40; height: 20 }\na:top (<) b:bottom\n"
            ),
            "the angle's edges are parallel — they never meet"
        );
    }

    // ── Leaders [SPEC 15.7] ──

    #[test]
    fn a_leader_tip_ray_casts_onto_the_outline_with_a_landing_elbow() {
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|oval#disc| { width: 40; height: 40 }\ndisc:top-right <- \"THRU\"\n",
        );
        let line = l
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Line && n.markers.start == MarkerKind::Arrow)
            .expect("the leader line");
        let pts = crate::layout::primitives::attr_points(&line.attrs, "points", line.span)
            .unwrap()
            .unwrap();
        assert_eq!(pts.len(), 3, "tip, elbow, landing");
        // The tip sits on the circle (r = 20), not the bbox corner.
        let tip = pts[0];
        assert!(
            (tip.0.hypot(tip.1) - 20.0).abs() < 0.75,
            "tip on the rim: {tip:?}"
        );
        // The landing is horizontal.
        assert!((pts[1].1 - pts[2].1).abs() < 1e-9, "horizontal landing");
        // Text past the landing.
        let (tx, ty, _) = text_at(&l.nodes, "THRU");
        assert!(tx > pts[2].0, "text past the landing");
        assert!((ty - pts[2].1).abs() < 1e-6, "text rides the landing");
    }

    #[test]
    fn a_word_leader_tips_the_rim_of_a_patterned_hole() {
        // The carrier's ray-cast recurses into a copy — the copy must not
        // still look like a carrier (the pattern attr made it return None
        // and the tip fell back to the hole's centre).
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#plate| { width: 120; height: 60 } [\n  |hole#pin| { width: 10; translate: -35 0; pattern: grid(2, 1, 70, 0) }\n]\nplate.pin <- \"THRU\" { side: top }\n",
        );
        let line = l
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Line && n.markers.start == MarkerKind::Arrow)
            .expect("the leader");
        let tip = crate::layout::primitives::attr_points(&line.attrs, "points", line.span)
            .unwrap()
            .unwrap()[0];
        let d = ((tip.0 - -35.0).powi(2) + tip.1.powi(2)).sqrt();
        assert!((d - 5.0).abs() < 0.75, "tip on the seed's rim: {tip:?}");
    }

    #[test]
    fn a_circle_diameter_runs_across_with_both_arrows() {
        // The ⌀ line is a diameter, not a word leader [SPEC 15.6]: it crosses
        // the circle, overshoots the far rim, and presses both rims inward.
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#plate| { width: 80; height: 40 }\n|hole#eye| { width: 12 }\neye (-)\n",
        );
        let arrows: Vec<_> = l
            .nodes
            .iter()
            .filter(|n| n.type_chain.iter().any(|t| t == "marker-dim"))
            .collect();
        assert_eq!(arrows.len(), 2, "an arrowhead on each rim");
        let line = l
            .nodes
            .iter()
            .find(|n| n.type_chain.iter().any(|t| t == "dim-line"))
            .expect("the ⌀ line");
        let pts = crate::layout::primitives::attr_points(&line.attrs, "points", line.span)
            .unwrap()
            .unwrap();
        let start_r = (pts[0].0.powi(2) + pts[0].1.powi(2)).sqrt();
        assert!(
            start_r > 6.0 && start_r < 20.0,
            "the line overshoots the far rim: {pts:?}"
        );
    }

    #[test]
    fn side_steers_a_leader() {
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|oval#disc| { width: 40; height: 40 }\ndisc <- \"A\" { side: left }\n",
        );
        let (tx, _, _) = text_at(&l.nodes, "A");
        assert!(tx < -20.0, "text left of the disc: {tx}");
    }

    #[test]
    fn the_datum_triangle_seats_on_the_surface() {
        // `>-` on a directed feature: the GD&T triangle's base lies flush
        // with the drawn edge (y = 15), its apex out along the surface
        // normal — never tilted by the leader's approach angle [SPEC 15.7].
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#block| { width: 60; height: 30 }\nblock:bottom >- \"A\"\n",
        );
        let tri = l
            .nodes
            .iter()
            .find(|n| n.type_chain.iter().any(|t| t == "marker-datum"))
            .expect("the seated datum triangle");
        let pts = crate::layout::primitives::attr_points(&tri.attrs, "points", tri.span)
            .unwrap()
            .unwrap();
        assert!(
            (pts[0].1 - 15.0).abs() < 1e-6 && (pts[1].1 - 15.0).abs() < 1e-6,
            "base on the bottom face: {pts:?}"
        );
        assert!(pts[2].1 > 15.0, "apex out along the normal: {pts:?}");
        // A point-anchored datum keeps the core marker, oriented by the line.
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|oval#pin| { width: 20; height: 20 }\npin >- \"B\"\n",
        );
        assert!(
            l.nodes
                .iter()
                .any(|n| n.kind == NodeKind::Line && n.markers.start == MarkerKind::Datum),
            "the fallback datum marker"
        );
    }

    #[test]
    fn a_two_ended_arrow_trims_at_the_rim_and_dots_within() {
        // `b1 -* part`: the line springs from the balloon's rim (default
        // anchor → trimmed) and its dot lands at the part's origin (within).
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#part| { width: 60; height: 30 }\n|balloon#b1| \"1\" { translate: 60 -40 }\nb1 -* part\n",
        );
        let line = l
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Line && n.markers.end == MarkerKind::Dot)
            .expect("the wire");
        let pts = crate::layout::primitives::attr_points(&line.attrs, "points", line.span)
            .unwrap()
            .unwrap();
        let b1 = by_id(&l.nodes, "b1");
        assert!(
            (pts[0].0 - b1.cx).hypot(pts[0].1 - b1.cy) > 7.0,
            "start off the balloon's centre: {:?}",
            pts[0]
        );
        assert_eq!(pts[1], (0.0, 0.0), "the dot lands on the part's origin");
    }

    #[test]
    fn a_leader_tip_lands_on_a_recessed_edge_not_the_box() {
        // The thread section sits below the profile's outer surface: the tip
        // must ray-cast onto the drawn edge (y = −63), not stop at the
        // geometry box (y = −75) — the floating-datum bug (`ray_line`'s
        // segment parameter accepted each segment's mirror about its start).
        let l = laid(
            "{ layout: drawing; scale: 3 }\n|sketch#body| { draw: move(-80, 0) up(21) right(38):thread right(32):land up(4) right(90) down(25); mirror: x-axis }\nbody:thread <- \"M42\" { side: top }\nbody:land >- \"A\"\n",
        );
        let arrow_tip = l
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Line && n.markers.start == MarkerKind::Arrow)
            .map(|n| {
                crate::layout::primitives::attr_points(&n.attrs, "points", n.span)
                    .unwrap()
                    .unwrap()[0]
            })
            .expect("the arrow leader");
        assert!(
            (arrow_tip.1 + 63.0).abs() < 1e-6,
            "the arrow touches the drawn surface: {arrow_tip:?}"
        );
        let tri = l
            .nodes
            .iter()
            .find(|n| n.type_chain.iter().any(|t| t == "marker-datum"))
            .expect("the seated datum triangle");
        let pts = crate::layout::primitives::attr_points(&tri.attrs, "points", tri.span)
            .unwrap()
            .unwrap();
        assert!(
            (pts[0].1 + 63.0).abs() < 1e-6 && (pts[1].1 + 63.0).abs() < 1e-6,
            "the datum base sits on the drawn surface: {pts:?}"
        );
    }

    // ── The anatomy's class hooks [SPEC 17] ──

    #[test]
    fn dimension_anatomy_wears_its_classes() {
        // Paint states once per class: the dim line, the light extension
        // lines, the marker-classed arrowheads — no per-element inline style.
        let l = laid(
            "{ layout: drawing; scale: 1 }\n|rect#a| { width: 60; height: 20 }\na:left <-> a:right { side: bottom }\n",
        );
        let with_chain = |name: &str| {
            l.nodes
                .iter()
                .filter(|n| n.type_chain.iter().any(|t| t == name))
                .collect::<Vec<_>>()
        };
        assert_eq!(with_chain("ext-line").len(), 2, "two extension springs");
        assert_eq!(with_chain("dim-line").len(), 1, "the dim line");
        assert_eq!(with_chain("marker-dim").len(), 2, "two arrowheads");
        // Extension lines take the light support tone [SPEC 15.6]…
        assert!(
            matches!(
                with_chain("ext-line")[0].attrs.get("stroke"),
                Some(ResolvedValue::LiveVar { name, .. }) if name == "stroke-light"
            ),
            "--stroke-light by default"
        );
        // …until the statement recolours — then the whole dim follows.
        let red = laid(
            "{ layout: drawing; scale: 1 }\n|rect#a| { width: 60; height: 20 }\na:left <-> a:right { side: bottom; stroke: red }\n",
        );
        let ext = red
            .nodes
            .iter()
            .find(|n| n.type_chain.iter().any(|t| t == "ext-line"))
            .expect("extension line");
        assert!(
            matches!(ext.attrs.get("stroke"), Some(ResolvedValue::Ident(c)) if c == "red"),
            "a recoloured statement recolours its extension lines too"
        );
    }

    // ── The drawing's `|-|` weight [SPEC 15.1] ──

    #[test]
    fn drawing_links_thin_to_stroke_width_1() {
        let width_of = |src: &str| {
            let l = laid(src);
            let dim_line = l
                .nodes
                .iter()
                .find(|n| n.kind == NodeKind::Line)
                .expect("a dim line");
            match dim_line.attrs.get("stroke-width") {
                Some(ResolvedValue::Number(w)) => *w,
                other => panic!("stroke-width: {other:?}"),
            }
        };
        assert_eq!(
            width_of(
                "{ layout: drawing; scale: 1 }\n|rect#a| { width: 40; height: 20 }\na:left <-> a:right\n"
            ),
            1.0,
            "the drawing-scope link default"
        );
        // A scope default, not a rule — a plain `|-|` rule overrides it.
        // And it is the *immediate* scope's default: a flow container nested
        // in a drawing owns ordinary routed links, weight 2.
        let l = laid(
            "|drawing#d| { scale: 1 } [\n  |rect#part| { width: 40; height: 20 }\n  |row#legend| { translate: 0 60 } [\n    |box#a| \"a\"\n    |box#b| \"b\"\n    a -> b\n  ]\n]\n",
        );
        let wire = l.links.first().expect("the routed flow link");
        assert!(
            wire.attrs.number("stroke-width").is_none_or(|w| w != 1.0),
            "a nested flow's links keep the flow weight: {:?}",
            wire.attrs.get("stroke-width")
        );
        assert_eq!(
            width_of(
                "{ layout: drawing; scale: 1;\n  |-| { stroke-width: 2 }\n}\n|rect#a| { width: 40; height: 20 }\na:left <-> a:right\n"
            ),
            2.0,
            "a user '|-|' rule wins over the scope default"
        );
    }
}

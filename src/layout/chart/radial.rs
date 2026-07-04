//! Radial (polar) gridlines and labels [SPEC 14.7]. The value axis's concentric
//! polygon **web** through the spokes, the spokes themselves (the domain gridlines),
//! the spoke (category) labels around the rim, and the radius tick labels up the top
//! spoke. The series reuse the cartesian builders through `Plot::project`; only these
//! gridlines and labels are radial-specific.

use super::model::Chart;
use super::project::Plot;
use super::scale::{self, Scale};
use crate::layout::PlacedNode;
use crate::layout::prim;
use crate::resolve::ResolvedValue;

const LABEL_SIZE: f64 = 11.0;

fn live(name: &str) -> ResolvedValue {
    ResolvedValue::LiveVar {
        name: name.into(),
        raw: false,
    }
}

/// The web (concentric polygons through the spokes at each radius tick) and the spokes
/// (centre → rim at each domain position), drawn first so the data sits over them.
pub fn gridlines(plot: &Plot, chart: &Chart, out: &mut Vec<PlacedNode>) {
    let Scale::Band { n } = chart.x.scale else {
        return;
    };
    if n == 0 {
        return;
    }
    let xs = &chart.x.scale;
    let vs = &chart.values[0].scale; // a radial chart has one radius (value) axis
    let grid = live("grid");
    for &t in vs.ticks() {
        if vs.frac(t) * plot.radius() < 1.0 {
            continue; // the centre ring collapses to the pole — skip the degenerate polygon
        }
        let mut poly: Vec<(f64, f64)> = (0..n).map(|i| plot.project(xs, i as f64, vs, t)).collect();
        if let Some(&p0) = poly.first() {
            poly.push(p0); // close the ring
        }
        out.push(prim::line(poly, grid.clone(), 1.0));
    }
    let (cx, cy) = plot.center();
    for i in 0..n {
        let a = plot.spoke_angle(xs, i as f64);
        let rim = (cx + plot.radius() * a.sin(), cy - plot.radius() * a.cos());
        out.push(prim::line(vec![(cx, cy), rim], grid.clone(), 1.0));
    }
}

/// The spoke (category) labels just outside the rim, and the radius tick labels up the
/// top spoke (offset right so they clear it).
pub fn labels(plot: &Plot, chart: &Chart, out: &mut Vec<PlacedNode>) {
    let Scale::Band { n } = chart.x.scale else {
        return;
    };
    let xs = &chart.x.scale;
    let vs = &chart.values[0].scale;
    let (cx, cy) = plot.center();
    let muted = live("muted");
    let lr = plot.radius() + LABEL_SIZE * 0.9;
    for i in 0..n {
        let a = plot.spoke_angle(xs, i as f64);
        let label = chart
            .x
            .labels
            .get(i)
            .cloned()
            .unwrap_or_else(|| (i + 1).to_string());
        let lx = cx + lr * a.sin();
        let ly = cy - lr * a.cos();
        out.push(prim::text(
            &label,
            lx,
            ly,
            LABEL_SIZE,
            Some(muted.clone()),
            false,
        ));
    }
    for &t in vs.ticks() {
        let r = vs.frac(t) * plot.radius();
        if r < 1.0 {
            continue; // skip the centre tick — it would pile on the pole
        }
        let label = scale::label(t, &chart.values[0].unit);
        out.push(prim::text_left(
            &label,
            cx + 3.0,
            cy - r,
            LABEL_SIZE,
            Some(muted.clone()),
        ));
    }
}

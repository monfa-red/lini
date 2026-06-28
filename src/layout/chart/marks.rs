//! Line and dot geometry ([CHARTS.md] §3): a polyline through the data (with
//! optional vertex markers), or one marker per datum. Both crop to the plot via the
//! shared clip, and a line honours `curve: linear | step` and `stroke-style`.

use super::model::{Chart, Curve, Data, Series, SeriesKind};
use super::prim;
use super::project::Plot;
use super::scale::{Scale, fmt_tick};
use crate::layout::PlacedNode;

/// One datum's data-space coordinate paired with its pixel coordinate.
type Plotted = ((f64, f64), (f64, f64));

pub fn lay_out(plot: &Plot, chart: &Chart, out: &mut Vec<PlacedNode>) {
    for ser in &chart.series {
        match ser.kind {
            SeriesKind::Line => line(plot, chart, ser, out),
            SeriesKind::Dots => dots(plot, chart, ser, out),
            SeriesKind::Bars => {}
        }
    }
}

/// (data-space coords, pixel coords) for every datum of a series.
fn samples(plot: &Plot, chart: &Chart, ser: &Series) -> Vec<Plotted> {
    let xs = &chart.x.scale;
    let vs = &chart.values[ser.axis].scale;
    match &ser.data {
        Data::Categorical(v) => v
            .iter()
            .enumerate()
            .map(|(i, &y)| ((i as f64, y), (plot.x_at(xs, i as f64), plot.y_at(vs, y))))
            .collect(),
        Data::Points(p) => p
            .iter()
            .map(|&(x, y)| ((x, y), (plot.x_at(xs, x), plot.y_at(vs, y))))
            .collect(),
    }
}

fn line(plot: &Plot, chart: &Chart, ser: &Series, out: &mut Vec<PlacedNode>) {
    let pts = samples(plot, chart, ser);
    if pts.len() < 2 {
        return;
    }
    let mut px: Vec<(f64, f64)> = pts.iter().map(|(_, p)| *p).collect();
    if matches!(ser.curve, Curve::Step) {
        px = step_points(&px);
    }
    for run in plot.clip(&px) {
        let mut ln = prim::line(run, ser.color.clone(), ser.thickness);
        if let Some(s) = &ser.stroke_style {
            ln.attrs.insert("stroke-style", s.clone());
        }
        out.push(ln);
    }
    // Vertex markers at each in-domain datum (the dot family generalised to every
    // vertex, [CHARTS.md] §3).
    if ser.marker {
        let d = (ser.thickness * 2.5).max(5.0);
        for ((xd, yd), (xp, yp)) in &pts {
            if in_domain(chart, ser, *xd, *yd) {
                out.push(prim::oval(*xp, *yp, d, d, ser.color.clone()));
            }
        }
    }
}

fn dots(plot: &Plot, chart: &Chart, ser: &Series, out: &mut Vec<PlacedNode>) {
    let (w, h) = ser.dot;
    for ((xd, yd), (xp, yp)) in samples(plot, chart, ser) {
        if !in_domain(chart, ser, xd, yd) {
            continue;
        }
        let mut dot = prim::oval(xp, yp, w, h, ser.color.clone());
        prim::set_title(&mut dot, dot_title(chart, ser, xd, yd));
        out.push(dot);
    }
}

/// Whether a datum's data coords lie inside both axes' domains (crop, §6).
fn in_domain(chart: &Chart, ser: &Series, x: f64, y: f64) -> bool {
    chart.x.scale.contains(x) && chart.values[ser.axis].scale.contains(y)
}

fn dot_title(chart: &Chart, ser: &Series, x: f64, y: f64) -> String {
    let name = ser.label.as_deref().unwrap_or("");
    let value = match &ser.data {
        Data::Points(_) => format!("{}, {}", fmt_tick(x), fmt_tick(y)),
        Data::Categorical(_) => match (chart.x.labels.get(x as usize), &chart.x.scale) {
            (Some(c), Scale::Band { .. }) => format!("{c}: {}", fmt_tick(y)),
            _ => fmt_tick(y),
        },
    };
    if name.is_empty() {
        value
    } else {
        format!("{name}: {value}")
    }
}

/// A staircase through the pixel points: hold at the current value, then step.
fn step_points(pts: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut out = vec![pts[0]];
    for w in pts.windows(2) {
        out.push((w[1].0, w[0].1));
        out.push((w[1].0, w[1].1));
    }
    out
}

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
            SeriesKind::Area => area(plot, chart, ser, out),
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
        // Sampled to `Points` in `model::build` before layout.
        Data::Formula(_) => Vec::new(),
    }
}

fn line(plot: &Plot, chart: &Chart, ser: &Series, out: &mut Vec<PlacedNode>) {
    let pts = samples(plot, chart, ser);
    if pts.len() < 2 {
        return;
    }
    let px: Vec<(f64, f64)> = pts.iter().map(|(_, p)| *p).collect();
    let curved = curve_points(&px, &ser.curve);
    for run in plot.clip(&curved) {
        let mut ln = prim::line(run, ser.color.clone(), ser.thickness);
        if let Some(s) = &ser.stroke_style {
            ln.attrs.insert("stroke-style", s.clone());
        }
        out.push(ln);
    }
    vertex_markers(chart, ser, &pts, out);
}

/// An `|area|`: a filled polygon from the (curved, plot-clamped) top edge down to the
/// baseline, plus the top edge as a line ([CHARTS.md] §3).
fn area(plot: &Plot, chart: &Chart, ser: &Series, out: &mut Vec<PlacedNode>) {
    let pts = samples(plot, chart, ser);
    if pts.len() < 2 {
        return;
    }
    let px: Vec<(f64, f64)> = pts.iter().map(|(_, p)| *p).collect();
    let top = curve_points(&px, &ser.curve);
    let scale = &chart.values[ser.axis].scale;
    let base = ser.baseline.unwrap_or(0.0);
    let base_y = plot.y_at(scale, scale.clamp(base));
    let mut poly: Vec<(f64, f64)> = top
        .iter()
        .map(|&(x, y)| (x.clamp(plot.x0, plot.x1), y.clamp(plot.y0, plot.y1)))
        .collect();
    if let (Some(&(fx, _)), Some(&(lx, _))) = (poly.first(), poly.last()) {
        poly.push((lx, base_y));
        poly.push((fx, base_y));
        out.push(prim::poly(poly, ser.color.clone(), 0.82));
    }
    for run in plot.clip(&top) {
        out.push(prim::line(run, ser.color.clone(), ser.thickness.max(2.0)));
    }
    vertex_markers(chart, ser, &pts, out);
}

/// Markers at each in-domain datum (the dot family generalised to every vertex,
/// [CHARTS.md] §3), drawn when a line / area sets `marker:`.
fn vertex_markers(chart: &Chart, ser: &Series, pts: &[Plotted], out: &mut Vec<PlacedNode>) {
    if !ser.marker {
        return;
    }
    let d = (ser.thickness * 2.5).max(5.0);
    for ((xd, yd), (xp, yp)) in pts {
        if in_domain(chart, ser, *xd, *yd) {
            out.push(prim::oval(*xp, *yp, d, d, ser.color.clone()));
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
        Data::Categorical(_) => match (chart.x.labels.get(x as usize), &chart.x.scale) {
            (Some(c), Scale::Band { .. }) => format!("{c}: {}", fmt_tick(y)),
            _ => fmt_tick(y),
        },
        // Points (incl. a sampled formula): the x,y pair.
        _ => format!("{}, {}", fmt_tick(x), fmt_tick(y)),
    };
    if name.is_empty() {
        value
    } else {
        format!("{name}: {value}")
    }
}

/// The pixel polyline a curve draws through `pts` ([CHARTS.md] §3): straight
/// segments, a staircase, or a monotone-cubic resampled into a dense polyline (so
/// the clip and polyline emitters are reused unchanged — no separate bézier path).
fn curve_points(pts: &[(f64, f64)], curve: &Curve) -> Vec<(f64, f64)> {
    match curve {
        Curve::Linear => pts.to_vec(),
        Curve::Step => {
            let mut out = vec![pts[0]];
            for w in pts.windows(2) {
                out.push((w[1].0, w[0].1));
                out.push((w[1].0, w[1].1));
            }
            out
        }
        Curve::Smooth => monotone_resample(pts),
    }
}

/// A monotone cubic Hermite (Fritsch–Carlson) through `pts`, resampled into a dense
/// polyline. Passes through every point and **never overshoots** — no invented peak
/// or sub-baseline dip ([CHARTS.md] §3). Assumes x is monotonic (line / fn data).
fn monotone_resample(pts: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let n = pts.len();
    if n < 3 {
        return pts.to_vec();
    }
    let dx: Vec<f64> = (0..n - 1).map(|i| pts[i + 1].0 - pts[i].0).collect();
    let s: Vec<f64> = (0..n - 1)
        .map(|i| {
            if dx[i].abs() < 1e-9 {
                0.0
            } else {
                (pts[i + 1].1 - pts[i].1) / dx[i]
            }
        })
        .collect();
    // Tangents: secant average inside, clamped to keep the fit monotone.
    let mut m = vec![0.0; n];
    m[0] = s[0];
    m[n - 1] = s[n - 2];
    for i in 1..n - 1 {
        m[i] = if s[i - 1] * s[i] <= 0.0 {
            0.0
        } else {
            (s[i - 1] + s[i]) / 2.0
        };
    }
    for i in 0..n - 1 {
        if s[i].abs() < 1e-12 {
            m[i] = 0.0;
            m[i + 1] = 0.0;
        } else {
            let (a, b) = (m[i] / s[i], m[i + 1] / s[i]);
            let h = a.hypot(b);
            if h > 3.0 {
                let t = 3.0 / h;
                m[i] = t * a * s[i];
                m[i + 1] = t * b * s[i];
            }
        }
    }
    const K: usize = 16; // sub-segments per interval
    let mut out = vec![pts[0]];
    for i in 0..n - 1 {
        let (x0, y0) = pts[i];
        let (x1, y1) = pts[i + 1];
        let h = x1 - x0;
        for k in 1..=K {
            let t = k as f64 / K as f64;
            let (t2, t3) = (t * t, t * t * t);
            let y = (2.0 * t3 - 3.0 * t2 + 1.0) * y0
                + (t3 - 2.0 * t2 + t) * h * m[i]
                + (-2.0 * t3 + 3.0 * t2) * y1
                + (t3 - t2) * h * m[i + 1];
            out.push((x0 + t * h, y));
        }
    }
    out
}

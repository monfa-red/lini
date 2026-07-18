//! Line and dot geometry [SPEC 14.2]: a polyline through the data (with
//! optional vertex markers), or one marker per datum. Both crop to the plot via the
//! shared clip, and a line honours `curve: linear | step` and `stroke-style`.

use super::metrics::AREA_OPACITY;
use super::model::{Chart, Curve, Data, Series, SeriesKind};
use super::palette::deepen;
use super::project::{Dir, Plot};
use super::scale::Scale;
use crate::layout::PlacedNode;
use crate::layout::prim;
use crate::ledger::format;
use crate::resolve::MarkerKind;

/// One datum's data-space coordinate paired with its pixel coordinate.
pub(super) type Plotted = ((f64, f64), (f64, f64));

/// All `|area|` series — drawn behind bars and lines [SPEC 14.9].
pub fn areas(plot: &Plot, chart: &Chart, out: &mut Vec<PlacedNode>) {
    for ser in &chart.series {
        if matches!(ser.kind, SeriesKind::Area) {
            draw_area(plot, chart, ser, out);
        }
    }
}

/// All `|line|` series — drawn over areas and bars [SPEC 14.9].
pub fn lines(plot: &Plot, chart: &Chart, out: &mut Vec<PlacedNode>) {
    for ser in &chart.series {
        if matches!(ser.kind, SeriesKind::Line) {
            draw_line(plot, chart, ser, out);
        }
    }
}

/// All `|dots|` series — drawn on top [SPEC 14.9].
pub fn dots(plot: &Plot, chart: &Chart, out: &mut Vec<PlacedNode>) {
    for ser in &chart.series {
        if matches!(ser.kind, SeriesKind::Dots) {
            draw_dots(plot, chart, ser, out);
        }
    }
}

/// (data-space coords, pixel coords) for every datum of a series. The pixel point comes
/// from the shared `Plot::project`, so a radar reuses these builders unchanged ([SPEC 14.7]).
/// Shared with the inline-label collector ([`super::labels`]), so a tag sits on the
/// exact point its marker does.
pub(super) fn samples(plot: &Plot, chart: &Chart, ser: &Series) -> Vec<Plotted> {
    let xs = &chart.x.scale;
    let vs = &chart.values[ser.axis].scale;
    match &ser.data {
        Data::Categorical(v) => v
            .iter()
            .enumerate()
            .map(|(i, &y)| ((i as f64, y), plot.project(xs, i as f64, vs, y)))
            .collect(),
        Data::Points(p) => p
            .iter()
            .map(|&(x, y)| ((x, y), plot.project(xs, x, vs, y)))
            .collect(),
        // Sampled to `Points` in `model::build` before layout.
        Data::Formula(_) => Vec::new(),
    }
}

fn draw_line(plot: &Plot, chart: &Chart, ser: &Series, out: &mut Vec<PlacedNode>) {
    let pts = samples(plot, chart, ser);
    if pts.len() < 2 {
        return;
    }
    let px: Vec<(f64, f64)> = pts.iter().map(|(_, p)| *p).collect();
    for run in line_runs(plot, &px, &ser.curve) {
        let mut ln = prim::line(run, ser.color.clone(), ser.thickness);
        if let Some(s) = &ser.stroke_style {
            ln.attrs.insert("stroke-style", s.clone());
        }
        out.push(ln);
    }
    vertex_markers(chart, ser, &pts, out);
}

/// The polyline run(s) for a series' pixel points: a radar (radial) closes the loop
/// back to the first point and is uncropped (it sits within the rim); a cartesian line
/// is interpolated by `curve:` and cropped to the plot [SPEC 14.4].
fn line_runs(plot: &Plot, px: &[(f64, f64)], curve: &Curve) -> Vec<Vec<(f64, f64)>> {
    if plot.is_radial() {
        let mut loop_ = px.to_vec();
        loop_.push(px[0]);
        return vec![loop_];
    }
    plot.clip(&curve_points(px, curve))
}

/// An `|area|`: a filled polygon from the (curved, plot-clamped) top edge down to the
/// baseline, plus the top edge as a line [SPEC 14.2].
fn draw_area(plot: &Plot, chart: &Chart, ser: &Series, out: &mut Vec<PlacedNode>) {
    let pts = samples(plot, chart, ser);
    if pts.len() < 2 {
        return;
    }
    let px: Vec<(f64, f64)> = pts.iter().map(|(_, p)| *p).collect();
    // The edge stroke [SPEC 14.6]: an explicit `stroke`, else a deep tier of the
    // fill so the area reads as a filled shape with a defined outline, not a flat blob.
    let edge = ser
        .outline
        .as_ref()
        .map(|(c, _)| c.clone())
        .unwrap_or_else(|| deepen(&ser.color));
    // A filled radar fills the closed spoke polygon — there is no baseline ([SPEC 14.7]).
    if plot.is_radial() {
        out.push(prim::poly(px.clone(), ser.color.clone(), AREA_OPACITY));
        for run in line_runs(plot, &px, &ser.curve) {
            out.push(prim::line(run, edge.clone(), ser.thickness));
        }
        vertex_markers(chart, ser, &pts, out);
        return;
    }
    // A column curves the top edge and fills down to a horizontal baseline; a row
    // fills across to a vertical baseline (the value-zero line, now on the left).
    let top = if plot.dir == Dir::Row {
        px.clone()
    } else {
        curve_points(&px, &ser.curve)
    };
    let scale = &chart.values[ser.axis].scale;
    let base = ser.baseline.unwrap_or(0.0);
    let mut poly: Vec<(f64, f64)> = top
        .iter()
        .map(|&(x, y)| (x.clamp(plot.x0, plot.x1), y.clamp(plot.y0, plot.y1)))
        .collect();
    if let (Some(&(fx, fy)), Some(&(lx, ly))) = (poly.first(), poly.last()) {
        match plot.dir {
            Dir::Row => {
                let bx = plot.x0 + scale.frac(scale.clamp(base)) * plot.w();
                poly.push((bx, ly));
                poly.push((bx, fy));
            }
            _ => {
                let by = plot.y_at(scale, scale.clamp(base));
                poly.push((lx, by));
                poly.push((fx, by));
            }
        }
        out.push(prim::poly(poly, ser.color.clone(), AREA_OPACITY));
    }
    for run in plot.clip(&top) {
        out.push(prim::line(run, edge.clone(), ser.thickness));
    }
    vertex_markers(chart, ser, &pts, out);
}

/// The diameter of a centred chart marker by kind [SPEC 14.2]: a `dot` stays small,
/// a `circle` / `diamond` is larger (hover-sized), each scaling gently with the line
/// thickness. A `|dots|` series sizes itself by `width` instead; a `|mark|` point passes a
/// nominal thickness. Shared by line/area vertices ([`vertex_markers`]) and mark points.
pub(super) fn marker_diameter(kind: MarkerKind, thickness: f64) -> f64 {
    match kind {
        MarkerKind::Circle | MarkerKind::Diamond => (thickness * 4.0).max(11.0),
        _ => (thickness * 2.5).max(5.0),
    }
}

/// Markers at each in-domain datum (the marker family generalised to every vertex,
/// [SPEC 14.2]), drawn when a line / area sets `marker:`. Each carries the datum's
/// `<title>`, so a marked point is a hover target ([SPEC 14.8]) — pick `circle` to size it for
/// hovering.
fn vertex_markers(chart: &Chart, ser: &Series, pts: &[Plotted], out: &mut Vec<PlacedNode>) {
    if ser.marker == MarkerKind::None {
        return;
    }
    let d = marker_diameter(ser.marker, ser.thickness);
    for ((xd, yd), (xp, yp)) in pts {
        if in_domain(chart, ser, *xd, *yd) {
            let mut m = prim::marker(ser.marker, *xp, *yp, d, d, ser.color.clone());
            prim::set_hint(&mut m, dot_title(chart, ser, *xd, *yd));
            out.push(m);
        }
    }
}

fn draw_dots(plot: &Plot, chart: &Chart, ser: &Series, out: &mut Vec<PlacedNode>) {
    let (w, h) = ser.dot;
    for ((xd, yd), (xp, yp)) in samples(plot, chart, ser) {
        if !in_domain(chart, ser, xd, yd) {
            continue;
        }
        let mut dot = prim::marker(ser.marker, xp, yp, w, h, ser.color.clone());
        prim::set_hint(&mut dot, dot_title(chart, ser, xd, yd));
        out.push(dot);
    }
}

/// Whether a datum's data coords lie inside both axes' domains (crop, [SPEC 6]).
pub(super) fn in_domain(chart: &Chart, ser: &Series, x: f64, y: f64) -> bool {
    chart.x.scale.contains(x) && chart.values[ser.axis].scale.contains(y)
}

fn dot_title(chart: &Chart, ser: &Series, x: f64, y: f64) -> String {
    let name = ser.label.as_deref().unwrap_or("");
    let value = match &ser.data {
        Data::Categorical(_) => match (chart.x.labels.get(x as usize), &chart.x.scale) {
            (Some(c), Scale::Band { .. }) => format!("{c}: {}", format::render(y, ser.fmt)),
            _ => format::render(y, ser.fmt),
        },
        // Points (incl. a sampled formula): the x,y pair.
        _ => format!(
            "{}, {}",
            format::render(x, chart.x.fmt),
            format::render(y, ser.fmt)
        ),
    };
    if name.is_empty() {
        value
    } else {
        format!("{name}: {value}")
    }
}

/// The pixel polyline a curve draws through `pts` [SPEC 14.2]: straight
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
/// or sub-baseline dip [SPEC 14.2]. Assumes x is monotonic (line / fn data).
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

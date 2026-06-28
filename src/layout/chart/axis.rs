//! Axis rendering ([CHARTS.md] §5/§9): gridlines (drawn behind the data), then the
//! tick labels and axis titles. The primary value axis and a numeric x axis draw
//! gridlines by default; `gridlines: none | colour` overrides per axis.

use super::model::{Chart, Grid, Side, ValueAxis};
use super::prim;
use super::project::Plot;
use super::scale::{self, Scale};
use crate::layout::PlacedNode;
use crate::resolve::ResolvedValue;

const LABEL_SIZE: f64 = 11.0;
const TITLE_SIZE: f64 = 11.0;

fn live(name: &str) -> ResolvedValue {
    ResolvedValue::LiveVar {
        name: name.into(),
        raw: false,
    }
}

fn muted() -> ResolvedValue {
    live("muted")
}

/// All gridlines, drawn first so the data sits over them.
pub fn gridlines(plot: &Plot, chart: &Chart, out: &mut Vec<PlacedNode>) {
    for axis in &chart.values {
        if let Some(color) = value_grid(axis) {
            for &t in axis.scale.ticks() {
                let y = plot.y_at(&axis.scale, t);
                out.push(prim::line(
                    vec![(plot.x0, y), (plot.x1, y)],
                    color.clone(),
                    1.0,
                ));
            }
        }
    }
    if let Some(color) = x_grid(&chart.x.grid, &chart.x.scale) {
        for &t in chart.x.scale.ticks() {
            let x = plot.x_at(&chart.x.scale, t);
            out.push(prim::line(
                vec![(x, plot.y0), (x, plot.y1)],
                color.clone(),
                1.0,
            ));
        }
    }
}

/// Tick labels and axis titles for every axis.
pub fn labels(plot: &Plot, chart: &Chart, out: &mut Vec<PlacedNode>) {
    for axis in &chart.values {
        value_labels(plot, axis, out);
    }
    x_labels(plot, chart, out);
    if let Some(t) = &chart.x.title {
        let y = plot.y1 + LABEL_SIZE * 1.4 + TITLE_SIZE;
        out.push(prim::text(
            t,
            (plot.x0 + plot.x1) / 2.0,
            y,
            TITLE_SIZE,
            Some(muted()),
            false,
        ));
    }
}

fn value_labels(plot: &Plot, axis: &ValueAxis, out: &mut Vec<PlacedNode>) {
    for &t in axis.scale.ticks() {
        let y = plot.y_at(&axis.scale, t);
        let label = scale::label(t, &axis.unit);
        let node = match axis.side {
            Side::Right => prim::text_left(&label, plot.x1 + 6.0, y, LABEL_SIZE, Some(muted())),
            _ => prim::text_right(&label, plot.x0 - 6.0, y, LABEL_SIZE, Some(muted())),
        };
        out.push(node);
    }
    // The axis title sits above the axis, aligned to its side.
    if let Some(title) = &axis.title {
        let y = plot.y0 - 6.0;
        let node = match axis.side {
            Side::Right => prim::text_right(title, plot.x1, y, TITLE_SIZE, Some(muted())),
            _ => prim::text_left(title, plot.x0, y, TITLE_SIZE, Some(muted())),
        };
        out.push(node);
    }
}

fn x_labels(plot: &Plot, chart: &Chart, out: &mut Vec<PlacedNode>) {
    let y = plot.y1 + 4.0 + LABEL_SIZE * 0.7;
    match &chart.x.scale {
        Scale::Band { n } => {
            for i in 0..*n {
                let label = chart
                    .x
                    .labels
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| (i + 1).to_string());
                out.push(prim::text(
                    &label,
                    plot.x_at(&chart.x.scale, i as f64),
                    y,
                    LABEL_SIZE,
                    Some(muted()),
                    false,
                ));
            }
        }
        Scale::Linear { .. } => {
            for &t in chart.x.scale.ticks() {
                let label = scale::label(t, &chart.x.unit);
                out.push(prim::text(
                    &label,
                    plot.x_at(&chart.x.scale, t),
                    y,
                    LABEL_SIZE,
                    Some(muted()),
                    false,
                ));
            }
        }
    }
}

/// The gridline colour for a value axis: an explicit tint, the faint default for the
/// primary axis, else none (a secondary axis adds none, avoiding moiré — §5).
fn value_grid(axis: &ValueAxis) -> Option<ResolvedValue> {
    match &axis.grid {
        Grid::Color(c) => Some(c.clone()),
        Grid::Off => None,
        Grid::Default => axis.primary.then(|| live("grid")),
    }
}

/// The gridline colour for the x axis: an explicit tint, the faint default for a
/// numeric x (vertical lines at ticks), none for a categorical band.
fn x_grid(grid: &Grid, scale: &Scale) -> Option<ResolvedValue> {
    match grid {
        Grid::Color(c) => Some(c.clone()),
        Grid::Off => None,
        Grid::Default => matches!(scale, Scale::Linear { .. }).then(|| live("grid")),
    }
}

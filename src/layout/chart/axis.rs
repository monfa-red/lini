//! The value axis (horizontal gridlines + right-aligned tick labels) and the x
//! category labels ([CHARTS.md] §5). One tick/label renderer — the later axis work
//! (explicit `|axis|` children, sides, log) generalises this rather than copying it.

use super::prim;
use super::project::Plot;
use super::scale::{Scale, fmt_tick};
use crate::layout::PlacedNode;
use crate::resolve::ResolvedValue;

const LABEL_SIZE: f64 = 11.0;

fn grid() -> ResolvedValue {
    ResolvedValue::LiveVar {
        name: "grid".into(),
        raw: false,
    }
}

fn muted() -> ResolvedValue {
    ResolvedValue::LiveVar {
        name: "muted".into(),
        raw: false,
    }
}

/// A horizontal gridline spanning the plot plus a right-aligned label at every value
/// tick. The `--lini-grid` role var keeps the lines faint and dark/light-aware.
pub fn value_axis(plot: &Plot, scale: &Scale, out: &mut Vec<PlacedNode>) {
    for &t in &scale.ticks {
        let y = plot.y(t);
        out.push(prim::line(vec![(plot.x0, y), (plot.x1, y)], grid(), 1.0));
        out.push(prim::text_right(
            &fmt_tick(t),
            plot.x0 - 6.0,
            y,
            LABEL_SIZE,
            Some(muted()),
        ));
    }
}

/// Category (x-axis) labels, centred under each slot. Falls back to 1-based indices
/// when no `categories:` were given ([CHARTS.md] §5).
pub fn x_labels(plot: &Plot, categories: &[String], out: &mut Vec<PlacedNode>) {
    let y = plot.baseline() + 4.0 + LABEL_SIZE * 0.7;
    for i in 0..plot.n {
        let label = categories
            .get(i)
            .cloned()
            .unwrap_or_else(|| (i + 1).to_string());
        out.push(prim::text(
            &label,
            plot.slot_center(i),
            y,
            LABEL_SIZE,
            Some(muted()),
            false,
        ));
    }
}

//! Plot-rect geometry [SPEC 14.7]: the label / title / legend gutters a chart
//! reserves, and the column / row / radial plot rects inset from them.

use super::*;

/// The plot rect = the chart box inset by the gutters its labels / titles / legend
/// need, all measured at compile time [SPEC 5].
pub(super) fn plot_rect(chart: &Chart, w: f64, h: f64) -> Plot {
    if chart.dir == Dir::Radial {
        return radial_plot(chart, w, h);
    }
    if chart.dir == Dir::Row {
        return row_plot(chart, w, h);
    }
    let left = nonzero(side_gutter(chart, false), 12.0);
    let right = nonzero(side_gutter(chart, true), 12.0);
    let title_h = title_reserve(chart.title.is_some(), chart.gap);
    let value_title_h = if chart.values.iter().any(|a| a.title.is_some()) {
        AXIS_TITLE_SIZE * 1.4
    } else {
        0.0
    };
    let x_title_h = if chart.x.title.is_some() {
        AXIS_TITLE_SIZE * 1.4
    } else {
        0.0
    };
    let legend_h = legend_reserve(legend_entries(chart).len(), chart.gap);
    let band_row = annot::x_band_row(chart);
    Plot {
        x0: -w / 2.0 + left,
        x1: w / 2.0 - right,
        y0: -h / 2.0 + 8.0 + title_h + value_title_h,
        y1: h / 2.0 - 6.0 - LABEL_SIZE * 1.4 - band_row - x_title_h - legend_h,
        dir: chart.dir,
    }
}

/// A row chart's plot rect: the cartesian flip [SPEC 14.7]. The domain
/// (category) labels sit on the left and the value labels along the bottom, so the
/// gutters swap — a left gutter sized to the widest category, a bottom gutter for the
/// value labels and axis title.
fn row_plot(chart: &Chart, w: f64, h: f64) -> Plot {
    let title_h = title_reserve(chart.title.is_some(), chart.gap);
    let value_title_h = if chart.values.iter().any(|a| a.title.is_some()) {
        AXIS_TITLE_SIZE * 1.4
    } else {
        0.0
    };
    let legend_h = legend_reserve(legend_entries(chart).len(), chart.gap);
    let left = nonzero(domain_gutter(chart), 12.0);
    Plot {
        x0: -w / 2.0 + left,
        x1: w / 2.0 - 12.0,
        y0: -h / 2.0 + 8.0 + title_h,
        y1: h / 2.0 - 6.0 - LABEL_SIZE * 1.4 - value_title_h - legend_h,
        dir: Dir::Row,
    }
}

/// The left-gutter width for a row chart's domain (category) labels.
fn domain_gutter(chart: &Chart) -> f64 {
    let maxw = chart
        .x
        .labels
        .iter()
        .map(|l| prim::text_width(l, LABEL_SIZE, crate::font::Font::regular(chart.font_kind)))
        .fold(0.0_f64, f64::max);
    if maxw > 0.0 { maxw + 8.0 } else { 0.0 }
}

/// A radial chart's plot rect: a centred square (the spoke-circle's bounding box),
/// inset from the chart box by the title (top), legend (bottom), and a margin all
/// round for the spoke labels that sit just outside the rim [SPEC 14.7].
fn radial_plot(chart: &Chart, w: f64, h: f64) -> Plot {
    let title_h = title_reserve(chart.title.is_some(), chart.gap);
    let legend_h = legend_reserve(legend_entries(chart).len(), chart.gap);
    let margin = LABEL_SIZE * 2.0;
    let top = title_h + margin;
    let avail_h = h - top - legend_h - margin;
    let side = avail_h.min(w - 2.0 * margin).max(0.0);
    let cy = -h / 2.0 + top + avail_h / 2.0;
    Plot {
        x0: -side / 2.0,
        x1: side / 2.0,
        y0: cy - side / 2.0,
        y1: cy + side / 2.0,
        dir: Dir::Radial,
    }
}

/// The label-width gutter for the value axes on one side (`right` = right edge), or
/// 0 if no value axis sits there.
fn side_gutter(chart: &Chart, right: bool) -> f64 {
    let mut maxw = 0.0_f64;
    let mut any = false;
    for axis in chart
        .values
        .iter()
        .filter(|a| matches!(a.side, Side::Right) == right)
    {
        any = true;
        for &t in axis.scale.ticks() {
            maxw = maxw.max(prim::text_width(
                &scale::label(t, &axis.unit),
                LABEL_SIZE,
                crate::font::Font::regular(chart.font_kind),
            ));
        }
    }
    if any { maxw + 10.0 } else { 0.0 }
}

fn nonzero(v: f64, fallback: f64) -> f64 {
    if v > 0.0 { v } else { fallback }
}

/// Space reserved above the plot for the title [SPEC 14.6]: its drawn height plus
/// the chart's `gap` (the clear gutter to the plot), or 0 with no title. The one place
/// the title inset is computed — column, row, radial, and pie all call it, so `gap:`
/// tunes the spacing identically everywhere (`gap: 0` ≈ touching).
pub(super) fn title_reserve(has_title: bool, gap: f64) -> f64 {
    if has_title {
        TITLE_SIZE * 1.2 + gap
    } else {
        0.0
    }
}

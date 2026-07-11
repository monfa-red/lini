//! Legend geometry [SPEC 14.6]: the series' legend entries and the centred
//! swatch-and-label row shared by chart and pie.

use super::*;

/// Space reserved below the plot for the legend [SPEC 14.6]: its band plus the
/// chart's `gap`, or 0 below two entries — shared with the title by `gap`
/// ([`title_reserve`]).
pub(super) fn legend_reserve(entries: usize, gap: f64) -> f64 {
    if entries >= 2 {
        LABEL_SIZE * 0.7 + gap
    } else {
        0.0
    }
}

/// A legend entry [SPEC 14.6]: its label, the swatch **fill**, and an optional
/// swatch **edge** — so the swatch mirrors a series' paint (an outlined bar / slice gets
/// an outlined swatch, a flat one a flat swatch).
pub(super) type LegendEntry = (String, ResolvedValue, Option<ResolvedValue>);

/// The legend entries — one per series that carries a label (no label → no entry,
/// [SPEC 14.6]). The swatch wears the series' fill and edge ([`swatch_edge`]).
pub(super) fn legend_entries(chart: &Chart) -> Vec<LegendEntry> {
    chart
        .series
        .iter()
        .filter_map(|s| {
            s.label
                .clone()
                .map(|l| (l, s.color.clone(), swatch_edge(s)))
        })
        .collect()
}

/// The edge a series' legend swatch should wear, mirroring what it draws [SPEC 14.6]:
/// a bar / slice its (default-deep or explicit) outline, an area its always-drawn deep
/// edge, a line / dots usually nothing.
fn swatch_edge(s: &Series) -> Option<ResolvedValue> {
    let explicit = s.outline.as_ref().map(|(c, _)| c.clone());
    match s.kind {
        SeriesKind::Area => Some(explicit.unwrap_or_else(|| palette::deepen(&s.color))),
        _ => explicit,
    }
}

/// A centred row of swatch + label entries at vertical `cy`. Shared by chart and pie.
pub(super) fn lay_out_legend(entries: &[LegendEntry], cy: f64, out: &mut Vec<PlacedNode>) {
    const SW: f64 = 11.0; // swatch side
    const GAP: f64 = 5.0; // swatch → label
    const ITEM_GAP: f64 = 16.0; // entry → entry
    let widths: Vec<f64> = entries
        .iter()
        .map(|(l, _, _)| prim::text_width(l, LABEL_SIZE))
        .collect();
    let per: f64 = widths.iter().map(|w| SW + GAP + w).sum();
    let total = per + ITEM_GAP * widths.len().saturating_sub(1) as f64;
    let mut x = -total / 2.0;
    for ((label, fill, edge), &tw) in entries.iter().zip(&widths) {
        let mut swatch = prim::rect(x + SW / 2.0, cy, SW, SW, fill.clone(), 1.0);
        prim::round(&mut swatch, 2.0); // soft swatch corners [SPEC 14.6]
        if let Some(edge) = edge {
            prim::outline(&mut swatch, edge.clone(), 1.0); // mirror the series' edge
        }
        out.push(swatch);
        // The legend stays bold (the chart's chrome), like the title [SPEC 14.6].
        out.push(prim::text(
            label,
            x + SW + GAP + tw / 2.0,
            cy,
            LABEL_SIZE,
            None,
            true,
        ));
        x += SW + GAP + tw + ITEM_GAP;
    }
}

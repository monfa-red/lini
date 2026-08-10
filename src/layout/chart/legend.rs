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

/// One legend entry for a labelled, painted thing — a series or a pie slice
/// [SPEC 14.6]. No label → no entry. The swatch mirrors what the thing draws: its
/// fill, and its explicit `stroke:` edge — or, when the shape is always drawn with
/// a deep edge (`deep_edge`, an `|area|`), that fallback edge.
pub(super) fn entry(
    label: &Option<String>,
    fill: &ResolvedValue,
    outline: &Option<(ResolvedValue, f64)>,
    deep_edge: bool,
) -> Option<LegendEntry> {
    let explicit = outline.as_ref().map(|(c, _)| c.clone());
    let edge = if deep_edge {
        Some(explicit.unwrap_or_else(|| palette::deepen(fill)))
    } else {
        explicit
    };
    label.clone().map(|l| (l, fill.clone(), edge))
}

/// The legend entries — one per series that carries a label ([`entry`]).
pub(super) fn legend_entries(chart: &Chart) -> Vec<LegendEntry> {
    chart
        .series
        .iter()
        .filter_map(|s| {
            entry(
                &s.label,
                &s.color,
                &s.outline,
                matches!(s.kind, SeriesKind::Area),
            )
        })
        .collect()
}

/// A centred row of swatch + label entries at vertical `cy`. Shared by chart and pie.
pub(super) fn lay_out_legend(
    entries: &[LegendEntry],
    cy: f64,
    kind: crate::font::Kind,
    out: &mut Vec<PlacedNode>,
) {
    const SW: f64 = 11.0; // swatch side
    const GAP: f64 = 5.0; // swatch → label
    const ITEM_GAP: f64 = 16.0; // entry → entry
    let widths: Vec<f64> = entries
        .iter()
        // Measured at the weight `prim::text(bold)` actually renders — semibold, the
        // chart's chrome [SPEC 14.6]; measuring at bold drifts the swatches on a
        // proportional family.
        .map(|(l, _, _)| prim::text_width(l, LABEL_SIZE, crate::font::Font::semibold(kind)))
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
            kind,
        ));
        x += SW + GAP + tw + ITEM_GAP;
    }
}

//! Inline data labels ([CHARTS.md] §14): a series' `tags:` drawn on the plot beside their
//! points, positioned by one greedy, deterministic pass. Each label takes the first
//! candidate offset that clears the labels already placed and stays inside the plot; an
//! `auto` label with nowhere to sit is dropped (its hover card still carries the tag), an
//! `always` label is forced. Fast and order-stable — O(labels²) over the *sparse* data
//! points, never the iterative relaxation links route with. This is the one home for "text
//! beside a chart point"; series tags feed it here, with bubbles / marks routed in next to
//! them, so every point label is placed by the same rule.

use super::marks;
use super::model::Chart;
use super::prim;
use super::project::Plot;
use crate::layout::PlacedNode;
use crate::resolve::ResolvedValue;

/// Inline-label font size — small, in the register of a link label ([CHARTS.md] §14).
const SIZE: f64 = 10.0;
/// Clearance from a point to its label box (clearing the marker), and the margin folded
/// into a box for label-vs-label / label-vs-edge spacing.
const GAP: f64 = 7.0;
const PAD: f64 = 2.0;

/// One label to place ([CHARTS.md] §14): the point it annotates (pixels), its text and
/// tint, and whether it is `forced` (`always` — placed regardless) or may drop to its
/// hover card (`auto`).
pub(super) struct Req {
    pub anchor: (f64, f64),
    pub text: String,
    pub color: ResolvedValue,
    pub forced: bool,
}

/// Collect the inline-label requests a chart's series raise ([CHARTS.md] §14): each
/// `tags:` entry on a series whose `tooltip:` shows inline, anchored on the datum's pixel
/// point. Reuses `marks::samples`, so a tag sits on exactly the point its marker does.
pub(super) fn collect(plot: &Plot, chart: &Chart) -> Vec<Req> {
    let mut reqs = Vec::new();
    for ser in &chart.series {
        if ser.tags.is_empty() || !ser.tooltip.inline() {
            continue;
        }
        for (((xd, yd), (xp, yp)), tag) in marks::samples(plot, chart, ser).iter().zip(&ser.tags) {
            if tag.is_empty() || !marks::in_domain(chart, ser, *xd, *yd) {
                continue;
            }
            reqs.push(Req {
                anchor: (*xp, *yp),
                text: tag.clone(),
                color: ser.tag_color.clone(),
                forced: ser.tooltip.forced(),
            });
        }
    }
    reqs
}

/// Place the requests and emit their text nodes ([CHARTS.md] §14). Greedy: each label
/// takes the first candidate offset that clears every label already placed and stays in
/// the plot; failing that, an `always` label keeps the first in-plot offset (else its
/// preferred one) while an `auto` label is dropped to its hover card. Deterministic —
/// source order in, the same candidate order each time.
pub(super) fn place(reqs: &[Req], plot: &Plot) -> Vec<PlacedNode> {
    let mut placed: Vec<Rect> = Vec::new();
    let mut out = Vec::new();
    for req in reqs {
        let w = prim::text_width(&req.text, SIZE);
        let h = prim::text_height(&req.text, SIZE);
        let cands = candidates(req.anchor, w, h);
        let free = cands.iter().copied().find(|&c| {
            let r = Rect::around(c, w, h);
            r.within(plot) && placed.iter().all(|p| !p.hits(&r))
        });
        let center = match free {
            Some(c) => c,
            None if req.forced => cands
                .iter()
                .copied()
                .find(|&c| Rect::around(c, w, h).within(plot))
                .unwrap_or(cands[0]),
            None => continue, // auto: the tag lives on in the hover card
        };
        placed.push(Rect::around(center, w, h));
        let mut t = prim::text(
            &req.text,
            center.0,
            center.1,
            SIZE,
            Some(req.color.clone()),
            false,
        );
        t.type_chain.push("chart-label".to_string());
        out.push(t);
    }
    out
}

/// Candidate label-box centres around a point, in priority order: above first (the
/// conventional data-label seat), then below, the sides, then the diagonals — enough
/// freedom for the greedy pass to fan a cluster out without a solver.
fn candidates((ax, ay): (f64, f64), w: f64, h: f64) -> [(f64, f64); 8] {
    let dx = w / 2.0 + GAP;
    let dy = h / 2.0 + GAP;
    [
        (ax, ay - dy),      // above
        (ax, ay + dy),      // below
        (ax + dx, ay),      // right
        (ax - dx, ay),      // left
        (ax + dx, ay - dy), // up-right
        (ax - dx, ay - dy), // up-left
        (ax + dx, ay + dy), // down-right
        (ax - dx, ay + dy), // down-left
    ]
}

/// An axis-aligned label box, inflated by [`PAD`] so placed labels keep a hair of air
/// from each other and the plot edge.
struct Rect {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Rect {
    fn around((cx, cy): (f64, f64), w: f64, h: f64) -> Rect {
        let (hw, hh) = (w / 2.0 + PAD, h / 2.0 + PAD);
        Rect {
            x0: cx - hw,
            y0: cy - hh,
            x1: cx + hw,
            y1: cy + hh,
        }
    }

    /// Whether the box sits fully inside the plot rect (no spill over the axes / edge).
    fn within(&self, plot: &Plot) -> bool {
        self.x0 >= plot.x0 && self.x1 <= plot.x1 && self.y0 >= plot.y0 && self.y1 <= plot.y1
    }

    /// Whether two label boxes overlap.
    fn hits(&self, o: &Rect) -> bool {
        self.x0 < o.x1 && o.x0 < self.x1 && self.y0 < o.y1 && o.y0 < self.y1
    }
}

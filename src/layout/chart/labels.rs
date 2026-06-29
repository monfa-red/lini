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
/// the tint it takes when placed *beside* the point, whether it is `forced` (`always` —
/// placed regardless) or may drop to its hover card (`auto`), and an optional `inside`
/// seat (a bubble label sits centred in the bubble when it fits).
pub(super) struct Req {
    pub anchor: (f64, f64),
    pub text: String,
    pub color: ResolvedValue,
    pub forced: bool,
    pub inside: Option<Inside>,
}

/// A bubble's first choice ([CHARTS.md] §14): when the text fits within `fit` (the
/// bubble's diameter) the label sits centred *inside*, tinted `color` (the on-fill role);
/// otherwise it falls through to the outside seats with the request's own tint.
pub(super) struct Inside {
    pub fit: f64,
    pub color: ResolvedValue,
}

/// Append the inline-label requests a chart's series raise ([CHARTS.md] §14): each
/// `tags:` entry on a series whose `tooltip:` shows inline, anchored on the datum's pixel
/// point. Reuses `marks::samples`, so a tag sits on exactly the point its marker does.
/// (`|bubble|` / `|mark|` push their own reqs as they lay out — the same `reqs` list, so
/// every point label dedups against every other.)
pub(super) fn collect_series(plot: &Plot, chart: &Chart, reqs: &mut Vec<Req>) {
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
                inside: None,
            });
        }
    }
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
        // Seats to try, in order, each with the tint it would wear: a bubble's inside seat
        // first (when the text fits), then the offsets beside the point.
        let mut seats: Vec<((f64, f64), &ResolvedValue)> = Vec::new();
        if let Some(ins) = &req.inside
            && w <= ins.fit
        {
            seats.push((req.anchor, &ins.color));
        }
        seats.extend(candidates(req.anchor, w, h).map(|c| (c, &req.color)));

        // Pick a seat (the borrow of `placed` is confined to this block, freed before the
        // push below). Greedy: the first clear, in-plot seat; a forced label falls back to
        // the first in-plot seat even if it collides, then to its anchor.
        let pick: Option<((f64, f64), ResolvedValue)> = {
            let clear = |c: (f64, f64)| {
                let r = Rect::around(c, w, h);
                r.within(plot) && placed.iter().all(|p| !p.hits(&r))
            };
            let chosen = seats.iter().find(|(c, _)| clear(*c)).or_else(|| {
                req.forced
                    .then(|| {
                        seats
                            .iter()
                            .find(|(c, _)| Rect::around(*c, w, h).within(plot))
                    })
                    .flatten()
            });
            match chosen {
                Some(&(c, col)) => Some((c, col.clone())),
                None if req.forced => Some((req.anchor, req.color.clone())),
                None => None, // auto: the label lives on in the hover card
            }
        };
        let Some((center, color)) = pick else {
            continue;
        };
        placed.push(Rect::around(center, w, h));
        let mut t = prim::text(&req.text, center.0, center.1, SIZE, Some(color), false);
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

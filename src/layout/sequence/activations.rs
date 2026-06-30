//! Activations (SPEC §10): the implicit bars showing when a participant is handling a
//! call. A call (`->`) **opens** a bar on its target's lifeline; the next **return**
//! (`-->`) from that target **closes** its most recent open one; nested calls **stack**,
//! each offset outward; an unclosed bar runs to the foot. Self / async messages open
//! none. The bars lower to thin `|block|`s through [`crate::layout::prim`], and message
//! endpoints attach to a live bar's edge ([`edge`]) so arrows meet the bar, not the line.

use super::messages::{Kind, Pair};
use crate::layout::PlacedNode;
use crate::layout::prim;
use std::collections::HashMap;

/// A bar's width on the lifeline.
const BAR_W: f64 = 10.0;
/// Each nesting level shifts the bar outward (to the right) so stacked calls read.
const NEST_DX: f64 = 4.0;

/// One activation bar: the participant it sits on, the rows it spans (open → close),
/// and its nesting depth (0 = on the lifeline, deeper = offset outward).
pub(super) struct Bar {
    participant: String,
    open_row: usize,
    close_row: usize,
    depth: usize,
}

/// Compute the bars from the time-ordered pairs — each pair's row is its index. A
/// per-participant LIFO stack: a call pushes a bar on its target, a return pops its
/// source's top (an orphan return pops nothing). An unclosed bar runs to the foot
/// (`close_row == pairs.len()`, whose `row_y` is the foot).
pub(super) fn bars(pairs: &[Pair]) -> Vec<Bar> {
    let mut open: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut bars: Vec<Bar> = Vec::new();
    for (row, p) in pairs.iter().enumerate() {
        match p.kind() {
            Kind::Call => {
                let stack = open.entry(p.to).or_default();
                bars.push(Bar {
                    participant: p.to.to_string(),
                    open_row: row,
                    close_row: pairs.len(),
                    depth: stack.len(),
                });
                stack.push(bars.len() - 1);
            }
            Kind::Return => {
                if let Some(idx) = open.get_mut(p.from).and_then(Vec::pop) {
                    bars[idx].close_row = row;
                }
            }
            Kind::Async | Kind::Self_ => {}
        }
    }
    bars
}

/// The x where a message endpoint on `id` at `row` should attach: the facing edge of the
/// outermost bar live across that row (a call meets the bar it opens; a mid-call message
/// meets the bar's side), or `None` when no bar is live (attach to the lifeline centre).
/// `toward` is the other endpoint's x, picking the near edge.
pub(super) fn edge(
    bars: &[Bar],
    id: &str,
    row: usize,
    lifeline_cx: f64,
    toward: f64,
) -> Option<f64> {
    let bar = bars
        .iter()
        .filter(|b| b.participant == id && b.open_row <= row && row <= b.close_row)
        .max_by_key(|b| b.depth)?;
    let bar_cx = lifeline_cx + bar.depth as f64 * NEST_DX;
    let half = BAR_W / 2.0;
    Some(if toward < bar_cx {
        bar_cx - half
    } else {
        bar_cx + half
    })
}

/// Lower each bar to a thin `|block|` on its participant's lifeline (SPEC §10 — `fill:
/// --fill`, outlined in the sequence's own `stroke` / `stroke-width` so it matches the
/// lifelines), spanning its open → close rows.
pub(super) fn draw(
    bars: &[Bar],
    lifeline_x: &HashMap<String, f64>,
    row_y: impl Fn(usize) -> f64,
    stroke: crate::resolve::ResolvedValue,
    width: f64,
) -> Vec<PlacedNode> {
    bars.iter()
        .filter_map(|b| {
            let cx = lifeline_x.get(&b.participant)? + b.depth as f64 * NEST_DX;
            let (top, bot) = (row_y(b.open_row), row_y(b.close_row));
            let mut bar = prim::rect(
                cx,
                (top + bot) / 2.0,
                BAR_W,
                (bot - top).max(1.0),
                super::live("fill"),
                1.0,
            );
            prim::outline(&mut bar, stroke.clone(), width);
            Some(bar)
        })
        .collect()
}

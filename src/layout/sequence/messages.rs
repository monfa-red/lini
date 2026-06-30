//! Sequence messages (SPEC §10): a link in a sequence scope, lowered to a horizontal
//! **time-row arrow** between two lifelines through [`crate::layout::prim`]. A message is a
//! `|line|` carrying the link's already-resolved paint (`stroke*`, mapped from `link*` at
//! resolve) and end marker (the arrowhead). A chain `a -> b -> c` splits into consecutive
//! pairs, each its own row; fans are already separate links.

use crate::ast::LineStyle;
use crate::layout::PlacedNode;
use crate::layout::prim;
use crate::resolve::ResolvedLink;
use std::collections::HashMap;

/// Link-label default size (SPEC §9), sitting just above the arrow.
const LABEL_SIZE: f64 = 11.0;
/// Clear space above the arrow for its label.
const LABEL_RISE: f64 = 5.0;
/// Clear space a label wants beyond its text when spacing participants.
const LABEL_MARGIN: f64 = 16.0;
/// A self-message hook's width and depth.
const SELF_DX: f64 = 26.0;
const SELF_DY: f64 = 16.0;

/// One drawn message: a pair of participants (by id) and the link it came from (its paint,
/// markers, and label). A chain `a -> b -> c` is two pairs.
pub(super) struct Pair<'a> {
    pub from: &'a str,
    pub to: &'a str,
    link: &'a ResolvedLink,
}

/// A message's kind on the time axis (SPEC §10), read from the operator — not from
/// `stroke-style`, which a `link-style:` override can change. It drives activations
/// (a call opens a bar, a return closes one; async / self open none).
#[derive(PartialEq, Eq, Clone, Copy)]
pub(super) enum Kind {
    Call,
    Return,
    Async,
    Self_,
}

impl Pair<'_> {
    fn label(&self) -> Option<&str> {
        self.link.texts.first().map(|t| t.text.as_str())
    }
    fn label_width(&self) -> f64 {
        self.label()
            .map_or(0.0, |l| prim::text_width(l, LABEL_SIZE))
    }
    /// This message's kind: a self-message (`a -> a`) regardless of operator, else by
    /// the operator's line (`~>` async · `-->` return · `->` / other call).
    pub(super) fn kind(&self) -> Kind {
        if self.from == self.to {
            Kind::Self_
        } else {
            match self.link.line {
                LineStyle::Wavy => Kind::Async,
                LineStyle::Dashed => Kind::Return,
                _ => Kind::Call,
            }
        }
    }
}

/// Flatten each scope message into consecutive participant pairs, in time order (the
/// messages arrive span-sorted; a chain's pairs keep their order).
pub(super) fn pairs<'a>(messages: &[&'a ResolvedLink]) -> Vec<Pair<'a>> {
    let mut out = Vec::new();
    for w in messages {
        for win in w.endpoints.windows(2) {
            out.push(Pair {
                from: leaf(&win[0].path),
                to: leaf(&win[1].path),
                link: w,
            });
        }
    }
    out
}

/// A participant is a direct child of the sequence, so an endpoint's last path segment is
/// its id (SPEC §10 — a message resolves to a participant).
fn leaf(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or(path)
}

/// Column x-centres for the participants, widened so a message label fits over its span
/// (SPEC §10: adjacent lifelines sit `max(gap-col, label + margin)` apart). Greedy and
/// deterministic — each message, in time order, widens the gaps it spans if its label
/// doesn't fit; the centres are then balanced on the origin.
pub(super) fn columns(widths: &[f64], ids: &[&str], pairs: &[Pair], gap_col: f64) -> Vec<f64> {
    let n = widths.len();
    if n == 0 {
        return Vec::new();
    }
    let col: HashMap<&str, usize> = ids.iter().enumerate().map(|(i, &id)| (id, i)).collect();
    let half = |i: usize| widths[i] / 2.0;
    let mut gaps = vec![gap_col; n.saturating_sub(1)];
    for p in pairs {
        let (Some(&a), Some(&b)) = (col.get(p.from), col.get(p.to)) else {
            continue;
        };
        if a == b {
            continue; // a self-message needs no inter-lifeline room
        }
        let (lo, hi) = (a.min(b), a.max(b));
        let mut dist = half(lo) + half(hi);
        (lo + 1..hi).for_each(|k| dist += widths[k]);
        gaps[lo..hi].iter().for_each(|g| dist += g);
        let needed = p.label_width() + LABEL_MARGIN;
        if needed > dist {
            let add = (needed - dist) / (hi - lo) as f64;
            gaps[lo..hi].iter_mut().for_each(|g| *g += add);
        }
    }
    // Cumulative centres, then balance the whole row on the origin.
    let mut centres = Vec::with_capacity(n);
    let mut x = 0.0;
    for (i, &w) in widths.iter().enumerate() {
        x += w / 2.0;
        centres.push(x);
        x += w / 2.0;
        if let Some(g) = gaps.get(i) {
            x += g;
        }
    }
    let shift = x / 2.0;
    centres.iter().map(|c| c - shift).collect()
}

/// Draw the messages: each pair is a horizontal arrow at its row carrying the link's paint
/// and end marker, label centred above. A self-message (`a -> a`) is a hook on the lifeline,
/// label to the right. `lifeline_x` gives each participant's centre (for direction and label
/// placement); `endpoint_x(id, row, toward)` gives the actual attach x — a live activation
/// bar's edge, or the lifeline centre — so an arrow meets the bar it opens (SPEC §10).
pub(super) fn draw(
    pairs: &[Pair],
    lifeline_x: &HashMap<String, f64>,
    endpoint_x: impl Fn(&str, usize, f64) -> f64,
    row_y: impl Fn(usize) -> f64,
) -> Vec<PlacedNode> {
    let mut out = Vec::new();
    for (i, p) in pairs.iter().enumerate() {
        let (Some(&fcx), Some(&tcx)) = (lifeline_x.get(p.from), lifeline_x.get(p.to)) else {
            continue;
        };
        let y = row_y(i);
        let stroke = p
            .link
            .attrs
            .get("stroke")
            .cloned()
            .unwrap_or_else(|| super::live("stroke"));
        let width = p.link.attrs.number("stroke-width").unwrap_or(2.0);
        if fcx == tcx {
            // A self-message: a hook on the lifeline, off its near (right) bar edge.
            let fx = endpoint_x(p.from, i, fcx + 1.0);
            let mut hook = prim::line(
                vec![
                    (fx, y),
                    (fx + SELF_DX, y),
                    (fx + SELF_DX, y + SELF_DY),
                    (fx, y + SELF_DY),
                ],
                stroke,
                width,
            );
            style(&mut hook, p.link);
            out.push(hook);
            if let Some(label) = p.label() {
                let lx = fx + SELF_DX + 6.0 + prim::text_width(label, LABEL_SIZE) / 2.0;
                out.push(prim::text(
                    label,
                    lx,
                    y + SELF_DY / 2.0,
                    LABEL_SIZE,
                    None,
                    false,
                ));
            }
        } else {
            let fx = endpoint_x(p.from, i, tcx);
            let tx = endpoint_x(p.to, i, fcx);
            let mut arrow = prim::line(vec![(fx, y), (tx, y)], stroke, width);
            style(&mut arrow, p.link);
            out.push(arrow);
            if let Some(label) = p.label() {
                out.push(prim::text(
                    label,
                    (fcx + tcx) / 2.0,
                    y - LABEL_RISE - LABEL_SIZE / 2.0,
                    LABEL_SIZE,
                    None,
                    false,
                ));
            }
        }
    }
    out
}

/// Copy the message's resolved detail onto its lowered `|line|`: the end marker (the
/// arrowhead, from the operator) and the dash pattern. Stroke + width are already set by
/// `prim::line`, so paint stays one mechanism.
fn style(n: &mut PlacedNode, link: &ResolvedLink) {
    n.markers = link.markers.clone();
    if let Some(ss) = link.attrs.get("stroke-style") {
        n.attrs.insert("stroke-style", ss.clone());
        n.own_style.insert("stroke-style", ss.clone());
    }
}

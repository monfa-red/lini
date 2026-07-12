//! Sequence messages [SPEC 13]: a link in a sequence scope, lowered to a
//! horizontal **time-row arrow** between two lifelines through the `straight`
//! strategy ([`crate::routing::straight`]) — the layout owns *where* (column
//! x, row y), the strategy owns the wire. Each message becomes a
//! [`RoutedLink`] carrying the link's resolved paint, its end marker, and its
//! label as riding text; the renderer's one link path draws it (fillets round
//! the self-hook). A chain `a -> b -> c` splits into consecutive pairs, each
//! its own row; fans are already separate links.

use crate::ast::LineStyle;
use crate::layout::ir::{RoutedLink, RoutedText, SEQUENCE_MESSAGE_CLASS};
use crate::layout::prim;
use crate::ledger::consts::DEFAULT_CLEARANCE;
use crate::resolve::{AttrMap, ResolvedLink};
use crate::routing::straight;
use std::collections::HashMap;

/// Sequence message-label size — larger than the generic 11px link label so the messages
/// read comfortably on the time axis [SPEC 13]. The `.lini-sequence-message` stylesheet
/// rule ([`crate::render`]) states the rendered size from this constant.
pub(crate) const LABEL_SIZE: f64 = 13.0;
/// Clear space above the arrow for its label.
const LABEL_RISE: f64 = 5.0;
/// Clear space a label wants beyond its text when spacing participants.
const LABEL_MARGIN: f64 = 16.0;
/// A self-hook starts on the activation-bar edge (`BAR_W / 2` off the lifeline); its reach
/// (this + the hook width) is the arrow area that may widen the layout — the label never
/// does. Matching the bar half-width keeps a frame's right inset equal to its other sides.
const HOOK_BAR_EDGE: f64 = 5.0;
/// Clear space a self-hook keeps from the next lifeline.
const HOOK_MARGIN: f64 = 10.0;

/// One drawn message: a pair of participants (by id) and the link it came from (its paint,
/// markers, and label). A chain `a -> b -> c` is two pairs.
pub(super) struct Pair<'a> {
    pub from: &'a str,
    pub to: &'a str,
    link: &'a ResolvedLink,
}

/// A message's kind on the time axis [SPEC 13], read from the operator — not from
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
    /// The source span of the link this pair came from — its time position, used to
    /// interleave messages with frames and notes [SPEC 13].
    pub(super) fn span(&self) -> crate::span::Span {
        self.link.span
    }
    /// The two participants this message touches, for a frame's lifeline span.
    pub(super) fn ends(&self) -> (&str, &str) {
        (self.from, self.to)
    }
    /// A self-message (`a -> a`): a hook on one lifeline, not an inter-lifeline arrow.
    pub(super) fn is_self(&self) -> bool {
        self.from == self.to
    }
    /// This message's `clearance` [SPEC 9] — drives the self-hook's size and corner
    /// radius, so the loop honours the same turn rule as a routed wire.
    fn clearance(&self) -> f64 {
        self.link
            .attrs
            .number("clearance")
            .unwrap_or(DEFAULT_CLEARANCE)
    }
    /// A self-hook's `(width, depth)`, from the message's `clearance` — the
    /// corner radius falls out of the render-time fillet pass.
    fn hook(&self) -> (f64, f64) {
        let s = self.clearance().max(straight::HOOK_MIN);
        (s, s)
    }
    /// How far a self-hook reaches right of its lifeline centre — the arrow area that may
    /// widen the layout and its frame (the label rides above and reserves nothing). 0 for
    /// a normal message.
    pub(super) fn hook_reach(&self) -> f64 {
        if self.is_self() {
            HOOK_BAR_EDGE + self.hook().0
        } else {
            0.0
        }
    }
    /// Extra vertical room a self-message needs below its row — its hook drops by its depth,
    /// so the next message clears it (0 for a normal message, which lives on its row alone).
    pub(super) fn hook_drop(&self) -> f64 {
        if self.is_self() { self.hook().1 } else { 0.0 }
    }
    /// This message's label as riding text on its wire, centred at `(cx, cy)`
    /// — measured at the sequence label size, which the text carries so the
    /// renderer sizes it identically.
    fn text_at(&self, cx: f64, cy: f64) -> Option<RoutedText> {
        let label = self.label()?;
        // The size rides the `.lini-sequence-message` class (stated once), not an
        // inline per label; layout still measures at `label_size()` [SPEC 13].
        Some(RoutedText {
            content: label.to_owned(),
            position: (cx, cy),
            tangent: (1.0, 0.0),
            attrs: AttrMap::new(),
            class: SEQUENCE_MESSAGE_CLASS,
            applied_styles: Vec::new(),
        })
    }
    /// The label's font size — the sequence default ([`LABEL_SIZE`]), used to measure the
    /// label for column spacing and to bound its bbox. The rendered size rides the
    /// `.lini-sequence-message` stylesheet rule, not an inline style.
    fn label_size(&self) -> f64 {
        LABEL_SIZE
    }
    fn label_width(&self) -> f64 {
        self.label().map_or(0.0, |l| {
            prim::text_width(
                l,
                self.label_size(),
                crate::font::Font::of(&self.link.attrs),
            )
        })
    }
    /// This message's kind: a self-message (`a -> a`) regardless of operator, else by
    /// the operator's line (`~>` async · `-->` return · `->` / other call).
    pub(super) fn kind(&self) -> Kind {
        if self.is_self() {
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
/// its id ([SPEC 13] — a message resolves to a participant).
fn leaf(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or(path)
}

/// Column x-centres for the participants, widened so a message label fits over its span
/// ([SPEC 13]: adjacent lifelines sit `max(gap-col, label + margin)` apart). Greedy and
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
            // A self-message: reserve its hook's reach to the next lifeline (the arrow area
            // may widen the layout) so the loop never crosses it; the label rides above and
            // reserves nothing.
            if a + 1 < n {
                let need = p.hook_reach() + HOOK_MARGIN;
                let dist = half(a) + gaps[a] + half(a + 1);
                if need > dist {
                    gaps[a] += need - dist;
                }
            }
            continue;
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

/// Draw the messages: each pair is a horizontal wire at its row carrying the
/// link's paint, end marker, and label — a [`RoutedLink`] through the
/// `straight` strategy, in the sequence's local frame. A self-message
/// (`a -> a`) is the strategy's rectangular hook on the lifeline, label
/// tucked over the loop. `lifeline_x` gives each participant's centre (for
/// direction and label placement); `endpoint_x(id, row, toward)` gives the
/// actual attach x — a live activation bar's edge, or the lifeline centre —
/// so an arrow meets the bar it opens [SPEC 13].
pub(super) fn draw(
    pairs: &[Pair],
    lifeline_x: &HashMap<String, f64>,
    endpoint_x: impl Fn(&str, usize, f64) -> f64,
    row_y: impl Fn(usize) -> f64,
) -> Vec<RoutedLink> {
    let mut out = Vec::new();
    for (i, p) in pairs.iter().enumerate() {
        let (Some(&fcx), Some(&tcx)) = (lifeline_x.get(p.from), lifeline_x.get(p.to)) else {
            continue;
        };
        let y = row_y(i);
        let size = p.label_size();
        let ly = y - LABEL_RISE - size / 2.0;
        let (path, text) = if fcx == tcx {
            // A self-message: the strategy's hook off the near (right) bar
            // edge, sized by the message's `clearance` so the loop bends
            // like a routed wire. Its label tucks over the loop: the left
            // edge starts at the loop's middle, just above its top arm — it
            // reads as coming out of the loop and reserves no width (a long
            // label overhangs; only the hook may widen the layout).
            let (dx, dy) = p.hook();
            let fx = endpoint_x(p.from, i, fcx + 1.0);
            let cx = fx + dx / 2.0 + p.label_width() / 2.0;
            (straight::hook(fx, y, y + dy, dx), p.text_at(cx, ly))
        } else {
            let fx = endpoint_x(p.from, i, tcx);
            let tx = endpoint_x(p.to, i, fcx);
            (vec![(fx, y), (tx, y)], p.text_at((fcx + tcx) / 2.0, ly))
        };
        out.push(straight::wire(
            path,
            text.into_iter().collect(),
            (p.from, p.to),
            (p.from, p.to),
            p.link.markers.clone(),
            &p.link.attrs,
            &p.link.applied_styles,
            p.link.span,
        ));
    }
    out
}

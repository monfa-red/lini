//! Chart tooltips [SPEC 14.8]. The baked-safe floor is the native `<title>` each
//! mark already carries; this adds the `tooltip:` mode on top — `none` strips the titles,
//! `title` keeps only them, `rich` (default) also emits a hidden `.lini-chart-tip` card
//! per titled mark in a **top layer** (appended last, so nothing paints over it), revealed
//! by a per-index `.lini-hit-N:hover ~ .lini-tip-N` rule (live-only — the renderer drops
//! the cards and strips the hooks when baking). One post-pass over the lowered nodes does
//! all three modes.

use crate::error::Error;
use crate::layout::prim;
use crate::layout::{Bbox, PlacedNode};
use crate::resolve::{AttrMap, ResolvedValue};
use crate::span::Span;

/// A datum's label presentation [SPEC 14.8]: nothing, hover only, inline-where-it-
/// fits (else hover), or inline always. `Auto` / `Always` draw an inline label; every
/// non-`None` mode keeps the hover `<title>` + card.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tooltip {
    None,
    Hover,
    Auto,
    Always,
}

impl Tooltip {
    /// Whether this mode draws an inline label on the plot [SPEC 14.8].
    pub fn inline(self) -> bool {
        matches!(self, Tooltip::Auto | Tooltip::Always)
    }

    /// Whether an inline label is forced even where it collides (`always`), versus
    /// dropped to its hover card when it cannot sit (`auto`).
    pub fn forced(self) -> bool {
        matches!(self, Tooltip::Always)
    }
}

const SIZE: f64 = 11.0;
const PAD: f64 = 5.0;
const GAP: f64 = 7.0;

/// The chart's `tooltip:` mode [SPEC 14.8], default `auto`.
pub fn read(attrs: &AttrMap) -> Result<Tooltip, Error> {
    read_or(attrs, Tooltip::Auto)
}

/// A node's `tooltip:` mode, falling back to `default` (the chart's, for a series that
/// sets none) — the cascade in one read [SPEC 14.8].
pub fn read_or(attrs: &AttrMap, default: Tooltip) -> Result<Tooltip, Error> {
    match attrs.get("tooltip") {
        None => Ok(default),
        Some(ResolvedValue::Ident(s)) => match s.as_str() {
            "none" => Ok(Tooltip::None),
            "hover" => Ok(Tooltip::Hover),
            "auto" => Ok(Tooltip::Auto),
            "always" => Ok(Tooltip::Always),
            _ => Err(Error::at(
                Span::empty(),
                "'tooltip' is none, hover, auto, or always",
            )),
        },
        _ => Err(Error::at(
            Span::empty(),
            "'tooltip' is none, hover, auto, or always",
        )),
    }
}

/// Apply the tooltip mode to a chart's lowered children, within the `w`×`h` box: `none`
/// strips the `<title>` floor, `title` keeps it, `rich` adds a hover card per titled mark.
///
/// The cards are appended **last** (a top layer), not interleaved after each mark, so a
/// card never hides behind a later-drawn mark. Each is linked back to its mark by an
/// index — the mark gains a `hit-N` class, the card a `tip-N` class — and revealed by a
/// `.lini-hit-N:hover ~ .lini-tip-N` rule (the mark stays the hover target, so a nested
/// label still triggers it). The index class is render-stripped when baking, so baked
/// output is unchanged.
pub fn apply(kids: Vec<PlacedNode>, mode: Tooltip, w: f64, h: f64) -> Vec<PlacedNode> {
    let mut kids = kids;
    if mode == Tooltip::None {
        // The chart suppresses every label: drop the `<title>` floor the marks carry.
        for n in &mut kids {
            n.attrs.remove("hint");
        }
        return kids;
    }
    // hover / auto / always all keep the floor and add a live hover card per titled mark.
    let mut cards = Vec::new();
    for node in kids.iter_mut() {
        let Some(text) = title_string(node) else {
            continue;
        };
        let i = cards.len();
        let (ax, ay) = anchor(node);
        cards.push(make_card(&text, ax, ay, i, w, h));
        node.type_chain.push(format!("hit-{i}"));
    }
    kids.extend(cards);
    kids
}

/// A titled mark's `<title>` text, cloned so the node can then be tagged.
fn title_string(node: &PlacedNode) -> Option<String> {
    match node.attrs.get("hint") {
        Some(ResolvedValue::String(s)) => Some(s.clone()),
        _ => None,
    }
}

/// A mark's placed centre — `cx`/`cy` plus its origin-relative bbox mid (a `poly` keeps
/// `cx`/`cy` at 0 with absolute points, so this holds for any kind).
fn anchor(node: &PlacedNode) -> (f64, f64) {
    (
        node.cx + (node.bbox.min_x + node.bbox.max_x) / 2.0,
        node.cy + (node.bbox.min_y + node.bbox.max_y) / 2.0,
    )
}

/// Card `index`'s `.lini-chart-tip` / `.lini-tip-{index}` group: a solid rounded box + its
/// text, up-right of `(ax, ay)` and clamped inside the chart box.
fn make_card(text: &str, ax: f64, ay: f64, index: usize, w: f64, h: f64) -> PlacedNode {
    let cw = prim::text_width(text, SIZE) + PAD * 2.0;
    let ch = SIZE + PAD * 2.0;
    let cx = (ax + GAP + cw / 2.0).clamp(-w / 2.0 + cw / 2.0, w / 2.0 - cw / 2.0);
    let cy = (ay - GAP - ch / 2.0).clamp(-h / 2.0 + ch / 2.0, h / 2.0 - ch / 2.0);
    let mut bg = prim::rect(cx, cy, cw, ch, live("tip-bg"), 1.0);
    prim::round(&mut bg, 3.0);
    let txt = prim::text(text, cx, cy, SIZE, Some(live("tip-fg")), false);
    let bbox = Bbox {
        min_x: cx - cw / 2.0,
        min_y: cy - ch / 2.0,
        max_x: cx + cw / 2.0,
        max_y: cy + ch / 2.0,
    };
    let classes = vec!["chart-tip".to_string(), format!("tip-{index}")];
    prim::group(vec![bg, txt], classes, bbox)
}

fn live(name: &str) -> ResolvedValue {
    ResolvedValue::LiveVar {
        name: name.into(),
        raw: false,
    }
}

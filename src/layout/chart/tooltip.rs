//! Chart tooltips ([CHARTS.md] §14). The baked-safe floor is the native `<title>` each
//! mark already carries; this adds the `tooltip:` mode on top — `none` strips the titles,
//! `title` keeps only them, `rich` (default) also emits a hidden `.lini-chart-tip` card
//! after each titled mark, revealed by a CSS `:hover` rule (live-only — the renderer drops
//! the card when baking). One post-pass over the lowered nodes does all three modes.

use super::prim;
use crate::error::Error;
use crate::layout::{Bbox, PlacedNode};
use crate::resolve::{AttrMap, ResolvedValue};
use crate::span::Span;

pub enum Tooltip {
    None,
    Title,
    Rich,
}

const SIZE: f64 = 11.0;
const PAD: f64 = 5.0;
const GAP: f64 = 7.0;

/// The chart's `tooltip:` mode ([CHARTS.md] §16), default `rich`.
pub fn read(attrs: &AttrMap) -> Result<Tooltip, Error> {
    match attrs.get("tooltip") {
        None => Ok(Tooltip::Rich),
        Some(ResolvedValue::Ident(s)) => match s.as_str() {
            "rich" => Ok(Tooltip::Rich),
            "title" => Ok(Tooltip::Title),
            "none" => Ok(Tooltip::None),
            _ => Err(Error::at(
                Span::empty(),
                "'tooltip' is rich, title, or none",
            )),
        },
        _ => Err(Error::at(
            Span::empty(),
            "'tooltip' is rich, title, or none",
        )),
    }
}

/// Apply the tooltip mode to a chart's lowered children, within the `w`×`h` box: `none`
/// strips the `<title>` floor, `title` keeps it, `rich` adds a hover card after each
/// titled mark.
pub fn apply(kids: Vec<PlacedNode>, mode: Tooltip, w: f64, h: f64) -> Vec<PlacedNode> {
    match mode {
        Tooltip::Title => kids,
        Tooltip::None => {
            let mut kids = kids;
            for n in &mut kids {
                n.attrs.remove("title");
            }
            kids
        }
        Tooltip::Rich => {
            let mut out = Vec::with_capacity(kids.len() * 2);
            for node in kids {
                let card = card_for(&node, w, h);
                out.push(node);
                out.extend(card);
            }
            out
        }
    }
}

/// The hover card for a titled mark, placed beside the mark's centre — or `None` for a
/// mark with no `<title>` (gridlines, labels, areas, lines).
fn card_for(node: &PlacedNode, w: f64, h: f64) -> Option<PlacedNode> {
    let ResolvedValue::String(text) = node.attrs.get("title")? else {
        return None;
    };
    // Absolute anchor = the mark's placed centre (cx/cy + its origin-relative bbox mid;
    // a `poly` keeps cx/cy at 0 and carries absolute points, so this holds for any kind).
    let ax = node.cx + (node.bbox.min_x + node.bbox.max_x) / 2.0;
    let ay = node.cy + (node.bbox.min_y + node.bbox.max_y) / 2.0;
    Some(make_card(text, ax, ay, w, h))
}

/// A `.lini-chart-tip` card: a solid rounded box + its text, up-right of `(ax, ay)` and
/// clamped inside the chart box.
fn make_card(text: &str, ax: f64, ay: f64, w: f64, h: f64) -> PlacedNode {
    let cw = prim::text_width(text, SIZE) + PAD * 2.0;
    let ch = SIZE + PAD * 2.0;
    let cx = (ax + GAP + cw / 2.0).clamp(-w / 2.0 + cw / 2.0, w / 2.0 - cw / 2.0);
    let cy = (ay - GAP - ch / 2.0).clamp(-h / 2.0 + ch / 2.0, h / 2.0 - ch / 2.0);
    let mut bg = prim::rect(cx, cy, cw, ch, live("tip-bg"), 1.0);
    bg.attrs.insert("radius", ResolvedValue::Number(3.0));
    let txt = prim::text(text, cx, cy, SIZE, Some(live("tip-fg")), false);
    let bbox = Bbox {
        min_x: cx - cw / 2.0,
        min_y: cy - ch / 2.0,
        max_x: cx + cw / 2.0,
        max_y: cy + ch / 2.0,
    };
    prim::group(vec![bg, txt], vec!["chart-tip".to_string()], bbox)
}

fn live(name: &str) -> ResolvedValue {
    ResolvedValue::LiveVar {
        name: name.into(),
        raw: false,
    }
}

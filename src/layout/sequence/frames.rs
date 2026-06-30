//! Frames (SPEC §10): `loop` / `opt` / `alt` fragments and the `alt`'s `else`
//! compartments. A frame is a node whose `[ ]` holds messages (hoisted to the sequence
//! at desugar — a frame opens no scope); the engine draws it as a dashed `|block|`
//! spanning the lifelines those messages touch, over the rows they occupy, with a
//! top-left tab carrying the operator and the guard. `alt` splits into compartments by
//! `|else|`. Frames **nest** — collected depth-first, laid out by source order on one
//! shared timeline with the messages, so an inner frame sits inside its outer.

use super::messages::Pair;
use crate::layout::PlacedNode;
use crate::layout::prim;
use crate::layout::primitives::{PaddingBox, padding};
use crate::resolve::{NodeKind, ResolvedInst, ResolvedValue};
use std::collections::HashMap;

/// Each nesting level draws its border inward, so a nested frame reads inside its parent.
const NEST_INSET: f64 = 6.0;
/// The title-tab height — reserved in the gap above a frame's first message.
const TAB_H: f64 = 16.0;
/// Minimum room below an `|else|` divider, so its guard clears the next message.
const GUARD_CLEAR: f64 = 16.0;
/// Tab keyword and guard text sizes.
const KEYWORD_SIZE: f64 = 10.0;
const GUARD_SIZE: f64 = 10.0;

/// The operator keywords that name a frame (SPEC §10). `else` is a compartment
/// separator collected within an `alt`, not a frame of its own.
const FRAME_KINDS: &[&str] = &["loop", "opt", "alt"];

/// A frame collected from the scene tree, flattened with its nesting depth, the `|else|`
/// separators inside it (empty for `loop` / `opt`), and its `padding` (the inset of the
/// border from the messages it spans and the lifelines it touches).
pub(super) struct Frame<'a> {
    inst: &'a ResolvedInst,
    keyword: &'a str,
    depth: usize,
    elses: Vec<&'a ResolvedInst>,
    pad: PaddingBox,
}

/// The y-extent a frame occupies on the timeline: its top and bottom border, and the y
/// of each `|else|` divider (parallel to [`Frame::elses`]).
pub(super) struct FrameGeom {
    top: f64,
    bot: f64,
    else_ys: Vec<f64>,
}

/// The timeline of the whole sequence body: each message's row y, each note's centre y, the
/// foot, and the frames' y-extents (parallel to the collected frames). Messages, frames, and
/// notes interleave by source order. Computed from `0`; the engine measures `foot_y` (the
/// body height) to centre the diagram, then [`Timeline::shift`]s.
pub(super) struct Timeline {
    pub msg_y: Vec<f64>,
    pub note_y: Vec<f64>,
    pub foot_y: f64,
    pub geom: Vec<FrameGeom>,
}

impl Timeline {
    /// Offset every y by `dy` (the engine's `header_bottom`), turning the relative layout
    /// into absolute, origin-centred coordinates.
    pub(super) fn shift(&mut self, dy: f64) {
        for y in self.msg_y.iter_mut().chain(&mut self.note_y) {
            *y += dy;
        }
        self.foot_y += dy;
        for g in &mut self.geom {
            g.top += dy;
            g.bot += dy;
            for e in &mut g.else_ys {
                *e += dy;
            }
        }
    }
}

/// Collect the frames under a sequence, depth-first (an inner frame follows its outer),
/// each tagged with its nesting depth — the order frames stack and draw.
pub(super) fn collect(children: &[ResolvedInst]) -> Vec<Frame<'_>> {
    let mut out = Vec::new();
    gather(children, 0, &mut out);
    out
}

fn gather<'a>(children: &'a [ResolvedInst], depth: usize, out: &mut Vec<Frame<'a>>) {
    for inst in children {
        if let Some(keyword) = keyword_of(inst) {
            out.push(Frame {
                inst,
                keyword,
                depth,
                elses: inst.children.iter().filter(|c| is_else(c)).collect(),
                pad: padding(&inst.attrs, inst.span).unwrap_or_default(),
            });
            gather(&inst.children, depth + 1, out);
        }
    }
}

/// The frame operator a node carries, if any (`loop` / `opt` / `alt`).
fn keyword_of(inst: &ResolvedInst) -> Option<&str> {
    inst.type_chain
        .iter()
        .map(String::as_str)
        .find(|t| FRAME_KINDS.contains(t))
}

fn is_else(inst: &ResolvedInst) -> bool {
    inst.type_chain.iter().any(|t| t == "else")
}

/// A node's guard — its smart label, lowered to the first text child at desugar.
fn guard(inst: &ResolvedInst) -> Option<&str> {
    inst.children
        .iter()
        .find(|c| c.kind == NodeKind::Text)
        .and_then(|c| c.label.as_deref())
}

/// Place messages, frames, and notes on one timeline by **source order** (SPEC §10): a
/// frame's open / close and each `|else|` reserve vertical room, a message takes a row at
/// `gap_row`, and a note reserves its own box height. `notes` is each note's `(source
/// position, box height)`. Walking events in span order makes a nested frame's extent fall
/// inside its outer. Ys are relative to `0` (the engine shifts to centre — [`Timeline::shift`]).
pub(super) fn timeline(
    pairs: &[Pair],
    frames: &[Frame],
    notes: &[(usize, f64)],
    gap_row: f64,
) -> Timeline {
    enum Ev {
        Open(usize),
        Else(usize, usize),
        Note(usize),
        Msg(usize),
        Close(usize),
    }
    // (source position, rank for ties, event) — Open, Else, then Note / Msg, then Close.
    let mut events: Vec<(usize, u8, Ev)> = Vec::new();
    for (f, fr) in frames.iter().enumerate() {
        events.push((fr.inst.span.start, 0, Ev::Open(f)));
        events.push((fr.inst.span.end, 4, Ev::Close(f)));
        for (e, el) in fr.elses.iter().enumerate() {
            events.push((el.span.start, 1, Ev::Else(f, e)));
        }
    }
    for (i, p) in pairs.iter().enumerate() {
        events.push((p.span().start, 2, Ev::Msg(i)));
    }
    for (i, (pos, _)) in notes.iter().enumerate() {
        events.push((*pos, 2, Ev::Note(i)));
    }
    events.sort_by_key(|(pos, rank, _)| (*pos, *rank));

    // Rows (messages, notes) sit a uniform `gap_row` apart; the frame chrome (tab, dividers,
    // borders) reserves room *within* that rhythm rather than on top of it, so the spacing
    // reads even at any `gap`. `placed` tracks whether the previous element was a row, so the
    // first message inside a frame doesn't add a second gap over the frame's own padding.
    let mut y = 0.0;
    let mut placed = true; // the header sits above; the first row gaps from it
    let mut msg_y = vec![0.0; pairs.len()];
    let mut note_y = vec![0.0; notes.len()];
    let mut geom: Vec<FrameGeom> = frames
        .iter()
        .map(|f| FrameGeom {
            top: 0.0,
            bot: 0.0,
            else_ys: vec![0.0; f.elses.len()],
        })
        .collect();
    // A frame is laid out like a stack of padded compartments: its `padding` insets the
    // messages from every structural line (the top border, each `|else|` divider, the bottom
    // border), messages within a compartment sit a `gap` apart, and the frame's outer borders
    // keep a `gap` from the messages outside it. When `padding == gap` the whole diagram is
    // perfectly even.
    for (_, _, ev) in &events {
        match ev {
            // The top border keeps a `gap` from the content above; the first message is the
            // frame's `padding` below it, but never less than the tab height (so its label
            // clears the operator tab).
            Ev::Open(f) => {
                if placed {
                    y += gap_row;
                }
                geom[*f].top = y;
                y += frames[*f].pad.top.max(TAB_H);
                placed = false;
            }
            // The divider is the compartments' shared edge — padded from the message above and
            // below, with room for the guard that labels the next compartment.
            Ev::Else(f, e) => {
                y += frames[*f].pad.bottom;
                geom[*f].else_ys[*e] = y;
                y += frames[*f].pad.top.max(GUARD_CLEAR);
                placed = false;
            }
            Ev::Msg(i) => {
                // Within a compartment, rows are a `gap` apart; the first message after a
                // border / divider instead clears it by its own label rise, so `padding`
                // measures border-to-label, not border-to-arrow.
                y += if placed {
                    gap_row
                } else {
                    pairs[*i].label_rise()
                };
                msg_y[*i] = y;
                y += pairs[*i].hook_drop(); // a self-message's hook drops below its row
                placed = true;
            }
            // A note reserves its own box height plus a row's clearance above and below.
            Ev::Note(i) => {
                if placed {
                    y += gap_row;
                }
                note_y[*i] = y + notes[*i].1 / 2.0;
                y += notes[*i].1;
                placed = true;
            }
            // The bottom border is the frame's `padding` below the last message; advance to it
            // so the next row keeps a full `gap` *from the border*, not from the last message.
            Ev::Close(f) => {
                y += frames[*f].pad.bottom;
                geom[*f].bot = y;
                placed = true;
            }
        }
    }
    Timeline {
        msg_y,
        note_y,
        foot_y: y + gap_row,
        geom,
    }
}

/// Lower the frames to primitives, split by z-order: `behind` holds the dashed borders +
/// fills + dividers (drawn under the lifelines, so a fill tints the region); `front` holds
/// the operator tabs + guard labels (drawn over the bars, so they stay readable). Outermost
/// first, so a nested frame draws over its parent.
pub(super) fn draw(
    frames: &[Frame],
    geom: &[FrameGeom],
    pairs: &[Pair],
    lifeline_x: &HashMap<String, f64>,
) -> (Vec<PlacedNode>, Vec<PlacedNode>) {
    let (mut behind, mut front) = (Vec::new(), Vec::new());
    for (fr, g) in frames.iter().zip(geom) {
        let Some((lo, hi)) = lifeline_span(fr, pairs, lifeline_x) else {
            continue; // a frame with no placed messages spans nothing
        };
        // The horizontal inset is the frame's `padding`, pulled inward by nesting depth so a
        // nested frame reads inside its parent.
        let nest = fr.depth as f64 * NEST_INSET;
        let (left, right) = (
            lo - (fr.pad.left - nest).max(4.0),
            hi + (fr.pad.right - nest).max(4.0),
        );
        behind.push(border(fr.inst, left, g.top, right, g.bot));
        front.extend(tab(fr, left, g.top));
        // `|else|` dividers split the alt into compartments; each carries its guard.
        for (el, &ey) in fr.elses.iter().zip(&g.else_ys) {
            behind.push(divider(fr.inst, left, right, ey));
            if let Some(text) = guard(el) {
                front.push(guard_text(text, left, ey + GUARD_SIZE));
            }
        }
    }
    (behind, front)
}

/// The lifeline x-span the frame's messages touch — every message whose source position
/// falls inside the frame (so nested-frame messages count too). `None` if it holds none.
fn lifeline_span(
    frame: &Frame,
    pairs: &[Pair],
    lifeline_x: &HashMap<String, f64>,
) -> Option<(f64, f64)> {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in pairs {
        let s = p.span();
        if frame.inst.span.start <= s.start && s.start <= frame.inst.span.end {
            let (a, b) = p.ends();
            for id in [a, b] {
                if let Some(&x) = lifeline_x.get(id) {
                    lo = lo.min(x);
                    hi = hi.max(x);
                }
            }
        }
    }
    (lo <= hi).then_some((lo, hi))
}

/// The dashed frame border — a `|block|` carrying the frame's own resolved paint (`fill:
/// none; stroke: --group-stroke; stroke-style: dashed; radius`), so the cascade still
/// styles it; never a hardcoded look.
fn border(inst: &ResolvedInst, left: f64, top: f64, right: f64, bot: f64) -> PlacedNode {
    let (cx, cy) = ((left + right) / 2.0, (top + bot) / 2.0);
    let fill = inst
        .attrs
        .get("fill")
        .cloned()
        .unwrap_or_else(|| ResolvedValue::Ident("none".into()));
    let mut n = prim::rect(cx, cy, right - left, bot - top, fill, 1.0);
    let stroke = inst
        .attrs
        .get("stroke")
        .cloned()
        .unwrap_or_else(|| super::live("group-stroke"));
    prim::outline(
        &mut n,
        stroke,
        inst.attrs.number("stroke-width").unwrap_or(1.0),
    );
    if let Some(ss) = inst.attrs.get("stroke-style") {
        n.attrs.insert("stroke-style", ss.clone());
        n.own_style.insert("stroke-style", ss.clone());
    }
    prim::round(&mut n, inst.attrs.number("radius").unwrap_or(0.0));
    n
}

/// The top-left title tab: a small filled rect carrying the operator keyword, with the
/// frame's guard (`[cond]`) just to its right.
fn tab(frame: &Frame, left: f64, top: f64) -> Vec<PlacedNode> {
    let tab_w = prim::text_width(frame.keyword, KEYWORD_SIZE) + 12.0;
    // An opaque fill so the tab reads even when it sits over an activation bar.
    let mut card = prim::rect(
        left + tab_w / 2.0,
        top + TAB_H / 2.0,
        tab_w,
        TAB_H,
        super::live("fill"),
        1.0,
    );
    prim::outline(&mut card, super::live("group-stroke"), 1.0);
    let mut out = vec![
        card,
        prim::text(
            frame.keyword,
            left + tab_w / 2.0,
            top + TAB_H / 2.0,
            KEYWORD_SIZE,
            None,
            true,
        ),
    ];
    if let Some(g) = guard(frame.inst) {
        out.push(guard_text(g, left + tab_w + 4.0, top + TAB_H / 2.0));
    }
    out
}

/// An `|else|` divider — a dashed line across the frame at its row, reusing the frame's
/// stroke so the compartment split reads as part of the same fragment.
fn divider(inst: &ResolvedInst, left: f64, right: f64, y: f64) -> PlacedNode {
    let stroke = inst
        .attrs
        .get("stroke")
        .cloned()
        .unwrap_or_else(|| super::live("group-stroke"));
    let mut line = prim::line(vec![(left, y), (right, y)], stroke, 1.0);
    let dashed = ResolvedValue::Ident("dashed".into());
    line.attrs.insert("stroke-style", dashed.clone());
    line.own_style.insert("stroke-style", dashed);
    line
}

/// A guard label `[cond]`, left-aligned at `x` — the condition on a frame or compartment.
fn guard_text(text: &str, x: f64, cy: f64) -> PlacedNode {
    prim::text_left(&format!("[{text}]"), x, cy, GUARD_SIZE, None)
}

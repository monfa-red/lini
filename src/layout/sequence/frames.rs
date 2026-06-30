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
use crate::resolve::{NodeKind, ResolvedInst, ResolvedValue};
use std::collections::HashMap;

/// Horizontal inset of the frame border past the outermost lifeline it spans.
const INSET: f64 = 14.0;
/// Each nesting level draws its border inward, so a nested frame reads inside its parent.
const NEST_INSET: f64 = 6.0;
/// Gap above a frame's top border (separating it from the message before it).
const TOP_MARGIN: f64 = 10.0;
/// Room between the top border and the first message — the tab plus the message's label.
const TAB_CLEAR: f64 = 26.0;
/// Room between the last message and the bottom border.
const BOT_MARGIN: f64 = 12.0;
/// Room from the previous message down to an `|else|` divider.
const ELSE_MARGIN: f64 = 12.0;
/// Room from an `|else|` divider to the next message (for its guard).
const ELSE_CLEAR: f64 = 22.0;
/// The title-tab height.
const TAB_H: f64 = 16.0;
/// Tab keyword and guard text sizes.
const KEYWORD_SIZE: f64 = 10.0;
const GUARD_SIZE: f64 = 10.0;

/// The operator keywords that name a frame (SPEC §10). `else` is a compartment
/// separator collected within an `alt`, not a frame of its own.
const FRAME_KINDS: &[&str] = &["loop", "opt", "alt"];

/// A frame collected from the scene tree, flattened with its nesting depth and the
/// `|else|` separators inside it (empty for `loop` / `opt`).
pub(super) struct Frame<'a> {
    inst: &'a ResolvedInst,
    keyword: &'a str,
    depth: usize,
    elses: Vec<&'a ResolvedInst>,
}

/// The y-extent a frame occupies on the timeline: its top and bottom border, and the y
/// of each `|else|` divider (parallel to [`Frame::elses`]).
pub(super) struct FrameGeom {
    top: f64,
    bot: f64,
    else_ys: Vec<f64>,
}

/// Vertical clear space above and below a note's box on its row.
const NOTE_MARGIN: f64 = 8.0;

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

    let mut y = 0.0;
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
    for (_, _, ev) in &events {
        match ev {
            // The top border sits just below the previous content; the first message clears
            // the tab and its own label.
            Ev::Open(f) => {
                geom[*f].top = y + TOP_MARGIN;
                y += TOP_MARGIN + TAB_CLEAR;
            }
            // The divider sits below the previous message; the next message clears the guard.
            Ev::Else(f, e) => {
                geom[*f].else_ys[*e] = y + ELSE_MARGIN;
                y += ELSE_MARGIN + ELSE_CLEAR;
            }
            Ev::Msg(i) => {
                y += gap_row;
                msg_y[*i] = y;
                y += pairs[*i].hook_drop(); // a self-message's hook drops below its row
            }
            // A note reserves its box height, centred with a margin above and below.
            Ev::Note(i) => {
                y += NOTE_MARGIN;
                note_y[*i] = y + notes[*i].1 / 2.0;
                y += notes[*i].1 + NOTE_MARGIN;
            }
            // The bottom border sits below the last message.
            Ev::Close(f) => {
                y += BOT_MARGIN;
                geom[*f].bot = y;
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

/// Lower the frames to primitives: a dashed border (reusing the frame's resolved paint),
/// a title tab with the operator + guard, and an `|else|` divider + guard per compartment.
/// Outermost first, so a nested frame draws over its parent.
pub(super) fn draw(
    frames: &[Frame],
    geom: &[FrameGeom],
    pairs: &[Pair],
    lifeline_x: &HashMap<String, f64>,
) -> Vec<PlacedNode> {
    let mut out = Vec::new();
    for (fr, g) in frames.iter().zip(geom) {
        let Some((lo, hi)) = lifeline_span(fr, pairs, lifeline_x) else {
            continue; // a frame with no placed messages spans nothing
        };
        let inset = (INSET - fr.depth as f64 * NEST_INSET).max(4.0);
        let (left, right) = (lo - inset, hi + inset);
        out.push(border(fr.inst, left, g.top, right, g.bot));
        out.extend(tab(fr, left, g.top));
        // `|else|` dividers split the alt into compartments; each carries its guard.
        for (el, &ey) in fr.elses.iter().zip(&g.else_ys) {
            out.push(divider(fr.inst, left, right, ey));
            if let Some(text) = guard(el) {
                out.push(guard_text(text, left, ey + GUARD_SIZE));
            }
        }
    }
    out
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
    let mut card = prim::rect(
        left + tab_w / 2.0,
        top + TAB_H / 2.0,
        tab_w,
        TAB_H,
        super::live("group-fill"),
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

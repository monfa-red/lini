//! Frames [SPEC 13]: `loop` / `opt` / `alt` fragments and the `alt`'s `else`
//! compartments. A frame is a node whose `[ ]` holds messages (hoisted to the sequence
//! at desugar — a frame opens no scope); the engine draws it as a dashed `|block|`
//! spanning the lifelines those messages touch, over the rows they occupy, with a
//! top-left tab carrying the operator and the guard. `alt` splits into compartments by
//! `|else|`. Frames **nest** — collected depth-first, laid out by source order on one
//! shared timeline with the messages, so an inner frame sits inside its outer.

use super::messages::Pair;
use crate::layout::prim;
use crate::layout::primitives::{PaddingBox, padding};
use crate::layout::{Bbox, PlacedNode};
use crate::resolve::{NodeKind, ResolvedInst, ResolvedValue};
use std::collections::HashMap;

/// Each nesting level draws its border inward, so a nested frame reads inside its parent.
const NEST_INSET: f64 = 6.0;
/// The title-tab height — reserved in the gap above a frame's first message.
const TAB_H: f64 = 16.0;
/// A note's clearance above the timeline, as a fraction of `gap_row` — it hugs rather
/// than reserving a whole row, so it doesn't waste vertical space.
const NOTE_CLEAR: f64 = 0.5;
/// The smallest gap a compartment keeps between a message and the border / divider it
/// sits against — so even `padding: 0` never lets an arrow touch the frame line.
const MIN_EDGE: f64 = 6.0;
/// The room a compartment's first message keeps below its tab / divider when its label is
/// **centred** (a normal arrow) — enough for the chrome (tab or guard) *and* the arrow-label
/// to stack above the arrow without overlap. A self-message's label sits to the side, so it
/// hugs at `MIN_EDGE` instead; only the centred case needs this.
const CHROME_CLEAR: f64 = 32.0;
/// Fallback tab / guard text size, used only if a frame has no resolved `font-size`.
const KEYWORD_SIZE: f64 = 10.0;

/// The operator keywords that name a frame [SPEC 13]. `else` is a compartment
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

/// How far a compartment's first arrow sits below its tab / divider chrome: a self-message's
/// label rides off to the side (clear of the corner tab), so it hugs at `MIN_EDGE`; a normal
/// centred label must clear the chrome, so it floors at `CHROME_CLEAR`.
fn top_clear(p: &Pair) -> f64 {
    if p.is_self() { MIN_EDGE } else { CHROME_CLEAR }
}

/// A node's guard — its smart label, lowered to the first text child at desugar.
fn guard(inst: &ResolvedInst) -> Option<&str> {
    inst.children
        .iter()
        .find(|c| c.kind == NodeKind::Text)
        .and_then(|c| c.label.as_deref())
}

/// Place messages, frames, and notes on one timeline by **source order** [SPEC 13]: a
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
    let mut pending: Option<usize> = None; // a just-opened compartment, its top inset deferred
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
    // `padding` is the inset from a frame's border to the arrows it wraps, equal on every
    // side. The top inset is **deferred to the first message** (`pending`): a self-message
    // hugs at `MIN_EDGE` (its label is off to the side, clear of the tab in the corner),
    // while a normal centred label clears the tab / guard chrome at `CHROME_CLEAR`. `padding`
    // grows either past its floor. So `padding: 0` hugs the links on every side, and the
    // arrows stay centred in their compartment.
    let top_inset = |f: usize, i: usize| frames[f].pad.top.max(top_clear(&pairs[i]));
    for (_, _, ev) in &events {
        match ev {
            Ev::Open(f) => {
                if placed {
                    y += gap_row;
                }
                geom[*f].top = y;
                pending = Some(*f);
                placed = false;
            }
            // A divider is two compartments' shared edge: `padding` below the last arrow above,
            // then the lower compartment's top inset is deferred to its first message, as at Open.
            Ev::Else(f, e) => {
                y += frames[*f].pad.bottom.max(MIN_EDGE);
                geom[*f].else_ys[*e] = y;
                pending = Some(*f);
                placed = false;
            }
            Ev::Msg(i) => {
                if let Some(f) = pending.take() {
                    y += top_inset(f, *i); // first in compartment: clear the chrome above it
                } else if placed {
                    y += gap_row; // the rest follow at the message pitch
                }
                msg_y[*i] = y;
                y += pairs[*i].hook_drop(); // a self-message's hook drops below its row
                placed = true;
            }
            // A note hugs the timeline — half a row's clearance, not a full gap — so it
            // annotates without pushing everything down [SPEC 13].
            Ev::Note(i) => {
                if let Some(f) = pending.take() {
                    y += frames[f].pad.top.max(MIN_EDGE);
                } else if placed {
                    y += gap_row * NOTE_CLEAR;
                }
                note_y[*i] = y + notes[*i].1 / 2.0;
                y += notes[*i].1;
                placed = true;
            }
            // The bottom border is the compartment's bottom `padding` below the last message
            // (floored so `padding: 0` still clears the line) — symmetric with the top inset.
            Ev::Close(f) => {
                if let Some(g) = pending.take() {
                    y += frames[g].pad.top.max(MIN_EDGE); // an empty compartment
                }
                y += frames[*f].pad.bottom.max(MIN_EDGE);
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
        // The horizontal inset is the frame's `padding` (floored like the vertical, so the
        // sides match the top / bottom), pulled inward by nesting depth so a nested frame
        // reads inside its parent.
        let nest = fr.depth as f64 * NEST_INSET;
        let (left, right) = (
            lo - (fr.pad.left - nest).max(MIN_EDGE),
            hi + (fr.pad.right - nest).max(MIN_EDGE),
        );
        behind.push(border(fr.inst, left, g.top, right, g.bot));
        front.extend(tab(fr, left, g.top));
        // `|else|` dividers split the alt into compartments; each carries its guard, in the
        // frame's own text colour.
        let size = text_size(fr.inst);
        let color = frame_color(fr.inst);
        for (el, &ey) in fr.elses.iter().zip(&g.else_ys) {
            behind.push(divider(fr.inst, left, right, ey));
            if let Some(text) = guard(el) {
                front.push(guard_text(text, left, ey + size, size, color.clone()));
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
            // A self-message's hook (the arrow area) extends right of its lifeline, so the
            // frame grows to contain it — the loop reads as inside the fragment [SPEC 13].
            if p.is_self()
                && let Some(&x) = lifeline_x.get(a)
            {
                hi = hi.max(x + p.hook_reach());
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
    prim::outline(
        &mut n,
        frame_stroke(inst),
        inst.attrs.number("stroke-width").unwrap_or(1.0),
    );
    if let Some(ss) = inst.attrs.get("stroke-style") {
        n.attrs.insert("stroke-style", ss.clone());
        n.own_style.insert("stroke-style", ss.clone());
    }
    prim::round(&mut n, inst.attrs.number("radius").unwrap_or(0.0));
    n
}

/// The frame's text size for its tab + guards — its resolved `font-size` (set by the bundle,
/// overridable by the cascade), not a hardcoded constant.
fn text_size(inst: &ResolvedInst) -> f64 {
    inst.attrs.number("font-size").unwrap_or(KEYWORD_SIZE)
}

/// The frame's resolved stroke colour — shared by the border, the `|else|` dividers, and the
/// tab outline, so the whole fragment (chrome included) reads in one colour [SPEC 13].
fn frame_stroke(inst: &ResolvedInst) -> ResolvedValue {
    inst.attrs
        .get("stroke")
        .cloned()
        .unwrap_or_else(|| super::live("group-stroke"))
}

/// The frame's text colour for its tab keyword + guards — its `color` if set (so styling the
/// fragment styles its labels), else the default text colour.
fn frame_color(inst: &ResolvedInst) -> Option<ResolvedValue> {
    inst.attrs.get("color").cloned()
}

/// The top-left title **banner**: the operator keyword in a tab whose top-left corner
/// rounds to match the frame's `radius` and whose bottom-right is clipped (the classic
/// UML fragment label), with the frame's guard (`[cond]`) just to its right.
fn tab(frame: &Frame, left: f64, top: f64) -> Vec<PlacedNode> {
    let size = text_size(frame.inst);
    let tab_w = prim::text_width(frame.keyword, size) + 14.0;
    let r = frame
        .inst
        .attrs
        .number("radius")
        .unwrap_or(0.0)
        .min(TAB_H / 2.0);
    let cut = (TAB_H * 0.42).min(tab_w / 2.0);
    let sw = frame.inst.attrs.number("stroke-width").unwrap_or(1.0);
    // The frame border (a `rect`) insets its outline by half its stroke; the tab (a `path`)
    // doesn't, so shift the tab's shared top / left edges in by the same half-stroke — the
    // banner sits flush on the frame line instead of poking a hair past it.
    let hw = sw / 2.0;
    let color = frame_color(frame.inst);
    // An opaque fill so the banner reads even when it sits over an activation bar; its outline
    // is the frame's own stroke colour (solid), so the chrome matches the dashed border.
    let mut banner = prim::path(
        tab_path(left + hw, top + hw, tab_w, TAB_H, r, cut),
        super::live("fill"),
        Bbox {
            min_x: left,
            min_y: top,
            max_x: left + tab_w,
            max_y: top + TAB_H,
        },
    );
    prim::outline(&mut banner, frame_stroke(frame.inst), sw);
    let mut out = vec![
        banner,
        // The operator keyword — bold via the `.lini-sequence-tab` stylesheet rule, not an
        // inline style; the default text colour reads as the structural label, distinct from
        // the guards (which take the frame's `color`).
        prim::text_classed(
            frame.keyword,
            left + hw + (tab_w - cut / 2.0) / 2.0,
            top + hw + TAB_H / 2.0,
            size,
            "sequence-tab",
        ),
    ];
    if let Some(g) = guard(frame.inst) {
        // Nudge the guard 1px down off the top border so it doesn't kiss the outline.
        out.push(guard_text(
            g,
            left + tab_w + 4.0,
            top + TAB_H / 2.0 + 1.0,
            size,
            color,
        ));
    }
    out
}

/// The banner outline `d`: a rectangle with a rounded top-left corner (radius `r`, the
/// frame's own) and a clipped bottom-right corner (chamfer `c`), drawn clockwise.
fn tab_path(l: f64, t: f64, w: f64, h: f64, r: f64, c: f64) -> String {
    let (rgt, bot) = (l + w, t + h);
    let mut d = format!(
        "M {} {} L {rgt} {t} L {rgt} {} L {} {bot} L {l} {bot} ",
        l + r,
        t,
        bot - c,
        rgt - c
    );
    if r > 0.5 {
        d.push_str(&format!("L {l} {} A {r} {r} 0 0 1 {} {t} Z", t + r, l + r));
    } else {
        d.push_str(&format!("L {l} {t} Z"));
    }
    d
}

/// An `|else|` divider — a dashed line across the frame at its row, reusing the frame's
/// stroke so the compartment split reads as part of the same fragment.
fn divider(inst: &ResolvedInst, left: f64, right: f64, y: f64) -> PlacedNode {
    let mut line = prim::line(vec![(left, y), (right, y)], frame_stroke(inst), 1.0);
    let dashed = ResolvedValue::Ident("dashed".into());
    line.attrs.insert("stroke-style", dashed.clone());
    line.own_style.insert("stroke-style", dashed);
    line
}

/// A guard label `[cond]`, left edge at `x` — the condition on a frame or compartment. Its
/// size / weight ride the `.lini-sequence-guard` stylesheet rule (not inline); only the
/// frame's text `color` is inlined, since it varies per fragment.
fn guard_text(text: &str, x: f64, cy: f64, size: f64, color: Option<ResolvedValue>) -> PlacedNode {
    let label = format!("[{text}]");
    let cx = x + prim::text_width(&label, size) / 2.0;
    let mut n = prim::text_classed(&label, cx, cy, size, "sequence-guard");
    if let Some(c) = color {
        n.attrs.insert("color", c.clone());
        n.own_style.insert("color", c);
    }
    n
}

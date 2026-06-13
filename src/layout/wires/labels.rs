//! Wire labels (WIRING §Model step 7, SPEC §10): a label rides its wire at
//! `start` / `mid` / `end` / a fraction of its statement's whole drawn
//! route, shifted by `offset` in the tangent frame (x along the wire,
//! y to its left). A label is an obstacle to nothing and the wire never
//! moves for it — but the label may slide along the wire to dodge node
//! bodies, node labels, and other wire labels.

use super::bundle::EdgeReq;
use super::rect::Rect;
use super::scene::SceneIndex;
use crate::layout::ir::{RoutedText, RoutedWire};
use crate::layout::text::{approx_height, approx_width};
use crate::resolve::{Program, ResolvedValue, WireAt};
use crate::span::Span;

/// Breathing room a label keeps from the things it dodges.
const MARGIN: f64 = 2.0;
/// Slide step and the search half-width (`STEPS × STEP` each way).
const STEP: f64 = 4.0;
const STEPS: usize = 40;

/// Place every wire statement's texts onto its drawn segments.
/// `req_of[k]` is the request behind `wires[k]`; statements are re-walked
/// exactly as [`super::bundle::requests`] numbered them, so a chain's
/// segments concatenate in declaration order and the label's anchor is a
/// fraction of the whole drawn route.
pub fn place(
    wires: &mut [RoutedWire],
    req_of: &[usize],
    reqs: &[EdgeReq],
    program: &Program,
    index: &SceneIndex,
) {
    let obstacles = index.obstacle_rects();
    let mut placed: Vec<Rect> = Vec::new();
    let mut stmt_ids: Vec<Span> = Vec::new();
    let mut expansions: Vec<usize> = Vec::new();
    for w in &program.wires {
        let stmt = match stmt_ids.iter().position(|s| *s == w.span) {
            Some(i) => i,
            None => {
                stmt_ids.push(w.span);
                expansions.push(0);
                stmt_ids.len() - 1
            }
        };
        let expansion = expansions[stmt];
        expansions[stmt] += 1;
        if w.texts.is_empty() {
            continue;
        }
        let mut segs: Vec<usize> = (0..wires.len())
            .filter(|&k| {
                let r = &reqs[req_of[k]];
                r.stmt == stmt && r.expansion == expansion
            })
            .collect();
        segs.sort_by_key(|&k| reqs[req_of[k]].seg);
        let lens: Vec<f64> = segs.iter().map(|&k| arc_len(&wires[k].path)).collect();
        let total: f64 = lens.iter().sum();
        if total <= 0.0 {
            continue;
        }

        for t in &w.texts {
            let size = t.attrs.number("text-size").unwrap_or(11.0);
            let (bw, bh) = (approx_width(&t.text, size), approx_height(&t.text, size));
            let (ox, oy) = offset_of(t.attrs.get("offset"));
            let s0 = match t.at {
                WireAt::Start => 0.0,
                WireAt::Mid => total / 2.0,
                WireAt::End => total,
                WireAt::Fraction(f) => f * total,
            };
            let spot = |s: f64| {
                let (p, tan, si) = at_arc(wires, &segs, &lens, s);
                let pos = (p.0 + ox * tan.0 - oy * tan.1, p.1 + ox * tan.1 + oy * tan.0);
                (pos, tan, si)
            };
            let boxed = |pos: (f64, f64)| {
                Rect::new(
                    pos.0 - bw / 2.0,
                    pos.1 - bh / 2.0,
                    pos.0 + bw / 2.0,
                    pos.1 + bh / 2.0,
                )
            };
            let clear = |b: Rect| {
                let pad = b.inflate(MARGIN);
                !obstacles
                    .iter()
                    .chain(placed.iter())
                    .any(|r| pad.intersect(r).is_some())
            };
            // The anchor first, then 4px slides alternating outward — the
            // first clear spot wins; nothing clear keeps the anchor.
            let mut chosen = None;
            for k in 0..=2 * STEPS {
                let delta = STEP * k.div_ceil(2) as f64 * if k % 2 == 1 { 1.0 } else { -1.0 };
                let s = (s0 + delta).clamp(0.0, total);
                let cand = spot(s);
                if clear(boxed(cand.0)) {
                    chosen = Some(cand);
                    break;
                }
            }
            let (pos, tangent, si) = chosen.unwrap_or_else(|| spot(s0));
            placed.push(boxed(pos));
            wires[segs[si]].texts.push(RoutedText {
                content: t.text.clone(),
                position: pos,
                tangent,
                attrs: t.attrs.clone(),
            });
        }
    }
}

fn arc_len(poly: &[(f64, f64)]) -> f64 {
    poly.windows(2)
        .map(|s| (s[1].0 - s[0].0).abs() + (s[1].1 - s[0].1).abs())
        .sum()
}

/// The point and unit tangent at arc position `s` along the statement's
/// concatenated segments, and which segment it landed in. Gaps between a
/// chain's segments (separate ports) contribute no length.
fn at_arc(
    wires: &[RoutedWire],
    segs: &[usize],
    lens: &[f64],
    s: f64,
) -> ((f64, f64), (f64, f64), usize) {
    let mut rem = s.max(0.0);
    for (si, &k) in segs.iter().enumerate() {
        if rem > lens[si] && si + 1 < segs.len() {
            rem -= lens[si];
            continue;
        }
        let poly = &wires[k].path;
        let n = poly.len().saturating_sub(1);
        for (j, seg) in poly.windows(2).enumerate() {
            let (dx, dy) = (seg[1].0 - seg[0].0, seg[1].1 - seg[0].1);
            let l = dx.abs() + dy.abs();
            if l <= 0.0 {
                continue;
            }
            if rem <= l || j + 1 == n {
                let f = (rem / l).min(1.0);
                let p = (seg[0].0 + dx * f, seg[0].1 + dy * f);
                return (p, (dx / l, dy / l), si);
            }
            rem -= l;
        }
    }
    // Unreachable for total > 0; a degenerate route anchors at its start.
    let k = segs[0];
    let p = wires[k].path[0];
    (p, (1.0, 0.0), 0)
}

/// `offset:(x, y)` in the tangent frame; anything else is no offset.
fn offset_of(v: Option<&ResolvedValue>) -> (f64, f64) {
    if let Some(ResolvedValue::Tuple(xs)) = v
        && let [Some(x), Some(y)] = [
            xs.first().and_then(|v| v.as_number()),
            xs.get(1).and_then(|v| v.as_number()),
        ]
    {
        return (x, y);
    }
    (0.0, 0.0)
}

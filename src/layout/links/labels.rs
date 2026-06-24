//! Link labels (LINKING §Model step 7, SPEC §9): a label rides its link at an
//! auto-distributed anchor or an explicit `along:` fraction of its statement's
//! whole drawn route, shifted by `translate: x y` in world coords (the same
//! nudge as on any node). A label is an obstacle to nothing and the link never
//! moves for it — but the label may slide along the link to dodge node
//! bodies, node labels, and other link labels.

use super::bundle::EdgeReq;
use super::rect::Rect;
use super::scene::SceneIndex;
use crate::layout::ir::{RoutedLink, RoutedText};
use crate::layout::text::{approx_height, approx_width};
use crate::resolve::{Along, Program, ResolvedText, ResolvedValue};
use crate::span::Span;

/// Breathing room a label keeps from the things it dodges.
const MARGIN: f64 = 2.0;
/// Slide step and the search half-width (`STEPS × STEP` each way).
const STEP: f64 = 4.0;
const STEPS: usize = 40;

/// Place every link statement's texts onto its drawn segments.
/// `req_of[k]` is the request behind `links[k]`; statements are re-walked
/// exactly as [`super::bundle::requests`] numbered them, so a chain's
/// segments concatenate in declaration order and the label's anchor is a
/// fraction of the whole drawn route.
pub fn place(
    links: &mut [RoutedLink],
    req_of: &[usize],
    reqs: &[EdgeReq],
    program: &Program,
    index: &SceneIndex,
) {
    let obstacles = index.obstacle_rects();
    let mut placed: Vec<Rect> = Vec::new();
    let mut stmt_ids: Vec<Span> = Vec::new();
    let mut expansions: Vec<usize> = Vec::new();
    for w in &program.links {
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
        let mut segs: Vec<usize> = (0..links.len())
            .filter(|&k| {
                let r = &reqs[req_of[k]];
                r.stmt == stmt && r.expansion == expansion
            })
            .collect();
        segs.sort_by_key(|&k| reqs[req_of[k]].seg);
        let lens: Vec<f64> = segs.iter().map(|&k| arc_len(&links[k].path)).collect();
        let total: f64 = lens.iter().sum();
        if total <= 0.0 {
            continue;
        }

        // Default (`at:auto`) labels distribute along the route: spread evenly
        // across the hops, each at its hop's `(j+1)/(k+1)` fractions — so one
        // never lands on a junction and several never pile up (SPEC §10).
        let auto_anchors = distribute_auto(&w.texts, &lens, total);
        let mut auto_i = 0;

        for t in &w.texts {
            let size = t.attrs.number("font-size").unwrap_or(0.0);
            let ls = t.attrs.number("letter-spacing").unwrap_or(0.0);
            let lsp = t.attrs.number("line-spacing").unwrap_or(0.0);
            let (bw, bh) = (
                approx_width(&t.text, size, ls),
                approx_height(&t.text, size, lsp),
            );
            let (tx, ty) = translate_of(t.attrs.get("translate"));
            let s0 = match t.along {
                Along::Auto => {
                    let s = auto_anchors[auto_i];
                    auto_i += 1;
                    s
                }
                Along::Fraction(f) => f * total,
            };
            // A label rides on the line; lift it off with `translate: x y` — a
            // world-frame nudge, the same as on any node (SPEC §6/§9).
            let spot = |s: f64| {
                let (p, tan, si) = at_arc(links, &segs, &lens, s);
                ((p.0 + tx, p.1 + ty), tan, si)
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
            links[segs[si]].texts.push(RoutedText {
                content: t.text.clone(),
                position: pos,
                tangent,
                attrs: t.attrs.clone(),
            });
        }
    }
}

/// Arc anchors for the `at:auto` texts, in their order within `texts`. Assigns
/// the `m` auto labels across the `n` hops (`hop = ⌊i·n/m⌋`) and places those
/// sharing a hop at its internal `(j+1)/(k+1)` fractions — even spread, never
/// on a junction. One entry per auto text, consumed in order.
fn distribute_auto(texts: &[ResolvedText], lens: &[f64], total: f64) -> Vec<f64> {
    let m = texts
        .iter()
        .filter(|t| matches!(t.along, Along::Auto))
        .count();
    let n = lens.len();
    if m == 0 {
        return Vec::new();
    }
    if n == 0 {
        return vec![total / 2.0; m];
    }
    // Cumulative arc length before each hop.
    let mut cum = vec![0.0; n + 1];
    for h in 0..n {
        cum[h + 1] = cum[h] + lens[h];
    }
    let hop = |i: usize| (i * n / m).min(n - 1);
    (0..m)
        .map(|i| {
            let h = hop(i);
            let k = (0..m).filter(|&x| hop(x) == h).count();
            let j = (0..i).filter(|&x| hop(x) == h).count();
            cum[h] + (j as f64 + 1.0) / (k as f64 + 1.0) * lens[h]
        })
        .collect()
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
    links: &[RoutedLink],
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
        let poly = &links[k].path;
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
    let p = links[k].path[0];
    (p, (1.0, 0.0), 0)
}

/// `translate:(x, y)` — a world-frame shift; anything else is no shift.
fn translate_of(v: Option<&ResolvedValue>) -> (f64, f64) {
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

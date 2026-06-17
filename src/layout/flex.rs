//! Row / column flex layout (SPEC §5).
//!
//! `justify` runs *along* the main axis, `align` runs *across* it; both default
//! `center`. `start`/`center`/`end` pack the line at an edge / centre /
//! opposite; `stretch` grows children's boxes to fill; `evenly` (main only)
//! spreads equal gaps between and around. All of these are **no-ops without
//! slack** — an auto-sized container is exactly its packed children, so
//! distribution needs an explicit `width`/`height`, passed in as `avail`.

use super::ir::{Bbox, PlacedNode};
use super::primitives;
use crate::error::Error;
use crate::resolve::{AttrMap, ResolvedValue, VarTable};
use crate::span::Span;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Row,
    Column,
}

/// A concrete bbox dimension, so stretch/measurement read the right axis.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Dim {
    W,
    H,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cross {
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Main {
    Start,
    Center,
    End,
    Stretch,
    Evenly,
}

/// Lay out already-bboxed children in row or column order, positioning each
/// child's `cx`/`cy` and returning the flow's bbox (centred on the container's
/// origin). `avail` is the container's content-area `(width, height)` when it is
/// explicitly sized — the only source of slack; `None` on an axis means auto.
pub fn lay_out_flex(
    axis: Axis,
    children: &mut [PlacedNode],
    attrs: &AttrMap,
    vars: &VarTable,
    span: Span,
    avail: (Option<f64>, Option<f64>),
) -> Result<Bbox, Error> {
    if children.is_empty() {
        return Ok(Bbox::empty());
    }
    let (gap_y, gap_x) = primitives::gap(attrs, vars, span)?;
    let gap = match axis {
        Axis::Row => gap_x,
        Axis::Column => gap_y,
    };
    let main = parse_main(attrs);
    let cross = parse_cross(attrs);
    let (main_dim, cross_dim) = match axis {
        Axis::Row => (Dim::W, Dim::H),
        Axis::Column => (Dim::H, Dim::W),
    };
    let (avail_main, avail_cross) = match axis {
        Axis::Row => (avail.0, avail.1),
        Axis::Column => (avail.1, avail.0),
    };
    let n = children.len() as f64;

    let packed = children.iter().map(|c| len(c, main_dim)).sum::<f64>() + gap * (n - 1.0);
    let main_extent = avail_main.map_or(packed, |a| a.max(packed));
    let max_cross = children.iter().map(|c| len(c, cross_dim)).fold(0.0, f64::max);
    let cross_extent = avail_cross.map_or(max_cross, |a| a.max(max_cross));

    // Cross stretch: each unpinned child's box fills the cross axis.
    if cross == Cross::Stretch {
        for c in children.iter_mut() {
            if !pinned(c, cross_dim) {
                set_dim(c, cross_dim, cross_extent);
            }
        }
    }
    // Main stretch: grow each unpinned child's box by an equal share of slack.
    if main == Main::Stretch {
        let slack = (main_extent - packed).max(0.0);
        let grow: Vec<usize> = (0..children.len())
            .filter(|&i| !pinned(&children[i], main_dim))
            .collect();
        if slack > 0.0 && !grow.is_empty() {
            let add = slack / grow.len() as f64;
            for &i in &grow {
                let m = len(&children[i], main_dim) + add;
                set_dim(&mut children[i], main_dim, m);
            }
        }
    }

    // Leading offset + inter-child gap from the main alignment.
    let used = children.iter().map(|c| len(c, main_dim)).sum::<f64>() + gap * (n - 1.0);
    let remaining = (main_extent - used).max(0.0);
    let (leading, inter) = match main {
        Main::Start => (0.0, gap),
        Main::Center | Main::Stretch => (remaining / 2.0, gap),
        Main::End => (remaining, gap),
        Main::Evenly => {
            let bodies = children.iter().map(|c| len(c, main_dim)).sum::<f64>();
            let eg = (main_extent - bodies) / (n + 1.0);
            (eg, eg)
        }
    };

    let mut cursor = -main_extent / 2.0 + leading;
    for c in children.iter_mut() {
        let m = len(c, main_dim);
        let main_center = cursor + m / 2.0;
        let cross_center = align_cross(cross_extent, len(c, cross_dim), cross);
        place(c, axis, main_center, cross_center);
        cursor += m + inter;
    }

    Ok(match axis {
        Axis::Row => Bbox::centered(main_extent, cross_extent),
        Axis::Column => Bbox::centered(cross_extent, main_extent),
    })
}

fn len(c: &PlacedNode, dim: Dim) -> f64 {
    match dim {
        Dim::W => c.bbox.w(),
        Dim::H => c.bbox.h(),
    }
}

/// Resize a child's box along one dimension, centred — the stretch fill. An
/// explicit size pins the axis (checked by the caller via [`pinned`]), so the
/// recentre never discards an author's dimension.
fn set_dim(c: &mut PlacedNode, dim: Dim, v: f64) {
    c.bbox = match dim {
        Dim::W => Bbox::centered(v, c.bbox.h()),
        Dim::H => Bbox::centered(c.bbox.w(), v),
    };
}

fn pinned(c: &PlacedNode, dim: Dim) -> bool {
    match dim {
        Dim::W => c.attrs.get("width").is_some(),
        Dim::H => c.attrs.get("height").is_some(),
    }
}

/// The child's cross-axis centre within `extent` for the given alignment.
fn align_cross(extent: f64, child: f64, cross: Cross) -> f64 {
    match cross {
        Cross::Start => -extent / 2.0 + child / 2.0,
        Cross::Center | Cross::Stretch => 0.0,
        Cross::End => extent / 2.0 - child / 2.0,
    }
}

/// Land the child's bbox centre at `(main_center, cross_center)` in the
/// container frame, subtracting the bbox's own centre offset (asymmetric
/// children — a line from local 0,0 — keep their geometry).
fn place(c: &mut PlacedNode, axis: Axis, main_center: f64, cross_center: f64) {
    let cbx = (c.bbox.min_x + c.bbox.max_x) / 2.0;
    let cby = (c.bbox.min_y + c.bbox.max_y) / 2.0;
    match axis {
        Axis::Row => {
            c.cx = main_center - cbx;
            c.cy = cross_center - cby;
        }
        Axis::Column => {
            c.cy = main_center - cby;
            c.cx = cross_center - cbx;
        }
    }
}

fn parse_main(attrs: &AttrMap) -> Main {
    match ident(attrs.get("justify")) {
        Some("start") => Main::Start,
        Some("end") => Main::End,
        Some("stretch") => Main::Stretch,
        Some("evenly") => Main::Evenly,
        _ => Main::Center,
    }
}

fn parse_cross(attrs: &AttrMap) -> Cross {
    match ident(attrs.get("align")) {
        Some("start") => Cross::Start,
        Some("end") => Cross::End,
        Some("stretch") => Cross::Stretch,
        _ => Cross::Center,
    }
}

fn ident(v: Option<&ResolvedValue>) -> Option<&str> {
    match v {
        Some(ResolvedValue::Ident(s)) => Some(s.as_str()),
        _ => None,
    }
}

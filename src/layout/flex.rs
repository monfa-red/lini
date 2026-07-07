//! Row / column flex layout [SPEC 12].
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
use crate::resolve::{AttrMap, ResolvedValue};
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
    /// Line children up **origin-to-origin** [SPEC 12]: a drawing's datum, a
    /// sketch's pen origin, an ordinary box's centre — how a row of views
    /// shares one axis [SPEC 15.8].
    Origin,
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
    span: Span,
    avail: (Option<f64>, Option<f64>),
) -> Result<Bbox, Error> {
    if children.is_empty() {
        return Ok(Bbox::empty());
    }
    let (gap_y, gap_x) = primitives::gap(attrs, span)?;
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
    // Origin alignment fixes every child's cross position (origins on one
    // line), so the cross extent is that arrangement's union, not the widest
    // child — see `origin_line` for where the shared line sits.
    let origin_line =
        (cross == Cross::Origin).then(|| origin_line(children, cross_dim, avail_cross));
    let max_cross = match origin_line {
        Some((_, extent)) => extent,
        None => children
            .iter()
            .map(|c| len(c, cross_dim))
            .fold(0.0, f64::max),
    };
    let cross_extent = avail_cross.map_or(max_cross, |a| a.max(max_cross));

    // Cross stretch: each child whose cross dimension is unset fills the axis.
    if cross == Cross::Stretch {
        for c in children.iter_mut() {
            if !dim_set(c, cross_dim) {
                set_dim(c, cross_dim, cross_extent);
            }
        }
    }
    // Main stretch: grow each child whose main dimension is unset by an equal
    // share of the slack.
    if main == Main::Stretch {
        let slack = (main_extent - packed).max(0.0);
        let grow: Vec<usize> = (0..children.len())
            .filter(|&i| !dim_set(&children[i], main_dim))
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
        let cross_center = match origin_line {
            // The box centre that puts this child's origin on the shared line.
            Some((line, _)) => cross_mid(c, cross_dim) - cross_origin(c, cross_dim) + line,
            None => align_cross(cross_extent, len(c, cross_dim), cross),
        };
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
/// explicit size fixes the axis (checked by the caller via [`dim_set`]), so the
/// recentre never discards an author's dimension.
fn set_dim(c: &mut PlacedNode, dim: Dim, v: f64) {
    c.bbox = match dim {
        Dim::W => Bbox::centered(v, c.bbox.h()),
        Dim::H => Bbox::centered(c.bbox.w(), v),
    };
}

fn dim_set(c: &PlacedNode, dim: Dim) -> bool {
    match dim {
        Dim::W => c.attrs.get("width").is_some(),
        Dim::H => c.attrs.get("height").is_some(),
    }
}

/// The child's cross-axis centre within `extent` for the given alignment.
/// (`origin` is per-child and handled in the placement loop.)
fn align_cross(extent: f64, child: f64, cross: Cross) -> f64 {
    match cross {
        Cross::Start => -extent / 2.0 + child / 2.0,
        Cross::Center | Cross::Stretch | Cross::Origin => 0.0,
        Cross::End => extent / 2.0 - child / 2.0,
    }
}

/// With every child's origin on one cross line, where that line sits —
/// returns `(line, extent)`. The group spans `[lo, hi]` about the line: given
/// an explicit cross size it **fits into**, the line is the container's
/// centre line — a small part's axis rides the sheet's centreline
/// ([SPEC 15.8]); an auto-sized (or overfull) axis centres the group around
/// the line instead, so a large ensemble stays balanced. For all-ordinary
/// children (origin = bbox centre) both cases degrade to plain `center`.
fn origin_line(children: &[PlacedNode], cross_dim: Dim, avail: Option<f64>) -> (f64, f64) {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for c in children {
        let o = cross_origin(c, cross_dim);
        let (min, max) = match cross_dim {
            Dim::W => (c.bbox.min_x, c.bbox.max_x),
            Dim::H => (c.bbox.min_y, c.bbox.max_y),
        };
        lo = lo.min(min - o);
        hi = hi.max(max - o);
    }
    if let Some(a) = avail
        && 2.0 * lo.abs().max(hi.abs()) <= a
    {
        return (0.0, a);
    }
    (-(lo + hi) / 2.0, hi - lo)
}

/// The node-local cross coordinate of a child's origin point.
fn cross_origin(c: &PlacedNode, cross_dim: Dim) -> f64 {
    match cross_dim {
        Dim::W => c.origin.0,
        Dim::H => c.origin.1,
    }
}

/// The node-local cross coordinate of a child's bbox centre.
fn cross_mid(c: &PlacedNode, cross_dim: Dim) -> f64 {
    match cross_dim {
        Dim::W => (c.bbox.min_x + c.bbox.max_x) / 2.0,
        Dim::H => (c.bbox.min_y + c.bbox.max_y) / 2.0,
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
        Some("origin") => Cross::Origin,
        _ => Cross::Center,
    }
}

fn ident(v: Option<&ResolvedValue>) -> Option<&str> {
    match v {
        Some(ResolvedValue::Ident(s)) => Some(s.as_str()),
        _ => None,
    }
}

#[cfg(test)]
mod origin_tests {
    use super::super::drawing::testutil::{by_id, laid};

    /// A child's origin on the parent's cross axis: `cy + origin.y`.
    fn origin_y(nodes: &[crate::layout::PlacedNode], id: &str) -> f64 {
        let n = by_id(nodes, id);
        n.cy + n.origin.1
    }

    #[test]
    fn a_row_of_views_lines_up_datum_to_datum() {
        // View `a` carries a bottom dim, so its bbox hangs below its datum;
        // view `b` is bare. `align: center` would drift their axes apart —
        // `align: origin` puts both datums on one line [SPEC 12/15.8].
        let src = |align: &str| {
            format!(
                "|row#views| {{ align: {align} }} [\n  |drawing#a| {{ scale: 1 }} [\n    |oval#c1| {{ width: 20; height: 20 }}\n    c1:left (-) c1:right {{ side: bottom }}\n  ]\n  |drawing#b| {{ scale: 1 }} [ |oval#c2| {{ width: 30; height: 30 }} ]\n]\n"
            )
        };
        let l = laid(&src("origin"));
        let (a, b) = (origin_y(&l.nodes, "a"), origin_y(&l.nodes, "b"));
        assert!((a - b).abs() < 1e-9, "datums share the line: {a} vs {b}");
        let c = laid(&src("center"));
        let (a, b) = (origin_y(&c.nodes, "a"), origin_y(&c.nodes, "b"));
        assert!((a - b).abs() > 1.0, "centre alignment drifts: {a} vs {b}");
    }

    #[test]
    fn a_sketch_aligns_by_its_pen_origin() {
        // The profile sits entirely above its axis, so its box is asymmetric
        // about the pen origin; a plain box's origin is its centre.
        let l = laid(
            "|row#r| { align: origin } [\n  |sketch#s| { draw: move(0, 0) up(10) right(20) down(10) }\n  |box#plain| { width: 20; height: 20 }\n]\n",
        );
        let s = by_id(&l.nodes, "s");
        let plain = by_id(&l.nodes, "plain");
        assert!(
            (s.cy - plain.cy).abs() < 1e-9,
            "pen origin on the box's centre line: {} vs {}",
            s.cy,
            plain.cy
        );
        assert!(
            s.cy + s.bbox.max_y < plain.cy + plain.bbox.max_y - 5.0,
            "the profile's body rides above the shared line"
        );
    }

    #[test]
    fn with_room_the_origin_line_is_the_containers_centre() {
        // An explicit cross size the group fits into pins the shared line to
        // the container's centre — a small part's axis rides the sheet's
        // centreline [SPEC 12/15.8]; view `a`'s bottom dim no longer drags it.
        let l = laid(
            "|row#views| { height: 200; align: origin } [\n  |drawing#a| { scale: 1 } [\n    |oval#c1| { width: 20; height: 20 }\n    c1:left (-) c1:right { side: bottom }\n  ]\n  |drawing#b| { scale: 1 } [ |oval#c2| { width: 30; height: 30 } ]\n]\n",
        );
        let a = by_id(&l.nodes, "a");
        assert!(
            (a.cy + a.origin.1).abs() < 1e-9,
            "the shared line sits at the container centre: {}",
            a.cy + a.origin.1
        );
        // Too little room: the group centres around the line instead.
        let tight = laid(
            "|row#views| { height: 30; align: origin } [\n  |drawing#a| { scale: 1 } [\n    |oval#c1| { width: 20; height: 20 }\n    c1:left (-) c1:right { side: bottom }\n  ]\n  |drawing#b| { scale: 1 } [ |oval#c2| { width: 30; height: 30 } ]\n]\n",
        );
        let a = by_id(&tight.nodes, "a");
        assert!(
            (a.cy + a.origin.1).abs() > 1.0,
            "an overfull axis balances the ensemble: {}",
            a.cy + a.origin.1
        );
    }

    #[test]
    fn ordinary_children_degrade_to_center() {
        let of = |align: &str| {
            let l = laid(&format!(
                "|row#r| {{ align: {align} }} [\n  |box#a| \"A\"\n  |box#b| {{ width: 40; height: 60 }}\n]\n"
            ));
            (by_id(&l.nodes, "a").cy, by_id(&l.nodes, "b").cy)
        };
        assert_eq!(of("origin"), of("center"));
    }

    #[test]
    fn a_grid_origin_row_shares_an_axis() {
        // Both views sit in one grid row: `justify: origin` (the vertical
        // axis) puts each datum on its row's centre line [SPEC 12].
        let l = laid(
            "|grid#g| { columns: auto auto; justify: origin } [\n  |drawing#a| { scale: 1 } [\n    |oval#c1| { width: 20; height: 20 }\n    c1:left (-) c1:right { side: bottom }\n  ]\n  |drawing#b| { scale: 1 } [ |oval#c2| { width: 30; height: 30 } ]\n]\n",
        );
        let (a, b) = (origin_y(&l.nodes, "a"), origin_y(&l.nodes, "b"));
        assert!((a - b).abs() < 1e-9, "row shares the axis: {a} vs {b}");
    }
}

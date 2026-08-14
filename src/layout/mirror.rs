//! `mirror:` on the node's **features** [SPEC 15.3]. The pen already folded the
//! node's own drawn path (`drawing::pen`); what is left is everything else the
//! node holds — its `[ ]` children, reflected about the axis through the node's
//! own origin.
//!
//! A feature takes the same split the subpaths take, read on its **position**:
//! one **on** the axis reflects onto itself and is drawn once; one **off** it
//! becomes a reflected second copy — a replication carrier built by
//! [`crate::layout::pattern::carry`], addressed and counted exactly like
//! `pattern:`'s. A child declines with `mirror: none` (and its subtree with
//! it); `|path|` and `|image|` read `none`, having no reflection to take.
//!
//! **A reflected copy is a copy whose coordinates are reflected**, never a node
//! wearing a flip: no `scale(-1, 1)` reaches the renderer, so a label reads
//! forward and every anchor, outline and halo stays handedness-free.

use super::drawing::geometry::{
    MirrorAxis, P, SEAM_EPS, Subpath, bearing_dir, dir_bearing, reflect_point, to_d,
};
use super::drawing::{Segment, SketchGeo, chrome, pen};
use super::ir::{Bbox, PlacedNode};
use super::pattern::{self, Placement};
use crate::error::Error;
use crate::layout::geom::cross;
use crate::resolve::{NodeKind, ResolvedValue};

/// Reflect the node's features about its `mirror:` axes, in place. Runs from
/// `layout_inst` once the children are placed and before the node's own
/// `pattern:` expands — the order [SPEC 15.3/15.10] states.
pub(super) fn expand(placed: &mut PlacedNode) -> Result<(), Error> {
    let Some(v) = placed.attrs.get("mirror") else {
        return Ok(());
    };
    let pen::Mirror::Axes(axes) = pen::read_mirror(v, placed.span)? else {
        return Ok(());
    };
    for child in placed.children.iter_mut() {
        // Chrome is not a feature: it belongs to the node that generated it and
        // is filled from the already-folded geometry [SPEC 15.7], so reflecting
        // it would draw a second centerline down the axis.
        if chrome::is_chrome(&child.attrs) || declines(child) {
            continue;
        }
        let places = placements(child, &axes);
        if places.len() < 2 {
            continue;
        }
        // The placements are the **parent's** frame, so the carrier stands
        // square and each copy wears the feature's own `rotate:` — a carrier
        // turn would turn the offsets with it, which is `pattern:`'s law, not
        // a reflection's.
        child.rotation = 0.0;
        pattern::carry(child, &places, Vec::new());
    }
    Ok(())
}

/// Whether a child refuses the reflection [SPEC 15.3]: `mirror: none` says so
/// outright, and `|path|` / `|image|` read it — a raw `d` has no parse/emit
/// round-trip here and a raster has no reflection at all.
fn declines(child: &PlacedNode) -> bool {
    if matches!(child.kind, NodeKind::Path | NodeKind::Image) {
        return true;
    }
    child.attrs.get("mirror").is_some_and(|v| {
        matches!(
            pen::read_mirror(v, child.span),
            Ok(pen::Mirror::None) // a malformed value already errored on the child itself
        )
    })
}

/// The copies one feature takes under a `mirror:` list [SPEC 15.3/15.4]: each
/// item doubles the set, a reflection following its original, so the copy order
/// is the addressing order. A copy **on** the axis reflects onto itself and is
/// not doubled; a lone survivor means no carrier at all.
fn placements(child: &PlacedNode, axes: &[MirrorAxis]) -> Vec<Placement> {
    let seed = (child.cx, child.cy);
    let mut spots: Vec<(P, Vec<MirrorAxis>)> = vec![(seed, Vec::new())];
    for &axis in axes {
        let u = axis.dir();
        let doubled: Vec<(P, Vec<MirrorAxis>)> = spots
            .iter()
            .filter(|(p, _)| cross(*p, u).abs() > SEAM_EPS)
            .map(|(p, taken)| {
                let mut taken = taken.clone();
                taken.push(axis);
                (reflect_point(*p, u), taken)
            })
            .collect();
        spots.extend(doubled);
    }
    spots
        .into_iter()
        .map(|(p, reflect)| Placement {
            at: (p.0 - seed.0, p.1 - seed.1),
            rotate: child.rotation,
            reflect,
        })
        .collect()
}

/// Reflect a node's **content** about the line through its own origin with unit
/// direction `u`: its rotation, its drawn geometry, and every descendant —
/// position and content alike. The node's own position is the caller's to place
/// (a copy sits where its placement says).
///
/// The two frames are one map: a descendant's *position* reflects about the
/// axis through this node's origin, its own *content* about the same direction
/// through its own origin — which is exactly this recursion one level down.
///
/// A reflection is an exact **cardinal** flip plus a turn: `Refl(u) =
/// rotate(2·Δ) ∘ Refl(nearest cardinal)`. Only the cardinal flip maps a box to
/// a box, and a box is what the renderer draws every primitive from — mapping
/// the corners of a box about a bearing and re-bounding them resizes the shape
/// (a ⌀8 hole under `mirror: 30` came out ⌀11.7). So the flip is exact and the
/// residual 2·Δ rides the copy's own rotation; a cardinal mirror has Δ = 0 and
/// nothing turns.
pub(super) fn reflect_content(node: &mut PlacedNode, u: P) {
    let (base, delta) = cardinal_of(u);
    flip(node, base);
    node.rotation += 2.0 * delta;
}

/// The cardinal axis nearest the line `u` and the bearing from it — the line's
/// bearing folded into [0, 180), so `Δ ∈ [−45, 45]` and an exact cardinal
/// reads exactly 0.
fn cardinal_of(u: P) -> (P, f64) {
    let b = dir_bearing(u).rem_euclid(180.0);
    let base = if b <= 45.0 {
        0.0
    } else if b < 135.0 {
        90.0
    } else {
        180.0
    };
    (bearing_dir(base), b - base)
}

/// The exact half: the content flipped about a **cardinal** line through the
/// node's own origin.
fn flip(node: &mut PlacedNode, u: P) {
    // Glyphs stay upright: a reflected label reads forward, it does not
    // mirror-write, so a text leaf takes its position and nothing else.
    if node.kind == NodeKind::Text {
        return;
    }
    node.rotation = -node.rotation;
    node.bbox = reflect_bbox(node.bbox, u);
    node.origin = reflect_point(node.origin, u);
    reflect_points(node, u);
    reflect_sketch(node, u);
    for c in &mut node.children {
        let p = reflect_point((c.cx, c.cy), u);
        c.cx = p.0;
        c.cy = p.1;
        flip(c, u);
    }
}

/// A box reflected about a cardinal line through the origin — still a box,
/// exactly.
fn reflect_bbox(b: Bbox, u: P) -> Bbox {
    Bbox::from_points(&[
        reflect_point((b.min_x, b.min_y), u),
        reflect_point((b.max_x, b.max_y), u),
    ])
}

/// A `|line|` / `|poly|`'s drawn vertices — and the generated chrome's, which
/// carries its geometry the same way.
fn reflect_points(node: &mut PlacedNode, u: P) {
    let Some(ResolvedValue::List(items)) = node.attrs.get("points") else {
        return;
    };
    let out: Vec<ResolvedValue> = items
        .iter()
        .map(|item| match item {
            ResolvedValue::Tuple(pair) => match pair.as_slice() {
                [ResolvedValue::Number(x), ResolvedValue::Number(y)] => {
                    let p = reflect_point((*x, *y), u);
                    ResolvedValue::Tuple(vec![
                        ResolvedValue::Number(p.0),
                        ResolvedValue::Number(p.1),
                    ])
                }
                _ => item.clone(),
            },
            _ => item.clone(),
        })
        .collect();
    node.attrs.insert("points", ResolvedValue::List(out));
}

/// A folded profile: its subpaths, the `d` they emit, its authored
/// `:segment`s, and the axes the unary readings measure against — all through
/// the pen's own reflection ([`super::drawing::geometry::PathSeg::reflect`]),
/// never re-derived.
fn reflect_sketch(node: &mut PlacedNode, u: P) {
    let Some(geo) = node.sketch.as_ref() else {
        return;
    };
    let outline: Vec<Subpath> = geo
        .outline
        .iter()
        .map(|sub| Subpath {
            segs: sub.segs.iter().map(|s| s.reflect(u)).collect(),
            closed: sub.closed,
        })
        .collect();
    node.attrs
        .insert("path", ResolvedValue::String(to_d(&outline)));
    let reflected = SketchGeo {
        segments: geo
            .segments
            .iter()
            .map(|(n, s)| (n.clone(), reflect_segment(*s, u)))
            .collect(),
        // An axis reflects like any direction: about a mirror at bearing `a`, a
        // bearing `b` reads `2a − b`.
        mirrors: geo
            .mirrors
            .iter()
            .map(|m| MirrorAxis {
                bearing: 2.0 * super::drawing::geometry::dir_bearing(u) - m.bearing,
            })
            .collect(),
        revolved: geo.revolved,
        threads: geo.threads.clone(),
        outline,
        view: geo.view.clone(),
    };
    node.sketch = Some(std::sync::Arc::new(reflected));
}

fn reflect_segment(s: Segment, u: P) -> Segment {
    let m = |p: P| reflect_point(p, u);
    match s {
        Segment::Point(p) => Segment::Point(m(p)),
        Segment::Edge(a, b) => Segment::Edge(m(a), m(b)),
        Segment::Arc { mid, r } => Segment::Arc { mid: m(mid), r },
        Segment::Circle { center, r } => Segment::Circle {
            center: m(center),
            r,
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::testutil::{all_placed, laid, layout_err, placed_by_id};

    /// A closed half-wall with `mirror: y-axis` — the reported bug's shape.
    const WALL: &str =
        "|sketch#wall| { draw: move(-40, -8) right(23) down(16) left(23) close(); mirror: y-axis }";

    /// Every **drawn** `|hole|`, x-positions relative to `part`'s origin — a
    /// carrier paints nothing, so its copies are what the reader sees.
    fn hole_xs(src: &str, part: &str) -> Vec<f64> {
        let l = laid(src);
        let (_, px, _) = placed_by_id(&l.nodes, part);
        let mut xs: Vec<f64> = all_placed(&l.nodes, &|n| {
            n.type_chain.iter().any(|t| t == "hole")
                && crate::layout::pattern::replicas(n).is_none()
        })
        .into_iter()
        .map(|(_, x, _)| x - px)
        .collect();
        xs.sort_by(f64::total_cmp);
        xs
    }

    #[test]
    fn an_off_axis_feature_reflects_into_a_second_copy() {
        // The reported bug: a half-drawn wall's cross-drilled hole appeared in
        // one wall only. It is a feature off the axis, so it doubles.
        let xs = hole_xs(
            &format!(
                "{{ layout: drawing; density: 1 }}\n{WALL} [\n  |hole#drain| {{ width: 4; translate: -28.5 0 }}\n]\n"
            ),
            "wall",
        );
        assert_eq!(xs, vec![-28.5, 28.5]);
    }

    #[test]
    fn an_on_axis_feature_is_drawn_once() {
        // On the axis it reflects onto itself [SPEC 15.3] — one hole, no
        // carrier; the same feature is doubled by the perpendicular mirror.
        let part = |m: &str| {
            format!(
                "{{ layout: drawing; density: 1 }}\n|sketch#bar| {{ draw: move(-40, -8) right(80) down(16) left(80) close(); mirror: {m} }} [\n  |hole#d| {{ width: 4; translate: 0 -4 }}\n]\n"
            )
        };
        assert_eq!(hole_xs(&part("y-axis"), "bar"), vec![0.0]);
        assert_eq!(hole_xs(&part("x-axis"), "bar"), vec![0.0, 0.0]);
    }

    #[test]
    fn mirror_none_declines_for_a_feature_and_its_subtree() {
        // `none` means no reflection touches it — nor anything it holds, which
        // rides along as its content [SPEC 15.3].
        let xs = hole_xs(
            &format!(
                "{{ layout: drawing; density: 1 }}\n{WALL} [\n  |hole#a| {{ width: 4; translate: -30 0 }}\n  |rect#pad| {{ width: 8; height: 8; translate: -22 0; mirror: none }} [\n    |hole#c| {{ width: 2; translate: 2 0 }}\n  ]\n]\n"
            ),
            "wall",
        );
        assert_eq!(xs, vec![-30.0, -20.0, 30.0]);
    }

    #[test]
    fn a_revolve_folds_the_profile_alone() {
        // A turned part's features are drilled, not turned [SPEC 15.3] — the
        // one hole stays one hole.
        let xs = hole_xs(
            "{ layout: drawing; density: 1 }\n|sketch#shaft| { draw: move(-40, 0) up(8) right(80) down(8); revolve: x-axis } [\n  |hole#d| { width: 4; translate: -20 0 }\n]\n",
            "shaft",
        );
        assert_eq!(xs, vec![-20.0]);
    }

    #[test]
    fn a_reflected_copy_reflects_its_own_drawn_geometry() {
        // Two frames, one map: the copy's *position* reflects about the axis
        // through the part's origin, its *content* about the same direction
        // through its own — so a leg pointing outward keeps pointing outward.
        let l = laid(&format!(
            "{{ layout: drawing; density: 1 }}\n{WALL} [\n  |line#leg| {{ points: 0 0, -6 0; translate: -28.5 0 }}\n]\n"
        ));
        let legs = all_placed(&l.nodes, &|n| {
            n.kind == crate::resolve::NodeKind::Line && n.attrs.get("points").is_some()
        });
        let ends: Vec<f64> = legs
            .iter()
            .map(|(n, ..)| match n.attrs.get("points") {
                Some(crate::resolve::ResolvedValue::List(items)) => match &items[1] {
                    crate::resolve::ResolvedValue::Tuple(p) => p[0].as_number().expect("x"),
                    _ => panic!("a point is a pair"),
                },
                _ => panic!("points"),
            })
            .collect();
        assert_eq!(ends, vec![-6.0, 6.0]);
    }

    #[test]
    fn a_raw_path_and_a_raster_have_no_reflection() {
        assert_eq!(
            layout_err("|path#p| { path: \"M 0 0 L 10 0\"; mirror: x-axis }\n"),
            "'|path|' has no reflection — draw it with the pen"
        );
        assert_eq!(
            layout_err(
                "|image#i| { src: \"https://example.com/a.png\"; width: 10; height: 10; mirror: x-axis }\n"
            ),
            "'|image|' has no reflection — draw it with the pen"
        );
        // Naming an axis is the error; spelling out the reading they already
        // take is not [SPEC 15.3].
        laid("|path#p| { path: \"M 0 0 L 10 0\"; mirror: none }\n");
        laid("|path#p| { path: \"M 0 0 L 10 0\"; mirror: auto }\n");
    }

    #[test]
    fn a_rotated_feature_reflects_to_the_mirrored_turn() {
        // `mirror:` reflects about the **parent's** axis, so each copy wears
        // the feature's own turn — negated on the reflected one. Left on the
        // carrier the turn rotated the offsets with it, and the copy landed
        // off the part entirely.
        let l = laid(
            "{ layout: drawing; density: 1 }\n|sketch#wall| { draw: move(-40, -14) right(30) down(28) left(30) close(); mirror: y-axis } [\n  |rect#tab| { width: 16; height: 5; translate: -25 0; rotate: 30 }\n]\n",
        );
        let (_, wx, _) = placed_by_id(&l.nodes, "wall");
        let tabs: Vec<(f64, f64)> = all_placed(&l.nodes, &|n| {
            n.type_chain.iter().any(|t| t == "rect")
                && crate::layout::pattern::replicas(n).is_none()
        })
        .iter()
        .map(|(n, x, _)| (x - wx, n.rotation))
        .collect();
        assert_eq!(tabs, vec![(-25.0, 30.0), (25.0, -30.0)]);
    }

    #[test]
    fn a_reflected_copy_keeps_its_size_at_any_bearing() {
        // A reflection is an exact cardinal flip plus a turn. Mapping a box's
        // corners about a bearing and re-bounding them gives the box that
        // *covers* the turned one — and the renderer draws every primitive
        // from its box, so a ⌀8 hole came out ⌀11.7.
        let part = |axis: &str| {
            let l = laid(&format!(
                "{{ layout: drawing; density: 1 }}\n|sketch#wall| {{ draw: move(-50, -30) right(100) down(60) left(100) close(); mirror: {axis} }} [\n  |hole#h| {{ width: 8; translate: -20 -14 }}\n]\n"
            ));
            let mut sizes: Vec<(f64, f64)> = all_placed(&l.nodes, &|n| {
                n.type_chain.iter().any(|t| t == "hole")
                    && crate::layout::pattern::replicas(n).is_none()
            })
            .iter()
            .map(|(n, ..)| (n.bbox.w(), n.bbox.h()))
            .collect();
            sizes.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
            sizes
        };
        assert_eq!(part("30"), part("y-axis"));
        assert_eq!(part("30").len(), 2);
    }
}

//! The sketch pen [SPEC 15.3]: fold a `|sketch|`'s structured `draw:` items
//! (kept as [`ResolvedValue::PenCall`] by resolve — a `:segment` always glues
//! to a call, stations via `point():name`) into [`Subpath`]s, apply corner
//! modifiers and `mirror:`, collect the authored `:segment`s, and emit the
//! SVG `d` + geometry bbox.
//!
//! Errors follow SPEC 20 verbatim where a message is specified there.

use super::super::ir::Bbox;
use super::Segment;
use super::corner::{Mod, apply_mod};
use super::geometry::{
    self, MirrorAxis, P, PathSeg, Subpath, arc_mid, bearing_dir, dir_bearing, dist, geometry_bbox,
    rotate_about, to_d,
};
use crate::error::{Code, Error};
use crate::resolve::{ResolvedCall, ResolvedInst, ResolvedValue};
use crate::span::Span;

mod parse;
use parse::{Pen, parse_mirror, parse_revolve};

/// A folded sketch: the path, its measurement bbox, and everything the drawing
/// engine reads later.
#[derive(Debug)]
pub struct Folded {
    pub d: String,
    /// The drawn extent, stroke excluded — the measurement box [SPEC 15.1].
    pub geometry: Bbox,
    /// Authored `:segment`s in source order (duplicates rejected at fold) —
    /// carried on the placed node so mates and dimensions anchor on them.
    pub segments: Vec<(String, Segment)>,
    /// The applied `mirror:` axes — the unary mirrored readings read them.
    pub mirror_axes: Vec<MirrorAxis>,
    /// The folded subpaths, scaled and break-clipped — the drawn outline
    /// leader tips ray-cast onto [SPEC 15.2].
    pub subs: Vec<Subpath>,
    /// The `break:` view map [SPEC 15.3] — identity without one. Segments
    /// stay model; the anchors map through this for display.
    pub view: super::breaks::ViewMap,
    /// The break cut edges, authored order — the `|breakline|` chrome's
    /// geometry [SPEC 15.7].
    pub cuts: Vec<super::breaks::CutEdge>,
    /// Whether any open subpath fused. The auto-centerline chrome keys on the
    /// same fact *syntactically* at desugar (an open subpath + `mirror:` —
    /// [SPEC 15.7]); the tests assert the two judgements agree.
    #[allow(
        dead_code,
        reason = "asserted against desugar's openness check in tests"
    )]
    pub fused: bool,
    /// Whether the profile is a `revolve:` — the `⌀` readings gate on it
    /// [SPEC 15.6], and the edge lines below exist only then.
    pub revolved: bool,
    /// The revolve's edge-line spans [SPEC 15.3] — displayed (scaled,
    /// break-clipped) point pairs the `|shoulder|` chrome is cloned from —
    /// including any `thread:`'s end lines (real edges).
    pub edges: Vec<(P, P)>,
    /// A `thread:`'s minor-line spans — the `|threadline|` chrome [SPEC 15.3].
    pub threads: Vec<(P, P)>,
    /// The composed spec per `thread:` group — the smart leader's source.
    pub thread_specs: Vec<super::threads::ThreadSpec>,
}

/// Fold a `|sketch|`'s `draw:` (+ `mirror:`) into its geometry, at the node's
/// own effective `scale:` (px per drawing unit — applied to the folded output,
/// so every call keeps its authored semantics; [SPEC 15.1]). The one entry
/// point — layout calls it; the drawing engine reads the same result.
pub fn fold(inst: &ResolvedInst, scale: f64) -> Result<Folded, Error> {
    let span = inst.span;
    let Some(draw) = inst.attrs.get("draw") else {
        return Err(Error::at(span, "'|sketch|' requires 'draw'").code(Code::MISSING_REQUIRED));
    };
    let items: Vec<&ResolvedValue> = match draw {
        ResolvedValue::Tuple(items) => items.iter().collect(),
        one => vec![one],
    };

    let mut pen = Pen::new(span);
    for item in items {
        match item {
            ResolvedValue::PenCall { call, segment } => pen.call(call, segment.as_deref())?,
            _ => {
                return Err(Error::at(
                    span,
                    "'draw' holds pen calls and ':segment' points — see SPEC 15.3",
                ));
            }
        }
    }
    let (mut subs, mut segments) = pen.finish()?;

    let mut mirror_axes = Vec::new();
    let mut fused = false;
    let mut revolve = None;
    if let Some(v) = inst.attrs.get("revolve") {
        if inst.attrs.get("mirror").is_some() {
            return Err(Error::at(
                span,
                "a sketch takes 'revolve:' or 'mirror:', not both",
            ));
        }
        let axis = parse_revolve(v, span)?;
        fused |= geometry::mirror(&mut subs, axis);
        mirror_axes.push(axis);
        revolve = Some(axis);
    }
    if let Some(v) = inst.attrs.get("mirror") {
        for axis in parse_mirror(v, span)? {
            fused |= geometry::mirror(&mut subs, axis);
            mirror_axes.push(axis);
        }
    }
    if scale != 1.0 {
        geometry::scale(&mut subs, scale);
        for (_, p) in &mut segments {
            *p = p.scaled(scale);
        }
    }
    let (view, cuts) = super::breaks::apply(inst, &mut subs, scale, span)?;
    let mut edges = match revolve {
        Some(axis) => super::edges::spans(&subs, axis),
        None => Vec::new(),
    };
    let dressing = super::threads::dress(inst, &segments, &subs, revolve, &view, scale, span)?;
    edges.extend(dressing.ends);

    let d = to_d(&subs);
    Ok(Folded {
        geometry: geometry_bbox(&d),
        d,
        segments,
        mirror_axes,
        subs,
        view,
        cuts,
        fused,
        revolved: revolve.is_some(),
        edges,
        threads: dressing.minors,
        thread_specs: dressing.specs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fold a `draw:` (+ optional `mirror:`) straight from source, through the
    /// real parse/desugar/resolve pipeline — the pen sees exactly what layout will.
    fn program(src: &str) -> Result<crate::resolve::Program, crate::error::Error> {
        let toks = crate::lexer::lex(src)?;
        let file = crate::syntax::parser::parse(src, &toks)?;
        let lowered = crate::desugar::desugar(&file)?;
        crate::resolve::resolve_with_theme(&lowered, &[])
    }

    fn folded(style: &str) -> Folded {
        let src = format!("|sketch#s| {{ {style} }}\n");
        let program = program(&src).expect("pipeline");
        fold(&program.scene.nodes[0], 1.0).expect("fold")
    }

    fn fold_err(style: &str) -> String {
        let src = format!("|sketch#s| {{ {style} }}\n");
        match program(&src) {
            Err(e) => e.message,
            Ok(p) => {
                fold(&p.scene.nodes[0], 1.0)
                    .expect_err("expected a fold error")
                    .message
            }
        }
    }

    #[test]
    fn point_names_a_station_and_needs_its_segment() {
        // `point():v` records the pen's current point — draws nothing, moves
        // nothing [SPEC 15.3]; bare `point()` and a pre-move station error.
        let f = folded("draw: move(-10, 0) right(20) point():v right(20) down(5);");
        assert!(matches!(
            f.segments.iter().find(|(n, _)| n == "v"),
            Some((_, Segment::Point((10.0, 0.0))))
        ));
        assert_eq!(
            fold_err("draw: move(0, 0) right(10) point() down(5);"),
            "'point()' names the pen's position — attach a ':segment'"
        );
        assert_eq!(
            fold_err("draw: point():v move(0, 0) right(10);"),
            "the pen starts with move(x, y)"
        );
    }

    #[test]
    fn a_rectangle_profile_folds_and_closes() {
        let f = folded("draw: move(0, 0) right(40) down(20) left(40) close();");
        assert_eq!(f.d, "M 0 0 L 40 0 L 40 20 L 0 20 Z");
        assert_eq!(
            (
                f.geometry.min_x,
                f.geometry.min_y,
                f.geometry.max_x,
                f.geometry.max_y
            ),
            (0.0, 0.0, 40.0, 20.0)
        );
    }

    #[test]
    fn verbs_are_visual_and_y_grows_down() {
        // up(10) must decrease y; the frame is the core one [SPEC 15.3].
        let f = folded("draw: move(0, 0) up(10) right(5);");
        assert_eq!(f.d, "M 0 0 L 0 -10 L 5 -10");
    }

    #[test]
    fn segments_collect_the_drawn_vocabulary() {
        let f = folded("draw: move(0, 0) right(40):flat point():station down(10) circle(4):bore;");
        let get = |n: &str| {
            f.segments
                .iter()
                .find(|(name, _)| name == n)
                .map(|(_, p)| *p)
                .expect("named")
        };
        assert_eq!(get("flat"), Segment::Edge((0.0, 0.0), (40.0, 0.0)));
        assert_eq!(get("station"), Segment::Point((40.0, 0.0)));
        assert_eq!(
            get("bore"),
            Segment::Circle {
                center: (40.0, 10.0),
                r: 4.0
            }
        );
    }

    #[test]
    fn chamfer_trims_both_legs() {
        let f = folded("draw: move(0, 0) right(20) chamfer(5) down(20);");
        assert_eq!(f.d, "M 0 0 L 15 0 L 20 5 L 20 20");
    }

    #[test]
    fn fillet_drops_a_tangent_arc() {
        // A square corner: trim = r, quarter arc, clockwise turn (right→down).
        let f = folded("draw: move(0, 0) right(20) fillet(5) down(20);");
        assert_eq!(f.d, "M 0 0 L 15 0 A 5 5 0 0 1 20 5 L 20 20");
    }

    #[test]
    fn cyclic_fillet_rounds_through_close() {
        // fillet(4) close() rounds the last-to-seam corner; close() fillet(4)
        // would round seam-to-first the same way [SPEC 15.3].
        let f = folded("draw: move(0, 0) right(20) down(20) left(20) fillet(4) close();");
        assert!(f.d.contains("A 4 4"), "seam corner rounded: {}", f.d);
        let g = folded("draw: move(0, 0) right(20) down(20) left(20) close() fillet(4);");
        assert!(g.d.contains("A 4 4"), "first corner rounded: {}", g.d);
    }

    #[test]
    fn tangent_arc_turns_the_heading() {
        // Heading right, 90° clockwise on r=10: quarter turn to heading down.
        let f = folded("draw: move(0, 0) right(10) arc(10, 90) right(5);");
        // After the turn the pen heads down; the trailing right(5) drew from
        // the arc's end (20, 10).
        assert_eq!(f.d, "M 0 0 L 10 0 A 10 10 0 0 1 20 10 L 25 10");
    }

    #[test]
    fn relative_arc_picks_the_sweep_by_sign() {
        let cw = folded("draw: move(0, 0) arc(10, 0, 5);");
        assert_eq!(cw.d, "M 0 0 A 5 5 0 0 1 10 0");
        let ccw = folded("draw: move(0, 0) arc(10, 0, -5);");
        assert_eq!(ccw.d, "M 0 0 A 5 5 0 0 0 10 0");
    }

    #[test]
    fn open_subpath_fuses_under_mirror() {
        // A half profile off the axis on one end: fused whole, one closed
        // subpath, with a seam segment at the off-axis end.
        let f = folded("draw: move(-10, 0) up(5) right(20) down(5); mirror: x-axis;");
        assert!(f.d.ends_with("Z"), "fused = closed: {}", f.d);
        assert_eq!(f.d.matches('M').count(), 1, "one fused subpath: {}", f.d);
        // The reflected walk-back visits (10, 5) and (-10, 5).
        assert!(f.d.contains("L 10 5") && f.d.contains("L -10 5"), "{}", f.d);
        assert_eq!(
            (f.geometry.min_y, f.geometry.max_y),
            (-5.0, 5.0),
            "symmetric about the axis"
        );
    }

    #[test]
    fn closed_subpath_duplicates_under_mirror() {
        let f = folded("draw: move(0, -10) circle(3); mirror: x-axis;");
        assert_eq!(
            f.d.matches('M').count(),
            2,
            "seed + reflected copy: {}",
            f.d
        );
        assert_eq!((f.geometry.min_y, f.geometry.max_y), (-13.0, 13.0));
    }

    #[test]
    fn fold_errors_speak_spec() {
        assert!(fold_err("draw: right(10);").contains("starts with move"));
        assert!(fold_err("draw: move(0, 0) wiggle(3);").contains("unknown draw call 'wiggle'"));
        assert!(
            fold_err("draw: move(0, 0) arc(100, 0, 2);").contains("smaller than half the chord")
        );
        assert!(
            fold_err("draw: move(0, 0) fillet(3) right(5);")
                .contains("corner between two segments")
        );
        assert!(fold_err("draw: move(0, 0) right(5) fillet(9) down(5);").contains("does not fit"));
        assert!(fold_err("draw: move(0, 0) right(5):left;").contains("built-in anchor"));
        assert!(fold_err("draw: move(0, 0) right(5):a up(2):a;").contains("already named"));
        assert!(fold_err("draw: move(0, 0) arc(4, 90);").contains("continues a heading"));
        assert!(fold_err("draw: move(0, 0):spot right(4);").contains("takes no segment"));
        assert!(
            fold_err("draw: move(0, 0) right(4); mirror: sideways;")
                .contains("x-axis, y-axis, or a bearing")
        );
    }

    #[test]
    fn fillet_and_chamfer_take_a_curved_leg() {
        // A fillet blends a line into an arc: its own arc + the drawn arc.
        let f = folded("draw: move(-30, 0) right(30) fillet(6) arc(30, -30, 40);");
        assert_eq!(f.d.matches(" A ").count(), 2, "line→arc fillet: {}", f.d);
        // A chamfer cuts back along an arc — the arc survives (trimmed), the
        // bevel is a straight run.
        let g = folded("draw: move(0, -40) arc(30, 30, 40) chamfer(3) right(30);");
        assert_eq!(
            g.d.matches(" A ").count(),
            1,
            "arc chamfer keeps one arc: {}",
            g.d
        );
        // Too large for its curved neighbour errors, never silently mis-draws.
        let e = fold_err("draw: move(-4, 0) right(4) fillet(9) arc(6, -6, 8);");
        assert!(e.contains("fit") || e.contains("large"), "got: {e}");
    }
}

//! The `straight` strategy (ROUTING.md Strategies): each link is one
//! segment between two anchors its caller supplies, trimmed to the endpoint
//! bodies, plus the rectangular self-hook. It avoids nothing and reports
//! nothing; markers and labels ride it like any wire, and corners (the
//! hook's) round through the shared render-time fillet pass.
//!
//! Two callers: a `routing: straight` scope's requests, whose anchors are
//! the body centres (the trim is [`stray_segment`]'s — one mechanism); and
//! sequence messages, whose layout owns *where* (column x, row y) and lowers
//! each wire through [`wire`] itself [SPEC 13].

use crate::layout::ir::{RoutedLink, RoutedText};
use crate::resolve::{AttrMap, Markers, Strategy};
use crate::routing::Routing;
use crate::routing::ortho::geometry::stray_segment;
use crate::routing::ortho::request::EdgeReq;
use crate::span::Span;

/// The smallest self-hook, so a `clearance: 0` link still shows a loop.
pub(crate) const HOOK_MIN: f64 = 14.0;

/// The rectangular self-hook: out of the side line `x` at `y0`, across
/// `reach`, back in at `y1`. Drawn rightward for positive `reach`; corners
/// round at render time like any wire's.
pub(crate) fn hook(x: f64, y0: f64, y1: f64, reach: f64) -> Vec<(f64, f64)> {
    vec![(x, y0), (x + reach, y0), (x + reach, y1), (x, y1)]
}

/// Assemble a drawn straight wire — the one place this strategy's
/// [`RoutedLink`] is built, shared by scope links and sequence messages.
/// `seg` names this segment's own endpoints, `data` the whole statement's
/// (they differ only on a chain's middle segments).
#[allow(clippy::too_many_arguments)]
pub(crate) fn wire(
    path: Vec<(f64, f64)>,
    texts: Vec<RoutedText>,
    seg: (&str, &str),
    data: (&str, &str),
    markers: Markers,
    attrs: &AttrMap,
    applied_styles: &[String],
    span: Span,
) -> RoutedLink {
    RoutedLink {
        path,
        strategy: Strategy::Straight,
        markers,
        attrs: attrs.clone(),
        applied_styles: applied_styles.to_vec(),
        texts,
        data_from: data.0.to_owned(),
        data_to: data.1.to_owned(),
        seg_from: seg.0.to_owned(),
        seg_to: seg.1.to_owned(),
        decl_span: span,
        fan_from: None,
        fan_to: None,
    }
}

/// Draw every `routing: straight` request: a centre-to-centre segment
/// trimmed to the two bodies, or the self-hook off the body's right side. A
/// pair whose trim leaves nothing (coincident or containing bodies) draws
/// nothing — this strategy has no report.
pub(crate) fn route(reqs: &[EdgeReq], routing: &mut Routing, req_of: &mut Vec<usize>) {
    for (i, req) in reqs.iter().enumerate() {
        if req.routing != Strategy::Straight {
            continue;
        }
        let path = if req.a_path == req.b_path {
            let r = req.a_rect;
            let s = req.clearance.max(HOOK_MIN);
            let cy = (r.y0 + r.y1) / 2.0;
            hook(r.x1, cy - s / 2.0, cy + s / 2.0, s)
        } else {
            match stray_segment(req.a_rect, req.b_rect) {
                Some((from, to)) => vec![from, to],
                None => continue,
            }
        };
        req_of.push(i);
        routing.links.push(wire(
            path,
            Vec::new(),
            (&req.a_path, &req.b_path),
            (&req.data_from, &req.data_to),
            req.markers.clone(),
            &req.attrs,
            &req.applied_styles,
            req.span,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::ortho::rect::Rect;

    #[test]
    fn a_diagonal_pair_trims_to_both_boundaries() {
        // The straight strategy shares the stray's trim: centre to centre,
        // cut where the ray leaves each body — oblique is lawful here.
        let a = Rect::new(0.0, 0.0, 40.0, 40.0);
        let b = Rect::new(100.0, 100.0, 140.0, 140.0);
        let (p, q) = stray_segment(a, b).expect("segment");
        assert_eq!(p, (40.0, 40.0));
        assert_eq!(q, (100.0, 100.0));
    }

    #[test]
    fn the_self_hook_is_a_rectangle_off_the_side() {
        let p = hook(80.0, 12.0, 28.0, 16.0);
        assert_eq!(
            p,
            vec![(80.0, 12.0), (96.0, 12.0), (96.0, 28.0), (80.0, 28.0)]
        );
    }

    #[test]
    fn wire_carries_markers_and_names_for_the_renderer() {
        let attrs = AttrMap::default();
        let w = wire(
            vec![(0.0, 0.0), (10.0, 5.0)],
            Vec::new(),
            ("a", "b"),
            ("a", "c"),
            Markers::default(),
            &attrs,
            &["loud".into()],
            Span::empty(),
        );
        assert_eq!((w.seg_from.as_str(), w.seg_to.as_str()), ("a", "b"));
        assert_eq!((w.data_from.as_str(), w.data_to.as_str()), ("a", "c"));
        assert_eq!(w.applied_styles, vec!["loud".to_string()]);
        assert_eq!(w.path.len(), 2);
    }
}

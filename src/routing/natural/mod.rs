//! The `natural` strategy (ROUTING.md The natural strategy): direct smooth
//! curves — no channels, no search, no capacity, no ledger, no strays. Three
//! decide-once steps: sides and ports for every request before any curve
//! exists ([`port`]), then per wire an independent direct spline fit
//! ([`curve`]) with bounded via dodges around offending bodies ([`dodge`]).
//! A wire that cannot dodge draws anyway and names the body it crosses;
//! wire-wire crossings are free at any angle, counted by the spine's shared
//! report. The `path` is the dense sampling of the exact cubics (port and
//! stub points exact), so every shared consumer — markers, the label
//! arc-walk, masks, crossing counts, the validator — reads true drawn
//! geometry with no strategy knowledge.

pub(crate) mod curve;
pub(crate) mod dodge;
pub(crate) mod port;

use crate::layout::ir::RoutedLink;
use crate::resolve::Strategy;
use crate::routing::ortho::cost::min_pitch;
use crate::routing::ortho::request::{self, EdgeReq, End};
use crate::routing::ortho::scene::SceneIndex;
use crate::routing::{Routing, Rule, Severity, Violation};

/// The unmarked stub: a hair of perpendicular contact, visually the curve
/// starting at the port.
const NUB: f64 = 2.0;

/// Route every natural request over the placed scene, appending drawn wires
/// and their request indices (the label pass's key) like the other strategy
/// drivers. Reports each unresolved body offence as a Clearance warning —
/// `--strict` promotes it; nothing strays.
pub(crate) fn route(
    index: &SceneIndex,
    reqs: &[EdgeReq],
    routing: &mut Routing,
    req_of: &mut Vec<usize>,
) {
    if !reqs.iter().any(|r| r.routing == Strategy::Natural) {
        return;
    }
    // The scope routes at the maximum clearance any of its links carries
    // (ROUTING.md Vocabulary); margin is natural's one derived number.
    let c = reqs
        .iter()
        .filter(|r| r.routing == Strategy::Natural)
        .map(|r| r.clearance)
        .fold(0.0_f64, f64::max);
    let m = min_pitch(c);
    let fans = request::fan_groups(reqs, Strategy::Natural);
    let lands = port::landings(index, reqs, &fans, c);

    for (i, req) in reqs.iter().enumerate() {
        if req.routing != Strategy::Natural {
            continue;
        }
        let [la, lb] = lands[i].expect("natural landing");
        // A natural stub exists for its marker alone: the marker's run-up
        // when one is worn, a hair otherwise — the curve begins at the
        // port, so a wire never runs straight and then turns.
        let thickness = req.attrs.number("stroke-width").unwrap_or(0.0);
        let stub = |kind: crate::resolve::MarkerKind| {
            if kind == crate::resolve::MarkerKind::None {
                NUB
            } else {
                crate::render::markers::marker_size(thickness)
            }
        };
        let (sa, sb) = (stub(req.markers.start), stub(req.markers.end));
        let refit = |vias: &[(curve::Pt, Option<curve::Pt>)]| {
            curve::direct(la.port, la.normal, sa, lb.port, lb.normal, sb, vias)
        };
        let tip =
            |l: &port::Landing, s: f64| (l.port.0 + l.normal.0 * s, l.port.1 + l.normal.1 * s);
        let tips = (tip(&la, sa), tip(&lb, sb));
        // One pipeline for every wire, self-loops included: the direct fit
        // hugs its own corner, its own body offends past the ports' excuse
        // zones, and the ordinary stadium sweep rounds it.
        let keep = dodge::Keepouts::build(
            index,
            [
                (&req.a_path, req.a_rect, la.port, sa),
                (&req.b_path, req.b_rect, lb.port, sb),
            ],
            m,
        );
        let ((path, cubics), offences) = dodge::dodge(&keep, tips, refit);
        for (body, d) in offences {
            routing.report.push(Violation {
                rule: Rule::Clearance,
                severity: Severity::Warning,
                links: vec![format!("{} -> {}", req.a_path, req.b_path)],
                detail: format!(
                    "natural wire passes {d:.1} px from the body at \
                     ({}, {})–({}, {}), under margin {m}; drawn anyway",
                    body.x0, body.y0, body.x1, body.y1
                ),
                span: req.span,
            });
        }
        routing.links.push(RoutedLink {
            path,
            curve: cubics,
            strategy: Strategy::Natural,
            markers: req.markers.clone(),
            attrs: req.attrs.clone(),
            applied_styles: req.applied_styles.clone(),
            texts: Vec::new(),
            data_from: req.data_from.clone(),
            data_to: req.data_to.clone(),
            seg_from: req.a_path.clone(),
            seg_to: req.b_path.clone(),
            decl_span: req.span,
            fan_from: fans.group_at(i, End::A).map(|g| g as u32),
            fan_to: fans.group_at(i, End::B).map(|g| g as u32),
        });
        req_of.push(i);
    }
}

//! **What a link statement may say, and where its endpoints may land**
//! [SPEC 9, 15, 20] — the gates `resolve_link` runs before it builds anything,
//! and the one error an endpoint that resolved nowhere gets.
//!
//! They are one concept because they are one reading of the scope: the drawing
//! ops, `||`, the wider anchor set and the auto-create-free endpoint all exist
//! only in a `layout: drawing`, so each answer here is that scope's vocabulary
//! stated for a different part of the statement.

use super::super::scene::PathIndex;
use super::LinkScope;
use crate::ast::{ChainOp, LinkMarker, Side};
use crate::error::{Code, Error};
use crate::syntax::ast::{Endpoint, Link};

/// The statement-shape gates [SPEC 15, 20]: the drawing ops need a drawing
/// scope; a mate takes no label; and a one-ended statement is legal only for
/// the leader-shaped and measuring ops, in a drawing.
pub(super) fn validate_statement(w: &Link, scope: &LinkScope) -> Result<(), Error> {
    let drawing = scope.drawing;
    if !drawing {
        match w.op() {
            ChainOp::Measure(d) => {
                return Err(Error::at(
                    w.span,
                    format!(
                        "'{}' draws a dimension — it belongs in a 'layout: drawing'",
                        d.as_str()
                    ),
                ));
            }
            ChainOp::Mate => {
                // Inside a layout-owning child of a drawing, the flow already
                // decided every position [SPEC 15.5] — name the container.
                if let Some(ty) = &scope.flow_in_drawing {
                    return Err(Error::at(
                        w.span,
                        format!("a '|{ty}|' places its own children — mates seat a drawing's"),
                    ));
                }
                return Err(Error::at(
                    w.span,
                    "a mate seats a drawing's parts — '||' belongs in a 'layout: drawing'",
                ));
            }
            ChainOp::Wire(_) => {}
        }
    }
    // `&` fans one-ended leaders (one note, a leg per feature) and the core
    // two-ended wire ops [SPEC 9, 15.7]; a measure or mate never fans — a
    // span chain is the drafting form [SPEC 21].
    if matches!(w.op(), ChainOp::Measure(_) | ChainOp::Mate)
        && w.chain.iter().any(|g| g.endpoints.len() > 1)
    {
        return Err(Error::at(
            w.span,
            "'&' fans one-ended leaders — chain dimensions instead ('a (-) b (-) c')",
        ));
    }
    // A leader's text is its **text** labels; a carried node is never a label
    // [SPEC 15.9] — but a mate rejects either kind of `[ ]` content.
    let labelled = w.label.is_some() || w.label_texts().next().is_some();
    if matches!(w.op(), ChainOp::Mate) && (labelled || !w.labels.is_empty()) {
        return Err(Error::at(w.span, "a mate takes no label"));
    }
    // `(o)` is unary-only [SPEC 15.6] — the circle pictures one round feature.
    if matches!(w.op(), ChainOp::Measure(crate::ast::DrawOp::Round)) && w.chain.len() > 1 {
        return Err(Error::at(
            w.span,
            "'(o)' measures one round feature — write 'a:top (o)' for a span",
        ));
    }
    if w.chain.len() > 1 {
        return Ok(());
    }
    // One-ended [SPEC 15.6/21]: a unary round / angle measure, or a leader toward
    // its text. The binary `(-)` (linear) needs two ends.
    match w.op() {
        ChainOp::Measure(crate::ast::DrawOp::Linear) => {
            Err(Error::at(w.span, "a linear dimension measures two anchors"))
        }
        ChainOp::Measure(_) => Ok(()),
        ChainOp::Mate => Err(Error::at(w.span, "a mate seats two parts")),
        ChainOp::Wire(op) => {
            if !drawing {
                return Err(Error::at(w.span, "link requires at least two endpoints")
                    .code(Code::CHAIN_TOO_SHORT));
            }
            let leader_tip = matches!(
                op.start,
                LinkMarker::Arrow | LinkMarker::Dot | LinkMarker::Crow
            ) && op.end == LinkMarker::None;
            if leader_tip {
                // A bare `<-` may compose its text from a threaded segment
                // ([SPEC 15.7]) — that is layout knowledge, so the empty-text
                // gate for the arrow leader moves there; `*-` / `>-` always
                // need their word here.
                if !labelled && op.start != LinkMarker::Arrow {
                    return Err(Error::at(
                        w.span,
                        "a leader needs its text — 'bolt <- \"THRU\"'",
                    ));
                }
                return Ok(());
            }
            if op.start == LinkMarker::None && op.end != LinkMarker::None {
                return Err(Error::at(
                    w.span,
                    "a leader points back at its feature — write 'a <- \"…\"'",
                ));
            }
            // A two-marker op (`<->`, `*-*`, …) is a plain annotation arrow here
            // [SPEC 15], not a dimension — it needs two ends like any link.
            Err(Error::at(w.span, "link requires at least two endpoints")
                .code(Code::CHAIN_TOO_SHORT))
        }
    }
}

/// An endpoint's `:point` [SPEC 9, 15.2]: a side everywhere; corners, `center`,
/// and authored names only in a drawing scope. A reversed corner gets its
/// did-you-mean; outside a drawing the message matches the scope's vocabulary.
pub(super) fn resolve_point(
    ep: &Endpoint,
    drawing: bool,
) -> Result<(Option<Side>, Option<String>), Error> {
    let Some(p) = &ep.point else {
        return Ok((None, None));
    };
    if let Some(side) = Side::parse(&p.name) {
        return Ok((Some(side), None));
    }
    if let Some(fix) = corner_reorder(&p.name) {
        return Err(Error::at(
            p.span,
            format!("':{}' is not an anchor — did you mean ':{}'?", p.name, fix),
        ));
    }
    if drawing {
        return Ok((None, Some(p.name.clone())));
    }
    if matches!(
        p.name.as_str(),
        "center" | "top-left" | "top-right" | "bottom-left" | "bottom-right"
    ) {
        Err(Error::at(
            p.span,
            format!(
                "':{}' is a drawing anchor — it belongs in a 'layout: drawing'",
                p.name
            ),
        ))
    } else {
        Err(Error::at(
            p.span,
            format!(
                "':{}' is not a side — use top, bottom, left, or right",
                p.name
            ),
        )
        .code(Code::UNKNOWN_SIDE))
    }
}

/// `right-top` → `top-right`: the corner glues vertical word first [SPEC 15.2].
fn corner_reorder(name: &str) -> Option<String> {
    let (a, b) = name.split_once('-')?;
    (matches!(a, "left" | "right") && matches!(b, "top" | "bottom")).then(|| format!("{b}-{a}"))
}
pub(super) fn endpoint_error(
    ep: &Endpoint,
    paths: &PathIndex,
    scope: &[String],
    op: ChainOp,
    drawing: bool,
) -> Error {
    // A capsule-declared endpoint whose trailing path fails [SPEC 9/21]: the
    // statement itself declared the node, so the path walked into a body the
    // inline form never authored — say that, not the generic walk error.
    // (`from_capsule` is the desugar hoist's hint; an inline define's
    // intrinsic children resolve normally and never reach here.)
    if let Some(ty) = &ep.from_capsule
        && ep.path.len() > 1
    {
        let (id, rest) = (&ep.path[0], ep.path[1..].join("."));
        return Error::at(
            ep.span,
            format!("'|{ty}#{id}|.{rest}' — an inline {ty} has no authored pins"),
        )
        .code(Code::UNKNOWN_ENDPOINT);
    }
    let where_ = if scope.is_empty() {
        "at scene root".to_string()
    } else {
        format!("in '{}'", scope.join("."))
    };
    // A drawing statement's endpoint is never auto-created [SPEC 15], so the
    // noun names what actually failed there — a `<->` in a drawing *is* a
    // dimension; elsewhere every statement is a link.
    let noun = match (op, drawing) {
        (_, false) => "link",
        (ChainOp::Mate, true) => "mate",
        (_, true) => "dimension",
    };
    // A copy index rides the spelling (`bolt.2`) — copies leak no ids, so the
    // carrierless form is exactly this unknown-endpoint error [SPEC 15.4].
    let mut spelled = ep.path.join(".");
    if let Some(k) = ep.copy {
        spelled.push('.');
        spelled.push_str(&k.to_string());
    }
    let mut msg = format!("{noun} endpoint '{spelled}' not found {where_}");
    let suggestions = paths.suggest(ep.path.last().expect("non-empty path"), scope);
    msg.push_str(&crate::suggest::did_you_mean(&suggestions));
    Error::at(ep.span, msg).code(Code::UNKNOWN_ENDPOINT)
}

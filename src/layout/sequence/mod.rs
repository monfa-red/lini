//! `layout: sequence` (SPEC §10) — a layout-owning container that reads its
//! participants, frames, and notes plus the scope's messages, fixes the lifeline
//! positions and time rows, and **lowers to primitive `PlacedNode`s** (lifelines /
//! arrows / frames / notes → `|line|` / `|block|` / text) through [`crate::layout::prim`].
//! The renderer, cascade, palette, and theming are reused unchanged, as for charts.
//!
//! It owns its scope's links: in a sequence scope a message's *order is time*, so the
//! orthogonal router ([LINKING.md]) is bypassed (`bundle` skips the scope) and the layout
//! draws each message itself — a horizontal arrow at its row (the `sequence` wiring
//! strategy, SPEC §10).
//!
//! Done: participants + lifelines, and messages (call / return / async / self) in
//! [`messages`]. Activations, frames, and notes follow in later steps (see PLAN.md).

mod messages;

use crate::error::Error;
use crate::layout::prim;
use crate::layout::{Bbox, PlacedNode};
use crate::resolve::{AttrMap, NodeKind, Program, ResolvedInst, ResolvedLink, ResolvedValue};
use crate::span::Span;
use std::collections::HashMap;

/// Type names that are **not** participants — the frames, the compartment separator, and
/// notes (SPEC §10). Every other box is a participant (the open fallback, unlike a chart's
/// closed series set).
const NON_PARTICIPANT: &[&str] = &["loop", "opt", "alt", "else", "note"];

/// Is this node a sequence container (SPEC §10)? Detected by its `layout:` attr — the same
/// key the chart / flow / grid dispatch reads — so it is intercepted before the generic
/// container path, exactly like `chart::is_chart`.
pub(super) fn is_sequence(attrs: &AttrMap) -> bool {
    matches!(attrs.get("layout"), Some(ResolvedValue::Ident(s)) if s == "sequence")
}

/// A `|sequence|` **node** (SPEC §10): lay out its participant children and return the
/// container `PlacedNode`. Intercepted in `layout_inst` before the generic path.
pub(super) fn layout_node(
    inst: &ResolvedInst,
    growth: &super::GapGrowth,
    path: &str,
    program: &Program,
) -> Result<PlacedNode, Error> {
    // Participants are real boxes — lay each out as usual, then arrange.
    let mut participants = Vec::new();
    for c in &inst.children {
        if is_participant(&c.kind, &c.type_chain) {
            participants.push(super::layout_inst(
                c,
                growth,
                &super::child_path(path, c),
                program,
            )?);
        }
    }
    let messages = messages_for(program, path);
    let (children, bbox) = lay_out(&inst.attrs, participants, &messages, inst.span)?;
    Ok(prim::container(inst, bbox, children))
}

/// A **root** sequence (`{ layout: sequence }`, SPEC §10): the scene's top-level nodes are
/// the participants (already laid out). Arrange them in place and append the lifelines,
/// returning the scene bbox. Intercepted in `attempt` before the generic arrange + route.
pub(super) fn layout_root(
    scene_nodes: &mut Vec<PlacedNode>,
    program: &Program,
) -> Result<Bbox, Error> {
    let participants = std::mem::take(scene_nodes)
        .into_iter()
        .filter(|p| is_participant(&p.kind, &p.type_chain))
        .collect();
    let messages = messages_for(program, "");
    let (children, bbox) = lay_out(&program.scene.attrs, participants, &messages, Span::empty())?;
    *scene_nodes = children;
    Ok(bbox)
}

/// Whether the container at `scope` is a `layout: sequence` — so the router skips its links
/// (they are drawn as time-row arrows here). Shared with the link partition (`bundle`).
pub(crate) fn is_sequence_scope(program: &Program, scope: &str) -> bool {
    scope_attrs(program, scope).is_some_and(is_sequence)
}

/// The attrs of the container at `scope` (`""` = the scene root).
fn scope_attrs<'a>(program: &'a Program, scope: &str) -> Option<&'a AttrMap> {
    if scope.is_empty() {
        Some(&program.scene.attrs)
    } else {
        super::node_at(program, scope).map(|i| &i.attrs)
    }
}

/// This sequence scope's messages — the resolved links written in it — in time (source)
/// order. The router never sees them ([`bundle`] skips a sequence scope).
fn messages_for<'a>(program: &'a Program, scope: &str) -> Vec<&'a ResolvedLink> {
    let mut msgs: Vec<&ResolvedLink> = program.links.iter().filter(|w| w.scope == scope).collect();
    msgs.sort_by_key(|w| w.span.start);
    msgs
}

/// Arrange participants across the top, drop a lifeline from each down to the last message
/// row, and draw the messages. Returns the lowered children (lifelines behind, headers,
/// then arrows) and the centred bbox. `gap: row col` — the column part spaces participants,
/// the row part is the message pitch (SPEC §10).
fn lay_out(
    attrs: &AttrMap,
    mut participants: Vec<PlacedNode>,
    messages: &[&ResolvedLink],
    span: Span,
) -> Result<(Vec<PlacedNode>, Bbox), Error> {
    if participants.is_empty() {
        return Err(Error::at(span, "a sequence needs at least one participant"));
    }
    let (gap_row, gap_col) = super::primitives::gap(attrs, span)?;

    // Time-ordered message pairs (a chain → consecutive pairs), and participant columns
    // widened so each message's label fits over its span.
    let pairs = messages::pairs(messages);
    let widths: Vec<f64> = participants.iter().map(|p| p.bbox.w()).collect();
    let ids: Vec<&str> = participants
        .iter()
        .map(|p| p.id.as_deref().unwrap_or(""))
        .collect();
    let centres = messages::columns(&widths, &ids, &pairs, gap_col);

    let header_h = participants
        .iter()
        .map(|p| p.bbox.h())
        .fold(0.0_f64, f64::max);
    let rows = pairs.len();
    let body_h = gap_row * (rows as f64 + 1.0); // a row per message at `gap_row`, plus a foot
    let total_h = header_h + body_h;
    let top = -total_h / 2.0;
    let header_bottom = top + header_h;
    let foot_y = header_bottom + body_h;
    let row_y = |i: usize| header_bottom + gap_row * (i as f64 + 1.0);

    // Place participants at their column centres, top-aligned; drop a lifeline to the foot.
    let stroke = lifeline_stroke(attrs);
    let mut lifelines = Vec::with_capacity(participants.len());
    let mut lifeline_x: HashMap<String, f64> = HashMap::new();
    for (p, &cx) in participants.iter_mut().zip(&centres) {
        p.cx = cx;
        p.cy = top + p.bbox.h() / 2.0;
        let head_bottom = p.cy + p.bbox.h() / 2.0;
        lifelines.push(prim::line(
            vec![(cx, head_bottom), (cx, foot_y)],
            stroke.clone(),
            1.0,
        ));
        if let Some(id) = p.id.as_deref() {
            lifeline_x.insert(id.to_string(), cx);
        }
    }

    let arrows = messages::draw(&pairs, &lifeline_x, row_y);

    // Lifelines behind, headers, then messages on top.
    let mut children = lifelines;
    children.extend(participants);
    children.extend(arrows);
    let bbox = enclosing_bbox(&children);
    Ok((children, bbox))
}

/// A symmetric, origin-centred bbox enclosing every child (lifelines, headers, arrows, and
/// labels — including any self-hook or label overflow), so a nested sequence's container is
/// sized correctly. Mirrors how `finish` takes the true visual extent.
fn enclosing_bbox(children: &[PlacedNode]) -> Bbox {
    let mut ext = Bbox::empty();
    for c in children {
        ext = ext.union(c.bbox.shifted(c.cx, c.cy));
    }
    let w = 2.0 * ext.min_x.abs().max(ext.max_x.abs());
    let h = 2.0 * ext.min_y.abs().max(ext.max_y.abs());
    Bbox::centered(w.max(1.0), h.max(1.0))
}

/// The lifeline colour: the scene's `stroke` if set, else the `--stroke` role var.
fn lifeline_stroke(attrs: &AttrMap) -> ResolvedValue {
    attrs
        .get("stroke")
        .cloned()
        .unwrap_or(ResolvedValue::LiveVar {
            name: "stroke".to_string(),
            raw: false,
        })
}

/// A participant is any drawn box that is not a frame / separator / note type (SPEC §10).
fn is_participant(kind: &NodeKind, type_chain: &[String]) -> bool {
    *kind != NodeKind::Text
        && !type_chain
            .iter()
            .any(|t| NON_PARTICIPANT.contains(&t.as_str()))
}

#[cfg(test)]
mod tests {
    /// Live-mode SVG for a source (palette vars stay `var(--lini-…)`).
    fn svg(src: &str) -> String {
        crate::compile_str(src).expect("compile")
    }

    /// The layout-phase error message for a sequence that resolves but won't lay out.
    fn layout_err(src: &str) -> String {
        let toks = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(&toks).expect("parse");
        let lowered = crate::desugar::desugar(&file).expect("desugar");
        let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
        crate::layout::layout(&program)
            .err()
            .expect("expected a layout error")
            .to_string()
    }

    #[test]
    fn root_sequence_draws_participant_headers_and_lifelines() {
        let s = svg("{ layout: sequence }\n|box#user| \"User\"\n|cyl#db| \"Store\"\n");
        assert!(s.contains(">User</text>"), "participant header: {s}");
        assert!(s.contains(">Store</text>"), "participant header: {s}");
        assert!(s.contains("lini-line"), "a lifeline per participant: {s}");
    }

    #[test]
    fn node_sequence_is_a_container_with_lifelines() {
        let s = svg("|sequence#s| [\n  |box#a| \"A\"\n  |box#b| \"B\"\n]\n");
        assert!(
            s.contains("lini-sequence"),
            "the sequence container class: {s}"
        );
        assert!(
            s.contains(">A</text>") && s.contains(">B</text>"),
            "headers: {s}"
        );
        assert!(s.contains("lini-line"), "lifelines: {s}");
    }

    #[test]
    fn participants_sit_in_a_row_left_to_right() {
        // Declaration order = left-to-right; distinct x centres prove the row layout.
        let toks = crate::lexer::lex("|sequence#s| [\n  |box#a| \"A\"\n  |box#b| \"B\"\n]\n")
            .expect("lex");
        let file = crate::syntax::parser::parse(&toks).expect("parse");
        let lowered = crate::desugar::desugar(&file).expect("desugar");
        let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
        let laid = crate::layout::layout(&program).expect("layout");
        let seq = &laid.nodes[0];
        let xs: Vec<f64> = seq
            .children
            .iter()
            .filter(|c| c.id.as_deref() == Some("a") || c.id.as_deref() == Some("b"))
            .map(|c| c.cx)
            .collect();
        assert_eq!(xs.len(), 2, "two participants placed");
        assert!(xs[0] < xs[1], "a left of b: {xs:?}");
    }

    #[test]
    fn an_empty_sequence_errors() {
        assert!(layout_err("|sequence#s|\n").contains("at least one participant"));
    }

    #[test]
    fn a_call_renders_as_an_arrow_not_routed() {
        let s = svg("{ layout: sequence }\n|box#a| \"A\"\n|box#b| \"B\"\na -> b \"hi\"\n");
        assert!(s.contains(">hi</text>"), "the message label: {s}");
        assert!(s.contains("lini-marker"), "an arrowhead: {s}");
        // The orthogonal router never sees a sequence message — it is lowered to an arrow.
        assert!(!s.contains("data-from"), "no routed link: {s}");
    }

    #[test]
    fn a_return_message_is_dashed() {
        let s = svg("{ layout: sequence }\n|box#a| \"A\"\n|box#b| \"B\"\nb --> a \"ok\"\n");
        assert!(
            s.contains("stroke-dasharray: 6"),
            "the return is dashed: {s}"
        );
    }

    #[test]
    fn an_async_message_is_wavy() {
        let s = svg("{ layout: sequence }\n|box#a| \"A\"\n|box#b| \"B\"\na ~> b \"event\"\n");
        assert!(
            s.contains("<path d=\"M"),
            "the async message is a wavy path: {s}"
        );
    }

    #[test]
    fn a_self_message_draws_a_hook() {
        let s = svg("{ layout: sequence }\n|box#a| \"A\"\na -> a \"retry\"\n");
        assert!(
            s.contains("<polyline"),
            "the self-message hook is a polyline: {s}"
        );
        assert!(s.contains(">retry</text>"), "its label: {s}");
    }
}

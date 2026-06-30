//! `layout: sequence` (SPEC §10) — a layout-owning container that reads its
//! participants, frames, and notes plus the scope's messages, fixes the lifeline
//! positions and time rows, and **lowers to primitive `PlacedNode`s** (lifelines /
//! arrows / frames / notes → `|line|` / `|block|` / text) through [`crate::layout::prim`].
//! The renderer, cascade, palette, and theming are reused unchanged, as for charts.
//!
//! It owns its scope's links: in a sequence scope a message's *order is time*, so the
//! orthogonal router ([LINKING.md]) is bypassed and the layout draws each message itself
//! (the `sequence` wiring strategy, SPEC §10) — that arrives in a later step.
//!
//! Step 2 (current): participants across the top, each with a lifeline. Messages,
//! activations, frames, and notes follow in later steps (see PLAN.md).

use crate::error::Error;
use crate::layout::prim;
use crate::layout::{Bbox, PlacedNode};
use crate::resolve::{AttrMap, NodeKind, ResolvedInst, ResolvedValue};
use crate::span::Span;

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
    funcs: &crate::expr::FuncTable,
) -> Result<PlacedNode, Error> {
    // Participants are real boxes — lay each out as usual, then arrange.
    let mut participants = Vec::new();
    for c in &inst.children {
        if is_participant(&c.kind, &c.type_chain) {
            participants.push(super::layout_inst(
                c,
                growth,
                &super::child_path(path, c),
                funcs,
            )?);
        }
    }
    let (children, bbox) = lay_out(&inst.attrs, participants, inst.span)?;
    Ok(prim::container(inst, bbox, children))
}

/// A **root** sequence (`{ layout: sequence }`, SPEC §10): the scene's top-level nodes are
/// the participants (already laid out). Arrange them in place and append the lifelines,
/// returning the scene bbox. Intercepted in `attempt` before the generic arrange + route.
pub(super) fn layout_root(
    scene_nodes: &mut Vec<PlacedNode>,
    attrs: &AttrMap,
) -> Result<Bbox, Error> {
    let participants = std::mem::take(scene_nodes)
        .into_iter()
        .filter(|p| is_participant(&p.kind, &p.type_chain))
        .collect();
    let (children, bbox) = lay_out(attrs, participants, Span::empty())?;
    *scene_nodes = children;
    Ok(bbox)
}

/// Arrange participants across the top, drop a lifeline from each, and return the lowered
/// children (lifelines behind, participants in front) plus the overall bbox — centred on
/// the origin, like a chart's lowered subtree.
fn lay_out(
    attrs: &AttrMap,
    mut participants: Vec<PlacedNode>,
    span: Span,
) -> Result<(Vec<PlacedNode>, Bbox), Error> {
    if participants.is_empty() {
        return Err(Error::at(span, "a sequence needs at least one participant"));
    }
    // `gap: row col` — the column part spaces participants; the row part is the message
    // pitch (used once messages land).
    let (gap_row, gap_col) = super::primitives::gap(attrs, span)?;
    let total_w: f64 = participants.iter().map(|p| p.bbox.w()).sum::<f64>()
        + gap_col * (participants.len() - 1) as f64;
    let header_h = participants
        .iter()
        .map(|p| p.bbox.h())
        .fold(0.0_f64, f64::max);
    // Step 2 reserves a short lifeline below the headers; Step 3 sets the foot to the last
    // message row.
    let body_h = gap_row.max(20.0) * 3.0;
    let total_h = header_h + body_h;
    let top = -total_h / 2.0;
    let foot_y = top + total_h;

    let stroke = lifeline_stroke(attrs);
    let mut lifelines = Vec::with_capacity(participants.len());
    let mut x = -total_w / 2.0;
    for p in &mut participants {
        let cx = x + p.bbox.w() / 2.0;
        p.cx = cx;
        p.cy = top + p.bbox.h() / 2.0; // headers top-aligned
        let head_bottom = p.cy + p.bbox.h() / 2.0;
        lifelines.push(prim::line(
            vec![(cx, head_bottom), (cx, foot_y)],
            stroke.clone(),
            1.0,
        ));
        x += p.bbox.w() + gap_col;
    }

    let mut children = lifelines; // drawn behind
    children.extend(participants); // headers in front
    Ok((children, Bbox::centered(total_w, total_h)))
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
}

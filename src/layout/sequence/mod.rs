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
//! Submodules: [`messages`] (call / return / async / self arrows), [`activations`]
//! (implicit bars), [`frames`] (`loop` / `opt` / `alt` + `else`). Notes follow (PLAN.md).

mod activations;
mod frames;
mod messages;
mod notes;

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
    // Participants and notes are real boxes — lay each out as usual, then arrange.
    let mut participants = Vec::new();
    let mut notes = Vec::new();
    for c in &inst.children {
        let placed = || super::layout_inst(c, growth, &super::child_path(path, c), program);
        if is_participant(&c.kind, &c.type_chain) {
            participants.push(placed()?);
        } else if is_note(&c.type_chain) {
            notes.push(placed()?);
        }
    }
    let messages = messages_for(program, path);
    let (children, bbox) = lay_out(
        &inst.attrs,
        participants,
        notes,
        &messages,
        &inst.children,
        inst.span,
    )?;
    Ok(prim::container(inst, bbox, children))
}

/// A **root** sequence (`{ layout: sequence }`, SPEC §10): the scene's top-level nodes are
/// the participants (already laid out). Arrange them in place and append the lifelines,
/// returning the scene bbox. Intercepted in `attempt` before the generic arrange + route.
pub(super) fn layout_root(
    scene_nodes: &mut Vec<PlacedNode>,
    program: &Program,
) -> Result<Bbox, Error> {
    let mut participants = Vec::new();
    let mut notes = Vec::new();
    for p in std::mem::take(scene_nodes) {
        if is_participant(&p.kind, &p.type_chain) {
            participants.push(p);
        } else if is_note(&p.type_chain) {
            notes.push(p);
        }
    }
    let messages = messages_for(program, "");
    let (children, bbox) = lay_out(
        &program.scene.attrs,
        participants,
        notes,
        &messages,
        &program.scene.nodes,
        Span::empty(),
    )?;
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
    notes: Vec<PlacedNode>,
    messages: &[&ResolvedLink],
    frame_src: &[ResolvedInst],
    span: Span,
) -> Result<(Vec<PlacedNode>, Bbox), Error> {
    if participants.is_empty() {
        return Err(Error::at(span, "a sequence needs at least one participant"));
    }
    let (gap_row, gap_col) = super::primitives::gap(attrs, span)?;

    // Time-ordered message pairs (a chain → consecutive pairs) and frames (depth-first),
    // then participant columns widened so each message's label fits over its span.
    let pairs = messages::pairs(messages);
    let seq_frames = frames::collect(frame_src);
    let widths: Vec<f64> = participants.iter().map(|p| p.bbox.w()).collect();
    let ids: Vec<&str> = participants
        .iter()
        .map(|p| p.id.as_deref().unwrap_or(""))
        .collect();
    let centres = messages::columns(&widths, &ids, &pairs, gap_col);

    // The shared timeline assigns each message a row y, each note its centre y, and each
    // frame its y-extent; its foot is the body height, which centres the diagram on origin.
    let note_rows: Vec<(usize, f64)> = notes.iter().map(|n| (n.span.start, n.bbox.h())).collect();
    let mut timeline = frames::timeline(&pairs, &seq_frames, &note_rows, gap_row);
    let header_h = participants
        .iter()
        .map(|p| p.bbox.h())
        .fold(0.0_f64, f64::max);
    let total_h = header_h + timeline.foot_y;
    let top = -total_h / 2.0;
    let header_bottom = top + header_h;
    timeline.shift(header_bottom);
    let foot_y = timeline.foot_y;
    let msg_y = &timeline.msg_y;
    let row_y = |i: usize| if i < msg_y.len() { msg_y[i] } else { foot_y };

    // Each participant lends its own paint to its **apparatus** — its lifeline and activation
    // bars (SPEC §10) — so colouring a participant colours its whole timeline, and a plain box
    // gives a `--stroke` line at width 1.5. Place participants at their column centres,
    // top-aligned, and drop a lifeline to the foot.
    let mut lifelines = Vec::with_capacity(participants.len());
    let mut lifeline_x: HashMap<String, f64> = HashMap::new();
    let mut paint: HashMap<String, Apparatus> = HashMap::new();
    for (p, &cx) in participants.iter_mut().zip(&centres) {
        p.cx = cx;
        p.cy = top + p.bbox.h() / 2.0;
        let head_bottom = p.cy + p.bbox.h() / 2.0;
        let a = Apparatus::of(&p.attrs);
        lifelines.push(prim::line(
            vec![(cx, head_bottom), (cx, foot_y)],
            a.stroke.clone(),
            a.width,
        ));
        if let Some(id) = p.id.as_deref() {
            lifeline_x.insert(id.to_string(), cx);
            paint.insert(id.to_string(), a);
        }
    }

    // Activation bars (SPEC §10): a per-participant LIFO stack over the messages, unless
    // `activation: none`. Message endpoints attach to a live bar's edge, so an arrow meets
    // the bar it opens rather than crossing the lifeline.
    let bars = if activations_on(attrs) {
        activations::bars(&pairs)
    } else {
        Vec::new()
    };
    let endpoint_x = |id: &str, row: usize, toward: f64| {
        let cx = lifeline_x.get(id).copied().unwrap_or(0.0);
        activations::edge(&bars, id, row, cx, toward).unwrap_or(cx)
    };
    let arrows = messages::draw(&pairs, &lifeline_x, endpoint_x, row_y);
    let bar_nodes = activations::draw(&bars, &lifeline_x, row_y, &paint);
    let (frames_behind, frames_front) =
        frames::draw(&seq_frames, &timeline.geom, &pairs, &lifeline_x);
    let placed_notes = place_notes(notes, &timeline.note_y, &lifeline_x);

    // Frame fills + borders behind (so a tinted fill backs the scene), then lifelines, bars,
    // headers, messages; frame tabs / guards and notes on top so they stay readable.
    let mut children = frames_behind;
    children.extend(lifelines);
    children.extend(bar_nodes);
    children.extend(participants);
    children.extend(arrows);
    children.extend(frames_front);
    children.extend(placed_notes);
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

/// The paint a participant lends its **apparatus** — its lifeline and activation bars (SPEC
/// §10). Read from the participant's own resolved attrs, so styling the participant styles
/// its timeline; a plain box falls back to `--fill` / `--stroke` at width 1.5.
pub(super) struct Apparatus {
    pub fill: ResolvedValue,
    pub stroke: ResolvedValue,
    pub width: f64,
}

impl Apparatus {
    fn of(attrs: &AttrMap) -> Self {
        Self {
            fill: attrs.get("fill").cloned().unwrap_or_else(|| live("fill")),
            stroke: attrs
                .get("stroke")
                .cloned()
                .unwrap_or_else(|| live("stroke")),
            width: attrs.number("stroke-width").unwrap_or(1.5),
        }
    }
}

/// Activation bars are drawn unless `activation: none` (SPEC §10).
fn activations_on(attrs: &AttrMap) -> bool {
    !matches!(attrs.get("activation"), Some(ResolvedValue::Ident(s)) if s == "none")
}

/// A `--name` role variable as a live value — palette colours stay themeable. The one
/// place the engine names a role var, shared by the lifelines, bars, and messages.
pub(super) fn live(name: &str) -> ResolvedValue {
    ResolvedValue::LiveVar {
        name: name.to_string(),
        raw: false,
    }
}

/// A participant is any drawn box that is not a frame / separator / note type (SPEC §10).
fn is_participant(kind: &NodeKind, type_chain: &[String]) -> bool {
    *kind != NodeKind::Text
        && !type_chain
            .iter()
            .any(|t| NON_PARTICIPANT.contains(&t.as_str()))
}

/// A `|note|` — a callout placed beside / over lifelines, not a participant (SPEC §10).
fn is_note(type_chain: &[String]) -> bool {
    type_chain.iter().any(|t| t == "note")
}

/// The properties valid only in a sequence (SPEC §16): a note's placement and the
/// activation toggle.
const SEQ_PROPS: &[&str] = &["over", "left", "right", "activation"];

/// Validate sequence structure (SPEC §16), before layout: a frame / note / `|else|` belongs
/// in a sequence (an `|else|` directly in an `|alt|`), a note needs a placement, and the
/// sequence properties are valid only in a sequence. Walks the scene tracking whether each
/// node sits in a sequence scope (a sequence's own body, or a frame nested in one) and
/// whether it sits directly in an `|alt|`.
pub(crate) fn validate(program: &Program) -> Result<(), Error> {
    let in_seq = is_sequence(&program.scene.attrs);
    for n in &program.scene.nodes {
        check_node(n, in_seq, false)?;
    }
    Ok(())
}

fn check_node(inst: &ResolvedInst, in_seq: bool, in_alt: bool) -> Result<(), Error> {
    let is = |t: &str| inst.type_chain.iter().any(|x| x == t);
    let seq_ctx = in_seq || is_sequence(&inst.attrs);

    // Frame and note types belong in a sequence.
    for ty in ["loop", "opt", "alt", "note"] {
        if is(ty) && !in_seq {
            return Err(Error::at(
                inst.span,
                format!("'|{ty}|' belongs in a 'layout: sequence'"),
            ));
        }
    }
    if is("else") && !in_alt {
        return Err(Error::at(
            inst.span,
            "'|else|' separates an '|alt|' — write it inside one",
        ));
    }
    if is("note") && notes::placement(&inst.attrs).is_none() {
        return Err(Error::at(
            inst.span,
            "a '|note|' needs 'over:', 'left:', or 'right:'",
        ));
    }
    if !seq_ctx {
        for p in SEQ_PROPS {
            if inst.attrs.get(p).is_some() {
                return Err(Error::at(
                    inst.span,
                    format!("'{p}' is valid only in a 'layout: sequence'"),
                ));
            }
        }
    }

    // A sequence's own body and the bodies of its frames are in-sequence; a participant's
    // children (its own content) are not. `|else|` only ever separates a direct `|alt|` child.
    let child_in_seq =
        is_sequence(&inst.attrs) || (in_seq && (is("loop") || is("opt") || is("alt")));
    for c in &inst.children {
        check_node(c, child_in_seq, is("alt"))?;
    }
    Ok(())
}

/// Fix each laid-out note box at its time row (`note_y`) and over its placed lifelines
/// (`over` / `left` / `right`). A note naming an unknown participant is dropped.
fn place_notes(
    notes: Vec<PlacedNode>,
    note_y: &[f64],
    lifeline_x: &HashMap<String, f64>,
) -> Vec<PlacedNode> {
    notes
        .into_iter()
        .zip(note_y)
        .filter_map(|(mut n, &y)| {
            let placement = notes::placement(&n.attrs)?;
            n.cx = notes::centre_x(&placement, n.bbox.w(), lifeline_x)?;
            n.cy = y;
            Some(n)
        })
        .collect()
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

    // ── Activations (SPEC §10) ──

    /// Activation bars are the anonymous `Block` rects on the lifelines — distinct from
    /// the id'd participant headers and the `Line` lifelines / arrows.
    fn bar_count(src: &str) -> usize {
        let toks = crate::lexer::lex(src).expect("lex");
        let file = crate::syntax::parser::parse(&toks).expect("parse");
        let lowered = crate::desugar::desugar(&file).expect("desugar");
        let program = crate::resolve::resolve_with_theme(&lowered, &[]).expect("resolve");
        let laid = crate::layout::layout(&program).expect("layout");
        laid.nodes[0]
            .children
            .iter()
            .filter(|c| c.kind == crate::resolve::NodeKind::Block && c.id.is_none())
            .count()
    }

    #[test]
    fn a_call_opens_one_activation_bar() {
        // A call opens a bar on its target; the matching return closes it — one bar.
        let n = bar_count(
            "|sequence#s| [\n  |box#a| \"A\"\n  |box#b| \"B\"\n  a -> b \"q\"\n  b --> a \"r\"\n]\n",
        );
        assert_eq!(n, 1, "one activation bar");
    }

    #[test]
    fn nested_calls_stack_two_bars() {
        // Two calls to the same target before any return stack two bars.
        let n = bar_count(
            "|sequence#s| [\n  |box#a| \"A\"\n  |box#b| \"B\"\n  a -> b \"c1\"\n  a -> b \"c2\"\n  b --> a \"r2\"\n  b --> a \"r1\"\n]\n",
        );
        assert_eq!(n, 2, "two stacked bars");
    }

    #[test]
    fn self_and_async_open_no_bar() {
        // A self-message and an async (`~>`) open none (SPEC §10).
        let n = bar_count(
            "|sequence#s| [\n  |box#a| \"A\"\n  |box#b| \"B\"\n  a -> a \"loop\"\n  a ~> b \"event\"\n]\n",
        );
        assert_eq!(n, 0, "self and async open no activation");
    }

    #[test]
    fn activation_none_draws_no_bars() {
        let n = bar_count(
            "|sequence#s| { activation: none } [\n  |box#a| \"A\"\n  |box#b| \"B\"\n  a -> b \"q\"\n  b --> a \"r\"\n]\n",
        );
        assert_eq!(n, 0, "activation: none suppresses bars");
    }

    // ── Frames (SPEC §10) ──

    #[test]
    fn a_loop_frame_draws_its_tab_and_guard() {
        let s = svg(
            "{ layout: sequence }\n|box#a| \"A\"\n|box#b| \"B\"\n|loop| \"5x\" [\n  a -> b \"poll\"\n]\n",
        );
        assert!(s.contains(">loop</text>"), "the operator tab: {s}");
        assert!(s.contains(">[5x]</text>"), "the guard: {s}");
    }

    #[test]
    fn an_alt_splits_into_guarded_compartments() {
        let s = svg(
            "{ layout: sequence }\n|box#a| \"A\"\n|box#b| \"B\"\n|alt| \"ok\" [\n  a -> b \"x\"\n  |else| \"no\"\n  a -> b \"y\"\n]\n",
        );
        assert!(s.contains(">alt</text>"), "the alt tab: {s}");
        assert!(
            s.contains(">[ok]</text>"),
            "the first compartment guard: {s}"
        );
        assert!(
            s.contains(">[no]</text>"),
            "the else compartment guard: {s}"
        );
    }

    #[test]
    fn frames_nest() {
        let s = svg(
            "{ layout: sequence }\n|box#a| \"A\"\n|box#b| \"B\"\n|loop| \"r\" [\n  |opt| \"o\" [\n    a -> b \"x\"\n  ]\n]\n",
        );
        assert!(
            s.contains(">loop</text>") && s.contains(">opt</text>"),
            "both nested frame tabs render: {s}"
        );
    }

    // ── Notes (SPEC §10) ──

    #[test]
    fn a_note_renders_over_its_lifelines() {
        let s = svg(
            "{ layout: sequence }\n|box#a| \"A\"\n|box#b| \"B\"\n|note| \"spanning\" { over: a b }\na -> b \"x\"\n",
        );
        assert!(s.contains(">spanning</text>"), "the note text renders: {s}");
    }

    // ── Structural errors (SPEC §16) ──

    #[test]
    fn a_frame_outside_a_sequence_errors() {
        assert!(layout_err("|loop| [\n  |box#a|\n]\n").contains("belongs in a 'layout: sequence'"));
    }

    #[test]
    fn an_else_outside_an_alt_errors() {
        assert!(
            layout_err("{ layout: sequence }\n|box#a| \"A\"\n|else| \"x\"\n")
                .contains("separates an '|alt|'")
        );
    }

    #[test]
    fn a_note_without_placement_errors() {
        assert!(
            layout_err("{ layout: sequence }\n|box#a| \"A\"\n|note| \"hi\"\n")
                .contains("needs 'over:', 'left:', or 'right:'")
        );
    }

    #[test]
    fn a_sequence_property_off_a_sequence_errors() {
        assert!(
            layout_err("|box#a| { activation: none }\n")
                .contains("valid only in a 'layout: sequence'")
        );
    }
}

//! `layout: sequence` [SPEC 13] — a layout-owning container that reads its
//! participants, frames, and notes plus the scope's messages, fixes the lifeline
//! positions and time rows, and **lowers to primitive `PlacedNode`s** (lifelines /
//! arrows / frames / notes → `|line|` / `|block|` / text) through [`crate::layout::prim`].
//! The renderer, cascade, palette, and theming are reused unchanged, as for charts.
//!
//! It owns its scope's links: in a sequence scope a message's *order is time*, so the
//! orthogonal router ([ROUTING.md]) is bypassed (`bundle` skips the scope) and the layout
//! draws each message itself — a horizontal arrow at its row (the `sequence` wiring
//! strategy, [SPEC 13]).
//!
//! Submodules: [`messages`] (call / return / async / self arrows), [`activations`]
//! (implicit bars), [`frames`] (`loop` / `opt` / `alt` + `else`). Notes follow [SPEC 13].

mod activations;
mod frames;
pub(crate) use frames::is_frame;
pub(crate) mod messages;
mod notes;

use crate::error::Error;
use crate::layout::prim;
use crate::layout::{Bbox, PlacedNode, RoutedLink};
use crate::resolve::{AttrMap, NodeKind, Program, ResolvedInst, ResolvedLink, ResolvedValue};
use crate::span::Span;
use std::collections::HashMap;

/// Type names that are **not** participants — the frames, the compartment separator, and
/// notes [SPEC 13]. Every other box is a participant (the open fallback, unlike a chart's
/// closed series set).
const NON_PARTICIPANT: &[&str] = &["loop", "opt", "alt", "else", "note"];

/// Is this node a sequence container [SPEC 13]? Detected by its `layout:` attr — the same
/// key the chart / flow / grid dispatch reads — so it is intercepted before the generic
/// container path, exactly like `chart::is_chart`.
pub(super) fn is_sequence(attrs: &AttrMap) -> bool {
    matches!(attrs.get("layout"), Some(ResolvedValue::Ident(s)) if s == "sequence")
}

/// A `|sequence|` **node** [SPEC 13]: lay out its participant children and return the
/// container `PlacedNode`. Intercepted in `layout_inst` before the generic path.
pub(super) fn layout_node(
    inst: &ResolvedInst,
    path: &str,
    program: &Program,
) -> Result<PlacedNode, Error> {
    // Participants and notes are real boxes — lay each out as usual, then arrange. A
    // sequence's interior is sheet-space [SPEC 15.1], so neither inherits an enclosing
    // drawing's view scale. Notes are gathered through the frames too [SPEC 13].
    let place = |c: &ResolvedInst| {
        super::layout_inst(c, &super::child_path(path, c), program, super::Ctx::sheet())
    };
    let mut participants = Vec::new();
    let mut rest: Vec<&ResolvedInst> = Vec::new();
    for c in &inst.children {
        if is_participant(&c.kind, &c.type_chain) {
            participants.push(place(c)?);
        } else {
            rest.push(c);
        }
    }
    let mut note_insts = Vec::new();
    notes::collect(rest, &mut note_insts);
    let notes = note_insts
        .into_iter()
        .map(place)
        .collect::<Result<Vec<_>, _>>()?;
    // An **anonymous** sequence node is scope-transparent [SPEC 9]: its path is
    // its parent's, and its (mis-scoped) messages resolved there — consuming by
    // path here would steal the parent's links instead.
    let messages = if inst.id.is_some() {
        messages_for(program, path)
    } else {
        Vec::new()
    };
    let (children, bbox, wires) = lay_out(
        &inst.attrs,
        participants,
        notes,
        &messages,
        &inst.children,
        inst.span,
    )?;
    let mut node = prim::container(inst, bbox, children);
    node.links = wires;
    Ok(node)
}

/// A **root** sequence (`{ layout: sequence }`, [SPEC 13]): the scene's top-level nodes are
/// the participants (already laid out). Arrange them in place and append the lifelines,
/// returning the scene bbox and the message wires (already in scene coordinates).
/// Intercepted in `layout` before the generic arrange + route.
pub(super) fn layout_root(
    scene_nodes: &mut Vec<PlacedNode>,
    program: &Program,
) -> Result<(Bbox, Vec<RoutedLink>), Error> {
    let mut participants = Vec::new();
    let mut rest = Vec::new();
    for p in std::mem::take(scene_nodes) {
        if is_participant(&p.kind, &p.type_chain) {
            participants.push(p);
        } else {
            rest.push(p);
        }
    }
    // The notes among them — inside the frames' placed boxes too [SPEC 13].
    let mut notes = Vec::new();
    notes::collect(rest, &mut notes);
    let messages = messages_for(program, "");
    let (children, bbox, wires) = lay_out(
        &program.scene.attrs,
        participants,
        notes,
        &messages,
        &program.scene.nodes,
        Span::empty(),
    )?;
    *scene_nodes = children;
    Ok((bbox, wires))
}

/// Whether the container at `scope` is a `layout: sequence` — so the router skips its links
/// (they are drawn as time-row arrows here). Shared with the link partition (`bundle`).
pub(crate) fn is_sequence_scope(program: &Program, scope: &str) -> bool {
    super::scope_attrs(program, scope).is_some_and(is_sequence)
}

/// This sequence scope's messages — the resolved links written in it — in time (source)
/// order. The router never sees them ([`bundle`] skips a sequence scope).
fn messages_for<'a>(program: &'a Program, scope: &str) -> Vec<&'a ResolvedLink> {
    let mut msgs: Vec<&ResolvedLink> = program.links.iter().filter(|w| w.scope == scope).collect();
    msgs.sort_by_key(|w| w.span.start);
    msgs
}

/// Arrange participants across the top, drop a lifeline from each down to the last message
/// row, and draw the messages through the `straight` strategy. Returns the lowered children
/// (lifelines behind, headers, frames, notes), the centred bbox, and the message wires (the
/// renderer's one link path draws them). `gap: row col` — the column part spaces
/// participants, the row part is the message pitch [SPEC 13].
fn lay_out(
    attrs: &AttrMap,
    mut participants: Vec<PlacedNode>,
    notes: Vec<PlacedNode>,
    messages: &[&ResolvedLink],
    frame_src: &[ResolvedInst],
    span: Span,
) -> Result<(Vec<PlacedNode>, Bbox, Vec<RoutedLink>), Error> {
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

    // Each participant lends its **paint** to its apparatus — lifeline and activation bars
    // [SPEC 13] — so colouring or weighting a participant carries through its whole timeline.
    // A node comes with its lifeline: the lifeline takes the participant's stroke colour *and*
    // width, and the bars keep the same paint. Place participants at their column centres,
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

    // Activation bars [SPEC 13]: a per-participant LIFO stack over the messages, unless
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
    let wires = messages::draw(&pairs, &lifeline_x, endpoint_x, row_y);
    let bar_nodes = activations::draw(&bars, &lifeline_x, row_y, &paint);
    let (frames_behind, frames_front) =
        frames::draw(&seq_frames, &timeline.geom, &pairs, &lifeline_x);
    let placed_notes = place_notes(notes, &timeline.note_y, &lifeline_x);

    // Frame fills + borders behind (so a tinted fill backs the scene), then lifelines, bars,
    // headers; frame tabs / guards and notes on top so they stay readable. The messages ride
    // the link layer, drawn over the whole scene by the renderer's one link path.
    let mut children = frames_behind;
    children.extend(lifelines);
    children.extend(bar_nodes);
    children.extend(participants);
    children.extend(frames_front);
    children.extend(placed_notes);
    let bbox = enclosing_bbox(&children, &wires);
    Ok((children, bbox, wires))
}

/// A symmetric, origin-centred bbox enclosing every child and every message
/// wire (including any self-hook or label overflow), so a nested sequence's
/// container is sized correctly. Mirrors how `finish` takes the true visual
/// extent.
fn enclosing_bbox(children: &[PlacedNode], wires: &[RoutedLink]) -> Bbox {
    let mut ext = Bbox::extent_of(children, |_| true);
    for w in wires {
        for &(x, y) in &w.path {
            ext = ext.union(Bbox {
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            });
        }
        for t in &w.texts {
            ext = ext.union(
                crate::layout::text::measure(&t.content, &t.attrs)
                    .shifted(t.position.0, t.position.1),
            );
        }
    }
    let w = 2.0 * ext.min_x.abs().max(ext.max_x.abs());
    let h = 2.0 * ext.min_y.abs().max(ext.max_y.abs());
    Bbox::centered(w.max(1.0), h.max(1.0))
}

/// The paint a participant lends its **apparatus** — its lifeline and activation bars
/// [SPEC 10]. Read from the participant's own resolved attrs, so styling the participant styles
/// its timeline; a plain box falls back to `--fill` / `--stroke` at width 1.5.
pub(super) struct Apparatus {
    pub fill: ResolvedValue,
    pub stroke: ResolvedValue,
    pub width: f64,
}

impl Apparatus {
    fn of(attrs: &AttrMap) -> Self {
        Self {
            fill: attrs
                .get("fill")
                .cloned()
                .unwrap_or_else(|| ResolvedValue::live("fill")),
            stroke: attrs
                .get("stroke")
                .cloned()
                .unwrap_or_else(|| ResolvedValue::live("stroke")),
            width: attrs.number("stroke-width").unwrap_or(2.0),
        }
    }
}

/// Activation bars are drawn unless `activation: none` [SPEC 13].
fn activations_on(attrs: &AttrMap) -> bool {
    !matches!(attrs.get("activation"), Some(ResolvedValue::Ident(s)) if s == "none")
}

/// A participant is any drawn box that is not a frame / separator / note type [SPEC 13].
fn is_participant(kind: &NodeKind, type_chain: &[String]) -> bool {
    *kind != NodeKind::Text
        && !type_chain
            .iter()
            .any(|t| NON_PARTICIPANT.contains(&t.as_str()))
}

/// The properties valid only in a sequence [SPEC 21]: a note's placement and the
/// activation toggle.
const SEQ_PROPS: &[&str] = &["place", "activation"];

/// Validate sequence structure [SPEC 21], before layout: a frame / note / `|else|` belongs
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

    // Frame types belong in a sequence. (A `|note|` is a core template
    // [SPEC 8] — legal in any layout; only its placement is sequence business.)
    for ty in ["loop", "opt", "alt"] {
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
    if in_seq && is("note") && notes::placement(&inst.attrs, inst.span)?.is_none() {
        return Err(Error::at(inst.span, "a sequence '|note|' needs 'place:'"));
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
            // Validated when the note was collected; a malformed `place:`
            // never reaches here.
            let placement = notes::placement(&n.attrs, n.span).ok().flatten()?;
            let (cx, w) = notes::box_at(&placement, n.bbox.w(), lifeline_x)?;
            n.cx = cx;
            n.cy = y;
            // An `over` note spanning several lifelines is a box across them
            // [SPEC 13]: widen the card and re-cut its silhouette at the new size.
            if w > n.bbox.w() {
                n.bbox = Bbox::centered(w, n.bbox.h());
                super::note::fold(&mut n);
            }
            // `translate: x y` nudges a note off its placement, so it can be positioned by
            // hand [SPEC 5] — the one post-placement mechanism, reused here.
            let _ = super::anchors::nudge(&mut n, super::anchors::SHEET_SPACE);
            // The silhouette was folded by the generic arranger — the core
            // |note| look [SPEC 8]; this engine only places the card.
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
    use crate::testutil::layout_err;

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
        let laid = crate::testutil::laid("|sequence#s| [\n  |box#a| \"A\"\n  |box#b| \"B\"\n]\n");
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
    fn a_call_renders_as_a_straight_time_row_wire() {
        let s = svg("{ layout: sequence }\n|box#a| \"A\"\n|box#b| \"B\"\na -> b \"hi\"\n");
        assert!(s.contains(">hi</text>"), "the message label: {s}");
        assert!(s.contains("lini-marker"), "an arrowhead: {s}");
        // The message rides the shared link layer through the `straight`
        // strategy [SPEC 13] — a drawn link, never an orthogonal route.
        assert!(
            s.contains(r#"data-from="a" data-to="b""#),
            "the message is a drawn link: {s}"
        );
    }

    #[test]
    fn a_return_message_is_dashed() {
        let s = svg("{ layout: sequence }\n|box#a| \"A\"\n|box#b| \"B\"\nb --> a \"ok\"\n");
        assert!(
            s.contains("stroke-dasharray: 6,4.5"),
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
        // The hook is a rounded path — an arc joins its legs (clearance-driven turn) —
        // ending in an arrowhead that returns to the lifeline; its label rides above.
        assert!(
            s.contains(" A "),
            "the self-message hook bends through an arc: {s}"
        );
        assert!(
            s.contains("lini-marker-arrow"),
            "the hook returns with an arrowhead: {s}"
        );
        assert!(s.contains(">retry</text>"), "its label: {s}");
    }

    // ── Activations [SPEC 13] ──

    /// Activation bars are the anonymous `Block` rects on the lifelines — distinct from
    /// the id'd participant headers and the `Line` lifelines / arrows.
    fn bar_count(src: &str) -> usize {
        crate::testutil::laid(src).nodes[0]
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
        // A self-message and an async (`~>`) open none [SPEC 13].
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

    // ── Frames [SPEC 13] ──

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

    // ── Notes [SPEC 13] ──

    #[test]
    fn a_note_renders_over_its_lifelines() {
        let s = svg(
            "{ layout: sequence }\n|box#a| \"A\"\n|box#b| \"B\"\n|note| \"spanning\" { place: over a b }\na -> b \"x\"\n",
        );
        assert!(s.contains(">spanning</text>"), "the note text renders: {s}");
    }

    #[test]
    fn a_note_inside_a_frame_is_kept() {
        // [SPEC 13]: a frame's `[ ]` opens no scope — its note is the
        // sequence's own, at its source-order row. It was silently dropped.
        let s = svg(
            "{ layout: sequence }\na -> b \"m1\"\n|alt| \"ok\" [\n  b --> a \"m2\"\n  |note| \"careful\" { place: over a }\n]\n",
        );
        assert!(
            s.contains(">careful</text>"),
            "the framed note renders: {s}"
        );
    }

    #[test]
    fn an_over_note_spans_its_lifelines() {
        // [SPEC 13]: `place: over a c` is a box spanning those lifelines and
        // any between — not a centred card.
        let l = crate::testutil::laid(
            "{ layout: sequence }\n|box#a| \"A\"\n|box#b| \"B\"\n|box#c| \"C\"\n|note| \"wide\" { place: over a c }\na -> c \"x\"\n",
        );
        let find = |pred: &crate::testutil::Pred<'_>| {
            crate::testutil::find_placed(&l.nodes, pred).map(|(n, x, _)| (n, x))
        };
        let (note, _) = find(&|n| n.type_chain.iter().any(|t| t == "note")).expect("the note node");
        let (_, ax) = find(&|n| n.id.as_deref() == Some("a")).expect("a");
        let (_, cx) = find(&|n| n.id.as_deref() == Some("c")).expect("c");
        assert!(
            note.bbox.w() >= (cx - ax).abs(),
            "the note spans a→c: note {} vs span {}",
            note.bbox.w(),
            (cx - ax).abs()
        );
    }

    // ── Structural errors [SPEC 21] ──

    /// Every structural refusal the sequence engine owns, one row per
    /// diagnostic — a message is a contract with the author [SPEC 13/21].
    #[test]
    fn sequence_errors_speak_spec() {
        for (src, want) in [
            (
                "|loop| [\n  |box#a|\n]\n",
                "belongs in a 'layout: sequence'",
            ),
            (
                "{ layout: sequence }\n|box#a| \"A\"\n|else| \"x\"\n",
                "separates an '|alt|'",
            ),
            (
                "{ layout: sequence }\n|box#a| \"A\"\n|note| \"hi\"\n",
                "needs 'place:'",
            ),
            // A mode then its lifelines [SPEC 13/20]: `left` takes exactly one.
            (
                "{ layout: sequence }\n|box#a| \"A\"\n|box#b| \"B\"\n|note| \"hi\" { place: left a b }\na -> b \"x\"\n",
                "'place' is a mode then its lifelines",
            ),
            (
                "|box#a| { activation: none }\n",
                "valid only in a 'layout: sequence'",
            ),
        ] {
            let e = layout_err(src);
            assert!(e.contains(want), "{src:?}\n  wanted {want:?}, got {e:?}");
        }
    }
}

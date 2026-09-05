//! Label / `along:` lowering helpers, used by the full desugar pass ([`super`]).
//! The smart label (a box's text, a group's caption, an icon's symbol) and a
//! link's auto-distributed `along:` fractions are each a small, reusable
//! transform [SPEC 3, 7, 9, 16].

use super::pose::Pose;
use super::{Lower, Nest, header_node, lower_node, schematic, synth};
use crate::ast::ChainOp;
use crate::error::Error;
use crate::resolve::NodeKind;
use crate::span::Span;
use crate::syntax::ast::{Child, Decl, LabelItem, Link, Node, TextNode, Value};

/// What a node is, as the smart label reads it — the one dispatch behind
/// [`lower_smart`], gathered where the node's chain is known.
pub(super) struct Smart {
    is_icon: bool,
    is_entity: bool,
    is_drawing: bool,
    /// A container whose body is a schematic scope [SPEC 16.6] — its caption
    /// sits inside the frame, as a sheet titles a block.
    is_schematic: bool,
    is_container: bool,
    /// Geometry-only shapes (line/poly/path/image) hold no text.
    text_capable: bool,
    sch: Option<schematic::SchKind>,
    /// A schematic part's pose [SPEC 16.1] — its value readout seats off the
    /// axis a turned part's wire runs down.
    pose: Pose,
}

impl Smart {
    pub(super) fn read(
        kind: NodeKind,
        chain: &[String],
        is_entity: bool,
        is_drawing: bool,
        is_schematic: bool,
        sch: Option<schematic::SchKind>,
        pose: Pose,
    ) -> Smart {
        Smart {
            is_icon: kind == NodeKind::Icon,
            is_entity,
            is_drawing,
            is_schematic,
            is_container: chain.iter().any(|n| n == "group"),
            text_capable: !matches!(
                kind,
                NodeKind::Line | NodeKind::Poly | NodeKind::Path | NodeKind::Image
            ),
            sch,
            pose,
        }
    }
}

/// The smart label, lowered per type [SPEC 3/7] — the single shared lowering
/// for a node's text (a link's labels go through the same [`TextNode`]). A
/// box-like type → centred text prepended; a group/table → a `|caption|`
/// child; an icon/sign → the `symbol`; a drawing → a `|footnote|` title; a
/// schematic part → its name/value readout [SPEC 16.2]. An empty `""` lowers
/// to nothing. Returns the label a geometry primitive keeps as its *name*.
pub(super) fn lower_smart(
    cx: &Lower,
    node: &Node,
    label: Option<&TextNode>,
    what: &Smart,
    style: &mut Vec<Decl>,
    children: &mut Vec<Child>,
) -> Result<Option<TextNode>, Error> {
    let mut kept_label = None;
    if let Some(label) = label {
        if what.is_icon {
            if style.iter().any(|d| d.name == "symbol") {
                return Err(Error::at(
                    node.span,
                    "an icon's symbol is its label or 'symbol:', not both",
                ));
            }
            style.push(symbol_decl(&label.text, node.span));
        } else if what.is_entity {
            // An entity's label is its title: the `|header|` at the grid's
            // top-left [SPEC 8]. It spans every column — a width the resolved
            // `columns:` gives it (`crate::resolve::tables`), never a count
            // read off the source here.
            let title = header_node(label);
            children.insert(0, Child::Box(lower_node(cx, &title, Nest::NONE)?));
        } else if what.is_drawing {
            // A drawing's smart label is its title, lowered to a |footnote|
            // under the view [SPEC 15.8] — drafting titles sit **under** the
            // view, so it rides the bottom-centred caption template and
            // `|drawing| |footnote| { … }` styles it.
            let title = lower_node(cx, &synth::labelled("footnote", label.clone()), Nest::NONE)?;
            children.insert(0, Child::Box(title));
        } else if let Some(kind) = what.sch.filter(|k| *k != schematic::SchKind::Label) {
            // A part's smart label is its name / value [SPEC 16.2/16.3], drawn
            // as readout chrome at the seat its family and pose give it.
            let readout = schematic::value_readout(cx, &label.text, kind, what.pose, children)?;
            children.push(readout);
        } else if what.is_container {
            // A container's label is a `|caption|` child [SPEC 3/8], lowered
            // through the normal node path so it gains its `.lini-caption`
            // chain and its centred text child — a schematic scope's the
            // `|sheet-caption|` seated inside its frame [SPEC 16.6].
            let ty = if what.is_schematic {
                "sheet-caption"
            } else {
                "caption"
            };
            let caption = lower_node(cx, &synth::labelled(ty, label.clone()), Nest::NONE)?;
            children.insert(0, Child::Box(caption));
        } else if what.text_capable {
            children.insert(0, Child::Text(label.clone()));
        } else {
            // Geometry primitives (line/poly/path/image) draw no text, but a label
            // still *names* the node — keep it so a chart can read a `|line|` series'
            // legend name. Inert for a standalone primitive (render ignores it).
            kept_label = Some(label.clone());
        }
    }
    // A view sourced from a marker (`of:`) with no authored label composes its
    // title [SPEC 15.8]: seed a placeholder |footnote| the engine fills where it
    // pins the title — the marker (a `|plane|` → `A-A`, a `|magnifier|` → `C`)
    // and the scale ratio are both known there.
    if what.is_drawing
        && node.style.iter().any(|d| d.name == "of")
        && node.label.as_ref().filter(|l| !l.text.is_empty()).is_none()
    {
        let foot = of_footnote(node.span);
        children.insert(0, Child::Box(lower_node(cx, &foot, Nest::NONE)?));
    }
    Ok(kept_label)
}

/// A placeholder title `|footnote|` for a marker-sourced view [SPEC 15.8]: a
/// `|drawing| { of: X }` with no authored label seeds this carrying a bare
/// `of-title` marker. The letter (and doubled-or-not) come from X's kind, and
/// the scale ratio from the seat — both known only at layout, so the drawing
/// engine fills the text where it pins the title.
pub(super) fn of_footnote(span: Span) -> Node {
    synth::styled(
        "footnote",
        vec![Decl {
            name: "of-title".to_string(),
            groups: vec![vec![Value::Ident("view".to_string())]],
            span,
        }],
        span,
    )
}

/// The `symbol: <name>` declaration an icon's smart label lowers to [SPEC 7].
pub(super) fn symbol_decl(name: &str, span: Span) -> Decl {
    Decl {
        name: "symbol".to_string(),
        groups: vec![vec![Value::Ident(name.to_string())]],
        span,
    }
}

/// Lower a link's labels [SPEC 9]: the head label leads, then the `[ ]` labels;
/// the combined list feeds auto-`along:`. The output carries `label: None`, the
/// full list in `labels`, and — when no `along:` was written — an even-fraction
/// `along:` prepended to its style.
/// Chain expansion [SPEC 9/18]: `a -> b -> c` is exactly `a -> b; b -> c` —
/// each hop an independent link carrying the operator's full markers and the
/// statement's label, classes, and `{ }` (they apply to every expanded link),
/// with its own hop operator and the statement's span (the router groups a
/// statement's wires by span, so hop labels and crossings stay per-statement).
/// Only wire chains split: a measure chain shares one dim row and a mate
/// seats pairs — their hop semantics belong to the drawing engine.
pub(super) fn split_chain(w: &Link) -> Vec<Link> {
    if w.chain.len() <= 2 || !matches!(w.op(), ChainOp::Wire(_)) {
        return vec![w.clone()];
    }
    w.chain
        .windows(2)
        .enumerate()
        .map(|(i, pair)| Link {
            chain: pair.to_vec(),
            ops: vec![w.ops[i]],
            classes: w.classes.clone(),
            style: w.style.clone(),
            style_span: w.style_span,
            label: w.label.clone(),
            labels: w.labels.clone(),
            span: w.span,
        })
        .collect()
}

pub(super) fn lower_link(w: &Link, cx: &super::Lower, nest: Nest) -> Result<Link, Error> {
    let mut labels = Vec::new();
    if let Some(head) = &w.label {
        labels.push(LabelItem::Text(head.clone()));
    }
    // Carried annotation nodes lower through the one node path (template →
    // primitive + `.lini-*`) in place [SPEC 15.9]; a node is never a label.
    for item in &w.labels {
        labels.push(match item {
            LabelItem::Text(t) => LabelItem::Text(t.clone()),
            LabelItem::Node(n) => LabelItem::Node(super::lower_node(cx, n, nest)?),
        });
    }

    let mut style = w.style.clone();
    let has_along = style.iter().any(|d| d.name == "along");
    let n = labels
        .iter()
        .filter(|it| matches!(it, LabelItem::Text(_)))
        .count();
    if n > 0 && !has_along {
        // Comma-shaped groups: `along` is a fraction **list** [SPEC 2].
        let fractions: Vec<Vec<Value>> = (0..n)
            .map(|i| {
                let f = (i as f64 + 1.0) / (n as f64 + 1.0);
                vec![Value::Number((f * 100.0).round() / 100.0)]
            })
            .collect();
        style.insert(
            0,
            Decl {
                name: "along".to_string(),
                groups: fractions,
                span: w.span,
            },
        );
    }
    Ok(Link {
        chain: w.chain.clone(),
        ops: w.ops.clone(),
        classes: w.classes.clone(),
        style,
        style_span: w.style_span,
        label: None,
        labels,
        span: w.span,
    })
}

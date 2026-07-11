//! Label / `along:` lowering helpers, used by the full desugar pass ([`super`]).
//! The smart label (a box's text, a group's caption, an icon's symbol) and a
//! link's auto-distributed `along:` fractions are each a small, reusable
//! transform [SPEC 3, 7, 9, 16].

use crate::ast::ChainOp;
use crate::span::Span;
use crate::syntax::ast::{Decl, Link, Node, TextNode, Value};

/// A `|caption|` node carrying a group/table's smart-label text [SPEC 3/8]: the
/// container's label lowers to this, then through the normal node path (so it
/// gains its `.lini-caption` chain and its centred text child).
pub(super) fn caption_node(label: &TextNode) -> Node {
    Node {
        id: None,
        ty: Some("caption".to_string()),
        label: Some(label.clone()),
        classes: Vec::new(),
        style: Vec::new(),
        style_span: None,
        children: Vec::new(),
        links: Vec::new(),
        span: label.span,
    }
}

/// A `|footnote|` node carrying a drawing's smart-label title [SPEC 15.8] —
/// drafting titles sit **under** the view, so the label lowers to the
/// bottom-centred caption template and `|drawing| |footnote| { … }` styles it.
pub(super) fn footnote_node(label: &TextNode) -> Node {
    Node {
        id: None,
        ty: Some("footnote".to_string()),
        label: Some(label.clone()),
        classes: Vec::new(),
        style: Vec::new(),
        style_span: None,
        children: Vec::new(),
        links: Vec::new(),
        span: label.span,
    }
}

/// A placeholder title `|footnote|` for a marker-sourced view [SPEC 15.8]: a
/// `|drawing| { of: X }` with no authored label seeds this carrying a bare
/// `of-title` marker. The letter (and doubled-or-not) come from X's kind, and
/// the scale ratio from the seat — both known only at layout, so the drawing
/// engine fills the text where it pins the title.
pub(super) fn of_footnote(span: Span) -> Node {
    Node {
        id: None,
        ty: Some("footnote".to_string()),
        label: None,
        classes: Vec::new(),
        style: vec![Decl {
            name: "of-title".to_string(),
            groups: vec![vec![Value::Ident("view".to_string())]],
            span,
        }],
        style_span: None,
        children: Vec::new(),
        links: Vec::new(),
        span,
    }
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

pub(super) fn lower_link(w: &Link) -> Link {
    let mut labels = Vec::new();
    if let Some(head) = &w.label {
        labels.push(head.clone());
    }
    labels.extend(w.labels.iter().cloned());

    let mut style = w.style.clone();
    let has_along = style.iter().any(|d| d.name == "along");
    if !labels.is_empty() && !has_along {
        let n = labels.len();
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
    Link {
        chain: w.chain.clone(),
        ops: w.ops.clone(),
        classes: w.classes.clone(),
        style,
        style_span: w.style_span,
        label: None,
        labels,
        span: w.span,
    }
}

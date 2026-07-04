//! Label / `along:` lowering helpers, used by the full desugar pass ([`super`]).
//! The smart label (a box's text, a group's caption, an icon's symbol) and a
//! link's auto-distributed `along:` fractions are each a small, reusable
//! transform [SPEC 3, 7, 9, 16].

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
        let fractions: Vec<Value> = (0..n)
            .map(|i| {
                let f = (i as f64 + 1.0) / (n as f64 + 1.0);
                Value::Number((f * 100.0).round() / 100.0)
            })
            .collect();
        style.insert(
            0,
            Decl {
                name: "along".to_string(),
                groups: vec![fractions],
                span: w.span,
            },
        );
    }
    Link {
        chain: w.chain.clone(),
        op: w.op,
        classes: w.classes.clone(),
        style,
        style_span: w.style_span,
        label: None,
        labels,
        span: w.span,
    }
}

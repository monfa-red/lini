//! Label / `along:` lowering helpers, used by the full desugar pass ([`super`]).
//! The id-as-label rule (a leaf box with no content shows its id) and a link's
//! auto-distributed `along:` fractions are each a small, reusable transform
//! (SPEC §3, §14).

use crate::syntax::ast::{Child, Decl, Link, Node, TextNode, Value};

/// The id-as-label text child for a leaf box (SPEC §3): a box that is neither an
/// `|icon|` (which consumes its text as a glyph name) nor a container (which holds
/// its children) shows its id. `None` when the node has no id or is icon/container;
/// the caller adds it only when the node has no other content.
pub(super) fn label_child_for(node: &Node, is_icon: bool, is_container: bool) -> Option<Child> {
    if is_icon || is_container {
        return None;
    }
    node.id.as_ref().map(|id| {
        Child::Text(TextNode {
            text: id.clone(),
            style: Vec::new(),
            style_span: None,
            span: node.span,
        })
    })
}

/// Make a link's auto-distributed labels explicit: prepend an `along:` list of even
/// fractions when labels are present and no `along:` was written (SPEC §14).
pub(super) fn auto_along(w: &Link) -> Link {
    let n = w.labels.len();
    let has_along = w.style.iter().any(|d| d.name == "along");
    if n == 0 || has_along {
        return w.clone();
    }
    let fractions: Vec<Value> = (0..n)
        .map(|i| {
            let f = (i as f64 + 1.0) / (n as f64 + 1.0);
            Value::Number((f * 100.0).round() / 100.0)
        })
        .collect();
    let mut style = w.style.clone();
    style.insert(
        0,
        Decl {
            name: "along".to_string(),
            groups: vec![fractions],
            span: w.span,
        },
    );
    Link {
        chain: w.chain.clone(),
        op: w.op,
        classes: w.classes.clone(),
        style,
        style_span: w.style_span,
        labels: w.labels.clone(),
        span: w.span,
    }
}

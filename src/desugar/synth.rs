//! The one synthesized-node literal. Every node desugar invents — a chrome
//! centerline, a `|caption|`, a table `|cell|`, a title-block field, a minted
//! net tag, an auto-created endpoint box — is built here and then filled with
//! only the fields it actually authors, so no producer can quietly disagree
//! with the others about what an anonymous generated node looks like.
//!
//! A generated *child* carries its parent's **tail** span (the empty span at
//! the parent's end): the body printer sorts by span, so a parent-headed span
//! would hoist the chrome above the parent's authored `[ ]`.

use crate::span::Span;
use crate::syntax::ast::{Decl, Node, TextNode};

/// A synthesized node with no type at all — for the one producer (the capsule
/// hoist) that carries the author's own `|type#id|` across.
pub(super) fn bare(span: Span) -> Node {
    Node {
        id: None,
        ty: None,
        label: None,
        classes: Vec::new(),
        style: Vec::new(),
        style_span: None,
        children: Vec::new(),
        links: Vec::new(),
        span,
    }
}

/// A synthesized node: anonymous, of type `ty`, spanned at `span`.
pub(super) fn node(ty: &str, span: Span) -> Node {
    let mut n = bare(span);
    n.ty = Some(ty.into());
    n
}

/// A synthesized node carrying a smart label — seated at the label's own span,
/// so `lini desugar` prints it where its text sits and re-parses to the same
/// order.
pub(super) fn labelled(ty: &str, label: TextNode) -> Node {
    let mut n = node(ty, label.span);
    n.label = Some(label);
    n
}

/// A synthesized node carrying `style` — the generated-chrome shape [SPEC 15.7]:
/// a typed, anonymous node whose `chrome:` marker (plus any pin / extra) is its
/// whole declaration.
pub(super) fn styled(ty: &str, style: Vec<Decl>, span: Span) -> Node {
    let mut n = node(ty, span);
    n.style = style;
    n
}
